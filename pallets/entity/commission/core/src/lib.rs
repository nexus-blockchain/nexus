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
    CommissionRecord, CommissionStatus, CommissionType,
    EntityReferrerProvider, LevelDiffPlanWriter, MemberCommissionStatsData, MemberProvider,
    ReferralPlanWriter, TeamPlanWriter, WithdrawalMode, WithdrawalTierConfig,
};
use pallet_entity_common::ShopProvider as ShopProviderT;
use sp_runtime::traits::Zero;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, Get},
    };
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{EntityProvider, ShopProvider};
    use pallet_commission_common::{CommissionPlan, ReferralPlanWriter, LevelDiffPlanWriter, TeamPlanWriter};
    use sp_runtime::traits::{Saturating, Zero};

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    pub type CommissionRecordOf<T> = CommissionRecord<
        <T as frame_system::Config>::AccountId,
        BalanceOf<T>,
        BlockNumberFor<T>,
    >;

    pub type MemberCommissionStatsOf<T> = MemberCommissionStatsData<BalanceOf<T>>;

    /// 全局返佣开关配置（per-entity）
    ///
    /// 会员返佣从卖家货款中扣除（max_commission_rate）
    /// 招商奖金比例由全局常量 ReferrerShareBps 控制
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct CoreCommissionConfig {
        /// 启用的返佣模式（位标志）
        pub enabled_modes: CommissionModes,
        /// 会员返佣上限比例（基点，10000 = 100%）
        /// 佣金从卖家货款中扣除，Entity Owner 可设置
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

        /// 招商推荐人查询接口
        type EntityReferrerProvider: EntityReferrerProvider<Self::AccountId>;

        /// 推荐链方案写入器
        type ReferralWriter: ReferralPlanWriter<BalanceOf<Self>>;

        /// 等级极差方案写入器
        type LevelDiffWriter: LevelDiffPlanWriter;

        /// 团队业绩方案写入器
        type TeamWriter: TeamPlanWriter<BalanceOf<Self>>;

        /// 平台账户（用于招商奖金从平台费中扣除）
        type PlatformAccount: Get<Self::AccountId>;

        /// 国库账户（接收平台费中推荐人奖金以外的部分）
        type TreasuryAccount: Get<Self::AccountId>;

        /// 招商推荐人分佣比例（基点，5000 = 平台费的50%）
        /// 全局固定规则：平台费 = 1%，referrer 50% + 国库 50%
        #[pallet::constant]
        type ReferrerShareBps: Get<u16>;

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
    #[pallet::getter(fn entity_commission_totals)]
    pub type ShopCommissionTotals<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        (BalanceOf<T>, u64),
        ValueQuery,
    >;

    /// Entity 待提取佣金总额 entity_id -> Balance
    #[pallet::storage]
    #[pallet::getter(fn entity_pending_total)]
    pub type ShopPendingTotal<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        BalanceOf<T>,
        ValueQuery,
    >;

    /// Entity 购物余额总额 entity_id -> Balance（资金锁定）
    #[pallet::storage]
    #[pallet::getter(fn entity_shopping_total)]
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

    /// 订单平台费转国库金额 order_id -> Balance（用于 cancel_commission 退款）
    #[pallet::storage]
    pub type OrderTreasuryTransfer<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        BalanceOf<T>,
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
        /// CC-M1 审计修复: 增加成功/失败计数，便于链上追踪部分退款
        CommissionCancelled { order_id: u64, refund_succeeded: u32, refund_failed: u32 },
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
        /// 平台费剩余部分转入国库
        PlatformFeeToTreasury {
            order_id: u64,
            amount: BalanceOf<T>,
        },
        /// 国库退款（订单取消时平台费退回平台账户）
        TreasuryRefund {
            order_id: u64,
            amount: BalanceOf<T>,
        },
        /// 佣金退款失败（Entity 账户余额不足，需人工干预）
        CommissionRefundFailed {
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
        /// 提现金额不能为 0
        ZeroWithdrawalAmount,
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
            entity_id: u64,
            modes: CommissionModes,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_entity_owner(entity_id, &who)?;

            CommissionConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(CoreCommissionConfig::default);
                config.enabled_modes = modes;
            });

            Self::deposit_event(Event::CommissionModesUpdated { entity_id, modes });
            Ok(())
        }

        /// 设置会员返佣上限（Entity 级，从卖家货款扣除）
        ///
        /// 仅 Entity Owner 可调用
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(40_000_000, 4_000))]
        pub fn set_commission_rate(
            origin: OriginFor<T>,
            entity_id: u64,
            max_rate: u16,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_entity_owner(entity_id, &who)?;
            ensure!(max_rate <= 10000, Error::<T>::InvalidCommissionRate);

            CommissionConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(CoreCommissionConfig::default);
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
            entity_id: u64,
            enabled: bool,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_entity_owner(entity_id, &who)?;

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

            MemberCommissionStats::<T>::try_mutate(entity_id, &who, |stats| -> DispatchResult {
                let total_amount = amount.unwrap_or(stats.pending);
                ensure!(stats.pending >= total_amount, Error::<T>::InsufficientCommission);
                ensure!(!total_amount.is_zero(), Error::<T>::ZeroWithdrawalAmount);

                // 如果目标不是自己，校验推荐关系
                if target != who {
                    if T::MemberProvider::is_member_by_entity(entity_id, &target) {
                        // 已是会员 → 推荐人必须是出资人
                        let referrer = T::MemberProvider::get_referrer_by_entity(entity_id, &target);
                        ensure!(referrer.as_ref() == Some(&who), Error::<T>::NotDirectReferral);
                    } else {
                        // 非会员 → 自动注册，推荐人 = 出资人（auto_register 需要 shop_id）
                        T::MemberProvider::auto_register(shop_id, &target, Some(who.clone()))
                            .map_err(|_| Error::<T>::AutoRegisterFailed)?;
                    }
                }

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
                    entity_id, &who, total_amount, requested_repurchase_rate,
                );

                // C1 审计修复: 偿付安全检查必须计入 repurchase+bonus 对 ShopShoppingTotal 的增量
                // 提现后状态: pending -= total_amount, shopping += repurchase + bonus, entity -= withdrawal
                // 需要: entity_balance - withdrawal >= (old_pending - total_amount) + (old_shopping + repurchase + bonus)
                let entity_account = T::EntityProvider::entity_account(entity_id);
                let entity_balance = T::Currency::free_balance(&entity_account);
                let remaining_pending = ShopPendingTotal::<T>::get(entity_id)
                    .saturating_sub(total_amount);
                let total_to_shopping = split.repurchase.saturating_add(split.bonus);
                let new_shopping_total = ShopShoppingTotal::<T>::get(entity_id)
                    .saturating_add(total_to_shopping);
                let required_reserve = remaining_pending.saturating_add(new_shopping_total);
                ensure!(
                    entity_balance >= split.withdrawal.saturating_add(required_reserve),
                    Error::<T>::InsufficientCommission
                );

                // 从 Entity 账户转账提现部分到用户钱包
                if !split.withdrawal.is_zero() {
                    T::Currency::transfer(
                        &entity_account,
                        &who,
                        split.withdrawal,
                        ExistenceRequirement::KeepAlive,
                    )?;
                }

                // 复购部分 + 奖励 转入目标账户的购物余额（total_to_shopping 已在偿付检查中计算）
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
            entity_id: u64,
            mode: WithdrawalMode,
            default_tier: WithdrawalTierConfig,
            level_overrides: BoundedVec<(u8, WithdrawalTierConfig), T::MaxCustomLevels>,
            voluntary_bonus_rate: u16,
            enabled: bool,
            shopping_balance_generates_commission: bool,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_entity_owner(entity_id, &who)?;

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
            entity_id: u64,
            plan: CommissionPlan,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_entity_owner(entity_id, &who)?;

            // 先清除旧配置（entity_id 作为 key）
            T::ReferralWriter::clear_config(entity_id)?;
            T::LevelDiffWriter::clear_config(entity_id)?;
            T::TeamWriter::clear_config(entity_id)?;

            match plan {
                CommissionPlan::None => {
                    CommissionConfigs::<T>::insert(entity_id, CoreCommissionConfig {
                        enabled_modes: CommissionModes(CommissionModes::NONE),
                        max_commission_rate: 0,
                        enabled: false,
                        withdrawal_cooldown: 0,
                    });
                }
                CommissionPlan::DirectOnly { rate } => {
                    ensure!(rate <= 10000, Error::<T>::InvalidCommissionRate);
                    CommissionConfigs::<T>::insert(entity_id, CoreCommissionConfig {
                        enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD),
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
                        max_commission_rate: diamond,
                        enabled: true,
                        withdrawal_cooldown: 0,
                    });
                    T::LevelDiffWriter::set_global_rates(entity_id, normal, silver, gold, platinum, diamond)?;
                }
                CommissionPlan::Custom => {
                    CommissionConfigs::<T>::insert(entity_id, CoreCommissionConfig {
                        enabled_modes: CommissionModes(CommissionModes::NONE),
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
            entity_id: u64,
            account: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_entity_owner(entity_id, &who)?;
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

        /// 验证 Entity 所有者权限（直接通过 entity_id）
        fn ensure_entity_owner(entity_id: u64, who: &T::AccountId) -> Result<(), DispatchError> {
            let owner = T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            ensure!(*who == owner, Error::<T>::NotEntityOwner);
            Ok(())
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
            who: &T::AccountId,
            total_amount: BalanceOf<T>,
            requested_repurchase_rate: Option<u16>,
        ) -> WithdrawalSplit<BalanceOf<T>> {
            let zero = BalanceOf::<T>::zero();
            let config = WithdrawalConfigs::<T>::get(entity_id);

            // Step 1: 根据模式确定 Entity 层面的复购比率
            // - mandatory_base_rate: 模式强制最低线（不含 governance）
            // - mode_final_rate: 模式最终值（MemberChoice 允许高于 mandatory_base_rate）
            let (mandatory_base_rate, mode_final_rate, voluntary_bonus_rate) = match config {
                Some(ref config) if config.enabled => {
                    match &config.mode {
                        WithdrawalMode::FullWithdrawal => (0u16, 0u16, config.voluntary_bonus_rate),
                        WithdrawalMode::FixedRate { repurchase_rate } => {
                            (*repurchase_rate, *repurchase_rate, config.voluntary_bonus_rate)
                        },
                        WithdrawalMode::LevelBased => {
                            let level_id = T::MemberProvider::custom_level_id_by_entity(entity_id, who);
                            let tier = config.level_overrides
                                .iter()
                                .find(|(id, _)| *id == level_id)
                                .map(|(_, t)| t.clone())
                                .unwrap_or(config.default_tier.clone());
                            (tier.repurchase_rate, tier.repurchase_rate, config.voluntary_bonus_rate)
                        },
                        WithdrawalMode::MemberChoice { min_repurchase_rate } => {
                            let requested = requested_repurchase_rate
                                .unwrap_or(*min_repurchase_rate)
                                .min(10000);
                            let mode_rate = requested.max(*min_repurchase_rate);
                            (*min_repurchase_rate, mode_rate, config.voluntary_bonus_rate)
                        },
                    }
                },
                _ => (0u16, 0u16, 0u16),
            };

            // Step 2: Governance 底线兜底
            let gov_min_rate = GlobalMinRepurchaseRate::<T>::get(entity_id);
            let mandatory_min_rate = mandatory_base_rate.max(gov_min_rate).min(10000);
            let final_repurchase_rate = mode_final_rate.max(gov_min_rate).min(10000);

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

        /// 调度引擎：处理订单返佣（双来源架构）
        ///
        /// 订单来自 shop_id，佣金记账在 entity_id 级。
        /// 双来源并行：
        /// - 池 A（平台费池）：platform_fee × ReferrerShareBps → 招商推荐人奖金（EntityReferral）
        /// - 池 B（卖家池）：seller_balance × max_commission_rate → 会员返佣（4 个插件）
        pub fn process_commission(
            shop_id: u64,
            order_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            available_pool: BalanceOf<T>,
            platform_fee: BalanceOf<T>,
        ) -> DispatchResult {
            let entity_id = Self::resolve_entity_id(shop_id)?;
            let platform_account = T::PlatformAccount::get();

            // ── 平台费无条件转国库（无论佣金是否配置，保障平台收入） ──
            // 全局固定规则：referrer 拿 ReferrerShareBps%，剩余进国库
            let config = CommissionConfigs::<T>::get(entity_id)
                .filter(|c| c.enabled);

            // 计算推荐人奖金占比（有 referrer 时才预留）
            let global_referrer_bps = T::ReferrerShareBps::get();
            let has_referrer = global_referrer_bps > 0
                && T::EntityReferrerProvider::entity_referrer(entity_id).is_some();
            let referrer_quota = if has_referrer {
                platform_fee
                    .saturating_mul(global_referrer_bps.into())
                    / 10000u32.into()
            } else {
                BalanceOf::<T>::zero()
            };
            let treasury_portion = platform_fee.saturating_sub(referrer_quota);

            if !treasury_portion.is_zero() {
                let treasury_account = T::TreasuryAccount::get();
                let platform_balance = T::Currency::free_balance(&platform_account);
                let min_balance = T::Currency::minimum_balance();
                let platform_transferable = platform_balance.saturating_sub(min_balance);
                // 为推荐人预留额度：先扣除 referrer_quota，剩余才给国库
                let treasury_cap = platform_transferable.saturating_sub(referrer_quota);
                let actual_treasury = treasury_portion.min(treasury_cap);
                if !actual_treasury.is_zero() {
                    T::Currency::transfer(
                        &platform_account,
                        &treasury_account,
                        actual_treasury,
                        ExistenceRequirement::KeepAlive,
                    )?;
                    OrderTreasuryTransfer::<T>::insert(order_id, actual_treasury);
                    Self::deposit_event(Event::PlatformFeeToTreasury {
                        order_id,
                        amount: actual_treasury,
                    });
                }
            }

            // 未配置佣金或未启用 → 平台费已入库，直接返回
            let config = match config {
                Some(c) => c,
                None => return Ok(()),
            };
            let seller = T::ShopProvider::shop_owner(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;
            let entity_account = T::EntityProvider::entity_account(entity_id);
            let now = <frame_system::Pallet<T>>::block_number();
            let buyer_stats = MemberCommissionStats::<T>::get(entity_id, buyer);
            let is_first_order = buyer_stats.order_count == 0;
            let enabled_modes = config.enabled_modes;

            let mut total_from_platform = BalanceOf::<T>::zero();
            let mut total_from_seller = BalanceOf::<T>::zero();

            // ── 池 A：招商推荐人奖金（从平台费扣除，比例由全局常量控制） ──
            let referrer_share_bps = T::ReferrerShareBps::get();
            if referrer_share_bps > 0 {
                if let Some(referrer) = T::EntityReferrerProvider::entity_referrer(entity_id) {
                    let referrer_quota = platform_fee
                        .saturating_mul(referrer_share_bps.into())
                        / 10000u32.into();
                    // KeepAlive 要求转账后余额 >= ED
                    let platform_balance = T::Currency::free_balance(&platform_account);
                    let min_balance = T::Currency::minimum_balance();
                    let transferable = platform_balance.saturating_sub(min_balance);
                    let referrer_amount = referrer_quota.min(transferable);
                    if !referrer_amount.is_zero() {
                        Self::credit_commission(
                            entity_id, shop_id, order_id, buyer, &referrer,
                            referrer_amount, CommissionType::EntityReferral, 0, now,
                        )?;
                        total_from_platform = referrer_amount;
                    }
                }
            }

            // ── 池 B：会员返佣（从卖家货款扣除） ──
            let max_commission = available_pool
                .saturating_mul(config.max_commission_rate.into())
                / 10000u32.into();
            let seller_balance = T::Currency::free_balance(&seller);
            let seller_min = T::Currency::minimum_balance();
            let seller_transferable = seller_balance.saturating_sub(seller_min);
            let mut remaining = max_commission.min(seller_transferable);

            if !remaining.is_zero() {
                let initial_remaining = remaining;

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

                // 4. Team Plugin
                let (outputs, new_remaining) = T::TeamPlugin::calculate(
                    entity_id, shop_id, buyer, order_amount, remaining, enabled_modes, is_first_order, buyer_stats.order_count,
                );
                remaining = new_remaining;
                for output in outputs {
                    Self::credit_commission(
                        entity_id, shop_id, order_id, buyer, &output.beneficiary, output.amount,
                        output.commission_type, output.level, now,
                    )?;
                }

                total_from_seller = initial_remaining.saturating_sub(remaining);
            }

            // 更新买家订单数（Entity 级）
            MemberCommissionStats::<T>::mutate(entity_id, buyer, |stats| {
                stats.order_count = stats.order_count.saturating_add(1);
            });

            let total_distributed = total_from_platform.saturating_add(total_from_seller);

            // 更新 Entity 统计
            ShopCommissionTotals::<T>::mutate(entity_id, |(total, orders)| {
                *total = total.saturating_add(total_distributed);
                *orders = orders.saturating_add(1);
            });

            // 将佣金资金转入 Entity 账户（双来源分别转）
            if !total_from_platform.is_zero() {
                T::Currency::transfer(
                    &platform_account,
                    &entity_account,
                    total_from_platform,
                    ExistenceRequirement::KeepAlive,
                )?;
            }

            if !total_from_seller.is_zero() {
                T::Currency::transfer(
                    &seller,
                    &entity_account,
                    total_from_seller,
                    ExistenceRequirement::KeepAlive,
                )?;
            }

            if !total_distributed.is_zero() {
                Self::deposit_event(Event::CommissionFundsTransferred {
                    entity_id,
                    shop_id,
                    amount: total_distributed,
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

        /// 取消订单返佣（双来源架构）
        ///
        /// 按 CommissionType 决定退款目标：
        /// - `EntityReferral`: Entity 账户 → 平台账户
        /// - 其余: Entity 账户 → 卖家 (shop_owner)
        ///
        /// H2 审计修复: 先尝试转账，成功后再取消记录和更新统计，
        /// 防止转账失败但记录已被标记为 Cancelled 导致资金丢失。
        pub fn cancel_commission(order_id: u64) -> DispatchResult {
            let records = OrderCommissionRecords::<T>::get(order_id);
            let platform_account = T::PlatformAccount::get();

            // 第一步：按 (entity_id, shop_id, is_platform) 分组汇总待退还金额
            // is_platform = true → EntityReferral（退平台），false → 会员返佣（退卖家）
            let mut refund_groups: alloc::vec::Vec<(u64, u64, bool, BalanceOf<T>)> = alloc::vec::Vec::new();

            for record in records.iter() {
                if record.status == CommissionStatus::Pending {
                    let is_platform = record.commission_type == CommissionType::EntityReferral;
                    if let Some(entry) = refund_groups.iter_mut().find(|(e, s, p, _)| *e == record.entity_id && *s == record.shop_id && *p == is_platform) {
                        entry.3 = entry.3.saturating_add(record.amount);
                    } else {
                        refund_groups.push((record.entity_id, record.shop_id, is_platform, record.amount));
                    }
                }
            }

            // 第二步：尝试转账
            let mut refund_succeeded: alloc::vec::Vec<(u64, u64, bool)> = alloc::vec::Vec::new();

            for (entity_id, shop_id, is_platform, refund_amount) in refund_groups.iter() {
                if refund_amount.is_zero() {
                    refund_succeeded.push((*entity_id, *shop_id, *is_platform));
                    continue;
                }
                let entity_account = T::EntityProvider::entity_account(*entity_id);

                let refund_target = if *is_platform {
                    platform_account.clone()
                } else {
                    match T::ShopProvider::shop_owner(*shop_id) {
                        Some(seller) => seller,
                        None => {
                            Self::deposit_event(Event::CommissionRefundFailed {
                                entity_id: *entity_id,
                                shop_id: *shop_id,
                                amount: *refund_amount,
                            });
                            continue;
                        }
                    }
                };

                if T::Currency::transfer(
                    &entity_account,
                    &refund_target,
                    *refund_amount,
                    ExistenceRequirement::KeepAlive,
                ).is_ok() {
                    refund_succeeded.push((*entity_id, *shop_id, *is_platform));
                } else {
                    Self::deposit_event(Event::CommissionRefundFailed {
                        entity_id: *entity_id,
                        shop_id: *shop_id,
                        amount: *refund_amount,
                    });
                }
            }

            // 第三步：仅取消转账成功的记录，更新统计
            OrderCommissionRecords::<T>::mutate(order_id, |records| {
                for record in records.iter_mut() {
                    if record.status == CommissionStatus::Pending {
                        let is_platform = record.commission_type == CommissionType::EntityReferral;
                        if refund_succeeded.iter().any(|(e, s, p)| *e == record.entity_id && *s == record.shop_id && *p == is_platform) {
                            MemberCommissionStats::<T>::mutate(record.entity_id, &record.beneficiary, |stats| {
                                stats.pending = stats.pending.saturating_sub(record.amount);
                                stats.total_earned = stats.total_earned.saturating_sub(record.amount);
                            });
                            ShopPendingTotal::<T>::mutate(record.entity_id, |total| {
                                *total = total.saturating_sub(record.amount);
                            });
                            record.status = CommissionStatus::Cancelled;
                        }
                    }
                }
            });

            // 第四步：退还国库部分（Treasury → PlatformAccount）
            let treasury_refund = OrderTreasuryTransfer::<T>::get(order_id);
            if !treasury_refund.is_zero() {
                let treasury_account = T::TreasuryAccount::get();
                if T::Currency::transfer(
                    &treasury_account,
                    &platform_account,
                    treasury_refund,
                    ExistenceRequirement::AllowDeath,
                ).is_ok() {
                    OrderTreasuryTransfer::<T>::remove(order_id);
                    Self::deposit_event(Event::TreasuryRefund {
                        order_id,
                        amount: treasury_refund,
                    });
                } else {
                    // 国库余额不足时记录事件，保留 Storage 供后续重试
                    Self::deposit_event(Event::CommissionRefundFailed {
                        entity_id: 0,
                        shop_id: 0,
                        amount: treasury_refund,
                    });
                }
            }

            // CC-M1: 汇总退款结果
            let succeeded = refund_succeeded.len() as u32;
            let failed = (refund_groups.len() as u32).saturating_sub(succeeded);
            Self::deposit_event(Event::CommissionCancelled { order_id, refund_succeeded: succeeded, refund_failed: failed });
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
        platform_fee: pallet::BalanceOf<T>,
    ) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::process_commission(shop_id, order_id, buyer, order_amount, available_pool, platform_fee)
    }

    fn cancel_commission(order_id: u64) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::cancel_commission(order_id)
    }

    fn pending_commission(shop_id: u64, account: &T::AccountId) -> pallet::BalanceOf<T> {
        match <T::ShopProvider as ShopProviderT<T::AccountId>>::shop_entity_id(shop_id) {
            Some(entity_id) => pallet::MemberCommissionStats::<T>::get(entity_id, account).pending,
            None => Zero::zero(),
        }
    }

    fn set_commission_modes(shop_id: u64, modes: u16) -> sp_runtime::DispatchResult {
        let allowed_modes = CommissionModes::DIRECT_REWARD
            | CommissionModes::MULTI_LEVEL
            | CommissionModes::TEAM_PERFORMANCE
            | CommissionModes::LEVEL_DIFF
            | CommissionModes::FIXED_AMOUNT
            | CommissionModes::FIRST_ORDER
            | CommissionModes::REPEAT_PURCHASE
            | CommissionModes::SINGLE_LINE_UPLINE
            | CommissionModes::SINGLE_LINE_DOWNLINE;
        frame_support::ensure!(modes & !allowed_modes == 0, sp_runtime::DispatchError::Other("InvalidModes"));

        let entity_id = <T::ShopProvider as ShopProviderT<T::AccountId>>::shop_entity_id(shop_id)
            .ok_or(sp_runtime::DispatchError::Other("ShopNotFound"))?;
        pallet::CommissionConfigs::<T>::mutate(entity_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::CoreCommissionConfig::default);
            config.enabled_modes = CommissionModes(modes);
        });
        Ok(())
    }

    fn set_direct_reward_rate(shop_id: u64, rate: u16) -> sp_runtime::DispatchResult {
        frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        let entity_id = <T::ShopProvider as ShopProviderT<T::AccountId>>::shop_entity_id(shop_id)
            .ok_or(sp_runtime::DispatchError::Other("ShopNotFound"))?;
        <T as pallet::Config>::ReferralWriter::set_direct_rate(entity_id, rate)
    }

    fn set_level_diff_config(shop_id: u64, normal: u16, silver: u16, gold: u16, platinum: u16, diamond: u16) -> sp_runtime::DispatchResult {
        frame_support::ensure!(normal <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        frame_support::ensure!(silver <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        frame_support::ensure!(gold <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        frame_support::ensure!(platinum <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        frame_support::ensure!(diamond <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
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
        frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        let entity_id = <T::ShopProvider as ShopProviderT<T::AccountId>>::shop_entity_id(shop_id)
            .ok_or(sp_runtime::DispatchError::Other("ShopNotFound"))?;
        <T as pallet::Config>::ReferralWriter::set_first_order(entity_id, amount, rate, use_amount)
    }

    fn set_repeat_purchase_config(shop_id: u64, rate: u16, min_orders: u32) -> sp_runtime::DispatchResult {
        frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
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
        match <T::ShopProvider as ShopProviderT<T::AccountId>>::shop_entity_id(shop_id) {
            Some(entity_id) => pallet::MemberShoppingBalance::<T>::get(entity_id, account),
            None => Zero::zero(),
        }
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
