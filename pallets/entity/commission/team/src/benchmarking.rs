//! Benchmarking for pallet-commission-team.
//!
//! Uses `frame_benchmarking::v2` macro style.
//! Root extrinsics are fully benchmarked; signed extrinsics use BenchmarkHelper
//! to set up EntityProvider state for owner/admin checks.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use pallet::*;

fn make_tiers<T: Config>(count: u32) -> BoundedVec<TeamPerformanceTier<BalanceOf<T>>, T::MaxTeamTiers> {
    let v: alloc::vec::Vec<TeamPerformanceTier<BalanceOf<T>>> = (0..count)
        .map(|i| TeamPerformanceTier {
            sales_threshold: ((i + 1) * 1000).into(),
            min_team_size: (i + 1) * 5,
            rate: ((i + 1) * 100).min(10000) as u16,
        })
        .collect();
    v.try_into().expect("count should be <= MaxTeamTiers")
}

fn seed_config<T: Config>(entity_id: u64) {
    let tiers = make_tiers::<T>(3);
    TeamPerformanceConfigs::<T>::insert(
        entity_id,
        TeamPerformanceConfig {
            tiers,
            max_depth: 10,
            allow_stacking: false,
            threshold_mode: SalesThresholdMode::Nex,
        },
    );
    TeamPerformanceEnabled::<T>::insert(entity_id, true);
}

/// Trait for setting up external pallet state for signed-extrinsic benchmarks.
#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId> {
    /// Set up EntityProvider so that:
    /// - entity_id exists, is active, not locked
    /// - caller is entity owner or admin with COMMISSION_MANAGE
    fn setup_entity(caller: &AccountId, entity_id: u64);
}

/// Fallback impl for mock and runtime.
#[cfg(feature = "runtime-benchmarks")]
impl<AccountId> BenchmarkHelper<AccountId> for () {
    fn setup_entity(_caller: &AccountId, _entity_id: u64) {
        // No-op: relies on entity already existing in storage.
    }
}

#[benchmarks]
mod benches {
    use super::*;

    // ================================================================
    // set_team_performance_config (call_index 0) — Signed
    // ================================================================
    #[benchmark]
    fn set_team_performance_config(t: Linear<1, 10>) {
        let entity_id: u64 = 1;
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_entity(&caller, entity_id);
        let tiers = make_tiers::<T>(t);
        let max_depth: u8 = 10;

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id, tiers, max_depth, false, SalesThresholdMode::Nex);

        assert!(TeamPerformanceConfigs::<T>::get(entity_id).is_some());
        assert!(TeamPerformanceEnabled::<T>::get(entity_id));
    }

    // ================================================================
    // clear_team_performance_config (call_index 1) — Signed
    // ================================================================
    #[benchmark]
    fn clear_team_performance_config() {
        let entity_id: u64 = 1;
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_entity(&caller, entity_id);
        seed_config::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id);

        assert!(TeamPerformanceConfigs::<T>::get(entity_id).is_none());
    }

    // ================================================================
    // update_team_performance_params (call_index 2) — Signed
    // ================================================================
    #[benchmark]
    fn update_team_performance_params() {
        let entity_id: u64 = 1;
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_entity(&caller, entity_id);
        seed_config::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id, Some(5u8), Some(true), Some(SalesThresholdMode::Usdt));

        let config = TeamPerformanceConfigs::<T>::get(entity_id).unwrap();
        assert_eq!(config.max_depth, 5);
        assert!(config.allow_stacking);
    }

    // ================================================================
    // force_set_team_performance_config (call_index 3) — Root
    // ================================================================
    #[benchmark]
    fn force_set_team_performance_config(t: Linear<1, 10>) {
        let entity_id: u64 = 9999;
        let tiers = make_tiers::<T>(t);
        let max_depth: u8 = 10;

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, tiers, max_depth, false, SalesThresholdMode::Nex);

        assert!(TeamPerformanceConfigs::<T>::get(entity_id).is_some());
        assert!(TeamPerformanceEnabled::<T>::get(entity_id));
    }

    // ================================================================
    // force_clear_team_performance_config (call_index 4) — Root
    // ================================================================
    #[benchmark]
    fn force_clear_team_performance_config() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        assert!(TeamPerformanceConfigs::<T>::get(entity_id).is_none());
    }

    // ================================================================
    // pause_team_performance (call_index 5) — Signed
    // ================================================================
    #[benchmark]
    fn pause_team_performance() {
        let entity_id: u64 = 1;
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_entity(&caller, entity_id);
        seed_config::<T>(entity_id); // sets enabled=true

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id);

        assert!(!TeamPerformanceEnabled::<T>::get(entity_id));
    }

    // ================================================================
    // resume_team_performance (call_index 6) — Signed
    // ================================================================
    #[benchmark]
    fn resume_team_performance() {
        let entity_id: u64 = 1;
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_entity(&caller, entity_id);
        seed_config::<T>(entity_id);
        TeamPerformanceEnabled::<T>::insert(entity_id, false); // pause it

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id);

        assert!(TeamPerformanceEnabled::<T>::get(entity_id));
    }

    // ================================================================
    // add_tier (call_index 7) — Signed
    // ================================================================
    #[benchmark]
    fn add_tier() {
        let entity_id: u64 = 1;
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_entity(&caller, entity_id);
        seed_config::<T>(entity_id); // 3 tiers with thresholds 1000,2000,3000

        let new_tier = TeamPerformanceTier {
            sales_threshold: 5000u32.into(),
            min_team_size: 50,
            rate: 800,
        };

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id, new_tier);

        let config = TeamPerformanceConfigs::<T>::get(entity_id).unwrap();
        assert_eq!(config.tiers.len(), 4);
    }

    // ================================================================
    // update_tier (call_index 8) — Signed
    // ================================================================
    #[benchmark]
    fn update_tier() {
        let entity_id: u64 = 1;
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_entity(&caller, entity_id);
        seed_config::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id, 0u32, None, Some(99u32), Some(500u16));

        let config = TeamPerformanceConfigs::<T>::get(entity_id).unwrap();
        assert_eq!(config.tiers[0].min_team_size, 99);
        assert_eq!(config.tiers[0].rate, 500);
    }

    // ================================================================
    // remove_tier (call_index 9) — Signed
    // ================================================================
    #[benchmark]
    fn remove_tier() {
        let entity_id: u64 = 1;
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_entity(&caller, entity_id);
        seed_config::<T>(entity_id); // 3 tiers

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id, 2u32);

        let config = TeamPerformanceConfigs::<T>::get(entity_id).unwrap();
        assert_eq!(config.tiers.len(), 2);
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
