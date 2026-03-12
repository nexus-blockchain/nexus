//! Unit tests for pallet-entity-shop

use crate::{mock::*, Error, ShopOperatingStatus, ShopType};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_entity_common::ShopProvider;

fn bounded_name(s: &[u8]) -> BoundedVec<u8, MaxShopNameLength> {
    BoundedVec::try_from(s.to_vec()).unwrap()
}

fn bounded_cid(s: &[u8]) -> BoundedVec<u8, MaxCidLength> {
    BoundedVec::try_from(s.to_vec()).unwrap()
}

/// 完整关闭流程：close_shop → 推进区块过宽限期 → finalize_close_shop
fn close_and_finalize(who: u64, shop_id: u64) {
    assert_ok!(Shop::close_shop(RuntimeOrigin::signed(who), shop_id));
    let now = System::block_number();
    System::set_block_number(now + 11); // grace = 10
    assert_ok!(Shop::finalize_close_shop(RuntimeOrigin::signed(who), shop_id));
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
            None, None, None, None,
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
                None, None, None, None,
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
            Error::<Test>::ShopNotActive
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

        // Step 1: close_shop sets Closing
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));
        let shop = Shop::shops(1).unwrap();
        assert_eq!(shop.status, ShopOperatingStatus::Closing);

        // Step 2: finalize after grace period
        let now = System::block_number();
        System::set_block_number(now + 11);
        assert_ok!(Shop::finalize_close_shop(RuntimeOrigin::signed(1), 1));
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
        assert_eq!(shop.entity_id, 1);
        assert_eq!(shop.status, ShopOperatingStatus::Active);

        assert!(<Shop as ShopProvider<u64>>::is_primary_shop(shop_id));
        assert_eq!(Shop::entity_primary_shop(1), Some(shop_id));
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

        assert!(!<Shop as ShopProvider<u64>>::is_primary_shop(1));
        assert!(Shop::entity_primary_shop(1).is_none() || Shop::entity_primary_shop(1) != Some(1));
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

        // Full close flow
        close_and_finalize(1, 1);

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

        // Full close flow (finalize refunds funds)
        close_and_finalize(1, 1);

        // finalize auto-refunds, so shop_account should be 0 now
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

        // Full close flow — finalize auto-refunds
        close_and_finalize(1, 1);

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
// H4: deduct_operating_fund rejects closed shop
// ============================================================================

#[test]
fn h4_deduct_operating_fund_fails_closed_shop() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        close_and_finalize(1, 1);

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
            Shop::update_shop(RuntimeOrigin::signed(1), 1, None, Some(Some(empty_cid)), None, None, None),
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
            Shop::update_shop(RuntimeOrigin::signed(1), 1, None, None, Some(Some(empty_cid)), None, None),
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
            Shop::set_location(RuntimeOrigin::signed(1), 1, None, Some(Some(empty_cid))),
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

        // trait pause_shop should reject closed shop (Closing != Active)
        assert_noop!(
            <Shop as ShopProvider<u64>>::pause_shop(1),
            Error::<Test>::ShopNotActive
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

        close_and_finalize(1, 1);

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
            Shop::update_shop(RuntimeOrigin::signed(1), 1, None, None, None, None, None),
            Error::<Test>::InvalidConfig
        );
    });
}

// ============================================================================
// P0: product_count increment/decrement tests
// ============================================================================

#[test]
fn p0_increment_product_count_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        assert_eq!(Shop::shops(1).unwrap().product_count, 0);

        assert_ok!(<Shop as ShopProvider<u64>>::increment_product_count(1));
        assert_eq!(Shop::shops(1).unwrap().product_count, 1);

        assert_ok!(<Shop as ShopProvider<u64>>::increment_product_count(1));
        assert_eq!(Shop::shops(1).unwrap().product_count, 2);
    });
}

#[test]
fn p0_decrement_product_count_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // Increment then decrement
        assert_ok!(<Shop as ShopProvider<u64>>::increment_product_count(1));
        assert_ok!(<Shop as ShopProvider<u64>>::increment_product_count(1));
        assert_eq!(Shop::shops(1).unwrap().product_count, 2);

        assert_ok!(<Shop as ShopProvider<u64>>::decrement_product_count(1));
        assert_eq!(Shop::shops(1).unwrap().product_count, 1);
    });
}

#[test]
fn p0_decrement_product_count_saturates_at_zero() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // Decrement from 0 should saturate
        assert_ok!(<Shop as ShopProvider<u64>>::decrement_product_count(1));
        assert_eq!(Shop::shops(1).unwrap().product_count, 0);
    });
}

#[test]
fn p0_product_count_fails_shop_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            <Shop as ShopProvider<u64>>::increment_product_count(999),
            Error::<Test>::ShopNotFound
        );
        assert_noop!(
            <Shop as ShopProvider<u64>>::decrement_product_count(999),
            Error::<Test>::ShopNotFound
        );
    });
}

