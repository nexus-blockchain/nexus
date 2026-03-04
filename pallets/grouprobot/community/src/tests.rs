use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use pallet_grouprobot_primitives::*;

// ============================================================================
// submit_action_log
// ============================================================================

#[test]
fn submit_action_log_works() {
	new_test_ext().execute_with(|| {
		let sig = test_sign(&community_hash(1), &ActionType::Ban, &[1u8; 32], 42, &[2u8; 32]);
		assert_ok!(GroupRobotCommunity::submit_action_log(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			ActionType::Ban,
			[1u8; 32],
			42,
			[2u8; 32],
			sig,
		));
		assert_eq!(ActionLogs::<Test>::get(community_hash(1)).len(), 1);

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
			let sig = test_sign(&community_hash(1), &ActionType::Kick, &[1u8; 32], i, &[2u8; 32]);
			assert_ok!(GroupRobotCommunity::submit_action_log(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				ActionType::Kick,
				[1u8; 32],
				i,
				[2u8; 32],
				sig,
			));
		}
		let sig = test_sign(&community_hash(1), &ActionType::Kick, &[1u8; 32], 99, &[2u8; 32]);
		assert_noop!(
			GroupRobotCommunity::submit_action_log(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				ActionType::Kick,
				[1u8; 32],
				99,
				[2u8; 32],
				sig,
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
		// 需要先创建 config, set_node_requirement 现在读写 CommunityConfigs
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		assert_ok!(GroupRobotCommunity::set_node_requirement(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			NodeRequirement::Any,
		));
		let config = CommunityConfigs::<Test>::get(community_hash(1)).unwrap();
		assert_eq!(config.node_requirement, NodeRequirement::Any);
	});
}

#[test]
fn set_node_requirement_fails_same() {
	new_test_ext().execute_with(|| {
		// 创建 config (default node_requirement = TeeOnly)
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
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
		let sig1 = test_sign(&community_hash(1), &ActionType::Ban, &[1u8; 32], 1, &[2u8; 32]);
		let sig2 = test_sign(&community_hash(1), &ActionType::Kick, &[4u8; 32], 2, &[5u8; 32]);
		let logs = vec![
			(ActionType::Ban, [1u8; 32], 1u64, [2u8; 32], sig1),
			(ActionType::Kick, [4u8; 32], 2u64, [5u8; 32], sig2),
		];
		assert_ok!(GroupRobotCommunity::batch_submit_logs(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			logs,
		));
		assert_eq!(ActionLogs::<Test>::get(community_hash(1)).len(), 2);
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
		let sig1 = test_sign(&community_hash(1), &ActionType::Ban, &[1u8; 32], 1, &[2u8; 32]);
		assert_ok!(GroupRobotCommunity::submit_action_log(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			ActionType::Ban, [1u8; 32], 1, [2u8; 32], sig1,
		));

		// Advance well past retention period
		System::set_block_number(500_000);

		// Submit another at block 500000
		let sig2 = test_sign(&community_hash(1), &ActionType::Kick, &[1u8; 32], 2, &[2u8; 32]);
		assert_ok!(GroupRobotCommunity::submit_action_log(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			ActionType::Kick, [1u8; 32], 2, [2u8; 32], sig2,
		));

		// Clear logs older than 432000 blocks (meets Basic tier retention)
		assert_ok!(GroupRobotCommunity::clear_expired_logs(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			432_000, // max_age_blocks >= 30 days * 14400
		));

		// Only the second log should remain (first log is 499999 blocks old > 432000)
		assert_eq!(ActionLogs::<Test>::get(community_hash(1)).len(), 1);
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
// Helper Functions
// ============================================================================

#[test]
fn helper_get_node_requirement() {
	new_test_ext().execute_with(|| {
		assert_eq!(GroupRobotCommunity::get_node_requirement(&community_hash(1)), NodeRequirement::TeeOnly);
		// Need config first since set_node_requirement now uses CommunityConfigs
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
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
fn helper_is_community_configured() {
	new_test_ext().execute_with(|| {
		assert!(!GroupRobotCommunity::is_community_configured(&community_hash(1)));

		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		assert!(GroupRobotCommunity::is_community_configured(&community_hash(1)));
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
		// community_hash(1) has valid bot_owner but no config created yet
		assert_noop!(
			GroupRobotCommunity::update_active_members(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
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
	});
}

#[test]
fn reset_reputation_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 50,
		));

		assert_ok!(GroupRobotCommunity::reset_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1),
		));
		let rec = MemberReputation::<Test>::get(community_hash(1), user_hash(1));
		assert_eq!(rec.score, 0);
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
fn cooldown_different_target_same_block() {
	new_test_ext().execute_with(|| {
		// Same operator, two different targets in same block → both succeed (no cooldown)
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 5,
		));
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(2), 5,
		));
		assert_eq!(MemberReputation::<Test>::get(community_hash(1), user_hash(1)).score, 5);
		assert_eq!(MemberReputation::<Test>::get(community_hash(1), user_hash(2)).score, 5);
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
		// community_hash(1) and community_hash(2) both have OWNER as bot_owner
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 10,
		));
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(2), user_hash(1), 20,
		));

		assert_eq!(MemberReputation::<Test>::get(community_hash(1), user_hash(1)).score, 10);
		assert_eq!(MemberReputation::<Test>::get(community_hash(2), user_hash(1)).score, 20);
	});
}

