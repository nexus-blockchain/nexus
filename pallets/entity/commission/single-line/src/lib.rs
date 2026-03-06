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
    use frame_system::pallet_prelude::*;
    use pallet_commission_common::{
        CommissionModes, CommissionOutput, CommissionType, MemberCommissionStatsData,
        MemberProvider,
    };
    use pallet_entity_common::{EntityProvider, AdminPermission};
    use sp_runtime::traits::{AtLeast32BitUnsigned, Zero};

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // ========================================================================
    // Config structs
    // ========================================================================

    /// 单线收益配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct SingleLineConfig<Balance> {
        pub upline_rate: u16,
        pub downline_rate: u16,
        pub base_upline_levels: u8,
        pub base_downline_levels: u8,
        pub level_increment_threshold: Balance,
        pub max_upline_levels: u8,
        pub max_downline_levels: u8,
    }

    impl<Balance: Default> Default for SingleLineConfig<Balance> {
        fn default() -> Self {
            Self {
                upline_rate: 10,
                downline_rate: 10,
                base_upline_levels: 10,
                base_downline_levels: 15,
                level_increment_threshold: Balance::default(),
                max_upline_levels: 20,
                max_downline_levels: 30,
            }
        }
    }

    /// 按会员等级自定义的层数配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct LevelBasedLevels {
        pub upline_levels: u8,
        pub downline_levels: u8,
    }

    // ========================================================================
    // Pallet Config
    // ========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: Currency<Self::AccountId>;

        /// 用于查询买家累计收益（从 core 的 MemberCommissionStats 读取）
        type StatsProvider: SingleLineStatsProvider<Self::AccountId, BalanceOf<Self>>;

        /// 用于查询买家会员等级 ID（可选，用于按等级自定义层数）
        type MemberLevelProvider: SingleLineMemberLevelProvider<Self::AccountId>;

        /// 实体查询接口（权限校验、Owner/Admin 判断）
        type EntityProvider: EntityProvider<Self::AccountId>;

        /// 会员查询接口（is_banned 检查）
        type MemberProvider: pallet_commission_common::MemberProvider<Self::AccountId>;

        #[pallet::constant]
        type MaxSingleLineLength: Get<u32>;
    }

    /// 统计查询接口（由 core pallet 实现）
    pub trait SingleLineStatsProvider<AccountId, Balance: Default> {
        fn get_member_stats(entity_id: u64, account: &AccountId) -> MemberCommissionStatsData<Balance>;
    }

    /// 空实现
    impl<AccountId, Balance: Default> SingleLineStatsProvider<AccountId, Balance> for () {
        fn get_member_stats(_: u64, _: &AccountId) -> MemberCommissionStatsData<Balance> {
            MemberCommissionStatsData::default()
        }
    }

    /// 会员等级查询接口（用于按等级自定义层数）
    pub trait SingleLineMemberLevelProvider<AccountId> {
        /// 返回买家的有效自定义等级 ID（考虑过期回退）
        fn custom_level_id(entity_id: u64, account: &AccountId) -> u8;
    }

    /// 空实现（不区分等级，所有查询返回默认值）
    impl<AccountId> SingleLineMemberLevelProvider<AccountId> for () {
        fn custom_level_id(_: u64, _: &AccountId) -> u8 { 0 }
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        #[cfg(feature = "std")]
        fn integrity_test() {
            assert!(
                T::MaxSingleLineLength::get() > 0,
                "MaxSingleLineLength must be > 0"
            );
        }
    }

    // ========================================================================
    // Storage
    // ========================================================================

    /// 单线配置 entity_id -> SingleLineConfig
    #[pallet::storage]
    #[pallet::getter(fn single_line_config)]
    pub type SingleLineConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        SingleLineConfig<BalanceOf<T>>,
    >;

    /// 分段单链 (entity_id, segment_id) -> Vec<AccountId>
    /// 每段最多 MaxSingleLineLength 个账户，段满自动扩展新段
    #[pallet::storage]
    pub type SingleLineSegments<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, u32,
        BoundedVec<T::AccountId, T::MaxSingleLineLength>,
        ValueQuery,
    >;

    /// 单链段数 entity_id -> segment_count（0 表示空链）
    #[pallet::storage]
    pub type SingleLineSegmentCount<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        u32,
        ValueQuery,
    >;

    /// 用户在单链中的位置 (entity_id, account) -> index
    #[pallet::storage]
    #[pallet::getter(fn single_line_index)]
    pub type SingleLineIndex<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        u32,
    >;

    /// 等级层数覆盖 (entity_id, custom_level_id) -> LevelBasedLevels
    #[pallet::storage]
    #[pallet::getter(fn custom_level_overrides)]
    pub type SingleLineCustomLevelOverrides<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, u8,
        LevelBasedLevels,
    >;

    /// 单线收益启用状态 entity_id -> bool（默认 true，即未设置时视为启用）
    #[pallet::storage]
    pub type SingleLineEnabled<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        bool,
        ValueQuery,
        EnabledDefault,
    >;

    /// 默认启用
    #[pallet::type_value]
    pub fn EnabledDefault() -> bool { true }

    // ========================================================================
    // Events / Errors
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 单线收益配置已更新（包含完整参数快照）
        SingleLineConfigUpdated {
            entity_id: u64,
            upline_rate: u16,
            downline_rate: u16,
            base_upline_levels: u8,
            base_downline_levels: u8,
            max_upline_levels: u8,
            max_downline_levels: u8,
        },
        /// 单线收益配置已清除
        SingleLineConfigCleared { entity_id: u64 },
        AddedToSingleLine { entity_id: u64, account: T::AccountId, index: u32 },
        /// 单链加入失败（段满自动扩展后仍失败，理论上不应触发）
        SingleLineJoinFailed { entity_id: u64, account: T::AccountId },
        /// 按等级自定义层数已更新
        LevelBasedLevelsUpdated { entity_id: u64, level_id: u8 },
        /// 按等级自定义层数已移除
        LevelBasedLevelsRemoved { entity_id: u64, level_id: u8 },
        /// 单线收益已暂停
        SingleLinePaused { entity_id: u64 },
        /// 单线收益已恢复
        SingleLineResumed { entity_id: u64 },
        /// 单链数据已重置
        SingleLineReset { entity_id: u64, removed_count: u32 },
        /// 新段已创建（前一段已满，自动扩展）
        NewSegmentCreated { entity_id: u64, segment_id: u32 },
        /// 等级层数覆盖已全部清除
        AllLevelOverridesCleared { entity_id: u64 },
    }

    #[pallet::error]
    pub enum Error<T> {
        InvalidRate,
        InvalidLevels,
        /// base_upline_levels > max_upline_levels 或 base_downline_levels > max_downline_levels
        BaseLevelsExceedMax,
        /// 实体不存在
        EntityNotFound,
        /// 非实体所有者或无 COMMISSION_MANAGE 权限
        NotEntityOwnerOrAdmin,
        /// 配置不存在（清除/更新时）
        ConfigNotFound,
        /// 更新参数全部为 None（无操作）
        NothingToUpdate,
        /// 实体已被全局锁定，所有配置操作不可用
        EntityLocked,
        /// 实体非活跃状态（暂停/封禁）
        EntityNotActive,
        /// 单线收益已暂停
        SingleLineIsPaused,
        /// 单线收益未暂停（恢复时）
        SingleLineNotPaused,
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 设置单线收益配置（Entity Owner 或持有 COMMISSION_MANAGE 权限的 Admin）
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
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

            SingleLineConfigs::<T>::insert(entity_id, SingleLineConfig {
                upline_rate,
                downline_rate,
                base_upline_levels,
                base_downline_levels,
                level_increment_threshold,
                max_upline_levels,
                max_downline_levels,
            });

            Self::deposit_event(Event::SingleLineConfigUpdated {
                entity_id, upline_rate, downline_rate,
                base_upline_levels, base_downline_levels,
                max_upline_levels, max_downline_levels,
            });
            Ok(())
        }

        /// 清除单线收益配置（Entity Owner 或持有 COMMISSION_MANAGE 权限的 Admin）
        ///
        /// F12: 级联清理 SingleLineCustomLevelOverrides
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(35_000_000, 4_000))]
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

        /// 部分更新单线收益参数（不重提全部参数）
        ///
        /// F3: 新增 base_upline_levels、base_downline_levels、max_upline_levels、max_downline_levels 可选参数
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
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
                let config = maybe.as_mut().ok_or(Error::<T>::ConfigNotFound)?;
                if let Some(r) = upline_rate {
                    ensure!(r <= 1000, Error::<T>::InvalidRate);
                    config.upline_rate = r;
                }
                if let Some(r) = downline_rate {
                    ensure!(r <= 1000, Error::<T>::InvalidRate);
                    config.downline_rate = r;
                }
                if let Some(t) = level_increment_threshold {
                    config.level_increment_threshold = t;
                }
                if let Some(v) = base_upline_levels {
                    config.base_upline_levels = v;
                }
                if let Some(v) = base_downline_levels {
                    config.base_downline_levels = v;
                }
                if let Some(v) = max_upline_levels {
                    config.max_upline_levels = v;
                }
                if let Some(v) = max_downline_levels {
                    config.max_downline_levels = v;
                }
                ensure!(config.base_upline_levels <= config.max_upline_levels, Error::<T>::BaseLevelsExceedMax);
                ensure!(config.base_downline_levels <= config.max_downline_levels, Error::<T>::BaseLevelsExceedMax);
                Ok(config.clone())
            })?;

            Self::deposit_event(Event::SingleLineConfigUpdated {
                entity_id,
                upline_rate: config.upline_rate,
                downline_rate: config.downline_rate,
                base_upline_levels: config.base_upline_levels,
                base_downline_levels: config.base_downline_levels,
                max_upline_levels: config.max_upline_levels,
                max_downline_levels: config.max_downline_levels,
            });
            Ok(())
        }

        /// 设置按会员等级自定义的收益层数（Entity Owner 或 Admin）
        ///
        /// 当买家拥有对应等级时，使用此处的 upline_levels/downline_levels 替代
        /// SingleLineConfig 中的 base_upline_levels/base_downline_levels。
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(30_000_000, 4_000))]
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

            let levels = LevelBasedLevels { upline_levels, downline_levels };
            SingleLineCustomLevelOverrides::<T>::insert(entity_id, level_id, levels);

            Self::deposit_event(Event::LevelBasedLevelsUpdated { entity_id, level_id });
            Ok(())
        }

        /// 移除指定等级的自定义层数配置（Entity Owner 或 Admin）
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
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

        /// [Root] 强制设置单线收益配置
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
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

            SingleLineConfigs::<T>::insert(entity_id, SingleLineConfig {
                upline_rate,
                downline_rate,
                base_upline_levels,
                base_downline_levels,
                level_increment_threshold,
                max_upline_levels,
                max_downline_levels,
            });

            Self::deposit_event(Event::SingleLineConfigUpdated {
                entity_id, upline_rate, downline_rate,
                base_upline_levels, base_downline_levels,
                max_upline_levels, max_downline_levels,
            });
            Ok(())
        }

        /// [Root] 强制清除单线收益配置（F12: 级联清理 LevelOverrides）
        #[pallet::call_index(6)]
        #[pallet::weight(Weight::from_parts(35_000_000, 4_000))]
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

        /// F7: [Root] 强制重置单链数据（清除所有段 + SingleLineIndex）
        #[pallet::call_index(7)]
        #[pallet::weight(Weight::from_parts(100_000_000, 10_000))]
        pub fn force_reset_single_line(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let removed = Self::do_reset_single_line(entity_id);
            if removed > 0 {
                Self::deposit_event(Event::SingleLineReset { entity_id, removed_count: removed });
            }
            Ok(())
        }

        /// F2: 暂停单线收益计算（Entity Owner 或 Admin）
        #[pallet::call_index(8)]
        #[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
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

        /// F2: 恢复单线收益计算（Entity Owner 或 Admin）
        #[pallet::call_index(9)]
        #[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
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

    }

    // ========================================================================
    // Internal functions
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

        /// 校验配置参数合法性
        fn validate_config(
            upline_rate: u16,
            downline_rate: u16,
            base_upline_levels: u8,
            base_downline_levels: u8,
            max_upline_levels: u8,
            max_downline_levels: u8,
        ) -> DispatchResult {
            ensure!(upline_rate <= 1000 && downline_rate <= 1000, Error::<T>::InvalidRate);
            ensure!(base_upline_levels <= max_upline_levels, Error::<T>::BaseLevelsExceedMax);
            ensure!(base_downline_levels <= max_downline_levels, Error::<T>::BaseLevelsExceedMax);
            Ok(())
        }

        /// 获取买家的有效基础层数
        ///
        /// 查询买家的自定义等级 ID，并检查是否有对应的层数覆盖。
        /// 无覆盖时回退到 config 基础值。
        pub(crate) fn effective_base_levels(
            entity_id: u64,
            buyer: &T::AccountId,
            config: &SingleLineConfig<BalanceOf<T>>,
        ) -> (u8, u8) {
            let level_id = T::MemberLevelProvider::custom_level_id(entity_id, buyer);
            if let Some(o) = SingleLineCustomLevelOverrides::<T>::get(entity_id, level_id) {
                return (o.upline_levels, o.downline_levels);
            }
            (config.base_upline_levels, config.base_downline_levels)
        }

        /// 将用户加入单链（分段存储，段满自动扩展新段）
        pub fn add_to_single_line(entity_id: u64, account: &T::AccountId) -> DispatchResult {
            if SingleLineIndex::<T>::contains_key(entity_id, account) {
                return Ok(());
            }

            let seg_size = T::MaxSingleLineLength::get();
            let seg_count = SingleLineSegmentCount::<T>::get(entity_id);

            // 尝试在最后一段追加
            if seg_count > 0 {
                let last_seg_id = seg_count - 1;
                let mut seg = SingleLineSegments::<T>::get(entity_id, last_seg_id);
                if (seg.len() as u32) < seg_size {
                    let global_index = last_seg_id * seg_size + seg.len() as u32;
                    seg.try_push(account.clone()).map_err(|_| DispatchError::Other("SegmentPushFailed"))?;
                    SingleLineSegments::<T>::insert(entity_id, last_seg_id, seg);
                    SingleLineIndex::<T>::insert(entity_id, account, global_index);
                    Self::deposit_event(Event::AddedToSingleLine {
                        entity_id,
                        account: account.clone(),
                        index: global_index,
                    });
                    return Ok(());
                }
            }

            // 当前段已满（或无段），创建新段
            let new_seg_id = seg_count;
            let global_index = new_seg_id * seg_size;
            let mut new_seg = BoundedVec::<T::AccountId, T::MaxSingleLineLength>::default();
            new_seg.try_push(account.clone()).map_err(|_| DispatchError::Other("SegmentPushFailed"))?;
            SingleLineSegments::<T>::insert(entity_id, new_seg_id, new_seg);
            SingleLineSegmentCount::<T>::insert(entity_id, new_seg_id + 1);
            SingleLineIndex::<T>::insert(entity_id, account, global_index);

            Self::deposit_event(Event::NewSegmentCreated {
                entity_id,
                segment_id: new_seg_id,
            });
            Self::deposit_event(Event::AddedToSingleLine {
                entity_id,
                account: account.clone(),
                index: global_index,
            });
            Ok(())
        }

        pub(crate) fn calc_extra_levels(threshold: BalanceOf<T>, total_earned: BalanceOf<T>) -> u8 {
            if threshold.is_zero() {
                return 0;
            }
            let threshold_u128: u128 = sp_runtime::SaturatedConversion::saturated_into(threshold);
            let earned_u128: u128 = sp_runtime::SaturatedConversion::saturated_into(total_earned);
            // H4 审计修复: 防止 u8 溢出，限制最大值为 255
            // M2-R4: threshold.is_zero() 已提前返回，此处 threshold_u128 > 0 恒成立
            (earned_u128 / threshold_u128).min(255) as u8
        }

        /// L5-R3 审计修复: 泛型化，NEX (BalanceOf<T>) 和 Token (TB) 共用同一实现
        /// 分段单链: 按需加载段，跨段边界时最多额外加载 1 个段
        pub fn process_upline<B>(
            entity_id: u64,
            buyer: &T::AccountId,
            order_amount: B,
            remaining: &mut B,
            config: &SingleLineConfig<BalanceOf<T>>,
            base_up: u8,
            outputs: &mut Vec<CommissionOutput<T::AccountId, B>>,
        ) where
            B: AtLeast32BitUnsigned + Copy,
        {
            if config.upline_rate == 0 { return; }

            let buyer_index = match SingleLineIndex::<T>::get(entity_id, buyer) {
                Some(idx) => idx,
                None => return,
            };
            if buyer_index == 0 { return; }

            let seg_size = T::MaxSingleLineLength::get();
            let buyer_stats = T::StatsProvider::get_member_stats(entity_id, buyer);
            let extra_levels = Self::calc_extra_levels(config.level_increment_threshold, buyer_stats.total_earned);
            let max_levels = base_up
                .saturating_add(extra_levels)
                .min(config.max_upline_levels) as u32;

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
                let upline = match cur_seg.get(local_pos) {
                    Some(a) => a,
                    None => break,
                };

                // P0: 跳过被封禁或未激活会员（消耗 depth 但不发佣金）
                // M1-R5: 补充 is_member_active 检查（与 referral/multi-level 一致）
                if T::MemberProvider::is_banned(entity_id, upline)
                    || !T::MemberProvider::is_activated(entity_id, upline)
                    || !T::MemberProvider::is_member_active(entity_id, upline)
                {
                    continue;
                }

                // C2 审计修复: 佣金基于当前订单金额，而非受益人累计收益
                let commission = order_amount
                    .saturating_mul(B::from(config.upline_rate as u32))
                    / B::from(10000u32);
                let actual = commission.min(*remaining);

                if !actual.is_zero() {
                    *remaining = remaining.saturating_sub(actual);
                    outputs.push(CommissionOutput {
                        beneficiary: upline.clone(),
                        amount: actual,
                        commission_type: CommissionType::SingleLineUpline,
                        level: i as u8,
                    });
                }
            }
        }

        /// L5-R3 审计修复: 泛型化，NEX (BalanceOf<T>) 和 Token (TB) 共用同一实现
        /// 分段单链: 按需加载段，跨段边界时最多额外加载 1 个段
        pub fn process_downline<B>(
            entity_id: u64,
            buyer: &T::AccountId,
            order_amount: B,
            remaining: &mut B,
            config: &SingleLineConfig<BalanceOf<T>>,
            base_down: u8,
            outputs: &mut Vec<CommissionOutput<T::AccountId, B>>,
        ) where
            B: AtLeast32BitUnsigned + Copy,
        {
            if config.downline_rate == 0 { return; }

            let buyer_index = match SingleLineIndex::<T>::get(entity_id, buyer) {
                Some(idx) => idx,
                None => return,
            };

            let total_len = Self::single_line_length(entity_id);
            if buyer_index >= total_len.saturating_sub(1) { return; }

            let seg_size = T::MaxSingleLineLength::get();
            let buyer_stats = T::StatsProvider::get_member_stats(entity_id, buyer);
            let extra_levels = Self::calc_extra_levels(config.level_increment_threshold, buyer_stats.total_earned);
            let max_levels = base_down
                .saturating_add(extra_levels)
                .min(config.max_downline_levels) as u32;

            let mut cur_seg_id = buyer_index / seg_size;
            let mut cur_seg = SingleLineSegments::<T>::get(entity_id, cur_seg_id);

            for i in 1..=max_levels {
                // L1 审计修复: 使用 saturating_add 防止 buyer_index + i 溢出 u32
                let target_index = buyer_index.saturating_add(i);
                if target_index >= total_len { break; }
                let seg_id = target_index / seg_size;
                if seg_id != cur_seg_id {
                    cur_seg = SingleLineSegments::<T>::get(entity_id, seg_id);
                    cur_seg_id = seg_id;
                }
                let local_pos = (target_index % seg_size) as usize;
                let downline = match cur_seg.get(local_pos) {
                    Some(a) => a,
                    None => break,
                };

                // P0: 跳过被封禁或未激活会员（消耗 depth 但不发佣金）
                // M1-R5: 补充 is_member_active 检查（与 referral/multi-level 一致）
                if T::MemberProvider::is_banned(entity_id, downline)
                    || !T::MemberProvider::is_activated(entity_id, downline)
                    || !T::MemberProvider::is_member_active(entity_id, downline)
                {
                    continue;
                }

                // C2 审计修复: 佣金基于当前订单金额，而非受益人累计收益
                let commission = order_amount
                    .saturating_mul(B::from(config.downline_rate as u32))
                    / B::from(10000u32);
                let actual = commission.min(*remaining);

                if !actual.is_zero() {
                    *remaining = remaining.saturating_sub(actual);
                    outputs.push(CommissionOutput {
                        beneficiary: downline.clone(),
                        amount: actual,
                        commission_type: CommissionType::SingleLineDownline,
                        level: i as u8,
                    });
                }
            }
        }

        /// F7: 重置单链数据（清除所有段 + SingleLineIndex）
        pub(crate) fn do_reset_single_line(entity_id: u64) -> u32 {
            let seg_count = SingleLineSegmentCount::<T>::take(entity_id);
            let mut total_removed = 0u32;
            for seg_id in 0..seg_count {
                let seg = SingleLineSegments::<T>::take(entity_id, seg_id);
                for account in seg.iter() {
                    SingleLineIndex::<T>::remove(entity_id, account);
                }
                total_removed += seg.len() as u32;
            }
            total_removed
        }

        /// F12: 清除指定 entity 的所有等级层数覆盖
        pub(crate) fn do_clear_all_level_overrides(entity_id: u64) {
            let has_overrides = SingleLineCustomLevelOverrides::<T>::iter_prefix(entity_id).next().is_some();
            let _ = SingleLineCustomLevelOverrides::<T>::clear_prefix(entity_id, u32::MAX, None);
            if has_overrides {
                Self::deposit_event(Event::AllLevelOverridesCleared { entity_id });
            }
        }

        // ====================================================================
        // F4: 查询辅助函数
        // ====================================================================

        /// 获取单链长度（所有段的总长度）
        pub fn single_line_length(entity_id: u64) -> u32 {
            let seg_size = T::MaxSingleLineLength::get();
            let seg_count = SingleLineSegmentCount::<T>::get(entity_id);
            if seg_count == 0 {
                return 0;
            }
            let last_seg = SingleLineSegments::<T>::get(entity_id, seg_count - 1);
            (seg_count - 1) * seg_size + last_seg.len() as u32
        }

        /// 获取当前段剩余容量（段满后自动扩展，此值仅反映当前段的剩余空间）
        pub fn single_line_remaining_capacity(entity_id: u64) -> u32 {
            let seg_size = T::MaxSingleLineLength::get();
            let seg_count = SingleLineSegmentCount::<T>::get(entity_id);
            if seg_count == 0 {
                return seg_size;
            }
            let last_seg = SingleLineSegments::<T>::get(entity_id, seg_count - 1);
            seg_size.saturating_sub(last_seg.len() as u32)
        }

        /// 获取用户在单链中的位置
        pub fn user_position(entity_id: u64, account: &T::AccountId) -> Option<u32> {
            SingleLineIndex::<T>::get(entity_id, account)
        }

        /// 获取用户的有效层数（base + extra，capped by max）
        pub fn user_effective_levels(entity_id: u64, account: &T::AccountId) -> Option<(u8, u8)> {
            let config = SingleLineConfigs::<T>::get(entity_id)?;
            let (base_up, base_down) = Self::effective_base_levels(entity_id, account, &config);
            let stats = T::StatsProvider::get_member_stats(entity_id, account);
            let extra = Self::calc_extra_levels(config.level_increment_threshold, stats.total_earned);
            let eff_up = base_up.saturating_add(extra).min(config.max_upline_levels);
            let eff_down = base_down.saturating_add(extra).min(config.max_downline_levels);
            Some((eff_up, eff_down))
        }

        /// 检查单线收益是否启用
        pub fn is_single_line_enabled(entity_id: u64) -> bool {
            SingleLineEnabled::<T>::get(entity_id)
        }
    }
}

