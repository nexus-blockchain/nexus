use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok, traits::Currency};

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
		assert_eq!(CommunityAdmin::<Test>::get(&ch), Some(STAKER));
		assert_eq!(CommunityStakerCount::<Test>::get(&ch), 1);
	});
}

#[test]
fn stake_cumulative_cap() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 10 * UNIT,
		));
		assert_eq!(CommunityAudienceCap::<Test>::get(&ch), 1_000);

		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 90 * UNIT,
		));
		assert_eq!(CommunityAudienceCap::<Test>::get(&ch), 5_000);
		// Same staker adding more should not increment count
		assert_eq!(CommunityStakerCount::<Test>::get(&ch), 1);
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
fn unstake_works_with_unbonding() {
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
		let queue = UnbondingRequests::<Test>::get(&ch, STAKER);
		assert_eq!(queue.len(), 1);
		assert_eq!(queue[0], (50 * UNIT, 11)); // block 1 + 10
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
// Multiple parallel unbonding requests
// ============================================================================

#[test]
fn multiple_unbonding_requests_work() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 100 * UNIT,
		));

		assert_ok!(AdsGroupRobot::unstake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 20 * UNIT,
		));
		assert_ok!(AdsGroupRobot::unstake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 30 * UNIT,
		));

		let queue = UnbondingRequests::<Test>::get(&ch, STAKER);
		assert_eq!(queue.len(), 2);
		assert_eq!(queue[0].0, 20 * UNIT);
		assert_eq!(queue[1].0, 30 * UNIT);
		assert_eq!(CommunityAdStake::<Test>::get(&ch), 50 * UNIT);
	});
}

#[test]
fn withdraw_unbonded_partial_maturity() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 100 * UNIT,
		));

		// First unstake at block 1 → unlocks at 11
		assert_ok!(AdsGroupRobot::unstake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 20 * UNIT,
		));
		// Advance to block 5, second unstake → unlocks at 15
		System::set_block_number(5);
		assert_ok!(AdsGroupRobot::unstake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 30 * UNIT,
		));

		// At block 12: first request matured, second not yet
		System::set_block_number(12);
		assert_ok!(AdsGroupRobot::withdraw_unbonded(
			RuntimeOrigin::signed(STAKER), ch,
		));
		// Only 20 UNIT withdrawn
		let queue = UnbondingRequests::<Test>::get(&ch, STAKER);
		assert_eq!(queue.len(), 1);
		assert_eq!(queue[0].0, 30 * UNIT);

		// At block 16: second request matured
		System::set_block_number(16);
		assert_ok!(AdsGroupRobot::withdraw_unbonded(
			RuntimeOrigin::signed(STAKER), ch,
		));
		let queue = UnbondingRequests::<Test>::get(&ch, STAKER);
		assert_eq!(queue.len(), 0);
	});
}

// ============================================================================
// MaxStakersPerCommunity
// ============================================================================

#[test]
fn stake_fails_max_stakers_reached() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		// MaxStakersPerCommunity = 50 in mock
		// Fill up with stakers (use accounts 100..149)
		for i in 100u64..150 {
			// Fund the account
			let _ = Balances::make_free_balance_be(&i, 10 * UNIT);
			assert_ok!(AdsGroupRobot::stake_for_ads(
				RuntimeOrigin::signed(i), ch, UNIT,
			));
		}
		assert_eq!(CommunityStakerCount::<Test>::get(&ch), 50);

		// 51st staker should fail
		let _ = Balances::make_free_balance_be(&200, 10 * UNIT);
		assert_noop!(
			AdsGroupRobot::stake_for_ads(RuntimeOrigin::signed(200), ch, UNIT),
			Error::<Test>::MaxStakersReached
		);

		// Existing staker adding more should still work
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(100), ch, UNIT,
		));
	});
}

// ============================================================================
// set_tee_ad_pct / set_community_ad_pct
// ============================================================================

#[test]
fn set_tee_ad_pct_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), Some(20)));
		assert_eq!(TeeNodeAdPct::<Test>::get(), Some(20));
	});
}

#[test]
fn set_tee_ad_pct_none_resets_default() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), Some(20)));
		assert_ok!(AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), None));
		assert_eq!(TeeNodeAdPct::<Test>::get(), None);
		assert_eq!(Pallet::<Test>::effective_tee_pct(), 15);
	});
}

