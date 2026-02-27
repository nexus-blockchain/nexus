// This is free and unencumbered software released into the public domain.
//
// Anyone is free to copy, modify, publish, use, compile, sell, or
// distribute this software, either in source code form or as a compiled
// binary, for any purpose, commercial or non-commercial, and by any
// means.
//
// In jurisdictions that recognize copyright laws, the author or authors
// of this software dedicate any and all copyright interest in the
// software to the public domain. We make this dedication for the benefit
// of the public at large and to the detriment of our heirs and
// successors. We intend this dedication to be an overt act of
// relinquishment in perpetuity of all present and future rights to this
// software under copyright law.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
// OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
// OTHER DEALINGS IN THE SOFTWARE.
//
// For more information, please refer to <http://unlicense.org>

// Substrate and Polkadot dependencies
use sp_runtime::{traits::AccountIdConversion, generic};
use crate::UncheckedExtrinsic;
use frame_support::{
	derive_impl, parameter_types,
	traits::{ConstBool, ConstU128, ConstU16, ConstU32, ConstU64, ConstU8, VariantCountOf},
	weights::{
		constants::{RocksDbWeight, WEIGHT_REF_TIME_PER_SECOND},
		IdentityFee, Weight,
	},
};
use frame_system::{limits::{BlockLength, BlockWeights}, EnsureRoot};
use pallet_transaction_payment::{ConstFeeMultiplier, FungibleAdapter, Multiplier};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_runtime::{traits::One, Perbill};
use sp_version::RuntimeVersion;

// Local module imports
use super::{
	AccountId, Aura, Balance, Balances, Block, BlockNumber, Hash, Nonce, PalletInfo, Runtime,
	RuntimeCall, RuntimeEvent, RuntimeFreezeReason, RuntimeHoldReason, RuntimeOrigin, RuntimeTask,
	System, Timestamp, EXISTENTIAL_DEPOSIT, SLOT_DURATION, VERSION, UNIT, MINUTES, HOURS, DAYS,
	TechnicalCommittee, ArbitrationCommittee, TreasuryCouncil, ContentCommittee,
	// Entity types (原 ShareMall)
	Assets, Escrow, EntityRegistry, EntityShop, EntityService, EntityTransaction, EntityToken, EntityKyc,
};

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

parameter_types! {
	pub const BlockHashCount: BlockNumber = 2400;
	pub const Version: RuntimeVersion = VERSION;

	/// We allow for 2 seconds of compute with a 6 second average block time.
	pub RuntimeBlockWeights: BlockWeights = BlockWeights::with_sensible_defaults(
		Weight::from_parts(2u64 * WEIGHT_REF_TIME_PER_SECOND, u64::MAX),
		NORMAL_DISPATCH_RATIO,
	);
	pub RuntimeBlockLength: BlockLength = BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub const SS58Prefix: u8 = 42;
}

/// The default types are being injected by [`derive_impl`](`frame_support::derive_impl`) from
/// [`SoloChainDefaultConfig`](`struct@frame_system::config_preludes::SolochainDefaultConfig`),
/// but overridden as needed.
#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig)]
impl frame_system::Config for Runtime {
	/// The block type for the runtime.
	type Block = Block;
	/// Block & extrinsics weights: base values and limits.
	type BlockWeights = RuntimeBlockWeights;
	/// The maximum length of a block (in bytes).
	type BlockLength = RuntimeBlockLength;
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The type for storing how many extrinsics an account has signed.
	type Nonce = Nonce;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = RocksDbWeight;
	/// Version of the runtime.
	type Version = Version;
	/// The data to be stored in an account.
	type AccountData = pallet_balances::AccountData<Balance>;
	/// This is used as an identifier of the chain. 42 is the generic substrate prefix.
	type SS58Prefix = SS58Prefix;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type DisabledValidators = ();
	type MaxAuthorities = ConstU32<32>;
	type AllowMultipleBlocksPerSlot = ConstBool<false>;
	type SlotDuration = pallet_aura::MinimumPeriodTimesTwo<Runtime>;
}

impl pallet_grandpa::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;

	type WeightInfo = ();
	type MaxAuthorities = ConstU32<32>;
	type MaxNominators = ConstU32<0>;
	type MaxSetIdSessionEntries = ConstU64<0>;

	type KeyOwnerProof = sp_core::Void;
	type EquivocationReportSystem = ();
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = Aura;
	type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
	type WeightInfo = ();
}

impl pallet_balances::Config for Runtime {
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<EXISTENTIAL_DEPOSIT>;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
	type FreezeIdentifier = RuntimeFreezeReason;
	type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type DoneSlashHandler = ();
}

parameter_types! {
	pub FeeMultiplier: Multiplier = Multiplier::one();
}

impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = FungibleAdapter<Balances, ()>;
	type OperationalFeeMultiplier = ConstU8<5>;
	type WeightToFee = IdentityFee<Balance>;
	type LengthToFee = IdentityFee<Balance>;
	type FeeMultiplierUpdate = ConstFeeMultiplier<FeeMultiplier>;
	type WeightInfo = pallet_transaction_payment::weights::SubstrateWeight<Runtime>;
}

impl pallet_sudo::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

// -------------------- 全局系统账户（简化方案：4 个核心账户）--------------------

parameter_types! {
	// 1. 国库账户 - 核心账户，含平台收入、存储补贴
	pub const TreasuryPalletId: frame_support::PalletId = frame_support::PalletId(*b"py/trsry");
	pub TreasuryAccountId: AccountId = TreasuryPalletId::get().into_account_truncating();
	
	// 2. 销毁账户 - 专用于代币销毁，必须独立
	pub const BurnPalletId: frame_support::PalletId = frame_support::PalletId(*b"py/burn!");
	pub BurnAccountId: AccountId = BurnPalletId::get().into_account_truncating();
}

// ============================================================================
// 随机数生成器
// ============================================================================

/// 安全随机数生成器 - 基于 Collective Coin Flipping 机制
/// 
/// 原理：
/// - 结合多个历史区块哈希（81个区块，对应九宫格 9x9）
/// - 混合当前区块信息和用户提供的 subject
/// - 使用 blake2_256 进行哈希混合
pub struct CollectiveFlipRandomness;

impl frame_support::traits::Randomness<Hash, BlockNumber> for CollectiveFlipRandomness {
	fn random(subject: &[u8]) -> (Hash, BlockNumber) {
		let block_number = System::block_number();
		
		// 收集最近 81 个区块的哈希
		let mut combined_entropy = alloc::vec::Vec::with_capacity(81 * 32 + subject.len() + 8);
		
		// 添加 subject 作为熵源
		combined_entropy.extend_from_slice(subject);
		
		// 添加当前区块号
		combined_entropy.extend_from_slice(&block_number.to_le_bytes());
		
		// 收集历史区块哈希
		let blocks_to_collect = core::cmp::min(block_number.saturating_sub(1), 81);
		for i in 1..=blocks_to_collect {
			let hash = System::block_hash(block_number.saturating_sub(i as u32));
			combined_entropy.extend_from_slice(hash.as_ref());
		}
		
		// 添加父区块哈希作为额外熵源
		let parent_hash = System::parent_hash();
		combined_entropy.extend_from_slice(parent_hash.as_ref());
		
		// 使用 blake2_256 生成最终随机值
		let final_hash = sp_core::hashing::blake2_256(&combined_entropy);
		
		(Hash::from_slice(&final_hash), block_number)
	}
}

// ============================================================================
// Common Utilities
// ============================================================================

/// 时间戳提供器 - 使用 pallet_timestamp
pub struct TimestampProvider;

impl frame_support::traits::UnixTime for TimestampProvider {
	fn now() -> core::time::Duration {
		let millis = pallet_timestamp::Pallet::<Runtime>::get();
		core::time::Duration::from_millis(millis)
	}
}

// ============================================================================
// Trading Pallets Configuration
// ============================================================================

/// Pricing Provider 实现 — 基于 pallet-nex-market 的 TWAP/LastTradePrice
///
/// 价格优先级：1h TWAP（抗操纵） → LastTradePrice → initial_price（治理设定）
pub struct TradingPricingProvider;

impl pallet_trading_common::PricingProvider<Balance> for TradingPricingProvider {
	fn get_cos_to_usd_rate() -> Option<Balance> {
		// 优先: 1h TWAP（抗操纵，平滑单笔极端成交）
		pallet_nex_market::Pallet::<Runtime>::calculate_twap(
			pallet_nex_market::pallet::TwapPeriod::OneHour
		)
		// 回退: 最近成交价（TWAP 数据不足时）
		.or_else(|| pallet_nex_market::pallet::LastTradePrice::<Runtime>::get())
		// 兜底: 治理设定的初始价格（冷启动期）
		.or_else(|| {
			pallet_nex_market::pallet::PriceProtectionStore::<Runtime>::get()
				.and_then(|config| config.initial_price)
		})
		.map(|price_u64| price_u64 as Balance)
	}
	
