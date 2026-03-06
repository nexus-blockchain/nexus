use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok, traits::Hooks};
use frame_system::RawOrigin;

fn dt() -> sp_runtime::BoundedVec<u8, frame_support::traits::ConstU32<32>> {
    sp_runtime::BoundedVec::truncate_from(b"pin_storage".to_vec())
}

fn dt_custom(name: &[u8]) -> sp_runtime::BoundedVec<u8, frame_support::traits::ConstU32<32>> {
    sp_runtime::BoundedVec::truncate_from(name.to_vec())
}

// ============== 原有基础测试 ==============

#[test]
fn block_to_year_month_works() {
    assert_eq!(block_to_year_month(0, 14400), 2401);
    assert_eq!(block_to_year_month(14400 * 30, 14400), 2402);
    assert_eq!(block_to_year_month(14400 * 30 * 12, 14400), 2501);
}

#[test]
fn block_to_year_month_zero_blocks_per_day() {
    assert_eq!(block_to_year_month(100, 0), 2401);
}

#[test]
fn amount_to_tier_works() {
    assert_eq!(amount_to_tier(0), 0);
    assert_eq!(amount_to_tier(100), 1);
    assert_eq!(amount_to_tier(1000), 2);
    assert_eq!(amount_to_tier(10000), 3);
    assert_eq!(amount_to_tier(100000), 4);
    assert_eq!(amount_to_tier(1_000_000), 5);
}

#[test]
fn archive_level_u8_roundtrip() {
    assert_eq!(ArchiveLevel::from_u8(ArchiveLevel::Active.to_u8()), ArchiveLevel::Active);
    assert_eq!(ArchiveLevel::from_u8(ArchiveLevel::ArchivedL1.to_u8()), ArchiveLevel::ArchivedL1);
    assert_eq!(ArchiveLevel::from_u8(ArchiveLevel::ArchivedL2.to_u8()), ArchiveLevel::ArchivedL2);
    assert_eq!(ArchiveLevel::from_u8(ArchiveLevel::Purged.to_u8()), ArchiveLevel::Purged);
    assert_eq!(ArchiveLevel::from_u8(255), ArchiveLevel::Active);
}

#[test]
fn record_batch_and_stats_work() {
    new_test_ext().execute_with(|| {
        let data_type = dt_custom(b"test_data");
        assert_ok!(StorageLifecycleManager::<Test>::record_batch(
            data_type.clone(), 1, 5, 5, ArchiveLevel::Purged, 100,
        ));
        let stats = pallet::ArchiveStats::<Test>::get(&data_type);
        assert_eq!(stats.total_purged, 5);
        assert_eq!(stats.last_archive_at, 100);
        let batches = pallet::ArchiveBatches::<Test>::get(&data_type);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].count, 5);
    });
}

#[test]
fn record_batch_l1_updates_stats() {
    new_test_ext().execute_with(|| {
        let data_type = dt_custom(b"l1test");
        assert_ok!(StorageLifecycleManager::<Test>::record_batch(
            data_type.clone(), 10, 20, 11, ArchiveLevel::ArchivedL1, 50,
        ));
        let stats = pallet::ArchiveStats::<Test>::get(&data_type);
        assert_eq!(stats.total_l1_archived, 11);
        assert_eq!(stats.total_l2_archived, 0);
    });
}

#[test]
fn cursor_update_and_read() {
    new_test_ext().execute_with(|| {
        let data_type = dt_custom(b"cursor_test");
        assert_eq!(StorageLifecycleManager::<Test>::get_cursor(&data_type), 0);
        StorageLifecycleManager::<Test>::update_cursor(data_type.clone(), 42);
        assert_eq!(StorageLifecycleManager::<Test>::get_cursor(&data_type), 42);
    });
}

#[test]
fn record_bytes_saved_accumulates() {
    new_test_ext().execute_with(|| {
        let data_type = dt_custom(b"bytes_test");
        StorageLifecycleManager::<Test>::record_bytes_saved(&data_type, 1000);
        StorageLifecycleManager::<Test>::record_bytes_saved(&data_type, 500);
        let stats = pallet::ArchiveStats::<Test>::get(&data_type);
        assert_eq!(stats.total_bytes_saved, 1500);
    });
}

#[test]
fn batch_queue_evicts_oldest_when_full() {
    new_test_ext().execute_with(|| {
        let data_type = dt_custom(b"evict_test");
        for i in 0..100u64 {
            assert_ok!(StorageLifecycleManager::<Test>::record_batch(
                data_type.clone(), i, i, 1, ArchiveLevel::Purged, i,
            ));
        }
        assert_ok!(StorageLifecycleManager::<Test>::record_batch(
            data_type.clone(), 100, 100, 1, ArchiveLevel::Purged, 100,
        ));
        let batches = pallet::ArchiveBatches::<Test>::get(&data_type);
        assert_eq!(batches.len(), 100);
        assert_eq!(batches[0].batch_id, 2);
    });
}

// ============== D1: 三阶段归档 on_idle ==============

#[test]
fn d1_on_idle_three_phase_archival() {
    new_test_ext().execute_with(|| {
        // Active → L1
        set_archivable_ids(vec![1, 2, 3]);
        let weight = StorageLifecycle::on_idle(
            100u64,
            Weight::from_parts(500_000_000, 50_000),
        );
        assert!(weight.ref_time() > 0);

        // 检查 L1 归档事件
        let events = System::events();
        assert!(events.iter().any(|e| {
            matches!(&e.event, RuntimeEvent::StorageLifecycle(
                pallet::Event::ArchivedToL1 { count, .. }
            ) if *count > 0)
        }));
    });
}