// ============================================================================
// P0: Pending/Closing removed — default is Active
// ============================================================================

#[test]
fn p0_default_shop_status_is_active() {
    // After removing Pending, the default ShopOperatingStatus should be Active
    assert_eq!(ShopOperatingStatus::default(), ShopOperatingStatus::Active);
}

#[test]
fn p0_shop_created_with_active_status() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // Shop should be created directly in Active status (no Pending)
        let shop = Shop::shops(1).unwrap();
        assert_eq!(shop.status, ShopOperatingStatus::Active);
    });
}

// ============================================================================
// Closing 宽限期 regression tests
// ============================================================================

#[test]
fn closing_grace_period_rejects_early_finalize() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::Closing);
        assert!(Shop::shop_closing_at(1).is_some());

        // Finalize before grace period should fail
        assert_noop!(
            Shop::finalize_close_shop(RuntimeOrigin::signed(1), 1),
            Error::<Test>::ClosingGracePeriodNotElapsed
        );

        // Advance to one block before grace expiry — should still fail
        System::set_block_number(10); // closing_at=1, grace=10, need >= 1+10=11
        assert_noop!(
            Shop::finalize_close_shop(RuntimeOrigin::signed(1), 1),
            Error::<Test>::ClosingGracePeriodNotElapsed
        );

        // Advance to exactly grace boundary (1+10=11) — should succeed
        System::set_block_number(11);
        assert_ok!(Shop::finalize_close_shop(RuntimeOrigin::signed(1), 1));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::Closed);
        // ShopClosingAt should be cleaned
        assert!(Shop::shop_closing_at(1).is_none());
    });
}

#[test]
fn closing_rejects_double_close() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        // Second close_shop should fail
        assert_noop!(
            Shop::close_shop(RuntimeOrigin::signed(1), 1),
            Error::<Test>::ShopAlreadyClosing
        );
    });
}

#[test]
fn closing_allows_fund_deposit() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        // fund_operating should still work during Closing (cover obligations)
        assert_ok!(Shop::fund_operating(RuntimeOrigin::signed(1), 1, 500));
    });
}

#[test]
fn force_close_cleans_closing_timer() {
    new_test_ext().execute_with(|| {
        let shop_id = <Shop as ShopProvider<u64>>::create_primary_shop(
            1, b"Primary Shop".to_vec(), ShopType::OnlineStore,
        ).unwrap();

        // Manually put into Closing state via close_shop won't work (primary), so set it directly
        crate::Shops::<Test>::mutate(shop_id, |maybe| {
            if let Some(shop) = maybe.as_mut() {
                shop.status = ShopOperatingStatus::Closing;
            }
        });
        crate::ShopClosingAt::<Test>::insert(shop_id, System::block_number());

        // force_close should clean up ShopClosingAt
        assert_ok!(<Shop as ShopProvider<u64>>::force_close_shop(shop_id));
        assert!(Shop::shop_closing_at(shop_id).is_none());
        assert_eq!(Shop::shops(shop_id).unwrap().status, ShopOperatingStatus::Closed);
    });
}

#[test]
fn finalize_anyone_can_call() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));
        System::set_block_number(12);

        // Account 5 (not owner) can finalize
        assert_ok!(Shop::finalize_close_shop(RuntimeOrigin::signed(5), 1));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::Closed);
    });
}

// ============================================================================
// CID clear regression tests
// ============================================================================

#[test]
fn update_shop_clear_logo_cid() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // Set logo_cid
        let cid: BoundedVec<u8, MaxCidLength> = BoundedVec::try_from(b"Qm123".to_vec()).unwrap();
        assert_ok!(Shop::update_shop(RuntimeOrigin::signed(1), 1, None, Some(Some(cid)), None, None, None));
        assert!(Shop::shops(1).unwrap().logo_cid.is_some());

        // Clear logo_cid with Some(None)
        assert_ok!(Shop::update_shop(RuntimeOrigin::signed(1), 1, None, Some(None), None, None, None));
        assert!(Shop::shops(1).unwrap().logo_cid.is_none());
    });
}

#[test]
fn update_shop_clear_description_cid() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        let cid: BoundedVec<u8, MaxCidLength> = BoundedVec::try_from(b"Qm456".to_vec()).unwrap();
        assert_ok!(Shop::update_shop(RuntimeOrigin::signed(1), 1, None, None, Some(Some(cid)), None, None));
        assert!(Shop::shops(1).unwrap().description_cid.is_some());

        // Clear with Some(None)
        assert_ok!(Shop::update_shop(RuntimeOrigin::signed(1), 1, None, None, Some(None), None, None));
        assert!(Shop::shops(1).unwrap().description_cid.is_none());
    });
}

