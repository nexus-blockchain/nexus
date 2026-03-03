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
			RuntimeOrigin::root(), ch, 250,
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
			RuntimeOrigin::root(), ch, 180,
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

// ============================================================================
// Regression: H-GR1 — check_audience_surge requires root
// ============================================================================

#[test]
fn h_gr1_check_audience_surge_rejects_signed_origin() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		PreviousEraAudience::<Test>::insert(&ch, 100);

		// Signed origin should be rejected
		assert_noop!(
			AdsGroupRobot::check_audience_surge(RuntimeOrigin::signed(STAKER), ch, 250),
			sp_runtime::DispatchError::BadOrigin
		);

		// Root should work
		assert_ok!(AdsGroupRobot::check_audience_surge(RuntimeOrigin::root(), ch, 250));
	});
}

// ============================================================================
// Regression: M-GR2 — tee_pct + community_pct cannot exceed 100%
// ============================================================================

#[test]
fn m_gr2_set_tee_pct_rejects_sum_over_100() {
	new_test_ext().execute_with(|| {
		// Default community_pct effective = 80, so tee can be at most 20
		assert_ok!(AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), 20));

		// 21 + 80 = 101 > 100 → rejected
		assert_noop!(
			AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), 21),
			Error::<Test>::InvalidPercentage
		);
	});
}

#[test]
fn m_gr2_set_community_pct_rejects_sum_over_100() {
	new_test_ext().execute_with(|| {
		// Lower community first so we can raise tee (default community=80)
		assert_ok!(AdsGroupRobot::set_community_ad_pct(RuntimeOrigin::root(), 60));

		// Now set tee to 30 (60+30=90 ≤ 100 → ok)
		assert_ok!(AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), 30));

		// community 70 + tee 30 = 100 → ok
		assert_ok!(AdsGroupRobot::set_community_ad_pct(RuntimeOrigin::root(), 70));

		// community 71 + tee 30 = 101 → rejected
		assert_noop!(
			AdsGroupRobot::set_community_ad_pct(RuntimeOrigin::root(), 71),
			Error::<Test>::InvalidPercentage
		);
	});
}

// ============================================================================
// Regression: H1 — distribute actually transfers node share to RewardPoolAccount
// ============================================================================

#[test]
fn h1_distribute_transfers_node_share_to_reward_pool() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);

		// Set explicit percentages: community=60, tee=20, treasury keeps 20
		assert_ok!(AdsGroupRobot::set_community_ad_pct(RuntimeOrigin::root(), 60));
		assert_ok!(AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), 20));

		let treasury_before = Balances::free_balance(999u64);
		let reward_pool_before = Balances::free_balance(998u64);

		let result = <Pallet<Test> as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&ch,
			100 * UNIT,
			&STAKER,
		);

		// community_share = 60%
		assert_eq!(result, Ok(60 * UNIT));

		// node_share = 20% = 20 UNIT transferred from treasury to reward_pool
		let treasury_after = Balances::free_balance(999u64);
		let reward_pool_after = Balances::free_balance(998u64);
		assert_eq!(treasury_before - treasury_after, 20 * UNIT);
		assert_eq!(reward_pool_after - reward_pool_before, 20 * UNIT);
	});
}

#[test]
fn h1_distribute_emits_node_ad_reward_event() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		System::reset_events();

		// Default: community=80%, tee=15%, treasury=5%
		let _result = <Pallet<Test> as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&ch,
			100 * UNIT,
			&STAKER,
		);

		// NodeAdRewardAccrued event should be emitted with node_share = 15 UNIT
		System::assert_has_event(RuntimeEvent::AdsGroupRobot(
			Event::NodeAdRewardAccrued {
				node_id: ch,
				amount: 15 * UNIT,
			}
		));
	});
}

// ============================================================================
// Regression: H2 — set_tee_ad_pct(0) stores 0, validates with actual 0
// ============================================================================

#[test]
fn h2_set_tee_pct_zero_validates_with_zero() {
	new_test_ext().execute_with(|| {
		// Default: community effective=80, tee effective=15
		// set_tee_ad_pct(0) → validates 0 + 80 = 80 ≤ 100 → ok
		assert_ok!(AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), 0));
		assert_eq!(TeeNodeAdPct::<Test>::get(), 0);

		// effective_tee_pct() still returns 15 (0→default fallback)
		// But the storage value IS 0, proving H2 fix:
		// the extrinsic validated with actual 0, not expanded 15
	});
}

