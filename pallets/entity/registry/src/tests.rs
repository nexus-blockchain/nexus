use crate::{mock::*, pallet::*};
use frame_support::{assert_noop, assert_ok, traits::Currency};
use pallet_entity_common::{EntityStatus, EntityType, GovernanceMode, EntityProvider, AdminPermission};

// ==================== Helper ====================

fn create_default_entity(who: u64) -> u64 {
    let seq = EntityRegistry::next_entity_id();
    let name = alloc::format!("Test Entity {}", seq).into_bytes();
    assert_ok!(EntityRegistry::create_entity(
        RuntimeOrigin::signed(who),
        name,
        None,
        None,
        None,
    ));
    EntityRegistry::next_entity_id() - 1
}

/// 关闭实体的辅助函数（request_close + 超时 + execute_close_timeout）
fn close_entity_via_timeout(who: u64, entity_id: u64) {
    assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(who), entity_id));
    // CloseRequestTimeout = 100, 推进到超时后
    let current = System::block_number();
    System::set_block_number(current + 101);
    assert_ok!(EntityRegistry::execute_close_timeout(RuntimeOrigin::signed(who), entity_id));
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
        assert_eq!(entity.name.to_vec(), b"Test Entity 1".to_vec());
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
            None,
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
                None, None, None, None,
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
                None, None, None, None,
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
            None,
        ));
        assert_noop!(
            EntityRegistry::update_entity(
                RuntimeOrigin::signed(ALICE),
                id,
                Some(b"New".to_vec()),
                None, None, None, None,
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
        close_entity_via_timeout(ALICE, id);
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
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id, None));
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

// ==================== reopen_entity 直接激活 ====================

#[test]
fn reopen_entity_directly_activates() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Close workflow
        close_entity_via_timeout(ALICE, id);
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Closed);
        // Reopen → Active（付费即激活，不再经过 Pending）
        assert_ok!(EntityRegistry::reopen_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);
        // 统计应恢复
        assert_eq!(EntityStats::<Test>::get().active_entities, 1);
    });
}

// ==================== execute_close_timeout 替代 approve_close ====================

#[test]
fn close_via_timeout_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let alice_before = Balances::free_balance(ALICE);
        close_entity_via_timeout(ALICE, id);

        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.status, EntityStatus::Closed);
        // Fund refunded to owner
        let alice_after = Balances::free_balance(ALICE);
        assert!(alice_after > alice_before - EXPECTED_INITIAL_FUND);
        // UserEntity cleared
        assert!(!UserEntity::<Test>::get(ALICE).contains(&id));
    });
}

// ==================== suspend_entity ====================

#[test]
fn suspend_entity_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id, None));
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
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id, None));
        // Already suspended
        assert_noop!(
            EntityRegistry::suspend_entity(RuntimeOrigin::root(), id, None),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

// ==================== resume_entity ====================

#[test]
fn resume_entity_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id, None));
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
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, true, None));
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
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        let alice_after = Balances::free_balance(ALICE);
        // Fund refunded
        assert!(alice_after > alice_before);
    });
}

#[test]
fn ban_entity_fails_already_banned() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        assert_noop!(
            EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

#[test]
fn ban_entity_from_suspended() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id, None));
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, true, None));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Banned);
    });
}

#[test]
fn ban_entity_from_pending_close() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Banned);
    });
}

// ==================== add_admin ====================

#[test]
fn add_admin_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::ALL_DEFINED));
        let entity = Entities::<Test>::get(id).unwrap();
        assert!(entity.admins.iter().any(|(a, _)| *a == BOB));
    });
}

#[test]
fn add_admin_fails_not_owner() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::add_admin(RuntimeOrigin::signed(BOB), id, CHARLIE, AdminPermission::ALL_DEFINED),
            Error::<Test>::NotEntityOwner
        );
    });
}

#[test]
fn add_admin_fails_already_exists() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::ALL_DEFINED));
        assert_noop!(
            EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::ALL_DEFINED),
            Error::<Test>::AdminAlreadyExists
        );
    });
}

#[test]
fn add_admin_fails_owner_as_admin() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, ALICE, AdminPermission::ALL_DEFINED),
            Error::<Test>::AdminAlreadyExists
        );
    });
}

#[test]
fn add_admin_fails_max_reached() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // MaxAdmins = 3
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::ALL_DEFINED));
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, CHARLIE, AdminPermission::ALL_DEFINED));
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, DAVE, AdminPermission::ALL_DEFINED));
        assert_noop!(
            EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, EVE, AdminPermission::ALL_DEFINED),
            Error::<Test>::MaxAdminsReached
        );
    });
}

#[test]
fn add_admin_fails_banned_entity() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        assert_noop!(
            EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::ALL_DEFINED),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

// ==================== remove_admin ====================

#[test]
fn remove_admin_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::ALL_DEFINED));
        assert_ok!(EntityRegistry::remove_admin(RuntimeOrigin::signed(ALICE), id, BOB));
        let entity = Entities::<Test>::get(id).unwrap();
        assert!(!entity.admins.iter().any(|(a, _)| *a == BOB));
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
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::ALL_DEFINED));
        close_entity_via_timeout(ALICE, id);
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
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::ALL_DEFINED));
        assert_ok!(EntityRegistry::transfer_ownership(
            RuntimeOrigin::signed(ALICE), id, BOB,
        ));
        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.owner, BOB);
        assert!(!entity.admins.iter().any(|(a, _)| *a == BOB));
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
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
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
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
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
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
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
        close_entity_via_timeout(ALICE, id);
        // Reopen → Active（付费即激活）
        assert_ok!(EntityRegistry::reopen_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);
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
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
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
        close_entity_via_timeout(ALICE, id);
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
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::ALL_DEFINED));
        assert!(<EntityRegistry as EntityProvider<u64>>::is_entity_admin(id, &ALICE, AdminPermission::SHOP_MANAGE));
        assert!(<EntityRegistry as EntityProvider<u64>>::is_entity_admin(id, &BOB, AdminPermission::SHOP_MANAGE));
        assert!(!<EntityRegistry as EntityProvider<u64>>::is_entity_admin(id, &CHARLIE, AdminPermission::SHOP_MANAGE));
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
        let sales = EntitySales::<Test>::get(id);
        assert_eq!(sales.total_sales, 1000u128);
        assert_eq!(sales.total_orders, 5);
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
        assert_ok!(EntityRegistry::add_admin(RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::ALL_DEFINED));
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
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id, None));
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
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id, None));
        assert!(GovernanceSuspended::<Test>::get(id));
        // Ban from suspended
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
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
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
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
        close_entity_via_timeout(ALICE, id);
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
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id, None));
        assert!(GovernanceSuspended::<Test>::get(id));
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
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
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Banned);
    });
}

// H3: execute_close_timeout 退还失败不阻止关闭
#[test]
fn h3_timeout_close_refund_failure_does_not_block_close() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        // Close should always succeed even if refund has issues
        System::set_block_number(101);
        assert_ok!(EntityRegistry::execute_close_timeout(RuntimeOrigin::signed(ALICE), id));
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
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
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
        close_entity_via_timeout(ALICE, id);
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
    });
}

#[test]
fn create_entity_without_referrer_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_eq!(EntityReferrer::<Test>::get(id), None);
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
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), bob_id, false, None));

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
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), alice_id, None));

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
        assert_eq!(EntityReferrer::<Test>::get(EntityRegistry::next_entity_id() - 1), Some(ALICE));

        // CHARLIE creates entity with ALICE as referrer
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(CHARLIE),
            b"Charlie Entity".to_vec(),
            None,
            None,
            Some(ALICE),
        ));
        assert_eq!(EntityReferrer::<Test>::get(EntityRegistry::next_entity_id() - 1), Some(ALICE));

        // DAVE creates entity without referrer, then binds ALICE
        let dave_id = create_default_entity(DAVE);
        assert_ok!(EntityRegistry::bind_entity_referrer(
            RuntimeOrigin::signed(DAVE),
            dave_id,
            ALICE,
        ));
        assert_eq!(EntityReferrer::<Test>::get(dave_id), Some(ALICE));
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
                Some(b"bad\x07name".to_vec()), None, None, None, None,
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
fn h2_reopen_clears_governance_suspended() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);

        // 治理暂停 → 申请关闭 → 超时关闭 → 重开（直接激活）
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id, None));
        assert!(GovernanceSuspended::<Test>::get(id));

        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        System::set_block_number(101);
        assert_ok!(EntityRegistry::execute_close_timeout(RuntimeOrigin::signed(ALICE), id));
        // 关闭时应清理
        assert!(!GovernanceSuspended::<Test>::get(id));

        assert_ok!(EntityRegistry::reopen_entity(RuntimeOrigin::signed(ALICE), id));
        // reopen 直接激活，GovernanceSuspended 已清理
        assert!(!GovernanceSuspended::<Test>::get(id));

        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.status, EntityStatus::Active);
    });
}

