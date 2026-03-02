use crate::{mock::*, pallet::*};
use frame_support::{assert_noop, assert_ok, traits::Currency};
use pallet_entity_common::{EntityStatus, EntityType, GovernanceMode, EntityProvider};

// ==================== Helper ====================

fn create_default_entity(who: u64) -> u64 {
    assert_ok!(EntityRegistry::create_entity(
        RuntimeOrigin::signed(who),
        b"Test Entity".to_vec(),
        None,
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
        // C1: Entity ID 从 1 开始（避免与 primary_shop_id=0 哨兵值冲突）
        assert_eq!(id, 1);

        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.owner, ALICE);
        assert_eq!(entity.name.to_vec(), b"Test Entity".to_vec());
        assert_eq!(entity.status, EntityStatus::Active);
        assert_eq!(entity.entity_type, EntityType::Merchant);
        assert_eq!(entity.governance_mode, GovernanceMode::None);
        assert!(!entity.verified);
        // UserEntity index
        assert!(UserEntity::<Test>::get(ALICE).contains(&id));
        // Stats
        let stats = EntityStats::<Test>::get();
        assert_eq!(stats.total_entities, 1);
        assert_eq!(stats.active_entities, 1);
        // NextEntityId
        assert_eq!(NextEntityId::<Test>::get(), 2);
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
                vec![b'A'; 65],
                None,
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
        // Duplicate shop_id rejected (H5: 区分重复注册 vs 容量已满)
        assert_noop!(
            <EntityRegistry as EntityProvider<u64>>::register_shop(id, 42),
            Error::<Test>::ShopAlreadyRegistered
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

// ==================== H2: transfer_ownership pre-check ====================

#[test]
fn h2_transfer_ownership_fails_new_owner_at_capacity() {
    new_test_ext().execute_with(|| {
        // BOB creates 3 entities (MaxEntitiesPerUser = 3)
        create_default_entity(BOB);
        create_default_entity(BOB);
        create_default_entity(BOB);
        // ALICE creates one
        let alice_id = create_default_entity(ALICE);
        // Transfer to BOB should fail — BOB already at capacity
        assert_noop!(
            EntityRegistry::transfer_ownership(RuntimeOrigin::signed(ALICE), alice_id, BOB),
            Error::<Test>::MaxEntitiesReached
        );
        // Verify ALICE still owns the entity (no partial write)
        let entity = Entities::<Test>::get(alice_id).unwrap();
        assert_eq!(entity.owner, ALICE);
        assert!(UserEntity::<Test>::get(ALICE).contains(&alice_id));
    });
}

// ==================== H3: ban clears GovernanceSuspended ====================

#[test]
fn h3_ban_clears_governance_suspended() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Governance suspend
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id));
        assert!(GovernanceSuspended::<Test>::get(id));
        // Ban from suspended
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false));
        // GovernanceSuspended should be cleared
        assert!(!GovernanceSuspended::<Test>::get(id));
    });
}

// ==================== H4: deduct_operating_fee entity status check ====================

#[test]
fn h4_deduct_fee_fails_banned_entity() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Ban without confiscating (treasury still has funds)
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false));
        // Deduct should fail
        assert_noop!(
            EntityRegistry::deduct_operating_fee(id, 1_000_000_000_000u128, FeeType::IpfsPin),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn h4_deduct_fee_fails_closed_entity() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_ok!(EntityRegistry::approve_close_entity(RuntimeOrigin::root(), id));
        // Closed entity — deduct should fail
        assert_noop!(
            EntityRegistry::deduct_operating_fee(id, 1_000_000_000_000u128, FeeType::Other),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn h4_deduct_fee_fails_nonexistent_entity() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityRegistry::deduct_operating_fee(999, 100, FeeType::Other),
            Error::<Test>::EntityNotFound
        );
    });
}

// ==================== H5: register_shop distinct errors ====================

#[test]
fn h5_register_shop_capacity_full() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // MaxShopsPerEntity = 16, fill all slots
        for shop_id in 1..=16u64 {
            assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, shop_id));
        }
        // 17th should fail with ShopLimitReached
        assert_noop!(
            <EntityRegistry as EntityProvider<u64>>::register_shop(id, 17),
            Error::<Test>::ShopLimitReached
        );
    });
}

// ==================== reopen + approve lifecycle ====================