	fn report_p2p_trade(_timestamp: u64, _price_usdt: u64, _nex_qty: u128) -> sp_runtime::DispatchResult {
		// nex-market 内部自行更新 TWAP，无需外部上报
		Ok(())
	}
}

/// 统一兑换比率接口实现 — 聚合 TWAP + 陈旧检测 + 置信度评估
pub struct NexExchangeRateProvider;

impl pallet_trading_common::ExchangeRateProvider for NexExchangeRateProvider {
	fn get_nex_usdt_rate() -> Option<u64> {
		// 与 TradingPricingProvider 相同优先级
		<TradingPricingProvider as pallet_trading_common::PricingProvider<Balance>>::get_cos_to_usd_rate()
			.map(|rate| rate as u64)
	}

	fn price_confidence() -> u8 {
		use pallet_trading_common::PriceOracle;
		type Oracle = pallet_nex_market::Pallet<Runtime>;

		// 1. 无任何价格数据
		let rate = <TradingPricingProvider as pallet_trading_common::PricingProvider<Balance>>::get_cos_to_usd_rate();
		if rate.is_none() {
			return 0;
		}

		// 2. 检查数据新鲜度（超过 4h 视为过时）
		let stale = Oracle::is_price_stale(2400);
		let trade_count = Oracle::get_trade_count();

		// 3. 判断数据来源层级
		let has_twap = pallet_nex_market::Pallet::<Runtime>::calculate_twap(
			pallet_nex_market::pallet::TwapPeriod::OneHour
		).is_some();
		let has_last_trade = pallet_nex_market::pallet::LastTradePrice::<Runtime>::get().is_some();

		if stale {
			// 价格过时：低置信度
			if has_twap { 25 } else if has_last_trade { 15 } else { 10 }
		} else if has_twap && trade_count >= 100 {
			// TWAP 可用 + 高交易量：最高置信度
			95
		} else if has_twap {
			// TWAP 可用但交易量低
			80
		} else if has_last_trade {
			// 仅 LastTradePrice（TWAP 数据不足）
			65
		} else {
			// 仅 initial_price（冷启动期）
			35
		}
	}
}

// -------------------- NEX Market (NEX/USDT 订单簿) --------------------

parameter_types! {
	pub NexMarketTreasuryAccount: AccountId = frame_support::PalletId(*b"nxm/trsy").into_account_truncating();
	pub NexMarketSeedAccount: AccountId = frame_support::PalletId(*b"nxm/seed").into_account_truncating();
	pub NexMarketRewardSource: AccountId = frame_support::PalletId(*b"nxm/rwds").into_account_truncating();
	pub const NexMarketSeedTronAddr: [u8; 34] = *b"T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWb1";
}

impl pallet_nex_market::Config for Runtime {
	type Currency = Balances;
	type WeightInfo = ();
	type DefaultOrderTTL = ConstU32<{ 24 * HOURS }>;
	type MaxActiveOrdersPerUser = ConstU32<100>;
	type UsdtTimeout = ConstU32<{ 12 * HOURS }>;
	type BlocksPerHour = ConstU32<{ 1 * HOURS }>;
	type BlocksPerDay = ConstU32<{ 24 * HOURS }>;
	type BlocksPerWeek = ConstU32<{ 7 * DAYS }>;
	type CircuitBreakerDuration = ConstU32<{ 1 * HOURS }>;
	type VerificationReward = ConstU128<{ UNIT / 10 }>;     // 0.1 NEX
	type RewardSource = NexMarketRewardSource;
	type BuyerDepositRate = ConstU16<1000>;                  // 10%
	type MinBuyerDeposit = ConstU128<{ 10 * UNIT }>;         // 10 NEX
	type DepositForfeitRate = ConstU16<10000>;               // 100%
	type UsdtToNexRate = ConstU64<10_000_000_000>;            // 1 USDT = 10 NEX
	type TreasuryAccount = NexMarketTreasuryAccount;
	type SeedLiquidityAccount = NexMarketSeedAccount;
	type MarketAdminOrigin = pallet_collective::EnsureProportionAtLeast<
		AccountId, TreasuryCollectiveInstance, 2, 3
	>;
	type FirstOrderTimeout = ConstU32<{ 1 * HOURS }>;      // 免保证金短超时 1h
	type MaxFirstOrderAmount = ConstU128<{ 100 * UNIT }>;   // 免保证金单笔上限 100 NEX
	type MaxWaivedSeedOrders = ConstU32<20>;                 // seed_liquidity 单次最多 20 笔
	type SeedPricePremiumBps = ConstU16<2000>;               // seed 溢价 20%
	type SeedOrderUsdtAmount = ConstU64<10_000_000>;          // 固定 10 USDT/笔
	type SeedTronAddress = NexMarketSeedTronAddr;
	type VerificationGracePeriod = ConstU32<{ 1 * HOURS }>;  // 1h 宽限期
	type UnderpaidGracePeriod = ConstU32<{ 2 * HOURS }>;    // 2h 补付窗口
}

// ============================================================================
// Escrow, Referral, IPFS Pallets Configuration
// ============================================================================

// -------------------- Escrow (托管) --------------------

parameter_types! {
	pub const EscrowPalletId: frame_support::PalletId = frame_support::PalletId(*b"py/escro");
}

/// 托管过期策略实现
pub struct DefaultExpiryPolicy;

impl pallet_escrow::ExpiryPolicy<AccountId, BlockNumber> for DefaultExpiryPolicy {
	fn on_expire(_id: u64) -> Result<pallet_escrow::ExpiryAction<AccountId>, sp_runtime::DispatchError> {
		// 默认策略：过期后不执行任何操作
		Ok(pallet_escrow::ExpiryAction::Noop)
	}

	fn now() -> BlockNumber {
		System::block_number()
	}
}

impl pallet_escrow::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type EscrowPalletId = EscrowPalletId;
	type AuthorizedOrigin = frame_system::EnsureSigned<AccountId>;
	type AdminOrigin = frame_system::EnsureRoot<AccountId>;
	type MaxExpiringPerBlock = ConstU32<100>;
	type MaxSplitEntries = ConstU32<20>;
	type ExpiryPolicy = DefaultExpiryPolicy;
	type WeightInfo = pallet_escrow::weights::SubstrateWeight<Runtime>;
}

// -------------------- Storage Service (存储服务) --------------------

parameter_types! {
	// 3. 存储服务主账户 - 核心账户，含费用收集
	pub const StorageServicePalletId: frame_support::PalletId = frame_support::PalletId(*b"py/storg");
	pub StoragePoolAccountId: AccountId = StorageServicePalletId::get().into_account_truncating();
	
	// 4. 运营商托管账户 - 必须独立
	pub OperatorEscrowAccountId: AccountId = StorageServicePalletId::get().into_sub_account_truncating(b"escrow");
}

// OCW unsigned transaction support for pallet-storage-service
impl frame_system::offchain::CreateTransactionBase<pallet_storage_service::Call<Runtime>> for Runtime {
	type Extrinsic = UncheckedExtrinsic;
	type RuntimeCall = RuntimeCall;
}

impl frame_system::offchain::CreateBare<pallet_storage_service::Call<Runtime>> for Runtime {
	fn create_bare(call: Self::RuntimeCall) -> Self::Extrinsic {
		generic::UncheckedExtrinsic::new_bare(call)
	}
}

impl pallet_storage_service::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type Balance = Balance;
	type FeeCollector = StoragePoolAccountId;
	// 内容委员会 1/2 多数通过（P0 治理集成）
	type GovernanceOrigin = pallet_collective::EnsureProportionAtLeast<
		AccountId,
		ContentCollectiveInstance,
		1, 2  // 1/2 多数通过
	>;
	type MaxCidHashLen = ConstU32<64>;
	type MaxPeerIdLen = ConstU32<128>;
	type MinOperatorBond = ConstU128<{ 100 * UNIT }>;
	type MinOperatorBondUsd = ConstU64<100_000_000>; // 100 USDT
	type DepositCalculator = pallet_trading_common::DepositCalculatorImpl<TradingPricingProvider, Balance>;
	type MinCapacityGiB = ConstU32<10>;
	type WeightInfo = pallet_storage_service::weights::SubstrateWeight<Runtime>;
	type SubjectPalletId = StorageServicePalletId;
	type IpfsPoolAccount = StoragePoolAccountId;
	type OperatorEscrowAccount = OperatorEscrowAccountId;
	type MonthlyPublicFeeQuota = ConstU128<{ 10 * UNIT }>;
	type QuotaResetPeriod = ConstU32<{ 30 * DAYS }>;
	type DefaultBillingPeriod = ConstU32<{ 30 * DAYS }>;
	type OperatorGracePeriod = ConstU32<{ 7 * DAYS }>;
}

