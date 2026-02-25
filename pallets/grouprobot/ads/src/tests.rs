use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_grouprobot_primitives::*;

/// 默认非 TEE 节点 (operator=NODE_OPERATOR=20)
fn default_node() -> NodeId {
	node_id(NODE_OPERATOR as u8, false)
}

/// TEE 节点 (operator=TEE_NODE_OPERATOR=30)
fn tee_node() -> NodeId {
	node_id(TEE_NODE_OPERATOR as u8, true)
}

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
	assert_ok!(GroupRobotAds::create_campaign(
		RuntimeOrigin::signed(advertiser),
		ad_text("Test Ad"),
		ad_url("https://example.com"),
		UNIT / 2,           // 0.5 UNIT per mille
		10 * UNIT,          // daily budget
		50 * UNIT,          // total budget
		AdTargetTag::All,
		0b001,              // ScheduledPost only
		1000,               // expires_at
	));
	id
}

fn create_approved_campaign(advertiser: u64) -> u64 {
	let id = create_default_campaign(advertiser);
	assert_ok!(GroupRobotAds::review_campaign(RuntimeOrigin::root(), id, true));
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
			GroupRobotAds::create_campaign(
				RuntimeOrigin::signed(ADVERTISER),
				ad_text(""),
				ad_url("https://x.com"),
				UNIT, 10 * UNIT, 50 * UNIT,
				AdTargetTag::All, 0b001, 1000,
			),
			Error::<Test>::EmptyAdText
		);
	});
}

#[test]
fn create_campaign_fails_bid_too_low() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotAds::create_campaign(
				RuntimeOrigin::signed(ADVERTISER),
				ad_text("Ad"),
				ad_url("https://x.com"),
				1,  // way below MinBidPerMille
				10 * UNIT, 50 * UNIT,
				AdTargetTag::All, 0b001, 1000,
			),
			Error::<Test>::BidTooLow
		);
	});
}

#[test]
fn create_campaign_fails_invalid_delivery_types() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotAds::create_campaign(
				RuntimeOrigin::signed(ADVERTISER),
				ad_text("Ad"),
				ad_url("https://x.com"),
				UNIT, 10 * UNIT, 50 * UNIT,
				AdTargetTag::All, 0b1000, // invalid
				1000,
			),
			Error::<Test>::InvalidDeliveryTypes
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
		assert_ok!(GroupRobotAds::fund_campaign(
			RuntimeOrigin::signed(ADVERTISER), id, 10 * UNIT,
		));

		let c = Campaigns::<Test>::get(id).unwrap();
		assert_eq!(c.total_budget, 60 * UNIT);
		assert_eq!(CampaignEscrow::<Test>::get(id), 60 * UNIT);
	});
}

#[test]
fn fund_campaign_fails_not_owner() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_noop!(
			GroupRobotAds::fund_campaign(RuntimeOrigin::signed(ADVERTISER2), id, 10 * UNIT),
			Error::<Test>::NotCampaignOwner
		);
	});
}

// ============================================================================
// pause / cancel
// ============================================================================

#[test]
fn pause_campaign_works() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_ok!(GroupRobotAds::pause_campaign(RuntimeOrigin::signed(ADVERTISER), id));
		assert_eq!(Campaigns::<Test>::get(id).unwrap().status, CampaignStatus::Paused);
	});
}

#[test]
fn cancel_campaign_refunds() {
	new_test_ext().execute_with(|| {
		let before = pallet_balances::Pallet::<Test>::free_balance(ADVERTISER);
		let id = create_default_campaign(ADVERTISER);
		let after_create = pallet_balances::Pallet::<Test>::free_balance(ADVERTISER);
		assert_eq!(before - after_create, 50 * UNIT); // reserved

		assert_ok!(GroupRobotAds::cancel_campaign(RuntimeOrigin::signed(ADVERTISER), id));
		let after_cancel = pallet_balances::Pallet::<Test>::free_balance(ADVERTISER);
		assert_eq!(after_cancel, before); // fully refunded

		assert_eq!(Campaigns::<Test>::get(id).unwrap().status, CampaignStatus::Cancelled);
		assert_eq!(CampaignEscrow::<Test>::get(id), 0);
	});
}

