use crate as pallet_commission_level_diff;
use frame_support::{derive_impl, parameter_types};
use sp_runtime::BuildStorage;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

pub type Balance = u128;
pub const OWNER: u64 = 100;
pub const ADMIN: u64 = 101;
pub const NON_OWNER: u64 = 999;

// ============================================================================
// Thread-local mock state
// ============================================================================

thread_local! {
    static REFERRERS: RefCell<BTreeMap<(u64, u64), u64>> = RefCell::new(BTreeMap::new());
    static CUSTOM_LEVEL_IDS: RefCell<BTreeMap<(u64, u64), u8>> = RefCell::new(BTreeMap::new());
    static LEVEL_BONUSES: RefCell<BTreeMap<(u64, u8), u16>> = RefCell::new(BTreeMap::new());
    static BANNED_MEMBERS: RefCell<BTreeSet<(u64, u64)>> = RefCell::new(BTreeSet::new());
    static UNACTIVATED_MEMBERS: RefCell<BTreeSet<(u64, u64)>> = RefCell::new(BTreeSet::new());
    static ENTITY_OWNERS: RefCell<BTreeMap<u64, u64>> = RefCell::new(BTreeMap::new());
    static ENTITY_ADMINS: RefCell<BTreeSet<(u64, u64)>> = RefCell::new(BTreeSet::new());
    static ENTITY_LOCKED: RefCell<BTreeSet<u64>> = RefCell::new(BTreeSet::new());
    static ENTITY_INACTIVE: RefCell<BTreeSet<u64>> = RefCell::new(BTreeSet::new());
    static NON_MEMBERS: RefCell<BTreeSet<(u64, u64)>> = RefCell::new(BTreeSet::new());
    static FROZEN_MEMBERS: RefCell<BTreeSet<(u64, u64)>> = RefCell::new(BTreeSet::new());
}

pub fn clear_mocks() {
    REFERRERS.with(|r| r.borrow_mut().clear());
    CUSTOM_LEVEL_IDS.with(|c| c.borrow_mut().clear());
    LEVEL_BONUSES.with(|l| l.borrow_mut().clear());
    BANNED_MEMBERS.with(|b| b.borrow_mut().clear());
    UNACTIVATED_MEMBERS.with(|u| u.borrow_mut().clear());
    ENTITY_OWNERS.with(|o| o.borrow_mut().clear());
    ENTITY_ADMINS.with(|a| a.borrow_mut().clear());
    ENTITY_LOCKED.with(|l| l.borrow_mut().clear());
    ENTITY_INACTIVE.with(|i| i.borrow_mut().clear());
    NON_MEMBERS.with(|n| n.borrow_mut().clear());
    FROZEN_MEMBERS.with(|f| f.borrow_mut().clear());
}

pub fn set_entity_owner(entity_id: u64, owner: u64) {
    ENTITY_OWNERS.with(|o| o.borrow_mut().insert(entity_id, owner));
}

pub fn set_entity_admin(entity_id: u64, admin: u64) {
    ENTITY_ADMINS.with(|a| a.borrow_mut().insert((entity_id, admin)));
}

pub fn lock_entity(entity_id: u64) {
    ENTITY_LOCKED.with(|l| l.borrow_mut().insert(entity_id));
}

pub fn ban_member(entity_id: u64, account: u64) {
    BANNED_MEMBERS.with(|b| b.borrow_mut().insert((entity_id, account)));
}

pub fn set_unactivated(entity_id: u64, account: u64) {
    UNACTIVATED_MEMBERS.with(|u| u.borrow_mut().insert((entity_id, account)));
}

pub fn deactivate_entity(entity_id: u64) {
    ENTITY_INACTIVE.with(|i| i.borrow_mut().insert(entity_id));
}

pub fn mark_non_member(entity_id: u64, account: u64) {
    NON_MEMBERS.with(|n| n.borrow_mut().insert((entity_id, account)));
}

pub fn freeze_member(entity_id: u64, account: u64) {
    FROZEN_MEMBERS.with(|f| f.borrow_mut().insert((entity_id, account)));
}

pub fn set_referrer(entity_id: u64, account: u64, referrer: u64) {
    REFERRERS.with(|r| r.borrow_mut().insert((entity_id, account), referrer));
}

pub fn set_custom_level_id(entity_id: u64, account: u64, level: u8) {
    CUSTOM_LEVEL_IDS.with(|c| c.borrow_mut().insert((entity_id, account), level));
}

