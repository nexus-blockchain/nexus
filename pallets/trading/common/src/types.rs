//! # 公共类型定义
//!
//! 本模块定义 Trading 相关的公共类型，供多个 pallet 共享。
//!
//! ## 版本历史
//! - v0.1.0 (2026-01-18): 初始版本，从 OTC/Swap/Maker 模块提取

use frame_support::{BoundedVec, pallet_prelude::ConstU32};

/// 函数级详细中文注释：TRON 地址类型（固定 34 字节）
///
/// ## 说明
/// - TRC20 地址以 'T' 开头，长度固定为 34 字符
/// - 用于 OTC 订单收款地址和 Swap 兑换地址
///
/// ## 使用者
/// - `pallet-trading-p2p`: 做市商收款地址 / 用户 USDT 接收地址
/// - `pallet-trading-maker`: 做市商注册地址
pub type TronAddress = BoundedVec<u8, ConstU32<34>>;

/// 函数级详细中文注释：时间戳类型（Unix 秒）
///
/// ## 说明
/// - 用于 OTC 订单的时间字段
/// - 精度为秒（非毫秒）
pub type MomentOf = u64;

/// 函数级详细中文注释：IPFS CID 类型（最大 64 字节）
///
/// ## 说明
/// - 用于存储 IPFS 内容标识符
/// - 如做市商的公开/私密资料
pub type Cid = BoundedVec<u8, ConstU32<64>>;

/// 函数级详细中文注释：交易哈希类型（最大 128 字节）
///
/// ## 说明
/// - 用于存储 TRON TRC20 交易哈希
/// - Swap 模块中使用
pub type TxHash = BoundedVec<u8, ConstU32<128>>;

/// TRON 交易哈希类型（64 字节 hex）
pub type TronTxHash = BoundedVec<u8, ConstU32<64>>;

// ==================== USDT 交易共享类型 ====================

/// USDT 交易状态（entity-market / nex-market 共享）
#[derive(
    codec::Encode, codec::Decode, codec::DecodeWithMemTracking,
    Clone, Copy, PartialEq, Eq,
    scale_info::TypeInfo, frame_support::pallet_prelude::MaxEncodedLen,
    sp_runtime::RuntimeDebug,
)]
pub enum UsdtTradeStatus {
    /// 等待买家支付 USDT
    AwaitingPayment,
    /// 等待 OCW 验证
    AwaitingVerification,
    /// 已完成
    Completed,
    /// 争议中
    Disputed,
    /// 已取消
    Cancelled,
    /// 已退款（超时）
    Refunded,
    /// 少付等待补付（补付窗口内）
    UnderpaidPending,
}

/// 买家保证金状态（entity-market / nex-market 共享）
#[derive(
    codec::Encode, codec::Decode, codec::DecodeWithMemTracking,
    Clone, Copy, PartialEq, Eq,
    scale_info::TypeInfo, frame_support::pallet_prelude::MaxEncodedLen,
    sp_runtime::RuntimeDebug, Default,
)]
pub enum BuyerDepositStatus {
    /// 无保证金
    #[default]
    None,
    /// 已锁定
    Locked,
    /// 已退还（交易完成）
    Released,
    /// 已没收（超时/违约）
    Forfeited,
    /// 部分没收（少付场景）
    PartiallyForfeited,
}

/// 付款金额验证结果（多档判定，entity-market / nex-market 共享）
///
/// | 实际金额        | 结果              |
/// |-----------------|-------------------|
/// | ≥ 100.5%        | Overpaid          |
/// | 99.5% ~ 100.5%  | Exact             |
/// | 50% ~ 99.5%     | Underpaid         |
/// | < 50%           | SeverelyUnderpaid |
/// | = 0             | Invalid           |
#[derive(
    codec::Encode, codec::Decode, codec::DecodeWithMemTracking,
    Clone, Copy, PartialEq, Eq,
    scale_info::TypeInfo, frame_support::pallet_prelude::MaxEncodedLen,
    sp_runtime::RuntimeDebug,
)]
pub enum PaymentVerificationResult {
    /// 验证通过（≥99.5%）
    Exact,
    /// 多付（≥100.5%）
    Overpaid,
    /// 少付（50%-99.5%）→ 按比例处理
    Underpaid,
    /// 严重少付（<50%）
    SeverelyUnderpaid,
    /// 无效（0 或交易失败）
    Invalid,
}

/// 计算付款金额验证结果（多档判定）
///
/// 由 entity-market 和 nex-market 共享，避免重复实现。
pub fn calculate_payment_verification_result(
    expected_amount: u64,
    actual_amount: u64,
) -> PaymentVerificationResult {
    if actual_amount == 0 {
        return PaymentVerificationResult::Invalid;
    }
    if expected_amount == 0 {
        return PaymentVerificationResult::Overpaid;
    }
    let ratio = (actual_amount as u128)
        .saturating_mul(10000)
        .saturating_div(expected_amount as u128) as u16;
    match ratio {
        r if r >= 10050 => PaymentVerificationResult::Overpaid,
        r if r >= 9950 => PaymentVerificationResult::Exact,
        r if r >= 5000 => PaymentVerificationResult::Underpaid,
        _ => PaymentVerificationResult::SeverelyUnderpaid,
    }
}

/// 保证金没收梯度计算（entity-market / nex-market 共享）
///
/// | 付款比例     | 没收比例 |
/// |-------------|---------|
/// | ≥ 99.5%     | 0%      |
/// | 95% - 99.5% | 20%     |
/// | 80% - 95%   | 50%     |
/// | < 80%       | 100%    |
pub fn calculate_deposit_forfeit_rate(payment_ratio: u16) -> u16 {
    match payment_ratio {
        r if r >= 9950 => 0,
        r if r >= 9500 => 2000,
        r if r >= 8000 => 5000,
        _ => 10000,
    }
}

