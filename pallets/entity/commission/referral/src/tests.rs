use crate::mock::*;
use crate::pallet;
use frame_support::{assert_noop, assert_ok};
use pallet_commission_common::{
    CommissionModes, CommissionPlugin, CommissionType, ReferralPlanWriter,
};

const ENTITY_ID: u64 = 1;
const OWNER: u64 = 100;
const ADMIN: u64 = 101;
const NON_OWNER: u64 = 102;

/// 设置 Entity Owner (用于 extrinsic 权限测试)
fn setup_entity() {
    set_entity_owner(ENTITY_ID, OWNER);
}

// ============================================================================
// Extrinsic tests — set_direct_reward_config
// ============================================================================

#[test]
fn set_direct_reward_config_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_ok!(CommissionReferral::set_direct_reward_config(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 500,
        ));
        let config = pallet::ReferralConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.direct_reward.rate, 500);
    });
}

#[test]
fn set_direct_reward_config_rejects_invalid_rate() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_noop!(
            CommissionReferral::set_direct_reward_config(RuntimeOrigin::signed(OWNER), ENTITY_ID, 10001),
            pallet::Error::<Test>::InvalidRate
        );
    });
}

#[test]
fn set_direct_reward_config_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_noop!(
            CommissionReferral::set_direct_reward_config(RuntimeOrigin::signed(NON_OWNER), ENTITY_ID, 500),
            pallet::Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

// ============================================================================
// Extrinsic tests — set_fixed_amount_config
// ============================================================================

#[test]
fn set_fixed_amount_config_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_ok!(CommissionReferral::set_fixed_amount_config(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 1000,
        ));
        let config = pallet::ReferralConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.fixed_amount.amount, 1000);
    });
}

// ============================================================================
// Extrinsic tests — set_first_order_config
// ============================================================================

#[test]
fn set_first_order_config_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_ok!(CommissionReferral::set_first_order_config(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 500, 300, true,
        ));
        let config = pallet::ReferralConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.first_order.amount, 500);
        assert_eq!(config.first_order.rate, 300);
        assert!(config.first_order.use_amount);
    });
}

#[test]
fn set_first_order_config_rejects_invalid_rate() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_noop!(
            CommissionReferral::set_first_order_config(RuntimeOrigin::signed(OWNER), ENTITY_ID, 500, 10001, false),
            pallet::Error::<Test>::InvalidRate
        );
    });
}

// ============================================================================
// Extrinsic tests — set_repeat_purchase_config
// ============================================================================

#[test]
fn set_repeat_purchase_config_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_ok!(CommissionReferral::set_repeat_purchase_config(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 200, 3,
        ));
        let config = pallet::ReferralConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.repeat_purchase.rate, 200);
        assert_eq!(config.repeat_purchase.min_orders, 3);
    });
}

#[test]
fn set_repeat_purchase_config_rejects_invalid_rate() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_noop!(
            CommissionReferral::set_repeat_purchase_config(RuntimeOrigin::signed(OWNER), ENTITY_ID, 10001, 3),
            pallet::Error::<Test>::InvalidRate
        );
    });
}

// ============================================================================
// CommissionPlugin — direct reward
// ============================================================================

#[test]
fn direct_reward_basic() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        // buyer=50, referrer=40
        setup_chain(1, 50, &[40]);

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            direct_reward: pallet::DirectRewardConfig { rate: 1000 }, // 10%
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::DIRECT_REWARD);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 1000); // 10000 * 10%
        assert_eq!(outputs[0].commission_type, CommissionType::DirectReward);
        assert_eq!(remaining, 9000);
    });
}

#[test]
fn direct_reward_no_referrer() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        // buyer=50, no referrer

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            direct_reward: pallet::DirectRewardConfig { rate: 1000 },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::DIRECT_REWARD);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

#[test]
fn direct_reward_zero_rate_skips() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            direct_reward: pallet::DirectRewardConfig { rate: 0 },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::DIRECT_REWARD);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

#[test]
fn direct_reward_capped_by_remaining() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            direct_reward: pallet::DirectRewardConfig { rate: 5000 }, // 50%
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::DIRECT_REWARD);
        // remaining=100, order=10000, 50% of 10000 = 5000 but capped at 100
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 100, modes, false, 0,
        );

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].amount, 100);
        assert_eq!(remaining, 0);
    });
}

// ============================================================================
// CommissionPlugin — fixed amount
// ============================================================================

#[test]
fn fixed_amount_basic() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            fixed_amount: pallet::FixedAmountConfig { amount: 500 },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::FIXED_AMOUNT);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].amount, 500);
        assert_eq!(outputs[0].commission_type, CommissionType::FixedAmount);
        assert_eq!(remaining, 9500);
    });
}

