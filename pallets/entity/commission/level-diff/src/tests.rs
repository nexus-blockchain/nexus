use super::*;
use crate::mock::*;
use frame_support::{assert_ok, assert_noop};
use pallet_commission_common::CommissionModes;

type Balance = u128;

// ========================================================================
// P1: commission_bonus 回退测试
// ========================================================================

#[test]
fn p1_fallback_to_commission_bonus_when_no_custom_config() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_chain(entity_id);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);
        set_level_bonus(entity_id, 0, 300);
        set_level_bonus(entity_id, 1, 600);

        let mut remaining: Balance = 10000;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff(entity_id, &50, 10000, &mut remaining, &mut outputs);

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 300);
        assert_eq!(outputs[1].beneficiary, 30);
        assert_eq!(outputs[1].amount, 300);
        assert_eq!(remaining, 10000 - 600);
    });
}

#[test]
fn p1_custom_config_takes_priority_over_commission_bonus() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_chain(entity_id);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);
        set_level_bonus(entity_id, 0, 100);
        set_level_bonus(entity_id, 1, 200);

        let level_rates = frame_support::BoundedVec::try_from(vec![500u16, 800]).unwrap();
        assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 10,
        ));

        let mut remaining: Balance = 10000;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff(entity_id, &50, 10000, &mut remaining, &mut outputs);

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].amount, 500);
        assert_eq!(outputs[1].amount, 300);
    });
}

// ========================================================================
// P2: 等级数量不匹配测试
// ========================================================================

#[test]
fn p2_level_id_out_of_bounds_falls_back_to_commission_bonus() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_chain(entity_id);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);
        set_custom_level_id(entity_id, 20, 2);
        set_level_bonus(entity_id, 2, 900);

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
        assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 10,
        ));

        let mut remaining: Balance = 10000;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff(entity_id, &50, 10000, &mut remaining, &mut outputs);

        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 300);
        assert_eq!(outputs[1].beneficiary, 30);
        assert_eq!(outputs[1].amount, 300);
        assert_eq!(outputs[2].beneficiary, 20);
        assert_eq!(outputs[2].amount, 300);
    });
}

#[test]
fn p2_level_id_out_of_bounds_no_bonus_yields_zero() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        set_referrer(entity_id, 50, 40);
        set_custom_level_id(entity_id, 40, 2);

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
        assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 10,
        ));

        let mut remaining: Balance = 10000;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff(entity_id, &50, 10000, &mut remaining, &mut outputs);

        assert_eq!(outputs.len(), 0);
        assert_eq!(remaining, 10000);
    });
}

// ========================================================================
// CommissionPlugin trait 测试
// ========================================================================

#[test]
fn plugin_skips_when_level_diff_mode_not_enabled() {
    new_test_ext().execute_with(|| {
        let modes = CommissionModes(CommissionModes::DIRECT_REWARD);
        let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 1,
        );
        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

#[test]
fn plugin_works_with_level_diff_mode_enabled() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        set_referrer(entity_id, 50, 40);
        set_custom_level_id(entity_id, 40, 0);
        set_level_bonus(entity_id, 0, 500);

        let modes = CommissionModes(CommissionModes::LEVEL_DIFF);
        let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::CommissionPlugin<u64, Balance>>::calculate(
            entity_id, &50, 10000, 10000, modes, false, 1,
        );

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].amount, 500);
        assert_eq!(remaining, 9500);
    });
}

// ========================================================================
// Extrinsic 校验测试
// ========================================================================

#[test]
fn set_config_validates_rates() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![10001u16]).unwrap();
        assert!(pallet::Pallet::<Test>::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 10,
        ).is_err());
    });
}

#[test]
fn set_config_validates_depth() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![100u16]).unwrap();
        assert!(pallet::Pallet::<Test>::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates.clone(), 0,
        ).is_err());
        assert!(pallet::Pallet::<Test>::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 21,
        ).is_err());
    });
}

// ========================================================================
// set_level_rates trait 路径校验
// ========================================================================

#[test]
fn set_level_rates_rejects_invalid_rate() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::LevelDiffPlanWriter;

        assert!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::set_level_rates(
            1, vec![10001], 5
        ).is_err());
        assert_ok!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::set_level_rates(
            1, vec![100, 200, 300], 5
        ));
        let config = pallet::CustomLevelDiffConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.level_rates.len(), 3);
        assert_eq!(config.max_depth, 5);
    });
}

// ========================================================================
// 自定义等级体系基础测试
// ========================================================================

