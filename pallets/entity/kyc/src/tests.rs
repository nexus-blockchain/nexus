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
        // H8: 超过 50 个国家应报 TooManyCountries 错误（H2-audit: 用合法大写字母代码）
        let countries: Vec<[u8; 2]> = (0..51).map(|i| [b'A' + (i / 26) as u8, b'A' + (i % 26) as u8]).collect();
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

// ==================== C1: get_kyc_level 过期检查 ====================

#[test]
fn c1_get_kyc_level_returns_none_after_expiry() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        // 未过期时返回 Standard
        assert_eq!(Pallet::<Test>::get_kyc_level(&USER), KycLevel::Standard);

        // StandardKycValidity = 500, 推进到过期后
        System::set_block_number(502);

        // C1: 过期后应返回 None
        assert_eq!(Pallet::<Test>::get_kyc_level(&USER), KycLevel::None);
    });
}

// ==================== H1: submit_kyc 拒绝 None 级别 ====================

#[test]
fn h1_submit_kyc_rejects_none_level() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), KycLevel::None, b"data".to_vec(), *b"CN",
            ),
            Error::<Test>::InvalidKycLevel
        );
    });
}

// ==================== H2: approve_kyc 风险评分验证 ====================

#[test]
fn h2_approve_kyc_rejects_risk_score_over_100() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"data".to_vec(), *b"CN",
        ));

        // risk_score=101 应拒绝
        assert_noop!(
            EntityKyc::approve_kyc(RuntimeOrigin::signed(PROVIDER), USER, 101),
            Error::<Test>::InvalidRiskScore
        );

        // risk_score=255 应拒绝
        assert_noop!(
            EntityKyc::approve_kyc(RuntimeOrigin::signed(PROVIDER), USER, 255),
            Error::<Test>::InvalidRiskScore
        );

        // risk_score=100 应接受
        assert_ok!(EntityKyc::approve_kyc(RuntimeOrigin::signed(PROVIDER), USER, 100));
        assert_eq!(KycRecords::<Test>::get(USER).unwrap().risk_score, 100);
    });
}

// ==================== M1: register_provider 拒绝 max_level=None ====================

#[test]
fn m1_register_provider_rejects_none_max_level() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityKyc::register_provider(
                RuntimeOrigin::root(), PROVIDER,
                b"Provider".to_vec(), ProviderType::Internal, KycLevel::None,
            ),
            Error::<Test>::InvalidKycLevel
        );
    });
}

// ==================== L1: register_provider 拒绝空名称 ====================

#[test]
fn l1_register_provider_rejects_empty_name() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityKyc::register_provider(
                RuntimeOrigin::root(), PROVIDER,
                vec![], ProviderType::Internal, KycLevel::Basic,
            ),
            Error::<Test>::EmptyProviderName
        );
    });
}

// ==================== 额外覆盖测试 ====================

#[test]
fn register_provider_fails_max_providers_reached() {
    new_test_ext().execute_with(|| {
        // MaxProviders = 20, 注册 20 个
        for i in 10u64..30 {
            assert_ok!(EntityKyc::register_provider(
                RuntimeOrigin::root(), i,
                format!("P{}", i).into_bytes(), ProviderType::Internal, KycLevel::Basic,
            ));
        }
        assert_eq!(ProviderCount::<Test>::get(), 20);

        // 第 21 个应失败
        assert_noop!(
            EntityKyc::register_provider(
                RuntimeOrigin::root(), 99,
                b"Overflow".to_vec(), ProviderType::Internal, KycLevel::Basic,
            ),
            Error::<Test>::MaxProvidersReached
        );
    });
}

#[test]
fn can_participate_blocked_by_high_risk_country() {
    new_test_ext().execute_with(|| {
        // 设置实体要求：不允许高风险国家
        assert_ok!(EntityKyc::set_entity_requirement(
            RuntimeOrigin::root(), 1,
            KycLevel::Basic, true, 0, false, 100,
        ));

        // 设置 CN 为高风险国家
        assert_ok!(EntityKyc::update_high_risk_countries(
            RuntimeOrigin::root(), vec![*b"CN"],
        ));

        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        // USER 国家 = CN，应被拒绝
        assert!(!Pallet::<Test>::can_participate_in_entity(&USER, 1));
    });
}

