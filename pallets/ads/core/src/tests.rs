use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok, traits::ReservableCurrency, BoundedVec};

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
fn review_campaign_allows_re_review_of_approved() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_ok!(AdsCore::review_campaign(RuntimeOrigin::root(), id, true));
		// Governance can re-review an approved campaign (e.g. revoke approval)
		assert_ok!(AdsCore::review_campaign(RuntimeOrigin::root(), id, false));
		assert_eq!(Campaigns::<Test>::get(id).unwrap().review_status, AdReviewStatus::Rejected);
	});
}

#[test]
fn review_campaign_fails_already_rejected() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_ok!(AdsCore::review_campaign(RuntimeOrigin::root(), id, false));
		assert_noop!(
			AdsCore::review_campaign(RuntimeOrigin::root(), id, true),
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

		// M1: 不同用户可继续举报
		assert_ok!(AdsCore::flag_placement(RuntimeOrigin::signed(ADVERTISER), pid));
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

// ============================================================================
// 审计回归测试
// ============================================================================

// C1: settle_era_ads 中 unreserve 不足时只结算实际解锁金额
#[test]
fn c1_settle_uses_actually_unreserved_amount() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let pid = placement_id(1);

		// 提交收据
		assert_ok!(AdsCore::submit_delivery_receipt(
			RuntimeOrigin::signed(PLACEMENT_ADMIN),
			id, pid, 100, 100,
		));

		// 手动减少 advertiser 的 reserved (模拟部分 unreserve 已被外部消耗)
		// 先查当前 reserved
		let reserved_before = pallet_balances::Pallet::<Test>::reserved_balance(ADVERTISER);
		assert!(reserved_before > 0);

		// unreserve 大部分，只留 1 token reserved
		let to_unreserve = reserved_before.saturating_sub(1);
		pallet_balances::Pallet::<Test>::unreserve(&ADVERTISER, to_unreserve);
		assert_eq!(pallet_balances::Pallet::<Test>::reserved_balance(ADVERTISER), 1);

		let treasury_before = pallet_balances::Pallet::<Test>::free_balance(TREASURY);

		// 结算 — 应该只结算 1 token (实际可 unreserve 的金额)
		assert_ok!(AdsCore::settle_era_ads(
			RuntimeOrigin::signed(ADVERTISER2), pid,
		));

		let treasury_after = pallet_balances::Pallet::<Test>::free_balance(TREASURY);
		// 国库只收到 1 token (实际解锁量)，而非原始 CPM 费用
		assert_eq!(treasury_after - treasury_before, 1);
	});
}

// H1: settle_era_ads transfer 失败时跳过而非回滚整体
#[test]
fn h1_settle_skips_failed_transfer_does_not_abort() {
	new_test_ext().execute_with(|| {
		// 两个广告主各创建一个 campaign
		let id1 = create_approved_campaign(ADVERTISER);
		let id2 = create_approved_campaign(ADVERTISER2);
		let pid = placement_id(1);

		// 两个 campaign 都提交收据到同一广告位
		assert_ok!(AdsCore::submit_delivery_receipt(
			RuntimeOrigin::signed(PLACEMENT_ADMIN),
			id1, pid, 100, 100,
		));
		assert_ok!(AdsCore::submit_delivery_receipt(
			RuntimeOrigin::signed(PLACEMENT_ADMIN),
			id2, pid, 100, 100,
		));

		// 让 ADVERTISER 的 reserved 为 0 (模拟资金已被消耗)
		let reserved1 = pallet_balances::Pallet::<Test>::reserved_balance(ADVERTISER);
		pallet_balances::Pallet::<Test>::unreserve(&ADVERTISER, reserved1);

		// 结算应成功 — ADVERTISER 的部分被跳过，ADVERTISER2 的部分正常结算
		assert_ok!(AdsCore::settle_era_ads(
			RuntimeOrigin::signed(PLACEMENT_ADMIN), pid,
		));

		// ADVERTISER2 的 campaign spent 应该更新
		let c2 = Campaigns::<Test>::get(id2).unwrap();
		assert!(c2.spent > 0, "ADVERTISER2 campaign should have been settled");

		// 收据已清空 (Era 结束)
		assert_eq!(DeliveryReceipts::<Test>::get(&pid).len(), 0);
	});
}