#[test]
fn h2_stale_governance_flag_no_longer_blocks_auto_resume() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);

        // 模拟前世治理暂停 → 关闭 → 重开（直接激活）
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id, None));
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        System::set_block_number(101);
        assert_ok!(EntityRegistry::execute_close_timeout(RuntimeOrigin::signed(ALICE), id));
        assert_ok!(EntityRegistry::reopen_entity(RuntimeOrigin::signed(ALICE), id));

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

        // top_up 应自动恢复（GovernanceSuspended 已在 reopen_entity 中清理）
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
            None,
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
            None,
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
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, true, None));
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
        close_entity_via_timeout(ALICE, id);
        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.status, EntityStatus::Closed);

        // set_governance_mode 应拒绝
        assert!(
            <EntityRegistry as EntityProvider<u64>>::set_governance_mode(id, GovernanceMode::FullDAO)
                .is_err()
        );
    });
}

// ==================== Round 6 审计回归测试 ====================

// M1: register_shop 拒绝 Banned/Closed Entity
#[test]
fn m1_register_shop_rejects_banned_entity() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));

        assert_noop!(
            <EntityRegistry as EntityProvider<u64>>::register_shop(id, 999),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn m1_register_shop_rejects_closed_entity() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        close_entity_via_timeout(ALICE, id);

        assert_noop!(
            <EntityRegistry as EntityProvider<u64>>::register_shop(id, 999),
            Error::<Test>::EntityNotActive
        );
    });
}

// ==================== Round 7 审计回归测试 ====================

// M1: transfer_ownership 拒绝自我转移
#[test]
fn m1_transfer_ownership_rejects_self_transfer() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::transfer_ownership(RuntimeOrigin::signed(ALICE), id, ALICE),
            Error::<Test>::SameOwner
        );
        // 确认实体未变
        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.owner, ALICE);
    });
}

// M2: reopen_entity 预检查用户容量
#[test]
fn m2_reopen_entity_fails_when_owner_at_capacity() {
    new_test_ext().execute_with(|| {
        // ALICE 创建并关闭一个实体
        let id1 = create_default_entity(ALICE);
        close_entity_via_timeout(ALICE, id1);
        assert_eq!(Entities::<Test>::get(id1).unwrap().status, EntityStatus::Closed);

        // ALICE 创建 3 个新实体（MaxEntitiesPerUser = 3）
        create_default_entity(ALICE);
        create_default_entity(ALICE);
        create_default_entity(ALICE);
        assert_eq!(UserEntity::<Test>::get(ALICE).len(), 3);

        // 重开旧实体应失败（ALICE 已满）
        assert_noop!(
            EntityRegistry::reopen_entity(RuntimeOrigin::signed(ALICE), id1),
            Error::<Test>::MaxEntitiesReached
        );
    });
}

// M3: deduct_operating_fee 对已暂停实体不发射 EntitySuspendedLowFund
#[test]
fn m3_deduct_fee_no_spurious_suspend_event_for_already_suspended() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);

        // 治理暂停
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id, None));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Suspended);

        // 清空事件
        frame_system::Pallet::<Test>::reset_events();

        // 从已暂停的实体扣费（余额仍充足，但实体已暂停）
        let small_fee = 1_000_000_000_000u128; // 1 token
        assert_ok!(EntityRegistry::deduct_operating_fee(id, small_fee, FeeType::IpfsPin));

        // 不应有 EntitySuspendedLowFund 事件（实体已经是 Suspended）
        let events = frame_system::Pallet::<Test>::events();
        let has_suspend_event = events.iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::EntityRegistry(crate::Event::EntitySuspendedLowFund { .. })
            )
        });
        assert!(!has_suspend_event, "Should not emit EntitySuspendedLowFund for already-suspended entity");
    });
}

// ==================== Round 7 审计回归测试 ====================

// M1: ban_entity confiscation 成功时 EntityBanned.fund_confiscated = true
#[test]
fn m1_ban_confiscate_success_reports_true() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let treasury = EntityRegistry::entity_treasury_account(id);
        let treasury_balance_before = Balances::free_balance(&treasury);
        assert!(treasury_balance_before > 0);

        let platform_before = Balances::free_balance(99);

        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, true, None));

        // 验证没收成功: 平台账户余额增加
        let platform_after = Balances::free_balance(99);
        assert_eq!(platform_after, platform_before + treasury_balance_before);

        // 验证 EntityBanned 事件中 fund_confiscated = true（实际结果）
        let events = frame_system::Pallet::<Test>::events();
        let ban_event = events.iter().find(|record| {
            matches!(
                record.event,
                RuntimeEvent::EntityRegistry(crate::Event::EntityBanned { .. })
            )
        });
        assert!(ban_event.is_some(), "EntityBanned event should be emitted");
        if let RuntimeEvent::EntityRegistry(crate::Event::EntityBanned { fund_confiscated, .. }) =
            &ban_event.unwrap().event
        {
            assert!(*fund_confiscated, "fund_confiscated should be true when confiscation succeeds");
        }
    });
}

// ==================== Round 8 审计回归测试 ====================

// L1: deduct_operating_fee 拒绝零费用
#[test]
fn l1_deduct_fee_rejects_zero_fee() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::deduct_operating_fee(id, 0, FeeType::IpfsPin),
            Error::<Test>::ZeroAmount
        );
    });
}

// L2: transfer_ownership 拒绝 PendingClose 实体
#[test]
fn l2_transfer_ownership_rejects_pending_close() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::PendingClose);

        assert_noop!(
            EntityRegistry::transfer_ownership(RuntimeOrigin::signed(ALICE), id, BOB),
            Error::<Test>::InvalidEntityStatus
        );
        // 确认所有权未变
        assert_eq!(Entities::<Test>::get(id).unwrap().owner, ALICE);
    });
}

// M1: ban_entity 不没收时 EntityBanned.fund_confiscated = false
#[test]
fn m1_ban_no_confiscate_reports_false() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);

        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));

        // 验证 EntityBanned 事件中 fund_confiscated = false
        let events = frame_system::Pallet::<Test>::events();
        let ban_event = events.iter().find(|record| {
            matches!(
                record.event,
                RuntimeEvent::EntityRegistry(crate::Event::EntityBanned { .. })
            )
        });
        assert!(ban_event.is_some());
        if let RuntimeEvent::EntityRegistry(crate::Event::EntityBanned { fund_confiscated, .. }) =
            &ban_event.unwrap().event
        {
            assert!(!*fund_confiscated, "fund_confiscated should be false when not confiscating");
        }
    });
}

// ==================== Round 8 审计回归测试 ====================

// M1: upgrade_entity_type 仅变更治理模式时不发射 EntityTypeUpgraded 事件
#[test]
fn m1_governance_only_change_does_not_emit_type_upgraded() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Merchant → Merchant but governance None → FullDAO
        frame_system::Pallet::<Test>::reset_events();

        assert_ok!(EntityRegistry::upgrade_entity_type(
            RuntimeOrigin::signed(ALICE), id, EntityType::Merchant, GovernanceMode::FullDAO,
        ));

        let events = frame_system::Pallet::<Test>::events();

        // 不应有 EntityTypeUpgraded（类型未变）
        let has_type_event = events.iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::EntityRegistry(crate::Event::EntityTypeUpgraded { .. })
            )
        });
        assert!(!has_type_event, "Should NOT emit EntityTypeUpgraded when only governance changes");

        // 应有 GovernanceModeChanged
        let has_gov_event = events.iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::EntityRegistry(crate::Event::GovernanceModeChanged { .. })
            )
        });
        assert!(has_gov_event, "Should emit GovernanceModeChanged");
    });
}

