//! Mock runtime for pallet-entity-transaction tests

use crate as pallet_entity_transaction;
use frame_support::{
    derive_impl, parameter_types,
    traits::{ConstU32, ConstU64, ConstU16},
};
use sp_runtime::BuildStorage;
use frame_support::weights::Weight;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Transaction: pallet_entity_transaction,
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

// ==================== Mock Escrow ====================

pub struct MockEscrow;

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static ESCROW_BALANCES: RefCell<HashMap<u64, u64>> = RefCell::new(HashMap::new());
    static ESCROW_STATES: RefCell<HashMap<u64, u8>> = RefCell::new(HashMap::new());
}

impl pallet_escrow::pallet::Escrow<u64, u64> for MockEscrow {
    fn escrow_account() -> u64 {
        999
    }

    fn lock_from(payer: &u64, id: u64, amount: u64) -> sp_runtime::DispatchResult {
        // 从 payer 扣余额，记到 escrow
        let current = pallet_balances::Pallet::<Test>::free_balance(payer);
        if current < amount {
            return Err(sp_runtime::DispatchError::Token(sp_runtime::TokenError::FundsUnavailable));
        }
        // 简化：直接记录 escrow 余额
        ESCROW_BALANCES.with(|b| {
            let mut map = b.borrow_mut();
            let entry = map.entry(id).or_insert(0);
            *entry = entry.saturating_add(amount);
        });
        Ok(())
    }

    fn transfer_from_escrow(id: u64, _to: &u64, amount: u64) -> sp_runtime::DispatchResult {
        ESCROW_BALANCES.with(|b| {
            let mut map = b.borrow_mut();
            let entry = map.entry(id).or_insert(0);
            if *entry < amount {
                return Err(sp_runtime::DispatchError::Token(sp_runtime::TokenError::FundsUnavailable));
            }
            *entry = entry.saturating_sub(amount);
            Ok(())
        })
    }

    fn release_all(id: u64, _to: &u64) -> sp_runtime::DispatchResult {
        ESCROW_BALANCES.with(|b| {
            b.borrow_mut().remove(&id);
        });
        Ok(())
    }

    fn refund_all(id: u64, _to: &u64) -> sp_runtime::DispatchResult {
        ESCROW_BALANCES.with(|b| {
            b.borrow_mut().remove(&id);
        });
        Ok(())
    }

    fn amount_of(id: u64) -> u64 {
        ESCROW_BALANCES.with(|b| {
            *b.borrow().get(&id).unwrap_or(&0)
        })
    }

    fn split_partial(_id: u64, _release_to: &u64, _refund_to: &u64, _bps: u16) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn set_disputed(id: u64) -> sp_runtime::DispatchResult {
        ESCROW_STATES.with(|s| s.borrow_mut().insert(id, 1));
        Ok(())
    }

    fn set_resolved(id: u64) -> sp_runtime::DispatchResult {
        ESCROW_STATES.with(|s| s.borrow_mut().insert(id, 0));
        Ok(())
    }
}

// ==================== Mock ShopProvider ====================

pub struct MockShopProvider;

impl pallet_entity_common::ShopProvider<u64> for MockShopProvider {
    fn shop_exists(shop_id: u64) -> bool {
        shop_id == SHOP_1 || shop_id == SHOP_2
    }

    fn is_shop_active(shop_id: u64) -> bool {
        shop_id == SHOP_1 || shop_id == SHOP_2
    }

    fn shop_entity_id(shop_id: u64) -> Option<u64> {
        match shop_id {
            SHOP_1 => Some(1),
            SHOP_2 => Some(2),
            _ => None,
        }
    }

    fn shop_owner(shop_id: u64) -> Option<u64> {
        match shop_id {
            SHOP_1 => Some(SELLER),
            SHOP_2 => Some(SELLER2),
            _ => None,
        }
    }

    fn shop_account(shop_id: u64) -> u64 {
        2000 + shop_id
    }

    fn shop_type(_shop_id: u64) -> Option<pallet_entity_common::ShopType> {
        Some(pallet_entity_common::ShopType::OnlineStore)
    }