#[test]
fn fixed_amount_zero_skips() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            fixed_amount: pallet::FixedAmountConfig { amount: 0 },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::FIXED_AMOUNT);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

// ============================================================================
// CommissionPlugin — first order
// ============================================================================

#[test]
fn first_order_by_amount() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            first_order: pallet::FirstOrderConfig { amount: 800, rate: 0, use_amount: true },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::FIRST_ORDER);
        // is_first_order = true
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, true, 0,
        );

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].amount, 800);
        assert_eq!(outputs[0].commission_type, CommissionType::FirstOrder);
        assert_eq!(remaining, 9200);
    });
}

#[test]
fn first_order_by_rate() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            first_order: pallet::FirstOrderConfig { amount: 0, rate: 500, use_amount: false }, // 5%
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::FIRST_ORDER);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, true, 0,
        );

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].amount, 500); // 10000 * 5%
        assert_eq!(remaining, 9500);
    });
}

#[test]
fn first_order_not_triggered_when_not_first() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            first_order: pallet::FirstOrderConfig { amount: 800, rate: 0, use_amount: true },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::FIRST_ORDER);
        // is_first_order = false
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

#[test]
fn h3_first_order_zero_amount_early_return() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);

        // use_amount=true, amount=0 → 早返回
        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            first_order: pallet::FirstOrderConfig { amount: 0, rate: 500, use_amount: true },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::FIRST_ORDER);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, true, 0,
        );

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

#[test]
fn h3_first_order_zero_rate_early_return() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);

        // use_amount=false, rate=0 → 早返回
        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            first_order: pallet::FirstOrderConfig { amount: 500, rate: 0, use_amount: false },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::FIRST_ORDER);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, true, 0,
        );

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

// ============================================================================
// CommissionPlugin — repeat purchase
// ============================================================================

#[test]
fn repeat_purchase_basic() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            repeat_purchase: pallet::RepeatPurchaseConfig { rate: 300, min_orders: 3 }, // 3%
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::REPEAT_PURCHASE);
        // buyer_order_count=5 >= min_orders=3
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 5,
        );

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].amount, 300); // 10000 * 3%
        assert_eq!(outputs[0].commission_type, CommissionType::RepeatPurchase);
        assert_eq!(remaining, 9700);
    });
}

#[test]
fn repeat_purchase_below_min_orders() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            repeat_purchase: pallet::RepeatPurchaseConfig { rate: 300, min_orders: 3 },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::REPEAT_PURCHASE);
        // buyer_order_count=2 < min_orders=3
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 2,
        );

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

// ============================================================================
// CommissionPlugin — mode not enabled
// ============================================================================

#[test]
fn mode_not_enabled_returns_empty() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            direct_reward: pallet::DirectRewardConfig { rate: 1000 },
            ..Default::default()
        });

        // FIXED_AMOUNT mode, but config only has direct_reward
        let modes = CommissionModes(CommissionModes::FIXED_AMOUNT);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

#[test]
fn no_config_returns_empty() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        let modes = CommissionModes(CommissionModes::DIRECT_REWARD);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

// ============================================================================
// ReferralPlanWriter tests
// ============================================================================

#[test]
fn plan_writer_set_direct_rate() {
    new_test_ext().execute_with(|| {
        assert_ok!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::set_direct_rate(1, 500));
        let config = pallet::ReferralConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.direct_reward.rate, 500);
    });
}

#[test]
fn h2_plan_writer_rejects_invalid_direct_rate() {
    new_test_ext().execute_with(|| {
        assert!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::set_direct_rate(1, 10001).is_err());
    });
}

#[test]
fn h2_plan_writer_rejects_invalid_first_order_rate() {
    new_test_ext().execute_with(|| {
        assert!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::set_first_order(
            1, 100, 10001, false,
        ).is_err());
    });
}

#[test]
fn h2_plan_writer_rejects_invalid_repeat_purchase_rate() {
    new_test_ext().execute_with(|| {
        assert!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::set_repeat_purchase(
            1, 10001, 3,
        ).is_err());
    });
}

#[test]
fn plan_writer_clear_config() {
    new_test_ext().execute_with(|| {
        assert_ok!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::set_direct_rate(1, 500));
        assert!(pallet::ReferralConfigs::<Test>::get(1).is_some());

        assert_ok!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::clear_config(1));
        assert!(pallet::ReferralConfigs::<Test>::get(1).is_none());
    });
}

// ============================================================================
// M1 regression: PlanWriter emits events
// ============================================================================

