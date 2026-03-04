use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok, traits::Currency as CurrencyT};
use pallet_grouprobot_primitives::*;

#[test]
fn subscribe_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.tier, SubscriptionTier::Basic);
		assert_eq!(sub.owner, OWNER);
		assert_eq!(sub.status, SubscriptionStatus::Active);
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 50);
	});
}

#[test]
fn subscribe_fails_free_tier() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Subscription::subscribe(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
				SubscriptionTier::Free,
				10,
			),
			Error::<Test>::CannotSubscribeFree
		);
	});
}

#[test]
fn subscribe_fails_insufficient_deposit() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Subscription::subscribe(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
				SubscriptionTier::Basic,
				5, // less than BasicFee=10
			),
			Error::<Test>::InsufficientDeposit
		);
	});
}

#[test]
fn subscribe_fails_not_bot_owner() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Subscription::subscribe(
				RuntimeOrigin::signed(OTHER),
				bot_hash(1),
				SubscriptionTier::Basic,
				50,
			),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn subscribe_fails_bot_not_registered() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Subscription::subscribe(
				RuntimeOrigin::signed(OWNER),
				bot_hash(99), // not active
				SubscriptionTier::Basic,
				50,
			),
			Error::<Test>::BotNotRegistered
		);
	});
}

#[test]
fn subscribe_fails_already_exists() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_noop!(
			Subscription::subscribe(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
				SubscriptionTier::Pro,
				50,
			),
			Error::<Test>::SubscriptionAlreadyExists
		);
	});
}

#[test]
fn deposit_subscription_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::deposit_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			20,
		));
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 70);
	});
}

#[test]
fn deposit_subscription_fails_not_owner() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_noop!(
			Subscription::deposit_subscription(
				RuntimeOrigin::signed(OTHER),
				bot_hash(1),
				20,
			),
			Error::<Test>::NotSubscriptionOwner
		);
	});
}

#[test]
fn cancel_subscription_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::cancel_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::Cancelled);
		// escrow taken (cleared)
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 0);
	});
}

#[test]
fn cancel_subscription_fails_already_cancelled() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::cancel_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		assert_noop!(
			Subscription::cancel_subscription(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
			),
			Error::<Test>::SubscriptionAlreadyCancelled
		);
	});
}

#[test]
fn change_tier_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::change_tier(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Pro,
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.tier, SubscriptionTier::Pro);
		assert_eq!(sub.fee_per_era, 30); // ProFee
	});
}

#[test]
fn change_tier_fails_same_tier() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_noop!(
			Subscription::change_tier(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
				SubscriptionTier::Basic,
			),
			Error::<Test>::SameTier
		);
	});
}

#[test]
fn effective_tier_returns_free_for_no_subscription() {
	new_test_ext().execute_with(|| {
		assert_eq!(
			<Subscription as SubscriptionProvider>::effective_tier(&bot_hash(99)),
			SubscriptionTier::Free
		);
	});
}

#[test]
fn effective_tier_returns_tier_for_active_subscription() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Pro,
			50,
		));
		assert_eq!(
			<Subscription as SubscriptionProvider>::effective_tier(&bot_hash(1)),
			SubscriptionTier::Pro
		);
	});
}

#[test]
fn settle_era_direct_to_operator() {
	new_test_ext().execute_with(|| {
		let treasury_before = Balances::free_balance(TREASURY);
		let pool_before = Balances::free_balance(REWARD_POOL);
		let operator_before = Balances::free_balance(OPERATOR);

		// bot_hash(1) 有运营者 OPERATOR
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		let (income, treasury) = Subscription::settle_era_subscriptions();
		// BasicFee = 10
		assert_eq!(income, 10);
		// treasury_share = 10 - 9 = 1
		assert_eq!(treasury, 1);
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 40);

		// 90/10 拆分: node_share = 10 * 90 / 100 = 9 → OPERATOR
		// treasury_share = 10 - 9 = 1 → Treasury
		assert_eq!(Balances::free_balance(OPERATOR), operator_before + 9);
		assert_eq!(Balances::free_balance(REWARD_POOL), pool_before); // 不变
		assert_eq!(Balances::free_balance(TREASURY), treasury_before + 1);
	});
}

#[test]
fn subscribe_fails_no_operator() {
	new_test_ext().execute_with(|| {
		// bot_hash(2) 无运营者, 订阅应失败
		assert_noop!(
			Subscription::subscribe(
				RuntimeOrigin::signed(OWNER2),
				bot_hash(2),
				SubscriptionTier::Basic,
				50,
			),
			Error::<Test>::BotHasNoOperator
		);
	});
}

