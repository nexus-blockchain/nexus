use crate::mock::*;
use crate::pallet::*;
use frame_support::{assert_noop, assert_ok, traits::Hooks};

// ==================== NEX 通道：挂单 ====================

#[test]
fn place_sell_order_works() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));
        let order = Orders::<Test>::get(0).expect("order exists");
        assert_eq!(order.maker, ALICE);
        assert_eq!(order.side, OrderSide::Sell);
        assert_eq!(order.token_amount, 1000u128);
        assert_eq!(order.price, 100u128);
        assert_eq!(order.status, OrderStatus::Open);
        // H1: Token 应已被锁定
        assert_eq!(get_token_balance(ENTITY_ID, ALICE), 10_000_000 - 1000);
        assert_eq!(get_token_reserved(ENTITY_ID, ALICE), 1000);
    });
}

#[test]
fn place_sell_order_fails_market_not_enabled() {
    ExtBuilder::build().execute_with(|| {
        // M6: 未配置市场 → MarketNotEnabled
        assert_noop!(
            EntityMarket::place_sell_order(RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100),
            Error::<Test>::MarketNotEnabled
        );
    });
}

#[test]
fn place_sell_order_fails_zero_price() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        assert_noop!(
            EntityMarket::place_sell_order(RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 0),
            Error::<Test>::ZeroPrice
        );
    });
}

#[test]
fn place_sell_order_fails_zero_amount() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        assert_noop!(
            EntityMarket::place_sell_order(RuntimeOrigin::signed(ALICE), ENTITY_ID, 0, 100),
            Error::<Test>::AmountTooSmall
        );
    });
}

#[test]
fn place_sell_order_fails_insufficient_token() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        // ALICE has 10_000_000, try to sell 20_000_000
        assert_noop!(
            EntityMarket::place_sell_order(RuntimeOrigin::signed(ALICE), ENTITY_ID, 20_000_000, 100),
            Error::<Test>::InsufficientTokenBalance
        );
    });
}

#[test]
fn place_buy_order_works() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        // price=100, amount=1000 → total NEX = 100_000
        assert_ok!(EntityMarket::place_buy_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));
        let order = Orders::<Test>::get(0).expect("order exists");
        assert_eq!(order.maker, ALICE);
        assert_eq!(order.side, OrderSide::Buy);
        // NEX should be reserved
        assert_eq!(Balances::reserved_balance(ALICE), 100_000);
    });
}

#[test]
fn place_buy_order_fails_insufficient_nex() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        // price=1_000_000_000, amount=1000 → overflow or insufficient
        assert_noop!(
            EntityMarket::place_buy_order(RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 1_000_000_000_000),
            Error::<Test>::InsufficientBalance
        );
    });
}

// ==================== NEX 通道：吃单 ====================

#[test]
fn take_sell_order_works() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        // ALICE 挂卖单: 1000 Token @ 100 NEX
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));

        let bob_nex_before = Balances::free_balance(BOB);

        // BOB 吃单
        assert_ok!(EntityMarket::take_order(
            RuntimeOrigin::signed(BOB), 0, None
        ));

        // H2: BOB 应该收到 Token
        assert_eq!(get_token_balance(ENTITY_ID, BOB), 10_000_000 + 1000);
        // ALICE 的 reserved Token 应减少
        assert_eq!(get_token_reserved(ENTITY_ID, ALICE), 0);

        // BOB 支付 NEX: net_amount(99_000) to maker + fee(1_000) to shop_owner = 100_000 total
        let total_paid = bob_nex_before - Balances::free_balance(BOB);
        assert_eq!(total_paid, 100_000);

        // 订单应该是 Filled
        let order = Orders::<Test>::get(0).expect("order exists");
        assert_eq!(order.status, OrderStatus::Filled);

        // M2: UserOrders 应已清除
        assert!(UserOrders::<Test>::get(ALICE).is_empty());
    });
}

