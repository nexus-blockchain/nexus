//! # Commission Team Performance Plugin (pallet-commission-team)
//!
//! 团队业绩返佣插件：基于推荐链上级的团队累计销售额，按阶梯比例发放奖金。
//!
//! ## 核心逻辑
//!
//! 当买家下单时，沿推荐链向上遍历（最多 `max_depth` 层），
//! 对每个上级查询其团队统计（team_size, total_spent），
//! 匹配最高达标的阶梯档位，按该档位比例对当前订单金额计算奖金。
//!
//! ## 与其他模式的区别
//!
//! - `LEVEL_DIFF`：按等级差价，每笔订单只取差额
//! - `TEAM_PERFORMANCE`：按团队累计业绩阶梯，每笔订单按档位比例发放

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

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
        CommissionOutput, CommissionType, MemberProvider,
    };
    use sp_runtime::traits::{Saturating, Zero};

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // ========================================================================
    // Config structs
    // ========================================================================

    /// 团队业绩阶梯档位
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct TeamPerformanceTier<Balance> {
        /// 团队累计销售额门槛
        pub sales_threshold: Balance,
        /// 团队最小人数门槛（0 = 不限制）
        pub min_team_size: u32,
        /// 奖金比例（基点，500 = 5%）
        pub rate: u16,
    }

    /// 团队业绩门槛数据源模式
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum SalesThresholdMode {
        /// 使用 get_member_stats 返回的 total_spent（NEX Balance 转 u128）
        Nex = 0,
        /// 使用 get_member_spent_usdt 返回的 USDT 累计（精度 10^6）
        Usdt = 1,
    }

    impl Default for SalesThresholdMode {
        fn default() -> Self { Self::Nex }
    }

    /// 团队业绩返佣配置（per-entity）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxTiers))]
    pub struct TeamPerformanceConfig<Balance, MaxTiers: Get<u32>> {
        /// 阶梯档位列表（按 sales_threshold 升序排列）
        pub tiers: BoundedVec<TeamPerformanceTier<Balance>, MaxTiers>,
        /// 沿推荐链向上最大遍历深度
        pub max_depth: u8,
        /// 是否允许多层叠加（false = 仅最近一个达标上级获得奖金）
        pub allow_stacking: bool,
        /// 门槛数据源模式（Nex=使用 NEX 累计, Usdt=使用 MemberSpentUsdt）
        pub threshold_mode: SalesThresholdMode,
    }

    impl<Balance: Default, MaxTiers: Get<u32>> Default for TeamPerformanceConfig<Balance, MaxTiers> {
        fn default() -> Self {
            Self {
                tiers: BoundedVec::default(),
                max_depth: 5,
                allow_stacking: false,
                threshold_mode: SalesThresholdMode::Nex,
            }
        }
    }

    pub type TeamPerformanceConfigOf<T> =
        TeamPerformanceConfig<BalanceOf<T>, <T as Config>::MaxTeamTiers>;

    // ========================================================================
    // Pallet Config
    // ========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: Currency<Self::AccountId>;
        type MemberProvider: MemberProvider<Self::AccountId>;

        /// 最大阶梯档位数
        #[pallet::constant]
        type MaxTeamTiers: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ========================================================================
    // Storage
    // ========================================================================

    /// 团队业绩配置 entity_id -> TeamPerformanceConfig
    #[pallet::storage]
    #[pallet::getter(fn team_performance_config)]
    pub type TeamPerformanceConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        TeamPerformanceConfigOf<T>,
    >;

    // ========================================================================
    // Events / Errors
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        TeamPerformanceConfigUpdated { entity_id: u64 },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// 费率无效（超过 10000 基点）
        InvalidRate,
        /// 档位数为 0
        EmptyTiers,
        /// 遍历深度无效
        InvalidMaxDepth,
        /// 阶梯门槛未严格递增
        TiersNotAscending,
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 设置团队业绩返佣配置
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(45_000_000, 4_000))]
        pub fn set_team_performance_config(
            origin: OriginFor<T>,
            entity_id: u64,
            tiers: BoundedVec<TeamPerformanceTier<BalanceOf<T>>, T::MaxTeamTiers>,
            max_depth: u8,
            allow_stacking: bool,
            threshold_mode: SalesThresholdMode,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(!tiers.is_empty(), Error::<T>::EmptyTiers);
            ensure!(max_depth > 0 && max_depth <= 30, Error::<T>::InvalidMaxDepth);

            // 校验每个档位
            for tier in tiers.iter() {
                ensure!(tier.rate <= 10000, Error::<T>::InvalidRate);
            }

            // 校验阶梯门槛严格递增
            for window in tiers.windows(2) {
                ensure!(
                    window[1].sales_threshold > window[0].sales_threshold,
                    Error::<T>::TiersNotAscending
                );
            }

            TeamPerformanceConfigs::<T>::insert(entity_id, TeamPerformanceConfig {
                tiers,
                max_depth,
                allow_stacking,
                threshold_mode,
            });

            Self::deposit_event(Event::TeamPerformanceConfigUpdated { entity_id });
            Ok(())
        }
    }

    // ========================================================================
    // Internal calculation
    // ========================================================================

    impl<T: Config> Pallet<T> {
        /// 匹配最高达标的阶梯档位
        ///
        /// tiers 按 sales_threshold 升序排列，返回最后一个满足条件的档位 rate
        pub(crate) fn match_tier(
            tiers: &[TeamPerformanceTier<BalanceOf<T>>],
            team_size: u32,
            total_spent: u128,
        ) -> Option<u16> {
            let mut matched_rate: Option<u16> = None;

            for tier in tiers.iter() {
                let threshold_u128: u128 =
                    sp_runtime::SaturatedConversion::saturated_into(tier.sales_threshold);

                // TM-M1 审计修复: 仅在 sales_threshold 不满足时 break（阶梯升序保证）。
                // min_team_size 不要求单调递增，跳过但不 break，以免遗漏更高档位。
                if total_spent < threshold_u128 {
                    break;
                }
                if tier.min_team_size == 0 || team_size >= tier.min_team_size {
                    matched_rate = Some(tier.rate);
                }
            }

            matched_rate
        }

        /// 处理团队业绩返佣
        pub fn process_team_performance(
            entity_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            remaining: &mut BalanceOf<T>,
            config: &TeamPerformanceConfigOf<T>,
            outputs: &mut Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>,
        ) {
            if config.tiers.is_empty() { return; }

            let mut current = T::MemberProvider::get_referrer(entity_id, buyer);
            let mut depth: u8 = 0;

            while let Some(ref ancestor) = current {
                depth += 1;
                if depth > config.max_depth { break; }
                if remaining.is_zero() { break; }

                // 查询团队统计：(direct_referrals, team_size, total_spent)
                let (_direct, team_size, nex_spent) =
                    T::MemberProvider::get_member_stats(entity_id, ancestor);
                let total_spent = match config.threshold_mode {
                    SalesThresholdMode::Nex => nex_spent,
                    SalesThresholdMode::Usdt => {
                        T::MemberProvider::get_member_spent_usdt(entity_id, ancestor) as u128
                    }
                };

                if let Some(rate) = Self::match_tier(&config.tiers, team_size, total_spent) {
                    if rate > 0 {
                        let commission = order_amount
                            .saturating_mul(rate.into())
                            / 10000u32.into();
                        let actual = commission.min(*remaining);

                        if !actual.is_zero() {
                            *remaining = remaining.saturating_sub(actual);
                            outputs.push(CommissionOutput {
                                beneficiary: ancestor.clone(),
                                amount: actual,
                                commission_type: CommissionType::TeamPerformance,
                                level: depth,
                            });
                        }

                        // 非叠加模式：仅奖励最近一个达标上级
                        if !config.allow_stacking {
                            break;
                        }
                    }
                }

                current = T::MemberProvider::get_referrer(entity_id, ancestor);
            }
        }
    }
}