    fn shop_member_mode(_shop_id: u64) -> pallet_entity_common::MemberMode {
        pallet_entity_common::MemberMode::Inherit
    }

    fn is_shop_manager(_shop_id: u64, _account: &u64) -> bool {
        false
    }

    fn shop_own_status(shop_id: u64) -> Option<pallet_entity_common::ShopOperatingStatus> {
        if shop_id == SHOP_1 || shop_id == SHOP_2 {
            Some(pallet_entity_common::ShopOperatingStatus::Active)
        } else {
            None
        }
    }

    fn effective_status(shop_id: u64) -> Option<pallet_entity_common::EffectiveShopStatus> {
        if shop_id == SHOP_1 || shop_id == SHOP_2 {
            Some(pallet_entity_common::EffectiveShopStatus::Active)
        } else {
            None
        }
    }

    fn update_shop_stats(_shop_id: u64, _sales_amount: u128, _order_count: u32) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn update_shop_rating(_shop_id: u64, _rating: u8) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn deduct_operating_fund(_shop_id: u64, _amount: u128) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn operating_balance(_shop_id: u64) -> u128 {
        10000
    }

    fn create_primary_shop(_entity_id: u64, _name: sp_std::vec::Vec<u8>, _shop_type: pallet_entity_common::ShopType, _member_mode: pallet_entity_common::MemberMode) -> Result<u64, sp_runtime::DispatchError> {
        Ok(1)
    }

    fn is_primary_shop(_shop_id: u64) -> bool {
        false
    }

    fn pause_shop(_shop_id: u64) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn resume_shop(_shop_id: u64) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn force_close_shop(_shop_id: u64) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }
}

// ==================== Mock ProductProvider ====================

pub struct MockProductProvider;

use pallet_entity_common::ProductCategory;

thread_local! {
    static PRODUCT_STOCK: RefCell<HashMap<u64, u32>> = RefCell::new(HashMap::new());
}

pub fn set_product_stock(product_id: u64, stock: u32) {
    PRODUCT_STOCK.with(|s| s.borrow_mut().insert(product_id, stock));
}

/// Product IDs:
/// 1 = Physical, shop 1, price 100, stock 10
/// 2 = Digital, shop 1, price 50
/// 3 = Service, shop 1, price 200, stock 5
/// 4 = Other, shop 2, price 150, stock 20
impl pallet_entity_common::ProductProvider<u64, u64> for MockProductProvider {
    fn product_exists(product_id: u64) -> bool {
        product_id >= 1 && product_id <= 4
    }

    fn is_product_on_sale(product_id: u64) -> bool {
        product_id >= 1 && product_id <= 4
    }

    fn product_shop_id(product_id: u64) -> Option<u64> {
        match product_id {
            1 | 2 | 3 => Some(SHOP_1),
            4 => Some(SHOP_2),
            _ => None,
        }
    }

    fn product_price(product_id: u64) -> Option<u64> {
        match product_id {
            1 => Some(100),
            2 => Some(50),
            3 => Some(200),
            4 => Some(150),
            _ => None,
        }
    }

    fn product_stock(product_id: u64) -> Option<u32> {
        PRODUCT_STOCK.with(|s| {
            s.borrow().get(&product_id).copied().or(match product_id {
                1 => Some(10),
                2 => None, // Digital: unlimited
                3 => Some(5),
                4 => Some(20),
                _ => None,
            })
        })
    }

    fn product_category(product_id: u64) -> Option<ProductCategory> {
        match product_id {
            1 => Some(ProductCategory::Physical),
            2 => Some(ProductCategory::Digital),
            3 => Some(ProductCategory::Service),
            4 => Some(ProductCategory::Other),
            _ => None,
        }
    }

    fn deduct_stock(_product_id: u64, _quantity: u32) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn restore_stock(_product_id: u64, _quantity: u32) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn add_sold_count(_product_id: u64, _quantity: u32) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }
}

// ==================== Mock EntityTokenProvider ====================