#[test]
fn reputation_storage_reads() {
	new_test_ext().execute_with(|| {
		assert_eq!(MemberReputation::<Test>::get(community_hash(1), user_hash(1)).score, 0);

		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 15,
		));

		assert_eq!(MemberReputation::<Test>::get(community_hash(1), user_hash(1)).score, 15);
	});
}

#[test]
fn reset_reputation_cross_community() {
	new_test_ext().execute_with(|| {
		// Award in two communities (both have OWNER as bot_owner)
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 10,
		));
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(2), user_hash(1), 20,
		));

		// Reset community 1
		assert_ok!(GroupRobotCommunity::reset_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1),
		));

		// Community 1 reset, community 2 unaffected
		assert_eq!(MemberReputation::<Test>::get(community_hash(1), user_hash(1)).score, 0);
		assert_eq!(MemberReputation::<Test>::get(community_hash(2), user_hash(1)).score, 20);
	});
}

// ============================================================================
// Tier gate
// ============================================================================

#[test]
fn submit_action_log_fails_free_tier() {
	new_test_ext().execute_with(|| {
		// community_hash(4) → active bot + Free tier
		let sig = test_sign(&community_hash(4), &ActionType::Ban, &[1u8; 32], 1, &[2u8; 32]);
		assert_noop!(
			GroupRobotCommunity::submit_action_log(
				RuntimeOrigin::signed(OWNER),
				community_hash(4),
				ActionType::Ban,
				[1u8; 32], 1, [2u8; 32], sig,
			),
			Error::<Test>::FreeTierNotAllowed
		);
	});
}

#[test]
fn batch_submit_logs_fails_free_tier() {
	new_test_ext().execute_with(|| {
		// community_hash(4) → active bot + Free tier
		let sig = test_sign(&community_hash(4), &ActionType::Ban, &[1u8; 32], 1, &[2u8; 32]);
		assert_noop!(
			GroupRobotCommunity::batch_submit_logs(
				RuntimeOrigin::signed(OWNER),
				community_hash(4),
				vec![(ActionType::Ban, [1u8; 32], 1, [2u8; 32], sig)],
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
		let sig = test_sign(&community_hash(1), &ActionType::Ban, &[1u8; 32], 1, &[2u8; 32]);
		assert_ok!(GroupRobotCommunity::submit_action_log(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			ActionType::Ban,
			[1u8; 32], 1, [2u8; 32], sig,
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

// ============================================================================
// H1-H3: Authorization checks (ensure_bot_owner)
// ============================================================================

#[test]
fn h1_set_node_requirement_rejects_non_owner() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::set_node_requirement(
				RuntimeOrigin::signed(OTHER),
				community_hash(1),
				NodeRequirement::Any,
			),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn h2_update_community_config_rejects_non_owner() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::update_community_config(
				RuntimeOrigin::signed(OTHER),
				community_hash(1),
				0,
				true, 10, 3, WarnAction::Kick,
				false, false, *b"en",
			),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn h3_update_active_members_rejects_non_owner() {
	new_test_ext().execute_with(|| {
		// First create config so we can test auth, not CommunityNotFound
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0,
			true, 10, 3, WarnAction::Kick,
			false, false, *b"en",
		));
		assert_noop!(
			GroupRobotCommunity::update_active_members(
				RuntimeOrigin::signed(OTHER),
				community_hash(1),
				100,
			),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn h4_submit_action_log_rejects_invalid_signature() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::submit_action_log(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				ActionType::Ban,
				[1u8; 32], 42, [2u8; 32],
				[0u8; 64], // invalid signature
			),
			Error::<Test>::InvalidSignature
		);
	});
}

#[test]
fn h5_reputation_rejects_non_bot_owner() {
	new_test_ext().execute_with(|| {
		// community_hash(10) has no bot_owner → NotBotOwner
		assert_noop!(
			GroupRobotCommunity::award_reputation(
				RuntimeOrigin::signed(OWNER),
				community_hash(10),
				user_hash(1),
				5,
			),
			Error::<Test>::NotBotOwner
		);
	});
}

// ============================================================================
// M2-fix: InvalidLanguageCode
// ============================================================================

#[test]
fn m2_update_config_rejects_invalid_language_uppercase() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::update_community_config(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				0, true, 10, 3, WarnAction::Kick, false, false, *b"EN",
			),
			Error::<Test>::InvalidLanguageCode
		);
	});
}

#[test]
fn m2_update_config_rejects_invalid_language_non_alpha() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::update_community_config(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				0, true, 10, 3, WarnAction::Kick, false, false, [0xFF, 0x00],
			),
			Error::<Test>::InvalidLanguageCode
		);
	});
}

