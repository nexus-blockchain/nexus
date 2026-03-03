//! # Commission Referral Plugin (pallet-commission-referral)
//!
//! 推荐链返佣插件，包含 4 种模式：
//! - 直推奖励 (DirectReward)
//! - 固定金额 (FixedAmount)
//! - 首单奖励 (FirstOrder)
//! - 复购奖励 (RepeatPurchase)
//!
//! 注: 多级分销 (MultiLevel) 已分离为独立 pallet: `pallet-commission-multi-level`

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::Currency,
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

    /// 直推奖励配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct DirectRewardConfig {
        pub rate: u16,
    }

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

    /// 推荐链返佣总配置（per-entity）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct ReferralConfig<Balance> {
        pub direct_reward: DirectRewardConfig,
        pub fixed_amount: FixedAmountConfig<Balance>,
        pub first_order: FirstOrderConfig<Balance>,
        pub repeat_purchase: RepeatPurchaseConfig,
    }

    impl<Balance: Default> Default for ReferralConfig<Balance> {
        fn default() -> Self {
            Self {
                direct_reward: DirectRewardConfig::default(),
                fixed_amount: FixedAmountConfig::default(),
                first_order: FirstOrderConfig::default(),
                repeat_purchase: RepeatPurchaseConfig::default(),
            }
        }
    }

    pub type ReferralConfigOf<T> = ReferralConfig<BalanceOf<T>>;

    // ========================================================================
    // Pallet Config
    // ========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: Currency<Self::AccountId>;
        type MemberProvider: MemberProvider<Self::AccountId>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ========================================================================
    // Storage
    // ========================================================================

    /// 推荐链返佣配置 entity_id -> ReferralConfig
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
        ReferralConfigUpdated { entity_id: u64 },
        ReferralConfigCleared { entity_id: u64 },
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
            entity_id: u64,
            rate: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(rate <= 10000, Error::<T>::InvalidRate);

            ReferralConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(ReferralConfig::default);
                config.direct_reward.rate = rate;
            });

            Self::deposit_event(Event::ReferralConfigUpdated { entity_id });
            Ok(())
        }

        /// 设置固定金额配置
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_fixed_amount_config(
            origin: OriginFor<T>,
            entity_id: u64,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ReferralConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(ReferralConfig::default);
                config.fixed_amount = FixedAmountConfig { amount };
            });

            Self::deposit_event(Event::ReferralConfigUpdated { entity_id });
            Ok(())
        }

        /// 设置首单奖励配置
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_first_order_config(
            origin: OriginFor<T>,
            entity_id: u64,
            amount: BalanceOf<T>,
            rate: u16,
            use_amount: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;
            // H1 审计修复: rate 用于比例模式计算，必须 <= 10000 基点
            ensure!(rate <= 10000, Error::<T>::InvalidRate);

            ReferralConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(ReferralConfig::default);
                config.first_order = FirstOrderConfig { amount, rate, use_amount };
            });

            Self::deposit_event(Event::ReferralConfigUpdated { entity_id });
            Ok(())
        }

        /// 设置复购奖励配置
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_repeat_purchase_config(
            origin: OriginFor<T>,
            entity_id: u64,
            rate: u16,
            min_orders: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;
            // H2 审计修复: rate 必须 <= 10000 基点
            ensure!(rate <= 10000, Error::<T>::InvalidRate);

            ReferralConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(ReferralConfig::default);
                config.repeat_purchase = RepeatPurchaseConfig { rate, min_orders };
            });

            Self::deposit_event(Event::ReferralConfigUpdated { entity_id });
            Ok(())
        }
    }

    // ========================================================================
    // Internal calculation functions
    // ========================================================================

    impl<T: Config> Pallet<T> {
        pub fn process_direct_reward(
            entity_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            remaining: &mut BalanceOf<T>,
            config: &DirectRewardConfig,
            outputs: &mut Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>,
        ) {
            if config.rate == 0 { return; }
            if let Some(referrer) = T::MemberProvider::get_referrer(entity_id, buyer) {
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

        pub fn process_fixed_amount(
            entity_id: u64,
            buyer: &T::AccountId,
            remaining: &mut BalanceOf<T>,
            config: &FixedAmountConfig<BalanceOf<T>>,
            outputs: &mut Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>,
        ) {
            if config.amount.is_zero() { return; }
            if let Some(referrer) = T::MemberProvider::get_referrer(entity_id, buyer) {
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
            entity_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            remaining: &mut BalanceOf<T>,
            config: &FirstOrderConfig<BalanceOf<T>>,
            outputs: &mut Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>,
        ) {
            // H3 审计修复: 零值早返回，避免不必要的 storage read
            if config.use_amount && config.amount.is_zero() { return; }
            if !config.use_amount && config.rate == 0 { return; }
            if let Some(referrer) = T::MemberProvider::get_referrer(entity_id, buyer) {
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
            entity_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            remaining: &mut BalanceOf<T>,
            config: &RepeatPurchaseConfig,
            buyer_order_count: u32,
            outputs: &mut Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>,
        ) {
            if config.rate == 0 || buyer_order_count < config.min_orders { return; }
            if let Some(referrer) = T::MemberProvider::get_referrer(entity_id, buyer) {
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
        buyer: &T::AccountId,
        order_amount: pallet::BalanceOf<T>,
        remaining: pallet::BalanceOf<T>,
        enabled_modes: pallet_commission_common::CommissionModes,
        is_first_order: bool,
        buyer_order_count: u32,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, pallet::BalanceOf<T>>>, pallet::BalanceOf<T>) {
        use pallet_commission_common::CommissionModes;

        let config = match pallet::ReferralConfigs::<T>::get(entity_id) {
            Some(c) => c,
            None => return (alloc::vec::Vec::new(), remaining),
        };

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        if enabled_modes.contains(CommissionModes::DIRECT_REWARD) {
            pallet::Pallet::<T>::process_direct_reward(
                entity_id, buyer, order_amount, &mut remaining, &config.direct_reward, &mut outputs,
            );
        }

        if enabled_modes.contains(CommissionModes::FIXED_AMOUNT) {
            pallet::Pallet::<T>::process_fixed_amount(
                entity_id, buyer, &mut remaining, &config.fixed_amount, &mut outputs,
            );
        }

        if enabled_modes.contains(CommissionModes::FIRST_ORDER) && is_first_order {
            pallet::Pallet::<T>::process_first_order(
                entity_id, buyer, order_amount, &mut remaining, &config.first_order, &mut outputs,
            );
        }

        if enabled_modes.contains(CommissionModes::REPEAT_PURCHASE) {
            pallet::Pallet::<T>::process_repeat_purchase(
                entity_id, buyer, order_amount, &mut remaining, &config.repeat_purchase, buyer_order_count, &mut outputs,
            );
        }

        (outputs, remaining)
    }
}

// ============================================================================
// Token 多资产 — TokenCommissionPlugin implementation
// ============================================================================

use pallet_commission_common::MemberProvider as _;

/// Token 版泛型计算辅助方法
///
/// 与 NEX 版共用同一份 ReferralConfig（rates 为 u16 bps，对任意 Balance 类型通用）。
/// 固定金额模式（FIXED_AMOUNT / FIRST_ORDER use_amount=true）对 Token 不生效。
impl<T: pallet::Config> pallet::Pallet<T> {
    fn process_direct_reward_token<TB>(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: TB,
        remaining: &mut TB,
        rate: u16,
        outputs: &mut alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>,
    ) where
        TB: sp_runtime::traits::AtLeast32BitUnsigned + Copy,
    {
        if rate == 0 { return; }
        if let Some(referrer) = T::MemberProvider::get_referrer(entity_id, buyer) {
            let commission = order_amount.saturating_mul(TB::from(rate as u32)) / TB::from(10000u32);
            let actual = commission.min(*remaining);
            if !actual.is_zero() {
                *remaining = remaining.saturating_sub(actual);
                outputs.push(pallet_commission_common::CommissionOutput {
                    beneficiary: referrer,
                    amount: actual,
                    commission_type: pallet_commission_common::CommissionType::DirectReward,
                    level: 1,
                });
            }
        }
    }

    fn process_first_order_token<TB>(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: TB,
        remaining: &mut TB,
        config: &pallet::FirstOrderConfig<pallet::BalanceOf<T>>,
        outputs: &mut alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>,
    ) where
        TB: sp_runtime::traits::AtLeast32BitUnsigned + Copy,
    {
        // Token: 仅支持 rate 模式，use_amount=true（固定金额）跳过
        if config.use_amount || config.rate == 0 { return; }
        if let Some(referrer) = T::MemberProvider::get_referrer(entity_id, buyer) {
            let commission = order_amount.saturating_mul(TB::from(config.rate as u32)) / TB::from(10000u32);
            let actual = commission.min(*remaining);
            if !actual.is_zero() {
                *remaining = remaining.saturating_sub(actual);
                outputs.push(pallet_commission_common::CommissionOutput {
                    beneficiary: referrer,
                    amount: actual,
                    commission_type: pallet_commission_common::CommissionType::FirstOrder,
                    level: 1,
                });
            }
        }
    }

    fn process_repeat_purchase_token<TB>(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: TB,
        remaining: &mut TB,
        config: &pallet::RepeatPurchaseConfig,
        buyer_order_count: u32,
        outputs: &mut alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>,
    ) where
        TB: sp_runtime::traits::AtLeast32BitUnsigned + Copy,
    {
        if config.rate == 0 || buyer_order_count < config.min_orders { return; }
        if let Some(referrer) = T::MemberProvider::get_referrer(entity_id, buyer) {
            let commission = order_amount.saturating_mul(TB::from(config.rate as u32)) / TB::from(10000u32);
            let actual = commission.min(*remaining);
            if !actual.is_zero() {
                *remaining = remaining.saturating_sub(actual);
                outputs.push(pallet_commission_common::CommissionOutput {
                    beneficiary: referrer,
                    amount: actual,
                    commission_type: pallet_commission_common::CommissionType::RepeatPurchase,
                    level: 1,
                });
            }
        }
    }
}

/// TokenCommissionPlugin: 泛型 Token 佣金计算
///
/// 对任意 `TB: AtLeast32BitUnsigned + Copy` 实现，无需修改 Config。
/// 共用 NEX 版 ReferralConfig 中的 rate 配置（bps），跳过固定金额模式。
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
        is_first_order: bool,
        buyer_order_count: u32,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>, TB) {
        use pallet_commission_common::CommissionModes;

        let config = match pallet::ReferralConfigs::<T>::get(entity_id) {
            Some(c) => c,
            None => return (alloc::vec::Vec::new(), remaining),
        };

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        if enabled_modes.contains(CommissionModes::DIRECT_REWARD) {
            pallet::Pallet::<T>::process_direct_reward_token::<TB>(
                entity_id, buyer, order_amount, &mut remaining,
                config.direct_reward.rate, &mut outputs,
            );
        }

        // FIXED_AMOUNT: 跳过（固定金额以 NEX 计价，不适用于 Token）

        if enabled_modes.contains(CommissionModes::FIRST_ORDER) && is_first_order {
            pallet::Pallet::<T>::process_first_order_token::<TB>(
                entity_id, buyer, order_amount, &mut remaining,
                &config.first_order, &mut outputs,
            );
        }

        if enabled_modes.contains(CommissionModes::REPEAT_PURCHASE) {
            pallet::Pallet::<T>::process_repeat_purchase_token::<TB>(
                entity_id, buyer, order_amount, &mut remaining,
                &config.repeat_purchase, buyer_order_count, &mut outputs,
            );
        }

        (outputs, remaining)
    }
}

// ============================================================================
// ReferralPlanWriter implementation
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::ReferralPlanWriter<pallet::BalanceOf<T>> for pallet::Pallet<T> {
    fn set_direct_rate(entity_id: u64, rate: u16) -> Result<(), sp_runtime::DispatchError> {
        // H2 审计修复: 防御性校验
        frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        pallet::ReferralConfigs::<T>::mutate(entity_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::ReferralConfig::default);
            config.direct_reward.rate = rate;
        });
        pallet::Pallet::<T>::deposit_event(pallet::Event::ReferralConfigUpdated { entity_id });
        Ok(())
    }

    fn set_fixed_amount(entity_id: u64, amount: pallet::BalanceOf<T>) -> Result<(), sp_runtime::DispatchError> {
        pallet::ReferralConfigs::<T>::mutate(entity_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::ReferralConfig::default);
            config.fixed_amount = pallet::FixedAmountConfig { amount };
        });
        pallet::Pallet::<T>::deposit_event(pallet::Event::ReferralConfigUpdated { entity_id });
        Ok(())
    }

    fn set_first_order(entity_id: u64, amount: pallet::BalanceOf<T>, rate: u16, use_amount: bool) -> Result<(), sp_runtime::DispatchError> {
        // H2 审计修复: 防御性校验
        frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        pallet::ReferralConfigs::<T>::mutate(entity_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::ReferralConfig::default);
            config.first_order = pallet::FirstOrderConfig { amount, rate, use_amount };
        });
        pallet::Pallet::<T>::deposit_event(pallet::Event::ReferralConfigUpdated { entity_id });
        Ok(())
    }

    fn set_repeat_purchase(entity_id: u64, rate: u16, min_orders: u32) -> Result<(), sp_runtime::DispatchError> {
        // H2 审计修复: 防御性校验
        frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        pallet::ReferralConfigs::<T>::mutate(entity_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::ReferralConfig::default);
            config.repeat_purchase = pallet::RepeatPurchaseConfig { rate, min_orders };
        });
        pallet::Pallet::<T>::deposit_event(pallet::Event::ReferralConfigUpdated { entity_id });
        Ok(())
    }

    fn clear_config(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        pallet::ReferralConfigs::<T>::remove(entity_id);
        pallet::Pallet::<T>::deposit_event(pallet::Event::ReferralConfigCleared { entity_id });
        Ok(())
    }
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
