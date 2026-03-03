//! 财务披露模块测试

use crate::{mock::*, pallet::*};
use frame_support::{assert_noop, assert_ok};

// ==================== configure_disclosure ====================

#[test]
fn configure_disclosure_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER),
            ENTITY_ID,
            DisclosureLevel::Standard,
            true,
            100u64,
        ));

        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.level, DisclosureLevel::Standard);
        assert!(config.insider_trading_control);
        assert_eq!(config.blackout_period_after, 100);
        // next_required = now(1) + StandardInterval(500) = 501
        assert_eq!(config.next_required_disclosure, 501);
        assert_eq!(config.violation_count, 0);
    });
}

#[test]
fn configure_disclosure_fails_not_owner() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::configure_disclosure(
                RuntimeOrigin::signed(ALICE), ENTITY_ID,
                DisclosureLevel::Basic, false, 0u64,
            ),
            Error::<Test>::NotAdmin
        );
    });
}

#[test]
fn configure_disclosure_fails_entity_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::configure_disclosure(
                RuntimeOrigin::signed(OWNER), 999,
                DisclosureLevel::Basic, false, 0u64,
            ),
            Error::<Test>::EntityNotFound
        );
    });
}

// ==================== publish_disclosure ====================

#[test]
fn publish_disclosure_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER),
            ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmTest123".to_vec(),
            Some(b"QmSummary".to_vec()),
        ));

        let disclosure = Disclosures::<Test>::get(0).unwrap();
        assert_eq!(disclosure.entity_id, ENTITY_ID);
        assert_eq!(disclosure.disclosure_type, DisclosureType::AnnualReport);
        assert_eq!(disclosure.status, DisclosureStatus::Published);
        assert_eq!(disclosure.discloser, OWNER);
        assert_eq!(disclosure.previous_id, None);

        // 历史索引已更新
        let history = EntityDisclosures::<Test>::get(ENTITY_ID);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], 0);

        // NextDisclosureId 已递增
        assert_eq!(NextDisclosureId::<Test>::get(), 1);
    });
}

#[test]
fn publish_disclosure_auto_blackout() {
    new_test_ext().execute_with(|| {
        // 先配置：启用内幕控制 + blackout_after=50
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 50u64,
        ));

        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::QuarterlyReport, b"QmReport".to_vec(), None,
        ));

        // 应自动进入黑窗口期
        assert!(Pallet::<Test>::is_in_blackout(ENTITY_ID));
        let (start, end) = BlackoutPeriods::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(start, 1);
        assert_eq!(end, 51);
    });
}

#[test]
fn publish_disclosure_no_blackout_when_disabled() {
    new_test_ext().execute_with(|| {
        // 配置：不启用内幕控制
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Basic, false, 50u64,
        ));

        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport, b"QmReport".to_vec(), None,
        ));

        // 不应进入黑窗口
        assert!(!Pallet::<Test>::is_in_blackout(ENTITY_ID));
    });
}

#[test]
fn publish_disclosure_fails_not_owner() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::publish_disclosure(
                RuntimeOrigin::signed(ALICE), ENTITY_ID,
                DisclosureType::AnnualReport, b"Qm".to_vec(), None,
            ),
            Error::<Test>::NotAdmin
        );
    });
}

#[test]
fn publish_disclosure_fails_cid_too_long() {
    new_test_ext().execute_with(|| {
        let long_cid = vec![0u8; 65]; // MaxCidLength = 64
        assert_noop!(
            EntityDisclosure::publish_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureType::AnnualReport, long_cid, None,
            ),
            Error::<Test>::CidTooLong
        );
    });
}

#[test]
fn publish_disclosure_fails_history_full() {
    new_test_ext().execute_with(|| {
        // MaxDisclosureHistory = 10
        for _ in 0..10 {
            assert_ok!(EntityDisclosure::publish_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureType::Other, b"QmCid".to_vec(), None,
            ));
        }
        // 11th should fail
        assert_noop!(
            EntityDisclosure::publish_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureType::Other, b"QmCid".to_vec(), None,
            ),
            Error::<Test>::HistoryFull
        );
    });
}

// ==================== withdraw_disclosure ====================

#[test]
fn withdraw_disclosure_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport, b"QmCid".to_vec(), None,
        ));

        assert_ok!(EntityDisclosure::withdraw_disclosure(
            RuntimeOrigin::signed(OWNER), 0,
        ));

        let record = Disclosures::<Test>::get(0).unwrap();
        assert_eq!(record.status, DisclosureStatus::Withdrawn);
    });
}

#[test]
fn withdraw_disclosure_fails_not_published() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport, b"QmCid".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::withdraw_disclosure(RuntimeOrigin::signed(OWNER), 0));

        // 再次撤回已撤回的应失败
        assert_noop!(
            EntityDisclosure::withdraw_disclosure(RuntimeOrigin::signed(OWNER), 0),
            Error::<Test>::InvalidDisclosureStatus
        );
    });
}

#[test]
fn withdraw_disclosure_fails_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::withdraw_disclosure(RuntimeOrigin::signed(OWNER), 999),
            Error::<Test>::DisclosureNotFound
        );
    });
}

// ==================== correct_disclosure ====================

#[test]
fn correct_disclosure_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport, b"QmOld".to_vec(), None,
        ));

        assert_ok!(EntityDisclosure::correct_disclosure(
            RuntimeOrigin::signed(OWNER), 0,
            b"QmNew".to_vec(), None,
        ));

        // 旧记录标记为 Corrected
        let old = Disclosures::<Test>::get(0).unwrap();
        assert_eq!(old.status, DisclosureStatus::Corrected);

        // 新记录指向旧记录
        let new_rec = Disclosures::<Test>::get(1).unwrap();
        assert_eq!(new_rec.status, DisclosureStatus::Published);
        assert_eq!(new_rec.previous_id, Some(0));
    });
}

#[test]
fn correct_disclosure_fails_withdrawn() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport, b"QmOld".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::withdraw_disclosure(RuntimeOrigin::signed(OWNER), 0));

        // H2: 不能更正已撤回的记录
        assert_noop!(
            EntityDisclosure::correct_disclosure(
                RuntimeOrigin::signed(OWNER), 0, b"QmNew".to_vec(), None,
            ),
            Error::<Test>::InvalidDisclosureStatus
        );
    });
}