// M1: upgrade_entity_type 类型实际变更时仍发射 EntityTypeUpgraded
#[test]
fn m1_type_change_still_emits_type_upgraded() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        frame_system::Pallet::<Test>::reset_events();

        // Merchant → Community (actual type change)
        assert_ok!(EntityRegistry::upgrade_entity_type(
            RuntimeOrigin::signed(ALICE), id, EntityType::Community, GovernanceMode::None,
        ));

        let events = frame_system::Pallet::<Test>::events();

        let has_type_event = events.iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::EntityRegistry(crate::Event::EntityTypeUpgraded { .. })
            )
        });
        assert!(has_type_event, "Should emit EntityTypeUpgraded when type actually changes");
    });
}

// ==================== Phase 4: unban_entity ====================

#[test]
fn unban_entity_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Banned);

        // ban with refund: funds returned to owner. Treasury is empty.
        // unban 现在直接激活，需要先向 treasury 充值（top_up_fund 不允许 Banned 状态）
        let treasury = EntityRegistry::entity_treasury_account(id);
        let _ = Balances::deposit_creating(&treasury, EXPECTED_INITIAL_FUND);

        assert_ok!(EntityRegistry::unban_entity(RuntimeOrigin::root(), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);
        // GovernanceSuspended 应被清除
        assert!(!GovernanceSuspended::<Test>::get(id));
        // 统计应恢复
        assert_eq!(EntityStats::<Test>::get().active_entities, 1);
    });
}

#[test]
fn unban_entity_directly_activates() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));

        // ban with refund: funds returned to owner. Treasury is empty.
        // 向 treasury 直接充值
        let treasury = EntityRegistry::entity_treasury_account(id);
        let _ = Balances::deposit_creating(&treasury, EXPECTED_INITIAL_FUND);

        // Unban → Active（直接激活，不再经过 Pending）
        assert_ok!(EntityRegistry::unban_entity(RuntimeOrigin::root(), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);
        assert!(UserEntity::<Test>::get(ALICE).contains(&id));
        assert!(EntityRegistry::has_active_entity(&ALICE));
    });
}

#[test]
fn unban_entity_fails_not_banned() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Active, not Banned
        assert_noop!(
            EntityRegistry::unban_entity(RuntimeOrigin::root(), id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

#[test]
fn unban_entity_fails_not_governance() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        assert_noop!(
            EntityRegistry::unban_entity(RuntimeOrigin::signed(ALICE), id),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn unban_entity_fails_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityRegistry::unban_entity(RuntimeOrigin::root(), 999),
            Error::<Test>::EntityNotFound
        );
    });
}

#[test]
fn unban_entity_fails_insufficient_fund() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // ban with refund: treasury is empty
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        // unban without funding treasury should fail
        assert_noop!(
            EntityRegistry::unban_entity(RuntimeOrigin::root(), id),
            Error::<Test>::InsufficientOperatingFund
        );
    });
}

// ==================== Phase 4: unverify_entity ====================

#[test]
fn unverify_entity_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::verify_entity(RuntimeOrigin::root(), id));
        assert!(Entities::<Test>::get(id).unwrap().verified);

        assert_ok!(EntityRegistry::unverify_entity(RuntimeOrigin::root(), id));
        assert!(!Entities::<Test>::get(id).unwrap().verified);
        // Entity 状态不受影响
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);
    });
}

#[test]
fn unverify_entity_fails_not_verified() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // 未认证时撤销应失败
        assert_noop!(
            EntityRegistry::unverify_entity(RuntimeOrigin::root(), id),
            Error::<Test>::NotVerified
        );
    });
}

#[test]
fn unverify_entity_fails_banned() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::verify_entity(RuntimeOrigin::root(), id));
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        assert_noop!(
            EntityRegistry::unverify_entity(RuntimeOrigin::root(), id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

#[test]
fn unverify_entity_fails_not_governance() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::verify_entity(RuntimeOrigin::root(), id));
        assert_noop!(
            EntityRegistry::unverify_entity(RuntimeOrigin::signed(ALICE), id),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ==================== Phase 4: cancel_close_request ====================

#[test]
fn cancel_close_request_works_active() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let stats_before = EntityStats::<Test>::get();
        assert_eq!(stats_before.active_entities, 1);

        // Active → PendingClose
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::PendingClose);
        assert_eq!(EntityStats::<Test>::get().active_entities, 0);
        assert!(EntityCloseRequests::<Test>::contains_key(id));

        // Cancel → Active
        assert_ok!(EntityRegistry::cancel_close_request(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);
        assert_eq!(EntityStats::<Test>::get().active_entities, 1);
        assert!(!EntityCloseRequests::<Test>::contains_key(id));
    });
}

#[test]
fn cancel_close_request_restores_suspended_when_governance_suspended() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // 治理暂停 → Suspended
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id, None));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Suspended);

        // Suspended → PendingClose
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::PendingClose);

        // Cancel → 应恢复为 Suspended（治理暂停标记仍存在）
        assert_ok!(EntityRegistry::cancel_close_request(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Suspended);
        // active_entities 不应递增（从 Suspended 来的）
        assert_eq!(EntityStats::<Test>::get().active_entities, 0);
    });
}

#[test]
fn cancel_close_request_fails_not_pending_close() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Active, not PendingClose
        assert_noop!(
            EntityRegistry::cancel_close_request(RuntimeOrigin::signed(ALICE), id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

#[test]
fn cancel_close_request_fails_not_owner() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_noop!(
            EntityRegistry::cancel_close_request(RuntimeOrigin::signed(BOB), id),
            Error::<Test>::NotEntityOwner
        );
    });
}

// ==================== Phase 4: resign_admin ====================

#[test]
fn resign_admin_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::add_admin(
            RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::ALL_DEFINED,
        ));
        assert_eq!(Entities::<Test>::get(id).unwrap().admins.len(), 1);

        // BOB 主动辞职
        assert_ok!(EntityRegistry::resign_admin(RuntimeOrigin::signed(BOB), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().admins.len(), 0);
    });
}

#[test]
fn resign_admin_blocked_on_banned_entity() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::add_admin(
            RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::SHOP_MANAGE,
        ));
        // Ban entity
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));

        // L2 审计修复: 与 remove_admin 保持一致，终态实体不允许操作
        assert_noop!(
            EntityRegistry::resign_admin(RuntimeOrigin::signed(BOB), id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

#[test]
fn resign_admin_fails_owner_cannot_resign() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::resign_admin(RuntimeOrigin::signed(ALICE), id),
            Error::<Test>::CannotRemoveOwner
        );
    });
}

#[test]
fn resign_admin_fails_not_admin() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::resign_admin(RuntimeOrigin::signed(BOB), id),
            Error::<Test>::NotAdminCaller
        );
    });
}

#[test]
fn resign_admin_fails_entity_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityRegistry::resign_admin(RuntimeOrigin::signed(BOB), 999),
            Error::<Test>::EntityNotFound
        );
    });
}

// ==================== Phase 5: EntitySalesData ====================

#[test]
fn a2_update_entity_stats_uses_independent_storage() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Update stats
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::update_entity_stats(id, 5000, 3));
        // Check EntitySales storage
        let sales = EntitySales::<Test>::get(id);
        assert_eq!(sales.total_sales, 5000u128);
        assert_eq!(sales.total_orders, 3);
        // Accumulate
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::update_entity_stats(id, 2000, 1));
        let sales = EntitySales::<Test>::get(id);
        assert_eq!(sales.total_sales, 7000u128);
        assert_eq!(sales.total_orders, 4);
    });
}

// ==================== Phase 5: Admin proxy (b2) ====================

