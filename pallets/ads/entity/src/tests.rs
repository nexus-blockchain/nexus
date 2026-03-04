use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};

// ============================================================================
// register_entity_placement
// ============================================================================

#[test]
fn register_entity_placement_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE),
			1,
		));

		let pid = entity_placement_id(1);
		let info = pallet::RegisteredPlacements::<Test>::get(&pid).unwrap();
		assert_eq!(info.entity_id, 1);
		assert_eq!(info.shop_id, 0);
		assert_eq!(info.level, PlacementLevel::Entity);
		assert!(info.active);
		assert_eq!(info.daily_impression_cap, 1000);
		assert_eq!(info.registered_by, ALICE);

		// 保证金已扣除
		assert_eq!(pallet::PlacementDeposits::<Test>::get(&pid), 100);

		// Entity placement IDs 列表
		let ids = pallet::EntityPlacementIds::<Test>::get(1);
		assert_eq!(ids.len(), 1);
		assert_eq!(ids[0], pid);
	});
}

#[test]
fn register_entity_placement_by_admin_works() {
	new_test_ext().execute_with(|| {
		// BOB is admin for entity_id=1
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(BOB),
			1,
		));
		let pid = entity_placement_id(1);
		assert!(pallet::RegisteredPlacements::<Test>::contains_key(&pid));
	});
}

#[test]
fn register_entity_placement_fails_not_admin() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsEntity::register_entity_placement(RuntimeOrigin::signed(CHARLIE), 1),
			Error::<Test>::NotEntityAdmin
		);
	});
}

#[test]
fn register_entity_placement_fails_entity_not_found() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsEntity::register_entity_placement(RuntimeOrigin::signed(ALICE), 99),
			Error::<Test>::EntityNotFound
		);
	});
}

#[test]
fn register_entity_placement_fails_entity_not_active() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsEntity::register_entity_placement(RuntimeOrigin::signed(ALICE), 2),
			Error::<Test>::EntityNotActive
		);
	});
}

#[test]
fn register_entity_placement_fails_already_registered() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		assert_noop!(
			AdsEntity::register_entity_placement(RuntimeOrigin::signed(ALICE), 1),
			Error::<Test>::PlacementAlreadyRegistered
		);
	});
}

#[test]
fn register_entity_placement_fails_banned() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::ban_entity(RuntimeOrigin::root(), 1));
		assert_noop!(
			AdsEntity::register_entity_placement(RuntimeOrigin::signed(ALICE), 1),
			Error::<Test>::EntityBanned
		);
	});
}

// ============================================================================
// register_shop_placement
// ============================================================================

#[test]
fn register_shop_placement_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_shop_placement(
			RuntimeOrigin::signed(ALICE),
			1,
			10,
		));

		let pid = shop_placement_id(10);
		let info = pallet::RegisteredPlacements::<Test>::get(&pid).unwrap();
		assert_eq!(info.entity_id, 1);
		assert_eq!(info.shop_id, 10);
		assert_eq!(info.level, PlacementLevel::Shop);
		assert!(info.active);
	});
}

#[test]
fn register_shop_placement_by_manager_works() {
	new_test_ext().execute_with(|| {
		// CHARLIE is shop manager for shop_id=10
		assert_ok!(AdsEntity::register_shop_placement(
			RuntimeOrigin::signed(CHARLIE),
			1,
			10,
		));
	});
}

#[test]
fn register_shop_placement_fails_shop_not_found() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsEntity::register_shop_placement(RuntimeOrigin::signed(ALICE), 1, 99),
			Error::<Test>::ShopNotFound
		);
	});
}

#[test]
fn register_shop_placement_fails_shop_not_active() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsEntity::register_shop_placement(RuntimeOrigin::signed(ALICE), 1, 20),
			Error::<Test>::ShopNotActive
		);
	});
}

