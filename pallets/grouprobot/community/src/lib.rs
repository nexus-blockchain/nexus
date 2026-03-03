#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet GroupRobot Community — 社区管理 + 群规则配置 + 节点准入策略 + 动作日志
//!
//! 整合现有 `pallet-bot-group-mgmt` + `group_config.rs` + `local_processor.rs` 链上部分。
//!
//! ## 功能
//! - 社区群规则配置 (防刷屏、警告限制、欢迎消息)
//! - 节点准入策略 (Any, TeeOnly, TeePreferred, MinTee)
//! - 动作日志存证 (Ed25519 签名验证)
//! - 批量提交日志
//! - 过期日志清理

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use pallet_grouprobot_primitives::*;
use scale_info::TypeInfo;
use sp_runtime::traits::Saturating;
use sp_core::ed25519;
use sp_io::crypto::ed25519_verify;

/// 群规则配置 (链上精简版)
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct CommunityConfig {
	pub node_requirement: NodeRequirement,
	pub anti_flood_enabled: bool,
	pub flood_limit: u16,
	pub warn_limit: u8,
	pub warn_action: WarnAction,
	pub welcome_enabled: bool,
	/// 是否接受广告投放 (Free/Basic 层级自动开启)
	pub ads_enabled: bool,
	/// 社区活跃成员数 (Bot 定期更新)
	pub active_members: u32,
	/// 社区语言 (ISO 639-1, 广告定向用)
	pub language: [u8; 2],
	/// 配置版本 (CAS 乐观锁)
	pub version: u32,
}

impl Default for CommunityConfig {
	fn default() -> Self {
		Self {
			node_requirement: NodeRequirement::TeeOnly,
			anti_flood_enabled: false,
			flood_limit: 10,
			warn_limit: 3,
			warn_action: WarnAction::Kick,
			welcome_enabled: false,
			ads_enabled: false,
			active_members: 0,
			language: *b"en",
			version: 0,
		}
	}
}

/// 声誉记录 (本地社区声誉)
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ReputationRecord<T: Config> {
	/// 声誉分数 (可负)
	pub score: i64,
	/// 累计奖励次数
	pub awards: u32,
	/// 累计扣分次数
	pub deductions: u32,
	/// 最后修改区块
	pub last_updated: BlockNumberFor<T>,
}

impl<T: Config> Default for ReputationRecord<T> {
	fn default() -> Self {
		Self { score: 0, awards: 0, deductions: 0, last_updated: Default::default() }
	}
}

