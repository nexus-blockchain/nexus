//! Benchmarking setup for pallet-ads-entity

#![cfg(feature = "runtime-benchmarks")]

use super::*;
#[allow(unused)]
use crate::Pallet as AdsEntity;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn placeholder() {
        #[extrinsic_call]
        _(RawOrigin::None);
    }

    impl_benchmark_test_suite!(
        AdsEntity,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