#[test]
fn m2_update_config_accepts_valid_language() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"zh",
		));
		let config = CommunityConfigs::<Test>::get(community_hash(1)).unwrap();
		assert_eq!(config.language, *b"zh");
	});
}

// ============================================================================
// M3-fix: submit_action_log / batch_submit_logs require bot owner
// ============================================================================

#[test]
fn m3_submit_action_log_rejects_non_owner() {
	new_test_ext().execute_with(|| {
		let sig = test_sign(&community_hash(1), &ActionType::Ban, &[1u8; 32], 1, &[2u8; 32]);
		assert_noop!(
			GroupRobotCommunity::submit_action_log(
				RuntimeOrigin::signed(OTHER),
				community_hash(1),
				ActionType::Ban,
				[1u8; 32], 1, [2u8; 32], sig,
			),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn m3_batch_submit_logs_rejects_non_owner() {
	new_test_ext().execute_with(|| {
		let sig = test_sign(&community_hash(1), &ActionType::Ban, &[1u8; 32], 1, &[2u8; 32]);
		assert_noop!(
			GroupRobotCommunity::batch_submit_logs(
				RuntimeOrigin::signed(OTHER),
				community_hash(1),
				vec![(ActionType::Ban, [1u8; 32], 1, [2u8; 32], sig)],
			),
			Error::<Test>::NotBotOwner
		);
	});
}

// ============================================================================
// M4-fix: SequenceNotMonotonic
// ============================================================================

#[test]
fn m4_submit_action_log_rejects_duplicate_sequence() {
	new_test_ext().execute_with(|| {
		let sig1 = test_sign(&community_hash(1), &ActionType::Ban, &[1u8; 32], 10, &[2u8; 32]);
		assert_ok!(GroupRobotCommunity::submit_action_log(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			ActionType::Ban, [1u8; 32], 10, [2u8; 32], sig1,
		));
		assert_eq!(LastSequence::<Test>::get(community_hash(1)), Some(10));

		// Same sequence again → SequenceNotMonotonic
		let sig2 = test_sign(&community_hash(1), &ActionType::Kick, &[3u8; 32], 10, &[4u8; 32]);
		assert_noop!(
			GroupRobotCommunity::submit_action_log(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				ActionType::Kick, [3u8; 32], 10, [4u8; 32], sig2,
			),
			Error::<Test>::SequenceNotMonotonic
		);
	});
}

#[test]
fn m4_submit_action_log_rejects_lower_sequence() {
	new_test_ext().execute_with(|| {
		let sig1 = test_sign(&community_hash(1), &ActionType::Ban, &[1u8; 32], 20, &[2u8; 32]);
		assert_ok!(GroupRobotCommunity::submit_action_log(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			ActionType::Ban, [1u8; 32], 20, [2u8; 32], sig1,
		));

		// Lower sequence → SequenceNotMonotonic
		let sig2 = test_sign(&community_hash(1), &ActionType::Kick, &[3u8; 32], 5, &[4u8; 32]);
		assert_noop!(
			GroupRobotCommunity::submit_action_log(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				ActionType::Kick, [3u8; 32], 5, [4u8; 32], sig2,
			),
			Error::<Test>::SequenceNotMonotonic
		);
	});
}

#[test]
fn m4_batch_submit_logs_rejects_non_monotonic_within_batch() {
	new_test_ext().execute_with(|| {
		let sig1 = test_sign(&community_hash(1), &ActionType::Ban, &[1u8; 32], 5, &[2u8; 32]);
		let sig2 = test_sign(&community_hash(1), &ActionType::Kick, &[3u8; 32], 3, &[4u8; 32]);
		assert_noop!(
			GroupRobotCommunity::batch_submit_logs(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				vec![
					(ActionType::Ban, [1u8; 32], 5, [2u8; 32], sig1),
					(ActionType::Kick, [3u8; 32], 3, [4u8; 32], sig2), // 3 < 5
				],
			),
			Error::<Test>::SequenceNotMonotonic
		);
	});
}

#[test]
fn m4_batch_submit_logs_rejects_stale_first_sequence() {
	new_test_ext().execute_with(|| {
		// Submit single log with seq=10
		let sig0 = test_sign(&community_hash(1), &ActionType::Ban, &[1u8; 32], 10, &[2u8; 32]);
		assert_ok!(GroupRobotCommunity::submit_action_log(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			ActionType::Ban, [1u8; 32], 10, [2u8; 32], sig0,
		));

		// Batch with first seq=5 (< 10) → SequenceNotMonotonic
		let sig1 = test_sign(&community_hash(1), &ActionType::Kick, &[3u8; 32], 5, &[4u8; 32]);
		assert_noop!(
			GroupRobotCommunity::batch_submit_logs(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				vec![(ActionType::Kick, [3u8; 32], 5, [4u8; 32], sig1)],
			),
			Error::<Test>::SequenceNotMonotonic
		);
	});
}

#[test]
fn m4_batch_submit_logs_updates_last_sequence() {
	new_test_ext().execute_with(|| {
		let sig1 = test_sign(&community_hash(1), &ActionType::Ban, &[1u8; 32], 10, &[2u8; 32]);
		let sig2 = test_sign(&community_hash(1), &ActionType::Kick, &[3u8; 32], 20, &[4u8; 32]);
		assert_ok!(GroupRobotCommunity::batch_submit_logs(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			vec![
				(ActionType::Ban, [1u8; 32], 10, [2u8; 32], sig1),
				(ActionType::Kick, [3u8; 32], 20, [4u8; 32], sig2),
			],
		));
		assert_eq!(LastSequence::<Test>::get(community_hash(1)), Some(20));
	});
}

// ============================================================================
// M1-R2: BotNotActive — inactive bot cannot perform operations
// ============================================================================

#[test]
fn m1_r2_submit_action_log_rejects_inactive_bot() {
	new_test_ext().execute_with(|| {
		// community_hash(3) → owner=OWNER, is_bot_active=false
		let sig = test_sign(&community_hash(3), &ActionType::Ban, &[1u8; 32], 1, &[2u8; 32]);
		assert_noop!(
			GroupRobotCommunity::submit_action_log(
				RuntimeOrigin::signed(OWNER),
				community_hash(3),
				ActionType::Ban, [1u8; 32], 1, [2u8; 32], sig,
			),
			Error::<Test>::BotNotActive
		);
	});
}

#[test]
fn m1_r2_batch_submit_logs_rejects_inactive_bot() {
	new_test_ext().execute_with(|| {
		let sig = test_sign(&community_hash(3), &ActionType::Ban, &[1u8; 32], 1, &[2u8; 32]);
		assert_noop!(
			GroupRobotCommunity::batch_submit_logs(
				RuntimeOrigin::signed(OWNER),
				community_hash(3),
				vec![(ActionType::Ban, [1u8; 32], 1, [2u8; 32], sig)],
			),
			Error::<Test>::BotNotActive
		);
	});
}

#[test]
fn m1_r2_award_reputation_rejects_inactive_bot() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::award_reputation(
				RuntimeOrigin::signed(OWNER), community_hash(3), user_hash(1), 10,
			),
			Error::<Test>::BotNotActive
		);
	});
}

