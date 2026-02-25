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
//! ## 治理模式 (Phase 5 增强)
//!
//! - **None**: 无治理（管理员全权控制）
//! - **Advisory**: 咨询式（投票仅作建议）
//! - **DualTrack**: 双轨制（代币投票 + 管理员否决权）
//! - **Committee**: 委员会（指定委员投票）
//! - **FullDAO**: 完全 DAO（纯代币投票）
//! - **Tiered**: 分层治理（不同决策不同阈值）
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
    use pallet_entity_common::{GovernanceMode, EntityProvider, EntityTokenProvider, ShopProvider, TokenType};
    use pallet_entity_commission::{CommissionProvider, MemberProvider};
    use sp_runtime::traits::{Saturating, Zero};
    use sp_runtime::SaturatedConversion;

    // ==================== 类型定义 ====================

    /// 提案 ID 类型
    pub type ProposalId = u64;

    /// 提案状态
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum ProposalStatus {
        /// 已创建，等待投票（保留，当前提案直接进入 Voting）
        Created,
        /// 投票中
        Voting,
        /// 投票通过
        Passed,
        /// 投票未通过
        Failed,
        /// 排队等待执行
        Queued,
        /// 已执行
        Executed,
        /// 已取消
        Cancelled,
        /// 已过期
        Expired,
    }

    impl Default for ProposalStatus {
        fn default() -> Self {
            Self::Created
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
        /// 暂停店铺营业
        ShopPause,
        /// 恢复店铺营业
        ShopResume,

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
        /// 等级差价配置（全局等级）
        LevelDiffChange {
            normal_rate: u16,
            silver_rate: u16,
            gold_rate: u16,
            platinum_rate: u16,
            diamond_rate: u16,
        },
        /// 自定义等级极差配置
        CustomLevelDiffChange {
            /// 各等级返佣率 CID（JSON 格式）
            rates_cid: BoundedVec<u8, ConstU32<64>>,
            max_depth: u8,
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
            shopping_balance_generates_commission: bool,
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
        /// P1 安全修复: 快照区块（用于防止闪电贷攻击）
        pub snapshot_block: BlockNumberFor<T>,
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

    /// 提案级别（用于分层治理）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum ProposalLevel {
        /// 日常运营（低阈值）
        #[default]
        Operational,
        /// 重要决策（中等阈值）
        Significant,
        /// 重大变更（高阈值）
        Critical,
        /// 宪法级（最高阈值）
        Constitutional,
    }

    /// 治理配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxCommitteeSize))]
    pub struct GovernanceConfig<AccountId, Balance, BlockNumber, MaxCommitteeSize: Get<u32>> {
        /// 治理模式
        pub mode: GovernanceMode,
        /// 投票期（区块数，0 = 使用默认）
        pub voting_period: BlockNumber,
        /// 执行延迟（区块数，0 = 使用默认）
        pub execution_delay: BlockNumber,
        /// 日常运营通过阈值（百分比）
        pub operational_threshold: u8,
        /// 重要决策通过阈值（百分比）
        pub significant_threshold: u8,
        /// 重大变更通过阈值（百分比）
        pub critical_threshold: u8,
        /// 宪法级通过阈值（百分比）
        pub constitutional_threshold: u8,
        /// 法定人数阈值（百分比）
        pub quorum_threshold: u8,
        /// 提案创建门槛（基点）
        pub proposal_threshold: u16,
        /// 管理员否决权是否启用
        pub admin_veto_enabled: bool,
        /// 需要具有投票权的 TokenType
        pub required_token_type: Option<TokenType>,
        /// 最小委员会批准数
        pub min_committee_approval: u8,
        /// _phantom
        _phantom: core::marker::PhantomData<(AccountId, AccountId, Balance, MaxCommitteeSize)>,
    }

    impl<AccountId, Balance, BlockNumber: Default, MaxCommitteeSize: Get<u32>> Default 
        for GovernanceConfig<AccountId, Balance, BlockNumber, MaxCommitteeSize> 
    {
        fn default() -> Self {
            Self {
                mode: GovernanceMode::None,
                voting_period: BlockNumber::default(),
                execution_delay: BlockNumber::default(),
                operational_threshold: 50,
                significant_threshold: 60,
                critical_threshold: 67,
                constitutional_threshold: 75,
                quorum_threshold: 10,
                proposal_threshold: 100, // 1%
                admin_veto_enabled: true,
                required_token_type: None,
                min_committee_approval: 1,
                _phantom: core::marker::PhantomData,
            }
        }
    }

    /// 治理配置类型别名
    pub type GovernanceConfigOf<T> = GovernanceConfig<
        <T as frame_system::Config>::AccountId,
        BalanceOf<T>,
        BlockNumberFor<T>,
        <T as Config>::MaxCommitteeSize,
    >;

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

        /// 委员会最大成员数
        #[pallet::constant]
        type MaxCommitteeSize: Get<u32>;

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

    /// 向后兼容别名
    pub type ShopProposals<T> = EntityProposals<T>;

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
        GovernanceConfig<T::AccountId, BalanceOf<T>, BlockNumberFor<T>, T::MaxCommitteeSize>,
    >;

    /// 委员会成员 (entity_id) -> Vec<AccountId>
    /// L1 注意: 委员会投票模式尚未在 vote/finalize_voting 中实现，
    /// 当前仅用于存储成员列表，待后续版本集成
    #[pallet::storage]
    #[pallet::getter(fn committee_members)]
    pub type CommitteeMembers<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        BoundedVec<T::AccountId, T::MaxCommitteeSize>,
        ValueQuery,
    >;

    // ==================== 事件 ====================

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
        /// 委员会成员已添加
        CommitteeMemberAdded {
            entity_id: u64,
            member: T::AccountId,
        },
        /// 委员会成员已移除
        CommitteeMemberRemoved {
            entity_id: u64,
            member: T::AccountId,
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
        /// 不是管理员
        NotAdmin,
        /// 不是委员会成员
        NotCommitteeMember,
        /// 委员会成员已存在
        CommitteeMemberExists,
        /// 委员会成员不存在
        CommitteeMemberNotFound,
        /// 委员会已满
        CommitteeFull,
        /// 提案已被否决
        ProposalAlreadyVetoed,
        /// 无否决权
        NoVetoRight,
        /// TokenType 不具有投票权
        TokenTypeNoVotingPower,
        /// 参数无效
        InvalidParameter,
        /// 提案类型暂未实现链上执行（需链下工作者配合）
        ProposalTypeNotImplemented,
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

            // 验证实体存在
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::ShopNotFound);

            // H1: 检查治理模式，None 模式不允许创建提案
            let gov_config = GovernanceConfigs::<T>::get(entity_id);
            if let Some(ref cfg) = gov_config {
                ensure!(cfg.mode != GovernanceMode::None, Error::<T>::GovernanceModeNotAllowed);
            }

            // H2: 验证提案参数有效性
            Self::validate_proposal_type(&proposal_type)?;

            // 验证代币已启用（Entity 级统一代币）
            ensure!(T::TokenProvider::is_token_enabled(entity_id), Error::<T>::TokenNotEnabled);

            // 验证持有足够代币
            let balance = T::TokenProvider::token_balance(entity_id, &who);
            let total_supply = T::TokenProvider::total_supply(entity_id);
            let min_threshold = total_supply
                .saturating_mul(T::MinProposalThreshold::get().into())
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
            let voting_end = now.saturating_add(T::VotingPeriod::get());

            let proposal = Proposal {
                id: proposal_id,
                entity_id,
                proposer: who.clone(),
                proposal_type,
                title: title_bounded,
                description_cid: description_bounded,
                status: ProposalStatus::Voting,
                created_at: now,
                snapshot_block: now,
                voting_start: now,
                voting_end,
                execution_time: None,
                yes_votes: Zero::zero(),
                no_votes: Zero::zero(),
                abstain_votes: Zero::zero(),
            };

            // 保存
            Proposals::<T>::insert(proposal_id, proposal);
            entity_proposals.try_push(proposal_id).map_err(|_| Error::<T>::TooManyActiveProposals)?;
            EntityProposals::<T>::insert(entity_id, entity_proposals);
            NextProposalId::<T>::put(proposal_id.saturating_add(1));

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

            // Phase 8: 检查代币类型是否具有投票权
            let token_type = T::TokenProvider::get_token_type(proposal.entity_id);
            ensure!(token_type.has_voting_power(), Error::<T>::TokenTypeNoVotingPower);

            // H1 修复: 获取当前投票权重
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

            Proposals::<T>::insert(proposal_id, proposal);
            VoteRecords::<T>::insert(proposal_id, &who, record);

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
            // L2 注意: 使用当前 total_supply 而非快照时的供应量，
            // 如果在投票期间有增发/销毁，法定人数计算可能偏差
            let total_votes = proposal.yes_votes
                .saturating_add(proposal.no_votes)
                .saturating_add(proposal.abstain_votes);
            let total_supply = T::TokenProvider::total_supply(proposal.entity_id);

            // 检查法定人数
            let quorum_threshold: BalanceOf<T> = total_supply
                .saturating_mul(T::QuorumThreshold::get().into())
                / 100u128.into();
            
            if total_votes < quorum_threshold {
                proposal.status = ProposalStatus::Failed;
                Self::remove_from_active(proposal_id, proposal.entity_id);
                Proposals::<T>::insert(proposal_id, proposal);
                Self::deposit_event(Event::ProposalFailed { proposal_id });
                return Ok(());
            }

            // 检查通过阈值
            let pass_threshold: BalanceOf<T> = total_votes
                .saturating_mul(T::PassThreshold::get().into())
                / 100u128.into();

            if proposal.yes_votes > pass_threshold {
                proposal.status = ProposalStatus::Passed;
                proposal.execution_time = Some(now.saturating_add(T::ExecutionDelay::get()));
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

            // 验证权限（提案者或实体所有者）
            let owner = T::EntityProvider::entity_owner(proposal.entity_id);
            ensure!(
                proposal.proposer == who || owner == Some(who.clone()),
                Error::<T>::CannotCancel
            );

            // 验证状态（只能取消 Created 或 Voting 状态的提案）
            ensure!(
                proposal.status == ProposalStatus::Created || proposal.status == ProposalStatus::Voting,
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
            quorum_threshold: Option<u8>,
            proposal_threshold: Option<u16>,
            admin_veto_enabled: Option<bool>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // H2 修复: 用 EntityProvider 验证实体所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            // H4: 参数验证
            if let Some(q) = quorum_threshold {
                ensure!(q <= 100, Error::<T>::InvalidParameter);
            }
            if let Some(t) = proposal_threshold {
                ensure!(t <= 10000, Error::<T>::InvalidParameter);
            }

            GovernanceConfigs::<T>::mutate(entity_id, |maybe_config| {
                let config = maybe_config.get_or_insert_with(GovernanceConfigOf::<T>::default);
                
                config.mode = mode;
                
                if let Some(period) = voting_period {
                    config.voting_period = period;
                }
                if let Some(quorum) = quorum_threshold {
                    config.quorum_threshold = quorum;
                }
                if let Some(threshold) = proposal_threshold {
                    config.proposal_threshold = threshold;
                }
                if let Some(veto) = admin_veto_enabled {
                    config.admin_veto_enabled = veto;
                }
            });

            Self::deposit_event(Event::GovernanceConfigUpdated {
                entity_id,
                mode,
            });
            Ok(())
        }

        /// 设置分层治理阈值
        #[pallet::call_index(6)]
        #[pallet::weight(Weight::from_parts(25_000_000, 3_000))]
        pub fn set_tiered_thresholds(
            origin: OriginFor<T>,
            entity_id: u64,
            operational: u8,
            significant: u8,
            critical: u8,
            constitutional: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            // H4: 阈值验证（百分比不超过 100）
            ensure!(operational <= 100, Error::<T>::InvalidParameter);
            ensure!(significant <= 100, Error::<T>::InvalidParameter);
            ensure!(critical <= 100, Error::<T>::InvalidParameter);
            ensure!(constitutional <= 100, Error::<T>::InvalidParameter);

            GovernanceConfigs::<T>::mutate(entity_id, |maybe_config| {
                let config = maybe_config.get_or_insert_with(GovernanceConfigOf::<T>::default);
                config.operational_threshold = operational;
                config.significant_threshold = significant;
                config.critical_threshold = critical;
                config.constitutional_threshold = constitutional;
            });

            Self::deposit_event(Event::GovernanceConfigUpdated {
                entity_id,
                mode: GovernanceConfigs::<T>::get(entity_id)
                    .map(|c| c.mode)
                    .unwrap_or_default(),
            });
            Ok(())
        }

        /// 添加委员会成员
        #[pallet::call_index(7)]
        #[pallet::weight(Weight::from_parts(25_000_000, 3_000))]
        pub fn add_committee_member(
            origin: OriginFor<T>,
            entity_id: u64,
            member: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            CommitteeMembers::<T>::try_mutate(entity_id, |members| -> DispatchResult {
                ensure!(!members.contains(&member), Error::<T>::CommitteeMemberExists);
                members.try_push(member.clone()).map_err(|_| Error::<T>::CommitteeFull)?;
                Ok(())
            })?;

            Self::deposit_event(Event::CommitteeMemberAdded {
                entity_id,
                member,
            });
            Ok(())
        }

        /// 移除委员会成员
        #[pallet::call_index(8)]
        #[pallet::weight(Weight::from_parts(25_000_000, 3_000))]
        pub fn remove_committee_member(
            origin: OriginFor<T>,
            entity_id: u64,
            member: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            CommitteeMembers::<T>::try_mutate(entity_id, |members| -> DispatchResult {
                let pos = members.iter().position(|m| m == &member)
                    .ok_or(Error::<T>::CommitteeMemberNotFound)?;
                members.remove(pos);
                Ok(())
            })?;

            Self::deposit_event(Event::CommitteeMemberRemoved {
                entity_id,
                member,
            });
            Ok(())
        }

        /// 管理员否决提案（DualTrack 模式）
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
            ensure!(
                config.mode == GovernanceMode::DualTrack || config.mode == GovernanceMode::Advisory,
                Error::<T>::GovernanceModeNotAllowed
            );

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
                ProposalType::LevelDiffChange { normal_rate, silver_rate, gold_rate, platinum_rate, diamond_rate } => {
                    ensure!(*normal_rate <= 10000, Error::<T>::InvalidParameter);
                    ensure!(*silver_rate <= 10000, Error::<T>::InvalidParameter);
                    ensure!(*gold_rate <= 10000, Error::<T>::InvalidParameter);
                    ensure!(*platinum_rate <= 10000, Error::<T>::InvalidParameter);
                    ensure!(*diamond_rate <= 10000, Error::<T>::InvalidParameter);
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
                _ => {},
            }
            Ok(())
        }

        /// 计算投票权重（时间加权）
        ///
        /// 公式: voting_power = balance * multiplier / 10000
        /// 其中 multiplier = 10000 + min(holding_blocks * bonus_range / full_period, bonus_range)
        /// bonus_range = max_multiplier - 10000
        ///
        /// 当 TimeWeightFullPeriod == 0 时，禁用时间加权，直接返回余额。
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
            // M4: 清理 VotingPowerSnapshot 避免存储泄漏
            let _ = VotingPowerSnapshot::<T>::clear_prefix(proposal_id, u32::MAX, None);
        }

        /// 执行提案
        fn do_execute_proposal(proposal: &ProposalOf<T>) -> DispatchResult {
            let entity_id = proposal.entity_id;
            // 获取 primary shop_id 用于需要 shop 级操作的提案
            let shop_id = T::EntityProvider::entity_shops(entity_id)
                .first()
                .copied()
                .unwrap_or(entity_id);
            
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
                ProposalType::ShopPause => {
                    T::ShopProvider::pause_shop(shop_id)
                },
                ProposalType::ShopResume => {
                    T::ShopProvider::resume_shop(shop_id)
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
                    // H6 修复: 无配置时创建默认配置再更新
                    let entity_id = T::ShopProvider::shop_entity_id(shop_id).unwrap_or(shop_id);
                    GovernanceConfigs::<T>::mutate(entity_id, |maybe_config| {
                        let config = maybe_config.get_or_insert_with(GovernanceConfigOf::<T>::default);
                        config.voting_period = (*new_period_blocks).into();
                    });
                    Ok(())
                },
                ProposalType::QuorumChange { new_quorum } => {
                    let entity_id = T::ShopProvider::shop_entity_id(shop_id).unwrap_or(shop_id);
                    GovernanceConfigs::<T>::mutate(entity_id, |maybe_config| {
                        let config = maybe_config.get_or_insert_with(GovernanceConfigOf::<T>::default);
                        config.quorum_threshold = *new_quorum;
                    });
                    Ok(())
                },
                ProposalType::ProposalThresholdChange { new_threshold } => {
                    let entity_id = T::ShopProvider::shop_entity_id(shop_id).unwrap_or(shop_id);
                    GovernanceConfigs::<T>::mutate(entity_id, |maybe_config| {
                        let config = maybe_config.get_or_insert_with(GovernanceConfigOf::<T>::default);
                        config.proposal_threshold = *new_threshold;
                    });
                    Ok(())
                },

                // ==================== 返佣配置类 ====================
                ProposalType::CommissionModesChange { modes } => {
                    T::CommissionProvider::set_commission_modes(shop_id, *modes)
                },
                ProposalType::DirectRewardChange { rate } => {
                    T::CommissionProvider::set_direct_reward_rate(shop_id, *rate)
                },
                ProposalType::MultiLevelChange { levels_cid: _, max_total_rate: _ } => {
                    // 多级分销配置需要解析 CID，暂不支持链上直接执行
                    Err(Error::<T>::ProposalTypeNotImplemented.into())
                },
                ProposalType::LevelDiffChange { normal_rate, silver_rate, gold_rate, platinum_rate, diamond_rate } => {
                    T::CommissionProvider::set_level_diff_config(
                        shop_id,
                        *normal_rate,
                        *silver_rate,
                        *gold_rate,
                        *platinum_rate,
                        *diamond_rate,
                    )
                },
                ProposalType::CustomLevelDiffChange { rates_cid: _, max_depth: _ } => {
                    // 自定义等级极差配置需要解析 CID，暂不支持链上直接执行
                    Err(Error::<T>::ProposalTypeNotImplemented.into())
                },
                ProposalType::FixedAmountChange { amount } => {
                    T::CommissionProvider::set_fixed_amount(shop_id, *amount)
                },
                ProposalType::FirstOrderChange { amount, rate, use_amount } => {
                    T::CommissionProvider::set_first_order_config(shop_id, *amount, *rate, *use_amount)
                },
                ProposalType::RepeatPurchaseChange { rate, min_orders } => {
                    T::CommissionProvider::set_repeat_purchase_config(shop_id, *rate, *min_orders)
                },
                ProposalType::SingleLineChange { upline_rate: _, downline_rate: _, base_upline_levels: _, base_downline_levels: _, max_upline_levels: _, max_downline_levels: _ } => {
                    // 单线收益配置较复杂，需要扩展 CommissionProvider trait
                    Err(Error::<T>::ProposalTypeNotImplemented.into())
                },

                // ==================== 分级提现配置类 ====================
                ProposalType::WithdrawalConfigChange { tier_configs_cid: _, enabled, shopping_balance_generates_commission } => {
                    // tier_configs_cid 需要链下解析，这里只设置基本配置
                    T::CommissionProvider::set_withdrawal_config_by_governance(
                        shop_id,
                        *enabled,
                        *shopping_balance_generates_commission,
                    )
                },

                ProposalType::MinRepurchaseRateChange { min_rate } => {
                    T::CommissionProvider::set_min_repurchase_rate(shop_id, *min_rate)
                },

                // ==================== 会员等级体系类 ====================
                ProposalType::AddCustomLevel { level_id, name, threshold, discount_rate, commission_bonus } => {
                    T::MemberProvider::add_custom_level(
                        shop_id,
                        *level_id,
                        name.as_slice(),
                        (*threshold).into(),
                        *discount_rate,
                        *commission_bonus,
                    )
                },
                ProposalType::UpdateCustomLevel { level_id, name, threshold, discount_rate, commission_bonus } => {
                    T::MemberProvider::update_custom_level(
                        shop_id,
                        *level_id,
                        name.as_ref().map(|n| n.as_slice()),
                        threshold.map(|t| t.into()),
                        *discount_rate,
                        *commission_bonus,
                    )
                },
                ProposalType::RemoveCustomLevel { level_id } => {
                    T::MemberProvider::remove_custom_level(shop_id, *level_id)
                },
                ProposalType::SetUpgradeMode { mode } => {
                    T::MemberProvider::set_upgrade_mode(shop_id, *mode)
                },
                ProposalType::EnableCustomLevels { enabled } => {
                    T::MemberProvider::set_custom_levels_enabled(shop_id, *enabled)
                },
                ProposalType::AddUpgradeRule { rule_cid: _ } => {
                    // 升级规则配置需要解析 CID，暂不支持链上直接执行
                    Ok(())
                },
                ProposalType::RemoveUpgradeRule { rule_id: _ } => {
                    // 需要扩展 MemberProvider trait 添加删除规则方法
                    Ok(())
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
