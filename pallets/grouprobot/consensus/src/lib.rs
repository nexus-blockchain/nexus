#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet GroupRobot Consensus — 节点质押 + TEE 加权奖励 + 消息去重 + 订阅管理
//!
//! 整合现有 `pallet-bot-consensus` 全部功能。
//!
//! ## 功能
//! - 节点注册 + 质押
//! - 节点退出 (冷却期)
//! - Equivocation 举报 + Slash
//! - 订阅管理 (subscribe/deposit/cancel/change_tier)
//! - 消息序列去重 (ProcessedSequences)
//! - TEE 加权奖励分配 (on_era_end)
//! - on_initialize: Era 边界检测

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
use sp_runtime::traits::{Saturating, UniqueSaturatedInto, Zero};

/// 节点信息
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ProjectNode<T: Config> {
	pub operator: T::AccountId,
	pub node_id: NodeId,
	pub status: NodeStatus,
	pub reputation: u32,
	pub uptime_blocks: u64,
	pub stake: BalanceOf<T>,
	pub registered_at: BlockNumberFor<T>,
	pub last_active: BlockNumberFor<T>,
	pub is_tee_node: bool,
}

/// Equivocation 证据
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct EquivocationRecord<T: Config> {
	pub node_id: NodeId,
	pub sequence: u64,
	pub msg_hash_a: [u8; 32],
	pub signature_a: [u8; 64],
	pub msg_hash_b: [u8; 32],
	pub signature_b: [u8; 64],
	pub reporter: T::AccountId,
	pub reported_at: BlockNumberFor<T>,
	pub resolved: bool,
}