#[test]
fn set_tee_ad_pct_fails_over_100() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), Some(101)),
			Error::<Test>::InvalidPercentage
		);
	});
}

#[test]
fn set_community_ad_pct_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsGroupRobot::set_community_ad_pct(RuntimeOrigin::root(), Some(70)));
		assert_eq!(CommunityAdPct::<Test>::get(), Some(70));
	});
}

#[test]
fn set_community_ad_pct_none_resets_default() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsGroupRobot::set_community_ad_pct(RuntimeOrigin::root(), Some(50)));
		assert_ok!(AdsGroupRobot::set_community_ad_pct(RuntimeOrigin::root(), None));
		assert_eq!(CommunityAdPct::<Test>::get(), None);
		assert_eq!(Pallet::<Test>::effective_community_pct(), 80);
	});
}

// ============================================================================
// set_community_admin
// ============================================================================

#[test]
fn set_community_admin_by_current_admin() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, UNIT,
		));
		assert_eq!(CommunityAdmin::<Test>::get(&ch), Some(STAKER));

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
		PreviousEraAudience::<Test>::insert(&ch, 100);

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
		CommunityAdStake::<Test>::insert(&ch, 100 * UNIT);
		CommunityAudienceCap::<Test>::insert(&ch, 200);

		let result = <Pallet<Test> as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&TEE_NODE_OPERATOR,
			&ch,
			500,
			None,
		);

		assert_eq!(result, Ok(200));
	});
}

#[test]
fn delivery_verifier_fails_not_tee() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		CommunityAdStake::<Test>::insert(&ch, 100 * UNIT);

		let result = <Pallet<Test> as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&NODE_OPERATOR,
			&ch,
			100,
			None,
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
		let admin = <Pallet<Test> as pallet_ads_primitives::PlacementAdminProvider<u64>>::placement_admin(&ch);
		assert_eq!(admin, Some(BOT_OWNER));
	});
}

#[test]
fn placement_status_returns_paused_for_admin_paused() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		AdminPausedAds::<Test>::insert(&ch, true);
		let status = <Pallet<Test> as pallet_ads_primitives::PlacementAdminProvider<u64>>::placement_status(&ch);
		assert_eq!(status, pallet_ads_primitives::PlacementStatus::Paused);
	});
}

#[test]
fn placement_status_returns_paused_for_surge_paused() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		AudienceSurgePaused::<Test>::insert(&ch, 1);
		let status = <Pallet<Test> as pallet_ads_primitives::PlacementAdminProvider<u64>>::placement_status(&ch);
		assert_eq!(status, pallet_ads_primitives::PlacementStatus::Paused);
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

		assert_eq!(result.unwrap().placement_share, 72 * UNIT);
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

		assert_eq!(result.unwrap().placement_share, 54 * UNIT);
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

		assert_noop!(
			AdsGroupRobot::check_audience_surge(RuntimeOrigin::signed(STAKER), ch, 250),
			sp_runtime::DispatchError::BadOrigin
		);

		assert_ok!(AdsGroupRobot::check_audience_surge(RuntimeOrigin::root(), ch, 250));
	});
}

// ============================================================================
// Regression: M-GR2 — tee_pct + community_pct cannot exceed 100%
// ============================================================================

#[test]
fn m_gr2_set_tee_pct_rejects_sum_over_100() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), Some(20)));

		assert_noop!(
			AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), Some(21)),
			Error::<Test>::InvalidPercentage
		);
	});
}