#[test]
fn m1_r2_deduct_reputation_rejects_inactive_bot() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::deduct_reputation(
				RuntimeOrigin::signed(OWNER), community_hash(3), user_hash(1), 10,
			),
			Error::<Test>::BotNotActive
		);
	});
}

#[test]
fn m1_r2_reset_reputation_rejects_inactive_bot() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::reset_reputation(
				RuntimeOrigin::signed(OWNER), community_hash(3), user_hash(1),
			),
			Error::<Test>::BotNotActive
		);
	});
}

#[test]
fn m1_r2_update_active_members_rejects_inactive_bot() {
	new_test_ext().execute_with(|| {
		// Create config for community_hash(3) first — set_node_requirement uses ensure_bot_owner (not active)
		// But update_community_config also uses ensure_bot_owner (not active), so we can create config
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(3), 0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		assert_noop!(
			GroupRobotCommunity::update_active_members(
				RuntimeOrigin::signed(OWNER), community_hash(3), 100,
			),
			Error::<Test>::BotNotActive
		);
	});
}

#[test]
fn m1_r2_config_operations_allow_inactive_bot() {
	new_test_ext().execute_with(|| {
		// update_community_config should work for inactive bots
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(3), 0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		// set_node_requirement needs config to exist first
		assert_ok!(GroupRobotCommunity::set_node_requirement(
			RuntimeOrigin::signed(OWNER), community_hash(3), NodeRequirement::Any,
		));
	});
}

// ============================================================================
// M2-R2: cleanup_expired_cooldowns
// ============================================================================

