use crate::mock::*;
use crate::pallet;
use frame_support::{assert_noop, assert_ok};
use pallet_commission_common::{
    CommissionModes, CommissionPlugin, MultiLevelPlanWriter,
};

const OWNER: u64 = 100;
const ADMIN: u64 = 200;
const NOBODY: u64 = 999;

fn setup_entity(entity_id: u64) {
    set_entity_owner(entity_id, OWNER);
    set_entity_admin(entity_id, ADMIN, pallet_entity_common::AdminPermission::COMMISSION_MANAGE);
}

// ============================================================================
// Extrinsic tests — set_multi_level_config (Owner/Admin)
// ============================================================================

#[test]
fn set_multi_level_config_works_by_owner() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        let tiers = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 500, required_directs: 3, required_team_size: 0, required_spent: 0 },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1, tiers.try_into().unwrap(), 2000,
        ));
        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.levels.len(), 2);
        assert_eq!(config.max_total_rate, 2000);
    });
}

#[test]
fn set_multi_level_config_works_by_admin_with_commission_manage() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        let tiers = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(ADMIN), 1, tiers.try_into().unwrap(), 2000,
        ));
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_some());
    });
}

#[test]
fn set_multi_level_config_rejects_admin_without_commission_manage() {
    new_test_ext().execute_with(|| {
        set_entity_owner(1, OWNER);
        // Admin has SHOP_MANAGE only, not COMMISSION_MANAGE
        set_entity_admin(1, ADMIN, pallet_entity_common::AdminPermission::SHOP_MANAGE);
        let tiers = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        assert_noop!(
            CommissionMultiLevel::set_multi_level_config(
                RuntimeOrigin::signed(ADMIN), 1, tiers.try_into().unwrap(), 2000,
            ),
            pallet::Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

#[test]
fn set_multi_level_config_rejects_invalid_rate() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        let tiers = vec![
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        assert_noop!(
            CommissionMultiLevel::set_multi_level_config(RuntimeOrigin::signed(OWNER), 1, tiers.try_into().unwrap(), 10001),
            pallet::Error::<Test>::InvalidRate
        );
    });
}

#[test]
fn set_multi_level_config_rejects_invalid_tier_rate() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        let tiers = vec![
            pallet::MultiLevelTier { rate: 10001, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        assert_noop!(
            CommissionMultiLevel::set_multi_level_config(RuntimeOrigin::signed(OWNER), 1, tiers.try_into().unwrap(), 5000),
            pallet::Error::<Test>::InvalidRate
        );
    });
}

#[test]
fn set_multi_level_config_rejects_entity_not_found() {
    new_test_ext().execute_with(|| {
        // Entity 999 does not exist
        let tiers = vec![
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        assert_noop!(
            CommissionMultiLevel::set_multi_level_config(RuntimeOrigin::signed(NOBODY), 999, tiers.try_into().unwrap(), 2000),
            pallet::Error::<Test>::EntityNotFound
        );
    });
}

#[test]
fn set_multi_level_config_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        let tiers = vec![
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        assert_noop!(
            CommissionMultiLevel::set_multi_level_config(RuntimeOrigin::signed(NOBODY), 1, tiers.try_into().unwrap(), 2000),
            pallet::Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

// ============================================================================
// Extrinsic tests — clear_multi_level_config (Owner/Admin)
// ============================================================================

#[test]
fn clear_multi_level_config_works_by_owner() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        let tiers = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1, tiers.try_into().unwrap(), 2000,
        ));
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_some());

        assert_ok!(CommissionMultiLevel::clear_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
        ));
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_none());
    });
}

#[test]
fn clear_multi_level_config_rejects_absent() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        assert_noop!(
            CommissionMultiLevel::clear_multi_level_config(RuntimeOrigin::signed(OWNER), 1),
            pallet::Error::<Test>::ConfigNotFound
        );
    });
}

#[test]
fn clear_multi_level_config_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: vec![
                pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            ].try_into().unwrap(),
            max_total_rate: 2000,
        });
        assert_noop!(
            CommissionMultiLevel::clear_multi_level_config(RuntimeOrigin::signed(NOBODY), 1),
            pallet::Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

// ============================================================================
// Extrinsic tests — force_set / force_clear (Root)
// ============================================================================

#[test]
fn force_set_multi_level_config_works_for_root() {
    new_test_ext().execute_with(|| {
        let tiers = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        assert_ok!(CommissionMultiLevel::force_set_multi_level_config(
            RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 2000,
        ));
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_some());
    });
}