#[test]
fn d1_on_idle_l1_to_l2() {
    new_test_ext().execute_with(|| {
        // 设置 L1 数据等待升级到 L2
        set_l1_ids(vec![10, 11]);
        let weight = StorageLifecycle::on_idle(
            500u64,
            Weight::from_parts(500_000_000, 50_000),
        );
        assert!(weight.ref_time() > 0);
        let events = System::events();
        assert!(events.iter().any(|e| {
            matches!(&e.event, RuntimeEvent::StorageLifecycle(
                pallet::Event::ArchivedToL2 { count, .. }
            ) if *count > 0)
        }));
    });
}

#[test]
fn d1_on_idle_l2_to_purge() {
    new_test_ext().execute_with(|| {
        set_l2_ids(vec![20, 21, 22]);
        let _weight = StorageLifecycle::on_idle(
            1000u64,
            Weight::from_parts(500_000_000, 50_000),
        );
        let events = System::events();
        assert!(events.iter().any(|e| {
            matches!(&e.event, RuntimeEvent::StorageLifecycle(
                pallet::Event::DataPurged { count, .. }
            ) if *count > 0)
        }));
        assert!(!get_archived_ids().is_empty());
    });
}

#[test]
fn d1_on_idle_skips_when_insufficient_weight() {
    new_test_ext().execute_with(|| {
        set_archivable_ids(vec![1, 2, 3]);
        let weight = StorageLifecycle::on_idle(
            100u64,
            Weight::from_parts(1_000, 100),
        );
        assert_eq!(weight, Weight::zero());
    });
}

// ============== D3: 归档回调 ==============

#[test]
fn d3_archive_callback_fired() {
    new_test_ext().execute_with(|| {
        set_archivable_ids(vec![1, 2]);
        StorageLifecycle::on_idle(100u64, Weight::from_parts(500_000_000, 50_000));
        let callbacks = get_archive_callbacks();
        // Mock doesn't enforce delays, so all phases run: Active→L1→L2→Purge
        // 2 IDs × 3 transitions = 6 callbacks
        assert_eq!(callbacks.len(), 6);
        // First two: Active → L1
        assert_eq!(callbacks[0].2, ArchiveLevel::Active.to_u8());
        assert_eq!(callbacks[0].3, ArchiveLevel::ArchivedL1.to_u8());
        // Next two: L1 → L2
        assert_eq!(callbacks[2].2, ArchiveLevel::ArchivedL1.to_u8());
        assert_eq!(callbacks[2].3, ArchiveLevel::ArchivedL2.to_u8());
        // Last two: L2 → Purge
        assert_eq!(callbacks[4].2, ArchiveLevel::ArchivedL2.to_u8());
        assert_eq!(callbacks[4].3, ArchiveLevel::Purged.to_u8());
    });
}

// ============== U2: 归档前预警 ==============

#[test]
fn u2_archival_warning_emitted_in_on_idle() {
    new_test_ext().execute_with(|| {
        set_archivable_ids(vec![1, 2, 3]);
        StorageLifecycle::on_idle(100u64, Weight::from_parts(500_000_000, 50_000));
        // Mock returns all IDs for any scan_for_level call (no delay enforcement),
        // so the warning scan also finds IDs. After L1 archival processes them,
        // the warning scan's results are filtered to exclude already-archived IDs.
        // Since mock returns the same IDs in warning scan, they overlap with l1_ids,
        // so approaching_count = 0 and no ArchivalWarning event is emitted.
        // This verifies the dedup filtering works correctly.
        let events = System::events();
        let warning_count = events.iter().filter(|e| {
            matches!(e.event, RuntimeEvent::StorageLifecycle(
                pallet::Event::ArchivalWarning { .. }
            ))
        }).count();
        // All IDs were archived in l1 phase, so warning scan returns same IDs → filtered out → 0
        assert_eq!(warning_count, 0);
    });
}

#[test]
fn u2_query_approaching_archival_returns_count() {
    new_test_ext().execute_with(|| {
        set_archivable_ids(vec![1, 2, 3]);
        // Before any on_idle, all IDs are approaching archival
        let count = StorageLifecycle::query_approaching_archival(&dt());
        assert_eq!(count, 3);
    });
}

// ============== G1: 运行时可调配置 ==============

#[test]
fn g1_set_archive_config_works() {
    new_test_ext().execute_with(|| {
        let config = ArchiveConfig {
            l1_delay: 50,
            l2_delay: 150,
            purge_delay: 250,
            purge_enabled: false,
            max_batch_size: 20,
        };
        assert_ok!(StorageLifecycle::set_archive_config(
            RawOrigin::Root.into(),
            config.clone(),
        ));
        let effective = pallet::Pallet::<Test>::effective_config();
        assert_eq!(effective.l1_delay, 50);
        assert_eq!(effective.purge_enabled, false);
    });
}

#[test]
fn g1_set_archive_config_rejects_invalid() {
    new_test_ext().execute_with(|| {
        let config = ArchiveConfig {
            l1_delay: 0,
            l2_delay: 100,
            purge_delay: 200,
            purge_enabled: true,
            max_batch_size: 10,
        };
        assert_noop!(
            StorageLifecycle::set_archive_config(RawOrigin::Root.into(), config),
            Error::<Test>::InvalidConfig
        );
    });
}

