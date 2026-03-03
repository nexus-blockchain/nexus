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

// ========================================================================
// Round 2 回归测试
// ========================================================================

#[test]
fn m1_r2_distribute_emits_reward_accrued_per_node() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		let weights = vec![(node_id(1), 100u128), (node_id(2), 100u128)];
		Rewards::distribute_and_record_era(0, 1000u128, 800u128, 200u128, 100u128, &weights, 2);

		// 应有 2 个 RewardAccrued 事件 (每节点一个) + 1 个 EraCompleted
		let events: Vec<_> = System::events().iter().filter_map(|e| {
			if let RuntimeEvent::Rewards(ref inner) = e.event {
				Some(inner.clone())
			} else {
				None
			}
		}).collect();

		// RewardAccrued for node 1
		assert!(events.contains(&Event::<Test>::RewardAccrued { node_id: node_id(1), amount: 500 }));
		// RewardAccrued for node 2
		assert!(events.contains(&Event::<Test>::RewardAccrued { node_id: node_id(2), amount: 500 }));
		// EraCompleted
		assert!(events.contains(&Event::<Test>::EraCompleted { era: 0, total_distributed: 1000 }));
	});
}

#[test]
fn m2_r2_rescue_stranded_rewards_works() {
	new_test_ext().execute_with(|| {
		// 模拟滞留奖励: 节点 99 不在 MockNodeConsensus (node_operator 返回 None)
		NodePendingRewards::<Test>::insert(node_id(99), 500u128);
		let pool_before = Balances::free_balance(REWARD_POOL);
		let op_before = Balances::free_balance(OPERATOR);

		// Root 救援
		assert_ok!(Rewards::rescue_stranded_rewards(
			RuntimeOrigin::root(), node_id(99), OPERATOR
		));
		assert_eq!(NodePendingRewards::<Test>::get(node_id(99)), 0);
		assert_eq!(NodeTotalEarned::<Test>::get(node_id(99)), 500);
		assert_eq!(Balances::free_balance(REWARD_POOL), pool_before - 500);
		assert_eq!(Balances::free_balance(OPERATOR), op_before + 500);
	});
}

#[test]
fn m2_r2_rescue_rejects_non_root() {
	new_test_ext().execute_with(|| {
		NodePendingRewards::<Test>::insert(node_id(99), 500u128);
		assert_noop!(
			Rewards::rescue_stranded_rewards(
				RuntimeOrigin::signed(OPERATOR), node_id(99), OPERATOR
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn m2_r2_rescue_rejects_active_node() {
	new_test_ext().execute_with(|| {
		// node_id(1) 仍在 MockNodeConsensus 中 (operator = OPERATOR)
		NodePendingRewards::<Test>::insert(node_id(1), 500u128);
		assert_noop!(
			Rewards::rescue_stranded_rewards(
				RuntimeOrigin::root(), node_id(1), OPERATOR
			),
			Error::<Test>::NodeStillActive
		);
	});
}

#[test]
fn m2_r2_rescue_rejects_no_pending() {
	new_test_ext().execute_with(|| {
		// node_id(99) 已退出, 但无滞留奖励
		assert_noop!(
			Rewards::rescue_stranded_rewards(
				RuntimeOrigin::root(), node_id(99), OPERATOR
			),
			Error::<Test>::NoPendingRewards
		);
	});
}

#[test]
fn distribute_zero_inflation_no_mint() {
	new_test_ext().execute_with(|| {
		let pool_before = Balances::free_balance(REWARD_POOL);
		let weights = vec![(node_id(1), 100u128)];
		// inflation = 0 → 不铸币
		Rewards::distribute_and_record_era(0, 500u128, 500u128, 0u128, 50u128, &weights, 1);
		// RewardPool 不增加 (无铸币)
		assert_eq!(Balances::free_balance(REWARD_POOL), pool_before);
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), 500);
	});
}

#[test]
fn distribute_empty_weights_zero_distributed() {
	new_test_ext().execute_with(|| {
		let pool_before = Balances::free_balance(REWARD_POOL);
		// 空权重列表 → total_weight=0, 不分配
		let weights: Vec<(NodeId, u128)> = vec![];
		let distributed = Rewards::distribute_and_record_era(
			0, 500u128, 400u128, 100u128, 50u128, &weights, 0,
		);
		assert_eq!(distributed, 0);
		// 通胀仍铸币到 pool
		assert_eq!(Balances::free_balance(REWARD_POOL), pool_before + 100);
		// Era 记录 total_distributed = 0
		let info = EraRewards::<Test>::get(0).unwrap();
		assert_eq!(info.total_distributed, 0);
		assert_eq!(info.inflation_mint, 100);
	});
}

#[test]
fn accrue_zero_amount_is_noop() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		<Rewards as RewardAccruer>::accrue_node_reward(&node_id(1), 0);
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), 0);
		// 零金额不应发出事件
		let reward_events: Vec<_> = System::events().into_iter().filter(|e| {
			matches!(e.event, RuntimeEvent::Rewards(Event::<Test>::RewardAccrued { .. }))
		}).collect();
		assert!(reward_events.is_empty());
	});
}

