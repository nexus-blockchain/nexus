use crate::{mock::*, pallet::*};
use frame_support::{assert_noop, assert_ok};
use pallet_entity_common::{EntityStatus, EntityType, GovernanceMode, EntityProvider};

// ==================== Helper ====================

fn create_default_entity(who: u64) -> u64 {
    assert_ok!(EntityRegistry::create_entity(
        RuntimeOrigin::signed(who),
        b"Test Entity".to_vec(),
        None,
        None,
    ));
    EntityRegistry::next_entity_id() - 1
}

// ==================== create_entity ====================

#[test]
fn create_entity_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_eq!(id, 0);

        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.owner, ALICE);
        assert_eq!(entity.name.to_vec(), b"Test Entity".to_vec());
        assert_eq!(entity.status, EntityStatus::Active);
        assert_eq!(entity.entity_type, EntityType::Merchant);
        assert_eq!(entity.governance_mode, GovernanceMode::None);
        assert!(!entity.verified);
        // shop_id set by MockShopProvider callback (not called in mock, so stays 0)
        // UserEntity index
        assert!(UserEntity::<Test>::get(ALICE).contains(&id));
        // Stats
        let stats = EntityStats::<Test>::get();
        assert_eq!(stats.total_entities, 1);
        assert_eq!(stats.active_entities, 1);
        // NextEntityId
        assert_eq!(NextEntityId::<Test>::get(), 1);
    });
}

#[test]
fn create_entity_fails_name_empty() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityRegistry::create_entity(
                RuntimeOrigin::signed(ALICE),
                vec![],
                None,
                None,
            ),
            Error::<Test>::NameEmpty
        );
    });
}

#[test]
fn create_entity_fails_name_too_long() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityRegistry::create_entity(
                RuntimeOrigin::signed(ALICE),
                vec![0u8; 65],
                None,
                None,
            ),
            Error::<Test>::NameTooLong
        );
    });
}

#[test]
fn create_entity_fails_max_entities_reached() {
    new_test_ext().execute_with(|| {
        // MaxEntitiesPerUser = 3
        create_default_entity(ALICE);
        create_default_entity(ALICE);
        create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::create_entity(
                RuntimeOrigin::signed(ALICE),
                b"Fourth".to_vec(),
                None,
                None,
            ),
            Error::<Test>::MaxEntitiesReached
        );
    });
}

#[test]
fn create_entity_fails_insufficient_balance() {
    new_test_ext().execute_with(|| {
        // EVE has very low balance (10_000_000_000 < 100 tokens needed)
        assert_noop!(
            EntityRegistry::create_entity(
                RuntimeOrigin::signed(EVE),
                b"Poor Entity".to_vec(),
                None,
                None,
            ),
            Error::<Test>::InsufficientBalanceForInitialFund
        );
    });
}

// ==================== update_entity ====================

#[test]
fn update_entity_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::update_entity(
            RuntimeOrigin::signed(ALICE),
            id,
            Some(b"New Name".to_vec()),
            Some(b"logo_cid".to_vec()),
            Some(b"desc_cid".to_vec()),
            Some(b"meta_uri".to_vec()),
        ));
        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.name.to_vec(), b"New Name".to_vec());
        assert!(entity.logo_cid.is_some());
        assert!(entity.description_cid.is_some());
        assert!(entity.metadata_uri.is_some());
    });
}

#[test]
fn update_entity_fails_not_owner() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::update_entity(
                RuntimeOrigin::signed(BOB),
                id,
                Some(b"Hijack".to_vec()),
                None, None, None,
            ),
            Error::<Test>::NotEntityOwner
        );
    });
}

#[test]
fn update_entity_fails_empty_name() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::update_entity(
                RuntimeOrigin::signed(ALICE),
                id,
                Some(vec![]),
                None, None, None,
            ),
            Error::<Test>::NameEmpty
        );
    });
}

