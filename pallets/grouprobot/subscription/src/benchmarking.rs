//! Benchmarking setup for pallet-grouprobot-subscription
//!
//! Every benchmark seeds only this pallet's own storage items.
//! External trait dependencies (`BotRegistry`) are satisfied by a
//! `BenchmarkHelper` trait that the runtime must implement.

#![cfg(feature = "runtime-benchmarks")]

extern crate alloc;

use super::*;
#[allow(unused)]
use crate::Pallet as GroupRobotSubscription;
use frame_benchmarking::v2::*;
use frame_support::traits::{Currency, Get, ReservableCurrency};
use frame_system::RawOrigin;
use pallet_grouprobot_primitives::*;
use sp_runtime::Saturating;

fn bot_hash(n: u8) -> BotIdHash {
    let mut h = [0u8; 32];
    h[0] = n;
    h
}

fn community_hash(n: u8) -> CommunityIdHash {
    let mut h = [0u8; 32];
    h[0] = n;
    h[1] = 0xCC;
    h
}

/// Seed an active subscription record directly into storage.
fn seed_subscription<T: Config>(
    bot_id_hash: &BotIdHash,
    owner: &T::AccountId,
    tier: SubscriptionTier,
    status: SubscriptionStatus,
) {
    let fee = Pallet::<T>::tier_fee(&tier);
    Subscriptions::<T>::insert(
        bot_id_hash,
        SubscriptionRecord::<T> {
            owner: owner.clone(),
            bot_id_hash: *bot_id_hash,
            tier,
            fee_per_era: fee,
            started_at: frame_system::Pallet::<T>::block_number(),
            status,
        },
    );
}

/// Seed escrow for a bot.
fn seed_escrow<T: Config>(bot_id_hash: &BotIdHash, amount: BalanceOf<T>) {
    SubscriptionEscrow::<T>::insert(bot_id_hash, amount);
}

/// Seed an ad commitment record.
fn seed_ad_commitment<T: Config>(
    bot_id_hash: &BotIdHash,
    owner: &T::AccountId,
    community_id_hash: CommunityIdHash,
    ads: u32,
    status: AdCommitmentStatus,
) {
    let tier = Pallet::<T>::ads_to_tier(ads);
    AdCommitments::<T>::insert(
        bot_id_hash,
        AdCommitmentRecord::<T> {
            owner: owner.clone(),
            bot_id_hash: *bot_id_hash,
            community_id_hash,
            committed_ads_per_era: ads,
            effective_tier: tier,
            underdelivery_eras: 0,
            status,
            started_at: frame_system::Pallet::<T>::block_number(),
        },
    );
}

/// Trait for setting up external pallet state for benchmarks.
#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId> {
    /// Set up BotRegistry so that bot is active, caller is owner, has operator.
    fn setup_active_bot_with_operator(caller: &AccountId, bot_id_hash: &BotIdHash);
    /// Set up bot_operator for the given bot.
    fn setup_bot_operator(operator: &AccountId, bot_id_hash: &BotIdHash);
    /// Fund the account with enough balance.
    fn fund_account(account: &AccountId);
}