// -------------------- Evidence (证据存证) --------------------

parameter_types! {
	pub const EvidenceNsBytes: [u8; 8] = *b"evidence";
}

/// 证据授权适配器 - 暂时允许所有签名用户
pub struct AlwaysAuthorizedEvidence;

impl pallet_evidence::pallet::EvidenceAuthorizer<AccountId> for AlwaysAuthorizedEvidence {
	fn is_authorized(_ns: [u8; 8], _who: &AccountId) -> bool {
		// 暂时允许所有签名用户提交证据
		// 后续可以对接更细粒度的权限系统
		true
	}
}

impl pallet_evidence::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	// Phase 1.5 新参数
	type MaxContentCidLen = ConstU32<64>;
	type MaxSchemeLen = ConstU32<32>;
	// 旧版参数（向后兼容）
	type MaxCidLen = ConstU32<64>;
	type MaxImg = ConstU32<20>;
	type MaxVid = ConstU32<10>;
	type MaxDoc = ConstU32<20>;
	type MaxMemoLen = ConstU32<512>;
	type MaxAuthorizedUsers = ConstU32<50>;
	type MaxKeyLen = ConstU32<512>;
	type EvidenceNsBytes = EvidenceNsBytes;
	type Authorizer = AlwaysAuthorizedEvidence;
	type MaxPerSubjectTarget = ConstU32<1000>;
	type MaxPerSubjectNs = ConstU32<1000>;
	type WindowBlocks = ConstU32<{ 10 * MINUTES }>;
	type MaxPerWindow = ConstU32<100>;
	type EnableGlobalCidDedup = ConstBool<true>;
	type MaxListLen = ConstU32<100>;
	type WeightInfo = pallet_evidence::weights::SubstrateWeight<Runtime>;
	// IPFS 相关
	type IpfsPinner = pallet_storage_service::Pallet<Runtime>;
	type Balance = Balance;
	type DefaultStoragePrice = ConstU128<{ UNIT / 10 }>;
	// 🆕 证据修改窗口（2天 ≈ 28800 blocks，按6秒/块）
	type EvidenceEditWindow = ConstU32<28800>;
	// 🆕 防膨胀: 归档记录 TTL (180天 ≈ 2_592_000 blocks)
	type ArchiveTtlBlocks = ConstU32<2_592_000>;
}

// -------------------- Arbitration (仲裁) --------------------

/// 商城订单域标识（8字节）
const DOMAIN_ENTITY_ORDER: [u8; 8] = *b"entorder";

/// 统一仲裁域路由器
/// 
/// 将仲裁决议路由到各业务模块执行
/// 当前支持：entorder（商城订单），其余域保留默认行为
pub struct UnifiedArbitrationRouter;

impl pallet_arbitration::pallet::ArbitrationRouter<AccountId, Balance> for UnifiedArbitrationRouter {
	/// 校验是否允许发起争议
	fn can_dispute(domain: [u8; 8], who: &AccountId, id: u64) -> bool {
		use pallet_entity_common::OrderProvider;
		if domain == DOMAIN_ENTITY_ORDER {
			<EntityTransaction as OrderProvider<AccountId, Balance>>::can_dispute(id, who)
		} else {
			// 其他域暂时允许（未来扩展 nex-market 等）
			true
		}
	}

	/// 应用裁决（放款/退款/部分分账）
	fn apply_decision(domain: [u8; 8], id: u64, decision: pallet_arbitration::pallet::Decision) -> sp_runtime::DispatchResult {
		use pallet_escrow::pallet::Escrow as EscrowTrait;
		if domain == DOMAIN_ENTITY_ORDER {
			use pallet_entity_common::OrderProvider;
			let buyer = <EntityTransaction as OrderProvider<AccountId, Balance>>::order_buyer(id)
				.ok_or(sp_runtime::DispatchError::Other("order not found"))?;
			let seller = <EntityTransaction as OrderProvider<AccountId, Balance>>::order_seller(id)
				.ok_or(sp_runtime::DispatchError::Other("order not found"))?;

			// 解除争议锁定
			let _ = <Escrow as EscrowTrait<AccountId, Balance>>::set_resolved(id);

			match decision {
				pallet_arbitration::pallet::Decision::Release => {
					// 卖家胜诉：释放给卖家
					<Escrow as EscrowTrait<AccountId, Balance>>::release_all(id, &seller)
				},
				pallet_arbitration::pallet::Decision::Refund => {
					// 买家胜诉：退款给买家
					<Escrow as EscrowTrait<AccountId, Balance>>::refund_all(id, &buyer)
				},
				pallet_arbitration::pallet::Decision::Partial(bps) => {
					// 部分裁决：按比例分账（bps/10000 给卖家，剩余退买家）
					<Escrow as EscrowTrait<AccountId, Balance>>::split_partial(id, &seller, &buyer, bps)
				},
			}
		} else {
			Ok(())
		}
	}

	/// 获取纠纷对方账户
	fn get_counterparty(domain: [u8; 8], initiator: &AccountId, id: u64) -> Result<AccountId, sp_runtime::DispatchError> {
		use pallet_entity_common::OrderProvider;
		if domain == DOMAIN_ENTITY_ORDER {
			let buyer = <EntityTransaction as OrderProvider<AccountId, Balance>>::order_buyer(id)
				.ok_or(sp_runtime::DispatchError::Other("order not found"))?;
			let seller = <EntityTransaction as OrderProvider<AccountId, Balance>>::order_seller(id)
				.ok_or(sp_runtime::DispatchError::Other("order not found"))?;
			// 如果发起人是买家，对方是卖家；反之亦然
			if *initiator == buyer {
				Ok(seller)
			} else if *initiator == seller {
				Ok(buyer)
			} else {
				Err(sp_runtime::DispatchError::Other("not a party"))
			}
		} else {
			Ok(TreasuryAccountId::get())
		}
	}

	/// 获取订单/交易金额（用于计算押金）
	fn get_order_amount(domain: [u8; 8], id: u64) -> Result<Balance, sp_runtime::DispatchError> {
		use pallet_entity_common::OrderProvider;
		if domain == DOMAIN_ENTITY_ORDER {
			<EntityTransaction as OrderProvider<AccountId, Balance>>::order_amount(id)
				.ok_or(sp_runtime::DispatchError::Other("order not found"))
		} else {
			Ok(10 * UNIT)
		}
	}

	/// 获取做市商ID（仅做市商域有效）
	fn get_maker_id(_domain: [u8; 8], _id: u64) -> Option<u64> {
		None
	}
}

/// 信用分更新器实现（做市商信用系统已移除，空实现）
pub struct TradingCreditUpdater;

impl pallet_arbitration::pallet::CreditUpdater for TradingCreditUpdater {
	fn record_maker_dispute_result(_maker_id: u64, _order_id: u64, _maker_win: bool) -> sp_runtime::DispatchResult {
		Ok(())
	}
}

impl pallet_arbitration::pallet::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MaxEvidence = ConstU32<20>;
	type MaxCidLen = ConstU32<64>;
	type Escrow = pallet_escrow::Pallet<Runtime>;
	type WeightInfo = pallet_arbitration::weights::SubstrateWeight<Runtime>;
	type Router = UnifiedArbitrationRouter;
	type DecisionOrigin = pallet_collective::EnsureProportionAtLeast<AccountId, ArbitrationCollectiveInstance, 2, 3>;
	type Fungible = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type DepositRatioBps = ConstU16<1500>; // 15% 押金比例
	type ResponseDeadline = ConstU32<{ 7 * DAYS }>; // 7天应诉期限
	type RejectedSlashBps = ConstU16<3000>; // 驳回时罚没30%
	type PartialSlashBps = ConstU16<5000>; // 部分胜诉罚没50%
	type ComplaintDeposit = ConstU128<{ UNIT / 10 }>; // 投诉押金兜底值 0.1 NEX
	type ComplaintDepositUsd = ConstU64<1_000_000>; // 投诉押金 1 USDT（精度10^6，使用pricing换算）
	type Pricing = TradingPricingProvider; // 定价接口
	type ComplaintSlashBps = ConstU16<5000>; // 投诉败诉罚没50%
	type TreasuryAccount = TreasuryAccountId;
	// 🆕 P2: CID 锁定管理器
	type CidLockManager = pallet_storage_service::Pallet<Runtime>;
	// 🆕 信用分更新器
	type CreditUpdater = TradingCreditUpdater;
	// 🆕 防膨胀: 归档记录 TTL (180天 ≈ 2_592_000 blocks)
	type ArchiveTtlBlocks = ConstU32<2_592_000>;
}

// ============================================================================
// Governance: Collective (Committees) Configuration
// ============================================================================