#[test]
fn set_location_clear_address_cid() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        let cid: BoundedVec<u8, MaxCidLength> = BoundedVec::try_from(b"QmAddr".to_vec()).unwrap();
        assert_ok!(Shop::set_location(RuntimeOrigin::signed(1), 1, None, Some(Some(cid))));
        assert!(Shop::shops(1).unwrap().address_cid.is_some());

        // Clear with Some(None)
        assert_ok!(Shop::set_location(RuntimeOrigin::signed(1), 1, None, Some(None)));
        assert!(Shop::shops(1).unwrap().address_cid.is_none());
    });
}

#[test]
fn update_shop_none_does_not_change_cid() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        let cid: BoundedVec<u8, MaxCidLength> = BoundedVec::try_from(b"Qm123".to_vec()).unwrap();
        assert_ok!(Shop::update_shop(RuntimeOrigin::signed(1), 1, None, Some(Some(cid.clone())), None, None, None));

        // None = no change, logo_cid should remain
        assert_ok!(Shop::update_shop(
            RuntimeOrigin::signed(1), 1,
            Some(bounded_name(b"New Name")), None, None, None, None,
        ));
        assert_eq!(Shop::shops(1).unwrap().logo_cid, Some(cid));
    });
}

// ============================================================================
// F3: transfer_shop
// ============================================================================

#[test]
fn f3_transfer_shop_works() {
    new_test_ext().execute_with(|| {
        // Entity 1 (owner=1) creates non-primary shop
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Shop B"), ShopType::OnlineStore, 500,
        ));

        assert_eq!(Shop::shops(1).unwrap().entity_id, 1);

        // Transfer to Entity 2
        assert_ok!(Shop::transfer_shop(RuntimeOrigin::signed(1), 1, 2));

        let shop = Shop::shops(1).unwrap();
        assert_eq!(shop.entity_id, 2);
        assert_eq!(Shop::shop_entity_id(1), Some(2));
    });
}

#[test]
fn f3_transfer_shop_rejects_primary() {
    new_test_ext().execute_with(|| {
        // Create a primary shop via trait
        assert_ok!(<Shop as ShopProvider<u64>>::create_primary_shop(1, b"Primary".to_vec(), ShopType::OnlineStore));

        assert_noop!(
            Shop::transfer_shop(RuntimeOrigin::signed(1), 1, 2),
            Error::<Test>::CannotTransferPrimaryShop
        );
    });
}

#[test]
fn f3_transfer_shop_rejects_same_entity() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Shop"), ShopType::OnlineStore, 500,
        ));

        assert_noop!(
            Shop::transfer_shop(RuntimeOrigin::signed(1), 1, 1),
            Error::<Test>::SameEntity
        );
    });
}

#[test]
fn f3_transfer_shop_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Shop"), ShopType::OnlineStore, 500,
        ));

        // Account 2 is not owner of Entity 1
        assert_noop!(
            Shop::transfer_shop(RuntimeOrigin::signed(2), 1, 2),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn f3_transfer_shop_rejects_inactive_target() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Shop"), ShopType::OnlineStore, 500,
        ));

        // Entity 3 exists but is Suspended (not active)
        assert_noop!(
            Shop::transfer_shop(RuntimeOrigin::signed(1), 1, 3),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f3_transfer_shop_rejects_closing() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Shop"), ShopType::OnlineStore, 500,
        ));
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        assert_noop!(
            Shop::transfer_shop(RuntimeOrigin::signed(1), 1, 2),
            Error::<Test>::ShopAlreadyClosed
        );
    });
}

// ============================================================================
// F4: set_primary_shop
// ============================================================================

#[test]
fn f4_set_primary_shop_works() {
    new_test_ext().execute_with(|| {
        // Create primary shop via trait
        assert_ok!(<Shop as ShopProvider<u64>>::create_primary_shop(1, b"Primary".to_vec(), ShopType::OnlineStore));
        // Create second shop
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Secondary"), ShopType::OnlineStore, 500,
        ));

        assert_eq!(Shop::entity_primary_shop(1), Some(1));

        assert_ok!(Shop::set_primary_shop(RuntimeOrigin::signed(1), 1, 2));

        assert_eq!(Shop::entity_primary_shop(1), Some(2));
    });
}

#[test]
fn f4_set_primary_shop_rejects_already_primary() {
    new_test_ext().execute_with(|| {
        assert_ok!(<Shop as ShopProvider<u64>>::create_primary_shop(1, b"Primary".to_vec(), ShopType::OnlineStore));

        assert_noop!(
            Shop::set_primary_shop(RuntimeOrigin::signed(1), 1, 1),
            Error::<Test>::InvalidConfig
        );
    });
}

