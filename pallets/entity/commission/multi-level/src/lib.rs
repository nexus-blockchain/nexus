//! # pallet-commission-multi-level
//!
//! 多级分销返佣插件 — N 层推荐链 + 三维激活条件 + 总佣金上限。
//!
//! 作为 `pallet-commission-core` 的 `CommissionPlugin` 插件运行，支持 NEX / EntityToken 双轨佣金。
//!
//! ## 功能
//!
//! - N 层推荐链遍历（`MaxMultiLevels` 上限）
//! - 每层独立激活条件（直推人数 / 团队规模 / USDT 累计消费，AND 逻辑）
//! - 总佣金上限（`max_total_rate` 基点制截断）
//! - 循环检测（`BTreeSet<AccountId>` 防止环形推荐链）
//! - 泛型佣金计算，NEX 和 EntityToken 共用 `process_multi_level`

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod weights;
pub use pallet::*;
pub use weights::WeightInfo;

use pallet_entity_common::EntityProvider as _;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::collections::BTreeSet;
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::Get,
    };
    use frame_system::pallet_prelude::*;
    use pallet_commission_common::{
        CommissionOutput, CommissionType, MemberProvider,
    };
    use pallet_entity_common::{AdminPermission, EntityProvider};

    // ========================================================================
    // 数据结构
    // ========================================================================

    /// 多级分销层级配置
    ///
    /// 每层包含佣金比率和三维激活条件（AND 逻辑，值为 0 的条件自动跳过）。
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct MultiLevelTier {
        /// 佣金比率（基点制，10000 = 100%），0 = 跳过（占位层）
        pub rate: u16,
        /// 有效直推人数（不含复购赠与），最大 10000，0 = 无要求
        pub required_directs: u32,
        /// 最低团队规模，0 = 无要求
        pub required_team_size: u32,
        /// 最低累计消费 USDT（精度 10^6），0 = 无要求
        pub required_spent: u128,
    }

    /// 多级分销配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(MaxLevels))]
    pub struct MultiLevelConfig<MaxLevels: Get<u32>> {
        /// 各层配置，索引 0 = L1
        pub levels: BoundedVec<MultiLevelTier, MaxLevels>,
        /// 佣金总和上限（基点制，默认 1500 = 15%）
        pub max_total_rate: u16,
    }

    impl<MaxLevels: Get<u32>> Clone for MultiLevelConfig<MaxLevels> {
        fn clone(&self) -> Self {
            Self { levels: self.levels.clone(), max_total_rate: self.max_total_rate }
        }
    }

    impl<MaxLevels: Get<u32>> PartialEq for MultiLevelConfig<MaxLevels> {
        fn eq(&self, other: &Self) -> bool {
            self.levels == other.levels && self.max_total_rate == other.max_total_rate
        }
    }

    impl<MaxLevels: Get<u32>> Eq for MultiLevelConfig<MaxLevels> {}

    impl<MaxLevels: Get<u32>> core::fmt::Debug for MultiLevelConfig<MaxLevels> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("MultiLevelConfig")
                .field("levels", &self.levels.len())
                .field("max_total_rate", &self.max_total_rate)
                .finish()
        }
    }

    impl<MaxLevels: Get<u32>> Default for MultiLevelConfig<MaxLevels> {
        fn default() -> Self {
            Self {
                levels: BoundedVec::default(),
                max_total_rate: 1500,
            }
        }
    }

    pub type MultiLevelConfigOf<T> = MultiLevelConfig<<T as Config>::MaxMultiLevels>;

    // F6: 个人多级佣金统计数据
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct MultiLevelStatsData {
        pub total_earned: u128,
        pub total_orders: u32,
        pub last_commission_block: u32,
    }

    // F13: Entity 级多级佣金统计数据
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct EntityStatsData {
        pub total_distributed: u128,
        pub total_orders: u32,
        pub total_beneficiaries: u32,
    }

    // F5: 激活进度
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct ActivationProgress {
        pub level: u8,
        pub activated: bool,
        pub directs_current: u32,
        pub directs_required: u32,
        pub team_current: u32,
        pub team_required: u32,
        pub spent_current: u128,
        pub spent_required: u128,
    }

    // F2: 配置变更日志条目
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(T))]  
    pub struct ConfigChangeEntry<T: Config> {
        pub who: T::AccountId,
        pub block_number: u32,
        pub change_type: ConfigChangeType,
    }

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum ConfigChangeType {
        SetConfig,
        ClearConfig,
        UpdateParams,
        AddTier { index: u32 },
        RemoveTier { index: u32 },
        ForceSet,
        ForceClear,
        Pause,
        Resume,
        PendingScheduled,
        PendingApplied,
        // M3-R7: 取消待生效配置审计日志
        PendingCancelled,
    }

    // F1: 待生效配置条目
    //
    // 手动实现 Clone/PartialEq/Eq 以避免 MaxMultiLevels 类型参数的不必要 trait bound。
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]  
    pub struct PendingConfigEntry<T: Config> {
        pub config: MultiLevelConfigOf<T>,
        pub effective_at: u32,
        pub scheduled_by: T::AccountId,
    }

    impl<T: Config> Clone for PendingConfigEntry<T> {
        fn clone(&self) -> Self {
            Self {
                config: self.config.clone(),
                effective_at: self.effective_at,
                scheduled_by: self.scheduled_by.clone(),
            }
        }
    }

    impl<T: Config> PartialEq for PendingConfigEntry<T> {
        fn eq(&self, other: &Self) -> bool {
            self.config == other.config
                && self.effective_at == other.effective_at
                && self.scheduled_by == other.scheduled_by
        }
    }

    impl<T: Config> Eq for PendingConfigEntry<T> {}

    impl<T: Config> core::fmt::Debug for PendingConfigEntry<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("PendingConfigEntry")
                .field("effective_at", &self.effective_at)
                .finish()
        }
    }

    // ========================================================================
    // Config
    // ========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// 推荐链 + 统计 + USDT 消费数据
        type MemberProvider: MemberProvider<Self::AccountId>;

        /// 实体查询接口（权限校验、Owner/Admin 判断）
        type EntityProvider: EntityProvider<Self::AccountId>;

        /// 最大层级数（默认 15）
        #[pallet::constant]
        type MaxMultiLevels: Get<u32>;

        /// F1: 配置变更延迟区块数
        #[pallet::constant]
        type ConfigChangeDelay: Get<u32>;

        /// 权重
        type WeightInfo: WeightInfo;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    // F9: integrity_test — MaxMultiLevels 校验
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn integrity_test() {
            assert!(
                T::MaxMultiLevels::get() >= 1,
                "MaxMultiLevels must be >= 1"
            );
            assert!(
                T::MaxMultiLevels::get() <= 100,
                "MaxMultiLevels must be <= 100 to prevent excessive computation"
            );
            assert!(
                T::ConfigChangeDelay::get() >= 1,
                "ConfigChangeDelay must be >= 1"
            );
        }
    }

    // ========================================================================
    // Storage
    // ========================================================================

    /// Entity → 多级分销配置
    #[pallet::storage]
    #[pallet::getter(fn multi_level_config)]
    pub type MultiLevelConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        MultiLevelConfigOf<T>,
    >;

    // F10: 全局暂停开关
    #[pallet::storage]
    pub type GlobalPaused<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        bool, ValueQuery,
    >;

    // F6/F13: 个人多级佣金统计 (entity_id, account) → stats
    #[pallet::storage]
    pub type MemberMultiLevelStats<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        MultiLevelStatsData,
        ValueQuery,
    >;

    // F13: Entity 级多级佣金统计
    #[pallet::storage]
    pub type EntityMultiLevelStats<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        EntityStatsData,
        ValueQuery,
    >;

    // F2: 配置变更审计日志 (entity_id, log_index) → log entry
    #[pallet::storage]
    pub type ConfigChangeLogCount<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        u32, ValueQuery,
    >;

    #[pallet::storage]
    pub type ConfigChangeLogs<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, u32,
        ConfigChangeEntry<T>,
    >;

    // F1: 待生效配置
    #[pallet::storage]
    pub type PendingConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        PendingConfigEntry<T>,
    >;

    // ========================================================================
    // Events
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        MultiLevelConfigUpdated { entity_id: u64 },
        MultiLevelConfigCleared { entity_id: u64 },
        /// 单层配置已更新
        TierUpdated { entity_id: u64, tier_index: u32 },
        /// max_total_rate 已更新
        MaxTotalRateUpdated { entity_id: u64, new_rate: u16 },
        /// 层级已插入
        TierInserted { entity_id: u64, tier_index: u32 },
        /// 层级已移除
        TierRemoved { entity_id: u64, tier_index: u32 },
        // F10: 全局暂停/恢复
        MultiLevelPaused { entity_id: u64 },
        MultiLevelResumed { entity_id: u64 },
        // F4: rates 总和超过 max_total_rate 警告
        // L2-R9: rates_sum 改为 u32，避免多层高 rate 场景下 u16 饱和导致报告值不准确
        RatesSumExceedsMax { entity_id: u64, rates_sum: u32, max_total_rate: u16 },
        // F7: 详细配置变更事件
        ConfigDetailedChange { entity_id: u64, old_levels_count: u32, new_levels_count: u32, old_max_rate: u16, new_max_rate: u16 },
        // F1: 待生效配置已调度
        PendingConfigScheduled { entity_id: u64, effective_at: u32 },
        // F1: 待生效配置已应用
        PendingConfigApplied { entity_id: u64 },
        // F1: 待生效配置已取消
        PendingConfigCancelled { entity_id: u64 },
    }

    // ========================================================================
    // Errors
    // ========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// rate 超过 10000
        InvalidRate,
        /// levels 为空
        EmptyLevels,
        /// 实体不存在
        EntityNotFound,
        /// 非实体所有者或无 COMMISSION_MANAGE 权限
        NotEntityOwnerOrAdmin,
        /// 配置不存在（清除时）
        ConfigNotFound,
        /// 实体已被全局锁定，所有配置操作不可用
        EntityLocked,
        /// 部分更新时所有字段均为 None
        NothingToUpdate,
        /// tier_index 超出 levels 范围
        TierIndexOutOfBounds,
        /// 层级数已达 MaxMultiLevels 上限
        TierLimitExceeded,
        // F10: 多级分销已暂停
        MultiLevelIsPaused,
        // F1: 已有待生效配置
        PendingConfigExists,
        // F1: 无待生效配置
        NoPendingConfig,
        // F1: 待生效配置尚未到达生效区块
        PendingConfigNotReady,
        /// required_directs 超过 10000
        InvalidDirects,
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 设置 Entity 多级分销配置（Entity Owner 或持有 COMMISSION_MANAGE 权限的 Admin）
        ///
        /// 校验：levels 非空，每层 `rate ≤ 10000`，`0 < max_total_rate ≤ 10000`。
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_multi_level_config(levels.len() as u32))]
        pub fn set_multi_level_config(
            origin: OriginFor<T>,
            entity_id: u64,
            levels: BoundedVec<MultiLevelTier, T::MaxMultiLevels>,
            max_total_rate: u16,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            Self::validate_config(&levels, max_total_rate)?;

            // F7: 详细变更事件
            let old_config = MultiLevelConfigs::<T>::get(entity_id);
            let (old_levels_count, old_max_rate) = old_config
                .as_ref()
                .map(|c| (c.levels.len() as u32, c.max_total_rate))
                .unwrap_or((0, 0));

            let new_config = MultiLevelConfig { levels, max_total_rate };
            // F4: rates 总和 vs max_total_rate 警告
            Self::check_rates_sum_warning(entity_id, &new_config);

            MultiLevelConfigs::<T>::insert(entity_id, new_config.clone());

            Self::deposit_event(Event::MultiLevelConfigUpdated { entity_id });
            Self::deposit_event(Event::ConfigDetailedChange {
                entity_id,
                old_levels_count,
                new_levels_count: new_config.levels.len() as u32,
                old_max_rate,
                new_max_rate: max_total_rate,
            });
            // F2: 审计日志
            Self::record_change_log(entity_id, &who, ConfigChangeType::SetConfig);
            Ok(())
        }

        /// 清除 Entity 多级分销配置（Entity Owner 或持有 COMMISSION_MANAGE 权限的 Admin）
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::clear_multi_level_config())]
        pub fn clear_multi_level_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(MultiLevelConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);

            MultiLevelConfigs::<T>::remove(entity_id);
            Self::deposit_event(Event::MultiLevelConfigCleared { entity_id });
            // F2: 审计日志
            Self::record_change_log(entity_id, &who, ConfigChangeType::ClearConfig);
            Ok(())
        }

        /// [Root] 强制设置 Entity 多级分销配置
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::set_multi_level_config(levels.len() as u32))]
        pub fn force_set_multi_level_config(
            origin: OriginFor<T>,
            entity_id: u64,
            levels: BoundedVec<MultiLevelTier, T::MaxMultiLevels>,
            max_total_rate: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::validate_config(&levels, max_total_rate)?;

            // M1-R7: 记录旧配置用于 ConfigDetailedChange 事件
            let old_config = MultiLevelConfigs::<T>::get(entity_id);
            let (old_levels_count, old_max_rate) = old_config
                .as_ref()
                .map(|c| (c.levels.len() as u32, c.max_total_rate))
                .unwrap_or((0, 0));

            let new_levels_count = levels.len() as u32;
            let new_config = MultiLevelConfig { levels, max_total_rate };
            Self::check_rates_sum_warning(entity_id, &new_config);
            MultiLevelConfigs::<T>::insert(entity_id, new_config);

            Self::deposit_event(Event::MultiLevelConfigUpdated { entity_id });
            Self::deposit_event(Event::ConfigDetailedChange {
                entity_id,
                old_levels_count,
                new_levels_count,
                old_max_rate,
                new_max_rate: max_total_rate,
            });
            Ok(())
        }

        /// [Root] 强制清除 Entity 多级分销配置（幂等，配置不存在时静默成功）
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::clear_multi_level_config())]
        pub fn force_clear_multi_level_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            if MultiLevelConfigs::<T>::contains_key(entity_id) {
                MultiLevelConfigs::<T>::remove(entity_id);
                Self::deposit_event(Event::MultiLevelConfigCleared { entity_id });
            }
            Ok(())
        }

        /// F3: 部分更新多级分销参数（Owner/Admin）
        ///
        /// 支持单独更新 `max_total_rate` 和/或指定层的配置，无需重提整个 levels 数组。
        /// 全部 None 返回 `NothingToUpdate`。
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::update_multi_level_params())]
        pub fn update_multi_level_params(
            origin: OriginFor<T>,
            entity_id: u64,
            max_total_rate: Option<u16>,
            tier_index: Option<u32>,
            tier_update: Option<MultiLevelTier>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(
                max_total_rate.is_some() || (tier_index.is_some() && tier_update.is_some()),
                Error::<T>::NothingToUpdate
            );

            MultiLevelConfigs::<T>::try_mutate(entity_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::ConfigNotFound)?;

                if let Some(new_rate) = max_total_rate {
                    ensure!(new_rate > 0 && new_rate <= 10000, Error::<T>::InvalidRate);
                    config.max_total_rate = new_rate;
                    Self::deposit_event(Event::MaxTotalRateUpdated { entity_id, new_rate });
                }

                if let (Some(idx), Some(tier)) = (tier_index, tier_update) {
                    let idx = idx as usize;
                    ensure!(idx < config.levels.len(), Error::<T>::TierIndexOutOfBounds);
                    ensure!(tier.rate <= 10000, Error::<T>::InvalidRate);
                    ensure!(tier.required_directs <= 10000, Error::<T>::InvalidDirects);
                    config.levels[idx] = tier;
                    Self::deposit_event(Event::TierUpdated { entity_id, tier_index: idx as u32 });
                }

                // F2: 审计日志
                Self::record_change_log(entity_id, &who, ConfigChangeType::UpdateParams);
                Ok(())
            })
        }

        /// F4: 在指定位置插入新层级（Owner/Admin）
        ///
        /// `index` 为插入位置（0-indexed），现有层级从 index 开始后移。
        /// index = levels.len() 表示追加到末尾。
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::add_tier())]
        pub fn add_tier(
            origin: OriginFor<T>,
            entity_id: u64,
            index: u32,
            tier: MultiLevelTier,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(tier.rate <= 10000, Error::<T>::InvalidRate);
            ensure!(tier.required_directs <= 10000, Error::<T>::InvalidDirects);

            MultiLevelConfigs::<T>::try_mutate(entity_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::ConfigNotFound)?;
                let idx = index as usize;
                ensure!(idx <= config.levels.len(), Error::<T>::TierIndexOutOfBounds);

                // 构建新 Vec 并插入
                let mut v = config.levels.to_vec();
                v.insert(idx, tier);
                config.levels = v.try_into().map_err(|_| Error::<T>::TierLimitExceeded)?;

                Self::deposit_event(Event::TierInserted { entity_id, tier_index: index });
                // F2: 审计日志
                Self::record_change_log(entity_id, &who, ConfigChangeType::AddTier { index });
                Ok(())
            })
        }

        /// F4: 移除指定位置的层级（Owner/Admin）
        ///
        /// 移除后 levels 不可为空（至少保留 1 层，否则应使用 clear）。
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::remove_tier())]
        pub fn remove_tier(
            origin: OriginFor<T>,
            entity_id: u64,
            index: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            MultiLevelConfigs::<T>::try_mutate(entity_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::ConfigNotFound)?;
                let idx = index as usize;
                ensure!(idx < config.levels.len(), Error::<T>::TierIndexOutOfBounds);
                ensure!(config.levels.len() > 1, Error::<T>::EmptyLevels);

                let mut v = config.levels.to_vec();
                v.remove(idx);
                config.levels = v.try_into().map_err(|_| Error::<T>::TierLimitExceeded)?;

                Self::deposit_event(Event::TierRemoved { entity_id, tier_index: index });
                // F2: 审计日志
                Self::record_change_log(entity_id, &who, ConfigChangeType::RemoveTier { index });
                Ok(())
            })
        }

        /// F10: 暂停 Entity 多级分销（Owner/Admin）
        #[pallet::call_index(7)]
        // L1-R7: 使用专用权重函数
        #[pallet::weight(T::WeightInfo::pause_multi_level())]
        pub fn pause_multi_level(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!GlobalPaused::<T>::get(entity_id), Error::<T>::MultiLevelIsPaused);

            GlobalPaused::<T>::insert(entity_id, true);
            Self::deposit_event(Event::MultiLevelPaused { entity_id });
            Self::record_change_log(entity_id, &who, ConfigChangeType::Pause);
            Ok(())
        }

        /// F10: 恢复 Entity 多级分销（Owner/Admin）
        #[pallet::call_index(8)]
        // L1-R7: 使用专用权重函数
        #[pallet::weight(T::WeightInfo::resume_multi_level())]
        pub fn resume_multi_level(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(GlobalPaused::<T>::get(entity_id), Error::<T>::NothingToUpdate);

            GlobalPaused::<T>::remove(entity_id);
            Self::deposit_event(Event::MultiLevelResumed { entity_id });
            Self::record_change_log(entity_id, &who, ConfigChangeType::Resume);
            Ok(())
        }

        /// F1: 调度延迟生效的配置变更（Owner/Admin）
        ///
        /// 配置将在 current_block + ConfigChangeDelay 后生效。
        /// 需调用 apply_pending_config 来最终应用。
        #[pallet::call_index(9)]
        // L2-R7: 使用专用权重函数
        #[pallet::weight(T::WeightInfo::schedule_config_change(levels.len() as u32))]
        pub fn schedule_config_change(
            origin: OriginFor<T>,
            entity_id: u64,
            levels: BoundedVec<MultiLevelTier, T::MaxMultiLevels>,
            max_total_rate: u16,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(!PendingConfigs::<T>::contains_key(entity_id), Error::<T>::PendingConfigExists);
            Self::validate_config(&levels, max_total_rate)?;

            let current_block: u32 = <frame_system::Pallet<T>>::block_number().try_into().unwrap_or(0u32);
            let effective_at = current_block.saturating_add(T::ConfigChangeDelay::get());

            PendingConfigs::<T>::insert(entity_id, PendingConfigEntry {
                config: MultiLevelConfig { levels, max_total_rate },
                effective_at,
                scheduled_by: who.clone(),
            });

            Self::deposit_event(Event::PendingConfigScheduled { entity_id, effective_at });
            Self::record_change_log(entity_id, &who, ConfigChangeType::PendingScheduled);
            Ok(())
        }

        /// F1: 应用待生效的配置变更（任何人可调用，但必须到达生效区块且实体未锁定）
        #[pallet::call_index(10)]
        // L3-R7: 使用专用权重函数
        #[pallet::weight(T::WeightInfo::apply_pending_config())]
        pub fn apply_pending_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            // M1-R9: 锁定的实体不允许应用待生效配置，防止绕过锁定保护
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            let pending = PendingConfigs::<T>::get(entity_id).ok_or(Error::<T>::NoPendingConfig)?;

            let current_block: u32 = <frame_system::Pallet<T>>::block_number().try_into().unwrap_or(0u32);
            ensure!(current_block >= pending.effective_at, Error::<T>::PendingConfigNotReady);

            // F7: 记录详细变更
            let old_config = MultiLevelConfigs::<T>::get(entity_id);
            let (old_levels_count, old_max_rate) = old_config
                .as_ref()
                .map(|c| (c.levels.len() as u32, c.max_total_rate))
                .unwrap_or((0, 0));

            MultiLevelConfigs::<T>::insert(entity_id, pending.config.clone());
            PendingConfigs::<T>::remove(entity_id);

            // F4: rates 总和 vs max_total_rate 警告
            Self::check_rates_sum_warning(entity_id, &pending.config);

            Self::deposit_event(Event::PendingConfigApplied { entity_id });
            Self::deposit_event(Event::ConfigDetailedChange {
                entity_id,
                old_levels_count,
                new_levels_count: pending.config.levels.len() as u32,
                old_max_rate,
                new_max_rate: pending.config.max_total_rate,
            });
            Self::record_change_log(entity_id, &who, ConfigChangeType::PendingApplied);
            Ok(())
        }

        /// F1: 取消待生效的配置变更（Owner/Admin）
        #[pallet::call_index(11)]
        // L4-R7: 使用专用权重函数
        #[pallet::weight(T::WeightInfo::cancel_pending_config())]
        pub fn cancel_pending_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(PendingConfigs::<T>::contains_key(entity_id), Error::<T>::NoPendingConfig);

            PendingConfigs::<T>::remove(entity_id);
            Self::deposit_event(Event::PendingConfigCancelled { entity_id });
            // M3-R7: 审计日志
            Self::record_change_log(entity_id, &who, ConfigChangeType::PendingCancelled);
            Ok(())
        }
    }

    // ========================================================================
    // 核心算法 — process_multi_level（泛型，NEX/Token 共用）
    // ========================================================================

    impl<T: Config> Pallet<T> {
        /// 验证 Entity Owner 或 Admin(COMMISSION_MANAGE) 权限
        fn ensure_owner_or_admin(entity_id: u64, who: &T::AccountId) -> DispatchResult {
            let owner = T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            if *who == owner {
                return Ok(());
            }
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, who, AdminPermission::COMMISSION_MANAGE),
                Error::<T>::NotEntityOwnerOrAdmin
            );
            Ok(())
        }

        /// 校验 levels + max_total_rate 参数合法性
        fn validate_config(
            levels: &BoundedVec<MultiLevelTier, T::MaxMultiLevels>,
            max_total_rate: u16,
        ) -> DispatchResult {
            ensure!(!levels.is_empty(), Error::<T>::EmptyLevels);
            for tier in levels.iter() {
                ensure!(tier.rate <= 10000, Error::<T>::InvalidRate);
                ensure!(tier.required_directs <= 10000, Error::<T>::InvalidDirects);
            }
            ensure!(max_total_rate > 0 && max_total_rate <= 10000, Error::<T>::InvalidRate);
            Ok(())
        }

        /// 多级分销佣金计算
        ///
        /// 逐层遍历推荐链，每层执行：
        /// 1. rate = 0 → 跳过（占位层），向上移动 referrer
        /// 2. 无推荐人 → 终止
        /// 3. 循环检测（BTreeSet）→ 命中则终止
        /// 4. 激活条件不满足 → 跳过该层，继续下一层
        /// 5. 计算佣金 = order_amount × rate / 10000，取 min(commission, remaining)
        /// 6. 总额上限检查 — 累计超过 max_total_rate 时截断最后一笔并终止
        pub fn process_multi_level<B>(
            entity_id: u64,
            buyer: &T::AccountId,
            order_amount: B,
            remaining: &mut B,
            config: &MultiLevelConfigOf<T>,
            outputs: &mut Vec<CommissionOutput<T::AccountId, B>>,
        ) where
            T::AccountId: Ord,
            B: sp_runtime::traits::AtLeast32BitUnsigned + Copy,
        {
            if config.levels.is_empty() { return; }

            let mut visited = BTreeSet::new();
            visited.insert(buyer.clone());

            let mut current_referrer = T::MemberProvider::get_referrer(entity_id, buyer);
            let mut total_commission = B::zero();
            let max_commission = order_amount
                .saturating_mul(B::from(config.max_total_rate as u32))
                / B::from(10000u32);

            for (level_idx, tier) in config.levels.iter().enumerate() {
                if tier.rate == 0 {
                    if let Some(ref r) = current_referrer {
                        visited.insert(r.clone());
                    }
                    current_referrer = current_referrer.and_then(|r| T::MemberProvider::get_referrer(entity_id, &r));
                    continue;
                }

                let Some(ref referrer) = current_referrer else { break };

                if visited.contains(referrer) { break; }
                visited.insert(referrer.clone());

                // F10: 跳过非会员（已退出/未注册的推荐人不应获得佣金）
                // M1-R4: is_member/is_banned 廉价检查提前，避免 check_tier_activation 的多余 DB read
                if !T::MemberProvider::is_member(entity_id, referrer) {
                    current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
                    continue;
                }

                // F9: 跳过被封禁、未激活或冻结/暂停的推荐人
                // M1-R8: 补充 is_member_active 检查（覆盖 frozen/suspended 状态）
                if T::MemberProvider::is_banned(entity_id, referrer)
                    || !T::MemberProvider::is_activated(entity_id, referrer)
                    || !T::MemberProvider::is_member_active(entity_id, referrer)
                {
                    current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
                    continue;
                }

                if !Self::check_tier_activation(entity_id, referrer, tier) {
                    current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
                    continue;
                }

                let commission = order_amount.saturating_mul(B::from(tier.rate as u32)) / B::from(10000u32);
                // L1-R9: 区分 remaining=0（预算耗尽→终止）与 commission=0（精度截断→跳过）
                if remaining.is_zero() { break; }
                let actual = commission.min(*remaining);
                if actual.is_zero() {
                    current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
                    continue;
                }

                let level = (level_idx + 1).min(255) as u8;

                let new_total = total_commission.saturating_add(actual);
                if new_total > max_commission {
                    let can_distribute = max_commission.saturating_sub(total_commission);
                    if !can_distribute.is_zero() {
                        *remaining = remaining.saturating_sub(can_distribute);
                        outputs.push(CommissionOutput {
                            beneficiary: referrer.clone(),
                            amount: can_distribute,
                            commission_type: CommissionType::MultiLevel,
                            level,
                        });
                    }
                    break;
                }

                *remaining = remaining.saturating_sub(actual);
                total_commission = total_commission.saturating_add(actual);
                outputs.push(CommissionOutput {
                    beneficiary: referrer.clone(),
                    amount: actual,
                    commission_type: CommissionType::MultiLevel,
                    level,
                });

                current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
            }
        }

        /// 三维激活条件检查（AND 逻辑，值为 0 自动跳过）
        ///
        /// | 条件 | 数据来源 | 精度 |
        /// |------|----------|------|
        /// | `required_directs` | `MemberProvider::get_member_stats().0` | 有效直推人数（不含复购赠与） |
        /// | `required_team_size` | `MemberProvider::get_member_stats().1` | 人数 |
        /// | `required_spent` | `MemberProvider::get_member_spent_usdt()` | USDT × 10^6 |
        pub fn check_tier_activation(
            entity_id: u64,
            account: &T::AccountId,
            tier: &MultiLevelTier,
        ) -> bool {
            if tier.required_directs == 0 && tier.required_team_size == 0 && tier.required_spent == 0 {
                return true;
            }
            // L1-R3: 仅在需要时读取 get_member_stats，避免 required_spent-only 场景的多余 DB read
            if tier.required_directs > 0 || tier.required_team_size > 0 {
                let (direct_referrals, team_size, _) = T::MemberProvider::get_member_stats(entity_id, account);
                if tier.required_directs > 0 && direct_referrals < tier.required_directs { return false; }
                if tier.required_team_size > 0 && team_size < tier.required_team_size { return false; }
            }
            if tier.required_spent > 0 {
                let spent_usdt: u128 = T::MemberProvider::get_member_spent_usdt(entity_id, account).into();
                if spent_usdt < tier.required_spent { return false; }
            }
            true
        }

        /// F11: 查询指定账户在各层级的激活状态
        ///
        /// 返回 `Vec<bool>`，长度 = levels.len()，true = 该层已激活。
        /// 配置不存在时返回空 Vec。
        pub fn get_activation_status(entity_id: u64, account: &T::AccountId) -> alloc::vec::Vec<bool> {
            let config = match MultiLevelConfigs::<T>::get(entity_id) {
                Some(c) => c,
                None => return alloc::vec::Vec::new(),
            };
            config.levels.iter().map(|tier| Self::check_tier_activation(entity_id, account, tier)).collect()
        }

        // L4-R9: 审计日志上限（环形缓冲，超过后覆盖最旧条目）
        const MAX_CONFIG_CHANGE_LOGS: u32 = 1000;

        // F2: 记录配置变更审计日志
        // L4-R9: 使用环形缓冲防止无限增长，slot = count % MAX，count 持续递增保留总计数
        pub(crate) fn record_change_log(entity_id: u64, who: &T::AccountId, change_type: ConfigChangeType) {
            let count = ConfigChangeLogCount::<T>::get(entity_id);
            let slot = count % Self::MAX_CONFIG_CHANGE_LOGS;
            let block_number: u32 = <frame_system::Pallet<T>>::block_number().try_into().unwrap_or(0u32);
            ConfigChangeLogs::<T>::insert(entity_id, slot, ConfigChangeEntry {
                who: who.clone(),
                block_number,
                change_type,
            });
            ConfigChangeLogCount::<T>::insert(entity_id, count.saturating_add(1));
        }

        // F4: rates 总和 vs max_total_rate 警告事件
        // L2-R9: rates_sum 使用 u32 避免饱和
        fn check_rates_sum_warning(entity_id: u64, config: &MultiLevelConfigOf<T>) {
            let rates_sum: u32 = config.levels.iter().map(|t| t.rate as u32).sum();
            if rates_sum > config.max_total_rate as u32 {
                Self::deposit_event(Event::RatesSumExceedsMax {
                    entity_id,
                    rates_sum,
                    max_total_rate: config.max_total_rate,
                });
            }
        }

        /// F5: 查询指定账户在各层级的激活进度（含当前值与要求值）
        pub fn get_activation_progress(entity_id: u64, account: &T::AccountId) -> Vec<ActivationProgress> {
            let config = match MultiLevelConfigs::<T>::get(entity_id) {
                Some(c) => c,
                None => return Vec::new(),
            };

            // M2-R7: 预加载一次，避免 check_tier_activation 每层重复 DB read
            let (directs, team_size, _) = T::MemberProvider::get_member_stats(entity_id, account);
            let spent_usdt: u128 = T::MemberProvider::get_member_spent_usdt(entity_id, account).into();

            config.levels.iter().enumerate().map(|(idx, tier)| {
                // 使用预加载的值内联检查，替代 check_tier_activation
                let activated = {
                    let directs_ok = tier.required_directs == 0 || directs >= tier.required_directs;
                    let team_ok = tier.required_team_size == 0 || team_size >= tier.required_team_size;
                    let spent_ok = tier.required_spent == 0 || spent_usdt >= tier.required_spent;
                    directs_ok && team_ok && spent_ok
                };
                ActivationProgress {
                    level: (idx + 1).min(255) as u8,
                    activated,
                    directs_current: directs,
                    directs_required: tier.required_directs,
                    team_current: team_size,
                    team_required: tier.required_team_size,
                    spent_current: spent_usdt,
                    spent_required: tier.required_spent,
                }
            }).collect()
        }

        /// F8: 预览佣金分配（不实际扣款，仅模拟计算）
        pub fn preview_commission(entity_id: u64, buyer: &T::AccountId, order_amount: u128) -> Vec<(T::AccountId, u128, u8)>
        where T::AccountId: Ord
        {
            // M2-R8: 与 calculate() 一致，未激活实体返空
            if !T::EntityProvider::is_entity_active(entity_id) {
                return Vec::new();
            }
            if GlobalPaused::<T>::get(entity_id) {
                return Vec::new();
            }
            let config = match MultiLevelConfigs::<T>::get(entity_id) {
                Some(c) => c,
                None => return Vec::new(),
            };

            let mut remaining = order_amount;
            let mut outputs = Vec::new();
            Self::process_multi_level(entity_id, buyer, order_amount, &mut remaining, &config, &mut outputs);
            outputs.into_iter().map(|o| (o.beneficiary, o.amount, o.level)).collect()
        }

        // F6/F13: 更新佣金统计（在 process_multi_level 输出后调用）
        pub(crate) fn update_stats(entity_id: u64, outputs: &[CommissionOutput<T::AccountId, u128>]) {
            if outputs.is_empty() { return; }

            let block: u32 = <frame_system::Pallet<T>>::block_number().try_into().unwrap_or(0u32);
            let mut total_distributed: u128 = 0;

            for output in outputs.iter() {
                // F6: 个人统计
                MemberMultiLevelStats::<T>::mutate(entity_id, &output.beneficiary, |stats| {
                    stats.total_earned = stats.total_earned.saturating_add(output.amount);
                    stats.total_orders = stats.total_orders.saturating_add(1);
                    stats.last_commission_block = block;
                });
                total_distributed = total_distributed.saturating_add(output.amount);
            }

            // F13: Entity 级统计
            EntityMultiLevelStats::<T>::mutate(entity_id, |stats| {
                stats.total_distributed = stats.total_distributed.saturating_add(total_distributed);
                stats.total_orders = stats.total_orders.saturating_add(1);
                stats.total_beneficiaries = stats.total_beneficiaries.saturating_add(outputs.len() as u32);
            });
        }

        /// F10: 查询多级分销是否暂停
        pub fn is_paused(entity_id: u64) -> bool {
            GlobalPaused::<T>::get(entity_id)
        }
    }
}

