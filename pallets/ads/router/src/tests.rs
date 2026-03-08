use crate::{mock::*, AdsRouter};
use frame_support::assert_ok;
use pallet_ads_primitives::*;

const UNIT: u128 = 1_000_000_000_000;

// ============================================================================
// Helpers — 注册 Entity 广告位 / 设置 GroupRobot 社区
// ============================================================================

fn setup_entity_placement() -> PlacementId {
	assert_ok!(AdsEntity::register_entity_placement(
		RuntimeOrigin::signed(ALICE),
		1,
	));
	entity_placement_id(1)
}

fn setup_grouprobot_community(n: u8) -> PlacementId {
	let ch = community_placement_id(n);
	assert_ok!(AdsGroupRobot::stake_for_ads(
		RuntimeOrigin::signed(ALICE),
		ch,
		100 * UNIT,
	));
	ch
}

// ============================================================================
// 1. DeliveryVerifier 路由
// ============================================================================

#[test]
fn delivery_verifier_routes_to_entity_for_registered_placement() {
	new_test_ext().execute_with(|| {
		let pid = setup_entity_placement();

		let result =
			<AdsRouter<Test> as DeliveryVerifier<u64>>::verify_and_cap_audience(
				&ALICE, &pid, 500, None,
			);
		assert!(result.is_ok());
		let effective = result.unwrap();
		assert!(effective <= 1000, "Entity cap is 1000, got {effective}");
		assert_eq!(effective, 500);
	});
}

#[test]
fn delivery_verifier_routes_to_grouprobot_for_unregistered_placement() {
	new_test_ext().execute_with(|| {
		let pid = setup_grouprobot_community(1);

		let result =
			<AdsRouter<Test> as DeliveryVerifier<u64>>::verify_and_cap_audience(
				&TEE_OPERATOR, &pid, 200, None,
			);
		assert!(result.is_ok());
		let effective = result.unwrap();
		assert!(effective <= 200);
	});
}

#[test]
fn delivery_verifier_entity_rejects_unauthorized_caller() {
	new_test_ext().execute_with(|| {
		let pid = setup_entity_placement();

		let result =
			<AdsRouter<Test> as DeliveryVerifier<u64>>::verify_and_cap_audience(
				&CHARLIE, &pid, 100, None,
			);
		assert!(result.is_err());
	});
}

#[test]
fn delivery_verifier_grouprobot_rejects_non_tee_caller() {
	new_test_ext().execute_with(|| {
		let pid = setup_grouprobot_community(1);

		let result =
			<AdsRouter<Test> as DeliveryVerifier<u64>>::verify_and_cap_audience(
				&ALICE, &pid, 100, None,
			);
		assert!(result.is_err());
	});
}

// ============================================================================
// 2. ClickVerifier 路由
// ============================================================================

#[test]
fn click_verifier_routes_to_entity_for_registered_placement() {
	new_test_ext().execute_with(|| {
		let pid = setup_entity_placement();

		let result =
			<AdsRouter<Test> as ClickVerifier<u64>>::verify_and_cap_clicks(
				&ALICE, &pid, 50, 50,
			);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), 50);
	});
}

#[test]
fn click_verifier_rejects_grouprobot_placement_with_structured_error() {
	new_test_ext().execute_with(|| {
		let pid = setup_grouprobot_community(1);

		let result =
			<AdsRouter<Test> as ClickVerifier<u64>>::verify_and_cap_clicks(
				&TEE_OPERATOR, &pid, 10, 10,
			);
		assert!(result.is_err());
		assert_eq!(
			result.unwrap_err(),
			AdsRouterError::CpcNotSupportedForPath.into(),
		);
	});
}

#[test]
fn click_verifier_rejects_unknown_placement_as_grouprobot_path() {
	new_test_ext().execute_with(|| {
		let pid = [0xFFu8; 32];

		let result =
			<AdsRouter<Test> as ClickVerifier<u64>>::verify_and_cap_clicks(
				&ALICE, &pid, 10, 10,
			);
		assert!(result.is_err());
		assert_eq!(
			result.unwrap_err(),
			AdsRouterError::CpcNotSupportedForPath.into(),
		);
	});
}

// ============================================================================
// 3. PlacementAdminProvider 路由
// ============================================================================

#[test]
fn placement_admin_routes_to_entity_returns_owner() {
	new_test_ext().execute_with(|| {
		let pid = setup_entity_placement();

		let admin =
			<AdsRouter<Test> as PlacementAdminProvider<u64>>::placement_admin(&pid);
		assert_eq!(admin, Some(ALICE));
	});
}

#[test]
fn placement_admin_routes_to_grouprobot_returns_community_admin() {
	new_test_ext().execute_with(|| {
		let pid = setup_grouprobot_community(1);

		let admin =
			<AdsRouter<Test> as PlacementAdminProvider<u64>>::placement_admin(&pid);
		assert_eq!(admin, Some(ALICE));
	});
}

#[test]
fn placement_status_entity_active() {
	new_test_ext().execute_with(|| {
		let pid = setup_entity_placement();

		let status =
			<AdsRouter<Test> as PlacementAdminProvider<u64>>::placement_status(&pid);
		assert_eq!(status, PlacementStatus::Active);
	});
}

