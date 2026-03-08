//! # Commission Single-Line Plugin (pallet-commission-single-line)
//!
//! 单线收益插件：基于全局消费注册顺序的上下线收益。
//! - 上线收益 (SingleLineUpline)
//! - 下线收益 (SingleLineDownline)
//! - 层数随消费额动态增长

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use pallet_entity_common::EntityProvider;

pub use pallet::*;
pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, Get},
    };
    use sp_runtime::Saturating;
    use frame_system::pallet_prelude::*;
    use pallet_commission_common::{
        CommissionOutput, CommissionType, MemberCommissionStatsData,
        MemberProvider,
    };
    use pallet_entity_common::{EntityProvider, AdminPermission};
    use sp_runtime::traits::{AtLeast32BitUnsigned, Zero};
    use crate::weights::WeightInfo;

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // ========================================================================
    // Config structs
    // ========================================================================

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct SingleLineConfig<Balance> {
        pub upline_rate: u16,
        pub downline_rate: u16,
        pub base_upline_levels: u8,
        pub base_downline_levels: u8,
        pub level_increment_threshold: Balance,
        pub max_upline_levels: u8,
        pub max_downline_levels: u8,
    }

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct LevelBasedLevels {
        pub upline_levels: u8,
        pub downline_levels: u8,
    }

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct PendingConfigChange<Balance, BlockNumber> {
        pub upline_rate: u16,
        pub downline_rate: u16,
        pub base_upline_levels: u8,
        pub base_downline_levels: u8,
        pub level_increment_threshold: Balance,
        pub max_upline_levels: u8,
        pub max_downline_levels: u8,
        pub apply_after: BlockNumber,
    }

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct ConfigChangeLogEntry<Balance, BlockNumber> {
        pub block_number: BlockNumber,
        pub upline_rate: u16,
        pub downline_rate: u16,
        pub base_upline_levels: u8,
        pub base_downline_levels: u8,
        pub max_upline_levels: u8,
        pub max_downline_levels: u8,
        pub level_increment_threshold: Balance,
    }

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct EntitySingleLineStatsData {
        pub total_orders: u32,
        pub total_upline_payouts: u32,
        pub total_downline_payouts: u32,
    }

    // ========================================================================
    // Pallet Config
    // ========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        type Currency: Currency<Self::AccountId>;

        type StatsProvider: SingleLineStatsProvider<Self::AccountId, BalanceOf<Self>>;
        type MemberLevelProvider: SingleLineMemberLevelProvider<Self::AccountId>;
        type EntityProvider: EntityProvider<Self::AccountId>;
        type MemberProvider: pallet_commission_common::MemberProvider<Self::AccountId>;
        type WeightInfo: WeightInfo;

        #[pallet::constant]
        type MaxSingleLineLength: Get<u32>;

        #[pallet::constant]
        type ConfigChangeDelay: Get<BlockNumberFor<Self>>;

        #[pallet::constant]
        type MaxSegmentCount: Get<u32>;

        /// upline_rate × max_upline_levels + downline_rate × max_downline_levels 的上限
        #[pallet::constant]
        type MaxTotalRateBps: Get<u32>;

        /// 每实体配置变更日志最大条数（循环覆写）
        #[pallet::constant]
        type MaxConfigChangeLogs: Get<u32>;
    }

    pub trait SingleLineStatsProvider<AccountId, Balance: Default> {
        fn get_member_stats(entity_id: u64, account: &AccountId) -> MemberCommissionStatsData<Balance>;
    }

    impl<AccountId, Balance: Default> SingleLineStatsProvider<AccountId, Balance> for () {
        fn get_member_stats(_: u64, _: &AccountId) -> MemberCommissionStatsData<Balance> {
            MemberCommissionStatsData::default()
        }
    }

    pub trait SingleLineMemberLevelProvider<AccountId> {
        fn custom_level_id(entity_id: u64, account: &AccountId) -> u8;
    }

    impl<AccountId> SingleLineMemberLevelProvider<AccountId> for () {
        fn custom_level_id(_: u64, _: &AccountId) -> u8 { 0 }
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        #[cfg(feature = "std")]
        fn integrity_test() {
            assert!(T::MaxSingleLineLength::get() > 0, "MaxSingleLineLength must be > 0");
            assert!(T::MaxSegmentCount::get() > 0, "MaxSegmentCount must be > 0");
        }
    }

    // ========================================================================
    // Storage
    // ========================================================================

    #[pallet::storage]
    #[pallet::getter(fn single_line_config)]
    pub type SingleLineConfigs<T: Config> = StorageMap<
        _, Blake2_128Concat, u64, SingleLineConfig<BalanceOf<T>>,
    >;

    #[pallet::storage]
    pub type SingleLineSegments<T: Config> = StorageDoubleMap<
        _, Blake2_128Concat, u64, Blake2_128Concat, u32,
        BoundedVec<T::AccountId, T::MaxSingleLineLength>, ValueQuery,
    >;

    #[pallet::storage]
    pub type SingleLineSegmentCount<T: Config> = StorageMap<
        _, Blake2_128Concat, u64, u32, ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn single_line_index)]
    pub type SingleLineIndex<T: Config> = StorageDoubleMap<
        _, Blake2_128Concat, u64, Blake2_128Concat, T::AccountId, u32,
    >;

    #[pallet::storage]
    #[pallet::getter(fn custom_level_overrides)]
    pub type SingleLineCustomLevelOverrides<T: Config> = StorageDoubleMap<
        _, Blake2_128Concat, u64, Blake2_128Concat, u8, LevelBasedLevels,
    >;

    #[pallet::storage]
    pub type SingleLineEnabled<T: Config> = StorageMap<
        _, Blake2_128Concat, u64, bool, ValueQuery, EnabledDefault,
    >;

    #[pallet::type_value]
    pub fn EnabledDefault() -> bool { true }

    #[pallet::storage]
    pub type PendingConfigChanges<T: Config> = StorageMap<
        _, Blake2_128Concat, u64,
        PendingConfigChange<BalanceOf<T>, BlockNumberFor<T>>,
    >;

    #[pallet::storage]
    pub type RemovedMembers<T: Config> = StorageDoubleMap<
        _, Blake2_128Concat, u64, Blake2_128Concat, T::AccountId, bool, ValueQuery,
    >;

    #[pallet::storage]
    pub type ConfigChangeLogs<T: Config> = StorageDoubleMap<
        _, Blake2_128Concat, u64, Blake2_128Concat, u32,
        ConfigChangeLogEntry<BalanceOf<T>, BlockNumberFor<T>>,
    >;

    #[pallet::storage]
    pub type ConfigChangeLogCount<T: Config> = StorageMap<
        _, Blake2_128Concat, u64, u32, ValueQuery,
    >;

    #[pallet::storage]
    pub type EntitySingleLineStats<T: Config> = StorageMap<
        _, Blake2_128Concat, u64, EntitySingleLineStatsData, ValueQuery,
    >;

    // ========================================================================
    // Events / Errors
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        SingleLineConfigUpdated {
            entity_id: u64, upline_rate: u16, downline_rate: u16,
            base_upline_levels: u8, base_downline_levels: u8,
            max_upline_levels: u8, max_downline_levels: u8,
        },
        SingleLineConfigCleared { entity_id: u64 },
        AddedToSingleLine { entity_id: u64, account: T::AccountId, index: u32 },
        SingleLineJoinFailed { entity_id: u64, account: T::AccountId },
        LevelBasedLevelsUpdated { entity_id: u64, level_id: u8 },
        LevelBasedLevelsRemoved { entity_id: u64, level_id: u8 },
        SingleLinePaused { entity_id: u64 },
        SingleLineResumed { entity_id: u64 },
        SingleLineReset { entity_id: u64, removed_count: u32 },
        SingleLineResetCompleted { entity_id: u64 },
        NewSegmentCreated { entity_id: u64, segment_id: u32 },
        AllLevelOverridesCleared { entity_id: u64 },
        ConfigChangeScheduled { entity_id: u64, apply_after: BlockNumberFor<T> },
        PendingConfigApplied { entity_id: u64 },
        PendingConfigCancelled { entity_id: u64 },
        MemberRemovedFromSingleLine { entity_id: u64, account: T::AccountId },
        MemberRestoredToSingleLine { entity_id: u64, account: T::AccountId },
    }

    #[pallet::error]
    pub enum Error<T> {
        InvalidRate,
        InvalidLevels,
        BaseLevelsExceedMax,
        EntityNotFound,
        NotEntityOwnerOrAdmin,
        ConfigNotFound,
        NothingToUpdate,
        EntityLocked,
        EntityNotActive,
        SingleLineIsPaused,
        SingleLineNotPaused,
        PendingConfigAlreadyExists,
        PendingConfigNotFound,
        PendingConfigNotReady,
        MemberNotInSingleLine,
        MaxSegmentCountReached,
        RatesTooHigh,
        LevelOverrideExceedsMax,
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_single_line_config())]
        pub fn set_single_line_config(
            origin: OriginFor<T>,
            entity_id: u64,
            upline_rate: u16,
            downline_rate: u16,
            base_upline_levels: u8,
            base_downline_levels: u8,
            level_increment_threshold: BalanceOf<T>,
            max_upline_levels: u8,
            max_downline_levels: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            Self::validate_config(upline_rate, downline_rate, base_upline_levels, base_downline_levels, max_upline_levels, max_downline_levels)?;

            let config = SingleLineConfig {
                upline_rate, downline_rate, base_upline_levels, base_downline_levels,
                level_increment_threshold, max_upline_levels, max_downline_levels,
            };
            SingleLineConfigs::<T>::insert(entity_id, &config);
            Self::record_change_log(entity_id, &config);
            Self::deposit_event(Event::SingleLineConfigUpdated {
                entity_id, upline_rate, downline_rate,
                base_upline_levels, base_downline_levels,
                max_upline_levels, max_downline_levels,
            });
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::clear_single_line_config())]
        pub fn clear_single_line_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(SingleLineConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);

            SingleLineConfigs::<T>::remove(entity_id);
            Self::do_clear_all_level_overrides(entity_id);
            Self::deposit_event(Event::SingleLineConfigCleared { entity_id });
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::update_single_line_params())]
        pub fn update_single_line_params(
            origin: OriginFor<T>,
            entity_id: u64,
            upline_rate: Option<u16>,
            downline_rate: Option<u16>,
            level_increment_threshold: Option<BalanceOf<T>>,
            base_upline_levels: Option<u8>,
            base_downline_levels: Option<u8>,
            max_upline_levels: Option<u8>,
            max_downline_levels: Option<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(
                upline_rate.is_some() || downline_rate.is_some() || level_increment_threshold.is_some()
                || base_upline_levels.is_some() || base_downline_levels.is_some()
                || max_upline_levels.is_some() || max_downline_levels.is_some(),
                Error::<T>::NothingToUpdate
            );

            let config = SingleLineConfigs::<T>::try_mutate(entity_id, |maybe| -> Result<SingleLineConfig<BalanceOf<T>>, DispatchError> {
                let c = maybe.as_mut().ok_or(Error::<T>::ConfigNotFound)?;
                if let Some(r) = upline_rate   { ensure!(r <= 1000, Error::<T>::InvalidRate); c.upline_rate = r; }
                if let Some(r) = downline_rate { ensure!(r <= 1000, Error::<T>::InvalidRate); c.downline_rate = r; }
                if let Some(t) = level_increment_threshold { c.level_increment_threshold = t; }
                if let Some(v) = base_upline_levels   { c.base_upline_levels = v; }
                if let Some(v) = base_downline_levels { c.base_downline_levels = v; }
                if let Some(v) = max_upline_levels    { c.max_upline_levels = v; }
                if let Some(v) = max_downline_levels  { c.max_downline_levels = v; }
                ensure!(c.base_upline_levels <= c.max_upline_levels, Error::<T>::BaseLevelsExceedMax);
                ensure!(c.base_downline_levels <= c.max_downline_levels, Error::<T>::BaseLevelsExceedMax);
                let max_payout = (c.upline_rate as u32).saturating_mul(c.max_upline_levels as u32)
                    .saturating_add((c.downline_rate as u32).saturating_mul(c.max_downline_levels as u32));
                ensure!(max_payout <= T::MaxTotalRateBps::get(), Error::<T>::RatesTooHigh);
                Ok(c.clone())
            })?;

            Self::record_change_log(entity_id, &config);
            Self::deposit_event(Event::SingleLineConfigUpdated {
                entity_id,
                upline_rate: config.upline_rate, downline_rate: config.downline_rate,
                base_upline_levels: config.base_upline_levels, base_downline_levels: config.base_downline_levels,
                max_upline_levels: config.max_upline_levels, max_downline_levels: config.max_downline_levels,
            });
            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::set_level_based_levels())]
        pub fn set_level_based_levels(
            origin: OriginFor<T>,
            entity_id: u64,
            level_id: u8,
            upline_levels: u8,
            downline_levels: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(upline_levels > 0 || downline_levels > 0, Error::<T>::InvalidLevels);

            if let Some(config) = SingleLineConfigs::<T>::get(entity_id) {
                ensure!(
                    upline_levels <= config.max_upline_levels && downline_levels <= config.max_downline_levels,
                    Error::<T>::LevelOverrideExceedsMax
                );
            }

            SingleLineCustomLevelOverrides::<T>::insert(entity_id, level_id, LevelBasedLevels { upline_levels, downline_levels });
            Self::deposit_event(Event::LevelBasedLevelsUpdated { entity_id, level_id });
            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::remove_level_based_levels())]
        pub fn remove_level_based_levels(
            origin: OriginFor<T>,
            entity_id: u64,
            level_id: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            if SingleLineCustomLevelOverrides::<T>::contains_key(entity_id, level_id) {
                SingleLineCustomLevelOverrides::<T>::remove(entity_id, level_id);
                Self::deposit_event(Event::LevelBasedLevelsRemoved { entity_id, level_id });
            }
            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::force_set_single_line_config())]
        pub fn force_set_single_line_config(
            origin: OriginFor<T>,
            entity_id: u64,
            upline_rate: u16,
            downline_rate: u16,
            base_upline_levels: u8,
            base_downline_levels: u8,
            level_increment_threshold: BalanceOf<T>,
            max_upline_levels: u8,
            max_downline_levels: u8,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::validate_config(upline_rate, downline_rate, base_upline_levels, base_downline_levels, max_upline_levels, max_downline_levels)?;

            let config = SingleLineConfig {
                upline_rate, downline_rate, base_upline_levels, base_downline_levels,
                level_increment_threshold, max_upline_levels, max_downline_levels,
            };
            SingleLineConfigs::<T>::insert(entity_id, &config);
            Self::record_change_log(entity_id, &config);
            Self::deposit_event(Event::SingleLineConfigUpdated {
                entity_id, upline_rate, downline_rate,
                base_upline_levels, base_downline_levels,
                max_upline_levels, max_downline_levels,
            });
            Ok(())
        }

        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::force_clear_single_line_config())]
        pub fn force_clear_single_line_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            if SingleLineConfigs::<T>::contains_key(entity_id) {
                SingleLineConfigs::<T>::remove(entity_id);
                Self::do_clear_all_level_overrides(entity_id);
                Self::deposit_event(Event::SingleLineConfigCleared { entity_id });
            }
            Ok(())
        }

        /// 分批重置单链数据（从末尾段开始，每次最多处理 `limit` 个段）
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::force_reset_single_line(*limit))]
        pub fn force_reset_single_line(
            origin: OriginFor<T>,
            entity_id: u64,
            limit: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let (removed, done) = Self::do_reset_single_line_batched(entity_id, limit);
            if removed > 0 {
                Self::deposit_event(Event::SingleLineReset { entity_id, removed_count: removed });
            }
            if done {
                Self::deposit_event(Event::SingleLineResetCompleted { entity_id });
            }
            Ok(())
        }

        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::pause_single_line())]
        pub fn pause_single_line(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(SingleLineEnabled::<T>::get(entity_id), Error::<T>::SingleLineIsPaused);

            SingleLineEnabled::<T>::insert(entity_id, false);
            Self::deposit_event(Event::SingleLinePaused { entity_id });
            Ok(())
        }

        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::resume_single_line())]
        pub fn resume_single_line(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(!SingleLineEnabled::<T>::get(entity_id), Error::<T>::SingleLineNotPaused);

            SingleLineEnabled::<T>::insert(entity_id, true);
            Self::deposit_event(Event::SingleLineResumed { entity_id });
            Ok(())
        }

        /// 调度配置变更（延迟 ConfigChangeDelay 个区块后才可 apply）
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::schedule_config_change())]
        pub fn schedule_config_change(
            origin: OriginFor<T>,
            entity_id: u64,
            upline_rate: u16,
            downline_rate: u16,
            base_upline_levels: u8,
            base_downline_levels: u8,
            level_increment_threshold: BalanceOf<T>,
            max_upline_levels: u8,
            max_downline_levels: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(!PendingConfigChanges::<T>::contains_key(entity_id), Error::<T>::PendingConfigAlreadyExists);
            Self::validate_config(upline_rate, downline_rate, base_upline_levels, base_downline_levels, max_upline_levels, max_downline_levels)?;

            let apply_after = <frame_system::Pallet<T>>::block_number()
                .saturating_add(T::ConfigChangeDelay::get());

            PendingConfigChanges::<T>::insert(entity_id, PendingConfigChange {
                upline_rate, downline_rate, base_upline_levels, base_downline_levels,
                level_increment_threshold, max_upline_levels, max_downline_levels, apply_after,
            });
            Self::deposit_event(Event::ConfigChangeScheduled { entity_id, apply_after });
            Ok(())
        }

        /// 应用待生效配置（任何人可调用，需过延迟期）
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::apply_pending_config())]
        pub fn apply_pending_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            let pending = PendingConfigChanges::<T>::get(entity_id)
                .ok_or(Error::<T>::PendingConfigNotFound)?;
            ensure!(
                <frame_system::Pallet<T>>::block_number() >= pending.apply_after,
                Error::<T>::PendingConfigNotReady
            );

            let config = SingleLineConfig {
                upline_rate: pending.upline_rate, downline_rate: pending.downline_rate,
                base_upline_levels: pending.base_upline_levels, base_downline_levels: pending.base_downline_levels,
                level_increment_threshold: pending.level_increment_threshold,
                max_upline_levels: pending.max_upline_levels, max_downline_levels: pending.max_downline_levels,
            };
            SingleLineConfigs::<T>::insert(entity_id, &config);
            PendingConfigChanges::<T>::remove(entity_id);
            Self::record_change_log(entity_id, &config);

            Self::deposit_event(Event::SingleLineConfigUpdated {
                entity_id,
                upline_rate: config.upline_rate, downline_rate: config.downline_rate,
                base_upline_levels: config.base_upline_levels, base_downline_levels: config.base_downline_levels,
                max_upline_levels: config.max_upline_levels, max_downline_levels: config.max_downline_levels,
            });
            Self::deposit_event(Event::PendingConfigApplied { entity_id });
            Ok(())
        }

        /// 取消待生效配置
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::cancel_pending_config())]
        pub fn cancel_pending_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(PendingConfigChanges::<T>::contains_key(entity_id), Error::<T>::PendingConfigNotFound);

            PendingConfigChanges::<T>::remove(entity_id);
            Self::deposit_event(Event::PendingConfigCancelled { entity_id });
            Ok(())
        }

        /// [Root] 逻辑移除成员（遍历时跳过，不改变链结构）
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::force_remove_from_single_line())]
        pub fn force_remove_from_single_line(
            origin: OriginFor<T>,
            entity_id: u64,
            account: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(SingleLineIndex::<T>::contains_key(entity_id, &account), Error::<T>::MemberNotInSingleLine);
            RemovedMembers::<T>::insert(entity_id, &account, true);
            Self::deposit_event(Event::MemberRemovedFromSingleLine { entity_id, account });
            Ok(())
        }

        /// [Root] 恢复已逻辑移除的成员
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::force_remove_from_single_line())]
        pub fn force_restore_to_single_line(
            origin: OriginFor<T>,
            entity_id: u64,
            account: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(RemovedMembers::<T>::get(entity_id, &account), Error::<T>::MemberNotInSingleLine);
            RemovedMembers::<T>::remove(entity_id, &account);
            Self::deposit_event(Event::MemberRestoredToSingleLine { entity_id, account });
            Ok(())
        }
    }

    // ========================================================================
    // Internal functions
    // ========================================================================

    impl<T: Config> Pallet<T> {
        fn ensure_owner_or_admin(entity_id: u64, who: &T::AccountId) -> DispatchResult {
            let owner = T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            if *who == owner { return Ok(()); }
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, who, AdminPermission::COMMISSION_MANAGE),
                Error::<T>::NotEntityOwnerOrAdmin
            );
            Ok(())
        }

        pub(crate) fn validate_config(
            upline_rate: u16, downline_rate: u16,
            base_upline_levels: u8, base_downline_levels: u8,
            max_upline_levels: u8, max_downline_levels: u8,
        ) -> DispatchResult {
            ensure!(upline_rate <= 1000 && downline_rate <= 1000, Error::<T>::InvalidRate);
            ensure!(base_upline_levels <= max_upline_levels, Error::<T>::BaseLevelsExceedMax);
            ensure!(base_downline_levels <= max_downline_levels, Error::<T>::BaseLevelsExceedMax);
            let max_payout = (upline_rate as u32).saturating_mul(max_upline_levels as u32)
                .saturating_add((downline_rate as u32).saturating_mul(max_downline_levels as u32));
            ensure!(max_payout <= T::MaxTotalRateBps::get(), Error::<T>::RatesTooHigh);
            Ok(())
        }

        pub(crate) fn effective_base_levels(
            entity_id: u64, buyer: &T::AccountId, config: &SingleLineConfig<BalanceOf<T>>,
        ) -> (u8, u8) {
            let level_id = T::MemberLevelProvider::custom_level_id(entity_id, buyer);
            if let Some(o) = SingleLineCustomLevelOverrides::<T>::get(entity_id, level_id) {
                return (o.upline_levels, o.downline_levels);
            }
            (config.base_upline_levels, config.base_downline_levels)
        }

        pub(crate) fn add_to_single_line(entity_id: u64, account: &T::AccountId) -> DispatchResult {
            if SingleLineIndex::<T>::contains_key(entity_id, account) {
                return Ok(());
            }

            let seg_size = T::MaxSingleLineLength::get();
            let seg_count = SingleLineSegmentCount::<T>::get(entity_id);

            if seg_count > 0 {
                let last_seg_id = seg_count - 1;
                let mut seg = SingleLineSegments::<T>::get(entity_id, last_seg_id);
                if (seg.len() as u32) < seg_size {
                    let global_index = last_seg_id * seg_size + seg.len() as u32;
                    seg.try_push(account.clone()).map_err(|_| DispatchError::Other("SegmentPushFailed"))?;
                    SingleLineSegments::<T>::insert(entity_id, last_seg_id, seg);
                    SingleLineIndex::<T>::insert(entity_id, account, global_index);
                    Self::deposit_event(Event::AddedToSingleLine {
                        entity_id, account: account.clone(), index: global_index,
                    });
                    return Ok(());
                }
            }

            let new_seg_id = seg_count;
            ensure!(new_seg_id < T::MaxSegmentCount::get(), Error::<T>::MaxSegmentCountReached);
            let global_index = new_seg_id * seg_size;
            let mut new_seg = BoundedVec::<T::AccountId, T::MaxSingleLineLength>::default();
            new_seg.try_push(account.clone()).map_err(|_| DispatchError::Other("SegmentPushFailed"))?;
            SingleLineSegments::<T>::insert(entity_id, new_seg_id, new_seg);
            SingleLineSegmentCount::<T>::insert(entity_id, new_seg_id + 1);
            SingleLineIndex::<T>::insert(entity_id, account, global_index);

            Self::deposit_event(Event::NewSegmentCreated { entity_id, segment_id: new_seg_id });
            Self::deposit_event(Event::AddedToSingleLine {
                entity_id, account: account.clone(), index: global_index,
            });
            Ok(())
        }

        pub(crate) fn calc_extra_levels(threshold: BalanceOf<T>, total_earned: BalanceOf<T>) -> u8 {
            if threshold.is_zero() { return 0; }
            let threshold_u128: u128 = sp_runtime::SaturatedConversion::saturated_into(threshold);
            let earned_u128: u128 = sp_runtime::SaturatedConversion::saturated_into(total_earned);
            (earned_u128 / threshold_u128).min(255) as u8
        }

        fn is_member_skipped(entity_id: u64, account: &T::AccountId) -> bool {
            T::MemberProvider::is_banned(entity_id, account)
                || !T::MemberProvider::is_activated(entity_id, account)
                || !T::MemberProvider::is_member_active(entity_id, account)
                || RemovedMembers::<T>::get(entity_id, account)
        }

        pub fn process_upline<B>(
            entity_id: u64, buyer: &T::AccountId, order_amount: B, remaining: &mut B,
            config: &SingleLineConfig<BalanceOf<T>>, base_up: u8,
            outputs: &mut Vec<CommissionOutput<T::AccountId, B>>,
        ) where B: AtLeast32BitUnsigned + Copy {
            if config.upline_rate == 0 { return; }
            let buyer_index = match SingleLineIndex::<T>::get(entity_id, buyer) {
                Some(idx) => idx, None => return,
            };
            if buyer_index == 0 { return; }

            let seg_size = T::MaxSingleLineLength::get();
            let buyer_stats = T::StatsProvider::get_member_stats(entity_id, buyer);
            let extra_levels = Self::calc_extra_levels(config.level_increment_threshold, buyer_stats.total_earned);
            let max_levels = base_up.saturating_add(extra_levels).min(config.max_upline_levels) as u32;

            let mut cur_seg_id = buyer_index / seg_size;
            let mut cur_seg = SingleLineSegments::<T>::get(entity_id, cur_seg_id);

            for i in 1..=max_levels {
                if buyer_index < i { break; }
                let target_index = buyer_index - i;
                let seg_id = target_index / seg_size;
                if seg_id != cur_seg_id {
                    cur_seg = SingleLineSegments::<T>::get(entity_id, seg_id);
                    cur_seg_id = seg_id;
                }
                let local_pos = (target_index % seg_size) as usize;
                let upline = match cur_seg.get(local_pos) { Some(a) => a, None => break };

                if Self::is_member_skipped(entity_id, upline) { continue; }

                let commission = order_amount
                    .saturating_mul(B::from(config.upline_rate as u32))
                    / B::from(10000u32);
                let actual = commission.min(*remaining);

                if !actual.is_zero() {
                    *remaining = remaining.saturating_sub(actual);
                    outputs.push(CommissionOutput {
                        beneficiary: upline.clone(), amount: actual,
                        commission_type: CommissionType::SingleLineUpline, level: i as u8,
                    });
                }
            }
        }

        pub fn process_downline<B>(
            entity_id: u64, buyer: &T::AccountId, order_amount: B, remaining: &mut B,
            config: &SingleLineConfig<BalanceOf<T>>, base_down: u8,
            outputs: &mut Vec<CommissionOutput<T::AccountId, B>>,
        ) where B: AtLeast32BitUnsigned + Copy {
            if config.downline_rate == 0 { return; }
            let buyer_index = match SingleLineIndex::<T>::get(entity_id, buyer) {
                Some(idx) => idx, None => return,
            };
            let total_len = Self::single_line_length(entity_id);
            if buyer_index >= total_len.saturating_sub(1) { return; }

            let seg_size = T::MaxSingleLineLength::get();
            let buyer_stats = T::StatsProvider::get_member_stats(entity_id, buyer);
            let extra_levels = Self::calc_extra_levels(config.level_increment_threshold, buyer_stats.total_earned);
            let max_levels = base_down.saturating_add(extra_levels).min(config.max_downline_levels) as u32;

            let mut cur_seg_id = buyer_index / seg_size;
            let mut cur_seg = SingleLineSegments::<T>::get(entity_id, cur_seg_id);

            for i in 1..=max_levels {
                let target_index = buyer_index.saturating_add(i);
                if target_index >= total_len { break; }
                let seg_id = target_index / seg_size;
                if seg_id != cur_seg_id {
                    cur_seg = SingleLineSegments::<T>::get(entity_id, seg_id);
                    cur_seg_id = seg_id;
                }
                let local_pos = (target_index % seg_size) as usize;
                let downline = match cur_seg.get(local_pos) { Some(a) => a, None => break };

                if Self::is_member_skipped(entity_id, downline) { continue; }

                let commission = order_amount
                    .saturating_mul(B::from(config.downline_rate as u32))
                    / B::from(10000u32);
                let actual = commission.min(*remaining);

                if !actual.is_zero() {
                    *remaining = remaining.saturating_sub(actual);
                    outputs.push(CommissionOutput {
                        beneficiary: downline.clone(), amount: actual,
                        commission_type: CommissionType::SingleLineDownline, level: i as u8,
                    });
                }
            }
        }

        pub(crate) fn do_reset_single_line_batched(entity_id: u64, max_segments: u32) -> (u32, bool) {
            let seg_count = SingleLineSegmentCount::<T>::get(entity_id);
            if seg_count == 0 { return (0, true); }

            let to_process = max_segments.min(seg_count);
            let mut total_removed = 0u32;

            for i in 0..to_process {
                let seg_id = seg_count - 1 - i;
                let seg = SingleLineSegments::<T>::take(entity_id, seg_id);
                for account in seg.iter() {
                    SingleLineIndex::<T>::remove(entity_id, account);
                }
                total_removed = total_removed.saturating_add(seg.len() as u32);
            }

            let remaining_segs = seg_count - to_process;
            if remaining_segs == 0 {
                SingleLineSegmentCount::<T>::remove(entity_id);
                (total_removed, true)
            } else {
                SingleLineSegmentCount::<T>::insert(entity_id, remaining_segs);
                (total_removed, false)
            }
        }

        pub(crate) fn do_clear_all_level_overrides(entity_id: u64) {
            let has_overrides = SingleLineCustomLevelOverrides::<T>::iter_prefix(entity_id).next().is_some();
            let _ = SingleLineCustomLevelOverrides::<T>::clear_prefix(entity_id, u32::MAX, None);
            if has_overrides {
                Self::deposit_event(Event::AllLevelOverridesCleared { entity_id });
            }
        }

        pub(crate) fn record_change_log(entity_id: u64, config: &SingleLineConfig<BalanceOf<T>>) {
            let count = ConfigChangeLogCount::<T>::get(entity_id);
            let max = T::MaxConfigChangeLogs::get();
            let slot = if max > 0 { count % max } else { 0 };
            ConfigChangeLogs::<T>::insert(entity_id, slot, ConfigChangeLogEntry {
                block_number: <frame_system::Pallet<T>>::block_number(),
                upline_rate: config.upline_rate, downline_rate: config.downline_rate,
                base_upline_levels: config.base_upline_levels, base_downline_levels: config.base_downline_levels,
                max_upline_levels: config.max_upline_levels, max_downline_levels: config.max_downline_levels,
                level_increment_threshold: config.level_increment_threshold,
            });
            ConfigChangeLogCount::<T>::insert(entity_id, count.saturating_add(1));
        }

        // ====================================================================
        // Query helpers
        // ====================================================================

        pub fn single_line_length(entity_id: u64) -> u32 {
            let seg_size = T::MaxSingleLineLength::get();
            let seg_count = SingleLineSegmentCount::<T>::get(entity_id);
            if seg_count == 0 { return 0; }
            let last_seg = SingleLineSegments::<T>::get(entity_id, seg_count - 1);
            (seg_count - 1) * seg_size + last_seg.len() as u32
        }

        pub fn single_line_remaining_capacity(entity_id: u64) -> u32 {
            let seg_size = T::MaxSingleLineLength::get();
            let seg_count = SingleLineSegmentCount::<T>::get(entity_id);
            if seg_count == 0 { return seg_size; }
            let last_seg = SingleLineSegments::<T>::get(entity_id, seg_count - 1);
            seg_size.saturating_sub(last_seg.len() as u32)
        }

        pub fn user_position(entity_id: u64, account: &T::AccountId) -> Option<u32> {
            SingleLineIndex::<T>::get(entity_id, account)
        }

        pub fn user_effective_levels(entity_id: u64, account: &T::AccountId) -> Option<(u8, u8)> {
            let config = SingleLineConfigs::<T>::get(entity_id)?;
            let (base_up, base_down) = Self::effective_base_levels(entity_id, account, &config);
            let stats = T::StatsProvider::get_member_stats(entity_id, account);
            let extra = Self::calc_extra_levels(config.level_increment_threshold, stats.total_earned);
            Some((
                base_up.saturating_add(extra).min(config.max_upline_levels),
                base_down.saturating_add(extra).min(config.max_downline_levels),
            ))
        }

        pub fn is_single_line_enabled(entity_id: u64) -> bool {
            SingleLineEnabled::<T>::get(entity_id)
        }

        pub fn preview_single_line_commission(
            entity_id: u64, buyer: &T::AccountId, order_amount: BalanceOf<T>,
        ) -> Vec<CommissionOutput<T::AccountId, BalanceOf<T>>> {
            let config = match SingleLineConfigs::<T>::get(entity_id) {
                Some(c) => c, None => return Vec::new(),
            };
            let mut remaining = order_amount;
            let mut outputs = Vec::new();
            let (base_up, base_down) = Self::effective_base_levels(entity_id, buyer, &config);
            Self::process_upline(entity_id, buyer, order_amount, &mut remaining, &config, base_up, &mut outputs);
            Self::process_downline(entity_id, buyer, order_amount, &mut remaining, &config, base_down, &mut outputs);
            outputs
        }
    }
}

