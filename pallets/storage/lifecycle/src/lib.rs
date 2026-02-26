//! # Storage Lifecycle Pallet
//!
//! 存储生命周期管理模块，提供分级归档框架
//!
//! ## 功能
//! - 定义 `ArchivableData` trait 用于数据生命周期管理
//! - 支持三级存储：活跃 → L1归档 → L2归档 → 清除
//! - 在 `on_idle` 中自动处理归档任务
//!
//! ## 归档策略
//! - L1归档：保留核心字段，压缩存储（~50-80%节省）
//! - L2归档：仅保留统计摘要（~90%+节省）
//! - 清除：完全删除，仅保留永久统计

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_support::pallet_prelude::*;
use sp_runtime::traits::Saturating;
use sp_std::marker::PhantomData;

/// 可归档数据 Trait
///
/// 所有需要生命周期管理的数据类型都应实现此 Trait
pub trait ArchivableData: Encode + Decode + Clone {
    /// 一级归档类型（精简摘要，~50-80%压缩）
    type ArchivedL1: Encode + Decode + Clone + MaxEncodedLen;
    /// 二级归档类型（最小摘要，~90%+压缩）
    type ArchivedL2: Encode + Decode + Clone + MaxEncodedLen;
    /// 永久统计类型
    type PermanentStats: Encode + Decode + Clone + MaxEncodedLen + Default;

    /// 获取数据ID
    fn get_id(&self) -> u64;

    /// 判断是否可以归档到 L1
    /// 
    /// # Arguments
    /// * `now` - 当前区块号
    /// * `l1_delay` - L1归档延迟（区块数）
    fn can_archive_l1(&self, now: u64, l1_delay: u64) -> bool;

    /// 转换为一级归档
    fn to_archived_l1(&self) -> Self::ArchivedL1;

    /// 判断L1归档是否可以转为L2
    /// 
    /// # Arguments
    /// * `archived` - L1归档数据
    /// * `now` - 当前区块号
    /// * `l2_delay` - L2归档延迟（区块数）
    fn can_archive_l2(archived: &Self::ArchivedL1, now: u64, l2_delay: u64) -> bool;

    /// 从一级归档转换为二级归档
    fn l1_to_l2(archived: &Self::ArchivedL1) -> Self::ArchivedL2;

    /// 更新永久统计
    fn update_stats(stats: &mut Self::PermanentStats, archived: &Self::ArchivedL1);
}

/// 存储归档器 Trait
///
/// 由 pallet-storage-service 实现，供 lifecycle pallet 调用。
/// 解耦归档逻辑与具体存储实现。
pub trait StorageArchiver {
    /// 扫描可归档的记录ID（已过期超过 delay 区块的 CID）
    ///
    /// # Arguments
    /// * `delay` - 过期后等待多少区块才可归档
    /// * `max_count` - 最多返回多少条
    ///
    /// # Returns
    /// 可归档的记录ID列表
    fn scan_archivable(delay: u64, max_count: u32) -> sp_std::vec::Vec<u64>;

    /// 执行归档清理（删除链上存储记录）
    fn archive_records(ids: &[u64]);
}

/// 空实现（runtime 未接入 service pallet 时使用）
impl StorageArchiver for () {
    fn scan_archivable(_delay: u64, _max_count: u32) -> sp_std::vec::Vec<u64> {
        sp_std::vec::Vec::new()
    }
    fn archive_records(_ids: &[u64]) {}
}

/// 归档状态
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum ArchiveLevel {
    /// 活跃数据（完整存储）
    Active,
    /// 一级归档（精简存储）
    ArchivedL1,
    /// 二级归档（最小存储）
    ArchivedL2,
    /// 已清除（仅统计）
    Purged,
}

impl Default for ArchiveLevel {
    fn default() -> Self {
        Self::Active
    }
}

impl ArchiveLevel {
    /// 转换为 u8
    pub fn to_u8(&self) -> u8 {
        match self {
            Self::Active => 0,
            Self::ArchivedL1 => 1,
            Self::ArchivedL2 => 2,
            Self::Purged => 3,
        }
    }

    /// 从 u8 转换
    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::Active,
            1 => Self::ArchivedL1,
            2 => Self::ArchivedL2,
            3 => Self::Purged,
            _ => Self::Active,
        }
    }
}

/// 归档记录
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ArchiveRecord {
    /// 数据ID
    pub data_id: u64,
    /// 归档级别
    pub level: ArchiveLevel,
    /// 归档时间（区块号）
    pub archived_at: u64,
    /// 原始大小（字节）
    pub original_size: u32,
    /// 归档后大小（字节）
    pub archived_size: u32,
}

