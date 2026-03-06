//! 财务披露模块测试

use crate::{mock::*, pallet::*};
use frame_support::{assert_noop, assert_ok, weights::Weight};
use pallet_entity_common::EntityStatus;

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
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 0,
        ));
        assert!(PinnedAnnouncements::<Test>::get(ENTITY_ID).contains(&0));

        // 撤回应清除置顶
        assert_ok!(EntityDisclosure::withdraw_announcement(
            RuntimeOrigin::signed(OWNER), 0,
        ));
        assert!(!PinnedAnnouncements::<Test>::get(ENTITY_ID).contains(&0));
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

// ==================== pin_announcement (P2-a14: multi-pin) ====================

#[test]
fn pin_announcement_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));

        assert_ok!(EntityDisclosure::pin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 0,
        ));

        let record = Announcements::<Test>::get(0).unwrap();
        assert!(record.is_pinned);
        assert!(PinnedAnnouncements::<Test>::get(ENTITY_ID).contains(&0));
    });
}

#[test]
fn p2a14_multi_pin_works() {
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

        // 置顶两个公告
        assert_ok!(EntityDisclosure::pin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 0,
        ));
        assert_ok!(EntityDisclosure::pin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 1,
        ));

        assert!(Announcements::<Test>::get(0).unwrap().is_pinned);
        assert!(Announcements::<Test>::get(1).unwrap().is_pinned);
        let pinned = PinnedAnnouncements::<Test>::get(ENTITY_ID);
        assert_eq!(pinned.len(), 2);
        assert!(pinned.contains(&0));
        assert!(pinned.contains(&1));
    });
}

#[test]
fn p2a14_pin_full_rejects() {
    new_test_ext().execute_with(|| {
        // MaxPinnedAnnouncements = 3
        for i in 0..4u64 {
            assert_ok!(EntityDisclosure::publish_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                AnnouncementCategory::General,
                format!("T{}", i).into_bytes(), format!("Qm{}", i).into_bytes(), None,
            ));
        }
        assert_ok!(EntityDisclosure::pin_announcement(RuntimeOrigin::signed(OWNER), ENTITY_ID, 0));
        assert_ok!(EntityDisclosure::pin_announcement(RuntimeOrigin::signed(OWNER), ENTITY_ID, 1));
        assert_ok!(EntityDisclosure::pin_announcement(RuntimeOrigin::signed(OWNER), ENTITY_ID, 2));

        // 第 4 个应失败
        assert_noop!(
            EntityDisclosure::pin_announcement(RuntimeOrigin::signed(OWNER), ENTITY_ID, 3),
            Error::<Test>::PinnedAnnouncementsFull
        );
    });
}

#[test]
fn p2a14_pin_idempotent() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::pin_announcement(RuntimeOrigin::signed(OWNER), ENTITY_ID, 0));
        // 重复置顶不应报错，也不应重复添加
        assert_ok!(EntityDisclosure::pin_announcement(RuntimeOrigin::signed(OWNER), ENTITY_ID, 0));
        assert_eq!(PinnedAnnouncements::<Test>::get(ENTITY_ID).len(), 1);
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
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 0,
        ));

        // 取消置顶
        assert_ok!(EntityDisclosure::unpin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 0,
        ));

        assert!(!Announcements::<Test>::get(0).unwrap().is_pinned);
        assert!(!PinnedAnnouncements::<Test>::get(ENTITY_ID).contains(&0));
    });
}

#[test]
fn p2a14_unpin_not_pinned_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));
        assert_noop!(
            EntityDisclosure::unpin_announcement(RuntimeOrigin::signed(OWNER), ENTITY_ID, 0),
            Error::<Test>::AnnouncementNotPinned
        );
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
                RuntimeOrigin::signed(ALICE), ENTITY_ID, 0,
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
                RuntimeOrigin::signed(OWNER), ENTITY_ID, 999,
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
                RuntimeOrigin::signed(OWNER_2), ENTITY_ID_2, 0,
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
                RuntimeOrigin::signed(OWNER), ENTITY_ID, 0,
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
fn get_active_pinned_announcements_works() {
    new_test_ext().execute_with(|| {
        assert!(Pallet::<Test>::get_active_pinned_announcements(ENTITY_ID).is_empty());

        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::pin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 0,
        ));

        assert_eq!(Pallet::<Test>::get_active_pinned_announcements(ENTITY_ID), vec![0]);
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
                RuntimeOrigin::signed(OWNER), ENTITY_ID, 0,
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
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 0,
        ));
        assert!(PinnedAnnouncements::<Test>::get(ENTITY_ID).contains(&0));

        advance_blocks(10);
        assert_ok!(EntityDisclosure::expire_announcement(
            RuntimeOrigin::signed(ALICE), 0,
        ));

        assert!(!PinnedAnnouncements::<Test>::get(ENTITY_ID).contains(&0));
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
fn m3r2_get_pinned_announcements_filters_expired() {
    new_test_ext().execute_with(|| {
        // 发布一个有过期时间的公告并置顶
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::Promotion,
            b"Pinned".to_vec(), b"QmCid".to_vec(), Some(20u64),
        ));
        assert_ok!(EntityDisclosure::pin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 0,
        ));

        // 过期前 — 应返回
        assert_eq!(EntityDisclosure::get_active_pinned_announcements(ENTITY_ID), vec![0]);

        // 过期后 — 应返回空
        advance_blocks(20);
        assert!(EntityDisclosure::get_active_pinned_announcements(ENTITY_ID).is_empty());
    });
}

#[test]
fn m3r2_get_pinned_announcements_filters_withdrawn() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::General,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::pin_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 0,
        ));

        // 撤回会从 PinnedAnnouncements 移除
        assert_ok!(EntityDisclosure::withdraw_announcement(
            RuntimeOrigin::signed(OWNER), 0,
        ));
        assert!(EntityDisclosure::get_active_pinned_announcements(ENTITY_ID).is_empty());
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

// ==================== P0-a7: report_disclosure_violation ====================

#[test]
fn p0a7_report_late_disclosure_works() {
    new_test_ext().execute_with(|| {
        // 配置 Standard 披露（间隔 500 blocks）
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.violation_count, 0);
        // next_required = 1 + 500 = 501

        // 推进到 502（逾期）
        advance_blocks(501);

        assert_ok!(EntityDisclosure::report_disclosure_violation(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
            ViolationType::LateDisclosure,
        ));

        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.violation_count, 1);

        // 事件验证
        System::assert_has_event(RuntimeEvent::EntityDisclosure(
            Event::DisclosureViolation {
                entity_id: ENTITY_ID,
                violation_type: ViolationType::LateDisclosure,
                violation_count: 1,
            }
        ));
    });
}

#[test]
fn p0a7_report_violation_rejects_not_overdue() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        // 当前 block=1，next_required=501，未逾期
        assert_noop!(
            EntityDisclosure::report_disclosure_violation(
                RuntimeOrigin::signed(ALICE), ENTITY_ID,
                ViolationType::LateDisclosure,
            ),
            Error::<Test>::DisclosureNotOverdue
        );
    });
}

#[test]
fn p0a7_report_violation_rejects_duplicate() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        advance_blocks(501);

        // 第一次举报成功
        assert_ok!(EntityDisclosure::report_disclosure_violation(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
            ViolationType::LateDisclosure,
        ));

        // 同一周期重复举报失败
        assert_noop!(
            EntityDisclosure::report_disclosure_violation(
                RuntimeOrigin::signed(BOB), ENTITY_ID,
                ViolationType::LateDisclosure,
            ),
            Error::<Test>::ViolationAlreadyRecorded
        );
    });
}

#[test]
fn p0a7_report_violation_rejects_no_config() {
    new_test_ext().execute_with(|| {
        // 未配置披露
        assert_noop!(
            EntityDisclosure::report_disclosure_violation(
                RuntimeOrigin::signed(ALICE), ENTITY_ID,
                ViolationType::LateDisclosure,
            ),
            Error::<Test>::DisclosureNotConfigured
        );
    });
}

#[test]
fn p0a7_report_violation_rejects_entity_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::report_disclosure_violation(
                RuntimeOrigin::signed(ALICE), 999,
                ViolationType::LateDisclosure,
            ),
            Error::<Test>::EntityNotFound
        );
    });
}

#[test]
fn p0a7_report_blackout_trading_violation() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 100u64,
        ));

        // 手动开始黑窗口期
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 50u64,
        ));

        assert_ok!(EntityDisclosure::report_disclosure_violation(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
            ViolationType::BlackoutTrading,
        ));

        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.violation_count, 1);
    });
}