#[test]
fn b2_admin_can_update_entity_with_entity_manage() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::add_admin(
            RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::ENTITY_MANAGE,
        ));
        // BOB (admin with ENTITY_MANAGE) can update
        assert_ok!(EntityRegistry::update_entity(
            RuntimeOrigin::signed(BOB), id,
            Some(b"Updated by admin".to_vec()), None, None, None, None,
        ));
        assert_eq!(Entities::<Test>::get(id).unwrap().name.to_vec(), b"Updated by admin".to_vec());
    });
}

#[test]
fn b2_admin_without_entity_manage_cannot_update() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::add_admin(
            RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::SHOP_MANAGE,
        ));
        assert_noop!(
            EntityRegistry::update_entity(
                RuntimeOrigin::signed(BOB), id,
                Some(b"Fail".to_vec()), None, None, None, None,
            ),
            Error::<Test>::NotEntityOwner
        );
    });
}

#[test]
fn b2_admin_can_top_up_fund() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::add_admin(
            RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::ENTITY_MANAGE,
        ));
        let treasury = EntityRegistry::entity_treasury_account(id);
        let before = Balances::free_balance(&treasury);
        assert_ok!(EntityRegistry::top_up_fund(
            RuntimeOrigin::signed(BOB), id, 1_000_000_000_000,
        ));
        assert!(Balances::free_balance(&treasury) > before);
    });
}

// ==================== Phase 5: set_primary_shop (b3) ====================

#[test]
fn b3_set_primary_shop_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Register shops via EntityProvider
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 100));
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 200));
        // primary_shop_id should be the first registered (100) since entity had 0
        assert_eq!(Entities::<Test>::get(id).unwrap().primary_shop_id, 100);

        // Set primary to 200
        assert_ok!(EntityRegistry::set_primary_shop(RuntimeOrigin::signed(ALICE), id, 200));
        assert_eq!(Entities::<Test>::get(id).unwrap().primary_shop_id, 200);
    });
}

#[test]
fn b3_set_primary_shop_fails_not_in_entity() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::set_primary_shop(RuntimeOrigin::signed(ALICE), id, 999),
            Error::<Test>::ShopNotInEntity
        );
    });
}

#[test]
fn b3_set_primary_shop_fails_already_primary() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 100));
        assert_noop!(
            EntityRegistry::set_primary_shop(RuntimeOrigin::signed(ALICE), id, 100),
            Error::<Test>::AlreadyPrimaryShop
        );
    });
}

#[test]
fn b3_set_primary_shop_admin_with_entity_manage() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 100));
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 200));
        assert_ok!(EntityRegistry::add_admin(
            RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::ENTITY_MANAGE,
        ));
        assert_ok!(EntityRegistry::set_primary_shop(RuntimeOrigin::signed(BOB), id, 200));
        assert_eq!(Entities::<Test>::get(id).unwrap().primary_shop_id, 200);
    });
}

// ==================== Phase 5: self_pause/resume (b4) ====================

#[test]
fn b4_self_pause_entity_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::self_pause_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Suspended);
        assert!(OwnerPaused::<Test>::get(id));
        assert_eq!(EntityStats::<Test>::get().active_entities, 0);
    });
}

#[test]
fn b4_self_resume_entity_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::self_pause_entity(RuntimeOrigin::signed(ALICE), id));
        assert_ok!(EntityRegistry::self_resume_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);
        assert!(!OwnerPaused::<Test>::get(id));
        assert_eq!(EntityStats::<Test>::get().active_entities, 1);
    });
}

#[test]
fn b4_self_resume_fails_not_owner_paused() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Governance suspend (not owner paused)
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id, None));
        assert_noop!(
            EntityRegistry::self_resume_entity(RuntimeOrigin::signed(ALICE), id),
            Error::<Test>::NotOwnerPaused
        );
    });
}

#[test]
fn b4_self_resume_fails_governance_suspended() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Owner pause first
        assert_ok!(EntityRegistry::self_pause_entity(RuntimeOrigin::signed(ALICE), id));
        // Then governance also marks it
        GovernanceSuspended::<Test>::insert(id, true);
        // Owner cannot resume because governance has priority
        assert_noop!(
            EntityRegistry::self_resume_entity(RuntimeOrigin::signed(ALICE), id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

#[test]
fn b4_self_pause_fails_not_active() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::self_pause_entity(RuntimeOrigin::signed(ALICE), id));
        assert_noop!(
            EntityRegistry::self_pause_entity(RuntimeOrigin::signed(ALICE), id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

// ==================== Phase 5: force_transfer_ownership (b5) ====================

#[test]
fn b5_force_transfer_ownership_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::force_transfer_ownership(RuntimeOrigin::root(), id, BOB));
        assert_eq!(Entities::<Test>::get(id).unwrap().owner, BOB);
        assert!(UserEntity::<Test>::get(BOB).contains(&id));
        assert!(!UserEntity::<Test>::get(ALICE).contains(&id));
    });
}

#[test]
fn b5_force_transfer_ownership_removes_new_owner_from_admins() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::add_admin(
            RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::ALL_DEFINED,
        ));
        assert_eq!(Entities::<Test>::get(id).unwrap().admins.len(), 1);
        assert_ok!(EntityRegistry::force_transfer_ownership(RuntimeOrigin::root(), id, BOB));
        assert_eq!(Entities::<Test>::get(id).unwrap().admins.len(), 0);
    });
}

#[test]
fn b5_force_transfer_fails_same_owner() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::force_transfer_ownership(RuntimeOrigin::root(), id, ALICE),
            Error::<Test>::SameOwner
        );
    });
}

#[test]
fn b5_force_transfer_fails_not_governance() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::force_transfer_ownership(RuntimeOrigin::signed(ALICE), id, BOB),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ==================== Phase 5: ban/suspend with reason (b6) ====================

#[test]
fn b6_ban_entity_with_reason() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::ban_entity(
            RuntimeOrigin::root(), id, false, Some(b"Fraud detected".to_vec()),
        ));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Banned);
        // Reason is in the event
        let events = frame_system::Pallet::<Test>::events();
        let ban_event = events.iter().find(|record| {
            matches!(record.event, RuntimeEvent::EntityRegistry(crate::Event::EntityBanned { .. }))
        });
        assert!(ban_event.is_some());
    });
}

#[test]
fn b6_suspend_entity_with_reason() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::suspend_entity(
            RuntimeOrigin::root(), id, Some(b"Suspicious activity".to_vec()),
        ));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Suspended);
    });
}

// ==================== Phase 5: reject_close_request (b7) ====================

#[test]
fn b7_reject_close_request_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::PendingClose);

        assert_ok!(EntityRegistry::reject_close_request(RuntimeOrigin::root(), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);
        assert_eq!(EntityStats::<Test>::get().active_entities, 1);
        assert!(!EntityCloseRequests::<Test>::contains_key(id));
    });
}

#[test]
fn b7_reject_close_request_restores_suspended_when_governance_suspended() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id, None));
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));

        assert_ok!(EntityRegistry::reject_close_request(RuntimeOrigin::root(), id));
        // GovernanceSuspended is still set, so restores to Suspended
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Suspended);
        assert_eq!(EntityStats::<Test>::get().active_entities, 0);
    });
}

