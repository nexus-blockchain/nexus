use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok, BoundedVec};

// ============================================================================
// Helpers
// ============================================================================

fn ad_text(s: &str) -> BoundedVec<u8, <Test as Config>::MaxAdTextLength> {
	BoundedVec::try_from(s.as_bytes().to_vec()).unwrap()
}

fn ad_url(s: &str) -> BoundedVec<u8, <Test as Config>::MaxAdUrlLength> {
	BoundedVec::try_from(s.as_bytes().to_vec()).unwrap()
}

const UNIT: u128 = 1_000_000_000_000;

fn create_default_campaign(advertiser: u64) -> u64 {
	let id = NextCampaignId::<Test>::get();
	assert_ok!(AdsCore::create_campaign(
		RuntimeOrigin::signed(advertiser),
		ad_text("Test Ad"),
		ad_url("https://example.com"),
		UNIT / 2,           // 0.5 UNIT per mille
		10 * UNIT,          // daily budget
		50 * UNIT,          // total budget
		0b001,              // type bit 0
		1000,               // expires_at
	));
	id
}

fn create_approved_campaign(advertiser: u64) -> u64 {
	let id = create_default_campaign(advertiser);
	assert_ok!(AdsCore::review_campaign(RuntimeOrigin::root(), id, true));
	id
}

// ============================================================================
// create_campaign
// ============================================================================

#[test]
fn create_campaign_works() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_eq!(id, 0);

		let campaign = Campaigns::<Test>::get(id).unwrap();
		assert_eq!(campaign.advertiser, ADVERTISER);
		assert_eq!(campaign.bid_per_mille, UNIT / 2);
		assert_eq!(campaign.total_budget, 50 * UNIT);
		assert_eq!(campaign.spent, 0);
		assert_eq!(campaign.status, CampaignStatus::Active);
		assert_eq!(campaign.review_status, AdReviewStatus::Pending);

		assert_eq!(CampaignEscrow::<Test>::get(id), 50 * UNIT);
		assert_eq!(NextCampaignId::<Test>::get(), 1);
	});
}

#[test]
fn create_campaign_fails_empty_text() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsCore::create_campaign(
				RuntimeOrigin::signed(ADVERTISER),
				ad_text(""),
				ad_url("https://x.com"),
				UNIT, 10 * UNIT, 50 * UNIT,
				0b001, 1000,
			),
			Error::<Test>::EmptyAdText
		);
	});
}

#[test]
fn create_campaign_fails_bid_too_low() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsCore::create_campaign(
				RuntimeOrigin::signed(ADVERTISER),
				ad_text("Ad"),
				ad_url("https://x.com"),
				1, 10 * UNIT, 50 * UNIT,
				0b001, 1000,
			),
			Error::<Test>::BidTooLow
		);
	});
}

#[test]
fn create_campaign_fails_invalid_delivery_types() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsCore::create_campaign(
				RuntimeOrigin::signed(ADVERTISER),
				ad_text("Ad"),
				ad_url("https://x.com"),
				UNIT, 10 * UNIT, 50 * UNIT,
				0b1000, // invalid
				1000,
			),
			Error::<Test>::InvalidDeliveryTypes
		);
	});
}

#[test]
fn create_campaign_fails_zero_budget() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsCore::create_campaign(
				RuntimeOrigin::signed(ADVERTISER),
				ad_text("Ad"),
				ad_url("https://x.com"),
				UNIT, 10 * UNIT, 0,
				0b001, 1000,
			),
			Error::<Test>::ZeroBudget
		);
	});
}

#[test]
fn create_campaign_fails_invalid_expiry() {
	new_test_ext().execute_with(|| {
		// block_number = 1, expires_at = 1 → not > now
		assert_noop!(
			AdsCore::create_campaign(
				RuntimeOrigin::signed(ADVERTISER),
				ad_text("Ad"),
				ad_url("https://x.com"),
				UNIT, 10 * UNIT, 50 * UNIT,
				0b001, 1, // expires_at == now
			),
			Error::<Test>::InvalidExpiry
		);
	});
}

// ============================================================================
// fund_campaign
// ============================================================================

#[test]
fn fund_campaign_works() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_ok!(AdsCore::fund_campaign(
			RuntimeOrigin::signed(ADVERTISER),
			id,
			10 * UNIT,
		));
		let c = Campaigns::<Test>::get(id).unwrap();
		assert_eq!(c.total_budget, 60 * UNIT);
		assert_eq!(CampaignEscrow::<Test>::get(id), 60 * UNIT);
	});
}

