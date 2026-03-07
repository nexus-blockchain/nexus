//! # Storage Lifecycle Pallet Benchmarking
//!
//! 覆盖所有 9 个 dispatchable extrinsics 的基准测试。

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use pallet::*;
use frame_support::BoundedVec;

/// 辅助：构造 data_type BoundedVec
fn dt<T: Config>() -> BoundedVec<u8, ConstU32<32>> {
    BoundedVec::truncate_from(b"pin_storage".to_vec())
}

/// 辅助：预设数据归档状态
fn seed_data_status<T: Config>(data_type: &BoundedVec<u8, ConstU32<32>>, data_id: u64, level: ArchiveLevel) {
    DataArchiveStatus::<T>::insert(data_type, data_id, level);
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // ==================== call_index(0): set_archive_config ====================
    #[benchmark]
    fn set_archive_config() {
        let config = ArchiveConfig {
            l1_delay: 100,
            l2_delay: 200,
            purge_delay: 300,
            purge_enabled: true,
            max_batch_size: 50,
        };
        #[extrinsic_call]
        _(RawOrigin::Root, config);
    }

    // ==================== call_index(1): pause_archival ====================
    #[benchmark]
    fn pause_archival() {
        #[extrinsic_call]
        _(RawOrigin::Root);
    }

    // ==================== call_index(2): resume_archival ====================
    #[benchmark]
    fn resume_archival() {
        ArchivalPaused::<T>::put(true);
        #[extrinsic_call]
        _(RawOrigin::Root);
    }

    // ==================== call_index(3): set_archive_policy ====================
    #[benchmark]
    fn set_archive_policy() {
        let data_type = dt::<T>();
        let policy = ArchivePolicy {
            l1_delay: 50,
            l2_delay: 100,
            purge_delay: 200,
            purge_enabled: true,
        };
        #[extrinsic_call]
        _(RawOrigin::Root, data_type, policy);
    }

    // ==================== call_index(4): force_archive ====================
    #[benchmark]
    fn force_archive() {
        let data_type = dt::<T>();
        // 预设 10 条 Active 数据
        for i in 1..=10u64 {
            seed_data_status::<T>(&data_type, i, ArchiveLevel::Active);
        }
        let data_ids: BoundedVec<u64, ConstU32<100>> =
            BoundedVec::truncate_from((1..=10u64).collect::<sp_std::vec::Vec<_>>());
        #[extrinsic_call]
        _(RawOrigin::Root, data_type, data_ids, 1u8); // target = ArchivedL1
    }

    // ==================== call_index(5): protect_from_purge ====================
    #[benchmark]
    fn protect_from_purge() {
        let data_type = dt::<T>();
        #[extrinsic_call]
        _(RawOrigin::Root, data_type, 1u64);
    }

    // ==================== call_index(6): remove_purge_protection ====================
    #[benchmark]
    fn remove_purge_protection() {
        let data_type = dt::<T>();
        PurgeProtected::<T>::insert(&data_type, 1u64, true);
        #[extrinsic_call]
        _(RawOrigin::Root, data_type, 1u64);
    }

    // ==================== call_index(7): extend_active_period ====================
    #[benchmark]
    fn extend_active_period() {
        let data_type = dt::<T>();
        seed_data_status::<T>(&data_type, 1, ArchiveLevel::Active);
        #[extrinsic_call]
        _(RawOrigin::Root, data_type, 1u64, 1000u64);
    }

    // ==================== call_index(8): restore_from_archive ====================
    #[benchmark]
    fn restore_from_archive() {
        let data_type = dt::<T>();
        seed_data_status::<T>(&data_type, 1, ArchiveLevel::ArchivedL1);
        #[extrinsic_call]
        _(RawOrigin::Root, data_type, 1u64);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
