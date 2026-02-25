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
}

/// 占位权重实现
impl WeightInfo for () {
    fn place_sell_order() -> Weight { Weight::from_parts(55_000, 6_000) }
    fn place_buy_order() -> Weight { Weight::from_parts(45_000, 5_000) }
    fn cancel_order() -> Weight { Weight::from_parts(40_000, 5_000) }
    fn reserve_sell_order() -> Weight { Weight::from_parts(70_000, 8_000) }
    fn accept_buy_order() -> Weight { Weight::from_parts(75_000, 8_000) }
    fn confirm_payment() -> Weight { Weight::from_parts(35_000, 5_000) }
    fn process_timeout() -> Weight { Weight::from_parts(55_000, 7_000) }
    fn submit_ocw_result() -> Weight { Weight::from_parts(30_000, 4_000) }
    fn claim_reward() -> Weight { Weight::from_parts(80_000, 10_000) }
    fn configure_price_protection() -> Weight { Weight::from_parts(25_000, 3_000) }
    fn set_initial_price() -> Weight { Weight::from_parts(30_000, 4_000) }
    fn lift_circuit_breaker() -> Weight { Weight::from_parts(20_000, 3_000) }
    fn fund_seed_account() -> Weight { Weight::from_parts(50_000, 6_000) }
    fn seed_liquidity() -> Weight { Weight::from_parts(200_000, 20_000) }
}
