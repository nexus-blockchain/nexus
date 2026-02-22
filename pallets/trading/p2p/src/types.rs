//! # P2P 交易类型定义
//!
//! 统一 Buy（USDT→NEX，原 OTC）和 Sell（NEX→USDT，原 Swap）两方向的数据结构。
//!
//! ## 设计原则
//! - Buy 和 Sell 使用独立的 Order 结构（流程差异大）
//! - 共享 KYC、归档、统计等基础类型
//! - `TradeDirection` 标记交易方向

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use scale_info::TypeInfo;

// ============================================================================
// 1. 交易方向
// ============================================================================

/// P2P 交易方向
#[derive(Encode, Decode, Clone, Copy, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum TradeDirection {
    /// Buy: USDT → NEX（原 OTC）
    Buy,
    /// Sell: NEX → USDT（原 Swap）
    Sell,
}

// ============================================================================
// 2. Buy-side 类型（原 OTC）
// ============================================================================

/// Buy 订单状态（原 OTC OrderState）
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum BuyOrderState {
    /// 已创建，等待买家付款
    Created,
    /// 买家已标记付款或做市商已确认
    PaidOrCommitted,
    /// NEX 已释放给买家
    Released,
    /// 已退款
    Refunded,
    /// 已取消
    Canceled,
    /// 争议中
    Disputed,
    /// 已关闭
    Closed,
    /// 已过期（超时未支付，自动取消）
    Expired,
}

/// 买家押金状态
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
pub enum DepositStatus {
    /// 无押金（首购/信用免押）
    #[default]
    None,
    /// 押金已锁定
    Locked,
    /// 押金已释放（订单完成）
    Released,
    /// 押金已没收（超时/取消/争议败诉）
    Forfeited,
    /// 押金部分没收（买家主动取消）
    PartiallyForfeited,
}

/// Buy 争议状态
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum BuyDisputeStatus {
    /// 等待做市商响应
    WaitingMakerResponse,
    /// 等待仲裁
    WaitingArbitration,
    /// 买家胜诉
    BuyerWon,
    /// 做市商胜诉
    MakerWon,
    /// 已取消
    Cancelled,
}

// ============================================================================
// 3. Sell-side 类型（原 Swap）
// ============================================================================

/// Sell 订单状态（原 SwapStatus）
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum SellOrderStatus {
    /// 待处理（等待做市商转 USDT）
    Pending,
    /// 等待 OCW 验证 TRC20 交易
    AwaitingVerification,
    /// 已完成
    Completed,
    /// OCW 验证失败
    VerificationFailed,
    /// 用户举报
    UserReported,
    /// 仲裁中
    Arbitrating,
    /// 仲裁通过
    ArbitrationApproved,
    /// 仲裁拒绝
    ArbitrationRejected,
    /// 超时退款
    Refunded,
    /// 严重少付争议（<50%）
    SeverelyDisputed,
}

// ============================================================================
// 4. KYC 类型（共享）
// ============================================================================

/// KYC 配置
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub struct KycConfig<BlockNumber> {
    /// 是否启用 KYC 要求
    pub enabled: bool,
    /// 最低认证等级（0=Unknown, 1=FeePaid, 2=Reasonable, 3=KnownGood）
    pub min_judgment_priority: u8,
    /// 配置生效区块
    pub effective_block: BlockNumber,
    /// 最后更新时间
    pub updated_at: BlockNumber,
}

/// KYC 验证结果
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub enum KycVerificationResult {
    /// 验证通过
    Passed,
    /// 验证失败
    Failed(KycFailureReason),
    /// 豁免
    Exempted,
    /// KYC 未启用，跳过
    Skipped,
}

/// KYC 失败原因
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum KycFailureReason {
    /// 未设置身份信息
    IdentityNotSet,
    /// 没有有效判断
    NoValidJudgement,
    /// 认证等级不足
    InsufficientLevel,
    /// 质量问题
    QualityIssue,
}

impl KycFailureReason {
    pub fn to_code(&self) -> u8 {
        match self {
            KycFailureReason::IdentityNotSet => 0,
            KycFailureReason::NoValidJudgement => 1,
            KycFailureReason::InsufficientLevel => 2,
            KycFailureReason::QualityIssue => 3,
        }
    }
}

// ============================================================================
// 5. 归档类型（共享 L2 格式，Buy/Sell 各有 L1）
// ============================================================================

/// 归档订单 L2（最小格式，~16字节，Buy/Sell 通用）
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
pub struct ArchivedOrderL2 {
    /// 订单ID
    pub id: u64,
    /// 交易方向（0=Buy, 1=Sell）
    pub direction: u8,
    /// 状态编码
    pub status: u8,
    /// 年月 (YYMM格式，如2601表示2026年1月)
    pub year_month: u16,
    /// 金额档位 (0-5)
    pub amount_tier: u8,
    /// 保留标志位
    pub flags: u8,
}

/// P2P 永久统计（合并 OTC + Swap 统计）
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
pub struct P2pPermanentStats {
    /// Buy 总订单数
    pub total_buy_orders: u64,
    /// Buy 已完成
    pub completed_buy_orders: u64,
    /// Buy 已取消
    pub cancelled_buy_orders: u64,
    /// Buy 总交易额（USDT，压缩）
    pub buy_volume: u64,
    /// Sell 总订单数
    pub total_sell_orders: u64,
    /// Sell 已完成
    pub completed_sell_orders: u64,
    /// Sell 已退款
    pub refunded_sell_orders: u64,
    /// Sell 总交易额（USDT）
    pub sell_volume: u64,
}
