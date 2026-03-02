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
    PoolRewardPlanWriter, ReferralPlanWriter, TeamPlanWriter, WithdrawalMode, WithdrawalTierConfig,
    TokenCommissionPlugin, TokenCommissionRecord, MemberTokenCommissionStatsData,
    TokenTransferProvider as TokenTransferProviderT,
};
use pallet_entity_common::ShopProvider as ShopProviderT;
use sp_runtime::traits::Zero;

/// Entity 参与权守卫（H3 修复: KYC / 合规检查接口）
///
/// 在 `withdraw_commission` 和 `do_consume_shopping_balance` 中调用，
/// 确保 target 账户满足 Entity 的参与要求（如 mandatory KYC）。
/// 默认空实现允许所有操作（适用于未配置 KYC 的 Entity）。
pub trait ParticipationGuard<AccountId> {
    fn can_participate(entity_id: u64, account: &AccountId) -> bool;
}

/// 默认空实现（无 KYC 系统时使用，所有账户均允许）
impl<AccountId> ParticipationGuard<AccountId> for () {
    fn can_participate(_entity_id: u64, _account: &AccountId) -> bool {
        true
    }
}

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

    pub type TokenBalanceOf<T> = <T as Config>::TokenBalance;

    pub type TokenCommissionRecordOf<T> = TokenCommissionRecord<
        <T as frame_system::Config>::AccountId,
        TokenBalanceOf<T>,
        BlockNumberFor<T>,
    >;

    pub type MemberTokenCommissionStatsOf<T> = MemberTokenCommissionStatsData<TokenBalanceOf<T>>;

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
    }

    impl<MaxLevels: Get<u32>> Default for EntityWithdrawalConfig<MaxLevels> {
        fn default() -> Self {
            Self {
                mode: WithdrawalMode::default(),
                default_tier: WithdrawalTierConfig::default(),
                level_overrides: BoundedVec::default(),
                voluntary_bonus_rate: 0,
                enabled: false,
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

        /// 沉淀池奖励方案写入器
        type PoolRewardWriter: PoolRewardPlanWriter;

        /// 平台账户（用于招商奖金从平台费中扣除）
        type PlatformAccount: Get<Self::AccountId>;

        /// 国库账户（接收平台费中推荐人奖金以外的部分）
        type TreasuryAccount: Get<Self::AccountId>;

        /// 招商推荐人分佣比例（基点，5000 = 平台费的50%）
        /// 全局固定规则：平台费 = 1%，referrer 50% + 国库 50%
        #[pallet::constant]
        type ReferrerShareBps: Get<u16>;

        /// Token 订单平台费率（基点，全局固定，100 = 1%）
        /// 与 NEX PlatformFeeRate 对称，不可 per-entity 配置
        #[pallet::constant]
        type TokenPlatformFeeRate: Get<u16>;

        /// 最大返佣记录数（每订单）
        #[pallet::constant]
        type MaxCommissionRecordsPerOrder: Get<u32>;

        /// 最大自定义等级数
        #[pallet::constant]
        type MaxCustomLevels: Get<u32>;

        /// 关闭 POOL_REWARD 后提取沉淀池资金的冷却期（区块数）
        /// 防止 Entity Owner 开启→积累→关闭→提取 的套利循环
        #[pallet::constant]
        type PoolRewardWithdrawCooldown: Get<BlockNumberFor<Self>>;

        /// Entity 参与权守卫（KYC / 合规检查）
        /// 默认使用 `()` 允许所有操作（无 KYC 要求）
        type ParticipationGuard: crate::ParticipationGuard<Self::AccountId>;

        // ====================================================================
        // Token 多资产扩展（方案 A）
        // ====================================================================

        /// Entity Token 余额类型
        type TokenBalance: codec::FullCodec
            + codec::MaxEncodedLen
            + TypeInfo
            + Copy
            + Default
            + core::fmt::Debug
            + sp_runtime::traits::AtLeast32BitUnsigned
            + From<u32>
            + Into<u128>;

        /// Token 版推荐链插件
        type TokenReferralPlugin: TokenCommissionPlugin<Self::AccountId, TokenBalanceOf<Self>>;

        /// Token 版等级极差插件
        type TokenLevelDiffPlugin: TokenCommissionPlugin<Self::AccountId, TokenBalanceOf<Self>>;

        /// Token 版单线收益插件
        type TokenSingleLinePlugin: TokenCommissionPlugin<Self::AccountId, TokenBalanceOf<Self>>;

        /// Token 版团队业绩插件
        type TokenTeamPlugin: TokenCommissionPlugin<Self::AccountId, TokenBalanceOf<Self>>;

        /// Token 转账接口（entity_id 级）
        type TokenTransferProvider: TokenTransferProviderT<Self::AccountId, TokenBalanceOf<Self>>;
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

    /// 未分配佣金沉淀资金池 entity_id -> Balance
    #[pallet::storage]
    #[pallet::getter(fn unallocated_pool)]
    pub type UnallocatedPool<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        BalanceOf<T>,
        ValueQuery,
    >;

    /// 订单未分配佣金记录 order_id -> (entity_id, shop_id, Balance)
    /// 用于 cancel_commission 时退还未分配部分给卖家
    #[pallet::storage]
    pub type OrderUnallocated<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        (u64, u64, BalanceOf<T>),
        ValueQuery,
    >;

    // ========================================================================
    // Token Storage（方案 A: 全插件多资产化）
    // ========================================================================

    /// Token 佣金统计 (entity_id, account) → MemberTokenCommissionStatsData
    #[pallet::storage]
    pub type MemberTokenCommissionStats<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        MemberTokenCommissionStatsOf<T>,
        ValueQuery,
    >;

    /// Token 佣金记录 order_id → Vec<TokenCommissionRecord>
    #[pallet::storage]
    pub type OrderTokenCommissionRecords<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        BoundedVec<TokenCommissionRecordOf<T>, T::MaxCommissionRecordsPerOrder>,
        ValueQuery,
    >;

    /// Token 待提取总额 entity_id → TokenBalance
    #[pallet::storage]
    pub type TokenPendingTotal<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        TokenBalanceOf<T>,
        ValueQuery,
    >;

    /// Token 未分配沉淀池 entity_id → TokenBalance
    #[pallet::storage]
    pub type UnallocatedTokenPool<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        TokenBalanceOf<T>,
        ValueQuery,
    >;

    /// Token 订单沉淀池记录 order_id → (entity_id, shop_id, TokenBalance)
    #[pallet::storage]
    pub type OrderTokenUnallocated<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        (u64, u64, TokenBalanceOf<T>),
        ValueQuery,
    >;

    // Token 平台费率已改为 Config 全局常量 TokenPlatformFeeRate（不再 per-entity 存储）

    /// Token 购物余额 (entity_id, account) → TokenBalance
    #[pallet::storage]
    #[pallet::getter(fn member_token_shopping_balance)]
    pub type MemberTokenShoppingBalance<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        TokenBalanceOf<T>,
        ValueQuery,
    >;

    /// Token 购物余额总额 entity_id → TokenBalance（资金锁定）
    #[pallet::storage]
    #[pallet::getter(fn token_shopping_total)]
    pub type TokenShoppingTotal<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        TokenBalanceOf<T>,
        ValueQuery,
    >;

    /// Token 提现配置 entity_id → EntityWithdrawalConfig（与 NEX 对称，独立配置）
    #[pallet::storage]
    #[pallet::getter(fn token_withdrawal_config)]
    pub type TokenWithdrawalConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        EntityWithdrawalConfigOf<T>,
    >;

    /// Token 全局最低复购比例 entity_id → u16（万分比，由 Governance 设定）
    #[pallet::storage]
    #[pallet::getter(fn global_min_token_repurchase_rate)]
    pub type GlobalMinTokenRepurchaseRate<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        u16,
        ValueQuery,
    >;

    /// entity_account Token 已记账余额（用于检测外部转入）
    /// 跟踪 entity_account 通过已知渠道（平台费入账、佣金提现、购物消费、退款）
    /// 应有的 Token 余额。actual_balance - accounted = 外部转入金额。
    #[pallet::storage]
    pub type EntityTokenAccountedBalance<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        TokenBalanceOf<T>,
    >;

    /// POOL_REWARD 关闭时的区块号（用于 cooldown 计算）
    /// 存在 = POOL_REWARD 已关闭，不存在 = POOL_REWARD 开启或从未配置
    #[pallet::storage]
    pub type PoolRewardDisabledAt<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        BlockNumberFor<T>,
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
            /// M3 审计修复: 购物余额实际接收账户（可能与 account 不同）
            repurchase_target: T::AccountId,
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
        /// 未分配佣金转入沉淀资金池
        UnallocatedCommissionPooled {
            entity_id: u64,
            order_id: u64,
            amount: BalanceOf<T>,
        },
        /// 沉淀池奖励发放（Phase 2）
        PoolRewardDistributed {
            entity_id: u64,
            order_id: u64,
            total_distributed: BalanceOf<T>,
        },
        /// 沉淀池退还卖家（订单取消）
        UnallocatedPoolRefunded {
            entity_id: u64,
            order_id: u64,
            amount: BalanceOf<T>,
        },

        // Token 多资产事件
        /// Token 佣金分发
        TokenCommissionDistributed {
            entity_id: u64,
            order_id: u64,
            beneficiary: T::AccountId,
            amount: TokenBalanceOf<T>,
            commission_type: CommissionType,
            level: u8,
        },
        /// Token 佣金提现
        TokenCommissionWithdrawn {
            entity_id: u64,
            account: T::AccountId,
            amount: TokenBalanceOf<T>,
        },
        /// Token 沉淀池入账
        TokenUnallocatedPooled {
            entity_id: u64,
            order_id: u64,
            amount: TokenBalanceOf<T>,
        },
        /// Token 沉淀池退还
        TokenUnallocatedPoolRefunded {
            entity_id: u64,
            order_id: u64,
            amount: TokenBalanceOf<T>,
        },
        /// Token 佣金取消
        TokenCommissionCancelled {
            order_id: u64,
            cancelled_count: u32,
        },
        /// Token 分层提现（含复购分流 + 赠与）
        TokenTieredWithdrawal {
            entity_id: u64,
            account: T::AccountId,
            repurchase_target: T::AccountId,
            withdrawn_amount: TokenBalanceOf<T>,
            repurchase_amount: TokenBalanceOf<T>,
            bonus_amount: TokenBalanceOf<T>,
        },
        /// Token 提现配置已更新
        TokenWithdrawalConfigUpdated { entity_id: u64 },
        /// Token 购物余额已使用
        TokenShoppingBalanceUsed {
            entity_id: u64,
            account: T::AccountId,
            amount: TokenBalanceOf<T>,
        },
        /// Entity Owner 提取 entity_account NEX 自由余额
        EntityFundsWithdrawn {
            entity_id: u64,
            to: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// Entity Owner 提取 entity_account Token 自由余额
        EntityTokenFundsWithdrawn {
            entity_id: u64,
            to: T::AccountId,
            amount: TokenBalanceOf<T>,
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
        /// LevelBased 配置中 level_id 重复
        DuplicateLevelId,
        /// 复购目标账户未通过审批（APPROVAL_REQUIRED 下待审批状态）
        TargetNotApprovedMember,
        /// 会员未激活（代注册会员需首次消费后激活）
        MemberNotActivated,
        /// H3: 复购目标账户不满足 Entity 的参与要求（如 mandatory KYC）
        TargetParticipationDenied,
        /// H3: 账户不满足 Entity 参与要求，无法提取购物余额
        ParticipationRequirementNotMet,
        /// 购物余额仅可用于购物（下单抵扣），不可直接提取为 NEX
        ShoppingBalanceWithdrawalDisabled,
        /// 沉淀资金池余额不足
        InsufficientUnallocatedPool,
        /// Token 佣金余额不足
        InsufficientTokenCommission,
        /// Token 转账失败
        TokenTransferFailed,
        /// Entity 账户 NEX 自由余额不足（提取后不能低于锁定总额）
        InsufficientEntityFunds,
        /// Entity 账户 Token 自由余额不足
        InsufficientEntityTokenFunds,
        /// POOL_REWARD 关闭后冷却期未满，暂时不可提取沉淀池资金
        PoolRewardCooldownActive,
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

            let old_has_pool = CommissionConfigs::<T>::get(entity_id)
                .map(|c| c.enabled_modes.contains(CommissionModes::POOL_REWARD))
                .unwrap_or(false);
            let new_has_pool = modes.contains(CommissionModes::POOL_REWARD);

            CommissionConfigs::<T>::mutate(entity_id, |maybe| {
                let config = maybe.get_or_insert_with(CoreCommissionConfig::default);
                config.enabled_modes = modes;
            });

            // 跟踪 POOL_REWARD 开关变化
            if old_has_pool && !new_has_pool {
                // POOL_REWARD 被关闭 → 记录时间，启动 cooldown
                let now = <frame_system::Pallet<T>>::block_number();
                PoolRewardDisabledAt::<T>::insert(entity_id, now);
            } else if !old_has_pool && new_has_pool {
                // POOL_REWARD 被开启 → 清除 cooldown 记录
                PoolRewardDisabledAt::<T>::remove(entity_id);
            }

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
        /// - `entity_id`: Entity ID（佣金统一在 Entity 级记账）
        /// - `amount`: 提现金额（None = 全部 pending）
        /// - `requested_repurchase_rate`: 会员请求的复购比率（万分比，MemberChoice 模式下使用）
        /// - `repurchase_target`: 复购购物余额的接收账户（None = 自己）
        ///   - 目标为非会员：自动注册，推荐人 = 出资人
        ///   - 目标为已有会员：推荐人必须是出资人，否则拒绝
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(80_000_000, 6_000))]
        pub fn withdraw_commission(
            origin: OriginFor<T>,
            entity_id: u64,
            amount: Option<BalanceOf<T>>,
            requested_repurchase_rate: Option<u16>,
            repurchase_target: Option<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 确定复购目标账户
            let target = repurchase_target.unwrap_or_else(|| who.clone());

            MemberCommissionStats::<T>::try_mutate(entity_id, &who, |stats| -> DispatchResult {
                let total_amount = amount.unwrap_or(stats.pending);
                ensure!(stats.pending >= total_amount, Error::<T>::InsufficientCommission);
                ensure!(!total_amount.is_zero(), Error::<T>::ZeroWithdrawalAmount);

                // 如果目标不是自己，校验推荐关系
                if target != who {
                    if T::MemberProvider::is_member(entity_id, &target) {
                        // 已是会员 → 推荐人必须是出资人
                        let referrer = T::MemberProvider::get_referrer(entity_id, &target);
                        ensure!(referrer.as_ref() == Some(&who), Error::<T>::NotDirectReferral);
                    } else {
                        // 非会员 → 自动注册，推荐人 = 出资人（复购赠与 → qualified=false）
                        T::MemberProvider::auto_register_qualified(entity_id, &target, Some(who.clone()), false)
                            .map_err(|_| Error::<T>::AutoRegisterFailed)?;
                        // H1 修复: 注册后验证 target 是否已成为正式会员
                        // APPROVAL_REQUIRED 策略下 auto_register 返回 Ok 但 target 仅进入 PendingMembers
                        ensure!(
                            T::MemberProvider::is_member(entity_id, &target),
                            Error::<T>::TargetNotApprovedMember
                        );
                    }

                    // H3 修复: 检查 target 是否满足 Entity 的参与要求（如 mandatory KYC）
                    ensure!(
                        T::ParticipationGuard::can_participate(entity_id, &target),
                        Error::<T>::TargetParticipationDenied
                    );
                }

                // H1 审计修复: 提现前检查 WithdrawalConfig 是否启用
                // 未启用时 calc_withdrawal_split 返回 0% 复购，会绕过 Governance 底线
                let withdrawal_config = WithdrawalConfigs::<T>::get(entity_id);
                if let Some(ref wc) = withdrawal_config {
                    ensure!(wc.enabled, Error::<T>::WithdrawalConfigNotEnabled);
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
                let unallocated = UnallocatedPool::<T>::get(entity_id);
                let required_reserve = remaining_pending.saturating_add(new_shopping_total).saturating_add(unallocated);
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
                // H3 审计修复: repurchased 应包含 bonus（两者均进入购物余额）
                stats.repurchased = stats.repurchased.saturating_add(split.repurchase).saturating_add(split.bonus);

                // 释放 pending 锁定
                ShopPendingTotal::<T>::mutate(entity_id, |total| {
                    *total = total.saturating_sub(total_amount);
                });

                // 发出事件
                Self::deposit_event(Event::TieredWithdrawal {
                    entity_id,
                    account: who.clone(),
                    repurchase_target: target.clone(),
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
            {
                let mut seen_ids = alloc::collections::BTreeSet::new();
                for (level_id, tier) in level_overrides.iter() {
                    ensure!(seen_ids.insert(*level_id), Error::<T>::DuplicateLevelId);
                    ensure!(
                        tier.withdrawal_rate.saturating_add(tier.repurchase_rate) == 10000,
                        Error::<T>::InvalidWithdrawalConfig
                    );
                }
            }

            // 校验 bonus rate
            ensure!(voluntary_bonus_rate <= 10000, Error::<T>::InvalidWithdrawalConfig);

            WithdrawalConfigs::<T>::insert(entity_id, EntityWithdrawalConfig {
                mode,
                default_tier,
                level_overrides,
                voluntary_bonus_rate,
                enabled,
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

            let old_has_pool = CommissionConfigs::<T>::get(entity_id)
                .map(|c| c.enabled_modes.contains(CommissionModes::POOL_REWARD))
                .unwrap_or(false);

            // 先清除旧配置（entity_id 作为 key）
            T::ReferralWriter::clear_config(entity_id)?;
            T::LevelDiffWriter::clear_config(entity_id)?;
            T::TeamWriter::clear_config(entity_id)?;
            T::PoolRewardWriter::clear_config(entity_id)?;

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
                CommissionPlan::LevelDiff { ref level_rates } => {
                    let mut max_rate = 0u16;
                    for &rate in level_rates.iter() {
                        ensure!(rate <= 10000, Error::<T>::InvalidCommissionRate);
                        if rate > max_rate { max_rate = rate; }
                    }
                    CommissionConfigs::<T>::insert(entity_id, CoreCommissionConfig {
                        enabled_modes: CommissionModes(
                            CommissionModes::DIRECT_REWARD | CommissionModes::LEVEL_DIFF
                        ),
                        max_commission_rate: max_rate,
                        enabled: true,
                        withdrawal_cooldown: 0,
                    });
                    let depth = level_rates.len() as u8;
                    T::LevelDiffWriter::set_level_rates(entity_id, level_rates.to_vec(), depth)?;
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

            // 跟踪 POOL_REWARD 开关变化
            let new_has_pool = CommissionConfigs::<T>::get(entity_id)
                .map(|c| c.enabled_modes.contains(CommissionModes::POOL_REWARD))
                .unwrap_or(false);
            if old_has_pool && !new_has_pool {
                let now = <frame_system::Pallet<T>>::block_number();
                PoolRewardDisabledAt::<T>::insert(entity_id, now);
            } else if !old_has_pool && new_has_pool {
                PoolRewardDisabledAt::<T>::remove(entity_id);
            }

            Self::deposit_event(Event::CommissionPlanInitialized { entity_id, plan });
            Ok(())
        }

        /// [已禁用] 购物余额仅可用于购物（place_order 下单抵扣），不可直接提取为 NEX。
        ///
        /// 保留 call_index(5) 以维持链上 call index 稳定。
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(10_000_000, 1_000))]
        pub fn use_shopping_balance(
            origin: OriginFor<T>,
            _entity_id: u64,
            _amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            Err(Error::<T>::ShoppingBalanceWithdrawalDisabled.into())
        }

        /// 设置 Token 提现配置（Entity 级，四种模式 + 自愿复购奖励）
        ///
        /// 与 NEX set_withdrawal_config 完全对称，配置独立存储在 TokenWithdrawalConfigs
        #[pallet::call_index(10)]
        #[pallet::weight(Weight::from_parts(50_000_000, 5_000))]
        pub fn set_token_withdrawal_config(
            origin: OriginFor<T>,
            entity_id: u64,
            mode: WithdrawalMode,
            default_tier: WithdrawalTierConfig,
            level_overrides: BoundedVec<(u8, WithdrawalTierConfig), T::MaxCustomLevels>,
            voluntary_bonus_rate: u16,
            enabled: bool,
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

            // 校验 level_overrides 无重复 level_id
            for (i, (id_a, _)) in level_overrides.iter().enumerate() {
                for (id_b, _) in level_overrides.iter().skip(i + 1) {
                    ensure!(id_a != id_b, Error::<T>::DuplicateLevelId);
                }
            }

            // 校验 bonus rate
            ensure!(voluntary_bonus_rate <= 10000, Error::<T>::InvalidWithdrawalConfig);

            TokenWithdrawalConfigs::<T>::insert(entity_id, EntityWithdrawalConfig {
                mode,
                default_tier,
                level_overrides,
                voluntary_bonus_rate,
                enabled,
            });

            Self::deposit_event(Event::TokenWithdrawalConfigUpdated { entity_id });
            Ok(())
        }

        /// 设置 Token 全局最低复购比例（Root，Governance 底线）
        ///
        /// Token 提现时实际复购比例 = max(entity 分层配置, 此底线)
        #[pallet::call_index(11)]
        #[pallet::weight(Weight::from_parts(30_000_000, 3_000))]
        pub fn set_global_min_token_repurchase_rate(
            origin: OriginFor<T>,
            entity_id: u64,
            rate: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(rate <= 10000, Error::<T>::InvalidCommissionRate);
            GlobalMinTokenRepurchaseRate::<T>::insert(entity_id, rate);
            Ok(())
        }

        /// Token 佣金提现（四种提现模式 + 复购分流 + 自愿复购奖励 + 赠与提现）
        ///
        /// - `entity_id`: Entity ID
        /// - `amount`: 提现金额（None = 全部 pending）
        /// - `requested_repurchase_rate`: 会员请求的复购比率（万分比，MemberChoice 模式下使用）
        /// - `repurchase_target`: 复购购物余额的接收账户（None = 自己）
        ///   - 目标为非会员：自动注册，推荐人 = 出资人
        ///   - 目标为已有会员：推荐人必须是出资人，否则拒绝
        #[pallet::call_index(8)]
        #[pallet::weight(Weight::from_parts(100_000_000, 8_000))]
        pub fn withdraw_token_commission(
            origin: OriginFor<T>,
            entity_id: u64,
            amount: Option<TokenBalanceOf<T>>,
            requested_repurchase_rate: Option<u16>,
            repurchase_target: Option<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // H1 审计修复: Token 提现也需检查参与权（与 NEX 提现一致）
            ensure!(
                T::ParticipationGuard::can_participate(entity_id, &who),
                Error::<T>::ParticipationRequirementNotMet
            );

            // 确定复购目标账户
            let target = repurchase_target.unwrap_or_else(|| who.clone());

            MemberTokenCommissionStats::<T>::try_mutate(entity_id, &who, |stats| -> DispatchResult {
                let total_amount = amount.unwrap_or(stats.pending);
                ensure!(stats.pending >= total_amount, Error::<T>::InsufficientTokenCommission);
                ensure!(!total_amount.is_zero(), Error::<T>::ZeroWithdrawalAmount);

                // 如果目标不是自己，校验推荐关系（赠与提现）
                if target != who {
                    if T::MemberProvider::is_member(entity_id, &target) {
                        let referrer = T::MemberProvider::get_referrer(entity_id, &target);
                        ensure!(referrer.as_ref() == Some(&who), Error::<T>::NotDirectReferral);
                    } else {
                        T::MemberProvider::auto_register_qualified(entity_id, &target, Some(who.clone()), false)
                            .map_err(|_| Error::<T>::AutoRegisterFailed)?;
                        ensure!(
                            T::MemberProvider::is_member(entity_id, &target),
                            Error::<T>::TargetNotApprovedMember
                        );
                    }

                    ensure!(
                        T::ParticipationGuard::can_participate(entity_id, &target),
                        Error::<T>::TargetParticipationDenied
                    );
                }

                // Token 提现配置启用检查
                let token_wc = TokenWithdrawalConfigs::<T>::get(entity_id);
                if let Some(ref wc) = token_wc {
                    ensure!(wc.enabled, Error::<T>::WithdrawalConfigNotEnabled);
                }

                // 冻结期检查（共用 NEX 冻结期配置）
                if let Some(config) = CommissionConfigs::<T>::get(entity_id) {
                    if config.withdrawal_cooldown > 0 {
                        let now = <frame_system::Pallet<T>>::block_number();
                        let last = MemberLastCredited::<T>::get(entity_id, &who);
                        let cooldown: BlockNumberFor<T> = config.withdrawal_cooldown.into();
                        ensure!(now >= last.saturating_add(cooldown),
                            Error::<T>::WithdrawalCooldownNotMet);
                    }
                }

                // 计算 Token 提现/复购/奖励分配
                let split = Self::calc_token_withdrawal_split(
                    entity_id, &who, total_amount, requested_repurchase_rate,
                );

                // Token 偿付安全检查: entity_token_balance >= withdrawal + pending_remaining + shopping_new + unallocated_pool
                let entity_account = T::EntityProvider::entity_account(entity_id);
                let entity_token_balance = T::TokenTransferProvider::token_balance_of(
                    entity_id, &entity_account,
                );
                let remaining_pending = TokenPendingTotal::<T>::get(entity_id)
                    .saturating_sub(total_amount);
                let total_to_shopping = split.repurchase.saturating_add(split.bonus);
                let new_shopping_total = TokenShoppingTotal::<T>::get(entity_id)
                    .saturating_add(total_to_shopping);
                let unallocated = UnallocatedTokenPool::<T>::get(entity_id);
                let required_reserve = remaining_pending
                    .saturating_add(new_shopping_total)
                    .saturating_add(unallocated);
                ensure!(
                    entity_token_balance >= split.withdrawal.saturating_add(required_reserve),
                    Error::<T>::InsufficientTokenCommission
                );

                // Token 转账: entity_account → who（提现部分）
                if !split.withdrawal.is_zero() {
                    T::TokenTransferProvider::token_transfer(
                        entity_id, &entity_account, &who, split.withdrawal,
                    ).map_err(|_| Error::<T>::TokenTransferFailed)?;
                    EntityTokenAccountedBalance::<T>::mutate(entity_id, |b| {
                        *b = b.map(|v| v.saturating_sub(split.withdrawal));
                    });
                }

                // 复购部分 + 奖励 → 目标账户的 Token 购物余额
                if !total_to_shopping.is_zero() {
                    MemberTokenShoppingBalance::<T>::mutate(entity_id, &target, |balance| {
                        *balance = balance.saturating_add(total_to_shopping);
                    });
                    TokenShoppingTotal::<T>::mutate(entity_id, |total| {
                        *total = total.saturating_add(total_to_shopping);
                    });
                }

                // 统计记在出资人名下
                stats.pending = stats.pending.saturating_sub(total_amount);
                stats.withdrawn = stats.withdrawn.saturating_add(split.withdrawal);
                stats.repurchased = stats.repurchased
                    .saturating_add(split.repurchase)
                    .saturating_add(split.bonus);

                // 释放 pending 锁定
                TokenPendingTotal::<T>::mutate(entity_id, |total| {
                    *total = total.saturating_sub(total_amount);
                });

                Self::deposit_event(Event::TokenTieredWithdrawal {
                    entity_id,
                    account: who.clone(),
                    repurchase_target: target.clone(),
                    withdrawn_amount: split.withdrawal,
                    repurchase_amount: split.repurchase,
                    bonus_amount: split.bonus,
                });

                Ok(())
            })
        }

        /// Entity Owner 提取 entity_account 中未被锁定的 NEX 自由余额
        ///
        /// 提取后 entity_balance 必须 ≥ PendingTotal + ShoppingTotal + UnallocatedPool
        #[pallet::call_index(12)]
        #[pallet::weight(Weight::from_parts(60_000_000, 5_000))]
        pub fn withdraw_entity_funds(
            origin: OriginFor<T>,
            entity_id: u64,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_entity_owner(entity_id, &who)?;
            ensure!(!amount.is_zero(), Error::<T>::ZeroWithdrawalAmount);

            let entity_account = T::EntityProvider::entity_account(entity_id);
            let entity_balance = T::Currency::free_balance(&entity_account);

            // 判断沉淀池是否锁定
            let pool_balance = UnallocatedPool::<T>::get(entity_id);
            let pool_locked = Self::is_pool_reward_locked(entity_id);
            let unallocated_reserve = if pool_locked {
                pool_balance
            } else {
                BalanceOf::<T>::zero()
            };

            let non_pool_reserved = ShopPendingTotal::<T>::get(entity_id)
                .saturating_add(ShopShoppingTotal::<T>::get(entity_id));
            let reserved = non_pool_reserved.saturating_add(unallocated_reserve);
            let min_balance = T::Currency::minimum_balance();
            let available = entity_balance
                .saturating_sub(reserved)
                .saturating_sub(min_balance);
            ensure!(amount <= available, Error::<T>::InsufficientEntityFunds);

            T::Currency::transfer(
                &entity_account,
                &who,
                amount,
                ExistenceRequirement::KeepAlive,
            )?;

            // 提取后同步扣减沉淀池（如果动用了池资金）
            if !pool_locked && !pool_balance.is_zero() {
                let new_balance = T::Currency::free_balance(&entity_account);
                let max_pool = new_balance
                    .saturating_sub(non_pool_reserved)
                    .saturating_sub(min_balance);
                if max_pool < pool_balance {
                    UnallocatedPool::<T>::insert(entity_id, max_pool);
                }
            }

            Self::deposit_event(Event::EntityFundsWithdrawn {
                entity_id,
                to: who,
                amount,
            });
            Ok(())
        }

        /// Entity Owner 提取 entity_account 中未被锁定的 Token 自由余额
        ///
        /// 提取后 entity_token_balance 必须 ≥ TokenPendingTotal + TokenShoppingTotal + UnallocatedTokenPool
        #[pallet::call_index(13)]
        #[pallet::weight(Weight::from_parts(60_000_000, 5_000))]
        pub fn withdraw_entity_token_funds(
            origin: OriginFor<T>,
            entity_id: u64,
            amount: TokenBalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_entity_owner(entity_id, &who)?;
            ensure!(!amount.is_zero(), Error::<T>::ZeroWithdrawalAmount);

            // 检测外部直接转入的 Token 并归入沉淀池（incoming=0，无已知入账）
            Self::sweep_token_free_balance(entity_id, TokenBalanceOf::<T>::zero());

            let entity_account = T::EntityProvider::entity_account(entity_id);
            let entity_token_balance = T::TokenTransferProvider::token_balance_of(
                entity_id, &entity_account,
            );

            // 判断沉淀池是否锁定
            let pool_balance = UnallocatedTokenPool::<T>::get(entity_id);
            let pool_locked = Self::is_pool_reward_locked(entity_id);
            let unallocated_reserve = if pool_locked {
                pool_balance
            } else {
                TokenBalanceOf::<T>::zero()
            };

            let non_pool_reserved = TokenPendingTotal::<T>::get(entity_id)
                .saturating_add(TokenShoppingTotal::<T>::get(entity_id));
            let reserved = non_pool_reserved.saturating_add(unallocated_reserve);
            let available = entity_token_balance.saturating_sub(reserved);
            ensure!(amount <= available, Error::<T>::InsufficientEntityTokenFunds);

            T::TokenTransferProvider::token_transfer(
                entity_id, &entity_account, &who, amount,
            ).map_err(|_| Error::<T>::TokenTransferFailed)?;
            EntityTokenAccountedBalance::<T>::mutate(entity_id, |b| {
                *b = b.map(|v| v.saturating_sub(amount));
            });

            // 提取后同步扣减沉淀池（如果动用了池资金）
            if !pool_locked && !pool_balance.is_zero() {
                let new_actual = entity_token_balance.saturating_sub(amount);
                let max_pool = new_actual.saturating_sub(non_pool_reserved);
                if max_pool < pool_balance {
                    UnallocatedTokenPool::<T>::insert(entity_id, max_pool);
                }
            }

            Self::deposit_event(Event::EntityTokenFundsWithdrawn {
                entity_id,
                to: who,
                amount,
            });
            Ok(())
        }

    }

    // ========================================================================
    // Internal functions
    // ========================================================================

    impl<T: Config> Pallet<T> {
        /// 判断沉淀池是否锁定（不可被 Entity Owner 提取）
        ///
        /// 锁定条件：
        /// - POOL_REWARD 开启 → 锁定（资金用于会员领奖）
        /// - POOL_REWARD 关闭但 cooldown 未满 → 锁定（防套利）
        /// - POOL_REWARD 关闭且 cooldown 已满 → 不锁定（可提取）
        /// - 从未配置 POOL_REWARD → 不锁定
        fn is_pool_reward_locked(entity_id: u64) -> bool {
            let pool_reward_on = CommissionConfigs::<T>::get(entity_id)
                .map(|c| c.enabled && c.enabled_modes.contains(CommissionModes::POOL_REWARD))
                .unwrap_or(false);

            if pool_reward_on {
                return true;
            }

            // POOL_REWARD 未开启，检查是否有 cooldown
            if let Some(disabled_at) = PoolRewardDisabledAt::<T>::get(entity_id) {
                let now = <frame_system::Pallet<T>>::block_number();
                let cooldown = T::PoolRewardWithdrawCooldown::get();
                if now < disabled_at.saturating_add(cooldown) {
                    return true; // cooldown 期内仍锁定
                }
            }

            false
        }

        /// 从 shop_id 解析 entity_id
        #[allow(dead_code)]
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

        /// 检测并归集外部直接转入 entity_account 的 Token 到沉淀池。
        ///
        /// `incoming`: 本次已知的合法入账金额（如 token_platform_fee），
        /// 在调用前已到达 entity_account，不应被视为外部转入。
        ///
        /// 原理：EntityTokenAccountedBalance 记录 entity_account 通过已知渠道应有的余额，
        /// actual_balance - (accounted + incoming) > 0 即为外部转入。
        fn sweep_token_free_balance(entity_id: u64, incoming: TokenBalanceOf<T>) {
            let entity_account = T::EntityProvider::entity_account(entity_id);
            let actual = T::TokenTransferProvider::token_balance_of(
                entity_id, &entity_account,
            );
            let accounted = EntityTokenAccountedBalance::<T>::get(entity_id)
                .unwrap_or_else(|| actual.saturating_sub(incoming));
            let expected = accounted.saturating_add(incoming);
            let external = actual.saturating_sub(expected);
            if !external.is_zero() {
                UnallocatedTokenPool::<T>::mutate(entity_id, |pool| {
                    *pool = pool.saturating_add(external);
                });
                Self::deposit_event(Event::TokenUnallocatedPooled {
                    entity_id,
                    order_id: 0,
                    amount: external,
                });
            }
            // 快照当前实际余额（含 incoming + external）
            EntityTokenAccountedBalance::<T>::insert(entity_id, actual);
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

        /// 消费购物余额（扣减记账 + NEX 从 Entity 账户转入会员钱包）
        ///
        /// 供 `use_shopping_balance` extrinsic 和 `ShoppingBalanceProvider::consume` 调用。
        /// 与 `do_use_shopping_balance`（纯记账）不同，本函数会实际转移 NEX。
        pub fn do_consume_shopping_balance(
            entity_id: u64,
            account: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure!(!amount.is_zero(), Error::<T>::ZeroWithdrawalAmount);

            // H3 修复: 检查账户是否满足 Entity 的参与要求（如 mandatory KYC）
            ensure!(
                T::ParticipationGuard::can_participate(entity_id, account),
                Error::<T>::ParticipationRequirementNotMet
            );

            MemberShoppingBalance::<T>::try_mutate(entity_id, account, |balance| -> DispatchResult {
                ensure!(*balance >= amount, Error::<T>::InsufficientShoppingBalance);
                *balance = balance.saturating_sub(amount);

                ShopShoppingTotal::<T>::mutate(entity_id, |total| {
                    *total = total.saturating_sub(amount);
                });

                // 将 NEX 从 Entity 账户转入会员钱包
                let entity_account = T::EntityProvider::entity_account(entity_id);
                T::Currency::transfer(
                    &entity_account,
                    account,
                    amount,
                    ExistenceRequirement::KeepAlive,
                )?;

                Self::deposit_event(Event::ShoppingBalanceUsed {
                    entity_id,
                    account: account.clone(),
                    amount,
                });

                Ok(())
            })
        }

        /// 消费 Token 购物余额（扣减记账 + Token 从 Entity 账户转入会员钱包）
        ///
        /// 与 `do_consume_shopping_balance`（NEX 版）对称，供订单模块调用。
        pub fn do_consume_token_shopping_balance(
            entity_id: u64,
            account: &T::AccountId,
            amount: TokenBalanceOf<T>,
        ) -> DispatchResult {
            ensure!(!amount.is_zero(), Error::<T>::ZeroWithdrawalAmount);

            // H3 修复: 检查账户是否满足 Entity 的参与要求（如 mandatory KYC）
            ensure!(
                T::ParticipationGuard::can_participate(entity_id, account),
                Error::<T>::ParticipationRequirementNotMet
            );

            MemberTokenShoppingBalance::<T>::try_mutate(entity_id, account, |balance| -> DispatchResult {
                ensure!(*balance >= amount, Error::<T>::InsufficientShoppingBalance);
                *balance = balance.saturating_sub(amount);

                TokenShoppingTotal::<T>::mutate(entity_id, |total| {
                    *total = total.saturating_sub(amount);
                });

                // 将 Token 从 Entity 账户转入会员钱包
                let entity_account = T::EntityProvider::entity_account(entity_id);
                T::TokenTransferProvider::token_transfer(
                    entity_id, &entity_account, account, amount,
                ).map_err(|_| Error::<T>::TokenTransferFailed)?;
                EntityTokenAccountedBalance::<T>::mutate(entity_id, |b| {
                    *b = b.map(|v| v.saturating_sub(amount));
                });

                Self::deposit_event(Event::TokenShoppingBalanceUsed {
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
                            let level_id = T::MemberProvider::custom_level_id(entity_id, who);
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

        /// Token 提现分配计算（与 NEX calc_withdrawal_split 对称，使用 Token 独立配置）
        fn calc_token_withdrawal_split(
            entity_id: u64,
            who: &T::AccountId,
            total_amount: TokenBalanceOf<T>,
            requested_repurchase_rate: Option<u16>,
        ) -> WithdrawalSplit<TokenBalanceOf<T>> {
            let zero = TokenBalanceOf::<T>::zero();
            let config = TokenWithdrawalConfigs::<T>::get(entity_id);

            let (mandatory_base_rate, mode_final_rate, voluntary_bonus_rate) = match config {
                Some(ref config) if config.enabled => {
                    match &config.mode {
                        WithdrawalMode::FullWithdrawal => (0u16, 0u16, config.voluntary_bonus_rate),
                        WithdrawalMode::FixedRate { repurchase_rate } => {
                            (*repurchase_rate, *repurchase_rate, config.voluntary_bonus_rate)
                        },
                        WithdrawalMode::LevelBased => {
                            let level_id = T::MemberProvider::custom_level_id(entity_id, who);
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

            // Governance 底线兜底（Token 独立配置）
            let gov_min_rate = GlobalMinTokenRepurchaseRate::<T>::get(entity_id);
            let mandatory_min_rate = mandatory_base_rate.max(gov_min_rate).min(10000);
            let final_repurchase_rate = mode_final_rate.max(gov_min_rate).min(10000);

            // 计算金额
            let final_withdrawal_rate = 10000u16.saturating_sub(final_repurchase_rate);
            let withdrawal = total_amount
                .saturating_mul(final_withdrawal_rate.into())
                / 10000u32.into();
            let repurchase = total_amount.saturating_sub(withdrawal);

            // 自愿多复购奖励
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
            entity_id: u64,
            shop_id: u64,
            order_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            available_pool: BalanceOf<T>,
            platform_fee: BalanceOf<T>,
        ) -> DispatchResult {
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
                            entity_id, order_id, buyer, &referrer,
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
                    entity_id, buyer, order_amount, remaining, enabled_modes, is_first_order, buyer_stats.order_count,
                );
                remaining = new_remaining;
                for output in outputs {
                    Self::credit_commission(
                        entity_id, order_id, buyer, &output.beneficiary, output.amount,
                        output.commission_type, output.level, now,
                    )?;
                }

                // 2. LevelDiff Plugin
                let (outputs, new_remaining) = T::LevelDiffPlugin::calculate(
                    entity_id, buyer, order_amount, remaining, enabled_modes, is_first_order, buyer_stats.order_count,
                );
                remaining = new_remaining;
                for output in outputs {
                    Self::credit_commission(
                        entity_id, order_id, buyer, &output.beneficiary, output.amount,
                        output.commission_type, output.level, now,
                    )?;
                }

                // 3. SingleLine Plugin
                let (outputs, new_remaining) = T::SingleLinePlugin::calculate(
                    entity_id, buyer, order_amount, remaining, enabled_modes, is_first_order, buyer_stats.order_count,
                );
                remaining = new_remaining;
                for output in outputs {
                    Self::credit_commission(
                        entity_id, order_id, buyer, &output.beneficiary, output.amount,
                        output.commission_type, output.level, now,
                    )?;
                }

                // 4. Team Plugin
                let (outputs, new_remaining) = T::TeamPlugin::calculate(
                    entity_id, buyer, order_amount, remaining, enabled_modes, is_first_order, buyer_stats.order_count,
                );
                remaining = new_remaining;
                for output in outputs {
                    Self::credit_commission(
                        entity_id, order_id, buyer, &output.beneficiary, output.amount,
                        output.commission_type, output.level, now,
                    )?;
                }

                total_from_seller = initial_remaining.saturating_sub(remaining);
            }

            // ── Phase 1.5：未分配佣金 → 沉淀资金池 ──
            let mut pool_funded = BalanceOf::<T>::zero();
            if enabled_modes.contains(CommissionModes::POOL_REWARD) && !remaining.is_zero() {
                let seller_balance_now = T::Currency::free_balance(&seller);
                let seller_min = T::Currency::minimum_balance();
                let seller_transferable_now = seller_balance_now.saturating_sub(seller_min);
                let actual_pool = remaining.min(seller_transferable_now);
                if !actual_pool.is_zero() {
                    T::Currency::transfer(
                        &seller,
                        &entity_account,
                        actual_pool,
                        ExistenceRequirement::KeepAlive,
                    )?;
                    UnallocatedPool::<T>::mutate(entity_id, |pool| {
                        *pool = pool.saturating_add(actual_pool);
                    });
                    OrderUnallocated::<T>::insert(order_id, (entity_id, shop_id, actual_pool));
                    pool_funded = actual_pool;
                    Self::deposit_event(Event::UnallocatedCommissionPooled {
                        entity_id,
                        order_id,
                        amount: actual_pool,
                    });
                }
            }

            // Phase 2 已移除：沉淀池奖励改为用户主动 claim（pool-reward v2）
            let total_pool_distributed = BalanceOf::<T>::zero();

            // 更新买家订单数（Entity 级）
            MemberCommissionStats::<T>::mutate(entity_id, buyer, |stats| {
                stats.order_count = stats.order_count.saturating_add(1);
            });

            // total_distributed 仅统计从外部转入的佣金（不含池内循环）
            let total_distributed = total_from_platform.saturating_add(total_from_seller);

            // 更新 Entity 统计（含池奖励）
            ShopCommissionTotals::<T>::mutate(entity_id, |(total, orders)| {
                *total = total.saturating_add(total_distributed).saturating_add(total_pool_distributed);
                *orders = orders.saturating_add(1);
            });

            // 将佣金资金转入 Entity 账户（双来源分别转；池资金已在 entity_account 中）
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

            if !total_distributed.is_zero() || !pool_funded.is_zero() {
                Self::deposit_event(Event::CommissionFundsTransferred {
                    entity_id,
                    shop_id,
                    amount: total_distributed.saturating_add(pool_funded),
                });
            }

            Ok(())
        }

        /// 记录并发放返佣（Entity 级记账）
        pub fn credit_commission(
            entity_id: u64,
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
                shop_id: 0,
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
                order_id,
                beneficiary: beneficiary.clone(),
                amount,
                commission_type,
                level,
            });

            Ok(())
        }

        // ====================================================================
        // Token 多资产管线
        // ====================================================================

        /// Token 调度引擎：处理 Token 订单返佣（双源架构）
        ///
        /// 池 A（Token 平台费池）：token_platform_fee → 招商推荐人 Token 奖金 + Entity 留存
        /// 池 B（Entity Token 池）：entity_account Token × max_rate → 4 插件 → 沉淀池
        pub fn process_token_commission(
            entity_id: u64,
            shop_id: u64,
            order_id: u64,
            buyer: &T::AccountId,
            token_order_amount: TokenBalanceOf<T>,
            token_platform_fee: TokenBalanceOf<T>,
        ) -> DispatchResult {
            let config = CommissionConfigs::<T>::get(entity_id)
                .filter(|c| c.enabled)
                .ok_or(Error::<T>::CommissionNotConfigured)?;

            // 检测外部直接转入的 Token 并归入沉淀池（token_platform_fee 是已知合法入账）
            Self::sweep_token_free_balance(entity_id, token_platform_fee);

            let enabled_modes = config.enabled_modes;
            let entity_account = T::EntityProvider::entity_account(entity_id);
            let now = <frame_system::Pallet<T>>::block_number();
            let buyer_stats = MemberTokenCommissionStats::<T>::get(entity_id, buyer);
            let is_first_order = buyer_stats.order_count == 0;

            // ── 池 A：Token 招商推荐人奖金（从 Token 平台费中分配） ──
            let mut pool_a_distributed = TokenBalanceOf::<T>::zero();
            let referrer_share_bps = T::ReferrerShareBps::get();
            if referrer_share_bps > 0 && !token_platform_fee.is_zero() {
                if let Some(referrer) = T::EntityReferrerProvider::entity_referrer(entity_id) {
                    let referrer_quota = token_platform_fee
                        .saturating_mul(referrer_share_bps.into())
                        / 10000u32.into();
                    if !referrer_quota.is_zero() {
                        Self::credit_token_commission(
                            entity_id, order_id, buyer, &referrer,
                            referrer_quota, CommissionType::EntityReferral, 0, now,
                        )?;
                        pool_a_distributed = referrer_quota;
                    }
                }
            }
            // 池 A 剩余部分计入沉淀池（不留为 FREE_BALANCE）
            let pool_a_retention = token_platform_fee.saturating_sub(pool_a_distributed);
            if !pool_a_retention.is_zero() {
                UnallocatedTokenPool::<T>::mutate(entity_id, |pool| {
                    *pool = pool.saturating_add(pool_a_retention);
                });
                Self::deposit_event(Event::TokenUnallocatedPooled {
                    entity_id, order_id, amount: pool_a_retention,
                });
            }

            // ── 池 B：会员 Token 返佣（从 entity_account Token 余额中分配） ──
            let max_commission = token_order_amount
                .saturating_mul(config.max_commission_rate.into())
                / 10000u32.into();

            let entity_token_balance = T::TokenTransferProvider::token_balance_of(
                entity_id, &entity_account,
            );
            let mut remaining = max_commission.min(entity_token_balance);

            if !remaining.is_zero() {
                // 1. Token Referral Plugin
                let (outputs, new_remaining) = T::TokenReferralPlugin::calculate_token(
                    entity_id, buyer, token_order_amount, remaining,
                    enabled_modes, is_first_order, buyer_stats.order_count,
                );
                remaining = new_remaining;
                for output in outputs {
                    Self::credit_token_commission(
                        entity_id, order_id, buyer, &output.beneficiary,
                        output.amount, output.commission_type, output.level, now,
                    )?;
                }

                // 2. Token LevelDiff Plugin
                let (outputs, new_remaining) = T::TokenLevelDiffPlugin::calculate_token(
                    entity_id, buyer, token_order_amount, remaining,
                    enabled_modes, is_first_order, buyer_stats.order_count,
                );
                remaining = new_remaining;
                for output in outputs {
                    Self::credit_token_commission(
                        entity_id, order_id, buyer, &output.beneficiary,
                        output.amount, output.commission_type, output.level, now,
                    )?;
                }

                // 3. Token SingleLine Plugin
                let (outputs, new_remaining) = T::TokenSingleLinePlugin::calculate_token(
                    entity_id, buyer, token_order_amount, remaining,
                    enabled_modes, is_first_order, buyer_stats.order_count,
                );
                remaining = new_remaining;
                for output in outputs {
                    Self::credit_token_commission(
                        entity_id, order_id, buyer, &output.beneficiary,
                        output.amount, output.commission_type, output.level, now,
                    )?;
                }

                // 4. Token Team Plugin
                let (outputs, new_remaining) = T::TokenTeamPlugin::calculate_token(
                    entity_id, buyer, token_order_amount, remaining,
                    enabled_modes, is_first_order, buyer_stats.order_count,
                );
                remaining = new_remaining;
                for output in outputs {
                    Self::credit_token_commission(
                        entity_id, order_id, buyer, &output.beneficiary,
                        output.amount, output.commission_type, output.level, now,
                    )?;
                }
            }

            // 剩余 Token → 沉淀池
            if enabled_modes.contains(CommissionModes::POOL_REWARD) && !remaining.is_zero() {
                UnallocatedTokenPool::<T>::mutate(entity_id, |pool| {
                    *pool = pool.saturating_add(remaining);
                });
                OrderTokenUnallocated::<T>::insert(order_id, (entity_id, shop_id, remaining));
                Self::deposit_event(Event::TokenUnallocatedPooled {
                    entity_id, order_id, amount: remaining,
                });
            }

            // 更新买家订单数（Token 版）
            MemberTokenCommissionStats::<T>::mutate(entity_id, buyer, |stats| {
                stats.order_count = stats.order_count.saturating_add(1);
            });

            Ok(())
        }

        /// Token 佣金记账（纯记账，不转账——Token 在 entity_account 中托管直到提现）
        pub fn credit_token_commission(
            entity_id: u64,
            order_id: u64,
            buyer: &T::AccountId,
            beneficiary: &T::AccountId,
            amount: TokenBalanceOf<T>,
            commission_type: CommissionType,
            level: u8,
            now: BlockNumberFor<T>,
        ) -> DispatchResult {
            let record = TokenCommissionRecord {
                entity_id,
                order_id,
                buyer: buyer.clone(),
                beneficiary: beneficiary.clone(),
                amount,
                commission_type,
                level,
                status: CommissionStatus::Pending,
                created_at: now,
            };

            OrderTokenCommissionRecords::<T>::try_mutate(order_id, |records| {
                records.try_push(record).map_err(|_| Error::<T>::RecordsFull)
            })?;

            MemberTokenCommissionStats::<T>::mutate(entity_id, beneficiary, |stats| {
                stats.total_earned = stats.total_earned.saturating_add(amount);
                stats.pending = stats.pending.saturating_add(amount);
            });

            TokenPendingTotal::<T>::mutate(entity_id, |total| {
                *total = total.saturating_add(amount);
            });

            Self::deposit_event(Event::TokenCommissionDistributed {
                entity_id, order_id,
                beneficiary: beneficiary.clone(),
                amount, commission_type, level,
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
            // PoolReward 记录不参与转账退款（资金回池）
            let mut refund_groups: alloc::vec::Vec<(u64, u64, bool, BalanceOf<T>)> = alloc::vec::Vec::new();
            let mut pool_return_groups: alloc::vec::Vec<(u64, BalanceOf<T>)> = alloc::vec::Vec::new();

            for record in records.iter() {
                if record.status == CommissionStatus::Pending {
                    if record.commission_type == CommissionType::PoolReward {
                        if let Some(entry) = pool_return_groups.iter_mut().find(|(e, _)| *e == record.entity_id) {
                            entry.1 = entry.1.saturating_add(record.amount);
                        } else {
                            pool_return_groups.push((record.entity_id, record.amount));
                        }
                    } else {
                        let is_platform = record.commission_type == CommissionType::EntityReferral;
                        if let Some(entry) = refund_groups.iter_mut().find(|(e, s, p, _)| *e == record.entity_id && *s == record.shop_id && *p == is_platform) {
                            entry.3 = entry.3.saturating_add(record.amount);
                        } else {
                            refund_groups.push((record.entity_id, record.shop_id, is_platform, record.amount));
                        }
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
            // PoolReward 记录无需转账，直接回池并取消
            for (entity_id, return_amount) in pool_return_groups.iter() {
                if !return_amount.is_zero() {
                    UnallocatedPool::<T>::mutate(entity_id, |pool| {
                        *pool = pool.saturating_add(*return_amount);
                    });
                }
            }

            OrderCommissionRecords::<T>::mutate(order_id, |records| {
                for record in records.iter_mut() {
                    if record.status == CommissionStatus::Pending {
                        if record.commission_type == CommissionType::PoolReward {
                            // PoolReward 记录已在上方回池，直接取消
                            MemberCommissionStats::<T>::mutate(record.entity_id, &record.beneficiary, |stats| {
                                stats.pending = stats.pending.saturating_sub(record.amount);
                                stats.total_earned = stats.total_earned.saturating_sub(record.amount);
                            });
                            ShopPendingTotal::<T>::mutate(record.entity_id, |total| {
                                *total = total.saturating_sub(record.amount);
                            });
                            record.status = CommissionStatus::Cancelled;
                        } else {
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

            // 第五步：退还本订单沉淀池贡献（entity_account → seller）
            let (unalloc_entity_id, unalloc_shop_id, unalloc_amount) = OrderUnallocated::<T>::get(order_id);
            if !unalloc_amount.is_zero() {
                let entity_account = T::EntityProvider::entity_account(unalloc_entity_id);
                if let Some(seller) = T::ShopProvider::shop_owner(unalloc_shop_id) {
                    if T::Currency::transfer(
                        &entity_account,
                        &seller,
                        unalloc_amount,
                        ExistenceRequirement::KeepAlive,
                    ).is_ok() {
                        UnallocatedPool::<T>::mutate(unalloc_entity_id, |pool| {
                            *pool = pool.saturating_sub(unalloc_amount);
                        });
                        OrderUnallocated::<T>::remove(order_id);
                        Self::deposit_event(Event::UnallocatedPoolRefunded {
                            entity_id: unalloc_entity_id,
                            order_id,
                            amount: unalloc_amount,
                        });
                    }
                }
            }

            // CC-M1: 汇总退款结果
            let succeeded = refund_succeeded.len() as u32;
            let failed = (refund_groups.len() as u32).saturating_sub(succeeded);
            Self::deposit_event(Event::CommissionCancelled { order_id, refund_succeeded: succeeded, refund_failed: failed });

            // ── 第六步：退还 Token 佣金记录（纯记账，无转账） ──
            let mut token_cancelled: u32 = 0;
            OrderTokenCommissionRecords::<T>::mutate(order_id, |records| {
                for record in records.iter_mut() {
                    if record.status == CommissionStatus::Pending {
                        MemberTokenCommissionStats::<T>::mutate(
                            record.entity_id, &record.beneficiary, |stats| {
                                stats.pending = stats.pending.saturating_sub(record.amount);
                                stats.total_earned = stats.total_earned.saturating_sub(record.amount);
                            }
                        );
                        TokenPendingTotal::<T>::mutate(record.entity_id, |total| {
                            *total = total.saturating_sub(record.amount);
                        });
                        record.status = CommissionStatus::Cancelled;
                        token_cancelled = token_cancelled.saturating_add(1);
                    }
                }
            });

            // ── H2 审计修复：第七步：退还 Token 沉淀池 — 仅在转账成功时扣减池余额 ──
            let (te_id, ts_id, t_amount) = OrderTokenUnallocated::<T>::get(order_id);
            if !t_amount.is_zero() {
                let mut refund_ok = false;
                let entity_account = T::EntityProvider::entity_account(te_id);
                if let Some(seller) = T::ShopProvider::shop_owner(ts_id) {
                    if T::TokenTransferProvider::token_transfer(
                        te_id, &entity_account, &seller, t_amount,
                    ).is_ok() {
                        refund_ok = true;
                    }
                }
                if refund_ok {
                    UnallocatedTokenPool::<T>::mutate(te_id, |pool| {
                        *pool = pool.saturating_sub(t_amount);
                    });
                    EntityTokenAccountedBalance::<T>::mutate(te_id, |b| {
                        *b = b.map(|v| v.saturating_sub(t_amount));
                    });
                    OrderTokenUnallocated::<T>::remove(order_id);
                    Self::deposit_event(Event::TokenUnallocatedPoolRefunded {
                        entity_id: te_id, order_id, amount: t_amount,
                    });
                }
            }

            if token_cancelled > 0 {
                Self::deposit_event(Event::TokenCommissionCancelled {
                    order_id, cancelled_count: token_cancelled,
                });
            }

            Ok(())
        }

        /// Token 佣金独立取消（供 TokenCommissionProvider::cancel_token_commission 调用）
        pub fn do_cancel_token_commission(order_id: u64) -> DispatchResult {
            let mut token_cancelled: u32 = 0;
            OrderTokenCommissionRecords::<T>::mutate(order_id, |records| {
                for record in records.iter_mut() {
                    if record.status == CommissionStatus::Pending {
                        MemberTokenCommissionStats::<T>::mutate(
                            record.entity_id, &record.beneficiary, |stats| {
                                stats.pending = stats.pending.saturating_sub(record.amount);
                                stats.total_earned = stats.total_earned.saturating_sub(record.amount);
                            }
                        );
                        TokenPendingTotal::<T>::mutate(record.entity_id, |total| {
                            *total = total.saturating_sub(record.amount);
                        });
                        record.status = CommissionStatus::Cancelled;
                        token_cancelled = token_cancelled.saturating_add(1);
                    }
                }
            });

            // H2 审计修复: 退还 Token 沉淀池 — 仅在转账成功时扣减池余额
            let (te_id, ts_id, t_amount) = OrderTokenUnallocated::<T>::get(order_id);
            if !t_amount.is_zero() {
                let mut refund_ok = false;
                let entity_account = T::EntityProvider::entity_account(te_id);
                if let Some(seller) = T::ShopProvider::shop_owner(ts_id) {
                    if T::TokenTransferProvider::token_transfer(
                        te_id, &entity_account, &seller, t_amount,
                    ).is_ok() {
                        refund_ok = true;
                    }
                }
                if refund_ok {
                    UnallocatedTokenPool::<T>::mutate(te_id, |pool| {
                        *pool = pool.saturating_sub(t_amount);
                    });
                    EntityTokenAccountedBalance::<T>::mutate(te_id, |b| {
                        *b = b.map(|v| v.saturating_sub(t_amount));
                    });
                    OrderTokenUnallocated::<T>::remove(order_id);
                    Self::deposit_event(Event::TokenUnallocatedPoolRefunded {
                        entity_id: te_id, order_id, amount: t_amount,
                    });
                }
            }

            if token_cancelled > 0 {
                Self::deposit_event(Event::TokenCommissionCancelled {
                    order_id, cancelled_count: token_cancelled,
                });
            }

            Ok(())
        }
    }
}

// ============================================================================
// CommissionProvider impl
// ============================================================================

/// CommissionFundGuard: 佣金资金已转入 Entity 账户，Shop 不再持有佣金资金
/// 因此 Entity 的 protected_funds 始终为 0
impl<T: pallet::Config> pallet_entity_common::CommissionFundGuard for pallet::Pallet<T> {
    fn protected_funds(_entity_id: u64) -> u128 {
        0
    }
}

/// CommissionProvider impl: 统一使用 entity_id，无需 shop_id 解析
impl<T: pallet::Config> CommissionProvider<T::AccountId, pallet::BalanceOf<T>> for pallet::Pallet<T> {
    fn process_commission(
        entity_id: u64,
        shop_id: u64,
        order_id: u64,
        buyer: &T::AccountId,
        order_amount: pallet::BalanceOf<T>,
        available_pool: pallet::BalanceOf<T>,
        platform_fee: pallet::BalanceOf<T>,
    ) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::process_commission(entity_id, shop_id, order_id, buyer, order_amount, available_pool, platform_fee)
    }

    fn cancel_commission(order_id: u64) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::cancel_commission(order_id)
    }

    fn pending_commission(entity_id: u64, account: &T::AccountId) -> pallet::BalanceOf<T> {
        pallet::MemberCommissionStats::<T>::get(entity_id, account).pending
    }

    fn set_commission_modes(entity_id: u64, modes: u16) -> sp_runtime::DispatchResult {
        let allowed_modes = CommissionModes::DIRECT_REWARD
            | CommissionModes::MULTI_LEVEL
            | CommissionModes::TEAM_PERFORMANCE
            | CommissionModes::LEVEL_DIFF
            | CommissionModes::FIXED_AMOUNT
            | CommissionModes::FIRST_ORDER
            | CommissionModes::REPEAT_PURCHASE
            | CommissionModes::SINGLE_LINE_UPLINE
            | CommissionModes::SINGLE_LINE_DOWNLINE
            | CommissionModes::POOL_REWARD;
        frame_support::ensure!(modes & !allowed_modes == 0, sp_runtime::DispatchError::Other("InvalidModes"));

        pallet::CommissionConfigs::<T>::mutate(entity_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::CoreCommissionConfig::default);
            config.enabled_modes = CommissionModes(modes);
        });
        Ok(())
    }

    fn set_direct_reward_rate(entity_id: u64, rate: u16) -> sp_runtime::DispatchResult {
        frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        <T as pallet::Config>::ReferralWriter::set_direct_rate(entity_id, rate)
    }

    fn set_level_diff_config(entity_id: u64, level_rates: alloc::vec::Vec<u16>) -> sp_runtime::DispatchResult {
        for &rate in level_rates.iter() {
            frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        }
        let depth = level_rates.len() as u8;
        <T as pallet::Config>::LevelDiffWriter::set_level_rates(entity_id, level_rates, depth)
    }

    fn set_fixed_amount(entity_id: u64, amount: pallet::BalanceOf<T>) -> sp_runtime::DispatchResult {
        <T as pallet::Config>::ReferralWriter::set_fixed_amount(entity_id, amount)
    }

    fn set_first_order_config(entity_id: u64, amount: pallet::BalanceOf<T>, rate: u16, use_amount: bool) -> sp_runtime::DispatchResult {
        frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        <T as pallet::Config>::ReferralWriter::set_first_order(entity_id, amount, rate, use_amount)
    }

    fn set_repeat_purchase_config(entity_id: u64, rate: u16, min_orders: u32) -> sp_runtime::DispatchResult {
        frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        <T as pallet::Config>::ReferralWriter::set_repeat_purchase(entity_id, rate, min_orders)
    }

    fn set_withdrawal_config_by_governance(
        entity_id: u64,
        enabled: bool,
    ) -> sp_runtime::DispatchResult {
        pallet::WithdrawalConfigs::<T>::mutate(entity_id, |maybe| {
            let config = maybe.get_or_insert_with(pallet::EntityWithdrawalConfig::default);
            config.enabled = enabled;
        });
        Ok(())
    }

    fn shopping_balance(entity_id: u64, account: &T::AccountId) -> pallet::BalanceOf<T> {
        pallet::MemberShoppingBalance::<T>::get(entity_id, account)
    }

    fn use_shopping_balance(entity_id: u64, account: &T::AccountId, amount: pallet::BalanceOf<T>) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::do_use_shopping_balance(entity_id, account, amount)
    }

    fn set_min_repurchase_rate(entity_id: u64, rate: u16) -> sp_runtime::DispatchResult {
        frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        pallet::GlobalMinRepurchaseRate::<T>::insert(entity_id, rate);
        Ok(())
    }
}

// ============================================================================
// PoolBalanceProvider 实现（供 pool-reward v2 访问沉淀池余额）
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::PoolBalanceProvider<pallet::BalanceOf<T>>
    for pallet::Pallet<T>
{
    fn pool_balance(entity_id: u64) -> pallet::BalanceOf<T> {
        pallet::UnallocatedPool::<T>::get(entity_id)
    }

    fn deduct_pool(entity_id: u64, amount: pallet::BalanceOf<T>) -> Result<(), sp_runtime::DispatchError> {
        pallet::UnallocatedPool::<T>::try_mutate(entity_id, |pool| {
            frame_support::ensure!(*pool >= amount, sp_runtime::DispatchError::Other("InsufficientPool"));
            *pool -= amount;
            Ok(())
        })
    }
}

// ============================================================================
// TokenPoolBalanceProvider 实现（供 pool-reward 访问 Token 沉淀池）
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::TokenPoolBalanceProvider<pallet::TokenBalanceOf<T>>
    for pallet::Pallet<T>
{
    fn token_pool_balance(entity_id: u64) -> pallet::TokenBalanceOf<T> {
        pallet::UnallocatedTokenPool::<T>::get(entity_id)
    }

    fn deduct_token_pool(entity_id: u64, amount: pallet::TokenBalanceOf<T>) -> Result<(), sp_runtime::DispatchError> {
        pallet::UnallocatedTokenPool::<T>::try_mutate(entity_id, |pool| {
            frame_support::ensure!(*pool >= amount, sp_runtime::DispatchError::Other("InsufficientTokenPool"));
            *pool -= amount;
            Ok(())
        })
    }
}

// ============================================================================
// TokenCommissionProvider 实现（供 transaction 模块调用 Token 佣金管线）
// ============================================================================

use frame_support::traits::Get as GetTrait;

impl<T: pallet::Config> pallet_commission_common::TokenCommissionProvider<T::AccountId, pallet::TokenBalanceOf<T>>
    for pallet::Pallet<T>
{
    fn process_token_commission(
        entity_id: u64,
        shop_id: u64,
        order_id: u64,
        buyer: &T::AccountId,
        token_order_amount: pallet::TokenBalanceOf<T>,
        token_platform_fee: pallet::TokenBalanceOf<T>,
    ) -> Result<(), sp_runtime::DispatchError> {
        pallet::Pallet::<T>::process_token_commission(entity_id, shop_id, order_id, buyer, token_order_amount, token_platform_fee)
    }

    fn cancel_token_commission(order_id: u64) -> Result<(), sp_runtime::DispatchError> {
        pallet::Pallet::<T>::do_cancel_token_commission(order_id)
    }

    fn pending_token_commission(entity_id: u64, account: &T::AccountId) -> pallet::TokenBalanceOf<T> {
        pallet::MemberTokenCommissionStats::<T>::get(entity_id, account).pending
    }

    fn token_platform_fee_rate(_entity_id: u64) -> u16 {
        T::TokenPlatformFeeRate::get()
    }
}
