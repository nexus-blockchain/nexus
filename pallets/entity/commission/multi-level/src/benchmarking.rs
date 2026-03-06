//! Benchmarking for pallet-commission-multi-level.
//!
//! Uses `frame_benchmarking::v2` macro style.
//! Root extrinsics are fully benchmarked; signed extrinsics require runtime
//! to provide valid EntityProvider/MemberProvider data via genesis or setup hooks.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use pallet::*;

fn make_tiers<T: Config>(count: u32) -> BoundedVec<MultiLevelTier, T::MaxMultiLevels> {
    let v: alloc::vec::Vec<MultiLevelTier> = (0..count)
        .map(|i| MultiLevelTier {
            rate: ((i + 1) * 100).min(10000) as u16,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
        })
        .collect();
    v.try_into().expect("count should be <= MaxMultiLevels")
}

fn seed_config<T: Config>(entity_id: u64) {
    let tiers = make_tiers::<T>(3);
    MultiLevelConfigs::<T>::insert(
        entity_id,
        MultiLevelConfig { levels: tiers, max_total_rate: 5000 },
    );
}

#[benchmarks]
mod benches {
    use super::*;

    #[benchmark]
    fn force_set_multi_level_config(l: Linear<1, 15>) {
        let entity_id: u64 = 9999;
        let levels = make_tiers::<T>(l);
        let max_total_rate: u16 = 5000;

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, levels, max_total_rate);

        assert!(MultiLevelConfigs::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn force_clear_multi_level_config() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        assert!(MultiLevelConfigs::<T>::get(entity_id).is_none());
    }

    #[benchmark]
    fn force_pause_multi_level() {
        let entity_id: u64 = 9999;

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        assert!(GlobalPaused::<T>::get(entity_id));
    }

    #[benchmark]
    fn force_resume_multi_level() {
        let entity_id: u64 = 9999;
        GlobalPaused::<T>::insert(entity_id, true);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        assert!(!GlobalPaused::<T>::get(entity_id));
    }

    #[benchmark]
    fn force_cleanup_entity(m: Linear<0, 1000>) {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id);
        GlobalPaused::<T>::insert(entity_id, true);
        ConfigChangeLogCount::<T>::insert(entity_id, 5u32);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, m);

        assert!(MultiLevelConfigs::<T>::get(entity_id).is_none());
        assert!(!GlobalPaused::<T>::get(entity_id));
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