// ============================================================================
// L5-R3 审计修复: 统一的返佣计算逻辑（NEX 和 Token 共用）
// ============================================================================

impl<T: pallet::Config> pallet::Pallet<T> {
    /// 统一分发逻辑 — NEX `calculate` 和 Token `calculate_token` 均委托此函数。
    ///
    /// L5-R3: 消除原先 NEX/Token 两套独立的 process + calculate 实现（~170 行重复），
    /// 现通过泛型 `B: AtLeast32BitUnsigned` 统一处理所有余额类型。
    ///
    /// F1: 检查 Entity 活跃状态
    /// F2: 检查单线收益启用状态
    fn do_calculate<B>(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: B,
        remaining: B,
        enabled_modes: pallet_commission_common::CommissionModes,
        _is_first_order: bool,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, B>>, B)
    where
        B: sp_runtime::traits::AtLeast32BitUnsigned + Copy,
    {
        use pallet_commission_common::CommissionModes;

        // F1: 封禁/暂停的 Entity 不发佣金
        if !T::EntityProvider::is_entity_active(entity_id) {
            return (alloc::vec::Vec::new(), remaining);
        }

        // F2: 单线收益已暂停时不发佣金
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

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        let (base_up, base_down) = Self::effective_base_levels(entity_id, buyer, &config);

        if has_upline {
            Self::process_upline(entity_id, buyer, order_amount, &mut remaining, &config, base_up, &mut outputs);
        }

        if has_downline {
            Self::process_downline(entity_id, buyer, order_amount, &mut remaining, &config, base_down, &mut outputs);
        }

        // 未在链中的用户每次消费都尝试加入（自动重试，段满自动扩展）
        if !pallet::SingleLineIndex::<T>::contains_key(entity_id, buyer) {
            if Self::add_to_single_line(entity_id, buyer).is_err() {
                Self::deposit_event(pallet::Event::SingleLineJoinFailed {
                    entity_id,
                    account: buyer.clone(),
                });
            }
        }

        (outputs, remaining)
    }
}

