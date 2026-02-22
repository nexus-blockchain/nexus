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
		// stake returned
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
		// 10% slash of 1000 = 100
		assert_eq!(node.stake, 900);
		assert_eq!(node.status, NodeStatus::Suspended);
		assert_eq!(ActiveNodeList::<Test>::get().len(), 0);

		let record = EquivocationRecords::<Test>::get(node_id(1), 42).unwrap();
		assert!(record.resolved);
	});
}

// ============================================================================
// subscribe / deposit / cancel / change_tier
// ============================================================================

#[test]
fn subscribe_works() {
	new_test_ext().execute_with(|| {
		// bot_hash(1) is active, owned by OWNER
		assert_ok!(GroupRobotConsensus::subscribe(
			RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Basic, 100
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.owner, OWNER);
		assert_eq!(sub.tier, SubscriptionTier::Basic);
		assert_eq!(sub.fee_per_era, 10); // BasicFee
		assert_eq!(sub.status, SubscriptionStatus::Active);
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 100);
	});
}

#[test]
fn subscribe_fails_bot_not_registered() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotConsensus::subscribe(
				RuntimeOrigin::signed(OWNER), bot_hash(99), SubscriptionTier::Basic, 100
			),
			Error::<Test>::BotNotRegistered
		);
	});
}

#[test]
fn subscribe_fails_not_owner() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotConsensus::subscribe(
				RuntimeOrigin::signed(OTHER), bot_hash(1), SubscriptionTier::Basic, 100
			),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn subscribe_fails_insufficient_deposit() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotConsensus::subscribe(
				RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Basic, 5
			),
			Error::<Test>::InsufficientDeposit
		);
	});
}

#[test]
fn deposit_subscription_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::subscribe(
			RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Basic, 100
		));
		assert_ok!(GroupRobotConsensus::deposit_subscription(
			RuntimeOrigin::signed(OWNER), bot_hash(1), 50
		));
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 150);
	});
}

#[test]
fn cancel_subscription_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::subscribe(
			RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Basic, 100
		));
		let balance_before = Balances::free_balance(OWNER);
		assert_ok!(GroupRobotConsensus::cancel_subscription(
			RuntimeOrigin::signed(OWNER), bot_hash(1)
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::Cancelled);
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 0);
		// escrow returned
		assert!(Balances::free_balance(OWNER) > balance_before);
	});
}

#[test]
fn cancel_subscription_fails_double() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::subscribe(
			RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Basic, 100
		));
		assert_ok!(GroupRobotConsensus::cancel_subscription(RuntimeOrigin::signed(OWNER), bot_hash(1)));
		assert_noop!(
			GroupRobotConsensus::cancel_subscription(RuntimeOrigin::signed(OWNER), bot_hash(1)),
			Error::<Test>::SubscriptionAlreadyCancelled
		);
	});
}

#[test]
fn change_tier_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::subscribe(
			RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Basic, 100
		));
		assert_ok!(GroupRobotConsensus::change_tier(
			RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Pro
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.tier, SubscriptionTier::Pro);
		assert_eq!(sub.fee_per_era, 30);
	});
}

#[test]
fn change_tier_fails_same() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::subscribe(
			RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Basic, 100
		));
		assert_noop!(
			GroupRobotConsensus::change_tier(
				RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Basic
			),
			Error::<Test>::SameTier
		);
	});
}

// ============================================================================
// claim_rewards
// ============================================================================

#[test]
fn claim_rewards_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		// Manually set pending rewards
		NodePendingRewards::<Test>::insert(node_id(1), 500u128);

		let balance_before = Balances::free_balance(OPERATOR);
		assert_ok!(GroupRobotConsensus::claim_rewards(RuntimeOrigin::signed(OPERATOR), node_id(1)));
		assert_eq!(Balances::free_balance(OPERATOR), balance_before + 500);
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), 0);
		assert_eq!(NodeTotalEarned::<Test>::get(node_id(1)), 500);
	});
}

#[test]
fn claim_rewards_fails_no_pending() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert_noop!(
			GroupRobotConsensus::claim_rewards(RuntimeOrigin::signed(OPERATOR), node_id(1)),
			Error::<Test>::NoPendingRewards
		);
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
		// Second call should not error, just emit SequenceDuplicate
		assert_ok!(GroupRobotConsensus::mark_sequence_processed(
			RuntimeOrigin::signed(OPERATOR), bot_hash(1), 42
		));
	});
}

// ============================================================================
// set_node_tee_status
// ============================================================================

#[test]
fn set_node_tee_status_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert!(!Nodes::<Test>::get(node_id(1)).unwrap().is_tee_node);

		assert_ok!(GroupRobotConsensus::set_node_tee_status(
			RuntimeOrigin::signed(OPERATOR), node_id(1), true
		));
		assert!(Nodes::<Test>::get(node_id(1)).unwrap().is_tee_node);
	});
}

#[test]
fn set_node_tee_status_fails_same() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert_noop!(
			GroupRobotConsensus::set_node_tee_status(RuntimeOrigin::signed(OPERATOR), node_id(1), false),
			Error::<Test>::SameTeeStatus
		);
	});
}

// ============================================================================
// on_era_end (via on_initialize)
// ============================================================================

#[test]
fn era_end_distributes_inflation() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));

		// Advance past first era (50 blocks)
		advance_to(52);

		// Check era advanced
		assert!(CurrentEra::<Test>::get() >= 1);

		// Node should have pending rewards from inflation
		let pending = NodePendingRewards::<Test>::get(node_id(1));
		assert!(pending > 0, "Node should have pending rewards");
	});
}

#[test]
fn era_end_multi_node_distribution() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR2), node_id(2), 200));

		advance_to(52);

		let pending1 = NodePendingRewards::<Test>::get(node_id(1));
		let pending2 = NodePendingRewards::<Test>::get(node_id(2));
		// Both should get equal rewards (same reputation)
		assert!(pending1 > 0);
		assert_eq!(pending1, pending2);
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
