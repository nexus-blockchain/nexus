//! Benchmarking for pallet-entity-market
//!
//! 全部 27 个 extrinsics 均有 benchmark。
//! benchmark 通过直接写入存储来构造前置状态，绕过外部 pallet 依赖。

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::{pallet_prelude::ConstU32, BoundedVec};
use frame_system::RawOrigin;
use pallet::*;
use pallet_entity_common::EntityTokenProvider;
use frame_support::traits::ReservableCurrency;
use sp_runtime::traits::Saturating;
use sp_runtime::SaturatedConversion;

const ENTITY_1: u64 = 1;

// ==================== Helper 函数 ====================

/// 在 test 环境下设置 mock 状态
fn setup_entity_for<T: Config>(_eid: u64, _owner: &T::AccountId) {
    #[cfg(test)]
    {
        use codec::Encode;
        use frame_support::traits::Currency;
        let bytes = _owner.encode();
        let id = if bytes.len() >= 8 {
            u64::from_le_bytes(bytes[..8].try_into().unwrap())
        } else {
            0u64
        };
        crate::mock::set_entity_owner(_eid, id);
        crate::mock::set_token_enabled(_eid, true);
        crate::mock::set_token_balance(_eid, id, 100_000_000);
        // 确保账户有 NEX 余额（benchmark 的 whitelisted_caller 可能没有 genesis 余额）
        let _ = T::Currency::deposit_creating(_owner, BalanceOf::<T>::from(100_000_000_000u128));
    }
}

/// 启用市场配置
fn seed_market_config<T: Config>(entity_id: u64) {
    MarketConfigs::<T>::insert(entity_id, MarketConfig {
        nex_enabled: true,
        min_order_amount: 1,
        order_ttl: 1000,
        paused: false,
    });
}

/// 启用市场配置（暂停状态）
fn seed_paused_market_config<T: Config>(entity_id: u64) {
    MarketConfigs::<T>::insert(entity_id, MarketConfig {
        nex_enabled: true,
        min_order_amount: 1,
        order_ttl: 1000,
        paused: true,
    });
}

/// 创建一个 Open 卖单并写入存储
fn seed_sell_order<T: Config>(entity_id: u64, maker: &T::AccountId, price: u128, amount: u128) -> u64 {
    let now = frame_system::Pallet::<T>::block_number();
    let order_id = NextOrderId::<T>::get();
    let order = TradeOrder::<T> {
        order_id,
        entity_id,
        maker: maker.clone(),
        side: OrderSide::Sell,
        order_type: OrderType::Limit,
        token_amount: amount.into(),
        filled_amount: T::TokenBalance::from(0u128),
        price: BalanceOf::<T>::from(price),
        status: OrderStatus::Open,
        created_at: now,
        expires_at: now.saturating_add(1000u32.into()),
    };
    Orders::<T>::insert(order_id, &order);
    NextOrderId::<T>::put(order_id.saturating_add(1));
    EntitySellOrders::<T>::mutate(entity_id, |orders| { let _ = orders.try_push(order_id); });
    UserOrders::<T>::mutate(maker, |orders| { let _ = orders.try_push(order_id); });
    // 更新统计
    MarketStatsStorage::<T>::mutate(entity_id, |stats| {
        stats.total_orders = stats.total_orders.saturating_add(1);
    });
    order_id
}

/// 创建一个 Open 买单并写入存储
fn seed_buy_order<T: Config>(entity_id: u64, maker: &T::AccountId, price: u128, amount: u128) -> u64 {
    let now = frame_system::Pallet::<T>::block_number();
    let order_id = NextOrderId::<T>::get();
    let order = TradeOrder::<T> {
        order_id,
        entity_id,
        maker: maker.clone(),
        side: OrderSide::Buy,
        order_type: OrderType::Limit,
        token_amount: amount.into(),
        filled_amount: T::TokenBalance::from(0u128),
        price: BalanceOf::<T>::from(price),
        status: OrderStatus::Open,
        created_at: now,
        expires_at: now.saturating_add(1000u32.into()),
    };
    Orders::<T>::insert(order_id, &order);
    NextOrderId::<T>::put(order_id.saturating_add(1));
    EntityBuyOrders::<T>::mutate(entity_id, |orders| { let _ = orders.try_push(order_id); });
    UserOrders::<T>::mutate(maker, |orders| { let _ = orders.try_push(order_id); });
    MarketStatsStorage::<T>::mutate(entity_id, |stats| {
        stats.total_orders = stats.total_orders.saturating_add(1);
    });
    order_id
}