// ============================================================================
// CommissionPlugin implementation
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::CommissionPlugin<T::AccountId, pallet::BalanceOf<T>>
    for pallet::Pallet<T>
{
    fn calculate(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: pallet::BalanceOf<T>,
        remaining: pallet::BalanceOf<T>,
        enabled_modes: pallet_commission_common::CommissionModes,
        _is_first_order: bool,
        _buyer_order_count: u32,
    ) -> (
        alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, pallet::BalanceOf<T>>>,
        pallet::BalanceOf<T>,
    ) {
        use pallet_commission_common::CommissionModes;

        if !enabled_modes.contains(CommissionModes::TEAM_PERFORMANCE) {
            return (alloc::vec::Vec::new(), remaining);
        }

        let config = match pallet::TeamPerformanceConfigs::<T>::get(entity_id) {
            Some(c) => c,
            None => return (alloc::vec::Vec::new(), remaining),
        };

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        pallet::Pallet::<T>::process_team_performance(
            entity_id, buyer, order_amount, &mut remaining, &config, &mut outputs,
        );

        (outputs, remaining)
    }
}

// ============================================================================
// Token 多资产 — TokenCommissionPlugin implementation
// ============================================================================

use pallet_commission_common::MemberProvider as _MemberProviderToken;

impl<T: pallet::Config> pallet::Pallet<T> {
    /// Token 版团队业绩计算（泛型，rate-based）
    ///
    /// 阶梯匹配逻辑与 NEX 版完全一致（基于 MemberProvider 的 team_size / total_spent）。
    /// 仅佣金金额计算使用泛型 TB。
    fn process_team_performance_token<TB>(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: TB,
        remaining: &mut TB,
        config: &pallet::TeamPerformanceConfigOf<T>,
        outputs: &mut alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>,
    ) where
        TB: sp_runtime::traits::AtLeast32BitUnsigned + Copy,
    {
        if config.tiers.is_empty() { return; }

        let mut current = T::MemberProvider::get_referrer(entity_id, buyer);
        let mut depth: u8 = 0;

        while let Some(ref ancestor) = current {
            depth += 1;
            if depth > config.max_depth { break; }
            if remaining.is_zero() { break; }

            let (_direct, team_size, nex_spent) =
                T::MemberProvider::get_member_stats(entity_id, ancestor);
            let total_spent = match config.threshold_mode {
                pallet::SalesThresholdMode::Nex => nex_spent,
                pallet::SalesThresholdMode::Usdt => {
                    T::MemberProvider::get_member_spent_usdt(entity_id, ancestor) as u128
                }
            };

            if let Some(rate) = Self::match_tier(&config.tiers, team_size, total_spent) {
                if rate > 0 {
                    let commission = order_amount
                        .saturating_mul(TB::from(rate as u32))
                        / TB::from(10000u32);
                    let actual = commission.min(*remaining);

                    if !actual.is_zero() {
                        *remaining = remaining.saturating_sub(actual);
                        outputs.push(pallet_commission_common::CommissionOutput {
                            beneficiary: ancestor.clone(),
                            amount: actual,
                            commission_type: pallet_commission_common::CommissionType::TeamPerformance,
                            level: depth,
                        });
                    }

                    if !config.allow_stacking {
                        break;
                    }
                }
            }

            current = T::MemberProvider::get_referrer(entity_id, ancestor);
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
        _is_first_order: bool,
        _buyer_order_count: u32,
    ) -> (
        alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>,
        TB,
    ) {
        use pallet_commission_common::CommissionModes;

        if !enabled_modes.contains(CommissionModes::TEAM_PERFORMANCE) {
            return (alloc::vec::Vec::new(), remaining);
        }

        let config = match pallet::TeamPerformanceConfigs::<T>::get(entity_id) {
            Some(c) => c,
            None => return (alloc::vec::Vec::new(), remaining),
        };

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        pallet::Pallet::<T>::process_team_performance_token::<TB>(
            entity_id, buyer, order_amount, &mut remaining, &config, &mut outputs,
        );

        (outputs, remaining)
    }
}

// ============================================================================
// TeamPlanWriter implementation
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::TeamPlanWriter<pallet::BalanceOf<T>>
    for pallet::Pallet<T>
{
    fn set_team_config(
        entity_id: u64,
        tiers: alloc::vec::Vec<(u128, u32, u16)>,
        max_depth: u8,
        allow_stacking: bool,
        threshold_mode: u8,
    ) -> Result<(), sp_runtime::DispatchError> {
        // TM-M2 审计修复: PlanWriter 路径与 extrinsic 一致的参数校验
        frame_support::ensure!(!tiers.is_empty(), sp_runtime::DispatchError::Other("EmptyTiers"));
        frame_support::ensure!(
            max_depth > 0 && max_depth <= 30,
            sp_runtime::DispatchError::Other("InvalidMaxDepth")
        );
        for &(_, _, rate) in tiers.iter() {
            frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        }
        // 校验阶梯门槛严格递增
        for window in tiers.windows(2) {
            frame_support::ensure!(
                window[1].0 > window[0].0,
                sp_runtime::DispatchError::Other("TiersNotAscending")
            );
        }

        let bounded: frame_support::BoundedVec<
            pallet::TeamPerformanceTier<pallet::BalanceOf<T>>,
            T::MaxTeamTiers,
        > = tiers
            .into_iter()
            .map(|(threshold, min_team_size, rate)| pallet::TeamPerformanceTier {
                sales_threshold: sp_runtime::SaturatedConversion::saturated_into(threshold),
                min_team_size,
                rate,
            })
            .collect::<alloc::vec::Vec<_>>()
            .try_into()
            .map_err(|_| sp_runtime::DispatchError::Other("TooManyTiers"))?;

        let mode = if threshold_mode == 1 {
            pallet::SalesThresholdMode::Usdt
        } else {
            pallet::SalesThresholdMode::Nex
        };

        pallet::TeamPerformanceConfigs::<T>::insert(
            entity_id,
            pallet::TeamPerformanceConfig {
                tiers: bounded,
                max_depth,
                allow_stacking,
                threshold_mode: mode,
            },
        );
        Ok(())
    }

    fn clear_config(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        pallet::TeamPerformanceConfigs::<T>::remove(entity_id);
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use frame_support::{
        assert_ok, assert_noop,
        traits::ConstU32,
        derive_impl,
    };
    use pallet_commission_common::CommissionModes;
    use sp_runtime::BuildStorage;

    type Balance = u128;

    // -- Mock MemberProvider --
    use core::cell::RefCell;
    use alloc::collections::BTreeMap;

    thread_local! {
        static REFERRERS: RefCell<BTreeMap<(u64, u64), u64>> = RefCell::new(BTreeMap::new());
        static MEMBER_STATS: RefCell<BTreeMap<(u64, u64), (u32, u32, u128)>> = RefCell::new(BTreeMap::new());
        static MEMBER_SPENT_USDT: RefCell<BTreeMap<(u64, u64), u64>> = RefCell::new(BTreeMap::new());
    }

    pub struct MockMemberProvider;

    impl pallet_commission_common::MemberProvider<u64> for MockMemberProvider {
        fn is_member(_: u64, _: &u64) -> bool { true }
        fn get_referrer(entity_id: u64, account: &u64) -> Option<u64> {
            REFERRERS.with(|r| r.borrow().get(&(entity_id, *account)).copied())
        }
        fn get_member_stats(entity_id: u64, account: &u64) -> (u32, u32, u128) {
            MEMBER_STATS.with(|s| s.borrow().get(&(entity_id, *account)).copied().unwrap_or((0, 0, 0)))
        }
        fn uses_custom_levels(_: u64) -> bool { false }
        fn custom_level_id(_: u64, _: &u64) -> u8 { 0 }
        fn get_level_commission_bonus(_: u64, _: u8) -> u16 { 0 }
        fn auto_register(_: u64, _: &u64, _: Option<u64>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn set_custom_levels_enabled(_: u64, _: bool) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn set_upgrade_mode(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn add_custom_level(_: u64, _: u8, _: &[u8], _: u128, _: u16, _: u16) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn update_custom_level(_: u64, _: u8, _: Option<&[u8]>, _: Option<u128>, _: Option<u16>, _: Option<u16>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn remove_custom_level(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn custom_level_count(_: u64) -> u8 { 0 }
        fn get_member_spent_usdt(entity_id: u64, account: &u64) -> u64 {
            MEMBER_SPENT_USDT.with(|s| s.borrow().get(&(entity_id, *account)).copied().unwrap_or(0))
        }
    }

    // -- Mock Runtime --
    frame_support::construct_runtime!(
        pub enum Test {
            System: frame_system,
            Balances: pallet_balances,
            CommissionTeam: pallet,
        }
    );

    #[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
    impl frame_system::Config for Test {
        type Block = frame_system::mocking::MockBlock<Test>;
        type AccountData = pallet_balances::AccountData<Balance>;
    }

    #[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
    impl pallet_balances::Config for Test {
        type AccountStore = System;
        type Balance = Balance;
    }

    impl pallet::Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type Currency = Balances;
        type MemberProvider = MockMemberProvider;
        type MaxTeamTiers = ConstU32<10>;
    }

    fn new_test_ext() -> sp_io::TestExternalities {
        let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| {
            System::set_block_number(1);
        });
        ext
    }

    fn setup_chain(entity_id: u64) {
        // 推荐链: 10 → 20 → 30 → 40 → 50 (buyer)
        REFERRERS.with(|r| {
            let mut m = r.borrow_mut();
            m.insert((entity_id, 50), 40);
            m.insert((entity_id, 40), 30);
            m.insert((entity_id, 30), 20);
            m.insert((entity_id, 20), 10);
        });
    }

    fn set_stats(entity_id: u64, account: u64, direct: u32, team_size: u32, total_spent: u128) {
        MEMBER_STATS.with(|s| {
            s.borrow_mut().insert((entity_id, account), (direct, team_size, total_spent));
        });
    }

    fn set_spent_usdt(entity_id: u64, account: u64, usdt: u64) {
        MEMBER_SPENT_USDT.with(|s| {
            s.borrow_mut().insert((entity_id, account), usdt);
        });
    }

    fn clear_thread_locals() {
        REFERRERS.with(|r| r.borrow_mut().clear());
        MEMBER_STATS.with(|s| s.borrow_mut().clear());
        MEMBER_SPENT_USDT.with(|s| s.borrow_mut().clear());
    }

    // ====================================================================
    // Extrinsic tests
    // ====================================================================

    #[test]
    fn set_config_works() {
        new_test_ext().execute_with(|| {
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 5, rate: 100 },
                pallet::TeamPerformanceTier { sales_threshold: 5000, min_team_size: 20, rate: 300 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::root(),
                1,
                tiers.try_into().unwrap(),
                10,
                false,
                pallet::SalesThresholdMode::Nex,
            ));
            let config = pallet::TeamPerformanceConfigs::<Test>::get(1).unwrap();
            assert_eq!(config.tiers.len(), 2);
            assert_eq!(config.max_depth, 10);
            assert!(!config.allow_stacking);
            assert_eq!(config.threshold_mode, pallet::SalesThresholdMode::Nex);
        });
    }

    #[test]
    fn set_config_rejects_empty_tiers() {
        new_test_ext().execute_with(|| {
            let tiers: Vec<pallet::TeamPerformanceTier<Balance>> = vec![];
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
                ),
                Error::<Test>::EmptyTiers
            );
        });
    }

