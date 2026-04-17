use crate::mock::*;
use crate::pallet;
use frame_support::{assert_noop, assert_ok, traits::Hooks};
use pallet_commission_common::{CommissionModes, CommissionPlugin, MultiLevelPlanWriter};

const OWNER: u64 = 100;
const ADMIN: u64 = 200;
const NOBODY: u64 = 999;

fn setup_entity(entity_id: u64) {
    set_entity_owner(entity_id, OWNER);
    set_entity_admin(
        entity_id,
        ADMIN,
        pallet_entity_common::AdminPermission::COMMISSION_MANAGE,
    );
}

// ============================================================================
// Extrinsic tests — set_multi_level_config (Owner/Admin)
// ============================================================================

#[test]
fn set_multi_level_config_works_by_owner() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        let tiers = vec![
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 3,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));
        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.levels.len(), 2);
    });
}

#[test]
fn set_multi_level_config_works_by_admin_with_commission_manage() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(ADMIN),
            1,
            tiers.try_into().unwrap(),
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
        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_noop!(
            CommissionMultiLevel::set_multi_level_config(
                RuntimeOrigin::signed(ADMIN),
                1,
                tiers.try_into().unwrap(),
            ),
            pallet::Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

#[test]
fn set_multi_level_config_rejects_invalid_tier_rate() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        let tiers = vec![pallet::MultiLevelTier {
            rate: 10001,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_noop!(
            CommissionMultiLevel::set_multi_level_config(
                RuntimeOrigin::signed(OWNER),
                1,
                tiers.try_into().unwrap()
            ),
            pallet::Error::<Test>::InvalidRate
        );
    });
}

#[test]
fn set_multi_level_config_rejects_entity_not_found() {
    new_test_ext().execute_with(|| {
        // Entity 999 does not exist
        let tiers = vec![pallet::MultiLevelTier {
            rate: 500,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_noop!(
            CommissionMultiLevel::set_multi_level_config(
                RuntimeOrigin::signed(NOBODY),
                999,
                tiers.try_into().unwrap()
            ),
            pallet::Error::<Test>::EntityNotFound
        );
    });
}

#[test]
fn set_multi_level_config_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        let tiers = vec![pallet::MultiLevelTier {
            rate: 500,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_noop!(
            CommissionMultiLevel::set_multi_level_config(
                RuntimeOrigin::signed(NOBODY),
                1,
                tiers.try_into().unwrap()
            ),
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
        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_some());

        assert_ok!(CommissionMultiLevel::clear_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
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
        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: vec![pallet::MultiLevelTier {
                    rate: 1000,
                    required_directs: 0,
                    required_team_size: 0,
                    required_spent: 0,
                    required_level_id: 0,
                }]
                .try_into()
                .unwrap(),
            },
        );
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
        clear_thread_locals();
        setup_entity(1);
        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::force_set_multi_level_config(
            RuntimeOrigin::root(),
            1,
            tiers.try_into().unwrap(),
        ));
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_some());
    });
}

#[test]
fn force_set_multi_level_config_rejects_non_root() {
    new_test_ext().execute_with(|| {
        let tiers = vec![pallet::MultiLevelTier {
            rate: 500,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_noop!(
            CommissionMultiLevel::force_set_multi_level_config(
                RuntimeOrigin::signed(1),
                1,
                tiers.try_into().unwrap(),
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn force_clear_multi_level_config_works() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: vec![pallet::MultiLevelTier {
                    rate: 1000,
                    required_directs: 0,
                    required_team_size: 0,
                    required_spent: 0,
                    required_level_id: 0,
                }]
                .try_into()
                .unwrap(),
            },
        );
        assert_ok!(CommissionMultiLevel::force_clear_multi_level_config(
            RuntimeOrigin::root(),
            1
        ));
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_none());
    });
}

#[test]
fn force_clear_multi_level_config_idempotent() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        // No config exists — should succeed silently without event (entity exists but no config)
        assert_ok!(CommissionMultiLevel::force_clear_multi_level_config(
            RuntimeOrigin::root(),
            1
        ));
        let events = System::events();
        assert!(!events.iter().any(|e| matches!(
            e.event,
            RuntimeEvent::CommissionMultiLevel(pallet::Event::MultiLevelConfigCleared { .. })
        )));
    });
}

#[test]
fn v6_force_clear_rejects_nonexistent_entity() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        // entity 999 不存在
        assert_noop!(
            CommissionMultiLevel::force_clear_multi_level_config(RuntimeOrigin::root(), 999),
            pallet::Error::<Test>::EntityNotFound
        );
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
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            }, // L1: 10%
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            }, // L2: 5%
            pallet::MultiLevelTier {
                rate: 200,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            }, // L3: 2%
        ];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
fn multi_level_activation_condition() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40, 30]);

        // account 40: 2 direct referrals (不满足 required_directs=3)
        set_stats(1, 40, 2, 10, 5000);
        // account 30: 5 direct referrals (满足)
        set_stats(1, 30, 5, 20, 10000);

        let levels = vec![
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 3,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 3,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: vec![pallet::MultiLevelTier {
                    rate: 1000,
                    required_directs: 0,
                    required_team_size: 0,
                    required_spent: 0,
                    required_level_id: 0,
                }]
                .try_into()
                .unwrap(),
            },
        );

        // DIRECT_REWARD mode, but we are the MultiLevel plugin
        let modes = CommissionModes(CommissionModes::DIRECT_REWARD);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
        assert_ok!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![500, 300, 200],)
        );
        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.levels.len(), 3);
        assert_eq!(config.levels[0].rate, 500);
    });
}

#[test]
fn h2_plan_writer_rejects_invalid_level_rate() {
    new_test_ext().execute_with(|| {
        assert!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![10001],)
                .is_err()
        );
    });
}

#[test]
fn plan_writer_too_many_levels() {
    new_test_ext().execute_with(|| {
        // MaxMultiLevels = 1000, try 1001
        let rates: Vec<u16> = (0..1001).map(|_| 1).collect();
        assert!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, rates,).is_err()
        );
    });
}

#[test]
fn plan_writer_clear_config() {
    new_test_ext().execute_with(|| {
        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![500],));
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
        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 100_000_000,
            required_level_id: 0,
        }];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 100_000_000,
            required_level_id: 0,
        }];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 10,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 10,
                required_spent: 0,
                required_level_id: 0,
            },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 3,
            required_team_size: 10,
            required_spent: 100_000_000,
            required_level_id: 0,
        }];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 3,
            required_team_size: 10,
            required_spent: 100_000_000,
            required_level_id: 0,
        }];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
        assert_ok!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![500, 300],)
        );
        let events = System::events();
        assert!(events.iter().any(|e| matches!(
            e.event,
            RuntimeEvent::CommissionMultiLevel(pallet::Event::MultiLevelConfigUpdated {
                entity_id: 1
            })
        )));
    });
}

#[test]
fn m2_r2_plan_writer_clear_emits_cleared_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![500],));
        System::reset_events();
        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::clear_multi_level_config(1));
        let events = System::events();
        assert!(events.iter().any(|e| matches!(
            e.event,
            RuntimeEvent::CommissionMultiLevel(pallet::Event::MultiLevelConfigCleared {
                entity_id: 1
            })
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
            CommissionMultiLevel::set_multi_level_config(
                RuntimeOrigin::signed(OWNER),
                1,
                empty.try_into().unwrap()
            ),
            pallet::Error::<Test>::EmptyLevels
        );
    });
}

#[test]
fn l1_r2_plan_writer_rejects_empty_levels() {
    new_test_ext().execute_with(|| {
        assert!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![],).is_err()
        );
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
            pallet::MultiLevelTier {
                rate: 0,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            }, // L1: placeholder
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            }, // L2: 10%
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            }, // L3: 5%
        ];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 200,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as TokenCommissionPlugin<u64, Balance>>::calculate_token(
                1, &50, 10000u128, 10000u128, modes, false, 0, 0,
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
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 200,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
fn f3_update_tier_only() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            vec![
                pallet::MultiLevelTier {
                    rate: 1000,
                    required_directs: 0,
                    required_team_size: 0,
                    required_spent: 0,
                    required_level_id: 0
                },
                pallet::MultiLevelTier {
                    rate: 500,
                    required_directs: 0,
                    required_team_size: 0,
                    required_spent: 0,
                    required_level_id: 0
                },
            ]
            .try_into()
            .unwrap(),
        ));

        // Update tier at index 1
        assert_ok!(CommissionMultiLevel::update_multi_level_params(
            RuntimeOrigin::signed(ADMIN),
            1,
            Some(1),
            Some(pallet::MultiLevelTier {
                rate: 800,
                required_directs: 5,
                required_team_size: 10,
                required_spent: 100,
                required_level_id: 0
            }),
        ));
        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.levels[0].rate, 1000); // unchanged
        assert_eq!(config.levels[1].rate, 800);
        assert_eq!(config.levels[1].required_directs, 5);
        assert_eq!(config.levels[1].required_team_size, 10);
        assert_eq!(config.levels[1].required_spent, 100);
    });
}

#[test]
fn f3_update_tier_rate() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            vec![pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0
            }]
            .try_into()
            .unwrap(),
        ));

        assert_ok!(CommissionMultiLevel::update_multi_level_params(
            RuntimeOrigin::signed(OWNER),
            1,
            Some(0),
            Some(pallet::MultiLevelTier {
                rate: 2000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0
            }),
        ));
        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.levels[0].rate, 2000);
    });
}

#[test]
fn f3_nothing_to_update() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            vec![pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0
            }]
            .try_into()
            .unwrap(),
        ));

        assert_noop!(
            CommissionMultiLevel::update_multi_level_params(
                RuntimeOrigin::signed(OWNER),
                1,
                None,
                None,
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
                RuntimeOrigin::signed(OWNER),
                1,
                Some(0),
                Some(pallet::MultiLevelTier {
                    rate: 500,
                    required_directs: 0,
                    required_team_size: 0,
                    required_spent: 0,
                    required_level_id: 0
                }),
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
            RuntimeOrigin::signed(OWNER),
            1,
            vec![pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0
            }]
            .try_into()
            .unwrap(),
        ));

        assert_noop!(
            CommissionMultiLevel::update_multi_level_params(
                RuntimeOrigin::signed(OWNER),
                1,
                Some(5),
                Some(pallet::MultiLevelTier {
                    rate: 500,
                    required_directs: 0,
                    required_team_size: 0,
                    required_spent: 0,
                    required_level_id: 0
                }),
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
            RuntimeOrigin::signed(OWNER),
            1,
            vec![pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0
            }]
            .try_into()
            .unwrap(),
        ));

        // Invalid tier rate
        assert_noop!(
            CommissionMultiLevel::update_multi_level_params(
                RuntimeOrigin::signed(OWNER),
                1,
                Some(0),
                Some(pallet::MultiLevelTier {
                    rate: 10001,
                    required_directs: 0,
                    required_team_size: 0,
                    required_spent: 0,
                    required_level_id: 0
                }),
            ),
            pallet::Error::<Test>::InvalidRate
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
        let tiers = vec![(1000u16, 5u32, 10u32, 500u128, 0u8), (500, 3, 0, 0, 0)];
        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level_full(1, tiers));

        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.levels.len(), 2);
        assert_eq!(config.levels[0].rate, 1000);
        assert_eq!(config.levels[0].required_directs, 5);
        assert_eq!(config.levels[0].required_team_size, 10);
        assert_eq!(config.levels[0].required_spent, 500);
        assert_eq!(config.levels[1].rate, 500);
        assert_eq!(config.levels[1].required_directs, 3);
    });
}