#[test]
fn p0a7_report_blackout_violation_fails_no_blackout() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));

        // 无黑窗口期
        assert_noop!(
            EntityDisclosure::report_disclosure_violation(
                RuntimeOrigin::signed(ALICE), ENTITY_ID,
                ViolationType::BlackoutTrading,
            ),
            Error::<Test>::BlackoutNotFound
        );
    });
}

#[test]
fn p0a7_violation_count_preserves_on_reconfigure() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        advance_blocks(501);

        // 举报违规 → violation_count=1
        assert_ok!(EntityDisclosure::report_disclosure_violation(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
            ViolationType::LateDisclosure,
        ));

        // 重新配置不应清除 violation_count
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Enhanced, true, 50u64,
        ));

        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.violation_count, 1);
        assert_eq!(config.level, DisclosureLevel::Enhanced);
    });
}

// ==================== P1-a5: 披露级别降级保护 ====================

#[test]
fn p1a5_configure_upgrade_level_works() {
    new_test_ext().execute_with(|| {
        // Basic → Standard 升级允许
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Basic, false, 0u64,
        ));
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.level, DisclosureLevel::Standard);
    });
}

#[test]
fn p1a5_configure_downgrade_rejected() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Enhanced, false, 0u64,
        ));
        // Enhanced → Basic 降级被拒绝
        assert_noop!(
            EntityDisclosure::configure_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureLevel::Basic, false, 0u64,
            ),
            Error::<Test>::DisclosureLevelDowngrade
        );
        // Enhanced → Standard 也被拒绝
        assert_noop!(
            EntityDisclosure::configure_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureLevel::Standard, false, 0u64,
            ),
            Error::<Test>::DisclosureLevelDowngrade
        );
    });
}

#[test]
fn p1a5_configure_same_level_allowed() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        // Standard → Standard 不算降级
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 50u64,
        ));
        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert!(config.insider_trading_control);
    });
}

// ==================== P1-a15: force_configure_disclosure ====================

#[test]
fn p1a15_force_configure_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::force_configure_disclosure(
            RuntimeOrigin::root(), ENTITY_ID,
            DisclosureLevel::Full, true, 100u64,
        ));
        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.level, DisclosureLevel::Full);
        assert!(config.insider_trading_control);
        assert_eq!(config.blackout_period_after, 100);

        System::assert_has_event(RuntimeEvent::EntityDisclosure(
            Event::DisclosureForceConfigured {
                entity_id: ENTITY_ID,
                level: DisclosureLevel::Full,
            }
        ));
    });
}

#[test]
fn p1a15_force_configure_can_downgrade() {
    new_test_ext().execute_with(|| {
        // 先设置 Full
        assert_ok!(EntityDisclosure::force_configure_disclosure(
            RuntimeOrigin::root(), ENTITY_ID,
            DisclosureLevel::Full, true, 100u64,
        ));
        // Root 可降级到 Basic
        assert_ok!(EntityDisclosure::force_configure_disclosure(
            RuntimeOrigin::root(), ENTITY_ID,
            DisclosureLevel::Basic, false, 0u64,
        ));
        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.level, DisclosureLevel::Basic);
    });
}

#[test]
fn p1a15_force_configure_rejects_non_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::force_configure_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureLevel::Full, true, 100u64,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn p1a15_force_configure_rejects_entity_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::force_configure_disclosure(
                RuntimeOrigin::root(), 999,
                DisclosureLevel::Basic, false, 0u64,
            ),
            Error::<Test>::EntityNotFound
        );
    });
}

#[test]
fn p1a15_force_configure_preserves_violation_count() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        advance_blocks(501);
        assert_ok!(EntityDisclosure::report_disclosure_violation(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
            ViolationType::LateDisclosure,
        ));

        // force_configure 保留 violation_count
        assert_ok!(EntityDisclosure::force_configure_disclosure(
            RuntimeOrigin::root(), ENTITY_ID,
            DisclosureLevel::Basic, false, 0u64,
        ));
        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.violation_count, 1);
        assert_eq!(config.level, DisclosureLevel::Basic);
    });
}

#[test]
fn p1a15_force_configure_rejects_blackout_exceeds_max() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::force_configure_disclosure(
                RuntimeOrigin::root(), ENTITY_ID,
                DisclosureLevel::Standard, true, 999u64,
            ),
            Error::<Test>::BlackoutExceedsMax
        );
    });
}

// ==================== P1-a20: cleanup_entity_disclosure ====================

#[test]
fn p1a20_cleanup_works_for_closed_entity() {
    new_test_ext().execute_with(|| {
        // 设置披露数据
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 50u64,
        ));
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 100u64,
        ));
        assert_ok!(EntityDisclosure::publish_announcement(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            AnnouncementCategory::Promotion,
            b"Title".to_vec(), b"QmCid".to_vec(), None,
        ));

        // 验证数据存在
        assert!(DisclosureConfigs::<Test>::get(ENTITY_ID).is_some());
        assert!(!Insiders::<Test>::get(ENTITY_ID).is_empty());
        assert!(BlackoutPeriods::<Test>::get(ENTITY_ID).is_some());
        assert!(!EntityAnnouncements::<Test>::get(ENTITY_ID).is_empty());

        // 活跃实体不能清理
        assert_noop!(
            EntityDisclosure::cleanup_entity_disclosure(
                RuntimeOrigin::signed(ALICE), ENTITY_ID,
            ),
            Error::<Test>::EntityNotClosed
        );

        // 将实体标记为 Closed
        set_entity_status(ENTITY_ID, pallet_entity_common::EntityStatus::Closed);

        // 清理成功
        assert_ok!(EntityDisclosure::cleanup_entity_disclosure(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
        ));

        // 验证存储已清理
        assert!(DisclosureConfigs::<Test>::get(ENTITY_ID).is_none());
        assert!(Insiders::<Test>::get(ENTITY_ID).is_empty());
        assert!(BlackoutPeriods::<Test>::get(ENTITY_ID).is_none());
        assert!(EntityAnnouncements::<Test>::get(ENTITY_ID).is_empty());
        assert!(PinnedAnnouncements::<Test>::get(ENTITY_ID).is_empty());

        System::assert_has_event(RuntimeEvent::EntityDisclosure(
            Event::EntityDisclosureCleaned { entity_id: ENTITY_ID }
        ));
    });
}

#[test]
fn p1a20_cleanup_works_for_banned_entity() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Basic, false, 0u64,
        ));

        set_entity_status(ENTITY_ID, pallet_entity_common::EntityStatus::Banned);

        assert_ok!(EntityDisclosure::cleanup_entity_disclosure(
            RuntimeOrigin::signed(BOB), ENTITY_ID,
        ));
        assert!(DisclosureConfigs::<Test>::get(ENTITY_ID).is_none());
    });
}

#[test]
fn p1a20_cleanup_works_for_nonexistent_entity() {
    new_test_ext().execute_with(|| {
        // entity_id=999 不存在（entity_status 返回 None）
        assert_ok!(EntityDisclosure::cleanup_entity_disclosure(
            RuntimeOrigin::signed(ALICE), 999,
        ));
    });
}

#[test]
fn p1a20_cleanup_rejects_active_entity() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::cleanup_entity_disclosure(
                RuntimeOrigin::signed(ALICE), ENTITY_ID,
            ),
            Error::<Test>::EntityNotClosed
        );
    });
}

#[test]
fn p1a20_cleanup_rejects_suspended_entity() {
    new_test_ext().execute_with(|| {
        set_entity_status(ENTITY_ID, pallet_entity_common::EntityStatus::Suspended);
        assert_noop!(
            EntityDisclosure::cleanup_entity_disclosure(
                RuntimeOrigin::signed(ALICE), ENTITY_ID,
            ),
            Error::<Test>::EntityNotClosed
        );
    });
}

// ==================== P1-a16: on_idle 违规自动检测 ====================

#[test]
fn p1a16_on_idle_detects_overdue_violation() {
    new_test_ext().execute_with(|| {
        use frame_support::traits::Hooks;

        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        // next_required = 1 + 500 = 501
        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.violation_count, 0);

        // 推进到逾期
        advance_blocks(501);

        // 触发 on_idle
        let weight = EntityDisclosure::on_idle(502, Weight::from_parts(u64::MAX, u64::MAX));
        assert!(weight.ref_time() > 0);

        // violation_count 应该递增
        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.violation_count, 1);

        System::assert_has_event(RuntimeEvent::EntityDisclosure(
            Event::AutoViolationDetected {
                entity_id: ENTITY_ID,
                violation_count: 1,
            }
        ));
    });
}