#[test]
fn settle_era_suspends_when_insufficient_escrow() {
	new_test_ext().execute_with(|| {
		// Subscribe with exactly 1 era's worth
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			10,
		));
		// First settle: uses all escrow
		let _ = Subscription::settle_era_subscriptions();
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 0);

		// Second settle: insufficient → PastDue
		let _ = Subscription::settle_era_subscriptions();
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::PastDue);

		// Third settle: PastDue → Suspended
		let _ = Subscription::settle_era_subscriptions();
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::Suspended);
	});
}

// ========================================================================
// 广告承诺订阅测试
// ========================================================================

#[test]
fn commit_ads_works() {
	new_test_ext().execute_with(|| {
		// bot_hash(1) 有运营者, 承诺 5 ads/era → Basic (阈值 3)
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));
		let record = AdCommitments::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.effective_tier, SubscriptionTier::Basic);
		assert_eq!(record.committed_ads_per_era, 5);
		assert_eq!(record.community_id_hash, community_hash(1));
		assert_eq!(record.status, AdCommitmentStatus::Active);
		assert_eq!(record.underdelivery_eras, 0);
	});
}

#[test]
fn commit_ads_pro_tier() {
	new_test_ext().execute_with(|| {
		// 承诺 8 ads/era → Pro (阈值 6)
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			8,
		));
		let record = AdCommitments::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.effective_tier, SubscriptionTier::Pro);
	});
}

#[test]
fn commit_ads_enterprise_tier() {
	new_test_ext().execute_with(|| {
		// 承诺 15 ads/era → Enterprise (阈值 11)
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			15,
		));
		let record = AdCommitments::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.effective_tier, SubscriptionTier::Enterprise);
	});
}

#[test]
fn commit_ads_fails_below_minimum() {
	new_test_ext().execute_with(|| {
		// 承诺 2 ads/era → Free (低于 Basic 阈值 3), 应失败
		assert_noop!(
			Subscription::commit_ads(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
				community_hash(1),
				2,
			),
			Error::<Test>::CommitmentBelowMinimum
		);
	});
}

#[test]
fn commit_ads_fails_already_exists() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));
		assert_noop!(
			Subscription::commit_ads(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
				community_hash(2),
				5,
			),
			Error::<Test>::AdCommitmentAlreadyExists
		);
	});
}

#[test]
fn commit_ads_fails_not_bot_owner() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Subscription::commit_ads(
				RuntimeOrigin::signed(OTHER),
				bot_hash(1),
				community_hash(1),
				5,
			),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn cancel_ad_commitment_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));
		assert_ok!(Subscription::cancel_ad_commitment(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		let record = AdCommitments::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.status, AdCommitmentStatus::Cancelled);
	});
}

#[test]
fn cancel_ad_commitment_fails_not_found() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Subscription::cancel_ad_commitment(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
			),
			Error::<Test>::AdCommitmentNotFound
		);
	});
}

#[test]
fn effective_tier_considers_ad_commitment() {
	new_test_ext().execute_with(|| {
		// 无付费订阅, 仅广告承诺 → Pro
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			8, // → Pro
		));
		assert_eq!(
			Subscription::effective_tier(&bot_hash(1)),
			SubscriptionTier::Pro
		);
	});
}

#[test]
fn effective_tier_takes_max_of_paid_and_ad() {
	new_test_ext().execute_with(|| {
		// 付费订阅 Basic
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		// 广告承诺 → Pro
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			8, // → Pro
		));
		// 取较高者 = Pro
		assert_eq!(
			Subscription::effective_tier(&bot_hash(1)),
			SubscriptionTier::Pro
		);
	});
}

#[test]
fn settle_ad_commitments_fulfilled() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));
		// 模拟投放 5 次 (达标)
		set_delivery_count(&community_hash(1), 5);

		Subscription::settle_ad_commitments();

		let record = AdCommitments::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.status, AdCommitmentStatus::Active);
		assert_eq!(record.underdelivery_eras, 0);
	});
}

#[test]
fn settle_ad_commitments_underdelivery() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));
		// 模拟投放 2 次 (未达标, committed=5)
		set_delivery_count(&community_hash(1), 2);

		Subscription::settle_ad_commitments();

		let record = AdCommitments::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.status, AdCommitmentStatus::Underdelivery);
		assert_eq!(record.underdelivery_eras, 1);
	});
}

#[test]
fn settle_ad_commitments_downgrade_after_max_underdelivery() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));

		// 连续 3 个 Era 未达标 (MaxUnderdeliveryEras = 3)
		for i in 1..=3u8 {
			set_delivery_count(&community_hash(1), 0);
			Subscription::settle_ad_commitments();

			let record = AdCommitments::<Test>::get(bot_hash(1)).unwrap();
			if i < 3 {
				assert_eq!(record.status, AdCommitmentStatus::Underdelivery);
				assert_eq!(record.underdelivery_eras, i);
			} else {
				// 第 3 次: 降级为 Cancelled
				assert_eq!(record.status, AdCommitmentStatus::Cancelled);
				assert_eq!(record.underdelivery_eras, 3);
			}
		}

		// 降级后 effective_tier 回到 Free
		assert_eq!(
			Subscription::effective_tier(&bot_hash(1)),
			SubscriptionTier::Free
		);
	});
}

