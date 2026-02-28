use crate::mock::*;
use crate::pallet;
use frame_support::{assert_noop, assert_ok};
use pallet_commission_common::{
    CommissionModes, CommissionPlugin, CommissionType, ReferralPlanWriter,
};

// ============================================================================
// Extrinsic tests — set_direct_reward_config
// ============================================================================

#[test]
fn set_direct_reward_config_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionReferral::set_direct_reward_config(
            RuntimeOrigin::root(), 1, 500,
        ));
        let config = pallet::ReferralConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.direct_reward.rate, 500);
    });
}

#[test]
fn set_direct_reward_config_rejects_invalid_rate() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionReferral::set_direct_reward_config(RuntimeOrigin::root(), 1, 10001),
            pallet::Error::<Test>::InvalidRate
        );
    });
}

#[test]
fn set_direct_reward_config_requires_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionReferral::set_direct_reward_config(RuntimeOrigin::signed(1), 1, 500),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ============================================================================
// Extrinsic tests — set_multi_level_config
// ============================================================================

#[test]
fn set_multi_level_config_works() {
    new_test_ext().execute_with(|| {
        let tiers = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 500, required_directs: 3, required_team_size: 0, required_spent: 0 },
        ];
        assert_ok!(CommissionReferral::set_multi_level_config(
            RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 2000,
        ));
        let config = pallet::ReferralConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.multi_level.levels.len(), 2);
        assert_eq!(config.multi_level.max_total_rate, 2000);
    });
}

#[test]
fn set_multi_level_config_rejects_invalid_rate() {
    new_test_ext().execute_with(|| {
        let tiers = vec![
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        assert_noop!(
            CommissionReferral::set_multi_level_config(RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10001),
            pallet::Error::<Test>::InvalidRate
        );
    });
}

// ============================================================================
// Extrinsic tests — set_fixed_amount_config
// ============================================================================

#[test]
fn set_fixed_amount_config_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionReferral::set_fixed_amount_config(
            RuntimeOrigin::root(), 1, 1000,
        ));
        let config = pallet::ReferralConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.fixed_amount.amount, 1000);
    });
}

// ============================================================================
// Extrinsic tests — set_first_order_config
// ============================================================================

#[test]
fn set_first_order_config_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionReferral::set_first_order_config(
            RuntimeOrigin::root(), 1, 500, 300, true,
        ));
        let config = pallet::ReferralConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.first_order.amount, 500);
        assert_eq!(config.first_order.rate, 300);
        assert!(config.first_order.use_amount);
    });
}

#[test]
fn set_first_order_config_rejects_invalid_rate() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionReferral::set_first_order_config(RuntimeOrigin::root(), 1, 500, 10001, false),
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
        assert_ok!(CommissionReferral::set_repeat_purchase_config(
            RuntimeOrigin::root(), 1, 200, 3,
        ));
        let config = pallet::ReferralConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.repeat_purchase.rate, 200);
        assert_eq!(config.repeat_purchase.min_orders, 3);
    });
}

#[test]
fn set_repeat_purchase_config_rejects_invalid_rate() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionReferral::set_repeat_purchase_config(RuntimeOrigin::root(), 1, 10001, 3),
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
// CommissionPlugin — multi level
// ============================================================================

#[test]
fn multi_level_basic() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        // buyer=50 -> 40 -> 30 -> 20
        setup_chain(1, 50, &[40, 30, 20]);

        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 }, // L1: 10%
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },  // L2: 5%
            pallet::MultiLevelTier { rate: 200, required_directs: 0, required_team_size: 0, required_spent: 0 },  // L3: 2%
        ];

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            multi_level: pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
                max_total_rate: 3000, // 30%
            },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 1000);
        assert_eq!(outputs[0].level, 1);
        assert_eq!(outputs[1].beneficiary, 30);
        assert_eq!(outputs[1].amount, 500);
        assert_eq!(outputs[1].level, 2);
        assert_eq!(outputs[2].beneficiary, 20);
        assert_eq!(outputs[2].amount, 200);
        assert_eq!(outputs[2].level, 3);
        assert_eq!(remaining, 8300);
    });
}

