//! Mock runtime for pallet-entity-shop tests

use crate as pallet_entity_shop;
use frame_support::derive_impl;
use frame_support::parameter_types;
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
    static ENTITY_PRIMARY_SHOP: RefCell<alloc::collections::BTreeMap<u64, u64>> = RefCell::new(alloc::collections::BTreeMap::new());
    static SHOPS_WITH_ACTIVE_ORDERS: RefCell<BTreeSet<u64>> = RefCell::new(BTreeSet::new());
}

pub fn set_entity_locked(entity_id: u64) {
    ENTITY_LOCKED.with(|l| l.borrow_mut().insert(entity_id));
}

pub fn set_entity_suspended(entity_id: u64) {
    ENTITY_SUSPENDED.with(|s| s.borrow_mut().insert(entity_id));
}

pub fn set_shop_has_active_orders(shop_id: u64) {
    SHOPS_WITH_ACTIVE_ORDERS.with(|s| s.borrow_mut().insert(shop_id));
}

pub fn clear_shop_active_orders(shop_id: u64) {
    SHOPS_WITH_ACTIVE_ORDERS.with(|s| s.borrow_mut().remove(&shop_id));
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
            let mut map = s.borrow_mut();
            let shops = map.entry(entity_id).or_default();
            let is_first = shops.is_empty();
            shops.push(shop_id);
            if is_first {
                ENTITY_PRIMARY_SHOP.with(|p| p.borrow_mut().insert(entity_id, shop_id));
            }
        });
        Ok(())
    }

    fn unregister_shop(entity_id: u64, shop_id: u64) -> Result<(), sp_runtime::DispatchError> {
        ENTITY_SHOPS.with(|s| {
            if let Some(shops) = s.borrow_mut().get_mut(&entity_id) {
                shops.retain(|&id| id != shop_id);
                // 若移除的是 primary，自动重选
                ENTITY_PRIMARY_SHOP.with(|p| {
                    let mut map = p.borrow_mut();
                    if map.get(&entity_id) == Some(&shop_id) {
                        if let Some(&first) = shops.first() {
                            map.insert(entity_id, first);
                        } else {
                            map.remove(&entity_id);
                        }
                    }
                });
            }
        });
        Ok(())
    }

    fn set_primary_shop_id(entity_id: u64, shop_id: u64) {
        ENTITY_PRIMARY_SHOP.with(|p| p.borrow_mut().insert(entity_id, shop_id));
    }

    fn get_primary_shop_id(entity_id: u64) -> u64 {
        ENTITY_PRIMARY_SHOP.with(|p| p.borrow().get(&entity_id).copied().unwrap_or(0))
    }
}

// ==================== Mock StoragePin ====================

pub struct MockProductProvider;
impl pallet_entity_common::ProductProvider<u64> for MockProductProvider {
    fn product_exists(_: u64) -> bool { false }
    fn is_product_on_sale(_: u64) -> bool { false }
    fn product_shop_id(_: u64) -> Option<u64> { None }
    fn product_stock(_: u64) -> Option<u32> { None }
    fn product_category(_: u64) -> Option<pallet_entity_common::ProductCategory> { None }
    fn deduct_stock(_: u64, _: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn restore_stock(_: u64, _: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn add_sold_count(_: u64, _: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
}

pub struct MockOrderProvider;
impl pallet_entity_common::OrderProvider<u64, u64> for MockOrderProvider {
    fn order_exists(_: u64) -> bool { false }
    fn order_buyer(_: u64) -> Option<u64> { None }
    fn order_seller(_: u64) -> Option<u64> { None }
    fn order_amount(_: u64) -> Option<u64> { None }
    fn order_shop_id(_: u64) -> Option<u64> { None }
    fn is_order_completed(_: u64) -> bool { false }
    fn is_order_disputed(_: u64) -> bool { false }
    fn can_dispute(_: u64, _: &u64) -> bool { false }
    fn has_active_orders_for_shop(shop_id: u64) -> bool {
        SHOPS_WITH_ACTIVE_ORDERS.with(|s| s.borrow().contains(&shop_id))
    }
}

pub struct MockStoragePin;
impl StoragePin<u64> for MockStoragePin {
    fn pin(_owner: u64, _domain: &[u8], _subject_id: u64, _entity_id: Option<u64>, _cid: Vec<u8>, _size_bytes: u64, _tier: PinTier) -> Result<(), sp_runtime::DispatchError> {
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
    pub const MinOperatingBalance: u64 = 100;
    pub const WarningThreshold: u64 = 200;
    pub const ShopClosingGracePeriod: u64 = 10;
    pub const MaxShopsPerEntity: u32 = 5;
}

pub struct MockPointsCleanup;
impl pallet_entity_common::PointsCleanup for MockPointsCleanup {
    fn cleanup_shop_points(_shop_id: u64) {}
}

impl pallet_entity_shop::Config for Test {
    type Currency = Balances;
    type EntityProvider = MockEntityProvider;
    type MaxShopNameLength = MaxShopNameLength;
    type MaxCidLength = MaxCidLength;
    type MaxManagers = MaxManagers;
    type MinOperatingBalance = MinOperatingBalance;
    type WarningThreshold = WarningThreshold;
    type CommissionFundGuard = ();
    type ShopClosingGracePeriod = ShopClosingGracePeriod;
    type MaxShopsPerEntity = MaxShopsPerEntity;
    type StoragePin = MockStoragePin;
    type ProductProvider = MockProductProvider;
    type PointsCleanup = MockPointsCleanup;
    type OrderProvider = MockOrderProvider;
    type WeightInfo = ();
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