#[test]
fn m2_r2_cleanup_expired_cooldowns_works() {
	new_test_ext().execute_with(|| {
		// Create a cooldown entry by awarding reputation
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 5,
		));
		// Verify cooldown exists
		assert!(ReputationCooldowns::<Test>::get((&OWNER, &community_hash(1), &user_hash(1))) > 0);

		// Advance past cooldown (5 blocks)
		System::set_block_number(10);

		// Cleanup should succeed
		assert_ok!(GroupRobotCommunity::cleanup_expired_cooldowns(
			RuntimeOrigin::signed(OTHER), // anyone can call
			OWNER,
			community_hash(1),
			user_hash(1),
		));

		// Cooldown entry should be removed
		assert_eq!(ReputationCooldowns::<Test>::get((&OWNER, &community_hash(1), &user_hash(1))), 0);
	});
}

#[test]
fn m2_r2_cleanup_cooldowns_rejects_not_expired() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 5,
		));

		// Still at block 1, cooldown not expired
		assert_noop!(
			GroupRobotCommunity::cleanup_expired_cooldowns(
				RuntimeOrigin::signed(OTHER),
				OWNER,
				community_hash(1),
				user_hash(1),
			),
			Error::<Test>::CooldownNotExpired
		);
	});
}

#[test]
fn m2_r2_cleanup_cooldowns_rejects_nonexistent() {
	new_test_ext().execute_with(|| {
		// No cooldown entry exists for this triple
		// M1-R3: 使用专用 CooldownNotFound 错误码
		assert_noop!(
			GroupRobotCommunity::cleanup_expired_cooldowns(
				RuntimeOrigin::signed(OTHER),
				OWNER,
				community_hash(1),
				user_hash(99),
			),
			Error::<Test>::CooldownNotFound
		);
	});
}

#[test]
fn m2_r2_cleanup_cooldowns_emits_event() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 5,
		));

		System::set_block_number(10);
		System::reset_events();

		assert_ok!(GroupRobotCommunity::cleanup_expired_cooldowns(
			RuntimeOrigin::signed(OTHER),
			OWNER,
			community_hash(1),
			user_hash(1),
		));

		System::assert_last_event(RuntimeEvent::GroupRobotCommunity(
			crate::Event::CooldownCleaned {
				community_id_hash: community_hash(1),
				user_hash: user_hash(1),
				operator: OWNER,
			},
		));
	});
}

// ============================================================================
// Round 3 regression tests
// ============================================================================

#[test]
fn m1_r3_cooldown_not_found_error_is_distinct() {
	new_test_ext().execute_with(|| {
		// CooldownNotFound should be returned (not NoLogsToClear)
		let result = GroupRobotCommunity::cleanup_expired_cooldowns(
			RuntimeOrigin::signed(OTHER),
			OWNER,
			community_hash(1),
			user_hash(50),
		);
		assert_noop!(
			GroupRobotCommunity::cleanup_expired_cooldowns(
				RuntimeOrigin::signed(OTHER),
				OWNER,
				community_hash(1),
				user_hash(50),
			),
			Error::<Test>::CooldownNotFound
		);
		// Verify it's NOT NoLogsToClear
		assert_ne!(
			result,
			Err(Error::<Test>::NoLogsToClear.into()),
		);
	});
}

#[test]
fn m2_r3_batch_respects_configurable_max_size() {
	new_test_ext().execute_with(|| {
		// MaxBatchSize = 50 in mock, create 51 logs
		let action = ActionType::Kick;
		let target = [0u8; 32];
		let msg_hash = [0u8; 32];
		let logs: Vec<(ActionType, [u8; 32], u64, [u8; 32], [u8; 64])> = (1..=51u64).map(|seq| {
			let sig = test_sign(&community_hash(1), &action, &target, seq, &msg_hash);
			(action.clone(), target, seq, msg_hash, sig)
		}).collect();

		assert_noop!(
			GroupRobotCommunity::batch_submit_logs(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				logs,
			),
			Error::<Test>::BatchTooLarge
		);

		// Exactly MaxBatchSize (50) should still be valid (if space permits in ActionLogs)
		// But ActionLogs MaxLogsPerCommunity=10, so 50 would hit LogsFull.
		// Test with 2 logs to confirm non-too-large passes
		let small_logs: Vec<_> = (1..=2u64).map(|seq| {
			let sig = test_sign(&community_hash(1), &action, &target, seq, &msg_hash);
			(action.clone(), target, seq, msg_hash, sig)
		}).collect();
		assert_ok!(GroupRobotCommunity::batch_submit_logs(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			small_logs,
		));
	});
}

