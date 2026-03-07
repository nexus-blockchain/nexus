use crate as pallet_entity_review;
use frame_support::{
    derive_impl,
    parameter_types,
    traits::ConstU32,
};
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};
use sp_core::ConstU64;
use pallet_entity_common::{EntityProvider, EntityStatus, OrderProvider, ShopProvider, ShopType, ShopOperatingStatus, EffectiveShopStatus};
use sp_runtime::DispatchError;
use core::cell::RefCell;

type Block = frame_system::mocking::MockBlock<Test>;

// ==================== 测试账户 ====================

pub const BUYER: u64 = 1;
pub const BUYER2: u64 = 2;
pub const OTHER: u64 = 3;
pub const ENTITY_OWNER: u64 = 10;
pub const ENTITY_ADMIN: u64 = 11;
pub const ENTITY_1: u64 = 1000;

// ==================== Mock 状态 ====================

thread_local! {
    // OrderProvider mock state: (buyer, shop_id, completed)
    static ORDERS: RefCell<std::collections::HashMap<u64, (u64, u64, bool)>> = RefCell::new(std::collections::HashMap::new());
    // Disputed orders
    static DISPUTED_ORDERS: RefCell<std::collections::HashSet<u64>> = RefCell::new(std::collections::HashSet::new());
    // Order completed_at block number
    static ORDER_COMPLETED_AT: RefCell<std::collections::HashMap<u64, u64>> = RefCell::new(std::collections::HashMap::new());
    // ShopProvider mock state: shop_id -> (rating_sum, rating_count)
    static SHOP_RATINGS: RefCell<std::collections::HashMap<u64, (u64, u64)>> = RefCell::new(std::collections::HashMap::new());
    // ShopProvider active state
    static ACTIVE_SHOPS: RefCell<std::collections::HashSet<u64>> = RefCell::new(std::collections::HashSet::new());
    // Whether update_shop_rating should fail
    static SHOP_RATING_FAIL: RefCell<bool> = RefCell::new(false);
    // EntityProvider mock state: entity_id -> (owner, active)
    static ENTITIES: RefCell<std::collections::HashMap<u64, (u64, bool)>> = RefCell::new(std::collections::HashMap::new());
    // M5: Admin permission map: (entity_id, account) -> permission bitmask
    static ENTITY_ADMINS: RefCell<std::collections::HashMap<(u64, u64), u32>> = RefCell::new(std::collections::HashMap::new());
    // Shop -> Entity mapping
    static SHOP_ENTITY: RefCell<std::collections::HashMap<u64, u64>> = RefCell::new(std::collections::HashMap::new());
    // Entity locked state
    static ENTITY_LOCKED: RefCell<std::collections::HashSet<u64>> = RefCell::new(std::collections::HashSet::new());
    // F3: Order -> product_id mapping
    static ORDER_PRODUCTS: RefCell<std::collections::HashMap<u64, u64>> = RefCell::new(std::collections::HashMap::new());
}

pub fn add_order(order_id: u64, buyer: u64, shop_id: u64, completed: bool) {
    ORDERS.with(|o| o.borrow_mut().insert(order_id, (buyer, shop_id, completed)));
}

pub fn add_shop(shop_id: u64) {
    ACTIVE_SHOPS.with(|s| s.borrow_mut().insert(shop_id));
    SHOP_RATINGS.with(|r| r.borrow_mut().insert(shop_id, (0, 0)));
}

pub fn set_shop_rating_fail(fail: bool) {
    SHOP_RATING_FAIL.with(|f| *f.borrow_mut() = fail);
}

pub fn get_shop_rating(shop_id: u64) -> Option<(u64, u64)> {
    SHOP_RATINGS.with(|r| r.borrow().get(&shop_id).copied())
}

pub fn add_entity(entity_id: u64, owner: u64, active: bool, admins: Vec<u64>) {
    ENTITIES.with(|e| e.borrow_mut().insert(entity_id, (owner, active)));
    // 默认给 admins 全权限（兼容旧测试）
    for admin in admins {
        set_entity_admin(entity_id, admin, u32::MAX);
    }
}

/// M5: 设置 Admin 权限位
pub fn set_entity_admin(entity_id: u64, account: u64, permissions: u32) {
    ENTITY_ADMINS.with(|m| m.borrow_mut().insert((entity_id, account), permissions));
}

pub fn set_shop_entity(shop_id: u64, entity_id: u64) {
    SHOP_ENTITY.with(|m| m.borrow_mut().insert(shop_id, entity_id));
}