#[test]
fn take_buy_order_works() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        // ALICE 挂买单: 1000 Token @ 100 NEX
        assert_ok!(EntityMarket::place_buy_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));

        let bob_token_before = get_token_balance(ENTITY_ID, BOB);

        // BOB 吃买单 (卖 Token)
        assert_ok!(EntityMarket::take_order(
            RuntimeOrigin::signed(BOB), 0, None
        ));

        // H3: ALICE 应该收到 Token
        assert_eq!(get_token_balance(ENTITY_ID, ALICE), 10_000_000 + 1000);
        // BOB 的 Token 应减少
        assert_eq!(get_token_balance(ENTITY_ID, BOB), bob_token_before - 1000);
        // BOB 应收到 NEX (扣除手续费)
        // ALICE 的 reserved NEX 应已释放
        assert_eq!(Balances::reserved_balance(ALICE), 0);

        let order = Orders::<Test>::get(0).expect("order exists");
        assert_eq!(order.status, OrderStatus::Filled);
    });
}

#[test]
fn take_order_partial_fill() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        // ALICE 挂卖单: 2000 Token @ 100 NEX
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 2000, 100
        ));

        // BOB 只吃 1000
        assert_ok!(EntityMarket::take_order(
            RuntimeOrigin::signed(BOB), 0, Some(1000)
        ));

        let order = Orders::<Test>::get(0).expect("order exists");
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
        assert_eq!(order.filled_amount, 1000u128);

        // UserOrders 应还保留（未完全成交）
        assert!(!UserOrders::<Test>::get(ALICE).is_empty());
    });
}

#[test]
fn take_order_fails_own_order() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));
        assert_noop!(
            EntityMarket::take_order(RuntimeOrigin::signed(ALICE), 0, None),
            Error::<Test>::CannotTakeOwnOrder
        );
    });
}

// ==================== NEX 通道：取消订单 ====================

#[test]
fn cancel_sell_order_works() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));
        // H4: Token 已锁定
        assert_eq!(get_token_reserved(ENTITY_ID, ALICE), 1000);

        assert_ok!(EntityMarket::cancel_order(RuntimeOrigin::signed(ALICE), 0));

        // H4: Token 应退还
        assert_eq!(get_token_reserved(ENTITY_ID, ALICE), 0);
        assert_eq!(get_token_balance(ENTITY_ID, ALICE), 10_000_000);

        let order = Orders::<Test>::get(0).expect("order exists");
        assert_eq!(order.status, OrderStatus::Cancelled);

        // M2: UserOrders 应已清除
        assert!(UserOrders::<Test>::get(ALICE).is_empty());
    });
}

#[test]
fn cancel_buy_order_works() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        assert_ok!(EntityMarket::place_buy_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));
        assert_eq!(Balances::reserved_balance(ALICE), 100_000);

        assert_ok!(EntityMarket::cancel_order(RuntimeOrigin::signed(ALICE), 0));

        assert_eq!(Balances::reserved_balance(ALICE), 0);
        let order = Orders::<Test>::get(0).expect("order exists");
        assert_eq!(order.status, OrderStatus::Cancelled);
    });
}

#[test]
fn cancel_order_fails_not_owner() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));
        assert_noop!(
            EntityMarket::cancel_order(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::NotOrderOwner
        );
    });
}

// ==================== 市场配置 ====================

#[test]
fn configure_market_works() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityMarket::configure_market(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID,
            true, 200, 10, 500,
        ));
        let config = MarketConfigs::<Test>::get(ENTITY_ID).expect("config exists");
        assert!(config.nex_enabled);
        assert_eq!(config.fee_rate, 200);
    });
}

#[test]
fn configure_market_fails_not_owner() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityMarket::configure_market(
                RuntimeOrigin::signed(ALICE), ENTITY_ID,
                true, 100, 1, 1000,
            ),
            Error::<Test>::NotEntityOwner
        );
    });
}

#[test]
fn configure_market_fails_invalid_fee_rate() {
    ExtBuilder::build().execute_with(|| {
        // H8: fee_rate > 5000 should fail
        assert_noop!(
            EntityMarket::configure_market(
                RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID,
                true, 5001, 1, 1000,
            ),
            Error::<Test>::InvalidFeeRate
        );
    });
}

// ==================== 价格保护配置 ====================

#[test]
fn configure_price_protection_works() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityMarket::configure_price_protection(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID,
            true, 2000, 500, 5000, 100,
        ));
        let config = PriceProtection::<Test>::get(ENTITY_ID).expect("config exists");
        assert!(config.enabled);
        assert_eq!(config.max_price_deviation, 2000);
    });
}