#[test]
fn force_set_multi_level_config_rejects_non_root() {
    new_test_ext().execute_with(|| {
        let tiers = vec![
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        assert_noop!(
            CommissionMultiLevel::force_set_multi_level_config(
                RuntimeOrigin::signed(1), 1, tiers.try_into().unwrap(), 2000,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn force_clear_multi_level_config_works() {
    new_test_ext().execute_with(|| {
        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: vec![
                pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            ].try_into().unwrap(),
            max_total_rate: 2000,
        });
        assert_ok!(CommissionMultiLevel::force_clear_multi_level_config(RuntimeOrigin::root(), 1));
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_none());
    });
}

#[test]
fn force_clear_multi_level_config_idempotent() {
    new_test_ext().execute_with(|| {
        // No config exists — should succeed silently without event
        assert_ok!(CommissionMultiLevel::force_clear_multi_level_config(RuntimeOrigin::root(), 1));
        let events = System::events();
        assert!(!events.iter().any(|e| matches!(
            e.event,
            RuntimeEvent::CommissionMultiLevel(pallet::Event::MultiLevelConfigCleared { .. })
        )));
    });
}

#[test]
fn force_clear_multi_level_config_rejects_non_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionMultiLevel::force_clear_multi_level_config(RuntimeOrigin::signed(1), 1),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ============================================================================
// CommissionPlugin — multi level basic
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

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000, // 30%
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

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 2500, // cap at 25%
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

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 5000,
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

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 5000,
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
// CommissionPlugin — mode not enabled
// ============================================================================

#[test]
fn mode_not_enabled_returns_empty() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40, 30]);

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: vec![
                pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            ].try_into().unwrap(),
            max_total_rate: 3000,
        });

        // DIRECT_REWARD mode, but we are the MultiLevel plugin
        let modes = CommissionModes(CommissionModes::DIRECT_REWARD);
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
        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

// ============================================================================
// MultiLevelPlanWriter tests
// ============================================================================

#[test]
fn plan_writer_set_multi_level() {
    new_test_ext().execute_with(|| {
        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(
            1, vec![500, 300, 200], 1000,
        ));
        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.levels.len(), 3);
        assert_eq!(config.levels[0].rate, 500);
        assert_eq!(config.max_total_rate, 1000);
    });
}

#[test]
fn h2_plan_writer_rejects_invalid_multi_level_rate() {
    new_test_ext().execute_with(|| {
        assert!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(
            1, vec![500], 10001,
        ).is_err());
    });
}

#[test]
fn h2_plan_writer_rejects_invalid_level_rate() {
    new_test_ext().execute_with(|| {
        assert!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(
            1, vec![10001], 5000,
        ).is_err());
    });
}

#[test]
fn plan_writer_too_many_levels() {
    new_test_ext().execute_with(|| {
        // MaxMultiLevels = 15, try 16
        let rates: Vec<u16> = (0..16).map(|_| 100).collect();
        assert!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(
            1, rates, 1600,
        ).is_err());
    });
}

#[test]
fn plan_writer_clear_config() {
    new_test_ext().execute_with(|| {
        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(
            1, vec![500], 1000,
        ));
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_some());

        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::clear_multi_level_config(1));
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_none());
    });
}

// ============================================================================
// H1 regression — required_spent uses USDT not NEX Balance
// ============================================================================

#[test]
fn h1_required_spent_uses_usdt_not_nex_balance() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);

        // account 40: NEX Balance = 10^12 (very large), USDT = 50_000_000 (50 USDT)
        set_stats(1, 40, 5, 10, 1_000_000_000_000);
        set_spent_usdt(1, 40, 50_000_000); // 50 USDT (precision 10^6)

        // required_spent = 100_000_000 (100 USDT) — should NOT pass despite huge NEX balance
        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 100_000_000 },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000,
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        // Account 40 only spent 50 USDT, required 100 USDT → activation fails
        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