// -------------------- 1. 技术委员会 (Technical Committee) --------------------
// 职责：紧急升级、runtime 参数调整、技术提案审核

pub type TechnicalCollectiveInstance = pallet_collective::Instance1;

parameter_types! {
	pub const TechnicalMotionDuration: BlockNumber = 7 * DAYS;
	pub const TechnicalMaxProposals: u32 = 100;
	pub const TechnicalMaxMembers: u32 = 11;
	pub MaxTechnicalProposalWeight: Weight = Perbill::from_percent(50) * RuntimeBlockWeights::get().max_block;
}

impl pallet_collective::Config<TechnicalCollectiveInstance> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = TechnicalMotionDuration;
	type MaxProposals = TechnicalMaxProposals;
	type MaxMembers = TechnicalMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
	type SetMembersOrigin = frame_system::EnsureRoot<AccountId>;
	type MaxProposalWeight = MaxTechnicalProposalWeight;
	type DisapproveOrigin = frame_system::EnsureRoot<AccountId>;
	type KillOrigin = frame_system::EnsureRoot<AccountId>;
	type Consideration = ();
}

// -------------------- 2. 仲裁委员会 (Arbitration Committee) --------------------
// 职责：处理 OTC/Bridge/供奉订单的争议裁决

pub type ArbitrationCollectiveInstance = pallet_collective::Instance2;

parameter_types! {
	pub const ArbitrationMotionDuration: BlockNumber = 3 * DAYS;
	pub const ArbitrationMaxProposals: u32 = 200;
	pub const ArbitrationMaxMembers: u32 = 15;
	pub MaxArbitrationProposalWeight: Weight = Perbill::from_percent(50) * RuntimeBlockWeights::get().max_block;
}

impl pallet_collective::Config<ArbitrationCollectiveInstance> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = ArbitrationMotionDuration;
	type MaxProposals = ArbitrationMaxProposals;
	type MaxMembers = ArbitrationMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
	type SetMembersOrigin = frame_system::EnsureRoot<AccountId>;
	type MaxProposalWeight = MaxArbitrationProposalWeight;
	type DisapproveOrigin = frame_system::EnsureRoot<AccountId>;
	type KillOrigin = frame_system::EnsureRoot<AccountId>;
	type Consideration = ();
}

// -------------------- 3. 财务委员会 (Treasury Council) --------------------
// 职责：审批国库支出、资金分配、生态激励

pub type TreasuryCollectiveInstance = pallet_collective::Instance3;

parameter_types! {
	pub const TreasuryMotionDuration: BlockNumber = 5 * DAYS;
	pub const TreasuryMaxProposals: u32 = 50;
	pub const TreasuryMaxMembers: u32 = 9;
	pub MaxTreasuryProposalWeight: Weight = Perbill::from_percent(50) * RuntimeBlockWeights::get().max_block;
}

impl pallet_collective::Config<TreasuryCollectiveInstance> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = TreasuryMotionDuration;
	type MaxProposals = TreasuryMaxProposals;
	type MaxMembers = TreasuryMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
	type SetMembersOrigin = frame_system::EnsureRoot<AccountId>;
	type MaxProposalWeight = MaxTreasuryProposalWeight;
	type DisapproveOrigin = frame_system::EnsureRoot<AccountId>;
	type KillOrigin = frame_system::EnsureRoot<AccountId>;
	type Consideration = ();
}

// -------------------- 4. 内容委员会 (Content Committee) --------------------
// 职责：审核直播内容合规、证据真实性

pub type ContentCollectiveInstance = pallet_collective::Instance4;

parameter_types! {
	pub const ContentMotionDuration: BlockNumber = 2 * DAYS;
	pub const ContentMaxProposals: u32 = 100;
	pub const ContentMaxMembers: u32 = 7;
	pub MaxContentProposalWeight: Weight = Perbill::from_percent(50) * RuntimeBlockWeights::get().max_block;
}

impl pallet_collective::Config<ContentCollectiveInstance> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = ContentMotionDuration;
	type MaxProposals = ContentMaxProposals;
	type MaxMembers = ContentMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
	type SetMembersOrigin = frame_system::EnsureRoot<AccountId>;
	type MaxProposalWeight = MaxContentProposalWeight;
	type DisapproveOrigin = frame_system::EnsureRoot<AccountId>;
	type KillOrigin = frame_system::EnsureRoot<AccountId>;
	type Consideration = ();
}

// -------------------- Membership Pallets for Committees --------------------

// 技术委员会成员管理
pub type TechnicalMembershipInstance = pallet_collective_membership::Instance1;

impl pallet_collective_membership::Config<TechnicalMembershipInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = frame_system::EnsureRoot<AccountId>;
	type RemoveOrigin = frame_system::EnsureRoot<AccountId>;
	type SwapOrigin = frame_system::EnsureRoot<AccountId>;
	type ResetOrigin = frame_system::EnsureRoot<AccountId>;
	type PrimeOrigin = frame_system::EnsureRoot<AccountId>;
	type MembershipInitialized = TechnicalCommittee;
	type MembershipChanged = TechnicalCommittee;
	type MaxMembers = TechnicalMaxMembers;
	type WeightInfo = pallet_collective_membership::weights::SubstrateWeight<Runtime>;
}

// 仲裁委员会成员管理
pub type ArbitrationMembershipInstance = pallet_collective_membership::Instance2;

impl pallet_collective_membership::Config<ArbitrationMembershipInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = frame_system::EnsureRoot<AccountId>;
	type RemoveOrigin = frame_system::EnsureRoot<AccountId>;
	type SwapOrigin = frame_system::EnsureRoot<AccountId>;
	type ResetOrigin = frame_system::EnsureRoot<AccountId>;
	type PrimeOrigin = frame_system::EnsureRoot<AccountId>;
	type MembershipInitialized = ArbitrationCommittee;
	type MembershipChanged = ArbitrationCommittee;
	type MaxMembers = ArbitrationMaxMembers;
	type WeightInfo = pallet_collective_membership::weights::SubstrateWeight<Runtime>;
}

// 财务委员会成员管理
pub type TreasuryMembershipInstance = pallet_collective_membership::Instance3;

impl pallet_collective_membership::Config<TreasuryMembershipInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = frame_system::EnsureRoot<AccountId>;
	type RemoveOrigin = frame_system::EnsureRoot<AccountId>;
	type SwapOrigin = frame_system::EnsureRoot<AccountId>;
	type ResetOrigin = frame_system::EnsureRoot<AccountId>;
	type PrimeOrigin = frame_system::EnsureRoot<AccountId>;
	type MembershipInitialized = TreasuryCouncil;
	type MembershipChanged = TreasuryCouncil;
	type MaxMembers = TreasuryMaxMembers;
	type WeightInfo = pallet_collective_membership::weights::SubstrateWeight<Runtime>;
}

// 内容委员会成员管理
pub type ContentMembershipInstance = pallet_collective_membership::Instance4;

impl pallet_collective_membership::Config<ContentMembershipInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = frame_system::EnsureRoot<AccountId>;
	type RemoveOrigin = frame_system::EnsureRoot<AccountId>;
	type SwapOrigin = frame_system::EnsureRoot<AccountId>;
	type ResetOrigin = frame_system::EnsureRoot<AccountId>;
	type PrimeOrigin = frame_system::EnsureRoot<AccountId>;
	type MembershipInitialized = ContentCommittee;
	type MembershipChanged = ContentCommittee;
	type MaxMembers = ContentMaxMembers;
	type WeightInfo = pallet_collective_membership::weights::SubstrateWeight<Runtime>;
}

// ============================================================================
// Storage Lifecycle Pallet Configuration
// ============================================================================

impl pallet_storage_lifecycle::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type L1ArchiveDelay = ConstU32<{ 30 * DAYS }>;  // 30天后归档到L1
	type L2ArchiveDelay = ConstU32<{ 90 * DAYS }>;  // L1后90天归档到L2
	type PurgeDelay = ConstU32<{ 180 * DAYS }>;     // L2后180天可清除
	type EnablePurge = ConstBool<false>;             // 默认不启用清除
	type MaxBatchSize = ConstU32<100>;               // 每次最多处理100条
	type StorageArchiver = ();                       // 空实现（P0 on_finalize 已处理清理）
}

// ============================================================================
// Contracts Pallet Configuration
// ============================================================================

parameter_types! {
	pub ContractsSchedule: pallet_contracts::Schedule<Runtime> = Default::default();
	pub const CodeHashLockupDepositPercent: Perbill = Perbill::from_percent(30);
}

/// 随机数源（使用确定性随机）
pub struct DummyRandomness;
impl frame_support::traits::Randomness<Hash, BlockNumber> for DummyRandomness {
	fn random(subject: &[u8]) -> (Hash, BlockNumber) {
		use sp_runtime::traits::Hash as HashT;
		let block_number = System::block_number();
		let hash = <Runtime as frame_system::Config>::Hashing::hash(subject);
		(hash, block_number)
	}
}