/// 订阅信息
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct Subscription<T: Config> {
	pub owner: T::AccountId,
	pub bot_id_hash: BotIdHash,
	pub tier: SubscriptionTier,
	pub fee_per_era: BalanceOf<T>,
	pub started_at: BlockNumberFor<T>,
	pub paid_until_era: u64,
	pub status: SubscriptionStatus,
}

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
	use frame_support::traits::{Currency, ReservableCurrency};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type Currency: ReservableCurrency<Self::AccountId>;
		/// 最大活跃节点数
		#[pallet::constant]
		type MaxActiveNodes: Get<u32>;
		/// 最小质押额
		#[pallet::constant]
		type MinStake: Get<BalanceOf<Self>>;
		/// 退出冷却期 (区块数)
		#[pallet::constant]
		type ExitCooldownPeriod: Get<BlockNumberFor<Self>>;
		/// Era 长度 (区块数)
		#[pallet::constant]
		type EraLength: Get<BlockNumberFor<Self>>;
		/// 每 Era 通胀铸币
		#[pallet::constant]
		type InflationPerEra: Get<BalanceOf<Self>>;
		/// Slash 百分比 (e.g. 10 = 10%)
		#[pallet::constant]
		type SlashPercentage: Get<u32>;
		/// Bot 注册查询
		type BotRegistry: BotRegistryProvider<Self::AccountId>;
		/// Basic 层级每 Era 费用
		#[pallet::constant]
		type BasicFeePerEra: Get<BalanceOf<Self>>;
		/// Pro 层级每 Era 费用
		#[pallet::constant]
		type ProFeePerEra: Get<BalanceOf<Self>>;
		/// Enterprise 层级每 Era 费用
		#[pallet::constant]
		type EnterpriseFeePerEra: Get<BalanceOf<Self>>;
	}

	// ========================================================================
	// Storage
	// ========================================================================

	/// 节点表: node_id → ProjectNode
	#[pallet::storage]
	pub type Nodes<T: Config> = StorageMap<_, Blake2_128Concat, NodeId, ProjectNode<T>>;

	/// 操作者 → 节点 ID
	#[pallet::storage]
	pub type OperatorNodes<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, NodeId>;

	/// 活跃节点列表
	#[pallet::storage]
	pub type ActiveNodeList<T: Config> =
		StorageValue<_, BoundedVec<NodeId, T::MaxActiveNodes>, ValueQuery>;

	/// 退出请求: node_id → 申请区块
	#[pallet::storage]
	pub type ExitRequests<T: Config> = StorageMap<_, Blake2_128Concat, NodeId, BlockNumberFor<T>>;

	/// 消息去重: (bot_id_hash, sequence) → 处理区块
	#[pallet::storage]
	pub type ProcessedSequences<T: Config> = StorageDoubleMap<
		_, Blake2_128Concat, BotIdHash,
		Blake2_128Concat, u64,
		BlockNumberFor<T>,
	>;

	/// 订阅表: bot_id_hash → Subscription
	#[pallet::storage]
	pub type Subscriptions<T: Config> =
		StorageMap<_, Blake2_128Concat, BotIdHash, Subscription<T>>;

	/// 订阅 Escrow: bot_id_hash → 预存余额
	#[pallet::storage]
	pub type SubscriptionEscrow<T: Config> =
		StorageMap<_, Blake2_128Concat, BotIdHash, BalanceOf<T>, ValueQuery>;

	/// Equivocation 记录: (node_id, sequence) → EquivocationRecord
	#[pallet::storage]
	pub type EquivocationRecords<T: Config> = StorageDoubleMap<
		_, Blake2_128Concat, NodeId,
		Blake2_128Concat, u64,
		EquivocationRecord<T>,
	>;

	/// 当前 Era
	#[pallet::storage]
	pub type CurrentEra<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Era 起始区块
	#[pallet::storage]
	pub type EraStartBlock<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

	/// 节点待领取奖励
	#[pallet::storage]
	pub type NodePendingRewards<T: Config> =
		StorageMap<_, Blake2_128Concat, NodeId, BalanceOf<T>, ValueQuery>;

	/// 节点累计已领取
	#[pallet::storage]
	pub type NodeTotalEarned<T: Config> =
		StorageMap<_, Blake2_128Concat, NodeId, BalanceOf<T>, ValueQuery>;

	/// TEE 奖励倍数 (basis points: 10000=1.0x, 15000=1.5x)
	#[pallet::storage]
	pub type TeeRewardMultiplier<T: Config> = StorageValue<_, u32, ValueQuery>;

	/// SGX 双证明额外奖励 (basis points)
	#[pallet::storage]
	pub type SgxEnclaveBonus<T: Config> = StorageValue<_, u32, ValueQuery>;

	/// Era 奖励记录: era → EraRewardInfo
	#[pallet::storage]
	pub type EraRewards<T: Config> =
		StorageMap<_, Blake2_128Concat, u64, EraRewardInfo<BalanceOf<T>>>;

	// ========================================================================
	// Events
	// ========================================================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		NodeRegistered { node_id: NodeId, operator: T::AccountId, stake: BalanceOf<T> },
		ExitRequested { node_id: NodeId },
		ExitFinalized { node_id: NodeId, stake_returned: BalanceOf<T> },
		EquivocationReported { node_id: NodeId, reporter: T::AccountId, sequence: u64 },
		NodeSlashed { node_id: NodeId, amount: BalanceOf<T> },
		Subscribed { bot_id_hash: BotIdHash, tier: SubscriptionTier, owner: T::AccountId },
		SubscriptionDeposited { bot_id_hash: BotIdHash, amount: BalanceOf<T> },
		SubscriptionCancelled { bot_id_hash: BotIdHash },
		TierChanged { bot_id_hash: BotIdHash, old_tier: SubscriptionTier, new_tier: SubscriptionTier },
		RewardsClaimed { node_id: NodeId, amount: BalanceOf<T> },
		SequenceProcessed { bot_id_hash: BotIdHash, sequence: u64 },
		SequenceDuplicate { bot_id_hash: BotIdHash, sequence: u64 },
		NodeTeeStatusChanged { node_id: NodeId, is_tee: bool },
		EraCompleted { era: u64, total_distributed: BalanceOf<T> },
	}

	// ========================================================================
	// Errors
	// ========================================================================

	#[pallet::error]
	pub enum Error<T> {
		/// 节点已注册
		NodeAlreadyRegistered,
		/// 节点不存在
		NodeNotFound,
		/// 不是节点操作者
		NotOperator,
		/// 质押不足
		InsufficientStake,
		/// 活跃节点数已满
		MaxNodesReached,
		/// 节点非活跃状态
		NodeNotActive,
		/// 节点已在退出中
		AlreadyExiting,
		/// 冷却期未到
		CooldownNotComplete,
		/// 节点不在退出状态
		NotExiting,
		/// Bot 未注册
		BotNotRegistered,
		/// 不是 Bot 所有者
		NotBotOwner,
		/// 订阅已存在
		SubscriptionAlreadyExists,
		/// 订阅不存在
		SubscriptionNotFound,
		/// 订阅已取消
		SubscriptionAlreadyCancelled,
		/// 层级相同
		SameTier,
		/// 预存不足
		InsufficientDeposit,
		/// 无待领取奖励
		NoPendingRewards,
		/// Equivocation 已举报
		EquivocationAlreadyReported,
		/// 序列已处理
		SequenceAlreadyProcessed,
		/// TEE 状态相同
		SameTeeStatus,
	}

	// ========================================================================
	// Hooks
	// ========================================================================

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: BlockNumberFor<T>) -> Weight {
			let era_length = T::EraLength::get();
			if era_length == BlockNumberFor::<T>::default() {
				return Weight::zero();
			}

			let era_start = EraStartBlock::<T>::get();
			if era_start == BlockNumberFor::<T>::default() {
				EraStartBlock::<T>::put(n);
				return Weight::from_parts(5_000_000, 1_000);
			}

			if n.saturating_sub(era_start) >= era_length {
				Self::on_era_end(n);
				Weight::from_parts(100_000_000, 20_000)
			} else {
				Weight::zero()
			}
		}
	}

	// ========================================================================
	// Extrinsics
	// ========================================================================

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// 注册节点 + 质押
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(60_000_000, 10_000))]
		pub fn register_node(
			origin: OriginFor<T>,
			node_id: NodeId,
			stake: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!Nodes::<T>::contains_key(&node_id), Error::<T>::NodeAlreadyRegistered);
			ensure!(!OperatorNodes::<T>::contains_key(&who), Error::<T>::NodeAlreadyRegistered);
			ensure!(stake >= T::MinStake::get(), Error::<T>::InsufficientStake);

			T::Currency::reserve(&who, stake)?;

			let now = frame_system::Pallet::<T>::block_number();
			let node = ProjectNode::<T> {
				operator: who.clone(),
				node_id,
				status: NodeStatus::Active,
				reputation: 5000,
				uptime_blocks: 0,
				stake,
				registered_at: now,
				last_active: now,
				is_tee_node: false,
			};

			ActiveNodeList::<T>::try_mutate(|list| -> DispatchResult {
				list.try_push(node_id).map_err(|_| Error::<T>::MaxNodesReached)?;
				Ok(())
			})?;

			Nodes::<T>::insert(&node_id, node);
			OperatorNodes::<T>::insert(&who, node_id);

			Self::deposit_event(Event::NodeRegistered { node_id, operator: who, stake });
			Ok(())
		}

		/// 申请退出 (冷却期)
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn request_exit(origin: OriginFor<T>, node_id: NodeId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Nodes::<T>::try_mutate(&node_id, |maybe_node| -> DispatchResult {
				let node = maybe_node.as_mut().ok_or(Error::<T>::NodeNotFound)?;
				ensure!(node.operator == who, Error::<T>::NotOperator);
				ensure!(node.status == NodeStatus::Active, Error::<T>::NodeNotActive);
				ensure!(!ExitRequests::<T>::contains_key(&node_id), Error::<T>::AlreadyExiting);
				node.status = NodeStatus::Exiting;
				Ok(())
			})?;

			let now = frame_system::Pallet::<T>::block_number();
			ExitRequests::<T>::insert(&node_id, now);

			ActiveNodeList::<T>::mutate(|list| {
				list.retain(|id| id != &node_id);
			});

			Self::deposit_event(Event::ExitRequested { node_id });
			Ok(())
		}

		/// 完成退出 + 退还质押
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::from_parts(50_000_000, 8_000))]
		pub fn finalize_exit(origin: OriginFor<T>, node_id: NodeId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let node = Nodes::<T>::get(&node_id).ok_or(Error::<T>::NodeNotFound)?;
			ensure!(node.operator == who, Error::<T>::NotOperator);
			ensure!(node.status == NodeStatus::Exiting, Error::<T>::NotExiting);

			let exit_block = ExitRequests::<T>::get(&node_id).ok_or(Error::<T>::NotExiting)?;
			let now = frame_system::Pallet::<T>::block_number();
			ensure!(
				now.saturating_sub(exit_block) >= T::ExitCooldownPeriod::get(),
				Error::<T>::CooldownNotComplete
			);

			let stake = node.stake;
			T::Currency::unreserve(&who, stake);

			Nodes::<T>::remove(&node_id);
			OperatorNodes::<T>::remove(&who);
			ExitRequests::<T>::remove(&node_id);

			Self::deposit_event(Event::ExitFinalized { node_id, stake_returned: stake });
			Ok(())
		}

		/// 举报 Equivocation
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::from_parts(55_000_000, 10_000))]
		pub fn report_equivocation(
			origin: OriginFor<T>,
			node_id: NodeId,
			sequence: u64,
			msg_hash_a: [u8; 32],
			signature_a: [u8; 64],
			msg_hash_b: [u8; 32],
			signature_b: [u8; 64],
		) -> DispatchResult {
			let reporter = ensure_signed(origin)?;
			ensure!(Nodes::<T>::contains_key(&node_id), Error::<T>::NodeNotFound);
			ensure!(
				!EquivocationRecords::<T>::contains_key(&node_id, sequence),
				Error::<T>::EquivocationAlreadyReported
			);

			let now = frame_system::Pallet::<T>::block_number();
			let record = EquivocationRecord::<T> {
				node_id,
				sequence,
				msg_hash_a,
				signature_a,
				msg_hash_b,
				signature_b,
				reporter: reporter.clone(),
				reported_at: now,
				resolved: false,
			};
			EquivocationRecords::<T>::insert(&node_id, sequence, record);

			Self::deposit_event(Event::EquivocationReported { node_id, reporter, sequence });
			Ok(())
		}

		/// 执行 Slash (root)
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::from_parts(50_000_000, 8_000))]
		pub fn slash_equivocation(
			origin: OriginFor<T>,
			node_id: NodeId,
			sequence: u64,
		) -> DispatchResult {
			ensure_root(origin)?;

			EquivocationRecords::<T>::try_mutate(&node_id, sequence, |maybe_record| -> DispatchResult {
				let record = maybe_record.as_mut().ok_or(Error::<T>::NodeNotFound)?;
				record.resolved = true;
				Ok(())
			})?;

			if let Some(node) = Nodes::<T>::get(&node_id) {
				let slash_pct = T::SlashPercentage::get();
				let slash_amount = node.stake * slash_pct.into() / 100u32.into();
				let _ = T::Currency::slash_reserved(&node.operator, slash_amount);

				Nodes::<T>::mutate(&node_id, |maybe_node| {
					if let Some(n) = maybe_node {
						n.stake = n.stake.saturating_sub(slash_amount);
						n.status = NodeStatus::Suspended;
					}
				});

				ActiveNodeList::<T>::mutate(|list| {
					list.retain(|id| id != &node_id);
				});

				Self::deposit_event(Event::NodeSlashed { node_id, amount: slash_amount });
			}
			Ok(())
		}

		/// 订阅 Bot 服务
		#[pallet::call_index(5)]
		#[pallet::weight(Weight::from_parts(50_000_000, 8_000))]
		pub fn subscribe(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			tier: SubscriptionTier,
			deposit: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(T::BotRegistry::is_bot_active(&bot_id_hash), Error::<T>::BotNotRegistered);
			ensure!(
				T::BotRegistry::bot_owner(&bot_id_hash) == Some(who.clone()),
				Error::<T>::NotBotOwner
			);
			ensure!(!Subscriptions::<T>::contains_key(&bot_id_hash), Error::<T>::SubscriptionAlreadyExists);

			let fee = Self::tier_fee(&tier);
			ensure!(deposit >= fee, Error::<T>::InsufficientDeposit);

			T::Currency::reserve(&who, deposit)?;
			SubscriptionEscrow::<T>::insert(&bot_id_hash, deposit);

			let now = frame_system::Pallet::<T>::block_number();
			let current_era = CurrentEra::<T>::get();
			let sub = Subscription::<T> {
				owner: who.clone(),
				bot_id_hash,
				tier,
				fee_per_era: fee,
				started_at: now,
				paid_until_era: current_era.saturating_add(1),
				status: SubscriptionStatus::Active,
			};
			Subscriptions::<T>::insert(&bot_id_hash, sub);

			Self::deposit_event(Event::Subscribed { bot_id_hash, tier, owner: who });
			Ok(())
		}

		/// 充值订阅
		#[pallet::call_index(6)]
		#[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
		pub fn deposit_subscription(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let sub = Subscriptions::<T>::get(&bot_id_hash).ok_or(Error::<T>::SubscriptionNotFound)?;
			ensure!(sub.status != SubscriptionStatus::Cancelled, Error::<T>::SubscriptionAlreadyCancelled);

			T::Currency::reserve(&who, amount)?;
			SubscriptionEscrow::<T>::mutate(&bot_id_hash, |escrow| {
				*escrow = escrow.saturating_add(amount);
			});

			// 如果是 PastDue/Suspended，重新激活
			if sub.status == SubscriptionStatus::PastDue || sub.status == SubscriptionStatus::Suspended {
				Subscriptions::<T>::mutate(&bot_id_hash, |maybe_sub| {
					if let Some(s) = maybe_sub {
						s.status = SubscriptionStatus::Active;
					}
				});
			}

			Self::deposit_event(Event::SubscriptionDeposited { bot_id_hash, amount });
			Ok(())
		}

		/// 取消订阅
		#[pallet::call_index(7)]
		#[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
		pub fn cancel_subscription(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Subscriptions::<T>::try_mutate(&bot_id_hash, |maybe_sub| -> DispatchResult {
				let sub = maybe_sub.as_mut().ok_or(Error::<T>::SubscriptionNotFound)?;
				ensure!(sub.owner == who, Error::<T>::NotBotOwner);
				ensure!(sub.status != SubscriptionStatus::Cancelled, Error::<T>::SubscriptionAlreadyCancelled);
				sub.status = SubscriptionStatus::Cancelled;
				Ok(())
			})?;

			// 退还 escrow 余额
			let escrow = SubscriptionEscrow::<T>::take(&bot_id_hash);
			if !escrow.is_zero() {
				T::Currency::unreserve(&who, escrow);
			}

			Self::deposit_event(Event::SubscriptionCancelled { bot_id_hash });
			Ok(())
		}

		/// 变更订阅层级
		#[pallet::call_index(8)]
		#[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
		pub fn change_tier(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			new_tier: SubscriptionTier,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Subscriptions::<T>::try_mutate(&bot_id_hash, |maybe_sub| -> DispatchResult {
				let sub = maybe_sub.as_mut().ok_or(Error::<T>::SubscriptionNotFound)?;
				ensure!(sub.owner == who, Error::<T>::NotBotOwner);
				ensure!(sub.tier != new_tier, Error::<T>::SameTier);

				let old_tier = sub.tier;
				sub.tier = new_tier;
				sub.fee_per_era = Self::tier_fee(&new_tier);

				Self::deposit_event(Event::TierChanged { bot_id_hash, old_tier, new_tier });
				Ok(())
			})
		}

		/// 领取节点奖励
		#[pallet::call_index(9)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn claim_rewards(origin: OriginFor<T>, node_id: NodeId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let node = Nodes::<T>::get(&node_id).ok_or(Error::<T>::NodeNotFound)?;
			ensure!(node.operator == who, Error::<T>::NotOperator);

			let pending = NodePendingRewards::<T>::get(&node_id);
			ensure!(!pending.is_zero(), Error::<T>::NoPendingRewards);

			// 铸币给操作者
			let _ = T::Currency::deposit_creating(&who, pending);

			NodePendingRewards::<T>::remove(&node_id);
			NodeTotalEarned::<T>::mutate(&node_id, |total| {
				*total = total.saturating_add(pending);
			});

			Self::deposit_event(Event::RewardsClaimed { node_id, amount: pending });
			Ok(())
		}

		/// 标记消息序列已处理 (去重)
		#[pallet::call_index(10)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn mark_sequence_processed(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			sequence: u64,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;
			if ProcessedSequences::<T>::contains_key(&bot_id_hash, sequence) {
				Self::deposit_event(Event::SequenceDuplicate { bot_id_hash, sequence });
				return Ok(());
			}
			let now = frame_system::Pallet::<T>::block_number();
			ProcessedSequences::<T>::insert(&bot_id_hash, sequence, now);
			Self::deposit_event(Event::SequenceProcessed { bot_id_hash, sequence });
			Ok(())
		}

		/// 设置节点 TEE 状态
		#[pallet::call_index(11)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn set_node_tee_status(
			origin: OriginFor<T>,
			node_id: NodeId,
			is_tee: bool,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Nodes::<T>::try_mutate(&node_id, |maybe_node| -> DispatchResult {
				let node = maybe_node.as_mut().ok_or(Error::<T>::NodeNotFound)?;
				ensure!(node.operator == who, Error::<T>::NotOperator);
				ensure!(node.is_tee_node != is_tee, Error::<T>::SameTeeStatus);
				node.is_tee_node = is_tee;
				Ok(())
			})?;
			Self::deposit_event(Event::NodeTeeStatusChanged { node_id, is_tee });
			Ok(())
		}
	}

	// ========================================================================
	// Internal Functions
	// ========================================================================

	impl<T: Config> Pallet<T> {
		/// 获取层级费用
		pub fn tier_fee(tier: &SubscriptionTier) -> BalanceOf<T> {
			match tier {
				SubscriptionTier::Basic => T::BasicFeePerEra::get(),
				SubscriptionTier::Pro => T::ProFeePerEra::get(),
				SubscriptionTier::Enterprise => T::EnterpriseFeePerEra::get(),
			}
		}

		/// Era 结束处理
		fn on_era_end(now: BlockNumberFor<T>) {
			let era = CurrentEra::<T>::get();
			let active_nodes = ActiveNodeList::<T>::get();
			let node_count = active_nodes.len() as u32;

			if node_count == 0 {
				CurrentEra::<T>::put(era.saturating_add(1));
				EraStartBlock::<T>::put(now);
				return;
			}

			// 1. 收取订阅费
			let mut subscription_income = BalanceOf::<T>::zero();
			for (_bot_hash, sub) in Subscriptions::<T>::iter() {
				if sub.status == SubscriptionStatus::Cancelled {
					continue;
				}
				let escrow = SubscriptionEscrow::<T>::get(&sub.bot_id_hash);
				if escrow >= sub.fee_per_era {
					SubscriptionEscrow::<T>::mutate(&sub.bot_id_hash, |e| {
						*e = e.saturating_sub(sub.fee_per_era);
					});
					T::Currency::unreserve(&sub.owner, sub.fee_per_era);
					subscription_income = subscription_income.saturating_add(sub.fee_per_era);
				} else {
					// 余额不足 → PastDue
					Subscriptions::<T>::mutate(&sub.bot_id_hash, |maybe_sub| {
						if let Some(s) = maybe_sub {
							if s.status == SubscriptionStatus::Active {
								s.status = SubscriptionStatus::PastDue;
							} else if s.status == SubscriptionStatus::PastDue {
								s.status = SubscriptionStatus::Suspended;
							}
						}
					});
				}
			}

			// 2. 拆分: 80% 节点, 10% 国库, 10% agent
			let node_share = subscription_income * 80u32.into() / 100u32.into();
			let treasury_share = subscription_income * 10u32.into() / 100u32.into();

			// 3. 铸币通胀
			let inflation = T::InflationPerEra::get();

			// 4. 可分配总额
			let total_pool = node_share.saturating_add(inflation);

			// 5. 按权重分配
			let mut total_weight: u128 = 0;
			let mut weights: alloc::vec::Vec<(NodeId, u128)> = alloc::vec::Vec::new();

			for node_id in active_nodes.iter() {
				if let Some(node) = Nodes::<T>::get(node_id) {
					let w = Self::compute_node_weight(&node);
					total_weight = total_weight.saturating_add(w);
					weights.push((*node_id, w));
				}
			}

			let mut total_distributed = BalanceOf::<T>::zero();
			if total_weight > 0 {
				for (node_id, w) in weights.iter() {
					let pool_u128: u128 = total_pool.unique_saturated_into();
					let reward_u128 = pool_u128.saturating_mul(*w) / total_weight;
					let reward: BalanceOf<T> = reward_u128.unique_saturated_into();
					if !reward.is_zero() {
						NodePendingRewards::<T>::mutate(node_id, |pending| {
							*pending = pending.saturating_add(reward);
						});
						total_distributed = total_distributed.saturating_add(reward);
					}
				}
			}

			// 记录
			let era_info = EraRewardInfo {
				subscription_income,
				inflation_mint: inflation,
				total_distributed,
				treasury_share,
				node_count,
			};
			EraRewards::<T>::insert(era, era_info);

			CurrentEra::<T>::put(era.saturating_add(1));
			EraStartBlock::<T>::put(now);

			Self::deposit_event(Event::EraCompleted { era, total_distributed });
		}

		/// 计算节点权重
		fn compute_node_weight(node: &ProjectNode<T>) -> u128 {
			let base = (node.reputation as u128).saturating_mul(100); // 简化: rep * 100

			let tee_factor = if node.is_tee_node {
				let multiplier = TeeRewardMultiplier::<T>::get();
				if multiplier == 0 { 10_000u128 } else { multiplier as u128 }
			} else {
				10_000u128
			};

			base.saturating_mul(tee_factor) / 10_000
		}

		/// 查询序列是否已处理
		pub fn is_sequence_processed(bot_id_hash: &BotIdHash, sequence: u64) -> bool {
			ProcessedSequences::<T>::contains_key(bot_id_hash, sequence)
		}
	}

	// ========================================================================
	// NodeConsensusProvider 实现
	// ========================================================================

	impl<T: Config> NodeConsensusProvider<T::AccountId> for Pallet<T> {
		fn is_node_active(node_id: &NodeId) -> bool {
			Nodes::<T>::get(node_id)
				.map(|n| n.status == NodeStatus::Active)
				.unwrap_or(false)
		}
		fn node_operator(node_id: &NodeId) -> Option<T::AccountId> {
			Nodes::<T>::get(node_id).map(|n| n.operator)
		}
		fn is_tee_node_by_operator(operator: &T::AccountId) -> bool {
			OperatorNodes::<T>::get(operator)
				.and_then(|node_id| Nodes::<T>::get(&node_id))
				.map(|n| n.is_tee_node)
				.unwrap_or(false)
		}
	}
}
