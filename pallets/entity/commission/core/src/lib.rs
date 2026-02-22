//! # Commission Core (pallet-commission-core)
//!
//! 返佣系统核心调度引擎。负责：
//! - 全局返佣配置（启用模式、来源、上限）
//! - 返佣记账（credit_commission）与取消（cancel_commission）
//! - 提现系统（分级提现 + 购物余额）
//! - 偿付安全（ShopPendingTotal + ShopShoppingTotal）
//! - 调度各插件（ReferralPlugin / LevelDiffPlugin / SingleLinePlugin / TeamPlugin）

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;
pub use pallet_commission_common::{
    CommissionModes, CommissionOutput, CommissionPlugin, CommissionPlan, CommissionProvider,
    CommissionRecord, CommissionSource, CommissionStatus, CommissionType,
    LevelDiffPlanWriter, MemberCommissionStatsData, MemberProvider,
    ReferralPlanWriter, WithdrawalMode, WithdrawalTierConfig,
};
use pallet_entity_common::ShopProvider as ShopProviderT;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, Get},
    };
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{EntityProvider, ShopProvider};
    use pallet_commission_common::{CommissionPlan, ReferralPlanWriter, LevelDiffPlanWriter};
    use sp_runtime::traits::{Saturating, Zero};

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    pub type CommissionRecordOf<T> = CommissionRecord<
        <T as frame_system::Config>::AccountId,
        BalanceOf<T>,
        BlockNumberFor<T>,
    >;

    pub type MemberCommissionStatsOf<T> = MemberCommissionStatsData<BalanceOf<T>>;

    /// 全局返佣开关配置（per-shop）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct CoreCommissionConfig {
        /// 启用的返佣模式（位标志）
        pub enabled_modes: CommissionModes,
        /// 返佣来源（预留）
        pub source: CommissionSource,
        /// 返佣上限比例（基点，10000 = 100%）
        pub max_commission_rate: u16,
        /// 是否全局启用
        pub enabled: bool,
        /// 提现冻结期（区块数，0 = 无冻结）
        pub withdrawal_cooldown: u32,
    }

    impl Default for CoreCommissionConfig {
        fn default() -> Self {
            Self {
                enabled_modes: CommissionModes::default(),
                source: CommissionSource::default(),
                max_commission_rate: 10000,
                enabled: false,
                withdrawal_cooldown: 0,
            }
        }
    }

    /// 实体提现配置（四种模式 + 自愿复购奖励）
    ///
    /// 模式：
    /// - `FullWithdrawal`: 不强制复购，Governance 底线仍生效
    /// - `FixedRate`: 所有会员统一复购比率
    /// - `LevelBased`: 按 level_id 查 default_tier / level_overrides
    /// - `MemberChoice`: 会员提现时自选比率，不低于 min_repurchase_rate
    ///
    /// `voluntary_bonus_rate`: 自愿多复购的奖励加成（万分比），
    /// 超出强制最低线的部分 × bonus_rate 额外计入购物余额。
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxLevels))]
    pub struct EntityWithdrawalConfig<MaxLevels: Get<u32>> {
        /// 提现模式
        pub mode: WithdrawalMode,
        /// LevelBased 模式下的默认提现/复购比例
        pub default_tier: WithdrawalTierConfig,
        /// LevelBased 模式下按 level_id 覆写的配置
        pub level_overrides: BoundedVec<(u8, WithdrawalTierConfig), MaxLevels>,
        /// 自愿多复购奖励加成（万分比，如 500 = 5%）
        pub voluntary_bonus_rate: u16,
        pub enabled: bool,
        pub shopping_balance_generates_commission: bool,
    }

    impl<MaxLevels: Get<u32>> Default for EntityWithdrawalConfig<MaxLevels> {
        fn default() -> Self {
            Self {
                mode: WithdrawalMode::default(),
                default_tier: WithdrawalTierConfig::default(),
                level_overrides: BoundedVec::default(),
                voluntary_bonus_rate: 0,
                enabled: false,
                shopping_balance_generates_commission: false,
            }
        }
    }

    pub type EntityWithdrawalConfigOf<T> = EntityWithdrawalConfig<<T as Config>::MaxCustomLevels>;

    /// 提现分配结果（内部使用）
    pub(crate) struct WithdrawalSplit<Balance> {
        pub withdrawal: Balance,
        pub repurchase: Balance,
        pub bonus: Balance,
    }

    // ========================================================================
    // Config
    // ========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: Currency<Self::AccountId>;

        /// Shop 查询接口
        type ShopProvider: ShopProvider<Self::AccountId>;

        /// Entity 查询接口
        type EntityProvider: EntityProvider<Self::AccountId>;

        /// 会员查询接口
        type MemberProvider: MemberProvider<Self::AccountId>;

        /// 推荐链返佣插件
        type ReferralPlugin: CommissionPlugin<Self::AccountId, BalanceOf<Self>>;

        /// 等级极差返佣插件
        type LevelDiffPlugin: CommissionPlugin<Self::AccountId, BalanceOf<Self>>;

        /// 单线收益插件
        type SingleLinePlugin: CommissionPlugin<Self::AccountId, BalanceOf<Self>>;

        /// 团队业绩插件（预留）
        type TeamPlugin: CommissionPlugin<Self::AccountId, BalanceOf<Self>>;

        /// 推荐链方案写入器
        type ReferralWriter: ReferralPlanWriter<BalanceOf<Self>>;

        /// 等级极差方案写入器
        type LevelDiffWriter: LevelDiffPlanWriter;

        /// 最大返佣记录数（每订单）
        #[pallet::constant]
        type MaxCommissionRecordsPerOrder: Get<u32>;

        /// 最大自定义等级数
        #[pallet::constant]
        type MaxCustomLevels: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ========================================================================
    // Storage
    // ========================================================================

    /// Entity 返佣核心配置 entity_id -> CoreCommissionConfig
    #[pallet::storage]
    #[pallet::getter(fn commission_config)]
    pub type CommissionConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        CoreCommissionConfig,
    >;

    /// 会员返佣统计 (entity_id, account) -> MemberCommissionStatsData
    #[pallet::storage]
    #[pallet::getter(fn member_commission_stats)]
    pub type MemberCommissionStats<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        MemberCommissionStatsOf<T>,
        ValueQuery,
    >;

    /// 订单返佣记录 order_id -> Vec<CommissionRecord>
    #[pallet::storage]
    #[pallet::getter(fn order_commission_records)]
    pub type OrderCommissionRecords<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        BoundedVec<CommissionRecordOf<T>, T::MaxCommissionRecordsPerOrder>,
        ValueQuery,
    >;

    /// Entity 返佣统计 entity_id -> (total_distributed, total_orders)
    #[pallet::storage]
    #[pallet::getter(fn shop_commission_totals)]
    pub type ShopCommissionTotals<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        (BalanceOf<T>, u64),
        ValueQuery,
    >;

    /// Entity 待提取佣金总额 entity_id -> Balance
    #[pallet::storage]
    #[pallet::getter(fn shop_pending_total)]
    pub type ShopPendingTotal<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        BalanceOf<T>,
        ValueQuery,
    >;

    /// Entity 购物余额总额 entity_id -> Balance（资金锁定）
    #[pallet::storage]
    #[pallet::getter(fn shop_shopping_total)]
    pub type ShopShoppingTotal<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        BalanceOf<T>,
        ValueQuery,
    >;

    /// 提现配置 entity_id -> EntityWithdrawalConfig
    #[pallet::storage]
    #[pallet::getter(fn withdrawal_config)]
    pub type WithdrawalConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        EntityWithdrawalConfigOf<T>,
    >;

    /// 会员购物余额 (entity_id, account) -> Balance
    #[pallet::storage]
    #[pallet::getter(fn member_shopping_balance)]
    pub type MemberShoppingBalance<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        BalanceOf<T>,
        ValueQuery,
    >;

    /// 会员最后入账区块 (entity_id, account) -> BlockNumber（用于冻结期检查）
    #[pallet::storage]
    pub type MemberLastCredited<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        BlockNumberFor<T>,
        ValueQuery,
    >;

    /// 全局最低复购比例 entity_id -> u16（万分比，由 Governance 设定）
    /// 提现时实际复购比例 = max(entity 分层配置, 此底线)
    #[pallet::storage]
    #[pallet::getter(fn global_min_repurchase_rate)]
    pub type GlobalMinRepurchaseRate<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        u16,
        ValueQuery,
    >;

    // ========================================================================
    // Events
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        CommissionConfigUpdated { entity_id: u64 },
        CommissionModesUpdated { entity_id: u64, modes: CommissionModes },
        CommissionDistributed {
            entity_id: u64,
            shop_id: u64,
            order_id: u64,
            beneficiary: T::AccountId,
            amount: BalanceOf<T>,
            commission_type: CommissionType,
            level: u8,
        },
        CommissionWithdrawn {
            entity_id: u64,
            account: T::AccountId,
            amount: BalanceOf<T>,
        },
        CommissionCancelled { order_id: u64 },
        CommissionPlanInitialized { entity_id: u64, plan: CommissionPlan },
        WithdrawalCooldownNotMet { entity_id: u64, account: T::AccountId, earliest_block: BlockNumberFor<T> },
        TieredWithdrawal {
            entity_id: u64,
            account: T::AccountId,
            withdrawn_amount: BalanceOf<T>,
            repurchase_amount: BalanceOf<T>,
            bonus_amount: BalanceOf<T>,
        },
        WithdrawalConfigUpdated { entity_id: u64 },
        ShoppingBalanceUsed {
            entity_id: u64,
            account: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// 佣金资金从 Shop 转入 Entity 账户
        CommissionFundsTransferred {
            entity_id: u64,
            shop_id: u64,
            amount: BalanceOf<T>,
        },
    }

    // ========================================================================
    // Errors
    // ========================================================================

    #[pallet::error]
    pub enum Error<T> {
        ShopNotFound,
        EntityNotFound,
        NotShopOwner,
        NotEntityOwner,
        CommissionNotConfigured,
        InsufficientCommission,
        InvalidCommissionRate,
        RecordsFull,
        Overflow,
        WithdrawalConfigNotEnabled,
        InvalidWithdrawalConfig,
        InsufficientShoppingBalance,
        /// 提现冻结期未满足
        WithdrawalCooldownNotMet,
        /// 复购目标账户不是出资人的直推下线
        NotDirectReferral,
        /// 自动注册会员失败
        AutoRegisterFailed,
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 设置启用的返佣模式（Entity 级）
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_commission_modes(
            origin: OriginFor<T>,
            shop_id: u64,
            modes: CommissionModes,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_entity_owner_via_shop(shop_id, &who)?;

            CommissionConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(CoreCommissionConfig::default);
                config.enabled_modes = modes;
            });

            Self::deposit_event(Event::CommissionModesUpdated { entity_id, modes });
            Ok(())
        }

        /// 设置返佣来源和上限（Entity 级）
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_commission_source(
            origin: OriginFor<T>,
            shop_id: u64,
            source: CommissionSource,
            max_rate: u16,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_entity_owner_via_shop(shop_id, &who)?;
            ensure!(max_rate <= 10000, Error::<T>::InvalidCommissionRate);

            CommissionConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(CoreCommissionConfig::default);
                config.source = source;
                config.max_commission_rate = max_rate;
            });

            Self::deposit_event(Event::CommissionConfigUpdated { entity_id });
            Ok(())
        }

        /// 启用/禁用返佣（Entity 级）
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(35_000_000, 4_000))]
        pub fn enable_commission(
            origin: OriginFor<T>,
            shop_id: u64,
            enabled: bool,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_entity_owner_via_shop(shop_id, &who)?;

            CommissionConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(CoreCommissionConfig::default);
                config.enabled = enabled;
            });

            Self::deposit_event(Event::CommissionConfigUpdated { entity_id });
            Ok(())
        }

        /// 提取返佣（四种提现模式 + 自愿复购奖励 + 指定复购目标，Entity 级佣金池）
        ///
        /// - `shop_id`: 任意关联 Shop（用于定位 Entity 和查询会员等级）
        /// - `amount`: 提现金额（None = 全部 pending）
        /// - `requested_repurchase_rate`: 会员请求的复购比率（万分比，MemberChoice 模式下使用）
        /// - `repurchase_target`: 复购购物余额的接收账户（None = 自己）
        ///   - 目标为非会员：自动注册，推荐人 = 出资人
        ///   - 目标为已有会员：推荐人必须是出资人，否则拒绝
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(80_000_000, 6_000))]
        pub fn withdraw_commission(
            origin: OriginFor<T>,
            shop_id: u64,
            amount: Option<BalanceOf<T>>,
            requested_repurchase_rate: Option<u16>,
            repurchase_target: Option<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::resolve_entity_id(shop_id)?;

            // 确定复购目标账户
            let target = repurchase_target.unwrap_or_else(|| who.clone());

            // 如果目标不是自己，校验推荐关系
            if target != who {
                if T::MemberProvider::is_member(shop_id, &target) {
                    // 已是会员 → 推荐人必须是出资人
                    let referrer = T::MemberProvider::get_referrer(shop_id, &target);
                    ensure!(referrer.as_ref() == Some(&who), Error::<T>::NotDirectReferral);
                } else {
                    // 非会员 → 自动注册，推荐人 = 出资人
                    T::MemberProvider::auto_register(shop_id, &target, Some(who.clone()))
                        .map_err(|_| Error::<T>::AutoRegisterFailed)?;
                }
            }

            MemberCommissionStats::<T>::try_mutate(entity_id, &who, |stats| -> DispatchResult {
                let total_amount = amount.unwrap_or(stats.pending);
                ensure!(stats.pending >= total_amount, Error::<T>::InsufficientCommission);

                // 冻结期检查
                if let Some(config) = CommissionConfigs::<T>::get(entity_id) {
                    if config.withdrawal_cooldown > 0 {
                        let now = <frame_system::Pallet<T>>::block_number();
                        let last_credited = MemberLastCredited::<T>::get(entity_id, &who);
                        let cooldown: BlockNumberFor<T> = config.withdrawal_cooldown.into();
                        let earliest = last_credited.saturating_add(cooldown);
                        ensure!(now >= earliest, Error::<T>::WithdrawalCooldownNotMet);
                    }
                }

                // 计算提现/复购/奖励分配
                let split = Self::calc_withdrawal_split(
                    entity_id, shop_id, &who, total_amount, requested_repurchase_rate,
                );

                // 从 Entity 账户转账提现部分到用户钱包
                if !split.withdrawal.is_zero() {
                    let entity_account = T::EntityProvider::entity_account(entity_id);
                    let entity_balance = T::Currency::free_balance(&entity_account);
                    // 偿付安全：确保提现后仍能覆盖其余已承诺资金
                    let remaining_pending = ShopPendingTotal::<T>::get(entity_id)
                        .saturating_sub(total_amount);
                    let shopping_total = ShopShoppingTotal::<T>::get(entity_id);
                    let required_reserve = remaining_pending.saturating_add(shopping_total);
                    ensure!(
                        entity_balance >= split.withdrawal.saturating_add(required_reserve),
                        Error::<T>::InsufficientCommission
                    );

                    T::Currency::transfer(
                        &entity_account,
                        &who,
                        split.withdrawal,
                        ExistenceRequirement::KeepAlive,
                    )?;
                }

                // 复购部分 + 奖励 转入目标账户的购物余额
                let total_to_shopping = split.repurchase.saturating_add(split.bonus);
                if !total_to_shopping.is_zero() {
                    MemberShoppingBalance::<T>::mutate(entity_id, &target, |balance| {
                        *balance = balance.saturating_add(total_to_shopping);
                    });
                    ShopShoppingTotal::<T>::mutate(entity_id, |total| {
                        *total = total.saturating_add(total_to_shopping);
                    });
                }

                // 统计记在出资人名下
                stats.pending = stats.pending.saturating_sub(total_amount);
                stats.withdrawn = stats.withdrawn.saturating_add(split.withdrawal);
                stats.repurchased = stats.repurchased.saturating_add(split.repurchase);

                // 释放 pending 锁定
                ShopPendingTotal::<T>::mutate(entity_id, |total| {
                    *total = total.saturating_sub(total_amount);
                });

                // 发出事件
                Self::deposit_event(Event::TieredWithdrawal {
                    entity_id,
                    account: who.clone(),
                    withdrawn_amount: split.withdrawal,
                    repurchase_amount: split.repurchase,
                    bonus_amount: split.bonus,
                });

                Ok(())
            })
        }

        /// 设置提现配置（Entity 级，四种模式 + 自愿复购奖励）
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(50_000_000, 5_000))]
        pub fn set_withdrawal_config(
            origin: OriginFor<T>,
            shop_id: u64,
            mode: WithdrawalMode,
            default_tier: WithdrawalTierConfig,
            level_overrides: BoundedVec<(u8, WithdrawalTierConfig), T::MaxCustomLevels>,
            voluntary_bonus_rate: u16,
            enabled: bool,
            shopping_balance_generates_commission: bool,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_entity_owner_via_shop(shop_id, &who)?;

            // 校验模式参数
            match &mode {
                WithdrawalMode::FixedRate { repurchase_rate } => {
                    ensure!(*repurchase_rate <= 10000, Error::<T>::InvalidWithdrawalConfig);
                },
                WithdrawalMode::MemberChoice { min_repurchase_rate } => {
                    ensure!(*min_repurchase_rate <= 10000, Error::<T>::InvalidWithdrawalConfig);
                },
                _ => {},
            }

            // 校验 LevelBased 配置（即使非 LevelBased 模式也允许预配置）
            ensure!(
                default_tier.withdrawal_rate.saturating_add(default_tier.repurchase_rate) == 10000,
                Error::<T>::InvalidWithdrawalConfig
            );
            for (_, tier) in level_overrides.iter() {
                ensure!(
                    tier.withdrawal_rate.saturating_add(tier.repurchase_rate) == 10000,
                    Error::<T>::InvalidWithdrawalConfig
                );
            }

            // 校验 bonus rate
            ensure!(voluntary_bonus_rate <= 10000, Error::<T>::InvalidWithdrawalConfig);

            WithdrawalConfigs::<T>::insert(entity_id, EntityWithdrawalConfig {
                mode,
                default_tier,
                level_overrides,
                voluntary_bonus_rate,
                enabled,
                shopping_balance_generates_commission,
            });

            Self::deposit_event(Event::WithdrawalConfigUpdated { entity_id });
            Ok(())
        }

        /// 一键初始化佣金方案（Entity 级）
        #[pallet::call_index(6)]
        #[pallet::weight(Weight::from_parts(60_000_000, 5_000))]
        pub fn init_commission_plan(
            origin: OriginFor<T>,
            shop_id: u64,
            plan: CommissionPlan,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_entity_owner_via_shop(shop_id, &who)?;

            // 先清除旧配置（entity_id 作为 key）
            T::ReferralWriter::clear_config(entity_id)?;
            T::LevelDiffWriter::clear_config(entity_id)?;

            match plan {
                CommissionPlan::None => {
                    CommissionConfigs::<T>::insert(entity_id, CoreCommissionConfig {
                        enabled_modes: CommissionModes(CommissionModes::NONE),
                        source: CommissionSource::default(),
                        max_commission_rate: 0,
                        enabled: false,
                        withdrawal_cooldown: 0,
                    });
                }
                CommissionPlan::DirectOnly { rate } => {
                    ensure!(rate <= 10000, Error::<T>::InvalidCommissionRate);
                    CommissionConfigs::<T>::insert(entity_id, CoreCommissionConfig {
                        enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD),
                        source: CommissionSource::default(),
                        max_commission_rate: rate,
                        enabled: true,
                        withdrawal_cooldown: 0,
                    });
                    T::ReferralWriter::set_direct_rate(entity_id, rate)?;
                }
                CommissionPlan::MultiLevel { levels, base_rate } => {
                    ensure!(base_rate <= 10000, Error::<T>::InvalidCommissionRate);
                    ensure!(levels > 0 && levels <= 15, Error::<T>::InvalidCommissionRate);

                    let mut level_rates = alloc::vec::Vec::new();
                    let mut current_rate = base_rate;
                    let mut total_rate: u16 = 0;
                    for _ in 0..levels {
                        level_rates.push(current_rate);
                        total_rate = total_rate.saturating_add(current_rate);
                        current_rate = current_rate * 80 / 100;
                    }

                    CommissionConfigs::<T>::insert(entity_id, CoreCommissionConfig {
                        enabled_modes: CommissionModes(
                            CommissionModes::DIRECT_REWARD | CommissionModes::MULTI_LEVEL
                        ),
                        source: CommissionSource::default(),
                        max_commission_rate: total_rate.min(10000),
                        enabled: true,
                        withdrawal_cooldown: 0,
                    });
                    T::ReferralWriter::set_direct_rate(entity_id, base_rate)?;
                    T::ReferralWriter::set_multi_level(entity_id, level_rates, total_rate.min(10000))?;
                }
                CommissionPlan::LevelDiff { normal, silver, gold, platinum, diamond } => {
                    ensure!(diamond <= 10000, Error::<T>::InvalidCommissionRate);
                    CommissionConfigs::<T>::insert(entity_id, CoreCommissionConfig {
                        enabled_modes: CommissionModes(
                            CommissionModes::DIRECT_REWARD | CommissionModes::LEVEL_DIFF
                        ),
                        source: CommissionSource::default(),
                        max_commission_rate: diamond,
                        enabled: true,
                        withdrawal_cooldown: 0,
                    });
                    T::LevelDiffWriter::set_global_rates(entity_id, normal, silver, gold, platinum, diamond)?;
                }
                CommissionPlan::Custom => {
                    CommissionConfigs::<T>::insert(entity_id, CoreCommissionConfig {
                        enabled_modes: CommissionModes(CommissionModes::NONE),
                        source: CommissionSource::default(),
                        max_commission_rate: 10000,
                        enabled: true,
                        withdrawal_cooldown: 0,
                    });
                }
            }

            Self::deposit_event(Event::CommissionPlanInitialized { entity_id, plan });
            Ok(())
        }

        /// 使用购物余额支付（需 Entity owner 授权，由订单流程触发）
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn use_shopping_balance(
            origin: OriginFor<T>,
            shop_id: u64,
            account: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_entity_owner_via_shop(shop_id, &who)?;
            Self::do_use_shopping_balance(entity_id, &account, amount)
        }
    }

    // ========================================================================
    // Internal functions
    // ========================================================================

    impl<T: Config> Pallet<T> {
        /// 从 shop_id 解析 entity_id
        fn resolve_entity_id(shop_id: u64) -> Result<u64, Error<T>> {
            T::ShopProvider::shop_entity_id(shop_id).ok_or(Error::<T>::ShopNotFound)
        }

        /// 验证 Entity 所有者权限（通过 shop_id 解析）
        fn ensure_entity_owner_via_shop(shop_id: u64, who: &T::AccountId) -> Result<u64, DispatchError> {
            let entity_id = Self::resolve_entity_id(shop_id)?;
            let owner = T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            ensure!(*who == owner, Error::<T>::NotEntityOwner);
            Ok(entity_id)
        }

        /// 使用购物余额内部实现（entity_id 级，供 extrinsic 和 CommissionProvider 调用）
        pub fn do_use_shopping_balance(
            entity_id: u64,
            account: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            MemberShoppingBalance::<T>::try_mutate(entity_id, account, |balance| -> DispatchResult {
                ensure!(*balance >= amount, Error::<T>::InsufficientShoppingBalance);
                *balance = balance.saturating_sub(amount);

                ShopShoppingTotal::<T>::mutate(entity_id, |total| {
                    *total = total.saturating_sub(amount);
                });

                Self::deposit_event(Event::ShoppingBalanceUsed {
                    entity_id,
                    account: account.clone(),
                    amount,
                });

                Ok(())
            })
        }

        /// 计算提现/复购/奖励分配（Entity 级，四种模式）
        ///
        /// 三层约束模型：
        /// ```text
        /// Governance 底线（强制）
        ///     ↓ max()
        /// Entity 模式设定（FullWithdrawal / FixedRate / LevelBased / MemberChoice）
        ///     ↓ max()
        /// 会员选择（MemberChoice 模式下的 requested_rate）
        ///     ↓
        /// 最终复购比率
        /// ```
        ///
        /// 自愿多复购奖励：超出强制最低线的部分 × voluntary_bonus_rate 额外计入购物余额
        fn calc_withdrawal_split(
            entity_id: u64,
            shop_id: u64,
            who: &T::AccountId,
            total_amount: BalanceOf<T>,
            requested_repurchase_rate: Option<u16>,
        ) -> WithdrawalSplit<BalanceOf<T>> {
            let zero = BalanceOf::<T>::zero();
            let config = WithdrawalConfigs::<T>::get(entity_id);

            // Step 1: 根据模式确定 Entity 层面的复购比率
            let (mode_repurchase_rate, voluntary_bonus_rate) = match config {
                Some(ref config) if config.enabled => {
                    let rate = match &config.mode {
                        WithdrawalMode::FullWithdrawal => 0u16,
                        WithdrawalMode::FixedRate { repurchase_rate } => *repurchase_rate,
                        WithdrawalMode::LevelBased => {
                            let level_id = T::MemberProvider::custom_level_id(shop_id, who);
                            let tier = config.level_overrides
                                .iter()
                                .find(|(id, _)| *id == level_id)
                                .map(|(_, t)| t.clone())
                                .unwrap_or(config.default_tier.clone());
                            tier.repurchase_rate
                        },
                        WithdrawalMode::MemberChoice { min_repurchase_rate } => {
                            let requested = requested_repurchase_rate
                                .unwrap_or(*min_repurchase_rate)
                                .min(10000);
                            requested.max(*min_repurchase_rate)
                        },
                    };
                    (rate, config.voluntary_bonus_rate)
                },
                _ => (0u16, 0u16),
            };

            // Step 2: Governance 底线兜底
            let gov_min_rate = GlobalMinRepurchaseRate::<T>::get(entity_id);
            let mandatory_min_rate = mode_repurchase_rate.max(gov_min_rate).min(10000);

            // Step 3: MemberChoice 模式下会员可选更高比率
            let final_repurchase_rate = if let Some(ref config) = config {
                if config.enabled {
                    if let WithdrawalMode::MemberChoice { .. } = config.mode {
                        let requested = requested_repurchase_rate.unwrap_or(0).min(10000);
                        requested.max(mandatory_min_rate)
                    } else {
                        mandatory_min_rate
                    }
                } else {
                    mandatory_min_rate
                }
            } else {
                mandatory_min_rate
            };

            // Step 4: 计算金额
            let final_withdrawal_rate = 10000u16.saturating_sub(final_repurchase_rate);
            let withdrawal = total_amount
                .saturating_mul(final_withdrawal_rate.into())
                / 10000u32.into();
            let repurchase = total_amount.saturating_sub(withdrawal);

            // Step 5: 计算自愿多复购奖励
            // 超出强制最低线的部分 × voluntary_bonus_rate
            let bonus = if voluntary_bonus_rate > 0 && final_repurchase_rate > mandatory_min_rate {
                let mandatory_repurchase = total_amount
                    .saturating_mul(mandatory_min_rate.into())
                    / 10000u32.into();
                let voluntary_extra = repurchase.saturating_sub(mandatory_repurchase);
                voluntary_extra
                    .saturating_mul(voluntary_bonus_rate.into())
                    / 10000u32.into()
            } else {
                zero
            };

            WithdrawalSplit { withdrawal, repurchase, bonus }
        }

        /// 调度引擎：处理订单返佣（Entity 级佣金池）
        ///
        /// 订单来自 shop_id，但佣金记账在 entity_id 级。
        /// 佣金资金从 Shop 账户转入 Entity 账户。
        pub fn process_commission(
            shop_id: u64,
            order_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            available_pool: BalanceOf<T>,
        ) -> DispatchResult {
            let entity_id = Self::resolve_entity_id(shop_id)?;

            let config = match CommissionConfigs::<T>::get(entity_id) {
                Some(c) if c.enabled => c,
                _ => return Ok(()),
            };

            // 计算最大可用返佣
            let max_commission = available_pool
                .saturating_mul(config.max_commission_rate.into())
                / 10000u32.into();

            // 偿付安全: Shop 可用 = Shop余额（佣金从 Shop 转出）
            let shop_account = T::ShopProvider::shop_account(shop_id);
            let shop_balance = T::Currency::free_balance(&shop_account);
            let mut remaining = max_commission.min(shop_balance);

            if remaining.is_zero() {
                return Ok(());
            }

            let now = <frame_system::Pallet<T>>::block_number();
            let buyer_stats = MemberCommissionStats::<T>::get(entity_id, buyer);
            let is_first_order = buyer_stats.order_count == 0;
            let enabled_modes = config.enabled_modes;

            // 1. Referral Plugin
            let (outputs, new_remaining) = T::ReferralPlugin::calculate(
                entity_id, shop_id, buyer, order_amount, remaining, enabled_modes, is_first_order, buyer_stats.order_count,
            );
            remaining = new_remaining;
            for output in outputs {
                Self::credit_commission(
                    entity_id, shop_id, order_id, buyer, &output.beneficiary, output.amount,
                    output.commission_type, output.level, now,
                )?;
            }

            // 2. LevelDiff Plugin
            let (outputs, new_remaining) = T::LevelDiffPlugin::calculate(
                entity_id, shop_id, buyer, order_amount, remaining, enabled_modes, is_first_order, buyer_stats.order_count,
            );
            remaining = new_remaining;
            for output in outputs {
                Self::credit_commission(
                    entity_id, shop_id, order_id, buyer, &output.beneficiary, output.amount,
                    output.commission_type, output.level, now,
                )?;
            }

            // 3. SingleLine Plugin
            let (outputs, new_remaining) = T::SingleLinePlugin::calculate(
                entity_id, shop_id, buyer, order_amount, remaining, enabled_modes, is_first_order, buyer_stats.order_count,
            );
            remaining = new_remaining;
            for output in outputs {
                Self::credit_commission(
                    entity_id, shop_id, order_id, buyer, &output.beneficiary, output.amount,
                    output.commission_type, output.level, now,
                )?;
            }

            // 4. Team Plugin (预留)
            let (outputs, _new_remaining) = T::TeamPlugin::calculate(
                entity_id, shop_id, buyer, order_amount, remaining, enabled_modes, is_first_order, buyer_stats.order_count,
            );
            for output in outputs {
                Self::credit_commission(
                    entity_id, shop_id, order_id, buyer, &output.beneficiary, output.amount,
                    output.commission_type, output.level, now,
                )?;
            }

            // 更新买家订单数（Entity 级）
            MemberCommissionStats::<T>::mutate(entity_id, buyer, |stats| {
                stats.order_count = stats.order_count.saturating_add(1);
            });

            // 更新 Entity 统计
            let distributed = max_commission.min(shop_balance).saturating_sub(remaining);
            ShopCommissionTotals::<T>::mutate(entity_id, |(total, orders)| {
                *total = total.saturating_add(distributed);
                *orders = orders.saturating_add(1);
            });

            // 将佣金资金从 Shop 账户转入 Entity 账户
            if !distributed.is_zero() {
                let entity_account = T::EntityProvider::entity_account(entity_id);
                T::Currency::transfer(
                    &shop_account,
                    &entity_account,
                    distributed,
                    ExistenceRequirement::KeepAlive,
                )?;

                Self::deposit_event(Event::CommissionFundsTransferred {
                    entity_id,
                    shop_id,
                    amount: distributed,
                });
            }

            Ok(())
        }

        /// 记录并发放返佣（Entity 级记账）
        pub fn credit_commission(
            entity_id: u64,
            shop_id: u64,
            order_id: u64,
            buyer: &T::AccountId,
            beneficiary: &T::AccountId,
            amount: BalanceOf<T>,
            commission_type: CommissionType,
            level: u8,
            now: BlockNumberFor<T>,
        ) -> DispatchResult {
            let record = CommissionRecord {
                entity_id,
                shop_id,
                order_id,
                buyer: buyer.clone(),
                beneficiary: beneficiary.clone(),
                amount,
                commission_type,
                level,
                status: CommissionStatus::Pending,
                created_at: now,
            };

            OrderCommissionRecords::<T>::try_mutate(order_id, |records| {
                records.try_push(record).map_err(|_| Error::<T>::RecordsFull)
            })?;

            MemberCommissionStats::<T>::mutate(entity_id, beneficiary, |stats| {
                stats.total_earned = stats.total_earned.saturating_add(amount);
                stats.pending = stats.pending.saturating_add(amount);
            });

            // 更新最后入账时间（用于冻结期检查）
            MemberLastCredited::<T>::insert(entity_id, beneficiary, now);

            ShopPendingTotal::<T>::mutate(entity_id, |total| {
                *total = total.saturating_add(amount);
            });

            Self::deposit_event(Event::CommissionDistributed {
                entity_id,
                shop_id,
                order_id,
                beneficiary: beneficiary.clone(),
                amount,
                commission_type,
                level,
            });

            Ok(())
        }

        /// 取消订单返佣（Entity 级）
        ///
        /// 退还佣金资金：Entity 账户 → Shop 账户（原订单所属 Shop）
        pub fn cancel_commission(order_id: u64) -> DispatchResult {
            let mut total_refund_by_shop: alloc::vec::Vec<(u64, u64, BalanceOf<T>)> = alloc::vec::Vec::new(); // (entity_id, shop_id, amount)

            OrderCommissionRecords::<T>::mutate(order_id, |records| {
                for record in records.iter_mut() {
                    if record.status == CommissionStatus::Pending {
                        MemberCommissionStats::<T>::mutate(record.entity_id, &record.beneficiary, |stats| {
                            stats.pending = stats.pending.saturating_sub(record.amount);
                            stats.total_earned = stats.total_earned.saturating_sub(record.amount);
                        });
                        ShopPendingTotal::<T>::mutate(record.entity_id, |total| {
                            *total = total.saturating_sub(record.amount);
                        });

                        // 累计需退还给 Shop 的金额
                        if let Some(entry) = total_refund_by_shop.iter_mut().find(|(e, s, _)| *e == record.entity_id && *s == record.shop_id) {
                            entry.2 = entry.2.saturating_add(record.amount);
                        } else {
                            total_refund_by_shop.push((record.entity_id, record.shop_id, record.amount));
                        }

                        record.status = CommissionStatus::Cancelled;
                    }
                }
            });

            // 退还佣金资金从 Entity 账户回到 Shop 账户
            for (entity_id, shop_id, refund_amount) in total_refund_by_shop {
                if !refund_amount.is_zero() {
                    let entity_account = T::EntityProvider::entity_account(entity_id);
                    let shop_account = T::ShopProvider::shop_account(shop_id);
                    // best-effort: 如果 Entity 账户余额不足则跳过（不阻塞取消）
                    let _ = T::Currency::transfer(
                        &entity_account,
                        &shop_account,
                        refund_amount,
                        ExistenceRequirement::KeepAlive,
                    );
                }
            }

            Self::deposit_event(Event::CommissionCancelled { order_id });
            Ok(())
        }
    }
}