#[test]
fn p1a16_on_idle_skips_non_overdue() {
    new_test_ext().execute_with(|| {
        use frame_support::traits::Hooks;

        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));

        // 未逾期，on_idle 不应递增
        let _ = EntityDisclosure::on_idle(1, Weight::from_parts(u64::MAX, u64::MAX));

        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.violation_count, 0);
    });
}

#[test]
fn p1a16_on_idle_no_duplicate_in_same_period() {
    new_test_ext().execute_with(|| {
        use frame_support::traits::Hooks;

        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        advance_blocks(501);

        // 第一次 on_idle 检测
        EntityDisclosure::on_idle(502, Weight::from_parts(u64::MAX, u64::MAX));
        assert_eq!(DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap().violation_count, 1);

        // 第二次 on_idle 不重复
        EntityDisclosure::on_idle(503, Weight::from_parts(u64::MAX, u64::MAX));
        assert_eq!(DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap().violation_count, 1);
    });
}

#[test]
fn p1a16_on_idle_respects_weight_limit() {
    new_test_ext().execute_with(|| {
        use frame_support::traits::Hooks;

        // 给极小的 weight，不应扫描任何实体
        let weight = EntityDisclosure::on_idle(1, Weight::from_parts(1_000, 0));
        assert_eq!(weight.ref_time(), 0);
    });
}

// ==================== P2-a2: 草稿/Draft 流程 ====================

#[test]
fn p2a2_create_draft_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::create_draft_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmDraft".to_vec(), None,
        ));

        let record = Disclosures::<Test>::get(0).unwrap();
        assert_eq!(record.status, DisclosureStatus::Draft);
        assert_eq!(record.entity_id, ENTITY_ID);

        // 草稿不进入 EntityDisclosures 历史
        assert!(EntityDisclosures::<Test>::get(ENTITY_ID).is_empty());

        System::assert_has_event(RuntimeEvent::EntityDisclosure(
            Event::DraftCreated {
                disclosure_id: 0,
                entity_id: ENTITY_ID,
                disclosure_type: DisclosureType::AnnualReport,
            }
        ));
    });
}

#[test]
fn p2a2_update_draft_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::create_draft_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmOld".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::update_draft(
            RuntimeOrigin::signed(OWNER), 0,
            b"QmNew".to_vec(), Some(b"QmSummary".to_vec()),
        ));

        let record = Disclosures::<Test>::get(0).unwrap();
        assert_eq!(record.content_cid.to_vec(), b"QmNew".to_vec());
        assert!(record.summary_cid.is_some());
    });
}

#[test]
fn p2a2_update_draft_rejects_published() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmContent".to_vec(), None,
        ));
        assert_noop!(
            EntityDisclosure::update_draft(
                RuntimeOrigin::signed(OWNER), 0,
                b"QmNew".to_vec(), None,
            ),
            Error::<Test>::DisclosureNotDraft
        );
    });
}

#[test]
fn p2a2_delete_draft_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::create_draft_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmDraft".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::delete_draft(
            RuntimeOrigin::signed(OWNER), 0,
        ));

        assert!(Disclosures::<Test>::get(0).is_none());

        System::assert_has_event(RuntimeEvent::EntityDisclosure(
            Event::DraftDeleted {
                disclosure_id: 0,
                entity_id: ENTITY_ID,
            }
        ));
    });
}

#[test]
fn p2a2_delete_draft_rejects_published() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmContent".to_vec(), None,
        ));
        assert_noop!(
            EntityDisclosure::delete_draft(RuntimeOrigin::signed(OWNER), 0),
            Error::<Test>::DisclosureNotDraft
        );
    });
}

#[test]
fn p2a2_publish_draft_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 50u64,
        ));

        assert_ok!(EntityDisclosure::create_draft_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmDraft".to_vec(), None,
        ));

        // 发布草稿
        assert_ok!(EntityDisclosure::publish_draft(
            RuntimeOrigin::signed(OWNER), 0,
        ));

        let record = Disclosures::<Test>::get(0).unwrap();
        assert_eq!(record.status, DisclosureStatus::Published);

        // 应进入 EntityDisclosures 历史
        assert_eq!(EntityDisclosures::<Test>::get(ENTITY_ID).len(), 1);

        // 应触发黑窗口期
        assert!(EntityDisclosure::is_in_blackout(ENTITY_ID));

        System::assert_has_event(RuntimeEvent::EntityDisclosure(
            Event::DisclosurePublished {
                disclosure_id: 0,
                entity_id: ENTITY_ID,
                disclosure_type: DisclosureType::AnnualReport,
                discloser: OWNER,
            }
        ));
    });
}

#[test]
fn p2a2_publish_draft_rejects_non_draft() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmContent".to_vec(), None,
        ));
        assert_noop!(
            EntityDisclosure::publish_draft(RuntimeOrigin::signed(OWNER), 0),
            Error::<Test>::DisclosureNotDraft
        );
    });
}

#[test]
fn p2a2_draft_not_admin_rejected() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::create_draft_disclosure(
                RuntimeOrigin::signed(ALICE), ENTITY_ID,
                DisclosureType::AnnualReport,
                b"QmDraft".to_vec(), None,
            ),
            Error::<Test>::NotAdmin
        );
    });
}

// ==================== P2-a3: update_insider_role ====================

#[test]
fn p2a3_update_insider_role_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));

        assert_ok!(EntityDisclosure::update_insider_role(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Auditor,
        ));

        let insiders = Insiders::<Test>::get(ENTITY_ID);
        assert_eq!(insiders[0].role, InsiderRole::Auditor);

        System::assert_has_event(RuntimeEvent::EntityDisclosure(
            Event::InsiderRoleUpdated {
                entity_id: ENTITY_ID,
                account: ALICE,
                old_role: InsiderRole::Admin,
                new_role: InsiderRole::Auditor,
            }
        ));
    });
}

#[test]
fn p2a3_update_insider_role_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::update_insider_role(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Auditor,
            ),
            Error::<Test>::InsiderNotFound
        );
    });
}

#[test]
fn p2a3_update_insider_role_not_admin() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_noop!(
            EntityDisclosure::update_insider_role(
                RuntimeOrigin::signed(ALICE), ENTITY_ID, ALICE, InsiderRole::Auditor,
            ),
            Error::<Test>::NotAdmin
        );
    });
}

// ==================== P2-a23: insider role change history ====================

#[test]
fn p2a23_add_insider_records_initial_history() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));

        let history = InsiderRoleHistory::<Test>::get(ENTITY_ID, ALICE);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].old_role, None);
        assert_eq!(history[0].new_role, InsiderRole::Admin);
        assert_eq!(history[0].changed_at, 1);
    });
}

#[test]
fn p2a23_update_insider_role_records_history() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));

        advance_blocks(5);

        assert_ok!(EntityDisclosure::update_insider_role(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Auditor,
        ));

        let history = InsiderRoleHistory::<Test>::get(ENTITY_ID, ALICE);
        assert_eq!(history.len(), 2);
        // 初始添加
        assert_eq!(history[0].old_role, None);
        assert_eq!(history[0].new_role, InsiderRole::Admin);
        // 角色变更
        assert_eq!(history[1].old_role, Some(InsiderRole::Admin));
        assert_eq!(history[1].new_role, InsiderRole::Auditor);
        assert_eq!(history[1].changed_at, 6);
    });
}

#[test]
fn p2a23_multiple_role_changes_accumulate() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_ok!(EntityDisclosure::update_insider_role(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Auditor,
        ));
        assert_ok!(EntityDisclosure::update_insider_role(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Advisor,
        ));

        let history = InsiderRoleHistory::<Test>::get(ENTITY_ID, ALICE);
        assert_eq!(history.len(), 3);
        assert_eq!(history[2].old_role, Some(InsiderRole::Auditor));
        assert_eq!(history[2].new_role, InsiderRole::Advisor);
    });
}

#[test]
fn p2a23_cleanup_clears_role_history() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));

        assert!(!InsiderRoleHistory::<Test>::get(ENTITY_ID, ALICE).is_empty());

        // 需要实体关闭才能清理
        set_entity_status(ENTITY_ID, pallet_entity_common::EntityStatus::Closed);
        assert_ok!(EntityDisclosure::cleanup_entity_disclosure(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
        ));

        assert!(InsiderRoleHistory::<Test>::get(ENTITY_ID, ALICE).is_empty());
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

// ==================== EntityLocked 回归测试 ====================

