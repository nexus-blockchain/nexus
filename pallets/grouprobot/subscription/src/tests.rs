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
fn settle_era_collects_fees() {
	new_test_ext().execute_with(|| {
		assert_ok!(Subscription::subscribe(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			SubscriptionTier::Basic,
			50,
		));
		let income = Subscription::settle_era_subscriptions();
		// BasicFee = 10
		assert_eq!(income, 10);
		// Escrow reduced by fee
		assert_eq!(SubscriptionEscrow::<Test>::get(bot_hash(1)), 40);
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
