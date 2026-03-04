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

        // H6: 已批准且未过期，同级别或低级别不能重新提交（升级到更高级别则允许）
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), KycLevel::Standard, b"same".to_vec(), *b"US",
            ),
            Error::<Test>::KycAlreadyApproved
        );
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), KycLevel::Basic, b"lower".to_vec(), *b"US",
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
fn revoke_kyc_fails_on_rejected() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"data".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::reject_kyc(
            RuntimeOrigin::signed(PROVIDER), USER, RejectionReason::Other, None,
        ));
        // Rejected 状态不能 revoke
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
        assert_ok!(EntityKyc::force_set_entity_requirement(
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
        assert_ok!(EntityKyc::force_set_entity_requirement(
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
        assert_ok!(EntityKyc::force_set_entity_requirement(
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
        assert_ok!(EntityKyc::force_set_entity_requirement(
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
        assert_ok!(EntityKyc::force_set_entity_requirement(
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
            EntityKyc::force_set_entity_requirement(
                RuntimeOrigin::root(), 1,
                KycLevel::Basic, true, 0, true, 101,
            ),
            Error::<Test>::InvalidRiskScore
        );
        assert_noop!(
            EntityKyc::force_set_entity_requirement(
                RuntimeOrigin::root(), 1,
                KycLevel::Basic, true, 0, true, 255,
            ),
            Error::<Test>::InvalidRiskScore
        );
        // 100 应通过
        assert_ok!(EntityKyc::force_set_entity_requirement(
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
        assert_ok!(EntityKyc::force_set_entity_requirement(
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
fn h1_r2_revoke_kyc_accepts_pending() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Basic, b"data".to_vec(), *b"CN",
        ));
        // H1-R2: Pending 状态现在可以撤销
        assert_ok!(EntityKyc::revoke_kyc(
            RuntimeOrigin::root(), USER, RejectionReason::Other,
        ));
        assert_eq!(KycRecords::<Test>::get(USER).unwrap().status, KycStatus::Revoked);
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

// ==================== Deep Audit Round 2 回归测试 ====================

// H1-R2: revoke_kyc 可以撤销 Pending 状态（解决卡死记录）
#[test]
fn h1_r2_revoke_kyc_clears_stuck_pending() {
    new_test_ext().execute_with(|| {
        // 用户提交 Institutional 级别 KYC
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Institutional, b"data".to_vec(), *b"CN",
        ));
        assert_eq!(KycRecords::<Test>::get(USER).unwrap().status, KycStatus::Pending);

        // 注册一个只支持 Basic 的 provider — 无法处理 Institutional
        assert_ok!(EntityKyc::register_provider(
            RuntimeOrigin::root(), PROVIDER,
            b"Basic Only".to_vec(), ProviderType::Internal, KycLevel::Basic,
        ));
        assert_noop!(
            EntityKyc::approve_kyc(RuntimeOrigin::signed(PROVIDER), USER, 10),
            Error::<Test>::ProviderLevelNotSupported
        );
        assert_noop!(
            EntityKyc::reject_kyc(RuntimeOrigin::signed(PROVIDER), USER, RejectionReason::Other, None),
            Error::<Test>::ProviderLevelNotSupported
        );

        // 用户不能重新提交（被 KycAlreadyPending 阻止）
        assert_noop!(
            EntityKyc::submit_kyc(RuntimeOrigin::signed(USER), KycLevel::Basic, b"new".to_vec(), *b"CN"),
            Error::<Test>::KycAlreadyPending
        );

        // H1-R2: Admin 可以撤销 Pending 记录
        assert_ok!(EntityKyc::revoke_kyc(
            RuntimeOrigin::root(), USER, RejectionReason::Other,
        ));
        assert_eq!(KycRecords::<Test>::get(USER).unwrap().status, KycStatus::Revoked);

        // 用户现在可以重新提交
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Basic, b"retry".to_vec(), *b"CN",
        ));
        assert_eq!(KycRecords::<Test>::get(USER).unwrap().status, KycStatus::Pending);
    });
}

// H1-R2: revoke_kyc 仍然拒绝 Rejected 和 Revoked 状态
#[test]
fn h1_r2_revoke_kyc_still_rejects_rejected_and_revoked() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"data".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::reject_kyc(
            RuntimeOrigin::signed(PROVIDER), USER, RejectionReason::Other, None,
        ));

        // Rejected 状态不能 revoke
        assert_noop!(
            EntityKyc::revoke_kyc(RuntimeOrigin::root(), USER, RejectionReason::Other),
            Error::<Test>::InvalidKycStatus
        );
    });
}

// M1: approve_kyc 拒绝自我审批
#[test]
fn m1_r2_approve_kyc_rejects_self_approval() {
    new_test_ext().execute_with(|| {
        // Provider 也是用户，提交自己的 KYC
        assert_ok!(EntityKyc::register_provider(
            RuntimeOrigin::root(), PROVIDER,
            b"Self Provider".to_vec(), ProviderType::Internal, KycLevel::Enhanced,
        ));
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(PROVIDER), KycLevel::Standard, b"data".to_vec(), *b"CN",
        ));

        // M1: Provider 审批自己应被拒绝
        assert_noop!(
            EntityKyc::approve_kyc(RuntimeOrigin::signed(PROVIDER), PROVIDER, 10),
            Error::<Test>::SelfApprovalNotAllowed
        );
    });
}

// ==================== Deep Audit Round 4 回归测试 ====================

// M1-R4: update_high_risk_countries 自动去重（替代 R2 的拒绝策略）
#[test]
fn m1_r4_update_high_risk_countries_deduplicates() {
    new_test_ext().execute_with(|| {
        // 含重复项 — 应自动去重而非拒绝
        assert_ok!(EntityKyc::update_high_risk_countries(
            RuntimeOrigin::root(), vec![*b"IR", *b"KP", *b"IR"],
        ));
        // 去重后只有 2 个
        assert_eq!(HighRiskCountries::<Test>::get().len(), 2);
        assert!(HighRiskCountries::<Test>::get().contains(&*b"IR"));
        assert!(HighRiskCountries::<Test>::get().contains(&*b"KP"));

        // 无重复仍正常工作
        assert_ok!(EntityKyc::update_high_risk_countries(
            RuntimeOrigin::root(), vec![*b"IR", *b"KP", *b"SY"],
        ));
        assert_eq!(HighRiskCountries::<Test>::get().len(), 3);
    });
}

// M1-R4: 大量重复去重后仍能存入（不超上限）
#[test]
fn m1_r4_dedup_preserves_capacity() {
    new_test_ext().execute_with(|| {
        // 3 个唯一 + 大量重复 = 去重后 3 个，远低于 50 上限
        let mut countries = vec![*b"IR"; 40];
        countries.extend_from_slice(&[*b"KP", *b"SY"]);
        assert_ok!(EntityKyc::update_high_risk_countries(
            RuntimeOrigin::root(), countries,
        ));
        assert_eq!(HighRiskCountries::<Test>::get().len(), 3);
    });
}

// M2-R4: reject_kyc 递增 verifications_count
#[test]
fn m2_r4_reject_kyc_increments_verifications_count() {
    new_test_ext().execute_with(|| {
        setup_provider();

        // 提交 KYC
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"data".to_vec(), *b"CN",
        ));

        // 拒绝前 count = 0
        assert_eq!(Providers::<Test>::get(PROVIDER).unwrap().verifications_count, 0);

        // 拒绝
        assert_ok!(EntityKyc::reject_kyc(
            RuntimeOrigin::signed(PROVIDER), USER,
            RejectionReason::UnclearDocument, None,
        ));

        // M2-R4: 拒绝后 count = 1
        assert_eq!(Providers::<Test>::get(PROVIDER).unwrap().verifications_count, 1);
    });
}

