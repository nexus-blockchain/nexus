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
    use pallet_entity_common::{EntityProvider, AdminPermission};
    use sp_runtime::traits::{Saturating, Zero};
    use sp_runtime::SaturatedConversion;

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

    /// F1: 推荐人激活条件配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct ReferrerGuardConfig {
        /// 推荐人最低累计消费（u128 匹配 MemberProvider::get_member_stats 返回值，0=无限制）
        pub min_referrer_spent: u128,
        /// 推荐人最低成功订单数（0=无限制）
        pub min_referrer_orders: u32,
    }

    /// F2: 返佣上限配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct CommissionCapConfig<Balance> {
        /// 单笔返佣上限（0=无限制）
        pub max_per_order: Balance,
        /// 推荐人累计返佣上限（0=无限制）
        pub max_total_earned: Balance,
    }

    impl<Balance: Default> Default for CommissionCapConfig<Balance> {
        fn default() -> Self {
            Self { max_per_order: Balance::default(), max_total_earned: Balance::default() }
        }
    }

    /// F5: 推荐关系有效期配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct ReferralValidityConfig {
        /// 推荐关系有效区块数（0=永久有效）
        pub validity_blocks: u32,
        /// 推荐关系有效订单数（0=无限制）
        pub valid_orders: u32,
    }

    /// F10: 配置变更模式（事件粒度增强）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum ReferralConfigMode {
        DirectReward,
        FixedAmount,
        FirstOrder,
        RepeatPurchase,
        ReferrerGuard,
        CommissionCap,
        ReferralValidity,
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
        /// 实体查询接口（权限校验、Owner/Admin 判断）
        type EntityProvider: EntityProvider<Self::AccountId>;
        /// F8: 全局推荐返佣率上限（基点，如 5000 = 50%，10000 = 无限制）
        #[pallet::constant]
        type MaxTotalReferralRate: Get<u16>;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
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

    /// F1: 推荐人激活条件 entity_id -> ReferrerGuardConfig
    #[pallet::storage]
    pub type ReferrerGuardConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        ReferrerGuardConfig,
    >;

    /// F2: 返佣上限配置 entity_id -> CommissionCapConfig
    #[pallet::storage]
    pub type CommissionCapConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        CommissionCapConfig<BalanceOf<T>>,
    >;

    /// F2: 推荐人累计获佣跟踪 (entity_id, referrer) -> total_earned
    #[pallet::storage]
    pub type ReferrerTotalEarned<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        BalanceOf<T>,
        ValueQuery,
    >;

    /// F3: 配置生效时间 entity_id -> block_number
    #[pallet::storage]
    pub type ConfigEffectiveAfter<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        BlockNumberFor<T>,
    >;

    /// F5: 推荐关系有效期配置 entity_id -> ReferralValidityConfig
    #[pallet::storage]
    pub type ReferralValidityConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        ReferralValidityConfig,
    >;

    // ========================================================================
    // Events
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// F10: 配置变更事件（含变更模式）
        ReferralConfigUpdated { entity_id: u64, mode: ReferralConfigMode },
        ReferralConfigCleared { entity_id: u64 },
        /// F3: 配置生效时间设置
        ConfigEffectiveAfterSet { entity_id: u64, block_number: BlockNumberFor<T> },
    }

    // ========================================================================
    // Errors
    // ========================================================================

    #[pallet::error]
    pub enum Error<T> {
        InvalidRate,
        /// 实体不存在
        EntityNotFound,
        /// 非实体所有者或无 COMMISSION_MANAGE 权限
        NotEntityOwnerOrAdmin,
        /// 配置不存在（清除时）
        ConfigNotFound,
        /// 实体已被全局锁定，所有配置操作不可用
        EntityLocked,
        /// F4: 实体未激活（暂停/封禁/关闭时不可修改配置）
        EntityNotActive,
    }

    // ========================================================================
    // Extrinsics (配置设置由各插件自己管理)
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 设置直推奖励配置（Entity Owner 或持有 COMMISSION_MANAGE 权限的 Admin）
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_direct_reward_config(
            origin: OriginFor<T>,
            entity_id: u64,
            rate: u16,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(rate <= 10000, Error::<T>::InvalidRate);

            ReferralConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(ReferralConfig::default);
                config.direct_reward.rate = rate;
            });

            Self::deposit_event(Event::ReferralConfigUpdated { entity_id, mode: ReferralConfigMode::DirectReward });
            Ok(())
        }

        /// 设置固定金额配置（Entity Owner 或持有 COMMISSION_MANAGE 权限的 Admin）
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_fixed_amount_config(
            origin: OriginFor<T>,
            entity_id: u64,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);

            ReferralConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(ReferralConfig::default);
                config.fixed_amount = FixedAmountConfig { amount };
            });

            Self::deposit_event(Event::ReferralConfigUpdated { entity_id, mode: ReferralConfigMode::FixedAmount });
            Ok(())
        }

        /// 设置首单奖励配置（Entity Owner 或持有 COMMISSION_MANAGE 权限的 Admin）
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_first_order_config(
            origin: OriginFor<T>,
            entity_id: u64,
            amount: BalanceOf<T>,
            rate: u16,
            use_amount: bool,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(rate <= 10000, Error::<T>::InvalidRate);

            ReferralConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(ReferralConfig::default);
                config.first_order = FirstOrderConfig { amount, rate, use_amount };
            });

            Self::deposit_event(Event::ReferralConfigUpdated { entity_id, mode: ReferralConfigMode::FirstOrder });
            Ok(())
        }

        /// 设置复购奖励配置（Entity Owner 或持有 COMMISSION_MANAGE 权限的 Admin）
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_repeat_purchase_config(
            origin: OriginFor<T>,
            entity_id: u64,
            rate: u16,
            min_orders: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(rate <= 10000, Error::<T>::InvalidRate);

            ReferralConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(ReferralConfig::default);
                config.repeat_purchase = RepeatPurchaseConfig { rate, min_orders };
            });

            Self::deposit_event(Event::ReferralConfigUpdated { entity_id, mode: ReferralConfigMode::RepeatPurchase });
            Ok(())
        }

        /// 清除推荐链返佣配置（Entity Owner 或持有 COMMISSION_MANAGE 权限的 Admin）
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(35_000_000, 4_000))]
        pub fn clear_referral_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(ReferralConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);

            Self::do_clear_all_config(entity_id);
            Self::deposit_event(Event::ReferralConfigCleared { entity_id });
            Ok(())
        }

        // ===== Root force_* 紧急覆写 extrinsics =====

        /// [Root] 强制设置直推奖励配置
        #[pallet::call_index(6)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn force_set_direct_reward_config(
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

            Self::deposit_event(Event::ReferralConfigUpdated { entity_id, mode: ReferralConfigMode::DirectReward });
            Ok(())
        }

        /// [Root] 强制设置固定金额配置
        #[pallet::call_index(7)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn force_set_fixed_amount_config(
            origin: OriginFor<T>,
            entity_id: u64,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ReferralConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(ReferralConfig::default);
                config.fixed_amount = FixedAmountConfig { amount };
            });

            Self::deposit_event(Event::ReferralConfigUpdated { entity_id, mode: ReferralConfigMode::FixedAmount });
            Ok(())
        }

        /// [Root] 强制设置首单奖励配置
        #[pallet::call_index(8)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn force_set_first_order_config(
            origin: OriginFor<T>,
            entity_id: u64,
            amount: BalanceOf<T>,
            rate: u16,
            use_amount: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(rate <= 10000, Error::<T>::InvalidRate);

            ReferralConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(ReferralConfig::default);
                config.first_order = FirstOrderConfig { amount, rate, use_amount };
            });

            Self::deposit_event(Event::ReferralConfigUpdated { entity_id, mode: ReferralConfigMode::FirstOrder });
            Ok(())
        }

        /// [Root] 强制设置复购奖励配置
        #[pallet::call_index(9)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn force_set_repeat_purchase_config(
            origin: OriginFor<T>,
            entity_id: u64,
            rate: u16,
            min_orders: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(rate <= 10000, Error::<T>::InvalidRate);

            ReferralConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(ReferralConfig::default);
                config.repeat_purchase = RepeatPurchaseConfig { rate, min_orders };
            });

            Self::deposit_event(Event::ReferralConfigUpdated { entity_id, mode: ReferralConfigMode::RepeatPurchase });
            Ok(())
        }

        /// [Root] 强制清除推荐链返佣配置
        #[pallet::call_index(10)]
        #[pallet::weight(Weight::from_parts(35_000_000, 4_000))]
        pub fn force_clear_referral_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            // X2: 仅配置存在时才 remove + emit，防止幻影事件
            if ReferralConfigs::<T>::contains_key(entity_id) {
                Self::do_clear_all_config(entity_id);
                Self::deposit_event(Event::ReferralConfigCleared { entity_id });
            }
            Ok(())
        }

        // ===== F1/F2/F3/F5 新增 extrinsics =====

        /// F1: 设置推荐人激活条件
        #[pallet::call_index(11)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_referrer_guard_config(
            origin: OriginFor<T>,
            entity_id: u64,
            min_referrer_spent: u128,
            min_referrer_orders: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);

            ReferrerGuardConfigs::<T>::insert(entity_id, ReferrerGuardConfig {
                min_referrer_spent,
                min_referrer_orders,
            });

            Self::deposit_event(Event::ReferralConfigUpdated { entity_id, mode: ReferralConfigMode::ReferrerGuard });
            Ok(())
        }

        /// F2: 设置返佣上限配置
        #[pallet::call_index(12)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_commission_cap_config(
            origin: OriginFor<T>,
            entity_id: u64,
            max_per_order: BalanceOf<T>,
            max_total_earned: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);

            CommissionCapConfigs::<T>::insert(entity_id, CommissionCapConfig {
                max_per_order,
                max_total_earned,
            });

            Self::deposit_event(Event::ReferralConfigUpdated { entity_id, mode: ReferralConfigMode::CommissionCap });
            Ok(())
        }

        /// F5: 设置推荐关系有效期配置
        #[pallet::call_index(13)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_referral_validity_config(
            origin: OriginFor<T>,
            entity_id: u64,
            validity_blocks: u32,
            valid_orders: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);

            ReferralValidityConfigs::<T>::insert(entity_id, ReferralValidityConfig {
                validity_blocks,
                valid_orders,
            });

            Self::deposit_event(Event::ReferralConfigUpdated { entity_id, mode: ReferralConfigMode::ReferralValidity });
            Ok(())
        }

        /// F3: 设置配置生效时间
        #[pallet::call_index(14)]
        #[pallet::weight(Weight::from_parts(35_000_000, 4_000))]
        pub fn set_config_effective_after(
            origin: OriginFor<T>,
            entity_id: u64,
            block_number: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);

            ConfigEffectiveAfter::<T>::insert(entity_id, block_number);
            Self::deposit_event(Event::ConfigEffectiveAfterSet { entity_id, block_number });
            Ok(())
        }
    }

    // ========================================================================
    // F9: integrity_test
    // ========================================================================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        #[cfg(feature = "std")]
        fn integrity_test() {
            let max_rate = T::MaxTotalReferralRate::get();
            assert!(
                max_rate <= 10000,
                "MaxTotalReferralRate must be <= 10000 (100%), got {}",
                max_rate,
            );
        }
    }

    // ========================================================================
    // Internal calculation functions
    // ========================================================================

    impl<T: Config> Pallet<T> {
        /// H1: 清除所有关联配置（主配置 + 附属存储）
        /// M1-R4: 同时清除 ReferrerTotalEarned，防止旧计划期累计值影响新 CommissionCapConfig
        pub(crate) fn do_clear_all_config(entity_id: u64) {
            ReferralConfigs::<T>::remove(entity_id);
            ReferrerGuardConfigs::<T>::remove(entity_id);
            CommissionCapConfigs::<T>::remove(entity_id);
            ReferralValidityConfigs::<T>::remove(entity_id);
            ConfigEffectiveAfter::<T>::remove(entity_id);
            let _ = ReferrerTotalEarned::<T>::clear_prefix(entity_id, u32::MAX, None);
        }

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

        /// F1/F5/F6: 验证推荐人资格（封禁/冻结/激活条件/有效期）
        pub(crate) fn is_referrer_qualified(
            entity_id: u64,
            buyer: &T::AccountId,
            referrer: &T::AccountId,
        ) -> bool {
            // M1 审计修复: 非会员推荐人不获佣（移除后推荐链未清理时防漏发）
            if !T::MemberProvider::is_member(entity_id, referrer) { return false; }
            // X1: 跳过被封禁或未激活的推荐人
            if T::MemberProvider::is_banned(entity_id, referrer) { return false; }
            if !T::MemberProvider::is_activated(entity_id, referrer) { return false; }
            // F6: 跳过冻结/暂停的推荐人
            if !T::MemberProvider::is_member_active(entity_id, referrer) { return false; }
            // F1: 推荐人激活条件检查
            if let Some(guard) = ReferrerGuardConfigs::<T>::get(entity_id) {
                if guard.min_referrer_spent > 0 {
                    let (_, _, spent) = T::MemberProvider::get_member_stats(entity_id, referrer);
                    if spent < guard.min_referrer_spent { return false; }
                }
                if guard.min_referrer_orders > 0 {
                    let orders = T::MemberProvider::completed_order_count(entity_id, referrer);
                    if orders < guard.min_referrer_orders { return false; }
                }
            }
            // F5: 推荐关系有效期检查
            if let Some(validity) = ReferralValidityConfigs::<T>::get(entity_id) {
                if validity.validity_blocks > 0 {
                    let registered_at = T::MemberProvider::referral_registered_at(entity_id, buyer);
                    if registered_at > 0 {
                        let now: u64 = frame_system::Pallet::<T>::block_number().saturated_into();
                        if now > registered_at.saturating_add(validity.validity_blocks as u64) {
                            return false;
                        }
                    }
                }
                if validity.valid_orders > 0 {
                    let buyer_completed = T::MemberProvider::completed_order_count(entity_id, buyer);
                    if buyer_completed >= validity.valid_orders { return false; }
                }
            }
            true
        }

        /// F2: 应用单笔和累计返佣上限
        fn apply_commission_cap(
            entity_id: u64,
            referrer: &T::AccountId,
            mut amount: BalanceOf<T>,
        ) -> BalanceOf<T> {
            if let Some(cap) = CommissionCapConfigs::<T>::get(entity_id) {
                // F2a: 单笔上限
                if !cap.max_per_order.is_zero() {
                    amount = amount.min(cap.max_per_order);
                }
                // F2b: 累计上限
                if !cap.max_total_earned.is_zero() {
                    let earned = ReferrerTotalEarned::<T>::get(entity_id, referrer);
                    let space = cap.max_total_earned.saturating_sub(earned);
                    amount = amount.min(space);
                }
            }
            amount
        }

        /// F2: 更新推荐人累计获佣
        fn track_referrer_earned(
            entity_id: u64,
            referrer: &T::AccountId,
            amount: BalanceOf<T>,
        ) {
            if !amount.is_zero() {
                ReferrerTotalEarned::<T>::mutate(entity_id, referrer, |earned| {
                    *earned = earned.saturating_add(amount);
                });
            }
        }

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
                if !Self::is_referrer_qualified(entity_id, buyer, &referrer) { return; }
                let commission = order_amount.saturating_mul(config.rate.into()) / 10000u32.into();
                let capped = Self::apply_commission_cap(entity_id, &referrer, commission);
                let actual = capped.min(*remaining);
                if !actual.is_zero() {
                    *remaining = remaining.saturating_sub(actual);
                    Self::track_referrer_earned(entity_id, &referrer, actual);
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
                if !Self::is_referrer_qualified(entity_id, buyer, &referrer) { return; }
                let capped = Self::apply_commission_cap(entity_id, &referrer, config.amount);
                let actual = capped.min(*remaining);
                if !actual.is_zero() {
                    *remaining = remaining.saturating_sub(actual);
                    Self::track_referrer_earned(entity_id, &referrer, actual);
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
                if !Self::is_referrer_qualified(entity_id, buyer, &referrer) { return; }
                let commission = if config.use_amount {
                    config.amount
                } else {
                    order_amount.saturating_mul(config.rate.into()) / 10000u32.into()
                };
                let capped = Self::apply_commission_cap(entity_id, &referrer, commission);
                let actual = capped.min(*remaining);
                if !actual.is_zero() {
                    *remaining = remaining.saturating_sub(actual);
                    Self::track_referrer_earned(entity_id, &referrer, actual);
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
                if !Self::is_referrer_qualified(entity_id, buyer, &referrer) { return; }
                let commission = order_amount.saturating_mul(config.rate.into()) / 10000u32.into();
                let capped = Self::apply_commission_cap(entity_id, &referrer, commission);
                let actual = capped.min(*remaining);
                if !actual.is_zero() {
                    *remaining = remaining.saturating_sub(actual);
                    Self::track_referrer_earned(entity_id, &referrer, actual);
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
        _is_first_order: bool,
        buyer_order_count: u32,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, pallet::BalanceOf<T>>>, pallet::BalanceOf<T>) {
        use pallet_commission_common::CommissionModes;
        use sp_runtime::traits::{Zero, Saturating};
        use frame_support::traits::Get;

        // F3: 配置生效时间检查
        if let Some(effective_after) = pallet::ConfigEffectiveAfter::<T>::get(entity_id) {
            let now = frame_system::Pallet::<T>::block_number();
            if now < effective_after {
                return (alloc::vec::Vec::new(), remaining);
            }
        }

        let config = match pallet::ReferralConfigs::<T>::get(entity_id) {
            Some(c) => c,
            None => return (alloc::vec::Vec::new(), remaining),
        };

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        // F7: 使用已完成订单数判断首单，取消/退款订单不影响首单判定
        let is_first_order = T::MemberProvider::completed_order_count(entity_id, buyer) == 0;

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

        // F8: 全局返佣率上限
        let max_rate: u16 = T::MaxTotalReferralRate::get();
        if max_rate < 10000u16 && !outputs.is_empty() {
            let max_amount = order_amount.saturating_mul((max_rate as u32).into()) / 10000u32.into();
            let mut total_used = pallet::BalanceOf::<T>::zero();
            for output in outputs.iter_mut() {
                let space = max_amount.saturating_sub(total_used);
                if output.amount > space {
                    let excess = output.amount.saturating_sub(space);
                    remaining = remaining.saturating_add(excess);
                    // M2 审计修复: F8 裁剪后修正 ReferrerTotalEarned，防止累计上限(F2b)虚高
                    pallet::ReferrerTotalEarned::<T>::mutate(entity_id, &output.beneficiary, |earned| {
                        *earned = earned.saturating_sub(excess);
                    });
                    output.amount = space;
                }
                total_used = total_used.saturating_add(output.amount);
            }
            outputs.retain(|o| !o.amount.is_zero());
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
            if !Self::is_referrer_qualified(entity_id, buyer, &referrer) { return; }
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
            if !Self::is_referrer_qualified(entity_id, buyer, &referrer) { return; }
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
            if !Self::is_referrer_qualified(entity_id, buyer, &referrer) { return; }
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
        _is_first_order: bool,
        buyer_order_count: u32,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>, TB) {
        use pallet_commission_common::CommissionModes;
        use frame_support::traits::Get;

        // F3: 配置生效时间检查
        if let Some(effective_after) = pallet::ConfigEffectiveAfter::<T>::get(entity_id) {
            let now = frame_system::Pallet::<T>::block_number();
            if now < effective_after {
                return (alloc::vec::Vec::new(), remaining);
            }
        }

        let config = match pallet::ReferralConfigs::<T>::get(entity_id) {
            Some(c) => c,
            None => return (alloc::vec::Vec::new(), remaining),
        };

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        // F7: 使用已完成订单数判断首单
        let is_first_order = T::MemberProvider::completed_order_count(entity_id, buyer) == 0;

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

        // F8: 全局返佣率上限
        let max_rate: u16 = T::MaxTotalReferralRate::get();
        if max_rate < 10000u16 && !outputs.is_empty() {
            let max_amount = order_amount.saturating_mul(TB::from(max_rate as u32)) / TB::from(10000u32);
            let mut total_used = TB::zero();
            for output in outputs.iter_mut() {
                let space = max_amount.saturating_sub(total_used);
                if output.amount > space {
                    remaining = remaining.saturating_add(output.amount.saturating_sub(space));
                    output.amount = space;
                }
                total_used = total_used.saturating_add(output.amount);
            }
            outputs.retain(|o| !o.amount.is_zero());
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
        pallet::Pallet::<T>::deposit_event(pallet::Event::ReferralConfigUpdated { entity_id, mode: pallet::ReferralConfigMode::DirectReward });
        Ok(())
    }

    fn set_fixed_amount(entity_id: u64, amount: pallet::BalanceOf<T>) -> Result<(), sp_runtime::DispatchError> {
        pallet::ReferralConfigs::<T>::mutate(entity_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::ReferralConfig::default);
            config.fixed_amount = pallet::FixedAmountConfig { amount };
        });
        pallet::Pallet::<T>::deposit_event(pallet::Event::ReferralConfigUpdated { entity_id, mode: pallet::ReferralConfigMode::FixedAmount });
        Ok(())
    }

    fn set_first_order(entity_id: u64, amount: pallet::BalanceOf<T>, rate: u16, use_amount: bool) -> Result<(), sp_runtime::DispatchError> {
        // H2 审计修复: 防御性校验
        frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        pallet::ReferralConfigs::<T>::mutate(entity_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::ReferralConfig::default);
            config.first_order = pallet::FirstOrderConfig { amount, rate, use_amount };
        });
        pallet::Pallet::<T>::deposit_event(pallet::Event::ReferralConfigUpdated { entity_id, mode: pallet::ReferralConfigMode::FirstOrder });
        Ok(())
    }

    fn set_repeat_purchase(entity_id: u64, rate: u16, min_orders: u32) -> Result<(), sp_runtime::DispatchError> {
        // H2 审计修复: 防御性校验
        frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        pallet::ReferralConfigs::<T>::mutate(entity_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::ReferralConfig::default);
            config.repeat_purchase = pallet::RepeatPurchaseConfig { rate, min_orders };
        });
        pallet::Pallet::<T>::deposit_event(pallet::Event::ReferralConfigUpdated { entity_id, mode: pallet::ReferralConfigMode::RepeatPurchase });
        Ok(())
    }

    fn clear_config(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        // X2: 仅配置存在时才 remove + emit，防止幻影事件误导 indexer
        // H1: 清除主配置 + 全部附属存储
        if pallet::ReferralConfigs::<T>::contains_key(entity_id) {
            pallet::Pallet::<T>::do_clear_all_config(entity_id);
            pallet::Pallet::<T>::deposit_event(pallet::Event::ReferralConfigCleared { entity_id });
        }
        Ok(())
    }
}

// ============================================================================
// ReferralQueryProvider 实现
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::ReferralQueryProvider<T::AccountId, BalanceOf<T>> for pallet::Pallet<T> {
    fn referrer_total_earned(entity_id: u64, account: &T::AccountId) -> BalanceOf<T> {
        pallet::ReferrerTotalEarned::<T>::get(entity_id, account)
    }

    fn cap_config(entity_id: u64) -> Option<(BalanceOf<T>, BalanceOf<T>)> {
        pallet::CommissionCapConfigs::<T>::get(entity_id)
            .map(|c| (c.max_per_order, c.max_total_earned))
    }
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
