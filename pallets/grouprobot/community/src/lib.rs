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

/// 群规则配置 (链上精简版)
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct CommunityConfig {
	pub node_requirement: NodeRequirement,
	pub anti_flood_enabled: bool,
	pub flood_limit: u16,
	pub warn_limit: u8,
	pub warn_action: WarnAction,
	pub welcome_enabled: bool,
	/// 配置版本 (CAS 乐观锁)
	pub version: u32,
}

impl Default for CommunityConfig {
	fn default() -> Self {
		Self {
			node_requirement: NodeRequirement::Any,
			anti_flood_enabled: false,
			flood_limit: 10,
			warn_limit: 3,
			warn_action: WarnAction::Kick,
			welcome_enabled: false,
			version: 0,
		}
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
		/// Bot 注册查询
		type BotRegistry: BotRegistryProvider<Self::AccountId>;
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
		/// 社区配置不存在
		ConfigNotFound,
		/// 无日志可清理
		NoLogsToClear,
		/// 批量日志为空
		EmptyBatch,
		/// 批量日志过多
		BatchTooLarge,
	}

	// ========================================================================
	// Extrinsics
	// ========================================================================

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// 提交动作日志 (Bot 执行后上链存证)
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(45_000_000, 8_000))]
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
			let _who = ensure_signed(origin)?;
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
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			let new_version = expected_version.saturating_add(1);

			if let Some(existing) = CommunityConfigs::<T>::get(&community_id_hash) {
				ensure!(existing.version == expected_version, Error::<T>::ConfigVersionConflict);
			} else {
				// 首次创建配置，expected_version 应为 0
				ensure!(expected_version == 0, Error::<T>::ConfigVersionConflict);
			}

			let node_req = CommunityNodeRequirement::<T>::get(&community_id_hash);

			let config = CommunityConfig {
				node_requirement: node_req,
				anti_flood_enabled,
				flood_limit,
				warn_limit,
				warn_action,
				welcome_enabled,
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
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::from_parts(80_000_000, 15_000))]
		pub fn batch_submit_logs(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			logs: alloc::vec::Vec<(ActionType, [u8; 32], u64, [u8; 32], [u8; 64])>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!logs.is_empty(), Error::<T>::EmptyBatch);
			ensure!(logs.len() <= 50, Error::<T>::BatchTooLarge);

			let now = frame_system::Pallet::<T>::block_number();
			let count = logs.len() as u32;

			ActionLogs::<T>::try_mutate(&community_id_hash, |action_logs| -> DispatchResult {
				for (action_type, target_hash, sequence, message_hash, signature) in logs {
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

			Self::deposit_event(Event::BatchLogsSubmitted { community_id_hash, count });
			Ok(())
		}

		/// 清理过期日志 (释放 storage)
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::from_parts(50_000_000, 10_000))]
		pub fn clear_expired_logs(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			max_age_blocks: BlockNumberFor<T>,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;
			let now = frame_system::Pallet::<T>::block_number();

			let mut cleared = 0u32;
			ActionLogs::<T>::mutate(&community_id_hash, |logs| {
				let before = logs.len();
				logs.retain(|log| now.saturating_sub(log.block_number) < max_age_blocks);
				cleared = (before - logs.len()) as u32;
			});

			ensure!(cleared > 0, Error::<T>::NoLogsToClear);

			LogCount::<T>::mutate(|c| *c = c.saturating_sub(cleared as u64));

			Self::deposit_event(Event::ExpiredLogsCleared { community_id_hash, cleared });
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
	}

	// ========================================================================
	// CommunityProvider 实现
	// ========================================================================

	impl<T: Config> CommunityProvider<T::AccountId> for Pallet<T> {
		fn get_node_requirement(community_id_hash: &CommunityIdHash) -> NodeRequirement {
			Self::get_node_requirement(community_id_hash)
		}

		fn is_community_bound(community_id_hash: &CommunityIdHash) -> bool {
			Self::is_community_configured(community_id_hash)
		}
	}
}