#[test]
fn f4_set_primary_shop_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        assert_ok!(<Shop as ShopProvider<u64>>::create_primary_shop(1, b"Primary".to_vec(), ShopType::OnlineStore));
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Secondary"), ShopType::OnlineStore, 500,
        ));

        assert_noop!(
            Shop::set_primary_shop(RuntimeOrigin::signed(2), 1, 2),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn f4_set_primary_shop_rejects_wrong_entity() {
    new_test_ext().execute_with(|| {
        assert_ok!(<Shop as ShopProvider<u64>>::create_primary_shop(1, b"Primary".to_vec(), ShopType::OnlineStore));
        // Create shop for Entity 2
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(2), 2,
            bounded_name(b"Other Shop"), ShopType::OnlineStore, 500,
        ));

        // Try to set Entity 2's shop as primary for Entity 1
        assert_noop!(
            Shop::set_primary_shop(RuntimeOrigin::signed(1), 1, 2),
            Error::<Test>::NotAuthorized
        );
    });
}

// ============================================================================
// F5: force_pause_shop
// ============================================================================

#[test]
fn f5_force_pause_shop_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        assert_ok!(Shop::force_pause_shop(RuntimeOrigin::root(), 1));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::Paused);

        // Can be resumed by owner
        assert_ok!(Shop::resume_shop(RuntimeOrigin::signed(1), 1));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::Active);
    });
}

#[test]
fn f5_force_pause_shop_rejects_non_root() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        assert_noop!(
            Shop::force_pause_shop(RuntimeOrigin::signed(1), 1),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn f5_force_pause_shop_rejects_already_paused() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::pause_shop(RuntimeOrigin::signed(1), 1));

        assert_noop!(
            Shop::force_pause_shop(RuntimeOrigin::root(), 1),
            Error::<Test>::ShopAlreadyPaused
        );
    });
}

#[test]
fn f5_force_pause_shop_rejects_closed() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Shop A"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Shop B"), ShopType::OnlineStore, 1000,
        ));
        close_and_finalize(1, 2);

        assert_noop!(
            Shop::force_pause_shop(RuntimeOrigin::root(), 2),
            Error::<Test>::ShopAlreadyClosed
        );
    });
}

#[test]
fn f5_force_pause_via_trait_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        assert_ok!(<Shop as ShopProvider<u64>>::force_pause_shop(1));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::Paused);
    });
}

// ============================================================================

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

// ============================================================================
// Audit Round 4: Regression Tests
// ============================================================================

#[test]
fn m2_finalize_close_shop_rejects_non_closing_with_correct_error() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // Shop is Active — finalize should return ShopNotClosing (not ShopNotPaused)
        assert_noop!(
            Shop::finalize_close_shop(RuntimeOrigin::signed(1), 1),
            Error::<Test>::ShopNotClosing
        );

        // Pause shop — finalize should still return ShopNotClosing
        assert_ok!(Shop::pause_shop(RuntimeOrigin::signed(1), 1));
        assert_noop!(
            Shop::finalize_close_shop(RuntimeOrigin::signed(1), 1),
            Error::<Test>::ShopNotClosing
        );
    });
}

#[test]
fn m4_force_close_cleans_entity_primary_shop_index() {
    new_test_ext().execute_with(|| {
        let shop_id = <Shop as ShopProvider<u64>>::create_primary_shop(
            1, b"Primary Shop".to_vec(), ShopType::OnlineStore,
        ).unwrap();

        // EntityPrimaryShop should be set
        assert_eq!(Shop::entity_primary_shop(1), Some(shop_id));

        // force_close the primary shop
        assert_ok!(<Shop as ShopProvider<u64>>::force_close_shop(shop_id));

        // M4: EntityPrimaryShop should be cleaned
        assert!(Shop::entity_primary_shop(1).is_none());
    });
}

#[test]
fn l4_pause_shop_returns_shop_not_active_for_fund_depleted() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // Trigger FundDepleted via deduct_operating_fund
        assert_ok!(<Shop as ShopProvider<u64>>::deduct_operating_fund(1, 950));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::FundDepleted);

        // L4: pause_shop should return ShopNotActive (not ShopAlreadyPaused)
        assert_noop!(
            Shop::pause_shop(RuntimeOrigin::signed(1), 1),
            Error::<Test>::ShopNotActive
        );

        // Trait version should also return ShopNotActive
        assert_noop!(
            <Shop as ShopProvider<u64>>::pause_shop(1),
            Error::<Test>::ShopNotActive
        );
    });
}

// ============================================================================
// force_close_shop (call_index 24) tests
// ============================================================================

#[test]
fn force_close_shop_root_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::Active);

        // Root can force close
        assert_ok!(Shop::force_close_shop(RuntimeOrigin::root(), 1));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::Closed);

        // ShopEntity index cleaned
        assert!(Shop::shop_entity(1).is_none());
    });
}