#[test]
fn cancel_campaign_fails_already_cancelled() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_ok!(GroupRobotAds::cancel_campaign(RuntimeOrigin::signed(ADVERTISER), id));
		assert_noop!(
			GroupRobotAds::cancel_campaign(RuntimeOrigin::signed(ADVERTISER), id),
			Error::<Test>::CampaignInactive
		);
	});
}

// ============================================================================
// review_campaign
// ============================================================================

#[test]
fn review_campaign_works() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_ok!(GroupRobotAds::review_campaign(RuntimeOrigin::root(), id, true));
		assert_eq!(Campaigns::<Test>::get(id).unwrap().review_status, AdReviewStatus::Approved);
	});
}

#[test]
fn review_campaign_reject() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_ok!(GroupRobotAds::review_campaign(RuntimeOrigin::root(), id, false));
		assert_eq!(Campaigns::<Test>::get(id).unwrap().review_status, AdReviewStatus::Rejected);
	});
}

#[test]
fn review_campaign_fails_already_reviewed() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_ok!(GroupRobotAds::review_campaign(RuntimeOrigin::root(), id, true));
		assert_noop!(
			GroupRobotAds::review_campaign(RuntimeOrigin::root(), id, true),
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
		let ch = community_hash(1);

		assert_ok!(GroupRobotAds::submit_delivery_receipt(
			RuntimeOrigin::signed(TEE_NODE_OPERATOR),
			id, ch, AdDeliveryType::ScheduledPost, 500, tee_node(), [0u8; 64],
		));

		let receipts = DeliveryReceipts::<Test>::get(&ch);
		assert_eq!(receipts.len(), 1);
		assert_eq!(receipts[0].audience_size, 500);
		assert_eq!(receipts[0].node_id, tee_node());
		assert!(!receipts[0].settled);

		assert_eq!(Campaigns::<Test>::get(id).unwrap().total_deliveries, 1);
	});
}

#[test]
fn submit_receipt_rejects_non_tee_node() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let ch = community_hash(1);

		assert_noop!(
			GroupRobotAds::submit_delivery_receipt(
				RuntimeOrigin::signed(NODE_OPERATOR),
				id, ch, AdDeliveryType::ScheduledPost, 500, default_node(), [0u8; 64],
			),
			Error::<Test>::NodeNotTee
		);
	});
}

#[test]
fn submit_receipt_fails_not_approved() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER); // not reviewed
		assert_noop!(
			GroupRobotAds::submit_delivery_receipt(
				RuntimeOrigin::signed(NODE_OPERATOR),
				id, community_hash(1), AdDeliveryType::ScheduledPost, 500, default_node(), [0u8; 64],
			),
			Error::<Test>::CampaignNotApproved
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
		let ch = community_hash(1);

		// Submit receipt: audience=1000, bid=0.5 UNIT/mille
		// Cost = 0.5 * 1000/1000 = 0.5 UNIT
		assert_ok!(GroupRobotAds::submit_delivery_receipt(
			RuntimeOrigin::signed(TEE_NODE_OPERATOR),
			id, ch, AdDeliveryType::ScheduledPost, 1000, tee_node(), [0u8; 64],
		));

		assert_ok!(GroupRobotAds::settle_era_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch,
		));

		// Cost = bid * audience / 1000 = (UNIT/2) * 1000 / 1000 = UNIT/2
		let expected_cost = UNIT / 2;
		let community_share = expected_cost * 60 / 100; // 60%
		let node_share = expected_cost * 32 / 100; // 32% (TEE)

		assert_eq!(CommunityClaimable::<Test>::get(&ch), community_share);
		assert_eq!(CommunityTotalRevenue::<Test>::get(&ch), expected_cost);

		// 节点应有待领取奖励
		assert_eq!(NodeAdPendingRewards::<Test>::get(&tee_node()), node_share);

		// Receipt should be settled
		let receipts = DeliveryReceipts::<Test>::get(&ch);
		assert!(receipts[0].settled);
	});
}

