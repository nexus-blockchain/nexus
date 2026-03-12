use crate as pallet_entity_registry;
use frame_support::{
    derive_impl,
    parameter_types,
    traits::{ConstU32, ConstU64, ConstU128},
};
use sp_runtime::{
    traits::IdentityLookup,
    BuildStorage,
};
use frame_system::EnsureRoot;
use pallet_entity_common::{ShopProvider, ShopType, ShopOperatingStatus, EffectiveShopStatus, PricingProvider, GovernanceProvider, GovernanceMode, OrderProvider, TokenSaleProvider, DisputeQueryProvider, MarketProvider};
use pallet_storage_service::{StoragePin, PinTier};
use core::cell::RefCell;
use sp_std::collections::btree_set::BTreeSet;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        EntityRegistry: pallet_entity_registry,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = u64;
    type Lookup = IdentityLookup<u64>;
    type AccountData = pallet_balances::AccountData<u128>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = u128;
    type AccountStore = System;
}

// ==================== Mock PricingProvider ====================

pub struct MockPricingProvider;
impl PricingProvider for MockPricingProvider {
    fn get_nex_usdt_price() -> u64 {
        // 1 NEX = 0.5 USDT → price = 500_000 (precision 10^6)
        500_000
    }
}

// ==================== Mock ShopProvider ====================

