//! Benchmarking setup for pallet-ads-entity
//!
//! Every benchmark seeds only this pallet's own storage items.
//! External trait dependencies (`EntityProvider`, `ShopProvider`) are satisfied
//! by a `BenchmarkHelper` trait that the runtime must implement for real
//! `EntityProvider`/`ShopProvider` setup, or by seeding storage directly for
//! extrinsics that only read this pallet's own storage.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
#[allow(unused)]
use crate::Pallet as AdsEntity;
use frame_benchmarking::v2::*;
use frame_support::traits::ReservableCurrency;
use frame_system::RawOrigin;
use pallet_ads_primitives::*;

/// Helper: seed a registered placement directly into storage.
fn seed_placement<T: Config>(
	placement_id: &PlacementId,
	entity_id: u64,
	shop_id: u64,
	level: PlacementLevel,
	registered_by: &T::AccountId,
) {
	let info = AdPlacementInfo::<T> {
		entity_id,
		shop_id,
		level,
		daily_impression_cap: T::DefaultDailyImpressionCap::get(),
		daily_click_cap: 0,
		registered_by: registered_by.clone(),
		registered_at: frame_system::Pallet::<T>::block_number(),
		active: true,
	};
	pallet::RegisteredPlacements::<T>::insert(placement_id, info);
	pallet::PlacementDeposits::<T>::insert(placement_id, T::AdPlacementDeposit::get());

	let mut ids = pallet::EntityPlacementIds::<T>::get(entity_id);
	let _ = ids.try_push(*placement_id);
	pallet::EntityPlacementIds::<T>::insert(entity_id, ids);
}

/// Trait that the runtime must implement for benchmarks requiring external state.
#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId> {
	/// Set up Entity so that:
	/// - entity_id exists, is active, not locked
	/// - caller is entity owner (and optionally admin)
	fn setup_entity(caller: &AccountId, entity_id: u64);

	/// Set up Entity + Shop so that:
	/// - entity_id exists, is active, not locked
	/// - shop_id exists, is active, belongs to entity_id
	/// - caller is entity owner or shop manager
	fn setup_entity_and_shop(caller: &AccountId, entity_id: u64, shop_id: u64);

	/// Fund the account with enough balance.
	fn fund_account(account: &AccountId);
}

/// Fallback for test mock and runtime.
#[cfg(feature = "runtime-benchmarks")]
impl<AccountId> BenchmarkHelper<AccountId> for () {
	fn setup_entity(_caller: &AccountId, _entity_id: u64) {}
	fn setup_entity_and_shop(_caller: &AccountId, _entity_id: u64, _shop_id: u64) {}
	fn fund_account(_account: &AccountId) {}
}

#[benchmarks]
mod benchmarks {
	use super::*;