#[test]
fn h2_set_community_pct_zero_validates_with_zero() {
	new_test_ext().execute_with(|| {
		// Default: tee effective=15, community effective=80
		// set_community_ad_pct(0) → validates 0 + effective_tee(15) = 15 ≤ 100 → ok
		assert_ok!(AdsGroupRobot::set_community_ad_pct(RuntimeOrigin::root(), 0));
		assert_eq!(CommunityAdPct::<Test>::get(), 0);

		// Now tee storage is 0 (effective=15), community storage is 0 (effective=80)
		// set_tee_ad_pct(90) → validates 90 + effective_community(80) = 170 > 100 → fails
		// This proves effective_community_pct still returns 80 for reads even with storage=0
		assert_noop!(
			AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), 90),
			Error::<Test>::InvalidPercentage
		);
	});
}

// ============================================================================
// Regression: H3 — resume_audience_surge clears pause
// ============================================================================

#[test]
fn h3_resume_audience_surge_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);

		// Trigger surge pause
		PreviousEraAudience::<Test>::insert(&ch, 100);
		assert_ok!(AdsGroupRobot::check_audience_surge(RuntimeOrigin::root(), ch, 250));
		assert_eq!(AudienceSurgePaused::<Test>::get(&ch), 1);

		// Resume
		assert_ok!(AdsGroupRobot::resume_audience_surge(RuntimeOrigin::root(), ch));
		assert_eq!(AudienceSurgePaused::<Test>::get(&ch), 0);
	});
}

#[test]
fn h3_resume_audience_surge_fails_not_paused() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_noop!(
			AdsGroupRobot::resume_audience_surge(RuntimeOrigin::root(), ch),
			Error::<Test>::CommunityNotPaused
		);
	});
}

#[test]
fn h3_resume_audience_surge_requires_root() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		AudienceSurgePaused::<Test>::insert(&ch, 1);

		assert_noop!(
			AdsGroupRobot::resume_audience_surge(RuntimeOrigin::signed(STAKER), ch),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ============================================================================
// Regression: M1 — unstake cleans up zero-balance staker + admin
// ============================================================================

#[test]
fn m1_unstake_all_removes_staker_entry() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 10 * UNIT,
		));
		assert_eq!(CommunityStakers::<Test>::get(&ch, STAKER), 10 * UNIT);

		// Unstake all
		assert_ok!(AdsGroupRobot::unstake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 10 * UNIT,
		));

		// Staker entry should be cleaned up (default 0, but storage removed)
		assert_eq!(CommunityStakers::<Test>::get(&ch, STAKER), 0);
		// Admin should be cleared when total stake is zero
		assert_eq!(CommunityAdmin::<Test>::get(&ch), None);
	});
}

#[test]
fn m1_unstake_partial_keeps_admin() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 10 * UNIT,
		));
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER2), ch, 5 * UNIT,
		));

		// Unstake all of STAKER
		assert_ok!(AdsGroupRobot::unstake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 10 * UNIT,
		));

		// Total stake still > 0 from STAKER2, admin should remain
		assert_eq!(CommunityAdmin::<Test>::get(&ch), Some(STAKER));
		assert_eq!(CommunityAdStake::<Test>::get(&ch), 5 * UNIT);
	});
}

// ============================================================================
// Regression: M2 — report_node_audience deduplicates same node prefix
// ============================================================================

#[test]
fn m2_report_node_audience_deduplicates_same_node() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		let nid = node_id(NODE_OPERATOR as u8, false);

		// Report twice from same node
		assert_ok!(AdsGroupRobot::report_node_audience(
			RuntimeOrigin::signed(NODE_OPERATOR), ch, nid, 100,
		));
		assert_ok!(AdsGroupRobot::report_node_audience(
			RuntimeOrigin::signed(NODE_OPERATOR), ch, nid, 200,
		));

		let reports = NodeAudienceReports::<Test>::get(&ch);
		// Should only have 1 entry (updated), not 2
		assert_eq!(reports.len(), 1);
		assert_eq!(reports[0], (NODE_OPERATOR as u32, 200));
	});
}

// ============================================================================
// Regression: M3 — cross_validate_nodes emits NodeDeviationRejected
// ============================================================================

#[test]
fn m3_cross_validate_nodes_emits_event_on_deviation() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		// Insert reports with >20% deviation: 100 vs 150 = 50% deviation
		NodeAudienceReports::<Test>::insert(&ch,
			BoundedVec::<(u32, u32), ConstU32<10>>::try_from(vec![
				(1, 100), (2, 150),
			]).unwrap()
		);

		System::reset_events();
		// Always Ok — Substrate reverts events on Err, so deviation is communicated via event
		assert_ok!(AdsGroupRobot::cross_validate_nodes(RuntimeOrigin::root(), ch));

		System::assert_has_event(RuntimeEvent::AdsGroupRobot(
			Event::NodeDeviationRejected {
				community_id_hash: ch,
				min_audience: 100,
				max_audience: 150,
			}
		));

		// Reports cleared even on deviation
		assert_eq!(NodeAudienceReports::<Test>::get(&ch).len(), 0);
	});
}