#[test]
fn h3_reopen_after_ban_then_governance_suspend_cleared() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Suspend via governance → ban → GovernanceSuspended should be cleared
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id));
        assert!(GovernanceSuspended::<Test>::get(id));
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false));
        assert!(!GovernanceSuspended::<Test>::get(id));
    });
}

// ==================== Round 4 审计回归测试 ====================

// H2: ban_entity 不没收时退还失败不阻止封禁
#[test]
fn h2_ban_entity_refund_failure_does_not_block_ban() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Entity 存在且 Active，ban without confiscate 应始终成功
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Banned);
    });
}

// H3: approve_close_entity 退还失败不阻止关闭
#[test]
fn h3_approve_close_refund_failure_does_not_block_close() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        // Close should always succeed even if refund has issues
        assert_ok!(EntityRegistry::approve_close_entity(RuntimeOrigin::root(), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Closed);
    });
}

// H4: EntityProvider::pause_entity / resume_entity 实现
#[test]
fn h4_pause_entity_trait_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::pause_entity(id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Suspended);
        assert!(GovernanceSuspended::<Test>::get(id));
        assert_eq!(EntityStats::<Test>::get().active_entities, 0);
    });
}

#[test]
fn h4_resume_entity_trait_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::pause_entity(id));
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::resume_entity(id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);
        assert!(!GovernanceSuspended::<Test>::get(id));
        assert_eq!(EntityStats::<Test>::get().active_entities, 1);
    });
}

#[test]
fn h4_pause_entity_fails_not_active() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::pause_entity(id));
        // Already suspended
        assert_noop!(
            <EntityRegistry as EntityProvider<u64>>::pause_entity(id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

#[test]
fn h4_resume_entity_fails_not_suspended() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Active, not suspended
        assert_noop!(
            <EntityRegistry as EntityProvider<u64>>::resume_entity(id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

// M2: upgrade_entity_type 拒绝同类型升级
#[test]
fn m2_upgrade_entity_type_rejects_same_type() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Merchant → Merchant with same governance = SameEntityType
        assert_noop!(
            EntityRegistry::upgrade_entity_type(
                RuntimeOrigin::signed(ALICE), id, EntityType::Merchant, GovernanceMode::None,
            ),
            Error::<Test>::SameEntityType
        );
    });
}

#[test]
fn m2_upgrade_entity_type_allows_governance_change_only() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Merchant → Merchant but with different governance = OK (type same, governance changed)
        assert_ok!(EntityRegistry::upgrade_entity_type(
            RuntimeOrigin::signed(ALICE), id, EntityType::Merchant, GovernanceMode::FullDAO,
        ));
        assert_eq!(Entities::<Test>::get(id).unwrap().governance_mode, GovernanceMode::FullDAO);
    });
}

// M4: deduct_operating_fee 拒绝 PendingClose 状态
#[test]
fn m4_deduct_fee_fails_pending_close_entity() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::PendingClose);
        // PendingClose — deduct should fail
        assert_noop!(
            EntityRegistry::deduct_operating_fee(id, 1_000_000_000_000u128, FeeType::IpfsPin),
            Error::<Test>::EntityNotActive
        );
    });
}

// L2: update_entity_stats 拒绝 Banned/Closed Entity
#[test]
fn l2_update_stats_fails_banned_entity() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false));
        assert_noop!(
            <EntityRegistry as EntityProvider<u64>>::update_entity_stats(id, 1000, 1),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn l2_update_stats_fails_closed_entity() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_ok!(EntityRegistry::approve_close_entity(RuntimeOrigin::root(), id));
        assert_noop!(
            <EntityRegistry as EntityProvider<u64>>::update_entity_stats(id, 1000, 1),
            Error::<Test>::EntityNotActive
        );
    });
}

// ==================== Entity 推荐人 ====================

#[test]
fn create_entity_with_referrer_works() {
    new_test_ext().execute_with(|| {
        // ALICE creates entity first (as referrer)
        let _alice_id = create_default_entity(ALICE);

        // BOB creates entity with ALICE as referrer
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(BOB),
            b"Bob Entity".to_vec(),
            None,
            None,
            Some(ALICE),
        ));
        let bob_id = EntityRegistry::next_entity_id() - 1;

        // Verify referrer stored
        assert_eq!(EntityReferrer::<Test>::get(bob_id), Some(ALICE));
        // Verify referral count
        assert_eq!(EntityReferralCount::<Test>::get(ALICE), 1);
    });
}