#[test]
fn fund_campaign_revives_exhausted() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		// Manually set to Exhausted
		Campaigns::<Test>::mutate(id, |c| {
			c.as_mut().unwrap().status = CampaignStatus::Exhausted;
		});
		assert_ok!(AdsCore::fund_campaign(
			RuntimeOrigin::signed(ADVERTISER), id, 5 * UNIT,
		));
		assert_eq!(Campaigns::<Test>::get(id).unwrap().status, CampaignStatus::Active);
	});
}

// ============================================================================
// pause / cancel
// ============================================================================

#[test]
fn pause_campaign_works() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_ok!(AdsCore::pause_campaign(RuntimeOrigin::signed(ADVERTISER), id));
		assert_eq!(Campaigns::<Test>::get(id).unwrap().status, CampaignStatus::Paused);
	});
}

#[test]
fn cancel_campaign_refunds() {
	new_test_ext().execute_with(|| {
		let before = pallet_balances::Pallet::<Test>::free_balance(ADVERTISER);
		let id = create_default_campaign(ADVERTISER);
		let after_create = pallet_balances::Pallet::<Test>::free_balance(ADVERTISER);
		// 50 UNIT reserved
		assert_eq!(before - after_create, 50 * UNIT);

		assert_ok!(AdsCore::cancel_campaign(RuntimeOrigin::signed(ADVERTISER), id));
		let after_cancel = pallet_balances::Pallet::<Test>::free_balance(ADVERTISER);
		assert_eq!(after_cancel, before); // fully refunded
		assert_eq!(Campaigns::<Test>::get(id).unwrap().status, CampaignStatus::Cancelled);
	});
}

// ============================================================================
// review
// ============================================================================

#[test]
fn review_campaign_works() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_ok!(AdsCore::review_campaign(RuntimeOrigin::root(), id, true));
		assert_eq!(Campaigns::<Test>::get(id).unwrap().review_status, AdReviewStatus::Approved);
	});
}

#[test]
fn review_campaign_reject() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_ok!(AdsCore::review_campaign(RuntimeOrigin::root(), id, false));
		assert_eq!(Campaigns::<Test>::get(id).unwrap().review_status, AdReviewStatus::Rejected);
	});
}

#[test]
fn review_campaign_fails_already_reviewed() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_ok!(AdsCore::review_campaign(RuntimeOrigin::root(), id, true));
		assert_noop!(
			AdsCore::review_campaign(RuntimeOrigin::root(), id, false),
			Error::<Test>::AlreadyReviewed
		);
	});
}

// ============================================================================
// submit_delivery_receipt
// ============================================================================

#[test]
fn submit_delivery_receipt_works() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let pid = placement_id(1);

		assert_ok!(AdsCore::submit_delivery_receipt(
			RuntimeOrigin::signed(PLACEMENT_ADMIN),
			id, pid, 100, 100,
		));

		let receipts = DeliveryReceipts::<Test>::get(&pid);
		assert_eq!(receipts.len(), 1);
		assert_eq!(receipts[0].audience_size, 100);
		assert_eq!(receipts[0].campaign_id, id);
		assert_eq!(PlacementEraDeliveries::<Test>::get(&pid), 1);
	});
}

#[test]
fn submit_receipt_caps_audience() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let pid = placement_id(1);

		// MockDeliveryVerifier caps at 500
		assert_ok!(AdsCore::submit_delivery_receipt(
			RuntimeOrigin::signed(PLACEMENT_ADMIN),
			id, pid, 1000, 100,
		));

		let receipts = DeliveryReceipts::<Test>::get(&pid);
		assert_eq!(receipts[0].audience_size, 500); // capped
	});
}

#[test]
fn submit_receipt_fails_not_approved() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER); // not reviewed
		let pid = placement_id(1);

		assert_noop!(
			AdsCore::submit_delivery_receipt(
				RuntimeOrigin::signed(PLACEMENT_ADMIN),
				id, pid, 100, 100,
			),
			Error::<Test>::CampaignNotApproved
		);
	});
}

#[test]
fn submit_receipt_fails_campaign_expired() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let pid = placement_id(1);

		// Advance past expiry
		System::set_block_number(1001);
		assert_noop!(
			AdsCore::submit_delivery_receipt(
				RuntimeOrigin::signed(PLACEMENT_ADMIN),
				id, pid, 100, 100,
			),
			Error::<Test>::CampaignExpired
		);
	});
}

#[test]
fn submit_receipt_fails_audience_below_min() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let pid = placement_id(1);

		// MinAudienceSize = 20, submit 10
		assert_noop!(
			AdsCore::submit_delivery_receipt(
				RuntimeOrigin::signed(PLACEMENT_ADMIN),
				id, pid, 10, 100,
			),
			Error::<Test>::AudienceBelowMinimum
		);
	});
}

