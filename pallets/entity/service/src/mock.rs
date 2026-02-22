use crate as pallet_entity_service;
use frame_support::{
    derive_impl,
    parameter_types,
    traits::{ConstU32, ConstU64, ConstU128},
};
use sp_runtime::BuildStorage;
use pallet_entity_common::{
    EntityProvider, EntityStatus, ShopProvider, ShopType, MemberMode,
    PricingProvider,
};
use sp_runtime::DispatchError;
use std::cell::RefCell;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        EntityService: pallet_entity_service,
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
    static PRICING: RefCell<u64> = RefCell::new(1_000_000); // 1 USDT/NEX
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

    fn shop_member_mode(_shop_id: u64) -> MemberMode {
        MemberMode::default()
    }

    fn is_shop_manager(_shop_id: u64, _account: &u64) -> bool {
        false
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

    fn update_entity_rating(_: u64, _: u8) -> Result<(), DispatchError> {
        Ok(())
    }
}

// ==================== Mock PricingProvider ====================

pub struct MockPricingProvider;

impl PricingProvider for MockPricingProvider {
    fn get_cos_usdt_price() -> u64 {
        PRICING.with(|p| *p.borrow())
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

// ==================== Pallet Config ====================

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
}

impl pallet_entity_service::Config for Test {
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
