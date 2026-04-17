//! Calibrated weights for `pallet_grouprobot_subscription`
//!
//! THIS FILE WAS CALIBRATED BY AUDITING EVERY EXTRINSIC'S STORAGE ACCESS PATHS.
//! DATE: 2026-03-08
//!
//! Weight methodology:
//! - reads/writes: counted by tracing every StorageMap/StorageValue access in worst-case path
//! - ref_time: base compute + Currency operations (~10M each)
//! - proof_size: sum of MaxEncodedLen for all accessed storage items
//!
//! Run `cargo run --release --features runtime-benchmarks -- benchmark pallet`
//! to regenerate with actual hardware measurements.

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

pub trait WeightInfo {
    fn subscribe() -> Weight;
    fn deposit_subscription() -> Weight;
    fn cancel_subscription() -> Weight;
    fn change_tier() -> Weight;
    fn commit_ads() -> Weight;
    fn cancel_ad_commitment() -> Weight;
    fn cleanup_subscription() -> Weight;
    fn cleanup_ad_commitment() -> Weight;
    fn update_tier_feature_gate() -> Weight;
    fn force_cancel_subscription() -> Weight;
    fn withdraw_escrow() -> Weight;
    fn update_ad_commitment() -> Weight;
    fn force_suspend_subscription() -> Weight;
    fn operator_deposit_subscription() -> Weight;
    fn reset_tier_feature_gate() -> Weight;
    fn force_change_tier() -> Weight;
    fn pause_subscription() -> Weight;
    fn resume_subscription() -> Weight;
    fn batch_cleanup() -> Weight;
    fn update_tier_fee() -> Weight;
    fn force_cancel_ad_commitment() -> Weight;
}