#[test]
fn custom_level_diff_basic() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_chain(entity_id);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);
        set_custom_level_id(entity_id, 20, 2);

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600, 900]).unwrap();
        assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 10,
        ));

        let mut remaining: Balance = 10000;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff(entity_id, &50, 10000, &mut remaining, &mut outputs);

        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 300);
        assert_eq!(outputs[1].beneficiary, 30);
        assert_eq!(outputs[1].amount, 300);
        assert_eq!(outputs[2].beneficiary, 20);
        assert_eq!(outputs[2].amount, 300);
        assert_eq!(remaining, 10000 - 900);
    });
}

// ========================================================================
// 额度耗尽提前退出测试
// ========================================================================

#[test]
fn remaining_exhaustion_caps_commission() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_chain(entity_id);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);
        set_custom_level_id(entity_id, 20, 2);
        set_level_bonus(entity_id, 0, 500);
        set_level_bonus(entity_id, 1, 1000);
        set_level_bonus(entity_id, 2, 1500);

        let mut remaining: Balance = 600;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff(entity_id, &50, 10000, &mut remaining, &mut outputs);

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].amount, 500);
        assert_eq!(outputs[1].amount, 100);
        assert_eq!(remaining, 0);
    });
}

// ========================================================================
// 相同等级跳过测试
// ========================================================================

#[test]
fn same_level_referrers_skipped() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_chain(entity_id);
        set_custom_level_id(entity_id, 40, 1);
        set_custom_level_id(entity_id, 30, 1);
        set_custom_level_id(entity_id, 20, 2);

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600, 900]).unwrap();
        assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 10,
        ));

        let mut remaining: Balance = 10000;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff(entity_id, &50, 10000, &mut remaining, &mut outputs);

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 600);
        assert_eq!(outputs[1].beneficiary, 20);
        assert_eq!(outputs[1].amount, 300);
    });
}

// ========================================================================
// max_depth 限制测试
// ========================================================================

#[test]
fn max_depth_limits_traversal() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_chain(entity_id);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);
        set_custom_level_id(entity_id, 20, 2);
        set_custom_level_id(entity_id, 10, 3);
        set_level_bonus(entity_id, 0, 300);
        set_level_bonus(entity_id, 1, 600);
        set_level_bonus(entity_id, 2, 900);
        set_level_bonus(entity_id, 3, 1200);

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600, 900, 1200]).unwrap();
        assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 2,
        ));

        let mut remaining: Balance = 10000;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff(entity_id, &50, 10000, &mut remaining, &mut outputs);

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[1].beneficiary, 30);
    });
}

// ========================================================================
// clear_config 清除测试
// ========================================================================

#[test]
fn clear_config_removes_config() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::LevelDiffPlanWriter;
        let entity_id = 1u64;

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 5,
        ));
        assert!(pallet::CustomLevelDiffConfigs::<Test>::get(entity_id).is_some());

        assert_ok!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::clear_config(entity_id));
        assert!(pallet::CustomLevelDiffConfigs::<Test>::get(entity_id).is_none());
    });
}

// ========================================================================
// 空推荐链测试
// ========================================================================

#[test]
fn empty_referral_chain_produces_no_output() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600, 900]).unwrap();
        assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 10,
        ));

        let mut remaining: Balance = 10000;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff(entity_id, &50, 10000, &mut remaining, &mut outputs);

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

// ========================================================================
// H1: 推荐链循环检测
// ========================================================================

#[test]
fn h1_referral_cycle_does_not_loop_forever() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;

        set_referrer(entity_id, 50, 40);
        set_referrer(entity_id, 40, 30);
        set_referrer(entity_id, 30, 40); // cycle back to 40

        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);
        set_level_bonus(entity_id, 0, 300);
        set_level_bonus(entity_id, 1, 600);

        let mut remaining: Balance = 10000;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff(entity_id, &50, 10000, &mut remaining, &mut outputs);

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 300);
        assert_eq!(outputs[1].beneficiary, 30);
        assert_eq!(outputs[1].amount, 300);
        assert_eq!(remaining, 10000 - 600);
    });
}

#[test]
fn h1_self_referral_cycle_breaks_immediately() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;

        set_referrer(entity_id, 50, 40);
        set_referrer(entity_id, 40, 40); // self-referral

        set_custom_level_id(entity_id, 40, 0);
        set_level_bonus(entity_id, 0, 500);

        let mut remaining: Balance = 10000;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff(entity_id, &50, 10000, &mut remaining, &mut outputs);

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].amount, 500);
    });
}

// ========================================================================
// H2: 空 level_rates 拒绝
// ========================================================================