#[test]
fn m_gr2_set_community_pct_rejects_sum_over_100() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsGroupRobot::set_community_ad_pct(RuntimeOrigin::root(), Some(60)));
		assert_ok!(AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), Some(30)));
		assert_ok!(AdsGroupRobot::set_community_ad_pct(RuntimeOrigin::root(), Some(70)));

		assert_noop!(
			AdsGroupRobot::set_community_ad_pct(RuntimeOrigin::root(), Some(71)),
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

		assert_ok!(AdsGroupRobot::set_community_ad_pct(RuntimeOrigin::root(), Some(60)));
		assert_ok!(AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), Some(20)));

		let treasury_before = Balances::free_balance(999u64);
		let reward_pool_before = Balances::free_balance(998u64);

		let result = <Pallet<Test> as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&ch,
			100 * UNIT,
			&STAKER,
		);

		assert_eq!(result.unwrap().placement_share, 54 * UNIT);

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

		let _result = <Pallet<Test> as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&ch,
			100 * UNIT,
			&STAKER,
		);

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
		assert_ok!(AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), Some(0)));
		assert_eq!(TeeNodeAdPct::<Test>::get(), Some(0));
		assert_eq!(Pallet::<Test>::effective_tee_pct(), 0);

		assert_ok!(AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), None));
		assert_eq!(TeeNodeAdPct::<Test>::get(), None);
		assert_eq!(Pallet::<Test>::effective_tee_pct(), 15);
	});
}

#[test]
fn h2_set_community_pct_zero_validates_with_zero() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsGroupRobot::set_community_ad_pct(RuntimeOrigin::root(), Some(0)));
		assert_eq!(CommunityAdPct::<Test>::get(), Some(0));
		assert_eq!(Pallet::<Test>::effective_community_pct(), 0);

		assert_ok!(AdsGroupRobot::set_community_ad_pct(RuntimeOrigin::root(), None));
		assert_eq!(Pallet::<Test>::effective_community_pct(), 80);

		assert_noop!(
			AdsGroupRobot::set_tee_ad_pct(RuntimeOrigin::root(), Some(90)),
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
		PreviousEraAudience::<Test>::insert(&ch, 100);
		assert_ok!(AdsGroupRobot::check_audience_surge(RuntimeOrigin::root(), ch, 250));
		assert_eq!(AudienceSurgePaused::<Test>::get(&ch), 1);

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

		assert_ok!(AdsGroupRobot::unstake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 10 * UNIT,
		));

		assert_eq!(CommunityStakers::<Test>::get(&ch, STAKER), 0);
		assert_eq!(CommunityAdmin::<Test>::get(&ch), None);
		assert_eq!(CommunityStakerCount::<Test>::get(&ch), 0);
		assert!(!UnbondingRequests::<Test>::get(&ch, STAKER).is_empty());
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

		assert_ok!(AdsGroupRobot::unstake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 10 * UNIT,
		));

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

		assert_ok!(AdsGroupRobot::report_node_audience(
			RuntimeOrigin::signed(NODE_OPERATOR), ch, nid, 100,
		));
		assert_ok!(AdsGroupRobot::report_node_audience(
			RuntimeOrigin::signed(NODE_OPERATOR), ch, nid, 200,
		));

		let reports = NodeAudienceReports::<Test>::get(&ch);
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
		NodeAudienceReports::<Test>::insert(&ch,
			BoundedVec::<(u32, u32), ConstU32<10>>::try_from(vec![
				(1, 100), (2, 150),
			]).unwrap()
		);

		System::reset_events();
		assert_ok!(AdsGroupRobot::cross_validate_nodes(RuntimeOrigin::root(), ch));

		System::assert_has_event(RuntimeEvent::AdsGroupRobot(
			Event::NodeDeviationRejected {
				community_id_hash: ch,
				min_audience: 100,
				max_audience: 150,
			}
		));

		assert_eq!(NodeAudienceReports::<Test>::get(&ch).len(), 0);
	});
}

#[test]
fn m3_cross_validate_nodes_passes_and_clears_reports() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		NodeAudienceReports::<Test>::insert(&ch,
			BoundedVec::<(u32, u32), ConstU32<10>>::try_from(vec![
				(1, 100), (2, 115),
			]).unwrap()
		);

		assert_ok!(AdsGroupRobot::cross_validate_nodes(RuntimeOrigin::root(), ch));
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
// Regression: M3+L2 — slash_community
// ============================================================================

