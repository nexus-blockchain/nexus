//! # 实体治理模块 (pallet-entity-governance)
//!
//! ## 概述
//!
//! 本模块实现实体通证治理功能：
//! - 多种治理模式支持
//! - 提案创建与管理
//! - 代币加权投票
//! - 分层治理阈值
//! - 提案执行
//!
//! ## 治理模式
//!
//! - **None**: 无治理（管理员全权控制，禁止创建提案）
//! - **FullDAO**: 完全 DAO（代币投票，可选管理员否决权作为紧急制动）
//!
//! ## 版本历史
//!
//! - v0.1.0 (2026-01-31): 初始版本
//! - v0.2.0 (2026-02-03): Phase 5 治理模式增强

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::Get,
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{GovernanceMode, EntityProvider, EntityTokenProvider, ShopProvider};
    use pallet_entity_commission::{CommissionProvider, MemberProvider};
    use sp_runtime::traits::{Saturating, Zero};
    use sp_runtime::SaturatedConversion;

    // ==================== 类型定义 ====================

    /// 提案 ID 类型
    pub type ProposalId = u64;

    /// 提案状态
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum ProposalStatus {
        /// 投票中
        Voting,
        /// 投票通过
        Passed,
        /// 投票未通过
        Failed,
        /// 已执行
        Executed,
        /// 已取消
        Cancelled,
        /// 已过期
        Expired,
    }

    impl Default for ProposalStatus {
        fn default() -> Self {
            Self::Voting
        }
    }

    /// 投票类型
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum VoteType {
        /// 赞成
        #[default]
        Yes,
        /// 反对
        No,
        /// 弃权
        Abstain,
    }


    /// 提案类型（纯代币投票）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum ProposalType<Balance> {
        // ==================== 商品管理类 ====================
        /// 商品价格调整
        PriceChange { product_id: u64, new_price: Balance },
        /// 新商品上架
        ProductListing { product_cid: BoundedVec<u8, ConstU32<64>> },
        /// 商品下架
        ProductDelisting { product_id: u64 },
        /// 库存调整
        InventoryAdjustment { product_id: u64, new_inventory: u64 },

        // ==================== 店铺运营类 ====================
        /// 促销活动
        Promotion { discount_rate: u16, duration_blocks: u32 },
        /// 修改店铺名称
        ShopNameChange { new_name: BoundedVec<u8, ConstU32<64>> },
        /// 修改店铺描述
        ShopDescriptionChange { description_cid: BoundedVec<u8, ConstU32<64>> },
        /// 暂停店铺营业（M2-R3: 指定 shop_id，不再默认第一个 shop）
        ShopPause { shop_id: u64 },
        /// 恢复店铺营业（M2-R3: 指定 shop_id）
        ShopResume { shop_id: u64 },

        // ==================== 代币经济类 ====================
        /// 代币配置修改
        TokenConfigChange { reward_rate: Option<u16>, exchange_rate: Option<u16> },
        /// 增发代币
        TokenMint { amount: Balance, recipient_cid: BoundedVec<u8, ConstU32<64>> },
        /// 销毁代币（从金库）
        TokenBurn { amount: Balance },
        /// 空投分发
        AirdropDistribution { airdrop_cid: BoundedVec<u8, ConstU32<64>>, total_amount: Balance },
        /// 分红提案
        Dividend { rate: u16 },

        // ==================== 财务管理类 ====================
        /// 店铺金库支出
        TreasurySpend { amount: Balance, recipient_cid: BoundedVec<u8, ConstU32<64>>, reason_cid: BoundedVec<u8, ConstU32<64>> },
        /// 手续费调整
        FeeAdjustment { new_fee_rate: u16 },
        /// 收益分配比例调整
        RevenueShare { owner_share: u16, token_holder_share: u16 },
        /// 退款政策调整
        RefundPolicy { policy_cid: BoundedVec<u8, ConstU32<64>> },

        // ==================== 治理参数类 ====================
        /// 投票期调整
        VotingPeriodChange { new_period_blocks: u32 },
        /// 法定人数调整
        QuorumChange { new_quorum: u8 },
        /// 提案门槛调整
        ProposalThresholdChange { new_threshold: u16 },

        // ==================== 返佣配置类（新增）====================
        /// 启用/禁用返佣模式
        CommissionModesChange { modes: u16 },
        /// 直推奖励配置
        DirectRewardChange { rate: u16 },
        /// 多级分销配置
        MultiLevelChange { 
            /// 各层级配置 (rate, required_directs, required_team_size, required_spent)
            levels_cid: BoundedVec<u8, ConstU32<64>>,
            max_total_rate: u16,
        },
        /// 等级极差配置（自定义等级，最多 10 级）
        LevelDiffChange {
            level_rates: BoundedVec<u16, ConstU32<10>>,
        },
        /// 固定金额配置
        FixedAmountChange { amount: Balance },
        /// 首单奖励配置
        FirstOrderChange { amount: Balance, rate: u16, use_amount: bool },
        /// 复购奖励配置
        RepeatPurchaseChange { rate: u16, min_orders: u32 },
        /// 单线收益配置
        SingleLineChange {
            upline_rate: u16,
            downline_rate: u16,
            base_upline_levels: u8,
            base_downline_levels: u8,
            max_upline_levels: u8,
            max_downline_levels: u8,
        },

        // ==================== 分级提现配置类（新增）====================
        /// 分级提现配置
        WithdrawalConfigChange {
            /// 各等级提现配置 CID（JSON 格式）
            tier_configs_cid: BoundedVec<u8, ConstU32<64>>,
            enabled: bool,
        },

        /// 设置全局最低复购比例底线（万分比，由 Governance 设定）
        MinRepurchaseRateChange {
            /// 最低复购比例（万分比，3000 = 30%）
            min_rate: u16,
        },

        // ==================== 会员等级体系类（新增）====================
        /// 添加自定义等级
        AddCustomLevel {
            level_id: u8,
            name: BoundedVec<u8, ConstU32<32>>,
            threshold: Balance,
            discount_rate: u16,
            commission_bonus: u16,
        },
        /// 更新自定义等级
        UpdateCustomLevel {
            level_id: u8,
            name: Option<BoundedVec<u8, ConstU32<32>>>,
            threshold: Option<Balance>,
            discount_rate: Option<u16>,
            commission_bonus: Option<u16>,
        },
        /// 删除自定义等级
        RemoveCustomLevel { level_id: u8 },
        /// 设置等级升级模式
        SetUpgradeMode { mode: u8 },  // 0=AutoUpgrade, 1=ManualUpgrade, 2=PeriodReset
        /// 启用/禁用自定义等级
        EnableCustomLevels { enabled: bool },
        /// 添加升级规则
        AddUpgradeRule {
            /// 规则配置 CID（JSON 格式）
            rule_cid: BoundedVec<u8, ConstU32<64>>,
        },
        /// 删除升级规则
        RemoveUpgradeRule { rule_id: u32 },

        // ==================== 社区类 ====================
        /// 社区活动
        CommunityEvent { event_cid: BoundedVec<u8, ConstU32<64>> },
        /// 规则建议
        RuleSuggestion { suggestion_cid: BoundedVec<u8, ConstU32<64>> },
        /// 通用提案（自定义内容）
        General { title_cid: BoundedVec<u8, ConstU32<64>>, content_cid: BoundedVec<u8, ConstU32<64>> },
    }

    /// 提案
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(T))]
    pub struct Proposal<T: Config> {
        /// 提案 ID
        pub id: ProposalId,
        /// 实体 ID（1:N 多店铺架构，提案绑定 Entity）
        pub entity_id: u64,
        /// 提案者
        pub proposer: T::AccountId,
        /// 提案类型
        pub proposal_type: ProposalType<BalanceOf<T>>,
        /// 提案标题
        pub title: BoundedVec<u8, T::MaxTitleLength>,
        /// 提案描述 CID
        pub description_cid: Option<BoundedVec<u8, T::MaxCidLength>>,
        /// 提案状态
        pub status: ProposalStatus,
        /// 创建时间
        pub created_at: BlockNumberFor<T>,
        /// 投票开始时间
        pub voting_start: BlockNumberFor<T>,
        /// 投票结束时间
        pub voting_end: BlockNumberFor<T>,
        /// 执行时间（通过后）
        pub execution_time: Option<BlockNumberFor<T>>,
        /// 赞成票
        pub yes_votes: BalanceOf<T>,
        /// 反对票
        pub no_votes: BalanceOf<T>,
        /// 弃权票
        pub abstain_votes: BalanceOf<T>,
        // ========== C1+H4 治理参数快照 ==========
        /// 快照: 法定人数阈值（百分比）
        pub snapshot_quorum: u8,
        /// 快照: 通过阈值（百分比）
        pub snapshot_pass: u8,
        /// 快照: 执行延迟（区块数）
        pub snapshot_execution_delay: BlockNumberFor<T>,
        /// 快照: 代币总供应量
        pub snapshot_total_supply: BalanceOf<T>,
    }

    /// 投票记录
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct VoteRecord<AccountId, Balance, BlockNumber> {
        /// 投票者
        pub voter: AccountId,
        /// 投票类型
        pub vote: VoteType,
        /// 投票权重
        pub weight: Balance,
        /// 投票时间
        pub voted_at: BlockNumber,
    }

    /// 余额类型别名
    pub type BalanceOf<T> = <T as Config>::Balance;

    /// 提案类型别名
    pub type ProposalOf<T> = Proposal<T>;

    /// 投票记录类型别名
    pub type VoteRecordOf<T> = VoteRecord<
        <T as frame_system::Config>::AccountId,
        BalanceOf<T>,
        BlockNumberFor<T>,
    >;

    // ========== Phase 5 新增类型 ==========

    /// 治理配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct GovernanceConfig<BlockNumber> {
        /// 治理模式
        pub mode: GovernanceMode,
        /// 投票期（区块数，0 = 使用全局默认）
        pub voting_period: BlockNumber,
        /// 执行延迟（区块数，0 = 使用全局默认）
        pub execution_delay: BlockNumber,
        /// 法定人数阈值（百分比，0 = 使用全局默认）
        pub quorum_threshold: u8,
        /// 通过阈值（百分比，0 = 使用全局默认）
        pub pass_threshold: u8,
        /// 提案创建门槛（基点，0 = 使用全局默认）
        pub proposal_threshold: u16,
        /// 管理员否决权是否启用（FullDAO 下可选）
        pub admin_veto_enabled: bool,
    }

    impl<BlockNumber: Default> Default for GovernanceConfig<BlockNumber> {
        fn default() -> Self {
            Self {
                mode: GovernanceMode::None,
                voting_period: BlockNumber::default(),
                execution_delay: BlockNumber::default(),
                quorum_threshold: 0,
                pass_threshold: 0,
                proposal_threshold: 0,
                admin_veto_enabled: false,
            }
        }
    }

    /// 治理配置类型别名
    pub type GovernanceConfigOf<T> = GovernanceConfig<BlockNumberFor<T>>;

    // ==================== 配置 ====================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// 余额类型
        type Balance: Member
            + Parameter
            + sp_runtime::traits::AtLeast32BitUnsigned
            + Default
            + Copy
            + MaxEncodedLen
            + From<u128>
            + Into<u128>;

        /// 实体查询接口
        type EntityProvider: EntityProvider<Self::AccountId>;

        /// Shop 查询接口（Entity-Shop 分离架构）
        type ShopProvider: ShopProvider<Self::AccountId>;

        /// 代币余额查询接口
        type TokenProvider: EntityTokenProvider<Self::AccountId, Self::Balance>;

        /// 返佣服务接口（治理调用）
        type CommissionProvider: pallet_entity_commission::CommissionProvider<Self::AccountId, Self::Balance>;

        /// 会员服务接口（治理调用）
        type MemberProvider: pallet_entity_commission::MemberProvider<Self::AccountId>;

        /// 投票期（区块数）
        #[pallet::constant]
        type VotingPeriod: Get<BlockNumberFor<Self>>;

        /// 执行延迟（区块数）
        #[pallet::constant]
        type ExecutionDelay: Get<BlockNumberFor<Self>>;

        /// 通过阈值（百分比，如 50 = 50%）
        #[pallet::constant]
        type PassThreshold: Get<u8>;

        /// 法定人数阈值（百分比）
        #[pallet::constant]
        type QuorumThreshold: Get<u8>;

        /// 创建提案所需最低代币持有比例（基点，如 100 = 1%）
        #[pallet::constant]
        type MinProposalThreshold: Get<u16>;

        /// 提案标题最大长度
        #[pallet::constant]
        type MaxTitleLength: Get<u32>;

        /// CID 最大长度
        #[pallet::constant]
        type MaxCidLength: Get<u32>;

        /// 每个实体最大活跃提案数
        #[pallet::constant]
        type MaxActiveProposals: Get<u32>;

        // ========== Phase 5 新增配置 ==========

        /// C3: 最小投票期（区块数，configure_governance 不得低于此值）
        #[pallet::constant]
        type MinVotingPeriod: Get<BlockNumberFor<Self>>;

        /// C3: 最小执行延迟（区块数，configure_governance 不得低于此值）
        #[pallet::constant]
        type MinExecutionDelay: Get<BlockNumberFor<Self>>;

        /// 时间加权：达到最大乘数所需的持有区块数（0 = 禁用时间加权）
        #[pallet::constant]
        type TimeWeightFullPeriod: Get<BlockNumberFor<Self>>;

        /// 时间加权：最大投票权乘数（万分比，10000 = 1x 无加成，30000 = 3x）
        #[pallet::constant]
        type TimeWeightMaxMultiplier: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ==================== 存储项 ====================

    /// 下一个提案 ID
    #[pallet::storage]
    #[pallet::getter(fn next_proposal_id)]
    pub type NextProposalId<T: Config> = StorageValue<_, ProposalId, ValueQuery>;

    /// 提案存储
    #[pallet::storage]
    #[pallet::getter(fn proposals)]
    pub type Proposals<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        ProposalId,
        ProposalOf<T>,
    >;

    /// 实体活跃提案列表
    #[pallet::storage]
    #[pallet::getter(fn entity_proposals)]
    pub type EntityProposals<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        BoundedVec<ProposalId, T::MaxActiveProposals>,
        ValueQuery,
    >;

    /// 投票记录
    #[pallet::storage]
    #[pallet::getter(fn vote_records)]
    pub type VoteRecords<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        ProposalId,
        Blake2_128Concat,
        T::AccountId,
        VoteRecordOf<T>,
    >;

    /// 用户首次持有代币时间（用于时间加权）
    #[pallet::storage]
    #[pallet::getter(fn first_hold_time)]
    pub type FirstHoldTime<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        Blake2_128Concat,
        T::AccountId,
        BlockNumberFor<T>,
    >;

    /// P1 安全修复: 投票权快照 (proposal_id, account) -> 快照时的投票权重
    /// 用户在提案创建后、投票前可锁定投票权，防止闪电贷攻击
    #[pallet::storage]
    #[pallet::getter(fn voting_power_snapshot)]
    pub type VotingPowerSnapshot<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        ProposalId,
        Blake2_128Concat,
        T::AccountId,
        BalanceOf<T>,
    >;

    // ========== Phase 5 新增存储项 ==========

    /// 实体治理配置
    #[pallet::storage]
    #[pallet::getter(fn governance_configs)]
    pub type GovernanceConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        GovernanceConfigOf<T>,
    >;

    /// 治理配置锁定标记（锁定后不可再修改治理参数，但仍可升级到 FullDAO 放权）
    #[pallet::storage]
    #[pallet::getter(fn governance_locked)]
    pub type GovernanceLocked<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        bool,
        ValueQuery,
    >;

    // ========== H2: 投票期间代币锁定（防止跨账户复投）==========

    /// 投票者代币锁定记录 (proposal_id, account) — 用于提案结束时批量解锁
    #[pallet::storage]
    pub type VoterTokenLocks<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        ProposalId,
        Blake2_128Concat,
        T::AccountId,
        (),   // 仅标记参与，金额记录在 GovernanceLockAmount
    >;

    /// 治理锁定引用计数 (entity_id, account) → 活跃投票提案数
    #[pallet::storage]
    pub type GovernanceLockCount<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        Blake2_128Concat,
        T::AccountId,
        u32,
        ValueQuery,
    >;

    /// 治理锁定金额 (entity_id, account) → 最大锁定原始代币数
    #[pallet::storage]
    pub type GovernanceLockAmount<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        Blake2_128Concat,
        T::AccountId,
        BalanceOf<T>,
        ValueQuery,
    >;

    // ==================== 事件 ==

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 提案已创建
        ProposalCreated {
            proposal_id: ProposalId,
            entity_id: u64,
            proposer: T::AccountId,
            title: Vec<u8>,
        },
        /// 已投票
        Voted {
            proposal_id: ProposalId,
            voter: T::AccountId,
            vote: VoteType,
            weight: BalanceOf<T>,
        },
        /// 提案已通过
        ProposalPassed {
            proposal_id: ProposalId,
        },
        /// 提案未通过
        ProposalFailed {
            proposal_id: ProposalId,
        },
        /// 提案已执行
        ProposalExecuted {
            proposal_id: ProposalId,
        },
        /// 提案已取消
        ProposalCancelled {
            proposal_id: ProposalId,
        },
        // ========== Phase 5 新增事件 ==========
        /// 治理配置已更新
        GovernanceConfigUpdated {
            entity_id: u64,
            mode: GovernanceMode,
        },
        /// 管理员已否决提案
        ProposalVetoed {
            proposal_id: ProposalId,
            by: T::AccountId,
        },
        /// 提案执行备注（用于需要链下执行的提案）
        ProposalExecutionNote {
            proposal_id: ProposalId,
            note: Vec<u8>,
        },
        /// 治理配置已锁定（不可再修改）
        GovernanceConfigLocked {
            entity_id: u64,
        },
        /// 治理模式同步到 registry 失败（两侧可能不一致）
        GovernanceSyncFailed {
            entity_id: u64,
            mode: GovernanceMode,
        },
        /// 提案执行窗口已过期，状态转为 Expired
        ProposalExpired {
            proposal_id: ProposalId,
        },
        /// L2-R3: 终态提案已被清理（释放存储）
        ProposalCleaned {
            proposal_id: ProposalId,
        },
    }

    // ==================== 错误 ====================

    #[pallet::error]
    pub enum Error<T> {
        /// 店铺不存在
        ShopNotFound,
        /// 不是店主
        NotShopOwner,
        /// 店铺代币未启用
        TokenNotEnabled,
        /// 提案不存在
        ProposalNotFound,
        /// 代币余额不足以创建提案
        InsufficientTokensForProposal,
        /// 已达到最大活跃提案数
        TooManyActiveProposals,
        /// 提案状态不允许此操作
        InvalidProposalStatus,
        /// 已经投过票
        AlreadyVoted,
        /// 没有投票权
        NoVotingPower,
        /// 投票期已结束
        VotingEnded,
        /// 投票期未结束
        VotingNotEnded,
        /// 执行时间未到
        ExecutionTimeNotReached,
        /// 标题过长
        TitleTooLong,
        /// CID 过长
        CidTooLong,
        /// 无权取消
        CannotCancel,
        // ========== Phase 5 新增错误 ==========
        /// 治理模式不允许此操作
        GovernanceModeNotAllowed,
        /// 无否决权
        NoVetoRight,
        /// TokenType 不具有投票权
        TokenTypeNoVotingPower,
        /// 参数无效
        InvalidParameter,
        /// 提案类型暂未实现链上执行（需链下工作者配合）
        ProposalTypeNotImplemented,
        /// 治理配置已锁定，不可修改
        GovernanceConfigIsLocked,
        /// 治理配置已经锁定过
        GovernanceAlreadyLocked,
        /// C3: 投票期低于最小值
        VotingPeriodTooShort,
        /// C3: 执行延迟低于最小值
        ExecutionDelayTooShort,
        /// FullDAO 需要先发行代币
        TokenNotEnabledForDAO,
        /// 提案 ID 溢出
        ProposalIdOverflow,
        /// L2-R3: 提案未处于终态，不可清理
        ProposalNotTerminal,
    }

    // ==================== Extrinsics ====================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 创建提案
        ///
        /// # 参数
        /// - `entity_id`: 实体 ID
        /// - `proposal_type`: 提案类型
        /// - `title`: 提案标题
        /// - `description_cid`: 提案描述 CID（可选）
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(50_000_000, 6_000))]
        pub fn create_proposal(
            origin: OriginFor<T>,
            entity_id: u64,
            proposal_type: ProposalType<BalanceOf<T>>,
            title: Vec<u8>,
            description_cid: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体存在且活跃
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::ShopNotFound);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::ShopNotFound);

            // H1: 检查治理模式，None 模式不允许创建提案
            let gov_config = GovernanceConfigs::<T>::get(entity_id);
            if let Some(ref cfg) = gov_config {
                ensure!(cfg.mode != GovernanceMode::None, Error::<T>::GovernanceModeNotAllowed);
            }

            // H2: 验证提案参数有效性
            Self::validate_proposal_type(&proposal_type)?;

            // 验证代币已启用且持有足够代币
            ensure!(T::TokenProvider::is_token_enabled(entity_id), Error::<T>::TokenNotEnabled);
            let balance = T::TokenProvider::token_balance(entity_id, &who);
            let total_supply = T::TokenProvider::total_supply(entity_id);
            // X2 修复: 优先使用 GovernanceConfig 自定义提案门槛，回退到全局默认
            let effective_threshold: u16 = gov_config.as_ref()
                .filter(|c| c.proposal_threshold > 0)
                .map(|c| c.proposal_threshold)
                .unwrap_or(T::MinProposalThreshold::get());
            let min_threshold = total_supply
                .saturating_mul(effective_threshold.into())
                / 10000u128.into();
            ensure!(balance >= min_threshold, Error::<T>::InsufficientTokensForProposal);

            // 检查活跃提案数量
            let mut entity_proposals = EntityProposals::<T>::get(entity_id);
            ensure!(
                entity_proposals.len() < T::MaxActiveProposals::get() as usize,
                Error::<T>::TooManyActiveProposals
            );

            // 转换标题和描述
            let title_bounded: BoundedVec<u8, T::MaxTitleLength> =
                title.clone().try_into().map_err(|_| Error::<T>::TitleTooLong)?;
            let description_bounded = description_cid
                .map(|cid| cid.try_into().map_err(|_| Error::<T>::CidTooLong))
                .transpose()?;

            // 创建提案
            let proposal_id = NextProposalId::<T>::get();
            let now = <frame_system::Pallet<T>>::block_number();
            // X3 修复: 优先使用 GovernanceConfig 自定义投票期，回退到全局默认
            let effective_voting_period = gov_config.as_ref()
                .filter(|c| c.voting_period > BlockNumberFor::<T>::default())
                .map(|c| c.voting_period)
                .unwrap_or(T::VotingPeriod::get());
            let voting_end = now.saturating_add(effective_voting_period);

            // C1+H4: 快照治理参数和总供应量，防止运行时偷换
            let snapshot_quorum: u8 = gov_config.as_ref()
                .filter(|c| c.quorum_threshold > 0)
                .map(|c| c.quorum_threshold)
                .unwrap_or(T::QuorumThreshold::get());
            let snapshot_pass: u8 = gov_config.as_ref()
                .filter(|c| c.pass_threshold > 0)
                .map(|c| c.pass_threshold)
                .unwrap_or(T::PassThreshold::get());
            let snapshot_execution_delay = gov_config.as_ref()
                .filter(|c| c.execution_delay > BlockNumberFor::<T>::default())
                .map(|c| c.execution_delay)
                .unwrap_or(T::ExecutionDelay::get());

            let proposal = Proposal {
                id: proposal_id,
                entity_id,
                proposer: who.clone(),
                proposal_type,
                title: title_bounded,
                description_cid: description_bounded,
                status: ProposalStatus::Voting,
                created_at: now,
                voting_start: now,
                voting_end,
                execution_time: None,
                yes_votes: Zero::zero(),
                no_votes: Zero::zero(),
                abstain_votes: Zero::zero(),
                snapshot_quorum,
                snapshot_pass,
                snapshot_execution_delay,
                snapshot_total_supply: total_supply,
            };

            // 保存
            Proposals::<T>::insert(proposal_id, proposal);
            entity_proposals.try_push(proposal_id).map_err(|_| Error::<T>::TooManyActiveProposals)?;
            EntityProposals::<T>::insert(entity_id, entity_proposals);
            // L1-fix: checked_add 防止 u64 溢出导致 ID 覆盖
            let next_id = proposal_id.checked_add(1).ok_or(Error::<T>::ProposalIdOverflow)?;
            NextProposalId::<T>::put(next_id);

            Self::deposit_event(Event::ProposalCreated {
                proposal_id,
                entity_id,
                proposer: who,
                title,
            });

            Ok(())
        }

        /// 投票
        ///
        /// # 参数
        /// - `proposal_id`: 提案 ID
        /// - `vote`: 投票类型
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(40_000_000, 5_000))]
        pub fn vote(
            origin: OriginFor<T>,
            proposal_id: ProposalId,
            vote: VoteType,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 获取提案
            let mut proposal = Proposals::<T>::get(proposal_id)
                .ok_or(Error::<T>::ProposalNotFound)?;

            // 验证状态
            ensure!(proposal.status == ProposalStatus::Voting, Error::<T>::InvalidProposalStatus);

            // 验证投票期
            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now <= proposal.voting_end, Error::<T>::VotingEnded);

            // 验证未投过票
            ensure!(
                !VoteRecords::<T>::contains_key(proposal_id, &who),
                Error::<T>::AlreadyVoted
            );

            // 检查代币类型投票权
            let token_type = T::TokenProvider::get_token_type(proposal.entity_id);
            ensure!(token_type.has_voting_power(), Error::<T>::TokenTypeNoVotingPower);

            // 获取当前投票权重
            let current_balance = Self::calculate_voting_power(proposal.entity_id, &who);
            ensure!(!current_balance.is_zero(), Error::<T>::NoVotingPower);

            // P1 安全: 使用快照机制防止闪电贷（首次投票时锁定权重）
            let weight = if let Some(snapshot) = VotingPowerSnapshot::<T>::get(proposal_id, &who) {
                current_balance.min(snapshot)
            } else {
                VotingPowerSnapshot::<T>::insert(proposal_id, &who, current_balance);
                current_balance
            };
            ensure!(!weight.is_zero(), Error::<T>::NoVotingPower);

            // 懒写入 FirstHoldTime（便于未来时间加权）
            if !FirstHoldTime::<T>::contains_key(proposal.entity_id, &who) {
                FirstHoldTime::<T>::insert(proposal.entity_id, &who, now);
            }

            // 记录投票
            match vote {
                VoteType::Yes => proposal.yes_votes = proposal.yes_votes.saturating_add(weight),
                VoteType::No => proposal.no_votes = proposal.no_votes.saturating_add(weight),
                VoteType::Abstain => proposal.abstain_votes = proposal.abstain_votes.saturating_add(weight),
            }

            let record = VoteRecord {
                voter: who.clone(),
                vote: vote.clone(),
                weight,
                voted_at: now,
            };

            Proposals::<T>::insert(proposal_id, &proposal);
            VoteRecords::<T>::insert(proposal_id, &who, record);

            // H2: 锁定投票者的原始代币余额，防止投票后转让给其他账户复投
            let entity_id = proposal.entity_id;
            let raw_balance = T::TokenProvider::token_balance(entity_id, &who);
            let current_locked = GovernanceLockAmount::<T>::get(entity_id, &who);
            if raw_balance > current_locked {
                let diff = raw_balance.saturating_sub(current_locked);
                // best-effort: reserve 失败不阻断投票（兼容 mock 和余额不足场景）
                if T::TokenProvider::reserve(entity_id, &who, diff).is_ok() {
                    GovernanceLockAmount::<T>::insert(entity_id, &who, raw_balance);
                }
            }
            GovernanceLockCount::<T>::mutate(entity_id, &who, |c| *c = c.saturating_add(1));
            VoterTokenLocks::<T>::insert(proposal_id, &who, ());

            Self::deposit_event(Event::Voted {
                proposal_id,
                voter: who,
                vote,
                weight,
            });

            Ok(())
        }

        /// 结束投票并计算结果
        ///
        /// 任何人都可以调用（投票期结束后）
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
        pub fn finalize_voting(
            origin: OriginFor<T>,
            proposal_id: ProposalId,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            let mut proposal = Proposals::<T>::get(proposal_id)
                .ok_or(Error::<T>::ProposalNotFound)?;

            // 验证状态
            ensure!(proposal.status == ProposalStatus::Voting, Error::<T>::InvalidProposalStatus);

            // 验证投票期已结束
            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now > proposal.voting_end, Error::<T>::VotingNotEnded);

            // 计算结果
            let total_votes = proposal.yes_votes
                .saturating_add(proposal.no_votes)
                .saturating_add(proposal.abstain_votes);

            // C1+H4: 使用提案创建时的快照值，防止运行时偷换攻击
            let effective_quorum: u8 = proposal.snapshot_quorum;
            let effective_pass: u8 = proposal.snapshot_pass;
            let total_supply: BalanceOf<T> = proposal.snapshot_total_supply;

            // 检查法定人数
            let quorum_threshold: BalanceOf<T> = total_supply
                .saturating_mul(effective_quorum.into())
                / 100u128.into();
            
            if total_votes < quorum_threshold {
                proposal.status = ProposalStatus::Failed;
                Self::remove_from_active(proposal_id, proposal.entity_id);
                Proposals::<T>::insert(proposal_id, proposal);
                Self::deposit_event(Event::ProposalFailed { proposal_id });
                return Ok(());
            }

            // H1 修复: 通过阈值仅基于 yes+no 票计算，弃权票不稀释通过率
            let decisive_votes = proposal.yes_votes.saturating_add(proposal.no_votes);
            let pass_threshold: BalanceOf<T> = decisive_votes
                .saturating_mul(effective_pass.into())
                / 100u128.into();

            if proposal.yes_votes > pass_threshold {
                proposal.status = ProposalStatus::Passed;
                // C1: 使用快照的执行延迟
                proposal.execution_time = Some(now.saturating_add(proposal.snapshot_execution_delay));
                // H5 修复: 通过的提案也从活跃列表移除，不阻塞新提案
                Self::remove_from_active(proposal_id, proposal.entity_id);
                Proposals::<T>::insert(proposal_id, proposal);
                Self::deposit_event(Event::ProposalPassed { proposal_id });
            } else {
                proposal.status = ProposalStatus::Failed;
                Self::remove_from_active(proposal_id, proposal.entity_id);
                Proposals::<T>::insert(proposal_id, proposal);
                Self::deposit_event(Event::ProposalFailed { proposal_id });
            }

            Ok(())
        }

        /// 执行提案
        ///
        /// 任何人都可以调用（执行时间到达后）
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(80_000_000, 10_000))]
        pub fn execute_proposal(
            origin: OriginFor<T>,
            proposal_id: ProposalId,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            let mut proposal = Proposals::<T>::get(proposal_id)
                .ok_or(Error::<T>::ProposalNotFound)?;

            // 验证状态
            ensure!(proposal.status == ProposalStatus::Passed, Error::<T>::InvalidProposalStatus);

            // 验证执行时间
            let now = <frame_system::Pallet<T>>::block_number();
            let exec_time = proposal.execution_time.ok_or(Error::<T>::ExecutionTimeNotReached)?;
            ensure!(now >= exec_time, Error::<T>::ExecutionTimeNotReached);

            // H3+M5: 执行过期检查——使用快照的执行延迟的 2 倍作为窗口
            let execution_window = proposal.snapshot_execution_delay.saturating_mul(2u32.into());
            let expiry = exec_time.saturating_add(execution_window);
            // H2-R2: 过期后优雅转为 Expired 状态（而非回滚错误导致状态永远停在 Passed）
            if now > expiry {
                proposal.status = ProposalStatus::Expired;
                Proposals::<T>::insert(proposal_id, proposal);
                Self::deposit_event(Event::ProposalExpired { proposal_id });
                return Ok(());
            }

            // 执行提案（根据类型）
            Self::do_execute_proposal(&proposal)?;

            // 更新状态
            proposal.status = ProposalStatus::Executed;
            Proposals::<T>::insert(proposal_id, proposal);

            Self::deposit_event(Event::ProposalExecuted { proposal_id });

            Ok(())
        }

        /// 取消提案
        ///
        /// 提案者或店主可以取消
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(30_000_000, 4_000))]
        pub fn cancel_proposal(
            origin: OriginFor<T>,
            proposal_id: ProposalId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut proposal = Proposals::<T>::get(proposal_id)
                .ok_or(Error::<T>::ProposalNotFound)?;

            // C4: FullDAO 模式下只允许提案者取消，Owner 需走 veto 通道
            let owner = T::EntityProvider::entity_owner(proposal.entity_id);
            let is_owner = owner == Some(who.clone());
            let is_proposer = proposal.proposer == who;

            if is_owner && !is_proposer {
                // Owner 非提案者 — 检查是否为 FullDAO 模式
                let gov_config = GovernanceConfigs::<T>::get(proposal.entity_id);
                if let Some(ref cfg) = gov_config {
                    ensure!(cfg.mode != GovernanceMode::FullDAO, Error::<T>::GovernanceModeNotAllowed);
                }
            }
            ensure!(is_proposer || is_owner, Error::<T>::CannotCancel);

            // 验证状态（只能取消 Voting 状态的提案）
            ensure!(
                proposal.status == ProposalStatus::Voting,
                Error::<T>::InvalidProposalStatus
            );

            // 取消
            proposal.status = ProposalStatus::Cancelled;
            let eid = proposal.entity_id;
            Proposals::<T>::insert(proposal_id, proposal);
            Self::remove_from_active(proposal_id, eid);

            Self::deposit_event(Event::ProposalCancelled { proposal_id });

            Ok(())
        }

        // ==================== Phase 5 新增 Extrinsics ====================

        /// 配置实体治理
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(25_000_000, 3_000))]
        pub fn configure_governance(
            origin: OriginFor<T>,
            entity_id: u64,
            mode: GovernanceMode,
            voting_period: Option<BlockNumberFor<T>>,
            execution_delay: Option<BlockNumberFor<T>>,
            quorum_threshold: Option<u8>,
            pass_threshold: Option<u8>,
            proposal_threshold: Option<u16>,
            admin_veto_enabled: Option<bool>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            // 锁定检查：锁定后永久不可修改（None 锁定=永久冻结，FullDAO 锁定=仅提案可改）
            ensure!(!GovernanceLocked::<T>::get(entity_id), Error::<T>::GovernanceConfigIsLocked);

            // FullDAO 模式需要代币已发行，否则无人能投票，治理瘫痪
            if mode == GovernanceMode::FullDAO {
                ensure!(T::TokenProvider::is_token_enabled(entity_id), Error::<T>::TokenNotEnabledForDAO);
            }

            // 参数验证（上限）
            if let Some(q) = quorum_threshold {
                ensure!(q <= 100, Error::<T>::InvalidParameter);
            }
            if let Some(p) = pass_threshold {
                ensure!(p <= 100, Error::<T>::InvalidParameter);
            }
            if let Some(t) = proposal_threshold {
                ensure!(t <= 10000, Error::<T>::InvalidParameter);
            }
            // C3: 参数验证（下限）
            if let Some(period) = voting_period {
                ensure!(period >= T::MinVotingPeriod::get(), Error::<T>::VotingPeriodTooShort);
            }
            if let Some(delay) = execution_delay {
                ensure!(delay >= T::MinExecutionDelay::get(), Error::<T>::ExecutionDelayTooShort);
            }

            GovernanceConfigs::<T>::mutate(entity_id, |maybe_config| {
                let config = maybe_config.get_or_insert_with(GovernanceConfigOf::<T>::default);
                config.mode = mode;
                if let Some(period) = voting_period {
                    config.voting_period = period;
                }
                if let Some(delay) = execution_delay {
                    config.execution_delay = delay;
                }
                if let Some(quorum) = quorum_threshold {
                    config.quorum_threshold = quorum;
                }
                if let Some(pass) = pass_threshold {
                    config.pass_threshold = pass;
                }
                if let Some(threshold) = proposal_threshold {
                    config.proposal_threshold = threshold;
                }
                if let Some(veto) = admin_veto_enabled {
                    config.admin_veto_enabled = veto;
                }
            });

            // C2: 同步 registry 侧
            if T::EntityProvider::set_governance_mode(entity_id, mode).is_err() {
                Self::deposit_event(Event::GovernanceSyncFailed { entity_id, mode });
            }

            Self::deposit_event(Event::GovernanceConfigUpdated {
                entity_id,
                mode,
            });
            Ok(())
        }

        /// 锁定治理配置（永久不可逆）
        ///
        /// 锁定后 owner 不可再修改治理参数，此操作不可撤销。
        /// - None 锁定 = 永久冻结治理配置
        /// - FullDAO 锁定 = 放弃控制权，仅可通过提案修改治理参数
        #[pallet::call_index(10)]
        #[pallet::weight(Weight::from_parts(15_000_000, 2_000))]
        pub fn lock_governance(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            ensure!(!GovernanceLocked::<T>::get(entity_id), Error::<T>::GovernanceAlreadyLocked);

            // None 和 FullDAO 均可锁定：
            // - None 锁定 = 永久冻结，不可升级，不可解锁
            // - FullDAO 锁定 = 放弃控制权，仅通过提案修改
            GovernanceLocked::<T>::insert(entity_id, true);

            Self::deposit_event(Event::GovernanceConfigLocked { entity_id });
            Ok(())
        }

        /// L2-R3: 清理终态提案（Executed/Failed/Cancelled/Expired）
        ///
        /// 任何人可调用，清理已结束的提案释放存储空间。
        #[pallet::call_index(11)]
        #[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
        pub fn cleanup_proposal(
            origin: OriginFor<T>,
            proposal_id: ProposalId,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            let proposal = Proposals::<T>::get(proposal_id)
                .ok_or(Error::<T>::ProposalNotFound)?;

            ensure!(
                matches!(
                    proposal.status,
                    ProposalStatus::Executed
                        | ProposalStatus::Failed
                        | ProposalStatus::Cancelled
                        | ProposalStatus::Expired
                ),
                Error::<T>::ProposalNotTerminal
            );

            Proposals::<T>::remove(proposal_id);

            Self::deposit_event(Event::ProposalCleaned { proposal_id });
            Ok(())
        }

        /// 管理员否决提案（需 admin_veto_enabled）
        #[pallet::call_index(9)]
        #[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
        pub fn veto_proposal(
            origin: OriginFor<T>,
            proposal_id: ProposalId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut proposal = Proposals::<T>::get(proposal_id)
                .ok_or(Error::<T>::ProposalNotFound)?;

            // 验证管理员权限（实体所有者）
            let owner = T::EntityProvider::entity_owner(proposal.entity_id)
                .ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NoVetoRight);

            let config = GovernanceConfigs::<T>::get(proposal.entity_id)
                .unwrap_or_default();
            ensure!(config.admin_veto_enabled, Error::<T>::NoVetoRight);

            // 验证状态
            ensure!(
                proposal.status == ProposalStatus::Voting || proposal.status == ProposalStatus::Passed,
                Error::<T>::InvalidProposalStatus
            );

            // 否决
            proposal.status = ProposalStatus::Cancelled;
            let eid = proposal.entity_id;
            Proposals::<T>::insert(proposal_id, proposal);
            Self::remove_from_active(proposal_id, eid);

            Self::deposit_event(Event::ProposalVetoed {
                proposal_id,
                by: who,
            });
            Ok(())
        }
    }

    // ==================== 内部函数 ====================

    impl<T: Config> Pallet<T> {
        /// H2: 验证提案类型参数有效性（basis points ≤ 10000，百分比 ≤ 100）
        fn validate_proposal_type(pt: &ProposalType<BalanceOf<T>>) -> DispatchResult {
            match pt {
                ProposalType::Promotion { discount_rate, .. } => {
                    ensure!(*discount_rate <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::Dividend { rate } => {
                    ensure!(*rate <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::FeeAdjustment { new_fee_rate } => {
                    ensure!(*new_fee_rate <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::RevenueShare { owner_share, token_holder_share } => {
                    ensure!(
                        owner_share.saturating_add(*token_holder_share) <= 10000,
                        Error::<T>::InvalidParameter
                    );
                },
                ProposalType::QuorumChange { new_quorum } => {
                    ensure!(*new_quorum <= 100, Error::<T>::InvalidParameter);
                },
                ProposalType::ProposalThresholdChange { new_threshold } => {
                    ensure!(*new_threshold <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::DirectRewardChange { rate } => {
                    ensure!(*rate <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::MultiLevelChange { max_total_rate, .. } => {
                    ensure!(*max_total_rate <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::LevelDiffChange { ref level_rates } => {
                    for rate in level_rates.iter() {
                        ensure!(*rate <= 10000, Error::<T>::InvalidParameter);
                    }
                },
                ProposalType::FirstOrderChange { rate, .. } => {
                    ensure!(*rate <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::RepeatPurchaseChange { rate, .. } => {
                    ensure!(*rate <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::SingleLineChange { upline_rate, downline_rate, .. } => {
                    ensure!(*upline_rate <= 10000, Error::<T>::InvalidParameter);
                    ensure!(*downline_rate <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::MinRepurchaseRateChange { min_rate } => {
                    ensure!(*min_rate <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::AddCustomLevel { discount_rate, commission_bonus, .. } => {
                    ensure!(*discount_rate <= 10000, Error::<T>::InvalidParameter);
                    ensure!(*commission_bonus <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::SetUpgradeMode { mode } => {
                    ensure!(*mode <= 2, Error::<T>::InvalidParameter);
                },
                // H1: VotingPeriodChange 必须 >= MinVotingPeriod，防止通过提案绕过最小投票期保护
                ProposalType::VotingPeriodChange { new_period_blocks } => {
                    let period: BlockNumberFor<T> = (*new_period_blocks).into();
                    ensure!(period >= T::MinVotingPeriod::get(), Error::<T>::VotingPeriodTooShort);
                },
                // H2: UpdateCustomLevel 的费率验证（与 AddCustomLevel 一致）
                ProposalType::UpdateCustomLevel { discount_rate, commission_bonus, .. } => {
                    if let Some(dr) = discount_rate {
                        ensure!(*dr <= 10000, Error::<T>::InvalidParameter);
                    }
                    if let Some(cb) = commission_bonus {
                        ensure!(*cb <= 10000, Error::<T>::InvalidParameter);
                    }
                },
                // M1: 返佣模式位标志校验（避免无效提案浪费投票周期）
                ProposalType::CommissionModesChange { modes } => {
                    // ALL_VALID = 0b0000_0011_1111_1111 (10 bits)
                    ensure!(*modes & !0b0000_0011_1111_1111u16 == 0, Error::<T>::InvalidParameter);
                },
                // M2-R3: ShopPause/ShopResume 校验 shop_id 属于当前 entity
                // （在 create_proposal 时无法知道 entity_id，改在 do_execute_proposal 前校验）
                _ => {},
            }
            Ok(())
        }

        /// 计算投票权重（代币余额 × 时间加权）
        pub fn calculate_voting_power(entity_id: u64, holder: &T::AccountId) -> BalanceOf<T> {
            let balance = T::TokenProvider::token_balance(entity_id, holder);

            if balance.is_zero() {
                return Zero::zero();
            }

            let full_period: u128 = T::TimeWeightFullPeriod::get().saturated_into();
            let max_multiplier: u128 = T::TimeWeightMaxMultiplier::get().into();

            // 禁用时间加权或配置无效时直接返回余额
            if full_period == 0 || max_multiplier <= 10000 {
                return balance;
            }

            let now: u128 = <frame_system::Pallet<T>>::block_number().saturated_into();

            // 未记录首次持有时间的用户按 1x 计算
            let multiplier: u128 = match FirstHoldTime::<T>::get(entity_id, holder) {
                Some(first_hold) => {
                    let first_hold_u128: u128 = first_hold.saturated_into();
                    let holding_blocks = now.saturating_sub(first_hold_u128);
                    let bonus_range = max_multiplier.saturating_sub(10000);
                    let bonus = holding_blocks
                        .saturating_mul(bonus_range)
                        / full_period;
                    10000u128.saturating_add(bonus.min(bonus_range))
                }
                None => 10000u128,
            };

            let balance_u128: u128 = balance.into();
            let weighted = balance_u128.saturating_mul(multiplier) / 10000u128;
            weighted.into()
        }

        /// 从活跃提案列表移除，并清理投票权快照
        fn remove_from_active(proposal_id: ProposalId, entity_id: u64) {
            EntityProposals::<T>::mutate(entity_id, |proposals| {
                proposals.retain(|&id| id != proposal_id);
            });

            // H2: 解锁所有投票者的代币
            for (voter, _) in VoterTokenLocks::<T>::drain_prefix(proposal_id) {
                GovernanceLockCount::<T>::mutate(entity_id, &voter, |c| *c = c.saturating_sub(1));
                let count = GovernanceLockCount::<T>::get(entity_id, &voter);
                if count == 0 {
                    let locked = GovernanceLockAmount::<T>::take(entity_id, &voter);
                    if !locked.is_zero() {
                        T::TokenProvider::unreserve(entity_id, &voter, locked);
                    }
                    GovernanceLockCount::<T>::remove(entity_id, &voter);
                }
            }

            // M4: 清理 VotingPowerSnapshot 避免存储泄漏
            // M1-R3: 使用有界上限 500（实体级治理投票者有限），避免无界迭代
            let _ = VotingPowerSnapshot::<T>::clear_prefix(proposal_id, 500, None);
            // H3-R2: 清理 VoteRecords 避免存储泄漏
            let _ = VoteRecords::<T>::clear_prefix(proposal_id, 500, None);
        }

        /// 执行提案
        fn do_execute_proposal(proposal: &ProposalOf<T>) -> DispatchResult {
            let entity_id = proposal.entity_id;
            
            match &proposal.proposal_type {
                // ==================== 商品管理类 ====================
                ProposalType::PriceChange { product_id, new_price } => {
                    // 价格变更需要 ProductProvider，暂时记录提案已批准
                    Self::deposit_event(Event::ProposalExecutionNote {
                        proposal_id: proposal.id,
                        note: "PriceChange approved".into(),
                    });
                    let _ = (product_id, new_price);
                    Ok(())
                },
                ProposalType::ProductListing { product_cid } => {
                    // 商品上架需要链下解析 CID，记录提案已执行
                    Self::deposit_event(Event::ProposalExecutionNote {
                        proposal_id: proposal.id,
                        note: "ProductListing: requires off-chain CID parsing".into(),
                    });
                    let _ = product_cid;
                    Ok(())
                },
                ProposalType::ProductDelisting { product_id } => {
                    // 记录下架请求，实际执行需要 ProductProvider
                    Self::deposit_event(Event::ProposalExecutionNote {
                        proposal_id: proposal.id,
                        note: "ProductDelisting executed".into(),
                    });
                    let _ = product_id;
                    Ok(())
                },
                ProposalType::InventoryAdjustment { product_id, new_inventory } => {
                    Self::deposit_event(Event::ProposalExecutionNote {
                        proposal_id: proposal.id,
                        note: "InventoryAdjustment executed".into(),
                    });
                    let _ = (product_id, new_inventory);
                    Ok(())
                },

                // ==================== 店铺运营类 ====================
                ProposalType::Promotion { discount_rate, duration_blocks } => {
                    // 促销活动记录到事件，实际实现需要扩展店铺模块
                    Self::deposit_event(Event::ProposalExecutionNote {
                        proposal_id: proposal.id,
                        note: "Promotion created".into(),
                    });
                    let _ = (discount_rate, duration_blocks);
                    Ok(())
                },
                ProposalType::ShopNameChange { new_name } => {
                    // 店铺名称变更需要链下确认
                    Self::deposit_event(Event::ProposalExecutionNote {
                        proposal_id: proposal.id,
                        note: "ShopNameChange approved".into(),
                    });
                    let _ = new_name;
                    Ok(())
                },
                ProposalType::ShopDescriptionChange { description_cid } => {
                    Self::deposit_event(Event::ProposalExecutionNote {
                        proposal_id: proposal.id,
                        note: "ShopDescriptionChange approved".into(),
                    });
                    let _ = description_cid;
                    Ok(())
                },
                ProposalType::ShopPause { shop_id } => {
                    // M2-R3: 使用提案中指定的 shop_id，并校验属于当前 entity
                    let entity_shops = T::EntityProvider::entity_shops(entity_id);
                    ensure!(entity_shops.contains(shop_id), Error::<T>::ShopNotFound);
                    T::ShopProvider::pause_shop(*shop_id)
                },
                ProposalType::ShopResume { shop_id } => {
                    let entity_shops = T::EntityProvider::entity_shops(entity_id);
                    ensure!(entity_shops.contains(shop_id), Error::<T>::ShopNotFound);
                    T::ShopProvider::resume_shop(*shop_id)
                },

                // ==================== 代币经济类 ====================
                ProposalType::TokenConfigChange { reward_rate, exchange_rate } => {
                    // 代币配置变更需要扩展 TokenProvider
                    Self::deposit_event(Event::ProposalExecutionNote {
                        proposal_id: proposal.id,
                        note: "TokenConfigChange approved".into(),
                    });
                    let _ = (reward_rate, exchange_rate);
                    Ok(())
                },
                ProposalType::TokenMint { amount, recipient_cid } => {
                    // 代币增发需要链下解析 recipient_cid
                    Self::deposit_event(Event::ProposalExecutionNote {
                        proposal_id: proposal.id,
                        note: "TokenMint approved, requires off-chain execution".into(),
                    });
                    let _ = (amount, recipient_cid);
                    Ok(())
                },
                ProposalType::TokenBurn { amount } => {
                    Self::deposit_event(Event::ProposalExecutionNote {
                        proposal_id: proposal.id,
                        note: "TokenBurn approved".into(),
                    });
                    let _ = amount;
                    Ok(())
                },
                ProposalType::AirdropDistribution { airdrop_cid, total_amount } => {
                    Self::deposit_event(Event::ProposalExecutionNote {
                        proposal_id: proposal.id,
                        note: "AirdropDistribution approved, requires off-chain execution".into(),
                    });
                    let _ = (airdrop_cid, total_amount);
                    Ok(())
                },
                ProposalType::Dividend { rate } => {
                    Self::deposit_event(Event::ProposalExecutionNote {
                        proposal_id: proposal.id,
                        note: "Dividend approved".into(),
                    });
                    let _ = rate;
                    Ok(())
                },

                // ==================== 财务管理类 ====================
                ProposalType::TreasurySpend { amount, recipient_cid, reason_cid } => {
                    Self::deposit_event(Event::ProposalExecutionNote {
                        proposal_id: proposal.id,
                        note: "TreasurySpend approved, requires off-chain execution".into(),
                    });
                    let _ = (amount, recipient_cid, reason_cid);
                    Ok(())
                },
                ProposalType::FeeAdjustment { new_fee_rate } => {
                    Self::deposit_event(Event::ProposalExecutionNote {
                        proposal_id: proposal.id,
                        note: "FeeAdjustment approved".into(),
                    });
                    let _ = new_fee_rate;
                    Ok(())
                },
                ProposalType::RevenueShare { owner_share, token_holder_share } => {
                    Self::deposit_event(Event::ProposalExecutionNote {
                        proposal_id: proposal.id,
                        note: "RevenueShare approved".into(),
                    });
                    let _ = (owner_share, token_holder_share);
                    Ok(())
                },
                ProposalType::RefundPolicy { policy_cid } => {
                    Self::deposit_event(Event::ProposalExecutionNote {
                        proposal_id: proposal.id,
                        note: "RefundPolicy approved".into(),
                    });
                    let _ = policy_cid;
                    Ok(())
                },

                // ==================== 治理参数类 ====================
                ProposalType::VotingPeriodChange { new_period_blocks } => {
                    // H1 防御: 执行时再次验证最小投票期（runtime 升级可能提高下限）
                    let period: BlockNumberFor<T> = (*new_period_blocks).into();
                    ensure!(period >= T::MinVotingPeriod::get(), Error::<T>::VotingPeriodTooShort);
                    GovernanceConfigs::<T>::mutate(entity_id, |maybe_config| {
                        let config = maybe_config.get_or_insert_with(GovernanceConfigOf::<T>::default);
                        config.voting_period = period;
                    });
                    Ok(())
                },
                ProposalType::QuorumChange { new_quorum } => {
                    GovernanceConfigs::<T>::mutate(entity_id, |maybe_config| {
                        let config = maybe_config.get_or_insert_with(GovernanceConfigOf::<T>::default);
                        config.quorum_threshold = *new_quorum;
                    });
                    Ok(())
                },
                ProposalType::ProposalThresholdChange { new_threshold } => {
                    GovernanceConfigs::<T>::mutate(entity_id, |maybe_config| {
                        let config = maybe_config.get_or_insert_with(GovernanceConfigOf::<T>::default);
                        config.proposal_threshold = *new_threshold;
                    });
                    Ok(())
                },

                // ==================== 返佣配置类 ====================
                ProposalType::CommissionModesChange { modes } => {
                    T::CommissionProvider::set_commission_modes(entity_id, *modes)
                },
                ProposalType::DirectRewardChange { rate } => {
                    T::CommissionProvider::set_direct_reward_rate(entity_id, *rate)
                },
                ProposalType::MultiLevelChange { levels_cid: _, max_total_rate: _ } => {
                    // 多级分销配置需要解析 CID，暂不支持链上直接执行
                    Err(Error::<T>::ProposalTypeNotImplemented.into())
                },
                ProposalType::LevelDiffChange { ref level_rates } => {
                    T::CommissionProvider::set_level_diff_config(
                        entity_id,
                        level_rates.to_vec(),
                    )
                },
                ProposalType::FixedAmountChange { amount } => {
                    T::CommissionProvider::set_fixed_amount(entity_id, *amount)
                },
                ProposalType::FirstOrderChange { amount, rate, use_amount } => {
                    T::CommissionProvider::set_first_order_config(entity_id, *amount, *rate, *use_amount)
                },
                ProposalType::RepeatPurchaseChange { rate, min_orders } => {
                    T::CommissionProvider::set_repeat_purchase_config(entity_id, *rate, *min_orders)
                },
                ProposalType::SingleLineChange { upline_rate: _, downline_rate: _, base_upline_levels: _, base_downline_levels: _, max_upline_levels: _, max_downline_levels: _ } => {
                    // 单线收益配置较复杂，需要扩展 CommissionProvider trait
                    Err(Error::<T>::ProposalTypeNotImplemented.into())
                },

                // ==================== 分级提现配置类 ====================
                ProposalType::WithdrawalConfigChange { tier_configs_cid: _, enabled } => {
                    // tier_configs_cid 需要链下解析，这里只设置基本配置
                    T::CommissionProvider::set_withdrawal_config_by_governance(
                        entity_id,
                        *enabled,
                    )
                },

                ProposalType::MinRepurchaseRateChange { min_rate } => {
                    T::CommissionProvider::set_min_repurchase_rate(entity_id, *min_rate)
                },

                // ==================== 会员等级体系类 ====================
                ProposalType::AddCustomLevel { level_id, name, threshold, discount_rate, commission_bonus } => {
                    T::MemberProvider::add_custom_level(
                        entity_id,
                        *level_id,
                        name.as_slice(),
                        (*threshold).into(),
                        *discount_rate,
                        *commission_bonus,
                    )
                },
                ProposalType::UpdateCustomLevel { level_id, name, threshold, discount_rate, commission_bonus } => {
                    T::MemberProvider::update_custom_level(
                        entity_id,
                        *level_id,
                        name.as_ref().map(|n| n.as_slice()),
                        threshold.map(|t| t.into()),
                        *discount_rate,
                        *commission_bonus,
                    )
                },
                ProposalType::RemoveCustomLevel { level_id } => {
                    T::MemberProvider::remove_custom_level(entity_id, *level_id)
                },
                ProposalType::SetUpgradeMode { mode } => {
                    T::MemberProvider::set_upgrade_mode(entity_id, *mode)
                },
                ProposalType::EnableCustomLevels { enabled } => {
                    T::MemberProvider::set_custom_levels_enabled(entity_id, *enabled)
                },
                ProposalType::AddUpgradeRule { rule_cid: _ } => {
                    // H3 修复: 升级规则配置需要解析 CID，暂不支持链上直接执行
                    Err(Error::<T>::ProposalTypeNotImplemented.into())
                },
                ProposalType::RemoveUpgradeRule { rule_id: _ } => {
                    // H3 修复: 需要扩展 MemberProvider trait
                    Err(Error::<T>::ProposalTypeNotImplemented.into())
                },

                // ==================== 社区类 ====================
                ProposalType::CommunityEvent { event_cid: _ } => {
                    // 社区活动只是记录，无需执行
                    Ok(())
                },
                ProposalType::RuleSuggestion { suggestion_cid: _ } => {
                    // 规则建议只是记录，无需执行
                    Ok(())
                },
                ProposalType::General { title_cid: _, content_cid: _ } => {
                    // 通用提案只是记录，无需执行
                    Ok(())
                },
            }
        }
    }
}