// M2-R4: approve + reject 都递增 verifications_count
#[test]
fn m2_r4_approve_and_reject_both_increment_count() {
    new_test_ext().execute_with(|| {
        setup_provider();
        let user2: u64 = 4;

        // 批准 USER
        submit_and_approve(USER, KycLevel::Standard);
        assert_eq!(Providers::<Test>::get(PROVIDER).unwrap().verifications_count, 1);

        // 提交 user2 并拒绝
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(user2), KycLevel::Standard, b"data2".to_vec(), *b"US",
        ));
        assert_ok!(EntityKyc::reject_kyc(
            RuntimeOrigin::signed(PROVIDER), user2,
            RejectionReason::InformationMismatch, None,
        ));

        // 1 approve + 1 reject = 2
        assert_eq!(Providers::<Test>::get(PROVIDER).unwrap().verifications_count, 2);
    });
}

// M3-R4: expire_kyc 标记已过期的 KYC
#[test]
fn m3_r4_expire_kyc_marks_expired_record() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Basic);

        let record = KycRecords::<Test>::get(USER).unwrap();
        assert_eq!(record.status, KycStatus::Approved);
        let expires_at = record.expires_at.unwrap();

        // 推进到过期后
        System::set_block_number(expires_at + 1);

        // 任何人可调用 expire_kyc
        let caller: u64 = 99;
        assert_ok!(EntityKyc::expire_kyc(
            RuntimeOrigin::signed(caller), USER,
        ));

        let record = KycRecords::<Test>::get(USER).unwrap();
        assert_eq!(record.status, KycStatus::Expired);

        // 验证事件
        System::assert_has_event(RuntimeEvent::EntityKyc(
            crate::Event::KycExpired { account: USER }
        ));
    });
}

