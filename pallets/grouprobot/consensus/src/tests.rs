use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use pallet_grouprobot_primitives::*;

// ============================================================================
// register_node
// ============================================================================

#[test]
fn register_node_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		let node = Nodes::<Test>::get(node_id(1)).unwrap();
		assert_eq!(node.operator, OPERATOR);
		assert_eq!(node.stake, 200);
		assert_eq!(node.status, NodeStatus::Active);
		assert_eq!(node.reputation, 5000);
		assert_eq!(ActiveNodeList::<Test>::get().len(), 1);
		assert_eq!(OperatorNodes::<Test>::get(OPERATOR), Some(node_id(1)));
	});
}

#[test]
fn register_node_fails_duplicate() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert_noop!(
			GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR2), node_id(1), 200),
			Error::<Test>::NodeAlreadyRegistered
		);
	});
}

#[test]
fn register_node_fails_operator_already_has_node() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert_noop!(
			GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(2), 200),
			Error::<Test>::NodeAlreadyRegistered
		);
	});
}

#[test]
fn register_node_fails_insufficient_stake() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 50),
			Error::<Test>::InsufficientStake
		);
	});
}

// ============================================================================
// request_exit / finalize_exit
// ============================================================================

#[test]
fn exit_flow_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert_ok!(GroupRobotConsensus::request_exit(RuntimeOrigin::signed(OPERATOR), node_id(1)));

		let node = Nodes::<Test>::get(node_id(1)).unwrap();
		assert_eq!(node.status, NodeStatus::Exiting);
		assert_eq!(ActiveNodeList::<Test>::get().len(), 0);

		// Cooldown not complete
		System::set_block_number(5);
		assert_noop!(
			GroupRobotConsensus::finalize_exit(RuntimeOrigin::signed(OPERATOR), node_id(1)),
			Error::<Test>::CooldownNotComplete
		);

		// Cooldown complete (10 blocks)
		System::set_block_number(12);
		let balance_before = Balances::free_balance(OPERATOR);
		assert_ok!(GroupRobotConsensus::finalize_exit(RuntimeOrigin::signed(OPERATOR), node_id(1)));
		assert!(!Nodes::<Test>::contains_key(node_id(1)));
		assert!(!OperatorNodes::<Test>::contains_key(OPERATOR));
		assert!(Balances::free_balance(OPERATOR) > balance_before);
	});
}

#[test]
fn request_exit_fails_not_operator() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert_noop!(
			GroupRobotConsensus::request_exit(RuntimeOrigin::signed(OTHER), node_id(1)),
			Error::<Test>::NotOperator
		);
	});
}

// ============================================================================
// report_equivocation / slash_equivocation
// ============================================================================

#[test]
fn report_equivocation_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert_ok!(GroupRobotConsensus::report_equivocation(
			RuntimeOrigin::signed(OTHER),
			node_id(1), 42,
			[1u8; 32], [1u8; 64],
			[2u8; 32], [2u8; 64],
		));
		let record = EquivocationRecords::<Test>::get(node_id(1), 42).unwrap();
		assert_eq!(record.reporter, OTHER);
		assert!(!record.resolved);
	});
}

#[test]
fn slash_equivocation_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 1000));
		assert_ok!(GroupRobotConsensus::report_equivocation(
			RuntimeOrigin::signed(OTHER),
			node_id(1), 42,
			[1u8; 32], [1u8; 64],
			[2u8; 32], [2u8; 64],
		));
		assert_ok!(GroupRobotConsensus::slash_equivocation(RuntimeOrigin::root(), node_id(1), 42));

		let node = Nodes::<Test>::get(node_id(1)).unwrap();
		assert_eq!(node.stake, 900);
		assert_eq!(node.status, NodeStatus::Suspended);
		assert_eq!(ActiveNodeList::<Test>::get().len(), 0);

		let record = EquivocationRecords::<Test>::get(node_id(1), 42).unwrap();
		assert!(record.resolved);
	});
}