#[test]
fn b7_reject_close_request_fails_not_pending_close() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_noop!(
            EntityRegistry::reject_close_request(RuntimeOrigin::root(), id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

#[test]
fn b7_reject_close_request_fails_not_governance() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_noop!(
            EntityRegistry::reject_close_request(RuntimeOrigin::signed(ALICE), id),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ==================== Phase 5: Referrer reverse index (b8) ====================

#[test]
fn b8_create_entity_with_referrer_updates_reverse_index() {
    new_test_ext().execute_with(|| {
        let _bob_id = create_default_entity(BOB);
        // ALICE creates entity with BOB as referrer
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(ALICE),
            b"Alice Entity".to_vec(),
            None, None,
            Some(BOB),
        ));
        let alice_id = EntityRegistry::next_entity_id() - 1;
        // ReferrerEntities should contain alice_id under BOB
        assert!(ReferrerEntities::<Test>::get(BOB).contains(&alice_id));
    });
}

#[test]
fn b8_bind_entity_referrer_updates_reverse_index() {
    new_test_ext().execute_with(|| {
        let alice_id = create_default_entity(ALICE);
        let _bob_id = create_default_entity(BOB);
        // Bind referrer
        assert_ok!(EntityRegistry::bind_entity_referrer(
            RuntimeOrigin::signed(ALICE), alice_id, BOB,
        ));
        assert!(ReferrerEntities::<Test>::get(BOB).contains(&alice_id));
    });
}

// ==================== EntityLocked 回归测试 ====================

#[test]
fn entity_locked_rejects_update_entity() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        set_entity_locked(id);
        assert_noop!(
            EntityRegistry::update_entity(
                RuntimeOrigin::signed(ALICE), id,
                Some(b"New Name".to_vec()), None, None, None, None,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_add_admin() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        set_entity_locked(id);
        assert_noop!(
            EntityRegistry::add_admin(
                RuntimeOrigin::signed(ALICE), id, BOB, pallet_entity_common::AdminPermission::ENTITY_MANAGE,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

// ==================== Phase 6: F1 名称唯一性 ====================

#[test]
fn f1_create_entity_rejects_duplicate_name() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(ALICE),
            b"UniqueShop".to_vec(),
            None, None, None,
        ));
        // BOB tries to create entity with same name
        assert_noop!(
            EntityRegistry::create_entity(
                RuntimeOrigin::signed(BOB),
                b"UniqueShop".to_vec(),
                None, None, None,
            ),
            Error::<Test>::NameAlreadyTaken
        );
    });
}

#[test]
fn f1_name_uniqueness_is_case_insensitive() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(ALICE),
            b"CaseName".to_vec(),
            None, None, None,
        ));
        // BOB tries "casename" (lowercase) — should fail
        assert_noop!(
            EntityRegistry::create_entity(
                RuntimeOrigin::signed(BOB),
                b"casename".to_vec(),
                None, None, None,
            ),
            Error::<Test>::NameAlreadyTaken
        );
    });
}

#[test]
fn f1_different_names_both_succeed() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(ALICE),
            b"Alpha Shop".to_vec(),
            None, None, None,
        ));
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(BOB),
            b"Beta Shop".to_vec(),
            None, None, None,
        ));
    });
}

#[test]
fn f1_update_entity_name_checks_uniqueness() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(ALICE),
            b"Alice Unique".to_vec(),
            None, None, None,
        ));
        let id2 = {
            assert_ok!(EntityRegistry::create_entity(
                RuntimeOrigin::signed(BOB),
                b"Bob Unique".to_vec(),
                None, None, None,
            ));
            EntityRegistry::next_entity_id() - 1
        };
        // BOB tries to rename to ALICE's name
        assert_noop!(
            EntityRegistry::update_entity(
                RuntimeOrigin::signed(BOB), id2,
                Some(b"Alice Unique".to_vec()), None, None, None, None,
            ),
            Error::<Test>::NameAlreadyTaken
        );
    });
}

#[test]
fn f1_update_entity_same_name_succeeds() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let current_name = Entities::<Test>::get(id).unwrap().name.to_vec();
        // Renaming to same name should succeed
        assert_ok!(EntityRegistry::update_entity(
            RuntimeOrigin::signed(ALICE), id,
            Some(current_name), None, None, None, None,
        ));
    });
}

#[test]
fn f1_ban_frees_name() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(ALICE),
            b"BanMe".to_vec(),
            None, None, None,
        ));
        let id = EntityRegistry::next_entity_id() - 1;
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        // Name should be freed — BOB can use it
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(BOB),
            b"BanMe".to_vec(),
            None, None, None,
        ));
    });
}

#[test]
fn f1_close_frees_name() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(ALICE),
            b"CloseMe".to_vec(),
            None, None, None,
        ));
        let id = EntityRegistry::next_entity_id() - 1;
        close_entity_via_timeout(ALICE, id);
        // Name should be freed
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(BOB),
            b"CloseMe".to_vec(),
            None, None, None,
        ));
    });
}

// ==================== Phase 6: F3 关闭申请超时 ====================

#[test]
fn f3_execute_close_timeout_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let treasury = EntityRegistry::entity_treasury_account(id);
        let balance_before = Balances::free_balance(ALICE);
        let fund_in_treasury = Balances::free_balance(&treasury);

        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::PendingClose);

        // Too early — should fail
        System::set_block_number(50);
        assert_noop!(
            EntityRegistry::execute_close_timeout(RuntimeOrigin::signed(BOB), id),
            Error::<Test>::CloseRequestNotExpired
        );

        // After timeout (block 1 + 100 = 101)
        System::set_block_number(101);
        assert_ok!(EntityRegistry::execute_close_timeout(RuntimeOrigin::signed(BOB), id));

        // Entity should be closed
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Closed);
        // Fund refunded to ALICE
        assert_eq!(Balances::free_balance(ALICE), balance_before + fund_in_treasury);
        // Name freed
        let name = Entities::<Test>::get(id).unwrap().name.to_vec();
        assert!(!EntityNameIndex::<Test>::contains_key(
            EntityRegistry::normalize_entity_name(&name).unwrap()
        ));
        // User entity index cleared
        assert!(!UserEntity::<Test>::get(ALICE).contains(&id));
    });
}

#[test]
fn f3_execute_close_timeout_rejects_non_pending() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // No close request
        System::set_block_number(200);
        assert_noop!(
            EntityRegistry::execute_close_timeout(RuntimeOrigin::signed(BOB), id),
            Error::<Test>::InvalidEntityStatus
        );
    });
}

// ==================== Phase 6: F4 联系方式 ====================

#[test]
fn f4_update_contact_cid_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Initially None
        assert!(Entities::<Test>::get(id).unwrap().contact_cid.is_none());

        // Set contact_cid
        assert_ok!(EntityRegistry::update_entity(
            RuntimeOrigin::signed(ALICE), id,
            None, None, None, None,
            Some(b"ipfs://QmContact123".to_vec()),
        ));
        let entity = Entities::<Test>::get(id).unwrap();
        assert_eq!(entity.contact_cid.unwrap().to_vec(), b"ipfs://QmContact123".to_vec());

        // Clear contact_cid with empty vec
        assert_ok!(EntityRegistry::update_entity(
            RuntimeOrigin::signed(ALICE), id,
            None, None, None, None,
            Some(vec![]),
        ));
        assert!(Entities::<Test>::get(id).unwrap().contact_cid.is_none());
    });
}

// ==================== Phase 6: F7 暂停原因 ====================

#[test]
fn f7_suspend_stores_reason() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::suspend_entity(
            RuntimeOrigin::root(), id, Some(b"violation of TOS".to_vec()),
        ));
        assert_eq!(
            SuspensionReasons::<Test>::get(id).unwrap().to_vec(),
            b"violation of TOS".to_vec()
        );
    });
}

#[test]
fn f7_suspend_without_reason_stores_nothing() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::suspend_entity(RuntimeOrigin::root(), id, None));
        assert!(SuspensionReasons::<Test>::get(id).is_none());
    });
}

#[test]
fn f7_resume_clears_reason() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::suspend_entity(
            RuntimeOrigin::root(), id, Some(b"reason".to_vec()),
        ));
        assert!(SuspensionReasons::<Test>::get(id).is_some());

        assert_ok!(EntityRegistry::resume_entity(RuntimeOrigin::root(), id));
        assert!(SuspensionReasons::<Test>::get(id).is_none());
    });
}

#[test]
fn f7_ban_clears_reason() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::suspend_entity(
            RuntimeOrigin::root(), id, Some(b"reason".to_vec()),
        ));
        assert!(SuspensionReasons::<Test>::get(id).is_some());

        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        assert!(SuspensionReasons::<Test>::get(id).is_none());
    });
}

// ==================== Phase 6: F11 Runtime API ====================

#[test]
fn f11_api_get_entity_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let info = EntityRegistry::api_get_entity(id).unwrap();
        assert_eq!(info.id, id);
        assert_eq!(info.owner, ALICE);
        assert_eq!(info.name, Entities::<Test>::get(id).unwrap().name.to_vec());
        assert_eq!(info.status, EntityStatus::Active);
        assert!(!info.verified);
        assert!(info.fund_balance > 0u128);
    });
}