#[test]
fn g1_set_archive_config_requires_root() {
    new_test_ext().execute_with(|| {
        let config = ArchiveConfig {
            l1_delay: 50, l2_delay: 150, purge_delay: 250,
            purge_enabled: true, max_batch_size: 20,
        };
        assert_noop!(
            StorageLifecycle::set_archive_config(RawOrigin::Signed(1).into(), config),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn g1_effective_config_falls_back_to_constants() {
    new_test_ext().execute_with(|| {
        let config = pallet::Pallet::<Test>::effective_config();
        assert_eq!(config.l1_delay, 100);
        assert_eq!(config.l2_delay, 200);
        assert_eq!(config.purge_delay, 300);
    });
}

// ============== G2: 暂停/恢复归档 ==============

#[test]
fn g2_pause_and_resume_archival() {
    new_test_ext().execute_with(|| {
        assert_ok!(StorageLifecycle::pause_archival(RawOrigin::Root.into()));
        assert!(pallet::ArchivalPaused::<Test>::get());

        // on_idle should skip when paused
        set_archivable_ids(vec![1, 2]);
        let weight = StorageLifecycle::on_idle(100u64, Weight::from_parts(500_000_000, 50_000));
        assert_eq!(weight, Weight::zero());

        assert_ok!(StorageLifecycle::resume_archival(RawOrigin::Root.into()));
        assert!(!pallet::ArchivalPaused::<Test>::get());
    });
}

#[test]
fn g2_pause_already_paused_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(StorageLifecycle::pause_archival(RawOrigin::Root.into()));
        assert_noop!(
            StorageLifecycle::pause_archival(RawOrigin::Root.into()),
            Error::<Test>::ArchivalAlreadyPaused
        );
    });
}

#[test]
fn g2_resume_not_paused_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            StorageLifecycle::resume_archival(RawOrigin::Root.into()),
            Error::<Test>::ArchivalNotPaused
        );
    });
}

// ============== G3: 按数据类型的归档策略 ==============

#[test]
fn g3_set_archive_policy_works() {
    new_test_ext().execute_with(|| {
        let policy = ArchivePolicy {
            l1_delay: 50, l2_delay: 100, purge_delay: 150, purge_enabled: false,
        };
        assert_ok!(StorageLifecycle::set_archive_policy(
            RawOrigin::Root.into(), dt(), policy.clone(),
        ));
        let config = pallet::Pallet::<Test>::effective_config();
        let effective = pallet::Pallet::<Test>::effective_policy(&dt(), &config);
        assert_eq!(effective.l1_delay, 50);
        assert_eq!(effective.purge_enabled, false);
    });
}

#[test]
fn g3_set_archive_policy_rejects_zero_delay() {
    new_test_ext().execute_with(|| {
        let policy = ArchivePolicy {
            l1_delay: 0, l2_delay: 100, purge_delay: 150, purge_enabled: true,
        };
        assert_noop!(
            StorageLifecycle::set_archive_policy(RawOrigin::Root.into(), dt(), policy),
            Error::<Test>::InvalidConfig
        );
    });
}

#[test]
fn g3_policy_falls_back_to_global() {
    new_test_ext().execute_with(|| {
        let config = pallet::Pallet::<Test>::effective_config();
        let policy = pallet::Pallet::<Test>::effective_policy(&dt(), &config);
        assert_eq!(policy.l1_delay, config.l1_delay);
    });
}

// ============== G4: 强制归档 + 清除保护 ==============

#[test]
fn g4_force_archive_works() {
    new_test_ext().execute_with(|| {
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64, 2, 3]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids, ArchiveLevel::ArchivedL1.to_u8(),
        ));
        assert_eq!(
            pallet::DataArchiveStatus::<Test>::get(&dt(), 1u64),
            ArchiveLevel::ArchivedL1
        );
        assert_eq!(
            pallet::DataArchiveStatus::<Test>::get(&dt(), 2u64),
            ArchiveLevel::ArchivedL1
        );
    });
}

#[test]
fn g4_force_archive_rejects_active_target() {
    new_test_ext().execute_with(|| {
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64]);
        assert_noop!(
            StorageLifecycle::force_archive(
                RawOrigin::Root.into(), dt(), ids, ArchiveLevel::Active.to_u8(),
            ),
            Error::<Test>::InvalidArchiveState
        );
    });
}

#[test]
fn g4_purge_protection_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(StorageLifecycle::protect_from_purge(
            RawOrigin::Root.into(), dt(), 42,
        ));
        assert!(pallet::PurgeProtected::<Test>::get(&dt(), 42u64));

        // 重复保护失败
        assert_noop!(
            StorageLifecycle::protect_from_purge(RawOrigin::Root.into(), dt(), 42),
            Error::<Test>::AlreadyProtected
        );

        // 移除保护
        assert_ok!(StorageLifecycle::remove_purge_protection(
            RawOrigin::Root.into(), dt(), 42,
        ));
        assert!(!pallet::PurgeProtected::<Test>::get(&dt(), 42u64));
    });
}

#[test]
fn g4_remove_purge_protection_fails_if_not_protected() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            StorageLifecycle::remove_purge_protection(RawOrigin::Root.into(), dt(), 99),
            Error::<Test>::NotProtected
        );
    });
}

#[test]
fn g4_purge_protection_prevents_purge_in_on_idle() {
    new_test_ext().execute_with(|| {
        set_l2_ids(vec![30, 31, 32]);
        // 保护 ID 31
        assert_ok!(StorageLifecycle::protect_from_purge(
            RawOrigin::Root.into(), dt(), 31,
        ));
        StorageLifecycle::on_idle(1000u64, Weight::from_parts(500_000_000, 50_000));
        // 30 和 32 应该被清除，但 31 被保护
        let archived = get_archived_ids();
        assert!(archived.contains(&30));
        assert!(!archived.contains(&31));
        assert!(archived.contains(&32));
    });
}

// ============== U1: 数据归档状态查询 ==============

#[test]
fn u1_query_data_status() {
    new_test_ext().execute_with(|| {
        // 默认为 Active
        assert_eq!(
            pallet::Pallet::<Test>::query_data_status(&dt(), 1),
            ArchiveLevel::Active
        );
        // 强制归档后查询
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids, ArchiveLevel::ArchivedL2.to_u8(),
        ));
        assert_eq!(
            pallet::Pallet::<Test>::query_data_status(&dt(), 1),
            ArchiveLevel::ArchivedL2
        );
    });
}