// M3-R4: expire_kyc 拒绝未过期的记录
#[test]
fn m3_r4_expire_kyc_rejects_not_expired() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Basic);

        // 在有效期内调用 — 应拒绝
        assert_noop!(
            EntityKyc::expire_kyc(RuntimeOrigin::signed(USER), USER),
            Error::<Test>::KycNotExpired
        );
    });
}

// M3-R4: expire_kyc 拒绝非 Approved 状态
#[test]
fn m3_r4_expire_kyc_rejects_non_approved() {
    new_test_ext().execute_with(|| {
        // Pending 状态
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Basic, b"data".to_vec(), *b"CN",
        ));
        assert_noop!(
            EntityKyc::expire_kyc(RuntimeOrigin::signed(USER), USER),
            Error::<Test>::InvalidKycStatus
        );
    });
}

// M3-R4: expire_kyc 后可以 revoke（H1-R2 Expired 路径现在可达）
#[test]
fn m3_r4_expire_then_revoke_works() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Basic);

        let expires_at = KycRecords::<Test>::get(USER).unwrap().expires_at.unwrap();
        System::set_block_number(expires_at + 1);

        // 先 expire
        assert_ok!(EntityKyc::expire_kyc(RuntimeOrigin::signed(USER), USER));
        assert_eq!(KycRecords::<Test>::get(USER).unwrap().status, KycStatus::Expired);

        // 然后 admin revoke
        assert_ok!(EntityKyc::revoke_kyc(
            RuntimeOrigin::root(), USER, RejectionReason::SuspiciousActivity,
        ));
        assert_eq!(KycRecords::<Test>::get(USER).unwrap().status, KycStatus::Revoked);
    });
}

// M3-R4: expire_kyc 后可以重新提交
#[test]
fn m3_r4_expire_then_resubmit_works() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Basic);

        let expires_at = KycRecords::<Test>::get(USER).unwrap().expires_at.unwrap();
        System::set_block_number(expires_at + 1);

        // Expire
        assert_ok!(EntityKyc::expire_kyc(RuntimeOrigin::signed(USER), USER));

        // 重新提交 — Expired 状态允许重新提交（与 Rejected/Revoked 一样）
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"new_data".to_vec(), *b"CN",
        ));
        assert_eq!(KycRecords::<Test>::get(USER).unwrap().status, KycStatus::Pending);
    });
}

// ==================== Deep Audit Round 5 回归测试 ====================

// L4-R5: expire_kyc 不绕过 grace_period — Expired 状态在宽限期内仍可参与实体活动
#[test]
fn l4_r5_expire_kyc_does_not_bypass_grace_period() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        let record = KycRecords::<Test>::get(USER).unwrap();
        let expires_at = record.expires_at.unwrap();

        // 设置实体要求：mandatory + grace_period = 100 blocks
        assert_ok!(EntityKyc::force_set_entity_requirement(
            RuntimeOrigin::root(), 1,
            KycLevel::Basic, true, 100, true, 50,
        ));

        // 推进到刚过期
        System::set_block_number(expires_at + 1);

        // 在 expire_kyc 之前 — lazy expiry，grace_period 生效
        assert!(Pallet::<Test>::can_participate_in_entity(&USER, 1));

        // 调用 expire_kyc — 状态变为 Expired
        assert_ok!(EntityKyc::expire_kyc(RuntimeOrigin::signed(USER), USER));
        assert_eq!(KycRecords::<Test>::get(USER).unwrap().status, KycStatus::Expired);

        // L4-R5: Expired 状态在宽限期内仍可参与
        assert!(Pallet::<Test>::can_participate_in_entity(&USER, 1));

        // 推进到宽限期结束后
        System::set_block_number(expires_at + 101);
        assert!(!Pallet::<Test>::can_participate_in_entity(&USER, 1));
    });
}