// ==================== add_insider / remove_insider ====================

#[test]
fn add_insider_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert!(Pallet::<Test>::is_insider(ENTITY_ID, &ALICE));
    });
}

#[test]
fn add_insider_fails_duplicate() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_noop!(
            EntityDisclosure::add_insider(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Auditor,
            ),
            Error::<Test>::InsiderExists
        );
    });
}

#[test]
fn add_insider_after_remove_works() {
    new_test_ext().execute_with(|| {
        // M3: 硬删除后重新添加，BoundedVec 长度先减再增
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_ok!(EntityDisclosure::remove_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE,
        ));
        assert!(!Pallet::<Test>::is_insider(ENTITY_ID, &ALICE));
        assert_eq!(Insiders::<Test>::get(ENTITY_ID).len(), 0);

        // 重新添加（不同角色）
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Auditor,
        ));
        assert!(Pallet::<Test>::is_insider(ENTITY_ID, &ALICE));
        assert_eq!(Insiders::<Test>::get(ENTITY_ID).len(), 1);
    });
}

#[test]
fn add_insider_fails_full() {
    new_test_ext().execute_with(|| {
        // MaxInsiders = 5
        for i in 10..15 {
            assert_ok!(EntityDisclosure::add_insider(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, i, InsiderRole::Advisor,
            ));
        }
        assert_noop!(
            EntityDisclosure::add_insider(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, 99, InsiderRole::Advisor,
            ),
            Error::<Test>::InsidersFull
        );
    });
}

#[test]
fn remove_insider_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_ok!(EntityDisclosure::remove_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE,
        ));
        assert!(!Pallet::<Test>::is_insider(ENTITY_ID, &ALICE));
    });
}

#[test]
fn remove_insider_fails_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::remove_insider(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE,
            ),
            Error::<Test>::InsiderNotFound
        );
    });
}

// ==================== blackout ====================

#[test]
fn blackout_period_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 100u64,
        ));
        assert!(Pallet::<Test>::is_in_blackout(ENTITY_ID));

        assert_ok!(EntityDisclosure::end_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
        ));
        assert!(!Pallet::<Test>::is_in_blackout(ENTITY_ID));
    });
}

#[test]
fn blackout_expires_naturally() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 50u64,
        ));
        assert!(Pallet::<Test>::is_in_blackout(ENTITY_ID));

        advance_blocks(51);
        assert!(!Pallet::<Test>::is_in_blackout(ENTITY_ID));
    });
}

// ==================== can_insider_trade ====================

#[test]
fn can_insider_trade_non_insider_always_true() {
    new_test_ext().execute_with(|| {
        // 非内幕人员始终可交易（即使在黑窗口期）
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 100u64,
        ));
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 100u64,
        ));
        assert!(Pallet::<Test>::can_insider_trade(ENTITY_ID, &ALICE));
    });
}

#[test]
fn can_insider_trade_blocked_during_blackout() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 100u64,
        ));
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 100u64,
        ));

        // 内幕人员在黑窗口期内不可交易
        assert!(!Pallet::<Test>::can_insider_trade(ENTITY_ID, &ALICE));

        // 黑窗口结束后可交易
        advance_blocks(101);
        assert!(Pallet::<Test>::can_insider_trade(ENTITY_ID, &ALICE));
    });
}

#[test]
fn can_insider_trade_allowed_when_control_disabled() {
    new_test_ext().execute_with(|| {
        // 未启用内幕控制，即使在黑窗口期也可交易
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 100u64,
        ));

        assert!(Pallet::<Test>::can_insider_trade(ENTITY_ID, &ALICE));
    });
}

// ==================== helper 函数 ====================

#[test]
fn calculate_next_disclosure_intervals() {
    new_test_ext().execute_with(|| {
        let now = 100u64;
        assert_eq!(Pallet::<Test>::calculate_next_disclosure(DisclosureLevel::Basic, now), 1100);
        assert_eq!(Pallet::<Test>::calculate_next_disclosure(DisclosureLevel::Standard, now), 600);
        assert_eq!(Pallet::<Test>::calculate_next_disclosure(DisclosureLevel::Enhanced, now), 200);
        assert_eq!(Pallet::<Test>::calculate_next_disclosure(DisclosureLevel::Full, now), 100); // +0
    });
}

#[test]
fn is_disclosure_overdue_works() {
    new_test_ext().execute_with(|| {
        // 无配置时不逾期
        assert!(!Pallet::<Test>::is_disclosure_overdue(ENTITY_ID));

        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Enhanced, false, 0u64,
        ));
        // 刚配置，next_required = 1+100=101
        assert!(!Pallet::<Test>::is_disclosure_overdue(ENTITY_ID));

        advance_blocks(101);
        assert!(Pallet::<Test>::is_disclosure_overdue(ENTITY_ID));
    });
}

#[test]
fn get_disclosure_level_default() {
    new_test_ext().execute_with(|| {
        assert_eq!(Pallet::<Test>::get_disclosure_level(ENTITY_ID), DisclosureLevel::Basic);
    });
}

// ==================== 审计回归测试 ====================

#[test]
fn h1_configure_disclosure_rejects_blackout_exceeds_max() {
    new_test_ext().execute_with(|| {
        // MaxBlackoutDuration = 200
        assert_noop!(
            EntityDisclosure::configure_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureLevel::Standard, true, 201u64,
            ),
            Error::<Test>::BlackoutExceedsMax
        );
        // 恰好等于上限应成功
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 200u64,
        ));
    });
}

#[test]
fn h2_publish_disclosure_rejects_empty_cid() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::publish_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureType::AnnualReport, vec![], None,
            ),
            Error::<Test>::EmptyCid
        );
    });
}

#[test]
fn h2_correct_disclosure_rejects_empty_cid() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport, b"QmOld".to_vec(), None,
        ));
        assert_noop!(
            EntityDisclosure::correct_disclosure(
                RuntimeOrigin::signed(OWNER), 0, vec![], None,
            ),
            Error::<Test>::EmptyCid
        );
    });
}