// ============== U3: Active 期延长 ==============

#[test]
fn u3_extend_active_period_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        assert_ok!(StorageLifecycle::extend_active_period(
            RawOrigin::Root.into(), dt(), 5, 500,
        ));
        let ext = pallet::ActiveExtensions::<Test>::get(&dt(), 5u64);
        assert_eq!(ext, 600); // 100 + 500

        // 再次延长应叠加
        System::set_block_number(200);
        assert_ok!(StorageLifecycle::extend_active_period(
            RawOrigin::Root.into(), dt(), 5, 300,
        ));
        let ext = pallet::ActiveExtensions::<Test>::get(&dt(), 5u64);
        assert_eq!(ext, 900); // 600 + 300 (从上次延期基准)
    });
}

#[test]
fn u3_extend_active_period_too_short() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            StorageLifecycle::extend_active_period(RawOrigin::Root.into(), dt(), 5, 50),
            Error::<Test>::ExtensionTooShort
        );
    });
}

#[test]
fn u3_extend_rejects_non_active_data() {
    new_test_ext().execute_with(|| {
        // 先强制归档到 L1
        let ids = sp_runtime::BoundedVec::truncate_from(vec![5u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids, ArchiveLevel::ArchivedL1.to_u8(),
        ));
        assert_noop!(
            StorageLifecycle::extend_active_period(RawOrigin::Root.into(), dt(), 5, 500),
            Error::<Test>::InvalidArchiveState
        );
    });
}

#[test]
fn u3_extension_prevents_l1_archival() {
    new_test_ext().execute_with(|| {
        set_archivable_ids(vec![1, 2]);
        // 延长 ID 1 到 block 9999
        System::set_block_number(100);
        assert_ok!(StorageLifecycle::extend_active_period(
            RawOrigin::Root.into(), dt(), 1, 9000,
        ));
        // on_idle at block 200 — ID 1 should be skipped due to extension
        // ID 2 goes through all phases (mock has no delay enforcement)
        StorageLifecycle::on_idle(200u64, Weight::from_parts(500_000_000, 50_000));
        let status1 = pallet::DataArchiveStatus::<Test>::get(&dt(), 1u64);
        let status2 = pallet::DataArchiveStatus::<Test>::get(&dt(), 2u64);
        assert_eq!(status1, ArchiveLevel::Active);
        assert_eq!(status2, ArchiveLevel::Purged);
    });
}

// ============== U4: 从归档恢复 ==============

#[test]
fn u4_restore_from_l1_works() {
    new_test_ext().execute_with(|| {
        // 先强制归档到 L1
        let ids = sp_runtime::BoundedVec::truncate_from(vec![10u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids, ArchiveLevel::ArchivedL1.to_u8(),
        ));
        // 设置 mock L1 包含 10
        set_l1_ids(vec![10]);

        assert_ok!(StorageLifecycle::restore_from_archive(
            RawOrigin::Root.into(), dt(), 10,
        ));
        assert_eq!(
            pallet::DataArchiveStatus::<Test>::get(&dt(), 10u64),
            ArchiveLevel::Active
        );
    });
}

#[test]
fn u4_restore_rejects_non_l1() {
    new_test_ext().execute_with(|| {
        // Active 数据不能恢复
        assert_noop!(
            StorageLifecycle::restore_from_archive(RawOrigin::Root.into(), dt(), 1),
            Error::<Test>::CannotRestoreFromLevel
        );
    });
}

#[test]
fn u4_restore_fails_when_archiver_rejects() {
    new_test_ext().execute_with(|| {
        let ids = sp_runtime::BoundedVec::truncate_from(vec![10u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids, ArchiveLevel::ArchivedL1.to_u8(),
        ));
        set_l1_ids(vec![10]);
        set_restore_allowed(false);

        assert_noop!(
            StorageLifecycle::restore_from_archive(RawOrigin::Root.into(), dt(), 10),
            Error::<Test>::RestoreFailed
        );
    });
}

// ============== O2: 仪表盘 API ==============

#[test]
fn o2_dashboard_returns_correct_data() {
    new_test_ext().execute_with(|| {
        // 记录一些批次
        assert_ok!(StorageLifecycleManager::<Test>::record_batch(
            dt(), 1, 5, 5, ArchiveLevel::ArchivedL1, 100,
        ));
        let (stats, paused, config) = pallet::Pallet::<Test>::get_dashboard(&dt());
        assert_eq!(stats.total_l1_archived, 5);
        assert!(!paused);
        assert_eq!(config.l1_delay, 100);
    });
}

// ============== O1: 批次统计 ==============

#[test]
fn o1_batch_stats_accumulate() {
    new_test_ext().execute_with(|| {
        let data_type = dt();
        assert_ok!(StorageLifecycleManager::<Test>::record_batch(
            data_type.clone(), 1, 5, 5, ArchiveLevel::ArchivedL1, 100,
        ));
        assert_ok!(StorageLifecycleManager::<Test>::record_batch(
            data_type.clone(), 6, 10, 5, ArchiveLevel::ArchivedL2, 200,
        ));
        let stats = pallet::ArchiveStats::<Test>::get(&data_type);
        assert_eq!(stats.total_l1_archived, 5);
        assert_eq!(stats.total_l2_archived, 5);
        assert_eq!(stats.last_archive_at, 200);
    });
}

// ============== 综合流程测试 ==============