/// 创建一个已过期的卖单
fn seed_expired_sell_order<T: Config>(entity_id: u64, maker: &T::AccountId, price: u128, amount: u128) -> u64 {
    let now = frame_system::Pallet::<T>::block_number();
    let order_id = NextOrderId::<T>::get();
    let order = TradeOrder::<T> {
        order_id,
        entity_id,
        maker: maker.clone(),
        side: OrderSide::Sell,
        order_type: OrderType::Limit,
        token_amount: amount.into(),
        filled_amount: T::TokenBalance::from(0u128),
        price: BalanceOf::<T>::from(price),
        status: OrderStatus::Open,
        created_at: now,
        expires_at: now, // 已过期
    };
    Orders::<T>::insert(order_id, &order);
    NextOrderId::<T>::put(order_id.saturating_add(1));
    EntitySellOrders::<T>::mutate(entity_id, |orders| { let _ = orders.try_push(order_id); });
    UserOrders::<T>::mutate(maker, |orders| { let _ = orders.try_push(order_id); });
    order_id
}

/// 设置价格保护配置（含熔断激活状态）
fn seed_active_circuit_breaker<T: Config>(entity_id: u64) {
    let current_block: u32 = frame_system::Pallet::<T>::block_number().saturated_into();
    PriceProtection::<T>::insert(entity_id, PriceProtectionConfig {
        enabled: true,
        max_price_deviation: 2000,
        max_slippage: 500,
        circuit_breaker_threshold: 5000,
        min_trades_for_twap: 100,
        circuit_breaker_active: true,
        circuit_breaker_until: current_block.saturating_add(1), // 即将到期
        initial_price: None,
    });
}