#[test]
fn configure_price_protection_fails_invalid_bps() {
    ExtBuilder::build().execute_with(|| {
        // M4: bps > 10000 should fail
        assert_noop!(
            EntityMarket::configure_price_protection(
                RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID,
                true, 10001, 500, 5000, 100,
            ),
            Error::<Test>::InvalidBasisPoints
        );
        assert_noop!(
            EntityMarket::configure_price_protection(
                RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID,
                true, 2000, 10001, 5000, 100,
            ),
            Error::<Test>::InvalidBasisPoints
        );
        assert_noop!(
            EntityMarket::configure_price_protection(
                RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID,
                true, 2000, 500, 10001, 100,
            ),
            Error::<Test>::InvalidBasisPoints
        );
    });
}

// ==================== 熔断 ====================

#[test]
fn lift_circuit_breaker_fails_not_active() {
    ExtBuilder::build().execute_with(|| {
        // M3: 熔断未激活时调用应失败（审计修复 H3: 使用正确的错误类型）
        assert_noop!(
            EntityMarket::lift_circuit_breaker(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID),
            Error::<Test>::CircuitBreakerNotActive
        );
    });
}

// ==================== 初始价格 ====================

#[test]
fn set_initial_price_works() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityMarket::set_initial_price(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID, 500
        ));
        let config = PriceProtection::<Test>::get(ENTITY_ID).expect("config exists");
        assert_eq!(config.initial_price, Some(500u128));
    });
}

// ==================== 市价单 ====================

#[test]
fn market_buy_works() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        // ALICE 挂卖单: 2000 Token @ 100 NEX
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 2000, 100
        ));

        // BOB 市价买 1000 Token, max_cost = 200_000
        assert_ok!(EntityMarket::market_buy(
            RuntimeOrigin::signed(BOB), ENTITY_ID, 1000, 200_000
        ));

        // H5: BOB 应收到 Token
        assert_eq!(get_token_balance(ENTITY_ID, BOB), 10_000_000 + 1000);
    });
}

#[test]
fn market_buy_fails_no_orders() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        assert_noop!(
            EntityMarket::market_buy(RuntimeOrigin::signed(BOB), ENTITY_ID, 1000, 200_000),
            Error::<Test>::NoOrdersAvailable
        );
    });
}

#[test]
fn market_sell_works() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        // ALICE 挂买单: 2000 Token @ 100 NEX
        assert_ok!(EntityMarket::place_buy_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 2000, 100
        ));

        let bob_nex_before = Balances::free_balance(BOB);

        // BOB 市价卖 1000 Token, min_receive = 50_000
        assert_ok!(EntityMarket::market_sell(
            RuntimeOrigin::signed(BOB), ENTITY_ID, 1000, 50_000
        ));

        // H5: BOB Token 应减少
        assert_eq!(get_token_balance(ENTITY_ID, BOB), 10_000_000 - 1000);
        // BOB 应收到 NEX
        assert!(Balances::free_balance(BOB) > bob_nex_before);
    });
}

// ==================== M6: 未配置市场默认不启用 ====================

#[test]
fn market_not_enabled_by_default() {
    ExtBuilder::build().execute_with(|| {
        // 不调用 configure_market → 默认 nex_enabled=false
        assert_noop!(
            EntityMarket::place_sell_order(RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100),
            Error::<Test>::MarketNotEnabled
        );
    });
}

// ==================== 查询接口 ====================

#[test]
fn get_order_book_depth_works() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));
        assert_ok!(EntityMarket::place_buy_order(
            RuntimeOrigin::signed(BOB), ENTITY_ID, 500, 80
        ));

        let depth = EntityMarket::get_order_book_depth(ENTITY_ID, 10);
        assert!(!depth.asks.is_empty());
        assert!(!depth.bids.is_empty());
        assert_eq!(depth.best_ask, Some(100u128));
        assert_eq!(depth.best_bid, Some(80u128));
    });
}

#[test]
fn get_market_summary_works() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));

        let summary = EntityMarket::get_market_summary(ENTITY_ID);
        assert_eq!(summary.best_ask, Some(100u128));
        assert_eq!(summary.total_ask_amount, 1000u128);
    });
}

// ==================== TWAP 和统计 ====================

#[test]
fn trade_updates_stats_and_twap() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));
        assert_ok!(EntityMarket::take_order(
            RuntimeOrigin::signed(BOB), 0, None
        ));

        let stats = MarketStatsStorage::<Test>::get(ENTITY_ID);
        assert_eq!(stats.total_trades, 1);
        assert!(stats.total_volume_nex > 0);

        // TWAP 累积器应该更新
        let twap = TwapAccumulators::<Test>::get(ENTITY_ID);
        assert!(twap.is_some());
    });
}

