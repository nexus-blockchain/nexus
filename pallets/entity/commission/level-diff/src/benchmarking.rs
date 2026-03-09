//! Benchmarking for pallet-commission-level-diff.
//!
//! Uses `frame_benchmarking::v2` macro style.
//! Root extrinsics are fully benchmarked; signed extrinsics require runtime
//! to provide valid EntityProvider/MemberProvider data via BenchmarkHelper.

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

/// Trait for setting up external pallet state for signed-extrinsic benchmarks.
#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId> {
    /// Set up EntityProvider so that:
    /// - entity_id exists, is active, not locked
    /// - caller is entity owner or admin with COMMISSION_MANAGE
    fn setup_entity(caller: &AccountId, entity_id: u64);
}

/// Fallback impl for mock and runtime.
/// In runtime context, signed extrinsic benchmarks need the runtime
/// to provide a real implementation that sets up EntityProvider state.
#[cfg(feature = "runtime-benchmarks")]
impl<AccountId> BenchmarkHelper<AccountId> for () {
    fn setup_entity(_caller: &AccountId, _entity_id: u64) {
        // No-op: relies on entity already existing in storage.
        // For mock: thread_local! routing handles entity_id=1, owner=OWNER(100).
        // For runtime: override this with a real impl that creates entity state.
    }
}

#[benchmarks]
mod benches {
    use super::*;

    // ================================================================
    // set_level_diff_config (call_index 1) — Signed
    // ================================================================
    #[benchmark]
    fn set_level_diff_config(l: Linear<1, 10>) {
        let entity_id: u64 = 1;
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_entity(&caller, entity_id);
        let level_rates = make_rates::<T>(l);
        let max_depth: u8 = 10;

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id, level_rates, max_depth);

        assert!(CustomLevelDiffConfigs::<T>::get(entity_id).is_some());
    }

    // ================================================================
    // clear_level_diff_config (call_index 2) — Signed
    // ================================================================
    #[benchmark]
    fn clear_level_diff_config() {
        let entity_id: u64 = 1;
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_entity(&caller, entity_id);
        seed_config::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id);

        assert!(CustomLevelDiffConfigs::<T>::get(entity_id).is_none());
    }

    // ================================================================
    // update_level_diff_config (call_index 5) — Signed
    // ================================================================
    #[benchmark]
    fn update_level_diff_config(l: Linear<1, 10>) {
        let entity_id: u64 = 1;
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_entity(&caller, entity_id);
        seed_config::<T>(entity_id);
        let new_rates = make_rates::<T>(l);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id, Some(new_rates), Some(5u8));

        let config = CustomLevelDiffConfigs::<T>::get(entity_id).unwrap();
        assert_eq!(config.max_depth, 5);
    }

    // ================================================================
    // force_set_level_diff_config (call_index 3) — Root
    // ================================================================
    #[benchmark]
    fn force_set_level_diff_config(l: Linear<1, 10>) {
        let entity_id: u64 = 1;
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_entity(&caller, entity_id);
        let level_rates = make_rates::<T>(l);
        let max_depth: u8 = 10;

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, level_rates, max_depth);

        assert!(CustomLevelDiffConfigs::<T>::get(entity_id).is_some());
    }

    // ================================================================
    // force_clear_level_diff_config (call_index 4) — Root
    // ================================================================
    #[benchmark]
    fn force_clear_level_diff_config() {
        let entity_id: u64 = 1;
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_entity(&caller, entity_id);
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