#[test]
fn h1_required_spent_passes_when_usdt_sufficient() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);

        set_stats(1, 40, 5, 10, 0); // NEX balance irrelevant
        set_spent_usdt(1, 40, 200_000_000); // 200 USDT

        // required_spent = 100_000_000 (100 USDT) — should pass
        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 100_000_000 },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000,
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 1000);
        assert_eq!(remaining, 9000);
    });
}

// ============================================================================
// M2 — required_team_size activation coverage
// ============================================================================

#[test]
fn m2_required_team_size_activation() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40, 30]);

        // account 40: team_size=5 (不满足 required_team_size=10)
        set_stats(1, 40, 5, 5, 0);
        // account 30: team_size=15 (满足)
        set_stats(1, 30, 3, 15, 0);

        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 10, required_spent: 0 },
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 10, required_spent: 0 },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 5000,
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        // L1: account 40 team_size=5 < 10 → skip
        // L2: account 30 team_size=15 >= 10 → gets 500
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].beneficiary, 30);
        assert_eq!(outputs[0].amount, 500);
        assert_eq!(outputs[0].level, 2);
        assert_eq!(remaining, 9500);
    });
}

#[test]
fn m2_combined_activation_all_conditions() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);

        // account 40: directs=5, team_size=20, USDT spent=500_000_000 (500 USDT)
        set_stats(1, 40, 5, 20, 0);
        set_spent_usdt(1, 40, 500_000_000);

        // All three conditions required
        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 3, required_team_size: 10, required_spent: 100_000_000 },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000,
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        // All conditions met: directs=5>=3, team=20>=10, usdt=500>=100
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].amount, 1000);
        assert_eq!(remaining, 9000);
    });
}

#[test]
fn m2_combined_activation_fails_one_condition() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40]);

        // account 40: directs=5, team_size=20, but USDT only 50 (below 100 threshold)
        set_stats(1, 40, 5, 20, 0);
        set_spent_usdt(1, 40, 50_000_000); // 50 USDT < 100 USDT required

        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 3, required_team_size: 10, required_spent: 100_000_000 },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000,
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        // directs and team pass, but spent fails → no commission
        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

// ============================================================================
// Round 2 audit regression tests
// ============================================================================

#[test]
fn m1_r2_plan_writer_emits_config_updated_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(
            1, vec![500, 300], 1000,
        ));
        let events = System::events();
        assert!(events.iter().any(|e| matches!(
            e.event,
            RuntimeEvent::CommissionMultiLevel(pallet::Event::MultiLevelConfigUpdated { entity_id: 1 })
        )));
    });
}

#[test]
fn m2_r2_plan_writer_clear_emits_cleared_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(
            1, vec![500], 1000,
        ));
        System::reset_events();
        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::clear_multi_level_config(1));
        let events = System::events();
        assert!(events.iter().any(|e| matches!(
            e.event,
            RuntimeEvent::CommissionMultiLevel(pallet::Event::MultiLevelConfigCleared { entity_id: 1 })
        )));
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_none());
    });
}

#[test]
fn l1_r2_set_config_rejects_empty_levels() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        let empty: Vec<pallet::MultiLevelTier> = vec![];
        assert_noop!(
            CommissionMultiLevel::set_multi_level_config(RuntimeOrigin::signed(OWNER), 1, empty.try_into().unwrap(), 1000),
            pallet::Error::<Test>::EmptyLevels
        );
    });
}

#[test]
fn l1_r2_plan_writer_rejects_empty_levels() {
    new_test_ext().execute_with(|| {
        assert!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(
            1, vec![], 1000,
        ).is_err());
    });
}

#[test]
fn l2_r2_set_config_rejects_zero_max_total_rate() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        let tiers = vec![
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        assert_noop!(
            CommissionMultiLevel::set_multi_level_config(RuntimeOrigin::signed(OWNER), 1, tiers.try_into().unwrap(), 0),
            pallet::Error::<Test>::InvalidRate
        );
    });
}

#[test]
fn l2_r2_plan_writer_rejects_zero_max_total_rate() {
    new_test_ext().execute_with(|| {
        assert!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(
            1, vec![500], 0,
        ).is_err());
    });
}

// ============================================================================
// L2-R5: rate=0 placeholder tier
// ============================================================================