// ==================== 审计回归测试 (H1-H6) ====================

#[test]
fn h3_market_sell_slippage_enforced_on_partial_fill() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        // BOB 挂买单: 500 Token @ 100 NEX (仅能提供 50000 NEX)
        assert_ok!(EntityMarket::place_buy_order(
            RuntimeOrigin::signed(BOB), ENTITY_ID, 500, 100
        ));

        // ALICE 想卖 1000 Token, 要求最少收到 80000 NEX
        // 只有 500 Token 能成交, 收到 ~49500 NEX (扣除 1% fee)
        // 49500 < 80000 → 应失败
        assert_noop!(
            EntityMarket::market_sell(RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 80000),
            Error::<Test>::SlippageExceeded
        );
    });
}

#[test]
fn h4_place_order_fails_entity_not_active() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        // 标记实体为非活跃 (模拟 Banned/Closed)
        set_entity_active(ENTITY_ID, false);

        assert_noop!(
            EntityMarket::place_sell_order(RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100),
            Error::<Test>::EntityNotActive
        );
        assert_noop!(
            EntityMarket::place_buy_order(RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn h6_configure_market_rejects_short_ttl() {
    ExtBuilder::build().execute_with(|| {
        // order_ttl = 5 < 10 → 应失败
        assert_noop!(
            EntityMarket::configure_market(
                RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID,
                true, 100, 1, 5,
            ),
            Error::<Test>::OrderTtlTooShort
        );
    });
}

// ==================== EntityTokenPriceProvider 测试 ====================

#[test]
fn token_price_returns_none_when_no_data() {
    use pallet_entity_common::EntityTokenPriceProvider;
    ExtBuilder::build().execute_with(|| {
        assert_eq!(EntityMarket::get_token_price(ENTITY_ID), None);
        assert_eq!(EntityMarket::get_token_price_usdt(ENTITY_ID), None);
        assert_eq!(EntityMarket::token_price_confidence(ENTITY_ID), 0);
        assert!(EntityMarket::is_token_price_stale(ENTITY_ID, 100));
        assert!(!EntityMarket::is_token_price_reliable(ENTITY_ID));
    });
}

#[test]
fn token_price_returns_initial_price() {
    use pallet_entity_common::EntityTokenPriceProvider;
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        // 设置初始价格: 100 NEX per Token
        assert_ok!(EntityMarket::set_initial_price(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID, 100
        ));

        assert_eq!(EntityMarket::get_token_price(ENTITY_ID), Some(100u128));
        // 置信度 = 35 (仅 initial_price，但 TWAP 累积器已初始化且 current_block=1)
        // initial_price → last_trade_price 也被设置了, 所以 has_last_trade = true
        let confidence = EntityMarket::token_price_confidence(ENTITY_ID);
        assert!(confidence >= 35);
        assert!(EntityMarket::is_token_price_reliable(ENTITY_ID));
    });
}

#[test]
fn token_price_usdt_returns_none() {
    use pallet_entity_common::EntityTokenPriceProvider;
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));
        assert_ok!(EntityMarket::take_order(
            RuntimeOrigin::signed(BOB), 0, None
        ));

        // USDT trading channel removed — always returns None
        let usdt_price = EntityMarket::get_token_price_usdt(ENTITY_ID);
        assert_eq!(usdt_price, None);
    });
}

#[test]
fn token_price_staleness_detection() {
    use pallet_entity_common::EntityTokenPriceProvider;
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));
        assert_ok!(EntityMarket::take_order(
            RuntimeOrigin::signed(BOB), 0, None
        ));

        // 当前区块 = 1, TWAP 累积器 current_block = 1
        // max_age = 100 → 不过时
        assert!(!EntityMarket::is_token_price_stale(ENTITY_ID, 100));

        // 推进区块到 200
        System::set_block_number(200);
        // max_age = 100 → 200 - 1 = 199 > 100 → 过时
        assert!(EntityMarket::is_token_price_stale(ENTITY_ID, 100));
        // max_age = 300 → 199 < 300 → 不过时
        assert!(!EntityMarket::is_token_price_stale(ENTITY_ID, 300));
    });
}

