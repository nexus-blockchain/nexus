//! Unit tests for pallet-entity-shop

use crate::{mock::*, Error, ShopOperatingStatus, ShopType};
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
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(owner),
            entity_id,
            name.clone(),
            shop_type,
            1000, // initial_fund
        ));

        // Check shop exists
        assert!(Shop::shops(1).is_some());
        let shop = Shop::shops(1).unwrap();
        assert_eq!(shop.entity_id, entity_id);
        assert_eq!(shop.shop_type, shop_type);
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
// withdraw_operating_fund tests
// ============================================================================

#[test]
fn withdraw_operating_fund_works() {
    new_test_ext().execute_with(|| {
        // Create shop with 1000 initial fund
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        let shop_account = Shop::shop_account_id(1);
        let owner_before = Balances::free_balance(1);
        let shop_before = Balances::free_balance(shop_account);

        // Withdraw 500 (leaves 500 which is > MinOperatingBalance=100)
        assert_ok!(Shop::withdraw_operating_fund(RuntimeOrigin::signed(1), 1, 500));

        assert_eq!(Balances::free_balance(shop_account), shop_before - 500);
        assert_eq!(Balances::free_balance(1), owner_before + 500);
    });
}

#[test]
fn withdraw_operating_fund_fails_not_owner() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // Account 2 is not entity owner
        assert_noop!(
            Shop::withdraw_operating_fund(RuntimeOrigin::signed(2), 1, 100),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn withdraw_operating_fund_fails_below_minimum() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // Try withdraw 950, leaving 50 < MinOperatingBalance(100)
        assert_noop!(
            Shop::withdraw_operating_fund(RuntimeOrigin::signed(1), 1, 950),
            Error::<Test>::WithdrawBelowMinimum
        );
    });
}

#[test]
fn withdraw_operating_fund_closed_shop_no_minimum() {
    new_test_ext().execute_with(|| {
        // Create non-primary shop (shop_id=1)
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Shop 2"), ShopType::OnlineStore, 1000,
        ));

        // Close shop first
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        // close_shop auto-refunds, so shop_account should be 0 now
        let shop_account = Shop::shop_account_id(1);
        assert_eq!(Balances::free_balance(shop_account), 0);
    });
}

#[test]
fn withdraw_operating_fund_fails_zero_amount() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_noop!(
            Shop::withdraw_operating_fund(RuntimeOrigin::signed(1), 1, 0),
            Error::<Test>::ZeroWithdrawAmount
        );
    });
}

#[test]
fn withdraw_operating_fund_fails_insufficient() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // Try withdraw more than available
        assert_noop!(
            Shop::withdraw_operating_fund(RuntimeOrigin::signed(1), 1, 5000),
            Error::<Test>::InsufficientOperatingFund
        );
    });
}

// ============================================================================
// close_shop auto-refund tests
// ============================================================================

#[test]
fn close_shop_refunds_operating_fund() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Shop 2"), ShopType::OnlineStore, 1000,
        ));

        let owner_before = Balances::free_balance(1);
        let shop_account = Shop::shop_account_id(1);
        let shop_balance = Balances::free_balance(shop_account);
        assert_eq!(shop_balance, 1000);

        // Close shop — should auto-refund
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        // shop_account should be empty
        assert_eq!(Balances::free_balance(shop_account), 0);
        // owner should have received the refund
        assert_eq!(Balances::free_balance(1), owner_before + 1000);
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
// H2: disable_points clears balances and total supply
// ============================================================================

#[test]
fn h2_disable_points_clears_balances() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // Enable points
        let name: BoundedVec<u8, MaxPointsNameLength> = BoundedVec::try_from(b"Points".to_vec()).unwrap();
        let symbol: BoundedVec<u8, MaxPointsSymbolLength> = BoundedVec::try_from(b"PTS".to_vec()).unwrap();
        assert_ok!(Shop::enable_points(
            RuntimeOrigin::signed(1), 1, name, symbol, 500, 1000, true,
        ));

        // Issue some points
        assert_ok!(Shop::issue_points(1, &2, 500));
        assert_eq!(Shop::shop_points_balance(1, 2), 500);
        assert_eq!(Shop::shop_points_total_supply(1), 500);

        // Disable points
        assert_ok!(Shop::disable_points(RuntimeOrigin::signed(1), 1));

        // H2: Balances and total supply should be cleaned
        assert_eq!(Shop::shop_points_balance(1, 2), 0);
        assert_eq!(Shop::shop_points_total_supply(1), 0);
        assert!(Shop::shop_points_config(1).is_none());
    });
}