#[test]
fn f7_plan_writer_set_multi_level_full_rejects_empty() {
    new_test_ext().execute_with(|| {
        assert!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level_full(1, vec![])
                .is_err()
        );
    });
}

#[test]
fn f7_plan_writer_set_multi_level_full_rejects_invalid_rate() {
    new_test_ext().execute_with(|| {
        assert!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level_full(
                1,
                vec![(10001, 0, 0, 0, 0)]
            )
            .is_err()
        );
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
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 200,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

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
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 2,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 5,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 200,
                required_directs: 0,
                required_team_size: 10,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

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
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        // Mark entity 1 as inactive
        set_entity_inactive(1);

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        // Entity 1 is active by default (not in INACTIVE_ENTITIES)
        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
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
        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_noop!(
            CommissionMultiLevel::set_multi_level_config(
                RuntimeOrigin::signed(OWNER),
                1,
                tiers.try_into().unwrap(),
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
        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));
        set_entity_locked(1);
        assert_noop!(
            CommissionMultiLevel::clear_multi_level_config(RuntimeOrigin::signed(OWNER), 1),
            pallet::Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_update_multi_level_params() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));
        set_entity_locked(1);
        assert_noop!(
            CommissionMultiLevel::update_multi_level_params(
                RuntimeOrigin::signed(OWNER),
                1,
                Some(0),
                Some(pallet::MultiLevelTier {
                    rate: 2000,
                    required_directs: 0,
                    required_team_size: 0,
                    required_spent: 0,
                    required_level_id: 0
                }),
            ),
            pallet::Error::<Test>::EntityLocked
        );
    });
}

// ==================== ConfigNotFound 回归测试 (L5-R4) ====================

// ============================================================================
// F10: 全局暂停开关
// ============================================================================

#[test]
fn f10_pause_multi_level_works() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        assert_ok!(CommissionMultiLevel::pause_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));
        assert!(pallet::GlobalPaused::<Test>::get(1));

        // Cannot pause again
        assert_noop!(
            CommissionMultiLevel::pause_multi_level(RuntimeOrigin::signed(OWNER), 1),
            pallet::Error::<Test>::MultiLevelIsPaused
        );
    });
}

#[test]
fn f10_resume_multi_level_works() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        assert_ok!(CommissionMultiLevel::pause_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));
        assert_ok!(CommissionMultiLevel::resume_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));
        assert!(!pallet::GlobalPaused::<Test>::get(1));
    });
}

#[test]
fn f10_resume_when_not_paused_fails_with_multi_level_not_paused() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        assert_noop!(
            CommissionMultiLevel::resume_multi_level(RuntimeOrigin::signed(OWNER), 1),
            pallet::Error::<Test>::MultiLevelNotPaused
        );
    });
}

#[test]
fn f10_paused_entity_skips_commission() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40, 30]);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        // Pause
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::pause_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0, 0,
            );

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

#[test]
fn f10_admin_can_pause_resume() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        assert_ok!(CommissionMultiLevel::pause_multi_level(
            RuntimeOrigin::signed(ADMIN),
            1
        ));
        assert!(pallet::GlobalPaused::<Test>::get(1));
        assert_ok!(CommissionMultiLevel::resume_multi_level(
            RuntimeOrigin::signed(ADMIN),
            1
        ));
        assert!(!pallet::GlobalPaused::<Test>::get(1));
    });
}

// ============================================================================
// F1: 配置变更生效延迟
// ============================================================================

#[test]
fn f1_schedule_config_change_works() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        // Pending config exists
        let pending = pallet::PendingConfigs::<Test>::get(1).unwrap();
        assert_eq!(pending.effective_at, 11); // block 1 + delay 10
    });
}

#[test]
fn f1_schedule_rejects_if_pending_exists() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.clone().try_into().unwrap(),
        ));
        assert_noop!(
            CommissionMultiLevel::schedule_config_change(
                RuntimeOrigin::signed(OWNER),
                1,
                levels.try_into().unwrap(),
            ),
            pallet::Error::<Test>::PendingConfigExists
        );
    });
}

#[test]
fn f1_apply_pending_config_works() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        // V1-FIX: 非 Owner/Admin 不能触发应用
        assert_noop!(
            CommissionMultiLevel::apply_pending_config(RuntimeOrigin::signed(NOBODY), 1),
            pallet::Error::<Test>::NotEntityOwnerOrAdmin
        );

        // Cannot apply before effective_at (even as Owner)
        assert_noop!(
            CommissionMultiLevel::apply_pending_config(RuntimeOrigin::signed(OWNER), 1),
            pallet::Error::<Test>::PendingConfigNotReady
        );

        // Advance to effective block
        System::set_block_number(11);

        assert_ok!(CommissionMultiLevel::apply_pending_config(
            RuntimeOrigin::signed(OWNER),
            1
        ));

        // Config applied
        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.levels.len(), 1);

        // Pending removed
        assert!(pallet::PendingConfigs::<Test>::get(1).is_none());
    });
}

#[test]
fn f1_cancel_pending_config_works() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        assert_ok!(CommissionMultiLevel::cancel_pending_config(
            RuntimeOrigin::signed(OWNER),
            1
        ));
        assert!(pallet::PendingConfigs::<Test>::get(1).is_none());
    });
}

#[test]
fn f1_cancel_no_pending_fails() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        assert_noop!(
            CommissionMultiLevel::cancel_pending_config(RuntimeOrigin::signed(OWNER), 1),
            pallet::Error::<Test>::NoPendingConfig
        );
    });
}

#[test]
fn f1_apply_no_pending_fails() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        assert_noop!(
            CommissionMultiLevel::apply_pending_config(RuntimeOrigin::signed(OWNER), 1),
            pallet::Error::<Test>::NoPendingConfig
        );
    });
}

// ============================================================================
// F2: 配置变更审计日志
// ============================================================================

#[test]
fn f2_set_config_records_audit_log() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        assert_eq!(pallet::ConfigChangeLogCount::<Test>::get(1), 1);
        let log = pallet::ConfigChangeLogs::<Test>::get(1, 0).unwrap();
        assert_eq!(log.who, OWNER);
        assert_eq!(log.change_type, pallet::ConfigChangeType::SetConfig);
    });
}

#[test]
fn f2_clear_config_records_audit_log() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));
        assert_ok!(CommissionMultiLevel::clear_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
        ));

        assert_eq!(pallet::ConfigChangeLogCount::<Test>::get(1), 2);
        let log = pallet::ConfigChangeLogs::<Test>::get(1, 1).unwrap();
        assert_eq!(log.change_type, pallet::ConfigChangeType::ClearConfig);
    });
}

#[test]
fn f2_pause_resume_records_audit_log() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        assert_ok!(CommissionMultiLevel::pause_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));
        assert_ok!(CommissionMultiLevel::resume_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));

        assert_eq!(pallet::ConfigChangeLogCount::<Test>::get(1), 2);
        let log0 = pallet::ConfigChangeLogs::<Test>::get(1, 0).unwrap();
        assert_eq!(log0.change_type, pallet::ConfigChangeType::Pause);
        let log1 = pallet::ConfigChangeLogs::<Test>::get(1, 1).unwrap();
        assert_eq!(log1.change_type, pallet::ConfigChangeType::Resume);
    });
}

// ============================================================================
// F4: rates 总和超 10000 警告
// ============================================================================

#[test]
fn f4_rates_sum_exceeds_max_emits_warning() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        // rates sum = 6000 + 5000 = 11000 > 10000 → warning
        let levels = vec![
            pallet::MultiLevelTier {
                rate: 6000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 5000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        // Check RatesSumWarning event emitted
        let events = System::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(pallet::Event::RatesSumWarning {
                    entity_id: 1,
                    rates_sum: 11000
                })
            )
        });
        assert!(found, "RatesSumWarning event should be emitted");
    });
}

#[test]
fn f4_rates_sum_within_max_no_warning() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        // rates sum = 500 < 10000 → no warning
        let levels = vec![pallet::MultiLevelTier {
            rate: 500,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        let events = System::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(pallet::Event::RatesSumWarning { .. })
            )
        });
        assert!(!found, "RatesSumWarning event should NOT be emitted");
    });
}

// ============================================================================
// F5: 激活进度查询
// ============================================================================

#[test]
fn f5_get_activation_progress_no_config() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        let progress = pallet::Pallet::<Test>::get_activation_progress(1, &50);
        assert!(progress.is_empty());
    });
}

#[test]
fn f5_get_activation_progress_with_data() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        set_stats(1, 50, 3, 5, 0);
        set_spent_usdt(1, 50, 1000);

        let levels = vec![
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 2,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 5,
                required_team_size: 10,
                required_spent: 500,
                required_level_id: 0,
            },
        ];
        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let progress = pallet::Pallet::<Test>::get_activation_progress(1, &50);
        assert_eq!(progress.len(), 2);

        // L1: directs 3 >= 2 → activated
        assert_eq!(progress[0].level, 1);
        assert!(progress[0].activated);
        assert_eq!(progress[0].directs_current, 3);
        assert_eq!(progress[0].directs_required, 2);

        // L2: directs 3 < 5 → not activated
        assert_eq!(progress[1].level, 2);
        assert!(!progress[1].activated);
        assert_eq!(progress[1].directs_required, 5);
        assert_eq!(progress[1].team_required, 10);
        assert_eq!(progress[1].spent_current, 1000);
        assert_eq!(progress[1].spent_required, 500);
    });
}

// ============================================================================
// F7: 详细配置变更事件
// ============================================================================

#[test]
fn f7_set_config_emits_detailed_change() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        let events = System::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(pallet::Event::ConfigDetailedChange {
                    entity_id: 1,
                    old_levels_count: 0,
                    new_levels_count: 2,
                })
            )
        });
        assert!(found, "ConfigDetailedChange event should be emitted");
    });
}

#[test]
fn f7_overwrite_config_shows_old_values() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        // First config: 1 level, rate 2000
        let levels1 = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            levels1.try_into().unwrap(),
        ));

        // Clear events
        System::reset_events();

        // Second config: 3 levels, rate 5000
        let levels2 = vec![
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 300,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 200,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            levels2.try_into().unwrap(),
        ));

        let events = System::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(pallet::Event::ConfigDetailedChange {
                    entity_id: 1,
                    old_levels_count: 1,
                    new_levels_count: 3,
                })
            )
        });
        assert!(found, "ConfigDetailedChange event should show old values");
    });
}

// ============================================================================
// F6/F13: 佣金统计
// ============================================================================