impl pallet_contracts::Config for Runtime {
	type Time = Timestamp;
	type Randomness = DummyRandomness;
	type Currency = Balances;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;

	/// 合约调用栈深度限制
	type CallStack = [pallet_contracts::Frame<Self>; 23];

	/// 合约存储押金（每字节）
	type DepositPerByte = ConstU128<{ UNIT / 1000 }>;
	/// 合约存储押金（每个存储项）
	type DepositPerItem = ConstU128<{ UNIT / 100 }>;
	/// 默认存款限制
	type DefaultDepositLimit = ConstU128<{ 100 * UNIT }>;

	/// 代码哈希锁定押金百分比
	type CodeHashLockupDepositPercent = CodeHashLockupDepositPercent;

	/// 权重信息
	type WeightPrice = pallet_transaction_payment::Pallet<Self>;
	type WeightInfo = pallet_contracts::weights::SubstrateWeight<Self>;

	/// 链扩展（无）
	type ChainExtension = ();

	/// 调度表
	type Schedule = ContractsSchedule;

	/// 地址生成器
	type AddressGenerator = pallet_contracts::DefaultAddressGenerator;

	/// 最大代码长度
	type MaxCodeLen = ConstU32<{ 256 * 1024 }>;
	/// 最大存储键长度
	type MaxStorageKeyLen = ConstU32<128>;

	/// 不安全的不稳定接口（生产环境应禁用）
	type UnsafeUnstableInterface = ConstBool<false>;

	/// 上传来源
	type UploadOrigin = frame_system::EnsureSigned<AccountId>;
	/// 实例化来源
	type InstantiateOrigin = frame_system::EnsureSigned<AccountId>;

	/// 最大调试缓冲区长度
	type MaxDebugBufferLen = ConstU32<{ 2 * 1024 * 1024 }>;

	/// 最大委托依赖数
	type MaxDelegateDependencies = ConstU32<32>;

	/// 运行时 Hold 原因
	type RuntimeHoldReason = RuntimeHoldReason;

	/// 环境类型
	type Environment = ();

	/// API 版本
	type ApiVersion = ();

	/// Xcm 相关（不使用）
	type Xcm = ();

	/// 迁移
	type Migrations = ();

	/// 调试
	type Debug = ();

	/// 调用过滤器（允许所有调用）
	type CallFilter = frame_support::traits::Everything;

	/// 最大瞬态存储大小
	type MaxTransientStorageSize = ConstU32<{ 1024 * 1024 }>;
}

// ============================================================================
// Assets Configuration (for ShareMall Token)
// ============================================================================

parameter_types! {
	/// 创建资产押金: 100 COS
	pub const AssetDeposit: Balance = 100 * UNIT;
	/// 账户持有资产押金: 1 COS
	pub const AssetAccountDeposit: Balance = UNIT;
	/// 元数据押金基础: 10 COS
	pub const MetadataDepositBase: Balance = 10 * UNIT;
	/// 元数据押金每字节: 0.1 NEX
	pub const MetadataDepositPerByte: Balance = UNIT / 10;
	/// 授权押金: 1 COS
	pub const ApprovalDeposit: Balance = UNIT;
	/// 字符串长度限制
	pub const AssetsStringLimit: u32 = 50;
}

impl pallet_assets::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type AssetId = u64;
	type AssetIdParameter = codec::Compact<u64>;
	type Currency = Balances;
	type CreateOrigin = frame_support::traits::AsEnsureOriginWithArg<frame_system::EnsureSigned<AccountId>>;
	type ForceOrigin = EnsureRoot<AccountId>;
	type AssetDeposit = AssetDeposit;
	type AssetAccountDeposit = AssetAccountDeposit;
	type MetadataDepositBase = MetadataDepositBase;
	type MetadataDepositPerByte = MetadataDepositPerByte;
	type ApprovalDeposit = ApprovalDeposit;
	type StringLimit = AssetsStringLimit;
	type Freezer = ();
	type Extra = ();
	type CallbackHandle = ();
	type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
	type RemoveItemsLimit = ConstU32<1000>;
	type ReserveData = ();
	type Holder = ();
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

// ============================================================================
// Entity Configuration (原 ShareMall，已重构)
// ============================================================================

parameter_types! {
	/// 最低实体保证金: 100 COS
	pub const EntityMinDeposit: Balance = 100 * UNIT;
	/// 平台费率: 2% (200 基点)
	pub const EntityPlatformFeeRate: u16 = 200;
	/// 发货超时: 约 3 天 (假设 6 秒一个块)
	pub const EntityShipTimeout: BlockNumber = 43200;
	/// 确认收货超时: 约 7 天
	pub const EntityConfirmTimeout: BlockNumber = 100800;
	/// 实体代币 ID 偏移量
	pub const EntityTokenOffset: u64 = 1_000_000;
	/// 投票期: 7 天
	pub const GovernanceVotingPeriod: BlockNumber = 100800;
	/// 执行延迟: 2 天
	pub const GovernanceExecutionDelay: BlockNumber = 28800;
	/// 通过阈值: 50%
	pub const GovernancePassThreshold: u8 = 50;
	/// 法定人数: 10%
	pub const GovernanceQuorumThreshold: u8 = 10;
	/// 创建提案所需最低代币持有比例: 1%
	pub const GovernanceMinProposalThreshold: u16 = 100;
	/// C3: 最小投票期: 1 天
	pub const GovernanceMinVotingPeriod: BlockNumber = 14400;
	/// C3: 最小执行延迟: 4 小时
	pub const GovernanceMinExecutionDelay: BlockNumber = 2400;
}

/// 平台账户
pub struct EntityPlatformAccount;
impl frame_support::traits::Get<AccountId> for EntityPlatformAccount {
	fn get() -> AccountId {
		frame_support::PalletId(*b"entity//").into_account_truncating()
	}
}

impl pallet_entity_registry::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type MaxEntityNameLength = ConstU32<64>;
	type MaxCidLength = ConstU32<64>;
	type GovernanceOrigin = EnsureRoot<AccountId>;
	type PricingProvider = EntityPricingProvider;
	type InitialFundUsdt = ConstU64<50_000_000>;  // 50 USDT
	type MinInitialFundCos = EntityMinDeposit;
	type MaxInitialFundCos = ConstU128<{ 1000 * UNIT }>;
	type MinOperatingBalance = ConstU128<{ UNIT / 10 }>;
	type FundWarningThreshold = ConstU128<{ UNIT }>;
	type MaxAdmins = ConstU32<10>;
	type MaxEntitiesPerUser = ConstU32<3>;
	type ShopProvider = EntityShop;
	type MaxShopsPerEntity = ConstU32<16>;
	type PlatformAccount = EntityPlatformAccount;
}

impl pallet_entity_shop::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type EntityProvider = EntityRegistry;
	type MaxShopNameLength = ConstU32<64>;
	type MaxCidLength = ConstU32<64>;
	type MaxManagers = ConstU32<10>;
	type MaxPointsNameLength = ConstU32<32>;
	type MaxPointsSymbolLength = ConstU32<8>;
	type MinOperatingBalance = ConstU128<{ UNIT / 10 }>;
	type WarningThreshold = ConstU128<{ UNIT }>;
	type CommissionFundGuard = crate::CommissionCore;
}

impl pallet_entity_service::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type EntityProvider = EntityRegistry;
	type ShopProvider = EntityShop;
	type PricingProvider = EntityPricingProvider;
	type MaxProductsPerShop = ConstU32<1000>;
	type MaxCidLength = ConstU32<64>;
	type ProductDepositUsdt = ConstU64<1_000_000>;  // 1 USDT
	type MinProductDepositCos = ConstU128<{ UNIT / 100 }>;
	type MaxProductDepositCos = ConstU128<{ 10 * UNIT }>;
}

impl pallet_entity_transaction::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type Escrow = Escrow;
	type ShopProvider = EntityShop;
	type ProductProvider = EntityService;
	type EntityToken = EntityToken;
	type PlatformAccount = EntityPlatformAccount;
	type PlatformFeeRate = EntityPlatformFeeRate;
	type ShipTimeout = EntityShipTimeout;
	type ConfirmTimeout = EntityConfirmTimeout;
	type ServiceConfirmTimeout = ConstU32<{ 7 * 24 * 600 }>;  // 7 天
	type CommissionHandler = OrderCommissionBridge;
	type MaxCidLength = ConstU32<64>;
}

impl pallet_entity_review::Config for Runtime {
	type OrderProvider = EntityTransaction;
	type ShopProvider = EntityShop;
	type MaxCidLength = ConstU32<64>;
	type WeightInfo = pallet_entity_review::weights::SubstrateWeight<Runtime>;
}