/// 设置价格保护配置（含已到期的熔断）
fn seed_expired_circuit_breaker<T: Config>(entity_id: u64) {
    PriceProtection::<T>::insert(entity_id, PriceProtectionConfig {
        enabled: true,
        max_price_deviation: 2000,
        max_slippage: 500,
        circuit_breaker_threshold: 5000,
        min_trades_for_twap: 100,
        circuit_breaker_active: true,
        circuit_breaker_until: 0, // 已到期
        initial_price: None,
    });
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // ==================== call_index(0): place_sell_order ====================
    #[benchmark]
    fn place_sell_order() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_market_config::<T>(ENTITY_1);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, T::TokenBalance::from(1000u128), BalanceOf::<T>::from(100u128));
    }

    // ==================== call_index(1): place_buy_order ====================
    #[benchmark]
    fn place_buy_order() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_market_config::<T>(ENTITY_1);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, T::TokenBalance::from(100u128), BalanceOf::<T>::from(100u128));
    }

    // ==================== call_index(2): take_order ====================
    #[benchmark]
    fn take_order() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_market_config::<T>(ENTITY_1);
        // 创建一个由其他人挂的卖单
        let maker: T::AccountId = account("maker", 0, 0);
        setup_entity_for::<T>(ENTITY_1, &maker);
        let order_id = seed_sell_order::<T>(ENTITY_1, &maker, 100, 1000);
        // 模拟 maker 已 reserve token
        T::TokenProvider::reserve(ENTITY_1, &maker, T::TokenBalance::from(1000u128)).ok();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), order_id, None);
    }

    // ==================== call_index(3): cancel_order ====================
    #[benchmark]
    fn cancel_order() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_market_config::<T>(ENTITY_1);
        let order_id = seed_sell_order::<T>(ENTITY_1, &caller, 100, 1000);
        // 模拟已 reserve token
        T::TokenProvider::reserve(ENTITY_1, &caller, T::TokenBalance::from(1000u128)).ok();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), order_id);
    }

    // ==================== call_index(4): configure_market ====================
    #[benchmark]
    fn configure_market() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, true, 1u128, 1000u32);
    }

    // ==================== call_index(15): configure_price_protection ====================
    #[benchmark]
    fn configure_price_protection() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, true, 2000u16, 500u16, 5000u16, 100u64);
    }

    // ==================== call_index(16): lift_circuit_breaker ====================
    #[benchmark]
    fn lift_circuit_breaker() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_expired_circuit_breaker::<T>(ENTITY_1);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1);
    }

    // ==================== call_index(17): set_initial_price ====================
    #[benchmark]
    fn set_initial_price() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, BalanceOf::<T>::from(100u128));
    }

    // ==================== call_index(12): market_buy ====================
    #[benchmark]
    fn market_buy() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_market_config::<T>(ENTITY_1);
        // 创建卖单供市价买入
        let maker: T::AccountId = account("maker", 0, 0);
        setup_entity_for::<T>(ENTITY_1, &maker);
        let _oid = seed_sell_order::<T>(ENTITY_1, &maker, 100, 10000);
        T::TokenProvider::reserve(ENTITY_1, &maker, T::TokenBalance::from(10000u128)).ok();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, T::TokenBalance::from(100u128), BalanceOf::<T>::from(100_000u128));
    }

    // ==================== call_index(13): market_sell ====================
    #[benchmark]
    fn market_sell() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_market_config::<T>(ENTITY_1);
        // 创建买单供市价卖出（maker 需要 NEX reserve）
        let maker: T::AccountId = account("maker", 0, 0);
        setup_entity_for::<T>(ENTITY_1, &maker);
        let _oid = seed_buy_order::<T>(ENTITY_1, &maker, 100, 10000);
        // maker 的 NEX 已通过 setup_entity_for 的 deposit_creating 获得
        // 模拟 reserve NEX（买单需要锁定 NEX）
        T::Currency::reserve(&maker, BalanceOf::<T>::from(1_000_000u128)).ok();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, T::TokenBalance::from(100u128), BalanceOf::<T>::from(0u128));
    }

    // ==================== call_index(23): force_cancel_order ====================
    #[benchmark]
    fn force_cancel_order() {
        let maker: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &maker);
        seed_market_config::<T>(ENTITY_1);
        let order_id = seed_sell_order::<T>(ENTITY_1, &maker, 100, 1000);
        T::TokenProvider::reserve(ENTITY_1, &maker, T::TokenBalance::from(1000u128)).ok();
        #[extrinsic_call]
        _(RawOrigin::Root, order_id);
    }

    // ==================== call_index(26): pause_market ====================
    #[benchmark]
    fn pause_market() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_market_config::<T>(ENTITY_1);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1);
    }

    // ==================== call_index(27): resume_market ====================
    #[benchmark]
    fn resume_market() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_paused_market_config::<T>(ENTITY_1);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1);
    }

    // ==================== call_index(28): batch_cancel_orders ====================
    #[benchmark]
    fn batch_cancel_orders() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_market_config::<T>(ENTITY_1);
        // 创建 5 个卖单
        let mut ids = sp_std::vec::Vec::new();
        for _ in 0..5u32 {
            let oid = seed_sell_order::<T>(ENTITY_1, &caller, 100, 100);
            T::TokenProvider::reserve(ENTITY_1, &caller, T::TokenBalance::from(100u128)).ok();
            ids.push(oid);
        }
        let bounded: BoundedVec<u64, ConstU32<50>> = ids.try_into().unwrap();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bounded);
    }

    // ==================== call_index(29): cleanup_expired_orders ====================
    #[benchmark]
    fn cleanup_expired_orders() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_market_config::<T>(ENTITY_1);
        // 创建过期卖单
        for _ in 0..5u32 {
            seed_expired_sell_order::<T>(ENTITY_1, &caller, 100, 100);
        }
        // 推进区块使其过期
        let now = frame_system::Pallet::<T>::block_number();
        frame_system::Pallet::<T>::set_block_number(now.saturating_add(10u32.into()));
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, 10u32);
    }

    // ==================== call_index(30): modify_order ====================
    #[benchmark]
    fn modify_order() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_market_config::<T>(ENTITY_1);
        let order_id = seed_sell_order::<T>(ENTITY_1, &caller, 100, 1000);
        T::TokenProvider::reserve(ENTITY_1, &caller, T::TokenBalance::from(1000u128)).ok();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), order_id, BalanceOf::<T>::from(110u128), T::TokenBalance::from(900u128));
    }

    // ==================== call_index(32): global_market_pause ====================
    #[benchmark]
    fn global_market_pause() {
        #[extrinsic_call]
        _(RawOrigin::Root, true);
    }

    // ==================== call_index(33): set_kyc_requirement ====================
    #[benchmark]
    fn set_kyc_requirement() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, 1u8);
    }

    // ==================== call_index(34): close_market ====================
    #[benchmark]
    fn close_market() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_market_config::<T>(ENTITY_1);
        // 创建一些订单供关闭时取消
        for _ in 0..3u32 {
            seed_sell_order::<T>(ENTITY_1, &caller, 100, 100);
        }
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1);
    }

    // ==================== call_index(35): cancel_all_entity_orders ====================
    #[benchmark]
    fn cancel_all_entity_orders() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_market_config::<T>(ENTITY_1);
        for _ in 0..5u32 {
            seed_sell_order::<T>(ENTITY_1, &caller, 100, 100);
        }
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1);
    }

    // ==================== call_index(36): governance_configure_market ====================
    #[benchmark]
    fn governance_configure_market() {
        setup_entity_for::<T>(ENTITY_1, &whitelisted_caller::<T::AccountId>());
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1, true, 1u128, 1000u32);
    }

    // ==================== call_index(37): force_close_market ====================
    #[benchmark]
    fn force_close_market() {
        setup_entity_for::<T>(ENTITY_1, &whitelisted_caller::<T::AccountId>());
        seed_market_config::<T>(ENTITY_1);
        let caller: T::AccountId = whitelisted_caller();
        for _ in 0..3u32 {
            seed_sell_order::<T>(ENTITY_1, &caller, 100, 100);
        }
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1);
    }

    // ==================== call_index(38): place_ioc_order ====================
    #[benchmark]
    fn place_ioc_order() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_market_config::<T>(ENTITY_1);
        // 创建对手方卖单供 IOC 撮合
        let maker: T::AccountId = account("maker", 0, 0);
        setup_entity_for::<T>(ENTITY_1, &maker);
        seed_sell_order::<T>(ENTITY_1, &maker, 100, 10000);
        T::TokenProvider::reserve(ENTITY_1, &maker, T::TokenBalance::from(10000u128)).ok();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, OrderSide::Buy, T::TokenBalance::from(100u128), BalanceOf::<T>::from(100u128));
    }

    // ==================== call_index(39): place_fok_order ====================
    #[benchmark]
    fn place_fok_order() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_market_config::<T>(ENTITY_1);
        // 创建足够的对手方卖单
        let maker: T::AccountId = account("maker", 0, 0);
        setup_entity_for::<T>(ENTITY_1, &maker);
        seed_sell_order::<T>(ENTITY_1, &maker, 100, 10000);
        T::TokenProvider::reserve(ENTITY_1, &maker, T::TokenBalance::from(10000u128)).ok();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, OrderSide::Buy, T::TokenBalance::from(100u128), BalanceOf::<T>::from(100u128));
    }

    // ==================== call_index(40): place_post_only_order ====================
    #[benchmark]
    fn place_post_only_order() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_market_config::<T>(ENTITY_1);
        // Post-Only 卖单：价格高于最优买价（无买单时任何价格都可以）
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, OrderSide::Sell, T::TokenBalance::from(1000u128), BalanceOf::<T>::from(200u128));
    }

    // ==================== call_index(41): governance_configure_price_protection ====================
    #[benchmark]
    fn governance_configure_price_protection() {
        setup_entity_for::<T>(ENTITY_1, &whitelisted_caller::<T::AccountId>());
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1, true, 2000u16, 500u16, 5000u16, 100u64);
    }

    // ==================== call_index(42): force_lift_circuit_breaker ====================
    #[benchmark]
    fn force_lift_circuit_breaker() {
        setup_entity_for::<T>(ENTITY_1, &whitelisted_caller::<T::AccountId>());
        seed_active_circuit_breaker::<T>(ENTITY_1);
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::build(), crate::mock::Test);
}