#[test]
fn entity_locked_rejects_configure_disclosure() {
    new_test_ext().execute_with(|| {
        set_entity_locked(ENTITY_ID);
        assert_noop!(
            EntityDisclosure::configure_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureLevel::Standard, true, 50u64,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_publish_disclosure() {
    new_test_ext().execute_with(|| {
        set_entity_locked(ENTITY_ID);
        assert_noop!(
            EntityDisclosure::publish_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureType::AnnualReport,
                b"QmContent".to_vec(), None,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_publish_announcement() {
    new_test_ext().execute_with(|| {
        set_entity_locked(ENTITY_ID);
        assert_noop!(
            EntityDisclosure::publish_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                AnnouncementCategory::General,
                b"Title".to_vec(), b"QmContent".to_vec(),
                None,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_add_insider() {
    new_test_ext().execute_with(|| {
        set_entity_locked(ENTITY_ID);
        assert_noop!(
            EntityDisclosure::add_insider(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_allows_cleanup_disclosure_history() {
    new_test_ext().execute_with(|| {
        // 先发布并撤回一条披露（未锁定时）
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmContent".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::withdraw_disclosure(
            RuntimeOrigin::signed(OWNER), 0,
        ));
        // 锁定后仍可清理
        set_entity_locked(ENTITY_ID);
        assert_ok!(EntityDisclosure::cleanup_disclosure_history(
            RuntimeOrigin::signed(ALICE), ENTITY_ID, 0,
        ));
    });
}

// ==================== F1: 批量内幕人员操作 ====================

#[test]
fn f1_batch_add_insiders_works() {
    new_test_ext().execute_with(|| {
        let batch = vec![
            (ALICE, InsiderRole::Admin),
            (BOB, InsiderRole::Auditor),
        ];
        assert_ok!(EntityDisclosure::batch_add_insiders(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, batch,
        ));

        let insiders = Insiders::<Test>::get(ENTITY_ID);
        assert_eq!(insiders.len(), 2);
        assert!(insiders.iter().any(|i| i.account == ALICE && i.role == InsiderRole::Admin));
        assert!(insiders.iter().any(|i| i.account == BOB && i.role == InsiderRole::Auditor));

        // 角色历史也应记录
        assert_eq!(InsiderRoleHistory::<Test>::get(ENTITY_ID, ALICE).len(), 1);
        assert_eq!(InsiderRoleHistory::<Test>::get(ENTITY_ID, BOB).len(), 1);

        System::assert_has_event(RuntimeEvent::EntityDisclosure(
            Event::InsidersBatchAdded { entity_id: ENTITY_ID, count: 2 }
        ));
    });
}

#[test]
fn f1_batch_add_insiders_rejects_empty() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::batch_add_insiders(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, vec![],
            ),
            Error::<Test>::EmptyBatch
        );
    });
}

#[test]
fn f1_batch_add_insiders_rejects_duplicate() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_noop!(
            EntityDisclosure::batch_add_insiders(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                vec![(ALICE, InsiderRole::Auditor)],
            ),
            Error::<Test>::InsiderExists
        );
    });
}

#[test]
fn f1_batch_add_insiders_rejects_full() {
    new_test_ext().execute_with(|| {
        // MaxInsiders = 5, add 5 individually first
        for i in 10u64..15 {
            assert_ok!(EntityDisclosure::add_insider(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, i, InsiderRole::Admin,
            ));
        }
        assert_noop!(
            EntityDisclosure::batch_add_insiders(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                vec![(20, InsiderRole::Admin)],
            ),
            Error::<Test>::InsidersFull
        );
    });
}

#[test]
fn f1_batch_remove_insiders_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::batch_add_insiders(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            vec![(ALICE, InsiderRole::Admin), (BOB, InsiderRole::Auditor)],
        ));

        assert_ok!(EntityDisclosure::batch_remove_insiders(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            vec![ALICE, BOB],
        ));

        assert!(Insiders::<Test>::get(ENTITY_ID).is_empty());

        System::assert_has_event(RuntimeEvent::EntityDisclosure(
            Event::InsidersBatchRemoved { entity_id: ENTITY_ID, count: 2 }
        ));
    });
}

#[test]
fn f1_batch_remove_insiders_rejects_empty() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::batch_remove_insiders(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, vec![],
            ),
            Error::<Test>::EmptyBatch
        );
    });
}

#[test]
fn f1_batch_remove_insiders_rejects_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::batch_remove_insiders(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                vec![ALICE],
            ),
            Error::<Test>::InsiderNotFound
        );
    });
}

// ==================== F2: 实体状态守卫 ====================

#[test]
fn f2_entity_not_active_rejects_configure_disclosure() {
    new_test_ext().execute_with(|| {
        set_entity_status(ENTITY_ID, pallet_entity_common::EntityStatus::Suspended);
        assert_noop!(
            EntityDisclosure::configure_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureLevel::Standard, true, 50u64,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f2_entity_not_active_rejects_publish_disclosure() {
    new_test_ext().execute_with(|| {
        set_entity_status(ENTITY_ID, pallet_entity_common::EntityStatus::Banned);
        assert_noop!(
            EntityDisclosure::publish_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureType::AnnualReport,
                b"QmContent".to_vec(), None,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f2_entity_not_active_rejects_add_insider() {
    new_test_ext().execute_with(|| {
        set_entity_status(ENTITY_ID, pallet_entity_common::EntityStatus::Suspended);
        assert_noop!(
            EntityDisclosure::add_insider(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f2_entity_not_active_rejects_publish_announcement() {
    new_test_ext().execute_with(|| {
        set_entity_status(ENTITY_ID, pallet_entity_common::EntityStatus::Suspended);
        assert_noop!(
            EntityDisclosure::publish_announcement(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                AnnouncementCategory::General,
                b"Title".to_vec(), b"QmContent".to_vec(), None,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f2_entity_not_active_allows_withdraw() {
    new_test_ext().execute_with(|| {
        // 先发布（Active时）
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmContent".to_vec(), None,
        ));
        // 设置为 Suspended
        set_entity_status(ENTITY_ID, pallet_entity_common::EntityStatus::Suspended);
        // 撤回应仍然可以
        assert_ok!(EntityDisclosure::withdraw_disclosure(
            RuntimeOrigin::signed(OWNER), 0,
        ));
    });
}

#[test]
fn f2_entity_not_active_rejects_batch_add() {
    new_test_ext().execute_with(|| {
        set_entity_status(ENTITY_ID, pallet_entity_common::EntityStatus::Suspended);
        assert_noop!(
            EntityDisclosure::batch_add_insiders(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                vec![(ALICE, InsiderRole::Admin)],
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

// ==================== F3: 披露级别与类型匹配校验 ====================

#[test]
fn f3_basic_level_allows_annual_report() {
    new_test_ext().execute_with(|| {
        // Basic是默认级别，无需配置
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmContent".to_vec(), None,
        ));
    });
}

#[test]
fn f3_basic_level_rejects_quarterly_report() {
    new_test_ext().execute_with(|| {
        // Basic 不允许 QuarterlyReport
        assert_noop!(
            EntityDisclosure::publish_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureType::QuarterlyReport,
                b"QmContent".to_vec(), None,
            ),
            Error::<Test>::DisclosureTypeNotAllowed
        );
    });
}

#[test]
fn f3_standard_level_allows_quarterly_report() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::QuarterlyReport,
            b"QmContent".to_vec(), None,
        ));
    });
}

#[test]
fn f3_standard_level_rejects_token_issuance() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        assert_noop!(
            EntityDisclosure::publish_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureType::TokenIssuance,
                b"QmContent".to_vec(), None,
            ),
            Error::<Test>::DisclosureTypeNotAllowed
        );
    });
}

#[test]
fn f3_enhanced_level_allows_most_types() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Enhanced, false, 0u64,
        ));
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::ManagementChange,
            b"QmContent".to_vec(), None,
        ));
    });
}

#[test]
fn f3_enhanced_level_rejects_buyback() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Enhanced, false, 0u64,
        ));
        assert_noop!(
            EntityDisclosure::publish_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureType::Buyback,
                b"QmContent".to_vec(), None,
            ),
            Error::<Test>::DisclosureTypeNotAllowed
        );
    });
}

#[test]
fn f3_full_level_allows_all() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Full, false, 0u64,
        ));
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::TokenIssuance,
            b"QmContent".to_vec(), None,
        ));
    });
}

#[test]
fn f3_draft_also_validates_type() {
    new_test_ext().execute_with(|| {
        // Basic 不允许 Buyback
        assert_noop!(
            EntityDisclosure::create_draft_disclosure(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
                DisclosureType::Buyback,
                b"QmDraft".to_vec(), None,
            ),
            Error::<Test>::DisclosureTypeNotAllowed
        );
    });
}

// ==================== F4: 内幕人员移除冷静期 ====================