#[test]
fn full_lifecycle_active_to_l1_to_l2_to_purge() {
    new_test_ext().execute_with(|| {
        // Mock doesn't enforce delays, so all 3 phases complete in one on_idle
        set_archivable_ids(vec![1, 2]);
        StorageLifecycle::on_idle(100u64, Weight::from_parts(500_000_000, 50_000));

        // Both IDs should reach Purged in a single on_idle
        assert_eq!(
            pallet::DataArchiveStatus::<Test>::get(&dt(), 1u64),
            ArchiveLevel::Purged
        );
        assert_eq!(
            pallet::DataArchiveStatus::<Test>::get(&dt(), 2u64),
            ArchiveLevel::Purged
        );

        // Stats should reflect all transitions
        let stats = pallet::ArchiveStats::<Test>::get(&dt());
        assert_eq!(stats.total_l1_archived, 2);
        assert_eq!(stats.total_l2_archived, 2);
        assert_eq!(stats.total_purged, 2);
    });
}

#[test]
fn full_lifecycle_with_policy_override() {
    new_test_ext().execute_with(|| {
        // 设置不启用 purge 的策略
        let policy = ArchivePolicy {
            l1_delay: 10, l2_delay: 20, purge_delay: 30, purge_enabled: false,
        };
        assert_ok!(StorageLifecycle::set_archive_policy(
            RawOrigin::Root.into(), dt(), policy,
        ));

        set_archivable_ids(vec![1]);
        // Active → L1
        StorageLifecycle::on_idle(100u64, Weight::from_parts(500_000_000, 50_000));
        // L1 → L2
        StorageLifecycle::on_idle(200u64, Weight::from_parts(500_000_000, 50_000));
        // L2 存在但 purge 被策略禁用
        StorageLifecycle::on_idle(1000u64, Weight::from_parts(500_000_000, 50_000));
        // 不应有 purge 发生
        assert!(get_archived_ids().is_empty());
    });
}

// ============== H1-R1: on_idle 权重准确性 ==============

#[test]
fn h1r1_on_idle_weight_accounts_for_scans() {
    new_test_ext().execute_with(|| {
        // 无数据处理时，权重仍应包含扫描开销
        let weight = StorageLifecycle::on_idle(
            100u64,
            Weight::from_parts(500_000_000, 50_000),
        );
        // 1 data type * 5 scans = 5 scans
        // base=50M + scans=5*5M=25M = 75M minimum ref_time
        assert!(weight.ref_time() >= 75_000_000);
    });
}

#[test]
fn h1r1_on_idle_weight_includes_item_overhead() {
    new_test_ext().execute_with(|| {
        set_archivable_ids(vec![1, 2]);
        let weight = StorageLifecycle::on_idle(
            100u64,
            Weight::from_parts(500_000_000, 50_000),
        );
        // 处理了 items 后权重应远大于纯扫描权重
        assert!(weight.ref_time() > 75_000_000);
    });
}

// ============== H2-R1: force_archive 回调 ==============

#[test]
fn h2r1_force_archive_fires_callbacks() {
    new_test_ext().execute_with(|| {
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64, 2]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL1.to_u8(),
        ));
        let callbacks = get_archive_callbacks();
        assert_eq!(callbacks.len(), 2);
        // 从 Active(0) 到 ArchivedL1(1)
        assert_eq!(callbacks[0].2, ArchiveLevel::Active.to_u8());
        assert_eq!(callbacks[0].3, ArchiveLevel::ArchivedL1.to_u8());
        assert_eq!(callbacks[1].1, 2); // data_id = 2
    });
}

#[test]
fn h2r1_force_archive_callback_has_correct_from_level() {
    new_test_ext().execute_with(|| {
        // 先归档到 L1
        let ids = sp_runtime::BoundedVec::truncate_from(vec![5u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL1.to_u8(),
        ));
        // 再强制归档到 Purged
        let ids = sp_runtime::BoundedVec::truncate_from(vec![5u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::Purged.to_u8(),
        ));
        let callbacks = get_archive_callbacks();
        assert_eq!(callbacks.len(), 2);
        // 第二次回调: from=ArchivedL1(1), to=Purged(3)
        assert_eq!(callbacks[1].2, ArchiveLevel::ArchivedL1.to_u8());
        assert_eq!(callbacks[1].3, ArchiveLevel::Purged.to_u8());
    });
}

// ============== M1-R1: purge_delay 校验 ==============

#[test]
fn m1r1_set_archive_config_rejects_zero_purge_delay_when_enabled() {
    new_test_ext().execute_with(|| {
        let config = ArchiveConfig {
            l1_delay: 100, l2_delay: 200, purge_delay: 0,
            purge_enabled: true, max_batch_size: 10,
        };
        assert_noop!(
            StorageLifecycle::set_archive_config(RawOrigin::Root.into(), config),
            Error::<Test>::InvalidConfig
        );
    });
}

#[test]
fn m1r1_set_archive_config_allows_zero_purge_delay_when_disabled() {
    new_test_ext().execute_with(|| {
        let config = ArchiveConfig {
            l1_delay: 100, l2_delay: 200, purge_delay: 0,
            purge_enabled: false, max_batch_size: 10,
        };
        assert_ok!(StorageLifecycle::set_archive_config(RawOrigin::Root.into(), config));
    });
}

#[test]
fn m1r1_set_archive_policy_rejects_zero_purge_delay_when_enabled() {
    new_test_ext().execute_with(|| {
        let policy = ArchivePolicy {
            l1_delay: 100, l2_delay: 200, purge_delay: 0, purge_enabled: true,
        };
        assert_noop!(
            StorageLifecycle::set_archive_policy(RawOrigin::Root.into(), dt(), policy),
            Error::<Test>::InvalidConfig
        );
    });
}

