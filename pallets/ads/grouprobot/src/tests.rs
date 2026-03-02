use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};

const UNIT: u128 = 1_000_000_000_000;

// ============================================================================
// stake / unstake
// ============================================================================

#[test]
fn stake_for_ads_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 100 * UNIT,
		));

		assert_eq!(CommunityAdStake::<Test>::get(&ch), 100 * UNIT);
		assert_eq!(CommunityStakers::<Test>::get(&ch, STAKER), 100 * UNIT);
		assert_eq!(CommunityAudienceCap::<Test>::get(&ch), 5_000);
		// First staker becomes admin
		assert_eq!(CommunityAdmin::<Test>::get(&ch), Some(STAKER));
	});
}

#[test]
fn stake_cumulative_cap() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		// 10 UNIT → cap 1000
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 10 * UNIT,
		));
		assert_eq!(CommunityAudienceCap::<Test>::get(&ch), 1_000);

		// Add 90 UNIT → total 100 → cap 5000
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 90 * UNIT,
		));
		assert_eq!(CommunityAudienceCap::<Test>::get(&ch), 5_000);
	});
}

#[test]
fn stake_fails_zero() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsGroupRobot::stake_for_ads(RuntimeOrigin::signed(STAKER), community_hash(1), 0),
			Error::<Test>::ZeroStakeAmount
		);
	});
}

#[test]
fn unstake_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 100 * UNIT,
		));

		assert_ok!(AdsGroupRobot::unstake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 50 * UNIT,
		));
		assert_eq!(CommunityAdStake::<Test>::get(&ch), 50 * UNIT);
		assert_eq!(CommunityStakers::<Test>::get(&ch, STAKER), 50 * UNIT);
	});
}

#[test]
fn unstake_fails_insufficient() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 10 * UNIT,
		));
		assert_noop!(
			AdsGroupRobot::unstake_for_ads(RuntimeOrigin::signed(STAKER), ch, 20 * UNIT),
			Error::<Test>::InsufficientStake
		);
	});
}

// ============================================================================
// set_tee_ad_pct / set_community_ad_pct
// ============================================================================

#[test]
fn set_tee_ad_pct_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), 20));
		assert_eq!(TeeNodeAdPct::<Test>::get(), 20);
	});
}

#[test]
fn set_tee_ad_pct_fails_over_100() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), 101),
			Error::<Test>::InvalidPercentage
		);
	});
}

#[test]
fn set_community_ad_pct_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsGroupRobot::set_community_ad_pct(RuntimeOrigin::root(), 70));
		assert_eq!(CommunityAdPct::<Test>::get(), 70);
	});
}

// ============================================================================
// set_community_admin
// ============================================================================

#[test]
fn set_community_admin_by_current_admin() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		// Stake to become admin
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, UNIT,
		));
		assert_eq!(CommunityAdmin::<Test>::get(&ch), Some(STAKER));

		// Current admin changes to STAKER2
		assert_ok!(AdsGroupRobot::set_community_admin(
			RuntimeOrigin::signed(STAKER), ch, STAKER2,
		));
		assert_eq!(CommunityAdmin::<Test>::get(&ch), Some(STAKER2));
	});
}

#[test]
fn set_community_admin_by_bot_owner() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		// BOT_OWNER is bot_owner for community_hash(1)
		assert_ok!(AdsGroupRobot::set_community_admin(
			RuntimeOrigin::signed(BOT_OWNER), ch, STAKER2,
		));
		assert_eq!(CommunityAdmin::<Test>::get(&ch), Some(STAKER2));
	});
}

#[test]
fn set_community_admin_fails_not_authorized() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_noop!(
			AdsGroupRobot::set_community_admin(
				RuntimeOrigin::signed(STAKER2), ch, STAKER,
			),
			Error::<Test>::NotCommunityAdmin
		);
	});
}

// ============================================================================
// report_node_audience
// ============================================================================

#[test]
fn report_node_audience_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		let nid = node_id(NODE_OPERATOR as u8, false);

		assert_ok!(AdsGroupRobot::report_node_audience(
			RuntimeOrigin::signed(NODE_OPERATOR), ch, nid, 100,
		));

		let reports = NodeAudienceReports::<Test>::get(&ch);
		assert_eq!(reports.len(), 1);
		assert_eq!(reports[0], (NODE_OPERATOR as u32, 100));
	});
}

#[test]
fn report_node_audience_fails_not_operator() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		let nid = node_id(NODE_OPERATOR as u8, false);

		assert_noop!(
			AdsGroupRobot::report_node_audience(
				RuntimeOrigin::signed(STAKER), ch, nid, 100,
			),
			Error::<Test>::NodeOperatorMismatch
		);
	});
}

// ============================================================================
// check_audience_surge
// ============================================================================