// H2: flag_campaign 不能 flag 已 Approved 的 Campaign (防 griefing)
#[test]
fn h2_flag_campaign_rejects_approved_campaign() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);

		// 尝试 flag 已审核通过的 campaign — 应失败
		assert_noop!(
			AdsCore::flag_campaign(RuntimeOrigin::signed(REPORTER), id),
			Error::<Test>::AlreadyReviewed
		);

		// Campaign 仍为 Approved，投放不受影响
		assert_eq!(
			Campaigns::<Test>::get(id).unwrap().review_status,
			AdReviewStatus::Approved
		);
	});
}

// H2: governance 可以重审已 Approved 的 Campaign (reject)
#[test]
fn h2_review_campaign_can_reject_approved() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		assert_eq!(
			Campaigns::<Test>::get(id).unwrap().review_status,
			AdReviewStatus::Approved
		);

		// Governance 可以 reject 已审核通过的 Campaign
		assert_ok!(AdsCore::review_campaign(RuntimeOrigin::root(), id, false));
		assert_eq!(
			Campaigns::<Test>::get(id).unwrap().review_status,
			AdReviewStatus::Rejected
		);
	});
}

// ============================================================================
// Round 2 审计回归测试
// ============================================================================

// H1: fund_campaign 阻止对已过期 Campaign 充值
#[test]
fn h1_fund_campaign_rejects_expired() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER); // expires_at = 1000
		// 推进到过期后
		System::set_block_number(1001);
		assert_noop!(
			AdsCore::fund_campaign(RuntimeOrigin::signed(ADVERTISER), id, 5 * UNIT),
			Error::<Test>::CampaignExpired
		);
	});
}

// H1: fund_campaign 在过期前仍可正常充值
#[test]
fn h1_fund_campaign_works_before_expiry() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER); // expires_at = 1000
		System::set_block_number(999);
		assert_ok!(AdsCore::fund_campaign(RuntimeOrigin::signed(ADVERTISER), id, 5 * UNIT));
		assert_eq!(Campaigns::<Test>::get(id).unwrap().total_budget, 55 * UNIT);
	});
}

// M1: flag_placement 同一用户不可重复举报同一广告位
#[test]
fn m1_flag_placement_rejects_duplicate() {
	new_test_ext().execute_with(|| {
		let pid = placement_id(1);
		assert_ok!(AdsCore::flag_placement(RuntimeOrigin::signed(REPORTER), pid));
		assert_eq!(PlacementFlagCount::<Test>::get(&pid), 1);

		// 同一用户再次举报 — 应失败
		assert_noop!(
			AdsCore::flag_placement(RuntimeOrigin::signed(REPORTER), pid),
			Error::<Test>::AlreadyFlaggedPlacement
		);
		// count 不变
		assert_eq!(PlacementFlagCount::<Test>::get(&pid), 1);
	});
}

// M1: 不同用户可以分别举报同一广告位
#[test]
fn m1_flag_placement_allows_different_reporters() {
	new_test_ext().execute_with(|| {
		let pid = placement_id(1);
		assert_ok!(AdsCore::flag_placement(RuntimeOrigin::signed(REPORTER), pid));
		assert_ok!(AdsCore::flag_placement(RuntimeOrigin::signed(ADVERTISER), pid));
		assert_eq!(PlacementFlagCount::<Test>::get(&pid), 2);
	});
}

// ============================================================================
// Round 2+ 审计回归测试
// ============================================================================

// M1-R2: resume_campaign 正常恢复已暂停 Campaign
#[test]
fn m1r2_resume_campaign_works() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_ok!(AdsCore::pause_campaign(RuntimeOrigin::signed(ADVERTISER), id));
		assert_eq!(Campaigns::<Test>::get(id).unwrap().status, CampaignStatus::Paused);

		assert_ok!(AdsCore::resume_campaign(RuntimeOrigin::signed(ADVERTISER), id));
		assert_eq!(Campaigns::<Test>::get(id).unwrap().status, CampaignStatus::Active);
	});
}

// M1-R2: resume_campaign 仅允许恢复 Paused 状态
#[test]
fn m1r2_resume_campaign_rejects_active() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_noop!(
			AdsCore::resume_campaign(RuntimeOrigin::signed(ADVERTISER), id),
			Error::<Test>::CampaignNotPaused
		);
	});
}

// M1-R2: resume_campaign 不允许恢复已过期的 Paused Campaign
#[test]
fn m1r2_resume_campaign_rejects_expired() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER); // expires_at = 1000
		assert_ok!(AdsCore::pause_campaign(RuntimeOrigin::signed(ADVERTISER), id));
		System::set_block_number(1001);
		assert_noop!(
			AdsCore::resume_campaign(RuntimeOrigin::signed(ADVERTISER), id),
			Error::<Test>::CampaignExpired
		);
	});
}