#[test]
fn update_entity_fails_banned() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Ban the entity
        assert_ok!(EntityRegistry::ban_entity(
            RuntimeOrigin::root(),
            id,
            false,
        ));
        assert_noop!(
            EntityRegistry::update_entity(
                RuntimeOrigin::signed(ALICE),
                id,
                Some(b"New".to_vec()),
                None, None, None,
            ),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

// ==================== request_close_entity ====================

#[test]
fn request_close_entity_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::request_close_entity(
            RuntimeOrigin::signed(ALICE),
            id,
        ));
        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.status, EntityStatus::PendingClose);
        let stats = EntityStats::<Test>::get();
        assert_eq!(stats.active_entities, 0); // decremented from 1
    });
}

#[test]
fn request_close_entity_fails_not_owner() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::request_close_entity(RuntimeOrigin::signed(BOB), id),
            Error::<Test>::NotEntityOwner
        );
    });
}

#[test]
fn request_close_entity_fails_already_closed() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_ok!(EntityRegistry::approve_close_entity(RuntimeOrigin::root(), id));
        // Now closed
        assert_noop!(
            EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

// ==================== top_up_fund ====================

#[test]
fn top_up_fund_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let amount = 10_000_000_000_000u128; // 10 tokens
        assert_ok!(EntityRegistry::top_up_fund(
            RuntimeOrigin::signed(ALICE),
            id,
            amount,
        ));
        let balance = EntityRegistry::get_entity_fund_balance(id);
        assert_eq!(balance, EXPECTED_INITIAL_FUND + amount);
    });
}

#[test]
fn top_up_fund_fails_zero_amount() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::top_up_fund(RuntimeOrigin::signed(ALICE), id, 0),
            Error::<Test>::ZeroAmount
        );
    });
}

#[test]
fn top_up_fund_fails_not_owner() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::top_up_fund(RuntimeOrigin::signed(BOB), id, 100),
            Error::<Test>::NotEntityOwner
        );
    });
}

#[test]
fn top_up_fund_restores_suspended_entity() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Suspend via governance
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id));
        assert_eq!(
            Entities::<Test>::get(id).unwrap().status,
            EntityStatus::Suspended
        );
        // top_up_fund should NOT auto-resume governance-suspended entity
        assert_ok!(EntityRegistry::top_up_fund(
            RuntimeOrigin::signed(ALICE),
            id,
            10_000_000_000_000,
        ));
        // Still suspended (GovernanceSuspended = true)
        assert_eq!(
            Entities::<Test>::get(id).unwrap().status,
            EntityStatus::Suspended
        );
    });
}

// ==================== approve_entity ====================

#[test]
fn approve_entity_works_after_reopen() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Close workflow
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_ok!(EntityRegistry::approve_close_entity(RuntimeOrigin::root(), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Closed);
        // Reopen → Pending
        assert_ok!(EntityRegistry::reopen_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Pending);
        // Approve → Active
        assert_ok!(EntityRegistry::approve_entity(RuntimeOrigin::root(), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);
    });
}

#[test]
fn approve_entity_fails_not_pending() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Active, not Pending
        assert_noop!(
            EntityRegistry::approve_entity(RuntimeOrigin::root(), id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

#[test]
fn approve_entity_fails_not_governance() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::approve_entity(RuntimeOrigin::signed(ALICE), id),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ==================== approve_close_entity ====================

#[test]
fn approve_close_entity_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let alice_before = Balances::free_balance(ALICE);
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_ok!(EntityRegistry::approve_close_entity(RuntimeOrigin::root(), id));

        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.status, EntityStatus::Closed);
        // Fund refunded to owner
        let alice_after = Balances::free_balance(ALICE);
        assert!(alice_after > alice_before - EXPECTED_INITIAL_FUND);
        // UserEntity cleared
        assert!(!UserEntity::<Test>::get(ALICE).contains(&id));
    });
}

#[test]
fn approve_close_entity_fails_not_pending_close() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::approve_close_entity(RuntimeOrigin::root(), id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

// ==================== suspend_entity ====================

#[test]
fn suspend_entity_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Suspended);
        assert!(GovernanceSuspended::<Test>::get(id));
        let stats = EntityStats::<Test>::get();
        assert_eq!(stats.active_entities, 0);
    });
}

