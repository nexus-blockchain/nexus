//! KYC 模块测试（Per-Entity KYC 模型 v2）

use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok, traits::ConstU32, BoundedVec};

// ==================== Helper ====================

fn setup_entity() {
    MockEntityProvider::set_entity_owner(ENTITY_1, ENTITY_OWNER);
}

fn setup_entity2() {
    MockEntityProvider::set_entity_owner(ENTITY_2, ENTITY_OWNER);
}

fn setup_provider() {
    assert_ok!(EntityKyc::register_provider(
        RuntimeOrigin::root(),
        PROVIDER,
        b"Test Provider".to_vec(),
        KycLevel::Enhanced,
    ));
}

fn setup_provider_for_entity(entity_id: u64) {
    setup_provider();
    assert_ok!(EntityKyc::authorize_provider(
        RuntimeOrigin::signed(ENTITY_OWNER),
        entity_id,
        PROVIDER,
    ));
}

fn submit_and_approve(entity_id: u64, user: u64, level: KycLevel) {
    assert_ok!(EntityKyc::submit_kyc(
        RuntimeOrigin::signed(user),
        entity_id,
        level,
        b"QmKycData".to_vec(),
        *b"CN",
    ));
    assert_ok!(EntityKyc::approve_kyc(
        RuntimeOrigin::signed(PROVIDER),
        entity_id,
        user,
        20,
    ));
}

// ==================== register_provider ====================

#[test]
fn register_provider_works() {
    new_test_ext().execute_with(|| {
        setup_provider();
        let provider = Providers::<Test>::get(PROVIDER).unwrap();
        assert_eq!(provider.max_level, KycLevel::Enhanced);
        assert_eq!(ProviderCount::<Test>::get(), 1);
    });
}

#[test]
fn register_provider_fails_duplicate() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_noop!(
            EntityKyc::register_provider(
                RuntimeOrigin::root(), PROVIDER,
                b"Dup".to_vec(), KycLevel::Basic,
            ),
            Error::<Test>::ProviderAlreadyExists
        );
    });
}

#[test]
fn register_provider_rejects_none_max_level() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityKyc::register_provider(
                RuntimeOrigin::root(), PROVIDER, b"Test".to_vec(), KycLevel::None,
            ),
            Error::<Test>::InvalidKycLevel
        );
    });
}

#[test]
fn register_provider_rejects_empty_name() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityKyc::register_provider(
                RuntimeOrigin::root(), PROVIDER, b"".to_vec(), KycLevel::Basic,
            ),
            Error::<Test>::EmptyProviderName
        );
    });
}

#[test]
fn register_provider_fails_max_providers_reached() {
    new_test_ext().execute_with(|| {
        for i in 100..120u64 {
            assert_ok!(EntityKyc::register_provider(
                RuntimeOrigin::root(), i, format!("Provider{i}").into_bytes(), KycLevel::Basic,
            ));
        }
        assert_noop!(
            EntityKyc::register_provider(
                RuntimeOrigin::root(), 200, b"TooMany".to_vec(), KycLevel::Basic,
            ),
            Error::<Test>::MaxProvidersReached
        );
    });
}

// ==================== remove_provider ====================

#[test]
fn remove_provider_works() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_ok!(EntityKyc::remove_provider(RuntimeOrigin::root(), PROVIDER));
        assert!(Providers::<Test>::get(PROVIDER).is_none());
        assert_eq!(ProviderCount::<Test>::get(), 0);
    });
}

#[test]
fn remove_provider_fails_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityKyc::remove_provider(RuntimeOrigin::root(), 99),
            Error::<Test>::ProviderNotFound
        );
    });
}

#[test]
fn remove_provider_cleans_up_authorizations() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_entity2();
        setup_provider();
        assert_ok!(EntityKyc::authorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, PROVIDER));
        assert_ok!(EntityKyc::authorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_2, PROVIDER));

        assert!(EntityAuthorizedProviders::<Test>::contains_key(ENTITY_1, PROVIDER));
        assert!(EntityAuthorizedProviders::<Test>::contains_key(ENTITY_2, PROVIDER));

        assert_ok!(EntityKyc::remove_provider(RuntimeOrigin::root(), PROVIDER));

        assert!(!EntityAuthorizedProviders::<Test>::contains_key(ENTITY_1, PROVIDER));
        assert!(!EntityAuthorizedProviders::<Test>::contains_key(ENTITY_2, PROVIDER));
    });
}

// ==================== authorize/deauthorize_provider ====================

#[test]
fn authorize_provider_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider();

        assert_ok!(EntityKyc::authorize_provider(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, PROVIDER,
        ));
        assert!(EntityAuthorizedProviders::<Test>::contains_key(ENTITY_1, PROVIDER));
    });
}

#[test]
fn authorize_provider_fails_not_owner() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider();

        assert_noop!(
            EntityKyc::authorize_provider(RuntimeOrigin::signed(USER), ENTITY_1, PROVIDER),
            Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

#[test]
fn authorize_provider_fails_already_authorized() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);

        assert_noop!(
            EntityKyc::authorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, PROVIDER),
            Error::<Test>::ProviderAlreadyAuthorized
        );
    });
}

#[test]
fn authorize_provider_fails_provider_not_registered() {
    new_test_ext().execute_with(|| {
        setup_entity();

        assert_noop!(
            EntityKyc::authorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, PROVIDER),
            Error::<Test>::ProviderNotFound
        );
    });
}

