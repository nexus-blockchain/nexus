//! Benchmarking for pallet-commission-team.
//!
//! Uses `frame_benchmarking::v2` macro style.
//! Root extrinsics are fully benchmarked; signed extrinsics use force_* variants
//! as proxy since they share the same storage write path. The additional overhead
//! of signed extrinsics (3 reads for owner/locked/active checks) is accounted for
//! in the WeightInfo hand-estimates.

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

#[benchmarks]
mod benches {
    use super::*;

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

    #[benchmark]
    fn force_clear_team_performance_config() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        assert!(TeamPerformanceConfigs::<T>::get(entity_id).is_none());
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
