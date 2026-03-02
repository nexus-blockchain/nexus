use crate as pallet_arbitration;
use frame_support::{
    derive_impl, parameter_types,
    traits::ConstU32,
    PalletId,
};
use sp_runtime::{
    traits::IdentityLookup,
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u128;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Escrow: pallet_escrow,
        Arbitration: pallet_arbitration,
    }
);

parameter_types! {
    pub const ExistentialDeposit: Balance = 1;
    pub const EscrowPalletId: PalletId = PalletId(*b"py/escro");
    pub const TreasuryAccountId: u64 = 99;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = u64;
    type Lookup = IdentityLookup<u64>;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
    type Balance = Balance;
    type ExistentialDeposit = ExistentialDeposit;
    type RuntimeHoldReason = RuntimeHoldReason;
}

// -- Escrow Config --

pub struct TestExpiryPolicy;
impl pallet_escrow::pallet::ExpiryPolicy<u64, u64> for TestExpiryPolicy {
    fn on_expire(id: u64) -> Result<pallet_escrow::pallet::ExpiryAction<u64>, sp_runtime::DispatchError> {
        if id % 2 == 0 {
            Ok(pallet_escrow::pallet::ExpiryAction::ReleaseAll(99))
        } else {
            Ok(pallet_escrow::pallet::ExpiryAction::RefundAll(99))
        }
    }
    fn now() -> u64 {
        System::block_number()
    }
}

impl pallet_escrow::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type EscrowPalletId = EscrowPalletId;
    type AuthorizedOrigin = frame_system::EnsureSigned<u64>;
    type AdminOrigin = frame_system::EnsureRoot<u64>;
    type MaxExpiringPerBlock = ConstU32<10>;
    type MaxSplitEntries = ConstU32<10>;
    type ExpiryPolicy = TestExpiryPolicy;
    type WeightInfo = ();
}

// -- Mock ArbitrationRouter --

use core::cell::RefCell;

thread_local! {
    static CAN_DISPUTE: RefCell<bool> = RefCell::new(true);
    static COUNTERPARTY: RefCell<Option<u64>> = RefCell::new(Some(2));
    static ORDER_AMOUNT: RefCell<Option<Balance>> = RefCell::new(Some(10_000));
    static APPLY_DECISION_OK: RefCell<bool> = RefCell::new(true);
    static EVIDENCE_EXISTS: RefCell<bool> = RefCell::new(true);
}

pub fn set_can_dispute(val: bool) {
    CAN_DISPUTE.with(|v| *v.borrow_mut() = val);
}

pub fn set_counterparty(val: Option<u64>) {
    COUNTERPARTY.with(|v| *v.borrow_mut() = val);
}

pub fn set_order_amount(val: Option<Balance>) {
    ORDER_AMOUNT.with(|v| *v.borrow_mut() = val);
}

pub fn set_evidence_exists(val: bool) {
    EVIDENCE_EXISTS.with(|v| *v.borrow_mut() = val);
}

pub struct MockRouter;
impl pallet_arbitration::pallet::ArbitrationRouter<u64, Balance> for MockRouter {
    fn can_dispute(_domain: [u8; 8], _who: &u64, _id: u64) -> bool {
        CAN_DISPUTE.with(|v| *v.borrow())
    }
    fn apply_decision(_domain: [u8; 8], _id: u64, _decision: pallet_arbitration::pallet::Decision) -> sp_runtime::DispatchResult {
        if APPLY_DECISION_OK.with(|v| *v.borrow()) {
            Ok(())
        } else {
            Err(sp_runtime::DispatchError::Other("router failed"))
        }
    }
    fn get_counterparty(_domain: [u8; 8], _initiator: &u64, _id: u64) -> Result<u64, sp_runtime::DispatchError> {
        COUNTERPARTY.with(|v| v.borrow().ok_or(sp_runtime::DispatchError::Other("no counterparty")))
    }
    fn get_order_amount(_domain: [u8; 8], _id: u64) -> Result<Balance, sp_runtime::DispatchError> {
        ORDER_AMOUNT.with(|v| v.borrow().ok_or(sp_runtime::DispatchError::Other("no order amount")))
    }
}

// -- Mock Escrow trait --