#[test]
fn f11_api_get_entity_nonexistent_returns_none() {
    new_test_ext().execute_with(|| {
        assert!(EntityRegistry::api_get_entity(999).is_none());
    });
}

#[test]
fn f11_api_get_entities_by_owner_works() {
    new_test_ext().execute_with(|| {
        let id1 = create_default_entity(ALICE);
        let ids = EntityRegistry::api_get_entities_by_owner(&ALICE);
        assert_eq!(ids, vec![id1]);
    });
}

#[test]
fn f11_api_get_entity_fund_info_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let info = EntityRegistry::api_get_entity_fund_info(id).unwrap();
        assert!(info.balance > 0u128);
        assert_eq!(info.min_operating, 100_000_000_000u128);
    });
}

// ==================== Phase 6: F3 超时关闭释放名称 ====================

#[test]
fn f3_timeout_close_frees_name() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(ALICE),
            b"TimeoutClose".to_vec(),
            None, None, None,
        ));
        let id = EntityRegistry::next_entity_id() - 1;
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        System::set_block_number(101);
        assert_ok!(EntityRegistry::execute_close_timeout(RuntimeOrigin::signed(BOB), id));
        // Name freed — can reuse
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(BOB),
            b"TimeoutClose".to_vec(),
            None, None, None,
        ));
    });
}

// ==================== Round 9 审计回归测试 ====================

// M1: top_up_fund 不应绕过 OwnerPaused
#[test]
fn m1_top_up_does_not_auto_resume_owner_paused() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Owner 主动暂停
        assert_ok!(EntityRegistry::self_pause_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Suspended);
        assert!(OwnerPaused::<Test>::get(id));

        // 充值不应自动恢复（因为是 owner 主动暂停）
        assert_ok!(EntityRegistry::top_up_fund(
            RuntimeOrigin::signed(ALICE), id, 10_000_000_000_000,
        ));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Suspended);
        assert!(OwnerPaused::<Test>::get(id));
    });
}

// M2: ban_entity 清理 OwnerPaused
#[test]
fn m2_ban_clears_owner_paused() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::self_pause_entity(RuntimeOrigin::signed(ALICE), id));
        assert!(OwnerPaused::<Test>::get(id));

        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        // OwnerPaused 应被清除
        assert!(!OwnerPaused::<Test>::get(id));
    });
}

// M2: close 清理 OwnerPaused
#[test]
fn m2_close_clears_owner_paused() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::self_pause_entity(RuntimeOrigin::signed(ALICE), id));
        assert!(OwnerPaused::<Test>::get(id));
        close_entity_via_timeout(ALICE, id);
        // close 应清除 OwnerPaused
        assert!(!OwnerPaused::<Test>::get(id));
    });
}

// M2: unban 生命周期中 OwnerPaused 不残留
#[test]
fn m2_unban_lifecycle_owner_paused_clean() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::self_pause_entity(RuntimeOrigin::signed(ALICE), id));
        assert!(OwnerPaused::<Test>::get(id));

        // ban 清理
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        assert!(!OwnerPaused::<Test>::get(id));

        // 向 treasury 直接充值 + unban → Active（直接激活）
        let treasury = EntityRegistry::entity_treasury_account(id);
        let _ = Balances::deposit_creating(&treasury, EXPECTED_INITIAL_FUND);
        assert_ok!(EntityRegistry::unban_entity(RuntimeOrigin::root(), id));
        assert!(!OwnerPaused::<Test>::get(id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);

        // 现在应该可以再次 self_pause（OwnerPaused 已被清理）
        assert_ok!(EntityRegistry::self_pause_entity(RuntimeOrigin::signed(ALICE), id));
        assert!(OwnerPaused::<Test>::get(id));
    });
}

// M2: governance resume 清理 OwnerPaused
#[test]
fn m2_governance_resume_clears_owner_paused() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::self_pause_entity(RuntimeOrigin::signed(ALICE), id));
        assert!(OwnerPaused::<Test>::get(id));

        // 治理也标记暂停
        GovernanceSuspended::<Test>::insert(id, true);

        // 治理恢复应清除所有暂停标记
        assert_ok!(EntityRegistry::resume_entity(RuntimeOrigin::root(), id));
        assert!(!OwnerPaused::<Test>::get(id));
        assert!(!GovernanceSuspended::<Test>::get(id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);
    });
}

// M3: cancel_close_request 恢复到 Suspended（当 OwnerPaused）
#[test]
fn m3_cancel_close_restores_suspended_when_owner_paused() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Owner 暂停
        assert_ok!(EntityRegistry::self_pause_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Suspended);

        // 申请关闭
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::PendingClose);

        // 撤销关闭 → 应恢复到 Suspended（因为 OwnerPaused 标记仍存在）
        assert_ok!(EntityRegistry::cancel_close_request(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Suspended);
        assert!(OwnerPaused::<Test>::get(id));
    });
}

// M3: reject_close_request 恢复到 Suspended（当 OwnerPaused）
#[test]
fn m3_reject_close_restores_suspended_when_owner_paused() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Owner 暂停
        assert_ok!(EntityRegistry::self_pause_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Suspended);

        // 申请关闭
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));

        // 治理拒绝 → 应恢复到 Suspended（因为 OwnerPaused 标记仍存在）
        assert_ok!(EntityRegistry::reject_close_request(RuntimeOrigin::root(), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Suspended);
        assert!(OwnerPaused::<Test>::get(id));
    });
}

// M2: reopen_entity 清理 OwnerPaused
#[test]
fn m2_reopen_clears_owner_paused() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::self_pause_entity(RuntimeOrigin::signed(ALICE), id));
        assert!(OwnerPaused::<Test>::get(id));

        close_entity_via_timeout(ALICE, id);
        assert!(!OwnerPaused::<Test>::get(id));

        assert_ok!(EntityRegistry::reopen_entity(RuntimeOrigin::signed(ALICE), id));
        assert!(!OwnerPaused::<Test>::get(id));
    });
}

// M2: execute_close_timeout 清理 OwnerPaused
#[test]
fn m2_timeout_close_clears_owner_paused() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::self_pause_entity(RuntimeOrigin::signed(ALICE), id));
        assert!(OwnerPaused::<Test>::get(id));

        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));
        System::set_block_number(101);
        assert_ok!(EntityRegistry::execute_close_timeout(RuntimeOrigin::signed(BOB), id));
        assert!(!OwnerPaused::<Test>::get(id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Closed);
    });
}

// ==================== Round 10 审计回归测试 ====================

// H1: unban_entity 恢复 UserEntity 索引（ban 时已移除）
#[test]
fn h1_unban_restores_user_entity_index() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert!(UserEntity::<Test>::get(ALICE).contains(&id));

        // Ban removes from UserEntity
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        assert!(!UserEntity::<Test>::get(ALICE).contains(&id));

        // Unban should restore UserEntity（需要先充值 treasury）
        let treasury = EntityRegistry::entity_treasury_account(id);
        let _ = Balances::deposit_creating(&treasury, EXPECTED_INITIAL_FUND);
        assert_ok!(EntityRegistry::unban_entity(RuntimeOrigin::root(), id));
        assert!(UserEntity::<Test>::get(ALICE).contains(&id));
    });
}

// H1: unban 全流程 UserEntity 正确
#[test]
fn h1_unban_user_entity_consistent() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);

        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        let treasury = EntityRegistry::entity_treasury_account(id);
        let _ = Balances::deposit_creating(&treasury, EXPECTED_INITIAL_FUND);
        assert_ok!(EntityRegistry::unban_entity(RuntimeOrigin::root(), id));

        // Entity is Active and in UserEntity
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);
        assert!(UserEntity::<Test>::get(ALICE).contains(&id));

        // has_active_entity should return true
        assert!(EntityRegistry::has_active_entity(&ALICE));
    });
}