#[test]
fn f4_remove_insider_records_cooldown() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_ok!(EntityDisclosure::remove_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE,
        ));

        // 冷静期应被记录 (block 1 + 50 = 51)
        assert_eq!(RemovedInsiders::<Test>::get(ENTITY_ID, ALICE), Some(51));

        System::assert_has_event(RuntimeEvent::EntityDisclosure(
            Event::InsiderCooldownStarted {
                entity_id: ENTITY_ID,
                account: ALICE,
                until: 51,
            }
        ));
    });
}

#[test]
fn f4_cooldown_blocks_trade_during_blackout() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 100u64,
        ));
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));

        // 移除内幕人员 → 进入冷静期
        assert_ok!(EntityDisclosure::remove_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE,
        ));

        // 启动黑窗口期
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 100u64,
        ));

        // 冷静期内 + 黑窗口期 = 不能交易
        assert!(!EntityDisclosure::can_insider_trade(ENTITY_ID, &ALICE));

        // 推进过冷静期
        advance_blocks(51);

        // 冷静期已过 + 非内幕人员 → 允许交易
        assert!(EntityDisclosure::can_insider_trade(ENTITY_ID, &ALICE));
    });
}

#[test]
fn f4_cooldown_no_effect_without_blackout() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 0u64,
        ));
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_ok!(EntityDisclosure::remove_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE,
        ));

        // 冷静期内但没有黑窗口期 → 允许交易
        assert!(EntityDisclosure::can_insider_trade(ENTITY_ID, &ALICE));
    });
}

#[test]
fn f4_batch_remove_records_cooldown() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::batch_add_insiders(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            vec![(ALICE, InsiderRole::Admin), (BOB, InsiderRole::Auditor)],
        ));
        assert_ok!(EntityDisclosure::batch_remove_insiders(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            vec![ALICE, BOB],
        ));

        assert!(RemovedInsiders::<Test>::get(ENTITY_ID, ALICE).is_some());
        assert!(RemovedInsiders::<Test>::get(ENTITY_ID, BOB).is_some());
    });
}

// ==================== F5: MajorHolder 阈值查询 ====================

#[test]
fn f5_major_holder_threshold_query() {
    new_test_ext().execute_with(|| {
        assert_eq!(EntityDisclosure::get_major_holder_threshold(), 500);
    });
}

// ==================== F6: 违规后果执行 ====================

#[test]
fn f6_violation_threshold_marks_high_risk() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));

        // 逾期
        advance_blocks(501);

        // 手动报告 3 次违规 (ViolationThreshold=3)
        // 第一次
        assert_ok!(EntityDisclosure::report_disclosure_violation(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
            ViolationType::LateDisclosure,
        ));
        assert!(!HighRiskEntities::<Test>::get(ENTITY_ID));

        // 需要推进到新周期才能再举报
        // 通过发布披露重置周期
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmContent".to_vec(), None,
        ));
        advance_blocks(501);
        assert_ok!(EntityDisclosure::report_disclosure_violation(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
            ViolationType::LateDisclosure,
        ));
        assert!(!HighRiskEntities::<Test>::get(ENTITY_ID));

        // 第三次
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmContent2".to_vec(), None,
        ));
        advance_blocks(501);
        assert_ok!(EntityDisclosure::report_disclosure_violation(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
            ViolationType::LateDisclosure,
        ));

        // 现在应该被标记为高风险
        assert!(HighRiskEntities::<Test>::get(ENTITY_ID));

        System::assert_has_event(RuntimeEvent::EntityDisclosure(
            Event::EntityMarkedHighRisk {
                entity_id: ENTITY_ID,
                violation_count: 3,
            }
        ));
    });
}

#[test]
fn f6_high_risk_queryable_via_provider() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::DisclosureProvider;
        assert!(!<EntityDisclosure as DisclosureProvider<u64>>::is_high_risk(ENTITY_ID));

        HighRiskEntities::<Test>::insert(ENTITY_ID, true);
        assert!(<EntityDisclosure as DisclosureProvider<u64>>::is_high_risk(ENTITY_ID));
    });
}

// ==================== F7: DisclosureProvider trait 扩展 ====================

#[test]
fn f7_get_violation_count_works() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::DisclosureProvider;
        assert_eq!(<EntityDisclosure as DisclosureProvider<u64>>::get_violation_count(ENTITY_ID), 0);

        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        advance_blocks(501);
        assert_ok!(EntityDisclosure::report_disclosure_violation(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
            ViolationType::LateDisclosure,
        ));

        assert_eq!(<EntityDisclosure as DisclosureProvider<u64>>::get_violation_count(ENTITY_ID), 1);
    });
}

#[test]
fn f7_get_insider_role_works() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::DisclosureProvider;
        assert_eq!(<EntityDisclosure as DisclosureProvider<u64>>::get_insider_role(ENTITY_ID, &ALICE), None);

        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Auditor,
        ));

        // Auditor = 2
        assert_eq!(<EntityDisclosure as DisclosureProvider<u64>>::get_insider_role(ENTITY_ID, &ALICE), Some(2));
    });
}

#[test]
fn f7_is_disclosure_configured_works() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::DisclosureProvider;
        assert!(!<EntityDisclosure as DisclosureProvider<u64>>::is_disclosure_configured(ENTITY_ID));

        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));

        assert!(<EntityDisclosure as DisclosureProvider<u64>>::is_disclosure_configured(ENTITY_ID));
    });
}

// ==================== F8: 违规记录清理 ====================

#[test]
fn f8_reset_violation_count_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        advance_blocks(501);
        assert_ok!(EntityDisclosure::report_disclosure_violation(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
            ViolationType::LateDisclosure,
        ));
        assert_eq!(DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap().violation_count, 1);

        assert_ok!(EntityDisclosure::reset_violation_count(
            RuntimeOrigin::root(), ENTITY_ID,
        ));

        assert_eq!(DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap().violation_count, 0);

        System::assert_has_event(RuntimeEvent::EntityDisclosure(
            Event::ViolationCountReset { entity_id: ENTITY_ID }
        ));
    });
}

#[test]
fn f8_reset_violation_count_clears_high_risk() {
    new_test_ext().execute_with(|| {
        HighRiskEntities::<Test>::insert(ENTITY_ID, true);
        assert!(HighRiskEntities::<Test>::get(ENTITY_ID));

        assert_ok!(EntityDisclosure::reset_violation_count(
            RuntimeOrigin::root(), ENTITY_ID,
        ));

        assert!(!HighRiskEntities::<Test>::get(ENTITY_ID));
    });
}

#[test]
fn f8_reset_violation_count_rejects_non_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::reset_violation_count(
                RuntimeOrigin::signed(OWNER), ENTITY_ID,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn f8_reset_violation_count_rejects_entity_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::reset_violation_count(
                RuntimeOrigin::root(), 999,
            ),
            Error::<Test>::EntityNotFound
        );
    });
}

// ==================== F9: 黑窗口期过期存储清理 ====================

#[test]
fn f9_expire_blackout_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 50u64,
        ));
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 10u64,
        ));
        assert!(BlackoutPeriods::<Test>::contains_key(ENTITY_ID));

        // 推进过黑窗口期
        advance_blocks(11);

        assert_ok!(EntityDisclosure::expire_blackout(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
        ));
        assert!(!BlackoutPeriods::<Test>::contains_key(ENTITY_ID));

        System::assert_has_event(RuntimeEvent::EntityDisclosure(
            Event::BlackoutExpired { entity_id: ENTITY_ID }
        ));
    });
}

#[test]
fn f9_expire_blackout_rejects_still_active() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 50u64,
        ));
        assert_ok!(EntityDisclosure::start_blackout(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 100u64,
        ));

        // 黑窗口期仍活跃
        assert_noop!(
            EntityDisclosure::expire_blackout(
                RuntimeOrigin::signed(ALICE), ENTITY_ID,
            ),
            Error::<Test>::BlackoutNotExpired
        );
    });
}

#[test]
fn f9_expire_blackout_rejects_no_blackout() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::expire_blackout(
                RuntimeOrigin::signed(ALICE), ENTITY_ID,
            ),
            Error::<Test>::BlackoutNotFound
        );
    });
}

// ==================== 清理测试：新存储也被清理 ====================

#[test]
fn cleanup_entity_disclosure_clears_cooldown_and_high_risk() {
    new_test_ext().execute_with(|| {
        // 设置一些 F4/F6 数据
        RemovedInsiders::<Test>::insert(ENTITY_ID, ALICE, 100u64);
        HighRiskEntities::<Test>::insert(ENTITY_ID, true);

        // 需要实体关闭才能清理
        set_entity_status(ENTITY_ID, pallet_entity_common::EntityStatus::Closed);
        assert_ok!(EntityDisclosure::cleanup_entity_disclosure(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
        ));

        assert!(RemovedInsiders::<Test>::get(ENTITY_ID, ALICE).is_none());
        assert!(!HighRiskEntities::<Test>::get(ENTITY_ID));
    });
}

