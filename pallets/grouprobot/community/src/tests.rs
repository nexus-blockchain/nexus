use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use pallet_grouprobot_primitives::*;

// ============================================================================
// submit_action_log
// ============================================================================

#[test]
fn submit_action_log_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::submit_action_log(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			ActionType::Ban,
			[1u8; 32],
			42,
			[2u8; 32],
			[3u8; 64],
		));
		assert_eq!(ActionLogs::<Test>::get(community_hash(1)).len(), 1);
		assert_eq!(LogCount::<Test>::get(), 1);

		let log = &ActionLogs::<Test>::get(community_hash(1))[0];
		assert_eq!(log.operator, OWNER);
		assert_eq!(log.sequence, 42);
		assert_eq!(log.action_type, ActionType::Ban);
	});
}

#[test]
fn submit_action_log_fails_full() {
	new_test_ext().execute_with(|| {
		// MaxLogsPerCommunity = 10
		for i in 0..10u64 {
			assert_ok!(GroupRobotCommunity::submit_action_log(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				ActionType::Kick,
				[1u8; 32],
				i,
				[2u8; 32],
				[3u8; 64],
			));
		}
		assert_noop!(
			GroupRobotCommunity::submit_action_log(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				ActionType::Kick,
				[1u8; 32],
				99,
				[2u8; 32],
				[3u8; 64],
			),
			Error::<Test>::LogsFull
		);
	});
}

// ============================================================================
// set_node_requirement
// ============================================================================

#[test]
fn set_node_requirement_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::set_node_requirement(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			NodeRequirement::TeeOnly,
		));
		assert_eq!(
			CommunityNodeRequirement::<Test>::get(community_hash(1)),
			NodeRequirement::TeeOnly
		);
	});
}

#[test]
fn set_node_requirement_fails_same() {
	new_test_ext().execute_with(|| {
		// Default is Any
		assert_noop!(
			GroupRobotCommunity::set_node_requirement(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				NodeRequirement::Any,
			),
			Error::<Test>::SameNodeRequirement
		);
	});
}

// ============================================================================
// update_community_config
// ============================================================================

#[test]
fn update_community_config_works() {
	new_test_ext().execute_with(|| {
		// First creation (expected_version=0)
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0,     // expected_version
			true,  // anti_flood_enabled
			20,    // flood_limit
			5,     // warn_limit
			WarnAction::Ban,
			true,  // welcome_enabled
		));
		let config = CommunityConfigs::<Test>::get(community_hash(1)).unwrap();
		assert_eq!(config.version, 1);
		assert!(config.anti_flood_enabled);
		assert_eq!(config.flood_limit, 20);
		assert_eq!(config.warn_limit, 5);
		assert_eq!(config.warn_action, WarnAction::Ban);
		assert!(config.welcome_enabled);
	});
}

#[test]
fn update_community_config_cas_conflict() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 20, 5, WarnAction::Ban, true,
		));

		// Try with wrong version
		assert_noop!(
			GroupRobotCommunity::update_community_config(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				0, // should be 1
				false, 10, 3, WarnAction::Kick, false,
			),
			Error::<Test>::ConfigVersionConflict
		);

		// Correct version
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			1, false, 10, 3, WarnAction::Kick, false,
		));
		assert_eq!(CommunityConfigs::<Test>::get(community_hash(1)).unwrap().version, 2);
	});
}

// ============================================================================
// batch_submit_logs
// ============================================================================

#[test]
fn batch_submit_logs_works() {
	new_test_ext().execute_with(|| {
		let logs = vec![
			(ActionType::Ban, [1u8; 32], 1u64, [2u8; 32], [3u8; 64]),
			(ActionType::Kick, [4u8; 32], 2u64, [5u8; 32], [6u8; 64]),
		];
		assert_ok!(GroupRobotCommunity::batch_submit_logs(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			logs,
		));
		assert_eq!(ActionLogs::<Test>::get(community_hash(1)).len(), 2);
		assert_eq!(LogCount::<Test>::get(), 2);
	});
}

#[test]
fn batch_submit_logs_fails_empty() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::batch_submit_logs(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				vec![],
			),
			Error::<Test>::EmptyBatch
		);
	});
}

// ============================================================================
// clear_expired_logs
// ============================================================================

#[test]
fn clear_expired_logs_works() {
	new_test_ext().execute_with(|| {
		// Submit at block 1
		assert_ok!(GroupRobotCommunity::submit_action_log(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			ActionType::Ban, [1u8; 32], 1, [2u8; 32], [3u8; 64],
		));

		// Advance to block 100
		System::set_block_number(100);

		// Submit another at block 100
		assert_ok!(GroupRobotCommunity::submit_action_log(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			ActionType::Kick, [1u8; 32], 2, [2u8; 32], [3u8; 64],
		));

		// Clear logs older than 50 blocks
		assert_ok!(GroupRobotCommunity::clear_expired_logs(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			50, // max_age_blocks
		));

		// Only the second log should remain
		assert_eq!(ActionLogs::<Test>::get(community_hash(1)).len(), 1);
		assert_eq!(LogCount::<Test>::get(), 1);
	});
}

#[test]
fn clear_expired_logs_fails_none() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::clear_expired_logs(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				100,
			),
			Error::<Test>::NoLogsToClear
		);
	});
}

// ============================================================================
// Helpers / CommunityProvider
// ============================================================================

#[test]
fn helper_get_node_requirement() {
	new_test_ext().execute_with(|| {
		assert_eq!(GroupRobotCommunity::get_node_requirement(&community_hash(1)), NodeRequirement::Any);
		assert_ok!(GroupRobotCommunity::set_node_requirement(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			NodeRequirement::TeePreferred,
		));
		assert_eq!(
			GroupRobotCommunity::get_node_requirement(&community_hash(1)),
			NodeRequirement::TeePreferred
		);
	});
}

#[test]
fn community_provider_trait() {
	new_test_ext().execute_with(|| {
		use pallet_grouprobot_primitives::CommunityProvider;
		assert!(!<GroupRobotCommunity as CommunityProvider<u64>>::is_community_bound(&community_hash(1)));

		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false,
		));
		assert!(<GroupRobotCommunity as CommunityProvider<u64>>::is_community_bound(&community_hash(1)));
	});
}