/// Calibrated weight functions for `pallet_grouprobot_subscription`.
///
/// Per-extrinsic breakdown documents every storage access in worst-case path.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    // subscribe:
    //   R=BotRegistry::is_bot_active + R=BotRegistry::bot_owner + R=Subscriptions(contains_key)
    //     + R=BotRegistry::bot_operator + R=TierFeeOverrides(get)
    //   W=Currency::reserve + W=SubscriptionEscrow(insert) + W=Subscriptions(insert)
    fn subscribe() -> Weight {
        Weight::from_parts(55_000_000, 10_000)
            .saturating_add(T::DbWeight::get().reads(5))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    // deposit_subscription:
    //   R=Subscriptions(get) + R=SubscriptionEscrow(get+mutate)
    //   W=Currency::reserve + W=SubscriptionEscrow(mutate) + W=Subscriptions(mutate) [reactivation]
    fn deposit_subscription() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    // cancel_subscription:
    //   R=Subscriptions(try_mutate) + R=SubscriptionEscrow(take) + R=EraStartBlockProvider
    //     + R=EraLength + R=frame_system::block_number
    //   W=Subscriptions(mutate) + W=SubscriptionEscrow(take) + W=Currency::unreserve
    //     + W=Currency::transfer (prorated fee to treasury)
    fn cancel_subscription() -> Weight {
        Weight::from_parts(55_000_000, 10_000)
            .saturating_add(T::DbWeight::get().reads(5))
            .saturating_add(T::DbWeight::get().writes(4))
    }

    // change_tier:
    //   R=Subscriptions(try_mutate) + R=TierFeeOverrides(get) + R=SubscriptionEscrow(get)
    //   W=Subscriptions(mutate)
    fn change_tier() -> Weight {
        Weight::from_parts(35_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // commit_ads:
    //   R=BotRegistry::is_bot_active + R=BotRegistry::bot_owner + R=AdCommitments(contains_key)
    //     + R=BotRegistry::bot_operator
    //   W=AdCommitments(insert)
    fn commit_ads() -> Weight {
        Weight::from_parts(45_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // cancel_ad_commitment:
    //   R=AdCommitments(try_mutate)
    //   W=AdCommitments(mutate)
    fn cancel_ad_commitment() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // cleanup_subscription:
    //   R=Subscriptions(get)
    //   W=Subscriptions(remove) + W=SubscriptionEscrow(remove)
    fn cleanup_subscription() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    // cleanup_ad_commitment:
    //   R=AdCommitments(get)
    //   W=AdCommitments(remove)
    fn cleanup_ad_commitment() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // update_tier_feature_gate:
    //   W=TierFeatureGateOverrides(insert)
    fn update_tier_feature_gate() -> Weight {
        Weight::from_parts(15_000_000, 3_000)
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // force_cancel_subscription:
    //   R=Subscriptions(get) + R=SubscriptionEscrow(take)
    //   W=SubscriptionEscrow(take) + W=Currency::unreserve + W=Currency::transfer
    //     + W=Subscriptions(mutate)
    fn force_cancel_subscription() -> Weight {
        Weight::from_parts(55_000_000, 10_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(4))
    }

    // withdraw_escrow:
    //   R=Subscriptions(get) + R=SubscriptionEscrow(get)
    //   W=Currency::unreserve + W=SubscriptionEscrow(insert)
    fn withdraw_escrow() -> Weight {
        Weight::from_parts(35_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    // update_ad_commitment:
    //   R=AdCommitments(try_mutate)
    //   W=AdCommitments(mutate)
    fn update_ad_commitment() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // force_suspend_subscription:
    //   R=Subscriptions(try_mutate)
    //   W=Subscriptions(mutate)
    fn force_suspend_subscription() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // operator_deposit_subscription:
    //   R=Subscriptions(get) + R=BotRegistry::bot_operator + R=SubscriptionEscrow(get+mutate)
    //   W=Currency::reserve + W=SubscriptionEscrow(mutate) + W=Subscriptions(mutate) [reactivation]
    fn operator_deposit_subscription() -> Weight {
        Weight::from_parts(45_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    // reset_tier_feature_gate:
    //   W=TierFeatureGateOverrides(remove)
    fn reset_tier_feature_gate() -> Weight {
        Weight::from_parts(15_000_000, 3_000)
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // force_change_tier:
    //   R=Subscriptions(try_mutate) + R=TierFeeOverrides(get)
    //   W=Subscriptions(mutate)
    fn force_change_tier() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // pause_subscription:
    //   R=Subscriptions(try_mutate)
    //   W=Subscriptions(mutate)
    fn pause_subscription() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // resume_subscription:
    //   R=Subscriptions(try_mutate) + R=SubscriptionEscrow(get)
    //   W=Subscriptions(mutate)
    fn resume_subscription() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // batch_cleanup: worst case = 50 subs + 50 ad_commitments
    //   R=Subscriptions(get) × 50 + R=AdCommitments(get) × 50
    //   W=Subscriptions(remove) × 50 + W=SubscriptionEscrow(remove) × 50
    //     + W=AdCommitments(remove) × 50
    fn batch_cleanup() -> Weight {
        Weight::from_parts(200_000_000, 50_000)
            .saturating_add(T::DbWeight::get().reads(100))
            .saturating_add(T::DbWeight::get().writes(150))
    }

    // update_tier_fee:
    //   W=TierFeeOverrides(insert)
    fn update_tier_fee() -> Weight {
        Weight::from_parts(15_000_000, 3_000)
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // force_cancel_ad_commitment:
    //   R=AdCommitments(try_mutate)
    //   W=AdCommitments(mutate)
    fn force_cancel_ad_commitment() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Default weight implementation (unit tests / fallback).
impl WeightInfo for () {
    fn subscribe() -> Weight { Weight::from_parts(55_000_000, 10_000) }
    fn deposit_subscription() -> Weight { Weight::from_parts(40_000_000, 6_000) }
    fn cancel_subscription() -> Weight { Weight::from_parts(55_000_000, 10_000) }
    fn change_tier() -> Weight { Weight::from_parts(35_000_000, 6_000) }
    fn commit_ads() -> Weight { Weight::from_parts(45_000_000, 8_000) }
    fn cancel_ad_commitment() -> Weight { Weight::from_parts(30_000_000, 5_000) }
    fn cleanup_subscription() -> Weight { Weight::from_parts(25_000_000, 4_000) }
    fn cleanup_ad_commitment() -> Weight { Weight::from_parts(25_000_000, 4_000) }
    fn update_tier_feature_gate() -> Weight { Weight::from_parts(15_000_000, 3_000) }
    fn force_cancel_subscription() -> Weight { Weight::from_parts(55_000_000, 10_000) }
    fn withdraw_escrow() -> Weight { Weight::from_parts(35_000_000, 6_000) }
    fn update_ad_commitment() -> Weight { Weight::from_parts(40_000_000, 6_000) }
    fn force_suspend_subscription() -> Weight { Weight::from_parts(25_000_000, 4_000) }
    fn operator_deposit_subscription() -> Weight { Weight::from_parts(45_000_000, 8_000) }
    fn reset_tier_feature_gate() -> Weight { Weight::from_parts(15_000_000, 3_000) }
    fn force_change_tier() -> Weight { Weight::from_parts(30_000_000, 5_000) }
    fn pause_subscription() -> Weight { Weight::from_parts(25_000_000, 4_000) }
    fn resume_subscription() -> Weight { Weight::from_parts(30_000_000, 5_000) }
    fn batch_cleanup() -> Weight { Weight::from_parts(200_000_000, 50_000) }
    fn update_tier_fee() -> Weight { Weight::from_parts(15_000_000, 3_000) }
    fn force_cancel_ad_commitment() -> Weight { Weight::from_parts(25_000_000, 4_000) }
}