#[test]
fn settle_respects_audience_cap() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let ch = community_hash(1);

		// Set audience_cap = 200 (via direct storage for test)
		CommunityAudienceCap::<Test>::insert(&ch, 200u32);

		// Bot reports 1000 but cap is 200 → effective = 200
		assert_ok!(GroupRobotAds::submit_delivery_receipt(
			RuntimeOrigin::signed(TEE_NODE_OPERATOR),
			id, ch, AdDeliveryType::ScheduledPost, 1000, tee_node(), [0u8; 64],
		));

		assert_ok!(GroupRobotAds::settle_era_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch,
		));

		// Cost = (UNIT/2) * 200 / 1000 = UNIT/10
		let expected_cost = UNIT / 10;
		assert_eq!(CommunityTotalRevenue::<Test>::get(&ch), expected_cost);
	});
}

// ============================================================================
// stake_for_ads / unstake
// ============================================================================

#[test]
fn stake_for_ads_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(GroupRobotAds::stake_for_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 50 * UNIT,
		));

		assert_eq!(CommunityAdStake::<Test>::get(&ch), 50 * UNIT);
		// 50 UNIT → cap = 50 * 20 = 1000
		assert_eq!(CommunityAudienceCap::<Test>::get(&ch), 1000);
	});
}

#[test]
fn unstake_from_ads_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(GroupRobotAds::stake_for_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 50 * UNIT,
		));
		assert_ok!(GroupRobotAds::unstake_from_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 20 * UNIT,
		));

		assert_eq!(CommunityAdStake::<Test>::get(&ch), 30 * UNIT);
		// 30 UNIT → cap = 30 * 20 = 600
		assert_eq!(CommunityAudienceCap::<Test>::get(&ch), 600);
	});
}

#[test]
fn unstake_fails_insufficient() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(GroupRobotAds::stake_for_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 10 * UNIT,
		));
		assert_noop!(
			GroupRobotAds::unstake_from_ads(
				RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 20 * UNIT,
			),
			Error::<Test>::InsufficientStake
		);
	});
}

#[test]
fn compute_audience_cap_tiers() {
	new_test_ext().execute_with(|| {
		// 0 UNIT → 0
		assert_eq!(GroupRobotAds::compute_audience_cap(0u128), 0);
		// 10 UNIT → 200
		assert_eq!(GroupRobotAds::compute_audience_cap(10 * UNIT), 200);
		// 50 UNIT → 1000
		assert_eq!(GroupRobotAds::compute_audience_cap(50 * UNIT), 1000);
		// 200 UNIT → 5000
		assert_eq!(GroupRobotAds::compute_audience_cap(200 * UNIT), 5000);
		// very large → capped at 10000
		assert_eq!(GroupRobotAds::compute_audience_cap(10_000 * UNIT), 10_000);
	});
}

// ============================================================================
// Bi-directional preference
// ============================================================================

#[test]
fn advertiser_block_community_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(GroupRobotAds::advertiser_block_community(
			RuntimeOrigin::signed(ADVERTISER), ch,
		));

		let list = AdvertiserBlacklist::<Test>::get(&ADVERTISER);
		assert_eq!(list.len(), 1);
		assert_eq!(list[0], ch);
	});
}

#[test]
fn advertiser_block_duplicate_fails() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(GroupRobotAds::advertiser_block_community(
			RuntimeOrigin::signed(ADVERTISER), ch,
		));
		assert_noop!(
			GroupRobotAds::advertiser_block_community(
				RuntimeOrigin::signed(ADVERTISER), ch,
			),
			Error::<Test>::AlreadyBlacklisted
		);
	});
}

#[test]
fn advertiser_unblock_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(GroupRobotAds::advertiser_block_community(
			RuntimeOrigin::signed(ADVERTISER), ch,
		));
		assert_ok!(GroupRobotAds::advertiser_unblock_community(
			RuntimeOrigin::signed(ADVERTISER), ch,
		));
		assert!(AdvertiserBlacklist::<Test>::get(&ADVERTISER).is_empty());
	});
}

#[test]
fn advertiser_prefer_community_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(2);
		assert_ok!(GroupRobotAds::advertiser_prefer_community(
			RuntimeOrigin::signed(ADVERTISER), ch,
		));
		assert_eq!(AdvertiserWhitelist::<Test>::get(&ADVERTISER).len(), 1);
	});
}

#[test]
fn community_block_advertiser_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(GroupRobotAds::community_block_advertiser(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, ADVERTISER,
		));
		assert_eq!(CommunityBlacklist::<Test>::get(&ch).len(), 1);
	});
}

