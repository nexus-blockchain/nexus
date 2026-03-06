//! KYC 模块测试 mock

use crate as pallet_entity_kyc;
use frame_support::{derive_impl, parameter_types, traits::ConstU32};
use frame_system::EnsureRoot;
use sp_runtime::{BuildStorage, DispatchError};
use core::cell::RefCell;
use std::collections::HashMap;

type Block = frame_system::mocking::MockBlock<Test>;

pub const ADMIN: u64 = 1;
pub const PROVIDER: u64 = 2;
pub const PROVIDER2: u64 = 7;
pub const USER: u64 = 3;
pub const USER2: u64 = 4;
pub const ENTITY_OWNER: u64 = 5;
pub const ENTITY_ADMIN_USER: u64 = 6;

pub const ENTITY_1: u64 = 1;
pub const ENTITY_2: u64 = 2;

// ==================== Mock EntityProvider ====================

thread_local! {
    static ENTITY_OWNERS: RefCell<HashMap<u64, u64>> = RefCell::new(HashMap::new());
    static ENTITY_ADMINS: RefCell<HashMap<(u64, u64), u32>> = RefCell::new(HashMap::new());
    static ENTITY_LOCKED: RefCell<std::collections::HashSet<u64>> = RefCell::new(std::collections::HashSet::new());
}

pub struct MockEntityProvider;

impl MockEntityProvider {
    pub fn set_entity_owner(entity_id: u64, owner: u64) {
        ENTITY_OWNERS.with(|m| m.borrow_mut().insert(entity_id, owner));
    }

    pub fn set_entity_admin(entity_id: u64, admin: u64, permissions: u32) {
        ENTITY_ADMINS.with(|m| m.borrow_mut().insert((entity_id, admin), permissions));
    }

    pub fn set_entity_locked(entity_id: u64) {
        ENTITY_LOCKED.with(|l| l.borrow_mut().insert(entity_id));
    }
}

impl pallet_entity_common::EntityProvider<u64> for MockEntityProvider {
    fn entity_exists(entity_id: u64) -> bool {
        ENTITY_OWNERS.with(|m| m.borrow().contains_key(&entity_id))
    }
    fn is_entity_active(_entity_id: u64) -> bool { true }
    fn entity_status(_entity_id: u64) -> Option<pallet_entity_common::EntityStatus> { None }
    fn entity_owner(entity_id: u64) -> Option<u64> {
        ENTITY_OWNERS.with(|m| m.borrow().get(&entity_id).cloned())
    }
    fn entity_account(_entity_id: u64) -> u64 { 0 }
    fn update_entity_stats(_entity_id: u64, _sales_amount: u128, _order_count: u32) -> Result<(), DispatchError> { Ok(()) }

    fn is_entity_admin(entity_id: u64, account: &u64, required_permission: u32) -> bool {
        ENTITY_ADMINS.with(|m| {
            m.borrow()
                .get(&(entity_id, *account))
                .map(|perms| perms & required_permission == required_permission)
                .unwrap_or(false)
        })
    }
    fn is_entity_locked(entity_id: u64) -> bool {
        ENTITY_LOCKED.with(|l| l.borrow().contains(&entity_id))
    }
}

// ==================== Runtime ====================

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        EntityKyc: pallet_entity_kyc,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

parameter_types! {
    pub const BasicKycValidity: u64 = 1000;
    pub const StandardKycValidity: u64 = 500;
    pub const EnhancedKycValidity: u64 = 2000;
    pub const InstitutionalKycValidity: u64 = 3000;
    pub const PendingKycTimeout: u64 = 100;
}

impl pallet_entity_kyc::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxCidLength = ConstU32<64>;
    type MaxProviderNameLength = ConstU32<64>;
    type MaxProviders = ConstU32<20>;
    type BasicKycValidity = BasicKycValidity;
    type StandardKycValidity = StandardKycValidity;
    type EnhancedKycValidity = EnhancedKycValidity;
    type InstitutionalKycValidity = InstitutionalKycValidity;
    type AdminOrigin = EnsureRoot<u64>;
    type EntityProvider = MockEntityProvider;
    type MaxHistoryEntries = ConstU32<50>;
    type PendingKycTimeout = PendingKycTimeout;
    type OnKycStatusChange = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    ENTITY_OWNERS.with(|m| m.borrow_mut().clear());
    ENTITY_ADMINS.with(|m| m.borrow_mut().clear());
    ENTITY_LOCKED.with(|l| l.borrow_mut().clear());

    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    t.into()
}