#[test]
fn l2_r5_rate_zero_placeholder_skips_referrer() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        // buyer=50 -> 40 -> 30 -> 20
        setup_chain(1, 50, &[40, 30, 20]);

        let levels = vec![
            pallet::MultiLevelTier { rate: 0, required_directs: 0, required_team_size: 0, required_spent: 0 },    // L1: placeholder
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },  // L2: 10%
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },   // L3: 5%
        ];

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000,
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        // L1: rate=0 → skip, advance past 40 to 30
        // L2: 30 gets 1000 at level 2
        // L3: 20 gets 500 at level 3
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 30);
        assert_eq!(outputs[0].amount, 1000);
        assert_eq!(outputs[0].level, 2);
        assert_eq!(outputs[1].beneficiary, 20);
        assert_eq!(outputs[1].amount, 500);
        assert_eq!(outputs[1].level, 3);
        assert_eq!(remaining, 8500);
    });
}

// ============================================================================
// L3-R5: chain shorter than configured levels (early break)
// ============================================================================

#[test]
fn l3_r5_chain_shorter_than_levels_breaks_early() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        // buyer=50 -> 40 (only 1 referrer, but 3 tiers configured)
        setup_chain(1, 50, &[40]);

        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 200, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000,
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        // Only 40 gets L1, L2/L3 have no referrer → break
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 1000);
        assert_eq!(remaining, 9000);
    });
}

// ============================================================================
// L4-R5: TokenCommissionPlugin path
// ============================================================================

#[test]
fn l4_r5_token_commission_plugin_works() {
    use pallet_commission_common::TokenCommissionPlugin;

    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40, 30]);

        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000,
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as TokenCommissionPlugin<u64, Balance>>::calculate_token(
            1, &50, 10000u128, 10000u128, modes, false, 0,
        );

        // Token path uses same process_multi_level — results should match NEX path
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 1000);
        assert_eq!(outputs[1].beneficiary, 30);
        assert_eq!(outputs[1].amount, 500);
        assert_eq!(remaining, 8500);
    });
}

// ============================================================================
// F9: is_banned check in process_multi_level
// ============================================================================

#[test]
fn f9_banned_referrer_skipped_in_multi_level() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        // buyer=50 -> 40 -> 30 -> 20
        setup_chain(1, 50, &[40, 30, 20]);

        // Ban account 40 (L1 referrer)
        ban_member(1, 40);

        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 200, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000,
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        // L1: 40 is banned → skipped, L2: 30 gets 500, L3: 20 gets 200
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 30);
        assert_eq!(outputs[0].amount, 500);
        assert_eq!(outputs[0].level, 2);
        assert_eq!(outputs[1].beneficiary, 20);
        assert_eq!(outputs[1].amount, 200);
        assert_eq!(outputs[1].level, 3);
        assert_eq!(remaining, 9300);
    });
}

#[test]
fn f9_non_banned_referrer_still_receives_commission() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40, 30]);

        // No one is banned — normal flow
        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000,
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 1000);
        assert_eq!(outputs[1].beneficiary, 30);
        assert_eq!(outputs[1].amount, 500);
        assert_eq!(remaining, 8500);
    });
}

#[test]
fn f9_all_referrers_banned_returns_empty() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40, 30]);

        ban_member(1, 40);
        ban_member(1, 30);

        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000,
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        // Both referrers banned → no commission
        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

// ============================================================================
// F3: update_multi_level_params — 部分更新
// ============================================================================

#[test]
fn f3_update_max_total_rate_only() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            vec![pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 }].try_into().unwrap(),
            5000,
        ));

        // Update only max_total_rate
        assert_ok!(CommissionMultiLevel::update_multi_level_params(
            RuntimeOrigin::signed(OWNER), 1, Some(8000), None, None,
        ));
        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.max_total_rate, 8000);
        assert_eq!(config.levels[0].rate, 1000); // unchanged
    });
}

#[test]
fn f3_update_tier_only() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            vec![
                pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
                pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
            ].try_into().unwrap(),
            5000,
        ));

        // Update tier at index 1
        assert_ok!(CommissionMultiLevel::update_multi_level_params(
            RuntimeOrigin::signed(ADMIN), 1, None, Some(1),
            Some(pallet::MultiLevelTier { rate: 800, required_directs: 5, required_team_size: 10, required_spent: 100 }),
        ));
        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.max_total_rate, 5000); // unchanged
        assert_eq!(config.levels[0].rate, 1000); // unchanged
        assert_eq!(config.levels[1].rate, 800);
        assert_eq!(config.levels[1].required_directs, 5);
        assert_eq!(config.levels[1].required_team_size, 10);
        assert_eq!(config.levels[1].required_spent, 100);
    });
}