// 使用 pallet-entity-token 内置的 Null 实现
pub type TokenKycProvider = pallet_entity_token::pallet::NullKycProvider;
pub type TokenMemberProvider = pallet_entity_token::pallet::NullMemberProvider;

impl pallet_entity_token::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = u64;
	type AssetBalance = Balance;
	type Assets = Assets;
	type EntityProvider = EntityRegistry;
	type ShopProvider = EntityShop;
	type ShopTokenOffset = ConstU64<1_000_000>;  // 店铺代币 ID 从 1,000,000 开始
	type MaxTokenNameLength = ConstU32<64>;
	type MaxTokenSymbolLength = ConstU32<8>;
	type MaxTransferListSize = ConstU32<1000>;
	type MaxDividendRecipients = ConstU32<500>;
	type KycProvider = TokenKycProvider;
	type MemberProvider = TokenMemberProvider;
	type WeightInfo = pallet_entity_token::weights::SubstrateWeight<Runtime>;
}

// EntityTokenProvider 使用 EntityToken pallet 直接实现
pub type EntityTokenProvider = EntityToken;

/// Entity PricingProvider 适配器
pub struct EntityPricingProvider;
impl pallet_entity_common::PricingProvider for EntityPricingProvider {
	fn get_nex_usdt_price() -> u64 {
		// 调用 TradingPricingProvider 获取 COS/USD 汇率（已优先 TWAP）
		<TradingPricingProvider as pallet_trading_common::PricingProvider<Balance>>::get_cos_to_usd_rate()
			.map(|rate| rate as u64)
			.unwrap_or(0)
	}
	fn is_price_stale() -> bool {
		// 超过 2400 区块（~4 小时 @6s/block）无交易则视为过时
		<pallet_nex_market::Pallet<Runtime> as pallet_trading_common::PriceOracle>::is_price_stale(2400)
	}
}

/// 桥接实现：将 EntityMember pallet 桥接到 commission 模块的 MemberProvider trait
pub struct MemberProviderBridge;

impl pallet_entity_commission::MemberProvider<AccountId> for MemberProviderBridge {
	fn is_member(shop_id: u64, account: &AccountId) -> bool {
		pallet_entity_member::Pallet::<Runtime>::is_member_of_shop(shop_id, account)
	}

	fn get_referrer(shop_id: u64, account: &AccountId) -> Option<AccountId> {
		pallet_entity_member::Pallet::<Runtime>::get_member_by_shop(shop_id, account)
			.and_then(|m| m.referrer)
	}

	fn member_level(shop_id: u64, account: &AccountId) -> Option<pallet_entity_common::MemberLevel> {
		pallet_entity_member::Pallet::<Runtime>::get_member_by_shop(shop_id, account)
			.map(|m| m.level)
	}

	fn get_member_stats(shop_id: u64, account: &AccountId) -> (u32, u32, u128) {
		pallet_entity_member::Pallet::<Runtime>::get_member_by_shop(shop_id, account)
			.map(|m| {
				let spent_usdt: u128 = sp_runtime::SaturatedConversion::saturated_into(m.total_spent);
				(m.direct_referrals, m.team_size, spent_usdt)
			})
			.unwrap_or((0, 0, 0))
	}

	fn uses_custom_levels(shop_id: u64) -> bool {
		pallet_entity_member::Pallet::<Runtime>::uses_custom_levels(shop_id)
	}

	fn custom_level_id(shop_id: u64, account: &AccountId) -> u8 {
		// H6 审计修复: 使用 get_effective_level 检查等级过期
		pallet_entity_member::Pallet::<Runtime>::get_effective_level(shop_id, account)
	}

	fn get_level_commission_bonus(shop_id: u64, level_id: u8) -> u16 {
		pallet_entity_member::Pallet::<Runtime>::get_level_commission_bonus(shop_id, level_id)
	}

	fn is_member_by_entity(entity_id: u64, account: &AccountId) -> bool {
		pallet_entity_member::EntityMembers::<Runtime>::contains_key(entity_id, account)
	}

	fn get_referrer_by_entity(entity_id: u64, account: &AccountId) -> Option<AccountId> {
		pallet_entity_member::EntityMembers::<Runtime>::get(entity_id, account)
			.and_then(|m| m.referrer)
	}

	fn custom_level_id_by_entity(entity_id: u64, account: &AccountId) -> u8 {
		pallet_entity_member::Pallet::<Runtime>::get_effective_level_by_entity(entity_id, account)
	}

	fn set_custom_levels_enabled(shop_id: u64, enabled: bool) -> sp_runtime::DispatchResult {
		// C2 审计修复: 解析 shop_id → entity_id（治理传入 shop_id，pallet 期望 entity_id）
		use pallet_entity_common::ShopProvider;
		let entity_id = EntityShop::shop_entity_id(shop_id).unwrap_or(shop_id);
		pallet_entity_member::Pallet::<Runtime>::governance_set_custom_levels_enabled(entity_id, enabled)
	}

	fn set_upgrade_mode(shop_id: u64, mode: u8) -> sp_runtime::DispatchResult {
		use pallet_entity_common::ShopProvider;
		let entity_id = EntityShop::shop_entity_id(shop_id).unwrap_or(shop_id);
		pallet_entity_member::Pallet::<Runtime>::governance_set_upgrade_mode(entity_id, mode)
	}

	fn add_custom_level(
		shop_id: u64,
		_level_id: u8,
		name: &[u8],
		threshold: u128,
		discount_rate: u16,
		commission_bonus: u16,
	) -> sp_runtime::DispatchResult {
		// level_id 由 pallet 自动分配，忽略传入值
		// C2 审计修复: 解析 shop_id → entity_id
		use pallet_entity_common::ShopProvider;
		let entity_id = EntityShop::shop_entity_id(shop_id).unwrap_or(shop_id);
		pallet_entity_member::Pallet::<Runtime>::governance_add_custom_level(
			entity_id, name, threshold, discount_rate, commission_bonus,
		)
	}

	fn update_custom_level(
		shop_id: u64,
		level_id: u8,
		name: Option<&[u8]>,
		threshold: Option<u128>,
		discount_rate: Option<u16>,
		commission_bonus: Option<u16>,
	) -> sp_runtime::DispatchResult {
		// C2 审计修复: 解析 shop_id → entity_id
		use pallet_entity_common::ShopProvider;
		let entity_id = EntityShop::shop_entity_id(shop_id).unwrap_or(shop_id);
		pallet_entity_member::Pallet::<Runtime>::governance_update_custom_level(
			entity_id, level_id, name, threshold, discount_rate, commission_bonus,
		)
	}

	fn remove_custom_level(shop_id: u64, level_id: u8) -> sp_runtime::DispatchResult {
		use pallet_entity_common::ShopProvider;
		let entity_id = EntityShop::shop_entity_id(shop_id).unwrap_or(shop_id);
		pallet_entity_member::Pallet::<Runtime>::governance_remove_custom_level(entity_id, level_id)
	}

	fn custom_level_count(shop_id: u64) -> u8 {
		pallet_entity_member::Pallet::<Runtime>::custom_level_count(shop_id)
	}

	fn auto_register(shop_id: u64, account: &AccountId, referrer: Option<AccountId>) -> sp_runtime::DispatchResult {
		pallet_entity_member::Pallet::<Runtime>::auto_register(shop_id, account, referrer)
	}
}

/// 桥接：OrderCommissionHandler → CommissionProvider（供 Transaction 模块调用）
pub struct OrderCommissionBridge;
impl pallet_entity_common::OrderCommissionHandler<AccountId, Balance> for OrderCommissionBridge {
	fn on_order_completed(
		shop_id: u64,
		order_id: u64,
		buyer: &AccountId,
		order_amount: Balance,
		platform_fee: Balance,
	) -> Result<(), sp_runtime::DispatchError> {
		// available_pool = order_amount，commission 内部会按 max_commission_rate、source 和账户余额封顶
		<pallet_commission_core::Pallet<Runtime> as pallet_commission_common::CommissionProvider<AccountId, Balance>>::process_commission(
			shop_id, order_id, buyer, order_amount, order_amount, platform_fee,
		)
	}
	fn on_order_cancelled(order_id: u64) -> Result<(), sp_runtime::DispatchError> {
		<pallet_commission_core::Pallet<Runtime> as pallet_commission_common::CommissionProvider<AccountId, Balance>>::cancel_commission(order_id)
	}
}

/// 桥接实现：CommissionCore pallet 作为 CommissionProvider
pub type EntityCommissionProvider = pallet_commission_core::Pallet<Runtime>;
pub type EntityMemberProvider = MemberProviderBridge;

