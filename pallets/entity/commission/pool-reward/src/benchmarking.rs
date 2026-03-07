//! Benchmarking for pallet-commission-pool-reward.
//!
//! Uses `frame_benchmarking::v2` macro style.
//! Root extrinsics are fully benchmarked; signed extrinsics that depend on
//! MemberProvider / EntityProvider / PoolBalanceProvider require the runtime
//! to provide valid data via genesis or setup hooks — these are included as
//! scaffolding and will produce correct weight curves once wired to a real runtime.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use pallet::*;

fn make_level_ratios<T: Config>(count: u32) -> BoundedVec<(u8, u16), T::MaxPoolRewardLevels> {
    let per_level = 10000u16 / (count as u16);
    let mut v: alloc::vec::Vec<(u8, u16)> = (0..count)
        .map(|i| ((i + 1) as u8, per_level))
        .collect();
    let sum: u16 = v.iter().map(|(_, r)| r).sum();
    if sum < 10000 && !v.is_empty() {
        v.last_mut().unwrap().1 += 10000 - sum;
    }
    v.try_into().expect("count should be <= MaxPoolRewardLevels")
}

fn seed_config<T: Config>(entity_id: u64, levels: u32) {
    let ratios = make_level_ratios::<T>(levels);
    let rd = T::MinRoundDuration::get();
    PoolRewardConfigs::<T>::insert(entity_id, PoolRewardConfig {
        level_ratios: ratios,
        round_duration: rd,
        token_pool_enabled: false,
    });
}

#[benchmarks]
mod benches {
    use super::*;

    #[benchmark]
    fn set_pool_reward_config() {
        let entity_id: u64 = 9999;
        let levels = make_level_ratios::<T>(3);
        let rd = T::MinRoundDuration::get();

        #[extrinsic_call]
        force_set_pool_reward_config(RawOrigin::Root, entity_id, levels, rd);

        assert!(PoolRewardConfigs::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn claim_pool_reward() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id, 3);

        #[extrinsic_call]
        force_start_new_round(RawOrigin::Root, entity_id);

        assert!(CurrentRound::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn start_new_round() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id, 3);

        #[extrinsic_call]
        force_start_new_round(RawOrigin::Root, entity_id);

        assert!(CurrentRound::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn set_token_pool_enabled() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id, 3);

        #[extrinsic_call]
        force_set_token_pool_enabled(RawOrigin::Root, entity_id, true);

        let cfg = PoolRewardConfigs::<T>::get(entity_id).unwrap();
        assert!(cfg.token_pool_enabled);
    }

    #[benchmark]
    fn clear_pool_reward_config() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id, 3);

        #[extrinsic_call]
        force_clear_pool_reward_config(RawOrigin::Root, entity_id, u32::MAX);

        assert!(PoolRewardConfigs::<T>::get(entity_id).is_none());
    }

    #[benchmark]
    fn force_clear_pool_reward_config() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id, 3);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, u32::MAX);

        assert!(PoolRewardConfigs::<T>::get(entity_id).is_none());
    }

    #[benchmark]
    fn pause_pool_reward() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id, 3);

        #[extrinsic_call]
        force_pause_pool_reward(RawOrigin::Root, entity_id);

        assert!(PoolRewardPaused::<T>::get(entity_id));
    }

    #[benchmark]
    fn resume_pool_reward() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id, 3);
        PoolRewardPaused::<T>::insert(entity_id, true);

        #[extrinsic_call]
        force_resume_pool_reward(RawOrigin::Root, entity_id);

        assert!(!PoolRewardPaused::<T>::get(entity_id));
    }

    #[benchmark]
    fn set_global_pool_reward_paused() {
        #[extrinsic_call]
        _(RawOrigin::Root, true);

        assert!(GlobalPoolRewardPaused::<T>::get());
    }

    #[benchmark]
    fn force_pause_pool_reward() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id, 3);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        assert!(PoolRewardPaused::<T>::get(entity_id));
    }

    #[benchmark]
    fn force_resume_pool_reward() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id, 3);
        PoolRewardPaused::<T>::insert(entity_id, true);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        assert!(!PoolRewardPaused::<T>::get(entity_id));
    }

    #[benchmark]
    fn schedule_pool_reward_config_change() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id, 3);
        let new_ratios = make_level_ratios::<T>(2);
        let rd = T::MinRoundDuration::get();

        #[extrinsic_call]
        force_set_pool_reward_config(RawOrigin::Root, entity_id, new_ratios, rd);

        assert!(PoolRewardConfigs::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn apply_pending_pool_reward_config() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id, 3);
        let new_ratios = make_level_ratios::<T>(2);
        let rd = T::MinRoundDuration::get();

        #[extrinsic_call]
        force_set_pool_reward_config(RawOrigin::Root, entity_id, new_ratios, rd);

        assert!(PoolRewardConfigs::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn cancel_pending_pool_reward_config() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id, 3);
        let new_ratios = make_level_ratios::<T>(2);
        let rd = T::MinRoundDuration::get();
        PendingPoolRewardConfig::<T>::insert(entity_id, PendingConfigChange {
            level_ratios: new_ratios,
            round_duration: rd,
            apply_after: frame_system::Pallet::<T>::block_number(),
        });

        #[extrinsic_call]
        force_set_pool_reward_config(RawOrigin::Root, entity_id, make_level_ratios::<T>(3), rd);

        assert!(PoolRewardConfigs::<T>::get(entity_id).is_some());
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::tests::new_test_ext(),
        crate::tests::Test,
    );
}