/// 归档批次信息
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ArchiveBatch {
    /// 批次ID
    pub batch_id: u64,
    /// 数据ID范围起始
    pub id_start: u64,
    /// 数据ID范围结束
    pub id_end: u64,
    /// 归档数量
    pub count: u32,
    /// 归档时间
    pub archived_at: u64,
    /// 归档级别 (0=Active, 1=L1, 2=L2, 3=Purged)
    pub level: u8,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 运行时事件类型
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// L1归档延迟（区块数），数据完成后多久可以归档到L1
        #[pallet::constant]
        type L1ArchiveDelay: Get<u32>;

        /// L2归档延迟（区块数），L1归档后多久可以转为L2
        #[pallet::constant]
        type L2ArchiveDelay: Get<u32>;

        /// 清除延迟（区块数），L2归档后多久可以清除
        #[pallet::constant]
        type PurgeDelay: Get<u32>;

        /// 是否启用清除功能
        #[pallet::constant]
        type EnablePurge: Get<bool>;

        /// 每次on_idle最大处理数量
        #[pallet::constant]
        type MaxBatchSize: Get<u32>;

        /// 存储归档器：提供可归档记录的扫描与清理接口
        type StorageArchiver: StorageArchiver;
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<frame_system::pallet_prelude::BlockNumberFor<T>> for Pallet<T> {
        fn on_idle(
            n: frame_system::pallet_prelude::BlockNumberFor<T>,
            remaining_weight: Weight,
        ) -> Weight {
            let max_batch = T::MaxBatchSize::get();
            // 最小权重门槛：至少能处理 1 条记录
            let min_weight = Weight::from_parts(50_000_000, 5_000);
            if remaining_weight.any_lt(min_weight) {
                return Weight::zero();
            }

            let now: u64 = n.try_into().unwrap_or(0u64);
            let l1_delay = T::L1ArchiveDelay::get() as u64;

            // 扫描可归档记录（已过期超过 L1ArchiveDelay 的 CID）
            let expired_ids = T::StorageArchiver::scan_archivable(l1_delay, max_batch);
            let count = expired_ids.len() as u32;

            if count > 0 {
                // 执行归档清理
                T::StorageArchiver::archive_records(&expired_ids);

                // 记录批次
                let data_type_vec: sp_runtime::BoundedVec<u8, ConstU32<32>> =
                    sp_runtime::BoundedVec::truncate_from(b"pin_storage".to_vec());
                let _ = StorageLifecycleManager::<T>::record_batch(
                    data_type_vec.clone(),
                    *expired_ids.first().unwrap_or(&0),
                    *expired_ids.last().unwrap_or(&0),
                    count,
                    ArchiveLevel::Purged,
                    now,
                );

                Self::deposit_event(Event::DataPurged {
                    data_type: data_type_vec,
                    count,
                });
            }

            // 返回消耗的权重
            Weight::from_parts(
                50_000_000u64.saturating_mul(count as u64 + 1),
                5_000u64.saturating_mul(count as u64 + 1),
            )
        }
    }

    /// 归档游标（按数据类型）
    #[pallet::storage]
    #[pallet::getter(fn archive_cursor)]
    pub type ArchiveCursor<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, ConstU32<32>>, // 数据类型标识
        u64,                           // 当前处理到的ID
        ValueQuery,
    >;

    /// 归档批次记录
    #[pallet::storage]
    #[pallet::getter(fn archive_batches)]
    pub type ArchiveBatches<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, ConstU32<32>>,                    // 数据类型标识
        BoundedVec<ArchiveBatch, ConstU32<100>>,         // 最近100个批次
        ValueQuery,
    >;

    /// 归档统计
    #[pallet::storage]
    #[pallet::getter(fn archive_stats)]
    pub type ArchiveStats<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, ConstU32<32>>, // 数据类型标识
        ArchiveStatistics,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 数据已归档到L1
        ArchivedToL1 {
            data_type: BoundedVec<u8, ConstU32<32>>,
            count: u32,
            saved_bytes: u64,
        },
        /// 数据已归档到L2
        ArchivedToL2 {
            data_type: BoundedVec<u8, ConstU32<32>>,
            count: u32,
            saved_bytes: u64,
        },
        /// 数据已清除
        DataPurged {
            data_type: BoundedVec<u8, ConstU32<32>>,
            count: u32,
        },
        /// 归档批次完成
        BatchCompleted {
            data_type: BoundedVec<u8, ConstU32<32>>,
            batch_id: u64,
            level: u8, // 0=Active, 1=L1, 2=L2, 3=Purged
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// 数据类型标识过长
        DataTypeTooLong,
        /// 归档批次已满
        BatchQueueFull,
        /// 数据不存在
        DataNotFound,
        /// 数据状态不允许归档
        InvalidArchiveState,
    }
}

/// 归档统计信息
#[derive(
    Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default,
)]
pub struct ArchiveStatistics {
    /// 总归档到L1数量
    pub total_l1_archived: u64,
    /// 总归档到L2数量
    pub total_l2_archived: u64,
    /// 总清除数量
    pub total_purged: u64,
    /// 节省的存储字节数
    pub total_bytes_saved: u64,
    /// 最后归档时间
    pub last_archive_at: u64,
}

