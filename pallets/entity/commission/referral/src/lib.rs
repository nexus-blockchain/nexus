//! # Commission Referral Plugin (pallet-commission-referral)
//!
//! 推荐链返佣插件，包含 5 种模式：
//! - 直推奖励 (DirectReward)
//! - 多级分销 (MultiLevel) — N 层 + 激活条件
//! - 固定金额 (FixedAmount)
//! - 首单奖励 (FirstOrder)
//! - 复购奖励 (RepeatPurchase)

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
        CommissionModes, CommissionOutput, CommissionPlugin, CommissionType, MemberProvider,
    };
    use sp_runtime::traits::{Saturating, Zero};

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // ========================================================================
    // Config structs
    // ========================================================================

    /// 直推奖励配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct DirectRewardConfig {
        pub rate: u16,
    }

    /// 多级分销层级配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct MultiLevelTier {
        pub rate: u16,
        pub required_directs: u32,
        pub required_team_size: u32,
        pub required_spent: u128,
    }

    /// 多级分销配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxLevels))]
    pub struct MultiLevelConfig<MaxLevels: Get<u32>> {
        pub levels: BoundedVec<MultiLevelTier, MaxLevels>,
        pub max_total_rate: u16,
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

    /// 固定金额配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct FixedAmountConfig<Balance> {
        pub amount: Balance,
    }

    impl<Balance: Default> Default for FixedAmountConfig<Balance> {
        fn default() -> Self {
            Self { amount: Balance::default() }
        }
    }

    /// 首单奖励配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct FirstOrderConfig<Balance> {
        pub amount: Balance,
        pub rate: u16,
        pub use_amount: bool,
    }

    impl<Balance: Default> Default for FirstOrderConfig<Balance> {
        fn default() -> Self {
            Self { amount: Balance::default(), rate: 0, use_amount: true }
        }
    }

    /// 复购奖励配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct RepeatPurchaseConfig {
        pub rate: u16,
        pub min_orders: u32,
    }

    /// 推荐链返佣总配置（per-shop）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxLevels))]
    pub struct ReferralConfig<Balance, MaxLevels: Get<u32>> {
        pub direct_reward: DirectRewardConfig,
        pub multi_level: MultiLevelConfig<MaxLevels>,
        pub fixed_amount: FixedAmountConfig<Balance>,
        pub first_order: FirstOrderConfig<Balance>,
        pub repeat_purchase: RepeatPurchaseConfig,
    }

    impl<Balance: Default, MaxLevels: Get<u32>> Default for ReferralConfig<Balance, MaxLevels> {
        fn default() -> Self {
            Self {
                direct_reward: DirectRewardConfig::default(),
                multi_level: MultiLevelConfig::default(),
                fixed_amount: FixedAmountConfig::default(),
                first_order: FirstOrderConfig::default(),
                repeat_purchase: RepeatPurchaseConfig::default(),
            }
        }
    }

    pub type ReferralConfigOf<T> = ReferralConfig<BalanceOf<T>, <T as Config>::MaxMultiLevels>;

    // ========================================================================
    // Pallet Config
    // ========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: Currency<Self::AccountId>;
        type MemberProvider: MemberProvider<Self::AccountId>;

        #[pallet::constant]
        type MaxMultiLevels: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ========================================================================
    // Storage
    // ========================================================================

    /// 推荐链返佣配置 shop_id -> ReferralConfig
    #[pallet::storage]
    #[pallet::getter(fn referral_config)]
    pub type ReferralConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        ReferralConfigOf<T>,
    >;

    // ========================================================================
    // Events
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ReferralConfigUpdated { shop_id: u64 },
    }

    // ========================================================================
    // Errors
    // ========================================================================

    #[pallet::error]
    pub enum Error<T> {
        InvalidRate,
    }

    // ========================================================================
    // Extrinsics (配置设置由各插件自己管理)
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 设置直推奖励配置
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_direct_reward_config(
            origin: OriginFor<T>,
            shop_id: u64,
            rate: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(rate <= 10000, Error::<T>::InvalidRate);

            ReferralConfigs::<T>::mutate(shop_id, |maybe| {
                let config = maybe.get_or_insert_with(ReferralConfig::default);
                config.direct_reward.rate = rate;
            });

            Self::deposit_event(Event::ReferralConfigUpdated { shop_id });
            Ok(())
        }

        /// 设置多级分销配置
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(45_000_000, 4_000))]
        pub fn set_multi_level_config(
            origin: OriginFor<T>,
            shop_id: u64,
            levels: BoundedVec<MultiLevelTier, T::MaxMultiLevels>,
            max_total_rate: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;
            for tier in levels.iter() {
                ensure!(tier.rate <= 10000, Error::<T>::InvalidRate);
            }
            ensure!(max_total_rate <= 10000, Error::<T>::InvalidRate);

            ReferralConfigs::<T>::mutate(shop_id, |maybe| {
                let config = maybe.get_or_insert_with(ReferralConfig::default);
                config.multi_level = MultiLevelConfig { levels, max_total_rate };
            });

            Self::deposit_event(Event::ReferralConfigUpdated { shop_id });
            Ok(())
        }

        /// 设置固定金额配置
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_fixed_amount_config(
            origin: OriginFor<T>,
            shop_id: u64,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ReferralConfigs::<T>::mutate(shop_id, |maybe| {
                let config = maybe.get_or_insert_with(ReferralConfig::default);
                config.fixed_amount = FixedAmountConfig { amount };
            });

            Self::deposit_event(Event::ReferralConfigUpdated { shop_id });
            Ok(())
        }

        /// 设置首单奖励配置
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_first_order_config(
            origin: OriginFor<T>,
            shop_id: u64,
            amount: BalanceOf<T>,
            rate: u16,
            use_amount: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ReferralConfigs::<T>::mutate(shop_id, |maybe| {
                let config = maybe.get_or_insert_with(ReferralConfig::default);
                config.first_order = FirstOrderConfig { amount, rate, use_amount };
            });

            Self::deposit_event(Event::ReferralConfigUpdated { shop_id });
            Ok(())
        }

        /// 设置复购奖励配置
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_repeat_purchase_config(
            origin: OriginFor<T>,
            shop_id: u64,
            rate: u16,
            min_orders: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ReferralConfigs::<T>::mutate(shop_id, |maybe| {
                let config = maybe.get_or_insert_with(ReferralConfig::default);
                config.repeat_purchase = RepeatPurchaseConfig { rate, min_orders };
            });

            Self::deposit_event(Event::ReferralConfigUpdated { shop_id });
            Ok(())
        }
    }

    // ========================================================================
    // Internal calculation functions
    // ========================================================================

    impl<T: Config> Pallet<T> {
        pub fn process_direct_reward(
            shop_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            remaining: &mut BalanceOf<T>,
            config: &DirectRewardConfig,
            outputs: &mut Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>,
        ) {
            if config.rate == 0 { return; }
            if let Some(referrer) = T::MemberProvider::get_referrer(shop_id, buyer) {
                let commission = order_amount.saturating_mul(config.rate.into()) / 10000u32.into();
                let actual = commission.min(*remaining);
                if !actual.is_zero() {
                    *remaining = remaining.saturating_sub(actual);
                    outputs.push(CommissionOutput {
                        beneficiary: referrer,
                        amount: actual,
                        commission_type: CommissionType::DirectReward,
                        level: 1,
                    });
                }
            }
        }

        pub fn process_multi_level(
            shop_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            remaining: &mut BalanceOf<T>,
            config: &MultiLevelConfigOf<T>,
            outputs: &mut Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>,
        ) {
            if config.levels.is_empty() { return; }

            let mut current_referrer = T::MemberProvider::get_referrer(shop_id, buyer);
            let mut total_commission = BalanceOf::<T>::zero();
            let max_commission = order_amount
                .saturating_mul(config.max_total_rate.into())
                / 10000u32.into();

            for (level_idx, tier) in config.levels.iter().enumerate() {
                if tier.rate == 0 {
                    current_referrer = current_referrer.and_then(|r| T::MemberProvider::get_referrer(shop_id, &r));
                    continue;
                }

                let Some(ref referrer) = current_referrer else { break };

                if !Self::check_tier_activation(shop_id, referrer, tier) {
                    current_referrer = T::MemberProvider::get_referrer(shop_id, referrer);
                    continue;
                }

                let commission = order_amount.saturating_mul(tier.rate.into()) / 10000u32.into();
                let actual = commission.min(*remaining);
                if actual.is_zero() { break; }

                let new_total = total_commission.saturating_add(actual);
                if new_total > max_commission {
                    let can_distribute = max_commission.saturating_sub(total_commission);
                    if !can_distribute.is_zero() {
                        *remaining = remaining.saturating_sub(can_distribute);
                        outputs.push(CommissionOutput {
                            beneficiary: referrer.clone(),
                            amount: can_distribute,
                            commission_type: CommissionType::MultiLevel,
                            level: (level_idx + 1) as u8,
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
                    level: (level_idx + 1) as u8,
                });

                current_referrer = T::MemberProvider::get_referrer(shop_id, referrer);
            }
        }

        pub fn check_tier_activation(
            shop_id: u64,
            account: &T::AccountId,
            tier: &MultiLevelTier,
        ) -> bool {
            if tier.required_directs == 0 && tier.required_team_size == 0 && tier.required_spent == 0 {
                return true;
            }
            let (direct_referrals, team_size, total_spent) = T::MemberProvider::get_member_stats(shop_id, account);
            if tier.required_directs > 0 && direct_referrals < tier.required_directs { return false; }
            if tier.required_team_size > 0 && team_size < tier.required_team_size { return false; }
            if tier.required_spent > 0 && total_spent < tier.required_spent { return false; }
            true
        }

        pub fn process_fixed_amount(
            shop_id: u64,
            buyer: &T::AccountId,
            remaining: &mut BalanceOf<T>,
            config: &FixedAmountConfig<BalanceOf<T>>,
            outputs: &mut Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>,
        ) {
            if config.amount.is_zero() { return; }
            if let Some(referrer) = T::MemberProvider::get_referrer(shop_id, buyer) {
                let actual = config.amount.min(*remaining);
                if !actual.is_zero() {
                    *remaining = remaining.saturating_sub(actual);
                    outputs.push(CommissionOutput {
                        beneficiary: referrer,
                        amount: actual,
                        commission_type: CommissionType::FixedAmount,
                        level: 1,
                    });
                }
            }
        }

        pub fn process_first_order(
            shop_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            remaining: &mut BalanceOf<T>,
            config: &FirstOrderConfig<BalanceOf<T>>,
            outputs: &mut Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>,
        ) {
            if let Some(referrer) = T::MemberProvider::get_referrer(shop_id, buyer) {
                let commission = if config.use_amount {
                    config.amount
                } else {
                    order_amount.saturating_mul(config.rate.into()) / 10000u32.into()
                };
                let actual = commission.min(*remaining);
                if !actual.is_zero() {
                    *remaining = remaining.saturating_sub(actual);
                    outputs.push(CommissionOutput {
                        beneficiary: referrer,
                        amount: actual,
                        commission_type: CommissionType::FirstOrder,
                        level: 1,
                    });
                }
            }
        }

        pub fn process_repeat_purchase(
            shop_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            remaining: &mut BalanceOf<T>,
            config: &RepeatPurchaseConfig,
            buyer_order_count: u32,
            outputs: &mut Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>,
        ) {
            if config.rate == 0 || buyer_order_count < config.min_orders { return; }
            if let Some(referrer) = T::MemberProvider::get_referrer(shop_id, buyer) {
                let commission = order_amount.saturating_mul(config.rate.into()) / 10000u32.into();
                let actual = commission.min(*remaining);
                if !actual.is_zero() {
                    *remaining = remaining.saturating_sub(actual);
                    outputs.push(CommissionOutput {
                        beneficiary: referrer,
                        amount: actual,
                        commission_type: CommissionType::RepeatPurchase,
                        level: 1,
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
        shop_id: u64,
        buyer: &T::AccountId,
        order_amount: pallet::BalanceOf<T>,
        remaining: pallet::BalanceOf<T>,
        enabled_modes: pallet_commission_common::CommissionModes,
        is_first_order: bool,
        buyer_order_count: u32,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, pallet::BalanceOf<T>>>, pallet::BalanceOf<T>) {
        use pallet_commission_common::CommissionModes;

        // 配置按 entity_id 查询，推荐链按 shop_id 查询
        let config = match pallet::ReferralConfigs::<T>::get(entity_id) {
            Some(c) => c,
            None => return (alloc::vec::Vec::new(), remaining),
        };

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        if enabled_modes.contains(CommissionModes::DIRECT_REWARD) {
            pallet::Pallet::<T>::process_direct_reward(
                shop_id, buyer, order_amount, &mut remaining, &config.direct_reward, &mut outputs,
            );
        }

        if enabled_modes.contains(CommissionModes::MULTI_LEVEL) {
            pallet::Pallet::<T>::process_multi_level(
                shop_id, buyer, order_amount, &mut remaining, &config.multi_level, &mut outputs,
            );
        }

        if enabled_modes.contains(CommissionModes::FIXED_AMOUNT) {
            pallet::Pallet::<T>::process_fixed_amount(
                shop_id, buyer, &mut remaining, &config.fixed_amount, &mut outputs,
            );
        }

        if enabled_modes.contains(CommissionModes::FIRST_ORDER) && is_first_order {
            pallet::Pallet::<T>::process_first_order(
                shop_id, buyer, order_amount, &mut remaining, &config.first_order, &mut outputs,
            );
        }

        if enabled_modes.contains(CommissionModes::REPEAT_PURCHASE) {
            pallet::Pallet::<T>::process_repeat_purchase(
                shop_id, buyer, order_amount, &mut remaining, &config.repeat_purchase, buyer_order_count, &mut outputs,
            );
        }

        (outputs, remaining)
    }
}

// ============================================================================
// ReferralPlanWriter implementation
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::ReferralPlanWriter<pallet::BalanceOf<T>> for pallet::Pallet<T> {
    fn set_direct_rate(shop_id: u64, rate: u16) -> Result<(), sp_runtime::DispatchError> {
        pallet::ReferralConfigs::<T>::mutate(shop_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::ReferralConfig::default);
            config.direct_reward.rate = rate;
        });
        Ok(())
    }

    fn set_multi_level(shop_id: u64, level_rates: alloc::vec::Vec<u16>, max_total_rate: u16) -> Result<(), sp_runtime::DispatchError> {
        pallet::ReferralConfigs::<T>::mutate(shop_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::ReferralConfig::default);
            let bounded: frame_support::BoundedVec<pallet::MultiLevelTier, T::MaxMultiLevels> = level_rates
                .into_iter()
                .map(|rate| pallet::MultiLevelTier { rate, required_directs: 0, required_team_size: 0, required_spent: 0 })
                .collect::<alloc::vec::Vec<_>>()
                .try_into()
                .unwrap_or_default();
            config.multi_level = pallet::MultiLevelConfig { levels: bounded, max_total_rate };
        });
        Ok(())
    }

    fn set_fixed_amount(shop_id: u64, amount: pallet::BalanceOf<T>) -> Result<(), sp_runtime::DispatchError> {
        pallet::ReferralConfigs::<T>::mutate(shop_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::ReferralConfig::default);
            config.fixed_amount = pallet::FixedAmountConfig { amount };
        });
        Ok(())
    }

    fn set_first_order(shop_id: u64, amount: pallet::BalanceOf<T>, rate: u16, use_amount: bool) -> Result<(), sp_runtime::DispatchError> {
        pallet::ReferralConfigs::<T>::mutate(shop_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::ReferralConfig::default);
            config.first_order = pallet::FirstOrderConfig { amount, rate, use_amount };
        });
        Ok(())
    }

    fn set_repeat_purchase(shop_id: u64, rate: u16, min_orders: u32) -> Result<(), sp_runtime::DispatchError> {
        pallet::ReferralConfigs::<T>::mutate(shop_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::ReferralConfig::default);
            config.repeat_purchase = pallet::RepeatPurchaseConfig { rate, min_orders };
        });
        Ok(())
    }

    fn clear_config(shop_id: u64) -> Result<(), sp_runtime::DispatchError> {
        pallet::ReferralConfigs::<T>::remove(shop_id);
        Ok(())
    }
}
