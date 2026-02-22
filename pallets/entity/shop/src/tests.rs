//! Unit tests for pallet-entity-shop

use crate::{mock::*, Error, ShopOperatingStatus, ShopType, MemberMode};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_entity_common::ShopProvider;

fn bounded_name(s: &[u8]) -> BoundedVec<u8, MaxShopNameLength> {
    BoundedVec::try_from(s.to_vec()).unwrap()
}

// ============================================================================
// create_shop tests
// ============================================================================

#[test]
fn create_shop_works() {
    new_test_ext().execute_with(|| {
        let entity_id = 1;
        let owner = 1;
        let name = bounded_name(b"Test Shop");
        let shop_type = ShopType::OnlineStore;
        let member_mode = MemberMode::Inherit;

        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(owner),
            entity_id,
            name.clone(),
            shop_type,
            member_mode,
            1000, // initial_fund
        ));

        // Check shop exists
        assert!(Shop::shops(1).is_some());
        let shop = Shop::shops(1).unwrap();
        assert_eq!(shop.entity_id, entity_id);
        assert_eq!(shop.shop_type, shop_type);
        assert_eq!(shop.member_mode, member_mode);
        assert_eq!(shop.status, ShopOperatingStatus::Active);

        // Check ShopEntity index
        assert_eq!(Shop::shop_entity(1), Some(entity_id));

        // Check NextShopId incremented
        assert_eq!(Shop::next_shop_id(), 2);
    });
}

#[test]
fn create_shop_fails_entity_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Shop::create_shop(
                RuntimeOrigin::signed(1),
                999, // non-existent entity
                bounded_name(b"Test Shop"),
                ShopType::OnlineStore,
                MemberMode::Inherit,
                1000,
            ),
            Error::<Test>::EntityNotFound
        );
    });
}

#[test]
fn create_shop_fails_not_entity_owner() {
    new_test_ext().execute_with(|| {
        // Account 2 tries to create shop for entity 1 (owned by account 1)
        assert_noop!(
            Shop::create_shop(
                RuntimeOrigin::signed(2),
                1,
                bounded_name(b"Test Shop"),
                ShopType::OnlineStore,
                MemberMode::Inherit,
                1000,
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn create_shop_fails_entity_not_active() {
    new_test_ext().execute_with(|| {
        // Entity 3 exists but is not active
        assert_noop!(
            Shop::create_shop(
                RuntimeOrigin::signed(3),
                3,
                bounded_name(b"Test Shop"),
                ShopType::OnlineStore,
                MemberMode::Inherit,
                1000,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

// ============================================================================
// update_shop tests
// ============================================================================

#[test]
fn update_shop_works() {
    new_test_ext().execute_with(|| {
        // Create shop first
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1),
            1,
            bounded_name(b"Test Shop"),
            ShopType::OnlineStore,
            MemberMode::Inherit,
            1000,
        ));

        // Update shop
        let new_name = bounded_name(b"Updated Shop");
        assert_ok!(Shop::update_shop(
            RuntimeOrigin::signed(1),
            1,
            Some(new_name.clone()),
            None,
            None,
        ));

        let shop = Shop::shops(1).unwrap();
        assert_eq!(shop.name, new_name);
    });
}

#[test]
fn update_shop_fails_not_manager() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1),
            1,
            bounded_name(b"Test Shop"),
            ShopType::OnlineStore,
            MemberMode::Inherit,
            1000,
        ));

        // Account 2 tries to update shop
        assert_noop!(
            Shop::update_shop(
                RuntimeOrigin::signed(2),
                1,
                Some(bounded_name(b"Hacked Shop")),
                None,
                None,
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

// ============================================================================
// add_manager / remove_manager tests
// ============================================================================

#[test]
fn add_manager_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1),
            1,
            bounded_name(b"Test Shop"),
            ShopType::OnlineStore,
            MemberMode::Inherit,
            1000,
        ));

        // Add manager
        assert_ok!(Shop::add_manager(RuntimeOrigin::signed(1), 1, 2));

        let shop = Shop::shops(1).unwrap();
        assert!(shop.managers.contains(&2));
    });
}

#[test]
fn add_manager_fails_already_manager() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1),
            1,
            bounded_name(b"Test Shop"),
            ShopType::OnlineStore,
            MemberMode::Inherit,
            1000,
        ));

        assert_ok!(Shop::add_manager(RuntimeOrigin::signed(1), 1, 2));

        // Try to add again
        assert_noop!(
            Shop::add_manager(RuntimeOrigin::signed(1), 1, 2),
            Error::<Test>::ManagerAlreadyExists
        );
    });
}

#[test]
fn remove_manager_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1),
            1,
            bounded_name(b"Test Shop"),
            ShopType::OnlineStore,
            MemberMode::Inherit,
            1000,
        ));

        assert_ok!(Shop::add_manager(RuntimeOrigin::signed(1), 1, 2));
        assert_ok!(Shop::remove_manager(RuntimeOrigin::signed(1), 1, 2));

        let shop = Shop::shops(1).unwrap();
        assert!(!shop.managers.contains(&2));
    });
}

// ============================================================================
// pause_shop / resume_shop tests
// ============================================================================