pub struct MockEscrow;
impl pallet_escrow::pallet::Escrow<u64, Balance> for MockEscrow {
    fn escrow_account() -> u64 { 50 }
    fn lock_from(_payer: &u64, _id: u64, _amount: Balance) -> sp_runtime::DispatchResult { Ok(()) }
    fn transfer_from_escrow(_id: u64, _to: &u64, _amount: Balance) -> sp_runtime::DispatchResult { Ok(()) }
    fn release_all(_id: u64, _to: &u64) -> sp_runtime::DispatchResult { Ok(()) }
    fn refund_all(_id: u64, _to: &u64) -> sp_runtime::DispatchResult { Ok(()) }
    fn amount_of(_id: u64) -> Balance { 100_000 }
    fn split_partial(_id: u64, _release_to: &u64, _refund_to: &u64, _bps: u16) -> sp_runtime::DispatchResult { Ok(()) }
    fn set_disputed(_id: u64) -> sp_runtime::DispatchResult { Ok(()) }
    fn set_resolved(_id: u64) -> sp_runtime::DispatchResult { Ok(()) }
}

// -- Mock CidLockManager --

pub struct MockCidLockManager;
impl pallet_storage_service::CidLockManager<sp_core::H256, u64> for MockCidLockManager {
    fn lock_cid(_cid_hash: sp_core::H256, _reason: Vec<u8>, _until: Option<u64>) -> sp_runtime::DispatchResult { Ok(()) }
    fn unlock_cid(_cid_hash: sp_core::H256, _reason: Vec<u8>) -> sp_runtime::DispatchResult { Ok(()) }
    fn is_locked(_cid_hash: &sp_core::H256) -> bool { false }
}

// -- Mock PricingProvider --

pub struct MockPricing;
impl pallet_trading_common::PricingProvider<Balance> for MockPricing {
    // Rate = 10_000_000 means 1 COS = 10 USD => deposit = 1_000_000 * 1_000_000 / 10_000_000 = 100
    // This equals ComplaintDeposit (min_deposit), so deposit_amount = 100
    fn get_cos_to_usd_rate() -> Option<Balance> { Some(10_000_000) }
    fn report_p2p_trade(_timestamp: u64, _price_usdt: u64, _nex_qty: u128) -> sp_runtime::DispatchResult { Ok(()) }
}

// -- Mock EvidenceExistenceChecker --

pub struct MockEvidenceChecker;
impl pallet_arbitration::pallet::EvidenceExistenceChecker for MockEvidenceChecker {
    fn evidence_exists(_id: u64) -> bool {
        EVIDENCE_EXISTS.with(|v| *v.borrow())
    }
}

// -- Arbitration Config --

parameter_types! {
    pub const DepositRatioBps: u16 = 1500; // 15%
    pub const ResponseDeadline: u64 = 100;
    pub const RejectedSlashBps: u16 = 3000; // 30%
    pub const PartialSlashBps: u16 = 5000; // 50%
    pub const ComplaintDeposit: Balance = 100;
    pub const ComplaintDepositUsd: u64 = 1_000_000; // 1 USDT
    pub const ComplaintSlashBps: u16 = 5000; // 50%
    pub const ArchiveTtlBlocks: u32 = 1000;
    pub const ComplaintArchiveDelayBlocks: u64 = 50;
}

impl pallet_arbitration::pallet::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxEvidence = ConstU32<10>;
    type MaxCidLen = ConstU32<64>;
    type Escrow = MockEscrow;
    type WeightInfo = ();
    type Router = MockRouter;
    type DecisionOrigin = frame_system::EnsureRoot<u64>;
    type Fungible = Balances;
    type RuntimeHoldReason = RuntimeHoldReason;
    type DepositRatioBps = DepositRatioBps;
    type ResponseDeadline = ResponseDeadline;
    type RejectedSlashBps = RejectedSlashBps;
    type PartialSlashBps = PartialSlashBps;
    type ComplaintDeposit = ComplaintDeposit;
    type ComplaintDepositUsd = ComplaintDepositUsd;
    type Pricing = MockPricing;
    type ComplaintSlashBps = ComplaintSlashBps;
    type TreasuryAccount = TreasuryAccountId;
    type CidLockManager = MockCidLockManager;
    type CreditUpdater = ();
    type ArchiveTtlBlocks = ArchiveTtlBlocks;
    type ComplaintArchiveDelayBlocks = ComplaintArchiveDelayBlocks;
    type EvidenceExists = MockEvidenceChecker;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    // Reset thread-local state
    CAN_DISPUTE.with(|v| *v.borrow_mut() = true);
    COUNTERPARTY.with(|v| *v.borrow_mut() = Some(2));
    ORDER_AMOUNT.with(|v| *v.borrow_mut() = Some(10_000));
    APPLY_DECISION_OK.with(|v| *v.borrow_mut() = true);
    EVIDENCE_EXISTS.with(|v| *v.borrow_mut() = true);

    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1, 10_000_000),
            (2, 10_000_000),
            (3, 10_000_000),
            (50, 10_000_000), // escrow account
            (99, 10_000_000), // treasury
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