#[test]
fn f6_f13_update_stats_works() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        use pallet_commission_common::{CommissionOutput, CommissionType};

        let outputs = vec![
            CommissionOutput {
                beneficiary: 40u64,
                amount: 1000u128,
                commission_type: CommissionType::MultiLevel,
                level: 1,
            },
            CommissionOutput {
                beneficiary: 30u64,
                amount: 500u128,
                commission_type: CommissionType::MultiLevel,
                level: 2,
            },
        ];

        pallet::Pallet::<Test>::update_stats(1, &outputs, true);

        // F6: Member stats
        let stats40 = pallet::MemberMultiLevelStats::<Test>::get(1, 40u64);
        assert_eq!(stats40.total_earned, 1000);
        assert_eq!(stats40.commission_receipt_count, 1);

        let stats30 = pallet::MemberMultiLevelStats::<Test>::get(1, 30u64);
        assert_eq!(stats30.total_earned, 500);
        assert_eq!(stats30.commission_receipt_count, 1);

        // F13: Entity stats
        let entity_stats = pallet::EntityMultiLevelStats::<Test>::get(1);
        assert_eq!(entity_stats.total_distributed, 1500);
        assert_eq!(entity_stats.order_count, 1);
        assert_eq!(entity_stats.total_distribution_entries, 2);
    });
}

#[test]
fn f6_f13_stats_accumulate() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        use pallet_commission_common::{CommissionOutput, CommissionType};

        let outputs1 = vec![CommissionOutput {
            beneficiary: 40u64,
            amount: 1000u128,
            commission_type: CommissionType::MultiLevel,
            level: 1,
        }];
        pallet::Pallet::<Test>::update_stats(1, &outputs1, true);

        let outputs2 = vec![
            CommissionOutput {
                beneficiary: 40u64,
                amount: 2000u128,
                commission_type: CommissionType::MultiLevel,
                level: 1,
            },
            CommissionOutput {
                beneficiary: 30u64,
                amount: 500u128,
                commission_type: CommissionType::MultiLevel,
                level: 2,
            },
        ];
        pallet::Pallet::<Test>::update_stats(1, &outputs2, true);

        let stats40 = pallet::MemberMultiLevelStats::<Test>::get(1, 40u64);
        assert_eq!(stats40.total_earned, 3000);
        assert_eq!(stats40.commission_receipt_count, 2);

        let entity_stats = pallet::EntityMultiLevelStats::<Test>::get(1);
        assert_eq!(entity_stats.total_distributed, 3500);
        assert_eq!(entity_stats.order_count, 2);
        assert_eq!(entity_stats.total_distribution_entries, 3);
    });
}

#[test]
fn f6_f13_empty_outputs_no_op() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();

        pallet::Pallet::<Test>::update_stats(1, &[], true);

        let entity_stats = pallet::EntityMultiLevelStats::<Test>::get(1);
        assert_eq!(entity_stats.total_distributed, 0);
        assert_eq!(entity_stats.order_count, 0);
    });
}

// ============================================================================
// F8: 佣金透明度查询 (preview_commission)
// ============================================================================

#[test]
fn f8_preview_commission_works() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_chain(1, 50, &[40, 30]);

        let levels = vec![
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let preview = pallet::Pallet::<Test>::preview_commission(1, &50, 10000);
        assert_eq!(preview.len(), 2);
        assert_eq!(preview[0], (40, 1000, 1));
        assert_eq!(preview[1], (30, 500, 2));
    });
}

#[test]
fn f8_preview_commission_no_config() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        let preview = pallet::Pallet::<Test>::preview_commission(1, &50, 10000);
        assert!(preview.is_empty());
    });
}

#[test]
fn f8_preview_commission_paused() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        setup_chain(1, 50, &[40]);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        assert_ok!(CommissionMultiLevel::pause_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));

        let preview = pallet::Pallet::<Test>::preview_commission(1, &50, 10000);
        assert!(preview.is_empty());
    });
}

// ============================================================================
// F10: is_paused 查询
// ============================================================================

#[test]
fn f10_is_paused_query() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        assert!(!pallet::Pallet::<Test>::is_paused(1));
        assert_ok!(CommissionMultiLevel::pause_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));
        assert!(pallet::Pallet::<Test>::is_paused(1));
        assert_ok!(CommissionMultiLevel::resume_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));
        assert!(!pallet::Pallet::<Test>::is_paused(1));
    });
}

// ============================================================================
// F1: apply_pending_config 触发 F4/F7 事件
// ============================================================================

#[test]
fn f1_apply_pending_emits_detailed_change_and_rates_warning() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        // Set initial config
        let levels1 = vec![pallet::MultiLevelTier {
            rate: 500,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            levels1.try_into().unwrap(),
        ));

        // Schedule change with rates_sum > 10000 to trigger warning
        let levels2 = vec![
            pallet::MultiLevelTier {
                rate: 6000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 5000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            levels2.try_into().unwrap(),
        ));

        System::set_block_number(11);
        System::reset_events();

        assert_ok!(CommissionMultiLevel::apply_pending_config(
            RuntimeOrigin::signed(OWNER),
            1
        ));

        let events = System::events();

        // F4: RatesSumWarning
        let rates_warning = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(pallet::Event::RatesSumWarning {
                    entity_id: 1,
                    rates_sum: 11000
                })
            )
        });
        assert!(rates_warning, "RatesSumWarning should be emitted on apply");

        // F7: ConfigDetailedChange
        let detailed = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(pallet::Event::ConfigDetailedChange {
                    entity_id: 1,
                    old_levels_count: 1,
                    new_levels_count: 2,
                })
            )
        });
        assert!(detailed, "ConfigDetailedChange should be emitted on apply");
    });
}

// ============================================================================
// F2: update_multi_level_params 审计日志
// ============================================================================

#[test]
fn f2_update_params_records_audit_log() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        assert_ok!(CommissionMultiLevel::update_multi_level_params(
            RuntimeOrigin::signed(OWNER),
            1,
            Some(0),
            Some(pallet::MultiLevelTier {
                rate: 2000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0
            }),
        ));

        assert_eq!(pallet::ConfigChangeLogCount::<Test>::get(1), 2);
        let log = pallet::ConfigChangeLogs::<Test>::get(1, 1).unwrap();
        assert_eq!(log.change_type, pallet::ConfigChangeType::UpdateParams);
    });
}

// ============================================================================
// Round 7 回归测试
// ============================================================================

#[test]
fn m1_r7_force_set_emits_detailed_change_event() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        // First force_set (no previous config)
        let levels1 = vec![pallet::MultiLevelTier {
            rate: 500,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::force_set_multi_level_config(
            RuntimeOrigin::root(),
            1,
            levels1.try_into().unwrap(),
        ));

        let events = System::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(pallet::Event::ConfigDetailedChange {
                    entity_id: 1,
                    old_levels_count: 0,
                    new_levels_count: 1,
                })
            )
        });
        assert!(
            found,
            "ConfigDetailedChange should be emitted on force_set (no previous)"
        );

        // Second force_set (overwrite existing)
        System::reset_events();
        let levels2 = vec![
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        assert_ok!(CommissionMultiLevel::force_set_multi_level_config(
            RuntimeOrigin::root(),
            1,
            levels2.try_into().unwrap(),
        ));

        let events = System::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(pallet::Event::ConfigDetailedChange {
                    entity_id: 1,
                    old_levels_count: 1,
                    new_levels_count: 2,
                })
            )
        });
        assert!(
            found,
            "ConfigDetailedChange should show old values on force_set overwrite"
        );
    });
}

#[test]
fn m3_r7_cancel_pending_records_audit_log() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        assert_ok!(CommissionMultiLevel::cancel_pending_config(
            RuntimeOrigin::signed(OWNER),
            1
        ));

        // schedule_config_change logs PendingScheduled (idx 0), cancel logs PendingCancelled (idx 1)
        assert_eq!(pallet::ConfigChangeLogCount::<Test>::get(1), 2);
        let log = pallet::ConfigChangeLogs::<Test>::get(1, 1).unwrap();
        assert_eq!(log.who, OWNER);
        assert_eq!(log.change_type, pallet::ConfigChangeType::PendingCancelled);
    });
}

// ============================================================================
// 审计 R8: M1 — 冻结/暂停推荐人不获佣
// ============================================================================

#[test]
fn m1_r8_frozen_member_skipped_in_multi_level() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        let buyer = 50u64;
        // chain: buyer -> 10 -> 20 -> 30
        setup_chain(1, buyer, &[10, 20, 30]);
        set_stats(1, 10, 5, 10, 0);
        set_stats(1, 20, 5, 10, 0);
        set_stats(1, 30, 5, 10, 0);

        let tiers = vec![
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 200,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));

        // 冻结 L1 推荐人
        freeze_member(1, 10);

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &buyer, 10000, 10000, modes, false, 0, 0,
            );
        // L1 (10) 被冻结跳过, L2 (20) 获 500, L3 (30) 获 200
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 20);
        assert_eq!(outputs[0].amount, 500);
        assert_eq!(outputs[1].beneficiary, 30);
        assert_eq!(outputs[1].amount, 200);
        assert_eq!(remaining, 9300);
    });
}

#[test]
fn m1_r8_unfrozen_member_still_gets_commission() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        let buyer = 50u64;
        setup_chain(1, buyer, &[10, 20]);
        set_stats(1, 10, 5, 10, 0);
        set_stats(1, 20, 5, 10, 0);

        let tiers = vec![
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));

        // 不冻结 — 默认正常
        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &buyer, 10000, 10000, modes, false, 0, 0,
            );
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].amount, 1000);
        assert_eq!(outputs[1].amount, 500);
        assert_eq!(remaining, 8500);
    });
}

// ============================================================================
// 审计 R8: M2 — preview_commission 未激活实体返空
// ============================================================================

#[test]
fn m2_r8_preview_returns_empty_for_inactive_entity() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        let buyer = 50u64;
        setup_chain(1, buyer, &[10]);
        set_stats(1, 10, 5, 10, 0);

        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));

        // 正常时有输出
        let preview = pallet::Pallet::<Test>::preview_commission(1, &buyer, 10000);
        assert_eq!(preview.len(), 1);

        // 设为未激活后应返空
        set_entity_inactive(1);
        let preview = pallet::Pallet::<Test>::preview_commission(1, &buyer, 10000);
        assert!(preview.is_empty());
    });
}

// ============================================================================
// 审计 R9 回归测试
// ============================================================================

// M1-R9: apply_pending_config 锁定实体拒绝
#[test]
fn m1_r9_apply_pending_rejects_locked_entity() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        System::set_block_number(11);

        // 锁定实体后，apply_pending_config 应被拒绝
        set_entity_locked(1);
        assert_noop!(
            CommissionMultiLevel::apply_pending_config(RuntimeOrigin::signed(OWNER), 1),
            pallet::Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn m1_r9_apply_pending_works_when_unlocked() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        System::set_block_number(11);

        // 未锁定时正常应用
        assert_ok!(CommissionMultiLevel::apply_pending_config(
            RuntimeOrigin::signed(OWNER),
            1
        ));
        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert!(config.levels.len() > 0);
    });
}