// ========================================================================
// Round 3 回归测试
// ========================================================================

#[test]
fn m1_r3_claim_event_includes_recipient() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		NodePendingRewards::<Test>::insert(node_id(1), 500u128);
		assert_ok!(Rewards::claim_rewards(RuntimeOrigin::signed(OPERATOR), node_id(1)));
		System::assert_last_event(
			Event::<Test>::RewardsClaimed {
				node_id: node_id(1),
				recipient: OPERATOR,
				amount: 500,
			}.into()
		);
	});
}

#[test]
fn m1_r3_rescue_event_includes_recipient() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		NodePendingRewards::<Test>::insert(node_id(99), 300u128);
		// Root 救援到 OTHER 账户
		assert_ok!(Rewards::rescue_stranded_rewards(
			RuntimeOrigin::root(), node_id(99), OTHER
		));
		System::assert_last_event(
			Event::<Test>::RewardsClaimed {
				node_id: node_id(99),
				recipient: OTHER,
				amount: 300,
			}.into()
		);
	});
}

#[test]
fn m2_r3_orphan_claim_failure_emits_event() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		let pool_balance = Balances::free_balance(REWARD_POOL);
		let excessive = pool_balance + 1;
		NodePendingRewards::<Test>::insert(node_id(1), excessive);

		Rewards::try_claim_orphan_rewards(&node_id(1), &OPERATOR);

		// M2-R3: 应发射 OrphanRewardClaimFailed 事件
		System::assert_last_event(
			Event::<Test>::OrphanRewardClaimFailed {
				node_id: node_id(1),
				amount: excessive,
			}.into()
		);
		// pending 保持不变
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), excessive);
	});
}

#[test]
fn m3_r3_distribute_era_skips_duplicate() {
	new_test_ext().execute_with(|| {
		let weights = vec![(node_id(1), 100u128)];
		// 第一次分配
		let d1 = Rewards::distribute_and_record_era(
			0, 500u128, 400u128, 100u128, 50u128, &weights, 1,
		);
		assert_eq!(d1, 500);
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), 500);

		let pool_after_first = Balances::free_balance(REWARD_POOL);

		// 第二次分配同一 era → 应被跳过, 不重复铸币/分配
		let d2 = Rewards::distribute_and_record_era(
			0, 500u128, 400u128, 100u128, 50u128, &weights, 1,
		);
		assert_eq!(d2, 0);
		// 不应重复铸币
		assert_eq!(Balances::free_balance(REWARD_POOL), pool_after_first);
		// 不应重复分配
		assert_eq!(NodePendingRewards::<Test>::get(node_id(1)), 500);
	});
}
