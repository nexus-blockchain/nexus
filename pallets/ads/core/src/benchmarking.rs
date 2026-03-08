//! Benchmarking for pallet-ads-core.
//!
//! Uses `frame_benchmarking::v2` macro style.
//! All 50 extrinsics are benchmarked.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use pallet::*;
use frame_support::traits::{Currency, Get};
use sp_runtime::traits::Bounded;
use sp_runtime::Saturating;
use pallet_ads_primitives::*;

fn funded_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
	let account: T::AccountId = frame_benchmarking::account(name, index, 0);
	let amount = BalanceOf::<T>::max_value() / 4u32.into();
	let _ = T::Currency::deposit_creating(&account, amount);
	account
}

fn placement_id(n: u8) -> PlacementId {
	let mut h = [0u8; 32];
	h[0] = n;
	h
}

fn register_seed_advertiser<T: Config>(who: &T::AccountId) {
	let now = frame_system::Pallet::<T>::block_number();
	AdvertiserRegisteredAt::<T>::insert(who, now);
}

fn seed_campaign<T: Config>(advertiser: &T::AccountId) -> u64 {
	let id = NextCampaignId::<T>::get();
	let text: BoundedVec<u8, T::MaxAdTextLength> =
		b"Benchmark Ad".to_vec().try_into().expect("text fits");
	let url: BoundedVec<u8, T::MaxAdUrlLength> =
		b"https://bench.example.com".to_vec().try_into().expect("url fits");
	let budget: BalanceOf<T> = T::MinBidPerMille::get().saturating_mul(1000u32.into());
	let bid = T::MinBidPerMille::get();
	let now = frame_system::Pallet::<T>::block_number();
	let expires_at = now.saturating_add(1000u32.into());

	T::Currency::reserve(advertiser, budget).expect("reserve ok");

	let campaign = AdCampaign::<T> {
		advertiser: advertiser.clone(),
		text,
		url,
		bid_per_mille: bid,
		bid_per_click: Zero::zero(),
		campaign_type: CampaignType::Cpm,
		daily_budget: Zero::zero(),
		total_budget: budget,
		spent: Zero::zero(),
		delivery_types: 0b001,
		status: CampaignStatus::Active,
		review_status: AdReviewStatus::Pending,
		total_deliveries: 0,
		total_clicks: 0,
		created_at: now,
		expires_at,
	};
	Campaigns::<T>::insert(id, campaign);
	CampaignEscrow::<T>::insert(id, budget);
	NextCampaignId::<T>::put(id.saturating_add(1));

	let _ = CampaignsByAdvertiser::<T>::try_mutate(advertiser, |list| {
		list.try_push(id)
	});

	id
}

fn seed_approved_campaign<T: Config>(advertiser: &T::AccountId) -> u64 {
	let id = seed_campaign::<T>(advertiser);
	Campaigns::<T>::mutate(id, |maybe| {
		if let Some(c) = maybe {
			c.review_status = AdReviewStatus::Approved;
		}
	});
	Pallet::<T>::index_add_active(id, 0b001);
	id
}

fn seed_cpc_campaign<T: Config>(advertiser: &T::AccountId) -> u64 {
	let id = NextCampaignId::<T>::get();
	let text: BoundedVec<u8, T::MaxAdTextLength> =
		b"CPC Bench Ad".to_vec().try_into().expect("text fits");
	let url: BoundedVec<u8, T::MaxAdUrlLength> =
		b"https://cpc-bench.example.com".to_vec().try_into().expect("url fits");
	let budget: BalanceOf<T> = T::MinBidPerClick::get().saturating_mul(1000u32.into());
	let bid = T::MinBidPerClick::get();
	let now = frame_system::Pallet::<T>::block_number();
	let expires_at = now.saturating_add(1000u32.into());

	T::Currency::reserve(advertiser, budget).expect("reserve ok");

	let campaign = AdCampaign::<T> {
		advertiser: advertiser.clone(),
		text,
		url,
		bid_per_mille: Zero::zero(),
		bid_per_click: bid,
		campaign_type: CampaignType::Cpc,
		daily_budget: Zero::zero(),
		total_budget: budget,
		spent: Zero::zero(),
		delivery_types: 0b001,
		status: CampaignStatus::Active,
		review_status: AdReviewStatus::Approved,
		total_deliveries: 0,
		total_clicks: 0,
		created_at: now,
		expires_at,
	};
	Campaigns::<T>::insert(id, campaign);
	CampaignEscrow::<T>::insert(id, budget);
	NextCampaignId::<T>::put(id.saturating_add(1));

	let _ = CampaignsByAdvertiser::<T>::try_mutate(advertiser, |list| {
		list.try_push(id)
	});

	id
}

