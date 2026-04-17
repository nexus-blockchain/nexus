use frame_support::weights::Weight;

pub trait WeightInfo {
    fn stake_for_ads() -> Weight;
    fn unstake_for_ads() -> Weight;
    fn set_tee_ad_pct() -> Weight;
    fn set_community_ad_pct() -> Weight;
    fn set_community_admin() -> Weight;
    fn report_node_audience() -> Weight;
    fn check_audience_surge() -> Weight;
    fn resume_audience_surge() -> Weight;
    fn cross_validate_nodes() -> Weight;
    fn slash_community(s: u32) -> Weight;
    fn admin_pause_ads() -> Weight;
    fn admin_resume_ads() -> Weight;
    fn resign_community_admin() -> Weight;
    fn withdraw_unbonded() -> Weight;
    fn set_stake_tiers() -> Weight;
    fn force_set_community_admin() -> Weight;
    fn set_global_ads_pause() -> Weight;
    fn set_bot_ads_enabled() -> Weight;
    fn claim_staker_reward() -> Weight;
    fn force_unstake(s: u32) -> Weight;
}

pub struct SubstrateWeight;
impl WeightInfo for SubstrateWeight {
    fn stake_for_ads() -> Weight {
        Weight::from_parts(60_000_000, 8_000)
    }
    fn unstake_for_ads() -> Weight {
        Weight::from_parts(60_000_000, 8_000)
    }
    fn set_tee_ad_pct() -> Weight {
        Weight::from_parts(20_000_000, 4_000)
    }
    fn set_community_ad_pct() -> Weight {
        Weight::from_parts(20_000_000, 4_000)
    }
    fn set_community_admin() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
    }
    fn report_node_audience() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn check_audience_surge() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
    }
    fn resume_audience_surge() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
    }
    fn cross_validate_nodes() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn slash_community(s: u32) -> Weight {
        Weight::from_parts(
            50_000_000u64.saturating_add(10_000_000u64.saturating_mul(s as u64)),
            8_000u64.saturating_add(1_000u64.saturating_mul(s as u64)),
        )
    }
    fn admin_pause_ads() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
    }
    fn admin_resume_ads() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
    }
    fn resign_community_admin() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
    }
    fn withdraw_unbonded() -> Weight {
        Weight::from_parts(50_000_000, 6_000)
    }
    fn set_stake_tiers() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
    }
    fn force_set_community_admin() -> Weight {
        Weight::from_parts(20_000_000, 4_000)
    }
    fn set_global_ads_pause() -> Weight {
        Weight::from_parts(15_000_000, 3_000)
    }
    fn set_bot_ads_enabled() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
    }
    fn claim_staker_reward() -> Weight {
        Weight::from_parts(50_000_000, 6_000)
    }
    fn force_unstake(s: u32) -> Weight {
        Weight::from_parts(
            50_000_000u64.saturating_add(10_000_000u64.saturating_mul(s as u64)),
            8_000u64.saturating_add(1_000u64.saturating_mul(s as u64)),
        )
    }
}

impl WeightInfo for () {
    fn stake_for_ads() -> Weight {
        Weight::zero()
    }
    fn unstake_for_ads() -> Weight {
        Weight::zero()
    }
    fn set_tee_ad_pct() -> Weight {
        Weight::zero()
    }
    fn set_community_ad_pct() -> Weight {
        Weight::zero()
    }
    fn set_community_admin() -> Weight {
        Weight::zero()
    }
    fn report_node_audience() -> Weight {
        Weight::zero()
    }
    fn check_audience_surge() -> Weight {
        Weight::zero()
    }
    fn resume_audience_surge() -> Weight {
        Weight::zero()
    }
    fn cross_validate_nodes() -> Weight {
        Weight::zero()
    }
    fn slash_community(_s: u32) -> Weight {
        Weight::zero()
    }
    fn admin_pause_ads() -> Weight {
        Weight::zero()
    }
    fn admin_resume_ads() -> Weight {
        Weight::zero()
    }
    fn resign_community_admin() -> Weight {
        Weight::zero()
    }
    fn withdraw_unbonded() -> Weight {
        Weight::zero()
    }
    fn set_stake_tiers() -> Weight {
        Weight::zero()
    }
    fn force_set_community_admin() -> Weight {
        Weight::zero()
    }
    fn set_global_ads_pause() -> Weight {
        Weight::zero()
    }
    fn set_bot_ads_enabled() -> Weight {
        Weight::zero()
    }
    fn claim_staker_reward() -> Weight {
        Weight::zero()
    }
    fn force_unstake(_s: u32) -> Weight {
        Weight::zero()
    }
}