// L1-R9: 小额订单 commission=0 跳过而非终止
#[test]
fn l1_r9_small_amount_zero_commission_skips_not_breaks() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        // buyer=50 -> 40 -> 30
        setup_chain(1, 50, &[40, 30]);

        // L1: rate=1 → 3 * 1 / 10000 = 0（精度截断），应跳过而非终止
        // L2: rate=5000 → 3 * 5000 / 10000 = 1（非零），应正常分配
        let levels = vec![
            pallet::MultiLevelTier {
                rate: 1,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 5000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 3, 3, modes, false, 0, 0,
            );

        // L1 跳过（commission=0），L2 获得 1
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].beneficiary, 30);
        assert_eq!(outputs[0].amount, 1);
        assert_eq!(outputs[0].level, 2);
        assert_eq!(remaining, 2);
    });
}

#[test]
fn l1_r9_remaining_zero_breaks_correctly() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        // buyer=50 -> 40 -> 30
        setup_chain(1, 50, &[40, 30]);

        // L1 消耗全部 remaining
        let levels = vec![
            pallet::MultiLevelTier {
                rate: 10000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 5000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, remaining) =
            <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 1000, 1000, modes, false, 0, 0,
            );

        // L1 消耗全部 1000, remaining=0 → L2 终止
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 1000);
        assert_eq!(remaining, 0);
    });
}

// L2-R9: rates_sum 使用 u32 不饱和
#[test]
fn l2_r9_rates_sum_u32_no_saturation() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        // 7 层 rate=10000 → rates_sum = 70000 > u16::MAX (65535)
        let levels: Vec<_> = (0..7)
            .map(|_| pallet::MultiLevelTier {
                rate: 10000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            })
            .collect();
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        let events = System::events();
        let found = events.iter().find(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(pallet::Event::RatesSumWarning { .. })
            )
        });
        assert!(found.is_some(), "RatesSumWarning should be emitted");

        // 验证 rates_sum = 70000（u32 精确值），而非 u16 饱和的 65535
        let correct_value = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(pallet::Event::RatesSumWarning {
                    entity_id: 1,
                    rates_sum: 70000
                })
            )
        });
        assert!(
            correct_value,
            "rates_sum should be 70000 (u32), not 65535 (saturated u16)"
        );
    });
}

// L3-R9: PlanWriter::clear 不存在配置时不发事件
#[test]
fn l3_r9_plan_writer_clear_no_event_when_absent() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        System::set_block_number(1);

        // 无配置时 clear — 应成功但不发事件
        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::clear_multi_level_config(1));
        let events = System::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(pallet::Event::MultiLevelConfigCleared { .. })
            )
        });
        assert!(
            !found,
            "Cleared event should NOT be emitted when no config exists"
        );
    });
}

#[test]
fn l3_r9_plan_writer_clear_emits_event_when_config_exists() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        System::set_block_number(1);

        // 先设配置
        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![500],));
        System::reset_events();

        // 配置存在时 clear — 应发事件
        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::clear_multi_level_config(1));
        let events = System::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(pallet::Event::MultiLevelConfigCleared {
                    entity_id: 1
                })
            )
        });
        assert!(found, "Cleared event should be emitted when config exists");
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_none());
    });
}

// L4-R9: 审计日志环形缓冲
#[test]
fn l4_r9_audit_log_ring_buffer_wraps_around() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        // 先设置配置（产生 1 条日志 at slot 0）
        let tiers = vec![
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));
        assert_eq!(pallet::ConfigChangeLogCount::<Test>::get(1), 1);

        // 模拟填充到 MAX_CONFIG_CHANGE_LOGS (1000)
        // 直接设置 count = 999，然后再记录一条（slot = 999 % 1000 = 999）
        pallet::ConfigChangeLogCount::<Test>::insert(1, 999);

        // 第 1000 条日志: count=999, slot=999%1000=999
        assert_ok!(CommissionMultiLevel::pause_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));
        assert_eq!(pallet::ConfigChangeLogCount::<Test>::get(1), 1000);
        let log_999 = pallet::ConfigChangeLogs::<Test>::get(1, 999).unwrap();
        assert_eq!(log_999.change_type, pallet::ConfigChangeType::Pause);

        // 第 1001 条日志: count=1000, slot=1000%1000=0 → 覆盖 slot 0 的旧日志
        assert_ok!(CommissionMultiLevel::resume_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));
        assert_eq!(pallet::ConfigChangeLogCount::<Test>::get(1), 1001);
        let log_0 = pallet::ConfigChangeLogs::<Test>::get(1, 0).unwrap();
        assert_eq!(log_0.change_type, pallet::ConfigChangeType::Resume);
    });
}

// ============================================================================
// R10-#1: Force 操作审计日志
// ============================================================================

#[test]
fn r10_1_force_set_records_audit_log() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        set_entity_owner(1, OWNER);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::force_set_multi_level_config(
            RuntimeOrigin::root(),
            1,
            levels.try_into().unwrap(),
        ));

        assert_eq!(pallet::ConfigChangeLogCount::<Test>::get(1), 1);
        let log = pallet::ConfigChangeLogs::<Test>::get(1, 0).unwrap();
        assert_eq!(log.change_type, pallet::ConfigChangeType::ForceSet);
    });
}

#[test]
fn r10_1_force_clear_records_audit_log() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        set_entity_owner(1, OWNER);

        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: vec![pallet::MultiLevelTier {
                    rate: 1000,
                    required_directs: 0,
                    required_team_size: 0,
                    required_spent: 0,
                    required_level_id: 0,
                }]
                .try_into()
                .unwrap(),
            },
        );

        assert_ok!(CommissionMultiLevel::force_clear_multi_level_config(
            RuntimeOrigin::root(),
            1
        ));
        assert_eq!(pallet::ConfigChangeLogCount::<Test>::get(1), 1);
        let log = pallet::ConfigChangeLogs::<Test>::get(1, 0).unwrap();
        assert_eq!(log.change_type, pallet::ConfigChangeType::ForceClear);
    });
}

#[test]
fn r10_1_force_clear_no_config_no_audit_log() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::force_clear_multi_level_config(
            RuntimeOrigin::root(),
            1
        ));
        assert_eq!(pallet::ConfigChangeLogCount::<Test>::get(1), 0);
    });
}

// ============================================================================
// R10-#2: 增量操作 rates_sum 警告
// ============================================================================

#[test]
fn r10_2_update_params_emits_rates_sum_warning() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            vec![
                pallet::MultiLevelTier {
                    rate: 6000,
                    required_directs: 0,
                    required_team_size: 0,
                    required_spent: 0,
                    required_level_id: 0
                },
                pallet::MultiLevelTier {
                    rate: 3000,
                    required_directs: 0,
                    required_team_size: 0,
                    required_spent: 0,
                    required_level_id: 0
                },
            ]
            .try_into()
            .unwrap(),
        ));
        System::reset_events();

        // Update tier rate to push sum above 10000: 6000 + 5000 = 11000 > 10000
        assert_ok!(CommissionMultiLevel::update_multi_level_params(
            RuntimeOrigin::signed(OWNER),
            1,
            Some(1),
            Some(pallet::MultiLevelTier {
                rate: 5000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0
            }),
        ));

        let events = System::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(pallet::Event::RatesSumWarning { .. })
            )
        });
        assert!(
            found,
            "RatesSumWarning should be emitted after update_multi_level_params"
        );
    });
}

// ============================================================================
// R10-#3: team_size / spent 上界校验
// ============================================================================

#[test]
fn r10_3_set_config_rejects_invalid_team_size() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        let tiers = vec![pallet::MultiLevelTier {
            rate: 500,
            required_directs: 0,
            required_team_size: 1_000_001,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_noop!(
            CommissionMultiLevel::set_multi_level_config(
                RuntimeOrigin::signed(OWNER),
                1,
                tiers.try_into().unwrap(),
            ),
            pallet::Error::<Test>::InvalidTeamSize
        );
    });
}

#[test]
fn r10_3_set_config_rejects_invalid_spent() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        let tiers = vec![pallet::MultiLevelTier {
            rate: 500,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 1_000_000_000_000_000_001,
            required_level_id: 0,
        }];
        assert_noop!(
            CommissionMultiLevel::set_multi_level_config(
                RuntimeOrigin::signed(OWNER),
                1,
                tiers.try_into().unwrap(),
            ),
            pallet::Error::<Test>::InvalidSpent
        );
    });
}

#[test]
fn r10_3_update_params_rejects_invalid_spent() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            vec![pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0
            }]
            .try_into()
            .unwrap(),
        ));
        assert_noop!(
            CommissionMultiLevel::update_multi_level_params(
                RuntimeOrigin::signed(OWNER),
                1,
                Some(0),
                Some(pallet::MultiLevelTier {
                    rate: 500,
                    required_directs: 0,
                    required_team_size: 0,
                    required_spent: 1_000_000_000_000_000_001,
                    required_level_id: 0
                }),
            ),
            pallet::Error::<Test>::InvalidSpent
        );
    });
}

// ============================================================================
// Budget cap validation (rates_sum ≤ multi_level_cap)
// ============================================================================

#[test]
fn set_config_rejects_rates_sum_exceeding_budget_cap() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        set_budget_cap(1, 2000); // 20%

        // rates_sum = 1000 + 1500 = 2500 > 2000 → rejected
        let tiers = vec![
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 1500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        assert_noop!(
            CommissionMultiLevel::set_multi_level_config(
                RuntimeOrigin::signed(OWNER),
                1,
                tiers.try_into().unwrap(),
            ),
            pallet::Error::<Test>::RatesSumExceedsBudgetCap
        );
    });
}

#[test]
fn set_config_accepts_rates_sum_within_budget_cap() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        set_budget_cap(1, 2000); // 20%

        // rates_sum = 1000 + 500 = 1500 ≤ 2000 → accepted
        let tiers = vec![
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));
    });
}

#[test]
fn set_config_no_cap_allows_any_rates() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        // cap = 0 (default, no limit)

        let tiers = vec![
            pallet::MultiLevelTier {
                rate: 5000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 5000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));
    });
}

#[test]
fn add_tier_rejects_when_exceeding_budget_cap() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        set_budget_cap(1, 1500);

        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));

        // Adding tier with rate 600 → total 1600 > 1500 → rejected
        assert_noop!(
            CommissionMultiLevel::add_tier(
                RuntimeOrigin::signed(OWNER),
                1,
                1,
                pallet::MultiLevelTier {
                    rate: 600,
                    required_directs: 0,
                    required_team_size: 0,
                    required_spent: 0,
                    required_level_id: 0
                },
            ),
            pallet::Error::<Test>::RatesSumExceedsBudgetCap
        );
    });
}

#[test]
fn update_tier_rejects_when_exceeding_budget_cap() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        set_budget_cap(1, 2000);

        let tiers = vec![
            pallet::MultiLevelTier {
                rate: 1000,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
            pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            },
        ];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));

        // Update tier 1 to 1500 → total 1000 + 1500 = 2500 > 2000 → rejected
        assert_noop!(
            CommissionMultiLevel::update_multi_level_params(
                RuntimeOrigin::signed(OWNER),
                1,
                Some(1),
                Some(pallet::MultiLevelTier {
                    rate: 1500,
                    required_directs: 0,
                    required_team_size: 0,
                    required_spent: 0,
                    required_level_id: 0
                }),
            ),
            pallet::Error::<Test>::RatesSumExceedsBudgetCap
        );
    });
}