#[test]
fn authorize_provider_fails_entity_not_active() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider();
        MockEntityProvider::set_entity_inactive(ENTITY_1);

        assert_noop!(
            EntityKyc::authorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, PROVIDER),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn deauthorize_provider_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);

        assert_ok!(EntityKyc::deauthorize_provider(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, PROVIDER,
        ));
        assert!(!EntityAuthorizedProviders::<Test>::contains_key(ENTITY_1, PROVIDER));
    });
}

#[test]
fn deauthorize_provider_fails_not_authorized() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider();

        assert_noop!(
            EntityKyc::deauthorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, PROVIDER),
            Error::<Test>::ProviderNotAuthorized
        );
    });
}

// ==================== submit_kyc ====================

#[test]
fn submit_kyc_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmKycData".to_vec(), *b"CN",
        ));

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.status, KycStatus::Pending);
        assert_eq!(record.level, KycLevel::Standard);
    });
}

#[test]
fn submit_kyc_fails_entity_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), 999, KycLevel::Basic, b"QmData".to_vec(), *b"CN",
            ),
            Error::<Test>::EntityNotFound
        );
    });
}

#[test]
fn submit_kyc_fails_entity_not_active() {
    new_test_ext().execute_with(|| {
        setup_entity();
        MockEntityProvider::set_entity_inactive(ENTITY_1);

        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, b"QmData".to_vec(), *b"CN",
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn submit_kyc_fails_already_pending() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, b"Qm1".to_vec(), *b"US",
        ));
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"Qm2".to_vec(), *b"US",
            ),
            Error::<Test>::KycAlreadyPending
        );
    });
}

#[test]
fn submit_kyc_fails_already_approved_not_expired() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, b"Qm2".to_vec(), *b"US",
            ),
            Error::<Test>::KycAlreadyApproved
        );
    });
}

#[test]
fn submit_kyc_allowed_after_expiry() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        System::set_block_number(1000);
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, b"QmNew".to_vec(), *b"CN",
        ));
    });
}

#[test]
fn submit_kyc_allowed_after_rejection() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmData".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::reject_kyc(
            RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, RejectionReason::UnclearDocument, None,
        ));
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmData2".to_vec(), *b"CN",
        ));
    });
}

#[test]
fn submit_kyc_fails_none_level() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::None, b"Qm".to_vec(), *b"CN",
            ),
            Error::<Test>::InvalidKycLevel
        );
    });
}

#[test]
fn submit_kyc_fails_empty_cid() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, b"".to_vec(), *b"CN",
            ),
            Error::<Test>::EmptyDataCid
        );
    });
}

#[test]
fn submit_kyc_fails_invalid_country_code() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, b"Qm".to_vec(), *b"cn",
            ),
            Error::<Test>::InvalidCountryCode
        );
    });
}

#[test]
fn submit_kyc_fails_cid_too_long() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, vec![b'X'; 65], *b"CN",
            ),
            Error::<Test>::CidTooLong
        );
    });
}

// ==================== 升级流程（P0 核心测试）====================

#[test]
fn upgrade_creates_separate_request() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmUpgrade".to_vec(), *b"CN",
        ));

        assert!(UpgradeRequests::<Test>::contains_key(ENTITY_1, USER));
        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.status, KycStatus::Approved);
        assert_eq!(record.level, KycLevel::Basic);
    });
}

#[test]
fn upgrade_preserves_permissions_during_review() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmUpgrade".to_vec(), *b"CN",
        ));

        assert_eq!(Pallet::<Test>::get_kyc_level(ENTITY_1, &USER), KycLevel::Basic);
        assert!(Pallet::<Test>::meets_kyc_requirement(ENTITY_1, &USER, KycLevel::Basic));
    });
}

#[test]
fn upgrade_approve_updates_level() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Enhanced, b"QmUpgrade".to_vec(), *b"US",
        ));
        assert_ok!(EntityKyc::approve_kyc(
            RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, 10,
        ));

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.level, KycLevel::Enhanced);
        assert_eq!(record.status, KycStatus::Approved);
        assert_eq!(record.risk_score, 10);
        assert_eq!(record.country_code, Some(*b"US"));
        assert!(!UpgradeRequests::<Test>::contains_key(ENTITY_1, USER));
    });
}

#[test]
fn upgrade_reject_keeps_original_level() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmUpgrade".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::reject_kyc(
            RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, RejectionReason::UnclearDocument, None,
        ));

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.level, KycLevel::Basic);
        assert_eq!(record.status, KycStatus::Approved);
        assert!(!UpgradeRequests::<Test>::contains_key(ENTITY_1, USER));
    });
}

#[test]
fn upgrade_cancel_keeps_original_level() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmUpgrade".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::cancel_kyc(RuntimeOrigin::signed(USER), ENTITY_1));

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.level, KycLevel::Basic);
        assert_eq!(record.status, KycStatus::Approved);
        assert!(!UpgradeRequests::<Test>::contains_key(ENTITY_1, USER));
    });
}

#[test]
fn upgrade_timeout_keeps_original_level() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmUpgrade".to_vec(), *b"CN",
        ));

        System::set_block_number(102);
        assert_ok!(EntityKyc::timeout_pending_kyc(RuntimeOrigin::signed(USER), ENTITY_1, USER));

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.level, KycLevel::Basic);
        assert_eq!(record.status, KycStatus::Approved);
        assert!(!UpgradeRequests::<Test>::contains_key(ENTITY_1, USER));
    });
}

#[test]
fn upgrade_update_data_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);

        System::set_block_number(10);
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmOld".to_vec(), *b"CN",
        ));

        System::set_block_number(50);
        assert_ok!(EntityKyc::update_kyc_data(
            RuntimeOrigin::signed(USER), ENTITY_1, b"QmNew".to_vec(),
        ));

        let upgrade = UpgradeRequests::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(upgrade.data_cid.to_vec(), b"QmNew".to_vec());
        assert_eq!(upgrade.submitted_at, 10, "submitted_at must not be reset by update_kyc_data");
    });
}