#[test]
fn h3_correct_disclosure_triggers_blackout() {
    new_test_ext().execute_with(|| {
        // 配置：启用内幕控制 + blackout_after=50
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 50u64,
        ));

        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport, b"QmOld".to_vec(), None,
        ));

        // publish 触发了黑窗口，先结束它
        assert_ok!(EntityDisclosure::end_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
        ));
        assert!(!Pallet::<Test>::is_in_blackout(ENTITY_ID));

        // 更正披露也应触发黑窗口
        assert_ok!(EntityDisclosure::correct_disclosure(
            RuntimeOrigin::signed(OWNER), 0,
            b"QmNew".to_vec(), None,
        ));
        assert!(Pallet::<Test>::is_in_blackout(ENTITY_ID));
    });
}

#[test]
fn h5_configure_preserves_last_disclosure() {
    new_test_ext().execute_with(|| {
        // 首次配置：last_disclosure 应为 0（无历史记录）
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.last_disclosure, 0);

        // 发布一个披露
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport, b"QmCid".to_vec(), None,
        ));
        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.last_disclosure, 1); // block 1

        // 重新配置不应重置 last_disclosure
        advance_blocks(10);
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Enhanced, false, 0u64,
        ));
        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.last_disclosure, 1); // 保持不变，不被伪造为 now(11)
        assert_eq!(config.level, DisclosureLevel::Enhanced);
    });
}

#[test]
fn l1_end_blackout_fails_when_none_exists() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::end_blackout(RuntimeOrigin::signed(OWNER), ENTITY_ID),
            Error::<Test>::BlackoutNotFound
        );
    });
}

#[test]
fn m3_start_blackout_respects_config_max() {
    new_test_ext().execute_with(|| {
        // MaxBlackoutDuration = 200
        assert_noop!(
            EntityDisclosure::start_blackout(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, 201u64,
            ),
            Error::<Test>::InvalidBlackoutDuration
        );
        // 零时长也不允许
        assert_noop!(
            EntityDisclosure::start_blackout(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, 0u64,
            ),
            Error::<Test>::InvalidBlackoutDuration
        );
        // 恰好 200 应成功
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 200u64,
        ));
    });
}

#[test]
fn l2_disclosure_type_has_default() {
    // DisclosureType 应有 Default trait
    let dt: DisclosureType = Default::default();
    assert_eq!(dt, DisclosureType::AnnualReport);
}

#[test]
fn h5_configure_preserves_violation_count() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        // 手动修改 violation_count 模拟累积
        DisclosureConfigs::<Test>::mutate(ENTITY_ID, |c| {
            if let Some(config) = c {
                config.violation_count = 3;
            }
        });

        // 重新配置不应清除 violation_count
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Enhanced, true, 50u64,
        ));
        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.violation_count, 3);
    });
}

// ==================== publish_announcement ====================

#[test]
fn publish_announcement_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER),
            ENTITY_ID,
            AnnouncementCategory::General,
            b"Hello World".to_vec(),
            b"QmAnnouncement1".to_vec(),
            None,
        ));

        let record = Announcements::<Test>::get(0).unwrap();
        assert_eq!(record.entity_id, ENTITY_ID);
        assert_eq!(record.category, AnnouncementCategory::General);
        assert_eq!(record.title.to_vec(), b"Hello World".to_vec());
        assert_eq!(record.content_cid.to_vec(), b"QmAnnouncement1".to_vec());
        assert_eq!(record.publisher, OWNER);
        assert_eq!(record.status, AnnouncementStatus::Active);
        assert_eq!(record.is_pinned, false);
        assert_eq!(record.expires_at, None);

        let history = EntityAnnouncements::<Test>::get(ENTITY_ID);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], 0);

        assert_eq!(NextAnnouncementId::<Test>::get(), 1);
    });
}

#[test]
fn publish_announcement_with_expiry() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER),
            ENTITY_ID,
            AnnouncementCategory::Promotion,
            b"Sale".to_vec(),
            b"QmSale".to_vec(),
            Some(100u64),
        ));

        let record = Announcements::<Test>::get(0).unwrap();
        assert_eq!(record.expires_at, Some(100));
        assert_eq!(record.category, AnnouncementCategory::Promotion);
    });
}

#[test]
fn publish_announcement_fails_not_owner() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::publish_announcement(
                RuntimeOrigin::signed(ALICE), ENTITY_ID,
                AnnouncementCategory::General,
                b"Title".to_vec(), b"QmCid".to_vec(), None,
            ),
            Error::<Test>::NotAdmin
        );
    });
}

#[test]
fn publish_announcement_fails_entity_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::publish_announcement(
                RuntimeOrigin::signed(OWNER), 999,
                AnnouncementCategory::General,
                b"Title".to_vec(), b"QmCid".to_vec(), None,
            ),
            Error::<Test>::EntityNotFound
        );
    });
}

#[test]
fn publish_announcement_fails_empty_title() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::publish_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                AnnouncementCategory::General,
                vec![], b"QmCid".to_vec(), None,
            ),
            Error::<Test>::EmptyTitle
        );
    });
}

#[test]
fn publish_announcement_fails_title_too_long() {
    new_test_ext().execute_with(|| {
        let long_title = vec![0u8; 129]; // MaxTitleLength = 128
        assert_noop!(
            EntityDisclosure::publish_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                AnnouncementCategory::General,
                long_title, b"QmCid".to_vec(), None,
            ),
            Error::<Test>::TitleTooLong
        );
    });
}

#[test]
fn publish_announcement_fails_empty_cid() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::publish_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                AnnouncementCategory::General,
                b"Title".to_vec(), vec![], None,
            ),
            Error::<Test>::EmptyCid
        );
    });
}

#[test]
fn publish_announcement_fails_cid_too_long() {
    new_test_ext().execute_with(|| {
        let long_cid = vec![0u8; 65]; // MaxCidLength = 64
        assert_noop!(
            EntityDisclosure::publish_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                AnnouncementCategory::General,
                b"Title".to_vec(), long_cid, None,
            ),
            Error::<Test>::CidTooLong
        );
    });
}

#[test]
fn publish_announcement_fails_invalid_expiry() {
    new_test_ext().execute_with(|| {
        // block_number = 1, expires_at = 1 (not > now)
        assert_noop!(
            EntityDisclosure::publish_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                AnnouncementCategory::General,
                b"Title".to_vec(), b"QmCid".to_vec(), Some(1u64),
            ),
            Error::<Test>::InvalidExpiry
        );
    });
}

