#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet GroupRobot Rewards — 节点奖励累积 + 领取 + Era 奖励记录
//!
//! 从 consensus pallet 拆分而来, 负责:
//! - 节点待领取奖励 (NodePendingRewards)
//! - 节点累计已领取 (NodeTotalEarned)
//! - Era 奖励记录 (EraRewards)
//! - Owner 分成 (RewardSplit + OwnerPendingRewards)
//! - 实现 RewardAccruer + EraRewardDistributor trait

extern crate alloc;

pub mod weights;
pub use weights::{WeightInfo, SubstrateWeight};

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

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
use sp_runtime::traits::{Saturating, UniqueSaturatedInto, Zero};

/// Era 奖励信息
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EraRewardInfo<Balance> {
	pub subscription_income: Balance,
	pub ads_income: Balance,
	pub inflation_mint: Balance,
	pub total_distributed: Balance,
	pub treasury_share: Balance,
	pub node_count: u32,
}

/// 节点奖励摘要 (查询用)
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub struct NodeRewardSummary<Balance> {
	pub pending: Balance,
	pub total_earned: Balance,
}

type BalanceOf<T> =
	<<T as Config>::Currency as frame_support::traits::Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::traits::{Currency, ExistenceRequirement};

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
		type Currency: frame_support::traits::Currency<Self::AccountId>;
		/// 节点共识查询 (验证 claim 时的 operator 身份)
		type NodeConsensus: NodeConsensusProvider<Self::AccountId>;
		/// Bot 注册查询 (验证 bot_owner 身份)
		type BotRegistry: BotRegistryProvider<Self::AccountId>;
		/// 奖励池账户 (订阅费节点份额 + 通胀铸币 存放于此, claim 时从此转出)
		type RewardPoolAccount: Get<Self::AccountId>;
		/// EraRewards 保留窗口 (仅保留最近 N 个 Era 的奖励记录)
		#[pallet::constant]
		type MaxEraHistory: Get<u64>;
		/// 单次 batch_claim 最大节点数
		#[pallet::constant]
		type MaxBatchClaim: Get<u32>;
		/// 权重信息（由 benchmark 生成，或使用默认占位值）
		type WeightInfo: crate::weights::WeightInfo;
	}

	// ========================================================================
	// Storage
	// ========================================================================

	/// 节点待领取奖励
	#[pallet::storage]
	pub type NodePendingRewards<T: Config> =
		StorageMap<_, Blake2_128Concat, NodeId, BalanceOf<T>, ValueQuery>;

	/// 节点累计已领取
	#[pallet::storage]
	pub type NodeTotalEarned<T: Config> =
		StorageMap<_, Blake2_128Concat, NodeId, BalanceOf<T>, ValueQuery>;

	/// Era 奖励记录: era → EraRewardInfo
	#[pallet::storage]
	pub type EraRewards<T: Config> =
		StorageMap<_, Blake2_128Concat, u64, EraRewardInfo<BalanceOf<T>>>;

	/// Era 奖励清理游标 (已清理到的 Era 编号)
	#[pallet::storage]
	pub type EraCleanupCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// 自定义收款地址: node_id → recipient
	#[pallet::storage]
	pub type RewardRecipient<T: Config> =
		StorageMap<_, Blake2_128Concat, NodeId, T::AccountId>;

	/// Owner 分成比例: bot_id_hash → bps (0-10000, e.g. 2000 = 20%)
	#[pallet::storage]
	pub type RewardSplitBps<T: Config> =
		StorageMap<_, Blake2_128Concat, BotIdHash, u32, ValueQuery>;

	/// Owner 待领取奖励: bot_id_hash → Balance
	#[pallet::storage]
	pub type OwnerPendingRewards<T: Config> =
		StorageMap<_, Blake2_128Concat, BotIdHash, BalanceOf<T>, ValueQuery>;

	/// Owner 累计已领取: bot_id_hash → Balance
	#[pallet::storage]
	pub type OwnerTotalEarned<T: Config> =
		StorageMap<_, Blake2_128Concat, BotIdHash, BalanceOf<T>, ValueQuery>;

	/// 奖励分配暂停标志
	#[pallet::storage]
	pub type DistributionPaused<T: Config> = StorageValue<_, bool, ValueQuery>;

	// ========================================================================
	// Events
	// ========================================================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		RewardsClaimed { node_id: NodeId, recipient: T::AccountId, amount: BalanceOf<T>, total_earned_after: BalanceOf<T> },
		EraCompleted { era: u64, total_distributed: BalanceOf<T>, era_info: EraRewardInfo<BalanceOf<T>> },
		/// 节点奖励已累加 (ads 或 Era 分配)
		RewardAccrued { node_id: NodeId, amount: BalanceOf<T> },
		/// 节点退出时残留奖励已自动领取
		OrphanRewardsClaimed { node_id: NodeId, operator: T::AccountId, amount: BalanceOf<T> },
		/// M2-R3: 孤儿奖励领取失败 (奖励池不足, 需 Root rescue)
		OrphanRewardClaimFailed { node_id: NodeId, amount: BalanceOf<T> },
		/// 批量领取完成
		BatchRewardsClaimed { operator: T::AccountId, total_amount: BalanceOf<T>, node_count: u32 },
		/// 自定义收款地址已设置
		RewardRecipientSet { node_id: NodeId, recipient: T::AccountId },
		/// 自定义收款地址已清除
		RewardRecipientCleared { node_id: NodeId },
		/// 待领取奖励被治理 Slash
		PendingRewardsSlashed { node_id: NodeId, amount: BalanceOf<T> },
		/// Owner 分成比例已设置
		RewardSplitSet { bot_id_hash: BotIdHash, owner_bps: u32 },
		/// Owner 奖励已领取
		OwnerRewardsClaimed { bot_id_hash: BotIdHash, owner: T::AccountId, amount: BalanceOf<T> },
		/// 奖励分配已暂停
		DistributionPausedEvent,
		/// 奖励分配已恢复
		DistributionResumedEvent,
		/// 待领取奖励被治理强制设置
		PendingRewardsForceSet { node_id: NodeId, old_amount: BalanceOf<T>, new_amount: BalanceOf<T> },
		/// 过期 Era 记录被强制清理
		EraRewardsForcePruned { from_era: u64, to_era: u64 },
	}

	// ========================================================================
	// Errors
	// ========================================================================

	#[pallet::error]
	pub enum Error<T> {
		/// 节点不存在
		NodeNotFound,
		/// 不是节点操作者
		NotOperator,
		/// 无待领取奖励
		NoPendingRewards,
		/// 奖励池余额不足
		RewardPoolInsufficient,
		/// 节点仍活跃, 应使用 claim_rewards
		NodeStillActive,
		/// 批量领取列表为空
		EmptyBatchList,
		/// 批量领取超出上限
		TooManyNodes,
		/// Slash 金额超过待领取奖励
		SlashExceedsPending,
		/// Owner 分成比例超出范围 (0-5000, 即 0-50%)
		InvalidSplitBps,
		/// 不是 Bot 拥有者
		NotBotOwner,
		/// Owner 无待领取奖励
		NoOwnerPendingRewards,
		/// 奖励分配已暂停
		DistributionIsPaused,
		/// 奖励分配未暂停
		DistributionNotPaused,
		/// 无可清理的 Era 记录
		NothingToPrune,
	}

	// ========================================================================
	// Extrinsics
	// ========================================================================

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn integrity_test() {
			assert!(
				T::MaxEraHistory::get() > 0,
				"MaxEraHistory must be > 0"
			);
			assert!(
				T::MaxBatchClaim::get() > 0,
				"MaxBatchClaim must be > 0"
			);
		}
	}

	/// force_prune_era_rewards 单次最大清理条数
	pub const MAX_FORCE_PRUNE: u64 = 100;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// 领取节点奖励
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::claim_rewards())]
		pub fn claim_rewards(origin: OriginFor<T>, node_id: NodeId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let operator = T::NodeConsensus::node_operator(&node_id)
				.ok_or(Error::<T>::NodeNotFound)?;
			ensure!(operator == who, Error::<T>::NotOperator);

			let recipient = RewardRecipient::<T>::get(node_id).unwrap_or(who);
			Self::do_claim_rewards(&node_id, &recipient)
		}

		/// M2-R2: Root 救援滞留奖励 (节点已退出, orphan claim 失败后的恢复手段)
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::rescue_stranded_rewards())]
		pub fn rescue_stranded_rewards(
			origin: OriginFor<T>,
			node_id: NodeId,
			recipient: T::AccountId,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(
				T::NodeConsensus::node_operator(&node_id).is_none(),
				Error::<T>::NodeStillActive
			);
			Self::do_claim_rewards(&node_id, &recipient)
		}

		/// P0: 批量领取多节点奖励
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::batch_claim_rewards(node_ids.len() as u32))]
		pub fn batch_claim_rewards(
			origin: OriginFor<T>,
			node_ids: alloc::vec::Vec<NodeId>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!node_ids.is_empty(), Error::<T>::EmptyBatchList);
			ensure!(
				node_ids.len() as u32 <= T::MaxBatchClaim::get(),
				Error::<T>::TooManyNodes
			);

			let mut total_claimed = BalanceOf::<T>::zero();
			let mut claimed_count = 0u32;
			for node_id in node_ids.iter() {
				let operator = T::NodeConsensus::node_operator(node_id);
				if operator.as_ref() != Some(&who) {
					continue;
				}
				let pending = NodePendingRewards::<T>::get(node_id);
				if pending.is_zero() {
					continue;
				}
				let recipient = RewardRecipient::<T>::get(node_id).unwrap_or(who.clone());
				if Self::do_claim_rewards(node_id, &recipient).is_ok() {
					total_claimed = total_claimed.saturating_add(pending);
					claimed_count += 1;
				}
			}
			ensure!(!total_claimed.is_zero(), Error::<T>::NoPendingRewards);
			Self::deposit_event(Event::BatchRewardsClaimed {
				operator: who,
				total_amount: total_claimed,
				node_count: claimed_count,
			});
			Ok(())
		}

		/// P0: 设置自定义收款地址
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::set_reward_recipient())]
		pub fn set_reward_recipient(
			origin: OriginFor<T>,
			node_id: NodeId,
			recipient: Option<T::AccountId>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let operator = T::NodeConsensus::node_operator(&node_id)
				.ok_or(Error::<T>::NodeNotFound)?;
			ensure!(operator == who, Error::<T>::NotOperator);

			match recipient {
				Some(r) => {
					RewardRecipient::<T>::insert(node_id, &r);
					Self::deposit_event(Event::RewardRecipientSet { node_id, recipient: r });
				}
				None => {
					RewardRecipient::<T>::remove(node_id);
					Self::deposit_event(Event::RewardRecipientCleared { node_id });
				}
			}
			Ok(())
		}

		/// P0: Root Slash 节点待领取奖励
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::force_slash_pending_rewards())]
		pub fn force_slash_pending_rewards(
			origin: OriginFor<T>,
			node_id: NodeId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			ensure_root(origin)?;
			let pending = NodePendingRewards::<T>::get(node_id);
			ensure!(pending >= amount, Error::<T>::SlashExceedsPending);
			NodePendingRewards::<T>::mutate(node_id, |p| {
				*p = p.saturating_sub(amount);
			});
			Self::deposit_event(Event::PendingRewardsSlashed { node_id, amount });
			Ok(())
		}

		/// P1: Bot Owner 设置奖励分成比例
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::set_reward_split())]
		pub fn set_reward_split(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			owner_bps: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let owner = T::BotRegistry::bot_owner(&bot_id_hash)
				.ok_or(Error::<T>::NotBotOwner)?;
			ensure!(owner == who, Error::<T>::NotBotOwner);
			ensure!(owner_bps <= 5000, Error::<T>::InvalidSplitBps);

			RewardSplitBps::<T>::insert(bot_id_hash, owner_bps);
			Self::deposit_event(Event::RewardSplitSet { bot_id_hash, owner_bps });
			Ok(())
		}

		/// P1: Bot Owner 领取分成奖励
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::claim_owner_rewards())]
		pub fn claim_owner_rewards(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let owner = T::BotRegistry::bot_owner(&bot_id_hash)
				.ok_or(Error::<T>::NotBotOwner)?;
			ensure!(owner == who, Error::<T>::NotBotOwner);

			let pending = OwnerPendingRewards::<T>::get(bot_id_hash);
			ensure!(!pending.is_zero(), Error::<T>::NoOwnerPendingRewards);

			let reward_pool = T::RewardPoolAccount::get();
			T::Currency::transfer(
				&reward_pool,
				&who,
				pending,
				ExistenceRequirement::AllowDeath,
			).map_err(|_| Error::<T>::RewardPoolInsufficient)?;

			OwnerPendingRewards::<T>::remove(bot_id_hash);
			OwnerTotalEarned::<T>::mutate(bot_id_hash, |t| {
				*t = t.saturating_add(pending);
			});

			Self::deposit_event(Event::OwnerRewardsClaimed {
				bot_id_hash,
				owner: who,
				amount: pending,
			});
			Ok(())
		}

		/// P1: Root 暂停奖励分配
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::pause_distribution())]
		pub fn pause_distribution(origin: OriginFor<T>) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(!DistributionPaused::<T>::get(), Error::<T>::DistributionIsPaused);
			DistributionPaused::<T>::put(true);
			Self::deposit_event(Event::DistributionPausedEvent);
			Ok(())
		}

		/// P1: Root 恢复奖励分配
		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::resume_distribution())]
		pub fn resume_distribution(origin: OriginFor<T>) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(DistributionPaused::<T>::get(), Error::<T>::DistributionNotPaused);
			DistributionPaused::<T>::put(false);
			Self::deposit_event(Event::DistributionResumedEvent);
			Ok(())
		}

		/// P1: Root 强制设置节点待领取奖励
		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::force_set_pending_rewards())]
		pub fn force_set_pending_rewards(
			origin: OriginFor<T>,
			node_id: NodeId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			ensure_root(origin)?;
			let old = NodePendingRewards::<T>::get(node_id);
			NodePendingRewards::<T>::insert(node_id, amount);
			Self::deposit_event(Event::PendingRewardsForceSet {
				node_id,
				old_amount: old,
				new_amount: amount,
			});
			Ok(())
		}

		/// P2: Root 强制清理过期 Era 记录 (每次最多 MAX_FORCE_PRUNE 条)
		#[pallet::call_index(10)]
		#[pallet::weight(T::WeightInfo::force_prune_era_rewards(
			target_era.saturating_sub(EraCleanupCursor::<T>::get()).min(MAX_FORCE_PRUNE) as u32
		))]
		pub fn force_prune_era_rewards(
			origin: OriginFor<T>,
			target_era: u64,
		) -> DispatchResult {
			ensure_root(origin)?;
			let mut cursor = EraCleanupCursor::<T>::get();
			ensure!(cursor < target_era, Error::<T>::NothingToPrune);
			let from = cursor;
			let cap = cursor.saturating_add(MAX_FORCE_PRUNE);
			let effective_target = target_era.min(cap);
			while cursor < effective_target {
				EraRewards::<T>::remove(cursor);
				cursor = cursor.saturating_add(1);
			}
			EraCleanupCursor::<T>::put(cursor);
			Self::deposit_event(Event::EraRewardsForcePruned { from_era: from, to_era: cursor });
			Ok(())
		}
	}

	// ========================================================================
	// Internal Functions
	// ========================================================================

	impl<T: Config> Pallet<T> {
		/// H2-fix: 内部领取逻辑 — 先转账后清除存储
		fn do_claim_rewards(node_id: &NodeId, recipient: &T::AccountId) -> DispatchResult {
			let pending = NodePendingRewards::<T>::get(node_id);
			ensure!(!pending.is_zero(), Error::<T>::NoPendingRewards);

			let reward_pool = T::RewardPoolAccount::get();
			T::Currency::transfer(
				&reward_pool,
				recipient,
				pending,
				ExistenceRequirement::AllowDeath,
			).map_err(|_| Error::<T>::RewardPoolInsufficient)?;

			NodePendingRewards::<T>::remove(node_id);
			NodeTotalEarned::<T>::mutate(node_id, |total| {
				*total = total.saturating_add(pending);
			});

			let total_earned_after = NodeTotalEarned::<T>::get(node_id);
			Self::deposit_event(Event::RewardsClaimed {
				node_id: *node_id,
				recipient: recipient.clone(),
				amount: pending,
				total_earned_after,
			});
			Ok(())
		}

		/// H3-fix: 节点退出时自动领取残留奖励 (best-effort, 失败不阻断退出)
		pub fn try_claim_orphan_rewards(node_id: &NodeId, operator: &T::AccountId) {
			let pending = NodePendingRewards::<T>::get(node_id);
			if pending.is_zero() {
				return;
			}
			let reward_pool = T::RewardPoolAccount::get();
			match T::Currency::transfer(
				&reward_pool,
				operator,
				pending,
				ExistenceRequirement::AllowDeath,
			) {
				Ok(_) => {
					NodePendingRewards::<T>::remove(node_id);
					NodeTotalEarned::<T>::mutate(node_id, |total| {
						*total = total.saturating_add(pending);
					});
					Self::deposit_event(Event::OrphanRewardsClaimed {
						node_id: *node_id,
						operator: operator.clone(),
						amount: pending,
					});
				}
				Err(_) => {
					log::warn!(
						"Failed to claim orphan rewards for node {:?}, amount: {:?}",
						node_id, pending
					);
					// M2-R3: 链上事件通知, 便于 Root 发现并 rescue
					Self::deposit_event(Event::OrphanRewardClaimFailed {
						node_id: *node_id,
						amount: pending,
					});
				}
			}
		}

		/// 向节点分配奖励并记录 Era 信息
		#[allow(clippy::too_many_arguments)]
		pub fn distribute_and_record_era(
			era: u64,
			total_pool: BalanceOf<T>,
			subscription_income: BalanceOf<T>,
			ads_income: BalanceOf<T>,
			inflation: BalanceOf<T>,
			treasury_share: BalanceOf<T>,
			node_weights: &[(NodeId, u128)],
			node_count: u32,
		) -> BalanceOf<T> {
			// M3-R3: 防止同一 era 被重复分配 (重复铸币 + 重复 accrue)
			if EraRewards::<T>::contains_key(era) {
				log::warn!("Era {} already distributed, skipping duplicate", era);
				return BalanceOf::<T>::zero();
			}

			// 暂停检查
			if DistributionPaused::<T>::get() {
				log::warn!("Era {} distribution skipped: paused", era);
				return BalanceOf::<T>::zero();
			}

			// C1-fix: 铸币通胀部分到奖励池 (订阅费节点份额已由 subscription pallet 转入)
			if !inflation.is_zero() {
				let reward_pool = T::RewardPoolAccount::get();
				let _imbalance = T::Currency::deposit_creating(&reward_pool, inflation);
			}

			let mut total_weight: u128 = 0;
			for (_, w) in node_weights.iter() {
				total_weight = total_weight.saturating_add(*w);
			}

			let mut total_distributed = BalanceOf::<T>::zero();
			if total_weight > 0 {
				for (node_id, w) in node_weights.iter() {
					let pool_u128: u128 = total_pool.unique_saturated_into();
					let reward_u128 = pool_u128.saturating_mul(*w) / total_weight;
					let reward: BalanceOf<T> = reward_u128.unique_saturated_into();
					if !reward.is_zero() {
						Self::accrue_with_split(node_id, reward);
						total_distributed = total_distributed.saturating_add(reward);
						Self::deposit_event(Event::RewardAccrued {
							node_id: *node_id,
							amount: reward,
						});
					}
				}
			}

			let era_info = EraRewardInfo {
				subscription_income,
				ads_income,
				inflation_mint: inflation,
				total_distributed,
				treasury_share,
				node_count,
			};
			EraRewards::<T>::insert(era, &era_info);

			Self::deposit_event(Event::EraCompleted { era, total_distributed, era_info });

			total_distributed
		}

		/// 带 Owner 分成的奖励累加
		fn accrue_with_split(node_id: &NodeId, amount: BalanceOf<T>) {
			let split = NodeBotSplitBinding::<T>::get(node_id)
				.and_then(|bot_hash| {
					let bps = RewardSplitBps::<T>::get(bot_hash);
					if bps > 0 { Some((bot_hash, bps)) } else { None }
				});

			if let Some((bot_hash, bps)) = split {
				let amount_u128: u128 = amount.unique_saturated_into();
				let owner_share_u128 = amount_u128.saturating_mul(bps as u128) / 10_000;
				let owner_share: BalanceOf<T> = owner_share_u128.unique_saturated_into();
				let operator_share = amount.saturating_sub(owner_share);

				if !owner_share.is_zero() {
					OwnerPendingRewards::<T>::mutate(bot_hash, |p| {
						*p = p.saturating_add(owner_share);
					});
				}
				if !operator_share.is_zero() {
					NodePendingRewards::<T>::mutate(node_id, |p| {
						*p = p.saturating_add(operator_share);
					});
				}
			} else {
				NodePendingRewards::<T>::mutate(node_id, |pending| {
					*pending = pending.saturating_add(amount);
				});
			}
		}

		/// H1-fix: 清理过期 EraRewards (每次最多清理 MAX_PRUNE_PER_CALL 条)
		pub fn prune_old_era_rewards(current_era: u64) {
			const MAX_PRUNE_PER_CALL: u64 = 10;
			let max_history = T::MaxEraHistory::get();
			if current_era <= max_history {
				return;
			}
			let oldest_to_keep = current_era.saturating_sub(max_history);
			let mut cursor = EraCleanupCursor::<T>::get();
			let mut pruned = 0u64;
			while cursor < oldest_to_keep && pruned < MAX_PRUNE_PER_CALL {
				EraRewards::<T>::remove(cursor);
				cursor = cursor.saturating_add(1);
				pruned += 1;
			}
			EraCleanupCursor::<T>::put(cursor);
		}

		// ====================================================================
		// 公共查询方法
		// ====================================================================

		/// 查询节点待领取奖励
		pub fn pending_rewards(node_id: &NodeId) -> BalanceOf<T> {
			NodePendingRewards::<T>::get(node_id)
		}

		/// 查询节点奖励摘要
		pub fn node_reward_summary(node_id: &NodeId) -> NodeRewardSummary<BalanceOf<T>> {
			NodeRewardSummary {
				pending: NodePendingRewards::<T>::get(node_id),
				total_earned: NodeTotalEarned::<T>::get(node_id),
			}
		}

		/// 查询奖励池余额
		pub fn reward_pool_balance() -> BalanceOf<T> {
			let pool = T::RewardPoolAccount::get();
			T::Currency::free_balance(&pool)
		}

		/// 查询 Owner 待领取奖励
		pub fn owner_pending(bot_id_hash: &BotIdHash) -> BalanceOf<T> {
			OwnerPendingRewards::<T>::get(bot_id_hash)
		}

		/// 注册节点→Bot 分成绑定 (由 consensus pallet 在 verify_node_tee 时调用)
		pub fn bind_node_bot_split(node_id: &NodeId, bot_id_hash: &BotIdHash) {
			NodeBotSplitBinding::<T>::insert(node_id, bot_id_hash);
		}

		/// 解除节点→Bot 分成绑定
		pub fn unbind_node_bot_split(node_id: &NodeId) {
			NodeBotSplitBinding::<T>::remove(node_id);
		}
	}

	// ========================================================================
	// RewardAccruer 实现
	// ========================================================================

	impl<T: Config> RewardAccruer for Pallet<T> {
		fn accrue_node_reward(node_id: &NodeId, amount: u128) {
			let balance: BalanceOf<T> = amount.unique_saturated_into();
			if !balance.is_zero() {
				Self::accrue_with_split(node_id, balance);
				Self::deposit_event(Event::RewardAccrued {
					node_id: *node_id,
					amount: balance,
				});
			}
		}
	}

	// ========================================================================
	// OrphanRewardClaimer 实现 (H3-fix)
	// ========================================================================

	impl<T: Config> OrphanRewardClaimer<T::AccountId> for Pallet<T> {
		fn try_claim_orphan_rewards(node_id: &NodeId, operator: &T::AccountId) {
			Self::try_claim_orphan_rewards(node_id, operator);
		}
	}

	// ========================================================================
	// EraRewardDistributor 实现
	// ========================================================================

	impl<T: Config> EraRewardDistributor for Pallet<T> {
		fn distribute_and_record(
			era: u64,
			total_pool: u128,
			subscription_income: u128,
			ads_income: u128,
			inflation: u128,
			treasury_share: u128,
			node_weights: &[(NodeId, u128)],
			node_count: u32,
		) -> u128 {
			let pool_bal: BalanceOf<T> = total_pool.unique_saturated_into();
			let income_bal: BalanceOf<T> = subscription_income.unique_saturated_into();
			let ads_bal: BalanceOf<T> = ads_income.unique_saturated_into();
			let inflation_bal: BalanceOf<T> = inflation.unique_saturated_into();
			let treasury_bal: BalanceOf<T> = treasury_share.unique_saturated_into();

			let distributed = Self::distribute_and_record_era(
				era, pool_bal, income_bal, ads_bal, inflation_bal, treasury_bal, node_weights, node_count,
			);
			distributed.unique_saturated_into()
		}

		fn prune_old_eras(current_era: u64) {
			Self::prune_old_era_rewards(current_era);
		}
	}

	/// 节点→Bot 分成绑定 (用于 Era 分配时查找 owner split)
	#[pallet::storage]
	pub type NodeBotSplitBinding<T: Config> =
		StorageMap<_, Blake2_128Concat, NodeId, BotIdHash>;
}