#[test]
fn create_entity_without_referrer_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_eq!(EntityReferrer::<Test>::get(id), None);
        assert_eq!(EntityReferralCount::<Test>::get(ALICE), 0);
    });
}

#[test]
fn create_entity_self_referral_fails() {
    new_test_ext().execute_with(|| {
        // ALICE needs an existing entity to be a valid referrer, but self-referral check is first
        let _alice_id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::create_entity(
                RuntimeOrigin::signed(ALICE),
                b"Second".to_vec(),
                None,
                None,
                Some(ALICE),
            ),
            Error::<Test>::SelfReferral
        );
    });
}

#[test]
fn create_entity_invalid_referrer_fails() {
    new_test_ext().execute_with(|| {
        // BOB has no entity — invalid referrer
        assert_noop!(
            EntityRegistry::create_entity(
                RuntimeOrigin::signed(ALICE),
                b"Test".to_vec(),
                None,
                None,
                Some(BOB),
            ),
            Error::<Test>::InvalidReferrer
        );
    });
}

#[test]
fn create_entity_referrer_must_have_active_entity() {
    new_test_ext().execute_with(|| {
        // BOB creates entity, then gets banned
        let bob_id = create_default_entity(BOB);
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), bob_id, false));

        // ALICE tries to create with BOB as referrer — BOB's entity is Banned
        assert_noop!(
            EntityRegistry::create_entity(
                RuntimeOrigin::signed(ALICE),
                b"Test".to_vec(),
                None,
                None,
                Some(BOB),
            ),
            Error::<Test>::InvalidReferrer
        );
    });
}

#[test]
fn bind_entity_referrer_works() {
    new_test_ext().execute_with(|| {
        // ALICE creates entity (no referrer)
        let alice_id = create_default_entity(ALICE);
        assert_eq!(EntityReferrer::<Test>::get(alice_id), None);

        // BOB creates entity (to be a valid referrer)
        let _bob_id = create_default_entity(BOB);

        // ALICE binds BOB as referrer
        assert_ok!(EntityRegistry::bind_entity_referrer(
            RuntimeOrigin::signed(ALICE),
            alice_id,
            BOB,
        ));
        assert_eq!(EntityReferrer::<Test>::get(alice_id), Some(BOB));
        assert_eq!(EntityReferralCount::<Test>::get(BOB), 1);
    });
}

#[test]
fn bind_entity_referrer_fails_already_bound() {
    new_test_ext().execute_with(|| {
        let _bob_id = create_default_entity(BOB);
        // ALICE creates with BOB as referrer
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(ALICE),
            b"Alice Entity".to_vec(),
            None,
            None,
            Some(BOB),
        ));
        let alice_id = EntityRegistry::next_entity_id() - 1;

        // CHARLIE creates entity (to be another potential referrer)
        let _charlie_id = create_default_entity(CHARLIE);

        // Try to re-bind — should fail
        assert_noop!(
            EntityRegistry::bind_entity_referrer(
                RuntimeOrigin::signed(ALICE),
                alice_id,
                CHARLIE,
            ),
            Error::<Test>::ReferrerAlreadyBound
        );
    });
}

#[test]
fn bind_entity_referrer_fails_not_owner() {
    new_test_ext().execute_with(|| {
        let alice_id = create_default_entity(ALICE);
        let _bob_id = create_default_entity(BOB);

        // BOB tries to bind referrer on ALICE's entity
        assert_noop!(
            EntityRegistry::bind_entity_referrer(
                RuntimeOrigin::signed(BOB),
                alice_id,
                BOB,
            ),
            Error::<Test>::NotEntityOwner
        );
    });
}

#[test]
fn bind_entity_referrer_fails_self_referral() {
    new_test_ext().execute_with(|| {
        let alice_id = create_default_entity(ALICE);

        assert_noop!(
            EntityRegistry::bind_entity_referrer(
                RuntimeOrigin::signed(ALICE),
                alice_id,
                ALICE,
            ),
            Error::<Test>::SelfReferral
        );
    });
}

#[test]
fn bind_entity_referrer_fails_invalid_referrer() {
    new_test_ext().execute_with(|| {
        let alice_id = create_default_entity(ALICE);
        // BOB has no entity
        assert_noop!(
            EntityRegistry::bind_entity_referrer(
                RuntimeOrigin::signed(ALICE),
                alice_id,
                BOB,
            ),
            Error::<Test>::InvalidReferrer
        );
    });
}