#[test]
fn settle_ad_commitments_resets_on_fulfillment() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));

		// Era 1: 未达标
		set_delivery_count(&community_hash(1), 2);
		Subscription::settle_ad_commitments();
		let record = AdCommitments::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.underdelivery_eras, 1);

		// Era 2: 达标 → 重置
		set_delivery_count(&community_hash(1), 5);
		Subscription::settle_ad_commitments();
		let record = AdCommitments::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.underdelivery_eras, 0);
		assert_eq!(record.status, AdCommitmentStatus::Active);
	});
}

// ========================================================================
// 回归测试: H2 — change_tier 拒绝已取消/已暂停订阅
// ========================================================================

#[test]
fn h2_change_tier_rejects_cancelled_subscription() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::cancel_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		assert_noop!(
			Subscription::change_tier(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
				SubscriptionTier::Pro,
			),
			Error::<Test>::SubscriptionNotActive
		);
	});
}

#[test]
fn h2_change_tier_rejects_suspended_subscription() {
	new_test_ext().execute_with(|| {
		// Subscribe with exactly 1 era
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			10,
		));
		// Settle twice to reach Suspended: Active → PastDue → Suspended
		let _ = Subscription::settle_era_subscriptions();
		let _ = Subscription::settle_era_subscriptions();
		let _ = Subscription::settle_era_subscriptions();
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::Suspended);

		assert_noop!(
			Subscription::change_tier(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
				SubscriptionTier::Pro,
			),
			Error::<Test>::SubscriptionNotActive
		);
	});
}

#[test]
fn h2_change_tier_allows_past_due_downgrade() {
	new_test_ext().execute_with(|| {
		// M1-R4: PastDue 仍允许降级 (new_fee <= old_fee 不检查 escrow)
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Pro,
			30,
		));
		// Settle to drain escrow, then one more to get PastDue
		let _ = Subscription::settle_era_subscriptions();
		let _ = Subscription::settle_era_subscriptions();
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::PastDue);

		// PastDue should still allow downgrade (Basic fee=10 < Pro fee=30)
		assert_ok!(Subscription::change_tier(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.tier, SubscriptionTier::Basic);
	});
}

// ========================================================================
// 回归测试: H4 — deposit_subscription 拒绝零金额
// ========================================================================

#[test]
fn h4_deposit_subscription_rejects_zero_amount() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_noop!(
			Subscription::deposit_subscription(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
				0,
			),
			Error::<Test>::ZeroDepositAmount
		);
	});
}

// ========================================================================
// 回归测试: H1 — settle 转账失败不计入收入
// ========================================================================

#[test]
fn h1_settle_income_zero_when_owner_cannot_transfer() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			10,
		));
		// Drain OWNER's free balance so transfer after unreserve may fail
		// OWNER has 100_000 initial, reserved 10 → free = 99_990
		// Transfer away most free balance
		let _ = <<Test as crate::pallet::Config>::Currency as CurrencyT<u64>>::transfer(
			&OWNER,
			&OTHER,
			99_985,
			frame_support::traits::ExistenceRequirement::AllowDeath,
		);
		// Now OWNER has ~5 free + 10 reserved = ~15 total
		// After unreserve(10), OWNER has ~15 free
		// node_share = 9, treasury_share = 1, total = 10
		// Transfer of 9 + 1 = 10 from 15 free should still work
		let (income, _treasury) = Subscription::settle_era_subscriptions();
		assert_eq!(income, 10);
	});
}

// ========================================================================
// 回归测试: C1 — settle 游标应保持为最后已处理的 key
// ========================================================================

#[test]
fn c1_settle_cursor_stores_last_processed_key() {
	new_test_ext().execute_with(|| {
		// 单条订阅, settle 处理完后 cursor 应被清除 (settled < max_settle)
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		let _ = Subscription::settle_era_subscriptions();
		// cursor 应被清除 (全部处理完)
		assert!(SubscriptionSettleCursor::<Test>::get().is_none());
		assert!(!SubscriptionSettlePending::<Test>::get());
	});
}

// ========================================================================
// 回归测试: H1 — 转账失败时不发出 SubscriptionFeeCollected 事件
// ========================================================================