#[test]
fn r10_3_valid_team_size_and_spent_accepted() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        let tiers = vec![pallet::MultiLevelTier {
            rate: 500,
            required_directs: 100,
            required_team_size: 1_000_000,
            required_spent: 1_000_000_000_000_000_000,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));
        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.levels[0].required_team_size, 1_000_000);
        assert_eq!(config.levels[0].required_spent, 1_000_000_000_000_000_000);
    });
}

// ============================================================================
// R10-#6: force_pause / force_resume
// ============================================================================

#[test]
fn r10_6_force_pause_works() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        assert_ok!(CommissionMultiLevel::force_pause_multi_level(
            RuntimeOrigin::root(),
            1
        ));
        assert!(pallet::GlobalPaused::<Test>::get(1));

        let log = pallet::ConfigChangeLogs::<Test>::get(1, 0).unwrap();
        assert_eq!(log.change_type, pallet::ConfigChangeType::ForcePause);
    });
}

#[test]
fn r10_6_force_pause_rejects_already_paused() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::force_pause_multi_level(
            RuntimeOrigin::root(),
            1
        ));
        assert_noop!(
            CommissionMultiLevel::force_pause_multi_level(RuntimeOrigin::root(), 1),
            pallet::Error::<Test>::MultiLevelIsPaused
        );
    });
}

#[test]
fn r10_6_force_resume_works() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        assert_ok!(CommissionMultiLevel::force_pause_multi_level(
            RuntimeOrigin::root(),
            1
        ));
        assert_ok!(CommissionMultiLevel::force_resume_multi_level(
            RuntimeOrigin::root(),
            1
        ));
        assert!(!pallet::GlobalPaused::<Test>::get(1));

        let log = pallet::ConfigChangeLogs::<Test>::get(1, 1).unwrap();
        assert_eq!(log.change_type, pallet::ConfigChangeType::ForceResume);
    });
}

#[test]
fn r10_6_force_resume_rejects_not_paused() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_noop!(
            CommissionMultiLevel::force_resume_multi_level(RuntimeOrigin::root(), 1),
            pallet::Error::<Test>::MultiLevelNotPaused
        );
    });
}

#[test]
fn r10_6_force_pause_rejects_non_root() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_noop!(
            CommissionMultiLevel::force_pause_multi_level(RuntimeOrigin::signed(OWNER), 1),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ============================================================================
// R10-#4: force_cleanup_entity
// ============================================================================

#[test]
fn r10_4_force_cleanup_entity_clears_all_storage() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));
        assert_ok!(CommissionMultiLevel::pause_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));

        // Write some stats
        use pallet_commission_common::{CommissionOutput, CommissionType};
        pallet::Pallet::<Test>::update_stats(
            1,
            &[CommissionOutput {
                beneficiary: 40u64,
                amount: 1000u128,
                commission_type: CommissionType::MultiLevel,
                level: 1,
            }],
            true,
        );

        // Verify data exists
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_some());
        assert!(pallet::GlobalPaused::<Test>::get(1));
        assert_eq!(
            pallet::MemberMultiLevelStats::<Test>::get(1, 40u64).total_earned,
            1000
        );
        assert!(pallet::ConfigChangeLogCount::<Test>::get(1) > 0);

        // Cleanup
        assert_ok!(CommissionMultiLevel::force_cleanup_entity(
            RuntimeOrigin::root(),
            1,
            10
        ));

        // All storage should be cleared
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_none());
        assert!(!pallet::GlobalPaused::<Test>::get(1));
        assert_eq!(
            pallet::MemberMultiLevelStats::<Test>::get(1, 40u64).total_earned,
            0
        );
        assert_eq!(
            pallet::EntityMultiLevelStats::<Test>::get(1).total_distributed,
            0
        );
        assert_eq!(pallet::ConfigChangeLogCount::<Test>::get(1), 0);
    });
}

#[test]
fn r10_4_force_cleanup_entity_rejects_non_root() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_noop!(
            CommissionMultiLevel::force_cleanup_entity(RuntimeOrigin::signed(OWNER), 1, 0),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ============================================================================
// R10-#7: MultiLevelCommissionDistributed 事件
// ============================================================================

#[test]
fn r10_7_update_stats_emits_commission_distributed_event() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        use pallet_commission_common::{CommissionOutput, CommissionType};

        let outputs = vec![
            CommissionOutput {
                beneficiary: 40u64,
                amount: 1000u128,
                commission_type: CommissionType::MultiLevel,
                level: 1,
            },
            CommissionOutput {
                beneficiary: 30u64,
                amount: 500u128,
                commission_type: CommissionType::MultiLevel,
                level: 2,
            },
        ];

        pallet::Pallet::<Test>::update_stats(1, &outputs, true);

        let events = System::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(
                    pallet::Event::MultiLevelCommissionDistributed {
                        entity_id: 1,
                        total_amount: 1500,
                        beneficiary_count: 2
                    }
                )
            )
        });
        assert!(
            found,
            "MultiLevelCommissionDistributed event should be emitted"
        );
    });
}

// ============================================================================
// R10-#9: get_recent_change_logs
// ============================================================================

#[test]
fn r10_9_get_recent_change_logs_returns_empty_when_none() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        let logs = pallet::Pallet::<Test>::get_recent_change_logs(1, 10);
        assert!(logs.is_empty());
    });
}

#[test]
fn r10_9_get_recent_change_logs_returns_in_reverse_order() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));
        assert_ok!(CommissionMultiLevel::pause_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));
        assert_ok!(CommissionMultiLevel::resume_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));

        let logs = pallet::Pallet::<Test>::get_recent_change_logs(1, 10);
        assert_eq!(logs.len(), 3);
        assert_eq!(logs[0].change_type, pallet::ConfigChangeType::Resume);
        assert_eq!(logs[1].change_type, pallet::ConfigChangeType::Pause);
        assert_eq!(logs[2].change_type, pallet::ConfigChangeType::SetConfig);
    });
}

#[test]
fn r10_9_get_recent_change_logs_respects_limit() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));
        assert_ok!(CommissionMultiLevel::pause_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));
        assert_ok!(CommissionMultiLevel::resume_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));

        let logs = pallet::Pallet::<Test>::get_recent_change_logs(1, 2);
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].change_type, pallet::ConfigChangeType::Resume);
        assert_eq!(logs[1].change_type, pallet::ConfigChangeType::Pause);
    });
}

// ============================================================================
// R10-#10: PlanWriter EntityLocked 检查
// ============================================================================

#[test]
fn r10_10_plan_writer_rejects_locked_entity() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        set_entity_owner(1, OWNER);
        set_entity_locked(1);

        assert!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![500],).is_err()
        );
    });
}

#[test]
fn r10_10_plan_writer_full_rejects_locked_entity() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        set_entity_owner(1, OWNER);
        set_entity_locked(1);

        assert!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level_full(
                1,
                vec![(500, 0, 0, 0, 0)],
            )
            .is_err()
        );
    });
}

#[test]
fn r10_10_plan_writer_clear_rejects_locked_entity() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        set_entity_owner(1, OWNER);

        // Set config first
        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![500],));

        set_entity_locked(1);

        assert!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::clear_multi_level_config(1).is_err()
        );
    });
}

#[test]
fn r10_10_plan_writer_works_when_unlocked() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        set_entity_owner(1, OWNER);

        assert_ok!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![500, 300],)
        );
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_some());
    });
}

// ============================================================================
// R10-#14: PendingConfigQueue + on_initialize 自动应用
// ============================================================================

#[test]
fn r10_14_schedule_adds_to_pending_queue() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        let queue = pallet::PendingConfigQueue::<Test>::get();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0], 1);
    });
}

#[test]
fn r10_14_cancel_removes_from_pending_queue() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));
        assert_eq!(pallet::PendingConfigQueue::<Test>::get().len(), 1);

        assert_ok!(CommissionMultiLevel::cancel_pending_config(
            RuntimeOrigin::signed(OWNER),
            1
        ));
        assert!(pallet::PendingConfigQueue::<Test>::get().is_empty());
    });
}

#[test]
fn r10_14_manual_apply_removes_from_pending_queue() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));
        assert_eq!(pallet::PendingConfigQueue::<Test>::get().len(), 1);

        System::set_block_number(11);
        assert_ok!(CommissionMultiLevel::apply_pending_config(
            RuntimeOrigin::signed(OWNER),
            1
        ));
        assert!(pallet::PendingConfigQueue::<Test>::get().is_empty());
    });
}

#[test]
fn r10_14_on_initialize_auto_applies_ready_config() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        // Not yet ready
        System::set_block_number(5);
        CommissionMultiLevel::on_initialize(5);
        assert!(pallet::PendingConfigs::<Test>::get(1).is_some());

        // Now ready (block 11 >= effective_at 11)
        System::set_block_number(11);
        CommissionMultiLevel::on_initialize(11);

        // Config applied, pending removed
        let config = pallet::MultiLevelConfigs::<Test>::get(1).unwrap();
        assert!(config.levels.len() > 0);
        assert!(pallet::PendingConfigs::<Test>::get(1).is_none());
        assert!(pallet::PendingConfigQueue::<Test>::get().is_empty());

        // Audit log records PendingAutoApplied
        let count = pallet::ConfigChangeLogCount::<Test>::get(1);
        let last_slot = (count - 1) % 1000;
        let log = pallet::ConfigChangeLogs::<Test>::get(1, last_slot).unwrap();
        assert_eq!(
            log.change_type,
            pallet::ConfigChangeType::PendingAutoApplied
        );
    });
}

#[test]
fn r10_14_on_initialize_skips_locked_entity() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        set_entity_locked(1);
        System::set_block_number(11);
        CommissionMultiLevel::on_initialize(11);

        // Should NOT be applied
        assert!(pallet::PendingConfigs::<Test>::get(1).is_some());
        assert_eq!(pallet::PendingConfigQueue::<Test>::get().len(), 1);
    });
}

// ============================================================================
// R10-#3: PlanWriter team_size / spent 校验
// ============================================================================

#[test]
fn r10_3_plan_writer_full_rejects_invalid_team_size() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        assert!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level_full(
                1,
                vec![(500, 0, 1_000_001, 0, 0)],
            )
            .is_err()
        );
    });
}

#[test]
fn r10_3_plan_writer_full_rejects_invalid_spent() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        assert!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level_full(
                1,
                vec![(500, 0, 0, 1_000_000_000_000_000_001, 0)],
            )
            .is_err()
        );
    });
}

// ============================================================================
// B-1: force_resume 非 Root 拒绝
// ============================================================================

#[test]
fn b1_force_resume_rejects_non_root() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        assert_ok!(CommissionMultiLevel::force_pause_multi_level(
            RuntimeOrigin::root(),
            1
        ));
        assert_noop!(
            CommissionMultiLevel::force_resume_multi_level(RuntimeOrigin::signed(OWNER), 1),
            sp_runtime::DispatchError::BadOrigin
        );
        assert_noop!(
            CommissionMultiLevel::force_resume_multi_level(RuntimeOrigin::signed(ADMIN), 1),
            sp_runtime::DispatchError::BadOrigin
        );
        assert_noop!(
            CommissionMultiLevel::force_resume_multi_level(RuntimeOrigin::signed(NOBODY), 1),
            sp_runtime::DispatchError::BadOrigin
        );
        assert!(pallet::GlobalPaused::<Test>::get(1));
    });
}

