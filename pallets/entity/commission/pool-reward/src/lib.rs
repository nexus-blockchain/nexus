//! # Commission Pool Reward Plugin (pallet-commission-pool-reward)
//!
//! 沉淀池奖励插件：将未分配佣金沉淀池中的资金，以领奖奖金形式回馈给推荐链上的高等级会员。
//!
//! ## 核心逻辑
//!
//! 当买家下单时，core 调度引擎将沉淀池余额作为 `remaining` 传入本插件。
//! 插件沿买家推荐链向上遍历（最多 `max_depth` 层），对每个上级查询其 `custom_level_id`，
//! 匹配配置中的等级奖励比例，按 `order_amount × rate / 10000` 计算奖金。
//!
//! 单次订单最大可消耗池余额的 `max_drain_rate / 10000`，防止单笔大单耗尽池子。
//!
//! ## Entity Owner 不可提取
//!
//! 沉淀池资金完全由算法驱动分配，Entity Owner 无法直接提取。

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

    /// 沉淀池奖励配置（per-entity）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxLevels))]
    pub struct PoolRewardConfig<MaxLevels: Get<u32>> {
        /// 各等级奖励比例（基点），按 (level_id, rate_bps) 索引
        /// 仅配置了的等级可获得奖励，未配置的等级不参与
        pub level_rates: BoundedVec<(u8, u16), MaxLevels>,
        /// 沿推荐链向上最大遍历深度
        pub max_depth: u8,
        /// 单次订单最大可消耗池余额比例（基点，如 500 = 5%）
        /// 防止单笔大单耗尽池子
        pub max_drain_rate: u16,
    }

    impl<MaxLevels: Get<u32>> Default for PoolRewardConfig<MaxLevels> {
        fn default() -> Self {
            Self {
                level_rates: BoundedVec::default(),
                max_depth: 10,
                max_drain_rate: 500, // 默认 5%
            }
        }
    }

    pub type PoolRewardConfigOf<T> = PoolRewardConfig<<T as Config>::MaxPoolRewardLevels>;

    // ========================================================================
    // Pallet Config
    // ========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: Currency<Self::AccountId>;
        type MemberProvider: MemberProvider<Self::AccountId>;

        /// 最大等级配置数
        #[pallet::constant]
        type MaxPoolRewardLevels: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ========================================================================
    // Storage
    // ========================================================================

    /// 沉淀池奖励配置 entity_id -> PoolRewardConfig
    #[pallet::storage]
    #[pallet::getter(fn pool_reward_config)]
    pub type PoolRewardConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        PoolRewardConfigOf<T>,
    >;

    // ========================================================================
    // Events / Errors
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        PoolRewardConfigUpdated { entity_id: u64 },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// 费率无效（超过 10000 基点）
        InvalidRate,
        /// 遍历深度无效
        InvalidMaxDepth,
        /// max_drain_rate 无效
        InvalidDrainRate,
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 设置沉淀池奖励配置（Root / Governance）
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(45_000_000, 4_000))]
        pub fn set_pool_reward_config(
            origin: OriginFor<T>,
            entity_id: u64,
            level_rates: BoundedVec<(u8, u16), T::MaxPoolRewardLevels>,
            max_depth: u8,
            max_drain_rate: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;

            for (_, rate) in level_rates.iter() {
                ensure!(*rate <= 10000, Error::<T>::InvalidRate);
            }
            ensure!(max_depth > 0 && max_depth <= 30, Error::<T>::InvalidMaxDepth);
            ensure!(max_drain_rate > 0 && max_drain_rate <= 10000, Error::<T>::InvalidDrainRate);

            PoolRewardConfigs::<T>::insert(entity_id, PoolRewardConfig {
                level_rates,
                max_depth,
                max_drain_rate,
            });

            Self::deposit_event(Event::PoolRewardConfigUpdated { entity_id });
            Ok(())
        }
    }

    // ========================================================================
    // Internal calculation
    // ========================================================================

    impl<T: Config> Pallet<T> {
        /// 处理沉淀池奖励分配
        ///
        /// remaining = 池余额（由 core 传入）
        /// 返回 (outputs, new_remaining)
        pub fn process_pool_reward(
            entity_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            remaining: &mut BalanceOf<T>,
            config: &PoolRewardConfigOf<T>,
            outputs: &mut Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>,
        ) {
            if config.level_rates.is_empty() || remaining.is_zero() {
                return;
            }

            // 计算本次可分配上限 = pool_balance × max_drain_rate / 10000
            let pool_balance = *remaining;
            let mut cap = pool_balance
                .saturating_mul(config.max_drain_rate.into())
                / 10000u32.into();

            let mut current = T::MemberProvider::get_referrer(entity_id, buyer);
            let mut depth: u8 = 0;

            while let Some(ref ancestor) = current {
                depth += 1;
                if depth > config.max_depth { break; }
                if cap.is_zero() { break; }

                let level_id = T::MemberProvider::custom_level_id(entity_id, ancestor);

                // 查找该等级的奖励比例
                let rate = config.level_rates.iter()
                    .find(|(id, _)| *id == level_id)
                    .map(|(_, r)| *r)
                    .unwrap_or(0);

                if rate > 0 {
                    let reward = order_amount
                        .saturating_mul(rate.into())
                        / 10000u32.into();
                    let actual = reward.min(cap).min(*remaining);

                    if !actual.is_zero() {
                        cap = cap.saturating_sub(actual);
                        *remaining = remaining.saturating_sub(actual);
                        outputs.push(CommissionOutput {
                            beneficiary: ancestor.clone(),
                            amount: actual,
                            commission_type: CommissionType::PoolReward,
                            level: depth,
                        });
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

        if !enabled_modes.contains(CommissionModes::POOL_REWARD) {
            return (alloc::vec::Vec::new(), remaining);
        }

        let config = match pallet::PoolRewardConfigs::<T>::get(entity_id) {
            Some(c) => c,
            None => return (alloc::vec::Vec::new(), remaining),
        };

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        pallet::Pallet::<T>::process_pool_reward(
            entity_id, buyer, order_amount, &mut remaining, &config, &mut outputs,
        );

        (outputs, remaining)
    }
}

// ============================================================================
// PoolRewardPlanWriter implementation
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::PoolRewardPlanWriter for pallet::Pallet<T> {
    fn set_pool_reward_config(
        entity_id: u64,
        level_rates: alloc::vec::Vec<(u8, u16)>,
        max_depth: u8,
        max_drain_rate: u16,
    ) -> Result<(), sp_runtime::DispatchError> {
        let bounded: frame_support::BoundedVec<(u8, u16), T::MaxPoolRewardLevels> = level_rates
            .try_into()
            .map_err(|_| sp_runtime::DispatchError::Other("TooManyLevels"))?;

        pallet::PoolRewardConfigs::<T>::insert(entity_id, pallet::PoolRewardConfig {
            level_rates: bounded,
            max_depth,
            max_drain_rate,
        });
        Ok(())
    }

    fn clear_config(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        pallet::PoolRewardConfigs::<T>::remove(entity_id);
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
        static CUSTOM_LEVEL_IDS: RefCell<BTreeMap<(u64, u64), u8>> = RefCell::new(BTreeMap::new());
    }

    fn clear_mocks() {
        REFERRERS.with(|r| r.borrow_mut().clear());
        CUSTOM_LEVEL_IDS.with(|c| c.borrow_mut().clear());
    }

    pub struct MockMemberProvider;

    impl pallet_commission_common::MemberProvider<u64> for MockMemberProvider {
        fn is_member(_: u64, _: &u64) -> bool { true }
        fn get_referrer(entity_id: u64, account: &u64) -> Option<u64> {
            REFERRERS.with(|r| r.borrow().get(&(entity_id, *account)).copied())
        }
        fn member_level(_: u64, _: &u64) -> Option<pallet_entity_common::MemberLevel> { None }
        fn get_member_stats(_: u64, _: &u64) -> (u32, u32, u128) { (0, 0, 0) }
        fn uses_custom_levels(_: u64) -> bool { true }
        fn custom_level_id(entity_id: u64, account: &u64) -> u8 {
            CUSTOM_LEVEL_IDS.with(|c| c.borrow().get(&(entity_id, *account)).copied().unwrap_or(0))
        }
        fn get_level_commission_bonus(_: u64, _: u8) -> u16 { 0 }
        fn auto_register(_: u64, _: &u64, _: Option<u64>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn set_custom_levels_enabled(_: u64, _: bool) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn set_upgrade_mode(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn add_custom_level(_: u64, _: u8, _: &[u8], _: u128, _: u16, _: u16) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn update_custom_level(_: u64, _: u8, _: Option<&[u8]>, _: Option<u128>, _: Option<u16>, _: Option<u16>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn remove_custom_level(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn custom_level_count(_: u64) -> u8 { 0 }
    }

    // -- Mock Runtime --
    frame_support::construct_runtime!(
        pub enum Test {
            System: frame_system,
            Balances: pallet_balances,
            CommissionPoolReward: pallet,
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
        type MaxPoolRewardLevels = ConstU32<10>;
    }

    fn new_test_ext() -> sp_io::TestExternalities {
        clear_mocks();
        let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| System::set_block_number(1));
        ext
    }

    fn setup_chain(entity_id: u64) {
        // 推荐链: 50 → 40 → 30 → 20 → 10
        REFERRERS.with(|r| {
            let mut m = r.borrow_mut();
            m.insert((entity_id, 50), 40);
            m.insert((entity_id, 40), 30);
            m.insert((entity_id, 30), 20);
            m.insert((entity_id, 20), 10);
        });
    }

    // ====================================================================
    // Extrinsic tests
    // ====================================================================

    #[test]
    fn set_config_works() {
        new_test_ext().execute_with(|| {
            let rates = vec![(0u8, 0u16), (1, 50), (2, 100), (3, 200)];
            assert_ok!(CommissionPoolReward::set_pool_reward_config(
                RuntimeOrigin::root(),
                1,
                rates.try_into().unwrap(),
                10,
                500,
            ));
            let config = pallet::PoolRewardConfigs::<Test>::get(1).unwrap();
            assert_eq!(config.level_rates.len(), 4);
            assert_eq!(config.max_depth, 10);
            assert_eq!(config.max_drain_rate, 500);
        });
    }

    #[test]
    fn set_config_rejects_invalid_rate() {
        new_test_ext().execute_with(|| {
            let rates = vec![(1u8, 10001u16)];
            assert_noop!(
                CommissionPoolReward::set_pool_reward_config(
                    RuntimeOrigin::root(), 1, rates.try_into().unwrap(), 10, 500,
                ),
                Error::<Test>::InvalidRate
            );
        });
    }

    #[test]
    fn set_config_rejects_invalid_depth() {
        new_test_ext().execute_with(|| {
            let rates = vec![(1u8, 100u16)];
            assert_noop!(
                CommissionPoolReward::set_pool_reward_config(
                    RuntimeOrigin::root(), 1, rates.try_into().unwrap(), 0, 500,
                ),
                Error::<Test>::InvalidMaxDepth
            );
        });
    }

    #[test]
    fn set_config_rejects_invalid_drain_rate() {
        new_test_ext().execute_with(|| {
            let rates = vec![(1u8, 100u16)];
            assert_noop!(
                CommissionPoolReward::set_pool_reward_config(
                    RuntimeOrigin::root(), 1, rates.try_into().unwrap(), 10, 0,
                ),
                Error::<Test>::InvalidDrainRate
            );
        });
    }

    #[test]
    fn set_config_requires_root() {
        new_test_ext().execute_with(|| {
            let rates = vec![(1u8, 100u16)];
            assert_noop!(
                CommissionPoolReward::set_pool_reward_config(
                    RuntimeOrigin::signed(1), 1, rates.try_into().unwrap(), 10, 500,
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
            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::POOL_REWARD);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 5000, modes, false, 0,
            );
            assert!(outputs.is_empty());
            assert_eq!(remaining, 5000);
        });
    }

    #[test]
    fn mode_not_enabled_returns_empty() {
        new_test_ext().execute_with(|| {
            let rates = vec![(1u8, 200u16)];
            assert_ok!(CommissionPoolReward::set_pool_reward_config(
                RuntimeOrigin::root(), 1, rates.try_into().unwrap(), 10, 500,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::DIRECT_REWARD);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 5000, modes, false, 0,
            );
            assert!(outputs.is_empty());
            assert_eq!(remaining, 5000);
        });
    }

    #[test]
    fn basic_pool_reward_distribution() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_chain(entity_id);

            // 等级设置: 40=level_1(50bps), 30=level_2(100bps), 20=level_0(不参与)
            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((entity_id, 40), 1);
                m.insert((entity_id, 30), 2);
                m.insert((entity_id, 20), 0);
            });

            // 配置: level_0=0bps, level_1=50bps(0.5%), level_2=100bps(1%)
            let rates = vec![(0u8, 0u16), (1, 50), (2, 100)];
            assert_ok!(CommissionPoolReward::set_pool_reward_config(
                RuntimeOrigin::root(),
                entity_id,
                rates.try_into().unwrap(),
                10,
                5000, // max_drain_rate = 50% of pool
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::POOL_REWARD);
            // pool_balance=10000, order_amount=100000
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                entity_id, &50, 100000, 10000, modes, false, 0,
            );

            // cap = 10000 * 5000 / 10000 = 5000
            // 40: level_1, rate=50 → 100000*50/10000 = 500, actual=500 (cap=5000), remaining=9500
            // 30: level_2, rate=100 → 100000*100/10000 = 1000, actual=1000 (cap=4500), remaining=8500
            // 20: level_0, rate=0 → skip
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 500);
            assert_eq!(outputs[0].commission_type, pallet_commission_common::CommissionType::PoolReward);
            assert_eq!(outputs[1].beneficiary, 30);
            assert_eq!(outputs[1].amount, 1000);
            assert_eq!(remaining, 10000 - 500 - 1000);
        });
    }

    #[test]
    fn max_drain_rate_caps_distribution() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_chain(entity_id);

            // 所有上级都是高等级
            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((entity_id, 40), 1);
                m.insert((entity_id, 30), 1);
                m.insert((entity_id, 20), 1);
                m.insert((entity_id, 10), 1);
            });

            // 高费率 + 低 drain rate
            let rates = vec![(1u8, 2000u16)]; // 20% per level
            assert_ok!(CommissionPoolReward::set_pool_reward_config(
                RuntimeOrigin::root(),
                entity_id,
                rates.try_into().unwrap(),
                10,
                100, // max_drain_rate = 1% of pool
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::POOL_REWARD);
            // pool=100000, order=100000
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                entity_id, &50, 100000, 100000, modes, false, 0,
            );

            // cap = 100000 * 100 / 10000 = 1000
            // 40: 100000*2000/10000 = 20000 → capped to 1000
            // cap exhausted, no more
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].amount, 1000);
            assert_eq!(remaining, 100000 - 1000);
        });
    }

    #[test]
    fn pool_balance_caps_distribution() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_chain(entity_id);

            CUSTOM_LEVEL_IDS.with(|c| {
                c.borrow_mut().insert((entity_id, 40), 1);
            });

            let rates = vec![(1u8, 5000u16)]; // 50%
            assert_ok!(CommissionPoolReward::set_pool_reward_config(
                RuntimeOrigin::root(),
                entity_id,
                rates.try_into().unwrap(),
                10,
                10000, // max_drain_rate = 100%
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::POOL_REWARD);
            // pool=100 (small), order=10000
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                entity_id, &50, 10000, 100, modes, false, 0,
            );

            // cap = 100 * 10000 / 10000 = 100
            // 40: 10000*5000/10000 = 5000 → capped by remaining=100 → actual=100
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].amount, 100);
            assert_eq!(remaining, 0);
        });
    }

    #[test]
    fn max_depth_limits_traversal() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_chain(entity_id);

            // 只有 account 10 有高等级（深度4）
            CUSTOM_LEVEL_IDS.with(|c| {
                c.borrow_mut().insert((entity_id, 10), 1);
            });

            let rates = vec![(1u8, 500u16)];
            assert_ok!(CommissionPoolReward::set_pool_reward_config(
                RuntimeOrigin::root(),
                entity_id,
                rates.try_into().unwrap(),
                2, // max_depth=2，只能到 account 30
                5000,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::POOL_REWARD);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                entity_id, &50, 10000, 10000, modes, false, 0,
            );

            // max_depth=2 只遍历到 30（深度2），10 在深度4 被截断
            assert!(outputs.is_empty());
            assert_eq!(remaining, 10000);
        });
    }

    #[test]
    fn plan_writer_works() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::PoolRewardPlanWriter;
            assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_pool_reward_config(
                1,
                vec![(0, 0), (1, 100), (2, 200)],
                8,
                1000,
            ));
            let config = pallet::PoolRewardConfigs::<Test>::get(1).unwrap();
            assert_eq!(config.level_rates.len(), 3);
            assert_eq!(config.level_rates[1], (1, 100));
            assert_eq!(config.max_depth, 8);
            assert_eq!(config.max_drain_rate, 1000);

            assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::clear_config(1));
            assert!(pallet::PoolRewardConfigs::<Test>::get(1).is_none());
        });
    }
}