impl pallet_entity_governance::Config for Runtime {
	type Balance = Balance;
	type EntityProvider = EntityRegistry;
	type ShopProvider = EntityShop;
	type TokenProvider = EntityTokenProvider;
	type CommissionProvider = EntityCommissionProvider;
	type MemberProvider = EntityMemberProvider;
	type VotingPeriod = GovernanceVotingPeriod;
	type ExecutionDelay = GovernanceExecutionDelay;
	type PassThreshold = GovernancePassThreshold;
	type QuorumThreshold = GovernanceQuorumThreshold;
	type MinProposalThreshold = GovernanceMinProposalThreshold;
	type MaxTitleLength = ConstU32<128>;
	type MaxCidLength = ConstU32<64>;
	type MaxActiveProposals = ConstU32<10>;
	type MinVotingPeriod = GovernanceMinVotingPeriod;
	type MinExecutionDelay = GovernanceMinExecutionDelay;
	type TimeWeightFullPeriod = ConstU32<{ 30 * DAYS }>;  // 30 天达到最大乘数
	type TimeWeightMaxMultiplier = ConstU32<30000>;        // 最大 3x 投票权
}

impl pallet_entity_member::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type EntityProvider = EntityRegistry;
	type ShopProvider = EntityShop;
	type MaxDirectReferrals = ConstU32<1000>;
	type MaxCustomLevels = ConstU32<10>;
	type SilverThreshold = ConstU64<100_000_000>;    // 100 USDT
	type GoldThreshold = ConstU64<500_000_000>;      // 500 USDT
	type PlatinumThreshold = ConstU64<2000_000_000>; // 2000 USDT
	type DiamondThreshold = ConstU64<10000_000_000>; // 10000 USDT
	type MaxUpgradeRules = ConstU32<50>;
	type MaxUpgradeHistory = ConstU32<100>;
}

/// 桥接：EntityReferrerProvider → EntityRegistry::entity_referrer()
pub struct EntityReferrerBridge;

impl pallet_commission_common::EntityReferrerProvider<AccountId> for EntityReferrerBridge {
	fn entity_referrer(entity_id: u64) -> Option<AccountId> {
		pallet_entity_registry::EntityReferrer::<Runtime>::get(entity_id)
	}
}

impl pallet_commission_core::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type ShopProvider = EntityShop;
	type EntityProvider = EntityRegistry;
	type MemberProvider = EntityMemberProvider;
	type ReferralPlugin = crate::CommissionReferral;
	type LevelDiffPlugin = crate::CommissionLevelDiff;
	type SingleLinePlugin = crate::CommissionSingleLine;
	type TeamPlugin = crate::CommissionTeam;
	type ReferralWriter = crate::CommissionReferral;
	type LevelDiffWriter = crate::CommissionLevelDiff;
	type TeamWriter = crate::CommissionTeam;
	type EntityReferrerProvider = EntityReferrerBridge;
	type PlatformAccount = EntityPlatformAccount;
	type TreasuryAccount = TreasuryAccountId;
	type ReferrerShareBps = ConstU16<5000>; // 50% of platform fee → referrer
	type MaxCommissionRecordsPerOrder = ConstU32<20>;
	type MaxCustomLevels = ConstU32<10>;
}

impl pallet_commission_referral::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type MemberProvider = EntityMemberProvider;
	type MaxMultiLevels = ConstU32<15>;
}

impl pallet_commission_level_diff::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type MemberProvider = EntityMemberProvider;
	type MaxCustomLevels = ConstU32<10>;
}

/// 桥接：CommissionCore 的 MemberCommissionStats 作为 SingleLine 的 StatsProvider
pub struct SingleLineStatsFromCore;

impl pallet_commission_single_line::pallet::SingleLineStatsProvider<AccountId, Balance> for SingleLineStatsFromCore {
	fn get_member_stats(shop_id: u64, account: &AccountId) -> pallet_commission_common::MemberCommissionStatsData<Balance> {
		use pallet_entity_common::ShopProvider;
		// M1 审计修复: shop_id 无效时返回默认空统计，避免读取 entity_id=0 的错误数据
		match EntityShop::shop_entity_id(shop_id) {
			Some(entity_id) => pallet_commission_core::MemberCommissionStats::<Runtime>::get(entity_id, account),
			None => Default::default(),
		}
	}
}

impl pallet_commission_team::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type MemberProvider = EntityMemberProvider;
	type MaxTeamTiers = ConstU32<10>;
}

impl pallet_commission_single_line::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type StatsProvider = SingleLineStatsFromCore;
	type MaxSingleLineLength = ConstU32<50>;
}

parameter_types! {
	pub MarketTreasuryAccount: AccountId = AccountId::from([0u8; 32]);
}

impl pallet_entity_market::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type Balance = Balance;
	type TokenBalance = Balance;
	type EntityProvider = EntityRegistry;
	type TokenProvider = EntityToken;
	type DefaultOrderTTL = ConstU32<{ 7 * 24 * 600 }>;  // 7 天
	type MaxActiveOrdersPerUser = ConstU32<100>;
	type DefaultFeeRate = ConstU16<30>;  // 0.3%
	type DefaultUsdtTimeout = ConstU32<{ 2 * 600 }>;  // 2 小时
	type BlocksPerHour = ConstU32<600>;
	type BlocksPerDay = ConstU32<{ 24 * 600 }>;
	type BlocksPerWeek = ConstU32<{ 7 * 24 * 600 }>;
	type CircuitBreakerDuration = ConstU32<600>;  // 1 小时
	type VerificationReward = ConstU128<{ UNIT / 10 }>;  // 0.1 NEX
	type RewardSource = MarketTreasuryAccount;
	type BuyerDepositRate = ConstU16<1000>;  // 10%
	type MinBuyerDeposit = ConstU128<{ UNIT }>;  // 1 COS
	type DepositForfeitRate = ConstU16<5000>;  // 50%
	type UsdtToNexRate = ConstU64<100_000>;  // 1 USDT = 0.1 NEX
	type TreasuryAccount = MarketTreasuryAccount;
	type VerificationGracePeriod = ConstU32<600>;  // 1 小时
	type UnderpaidGracePeriod = ConstU32<{ 2 * 600 }>;  // 2 小时
	type NexUsdtPrice = EntityPricingProvider;
}

// ============================================================================
// Entity Pallets Config (Phase 6-8 新模块)
// ============================================================================

parameter_types! {
	// KYC 有效期
	pub const BasicKycValidity: BlockNumber = 525600;      // ~1 年
	pub const StandardKycValidity: BlockNumber = 262800;   // ~6 个月
	pub const EnhancedKycValidity: BlockNumber = 525600;   // ~1 年
	pub const InstitutionalKycValidity: BlockNumber = 1051200; // ~2 年
	// 披露间隔
	pub const BasicDisclosureInterval: BlockNumber = 5256000;    // ~1 年
	pub const StandardDisclosureInterval: BlockNumber = 1314000; // ~3 个月
	pub const EnhancedDisclosureInterval: BlockNumber = 438000;  // ~1 个月
	pub const MaxBlackoutDuration: BlockNumber = 100800;         // ~7 天 @ 6s/block
}

impl pallet_entity_disclosure::Config for Runtime {
	type EntityProvider = EntityRegistry;
	type MaxCidLength = ConstU32<64>;
	type MaxInsiders = ConstU32<50>;
	type MaxDisclosureHistory = ConstU32<100>;
	type BasicDisclosureInterval = BasicDisclosureInterval;
	type StandardDisclosureInterval = StandardDisclosureInterval;
	type EnhancedDisclosureInterval = EnhancedDisclosureInterval;
	type MajorHolderThreshold = ConstU16<500>; // 5%
	type MaxBlackoutDuration = MaxBlackoutDuration;
	type MaxAnnouncementHistory = ConstU32<200>;
	type MaxTitleLength = ConstU32<128>;
}

impl pallet_entity_kyc::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MaxCidLength = ConstU32<64>;
	type MaxProviderNameLength = ConstU32<64>;
	type MaxProviders = ConstU32<20>;
	type BasicKycValidity = BasicKycValidity;
	type StandardKycValidity = StandardKycValidity;
	type EnhancedKycValidity = EnhancedKycValidity;
	type InstitutionalKycValidity = InstitutionalKycValidity;
	type AdminOrigin = EnsureRoot<AccountId>;
}

/// TokenSale KYC 适配器（桥接 pallet-entity-kyc → KycChecker trait）
pub struct TokenSaleKycBridge;
impl pallet_entity_tokensale::KycChecker<AccountId> for TokenSaleKycBridge {
	fn kyc_level(account: &AccountId) -> u8 {
		use pallet_entity_kyc::pallet::KycLevel;
		match EntityKyc::get_kyc_level(account) {
			KycLevel::None => 0,
			KycLevel::Basic => 1,
			KycLevel::Standard => 2,
			KycLevel::Enhanced => 3,
			KycLevel::Institutional => 4,
		}
	}
}

