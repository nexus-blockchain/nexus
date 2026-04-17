//! Benchmarking setup for pallet-ads-grouprobot

#![cfg(feature = "runtime-benchmarks")]

use super::*;
#[allow(unused)]
use crate::Pallet as AdsGroupRobot;
use frame_benchmarking::v2::*;
use frame_support::traits::{Currency, Get, ReservableCurrency};
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use pallet_grouprobot_primitives::*;
use sp_runtime::traits::Bounded;
use sp_runtime::Saturating;

fn community_hash(n: u8) -> CommunityIdHash {
    let mut h = [0u8; 32];
    h[0] = n;
    h
}

fn funded_caller<T: Config>(name: &'static str, idx: u32) -> T::AccountId {
    let account: T::AccountId = frame_benchmarking::account(name, idx, 0);
    let amount = BalanceOf::<T>::max_value() / 4u32.into();
    let _ = T::Currency::deposit_creating(&account, amount);
    account
}

fn seed_admin<T: Config>(cid: &CommunityIdHash, admin: &T::AccountId) {
    CommunityAdmin::<T>::insert(cid, admin);
}

fn seed_stake<T: Config>(cid: &CommunityIdHash, staker: &T::AccountId, amount: BalanceOf<T>) {
    let _ = T::Currency::reserve(staker, amount);
    CommunityAdStake::<T>::insert(cid, amount);
    CommunityStakers::<T>::insert(cid, staker, amount);
    CommunityStakerCount::<T>::insert(cid, 1u32);
    CommunityAudienceCap::<T>::insert(cid, Pallet::<T>::compute_audience_cap(amount));
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn stake_for_ads() {
        let caller = funded_caller::<T>("caller", 0);
        let cid = community_hash(1);
        let amount: BalanceOf<T> = 1000u32.into();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid, amount);
    }

    #[benchmark]
    fn unstake_for_ads() {
        let caller = funded_caller::<T>("caller", 0);
        let cid = community_hash(1);
        let amount: BalanceOf<T> = 1000u32.into();
        seed_stake::<T>(&cid, &caller, amount);
        seed_admin::<T>(&cid, &caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid, amount);
    }

    #[benchmark]
    fn set_tee_ad_pct() {
        #[extrinsic_call]
        _(RawOrigin::Root, Some(5u32));
    }

    #[benchmark]
    fn set_community_ad_pct() {
        #[extrinsic_call]
        _(RawOrigin::Root, Some(80u32));
    }

    #[benchmark]
    fn set_community_admin() {
        let caller = funded_caller::<T>("caller", 0);
        let cid = community_hash(1);
        let new_admin: T::AccountId = frame_benchmarking::account("admin", 1, 0);
        seed_admin::<T>(&cid, &caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid, new_admin);
    }

    #[benchmark]
    fn report_node_audience() {
        let caller = funded_caller::<T>("caller", 0);
        let cid = community_hash(1);
        seed_admin::<T>(&cid, &caller);
        let node_id: NodeId = [0u8; 32];

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid, node_id, 100u32);
    }

    #[benchmark]
    fn check_audience_surge() {
        let caller = funded_caller::<T>("caller", 0);
        let cid = community_hash(1);
        seed_admin::<T>(&cid, &caller);
        CommunityAudienceCap::<T>::insert(&cid, 50u32);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid, 100u32);
    }

    #[benchmark]
    fn resume_audience_surge() {
        let cid = community_hash(1);
        AudienceSurgePaused::<T>::insert(&cid, 1u32);

        #[extrinsic_call]
        _(RawOrigin::Root, cid);
    }

    #[benchmark]
    fn cross_validate_nodes() {
        let caller = funded_caller::<T>("caller", 0);
        let cid = community_hash(1);
        seed_admin::<T>(&cid, &caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid);
    }

    #[benchmark]
    fn slash_community(s: Linear<1, 5>) {
        let cid = community_hash(1);
        for i in 0..s {
            let staker: T::AccountId = frame_benchmarking::account("staker", i, 0);
            let amount: BalanceOf<T> = 100u32.into();
            let _ = T::Currency::deposit_creating(&staker, 10_000u32.into());
            let _ = T::Currency::reserve(&staker, amount);
            CommunityStakers::<T>::insert(&cid, &staker, amount);
            CommunityAdStake::<T>::mutate(&cid, |s| *s = s.saturating_add(amount));
        }
        CommunityStakerCount::<T>::insert(&cid, s);
        CommunityAudienceCap::<T>::insert(&cid, 100u32);

        #[extrinsic_call]
        _(RawOrigin::Root, cid);
    }

    #[benchmark]
    fn admin_pause_ads() {
        let caller = funded_caller::<T>("caller", 0);
        let cid = community_hash(1);
        seed_admin::<T>(&cid, &caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid);
    }

    #[benchmark]
    fn admin_resume_ads() {
        let caller = funded_caller::<T>("caller", 0);
        let cid = community_hash(1);
        seed_admin::<T>(&cid, &caller);
        AdminPausedAds::<T>::insert(&cid, true);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid);
    }

    #[benchmark]
    fn resign_community_admin() {
        let caller = funded_caller::<T>("caller", 0);
        let cid = community_hash(1);
        seed_admin::<T>(&cid, &caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid);
    }

    #[benchmark]
    fn withdraw_unbonded() {
        let caller = funded_caller::<T>("caller", 0);
        let cid = community_hash(1);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid);
    }

    #[benchmark]
    fn set_stake_tiers() {
        let tiers: BoundedVec<(u128, u32), frame_support::traits::ConstU32<10>> =
            BoundedVec::try_from(alloc::vec![(1000u128, 50u32), (5000u128, 200u32)]).unwrap();

        #[extrinsic_call]
        _(RawOrigin::Root, tiers);
    }

    #[benchmark]
    fn force_set_community_admin() {
        let new_admin: T::AccountId = frame_benchmarking::account("admin", 0, 0);
        let cid = community_hash(1);

        #[extrinsic_call]
        _(RawOrigin::Root, cid, new_admin);
    }

    #[benchmark]
    fn set_global_ads_pause() {
        #[extrinsic_call]
        _(RawOrigin::Root, true);
    }

    #[benchmark]
    fn set_bot_ads_enabled() {
        let cid = community_hash(1);

        #[extrinsic_call]
        _(RawOrigin::Root, cid, false);
    }

    #[benchmark]
    fn claim_staker_reward() {
        let caller = funded_caller::<T>("caller", 0);
        let cid = community_hash(1);
        StakerClaimable::<T>::insert(&cid, &caller, BalanceOf::<T>::from(0u32));

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid);
    }

    #[benchmark]
    fn force_unstake(s: Linear<1, 5>) {
        let cid = community_hash(1);
        for i in 0..s {
            let staker: T::AccountId = frame_benchmarking::account("staker", i, 0);
            let amount: BalanceOf<T> = 100u32.into();
            let _ = T::Currency::deposit_creating(&staker, 10_000u32.into());
            let _ = T::Currency::reserve(&staker, amount);
            CommunityStakers::<T>::insert(&cid, &staker, amount);
            CommunityAdStake::<T>::mutate(&cid, |s| *s = s.saturating_add(amount));
        }
        CommunityStakerCount::<T>::insert(&cid, s);
        CommunityAudienceCap::<T>::insert(&cid, 100u32);

        #[extrinsic_call]
        _(RawOrigin::Root, cid);
    }

    impl_benchmark_test_suite!(
        AdsGroupRobot,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
