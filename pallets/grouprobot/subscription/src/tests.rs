use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
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
		let income = Subscription::settle_era_subscriptions();
		// BasicFee = 10
		assert_eq!(income, 10);
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
