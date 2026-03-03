#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet GroupRobot Consensus — 节点管理 + TEE 验证 + Era 编排
//!
//! 负责节点生命周期管理和 Era 编排, 订阅和奖励已拆分到独立 pallet.
//!
//! ## 功能
//! - 节点注册 + 质押
//! - 节点退出 (冷却期)
//! - Equivocation 举报 + Slash
//! - 消息序列去重 (ProcessedSequences)
//! - TEE 验证 + 权重计算
//! - on_era_end 编排: 调用 SubscriptionSettler + EraRewardDistributor

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
use sp_runtime::traits::{Saturating, UniqueSaturatedInto};

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

type BalanceOf<T> =
	<<T as Config>::Currency as frame_support::traits::Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::traits::ReservableCurrency;

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
		/// 防膨胀: ProcessedSequences 过期区块数
		#[pallet::constant]
		type SequenceTtlBlocks: Get<BlockNumberFor<Self>>;
		/// 防膨胀: 每块最多清理的过期 Sequence 数
		#[pallet::constant]
		type MaxSequenceCleanupPerBlock: Get<u32>;
		/// 订阅结算 (Era 结束时调用)
		type SubscriptionSettler: SubscriptionSettler;
		/// 奖励分配 (Era 结束时调用)
		type RewardDistributor: EraRewardDistributor;
		/// 订阅层级查询 (tier gate)
		type Subscription: SubscriptionProvider;
		/// Peer Uptime 记录 (Era 结束时调用 registry pallet)
		type PeerUptimeRecorder: PeerUptimeRecorder;
		/// H3-fix: 节点退出时领取残留奖励
		type OrphanRewardClaimer: OrphanRewardClaimer<Self::AccountId>;
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


	/// TEE 奖励倍数 (basis points: 10000=1.0x, 15000=1.5x)
	#[pallet::storage]
	pub type TeeRewardMultiplier<T: Config> = StorageValue<_, u32, ValueQuery>;

	/// SGX 双证明额外奖励 (basis points)
	#[pallet::storage]
	pub type SgxEnclaveBonus<T: Config> = StorageValue<_, u32, ValueQuery>;

	/// 节点→Bot 绑定: 记录节点通过哪个 Bot 的证明验证了 TEE 状态
	#[pallet::storage]
	pub type NodeBotBinding<T: Config> =
		StorageMap<_, Blake2_128Concat, NodeId, BotIdHash>;


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
		SequenceProcessed { bot_id_hash: BotIdHash, sequence: u64 },
		SequenceDuplicate { bot_id_hash: BotIdHash, sequence: u64 },
		NodeTeeStatusChanged { node_id: NodeId, is_tee: bool },
		EraCompleted { era: u64, total_distributed: BalanceOf<T> },
		TeeRewardParamsUpdated { tee_multiplier: u32, sgx_bonus: u32 },
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
		/// Equivocation 已举报
		EquivocationAlreadyReported,
		/// 序列已处理
		SequenceAlreadyProcessed,
		/// Bot 所有者与节点操作者不匹配
		BotOwnerMismatch,
		/// TEE 证明无效或已过期
		AttestationNotValid,
		/// 节点已是 TEE 节点
		AlreadyTeeVerified,
		/// Free 层级不允许使用此功能
		FreeTierNotAllowed,
		/// Equivocation 证据无效 (相同消息哈希或签名)
		InvalidEquivocationEvidence,
		/// Equivocation 记录不存在
		EquivocationNotFound,
		/// 调用者不是 Bot 操作者或所有者
		NotBotOperator,
		/// TEE 奖励参数超出允许范围
		InvalidTeeRewardParams,
		/// Equivocation 已被解决 (不可重复 Slash)
		EquivocationAlreadyResolved,
		/// Equivocation 尚未解决 (不可清理)
		EquivocationNotResolved,
	}

	// ========================================================================
	// Hooks
	// ========================================================================

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: BlockNumberFor<T>) -> Weight {
			let mut weight = Weight::zero();

			// 🆕 防膨胀: 清理过期 ProcessedSequences
			weight = weight.saturating_add(Self::cleanup_expired_sequences(n));

			let era_length = T::EraLength::get();
			if era_length == BlockNumberFor::<T>::default() {
				return weight;
			}

			let era_start = EraStartBlock::<T>::get();
			if era_start == BlockNumberFor::<T>::default() {
				EraStartBlock::<T>::put(n);
				return weight.saturating_add(Weight::from_parts(5_000_000, 1_000));
			}

			if n.saturating_sub(era_start) >= era_length {
				Self::on_era_end(n);
				weight.saturating_add(Weight::from_parts(100_000_000, 20_000))
			} else {
				weight
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
				// H5-fix: 允许 Suspended 节点退出 (slash 后可回收剩余质押)
				ensure!(
					node.status == NodeStatus::Active || node.status == NodeStatus::Suspended,
					Error::<T>::NodeNotActive
				);
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

			// H3-fix: 节点退出前尝试领取残留奖励 (best-effort)
			T::OrphanRewardClaimer::try_claim_orphan_rewards(&node_id, &who);

			let stake = node.stake;
			T::Currency::unreserve(&who, stake);

			Nodes::<T>::remove(&node_id);
			OperatorNodes::<T>::remove(&who);
			ExitRequests::<T>::remove(&node_id);
			// H4-fix: 清理 NodeBotBinding 防止存储泄漏
			NodeBotBinding::<T>::remove(&node_id);

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
			// H1-fix: 验证 equivocation 证据有效性 (两条消息必须不同)
			ensure!(msg_hash_a != msg_hash_b, Error::<T>::InvalidEquivocationEvidence);
			ensure!(signature_a != signature_b, Error::<T>::InvalidEquivocationEvidence);
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
				// M3-fix: 使用正确的错误码
				let record = maybe_record.as_mut().ok_or(Error::<T>::EquivocationNotFound)?;
				// H1-R1: 防止重复 Slash
				ensure!(!record.resolved, Error::<T>::EquivocationAlreadyResolved);
				record.resolved = true;
				Ok(())
			})?;

			if let Some(node) = Nodes::<T>::get(&node_id) {
				let slash_pct = T::SlashPercentage::get();
				let slash_amount = node.stake * slash_pct.into() / 100u32.into();
				// NH3-fix: 使用实际 slash 金额 (slash_reserved 可能只 slash 部分)
				let (_, remaining) = T::Currency::slash_reserved(&node.operator, slash_amount);
				let actual_slashed = slash_amount.saturating_sub(remaining);

				Nodes::<T>::mutate(&node_id, |maybe_node| {
					if let Some(n) = maybe_node {
						n.stake = n.stake.saturating_sub(actual_slashed);
						n.status = NodeStatus::Suspended;
						// H8-fix: 重置 TEE 状态防止陈旧标记
						n.is_tee_node = false;
					}
				});

				ActiveNodeList::<T>::mutate(|list| {
					list.retain(|id| id != &node_id);
				});

				// M2-fix: 清理被 Slash 节点的 Bot 绑定
				NodeBotBinding::<T>::remove(&node_id);

				Self::deposit_event(Event::NodeSlashed { node_id, amount: actual_slashed });
			}
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
			let who = ensure_signed(origin)?;

			// Tier gate: Free 层级不允许链上序列号去重
			ensure!(
				T::Subscription::effective_tier(&bot_id_hash).is_paid(),
				Error::<T>::FreeTierNotAllowed
			);

			// H2-fix: 验证调用者是 Bot 所有者或操作者
			let is_owner = T::BotRegistry::bot_owner(&bot_id_hash)
				.map(|owner| owner == who)
				.unwrap_or(false);
			let is_operator = T::BotRegistry::bot_operator(&bot_id_hash)
				.map(|op| op == who)
				.unwrap_or(false);
			ensure!(is_owner || is_operator, Error::<T>::NotBotOperator);

			if ProcessedSequences::<T>::contains_key(&bot_id_hash, sequence) {
				Self::deposit_event(Event::SequenceDuplicate { bot_id_hash, sequence });
				return Ok(());
			}
			let now = frame_system::Pallet::<T>::block_number();
			ProcessedSequences::<T>::insert(&bot_id_hash, sequence, now);
			Self::deposit_event(Event::SequenceProcessed { bot_id_hash, sequence });
			Ok(())
		}

		/// 验证节点 TEE 状态 (通过 Registry 证明验证)
		///
		/// 节点操作者必须同时是 Bot 所有者, 且 Bot 必须有有效的 TEE 证明。
		/// 不接受自我声明, 仅信任 Registry 的证明记录。
		#[pallet::call_index(11)]
		#[pallet::weight(Weight::from_parts(40_000_000, 8_000))]
		pub fn verify_node_tee(
			origin: OriginFor<T>,
			node_id: NodeId,
			bot_id_hash: BotIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let node = Nodes::<T>::get(&node_id).ok_or(Error::<T>::NodeNotFound)?;
			ensure!(node.operator == who, Error::<T>::NotOperator);
			// M3-R1: 仅活跃节点可验证 TEE
			ensure!(node.status == NodeStatus::Active, Error::<T>::NodeNotActive);
			ensure!(!node.is_tee_node, Error::<T>::AlreadyTeeVerified);

			// 验证 Bot 活跃且属于该操作者
			ensure!(T::BotRegistry::is_bot_active(&bot_id_hash), Error::<T>::BotNotRegistered);
			let bot_owner = T::BotRegistry::bot_owner(&bot_id_hash)
				.ok_or(Error::<T>::BotNotRegistered)?;
			ensure!(bot_owner == who, Error::<T>::BotOwnerMismatch);

			// 验证 Bot 有有效的 TEE 证明 (Registry 的证明记录)
			ensure!(T::BotRegistry::is_tee_node(&bot_id_hash), Error::<T>::AttestationNotValid);
			ensure!(T::BotRegistry::is_attestation_fresh(&bot_id_hash), Error::<T>::AttestationNotValid);

			// 设置 TEE 状态 + 绑定 Bot
			Nodes::<T>::mutate(&node_id, |maybe_node| {
				if let Some(n) = maybe_node {
					n.is_tee_node = true;
				}
			});
			NodeBotBinding::<T>::insert(&node_id, bot_id_hash);

			Self::deposit_event(Event::NodeTeeStatusChanged { node_id, is_tee: true });
			Ok(())
		}

		/// 清理已解决的 Equivocation 记录 (任何人可调用)
		///
		/// 仅允许清理 `resolved=true` 的记录, 释放链上存储。
		#[pallet::call_index(13)]
		#[pallet::weight(Weight::from_parts(20_000_000, 4_000))]
		pub fn cleanup_resolved_equivocation(
			origin: OriginFor<T>,
			node_id: NodeId,
			sequence: u64,
		) -> DispatchResult {
			ensure_signed(origin)?;
			let record = EquivocationRecords::<T>::get(&node_id, sequence)
				.ok_or(Error::<T>::EquivocationNotFound)?;
			ensure!(record.resolved, Error::<T>::EquivocationNotResolved);
			EquivocationRecords::<T>::remove(&node_id, sequence);
			Ok(())
		}

		/// 治理设置 TEE 奖励参数
		///
		/// - `tee_multiplier`: TEE 节点奖励倍数 (basis points, 10000=1.0x, 15000=1.5x, 0=使用默认 1.0x)
		/// - `sgx_bonus`: SGX 双证明额外奖励 (basis points, 叠加到 TEE 倍数上, 例如 2000=+0.2x)
		#[pallet::call_index(12)]
		#[pallet::weight(Weight::from_parts(10_000_000, 2_000))]
		pub fn set_tee_reward_params(
			origin: OriginFor<T>,
			tee_multiplier: u32,
			sgx_bonus: u32,
		) -> DispatchResult {
			ensure_root(origin)?;
			// H6-fix: 上限校验 (tee_multiplier 最大 50000=5x, sgx_bonus 最大 10000=1x)
			ensure!(tee_multiplier <= 50_000, Error::<T>::InvalidTeeRewardParams);
			ensure!(sgx_bonus <= 10_000, Error::<T>::InvalidTeeRewardParams);
			TeeRewardMultiplier::<T>::put(tee_multiplier);
			SgxEnclaveBonus::<T>::put(sgx_bonus);
			Self::deposit_event(Event::TeeRewardParamsUpdated { tee_multiplier, sgx_bonus });
			Ok(())
		}
	}

	// ========================================================================
	// Internal Functions
	// ========================================================================

	impl<T: Config> Pallet<T> {
		/// 🆕 防膨胀: 清理过期 ProcessedSequences
		///
		/// 游标式迭代: 每块最多清理 MaxSequenceCleanupPerBlock 条过期记录,
		/// 避免单块计算量过大。过期标准: 记录的区块号 + SequenceTtlBlocks < 当前块。
		///
		/// NH1-fix: 扫描上限 = max_cleanup * 3, 防止全表 O(N) 迭代。
		fn cleanup_expired_sequences(now: BlockNumberFor<T>) -> Weight {
			let ttl = T::SequenceTtlBlocks::get();
			let max_cleanup = T::MaxSequenceCleanupPerBlock::get();
			let max_scan = max_cleanup.saturating_mul(3); // NH1-fix: 扫描上限
			let mut cleaned = 0u32;
			let mut scanned = 0u32;
			let mut to_remove: alloc::vec::Vec<(BotIdHash, u64)> = alloc::vec::Vec::new();

			for (bot_hash, seq, recorded_block) in ProcessedSequences::<T>::iter() {
				scanned += 1;
				if scanned > max_scan || cleaned >= max_cleanup {
					break;
				}
				if now.saturating_sub(recorded_block) > ttl {
					to_remove.push((bot_hash, seq));
					cleaned += 1;
				}
			}

			for (bot_hash, seq) in to_remove {
				ProcessedSequences::<T>::remove(&bot_hash, seq);
			}

			// 1 read per scanned + 1 write per cleaned
			Weight::from_parts(
				5_000_000u64
					.saturating_add(scanned as u64 * 5_000_000)
					.saturating_add(cleaned as u64 * 10_000_000),
				1_000u64
					.saturating_add(scanned as u64 * 100)
					.saturating_add(cleaned as u64 * 200),
			)
		}

		/// Era 结束处理
		///
		/// 编排: 调用 SubscriptionSettler + EraRewardDistributor 完成订阅结算和奖励分配
		fn on_era_end(now: BlockNumberFor<T>) {
			let era = CurrentEra::<T>::get();
			let active_nodes = ActiveNodeList::<T>::get();
			let node_count = active_nodes.len() as u32;

			// M5-R1: 订阅结算始终执行, 即使无活跃节点 (费用收取/逾期标记独立于节点)
			// 1. 委托 subscription pallet 结算订阅费
			//    90% node_share 已由 subscription pallet 直接转给 Bot 运营者
			//    10% → Treasury (由 subscription pallet 内部完成)
			let settlement = T::SubscriptionSettler::settle_era();
			let subscription_income_u128 = settlement.total_income;
			let treasury_share_u128 = settlement.treasury_share;

			if node_count == 0 {
				// M5-R1: 无节点时仍执行 uptime 快照和 era 清理
				T::PeerUptimeRecorder::record_era_uptime(era);
				CurrentEra::<T>::put(era.saturating_add(1));
				EraStartBlock::<T>::put(now);
				T::RewardDistributor::prune_old_eras(era);
				Self::deposit_event(Event::EraCompleted { era, total_distributed: BalanceOf::<T>::default() });
				return;
			}

			// 2. 通胀额度 (由 rewards pallet 在 distribute_and_record 时铸币到 RewardPool)
			let inflation = T::InflationPerEra::get();

			// 4. 可分配总额 = 仅通胀 (订阅 node_share 已全部直接分配给运营者, 不参与权重分配)
			let total_pool = inflation;

			// M1-fix: 合并 TEE 检查 + 权重计算为单次循环, 避免双重 Nodes::get()
			let mut weights: alloc::vec::Vec<(NodeId, u128)> = alloc::vec::Vec::new();
			for node_id in active_nodes.iter() {
				if let Some(mut node) = Nodes::<T>::get(node_id) {
					// 5. 检查 TEE 证明有效性, 过期则降级
					if node.is_tee_node {
						let still_valid = NodeBotBinding::<T>::get(node_id)
							.map(|bot_hash| {
								T::BotRegistry::is_tee_node(&bot_hash)
									&& T::BotRegistry::is_attestation_fresh(&bot_hash)
							})
							.unwrap_or(false);
						if !still_valid {
							node.is_tee_node = false;
							Nodes::<T>::insert(node_id, &node);
							NodeBotBinding::<T>::remove(node_id);
							Self::deposit_event(Event::NodeTeeStatusChanged {
								node_id: *node_id, is_tee: false,
							});
						}
					}
					// 6. 计算节点权重 (使用已更新的 node)
					let w = Self::compute_node_weight(&node, node_id);
					weights.push((*node_id, w));
				}
			}

			// 6. 委托 rewards pallet 分配奖励并记录 Era 信息
			let total_pool_u128: u128 = total_pool.unique_saturated_into();
			let inflation_u128: u128 = inflation.unique_saturated_into();
			let total_distributed_u128 = T::RewardDistributor::distribute_and_record(
				era,
				total_pool_u128,
				subscription_income_u128,
				inflation_u128,
				treasury_share_u128,
				&weights,
				node_count,
			);
			let total_distributed: BalanceOf<T> = total_distributed_u128.unique_saturated_into();

			// 委托 registry pallet 快照 Peer 心跳并清理历史
			T::PeerUptimeRecorder::record_era_uptime(era);

			CurrentEra::<T>::put(era.saturating_add(1));
			EraStartBlock::<T>::put(now);

			// 委托 rewards pallet 清理过期记录
			T::RewardDistributor::prune_old_eras(era);

			Self::deposit_event(Event::EraCompleted { era, total_distributed });
		}

		/// 计算节点权重 (非 TEE 节点返回 0, 不参与 Era 奖励分配)
		///
		/// TEE 节点: base × tee_factor / 10000
		/// SGX 双证明节点: base × (tee_factor + sgx_bonus) / 10000
		fn compute_node_weight(node: &ProjectNode<T>, node_id: &NodeId) -> u128 {
			if !node.is_tee_node {
				return 0u128;
			}
			let base = (node.reputation as u128).saturating_mul(100);

			let tee_multiplier = {
				let m = TeeRewardMultiplier::<T>::get();
				if m == 0 { 10_000u128 } else { m as u128 }
			};

			// 查询绑定 Bot 是否有 SGX 双证明
			let sgx_bonus = NodeBotBinding::<T>::get(node_id)
				.filter(|bot_hash| T::BotRegistry::has_dual_attestation(bot_hash))
				.map(|_| SgxEnclaveBonus::<T>::get() as u128)
				.unwrap_or(0u128);

			let total_factor = tee_multiplier.saturating_add(sgx_bonus);
			base.saturating_mul(total_factor) / 10_000
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