#[test]
fn publish_announcement_fails_history_full() {
    new_test_ext().execute_with(|| {
        // MaxAnnouncementHistory = 10
        for i in 0..10 {
            assert_ok!(EntityDisclosure::publish_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                AnnouncementCategory::General,
                format!("Title {}", i).into_bytes(),
                b"QmCid".to_vec(), None,
            ));
        }
        assert_noop!(
            EntityDisclosure::publish_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                AnnouncementCategory::General,
                b"Title 11".to_vec(), b"QmCid".to_vec(), None,
            ),
            Error::<Test>::AnnouncementHistoryFull
        );
    });
}

// ==================== update_announcement ====================

#[test]
fn update_announcement_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Old Title".to_vec(), b"QmOld".to_vec(), None,
        ));

        assert_ok!(EntityDisclosure::update_announcement(
            RuntimeOrigin::signed(OWNER), 0,
            Some(b"New Title".to_vec()),
            Some(b"QmNew".to_vec()),
            Some(AnnouncementCategory::SystemUpdate),
            Some(Some(200u64)),
        ));

        let record = Announcements::<Test>::get(0).unwrap();
        assert_eq!(record.title.to_vec(), b"New Title".to_vec());
        assert_eq!(record.content_cid.to_vec(), b"QmNew".to_vec());
        assert_eq!(record.category, AnnouncementCategory::SystemUpdate);
        assert_eq!(record.expires_at, Some(200));
    });
}

#[test]
fn update_announcement_partial_update() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), Some(100u64),
        ));

        // 只更新标题
        assert_ok!(EntityDisclosure::update_announcement(
            RuntimeOrigin::signed(OWNER), 0,
            Some(b"Updated Title".to_vec()),
            None, None, None,
        ));

        let record = Announcements::<Test>::get(0).unwrap();
        assert_eq!(record.title.to_vec(), b"Updated Title".to_vec());
        assert_eq!(record.content_cid.to_vec(), b"QmCid".to_vec());
        assert_eq!(record.category, AnnouncementCategory::General);
        assert_eq!(record.expires_at, Some(100));
    });
}

#[test]
fn update_announcement_clear_expiry() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), Some(100u64),
        ));

        // 清除过期时间 → 永不过期
        assert_ok!(EntityDisclosure::update_announcement(
            RuntimeOrigin::signed(OWNER), 0,
            None, None, None, Some(None),
        ));

        let record = Announcements::<Test>::get(0).unwrap();
        assert_eq!(record.expires_at, None);
    });
}

#[test]
fn update_announcement_fails_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::update_announcement(
                RuntimeOrigin::signed(OWNER), 999,
                Some(b"Title".to_vec()), None, None, None,
            ),
            Error::<Test>::AnnouncementNotFound
        );
    });
}

#[test]
fn update_announcement_fails_not_owner() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));

        assert_noop!(
            EntityDisclosure::update_announcement(
                RuntimeOrigin::signed(ALICE), 0,
                Some(b"New".to_vec()), None, None, None,
            ),
            Error::<Test>::NotAdmin
        );
    });
}

#[test]
fn update_announcement_fails_withdrawn() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::withdraw_announcement(RuntimeOrigin::signed(OWNER), 0));

        assert_noop!(
            EntityDisclosure::update_announcement(
                RuntimeOrigin::signed(OWNER), 0,
                Some(b"New".to_vec()), None, None, None,
            ),
            Error::<Test>::AnnouncementNotActive
        );
    });
}

#[test]
fn update_announcement_fails_empty_title() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));

        assert_noop!(
            EntityDisclosure::update_announcement(
                RuntimeOrigin::signed(OWNER), 0,
                Some(vec![]), None, None, None,
            ),
            Error::<Test>::EmptyTitle
        );
    });
}

#[test]
fn update_announcement_fails_invalid_expiry() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));

        // block_number = 1, expires_at = 1 (not > now)
        assert_noop!(
            EntityDisclosure::update_announcement(
                RuntimeOrigin::signed(OWNER), 0,
                None, None, None, Some(Some(1u64)),
            ),
            Error::<Test>::InvalidExpiry
        );
    });
}

// ==================== withdraw_announcement ====================

#[test]
fn withdraw_announcement_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));

        assert_ok!(EntityDisclosure::withdraw_announcement(
            RuntimeOrigin::signed(OWNER), 0,
        ));

        let record = Announcements::<Test>::get(0).unwrap();
        assert_eq!(record.status, AnnouncementStatus::Withdrawn);
    });
}

#[test]
fn withdraw_announcement_clears_pinned() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::pin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, Some(0),
        ));
        assert_eq!(PinnedAnnouncement::<Test>::get(ENTITY_ID), Some(0));

        // 撤回应清除置顶
        assert_ok!(EntityDisclosure::withdraw_announcement(
            RuntimeOrigin::signed(OWNER), 0,
        ));
        assert_eq!(PinnedAnnouncement::<Test>::get(ENTITY_ID), None);
        assert_eq!(Announcements::<Test>::get(0).unwrap().is_pinned, false);
    });
}

#[test]
fn withdraw_announcement_fails_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::withdraw_announcement(RuntimeOrigin::signed(OWNER), 999),
            Error::<Test>::AnnouncementNotFound
        );
    });
}

#[test]
fn withdraw_announcement_fails_already_withdrawn() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::withdraw_announcement(RuntimeOrigin::signed(OWNER), 0));

        assert_noop!(
            EntityDisclosure::withdraw_announcement(RuntimeOrigin::signed(OWNER), 0),
            Error::<Test>::AnnouncementNotActive
        );
    });
}

#[test]
fn withdraw_announcement_fails_not_admin() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));

        assert_noop!(
            EntityDisclosure::withdraw_announcement(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::NotAdmin
        );
    });
}

// ==================== pin_announcement ====================

#[test]
fn pin_announcement_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));

        assert_ok!(EntityDisclosure::pin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, Some(0),
        ));

        let record = Announcements::<Test>::get(0).unwrap();
        assert!(record.is_pinned);
        assert_eq!(PinnedAnnouncement::<Test>::get(ENTITY_ID), Some(0));
    });
}