#[test]
fn force_close_shop_rejects_non_root() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        assert_noop!(
            Shop::force_close_shop(RuntimeOrigin::signed(1), 1),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn force_close_shop_rejects_already_closed() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::force_close_shop(RuntimeOrigin::root(), 1));

        // Cannot force close again
        assert_noop!(
            Shop::force_close_shop(RuntimeOrigin::root(), 1),
            Error::<Test>::ShopAlreadyClosed
        );
    });
}

// ============================================================================
// update_shop: business_hours_cid / policies_cid via update_shop
// ============================================================================

#[test]
fn update_shop_business_hours_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        let cid = bounded_cid(b"QmBusinessHours123");
        assert_ok!(Shop::update_shop(
            RuntimeOrigin::signed(1), 1,
            None, None, None, Some(Some(cid.clone())), None,
        ));
        assert_eq!(Shop::shops(1).unwrap().business_hours_cid, Some(cid));

        assert_ok!(Shop::update_shop(
            RuntimeOrigin::signed(1), 1,
            None, None, None, Some(None), None,
        ));
        assert_eq!(Shop::shops(1).unwrap().business_hours_cid, None);
    });
}

#[test]
fn update_shop_policies_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        let cid = bounded_cid(b"QmReturnPolicy123");
        assert_ok!(Shop::update_shop(
            RuntimeOrigin::signed(1), 1,
            None, None, None, None, Some(Some(cid.clone())),
        ));
        assert_eq!(Shop::shops(1).unwrap().policies_cid, Some(cid));

        assert_ok!(Shop::update_shop(
            RuntimeOrigin::signed(1), 1,
            None, None, None, None, Some(None),
        ));
        assert_eq!(Shop::shops(1).unwrap().policies_cid, None);
    });
}

#[test]
fn update_shop_rejects_empty_business_hours_cid() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_noop!(
            Shop::update_shop(
                RuntimeOrigin::signed(1), 1,
                None, None, None, Some(Some(bounded_cid(b""))), None,
            ),
            Error::<Test>::EmptyCid
        );
    });
}

#[test]
fn update_shop_rejects_empty_policies_cid() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_noop!(
            Shop::update_shop(
                RuntimeOrigin::signed(1), 1,
                None, None, None, None, Some(Some(bounded_cid(b""))),
            ),
            Error::<Test>::EmptyCid
        );
    });
}

// ============================================================================
// P0 Security: withdraw_operating_fund ban/closing protection
// ============================================================================

#[test]
fn withdraw_operating_fund_rejects_banned() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::ban_shop(RuntimeOrigin::root(), 1, bounded_cid(b"violation")));
        assert_noop!(
            Shop::withdraw_operating_fund(RuntimeOrigin::signed(1), 1, 100),
            Error::<Test>::ShopBanned
        );
    });
}

#[test]
fn withdraw_operating_fund_rejects_closing() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));
        assert_noop!(
            Shop::withdraw_operating_fund(RuntimeOrigin::signed(1), 1, 100),
            Error::<Test>::ShopAlreadyClosing
        );
    });
}

#[test]
fn transfer_shop_rejects_banned() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::ban_shop(RuntimeOrigin::root(), 1, bounded_cid(b"violation")));
        assert_noop!(
            Shop::transfer_shop(RuntimeOrigin::signed(1), 1, 2),
            Error::<Test>::ShopBanned
        );
    });
}

#[test]
fn deduct_operating_fund_rejects_banned() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::ban_shop(RuntimeOrigin::root(), 1, bounded_cid(b"violation")));
        assert_noop!(
            <Shop as ShopProvider<u64>>::deduct_operating_fund(1, 100),
            Error::<Test>::ShopBanned
        );
    });
}

#[test]
fn ban_shop_requires_reason() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_noop!(
            Shop::ban_shop(RuntimeOrigin::root(), 1, bounded_cid(b"")),
            Error::<Test>::EmptyCid
        );
    });
}

// ============================================================================
// set_shop_type (call_index 27) tests
// ============================================================================

#[test]
fn set_shop_type_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_eq!(Shop::shops(1).unwrap().shop_type, ShopType::OnlineStore);

        assert_ok!(Shop::set_shop_type(RuntimeOrigin::signed(1), 1, ShopType::PhysicalStore));
        assert_eq!(Shop::shops(1).unwrap().shop_type, ShopType::PhysicalStore);
    });
}

#[test]
fn set_shop_type_rejects_same_type() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        assert_noop!(
            Shop::set_shop_type(RuntimeOrigin::signed(1), 1, ShopType::OnlineStore),
            Error::<Test>::ShopTypeSame
        );
    });
}

