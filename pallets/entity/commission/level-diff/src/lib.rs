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
                    custom_config.as_ref()
                        .and_then(|c| c.level_rates.get(level_id as usize).copied())
                        .unwrap_or(0)
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