// ============================================================================
// CommissionPlugin implementation (NEX)
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::CommissionPlugin<T::AccountId, pallet::BalanceOf<T>> for pallet::Pallet<T> {
    fn calculate(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: pallet::BalanceOf<T>,
        remaining: pallet::BalanceOf<T>,
        enabled_modes: pallet_commission_common::CommissionModes,
        is_first_order: bool,
        _buyer_order_count: u32,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, pallet::BalanceOf<T>>>, pallet::BalanceOf<T>) {
        Self::do_calculate(entity_id, buyer, order_amount, remaining, enabled_modes, is_first_order)
    }
}

// ============================================================================
// TokenCommissionPlugin implementation (多资产)
// ============================================================================

impl<T: pallet::Config, TB> pallet_commission_common::TokenCommissionPlugin<T::AccountId, TB>
    for pallet::Pallet<T>
where
    TB: sp_runtime::traits::AtLeast32BitUnsigned + Copy + Default + core::fmt::Debug,
{
    fn calculate_token(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: TB,
        remaining: TB,
        enabled_modes: pallet_commission_common::CommissionModes,
        is_first_order: bool,
        _buyer_order_count: u32,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>, TB) {
        Self::do_calculate(entity_id, buyer, order_amount, remaining, enabled_modes, is_first_order)
    }
}