#[test]
fn suspend_entity_fails_not_active() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id));
        // Already suspended
        assert_noop!(
            EntityRegistry::suspend_entity(RuntimeOrigin::root(), id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

// ==================== resume_entity ====================

#[test]
fn resume_entity_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id));
        assert_ok!(EntityRegistry::resume_entity(RuntimeOrigin::root(), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);
        assert!(!GovernanceSuspended::<Test>::get(id));
        let stats = EntityStats::<Test>::get();
        assert_eq!(stats.active_entities, 1);
    });
}

#[test]
fn resume_entity_fails_not_suspended() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::resume_entity(RuntimeOrigin::root(), id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

// ==================== ban_entity ====================

#[test]
fn ban_entity_confiscate_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let platform_before = Balances::free_balance(99);
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, true));
        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.status, EntityStatus::Banned);
        // Fund confiscated to platform
        let platform_after = Balances::free_balance(99);
        assert!(platform_after > platform_before);
        // UserEntity cleared
        assert!(!UserEntity::<Test>::get(ALICE).contains(&id));
    });
}

#[test]
fn ban_entity_refund_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let alice_before = Balances::free_balance(ALICE);
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false));
        let alice_after = Balances::free_balance(ALICE);
        // Fund refunded
        assert!(alice_after > alice_before);
    });
}

#[test]
fn ban_entity_fails_already_banned() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false));
        assert_noop!(
            EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

#[test]
fn ban_entity_from_suspended() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id));
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, true));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Banned);
    });
}

#[test]
fn ban_entity_from_pending_close() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Banned);
    });
}

// ==================== add_admin ====================

#[test]
fn add_admin_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB));
        let entity = Entities::<Test>::get(id).unwrap();
        assert!(entity.admins.contains(&BOB));
    });
}

#[test]
fn add_admin_fails_not_owner() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::add_admin(RuntimeOrigin::signed(BOB), id, CHARLIE),
            Error::<Test>::NotEntityOwner
        );
    });
}

#[test]
fn add_admin_fails_already_exists() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB));
        assert_noop!(
            EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB),
            Error::<Test>::AdminAlreadyExists
        );
    });
}

#[test]
fn add_admin_fails_owner_as_admin() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, ALICE),
            Error::<Test>::AdminAlreadyExists
        );
    });
}

#[test]
fn add_admin_fails_max_reached() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // MaxAdmins = 3
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB));
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, CHARLIE));
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, DAVE));
        assert_noop!(
            EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, EVE),
            Error::<Test>::MaxAdminsReached
        );
    });
}

#[test]
fn add_admin_fails_banned_entity() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false));
        assert_noop!(
            EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

// ==================== remove_admin ====================

#[test]
fn remove_admin_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB));
        assert_ok!(EntityRegistry::remove_admin(RuntimeOrigin::signed(ALICE), id, BOB));
        let entity = Entities::<Test>::get(id).unwrap();
        assert!(!entity.admins.contains(&BOB));
    });
}

#[test]
fn remove_admin_fails_not_found() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::remove_admin(RuntimeOrigin::signed(ALICE), id, BOB),
            Error::<Test>::AdminNotFound
        );
    });
}

#[test]
fn remove_admin_fails_cannot_remove_owner() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::remove_admin(RuntimeOrigin::signed(ALICE), id, ALICE),
            Error::<Test>::CannotRemoveOwner
        );
    });
}

#[test]
fn remove_admin_fails_closed_entity() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB));
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_ok!(EntityRegistry::approve_close_entity(RuntimeOrigin::root(), id));
        assert_noop!(
            EntityRegistry::remove_admin(RuntimeOrigin::signed(ALICE), id, BOB),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

// ==================== transfer_ownership ====================

#[test]
fn transfer_ownership_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::transfer_ownership(
            RuntimeOrigin::signed(ALICE), id, BOB,
        ));
        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.owner, BOB);
        assert!(!UserEntity::<Test>::get(ALICE).contains(&id));
        assert!(UserEntity::<Test>::get(BOB).contains(&id));
    });
}

