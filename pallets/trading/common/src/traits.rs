//! # 公共 Trait 定义
//!
//! 本模块定义交易相关的公共接口，供多个 pallet 共享。
//!
//! ## 保留 Trait
//! - `PricingProvider`: NEX/USD 汇率查询（被 arbitration, storage-service 使用）
//! - `DepositCalculator`: 统一保证金计算（被 storage-service 使用）


/// 函数级详细中文注释：定价服务接口
///
/// ## 说明
/// 提供 NEX/USD 实时汇率查询功能
///
/// ## 使用者
/// - `pallet-trading-p2p`: 计算订单金额、上报成交价
/// - `pallet-trading-maker`: 计算押金价值
/// - `pallet-arbitration`: 投诉押金换算
/// - `pallet-storage-service`: 运营者保证金计算
///
/// ## 实现者
/// - `pallet-trading-pricing`: 提供聚合价格
pub trait PricingProvider<Balance> {
    /// 获取 NEX/USD 汇率（精度 10^6）
    ///
    /// ## 返回
    /// - `Some(rate)`: 当前汇率（如 1_000_000 表示 1 NEX = 1 USD）
    /// - `None`: 价格不可用（冷启动期或无数据）
    fn get_cos_to_usd_rate() -> Option<Balance>;
    
    /// 🆕 v0.2.0: 上报 P2P 成交到价格聚合（统一 Buy/Sell 两方向）
    ///
    /// ## 参数
    /// - `timestamp`: 交易时间戳（Unix 毫秒）
    /// - `price_usdt`: USDT 单价（精度 10^6）
    /// - `nex_qty`: NEX 数量（精度 10^12）
    ///
    /// ## 返回
    /// - `Ok(())`: 成功
    /// - `Err`: 失败
    fn report_p2p_trade(timestamp: u64, price_usdt: u64, nex_qty: u128) -> sp_runtime::DispatchResult;

    /// 🔄 向后兼容：原 report_swap_order，转发到 report_p2p_trade
    #[deprecated(note = "use report_p2p_trade instead")]
    fn report_swap_order(timestamp: u64, price_usdt: u64, nex_qty: u128) -> sp_runtime::DispatchResult {
        Self::report_p2p_trade(timestamp, price_usdt, nex_qty)
    }
}

// ===== 默认实现（用于测试和 Mock）=====

/// PricingProvider 的空实现
impl<Balance> PricingProvider<Balance> for () {
    fn get_cos_to_usd_rate() -> Option<Balance> {
        None
    }
    
    fn report_p2p_trade(_timestamp: u64, _price_usdt: u64, _nex_qty: u128) -> sp_runtime::DispatchResult {
        Ok(())
    }
}

// ===== 🆕 v0.5.0: 统一保证金计算接口 =====

/// 函数级详细中文注释：保证金计算接口
///
/// ## 说明
/// 提供统一的 USD 价值动态计算保证金功能
/// 所有需要保证金的模块都应使用此接口
///
/// ## 使用者
/// - `pallet-livestream`: 直播间创建保证金
/// - `pallet-storage-service`: 运营者保证金
/// - `pallet-trading-maker`: 做市商押金
/// - `pallet-trading-p2p`: 交易押金
/// - `pallet-arbitration`: 投诉押金
///
/// ## 实现者
/// - 各模块通过 `DepositCalculatorImpl` 实现
pub trait DepositCalculator<Balance> {
    /// 计算 USD 等值的 NEX 保证金
    ///
    /// ## 参数
    /// - `usd_amount`: USD 金额（精度 10^6，如 5_000_000 = 5 USDT）
    /// - `fallback`: 汇率不可用时的兜底金额（NEX）
    ///
    /// ## 返回
    /// - 计算后的 NEX 金额
    ///
    /// ## 计算公式
    /// ```text
    /// nex_amount = usd_amount * 10^12 / rate
    /// ```
    /// 其中 rate 是 NEX/USD 汇率（精度 10^6）
    fn calculate_deposit(usd_amount: u64, fallback: Balance) -> Balance;
}

