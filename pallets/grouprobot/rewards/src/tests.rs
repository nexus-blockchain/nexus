use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use pallet_grouprobot_primitives::*;

#[test]
fn claim_rewards_works() {
	new_test_ext().execute_with(|| {
		// Seed pending rewards
		NodePendingRewards::<Test>::insert(node_id(1), 500u128);

		let pool_before = Balances::free_balance(REWARD_POOL);
		let op_before = Balances::free_balance(OPERATOR);

		assert_ok!(Rewards::claim_rewards(RuntimeOrigin::signed(OPERATOR), node_id(1)));
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), 0);
		assert_eq!(NodeTotalEarned::<Test>::get(node_id(1)), 500);

		// C1-fix: rewards transferred from RewardPool, not minted
		assert_eq!(Balances::free_balance(REWARD_POOL), pool_before - 500);
		assert_eq!(Balances::free_balance(OPERATOR), op_before + 500);
	});
}

#[test]
fn claim_rewards_fails_no_pending() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Rewards::claim_rewards(RuntimeOrigin::signed(OPERATOR), node_id(1)),
			Error::<Test>::NoPendingRewards
		);
	});
}

#[test]
fn claim_rewards_fails_not_operator() {
	new_test_ext().execute_with(|| {
		NodePendingRewards::<Test>::insert(node_id(99), 500u128);
		assert_noop!(
			Rewards::claim_rewards(RuntimeOrigin::signed(OTHER), node_id(99)),
			Error::<Test>::NodeNotFound
		);
	});
}

#[test]
fn claim_rewards_fails_wrong_operator() {
	new_test_ext().execute_with(|| {
		NodePendingRewards::<Test>::insert(node_id(1), 500u128);
		// OPERATOR2 is operator of node_id(2), not node_id(1)
		assert_noop!(
			Rewards::claim_rewards(RuntimeOrigin::signed(OPERATOR2), node_id(1)),
			Error::<Test>::NotOperator
		);
	});
}

#[test]
fn accrue_node_reward_works() {
	new_test_ext().execute_with(|| {
		<Rewards as RewardAccruer>::accrue_node_reward(&node_id(1), 100);
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), 100);
		<Rewards as RewardAccruer>::accrue_node_reward(&node_id(1), 200);
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), 300);
	});
}

#[test]
fn distribute_and_record_era_works() {
	new_test_ext().execute_with(|| {
		let pool_before = Balances::free_balance(REWARD_POOL);

		let weights = vec![(node_id(1), 100u128), (node_id(2), 100u128)];
		let distributed = Rewards::distribute_and_record_era(
			0, 1000u128, 800u128, 200u128, 100u128, &weights, 2,
		);
		// Equal weights → 500 each
		assert_eq!(distributed, 1000);
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), 500);
		assert_eq!(NodePendingRewards::<Test>::get(node_id(2)), 500);

		// C1-fix: inflation (200) minted to RewardPool
		assert_eq!(Balances::free_balance(REWARD_POOL), pool_before + 200);

		// Era info recorded
		let info = EraRewards::<Test>::get(0).unwrap();
		assert_eq!(info.subscription_income, 800);
		assert_eq!(info.inflation_mint, 200);
		assert_eq!(info.total_distributed, 1000);
		assert_eq!(info.node_count, 2);
	});
}

#[test]
fn distribute_with_unequal_weights() {
	new_test_ext().execute_with(|| {
		let weights = vec![(node_id(1), 300u128), (node_id(2), 100u128)];
		let distributed = Rewards::distribute_and_record_era(
			1, 400u128, 300u128, 100u128, 50u128, &weights, 2,
		);
		// 300/400 * 400 = 300, 100/400 * 400 = 100
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), 300);
		assert_eq!(NodePendingRewards::<Test>::get(node_id(2)), 100);
		assert_eq!(distributed, 400);
	});
}

#[test]
fn era_reward_distributor_trait_works() {
	new_test_ext().execute_with(|| {
		let weights = vec![(node_id(1), 50u128), (node_id(2), 50u128)];
		let distributed = <Rewards as EraRewardDistributor>::distribute_and_record(
			5, 200, 150, 50, 20, &weights, 2,
		);
		assert_eq!(distributed, 200);
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), 100);
		assert_eq!(NodePendingRewards::<Test>::get(node_id(2)), 100);
	});
}

