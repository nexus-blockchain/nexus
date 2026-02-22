//! # Buyer Credit Module (买家信用模块)
//!
//! ## 函数级详细中文注释：买家信用管理
//!
//! ### 核心功能
//! - 多维度信任评估（资产、账户年龄、活跃度、社交）
//! - 新用户分层冷启动（Premium/Standard/Basic/Restricted）
//! - 信用等级体系（Newbie/Bronze/Silver/Gold/Diamond）
//! - 快速学习机制（前3笔5x权重）
//! - 社交信任网络（邀请人、推荐）

use codec::{Encode, Decode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use sp_runtime::traits::Zero;

// ===== 数据结构 =====

/// 函数级详细中文注释：买家信用等级枚举
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum CreditLevel {
    /// 新手（0-5笔成功订单）
    Newbie = 0,
    /// 铜牌（6-20笔）
    Bronze = 1,
    /// 银牌（21-50笔）
    Silver = 2,
    /// 金牌（51-100笔）
    Gold = 3,
    /// 钻石（101+笔）
    Diamond = 4,
}

impl Default for CreditLevel {
    fn default() -> Self {
        CreditLevel::Newbie
    }
}

impl CreditLevel {
    /// 函数级详细中文注释：根据完成订单数确定等级
    pub fn from_completed_orders(count: u32) -> Self {
        match count {
            0..=5 => CreditLevel::Newbie,
            6..=20 => CreditLevel::Bronze,
            21..=50 => CreditLevel::Silver,
            51..=100 => CreditLevel::Gold,
            _ => CreditLevel::Diamond,
        }
    }

    /// 函数级详细中文注释：获取等级对应的基础限额（USDT）
    pub fn get_base_limits(&self) -> (u64, u64) {
        match self {
            CreditLevel::Newbie => (100, 500),
            CreditLevel::Bronze => (500, 2000),
            CreditLevel::Silver => (2000, 10000),
            CreditLevel::Gold => (10000, 50000),
            CreditLevel::Diamond => (50000, 0), // 0表示无限制
        }
    }
}

/// 函数级详细中文注释：新用户等级枚举
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum NewUserTier {
    /// 优质新用户（风险分0-300）
    Premium,
    /// 标准新用户（风险分301-500）
    Standard,
    /// 基础新用户（风险分501-700）
    Basic,
    /// 受限新用户（风险分701-1000）
    Restricted,
}

impl NewUserTier {
    /// 函数级详细中文注释：根据风险分确定新用户等级
    pub fn from_risk_score(risk_score: u16) -> Self {
        match risk_score {
            0..=300 => NewUserTier::Premium,
            301..=500 => NewUserTier::Standard,
            501..=700 => NewUserTier::Basic,
            _ => NewUserTier::Restricted,
        }
    }

    /// 函数级详细中文注释：获取新用户等级限额（USDT）和冷却期（小时）
    pub fn get_limits(&self) -> (u64, u64, u32) {
        match self {
            NewUserTier::Premium => (5000, 20000, 0),      // 单笔5000U，日限20000U，无冷却
            NewUserTier::Standard => (1000, 5000, 12),     // 单笔1000U，日限5000U，12小时
            NewUserTier::Basic => (500, 2000, 24),         // 单笔500U，日限2000U，24小时
            NewUserTier::Restricted => (100, 500, 48),     // 单笔100U，日限500U，48小时
        }
    }
}

/// 函数级详细中文注释：行为模式枚举
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum BehaviorPattern {
    /// 高质量用户
    HighQuality,
    /// 良好用户
    Good,
    /// 普通用户
    Normal,
    /// 可疑用户
    Suspicious,
    /// 数据不足
    Insufficient,
}

/// 函数级详细中文注释：买家信用记录
#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct CreditScore<T: crate::pallet::Config> {
    /// 当前等级
    pub level: CreditLevel,
    /// 新用户等级（仅前20笔有效）
    pub new_user_tier: Option<NewUserTier>,
    /// 成功完成订单数
    pub completed_orders: u32,
    /// 累计购买金额（NEX）
    pub total_volume: crate::pallet::BalanceOf<T>,
    /// 违约次数（超时未付款）
    pub default_count: u32,
    /// 争议次数
    pub dispute_count: u32,
    /// 上次购买时间
    pub last_purchase_at: frame_system::pallet_prelude::BlockNumberFor<T>,
    /// 风险分（0-1000，越低越可信）
    pub risk_score: u16,
    /// 账户创建时间（首次下单时记录）
    pub account_created_at: frame_system::pallet_prelude::BlockNumberFor<T>,
}

impl<T: crate::pallet::Config> Default for CreditScore<T> {
    fn default() -> Self {
        Self {
            level: CreditLevel::Newbie,
            new_user_tier: None,
            completed_orders: 0,
            total_volume: Zero::zero(),
            default_count: 0,
            dispute_count: 0,
            last_purchase_at: Zero::zero(),
            risk_score: 1000, // 默认最高风险
            account_created_at: Zero::zero(),
        }
    }
}

/// 函数级详细中文注释：订单记录（用于行为分析）
#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct OrderRecord {
    /// 订单金额（USDT，精度6）
    pub amount_usdt: u64,
    /// 付款时间（秒）
    pub payment_time_seconds: u64,
    /// 订单创建时间（区块号）
    pub created_at_block: u32,
}

/// 函数级详细中文注释：推荐关系
#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct Endorsement<T: crate::pallet::Config> {
    /// 推荐人
    pub endorser: T::AccountId,
    /// 推荐时间
    pub endorsed_at: frame_system::pallet_prelude::BlockNumberFor<T>,
    /// 是否仍然有效（被推荐人违约则失效）
    pub is_active: bool,
}

// ===== 辅助函数 =====

/// 函数级详细中文注释：根据订单序号计算权重系数（快速学习）
pub fn get_order_weight(order_index: u32) -> u8 {
    match order_index {
        1..=3 => 50,    // 前3笔：权重 5.0x
        4..=5 => 30,    // 第4-5笔：权重 3.0x
        6..=10 => 20,   // 第6-10笔：权重 2.0x
        11..=20 => 15,  // 第11-20笔：权重 1.5x
        _ => 10,        // 21笔以上：权重 1.0x
    }
}