#[test]
fn submit_receipt_fails_banned_placement() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let pid = placement_id(1);
		BannedPlacements::<Test>::insert(&pid, true);

		assert_noop!(
			AdsCore::submit_delivery_receipt(
				RuntimeOrigin::signed(PLACEMENT_ADMIN),
				id, pid, 100, 100,
			),
			Error::<Test>::PlacementBanned
		);
	});
}

// ============================================================================
// settle_era_ads
// ============================================================================

#[test]
fn settle_era_ads_works() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let pid = placement_id(1);

		// Submit receipt: audience=100, multiplier=100 (1.0x)
		assert_ok!(AdsCore::submit_delivery_receipt(
			RuntimeOrigin::signed(PLACEMENT_ADMIN),
			id, pid, 100, 100,
		));

		// Expected cost: bid(0.5 UNIT) * 100 * 100 / 100_000 = 0.05 UNIT
		let expected_cost = UNIT / 2 * 100 * 100 / 100_000;

		assert_ok!(AdsCore::settle_era_ads(
			RuntimeOrigin::signed(ADVERTISER2),
			pid,
		));

		// Receipts cleared
		assert_eq!(DeliveryReceipts::<Test>::get(&pid).len(), 0);

		// Revenue recorded
		assert_eq!(EraAdRevenue::<Test>::get(&pid), expected_cost);
		assert_eq!(PlacementTotalRevenue::<Test>::get(&pid), expected_cost);

		// Claimable = 80% of cost (MockRevenueDistributor)
		assert_eq!(PlacementClaimable::<Test>::get(&pid), expected_cost * 80 / 100);

		// Campaign spent updated
		let c = Campaigns::<Test>::get(id).unwrap();
		assert_eq!(c.spent, expected_cost);
	});
}

// ============================================================================
// claim_ad_revenue
// ============================================================================

#[test]
fn claim_ad_revenue_works() {
	new_test_ext().execute_with(|| {
		let pid = placement_id(1);
		// Manually set claimable
		PlacementClaimable::<Test>::insert(&pid, 10 * UNIT);

		let before = pallet_balances::Pallet::<Test>::free_balance(PLACEMENT_ADMIN);
		assert_ok!(AdsCore::claim_ad_revenue(
			RuntimeOrigin::signed(PLACEMENT_ADMIN), pid,
		));
		let after = pallet_balances::Pallet::<Test>::free_balance(PLACEMENT_ADMIN);
		assert_eq!(after - before, 10 * UNIT);
		assert_eq!(PlacementClaimable::<Test>::get(&pid), 0);
	});
}

#[test]
fn claim_ad_revenue_fails_not_admin() {
	new_test_ext().execute_with(|| {
		let pid = placement_id(1);
		PlacementClaimable::<Test>::insert(&pid, 10 * UNIT);
		assert_noop!(
			AdsCore::claim_ad_revenue(RuntimeOrigin::signed(ADVERTISER), pid),
			Error::<Test>::NotPlacementAdmin
		);
	});
}

#[test]
fn claim_ad_revenue_fails_nothing_to_claim() {
	new_test_ext().execute_with(|| {
		let pid = placement_id(1);
		assert_noop!(
			AdsCore::claim_ad_revenue(RuntimeOrigin::signed(PLACEMENT_ADMIN), pid),
			Error::<Test>::NothingToClaim
		);
	});
}

// ============================================================================
// Bidirectional preferences
// ============================================================================

#[test]
fn advertiser_blacklist_works() {
	new_test_ext().execute_with(|| {
		let pid = placement_id(1);
		assert_ok!(AdsCore::advertiser_block_placement(
			RuntimeOrigin::signed(ADVERTISER), pid,
		));
		assert_eq!(AdvertiserBlacklist::<Test>::get(ADVERTISER).len(), 1);

		assert_ok!(AdsCore::advertiser_unblock_placement(
			RuntimeOrigin::signed(ADVERTISER), pid,
		));
		assert_eq!(AdvertiserBlacklist::<Test>::get(ADVERTISER).len(), 0);
	});
}

#[test]
fn advertiser_whitelist_works() {
	new_test_ext().execute_with(|| {
		let pid = placement_id(1);
		assert_ok!(AdsCore::advertiser_prefer_placement(
			RuntimeOrigin::signed(ADVERTISER), pid,
		));
		assert_eq!(AdvertiserWhitelist::<Test>::get(ADVERTISER).len(), 1);

		assert_ok!(AdsCore::advertiser_unprefer_placement(
			RuntimeOrigin::signed(ADVERTISER), pid,
		));
		assert_eq!(AdvertiserWhitelist::<Test>::get(ADVERTISER).len(), 0);
	});
}

