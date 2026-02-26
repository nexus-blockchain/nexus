use crate as pallet_commission_core;
use frame_support::{derive_impl, parameter_types, traits::ConstU32};
use sp_runtime::BuildStorage;
use std::cell::RefCell;
use std::collections::BTreeMap;

pub type Balance = u128;
pub type AccountId = u64;

pub const PLATFORM: u64 = 99;
pub const SELLER: u64 = 10;
pub const BUYER: u64 = 50;
pub const REFERRER: u64 = 88;

pub const TREASURY: u64 = 98;
pub const ENTITY_ID: u64 = 1;
pub const SHOP_ID: u64 = 100;

// ============================================================================
// Thread-local mock state
// ============================================================================

thread_local! {
    static SHOP_ENTITY: RefCell<BTreeMap<u64, u64>> = RefCell::new(BTreeMap::new());
    static ENTITY_OWNERS: RefCell<BTreeMap<u64, u64>> = RefCell::new(BTreeMap::new());
    static SHOP_OWNERS: RefCell<BTreeMap<u64, u64>> = RefCell::new(BTreeMap::new());
    static REFERRERS: RefCell<BTreeMap<(u64, u64), u64>> = RefCell::new(BTreeMap::new());
    static ENTITY_REFERRERS: RefCell<BTreeMap<u64, u64>> = RefCell::new(BTreeMap::new());
}

pub fn setup_default() {
    SHOP_ENTITY.with(|m| m.borrow_mut().insert(SHOP_ID, ENTITY_ID));
    ENTITY_OWNERS.with(|m| m.borrow_mut().insert(ENTITY_ID, SELLER));
    SHOP_OWNERS.with(|m| m.borrow_mut().insert(SHOP_ID, SELLER));
}

pub fn set_entity_referrer(entity_id: u64, referrer: u64) {
    ENTITY_REFERRERS.with(|m| m.borrow_mut().insert(entity_id, referrer));
}

pub fn set_referrer(shop_id: u64, account: u64, referrer: u64) {
    REFERRERS.with(|m| m.borrow_mut().insert((shop_id, account), referrer));
}

pub fn clear_thread_locals() {
    SHOP_ENTITY.with(|m| m.borrow_mut().clear());
    ENTITY_OWNERS.with(|m| m.borrow_mut().clear());
    SHOP_OWNERS.with(|m| m.borrow_mut().clear());
    REFERRERS.with(|m| m.borrow_mut().clear());
    ENTITY_REFERRERS.with(|m| m.borrow_mut().clear());
}

// ============================================================================
// Mock Providers
// ============================================================================

pub struct MockShopProvider;

impl pallet_entity_common::ShopProvider<u64> for MockShopProvider {
    fn shop_exists(shop_id: u64) -> bool {
        SHOP_ENTITY.with(|m| m.borrow().contains_key(&shop_id))
    }
    fn is_shop_active(_: u64) -> bool { true }
    fn shop_entity_id(shop_id: u64) -> Option<u64> {
        SHOP_ENTITY.with(|m| m.borrow().get(&shop_id).copied())
    }
    fn shop_owner(shop_id: u64) -> Option<u64> {
        SHOP_OWNERS.with(|m| m.borrow().get(&shop_id).copied())
    }
    fn shop_account(_: u64) -> u64 { 0 }
    fn shop_type(_: u64) -> Option<pallet_entity_common::ShopType> { None }
    fn shop_member_mode(_: u64) -> pallet_entity_common::MemberMode { pallet_entity_common::MemberMode::Inherit }
    fn is_shop_manager(_: u64, _: &u64) -> bool { false }
    fn update_shop_stats(_: u64, _: u128, _: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn update_shop_rating(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn deduct_operating_fund(_: u64, _: u128) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn operating_balance(_: u64) -> u128 { 0 }
}

pub struct MockEntityProvider;

impl pallet_entity_common::EntityProvider<u64> for MockEntityProvider {
    fn entity_exists(entity_id: u64) -> bool {
        ENTITY_OWNERS.with(|m| m.borrow().contains_key(&entity_id))
    }
    fn is_entity_active(_: u64) -> bool { true }
    fn entity_status(_: u64) -> Option<pallet_entity_common::EntityStatus> { None }
    fn entity_owner(entity_id: u64) -> Option<u64> {
        ENTITY_OWNERS.with(|m| m.borrow().get(&entity_id).copied())
    }
    fn entity_account(entity_id: u64) -> u64 {
        entity_id + 9000
    }
    fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn update_entity_rating(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
}

pub struct MockMemberProvider;

impl pallet_commission_common::MemberProvider<u64> for MockMemberProvider {
    fn is_member(_: u64, _: &u64) -> bool { true }
    fn get_referrer(shop_id: u64, account: &u64) -> Option<u64> {
        REFERRERS.with(|r| r.borrow().get(&(shop_id, *account)).copied())
    }
    fn member_level(_: u64, _: &u64) -> Option<pallet_entity_common::MemberLevel> { None }
    fn get_member_stats(_: u64, _: &u64) -> (u32, u32, u128) { (0, 0, 0) }
    fn uses_custom_levels(_: u64) -> bool { false }
    fn custom_level_id(_: u64, _: &u64) -> u8 { 0 }
    fn get_level_commission_bonus(_: u64, _: u8) -> u16 { 0 }
    fn auto_register(_: u64, _: &u64, _: Option<u64>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn set_custom_levels_enabled(_: u64, _: bool) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn set_upgrade_mode(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn add_custom_level(_: u64, _: u8, _: &[u8], _: u128, _: u16, _: u16) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn update_custom_level(_: u64, _: u8, _: Option<&[u8]>, _: Option<u128>, _: Option<u16>, _: Option<u16>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn remove_custom_level(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn custom_level_count(_: u64) -> u8 { 0 }
}

pub struct MockEntityReferrerProvider;

impl pallet_commission_common::EntityReferrerProvider<u64> for MockEntityReferrerProvider {
    fn entity_referrer(entity_id: u64) -> Option<u64> {
        ENTITY_REFERRERS.with(|m| m.borrow().get(&entity_id).copied())
    }
}

// ============================================================================
// Mock Runtime
// ============================================================================

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        CommissionCore: pallet_commission_core,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = frame_system::mocking::MockBlock<Test>;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
    type Balance = Balance;
}

parameter_types! {
    pub const PlatformAccount: u64 = PLATFORM;
    pub const TreasuryAccount: u64 = TREASURY;
    pub const ReferrerShareBps: u16 = 5000; // 50% of platform fee
}

impl pallet_commission_core::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type ShopProvider = MockShopProvider;
    type EntityProvider = MockEntityProvider;
    type MemberProvider = MockMemberProvider;
    type ReferralPlugin = ();
    type LevelDiffPlugin = ();
    type SingleLinePlugin = ();
    type TeamPlugin = ();
    type EntityReferrerProvider = MockEntityReferrerProvider;
    type ReferralWriter = ();
    type LevelDiffWriter = ();
    type TeamWriter = ();
    type PlatformAccount = PlatformAccount;
    type TreasuryAccount = TreasuryAccount;
    type ReferrerShareBps = ReferrerShareBps;
    type MaxCommissionRecordsPerOrder = ConstU32<20>;
    type MaxCustomLevels = ConstU32<10>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    clear_thread_locals();
    let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        setup_default();
    });
    ext
}

/// 给账户注资
pub fn fund(account: u64, amount: Balance) {
    let _ = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::deposit_creating(&account, amount);
}

/// Entity 派生账户
pub fn entity_account(entity_id: u64) -> u64 {
    entity_id + 9000
}