/// 基于 PricingProvider 的保证金计算实现
///
/// ## 使用方式
/// ```ignore
/// type DepositCalculator = DepositCalculatorImpl<TradingPricingProvider, Balance>;
/// let deposit = T::DepositCalculator::calculate_deposit(5_000_000, fallback);
/// ```
pub struct DepositCalculatorImpl<P, Balance>(core::marker::PhantomData<(P, Balance)>);

impl<P, Balance> DepositCalculator<Balance> for DepositCalculatorImpl<P, Balance>
where
    P: PricingProvider<Balance>,
    Balance: sp_runtime::traits::AtLeast32BitUnsigned + Copy + TryFrom<u128> + Into<u128>,
{
    fn calculate_deposit(usd_amount: u64, fallback: Balance) -> Balance {
        // 尝试使用实时汇率计算
        if let Some(rate) = P::get_cos_to_usd_rate() {
            if rate > Balance::zero() {
                // 🆕 M7修复: NEX 精度为 10^12（UNIT = 1_000_000_000_000）
                // nex_amount = usd_amount * 10^12 / rate
                // 其中 usd_amount 精度 10^6，rate 精度 10^6
                // 结果精度 10^12（NEX 标准精度）
                let usd_u128 = usd_amount as u128;
                let rate_u128: u128 = rate.into();
                let cos_precision: u128 = 1_000_000_000_000u128; // 10^12
                let nex_amount_u128 = usd_u128.saturating_mul(cos_precision) / rate_u128;
                
                if let Ok(amount) = Balance::try_from(nex_amount_u128) {
                    return amount;
                }
            }
        }
        // 汇率不可用时使用兜底金额
        fallback
    }
}

/// DepositCalculator 的空实现（用于测试）
impl<Balance: Default> DepositCalculator<Balance> for () {
    fn calculate_deposit(_usd_amount: u64, fallback: Balance) -> Balance {
        fallback
    }
}

// ===== 🆕 v0.6.0: TWAP 价格预言机接口 =====

/// TWAP 查询窗口
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TwapWindow {
    /// ~10min 实际窗口
    OneHour,
    /// ~1-2h 实际窗口
    OneDay,
    /// ~24-48h 实际窗口（最抗操纵）
    OneWeek,
}

/// NEX/USDT 链上价格预言机接口
///
/// ## 说明
/// 基于 pallet-nex-market 的 TWAP 累积器提供价格查询。
/// 其他模块通过此 trait 获取真实市场价格（涨跌都反映）。
///
/// ## 使用者
/// - 保证金计算、清算引擎、手续费定价等
///
/// ## 注意
/// - `get_twap` 返回的价格反映真实涨跌，不做方向保护
/// - 低交易量时 TWAP 可能滞后，消费方应检查 `is_price_stale`
pub trait PriceOracle {
    /// 获取指定窗口的 TWAP（精度与 usdt_price 一致，10^6 = 1 USDT）
    fn get_twap(window: TwapWindow) -> Option<u64>;

    /// 获取最新成交价
    fn get_last_trade_price() -> Option<u64>;

    /// 价格数据是否过时（超过 max_age_blocks 个区块未更新）
    fn is_price_stale(max_age_blocks: u32) -> bool;

    /// 获取累计交易数（用于判断数据可信度）
    fn get_trade_count() -> u64;
}

/// PriceOracle 的空实现（用于测试和不需要价格的模块）
impl PriceOracle for () {
    fn get_twap(_window: TwapWindow) -> Option<u64> { None }
    fn get_last_trade_price() -> Option<u64> { None }
    fn is_price_stale(_max_age_blocks: u32) -> bool { true }
    fn get_trade_count() -> u64 { 0 }
}