#[test]
fn f3_update_both_rate_and_tier() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            vec![pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 }].try_into().unwrap(),
            5000,
        ));

        assert_ok!(CommissionMultiLevel::update_multi_level_params(
            RuntimeOrigin::signed(OWNER), 1, Some(9000), Some(0),
            Some(pallet::MultiLevelTier { rate: 2000, required_directs: 0, required_team_size: 0, required_spent: 0 }),
        ));
        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.max_total_rate, 9000);
        assert_eq!(config.levels[0].rate, 2000);
    });
}

#[test]
fn f3_nothing_to_update() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            vec![pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 }].try_into().unwrap(),
            5000,
        ));

        assert_noop!(
            CommissionMultiLevel::update_multi_level_params(
                RuntimeOrigin::signed(OWNER), 1, None, None, None,
            ),
            pallet::Error::<Test>::NothingToUpdate
        );
    });
}

#[test]
fn f3_config_not_found() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        assert_noop!(
            CommissionMultiLevel::update_multi_level_params(
                RuntimeOrigin::signed(OWNER), 1, Some(5000), None, None,
            ),
            pallet::Error::<Test>::ConfigNotFound
        );
    });
}

#[test]
fn f3_tier_index_out_of_bounds() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            vec![pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 }].try_into().unwrap(),
            5000,
        ));

        assert_noop!(
            CommissionMultiLevel::update_multi_level_params(
                RuntimeOrigin::signed(OWNER), 1, None, Some(5),
                Some(pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 }),
            ),
            pallet::Error::<Test>::TierIndexOutOfBounds
        );
    });
}

#[test]
fn f3_invalid_rate() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            vec![pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 }].try_into().unwrap(),
            5000,
        ));

        // Invalid max_total_rate
        assert_noop!(
            CommissionMultiLevel::update_multi_level_params(
                RuntimeOrigin::signed(OWNER), 1, Some(0), None, None,
            ),
            pallet::Error::<Test>::InvalidRate
        );

        // Invalid tier rate
        assert_noop!(
            CommissionMultiLevel::update_multi_level_params(
                RuntimeOrigin::signed(OWNER), 1, None, Some(0),
                Some(pallet::MultiLevelTier { rate: 10001, required_directs: 0, required_team_size: 0, required_spent: 0 }),
            ),
            pallet::Error::<Test>::InvalidRate
        );
    });
}

// ============================================================================
// F4: add_tier / remove_tier
// ============================================================================

#[test]
fn f4_add_tier_appends() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            vec![pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 }].try_into().unwrap(),
            5000,
        ));

        // Append at end (index = 1 = len)
        assert_ok!(CommissionMultiLevel::add_tier(
            RuntimeOrigin::signed(OWNER), 1, 1,
            pallet::MultiLevelTier { rate: 500, required_directs: 3, required_team_size: 0, required_spent: 0 },
        ));
        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.levels.len(), 2);
        assert_eq!(config.levels[1].rate, 500);
        assert_eq!(config.levels[1].required_directs, 3);
    });
}

#[test]
fn f4_add_tier_inserts_at_beginning() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            vec![pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 }].try_into().unwrap(),
            5000,
        ));

        // Insert at beginning
        assert_ok!(CommissionMultiLevel::add_tier(
            RuntimeOrigin::signed(ADMIN), 1, 0,
            pallet::MultiLevelTier { rate: 200, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ));
        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.levels.len(), 2);
        assert_eq!(config.levels[0].rate, 200);
        assert_eq!(config.levels[1].rate, 1000); // shifted
    });
}

#[test]
fn f4_add_tier_rejects_over_limit() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        // Fill up to MaxMultiLevels (15)
        let levels: Vec<_> = (0..15).map(|_| pallet::MultiLevelTier { rate: 100, required_directs: 0, required_team_size: 0, required_spent: 0 }).collect();
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            levels.try_into().unwrap(),
            5000,
        ));

        assert_noop!(
            CommissionMultiLevel::add_tier(
                RuntimeOrigin::signed(OWNER), 1, 15,
                pallet::MultiLevelTier { rate: 100, required_directs: 0, required_team_size: 0, required_spent: 0 },
            ),
            pallet::Error::<Test>::TierLimitExceeded
        );
    });
}