#[test]
fn register_shop_placement_fails_entity_mismatch() {
	new_test_ext().execute_with(|| {
		// shop_id=10 belongs to entity_id=1, but we claim entity_id=2
		assert_noop!(
			AdsEntity::register_shop_placement(RuntimeOrigin::signed(ALICE), 2, 10),
			Error::<Test>::EntityNotActive  // entity 2 is not active
		);
	});
}

// ============================================================================
// deregister_placement
// ============================================================================

#[test]
fn deregister_placement_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));

		let pid = entity_placement_id(1);
		let balance_before = Balances::free_balance(ALICE);

		assert_ok!(AdsEntity::deregister_placement(
			RuntimeOrigin::signed(ALICE),
			pid,
		));

		assert!(!pallet::RegisteredPlacements::<Test>::contains_key(&pid));
		// 保证金已退还
		assert_eq!(Balances::free_balance(ALICE), balance_before + 100);
		// 列表已清理
		assert!(pallet::EntityPlacementIds::<Test>::get(1).is_empty());
	});
}

#[test]
fn deregister_placement_fails_not_registered() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsEntity::deregister_placement(
				RuntimeOrigin::signed(ALICE),
				[0u8; 32],
			),
			Error::<Test>::PlacementNotRegistered
		);
	});
}

#[test]
fn deregister_placement_fails_not_admin() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let pid = entity_placement_id(1);

		// CHARLIE is neither owner, admin, nor registrant
		assert_noop!(
			AdsEntity::deregister_placement(RuntimeOrigin::signed(CHARLIE), pid),
			Error::<Test>::NotEntityAdmin
		);
	});
}

// ============================================================================
// set_placement_active
// ============================================================================

#[test]
fn set_placement_active_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let pid = entity_placement_id(1);

		// 禁用
		assert_ok!(AdsEntity::set_placement_active(
			RuntimeOrigin::signed(ALICE), pid, false,
		));
		let info = pallet::RegisteredPlacements::<Test>::get(&pid).unwrap();
		assert!(!info.active);

		// 重新启用
		assert_ok!(AdsEntity::set_placement_active(
			RuntimeOrigin::signed(ALICE), pid, true,
		));
		let info = pallet::RegisteredPlacements::<Test>::get(&pid).unwrap();
		assert!(info.active);
	});
}

// ============================================================================
// set_impression_cap
// ============================================================================

#[test]
fn set_impression_cap_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let pid = entity_placement_id(1);

		assert_ok!(AdsEntity::set_impression_cap(
			RuntimeOrigin::signed(ALICE), pid, 5000,
		));
		let info = pallet::RegisteredPlacements::<Test>::get(&pid).unwrap();
		assert_eq!(info.daily_impression_cap, 5000);
	});
}

// ============================================================================
// set_entity_ad_share
// ============================================================================

#[test]
fn set_entity_ad_share_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::set_entity_ad_share(
			RuntimeOrigin::signed(ALICE), 1, 8000,
		));
		assert_eq!(pallet::EntityAdShareBps::<Test>::get(1), 8000);
	});
}

#[test]
fn set_entity_ad_share_fails_invalid_bps() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsEntity::set_entity_ad_share(RuntimeOrigin::signed(ALICE), 1, 10001),
			Error::<Test>::InvalidShareBps
		);
	});
}

#[test]
fn set_entity_ad_share_fails_not_owner() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsEntity::set_entity_ad_share(RuntimeOrigin::signed(CHARLIE), 1, 9000),
			Error::<Test>::NotEntityAdmin
		);
	});
}

// ============================================================================
// ban / unban entity
// ============================================================================

#[test]
fn ban_unban_entity_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::ban_entity(RuntimeOrigin::root(), 1));
		assert!(pallet::BannedEntities::<Test>::get(1));

		assert_ok!(AdsEntity::unban_entity(RuntimeOrigin::root(), 1));
		assert!(!pallet::BannedEntities::<Test>::get(1));
	});
}

