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
//! - 待生效配置自动应用（`on_initialize` + `PendingConfigQueue`）

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
        /// 最低团队规模，最大 1_000_000，0 = 无要求
        pub required_team_size: u32,
        /// 最低累计消费 USDT（精度 10^6），最大 10^18，0 = 无要求
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
    // R10-#8: total_beneficiaries → total_distribution_entries（实际为累计分发条目数，非去重受益人数）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct EntityStatsData {
        pub total_distributed: u128,
        pub total_orders: u32,
        pub total_distribution_entries: u32,
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
        // R10-#1: Root 强制操作审计日志
        ForceSet,
        ForceClear,
        Pause,
        Resume,
        PendingScheduled,
        PendingApplied,
        PendingCancelled,
        // R10-#14: on_initialize 自动应用
        PendingAutoApplied,
        // R10-#6: Root 强制暂停/恢复
        ForcePause,
        ForceResume,
        // R11-B3: 治理路径审计日志
        GovernanceSet,
        GovernanceClear,
    }

    // F1: 待生效配置条目
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

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    // R10-#14: on_initialize 自动应用待生效配置 + integrity_test
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(n: BlockNumberFor<T>) -> Weight {
            Self::auto_apply_pending_configs(n)
        }

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

    // R10-#14: 待生效配置自动应用队列（entity_id 列表，供 on_initialize 遍历）
    #[pallet::storage]
    pub type PendingConfigQueue<T: Config> = StorageValue<
        _,
        BoundedVec<u64, ConstU32<100>>,
        ValueQuery,
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
        // R10-#7: 佣金分发统计更新
        MultiLevelCommissionDistributed { entity_id: u64, total_amount: u128, beneficiary_count: u32 },
        // R10-#4: Entity 存储已清理
        EntityStorageCleaned { entity_id: u64 },
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
        // R10-#3: required_team_size 超过 1_000_000
        InvalidTeamSize,
        // R10-#3: required_spent 超过 10^18
        InvalidSpent,
        // R10-#14: 待生效配置队列已满
        PendingQueueFull,
        // R10-#6: 多级分销未暂停（force_resume 时）
        MultiLevelNotPaused,
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
        // R10-#1: 补充审计日志（使用 entity_account 标识 Root 操作）
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
            let root_who = T::EntityProvider::entity_account(entity_id);
            Self::record_change_log(entity_id, &root_who, ConfigChangeType::ForceSet);
            Ok(())
        }

        /// [Root] 强制清除 Entity 多级分销配置（幂等，配置不存在时静默成功）
        // R10-#1: 补充审计日志
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
                let root_who = T::EntityProvider::entity_account(entity_id);
                Self::record_change_log(entity_id, &root_who, ConfigChangeType::ForceClear);
            }
            Ok(())
        }

        /// F3: 部分更新多级分销参数（Owner/Admin）
        ///
        /// 支持单独更新 `max_total_rate` 和/或指定层的配置，无需重提整个 levels 数组。
        /// 全部 None 返回 `NothingToUpdate`。
        // R10-#2: 变更后触发 rates_sum 警告检查
        // R10-#3: 补充 team_size / spent 上界校验
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
                    Self::validate_tier(&tier)?;
                    config.levels[idx] = tier;
                    Self::deposit_event(Event::TierUpdated { entity_id, tier_index: idx as u32 });
                }

                Self::check_rates_sum_warning(entity_id, config);
                Self::record_change_log(entity_id, &who, ConfigChangeType::UpdateParams);
                Ok(())
            })
        }

        /// F4: 在指定位置插入新层级（Owner/Admin）
        ///
        /// `index` 为插入位置（0-indexed），现有层级从 index 开始后移。
        /// index = levels.len() 表示追加到末尾。
        // R10-#2: 插入后触发 rates_sum 警告检查
        // R10-#3: 补充 team_size / spent 上界校验
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
            Self::validate_tier(&tier)?;

            MultiLevelConfigs::<T>::try_mutate(entity_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::ConfigNotFound)?;
                let idx = index as usize;
                ensure!(idx <= config.levels.len(), Error::<T>::TierIndexOutOfBounds);

                let mut v = config.levels.to_vec();
                v.insert(idx, tier);
                config.levels = v.try_into().map_err(|_| Error::<T>::TierLimitExceeded)?;

                Self::check_rates_sum_warning(entity_id, config);
                Self::deposit_event(Event::TierInserted { entity_id, tier_index: index });
                Self::record_change_log(entity_id, &who, ConfigChangeType::AddTier { index });
                Ok(())
            })
        }

        /// F4: 移除指定位置的层级（Owner/Admin）
        ///
        /// 移除后 levels 不可为空（至少保留 1 层，否则应使用 clear）。
        // R10-#2: 移除后触发 rates_sum 警告检查
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

                Self::check_rates_sum_warning(entity_id, config);
                Self::deposit_event(Event::TierRemoved { entity_id, tier_index: index });
                Self::record_change_log(entity_id, &who, ConfigChangeType::RemoveTier { index });
                Ok(())
            })
        }

        /// F10: 暂停 Entity 多级分销（Owner/Admin）
        #[pallet::call_index(7)]
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
        #[pallet::weight(T::WeightInfo::resume_multi_level())]
        pub fn resume_multi_level(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(GlobalPaused::<T>::get(entity_id), Error::<T>::MultiLevelNotPaused);

            GlobalPaused::<T>::remove(entity_id);
            Self::deposit_event(Event::MultiLevelResumed { entity_id });
            Self::record_change_log(entity_id, &who, ConfigChangeType::Resume);
            Ok(())
        }

        /// F1: 调度延迟生效的配置变更（Owner/Admin）
        ///
        /// 配置将在 current_block + ConfigChangeDelay 后生效。
        /// 到达生效区块后由 on_initialize 自动应用，或手动调用 apply_pending_config。
        // R10-#14: 同时加入 PendingConfigQueue 供 on_initialize 自动应用
        #[pallet::call_index(9)]
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

            PendingConfigQueue::<T>::try_mutate(|q| -> DispatchResult {
                q.try_push(entity_id).map_err(|_| Error::<T>::PendingQueueFull)?;
                Ok(())
            })?;

            Self::deposit_event(Event::PendingConfigScheduled { entity_id, effective_at });
            Self::record_change_log(entity_id, &who, ConfigChangeType::PendingScheduled);
            Ok(())
        }

        /// F1: 应用待生效的配置变更（任何人可调用，但必须到达生效区块且实体未锁定）
        // R10-#14: 手动应用时也从 PendingConfigQueue 移除
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::apply_pending_config())]
        pub fn apply_pending_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            let pending = PendingConfigs::<T>::get(entity_id).ok_or(Error::<T>::NoPendingConfig)?;

            let current_block: u32 = <frame_system::Pallet<T>>::block_number().try_into().unwrap_or(0u32);
            ensure!(current_block >= pending.effective_at, Error::<T>::PendingConfigNotReady);

            Self::do_apply_pending(entity_id, &pending)?;
            Self::remove_from_pending_queue(entity_id);
            Self::record_change_log(entity_id, &who, ConfigChangeType::PendingApplied);
            Ok(())
        }

        /// F1: 取消待生效的配置变更（Owner/Admin）
        // R10-#14: 同时从 PendingConfigQueue 移除
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::cancel_pending_config())]
        pub fn cancel_pending_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(PendingConfigs::<T>::contains_key(entity_id), Error::<T>::NoPendingConfig);

            PendingConfigs::<T>::remove(entity_id);
            Self::remove_from_pending_queue(entity_id);
            Self::deposit_event(Event::PendingConfigCancelled { entity_id });
            Self::record_change_log(entity_id, &who, ConfigChangeType::PendingCancelled);
            Ok(())
        }

        /// R10-#6: [Root] 强制暂停 Entity 多级分销
        ///
        /// 用于紧急安全响应，不受 EntityLocked 限制。
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::force_pause_multi_level())]
        pub fn force_pause_multi_level(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(!GlobalPaused::<T>::get(entity_id), Error::<T>::MultiLevelIsPaused);

            GlobalPaused::<T>::insert(entity_id, true);
            Self::deposit_event(Event::MultiLevelPaused { entity_id });
            let root_who = T::EntityProvider::entity_account(entity_id);
            Self::record_change_log(entity_id, &root_who, ConfigChangeType::ForcePause);
            Ok(())
        }

        /// R10-#6: [Root] 强制恢复 Entity 多级分销
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::force_resume_multi_level())]
        pub fn force_resume_multi_level(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(GlobalPaused::<T>::get(entity_id), Error::<T>::MultiLevelNotPaused);

            GlobalPaused::<T>::remove(entity_id);
            Self::deposit_event(Event::MultiLevelResumed { entity_id });
            let root_who = T::EntityProvider::entity_account(entity_id);
            Self::record_change_log(entity_id, &root_who, ConfigChangeType::ForceResume);
            Ok(())
        }

        /// R10-#4: [Root] 清理 Entity 下所有多级分销存储
        ///
        /// 用于 Entity 被删除后回收存储。清除配置、统计、审计日志、待生效配置等。
        /// `member_count_hint` 为预估成员数，用于准确计费；不影响实际清理范围。
        // R11-B1: weight 参数化，按 member_count_hint 缩放
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::force_cleanup_entity(*member_count_hint))]
        pub fn force_cleanup_entity(
            origin: OriginFor<T>,
            entity_id: u64,
            _member_count_hint: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::cleanup_entity_storage(entity_id);
            Self::deposit_event(Event::EntityStorageCleaned { entity_id });
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

        // R10-#3: 单层 tier 参数校验（rate + directs + team_size + spent 上界）
        pub fn validate_tier(tier: &MultiLevelTier) -> DispatchResult {
            ensure!(tier.rate <= 10000, Error::<T>::InvalidRate);
            ensure!(tier.required_directs <= 10000, Error::<T>::InvalidDirects);
            ensure!(tier.required_team_size <= 1_000_000, Error::<T>::InvalidTeamSize);
            ensure!(tier.required_spent <= 1_000_000_000_000_000_000u128, Error::<T>::InvalidSpent);
            Ok(())
        }

        /// 校验 levels + max_total_rate 参数合法性
        pub fn validate_config(
            levels: &BoundedVec<MultiLevelTier, T::MaxMultiLevels>,
            max_total_rate: u16,
        ) -> DispatchResult {
            ensure!(!levels.is_empty(), Error::<T>::EmptyLevels);
            for tier in levels.iter() {
                Self::validate_tier(tier)?;
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

                if !T::MemberProvider::is_member(entity_id, referrer) {
                    current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
                    continue;
                }

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
        pub fn check_tier_activation(
            entity_id: u64,
            account: &T::AccountId,
            tier: &MultiLevelTier,
        ) -> bool {
            if tier.required_directs == 0 && tier.required_team_size == 0 && tier.required_spent == 0 {
                return true;
            }
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
        pub fn get_activation_status(entity_id: u64, account: &T::AccountId) -> alloc::vec::Vec<bool> {
            let config = match MultiLevelConfigs::<T>::get(entity_id) {
                Some(c) => c,
                None => return alloc::vec::Vec::new(),
            };
            config.levels.iter().map(|tier| Self::check_tier_activation(entity_id, account, tier)).collect()
        }

        const MAX_CONFIG_CHANGE_LOGS: u32 = 1000;

        pub fn record_change_log(entity_id: u64, who: &T::AccountId, change_type: ConfigChangeType) {
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

        pub fn check_rates_sum_warning(entity_id: u64, config: &MultiLevelConfigOf<T>) {
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

            let (directs, team_size, _) = T::MemberProvider::get_member_stats(entity_id, account);
            let spent_usdt: u128 = T::MemberProvider::get_member_spent_usdt(entity_id, account).into();

            config.levels.iter().enumerate().map(|(idx, tier)| {
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

        // R10-#7: 更新佣金统计 + 发出 MultiLevelCommissionDistributed 事件
        // R10-#8: 字段重命名 total_beneficiaries → total_distribution_entries
        // R11-B5: 提升为 pub，允许 pallet-commission-core 调用
        pub fn update_stats(entity_id: u64, outputs: &[CommissionOutput<T::AccountId, u128>]) {
            if outputs.is_empty() { return; }

            let block: u32 = <frame_system::Pallet<T>>::block_number().try_into().unwrap_or(0u32);
            let mut total_distributed: u128 = 0;

            for output in outputs.iter() {
                MemberMultiLevelStats::<T>::mutate(entity_id, &output.beneficiary, |stats| {
                    stats.total_earned = stats.total_earned.saturating_add(output.amount);
                    stats.total_orders = stats.total_orders.saturating_add(1);
                    stats.last_commission_block = block;
                });
                total_distributed = total_distributed.saturating_add(output.amount);
            }

            EntityMultiLevelStats::<T>::mutate(entity_id, |stats| {
                stats.total_distributed = stats.total_distributed.saturating_add(total_distributed);
                stats.total_orders = stats.total_orders.saturating_add(1);
                stats.total_distribution_entries = stats.total_distribution_entries.saturating_add(outputs.len() as u32);
            });

            Self::deposit_event(Event::MultiLevelCommissionDistributed {
                entity_id,
                total_amount: total_distributed,
                beneficiary_count: outputs.len() as u32,
            });
        }

        /// F10: 查询多级分销是否暂停
        pub fn is_paused(entity_id: u64) -> bool {
            GlobalPaused::<T>::get(entity_id)
        }

        // R10-#9: 查询最近的配置变更审计日志（按时间逆序，最多 limit 条）
        pub fn get_recent_change_logs(entity_id: u64, limit: u32) -> Vec<ConfigChangeEntry<T>> {
            let total_count = ConfigChangeLogCount::<T>::get(entity_id);
            if total_count == 0 { return Vec::new(); }

            let effective_count = total_count.min(Self::MAX_CONFIG_CHANGE_LOGS);
            let fetch_count = limit.min(effective_count);
            let mut result = Vec::with_capacity(fetch_count as usize);

            for i in 0..fetch_count {
                let idx = total_count.saturating_sub(1).saturating_sub(i);
                let slot = idx % Self::MAX_CONFIG_CHANGE_LOGS;
                if let Some(entry) = ConfigChangeLogs::<T>::get(entity_id, slot) {
                    result.push(entry);
                }
            }

            result
        }

        // R10-#4: 清理 Entity 下所有多级分销相关存储
        pub fn cleanup_entity_storage(entity_id: u64) {
            MultiLevelConfigs::<T>::remove(entity_id);
            GlobalPaused::<T>::remove(entity_id);
            EntityMultiLevelStats::<T>::remove(entity_id);
            ConfigChangeLogCount::<T>::remove(entity_id);
            PendingConfigs::<T>::remove(entity_id);
            let _ = MemberMultiLevelStats::<T>::clear_prefix(entity_id, u32::MAX, None);
            let _ = ConfigChangeLogs::<T>::clear_prefix(entity_id, u32::MAX, None);
            Self::remove_from_pending_queue(entity_id);
        }

        // R10-#14: 从待生效配置队列移除指定 entity_id
        fn remove_from_pending_queue(entity_id: u64) {
            PendingConfigQueue::<T>::mutate(|q| {
                q.retain(|id| *id != entity_id);
            });
        }

        // R10-#14: 应用待生效配置的内部逻辑（extrinsic 和 on_initialize 共用）
        fn do_apply_pending(entity_id: u64, pending: &PendingConfigEntry<T>) -> DispatchResult {
            let old_config = MultiLevelConfigs::<T>::get(entity_id);
            let (old_levels_count, old_max_rate) = old_config
                .as_ref()
                .map(|c| (c.levels.len() as u32, c.max_total_rate))
                .unwrap_or((0, 0));

            MultiLevelConfigs::<T>::insert(entity_id, pending.config.clone());
            PendingConfigs::<T>::remove(entity_id);
            Self::check_rates_sum_warning(entity_id, &pending.config);

            Self::deposit_event(Event::PendingConfigApplied { entity_id });
            Self::deposit_event(Event::ConfigDetailedChange {
                entity_id,
                old_levels_count,
                new_levels_count: pending.config.levels.len() as u32,
                old_max_rate,
                new_max_rate: pending.config.max_total_rate,
            });

            Ok(())
        }

        // R10-#14: on_initialize — 自动应用已到达生效区块的待生效配置
        // 每区块最多检查 MAX_AUTO_APPLY 个条目，避免无限遍历。
        fn auto_apply_pending_configs(n: BlockNumberFor<T>) -> Weight {
            const MAX_AUTO_APPLY: u32 = 5;
            let current_block: u32 = n.try_into().unwrap_or(0u32);
            let queue = PendingConfigQueue::<T>::get();
            if queue.is_empty() {
                return Weight::from_parts(5_000_000, 1_000);
            }

            let mut applied_ids = Vec::new();
            let mut checked = 0u32;
            let mut applied = 0u32;

            for &entity_id in queue.iter() {
                if checked >= MAX_AUTO_APPLY { break; }
                checked += 1;

                if let Some(pending) = PendingConfigs::<T>::get(entity_id) {
                    if current_block >= pending.effective_at
                        && !T::EntityProvider::is_entity_locked(entity_id)
                    {
                        if Self::do_apply_pending(entity_id, &pending).is_ok() {
                            let auto_who = T::EntityProvider::entity_account(entity_id);
                            Self::record_change_log(entity_id, &auto_who, ConfigChangeType::PendingAutoApplied);
                            applied_ids.push(entity_id);
                            applied += 1;
                        }
                    }
                } else {
                    applied_ids.push(entity_id);
                }
            }

            if !applied_ids.is_empty() {
                PendingConfigQueue::<T>::mutate(|q| {
                    q.retain(|id| !applied_ids.contains(id));
                });
            }

            // weight = queue read + per-checked (PendingConfigs read + EntityProvider read)
            //        + per-applied (config read/write + pending remove + audit log write)
            //        + queue write (if any applied)
            let write_overhead = if applied_ids.is_empty() { 0u64 } else { 10_000_000u64 };
            Weight::from_parts(
                5_000_000u64
                    .saturating_add(10_000_000u64.saturating_mul(checked as u64))
                    .saturating_add(40_000_000u64.saturating_mul(applied as u64))
                    .saturating_add(write_overhead),
                1_000u64
                    .saturating_add(2_000u64.saturating_mul(checked as u64))
                    .saturating_add(5_000u64.saturating_mul(applied as u64)),
            )
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

        if !T::EntityProvider::is_entity_active(entity_id) {
            return (alloc::vec::Vec::new(), remaining);
        }

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

        if !T::EntityProvider::is_entity_active(entity_id) {
            return (alloc::vec::Vec::new(), remaining);
        }

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
// R10-#10: PlanWriter 路径补充 EntityLocked 检查
// R10-#3: 补充 team_size / spent 上界校验
// R11-B2: 复用 validate_config / validate_tier
// R11-B3: 补充 GovernanceSet / GovernanceClear 审计日志
// R11-B4: 补充 ConfigDetailedChange 事件
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::MultiLevelPlanWriter for pallet::Pallet<T> {
    fn set_multi_level(entity_id: u64, level_rates: alloc::vec::Vec<u16>, max_total_rate: u16) -> Result<(), sp_runtime::DispatchError> {
        frame_support::ensure!(
            !T::EntityProvider::is_entity_locked(entity_id),
            sp_runtime::DispatchError::Other("EntityLocked")
        );
        let bounded: frame_support::BoundedVec<pallet::MultiLevelTier, T::MaxMultiLevels> = level_rates
            .into_iter()
            .map(|rate| pallet::MultiLevelTier { rate, required_directs: 0, required_team_size: 0, required_spent: 0 })
            .collect::<alloc::vec::Vec<_>>()
            .try_into()
            .map_err(|_| sp_runtime::DispatchError::Other("TooManyLevels"))?;
        Self::validate_config(&bounded, max_total_rate)?;

        let old_config = pallet::MultiLevelConfigs::<T>::get(entity_id);
        let (old_levels_count, old_max_rate) = old_config
            .as_ref()
            .map(|c| (c.levels.len() as u32, c.max_total_rate))
            .unwrap_or((0, 0));
        let new_levels_count = bounded.len() as u32;

        let new_config = pallet::MultiLevelConfig { levels: bounded, max_total_rate };
        Self::check_rates_sum_warning(entity_id, &new_config);
        pallet::MultiLevelConfigs::<T>::insert(entity_id, new_config);

        Self::deposit_event(pallet::Event::MultiLevelConfigUpdated { entity_id });
        Self::deposit_event(pallet::Event::ConfigDetailedChange {
            entity_id, old_levels_count, new_levels_count, old_max_rate, new_max_rate: max_total_rate,
        });
        let gov_who = T::EntityProvider::entity_account(entity_id);
        Self::record_change_log(entity_id, &gov_who, pallet::ConfigChangeType::GovernanceSet);
        Ok(())
    }

    fn set_multi_level_full(
        entity_id: u64,
        tiers: alloc::vec::Vec<(u16, u32, u32, u128)>,
        max_total_rate: u16,
    ) -> Result<(), sp_runtime::DispatchError> {
        frame_support::ensure!(
            !T::EntityProvider::is_entity_locked(entity_id),
            sp_runtime::DispatchError::Other("EntityLocked")
        );
        let bounded: frame_support::BoundedVec<pallet::MultiLevelTier, T::MaxMultiLevels> = tiers
            .into_iter()
            .map(|(rate, required_directs, required_team_size, required_spent)| {
                pallet::MultiLevelTier { rate, required_directs, required_team_size, required_spent }
            })
            .collect::<alloc::vec::Vec<_>>()
            .try_into()
            .map_err(|_| sp_runtime::DispatchError::Other("TooManyLevels"))?;
        Self::validate_config(&bounded, max_total_rate)?;

        let old_config = pallet::MultiLevelConfigs::<T>::get(entity_id);
        let (old_levels_count, old_max_rate) = old_config
            .as_ref()
            .map(|c| (c.levels.len() as u32, c.max_total_rate))
            .unwrap_or((0, 0));
        let new_levels_count = bounded.len() as u32;

        let new_config = pallet::MultiLevelConfig { levels: bounded, max_total_rate };
        Self::check_rates_sum_warning(entity_id, &new_config);
        pallet::MultiLevelConfigs::<T>::insert(entity_id, new_config);

        Self::deposit_event(pallet::Event::MultiLevelConfigUpdated { entity_id });
        Self::deposit_event(pallet::Event::ConfigDetailedChange {
            entity_id, old_levels_count, new_levels_count, old_max_rate, new_max_rate: max_total_rate,
        });
        let gov_who = T::EntityProvider::entity_account(entity_id);
        Self::record_change_log(entity_id, &gov_who, pallet::ConfigChangeType::GovernanceSet);
        Ok(())
    }

    fn clear_multi_level_config(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        frame_support::ensure!(
            !T::EntityProvider::is_entity_locked(entity_id),
            sp_runtime::DispatchError::Other("EntityLocked")
        );
        if pallet::MultiLevelConfigs::<T>::contains_key(entity_id) {
            let old_config = pallet::MultiLevelConfigs::<T>::get(entity_id);
            let (old_levels_count, old_max_rate) = old_config
                .as_ref()
                .map(|c| (c.levels.len() as u32, c.max_total_rate))
                .unwrap_or((0, 0));

            pallet::MultiLevelConfigs::<T>::remove(entity_id);
            Self::deposit_event(pallet::Event::MultiLevelConfigCleared { entity_id });
            Self::deposit_event(pallet::Event::ConfigDetailedChange {
                entity_id, old_levels_count, new_levels_count: 0, old_max_rate, new_max_rate: 0,
            });
            let gov_who = T::EntityProvider::entity_account(entity_id);
            Self::record_change_log(entity_id, &gov_who, pallet::ConfigChangeType::GovernanceClear);
        }
        Ok(())
    }
}

// ============================================================================
// MultiLevelQueryProvider 实现
// ============================================================================

impl<T: Config> pallet_commission_common::MultiLevelQueryProvider<T::AccountId> for Pallet<T>
where
    T::AccountId: Ord,
{
    fn activation_progress(entity_id: u64, account: &T::AccountId) -> alloc::vec::Vec<pallet_commission_common::MultiLevelActivationInfo> {
        Self::get_activation_progress(entity_id, account)
            .into_iter()
            .map(|p| pallet_commission_common::MultiLevelActivationInfo {
                level: p.level,
                activated: p.activated,
                directs_current: p.directs_current,
                directs_required: p.directs_required,
                team_current: p.team_current,
                team_required: p.team_required,
                spent_current: p.spent_current,
                spent_required: p.spent_required,
            })
            .collect()
    }

    fn is_paused(entity_id: u64) -> bool {
        Self::is_paused(entity_id)
    }

    fn member_stats(entity_id: u64, account: &T::AccountId) -> Option<pallet_commission_common::MultiLevelMemberStats> {
        let stats = MemberMultiLevelStats::<T>::get(entity_id, account);
        if stats.total_earned == 0 && stats.total_orders == 0 {
            return None;
        }
        Some(pallet_commission_common::MultiLevelMemberStats {
            total_earned: stats.total_earned,
            total_orders: stats.total_orders,
            last_commission_block: stats.last_commission_block,
        })
    }
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