// ==================== 审计回归测试 ====================

// H1-R2: on_idle 游标扫描 — skip-count 正确推进
#[test]
fn h1r2_on_idle_cursor_advances() {
    new_test_ext().execute_with(|| {
        use frame_support::traits::Hooks;

        // 为 ENTITY_ID 配置披露
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        // next_required = 1 + 500 = 501

        advance_blocks(501);

        // 第一次 on_idle — 扫描 1 个实体
        EntityDisclosure::on_idle(502, Weight::from_parts(u64::MAX, u64::MAX));
        assert_eq!(DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap().violation_count, 1);

        // 游标应为 1（跳过计数）
        assert_eq!(AutoViolationCursor::<Test>::get(), 1);

        // 第二次 on_idle — skip_count=1 跳过唯一实体，scanned=0，归零
        EntityDisclosure::on_idle(503, Weight::from_parts(u64::MAX, u64::MAX));
        assert_eq!(AutoViolationCursor::<Test>::get(), 0);

        // 同一周期 ViolationRecords 去重，violation_count 仍为 1
        assert_eq!(DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap().violation_count, 1);
    });
}

// H1-R2: on_idle 两个实体都能被扫描到（验证无永久跳过）
#[test]
fn h1r2_on_idle_scans_all_entities() {
    new_test_ext().execute_with(|| {
        use frame_support::traits::Hooks;

        // 配置两个实体
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER_2), ENTITY_ID_2,
            DisclosureLevel::Standard, false, 0u64,
        ));

        advance_blocks(501);

        // 第一次 on_idle — 扫描两个实体（MAX_SCAN=10 > 2）
        EntityDisclosure::on_idle(502, Weight::from_parts(u64::MAX, u64::MAX));

        // 两个实体都应该被检测到违规
        assert_eq!(DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap().violation_count, 1);
        assert_eq!(DisclosureConfigs::<Test>::get(ENTITY_ID_2).unwrap().violation_count, 1);

        // 游标应为 2
        assert_eq!(AutoViolationCursor::<Test>::get(), 2);
    });
}

// H1-R2: on_idle 游标回绕后重新扫描
#[test]
fn h1r2_on_idle_cursor_wraps_and_rescans() {
    new_test_ext().execute_with(|| {
        use frame_support::traits::Hooks;

        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0u64,
        ));

        advance_blocks(501);

        // 第一次扫描
        EntityDisclosure::on_idle(502, Weight::from_parts(u64::MAX, u64::MAX));
        assert_eq!(AutoViolationCursor::<Test>::get(), 1);

        // 第二次扫描 — 跳过 1 个后无实体，归零
        EntityDisclosure::on_idle(503, Weight::from_parts(u64::MAX, u64::MAX));
        assert_eq!(AutoViolationCursor::<Test>::get(), 0);

        // 发布披露重置周期
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport,
            b"QmContent".to_vec(), None,
        ));
        advance_blocks(501);

        // 第三次扫描 — cursor=0，从头开始，检测新周期违规
        EntityDisclosure::on_idle(1004, Weight::from_parts(u64::MAX, u64::MAX));
        assert_eq!(DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap().violation_count, 2);
    });
}

// M1: governance_configure_disclosure 拒绝超出上限的 blackout_period_after
#[test]
fn m1_governance_configure_rejects_blackout_exceeds_max() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::DisclosureProvider;

        // MaxBlackoutDuration = 200, 设置 201 应失败
        let result = <EntityDisclosure as DisclosureProvider<u64>>::governance_configure_disclosure(
            ENTITY_ID,
            DisclosureLevel::Standard,
            true,
            201,
        );
        assert!(result.is_err());

        // 恰好 200 应成功
        let result = <EntityDisclosure as DisclosureProvider<u64>>::governance_configure_disclosure(
            ENTITY_ID,
            DisclosureLevel::Standard,
            true,
            200,
        );
        assert!(result.is_ok());

        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.blackout_period_after, 200);
    });
}

// M2: on_idle 受限于 proof_size — 极小的 proof_size 应返回零权重
#[test]
fn m2_on_idle_respects_proof_size_limit() {
    new_test_ext().execute_with(|| {
        use frame_support::traits::Hooks;

        // ref_time 充足但 proof_size 不足
        let weight = EntityDisclosure::on_idle(1, Weight::from_parts(u64::MAX, 0));
        assert_eq!(weight.ref_time(), 0);
        assert_eq!(weight.proof_size(), 0);
    });
}

// ==================== v0.6 新功能测试 ====================

// ==================== P0 #2: 多方审批签核 ====================

#[test]
fn v06_configure_approval_requirements_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_approval_requirements(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 2, 0x04, // Auditor only
        ));
        let config = ApprovalConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.required_approvals, 2);
        assert_eq!(config.allowed_roles, 0x04);

        // 设置 0 禁用审批
        assert_ok!(EntityDisclosure::configure_approval_requirements(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 0, 0,
        ));
        assert!(ApprovalConfigs::<Test>::get(ENTITY_ID).is_none());
    });
}

#[test]
fn v06_configure_approval_invalid_roles_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::configure_approval_requirements(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, 2, 0, // roles=0 invalid when required>0
            ),
            Error::<Test>::InvalidApprovalRoles
        );
    });
}

#[test]
fn v06_approve_disclosure_works() {
    new_test_ext().execute_with(|| {
        // 配置披露
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 10,
        ));

        // 配置审批要求: 需要 1 个 Auditor 审批
        assert_ok!(EntityDisclosure::configure_approval_requirements(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 1, 0x04,
        ));

        // 添加审计员作为内幕人员
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Auditor,
        ));

        // 创建草稿
        assert_ok!(EntityDisclosure::create_draft_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::QuarterlyReport, b"Qhash1".to_vec(), None,
        ));

        // 尝试未审批直接发布 → 失败
        assert_noop!(
            EntityDisclosure::publish_draft(RuntimeOrigin::signed(OWNER), 0),
            Error::<Test>::InsufficientApprovals
        );

        // Auditor 审批
        assert_ok!(EntityDisclosure::approve_disclosure(
            RuntimeOrigin::signed(ALICE), 0,
        ));
        assert_eq!(DisclosureApprovalCounts::<Test>::get(0), 1);

        // 不能重复审批
        assert_noop!(
            EntityDisclosure::approve_disclosure(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::AlreadyApproved
        );

        // 发布成功
        assert_ok!(EntityDisclosure::publish_draft(RuntimeOrigin::signed(OWNER), 0));
        let record = Disclosures::<Test>::get(0).unwrap();
        assert_eq!(record.status, DisclosureStatus::Published);

        // 审批记录已清理
        assert_eq!(DisclosureApprovalCounts::<Test>::get(0), 0);
    });
}

#[test]
fn v06_reject_disclosure_resets_approvals() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0,
        ));
        assert_ok!(EntityDisclosure::configure_approval_requirements(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 2, 0x04 | 0x01, // Auditor + Owner
        ));
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Auditor,
        ));
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, OWNER, InsiderRole::Owner,
        ));

        assert_ok!(EntityDisclosure::create_draft_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport, b"cid1".to_vec(), None,
        ));

        // Owner 审批
        assert_ok!(EntityDisclosure::approve_disclosure(RuntimeOrigin::signed(OWNER), 0));
        assert_eq!(DisclosureApprovalCounts::<Test>::get(0), 1);

        // Auditor 拒绝 → 重置
        assert_ok!(EntityDisclosure::reject_disclosure(RuntimeOrigin::signed(ALICE), 0));
        assert_eq!(DisclosureApprovalCounts::<Test>::get(0), 0);
    });
}

#[test]
fn v06_approve_not_allowed_role_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0,
        ));
        assert_ok!(EntityDisclosure::configure_approval_requirements(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 1, 0x04, // Auditor only
        ));
        // 添加 ALICE 作为 Admin（不是 Auditor）
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_ok!(EntityDisclosure::create_draft_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport, b"cid1".to_vec(), None,
        ));

        // Admin 不在允许审批角色中
        assert_noop!(
            EntityDisclosure::approve_disclosure(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::NotApprover
        );
    });
}

// ==================== P0 #1: 大股东自动注册 ====================