#[test]
fn h2_set_config_rejects_empty_level_rates() {
    new_test_ext().execute_with(|| {
        let empty_rates = frame_support::BoundedVec::try_from(vec![]).unwrap();
        assert!(pallet::Pallet::<Test>::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, empty_rates, 10,
        ).is_err());
        assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_none());
    });
}

// ========================================================================
// M1: trait 路径发出事件
// ========================================================================

#[test]
fn m1_set_level_rates_trait_emits_event() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::LevelDiffPlanWriter;
        let entity_id = 42u64;
        set_entity_owner(entity_id, OWNER);

        assert_ok!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::set_level_rates(
            entity_id, vec![100, 200], 5
        ));

        let events = frame_system::Pallet::<Test>::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionLevelDiff(pallet::Event::LevelDiffConfigUpdated { entity_id: 42, levels_count: 2 })
            )
        });
        assert!(found, "LevelDiffConfigUpdated event should be emitted via trait path");
    });
}

// ========================================================================
// M1-R3: clear_config 发出事件
// ========================================================================

#[test]
fn m1r3_clear_config_emits_event() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::LevelDiffPlanWriter;
        let entity_id = 7u64;
        set_entity_owner(entity_id, OWNER);

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 5,
        ));
        assert!(pallet::CustomLevelDiffConfigs::<Test>::get(entity_id).is_some());

        assert_ok!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::clear_config(entity_id));
        assert!(pallet::CustomLevelDiffConfigs::<Test>::get(entity_id).is_none());

        let events = frame_system::Pallet::<Test>::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionLevelDiff(pallet::Event::LevelDiffConfigCleared { entity_id: 7 })
            )
        });
        assert!(found, "LevelDiffConfigCleared event should be emitted");
    });
}

// ========================================================================
// M2-R4: TokenCommissionPlugin 测试覆盖
// ========================================================================

#[test]
fn m2r4_token_plugin_basic_calculation() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_chain(entity_id);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
        assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 10,
        ));

        let modes = CommissionModes(CommissionModes::LEVEL_DIFF);
        let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::TokenCommissionPlugin<u64, u128>>::calculate_token(
            entity_id, &50, 10000u128, 10000u128, modes, false, 1,
        );

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 300u128);
        assert_eq!(outputs[1].beneficiary, 30);
        assert_eq!(outputs[1].amount, 300u128);
        assert_eq!(remaining, 10000 - 600);
    });
}

#[test]
fn m2r4_token_plugin_skips_when_mode_not_enabled() {
    new_test_ext().execute_with(|| {
        let modes = CommissionModes(CommissionModes::DIRECT_REWARD);
        let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::TokenCommissionPlugin<u64, u128>>::calculate_token(
            1, &50, 10000u128, 10000u128, modes, false, 1,
        );
        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000u128);
    });
}

#[test]
fn m2r4_token_plugin_cycle_detection() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;

        set_referrer(entity_id, 50, 40);
        set_referrer(entity_id, 40, 30);
        set_referrer(entity_id, 30, 40);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
        assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 10,
        ));

        let modes = CommissionModes(CommissionModes::LEVEL_DIFF);
        let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::TokenCommissionPlugin<u64, u128>>::calculate_token(
            entity_id, &50, 10000u128, 10000u128, modes, false, 1,
        );

        assert_eq!(outputs.len(), 2);
        assert_eq!(remaining, 10000u128 - 600);
    });
}

// ========================================================================
// M2-R3: trait 路径拒绝空 level_rates
// ========================================================================

#[test]
fn m2r3_set_level_rates_trait_rejects_empty() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::LevelDiffPlanWriter;

        let result = <pallet::Pallet<Test> as LevelDiffPlanWriter>::set_level_rates(1, vec![], 5);
        assert!(result.is_err());
        assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_none());
    });
}

// ========================================================================
// X1: Owner/Admin 权限模型
// ========================================================================

#[test]
fn x1_owner_can_set_config() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
        assert_ok!(CommissionLevelDiff::set_level_diff_config(
            RuntimeOrigin::signed(OWNER), 1, rates, 10,
        ));
        assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_some());
    });
}

#[test]
fn x1_admin_can_set_config() {
    new_test_ext().execute_with(|| {
        set_entity_admin(1, ADMIN);
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_ok!(CommissionLevelDiff::set_level_diff_config(
            RuntimeOrigin::signed(ADMIN), 1, rates, 5,
        ));
        assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_some());
    });
}