#[test]
fn m3_slash_community_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 100 * UNIT,
		));

		let treasury_before = Balances::free_balance(999u64);
		System::reset_events();

		assert_ok!(AdsGroupRobot::slash_community(RuntimeOrigin::root(), ch));

		assert_eq!(CommunityAdStake::<Test>::get(&ch), 70 * UNIT);
		assert_eq!(CommunityStakers::<Test>::get(&ch, STAKER), 70 * UNIT);

		let treasury_after = Balances::free_balance(999u64);
		assert_eq!(treasury_after - treasury_before, 30 * UNIT);

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

		System::assert_has_event(RuntimeEvent::AdsGroupRobot(
			Event::CommunitySlashed {
				community_id_hash: ch,
				slashed_amount: 30 * UNIT,
				slash_count: 1,
			}
		));

		assert!(!System::events().iter().any(|e| matches!(
			e.event,
			RuntimeEvent::AdsGroupRobot(Event::SlashTransferFailed { .. })
		)));

		let treasury_after = Balances::free_balance(999u64);
		assert_eq!(treasury_after - treasury_before, 30 * UNIT);
		assert_eq!(CommunityAdStake::<Test>::get(&ch), 70 * UNIT);
		assert_eq!(CommunityStakers::<Test>::get(&ch, STAKER), 70 * UNIT);

		assert_eq!(Balances::free_balance(STAKER), staker_free_before);
		assert_eq!(Balances::reserved_balance(STAKER), 70 * UNIT);
	});
}

#[test]
fn m4_distribute_rejects_paused_community() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
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
		let result = <Pallet<Test> as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&ch,
			100 * UNIT,
			&STAKER,
		);

		assert!(result.is_ok());
	});
}

// ============================================================================
// Unbonding + withdraw_unbonded
// ============================================================================

#[test]
fn withdraw_unbonded_works_after_period() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 50 * UNIT,
		));

		assert_ok!(AdsGroupRobot::unstake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 50 * UNIT,
		));
		assert!(!UnbondingRequests::<Test>::get(&ch, STAKER).is_empty());
		assert_eq!(Balances::reserved_balance(STAKER), 50 * UNIT);

		System::set_block_number(12);
		assert_ok!(AdsGroupRobot::withdraw_unbonded(
			RuntimeOrigin::signed(STAKER), ch,
		));
		assert_eq!(Balances::reserved_balance(STAKER), 0);
		assert!(UnbondingRequests::<Test>::get(&ch, STAKER).is_empty());
	});
}

#[test]
fn withdraw_unbonded_fails_before_period() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 50 * UNIT,
		));
		assert_ok!(AdsGroupRobot::unstake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 50 * UNIT,
		));

		assert_noop!(
			AdsGroupRobot::withdraw_unbonded(RuntimeOrigin::signed(STAKER), ch),
			Error::<Test>::UnbondingNotReady
		);
	});
}

#[test]
fn withdraw_unbonded_fails_no_request() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_noop!(
			AdsGroupRobot::withdraw_unbonded(RuntimeOrigin::signed(STAKER), ch),
			Error::<Test>::NothingToWithdraw
		);
	});
}

#[test]
fn unstake_emits_unbonding_started_event() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 50 * UNIT,
		));
		System::reset_events();
		assert_ok!(AdsGroupRobot::unstake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 20 * UNIT,
		));
		System::assert_has_event(RuntimeEvent::AdsGroupRobot(
			Event::UnbondingStarted {
				community_id_hash: ch,
				who: STAKER,
				amount: 20 * UNIT,
				unlock_at: 11,
			}
		));
	});
}

// ============================================================================
// admin_pause_ads / admin_resume_ads
// ============================================================================

#[test]
fn admin_pause_ads_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 10 * UNIT,
		));
		assert_ok!(AdsGroupRobot::admin_pause_ads(
			RuntimeOrigin::signed(STAKER), ch,
		));
		assert!(AdminPausedAds::<Test>::get(&ch));
	});
}

#[test]
fn admin_pause_ads_by_bot_owner() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::admin_pause_ads(
			RuntimeOrigin::signed(BOT_OWNER), ch,
		));
		assert!(AdminPausedAds::<Test>::get(&ch));
	});
}

#[test]
fn admin_pause_ads_fails_not_authorized() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_noop!(
			AdsGroupRobot::admin_pause_ads(RuntimeOrigin::signed(STAKER2), ch),
			Error::<Test>::NotCommunityAdmin
		);
	});
}