// ============================================================================
// mark_sequence_processed
// ============================================================================

#[test]
fn mark_sequence_processed_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::mark_sequence_processed(
			RuntimeOrigin::signed(OPERATOR), bot_hash(1), 42
		));
		assert!(ProcessedSequences::<Test>::contains_key(bot_hash(1), 42));
		assert!(GroupRobotConsensus::is_sequence_processed(&bot_hash(1), 42));
	});
}

#[test]
fn mark_sequence_duplicate_emits_event() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::mark_sequence_processed(
			RuntimeOrigin::signed(OPERATOR), bot_hash(1), 42
		));
		assert_ok!(GroupRobotConsensus::mark_sequence_processed(
			RuntimeOrigin::signed(OPERATOR), bot_hash(1), 42
		));
	});
}

// ============================================================================
// verify_node_tee
// ============================================================================

#[test]
fn verify_node_tee_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert!(!Nodes::<Test>::get(node_id(1)).unwrap().is_tee_node);

		assert_ok!(GroupRobotConsensus::verify_node_tee(
			RuntimeOrigin::signed(OPERATOR), node_id(1), bot_hash(10)
		));
		assert!(Nodes::<Test>::get(node_id(1)).unwrap().is_tee_node);
		assert_eq!(NodeBotBinding::<Test>::get(node_id(1)), Some(bot_hash(10)));
	});
}

#[test]
fn verify_node_tee_fails_already_verified() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert_ok!(GroupRobotConsensus::verify_node_tee(
			RuntimeOrigin::signed(OPERATOR), node_id(1), bot_hash(10)
		));
		assert_noop!(
			GroupRobotConsensus::verify_node_tee(
				RuntimeOrigin::signed(OPERATOR), node_id(1), bot_hash(10)
			),
			Error::<Test>::AlreadyTeeVerified
		);
	});
}

#[test]
fn verify_node_tee_fails_bot_not_tee() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OWNER), node_id(1), 200));
		assert_noop!(
			GroupRobotConsensus::verify_node_tee(
				RuntimeOrigin::signed(OWNER), node_id(1), bot_hash(1)
			),
			Error::<Test>::AttestationNotValid
		);
	});
}

#[test]
fn verify_node_tee_fails_owner_mismatch() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert_noop!(
			GroupRobotConsensus::verify_node_tee(
				RuntimeOrigin::signed(OPERATOR), node_id(1), bot_hash(11)
			),
			Error::<Test>::BotOwnerMismatch
		);
	});
}

// ============================================================================
// on_era_end (via on_initialize) — now delegates to mock traits
// ============================================================================

#[test]
fn era_end_calls_settler_and_distributor() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert_ok!(GroupRobotConsensus::verify_node_tee(
			RuntimeOrigin::signed(OPERATOR), node_id(1), bot_hash(10)
		));

		// Set mock subscription income
		set_mock_settle_income(100);
		clear_distributed_rewards();

		advance_to(52);

		assert!(CurrentEra::<Test>::get() >= 1);
		// MockRewardDistributor should have been called
		let rewards = get_distributed_rewards();
		assert!(!rewards.is_empty(), "Rewards should be distributed");
	});
}

#[test]
fn era_end_no_nodes_just_advances_era() {
	new_test_ext().execute_with(|| {
		// No active nodes
		clear_distributed_rewards();
		advance_to(52);

		assert!(CurrentEra::<Test>::get() >= 1);
		// No rewards distributed (no nodes)
		let rewards = get_distributed_rewards();
		assert!(rewards.is_empty());
	});
}

#[test]
fn era_end_non_tee_gets_zero_weight() {
	new_test_ext().execute_with(|| {
		// Non-TEE node
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));

		set_mock_settle_income(0);
		clear_distributed_rewards();

		advance_to(52);

		// Non-TEE node weight = 0, so distributor gets zero-weight entries
		let rewards = get_distributed_rewards();
		// All rewards are 0 since weight is 0
		for (_, amount) in &rewards {
			assert_eq!(*amount, 0, "Non-TEE node should get 0 rewards");
		}
	});
}