fn seed_receipt<T: Config>(campaign_id: u64, pid: PlacementId, submitter: &T::AccountId) -> u32 {
	let now = frame_system::Pallet::<T>::block_number();
	let receipt_index = DeliveryReceipts::<T>::get(&pid).len() as u32;
	let receipt = DeliveryReceipt::<T> {
		campaign_id,
		placement_id: pid,
		audience_size: 100,
		click_count: 0,
		verified_clicks: 0,
		cpm_multiplier_bps: 100,
		delivered_at: now,
		settled: false,
		submitter: submitter.clone(),
	};
	let _ = DeliveryReceipts::<T>::try_mutate(&pid, |receipts| {
		receipts.try_push(receipt)
	});
	ReceiptConfirmation::<T>::insert(
		(campaign_id, pid, receipt_index),
		ReceiptStatus::Pending,
	);
	ReceiptSubmittedAt::<T>::insert(
		(campaign_id, pid, receipt_index),
		now,
	);
	PlacementEraDeliveries::<T>::mutate(&pid, |c| *c = c.saturating_add(1));
	receipt_index
}

#[benchmarks]
mod benches {
	use super::*;

	#[benchmark]
	fn create_campaign() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let text: BoundedVec<u8, T::MaxAdTextLength> =
			b"Benchmark Ad".to_vec().try_into().unwrap();
		let url: BoundedVec<u8, T::MaxAdUrlLength> =
			b"https://bench.example.com".to_vec().try_into().unwrap();
		let bid = T::MinBidPerMille::get();
		let budget = bid.saturating_mul(1000u32.into());
		let now = frame_system::Pallet::<T>::block_number();
		let expires: BlockNumberFor<T> = now.saturating_add(1000u32.into());

		#[extrinsic_call]
		_(
			RawOrigin::Signed(caller),
			text, url, bid, Zero::zero(), budget,
			0b001u8, expires, None,
			CampaignType::Cpm, Zero::zero(),
		);

