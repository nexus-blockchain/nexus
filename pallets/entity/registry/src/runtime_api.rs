//! Runtime API 数据结构：用于前端查询 Entity 信息
//!
//! 提供以下接口：
//! - `get_entity`: 查询实体详情
//! - `get_entities_by_owner`: 查询用户拥有的实体列表
//! - `get_entity_fund_info`: 查询实体资金信息
//! - `get_verified_entities`: 查询已验证的活跃实体列表

use codec::{Codec, Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use pallet_entity_common::{EntityType, EntityStatus, GovernanceMode};

extern crate alloc;
use alloc::vec::Vec;

/// 实体信息摘要（Runtime API 返回值）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
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
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct EntityFundInfo<Balance> {
    pub balance: Balance,
    pub health: u8,
    pub min_operating: Balance,
    pub warning_threshold: Balance,
}

/// 关闭申请信息（Runtime API 返回值）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
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

        /// 查询实体资金信息
        fn get_entity_fund_info(entity_id: u64) -> Option<EntityFundInfo<Balance>>;

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