#[test]
fn prune_old_era_rewards_works() {
	new_test_ext().execute_with(|| {
		// Insert era 0..5
		for era in 0..5u64 {
			let info = EraRewardInfo {
				subscription_income: 100u128,
				inflation_mint: 50u128,
				total_distributed: 150u128,
				treasury_share: 10u128,
				node_count: 2,
			};
			EraRewards::<Test>::insert(era, info);
		}
		// MaxEraHistory=10, current_era=15 → oldest_to_keep=5, prune era 0..4
		Rewards::prune_old_era_rewards(15);
		// H1-fix: batch prune removes all 5 stale eras in one call (max 10)
		assert!(EraRewards::<Test>::get(0).is_none());
		assert!(EraRewards::<Test>::get(1).is_none());
		assert!(EraRewards::<Test>::get(2).is_none());
		assert!(EraRewards::<Test>::get(3).is_none());
		assert!(EraRewards::<Test>::get(4).is_none());
		assert_eq!(EraCleanupCursor::<Test>::get(), 5);
	});
}

#[test]
fn h1_prune_batch_bounded_at_10() {
	new_test_ext().execute_with(|| {
		// Insert eras 0..20
		for era in 0..20u64 {
			let info = EraRewardInfo {
				subscription_income: 100u128,
				inflation_mint: 50u128,
				total_distributed: 150u128,
				treasury_share: 10u128,
				node_count: 1,
			};
			EraRewards::<Test>::insert(era, info);
		}
		// MaxEraHistory=10, current_era=25 → oldest_to_keep=15
		// cursor=0, need to prune 0..14 (15 eras), but max 10 per call
		Rewards::prune_old_era_rewards(25);
		assert_eq!(EraCleanupCursor::<Test>::get(), 10);
		assert!(EraRewards::<Test>::get(9).is_none());
		assert!(EraRewards::<Test>::get(10).is_some());

		// Second call prunes remaining 5
		Rewards::prune_old_era_rewards(25);
		assert_eq!(EraCleanupCursor::<Test>::get(), 15);
		assert!(EraRewards::<Test>::get(14).is_none());
		assert!(EraRewards::<Test>::get(15).is_some());
	});
}

#[test]
fn h2_claim_fails_insufficient_pool_preserves_pending() {
	new_test_ext().execute_with(|| {
		// Set pending rewards larger than RewardPool balance
		let pool_balance = Balances::free_balance(REWARD_POOL);
		let excessive = pool_balance + 1;
		NodePendingRewards::<Test>::insert(node_id(1), excessive);

		assert_noop!(
			Rewards::claim_rewards(RuntimeOrigin::signed(OPERATOR), node_id(1)),
			Error::<Test>::RewardPoolInsufficient
		);

		// H2-fix: pending rewards NOT cleared on transfer failure
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), excessive);
		assert_eq!(NodeTotalEarned::<Test>::get(node_id(1)), 0);
	});
}

#[test]
fn h3_try_claim_orphan_rewards_works() {
	new_test_ext().execute_with(|| {
		NodePendingRewards::<Test>::insert(node_id(1), 300u128);
		let pool_before = Balances::free_balance(REWARD_POOL);
		let op_before = Balances::free_balance(OPERATOR);

		Rewards::try_claim_orphan_rewards(&node_id(1), &OPERATOR);

		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), 0);
		assert_eq!(NodeTotalEarned::<Test>::get(node_id(1)), 300);
		assert_eq!(Balances::free_balance(REWARD_POOL), pool_before - 300);
		assert_eq!(Balances::free_balance(OPERATOR), op_before + 300);
	});
}

#[test]
fn h3_try_claim_orphan_no_pending_is_noop() {
	new_test_ext().execute_with(|| {
		let pool_before = Balances::free_balance(REWARD_POOL);
		Rewards::try_claim_orphan_rewards(&node_id(1), &OPERATOR);
		// No changes
		assert_eq!(Balances::free_balance(REWARD_POOL), pool_before);
	});
}

#[test]
fn h3_try_claim_orphan_insufficient_pool_preserves_pending() {
	new_test_ext().execute_with(|| {
		let pool_balance = Balances::free_balance(REWARD_POOL);
		let excessive = pool_balance + 1;
		NodePendingRewards::<Test>::insert(node_id(1), excessive);

		// Should not panic, just log warning
		Rewards::try_claim_orphan_rewards(&node_id(1), &OPERATOR);

		// Pending rewards preserved on failure
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), excessive);
	});
}

#[test]
fn h3_orphan_reward_claimer_trait_works() {
	new_test_ext().execute_with(|| {
		NodePendingRewards::<Test>::insert(node_id(1), 200u128);
		<Rewards as OrphanRewardClaimer<u64>>::try_claim_orphan_rewards(&node_id(1), &OPERATOR);
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), 0);
		assert_eq!(NodeTotalEarned::<Test>::get(node_id(1)), 200);
	});
}

#[test]
fn m1_accrue_node_reward_emits_event() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		<Rewards as RewardAccruer>::accrue_node_reward(&node_id(1), 100);
		System::assert_last_event(
			Event::<Test>::RewardAccrued { node_id: node_id(1), amount: 100 }.into()
		);
	});
}