// ============================================================================
// B-2: on_initialize 多 entity 同时生效
// ============================================================================

#[test]
fn b2_on_initialize_applies_multiple_entities_simultaneously() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        setup_entity(2);
        setup_entity(3);

        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];

        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.clone().try_into().unwrap(),
        ));
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            2,
            tiers.clone().try_into().unwrap(),
        ));
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            3,
            tiers.try_into().unwrap(),
        ));
        assert_eq!(pallet::PendingConfigQueue::<Test>::get().len(), 3);

        System::set_block_number(11);
        CommissionMultiLevel::on_initialize(11);

        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_some());
        assert!(pallet::MultiLevelConfigs::<Test>::get(2).is_some());
        assert!(pallet::MultiLevelConfigs::<Test>::get(3).is_some());
        assert!(pallet::PendingConfigs::<Test>::get(1).is_none());
        assert!(pallet::PendingConfigs::<Test>::get(2).is_none());
        assert!(pallet::PendingConfigs::<Test>::get(3).is_none());
        assert!(pallet::PendingConfigQueue::<Test>::get().is_empty());
    });
}

// ============================================================================
// B-3: on_initialize MAX_AUTO_APPLY=5 上限
// ============================================================================

#[test]
fn b3_on_initialize_caps_at_max_auto_apply() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        for eid in 1u64..=8u64 {
            setup_entity(eid);
            let tiers = vec![pallet::MultiLevelTier {
                rate: 500,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            }];
            assert_ok!(CommissionMultiLevel::schedule_config_change(
                RuntimeOrigin::signed(OWNER),
                eid,
                tiers.try_into().unwrap(),
            ));
        }
        assert_eq!(pallet::PendingConfigQueue::<Test>::get().len(), 8);

        System::set_block_number(11);
        CommissionMultiLevel::on_initialize(11);

        let mut applied = 0u64;
        let mut remaining = 0u64;
        for eid in 1u64..=8u64 {
            if pallet::PendingConfigs::<Test>::get(eid).is_none() {
                applied += 1;
            } else {
                remaining += 1;
            }
        }
        assert_eq!(applied, 5, "exactly 5 should be applied per block");
        assert_eq!(remaining, 3, "3 should remain pending");
        assert_eq!(pallet::PendingConfigQueue::<Test>::get().len(), 3);

        // Second on_initialize processes the remaining 3
        CommissionMultiLevel::on_initialize(11);
        for eid in 1u64..=8u64 {
            assert!(pallet::PendingConfigs::<Test>::get(eid).is_none());
        }
        assert!(pallet::PendingConfigQueue::<Test>::get().is_empty());
    });
}

// ============================================================================
// B-4: PendingQueueFull 错误路径
// ============================================================================

#[test]
fn b4_pending_queue_full_error() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();

        // Fill the queue to capacity (ConstU32<100>)
        for eid in 1u64..=100u64 {
            setup_entity(eid);
            let tiers = vec![pallet::MultiLevelTier {
                rate: 100,
                required_directs: 0,
                required_team_size: 0,
                required_spent: 0,
                required_level_id: 0,
            }];
            assert_ok!(CommissionMultiLevel::schedule_config_change(
                RuntimeOrigin::signed(OWNER),
                eid,
                tiers.try_into().unwrap(),
            ));
        }
        assert_eq!(pallet::PendingConfigQueue::<Test>::get().len(), 100);

        // The 101st should fail with PendingQueueFull
        setup_entity(101);
        let tiers = vec![pallet::MultiLevelTier {
            rate: 100,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_noop!(
            CommissionMultiLevel::schedule_config_change(
                RuntimeOrigin::signed(OWNER),
                101,
                tiers.try_into().unwrap(),
            ),
            pallet::Error::<Test>::PendingQueueFull
        );
    });
}

// ============================================================================
// B-5: on_initialize 孤立队列条目清理（PendingConfigs 已被手动删除）
// ============================================================================

#[test]
fn b5_on_initialize_cleans_orphaned_queue_entries() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        setup_entity(2);

        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];

        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.clone().try_into().unwrap(),
        ));
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            2,
            tiers.try_into().unwrap(),
        ));

        // Manually remove PendingConfigs for entity 1 (simulating manual apply or cancel)
        pallet::PendingConfigs::<Test>::remove(1);

        System::set_block_number(11);
        CommissionMultiLevel::on_initialize(11);

        // Entity 1 had no PendingConfigs → orphan cleaned from queue
        // Entity 2 should be applied normally
        assert!(pallet::PendingConfigQueue::<Test>::get().is_empty());
        assert!(pallet::MultiLevelConfigs::<Test>::get(2).is_some());
    });
}

// ============================================================================
// B-6: force_cleanup 清除 PendingConfigQueue
// ============================================================================

#[test]
fn b6_force_cleanup_removes_from_pending_queue() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        setup_entity(2);

        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];

        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.clone().try_into().unwrap(),
        ));
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            2,
            tiers.try_into().unwrap(),
        ));
        assert_eq!(pallet::PendingConfigQueue::<Test>::get().len(), 2);

        assert_ok!(CommissionMultiLevel::force_cleanup_entity(
            RuntimeOrigin::root(),
            1,
            0
        ));

        // Entity 1 removed from queue, entity 2 remains
        let queue = pallet::PendingConfigQueue::<Test>::get();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0], 2);

        // All storage for entity 1 cleared
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_none());
        assert!(pallet::PendingConfigs::<Test>::get(1).is_none());
        assert_eq!(pallet::ConfigChangeLogCount::<Test>::get(1), 0);
    });
}

// ============================================================================
// B-7: force_cleanup 幂等性（重复调用不报错）
// ============================================================================

#[test]
fn b7_force_cleanup_is_idempotent() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));

        // First cleanup
        assert_ok!(CommissionMultiLevel::force_cleanup_entity(
            RuntimeOrigin::root(),
            1,
            10
        ));
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_none());

        // Second cleanup on already-cleaned entity — should succeed (idempotent)
        assert_ok!(CommissionMultiLevel::force_cleanup_entity(
            RuntimeOrigin::root(),
            1,
            0
        ));
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_none());

        // Cleanup on entity that never had data — also idempotent
        assert_ok!(CommissionMultiLevel::force_cleanup_entity(
            RuntimeOrigin::root(),
            999,
            0
        ));
    });
}

// ============================================================================
// R11-B2: PlanWriter 复用 validate_config（校验与 extrinsic 路径一致）
// ============================================================================

#[test]
fn r11_b2_plan_writer_set_multi_level_uses_validate_config() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        // rate > 10000 should be rejected (via validate_tier inside validate_config)
        assert!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![10001],)
                .is_err()
        );
        // valid input works
        assert_ok!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![500, 300],)
        );
    });
}

#[test]
fn r11_b2_plan_writer_full_uses_validate_config() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        // team_size > 1M rejected
        assert!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level_full(
                1,
                vec![(500, 0, 1_000_001, 0, 0)],
            )
            .is_err()
        );
        // spent > 10^18 rejected
        assert!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level_full(
                1,
                vec![(500, 0, 0, 1_000_000_000_000_000_001, 0)],
            )
            .is_err()
        );
        // empty levels rejected
        assert!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level_full(1, vec![],)
                .is_err()
        );
    });
}

// ============================================================================
// R11-B3: PlanWriter 审计日志（GovernanceSet / GovernanceClear）
// ============================================================================

#[test]
fn r11_b3_plan_writer_set_records_governance_audit_log() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        System::set_block_number(1);

        assert_ok!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![500, 300],)
        );

        let count = pallet::ConfigChangeLogCount::<Test>::get(1);
        assert_eq!(count, 1);
        let log = pallet::ConfigChangeLogs::<Test>::get(1, 0).unwrap();
        assert_eq!(log.change_type, pallet::ConfigChangeType::GovernanceSet);
    });
}

#[test]
fn r11_b3_plan_writer_full_records_governance_audit_log() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        System::set_block_number(1);

        assert_ok!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level_full(
                1,
                vec![(500, 0, 0, 0, 0)],
            )
        );

        let count = pallet::ConfigChangeLogCount::<Test>::get(1);
        assert_eq!(count, 1);
        let log = pallet::ConfigChangeLogs::<Test>::get(1, 0).unwrap();
        assert_eq!(log.change_type, pallet::ConfigChangeType::GovernanceSet);
    });
}

#[test]
fn r11_b3_plan_writer_clear_records_governance_audit_log() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        System::set_block_number(1);

        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![500],));
        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::clear_multi_level_config(1));

        let count = pallet::ConfigChangeLogCount::<Test>::get(1);
        assert_eq!(count, 2);
        let log = pallet::ConfigChangeLogs::<Test>::get(1, 1).unwrap();
        assert_eq!(log.change_type, pallet::ConfigChangeType::GovernanceClear);
    });
}

#[test]
fn r11_b3_plan_writer_clear_no_log_when_absent() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        System::set_block_number(1);

        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::clear_multi_level_config(1));
        assert_eq!(pallet::ConfigChangeLogCount::<Test>::get(1), 0);
    });
}

// ============================================================================
// R11-B4: PlanWriter emits ConfigDetailedChange
// ============================================================================

#[test]
fn r11_b4_plan_writer_set_emits_config_detailed_change() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        System::set_block_number(1);

        assert_ok!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![500, 300],)
        );

        let events = System::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(pallet::Event::ConfigDetailedChange {
                    entity_id: 1,
                    old_levels_count: 0,
                    new_levels_count: 2,
                })
            )
        });
        assert!(
            found,
            "ConfigDetailedChange should be emitted for new config"
        );
    });
}

#[test]
fn r11_b4_plan_writer_set_overwrite_shows_old_values() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        System::set_block_number(1);

        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![500],));
        System::reset_events();
        assert_ok!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level_full(
                1,
                vec![(300, 0, 0, 0, 0), (200, 5, 0, 0, 0)],
            )
        );

        let events = System::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(pallet::Event::ConfigDetailedChange {
                    entity_id: 1,
                    old_levels_count: 1,
                    new_levels_count: 2,
                })
            )
        });
        assert!(found, "ConfigDetailedChange should show old config values");
    });
}

#[test]
fn r11_b4_plan_writer_clear_emits_config_detailed_change() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        System::set_block_number(1);

        assert_ok!(
            <pallet::Pallet<Test> as MultiLevelPlanWriter>::set_multi_level(1, vec![500, 300],)
        );
        System::reset_events();
        assert_ok!(<pallet::Pallet<Test> as MultiLevelPlanWriter>::clear_multi_level_config(1));

        let events = System::events();
        let found = events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::CommissionMultiLevel(pallet::Event::ConfigDetailedChange {
                    entity_id: 1,
                    old_levels_count: 2,
                    new_levels_count: 0,
                })
            )
        });
        assert!(
            found,
            "ConfigDetailedChange should show cleared config values"
        );
    });
}