#[test]
fn v06_register_major_holder_works() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::DisclosureProvider;

        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 10,
        ));

        // 通过 trait 注册大股东
        assert_ok!(<EntityDisclosure as DisclosureProvider<u64>>::register_major_holder(
            ENTITY_ID, &ALICE,
        ));

        assert!(EntityDisclosure::is_insider(ENTITY_ID, &ALICE));

        let role = <EntityDisclosure as DisclosureProvider<u64>>::get_insider_role(ENTITY_ID, &ALICE);
        assert_eq!(role, Some(4)); // MajorHolder = 4

        // 重复注册不报错（幂等）
        assert_ok!(<EntityDisclosure as DisclosureProvider<u64>>::register_major_holder(
            ENTITY_ID, &ALICE,
        ));
    });
}

#[test]
fn v06_deregister_major_holder_works() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::DisclosureProvider;

        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 10,
        ));

        assert_ok!(<EntityDisclosure as DisclosureProvider<u64>>::register_major_holder(
            ENTITY_ID, &ALICE,
        ));
        assert!(EntityDisclosure::is_insider(ENTITY_ID, &ALICE));

        assert_ok!(<EntityDisclosure as DisclosureProvider<u64>>::deregister_major_holder(
            ENTITY_ID, &ALICE,
        ));
        assert!(!EntityDisclosure::is_insider(ENTITY_ID, &ALICE));

        // 冷静期生效
        assert!(RemovedInsiders::<Test>::contains_key(ENTITY_ID, &ALICE));
    });
}

#[test]
fn v06_major_holder_no_config_noop() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::DisclosureProvider;

        // 无披露配置时注册大股东应静默成功
        assert_ok!(<EntityDisclosure as DisclosureProvider<u64>>::register_major_holder(
            ENTITY_ID, &ALICE,
        ));
        // 但不应添加为内幕人员
        assert!(!EntityDisclosure::is_insider(ENTITY_ID, &ALICE));
    });
}

// ==================== P0 #3: 渐进式处罚 ====================

#[test]
fn v06_auto_penalty_escalation() {
    new_test_ext().execute_with(|| {
        use frame_support::traits::Hooks;
        use pallet_entity_common::DisclosureProvider;

        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0,
        ));

        // ViolationThreshold = 3
        // 使用 report_disclosure_violation 触发违规（通过公开接口）
        // 每次举报需要不同的逾期周期

        // 设置 3 次违规，通过 on_idle 自动检测
        for i in 0..3u64 {
            // 设置已过期的截止时间
            DisclosureConfigs::<Test>::mutate(ENTITY_ID, |c| {
                if let Some(config) = c {
                    config.next_required_disclosure = 1 + i;
                }
            });
            advance_blocks(1);

            let _ = EntityDisclosure::on_idle(
                System::block_number(),
                Weight::from_parts(u64::MAX, u64::MAX),
            );
            // 重置游标
            AutoViolationCursor::<Test>::put(0u32);
        }

        let count = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap().violation_count;
        assert_eq!(count, 3);

        // 3 次违规 >= threshold(3) → Restricted
        assert_eq!(EntityPenalties::<Test>::get(ENTITY_ID), PenaltyLevel::Restricted);
        assert!(HighRiskEntities::<Test>::get(ENTITY_ID));

        assert!(<EntityDisclosure as DisclosureProvider<u64>>::is_penalty_active(ENTITY_ID));
        assert_eq!(<EntityDisclosure as DisclosureProvider<u64>>::get_penalty_level(ENTITY_ID), 2);
    });
}

#[test]
fn v06_manual_escalate_penalty_works() {
    new_test_ext().execute_with(|| {
        // Root 手动升级
        assert_ok!(EntityDisclosure::escalate_penalty(
            RuntimeOrigin::root(), ENTITY_ID, PenaltyLevel::Warning,
        ));
        assert_eq!(EntityPenalties::<Test>::get(ENTITY_ID), PenaltyLevel::Warning);

        // 不能降级
        assert_noop!(
            EntityDisclosure::escalate_penalty(
                RuntimeOrigin::root(), ENTITY_ID, PenaltyLevel::None,
            ),
            Error::<Test>::PenaltyAlreadyAtLevel
        );

        // 可以升级
        assert_ok!(EntityDisclosure::escalate_penalty(
            RuntimeOrigin::root(), ENTITY_ID, PenaltyLevel::Suspended,
        ));
        assert_eq!(EntityPenalties::<Test>::get(ENTITY_ID), PenaltyLevel::Suspended);
    });
}

#[test]
fn v06_reset_penalty_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::escalate_penalty(
            RuntimeOrigin::root(), ENTITY_ID, PenaltyLevel::Restricted,
        ));
        assert_ok!(EntityDisclosure::reset_penalty(RuntimeOrigin::root(), ENTITY_ID));
        assert_eq!(EntityPenalties::<Test>::get(ENTITY_ID), PenaltyLevel::None);
    });
}

// ==================== P0 #4: OnEntityStatusChange ====================

#[test]
fn v06_on_entity_suspended_pauses_deadline() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::OnEntityStatusChange;

        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0,
        ));

        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        let deadline = config.next_required_disclosure;
        assert!(deadline > System::block_number());

        advance_blocks(100);

        // 模拟实体暂停
        <EntityDisclosure as OnEntityStatusChange>::on_entity_suspended(ENTITY_ID);

        assert!(PausedDeadlines::<Test>::contains_key(ENTITY_ID));
        let (_, remaining) = PausedDeadlines::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(remaining, deadline - 101); // deadline - (1 + 100)
    });
}

#[test]
fn v06_on_entity_resumed_restores_deadline() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::OnEntityStatusChange;

        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0,
        ));

        advance_blocks(100);
        <EntityDisclosure as OnEntityStatusChange>::on_entity_suspended(ENTITY_ID);

        let (_, remaining) = PausedDeadlines::<Test>::get(ENTITY_ID).unwrap();

        advance_blocks(200);
        <EntityDisclosure as OnEntityStatusChange>::on_entity_resumed(ENTITY_ID);

        assert!(!PausedDeadlines::<Test>::contains_key(ENTITY_ID));

        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        // 新截止 = 恢复时区块 + 剩余区块
        assert_eq!(config.next_required_disclosure, 301 + remaining);
    });
}

#[test]
fn v06_on_entity_closed_removes_paused() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::OnEntityStatusChange;

        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0,
        ));
        <EntityDisclosure as OnEntityStatusChange>::on_entity_suspended(ENTITY_ID);
        assert!(PausedDeadlines::<Test>::contains_key(ENTITY_ID));

        <EntityDisclosure as OnEntityStatusChange>::on_entity_closed(ENTITY_ID);
        assert!(!PausedDeadlines::<Test>::contains_key(ENTITY_ID));
    });
}

#[test]
fn v06_on_idle_skips_paused_entities() {
    new_test_ext().execute_with(|| {
        use frame_support::traits::Hooks;
        use pallet_entity_common::OnEntityStatusChange;

        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0,
        ));

        // 设置为已过期
        DisclosureConfigs::<Test>::mutate(ENTITY_ID, |c| {
            if let Some(config) = c {
                config.next_required_disclosure = 0;
            }
        });

        // 暂停实体
        <EntityDisclosure as OnEntityStatusChange>::on_entity_suspended(ENTITY_ID);

        // 运行 on_idle
        let _ = EntityDisclosure::on_idle(10, Weight::from_parts(u64::MAX, u64::MAX));

        // 不应产生违规
        let config = DisclosureConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.violation_count, 0);
    });
}

// ==================== P1 #5: 内幕人员交易申报 ====================

#[test]
fn v06_insider_transaction_report_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));

        assert_ok!(EntityDisclosure::report_insider_transaction(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
            InsiderTransactionType::Buy, 1000, 1,
        ));

        let reports = InsiderTransactionReports::<Test>::get(ENTITY_ID, &ALICE);
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].token_amount, 1000);
        assert_eq!(reports[0].transaction_type, InsiderTransactionType::Buy);
    });
}

#[test]
fn v06_insider_transaction_report_non_insider_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::report_insider_transaction(
                RuntimeOrigin::signed(ALICE), ENTITY_ID,
                InsiderTransactionType::Sell, 500, 1,
            ),
            Error::<Test>::NotInsider
        );
    });
}

#[test]
fn v06_insider_transaction_report_cooldown_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_ok!(EntityDisclosure::remove_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE,
        ));

        // ALICE 在冷静期内仍可申报
        assert_ok!(EntityDisclosure::report_insider_transaction(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
            InsiderTransactionType::Sell, 2000, 1,
        ));
    });
}

// ==================== P1 #6: 紧急披露 ====================