// L4-R5: Revoked/Rejected 状态不享受 grace_period（仅 Approved + Expired）
#[test]
fn l4_r5_revoked_rejected_still_denied_participation() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        assert_ok!(EntityKyc::force_set_entity_requirement(
            RuntimeOrigin::root(), 1,
            KycLevel::Basic, true, 100, true, 50,
        ));

        // Revoke — 不享受 grace_period
        assert_ok!(EntityKyc::revoke_kyc(
            RuntimeOrigin::root(), USER, RejectionReason::SuspiciousActivity,
        ));
        assert!(!Pallet::<Test>::can_participate_in_entity(&USER, 1));
    });
}

// ==================== 新功能测试 ====================

// ---- P0-1: Entity Owner 自主设置 KYC 要求 ----

#[test]
fn p0_entity_owner_can_set_kyc_requirement() {
    new_test_ext().execute_with(|| {
        MockEntityProvider::set_entity_owner(1, ENTITY_OWNER);

        assert_ok!(EntityKyc::set_entity_requirement(
            RuntimeOrigin::signed(ENTITY_OWNER), 1,
            KycLevel::Standard, true, 100, false, 50,
        ));

        let req = EntityRequirements::<Test>::get(1).unwrap();
        assert_eq!(req.min_level, KycLevel::Standard);
        assert!(req.mandatory);
    });
}

#[test]
fn p0_non_owner_cannot_set_kyc_requirement() {
    new_test_ext().execute_with(|| {
        MockEntityProvider::set_entity_owner(1, ENTITY_OWNER);

        assert_noop!(
            EntityKyc::set_entity_requirement(
                RuntimeOrigin::signed(USER), 1,
                KycLevel::Basic, true, 0, true, 100,
            ),
            Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

#[test]
fn p0_set_entity_requirement_fails_entity_not_found() {
    new_test_ext().execute_with(|| {
        // Entity 999 不存在
        assert_noop!(
            EntityKyc::set_entity_requirement(
                RuntimeOrigin::signed(ENTITY_OWNER), 999,
                KycLevel::Basic, true, 0, true, 100,
            ),
            Error::<Test>::EntityNotFound
        );
    });
}

#[test]
fn p0_force_set_entity_requirement_works_without_entity() {
    new_test_ext().execute_with(|| {
        // force 版本不需要 Entity 存在
        assert_ok!(EntityKyc::force_set_entity_requirement(
            RuntimeOrigin::root(), 999,
            KycLevel::Enhanced, true, 0, false, 80,
        ));
        let req = EntityRequirements::<Test>::get(999).unwrap();
        assert_eq!(req.min_level, KycLevel::Enhanced);
    });
}

// ---- P1-1: KycLevel as_u8 / try_from_u8 ----

#[test]
fn p1_kyc_level_as_u8_roundtrip() {
    assert_eq!(KycLevel::None.as_u8(), 0);
    assert_eq!(KycLevel::Basic.as_u8(), 1);
    assert_eq!(KycLevel::Standard.as_u8(), 2);
    assert_eq!(KycLevel::Enhanced.as_u8(), 3);
    assert_eq!(KycLevel::Institutional.as_u8(), 4);

    for v in 0..=4u8 {
        assert_eq!(KycLevel::try_from_u8(v).unwrap().as_u8(), v);
    }
    assert!(KycLevel::try_from_u8(5).is_none());
    assert!(KycLevel::try_from_u8(255).is_none());
}

// ---- P1-2: cancel_kyc ----

#[test]
fn p1_cancel_kyc_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"data".to_vec(), *b"CN",
        ));
        assert_eq!(KycRecords::<Test>::get(USER).unwrap().status, KycStatus::Pending);

        assert_ok!(EntityKyc::cancel_kyc(RuntimeOrigin::signed(USER)));
        assert!(KycRecords::<Test>::get(USER).is_none());

        // 验证事件
        System::assert_has_event(RuntimeEvent::EntityKyc(
            crate::Event::KycCancelled { account: USER }
        ));
    });
}

