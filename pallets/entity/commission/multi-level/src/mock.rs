use crate as pallet_commission_multi_level;
use frame_support::{derive_impl, parameter_types};
use sp_runtime::BuildStorage;
use std::cell::RefCell;
use std::collections::BTreeMap;

pub type Balance = u128;

// ============================================================================
// Thread-local mock state
// ============================================================================

thread_local! {
    static REFERRERS: RefCell<BTreeMap<(u64, u64), u64>> = RefCell::new(BTreeMap::new());
    static MEMBER_STATS: RefCell<BTreeMap<(u64, u64), (u32, u32, u128)>> = RefCell::new(BTreeMap::new());
    static MEMBER_SPENT_USDT: RefCell<BTreeMap<(u64, u64), u64>> = RefCell::new(BTreeMap::new());
}

pub fn set_referrer(entity_id: u64, account: u64, referrer: u64) {
    REFERRERS.with(|r| {
        r.borrow_mut().insert((entity_id, account), referrer);
    });
}

pub fn set_stats(entity_id: u64, account: u64, direct: u32, team_size: u32, total_spent: u128) {
    MEMBER_STATS.with(|s| {
        s.borrow_mut().insert((entity_id, account), (direct, team_size, total_spent));
    });
}

pub fn set_spent_usdt(entity_id: u64, account: u64, usdt: u64) {
    MEMBER_SPENT_USDT.with(|s| {
        s.borrow_mut().insert((entity_id, account), usdt);
    });
}

pub fn clear_thread_locals() {
    REFERRERS.with(|r| r.borrow_mut().clear());
    MEMBER_STATS.with(|s| s.borrow_mut().clear());
    MEMBER_SPENT_USDT.with(|s| s.borrow_mut().clear());
}

/// 设置线性推荐链: buyer -> r1 -> r2 -> r3 -> ...
pub fn setup_chain(entity_id: u64, buyer: u64, referrers: &[u64]) {
    let mut prev = buyer;
    for &r in referrers {
        set_referrer(entity_id, prev, r);
        prev = r;
    }
}

// ============================================================================
// MockMemberProvider
// ============================================================================

pub struct MockMemberProvider;

impl pallet_commission_common::MemberProvider<u64> for MockMemberProvider {
    fn is_member(_: u64, _: &u64) -> bool { true }
    fn get_referrer(entity_id: u64, account: &u64) -> Option<u64> {
        REFERRERS.with(|r| r.borrow().get(&(entity_id, *account)).copied())
    }
    fn get_member_stats(entity_id: u64, account: &u64) -> (u32, u32, u128) {
        MEMBER_STATS.with(|s| s.borrow().get(&(entity_id, *account)).copied().unwrap_or((0, 0, 0)))
    }
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
    fn member_count_by_level(_: u64, _: u8) -> u32 { 0 }
    fn get_member_spent_usdt(entity_id: u64, account: &u64) -> u64 {
        MEMBER_SPENT_USDT.with(|s| s.borrow().get(&(entity_id, *account)).copied().unwrap_or(0))
    }
    fn auto_register_qualified(_: u64, _: &u64, _: Option<u64>, _: bool) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn is_activated(_: u64, _: &u64) -> bool { true }
}

// ============================================================================
// Mock Runtime
// ============================================================================

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        CommissionMultiLevel: pallet_commission_multi_level,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = frame_system::mocking::MockBlock<Test>;
}

parameter_types! {
    pub const MaxMultiLevels: u32 = 15;
}

impl pallet_commission_multi_level::Config for Test {
    type MemberProvider = MockMemberProvider;
    type MaxMultiLevels = MaxMultiLevels;
    type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
    });
    ext
}