pub struct MockEntityToken;

impl pallet_entity_common::EntityTokenProvider<u64, u64> for MockEntityToken {
    fn is_token_enabled(_entity_id: u64) -> bool {
        false
    }

    fn token_balance(_entity_id: u64, _holder: &u64) -> u64 {
        0
    }

    fn reward_on_purchase(_: u64, _: &u64, _: u64) -> Result<u64, sp_runtime::DispatchError> {
        Ok(0)
    }

    fn redeem_for_discount(_: u64, _: &u64, _: u64) -> Result<u64, sp_runtime::DispatchError> {
        Ok(0)
    }

    fn transfer(_: u64, _: &u64, _: &u64, _: u64) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn reserve(_: u64, _: &u64, _: u64) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn unreserve(_: u64, _: &u64, _: u64) -> u64 {
        0
    }

    fn repatriate_reserved(_: u64, _: &u64, _: &u64, _: u64) -> Result<u64, sp_runtime::DispatchError> {
        Ok(0)
    }

    fn get_token_type(_entity_id: u64) -> pallet_entity_common::TokenType {
        pallet_entity_common::TokenType::default()
    }

    fn total_supply(_entity_id: u64) -> u64 {
        0
    }
}

// ==================== Mock CommissionHandler ====================

pub struct MockCommissionHandler;

thread_local! {
    static CANCELLED_ORDERS: RefCell<Vec<u64>> = RefCell::new(Vec::new());
}

pub fn get_cancelled_orders() -> Vec<u64> {
    CANCELLED_ORDERS.with(|c| c.borrow().clone())
}

impl pallet_entity_common::OrderCommissionHandler<u64, u64> for MockCommissionHandler {
    fn on_order_completed(_shop_id: u64, _order_id: u64, _buyer: &u64, _amount: u64, _platform_fee: u64) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn on_order_cancelled(order_id: u64) -> Result<(), sp_runtime::DispatchError> {
        CANCELLED_ORDERS.with(|c| c.borrow_mut().push(order_id));
        Ok(())
    }
}

// ==================== Test Constants ====================

pub const BUYER: u64 = 1;
pub const BUYER2: u64 = 2;
pub const SELLER: u64 = 10;
pub const SELLER2: u64 = 20;
pub const SHOP_1: u64 = 1;
pub const SHOP_2: u64 = 2;

// ==================== Pallet Config ====================

parameter_types! {
    pub PlatformAccount: u64 = 100;
}

impl pallet_entity_transaction::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type Escrow = MockEscrow;
    type ShopProvider = MockShopProvider;
    type ProductProvider = MockProductProvider;
    type EntityToken = MockEntityToken;
    type PlatformAccount = PlatformAccount;
    type PlatformFeeRate = ConstU16<200>; // 2%
    type ShipTimeout = ConstU64<100>;
    type ConfirmTimeout = ConstU64<200>;
    type ServiceConfirmTimeout = ConstU64<150>;
    type CommissionHandler = MockCommissionHandler;
    type MaxCidLength = ConstU32<64>;
}

// ==================== Test Helpers ====================

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (BUYER, 100_000),
            (BUYER2, 100_000),
            (SELLER, 100_000),
            (SELLER2, 100_000),
        ],
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        crate::NextOrderId::<Test>::put(1);
        // 清理 thread_local 状态
        ESCROW_BALANCES.with(|b| b.borrow_mut().clear());
        ESCROW_STATES.with(|s| s.borrow_mut().clear());
        PRODUCT_STOCK.with(|s| s.borrow_mut().clear());
        CANCELLED_ORDERS.with(|c| c.borrow_mut().clear());
    });
    ext
}

pub fn run_to_block(n: u32) {
    while System::block_number() < n.into() {
        let next = System::block_number() + 1;
        System::set_block_number(next);
        // 触发 on_idle
        <Transaction as frame_support::traits::Hooks<u64>>::on_idle(
            next,
            Weight::from_parts(10_000_000_000, 1_000_000),
        );
    }
}