#[test]
fn x1_non_owner_rejected() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_noop!(
            CommissionLevelDiff::set_level_diff_config(
                RuntimeOrigin::signed(NON_OWNER), 1, rates, 5,
            ),
            Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

#[test]
fn x1_entity_not_found_rejected() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_noop!(
            CommissionLevelDiff::set_level_diff_config(
                RuntimeOrigin::signed(OWNER), 999, rates, 5,
            ),
            Error::<Test>::EntityNotFound
        );
    });
}

// ========================================================================
// X2: force_set (Root-only)
// ========================================================================

#[test]
fn x2_force_set_works_for_root() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![500u16]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 10,
        ));
        let config = pallet::CustomLevelDiffConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.level_rates[0], 500);
    });
}

#[test]
fn x2_force_set_rejects_non_root() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![500u16]).unwrap();
        assert_noop!(
            CommissionLevelDiff::force_set_level_diff_config(
                RuntimeOrigin::signed(OWNER), 1, rates, 10,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ========================================================================
// X3: clear_level_diff_config (signed, Owner/Admin)
// ========================================================================

#[test]
fn x3_owner_can_clear_config() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 5,
        ));
        assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_some());

        assert_ok!(CommissionLevelDiff::clear_level_diff_config(
            RuntimeOrigin::signed(OWNER), 1,
        ));
        assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_none());
    });
}

#[test]
fn x3_clear_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 5,
        ));

        assert_noop!(
            CommissionLevelDiff::clear_level_diff_config(
                RuntimeOrigin::signed(NON_OWNER), 1,
            ),
            Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

#[test]
fn x3_clear_rejects_absent_config() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionLevelDiff::clear_level_diff_config(
                RuntimeOrigin::signed(OWNER), 1,
            ),
            Error::<Test>::ConfigNotFound
        );
    });
}

// ========================================================================
// X4: force_clear (Root-only)
// ========================================================================

#[test]
fn x4_force_clear_works() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 5,
        ));
        assert_ok!(CommissionLevelDiff::force_clear_level_diff_config(
            RuntimeOrigin::root(), 1,
        ));
        assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_none());
    });
}

#[test]
fn x4_force_clear_rejects_non_root() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 5,
        ));
        assert_noop!(
            CommissionLevelDiff::force_clear_level_diff_config(
                RuntimeOrigin::signed(OWNER), 1,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn x4_force_clear_rejects_absent() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionLevelDiff::force_clear_level_diff_config(
                RuntimeOrigin::root(), 1,
            ),
            Error::<Test>::ConfigNotFound
        );
    });
}

// ========================================================================
// X6: 治理锁检查
// ========================================================================

#[test]
fn x6_locked_entity_rejects_set() {
    new_test_ext().execute_with(|| {
        lock_entity(1);
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_noop!(
            CommissionLevelDiff::set_level_diff_config(
                RuntimeOrigin::signed(OWNER), 1, rates, 5,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn x6_locked_entity_rejects_clear() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 5,
        ));
        lock_entity(1);
        assert_noop!(
            CommissionLevelDiff::clear_level_diff_config(
                RuntimeOrigin::signed(OWNER), 1,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn x6_force_set_bypasses_lock() {
    new_test_ext().execute_with(|| {
        lock_entity(1);
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 5,
        ));
        assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_some());
    });
}

// ========================================================================
// X8: is_banned 守卫
// ========================================================================

#[test]
fn x8_banned_referrer_skipped_in_nex() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_chain(entity_id);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);
        set_custom_level_id(entity_id, 20, 2);

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600, 900]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 10,
        ));

        ban_member(entity_id, 30);

        let mut remaining: Balance = 10000;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff(entity_id, &50, 10000, &mut remaining, &mut outputs);

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 300);
        assert_eq!(outputs[1].beneficiary, 20);
        assert_eq!(outputs[1].amount, 600);
    });
}

#[test]
fn x8_banned_referrer_skipped_in_token() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_chain(entity_id);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);
        set_custom_level_id(entity_id, 20, 2);

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600, 900]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 10,
        ));

        ban_member(entity_id, 30);

        let modes = CommissionModes(CommissionModes::LEVEL_DIFF);
        let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::TokenCommissionPlugin<u64, u128>>::calculate_token(
            entity_id, &50, 10000u128, 10000u128, modes, false, 1,
        );

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 300u128);
        assert_eq!(outputs[1].beneficiary, 20);
        assert_eq!(outputs[1].amount, 600u128);
        assert_eq!(remaining, 10000u128 - 900);
    });
}