#[test]
fn m1_plan_writer_set_direct_rate_emits_event() {
    new_test_ext().execute_with(|| {
        System::reset_events();
        assert_ok!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::set_direct_rate(1, 500));
        System::assert_last_event(
            pallet::Event::<Test>::ReferralConfigUpdated { entity_id: 1 }.into(),
        );
    });
}

#[test]
fn m1_plan_writer_set_fixed_amount_emits_event() {
    new_test_ext().execute_with(|| {
        System::reset_events();
        assert_ok!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::set_fixed_amount(1, 1000));
        System::assert_last_event(
            pallet::Event::<Test>::ReferralConfigUpdated { entity_id: 1 }.into(),
        );
    });
}

#[test]
fn m1_plan_writer_set_first_order_emits_event() {
    new_test_ext().execute_with(|| {
        System::reset_events();
        assert_ok!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::set_first_order(1, 500, 300, true));
        System::assert_last_event(
            pallet::Event::<Test>::ReferralConfigUpdated { entity_id: 1 }.into(),
        );
    });
}

#[test]
fn m1_plan_writer_set_repeat_purchase_emits_event() {
    new_test_ext().execute_with(|| {
        System::reset_events();
        assert_ok!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::set_repeat_purchase(1, 200, 3));
        System::assert_last_event(
            pallet::Event::<Test>::ReferralConfigUpdated { entity_id: 1 }.into(),
        );
    });
}

#[test]
fn m1_plan_writer_clear_config_emits_cleared_event() {
    new_test_ext().execute_with(|| {
        assert_ok!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::set_direct_rate(1, 500));
        System::reset_events();
        assert_ok!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::clear_config(1));
        System::assert_last_event(
            pallet::Event::<Test>::ReferralConfigCleared { entity_id: 1 }.into(),
        );
    });
}

// ============================================================================
// X1 regression: is_banned skips banned referrer
// ============================================================================

#[test]
fn x1_direct_reward_skips_banned_referrer() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);
        ban_member(1, 40);

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            direct_reward: pallet::DirectRewardConfig { rate: 1000 },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::DIRECT_REWARD);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

#[test]
fn x1_fixed_amount_skips_banned_referrer() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);
        ban_member(1, 40);

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            fixed_amount: pallet::FixedAmountConfig { amount: 500 },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::FIXED_AMOUNT);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

#[test]
fn x1_first_order_skips_banned_referrer() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);
        ban_member(1, 40);

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            first_order: pallet::FirstOrderConfig { amount: 800, rate: 0, use_amount: true },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::FIRST_ORDER);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, true, 0,
        );

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

#[test]
fn x1_repeat_purchase_skips_banned_referrer() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);
        ban_member(1, 40);

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            repeat_purchase: pallet::RepeatPurchaseConfig { rate: 300, min_orders: 3 },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::REPEAT_PURCHASE);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 5,
        );

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

#[test]
fn x1_non_banned_referrer_still_receives_commission() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);
        // ban a different member, not the referrer
        ban_member(1, 99);

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            direct_reward: pallet::DirectRewardConfig { rate: 1000 },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::DIRECT_REWARD);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(remaining, 9000);
    });
}

// ============================================================================
// X2 regression: phantom event guard
// ============================================================================

#[test]
fn x2_plan_writer_clear_no_phantom_event_when_config_absent() {
    new_test_ext().execute_with(|| {
        System::reset_events();
        // clear on non-existent config — should NOT emit event
        assert_ok!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::clear_config(999));
        assert!(System::events().is_empty());
    });
}

#[test]
fn x2_plan_writer_clear_emits_event_when_config_exists() {
    new_test_ext().execute_with(|| {
        assert_ok!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::set_direct_rate(1, 500));
        System::reset_events();
        assert_ok!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::clear_config(1));
        System::assert_last_event(
            pallet::Event::<Test>::ReferralConfigCleared { entity_id: 1 }.into(),
        );
        assert!(pallet::ReferralConfigs::<Test>::get(1).is_none());
    });
}

#[test]
fn x2_force_clear_no_phantom_event() {
    new_test_ext().execute_with(|| {
        System::reset_events();
        assert_ok!(CommissionReferral::force_clear_referral_config(
            RuntimeOrigin::root(), 999,
        ));
        assert!(System::events().is_empty());
    });
}

#[test]
fn x2_force_clear_emits_event_when_config_exists() {
    new_test_ext().execute_with(|| {
        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig::<Balance>::default());
        System::reset_events();
        assert_ok!(CommissionReferral::force_clear_referral_config(
            RuntimeOrigin::root(), 1,
        ));
        System::assert_last_event(
            pallet::Event::<Test>::ReferralConfigCleared { entity_id: 1 }.into(),
        );
    });
}

// ============================================================================
// X3 regression: Owner/Admin permission + force_* Root
// ============================================================================

