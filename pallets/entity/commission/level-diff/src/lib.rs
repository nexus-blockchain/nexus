//! # Commission Level-Diff Plugin (pallet-commission-level-diff)
//!
//! 等级极差返佣插件，支持：
//! - 全局等级体系（Normal/Silver/Gold/Platinum/Diamond）
//! - 自定义等级体系（shop 自定义等级 + 返佣率）

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
        CommissionModes, CommissionOutput, CommissionType, MemberProvider,
    };
    use pallet_entity_common::MemberLevel;
    use sp_runtime::traits::{Saturating, Zero};

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // ========================================================================
    // Config structs
    // ========================================================================

    /// 全局等级差价配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct LevelDiffConfig {
        pub normal_rate: u16,
        pub silver_rate: u16,
        pub gold_rate: u16,
        pub platinum_rate: u16,
        pub diamond_rate: u16,
    }

    impl LevelDiffConfig {
        pub fn rate_for_level(&self, level: MemberLevel) -> u16 {
            match level {
                MemberLevel::Normal => self.normal_rate,
                MemberLevel::Silver => self.silver_rate,
                MemberLevel::Gold => self.gold_rate,
                MemberLevel::Platinum => self.platinum_rate,
                MemberLevel::Diamond => self.diamond_rate,
            }
        }
    }

    /// 自定义等级极差配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxLevels))]
    pub struct CustomLevelDiffConfig<MaxLevels: Get<u32>> {
        pub level_rates: BoundedVec<u16, MaxLevels>,
        pub max_depth: u8,
    }

    impl<MaxLevels: Get<u32>> Default for CustomLevelDiffConfig<MaxLevels> {
        fn default() -> Self {
            Self {
                level_rates: BoundedVec::default(),
                max_depth: 10,
            }
        }
    }

    pub type CustomLevelDiffConfigOf<T> = CustomLevelDiffConfig<<T as Config>::MaxCustomLevels>;

    // ========================================================================
    // Pallet Config
    // ========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: Currency<Self::AccountId>;
        type MemberProvider: MemberProvider<Self::AccountId>;

        #[pallet::constant]
        type MaxCustomLevels: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ========================================================================
    // Storage
    // ========================================================================

    /// 全局等级差价配置 entity_id -> LevelDiffConfig
    #[pallet::storage]
    #[pallet::getter(fn level_diff_config)]
    pub type LevelDiffConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        LevelDiffConfig,
    >;

    /// 自定义等级极差配置 entity_id -> CustomLevelDiffConfig
    #[pallet::storage]
    #[pallet::getter(fn custom_level_diff_config)]
    pub type CustomLevelDiffConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        CustomLevelDiffConfigOf<T>,
    >;

    // ========================================================================
    // Events / Errors
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        LevelDiffConfigUpdated { entity_id: u64 },
        CustomLevelDiffConfigUpdated { entity_id: u64 },
    }

    #[pallet::error]
    pub enum Error<T> {
        InvalidRate,
        InvalidMaxDepth,
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 设置全局等级差价配置
        ///
        /// CLD-H1 审计修复: 参数统一为 entity_id，与插件查询键一致
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_level_diff_config(
            origin: OriginFor<T>,
            entity_id: u64,
            normal_rate: u16,
            silver_rate: u16,
            gold_rate: u16,
            platinum_rate: u16,
            diamond_rate: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(normal_rate <= 10000, Error::<T>::InvalidRate);
            ensure!(silver_rate <= 10000, Error::<T>::InvalidRate);
            ensure!(gold_rate <= 10000, Error::<T>::InvalidRate);
            ensure!(platinum_rate <= 10000, Error::<T>::InvalidRate);
            ensure!(diamond_rate <= 10000, Error::<T>::InvalidRate);

            LevelDiffConfigs::<T>::insert(entity_id, LevelDiffConfig {
                normal_rate,
                silver_rate,
                gold_rate,
                platinum_rate,
                diamond_rate,
            });

            Self::deposit_event(Event::LevelDiffConfigUpdated { entity_id });
            Ok(())
        }

        /// 设置自定义等级极差配置
        ///
        /// CLD-H1 审计修复: 参数统一为 entity_id
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(45_000_000, 4_000))]
        pub fn set_custom_level_diff_config(
            origin: OriginFor<T>,
            entity_id: u64,
            level_rates: BoundedVec<u16, T::MaxCustomLevels>,
            max_depth: u8,
        ) -> DispatchResult {
            ensure_root(origin)?;

            for rate in level_rates.iter() {
                ensure!(*rate <= 10000, Error::<T>::InvalidRate);
            }
            ensure!(max_depth > 0 && max_depth <= 20, Error::<T>::InvalidMaxDepth);

            CustomLevelDiffConfigs::<T>::insert(entity_id, CustomLevelDiffConfig {
                level_rates,
                max_depth,
            });

            Self::deposit_event(Event::CustomLevelDiffConfigUpdated { entity_id });
            Ok(())
        }
    }

    // ========================================================================
    // Internal calculation
    // ========================================================================

    impl<T: Config> Pallet<T> {
        pub fn process_level_diff(
            entity_id: u64,
            shop_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            remaining: &mut BalanceOf<T>,
            outputs: &mut Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>,
        ) {
            let global_config = LevelDiffConfigs::<T>::get(entity_id);
            let uses_custom = T::MemberProvider::uses_custom_levels(shop_id);
            let custom_config = CustomLevelDiffConfigs::<T>::get(entity_id);

            let max_depth = if uses_custom {
                custom_config.as_ref().map(|c| c.max_depth).unwrap_or(10)
            } else {
                10
            };

            let mut current_referrer = T::MemberProvider::get_referrer(shop_id, buyer);
            let mut prev_rate: u16 = 0;
            let mut level: u8 = 0;

            while let Some(ref referrer) = current_referrer {
                level += 1;
                if level > max_depth { break; }
                // M2 审计修复: 额度耗尽后提前退出，避免无意义的 storage read
                if remaining.is_zero() { break; }

                let referrer_rate = if uses_custom {
                    let level_id = T::MemberProvider::custom_level_id(shop_id, referrer);
                    // P1/P2 修复: CustomLevelDiffConfig 优先；无配置或 level_id 越界时
                    // 回退到 CustomLevel.commission_bonus（通过 MemberProvider）
                    custom_config.as_ref()
                        .and_then(|c| c.level_rates.get(level_id as usize).copied())
                        .unwrap_or_else(|| T::MemberProvider::get_level_commission_bonus(shop_id, level_id))
                } else {
                    let referrer_level = T::MemberProvider::member_level(shop_id, referrer)
                        .unwrap_or(MemberLevel::Normal);
                    global_config.as_ref()
                        .map(|c| c.rate_for_level(referrer_level))
                        .unwrap_or(0)
                };

                if referrer_rate > prev_rate {
                    let diff_rate = referrer_rate - prev_rate;
                    let commission = order_amount
                        .saturating_mul(diff_rate.into())
                        / 10000u32.into();
                    let actual = commission.min(*remaining);

                    if !actual.is_zero() {
                        *remaining = remaining.saturating_sub(actual);
                        outputs.push(CommissionOutput {
                            beneficiary: referrer.clone(),
                            amount: actual,
                            commission_type: CommissionType::LevelDiff,
                            level,
                        });
                    }

                    prev_rate = referrer_rate;
                }

                current_referrer = T::MemberProvider::get_referrer(shop_id, referrer);
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
        shop_id: u64,
        buyer: &T::AccountId,
        order_amount: pallet::BalanceOf<T>,
        remaining: pallet::BalanceOf<T>,
        enabled_modes: pallet_commission_common::CommissionModes,
        _is_first_order: bool,
        _buyer_order_count: u32,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, pallet::BalanceOf<T>>>, pallet::BalanceOf<T>) {
        use pallet_commission_common::CommissionModes;

        if !enabled_modes.contains(CommissionModes::LEVEL_DIFF) {
            return (alloc::vec::Vec::new(), remaining);
        }

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        // entity_id for config lookup, shop_id for MemberProvider
        pallet::Pallet::<T>::process_level_diff(
            entity_id, shop_id, buyer, order_amount, &mut remaining, &mut outputs,
        );

        (outputs, remaining)
    }
}

// ============================================================================
// LevelDiffPlanWriter implementation
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::LevelDiffPlanWriter for pallet::Pallet<T> {
    fn set_global_rates(entity_id: u64, normal: u16, silver: u16, gold: u16, platinum: u16, diamond: u16) -> Result<(), sp_runtime::DispatchError> {
        pallet::LevelDiffConfigs::<T>::insert(entity_id, pallet::LevelDiffConfig {
            normal_rate: normal,
            silver_rate: silver,
            gold_rate: gold,
            platinum_rate: platinum,
            diamond_rate: diamond,
        });
        Ok(())
    }

    fn clear_config(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        pallet::LevelDiffConfigs::<T>::remove(entity_id);
        pallet::CustomLevelDiffConfigs::<T>::remove(entity_id);
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use frame_support::{derive_impl, parameter_types, assert_ok};
    use sp_runtime::BuildStorage;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use pallet_commission_common::CommissionModes;

    type Balance = u128;

    // ---- Thread-local mock state ----
    thread_local! {
        static REFERRERS: RefCell<BTreeMap<(u64, u64), u64>> = RefCell::new(BTreeMap::new());
        static CUSTOM_LEVELS: RefCell<bool> = RefCell::new(false);
        static CUSTOM_LEVEL_IDS: RefCell<BTreeMap<(u64, u64), u8>> = RefCell::new(BTreeMap::new());
        static LEVEL_BONUSES: RefCell<BTreeMap<(u64, u8), u16>> = RefCell::new(BTreeMap::new());
        static MEMBER_LEVELS: RefCell<BTreeMap<(u64, u64), pallet_entity_common::MemberLevel>> = RefCell::new(BTreeMap::new());
    }

    fn clear_mocks() {
        REFERRERS.with(|r| r.borrow_mut().clear());
        CUSTOM_LEVELS.with(|c| *c.borrow_mut() = false);
        CUSTOM_LEVEL_IDS.with(|c| c.borrow_mut().clear());
        LEVEL_BONUSES.with(|l| l.borrow_mut().clear());
        MEMBER_LEVELS.with(|m| m.borrow_mut().clear());
    }

    pub struct MockMemberProvider;

    impl pallet_commission_common::MemberProvider<u64> for MockMemberProvider {
        fn is_member(_: u64, _: &u64) -> bool { true }
        fn get_referrer(shop_id: u64, account: &u64) -> Option<u64> {
            REFERRERS.with(|r| r.borrow().get(&(shop_id, *account)).copied())
        }
        fn member_level(shop_id: u64, account: &u64) -> Option<pallet_entity_common::MemberLevel> {
            MEMBER_LEVELS.with(|m| m.borrow().get(&(shop_id, *account)).copied())
        }
        fn get_member_stats(_: u64, _: &u64) -> (u32, u32, u128) { (0, 0, 0) }
        fn uses_custom_levels(_shop_id: u64) -> bool {
            CUSTOM_LEVELS.with(|c| *c.borrow())
        }
        fn custom_level_id(shop_id: u64, account: &u64) -> u8 {
            CUSTOM_LEVEL_IDS.with(|c| c.borrow().get(&(shop_id, *account)).copied().unwrap_or(0))
        }
        fn get_level_commission_bonus(shop_id: u64, level_id: u8) -> u16 {
            LEVEL_BONUSES.with(|l| l.borrow().get(&(shop_id, level_id)).copied().unwrap_or(0))
        }
        fn auto_register(_: u64, _: &u64, _: Option<u64>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn set_custom_levels_enabled(_: u64, _: bool) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn set_upgrade_mode(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn add_custom_level(_: u64, _: u8, _: &[u8], _: u128, _: u16, _: u16) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn update_custom_level(_: u64, _: u8, _: Option<&[u8]>, _: Option<u128>, _: Option<u16>, _: Option<u16>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn remove_custom_level(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn custom_level_count(_: u64) -> u8 { 0 }
    }

    // ---- Mock Runtime ----
    frame_support::construct_runtime!(
        pub enum Test {
            System: frame_system,
            Balances: pallet_balances,
            CommissionLevelDiff: pallet,
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

    parameter_types! {
        pub const MaxCustomLevels: u32 = 10;
    }

    impl pallet::Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type Currency = Balances;
        type MemberProvider = MockMemberProvider;
        type MaxCustomLevels = MaxCustomLevels;
    }

    fn new_test_ext() -> sp_io::TestExternalities {
        clear_mocks();
        let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| System::set_block_number(1));
        ext
    }

    // Helper: setup chain buyer(50) → 40 → 30 → 20 → 10
    fn setup_chain(shop_id: u64) {
        REFERRERS.with(|r| {
            let mut m = r.borrow_mut();
            m.insert((shop_id, 50), 40);
            m.insert((shop_id, 40), 30);
            m.insert((shop_id, 30), 20);
            m.insert((shop_id, 20), 10);
        });
    }

    // ========================================================================
    // P1: commission_bonus 回退测试
    // ========================================================================

    #[test]
    fn p1_fallback_to_commission_bonus_when_no_custom_config() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            let shop_id = 1u64;

            // 推荐链: 50 → 40 → 30
            setup_chain(shop_id);

            // 启用自定义等级
            CUSTOM_LEVELS.with(|c| *c.borrow_mut() = true);

            // 设置等级: 40=level_0, 30=level_1
            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((shop_id, 40), 0);
                m.insert((shop_id, 30), 1);
            });

            // 设置 commission_bonus（来自 CustomLevel 定义）
            LEVEL_BONUSES.with(|l| {
                let mut m = l.borrow_mut();
                m.insert((shop_id, 0), 300);  // 3%
                m.insert((shop_id, 1), 600);  // 6%
            });

            // 不设置 CustomLevelDiffConfig — 应回退到 commission_bonus
            let mut remaining: Balance = 10000;
            let mut outputs = alloc::vec::Vec::new();

            pallet::Pallet::<Test>::process_level_diff(
                entity_id, shop_id, &50, 10000, &mut remaining, &mut outputs,
            );

            // 40: rate=300, prev=0, diff=300 → 10000×300/10000 = 300
            // 30: rate=600, prev=300, diff=300 → 10000×300/10000 = 300
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 300);
            assert_eq!(outputs[1].beneficiary, 30);
            assert_eq!(outputs[1].amount, 300);
            assert_eq!(remaining, 10000 - 600);
        });
    }

    #[test]
    fn p1_custom_config_takes_priority_over_commission_bonus() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            let shop_id = 1u64;

            setup_chain(shop_id);
            CUSTOM_LEVELS.with(|c| *c.borrow_mut() = true);

            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((shop_id, 40), 0);
                m.insert((shop_id, 30), 1);
            });

            // commission_bonus = 100, 200
            LEVEL_BONUSES.with(|l| {
                let mut m = l.borrow_mut();
                m.insert((shop_id, 0), 100);
                m.insert((shop_id, 1), 200);
            });

            // CustomLevelDiffConfig 配置 = 500, 800 → 应优先使用
            let level_rates = frame_support::BoundedVec::try_from(vec![500u16, 800]).unwrap();
            assert_ok!(pallet::Pallet::<Test>::set_custom_level_diff_config(
                frame_system::RawOrigin::Root.into(),
                entity_id,
                level_rates,
                10,
            ));

            let mut remaining: Balance = 10000;
            let mut outputs = alloc::vec::Vec::new();

            pallet::Pallet::<Test>::process_level_diff(
                entity_id, shop_id, &50, 10000, &mut remaining, &mut outputs,
            );

            // 40: rate=500, prev=0, diff=500 → 500
            // 30: rate=800, prev=500, diff=300 → 300
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].amount, 500);
            assert_eq!(outputs[1].amount, 300);
        });
    }

    // ========================================================================
    // P2: 等级数量不匹配测试
    // ========================================================================

    #[test]
    fn p2_level_id_out_of_bounds_falls_back_to_commission_bonus() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            let shop_id = 1u64;

            setup_chain(shop_id);
            CUSTOM_LEVELS.with(|c| *c.borrow_mut() = true);

            // 3 个自定义等级（id=0,1,2），但 CustomLevelDiffConfig 只有 2 条 rate
            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((shop_id, 40), 0);
                m.insert((shop_id, 30), 1);
                m.insert((shop_id, 20), 2);  // 越界！level_rates 只有 [0] 和 [1]
            });

            // commission_bonus 回退
            LEVEL_BONUSES.with(|l| {
                let mut m = l.borrow_mut();
                m.insert((shop_id, 2), 900);  // level_id=2 的回退
            });

            // level_rates 只有 2 个元素: [300, 600]
            let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
            assert_ok!(pallet::Pallet::<Test>::set_custom_level_diff_config(
                frame_system::RawOrigin::Root.into(),
                entity_id,
                level_rates,
                10,
            ));

            let mut remaining: Balance = 10000;
            let mut outputs = alloc::vec::Vec::new();

            pallet::Pallet::<Test>::process_level_diff(
                entity_id, shop_id, &50, 10000, &mut remaining, &mut outputs,
            );

            // 40: rate=300 (from level_rates[0]), prev=0, diff=300 → 300
            // 30: rate=600 (from level_rates[1]), prev=300, diff=300 → 300
            // 20: rate=900 (fallback commission_bonus for level_id=2), prev=600, diff=300 → 300
            assert_eq!(outputs.len(), 3);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 300);
            assert_eq!(outputs[1].beneficiary, 30);
            assert_eq!(outputs[1].amount, 300);
            assert_eq!(outputs[2].beneficiary, 20);
            assert_eq!(outputs[2].amount, 300);
        });
    }

    #[test]
    fn p2_level_id_out_of_bounds_no_bonus_yields_zero() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            let shop_id = 1u64;

            // 50 → 40
            REFERRERS.with(|r| r.borrow_mut().insert((shop_id, 50), 40));
            CUSTOM_LEVELS.with(|c| *c.borrow_mut() = true);

            // level_id=2 超出 level_rates（空配置）
            CUSTOM_LEVEL_IDS.with(|c| c.borrow_mut().insert((shop_id, 40), 2));
            // 不设置 commission_bonus → 回退为 0

            let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
            assert_ok!(pallet::Pallet::<Test>::set_custom_level_diff_config(
                frame_system::RawOrigin::Root.into(),
                entity_id,
                level_rates,
                10,
            ));

            let mut remaining: Balance = 10000;
            let mut outputs = alloc::vec::Vec::new();

            pallet::Pallet::<Test>::process_level_diff(
                entity_id, shop_id, &50, 10000, &mut remaining, &mut outputs,
            );

            // rate=0 (fallback, no bonus), prev=0 → no diff → no commission
            assert_eq!(outputs.len(), 0);
            assert_eq!(remaining, 10000);
        });
    }

    // ========================================================================
    // CommissionPlugin trait 测试
    // ========================================================================

    #[test]
    fn plugin_skips_when_level_diff_mode_not_enabled() {
        new_test_ext().execute_with(|| {
            let modes = CommissionModes(CommissionModes::DIRECT_REWARD);
            let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::CommissionPlugin<u64, Balance>>::calculate(
                1, 1, &50, 10000, 10000, modes, false, 1,
            );
            assert!(outputs.is_empty());
            assert_eq!(remaining, 10000);
        });
    }

    #[test]
    fn plugin_works_with_level_diff_mode_enabled() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            let shop_id = 1u64;

            REFERRERS.with(|r| r.borrow_mut().insert((shop_id, 50), 40));
            CUSTOM_LEVELS.with(|c| *c.borrow_mut() = true);
            CUSTOM_LEVEL_IDS.with(|c| c.borrow_mut().insert((shop_id, 40), 0));
            LEVEL_BONUSES.with(|l| l.borrow_mut().insert((shop_id, 0), 500));

            let modes = CommissionModes(CommissionModes::LEVEL_DIFF);
            let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::CommissionPlugin<u64, Balance>>::calculate(
                entity_id, shop_id, &50, 10000, 10000, modes, false, 1,
            );

            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].amount, 500);
            assert_eq!(remaining, 9500);
        });
    }

    // ========================================================================
    // Extrinsic 校验测试
    // ========================================================================

    #[test]
    fn set_custom_config_validates_rates() {
        new_test_ext().execute_with(|| {
            let rates = frame_support::BoundedVec::try_from(vec![10001u16]).unwrap();
            assert!(pallet::Pallet::<Test>::set_custom_level_diff_config(
                frame_system::RawOrigin::Root.into(), 1, rates, 10,
            ).is_err());
        });
    }

    #[test]
    fn set_custom_config_validates_depth() {
        new_test_ext().execute_with(|| {
            let rates = frame_support::BoundedVec::try_from(vec![100u16]).unwrap();
            // depth=0 invalid
            assert!(pallet::Pallet::<Test>::set_custom_level_diff_config(
                frame_system::RawOrigin::Root.into(), 1, rates.clone(), 0,
            ).is_err());
            // depth=21 invalid
            assert!(pallet::Pallet::<Test>::set_custom_level_diff_config(
                frame_system::RawOrigin::Root.into(), 1, rates, 21,
            ).is_err());
        });
    }
}