#[test]
fn admin_pause_ads_fails_already_paused() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::admin_pause_ads(
			RuntimeOrigin::signed(BOT_OWNER), ch,
		));
		assert_noop!(
			AdsGroupRobot::admin_pause_ads(RuntimeOrigin::signed(BOT_OWNER), ch),
			Error::<Test>::AdsPausedByAdmin
		);
	});
}

#[test]
fn admin_resume_ads_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::admin_pause_ads(
			RuntimeOrigin::signed(BOT_OWNER), ch,
		));
		assert_ok!(AdsGroupRobot::admin_resume_ads(
			RuntimeOrigin::signed(BOT_OWNER), ch,
		));
		assert!(!AdminPausedAds::<Test>::get(&ch));
	});
}

#[test]
fn admin_resume_ads_fails_not_paused() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_noop!(
			AdsGroupRobot::admin_resume_ads(RuntimeOrigin::signed(BOT_OWNER), ch),
			Error::<Test>::AdsNotPausedByAdmin
		);
	});
}

// ============================================================================
// resign_community_admin
// ============================================================================

#[test]
fn resign_community_admin_falls_back_to_bot_owner() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 10 * UNIT,
		));
		assert_eq!(CommunityAdmin::<Test>::get(&ch), Some(STAKER));

		assert_ok!(AdsGroupRobot::resign_community_admin(
			RuntimeOrigin::signed(STAKER), ch,
		));
		assert_eq!(CommunityAdmin::<Test>::get(&ch), Some(BOT_OWNER));
	});
}

#[test]
fn resign_community_admin_clears_if_no_bot_owner() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(3);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 10 * UNIT,
		));
		assert_ok!(AdsGroupRobot::resign_community_admin(
			RuntimeOrigin::signed(STAKER), ch,
		));
		assert_eq!(CommunityAdmin::<Test>::get(&ch), None);
	});
}

#[test]
fn resign_community_admin_fails_not_admin() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 10 * UNIT,
		));
		assert_noop!(
			AdsGroupRobot::resign_community_admin(RuntimeOrigin::signed(STAKER2), ch),
			Error::<Test>::NotCommunityAdmin
		);
	});
}

// ============================================================================
// set_stake_tiers
// ============================================================================

#[test]
fn set_stake_tiers_works() {
	new_test_ext().execute_with(|| {
		use frame_support::BoundedVec;
		let tiers = BoundedVec::<(u128, u32), ConstU32<10>>::try_from(vec![
			(500 * UNIT, 20_000),
			(50 * UNIT, 3_000),
			(5 * UNIT, 500),
		]).unwrap();
		assert_ok!(AdsGroupRobot::set_stake_tiers(RuntimeOrigin::root(), tiers));

		assert_eq!(Pallet::<Test>::compute_audience_cap((500 * UNIT).try_into().unwrap()), 20_000);
		assert_eq!(Pallet::<Test>::compute_audience_cap((50 * UNIT).try_into().unwrap()), 3_000);
		assert_eq!(Pallet::<Test>::compute_audience_cap((5 * UNIT).try_into().unwrap()), 500);
		assert_eq!(Pallet::<Test>::compute_audience_cap((4 * UNIT).try_into().unwrap()), 0);
	});
}

#[test]
fn set_stake_tiers_fails_empty() {
	new_test_ext().execute_with(|| {
		use frame_support::BoundedVec;
		let tiers = BoundedVec::<(u128, u32), ConstU32<10>>::try_from(vec![]).unwrap();
		assert_noop!(
			AdsGroupRobot::set_stake_tiers(RuntimeOrigin::root(), tiers),
			Error::<Test>::InvalidStakeTiers
		);
	});
}

#[test]
fn set_stake_tiers_fails_not_descending() {
	new_test_ext().execute_with(|| {
		use frame_support::BoundedVec;
		let tiers = BoundedVec::<(u128, u32), ConstU32<10>>::try_from(vec![
			(10 * UNIT, 500),
			(50 * UNIT, 3_000),
		]).unwrap();
		assert_noop!(
			AdsGroupRobot::set_stake_tiers(RuntimeOrigin::root(), tiers),
			Error::<Test>::InvalidStakeTiers
		);
	});
}

