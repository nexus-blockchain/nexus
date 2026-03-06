//! Runtime API 定义：沉淀池奖励详情查询接口
//!
//! 提供以下接口：
//! - `get_pool_reward_member_view`: 会员沉淀池详情（个人状态 + 轮次进度 + 领取历史）
//! - `get_pool_reward_admin_view`: 管理者沉淀池总览（配置 + 统计 + 历史 + 待生效变更）

use codec::{Codec, Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

// ============================================================================
// 共享 DTO
// ============================================================================

/// 等级领取进度
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct LevelProgressInfo<Balance> {
    pub level_id: u8,
    pub ratio_bps: u16,
    pub member_count: u32,
    pub claimed_count: u32,
    pub per_member_reward: Balance,
}

/// 领取记录（区块号统一为 u64）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct ClaimRecordInfo<Balance, TokenBalance> {
    pub round_id: u64,
    pub amount: Balance,
    pub token_amount: TokenBalance,
    pub level_id: u8,
    pub claimed_at: u64,
}

/// 轮次详情
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct RoundDetailInfo<Balance, TokenBalance> {
    pub round_id: u64,
    pub start_block: u64,
    pub end_block: u64,
    pub pool_snapshot: Balance,
    pub token_pool_snapshot: Option<TokenBalance>,
    pub level_snapshots: Vec<LevelProgressInfo<Balance>>,
    pub token_level_snapshots: Option<Vec<LevelProgressInfo<TokenBalance>>>,
}

/// 已完成轮次摘要
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct CompletedRoundInfo<Balance, TokenBalance> {
    pub round_id: u64,
    pub start_block: u64,
    pub end_block: u64,
    pub pool_snapshot: Balance,
    pub token_pool_snapshot: Option<TokenBalance>,
    pub level_snapshots: Vec<LevelProgressInfo<Balance>>,
    pub token_level_snapshots: Option<Vec<LevelProgressInfo<TokenBalance>>>,
}

/// 待生效配置变更
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct PendingConfigInfo {
    pub level_ratios: Vec<(u8, u16)>,
    pub round_duration: u64,
    pub apply_after: u64,
}

// ============================================================================
// 会员视角 DTO
// ============================================================================

/// 会员沉淀池详情视图
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct PoolRewardMemberView<Balance, TokenBalance> {
    pub round_duration: u64,
    pub token_pool_enabled: bool,
    pub level_ratios: Vec<(u8, u16)>,

    pub current_round_id: u64,
    pub round_start_block: u64,
    pub round_end_block: u64,
    pub pool_snapshot: Balance,
    pub token_pool_snapshot: Option<TokenBalance>,

    pub effective_level: u8,
    pub claimable_nex: Balance,
    pub claimable_token: TokenBalance,
    pub already_claimed: bool,
    pub round_expired: bool,
    pub last_claimed_round: u64,

    pub level_progress: Vec<LevelProgressInfo<Balance>>,
    pub token_level_progress: Option<Vec<LevelProgressInfo<TokenBalance>>>,

    pub claim_history: Vec<ClaimRecordInfo<Balance, TokenBalance>>,

    pub is_paused: bool,
    pub has_pending_config: bool,
}

// ============================================================================
// 管理者视角 DTO
// ============================================================================

/// 管理者沉淀池总览视图
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct PoolRewardAdminView<Balance, TokenBalance> {
    pub level_ratios: Vec<(u8, u16)>,
    pub round_duration: u64,
    pub token_pool_enabled: bool,

    pub current_round: Option<RoundDetailInfo<Balance, TokenBalance>>,

    pub total_nex_distributed: Balance,
    pub total_token_distributed: TokenBalance,
    pub total_rounds_completed: u64,
    pub total_claims: u64,

    pub round_history: Vec<CompletedRoundInfo<Balance, TokenBalance>>,

    pub pending_config: Option<PendingConfigInfo>,

    pub is_paused: bool,
    pub is_global_paused: bool,

    pub current_pool_balance: Balance,
    pub current_token_pool_balance: TokenBalance,

    pub token_pool_deficit: TokenBalance,
}

// ============================================================================
// Runtime API 声明
// ============================================================================

sp_api::decl_runtime_apis! {
    /// 沉淀池奖励详情查询 API
    ///
    /// 补充 CommissionDashboardApi 的摘要视图，提供沉淀池专属详情页所需的完整数据。
    pub trait PoolRewardDetailApi<AccountId, Balance, TokenBalance>
    where
        AccountId: Codec,
        Balance: Codec,
        TokenBalance: Codec,
    {
        /// 会员沉淀池详情（个人状态 + 轮次进度 + 领取历史）
        ///
        /// ### 参数
        /// - `entity_id`: 实体 ID
        /// - `account`: 查询的会员账户
        ///
        /// ### 返回
        /// - `Some(PoolRewardMemberView)` — 配置存在且会员有效时
        /// - `None` — 无配置或非会员
        fn get_pool_reward_member_view(
            entity_id: u64,
            account: AccountId,
        ) -> Option<PoolRewardMemberView<Balance, TokenBalance>>;

        /// 管理者沉淀池总览（配置 + 统计 + 历史 + 待生效变更）
        ///
        /// ### 参数
        /// - `entity_id`: 实体 ID
        ///
        /// ### 返回
        /// - `Some(PoolRewardAdminView)` — 配置存在时
        /// - `None` — 无配置
        fn get_pool_reward_admin_view(
            entity_id: u64,
        ) -> Option<PoolRewardAdminView<Balance, TokenBalance>>;
    }
}
