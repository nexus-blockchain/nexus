//! 财务披露模块测试 mock

use crate as pallet_entity_disclosure;
use frame_support::{
    derive_impl,
    parameter_types,
    traits::ConstU32,
};
use pallet_entity_common::{EntityProvider, EntityStatus};
use sp_runtime::{BuildStorage, DispatchError};
use std::cell::RefCell;
use std::collections::BTreeSet;

type Block = frame_system::mocking::MockBlock<Test>;

pub const OWNER: u64 = 1;
pub const OWNER_2: u64 = 2;
pub const ALICE: u64 = 10;
pub const BOB: u64 = 11;
pub const ADMIN: u64 = 20;
pub const ENTITY_ID: u64 = 1;
pub const ENTITY_ID_2: u64 = 2;

thread_local! {
    static ADMINS: RefCell<BTreeSet<(u64, u64)>> = RefCell::new(BTreeSet::new());
    static ENTITY_STATUSES: RefCell<std::collections::BTreeMap<u64, EntityStatus>> = RefCell::new(std::collections::BTreeMap::new());
    static ENTITY_LOCKED: RefCell<BTreeSet<u64>> = RefCell::new(BTreeSet::new());
}

pub fn set_admin(entity_id: u64, account: u64) {
    ADMINS.with(|a| a.borrow_mut().insert((entity_id, account)));
}

pub fn set_entity_status(entity_id: u64, status: EntityStatus) {
    ENTITY_STATUSES.with(|s| s.borrow_mut().insert(entity_id, status));
}

pub fn set_entity_locked(entity_id: u64) {
    ENTITY_LOCKED.with(|l| l.borrow_mut().insert(entity_id));
}

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        EntityDisclosure: pallet_entity_disclosure,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

parameter_types! {
    pub const BasicDisclosureInterval: u64 = 1000;
    pub const StandardDisclosureInterval: u64 = 500;
    pub const EnhancedDisclosureInterval: u64 = 100;
}

/// Mock EntityProvider — 正确实现 EntityProvider trait
pub struct MockEntityProvider;
impl EntityProvider<u64> for MockEntityProvider {
    fn entity_exists(entity_id: u64) -> bool {
        entity_id == ENTITY_ID || entity_id == ENTITY_ID_2
    }
    fn is_entity_active(entity_id: u64) -> bool {
        let override_status = ENTITY_STATUSES.with(|s| s.borrow().get(&entity_id).cloned());
        if let Some(status) = override_status {
            return status == EntityStatus::Active;
        }
        entity_id == ENTITY_ID || entity_id == ENTITY_ID_2
    }
    fn entity_status(entity_id: u64) -> Option<EntityStatus> {
        // 优先使用覆盖状态
        let override_status = ENTITY_STATUSES.with(|s| s.borrow().get(&entity_id).cloned());
        if let Some(status) = override_status {
            return Some(status);
        }
        if entity_id <= 2 { Some(EntityStatus::Active) } else { None }
    }
    fn entity_owner(entity_id: u64) -> Option<u64> {
        match entity_id {
            ENTITY_ID => Some(OWNER),
            ENTITY_ID_2 => Some(OWNER_2),
            _ => None,
        }
    }
    fn entity_account(entity_id: u64) -> u64 {
        100 + entity_id
    }
    fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), DispatchError> { Ok(()) }
    fn is_entity_admin(entity_id: u64, account: &u64, _required_permission: u32) -> bool {
        // Owner 天然拥有全部权限
        if Self::entity_owner(entity_id) == Some(*account) {
            return true;
        }
        ADMINS.with(|a| a.borrow().contains(&(entity_id, *account)))
    }
    fn is_entity_locked(entity_id: u64) -> bool {
        ENTITY_LOCKED.with(|l| l.borrow().contains(&entity_id))
    }
}

parameter_types! {
    pub const MaxBlackoutDuration: u64 = 200; // 测试用较小值
    pub const InsiderCooldownPeriod: u64 = 50; // F4: 测试用冷静期
    pub const MajorHolderThreshold: u32 = 500; // F5: 5% (basis points)
    pub const ViolationThreshold: u32 = 3;     // F6: 3次违规标记高风险
    pub const EmergencyBlackoutMultiplier: u32 = 3; // v0.6: 紧急披露黑窗口 3 倍
}

impl pallet_entity_disclosure::Config for Test {
    type EntityProvider = MockEntityProvider;
    type MaxCidLength = ConstU32<64>;
    type MaxInsiders = ConstU32<5>;
    type MaxDisclosureHistory = ConstU32<10>;
    type BasicDisclosureInterval = BasicDisclosureInterval;
    type StandardDisclosureInterval = StandardDisclosureInterval;
    type EnhancedDisclosureInterval = EnhancedDisclosureInterval;
    type MaxBlackoutDuration = MaxBlackoutDuration;
    type MaxAnnouncementHistory = ConstU32<10>;
    type MaxTitleLength = ConstU32<128>;
    type MaxPinnedAnnouncements = ConstU32<3>;
    type MaxInsiderRoleHistory = ConstU32<10>;
    type InsiderCooldownPeriod = InsiderCooldownPeriod;
    type MajorHolderThreshold = MajorHolderThreshold;
    type ViolationThreshold = ViolationThreshold;
    type MaxApprovers = ConstU32<5>;
    type MaxInsiderTransactionHistory = ConstU32<20>;
    type EmergencyBlackoutMultiplier = EmergencyBlackoutMultiplier;
    type OnDisclosureViolation = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    // L1-audit: 清理 thread-local 状态，防止测试间泄漏
    ADMINS.with(|a| a.borrow_mut().clear());
    ENTITY_STATUSES.with(|s| s.borrow_mut().clear());
    ENTITY_LOCKED.with(|l| l.borrow_mut().clear());

    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

pub fn advance_blocks(n: u64) {
    let current = System::block_number();
    System::set_block_number(current + n);
}