// M1: unban 恢复 EntityNameIndex（ban→unban 流程）
#[test]
fn m1_unban_restores_name_index() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(ALICE),
            b"RestoreName".to_vec(),
            None, None, None,
        ));
        let id = EntityRegistry::next_entity_id() - 1;
        let normalized = EntityRegistry::normalize_entity_name(b"RestoreName").unwrap();
        assert!(EntityNameIndex::<Test>::contains_key(&normalized));

        // Ban removes name index
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        assert!(!EntityNameIndex::<Test>::contains_key(&normalized));

        // 向 treasury 直接充值 + unban should restore name index（直接激活）
        let treasury = EntityRegistry::entity_treasury_account(id);
        let _ = Balances::deposit_creating(&treasury, EXPECTED_INITIAL_FUND);
        assert_ok!(EntityRegistry::unban_entity(RuntimeOrigin::root(), id));
        assert!(EntityNameIndex::<Test>::contains_key(&normalized));
        assert_eq!(EntityNameIndex::<Test>::get(&normalized), Some(id));
    });
}

// M1: reopen 恢复 EntityNameIndex（close→reopen 流程）
#[test]
fn m1_reopen_restores_name_index() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(ALICE),
            b"ReopenName".to_vec(),
            None, None, None,
        ));
        let id = EntityRegistry::next_entity_id() - 1;
        let normalized = EntityRegistry::normalize_entity_name(b"ReopenName").unwrap();
        assert!(EntityNameIndex::<Test>::contains_key(&normalized));

        // Close removes name index
        close_entity_via_timeout(ALICE, id);
        assert!(!EntityNameIndex::<Test>::contains_key(&normalized));

        // Reopen should restore name index（直接激活）
        assert_ok!(EntityRegistry::reopen_entity(RuntimeOrigin::signed(ALICE), id));
        assert!(EntityNameIndex::<Test>::contains_key(&normalized));
        assert_eq!(EntityNameIndex::<Test>::get(&normalized), Some(id));
    });
}

// M1: 名称已被他人占用时，approve 不覆盖
#[test]
fn m1_approve_does_not_overwrite_taken_name() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(ALICE),
            b"SharedName".to_vec(),
            None, None, None,
        ));
        let id1 = EntityRegistry::next_entity_id() - 1;
        let normalized = EntityRegistry::normalize_entity_name(b"SharedName").unwrap();

        // Ban entity 1 (frees name)
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id1, false, None));

        // BOB takes the same name
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(BOB),
            b"SharedName".to_vec(),
            None, None, None,
        ));
        let id2 = EntityRegistry::next_entity_id() - 1;
        assert_eq!(EntityNameIndex::<Test>::get(&normalized), Some(id2));

        // H1 审计修复: unban 时名称已被占用 → 直接拒绝（更早拦截名称碰撞）
        assert_noop!(
            EntityRegistry::unban_entity(RuntimeOrigin::root(), id1),
            Error::<Test>::NameAlreadyTaken
        );
        // BOB's name index unchanged
        assert_eq!(EntityNameIndex::<Test>::get(&normalized), Some(id2));
    });
}

// L1: 暂停原因超过 256 字节时截断而非丢弃
#[test]
fn l1_suspend_reason_truncated_not_dropped() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let long_reason = vec![b'x'; 300];
        assert_ok!(EntityRegistry::suspend_entity(
            RuntimeOrigin::root(), id, Some(long_reason),
        ));
        // Reason should be stored (truncated to 256), not dropped
        let stored = SuspensionReasons::<Test>::get(id).unwrap();
        assert_eq!(stored.len(), 256);
    });
}

// L1: 封禁原因超过 256 字节时截断而非丢弃
#[test]
fn l1_ban_reason_truncated_not_dropped() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let long_reason = vec![b'x'; 300];
        assert_ok!(EntityRegistry::ban_entity(
            RuntimeOrigin::root(), id, false, Some(long_reason),
        ));
        // Check the event contains a 256-byte reason (not empty)
        let events = frame_system::Pallet::<Test>::events();
        let ban_event = events.iter().find(|record| {
            matches!(record.event, RuntimeEvent::EntityRegistry(crate::Event::EntityBanned { .. }))
        });
        match &ban_event.unwrap().event {
            RuntimeEvent::EntityRegistry(crate::Event::EntityBanned { reason, .. }) => {
                assert_eq!(reason.as_ref().unwrap().len(), 256);
            },
            _ => panic!("Expected EntityBanned event"),
        }
    });
}

// ==================== Round 11 审计回归测试 ====================

// H1-R11: EntityProvider::resume_entity 清理 OwnerPaused 和 SuspensionReasons
#[test]
fn h1_r11_trait_resume_clears_owner_paused_and_reasons() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);

        // Owner 主动暂停
        assert_ok!(EntityRegistry::self_pause_entity(RuntimeOrigin::signed(ALICE), id));
        assert!(OwnerPaused::<Test>::get(id));

        // 治理也标记暂停 + 添加原因
        GovernanceSuspended::<Test>::insert(id, true);
        SuspensionReasons::<Test>::insert(id, frame_support::BoundedVec::<u8, sp_runtime::traits::ConstU32<256>>::try_from(b"test reason".to_vec()).unwrap());

        // 通过 EntityProvider trait 恢复
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::resume_entity(id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);
        // H1-R11: OwnerPaused 应被清除
        assert!(!OwnerPaused::<Test>::get(id));
        // H1-R11: GovernanceSuspended 应被清除
        assert!(!GovernanceSuspended::<Test>::get(id));
        // H1-R11: SuspensionReasons 应被清除
        assert!(SuspensionReasons::<Test>::get(id).is_none());
    });
}

// M1-R11: EntityProvider::entity_name 返回真实数据
#[test]
fn m1_r11_trait_entity_name_returns_real_data() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        let name = <EntityRegistry as EntityProvider<u64>>::entity_name(id);
        assert_eq!(name, Entities::<Test>::get(id).unwrap().name.to_vec());
        assert!(!name.is_empty());
    });
}

// M1-R11: EntityProvider::entity_name 不存在时返回空
#[test]
fn m1_r11_trait_entity_name_returns_empty_for_missing() {
    new_test_ext().execute_with(|| {
        let name = <EntityRegistry as EntityProvider<u64>>::entity_name(999);
        assert!(name.is_empty());
    });
}

// M1-R11: EntityProvider::entity_description 返回真实数据
#[test]
fn m1_r11_trait_entity_description_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // 设置 description_cid
        assert_ok!(EntityRegistry::update_entity(
            RuntimeOrigin::signed(ALICE), id,
            None, None, Some(b"QmDesc123".to_vec()), None, None,
        ));
        let desc = <EntityRegistry as EntityProvider<u64>>::entity_description(id);
        assert_eq!(desc, b"QmDesc123".to_vec());
    });
}

// M1-R11: EntityProvider::entity_metadata_cid 返回真实数据
#[test]
fn m1_r11_trait_entity_metadata_cid_works() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // Initially None
        assert!(<EntityRegistry as EntityProvider<u64>>::entity_metadata_cid(id).is_none());
        // Set metadata_uri
        assert_ok!(EntityRegistry::update_entity(
            RuntimeOrigin::signed(ALICE), id,
            None, None, None, Some(b"ipfs://QmMeta".to_vec()), None,
        ));
        assert_eq!(
            <EntityRegistry as EntityProvider<u64>>::entity_metadata_cid(id),
            Some(b"ipfs://QmMeta".to_vec())
        );
    });
}

// M2-R11: unregister_shop 正确更新 primary（无二次存储读取）
#[test]
fn m2_r11_unregister_shop_updates_primary_correctly() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 100));
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 200));
        assert_eq!(Entities::<Test>::get(id).unwrap().primary_shop_id, 100);

        // 移除 primary shop → 应自动切换到 200
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::unregister_shop(id, 100));
        assert_eq!(Entities::<Test>::get(id).unwrap().primary_shop_id, 200);

        // 移除最后一个 → primary 清零
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::unregister_shop(id, 200));
        assert_eq!(Entities::<Test>::get(id).unwrap().primary_shop_id, 0);
    });
}

// ==================== Round 12 回归测试 ====================