#[test]
fn community_unblock_advertiser_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(GroupRobotAds::community_block_advertiser(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, ADVERTISER,
		));
		assert_ok!(GroupRobotAds::community_unblock_advertiser(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, ADVERTISER,
		));
		assert!(CommunityBlacklist::<Test>::get(&ch).is_empty());
	});
}

#[test]
fn community_prefer_advertiser_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(GroupRobotAds::community_prefer_advertiser(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, ADVERTISER,
		));
		assert_eq!(CommunityWhitelist::<Test>::get(&ch).len(), 1);
	});
}

// ============================================================================
// Slash
// ============================================================================

#[test]
fn slash_community_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		// Stake 100 UNIT
		assert_ok!(GroupRobotAds::stake_for_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 100 * UNIT,
		));
		let cap_before = CommunityAudienceCap::<Test>::get(&ch);

		assert_ok!(GroupRobotAds::slash_community(
			RuntimeOrigin::root(), ch, REPORTER,
		));

		// 30% slashed = 30 UNIT
		assert_eq!(CommunityAdStake::<Test>::get(&ch), 70 * UNIT);
		// cap halved
		assert_eq!(CommunityAudienceCap::<Test>::get(&ch), cap_before / 2);
		assert_eq!(SlashCount::<Test>::get(&ch), 1);
		assert!(!BannedCommunities::<Test>::get(&ch));
	});
}

#[test]
fn slash_three_times_bans() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(GroupRobotAds::stake_for_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 100 * UNIT,
		));

		for _ in 0..3 {
			assert_ok!(GroupRobotAds::slash_community(RuntimeOrigin::root(), ch, REPORTER));
		}

		assert!(BannedCommunities::<Test>::get(&ch));
		assert_eq!(SlashCount::<Test>::get(&ch), 3);
	});
}

#[test]
fn banned_community_cannot_stake() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(GroupRobotAds::stake_for_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 100 * UNIT,
		));

		for _ in 0..3 {
			assert_ok!(GroupRobotAds::slash_community(RuntimeOrigin::root(), ch, REPORTER));
		}

		assert_noop!(
			GroupRobotAds::stake_for_ads(
				RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 10 * UNIT,
			),
			Error::<Test>::CommunityBanned
		);
	});
}

// ============================================================================
// flag_campaign
// ============================================================================

#[test]
fn flag_campaign_works() {
	new_test_ext().execute_with(|| {
		let id = create_default_campaign(ADVERTISER);
		assert_ok!(GroupRobotAds::flag_campaign(RuntimeOrigin::signed(REPORTER), id));
		assert_eq!(Campaigns::<Test>::get(id).unwrap().review_status, AdReviewStatus::Flagged);
	});
}

// ============================================================================
// claim_ad_revenue
// ============================================================================

#[test]
fn claim_ad_revenue_works() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let ch = community_hash(1);

		assert_ok!(GroupRobotAds::submit_delivery_receipt(
			RuntimeOrigin::signed(TEE_NODE_OPERATOR),
			id, ch, AdDeliveryType::ScheduledPost, 1000, tee_node(), [0u8; 64],
		));
		assert_ok!(GroupRobotAds::settle_era_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch,
		));

		let claimable = CommunityClaimable::<Test>::get(&ch);
		assert!(!claimable.is_zero());

		let before = pallet_balances::Pallet::<Test>::free_balance(COMMUNITY_OWNER);
		assert_ok!(GroupRobotAds::claim_ad_revenue(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch,
		));
		let after = pallet_balances::Pallet::<Test>::free_balance(COMMUNITY_OWNER);
		assert_eq!(after - before, claimable);
		assert_eq!(CommunityClaimable::<Test>::get(&ch), 0);
	});
}

#[test]
fn claim_fails_nothing() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotAds::claim_ad_revenue(
				RuntimeOrigin::signed(COMMUNITY_OWNER), community_hash(1),
			),
			Error::<Test>::NothingToClaim
		);
	});
}

// ============================================================================
// AdScheduleProvider trait
// ============================================================================