#[test]
fn placement_blacklist_works() {
	new_test_ext().execute_with(|| {
		let pid = placement_id(1);
		assert_ok!(AdsCore::placement_block_advertiser(
			RuntimeOrigin::signed(PLACEMENT_ADMIN), pid, ADVERTISER,
		));
		assert_eq!(PlacementBlacklist::<Test>::get(&pid).len(), 1);

		assert_ok!(AdsCore::placement_unblock_advertiser(
			RuntimeOrigin::signed(PLACEMENT_ADMIN), pid, ADVERTISER,
		));
		assert_eq!(PlacementBlacklist::<Test>::get(&pid).len(), 0);
	});
}

#[test]
fn placement_whitelist_works() {
	new_test_ext().execute_with(|| {
		let pid = placement_id(1);
		assert_ok!(AdsCore::placement_prefer_advertiser(
			RuntimeOrigin::signed(PLACEMENT_ADMIN), pid, ADVERTISER,
		));
		assert_eq!(PlacementWhitelist::<Test>::get(&pid).len(), 1);

		assert_ok!(AdsCore::placement_unprefer_advertiser(
			RuntimeOrigin::signed(PLACEMENT_ADMIN), pid, ADVERTISER,
		));
		assert_eq!(PlacementWhitelist::<Test>::get(&pid).len(), 0);
	});
}

#[test]
fn placement_ops_fail_not_admin() {
	new_test_ext().execute_with(|| {
		let pid = placement_id(1);
		assert_noop!(
			AdsCore::placement_block_advertiser(
				RuntimeOrigin::signed(ADVERTISER), pid, ADVERTISER2,
			),
			Error::<Test>::NotPlacementAdmin
		);
	});
}

// ============================================================================
// flag / slash
// ============================================================================

#[test]
fn flag_campaign_works() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_ok!(AdsCore::flag_campaign(RuntimeOrigin::signed(REPORTER), id));
		assert_eq!(
			Campaigns::<Test>::get(id).unwrap().review_status,
			AdReviewStatus::Flagged
		);
	});
}

#[test]
fn flag_placement_works() {
	new_test_ext().execute_with(|| {
		let pid = placement_id(1);
		assert_ok!(AdsCore::flag_placement(RuntimeOrigin::signed(REPORTER), pid));
		assert_eq!(PlacementFlagCount::<Test>::get(&pid), 1);

		assert_ok!(AdsCore::flag_placement(RuntimeOrigin::signed(REPORTER), pid));
		assert_eq!(PlacementFlagCount::<Test>::get(&pid), 2);
	});
}

#[test]
fn slash_placement_bans_after_3() {
	new_test_ext().execute_with(|| {
		let pid = placement_id(1);
		assert_ok!(AdsCore::slash_placement(RuntimeOrigin::root(), pid, REPORTER));
		assert_eq!(SlashCount::<Test>::get(&pid), 1);
		assert!(!BannedPlacements::<Test>::get(&pid));

		assert_ok!(AdsCore::slash_placement(RuntimeOrigin::root(), pid, REPORTER));
		assert_eq!(SlashCount::<Test>::get(&pid), 2);

		assert_ok!(AdsCore::slash_placement(RuntimeOrigin::root(), pid, REPORTER));
		assert_eq!(SlashCount::<Test>::get(&pid), 3);
		assert!(BannedPlacements::<Test>::get(&pid));
	});
}

// ============================================================================
// register_private_ad
// ============================================================================

#[test]
fn register_private_ad_works() {
	new_test_ext().execute_with(|| {
		let pid = placement_id(1);
		assert_ok!(AdsCore::register_private_ad(
			RuntimeOrigin::signed(PLACEMENT_ADMIN), pid, 3,
		));
		assert_eq!(PrivateAdCount::<Test>::get(&pid), 3);
		assert_eq!(PlacementEraDeliveries::<Test>::get(&pid), 3);
	});
}

#[test]
fn register_private_ad_fails_not_admin() {
	new_test_ext().execute_with(|| {
		let pid = placement_id(1);
		assert_noop!(
			AdsCore::register_private_ad(RuntimeOrigin::signed(ADVERTISER), pid, 1),
			Error::<Test>::NotPlacementAdmin
		);
	});
}

#[test]
fn register_private_ad_fails_zero_count() {
	new_test_ext().execute_with(|| {
		let pid = placement_id(1);
		assert_noop!(
			AdsCore::register_private_ad(RuntimeOrigin::signed(PLACEMENT_ADMIN), pid, 0),
			Error::<Test>::ZeroPrivateAdCount
		);
	});
}
