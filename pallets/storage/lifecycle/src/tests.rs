use crate::{mock::*, *};
use frame_support::{assert_ok, traits::Hooks};
use frame_system::pallet_prelude::BlockNumberFor;

#[test]
fn block_to_year_month_works() {
    // 14400 blocks/day (6s/block)
    assert_eq!(block_to_year_month(0, 14400), 2401); // 2024-01
    assert_eq!(block_to_year_month(14400 * 30, 14400), 2402); // 2024-02
    assert_eq!(block_to_year_month(14400 * 30 * 12, 14400), 2501); // 2025-01
}

#[test]
fn block_to_year_month_zero_blocks_per_day() {
    assert_eq!(block_to_year_month(100, 0), 2401);
}

#[test]
fn amount_to_tier_works() {
    assert_eq!(amount_to_tier(0), 0);
    assert_eq!(amount_to_tier(99), 0);
    assert_eq!(amount_to_tier(100), 1);
    assert_eq!(amount_to_tier(999), 1);
    assert_eq!(amount_to_tier(1000), 2);
    assert_eq!(amount_to_tier(9999), 2);
    assert_eq!(amount_to_tier(10000), 3);
    assert_eq!(amount_to_tier(99999), 3);
    assert_eq!(amount_to_tier(100000), 4);
    assert_eq!(amount_to_tier(999999), 4);
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
        let data_type: sp_runtime::BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            sp_runtime::BoundedVec::truncate_from(b"test_data".to_vec());

        assert_ok!(StorageLifecycleManager::<Test>::record_batch(
            data_type.clone(), 1, 5, 5, ArchiveLevel::Purged, 100,
        ));

        let stats = pallet::ArchiveStats::<Test>::get(&data_type);
        assert_eq!(stats.total_purged, 5);
        assert_eq!(stats.last_archive_at, 100);

        let batches = pallet::ArchiveBatches::<Test>::get(&data_type);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].batch_id, 1);
        assert_eq!(batches[0].id_start, 1);
        assert_eq!(batches[0].id_end, 5);
        assert_eq!(batches[0].count, 5);
    });
}

#[test]
fn record_batch_l1_updates_stats() {
    new_test_ext().execute_with(|| {
        let data_type: sp_runtime::BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            sp_runtime::BoundedVec::truncate_from(b"l1test".to_vec());

        assert_ok!(StorageLifecycleManager::<Test>::record_batch(
            data_type.clone(), 10, 20, 11, ArchiveLevel::ArchivedL1, 50,
        ));

        let stats = pallet::ArchiveStats::<Test>::get(&data_type);
        assert_eq!(stats.total_l1_archived, 11);
        assert_eq!(stats.total_l2_archived, 0);
        assert_eq!(stats.total_purged, 0);
    });
}

#[test]
fn cursor_update_and_read() {
    new_test_ext().execute_with(|| {
        let data_type: sp_runtime::BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            sp_runtime::BoundedVec::truncate_from(b"cursor_test".to_vec());

        assert_eq!(StorageLifecycleManager::<Test>::get_cursor(&data_type), 0);
        StorageLifecycleManager::<Test>::update_cursor(data_type.clone(), 42);
        assert_eq!(StorageLifecycleManager::<Test>::get_cursor(&data_type), 42);
    });
}

#[test]
fn record_bytes_saved_accumulates() {
    new_test_ext().execute_with(|| {
        let data_type: sp_runtime::BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            sp_runtime::BoundedVec::truncate_from(b"bytes_test".to_vec());

        StorageLifecycleManager::<Test>::record_bytes_saved(&data_type, 1000);
        StorageLifecycleManager::<Test>::record_bytes_saved(&data_type, 500);

        let stats = pallet::ArchiveStats::<Test>::get(&data_type);
        assert_eq!(stats.total_bytes_saved, 1500);
    });
}

#[test]
fn on_idle_processes_archivable_records() {
    new_test_ext().execute_with(|| {
        set_archivable_ids(vec![1, 2, 3]);

        let weight = StorageLifecycle::on_idle(
            100u64,
            Weight::from_parts(500_000_000, 50_000),
        );

        assert!(weight.ref_time() > 0);
        assert_eq!(get_archived_ids(), vec![1, 2, 3]);
    });
}

#[test]
fn on_idle_skips_when_no_records() {
    new_test_ext().execute_with(|| {
        let weight = StorageLifecycle::on_idle(
            100u64,
            Weight::from_parts(500_000_000, 50_000),
        );

        assert_eq!(weight.ref_time(), 50_000_000);
        assert!(get_archived_ids().is_empty());
    });
}

#[test]
fn on_idle_skips_when_insufficient_weight() {
    new_test_ext().execute_with(|| {
        set_archivable_ids(vec![1, 2, 3]);

        let weight = StorageLifecycle::on_idle(
            100u64,
            Weight::from_parts(1_000, 100),
        );

        assert_eq!(weight, Weight::zero());
        assert!(get_archived_ids().is_empty());
    });
}

#[test]
fn batch_queue_evicts_oldest_when_full() {
    new_test_ext().execute_with(|| {
        let data_type: sp_runtime::BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            sp_runtime::BoundedVec::truncate_from(b"evict_test".to_vec());

        for i in 0..100u64 {
            assert_ok!(StorageLifecycleManager::<Test>::record_batch(
                data_type.clone(), i, i, 1, ArchiveLevel::Purged, i,
            ));
        }

        // Queue is full (100), adding one more should evict the oldest
        assert_ok!(StorageLifecycleManager::<Test>::record_batch(
            data_type.clone(), 100, 100, 1, ArchiveLevel::Purged, 100,
        ));

        let batches = pallet::ArchiveBatches::<Test>::get(&data_type);
        assert_eq!(batches.len(), 100);
        // First batch should now be batch_id=2 (oldest evicted)
        assert_eq!(batches[0].batch_id, 2);
    });
}