#[test]
fn ban_entity_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsEntity::ban_entity(RuntimeOrigin::signed(ALICE), 1),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ============================================================================
// DeliveryVerifier trait 实现
// ============================================================================

#[test]
fn delivery_verifier_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let pid = entity_placement_id(1);

		// ALICE (entity owner) 提交展示量
		let result = <AdsEntity as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&ALICE, &pid, 500, None,
		);
		assert_ok!(&result);
		assert_eq!(result.unwrap(), 500);

		// 展示量已记录
		assert_eq!(pallet::DailyImpressions::<Test>::get(&pid), 500);
		assert_eq!(pallet::TotalImpressions::<Test>::get(&pid), 500);
	});
}

#[test]
fn delivery_verifier_caps_audience() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let pid = entity_placement_id(1);

		// 默认 cap = 1000, 提交 1500 → 只算 1000
		let result = <AdsEntity as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&ALICE, &pid, 1500, None,
		);
		assert_eq!(result.unwrap(), 1000);

		// 再提交 500 → 剩余 0, 应该失败
		let result = <AdsEntity as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&ALICE, &pid, 500, None,
		);
		assert_noop!(result, Error::<Test>::DailyImpressionCapReached);
	});
}

#[test]
fn delivery_verifier_fails_not_registered() {
	new_test_ext().execute_with(|| {
		let pid = entity_placement_id(1);
		let result = <AdsEntity as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&ALICE, &pid, 100, None,
		);
		assert_noop!(result, Error::<Test>::PlacementNotRegistered);
	});
}

#[test]
fn delivery_verifier_fails_not_active() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let pid = entity_placement_id(1);

		// 禁用广告位
		assert_ok!(AdsEntity::set_placement_active(
			RuntimeOrigin::signed(ALICE), pid, false,
		));

		let result = <AdsEntity as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&ALICE, &pid, 100, None,
		);
		assert_noop!(result, Error::<Test>::PlacementNotActive);
	});
}

#[test]
fn delivery_verifier_fails_banned() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let pid = entity_placement_id(1);

		assert_ok!(AdsEntity::ban_entity(RuntimeOrigin::root(), 1));

		let result = <AdsEntity as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&ALICE, &pid, 100, None,
		);
		assert_noop!(result, Error::<Test>::EntityBanned);
	});
}

#[test]
fn delivery_verifier_fails_not_authorized() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let pid = entity_placement_id(1);

		// 用户 4 既不是 owner 也不是 admin
		let result = <AdsEntity as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&4u64, &pid, 100, None,
		);
		assert_noop!(result, Error::<Test>::NotEntityAdmin);
	});
}

#[test]
fn delivery_verifier_shop_manager_authorized() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_shop_placement(
			RuntimeOrigin::signed(ALICE), 1, 10,
		));
		let pid = shop_placement_id(10);

		// CHARLIE 是 shop manager, 可以提交展示量
		let result = <AdsEntity as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&CHARLIE, &pid, 200, None,
		);
		assert_ok!(&result);
		assert_eq!(result.unwrap(), 200);
	});
}

// ============================================================================
// PlacementAdminProvider trait 实现
// ============================================================================

#[test]
fn placement_admin_entity_level() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let pid = entity_placement_id(1);

		let admin = <AdsEntity as pallet_ads_primitives::PlacementAdminProvider<u64>>::placement_admin(&pid);
		assert_eq!(admin, Some(ALICE));
	});
}

#[test]
fn placement_admin_shop_level() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_shop_placement(
			RuntimeOrigin::signed(ALICE), 1, 10,
		));
		let pid = shop_placement_id(10);

		let admin = <AdsEntity as pallet_ads_primitives::PlacementAdminProvider<u64>>::placement_admin(&pid);
		// shop_owner returns ALICE
		assert_eq!(admin, Some(ALICE));
	});
}