#[test]
fn transfer_ownership_removes_new_owner_from_admins() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB));
        assert_ok!(EntityRegistry::transfer_ownership(
            RuntimeOrigin::signed(ALICE), id, BOB,
        ));
        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.owner, BOB);
        assert!(!entity.admins.contains(&BOB));
    });
}

#[test]
fn transfer_ownership_fails_not_owner() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::transfer_ownership(RuntimeOrigin::signed(BOB), id, CHARLIE),
            Error::<Test>::NotEntityOwner
        );
    });
}

#[test]
fn transfer_ownership_fails_banned() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false));
        assert_noop!(
            EntityRegistry::transfer_ownership(RuntimeOrigin::signed(ALICE), id, BOB),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

// ==================== upgrade_entity_type ====================

#[test]
fn upgrade_entity_type_owner_merchant_to_community() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::upgrade_entity_type(
            RuntimeOrigin::signed(ALICE),
            id,
            EntityType::Community,
            GovernanceMode::None,
        ));
        assert_eq!(Entities::<Test>::get(id).unwrap().entity_type, EntityType::Community);
    });
}

#[test]
fn upgrade_entity_type_community_to_dao_requires_governance() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Merchant → Community
        assert_ok!(EntityRegistry::upgrade_entity_type(
            RuntimeOrigin::signed(ALICE), id, EntityType::Community, GovernanceMode::None,
        ));
        // Community → DAO (no governance = error)
        assert_noop!(
            EntityRegistry::upgrade_entity_type(
                RuntimeOrigin::signed(ALICE), id, EntityType::DAO, GovernanceMode::None,
            ),
            Error::<Test>::DAORequiresGovernance
        );
        // Community → DAO (with governance = ok)
        assert_ok!(EntityRegistry::upgrade_entity_type(
            RuntimeOrigin::signed(ALICE), id, EntityType::DAO, GovernanceMode::FullDAO,
        ));
    });
}

#[test]
fn upgrade_entity_type_invalid_path() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Merchant → Community
        assert_ok!(EntityRegistry::upgrade_entity_type(
            RuntimeOrigin::signed(ALICE), id, EntityType::Community, GovernanceMode::None,
        ));
        // Community → Enterprise (not allowed for owner)
        assert_noop!(
            EntityRegistry::upgrade_entity_type(
                RuntimeOrigin::signed(ALICE), id, EntityType::Enterprise, GovernanceMode::None,
            ),
            Error::<Test>::InvalidEntityTypeUpgrade
        );
    });
}

#[test]
fn upgrade_entity_type_governance_bypasses_path() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Merchant → Community
        assert_ok!(EntityRegistry::upgrade_entity_type(
            RuntimeOrigin::signed(ALICE), id, EntityType::Community, GovernanceMode::None,
        ));
        // Governance can do Community → Enterprise
        assert_ok!(EntityRegistry::upgrade_entity_type(
            RuntimeOrigin::root(), id, EntityType::Enterprise, GovernanceMode::None,
        ));
        assert_eq!(Entities::<Test>::get(id).unwrap().entity_type, EntityType::Enterprise);
    });
}

#[test]
fn upgrade_entity_type_fails_banned() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false));
        assert_noop!(
            EntityRegistry::upgrade_entity_type(
                RuntimeOrigin::root(), id, EntityType::DAO, GovernanceMode::FullDAO,
            ),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

// ==================== change_governance_mode ====================

#[test]
fn change_governance_mode_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::change_governance_mode(
            RuntimeOrigin::root(), id, GovernanceMode::Committee,
        ));
        assert_eq!(
            Entities::<Test>::get(id).unwrap().governance_mode,
            GovernanceMode::Committee,
        );
    });
}

#[test]
fn change_governance_mode_dao_cannot_be_none() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Upgrade to DAO
        assert_ok!(EntityRegistry::upgrade_entity_type(
            RuntimeOrigin::root(), id, EntityType::DAO, GovernanceMode::FullDAO,
        ));
        // Cannot set governance to None for DAO
        assert_noop!(
            EntityRegistry::change_governance_mode(RuntimeOrigin::root(), id, GovernanceMode::None),
            Error::<Test>::DAORequiresGovernance
        );
    });
}

// ==================== verify_entity ====================

