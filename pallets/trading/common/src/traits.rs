//! # 公共 Trait 定义
//!
//! 本模块定义 Trading 相关的公共接口，供多个 pallet 共享。
//!
//! ## 版本历史
//! - v0.1.0 (2026-01-18): 初始版本，从 OTC/Swap/Maker 模块提取
//! - v0.2.0 (2026-02-08): PricingProvider 适配 P2P 统一模型，report_swap_order → report_p2p_trade

use sp_runtime::{DispatchResult, DispatchError};
use crate::types::MakerApplicationInfo;

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

/// 函数级详细中文注释：Maker Pallet 接口
///
/// ## 说明
/// 提供做市商信息查询功能
///
/// ## 使用者
/// - `pallet-trading-p2p`: 验证做市商和获取收款地址
///
/// ## 实现者
/// - `pallet-trading-maker`: 提供做市商管理
pub trait MakerInterface<AccountId, Balance> {
    /// 查询做市商申请信息
    ///
    /// ## 参数
    /// - `maker_id`: 做市商ID
    ///
    /// ## 返回
    /// - `Some(info)`: 做市商信息
    /// - `None`: 做市商不存在
    fn get_maker_application(maker_id: u64) -> Option<MakerApplicationInfo<AccountId, Balance>>;
    
    /// 检查做市商是否激活
    ///
    /// ## 参数
    /// - `maker_id`: 做市商ID
    ///
    /// ## 返回
    /// - `true`: 激活状态
    /// - `false`: 未激活或不存在
    fn is_maker_active(maker_id: u64) -> bool;
    
    /// 获取做市商 ID（通过账户）
    ///
    /// ## 参数
    /// - `who`: 账户地址
    ///
    /// ## 返回
    /// - `Some(maker_id)`: 做市商ID
    /// - `None`: 该账户不是做市商
    fn get_maker_id(who: &AccountId) -> Option<u64>;
    
    /// 获取做市商押金的 USD 价值（精度 10^6）
    ///
    /// ## 参数
    /// - `maker_id`: 做市商ID
    ///
    /// ## 返回
    /// - `Ok(usd_value)`: 押金USD价值
    /// - `Err(...)`: 做市商不存在或查询失败
    fn get_deposit_usd_value(maker_id: u64) -> Result<u64, DispatchError>;

    /// 🆕 验证做市商并返回信息（组合验证）
    ///
    /// ## 说明
    /// 统一的做市商验证逻辑，检查做市商存在且激活
    ///
    /// ## 参数
    /// - `maker_id`: 做市商ID
    ///
    /// ## 返回
    /// - `Ok(info)`: 做市商信息
    /// - `Err(MakerNotFound)`: 做市商不存在
    /// - `Err(MakerNotActive)`: 做市商未激活
    fn validate_maker(maker_id: u64) -> Result<MakerApplicationInfo<AccountId, Balance>, MakerValidationError> {
        let info = Self::get_maker_application(maker_id)
            .ok_or(MakerValidationError::NotFound)?;
        if !info.is_active {
            return Err(MakerValidationError::NotActive);
        }
        Ok(info)
    }

    /// 🆕 罚没做市商保证金（Swap 严重少付场景）
    ///
    /// ## 说明
    /// 当做市商在 NEX → USDT 兑换中严重少付（< 50%）时，
    /// 从其保证金中扣除 10% 作为惩罚，转入国库。
    ///
    /// ## 参数
    /// - `maker_id`: 做市商ID
    /// - `swap_id`: 兑换订单ID
    /// - `expected_usdt`: 预期 USDT 金额（精度 10^6）
    /// - `actual_usdt`: 实际 USDT 金额（精度 10^6）
    /// - `penalty_rate_bps`: 罚金比例（基点，如 1000 = 10%）
    ///
    /// ## 返回
    /// - `Ok(penalty_id)`: 惩罚记录ID
    /// - `Err(...)`: 失败原因
    fn slash_deposit_for_severely_underpaid(
        maker_id: u64,
        swap_id: u64,
        expected_usdt: u64,
        actual_usdt: u64,
        penalty_rate_bps: u32,
    ) -> Result<u64, DispatchError>;
}

/// 🆕 做市商验证错误类型
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MakerValidationError {
    /// 做市商不存在
    NotFound,
    /// 做市商未激活
    NotActive,
}

/// 函数级详细中文注释：做市商信用接口
///
/// ## 说明
/// 提供做市商信用分管理功能
///
/// ## 使用者
/// - `pallet-trading-p2p`: 订单完成/超时/争议时调用
///
/// ## 实现者
/// - `pallet-trading-credit`: 提供信用分管理
pub trait MakerCreditInterface {
    /// 记录做市商订单完成（提升信用分）
    ///
    /// ## 参数
    /// - `maker_id`: 做市商ID
    /// - `order_id`: 订单ID
    /// - `response_time_seconds`: 响应时间（秒）
    fn record_maker_order_completed(
        maker_id: u64,
        order_id: u64,
        response_time_seconds: u32,
    ) -> DispatchResult;
    
    /// 记录做市商订单超时（降低信用分）
    ///
    /// ## 参数
    /// - `maker_id`: 做市商ID
    /// - `order_id`: 订单ID
    fn record_maker_order_timeout(
        maker_id: u64,
        order_id: u64,
    ) -> DispatchResult;
    