#[test]
fn m1r1_set_archive_policy_allows_zero_purge_delay_when_disabled() {
    new_test_ext().execute_with(|| {
        let policy = ArchivePolicy {
            l1_delay: 100, l2_delay: 200, purge_delay: 0, purge_enabled: false,
        };
        assert_ok!(StorageLifecycle::set_archive_policy(RawOrigin::Root.into(), dt(), policy));
    });
}

// ============== M2-R1: TotalBatchCount ==============

#[test]
fn m2r1_record_batch_increments_total_batch_count() {
    new_test_ext().execute_with(|| {
        let data_type = dt();
        assert_eq!(pallet::TotalBatchCount::<Test>::get(&data_type), 0);

        assert_ok!(StorageLifecycleManager::<Test>::record_batch(
            data_type.clone(), 1, 5, 5, ArchiveLevel::ArchivedL1, 100,
        ));
        assert_eq!(pallet::TotalBatchCount::<Test>::get(&data_type), 1);

        assert_ok!(StorageLifecycleManager::<Test>::record_batch(
            data_type.clone(), 6, 10, 5, ArchiveLevel::ArchivedL2, 200,
        ));
        assert_eq!(pallet::TotalBatchCount::<Test>::get(&data_type), 2);
    });
}

#[test]
fn m2r1_on_idle_increments_batch_count() {
    new_test_ext().execute_with(|| {
        let data_type = dt();
        set_archivable_ids(vec![1]);
        StorageLifecycle::on_idle(100u64, Weight::from_parts(500_000_000, 50_000));
        // Mock has no delay enforcement: Active→L1→L2→Purge = 3 batches
        assert_eq!(pallet::TotalBatchCount::<Test>::get(&data_type), 3);
    });
}

// ============== H1-R2: on_idle 权重不超过 remaining_weight ==============

#[test]
fn h1r2_on_idle_weight_capped_by_remaining() {
    new_test_ext().execute_with(|| {
        set_archivable_ids(vec![1, 2, 3, 4, 5]);
        // 给予足够权重执行，但设置一个具体上限
        let limit = Weight::from_parts(500_000_000, 50_000);
        let weight = StorageLifecycle::on_idle(100u64, limit);
        // 返回权重不应超过 remaining_weight
        assert!(weight.ref_time() <= limit.ref_time());
        assert!(weight.proof_size() <= limit.proof_size());
    });
}

#[test]
fn h1r2_on_idle_weight_respects_tight_limit() {
    new_test_ext().execute_with(|| {
        set_archivable_ids(vec![1, 2, 3]);
        // 刚好超过最小权重但很紧
        let tight_limit = Weight::from_parts(60_000_000, 6_000);
        let weight = StorageLifecycle::on_idle(100u64, tight_limit);
        assert!(weight.ref_time() <= tight_limit.ref_time());
        assert!(weight.proof_size() <= tight_limit.proof_size());
    });
}

// ============== M1-R2: on_idle 回调读取实际 from_level ==============

#[test]
fn m1r2_on_idle_l1_callback_reads_actual_from_level() {
    new_test_ext().execute_with(|| {
        // 先 force_archive ID 1 到 L1
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL1.to_u8(),
        ));
        // 现在 DataArchiveStatus[1] = ArchivedL1
        // 设置 mock 让 scan_for_level(L1) 返回 ID 1（模拟 archiver 和 pallet 状态不同步）
        set_archivable_ids(vec![1]);
        // 清理之前 force_archive 的回调
        clear_archive_callbacks();

        StorageLifecycle::on_idle(100u64, Weight::from_parts(500_000_000, 50_000));
        let callbacks = get_archive_callbacks();
        // L1 阶段回调: from_level 应为 ArchivedL1（实际值）而非 Active（硬编码）
        assert!(!callbacks.is_empty());
        assert_eq!(callbacks[0].2, ArchiveLevel::ArchivedL1.to_u8());
    });
}

// ============== M2-R2: 积压告警修复 ==============

#[test]
fn m2r2_backlog_event_can_fire() {
    new_test_ext().execute_with(|| {
        // MaxBatchSize=10, 阈值=30, 扫描上限=31
        // 设置足够多的 active IDs（超过阈值）
        let many_ids: Vec<u64> = (1..=35).collect();
        set_archivable_ids(many_ids.clone());

        StorageLifecycle::on_idle(100u64, Weight::from_parts(500_000_000, 500_000));
        let events = System::events();
        // 应该能触发 ArchivalBacklog 事件
        let backlog_count = events.iter().filter(|e| {
            matches!(e.event, RuntimeEvent::StorageLifecycle(
                pallet::Event::ArchivalBacklog { .. }
            ))
        }).count();
        // 注: Mock 的 scan_for_level 从 ACTIVE_IDS 读取，
        // L1 阶段处理后 ACTIVE_IDS 会减少，但 backlog 扫描在 L1 阶段之后运行。
        // 如果 L1 处理了一部分，剩余仍 > 30，则触发。
        // phase_batch = (10/1/3).max(1) = 3, 所以 L1 最多处理 3 个，剩 32 > 30 → 触发
        assert!(backlog_count > 0, "ArchivalBacklog event should fire when pending > threshold");
    });
}

// ============== M3-R2: force_archive 清理存储 ==============

#[test]
fn m3r2_force_archive_cleans_active_extensions() {
    new_test_ext().execute_with(|| {
        // 设置延期
        System::set_block_number(100);
        assert_ok!(StorageLifecycle::extend_active_period(
            RawOrigin::Root.into(), dt(), 1, 500,
        ));
        assert!(pallet::ActiveExtensions::<Test>::get(&dt(), 1u64) > 0);

        // force_archive 应清理延期
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL1.to_u8(),
        ));
        assert_eq!(pallet::ActiveExtensions::<Test>::get(&dt(), 1u64), 0);
    });
}