// ============================================================================
// R11-B1: force_cleanup weight 参数化
// ============================================================================

#[test]
fn r11_b1_force_cleanup_with_member_count_hint() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let tiers = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::set_multi_level_config(
            RuntimeOrigin::signed(OWNER),
            1,
            tiers.try_into().unwrap(),
        ));

        // hint=100, hint=0 both work — hint only affects weight, not behavior
        assert_ok!(CommissionMultiLevel::force_cleanup_entity(
            RuntimeOrigin::root(),
            1,
            100
        ));
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_none());
    });
}

// ============================================================================
// V2-FIX: update_stats count_order 参数 — 防止 NEX+Token 双管道重复计入 order_count
// ============================================================================

#[test]
fn v2_update_stats_count_order_true_increments_orders() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        use pallet_commission_common::{CommissionOutput, CommissionType};

        let outputs = vec![CommissionOutput {
            beneficiary: 40u64,
            amount: 1000u128,
            commission_type: CommissionType::MultiLevel,
            level: 1,
        }];
        pallet::Pallet::<Test>::update_stats(1, &outputs, true);

        let member_stats = pallet::MemberMultiLevelStats::<Test>::get(1, 40u64);
        assert_eq!(member_stats.commission_receipt_count, 1);
        assert_eq!(member_stats.total_earned, 1000);

        let entity_stats = pallet::EntityMultiLevelStats::<Test>::get(1);
        assert_eq!(entity_stats.order_count, 1);
        assert_eq!(entity_stats.total_distributed, 1000);
        assert_eq!(entity_stats.total_distribution_entries, 1);
    });
}

#[test]
fn v2_update_stats_count_order_false_skips_order_count() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        use pallet_commission_common::{CommissionOutput, CommissionType};

        let outputs = vec![CommissionOutput {
            beneficiary: 40u64,
            amount: 500u128,
            commission_type: CommissionType::MultiLevel,
            level: 1,
        }];
        // Token 管道：count_order=false
        pallet::Pallet::<Test>::update_stats(1, &outputs, false);

        let member_stats = pallet::MemberMultiLevelStats::<Test>::get(1, 40u64);
        assert_eq!(
            member_stats.commission_receipt_count, 0,
            "Token pipeline should NOT increment receipt count"
        );
        assert_eq!(
            member_stats.total_earned, 500,
            "total_earned should still accumulate"
        );

        let entity_stats = pallet::EntityMultiLevelStats::<Test>::get(1);
        assert_eq!(
            entity_stats.order_count, 0,
            "Entity order_count should NOT increment"
        );
        assert_eq!(
            entity_stats.total_distributed, 500,
            "total_distributed should still accumulate"
        );
        assert_eq!(
            entity_stats.total_distribution_entries, 1,
            "distribution_entries should still count"
        );
    });
}

#[test]
fn v2_dual_pipeline_order_counted_once() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        use pallet_commission_common::{CommissionOutput, CommissionType};

        // 模拟同一订单：NEX 管道先执行（count_order=true）
        let nex_outputs = vec![
            CommissionOutput {
                beneficiary: 40u64,
                amount: 1000u128,
                commission_type: CommissionType::MultiLevel,
                level: 1,
            },
            CommissionOutput {
                beneficiary: 30u64,
                amount: 500u128,
                commission_type: CommissionType::MultiLevel,
                level: 2,
            },
        ];
        pallet::Pallet::<Test>::update_stats(1, &nex_outputs, true);

        // Token 管道后执行（count_order=false）
        let token_outputs = vec![
            CommissionOutput {
                beneficiary: 40u64,
                amount: 2000u128,
                commission_type: CommissionType::MultiLevel,
                level: 1,
            },
            CommissionOutput {
                beneficiary: 30u64,
                amount: 800u128,
                commission_type: CommissionType::MultiLevel,
                level: 2,
            },
        ];
        pallet::Pallet::<Test>::update_stats(1, &token_outputs, false);

        // Member 40: earned = 1000 + 2000 = 3000, receipt_count = 1（不是 2）
        let stats40 = pallet::MemberMultiLevelStats::<Test>::get(1, 40u64);
        assert_eq!(stats40.total_earned, 3000);
        assert_eq!(
            stats40.commission_receipt_count, 1,
            "Receipt should be counted once, not twice"
        );

        // Member 30: earned = 500 + 800 = 1300, receipt_count = 1（不是 2）
        let stats30 = pallet::MemberMultiLevelStats::<Test>::get(1, 30u64);
        assert_eq!(stats30.total_earned, 1300);
        assert_eq!(
            stats30.commission_receipt_count, 1,
            "Receipt should be counted once, not twice"
        );

        // Entity: distributed = 4300, order_count = 1（不是 2）, entries = 4
        let entity_stats = pallet::EntityMultiLevelStats::<Test>::get(1);
        assert_eq!(entity_stats.total_distributed, 4300);
        assert_eq!(
            entity_stats.order_count, 1,
            "Entity order should be counted once"
        );
        assert_eq!(
            entity_stats.total_distribution_entries, 4,
            "All entries from both pipelines"
        );
    });
}

// ============================================================================
// V4-FIX: 审计日志 wrapping_add + get_recent_change_logs 回绕安全
// ============================================================================

#[test]
fn v4_record_change_log_wrapping_add_at_u32_max() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        // 将 count 设为 u32::MAX - 1，模拟即将溢出
        pallet::ConfigChangeLogCount::<Test>::insert(1, u32::MAX - 1);

        // 写入两条日志：count 从 MAX-1 → MAX → 0（wrapping）
        pallet::Pallet::<Test>::record_change_log(1, &OWNER, pallet::ConfigChangeType::SetConfig);
        assert_eq!(pallet::ConfigChangeLogCount::<Test>::get(1), u32::MAX);

        pallet::Pallet::<Test>::record_change_log(1, &OWNER, pallet::ConfigChangeType::ClearConfig);
        assert_eq!(
            pallet::ConfigChangeLogCount::<Test>::get(1),
            0,
            "count should wrap to 0"
        );

        // 继续写入应该正常工作（count = 0 → 1）
        pallet::Pallet::<Test>::record_change_log(1, &OWNER, pallet::ConfigChangeType::Pause);
        assert_eq!(pallet::ConfigChangeLogCount::<Test>::get(1), 1);

        // 验证三条日志写入了不同的 slot
        let slot_max_minus_1 = (u32::MAX - 1) % 1000; // 第一条
        let slot_max = u32::MAX % 1000; // 第二条
        let slot_0 = 0u32 % 1000; // 第三条

        let log1 = pallet::ConfigChangeLogs::<Test>::get(1, slot_max_minus_1);
        assert!(log1.is_some());
        assert_eq!(
            log1.unwrap().change_type,
            pallet::ConfigChangeType::SetConfig
        );

        let log2 = pallet::ConfigChangeLogs::<Test>::get(1, slot_max);
        assert!(log2.is_some());
        assert_eq!(
            log2.unwrap().change_type,
            pallet::ConfigChangeType::ClearConfig
        );

        let log3 = pallet::ConfigChangeLogs::<Test>::get(1, slot_0);
        assert!(log3.is_some());
        assert_eq!(log3.unwrap().change_type, pallet::ConfigChangeType::Pause);
    });
}

#[test]
fn v4_get_recent_change_logs_after_count_wraps() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        // 设 count = u32::MAX，写入一条日志使 count wrap 到 0
        pallet::ConfigChangeLogCount::<Test>::insert(1, u32::MAX);

        // slot = u32::MAX % 1000 = 295 — 写入一条日志
        pallet::Pallet::<Test>::record_change_log(1, &OWNER, pallet::ConfigChangeType::ForceSet);
        assert_eq!(
            pallet::ConfigChangeLogCount::<Test>::get(1),
            0,
            "count wrapped to 0"
        );

        // get_recent_change_logs 应该仍能读取（count=0 但有数据）
        let logs = pallet::Pallet::<Test>::get_recent_change_logs(1, 5);
        assert!(
            !logs.is_empty(),
            "Should return logs even when count wrapped to 0"
        );
    });
}

#[test]
fn v4_get_recent_change_logs_truly_empty() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();

        // count=0 且无数据 → 真正空
        let logs = pallet::Pallet::<Test>::get_recent_change_logs(1, 5);
        assert!(logs.is_empty(), "Should be empty when no logs ever written");
    });
}

// ============================================================================
// V1-FIX: apply_pending_config 权限检查 + 审计日志记录 scheduled_by
// ============================================================================

#[test]
fn v1_apply_pending_rejects_non_owner_admin() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        System::set_block_number(11);

        // NOBODY 无权触发应用
        assert_noop!(
            CommissionMultiLevel::apply_pending_config(RuntimeOrigin::signed(NOBODY), 1),
            pallet::Error::<Test>::NotEntityOwnerOrAdmin
        );

        // OWNER 可以
        assert_ok!(CommissionMultiLevel::apply_pending_config(
            RuntimeOrigin::signed(OWNER),
            1
        ));
        assert!(pallet::MultiLevelConfigs::<Test>::get(1).is_some());
    });
}

#[test]
fn v1_apply_pending_audit_log_records_scheduled_by() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        // OWNER 调度
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        System::set_block_number(11);

        // ADMIN 触发应用（有 COMMISSION_MANAGE 权限）
        let admin = 200u64;
        set_entity_admin(
            1,
            admin,
            pallet_entity_common::AdminPermission::COMMISSION_MANAGE,
        );
        assert_ok!(CommissionMultiLevel::apply_pending_config(
            RuntimeOrigin::signed(admin),
            1
        ));

        // 审计日志应记录 OWNER（scheduled_by），不是 admin（执行者）
        let logs = pallet::Pallet::<Test>::get_recent_change_logs(1, 5);
        let applied_log = logs
            .iter()
            .find(|l| l.change_type == pallet::ConfigChangeType::PendingApplied);
        assert!(applied_log.is_some(), "PendingApplied log should exist");
        assert_eq!(
            applied_log.unwrap().who,
            OWNER,
            "Audit log should record scheduled_by, not executor"
        );
    });
}

// ============================================================================
// V3-FIX: auto_apply_pending_configs 暂停时不应用
// ============================================================================

#[test]
fn v3_auto_apply_skips_paused_entity() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);

        let levels = vec![pallet::MultiLevelTier {
            rate: 1000,
            required_directs: 0,
            required_team_size: 0,
            required_spent: 0,
            required_level_id: 0,
        }];
        assert_ok!(CommissionMultiLevel::schedule_config_change(
            RuntimeOrigin::signed(OWNER),
            1,
            levels.try_into().unwrap(),
        ));

        // 暂停
        assert_ok!(CommissionMultiLevel::pause_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));

        // on_initialize 到达生效区块
        System::set_block_number(11);
        CommissionMultiLevel::on_initialize(11);

        // 配置不应被应用（暂停中）
        assert!(
            pallet::MultiLevelConfigs::<Test>::get(1).is_none(),
            "Config should NOT be applied while paused"
        );
        assert!(
            pallet::PendingConfigs::<Test>::get(1).is_some(),
            "Pending config should still exist"
        );
        assert!(
            !pallet::PendingConfigQueue::<Test>::get().is_empty(),
            "Queue entry should be retained"
        );

        // 恢复后，下一个区块应自动应用
        assert_ok!(CommissionMultiLevel::resume_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));
        System::set_block_number(12);
        CommissionMultiLevel::on_initialize(12);

        assert!(
            pallet::MultiLevelConfigs::<Test>::get(1).is_some(),
            "Config should be applied after resume"
        );
        assert!(
            pallet::PendingConfigs::<Test>::get(1).is_none(),
            "Pending config should be removed"
        );
    });
}