/// 存储生命周期管理器
/// 
/// 提供分级归档的核心逻辑
pub struct StorageLifecycleManager<T: Config> {
    _marker: PhantomData<T>,
}

impl<T: Config> StorageLifecycleManager<T> {
    /// 创建新的管理器实例
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }

    /// 处理分级归档（在 on_idle 中调用）
    ///
    /// # Arguments
    /// * `now` - 当前区块号
    /// * `max_to_process` - 最大处理数量
    /// * `data_type` - 数据类型标识
    /// * `archive_fn` - 归档处理函数
    ///
    /// # Returns
    /// 实际处理的数量
    pub fn process_archival(
        now: u64,
        max_to_process: u32,
        data_type: &[u8],
    ) -> u32 {
        let batch_size = max_to_process / 3;
        let mut processed = 0u32;

        // 阶段1: 活跃 → L1 (由各模块自行实现)
        // 阶段2: L1 → L2 (由各模块自行实现)
        // 阶段3: L2 → 清除 (如果启用)

        if T::EnablePurge::get() {
            // 清除逻辑由各模块自行实现
            processed = processed.saturating_add(batch_size);
        }

        processed
    }

    /// 记录归档批次
    pub fn record_batch(
        data_type: BoundedVec<u8, ConstU32<32>>,
        id_start: u64,
        id_end: u64,
        count: u32,
        level: ArchiveLevel,
        now: u64,
    ) -> Result<(), Error<T>> {
        let batch = ArchiveBatch {
            batch_id: Self::next_batch_id(&data_type),
            id_start,
            id_end,
            count,
            archived_at: now,
            level: level.to_u8(),
        };

        ArchiveBatches::<T>::try_mutate(&data_type, |batches| {
            // LC3修复：队列满时淘汰最旧批次，避免永久失败
            if batches.try_push(batch.clone()).is_err() {
                batches.remove(0);
                batches.try_push(batch).map_err(|_| Error::<T>::BatchQueueFull)?;
            }
            Ok::<(), Error<T>>(())
        })?;

        // 更新统计
        ArchiveStats::<T>::mutate(&data_type, |stats| {
            match level {
                ArchiveLevel::ArchivedL1 => {
                    stats.total_l1_archived = stats.total_l1_archived.saturating_add(count as u64);
                }
                ArchiveLevel::ArchivedL2 => {
                    stats.total_l2_archived = stats.total_l2_archived.saturating_add(count as u64);
                }
                ArchiveLevel::Purged => {
                    stats.total_purged = stats.total_purged.saturating_add(count as u64);
                }
                _ => {}
            }
            stats.last_archive_at = now;
        });

        Ok(())
    }

    /// 获取下一个批次ID
    fn next_batch_id(data_type: &BoundedVec<u8, ConstU32<32>>) -> u64 {
        ArchiveBatches::<T>::get(data_type)
            .last()
            .map(|b| b.batch_id.saturating_add(1))
            .unwrap_or(1)
    }

    /// 更新归档游标
    pub fn update_cursor(data_type: BoundedVec<u8, ConstU32<32>>, cursor: u64) {
        ArchiveCursor::<T>::insert(data_type, cursor);
    }

    /// 获取归档游标
    pub fn get_cursor(data_type: &BoundedVec<u8, ConstU32<32>>) -> u64 {
        ArchiveCursor::<T>::get(data_type)
    }

    /// 更新节省的存储统计
    pub fn record_bytes_saved(data_type: &BoundedVec<u8, ConstU32<32>>, bytes: u64) {
        ArchiveStats::<T>::mutate(data_type, |stats| {
            stats.total_bytes_saved = stats.total_bytes_saved.saturating_add(bytes);
        });
    }
}

/// 辅助函数：将区块号转换为年月格式
pub fn block_to_year_month(block_number: u32, blocks_per_day: u32) -> u16 {
    // LC2修复：防止除零
    if blocks_per_day == 0 {
        return 2401; // 默认返回 2024年1月
    }
    // 假设创世区块是2024年1月
    let days = block_number / blocks_per_day;
    let months = days / 30;
    let year = 24 + (months / 12) as u16;
    let month = (months % 12 + 1) as u16;
    year * 100 + month // YYMM 格式
}

/// 辅助函数：将金额转换为档位
pub fn amount_to_tier(amount: u64) -> u8 {
    match amount {
        0..=99 => 0,           // < 100
        100..=999 => 1,        // 100-999
        1000..=9999 => 2,      // 1K-10K
        10000..=99999 => 3,    // 10K-100K
        100000..=999999 => 4,  // 100K-1M
        _ => 5,                // > 1M
    }
}
