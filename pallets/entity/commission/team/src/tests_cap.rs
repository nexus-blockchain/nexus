use crate::mock::*;
use crate::pallet;
use frame_support::{assert_noop, assert_ok, BoundedVec};

const ENTITY_ID: u64 = 1;
const OWNER: u64 = 100;

// ============================================================================
// Budget Cap Validation Tests
// ============================================================================

#[test]
fn set_team_performance_config_rejects_max_rate_exceeding_budget_cap() {
    new_test_ext().execute_with(|| {
        set_budget_cap(ENTITY_ID, 2000); // cap = 20%
        let tiers: BoundedVec<_, _> = vec![
            pallet::TeamPerformanceTier {
                sales_threshold: 1000u128,
                min_team_size: 0,
                rate: 1000,
            },
            pallet::TeamPerformanceTier {
                sales_threshold: 5000u128,
                min_team_size: 0,
                rate: 2001,
            }, // exceeds cap
        ]
        .try_into()
        .unwrap();
        assert_noop!(
            CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(OWNER),
                ENTITY_ID,
                tiers,
                5,
                false,
                pallet::SalesThresholdMode::TotalSpent,
            ),
            pallet::Error::<Test>::MaxRateExceedsBudgetCap
        );
        // max rate == cap should pass
        let tiers_ok: BoundedVec<_, _> = vec![
            pallet::TeamPerformanceTier {
                sales_threshold: 1000u128,
                min_team_size: 0,
                rate: 1000,
            },
            pallet::TeamPerformanceTier {
                sales_threshold: 5000u128,
                min_team_size: 0,
                rate: 2000,
            },
        ]
        .try_into()
        .unwrap();
        assert_ok!(CommissionTeam::set_team_performance_config(
            RuntimeOrigin::signed(OWNER),
            ENTITY_ID,
            tiers_ok,
            5,
            false,
            pallet::SalesThresholdMode::TotalSpent,
        ));
    });
}

#[test]
fn force_set_team_performance_config_rejects_max_rate_exceeding_budget_cap() {
    new_test_ext().execute_with(|| {
        set_budget_cap(ENTITY_ID, 1500);
        let tiers: BoundedVec<_, _> = vec![pallet::TeamPerformanceTier {
            sales_threshold: 1000u128,
            min_team_size: 0,
            rate: 1501,
        }]
        .try_into()
        .unwrap();
        assert_noop!(
            CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(),
                ENTITY_ID,
                tiers,
                5,
                false,
                pallet::SalesThresholdMode::TotalSpent,
            ),
            pallet::Error::<Test>::MaxRateExceedsBudgetCap
        );
    });
}

#[test]
fn add_tier_rejects_rate_exceeding_budget_cap() {
    new_test_ext().execute_with(|| {
        // First set a config
        let tiers: BoundedVec<_, _> = vec![pallet::TeamPerformanceTier {
            sales_threshold: 1000u128,
            min_team_size: 0,
            rate: 500,
        }]
        .try_into()
        .unwrap();
        assert_ok!(CommissionTeam::set_team_performance_config(
            RuntimeOrigin::signed(OWNER),
            ENTITY_ID,
            tiers,
            5,
            false,
            pallet::SalesThresholdMode::TotalSpent,
        ));
        // Now set cap and try to add a tier with rate > cap
        set_budget_cap(ENTITY_ID, 1000);
        assert_noop!(
            CommissionTeam::add_tier(
                RuntimeOrigin::signed(OWNER),
                ENTITY_ID,
                pallet::TeamPerformanceTier {
                    sales_threshold: 5000u128,
                    min_team_size: 0,
                    rate: 1001
                },
            ),
            pallet::Error::<Test>::MaxRateExceedsBudgetCap
        );
        // rate == cap should pass
        assert_ok!(CommissionTeam::add_tier(
            RuntimeOrigin::signed(OWNER),
            ENTITY_ID,
            pallet::TeamPerformanceTier {
                sales_threshold: 5000u128,
                min_team_size: 0,
                rate: 1000
            },
        ));
    });
}

#[test]
fn update_tier_rejects_rate_exceeding_budget_cap() {
    new_test_ext().execute_with(|| {
        let tiers: BoundedVec<_, _> = vec![pallet::TeamPerformanceTier {
            sales_threshold: 1000u128,
            min_team_size: 0,
            rate: 500,
        }]
        .try_into()
        .unwrap();
        assert_ok!(CommissionTeam::set_team_performance_config(
            RuntimeOrigin::signed(OWNER),
            ENTITY_ID,
            tiers,
            5,
            false,
            pallet::SalesThresholdMode::TotalSpent,
        ));
        set_budget_cap(ENTITY_ID, 800);
        assert_noop!(
            CommissionTeam::update_tier(
                RuntimeOrigin::signed(OWNER),
                ENTITY_ID,
                0,
                None,
                None,
                Some(801)
            ),
            pallet::Error::<Test>::MaxRateExceedsBudgetCap
        );
        assert_ok!(CommissionTeam::update_tier(
            RuntimeOrigin::signed(OWNER),
            ENTITY_ID,
            0,
            None,
            None,
            Some(800)
        ));
    });
}

#[test]
fn budget_cap_zero_means_no_limit() {
    new_test_ext().execute_with(|| {
        // cap=0 default, rate=10000 should pass
        let tiers: BoundedVec<_, _> = vec![pallet::TeamPerformanceTier {
            sales_threshold: 1000u128,
            min_team_size: 0,
            rate: 10000,
        }]
        .try_into()
        .unwrap();
        assert_ok!(CommissionTeam::set_team_performance_config(
            RuntimeOrigin::signed(OWNER),
            ENTITY_ID,
            tiers,
            5,
            false,
            pallet::SalesThresholdMode::TotalSpent,
        ));
    });
}

#[test]
fn plan_writer_rejects_max_rate_exceeding_budget_cap() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::TeamPlanWriter;
        set_budget_cap(ENTITY_ID, 1000);
        assert!(
            <pallet::Pallet<Test> as TeamPlanWriter<u128>>::set_team_config(
                ENTITY_ID,
                vec![(1000, 0, 1001)],
                5,
                false,
                0,
            )
            .is_err()
        );
        assert_ok!(
            <pallet::Pallet<Test> as TeamPlanWriter<u128>>::set_team_config(
                ENTITY_ID,
                vec![(1000, 0, 1000)],
                5,
                false,
                0,
            )
        );
    });
}