// ============================================================================
// SingleLinePlanWriter implementation
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::SingleLinePlanWriter
    for pallet::Pallet<T>
{
    fn set_single_line_config(
        entity_id: u64,
        upline_rate: u16,
        downline_rate: u16,
        base_upline_levels: u8,
        base_downline_levels: u8,
        level_increment_threshold: u128,
        max_upline_levels: u8,
        max_downline_levels: u8,
    ) -> Result<(), sp_runtime::DispatchError> {
        frame_support::ensure!(
            upline_rate <= 1000 && downline_rate <= 1000,
            sp_runtime::DispatchError::Other("InvalidRate")
        );
        frame_support::ensure!(
            base_upline_levels <= max_upline_levels,
            sp_runtime::DispatchError::Other("BaseLevelsExceedMax")
        );
        frame_support::ensure!(
            base_downline_levels <= max_downline_levels,
            sp_runtime::DispatchError::Other("BaseLevelsExceedMax")
        );

        let threshold: pallet::BalanceOf<T> =
            sp_runtime::SaturatedConversion::saturated_into(level_increment_threshold);

        pallet::SingleLineConfigs::<T>::insert(
            entity_id,
            pallet::SingleLineConfig {
                upline_rate,
                downline_rate,
                base_upline_levels,
                base_downline_levels,
                level_increment_threshold: threshold,
                max_upline_levels,
                max_downline_levels,
            },
        );
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
        entity_id: u64,
        level_id: u8,
        upline_levels: u8,
        downline_levels: u8,
    ) -> Result<(), sp_runtime::DispatchError> {
        frame_support::ensure!(
            upline_levels > 0 || downline_levels > 0,
            sp_runtime::DispatchError::Other("InvalidLevels")
        );
        pallet::SingleLineCustomLevelOverrides::<T>::insert(
            entity_id,
            level_id,
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