#[test]
fn pin_announcement_replaces_old_pin() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"First".to_vec(), b"QmFirst".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Second".to_vec(), b"QmSecond".to_vec(), None,
        ));

        // 置顶第一个
        assert_ok!(EntityDisclosure::pin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, Some(0),
        ));
        assert!(Announcements::<Test>::get(0).unwrap().is_pinned);

        // 置顶第二个 — 自动替换
        assert_ok!(EntityDisclosure::pin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, Some(1),
        ));

        assert!(!Announcements::<Test>::get(0).unwrap().is_pinned);
        assert!(Announcements::<Test>::get(1).unwrap().is_pinned);
        assert_eq!(PinnedAnnouncement::<Test>::get(ENTITY_ID), Some(1));
    });
}

#[test]
fn unpin_announcement_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::pin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, Some(0),
        ));

        // 取消置顶
        assert_ok!(EntityDisclosure::pin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, None,
        ));

        assert!(!Announcements::<Test>::get(0).unwrap().is_pinned);
        assert_eq!(PinnedAnnouncement::<Test>::get(ENTITY_ID), None);
    });
}

#[test]
fn pin_announcement_fails_not_owner() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));

        assert_noop!(
            EntityDisclosure::pin_announcement(
                RuntimeOrigin::signed(ALICE), ENTITY_ID, Some(0),
            ),
            Error::<Test>::NotAdmin
        );
    });
}

#[test]
fn pin_announcement_fails_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::pin_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, Some(999),
            ),
            Error::<Test>::AnnouncementNotFound
        );
    });
}

#[test]
fn pin_announcement_fails_wrong_entity() {
    new_test_ext().execute_with(|| {
        // 公告属于 ENTITY_ID
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));

        // 尝试在 ENTITY_ID_2 下置顶
        assert_noop!(
            EntityDisclosure::pin_announcement(
                RuntimeOrigin::signed(OWNER_2), ENTITY_ID_2, Some(0),
            ),
            Error::<Test>::AnnouncementNotFound
        );
    });
}

#[test]
fn pin_announcement_fails_withdrawn() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::withdraw_announcement(RuntimeOrigin::signed(OWNER), 0));

        assert_noop!(
            EntityDisclosure::pin_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, Some(0),
            ),
            Error::<Test>::AnnouncementNotActive
        );
    });
}

// ==================== 公告 helper 函数 ====================

#[test]
fn is_announcement_expired_works() {
    new_test_ext().execute_with(|| {
        // 不过期公告
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"No Expiry".to_vec(), b"QmCid".to_vec(), None,
        ));
        assert!(!Pallet::<Test>::is_announcement_expired(0));

        // 有过期时间公告
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::Promotion,
            b"Expires Soon".to_vec(), b"QmCid2".to_vec(), Some(50u64),
        ));
        assert!(!Pallet::<Test>::is_announcement_expired(1));

        // 过期后
        advance_blocks(50);
        assert!(Pallet::<Test>::is_announcement_expired(1));
    });
}

#[test]
fn get_pinned_announcement_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(Pallet::<Test>::get_pinned_announcement(ENTITY_ID), None);

        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::pin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, Some(0),
        ));

        assert_eq!(Pallet::<Test>::get_pinned_announcement(ENTITY_ID), Some(0));
    });
}

#[test]
fn announcement_category_default() {
    let cat: AnnouncementCategory = Default::default();
    assert_eq!(cat, AnnouncementCategory::General);
}

#[test]
fn announcement_status_default() {
    let status: AnnouncementStatus = Default::default();
    assert_eq!(status, AnnouncementStatus::Active);
}

// ==================== 审计回归测试 ====================

#[test]
fn h1_disclosure_id_overflow_rejected() {
    new_test_ext().execute_with(|| {
        // 将 NextDisclosureId 设为 u64::MAX
        NextDisclosureId::<Test>::put(u64::MAX);

        assert_noop!(
            EntityDisclosure::publish_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureType::AnnualReport,
                b"QmContent".to_vec(), None,
            ),
            Error::<Test>::IdOverflow
        );
    });
}

#[test]
fn h1_announcement_id_overflow_rejected() {
    new_test_ext().execute_with(|| {
        NextAnnouncementId::<Test>::put(u64::MAX);

        assert_noop!(
            EntityDisclosure::publish_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                AnnouncementCategory::General,
                b"Title".to_vec(), b"QmCid".to_vec(), None,
            ),
            Error::<Test>::IdOverflow
        );
    });
}

#[test]
fn h2_admin_can_publish_disclosure() {
    new_test_ext().execute_with(|| {
        // ADMIN 不是 owner，但设为管理员
        set_admin(ENTITY_ID, ADMIN);

        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(ADMIN), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmAdminContent".to_vec(), None,
        ));

        let record = Disclosures::<Test>::get(0).unwrap();
        assert_eq!(record.discloser, ADMIN);
    });
}

#[test]
fn h2_admin_can_publish_announcement() {
    new_test_ext().execute_with(|| {
        set_admin(ENTITY_ID, ADMIN);

        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(ADMIN), ENTITY_ID,
            AnnouncementCategory::General,
            b"Admin Title".to_vec(), b"QmCid".to_vec(), None,
        ));

        let record = Announcements::<Test>::get(0).unwrap();
        assert_eq!(record.publisher, ADMIN);
    });
}

#[test]
fn h2_non_admin_rejected() {
    new_test_ext().execute_with(|| {
        // ALICE 既不是 owner 也不是 admin
        assert_noop!(
            EntityDisclosure::publish_disclosure(
                RuntimeOrigin::signed(ALICE), ENTITY_ID,
                DisclosureType::AnnualReport,
                b"QmContent".to_vec(), None,
            ),
            Error::<Test>::NotAdmin
        );
    });
}

#[test]
fn m1_pin_announcement_rejects_expired() {
    new_test_ext().execute_with(|| {
        // 发布一个即将过期的公告
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::Promotion,
            b"Expiring".to_vec(), b"QmCid".to_vec(), Some(10u64),
        ));

        // 过期后尝试置顶
        advance_blocks(10);
        assert_noop!(
            EntityDisclosure::pin_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, Some(0),
            ),
            Error::<Test>::AnnouncementExpired
        );
    });
}