#[test]
fn upgrade_fails_duplicate_pending() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmUp1".to_vec(), *b"CN",
        ));
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Enhanced, b"QmUp2".to_vec(), *b"CN",
            ),
            Error::<Test>::KycAlreadyPending
        );
    });
}

#[test]
fn upgrade_approve_updates_counts_from_expired() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);
        assert_eq!(Pallet::<Test>::get_kyc_stats(ENTITY_1), (0, 1));

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Enhanced, b"QmUp".to_vec(), *b"US",
        ));
        assert_eq!(Pallet::<Test>::get_kyc_stats(ENTITY_1), (1, 1));

        System::set_block_number(1002);
        assert_ok!(EntityKyc::expire_kyc(RuntimeOrigin::signed(USER), ENTITY_1, USER));
        assert_eq!(Pallet::<Test>::get_kyc_stats(ENTITY_1), (1, 0));

        assert_ok!(EntityKyc::approve_kyc(
            RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, 5,
        ));
        assert_eq!(Pallet::<Test>::get_kyc_stats(ENTITY_1), (0, 1));

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.level, KycLevel::Enhanced);
        assert_eq!(record.status, KycStatus::Approved);
    });
}

#[test]
fn upgrade_pending_count_correct() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);

        assert_eq!(Pallet::<Test>::get_kyc_stats(ENTITY_1), (0, 1));

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmUp".to_vec(), *b"CN",
        ));
        assert_eq!(Pallet::<Test>::get_kyc_stats(ENTITY_1), (1, 1));

        assert_ok!(EntityKyc::approve_kyc(
            RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, 10,
        ));
        assert_eq!(Pallet::<Test>::get_kyc_stats(ENTITY_1), (0, 1));
    });
}

// ==================== Per-Entity 隔离核心测试 ====================

#[test]
fn different_entities_have_independent_kyc() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_entity2();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        submit_and_approve(ENTITY_1, USER, KycLevel::Enhanced);

        assert_eq!(Pallet::<Test>::get_kyc_level(ENTITY_1, &USER), KycLevel::Enhanced);
        assert_eq!(Pallet::<Test>::get_kyc_level(ENTITY_2, &USER), KycLevel::None);

        assert!(KycRecords::<Test>::get(ENTITY_2, USER).is_none());
    });
}

#[test]
fn user_can_have_different_levels_in_different_entities() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_entity2();
        setup_provider();
        assert_ok!(EntityKyc::authorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, PROVIDER));
        assert_ok!(EntityKyc::authorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_2, PROVIDER));
        System::set_block_number(1);

        submit_and_approve(ENTITY_1, USER, KycLevel::Enhanced);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_2, KycLevel::Basic, b"QmBasic".to_vec(), *b"US",
        ));
        assert_ok!(EntityKyc::approve_kyc(
            RuntimeOrigin::signed(PROVIDER), ENTITY_2, USER, 10,
        ));

        assert_eq!(Pallet::<Test>::get_kyc_level(ENTITY_1, &USER), KycLevel::Enhanced);
        assert_eq!(Pallet::<Test>::get_kyc_level(ENTITY_2, &USER), KycLevel::Basic);
    });
}

#[test]
fn revoke_in_one_entity_does_not_affect_another() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_entity2();
        setup_provider();
        assert_ok!(EntityKyc::authorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, PROVIDER));
        assert_ok!(EntityKyc::authorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_2, PROVIDER));
        System::set_block_number(1);

        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);
        submit_and_approve(ENTITY_2, USER, KycLevel::Standard);

        assert_ok!(EntityKyc::revoke_kyc(RuntimeOrigin::root(), ENTITY_1, USER, RejectionReason::SuspiciousActivity));

        assert_eq!(KycRecords::<Test>::get(ENTITY_1, USER).unwrap().status, KycStatus::Revoked);
        assert_eq!(KycRecords::<Test>::get(ENTITY_2, USER).unwrap().status, KycStatus::Approved);
    });
}

#[test]
fn per_entity_counts_are_independent() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_entity2();
        setup_provider();
        assert_ok!(EntityKyc::authorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, PROVIDER));
        assert_ok!(EntityKyc::authorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_2, PROVIDER));
        System::set_block_number(1);

        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER2), ENTITY_2, KycLevel::Basic, b"QmData".to_vec(), *b"US",
        ));

        assert_eq!(Pallet::<Test>::get_kyc_stats(ENTITY_1), (0, 1));
        assert_eq!(Pallet::<Test>::get_kyc_stats(ENTITY_2), (1, 0));
    });
}

// ==================== approve_kyc ====================

#[test]
fn approve_kyc_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmData".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::approve_kyc(
            RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, 20,
        ));

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.status, KycStatus::Approved);
        assert_eq!(record.risk_score, 20);
    });
}

#[test]
fn approve_kyc_fails_provider_not_authorized() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider();
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, b"QmData".to_vec(), *b"CN",
        ));
        assert_noop!(
            EntityKyc::approve_kyc(RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, 20),
            Error::<Test>::ProviderNotAuthorized
        );
    });
}

#[test]
fn approve_kyc_by_entity_owner_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmData".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::approve_kyc(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, USER, 15,
        ));

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.status, KycStatus::Approved);
    });
}