#[test]
fn m3_r3_clear_expired_logs_uses_configurable_blocks_per_day() {
	new_test_ext().execute_with(|| {
		// community_hash(1) = Basic tier, log_retention_days=30
		// BlocksPerDay=14400, so min_retention = 30 * 14400 = 432000
		let action = ActionType::Kick;
		let target = [0u8; 32];
		let msg_hash = [0u8; 32];
		let sig = test_sign(&community_hash(1), &action, &target, 1, &msg_hash);

		assert_ok!(GroupRobotCommunity::submit_action_log(
			RuntimeOrigin::signed(OWNER),
			community_hash(1), action, target, 1, msg_hash, sig,
		));

		// Advance well past retention
		System::set_block_number(500_000);

		// max_age < min_retention (432000) should fail
		assert_noop!(
			GroupRobotCommunity::clear_expired_logs(
				RuntimeOrigin::signed(OTHER),
				community_hash(1),
				431_999u64,
			),
			Error::<Test>::RetentionPeriodNotExpired
		);

		// max_age >= min_retention should succeed
		assert_ok!(GroupRobotCommunity::clear_expired_logs(
			RuntimeOrigin::signed(OTHER),
			community_hash(1),
			432_000u64,
		));
	});
}

#[test]
fn l2_r3_enterprise_tier_rejects_log_clearing() {
	new_test_ext().execute_with(|| {
		// community_hash(5) = Enterprise tier (log_retention_days=0, permanent)
		// Enterprise tier should always reject clearing
		assert_noop!(
			GroupRobotCommunity::clear_expired_logs(
				RuntimeOrigin::signed(OTHER),
				community_hash(5),
				999_999u64,
			),
			Error::<Test>::RetentionPeriodNotExpired
		);
	});
}

#[test]
fn l2_r3_free_tier_log_clearing_respects_retention() {
	new_test_ext().execute_with(|| {
		// community_hash(2) = Pro tier now, use community_hash(3) for Free
		// Actually community_hash(2) was Free, now Pro. Use community_hash(10) for Free.
		// But mock maps anything not 1/2/5 to Free. Use community_hash(3).
		let free_community = community_hash(3);

		// Free tier, max_age < 100800 should fail with RetentionPeriodNotExpired
		assert_noop!(
			GroupRobotCommunity::clear_expired_logs(
				RuntimeOrigin::signed(OTHER),
				free_community,
				100_799u64,
			),
			Error::<Test>::RetentionPeriodNotExpired
		);

		// Free tier, max_age >= 100800 but no logs → NoLogsToClear
		System::set_block_number(200_000);
		assert_noop!(
			GroupRobotCommunity::clear_expired_logs(
				RuntimeOrigin::signed(OTHER),
				free_community,
				100_800u64,
			),
			Error::<Test>::NoLogsToClear
		);
	});
}

// ============================================================================
// delete_community_config
// ============================================================================

#[test]
fn delete_community_config_works() {
	new_test_ext().execute_with(|| {
		// Create config first
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		assert!(CommunityConfigs::<Test>::contains_key(community_hash(1)));

		// Submit a log so we have data to clean
		let sig = test_sign(&community_hash(1), &ActionType::Ban, &[1u8; 32], 1, &[2u8; 32]);
		assert_ok!(GroupRobotCommunity::submit_action_log(
			RuntimeOrigin::signed(OWNER),
			community_hash(1), ActionType::Ban, [1u8; 32], 1, [2u8; 32], sig,
		));

		// Delete config
		assert_ok!(GroupRobotCommunity::delete_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
		));

		assert!(!CommunityConfigs::<Test>::contains_key(community_hash(1)));
		assert!(ActionLogs::<Test>::get(community_hash(1)).is_empty());
		assert_eq!(LastSequence::<Test>::get(community_hash(1)), None);
	});
}

#[test]
fn delete_community_config_fails_not_owner() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		assert_noop!(
			GroupRobotCommunity::delete_community_config(
				RuntimeOrigin::signed(OTHER),
				community_hash(1),
			),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn delete_community_config_fails_not_found() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::delete_community_config(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
			),
			Error::<Test>::CommunityNotFound
		);
	});
}

// ============================================================================
// force_remove_community
// ============================================================================

#[test]
fn force_remove_community_works() {
	new_test_ext().execute_with(|| {
		// Create config + reputation data
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 10,
		));

		// Force remove as Root
		assert_ok!(GroupRobotCommunity::force_remove_community(
			RuntimeOrigin::root(),
			community_hash(1),
		));

		assert!(!CommunityConfigs::<Test>::contains_key(community_hash(1)));
		assert_eq!(MemberReputation::<Test>::get(community_hash(1), user_hash(1)).score, 0);
	});
}

#[test]
fn force_remove_community_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::force_remove_community(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ============================================================================
// ban_community / unban_community
// ============================================================================

#[test]
fn ban_community_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));

		assert_ok!(GroupRobotCommunity::ban_community(
			RuntimeOrigin::root(), community_hash(1),
		));

		let config = CommunityConfigs::<Test>::get(community_hash(1)).unwrap();
		assert_eq!(config.status, CommunityStatus::Banned);
	});
}