#[test]
fn ad_schedule_provider_trait() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert!(!<GroupRobotAds as AdScheduleProvider>::is_ads_enabled(&ch));

		// Insert a schedule
		CommunitySchedules::<Test>::insert(&ch, CommunityAdSchedule {
			community_id_hash: ch,
			scheduled_campaigns: BoundedVec::default(),
			daily_limit: 2,
			delivered_this_era: 0,
		});
		assert!(<GroupRobotAds as AdScheduleProvider>::is_ads_enabled(&ch));
		assert_eq!(<GroupRobotAds as AdScheduleProvider>::community_ad_revenue(&ch), 0);
	});
}

// ============================================================================
// Phase 5: 反作弊 — flag_community
// ============================================================================

#[test]
fn flag_community_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_eq!(CommunityFlagCount::<Test>::get(&ch), 0);

		assert_ok!(GroupRobotAds::flag_community(RuntimeOrigin::signed(REPORTER), ch));
		assert_eq!(CommunityFlagCount::<Test>::get(&ch), 1);

		assert_ok!(GroupRobotAds::flag_community(RuntimeOrigin::signed(ADVERTISER), ch));
		assert_eq!(CommunityFlagCount::<Test>::get(&ch), 2);

		System::assert_last_event(Event::CommunityFlagged {
			community_id_hash: ch,
			reporter: ADVERTISER,
			flag_count: 2,
		}.into());
	});
}

// ============================================================================
// Phase 5: L3 — audience 突增检测
// ============================================================================

#[test]
fn check_audience_surge_first_report_stores() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_eq!(PreviousEraAudience::<Test>::get(&ch), 0);

		assert_ok!(GroupRobotAds::check_audience_surge(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 500,
		));
		assert_eq!(PreviousEraAudience::<Test>::get(&ch), 500);
		// 首次不触发暂停
		assert_eq!(AudienceSurgePaused::<Test>::get(&ch), 0);
	});
}

#[test]
fn check_audience_surge_normal_growth() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		// 设置 previous = 500
		PreviousEraAudience::<Test>::insert(&ch, 500u32);

		// 800 = 60% 增长, 阈值 100% → 不触发
		assert_ok!(GroupRobotAds::check_audience_surge(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 800,
		));
		assert_eq!(AudienceSurgePaused::<Test>::get(&ch), 0);
		assert_eq!(PreviousEraAudience::<Test>::get(&ch), 800);
	});
}

#[test]
fn check_audience_surge_triggers_pause() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		PreviousEraAudience::<Test>::insert(&ch, 500u32);

		// 1100 = 120% 增长, 阈值 100% → 触发暂停
		assert_ok!(GroupRobotAds::check_audience_surge(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 1100,
		));
		assert_eq!(AudienceSurgePaused::<Test>::get(&ch), 2);

		System::assert_last_event(Event::AudienceSurgePausedEvent {
			community_id_hash: ch,
			previous: 500,
			current: 1100,
		}.into());
	});
}

#[test]
fn surge_pause_blocks_receipt_submission() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		let id = create_approved_campaign(ADVERTISER);

		// 暂停社区广告
		AudienceSurgePaused::<Test>::insert(&ch, 2u32);

		assert_noop!(
			GroupRobotAds::submit_delivery_receipt(
				RuntimeOrigin::signed(TEE_NODE_OPERATOR),
				id, ch, AdDeliveryType::ScheduledPost, 500, tee_node(), [0u8; 64],
			),
			Error::<Test>::CommunityAdsPaused
		);
	});
}

#[test]
fn surge_pause_blocks_settlement() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		AudienceSurgePaused::<Test>::insert(&ch, 1u32);

		assert_noop!(
			GroupRobotAds::settle_era_ads(RuntimeOrigin::signed(COMMUNITY_OWNER), ch),
			Error::<Test>::CommunityAdsPaused
		);
	});
}

#[test]
fn surge_pause_decrements_and_resumes() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		PreviousEraAudience::<Test>::insert(&ch, 500u32);
		AudienceSurgePaused::<Test>::insert(&ch, 2u32);

		// 正常 audience (600 = 20% growth, under threshold)
		assert_ok!(GroupRobotAds::check_audience_surge(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 600,
		));
		assert_eq!(AudienceSurgePaused::<Test>::get(&ch), 1);

		// 再次正常
		assert_ok!(GroupRobotAds::check_audience_surge(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 700,
		));
		assert_eq!(AudienceSurgePaused::<Test>::get(&ch), 0);

		System::assert_last_event(Event::AudienceSurgeResumed {
			community_id_hash: ch,
		}.into());
	});
}