// ============================================================================
// CommissionPlugin — NEX（供 core 通过 type MultiLevelPlugin 调用）
// ============================================================================

impl<T: pallet::Config, B> pallet_commission_common::CommissionPlugin<T::AccountId, B> for pallet::Pallet<T>
where
    B: sp_runtime::traits::AtLeast32BitUnsigned + Copy + Default + core::fmt::Debug,
    T::AccountId: Ord,
{
    fn calculate(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: B,
        remaining: B,
        enabled_modes: pallet_commission_common::CommissionModes,
        _is_first_order: bool,
        _buyer_order_count: u32,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, B>>, B) {
        use pallet_commission_common::CommissionModes;

        if !enabled_modes.contains(CommissionModes::MULTI_LEVEL) {
            return (alloc::vec::Vec::new(), remaining);
        }

        // F12: Entity 未激活时跳过佣金计算
        if !T::EntityProvider::is_entity_active(entity_id) {
            return (alloc::vec::Vec::new(), remaining);
        }

        // F10: 全局暂停检查
        if pallet::GlobalPaused::<T>::get(entity_id) {
            return (alloc::vec::Vec::new(), remaining);
        }

        let config = match pallet::MultiLevelConfigs::<T>::get(entity_id) {
            Some(c) => c,
            None => return (alloc::vec::Vec::new(), remaining),
        };

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        pallet::Pallet::<T>::process_multi_level(
            entity_id, buyer, order_amount, &mut remaining, &config, &mut outputs,
        );

        (outputs, remaining)
    }
}