// ============================================================================
// H3: add_manager / remove_manager reject closed shop
// ============================================================================

#[test]
fn h3_add_manager_fails_closed_shop() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        assert_noop!(
            Shop::add_manager(RuntimeOrigin::signed(1), 1, 2),
            Error::<Test>::ShopAlreadyClosed
        );
    });
}

#[test]
fn h3_remove_manager_fails_closed_shop() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::add_manager(RuntimeOrigin::signed(1), 1, 2));
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        assert_noop!(
            Shop::remove_manager(RuntimeOrigin::signed(1), 1, 2),
            Error::<Test>::ShopAlreadyClosed
        );
    });
}

// ============================================================================
// M1: enable_points rejects empty name/symbol
// ============================================================================

#[test]
fn m1_enable_points_rejects_empty_name() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        let empty_name: BoundedVec<u8, MaxPointsNameLength> = BoundedVec::default();
        let symbol: BoundedVec<u8, MaxPointsSymbolLength> = BoundedVec::try_from(b"PTS".to_vec()).unwrap();
        assert_noop!(
            Shop::enable_points(RuntimeOrigin::signed(1), 1, empty_name, symbol, 500, 1000, true),
            Error::<Test>::PointsNameEmpty
        );
    });
}

#[test]
fn m1_enable_points_rejects_empty_symbol() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        let name: BoundedVec<u8, MaxPointsNameLength> = BoundedVec::try_from(b"Points".to_vec()).unwrap();
        let empty_symbol: BoundedVec<u8, MaxPointsSymbolLength> = BoundedVec::default();
        assert_noop!(
            Shop::enable_points(RuntimeOrigin::signed(1), 1, name, empty_symbol, 500, 1000, true),
            Error::<Test>::InvalidConfig
        );
    });
}

// ============================================================================
// M3: close_shop cleans points data
// ============================================================================

#[test]
fn m3_close_shop_cleans_points_data() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // Enable and issue points
        let name: BoundedVec<u8, MaxPointsNameLength> = BoundedVec::try_from(b"Points".to_vec()).unwrap();
        let symbol: BoundedVec<u8, MaxPointsSymbolLength> = BoundedVec::try_from(b"PTS".to_vec()).unwrap();
        assert_ok!(Shop::enable_points(
            RuntimeOrigin::signed(1), 1, name, symbol, 500, 1000, true,
        ));
        assert_ok!(Shop::issue_points(1, &2, 500));

        // Close shop
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        // M3: Points data should be cleaned
        assert!(Shop::shop_points_config(1).is_none());
        assert_eq!(Shop::shop_points_balance(1, 2), 0);
        assert_eq!(Shop::shop_points_total_supply(1), 0);
    });
}

// ============================================================================
// H4: deduct_operating_fund rejects closed shop
// ============================================================================

#[test]
fn h4_deduct_operating_fund_fails_closed_shop() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        // H4: deduct_operating_fund should fail on closed shop
        assert_noop!(
            <Shop as ShopProvider<u64>>::deduct_operating_fund(1, 100),
            Error::<Test>::ShopAlreadyClosed
        );
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
            1000,
        ));

        // Create second shop for same entity
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1),
            1,
            bounded_name(b"Shop 2"),
            ShopType::PhysicalStore,
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

// ============================================================================
// H2: update_shop / set_location reject empty CID
// ============================================================================

#[test]
fn h2_update_shop_rejects_empty_logo_cid() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        let empty_cid: BoundedVec<u8, MaxCidLength> = BoundedVec::default();
        assert_noop!(
            Shop::update_shop(RuntimeOrigin::signed(1), 1, None, Some(empty_cid), None),
            Error::<Test>::EmptyCid
        );
    });
}

#[test]
fn h2_update_shop_rejects_empty_description_cid() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        let empty_cid: BoundedVec<u8, MaxCidLength> = BoundedVec::default();
        assert_noop!(
            Shop::update_shop(RuntimeOrigin::signed(1), 1, None, None, Some(empty_cid)),
            Error::<Test>::EmptyCid
        );
    });
}

#[test]
fn h2_set_location_rejects_empty_address_cid() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        let empty_cid: BoundedVec<u8, MaxCidLength> = BoundedVec::default();
        assert_noop!(
            Shop::set_location(RuntimeOrigin::signed(1), 1, None, Some(empty_cid), None),
            Error::<Test>::EmptyCid
        );
    });
}

// ============================================================================
// H3: trait pause_shop / resume_shop state validation
// ============================================================================