#[test]
fn set_stake_tiers_requires_root() {
	new_test_ext().execute_with(|| {
		use frame_support::BoundedVec;
		let tiers = BoundedVec::<(u128, u32), ConstU32<10>>::try_from(vec![
			(100 * UNIT, 5_000),
		]).unwrap();
		assert_noop!(
			AdsGroupRobot::set_stake_tiers(RuntimeOrigin::signed(STAKER), tiers),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ============================================================================
// force_set_community_admin
// ============================================================================

#[test]
fn force_set_community_admin_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::force_set_community_admin(
			RuntimeOrigin::root(), ch, STAKER2,
		));
		assert_eq!(CommunityAdmin::<Test>::get(&ch), Some(STAKER2));
	});
}

#[test]
fn force_set_community_admin_requires_root() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_noop!(
			AdsGroupRobot::force_set_community_admin(RuntimeOrigin::signed(STAKER), ch, STAKER2),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ============================================================================
// set_global_ads_pause
// ============================================================================

#[test]
fn global_ads_pause_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsGroupRobot::set_global_ads_pause(RuntimeOrigin::root(), true));
		assert!(GlobalAdsPaused::<Test>::get());

		assert_ok!(AdsGroupRobot::set_global_ads_pause(RuntimeOrigin::root(), false));
		assert!(!GlobalAdsPaused::<Test>::get());
	});
}

#[test]
fn global_ads_pause_requires_root() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsGroupRobot::set_global_ads_pause(RuntimeOrigin::signed(STAKER), true),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn global_pause_blocks_delivery_verifier() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		CommunityAdStake::<Test>::insert(&ch, 100 * UNIT);
		CommunityAudienceCap::<Test>::insert(&ch, 200);

		GlobalAdsPaused::<Test>::put(true);
		let result = <Pallet<Test> as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&TEE_NODE_OPERATOR, &ch, 100, None,
		);
		assert!(result.is_err());
	});
}

#[test]
fn global_pause_blocks_distribute() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		GlobalAdsPaused::<Test>::put(true);
		let result = <Pallet<Test> as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&ch, 100 * UNIT, &STAKER,
		);
		assert!(result.is_err());
	});
}

// ============================================================================
// set_bot_ads_enabled
// ============================================================================

#[test]
fn bot_ads_toggle_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::set_bot_ads_enabled(
			RuntimeOrigin::signed(BOT_OWNER), ch, true,
		));
		assert!(BotAdsDisabled::<Test>::get(&ch));

		assert_ok!(AdsGroupRobot::set_bot_ads_enabled(
			RuntimeOrigin::signed(BOT_OWNER), ch, false,
		));
		assert!(!BotAdsDisabled::<Test>::get(&ch));
	});
}

#[test]
fn bot_ads_toggle_fails_not_owner() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_noop!(
			AdsGroupRobot::set_bot_ads_enabled(RuntimeOrigin::signed(STAKER), ch, true),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn bot_ads_disabled_blocks_delivery_verifier() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		CommunityAdStake::<Test>::insert(&ch, 100 * UNIT);
		CommunityAudienceCap::<Test>::insert(&ch, 200);
		BotAdsDisabled::<Test>::insert(&ch, true);

		let result = <Pallet<Test> as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&TEE_NODE_OPERATOR, &ch, 100, None,
		);
		assert!(result.is_err());
	});
}

#[test]
fn admin_paused_blocks_delivery_verifier() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		CommunityAdStake::<Test>::insert(&ch, 100 * UNIT);
		CommunityAudienceCap::<Test>::insert(&ch, 200);
		AdminPausedAds::<Test>::insert(&ch, true);

		let result = <Pallet<Test> as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&TEE_NODE_OPERATOR, &ch, 100, None,
		);
		assert!(result.is_err());
	});
}

#[test]
fn admin_paused_blocks_distribute() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		AdminPausedAds::<Test>::insert(&ch, true);
		let result = <Pallet<Test> as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&ch, 100 * UNIT, &STAKER,
		);
		assert!(result.is_err());
	});
}

#[test]
fn bot_ads_disabled_blocks_distribute() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		BotAdsDisabled::<Test>::insert(&ch, true);
		let result = <Pallet<Test> as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&ch, 100 * UNIT, &STAKER,
		);
		assert!(result.is_err());
	});
}

// ============================================================================
// claim_staker_reward
// ============================================================================