#[test]
fn x8_non_banned_still_receives() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        set_referrer(entity_id, 50, 40);
        set_custom_level_id(entity_id, 40, 0);
        set_level_bonus(entity_id, 0, 500);

        let mut remaining: Balance = 10000;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff(entity_id, &50, 10000, &mut remaining, &mut outputs);

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].amount, 500);
    });
}

// ========================================================================
// X10: 幻影事件守卫
// ========================================================================

#[test]
fn x10_trait_clear_no_phantom_event_when_absent() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::LevelDiffPlanWriter;

        assert_ok!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::clear_config(1));

        let events = frame_system::Pallet::<Test>::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionLevelDiff(pallet::Event::LevelDiffConfigCleared { .. })
            )
        });
        assert!(!found, "No LevelDiffConfigCleared event should be emitted when config absent");
    });
}

#[test]
fn x10_trait_clear_emits_event_when_present() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::LevelDiffPlanWriter;

        assert_ok!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::set_level_rates(1, vec![100], 5));
        assert_ok!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::clear_config(1));

        let events = frame_system::Pallet::<Test>::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionLevelDiff(pallet::Event::LevelDiffConfigCleared { entity_id: 1 })
            )
        });
        assert!(found, "LevelDiffConfigCleared should be emitted when config exists");
    });
}

// ========================================================================
// F1: Entity 活跃状态检查
// ========================================================================

#[test]
fn f1_set_config_rejects_inactive_entity() {
    new_test_ext().execute_with(|| {
        deactivate_entity(1);
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_noop!(
            CommissionLevelDiff::set_level_diff_config(
                RuntimeOrigin::signed(OWNER), 1, rates, 5,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f1_clear_config_rejects_inactive_entity() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 5,
        ));
        deactivate_entity(1);
        assert_noop!(
            CommissionLevelDiff::clear_level_diff_config(
                RuntimeOrigin::signed(OWNER), 1,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f1_update_config_rejects_inactive_entity() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 5,
        ));
        deactivate_entity(1);
        assert_noop!(
            CommissionLevelDiff::update_level_diff_config(
                RuntimeOrigin::signed(OWNER), 1, None, Some(3),
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f1_force_set_ignores_inactive_entity() {
    new_test_ext().execute_with(|| {
        deactivate_entity(1);
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 5,
        ));
        assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_some());
    });
}

// ========================================================================
// F2: 部分更新 — update_level_diff_config
// ========================================================================

#[test]
fn f2_update_max_depth_only() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
        assert_ok!(CommissionLevelDiff::set_level_diff_config(
            RuntimeOrigin::signed(OWNER), 1, rates, 10,
        ));

        assert_ok!(CommissionLevelDiff::update_level_diff_config(
            RuntimeOrigin::signed(OWNER), 1, None, Some(3),
        ));

        let config = pallet::CustomLevelDiffConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.level_rates.len(), 2);
        assert_eq!(config.level_rates[0], 300);
        assert_eq!(config.level_rates[1], 600);
        assert_eq!(config.max_depth, 3);
    });
}

#[test]
fn f2_update_level_rates_only() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_ok!(CommissionLevelDiff::set_level_diff_config(
            RuntimeOrigin::signed(OWNER), 1, rates, 10,
        ));

        let new_rates = frame_support::BoundedVec::try_from(vec![400u16, 800]).unwrap();
        assert_ok!(CommissionLevelDiff::update_level_diff_config(
            RuntimeOrigin::signed(OWNER), 1, Some(new_rates), None,
        ));

        let config = pallet::CustomLevelDiffConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.level_rates.len(), 2);
        assert_eq!(config.level_rates[0], 400);
        assert_eq!(config.level_rates[1], 800);
        assert_eq!(config.max_depth, 10);
    });
}

#[test]
fn f2_update_both_params() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_ok!(CommissionLevelDiff::set_level_diff_config(
            RuntimeOrigin::signed(OWNER), 1, rates, 10,
        ));

        let new_rates = frame_support::BoundedVec::try_from(vec![500u16, 1000]).unwrap();
        assert_ok!(CommissionLevelDiff::update_level_diff_config(
            RuntimeOrigin::signed(OWNER), 1, Some(new_rates), Some(5),
        ));

        let config = pallet::CustomLevelDiffConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.level_rates[0], 500);
        assert_eq!(config.level_rates[1], 1000);
        assert_eq!(config.max_depth, 5);
    });
}

#[test]
fn f2_update_rejects_no_existing_config() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionLevelDiff::update_level_diff_config(
                RuntimeOrigin::signed(OWNER), 1, None, Some(3),
            ),
            Error::<Test>::ConfigNotFound
        );
    });
}

