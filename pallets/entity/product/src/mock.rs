use crate as pallet_entity_product;
use frame_support::{
    derive_impl,
    parameter_types,
    traits::{ConstU32, ConstU64, ConstU128},
};
use sp_runtime::BuildStorage;
use pallet_entity_common::{
    EntityProvider, EntityStatus, ShopProvider, ShopType,
    PricingProvider,
};
use pallet_storage_service::{IpfsPinner, SubjectType, PinTier};
use sp_runtime::DispatchError;
use std::cell::RefCell;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        EntityProduct: pallet_entity_product,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u128>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = u128;
    type AccountStore = System;
}

// ==================== Mock ShopProvider ====================

thread_local! {
    static SHOP_EXISTS: RefCell<Vec<u64>> = RefCell::new(vec![1, 2]);
    static SHOP_ACTIVE: RefCell<Vec<u64>> = RefCell::new(vec![1]);
    static SHOP_OWNERS: RefCell<Vec<(u64, u64)>> = RefCell::new(vec![(1, 1), (2, 2)]);
    static SHOP_MANAGERS: RefCell<Vec<(u64, u64)>> = RefCell::new(vec![]); // (shop_id, account)
    static ENTITY_ADMINS: RefCell<Vec<(u64, u64, u32)>> = RefCell::new(vec![]); // (entity_id, account, permissions)
    static PRICING: RefCell<u64> = RefCell::new(1_000_000); // 1 USDT/NEX
    static PRICE_STALE: RefCell<bool> = RefCell::new(false);
    // IPFS Pin tracking
    static PINNED_CIDS: RefCell<Vec<(u64, Vec<u8>)>> = RefCell::new(vec![]); // (subject_id, cid)
    static UNPINNED_CIDS: RefCell<Vec<Vec<u8>>> = RefCell::new(vec![]);
    static PIN_SHOULD_FAIL: RefCell<bool> = RefCell::new(false);
    static ENTITY_LOCKED: RefCell<std::collections::HashSet<u64>> = RefCell::new(std::collections::HashSet::new());
}

pub struct MockShopProvider;

impl ShopProvider<u64> for MockShopProvider {
    fn shop_exists(shop_id: u64) -> bool {
        SHOP_EXISTS.with(|s| s.borrow().contains(&shop_id))
    }

    fn is_shop_active(shop_id: u64) -> bool {
        SHOP_ACTIVE.with(|s| s.borrow().contains(&shop_id))
    }

    fn shop_entity_id(_shop_id: u64) -> Option<u64> {
        Some(1)
    }

    fn shop_owner(shop_id: u64) -> Option<u64> {
        SHOP_OWNERS.with(|s| {
            s.borrow().iter().find(|(id, _)| *id == shop_id).map(|(_, owner)| *owner)
        })
    }

    fn shop_account(shop_id: u64) -> u64 {
        // 派生账户: shop_id * 100 + 10
        shop_id * 100 + 10
    }

    fn shop_type(_shop_id: u64) -> Option<ShopType> {
        Some(ShopType::default())
    }

    fn is_shop_manager(shop_id: u64, account: &u64) -> bool {
        SHOP_MANAGERS.with(|m| {
            m.borrow().iter().any(|(sid, acc)| *sid == shop_id && *acc == *account)
        })
    }

    fn update_shop_stats(_: u64, _: u128, _: u32) -> Result<(), DispatchError> {
        Ok(())
    }

    fn update_shop_rating(_: u64, _: u8) -> Result<(), DispatchError> {
        Ok(())
    }

    fn deduct_operating_fund(_: u64, _: u128) -> Result<(), DispatchError> {
        Ok(())
    }

    fn operating_balance(_: u64) -> u128 {
        0
    }
}

// ==================== Mock EntityProvider ====================

pub struct MockEntityProvider;

impl EntityProvider<u64> for MockEntityProvider {
    fn entity_exists(_entity_id: u64) -> bool {
        true
    }

    fn is_entity_active(_entity_id: u64) -> bool {
        true
    }

    fn entity_status(_entity_id: u64) -> Option<EntityStatus> {
        Some(EntityStatus::default())
    }

    fn entity_owner(_entity_id: u64) -> Option<u64> {
        Some(1)
    }

    fn entity_account(_entity_id: u64) -> u64 {
        100
    }

    fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), DispatchError> {
        Ok(())
    }

    fn is_entity_admin(entity_id: u64, account: &u64, required_permission: u32) -> bool {
        ENTITY_ADMINS.with(|a| {
            a.borrow().iter().any(|(eid, acc, perms)| {
                *eid == entity_id && *acc == *account && (perms & required_permission) == required_permission
            })
        })
    }

    fn is_entity_locked(entity_id: u64) -> bool {
        ENTITY_LOCKED.with(|l| l.borrow().contains(&entity_id))
    }
}

// ==================== Mock PricingProvider ====================

pub struct MockPricingProvider;

impl PricingProvider for MockPricingProvider {
    fn get_nex_usdt_price() -> u64 {
        PRICING.with(|p| *p.borrow())
    }