#[test]
fn ban_community_fails_already_banned() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		assert_ok!(GroupRobotCommunity::ban_community(
			RuntimeOrigin::root(), community_hash(1),
		));
		assert_noop!(
			GroupRobotCommunity::ban_community(
				RuntimeOrigin::root(), community_hash(1),
			),
			Error::<Test>::CommunityAlreadyInStatus
		);
	});
}

#[test]
fn ban_community_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::ban_community(
				RuntimeOrigin::signed(OWNER), community_hash(1),
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn ban_community_fails_not_found() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::ban_community(
				RuntimeOrigin::root(), community_hash(99),
			),
			Error::<Test>::CommunityNotFound
		);
	});
}

#[test]
fn unban_community_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		assert_ok!(GroupRobotCommunity::ban_community(
			RuntimeOrigin::root(), community_hash(1),
		));
		assert_ok!(GroupRobotCommunity::unban_community(
			RuntimeOrigin::root(), community_hash(1),
		));

		let config = CommunityConfigs::<Test>::get(community_hash(1)).unwrap();
		assert_eq!(config.status, CommunityStatus::Active);
	});
}

#[test]
fn unban_community_fails_not_banned() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		assert_noop!(
			GroupRobotCommunity::unban_community(
				RuntimeOrigin::root(), community_hash(1),
			),
			Error::<Test>::CommunityNotBanned
		);
	});
}

// ============================================================================
// Ban blocks operations
// ============================================================================

#[test]
fn banned_community_blocks_submit_action_log() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		assert_ok!(GroupRobotCommunity::ban_community(
			RuntimeOrigin::root(), community_hash(1),
		));

		let sig = test_sign(&community_hash(1), &ActionType::Ban, &[1u8; 32], 1, &[2u8; 32]);
		assert_noop!(
			GroupRobotCommunity::submit_action_log(
				RuntimeOrigin::signed(OWNER),
				community_hash(1), ActionType::Ban, [1u8; 32], 1, [2u8; 32], sig,
			),
			Error::<Test>::CommunityBanned
		);
	});
}

#[test]
fn banned_community_blocks_award_reputation() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		assert_ok!(GroupRobotCommunity::ban_community(
			RuntimeOrigin::root(), community_hash(1),
		));
		assert_noop!(
			GroupRobotCommunity::award_reputation(
				RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 10,
			),
			Error::<Test>::CommunityBanned
		);
	});
}

#[test]
fn banned_community_blocks_update_config() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		assert_ok!(GroupRobotCommunity::ban_community(
			RuntimeOrigin::root(), community_hash(1),
		));
		assert_noop!(
			GroupRobotCommunity::update_community_config(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				1, false, 5, 2, WarnAction::Ban, false, false, *b"zh",
			),
			Error::<Test>::CommunityBanned
		);
	});
}

// ============================================================================
// force_update_community_config
// ============================================================================

#[test]
fn force_update_community_config_works() {
	new_test_ext().execute_with(|| {
		// Create initial config
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));

		// Root force update
		assert_ok!(GroupRobotCommunity::force_update_community_config(
			RuntimeOrigin::root(),
			community_hash(1),
			false, 50, 8, WarnAction::Ban, true, true, *b"zh",
		));

		let config = CommunityConfigs::<Test>::get(community_hash(1)).unwrap();
		assert_eq!(config.version, 2);
		assert!(!config.anti_flood_enabled);
		assert_eq!(config.flood_limit, 50);
		assert_eq!(config.warn_limit, 8);
		assert_eq!(config.warn_action, WarnAction::Ban);
		assert!(config.welcome_enabled);
		assert!(config.ads_enabled);
		assert_eq!(config.language, *b"zh");
	});
}

#[test]
fn force_update_community_config_creates_new() {
	new_test_ext().execute_with(|| {
		// No existing config — force update creates one
		assert_ok!(GroupRobotCommunity::force_update_community_config(
			RuntimeOrigin::root(),
			community_hash(1),
			true, 20, 5, WarnAction::Kick, false, false, *b"en",
		));

		let config = CommunityConfigs::<Test>::get(community_hash(1)).unwrap();
		assert_eq!(config.version, 1);
		assert_eq!(config.status, CommunityStatus::Active);
	});
}

#[test]
fn force_update_community_config_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::force_update_community_config(
				RuntimeOrigin::signed(OWNER),
				community_hash(1),
				true, 20, 5, WarnAction::Kick, false, false, *b"en",
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn force_update_community_config_preserves_status() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		assert_ok!(GroupRobotCommunity::ban_community(
			RuntimeOrigin::root(), community_hash(1),
		));

		// Force update preserves Banned status
		assert_ok!(GroupRobotCommunity::force_update_community_config(
			RuntimeOrigin::root(),
			community_hash(1),
			false, 50, 8, WarnAction::Ban, true, true, *b"zh",
		));

		let config = CommunityConfigs::<Test>::get(community_hash(1)).unwrap();
		assert_eq!(config.status, CommunityStatus::Banned);
	});
}

