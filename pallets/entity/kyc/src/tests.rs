//! KYC 模块测试

use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};

// ==================== Helper ====================

fn setup_provider() {
    assert_ok!(EntityKyc::register_provider(
        RuntimeOrigin::root(),
        PROVIDER,
        b"Test Provider".to_vec(),
        ProviderType::ThirdParty,
        KycLevel::Enhanced,
    ));
}

fn submit_and_approve(user: u64, level: KycLevel) {
    assert_ok!(EntityKyc::submit_kyc(
        RuntimeOrigin::signed(user),
        level,
        b"QmKycData".to_vec(),
        *b"CN",
    ));
    assert_ok!(EntityKyc::approve_kyc(
        RuntimeOrigin::signed(PROVIDER),
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
        assert!(provider.active);
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
                b"Dup".to_vec(), ProviderType::Internal, KycLevel::Basic,
            ),
            Error::<Test>::ProviderAlreadyExists
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

// ==================== submit_kyc ====================

#[test]
fn submit_kyc_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER),
            KycLevel::Standard,
            b"QmKycData".to_vec(),
            *b"CN",
        ));

        let record = KycRecords::<Test>::get(USER).unwrap();
        assert_eq!(record.level, KycLevel::Standard);
        assert_eq!(record.status, KycStatus::Pending);
    });
}

#[test]
fn submit_kyc_fails_already_pending() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Basic, b"data".to_vec(), *b"US",
        ));
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), KycLevel::Standard, b"data2".to_vec(), *b"US",
            ),
            Error::<Test>::KycAlreadyPending
        );
    });
}

#[test]
fn submit_kyc_fails_already_approved_not_expired() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        // H6: 已批准且未过期，不能重新提交
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), KycLevel::Enhanced, b"new".to_vec(), *b"US",
            ),
            Error::<Test>::KycAlreadyApproved
        );
    });
}

#[test]
fn submit_kyc_allowed_after_expiry() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        // StandardKycValidity = 500, advance past expiry
        System::set_block_number(502);

        // H6: 过期后可以重新提交
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Enhanced, b"new".to_vec(), *b"CN",
        ));
        assert_eq!(KycRecords::<Test>::get(USER).unwrap().status, KycStatus::Pending);
    });
}

#[test]
fn submit_kyc_allowed_after_rejection() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"data".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::reject_kyc(
            RuntimeOrigin::signed(PROVIDER), USER,
            RejectionReason::ExpiredDocument, None,
        ));

        // 被拒绝后可以重新提交
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"data2".to_vec(), *b"CN",
        ));
    });
}

#[test]
fn submit_kyc_allowed_after_revocation() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);
        assert_ok!(EntityKyc::revoke_kyc(
            RuntimeOrigin::root(), USER, RejectionReason::SuspiciousActivity,
        ));

        // 被撤销后可以重新提交
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"new".to_vec(), *b"CN",
        ));
    });
}

#[test]
fn submit_kyc_fails_cid_too_long() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), KycLevel::Basic,
                vec![0u8; 65], // MaxCidLength = 64
                *b"CN",
            ),
            Error::<Test>::CidTooLong
        );
    });
}

// ==================== approve_kyc ====================

#[test]
fn approve_kyc_works() {
    new_test_ext().execute_with(|| {
        setup_provider();

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"QmKycData".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::approve_kyc(RuntimeOrigin::signed(PROVIDER), USER, 20));

        let record = KycRecords::<Test>::get(USER).unwrap();
        assert_eq!(record.status, KycStatus::Approved);
        assert_eq!(record.risk_score, 20);
        assert!(record.expires_at.is_some());
        assert!(record.verified_at.is_some());
    });
}

#[test]
fn approve_kyc_fails_not_provider() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Basic, b"data".to_vec(), *b"CN",
        ));
        assert_noop!(
            EntityKyc::approve_kyc(RuntimeOrigin::signed(USER), USER, 10),
            Error::<Test>::ProviderNotFound
        );
    });
}

#[test]
fn approve_kyc_fails_level_too_high() {
    new_test_ext().execute_with(|| {
        // Provider max_level = Enhanced
        setup_provider();

        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Institutional, b"data".to_vec(), *b"CN",
        ));
        assert_noop!(
            EntityKyc::approve_kyc(RuntimeOrigin::signed(PROVIDER), USER, 10),
            Error::<Test>::ProviderLevelNotSupported
        );
    });
}

// ==================== reject_kyc ====================

#[test]
fn reject_kyc_works() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"data".to_vec(), *b"CN",
        ));

        assert_ok!(EntityKyc::reject_kyc(
            RuntimeOrigin::signed(PROVIDER), USER,
            RejectionReason::ExpiredDocument,
            Some(b"QmDetails".to_vec()),
        ));

        let record = KycRecords::<Test>::get(USER).unwrap();
        assert_eq!(record.status, KycStatus::Rejected);
        assert_eq!(record.rejection_reason, Some(RejectionReason::ExpiredDocument));
    });
}

#[test]
fn reject_kyc_fails_not_pending() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        assert_noop!(
            EntityKyc::reject_kyc(
                RuntimeOrigin::signed(PROVIDER), USER,
                RejectionReason::Other, None,
            ),
            Error::<Test>::InvalidKycStatus
        );
    });
}

// ==================== revoke_kyc ====================