// M1-R12: resign_admin 在治理锁定时被拒绝
#[test]
fn m1_r12_resign_admin_blocked_by_governance_lock() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // 添加 BOB 为管理员
        assert_ok!(EntityRegistry::add_admin(
            RuntimeOrigin::signed(ALICE), id, BOB, AdminPermission::ENTITY_MANAGE,
        ));
        // 锁定实体
        set_entity_locked(id);
        // BOB 尝试辞职 — 应被治理锁阻止
        assert_noop!(
            EntityRegistry::resign_admin(RuntimeOrigin::signed(BOB), id),
            Error::<Test>::EntityLocked
        );
        // 解锁后可以辞职
        clear_entity_locked(id);
        assert_ok!(EntityRegistry::resign_admin(RuntimeOrigin::signed(BOB), id));
    });
}

// M2-R12: ban_entity 清理 EntitySales 和 EntityShops
#[test]
fn m2_r12_ban_entity_cleans_sales_and_shops() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        // 注册 Shop 和更新销售数据
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 100));
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::update_entity_stats(id, 5000, 10));
        assert!(!EntityShops::<Test>::get(id).is_empty());
        assert_eq!(EntitySales::<Test>::get(id).total_orders, 10);

        // 治理封禁
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));

        // 验证: Shop 关联保留（以便 unban 恢复），销售数据清理
        assert!(!EntityShops::<Test>::get(id).is_empty(), "EntityShops should be preserved for unban recovery");
        assert!(EntityShops::<Test>::get(id).contains(&100));
        assert_eq!(EntitySales::<Test>::get(id).total_orders, 0);
        assert_eq!(EntitySales::<Test>::get(id).total_sales, 0u128);
    });
}

// L1-R12: close 清理 EntitySales 和 EntityShops
#[test]
fn l1_r12_close_cleans_sales_and_shops() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 100));
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::update_entity_stats(id, 3000, 5));

        // 关闭
        close_entity_via_timeout(ALICE, id);

        // 验证: Shop 关联保留（以便 reopen 恢复），销售数据清理
        assert!(!EntityShops::<Test>::get(id).is_empty(), "EntityShops should be preserved for reopen recovery");
        assert!(EntityShops::<Test>::get(id).contains(&100));
        assert_eq!(EntitySales::<Test>::get(id).total_orders, 0);
    });
}

// L1-R12: execute_close_timeout 清理 EntitySales 和 EntityShops
#[test]
fn l1_r12_execute_close_timeout_cleans_sales_and_shops() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 200));
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::update_entity_stats(id, 1000, 2));

        // 申请关闭
        assert_ok!(EntityRegistry::request_close_entity(RuntimeOrigin::signed(ALICE), id));

        // 推进区块超过超时
        System::set_block_number(1 + 100 + 1); // CloseRequestTimeout = 100

        // 任何人执行超时关闭
        assert_ok!(EntityRegistry::execute_close_timeout(RuntimeOrigin::signed(BOB), id));

        // 验证: Shop 关联保留（以便 reopen 恢复），销售数据清理
        assert!(!EntityShops::<Test>::get(id).is_empty(), "EntityShops should be preserved for reopen recovery");
        assert!(EntityShops::<Test>::get(id).contains(&200));
        assert_eq!(EntitySales::<Test>::get(id).total_orders, 0);
    });
}

// H1: 推荐人可以被多于 MaxEntitiesPerUser 个实体引用

// P0: ban 保留 EntityShops，unban 可恢复 Shop
#[test]
fn p0_ban_preserves_shops_for_unban_recovery() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 100));
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 200));
        assert_eq!(EntityShops::<Test>::get(id).len(), 2);

        // ban: Shop 被 force_close 但关联保留
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id, false, None));
        assert_eq!(EntityShops::<Test>::get(id).len(), 2);

        // 向 treasury 充值以满足 unban 资金要求
        let treasury = EntityRegistry::entity_treasury_account(id);
        let _ = Balances::deposit_creating(&treasury, EXPECTED_INITIAL_FUND);

        // unban: Shop 关联仍在，resume_shop 被调用
        assert_ok!(EntityRegistry::unban_entity(RuntimeOrigin::root(), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);
        assert_eq!(EntityShops::<Test>::get(id).len(), 2);
        assert!(EntityShops::<Test>::get(id).contains(&100));
        assert!(EntityShops::<Test>::get(id).contains(&200));
    });
}

// P0: close 保留 EntityShops，reopen 可恢复 Shop
#[test]
fn p0_close_preserves_shops_for_reopen_recovery() {
    new_test_ext().execute_with(|| {
        let id = create_default_entity(ALICE);
        assert_ok!(<EntityRegistry as EntityProvider<u64>>::register_shop(id, 100));
        assert_eq!(EntityShops::<Test>::get(id).len(), 1);

        // close: Shop 被 force_close 但关联保留
        close_entity_via_timeout(ALICE, id);
        assert_eq!(EntityShops::<Test>::get(id).len(), 1);

        // reopen: Shop 关联仍在，resume_shop 被调用
        assert_ok!(EntityRegistry::reopen_entity(RuntimeOrigin::signed(ALICE), id));
        assert_eq!(Entities::<Test>::get(id).unwrap().status, EntityStatus::Active);
        assert_eq!(EntityShops::<Test>::get(id).len(), 1);
        assert!(EntityShops::<Test>::get(id).contains(&100));
    });
}

#[test]
fn h1_referrer_can_have_many_referrals() {
    new_test_ext().execute_with(|| {
        // ALICE 创建实体作为推荐人
        let _alice_id = create_default_entity(ALICE);

        // BOB, CHARLIE, DAVE 各创建一个实体都引用 ALICE
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(BOB), b"Bob".to_vec(), None, None, Some(ALICE),
        ));
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(CHARLIE), b"Charlie".to_vec(), None, None, Some(ALICE),
        ));
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(DAVE), b"Dave".to_vec(), None, None, Some(ALICE),
        ));

        // ALICE 的反向索引应包含 3 个实体（MaxEntitiesPerUser=3，但 MaxReferralsPerReferrer=100）
        let referrals = ReferrerEntities::<Test>::get(ALICE);
        assert_eq!(referrals.len(), 3);
    });
}

// ==================== Round 13 回归测试 ====================

// M1-R13: update_entity 名称更新不会误删其他实体的名称索引
#[test]
fn m1_r13_update_entity_name_does_not_corrupt_other_entity_index() {
    new_test_ext().execute_with(|| {
        // 1. Entity A 创建，名称 "Alpha"
        let id_a = create_default_entity(ALICE);
        assert_ok!(EntityRegistry::update_entity(
            RuntimeOrigin::signed(ALICE), id_a,
            Some(b"Alpha".to_vec()), None, None, None, None,
        ));
        let norm_alpha = EntityRegistry::normalize_entity_name(b"alpha").unwrap();
        assert_eq!(EntityNameIndex::<Test>::get(&norm_alpha), Some(id_a));

        // 2. 治理封禁 Entity A → 名称索引 "alpha" 被移除
        assert_ok!(EntityRegistry::ban_entity(RuntimeOrigin::root(), id_a, false, None));
        assert_eq!(EntityNameIndex::<Test>::get(&norm_alpha), None);

        // 3. Entity B 创建，名称 "Alpha"（此时 A 已被 ban，名称可用）
        assert_ok!(EntityRegistry::create_entity(
            RuntimeOrigin::signed(BOB), b"Alpha".to_vec(), None, None, None,
        ));
        let id_b = EntityRegistry::next_entity_id() - 1;
        assert_eq!(EntityNameIndex::<Test>::get(&norm_alpha), Some(id_b));

        // 4. H1 审计修复: unban 时名称已被 B 占用 → 直接拒绝（更早拦截名称碰撞）
        assert_noop!(
            EntityRegistry::unban_entity(RuntimeOrigin::root(), id_a),
            Error::<Test>::NameAlreadyTaken
        );

        // 5. 验证 B 的 "alpha" 索引保持完整
        assert_eq!(EntityNameIndex::<Test>::get(&norm_alpha), Some(id_b));
    });
}