#[test]
fn bind_entity_referrer_fails_entity_not_active() {
    new_test_ext().execute_with(|| {
        let alice_id = create_default_entity(ALICE);
        let _bob_id = create_default_entity(BOB);

        // Suspend ALICE's entity
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), alice_id));

        assert_noop!(
            EntityRegistry::bind_entity_referrer(
                RuntimeOrigin::signed(ALICE),
                alice_id,
                BOB,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn referral_count_increments_correctly() {
    new_test_ext().execute_with(|| {
        // ALICE creates entity (referrer candidate)
        let _alice_id = create_default_entity(ALICE);

        // BOB creates entity with ALICE as referrer
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(BOB),
            b"Bob Entity".to_vec(),
            None,
            None,
            Some(ALICE),
        ));
        assert_eq!(EntityReferralCount::<Test>::get(ALICE), 1);

        // CHARLIE creates entity with ALICE as referrer
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(CHARLIE),
            b"Charlie Entity".to_vec(),
            None,
            None,
            Some(ALICE),
        ));
        assert_eq!(EntityReferralCount::<Test>::get(ALICE), 2);

        // DAVE creates entity without referrer, then binds ALICE
        let dave_id = create_default_entity(DAVE);
        assert_ok!(EntityRegistry::bind_entity_referrer(
            RuntimeOrigin::signed(DAVE),
            dave_id,
            ALICE,
        ));
        assert_eq!(EntityReferralCount::<Test>::get(ALICE), 3);
    });
}

// ==================== H2: 名称内容校验 ====================

#[test]
fn h2_create_entity_fails_whitespace_only_name() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityRegistry::create_entity(
                RuntimeOrigin::signed(ALICE),
                b"   ".to_vec(),
                None, None, None,
            ),
            Error::<Test>::NameEmpty
        );
    });
}

#[test]
fn h2_create_entity_fails_control_chars() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityRegistry::create_entity(
                RuntimeOrigin::signed(ALICE),
                b"\x00hello".to_vec(),
                None, None, None,
            ),
            Error::<Test>::InvalidName
        );
    });
}

#[test]
fn h2_create_entity_fails_non_utf8() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityRegistry::create_entity(
                RuntimeOrigin::signed(ALICE),
                vec![0xFF, 0xFE, 0x41],
                None, None, None,
            ),
            Error::<Test>::InvalidName
        );
    });
}

#[test]
fn h2_create_entity_accepts_chinese_name() {
    new_test_ext().execute_with(|| {
        let id = EntityRegistry::create_entity(
            RuntimeOrigin::signed(ALICE),
            "张三的店铺".as_bytes().to_vec(),
            None, None, None,
        );
        assert_ok!(id);
    });
}

#[test]
fn h2_update_entity_fails_control_chars() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::update_entity(
                RuntimeOrigin::signed(ALICE), id,
                Some(b"bad\x07name".to_vec()), None, None, None,
            ),
            Error::<Test>::InvalidName
        );
    });
}

// ==================== H1: 空 CID 转为 None ====================

#[test]
fn h1_empty_cid_becomes_none() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(ALICE),
            b"Test".to_vec(),
            Some(vec![]),       // 空 logo_cid
            Some(vec![]),       // 空 description_cid
            None,
        ));
        let id = EntityRegistry::next_entity_id() - 1;
        let entity = Entities::<Test>::get(id).unwrap();
        // 空 CID 应被转为 None，不浪费存储
        assert!(entity.logo_cid.is_none());
        assert!(entity.description_cid.is_none());
    });
}

// ==================== Round 5 审计回归测试 ====================

#[test]
fn h1_create_entity_rejects_at_max_entity_id() {
    new_test_ext().execute_with(|| {
        // 将 NextEntityId 设为 u64::MAX，模拟溢出边界
        NextEntityId::<Test>::put(u64::MAX);
        assert_noop!(
            EntityRegistry::create_entity(
                RuntimeOrigin::signed(ALICE),
                b"Overflow".to_vec(),
                None,
                None,
                None,
            ),
            Error::<Test>::ArithmeticOverflow
        );
    });
}