#[test]
fn f4_add_tier_index_out_of_bounds() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            vec![pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 }].try_into().unwrap(),
            5000,
        ));

        // index = 2 > len = 1
        assert_noop!(
            CommissionMultiLevel::add_tier(
                RuntimeOrigin::signed(OWNER), 1, 2,
                pallet::MultiLevelTier { rate: 100, required_directs: 0, required_team_size: 0, required_spent: 0 },
            ),
            pallet::Error::<Test>::TierIndexOutOfBounds
        );
    });
}

#[test]
fn f4_remove_tier_works() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            vec![
                pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
                pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
                pallet::MultiLevelTier { rate: 200, required_directs: 0, required_team_size: 0, required_spent: 0 },
            ].try_into().unwrap(),
            5000,
        ));

        // Remove middle tier (index 1)
        assert_ok!(CommissionMultiLevel::remove_tier(
            RuntimeOrigin::signed(OWNER), 1, 1,
        ));
        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.levels.len(), 2);
        assert_eq!(config.levels[0].rate, 1000);
        assert_eq!(config.levels[1].rate, 200); // was at index 2, now 1
    });
}

#[test]
fn f4_remove_last_tier_rejected() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            vec![pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 }].try_into().unwrap(),
            5000,
        ));

        // Cannot remove last tier
        assert_noop!(
            CommissionMultiLevel::remove_tier(RuntimeOrigin::signed(OWNER), 1, 0),
            pallet::Error::<Test>::EmptyLevels
        );
    });
}

#[test]
fn f4_remove_tier_index_out_of_bounds() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            vec![
                pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
                pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
            ].try_into().unwrap(),
            5000,
        ));

        assert_noop!(
            CommissionMultiLevel::remove_tier(RuntimeOrigin::signed(OWNER), 1, 5),
            pallet::Error::<Test>::TierIndexOutOfBounds
        );
    });
}

// ============================================================================
// F7: PlanWriter set_multi_level_full
// ============================================================================

#[test]
fn f7_plan_writer_set_multi_level_full_works() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        let tiers = vec![
            (1000u16, 5u32, 10u32, 500u128),
            (500, 3, 0, 0),
        ];
        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level_full(1, tiers, 3000));

        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.levels.len(), 2);
        assert_eq!(config.levels[0].rate, 1000);
        assert_eq!(config.levels[0].required_directs, 5);
        assert_eq!(config.levels[0].required_team_size, 10);
        assert_eq!(config.levels[0].required_spent, 500);
        assert_eq!(config.levels[1].rate, 500);
        assert_eq!(config.levels[1].required_directs, 3);
        assert_eq!(config.max_total_rate, 3000);
    });
}

#[test]
fn f7_plan_writer_set_multi_level_full_rejects_empty() {
    new_test_ext().execute_with(|| {
        assert!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level_full(1, vec![], 3000).is_err());
    });
}

#[test]
fn f7_plan_writer_set_multi_level_full_rejects_invalid_rate() {
    new_test_ext().execute_with(|| {
        assert!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level_full(1, vec![(10001, 0, 0, 0)], 3000).is_err());
        assert!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level_full(1, vec![(100, 0, 0, 0)], 0).is_err());
    });
}

// ============================================================================
// F10: is_member check in process_multi_level
// ============================================================================

#[test]
fn f10_non_member_referrer_skipped() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40, 30, 20]);

        // Mark referrer 40 as non-member
        set_non_member(1, 40);

        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 200, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000,
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        // L1: 40 is non-member → skipped, L2: 30 gets 500, L3: 20 gets 200
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 30);
        assert_eq!(outputs[0].amount, 500);
        assert_eq!(outputs[1].beneficiary, 20);
        assert_eq!(outputs[1].amount, 200);
        assert_eq!(remaining, 9300);
    });
}

#[test]
fn f10_all_referrers_non_member_returns_empty() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40, 30]);

        set_non_member(1, 40);
        set_non_member(1, 30);

        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000,
        });

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