#[test]
fn f2_update_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 5,
        ));
        assert_noop!(
            CommissionLevelDiff::update_level_diff_config(
                RuntimeOrigin::signed(NON_OWNER), 1, None, Some(3),
            ),
            Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

#[test]
fn f2_update_rejects_locked_entity() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 5,
        ));
        lock_entity(1);
        assert_noop!(
            CommissionLevelDiff::update_level_diff_config(
                RuntimeOrigin::signed(OWNER), 1, None, Some(3),
            ),
            Error::<Test>::EntityLocked
        );
    });
}

// ========================================================================
// F3: 等级率单调递增校验
// ========================================================================

#[test]
fn f3_rejects_non_monotonic_rates_extrinsic() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![600u16, 300]).unwrap();
        assert_noop!(
            CommissionLevelDiff::set_level_diff_config(
                RuntimeOrigin::signed(OWNER), 1, rates, 10,
            ),
            Error::<Test>::RatesNotMonotonic
        );
    });
}

#[test]
fn f3_rejects_non_monotonic_rates_force() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![900u16, 300, 600]).unwrap();
        assert_noop!(
            CommissionLevelDiff::force_set_level_diff_config(
                RuntimeOrigin::root(), 1, rates, 10,
            ),
            Error::<Test>::RatesNotMonotonic
        );
    });
}

#[test]
fn f3_rejects_non_monotonic_rates_trait_path() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::LevelDiffPlanWriter;

        let result = <pallet::Pallet<Test> as LevelDiffPlanWriter>::set_level_rates(
            1, vec![500, 200, 400], 5
        );
        assert!(result.is_err());
    });
}

#[test]
fn f3_accepts_equal_rates() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16, 300, 600]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 10,
        ));
        let config = pallet::CustomLevelDiffConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.level_rates.len(), 3);
    });
}

#[test]
fn f3_single_rate_always_passes() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![500u16]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 10,
        ));
    });
}

// ========================================================================
// F4: is_member 检查 — 计算路径
// ========================================================================

#[test]
fn f4_skips_non_member_referrer() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        set_referrer(entity_id, 50, 40);
        set_referrer(entity_id, 40, 30);
        mark_non_member(entity_id, 40);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 10,
        ));

        let mut remaining: Balance = 10000;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff(entity_id, &50, 10000, &mut remaining, &mut outputs);

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].beneficiary, 30);
        assert_eq!(outputs[0].amount, 600);
    });
}

#[test]
fn f4_skips_non_member_in_token_path() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        set_referrer(entity_id, 50, 40);
        set_referrer(entity_id, 40, 30);
        mark_non_member(entity_id, 40);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 10,
        ));

        let mut remaining: u128 = 10000;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff_token::<u128>(
            entity_id, &50, 10000u128, &mut remaining, &mut outputs,
        );

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].beneficiary, 30);
        assert_eq!(outputs[0].amount, 600u128);
    });
}

// ========================================================================
// F5: 佣金分配明细事件
// ========================================================================

#[test]
fn f5_emits_commission_detail_events() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        set_referrer(entity_id, 50, 40);
        set_referrer(entity_id, 40, 30);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 10,
        ));

        frame_system::Pallet::<Test>::reset_events();

        let mut remaining: Balance = 10000;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff(entity_id, &50, 10000, &mut remaining, &mut outputs);

        let events = frame_system::Pallet::<Test>::events();
        let detail_events: Vec<_> = events.iter().filter(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionLevelDiff(pallet::Event::LevelDiffCommissionDetail { .. })
            )
        }).collect();

        assert_eq!(detail_events.len(), 2, "Should emit 2 detail events");

        match &detail_events[0].event {
            RuntimeEvent::CommissionLevelDiff(pallet::Event::LevelDiffCommissionDetail {
                beneficiary, referrer_rate, prev_rate, diff_rate, amount, level, ..
            }) => {
                assert_eq!(*beneficiary, 40);
                assert_eq!(*referrer_rate, 300);
                assert_eq!(*prev_rate, 0);
                assert_eq!(*diff_rate, 300);
                assert_eq!(*amount, 300u128);
                assert_eq!(*level, 1);
            },
            _ => panic!("Unexpected event type"),
        }

        match &detail_events[1].event {
            RuntimeEvent::CommissionLevelDiff(pallet::Event::LevelDiffCommissionDetail {
                beneficiary, referrer_rate, prev_rate, diff_rate, amount, level, ..
            }) => {
                assert_eq!(*beneficiary, 30);
                assert_eq!(*referrer_rate, 600);
                assert_eq!(*prev_rate, 300);
                assert_eq!(*diff_rate, 300);
                assert_eq!(*amount, 300u128);
                assert_eq!(*level, 2);
            },
            _ => panic!("Unexpected event type"),
        }
    });
}