#[test]
fn claim_staker_reward_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		StakerClaimable::<Test>::insert(&ch, STAKER, 5 * UNIT);

		let before = Balances::free_balance(STAKER);
		assert_ok!(AdsGroupRobot::claim_staker_reward(
			RuntimeOrigin::signed(STAKER), ch,
		));
		assert_eq!(Balances::free_balance(STAKER), before + 5 * UNIT);
		assert_eq!(StakerClaimable::<Test>::get(&ch, STAKER), 0);
	});
}

#[test]
fn claim_staker_reward_fails_nothing() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_noop!(
			AdsGroupRobot::claim_staker_reward(RuntimeOrigin::signed(STAKER), ch),
			Error::<Test>::NoClaimableReward
		);
	});
}

// ============================================================================
// force_unstake
// ============================================================================

#[test]
fn force_unstake_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 50 * UNIT,
		));
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER2), ch, 20 * UNIT,
		));

		System::reset_events();
		assert_ok!(AdsGroupRobot::force_unstake(RuntimeOrigin::root(), ch));

		assert_eq!(CommunityAdStake::<Test>::get(&ch), 0);
		assert_eq!(CommunityStakers::<Test>::get(&ch, STAKER), 0);
		assert_eq!(CommunityStakers::<Test>::get(&ch, STAKER2), 0);
		assert_eq!(CommunityAdmin::<Test>::get(&ch), None);
		assert_eq!(CommunityStakerCount::<Test>::get(&ch), 0);
		assert_eq!(Balances::reserved_balance(STAKER), 0);
		assert_eq!(Balances::reserved_balance(STAKER2), 0);

		System::assert_has_event(RuntimeEvent::AdsGroupRobot(
			Event::ForceUnstaked {
				community_id_hash: ch,
				total_amount: 70 * UNIT,
				staker_count: 2,
			}
		));
	});
}

#[test]
fn force_unstake_fails_no_stake() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_noop!(
			AdsGroupRobot::force_unstake(RuntimeOrigin::root(), ch),
			Error::<Test>::NoStakeInCommunity
		);
	});
}

#[test]
fn force_unstake_requires_root() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 10 * UNIT,
		));
		assert_noop!(
			AdsGroupRobot::force_unstake(RuntimeOrigin::signed(STAKER), ch),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ============================================================================
// Slash history tracking
// ============================================================================

#[test]
fn slash_increments_community_slash_count() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 100 * UNIT,
		));
		assert_eq!(CommunitySlashCount::<Test>::get(&ch), 0);

		assert_ok!(AdsGroupRobot::slash_community(RuntimeOrigin::root(), ch));
		assert_eq!(CommunitySlashCount::<Test>::get(&ch), 1);

		assert_ok!(AdsGroupRobot::slash_community(RuntimeOrigin::root(), ch));
		assert_eq!(CommunitySlashCount::<Test>::get(&ch), 2);
	});
}

// ============================================================================
// Staker reward distribution integration
// ============================================================================

#[test]
fn distribute_allocates_staker_rewards() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER), ch, 80 * UNIT,
		));
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(STAKER2), ch, 20 * UNIT,
		));

		let result = <Pallet<Test> as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&ch, 100 * UNIT, &BOT_OWNER,
		);

		assert_eq!(result.unwrap().placement_share, 72 * UNIT);

		let s1_claimable = StakerClaimable::<Test>::get(&ch, STAKER);
		assert_eq!(s1_claimable, 8 * UNIT * 80 / 100);

		let s2_claimable = StakerClaimable::<Test>::get(&ch, STAKER2);
		assert_eq!(s2_claimable, 8 * UNIT * 20 / 100);
	});
}

// ============================================================================
// report_node_audience emits event
// ============================================================================

#[test]
fn report_node_audience_emits_event() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		let nid = node_id(NODE_OPERATOR as u8, false);
		System::reset_events();

		assert_ok!(AdsGroupRobot::report_node_audience(
			RuntimeOrigin::signed(NODE_OPERATOR), ch, nid, 100,
		));

		System::assert_has_event(RuntimeEvent::AdsGroupRobot(
			Event::NodeAudienceReported {
				community_id_hash: ch,
				node_id: nid,
				audience_size: 100,
			}
		));
	});
}