// ============================================================================
// Phase 5: L5 — 多节点交叉验证
// ============================================================================

#[test]
fn report_node_audience_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);

		assert_ok!(GroupRobotAds::report_node_audience(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 500, 1,
		));
		assert_ok!(GroupRobotAds::report_node_audience(
			RuntimeOrigin::signed(ADVERTISER), ch, 520, 2,
		));

		let reports = NodeAudienceReports::<Test>::get(&ch);
		assert_eq!(reports.len(), 2);
		assert_eq!(reports[0], (1, 500));
		assert_eq!(reports[1], (2, 520));
	});
}

#[test]
fn report_node_audience_banned_community_rejected() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		BannedCommunities::<Test>::insert(&ch, true);

		assert_noop!(
			GroupRobotAds::report_node_audience(
				RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 500, 1,
			),
			Error::<Test>::CommunityBanned
		);
	});
}

#[test]
fn validate_node_reports_single_node_ok() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		assert_ok!(GroupRobotAds::report_node_audience(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 500, 1,
		));

		// 单节点 → 跳过交叉验证, 返回 Ok
		let result = GroupRobotAds::validate_node_reports(&ch);
		assert_eq!(result, Ok(Some(500)));
	});
}

#[test]
fn validate_node_reports_within_threshold() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		// 节点 1: 500, 节点 2: 590 → 偏差 18% < 20%
		assert_ok!(GroupRobotAds::report_node_audience(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 500, 1,
		));
		assert_ok!(GroupRobotAds::report_node_audience(
			RuntimeOrigin::signed(ADVERTISER), ch, 590, 2,
		));

		let result = GroupRobotAds::validate_node_reports(&ch);
		assert!(result.is_ok());
	});
}

#[test]
fn validate_node_reports_exceeds_threshold() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		// 节点 1: 500, 节点 2: 700 → 偏差 40% > 20%
		assert_ok!(GroupRobotAds::report_node_audience(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 500, 1,
		));
		assert_ok!(GroupRobotAds::report_node_audience(
			RuntimeOrigin::signed(ADVERTISER), ch, 700, 2,
		));

		let result = GroupRobotAds::validate_node_reports(&ch);
		assert_eq!(result, Err((500, 700)));
	});
}

#[test]
fn settle_era_ads_rejected_by_node_deviation() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		let id = create_approved_campaign(ADVERTISER);

		// 提交收据
		assert_ok!(GroupRobotAds::submit_delivery_receipt(
			RuntimeOrigin::signed(TEE_NODE_OPERATOR),
			id, ch, AdDeliveryType::ScheduledPost, 500, tee_node(), [0u8; 64],
		));

		// 节点上报: 偏差过大
		assert_ok!(GroupRobotAds::report_node_audience(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 200, 1,
		));
		assert_ok!(GroupRobotAds::report_node_audience(
			RuntimeOrigin::signed(ADVERTISER), ch, 600, 2,
		));

		// 结算被拒 (transactional: 所有 storage 变更回滚)
		assert_noop!(
			GroupRobotAds::settle_era_ads(RuntimeOrigin::signed(COMMUNITY_OWNER), ch),
			Error::<Test>::NodeDeviationTooHigh
		);

		// transactional 回滚: 报告仍存在
		assert_eq!(NodeAudienceReports::<Test>::get(&ch).len(), 2);
	});
}

#[test]
fn settle_era_ads_passes_with_valid_node_reports() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		let id = create_approved_campaign(ADVERTISER);

		assert_ok!(GroupRobotAds::submit_delivery_receipt(
			RuntimeOrigin::signed(TEE_NODE_OPERATOR),
			id, ch, AdDeliveryType::ScheduledPost, 500, tee_node(), [0u8; 64],
		));

		// 节点上报: 偏差在范围内 (510 vs 500 = 2%)
		assert_ok!(GroupRobotAds::report_node_audience(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch, 500, 1,
		));
		assert_ok!(GroupRobotAds::report_node_audience(
			RuntimeOrigin::signed(ADVERTISER), ch, 510, 2,
		));

		// 结算成功
		assert_ok!(GroupRobotAds::settle_era_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch,
		));

		// 节点报告已清除
		assert_eq!(NodeAudienceReports::<Test>::get(&ch).len(), 0);
	});
}

