//! Benchmarking for pallet-grouprobot-rewards.
//!
//! Uses `frame_benchmarking::v2` macro style.
//! All 11 extrinsics are benchmarked.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use pallet::*;
use sp_runtime::traits::Bounded;

fn funded_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
    let account: T::AccountId = frame_benchmarking::account(name, index, 0);
    let amount = BalanceOf::<T>::max_value() / 2u32.into();
    let _ = T::Currency::deposit_creating(&account, amount);
    account
}

fn node_id_bench(n: u8) -> NodeId {
    let mut id = [0u8; 32];
    id[0] = n;
    id
}

fn bot_hash_bench(n: u8) -> BotIdHash {
    let mut h = [0u8; 32];
    h[0] = n;
    h
}

#[benchmarks]
mod benches {
    use super::*;

    #[benchmark]
    fn claim_rewards() {
        let caller = funded_account::<T>("operator", 0);
        let node = node_id_bench(1);
        NodePendingRewards::<T>::insert(node, BalanceOf::<T>::from(500u32));
        let pool = T::RewardPoolAccount::get();
        let _ = T::Currency::deposit_creating(&pool, BalanceOf::<T>::from(1_000_000u32));

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), node);

        assert_eq!(
            NodePendingRewards::<T>::get(node),
            BalanceOf::<T>::from(0u32)
        );
    }

    #[benchmark]
    fn rescue_stranded_rewards() {
        let recipient = funded_account::<T>("recipient", 0);
        let node = node_id_bench(99);
        NodePendingRewards::<T>::insert(node, BalanceOf::<T>::from(500u32));
        let pool = T::RewardPoolAccount::get();
        let _ = T::Currency::deposit_creating(&pool, BalanceOf::<T>::from(1_000_000u32));

        #[extrinsic_call]
        _(RawOrigin::Root, node, recipient);

        assert_eq!(
            NodePendingRewards::<T>::get(node),
            BalanceOf::<T>::from(0u32)
        );
    }

    #[benchmark]
    fn batch_claim_rewards(n: Linear<1, 10>) {
        let caller = funded_account::<T>("operator", 0);
        let pool = T::RewardPoolAccount::get();
        let _ = T::Currency::deposit_creating(&pool, BalanceOf::<T>::from(10_000_000u32));

        let mut ids = alloc::vec::Vec::new();
        for i in 0..n {
            let node = node_id_bench(i as u8 + 1);
            NodePendingRewards::<T>::insert(node, BalanceOf::<T>::from(100u32));
            ids.push(node);
        }

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ids);
    }

    #[benchmark]
    fn set_reward_recipient() {
        let caller = funded_account::<T>("operator", 0);
        let recipient = funded_account::<T>("recipient", 1);
        let node = node_id_bench(1);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), node, Some(recipient));
    }

    #[benchmark]
    fn force_slash_pending_rewards() {
        let node = node_id_bench(1);
        NodePendingRewards::<T>::insert(node, BalanceOf::<T>::from(1000u32));

        #[extrinsic_call]
        _(RawOrigin::Root, node, BalanceOf::<T>::from(500u32));

        assert_eq!(
            NodePendingRewards::<T>::get(node),
            BalanceOf::<T>::from(500u32)
        );
    }

    #[benchmark]
    fn set_reward_split() {
        let caller = funded_account::<T>("bot_owner", 0);
        let bot = bot_hash_bench(10);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bot, 2000u32);
    }

    #[benchmark]
    fn claim_owner_rewards() {
        let caller = funded_account::<T>("bot_owner", 0);
        let bot = bot_hash_bench(10);
        OwnerPendingRewards::<T>::insert(bot, BalanceOf::<T>::from(500u32));
        let pool = T::RewardPoolAccount::get();
        let _ = T::Currency::deposit_creating(&pool, BalanceOf::<T>::from(1_000_000u32));

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bot);

        assert_eq!(
            OwnerPendingRewards::<T>::get(bot),
            BalanceOf::<T>::from(0u32)
        );
    }

    #[benchmark]
    fn pause_distribution() {
        #[extrinsic_call]
        _(RawOrigin::Root);

        assert!(DistributionPaused::<T>::get());
    }

    #[benchmark]
    fn resume_distribution() {
        DistributionPaused::<T>::put(true);

        #[extrinsic_call]
        _(RawOrigin::Root);

        assert!(!DistributionPaused::<T>::get());
    }

    #[benchmark]
    fn force_set_pending_rewards() {
        let node = node_id_bench(1);

        #[extrinsic_call]
        _(RawOrigin::Root, node, BalanceOf::<T>::from(1000u32));

        assert_eq!(
            NodePendingRewards::<T>::get(node),
            BalanceOf::<T>::from(1000u32)
        );
    }

    #[benchmark]
    fn force_prune_era_rewards(n: Linear<1, 100>) {
        for era in 0..n as u64 {
            let info = EraRewardInfo {
                subscription_income: BalanceOf::<T>::from(100u32),
                ads_income: BalanceOf::<T>::from(0u32),
                inflation_mint: BalanceOf::<T>::from(50u32),
                total_distributed: BalanceOf::<T>::from(150u32),
                treasury_share: BalanceOf::<T>::from(10u32),
                node_count: 1,
            };
            EraRewards::<T>::insert(era, info);
        }
        EraCleanupCursor::<T>::put(0u64);

        #[extrinsic_call]
        _(RawOrigin::Root, n as u64);

        assert_eq!(EraCleanupCursor::<T>::get(), n as u64);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test,);
}