#[test]
fn token_price_confidence_levels() {
    use pallet_entity_common::EntityTokenPriceProvider;
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        // 仅 initial_price: 置信度 35 (冷启动)
        assert_ok!(EntityMarket::set_initial_price(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID, 100
        ));
        let c1 = EntityMarket::token_price_confidence(ENTITY_ID);
        // set_initial_price 同时设置 LastTradePrice, 所以 has_last_trade = true → 65
        assert!(c1 >= 35);

        // 有交易后: 置信度应更高
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 500, 100
        ));
        assert_ok!(EntityMarket::take_order(
            RuntimeOrigin::signed(BOB), 0, None
        ));
        let c2 = EntityMarket::token_price_confidence(ENTITY_ID);
        assert!(c2 >= c1);
    });
}

// ==================== Round 3 审计回归测试 ====================

/// C1: deviation_bps u16 截断不应绕过价格保护
#[test]
fn c1_extreme_deviation_not_bypassed_by_u16_wrap() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        // 启用价格保护，设置初始参考价格和较小的 max_deviation
        assert_ok!(EntityMarket::configure_price_protection(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID,
            true,   // enabled
            1000,   // max_deviation = 10%
            500,    // max_slippage
            5000,   // circuit_breaker_threshold
            0,      // min_trades_for_twap = 0 → 使用 initial_price
        ));
        assert_ok!(EntityMarket::set_initial_price(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID, 100
        ));

        // 正常偏离 5% (price=105) → 应通过
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 100, 105
        ));

        // 极端偏离 price = 100_000 (99900% 偏离) → 修复前 as u16 会 wrap
        // 99900 * 10000 / 100 = 9_990_000 bps → 超过 u16::MAX(65535)
        // 修复前: wrap 到 (9_990_000 % 65536) = 某个小值 → 绕过
        // 修复后: .min(u16::MAX) = 65535 > 1000 → PriceDeviationTooHigh
        assert_noop!(
            EntityMarket::place_sell_order(
                RuntimeOrigin::signed(ALICE), ENTITY_ID, 100, 100_000
            ),
            Error::<Test>::PriceDeviationTooHigh
        );
    });
}

/// H2: 过期订单不可吃单
#[test]
fn h2_take_order_rejects_expired_order() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        // ALICE 挂卖单: TTL = 1000
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));
        let order = Orders::<Test>::get(0).unwrap();

        // 推进到过期之后（但 on_idle 未运行，订单仍为 Open）
        System::set_block_number(order.expires_at + 1);

        // BOB 尝试吃单 → 应失败
        assert_noop!(
            EntityMarket::take_order(RuntimeOrigin::signed(BOB), 0, None),
            Error::<Test>::OrderClosed
        );
    });
}

/// H3: 过期订单不出现在 best prices 和 market orders
#[test]
fn h3_expired_orders_excluded_from_best_prices() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        // ALICE 挂卖单
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));
        // BOB 挂买单
        assert_ok!(EntityMarket::place_buy_order(
            RuntimeOrigin::signed(BOB), ENTITY_ID, 500, 80
        ));

        // 验证 best prices 有效
        let (ask, bid) = EntityMarket::get_best_prices(ENTITY_ID);
        assert_eq!(ask, Some(100u128));
        assert_eq!(bid, Some(80u128));

        // 推进到订单过期后（但不运行 on_idle）
        let order = Orders::<Test>::get(0).unwrap();
        System::set_block_number(order.expires_at + 1);

        // best prices 应为 None（过期单被过滤）
        let (ask2, bid2) = EntityMarket::get_best_prices(ENTITY_ID);
        assert_eq!(ask2, None);
        assert_eq!(bid2, None);

        // 市价买单应失败（无有效卖单）
        assert_noop!(
            EntityMarket::market_buy(RuntimeOrigin::signed(CHARLIE), ENTITY_ID, 100, 200_000),
            Error::<Test>::NoOrdersAvailable
        );
    });
}

// ==================== EntityLocked 回归测试 ====================

#[test]
fn entity_locked_rejects_configure_market() {
    ExtBuilder::build().execute_with(|| {
        set_entity_locked(ENTITY_ID);
        assert_noop!(
            EntityMarket::configure_market(
                RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID,
                true, 100, 1, 1000,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

// ==================== 新 extrinsics 测试 ====================

// --- force_cancel_order ---

#[test]
fn force_cancel_order_works() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));

        // Root 强制取消
        assert_ok!(EntityMarket::force_cancel_order(RuntimeOrigin::root(), 0));

        let order = Orders::<Test>::get(0).unwrap();
        assert_eq!(order.status, OrderStatus::Cancelled);
        // Token 退还
        assert_eq!(get_token_reserved(ENTITY_ID, ALICE), 0);
    });
}

