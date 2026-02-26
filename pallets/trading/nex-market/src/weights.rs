use frame_support::weights::Weight;

pub trait WeightInfo {
    fn place_sell_order() -> Weight;
    fn place_buy_order() -> Weight;
    fn cancel_order() -> Weight;
    fn reserve_sell_order() -> Weight;
    fn accept_buy_order() -> Weight;
    fn confirm_payment() -> Weight;
    fn process_timeout() -> Weight;
    fn submit_ocw_result() -> Weight;
    fn claim_reward() -> Weight;
    fn configure_price_protection() -> Weight;
    fn set_initial_price() -> Weight;
    fn lift_circuit_breaker() -> Weight;
    fn fund_seed_account() -> Weight;
    fn seed_liquidity() -> Weight;
    fn auto_confirm_payment() -> Weight;
    fn submit_underpaid_update() -> Weight;
    fn finalize_underpaid() -> Weight;
}

/// 保守权重估算（未经 benchmark，基于 DB 读写分析）
///
/// 公式: ref_time ≈ 25M * reads + 50M * writes + 10M * reserve_ops
///        proof_size ≈ 3500 + 500 * storage_items_touched
///
/// TODO: 替换为 frame_benchmarking 生成的精确值
impl WeightInfo for () {
    // 0: place_sell_order — R:3(PriceProtection+TWAP+OrderId) W:4(Order+SellOrders+UserOrders+Stats) + reserve + update_best_prices(R:2000)
    fn place_sell_order() -> Weight { Weight::from_parts(80_000_000, 6_000) }
    // 1: place_buy_order — R:3 W:4 + reserve + deposit_calc + update_best_prices
    fn place_buy_order() -> Weight { Weight::from_parts(80_000_000, 6_000) }
    // 2: cancel_order — R:1(Order) W:3(Order+OrderBook+UserOrders) + unreserve + update_best_prices
    fn cancel_order() -> Weight { Weight::from_parts(60_000_000, 5_000) }
    // 3: reserve_sell_order — R:4(Order+PriceProtection+TWAP+CompletedBuyers) W:5(Trade+Order+OrderBook+UserOrders+AwaitingPayment) + reserve
    fn reserve_sell_order() -> Weight { Weight::from_parts(90_000_000, 8_000) }
    // 4: accept_buy_order — R:3 W:5 + reserve
    fn accept_buy_order() -> Weight { Weight::from_parts(90_000_000, 8_000) }
    // 5: confirm_payment — R:2(Trade+AwaitingPayment) W:3(Trade+AwaitingPayment+PendingUsdt)
    fn confirm_payment() -> Weight { Weight::from_parts(50_000_000, 5_000) }
    // 6: process_timeout — R:5(Trade+OcwResult+Order+queues) W:8(Trade+Order+queues+forfeit+unreserve) worst case
    fn process_timeout() -> Weight { Weight::from_parts(120_000_000, 10_000) }
    // 7: submit_ocw_result — R:2(Trade+queue) W:3(Trade+OcwResult+queue)
    fn submit_ocw_result() -> Weight { Weight::from_parts(60_000_000, 5_000) }
    // 8: claim_reward — R:3(Trade+OcwResult+Order) W:6(Trade+queues+transfer+Stats+TWAP)
    fn claim_reward() -> Weight { Weight::from_parts(100_000_000, 8_000) }
    // 9: configure_price_protection — R:1 W:1
    fn configure_price_protection() -> Weight { Weight::from_parts(30_000_000, 4_000) }
    // 10: set_initial_price — R:2(PriceProtection+TWAP) W:3(PriceProtection+TWAP+LastTradePrice)
    fn set_initial_price() -> Weight { Weight::from_parts(40_000_000, 5_000) }
    // 11: lift_circuit_breaker — R:1 W:1
    fn lift_circuit_breaker() -> Weight { Weight::from_parts(30_000_000, 4_000) }
    // 13: fund_seed_account — R:2 W:2 + transfer
    fn fund_seed_account() -> Weight { Weight::from_parts(50_000_000, 5_000) }
    // 14: seed_liquidity — R:3+N*2 W:3+N*4 (N=order_count, max 10)
    fn seed_liquidity() -> Weight { Weight::from_parts(200_000_000, 20_000) }
    // 15: auto_confirm_payment — R:2 W:4 (similar to confirm+submit combined)
    fn auto_confirm_payment() -> Weight { Weight::from_parts(70_000_000, 6_000) }
    // 16: submit_underpaid_update — R:2(Trade+OcwResult) W:3(OcwResult+Trade+queues)
    fn submit_underpaid_update() -> Weight { Weight::from_parts(60_000_000, 5_000) }
    // 17: finalize_underpaid — R:4 W:7 (similar to claim_reward + deposit forfeit)
    fn finalize_underpaid() -> Weight { Weight::from_parts(110_000_000, 8_000) }
}
