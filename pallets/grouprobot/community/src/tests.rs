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
			NodeRequirement::Any,
		));
		assert_eq!(
			CommunityNodeRequirement::<Test>::get(community_hash(1)),
			NodeRequirement::Any
		);
	});
}

#[test]
fn set_node_requirement_fails_same() {
	new_test_ext().execute_with(|| {
		// Default is TeeOnly
		assert_noop!(
			GroupRobotCommunity::set_node_requirement(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				NodeRequirement::TeeOnly,
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
			false, // ads_enabled
			*b"en", // language
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
			0, true, 20, 5, WarnAction::Ban, true, false, *b"en",
		));

		// Try with wrong version
		assert_noop!(
			GroupRobotCommunity::update_community_config(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				0, // should be 1
				false, 10, 3, WarnAction::Kick, false, false, *b"en",
			),
			Error::<Test>::ConfigVersionConflict
		);

		// Correct version
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			1, false, 10, 3, WarnAction::Kick, false, false, *b"en",
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
		// Basic tier: log_retention_days=30, min_retention_blocks=432000
		// Submit at block 1
		assert_ok!(GroupRobotCommunity::submit_action_log(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			ActionType::Ban, [1u8; 32], 1, [2u8; 32], [3u8; 64],
		));

		// Advance well past retention period
		System::set_block_number(500_000);

		// Submit another at block 500000
		assert_ok!(GroupRobotCommunity::submit_action_log(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			ActionType::Kick, [1u8; 32], 2, [2u8; 32], [3u8; 64],
		));

		// Clear logs older than 432000 blocks (meets Basic tier retention)
		assert_ok!(GroupRobotCommunity::clear_expired_logs(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			432_000, // max_age_blocks >= 30 days * 14400
		));

		// Only the second log should remain (first log is 499999 blocks old > 432000)
		assert_eq!(ActionLogs::<Test>::get(community_hash(1)).len(), 1);
		assert_eq!(LogCount::<Test>::get(), 1);
	});
}

#[test]
fn clear_expired_logs_fails_none() {
	new_test_ext().execute_with(|| {
		// Use max_age >= retention period so we pass tier gate, but no logs exist
		assert_noop!(
			GroupRobotCommunity::clear_expired_logs(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				432_000, // meets Basic tier retention
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
		assert_eq!(GroupRobotCommunity::get_node_requirement(&community_hash(1)), NodeRequirement::TeeOnly);
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
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		assert!(<GroupRobotCommunity as CommunityProvider<u64>>::is_community_bound(&community_hash(1)));
	});
}

// ============================================================================
// Reputation System
// ============================================================================

// ============================================================================
// Ads fields
// ============================================================================

#[test]
fn update_community_config_ads_enabled() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, true, *b"zh",
		));
		let config = CommunityConfigs::<Test>::get(community_hash(1)).unwrap();
		assert!(config.ads_enabled);
		assert_eq!(config.language, *b"zh");
		assert_eq!(config.active_members, 0); // default, not set by config
	});
}

#[test]
fn update_active_members_works() {
	new_test_ext().execute_with(|| {
		// Must create config first
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, true, *b"en",
		));

		// Update active_members
		assert_ok!(GroupRobotCommunity::update_active_members(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			150,
		));
		let config = CommunityConfigs::<Test>::get(community_hash(1)).unwrap();
		assert_eq!(config.active_members, 150);
	});
}

#[test]
fn update_active_members_fails_no_config() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::update_active_members(
				RuntimeOrigin::signed(OWNER),
				community_hash(99),
				100,
			),
			Error::<Test>::CommunityNotFound
		);
	});
}

#[test]
fn update_config_preserves_active_members() {
	new_test_ext().execute_with(|| {
		// Create config
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, true, *b"en",
		));

		// Bot sets active_members = 200
		assert_ok!(GroupRobotCommunity::update_active_members(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			200,
		));

		// Admin updates config (should NOT reset active_members)
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			1, false, 20, 5, WarnAction::Ban, true, false, *b"zh",
		));
		let config = CommunityConfigs::<Test>::get(community_hash(1)).unwrap();
		assert_eq!(config.active_members, 200); // preserved!
		assert!(!config.ads_enabled); // updated
		assert_eq!(config.language, *b"zh"); // updated
	});
}

// ============================================================================
// Reputation System
// ============================================================================

fn user_hash(n: u8) -> [u8; 32] {
	let mut h = [0u8; 32];
	h[0] = n;
	h
}

#[test]
fn award_reputation_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			user_hash(1),
			10,
		));
		let rec = MemberReputation::<Test>::get(community_hash(1), user_hash(1));
		assert_eq!(rec.score, 10);
		assert_eq!(rec.awards, 1);
		assert_eq!(rec.deductions, 0);
		assert_eq!(GlobalReputation::<Test>::get(user_hash(1)), 10);
	});
}

#[test]
fn deduct_reputation_works() {
	new_test_ext().execute_with(|| {
		// Award first
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 20,
		));

		// Advance past cooldown
		System::set_block_number(10);

		assert_ok!(GroupRobotCommunity::deduct_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 5,
		));
		let rec = MemberReputation::<Test>::get(community_hash(1), user_hash(1));
		assert_eq!(rec.score, 15);
		assert_eq!(rec.awards, 1);
		assert_eq!(rec.deductions, 1);
		assert_eq!(GlobalReputation::<Test>::get(user_hash(1)), 15);
	});
}