#[test]
fn approve_kyc_by_entity_admin_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        MockEntityProvider::set_entity_admin(ENTITY_1, ENTITY_ADMIN_USER, pallet_entity_common::AdminPermission::KYC_MANAGE);
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, b"QmData".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::approve_kyc(
            RuntimeOrigin::signed(ENTITY_ADMIN_USER), ENTITY_1, USER, 10,
        ));

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.status, KycStatus::Approved);
    });
}

#[test]
fn approve_kyc_fails_self_approval() {
    new_test_ext().execute_with(|| {
        setup_entity();
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, KycLevel::Basic, b"QmData".to_vec(), *b"CN",
        ));
        assert_noop!(
            EntityKyc::approve_kyc(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, ENTITY_OWNER, 10),
            Error::<Test>::SelfApprovalNotAllowed
        );
    });
}

#[test]
fn approve_kyc_fails_level_too_high() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Institutional, b"QmData".to_vec(), *b"CN",
        ));
        assert_noop!(
            EntityKyc::approve_kyc(RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, 20),
            Error::<Test>::ProviderLevelNotSupported
        );
    });
}

#[test]
fn approve_kyc_fails_risk_score_over_100() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, b"QmData".to_vec(), *b"CN",
        ));
        assert_noop!(
            EntityKyc::approve_kyc(RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, 101),
            Error::<Test>::InvalidRiskScore
        );
    });
}

#[test]
fn approve_kyc_fails_entity_not_active() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, b"QmData".to_vec(), *b"CN",
        ));
        MockEntityProvider::set_entity_inactive(ENTITY_1);
        assert_noop!(
            EntityKyc::approve_kyc(RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, 20),
            Error::<Test>::EntityNotActive
        );
    });
}

// ==================== reject_kyc ====================

#[test]
fn reject_kyc_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmData".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::reject_kyc(
            RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, RejectionReason::UnclearDocument, None,
        ));

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.status, KycStatus::Rejected);
    });
}

// ==================== revoke_kyc ====================

#[test]
fn revoke_kyc_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        assert_ok!(EntityKyc::revoke_kyc(
            RuntimeOrigin::root(), ENTITY_1, USER, RejectionReason::SuspiciousActivity,
        ));

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.status, KycStatus::Revoked);
    });
}

#[test]
fn revoke_kyc_fails_on_rejected() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmData".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::reject_kyc(
            RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, RejectionReason::UnclearDocument, None,
        ));
        assert_noop!(
            EntityKyc::revoke_kyc(RuntimeOrigin::root(), ENTITY_1, USER, RejectionReason::Other),
            Error::<Test>::InvalidKycStatus
        );
    });
}

#[test]
fn revoke_kyc_accepts_pending() {
    new_test_ext().execute_with(|| {
        setup_entity();
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmData".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::revoke_kyc(
            RuntimeOrigin::root(), ENTITY_1, USER, RejectionReason::SuspiciousActivity,
        ));
    });
}

#[test]
fn revoke_kyc_cleans_up_pending_upgrade() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmUp".to_vec(), *b"CN",
        ));
        assert!(UpgradeRequests::<Test>::contains_key(ENTITY_1, USER));

        assert_ok!(EntityKyc::revoke_kyc(
            RuntimeOrigin::root(), ENTITY_1, USER, RejectionReason::SuspiciousActivity,
        ));
        assert!(!UpgradeRequests::<Test>::contains_key(ENTITY_1, USER));
        assert_eq!(KycRecords::<Test>::get(ENTITY_1, USER).unwrap().status, KycStatus::Revoked);
    });
}

// ==================== entity_revoke_kyc (P1) ====================

#[test]
fn entity_revoke_kyc_by_owner_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        assert_ok!(EntityKyc::entity_revoke_kyc(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, USER, RejectionReason::SuspiciousActivity,
        ));

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.status, KycStatus::Revoked);
    });
}

#[test]
fn entity_revoke_kyc_by_admin_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        MockEntityProvider::set_entity_admin(ENTITY_1, ENTITY_ADMIN_USER, pallet_entity_common::AdminPermission::KYC_MANAGE);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        assert_ok!(EntityKyc::entity_revoke_kyc(
            RuntimeOrigin::signed(ENTITY_ADMIN_USER), ENTITY_1, USER, RejectionReason::ForgedDocument,
        ));

        assert_eq!(KycRecords::<Test>::get(ENTITY_1, USER).unwrap().status, KycStatus::Revoked);
    });
}

#[test]
fn entity_revoke_kyc_fails_not_owner() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        assert_noop!(
            EntityKyc::entity_revoke_kyc(
                RuntimeOrigin::signed(USER2), ENTITY_1, USER, RejectionReason::Other,
            ),
            Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

#[test]
fn entity_revoke_kyc_fails_entity_not_active() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        MockEntityProvider::set_entity_inactive(ENTITY_1);
        assert_noop!(
            EntityKyc::entity_revoke_kyc(
                RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, USER, RejectionReason::Other,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

// ==================== set_entity_requirement ====================

#[test]
fn set_entity_requirement_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_ok!(EntityKyc::set_entity_requirement(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, KycLevel::Standard, true, 100, false, 80,
        ));
        let req = EntityRequirements::<Test>::get(ENTITY_1).unwrap();
        assert_eq!(req.min_level, KycLevel::Standard);
        assert!(req.mandatory);
    });
}

#[test]
fn set_entity_requirement_by_admin() {
    new_test_ext().execute_with(|| {
        setup_entity();
        MockEntityProvider::set_entity_admin(ENTITY_1, ENTITY_ADMIN_USER, pallet_entity_common::AdminPermission::KYC_MANAGE);

        assert_ok!(EntityKyc::set_entity_requirement(
            RuntimeOrigin::signed(ENTITY_ADMIN_USER), ENTITY_1, KycLevel::Basic, true, 0, true, 100,
        ));
    });
}