// ============================================================================
// Unified calculate logic
// ============================================================================

impl<T: pallet::Config> pallet::Pallet<T> {
    fn do_calculate<B>(
        entity_id: u64, buyer: &T::AccountId, order_amount: B, remaining: B,
        enabled_modes: pallet_commission_common::CommissionModes, _is_first_order: bool,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, B>>, B)
    where B: sp_runtime::traits::AtLeast32BitUnsigned + Copy,
    {
        use pallet_commission_common::CommissionModes;

        if !T::EntityProvider::is_entity_active(entity_id) {
            return (alloc::vec::Vec::new(), remaining);
        }
        if !pallet::SingleLineEnabled::<T>::get(entity_id) {
            return (alloc::vec::Vec::new(), remaining);
        }
        let config = match pallet::SingleLineConfigs::<T>::get(entity_id) {
            Some(c) => c,
            None => return (alloc::vec::Vec::new(), remaining),
        };

        let has_upline = enabled_modes.contains(CommissionModes::SINGLE_LINE_UPLINE);
        let has_downline = enabled_modes.contains(CommissionModes::SINGLE_LINE_DOWNLINE);
        if !has_upline && !has_downline {
            return (alloc::vec::Vec::new(), remaining);
        }

        let buyer_in_chain = pallet::SingleLineIndex::<T>::contains_key(entity_id, buyer);
        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();
        let (base_up, base_down) = Self::effective_base_levels(entity_id, buyer, &config);

        if has_upline {
            Self::process_upline(entity_id, buyer, order_amount, &mut remaining, &config, base_up, &mut outputs);
        }
        if has_downline {
            Self::process_downline(entity_id, buyer, order_amount, &mut remaining, &config, base_down, &mut outputs);
        }

        if !outputs.is_empty() {
            pallet::EntitySingleLineStats::<T>::mutate(entity_id, |stats| {
                stats.total_orders = stats.total_orders.saturating_add(1);
                for o in &outputs {
                    match o.commission_type {
                        pallet_commission_common::CommissionType::SingleLineUpline => {
                            stats.total_upline_payouts = stats.total_upline_payouts.saturating_add(1);
                        }
                        pallet_commission_common::CommissionType::SingleLineDownline => {
                            stats.total_downline_payouts = stats.total_downline_payouts.saturating_add(1);
                        }
                        _ => {}
                    }
                }
            });
        }

        if !buyer_in_chain {
            if Self::add_to_single_line(entity_id, buyer).is_err() {
                Self::deposit_event(pallet::Event::SingleLineJoinFailed {
                    entity_id, account: buyer.clone(),
                });
            }
        }

        (outputs, remaining)
    }
}