// ============================================================================
// F11: check_activation_status helper
// ============================================================================

#[test]
fn f11_get_activation_status_no_config() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        let status = pallet::Pallet::<Test>::get_activation_status(1, &50);
        assert!(status.is_empty());
    });
}

#[test]
fn f11_get_activation_status_all_pass() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        // No activation conditions → all pass
        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000,
        });

        let status = pallet::Pallet::<Test>::get_activation_status(1, &50);
        assert_eq!(status, vec![true, true]);
    });
}

#[test]
fn f11_get_activation_status_partial() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        // Set stats: 50 has 3 direct referrals, team 5
        set_stats(1, 50, 3, 5, 0);

        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 2, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 500, required_directs: 5, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 200, required_directs: 0, required_team_size: 10, required_spent: 0 },
        ];
        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000,
        });

        let status = pallet::Pallet::<Test>::get_activation_status(1, &50);
        // L1: 3 >= 2 → true, L2: 3 < 5 → false, L3: 5 < 10 → false
        assert_eq!(status, vec![true, false, false]);
    });
}

// ============================================================================
// F12: Entity activation check in calculate
// ============================================================================

#[test]
fn f12_inactive_entity_skips_commission() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40, 30]);

        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000,
        });

        // Mark entity 1 as inactive
        set_entity_inactive(1);

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        // Entity inactive → no commission
        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

#[test]
fn f12_active_entity_calculates_normally() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40, 30]);

        let levels = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        pallet::MultiLevelConfigs::<Test>::insert(1, pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
            max_total_rate: 3000,
        });

        // Entity 1 is active by default (not in INACTIVE_ENTITIES)
        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10000, 10000, modes, false, 0,
        );

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 1000);
        assert_eq!(outputs[1].beneficiary, 30);
        assert_eq!(outputs[1].amount, 500);
        assert_eq!(remaining, 8500);
    });
}

// ==================== EntityLocked 回归测试 ====================

#[test]
fn entity_locked_rejects_set_multi_level_config() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        set_entity_locked(1);
        let tiers = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        assert_noop!(
            CommissionMultiLevel::set_multi_level_config(
                RuntimeOrigin::signed(OWNER), 1,
                tiers.try_into().unwrap(), 5000,
            ),
            pallet::Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_clear_multi_level_config() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        let tiers = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            tiers.try_into().unwrap(), 5000,
        ));
        set_entity_locked(1);
        assert_noop!(
            CommissionMultiLevel::clear_multi_level_config(RuntimeOrigin::signed(OWNER), 1),
            pallet::Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_add_tier() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        let tiers = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            tiers.try_into().unwrap(), 5000,
        ));
        set_entity_locked(1);
        let new_tier = pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 };
        assert_noop!(
            CommissionMultiLevel::add_tier(RuntimeOrigin::signed(OWNER), 1, 1, new_tier),
            pallet::Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_remove_tier() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        let tiers = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
            pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            tiers.try_into().unwrap(), 5000,
        ));
        set_entity_locked(1);
        assert_noop!(
            CommissionMultiLevel::remove_tier(RuntimeOrigin::signed(OWNER), 1, 0),
            pallet::Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_update_multi_level_params() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        let tiers = vec![
            pallet::MultiLevelTier { rate: 1000, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER), 1,
            tiers.try_into().unwrap(), 5000,
        ));
        set_entity_locked(1);
        assert_noop!(
            CommissionMultiLevel::update_multi_level_params(
                RuntimeOrigin::signed(OWNER), 1, Some(8000), None, None,
            ),
            pallet::Error::<Test>::EntityLocked
        );
    });
}

// ==================== ConfigNotFound 回归测试 (L5-R4) ====================

#[test]
fn add_tier_rejects_no_config() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        // No config set for entity 1
        assert_noop!(
            CommissionMultiLevel::add_tier(
                RuntimeOrigin::signed(OWNER), 1, 0,
                pallet::MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
            ),
            pallet::Error::<Test>::ConfigNotFound
        );
    });
}

#[test]
fn remove_tier_rejects_no_config() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        // No config set for entity 1
        assert_noop!(
            CommissionMultiLevel::remove_tier(RuntimeOrigin::signed(OWNER), 1, 0),
            pallet::Error::<Test>::ConfigNotFound
        );
    });
}