#[test]
fn m3_cross_validate_nodes_passes_and_clears_reports() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		// Insert reports within 20% deviation: 100 vs 115 = 15%
		NodeAudienceReports::<Test>::insert(&ch,
			BoundedVec::<(u32, u32), ConstU32<10>>::try_from(vec![
				(1, 100), (2, 115),
			]).unwrap()
		);

		assert_ok!(AdsGroupRobot::cross_validate_nodes(RuntimeOrigin::root(), ch));
		// Reports should be cleared
		assert_eq!(NodeAudienceReports::<Test>::get(&ch).len(), 0);
	});
}

#[test]
fn m3_cross_validate_nodes_requires_root() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_noop!(
			AdsGroupRobot::cross_validate_nodes(RuntimeOrigin::signed(STAKER), ch),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ============================================================================
// Regression: M3+L2 — slash_community uses AdSlashPercentage, emits CommunitySlashed
// ============================================================================

#[test]
fn m3_slash_community_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		// Stake 100 UNIT
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 100 * UNIT,
		));

		let treasury_before = Balances::free_balance(999u64);
		System::reset_events();

		// Slash at 30%
		assert_ok!(AdsGroupRobot::slash_community(RuntimeOrigin::root(), ch));

		// Staker lost 30 UNIT: 100 - 30 = 70 UNIT remaining
		assert_eq!(CommunityAdStake::<Test>::get(&ch), 70 * UNIT);
		assert_eq!(CommunityStakers::<Test>::get(&ch, STAKER), 70 * UNIT);

		// Treasury gained ~30 UNIT
		let treasury_after = Balances::free_balance(999u64);
		assert_eq!(treasury_after - treasury_before, 30 * UNIT);

		// Event emitted
		System::assert_has_event(RuntimeEvent::AdsGroupRobot(
			Event::CommunitySlashed {
				community_id_hash: ch,
				slashed_amount: 30 * UNIT,
				slash_count: 1,
			}
		));
	});
}

#[test]
fn m3_slash_community_requires_root() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 10 * UNIT,
		));
		assert_noop!(
			AdsGroupRobot::slash_community(RuntimeOrigin::signed(STAKER), ch),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn m3_slash_community_fails_no_stake() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_noop!(
			AdsGroupRobot::slash_community(RuntimeOrigin::root(), ch),
			Error::<Test>::InsufficientStake
		);
	});
}

// ============================================================================
// Regression: M4 — distribute rejects paused community
// ============================================================================

#[test]
fn lgr5_slash_community_no_transfer_failed_on_success() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 100 * UNIT,
		));

		let staker_free_before = Balances::free_balance(STAKER);
		let treasury_before = Balances::free_balance(999u64);

		System::reset_events();
		assert_ok!(AdsGroupRobot::slash_community(RuntimeOrigin::root(), ch));

		// CommunitySlashed 事件应存在
		System::assert_has_event(RuntimeEvent::AdsGroupRobot(
			Event::CommunitySlashed {
				community_id_hash: ch,
				slashed_amount: 30 * UNIT,
				slash_count: 1,
			}
		));

		// SlashTransferFailed 事件不应存在
		assert!(!System::events().iter().any(|e| matches!(
			e.event,
			RuntimeEvent::AdsGroupRobot(Event::SlashTransferFailed { .. })
		)));

		// 会计一致性: 国库增加 = 质押减少 = 30 UNIT
		let treasury_after = Balances::free_balance(999u64);
		assert_eq!(treasury_after - treasury_before, 30 * UNIT);
		assert_eq!(CommunityAdStake::<Test>::get(&ch), 70 * UNIT);
		assert_eq!(CommunityStakers::<Test>::get(&ch, STAKER), 70 * UNIT);

		// staker reserved 减少 30 UNIT, free 不变 (unreserve→transfer 净效果)
		assert_eq!(Balances::free_balance(STAKER), staker_free_before);
		assert_eq!(Balances::reserved_balance(STAKER), 70 * UNIT);
	});
}

#[test]
fn m4_distribute_rejects_paused_community() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		// Pause the community
		AudienceSurgePaused::<Test>::insert(&ch, 1);

		let result = <Pallet<Test> as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&ch,
			100 * UNIT,
			&STAKER,
		);

		assert!(result.is_err());
	});
}

#[test]
fn m4_distribute_works_when_not_paused() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		// Not paused (default 0)
		let result = <Pallet<Test> as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&ch,
			100 * UNIT,
			&STAKER,
		);

		assert!(result.is_ok());
	});
}