#[test]
fn placement_banned_check() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let pid = entity_placement_id(1);

		assert!(!<AdsEntity as pallet_ads_primitives::PlacementAdminProvider<u64>>::is_placement_banned(&pid));

		assert_ok!(AdsEntity::ban_entity(RuntimeOrigin::root(), 1));
		assert!(<AdsEntity as pallet_ads_primitives::PlacementAdminProvider<u64>>::is_placement_banned(&pid));
	});
}

// ============================================================================
// RevenueDistributor trait 实现
// ============================================================================

#[test]
fn revenue_distributor_default_split() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let pid = entity_placement_id(1);

		// 默认: platform 20%, entity 80%
		let result = <AdsEntity as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&pid, 10_000, &99,
		);
		assert_eq!(result.unwrap().placement_share, 8_000); // 80%
	});
}

#[test]
fn revenue_distributor_custom_split() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let pid = entity_placement_id(1);

		// 自定义: entity 拿 7000 基点 = 70%
		assert_ok!(AdsEntity::set_entity_ad_share(
			RuntimeOrigin::signed(ALICE), 1, 7000,
		));

		let result = <AdsEntity as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&pid, 10_000, &99,
		);
		assert_eq!(result.unwrap().placement_share, 7_000); // 70%
	});
}

// ============================================================================
// Helper functions
// ============================================================================

#[test]
fn placement_id_encoding() {
	let eid = entity_placement_id(1);
	let sid = shop_placement_id(10);
	// 不同输入产生不同 hash
	assert_ne!(eid, sid);
	// 相同输入产生相同 hash
	assert_eq!(entity_placement_id(1), entity_placement_id(1));
	assert_eq!(shop_placement_id(10), shop_placement_id(10));
}

#[test]
fn bps_of_calculation() {
	new_test_ext().execute_with(|| {
		assert_eq!(AdsEntity::bps_of(10_000u128, 2000), 2_000);  // 20%
		assert_eq!(AdsEntity::bps_of(10_000u128, 8000), 8_000);  // 80%
		assert_eq!(AdsEntity::bps_of(10_000u128, 10000), 10_000); // 100%
		assert_eq!(AdsEntity::bps_of(10_000u128, 0), 0);          // 0%
	});
}

#[test]
fn effective_entity_share_defaults() {
	new_test_ext().execute_with(|| {
		// 未自定义: 10000 - 2000(platform) = 8000
		assert_eq!(AdsEntity::effective_entity_share_bps(1), 8000);

		// 自定义后
		pallet::EntityAdShareBps::<Test>::insert(1, 9000u16);
		assert_eq!(AdsEntity::effective_entity_share_bps(1), 9000);
	});
}

// ============================================================================
// Regression: M-ENT2 — set_entity_ad_share capped at 10000 - PlatformAdShareBps
// ============================================================================

#[test]
fn m_ent2_set_entity_ad_share_rejects_exceeding_platform_complement() {
	new_test_ext().execute_with(|| {
		// PlatformAdShareBps = 2000 (20%), so max entity share = 8000
		assert_ok!(AdsEntity::set_entity_ad_share(
			RuntimeOrigin::signed(ALICE), 1, 8000,
		));

		// 8001 exceeds complement → rejected
		assert_noop!(
			AdsEntity::set_entity_ad_share(RuntimeOrigin::signed(ALICE), 1, 8001),
			Error::<Test>::InvalidShareBps
		);

		// 10000 (100%) → rejected
		assert_noop!(
			AdsEntity::set_entity_ad_share(RuntimeOrigin::signed(ALICE), 1, 10000),
			Error::<Test>::InvalidShareBps
		);
	});
}

// ============================================================================
// Regression: M1 — EntityPlacementIds bound matches MaxPlacementsPerEntity
// ============================================================================

