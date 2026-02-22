//! P2P 交易模块权重定义

use frame_support::weights::Weight;

/// WeightInfo trait - 后续通过 benchmarking 生成实际权重
pub trait WeightInfo {
    // Buy-side
    fn create_buy_order() -> Weight;
    fn mark_paid() -> Weight;
    fn release_nex() -> Weight;
    fn cancel_buy_order() -> Weight;
    fn dispute_buy_order() -> Weight;
    // Sell-side
    fn create_sell_order() -> Weight;
    fn mark_sell_complete() -> Weight;
    fn report_sell() -> Weight;
    fn confirm_sell_verification() -> Weight;
    // KYC
    fn enable_kyc() -> Weight;
    fn disable_kyc() -> Weight;
}

/// 默认权重实现（占位）
impl WeightInfo for () {
    fn create_buy_order() -> Weight { Weight::from_parts(50_000, 0) }
    fn mark_paid() -> Weight { Weight::from_parts(30_000, 0) }
    fn release_nex() -> Weight { Weight::from_parts(40_000, 0) }
    fn cancel_buy_order() -> Weight { Weight::from_parts(30_000, 0) }
    fn dispute_buy_order() -> Weight { Weight::from_parts(30_000, 0) }
    fn create_sell_order() -> Weight { Weight::from_parts(50_000, 0) }
    fn mark_sell_complete() -> Weight { Weight::from_parts(40_000, 0) }
    fn report_sell() -> Weight { Weight::from_parts(20_000, 0) }
    fn confirm_sell_verification() -> Weight { Weight::from_parts(40_000, 0) }
    fn enable_kyc() -> Weight { Weight::from_parts(10_000, 0) }
    fn disable_kyc() -> Weight { Weight::from_parts(10_000, 0) }
}