#[test]
fn placement_status_grouprobot_active() {
	new_test_ext().execute_with(|| {
		let pid = setup_grouprobot_community(1);

		let status =
			<AdsRouter<Test> as PlacementAdminProvider<u64>>::placement_status(&pid);
		assert_eq!(status, PlacementStatus::Active);
	});
}

#[test]
fn placement_banned_entity_false_by_default() {
	new_test_ext().execute_with(|| {
		let pid = setup_entity_placement();

		let banned =
			<AdsRouter<Test> as PlacementAdminProvider<u64>>::is_placement_banned(&pid);
		assert!(!banned);
	});
}

#[test]
fn placement_banned_grouprobot_false_by_default() {
	new_test_ext().execute_with(|| {
		let pid = setup_grouprobot_community(1);

		let banned =
			<AdsRouter<Test> as PlacementAdminProvider<u64>>::is_placement_banned(&pid);
		assert!(!banned);
	});
}

#[test]
fn placement_admin_unknown_returns_none_for_grouprobot_path() {
	new_test_ext().execute_with(|| {
		let pid = [0u8; 32];

		let admin =
			<AdsRouter<Test> as PlacementAdminProvider<u64>>::placement_admin(&pid);
		assert_eq!(admin, None);
	});
}

// ============================================================================
// 4. RevenueDistributor 路由
// ============================================================================

#[test]
fn revenue_distributor_entity_path_two_way_split() {
	new_test_ext().execute_with(|| {
		let pid = setup_entity_placement();

		let result =
			<AdsRouter<Test> as RevenueDistributor<u64, u128>>::distribute(
				&pid, 10_000, &BOB,
			);
		assert!(result.is_ok());
		let breakdown = result.unwrap();

		// Entity: PlatformAdShareBps = 2000 (20%), Entity gets 80%
		assert_eq!(breakdown.placement_share, 8_000);
		assert_eq!(breakdown.node_share, 0);
		assert_eq!(breakdown.platform_share, 2_000);
		assert_eq!(
			breakdown.placement_share + breakdown.node_share + breakdown.platform_share,
			10_000,
		);
	});
}

#[test]
fn revenue_distributor_grouprobot_path_three_way_split() {
	new_test_ext().execute_with(|| {
		let pid = setup_grouprobot_community(1);

		let result =
			<AdsRouter<Test> as RevenueDistributor<u64, u128>>::distribute(
				&pid, 10_000, &BOB,
			);
		assert!(result.is_ok());
		let breakdown = result.unwrap();

		// GroupRobot defaults: community_pct=80, tee_pct=15, staker_pct=10
		// community_share = 80% of 10000 = 8000
		// staker_pool = 10% of 8000 = 800
		// net placement_share = 8000 - 800 = 7200
		// node_share = 15% of 10000 = 1500
		// platform_share = 10000 - 8000 - 1500 = 500
		assert_eq!(breakdown.placement_share, 7_200);
		assert_eq!(breakdown.node_share, 1_500);
		assert_eq!(breakdown.platform_share, 500);
	});
}

#[test]
fn revenue_distributor_entity_zero_cost() {
	new_test_ext().execute_with(|| {
		let pid = setup_entity_placement();

		let result =
			<AdsRouter<Test> as RevenueDistributor<u64, u128>>::distribute(
				&pid, 0, &BOB,
			);
		assert!(result.is_ok());
		let breakdown = result.unwrap();
		assert_eq!(breakdown.placement_share, 0);
		assert_eq!(breakdown.node_share, 0);
		assert_eq!(breakdown.platform_share, 0);
	});
}

// ============================================================================
// 5. Routing boundary — same ID registered then deregistered
// ============================================================================

#[test]
fn deregistered_entity_placement_falls_back_to_grouprobot() {
	new_test_ext().execute_with(|| {
		let pid = setup_entity_placement();

		// Verify it routes to Entity first — placement_status is Active
		let status =
			<AdsRouter<Test> as PlacementAdminProvider<u64>>::placement_status(&pid);
		assert_eq!(status, PlacementStatus::Active);

		// Deregister
		let balance_before = Balances::free_balance(ALICE);
		assert_ok!(AdsEntity::deregister_placement(
			RuntimeOrigin::signed(ALICE),
			pid,
		));
		assert!(Balances::free_balance(ALICE) > balance_before);

		// Now the same PlacementId is no longer in Entity's RegisteredPlacements,
		// so it falls back to GroupRobot path
		assert!(!pallet_ads_entity::RegisteredPlacements::<Test>::contains_key(&pid));

		// GroupRobot path: no community staked for this hash, no CommunityAdmin set.
		// placement_status should NOT be Entity Active anymore.
		// GroupRobot's placement_status checks GlobalAdsPaused, BotAdsDisabled,
		// then falls back to checking if placement_admin exists (via BotRegistry).
		// Since this is a blake2 hash with non-zero first byte, MockBotRegistry
		// returns Some(ALICE) as bot_owner, so status = Active via GroupRobot path.
		// The key verification: the routing DID change — ClickVerifier now rejects CPC.
		let click_result =
			<AdsRouter<Test> as ClickVerifier<u64>>::verify_and_cap_clicks(
				&ALICE, &pid, 10, 10,
			);
		assert_eq!(
			click_result.unwrap_err(),
			AdsRouterError::CpcNotSupportedForPath.into(),
		);
	});
}