    #[test]
    fn set_config_rejects_invalid_rate() {
        new_test_ext().execute_with(|| {
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 10001 },
            ];
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
                ),
                Error::<Test>::InvalidRate
            );
        });
    }

    #[test]
    fn set_config_rejects_invalid_depth_zero() {
        new_test_ext().execute_with(|| {
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 0, false, pallet::SalesThresholdMode::Nex,
                ),
                Error::<Test>::InvalidMaxDepth
            );
        });
    }

    #[test]
    fn set_config_rejects_invalid_depth_over_30() {
        new_test_ext().execute_with(|| {
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 31, false, pallet::SalesThresholdMode::Nex,
                ),
                Error::<Test>::InvalidMaxDepth
            );
        });
    }

    #[test]
    fn set_config_rejects_non_ascending_thresholds() {
        new_test_ext().execute_with(|| {
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 5000, min_team_size: 0, rate: 300 },
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
                ),
                Error::<Test>::TiersNotAscending
            );
        });
    }

    #[test]
    fn set_config_requires_root() {
        new_test_ext().execute_with(|| {
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::signed(1), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
                ),
                sp_runtime::DispatchError::BadOrigin
            );
        });
    }

    // ====================================================================
    // CommissionPlugin calculation tests
    // ====================================================================

    #[test]
    fn no_config_returns_empty() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );
            assert!(outputs.is_empty());
            assert_eq!(remaining, 10000);
        });
    }

    #[test]
    fn mode_not_enabled_returns_empty() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            // 配置存在但模式位未启用
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 100, min_team_size: 0, rate: 500 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::DIRECT_REWARD); // 不含 TEAM_PERFORMANCE
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );
            assert!(outputs.is_empty());
            assert_eq!(remaining, 10000);
        });
    }

    #[test]
    fn single_tier_non_stacking() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            // account 40: team_size=10, total_spent=5000
            set_stats(1, 40, 3, 10, 5000);
            // account 30: team_size=50, total_spent=20000
            set_stats(1, 30, 8, 50, 20000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 3000, min_team_size: 5, rate: 200 },
                pallet::TeamPerformanceTier { sales_threshold: 10000, min_team_size: 20, rate: 500 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            // buyer=50, order=10000
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // non-stacking: 最近达标上级 = account 40 (tier1: 3000/5 → rate 200)
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].beneficiary, 40);
            // 10000 * 200 / 10000 = 200
            assert_eq!(outputs[0].amount, 200);
            assert_eq!(outputs[0].level, 1);
            assert_eq!(remaining, 9800);
        });
    }

    #[test]
    fn stacking_mode_rewards_multiple() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            set_stats(1, 40, 3, 10, 5000);  // 达标 tier1
            set_stats(1, 30, 8, 50, 20000); // 达标 tier2

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 3000, min_team_size: 5, rate: 200 },
                pallet::TeamPerformanceTier { sales_threshold: 10000, min_team_size: 20, rate: 500 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, true, pallet::SalesThresholdMode::Nex, // allow_stacking
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // stacking: account 40 gets tier1 rate=200, account 30 gets tier2 rate=500
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 200); // 10000 * 200 / 10000
            assert_eq!(outputs[1].beneficiary, 30);
            assert_eq!(outputs[1].amount, 500); // 10000 * 500 / 10000 (but capped by remaining=9800, 500 < 9800)
            assert_eq!(remaining, 10000 - 200 - 500);
        });
    }

    #[test]
    fn team_size_threshold_filters() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            // account 40: 足够的销售额但团队太小
            set_stats(1, 40, 1, 3, 5000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 3000, min_team_size: 5, rate: 200 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // account 40 团队人数 3 < 5，不达标
            assert!(outputs.is_empty());
            assert_eq!(remaining, 10000);
        });
    }

    #[test]
    fn remaining_caps_commission() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            set_stats(1, 40, 5, 20, 10000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 5000 }, // 50%
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            // remaining 仅 100
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 100, modes, false, 0,
            );

            // 计算 10000*5000/10000=5000 但 capped by remaining=100
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].amount, 100);
            assert_eq!(remaining, 0);
        });
    }

    #[test]
    fn max_depth_limits_traversal() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            // 只有 account 10 达标（深度4）
            set_stats(1, 10, 10, 100, 50000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 300 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 2, false, pallet::SalesThresholdMode::Nex, // max_depth=2
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // max_depth=2 只遍历到 account 30 (深度2)，account 10 在深度4，被截断
            assert!(outputs.is_empty());
            assert_eq!(remaining, 10000);
        });
    }

    #[test]
    fn plan_writer_works() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::TeamPlanWriter;
            assert_ok!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                1,
                vec![(1000, 5, 200), (5000, 20, 500)],
                8,
                true,
                0, // Nex mode
            ));
            let config = pallet::TeamPerformanceConfigs::<Test>::get(1).unwrap();
            assert_eq!(config.tiers.len(), 2);
            assert_eq!(config.tiers[0].rate, 200);
            assert_eq!(config.tiers[1].sales_threshold, 5000);
            assert_eq!(config.max_depth, 8);
            assert!(config.allow_stacking);
            assert_eq!(config.threshold_mode, pallet::SalesThresholdMode::Nex);

            // Usdt mode via PlanWriter
            assert_ok!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                2,
                vec![(50_000_000, 5, 300)],
                5,
                false,
                1, // Usdt mode
            ));
            let config2 = pallet::TeamPerformanceConfigs::<Test>::get(2).unwrap();
            assert_eq!(config2.threshold_mode, pallet::SalesThresholdMode::Usdt);

            assert_ok!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::clear_config(1));
            assert!(pallet::TeamPerformanceConfigs::<Test>::get(1).is_none());
        });
    }

    #[test]
    fn usdt_mode_uses_member_spent_usdt() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            // account 40: NEX spent=100 (low), but USDT spent=5_000_000 (5 USDT, 10^6 precision)
            set_stats(1, 40, 3, 10, 100);
            set_spent_usdt(1, 40, 5_000_000);

            // Threshold in USDT: 3_000_000 = 3 USDT
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 3_000_000, min_team_size: 5, rate: 200 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Usdt,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // USDT mode: 5_000_000 >= 3_000_000 threshold, team_size 10 >= 5 → matched
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 200); // 10000 * 200 / 10000
            assert_eq!(remaining, 9800);
        });
    }

    #[test]
    fn usdt_mode_nex_spent_ignored() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            // account 40: high NEX spent but low USDT spent
            set_stats(1, 40, 3, 10, 999_999_999);
            set_spent_usdt(1, 40, 1_000_000); // only 1 USDT

            // Threshold 2 USDT
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 2_000_000, min_team_size: 0, rate: 500 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Usdt,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, _) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // USDT 1_000_000 < 2_000_000 threshold → no match despite huge NEX spent
            assert!(outputs.is_empty());
        });
    }

    // ====================================================================
    // TM-M1: match_tier non-monotonic min_team_size
    // ====================================================================

    #[test]
    fn tm_m1_non_monotonic_team_size_matches_higher_tier() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            // account 40: high spent, moderate team_size
            // Fails tier0 (min_team_size=50) but matches tier1 (min_team_size=5)
            set_stats(1, 40, 3, 10, 20000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 50, rate: 100 },
                pallet::TeamPerformanceTier { sales_threshold: 5000, min_team_size: 5, rate: 300 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // Before fix: would break at tier0 (team_size fail), never check tier1
            // After fix: skips tier0, matches tier1 (rate=300)
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 300); // 10000 * 300 / 10000
            assert_eq!(remaining, 9700);
        });
    }

    // ====================================================================
    // TM-M2: PlanWriter validation
    // ====================================================================

    #[test]
    fn tm_m2_plan_writer_rejects_empty_tiers() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::TeamPlanWriter;
            assert!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                1, vec![], 5, false, 0,
            ).is_err());
        });
    }

    #[test]
    fn tm_m2_plan_writer_rejects_invalid_rate() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::TeamPlanWriter;
            assert!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                1, vec![(1000, 5, 10001)], 5, false, 0,
            ).is_err());
        });
    }

    #[test]
    fn tm_m2_plan_writer_rejects_invalid_depth() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::TeamPlanWriter;
            // depth=0
            assert!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                1, vec![(1000, 5, 200)], 0, false, 0,
            ).is_err());
            // depth=31
            assert!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                1, vec![(1000, 5, 200)], 31, false, 0,
            ).is_err());
        });
    }

    #[test]
    fn tm_m2_plan_writer_rejects_non_ascending_thresholds() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::TeamPlanWriter;
            assert!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                1, vec![(5000, 0, 200), (1000, 0, 100)], 5, false, 0,
            ).is_err());
        });
    }
}