// ============================================================================
// 分佣历史记录 (Payout History) 测试
// ============================================================================

fn tier(rate: u16) -> pallet::MultiLevelTier {
    pallet::MultiLevelTier {
        rate,
        required_directs: 0,
        required_team_size: 0,
        required_spent: 0,
        required_level_id: 0,
    }
}

fn setup_3level_config(entity_id: u64) {
    let levels = vec![tier(1000), tier(500), tier(200)]; // L1: 10%, L2: 5%, L3: 2%
    pallet::MultiLevelConfigs::<Test>::insert(
        entity_id,
        pallet::MultiLevelConfig {
            levels: levels.try_into().unwrap(),
        },
    );
}

#[test]
fn payout_history_basic_records() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        setup_chain(1, 50, &[40, 30, 20]);
        setup_3level_config(1);

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, _) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10_000, 10_000, modes, false, 0, 1001,
        );
        assert_eq!(outputs.len(), 3);

        // account=40 (L1) 的记录
        let payouts_40 = pallet::MemberMultiLevelPayouts::<Test>::get(1, 40u64);
        assert_eq!(payouts_40.len(), 1);
        assert_eq!(payouts_40[0].buyer, 50);
        assert_eq!(payouts_40[0].order_id, 1001);
        assert_eq!(payouts_40[0].amount, 1000);
        assert_eq!(payouts_40[0].level, 1);
        assert_eq!(payouts_40[0].block_number, 1);

        // account=30 (L2)
        let payouts_30 = pallet::MemberMultiLevelPayouts::<Test>::get(1, 30u64);
        assert_eq!(payouts_30.len(), 1);
        assert_eq!(payouts_30[0].order_id, 1001);
        assert_eq!(payouts_30[0].amount, 500);
        assert_eq!(payouts_30[0].level, 2);

        // account=20 (L3)
        let payouts_20 = pallet::MemberMultiLevelPayouts::<Test>::get(1, 20u64);
        assert_eq!(payouts_20.len(), 1);
        assert_eq!(payouts_20[0].order_id, 1001);
        assert_eq!(payouts_20[0].amount, 200);
        assert_eq!(payouts_20[0].level, 3);
    });
}

#[test]
fn payout_history_summary_accumulates() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        setup_chain(1, 50, &[40, 30]);
        setup_3level_config(1);

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);

        // 第一笔
        let _ = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10_000, 10_000, modes, false, 0, 2001,
        );

        // 第二笔
        System::set_block_number(5);
        let _ = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 20_000, 20_000, modes, false, 1, 2002,
        );

        let summary_40 = pallet::MemberMultiLevelSummaryStats::<Test>::get(1, 40u64);
        assert_eq!(summary_40.total_earned, 1000 + 2000); // 10000*10% + 20000*10%
        assert_eq!(summary_40.total_payout_count, 2);
        assert_eq!(summary_40.last_payout_block, 5);

        let payouts_40 = pallet::MemberMultiLevelPayouts::<Test>::get(1, 40u64);
        assert_eq!(payouts_40.len(), 2);
        assert_eq!(payouts_40[0].order_id, 2001);
        assert_eq!(payouts_40[0].amount, 1000);
        assert_eq!(payouts_40[1].order_id, 2002);
        assert_eq!(payouts_40[1].amount, 2000);
    });
}

#[test]
fn payout_history_fifo_eviction() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        // 单层链: buyer -> referrer
        setup_chain(1, 50, &[40]);
        let levels = vec![tier(1000)]; // L1: 10%
        pallet::MultiLevelConfigs::<Test>::insert(
            1,
            pallet::MultiLevelConfig {
                levels: levels.try_into().unwrap(),
            },
        );

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);

        // 发 55 笔, MaxPayoutRecords = 50
        for i in 0..55u64 {
            System::set_block_number((i + 1) as u64);
            let _ = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1,
                &50,
                1_000 * (i + 1) as u128,
                10_000_000,
                modes,
                false,
                i as u32,
                3000 + i,
            );
        }

        let payouts_40 = pallet::MemberMultiLevelPayouts::<Test>::get(1, 40u64);
        assert_eq!(payouts_40.len(), 50);

        // 最旧的应该是第 6 笔 (i=5, block=6, order_id=3005, amount = 1000*6*10% = 600)
        assert_eq!(payouts_40[0].block_number, 6);
        assert_eq!(payouts_40[0].order_id, 3005);
        assert_eq!(payouts_40[0].amount, 600);

        // 最新的应该是第 55 笔
        assert_eq!(payouts_40[49].block_number, 55);
        assert_eq!(payouts_40[49].order_id, 3054);
        assert_eq!(payouts_40[49].amount, 5500);

        // 汇总包含全部 55 笔
        let summary_40 = pallet::MemberMultiLevelSummaryStats::<Test>::get(1, 40u64);
        assert_eq!(summary_40.total_payout_count, 55);
        // sum(1000*(i+1)*10/100 for i in 0..55) = 100 * sum(1..=55) = 100 * 1540 = 154000
        assert_eq!(summary_40.total_earned, 154_000);
    });
}

#[test]
fn payout_history_cross_entity_isolation() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        setup_entity(2);
        setup_chain(1, 50, &[40]);
        setup_chain(2, 50, &[40]);
        setup_3level_config(1);
        setup_3level_config(2);

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);

        let _ = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10_000, 10_000, modes, false, 0, 0,
        );
        let _ = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            2, &50, 20_000, 20_000, modes, false, 0, 0,
        );

        let payouts_e1 = pallet::MemberMultiLevelPayouts::<Test>::get(1, 40u64);
        let payouts_e2 = pallet::MemberMultiLevelPayouts::<Test>::get(2, 40u64);
        assert_eq!(payouts_e1[0].amount, 1000);
        assert_eq!(payouts_e2[0].amount, 2000);

        let summary_e1 = pallet::MemberMultiLevelSummaryStats::<Test>::get(1, 40u64);
        let summary_e2 = pallet::MemberMultiLevelSummaryStats::<Test>::get(2, 40u64);
        assert_eq!(summary_e1.total_earned, 1000);
        assert_eq!(summary_e2.total_earned, 2000);
    });
}

#[test]
fn payout_history_token_mode_does_not_record() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        use pallet_commission_common::TokenCommissionPlugin;
        setup_entity(1);
        setup_chain(1, 50, &[40, 30]);
        setup_3level_config(1);

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);

        let (outputs, _) =
            <pallet::Pallet<Test> as TokenCommissionPlugin<u64, Balance>>::calculate_token(
                1, &50, 10_000, 10_000, modes, false, 0, 0,
            );
        assert!(!outputs.is_empty());

        // Token 模式不应写入 payout history
        let payouts_40 = pallet::MemberMultiLevelPayouts::<Test>::get(1, 40u64);
        assert!(payouts_40.is_empty());

        let summary_40 = pallet::MemberMultiLevelSummaryStats::<Test>::get(1, 40u64);
        assert_eq!(summary_40.total_payout_count, 0);
    });
}

#[test]
fn payout_history_cleanup_clears_records() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        setup_chain(1, 50, &[40]);
        setup_3level_config(1);

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let _ = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10_000, 10_000, modes, false, 0, 0,
        );

        assert!(!pallet::MemberMultiLevelPayouts::<Test>::get(1, 40u64).is_empty());
        assert_ne!(
            pallet::MemberMultiLevelSummaryStats::<Test>::get(1, 40u64).total_payout_count,
            0
        );

        // force_cleanup_entity
        assert_ok!(CommissionMultiLevel::force_cleanup_entity(
            RuntimeOrigin::root(),
            1,
            10
        ));

        assert!(pallet::MemberMultiLevelPayouts::<Test>::get(1, 40u64).is_empty());
        assert_eq!(
            pallet::MemberMultiLevelSummaryStats::<Test>::get(1, 40u64).total_payout_count,
            0
        );
    });
}

#[test]
fn payout_history_no_records_when_paused() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        setup_chain(1, 50, &[40]);
        setup_3level_config(1);

        // 暂停
        assert_ok!(CommissionMultiLevel::pause_multi_level(
            RuntimeOrigin::signed(OWNER),
            1
        ));

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let (outputs, _) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10_000, 10_000, modes, false, 0, 0,
        );
        assert!(outputs.is_empty());

        assert!(pallet::MemberMultiLevelPayouts::<Test>::get(1, 40u64).is_empty());
    });
}

#[test]
fn payout_history_skipped_banned_member_no_records() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        // buyer=50 -> 40(banned) -> 30
        setup_chain(1, 50, &[40, 30]);
        ban_member(1, 40);
        setup_3level_config(1);

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);
        let _ = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10_000, 10_000, modes, false, 0, 0,
        );

        // 40 被 ban，不应有记录
        assert!(pallet::MemberMultiLevelPayouts::<Test>::get(1, 40u64).is_empty());

        // 30 作为 L2 应有记录
        let payouts_30 = pallet::MemberMultiLevelPayouts::<Test>::get(1, 30u64);
        assert_eq!(payouts_30.len(), 1);
        assert_eq!(payouts_30[0].level, 2);
    });
}

#[test]
fn payout_history_summary_default_when_empty() {
    new_test_ext().execute_with(|| {
        let summary = pallet::MemberMultiLevelSummaryStats::<Test>::get(1, 999u64);
        assert_eq!(summary.total_earned, 0);
        assert_eq!(summary.total_payout_count, 0);
        assert_eq!(summary.last_payout_block, 0);

        let payouts = pallet::MemberMultiLevelPayouts::<Test>::get(1, 999u64);
        assert!(payouts.is_empty());
    });
}

#[test]
fn payout_history_block_number_tracks_correctly() {
    new_test_ext().execute_with(|| {
        clear_thread_locals();
        setup_entity(1);
        setup_chain(1, 50, &[40]);
        setup_3level_config(1);

        let modes = CommissionModes(CommissionModes::MULTI_LEVEL);

        System::set_block_number(100);
        let _ = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10_000, 10_000, modes, false, 0, 0,
        );

        System::set_block_number(200);
        let _ = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
            1, &50, 10_000, 10_000, modes, false, 1, 0,
        );

        let payouts_40 = pallet::MemberMultiLevelPayouts::<Test>::get(1, 40u64);
        assert_eq!(payouts_40[0].block_number, 100);
        assert_eq!(payouts_40[1].block_number, 200);

        let summary_40 = pallet::MemberMultiLevelSummaryStats::<Test>::get(1, 40u64);
        assert_eq!(summary_40.last_payout_block, 200);
    });
}