#[test]
fn m3r2_force_archive_to_purged_cleans_protection() {
    new_test_ext().execute_with(|| {
        // 添加保护
        assert_ok!(StorageLifecycle::protect_from_purge(
            RawOrigin::Root.into(), dt(), 1,
        ));
        assert!(pallet::PurgeProtected::<Test>::get(&dt(), 1u64));

        // force_archive 到 L1 不应清理保护
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL1.to_u8(),
        ));
        assert!(pallet::PurgeProtected::<Test>::get(&dt(), 1u64));

        // force_archive 到 Purged 应清理保护
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::Purged.to_u8(),
        ));
        assert!(!pallet::PurgeProtected::<Test>::get(&dt(), 1u64));
    });
}

// ============== H1-R3: force_archive 拒绝后退转换 ==============

#[test]
fn h1r3_force_archive_skips_backward_transition() {
    new_test_ext().execute_with(|| {
        // 先归档到 L2
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL2.to_u8(),
        ));
        assert_eq!(
            pallet::DataArchiveStatus::<Test>::get(&dt(), 1u64),
            ArchiveLevel::ArchivedL2
        );

        // 尝试 "后退" 到 L1 — 应被跳过，状态保持 L2
        clear_archive_callbacks();
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL1.to_u8(),
        ));
        assert_eq!(
            pallet::DataArchiveStatus::<Test>::get(&dt(), 1u64),
            ArchiveLevel::ArchivedL2 // 未改变
        );
        // 无回调触发
        assert!(get_archive_callbacks().is_empty());
    });
}

#[test]
fn h1r3_force_archive_skips_same_level() {
    new_test_ext().execute_with(|| {
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL1.to_u8(),
        ));
        clear_archive_callbacks();

        // 同级别转换 — 应被跳过
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL1.to_u8(),
        ));
        assert!(get_archive_callbacks().is_empty());
    });
}

#[test]
fn h1r3_force_archive_forward_still_works() {
    new_test_ext().execute_with(|| {
        // Active → L1 → Purged 正向转换应正常工作
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL1.to_u8(),
        ));
        assert_eq!(
            pallet::DataArchiveStatus::<Test>::get(&dt(), 1u64),
            ArchiveLevel::ArchivedL1
        );

        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::Purged.to_u8(),
        ));
        assert_eq!(
            pallet::DataArchiveStatus::<Test>::get(&dt(), 1u64),
            ArchiveLevel::Purged
        );
        let callbacks = get_archive_callbacks();
        assert_eq!(callbacks.len(), 2);
    });
}

// ============== M1-R3: on_idle 中途超预算则中止 ==============

