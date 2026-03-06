//! Runtime API 定义：Commission 聚合查询接口
//!
//! 提供以下接口：
//! - `get_member_commission_dashboard`: 会员佣金仪表盘（聚合所有子模块数据）
//! - `get_direct_referral_info`: 直推返佣信息
//! - `get_team_performance_info`: 团队业绩信息
//! - `get_entity_commission_overview`: Entity 佣金总览（Owner 视角）

use alloc::vec::Vec;
use codec::{Codec, Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

pub use pallet_commission_common::{MultiLevelActivationInfo, MultiLevelMemberStats, TeamTierInfo};

// ============================================================================
// 会员佣金仪表盘（聚合查询结构体）
// ============================================================================

/// NEX 佣金统计
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, Default)]
pub struct NexCommissionStats<Balance> {
    pub total_earned: Balance,
    pub pending: Balance,
    pub withdrawn: Balance,
    pub repurchased: Balance,
    pub order_count: u32,
}

/// Token 佣金统计
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, Default)]
pub struct TokenCommissionStats<TokenBalance> {
    pub total_earned: TokenBalance,
    pub pending: TokenBalance,
    pub withdrawn: TokenBalance,
    pub repurchased: TokenBalance,
    pub order_count: u32,
}

/// 单线收益快照
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct SingleLineSnapshot {
    pub position: Option<u32>,
    pub upline_levels: Option<u8>,
    pub downline_levels: Option<u8>,
    pub is_enabled: bool,
    pub queue_length: u32,
}

/// 沉淀池快照
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct PoolRewardSnapshot<Balance, TokenBalance> {
    pub claimable_nex: Balance,
    pub claimable_token: TokenBalance,
    pub is_paused: bool,
    pub current_round_id: u64,
}

/// 推荐返佣快照
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct ReferralSnapshot<Balance> {
    pub total_earned: Balance,
    pub cap_max_per_order: Option<Balance>,
    pub cap_max_total: Option<Balance>,
}

/// 会员佣金仪表盘（聚合所有子模块数据）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct MemberCommissionDashboard<Balance, TokenBalance> {
    pub nex_stats: NexCommissionStats<Balance>,
    pub token_stats: TokenCommissionStats<TokenBalance>,
    pub nex_shopping_balance: Balance,
    pub token_shopping_balance: TokenBalance,

    pub multi_level_progress: Vec<MultiLevelActivationInfo>,
    pub multi_level_stats: Option<MultiLevelMemberStats>,

    pub team_tier: Option<TeamTierInfo<Balance>>,

    pub single_line: SingleLineSnapshot,

    pub pool_reward: PoolRewardSnapshot<Balance, TokenBalance>,

    pub referral: ReferralSnapshot<Balance>,
}

// ============================================================================
// 直推返佣信息
// ============================================================================

/// 直推返佣信息
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct DirectReferralInfo<Balance> {
    pub referral_total_earned: Balance,
    pub cap_max_per_order: Option<Balance>,
    pub cap_max_total: Option<Balance>,
    pub cap_remaining: Option<Balance>,
}

// ============================================================================
// 团队业绩信息
// ============================================================================

/// 团队业绩信息
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct TeamPerformanceInfo<Balance> {
    pub team_size: u32,
    pub direct_referrals: u32,
    pub total_spent: u128,
    pub current_tier: Option<TeamTierInfo<Balance>>,
    pub is_enabled: bool,
    pub config_exists: bool,
}

// ============================================================================
// Entity 佣金总览
// ============================================================================

/// Entity 佣金总览（Owner 视角）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct EntityCommissionOverview<Balance, TokenBalance> {
    pub enabled_modes: u16,
    pub commission_rate: u16,
    pub is_enabled: bool,
    pub pending_total_nex: Balance,
    pub pending_total_token: TokenBalance,
    pub unallocated_pool_nex: Balance,
    pub unallocated_pool_token: TokenBalance,
    pub shopping_total_nex: Balance,
    pub shopping_total_token: TokenBalance,
    pub multi_level_paused: bool,
    pub single_line_enabled: bool,
    pub team_status: (bool, bool),
    pub pool_reward_paused: bool,
    pub withdrawal_paused: bool,
}

// ============================================================================
// 直推会员详情
// ============================================================================

/// 单个直推会员详情
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct DirectReferralMember<AccountId, Balance> {
    pub account: AccountId,
    /// 有效等级 ID
    pub level_id: u8,
    /// 累计消费（USDT 精度 10^6）
    pub total_spent: u64,
    /// 订单数
    pub order_count: u32,
    /// 加入时间（区块号）
    pub joined_at: u64,
    /// 最后活跃时间（区块号）
    pub last_active_at: u64,
    /// 是否活跃（非封禁且已激活）
    pub is_active: bool,
    /// 团队总人数
    pub team_size: u32,
    /// 该直推的直推人数
    pub direct_referrals: u32,
    /// 该直推为推荐人贡献的佣金总额（仅推荐链类型）
    pub commission_contributed: Balance,
}

/// 直推会员详情列表
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct DirectReferralDetails<AccountId, Balance> {
    /// 直推会员列表
    pub referrals: Vec<DirectReferralMember<AccountId, Balance>>,
    /// 直推总数
    pub total_count: u32,
    /// 所有直推贡献佣金总额
    pub total_commission_earned: Balance,
    /// 推荐返佣累计上限（None = 无上限）
    pub cap_max_total: Option<Balance>,
    /// 剩余可获佣额度（None = 无上限）
    pub cap_remaining: Option<Balance>,
}

// ============================================================================
// Runtime API 声明
// ============================================================================

sp_api::decl_runtime_apis! {
    /// Commission 聚合查询 API
    ///
    /// 聚合 core + multi-level + team + single-line + pool-reward + referral 数据，
    /// 前端一次 RPC 获取完整佣金视图。
    pub trait CommissionDashboardApi<AccountId, Balance, TokenBalance>
    where
        AccountId: Codec,
        Balance: Codec,
        TokenBalance: Codec,
    {
        /// 会员佣金仪表盘（聚合所有子模块数据）
        fn get_member_commission_dashboard(
            entity_id: u64,
            account: AccountId,
        ) -> Option<MemberCommissionDashboard<Balance, TokenBalance>>;

        /// 直推返佣信息
        fn get_direct_referral_info(
            entity_id: u64,
            account: AccountId,
        ) -> DirectReferralInfo<Balance>;

        /// 团队业绩信息
        fn get_team_performance_info(
            entity_id: u64,
            account: AccountId,
        ) -> TeamPerformanceInfo<Balance>;

        /// Entity 佣金总览（Owner 视角）
        fn get_entity_commission_overview(
            entity_id: u64,
        ) -> EntityCommissionOverview<Balance, TokenBalance>;

        /// 直推会员详情（含每个直推的佣金贡献）
        fn get_direct_referral_details(
            entity_id: u64,
            account: AccountId,
        ) -> DirectReferralDetails<AccountId, Balance>;
    }
}