#[test]
fn revoke_kyc_works() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        assert_ok!(EntityKyc::revoke_kyc(
            RuntimeOrigin::root(), USER, RejectionReason::SuspiciousActivity,
        ));

        let record = KycRecords::<Test>::get(USER).unwrap();
        assert_eq!(record.status, KycStatus::Revoked);
    });
}

#[test]
fn revoke_kyc_fails_not_approved() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Basic, b"data".to_vec(), *b"CN",
        ));
        assert_noop!(
            EntityKyc::revoke_kyc(RuntimeOrigin::root(), USER, RejectionReason::Other),
            Error::<Test>::InvalidKycStatus
        );
    });
}

#[test]
fn revoke_kyc_fails_not_admin() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);
        assert_noop!(
            EntityKyc::revoke_kyc(
                RuntimeOrigin::signed(USER), USER, RejectionReason::Other,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ==================== set_entity_requirement ====================

#[test]
fn set_entity_requirement_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityKyc::set_entity_requirement(
            RuntimeOrigin::root(), 1,
            KycLevel::Standard, true, 100, false, 50,
        ));

        let req = EntityRequirements::<Test>::get(1).unwrap();
        assert_eq!(req.min_level, KycLevel::Standard);
        assert!(req.mandatory);
        assert_eq!(req.grace_period, 100);
        assert!(!req.allow_high_risk_countries);
        assert_eq!(req.max_risk_score, 50);
    });
}

// ==================== update_high_risk_countries ====================

#[test]
fn update_high_risk_countries_works() {
    new_test_ext().execute_with(|| {
        let countries = vec![*b"IR", *b"KP", *b"SY"];
        assert_ok!(EntityKyc::update_high_risk_countries(
            RuntimeOrigin::root(), countries,
        ));

        let stored = HighRiskCountries::<Test>::get();
        assert_eq!(stored.len(), 3);
        assert!(stored.contains(&*b"IR"));
    });
}

#[test]
fn update_high_risk_countries_fails_too_many() {
    new_test_ext().execute_with(|| {
        // H8: 超过 50 个国家应报 TooManyCountries 错误
        let countries: Vec<[u8; 2]> = (0..51).map(|i| [b'A', i as u8]).collect();
        assert_noop!(
            EntityKyc::update_high_risk_countries(RuntimeOrigin::root(), countries),
            Error::<Test>::TooManyCountries
        );
    });
}

// ==================== Helper functions ====================

#[test]
fn get_kyc_level_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(Pallet::<Test>::get_kyc_level(&USER), KycLevel::None);

        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        assert_eq!(Pallet::<Test>::get_kyc_level(&USER), KycLevel::Standard);
    });
}

#[test]
fn meets_kyc_requirement_works() {
    new_test_ext().execute_with(|| {
        // 未认证用户不满足 Standard 要求
        assert!(!Pallet::<Test>::meets_kyc_requirement(&USER, KycLevel::Standard));
        // 未认证用户满足 None 要求
        assert!(Pallet::<Test>::meets_kyc_requirement(&USER, KycLevel::None));

        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        assert!(Pallet::<Test>::meets_kyc_requirement(&USER, KycLevel::Standard));
        assert!(Pallet::<Test>::meets_kyc_requirement(&USER, KycLevel::Basic));
        assert!(!Pallet::<Test>::meets_kyc_requirement(&USER, KycLevel::Enhanced));
    });
}

#[test]
fn meets_kyc_requirement_expired() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        // StandardKycValidity = 500
        System::set_block_number(502);

        // 过期后不满足要求
        assert!(!Pallet::<Test>::meets_kyc_requirement(&USER, KycLevel::Standard));
    });
}

#[test]
fn is_high_risk_country_works() {
    new_test_ext().execute_with(|| {
        // 设置高风险国家
        assert_ok!(EntityKyc::update_high_risk_countries(
            RuntimeOrigin::root(), vec![*b"IR", *b"KP"],
        ));

        // 提交来自高风险国家的 KYC
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Basic, b"data".to_vec(), *b"IR",
        ));
        assert!(Pallet::<Test>::is_high_risk_country(&USER));

        // 非高风险国家
        assert!(!Pallet::<Test>::is_high_risk_country(&PROVIDER));
    });
}

#[test]
fn can_participate_in_entity_works() {
    new_test_ext().execute_with(|| {
        // 无要求时默认允许
        assert!(Pallet::<Test>::can_participate_in_entity(&USER, 1));

        // 设置实体要求
        assert_ok!(EntityKyc::set_entity_requirement(
            RuntimeOrigin::root(), 1,
            KycLevel::Standard, true, 0, true, 100,
        ));

        // 未认证用户不满足
        assert!(!Pallet::<Test>::can_participate_in_entity(&USER, 1));

        // 认证后满足
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);
        assert!(Pallet::<Test>::can_participate_in_entity(&USER, 1));
    });
}

#[test]
fn get_risk_score_works() {
    new_test_ext().execute_with(|| {
        // 未认证用户：最高风险 100
        assert_eq!(Pallet::<Test>::get_risk_score(&USER), 100);

        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        assert_eq!(Pallet::<Test>::get_risk_score(&USER), 20);
    });
}

#[test]
fn provider_verification_count_increments() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        let provider = Providers::<Test>::get(PROVIDER).unwrap();
        assert_eq!(provider.verifications_count, 1);
    });
}