// ===== 🆕 v0.7.0: 统一兑换比率接口 =====

/// NEX/USDT 统一兑换比率接口
///
/// ## 说明
/// 聚合 TWAP + 陈旧检测 + 置信度评估的统一查询接口。
/// 适用于需要简单获取可靠汇率的模块（佣金换算、直播打赏定价等）。
///
/// ## 与 PricingProvider / PriceOracle 的关系
/// - `PricingProvider`: 底层原始汇率查询（已有多个消费方，保留不变）
/// - `PriceOracle`: 底层 TWAP/成交价/陈旧检测（已有，保留不变）
/// - `ExchangeRateProvider`: 高级封装，内部组合上述接口，提供带置信度的汇率
///
/// ## 置信度等级
/// - 90-100: TWAP 可用且新鲜（高交易量）
/// - 60-89:  LastTradePrice 可用但非 TWAP
/// - 30-59:  仅 initial_price 可用（冷启动期）
/// - 0-29:   价格过时或不可用
pub trait ExchangeRateProvider {
    /// 获取 NEX/USDT 兑换比率（精度 10^6）
    ///
    /// 内部优先级：1h TWAP → LastTradePrice → initial_price
    /// 返回 `None` 表示完全不可用
    fn get_nex_usdt_rate() -> Option<u64>;

    /// 价格置信度 (0-100)
    ///
    /// 基于数据来源、新鲜度和交易量综合评估
    fn price_confidence() -> u8;

    /// 价格是否可信赖（置信度 >= 阈值）
    ///
    /// 默认阈值 30，可覆盖
    fn is_rate_reliable() -> bool {
        Self::price_confidence() >= 30
    }
}

/// ExchangeRateProvider 的空实现
impl ExchangeRateProvider for () {
    fn get_nex_usdt_rate() -> Option<u64> { None }
    fn price_confidence() -> u8 { 0 }
}