#[test]
fn verify_entity_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::verify_entity(RuntimeOrigin::root(), id));
        assert!(EntityRegistry::is_verified(id));
    });
}

#[test]
fn verify_entity_fails_already_verified() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::verify_entity(RuntimeOrigin::root(), id));
        assert_noop!(
            EntityRegistry::verify_entity(RuntimeOrigin::root(), id),
            Error::<Test>::AlreadyVerified
        );
    });
}

#[test]
fn verify_entity_fails_banned() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false));
        assert_noop!(
            EntityRegistry::verify_entity(RuntimeOrigin::root(), id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

// ==================== reopen_entity ====================

#[test]
fn reopen_entity_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Close workflow
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_ok!(EntityRegistry::approve_close_entity(RuntimeOrigin::root(), id));
        // Reopen
        assert_ok!(EntityRegistry::reopen_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Pending);
        // UserEntity restored
        assert!(UserEntity::<Test>::get(ALICE).contains(&id));
    });
}

#[test]
fn reopen_entity_fails_not_closed() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::reopen_entity(RuntimeOrigin::signed(ALICE), id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

#[test]
fn reopen_entity_fails_banned() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false));
        // Banned, not Closed
        assert_noop!(
            EntityRegistry::reopen_entity(RuntimeOrigin::signed(ALICE), id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

#[test]
fn reopen_entity_fails_not_owner() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_ok!(EntityRegistry::approve_close_entity(RuntimeOrigin::root(), id));
        assert_noop!(
            EntityRegistry::reopen_entity(RuntimeOrigin::signed(BOB), id),
            Error::<Test>::NotEntityOwner
        );
    });
}

// ==================== EntityProvider trait ====================

#[test]
fn entity_provider_basic_methods() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert!(<EntityRegistry as EntityProvider<u64>>::entity_exists(id));
        assert!(<EntityRegistry as EntityProvider<u64>>::is_entity_active(id));
        assert_eq!(
            <EntityRegistry as EntityProvider<u64>>::entity_status(id),
            Some(EntityStatus::Active)
        );
        assert_eq!(
            <EntityRegistry as EntityProvider<u64>>::entity_owner(id),
            Some(ALICE)
        );
    });
}

#[test]
fn entity_provider_is_entity_admin() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB));
        assert!(<EntityRegistry as EntityProvider<u64>>::is_entity_admin(id, &ALICE));
        assert!(<EntityRegistry as EntityProvider<u64>>::is_entity_admin(id, &BOB));
        assert!(!<EntityRegistry as EntityProvider<u64>>::is_entity_admin(id, &CHARLIE));
    });
}

#[test]
fn entity_provider_register_unregister_shop() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Entity starts with primary_shop_id = 0
        // Manually call register_shop
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 42));
        assert_eq!(Entities::<Test>::get(id).unwrap().primary_shop_id, 42);
        let shops = <EntityRegistry as EntityProvider<u64>>::entity_shops(id);
        assert_eq!(shops, vec![42]);

        // Register a second shop (1:N)
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 43));
        let shops = <EntityRegistry as EntityProvider<u64>>::entity_shops(id);
        assert_eq!(shops, vec![42, 43]);
        // primary_shop_id remains 42 (first registered)
        assert_eq!(Entities::<Test>::get(id).unwrap().primary_shop_id, 42);

        // Unregister shop 42 (was primary)
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::unregister_shop(id, 42));
        // primary_shop_id should auto-reassign to 43
        assert_eq!(Entities::<Test>::get(id).unwrap().primary_shop_id, 43);
        let shops = <EntityRegistry as EntityProvider<u64>>::entity_shops(id);
        assert_eq!(shops, vec![43]);

        // Unregister last shop
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::unregister_shop(id, 43));
        assert_eq!(Entities::<Test>::get(id).unwrap().primary_shop_id, 0);
        assert!(<EntityRegistry as EntityProvider<u64>>::entity_shops(id).is_empty());
    });
}

#[test]
fn entity_provider_unregister_shop_fails_not_registered() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 42));
        // Shop 99 not registered
        assert_noop!(
            <EntityRegistry as EntityProvider<u64>>::unregister_shop(id, 99),
            Error::<Test>::ShopNotRegistered
        );
    });
}