pub fn set_order_disputed(order_id: u64) {
    DISPUTED_ORDERS.with(|d| d.borrow_mut().insert(order_id));
}

pub fn set_order_completed_at(order_id: u64, block: u64) {
    ORDER_COMPLETED_AT.with(|m| m.borrow_mut().insert(order_id, block));
}

pub fn set_entity_locked(entity_id: u64) {
    ENTITY_LOCKED.with(|l| l.borrow_mut().insert(entity_id));
}

pub fn set_order_product_id(order_id: u64, product_id: u64) {
    ORDER_PRODUCTS.with(|m| m.borrow_mut().insert(order_id, product_id));
}

pub fn reset_mock_state() {
    ORDERS.with(|o| o.borrow_mut().clear());
    DISPUTED_ORDERS.with(|d| d.borrow_mut().clear());
    SHOP_RATINGS.with(|r| r.borrow_mut().clear());
    ACTIVE_SHOPS.with(|s| s.borrow_mut().clear());
    SHOP_RATING_FAIL.with(|f| *f.borrow_mut() = false);
    ENTITIES.with(|e| e.borrow_mut().clear());
    ENTITY_ADMINS.with(|m| m.borrow_mut().clear());
    SHOP_ENTITY.with(|m| m.borrow_mut().clear());
    ORDER_COMPLETED_AT.with(|m| m.borrow_mut().clear());
    ENTITY_LOCKED.with(|l| l.borrow_mut().clear());
    ORDER_PRODUCTS.with(|m| m.borrow_mut().clear());
}

// ==================== Mock OrderProvider ====================

pub struct MockOrderProvider;

impl OrderProvider<u64, u128> for MockOrderProvider {
    fn order_exists(order_id: u64) -> bool {
        ORDERS.with(|o| o.borrow().contains_key(&order_id))
    }

    fn order_buyer(order_id: u64) -> Option<u64> {
        ORDERS.with(|o| o.borrow().get(&order_id).map(|(buyer, _, _)| *buyer))
    }

    fn order_seller(_order_id: u64) -> Option<u64> {
        None
    }

    fn order_amount(_order_id: u64) -> Option<u128> {
        None
    }

    fn order_shop_id(order_id: u64) -> Option<u64> {
        ORDERS.with(|o| o.borrow().get(&order_id).map(|(_, shop_id, _)| *shop_id))
    }

    fn is_order_completed(order_id: u64) -> bool {
        ORDERS.with(|o| o.borrow().get(&order_id).map(|(_, _, c)| *c).unwrap_or(false))
    }

    fn is_order_disputed(order_id: u64) -> bool {
        DISPUTED_ORDERS.with(|d| d.borrow().contains(&order_id))
    }

    fn can_dispute(_order_id: u64, _who: &u64) -> bool {
        false
    }

    fn order_completed_at(order_id: u64) -> Option<u64> {
        ORDER_COMPLETED_AT.with(|m| m.borrow().get(&order_id).copied())
    }

    fn order_product_id(order_id: u64) -> Option<u64> {
        ORDER_PRODUCTS.with(|m| m.borrow().get(&order_id).copied())
    }
}

// ==================== Mock ShopProvider ====================

pub struct MockShopProvider;

impl ShopProvider<u64> for MockShopProvider {
    fn shop_exists(shop_id: u64) -> bool {
        ACTIVE_SHOPS.with(|s| s.borrow().contains(&shop_id))
    }

    fn is_shop_active(shop_id: u64) -> bool {
        ACTIVE_SHOPS.with(|s| s.borrow().contains(&shop_id))
    }

    fn shop_entity_id(shop_id: u64) -> Option<u64> {
        SHOP_ENTITY.with(|m| m.borrow().get(&shop_id).copied())
    }
    fn shop_owner(_shop_id: u64) -> Option<u64> { None }
    fn shop_account(_shop_id: u64) -> u64 { 0 }
    fn shop_type(_shop_id: u64) -> Option<ShopType> { None }
    fn is_shop_manager(_shop_id: u64, _account: &u64) -> bool { false }
    fn shop_own_status(_shop_id: u64) -> Option<ShopOperatingStatus> { None }
    fn effective_status(_shop_id: u64) -> Option<EffectiveShopStatus> { None }

    fn update_shop_stats(_shop_id: u64, _sales_amount: u128, _order_count: u32) -> Result<(), DispatchError> {
        Ok(())
    }

