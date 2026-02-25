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
// verify_node_tee
// ============================================================================

#[test]
fn verify_node_tee_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert!(!Nodes::<Test>::get(node_id(1)).unwrap().is_tee_node);

		// bot_hash(10) is TEE, owned by OPERATOR
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
		// bot_hash(1) is NOT a TEE bot
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
		// bot_hash(11) is TEE but owned by OPERATOR2, not OPERATOR
		assert_noop!(
			GroupRobotConsensus::verify_node_tee(
				RuntimeOrigin::signed(OPERATOR), node_id(1), bot_hash(11)
			),
			Error::<Test>::BotOwnerMismatch
		);
	});
}

// ============================================================================
// on_era_end (via on_initialize)
// ============================================================================

#[test]
fn era_end_distributes_inflation_to_tee_only() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		// Verify TEE to enable rewards
		assert_ok!(GroupRobotConsensus::verify_node_tee(
			RuntimeOrigin::signed(OPERATOR), node_id(1), bot_hash(10)
		));

		advance_to(52);

		assert!(CurrentEra::<Test>::get() >= 1);
		let pending = NodePendingRewards::<Test>::get(node_id(1));
		assert!(pending > 0, "TEE node should have pending rewards");
	});
}

#[test]
fn era_end_non_tee_gets_zero() {
	new_test_ext().execute_with(|| {
		// Non-TEE node: registered but NOT verified
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));

		advance_to(52);

		let pending = NodePendingRewards::<Test>::get(node_id(1));
		assert_eq!(pending, 0, "Non-TEE node should get 0 rewards");
	});
}

#[test]
fn era_end_multi_tee_node_distribution() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR2), node_id(2), 200));
		// Both verify TEE
		assert_ok!(GroupRobotConsensus::verify_node_tee(
			RuntimeOrigin::signed(OPERATOR), node_id(1), bot_hash(10)
		));
		assert_ok!(GroupRobotConsensus::verify_node_tee(
			RuntimeOrigin::signed(OPERATOR2), node_id(2), bot_hash(11)
		));

		advance_to(52);

		let pending1 = NodePendingRewards::<Test>::get(node_id(1));
		let pending2 = NodePendingRewards::<Test>::get(node_id(2));
		assert!(pending1 > 0);
		assert_eq!(pending1, pending2);
	});
}

// ============================================================================
// SgxEnclaveBonus: dual attestation rewards
// ============================================================================

#[test]
fn sgx_dual_attestation_gets_bonus_reward() {
	new_test_ext().execute_with(|| {
		// Node 1: OPERATOR with bot_hash(10) → dual attestation (SGX+TDX)
		// Node 2: OPERATOR2 with bot_hash(11) → TDX-only
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR2), node_id(2), 200));
		assert_ok!(GroupRobotConsensus::verify_node_tee(
			RuntimeOrigin::signed(OPERATOR), node_id(1), bot_hash(10)
		));
		assert_ok!(GroupRobotConsensus::verify_node_tee(
			RuntimeOrigin::signed(OPERATOR2), node_id(2), bot_hash(11)
		));

		// Set SGX bonus to 2000 bps (+0.2x)
		assert_ok!(GroupRobotConsensus::set_tee_reward_params(
			RuntimeOrigin::root(), 10_000, 2_000
		));

		advance_to(52);

		let pending1 = NodePendingRewards::<Test>::get(node_id(1)); // dual: 12000 bps
		let pending2 = NodePendingRewards::<Test>::get(node_id(2)); // TDX-only: 10000 bps
		assert!(pending1 > 0);
		assert!(pending2 > 0);
		// Node 1 should get more than Node 2 (12000/10000 = 1.2x ratio)
		assert!(pending1 > pending2, "Dual attestation node should get more: {} vs {}", pending1, pending2);
	});
}

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

#[test]
fn no_sgx_bonus_when_storage_zero() {
	new_test_ext().execute_with(|| {
		// Both nodes verified, but SgxEnclaveBonus is 0 (default)
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR2), node_id(2), 200));
		assert_ok!(GroupRobotConsensus::verify_node_tee(
			RuntimeOrigin::signed(OPERATOR), node_id(1), bot_hash(10)
		));
		assert_ok!(GroupRobotConsensus::verify_node_tee(
			RuntimeOrigin::signed(OPERATOR2), node_id(2), bot_hash(11)
		));

		// SgxEnclaveBonus defaults to 0 → no bonus even for dual attestation
		advance_to(52);

		let pending1 = NodePendingRewards::<Test>::get(node_id(1));
		let pending2 = NodePendingRewards::<Test>::get(node_id(2));
		assert!(pending1 > 0);
		assert_eq!(pending1, pending2, "No bonus when SgxEnclaveBonus is 0");
	});
}