// ============================================================================
// CommissionProvider impl
// ============================================================================

/// CommissionFundGuard: 佣金资金已转入 Entity 账户，Shop 不再持有佣金资金
/// 因此 Shop 的 protected_funds 始终为 0
impl<T: pallet::Config> pallet_entity_common::CommissionFundGuard for pallet::Pallet<T> {
    fn protected_funds(_shop_id: u64) -> u128 {
        0
    }
}

/// CommissionProvider impl: 外部接口仍接收 shop_id，内部解析 entity_id
impl<T: pallet::Config> CommissionProvider<T::AccountId, pallet::BalanceOf<T>> for pallet::Pallet<T> {
    fn process_commission(
        shop_id: u64,
        order_id: u64,
        buyer: &T::AccountId,
        order_amount: pallet::BalanceOf<T>,
        available_pool: pallet::BalanceOf<T>,
    ) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::process_commission(shop_id, order_id, buyer, order_amount, available_pool)
    }

    fn cancel_commission(order_id: u64) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::cancel_commission(order_id)
    }

    fn pending_commission(shop_id: u64, account: &T::AccountId) -> pallet::BalanceOf<T> {
        let entity_id = <T::ShopProvider as ShopProviderT<T::AccountId>>::shop_entity_id(shop_id).unwrap_or(0);
        pallet::MemberCommissionStats::<T>::get(entity_id, account).pending
    }

    fn set_commission_modes(shop_id: u64, modes: u16) -> sp_runtime::DispatchResult {
        let entity_id = <T::ShopProvider as ShopProviderT<T::AccountId>>::shop_entity_id(shop_id)
            .ok_or(sp_runtime::DispatchError::Other("ShopNotFound"))?;
        pallet::CommissionConfigs::<T>::mutate(entity_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::CoreCommissionConfig::default);
            config.enabled_modes = CommissionModes(modes);
        });
        Ok(())
    }

    fn set_direct_reward_rate(shop_id: u64, rate: u16) -> sp_runtime::DispatchResult {
        let entity_id = <T::ShopProvider as ShopProviderT<T::AccountId>>::shop_entity_id(shop_id)
            .ok_or(sp_runtime::DispatchError::Other("ShopNotFound"))?;
        <T as pallet::Config>::ReferralWriter::set_direct_rate(entity_id, rate)
    }

    fn set_level_diff_config(shop_id: u64, normal: u16, silver: u16, gold: u16, platinum: u16, diamond: u16) -> sp_runtime::DispatchResult {
        let entity_id = <T::ShopProvider as ShopProviderT<T::AccountId>>::shop_entity_id(shop_id)
            .ok_or(sp_runtime::DispatchError::Other("ShopNotFound"))?;
        <T as pallet::Config>::LevelDiffWriter::set_global_rates(entity_id, normal, silver, gold, platinum, diamond)
    }

    fn set_fixed_amount(shop_id: u64, amount: pallet::BalanceOf<T>) -> sp_runtime::DispatchResult {
        let entity_id = <T::ShopProvider as ShopProviderT<T::AccountId>>::shop_entity_id(shop_id)
            .ok_or(sp_runtime::DispatchError::Other("ShopNotFound"))?;
        <T as pallet::Config>::ReferralWriter::set_fixed_amount(entity_id, amount)
    }

    fn set_first_order_config(shop_id: u64, amount: pallet::BalanceOf<T>, rate: u16, use_amount: bool) -> sp_runtime::DispatchResult {
        let entity_id = <T::ShopProvider as ShopProviderT<T::AccountId>>::shop_entity_id(shop_id)
            .ok_or(sp_runtime::DispatchError::Other("ShopNotFound"))?;
        <T as pallet::Config>::ReferralWriter::set_first_order(entity_id, amount, rate, use_amount)
    }

    fn set_repeat_purchase_config(shop_id: u64, rate: u16, min_orders: u32) -> sp_runtime::DispatchResult {
        let entity_id = <T::ShopProvider as ShopProviderT<T::AccountId>>::shop_entity_id(shop_id)
            .ok_or(sp_runtime::DispatchError::Other("ShopNotFound"))?;
        <T as pallet::Config>::ReferralWriter::set_repeat_purchase(entity_id, rate, min_orders)
    }

    fn set_withdrawal_config_by_governance(
        shop_id: u64,
        enabled: bool,
        shopping_balance_generates_commission: bool,
    ) -> sp_runtime::DispatchResult {
        let entity_id = <T::ShopProvider as ShopProviderT<T::AccountId>>::shop_entity_id(shop_id)
            .ok_or(sp_runtime::DispatchError::Other("ShopNotFound"))?;
        pallet::WithdrawalConfigs::<T>::mutate(entity_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::EntityWithdrawalConfig::default);
            config.enabled = enabled;
            config.shopping_balance_generates_commission = shopping_balance_generates_commission;
        });
        Ok(())
    }

    fn shopping_balance(shop_id: u64, account: &T::AccountId) -> pallet::BalanceOf<T> {
        let entity_id = <T::ShopProvider as ShopProviderT<T::AccountId>>::shop_entity_id(shop_id).unwrap_or(0);
        pallet::MemberShoppingBalance::<T>::get(entity_id, account)
    }

    fn use_shopping_balance(shop_id: u64, account: &T::AccountId, amount: pallet::BalanceOf<T>) -> sp_runtime::DispatchResult {
        let entity_id = <T::ShopProvider as ShopProviderT<T::AccountId>>::shop_entity_id(shop_id)
            .ok_or(sp_runtime::DispatchError::Other("ShopNotFound"))?;
        pallet::Pallet::<T>::do_use_shopping_balance(entity_id, account, amount)
    }

    fn set_min_repurchase_rate(shop_id: u64, rate: u16) -> sp_runtime::DispatchResult {
        let entity_id = <T::ShopProvider as ShopProviderT<T::AccountId>>::shop_entity_id(shop_id)
            .ok_or(sp_runtime::DispatchError::Other("ShopNotFound"))?;
        frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        pallet::GlobalMinRepurchaseRate::<T>::insert(entity_id, rate);
        Ok(())
    }
}
