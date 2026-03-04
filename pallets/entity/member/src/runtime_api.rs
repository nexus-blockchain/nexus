//! Runtime API 定义：用于前端查询会员仪表盘及推荐团队信息
//!
//! 提供以下接口：
//! - `get_member_info`: 会员仪表盘数据（等级/消费/推荐/过期/升级历史）
//! - `get_referral_team`: 批量返回直推列表 + 各下线的等级/消费概览（支持 1-2 层深度）

use codec::{Codec, Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

/// 升级记录（Runtime API 返回用，区块号统一为 u64）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct UpgradeRecordInfo {
    /// 触发的规则 ID
    pub rule_id: u32,
    /// 升级前等级
    pub from_level_id: u8,
    /// 升级后等级
    pub to_level_id: u8,
    /// 升级时间（区块号）
    pub upgraded_at: u64,
    /// 过期时间（区块号，None = 永久）
    pub expires_at: Option<u64>,
}

/// 会员仪表盘信息（聚合查询）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct MemberDashboardInfo<AccountId> {
    /// 推荐人
    pub referrer: Option<AccountId>,
    /// 存储中的等级 ID（raw，不考虑过期）
    pub custom_level_id: u8,
    /// 有效等级 ID（考虑过期后的实际等级）
    pub effective_level_id: u8,
    /// 累计消费（USDT 精度 10^6）
    pub total_spent: u64,
    /// 直接推荐人数（含所有来源）
    pub direct_referrals: u32,
    /// 有效直推人数（不含复购赠与）
    pub qualified_referrals: u32,
    /// 间接推荐人数
    pub indirect_referrals: u32,
    /// 团队总人数
    pub team_size: u32,
    /// 订单数量
    pub order_count: u32,
    /// 加入时间（区块号）
    pub joined_at: u64,
    /// 最后活跃时间（区块号）
    pub last_active_at: u64,
    /// 是否被封禁
    pub is_banned: bool,
    /// 封禁时间（区块号，None = 未封禁）
    pub banned_at: Option<u64>,
    /// 等级过期时间（区块号，None = 永久或无规则升级）
    pub level_expires_at: Option<u64>,
    /// 升级历史记录
    pub upgrade_history: Vec<UpgradeRecordInfo>,
}

/// 团队成员概览信息
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct TeamMemberInfo<AccountId> {
    /// 会员账户
    pub account: AccountId,
    /// 自定义等级 ID（有效等级，已考虑过期）
    pub level_id: u8,
    /// 累计消费（USDT 精度 10^6）
    pub total_spent: u64,
    /// 直推人数
    pub direct_referrals: u32,
    /// 团队总人数
    pub team_size: u32,
    /// 加入时间（区块号）
    pub joined_at: u64,
    /// 最后活跃时间（区块号）
    pub last_active_at: u64,
    /// 是否被封禁
    pub is_banned: bool,
    /// 下级列表（depth > 1 时递归填充，否则为空）
    pub children: Vec<TeamMemberInfo<AccountId>>,
}

/// O1: 分页会员列表项
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct PaginatedMemberInfo<AccountId> {
    /// 会员账户
    pub account: AccountId,
    /// 有效等级 ID
    pub level_id: u8,
    /// 累计消费（USDT 精度 10^6）
    pub total_spent: u64,
    /// 直推人数
    pub direct_referrals: u32,
    /// 团队总人数
    pub team_size: u32,
    /// 加入时间（区块号）
    pub joined_at: u64,
    /// 是否被封禁
    pub is_banned: bool,
    /// A2: 封禁原因
    pub ban_reason: Option<Vec<u8>>,
}

/// O1: 分页查询结果
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct PaginatedMembersResult<AccountId> {
    /// 会员列表
    pub members: Vec<PaginatedMemberInfo<AccountId>>,
    /// 总会员数
    pub total: u32,
    /// 是否还有更多数据
    pub has_more: bool,
}

/// Entity 会员总览信息（Owner 视角）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct EntityMemberOverview {
    /// 会员总数
    pub total_members: u32,
    /// 各等级会员分布 (level_id, count)
    pub level_distribution: Vec<(u8, u32)>,
    /// 待审批会员数量
    pub pending_count: u32,
    /// 被封禁会员数量
    pub banned_count: u32,
}

sp_api::decl_runtime_apis! {
    /// 会员 Runtime API
    ///
    /// 提供会员仪表盘查询、推荐团队树查询、Entity 总览查询
    pub trait MemberTeamApi<AccountId>
    where
        AccountId: Codec,
    {
        /// 获取会员仪表盘信息（聚合查询）
        ///
        /// ### 参数
        /// - `entity_id`: 实体 ID
        /// - `account`: 查询的会员账户
        ///
        /// ### 返回
        /// - `Some(MemberDashboardInfo)` — 会员存在时返回聚合信息
        /// - `None` — 会员不存在
        fn get_member_info(entity_id: u64, account: AccountId) -> Option<MemberDashboardInfo<AccountId>>;

        /// 获取指定会员的推荐团队树
        ///
        /// ### 参数
        /// - `entity_id`: 实体 ID
        /// - `account`: 查询的会员账户
        /// - `depth`: 查询深度（1 = 仅直推，2 = 直推 + 二级）
        ///
        /// ### 返回
        /// - 直推成员列表，每个成员包含等级/消费概览及可选的下级列表
        fn get_referral_team(entity_id: u64, account: AccountId, depth: u32) -> Vec<TeamMemberInfo<AccountId>>;

        /// 获取 Entity 会员总览（Owner 视角）
        ///
        /// ### 参数
        /// - `entity_id`: 实体 ID
        ///
        /// ### 返回
        /// - 会员总数、等级分布、待审批数、封禁数
        fn get_entity_member_overview(entity_id: u64) -> EntityMemberOverview;

        /// O1: 分页查询实体会员列表
        ///
        /// ### 参数
        /// - `entity_id`: 实体 ID
        /// - `page_size`: 每页数量（上限 100）
        /// - `page_index`: 页码（0-based）
        ///
        /// ### 返回
        /// - 分页会员列表、总数、是否有更多
        fn get_members_paginated(entity_id: u64, page_size: u32, page_index: u32) -> PaginatedMembersResult<AccountId>;
    }
}