// ========================================================================
// F6: LevelDiffPlanWriter entity 存在性验证
// ========================================================================

#[test]
fn f6_trait_set_rejects_nonexistent_entity() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::LevelDiffPlanWriter;

        let result = <pallet::Pallet<Test> as LevelDiffPlanWriter>::set_level_rates(
            999, vec![100, 200], 5
        );
        assert!(result.is_err());
        assert!(pallet::CustomLevelDiffConfigs::<Test>::get(999).is_none());
    });
}

#[test]
fn f6_trait_set_works_for_existing_entity() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::LevelDiffPlanWriter;

        assert_ok!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::set_level_rates(
            1, vec![100, 200], 5
        ));
        assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_some());
    });
}

// ========================================================================
// F8: levels_count 事件字段
// ========================================================================

#[test]
fn f8_config_updated_event_contains_levels_count() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![100u16, 200, 300]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), 1, rates, 5,
        ));

        let events = frame_system::Pallet::<Test>::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionLevelDiff(pallet::Event::LevelDiffConfigUpdated {
                    entity_id: 1,
                    levels_count: 3,
                })
            )
        });
        assert!(found, "LevelDiffConfigUpdated should include levels_count=3");
    });
}

#[test]
fn f8_trait_path_event_contains_levels_count() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::LevelDiffPlanWriter;

        assert_ok!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::set_level_rates(
            1, vec![100, 200, 300, 400], 5
        ));

        let events = frame_system::Pallet::<Test>::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionLevelDiff(pallet::Event::LevelDiffConfigUpdated {
                    entity_id: 1,
                    levels_count: 4,
                })
            )
        });
        assert!(found, "Trait path should emit LevelDiffConfigUpdated with levels_count=4");
    });
}

// ========================================================================
// M1-R7: Token 路径佣金明细事件
// ========================================================================

#[test]
fn m1r7_token_path_emits_commission_detail_event() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        set_referrer(entity_id, 50, 40);
        set_referrer(entity_id, 40, 30);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 10,
        ));

        frame_system::Pallet::<Test>::reset_events();

        let modes = CommissionModes(CommissionModes::LEVEL_DIFF);
        let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::TokenCommissionPlugin<u64, u128>>::calculate_token(
            entity_id, &50, 10000u128, 10000u128, modes, false, 1,
        );

        assert_eq!(outputs.len(), 2);
        assert_eq!(remaining, 10000u128 - 600);

        let events = frame_system::Pallet::<Test>::events();
        let token_detail_events: Vec<_> = events.iter().filter(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionLevelDiff(pallet::Event::LevelDiffTokenCommissionDetail { .. })
            )
        }).collect();

        assert_eq!(token_detail_events.len(), 2, "Should emit 2 token detail events");

        match &token_detail_events[0].event {
            RuntimeEvent::CommissionLevelDiff(pallet::Event::LevelDiffTokenCommissionDetail {
                beneficiary, referrer_rate, prev_rate, diff_rate, token_amount, level, ..
            }) => {
                assert_eq!(*beneficiary, 40);
                assert_eq!(*referrer_rate, 300);
                assert_eq!(*prev_rate, 0);
                assert_eq!(*diff_rate, 300);
                assert_eq!(*token_amount, 300u128);
                assert_eq!(*level, 1);
            },
            _ => panic!("Unexpected event type"),
        }

        match &token_detail_events[1].event {
            RuntimeEvent::CommissionLevelDiff(pallet::Event::LevelDiffTokenCommissionDetail {
                beneficiary, referrer_rate, prev_rate, diff_rate, token_amount, level, ..
            }) => {
                assert_eq!(*beneficiary, 30);
                assert_eq!(*referrer_rate, 600);
                assert_eq!(*prev_rate, 300);
                assert_eq!(*diff_rate, 300);
                assert_eq!(*token_amount, 300u128);
                assert_eq!(*level, 2);
            },
            _ => panic!("Unexpected event type"),
        }
    });
}

// ========================================================================
// M2-R7: 冻结会员跳过测试
// ========================================================================