/// Fallback impl (no-op).
#[cfg(feature = "runtime-benchmarks")]
impl<AccountId> BenchmarkHelper<AccountId> for () {
    fn setup_active_bot_with_operator(_caller: &AccountId, _bot_id_hash: &BotIdHash) {}
    fn setup_bot_operator(_operator: &AccountId, _bot_id_hash: &BotIdHash) {}
    fn fund_account(_account: &AccountId) {}
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // ================================================================
    // subscribe (call_index 0)
    // ================================================================
    #[benchmark]
    fn subscribe() {
        let bh = bot_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_active_bot_with_operator(&caller, &bh);
        T::BenchmarkHelper::fund_account(&caller);
        let deposit: BalanceOf<T> = T::BasicFeePerEra::get().saturating_mul(2u32.into());

        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller),
            bh,
            SubscriptionTier::Basic,
            deposit,
        );

        assert!(Subscriptions::<T>::contains_key(&bh));
    }

    // ================================================================
    // deposit_subscription (call_index 1)
    // ================================================================
    #[benchmark]
    fn deposit_subscription() {
        let bh = bot_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::fund_account(&caller);
        seed_subscription::<T>(
            &bh,
            &caller,
            SubscriptionTier::Basic,
            SubscriptionStatus::Active,
        );
        seed_escrow::<T>(&bh, T::BasicFeePerEra::get());
        let amount: BalanceOf<T> = T::BasicFeePerEra::get();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bh, amount);
    }

    // ================================================================
    // cancel_subscription (call_index 2)
    // ================================================================
    #[benchmark]
    fn cancel_subscription() {
        let bh = bot_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::fund_account(&caller);
        let fee = T::BasicFeePerEra::get();
        let _ = T::Currency::reserve(&caller, fee);
        seed_subscription::<T>(
            &bh,
            &caller,
            SubscriptionTier::Basic,
            SubscriptionStatus::Active,
        );
        seed_escrow::<T>(&bh, fee);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bh);

        let sub = Subscriptions::<T>::get(&bh).unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Cancelled);
    }

    // ================================================================
    // change_tier (call_index 3)
    // ================================================================
    #[benchmark]
    fn change_tier() {
        let bh = bot_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::fund_account(&caller);
        seed_subscription::<T>(
            &bh,
            &caller,
            SubscriptionTier::Basic,
            SubscriptionStatus::Active,
        );
        seed_escrow::<T>(
            &bh,
            T::EnterpriseFeePerEra::get().saturating_mul(2u32.into()),
        );

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bh, SubscriptionTier::Pro);

        let sub = Subscriptions::<T>::get(&bh).unwrap();
        assert_eq!(sub.tier, SubscriptionTier::Pro);
    }

    // ================================================================
    // commit_ads (call_index 4)
    // ================================================================
    #[benchmark]
    fn commit_ads() {
        let bh = bot_hash(1);
        let ch = community_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_active_bot_with_operator(&caller, &bh);
        let ads = T::AdBasicThreshold::get();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bh, ch, ads);

        assert!(AdCommitments::<T>::contains_key(&bh));
    }

    // ================================================================
    // cancel_ad_commitment (call_index 5)
    // ================================================================
    #[benchmark]
    fn cancel_ad_commitment() {
        let bh = bot_hash(1);
        let ch = community_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        let ads = T::AdBasicThreshold::get();
        seed_ad_commitment::<T>(&bh, &caller, ch, ads, AdCommitmentStatus::Active);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bh);

        let rec = AdCommitments::<T>::get(&bh).unwrap();
        assert_eq!(rec.status, AdCommitmentStatus::Cancelled);
    }

    // ================================================================
    // cleanup_subscription (call_index 6)
    // ================================================================
    #[benchmark]
    fn cleanup_subscription() {
        let bh = bot_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        seed_subscription::<T>(
            &bh,
            &caller,
            SubscriptionTier::Basic,
            SubscriptionStatus::Cancelled,
        );

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bh);

        assert!(!Subscriptions::<T>::contains_key(&bh));
    }

    // ================================================================
    // cleanup_ad_commitment (call_index 7)
    // ================================================================
    #[benchmark]
    fn cleanup_ad_commitment() {
        let bh = bot_hash(1);
        let ch = community_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        let ads = T::AdBasicThreshold::get();
        seed_ad_commitment::<T>(&bh, &caller, ch, ads, AdCommitmentStatus::Cancelled);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bh);

        assert!(!AdCommitments::<T>::contains_key(&bh));
    }

    // ================================================================
    // update_tier_feature_gate (call_index 8) — Root
    // ================================================================
    #[benchmark]
    fn update_tier_feature_gate() {
        let gate = TierFeatureGate {
            max_rules: 100,
            log_retention_days: 90,
            forced_ads_per_day: 0,
            can_disable_ads: true,
            tee_access: true,
        };

        #[extrinsic_call]
        _(RawOrigin::Root, SubscriptionTier::Basic, gate);

        assert!(TierFeatureGateOverrides::<T>::contains_key(
            &SubscriptionTier::Basic
        ));
    }

    // ================================================================
    // force_cancel_subscription (call_index 9) — Root
    // ================================================================
    #[benchmark]
    fn force_cancel_subscription() {
        let bh = bot_hash(1);
        let owner: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::fund_account(&owner);
        let fee = T::BasicFeePerEra::get();
        let _ = T::Currency::reserve(&owner, fee);
        seed_subscription::<T>(
            &bh,
            &owner,
            SubscriptionTier::Basic,
            SubscriptionStatus::Active,
        );
        seed_escrow::<T>(&bh, fee);

        #[extrinsic_call]
        _(RawOrigin::Root, bh);

        let sub = Subscriptions::<T>::get(&bh).unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Cancelled);
    }

    // ================================================================
    // withdraw_escrow (call_index 10)
    // ================================================================
    #[benchmark]
    fn withdraw_escrow() {
        let bh = bot_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::fund_account(&caller);
        let fee = T::BasicFeePerEra::get();
        let big_escrow = fee.saturating_mul(5u32.into());
        let _ = T::Currency::reserve(&caller, big_escrow);
        seed_subscription::<T>(
            &bh,
            &caller,
            SubscriptionTier::Basic,
            SubscriptionStatus::Active,
        );
        seed_escrow::<T>(&bh, big_escrow);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bh, fee);
    }

    // ================================================================
    // update_ad_commitment (call_index 11)
    // ================================================================
    #[benchmark]
    fn update_ad_commitment() {
        let bh = bot_hash(1);
        let ch = community_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        let ads = T::AdBasicThreshold::get();
        seed_ad_commitment::<T>(&bh, &caller, ch, ads, AdCommitmentStatus::Active);
        let new_ads = T::AdProThreshold::get();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bh, new_ads, None);

        let rec = AdCommitments::<T>::get(&bh).unwrap();
        assert_eq!(rec.committed_ads_per_era, new_ads);
    }

    // ================================================================
    // force_suspend_subscription (call_index 12) — Root
    // ================================================================
    #[benchmark]
    fn force_suspend_subscription() {
        let bh = bot_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        seed_subscription::<T>(
            &bh,
            &caller,
            SubscriptionTier::Basic,
            SubscriptionStatus::Active,
        );

        #[extrinsic_call]
        _(RawOrigin::Root, bh);

        let sub = Subscriptions::<T>::get(&bh).unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Suspended);
    }

    // ================================================================
    // operator_deposit_subscription (call_index 13)
    // ================================================================
    #[benchmark]
    fn operator_deposit_subscription() {
        let bh = bot_hash(1);
        let owner: T::AccountId = account("owner", 0, 0);
        let operator: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_bot_operator(&operator, &bh);
        T::BenchmarkHelper::fund_account(&operator);
        seed_subscription::<T>(
            &bh,
            &owner,
            SubscriptionTier::Basic,
            SubscriptionStatus::Active,
        );
        seed_escrow::<T>(&bh, T::BasicFeePerEra::get());
        let amount: BalanceOf<T> = T::BasicFeePerEra::get();

        #[extrinsic_call]
        _(RawOrigin::Signed(operator), bh, amount);
    }

    // ================================================================
    // reset_tier_feature_gate (call_index 14) — Root
    // ================================================================
    #[benchmark]
    fn reset_tier_feature_gate() {
        TierFeatureGateOverrides::<T>::insert(
            &SubscriptionTier::Basic,
            TierFeatureGate {
                max_rules: 50,
                log_retention_days: 30,
                forced_ads_per_day: 0,
                can_disable_ads: false,
                tee_access: false,
            },
        );

        #[extrinsic_call]
        _(RawOrigin::Root, SubscriptionTier::Basic);

        assert!(!TierFeatureGateOverrides::<T>::contains_key(
            &SubscriptionTier::Basic
        ));
    }

    // ================================================================
    // force_change_tier (call_index 15) — Root
    // ================================================================
    #[benchmark]
    fn force_change_tier() {
        let bh = bot_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        seed_subscription::<T>(
            &bh,
            &caller,
            SubscriptionTier::Basic,
            SubscriptionStatus::Active,
        );

        #[extrinsic_call]
        _(RawOrigin::Root, bh, SubscriptionTier::Pro);

        let sub = Subscriptions::<T>::get(&bh).unwrap();
        assert_eq!(sub.tier, SubscriptionTier::Pro);
    }

    // ================================================================
    // pause_subscription (call_index 16)
    // ================================================================
    #[benchmark]
    fn pause_subscription() {
        let bh = bot_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        seed_subscription::<T>(
            &bh,
            &caller,
            SubscriptionTier::Basic,
            SubscriptionStatus::Active,
        );

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bh);

        let sub = Subscriptions::<T>::get(&bh).unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Paused);
    }

    // ================================================================
    // resume_subscription (call_index 17)
    // ================================================================
    #[benchmark]
    fn resume_subscription() {
        let bh = bot_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        seed_subscription::<T>(
            &bh,
            &caller,
            SubscriptionTier::Basic,
            SubscriptionStatus::Paused,
        );
        seed_escrow::<T>(&bh, T::BasicFeePerEra::get().saturating_mul(2u32.into()));

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bh);

        let sub = Subscriptions::<T>::get(&bh).unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Active);
    }

    // ================================================================
    // batch_cleanup (call_index 18)
    // ================================================================
    #[benchmark]
    fn batch_cleanup() {
        let caller: T::AccountId = whitelisted_caller();
        let mut sub_ids = alloc::vec::Vec::new();
        let mut ad_ids = alloc::vec::Vec::new();
        let ads = T::AdBasicThreshold::get();
        for i in 0u8..5 {
            let bh_s = bot_hash(10 + i);
            seed_subscription::<T>(
                &bh_s,
                &caller,
                SubscriptionTier::Basic,
                SubscriptionStatus::Cancelled,
            );
            sub_ids.push(bh_s);

            let bh_a = bot_hash(20 + i);
            let ch = community_hash(i);
            seed_ad_commitment::<T>(&bh_a, &caller, ch, ads, AdCommitmentStatus::Cancelled);
            ad_ids.push(bh_a);
        }

        let sub_bounded: sp_runtime::BoundedVec<_, frame_support::traits::ConstU32<50>> =
            sub_ids.try_into().unwrap();
        let ad_bounded: sp_runtime::BoundedVec<_, frame_support::traits::ConstU32<50>> =
            ad_ids.try_into().unwrap();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), sub_bounded, ad_bounded);

        assert!(!Subscriptions::<T>::contains_key(&bot_hash(10)));
        assert!(!AdCommitments::<T>::contains_key(&bot_hash(20)));
    }

    // ================================================================
    // update_tier_fee (call_index 19) — Root
    // ================================================================
    #[benchmark]
    fn update_tier_fee() {
        let new_fee: BalanceOf<T> = 999u32.into();

        #[extrinsic_call]
        _(RawOrigin::Root, SubscriptionTier::Basic, new_fee);

        assert_eq!(
            TierFeeOverrides::<T>::get(&SubscriptionTier::Basic),
            Some(new_fee)
        );
    }

    // ================================================================
    // force_cancel_ad_commitment (call_index 20) — Root
    // ================================================================
    #[benchmark]
    fn force_cancel_ad_commitment() {
        let bh = bot_hash(1);
        let ch = community_hash(1);
        let owner: T::AccountId = whitelisted_caller();
        let ads = T::AdBasicThreshold::get();
        seed_ad_commitment::<T>(&bh, &owner, ch, ads, AdCommitmentStatus::Active);

        #[extrinsic_call]
        _(RawOrigin::Root, bh);

        let rec = AdCommitments::<T>::get(&bh).unwrap();
        assert_eq!(rec.status, AdCommitmentStatus::Cancelled);
    }

    impl_benchmark_test_suite!(
        GroupRobotSubscription,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