    /// 记录做市商争议结果
    ///
    /// ## 参数
    /// - `maker_id`: 做市商ID
    /// - `order_id`: 订单ID
    /// - `maker_win`: true = 做市商胜诉
    fn record_maker_dispute_result(
        maker_id: u64,
        order_id: u64,
        maker_win: bool,
    ) -> DispatchResult;
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

/// MakerInterface 的空实现
impl<AccountId, Balance> MakerInterface<AccountId, Balance> for () {
    fn get_maker_application(_maker_id: u64) -> Option<MakerApplicationInfo<AccountId, Balance>> {
        None
    }
    
    fn is_maker_active(_maker_id: u64) -> bool {
        false
    }
    
    fn get_maker_id(_who: &AccountId) -> Option<u64> {
        None
    }
    
    fn get_deposit_usd_value(_maker_id: u64) -> Result<u64, DispatchError> {
        Err(sp_runtime::DispatchError::Other("NotImplemented"))
    }

    fn slash_deposit_for_severely_underpaid(
        _maker_id: u64,
        _swap_id: u64,
        _expected_usdt: u64,
        _actual_usdt: u64,
        _penalty_rate_bps: u32,
    ) -> Result<u64, DispatchError> {
        Err(sp_runtime::DispatchError::Other("NotImplemented"))
    }
}

/// MakerCreditInterface 的空实现
impl MakerCreditInterface for () {
    fn record_maker_order_completed(
        _maker_id: u64,
        _order_id: u64,
        _response_time_seconds: u32,
    ) -> DispatchResult {
        Ok(())
    }
    
    fn record_maker_order_timeout(
        _maker_id: u64,
        _order_id: u64,
    ) -> DispatchResult {
        Ok(())
    }
    
    fn record_maker_dispute_result(
        _maker_id: u64,
        _order_id: u64,
        _maker_win: bool,
    ) -> DispatchResult {
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
    /// nex_amount = usd_amount * 10^18 / rate
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
                // nex_amount = usd_amount * 10^18 / rate
                // 其中 usd_amount 精度 10^6，rate 精度 10^6
                // 结果精度 10^18（NEX 标准精度）
                let usd_u128 = usd_amount as u128;
                let rate_u128: u128 = rate.into();
                let cos_precision: u128 = 1_000_000_000_000_000_000u128; // 10^18
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
        // 预期: 5_000_000 * 10^18 / 100_000 = 50 * 10^18 = 50 NEX
        let usd_amount: u64 = 5_000_000;
        let fallback: u128 = 10_000_000_000_000_000_000; // 10 NEX
        
        let result = Calculator::calculate_deposit(usd_amount, fallback);
        let expected: u128 = 50_000_000_000_000_000_000; // 50 NEX
        assert_eq!(result, expected);
    }

    #[test]
    fn test_deposit_calculator_fallback_no_price() {
        type Calculator = DepositCalculatorImpl<NoPricePricingProvider, u128>;
        
        let usd_amount: u64 = 5_000_000;
        let fallback: u128 = 10_000_000_000_000_000_000;
        
        let result = Calculator::calculate_deposit(usd_amount, fallback);
        assert_eq!(result, fallback);
    }

    #[test]
    fn test_deposit_calculator_fallback_zero_price() {
        type Calculator = DepositCalculatorImpl<ZeroPricePricingProvider, u128>;
        
        let usd_amount: u64 = 5_000_000;
        let fallback: u128 = 10_000_000_000_000_000_000;
        
        let result = Calculator::calculate_deposit(usd_amount, fallback);
        assert_eq!(result, fallback);
    }

    #[test]
    fn test_deposit_calculator_empty_impl() {
        let usd_amount: u64 = 5_000_000;
        let fallback: u128 = 10_000_000_000_000_000_000;
        
        let result = <() as DepositCalculator<u128>>::calculate_deposit(usd_amount, fallback);
        assert_eq!(result, fallback);
    }

    #[test]
    fn test_deposit_calculator_various_amounts() {
        type Calculator = DepositCalculatorImpl<MockPricingProvider, u128>;
        
        // 1 USDT -> 10 NEX
        let result_1 = Calculator::calculate_deposit(1_000_000, 0);
        assert_eq!(result_1, 10_000_000_000_000_000_000u128);
        
        // 100 USDT -> 1000 NEX
        let result_100 = Calculator::calculate_deposit(100_000_000, 0);
        assert_eq!(result_100, 1_000_000_000_000_000_000_000u128);
        
        // 0.01 USDT -> 0.1 NEX
        let result_001 = Calculator::calculate_deposit(10_000, 0);
        assert_eq!(result_001, 100_000_000_000_000_000u128);
    }

    #[test]
    fn test_pricing_provider_empty_impl() {
        let rate = <() as PricingProvider<u128>>::get_cos_to_usd_rate();
        assert!(rate.is_none());
        
        let result = <() as PricingProvider<u128>>::report_p2p_trade(0, 0, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_maker_credit_interface_empty_impl() {
        let result1 = <() as MakerCreditInterface>::record_maker_order_completed(1, 1, 100);
        assert!(result1.is_ok());
        
        let result2 = <() as MakerCreditInterface>::record_maker_order_timeout(1, 1);
        assert!(result2.is_ok());
        
        let result3 = <() as MakerCreditInterface>::record_maker_dispute_result(1, 1, true);
        assert!(result3.is_ok());
    }

    #[test]
    fn test_maker_validation_error() {
        let not_found = MakerValidationError::NotFound;
        let not_active = MakerValidationError::NotActive;
        
        assert_ne!(not_found, not_active);
        assert_eq!(not_found, MakerValidationError::NotFound);
    }
}
