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

sp_api::decl_runtime_apis! {
    /// Entity Registry Runtime API
    ///
    /// 供前端查询实体信息、资金状态、验证列表
    pub trait EntityRegistryApi<AccountId, Balance>
    where
        AccountId: Codec,
        Balance: Codec,
    {
        /// 查询实体详情
        fn get_entity(entity_id: u64) -> Option<EntityInfo<AccountId, Balance>>;

        /// 查询用户拥有的实体 ID 列表
        fn get_entities_by_owner(account: AccountId) -> Vec<u64>;

        /// 查询实体资金信息
        fn get_entity_fund_info(entity_id: u64) -> Option<EntityFundInfo<Balance>>;

        /// 查询已验证的活跃实体列表（带分页）
        fn get_verified_entities(offset: u32, limit: u32) -> Vec<u64>;
    }
}