#[test]
fn set_shop_type_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        // Add manager — managers cannot change shop type
        assert_ok!(Shop::add_manager(RuntimeOrigin::signed(1), 1, 4));

        assert_noop!(
            Shop::set_shop_type(RuntimeOrigin::signed(4), 1, ShopType::PhysicalStore),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn set_shop_type_rejects_closed_shop() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        close_and_finalize(1, 1);

        assert_noop!(
            Shop::set_shop_type(RuntimeOrigin::signed(1), 1, ShopType::Warehouse),
            Error::<Test>::ShopAlreadyClosed
        );
    });
}

// ============================================================================
// cancel_close_shop (call_index 28) tests
// ============================================================================

#[test]
fn cancel_close_shop_works_restores_active() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // Enter closing state
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::Closing);
        assert!(Shop::shop_closing_at(1).is_some());

        // Cancel close — should restore to Active (has funds)
        assert_ok!(Shop::cancel_close_shop(RuntimeOrigin::signed(1), 1));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::Active);
        assert!(Shop::shop_closing_at(1).is_none());
    });
}

#[test]
fn cancel_close_shop_restores_fund_depleted_when_low_balance() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 200,
        ));

        // Deplete funds first
        assert_ok!(<Shop as ShopProvider<u64>>::deduct_operating_fund(1, 150));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::FundDepleted);

        // Resume to Active first (fund it back to just enough)
        assert_ok!(Shop::fund_operating(RuntimeOrigin::signed(1), 1, 100));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::Active);

        // Now deduct again to make it low, then enter closing
        assert_ok!(<Shop as ShopProvider<u64>>::deduct_operating_fund(1, 100));

        // Manually enter closing by calling close_shop
        // But FundDepleted can't close_shop directly, let's just test the cancel path
        // with sufficient funds first, then test cancel when low:

        // Start fresh: create shop with low balance
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Shop2"), ShopType::OnlineStore, 1000,
        ));
        let shop_id = 2;

        // Enter Closing
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), shop_id));

        // Drain funds from shop account
        let shop_account = Shop::shop_account_id(shop_id);
        let balance = Balances::free_balance(&shop_account);
        // Transfer all out (simulating depletion)
        let _ = <Balances as frame_support::traits::Currency<u64>>::transfer(
            &shop_account, &5, balance.saturating_sub(1),
            frame_support::traits::ExistenceRequirement::KeepAlive,
        );

        // Cancel close — balance < MinOperatingBalance → FundDepleted
        assert_ok!(Shop::cancel_close_shop(RuntimeOrigin::signed(1), shop_id));
        assert_eq!(Shop::shops(shop_id).unwrap().status, ShopOperatingStatus::FundDepleted);
    });
}

#[test]
fn cancel_close_shop_rejects_not_closing() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // Active shop → not closing
        assert_noop!(
            Shop::cancel_close_shop(RuntimeOrigin::signed(1), 1),
            Error::<Test>::ShopNotClosing
        );
    });
}

#[test]
fn cancel_close_shop_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::close_shop(RuntimeOrigin::signed(1), 1));

        // Non-owner cannot cancel close
        assert_noop!(
            Shop::cancel_close_shop(RuntimeOrigin::signed(2), 1),
            Error::<Test>::NotAuthorized
        );
    });
}

// ============================================================================
// policies_cid field in create_shop
// ============================================================================

#[test]
fn create_shop_initializes_policies_cid_none() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_eq!(Shop::shops(1).unwrap().policies_cid, None);
    });
}

// ==================== EntityLocked 回归测试 ====================

#[test]
fn entity_locked_rejects_create_shop() {
    new_test_ext().execute_with(|| {
        set_entity_locked(1);
        assert_noop!(
            Shop::create_shop(
                RuntimeOrigin::signed(1), 1,
                bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_update_shop() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        set_entity_locked(1);
        assert_noop!(
            Shop::update_shop(
                RuntimeOrigin::signed(1), 1,
                Some(bounded_name(b"New Name")), None, None, None, None,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

// ==================== Feature: Shop 创建数量预检 ====================

#[test]
fn create_shop_rejects_when_limit_reached() {
    new_test_ext().execute_with(|| {
        // MaxShopsPerEntity = 5
        for i in 0..5 {
            let name = bounded_name(format!("Shop {}", i).as_bytes());
            assert_ok!(Shop::create_shop(
                RuntimeOrigin::signed(1), 1,
                name, ShopType::OnlineStore, 1000,
            ));
        }
        // 6th shop should fail
        assert_noop!(
            Shop::create_shop(
                RuntimeOrigin::signed(1), 1,
                bounded_name(b"Shop 6"), ShopType::OnlineStore, 1000,
            ),
            Error::<Test>::ShopLimitReached
        );
    });
}

// ==================== Feature: resign_manager ====================

#[test]
fn resign_manager_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        // Add account 3 as manager
        assert_ok!(Shop::add_manager(RuntimeOrigin::signed(1), 1, 3));
        assert!(Shop::shops(1).unwrap().managers.contains(&3));

        // Manager 3 resigns
        assert_ok!(Shop::resign_manager(RuntimeOrigin::signed(3), 1));
        assert!(!Shop::shops(1).unwrap().managers.contains(&3));
    });
}

#[test]
fn resign_manager_rejects_non_manager() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        // Account 5 is not a manager
        assert_noop!(
            Shop::resign_manager(RuntimeOrigin::signed(5), 1),
            Error::<Test>::NotManager
        );
    });
}

#[test]
fn resign_manager_rejects_shop_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Shop::resign_manager(RuntimeOrigin::signed(3), 999),
            Error::<Test>::ShopNotFound
        );
    });
}