#[test]
fn set_entity_requirement_fails_non_owner() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_noop!(
            EntityKyc::set_entity_requirement(
                RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, true, 0, true, 100,
            ),
            Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

#[test]
fn set_entity_requirement_fails_entity_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityKyc::set_entity_requirement(
                RuntimeOrigin::signed(ENTITY_OWNER), 999, KycLevel::Basic, true, 0, true, 100,
            ),
            Error::<Test>::EntityNotFound
        );
    });
}

#[test]
fn set_entity_requirement_fails_locked() {
    new_test_ext().execute_with(|| {
        setup_entity();
        MockEntityProvider::set_entity_locked(ENTITY_1);
        assert_noop!(
            EntityKyc::set_entity_requirement(
                RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, KycLevel::Basic, true, 0, true, 100,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

// ==================== update_high_risk_countries ====================

#[test]
fn update_high_risk_countries_works() {
    new_test_ext().execute_with(|| {
        let countries: BoundedVec<[u8; 2], ConstU32<50>> = vec![*b"IR", *b"KP"].try_into().unwrap();
        assert_ok!(EntityKyc::update_high_risk_countries(RuntimeOrigin::root(), countries));
        assert_eq!(HighRiskCountries::<Test>::get().len(), 2);
    });
}

#[test]
fn update_high_risk_countries_deduplicates() {
    new_test_ext().execute_with(|| {
        let countries: BoundedVec<[u8; 2], ConstU32<50>> = vec![*b"IR", *b"IR", *b"KP"].try_into().unwrap();
        assert_ok!(EntityKyc::update_high_risk_countries(RuntimeOrigin::root(), countries));
        assert_eq!(HighRiskCountries::<Test>::get().len(), 2);
    });
}

// ==================== expire_kyc ====================

#[test]
fn expire_kyc_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        System::set_block_number(502);
        assert_ok!(EntityKyc::expire_kyc(RuntimeOrigin::signed(USER), ENTITY_1, USER));
        assert_eq!(KycRecords::<Test>::get(ENTITY_1, USER).unwrap().status, KycStatus::Expired);
    });
}

#[test]
fn expire_kyc_fails_not_expired() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        System::set_block_number(10);
        assert_noop!(
            EntityKyc::expire_kyc(RuntimeOrigin::signed(USER), ENTITY_1, USER),
            Error::<Test>::KycNotExpired
        );
    });
}

// ==================== cancel_kyc ====================

#[test]
fn cancel_kyc_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, b"QmData".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::cancel_kyc(RuntimeOrigin::signed(USER), ENTITY_1));
        assert!(KycRecords::<Test>::get(ENTITY_1, USER).is_none());
    });
}

#[test]
fn cancel_kyc_fails_not_pending() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);

        assert_noop!(
            EntityKyc::cancel_kyc(RuntimeOrigin::signed(USER), ENTITY_1),
            Error::<Test>::InvalidKycStatus
        );
    });
}

// ==================== force_approve_kyc ====================

#[test]
fn force_approve_kyc_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(EntityKyc::force_approve_kyc(
            RuntimeOrigin::root(), ENTITY_1, USER, KycLevel::Enhanced, 30, *b"CN",
        ));

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.status, KycStatus::Approved);
        assert_eq!(record.level, KycLevel::Enhanced);
        assert_eq!(record.risk_score, 30);
    });
}

// ==================== renew_kyc ====================

#[test]
fn renew_kyc_works_for_approved() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        let old_expires = KycRecords::<Test>::get(ENTITY_1, USER).unwrap().expires_at.unwrap();

        System::set_block_number(100);
        assert_ok!(EntityKyc::renew_kyc(RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER));

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.status, KycStatus::Approved);
        assert!(record.expires_at.unwrap() > old_expires);
    });
}

#[test]
fn renew_kyc_works_for_expired() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        System::set_block_number(502);
        assert_ok!(EntityKyc::expire_kyc(RuntimeOrigin::signed(USER), ENTITY_1, USER));
        assert_ok!(EntityKyc::renew_kyc(RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER));
        assert_eq!(KycRecords::<Test>::get(ENTITY_1, USER).unwrap().status, KycStatus::Approved);
    });
}

#[test]
fn renew_kyc_fails_self() {
    new_test_ext().execute_with(|| {
        setup_entity();
        System::set_block_number(1);

        assert_ok!(EntityKyc::force_approve_kyc(
            RuntimeOrigin::root(), ENTITY_1, USER, KycLevel::Basic, 10, *b"CN",
        ));
        MockEntityProvider::set_entity_owner(ENTITY_1, USER);
        assert_noop!(
            EntityKyc::renew_kyc(RuntimeOrigin::signed(USER), ENTITY_1, USER),
            Error::<Test>::SelfApprovalNotAllowed
        );
    });
}

#[test]
fn renew_kyc_fails_rejected() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, b"QmData".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::reject_kyc(
            RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, RejectionReason::Other, None,
        ));
        assert_noop!(
            EntityKyc::renew_kyc(RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER),
            Error::<Test>::KycNotRenewable
        );
    });
}

// ==================== update_kyc_data ====================

#[test]
fn update_kyc_data_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmOld".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::update_kyc_data(
            RuntimeOrigin::signed(USER), ENTITY_1, b"QmNew".to_vec(),
        ));
        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.data_cid.unwrap().to_vec(), b"QmNew".to_vec());
    });
}