#[test]
fn m1_entity_placement_ids_respects_config_max() {
	new_test_ext().execute_with(|| {
		// MaxPlacementsPerEntity = 10 in mock config
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let ids = pallet::EntityPlacementIds::<Test>::get(1);
		assert_eq!(ids.len(), 1);

		// The BoundedVec bound should equal MaxPlacementsPerEntity (10),
		// not the old hardcoded 50.
		assert_eq!(
			<<Test as crate::pallet::Config>::MaxPlacementsPerEntity as frame_support::traits::Get<u32>>::get(),
			10u32,
		);
	});
}

// ============================================================================
// Regression: M3 — ban/unban entity existence + idempotency checks
// ============================================================================

#[test]
fn m3_ban_entity_rejects_nonexistent() {
	new_test_ext().execute_with(|| {
		// entity_id=99 does not exist
		assert_noop!(
			AdsEntity::ban_entity(RuntimeOrigin::root(), 99),
			Error::<Test>::EntityNotFound
		);
	});
}

#[test]
fn m3_ban_entity_rejects_already_banned() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::ban_entity(RuntimeOrigin::root(), 1));
		assert_noop!(
			AdsEntity::ban_entity(RuntimeOrigin::root(), 1),
			Error::<Test>::EntityAlreadyBanned
		);
	});
}

#[test]
fn m3_unban_entity_rejects_nonexistent() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AdsEntity::unban_entity(RuntimeOrigin::root(), 99),
			Error::<Test>::EntityNotFound
		);
	});
}

#[test]
fn m3_unban_entity_rejects_not_banned() {
	new_test_ext().execute_with(|| {
		// entity 1 is not banned → unban should fail
		assert_noop!(
			AdsEntity::unban_entity(RuntimeOrigin::root(), 1),
			Error::<Test>::EntityNotBanned
		);
	});
}

// ============================================================================
// Regression: L3 — set_placement_active state-change detection
// ============================================================================

#[test]
fn l3_set_placement_active_rejects_unchanged() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let pid = entity_placement_id(1);

		// Placement starts active=true, setting true again should fail
		assert_noop!(
			AdsEntity::set_placement_active(RuntimeOrigin::signed(ALICE), pid, true),
			Error::<Test>::PlacementStatusUnchanged
		);

		// Deactivate → succeeds
		assert_ok!(AdsEntity::set_placement_active(
			RuntimeOrigin::signed(ALICE), pid, false,
		));

		// Setting false again should fail
		assert_noop!(
			AdsEntity::set_placement_active(RuntimeOrigin::signed(ALICE), pid, false),
			Error::<Test>::PlacementStatusUnchanged
		);
	});
}

// ============================================================================
// Regression: M1-R2 — distribute errors on unregistered placement
// ============================================================================

#[test]
fn m1r2_distribute_fails_unregistered_placement() {
	new_test_ext().execute_with(|| {
		let fake_pid = [0xFFu8; 32];
		// distribute for an unregistered placement should error, not silently use entity_id=0
		let result = <AdsEntity as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&fake_pid, 10_000, &99,
		);
		assert_noop!(result, Error::<Test>::PlacementNotRegistered);
	});
}

#[test]
fn m1r2_distribute_works_registered_placement() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let pid = entity_placement_id(1);

		let result = <AdsEntity as pallet_ads_primitives::RevenueDistributor<u64, u128>>::distribute(
			&pid, 10_000, &99,
		);
		assert_eq!(result.unwrap().placement_share, 8_000); // default 80%
	});
}

// ============================================================================
// Regression: L1-R2 — set_impression_cap state-change detection
// ============================================================================

#[test]
fn l1r2_set_impression_cap_rejects_unchanged() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let pid = entity_placement_id(1);

		// Default cap = 1000; setting same value should fail
		assert_noop!(
			AdsEntity::set_impression_cap(RuntimeOrigin::signed(ALICE), pid, 1000),
			Error::<Test>::ImpressionCapUnchanged
		);

		// Change to 5000 → succeeds
		assert_ok!(AdsEntity::set_impression_cap(
			RuntimeOrigin::signed(ALICE), pid, 5000,
		));

		// Setting 5000 again → fails
		assert_noop!(
			AdsEntity::set_impression_cap(RuntimeOrigin::signed(ALICE), pid, 5000),
			Error::<Test>::ImpressionCapUnchanged
		);
	});
}