#[test]
fn force_cancel_order_fails_not_root() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));

        assert_noop!(
            EntityMarket::force_cancel_order(RuntimeOrigin::signed(ALICE), 0),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// --- pause_market / resume_market ---

#[test]
fn pause_and_resume_market_works() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        // 暂停
        assert_ok!(EntityMarket::pause_market(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID));
        let config = MarketConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert!(config.paused);

        // 暂停后不能下单
        assert_noop!(
            EntityMarket::place_sell_order(RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100),
            Error::<Test>::MarketPaused
        );

        // 恢复
        assert_ok!(EntityMarket::resume_market(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID));
        let config = MarketConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert!(!config.paused);

        // 恢复后可以下单
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));
    });
}

#[test]
fn pause_market_fails_not_owner() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        assert_noop!(
            EntityMarket::pause_market(RuntimeOrigin::signed(ALICE), ENTITY_ID),
            Error::<Test>::NotEntityOwner
        );
    });
}

// --- global_market_pause ---

#[test]
fn global_market_pause_works() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        // Root 全局暂停
        assert_ok!(EntityMarket::global_market_pause(RuntimeOrigin::root(), true));
        assert!(GlobalMarketPaused::<Test>::get());

        // 全局暂停后不能下单
        assert_noop!(
            EntityMarket::place_sell_order(RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100),
            Error::<Test>::GlobalMarketPausedError
        );

        // 解除全局暂停
        assert_ok!(EntityMarket::global_market_pause(RuntimeOrigin::root(), false));
        assert!(!GlobalMarketPaused::<Test>::get());

        // 可以下单
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));
    });
}

#[test]
fn global_market_pause_fails_not_root() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityMarket::global_market_pause(RuntimeOrigin::signed(ALICE), true),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// --- batch_cancel_orders ---

#[test]
fn batch_cancel_orders_works() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 500, 100
        ));
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 500, 200
        ));

        assert_ok!(EntityMarket::batch_cancel_orders(
            RuntimeOrigin::signed(ALICE), vec![0, 1]
        ));

        assert_eq!(Orders::<Test>::get(0).unwrap().status, OrderStatus::Cancelled);
        assert_eq!(Orders::<Test>::get(1).unwrap().status, OrderStatus::Cancelled);
        assert_eq!(get_token_reserved(ENTITY_ID, ALICE), 0);
    });
}

// --- cleanup_expired_orders ---

#[test]
fn cleanup_expired_orders_works() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));

        let order = Orders::<Test>::get(0).unwrap();
        // 推进到过期后
        System::set_block_number(order.expires_at + 1);

        assert_ok!(EntityMarket::cleanup_expired_orders(
            RuntimeOrigin::signed(CHARLIE), ENTITY_ID, 10
        ));

        let order = Orders::<Test>::get(0).unwrap();
        assert_eq!(order.status, OrderStatus::Expired);

        // Token 应已退还给 ALICE
        assert_eq!(get_token_reserved(ENTITY_ID, ALICE), 0);
    });
}

// --- modify_order ---

#[test]
fn modify_order_works() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 2000, 100
        ));

        // 修改价格和减少数量
        assert_ok!(EntityMarket::modify_order(
            RuntimeOrigin::signed(ALICE), 0, 150, 1000
        ));

        let order = Orders::<Test>::get(0).unwrap();
        assert_eq!(order.token_amount, 1000u128);
        assert_eq!(order.price, 150u128);
    });
}

#[test]
fn modify_order_fails_increase_amount() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));

        assert_noop!(
            EntityMarket::modify_order(RuntimeOrigin::signed(ALICE), 0, 100, 2000),
            Error::<Test>::ModifyAmountExceedsOriginal
        );
    });
}

#[test]
fn modify_order_fails_not_owner() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));

        assert_noop!(
            EntityMarket::modify_order(RuntimeOrigin::signed(BOB), 0, 100, 500),
            Error::<Test>::NotOrderOwner
        );
    });
}

// ==================== 审计回归测试 (Round 2) ====================

