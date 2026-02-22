//! # P2P Pallet Mock Runtime
//!
//! 单元测试用 mock 环境。

use crate as pallet_trading_p2p;
use frame_support::{
    parameter_types,
    traits::{ConstU16, ConstU32, ConstU64},
};
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};
use frame_support::dispatch::DispatchResult;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        P2pTrading: pallet_trading_p2p,
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ConstU16<42>;
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
    type RuntimeTask = ();
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
    type ExtensionsWeightInfo = ();
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
}

impl pallet_balances::Config for Test {
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type Balance = u128;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type DoneSlashHandler = ();
}

// ==================== Stub Implementations ====================

/// Mock Escrow: 所有操作成功
pub struct MockEscrow;
impl pallet_escrow::Escrow<u64, u128> for MockEscrow {
    fn escrow_account() -> u64 { 999 }
    fn lock_from(_payer: &u64, _id: u64, _amount: u128) -> DispatchResult { Ok(()) }
    fn transfer_from_escrow(_id: u64, _to: &u64, _amount: u128) -> DispatchResult { Ok(()) }
    fn release_all(_id: u64, _to: &u64) -> DispatchResult { Ok(()) }
    fn refund_all(_id: u64, _to: &u64) -> DispatchResult { Ok(()) }
    fn amount_of(_id: u64) -> u128 { 1_000_000_000_000 }
    fn split_partial(_id: u64, _release_to: &u64, _refund_to: &u64, _bps: u16) -> DispatchResult { Ok(()) }
}

/// Mock BuyerCredit: 所有检查通过
pub struct MockBuyerCredit;
impl pallet_trading_credit::BuyerCreditInterface<u64> for MockBuyerCredit {
    fn get_buyer_credit_score(_buyer: &u64) -> Result<u16, sp_runtime::DispatchError> { Ok(500) }
    fn check_buyer_daily_limit(_buyer: &u64, _amount: u64) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn check_buyer_single_limit(_buyer: &u64, _amount: u64) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
}

impl pallet_trading_credit::quota::BuyerQuotaInterface<u64> for MockBuyerCredit {
    fn get_available_quota(_buyer: &u64) -> Result<u64, sp_runtime::DispatchError> { Ok(1_000_000_000) }
    fn occupy_quota(_buyer: &u64, _amount: u64) -> DispatchResult { Ok(()) }
    fn release_quota(_buyer: &u64, _amount: u64) -> DispatchResult { Ok(()) }
    fn check_concurrent_limit(_buyer: &u64) -> Result<bool, sp_runtime::DispatchError> { Ok(true) }
    fn record_order_completed(_buyer: &u64, _order_id: u64) -> DispatchResult { Ok(()) }
    fn record_order_cancelled(_buyer: &u64, _order_id: u64) -> DispatchResult { Ok(()) }
    fn record_violation(
        _buyer: &u64,
        _violation_type: pallet_trading_credit::quota::ViolationType,
    ) -> DispatchResult { Ok(()) }
    fn is_suspended(_buyer: &u64) -> Result<bool, sp_runtime::DispatchError> { Ok(false) }
    fn is_blacklisted(_buyer: &u64) -> Result<bool, sp_runtime::DispatchError> { Ok(false) }
}

/// Mock MakerCredit: 所有操作成功
pub struct MockMakerCredit;
impl pallet_trading_common::MakerCreditInterface for MockMakerCredit {
    fn record_maker_order_completed(_maker_id: u64, _order_id: u64, _response_time: u32) -> DispatchResult { Ok(()) }
    fn record_maker_order_timeout(_maker_id: u64, _order_id: u64) -> DispatchResult { Ok(()) }
    fn record_maker_dispute_result(_maker_id: u64, _order_id: u64, _maker_win: bool) -> DispatchResult { Ok(()) }
}

/// Mock PricingProvider: 返回固定价格 0.1 USDT/NEX
pub struct MockPricing;
impl pallet_trading_common::PricingProvider<u128> for MockPricing {
    fn get_cos_to_usd_rate() -> Option<u128> { Some(100_000) } // 0.1 USDT/NEX (精度 10^6)
    fn report_p2p_trade(_timestamp: u64, _price_usdt: u64, _nex_qty: u128) -> DispatchResult { Ok(()) }
}

/// Mock MakerInterface: 做市商 #1 已激活，账户为 10
pub struct MockMakerPallet;
impl pallet_trading_common::MakerInterface<u64, u128> for MockMakerPallet {
    fn get_maker_application(maker_id: u64) -> Option<pallet_trading_common::MakerApplicationInfo<u64, u128>> {
        if maker_id == 1 {
            Some(pallet_trading_common::MakerApplicationInfo {
                account: 10u64,
                tron_address: pallet_trading_common::TronAddress::default(),
                is_active: true,
                _phantom: core::marker::PhantomData,
            })
        } else {
            None
        }
    }
    fn is_maker_active(maker_id: u64) -> bool { maker_id == 1 }
    fn get_maker_id(who: &u64) -> Option<u64> {
        if *who == 10 { Some(1) } else { None }
    }
    fn get_deposit_usd_value(_maker_id: u64) -> Result<u64, sp_runtime::DispatchError> {
        Ok(1_000_000_000) // 1000 USD
    }
    fn slash_deposit_for_severely_underpaid(
        _maker_id: u64, _swap_id: u64, _expected: u64, _actual: u64, _rate: u32,
    ) -> Result<u64, sp_runtime::DispatchError> {
        Ok(0)
    }
}

/// Mock Timestamp: 返回固定时间
pub struct MockTimestamp;
impl frame_support::traits::UnixTime for MockTimestamp {
    fn now() -> core::time::Duration {
        core::time::Duration::from_secs(1_700_000_000)
    }
}