#[test]
fn p1_cancel_kyc_fails_not_pending() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        // Approved 状态不能 cancel
        assert_noop!(
            EntityKyc::cancel_kyc(RuntimeOrigin::signed(USER)),
            Error::<Test>::InvalidKycStatus
        );
    });
}

#[test]
fn p1_cancel_kyc_fails_no_record() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityKyc::cancel_kyc(RuntimeOrigin::signed(USER)),
            Error::<Test>::KycNotFound
        );
    });
}

#[test]
fn p1_cancel_then_resubmit_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Basic, b"data".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::cancel_kyc(RuntimeOrigin::signed(USER)));

        // 取消后可重新提交
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"data2".to_vec(), *b"US",
        ));
        let record = KycRecords::<Test>::get(USER).unwrap();
        assert_eq!(record.level, KycLevel::Standard);
        assert_eq!(record.status, KycStatus::Pending);
    });
}

// ---- P1-3: upgrade_kyc (submit_kyc 允许升级) ----

#[test]
fn p1_upgrade_kyc_while_approved_works() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Basic);
        assert_eq!(KycRecords::<Test>::get(USER).unwrap().level, KycLevel::Basic);

        // 已批准 Basic，升级到 Standard
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"upgrade_data".to_vec(), *b"CN",
        ));
        let record = KycRecords::<Test>::get(USER).unwrap();
        assert_eq!(record.level, KycLevel::Standard);
        assert_eq!(record.status, KycStatus::Pending);
    });
}

#[test]
fn p1_upgrade_kyc_same_level_fails() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        // 同级别不算升级
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), KycLevel::Standard, b"same".to_vec(), *b"CN",
            ),
            Error::<Test>::KycAlreadyApproved
        );
    });
}

#[test]
fn p1_upgrade_kyc_lower_level_fails() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        // 降级不允许
        assert_noop!(
            EntityKyc::submit_kyc(
                RuntimeOrigin::signed(USER), KycLevel::Basic, b"lower".to_vec(), *b"CN",
            ),
            Error::<Test>::KycAlreadyApproved
        );
    });
}

// ---- P2-1: update_risk_score ----

#[test]
fn p2_update_risk_score_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        assert_eq!(KycRecords::<Test>::get(USER).unwrap().risk_score, 20);

        assert_ok!(EntityKyc::update_risk_score(
            RuntimeOrigin::signed(PROVIDER), USER, 75,
        ));

        assert_eq!(KycRecords::<Test>::get(USER).unwrap().risk_score, 75);

        System::assert_has_event(RuntimeEvent::EntityKyc(
            crate::Event::RiskScoreUpdated { account: USER, old_score: 20, new_score: 75 }
        ));
    });
}

#[test]
fn p2_update_risk_score_fails_not_provider() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        assert_noop!(
            EntityKyc::update_risk_score(RuntimeOrigin::signed(USER), USER, 50),
            Error::<Test>::ProviderNotFound
        );
    });
}

#[test]
fn p2_update_risk_score_fails_not_approved() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"data".to_vec(), *b"CN",
        ));

        // Pending 状态不能更新风险评分
        assert_noop!(
            EntityKyc::update_risk_score(RuntimeOrigin::signed(PROVIDER), USER, 50),
            Error::<Test>::InvalidKycStatus
        );
    });
}

#[test]
fn p2_update_risk_score_fails_over_100() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        assert_noop!(
            EntityKyc::update_risk_score(RuntimeOrigin::signed(PROVIDER), USER, 101),
            Error::<Test>::InvalidRiskScore
        );
    });
}

// ---- P2-2: update_provider / suspend_provider / resume_provider ----

