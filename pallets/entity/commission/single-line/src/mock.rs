use crate as pallet_commission_single_line;
use frame_support::{derive_impl, parameter_types};
use pallet_commission_common::MemberCommissionStatsData;
use sp_runtime::BuildStorage;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

type Balance = u128;

// ============================================================================
// Thread-local mock state
// ============================================================================

thread_local! {
    pub static MEMBER_STATS: RefCell<BTreeMap<(u64, u64), MemberCommissionStatsData<Balance>>> =
        RefCell::new(BTreeMap::new());
    /// 自定义等级 (entity_id, account) -> custom_level_id
    pub static CUSTOM_LEVEL_IDS: RefCell<BTreeMap<(u64, u64), u8>> =
        RefCell::new(BTreeMap::new());
    /// Entity owners: entity_id -> owner_account
    pub static ENTITY_OWNERS: RefCell<BTreeMap<u64, u64>> =
        RefCell::new(BTreeMap::new());
    /// Entity admins: (entity_id, account) -> permission_mask
    pub static ENTITY_ADMINS: RefCell<BTreeMap<(u64, u64), u32>> =
        RefCell::new(BTreeMap::new());
    /// Banned members: (entity_id, account)
    pub static BANNED_MEMBERS: RefCell<BTreeSet<(u64, u64)>> =
        RefCell::new(BTreeSet::new());
    /// Locked entities
    pub static ENTITY_LOCKED: RefCell<BTreeSet<u64>> =
        RefCell::new(BTreeSet::new());
}

pub fn clear_mocks() {
    MEMBER_STATS.with(|m| m.borrow_mut().clear());
    CUSTOM_LEVEL_IDS.with(|m| m.borrow_mut().clear());
    ENTITY_OWNERS.with(|m| m.borrow_mut().clear());
    ENTITY_ADMINS.with(|m| m.borrow_mut().clear());
    BANNED_MEMBERS.with(|m| m.borrow_mut().clear());
    ENTITY_LOCKED.with(|m| m.borrow_mut().clear());
}

pub fn set_member_stats(entity_id: u64, account: u64, total_earned: Balance) {
    MEMBER_STATS.with(|m| {
        m.borrow_mut().insert(
            (entity_id, account),
            MemberCommissionStatsData {
                total_earned,
                ..Default::default()
            },
        );
    });
}

/// 设置自定义等级 ID
pub fn set_custom_level(entity_id: u64, account: u64, level_id: u8) {
    CUSTOM_LEVEL_IDS.with(|m| {
        m.borrow_mut().insert((entity_id, account), level_id);
    });
}

pub fn set_entity_owner(entity_id: u64, owner: u64) {
    ENTITY_OWNERS.with(|m| { m.borrow_mut().insert(entity_id, owner); });
}

pub fn set_entity_admin(entity_id: u64, account: u64, permission: u32) {
    ENTITY_ADMINS.with(|m| { m.borrow_mut().insert((entity_id, account), permission); });
}

pub fn set_banned(entity_id: u64, account: u64) {
    BANNED_MEMBERS.with(|m| { m.borrow_mut().insert((entity_id, account)); });
}

pub fn set_entity_locked(entity_id: u64) {
    ENTITY_LOCKED.with(|m| { m.borrow_mut().insert(entity_id); });
}

// ============================================================================
// Mock providers
// ============================================================================

pub struct MockStatsProvider;

impl crate::pallet::SingleLineStatsProvider<u64, Balance> for MockStatsProvider {
    fn get_member_stats(entity_id: u64, account: &u64) -> MemberCommissionStatsData<Balance> {
        MEMBER_STATS.with(|m| {
            m.borrow()
                .get(&(entity_id, *account))
                .cloned()
                .unwrap_or_default()
        })
    }
}

pub struct MockMemberLevelProvider;

impl crate::pallet::SingleLineMemberLevelProvider<u64> for MockMemberLevelProvider {
    fn custom_level_id(entity_id: u64, account: &u64) -> u8 {
        CUSTOM_LEVEL_IDS.with(|m| m.borrow().get(&(entity_id, *account)).copied().unwrap_or(0))
    }
}

pub struct MockEntityProvider;

impl pallet_entity_common::EntityProvider<u64> for MockEntityProvider {
    fn entity_exists(entity_id: u64) -> bool {
        ENTITY_OWNERS.with(|m| m.borrow().contains_key(&entity_id))
    }
    fn is_entity_active(entity_id: u64) -> bool {
        ENTITY_OWNERS.with(|m| m.borrow().contains_key(&entity_id))
    }
    fn entity_status(_entity_id: u64) -> Option<pallet_entity_common::EntityStatus> { None }
    fn entity_owner(entity_id: u64) -> Option<u64> {
        ENTITY_OWNERS.with(|m| m.borrow().get(&entity_id).copied())
    }
    fn entity_account(_entity_id: u64) -> u64 { 0 }
    fn update_entity_stats(_entity_id: u64, _sales_amount: u128, _order_count: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn is_entity_admin(entity_id: u64, account: &u64, required_permission: u32) -> bool {
        ENTITY_ADMINS.with(|m| {
            m.borrow().get(&(entity_id, *account))
                .map(|p| p & required_permission == required_permission)
                .unwrap_or(false)
        })
    }
    fn is_entity_locked(entity_id: u64) -> bool {
        ENTITY_LOCKED.with(|m| m.borrow().contains(&entity_id))
    }
}

pub struct MockMemberProvider;

impl pallet_commission_common::MemberProvider<u64> for MockMemberProvider {
    fn is_member(_entity_id: u64, _account: &u64) -> bool { true }
    fn get_referrer(_entity_id: u64, _account: &u64) -> Option<u64> { None }
    fn custom_level_id(_entity_id: u64, _account: &u64) -> u8 { 0 }
    fn get_level_commission_bonus(_entity_id: u64, _level_id: u8) -> u16 { 0 }
    fn uses_custom_levels(_entity_id: u64) -> bool { false }
    fn get_member_stats(_entity_id: u64, _account: &u64) -> (u32, u32, u128) { (0, 0, 0) }
    fn auto_register(_entity_id: u64, _account: &u64, _referrer: Option<u64>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn is_banned(entity_id: u64, account: &u64) -> bool {
        BANNED_MEMBERS.with(|m| m.borrow().contains(&(entity_id, *account)))
    }
}

// ============================================================================
// Mock Runtime
// ============================================================================

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        CommissionSingleLine: pallet_commission_single_line,
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
    pub const MaxSingleLineLength: u32 = 100;
}

impl pallet_commission_single_line::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type StatsProvider = MockStatsProvider;
    type MemberLevelProvider = MockMemberLevelProvider;
    type EntityProvider = MockEntityProvider;
    type MemberProvider = MockMemberProvider;
    type MaxSingleLineLength = MaxSingleLineLength;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    clear_mocks();
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