/// 动作日志记录
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ActionLog<T: Config> {
	pub community_id_hash: CommunityIdHash,
	pub action_type: ActionType,
	pub operator: T::AccountId,
	pub target_hash: [u8; 32],
	pub sequence: u64,
	pub message_hash: [u8; 32],
	pub signature: [u8; 64],
	pub block_number: BlockNumberFor<T>,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// 每个社区最大日志数
		#[pallet::constant]
		type MaxLogsPerCommunity: Get<u32>;
		/// 声誉变更冷却区块数 (同一操作者对同一目标)
		#[pallet::constant]
		type ReputationCooldown: Get<BlockNumberFor<Self>>;
		/// 单次声誉变更最大绝对值
		#[pallet::constant]
		type MaxReputationDelta: Get<u32>;
		/// M2-R3: 批量提交日志最大条数
		#[pallet::constant]
		type MaxBatchSize: Get<u32>;
		/// M3-R3: 每日区块数 (用于日志保留期计算, 6s/block = 14400)
		#[pallet::constant]
		type BlocksPerDay: Get<u32>;
		/// Bot 注册查询
		type BotRegistry: BotRegistryProvider<Self::AccountId>;
		/// 订阅层级查询 (tier gate)
		type Subscription: SubscriptionProvider;
	}

	/// L1-R3: Config 参数完整性校验
	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		#[cfg(feature = "std")]
		fn integrity_test() {
			assert!(T::MaxLogsPerCommunity::get() > 0, "MaxLogsPerCommunity must be > 0");
			assert!(T::MaxReputationDelta::get() > 0, "MaxReputationDelta must be > 0");
			assert!(T::MaxBatchSize::get() > 0, "MaxBatchSize must be > 0");
			assert!(T::BlocksPerDay::get() > 0, "BlocksPerDay must be > 0");
		}
	}

	// ========================================================================
	// Storage
	// ========================================================================

	/// 社区配置: community_id_hash → CommunityConfig
	#[pallet::storage]
	pub type CommunityConfigs<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, CommunityConfig>;

	/// 节点准入策略 (快速查询): community_id_hash → NodeRequirement
	#[pallet::storage]
	pub type CommunityNodeRequirement<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, NodeRequirement, ValueQuery>;

	/// 动作日志: community_id_hash → BoundedVec<ActionLog>
	#[pallet::storage]
	pub type ActionLogs<T: Config> = StorageMap<
		_, Blake2_128Concat, CommunityIdHash,
		BoundedVec<ActionLog<T>, T::MaxLogsPerCommunity>, ValueQuery,
	>;

	/// 日志总数
	#[pallet::storage]
	pub type LogCount<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// 社区内用户声誉: (community_id_hash, user_hash) → ReputationRecord
	#[pallet::storage]
	pub type MemberReputation<T: Config> = StorageDoubleMap<
		_, Blake2_128Concat, CommunityIdHash, Blake2_128Concat, [u8; 32],
		ReputationRecord<T>, ValueQuery,
	>;

	/// 全局用户声誉: user_hash → score (所有社区声誉之和)
	#[pallet::storage]
	pub type GlobalReputation<T: Config> =
		StorageMap<_, Blake2_128Concat, [u8; 32], i64, ValueQuery>;

	/// 声誉变更冷却: (operator, community_id_hash, user_hash) → last_block
	#[pallet::storage]
	pub type ReputationCooldowns<T: Config> = StorageNMap<
		_,
		(
			NMapKey<Blake2_128Concat, T::AccountId>,
			NMapKey<Blake2_128Concat, CommunityIdHash>,
			NMapKey<Blake2_128Concat, [u8; 32]>,
		),
		BlockNumberFor<T>,
		ValueQuery,
	>;

	/// M4-fix: 每个社区最后提交的日志 sequence (单调递增, None = 无日志)
	#[pallet::storage]
	pub type LastSequence<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, u64>;

	// ========================================================================
	// Events
	// ========================================================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		ActionLogSubmitted {
			community_id_hash: CommunityIdHash,
			action_type: ActionType,
			operator: T::AccountId,
			sequence: u64,
		},
		BatchLogsSubmitted {
			community_id_hash: CommunityIdHash,
			count: u32,
		},
		NodeRequirementUpdated {
			community_id_hash: CommunityIdHash,
			requirement: NodeRequirement,
		},
		CommunityConfigUpdated {
			community_id_hash: CommunityIdHash,
			version: u32,
		},
		ExpiredLogsCleared {
			community_id_hash: CommunityIdHash,
			cleared: u32,
		},
		ReputationAwarded {
			community_id_hash: CommunityIdHash,
			user_hash: [u8; 32],
			delta: i64,
			new_score: i64,
			operator: T::AccountId,
		},
		ReputationDeducted {
			community_id_hash: CommunityIdHash,
			user_hash: [u8; 32],
			delta: i64,
			new_score: i64,
			operator: T::AccountId,
		},
		ReputationReset {
			community_id_hash: CommunityIdHash,
			user_hash: [u8; 32],
			old_score: i64,
		},
		ActiveMembersUpdated {
			community_id_hash: CommunityIdHash,
			active_members: u32,
		},
		/// M2-R2: 过期冷却条目已清理
		CooldownCleaned {
			community_id_hash: CommunityIdHash,
			user_hash: [u8; 32],
			operator: T::AccountId,
		},
	}

	// ========================================================================
	// Errors
	// ========================================================================

	#[pallet::error]
	pub enum Error<T> {
		/// 日志数量已满
		LogsFull,
		/// 节点准入策略相同
		SameNodeRequirement,
		/// 配置版本冲突 (CAS)
		ConfigVersionConflict,
		/// 无日志可清理
		NoLogsToClear,
		/// 批量日志为空
		EmptyBatch,
		/// 批量日志过多
		BatchTooLarge,
		/// 声誉变更冷却中
		ReputationOnCooldown,
		/// 声誉变更值过大
		ReputationDeltaTooLarge,
		/// 清理日志的最大年龄不能为零
		InvalidMaxAge,
		/// 声誉变更值为零
		ReputationDeltaZero,
		/// 社区配置不存在 (需先 update_community_config)
		CommunityNotFound,
		/// Free 层级不允许使用此功能
		FreeTierNotAllowed,
		/// 日志未超过层级保留期限
		RetentionPeriodNotExpired,
		/// P2-fix: 签名验证失败 (Ed25519 签名无效或 Bot public_key 不匹配)
		InvalidSignature,
		/// P2-fix: Bot 未注册或 public_key 不可用
		BotPublicKeyNotFound,
		/// P4-fix: 调用者不是该社区绑定 Bot 的 owner
		NotBotOwner,
		/// M2-fix: 语言代码无效 (须为 ISO 639-1 ASCII 小写字母)
		InvalidLanguageCode,
		/// M4-fix: 日志 sequence 必须严格递增
		SequenceNotMonotonic,
		/// M1-R2: Bot 未激活（停用/banned）
		BotNotActive,
		/// M2-R2: 冷却条目尚未过期
		CooldownNotExpired,
		/// M1-R3: 冷却条目不存在
		CooldownNotFound,
	}

	// ========================================================================
	// Extrinsics
	// ========================================================================

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// 提交动作日志 (Bot 执行后上链存证)
		///
		/// P2-fix: 链上验证 Ed25519 签名,确保日志由持有 Bot private key 的 TEE 节点产生。
		/// 签名消息格式: community_id_hash(32) || action_type(u8) || target_hash(32) || sequence(u64 LE) || message_hash(32)
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(55_000_000, 8_000))]
		pub fn submit_action_log(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			action_type: ActionType,
			target_hash: [u8; 32],
			sequence: u64,
			message_hash: [u8; 32],
			signature: [u8; 64],
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			// M3-fix: 仅 Bot owner 可提交日志
			// M1-R2: 同时检查 Bot 是否激活
			Self::ensure_active_bot_owner(&who, &community_id_hash)?;

			ensure!(
				T::Subscription::effective_tier(&community_id_hash).is_paid(),
				Error::<T>::FreeTierNotAllowed
			);

			// M4-fix: sequence 必须严格递增
			Self::ensure_sequence_monotonic(&community_id_hash, sequence)?;

			Self::verify_action_log_signature(
				&community_id_hash,
				&action_type,
				&target_hash,
				sequence,
				&message_hash,
				&signature,
			)?;

			let now = frame_system::Pallet::<T>::block_number();

			let log = ActionLog::<T> {
				community_id_hash,
				action_type: action_type.clone(),
				operator: who.clone(),
				target_hash,
				sequence,
				message_hash,
				signature,
				block_number: now,
			};

			ActionLogs::<T>::try_mutate(&community_id_hash, |logs| -> DispatchResult {
				logs.try_push(log).map_err(|_| Error::<T>::LogsFull)?;
				Ok(())
			})?;

			LogCount::<T>::mutate(|c| *c = c.saturating_add(1));
			LastSequence::<T>::insert(&community_id_hash, sequence);

			Self::deposit_event(Event::ActionLogSubmitted {
				community_id_hash,
				action_type,
				operator: who,
				sequence,
			});
			Ok(())
		}

		/// 设置社区节点准入策略
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn set_node_requirement(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			requirement: NodeRequirement,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_bot_owner(&who, &community_id_hash)?;
			let current = CommunityNodeRequirement::<T>::get(&community_id_hash);
			ensure!(current != requirement, Error::<T>::SameNodeRequirement);

			CommunityNodeRequirement::<T>::insert(&community_id_hash, requirement.clone());

			// 同步到 CommunityConfig
			CommunityConfigs::<T>::mutate(&community_id_hash, |maybe_config| {
				if let Some(config) = maybe_config {
					config.node_requirement = requirement.clone();
				}
			});

			Self::deposit_event(Event::NodeRequirementUpdated { community_id_hash, requirement });
			Ok(())
		}

		/// 更新社区群规则配置 (CAS 乐观锁)
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn update_community_config(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			expected_version: u32,
			anti_flood_enabled: bool,
			flood_limit: u16,
			warn_limit: u8,
			warn_action: WarnAction,
			welcome_enabled: bool,
			ads_enabled: bool,
			language: [u8; 2],
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_bot_owner(&who, &community_id_hash)?;

			// M2-fix: language 须为 ASCII 小写字母 (ISO 639-1)
			ensure!(
				language[0].is_ascii_lowercase() && language[1].is_ascii_lowercase(),
				Error::<T>::InvalidLanguageCode
			);

			let new_version = expected_version.saturating_add(1);

			// M1-fix: 单次读取 CommunityConfigs，缓存 existing 用于版本检查和 active_members
			let existing = CommunityConfigs::<T>::get(&community_id_hash);
			if let Some(ref ex) = existing {
				ensure!(ex.version == expected_version, Error::<T>::ConfigVersionConflict);
			} else {
				// 首次创建配置，expected_version 应为 0
				ensure!(expected_version == 0, Error::<T>::ConfigVersionConflict);
			}

			let node_req = CommunityNodeRequirement::<T>::get(&community_id_hash);
			// 保留已有 active_members (由 Bot 单独更新, 不由 config 覆盖)
			let active_members = existing.map(|c| c.active_members).unwrap_or(0);

			let config = CommunityConfig {
				node_requirement: node_req,
				anti_flood_enabled,
				flood_limit,
				warn_limit,
				warn_action,
				welcome_enabled,
				ads_enabled,
				active_members,
				language,
				version: new_version,
			};

			CommunityConfigs::<T>::insert(&community_id_hash, config);

			Self::deposit_event(Event::CommunityConfigUpdated {
				community_id_hash,
				version: new_version,
			});
			Ok(())
		}

		/// 批量提交动作日志
		///
		/// P2-fix: 每条日志独立验证 Ed25519 签名。
		/// H1-fix: weight 按日志数量线性缩放。
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::from_parts(
			30_000_000u64.saturating_add(55_000_000u64.saturating_mul(logs.len() as u64)),
			5_000u64.saturating_add(500u64.saturating_mul(logs.len() as u64)),
		))]
		pub fn batch_submit_logs(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			logs: alloc::vec::Vec<(ActionType, [u8; 32], u64, [u8; 32], [u8; 64])>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			// M3-fix: 仅 Bot owner 可提交日志
			// M1-R2: 同时检查 Bot 是否激活
			Self::ensure_active_bot_owner(&who, &community_id_hash)?;

			ensure!(
				T::Subscription::effective_tier(&community_id_hash).is_paid(),
				Error::<T>::FreeTierNotAllowed
			);

			ensure!(!logs.is_empty(), Error::<T>::EmptyBatch);
			// M2-R3: 使用可配置的 MaxBatchSize 替代硬编码 50
			ensure!(logs.len() <= T::MaxBatchSize::get() as usize, Error::<T>::BatchTooLarge);

			// M4-fix: 批量日志内 sequence 须严格递增，且首条须大于链上最后 sequence
			let mut prev_seq = LastSequence::<T>::get(&community_id_hash);
			for (_, _, sequence, _, _) in &logs {
				if let Some(last) = prev_seq {
					ensure!(*sequence > last, Error::<T>::SequenceNotMonotonic);
				}
				prev_seq = Some(*sequence);
			}

			for (action_type, target_hash, sequence, message_hash, signature) in &logs {
				Self::verify_action_log_signature(
					&community_id_hash,
					action_type,
					target_hash,
					*sequence,
					message_hash,
					signature,
				)?;
			}

			let now = frame_system::Pallet::<T>::block_number();
			let count = logs.len() as u32;

			let mut last_seq = 0u64;
			ActionLogs::<T>::try_mutate(&community_id_hash, |action_logs| -> DispatchResult {
				for (action_type, target_hash, sequence, message_hash, signature) in logs {
					last_seq = sequence;
					let log = ActionLog::<T> {
						community_id_hash,
						action_type,
						operator: who.clone(),
						target_hash,
						sequence,
						message_hash,
						signature,
						block_number: now,
					};
					action_logs.try_push(log).map_err(|_| Error::<T>::LogsFull)?;
				}
				Ok(())
			})?;

			LogCount::<T>::mutate(|c| *c = c.saturating_add(count as u64));
			LastSequence::<T>::insert(&community_id_hash, last_seq);

			Self::deposit_event(Event::BatchLogsSubmitted { community_id_hash, count });
			Ok(())
		}

		/// 奖励声誉
		///
		/// P4-fix: 仅该社区绑定的 Bot owner 可操作。
		/// L1-R2: 委托到 do_modify_reputation 共享 helper。
		#[pallet::call_index(5)]
		#[pallet::weight(Weight::from_parts(35_000_000, 6_000))]
		pub fn award_reputation(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			user_hash: [u8; 32],
			delta: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::do_modify_reputation(who, community_id_hash, user_hash, delta, true)
		}

		/// 扣除声誉
		///
		/// P4-fix: 仅该社区绑定的 Bot owner 可操作。
		/// L1-R2: 委托到 do_modify_reputation 共享 helper。
		#[pallet::call_index(6)]
		#[pallet::weight(Weight::from_parts(35_000_000, 6_000))]
		pub fn deduct_reputation(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			user_hash: [u8; 32],
			delta: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::do_modify_reputation(who, community_id_hash, user_hash, delta, false)
		}

		/// 重置用户声誉 (管理员)
		///
		/// P4-fix: 仅该社区绑定的 Bot owner 可操作。
		#[pallet::call_index(7)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn reset_reputation(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			user_hash: [u8; 32],
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;
			// M1-R2: Bot 必须激活
			Self::ensure_active_bot_owner(&_who, &community_id_hash)?;

			let old_score = MemberReputation::<T>::get(&community_id_hash, &user_hash).score;
			MemberReputation::<T>::remove(&community_id_hash, &user_hash);
			GlobalReputation::<T>::mutate(&user_hash, |g| *g = g.saturating_sub(old_score));

			Self::deposit_event(Event::ReputationReset {
				community_id_hash, user_hash, old_score,
			});
			Ok(())
		}

		/// Bot 更新社区活跃成员数 (供广告 CPM 计费使用)
		///
		/// active_members 由 Bot 统计 (7天内有效发言人数), 独立于 update_community_config。
		#[pallet::call_index(8)]
		#[pallet::weight(Weight::from_parts(20_000_000, 4_000))]
		pub fn update_active_members(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			active_members: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			// M1-R2: Bot 必须激活
			Self::ensure_active_bot_owner(&who, &community_id_hash)?;

			CommunityConfigs::<T>::try_mutate(&community_id_hash, |maybe_config| {
				let config = maybe_config.as_mut().ok_or(Error::<T>::CommunityNotFound)?;
				config.active_members = active_members;
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::ActiveMembersUpdated {
				community_id_hash,
				active_members,
			});
			Ok(())
		}

		/// 清理过期日志 (释放 storage)
		///
		/// CM1-fix: max_age_blocks 不能为 0, 防止任意用户一次擦除全部日志。
		/// 使用 `<=` 比较确保恰好等于 max_age 的日志不会被误删。
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::from_parts(50_000_000, 10_000))]
		pub fn clear_expired_logs(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			max_age_blocks: BlockNumberFor<T>,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;
			// CM1-fix: 防止 max_age_blocks=0 擦除所有日志
			ensure!(max_age_blocks > BlockNumberFor::<T>::default(), Error::<T>::InvalidMaxAge);

			// Tier gate: 强制层级日志保留期下限 (6s/block → 14400 blocks/day)
			let gate = T::Subscription::effective_feature_gate(&community_id_hash);
			if gate.log_retention_days > 0 {
				// M3-R3: 使用可配置的 BlocksPerDay 替代硬编码 14_400
				let min_retention: u32 = (gate.log_retention_days as u32).saturating_mul(T::BlocksPerDay::get());
				let min_retention_blocks: BlockNumberFor<T> = min_retention.into();
				ensure!(max_age_blocks >= min_retention_blocks, Error::<T>::RetentionPeriodNotExpired);
			} else {
				// Enterprise (log_retention_days=0): 日志永久保留, 不允许清理
				return Err(Error::<T>::RetentionPeriodNotExpired.into());
			}

			let now = frame_system::Pallet::<T>::block_number();

			let mut cleared = 0u32;
			ActionLogs::<T>::mutate(&community_id_hash, |logs| {
				let before = logs.len();
				logs.retain(|log| now.saturating_sub(log.block_number) <= max_age_blocks);
				cleared = (before - logs.len()) as u32;
			});

			ensure!(cleared > 0, Error::<T>::NoLogsToClear);

			LogCount::<T>::mutate(|c| *c = c.saturating_sub(cleared as u64));

			Self::deposit_event(Event::ExpiredLogsCleared { community_id_hash, cleared });
			Ok(())
		}

		/// M2-R2: 清理已过期的冷却条目 (释放 storage)
		///
		/// 任何人可调用。检查指定 (operator, community, user) 的冷却是否已过期,
		/// 若已过期则删除该条目。
		#[pallet::call_index(9)]
		#[pallet::weight(Weight::from_parts(15_000_000, 4_000))]
		pub fn cleanup_expired_cooldowns(
			origin: OriginFor<T>,
			operator: T::AccountId,
			community_id_hash: CommunityIdHash,
			user_hash: [u8; 32],
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			let last = ReputationCooldowns::<T>::get((&operator, &community_id_hash, &user_hash));
			// M1-R3: 使用专用错误码替代复用 NoLogsToClear
			ensure!(last > BlockNumberFor::<T>::default(), Error::<T>::CooldownNotFound);

			let now = frame_system::Pallet::<T>::block_number();
			ensure!(
				now.saturating_sub(last) >= T::ReputationCooldown::get(),
				Error::<T>::CooldownNotExpired
			);

			ReputationCooldowns::<T>::remove((&operator, &community_id_hash, &user_hash));

			Self::deposit_event(Event::CooldownCleaned {
				community_id_hash,
				user_hash,
				operator,
			});
			Ok(())
		}
	}

	// ========================================================================
	// Helper Functions
	// ========================================================================

	impl<T: Config> Pallet<T> {
		/// 获取社区节点准入策略
		pub fn get_node_requirement(community_id_hash: &CommunityIdHash) -> NodeRequirement {
			CommunityNodeRequirement::<T>::get(community_id_hash)
		}

		/// 社区是否已绑定 (有配置)
		pub fn is_community_configured(community_id_hash: &CommunityIdHash) -> bool {
			CommunityConfigs::<T>::contains_key(community_id_hash)
		}

		/// 获取社区日志数
		pub fn log_count_for(community_id_hash: &CommunityIdHash) -> u32 {
			ActionLogs::<T>::get(community_id_hash).len() as u32
		}

		/// P2-fix: 验证动作日志的 Ed25519 签名
		///
		/// 通过 BotRegistryProvider 获取该社区绑定 Bot 的 public_key,
		/// 重建签名消息并验证 Ed25519 签名。
		/// 签名消息 = community_id_hash(32) || action_type_encoded || target_hash(32) || sequence(u64 LE) || message_hash(32)
		fn verify_action_log_signature(
			community_id_hash: &CommunityIdHash,
			action_type: &ActionType,
			target_hash: &[u8; 32],
			sequence: u64,
			message_hash: &[u8; 32],
			signature: &[u8; 64],
		) -> DispatchResult {
			let pk_bytes = T::BotRegistry::bot_public_key(community_id_hash)
				.ok_or(Error::<T>::BotPublicKeyNotFound)?;

			let mut msg = alloc::vec::Vec::with_capacity(32 + 4 + 32 + 8 + 32);
			msg.extend_from_slice(community_id_hash);
			msg.extend_from_slice(&action_type.encode());
			msg.extend_from_slice(target_hash);
			msg.extend_from_slice(&sequence.to_le_bytes());
			msg.extend_from_slice(message_hash);

			let ed_sig = ed25519::Signature::from_raw(*signature);
			let ed_pk = ed25519::Public::from_raw(pk_bytes);

			ensure!(ed25519_verify(&ed_sig, &msg, &ed_pk), Error::<T>::InvalidSignature);
			Ok(())
		}

		/// P4-fix: 检查调用者是否为该社区绑定 Bot 的 owner
		fn ensure_bot_owner(
			who: &T::AccountId,
			community_id_hash: &CommunityIdHash,
		) -> DispatchResult {
			let owner = T::BotRegistry::bot_owner(community_id_hash)
				.ok_or(Error::<T>::NotBotOwner)?;
			ensure!(*who == owner, Error::<T>::NotBotOwner);
			Ok(())
		}

		/// L1-R2: award/deduct 共享 helper
		fn do_modify_reputation(
			who: T::AccountId,
			community_id_hash: CommunityIdHash,
			user_hash: [u8; 32],
			delta: u32,
			is_award: bool,
		) -> DispatchResult {
			// M1-R2: Bot 必须激活
			Self::ensure_active_bot_owner(&who, &community_id_hash)?;
			ensure!(delta > 0, Error::<T>::ReputationDeltaZero);
			ensure!(delta <= T::MaxReputationDelta::get(), Error::<T>::ReputationDeltaTooLarge);
			Self::check_cooldown(&who, &community_id_hash, &user_hash)?;

			let signed_delta = delta as i64;
			let new_score = MemberReputation::<T>::mutate(
				&community_id_hash, &user_hash, |rec| {
					if is_award {
						rec.score = rec.score.saturating_add(signed_delta);
						rec.awards = rec.awards.saturating_add(1);
					} else {
						rec.score = rec.score.saturating_sub(signed_delta);
						rec.deductions = rec.deductions.saturating_add(1);
					}
					rec.last_updated = frame_system::Pallet::<T>::block_number();
					rec.score
				},
			);

			if is_award {
				GlobalReputation::<T>::mutate(&user_hash, |g| *g = g.saturating_add(signed_delta));
			} else {
				GlobalReputation::<T>::mutate(&user_hash, |g| *g = g.saturating_sub(signed_delta));
			}
			Self::set_cooldown(&who, &community_id_hash, &user_hash);

			if is_award {
				Self::deposit_event(Event::ReputationAwarded {
					community_id_hash, user_hash, delta: signed_delta, new_score, operator: who,
				});
			} else {
				Self::deposit_event(Event::ReputationDeducted {
					community_id_hash, user_hash, delta: signed_delta, new_score, operator: who,
				});
			}
			Ok(())
		}

		/// M1-R2: ensure_bot_owner + is_bot_active 组合检查
		fn ensure_active_bot_owner(
			who: &T::AccountId,
			community_id_hash: &CommunityIdHash,
		) -> DispatchResult {
			Self::ensure_bot_owner(who, community_id_hash)?;
			ensure!(T::BotRegistry::is_bot_active(community_id_hash), Error::<T>::BotNotActive);
			Ok(())
		}

		/// 检查声誉变更冷却
		fn check_cooldown(
			operator: &T::AccountId,
			community_id_hash: &CommunityIdHash,
			user_hash: &[u8; 32],
		) -> DispatchResult {
			let last = ReputationCooldowns::<T>::get((operator, community_id_hash, user_hash));
			let now = frame_system::Pallet::<T>::block_number();
			if last > BlockNumberFor::<T>::default() {
				ensure!(
					now.saturating_sub(last) >= T::ReputationCooldown::get(),
					Error::<T>::ReputationOnCooldown
				);
			}
			Ok(())
		}

		/// 设置冷却时间戳
		fn set_cooldown(
			operator: &T::AccountId,
			community_id_hash: &CommunityIdHash,
			user_hash: &[u8; 32],
		) {
			let now = frame_system::Pallet::<T>::block_number();
			ReputationCooldowns::<T>::insert((operator, community_id_hash, user_hash), now);
		}

		/// M4-fix: 确保 sequence 严格递增
		fn ensure_sequence_monotonic(
			community_id_hash: &CommunityIdHash,
			sequence: u64,
		) -> DispatchResult {
			if let Some(last) = LastSequence::<T>::get(community_id_hash) {
				ensure!(sequence > last, Error::<T>::SequenceNotMonotonic);
			}
			Ok(())
		}
	}

}