/// Mock CidLockManager: 所有操作成功
pub struct MockCidLockManager;
impl pallet_storage_service::CidLockManager<H256, u64> for MockCidLockManager {
    fn lock_cid(_cid_hash: H256, _reason: sp_std::vec::Vec<u8>, _until: Option<u64>) -> DispatchResult { Ok(()) }
    fn unlock_cid(_cid_hash: H256, _reason: sp_std::vec::Vec<u8>) -> DispatchResult { Ok(()) }
    fn is_locked(_cid_hash: &H256) -> bool { false }
}

// ==================== P2P Config ====================

parameter_types! {
    // Buy-side 常量
    pub const BuyOrderTimeout: u64 = 3_600_000; // 1 hour in ms
    pub const EvidenceWindow: u64 = 86_400_000; // 24 hours in ms
    pub const FirstPurchaseUsdValue: u128 = 10_000_000; // 10 USD
    pub const MinFirstPurchaseCosAmount: u128 = 1_000_000_000_000; // 1 NEX
    pub const MaxFirstPurchaseCosAmount: u128 = 1_000_000_000_000_000; // 1000 NEX
    pub const MaxOrderUsdAmount: u64 = 200_000_000; // 200 USD
    pub const MinOrderUsdAmount: u64 = 20_000_000; // 20 USD
    pub const FirstPurchaseUsdAmount: u64 = 10_000_000; // 10 USD
    pub const AmountValidationTolerance: u16 = 100; // 1%
    pub const MaxFirstPurchaseOrdersPerMaker: u32 = 5;
    pub const MinDeposit: u128 = 1_000_000_000_000; // 1 NEX
    pub const DepositRate: u16 = 1000; // 10%
    pub const CancelPenaltyRate: u16 = 3000; // 30%
    pub const MinMakerDepositUsd: u64 = 800_000_000; // 800 USD
    pub const DisputeResponseTimeout: u64 = 86400; // 24 hours (seconds)
    pub const DisputeArbitrationTimeout: u64 = 172800; // 48 hours (seconds)

    // Sell-side 常量
    pub const SellTimeoutBlocks: u64 = 100;
    pub const VerificationTimeoutBlocks: u64 = 50;
    pub const MinSellAmount: u128 = 1_000_000_000_000; // 1 NEX
    pub const TxHashTtlBlocks: u64 = 14400;
    pub const VerificationReward: u128 = 100_000_000_000; // 0.1 NEX
    pub const SellFeeRateBps: u32 = 30; // 0.3%
    pub const MinSellFee: u128 = 100_000_000_000; // 0.1 NEX
}

impl pallet_trading_p2p::Config for Test {
    type Currency = Balances;
    type Timestamp = MockTimestamp;
    type Escrow = MockEscrow;
    type BuyerCredit = MockBuyerCredit;
    type MakerCredit = MockMakerCredit;
    type Pricing = MockPricing;
    type MakerPallet = MockMakerPallet;
    type CommitteeOrigin = frame_system::EnsureRoot<u64>;
    type IdentityProvider = ();
    type VerificationOrigin = frame_system::EnsureRoot<u64>;
    type ArbitratorOrigin = frame_system::EnsureRoot<u64>;
    type CidLockManager = MockCidLockManager;
    type WeightInfo = ();

    // Buy-side 常量
    type BuyOrderTimeout = BuyOrderTimeout;
    type EvidenceWindow = EvidenceWindow;
    type FirstPurchaseUsdValue = FirstPurchaseUsdValue;
    type MinFirstPurchaseCosAmount = MinFirstPurchaseCosAmount;
    type MaxFirstPurchaseCosAmount = MaxFirstPurchaseCosAmount;
    type MaxOrderUsdAmount = MaxOrderUsdAmount;
    type MinOrderUsdAmount = MinOrderUsdAmount;
    type FirstPurchaseUsdAmount = FirstPurchaseUsdAmount;
    type AmountValidationTolerance = AmountValidationTolerance;
    type MaxFirstPurchaseOrdersPerMaker = MaxFirstPurchaseOrdersPerMaker;
    type MinDeposit = MinDeposit;
    type DepositRate = DepositRate;
    type CancelPenaltyRate = CancelPenaltyRate;
    type MinMakerDepositUsd = MinMakerDepositUsd;
    type DisputeResponseTimeout = DisputeResponseTimeout;
    type DisputeArbitrationTimeout = DisputeArbitrationTimeout;

    // Sell-side 常量
    type SellTimeoutBlocks = SellTimeoutBlocks;
    type VerificationTimeoutBlocks = VerificationTimeoutBlocks;
    type MinSellAmount = MinSellAmount;
    type TxHashTtlBlocks = TxHashTtlBlocks;
    type VerificationReward = VerificationReward;
    type SellFeeRateBps = SellFeeRateBps;
    type MinSellFee = MinSellFee;
}

// ==================== Helper ====================

/// 账户常量
pub const BUYER: u64 = 1;
pub const BUYER2: u64 = 2;
pub const MAKER_ACCOUNT: u64 = 10;
pub const MAKER_ID: u64 = 1;
pub const ESCROW_ACCOUNT: u64 = 999;
pub const PALLET_ACCOUNT: u64 = 3524478956; // PalletId(*b"p2p/trad").into_account_truncating()

/// 创建测试环境，预设余额
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (BUYER, 1_000_000_000_000_000),       // buyer: 1000 NEX
            (BUYER2, 1_000_000_000_000_000),       // buyer2: 1000 NEX
            (MAKER_ACCOUNT, 10_000_000_000_000_000), // maker: 10000 NEX
            (ESCROW_ACCOUNT, 100_000_000_000_000_000), // escrow
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}