#[test]
fn deduct_can_go_negative() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::deduct_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 10,
		));
		let rec = MemberReputation::<Test>::get(community_hash(1), user_hash(1));
		assert_eq!(rec.score, -10);
		assert_eq!(GlobalReputation::<Test>::get(user_hash(1)), -10);
	});
}

#[test]
fn reset_reputation_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 50,
		));
		assert_eq!(GlobalReputation::<Test>::get(user_hash(1)), 50);

		assert_ok!(GroupRobotCommunity::reset_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1),
		));
		let rec = MemberReputation::<Test>::get(community_hash(1), user_hash(1));
		assert_eq!(rec.score, 0);
		assert_eq!(GlobalReputation::<Test>::get(user_hash(1)), 0);
	});
}

#[test]
fn cooldown_enforced() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 5,
		));

		// Same operator, same target, within cooldown → fail
		assert_noop!(
			GroupRobotCommunity::award_reputation(
				RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 5,
			),
			Error::<Test>::ReputationOnCooldown
		);

		// Advance past cooldown (5 blocks)
		System::set_block_number(7);

		// Now should work
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 5,
		));
		assert_eq!(MemberReputation::<Test>::get(community_hash(1), user_hash(1)).score, 10);
	});
}

#[test]
fn cooldown_different_operator_independent() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 5,
		));

		// Different operator, same target → no cooldown
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OTHER), community_hash(1), user_hash(1), 5,
		));
		assert_eq!(MemberReputation::<Test>::get(community_hash(1), user_hash(1)).score, 10);
	});
}

#[test]
fn cooldown_different_target_independent() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 5,
		));

		// Same operator, different target → no cooldown
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(2), 5,
		));
	});
}

#[test]
fn delta_too_large_rejected() {
	new_test_ext().execute_with(|| {
		// MaxReputationDelta = 100
		assert_noop!(
			GroupRobotCommunity::award_reputation(
				RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 101,
			),
			Error::<Test>::ReputationDeltaTooLarge
		);
	});
}

#[test]
fn delta_zero_rejected() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::award_reputation(
				RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 0,
			),
			Error::<Test>::ReputationDeltaZero
		);
		assert_noop!(
			GroupRobotCommunity::deduct_reputation(
				RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 0,
			),
			Error::<Test>::ReputationDeltaZero
		);
	});
}

#[test]
fn global_reputation_aggregates_communities() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 10,
		));
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(2), user_hash(1), 20,
		));

		assert_eq!(MemberReputation::<Test>::get(community_hash(1), user_hash(1)).score, 10);
		assert_eq!(MemberReputation::<Test>::get(community_hash(2), user_hash(1)).score, 20);
		assert_eq!(GlobalReputation::<Test>::get(user_hash(1)), 30);
	});
}

#[test]
fn reputation_provider_trait() {
	new_test_ext().execute_with(|| {
		use pallet_grouprobot_primitives::ReputationProvider;

		assert_eq!(<GroupRobotCommunity as ReputationProvider>::get_reputation(&community_hash(1), &user_hash(1)), 0);
		assert_eq!(<GroupRobotCommunity as ReputationProvider>::get_global_reputation(&user_hash(1)), 0);

		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 15,
		));

		assert_eq!(<GroupRobotCommunity as ReputationProvider>::get_reputation(&community_hash(1), &user_hash(1)), 15);
		assert_eq!(<GroupRobotCommunity as ReputationProvider>::get_global_reputation(&user_hash(1)), 15);
	});
}

#[test]
fn reset_reputation_adjusts_global() {
	new_test_ext().execute_with(|| {
		// Award in two communities
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 10,
		));
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(2), user_hash(1), 20,
		));
		assert_eq!(GlobalReputation::<Test>::get(user_hash(1)), 30);

		// Reset community 1
		assert_ok!(GroupRobotCommunity::reset_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1),
		));

		// Global should now be 20 (only community 2 remains)
		assert_eq!(GlobalReputation::<Test>::get(user_hash(1)), 20);
		assert_eq!(MemberReputation::<Test>::get(community_hash(1), user_hash(1)).score, 0);
	});
}

// ============================================================================
// Tier gate
// ============================================================================

#[test]
fn submit_action_log_fails_free_tier() {
	new_test_ext().execute_with(|| {
		// community_hash(2) → Free tier in MockSubscription
		assert_noop!(
			GroupRobotCommunity::submit_action_log(
				RuntimeOrigin::signed(OTHER),
				community_hash(2),
				ActionType::Ban,
				[1u8; 32], 1, [2u8; 32], [3u8; 64],
			),
			Error::<Test>::FreeTierNotAllowed
		);
	});
}

#[test]
fn batch_submit_logs_fails_free_tier() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::batch_submit_logs(
				RuntimeOrigin::signed(OTHER),
				community_hash(2),
				vec![(ActionType::Ban, [1u8; 32], 1, [2u8; 32], [3u8; 64])],
			),
			Error::<Test>::FreeTierNotAllowed
		);
	});
}

#[test]
fn clear_expired_logs_enforces_retention_period() {
	new_test_ext().execute_with(|| {
		// community_hash(1) → Basic tier (log_retention_days=30, min blocks=30*14400=432000)
		// Submit a log first
		assert_ok!(GroupRobotCommunity::submit_action_log(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			ActionType::Ban,
			[1u8; 32], 1, [2u8; 32], [3u8; 64],
		));

		// Try to clear with max_age_blocks < 432000 → RetentionPeriodNotExpired
		assert_noop!(
			GroupRobotCommunity::clear_expired_logs(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				100, // too small
			),
			Error::<Test>::RetentionPeriodNotExpired
		);
	});
}