#[test]
fn can_participate_blocked_by_risk_score() {
    new_test_ext().execute_with(|| {
        // 设置实体要求：max_risk_score=10
        assert_ok!(EntityKyc::set_entity_requirement(
            RuntimeOrigin::root(), 1,
            KycLevel::Basic, true, 0, true, 10,
        ));

        setup_provider();

        // 提交并批准，风险评分=20
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"data".to_vec(), *b"US",
        ));
        assert_ok!(EntityKyc::approve_kyc(RuntimeOrigin::signed(PROVIDER), USER, 20));

        // risk_score=20 > max_risk_score=10，应被拒绝
        assert!(!Pallet::<Test>::can_participate_in_entity(&USER, 1));
    });
}

#[test]
fn can_participate_blocked_by_expired_kyc() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityKyc::set_entity_requirement(
            RuntimeOrigin::root(), 1,
            KycLevel::Basic, true, 0, true, 100,
        ));

        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        assert!(Pallet::<Test>::can_participate_in_entity(&USER, 1));

        // 推进到过期后
        System::set_block_number(502);
        assert!(!Pallet::<Test>::can_participate_in_entity(&USER, 1));
    });
}

#[test]
fn approve_kyc_fails_provider_inactive() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Basic, b"data".to_vec(), *b"CN",
        ));

        // 移除提供者后再尝试批准
        assert_ok!(EntityKyc::remove_provider(RuntimeOrigin::root(), PROVIDER));
        assert_noop!(
            EntityKyc::approve_kyc(RuntimeOrigin::signed(PROVIDER), USER, 10),
            Error::<Test>::ProviderNotFound
        );
    });
}

// ==================== H1-audit: submit_kyc 拒绝空 CID ====================

#[test]
fn h1_submit_kyc_rejects_empty_cid() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), KycLevel::Basic, vec![], *b"CN",
            ),
            Error::<Test>::EmptyDataCid
        );
    });
}

// ==================== H2-audit: country_code 格式验证 ====================

#[test]
fn h2_submit_kyc_rejects_invalid_country_code() {
    new_test_ext().execute_with(|| {
        // 小写字母
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), KycLevel::Basic, b"data".to_vec(), *b"cn",
            ),
            Error::<Test>::InvalidCountryCode
        );
        // 数字
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), KycLevel::Basic, b"data".to_vec(), *b"12",
            ),
            Error::<Test>::InvalidCountryCode
        );
        // 零字节
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), KycLevel::Basic, b"data".to_vec(), [0, 0],
            ),
            Error::<Test>::InvalidCountryCode
        );
        // 正常大写字母应通过
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Basic, b"data".to_vec(), *b"US",
        ));
    });
}

#[test]
fn h2_update_high_risk_countries_rejects_invalid_codes() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityKyc::update_high_risk_countries(
                RuntimeOrigin::root(), vec![*b"IR", *b"kp"],
            ),
            Error::<Test>::InvalidCountryCode
        );
        // 全部大写应通过
        assert_ok!(EntityKyc::update_high_risk_countries(
            RuntimeOrigin::root(), vec![*b"IR", *b"KP"],
        ));
    });
}

// ==================== H3-audit: set_entity_requirement 验证 max_risk_score ====================

#[test]
fn h3_set_entity_requirement_rejects_risk_score_over_100() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityKyc::set_entity_requirement(
                RuntimeOrigin::root(), 1,
                KycLevel::Basic, true, 0, true, 101,
            ),
            Error::<Test>::InvalidRiskScore
        );
        assert_noop!(
            EntityKyc::set_entity_requirement(
                RuntimeOrigin::root(), 1,
                KycLevel::Basic, true, 0, true, 255,
            ),
            Error::<Test>::InvalidRiskScore
        );
        // 100 应通过
        assert_ok!(EntityKyc::set_entity_requirement(
            RuntimeOrigin::root(), 1,
            KycLevel::Basic, true, 0, true, 100,
        ));
    });
}

// ==================== M1-audit: reject_kyc 检查 provider level ====================

#[test]
fn m1_reject_kyc_checks_provider_level() {
    new_test_ext().execute_with(|| {
        // 注册一个只能处理 Basic 级别的提供者
        assert_ok!(EntityKyc::register_provider(
            RuntimeOrigin::root(), PROVIDER,
            b"Basic Only".to_vec(), ProviderType::ThirdParty, KycLevel::Basic,
        ));

        // 用户提交 Enhanced 级别
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Enhanced, b"data".to_vec(), *b"CN",
        ));

        // Basic 级别提供者不能拒绝 Enhanced 级别记录
        assert_noop!(
            EntityKyc::reject_kyc(
                RuntimeOrigin::signed(PROVIDER), USER,
                RejectionReason::Other, None,
            ),
            Error::<Test>::ProviderLevelNotSupported
        );
    });
}