impl pallet_entity_tokensale::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type AssetId = u64;
	type EntityProvider = EntityRegistry;
	type TokenProvider = EntityToken;
	type KycChecker = TokenSaleKycBridge;
	type MaxPaymentOptions = ConstU32<5>;
	type MaxWhitelistSize = ConstU32<1000>;
	type MaxRoundsHistory = ConstU32<50>;
	type MaxSubscriptionsPerRound = ConstU32<10000>;
	type MaxActiveRounds = ConstU32<20>;
	type RefundGracePeriod = ConstU32<{ 7 * DAYS }>;
}

// ============================================================================
// GroupRobot Pallets Config
// ============================================================================

parameter_types! {
	/// TEE 证明有效期 ~24h @ 6s/block = 43200
	pub const GrAttestationValidityBlocks: BlockNumber = 43_200;
	/// 证明过期扫描间隔: 每 100 区块 (~10 分钟)
	pub const GrAttestationCheckInterval: BlockNumber = 100;
	/// 节点最低质押: 1000 NEX
	pub const GrMinNodeStake: Balance = 1000 * UNIT;
	/// 节点退出冷却期: 1 天
	pub const GrExitCooldown: BlockNumber = DAYS;
	/// Era 长度: 1 天
	pub const GrEraLength: BlockNumber = DAYS;
	/// 每 Era 通胀铸币: 100 NEX (Phase 0)
	pub const GrInflationPerEra: Balance = 100 * UNIT;
	/// Basic 层级每 Era 费用: ~0.33 NEX/天
	pub const GrBasicFeePerEra: Balance = 10 * UNIT / 30;
	/// Pro 层级每 Era 费用: 1 NEX/天
	pub const GrProFeePerEra: Balance = 30 * UNIT / 30;
	/// Enterprise 层级每 Era 费用: ~3.33 NEX/天
	pub const GrEnterpriseFeePerEra: Balance = 100 * UNIT / 30;
	/// 仪式有效期: 180 天
	pub const GrCeremonyValidityBlocks: BlockNumber = 180 * DAYS;
	/// 仪式检查间隔: 每 1000 区块 (~100 分钟)
	pub const GrCeremonyCheckInterval: BlockNumber = 1000;
	/// Peer 心跳过期阈值: 2 小时 (~1200 区块 @ 6s/block)
	pub const GrPeerHeartbeatTimeout: BlockNumber = 1200;
}

impl pallet_grouprobot_registry::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MaxBotsPerOwner = ConstU32<20>;
	type MaxPlatformsPerCommunity = ConstU32<5>;
	type MaxPlatformBindingsPerUser = ConstU32<5>;
	type AttestationValidityBlocks = GrAttestationValidityBlocks;
	type AttestationCheckInterval = GrAttestationCheckInterval;
	type MaxQuoteLen = ConstU32<8192>;
	type MaxPeersPerBot = ConstU32<16>;
	type MaxEndpointLen = ConstU32<256>;
	type PeerHeartbeatTimeout = GrPeerHeartbeatTimeout;
	type Subscription = pallet_grouprobot_subscription::Pallet<Runtime>;
}

/// GroupRobot BotRegistry Bridge: 将 pallet-grouprobot-registry 桥接到 BotRegistryProvider trait
pub struct GrBotRegistryBridge;

impl pallet_grouprobot_primitives::BotRegistryProvider<AccountId> for GrBotRegistryBridge {
	fn is_bot_active(bot_id_hash: &[u8; 32]) -> bool {
		pallet_grouprobot_registry::Pallet::<Runtime>::is_bot_active(bot_id_hash)
	}
	fn is_tee_node(bot_id_hash: &[u8; 32]) -> bool {
		pallet_grouprobot_registry::Pallet::<Runtime>::is_tee_node(bot_id_hash)
	}
	fn has_dual_attestation(bot_id_hash: &[u8; 32]) -> bool {
		pallet_grouprobot_registry::Pallet::<Runtime>::has_dual_attestation(bot_id_hash)
	}
	fn is_attestation_fresh(bot_id_hash: &[u8; 32]) -> bool {
		pallet_grouprobot_registry::Pallet::<Runtime>::is_attestation_fresh(bot_id_hash)
	}
	fn bot_owner(bot_id_hash: &[u8; 32]) -> Option<AccountId> {
		pallet_grouprobot_registry::Pallet::<Runtime>::bot_owner(bot_id_hash)
	}
	fn bot_public_key(bot_id_hash: &[u8; 32]) -> Option<[u8; 32]> {
		pallet_grouprobot_registry::Pallet::<Runtime>::bot_public_key(bot_id_hash)
	}
	fn peer_count(bot_id_hash: &[u8; 32]) -> u32 {
		pallet_grouprobot_registry::Pallet::<Runtime>::peer_count(bot_id_hash)
	}
}

parameter_types! {
	pub const GrSequenceTtlBlocks: BlockNumber = 14_400; // ~24h (6s/block)
	pub const GrMaxEraHistory: u64 = 365;               // 保留最近 365 个 Era
}

impl pallet_grouprobot_consensus::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type MaxActiveNodes = ConstU32<100>;
	type MinStake = GrMinNodeStake;
	type ExitCooldownPeriod = GrExitCooldown;
	type EraLength = GrEraLength;
	type InflationPerEra = GrInflationPerEra;
	type SlashPercentage = ConstU32<10>;
	type BotRegistry = GrBotRegistryBridge;
	type SequenceTtlBlocks = GrSequenceTtlBlocks;
	type MaxSequenceCleanupPerBlock = ConstU32<20>;
	type SubscriptionSettler = pallet_grouprobot_subscription::Pallet<Runtime>;
	type RewardDistributor = pallet_grouprobot_rewards::Pallet<Runtime>;
	type Subscription = pallet_grouprobot_subscription::Pallet<Runtime>;
}

/// 订阅 EraStartBlock 提供者: 从 consensus 读取
pub struct GrEraStartBlockProvider;
impl frame_support::traits::Get<BlockNumber> for GrEraStartBlockProvider {
	fn get() -> BlockNumber {
		pallet_grouprobot_consensus::EraStartBlock::<Runtime>::get()
	}
}

/// 当前 Era 提供者: 从 consensus 读取
pub struct GrCurrentEraProvider;
impl frame_support::traits::Get<u64> for GrCurrentEraProvider {
	fn get() -> u64 {
		pallet_grouprobot_consensus::CurrentEra::<Runtime>::get()
	}
}

impl pallet_grouprobot_subscription::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type BotRegistry = GrBotRegistryBridge;
	type BasicFeePerEra = GrBasicFeePerEra;
	type ProFeePerEra = GrProFeePerEra;
	type EnterpriseFeePerEra = GrEnterpriseFeePerEra;
	type TreasuryAccount = TreasuryAccountId;
	type MaxSubscriptionSettlePerEra = ConstU32<200>;
	type EraLength = GrEraLength;
	type EraStartBlockProvider = GrEraStartBlockProvider;
	type CurrentEraProvider = GrCurrentEraProvider;
}

/// NodeConsensus Bridge: 从 consensus pallet 读取节点信息
pub struct GrNodeConsensusBridge;
impl pallet_grouprobot_primitives::NodeConsensusProvider<AccountId> for GrNodeConsensusBridge {
	fn is_node_active(node_id: &[u8; 32]) -> bool {
		pallet_grouprobot_consensus::Pallet::<Runtime>::is_node_active(node_id)
	}
	fn node_operator(node_id: &[u8; 32]) -> Option<AccountId> {
		pallet_grouprobot_consensus::Pallet::<Runtime>::node_operator(node_id)
	}
	fn is_tee_node_by_operator(operator: &AccountId) -> bool {
		pallet_grouprobot_consensus::Pallet::<Runtime>::is_tee_node_by_operator(operator)
	}
}

impl pallet_grouprobot_rewards::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type NodeConsensus = GrNodeConsensusBridge;
	type MaxEraHistory = GrMaxEraHistory;
}

impl pallet_grouprobot_community::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MaxLogsPerCommunity = ConstU32<10000>;
	type ReputationCooldown = ConstU32<100>;
	type MaxReputationDelta = ConstU32<1000>;
	type BotRegistry = GrBotRegistryBridge;
	type Subscription = pallet_grouprobot_subscription::Pallet<Runtime>;
}

impl pallet_grouprobot_ceremony::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MaxParticipants = ConstU32<20>;
	type MaxCeremonyHistory = ConstU32<10>;
	type CeremonyValidityBlocks = GrCeremonyValidityBlocks;
	type CeremonyCheckInterval = GrCeremonyCheckInterval;
	type BotRegistry = GrBotRegistryBridge;
	type Subscription = pallet_grouprobot_subscription::Pallet<Runtime>;
}