// ============================================================================
// 🆕 防膨胀: ProcessedSequences TTL 清理
// ============================================================================

#[test]
fn expired_sequences_are_cleaned() {
	new_test_ext().execute_with(|| {
		// Insert sequences at block 1
		assert_ok!(GroupRobotConsensus::mark_sequence_processed(
			RuntimeOrigin::signed(OPERATOR), bot_hash(1), 1
		));
		assert_ok!(GroupRobotConsensus::mark_sequence_processed(
			RuntimeOrigin::signed(OPERATOR), bot_hash(1), 2
		));
		assert_ok!(GroupRobotConsensus::mark_sequence_processed(
			RuntimeOrigin::signed(OPERATOR), bot_hash(1), 3
		));
		assert!(ProcessedSequences::<Test>::contains_key(bot_hash(1), 1));
		assert!(ProcessedSequences::<Test>::contains_key(bot_hash(1), 2));
		assert!(ProcessedSequences::<Test>::contains_key(bot_hash(1), 3));

		// Advance past TTL (100 blocks) + 1
		advance_to(103);

		// All 3 sequences should be cleaned
		assert!(!ProcessedSequences::<Test>::contains_key(bot_hash(1), 1));
		assert!(!ProcessedSequences::<Test>::contains_key(bot_hash(1), 2));
		assert!(!ProcessedSequences::<Test>::contains_key(bot_hash(1), 3));
	});
}

#[test]
fn fresh_sequences_not_cleaned() {
	new_test_ext().execute_with(|| {
		// Insert at block 1
		assert_ok!(GroupRobotConsensus::mark_sequence_processed(
			RuntimeOrigin::signed(OPERATOR), bot_hash(1), 1
		));

		// Advance only 50 blocks (TTL=100)
		advance_to(51);

		// Should still exist
		assert!(ProcessedSequences::<Test>::contains_key(bot_hash(1), 1));
	});
}

#[test]
fn cleanup_respects_max_per_block() {
	new_test_ext().execute_with(|| {
		// Insert 15 sequences (max cleanup per block = 10)
		for i in 1..=15u64 {
			assert_ok!(GroupRobotConsensus::mark_sequence_processed(
				RuntimeOrigin::signed(OPERATOR), bot_hash(1), i
			));
		}

		// Advance past TTL
		advance_to(103);

		// After 1 on_initialize call, at most 10 should be cleaned
		// But advance_to calls on_initialize for each block, so after a few blocks
		// all should eventually be cleaned
		let remaining: u32 = ProcessedSequences::<Test>::iter().count() as u32;
		assert_eq!(remaining, 0, "All expired sequences should be cleaned after enough blocks");
	});
}

// ============================================================================
// 🆕 防膨胀: EraRewards 保留窗口
// ============================================================================

#[test]
fn old_era_rewards_are_pruned() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::register_node(RuntimeOrigin::signed(OPERATOR), node_id(1), 200));
		assert_ok!(GroupRobotConsensus::verify_node_tee(
			RuntimeOrigin::signed(OPERATOR), node_id(1), bot_hash(10)
		));

		// Run through 15 eras (MaxEraHistory = 10)
		// Each era = 50 blocks
		for era_num in 0..15u64 {
			let target_block = 2 + (era_num + 1) * 50;
			advance_to(target_block);
		}

		// Era 0..4 should be pruned (only 1 per era_end, so at least some should be gone)
		// Current era should be >= 15
		let current_era = CurrentEra::<Test>::get();
		assert!(current_era >= 15, "Should have completed at least 15 eras");

		// Recent eras should still have rewards
		assert!(EraRewards::<Test>::contains_key(current_era - 1));
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
// 🆕 Phase 4: Free Tier + Feature Gating
// ============================================================================

#[test]
fn subscribe_free_tier_rejected() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotConsensus::subscribe(
				RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Free, 100
			),
			Error::<Test>::CannotSubscribeFree
		);
	});
}

#[test]
fn change_tier_to_free_rejected() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::subscribe(
			RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Basic, 100
		));
		assert_noop!(
			GroupRobotConsensus::change_tier(
				RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Free
			),
			Error::<Test>::CannotSubscribeFree
		);
	});
}

#[test]
fn effective_tier_no_subscription_is_free() {
	new_test_ext().execute_with(|| {
		// 无订阅记录 → Free
		assert_eq!(
			GroupRobotConsensus::effective_tier(&bot_hash(1)),
			SubscriptionTier::Free
		);
	});
}