#[test]
fn x3_admin_with_commission_manage_can_set_config() {
    new_test_ext().execute_with(|| {
        setup_entity();
        set_entity_admin(ENTITY_ID, ADMIN, pallet_entity_common::AdminPermission::COMMISSION_MANAGE);
        assert_ok!(CommissionReferral::set_direct_reward_config(
            RuntimeOrigin::signed(ADMIN), ENTITY_ID, 800,
        ));
        let config = pallet::ReferralConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.direct_reward.rate, 800);
    });
}

#[test]
fn x3_admin_without_commission_manage_rejected() {
    new_test_ext().execute_with(|| {
        setup_entity();
        // Admin has SHOP_MANAGE but not COMMISSION_MANAGE
        set_entity_admin(ENTITY_ID, ADMIN, pallet_entity_common::AdminPermission::SHOP_MANAGE);
        assert_noop!(
            CommissionReferral::set_direct_reward_config(RuntimeOrigin::signed(ADMIN), ENTITY_ID, 800),
            pallet::Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

#[test]
fn x3_entity_not_found_rejected() {
    new_test_ext().execute_with(|| {
        // No entity setup → entity_owner returns None
        assert_noop!(
            CommissionReferral::set_direct_reward_config(RuntimeOrigin::signed(OWNER), 999, 500),
            pallet::Error::<Test>::EntityNotFound
        );
    });
}

#[test]
fn x3_force_set_direct_reward_works_for_root() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionReferral::force_set_direct_reward_config(
            RuntimeOrigin::root(), ENTITY_ID, 500,
        ));
        let config = pallet::ReferralConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.direct_reward.rate, 500);
    });
}

#[test]
fn x3_force_set_rejects_non_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionReferral::force_set_direct_reward_config(RuntimeOrigin::signed(OWNER), ENTITY_ID, 500),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn x3_force_set_fixed_amount_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionReferral::force_set_fixed_amount_config(
            RuntimeOrigin::root(), ENTITY_ID, 2000,
        ));
        let config = pallet::ReferralConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.fixed_amount.amount, 2000);
    });
}

#[test]
fn x3_force_set_first_order_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionReferral::force_set_first_order_config(
            RuntimeOrigin::root(), ENTITY_ID, 100, 500, false,
        ));
        let config = pallet::ReferralConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.first_order.amount, 100);
        assert_eq!(config.first_order.rate, 500);
        assert!(!config.first_order.use_amount);
    });
}

#[test]
fn x3_force_set_repeat_purchase_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionReferral::force_set_repeat_purchase_config(
            RuntimeOrigin::root(), ENTITY_ID, 300, 5,
        ));
        let config = pallet::ReferralConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.repeat_purchase.rate, 300);
        assert_eq!(config.repeat_purchase.min_orders, 5);
    });
}

#[test]
fn x3_clear_referral_config_by_owner() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_ok!(CommissionReferral::set_direct_reward_config(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 500,
        ));
        assert!(pallet::ReferralConfigs::<Test>::get(ENTITY_ID).is_some());

        assert_ok!(CommissionReferral::clear_referral_config(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
        ));
        assert!(pallet::ReferralConfigs::<Test>::get(ENTITY_ID).is_none());
    });
}

#[test]
fn x3_clear_referral_config_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        setup_entity();
        pallet::ReferralConfigs::<Test>::insert(ENTITY_ID, pallet::ReferralConfig::<Balance>::default());
        assert_noop!(
            CommissionReferral::clear_referral_config(RuntimeOrigin::signed(NON_OWNER), ENTITY_ID),
            pallet::Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

#[test]
fn x3_clear_referral_config_rejects_absent() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_noop!(
            CommissionReferral::clear_referral_config(RuntimeOrigin::signed(OWNER), ENTITY_ID),
            pallet::Error::<Test>::ConfigNotFound
        );
    });
}

// ==================== EntityLocked 回归测试 ====================

#[test]
fn entity_locked_rejects_set_direct_reward_config() {
    new_test_ext().execute_with(|| {
        setup_entity();
        set_entity_locked(ENTITY_ID);
        assert_noop!(
            CommissionReferral::set_direct_reward_config(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, 1000,
            ),
            pallet::Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_clear_referral_config() {
    new_test_ext().execute_with(|| {
        setup_entity();
        // 先设置配置
        assert_ok!(CommissionReferral::set_direct_reward_config(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 1000,
        ));
        // 锁定后无法清除
        set_entity_locked(ENTITY_ID);
        assert_noop!(
            CommissionReferral::clear_referral_config(RuntimeOrigin::signed(OWNER), ENTITY_ID),
            pallet::Error::<Test>::EntityLocked
        );
    });
}
