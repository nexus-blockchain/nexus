use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use pallet_grouprobot_primitives::*;

#[test]
fn claim_rewards_works() {
	new_test_ext().execute_with(|| {
		// Seed pending rewards
		NodePendingRewards::<Test>::insert(node_id(1), 500u128);

		assert_ok!(Rewards::claim_rewards(RuntimeOrigin::signed(OPERATOR), node_id(1)));
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), 0);
		assert_eq!(NodeTotalEarned::<Test>::get(node_id(1)), 500);
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
		let weights = vec![(node_id(1), 100u128), (node_id(2), 100u128)];
		let distributed = Rewards::distribute_and_record_era(
			0, 1000u128, 800u128, 200u128, 100u128, &weights, 2,
		);
		// Equal weights → 500 each
		assert_eq!(distributed, 1000);
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), 500);
		assert_eq!(NodePendingRewards::<Test>::get(node_id(2)), 500);

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
		// MaxEraHistory=10, current_era=15 → prune era 0
		Rewards::prune_old_era_rewards(15);
		assert!(EraRewards::<Test>::get(0).is_none());
		// era 1 still exists (only one pruned per call)
		assert!(EraRewards::<Test>::get(1).is_some());
	});
}
