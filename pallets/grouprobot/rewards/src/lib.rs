#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet GroupRobot Rewards — 节点奖励累积 + 领取 + Era 奖励记录
//!
//! 从 consensus pallet 拆分而来, 负责:
//! - 节点待领取奖励 (NodePendingRewards)
//! - 节点累计已领取 (NodeTotalEarned)
//! - Era 奖励记录 (EraRewards)
//! - 实现 RewardAccruer + EraRewardDistributor trait

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
	pub inflation_mint: Balance,
	pub total_distributed: Balance,
	pub treasury_share: Balance,
	pub node_count: u32,
}

type BalanceOf<T> =
	<<T as Config>::Currency as frame_support::traits::Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::traits::{Currency, ExistenceRequirement};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type Currency: frame_support::traits::Currency<Self::AccountId>;
		/// 节点共识查询 (验证 claim 时的 operator 身份)
		type NodeConsensus: NodeConsensusProvider<Self::AccountId>;
		/// 奖励池账户 (订阅费节点份额 + 通胀铸币 存放于此, claim 时从此转出)
		type RewardPoolAccount: Get<Self::AccountId>;
		/// EraRewards 保留窗口 (仅保留最近 N 个 Era 的奖励记录)
		#[pallet::constant]
		type MaxEraHistory: Get<u64>;
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

	// ========================================================================
	// Events
	// ========================================================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		RewardsClaimed { node_id: NodeId, recipient: T::AccountId, amount: BalanceOf<T> },
		EraCompleted { era: u64, total_distributed: BalanceOf<T> },
		/// 节点奖励已累加 (ads 或 Era 分配)
		RewardAccrued { node_id: NodeId, amount: BalanceOf<T> },
		/// 节点退出时残留奖励已自动领取
		OrphanRewardsClaimed { node_id: NodeId, operator: T::AccountId, amount: BalanceOf<T> },
		/// M2-R3: 孤儿奖励领取失败 (奖励池不足, 需 Root rescue)
		OrphanRewardClaimFailed { node_id: NodeId, amount: BalanceOf<T> },
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
	}

	// ========================================================================
	// Extrinsics
	// ========================================================================

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// 领取节点奖励
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn claim_rewards(origin: OriginFor<T>, node_id: NodeId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let operator = T::NodeConsensus::node_operator(&node_id)
				.ok_or(Error::<T>::NodeNotFound)?;
			ensure!(operator == who, Error::<T>::NotOperator);

			Self::do_claim_rewards(&node_id, &who)
		}

		/// M2-R2: Root 救援滞留奖励 (节点已退出, orphan claim 失败后的恢复手段)
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn rescue_stranded_rewards(
			origin: OriginFor<T>,
			node_id: NodeId,
			recipient: T::AccountId,
		) -> DispatchResult {
			ensure_root(origin)?;
			// 仅允许救援已退出节点 (node_operator 返回 None)
			ensure!(
				T::NodeConsensus::node_operator(&node_id).is_none(),
				Error::<T>::NodeStillActive
			);
			Self::do_claim_rewards(&node_id, &recipient)
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

			// H2-fix: 先转账, 成功后再清除存储 (防止转账失败导致奖励丢失)
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

			Self::deposit_event(Event::RewardsClaimed {
				node_id: *node_id,
				recipient: recipient.clone(),
				amount: pending,
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
		pub fn distribute_and_record_era(
			era: u64,
			total_pool: BalanceOf<T>,
			subscription_income: BalanceOf<T>,
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

			// C1-fix: 铸币通胀部分到奖励池 (订阅费节点份额已由 subscription pallet 转入)
			if !inflation.is_zero() {
				let reward_pool = T::RewardPoolAccount::get();
				// L2-fix: deposit_creating 返回 PositiveImbalance, drop 时自增 TotalIssuance
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
						NodePendingRewards::<T>::mutate(node_id, |pending| {
							*pending = pending.saturating_add(reward);
						});
						total_distributed = total_distributed.saturating_add(reward);
						// M1-R2: 逐节点事件, 与 accrue_node_reward (ads 路径) 保持一致
						Self::deposit_event(Event::RewardAccrued {
							node_id: *node_id,
							amount: reward,
						});
					}
				}
			}

			// 记录 Era 信息
			let era_info = EraRewardInfo {
				subscription_income,
				inflation_mint: inflation,
				total_distributed,
				treasury_share,
				node_count,
			};
			EraRewards::<T>::insert(era, era_info);

			Self::deposit_event(Event::EraCompleted { era, total_distributed });

			total_distributed
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
	}

	// ========================================================================
	// RewardAccruer 实现
	// ========================================================================

	impl<T: Config> RewardAccruer for Pallet<T> {
		fn accrue_node_reward(node_id: &NodeId, amount: u128) {
			let balance: BalanceOf<T> = amount.unique_saturated_into();
			if !balance.is_zero() {
				NodePendingRewards::<T>::mutate(node_id, |pending| {
					*pending = pending.saturating_add(balance);
				});
				// M1-fix: 发出事件以提供链上审计轨迹
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
			inflation: u128,
			treasury_share: u128,
			node_weights: &[(NodeId, u128)],
			node_count: u32,
		) -> u128 {
			let pool_bal: BalanceOf<T> = total_pool.unique_saturated_into();
			let income_bal: BalanceOf<T> = subscription_income.unique_saturated_into();
			let inflation_bal: BalanceOf<T> = inflation.unique_saturated_into();
			let treasury_bal: BalanceOf<T> = treasury_share.unique_saturated_into();

			let distributed = Self::distribute_and_record_era(
				era, pool_bal, income_bal, inflation_bal, treasury_bal, node_weights, node_count,
			);
			distributed.unique_saturated_into()
		}

		fn prune_old_eras(current_era: u64) {
			Self::prune_old_era_rewards(current_era);
		}
	}
}