// ============================================================================
// CommissionPlugin (NEX)
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::CommissionPlugin<T::AccountId, pallet::BalanceOf<T>> for pallet::Pallet<T> {
    fn calculate(
        entity_id: u64, buyer: &T::AccountId, order_amount: pallet::BalanceOf<T>,
        remaining: pallet::BalanceOf<T>, enabled_modes: pallet_commission_common::CommissionModes,
        is_first_order: bool, _buyer_order_count: u32,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, pallet::BalanceOf<T>>>, pallet::BalanceOf<T>) {
        Self::do_calculate(entity_id, buyer, order_amount, remaining, enabled_modes, is_first_order)
    }
}

// ============================================================================
// TokenCommissionPlugin
// ============================================================================

impl<T: pallet::Config, TB> pallet_commission_common::TokenCommissionPlugin<T::AccountId, TB>
    for pallet::Pallet<T>
where TB: sp_runtime::traits::AtLeast32BitUnsigned + Copy + Default + core::fmt::Debug,
{
    fn calculate_token(
        entity_id: u64, buyer: &T::AccountId, order_amount: TB, remaining: TB,
        enabled_modes: pallet_commission_common::CommissionModes,
        is_first_order: bool, _buyer_order_count: u32,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>, TB) {
        Self::do_calculate(entity_id, buyer, order_amount, remaining, enabled_modes, is_first_order)
    }
}