#[test]
fn h3_trait_pause_rejects_closed_shop() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        // trait pause_shop should reject closed shop
        assert_noop!(
            <Shop as ShopProvider<u64>>::pause_shop(1),
            Error::<Test>::ShopAlreadyPaused
        );
    });
}

#[test]
fn h3_trait_resume_rejects_closed_shop() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        // trait resume_shop should reject closed shop
        assert_noop!(
            <Shop as ShopProvider<u64>>::resume_shop(1),
            Error::<Test>::ShopNotPaused
        );
    });
}

// ============================================================================
// M1: update_points_config rejects closed shop
// ============================================================================

#[test]
fn m1_update_points_config_rejects_closed_shop() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // Enable points first
        let name: BoundedVec<u8, MaxPointsNameLength> = BoundedVec::try_from(b"Points".to_vec()).unwrap();
        let symbol: BoundedVec<u8, MaxPointsSymbolLength> = BoundedVec::try_from(b"PTS".to_vec()).unwrap();
        assert_ok!(Shop::enable_points(
            RuntimeOrigin::signed(1), 1, name, symbol, 500, 1000, true,
        ));

        // Close shop
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        // M1: update_points_config should fail
        assert_noop!(
            Shop::update_points_config(RuntimeOrigin::signed(1), 1, Some(200), None, None),
            Error::<Test>::ShopAlreadyClosed
        );
    });
}

// ============================================================================
// M2: transfer_points rejects closed shop
// ============================================================================

#[test]
fn m2_transfer_points_rejects_closed_shop() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        let name: BoundedVec<u8, MaxPointsNameLength> = BoundedVec::try_from(b"Points".to_vec()).unwrap();
        let symbol: BoundedVec<u8, MaxPointsSymbolLength> = BoundedVec::try_from(b"PTS".to_vec()).unwrap();
        assert_ok!(Shop::enable_points(
            RuntimeOrigin::signed(1), 1, name, symbol, 500, 1000, true,
        ));

        // Issue points to account 2
        assert_ok!(Shop::issue_points(1, &2, 500));

        // Close shop
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        // M2: transfer_points should fail on closed shop
        assert_noop!(
            Shop::transfer_points(RuntimeOrigin::signed(2), 1, 3, 100),
            Error::<Test>::ShopAlreadyClosed
        );
    });
}

// ============================================================================
// M4: fund_operating rejects zero amount
// ============================================================================

#[test]
fn m4_fund_operating_rejects_zero_amount() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        assert_noop!(
            Shop::fund_operating(RuntimeOrigin::signed(1), 1, 0),
            Error::<Test>::ZeroFundAmount
        );
    });
}

// ============================================================================
// Audit Round 2: Regression Tests
// ============================================================================

#[test]
fn m1_close_shop_clears_shop_entity_index() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // ShopEntity should exist before close
        assert_eq!(Shop::shop_entity(1), Some(1));

        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        // M1: ShopEntity reverse index should be cleaned up after close
        assert_eq!(Shop::shop_entity(1), None);
    });
}

#[test]
fn m1_force_close_shop_clears_shop_entity_index() {
    new_test_ext().execute_with(|| {
        let shop_id = <Shop as ShopProvider<u64>>::create_primary_shop(
            1, b"Primary Shop".to_vec(), ShopType::OnlineStore,
        ).unwrap();

        assert_eq!(Shop::shop_entity(shop_id), Some(1));

        // force_close bypasses is_primary check
        assert_ok!(<Shop as ShopProvider<u64>>::force_close_shop(shop_id));

        // M1: ShopEntity reverse index should be cleaned up
        assert_eq!(Shop::shop_entity(shop_id), None);
    });
}

#[test]
fn m2_trait_resume_shop_requires_operating_fund() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // Pause shop
        assert_ok!(<Shop as ShopProvider<u64>>::pause_shop(1));

        // Drain the shop account balance to below MinOperatingBalance
        let shop_account = Shop::shop_account_id(1);
        let balance = Balances::free_balance(shop_account);
        // Transfer most funds away so balance < MinOperatingBalance (100)
        let _ = <Balances as frame_support::traits::Currency<u64>>::transfer(
            &shop_account, &5, balance - 50,
            frame_support::traits::ExistenceRequirement::AllowDeath,
        );
        assert!(Balances::free_balance(shop_account) < 100);

        // M2: trait resume_shop should reject when fund < MinOperatingBalance
        assert_noop!(
            <Shop as ShopProvider<u64>>::resume_shop(1),
            Error::<Test>::InsufficientOperatingFund
        );
    });
}