// ==================== M2-audit: grace_period 宽限期生效 ====================

#[test]
fn m2_grace_period_extends_participation() {
    new_test_ext().execute_with(|| {
        // 设置实体要求：grace_period = 100 blocks
        assert_ok!(EntityKyc::set_entity_requirement(
            RuntimeOrigin::root(), 1,
            KycLevel::Basic, true, 100, true, 100,
        ));

        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        // StandardKycValidity = 500, expires at block 500
        // 推进到 block 501 — 刚过期但在宽限期内 (500 + 100 = 600)
        System::set_block_number(501);
        assert!(Pallet::<Test>::can_participate_in_entity(&USER, 1));

        // 推进到 block 599 — 仍在宽限期内
        System::set_block_number(599);
        assert!(Pallet::<Test>::can_participate_in_entity(&USER, 1));

        // 推进到 block 601 — 超出宽限期
        System::set_block_number(601);
        assert!(!Pallet::<Test>::can_participate_in_entity(&USER, 1));
    });
}

// ==================== H2-new: revoke_kyc 支持 Expired 状态 ====================

#[test]
fn h2_revoke_kyc_works_on_expired_record() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        // StandardKycValidity = 500, 手动设置过期状态
        System::set_block_number(502);

        // 先将记录标记为 Expired（模拟过期场景）
        KycRecords::<Test>::mutate(USER, |maybe| {
            if let Some(record) = maybe {
                record.status = KycStatus::Expired;
            }
        });

        // H2: 管理员可以撤销 Expired 状态的 KYC
        assert_ok!(EntityKyc::revoke_kyc(
            RuntimeOrigin::root(), USER, RejectionReason::SuspiciousActivity,
        ));
        assert_eq!(KycRecords::<Test>::get(USER).unwrap().status, KycStatus::Revoked);
    });
}

#[test]
fn h2_revoke_kyc_still_rejects_pending() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Basic, b"data".to_vec(), *b"CN",
        ));
        // Pending 状态仍然不能撤销
        assert_noop!(
            EntityKyc::revoke_kyc(RuntimeOrigin::root(), USER, RejectionReason::Other),
            Error::<Test>::InvalidKycStatus
        );
    });
}

// ==================== M1-new: reject_kyc 拒绝空 details_cid ====================

#[test]
fn m1_reject_kyc_rejects_empty_details_cid() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"data".to_vec(), *b"CN",
        ));

        // Some(vec![]) 空 CID 应被拒绝
        assert_noop!(
            EntityKyc::reject_kyc(
                RuntimeOrigin::signed(PROVIDER), USER,
                RejectionReason::UnclearDocument, Some(vec![]),
            ),
            Error::<Test>::EmptyDataCid
        );

        // None 仍然可以
        assert_ok!(EntityKyc::reject_kyc(
            RuntimeOrigin::signed(PROVIDER), USER,
            RejectionReason::UnclearDocument, None,
        ));
    });
}

#[test]
fn m1_reject_kyc_accepts_nonempty_details_cid() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"data".to_vec(), *b"CN",
        ));

        // 非空 CID 应接受
        assert_ok!(EntityKyc::reject_kyc(
            RuntimeOrigin::signed(PROVIDER), USER,
            RejectionReason::UnclearDocument, Some(b"QmRejectDetails".to_vec()),
        ));
        let record = KycRecords::<Test>::get(USER).unwrap();
        assert!(record.rejection_details_cid.is_some());
    });
}

// ==================== M3-audit: Institutional 独立有效期 ====================

#[test]
fn m3_institutional_has_own_validity() {
    new_test_ext().execute_with(|| {
        // InstitutionalKycValidity = 3000 (vs EnhancedKycValidity = 2000)
        assert_eq!(
            Pallet::<Test>::get_validity_period(KycLevel::Institutional),
            3000u64,
        );
        assert_eq!(
            Pallet::<Test>::get_validity_period(KycLevel::Enhanced),
            2000u64,
        );
        assert_ne!(
            Pallet::<Test>::get_validity_period(KycLevel::Institutional),
            Pallet::<Test>::get_validity_period(KycLevel::Enhanced),
        );
    });
}