#[test]
fn v06_emergency_disclosure_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Full, true, 20,
        ));

        assert_ok!(EntityDisclosure::publish_emergency_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::MaterialEvent, b"emergency-cid".to_vec(), None,
        ));

        let record = Disclosures::<Test>::get(0).unwrap();
        assert_eq!(record.status, DisclosureStatus::Published);

        // 紧急元数据
        let meta = DisclosureMetadataStore::<Test>::get(0).unwrap();
        assert!(meta.is_emergency);

        // 黑窗口期 = 20 * 3 = 60（3 倍乘数）
        let (_, end) = BlackoutPeriods::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(end, 1 + 60); // now(1) + 60
    });
}

#[test]
fn v06_emergency_disclosure_caps_blackout() {
    new_test_ext().execute_with(|| {
        // MaxBlackoutDuration = 200, blackout_after = 100, multiplier = 3 → 300 → capped to 200
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Full, true, 100,
        ));

        assert_ok!(EntityDisclosure::publish_emergency_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::MaterialEvent, b"emerg".to_vec(), None,
        ));

        let (_, end) = BlackoutPeriods::<Test>::get(ENTITY_ID).unwrap();
        // 100 * 3 = 300, capped to MaxBlackoutDuration=200
        assert_eq!(end, 1 + 200);
    });
}

// ==================== P1 #7: 财务年度配置 ====================

#[test]
fn v06_configure_fiscal_year_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_fiscal_year(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 100, 1000,
        ));

        let config = FiscalYearConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.year_start_block, 100);
        assert_eq!(config.year_length, 1000);
    });
}

#[test]
fn v06_configure_fiscal_year_zero_length_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityDisclosure::configure_fiscal_year(
                RuntimeOrigin::signed(OWNER), ENTITY_ID, 100, 0,
            ),
            Error::<Test>::ZeroFiscalYearLength
        );
    });
}

// ==================== P1 #8: 披露扩展元数据 ====================

#[test]
fn v06_set_disclosure_metadata_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0,
        ));
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::QuarterlyReport, b"cid1".to_vec(), None,
        ));

        assert_ok!(EntityDisclosure::set_disclosure_metadata(
            RuntimeOrigin::signed(OWNER), 0,
            Some(100), Some(200), true,
        ));

        let meta = DisclosureMetadataStore::<Test>::get(0).unwrap();
        assert_eq!(meta.period_start, Some(100));
        assert_eq!(meta.period_end, Some(200));
        assert_eq!(meta.audit_status, AuditStatus::Pending);
        assert!(!meta.is_emergency);
    });
}

#[test]
fn v06_set_disclosure_metadata_invalid_period_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0,
        ));
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::QuarterlyReport, b"cid1".to_vec(), None,
        ));

        assert_noop!(
            EntityDisclosure::set_disclosure_metadata(
                RuntimeOrigin::signed(OWNER), 0,
                Some(200), Some(100), true, // start > end
            ),
            Error::<Test>::InvalidReportingPeriod
        );
    });
}

// ==================== P1 #8: 审计员签核 ====================

#[test]
fn v06_audit_disclosure_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0,
        ));
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Auditor,
        ));
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport, b"annual".to_vec(), None,
        ));

        // 设置需要审计
        assert_ok!(EntityDisclosure::set_disclosure_metadata(
            RuntimeOrigin::signed(OWNER), 0, None, None, true,
        ));
        assert_eq!(
            DisclosureMetadataStore::<Test>::get(0).unwrap().audit_status,
            AuditStatus::Pending,
        );

        // 审计员签核
        assert_ok!(EntityDisclosure::audit_disclosure(
            RuntimeOrigin::signed(ALICE), 0, true,
        ));
        assert_eq!(
            DisclosureMetadataStore::<Test>::get(0).unwrap().audit_status,
            AuditStatus::Approved,
        );
    });
}

#[test]
fn v06_audit_disclosure_reject_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0,
        ));
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Auditor,
        ));
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport, b"cid".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::set_disclosure_metadata(
            RuntimeOrigin::signed(OWNER), 0, None, None, true,
        ));

        assert_ok!(EntityDisclosure::audit_disclosure(
            RuntimeOrigin::signed(ALICE), 0, false,
        ));
        assert_eq!(
            DisclosureMetadataStore::<Test>::get(0).unwrap().audit_status,
            AuditStatus::Rejected,
        );
    });
}

#[test]
fn v06_audit_non_auditor_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, false, 0,
        ));
        // Add ALICE as Admin, not Auditor
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_ok!(EntityDisclosure::publish_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureType::AnnualReport, b"cid".to_vec(), None,
        ));
        assert_ok!(EntityDisclosure::set_disclosure_metadata(
            RuntimeOrigin::signed(OWNER), 0, None, None, true,
        ));

        assert_noop!(
            EntityDisclosure::audit_disclosure(RuntimeOrigin::signed(ALICE), 0, true),
            Error::<Test>::NotApprover
        );
    });
}

// ==================== P1 #9: 冷静期清理 ====================

#[test]
fn v06_cleanup_expired_cooldowns_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, BOB, InsiderRole::Advisor,
        ));

        assert_ok!(EntityDisclosure::remove_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE,
        ));
        assert_ok!(EntityDisclosure::remove_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, BOB,
        ));

        // 两个冷静期记录
        assert!(RemovedInsiders::<Test>::contains_key(ENTITY_ID, &ALICE));
        assert!(RemovedInsiders::<Test>::contains_key(ENTITY_ID, &BOB));

        // 超过冷静期
        advance_blocks(100); // InsiderCooldownPeriod = 50

        assert_ok!(EntityDisclosure::cleanup_expired_cooldowns(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
        ));

        assert!(!RemovedInsiders::<Test>::contains_key(ENTITY_ID, &ALICE));
        assert!(!RemovedInsiders::<Test>::contains_key(ENTITY_ID, &BOB));
    });
}

#[test]
fn v06_cleanup_cooldowns_not_yet_expired() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::add_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE, InsiderRole::Admin,
        ));
        assert_ok!(EntityDisclosure::remove_insider(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, ALICE,
        ));

        advance_blocks(10); // < 50

        assert_ok!(EntityDisclosure::cleanup_expired_cooldowns(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
        ));

        // 尚未过期，不应清理
        assert!(RemovedInsiders::<Test>::contains_key(ENTITY_ID, &ALICE));
    });
}

// ==================== PenaltyLevel 类型测试 ====================

#[test]
fn v06_penalty_level_ordering() {
    assert!(PenaltyLevel::None < PenaltyLevel::Warning);
    assert!(PenaltyLevel::Warning < PenaltyLevel::Restricted);
    assert!(PenaltyLevel::Restricted < PenaltyLevel::Suspended);
    assert!(PenaltyLevel::Suspended < PenaltyLevel::Delisted);
}

#[test]
fn v06_penalty_level_next() {
    assert_eq!(PenaltyLevel::None.next(), PenaltyLevel::Warning);
    assert_eq!(PenaltyLevel::Delisted.next(), PenaltyLevel::Delisted);
}

#[test]
fn v06_penalty_level_roundtrip() {
    for level in [PenaltyLevel::None, PenaltyLevel::Warning, PenaltyLevel::Restricted,
                  PenaltyLevel::Suspended, PenaltyLevel::Delisted] {
        assert_eq!(PenaltyLevel::from_u8(level.as_u8()), level);
    }
}

// ==================== 清理整合测试 ====================

#[test]
fn v06_cleanup_entity_disclosure_clears_new_storage() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityDisclosure::configure_disclosure(
            RuntimeOrigin::signed(OWNER), ENTITY_ID,
            DisclosureLevel::Standard, true, 10,
        ));
        assert_ok!(EntityDisclosure::configure_approval_requirements(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 1, 0x04,
        ));
        assert_ok!(EntityDisclosure::configure_fiscal_year(
            RuntimeOrigin::signed(OWNER), ENTITY_ID, 1, 1000,
        ));
        EntityPenalties::<Test>::insert(ENTITY_ID, PenaltyLevel::Warning);
        PausedDeadlines::<Test>::insert(ENTITY_ID, (1u64, 100u64));

        // 关闭实体后清理
        set_entity_status(ENTITY_ID, EntityStatus::Closed);

        assert_ok!(EntityDisclosure::cleanup_entity_disclosure(
            RuntimeOrigin::signed(ALICE), ENTITY_ID,
        ));

        assert!(DisclosureConfigs::<Test>::get(ENTITY_ID).is_none());
        assert!(ApprovalConfigs::<Test>::get(ENTITY_ID).is_none());
        assert!(FiscalYearConfigs::<Test>::get(ENTITY_ID).is_none());
        assert_eq!(EntityPenalties::<Test>::get(ENTITY_ID), PenaltyLevel::None);
        assert!(PausedDeadlines::<Test>::get(ENTITY_ID).is_none());
    });
}