/// H1: take_order 在市场暂停时应失败
#[test]
fn h1_take_order_rejects_paused_market() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        // ALICE 挂卖单
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));

        // 暂停市场
        assert_ok!(EntityMarket::pause_market(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID));

        // BOB 尝试吃单 → 应失败
        assert_noop!(
            EntityMarket::take_order(RuntimeOrigin::signed(BOB), 0, None),
            Error::<Test>::MarketPaused
        );

        // 恢复后可吃单
        assert_ok!(EntityMarket::resume_market(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID));
        assert_ok!(EntityMarket::take_order(RuntimeOrigin::signed(BOB), 0, None));
    });
}

/// H1: take_order 在实体非活跃时应失败
#[test]
fn h1_take_order_rejects_inactive_entity() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));

        // 标记实体为非活跃
        set_entity_active(ENTITY_ID, false);

        assert_noop!(
            EntityMarket::take_order(RuntimeOrigin::signed(BOB), 0, None),
            Error::<Test>::EntityNotActive
        );
    });
}

/// H3: lift_circuit_breaker 未激活时返回 CircuitBreakerNotActive
#[test]
fn h3_lift_circuit_breaker_correct_error_when_not_active() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityMarket::lift_circuit_breaker(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID),
            Error::<Test>::CircuitBreakerNotActive
        );
    });
}

/// M1: fee_rate = 0 应有效（零手续费交易）
#[test]
fn m1_zero_fee_rate_works() {
    ExtBuilder::build().execute_with(|| {
        // 配置 fee_rate = 0
        assert_ok!(EntityMarket::configure_market(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID,
            true, 0, 1, 1000,
        ));

        // ALICE 挂卖单
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));

        let bob_before = Balances::free_balance(BOB);
        let alice_before = Balances::free_balance(ALICE);

        // BOB 吃单
        assert_ok!(EntityMarket::take_order(RuntimeOrigin::signed(BOB), 0, None));

        // 零手续费: BOB 支付 100_000, ALICE 收到 100_000（无扣除）
        let bob_paid = bob_before - Balances::free_balance(BOB);
        let alice_received = Balances::free_balance(ALICE) - alice_before;
        assert_eq!(bob_paid, 100_000);
        assert_eq!(alice_received, 100_000);
    });
}

/// M2: force_cancel_order 后 best_prices 应更新
#[test]
fn m2_force_cancel_updates_best_prices() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));
        assert_eq!(BestAsk::<Test>::get(ENTITY_ID), Some(100u128));

        // Root 强制取消
        assert_ok!(EntityMarket::force_cancel_order(RuntimeOrigin::root(), 0));

        // best_ask 应清除
        assert_eq!(BestAsk::<Test>::get(ENTITY_ID), None);
    });
}

/// M2: batch_cancel_orders 后 best_prices 应更新
#[test]
fn m2_batch_cancel_updates_best_prices() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 500, 100
        ));
        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 500, 200
        ));
        assert_eq!(BestAsk::<Test>::get(ENTITY_ID), Some(100u128));

        // 取消最优卖单
        assert_ok!(EntityMarket::batch_cancel_orders(
            RuntimeOrigin::signed(ALICE), vec![0]
        ));

        // best_ask 应更新为次优 200
        assert_eq!(BestAsk::<Test>::get(ENTITY_ID), Some(200u128));
    });
}

/// M2: cleanup_expired_orders 后 best_prices 应更新
#[test]
fn m2_cleanup_expired_updates_best_prices() {
    ExtBuilder::build().execute_with(|| {
        configure_market_enabled(ENTITY_ID);

        assert_ok!(EntityMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 1000, 100
        ));
        assert_eq!(BestAsk::<Test>::get(ENTITY_ID), Some(100u128));

        let order = Orders::<Test>::get(0).unwrap();
        System::set_block_number(order.expires_at + 1);

        assert_ok!(EntityMarket::cleanup_expired_orders(
            RuntimeOrigin::signed(CHARLIE), ENTITY_ID, 10
        ));

        // best_ask 应清除
        assert_eq!(BestAsk::<Test>::get(ENTITY_ID), None);
    });
}

// --- nex_enabled 配置测试 ---

#[test]
fn market_paused_field_persists() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityMarket::configure_market(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_ID,
            true, 100, 1, 1000,
        ));
        let config = MarketConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert!(!config.paused); // 默认 false
        assert!(config.nex_enabled);
    });
}