#[test]
fn update_kyc_data_fails_not_pending() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);

        assert_noop!(
            EntityKyc::update_kyc_data(RuntimeOrigin::signed(USER), ENTITY_1, b"QmNew".to_vec()),
            Error::<Test>::InvalidKycStatus
        );
    });
}

// ==================== purge_kyc_data (P2: GDPR) ====================

#[test]
fn purge_kyc_data_works_for_rejected() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmData".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::reject_kyc(
            RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, RejectionReason::UnclearDocument,
            Some(b"QmDetails".to_vec()),
        ));
        assert_ok!(EntityKyc::purge_kyc_data(RuntimeOrigin::signed(USER), ENTITY_1));

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert!(record.data_cid.is_none());
        assert!(record.rejection_details_cid.is_none());
        assert!(record.country_code.is_none());
        assert_eq!(record.risk_score, 0);
    });
}

#[test]
fn purge_kyc_data_fails_approved() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);

        assert_noop!(
            EntityKyc::purge_kyc_data(RuntimeOrigin::signed(USER), ENTITY_1),
            Error::<Test>::KycDataCannotBePurged
        );
    });
}

// ==================== remove_entity_requirement ====================

#[test]
fn remove_entity_requirement_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_ok!(EntityKyc::set_entity_requirement(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, KycLevel::Basic, true, 0, true, 100,
        ));
        assert_ok!(EntityKyc::remove_entity_requirement(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1,
        ));
        assert!(EntityRequirements::<Test>::get(ENTITY_1).is_none());
    });
}

#[test]
fn remove_entity_requirement_fails_not_owner() {
    new_test_ext().execute_with(|| {
        setup_entity();
        assert_ok!(EntityKyc::force_set_entity_requirement(
            RuntimeOrigin::root(), ENTITY_1, KycLevel::Basic, true, 0, true, 100,
        ));
        assert_noop!(
            EntityKyc::remove_entity_requirement(RuntimeOrigin::signed(USER), ENTITY_1),
            Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

// ==================== timeout_pending_kyc (P2: TimedOut) ====================

#[test]
fn timeout_pending_kyc_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmData".to_vec(), *b"CN",
        ));

        System::set_block_number(102);
        assert_ok!(EntityKyc::timeout_pending_kyc(RuntimeOrigin::signed(USER), ENTITY_1, USER));

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.status, KycStatus::Rejected);
        assert_eq!(record.rejection_reason, Some(RejectionReason::TimedOut));
    });
}

#[test]
fn timeout_pending_kyc_fails_not_timed_out() {
    new_test_ext().execute_with(|| {
        setup_entity();
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmData".to_vec(), *b"CN",
        ));

        System::set_block_number(50);
        assert_noop!(
            EntityKyc::timeout_pending_kyc(RuntimeOrigin::signed(USER), ENTITY_1, USER),
            Error::<Test>::PendingNotTimedOut
        );
    });
}

// ==================== batch_revoke_by_provider ====================

#[test]
fn batch_revoke_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);
        submit_and_approve(ENTITY_1, USER2, KycLevel::Basic);

        let accounts: BoundedVec<u64, ConstU32<100>> = vec![USER, USER2].try_into().unwrap();
        assert_ok!(EntityKyc::batch_revoke_by_provider(
            RuntimeOrigin::root(), ENTITY_1, PROVIDER, accounts, RejectionReason::SuspiciousActivity,
        ));

        assert_eq!(KycRecords::<Test>::get(ENTITY_1, USER).unwrap().status, KycStatus::Revoked);
        assert_eq!(KycRecords::<Test>::get(ENTITY_1, USER2).unwrap().status, KycStatus::Revoked);
    });
}

#[test]
fn batch_revoke_fails_empty_list() {
    new_test_ext().execute_with(|| {
        setup_provider();
        let accounts: BoundedVec<u64, ConstU32<100>> = vec![].try_into().unwrap();
        assert_noop!(
            EntityKyc::batch_revoke_by_provider(
                RuntimeOrigin::root(), ENTITY_1, PROVIDER, accounts, RejectionReason::Other,
            ),
            Error::<Test>::EmptyAccountList
        );
    });
}

#[test]
fn batch_revoke_fails_provider_mismatch() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, b"QmData".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::approve_kyc(
            RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, USER, 10,
        ));

        let accounts: BoundedVec<u64, ConstU32<100>> = vec![USER].try_into().unwrap();
        assert_noop!(
            EntityKyc::batch_revoke_by_provider(
                RuntimeOrigin::root(), ENTITY_1, PROVIDER, accounts, RejectionReason::Other,
            ),
            Error::<Test>::ProviderMismatch
        );
    });
}

// ==================== update_risk_score ====================

#[test]
fn update_risk_score_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        assert_ok!(EntityKyc::update_risk_score(
            RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, 50,
        ));
        assert_eq!(KycRecords::<Test>::get(ENTITY_1, USER).unwrap().risk_score, 50);
    });
}

#[test]
fn update_risk_score_fails_not_authorized() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        assert_noop!(
            EntityKyc::update_risk_score(RuntimeOrigin::signed(99), ENTITY_1, USER, 50),
            Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

// ==================== update/suspend/resume_provider ====================

#[test]
fn update_provider_works() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_ok!(EntityKyc::update_provider(
            RuntimeOrigin::root(), PROVIDER, Some(b"New Name".to_vec()), None,
        ));
        assert_eq!(Providers::<Test>::get(PROVIDER).unwrap().name.to_vec(), b"New Name".to_vec());
    });
}

#[test]
fn update_provider_rejects_noop() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_noop!(
            EntityKyc::update_provider(RuntimeOrigin::root(), PROVIDER, None, None),
            Error::<Test>::NothingToUpdate
        );
    });
}