#[test]
fn settle_no_node_reports_still_works() {
	new_test_ext().execute_with(|| {
		let ch = community_hash(1);
		let id = create_approved_campaign(ADVERTISER);

		assert_ok!(GroupRobotAds::submit_delivery_receipt(
			RuntimeOrigin::signed(TEE_NODE_OPERATOR),
			id, ch, AdDeliveryType::ScheduledPost, 500, tee_node(), [0u8; 64],
		));

		// 无节点报告 → 跳过 L5, 正常结算
		assert_ok!(GroupRobotAds::settle_era_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch,
		));
	});
}

// ============================================================================
// TEE 节点广告加成
// ============================================================================

#[test]
fn tee_node_gets_bonus_on_settle() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let ch = community_hash(1);

		// TEE 节点提交收据
		assert_ok!(GroupRobotAds::submit_delivery_receipt(
			RuntimeOrigin::signed(TEE_NODE_OPERATOR),
			id, ch, AdDeliveryType::ScheduledPost, 1000, tee_node(), [0u8; 64],
		));

		assert_ok!(GroupRobotAds::settle_era_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch,
		));

		// Cost = (UNIT/2) * 1000 / 1000 = UNIT/2
		let expected_cost = UNIT / 2;
		let tee_node_share = expected_cost * 32 / 100; // TEE = 32%

		let pending = NodeAdPendingRewards::<Test>::get(&tee_node());
		assert_eq!(pending, tee_node_share);
	});
}

#[test]
fn non_tee_node_cannot_submit_receipt() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let ch = community_hash(1);

		// 非 TEE 节点提交收据应被拒绝 (BOT_TOKEN 安全风险)
		assert_noop!(
			GroupRobotAds::submit_delivery_receipt(
				RuntimeOrigin::signed(NODE_OPERATOR),
				id, ch, AdDeliveryType::ScheduledPost, 1000, default_node(), [0u8; 64],
			),
			Error::<Test>::NodeNotTee
		);
	});
}

#[test]
fn tee_bonus_comes_from_treasury_share() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let ch = community_hash(1);

		let treasury_before = pallet_balances::Pallet::<Test>::free_balance(TREASURY);

		// TEE 节点
		assert_ok!(GroupRobotAds::submit_delivery_receipt(
			RuntimeOrigin::signed(TEE_NODE_OPERATOR),
			id, ch, AdDeliveryType::ScheduledPost, 1000, tee_node(), [0u8; 64],
		));
		assert_ok!(GroupRobotAds::settle_era_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch,
		));

		let expected_cost = UNIT / 2;
		let community_share = expected_cost * 60 / 100;
		let tee_node_share = expected_cost * 32 / 100; // 32%
		// 国库 = 剩余 (8%)
		let treasury_direct = expected_cost - community_share - tee_node_share;
		// 国库实际变化 = treasury_direct + tee_node_share (代管)
		let treasury_after = pallet_balances::Pallet::<Test>::free_balance(TREASURY);
		let treasury_change = treasury_after - treasury_before;
		assert_eq!(treasury_change, treasury_direct + tee_node_share);
	});
}

#[test]
fn claim_node_ad_revenue_works() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let ch = community_hash(1);

		assert_ok!(GroupRobotAds::submit_delivery_receipt(
			RuntimeOrigin::signed(TEE_NODE_OPERATOR),
			id, ch, AdDeliveryType::ScheduledPost, 1000, tee_node(), [0u8; 64],
		));
		assert_ok!(GroupRobotAds::settle_era_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch,
		));

		let pending = NodeAdPendingRewards::<Test>::get(&tee_node());
		assert!(pending > 0);

		let before = pallet_balances::Pallet::<Test>::free_balance(TEE_NODE_OPERATOR);
		assert_ok!(GroupRobotAds::claim_node_ad_revenue(
			RuntimeOrigin::signed(TEE_NODE_OPERATOR), tee_node(),
		));
		let after = pallet_balances::Pallet::<Test>::free_balance(TEE_NODE_OPERATOR);

		assert_eq!(after - before, pending);
		assert_eq!(NodeAdPendingRewards::<Test>::get(&tee_node()), 0);
		assert_eq!(NodeAdTotalEarned::<Test>::get(&tee_node()), pending);
	});
}

