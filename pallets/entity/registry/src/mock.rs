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
use pallet_entity_common::{ShopProvider, ShopType, MemberMode, ShopOperatingStatus, EffectiveShopStatus, PricingProvider};

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
    fn get_cos_usdt_price() -> u64 {
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
    fn shop_member_mode(_shop_id: u64) -> MemberMode { MemberMode::Inherit }
    fn is_shop_manager(_shop_id: u64, _account: &u64) -> bool { false }
    fn update_shop_stats(_shop_id: u64, _sales_amount: u128, _order_count: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn update_shop_rating(_shop_id: u64, _rating: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn deduct_operating_fund(_shop_id: u64, _amount: u128) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn operating_balance(_shop_id: u64) -> u128 { 0 }
    fn create_primary_shop(
        _entity_id: u64,
        _name: sp_std::vec::Vec<u8>,
        _shop_type: ShopType,
        _member_mode: MemberMode,
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

// ==================== PlatformAccount ====================

parameter_types! {
    pub PlatformAccountId: u64 = 99;
}

impl pallet_entity_registry::Config for Test {
    type RuntimeEvent = RuntimeEvent;
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