// ============================================================================
// Regression: L2-R2 — daily impression reset after 14400 blocks
// ============================================================================

#[test]
fn l2r2_daily_impression_reset_after_14400_blocks() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(
			RuntimeOrigin::signed(ALICE), 1,
		));
		let pid = entity_placement_id(1);

		// Fill to cap (1000)
		let result = <AdsEntity as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&ALICE, &pid, 1000, None,
		);
		assert_eq!(result.unwrap(), 1000);
		assert_eq!(pallet::DailyImpressions::<Test>::get(&pid), 1000);

		// Still at block 1 → cap reached
		let result = <AdsEntity as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&ALICE, &pid, 1, None,
		);
		assert_noop!(result, Error::<Test>::DailyImpressionCapReached);

		// Advance to block 14401 (>= 14400 blocks after block 1)
		System::set_block_number(14401);

		// Now daily counter should reset, new impressions accepted
		let result = <AdsEntity as pallet_ads_primitives::DeliveryVerifier<u64>>::verify_and_cap_audience(
			&ALICE, &pid, 500, None,
		);
		assert_eq!(result.unwrap(), 500);
		assert_eq!(pallet::DailyImpressions::<Test>::get(&pid), 500);

		// Total impressions accumulated across days
		assert_eq!(pallet::TotalImpressions::<Test>::get(&pid), 1500);
	});
}

// ============================================================================
// EntityLocked 回归测试
// ============================================================================

#[test]
fn entity_locked_rejects_register_entity_placement() {
	new_test_ext().execute_with(|| {
		set_entity_locked(1);
		assert_noop!(
			AdsEntity::register_entity_placement(RuntimeOrigin::signed(ALICE), 1),
			Error::<Test>::EntityLocked
		);
	});
}

#[test]
fn entity_locked_rejects_register_shop_placement() {
	new_test_ext().execute_with(|| {
		set_entity_locked(1);
		assert_noop!(
			AdsEntity::register_shop_placement(RuntimeOrigin::signed(ALICE), 1, 10),
			Error::<Test>::EntityLocked
		);
	});
}

#[test]
fn entity_locked_rejects_deregister_placement() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(RuntimeOrigin::signed(ALICE), 1));
		let pid = crate::entity_placement_id(1);
		set_entity_locked(1);
		assert_noop!(
			AdsEntity::deregister_placement(RuntimeOrigin::signed(ALICE), pid),
			Error::<Test>::EntityLocked
		);
	});
}

#[test]
fn entity_locked_rejects_set_placement_active() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(RuntimeOrigin::signed(ALICE), 1));
		let pid = crate::entity_placement_id(1);
		set_entity_locked(1);
		assert_noop!(
			AdsEntity::set_placement_active(RuntimeOrigin::signed(ALICE), pid, false),
			Error::<Test>::EntityLocked
		);
	});
}

#[test]
fn entity_locked_rejects_set_impression_cap() {
	new_test_ext().execute_with(|| {
		assert_ok!(AdsEntity::register_entity_placement(RuntimeOrigin::signed(ALICE), 1));
		let pid = crate::entity_placement_id(1);
		set_entity_locked(1);
		assert_noop!(
			AdsEntity::set_impression_cap(RuntimeOrigin::signed(ALICE), pid, 500),
			Error::<Test>::EntityLocked
		);
	});
}

#[test]
fn entity_locked_rejects_set_entity_ad_share() {
	new_test_ext().execute_with(|| {
		set_entity_locked(1);
		assert_noop!(
			AdsEntity::set_entity_ad_share(RuntimeOrigin::signed(ALICE), 1, 5000),
			Error::<Test>::EntityLocked
		);
	});
}