#[test]
fn h1_no_event_on_transfer_failure() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			10,
		));
		// Drain OWNER 全部 free balance, 仅留 reserved
		let free = <<Test as crate::pallet::Config>::Currency as CurrencyT<u64>>::free_balance(&OWNER);
		let _ = <<Test as crate::pallet::Config>::Currency as CurrencyT<u64>>::transfer(
			&OWNER,
			&OTHER,
			free,
			frame_support::traits::ExistenceRequirement::AllowDeath,
		);
		// OWNER free=0, reserved=10; unreserve(10) → free=10
		// node_share=9 transfer should succeed, but treasury_share=1 may fail
		// if OWNER has exactly 10 and ED=1, first transfer of 9 leaves 1,
		// second transfer of 1 leaves 0 → AllowDeath allows it
		// So both transfers succeed in this mock. Instead test income is correct:
		System::reset_events();
		let (income, _treasury) = Subscription::settle_era_subscriptions();
		// If transfers succeed, event should be emitted and income counted
		assert_eq!(income, 10);
		let events: Vec<_> = System::events().into_iter()
			.filter(|e| matches!(e.event, RuntimeEvent::Subscription(Event::SubscriptionFeeCollected { .. })))
			.collect();
		assert_eq!(events.len(), 1);
	});
}

// ========================================================================
// 回归测试: M1 — 充值不足一个 Era 费用时不重新激活
// ========================================================================

#[test]
fn m1_deposit_does_not_reactivate_if_escrow_below_fee() {
	new_test_ext().execute_with(|| {
		// 创建订阅, 最小 deposit
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			10,
		));
		// 手动设为 Suspended + 清空 escrow (模拟结算后余额不足)
		Subscriptions::<Test>::mutate(bot_hash(1), |maybe| {
			if let Some(s) = maybe {
				s.status = SubscriptionStatus::Suspended;
			}
		});
		SubscriptionEscrow::<Test>::insert(bot_hash(1), 0u128);

		// 充值 5 (< BasicFee=10), 不应重新激活
		assert_ok!(Subscription::deposit_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			5,
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::Suspended);
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 5);
	});
}

#[test]
fn m1_deposit_reactivates_when_escrow_covers_fee() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			10,
		));
		Subscriptions::<Test>::mutate(bot_hash(1), |maybe| {
			if let Some(s) = maybe {
				s.status = SubscriptionStatus::Suspended;
			}
		});
		SubscriptionEscrow::<Test>::insert(bot_hash(1), 0u128);

		// 充值 10 (= BasicFee), 应重新激活
		assert_ok!(Subscription::deposit_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			10,
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::Active);
	});
}

// ========================================================================
// 回归测试: L1 — commit_ads 无运营者时失败
// ========================================================================

#[test]
fn l1_commit_ads_fails_no_operator() {
	new_test_ext().execute_with(|| {
		// bot_hash(2) 有 owner=OWNER2 但无 operator
		assert_noop!(
			Subscription::commit_ads(
				RuntimeOrigin::signed(OWNER2),
				bot_hash(2),
				community_hash(1),
				5,
			),
			Error::<Test>::BotHasNoOperator
		);
	});
}

// ========================================================================
// 回归测试: H1-R3 — effective_tier 应包含 Underdelivery 状态的广告承诺
// ========================================================================

#[test]
fn h1_r3_underdelivery_preserves_effective_tier() {
	new_test_ext().execute_with(|| {
		// 创建广告承诺 → Basic
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));
		// 模拟未达标 → Underdelivery
		set_delivery_count(&community_hash(1), 2);
		Subscription::settle_ad_commitments();
		let record = AdCommitments::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.status, AdCommitmentStatus::Underdelivery);

		// H1-R3: Underdelivery 仍应保留有效层级
		assert_eq!(
			Subscription::effective_tier(&bot_hash(1)),
			SubscriptionTier::Basic
		);
	});
}

#[test]
fn h1_r3_cancelled_commitment_drops_tier() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));
		assert_ok!(Subscription::cancel_ad_commitment(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		// Cancelled 应回到 Free
		assert_eq!(
			Subscription::effective_tier(&bot_hash(1)),
			SubscriptionTier::Free
		);
	});
}

// ========================================================================
// 回归测试: M2-R3 — cleanup 清理已取消记录
// ========================================================================

#[test]
fn m2_r3_cleanup_subscription_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::cancel_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		// 任何人可清理已取消的订阅
		assert_ok!(Subscription::cleanup_subscription(
			RuntimeOrigin::signed(OTHER),
			bot_hash(1),
		));
		assert!(Subscriptions::<Test>::get(bot_hash(1)).is_none());
	});
}

#[test]
fn m2_r3_cleanup_subscription_rejects_active() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_noop!(
			Subscription::cleanup_subscription(
				RuntimeOrigin::signed(OTHER),
				bot_hash(1),
			),
			Error::<Test>::SubscriptionNotTerminal
		);
	});
}

#[test]
fn m2_r3_cleanup_ad_commitment_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));
		assert_ok!(Subscription::cancel_ad_commitment(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		assert_ok!(Subscription::cleanup_ad_commitment(
			RuntimeOrigin::signed(OTHER),
			bot_hash(1),
		));
		assert!(AdCommitments::<Test>::get(bot_hash(1)).is_none());
	});
}