// M1-R2: resume_campaign 仅允许 owner 操作
#[test]
fn m1r2_resume_campaign_rejects_non_owner() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_ok!(AdsCore::pause_campaign(RuntimeOrigin::signed(ADVERTISER), id));
		assert_noop!(
			AdsCore::resume_campaign(RuntimeOrigin::signed(ADVERTISER2), id),
			Error::<Test>::NotCampaignOwner
		);
	});
}

// M2-R2: 广告主拉黑广告位后, 该广告位不能提交投放收据
#[test]
fn m2r2_submit_receipt_blocked_by_advertiser_blacklist() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let pid = placement_id(1);

		// 广告主拉黑该广告位
		assert_ok!(AdsCore::advertiser_block_placement(
			RuntimeOrigin::signed(ADVERTISER), pid,
		));

		// 提交投放收据应失败
		assert_noop!(
			AdsCore::submit_delivery_receipt(
				RuntimeOrigin::signed(PLACEMENT_ADMIN),
				id, pid, 100, 100,
			),
			Error::<Test>::AdvertiserBlacklistedPlacement
		);
	});
}

// M2-R2: 广告位拉黑广告主后, 该广告主的 Campaign 不能投放到该广告位
#[test]
fn m2r2_submit_receipt_blocked_by_placement_blacklist() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let pid = placement_id(1);

		// 广告位拉黑该广告主
		assert_ok!(AdsCore::placement_block_advertiser(
			RuntimeOrigin::signed(PLACEMENT_ADMIN), pid, ADVERTISER,
		));

		// 提交投放收据应失败
		assert_noop!(
			AdsCore::submit_delivery_receipt(
				RuntimeOrigin::signed(PLACEMENT_ADMIN),
				id, pid, 100, 100,
			),
			Error::<Test>::PlacementBlacklistedAdvertiser
		);
	});
}

// ============================================================================
// Regression: L-CORE4 — expire_campaign
// ============================================================================

#[test]
fn lcore4_expire_campaign_works() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		// Campaign expires_at = 1000, advance past it
		System::set_block_number(1001);

		let free_before = Balances::free_balance(ADVERTISER);
		System::reset_events();

		// Anyone can call expire_campaign
		assert_ok!(AdsCore::expire_campaign(RuntimeOrigin::signed(42), id));

		let c = Campaigns::<Test>::get(id).unwrap();
		assert_eq!(c.status, CampaignStatus::Expired);

		// Refund: total_budget(50 UNIT) - spent(0) = 50 UNIT unreserved
		let free_after = Balances::free_balance(ADVERTISER);
		assert_eq!(free_after - free_before, 50 * UNIT);

		System::assert_has_event(RuntimeEvent::AdsCore(
			Event::CampaignMarkedExpired { campaign_id: id, refunded: 50 * UNIT }
		));
	});
}

#[test]
fn lcore4_expire_campaign_rejects_not_expired() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		// Still at block 1, expires_at = 1000
		assert_noop!(
			AdsCore::expire_campaign(RuntimeOrigin::signed(42), id),
			Error::<Test>::CampaignNotExpired
		);
	});
}

#[test]
fn lcore4_expire_campaign_rejects_cancelled() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		assert_ok!(AdsCore::cancel_campaign(RuntimeOrigin::signed(ADVERTISER), id));
		System::set_block_number(1001);
		assert_noop!(
			AdsCore::expire_campaign(RuntimeOrigin::signed(42), id),
			Error::<Test>::CampaignInactive
		);
	});
}

#[test]
fn lcore4_expire_campaign_rejects_already_expired() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		System::set_block_number(1001);
		assert_ok!(AdsCore::expire_campaign(RuntimeOrigin::signed(42), id));
		// Second call should fail — now status is Expired (not Active/Paused/Exhausted)
		assert_noop!(
			AdsCore::expire_campaign(RuntimeOrigin::signed(42), id),
			Error::<Test>::CampaignInactive
		);
	});
}

// M2-R2: 取消拉黑后可以正常提交收据
#[test]
fn m2r2_submit_receipt_works_after_unblock() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let pid = placement_id(1);

		// 拉黑再解除
		assert_ok!(AdsCore::advertiser_block_placement(
			RuntimeOrigin::signed(ADVERTISER), pid,
		));
		assert_ok!(AdsCore::advertiser_unblock_placement(
			RuntimeOrigin::signed(ADVERTISER), pid,
		));

		// 提交收据应成功
		assert_ok!(AdsCore::submit_delivery_receipt(
			RuntimeOrigin::signed(PLACEMENT_ADMIN),
			id, pid, 100, 100,
		));
	});
}