// ============================================================================
// SingleLinePlanWriter (R5: reuses validate_config)
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::SingleLinePlanWriter for pallet::Pallet<T> {
    fn set_single_line_config(
        entity_id: u64, upline_rate: u16, downline_rate: u16,
        base_upline_levels: u8, base_downline_levels: u8,
        level_increment_threshold: u128, max_upline_levels: u8, max_downline_levels: u8,
    ) -> Result<(), sp_runtime::DispatchError> {
        Self::validate_config(
            upline_rate, downline_rate,
            base_upline_levels, base_downline_levels,
            max_upline_levels, max_downline_levels,
        )?;

        let threshold: pallet::BalanceOf<T> =
            sp_runtime::SaturatedConversion::saturated_into(level_increment_threshold);
        let config = pallet::SingleLineConfig {
            upline_rate, downline_rate, base_upline_levels, base_downline_levels,
            level_increment_threshold: threshold, max_upline_levels, max_downline_levels,
        };
        pallet::SingleLineConfigs::<T>::insert(entity_id, &config);
        Self::record_change_log(entity_id, &config);
        pallet::Pallet::<T>::deposit_event(pallet::Event::SingleLineConfigUpdated {
            entity_id, upline_rate, downline_rate,
            base_upline_levels, base_downline_levels,
            max_upline_levels, max_downline_levels,
        });
        Ok(())
    }

    fn clear_config(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        if pallet::SingleLineConfigs::<T>::contains_key(entity_id) {
            pallet::SingleLineConfigs::<T>::remove(entity_id);
            Self::do_clear_all_level_overrides(entity_id);
            pallet::Pallet::<T>::deposit_event(pallet::Event::SingleLineConfigCleared { entity_id });
        }
        Ok(())
    }

    fn set_level_based_levels(
        entity_id: u64, level_id: u8, upline_levels: u8, downline_levels: u8,
    ) -> Result<(), sp_runtime::DispatchError> {
        frame_support::ensure!(
            upline_levels > 0 || downline_levels > 0,
            sp_runtime::DispatchError::Other("InvalidLevels")
        );
        pallet::SingleLineCustomLevelOverrides::<T>::insert(
            entity_id, level_id,
            pallet::LevelBasedLevels { upline_levels, downline_levels },
        );
        pallet::Pallet::<T>::deposit_event(pallet::Event::LevelBasedLevelsUpdated { entity_id, level_id });
        Ok(())
    }

    fn clear_level_overrides(entity_id: u64, level_id: u8) -> Result<(), sp_runtime::DispatchError> {
        if pallet::SingleLineCustomLevelOverrides::<T>::contains_key(entity_id, level_id) {
            pallet::SingleLineCustomLevelOverrides::<T>::remove(entity_id, level_id);
            pallet::Pallet::<T>::deposit_event(pallet::Event::LevelBasedLevelsRemoved { entity_id, level_id });
        }
        Ok(())
    }
}

// ============================================================================
// SingleLineQueryProvider 实现
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::SingleLineQueryProvider<T::AccountId> for pallet::Pallet<T> {
    fn position(entity_id: u64, account: &T::AccountId) -> Option<u32> {
        Self::user_position(entity_id, account)
    }

    fn effective_levels(entity_id: u64, account: &T::AccountId) -> Option<(u8, u8)> {
        Self::user_effective_levels(entity_id, account)
    }

    fn is_enabled(entity_id: u64) -> bool {
        Self::is_single_line_enabled(entity_id)
    }

    fn queue_length(entity_id: u64) -> u32 {
        Self::single_line_length(entity_id)
    }
}