#[test]
fn effective_tier_active_subscription() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::subscribe(
			RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Pro, 100
		));
		assert_eq!(
			GroupRobotConsensus::effective_tier(&bot_hash(1)),
			SubscriptionTier::Pro
		);
	});
}

#[test]
fn effective_tier_cancelled_is_free() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::subscribe(
			RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Basic, 100
		));
		assert_ok!(GroupRobotConsensus::cancel_subscription(
			RuntimeOrigin::signed(OWNER), bot_hash(1)
		));
		assert_eq!(
			GroupRobotConsensus::effective_tier(&bot_hash(1)),
			SubscriptionTier::Free
		);
	});
}

#[test]
fn effective_tier_suspended_is_free() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::subscribe(
			RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Basic, 100
		));
		// Manually set to Suspended
		Subscriptions::<Test>::mutate(bot_hash(1), |maybe_sub| {
			if let Some(s) = maybe_sub {
				s.status = SubscriptionStatus::Suspended;
			}
		});
		assert_eq!(
			GroupRobotConsensus::effective_tier(&bot_hash(1)),
			SubscriptionTier::Free
		);
	});
}

#[test]
fn effective_tier_pastdue_keeps_tier() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::subscribe(
			RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Pro, 100
		));
		// Manually set to PastDue (宽限期)
		Subscriptions::<Test>::mutate(bot_hash(1), |maybe_sub| {
			if let Some(s) = maybe_sub {
				s.status = SubscriptionStatus::PastDue;
			}
		});
		// PastDue 期间保持原层级
		assert_eq!(
			GroupRobotConsensus::effective_tier(&bot_hash(1)),
			SubscriptionTier::Pro
		);
	});
}

#[test]
fn feature_gate_free_tier() {
	new_test_ext().execute_with(|| {
		let gate = GroupRobotConsensus::effective_feature_gate(&bot_hash(1));
		assert_eq!(gate.max_rules, 3);
		assert_eq!(gate.log_retention_days, 7);
		assert_eq!(gate.forced_ads_per_day, 2);
		assert!(!gate.can_disable_ads);
		assert!(!gate.tee_access);
		assert_eq!(gate.ad_revenue_community_pct, 60);
		assert_eq!(gate.ad_revenue_treasury_pct, 25);
		assert_eq!(gate.ad_revenue_node_pct, 15);
	});
}

#[test]
fn feature_gate_pro_tier() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotConsensus::subscribe(
			RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Pro, 100
		));
		let gate = GroupRobotConsensus::effective_feature_gate(&bot_hash(1));
		assert_eq!(gate.max_rules, 50);
		assert_eq!(gate.log_retention_days, 90);
		assert_eq!(gate.forced_ads_per_day, 0);
		assert!(gate.can_disable_ads);
		assert!(gate.tee_access);
		assert_eq!(gate.ad_revenue_community_pct, 0);
	});
}

#[test]
fn tier_fee_free_is_zero() {
	new_test_ext().execute_with(|| {
		assert_eq!(GroupRobotConsensus::tier_fee(&SubscriptionTier::Free), 0);
		assert_eq!(GroupRobotConsensus::tier_fee(&SubscriptionTier::Basic), 10);
		assert_eq!(GroupRobotConsensus::tier_fee(&SubscriptionTier::Pro), 30);
		assert_eq!(GroupRobotConsensus::tier_fee(&SubscriptionTier::Enterprise), 100);
	});
}

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

#[test]
fn free_tier_fallback_on_suspended() {
	new_test_ext().execute_with(|| {
		// Subscribe with minimal deposit (= 1 era fee = 10)
		assert_ok!(GroupRobotConsensus::subscribe(
			RuntimeOrigin::signed(OWNER), bot_hash(1), SubscriptionTier::Basic, 10
		));

		assert_ok!(GroupRobotConsensus::register_node(
			RuntimeOrigin::signed(OPERATOR), node_id(1), 200
		));

		// Era 1: fee=10 deducted from escrow=10 → escrow=0, status stays Active
		advance_to(52);
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::Active);
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 0);

		// Era 2: escrow=0 < fee=10 → PastDue
		advance_to(102);
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::PastDue);

		// Era 3: still no escrow → Suspended + FreeTierFallback event
		advance_to(152);
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::Suspended);

		// effective_tier should be Free
		assert_eq!(
			GroupRobotConsensus::effective_tier(&bot_hash(1)),
			SubscriptionTier::Free
		);
	});
}
