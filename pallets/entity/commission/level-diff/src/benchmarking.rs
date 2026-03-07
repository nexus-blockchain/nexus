//! Benchmarking for pallet-commission-level-diff.
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

fn make_rates<T: Config>(count: u32) -> BoundedVec<u16, T::MaxCustomLevels> {
    let v: alloc::vec::Vec<u16> = (0..count)
        .map(|i| ((i + 1) * 100).min(10000) as u16)
        .collect();
    v.try_into().expect("count should be <= MaxCustomLevels")
}

fn seed_config<T: Config>(entity_id: u64) {
    let rates = make_rates::<T>(3);
    CustomLevelDiffConfigs::<T>::insert(
        entity_id,
        CustomLevelDiffConfig {
            level_rates: rates,
            max_depth: 10,
        },
    );
}

#[benchmarks]
mod benches {
    use super::*;

    #[benchmark]
    fn force_set_level_diff_config(l: Linear<1, 10>) {
        let entity_id: u64 = 9999;
        let level_rates = make_rates::<T>(l);
        let max_depth: u8 = 10;

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, level_rates, max_depth);

        assert!(CustomLevelDiffConfigs::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn force_clear_level_diff_config() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        assert!(CustomLevelDiffConfigs::<T>::get(entity_id).is_none());
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