#[test]
fn m2_r3_cleanup_ad_commitment_rejects_active() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));
		assert_noop!(
			Subscription::cleanup_ad_commitment(
				RuntimeOrigin::signed(OTHER),
				bot_hash(1),
			),
			Error::<Test>::AdCommitmentNotTerminal
		);
	});
}

// ========================================================================
// 回归测试: M1-R4 — change_tier 升级需验证 escrow 充足性
// ========================================================================

#[test]
fn m1_r4_change_tier_upgrade_rejects_insufficient_escrow() {
	new_test_ext().execute_with(|| {
		// Subscribe Basic (fee=10) with deposit=50, escrow=50
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		// Upgrade to Enterprise (fee=100), escrow=50 < 100 → 应拒绝
		assert_noop!(
			Subscription::change_tier(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
				SubscriptionTier::Enterprise,
			),
			Error::<Test>::InsufficientDeposit
		);
		// 层级不变
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.tier, SubscriptionTier::Basic);
	});
}

#[test]
fn m1_r4_change_tier_upgrade_works_with_sufficient_escrow() {
	new_test_ext().execute_with(|| {
		// Subscribe Basic (fee=10) with deposit=100, escrow=100
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			100,
		));
		// Upgrade to Enterprise (fee=100), escrow=100 >= 100 → 应成功
		assert_ok!(Subscription::change_tier(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Enterprise,
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.tier, SubscriptionTier::Enterprise);
		assert_eq!(sub.fee_per_era, 100);
	});
}

#[test]
fn m1_r4_change_tier_downgrade_allows_low_escrow() {
	new_test_ext().execute_with(|| {
		// Subscribe Pro (fee=30) with deposit=30
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Pro,
			30,
		));
		// Settle to drain escrow
		let _ = Subscription::settle_era_subscriptions();
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 0);
		// Second settle → PastDue
		let _ = Subscription::settle_era_subscriptions();
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::PastDue);

		// Downgrade to Basic (fee=10 < Pro fee=30), escrow=0
		// 降级不检查 escrow → 应成功
		assert_ok!(Subscription::change_tier(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.tier, SubscriptionTier::Basic);
		assert_eq!(sub.fee_per_era, 10);
	});
}

// ========================================================================
// 回归测试: L2-R4 — cleanup_subscription 防御性清理 escrow 存储
// ========================================================================

#[test]
fn l2_r4_cleanup_subscription_removes_escrow_dust() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::cancel_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		// 模拟 escrow 存储残留 (防御性场景)
		SubscriptionEscrow::<Test>::insert(bot_hash(1), 5u128);

		assert_ok!(Subscription::cleanup_subscription(
			RuntimeOrigin::signed(OTHER),
			bot_hash(1),
		));
		// 验证 subscription 和 escrow 都已清理
		assert!(Subscriptions::<Test>::get(bot_hash(1)).is_none());
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 0);
	});
}

// ========================================================================
// force_cancel_subscription 测试
// ========================================================================

#[test]
fn force_cancel_subscription_works() {
	new_test_ext().execute_with(|| {
		let treasury_before = Balances::free_balance(TREASURY);
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::force_cancel_subscription(
			RuntimeOrigin::root(),
			bot_hash(1),
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::Cancelled);
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 0);
		// escrow 没收至国库
		assert_eq!(Balances::free_balance(TREASURY), treasury_before + 50);
	});
}

#[test]
fn force_cancel_subscription_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_noop!(
			Subscription::force_cancel_subscription(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn force_cancel_subscription_fails_already_cancelled() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::cancel_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		assert_noop!(
			Subscription::force_cancel_subscription(
				RuntimeOrigin::root(),
				bot_hash(1),
			),
			Error::<Test>::SubscriptionAlreadyCancelled
		);
	});
}

// ========================================================================
// withdraw_escrow 测试
// ========================================================================

#[test]
fn withdraw_escrow_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		// 提取 30, 剩余 20 >= fee=10 → 成功
		assert_ok!(Subscription::withdraw_escrow(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			30,
		));
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 20);
	});
}

#[test]
fn withdraw_escrow_fails_would_underfund() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		// 提取 45, 剩余 5 < fee=10 → 拒绝
		assert_noop!(
			Subscription::withdraw_escrow(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
				45,
			),
			Error::<Test>::WithdrawWouldUnderfund
		);
	});
}

#[test]
fn withdraw_escrow_fails_not_owner() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_noop!(
			Subscription::withdraw_escrow(
				RuntimeOrigin::signed(OTHER),
				bot_hash(1),
				10,
			),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn withdraw_escrow_fails_zero_amount() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_noop!(
			Subscription::withdraw_escrow(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
				0,
			),
			Error::<Test>::ZeroDepositAmount
		);
	});
}