#[test]
fn suspend_and_resume_provider_works() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_ok!(EntityKyc::suspend_provider(RuntimeOrigin::root(), PROVIDER));
        assert!(Providers::<Test>::get(PROVIDER).unwrap().suspended);
        assert_ok!(EntityKyc::resume_provider(RuntimeOrigin::root(), PROVIDER));
        assert!(!Providers::<Test>::get(PROVIDER).unwrap().suspended);
    });
}

#[test]
fn suspended_provider_cannot_approve() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, b"QmData".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::suspend_provider(RuntimeOrigin::root(), PROVIDER));
        assert_noop!(
            EntityKyc::approve_kyc(RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, 20),
            Error::<Test>::ProviderIsSuspended
        );
    });
}

// ==================== 辅助函数测试 ====================

#[test]
fn get_kyc_level_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert_eq!(Pallet::<Test>::get_kyc_level(ENTITY_1, &USER), KycLevel::None);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);
        assert_eq!(Pallet::<Test>::get_kyc_level(ENTITY_1, &USER), KycLevel::Standard);
    });
}

#[test]
fn get_kyc_level_returns_none_after_expiry() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        System::set_block_number(502);
        assert_eq!(Pallet::<Test>::get_kyc_level(ENTITY_1, &USER), KycLevel::None);
    });
}

#[test]
fn meets_kyc_requirement_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        assert!(Pallet::<Test>::meets_kyc_requirement(ENTITY_1, &USER, KycLevel::Basic));
        assert!(Pallet::<Test>::meets_kyc_requirement(ENTITY_1, &USER, KycLevel::Standard));
        assert!(!Pallet::<Test>::meets_kyc_requirement(ENTITY_1, &USER, KycLevel::Enhanced));
    });
}

#[test]
fn can_participate_in_entity_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert!(Pallet::<Test>::can_participate_in_entity(&USER, ENTITY_1));

        assert_ok!(EntityKyc::force_set_entity_requirement(
            RuntimeOrigin::root(), ENTITY_1, KycLevel::Standard, true, 0, true, 100,
        ));
        assert!(!Pallet::<Test>::can_participate_in_entity(&USER, ENTITY_1));

        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);
        assert!(Pallet::<Test>::can_participate_in_entity(&USER, ENTITY_1));
    });
}

#[test]
fn can_participate_blocked_by_high_risk_country() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        let countries: BoundedVec<[u8; 2], ConstU32<50>> = vec![*b"CN"].try_into().unwrap();
        assert_ok!(EntityKyc::update_high_risk_countries(RuntimeOrigin::root(), countries));
        assert_ok!(EntityKyc::force_set_entity_requirement(
            RuntimeOrigin::root(), ENTITY_1, KycLevel::Basic, true, 0, false, 100,
        ));
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        assert!(!Pallet::<Test>::can_participate_in_entity(&USER, ENTITY_1));
    });
}

#[test]
fn can_participate_blocked_by_risk_score() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert_ok!(EntityKyc::force_set_entity_requirement(
            RuntimeOrigin::root(), ENTITY_1, KycLevel::Basic, true, 0, true, 10,
        ));

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Standard, b"QmData".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::approve_kyc(
            RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, 50,
        ));

        assert!(!Pallet::<Test>::can_participate_in_entity(&USER, ENTITY_1));
    });
}

#[test]
fn get_risk_score_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert_eq!(Pallet::<Test>::get_risk_score(ENTITY_1, &USER), 100);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);
        assert_eq!(Pallet::<Test>::get_risk_score(ENTITY_1, &USER), 20);
    });
}

#[test]
fn get_risk_score_returns_100_for_expired() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        System::set_block_number(502);
        assert_eq!(Pallet::<Test>::get_risk_score(ENTITY_1, &USER), 100);
    });
}

#[test]
fn is_high_risk_country_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        let countries: BoundedVec<[u8; 2], ConstU32<50>> = vec![*b"CN"].try_into().unwrap();
        assert_ok!(EntityKyc::update_high_risk_countries(RuntimeOrigin::root(), countries));
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        assert!(Pallet::<Test>::is_high_risk_country(ENTITY_1, &USER));
        assert!(!Pallet::<Test>::is_high_risk_country(ENTITY_1, &USER2));
    });
}

// ==================== 历史记录测试 ====================

#[test]
fn history_records_per_entity() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_entity2();
        setup_provider();
        assert_ok!(EntityKyc::authorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, PROVIDER));
        assert_ok!(EntityKyc::authorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_2, PROVIDER));
        System::set_block_number(1);

        submit_and_approve(ENTITY_1, USER, KycLevel::Basic);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_2, KycLevel::Standard, b"QmData".to_vec(), *b"US",
        ));

        let h1 = Pallet::<Test>::get_kyc_history(ENTITY_1, &USER);
        let h2 = Pallet::<Test>::get_kyc_history(ENTITY_2, &USER);

        assert_eq!(h1.len(), 2);
        assert_eq!(h1[0].action, KycAction::Submitted);
        assert_eq!(h1[1].action, KycAction::Approved);

        assert_eq!(h2.len(), 1);
        assert_eq!(h2[0].action, KycAction::Submitted);
    });
}

#[test]
fn history_bounded_by_max() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        for i in 0..55u64 {
            System::set_block_number(i * 10000 + 1);
            assert_ok!(EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, b"QmData".to_vec(), *b"CN",
            ));
            assert_ok!(EntityKyc::reject_kyc(
                RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, RejectionReason::Other, None,
            ));
        }

        let history = Pallet::<Test>::get_kyc_history(ENTITY_1, &USER);
        assert!(history.len() <= 50);
    });
}