#[test]
fn m2_update_announcement_rejects_all_none() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));

        // 所有参数均为 None — 应拒绝
        assert_noop!(
            EntityDisclosure::update_announcement(
                RuntimeOrigin::signed(OWNER), 0,
                None, None, None, None,
            ),
            Error::<Test>::NoUpdateProvided
        );
    });
}

#[test]
fn m3_publish_disclosure_rejects_empty_summary_cid() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::publish_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureType::AnnualReport,
                b"QmContent".to_vec(),
                Some(vec![]),  // 空 summary_cid
            ),
            Error::<Test>::EmptyCid
        );
    });
}

#[test]
fn m3_correct_disclosure_rejects_empty_summary_cid() {
    new_test_ext().execute_with(|| {
        // 先发布一个披露
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmOriginal".to_vec(), None,
        ));

        // 更正时提供空 summary_cid
        assert_noop!(
            EntityDisclosure::correct_disclosure(
                RuntimeOrigin::signed(OWNER), 0,
                b"QmCorrected".to_vec(),
                Some(vec![]),
            ),
            Error::<Test>::EmptyCid
        );
    });
}

// ==================== Deep Audit Round 回归测试 ====================

// M1: update_announcement 不允许更新已过期公告
#[test]
fn m1_deep_update_announcement_rejects_expired() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::Promotion,
            b"Sale".to_vec(), b"QmSale".to_vec(), Some(10u64),
        ));

        // 过期后尝试更新
        advance_blocks(10);
        assert_noop!(
            EntityDisclosure::update_announcement(
                RuntimeOrigin::signed(OWNER), 0,
                Some(b"New Title".to_vec()), None, None, None,
            ),
            Error::<Test>::AnnouncementExpired
        );
    });
}

// M1: 未过期公告仍可正常更新
#[test]
fn m1_deep_update_announcement_works_before_expiry() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::Promotion,
            b"Sale".to_vec(), b"QmSale".to_vec(), Some(100u64),
        ));

        // 过期前更新应成功
        assert_ok!(EntityDisclosure::update_announcement(
            RuntimeOrigin::signed(OWNER), 0,
            Some(b"Updated Sale".to_vec()), None, None, None,
        ));
        assert_eq!(Announcements::<Test>::get(0).unwrap().title.to_vec(), b"Updated Sale".to_vec());
    });
}

// M2: expire_announcement 标记已过期公告
#[test]
fn m2_deep_expire_announcement_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::Promotion,
            b"Promo".to_vec(), b"QmPromo".to_vec(), Some(10u64),
        ));

        // 过期后调用 expire_announcement
        advance_blocks(10);
        assert_ok!(EntityDisclosure::expire_announcement(
            RuntimeOrigin::signed(ALICE), 0,  // 任何人可调用
        ));

        let record = Announcements::<Test>::get(0).unwrap();
        assert_eq!(record.status, AnnouncementStatus::Expired);
        assert!(!record.is_pinned);
    });
}

// M2: expire_announcement 清除置顶
#[test]
fn m2_deep_expire_announcement_clears_pin() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::Promotion,
            b"Pinned Promo".to_vec(), b"QmPin".to_vec(), Some(10u64),
        ));
        assert_ok!(EntityDisclosure::pin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, Some(0),
        ));
        assert_eq!(PinnedAnnouncement::<Test>::get(ENTITY_ID), Some(0));

        advance_blocks(10);
        assert_ok!(EntityDisclosure::expire_announcement(
            RuntimeOrigin::signed(ALICE), 0,
        ));

        assert_eq!(PinnedAnnouncement::<Test>::get(ENTITY_ID), None);
        assert_eq!(Announcements::<Test>::get(0).unwrap().status, AnnouncementStatus::Expired);
    });
}

// M2: expire_announcement 拒绝未过期公告
#[test]
fn m2_deep_expire_announcement_rejects_not_expired() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), Some(100u64),
        ));

        // 未过期
        assert_noop!(
            EntityDisclosure::expire_announcement(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::AnnouncementNotExpired
        );
    });
}

// M2: expire_announcement 拒绝无过期时间公告
#[test]
fn m2_deep_expire_announcement_rejects_no_expiry() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));

        assert_noop!(
            EntityDisclosure::expire_announcement(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::AnnouncementNotExpired
        );
    });
}

// M2: expire_announcement 拒绝非 Active 公告
#[test]
fn m2_deep_expire_announcement_rejects_non_active() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::Promotion,
            b"Title".to_vec(), b"QmCid".to_vec(), Some(10u64),
        ));
        // 先撤回
        assert_ok!(EntityDisclosure::withdraw_announcement(RuntimeOrigin::signed(OWNER), 0));

        advance_blocks(10);
        assert_noop!(
            EntityDisclosure::expire_announcement(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::AnnouncementNotActive
        );
    });
}

// M3: 硬删除后容量释放 — 移除全部后可重新添加到满
#[test]
fn m3_deep_remove_insider_frees_capacity() {
    new_test_ext().execute_with(|| {
        // MaxInsiders = 5，先填满
        for i in 10..15 {
            assert_ok!(EntityDisclosure::add_insider(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, i, InsiderRole::Advisor,
            ));
        }
        // 已满
        assert_noop!(
            EntityDisclosure::add_insider(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, 99, InsiderRole::Advisor,
            ),
            Error::<Test>::InsidersFull
        );

        // 移除全部
        for i in 10..15 {
            assert_ok!(EntityDisclosure::remove_insider(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, i,
            ));
        }
        assert_eq!(Insiders::<Test>::get(ENTITY_ID).len(), 0);

        // 重新添加 5 个全新 insider
        for i in 20..25 {
            assert_ok!(EntityDisclosure::add_insider(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, i, InsiderRole::Admin,
            ));
        }
        assert_eq!(Insiders::<Test>::get(ENTITY_ID).len(), 5);
    });
}

// M3: 硬删除后 is_insider 立即返回 false
#[test]
fn m3_deep_remove_insider_immediate_effect() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert!(Pallet::<Test>::is_insider(ENTITY_ID, &ALICE));

        assert_ok!(EntityDisclosure::remove_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE,
        ));
        assert!(!Pallet::<Test>::is_insider(ENTITY_ID, &ALICE));

        // 再次移除应失败
        assert_noop!(
            EntityDisclosure::remove_insider(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE,
            ),
            Error::<Test>::InsiderNotFound
        );
    });
}

