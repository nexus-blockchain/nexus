//! Benchmarking for pallet-commission-referral.
//!
//! Uses `frame_benchmarking::v2` macro style.
//! Root extrinsics are fully benchmarked; signed extrinsics use force_* variants
//! as proxy since they share the same storage write path. The additional overhead
//! of signed extrinsics (3 reads for owner/locked/active checks) is accounted for
//! in the WeightInfo hand-estimates.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use pallet::*;

fn seed_config<T: Config>(entity_id: u64) {
    ReferralConfigs::<T>::insert(
        entity_id,
        ReferralConfig {
            direct_reward: DirectRewardConfig { rate: 500 },
            fixed_amount: FixedAmountConfig { amount: 1000u32.into() },
            first_order: FirstOrderConfig { amount: 500u32.into(), rate: 300, use_amount: true },
            repeat_purchase: RepeatPurchaseConfig { rate: 200, min_orders: 3 },
        },
    );
    ReferrerGuardConfigs::<T>::insert(entity_id, ReferrerGuardConfig {
        min_referrer_spent: 1000,
        min_referrer_orders: 5,
    });
    CommissionCapConfigs::<T>::insert(entity_id, CommissionCapConfig {
        max_per_order: 500u32.into(),
        max_total_earned: 10000u32.into(),
    });
    ReferralValidityConfigs::<T>::insert(entity_id, ReferralValidityConfig {
        validity_blocks: 1000,
        valid_orders: 10,
    });
    ConfigEffectiveAfter::<T>::insert(entity_id, frame_system::Pallet::<T>::block_number());
}

#[benchmarks]
mod benches {
    use super::*;

    // ===== Root force_* extrinsics =====

    #[benchmark]
    fn force_set_direct_reward_config() {
        let entity_id: u64 = 9999;

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, 500u16);

        assert!(ReferralConfigs::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn force_set_fixed_amount_config() {
        let entity_id: u64 = 9999;

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, 1000u32.into());

        assert!(ReferralConfigs::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn force_set_first_order_config() {
        let entity_id: u64 = 9999;

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, 500u32.into(), 300u16, true);

        assert!(ReferralConfigs::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn force_set_repeat_purchase_config() {
        let entity_id: u64 = 9999;

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, 200u16, 3u32);

        assert!(ReferralConfigs::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn force_clear_referral_config() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        assert!(ReferralConfigs::<T>::get(entity_id).is_none());
    }

    // ===== Signed extrinsics — proxy via force_* variants =====
    // Signed extrinsics add 3 reads (entity_owner, entity_locked, entity_active)
    // on top of the storage write. WeightInfo accounts for this overhead.

    #[benchmark]
    fn set_direct_reward_config() {
        let entity_id: u64 = 9999;

        #[extrinsic_call]
        force_set_direct_reward_config(RawOrigin::Root, entity_id, 800u16);

        let config = ReferralConfigs::<T>::get(entity_id).unwrap();
        assert_eq!(config.direct_reward.rate, 800);
    }

    #[benchmark]
    fn set_fixed_amount_config() {
        let entity_id: u64 = 9999;

        #[extrinsic_call]
        force_set_fixed_amount_config(RawOrigin::Root, entity_id, 2000u32.into());

        assert!(ReferralConfigs::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn set_first_order_config() {
        let entity_id: u64 = 9999;

        #[extrinsic_call]
        force_set_first_order_config(RawOrigin::Root, entity_id, 500u32.into(), 300u16, false);

        assert!(ReferralConfigs::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn set_repeat_purchase_config() {
        let entity_id: u64 = 9999;

        #[extrinsic_call]
        force_set_repeat_purchase_config(RawOrigin::Root, entity_id, 300u16, 5u32);

        assert!(ReferralConfigs::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn clear_referral_config() {
        let entity_id: u64 = 9999;
        seed_config::<T>(entity_id);

        #[extrinsic_call]
        force_clear_referral_config(RawOrigin::Root, entity_id);

        assert!(ReferralConfigs::<T>::get(entity_id).is_none());
    }

    // ===== F1/F2/F5/F3 附加配置 — 直接写 storage 测量 =====

    #[benchmark]
    fn set_referrer_guard_config() {
        let entity_id: u64 = 9999;

        #[block]
        {
            ReferrerGuardConfigs::<T>::insert(entity_id, ReferrerGuardConfig {
                min_referrer_spent: 5000,
                min_referrer_orders: 3,
            });
        }

        assert!(ReferrerGuardConfigs::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn set_commission_cap_config() {
        let entity_id: u64 = 9999;

        #[block]
        {
            CommissionCapConfigs::<T>::insert(entity_id, CommissionCapConfig::<BalanceOf<T>> {
                max_per_order: 500u32.into(),
                max_total_earned: 10000u32.into(),
            });
        }

        assert!(CommissionCapConfigs::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn set_referral_validity_config() {
        let entity_id: u64 = 9999;

        #[block]
        {
            ReferralValidityConfigs::<T>::insert(entity_id, ReferralValidityConfig {
                validity_blocks: 1000,
                valid_orders: 10,
            });
        }

        assert!(ReferralValidityConfigs::<T>::get(entity_id).is_some());
    }

    #[benchmark]
    fn set_config_effective_after() {
        let entity_id: u64 = 9999;
        let block = frame_system::Pallet::<T>::block_number();

        #[block]
        {
            ConfigEffectiveAfter::<T>::insert(entity_id, block);
        }

        assert!(ConfigEffectiveAfter::<T>::get(entity_id).is_some());
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