// ============================================================================
// TokenCommissionPlugin — EntityToken（供 core 通过 type TokenMultiLevelPlugin 调用）
// ============================================================================

impl<T: pallet::Config, TB> pallet_commission_common::TokenCommissionPlugin<T::AccountId, TB>
    for pallet::Pallet<T>
where
    TB: sp_runtime::traits::AtLeast32BitUnsigned + Copy + Default + core::fmt::Debug,
    T::AccountId: Ord,
{
    fn calculate_token(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: TB,
        remaining: TB,
        enabled_modes: pallet_commission_common::CommissionModes,
        _is_first_order: bool,
        _buyer_order_count: u32,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>, TB) {
        use pallet_commission_common::CommissionModes;

        if !enabled_modes.contains(CommissionModes::MULTI_LEVEL) {
            return (alloc::vec::Vec::new(), remaining);
        }

        // F12: Entity 未激活时跳过佣金计算
        if !T::EntityProvider::is_entity_active(entity_id) {
            return (alloc::vec::Vec::new(), remaining);
        }

        // F10: 全局暂停检查
        if pallet::GlobalPaused::<T>::get(entity_id) {
            return (alloc::vec::Vec::new(), remaining);
        }

        let config = match pallet::MultiLevelConfigs::<T>::get(entity_id) {
            Some(c) => c,
            None => return (alloc::vec::Vec::new(), remaining),
        };

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        pallet::Pallet::<T>::process_multi_level(
            entity_id, buyer, order_amount, &mut remaining, &config, &mut outputs,
        );

        (outputs, remaining)
    }
}

