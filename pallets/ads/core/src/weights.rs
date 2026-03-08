//! 权重定义模块
//!
//! 定义 `WeightInfo` trait 和默认 `SubstrateWeight` 实现。
//! 生产环境应通过 `frame_benchmarking` 生成真实权重替换默认值。
//!
//! 运行 benchmark 命令：
//! ```bash
//! cargo run --release --features runtime-benchmarks -- benchmark pallet \
//!   --chain dev \
//!   --pallet pallet_ads_core \
//!   --extrinsic '*' \
//!   --steps 50 \
//!   --repeat 20 \
//!   --output pallets/ads/core/src/weights.rs \
//!   --template .maintain/frame-weight-template.hbs
//! ```

use frame_support::weights::Weight;

pub trait WeightInfo {
    fn create_campaign() -> Weight;
    fn fund_campaign() -> Weight;
    fn pause_campaign() -> Weight;
    fn cancel_campaign() -> Weight;
    fn review_campaign() -> Weight;
    fn submit_delivery_receipt() -> Weight;
    fn settle_era_ads() -> Weight;
    fn flag_campaign() -> Weight;
    fn claim_ad_revenue() -> Weight;
    fn advertiser_block_placement() -> Weight;
    fn advertiser_unblock_placement() -> Weight;
    fn advertiser_prefer_placement() -> Weight;
    fn advertiser_unprefer_placement() -> Weight;
    fn placement_block_advertiser() -> Weight;
    fn placement_unblock_advertiser() -> Weight;
    fn placement_prefer_advertiser() -> Weight;
    fn placement_unprefer_advertiser() -> Weight;
    fn flag_placement() -> Weight;
    fn slash_placement() -> Weight;
    fn register_private_ad() -> Weight;
    fn resume_campaign() -> Weight;
    fn expire_campaign() -> Weight;
    fn update_campaign() -> Weight;
    fn extend_campaign_expiry() -> Weight;
    fn force_cancel_campaign() -> Weight;
    fn unban_placement() -> Weight;
    fn reset_slash_count() -> Weight;
    fn clear_placement_flags() -> Weight;
    fn suspend_campaign() -> Weight;
    fn unsuspend_campaign() -> Weight;
    fn report_approved_campaign() -> Weight;
    fn resubmit_campaign() -> Weight;
    fn set_placement_delivery_types() -> Weight;
    fn unregister_private_ad() -> Weight;
    fn cleanup_campaign() -> Weight;
    fn force_settle_era_ads() -> Weight;
    fn set_campaign_targets() -> Weight;
    fn clear_campaign_targets() -> Weight;
    fn set_campaign_multiplier() -> Weight;
    fn set_placement_multiplier() -> Weight;
    fn set_placement_approval_required() -> Weight;
    fn approve_campaign_for_placement() -> Weight;
    fn reject_campaign_for_placement() -> Weight;
    fn confirm_receipt() -> Weight;
    fn dispute_receipt() -> Weight;
    fn auto_confirm_receipt() -> Weight;
    fn register_advertiser() -> Weight;
    fn force_register_advertiser() -> Weight;
    fn claim_referral_earnings() -> Weight;
    fn submit_click_receipt() -> Weight;
}

/// 默认权重实现（保守估计，生产前需 benchmark 替换）
pub struct SubstrateWeight;
impl WeightInfo for SubstrateWeight {
    fn create_campaign() -> Weight {
        Weight::from_parts(80_000_000, 10_000)
    }
    fn fund_campaign() -> Weight {
        Weight::from_parts(50_000_000, 6_000)
    }
    fn pause_campaign() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn cancel_campaign() -> Weight {
        Weight::from_parts(60_000_000, 8_000)
    }
    fn review_campaign() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn submit_delivery_receipt() -> Weight {
        Weight::from_parts(70_000_000, 10_000)
    }
    fn settle_era_ads() -> Weight {
        Weight::from_parts(150_000_000, 20_000)
    }
    fn flag_campaign() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn claim_ad_revenue() -> Weight {
        Weight::from_parts(50_000_000, 6_000)
    }
    fn advertiser_block_placement() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn advertiser_unblock_placement() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn advertiser_prefer_placement() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn advertiser_unprefer_placement() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn placement_block_advertiser() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn placement_unblock_advertiser() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn placement_prefer_advertiser() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn placement_unprefer_advertiser() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn flag_placement() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn slash_placement() -> Weight {
        Weight::from_parts(70_000_000, 8_000)
    }
    fn register_private_ad() -> Weight {
        Weight::from_parts(50_000_000, 6_000)
    }
    fn resume_campaign() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn expire_campaign() -> Weight {
        Weight::from_parts(60_000_000, 8_000)
    }
    fn update_campaign() -> Weight {
        Weight::from_parts(60_000_000, 8_000)
    }
    fn extend_campaign_expiry() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn force_cancel_campaign() -> Weight {
        Weight::from_parts(70_000_000, 8_000)
    }
    fn unban_placement() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn reset_slash_count() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
    }
    fn clear_placement_flags() -> Weight {
        Weight::from_parts(60_000_000, 8_000)
    }
    fn suspend_campaign() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn unsuspend_campaign() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn report_approved_campaign() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn resubmit_campaign() -> Weight {
        Weight::from_parts(70_000_000, 10_000)
    }
    fn set_placement_delivery_types() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn unregister_private_ad() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn cleanup_campaign() -> Weight {
        Weight::from_parts(80_000_000, 10_000)
    }
    fn force_settle_era_ads() -> Weight {
        Weight::from_parts(150_000_000, 20_000)
    }
    fn set_campaign_targets() -> Weight {
        Weight::from_parts(50_000_000, 6_000)
    }
    fn clear_campaign_targets() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn set_campaign_multiplier() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn set_placement_multiplier() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn set_placement_approval_required() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
    }
    fn approve_campaign_for_placement() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
    }
    fn reject_campaign_for_placement() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
    }
    fn confirm_receipt() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn dispute_receipt() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn auto_confirm_receipt() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn register_advertiser() -> Weight {
        Weight::from_parts(50_000_000, 6_000)
    }
    fn force_register_advertiser() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
    }
    fn claim_referral_earnings() -> Weight {
        Weight::from_parts(50_000_000, 6_000)
    }
    fn submit_click_receipt() -> Weight {
        Weight::from_parts(80_000_000, 12_000)
    }
}