    fn update_shop_rating(shop_id: u64, rating: u8) -> Result<(), DispatchError> {
        if SHOP_RATING_FAIL.with(|f| *f.borrow()) {
            return Err(DispatchError::Other("shop rating update failed"));
        }
        SHOP_RATINGS.with(|r| {
            let mut map = r.borrow_mut();
            let entry = map.entry(shop_id).or_insert((0, 0));
            entry.0 += rating as u64;
            entry.1 += 1;
        });
        Ok(())
    }

    fn revert_shop_rating(shop_id: u64, old_rating: u8, new_rating: Option<u8>) -> Result<(), DispatchError> {
        if SHOP_RATING_FAIL.with(|f| *f.borrow()) {
            return Err(DispatchError::Other("shop rating revert failed"));
        }
        SHOP_RATINGS.with(|r| {
            let mut map = r.borrow_mut();
            let entry = map.entry(shop_id).or_insert((0, 0));
            entry.0 = entry.0.saturating_sub(old_rating as u64);
            match new_rating {
                Some(nr) => {
                    entry.0 += nr as u64;
                },
                None => {
                    entry.1 = entry.1.saturating_sub(1);
                },
            }
        });
        Ok(())
    }

    fn deduct_operating_fund(_shop_id: u64, _amount: u128) -> Result<(), DispatchError> { Ok(()) }
    fn operating_balance(_shop_id: u64) -> u128 { 0 }
}

// ==================== 构建 Mock Runtime ====================

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        EntityReview: pallet_entity_review,
    }
);

// ==================== frame_system ====================

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Hash = sp_core::H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = ConstU64<250>;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

// ==================== pallet_balances ====================

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = u128;
    type DustRemoval = ();
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ConstU32<50>;
    type ReserveIdentifier = [u8; 8];
    type RuntimeHoldReason = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ConstU32<0>;
    type RuntimeFreezeReason = ();
}

// ==================== pallet_entity_review ====================

// ==================== Mock EntityProvider ====================

pub struct MockEntityProvider;

impl EntityProvider<u64> for MockEntityProvider {
    fn entity_exists(entity_id: u64) -> bool {
        ENTITIES.with(|e| e.borrow().contains_key(&entity_id))
    }

    fn is_entity_active(entity_id: u64) -> bool {
        ENTITIES.with(|e| e.borrow().get(&entity_id).map(|(_, active)| *active).unwrap_or(false))
    }

    fn entity_status(entity_id: u64) -> Option<EntityStatus> {
        ENTITIES.with(|e| e.borrow().get(&entity_id).map(|(_, active)| {
            if *active { EntityStatus::Active } else { EntityStatus::Suspended }
        }))
    }

    fn entity_owner(entity_id: u64) -> Option<u64> {
        ENTITIES.with(|e| e.borrow().get(&entity_id).map(|(owner, _)| *owner))
    }

    fn entity_account(_entity_id: u64) -> u64 { 0 }

    fn update_entity_stats(_entity_id: u64, _sales_amount: u128, _order_count: u32) -> Result<(), DispatchError> {
        Ok(())
    }

    /// M5: 检查 owner 或 admin（需持有 required_permission 位）
    fn is_entity_admin(entity_id: u64, account: &u64, required_permission: u32) -> bool {
        // Owner 始终通过
        let is_owner = ENTITIES.with(|e| {
            e.borrow().get(&entity_id).map(|(owner, _)| *account == *owner).unwrap_or(false)
        });
        if is_owner {
            return true;
        }
        // Admin 需持有 required_permission 位
        ENTITY_ADMINS.with(|m| {
            m.borrow().get(&(entity_id, *account))
                .map(|perms| perms & required_permission == required_permission)
                .unwrap_or(false)
        })
    }

    fn is_entity_locked(entity_id: u64) -> bool {
        ENTITY_LOCKED.with(|l| l.borrow().contains(&entity_id))
    }
}

parameter_types! {
    pub const ReviewWindowBlocks: u64 = 100800; // ~7 days at 6s blocks
    pub const EditWindowBlocks: u64 = 14400; // ~1 day at 6s blocks
}

impl pallet_entity_review::Config for Test {
    type EntityProvider = MockEntityProvider;
    type OrderProvider = MockOrderProvider;
    type ShopProvider = MockShopProvider;
    type MaxCidLength = ConstU32<64>;
    type MaxReviewsPerUser = ConstU32<100>;
    type ReviewWindowBlocks = ReviewWindowBlocks;
    type EditWindowBlocks = EditWindowBlocks;
    type MaxProductReviews = ConstU32<200>;
    type WeightInfo = ();
}

// ==================== 辅助函数 ====================

pub fn new_test_ext() -> sp_io::TestExternalities {
    reset_mock_state();
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
