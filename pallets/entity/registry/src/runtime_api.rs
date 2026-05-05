//! Runtime API 数据结构：用于前端查询 Entity 信息
//!
//! 提供以下接口：
//! - `get_entity`: 查询实体详情
//! - `get_entities_by_owner`: 查询用户拥有的实体列表
//! - `get_entity_fund_info`: 查询实体资金信息（轻量级）
//! - `get_entity_funds`: 查询实体资金全景（protected 明细 + available + 保护规则）
//! - `get_verified_entities`: 查询已验证的活跃实体列表

use codec::{Codec, Decode, Encode};
use pallet_entity_common::{EntityStatus, EntityType, GovernanceMode};
use scale_info::TypeInfo;
use Debug;

extern crate alloc;
use alloc::vec::Vec;

/// 实体信息摘要（Runtime API 返回值）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
pub struct EntityInfo<AccountId, Balance> {
    pub id: u64,
    pub owner: AccountId,
    pub name: Vec<u8>,
    pub entity_type: EntityType,
    pub status: EntityStatus,
    pub verified: bool,
    pub governance_mode: GovernanceMode,
    pub primary_shop_id: u64,
    pub fund_balance: Balance,
    pub fund_health: u8,
    pub contact_cid: Option<Vec<u8>>,
    pub metadata_uri: Option<Vec<u8>>,
    pub created_at: u64,
}

/// 实体资金信息（Runtime API 返回值）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
pub struct EntityFundInfo<Balance> {
    pub balance: Balance,
    pub health: u8,
    pub min_operating: Balance,
    pub warning_threshold: Balance,
}

/// 关闭申请信息（Runtime API 返回值）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
pub struct CloseRequestInfo<BlockNumber> {
    /// 申请时间（区块号）
    pub request_at: BlockNumber,
    /// 超时区块数
    pub timeout: BlockNumber,
    /// 剩余区块数（0 表示已超时）
    pub remaining_blocks: BlockNumber,
    /// 可执行的区块号
    pub executable_at: BlockNumber,
}

// ============================================================================
// Entity 资金全景（get_entity_funds 返回值）
// ============================================================================

/// 已承诺资金分项明细
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug, Default)]
pub struct ProtectedFundsBreakdown<Balance> {
    /// 待提取佣金总额（NEX）— 会员已赚取但未提现的佣金
    pub pending_commission: Balance,
    /// 购物余额总额（NEX）— 复购产生的可抵扣余额
    pub shopping_balance: Balance,
    /// 未分配佣金池（NEX）— 订单佣金计算后无人领取的沉淀部分
    pub unallocated_pool: Balance,
    /// 待退款总额（NEX）— 退款失败等待重试的金额
    pub pending_refund: Balance,
}

/// 资金保护规则（来自治理配置）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
pub struct FundProtectionRules<Balance> {
    /// 最低金库余额阈值（低于此值发出 TreasuryBelowThreshold 告警，0 = 禁用）
    pub min_treasury_threshold: Balance,
    /// 单笔最大支出限额（超出发出 SingleSpendExceeded 告警，0 = 不限）
    pub max_single_spend: Balance,
    /// 每日最大支出限额（超出发出 DailySpendExceeded 告警，0 = 不限）
    pub max_daily_spend: Balance,
    /// 当日已累计支出
    pub daily_spent: Balance,
    /// 当日剩余可支出（max_daily_spend - daily_spent，max_daily_spend 为 0 时此字段也为 0）
    pub daily_remaining: Balance,
}

/// 资金健康度
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
pub struct FundHealthStatus<Balance> {
    /// 健康等级: 0=Critical, 1=Warning, 2=Healthy
    pub level: u8,
    /// 系统最低运营余额（runtime constant）
    pub min_operating: Balance,
    /// 系统预警阈值（runtime constant）
    pub warning_threshold: Balance,
    /// 金库余额是否低于治理设置的 min_treasury_threshold
    pub below_threshold: bool,
    /// available 是否低于系统 min_operating（紧急）
    pub below_min_operating: bool,
}

/// Entity 资金全景（前端一次 RPC 获取完整资金视图）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
pub struct EntityFundsView<Balance> {
    /// 金库总余额（Entity 派生账户 free_balance）
    pub treasury_balance: Balance,
    /// 已承诺资金总计（不可动用）
    pub protected_total: Balance,
    /// 可动资金 = treasury_balance - protected_total
    pub available: Balance,
    /// Protected 分项明细
    pub protected: ProtectedFundsBreakdown<Balance>,
    /// 资金保护规则（None = 未配置）
    pub protection_config: Option<FundProtectionRules<Balance>>,
    /// 资金健康度
    pub health: FundHealthStatus<Balance>,
}

sp_api::decl_runtime_apis! {
    /// Entity Registry Runtime API
    ///
    /// 供前端查询实体信息、资金状态、验证列表
    pub trait EntityRegistryApi<AccountId, Balance, BlockNumber>
    where
        AccountId: Codec,
        Balance: Codec,
        BlockNumber: Codec,
    {
        /// 查询实体详情
        fn get_entity(entity_id: u64) -> Option<EntityInfo<AccountId, Balance>>;

        /// 查询用户拥有的实体 ID 列表
        fn get_entities_by_owner(account: AccountId) -> Vec<u64>;

        /// 查询实体资金信息（轻量级，向后兼容）
        fn get_entity_fund_info(entity_id: u64) -> Option<EntityFundInfo<Balance>>;

        /// 查询 Entity 资金全景（protected 明细 + available + 保护规则 + 健康度）
        fn get_entity_funds(entity_id: u64) -> Option<EntityFundsView<Balance>>;

        /// 查询已验证的活跃实体列表（带分页）
        fn get_verified_entities(offset: u32, limit: u32) -> Vec<u64>;

        /// 按名称查找实体（normalized 匹配）
        fn get_entity_by_name(name: Vec<u8>) -> Option<u64>;

        /// 查询实体管理员列表（含权限位掩码）
        fn get_entity_admins(entity_id: u64) -> Vec<(AccountId, u32)>;

        /// 查询实体暂停原因
        fn get_entity_suspension_reason(entity_id: u64) -> Option<Vec<u8>>;

        /// 查询实体销售统计 (total_sales, total_orders)
        fn get_entity_sales(entity_id: u64) -> Option<(Balance, u64)>;

        /// 查询实体推荐人
        fn get_entity_referrer(entity_id: u64) -> Option<AccountId>;

        /// 查询推荐人的所有推荐实体
        fn get_referrer_entities(account: AccountId) -> Vec<u64>;

        /// 按状态查询实体列表（分页）
        fn get_entities_by_status(status: EntityStatus, offset: u32, limit: u32) -> Vec<u64>;

        /// 查询账户在实体中的权限（owner 返回 ALL_DEFINED，admin 返回权限位，非成员返回 None）
        fn check_admin_permission(entity_id: u64, account: AccountId) -> Option<u32>;

        /// 查询关闭申请信息（PendingClose 状态下的超时进度）
        fn get_close_request_info(entity_id: u64) -> Option<CloseRequestInfo<BlockNumber>>;
    }
}