// ==================== Round 2 审计回归测试 ====================

#[test]
fn m1r2_start_blackout_cannot_shorten_active() {
    new_test_ext().execute_with(|| {
        // 先启动一个 150 区块的黑窗口期
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 150u64,
        ));

        // 尝试用更短的 50 区块覆盖 — 应被拒绝
        assert_noop!(
            EntityDisclosure::start_blackout(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, 50u64,
            ),
            Error::<Test>::BlackoutStillActive
        );
    });
}

#[test]
fn m1r2_start_blackout_can_extend_active() {
    new_test_ext().execute_with(|| {
        // 先启动 100 区块的黑窗口期
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 100u64,
        ));

        // 延长到 200 区块 — 应允许
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 200u64,
        ));

        let (_, end) = BlackoutPeriods::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(end, 1 + 200); // now(1) + 200
    });
}

#[test]
fn m1r2_start_blackout_ok_after_expired() {
    new_test_ext().execute_with(|| {
        // 先启动 50 区块的黑窗口期
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 50u64,
        ));

        // 黑窗口期过期后，可以启动更短的新黑窗口期
        advance_blocks(60);
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 10u64,
        ));
    });
}

#[test]
fn m2r2_update_announcement_rejects_expired() {
    new_test_ext().execute_with(|| {
        // 发布一个即将过期的公告
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), Some(10u64),
        ));

        // 过期后尝试更新
        advance_blocks(10);
        assert_noop!(
            EntityDisclosure::update_announcement(
                RuntimeOrigin::signed(OWNER), 0,
                Some(b"NewTitle".to_vec()), None, None, None,
            ),
            Error::<Test>::AnnouncementExpired
        );
    });
}

#[test]
fn m3r2_get_pinned_announcement_filters_expired() {
    new_test_ext().execute_with(|| {
        // 发布一个有过期时间的公告并置顶
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::Promotion,
            b"Pinned".to_vec(), b"QmCid".to_vec(), Some(20u64),
        ));
        assert_ok!(EntityDisclosure::pin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, Some(0),
        ));

        // 过期前 — 应返回 Some
        assert_eq!(EntityDisclosure::get_pinned_announcement(ENTITY_ID), Some(0));

        // 过期后 — 应返回 None
        advance_blocks(20);
        assert_eq!(EntityDisclosure::get_pinned_announcement(ENTITY_ID), None);
    });
}

#[test]
fn m3r2_get_pinned_announcement_filters_withdrawn() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::pin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, Some(0),
        ));

        // 撤回会清除 PinnedAnnouncement，但即使存储残留也应过滤
        // 这里先测正常 withdraw 路径：withdraw 已正确清除 PinnedAnnouncement
        assert_ok!(EntityDisclosure::withdraw_announcement(
            RuntimeOrigin::signed(OWNER), 0,
        ));
        assert_eq!(EntityDisclosure::get_pinned_announcement(ENTITY_ID), None);
    });
}

// ==================== Deep Audit 回归测试 ====================

// M1-deep: publish_disclosure 的 auto-blackout 不应缩短已有的更长黑窗口期
#[test]
fn m1_deep_auto_blackout_does_not_shorten_existing() {
    new_test_ext().execute_with(|| {
        // 配置: insider_trading_control=true, blackout_after=50
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 50u64,
        ));

        // 手动开始较长的黑窗口期: 200 blocks (结束于 block 201)
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 200u64,
        ));
        let (_, manual_end) = BlackoutPeriods::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(manual_end, 201); // block 1 + 200

        // 推进到 block 100，发布披露 → auto-blackout = 50 blocks (结束于 150)
        advance_blocks(99); // now = 100
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmContent".to_vec(), None,
        ));

        // M1-deep: 黑窗口期结束时间应保持 201（不被 150 覆盖）
        let (_, actual_end) = BlackoutPeriods::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(actual_end, manual_end, "auto-blackout should NOT shorten existing longer blackout");
    });
}

// M1-deep: correct_disclosure 的 auto-blackout 同样不应缩短
#[test]
fn m1_deep_correct_disclosure_blackout_does_not_shorten() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 50u64,
        ));

        // 发布初始披露
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmOriginal".to_vec(), None,
        ));

        // 手动开始较长黑窗口期
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 200u64,
        ));
        let (_, manual_end) = BlackoutPeriods::<Test>::get(ENTITY_ID).unwrap();

        // 更正披露 → auto-blackout 不应缩短
        advance_blocks(10);
        assert_ok!(EntityDisclosure::correct_disclosure(
            RuntimeOrigin::signed(OWNER), 0,
            b"QmCorrected".to_vec(), None,
        ));

        let (_, actual_end) = BlackoutPeriods::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(actual_end, manual_end, "correct_disclosure auto-blackout should NOT shorten existing");
    });
}

// M1-deep: 无已有黑窗口时，auto-blackout 正常设置
#[test]
fn m1_deep_auto_blackout_sets_when_none_exists() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 50u64,
        ));

        assert!(BlackoutPeriods::<Test>::get(ENTITY_ID).is_none());

        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmContent".to_vec(), None,
        ));

        let (start, end) = BlackoutPeriods::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(start, 1);
        assert_eq!(end, 51); // block 1 + 50
    });
}

// M2-deep: remove_insider 释放槽位后可重新添加到满
#[test]
fn m2_deep_remove_insider_frees_slot_for_new_addition() {
    new_test_ext().execute_with(|| {
        // MaxInsiders = 5, 先填满
        for i in 10..15u64 {
            assert_ok!(EntityDisclosure::add_insider(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, i, InsiderRole::Advisor,
            ));
        }
        assert_eq!(Insiders::<Test>::get(ENTITY_ID).len(), 5);

        // 满了不能添加
        assert_noop!(
            EntityDisclosure::add_insider(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, 99, InsiderRole::Advisor,
            ),
            Error::<Test>::InsidersFull
        );

        // 移除一个 → 释放槽位
        assert_ok!(EntityDisclosure::remove_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 12,
        ));
        assert_eq!(Insiders::<Test>::get(ENTITY_ID).len(), 4);

        // 现在可以添加新的
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 99, InsiderRole::Admin,
        ));
        assert_eq!(Insiders::<Test>::get(ENTITY_ID).len(), 5);
        assert!(Pallet::<Test>::is_insider(ENTITY_ID, &99));
        assert!(!Pallet::<Test>::is_insider(ENTITY_ID, &12));
    });
}