// ==================== 状态计数测试 ====================

#[test]
fn status_counts_lifecycle() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), ENTITY_1, KycLevel::Basic, b"QmData".to_vec(), *b"CN",
        ));
        assert_eq!(Pallet::<Test>::get_kyc_stats(ENTITY_1), (1, 0));

        assert_ok!(EntityKyc::approve_kyc(
            RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER, 10,
        ));
        assert_eq!(Pallet::<Test>::get_kyc_stats(ENTITY_1), (0, 1));

        System::set_block_number(1002);
        assert_ok!(EntityKyc::expire_kyc(RuntimeOrigin::signed(USER), ENTITY_1, USER));
        assert_eq!(Pallet::<Test>::get_kyc_stats(ENTITY_1), (0, 0));

        assert_ok!(EntityKyc::renew_kyc(RuntimeOrigin::signed(PROVIDER), ENTITY_1, USER));
        assert_eq!(Pallet::<Test>::get_kyc_stats(ENTITY_1), (0, 1));
    });
}

// ==================== KycProvider trait 实现测试 ====================

#[test]
fn kyc_provider_trait_uses_entity_id() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_entity2();
        setup_provider();
        assert_ok!(EntityKyc::authorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, PROVIDER));
        assert_ok!(EntityKyc::authorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_2, PROVIDER));
        System::set_block_number(1);

        submit_and_approve(ENTITY_1, USER, KycLevel::Enhanced);

        use pallet_entity_common::KycProvider;
        assert_eq!(<Pallet<Test> as KycProvider<u64>>::kyc_level(ENTITY_1, &USER), 3);
        assert_eq!(<Pallet<Test> as KycProvider<u64>>::kyc_level(ENTITY_2, &USER), 0);
        assert!(<Pallet<Test> as KycProvider<u64>>::is_kyc_approved(ENTITY_1, &USER));
        assert!(!<Pallet<Test> as KycProvider<u64>>::is_kyc_approved(ENTITY_2, &USER));
    });
}

#[test]
fn kyc_provider_is_kyc_expired_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        use pallet_entity_common::KycProvider;
        assert!(!<Pallet<Test> as KycProvider<u64>>::is_kyc_expired(ENTITY_1, &USER));

        System::set_block_number(502);
        assert!(<Pallet<Test> as KycProvider<u64>>::is_kyc_expired(ENTITY_1, &USER));
    });
}

#[test]
fn kyc_provider_can_participate_uses_entity_logic() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        assert_ok!(EntityKyc::force_set_entity_requirement(
            RuntimeOrigin::root(), ENTITY_1, KycLevel::Standard, true, 0, true, 100,
        ));

        use pallet_entity_common::KycProvider;
        assert!(!<Pallet<Test> as KycProvider<u64>>::can_participate(ENTITY_1, &USER));

        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);
        assert!(<Pallet<Test> as KycProvider<u64>>::can_participate(ENTITY_1, &USER));
    });
}

#[test]
fn kyc_provider_expires_at_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);

        use pallet_entity_common::KycProvider;
        assert_eq!(<Pallet<Test> as KycProvider<u64>>::kyc_expires_at(ENTITY_1, &USER), 0);

        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);
        assert_eq!(<Pallet<Test> as KycProvider<u64>>::kyc_expires_at(ENTITY_1, &USER), 501);
    });
}

// ==================== 合规检查测试 ====================

#[test]
fn compliance_non_mandatory_always_passes() {
    new_test_ext().execute_with(|| {
        let req = EntityKycRequirement {
            min_level: KycLevel::Enhanced,
            mandatory: false,
            grace_period: 0,
            allow_high_risk_countries: true,
            max_risk_score: 100,
        };
        assert!(Pallet::<Test>::check_account_compliance(ENTITY_1, &USER, &req));
    });
}

#[test]
fn compliance_grace_period_works() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider_for_entity(ENTITY_1);
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Standard);

        let req = EntityKycRequirement {
            min_level: KycLevel::Basic,
            mandatory: true,
            grace_period: 100,
            allow_high_risk_countries: true,
            max_risk_score: 100,
        };

        System::set_block_number(502);
        assert_ok!(EntityKyc::expire_kyc(RuntimeOrigin::signed(USER), ENTITY_1, USER));

        System::set_block_number(590);
        assert!(Pallet::<Test>::check_account_compliance(ENTITY_1, &USER, &req));

        System::set_block_number(602);
        assert!(!Pallet::<Test>::check_account_compliance(ENTITY_1, &USER, &req));
    });
}

// ==================== force_set_entity_requirement ====================

#[test]
fn force_set_entity_requirement_works_without_entity() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityKyc::force_set_entity_requirement(
            RuntimeOrigin::root(), 999, KycLevel::Basic, true, 0, true, 100,
        ));
        assert!(EntityRequirements::<Test>::get(999).is_some());
    });
}

// ==================== Institutional 有效期 ====================

#[test]
fn institutional_has_own_validity() {
    new_test_ext().execute_with(|| {
        setup_entity();
        setup_provider();
        assert_ok!(EntityKyc::update_provider(
            RuntimeOrigin::root(), PROVIDER, None, Some(KycLevel::Institutional),
        ));
        assert_ok!(EntityKyc::authorize_provider(RuntimeOrigin::signed(ENTITY_OWNER), ENTITY_1, PROVIDER));
        System::set_block_number(1);
        submit_and_approve(ENTITY_1, USER, KycLevel::Institutional);

        let record = KycRecords::<Test>::get(ENTITY_1, USER).unwrap();
        assert_eq!(record.expires_at.unwrap(), 3001);
    });
}
