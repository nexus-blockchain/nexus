//! Mock runtime for pallet-entity-shop tests

use crate as pallet_entity_shop;
use frame_support::{
    derive_impl, parameter_types,
    traits::{ConstU32, ConstU64},
};
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Shop: pallet_entity_shop,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u64>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

// Mock EntityProvider

use pallet_storage_service::{StoragePin, PinTier};
use core::cell::RefCell;
use alloc::collections::BTreeSet;
extern crate alloc;

thread_local! {
    static ENTITY_LOCKED: RefCell<BTreeSet<u64>> = RefCell::new(BTreeSet::new());
    static ENTITY_SHOPS: RefCell<alloc::collections::BTreeMap<u64, alloc::vec::Vec<u64>>> = RefCell::new(alloc::collections::BTreeMap::new());
    static ENTITY_SUSPENDED: RefCell<BTreeSet<u64>> = RefCell::new(BTreeSet::new());
}

pub fn set_entity_locked(entity_id: u64) {
    ENTITY_LOCKED.with(|l| l.borrow_mut().insert(entity_id));
}

pub fn set_entity_suspended(entity_id: u64) {
    ENTITY_SUSPENDED.with(|s| s.borrow_mut().insert(entity_id));
}

pub struct MockEntityProvider;

impl pallet_entity_common::EntityProvider<u64> for MockEntityProvider {
    fn entity_exists(entity_id: u64) -> bool {
        // Entity 1, 2, 3 exist
        entity_id >= 1 && entity_id <= 3
    }

    fn is_entity_active(entity_id: u64) -> bool {
        if ENTITY_SUSPENDED.with(|s| s.borrow().contains(&entity_id)) {
            return false;
        }
        entity_id == 1 || entity_id == 2
    }

    fn entity_status(entity_id: u64) -> Option<pallet_entity_common::EntityStatus> {
        match entity_id {
            1 | 2 => Some(pallet_entity_common::EntityStatus::Active),
            3 => Some(pallet_entity_common::EntityStatus::Suspended),
            _ => None,
        }
    }

    fn entity_owner(entity_id: u64) -> Option<u64> {
        match entity_id {
            1 => Some(1), // Entity 1 owned by account 1
            2 => Some(2), // Entity 2 owned by account 2
            3 => Some(3), // Entity 3 owned by account 3
            _ => None,
        }
    }

    fn entity_account(entity_id: u64) -> u64 {
        1000 + entity_id
    }

    fn update_entity_stats(_entity_id: u64, _sales_amount: u128, _order_count: u32) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn is_entity_admin(entity_id: u64, account: &u64, _required_permission: u32) -> bool {
        // Account 10 is admin of Entity 1 (full permissions in mock)
        entity_id == 1 && *account == 10
    }
    fn is_entity_locked(entity_id: u64) -> bool {
        ENTITY_LOCKED.with(|l| l.borrow().contains(&entity_id))
    }

    fn entity_shops(entity_id: u64) -> sp_std::vec::Vec<u64> {
        ENTITY_SHOPS.with(|s| s.borrow().get(&entity_id).cloned().unwrap_or_default())
    }

    fn register_shop(entity_id: u64, shop_id: u64) -> Result<(), sp_runtime::DispatchError> {
        ENTITY_SHOPS.with(|s| {
            s.borrow_mut().entry(entity_id).or_default().push(shop_id);
        });
        Ok(())
    }

    fn unregister_shop(entity_id: u64, shop_id: u64) -> Result<(), sp_runtime::DispatchError> {
        ENTITY_SHOPS.with(|s| {
            if let Some(shops) = s.borrow_mut().get_mut(&entity_id) {
                shops.retain(|&id| id != shop_id);
            }
        });
        Ok(())
    }
}

// ==================== Mock StoragePin ====================

pub struct MockProductProvider;
impl pallet_entity_common::ProductProvider<u64, u64> for MockProductProvider {
    fn product_exists(_: u64) -> bool { false }
    fn is_product_on_sale(_: u64) -> bool { false }
    fn product_shop_id(_: u64) -> Option<u64> { None }
    fn product_price(_: u64) -> Option<u64> { None }
    fn product_stock(_: u64) -> Option<u32> { None }
    fn product_category(_: u64) -> Option<pallet_entity_common::ProductCategory> { None }
    fn deduct_stock(_: u64, _: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn restore_stock(_: u64, _: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn add_sold_count(_: u64, _: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
}

pub struct MockStoragePin;
impl StoragePin<u64> for MockStoragePin {
    fn pin(_owner: u64, _domain: &[u8], _subject_id: u64, _entity_id: Option<u64>, _cid: Vec<u8>, _tier: PinTier) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }
    fn unpin(_owner: u64, _cid: Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }
}

parameter_types! {
    pub const MaxShopNameLength: u32 = 64;
    pub const MaxCidLength: u32 = 64;
    pub const MaxManagers: u32 = 10;
    pub const MaxPointsNameLength: u32 = 32;
    pub const MaxPointsSymbolLength: u32 = 8;
    pub const MinOperatingBalance: u64 = 100;
    pub const WarningThreshold: u64 = 200;
    pub const ShopClosingGracePeriod: u64 = 10;
    pub const MaxShopsPerEntity: u32 = 5;
}

impl pallet_entity_shop::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type EntityProvider = MockEntityProvider;
    type MaxShopNameLength = MaxShopNameLength;
    type MaxCidLength = MaxCidLength;
    type MaxManagers = MaxManagers;
    type MaxPointsNameLength = MaxPointsNameLength;
    type MaxPointsSymbolLength = MaxPointsSymbolLength;
    type MinOperatingBalance = MinOperatingBalance;
    type WarningThreshold = WarningThreshold;
    type CommissionFundGuard = ();
    type ShopClosingGracePeriod = ShopClosingGracePeriod;
    type MaxShopsPerEntity = MaxShopsPerEntity;
    type StoragePin = MockStoragePin;
    type ProductProvider = MockProductProvider;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1, 10000),
            (2, 10000),
            (3, 10000),
            (4, 10000),
            (5, 10000),
        ],
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        // Shop IDs 从 1 开始（与 Entity ID 保持一致）
        crate::NextShopId::<Test>::put(1);
    });
    ext
}