#[test]
fn m3_update_points_config_emits_event() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        let name: BoundedVec<u8, MaxPointsNameLength> = BoundedVec::try_from(b"Points".to_vec()).unwrap();
        let symbol: BoundedVec<u8, MaxPointsSymbolLength> = BoundedVec::try_from(b"PTS".to_vec()).unwrap();
        assert_ok!(Shop::enable_points(
            RuntimeOrigin::signed(1), 1, name, symbol, 500, 1000, true,
        ));

        // Clear events
        System::reset_events();

        // Update points config
        assert_ok!(Shop::update_points_config(
            RuntimeOrigin::signed(1), 1, Some(200), Some(300), None,
        ));

        // M3: Should have emitted ShopUpdated event
        let events = System::events();
        assert!(events.iter().any(|e| {
            matches!(
                e.event,
                RuntimeEvent::Shop(crate::Event::ShopUpdated { shop_id: 1 })
            )
        }), "update_points_config should emit ShopUpdated event");

        // Verify config was actually updated
        let config = Shop::shop_points_config(1).unwrap();
        assert_eq!(config.reward_rate, 200);
        assert_eq!(config.exchange_rate, 300);
    });
}

#[test]
fn l2_enable_points_empty_name_returns_points_name_empty() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        let empty_name: BoundedVec<u8, MaxPointsNameLength> = BoundedVec::default();
        let symbol: BoundedVec<u8, MaxPointsSymbolLength> = BoundedVec::try_from(b"PTS".to_vec()).unwrap();

        // L2: Should return PointsNameEmpty, not ShopNameEmpty
        assert_noop!(
            Shop::enable_points(RuntimeOrigin::signed(1), 1, empty_name, symbol, 500, 1000, true),
            Error::<Test>::PointsNameEmpty
        );
    });
}

// ============================================================================
// Audit Round 3: Regression Tests
// ============================================================================

#[test]
fn m1r3_force_close_shop_rejects_double_close() {
    new_test_ext().execute_with(|| {
        let shop_id = <Shop as ShopProvider<u64>>::create_primary_shop(
            1, b"Primary Shop".to_vec(), ShopType::OnlineStore,
        ).unwrap();

        // First force_close should succeed
        assert_ok!(<Shop as ShopProvider<u64>>::force_close_shop(shop_id));
        assert_eq!(Shop::shops(shop_id).unwrap().status, ShopOperatingStatus::Closed);

        // M1-R3: Second force_close should fail (no duplicate ShopClosed event)
        assert_noop!(
            <Shop as ShopProvider<u64>>::force_close_shop(shop_id),
            Error::<Test>::ShopAlreadyClosed
        );
    });
}

#[test]
fn m2r3_update_shop_stats_works_after_shop_entity_cleared() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // M2-R3: update_shop_stats reads entity_id from shop struct, not ShopEntity
        assert_ok!(<Shop as ShopProvider<u64>>::update_shop_stats(1, 5000, 3));

        let shop = Shop::shops(1).unwrap();
        assert_eq!(shop.total_sales, 5000);
        assert_eq!(shop.total_orders, 3);
    });
}

#[test]
fn l1r3_update_shop_rejects_all_none() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // L1-R3: All-None update should be rejected
        assert_noop!(
            Shop::update_shop(RuntimeOrigin::signed(1), 1, None, None, None),
            Error::<Test>::InvalidConfig
        );
    });
}

#[test]
fn l2r3_update_points_config_rejects_all_none() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        let name: BoundedVec<u8, MaxPointsNameLength> = BoundedVec::try_from(b"Points".to_vec()).unwrap();
        let symbol: BoundedVec<u8, MaxPointsSymbolLength> = BoundedVec::try_from(b"PTS".to_vec()).unwrap();
        assert_ok!(Shop::enable_points(
            RuntimeOrigin::signed(1), 1, name, symbol, 500, 1000, true,
        ));

        // L2-R3: All-None update should be rejected
        assert_noop!(
            Shop::update_points_config(RuntimeOrigin::signed(1), 1, None, None, None),
            Error::<Test>::InvalidConfig
        );
    });
}

#[test]
fn h3_deduct_operating_fund_triggers_fund_depleted() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        let shop = Shop::shops(1).unwrap();
        assert_eq!(shop.status, ShopOperatingStatus::Active);

        // Deduct most of the fund to trigger FundDepleted
        // MinOperatingBalance = 100, so deducting 950 leaves 50 < 100
        assert_ok!(<Shop as ShopProvider<u64>>::deduct_operating_fund(1, 950));

        // H3: Shop status should be FundDepleted
        let shop = Shop::shops(1).unwrap();
        assert_eq!(shop.status, ShopOperatingStatus::FundDepleted);
    });
}