// ==================== Feature: ban_shop / unban_shop ====================

#[test]
fn ban_shop_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::ban_shop(RuntimeOrigin::root(), 1, bounded_cid(b"violation")));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::Banned);
    });
}

#[test]
fn ban_shop_rejects_non_root() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_noop!(
            Shop::ban_shop(RuntimeOrigin::signed(1), 1, bounded_cid(b"violation")),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn ban_shop_rejects_already_banned() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::ban_shop(RuntimeOrigin::root(), 1, bounded_cid(b"violation")));
        assert_noop!(
            Shop::ban_shop(RuntimeOrigin::root(), 1, bounded_cid(b"violation")),
            Error::<Test>::ShopBanned
        );
    });
}

#[test]
fn ban_shop_rejects_closed() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Shop A"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Shop B"), ShopType::OnlineStore, 1000,
        ));
        close_and_finalize(1, 2);
        assert_noop!(
            Shop::ban_shop(RuntimeOrigin::root(), 2, bounded_cid(b"violation")),
            Error::<Test>::ShopAlreadyClosed
        );
    });
}

#[test]
fn unban_shop_restores_previous_status() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        // Pause shop first, then ban
        assert_ok!(Shop::pause_shop(RuntimeOrigin::signed(1), 1));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::Paused);

        assert_ok!(Shop::ban_shop(RuntimeOrigin::root(), 1, bounded_cid(b"violation")));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::Banned);

        // Unban should restore to Paused
        assert_ok!(Shop::unban_shop(RuntimeOrigin::root(), 1));
        assert_eq!(Shop::shops(1).unwrap().status, ShopOperatingStatus::Paused);
    });
}

#[test]
fn unban_shop_rejects_not_banned() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_noop!(
            Shop::unban_shop(RuntimeOrigin::root(), 1),
            Error::<Test>::ShopNotBanned
        );
    });
}

#[test]
fn resume_shop_rejects_banned() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::ban_shop(RuntimeOrigin::root(), 1, bounded_cid(b"violation")));
        assert_noop!(
            Shop::resume_shop(RuntimeOrigin::signed(1), 1),
            Error::<Test>::ShopBanned
        );
    });
}

#[test]
fn banned_shop_blocks_update() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::ban_shop(RuntimeOrigin::root(), 1, bounded_cid(b"violation")));
        assert_noop!(
            Shop::update_shop(
                RuntimeOrigin::signed(1), 1,
                Some(bounded_name(b"New Name")), None, None, None, None,
            ),
            Error::<Test>::ShopAlreadyClosed
        );
    });
}

#[test]
fn banned_shop_blocks_fund_operating() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::ban_shop(RuntimeOrigin::root(), 1, bounded_cid(b"violation")));
        assert_noop!(
            Shop::fund_operating(RuntimeOrigin::signed(1), 1, 500),
            Error::<Test>::ShopBanned
        );
    });
}

#[test]
fn force_pause_rejects_banned() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::ban_shop(RuntimeOrigin::root(), 1, bounded_cid(b"violation")));
        assert_noop!(
            Shop::force_pause_shop(RuntimeOrigin::root(), 1),
            Error::<Test>::ShopBanned
        );
    });
}

// ==================== Feature: Entity active check ====================

#[test]
fn entity_not_active_rejects_update_shop() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        set_entity_suspended(1);
        assert_noop!(
            Shop::update_shop(
                RuntimeOrigin::signed(1), 1,
                Some(bounded_name(b"New Name")), None, None, None, None,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

// ==================== Feature: integrity_test ====================

#[test]
fn integrity_test_passes() {
    // This test verifies that the integrity_test hook does not panic
    // with the current mock configuration.
    use frame_support::traits::Hooks;
    new_test_ext().execute_with(|| {
        <Shop as Hooks<u64>>::integrity_test();
    });
}

// ============================================================================
// Audit Round 5: Regression Tests
// ============================================================================

/// M1: close_shop 拒绝 Banned 状态 — 防止 owner 绕过封禁
#[test]
fn m1_close_shop_rejects_banned() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));

        // Root ban the shop
        assert_ok!(Shop::ban_shop(RuntimeOrigin::root(), 1, bounded_cid(b"violation")));

        // Owner tries to close — should be rejected
        assert_noop!(
            Shop::close_shop(RuntimeOrigin::signed(1), 1),
            Error::<Test>::ShopBanned
        );
    });
}