#[test]
fn multi_level_max_total_rate_caps() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40, 30]);

        let levels = vec![
            pallet::MultiLevelTier { rate: 2000, required_directs: 0, required_team_size: 0, required_spent: 0 }, // 20%
            pallet::MultiLevelTier { rate: 2000, required_directs: 0, required_team_size: 0, required_spent: 0 }, // 20%
        ];

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            multi_level: pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
                max_total_rate: 2500, // cap at 25%
            },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        // L1: 2000, L2: 2000 but total capped at 2500 → L2 gets 500
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].amount, 2000);
        assert_eq!(outputs[1].amount, 500);
        assert_eq!(remaining, 7500);
    });
}

#[test]
fn multi_level_activation_condition() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40, 30]);

        // account 40: 2 direct referrals (不满足 required_directs=3)
        set_stats(1, 40, 2, 10, 5000);
        // account 30: 5 direct referrals (满足)
        set_stats(1, 30, 5, 20, 10000);

        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 3, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 500, required_directs: 3, required_team_size: 0, required_spent: 0 },
        ];

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            multi_level: pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
                max_total_rate: 5000,
            },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        // L1: account 40 不满足 → 跳过，L2: account 30 满足 → 获得 500
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].beneficiary, 30);
        assert_eq!(outputs[0].amount, 500);
        assert_eq!(outputs[0].level, 2);
        assert_eq!(remaining, 9500);
    });
}

#[test]
fn h1_multi_level_cycle_detection() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        // 构造环形推荐链: 50 -> 40 -> 30 -> 50 (循环)
        set_referrer(1, 50, 40);
        set_referrer(1, 40, 30);
        set_referrer(1, 30, 50); // cycle!

        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            multi_level: pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
                max_total_rate: 5000,
            },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        // 只有 40 和 30 获得佣金，50 是 buyer 被 visited 标记，循环在 L3 被检测到
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[1].beneficiary, 30);
        assert_eq!(remaining, 8000);
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

        // MULTI_LEVEL mode, but config only has direct_reward
        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
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
// CommissionPlugin — combined modes
// ============================================================================

#[test]
fn combined_direct_and_multi_level() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40, 30]);

        let levels = vec![
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 300, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];

        pallet::ReferralConfigs::<Test>::insert(1, pallet::ReferralConfig {
            direct_reward: pallet::DirectRewardConfig { rate: 1000 }, // 10%
            multi_level: pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
                max_total_rate: 2000,
            },
            ..Default::default()
        });

        let modes = CommissionModes(CommissionModes::DIRECT_REWARD | CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        // Direct: 40 gets 1000, Multi L1: 40 gets 500, Multi L2: 30 gets 300
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].commission_type, CommissionType::DirectReward);
        assert_eq!(outputs[0].amount, 1000);
        assert_eq!(outputs[1].commission_type, CommissionType::MultiLevel);
        assert_eq!(outputs[1].amount, 500);
        assert_eq!(outputs[2].commission_type, CommissionType::MultiLevel);
        assert_eq!(outputs[2].amount, 300);
        assert_eq!(remaining, 8200);
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
fn plan_writer_set_multi_level() {
    new_test_ext().execute_with(|| {
        assert_ok!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::set_multi_level(
            1, vec![500, 300, 200], 1000,
        ));
        let config = pallet::ReferralConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.multi_level.levels.len(), 3);
        assert_eq!(config.multi_level.levels[0].rate, 500);
        assert_eq!(config.multi_level.max_total_rate, 1000);
    });
}

#[test]
fn h2_plan_writer_rejects_invalid_multi_level_rate() {
    new_test_ext().execute_with(|| {
        assert!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::set_multi_level(
            1, vec![500], 10001,
        ).is_err());
    });
}

#[test]
fn h2_plan_writer_rejects_invalid_level_rate() {
    new_test_ext().execute_with(|| {
        assert!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::set_multi_level(
            1, vec![10001], 5000,
        ).is_err());
    });
}

#[test]
fn plan_writer_too_many_levels() {
    new_test_ext().execute_with(|| {
        // MaxMultiLevels = 15, try 16
        let rates: Vec<u16> = (0..16).map(|_| 100).collect();
        assert!(<pallet::Pallet<Test> as ReferralPlanWriter<Balance>>::set_multi_level(
            1, rates, 1600,
        ).is_err());
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