// ============================================================================
// MultiLevelPlanWriter — 治理路径
//
// PlanWriter 校验 rate / max_total_rate / 层数上限。
// 限制：PlanWriter 创建的 tiers 激活条件全为 0，需通过 Root extrinsic 配置完整条件。
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::MultiLevelPlanWriter for pallet::Pallet<T> {
    fn set_multi_level(entity_id: u64, level_rates: alloc::vec::Vec<u16>, max_total_rate: u16) -> Result<(), sp_runtime::DispatchError> {
        frame_support::ensure!(!level_rates.is_empty(), sp_runtime::DispatchError::Other("EmptyLevels"));
        frame_support::ensure!(max_total_rate > 0 && max_total_rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        for &rate in level_rates.iter() {
            frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        }
        let bounded: frame_support::BoundedVec<pallet::MultiLevelTier, T::MaxMultiLevels> = level_rates
            .into_iter()
            .map(|rate| pallet::MultiLevelTier { rate, required_directs: 0, required_team_size: 0, required_spent: 0 })
            .collect::<alloc::vec::Vec<_>>()
            .try_into()
            .map_err(|_| sp_runtime::DispatchError::Other("TooManyLevels"))?;
        pallet::MultiLevelConfigs::<T>::insert(entity_id, pallet::MultiLevelConfig { levels: bounded, max_total_rate });
        // M1-R2 审计修复: PlanWriter 路径也需 emit 事件，供 off-chain indexer 感知
        Self::deposit_event(pallet::Event::MultiLevelConfigUpdated { entity_id });
        Ok(())
    }

    fn set_multi_level_full(
        entity_id: u64,
        tiers: alloc::vec::Vec<(u16, u32, u32, u128)>,
        max_total_rate: u16,
    ) -> Result<(), sp_runtime::DispatchError> {
        frame_support::ensure!(!tiers.is_empty(), sp_runtime::DispatchError::Other("EmptyLevels"));
        frame_support::ensure!(max_total_rate > 0 && max_total_rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        for &(rate, required_directs, _, _) in tiers.iter() {
            frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
            frame_support::ensure!(required_directs <= 10000, sp_runtime::DispatchError::Other("InvalidDirects"));
        }
        let bounded: frame_support::BoundedVec<pallet::MultiLevelTier, T::MaxMultiLevels> = tiers
            .into_iter()
            .map(|(rate, required_directs, required_team_size, required_spent)| {
                pallet::MultiLevelTier { rate, required_directs, required_team_size, required_spent }
            })
            .collect::<alloc::vec::Vec<_>>()
            .try_into()
            .map_err(|_| sp_runtime::DispatchError::Other("TooManyLevels"))?;
        pallet::MultiLevelConfigs::<T>::insert(entity_id, pallet::MultiLevelConfig { levels: bounded, max_total_rate });
        Self::deposit_event(pallet::Event::MultiLevelConfigUpdated { entity_id });
        Ok(())
    }

    fn clear_multi_level_config(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        // L3-R9: 仅在配置存在时发送 Cleared 事件，避免误导 off-chain indexer
        if pallet::MultiLevelConfigs::<T>::contains_key(entity_id) {
            pallet::MultiLevelConfigs::<T>::remove(entity_id);
            Self::deposit_event(pallet::Event::MultiLevelConfigCleared { entity_id });
        }
        Ok(())
    }
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