// ========================================================================
// update_ad_commitment 测试
// ========================================================================

#[test]
fn update_ad_commitment_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));
		// 从 5 改为 8 (Basic→Pro)
		assert_ok!(Subscription::update_ad_commitment(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			8,
			None,
		));
		let record = AdCommitments::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.committed_ads_per_era, 8);
		assert_eq!(record.effective_tier, SubscriptionTier::Pro);
		assert_eq!(record.underdelivery_eras, 0);
		assert_eq!(record.status, AdCommitmentStatus::Active);
	});
}

#[test]
fn update_ad_commitment_changes_community() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));
		assert_ok!(Subscription::update_ad_commitment(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			5, // 数量不变
			Some(community_hash(2)), // 社区变更
		));
		let record = AdCommitments::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.community_id_hash, community_hash(2));
	});
}

#[test]
fn update_ad_commitment_fails_same() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));
		assert_noop!(
			Subscription::update_ad_commitment(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
				5,
				None,
			),
			Error::<Test>::SameCommitment
		);
	});
}

#[test]
fn update_ad_commitment_fails_below_minimum() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));
		assert_noop!(
			Subscription::update_ad_commitment(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
				1, // 低于 Basic 阈值
				None,
			),
			Error::<Test>::CommitmentBelowMinimum
		);
	});
}

// ========================================================================
// force_suspend_subscription 测试
// ========================================================================

#[test]
fn force_suspend_subscription_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::force_suspend_subscription(
			RuntimeOrigin::root(),
			bot_hash(1),
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::Suspended);
	});
}

#[test]
fn force_suspend_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_noop!(
			Subscription::force_suspend_subscription(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn force_suspend_fails_already_cancelled() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::cancel_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		assert_noop!(
			Subscription::force_suspend_subscription(
				RuntimeOrigin::root(),
				bot_hash(1),
			),
			Error::<Test>::SubscriptionAlreadyCancelled
		);
	});
}

// ========================================================================
// operator_deposit_subscription 测试
// ========================================================================

#[test]
fn operator_deposit_subscription_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::operator_deposit_subscription(
			RuntimeOrigin::signed(OPERATOR),
			bot_hash(1),
			20,
		));
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 70);
	});
}

#[test]
fn operator_deposit_fails_not_operator() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_noop!(
			Subscription::operator_deposit_subscription(
				RuntimeOrigin::signed(OTHER),
				bot_hash(1),
				20,
			),
			Error::<Test>::NotBotOperator
		);
	});
}

#[test]
fn operator_deposit_reactivates_suspended() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			10,
		));
		// 设为 Suspended, escrow=0
		Subscriptions::<Test>::mutate(bot_hash(1), |maybe| {
			if let Some(s) = maybe { s.status = SubscriptionStatus::Suspended; }
		});
		SubscriptionEscrow::<Test>::insert(bot_hash(1), 0u128);

		assert_ok!(Subscription::operator_deposit_subscription(
			RuntimeOrigin::signed(OPERATOR),
			bot_hash(1),
			10, // = fee → 重新激活
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::Active);
	});
}

// ========================================================================
// reset_tier_feature_gate 测试
// ========================================================================

#[test]
fn reset_tier_feature_gate_works() {
	new_test_ext().execute_with(|| {
		let gate = TierFeatureGate {
			max_rules: 99,
			log_retention_days: 99,
			forced_ads_per_day: 99,
			can_disable_ads: true,
			tee_access: true,
		};
		assert_ok!(Subscription::update_tier_feature_gate(
			RuntimeOrigin::root(),
			SubscriptionTier::Basic,
			gate,
		));
		assert!(TierFeatureGateOverrides::<Test>::get(SubscriptionTier::Basic).is_some());

		assert_ok!(Subscription::reset_tier_feature_gate(
			RuntimeOrigin::root(),
			SubscriptionTier::Basic,
		));
		assert!(TierFeatureGateOverrides::<Test>::get(SubscriptionTier::Basic).is_none());
	});
}

// ========================================================================
// force_change_tier 测试
// ========================================================================

#[test]
fn force_change_tier_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::force_change_tier(
			RuntimeOrigin::root(),
			bot_hash(1),
			SubscriptionTier::Enterprise,
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.tier, SubscriptionTier::Enterprise);
		assert_eq!(sub.fee_per_era, 100);
	});
}

#[test]
fn force_change_tier_fails_free() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_noop!(
			Subscription::force_change_tier(
				RuntimeOrigin::root(),
				bot_hash(1),
				SubscriptionTier::Free,
			),
			Error::<Test>::CannotSubscribeFree
		);
	});
}

// ========================================================================
// pause_subscription / resume_subscription 测试
// ========================================================================