#[test]
fn claim_node_ad_revenue_fails_not_operator() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let ch = community_hash(1);

		assert_ok!(GroupRobotAds::submit_delivery_receipt(
			RuntimeOrigin::signed(TEE_NODE_OPERATOR),
			id, ch, AdDeliveryType::ScheduledPost, 1000, tee_node(), [0u8; 64],
		));
		assert_ok!(GroupRobotAds::settle_era_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch,
		));

		// 非运营者尝试领取
		assert_noop!(
			GroupRobotAds::claim_node_ad_revenue(
				RuntimeOrigin::signed(ADVERTISER), tee_node(),
			),
			Error::<Test>::NotNodeOperator
		);
	});
}

#[test]
fn claim_node_ad_revenue_fails_nothing() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotAds::claim_node_ad_revenue(
				RuntimeOrigin::signed(NODE_OPERATOR), default_node(),
			),
			Error::<Test>::NoNodeAdReward
		);
	});
}

#[test]
fn set_tee_ad_percentage_works() {
	new_test_ext().execute_with(|| {
		// 默认 0 (使用硬编码默认 32%)
		assert_eq!(TeeNodeAdPct::<Test>::get(), 0);

		assert_ok!(GroupRobotAds::set_tee_ad_percentage(
			RuntimeOrigin::root(), 35,
		));
		assert_eq!(TeeNodeAdPct::<Test>::get(), 35);

		System::assert_last_event(Event::TeeAdPercentUpdated {
			tee_pct: 35,
		}.into());
	});
}

#[test]
fn set_tee_ad_percentage_rejects_invalid() {
	new_test_ext().execute_with(|| {
		// 超过 40%
		assert_noop!(
			GroupRobotAds::set_tee_ad_percentage(RuntimeOrigin::root(), 50),
			Error::<Test>::InvalidPercentage
		);
		assert_noop!(
			GroupRobotAds::set_tee_ad_percentage(RuntimeOrigin::root(), 41),
			Error::<Test>::InvalidPercentage
		);
		// 边界值: 40% 应成功
		assert_ok!(GroupRobotAds::set_tee_ad_percentage(RuntimeOrigin::root(), 40));
		assert_eq!(TeeNodeAdPct::<Test>::get(), 40);
	});
}

#[test]
fn governance_tee_percentage_applied() {
	new_test_ext().execute_with(|| {
		// 治理调整: TEE=25%
		assert_ok!(GroupRobotAds::set_tee_ad_percentage(RuntimeOrigin::root(), 25));

		let id = create_approved_campaign(ADVERTISER);
		let ch = community_hash(1);

		assert_ok!(GroupRobotAds::submit_delivery_receipt(
			RuntimeOrigin::signed(TEE_NODE_OPERATOR),
			id, ch, AdDeliveryType::ScheduledPost, 1000, tee_node(), [0u8; 64],
		));
		assert_ok!(GroupRobotAds::settle_era_ads(
			RuntimeOrigin::signed(COMMUNITY_OWNER), ch,
		));

		let expected_cost = UNIT / 2;
		let tee_share = expected_cost * 25 / 100; // 治理设置的 25%

		assert_eq!(NodeAdPendingRewards::<Test>::get(&tee_node()), tee_share);
	});
}

#[test]
fn non_tee_always_rejected() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let ch = community_hash(1);

		// 非 TEE 节点始终无法提交收据
		assert_noop!(
			GroupRobotAds::submit_delivery_receipt(
				RuntimeOrigin::signed(NODE_OPERATOR),
				id, ch, AdDeliveryType::ScheduledPost, 1000, default_node(), [0u8; 64],
			),
			Error::<Test>::NodeNotTee
		);
	});
}

#[test]
fn submit_receipt_fails_inactive_node() {
	new_test_ext().execute_with(|| {
		let id = create_approved_campaign(ADVERTISER);
		let ch = community_hash(1);

		// node_id[0] == 0 → inactive in MockNodeConsensus
		let inactive = node_id(0, false);
		assert_noop!(
			GroupRobotAds::submit_delivery_receipt(
				RuntimeOrigin::signed(COMMUNITY_OWNER),
				id, ch, AdDeliveryType::ScheduledPost, 500, inactive, [0u8; 64],
			),
			Error::<Test>::NodeNotActive
		);
	});
}