// ===== 单元测试 =====

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock PricingProvider: 1 NEX = 0.1 USD (rate = 100_000)
    pub struct MockPricingProvider;

    impl PricingProvider<u128> for MockPricingProvider {
        fn get_cos_to_usd_rate() -> Option<u128> {
            Some(100_000) // 0.1 USD/NEX
        }
        
        fn report_p2p_trade(_timestamp: u64, _price_usdt: u64, _nex_qty: u128) -> sp_runtime::DispatchResult {
            Ok(())
        }
    }

    /// 无价格的 Mock PricingProvider
    pub struct NoPricePricingProvider;

    impl PricingProvider<u128> for NoPricePricingProvider {
        fn get_cos_to_usd_rate() -> Option<u128> {
            None
        }
        
        fn report_p2p_trade(_timestamp: u64, _price_usdt: u64, _nex_qty: u128) -> sp_runtime::DispatchResult {
            Ok(())
        }
    }

    /// 零价格的 Mock PricingProvider
    pub struct ZeroPricePricingProvider;

    impl PricingProvider<u128> for ZeroPricePricingProvider {
        fn get_cos_to_usd_rate() -> Option<u128> {
            Some(0)
        }
        
        fn report_p2p_trade(_timestamp: u64, _price_usdt: u64, _nex_qty: u128) -> sp_runtime::DispatchResult {
            Ok(())
        }
    }

    #[test]
    fn test_deposit_calculator_with_price() {
        type Calculator = DepositCalculatorImpl<MockPricingProvider, u128>;
        
        // 5 USDT = 5_000_000 (精度 10^6)
        // rate = 100_000 (0.1 USD/NEX)
        // 🆕 M7修复: 精度 10^12
        // 预期: 5_000_000 * 10^12 / 100_000 = 50 * 10^12 = 50 NEX
        let usd_amount: u64 = 5_000_000;
        let fallback: u128 = 10_000_000_000_000; // 10 NEX (10^13)
        
        let result = Calculator::calculate_deposit(usd_amount, fallback);
        let expected: u128 = 50_000_000_000_000; // 50 NEX (50 * 10^12)
        assert_eq!(result, expected);
    }

    #[test]
    fn test_deposit_calculator_fallback_no_price() {
        type Calculator = DepositCalculatorImpl<NoPricePricingProvider, u128>;
        
        let usd_amount: u64 = 5_000_000;
        let fallback: u128 = 10_000_000_000_000; // 10 NEX
        
        let result = Calculator::calculate_deposit(usd_amount, fallback);
        assert_eq!(result, fallback);
    }

    #[test]
    fn test_deposit_calculator_fallback_zero_price() {
        type Calculator = DepositCalculatorImpl<ZeroPricePricingProvider, u128>;
        
        let usd_amount: u64 = 5_000_000;
        let fallback: u128 = 10_000_000_000_000; // 10 NEX
        
        let result = Calculator::calculate_deposit(usd_amount, fallback);
        assert_eq!(result, fallback);
    }

    #[test]
    fn test_deposit_calculator_empty_impl() {
        let usd_amount: u64 = 5_000_000;
        let fallback: u128 = 10_000_000_000_000; // 10 NEX
        
        let result = <() as DepositCalculator<u128>>::calculate_deposit(usd_amount, fallback);
        assert_eq!(result, fallback);
    }

    #[test]
    fn test_deposit_calculator_various_amounts() {
        type Calculator = DepositCalculatorImpl<MockPricingProvider, u128>;
        
        // 🆕 M7修复: NEX 精度 10^12
        // 1 USDT -> 10 NEX (rate=0.1 USD/NEX)
        let result_1 = Calculator::calculate_deposit(1_000_000, 0);
        assert_eq!(result_1, 10_000_000_000_000u128); // 10 * 10^12
        
        // 100 USDT -> 1000 NEX
        let result_100 = Calculator::calculate_deposit(100_000_000, 0);
        assert_eq!(result_100, 1_000_000_000_000_000u128); // 1000 * 10^12
        
        // 0.01 USDT -> 0.1 NEX
        let result_001 = Calculator::calculate_deposit(10_000, 0);
        assert_eq!(result_001, 100_000_000_000u128); // 0.1 * 10^12
    }

    #[test]
    fn test_pricing_provider_empty_impl() {
        let rate = <() as PricingProvider<u128>>::get_cos_to_usd_rate();
        assert!(rate.is_none());
        
        let result = <() as PricingProvider<u128>>::report_p2p_trade(0, 0, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_exchange_rate_provider_empty_impl() {
        assert_eq!(<() as ExchangeRateProvider>::get_nex_usdt_rate(), None);
        assert_eq!(<() as ExchangeRateProvider>::price_confidence(), 0);
        assert!(!<() as ExchangeRateProvider>::is_rate_reliable());
    }

    #[test]
    fn test_exchange_rate_provider_reliability_threshold() {
        // 置信度 < 30 → 不可信
        struct LowConfidence;
        impl ExchangeRateProvider for LowConfidence {
            fn get_nex_usdt_rate() -> Option<u64> { Some(500_000) }
            fn price_confidence() -> u8 { 29 }
        }
        assert!(!LowConfidence::is_rate_reliable());

        // 置信度 >= 30 → 可信
        struct MidConfidence;
        impl ExchangeRateProvider for MidConfidence {
            fn get_nex_usdt_rate() -> Option<u64> { Some(500_000) }
            fn price_confidence() -> u8 { 30 }
        }
        assert!(MidConfidence::is_rate_reliable());

        // 高置信度
        struct HighConfidence;
        impl ExchangeRateProvider for HighConfidence {
            fn get_nex_usdt_rate() -> Option<u64> { Some(500_000) }
            fn price_confidence() -> u8 { 95 }
        }
        assert!(HighConfidence::is_rate_reliable());
    }

}