	// ================================================================
	// register_entity_placement (call_index 0)
	// ================================================================
	#[benchmark]
	fn register_entity_placement() {
		let entity_id: u64 = 1;
		let caller: T::AccountId = whitelisted_caller();
		T::BenchmarkHelper::setup_entity(&caller, entity_id);
		T::BenchmarkHelper::fund_account(&caller);

		let placement_id = entity_placement_id(entity_id);
		// Ensure not already registered
		pallet::RegisteredPlacements::<T>::remove(&placement_id);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), entity_id);

		assert!(pallet::RegisteredPlacements::<T>::contains_key(&placement_id));
	}

	// ================================================================
	// register_shop_placement (call_index 1)
	// ================================================================
	#[benchmark]
	fn register_shop_placement() {
		let entity_id: u64 = 1;
		let shop_id: u64 = 10;
		let caller: T::AccountId = whitelisted_caller();
		T::BenchmarkHelper::setup_entity_and_shop(&caller, entity_id, shop_id);
		T::BenchmarkHelper::fund_account(&caller);

		let placement_id = shop_placement_id(shop_id);
		pallet::RegisteredPlacements::<T>::remove(&placement_id);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), entity_id, shop_id);

		assert!(pallet::RegisteredPlacements::<T>::contains_key(&placement_id));
	}

	// ================================================================
	// deregister_placement (call_index 2)
	// ================================================================
	#[benchmark]
	fn deregister_placement() {
		let entity_id: u64 = 1;
		let caller: T::AccountId = whitelisted_caller();
		T::BenchmarkHelper::setup_entity(&caller, entity_id);
		T::BenchmarkHelper::fund_account(&caller);

		let placement_id = entity_placement_id(entity_id);
		seed_placement::<T>(&placement_id, entity_id, 0, PlacementLevel::Entity, &caller);
		// Reserve deposit matching what was stored
		let deposit = T::AdPlacementDeposit::get();
		let _ = T::Currency::reserve(&caller, deposit);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), placement_id);

		assert!(!pallet::RegisteredPlacements::<T>::contains_key(&placement_id));
	}

	// ================================================================
	// set_placement_active (call_index 3)
	// ================================================================
	#[benchmark]
	fn set_placement_active() {
		let entity_id: u64 = 1;
		let caller: T::AccountId = whitelisted_caller();
		T::BenchmarkHelper::setup_entity(&caller, entity_id);

		let placement_id = entity_placement_id(entity_id);
		seed_placement::<T>(&placement_id, entity_id, 0, PlacementLevel::Entity, &caller);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), placement_id, false);

		let info = pallet::RegisteredPlacements::<T>::get(&placement_id).unwrap();
		assert!(!info.active);
	}

	// ================================================================
	// set_impression_cap (call_index 4)
	// ================================================================
	#[benchmark]
	fn set_impression_cap() {
		let entity_id: u64 = 1;
		let caller: T::AccountId = whitelisted_caller();
		T::BenchmarkHelper::setup_entity(&caller, entity_id);

		let placement_id = entity_placement_id(entity_id);
		seed_placement::<T>(&placement_id, entity_id, 0, PlacementLevel::Entity, &caller);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), placement_id, 5000u32);

		let info = pallet::RegisteredPlacements::<T>::get(&placement_id).unwrap();
		assert_eq!(info.daily_impression_cap, 5000);
	}

	// ================================================================
	// set_entity_ad_share (call_index 5)
	// ================================================================
	#[benchmark]
	fn set_entity_ad_share() {
		let entity_id: u64 = 1;
		let caller: T::AccountId = whitelisted_caller();
		T::BenchmarkHelper::setup_entity(&caller, entity_id);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), entity_id, 5000u16);

		assert_eq!(pallet::EntityAdShareBps::<T>::get(entity_id), 5000);
	}

	// ================================================================
	// ban_entity (call_index 6) — Root
	// ================================================================
	#[benchmark]
	fn ban_entity() {
		let entity_id: u64 = 1;
		let caller: T::AccountId = whitelisted_caller();
		T::BenchmarkHelper::setup_entity(&caller, entity_id);
		// Ensure not already banned
		pallet::BannedEntities::<T>::remove(entity_id);

		#[extrinsic_call]
		_(RawOrigin::Root, entity_id);

		assert!(pallet::BannedEntities::<T>::get(entity_id));
	}

	// ================================================================
	// unban_entity (call_index 7) — Root
	// ================================================================
	#[benchmark]
	fn unban_entity() {
		let entity_id: u64 = 1;
		let caller: T::AccountId = whitelisted_caller();
		T::BenchmarkHelper::setup_entity(&caller, entity_id);
		pallet::BannedEntities::<T>::insert(entity_id, true);

		#[extrinsic_call]
		_(RawOrigin::Root, entity_id);

		assert!(!pallet::BannedEntities::<T>::get(entity_id));
	}

	// ================================================================
	// set_click_cap (call_index 8)
	// ================================================================
	#[benchmark]
	fn set_click_cap() {
		let entity_id: u64 = 1;
		let caller: T::AccountId = whitelisted_caller();
		T::BenchmarkHelper::setup_entity(&caller, entity_id);

		let placement_id = entity_placement_id(entity_id);
		seed_placement::<T>(&placement_id, entity_id, 0, PlacementLevel::Entity, &caller);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), placement_id, 500u32);

		let info = pallet::RegisteredPlacements::<T>::get(&placement_id).unwrap();
		assert_eq!(info.daily_click_cap, 500);
	}

	impl_benchmark_test_suite!(
		AdsEntity,
		crate::mock::new_test_ext(),
		crate::mock::Test,
	);
}