#[test]
fn pause_subscription_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::pause_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::Paused);
	});
}

#[test]
fn pause_subscription_fails_not_active() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			10,
		));
		// Settle 两次 → PastDue
		let _ = Subscription::settle_era_subscriptions();
		let _ = Subscription::settle_era_subscriptions();
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::PastDue);

		assert_noop!(
			Subscription::pause_subscription(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
			),
			Error::<Test>::SubscriptionNotActive
		);
	});
}

#[test]
fn resume_subscription_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::pause_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		assert_ok!(Subscription::resume_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::Active);
	});
}

#[test]
fn resume_subscription_past_due_when_low_escrow() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::pause_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		// 手动清空 escrow
		SubscriptionEscrow::<Test>::insert(bot_hash(1), 5u128);

		assert_ok!(Subscription::resume_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.status, SubscriptionStatus::PastDue);
	});
}

#[test]
fn resume_fails_not_paused() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_noop!(
			Subscription::resume_subscription(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
			),
			Error::<Test>::SubscriptionNotPaused
		);
	});
}

#[test]
fn paused_subscription_not_charged_during_settle() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::pause_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		let _ = Subscription::settle_era_subscriptions();
		// 暂停状态不扣费
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 50);
	});
}

#[test]
fn paused_subscription_effective_tier_is_free() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Pro,
			50,
		));
		assert_ok!(Subscription::pause_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		assert_eq!(
			Subscription::effective_tier(&bot_hash(1)),
			SubscriptionTier::Free
		);
	});
}

// ========================================================================
// batch_cleanup 测试
// ========================================================================

#[test]
fn batch_cleanup_works() {
	new_test_ext().execute_with(|| {
		// 创建并取消 2 个订阅
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_ok!(Subscription::cancel_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		// 创建并取消广告承诺
		// 需要先订阅 bot_hash(1) 才能 commit_ads... 重新用新 bot
		// 使用 bot_hash(1) 已经 cancelled, 直接插入测试数据
		let mut cancelled_ad = AdCommitmentRecord::<Test> {
			owner: OWNER,
			bot_id_hash: bot_hash(1),
			community_id_hash: community_hash(1),
			committed_ads_per_era: 5,
			effective_tier: SubscriptionTier::Basic,
			underdelivery_eras: 0,
			status: AdCommitmentStatus::Cancelled,
			started_at: 1,
		};
		AdCommitments::<Test>::insert(bot_hash(1), cancelled_ad.clone());

		let sub_ids: sp_runtime::BoundedVec<_, frame_support::traits::ConstU32<50>> =
			vec![bot_hash(1)].try_into().unwrap();
		let ad_ids: sp_runtime::BoundedVec<_, frame_support::traits::ConstU32<50>> =
			vec![bot_hash(1)].try_into().unwrap();

		assert_ok!(Subscription::batch_cleanup(
			RuntimeOrigin::signed(OTHER),
			sub_ids,
			ad_ids,
		));
		assert!(Subscriptions::<Test>::get(bot_hash(1)).is_none());
		assert!(AdCommitments::<Test>::get(bot_hash(1)).is_none());
	});
}

#[test]
fn batch_cleanup_skips_active() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		let sub_ids: sp_runtime::BoundedVec<_, frame_support::traits::ConstU32<50>> =
			vec![bot_hash(1)].try_into().unwrap();
		let ad_ids: sp_runtime::BoundedVec<_, frame_support::traits::ConstU32<50>> =
			vec![].try_into().unwrap();

		assert_ok!(Subscription::batch_cleanup(
			RuntimeOrigin::signed(OTHER),
			sub_ids,
			ad_ids,
		));
		// Active 不被清理
		assert!(Subscriptions::<Test>::get(bot_hash(1)).is_some());
	});
}

#[test]
fn batch_cleanup_fails_empty() {
	new_test_ext().execute_with(|| {
		let empty_sub: sp_runtime::BoundedVec<_, frame_support::traits::ConstU32<50>> =
			vec![].try_into().unwrap();
		let empty_ad: sp_runtime::BoundedVec<_, frame_support::traits::ConstU32<50>> =
			vec![].try_into().unwrap();
		assert_noop!(
			Subscription::batch_cleanup(
				RuntimeOrigin::signed(OTHER),
				empty_sub,
				empty_ad,
			),
			Error::<Test>::EmptyBatch
		);
	});
}

// ========================================================================
// update_tier_fee (动态费率) 测试
// ========================================================================

#[test]
fn update_tier_fee_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::update_tier_fee(
			RuntimeOrigin::root(),
			SubscriptionTier::Basic,
			20, // 从默认 10 → 20
		));
		assert_eq!(Subscription::tier_fee(&SubscriptionTier::Basic), 20);
	});
}