#[test]
fn pause_and_resume_shop_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1),
            1,
            bounded_name(b"Test Shop"),
            ShopType::OnlineStore,
            MemberMode::Inherit,
            1000,
        ));

        // Pause
        assert_ok!(Shop::pause_shop(RuntimeOrigin::signed(1), 1));
        let shop = Shop::shops(1).unwrap();
        assert_eq!(shop.status, ShopOperatingStatus::Paused);

        // Resume
        assert_ok!(Shop::resume_shop(RuntimeOrigin::signed(1), 1));
        let shop = Shop::shops(1).unwrap();
        assert_eq!(shop.status, ShopOperatingStatus::Active);
    });
}

#[test]
fn pause_shop_fails_already_paused() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1),
            1,
            bounded_name(b"Test Shop"),
            ShopType::OnlineStore,
            MemberMode::Inherit,
            1000,
        ));

        assert_ok!(Shop::pause_shop(RuntimeOrigin::signed(1), 1));

        assert_noop!(
            Shop::pause_shop(RuntimeOrigin::signed(1), 1),
            Error::<Test>::ShopAlreadyPaused
        );
    });
}

// ============================================================================
// close_shop tests
// ============================================================================

#[test]
fn close_shop_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1),
            1,
            bounded_name(b"Test Shop"),
            ShopType::OnlineStore,
            MemberMode::Inherit,
            1000,
        ));

        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        let shop = Shop::shops(1).unwrap();
        assert_eq!(shop.status, ShopOperatingStatus::Closed);
    });
}

// ============================================================================
// Primary Shop tests
// ============================================================================

#[test]
fn create_primary_shop_works() {
    new_test_ext().execute_with(|| {
        let shop_id = <Shop as ShopProvider<u64>>::create_primary_shop(
            1,
            b"Primary Shop".to_vec(),
            ShopType::OnlineStore,
            MemberMode::Inherit,
        ).unwrap();

        assert!(Shop::shops(shop_id).is_some());
        let shop = Shop::shops(shop_id).unwrap();
        assert!(shop.is_primary);
        assert_eq!(shop.entity_id, 1);
        assert_eq!(shop.status, ShopOperatingStatus::Active);

        // is_primary_shop trait method
        assert!(<Shop as ShopProvider<u64>>::is_primary_shop(shop_id));
    });
}

#[test]
fn cannot_close_primary_shop() {
    new_test_ext().execute_with(|| {
        let shop_id = <Shop as ShopProvider<u64>>::create_primary_shop(
            1,
            b"Primary Shop".to_vec(),
            ShopType::OnlineStore,
            MemberMode::Inherit,
        ).unwrap();

        assert_noop!(
            Shop::close_shop(RuntimeOrigin::signed(1), shop_id),
            Error::<Test>::CannotClosePrimaryShop
        );
    });
}

#[test]
fn normal_shop_is_not_primary() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1),
            1,
            bounded_name(b"Normal Shop"),
            ShopType::OnlineStore,
            MemberMode::Inherit,
            1000,
        ));

        let shop = Shop::shops(1).unwrap();
        assert!(!shop.is_primary);
        assert!(!<Shop as ShopProvider<u64>>::is_primary_shop(1));
    });
}

// ============================================================================
// H2: fund_operating fails on closed shop
// ============================================================================

#[test]
fn fund_operating_fails_on_closed_shop() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1),
            1,
            bounded_name(b"Test Shop"),
            ShopType::OnlineStore,
            MemberMode::Inherit,
            1000,
        ));

        // Close the shop
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        // H2: Funding a closed shop should fail
        assert_noop!(
            Shop::fund_operating(RuntimeOrigin::signed(1), 1, 500),
            Error::<Test>::ShopAlreadyClosed
        );
    });
}

// ============================================================================
// ShopProvider trait tests
// ============================================================================

#[test]
fn shop_provider_trait_works() {
    new_test_ext().execute_with(|| {
        // Before creating shop
        assert!(!<Shop as ShopProvider<u64>>::shop_exists(1));

        // Create shop
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1),
            1,
            bounded_name(b"Test Shop"),
            ShopType::OnlineStore,
            MemberMode::Inherit,
            1000,
        ));

        // Test trait methods
        assert!(<Shop as ShopProvider<u64>>::shop_exists(1));
        assert!(<Shop as ShopProvider<u64>>::is_shop_active(1));
        assert_eq!(<Shop as ShopProvider<u64>>::shop_entity_id(1), Some(1));

        // Pause shop
        assert_ok!(Shop::pause_shop(RuntimeOrigin::signed(1), 1));
        assert!(!<Shop as ShopProvider<u64>>::is_shop_active(1));
    });
}

// ============================================================================
// Multiple shops per entity tests
// ============================================================================

#[test]
fn multiple_shops_per_entity_works() {
    new_test_ext().execute_with(|| {
        // Create first shop
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1),
            1,
            bounded_name(b"Shop 1"),
            ShopType::OnlineStore,
            MemberMode::Inherit,
            1000,
        ));

        // Create second shop for same entity
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1),
            1,
            bounded_name(b"Shop 2"),
            ShopType::PhysicalStore,
            MemberMode::Independent,
            1000,
        ));

        // Both shops should exist
        assert!(Shop::shops(1).is_some());
        assert!(Shop::shops(2).is_some());

        // Both should belong to entity 1
        assert_eq!(Shop::shop_entity(1), Some(1));
        assert_eq!(Shop::shop_entity(2), Some(1));

        // Check different shop types
        assert_eq!(Shop::shops(1).unwrap().shop_type, ShopType::OnlineStore);
        assert_eq!(Shop::shops(2).unwrap().shop_type, ShopType::PhysicalStore);
    });
}
