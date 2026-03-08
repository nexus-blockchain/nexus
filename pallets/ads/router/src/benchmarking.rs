//! Lightweight overhead measurement for the AdsRouter routing layer.
//!
//! The router adds exactly ONE `contains_key` storage read per trait call
//! to determine Entity vs GroupRobot path. This module provides test-based
//! benchmarks to document that overhead.

#![cfg(test)]

use crate::mock::*;
use crate::AdsRouter;
use frame_support::assert_ok;
use pallet_ads_primitives::*;
use sp_io::TestExternalities;

const UNIT: u128 = 1_000_000_000_000;

fn setup_ext_with_entity() -> (TestExternalities, PlacementId) {
	let mut ext = new_test_ext();
	let pid = ext.execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		entity_placement_id(1)
	});
	(ext, pid)
}

fn setup_ext_with_grouprobot() -> (TestExternalities, PlacementId) {
	let mut ext = new_test_ext();
	let pid = ext.execute_with(|| {
		let ch = community_placement_id(1);
		assert_ok!(AdsGroupRobot::stake_for_ads(
			RuntimeOrigin::signed(ALICE), ch, 100 * UNIT,
		));
		ch
	});
	(ext, pid)
}

#[test]
fn bench_delivery_verifier_entity_path() {
	let (mut ext, pid) = setup_ext_with_entity();
	ext.execute_with(|| {
		let start = std::time::Instant::now();
		for _ in 0..1000 {
			let _ = <AdsRouter<Test> as DeliveryVerifier<u64>>::verify_and_cap_audience(
				&ALICE, &pid, 10, None,
			);
		}
		let elapsed = start.elapsed();
		println!(
			"[bench] DeliveryVerifier Entity path: 1000 calls in {:?} ({:?}/call)",
			elapsed,
			elapsed / 1000,
		);
	});
}

#[test]
fn bench_delivery_verifier_grouprobot_path() {
	let (mut ext, pid) = setup_ext_with_grouprobot();
	ext.execute_with(|| {
		let start = std::time::Instant::now();
		for _ in 0..1000 {
			let _ = <AdsRouter<Test> as DeliveryVerifier<u64>>::verify_and_cap_audience(
				&TEE_OPERATOR, &pid, 10, None,
			);
		}
		let elapsed = start.elapsed();
		println!(
			"[bench] DeliveryVerifier GroupRobot path: 1000 calls in {:?} ({:?}/call)",
			elapsed,
			elapsed / 1000,
		);
	});
}

#[test]
fn bench_placement_admin_entity_path() {
	let (mut ext, pid) = setup_ext_with_entity();
	ext.execute_with(|| {
		let start = std::time::Instant::now();
		for _ in 0..1000 {
			let _ = <AdsRouter<Test> as PlacementAdminProvider<u64>>::placement_admin(&pid);
		}
		let elapsed = start.elapsed();
		println!(
			"[bench] PlacementAdminProvider Entity path: 1000 calls in {:?} ({:?}/call)",
			elapsed,
			elapsed / 1000,
		);
	});
}

#[test]
fn bench_revenue_distributor_entity_path() {
	let (mut ext, pid) = setup_ext_with_entity();
	ext.execute_with(|| {
		let start = std::time::Instant::now();
		for _ in 0..1000 {
			let _ = <AdsRouter<Test> as RevenueDistributor<u64, u128>>::distribute(
				&pid, 10_000, &BOB,
			);
		}
		let elapsed = start.elapsed();
		println!(
			"[bench] RevenueDistributor Entity path: 1000 calls in {:?} ({:?}/call)",
			elapsed,
			elapsed / 1000,
		);
	});
}

#[test]
fn bench_revenue_distributor_grouprobot_path() {
	let (mut ext, pid) = setup_ext_with_grouprobot();
	ext.execute_with(|| {
		let start = std::time::Instant::now();
		for _ in 0..1000 {
			let _ = <AdsRouter<Test> as RevenueDistributor<u64, u128>>::distribute(
				&pid, 10_000, &BOB,
			);
		}
		let elapsed = start.elapsed();
		println!(
			"[bench] RevenueDistributor GroupRobot path: 1000 calls in {:?} ({:?}/call)",
			elapsed,
			elapsed / 1000,
		);
	});
}