pub struct MockShopProvider;
impl ShopProvider<u64> for MockShopProvider {
    fn shop_exists(_shop_id: u64) -> bool { false }
    fn is_shop_active(_shop_id: u64) -> bool { false }
    fn shop_entity_id(_shop_id: u64) -> Option<u64> { None }
    fn shop_owner(_shop_id: u64) -> Option<u64> { None }
    fn shop_account(_shop_id: u64) -> u64 { 0 }
    fn shop_type(_shop_id: u64) -> Option<ShopType> { None }
    fn is_shop_manager(_shop_id: u64, _account: &u64) -> bool { false }
    fn update_shop_stats(_shop_id: u64, _sales_amount: u128, _order_count: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn update_shop_rating(_shop_id: u64, _rating: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn deduct_operating_fund(_shop_id: u64, _amount: u128) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn operating_balance(_shop_id: u64) -> u128 { 0 }
    fn create_primary_shop(
        _entity_id: u64,
        _name: sp_std::vec::Vec<u8>,
        _shop_type: ShopType,
    ) -> Result<u64, sp_runtime::DispatchError> {
        // Return a mock shop_id = 1
        Ok(1)
    }
    fn is_primary_shop(_shop_id: u64) -> bool { false }
    fn shop_own_status(_shop_id: u64) -> Option<ShopOperatingStatus> { None }
    fn effective_status(_shop_id: u64) -> Option<EffectiveShopStatus> { None }
    fn pause_shop(_shop_id: u64) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn resume_shop(_shop_id: u64) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn force_close_shop(_shop_id: u64) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
}

// ==================== Mock GovernanceProvider ====================

thread_local! {
    static LOCKED_ENTITIES: RefCell<BTreeSet<u64>> = RefCell::new(BTreeSet::new());
    static ACTIVE_PROPOSALS: RefCell<BTreeSet<u64>> = RefCell::new(BTreeSet::new());
    static ACTIVE_SALE: RefCell<BTreeSet<u64>> = RefCell::new(BTreeSet::new());
    static ACTIVE_MARKET: RefCell<BTreeSet<u64>> = RefCell::new(BTreeSet::new());
    static ACTIVE_SHOP_ORDERS: RefCell<BTreeSet<u64>> = RefCell::new(BTreeSet::new());
    static ACTIVE_DISPUTES: RefCell<BTreeSet<u64>> = RefCell::new(BTreeSet::new());
}

pub struct MockGovernanceProvider;
impl GovernanceProvider for MockGovernanceProvider {
    fn governance_mode(_entity_id: u64) -> GovernanceMode { GovernanceMode::None }
    fn has_active_proposals(entity_id: u64) -> bool {
        ACTIVE_PROPOSALS.with(|m| m.borrow().contains(&entity_id))
    }
    fn is_governance_locked(entity_id: u64) -> bool {
        LOCKED_ENTITIES.with(|m| m.borrow().contains(&entity_id))
    }
}

pub fn set_entity_locked(entity_id: u64) {
    LOCKED_ENTITIES.with(|m| m.borrow_mut().insert(entity_id));
}

#[allow(dead_code)]
pub fn clear_entity_locked(entity_id: u64) {
    LOCKED_ENTITIES.with(|m| m.borrow_mut().remove(&entity_id));
}

// ==================== Close Guard Mock Helpers ====================

pub fn set_active_proposals(entity_id: u64) {
    ACTIVE_PROPOSALS.with(|m| m.borrow_mut().insert(entity_id));
}

#[allow(dead_code)]
pub fn clear_active_proposals(entity_id: u64) {
    ACTIVE_PROPOSALS.with(|m| m.borrow_mut().remove(&entity_id));
}

pub fn set_active_sale(entity_id: u64) {
    ACTIVE_SALE.with(|m| m.borrow_mut().insert(entity_id));
}

#[allow(dead_code)]
pub fn clear_active_sale(entity_id: u64) {
    ACTIVE_SALE.with(|m| m.borrow_mut().remove(&entity_id));
}

pub fn set_active_market(entity_id: u64) {
    ACTIVE_MARKET.with(|m| m.borrow_mut().insert(entity_id));
}

#[allow(dead_code)]
pub fn clear_active_market(entity_id: u64) {
    ACTIVE_MARKET.with(|m| m.borrow_mut().remove(&entity_id));
}

pub fn set_active_shop_orders(shop_id: u64) {
    ACTIVE_SHOP_ORDERS.with(|m| m.borrow_mut().insert(shop_id));
}

#[allow(dead_code)]
pub fn clear_active_shop_orders(shop_id: u64) {
    ACTIVE_SHOP_ORDERS.with(|m| m.borrow_mut().remove(&shop_id));
}

pub fn set_active_disputes(entity_id: u64) {
    ACTIVE_DISPUTES.with(|m| m.borrow_mut().insert(entity_id));
}

#[allow(dead_code)]
pub fn clear_active_disputes(entity_id: u64) {
    ACTIVE_DISPUTES.with(|m| m.borrow_mut().remove(&entity_id));
}

// ==================== Mock OrderProvider ====================

pub struct MockOrderProvider;
impl OrderProvider<u64, u128> for MockOrderProvider {
    fn order_exists(_order_id: u64) -> bool { false }
    fn order_buyer(_order_id: u64) -> Option<u64> { None }
    fn order_seller(_order_id: u64) -> Option<u64> { None }
    fn order_amount(_order_id: u64) -> Option<u128> { None }
    fn order_shop_id(_order_id: u64) -> Option<u64> { None }
    fn is_order_completed(_order_id: u64) -> bool { false }
    fn is_order_disputed(_order_id: u64) -> bool { false }
    fn can_dispute(_order_id: u64, _who: &u64) -> bool { false }
    fn has_active_orders_for_shop(shop_id: u64) -> bool {
        ACTIVE_SHOP_ORDERS.with(|m| m.borrow().contains(&shop_id))
    }
}

// ==================== Mock TokenSaleProvider ====================

pub struct MockTokenSaleProvider;
impl TokenSaleProvider<u128> for MockTokenSaleProvider {
    fn active_sale_round(entity_id: u64) -> Option<u64> {
        if ACTIVE_SALE.with(|m| m.borrow().contains(&entity_id)) {
            Some(1)
        } else {
            None
        }
    }
    fn sale_round_status(_round_id: u64) -> Option<pallet_entity_common::TokenSaleStatus> { None }
    fn sold_amount(_round_id: u64) -> Option<u128> { None }
    fn remaining_amount(_round_id: u64) -> Option<u128> { None }
    fn participants_count(_round_id: u64) -> Option<u32> { None }
}

// ==================== Mock DisputeQueryProvider ====================

pub struct MockDisputeQueryProvider;
impl DisputeQueryProvider<u64> for MockDisputeQueryProvider {
    fn order_dispute_status(_order_id: u64) -> pallet_entity_common::DisputeStatus {
        pallet_entity_common::DisputeStatus::None
    }
    fn dispute_resolution(_dispute_id: u64) -> Option<pallet_entity_common::DisputeResolution> { None }
    fn active_dispute_count(_domain: u8, _account: &u64) -> u32 { 0 }
    fn has_active_disputes_for_entity(entity_id: u64) -> bool {
        ACTIVE_DISPUTES.with(|m| m.borrow().contains(&entity_id))
    }
}

// ==================== Mock MarketProvider ====================

pub struct MockMarketProvider;
impl MarketProvider<u64, u128> for MockMarketProvider {
    fn has_active_market(entity_id: u64) -> bool {
        ACTIVE_MARKET.with(|m| m.borrow().contains(&entity_id))
    }
    fn trading_volume_24h(_entity_id: u64) -> u128 { 0 }
    fn best_bid(_entity_id: u64) -> Option<u128> { None }
    fn best_ask(_entity_id: u64) -> Option<u128> { None }
}

// ==================== Mock StoragePin ====================

pub struct MockStoragePin;
impl StoragePin<u64> for MockStoragePin {
    fn pin(
        _owner: u64,
        _domain: &[u8],
        _subject_id: u64,
        _entity_id: Option<u64>,
        _cid: sp_std::vec::Vec<u8>,
        _size_bytes: u64,
        _tier: PinTier,
    ) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }
    fn unpin(_owner: u64, _cid: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }
}

// ==================== PlatformAccount ====================

parameter_types! {
    pub PlatformAccountId: u64 = 99;
}

impl pallet_entity_registry::Config for Test {
    type Currency = Balances;
    type MaxEntityNameLength = ConstU32<64>;
    type MaxCidLength = ConstU32<64>;
    type GovernanceOrigin = EnsureRoot<u64>;
    type PricingProvider = MockPricingProvider;
    type InitialFundUsdt = ConstU64<50_000_000>;     // 50 USDT
    type MinInitialFundCos = ConstU128<1_000_000_000_000>;  // 1 token (10^12)
    type MaxInitialFundCos = ConstU128<{ 1000 * 1_000_000_000_000 }>; // 1000 tokens
    type MinOperatingBalance = ConstU128<100_000_000_000>;  // 0.1 token
    type FundWarningThreshold = ConstU128<1_000_000_000_000>; // 1 token
    type MaxAdmins = ConstU32<3>;
    type MaxEntitiesPerUser = ConstU32<3>;
    type ShopProvider = MockShopProvider;
    type MaxShopsPerEntity = ConstU32<16>;
    type PlatformAccount = PlatformAccountId;
    type GovernanceProvider = MockGovernanceProvider;
    type CloseRequestTimeout = ConstU64<100>;
    type MaxReferralsPerReferrer = ConstU32<100>;
    type StoragePin = MockStoragePin;
    type OnEntityStatusChange = ();
    type OrderProvider = MockOrderProvider;
    type TokenSaleProvider = MockTokenSaleProvider;
    type DisputeQueryProvider = MockDisputeQueryProvider;
    type MarketProvider = MockMarketProvider;
    type WeightInfo = ();
}

// ==================== Test Externalities Builder ====================

pub const ALICE: u64 = 1;
pub const BOB: u64 = 2;
pub const CHARLIE: u64 = 3;
pub const DAVE: u64 = 4;
pub const EVE: u64 = 5;

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (ALICE, 1_000_000_000_000_000),   // 1000 tokens
            (BOB, 1_000_000_000_000_000),
            (CHARLIE, 1_000_000_000_000_000),
            (DAVE, 1_000_000_000_000_000),
            (EVE, 10_000_000_000),              // very low balance
            (99, 1_000_000_000_000_000),        // platform account
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

/// 计算预期的初始资金
/// price = 500_000 (0.5 USDT/NEX), usdt = 50_000_000 (50 USDT)
/// nex = 50_000_000 * 10^12 / 500_000 = 100 * 10^12 = 100 tokens
/// clamped to [1 token, 1000 tokens] → 100 tokens
pub const EXPECTED_INITIAL_FUND: u128 = 100_000_000_000_000;