#[test]
fn p2_update_provider_name_works() {
    new_test_ext().execute_with(|| {
        setup_provider();

        assert_ok!(EntityKyc::update_provider(
            RuntimeOrigin::root(), PROVIDER,
            Some(b"New Name".to_vec()), None,
        ));

        let p = Providers::<Test>::get(PROVIDER).unwrap();
        assert_eq!(p.name.to_vec(), b"New Name".to_vec());
        assert_eq!(p.max_level, KycLevel::Enhanced); // 不变
    });
}

#[test]
fn p2_update_provider_max_level_works() {
    new_test_ext().execute_with(|| {
        setup_provider();

        assert_ok!(EntityKyc::update_provider(
            RuntimeOrigin::root(), PROVIDER,
            None, Some(KycLevel::Institutional),
        ));

        let p = Providers::<Test>::get(PROVIDER).unwrap();
        assert_eq!(p.max_level, KycLevel::Institutional);
    });
}

#[test]
fn p2_update_provider_rejects_empty_name() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_noop!(
            EntityKyc::update_provider(
                RuntimeOrigin::root(), PROVIDER, Some(vec![]), None,
            ),
            Error::<Test>::EmptyProviderName
        );
    });
}

#[test]
fn p2_update_provider_rejects_none_max_level() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_noop!(
            EntityKyc::update_provider(
                RuntimeOrigin::root(), PROVIDER, None, Some(KycLevel::None),
            ),
            Error::<Test>::InvalidKycLevel
        );
    });
}

#[test]
fn p2_suspend_and_resume_provider_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        setup_provider();

        // 暂停
        assert_ok!(EntityKyc::suspend_provider(RuntimeOrigin::root(), PROVIDER));
        assert!(Providers::<Test>::get(PROVIDER).unwrap().suspended);

        System::assert_has_event(RuntimeEvent::EntityKyc(
            crate::Event::ProviderSuspended { provider: PROVIDER }
        ));

        // 暂停后不能审批
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Basic, b"data".to_vec(), *b"CN",
        ));
        assert_noop!(
            EntityKyc::approve_kyc(RuntimeOrigin::signed(PROVIDER), USER, 10),
            Error::<Test>::ProviderIsSuspended
        );

        // 恢复
        assert_ok!(EntityKyc::resume_provider(RuntimeOrigin::root(), PROVIDER));
        assert!(!Providers::<Test>::get(PROVIDER).unwrap().suspended);

        // 恢复后可以审批
        assert_ok!(EntityKyc::approve_kyc(RuntimeOrigin::signed(PROVIDER), USER, 10));
    });
}

#[test]
fn p2_suspend_provider_rejects_already_suspended() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_ok!(EntityKyc::suspend_provider(RuntimeOrigin::root(), PROVIDER));

        assert_noop!(
            EntityKyc::suspend_provider(RuntimeOrigin::root(), PROVIDER),
            Error::<Test>::ProviderIsSuspended
        );
    });
}

#[test]
fn p2_resume_provider_rejects_not_suspended() {
    new_test_ext().execute_with(|| {
        setup_provider();

        assert_noop!(
            EntityKyc::resume_provider(RuntimeOrigin::root(), PROVIDER),
            Error::<Test>::ProviderNotSuspended
        );
    });
}

#[test]
fn p2_suspended_provider_cannot_reject() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"data".to_vec(), *b"CN",
        ));

        assert_ok!(EntityKyc::suspend_provider(RuntimeOrigin::root(), PROVIDER));

        assert_noop!(
            EntityKyc::reject_kyc(
                RuntimeOrigin::signed(PROVIDER), USER, RejectionReason::Other, None,
            ),
            Error::<Test>::ProviderIsSuspended
        );
    });
}

#[test]
fn p2_suspended_provider_cannot_update_risk_score() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);

        assert_ok!(EntityKyc::suspend_provider(RuntimeOrigin::root(), PROVIDER));

        assert_noop!(
            EntityKyc::update_risk_score(RuntimeOrigin::signed(PROVIDER), USER, 50),
            Error::<Test>::ProviderIsSuspended
        );
    });
}

// ---- P2-3: force_approve_kyc ----

#[test]
fn p2_force_approve_kyc_works() {
    new_test_ext().execute_with(|| {
        // 无需 Provider、无需 submit — Admin 直接批准
        assert_ok!(EntityKyc::force_approve_kyc(
            RuntimeOrigin::root(), USER,
            KycLevel::Enhanced, 15, *b"US",
        ));

        let record = KycRecords::<Test>::get(USER).unwrap();
        assert_eq!(record.level, KycLevel::Enhanced);
        assert_eq!(record.status, KycStatus::Approved);
        assert_eq!(record.risk_score, 15);
        assert!(record.provider.is_none()); // force approve 无 provider
        assert!(record.expires_at.is_some());
    });
}