#[test]
fn h2_approve_entity_clears_governance_suspended() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);

        // 治理暂停 → 申请关闭 → 审批关闭 → 重开 → 审批通过
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id));
        assert!(GovernanceSuspended::<Test>::get(id));

        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_ok!(EntityRegistry::approve_close_entity(RuntimeOrigin::root(), id));
        // L2: approve_close_entity 也应清理
        assert!(!GovernanceSuspended::<Test>::get(id));

        assert_ok!(EntityRegistry::reopen_entity(RuntimeOrigin::signed(ALICE), id));
        // reopen 不清理（Pending 状态，治理标记无语义），但 approve_close 已清理
        assert!(!GovernanceSuspended::<Test>::get(id));

        assert_ok!(EntityRegistry::approve_entity(RuntimeOrigin::root(), id));
        // H2: approve_entity 也主动清理，确保新生命周期干净
        assert!(!GovernanceSuspended::<Test>::get(id));

        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.status, EntityStatus::Active);
    });
}

#[test]
fn h2_stale_governance_flag_no_longer_blocks_auto_resume() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);

        // 模拟前世治理暂停 → 关闭 → 重开 → 审批
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id));
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_ok!(EntityRegistry::approve_close_entity(RuntimeOrigin::root(), id));
        assert_ok!(EntityRegistry::reopen_entity(RuntimeOrigin::signed(ALICE), id));
        assert_ok!(EntityRegistry::approve_entity(RuntimeOrigin::root(), id));

        // 新生命周期：资金耗尽 → auto-suspend → top_up 应自动恢复
        // 先耗尽资金至触发暂停
        let treasury = EntityRegistry::entity_treasury_account(id);
        let balance = Balances::free_balance(&treasury);
        // 扣除到低于 MinOperatingBalance 触发 auto-suspend
        let deduct = balance.saturating_sub(50_000_000_000); // 留 0.05 token < MinOperatingBalance(0.1)
        if deduct > 0 {
            let _ = <<Test as crate::Config>::Currency as Currency<u64>>::transfer(
                &treasury,
                &99, // platform
                deduct,
                frame_support::traits::ExistenceRequirement::AllowDeath,
            );
        }

        // 检查实体仍为 Active（手动扣款不触发 auto-suspend）
        // 通过 deduct_operating_fee 触发 auto-suspend
        let small_fee = 40_000_000_000; // 0.04 token
        let _ = EntityRegistry::deduct_operating_fee(id, small_fee, crate::FeeType::TransactionFee);
        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.status, EntityStatus::Suspended);

        // top_up 应自动恢复（GovernanceSuspended 已在 approve_entity 中清理）
        assert_ok!(EntityRegistry::top_up_fund(
            RuntimeOrigin::signed(ALICE),
            id,
            10_000_000_000_000, // 10 tokens
        ));
        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.status, EntityStatus::Active);
    });
}

#[test]
fn m1_update_entity_empty_metadata_uri_becomes_none() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);

        // 先设置非空 metadata_uri
        assert_ok!(EntityRegistry::update_entity(
            RuntimeOrigin::signed(ALICE),
            id,
            None,
            None,
            None,
            Some(b"ipfs://Qm123".to_vec()),
        ));
        let entity = Entities::<Test>::get(id).unwrap();
        assert!(entity.metadata_uri.is_some());

        // 空 vec 应清除为 None
        assert_ok!(EntityRegistry::update_entity(
            RuntimeOrigin::signed(ALICE),
            id,
            None,
            None,
            None,
            Some(vec![]),
        ));
        let entity = Entities::<Test>::get(id).unwrap();
        assert!(entity.metadata_uri.is_none());
    });
}

#[test]
fn m2_set_governance_mode_rejects_banned_entity() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);

        // ban entity
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, true));
        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.status, EntityStatus::Banned);

        // set_governance_mode 应拒绝
        assert!(
            <EntityRegistry as EntityProvider<u64>>::set_governance_mode(id, GovernanceMode::FullDAO)
                .is_err()
        );
    });
}

#[test]
fn m2_set_governance_mode_rejects_closed_entity() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);

        // close entity
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_ok!(EntityRegistry::approve_close_entity(RuntimeOrigin::root(), id));
        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.status, EntityStatus::Closed);

        // set_governance_mode 应拒绝
        assert!(
            <EntityRegistry as EntityProvider<u64>>::set_governance_mode(id, GovernanceMode::FullDAO)
                .is_err()
        );
    });
}