#[test]
fn m2r7_frozen_referrer_skipped_in_nex() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_chain(entity_id);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);
        set_custom_level_id(entity_id, 20, 2);

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600, 900]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 10,
        ));

        freeze_member(entity_id, 30);

        let mut remaining: Balance = 10000;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff(entity_id, &50, 10000, &mut remaining, &mut outputs);

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 300);
        assert_eq!(outputs[1].beneficiary, 20);
        assert_eq!(outputs[1].amount, 600);
    });
}

#[test]
fn m2r7_frozen_referrer_skipped_in_token() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_chain(entity_id);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);
        set_custom_level_id(entity_id, 20, 2);

        let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600, 900]).unwrap();
        assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
            RuntimeOrigin::root(), entity_id, level_rates, 10,
        ));

        freeze_member(entity_id, 30);

        let modes = CommissionModes(CommissionModes::LEVEL_DIFF);
        let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::TokenCommissionPlugin<u64, u128>>::calculate_token(
            entity_id, &50, 10000u128, 10000u128, modes, false, 1,
        );

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 300u128);
        assert_eq!(outputs[1].beneficiary, 20);
        assert_eq!(outputs[1].amount, 600u128);
        assert_eq!(remaining, 10000u128 - 900);
    });
}

// ========================================================================
// L1-R7: force extrinsic entity_exists 校验
// ========================================================================

#[test]
fn l1r7_force_set_rejects_nonexistent_entity() {
    new_test_ext().execute_with(|| {
        let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
        assert_noop!(
            CommissionLevelDiff::force_set_level_diff_config(
                RuntimeOrigin::root(), 999, rates, 5,
            ),
            Error::<Test>::EntityNotFound
        );
    });
}

#[test]
fn l1r7_force_clear_rejects_nonexistent_entity() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionLevelDiff::force_clear_level_diff_config(
                RuntimeOrigin::root(), 999,
            ),
            Error::<Test>::EntityNotFound
        );
    });
}

// ========================================================================
// L2-R7: Token 路径 remaining 耗尽测试
// ========================================================================

#[test]
fn l2r7_token_remaining_exhaustion_caps_commission() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_chain(entity_id);
        set_custom_level_id(entity_id, 40, 0);
        set_custom_level_id(entity_id, 30, 1);
        set_custom_level_id(entity_id, 20, 2);
        set_level_bonus(entity_id, 0, 500);
        set_level_bonus(entity_id, 1, 1000);
        set_level_bonus(entity_id, 2, 1500);

        let mut remaining: u128 = 600;
        let mut outputs = alloc::vec::Vec::new();
        pallet::Pallet::<Test>::process_level_diff_token::<u128>(
            entity_id, &50, 10000u128, &mut remaining, &mut outputs,
        );

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].amount, 500u128);
        assert_eq!(outputs[1].amount, 100u128);
        assert_eq!(remaining, 0u128);
    });
}

// ========================================================================
// 审计 R8: M1 — 未激活实体插件路径返空
// ========================================================================

#[test]
fn m1r8_inactive_entity_nex_plugin_returns_empty() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        set_referrer(entity_id, 50, 40);
        set_custom_level_id(entity_id, 40, 0);
        set_level_bonus(entity_id, 0, 500);

        let modes = CommissionModes(CommissionModes::LEVEL_DIFF);
        let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::CommissionPlugin<u64, Balance>>::calculate(
            entity_id, &50, 10000, 10000, modes, false, 1,
        );
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].amount, 500);
        assert_eq!(remaining, 9500);

        deactivate_entity(entity_id);
        let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::CommissionPlugin<u64, Balance>>::calculate(
            entity_id, &50, 10000, 10000, modes, false, 1,
        );
        assert!(outputs.is_empty(), "inactive entity should yield no commission");
        assert_eq!(remaining, 10000, "remaining should be unchanged");
    });
}

#[test]
fn m1r8_inactive_entity_token_plugin_returns_empty() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        set_referrer(entity_id, 50, 40);
        set_custom_level_id(entity_id, 40, 0);
        set_level_bonus(entity_id, 0, 500);

        let modes = CommissionModes(CommissionModes::LEVEL_DIFF);

        let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::TokenCommissionPlugin<u64, u128>>::calculate_token(
            entity_id, &50, 10000u128, 10000u128, modes, false, 1,
        );
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].amount, 500u128);
        assert_eq!(remaining, 9500u128);

        deactivate_entity(entity_id);
        let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::TokenCommissionPlugin<u64, u128>>::calculate_token(
            entity_id, &50, 10000u128, 10000u128, modes, false, 1,
        );
        assert!(outputs.is_empty(), "inactive entity should yield no token commission");
        assert_eq!(remaining, 10000u128, "remaining should be unchanged");
    });
}
