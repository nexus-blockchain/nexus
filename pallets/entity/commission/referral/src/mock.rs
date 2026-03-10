use crate as pallet_commission_referral;
use frame_support::derive_impl;
use sp_runtime::BuildStorage;
use std::cell::RefCell;
use std::collections::BTreeMap;

pub type Balance = u128;
pub type AccountId = u64;

// ============================================================================
// Thread-local mock state
// ============================================================================

thread_local! {
    static REFERRERS: RefCell<BTreeMap<(u64, u64), u64>> = RefCell::new(BTreeMap::new());
    static MEMBER_STATS: RefCell<BTreeMap<(u64, u64), (u32, u32, u128)>> = RefCell::new(BTreeMap::new());
    static BANNED_MEMBERS: RefCell<std::collections::BTreeSet<(u64, u64)>> = RefCell::new(std::collections::BTreeSet::new());
    static UNACTIVATED_MEMBERS: RefCell<std::collections::BTreeSet<(u64, u64)>> = RefCell::new(std::collections::BTreeSet::new());
    static ENTITY_OWNERS: RefCell<BTreeMap<u64, u64>> = RefCell::new(BTreeMap::new());
    static ENTITY_ADMINS: RefCell<BTreeMap<(u64, u64), u32>> = RefCell::new(BTreeMap::new());
    static ENTITY_LOCKED: RefCell<std::collections::BTreeSet<u64>> = RefCell::new(std::collections::BTreeSet::new());
    // F4/F6 mock state
    static ENTITY_INACTIVE: RefCell<std::collections::BTreeSet<u64>> = RefCell::new(std::collections::BTreeSet::new());
    static FROZEN_MEMBERS: RefCell<std::collections::BTreeSet<(u64, u64)>> = RefCell::new(std::collections::BTreeSet::new());
    // F5/F7 mock state
    static REFERRAL_REGISTERED_AT: RefCell<BTreeMap<(u64, u64), u64>> = RefCell::new(BTreeMap::new());
    static COMPLETED_ORDERS: RefCell<BTreeMap<(u64, u64), u32>> = RefCell::new(BTreeMap::new());
    // M1 审计: 非会员集合
    static NON_MEMBERS: RefCell<std::collections::BTreeSet<(u64, u64)>> = RefCell::new(std::collections::BTreeSet::new());
    // M2 审计: 可配置 MaxTotalReferralRate
    static MAX_REFERRAL_RATE: RefCell<u16> = RefCell::new(10000);
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

pub fn ban_member(entity_id: u64, account: u64) {
    BANNED_MEMBERS.with(|b| { b.borrow_mut().insert((entity_id, account)); });
}

pub fn set_unactivated(entity_id: u64, account: u64) {
    UNACTIVATED_MEMBERS.with(|u| { u.borrow_mut().insert((entity_id, account)); });
}

pub fn set_entity_owner(entity_id: u64, owner: u64) {
    ENTITY_OWNERS.with(|e| { e.borrow_mut().insert(entity_id, owner); });
}

pub fn set_entity_admin(entity_id: u64, admin: u64, permissions: u32) {
    ENTITY_ADMINS.with(|a| { a.borrow_mut().insert((entity_id, admin), permissions); });
}

pub fn set_entity_locked(entity_id: u64) {
    ENTITY_LOCKED.with(|l| { l.borrow_mut().insert(entity_id); });
}

pub fn deactivate_entity(entity_id: u64) {
    ENTITY_INACTIVE.with(|e| { e.borrow_mut().insert(entity_id); });
}

pub fn freeze_member(entity_id: u64, account: u64) {
    FROZEN_MEMBERS.with(|f| { f.borrow_mut().insert((entity_id, account)); });
}

pub fn set_referral_registered_at(entity_id: u64, account: u64, block: u64) {
    REFERRAL_REGISTERED_AT.with(|r| { r.borrow_mut().insert((entity_id, account), block); });
}

pub fn set_completed_orders(entity_id: u64, account: u64, count: u32) {
    COMPLETED_ORDERS.with(|c| { c.borrow_mut().insert((entity_id, account), count); });
}

pub fn set_non_member(entity_id: u64, account: u64) {
    NON_MEMBERS.with(|n| { n.borrow_mut().insert((entity_id, account)); });
}

pub fn set_max_referral_rate(rate: u16) {
    MAX_REFERRAL_RATE.with(|r| { *r.borrow_mut() = rate; });
}

pub fn clear_thread_locals() {
    REFERRERS.with(|r| r.borrow_mut().clear());
    MEMBER_STATS.with(|s| s.borrow_mut().clear());
    BANNED_MEMBERS.with(|b| b.borrow_mut().clear());
    UNACTIVATED_MEMBERS.with(|u| u.borrow_mut().clear());
    ENTITY_OWNERS.with(|e| e.borrow_mut().clear());
    ENTITY_ADMINS.with(|a| a.borrow_mut().clear());
    ENTITY_LOCKED.with(|l| l.borrow_mut().clear());
    ENTITY_INACTIVE.with(|e| e.borrow_mut().clear());
    FROZEN_MEMBERS.with(|f| f.borrow_mut().clear());
    REFERRAL_REGISTERED_AT.with(|r| r.borrow_mut().clear());
    COMPLETED_ORDERS.with(|c| c.borrow_mut().clear());
    NON_MEMBERS.with(|n| n.borrow_mut().clear());
    MAX_REFERRAL_RATE.with(|r| { *r.borrow_mut() = 10000; });
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
    fn is_member(entity_id: u64, account: &u64) -> bool {
        !NON_MEMBERS.with(|n| n.borrow().contains(&(entity_id, *account)))
    }
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
    fn is_banned(entity_id: u64, account: &u64) -> bool {
        BANNED_MEMBERS.with(|b| b.borrow().contains(&(entity_id, *account)))
    }
    fn is_activated(entity_id: u64, account: &u64) -> bool {
        !UNACTIVATED_MEMBERS.with(|u| u.borrow().contains(&(entity_id, *account)))
    }
    fn is_member_active(entity_id: u64, account: &u64) -> bool {
        !Self::is_banned(entity_id, account) &&
        !FROZEN_MEMBERS.with(|f| f.borrow().contains(&(entity_id, *account)))
    }
    fn referral_registered_at(entity_id: u64, account: &u64) -> u64 {
        REFERRAL_REGISTERED_AT.with(|r| r.borrow().get(&(entity_id, *account)).copied().unwrap_or(0))
    }
    fn completed_order_count(entity_id: u64, account: &u64) -> u32 {
        COMPLETED_ORDERS.with(|c| c.borrow().get(&(entity_id, *account)).copied().unwrap_or(0))
    }
}

// ============================================================================
// MockEntityProvider
// ============================================================================

pub struct MockEntityProvider;

impl pallet_entity_common::EntityProvider<u64> for MockEntityProvider {
    fn entity_exists(entity_id: u64) -> bool {
        ENTITY_OWNERS.with(|e| e.borrow().contains_key(&entity_id))
    }
    fn is_entity_active(entity_id: u64) -> bool {
        !ENTITY_INACTIVE.with(|e| e.borrow().contains(&entity_id))
    }
    fn entity_status(_entity_id: u64) -> Option<pallet_entity_common::EntityStatus> { None }
    fn entity_owner(entity_id: u64) -> Option<u64> {
        ENTITY_OWNERS.with(|e| e.borrow().get(&entity_id).copied())
    }
    fn entity_account(_entity_id: u64) -> u64 { 0 }
    fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn is_entity_admin(entity_id: u64, account: &u64, required_permission: u32) -> bool {
        ENTITY_ADMINS.with(|a| {
            a.borrow().get(&(entity_id, *account))
                .map(|perms| perms & required_permission == required_permission)
                .unwrap_or(false)
        })
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
        CommissionReferral: pallet_commission_referral,
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

pub struct MaxTotalReferralRate;
impl frame_support::traits::Get<u16> for MaxTotalReferralRate {
    fn get() -> u16 {
        MAX_REFERRAL_RATE.with(|r| *r.borrow())
    }
}

impl pallet_commission_referral::Config for Test {
    type Currency = Balances;
    type MemberProvider = MockMemberProvider;
    type EntityProvider = MockEntityProvider;
    type MaxTotalReferralRate = MaxTotalReferralRate;
    type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    clear_thread_locals();
    let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
    });
    ext
}

