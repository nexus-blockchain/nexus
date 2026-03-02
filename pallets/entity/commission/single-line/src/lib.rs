//! # Commission Single-Line Plugin (pallet-commission-single-line)
//!
//! 单线收益插件：基于全局消费注册顺序的上下线收益。
//! - 上线收益 (SingleLineUpline)
//! - 下线收益 (SingleLineDownline)
//! - 层数随消费额动态增长

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

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
    };
    use sp_runtime::traits::{Saturating, Zero};

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

    #[pallet::pallet]
    pub struct Pallet<T>(_);

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

    /// 消费单链 entity_id -> Vec<AccountId>（按首次消费顺序）
    #[pallet::storage]
    #[pallet::getter(fn single_line)]
    pub type SingleLines<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        BoundedVec<T::AccountId, T::MaxSingleLineLength>,
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

    // ========================================================================
    // Events / Errors
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        SingleLineConfigUpdated { entity_id: u64 },
        AddedToSingleLine { entity_id: u64, account: T::AccountId, index: u32 },
        /// 单链加入失败（可能链已满，需人工干预）
        SingleLineJoinFailed { entity_id: u64, account: T::AccountId },
        /// 按等级自定义层数已更新
        LevelBasedLevelsUpdated { entity_id: u64, level_id: u8 },
        /// 按等级自定义层数已移除
        LevelBasedLevelsRemoved { entity_id: u64, level_id: u8 },
    }

    #[pallet::error]
    pub enum Error<T> {
        InvalidRate,
        SingleLineFull,
        InvalidLevels,
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 设置单线收益配置
        ///
        /// CSL-H1 审计修复: 参数统一为 entity_id，与插件查询键一致
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
            ensure_root(origin)?;
            ensure!(upline_rate <= 1000 && downline_rate <= 1000, Error::<T>::InvalidRate);

            SingleLineConfigs::<T>::insert(entity_id, SingleLineConfig {
                upline_rate,
                downline_rate,
                base_upline_levels,
                base_downline_levels,
                level_increment_threshold,
                max_upline_levels,
                max_downline_levels,
            });

            Self::deposit_event(Event::SingleLineConfigUpdated { entity_id });
            Ok(())
        }

        /// 设置按会员等级自定义的收益层数
        ///
        /// 当买家拥有对应等级时，使用此处的 upline_levels/downline_levels 替代
        /// SingleLineConfig 中的 base_upline_levels/base_downline_levels。
        ///
        /// level_id 为自定义等级 ID（对应 EntityLevelSystems）
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(30_000_000, 4_000))]
        pub fn set_level_based_levels(
            origin: OriginFor<T>,
            entity_id: u64,
            level_id: u8,
            upline_levels: u8,
            downline_levels: u8,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(upline_levels > 0 || downline_levels > 0, Error::<T>::InvalidLevels);

            let levels = LevelBasedLevels { upline_levels, downline_levels };
            SingleLineCustomLevelOverrides::<T>::insert(entity_id, level_id, levels);

            Self::deposit_event(Event::LevelBasedLevelsUpdated { entity_id, level_id });
            Ok(())
        }

        /// 移除指定等级的自定义层数配置（回退到 SingleLineConfig 基础值）
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
        pub fn remove_level_based_levels(
            origin: OriginFor<T>,
            entity_id: u64,
            level_id: u8,
        ) -> DispatchResult {
            ensure_root(origin)?;
            // M3 修复: 仅在存在时移除并发出事件，避免幽灵事件
            if SingleLineCustomLevelOverrides::<T>::contains_key(entity_id, level_id) {
                SingleLineCustomLevelOverrides::<T>::remove(entity_id, level_id);
                Self::deposit_event(Event::LevelBasedLevelsRemoved { entity_id, level_id });
            }
            Ok(())
        }
    }

    // ========================================================================
    // Internal functions
    // ========================================================================

    impl<T: Config> Pallet<T> {
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

        /// 将用户加入单链（首次消费时调用）
        pub fn add_to_single_line(entity_id: u64, account: &T::AccountId) -> DispatchResult {
            if SingleLineIndex::<T>::contains_key(entity_id, account) {
                return Ok(());
            }

            SingleLines::<T>::try_mutate(entity_id, |line| {
                let index = line.len() as u32;
                line.try_push(account.clone()).map_err(|_| Error::<T>::SingleLineFull)?;
                SingleLineIndex::<T>::insert(entity_id, account, index);
                Ok(())
            })
        }

        pub(crate) fn calc_extra_levels(threshold: BalanceOf<T>, total_earned: BalanceOf<T>) -> u8 {
            if threshold.is_zero() {
                return 0;
            }
            let threshold_u128: u128 = sp_runtime::SaturatedConversion::saturated_into(threshold);
            let earned_u128: u128 = sp_runtime::SaturatedConversion::saturated_into(total_earned);
            if threshold_u128 > 0 {
                // H4 审计修复: 防止 u8 溢出，限制最大值为 255
                (earned_u128 / threshold_u128).min(255) as u8
            } else {
                0
            }
        }

        pub fn process_upline(
            entity_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            remaining: &mut BalanceOf<T>,
            config: &SingleLineConfig<BalanceOf<T>>,
            base_up: u8,
            outputs: &mut Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>,
        ) {
            if config.upline_rate == 0 { return; }

            let buyer_index = match SingleLineIndex::<T>::get(entity_id, buyer) {
                Some(idx) => idx,
                None => return,
            };
            if buyer_index == 0 { return; }

            let line = SingleLines::<T>::get(entity_id);
            let buyer_stats = T::StatsProvider::get_member_stats(entity_id, buyer);
            let extra_levels = Self::calc_extra_levels(config.level_increment_threshold, buyer_stats.total_earned);
            let max_levels = base_up
                .saturating_add(extra_levels)
                .min(config.max_upline_levels) as u32;

            for i in 1..=max_levels {
                if buyer_index < i { break; }
                let upline_index = (buyer_index - i) as usize;
                if upline_index >= line.len() { break; }
                let upline = &line[upline_index];

                // C2 审计修复: 佣金基于当前订单金额，而非受益人累计收益
                let commission = order_amount
                    .saturating_mul(config.upline_rate.into())
                    / 10000u32.into();
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

        pub fn process_downline(
            entity_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            remaining: &mut BalanceOf<T>,
            config: &SingleLineConfig<BalanceOf<T>>,
            base_down: u8,
            outputs: &mut Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>,
        ) {
            if config.downline_rate == 0 { return; }

            let buyer_index = match SingleLineIndex::<T>::get(entity_id, buyer) {
                Some(idx) => idx,
                None => return,
            };

            let line = SingleLines::<T>::get(entity_id);
            let line_len = line.len() as u32;
            if buyer_index >= line_len.saturating_sub(1) { return; }

            let buyer_stats = T::StatsProvider::get_member_stats(entity_id, buyer);
            let extra_levels = Self::calc_extra_levels(config.level_increment_threshold, buyer_stats.total_earned);
            let max_levels = base_down
                .saturating_add(extra_levels)
                .min(config.max_downline_levels) as u32;

            for i in 1..=max_levels {
                let downline_index = (buyer_index + i) as usize;
                if downline_index >= line.len() { break; }
                let downline = &line[downline_index];

                // C2 审计修复: 佣金基于当前订单金额，而非受益人累计收益
                let commission = order_amount
                    .saturating_mul(config.downline_rate.into())
                    / 10000u32.into();
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
    }
}

// ============================================================================
// CommissionPlugin implementation
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
        use pallet_commission_common::CommissionModes;

        // 配置按 entity_id，单链按 entity_id（跨店共享单链）
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

        // M1 性能修复: 预计算等级基础层数，避免 upline+downline 各调用一次
        let (base_up, base_down) = pallet::Pallet::<T>::effective_base_levels(entity_id, buyer, &config);

        if has_upline {
            pallet::Pallet::<T>::process_upline(entity_id, buyer, order_amount, &mut remaining, &config, base_up, &mut outputs);
        }

        if has_downline {
            pallet::Pallet::<T>::process_downline(entity_id, buyer, order_amount, &mut remaining, &config, base_down, &mut outputs);
        }

        // 首次消费加入单链（Entity 级，失败发事件）
        if is_first_order {
            if pallet::Pallet::<T>::add_to_single_line(entity_id, buyer).is_err() {
                pallet::Pallet::<T>::deposit_event(pallet::Event::SingleLineJoinFailed {
                    entity_id,
                    account: buyer.clone(),
                });
            }
        }

        (outputs, remaining)
    }
}

// ============================================================================
// Token 多资产 — TokenCommissionPlugin implementation
// ============================================================================

impl<T: pallet::Config> pallet::Pallet<T> {
    /// Token 版上线收益（泛型，rate-based）
    ///
    /// extra_levels 仍基于 NEX 版 StatsProvider（级别扩展取决于 NEX 佣金历史）。
    fn process_upline_token<TB>(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: TB,
        remaining: &mut TB,
        config: &pallet::SingleLineConfig<pallet::BalanceOf<T>>,
        base_up: u8,
        outputs: &mut alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>,
    ) where
        TB: sp_runtime::traits::AtLeast32BitUnsigned + Copy,
    {
        if config.upline_rate == 0 { return; }

        let buyer_index = match pallet::SingleLineIndex::<T>::get(entity_id, buyer) {
            Some(idx) => idx,
            None => return,
        };
        if buyer_index == 0 { return; }

        let line = pallet::SingleLines::<T>::get(entity_id);
        let buyer_stats = T::StatsProvider::get_member_stats(entity_id, buyer);
        let extra_levels = Self::calc_extra_levels(config.level_increment_threshold, buyer_stats.total_earned);
        let max_levels = base_up
            .saturating_add(extra_levels)
            .min(config.max_upline_levels) as u32;

        for i in 1..=max_levels {
            if buyer_index < i { break; }
            let upline_index = (buyer_index - i) as usize;
            if upline_index >= line.len() { break; }
            let upline = &line[upline_index];

            let commission = order_amount
                .saturating_mul(TB::from(config.upline_rate as u32))
                / TB::from(10000u32);
            let actual = commission.min(*remaining);

            if !actual.is_zero() {
                *remaining = remaining.saturating_sub(actual);
                outputs.push(pallet_commission_common::CommissionOutput {
                    beneficiary: upline.clone(),
                    amount: actual,
                    commission_type: pallet_commission_common::CommissionType::SingleLineUpline,
                    level: i as u8,
                });
            }
        }
    }

    /// Token 版下线收益（泛型，rate-based）
    fn process_downline_token<TB>(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: TB,
        remaining: &mut TB,
        config: &pallet::SingleLineConfig<pallet::BalanceOf<T>>,
        base_down: u8,
        outputs: &mut alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>,
    ) where
        TB: sp_runtime::traits::AtLeast32BitUnsigned + Copy,
    {
        if config.downline_rate == 0 { return; }

        let buyer_index = match pallet::SingleLineIndex::<T>::get(entity_id, buyer) {
            Some(idx) => idx,
            None => return,
        };

        let line = pallet::SingleLines::<T>::get(entity_id);
        let line_len = line.len() as u32;
        if buyer_index >= line_len.saturating_sub(1) { return; }

        let buyer_stats = T::StatsProvider::get_member_stats(entity_id, buyer);
        let extra_levels = Self::calc_extra_levels(config.level_increment_threshold, buyer_stats.total_earned);
        let max_levels = base_down
            .saturating_add(extra_levels)
            .min(config.max_downline_levels) as u32;

        for i in 1..=max_levels {
            let downline_index = (buyer_index + i) as usize;
            if downline_index >= line.len() { break; }
            let downline = &line[downline_index];

            let commission = order_amount
                .saturating_mul(TB::from(config.downline_rate as u32))
                / TB::from(10000u32);
            let actual = commission.min(*remaining);

            if !actual.is_zero() {
                *remaining = remaining.saturating_sub(actual);
                outputs.push(pallet_commission_common::CommissionOutput {
                    beneficiary: downline.clone(),
                    amount: actual,
                    commission_type: pallet_commission_common::CommissionType::SingleLineDownline,
                    level: i as u8,
                });
            }
        }
    }
}

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
        use pallet_commission_common::CommissionModes;

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

        // M1 性能修复: 预计算等级基础层数，避免 upline+downline 各调用一次
        let (base_up, base_down) = pallet::Pallet::<T>::effective_base_levels(entity_id, buyer, &config);

        if has_upline {
            pallet::Pallet::<T>::process_upline_token::<TB>(
                entity_id, buyer, order_amount, &mut remaining, &config, base_up, &mut outputs,
            );
        }

        if has_downline {
            pallet::Pallet::<T>::process_downline_token::<TB>(
                entity_id, buyer, order_amount, &mut remaining, &config, base_down, &mut outputs,
            );
        }

        // 首次消费加入单链（与 NEX 版共享，仅触发一次）
        if is_first_order {
            if pallet::Pallet::<T>::add_to_single_line(entity_id, buyer).is_err() {
                pallet::Pallet::<T>::deposit_event(pallet::Event::SingleLineJoinFailed {
                    entity_id,
                    account: buyer.clone(),
                });
            }
        }

        (outputs, remaining)
    }
}
