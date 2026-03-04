//! Runtime API 定义：用于前端查询可投放的 Campaign 及详情
//!
//! 提供以下接口：
//! - `available_campaigns_for_placement`: 查询指定广告位可投放的 Campaign 列表
//! - `campaign_details`: 查询 Campaign 详情

use codec::{Codec, Decode, Encode};
use pallet_ads_primitives::PlacementId;
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

extern crate alloc;
use alloc::vec::Vec;

/// Campaign 摘要 (列表查询用)
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct CampaignSummary<AccountId, Balance> {
    pub campaign_id: u64,
    pub advertiser: AccountId,
    pub bid_per_mille: Balance,
    pub daily_budget: Balance,
    pub total_budget: Balance,
    pub spent: Balance,
    pub delivery_types: u8,
    pub multiplier_bps: u32,
}

/// Campaign 详情 (单条查询用)
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct CampaignDetail<AccountId, Balance> {
    pub campaign_id: u64,
    pub advertiser: AccountId,
    pub text: Vec<u8>,
    pub url: Vec<u8>,
    pub bid_per_mille: Balance,
    pub daily_budget: Balance,
    pub total_budget: Balance,
    pub spent: Balance,
    pub delivery_types: u8,
    pub status: u8,
    pub review_status: u8,
    pub total_deliveries: u32,
    pub created_at: u64,
    pub expires_at: u64,
    pub multiplier_bps: u32,
    pub targets: Option<Vec<PlacementId>>,
}

sp_api::decl_runtime_apis! {
    /// 广告发现 Runtime API
    ///
    /// 提供广告位可投放 Campaign 查询、Campaign 详情查询
    pub trait AdsDiscoveryApi<AccountId, Balance>
    where
        AccountId: Codec,
        Balance: Codec,
    {
        /// 查询指定广告位可投放的 Campaign 列表
        ///
        /// 返回 Active + Approved 且匹配该广告位的 Campaign 摘要列表
        fn available_campaigns_for_placement(
            placement_id: PlacementId,
            max_results: u32,
        ) -> Vec<CampaignSummary<AccountId, Balance>>;

        /// 查询 Campaign 详情
        fn campaign_details(campaign_id: u64) -> Option<CampaignDetail<AccountId, Balance>>;
    }
}