    fn is_price_stale() -> bool {
        PRICE_STALE.with(|s| *s.borrow())
    }
}

// ==================== Helper functions ====================

pub fn set_shop_active(shop_id: u64, active: bool) {
    SHOP_ACTIVE.with(|s| {
        let mut v = s.borrow_mut();
        v.retain(|&id| id != shop_id);
        if active {
            v.push(shop_id);
        }
    });
}

pub fn set_pricing(price: u64) {
    PRICING.with(|p| *p.borrow_mut() = price);
}

pub fn set_price_stale(stale: bool) {
    PRICE_STALE.with(|s| *s.borrow_mut() = stale);
}

pub fn set_pin_should_fail(fail: bool) {
    PIN_SHOULD_FAIL.with(|f| *f.borrow_mut() = fail);
}

pub fn get_pinned_cids() -> Vec<(u64, Vec<u8>)> {
    PINNED_CIDS.with(|c| c.borrow().clone())
}

pub fn get_unpinned_cids() -> Vec<Vec<u8>> {
    UNPINNED_CIDS.with(|c| c.borrow().clone())
}

pub fn clear_pin_tracking() {
    PINNED_CIDS.with(|c| c.borrow_mut().clear());
    UNPINNED_CIDS.with(|c| c.borrow_mut().clear());
}

pub fn add_shop(shop_id: u64, owner: u64, active: bool) {
    SHOP_EXISTS.with(|s| {
        let mut v = s.borrow_mut();
        if !v.contains(&shop_id) {
            v.push(shop_id);
        }
    });
    SHOP_OWNERS.with(|s| {
        let mut v = s.borrow_mut();
        v.retain(|(id, _)| *id != shop_id);
        v.push((shop_id, owner));
    });
    if active {
        SHOP_ACTIVE.with(|s| {
            let mut v = s.borrow_mut();
            if !v.contains(&shop_id) {
                v.push(shop_id);
            }
        });
    }
}

pub fn add_shop_manager(shop_id: u64, account: u64) {
    SHOP_MANAGERS.with(|m| {
        let mut v = m.borrow_mut();
        if !v.iter().any(|(s, a)| *s == shop_id && *a == account) {
            v.push((shop_id, account));
        }
    });
}

pub fn set_entity_locked(entity_id: u64) {
    ENTITY_LOCKED.with(|l| l.borrow_mut().insert(entity_id));
}

pub fn add_entity_admin(entity_id: u64, account: u64, permissions: u32) {
    ENTITY_ADMINS.with(|a| {
        let mut v = a.borrow_mut();
        v.retain(|(eid, acc, _)| !(*eid == entity_id && *acc == account));
        v.push((entity_id, account, permissions));
    });
}

// ==================== Mock IpfsPinner ====================

pub struct MockIpfsPinner;

impl IpfsPinner<u64, u128> for MockIpfsPinner {
    fn pin_cid_for_subject(
        _caller: u64,
        _subject_type: SubjectType,
        subject_id: u64,
        cid: Vec<u8>,
        _tier: Option<PinTier>,
    ) -> Result<(), DispatchError> {
        if PIN_SHOULD_FAIL.with(|f| *f.borrow()) {
            return Err(DispatchError::Other("MockPinFailed"));
        }
        PINNED_CIDS.with(|c| c.borrow_mut().push((subject_id, cid)));
        Ok(())
    }

    fn unpin_cid(
        _caller: u64,
        cid: Vec<u8>,
    ) -> Result<(), DispatchError> {
        if PIN_SHOULD_FAIL.with(|f| *f.borrow()) {
            return Err(DispatchError::Other("MockUnpinFailed"));
        }
        UNPINNED_CIDS.with(|c| c.borrow_mut().push(cid));
        Ok(())
    }
}

// ==================== Pallet Config ====================

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
}

impl pallet_entity_product::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type EntityProvider = MockEntityProvider;
    type ShopProvider = MockShopProvider;
    type PricingProvider = MockPricingProvider;
    type MaxProductsPerShop = ConstU32<10>;
    type MaxCidLength = ConstU32<64>;
    type ProductDepositUsdt = ConstU64<1_000_000>;       // 1 USDT
    type MinProductDepositCos = ConstU128<100>;           // 最小 100
    type MaxProductDepositCos = ConstU128<10_000_000_000_000>; // 最大 10_000 UNIT
    type IpfsPinner = MockIpfsPinner;
    type MaxBatchSize = ConstU32<20>;
    type MaxReasonLength = ConstU32<256>;
}

// ==================== Test Externalities ====================

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1, 100_000_000_000_000),    // 账户 1（shop 1 owner）
            (2, 100_000_000_000_000),    // 账户 2（shop 2 owner）
            (3, 100_000_000_000_000),    // 账户 3（admin / manager 测试）
            (4, 100_000_000_000_000),    // 账户 4（manager 测试）
            (5, 100_000_000_000_000),    // 账户 5（无权限用户）
            (110, 50_000_000_000_000),   // shop 1 派生账户 (1*100+10)
            (210, 50_000_000_000_000),   // shop 2 派生账户 (2*100+10)
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