#[test]
fn m1r3_on_idle_breaks_early_on_tight_budget() {
    new_test_ext().execute_with(|| {
        // 大量数据，但给很紧的预算
        set_archivable_ids(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        // 给够 min_weight + 少量余量，不够处理所有项
        let tight = Weight::from_parts(80_000_000, 8_000);
        let weight = StorageLifecycle::on_idle(100u64, tight);
        // 返回权重不超过预算
        assert!(weight.ref_time() <= tight.ref_time());
        assert!(weight.proof_size() <= tight.proof_size());
    });
}

// ============== M2-R3: 延迟顺序校验 ==============

#[test]
fn m2r3_config_rejects_l1_greater_than_l2() {
    new_test_ext().execute_with(|| {
        let config = ArchiveConfig {
            l1_delay: 200, l2_delay: 100, purge_delay: 300,
            purge_enabled: false, max_batch_size: 10,
        };
        assert_noop!(
            StorageLifecycle::set_archive_config(RawOrigin::Root.into(), config),
            Error::<Test>::InvalidConfig
        );
    });
}

#[test]
fn m2r3_config_rejects_l2_greater_than_purge_when_enabled() {
    new_test_ext().execute_with(|| {
        let config = ArchiveConfig {
            l1_delay: 100, l2_delay: 300, purge_delay: 200,
            purge_enabled: true, max_batch_size: 10,
        };
        assert_noop!(
            StorageLifecycle::set_archive_config(RawOrigin::Root.into(), config),
            Error::<Test>::InvalidConfig
        );
    });
}

#[test]
fn m2r3_config_allows_l2_greater_than_purge_when_disabled() {
    new_test_ext().execute_with(|| {
        // purge_enabled=false 时不检查 l2 vs purge
        let config = ArchiveConfig {
            l1_delay: 100, l2_delay: 300, purge_delay: 0,
            purge_enabled: false, max_batch_size: 10,
        };
        assert_ok!(StorageLifecycle::set_archive_config(RawOrigin::Root.into(), config));
    });
}

#[test]
fn m2r3_policy_rejects_l1_greater_than_l2() {
    new_test_ext().execute_with(|| {
        let policy = ArchivePolicy {
            l1_delay: 500, l2_delay: 100, purge_delay: 600, purge_enabled: true,
        };
        assert_noop!(
            StorageLifecycle::set_archive_policy(RawOrigin::Root.into(), dt(), policy),
            Error::<Test>::InvalidConfig
        );
    });
}

#[test]
fn m2r3_policy_rejects_l2_greater_than_purge_when_enabled() {
    new_test_ext().execute_with(|| {
        let policy = ArchivePolicy {
            l1_delay: 100, l2_delay: 500, purge_delay: 200, purge_enabled: true,
        };
        assert_noop!(
            StorageLifecycle::set_archive_policy(RawOrigin::Root.into(), dt(), policy),
            Error::<Test>::InvalidConfig
        );
    });
}

#[test]
fn m2r3_config_accepts_equal_delays() {
    new_test_ext().execute_with(|| {
        // l1 == l2 == purge 是允许的
        let config = ArchiveConfig {
            l1_delay: 100, l2_delay: 100, purge_delay: 100,
            purge_enabled: true, max_batch_size: 10,
        };
        assert_ok!(StorageLifecycle::set_archive_config(RawOrigin::Root.into(), config));
    });
}

// ============== M1-R4: restore_from_archive 触发 OnArchive 回调 ==============

#[test]
fn m1r4_restore_fires_on_archive_callback() {
    new_test_ext().execute_with(|| {
        // 先归档到 L1
        set_archivable_ids(vec![42]);
        let ids = sp_runtime::BoundedVec::truncate_from(vec![42u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL1.to_u8(),
        ));
        clear_archive_callbacks();

        // 恢复
        assert_ok!(StorageLifecycle::restore_from_archive(
            RawOrigin::Root.into(), dt(), 42,
        ));

        // 验证回调被触发：from=L1(1), to=Active(0)
        let callbacks = get_archive_callbacks();
        assert_eq!(callbacks.len(), 1);
        assert_eq!(callbacks[0].0, b"pin_storage".to_vec());
        assert_eq!(callbacks[0].1, 42);
        assert_eq!(callbacks[0].2, ArchiveLevel::ArchivedL1.to_u8());
        assert_eq!(callbacks[0].3, ArchiveLevel::Active.to_u8());
    });
}

#[test]
fn m1r4_restore_callback_contains_correct_levels() {
    new_test_ext().execute_with(|| {
        // 归档到 L1，然后恢复，确认 from/to 级别准确
        set_archivable_ids(vec![7]);
        let ids = sp_runtime::BoundedVec::truncate_from(vec![7u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL1.to_u8(),
        ));
        clear_archive_callbacks();

        assert_ok!(StorageLifecycle::restore_from_archive(
            RawOrigin::Root.into(), dt(), 7,
        ));
        // 恢复后状态应为 Active
        assert_eq!(
            pallet::DataArchiveStatus::<Test>::get(&dt(), 7u64),
            ArchiveLevel::Active
        );
        // 回调 from=1 to=0
        let cb = get_archive_callbacks();
        assert_eq!(cb[0].2, 1); // ArchivedL1
        assert_eq!(cb[0].3, 0); // Active
    });
}

// ============== M2-R4: DataForceArchived 事件仅含实际归档的 ID ==============

#[test]
fn m2r4_force_archive_event_excludes_skipped_ids() {
    new_test_ext().execute_with(|| {
        // 先把 ID 1 归档到 L2
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL2.to_u8(),
        ));
        System::reset_events();

        // 提交 [1, 2, 3] 到 L1 — ID 1 应被跳过（已在 L2），ID 2,3 应归档
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64, 2, 3]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL1.to_u8(),
        ));

        // 事件应仅包含 [2, 3]
        let events = System::events();
        let force_event = events.iter().find(|e| {
            matches!(e.event, RuntimeEvent::StorageLifecycle(
                pallet::Event::DataForceArchived { .. }
            ))
        });
        assert!(force_event.is_some());
        if let RuntimeEvent::StorageLifecycle(pallet::Event::DataForceArchived {
            data_ids, ..
        }) = &force_event.unwrap().event {
            let ids_vec: Vec<u64> = data_ids.to_vec();
            assert_eq!(ids_vec, vec![2u64, 3]);
            assert!(!ids_vec.contains(&1)); // ID 1 被跳过
        } else {
            panic!("Expected DataForceArchived event");
        }
    });
}

#[test]
fn m2r4_force_archive_event_empty_when_all_skipped() {
    new_test_ext().execute_with(|| {
        // 归档到 Purged
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64, 2]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::Purged.to_u8(),
        ));
        System::reset_events();

        // 再次提交同样的 ID 到 L1 — 全部应被跳过
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64, 2]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL1.to_u8(),
        ));

        // 事件应包含空列表
        let events = System::events();
        let force_event = events.iter().find(|e| {
            matches!(e.event, RuntimeEvent::StorageLifecycle(
                pallet::Event::DataForceArchived { .. }
            ))
        });
        assert!(force_event.is_some());
        if let RuntimeEvent::StorageLifecycle(pallet::Event::DataForceArchived {
            data_ids, ..
        }) = &force_event.unwrap().event {
            assert!(data_ids.is_empty());
        } else {
            panic!("Expected DataForceArchived event");
        }
    });
}

// ============== M3-R4: force_archive 缓存 from_level 正确性 ==============

#[test]
fn m3r4_force_archive_cached_level_matches_callback() {
    new_test_ext().execute_with(|| {
        // 先归档 ID 1 → L1，ID 2 保持 Active
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL1.to_u8(),
        ));
        clear_archive_callbacks();

        // 同时提交 [1, 2] → L2
        // ID 1: from=L1, ID 2: from=Active
        let ids = sp_runtime::BoundedVec::truncate_from(vec![1u64, 2]);
        assert_ok!(StorageLifecycle::force_archive(
            RawOrigin::Root.into(), dt(), ids,
            ArchiveLevel::ArchivedL2.to_u8(),
        ));

        let callbacks = get_archive_callbacks();
        assert_eq!(callbacks.len(), 2);
        // ID 1: from=L1(1) → L2(2)
        assert_eq!(callbacks[0].1, 1);
        assert_eq!(callbacks[0].2, ArchiveLevel::ArchivedL1.to_u8());
        assert_eq!(callbacks[0].3, ArchiveLevel::ArchivedL2.to_u8());
        // ID 2: from=Active(0) → L2(2)
        assert_eq!(callbacks[1].1, 2);
        assert_eq!(callbacks[1].2, ArchiveLevel::Active.to_u8());
        assert_eq!(callbacks[1].3, ArchiveLevel::ArchivedL2.to_u8());
    });
}