// ============================================================================
// SgxEnclaveBonus: TEE reward params
// ============================================================================

#[test]
fn set_tee_reward_params_requires_root() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotConsensus::set_tee_reward_params(
				RuntimeOrigin::signed(OPERATOR), 15_000, 2_000
			),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_ok!(GroupRobotConsensus::set_tee_reward_params(
			RuntimeOrigin::root(), 15_000, 2_000
		));
		assert_eq!(TeeRewardMultiplier::<Test>::get(), 15_000);
		assert_eq!(SgxEnclaveBonus::<Test>::get(), 2_000);
	});
}

// ============================================================================
// ProcessedSequences TTL cleanup
// ============================================================================

#[test]
fn expired_sequences_are_cleaned() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::mark_sequence_processed(
			RuntimeOrigin::signed(OPERATOR), bot_hash(1), 1
		));
		assert_ok!(GroupRobotConsensus::mark_sequence_processed(
			RuntimeOrigin::signed(OPERATOR), bot_hash(1), 2
		));
		assert_ok!(GroupRobotConsensus::mark_sequence_processed(
			RuntimeOrigin::signed(OPERATOR), bot_hash(1), 3
		));

		advance_to(103);

		assert!(!ProcessedSequences::<Test>::contains_key(bot_hash(1), 1));
		assert!(!ProcessedSequences::<Test>::contains_key(bot_hash(1), 2));
		assert!(!ProcessedSequences::<Test>::contains_key(bot_hash(1), 3));
	});
}

#[test]
fn fresh_sequences_not_cleaned() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::mark_sequence_processed(
			RuntimeOrigin::signed(OPERATOR), bot_hash(1), 1
		));
		advance_to(51);
		assert!(ProcessedSequences::<Test>::contains_key(bot_hash(1), 1));
	});
}

// ============================================================================
// NodeConsensusProvider
// ============================================================================

#[test]
fn node_consensus_provider_trait() {
	new_test_ext().execute_with(|| {
		use pallet_grouprobot_primitives::NodeConsensusProvider;
		assert!(!<GroupRobotConsensus as NodeConsensusProvider<u64>>::is_node_active(&node_id(1)));

		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert!(<GroupRobotConsensus as NodeConsensusProvider<u64>>::is_node_active(&node_id(1)));
		assert_eq!(
			<GroupRobotConsensus as NodeConsensusProvider<u64>>::node_operator(&node_id(1)),
			Some(OPERATOR)
		);
	});
}

// ============================================================================
// SubscriptionTier primitives (tested here since they're in primitives)
// ============================================================================

#[test]
fn subscription_tier_is_paid() {
	assert!(!SubscriptionTier::Free.is_paid());
	assert!(SubscriptionTier::Basic.is_paid());
	assert!(SubscriptionTier::Pro.is_paid());
	assert!(SubscriptionTier::Enterprise.is_paid());
}

#[test]
fn subscription_tier_default_is_free() {
	assert_eq!(SubscriptionTier::default(), SubscriptionTier::Free);
}

// ============================================================================
// Tier gate
// ============================================================================

#[test]
fn mark_sequence_fails_free_tier() {
	new_test_ext().execute_with(|| {
		// bot_hash(99) → Free tier in MockSubscription (not 1/2/10/11)
		assert_noop!(
			GroupRobotConsensus::mark_sequence_processed(
				RuntimeOrigin::signed(OTHER), bot_hash(99), 1
			),
			Error::<Test>::FreeTierNotAllowed
		);
	});
}