		let id = NextCampaignId::<T>::get() - 1;
		assert!(Campaigns::<T>::contains_key(id));
	}

	#[benchmark]
	fn fund_campaign() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);
		let amount = T::MinBidPerMille::get();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id, amount);

		assert!(CampaignEscrow::<T>::get(id) > Zero::zero());
	}

	#[benchmark]
	fn pause_campaign() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id);

		assert_eq!(Campaigns::<T>::get(id).unwrap().status, CampaignStatus::Paused);
	}

	#[benchmark]
	fn cancel_campaign() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id);

		assert_eq!(Campaigns::<T>::get(id).unwrap().status, CampaignStatus::Cancelled);
	}

	#[benchmark]
	fn review_campaign() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);

		#[extrinsic_call]
		_(RawOrigin::Root, id, true);

		assert_eq!(Campaigns::<T>::get(id).unwrap().review_status, AdReviewStatus::Approved);
	}

	#[benchmark]
	fn submit_delivery_receipt() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_approved_campaign::<T>(&caller);
		let pid = placement_id(1);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id, pid, 100u32);

		assert_eq!(DeliveryReceipts::<T>::get(&pid).len(), 1);
	}

	#[benchmark]
	fn settle_era_ads() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_approved_campaign::<T>(&caller);
		let pid = placement_id(1);
		let idx = seed_receipt::<T>(id, pid, &caller);
		ReceiptConfirmation::<T>::insert((id, pid, idx), ReceiptStatus::Confirmed);
		let settler = funded_account::<T>("settler", 1);

		#[extrinsic_call]
		_(RawOrigin::Signed(settler), pid);

		assert!(DeliveryReceipts::<T>::get(&pid).is_empty());
	}

	#[benchmark]
	fn flag_campaign() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);
		let reporter = funded_account::<T>("reporter", 1);

		#[extrinsic_call]
		_(RawOrigin::Signed(reporter), id);

		assert_eq!(Campaigns::<T>::get(id).unwrap().review_status, AdReviewStatus::Flagged);
	}

	#[benchmark]
	fn claim_ad_revenue() {
		let admin = funded_account::<T>("admin", 0);
		let pid = placement_id(1);
		let amount = T::MinBidPerMille::get().saturating_mul(10u32.into());
		PlacementClaimable::<T>::insert(&pid, amount);
		let treasury = T::TreasuryAccount::get();
		let _ = T::Currency::deposit_creating(&treasury, amount.saturating_mul(2u32.into()));

		#[extrinsic_call]
		_(RawOrigin::Signed(admin), pid, Zero::zero());

		assert!(PlacementClaimable::<T>::get(&pid).is_zero());
	}

	#[benchmark]
	fn advertiser_block_placement() {
		let caller = funded_account::<T>("caller", 0);
		let pid = placement_id(1);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), pid);
	}

	#[benchmark]
	fn advertiser_unblock_placement() {
		let caller = funded_account::<T>("caller", 0);
		let pid = placement_id(1);
		let _ = AdvertiserBlacklist::<T>::try_mutate(&caller, |list| {
			list.try_push(pid)
		});

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), pid);
	}

	#[benchmark]
	fn advertiser_prefer_placement() {
		let caller = funded_account::<T>("caller", 0);
		let pid = placement_id(1);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), pid);
	}

	#[benchmark]
	fn advertiser_unprefer_placement() {
		let caller = funded_account::<T>("caller", 0);
		let pid = placement_id(1);
		let _ = AdvertiserWhitelist::<T>::try_mutate(&caller, |list| {
			list.try_push(pid)
		});

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), pid);
	}

	#[benchmark]
	fn placement_block_advertiser() {
		let admin = funded_account::<T>("admin", 0);
		let pid = placement_id(1);
		let advertiser = funded_account::<T>("advertiser", 1);

		#[extrinsic_call]
		_(RawOrigin::Signed(admin), pid, advertiser);
	}

	#[benchmark]
	fn placement_unblock_advertiser() {
		let admin = funded_account::<T>("admin", 0);
		let pid = placement_id(1);
		let advertiser = funded_account::<T>("advertiser", 1);
		let _ = PlacementBlacklist::<T>::try_mutate(&pid, |list| {
			list.try_push(advertiser.clone())
		});

		#[extrinsic_call]
		_(RawOrigin::Signed(admin), pid, advertiser);
	}

	#[benchmark]
	fn placement_prefer_advertiser() {
		let admin = funded_account::<T>("admin", 0);
		let pid = placement_id(1);
		let advertiser = funded_account::<T>("advertiser", 1);

		#[extrinsic_call]
		_(RawOrigin::Signed(admin), pid, advertiser);
	}

	#[benchmark]
	fn placement_unprefer_advertiser() {
		let admin = funded_account::<T>("admin", 0);
		let pid = placement_id(1);
		let advertiser = funded_account::<T>("advertiser", 1);
		let _ = PlacementWhitelist::<T>::try_mutate(&pid, |list| {
			list.try_push(advertiser.clone())
		});

		#[extrinsic_call]
		_(RawOrigin::Signed(admin), pid, advertiser);
	}

	#[benchmark]
	fn flag_placement() {
		let reporter = funded_account::<T>("reporter", 0);
		let pid = placement_id(1);

		#[extrinsic_call]
		_(RawOrigin::Signed(reporter), pid);

		assert_eq!(PlacementFlagCount::<T>::get(&pid), 1);
	}

	#[benchmark]
	fn slash_placement() {
		let pid = placement_id(1);
		let amount = T::MinBidPerMille::get().saturating_mul(100u32.into());
		PlacementClaimable::<T>::insert(&pid, amount);
		let reporter = funded_account::<T>("reporter", 0);

		#[extrinsic_call]
		_(RawOrigin::Root, pid, reporter);

		assert_eq!(SlashCount::<T>::get(&pid), 1);
	}

	#[benchmark]
	fn register_private_ad() {
		let admin = funded_account::<T>("admin", 0);
		let pid = placement_id(1);
		let treasury = T::TreasuryAccount::get();
		let _ = T::Currency::deposit_creating(&treasury, T::PrivateAdRegistrationFee::get());

		#[extrinsic_call]
		_(RawOrigin::Signed(admin), pid, 1u32);

		assert_eq!(PrivateAdCount::<T>::get(&pid), 1);
	}

	#[benchmark]
	fn resume_campaign() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);
		Campaigns::<T>::mutate(id, |c| { c.as_mut().unwrap().status = CampaignStatus::Paused; });

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id);

		assert_eq!(Campaigns::<T>::get(id).unwrap().status, CampaignStatus::Active);
	}

	#[benchmark]
	fn expire_campaign() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_approved_campaign::<T>(&caller);
		let expires = Campaigns::<T>::get(id).unwrap().expires_at;
		frame_system::Pallet::<T>::set_block_number(expires.saturating_add(1u32.into()));
		let anyone = funded_account::<T>("anyone", 1);

		#[extrinsic_call]
		_(RawOrigin::Signed(anyone), id);

		assert_eq!(Campaigns::<T>::get(id).unwrap().status, CampaignStatus::Expired);
	}

	#[benchmark]
	fn update_campaign() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);
		let new_text: BoundedVec<u8, T::MaxAdTextLength> =
			b"Updated".to_vec().try_into().unwrap();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id, Some(new_text), None, None, None, None, None);
	}

	#[benchmark]
	fn extend_campaign_expiry() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);
		let new_expiry = Campaigns::<T>::get(id).unwrap().expires_at.saturating_add(1000u32.into());

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id, new_expiry);
	}

	#[benchmark]
	fn force_cancel_campaign() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);

		#[extrinsic_call]
		_(RawOrigin::Root, id);

		assert_eq!(Campaigns::<T>::get(id).unwrap().status, CampaignStatus::Cancelled);
	}

	#[benchmark]
	fn unban_placement() {
		let pid = placement_id(1);
		BannedPlacements::<T>::insert(&pid, true);
		SlashCount::<T>::insert(&pid, 3);

		#[extrinsic_call]
		_(RawOrigin::Root, pid);

		assert!(!BannedPlacements::<T>::get(&pid));
	}

	#[benchmark]
	fn reset_slash_count() {
		let pid = placement_id(1);
		SlashCount::<T>::insert(&pid, 2);

		#[extrinsic_call]
		_(RawOrigin::Root, pid);

		assert_eq!(SlashCount::<T>::get(&pid), 0);
	}

	#[benchmark]
	fn clear_placement_flags() {
		let pid = placement_id(1);
		PlacementFlagCount::<T>::insert(&pid, 5);

		#[extrinsic_call]
		_(RawOrigin::Root, pid);

		assert_eq!(PlacementFlagCount::<T>::get(&pid), 0);
	}

	#[benchmark]
	fn suspend_campaign() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);

		#[extrinsic_call]
		_(RawOrigin::Root, id);

		assert_eq!(Campaigns::<T>::get(id).unwrap().status, CampaignStatus::Suspended);
	}

	#[benchmark]
	fn unsuspend_campaign() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);
		Campaigns::<T>::mutate(id, |c| { c.as_mut().unwrap().status = CampaignStatus::Suspended; });

		#[extrinsic_call]
		_(RawOrigin::Root, id);

		assert_eq!(Campaigns::<T>::get(id).unwrap().status, CampaignStatus::Active);
	}

	#[benchmark]
	fn report_approved_campaign() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_approved_campaign::<T>(&caller);
		let reporter = funded_account::<T>("reporter", 1);

		#[extrinsic_call]
		_(RawOrigin::Signed(reporter), id);

		assert_eq!(CampaignReportCount::<T>::get(id), 1);
	}

	#[benchmark]
	fn resubmit_campaign() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);
		// Set to Rejected + unreserve (simulate reject flow)
		let escrow = CampaignEscrow::<T>::take(id);
		T::Currency::unreserve(&caller, escrow);
		Campaigns::<T>::mutate(id, |c| {
			let c = c.as_mut().unwrap();
			c.review_status = AdReviewStatus::Rejected;
			c.status = CampaignStatus::Cancelled;
		});
		let text: BoundedVec<u8, T::MaxAdTextLength> =
			b"Resubmitted".to_vec().try_into().unwrap();
		let url: BoundedVec<u8, T::MaxAdUrlLength> =
			b"https://resubmit.com".to_vec().try_into().unwrap();
		let budget = T::MinBidPerMille::get().saturating_mul(500u32.into());

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id, text, url, budget);

		assert_eq!(Campaigns::<T>::get(id).unwrap().review_status, AdReviewStatus::Pending);
	}

	#[benchmark]
	fn set_placement_delivery_types() {
		let admin = funded_account::<T>("admin", 0);
		let pid = placement_id(1);

		#[extrinsic_call]
		_(RawOrigin::Signed(admin), pid, 0b010u8);

		assert_eq!(PlacementDeliveryTypes::<T>::get(&pid), 0b010);
	}

	#[benchmark]
	fn unregister_private_ad() {
		let admin = funded_account::<T>("admin", 0);
		let pid = placement_id(1);
		PrivateAdCount::<T>::insert(&pid, 5);

		#[extrinsic_call]
		_(RawOrigin::Signed(admin), pid, 2u32);

		assert_eq!(PrivateAdCount::<T>::get(&pid), 3);
	}

	#[benchmark]
	fn cleanup_campaign() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);
		let escrow = CampaignEscrow::<T>::take(id);
		T::Currency::unreserve(&caller, escrow);
		Campaigns::<T>::mutate(id, |c| { c.as_mut().unwrap().status = CampaignStatus::Cancelled; });
		CampaignEscrow::<T>::remove(id);
		let anyone = funded_account::<T>("anyone", 1);

		#[extrinsic_call]
		_(RawOrigin::Signed(anyone), id);

		assert!(Campaigns::<T>::get(id).is_none());
	}

	#[benchmark]
	fn force_settle_era_ads() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_approved_campaign::<T>(&caller);
		let pid = placement_id(1);
		let idx = seed_receipt::<T>(id, pid, &caller);
		ReceiptConfirmation::<T>::insert((id, pid, idx), ReceiptStatus::Confirmed);

		#[extrinsic_call]
		_(RawOrigin::Root, pid);

		assert!(DeliveryReceipts::<T>::get(&pid).is_empty());
	}

	#[benchmark]
	fn set_campaign_targets() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);
		let targets: BoundedVec<PlacementId, T::MaxTargetsPerCampaign> =
			BoundedVec::try_from(sp_runtime::Vec::from([placement_id(1)])).unwrap();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id, targets);

		assert!(CampaignTargets::<T>::get(id).is_some());
	}

	#[benchmark]
	fn clear_campaign_targets() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);
		let targets: BoundedVec<PlacementId, T::MaxTargetsPerCampaign> =
			BoundedVec::try_from(sp_runtime::Vec::from([placement_id(1)])).unwrap();
		CampaignTargets::<T>::insert(id, targets);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id);

		assert!(CampaignTargets::<T>::get(id).is_none());
	}

	#[benchmark]
	fn set_campaign_multiplier() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id, 200u32);

		assert_eq!(CampaignMultiplier::<T>::get(id), Some(200));
	}

	#[benchmark]
	fn set_placement_multiplier() {
		let admin = funded_account::<T>("admin", 0);
		let pid = placement_id(1);

		#[extrinsic_call]
		_(RawOrigin::Signed(admin), pid, 150u32);

		assert_eq!(PlacementMultiplier::<T>::get(&pid), Some(150));
	}

	#[benchmark]
	fn set_placement_approval_required() {
		let admin = funded_account::<T>("admin", 0);
		let pid = placement_id(1);

		#[extrinsic_call]
		_(RawOrigin::Signed(admin), pid, true);

		assert!(PlacementRequiresApproval::<T>::get(&pid));
	}

	#[benchmark]
	fn approve_campaign_for_placement() {
		let admin = funded_account::<T>("admin", 0);
		let caller = funded_account::<T>("caller", 1);
		register_seed_advertiser::<T>(&caller);
		let id = seed_campaign::<T>(&caller);
		let pid = placement_id(1);

		#[extrinsic_call]
		_(RawOrigin::Signed(admin), pid, id);

		assert!(PlacementCampaignApproval::<T>::get(&pid, id));
	}

	#[benchmark]
	fn reject_campaign_for_placement() {
		let admin = funded_account::<T>("admin", 0);
		let pid = placement_id(1);
		PlacementCampaignApproval::<T>::insert(&pid, 0u64, true);

		#[extrinsic_call]
		_(RawOrigin::Signed(admin), pid, 0u64);

		assert!(!PlacementCampaignApproval::<T>::get(&pid, 0u64));
	}

	#[benchmark]
	fn confirm_receipt() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_approved_campaign::<T>(&caller);
		let pid = placement_id(1);
		let idx = seed_receipt::<T>(id, pid, &caller);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id, pid, idx);

		assert_eq!(ReceiptConfirmation::<T>::get((id, pid, idx)), Some(ReceiptStatus::Confirmed));
	}

	#[benchmark]
	fn dispute_receipt() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_approved_campaign::<T>(&caller);
		let pid = placement_id(1);
		let idx = seed_receipt::<T>(id, pid, &caller);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id, pid, idx);

		assert_eq!(ReceiptConfirmation::<T>::get((id, pid, idx)), Some(ReceiptStatus::Disputed));
	}

	#[benchmark]
	fn auto_confirm_receipt() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_approved_campaign::<T>(&caller);
		let pid = placement_id(1);
		let idx = seed_receipt::<T>(id, pid, &caller);
		let submitted = ReceiptSubmittedAt::<T>::get((id, pid, idx)).unwrap();
		let window = T::ReceiptConfirmationWindow::get();
		frame_system::Pallet::<T>::set_block_number(submitted.saturating_add(window).saturating_add(1u32.into()));
		let anyone = funded_account::<T>("anyone", 1);

		#[extrinsic_call]
		_(RawOrigin::Signed(anyone), id, pid, idx);

		assert_eq!(ReceiptConfirmation::<T>::get((id, pid, idx)), Some(ReceiptStatus::AutoConfirmed));
	}

	#[benchmark]
	fn register_advertiser() {
		let referrer = funded_account::<T>("referrer", 0);
		register_seed_advertiser::<T>(&referrer);
		let new_adv = funded_account::<T>("new_adv", 1);

		#[extrinsic_call]
		_(RawOrigin::Signed(new_adv.clone()), referrer);

		assert!(AdvertiserRegisteredAt::<T>::contains_key(&new_adv));
	}

	#[benchmark]
	fn force_register_advertiser() {
		let new_adv = funded_account::<T>("new_adv", 0);

		#[extrinsic_call]
		_(RawOrigin::Root, new_adv.clone());

		assert!(AdvertiserRegisteredAt::<T>::contains_key(&new_adv));
	}

	#[benchmark]
	fn claim_referral_earnings() {
		let caller = funded_account::<T>("caller", 0);
		let amount = T::MinBidPerMille::get().saturating_mul(10u32.into());
		ReferrerClaimable::<T>::insert(&caller, amount);
		let treasury = T::TreasuryAccount::get();
		let _ = T::Currency::deposit_creating(&treasury, amount.saturating_mul(2u32.into()));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()));

		assert!(ReferrerClaimable::<T>::get(&caller).is_zero());
	}

	#[benchmark]
	fn submit_click_receipt() {
		let caller = funded_account::<T>("caller", 0);
		register_seed_advertiser::<T>(&caller);
		let id = seed_cpc_campaign::<T>(&caller);
		let pid = placement_id(1);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id, pid, 50u32, 50u32);

		assert_eq!(DeliveryReceipts::<T>::get(&pid).len(), 1);
	}

	impl_benchmark_test_suite!(
		Pallet,
		crate::mock::new_test_ext(),
		crate::mock::Test,
	);
}