#[test]
fn check_audience_surge_triggers_pause() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);

		// Set previous era audience
		PreviousEraAudience::<Test>::insert(&ch, 100);

		// Current audience = 250 → increase 150% > threshold 100%
		assert_ok!(AdsGroupRobot::check_audience_surge(
			RuntimeOrigin::signed(STAKER), ch, 250,
		));

		assert_eq!(AudienceSurgePaused::<Test>::get(&ch), 1);
	});
}

#[test]
fn check_audience_surge_no_pause_within_threshold() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		PreviousEraAudience::<Test>::insert(&ch, 100);

		// Current audience = 180 → increase 80% < threshold 100%
		assert_ok!(AdsGroupRobot::check_audience_surge(
			RuntimeOrigin::signed(STAKER), ch, 180,
		));

		assert_eq!(AudienceSurgePaused::<Test>::get(&ch), 0);
	});
}

// ============================================================================
// validate_node_reports (L5)
// ============================================================================

#[test]
fn validate_node_reports_passes_within_threshold() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		NodeAudienceReports::<Test>::insert(&ch,
			BoundedVec::<(u32, u32), ConstU32<10>>::try_from(vec![
				(1, 100), (2, 115),
			]).unwrap()
		);

		// 15% deviation < 20% threshold
		assert!(Pallet::<Test>::validate_node_reports(&ch).is_ok());
	});
}

#[test]
fn validate_node_reports_fails_high_deviation() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		NodeAudienceReports::<Test>::insert(&ch,
			BoundedVec::<(u32, u32), ConstU32<10>>::try_from(vec![
				(1, 100), (2, 150),
			]).unwrap()
		);

		// 50% deviation > 20% threshold
		let result = Pallet::<Test>::validate_node_reports(&ch);
		assert_eq!(result, Err((100, 150)));
	});
}

// ============================================================================
// DeliveryVerifier impl
// ============================================================================

#[test]
fn delivery_verifier_caps_audience() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		// Pro tier → tee_access = true, can_disable_ads = true
		// Need stake to pass AdsDisabledByTier
		CommunityAdStake::<Test>::insert(&ch, 100 * UNIT);
		CommunityAudienceCap::<Test>::insert(&ch, 200);

		let result = <Pallet<Test> as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&TEE_NODE_OPERATOR,
			&ch,
			500,
		);

		assert_eq!(result, Ok(200)); // capped to 200
	});
}

#[test]
fn delivery_verifier_fails_not_tee() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		CommunityAdStake::<Test>::insert(&ch, 100 * UNIT);

		let result = <Pallet<Test> as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&NODE_OPERATOR, // not TEE
			&ch,
			100,
		);

		assert!(result.is_err());
	});
}

// ============================================================================
// PlacementAdminProvider impl
// ============================================================================

#[test]
fn placement_admin_from_storage() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		CommunityAdmin::<Test>::insert(&ch, STAKER);

		let admin = <Pallet<Test> as pallet_ads_primitives::PlacementAdminProvider<u64>>::placement_admin(&ch);
		assert_eq!(admin, Some(STAKER));
	});
}

#[test]
fn placement_admin_fallback_to_bot_owner() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		// No CommunityAdmin set, falls back to bot_owner
		let admin = <Pallet<Test> as pallet_ads_primitives::PlacementAdminProvider<u64>>::placement_admin(&ch);
		assert_eq!(admin, Some(BOT_OWNER));
	});
}

// ============================================================================
// RevenueDistributor impl
// ============================================================================

#[test]
fn revenue_distributor_default_80pct() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		let result = <Pallet<Test> as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&ch,
			100 * UNIT,
			&STAKER,
		);

		// Default community_pct = 80%
		assert_eq!(result, Ok(80 * UNIT));
	});
}

#[test]
fn revenue_distributor_custom_pct() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		CommunityAdPct::<Test>::put(60);

		let result = <Pallet<Test> as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&ch,
			100 * UNIT,
			&STAKER,
		);

		assert_eq!(result, Ok(60 * UNIT));
	});
}

// ============================================================================
// compute_audience_cap
// ============================================================================

#[test]
fn compute_audience_cap_tiers() {
	new_test_ext().execute_with(|| {
		assert_eq!(Pallet::<Test>::compute_audience_cap(0), 0);
		assert_eq!(Pallet::<Test>::compute_audience_cap(UNIT / 2), 0);
		assert_eq!(Pallet::<Test>::compute_audience_cap(UNIT), 200);
		assert_eq!(Pallet::<Test>::compute_audience_cap(10 * UNIT), 1_000);
		assert_eq!(Pallet::<Test>::compute_audience_cap(100 * UNIT), 5_000);
		assert_eq!(Pallet::<Test>::compute_audience_cap(1000 * UNIT), 10_000);
	});
}