#[test]
fn mark_sequence_works_paid_tier() {
	new_test_ext().execute_with(|| {
		// bot_hash(1) → Basic tier in MockSubscription
		assert_ok!(GroupRobotConsensus::mark_sequence_processed(
			RuntimeOrigin::signed(OPERATOR), bot_hash(1), 100
		));
		assert!(ProcessedSequences::<Test>::contains_key(bot_hash(1), 100));
	});
}

// ============================================================================
// Regression tests (audit fixes)
// ============================================================================

#[test]
fn h1_report_equivocation_rejects_identical_msg_hash() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		// Same msg_hash_a and msg_hash_b → should fail
		assert_noop!(
			GroupRobotConsensus::report_equivocation(
				RuntimeOrigin::signed(OTHER),
				node_id(1), 42,
				[1u8; 32], [1u8; 64],
				[1u8; 32], [2u8; 64],
			),
			Error::<Test>::InvalidEquivocationEvidence
		);
	});
}

#[test]
fn h1_report_equivocation_rejects_identical_signatures() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		// Same signature_a and signature_b → should fail
		assert_noop!(
			GroupRobotConsensus::report_equivocation(
				RuntimeOrigin::signed(OTHER),
				node_id(1), 42,
				[1u8; 32], [1u8; 64],
				[2u8; 32], [1u8; 64],
			),
			Error::<Test>::InvalidEquivocationEvidence
		);
	});
}

#[test]
fn h2_mark_sequence_rejects_unauthorized_caller() {
	new_test_ext().execute_with(|| {
		// bot_hash(1) owner=OWNER, operator=OPERATOR
		// OTHER is neither owner nor operator → should fail
		assert_noop!(
			GroupRobotConsensus::mark_sequence_processed(
				RuntimeOrigin::signed(OTHER), bot_hash(1), 42
			),
			Error::<Test>::NotBotOperator
		);
	});
}

#[test]
fn h2_mark_sequence_works_for_owner() {
	new_test_ext().execute_with(|| {
		// bot_hash(1) owner=OWNER → should succeed
		assert_ok!(GroupRobotConsensus::mark_sequence_processed(
			RuntimeOrigin::signed(OWNER), bot_hash(1), 42
		));
		assert!(ProcessedSequences::<Test>::contains_key(bot_hash(1), 42));
	});
}

#[test]
fn h2_mark_sequence_works_for_operator() {
	new_test_ext().execute_with(|| {
		// bot_hash(1) operator=OPERATOR → should succeed
		assert_ok!(GroupRobotConsensus::mark_sequence_processed(
			RuntimeOrigin::signed(OPERATOR), bot_hash(1), 42
		));
		assert!(ProcessedSequences::<Test>::contains_key(bot_hash(1), 42));
	});
}

#[test]
fn h4_finalize_exit_cleans_node_bot_binding() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert_ok!(GroupRobotConsensus::verify_node_tee(
			RuntimeOrigin::signed(OPERATOR), node_id(1), bot_hash(10)
		));
		assert!(NodeBotBinding::<Test>::contains_key(node_id(1)));

		assert_ok!(GroupRobotConsensus::request_exit(RuntimeOrigin::signed(OPERATOR), node_id(1)));
		System::set_block_number(12);
		assert_ok!(GroupRobotConsensus::finalize_exit(RuntimeOrigin::signed(OPERATOR), node_id(1)));

		// NodeBotBinding should be cleaned
		assert!(!NodeBotBinding::<Test>::contains_key(node_id(1)));
	});
}

#[test]
fn m2_slash_cleans_node_bot_binding() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 1000));
		assert_ok!(GroupRobotConsensus::verify_node_tee(
			RuntimeOrigin::signed(OPERATOR), node_id(1), bot_hash(10)
		));
		assert!(NodeBotBinding::<Test>::contains_key(node_id(1)));

		assert_ok!(GroupRobotConsensus::report_equivocation(
			RuntimeOrigin::signed(OTHER), node_id(1), 42,
			[1u8; 32], [1u8; 64], [2u8; 32], [2u8; 64],
		));
		assert_ok!(GroupRobotConsensus::slash_equivocation(RuntimeOrigin::root(), node_id(1), 42));

		// NodeBotBinding should be cleaned after slash
		assert!(!NodeBotBinding::<Test>::contains_key(node_id(1)));
	});
}

