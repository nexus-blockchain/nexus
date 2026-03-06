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
    fn force_pause_market() -> Weight;
    fn force_resume_market() -> Weight;
    fn force_settle_trade() -> Weight;
    fn force_cancel_trade() -> Weight;
    fn dispute_trade() -> Weight;
    fn resolve_dispute() -> Weight;
    fn set_trading_fee() -> Weight;
    fn update_order_price() -> Weight;
    fn update_deposit_exchange_rate() -> Weight;
    fn seller_confirm_received() -> Weight;
    fn ban_user() -> Weight;
    fn unban_user() -> Weight;
    fn submit_counter_evidence() -> Weight;
    fn update_order_amount() -> Weight;
    fn batch_force_settle() -> Weight;
    fn batch_force_cancel() -> Weight;
}

/// 保守权重估算（未经 benchmark，基于 DB 读写分析）
///
/// 公式: ref_time ≈ 25M * reads + 50M * writes + 10M * reserve_ops
///        proof_size ≈ 3500 + 500 * storage_items_touched
///
/// TODO: 替换为 frame_benchmarking 生成的精确值
impl WeightInfo for () {
    fn place_sell_order() -> Weight { Weight::from_parts(50_000_000, 6_000) }
    fn place_buy_order() -> Weight { Weight::from_parts(50_000_000, 6_000) }
    fn cancel_order() -> Weight { Weight::from_parts(60_000_000, 5_000) }
    fn reserve_sell_order() -> Weight { Weight::from_parts(90_000_000, 8_000) }
    fn accept_buy_order() -> Weight { Weight::from_parts(90_000_000, 8_000) }
    fn confirm_payment() -> Weight { Weight::from_parts(50_000_000, 5_000) }
    fn process_timeout() -> Weight { Weight::from_parts(120_000_000, 10_000) }
    fn submit_ocw_result() -> Weight { Weight::from_parts(100_000_000, 8_000) }
    fn claim_reward() -> Weight { Weight::from_parts(100_000_000, 8_000) }
    fn configure_price_protection() -> Weight { Weight::from_parts(30_000_000, 4_000) }
    fn set_initial_price() -> Weight { Weight::from_parts(40_000_000, 5_000) }
    fn lift_circuit_breaker() -> Weight { Weight::from_parts(30_000_000, 4_000) }
    fn fund_seed_account() -> Weight { Weight::from_parts(50_000_000, 5_000) }
    fn seed_liquidity() -> Weight { Weight::from_parts(200_000_000, 20_000) }
    fn auto_confirm_payment() -> Weight { Weight::from_parts(100_000_000, 8_000) }
    fn submit_underpaid_update() -> Weight { Weight::from_parts(60_000_000, 5_000) }
    fn finalize_underpaid() -> Weight { Weight::from_parts(110_000_000, 8_000) }
    fn force_pause_market() -> Weight { Weight::from_parts(30_000_000, 4_000) }
    fn force_resume_market() -> Weight { Weight::from_parts(30_000_000, 4_000) }
    fn force_settle_trade() -> Weight { Weight::from_parts(120_000_000, 10_000) }
    fn force_cancel_trade() -> Weight { Weight::from_parts(100_000_000, 8_000) }
    fn dispute_trade() -> Weight { Weight::from_parts(50_000_000, 5_000) }
    fn resolve_dispute() -> Weight { Weight::from_parts(80_000_000, 6_000) }
    fn set_trading_fee() -> Weight { Weight::from_parts(30_000_000, 4_000) }
    fn update_order_price() -> Weight { Weight::from_parts(60_000_000, 5_000) }
    fn update_deposit_exchange_rate() -> Weight { Weight::from_parts(30_000_000, 4_000) }
    fn seller_confirm_received() -> Weight { Weight::from_parts(100_000_000, 8_000) }
    fn ban_user() -> Weight { Weight::from_parts(30_000_000, 4_000) }
    fn unban_user() -> Weight { Weight::from_parts(30_000_000, 4_000) }
    fn submit_counter_evidence() -> Weight { Weight::from_parts(50_000_000, 5_000) }
    fn update_order_amount() -> Weight { Weight::from_parts(60_000_000, 5_000) }
    fn batch_force_settle() -> Weight { Weight::from_parts(200_000_000, 20_000) }
    fn batch_force_cancel() -> Weight { Weight::from_parts(200_000_000, 20_000) }
}