// ============================================================================
// force_reset_community_reputation
// ============================================================================

#[test]
fn force_reset_community_reputation_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(1), 10,
		));
		System::set_block_number(10);
		assert_ok!(GroupRobotCommunity::award_reputation(
			RuntimeOrigin::signed(OWNER), community_hash(1), user_hash(2), 20,
		));

		assert_eq!(MemberReputation::<Test>::get(community_hash(1), user_hash(1)).score, 10);
		assert_eq!(MemberReputation::<Test>::get(community_hash(1), user_hash(2)).score, 20);

		// Root force reset all reputation
		assert_ok!(GroupRobotCommunity::force_reset_community_reputation(
			RuntimeOrigin::root(), community_hash(1),
		));

		assert_eq!(MemberReputation::<Test>::get(community_hash(1), user_hash(1)).score, 0);
		assert_eq!(MemberReputation::<Test>::get(community_hash(1), user_hash(2)).score, 0);
	});
}

#[test]
fn force_reset_community_reputation_fails_not_found() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::force_reset_community_reputation(
				RuntimeOrigin::root(), community_hash(99),
			),
			Error::<Test>::CommunityNotFound
		);
	});
}

#[test]
fn force_reset_community_reputation_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCommunity::force_reset_community_reputation(
				RuntimeOrigin::signed(OWNER), community_hash(1),
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ============================================================================
// Tier gate for reputation
// ============================================================================

#[test]
fn award_reputation_fails_free_tier() {
	new_test_ext().execute_with(|| {
		// community_hash(4) → active bot + Free tier
		assert_noop!(
			GroupRobotCommunity::award_reputation(
				RuntimeOrigin::signed(OWNER), community_hash(4), user_hash(1), 10,
			),
			Error::<Test>::FreeTierNotAllowed
		);
	});
}

#[test]
fn deduct_reputation_fails_free_tier() {
	new_test_ext().execute_with(|| {
		// community_hash(4) → active bot + Free tier
		assert_noop!(
			GroupRobotCommunity::deduct_reputation(
				RuntimeOrigin::signed(OWNER), community_hash(4), user_hash(1), 10,
			),
			Error::<Test>::FreeTierNotAllowed
		);
	});
}

// ============================================================================
// CommunityProvider trait
// ============================================================================

#[test]
fn community_provider_is_configured() {
	new_test_ext().execute_with(|| {
		use pallet_grouprobot_primitives::CommunityProvider;

		assert!(!<GroupRobotCommunity as CommunityProvider>::is_community_configured(&community_hash(1)));

		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));

		assert!(<GroupRobotCommunity as CommunityProvider>::is_community_configured(&community_hash(1)));
	});
}

#[test]
fn community_provider_is_banned() {
	new_test_ext().execute_with(|| {
		use pallet_grouprobot_primitives::CommunityProvider;

		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"en",
		));
		assert!(!<GroupRobotCommunity as CommunityProvider>::is_community_banned(&community_hash(1)));

		assert_ok!(GroupRobotCommunity::ban_community(RuntimeOrigin::root(), community_hash(1)));
		assert!(<GroupRobotCommunity as CommunityProvider>::is_community_banned(&community_hash(1)));
	});
}

#[test]
fn community_provider_ads_disabled_when_banned() {
	new_test_ext().execute_with(|| {
		use pallet_grouprobot_primitives::CommunityProvider;

		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, true, *b"en", // ads_enabled=true
		));
		assert!(<GroupRobotCommunity as CommunityProvider>::is_ads_enabled(&community_hash(1)));

		assert_ok!(GroupRobotCommunity::ban_community(RuntimeOrigin::root(), community_hash(1)));
		assert!(!<GroupRobotCommunity as CommunityProvider>::is_ads_enabled(&community_hash(1)));
	});
}

#[test]
fn community_provider_language() {
	new_test_ext().execute_with(|| {
		use pallet_grouprobot_primitives::CommunityProvider;

		// Default when not configured
		assert_eq!(<GroupRobotCommunity as CommunityProvider>::language(&community_hash(1)), *b"en");

		assert_ok!(GroupRobotCommunity::update_community_config(
			RuntimeOrigin::signed(OWNER),
			community_hash(1),
			0, true, 10, 3, WarnAction::Kick, false, false, *b"zh",
		));
		assert_eq!(<GroupRobotCommunity as CommunityProvider>::language(&community_hash(1)), *b"zh");
	});
}