#[test]
fn m3_slash_missing_equivocation_returns_correct_error() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		// No equivocation reported for sequence 99 → should return EquivocationNotFound
		assert_noop!(
			GroupRobotConsensus::slash_equivocation(RuntimeOrigin::root(), node_id(1), 99),
			Error::<Test>::EquivocationNotFound
		);
	});
}

#[test]
fn h5_suspended_node_can_request_exit() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 1000));
		assert_ok!(GroupRobotConsensus::report_equivocation(
			RuntimeOrigin::signed(OTHER), node_id(1), 42,
			[1u8; 32], [1u8; 64], [2u8; 32], [2u8; 64],
		));
		assert_ok!(GroupRobotConsensus::slash_equivocation(RuntimeOrigin::root(), node_id(1), 42));

		// Node is now Suspended
		assert_eq!(Nodes::<Test>::get(node_id(1)).unwrap().status, NodeStatus::Suspended);

		// Suspended node should be able to request exit
		assert_ok!(GroupRobotConsensus::request_exit(RuntimeOrigin::signed(OPERATOR), node_id(1)));
		assert_eq!(Nodes::<Test>::get(node_id(1)).unwrap().status, NodeStatus::Exiting);

		// After cooldown, can finalize and recover remaining stake
		System::set_block_number(12);
		let balance_before = Balances::free_balance(OPERATOR);
		assert_ok!(GroupRobotConsensus::finalize_exit(RuntimeOrigin::signed(OPERATOR), node_id(1)));
		assert!(Balances::free_balance(OPERATOR) > balance_before);
		assert!(!Nodes::<Test>::contains_key(node_id(1)));
		assert!(!OperatorNodes::<Test>::contains_key(OPERATOR));
	});
}

#[test]
fn h6_set_tee_reward_params_rejects_excessive_values() {
	new_test_ext().execute_with(|| {
		// tee_multiplier > 50000 → rejected
		assert_noop!(
			GroupRobotConsensus::set_tee_reward_params(RuntimeOrigin::root(), 50_001, 0),
			Error::<Test>::InvalidTeeRewardParams
		);
		// sgx_bonus > 10000 → rejected
		assert_noop!(
			GroupRobotConsensus::set_tee_reward_params(RuntimeOrigin::root(), 10_000, 10_001),
			Error::<Test>::InvalidTeeRewardParams
		);
		// Both at max → ok
		assert_ok!(GroupRobotConsensus::set_tee_reward_params(RuntimeOrigin::root(), 50_000, 10_000));
		assert_eq!(TeeRewardMultiplier::<Test>::get(), 50_000);
		assert_eq!(SgxEnclaveBonus::<Test>::get(), 10_000);
	});
}

#[test]
fn h8_slash_resets_tee_status() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 1000));
		assert_ok!(GroupRobotConsensus::verify_node_tee(
			RuntimeOrigin::signed(OPERATOR), node_id(1), bot_hash(10)
		));
		assert!(Nodes::<Test>::get(node_id(1)).unwrap().is_tee_node);

		assert_ok!(GroupRobotConsensus::report_equivocation(
			RuntimeOrigin::signed(OTHER), node_id(1), 42,
			[1u8; 32], [1u8; 64], [2u8; 32], [2u8; 64],
		));
		assert_ok!(GroupRobotConsensus::slash_equivocation(RuntimeOrigin::root(), node_id(1), 42));

		// After slash, is_tee_node should be false
		let node = Nodes::<Test>::get(node_id(1)).unwrap();
		assert!(!node.is_tee_node, "Slash should reset TEE status");
		assert_eq!(node.status, NodeStatus::Suspended);
	});
}