#[test]
fn entity_provider_register_shop_duplicate_rejected() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 42));
        // Duplicate shop_id rejected
        assert_noop!(
            <EntityRegistry as EntityProvider<u64>>::register_shop(id, 42),
            Error::<Test>::ShopLimitReached
        );
    });
}

#[test]
fn entity_provider_update_entity_stats() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::update_entity_stats(id, 1000, 5));
        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.total_sales, 1000u128);
        assert_eq!(entity.total_orders, 5);
    });
}

// ==================== deduct_operating_fee ====================

#[test]
fn deduct_operating_fee_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let before = EntityRegistry::get_entity_fund_balance(id);
        let fee = 1_000_000_000_000u128; // 1 token
        assert_ok!(EntityRegistry::deduct_operating_fee(id, fee, FeeType::IpfsPin));
        let after = EntityRegistry::get_entity_fund_balance(id);
        assert_eq!(after, before - fee);
    });
}

#[test]
fn deduct_operating_fee_suspends_on_low_fund() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let balance = EntityRegistry::get_entity_fund_balance(id);
        // Deduct almost all funds, leaving below min_operating_balance (0.1 token)
        let fee = balance - 50_000_000_000; // leave 0.05 tokens
        assert_ok!(EntityRegistry::deduct_operating_fee(id, fee, FeeType::StorageRent));
        // Entity should be suspended
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Suspended);
    });
}

#[test]
fn deduct_operating_fee_fails_insufficient() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let balance = EntityRegistry::get_entity_fund_balance(id);
        assert_noop!(
            EntityRegistry::deduct_operating_fee(id, balance + 1, FeeType::Other),
            Error::<Test>::InsufficientOperatingFund
        );
    });
}

// ==================== calculate_initial_fund ====================

#[test]
fn calculate_initial_fund_works() {
    new_test_ext().execute_with(|| {
        let fund = EntityRegistry::calculate_initial_fund().unwrap();
        assert_eq!(fund, EXPECTED_INITIAL_FUND);
    });
}

// ==================== Helper functions ====================

#[test]
fn is_admin_helper() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert!(EntityRegistry::is_admin(id, &ALICE));
        assert!(!EntityRegistry::is_admin(id, &BOB));
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB));
        assert!(EntityRegistry::is_admin(id, &BOB));
    });
}

#[test]
fn fund_health_levels() {
    new_test_ext().execute_with(|| {
        use crate::pallet::FundHealth;
        // Healthy: > warning_threshold (1 token)
        assert_eq!(
            EntityRegistry::get_fund_health(2_000_000_000_000u128),
            FundHealth::Healthy,
        );
        // Warning: > min (0.1 token) && <= warning (1 token)
        assert_eq!(
            EntityRegistry::get_fund_health(500_000_000_000u128),
            FundHealth::Warning,
        );
        // Critical: > 0 && <= min (0.1 token)
        assert_eq!(
            EntityRegistry::get_fund_health(50_000_000_000u128),
            FundHealth::Critical,
        );
        // Depleted: 0
        assert_eq!(
            EntityRegistry::get_fund_health(0u128),
            FundHealth::Depleted,
        );
    });
}

// ==================== Suspended entity + request_close ====================

#[test]
fn suspended_entity_can_request_close() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id));
        // Suspended entity can still request close
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::PendingClose);
        // active_entities should NOT be decremented again (was already 0 from suspend)
        let stats = EntityStats::<Test>::get();
        assert_eq!(stats.active_entities, 0);
    });
}

// ==================== Multiple entities per user ====================

#[test]
fn multiple_entities_per_user() {
    new_test_ext().execute_with(|| {
        let id1 = create_default_entity(ALICE);
        let id2 = create_default_entity(ALICE);
        let id3 = create_default_entity(ALICE);
        let entities = UserEntity::<Test>::get(ALICE);
        assert_eq!(entities.len(), 3);
        assert!(entities.contains(&id1));
        assert!(entities.contains(&id2));
        assert!(entities.contains(&id3));
    });
}
