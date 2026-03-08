//! Mock runtime for pallet-entity-order tests

use crate as pallet_entity_order;
use frame_support::{
    derive_impl, parameter_types,
    traits::{ConstU32, ConstU64},
};
use sp_runtime::BuildStorage;
use frame_support::weights::Weight;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Transaction: pallet_entity_order,
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

impl pallet_dispute_escrow::pallet::Escrow<u64, u64> for MockEscrow {
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

    fn refund_partial(id: u64, _to: &u64, amount: u64) -> sp_runtime::DispatchResult {
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

    fn release_partial(id: u64, _to: &u64, amount: u64) -> sp_runtime::DispatchResult {
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
}

// ==================== Mock ShopProvider ====================

pub struct MockShopProvider;

impl pallet_entity_common::ShopProvider<u64> for MockShopProvider {
    fn shop_exists(shop_id: u64) -> bool {
        shop_id == SHOP_1 || shop_id == SHOP_2
    }

    fn is_shop_active(shop_id: u64) -> bool {
        SHOP_ACTIVE_MAP.with(|a| {
            a.borrow().get(&shop_id).copied()
                .unwrap_or(shop_id == SHOP_1 || shop_id == SHOP_2)
        })
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

    fn create_primary_shop(_entity_id: u64, _name: sp_std::vec::Vec<u8>, _shop_type: pallet_entity_common::ShopType) -> Result<u64, sp_runtime::DispatchError> {
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

use pallet_entity_common::{ProductCategory, ProductStatus, ProductVisibility};

thread_local! {
    static PRODUCT_STOCK: RefCell<HashMap<u64, u32>> = RefCell::new(HashMap::new());
    static PRODUCT_VISIBILITY_MAP: RefCell<HashMap<u64, ProductVisibility>> = RefCell::new(HashMap::new());
    static PRODUCT_MIN_QTY: RefCell<HashMap<u64, u32>> = RefCell::new(HashMap::new());
    static PRODUCT_MAX_QTY: RefCell<HashMap<u64, u32>> = RefCell::new(HashMap::new());
    static SHOP_ACTIVE_MAP: RefCell<HashMap<u64, bool>> = RefCell::new(HashMap::new());
    static REDEEM_DISCOUNT: RefCell<HashMap<(u64, u64), u64>> = RefCell::new(HashMap::new());
}

pub fn set_product_stock(product_id: u64, stock: u32) {
    PRODUCT_STOCK.with(|s| s.borrow_mut().insert(product_id, stock));
}

#[allow(dead_code)]
pub fn set_product_visibility(product_id: u64, vis: ProductVisibility) {
    PRODUCT_VISIBILITY_MAP.with(|v| v.borrow_mut().insert(product_id, vis));
}

#[allow(dead_code)]
pub fn set_product_min_quantity(product_id: u64, min: u32) {
    PRODUCT_MIN_QTY.with(|q| q.borrow_mut().insert(product_id, min));
}

#[allow(dead_code)]
pub fn set_product_max_quantity(product_id: u64, max: u32) {
    PRODUCT_MAX_QTY.with(|q| q.borrow_mut().insert(product_id, max));
}

#[allow(dead_code)]
pub fn set_shop_active(shop_id: u64, active: bool) {
    SHOP_ACTIVE_MAP.with(|a| a.borrow_mut().insert(shop_id, active));
}

#[allow(dead_code)]
pub fn set_redeem_discount(entity_id: u64, account: u64, discount: u64) {
    REDEEM_DISCOUNT.with(|d| d.borrow_mut().insert((entity_id, account), discount));
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

    fn deduct_stock(product_id: u64, quantity: u32) -> Result<(), sp_runtime::DispatchError> {
        PRODUCT_STOCK.with(|s| {
            let mut map = s.borrow_mut();
            if let Some(stock) = map.get_mut(&product_id) {
                if *stock > 0 {
                    *stock = stock.saturating_sub(quantity);
                }
            }
        });
        Ok(())
    }

    fn restore_stock(product_id: u64, quantity: u32) -> Result<(), sp_runtime::DispatchError> {
        PRODUCT_STOCK.with(|s| {
            let mut map = s.borrow_mut();
            if let Some(stock) = map.get_mut(&product_id) {
                *stock = stock.saturating_add(quantity);
            }
        });
        Ok(())
    }

    fn add_sold_count(_product_id: u64, _quantity: u32) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn product_visibility(product_id: u64) -> Option<pallet_entity_common::ProductVisibility> {
        Some(PRODUCT_VISIBILITY_MAP.with(|v| {
            v.borrow().get(&product_id).cloned()
                .unwrap_or(pallet_entity_common::ProductVisibility::Public)
        }))
    }

    fn product_min_order_quantity(product_id: u64) -> Option<u32> {
        PRODUCT_MIN_QTY.with(|q| q.borrow().get(&product_id).copied())
    }
    fn product_max_order_quantity(product_id: u64) -> Option<u32> {
        PRODUCT_MAX_QTY.with(|q| q.borrow().get(&product_id).copied())
    }

    fn get_product_info(product_id: u64) -> Option<pallet_entity_common::ProductQueryInfo<u64>> {
        if product_id < 1 || product_id > 4 { return None; }
        let default_stock: u32 = match product_id {
            1 => 10, 2 => 0, 3 => 5, 4 => 20, _ => 0,
        };
        let stock = PRODUCT_STOCK.with(|s| {
            s.borrow().get(&product_id).copied().unwrap_or(default_stock)
        });
        let status = if stock == 0 && default_stock > 0 {
            ProductStatus::SoldOut
        } else {
            ProductStatus::OnSale
        };
        let visibility = PRODUCT_VISIBILITY_MAP.with(|v| {
            v.borrow().get(&product_id).cloned().unwrap_or(ProductVisibility::Public)
        });
        let min_order_quantity = PRODUCT_MIN_QTY.with(|q| {
            q.borrow().get(&product_id).copied().unwrap_or(0)
        });
        let max_order_quantity = PRODUCT_MAX_QTY.with(|q| {
            q.borrow().get(&product_id).copied().unwrap_or(0)
        });
        Some(pallet_entity_common::ProductQueryInfo {
            shop_id: match product_id { 1 | 2 | 3 => SHOP_1, 4 => SHOP_2, _ => 0 },
            price: match product_id { 1 => 100, 2 => 50, 3 => 200, 4 => 150, _ => 0 },
            usdt_price: 0,
            stock,
            status,
            category: match product_id {
                1 => ProductCategory::Physical,
                2 => ProductCategory::Digital,
                3 => ProductCategory::Service,
                4 => ProductCategory::Other,
                _ => ProductCategory::Other,
            },
            visibility,
            min_order_quantity,
            max_order_quantity,
        })
    }
}

// ==================== Mock EntityTokenProvider ====================

pub struct MockEntityToken;

thread_local! {
    static TOKEN_ENABLED: RefCell<HashMap<u64, bool>> = RefCell::new(HashMap::new());
    // (entity_id, account) → free balance
    static TOKEN_BALANCES: RefCell<HashMap<(u64, u64), u64>> = RefCell::new(HashMap::new());
    // (entity_id, account) → reserved balance
    static TOKEN_RESERVED: RefCell<HashMap<(u64, u64), u64>> = RefCell::new(HashMap::new());
}

#[allow(dead_code)]
pub fn set_token_enabled(entity_id: u64, enabled: bool) {
    TOKEN_ENABLED.with(|e| e.borrow_mut().insert(entity_id, enabled));
}

#[allow(dead_code)]
pub fn set_token_balance(entity_id: u64, holder: u64, balance: u64) {
    TOKEN_BALANCES.with(|b| b.borrow_mut().insert((entity_id, holder), balance));
}

#[allow(dead_code)]
pub fn get_token_balance(entity_id: u64, holder: u64) -> u64 {
    TOKEN_BALANCES.with(|b| b.borrow().get(&(entity_id, holder)).copied().unwrap_or(0))
}

#[allow(dead_code)]
pub fn get_token_reserved(entity_id: u64, holder: u64) -> u64 {
    TOKEN_RESERVED.with(|r| r.borrow().get(&(entity_id, holder)).copied().unwrap_or(0))
}

impl pallet_entity_common::EntityTokenProvider<u64, u64> for MockEntityToken {
    fn is_token_enabled(entity_id: u64) -> bool {
        TOKEN_ENABLED.with(|e| e.borrow().get(&entity_id).copied().unwrap_or(false))
    }

    fn token_balance(entity_id: u64, holder: &u64) -> u64 {
        TOKEN_BALANCES.with(|b| b.borrow().get(&(entity_id, *holder)).copied().unwrap_or(0))
    }

    fn reward_on_purchase(_: u64, _: &u64, _: u64) -> Result<u64, sp_runtime::DispatchError> {
        Ok(0)
    }

    fn redeem_for_discount(entity_id: u64, who: &u64, tokens: u64) -> Result<u64, sp_runtime::DispatchError> {
        let discount = REDEEM_DISCOUNT.with(|d| {
            d.borrow().get(&(entity_id, *who)).copied().unwrap_or(0)
        });
        if discount > 0 {
            TOKEN_BALANCES.with(|b| {
                let mut map = b.borrow_mut();
                let bal = map.get(&(entity_id, *who)).copied().unwrap_or(0);
                if bal < tokens {
                    return Err(sp_runtime::DispatchError::Other("InsufficientTokenBalance"));
                }
                map.insert((entity_id, *who), bal - tokens);
                Ok(())
            })?;
            Ok(discount)
        } else {
            Ok(0)
        }
    }

    fn transfer(_: u64, _: &u64, _: &u64, _: u64) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn reserve(entity_id: u64, who: &u64, amount: u64) -> Result<(), sp_runtime::DispatchError> {
        TOKEN_BALANCES.with(|b| {
            let mut map = b.borrow_mut();
            let bal = map.get(&(entity_id, *who)).copied().unwrap_or(0);
            if bal < amount {
                return Err(sp_runtime::DispatchError::Other("InsufficientTokenBalance"));
            }
            map.insert((entity_id, *who), bal - amount);
            TOKEN_RESERVED.with(|r| {
                let mut rmap = r.borrow_mut();
                let reserved = rmap.get(&(entity_id, *who)).copied().unwrap_or(0);
                rmap.insert((entity_id, *who), reserved + amount);
            });
            Ok(())
        })
    }

    fn unreserve(entity_id: u64, who: &u64, amount: u64) -> u64 {
        TOKEN_RESERVED.with(|r| {
            let mut rmap = r.borrow_mut();
            let reserved = rmap.get(&(entity_id, *who)).copied().unwrap_or(0);
            let actual = reserved.min(amount);
            rmap.insert((entity_id, *who), reserved - actual);
            TOKEN_BALANCES.with(|b| {
                let mut map = b.borrow_mut();
                let bal = map.get(&(entity_id, *who)).copied().unwrap_or(0);
                map.insert((entity_id, *who), bal + actual);
            });
            actual
        })
    }

    fn repatriate_reserved(entity_id: u64, from: &u64, to: &u64, amount: u64) -> Result<u64, sp_runtime::DispatchError> {
        TOKEN_RESERVED.with(|r| {
            let mut rmap = r.borrow_mut();
            let reserved = rmap.get(&(entity_id, *from)).copied().unwrap_or(0);
            let actual = reserved.min(amount);
            rmap.insert((entity_id, *from), reserved - actual);
            TOKEN_BALANCES.with(|b| {
                let mut map = b.borrow_mut();
                let bal = map.get(&(entity_id, *to)).copied().unwrap_or(0);
                map.insert((entity_id, *to), bal + actual);
            });
            Ok(amount - actual) // return shortfall
        })
    }

    fn get_token_type(_entity_id: u64) -> pallet_entity_common::TokenType {
        pallet_entity_common::TokenType::default()
    }

    fn total_supply(_entity_id: u64) -> u64 {
        0
    }

    fn governance_burn(_entity_id: u64, _amount: u64) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }
    fn available_balance(_: u64, _: &u64) -> u64 { 0 }
}

// ==================== Mock ShoppingBalanceProvider ====================

pub struct MockShoppingBalanceProvider;

thread_local! {
    static SHOPPING_BALANCES: RefCell<HashMap<(u64, u64), u64>> = RefCell::new(HashMap::new());
}

#[allow(dead_code)]
pub fn set_shopping_balance(entity_id: u64, account: u64, amount: u64) {
    SHOPPING_BALANCES.with(|b| b.borrow_mut().insert((entity_id, account), amount));
}

impl pallet_entity_common::ShoppingBalanceProvider<u64, u64> for MockShoppingBalanceProvider {
    fn shopping_balance(entity_id: u64, account: &u64) -> u64 {
        SHOPPING_BALANCES.with(|b| *b.borrow().get(&(entity_id, *account)).unwrap_or(&0))
    }

    fn consume_shopping_balance(entity_id: u64, account: &u64, amount: u64) -> Result<(), sp_runtime::DispatchError> {
        SHOPPING_BALANCES.with(|b| {
            let mut map = b.borrow_mut();
            let balance = map.entry((entity_id, *account)).or_insert(0);
            if *balance < amount {
                return Err(sp_runtime::DispatchError::Other("InsufficientShoppingBalance"));
            }
            *balance -= amount;
            // Simulate: transfer NEX from Entity to buyer (in real impl, commission-core does this)
            // For tests, we just credit the buyer's balance conceptually
            Ok(())
        })
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
    fn on_order_completed(_entity_id: u64, _shop_id: u64, _order_id: u64, _buyer: &u64, _amount: u64, _platform_fee: u64) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }

    fn on_order_cancelled(order_id: u64) -> Result<(), sp_runtime::DispatchError> {
        CANCELLED_ORDERS.with(|c| c.borrow_mut().push(order_id));
        Ok(())
    }
}

// ==================== Mock TokenCommissionHandler ====================

pub struct MockTokenCommissionHandler;

thread_local! {
    static TOKEN_CANCELLED_ORDERS: RefCell<Vec<u64>> = RefCell::new(Vec::new());
    static TOKEN_COMPLETED_ORDERS: RefCell<Vec<(u64, u64, u64, u64, u128, u128)>> = RefCell::new(Vec::new());
    static TOKEN_FEE_RATES: RefCell<HashMap<u64, u16>> = RefCell::new(HashMap::new());
}

#[allow(dead_code)]
pub fn set_token_fee_rate(entity_id: u64, rate: u16) {
    TOKEN_FEE_RATES.with(|r| r.borrow_mut().insert(entity_id, rate));
}

#[allow(dead_code)]
pub fn get_token_cancelled_orders() -> Vec<u64> {
    TOKEN_CANCELLED_ORDERS.with(|c| c.borrow().clone())
}

#[allow(dead_code)]
pub fn get_token_completed_orders() -> Vec<(u64, u64, u64, u64, u128, u128)> {
    TOKEN_COMPLETED_ORDERS.with(|c| c.borrow().clone())
}

impl pallet_entity_common::TokenOrderCommissionHandler<u64> for MockTokenCommissionHandler {
    fn on_token_order_completed(entity_id: u64, shop_id: u64, order_id: u64, buyer: &u64, token_amount: u128, token_platform_fee: u128) -> Result<(), sp_runtime::DispatchError> {
        TOKEN_COMPLETED_ORDERS.with(|c| c.borrow_mut().push((entity_id, shop_id, order_id, *buyer, token_amount, token_platform_fee)));
        Ok(())
    }

    fn on_token_order_cancelled(order_id: u64) -> Result<(), sp_runtime::DispatchError> {
        TOKEN_CANCELLED_ORDERS.with(|c| c.borrow_mut().push(order_id));
        Ok(())
    }

    fn token_platform_fee_rate(entity_id: u64) -> u16 {
        TOKEN_FEE_RATES.with(|r| *r.borrow().get(&entity_id).unwrap_or(&0))
    }

    fn entity_account(entity_id: u64) -> u64 { entity_id + 9000 }
}

// ==================== Mock PricingProvider ====================

pub struct MockPricingProvider;

impl pallet_entity_common::PricingProvider for MockPricingProvider {
    fn get_nex_usdt_price() -> u64 {
        // 测试价格: 1 USDT/NEX (精度 10^6)
        1_000_000
    }
}

// ==================== Mock TokenPriceProvider ====================

pub struct MockTokenPriceProvider;

thread_local! {
    static TOKEN_PRICE_NEX: RefCell<HashMap<u64, u64>> = RefCell::new(HashMap::new());
    static TOKEN_PRICE_RELIABLE: RefCell<HashMap<u64, bool>> = RefCell::new(HashMap::new());
}

#[allow(dead_code)]
pub fn set_token_price(entity_id: u64, price_nex: u64) {
    TOKEN_PRICE_NEX.with(|p| p.borrow_mut().insert(entity_id, price_nex));
}

#[allow(dead_code)]
pub fn set_token_price_reliable(entity_id: u64, reliable: bool) {
    TOKEN_PRICE_RELIABLE.with(|r| r.borrow_mut().insert(entity_id, reliable));
}

impl pallet_entity_common::EntityTokenPriceProvider for MockTokenPriceProvider {
    type Balance = u64;

    fn get_token_price(entity_id: u64) -> Option<u64> {
        TOKEN_PRICE_NEX.with(|p| p.borrow().get(&entity_id).copied())
    }

    fn get_token_price_usdt(_entity_id: u64) -> Option<u64> {
        None
    }

    fn token_price_confidence(entity_id: u64) -> u8 {
        if TOKEN_PRICE_NEX.with(|p| p.borrow().contains_key(&entity_id)) { 80 } else { 0 }
    }

    fn is_token_price_stale(_entity_id: u64, _max_age_blocks: u32) -> bool {
        false
    }

    fn is_token_price_reliable(entity_id: u64) -> bool {
        TOKEN_PRICE_RELIABLE.with(|r| r.borrow().get(&entity_id).copied().unwrap_or(false))
    }
}

// ==================== Mock MemberProvider ====================

pub struct MockMemberProvider;

thread_local! {
    static MEMBER_SET: RefCell<HashMap<(u64, u64), bool>> = RefCell::new(HashMap::new());
    static MEMBER_LEVELS: RefCell<HashMap<(u64, u64), u8>> = RefCell::new(HashMap::new());
}

#[allow(dead_code)]
pub fn set_member(entity_id: u64, account: u64, is_member: bool) {
    MEMBER_SET.with(|m| m.borrow_mut().insert((entity_id, account), is_member));
}

#[allow(dead_code)]
pub fn set_member_level(entity_id: u64, account: u64, level: u8) {
    MEMBER_LEVELS.with(|l| l.borrow_mut().insert((entity_id, account), level));
}

thread_local! {
    static BANNED_SET: RefCell<HashMap<(u64, u64), bool>> = RefCell::new(HashMap::new());
    static LEVEL_DISCOUNTS: RefCell<HashMap<(u64, u8), u16>> = RefCell::new(HashMap::new());
}

#[allow(dead_code)]
pub fn set_banned(entity_id: u64, account: u64, banned: bool) {
    BANNED_SET.with(|b| b.borrow_mut().insert((entity_id, account), banned));
}

#[allow(dead_code)]
pub fn set_level_discount(entity_id: u64, level_id: u8, discount_bps: u16) {
    LEVEL_DISCOUNTS.with(|d| d.borrow_mut().insert((entity_id, level_id), discount_bps));
}

impl pallet_entity_common::MemberProvider<u64> for MockMemberProvider {
    fn is_member(entity_id: u64, account: &u64) -> bool {
        MEMBER_SET.with(|m| m.borrow().get(&(entity_id, *account)).copied().unwrap_or(false))
    }

    fn get_referrer(_entity_id: u64, _account: &u64) -> Option<u64> { None }
    fn custom_level_id(_entity_id: u64, _account: &u64) -> u8 { 0 }

    fn get_effective_level(entity_id: u64, account: &u64) -> u8 {
        MEMBER_LEVELS.with(|l| l.borrow().get(&(entity_id, *account)).copied().unwrap_or(0))
    }

    fn get_level_discount(entity_id: u64, level_id: u8) -> u16 {
        LEVEL_DISCOUNTS.with(|d| d.borrow().get(&(entity_id, level_id)).copied().unwrap_or(0))
    }

    fn get_level_commission_bonus(_entity_id: u64, _level_id: u8) -> u16 { 0 }
    fn uses_custom_levels(_entity_id: u64) -> bool { false }
    fn get_member_stats(_entity_id: u64, _account: &u64) -> (u32, u32, u128) { (0, 0, 0) }

    fn is_banned(entity_id: u64, account: &u64) -> bool {
        BANNED_SET.with(|b| b.borrow().get(&(entity_id, *account)).copied().unwrap_or(false))
    }

    fn auto_register(entity_id: u64, account: &u64, _referrer: Option<u64>) -> Result<(), sp_runtime::DispatchError> {
        MEMBER_REGISTERED.with(|r| r.borrow_mut().push((entity_id, *account)));
        Ok(())
    }
    fn update_spent(entity_id: u64, account: &u64, amount_usdt: u64) -> Result<(), sp_runtime::DispatchError> {
        MEMBER_SPENT.with(|s| s.borrow_mut().push((entity_id, *account, amount_usdt)));
        Ok(())
    }
    fn check_order_upgrade_rules(_entity_id: u64, _buyer: &u64, _product_id: u64, _amount_usdt: u64) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }
}

thread_local! {
    static MEMBER_REGISTERED: RefCell<Vec<(u64, u64)>> = RefCell::new(Vec::new());
    static MEMBER_SPENT: RefCell<Vec<(u64, u64, u64)>> = RefCell::new(Vec::new());
}

#[allow(dead_code)]
pub fn get_member_registered() -> Vec<(u64, u64)> {
    MEMBER_REGISTERED.with(|r| r.borrow().clone())
}

#[allow(dead_code)]
pub fn get_member_spent() -> Vec<(u64, u64, u64)> {
    MEMBER_SPENT.with(|s| s.borrow().clone())
}

// ==================== Test Constants ====================

pub const BUYER: u64 = 1;
pub const BUYER2: u64 = 2;
pub const SELLER: u64 = 10;
pub const SELLER2: u64 = 20;
pub const SHOP_1: u64 = 1;
pub const SHOP_2: u64 = 2;
pub const ENTITY_1: u64 = 1;

// ==================== Pallet Config ====================

parameter_types! {
    pub PlatformAccount: u64 = 100;
}

impl pallet_entity_order::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type Escrow = MockEscrow;
    type ShopProvider = MockShopProvider;
    type ProductProvider = MockProductProvider;
    type EntityToken = MockEntityToken;
    type PlatformAccount = PlatformAccount;

    type ShipTimeout = ConstU64<100>;
    type ConfirmTimeout = ConstU64<200>;
    type ServiceConfirmTimeout = ConstU64<150>;
    type DisputeTimeout = ConstU64<300>;
    type ConfirmExtension = ConstU64<100>;
    type CommissionHandler = MockCommissionHandler;
    type TokenCommissionHandler = MockTokenCommissionHandler;
    type ShoppingBalance = MockShoppingBalanceProvider;
    type PricingProvider = MockPricingProvider;
    type TokenPriceProvider = MockTokenPriceProvider;
    type MemberProvider = MockMemberProvider;
    type MaxCidLength = ConstU32<64>;
    type MaxBuyerOrders = ConstU32<1000>;
    type MaxShopOrders = ConstU32<10000>;
    type MaxExpiryQueueSize = ConstU32<500>;
    type WeightInfo = ();
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
        PRODUCT_VISIBILITY_MAP.with(|v| v.borrow_mut().clear());
        PRODUCT_MIN_QTY.with(|q| q.borrow_mut().clear());
        PRODUCT_MAX_QTY.with(|q| q.borrow_mut().clear());
        SHOP_ACTIVE_MAP.with(|a| a.borrow_mut().clear());
        REDEEM_DISCOUNT.with(|d| d.borrow_mut().clear());
        CANCELLED_ORDERS.with(|c| c.borrow_mut().clear());
        TOKEN_CANCELLED_ORDERS.with(|c| c.borrow_mut().clear());
        TOKEN_COMPLETED_ORDERS.with(|c| c.borrow_mut().clear());
        TOKEN_ENABLED.with(|e| e.borrow_mut().clear());
        TOKEN_BALANCES.with(|b| b.borrow_mut().clear());
        TOKEN_RESERVED.with(|r| r.borrow_mut().clear());
        SHOPPING_BALANCES.with(|b| b.borrow_mut().clear());
        MEMBER_REGISTERED.with(|r| r.borrow_mut().clear());
        MEMBER_SPENT.with(|s| s.borrow_mut().clear());
        MEMBER_SET.with(|m| m.borrow_mut().clear());
        MEMBER_LEVELS.with(|l| l.borrow_mut().clear());
        BANNED_SET.with(|b| b.borrow_mut().clear());
        LEVEL_DISCOUNTS.with(|d| d.borrow_mut().clear());
        TOKEN_FEE_RATES.with(|r| r.borrow_mut().clear());
        TOKEN_PRICE_NEX.with(|p| p.borrow_mut().clear());
        TOKEN_PRICE_RELIABLE.with(|r| r.borrow_mut().clear());
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