#[test]
fn p2_force_approve_kyc_overwrites_existing() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Basic);

        // Force 覆盖为更高级别
        assert_ok!(EntityKyc::force_approve_kyc(
            RuntimeOrigin::root(), USER,
            KycLevel::Institutional, 5, *b"US",
        ));

        let record = KycRecords::<Test>::get(USER).unwrap();
        assert_eq!(record.level, KycLevel::Institutional);
        assert_eq!(record.risk_score, 5);
    });
}

#[test]
fn p2_force_approve_kyc_rejects_none_level() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityKyc::force_approve_kyc(
                RuntimeOrigin::root(), USER, KycLevel::None, 0, *b"US",
            ),
            Error::<Test>::InvalidKycLevel
        );
    });
}

#[test]
fn p2_force_approve_kyc_rejects_bad_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityKyc::force_approve_kyc(
                RuntimeOrigin::signed(USER), USER, KycLevel::Basic, 0, *b"US",
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn p2_force_approve_kyc_validates_inputs() {
    new_test_ext().execute_with(|| {
        // risk_score > 100
        assert_noop!(
            EntityKyc::force_approve_kyc(
                RuntimeOrigin::root(), USER, KycLevel::Basic, 101, *b"US",
            ),
            Error::<Test>::InvalidRiskScore
        );
        // 无效国家代码
        assert_noop!(
            EntityKyc::force_approve_kyc(
                RuntimeOrigin::root(), USER, KycLevel::Basic, 10, *b"us",
            ),
            Error::<Test>::InvalidCountryCode
        );
    });
}

// ==================== Deep Audit Round 6 回归测试 ====================

// M1-R6: get_risk_score 仅对 Approved 且未过期的记录返回实际评分
#[test]
fn m1_r6_get_risk_score_returns_100_for_pending() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"data".to_vec(), *b"CN",
        ));
        // Pending 状态 — 应返回 100（最高风险），而非记录中的 0
        assert_eq!(Pallet::<Test>::get_risk_score(&USER), 100);
    });
}

#[test]
fn m1_r6_get_risk_score_returns_100_for_rejected() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_ok!(EntityKyc::submit_kyc(
            RuntimeOrigin::signed(USER), KycLevel::Standard, b"data".to_vec(), *b"CN",
        ));
        assert_ok!(EntityKyc::reject_kyc(
            RuntimeOrigin::signed(PROVIDER), USER, RejectionReason::Other, None,
        ));
        // Rejected 状态 — 应返回 100
        assert_eq!(Pallet::<Test>::get_risk_score(&USER), 100);
    });
}

#[test]
fn m1_r6_get_risk_score_returns_100_for_revoked() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);
        assert_eq!(Pallet::<Test>::get_risk_score(&USER), 20); // Approved 时正常

        assert_ok!(EntityKyc::revoke_kyc(
            RuntimeOrigin::root(), USER, RejectionReason::SuspiciousActivity,
        ));
        // Revoked 状态 — 应返回 100，不再返回之前批准时的分数
        assert_eq!(Pallet::<Test>::get_risk_score(&USER), 100);
    });
}

#[test]
fn m1_r6_get_risk_score_returns_100_for_expired() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);
        assert_eq!(Pallet::<Test>::get_risk_score(&USER), 20);

        // StandardKycValidity = 500, 推进到过期后
        System::set_block_number(502);
        // 过期后应返回 100
        assert_eq!(Pallet::<Test>::get_risk_score(&USER), 100);
    });
}

#[test]
fn m1_r6_get_risk_score_returns_actual_for_approved() {
    new_test_ext().execute_with(|| {
        setup_provider();
        submit_and_approve(USER, KycLevel::Standard);
        // Approved 且未过期 — 返回 provider 设置的实际评分
        assert_eq!(Pallet::<Test>::get_risk_score(&USER), 20);
    });
}