pub fn set_level_bonus(entity_id: u64, level_id: u8, bonus: u16) {
    LEVEL_BONUSES.with(|l| l.borrow_mut().insert((entity_id, level_id), bonus));
}

/// Helper: setup chain buyer(50) → 40 → 30 → 20 → 10
pub fn setup_chain(entity_id: u64) {
    REFERRERS.with(|r| {
        let mut m = r.borrow_mut();
        m.insert((entity_id, 50), 40);
        m.insert((entity_id, 40), 30);
        m.insert((entity_id, 30), 20);
        m.insert((entity_id, 20), 10);
    });
}

// ============================================================================
// MockMemberProvider
// ============================================================================

pub struct MockMemberProvider;

impl pallet_commission_common::MemberProvider<u64> for MockMemberProvider {
    fn is_member(entity_id: u64, account: &u64) -> bool {
        !NON_MEMBERS.with(|n| n.borrow().contains(&(entity_id, *account)))
    }
    fn get_referrer(entity_id: u64, account: &u64) -> Option<u64> {
        REFERRERS.with(|r| r.borrow().get(&(entity_id, *account)).copied())
    }
    fn get_member_stats(_: u64, _: &u64) -> (u32, u32, u128) { (0, 0, 0) }
    fn uses_custom_levels(_entity_id: u64) -> bool { true }
    fn custom_level_id(entity_id: u64, account: &u64) -> u8 {
        CUSTOM_LEVEL_IDS.with(|c| c.borrow().get(&(entity_id, *account)).copied().unwrap_or(0))
    }
    fn get_level_commission_bonus(entity_id: u64, level_id: u8) -> u16 {
        LEVEL_BONUSES.with(|l| l.borrow().get(&(entity_id, level_id)).copied().unwrap_or(0))
    }
    fn is_banned(entity_id: u64, account: &u64) -> bool {
        BANNED_MEMBERS.with(|b| b.borrow().contains(&(entity_id, *account)))
    }
    fn is_activated(entity_id: u64, account: &u64) -> bool {
        !UNACTIVATED_MEMBERS.with(|u| u.borrow().contains(&(entity_id, *account)))
    }
    fn is_member_active(entity_id: u64, account: &u64) -> bool {
        !FROZEN_MEMBERS.with(|f| f.borrow().contains(&(entity_id, *account)))
    }
    fn auto_register(_: u64, _: &u64, _: Option<u64>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn set_custom_levels_enabled(_: u64, _: bool) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn set_upgrade_mode(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn add_custom_level(_: u64, _: u8, _: &[u8], _: u128, _: u16, _: u16) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn update_custom_level(_: u64, _: u8, _: Option<&[u8]>, _: Option<u128>, _: Option<u16>, _: Option<u16>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn remove_custom_level(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn custom_level_count(_: u64) -> u8 { 0 }
}

// ============================================================================
// MockEntityProvider
// ============================================================================

pub struct MockEntityProvider;

impl pallet_entity_common::EntityProvider<u64> for MockEntityProvider {
    fn entity_exists(entity_id: u64) -> bool {
        ENTITY_OWNERS.with(|o| o.borrow().contains_key(&entity_id))
    }
    fn is_entity_active(entity_id: u64) -> bool {
        !ENTITY_INACTIVE.with(|i| i.borrow().contains(&entity_id))
    }
    fn entity_status(_entity_id: u64) -> Option<pallet_entity_common::EntityStatus> {
        Some(pallet_entity_common::EntityStatus::Active)
    }
    fn entity_owner(entity_id: u64) -> Option<u64> {
        ENTITY_OWNERS.with(|o| o.borrow().get(&entity_id).copied())
    }
    fn entity_account(_entity_id: u64) -> u64 { 0 }
    fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn is_entity_admin(entity_id: u64, who: &u64, _perm: u32) -> bool {
        ENTITY_ADMINS.with(|a| a.borrow().contains(&(entity_id, *who)))
    }
    fn is_entity_locked(entity_id: u64) -> bool {
        ENTITY_LOCKED.with(|l| l.borrow().contains(&entity_id))
    }
}

// ============================================================================
// Mock Runtime
// ============================================================================

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        CommissionLevelDiff: pallet_commission_level_diff,
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
    pub const MaxCustomLevels: u32 = 10;
}

impl pallet_commission_level_diff::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MemberProvider = MockMemberProvider;
    type EntityProvider = MockEntityProvider;
    type MaxCustomLevels = MaxCustomLevels;
    type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    clear_mocks();
    let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        set_entity_owner(1, OWNER);
    });
    ext
}