// ==================== M1-R3: 历史清理回归测试 ====================

// M1-R3: cleanup_disclosure_history 释放槽位后可发布新披露
#[test]
fn m1r3_cleanup_disclosure_history_frees_slot() {
    new_test_ext().execute_with(|| {
        // MaxDisclosureHistory = 10，先填满
        for _ in 0..10 {
            assert_ok!(EntityDisclosure::publish_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureType::AnnualReport,
                b"QmContent".to_vec(), None,
            ));
        }
        // 已满，无法发布新的
        assert_noop!(
            EntityDisclosure::publish_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureType::AnnualReport,
                b"QmNew".to_vec(), None,
            ),
            Error::<Test>::HistoryFull
        );

        // 撤回第一个披露
        assert_ok!(EntityDisclosure::withdraw_disclosure(
            RuntimeOrigin::signed(OWNER), 0,
        ));

        // 仍然满 — 撤回不释放索引
        assert_noop!(
            EntityDisclosure::publish_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureType::AnnualReport,
                b"QmNew".to_vec(), None,
            ),
            Error::<Test>::HistoryFull
        );

        // 清理 — 任何人可调用
        assert_ok!(EntityDisclosure::cleanup_disclosure_history(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 0,
        ));

        // 现在可以发布新的
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmNew".to_vec(), None,
        ));

        // 清理后记录本身仍可查询
        let record = Disclosures::<Test>::get(0).unwrap();
        assert_eq!(record.status, DisclosureStatus::Withdrawn);
    });
}

// M1-R3: cleanup_disclosure_history 拒绝清理 Published 状态
#[test]
fn m1r3_cleanup_disclosure_history_rejects_published() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmContent".to_vec(), None,
        ));

        assert_noop!(
            EntityDisclosure::cleanup_disclosure_history(
                RuntimeOrigin::signed(ALICE), ENTITY_ID, 0,
            ),
            Error::<Test>::DisclosureNotTerminal
        );
    });
}

// M1-R3: cleanup_disclosure_history 可清理 Corrected 状态
#[test]
fn m1r3_cleanup_disclosure_history_accepts_corrected() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmOriginal".to_vec(), None,
        ));
        // 更正 → 旧记录变为 Corrected
        assert_ok!(EntityDisclosure::correct_disclosure(
            RuntimeOrigin::signed(OWNER), 0,
            b"QmCorrected".to_vec(), None,
        ));

        assert_eq!(Disclosures::<Test>::get(0).unwrap().status, DisclosureStatus::Corrected);
        assert_ok!(EntityDisclosure::cleanup_disclosure_history(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 0,
        ));
        // 索引中不再包含 0
        assert!(!EntityDisclosures::<Test>::get(ENTITY_ID).contains(&0));
    });
}

// M1-R3: cleanup_announcement_history 释放槽位后可发布新公告
#[test]
fn m1r3_cleanup_announcement_history_frees_slot() {
    new_test_ext().execute_with(|| {
        // MaxAnnouncementHistory = 10，先填满
        for i in 0..10 {
            assert_ok!(EntityDisclosure::publish_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                AnnouncementCategory::General,
                format!("Title {}", i).into_bytes(),
                b"QmCid".to_vec(), None,
            ));
        }
        // 已满
        assert_noop!(
            EntityDisclosure::publish_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                AnnouncementCategory::General,
                b"Title New".to_vec(), b"QmCid".to_vec(), None,
            ),
            Error::<Test>::AnnouncementHistoryFull
        );

        // 撤回一个
        assert_ok!(EntityDisclosure::withdraw_announcement(
            RuntimeOrigin::signed(OWNER), 0,
        ));

        // 仍然满
        assert_noop!(
            EntityDisclosure::publish_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                AnnouncementCategory::General,
                b"Title New".to_vec(), b"QmCid".to_vec(), None,
            ),
            Error::<Test>::AnnouncementHistoryFull
        );

        // 清理撤回的公告
        assert_ok!(EntityDisclosure::cleanup_announcement_history(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 0,
        ));

        // 现在可以发布
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title New".to_vec(), b"QmCid".to_vec(), None,
        ));
    });
}

// M1-R3: cleanup_announcement_history 拒绝清理 Active 状态
#[test]
fn m1r3_cleanup_announcement_history_rejects_active() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));

        assert_noop!(
            EntityDisclosure::cleanup_announcement_history(
                RuntimeOrigin::signed(ALICE), ENTITY_ID, 0,
            ),
            Error::<Test>::AnnouncementNotTerminal
        );
    });
}

// M1-R3: cleanup_announcement_history 可清理 Expired 状态
#[test]
fn m1r3_cleanup_announcement_history_accepts_expired() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::Promotion,
            b"Promo".to_vec(), b"QmCid".to_vec(), Some(10u64),
        ));
        advance_blocks(10);
        assert_ok!(EntityDisclosure::expire_announcement(
            RuntimeOrigin::signed(ALICE), 0,
        ));

        assert_eq!(Announcements::<Test>::get(0).unwrap().status, AnnouncementStatus::Expired);
        assert_ok!(EntityDisclosure::cleanup_announcement_history(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 0,
        ));
        assert!(!EntityAnnouncements::<Test>::get(ENTITY_ID).contains(&0));
    });
}

// M1-R3: cleanup_disclosure_history 拒绝错误的 entity_id
#[test]
fn m1r3_cleanup_disclosure_history_rejects_wrong_entity() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmContent".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::withdraw_disclosure(
            RuntimeOrigin::signed(OWNER), 0,
        ));

        // 用错误的 entity_id 清理
        assert_noop!(
            EntityDisclosure::cleanup_disclosure_history(
                RuntimeOrigin::signed(ALICE), ENTITY_ID_2, 0,
            ),
            Error::<Test>::DisclosureNotFound
        );
    });
}