/// M3: transfer_shop 拒绝目标 Entity 已达 Shop 数量上限
#[test]
fn m3_transfer_shop_rejects_target_entity_at_limit() {
    new_test_ext().execute_with(|| {
        // Create MaxShopsPerEntity(5) shops for entity 2
        for i in 0..5 {
            assert_ok!(Shop::create_shop(
                RuntimeOrigin::signed(2), 2,
                bounded_name(format!("Shop {}", i).as_bytes()),
                ShopType::OnlineStore, 100,
            ));
        }

        // Create a shop for entity 1
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Transfer Shop"), ShopType::OnlineStore, 100,
        ));
        let transfer_shop_id = 6; // shops 1-5 for entity 2, 6 for entity 1

        // Transfer entity 1's shop to entity 2 — should fail (entity 2 at limit)
        assert_noop!(
            Shop::transfer_shop(RuntimeOrigin::signed(1), transfer_shop_id, 2),
            Error::<Test>::ShopLimitReached
        );
    });
}

/// M3: transfer_shop 在目标 Entity 未达上限时成功
#[test]
fn m3_transfer_shop_works_when_target_has_capacity() {
    new_test_ext().execute_with(|| {
        // Entity 2 has 1 shop
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(2), 2,
            bounded_name(b"Entity2 Shop"), ShopType::OnlineStore, 100,
        ));

        // Entity 1 has 1 shop
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Transfer Shop"), ShopType::OnlineStore, 100,
        ));

        // Transfer entity 1's shop to entity 2 — should succeed
        assert_ok!(Shop::transfer_shop(RuntimeOrigin::signed(1), 2, 2));
        assert_eq!(Shop::shops(2).unwrap().entity_id, 2);
    });
}

/// M1: banned shop 不可添加管理员
#[test]
fn m1_add_manager_rejects_banned() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::ban_shop(RuntimeOrigin::root(), 1, bounded_cid(b"violation")));
        assert_noop!(
            Shop::add_manager(RuntimeOrigin::signed(1), 1, 2),
            Error::<Test>::ShopAlreadyClosed
        );
    });
}

/// M1: banned shop 不可移除管理员
#[test]
fn m1_remove_manager_rejects_banned() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::add_manager(RuntimeOrigin::signed(1), 1, 2));
        assert_ok!(Shop::ban_shop(RuntimeOrigin::root(), 1, bounded_cid(b"violation")));
        assert_noop!(
            Shop::remove_manager(RuntimeOrigin::signed(1), 1, 2),
            Error::<Test>::ShopAlreadyClosed
        );
    });
}

/// M2: banned shop 不可设为主店
#[test]
fn m2_set_primary_shop_rejects_banned() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Shop A"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Shop B"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::ban_shop(RuntimeOrigin::root(), 2, bounded_cid(b"violation")));
        assert_noop!(
            Shop::set_primary_shop(RuntimeOrigin::signed(1), 1, 2),
            Error::<Test>::ShopAlreadyClosed
        );
    });
}

/// M3: entity 非活跃时不可添加管理员
#[test]
fn m3_add_manager_rejects_entity_not_active() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        set_entity_suspended(1);
        assert_noop!(
            Shop::add_manager(RuntimeOrigin::signed(1), 1, 2),
            Error::<Test>::EntityNotActive
        );
    });
}

/// L2: update_shop_rating 拒绝 rating=0 和 rating>5
#[test]
fn l2_update_shop_rating_rejects_invalid_rating() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_noop!(
            <Shop as ShopProvider<u64>>::update_shop_rating(1, 0),
            Error::<Test>::InvalidRating
        );
        assert_noop!(
            <Shop as ShopProvider<u64>>::update_shop_rating(1, 6),
            Error::<Test>::InvalidRating
        );
        assert_ok!(
            <Shop as ShopProvider<u64>>::update_shop_rating(1, 5)
        );
    });
}

/// L3: trait resume_shop 对 banned shop 返回 ShopBanned 而非 ShopNotPaused
#[test]
fn l3_trait_resume_shop_returns_shop_banned() {
    new_test_ext().execute_with(|| {
        assert_ok!(Shop::create_shop(
            RuntimeOrigin::signed(1), 1,
            bounded_name(b"Test Shop"), ShopType::OnlineStore, 1000,
        ));
        assert_ok!(Shop::ban_shop(RuntimeOrigin::root(), 1, bounded_cid(b"violation")));
        assert_noop!(
            <Shop as ShopProvider<u64>>::resume_shop(1),
            Error::<Test>::ShopBanned
        );
    });
}