// L1-R6: update_provider 拒绝 no-op 调用
#[test]
fn l1_r6_update_provider_rejects_noop() {
    new_test_ext().execute_with(|| {
        setup_provider();
        assert_noop!(
            EntityKyc::update_provider(
                RuntimeOrigin::root(), PROVIDER, None, None,
            ),
            Error::<Test>::NothingToUpdate
        );
    });
}

// L2-R6: set_entity_requirement（Entity Owner 路径）验证 risk_score
#[test]
fn l2_r6_set_entity_requirement_owner_rejects_risk_over_100() {
    new_test_ext().execute_with(|| {
        MockEntityProvider::set_entity_owner(1, ENTITY_OWNER);

        assert_noop!(
            EntityKyc::set_entity_requirement(
                RuntimeOrigin::signed(ENTITY_OWNER), 1,
                KycLevel::Basic, true, 0, true, 101,
            ),
            Error::<Test>::InvalidRiskScore
        );
        // 100 应通过
        assert_ok!(EntityKyc::set_entity_requirement(
            RuntimeOrigin::signed(ENTITY_OWNER), 1,
            KycLevel::Basic, true, 0, true, 100,
        ));
    });
}

// L3-R6: Entity Admin（非 Owner）有 KYC_MANAGE 权限可以设置 KYC 要求
#[test]
fn l3_r6_entity_admin_with_kyc_manage_can_set_requirement() {
    new_test_ext().execute_with(|| {
        MockEntityProvider::set_entity_owner(1, ENTITY_OWNER);
        MockEntityProvider::set_entity_admin(
            1, ENTITY_ADMIN_USER,
            pallet_entity_common::AdminPermission::KYC_MANAGE,
        );

        assert_ok!(EntityKyc::set_entity_requirement(
            RuntimeOrigin::signed(ENTITY_ADMIN_USER), 1,
            KycLevel::Standard, true, 50, false, 80,
        ));

        let req = EntityRequirements::<Test>::get(1).unwrap();
        assert_eq!(req.min_level, KycLevel::Standard);
        assert_eq!(req.max_risk_score, 80);
    });
}

#[test]
fn l3_r6_entity_admin_without_kyc_manage_rejected() {
    new_test_ext().execute_with(|| {
        MockEntityProvider::set_entity_owner(1, ENTITY_OWNER);
        // 仅有 SHOP_MANAGE 权限，没有 KYC_MANAGE
        MockEntityProvider::set_entity_admin(
            1, ENTITY_ADMIN_USER,
            pallet_entity_common::AdminPermission::SHOP_MANAGE,
        );

        assert_noop!(
            EntityKyc::set_entity_requirement(
                RuntimeOrigin::signed(ENTITY_ADMIN_USER), 1,
                KycLevel::Basic, true, 0, true, 100,
            ),
            Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

// L4-R6: update_provider 提供者不存在
#[test]
fn l4_r6_update_provider_fails_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityKyc::update_provider(
                RuntimeOrigin::root(), 99, Some(b"Name".to_vec()), None,
            ),
            Error::<Test>::ProviderNotFound
        );
    });
}

// L5-R6: register_provider 名称过长
#[test]
fn l5_r6_register_provider_rejects_name_too_long() {
    new_test_ext().execute_with(|| {
        // MaxProviderNameLength = 64, 使用 65 字节
        assert_noop!(
            EntityKyc::register_provider(
                RuntimeOrigin::root(), PROVIDER,
                vec![b'A'; 65], ProviderType::Internal, KycLevel::Basic,
            ),
            Error::<Test>::NameTooLong
        );
        // 64 字节应通过
        assert_ok!(EntityKyc::register_provider(
            RuntimeOrigin::root(), PROVIDER,
            vec![b'A'; 64], ProviderType::Internal, KycLevel::Basic,
        ));
    });
}

// ==================== EntityLocked 回归测试 ====================

#[test]
fn entity_locked_rejects_set_entity_requirement() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        MockEntityProvider::set_entity_owner(entity_id, ENTITY_OWNER);
        MockEntityProvider::set_entity_locked(entity_id);
        assert_noop!(
            EntityKyc::set_entity_requirement(
                RuntimeOrigin::signed(ENTITY_OWNER), entity_id,
                KycLevel::Basic, true, 0, true, 100,
            ),
            Error::<Test>::EntityLocked
        );
    });
}