/// 单元测试用零权重实现
impl WeightInfo for () {
    fn create_campaign() -> Weight { Weight::zero() }
    fn fund_campaign() -> Weight { Weight::zero() }
    fn pause_campaign() -> Weight { Weight::zero() }
    fn cancel_campaign() -> Weight { Weight::zero() }
    fn review_campaign() -> Weight { Weight::zero() }
    fn submit_delivery_receipt() -> Weight { Weight::zero() }
    fn settle_era_ads() -> Weight { Weight::zero() }
    fn flag_campaign() -> Weight { Weight::zero() }
    fn claim_ad_revenue() -> Weight { Weight::zero() }
    fn advertiser_block_placement() -> Weight { Weight::zero() }
    fn advertiser_unblock_placement() -> Weight { Weight::zero() }
    fn advertiser_prefer_placement() -> Weight { Weight::zero() }
    fn advertiser_unprefer_placement() -> Weight { Weight::zero() }
    fn placement_block_advertiser() -> Weight { Weight::zero() }
    fn placement_unblock_advertiser() -> Weight { Weight::zero() }
    fn placement_prefer_advertiser() -> Weight { Weight::zero() }
    fn placement_unprefer_advertiser() -> Weight { Weight::zero() }
    fn flag_placement() -> Weight { Weight::zero() }
    fn slash_placement() -> Weight { Weight::zero() }
    fn register_private_ad() -> Weight { Weight::zero() }
    fn resume_campaign() -> Weight { Weight::zero() }
    fn expire_campaign() -> Weight { Weight::zero() }
    fn update_campaign() -> Weight { Weight::zero() }
    fn extend_campaign_expiry() -> Weight { Weight::zero() }
    fn force_cancel_campaign() -> Weight { Weight::zero() }
    fn unban_placement() -> Weight { Weight::zero() }
    fn reset_slash_count() -> Weight { Weight::zero() }
    fn clear_placement_flags() -> Weight { Weight::zero() }
    fn suspend_campaign() -> Weight { Weight::zero() }
    fn unsuspend_campaign() -> Weight { Weight::zero() }
    fn report_approved_campaign() -> Weight { Weight::zero() }
    fn resubmit_campaign() -> Weight { Weight::zero() }
    fn set_placement_delivery_types() -> Weight { Weight::zero() }
    fn unregister_private_ad() -> Weight { Weight::zero() }
    fn cleanup_campaign() -> Weight { Weight::zero() }
    fn force_settle_era_ads() -> Weight { Weight::zero() }
    fn set_campaign_targets() -> Weight { Weight::zero() }
    fn clear_campaign_targets() -> Weight { Weight::zero() }
    fn set_campaign_multiplier() -> Weight { Weight::zero() }
    fn set_placement_multiplier() -> Weight { Weight::zero() }
    fn set_placement_approval_required() -> Weight { Weight::zero() }
    fn approve_campaign_for_placement() -> Weight { Weight::zero() }
    fn reject_campaign_for_placement() -> Weight { Weight::zero() }
    fn confirm_receipt() -> Weight { Weight::zero() }
    fn dispute_receipt() -> Weight { Weight::zero() }
    fn auto_confirm_receipt() -> Weight { Weight::zero() }
    fn register_advertiser() -> Weight { Weight::zero() }
    fn force_register_advertiser() -> Weight { Weight::zero() }
    fn claim_referral_earnings() -> Weight { Weight::zero() }
    fn submit_click_receipt() -> Weight { Weight::zero() }
}
