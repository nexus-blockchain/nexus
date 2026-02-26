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
            50u64,
            100u64,
        ));

        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.level, DisclosureLevel::Standard);
        assert!(config.insider_trading_control);
        assert_eq!(config.blackout_period_before, 50);
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
                DisclosureLevel::Basic, false, 0u64, 0u64,
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
                DisclosureLevel::Basic, false, 0u64, 0u64,
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
            DisclosureLevel::Standard, true, 0u64, 50u64,
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
            DisclosureLevel::Basic, false, 0u64, 50u64,
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
fn add_insider_reactivates_removed() {
    new_test_ext().execute_with(|| {
        // H3: 添加后移除再添加，应重用旧记录
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_ok!(EntityDisclosure::remove_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE,
        ));
        assert!(!Pallet::<Test>::is_insider(ENTITY_ID, &ALICE));

        // 重新添加（不同角色）
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Auditor,
        ));
        assert!(Pallet::<Test>::is_insider(ENTITY_ID, &ALICE));

        // BoundedVec 长度不应增加
        let insiders = Insiders::<Test>::get(ENTITY_ID);
        assert_eq!(insiders.len(), 1);
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
            DisclosureLevel::Standard, true, 0u64, 100u64,
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
            DisclosureLevel::Standard, true, 0u64, 100u64,
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
            DisclosureLevel::Standard, false, 0u64, 0u64,
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
            DisclosureLevel::Enhanced, false, 0u64, 0u64,
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
                DisclosureLevel::Standard, true, 0u64, 201u64,
            ),
            Error::<Test>::BlackoutExceedsMax
        );
        // 恰好等于上限应成功
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 0u64, 200u64,
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
            DisclosureLevel::Standard, true, 0u64, 50u64,
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
            DisclosureLevel::Standard, false, 0u64, 0u64,
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
            DisclosureLevel::Enhanced, false, 0u64, 0u64,
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
            DisclosureLevel::Standard, false, 0u64, 0u64,
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
            DisclosureLevel::Enhanced, true, 0u64, 50u64,
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