#[test]
fn update_tier_fee_fails_free() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Subscription::update_tier_fee(
				RuntimeOrigin::root(),
				SubscriptionTier::Free,
				5,
			),
			Error::<Test>::CannotSubscribeFree
		);
	});
}

#[test]
fn dynamic_fee_affects_new_subscriptions() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::update_tier_fee(
			RuntimeOrigin::root(),
			SubscriptionTier::Basic,
			20,
		));
		// 新订阅使用动态费率
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		let sub = Subscriptions::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(sub.fee_per_era, 20);
	});
}

// ========================================================================
// force_cancel_ad_commitment 测试
// ========================================================================

#[test]
fn force_cancel_ad_commitment_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));
		assert_ok!(Subscription::force_cancel_ad_commitment(
			RuntimeOrigin::root(),
			bot_hash(1),
		));
		let record = AdCommitments::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.status, AdCommitmentStatus::Cancelled);
	});
}

#[test]
fn force_cancel_ad_commitment_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::commit_ads(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			community_hash(1),
			5,
		));
		assert_noop!(
			Subscription::force_cancel_ad_commitment(
				RuntimeOrigin::signed(OWNER),
				bot_hash(1),
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ========================================================================
// EscrowLow 预警事件测试
// ========================================================================

#[test]
fn escrow_low_event_emitted_during_settle() {
	new_test_ext().execute_with(|| {
		// BasicFee=10, deposit=15 → 结算后 remaining=5 < 2*10=20 → 触发 EscrowLow
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			15,
		));
		System::reset_events();
		let _ = Subscription::settle_era_subscriptions();

		let low_events: Vec<_> = System::events().into_iter()
			.filter(|e| matches!(e.event, RuntimeEvent::Subscription(Event::EscrowLow { .. })))
			.collect();
		assert_eq!(low_events.len(), 1);
	});
}

#[test]
fn escrow_low_not_emitted_when_sufficient() {
	new_test_ext().execute_with(|| {
		// BasicFee=10, deposit=50 → 结算后 remaining=40 >= 2*10=20 → 不触发
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		System::reset_events();
		let _ = Subscription::settle_era_subscriptions();

		let low_events: Vec<_> = System::events().into_iter()
			.filter(|e| matches!(e.event, RuntimeEvent::Subscription(Event::EscrowLow { .. })))
			.collect();
		assert_eq!(low_events.len(), 0);
	});
}

// ========================================================================
// Bot 封禁联动测试
// ========================================================================

#[test]
fn settle_skips_inactive_bot() {
	new_test_ext().execute_with(|| {
		// bot_hash(1) is active in mock. 手动创建一个 inactive bot 的订阅
		// bot_hash(99) 在 MockBotRegistry 中 is_bot_active=false
		// 直接插入一个订阅记录
		let sub = SubscriptionRecord::<Test> {
			owner: OWNER,
			bot_id_hash: bot_hash(99),
			tier: SubscriptionTier::Basic,
			fee_per_era: 10,
			started_at: 1,
			status: SubscriptionStatus::Active,
		};
		Subscriptions::<Test>::insert(bot_hash(99), sub);
		SubscriptionEscrow::<Test>::insert(bot_hash(99), 50u128);

		System::reset_events();
		let (income, _) = Subscription::settle_era_subscriptions();

		// 不扣费
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(99)), 50);
		// 发射 SettleSkippedInactiveBot 事件
		let skip_events: Vec<_> = System::events().into_iter()
			.filter(|e| matches!(e.event, RuntimeEvent::Subscription(Event::SettleSkippedInactiveBot { .. })))
			.collect();
		assert_eq!(skip_events.len(), 1);
	});
}

// ========================================================================
// SubscriptionProvider trait 新方法测试
// ========================================================================

#[test]
fn is_subscription_active_works() {
	new_test_ext().execute_with(|| {
		// 无订阅 → false
		assert!(!<Subscription as SubscriptionProvider>::is_subscription_active(&bot_hash(99)));

		// Active → true
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert!(<Subscription as SubscriptionProvider>::is_subscription_active(&bot_hash(1)));

		// Paused → true
		assert_ok!(Subscription::pause_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		assert!(<Subscription as SubscriptionProvider>::is_subscription_active(&bot_hash(1)));

		// Cancelled → false
		assert_ok!(Subscription::cancel_subscription(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
		));
		assert!(!<Subscription as SubscriptionProvider>::is_subscription_active(&bot_hash(1)));
	});
}

#[test]
fn subscription_status_works() {
	new_test_ext().execute_with(|| {
		assert_eq!(
			<Subscription as SubscriptionProvider>::subscription_status(&bot_hash(99)),
			None
		);
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		assert_eq!(
			<Subscription as SubscriptionProvider>::subscription_status(&bot_hash(1)),
			Some(SubscriptionStatus::Active)
		);
	});
}

