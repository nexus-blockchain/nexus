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

pub mod weights;
pub use weights::WeightInfo;

pub mod migration;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

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
    use pallet_entity_common::{
        DisclosureProvider, GovernanceMode, EntityProvider, EntityTokenProvider,
        ProductProvider, ShopProvider,
        // Phase 4.2: 领域治理执行 Port
        MarketGovernancePort, CommissionGovernancePort, SingleLineGovernancePort,
        KycGovernancePort, ShopGovernancePort, TokenGovernancePort,
        // Phase 4.3: 资金查询
        EntityTreasuryPort,
    };
    use pallet_entity_commission::{CommissionProvider, MemberProvider, MultiLevelPlanWriter, TeamPlanWriter};
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
        /// F5: 执行失败（链上 provider 返回错误，提案进入终态而非回滚）
        ExecutionFailed,
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

    /// 资金预警类型
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum FundAlertType {
        /// 金库余额低于最低阈值
        TreasuryBelowThreshold,
        /// 单笔支出超过限额
        SingleSpendExceeded,
        /// 每日累计支出超过限额
        DailySpendExceeded,
    }

    /// 资金保护配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct FundProtectionConfig<Balance: Default> {
        /// 最低金库余额阈值（低于此值触发预警，0 = 禁用）
        pub min_treasury_threshold: Balance,
        /// 单笔最大支出限额（超出触发预警，0 = 不限）
        pub max_single_spend: Balance,
        /// 每日最大支出限额（0 = 不限）
        pub max_daily_spend: Balance,
    }

    /// 资金保护配置类型别名
    pub type FundProtectionConfigOf<T> = FundProtectionConfig<BalanceOf<T>>;

    /// 提案领域分组
    ///
    /// Phase 4.1: 将 87 种提案按业务领域归类，供前端展示、查询过滤和权限策略使用。
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum ProposalDomain {
        /// 商品管理（价格、上下架、库存、可见性）
        Product,
        /// 店铺运营（名称、描述、暂停/恢复、关闭、类型、政策、积分）
        Shop,
        /// 代币经济（配置、增发、销毁、空投、分红、供应量、类型、转账限制、黑名单）
        Token,
        /// 财务管理（金库支出、手续费、收益分配、退款政策）
        Treasury,
        /// 治理参数（投票期、法定人数、门槛、延迟、阈值、否决权）
        Governance,
        /// 返佣配置（模式、直推、多级、极差、固定、首单、复购、费率、开关、提现）
        Commission,
        /// 会员管理（等级、升级模式、注册策略、统计策略）
        Member,
        /// 披露合规（级别、违规重置、内部人、处罚）
        Disclosure,
        /// DAO 紧急权限（暂停开关、批量取消开关）
        Emergency,
        /// 社区（活动、规则建议、通用提案）
        Community,
        /// 市场交易（配置、暂停/恢复/关闭、价格保护、KYC、熔断）
        Market,
        /// 单线收益（配置、暂停/恢复）
        SingleLine,
        /// KYC 管理（等级要求、提供者授权）
        Kyc,
        /// 资金保护（阈值、预警规则）
        FundProtection,
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
        /// 执行延迟调整（lock 后仍可通过提案修改）
        ExecutionDelayChange { new_delay_blocks: u32 },
        /// 通过阈值调整（lock 后仍可通过提案修改）
        PassThresholdChange { new_pass: u8 },
        /// 管理员否决权开关（lock 后仍可通过提案修改）
        AdminVetoToggle { enabled: bool },

        // ==================== 返佣配置类（新增）====================
        /// 启用/禁用返佣模式
        CommissionModesChange { modes: u16 },
        /// 直推奖励配置
        DirectRewardChange { rate: u16 },
        /// 多级分销配置（F8: 改为链上内联数据，支持治理落地）
        MultiLevelChange { 
            /// 各层级配置: Vec<(rate_bps, required_directs, required_team_size, required_spent_usdt)>
            tiers: BoundedVec<(u16, u32, u32, u128), ConstU32<15>>,
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

        // ==================== 披露管理类（F10）====================
        /// F5: 团队业绩配置变更
        TeamPerformanceChange {
            /// 阶梯配置: Vec<(sales_threshold_u128, min_team_size, rate_bps)>
            tiers: BoundedVec<(u128, u32, u16), ConstU32<10>>,
            max_depth: u8,
            allow_stacking: bool,
            /// 0=Nex, 1=Usdt
            threshold_mode: u8,
        },
        /// F5: 暂停团队业绩返佣
        TeamPerformancePause,
        /// F5: 恢复团队业绩返佣
        TeamPerformanceResume,

        /// 披露级别变更
        DisclosureLevelChange {
            level: u8,
            insider_trading_control: bool,
            blackout_period_after: u32,
        },
        /// 重置披露违规记录
        DisclosureResetViolations,

        // ==================== DAO 可控紧急权限类（R8）====================
        /// R8: Owner 紧急暂停权限开关（FullDAO 锁定后，DAO 可关闭 Owner 的 pause 能力）
        EmergencyPauseToggle { enabled: bool },
        /// R8: Owner 批量取消权限开关（FullDAO 锁定后，DAO 可关闭 Owner 的 batch_cancel 能力）
        BatchCancelToggle { enabled: bool },

        // ==================== 社区类 ====================
        /// 社区活动
        CommunityEvent { event_cid: BoundedVec<u8, ConstU32<64>> },
        /// 规则建议
        RuleSuggestion { suggestion_cid: BoundedVec<u8, ConstU32<64>> },
        /// 通用提案（自定义内容）
        General { title_cid: BoundedVec<u8, ConstU32<64>>, content_cid: BoundedVec<u8, ConstU32<64>> },

        // ==================== R10: 市场管理类 ====================
        /// 市场配置变更（最小下单额、挂单有效期）
        MarketConfigChange { min_order_amount: Balance, order_ttl: u32 },
        /// 市场暂停（冻结交易）
        MarketPause,
        /// 市场恢复
        MarketResume,
        /// 市场永久关闭（不可逆，取消所有挂单）
        MarketClose,
        /// 价格保护参数变更（偏差/滑点/熔断/TWAP）
        PriceProtectionChange { max_price_deviation: u16, max_slippage: u16, circuit_breaker_threshold: u16, min_trades_for_twap: u32 },
        /// 市场 KYC 门槛变更
        MarketKycChange { min_kyc_level: u8 },
        /// 解除熔断
        CircuitBreakerLift,

        // ==================== R10: 单线收益类 ====================
        /// 单线收益配置变更
        SingleLineConfigChange { upline_rate: u16, downline_rate: u16, base_upline_levels: u8, base_downline_levels: u8, max_upline_levels: u8, max_downline_levels: u8 },
        /// 单线收益暂停
        SingleLinePause,
        /// 单线收益恢复
        SingleLineResume,

        // ==================== R10: 代币扩展类 ====================
        /// 代币最大供应量变更
        TokenMaxSupplyChange { new_max_supply: Balance },
        /// 代币类型变更 (0=Points,1=Governance,2=Equity,3=Membership)
        TokenTypeChange { new_type: u8 },
        /// 转账限制模式变更 (0=None,1=Whitelist,2=KycRequired)
        TransferRestrictionChange { restriction: u8, min_receiver_kyc: u8 },
        /// 代币黑名单管理
        TokenBlacklistManage { account_cid: BoundedVec<u8, ConstU32<64>>, add: bool },

        // ==================== R10: 返佣核心配置类 ====================
        /// 最大返佣比率变更
        CommissionRateChange { new_rate: u16 },
        /// 返佣总开关
        CommissionToggle { enabled: bool },
        /// 创建者分成比率变更
        CreatorRewardRateChange { new_rate: u16 },
        /// 提现冷却期变更（区块数）
        WithdrawalCooldownChange { nex_cooldown: u32, token_cooldown: u32 },
        /// 代币提现配置变更
        TokenWithdrawalConfigChange { enabled: bool },
        /// 提现暂停/恢复
        WithdrawalPauseToggle { paused: bool },

        // ==================== R10: 推荐人配置类 ====================
        /// 推荐人资格门槛变更
        ReferrerGuardChange { min_referrer_spent: Balance, min_referrer_orders: u32 },
        /// 返佣上限变更
        CommissionCapChange { max_per_order: Balance, max_total_earned: Balance },
        /// 推荐有效期变更
        ReferralValidityChange { validity_blocks: u32, valid_orders: u32 },

        // ==================== R10: 多级暂停类 ====================
        /// 多级分销暂停
        MultiLevelPause,
        /// 多级分销恢复
        MultiLevelResume,

        // ==================== R10: 会员管理类 ====================
        /// 注册策略变更 (0=Open,1=ReferralOnly,2=ApprovalRequired,3=KycRequired)
        MemberPolicyChange { policy: u8 },
        /// 升级规则系统开关
        UpgradeRuleToggle { enabled: bool },
        /// 统计策略变更
        MemberStatsPolicyChange { qualified_only: bool, subtract_on_removal: bool },

        // ==================== R10: KYC 管理类 ====================
        /// KYC 等级要求变更
        KycRequirementChange { min_level: u8, mandatory: bool, grace_period: u32 },
        /// 授权 KYC 提供者
        KycProviderAuthorize { provider_id: u64 },
        /// 取消 KYC 提供者授权
        KycProviderDeauthorize { provider_id: u64 },

        // ==================== R10: 店铺扩展类 ====================
        /// 积分配置变更
        PointsConfigChange { reward_rate: u16, exchange_rate: u16, transferable: bool },
        /// 积分系统开关（关闭会清空所有余额）
        PointsToggle { enabled: bool },
        /// 店铺政策变更（退换货/物流政策）
        ShopPoliciesChange { policies_cid: BoundedVec<u8, ConstU32<256>> },
        /// 店铺关闭（不可逆）
        ShopClose { shop_id: u64 },
        /// 店铺类型变更 (0=Retail,1=Wholesale,2=Service,3=Digital)
        ShopTypeChange { shop_id: u64, new_type: u8 },

        // ==================== R10: 商品扩展类 ====================
        /// 商品可见性变更 (0=Public,1=MembersOnly,2=LevelGated)
        ProductVisibilityChange { product_id: u64, visibility: u8 },

        // ==================== R10: 披露扩展类 ====================
        /// 内部人管理（添加/移除）
        DisclosureInsiderManage { account_cid: BoundedVec<u8, ConstU32<64>>, add: bool },
        /// 处罚级别变更 (0=None/重置, 1=Warning, 2=Restricted, 3=Suspended, 4=Delisted)
        DisclosurePenaltyChange { level: u8 },

        // ==================== Phase 4.3: 资金保护类 ====================
        /// 资金保护配置变更（阈值、预警规则）
        FundProtectionChange {
            /// 最低金库余额阈值（低于此值触发预警，0 = 禁用）
            min_treasury_threshold: Balance,
            /// 单笔最大支出限额（超出需额外提案确认，0 = 不限）
            max_single_spend: Balance,
            /// 每日最大支出限额（0 = 不限）
            max_daily_spend: Balance,
        },
    }

    impl<Balance> ProposalType<Balance> {
        /// 返回提案所属的业务领域分组
        pub fn domain(&self) -> ProposalDomain {
            match self {
                // 商品管理
                Self::PriceChange { .. }
                | Self::ProductListing { .. }
                | Self::ProductDelisting { .. }
                | Self::InventoryAdjustment { .. }
                | Self::ProductVisibilityChange { .. } => ProposalDomain::Product,

                // 店铺运营
                Self::Promotion { .. }
                | Self::ShopNameChange { .. }
                | Self::ShopDescriptionChange { .. }
                | Self::ShopPause { .. }
                | Self::ShopResume { .. }
                | Self::ShopClose { .. }
                | Self::ShopTypeChange { .. }
                | Self::ShopPoliciesChange { .. }
                | Self::PointsConfigChange { .. }
                | Self::PointsToggle { .. } => ProposalDomain::Shop,

                // 代币经济
                Self::TokenConfigChange { .. }
                | Self::TokenMint { .. }
                | Self::TokenBurn { .. }
                | Self::AirdropDistribution { .. }
                | Self::Dividend { .. }
                | Self::TokenMaxSupplyChange { .. }
                | Self::TokenTypeChange { .. }
                | Self::TransferRestrictionChange { .. }
                | Self::TokenBlacklistManage { .. } => ProposalDomain::Token,

                // 财务管理
                Self::TreasurySpend { .. }
                | Self::FeeAdjustment { .. }
                | Self::RevenueShare { .. }
                | Self::RefundPolicy { .. } => ProposalDomain::Treasury,

                // 治理参数
                Self::VotingPeriodChange { .. }
                | Self::QuorumChange { .. }
                | Self::ProposalThresholdChange { .. }
                | Self::ExecutionDelayChange { .. }
                | Self::PassThresholdChange { .. }
                | Self::AdminVetoToggle { .. } => ProposalDomain::Governance,

                // 返佣配置
                Self::CommissionModesChange { .. }
                | Self::DirectRewardChange { .. }
                | Self::MultiLevelChange { .. }
                | Self::LevelDiffChange { .. }
                | Self::FixedAmountChange { .. }
                | Self::FirstOrderChange { .. }
                | Self::RepeatPurchaseChange { .. }
                | Self::WithdrawalConfigChange { .. }
                | Self::MinRepurchaseRateChange { .. }
                | Self::CommissionRateChange { .. }
                | Self::CommissionToggle { .. }
                | Self::CreatorRewardRateChange { .. }
                | Self::WithdrawalCooldownChange { .. }
                | Self::TokenWithdrawalConfigChange { .. }
                | Self::WithdrawalPauseToggle { .. }
                | Self::ReferrerGuardChange { .. }
                | Self::CommissionCapChange { .. }
                | Self::ReferralValidityChange { .. }
                | Self::MultiLevelPause
                | Self::MultiLevelResume
                | Self::TeamPerformanceChange { .. }
                | Self::TeamPerformancePause
                | Self::TeamPerformanceResume => ProposalDomain::Commission,

                // 会员管理
                Self::AddCustomLevel { .. }
                | Self::UpdateCustomLevel { .. }
                | Self::RemoveCustomLevel { .. }
                | Self::SetUpgradeMode { .. }
                | Self::EnableCustomLevels { .. }
                | Self::MemberPolicyChange { .. }
                | Self::UpgradeRuleToggle { .. }
                | Self::MemberStatsPolicyChange { .. } => ProposalDomain::Member,

                // 披露合规
                Self::DisclosureLevelChange { .. }
                | Self::DisclosureResetViolations
                | Self::DisclosureInsiderManage { .. }
                | Self::DisclosurePenaltyChange { .. } => ProposalDomain::Disclosure,

                // DAO 紧急权限
                Self::EmergencyPauseToggle { .. }
                | Self::BatchCancelToggle { .. } => ProposalDomain::Emergency,

                // 社区
                Self::CommunityEvent { .. }
                | Self::RuleSuggestion { .. }
                | Self::General { .. } => ProposalDomain::Community,

                // 市场交易
                Self::MarketConfigChange { .. }
                | Self::MarketPause
                | Self::MarketResume
                | Self::MarketClose
                | Self::PriceProtectionChange { .. }
                | Self::MarketKycChange { .. }
                | Self::CircuitBreakerLift => ProposalDomain::Market,

                // 单线收益
                Self::SingleLineConfigChange { .. }
                | Self::SingleLinePause
                | Self::SingleLineResume => ProposalDomain::SingleLine,

                // KYC 管理
                Self::KycRequirementChange { .. }
                | Self::KycProviderAuthorize { .. }
                | Self::KycProviderDeauthorize { .. } => ProposalDomain::Kyc,

                // 资金保护
                Self::FundProtectionChange { .. } => ProposalDomain::FundProtection,
            }
        }
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
        /// F1: 独立投票者数量
        pub voter_count: u32,
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

        /// F5: 每个委托接收者最大委托人数
        #[pallet::constant]
        type MaxDelegatorsPerDelegate: Get<u32>;

        // ========== Phase 5 新增配置 ==========

        /// C3: 最小投票期（区块数，configure_governance 不得低于此值）
        #[pallet::constant]
        type MinVotingPeriod: Get<BlockNumberFor<Self>>;

        /// C3: 最小执行延迟（区块数，configure_governance 不得低于此值）
        #[pallet::constant]
        type MinExecutionDelay: Get<BlockNumberFor<Self>>;

        /// S2: 最大投票期（区块数，configure_governance 和提案不得超过此值）
        #[pallet::constant]
        type MaxVotingPeriod: Get<BlockNumberFor<Self>>;

        /// S2: 最大执行延迟（区块数，configure_governance 和提案不得超过此值）
        #[pallet::constant]
        type MaxExecutionDelay: Get<BlockNumberFor<Self>>;

        /// 时间加权：达到最大乘数所需的持有区块数（0 = 禁用时间加权）
        #[pallet::constant]
        type TimeWeightFullPeriod: Get<BlockNumberFor<Self>>;

        /// 时间加权：最大投票权乘数（万分比，10000 = 1x 无加成，30000 = 3x）
        #[pallet::constant]
        type TimeWeightMaxMultiplier: Get<u32>;

        /// F3: 商品查询接口（治理提案执行 PriceChange/InventoryAdjustment）
        type ProductProvider: pallet_entity_common::ProductProvider<Self::AccountId, Self::Balance>;

        /// F8: 多级分销写入接口（治理提案执行时调用）
        type MultiLevelWriter: MultiLevelPlanWriter;

        /// F5: 团队业绩写入接口（治理提案执行时调用）
        type TeamWriter: TeamPlanWriter<Self::Balance>;

        /// F10: 披露服务接口（治理提案执行披露配置变更）
        type DisclosureProvider: pallet_entity_common::DisclosureProvider<Self::AccountId>;

        // ========== Phase 4.2: 领域治理执行 Port ==========

        /// 市场治理执行接口
        type MarketGovernance: MarketGovernancePort<Self::Balance>;

        /// 返佣治理扩展执行接口
        type CommissionGovernance: CommissionGovernancePort<Self::Balance>;

        /// 单线收益治理执行接口
        type SingleLineGovernance: SingleLineGovernancePort;

        /// KYC 治理执行接口
        type KycGovernance: KycGovernancePort;

        /// 店铺治理扩展执行接口
        type ShopGovernance: ShopGovernancePort;

        /// 代币治理扩展执行接口
        type TokenGovernance: TokenGovernancePort<Self::AccountId>;

        /// Phase 4.3: Entity 资金库查询（资金保护规则用）
        type TreasuryPort: EntityTreasuryPort;

        /// P2: 提案创建冷却期（区块数，同一用户在同一实体连续创建提案的最小间隔，0 = 禁用）
        #[pallet::constant]
        type ProposalCooldown: Get<BlockNumberFor<Self>>;

        /// P2: 紧急管理员 origin（用于 FullDAO 死锁恢复，通常为 sudo 或多签）
        type EmergencyOrigin: frame_support::traits::EnsureOrigin<Self::RuntimeOrigin>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: crate::weights::WeightInfo;
    }

    /// C2: 存储版本声明（P3: v1 — 新增 LastProposalCreatedAt 存储项）
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    // ==================== Hooks ====================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// C3: 自动 finalize 过期的 Voting 提案 + 自动 expire 超窗口的 Passed 提案
        ///
        /// 防止投票者代币永久锁定和 EntityProposals 耗尽。
        /// 每区块最多处理 5 个提案，weight-bounded。
        /// BUG-1 修复: 按 proposal.voter_count 动态计算 on_idle 权重
        ///
        /// Voting→Passed/Failed 路径会调用 remove_from_active → unlock_voters_for_proposal，
        /// 每个投票者需 ~5 次 DB 操作。旧代码使用固定 reads_writes(3,5) 严重低估。
        fn on_idle(now: BlockNumberFor<T>, mut remaining_weight: Weight) -> Weight {
            let base_read = T::DbWeight::get().reads(1);
            let cursor_weight = T::DbWeight::get().reads_writes(1, 1);
            let per_scan_read = T::DbWeight::get().reads(1);
            // BUG-1: 拆分为提案基础权重 + 每投票者权重
            let per_proposal_base = T::DbWeight::get().reads_writes(4, 4)
                .saturating_add(Weight::from_parts(30_000_000, 3_000));
            let per_voter_weight = T::DbWeight::get().reads_writes(5, 5)
                .saturating_add(Weight::from_parts(5_000_000, 1_000));

            let min_weight = base_read.saturating_add(cursor_weight).saturating_add(per_scan_read);
            if remaining_weight.ref_time() < min_weight.ref_time()
                || remaining_weight.proof_size() < min_weight.proof_size()
            {
                return Weight::zero();
            }

            let next_id = NextProposalId::<T>::get();
            remaining_weight = remaining_weight.saturating_sub(base_read).saturating_sub(cursor_weight);
            let mut consumed = base_read.saturating_add(cursor_weight);

            if next_id == 0 {
                return consumed;
            }

            let scan_start = ProposalScanCursor::<T>::get();
            let mut processed = 0u32;
            const MAX_PER_BLOCK: u32 = 5;

            let mut cursor = scan_start;
            let mut scanned = 0u64;

            while scanned < 100 && processed < MAX_PER_BLOCK {
                if remaining_weight.ref_time() < per_scan_read.ref_time()
                    || remaining_weight.proof_size() < per_scan_read.proof_size()
                {
                    break;
                }

                if cursor >= next_id {
                    cursor = 0;
                    if scanned > 0 { break; }
                }

                remaining_weight = remaining_weight.saturating_sub(per_scan_read);
                consumed = consumed.saturating_add(per_scan_read);
                scanned += 1;

                if let Some(proposal) = Proposals::<T>::get(cursor) {
                    let should_process = match proposal.status {
                        ProposalStatus::Voting => now > proposal.voting_end,
                        ProposalStatus::Passed => {
                            if let Some(exec_time) = proposal.execution_time {
                                let window = proposal.snapshot_execution_delay.saturating_mul(2u32.into());
                                now > exec_time.saturating_add(window)
                            } else {
                                false
                            }
                        },
                        _ => false,
                    };

                    if should_process {
                        // BUG-1: Voting 路径按 voter_count 动态估权
                        let needed = match proposal.status {
                            ProposalStatus::Voting => per_proposal_base.saturating_add(
                                per_voter_weight.saturating_mul(proposal.voter_count.into())
                            ),
                            _ => per_proposal_base,
                        };

                        if remaining_weight.ref_time() < needed.ref_time()
                            || remaining_weight.proof_size() < needed.proof_size()
                        {
                            cursor = cursor.saturating_add(1);
                            continue;
                        }

                        remaining_weight = remaining_weight.saturating_sub(needed);
                        consumed = consumed.saturating_add(needed);
                        Self::auto_finalize_proposal(cursor, proposal, now);
                        processed += 1;
                    }
                }

                cursor = cursor.saturating_add(1);
            }

            ProposalScanCursor::<T>::put(cursor);
            consumed
        }

        /// F8: integrity_test — 验证 runtime 配置常量的合理性
        fn integrity_test() {
            // 最小投票期必须 > 0
            assert!(
                T::MinVotingPeriod::get() > 0u32.into(),
                "MinVotingPeriod must be > 0"
            );
            // 最小执行延迟必须 > 0
            assert!(
                T::MinExecutionDelay::get() > 0u32.into(),
                "MinExecutionDelay must be > 0"
            );
            // 默认投票期 >= 最小投票期
            assert!(
                T::VotingPeriod::get() >= T::MinVotingPeriod::get(),
                "VotingPeriod must be >= MinVotingPeriod"
            );
            // 默认执行延迟 >= 最小执行延迟
            assert!(
                T::ExecutionDelay::get() >= T::MinExecutionDelay::get(),
                "ExecutionDelay must be >= MinExecutionDelay"
            );
            // S2: MaxVotingPeriod >= MinVotingPeriod
            assert!(
                T::MaxVotingPeriod::get() >= T::MinVotingPeriod::get(),
                "MaxVotingPeriod must be >= MinVotingPeriod"
            );
            // S2: MaxExecutionDelay >= MinExecutionDelay
            assert!(
                T::MaxExecutionDelay::get() >= T::MinExecutionDelay::get(),
                "MaxExecutionDelay must be >= MinExecutionDelay"
            );
            // 法定人数阈值 1..=100
            assert!(
                T::QuorumThreshold::get() >= 1 && T::QuorumThreshold::get() <= 100,
                "QuorumThreshold must be in [1, 100]"
            );
            // 通过阈值 1..=100
            assert!(
                T::PassThreshold::get() >= 1 && T::PassThreshold::get() <= 100,
                "PassThreshold must be in [1, 100]"
            );
            // 提案门槛 <= 10000 bps
            assert!(
                T::MinProposalThreshold::get() <= 10000,
                "MinProposalThreshold must be <= 10000"
            );
            // 时间加权乘数 >= 10000 (1x)
            assert!(
                T::TimeWeightMaxMultiplier::get() >= 10000,
                "TimeWeightMaxMultiplier must be >= 10000 (1x)"
            );
            // MaxActiveProposals > 0
            assert!(
                T::MaxActiveProposals::get() > 0,
                "MaxActiveProposals must be > 0"
            );
            // MaxDelegatorsPerDelegate > 0
            assert!(
                T::MaxDelegatorsPerDelegate::get() > 0,
                "MaxDelegatorsPerDelegate must be > 0"
            );
        }
    }

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

    // ========== C3: on_idle 扫描游标 ==========

    /// 提案扫描游标（on_idle 用，跟踪上次扫描到的 proposal_id）
    #[pallet::storage]
    pub type ProposalScanCursor<T: Config> = StorageValue<_, ProposalId, ValueQuery>;

    // ========== F6: 治理暂停 ==========

    /// 治理暂停状态 (entity_id → bool)
    #[pallet::storage]
    pub type GovernancePaused<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        bool,
        ValueQuery,
    >;

    // ========== R8: DAO 可控紧急权限 ==========

    /// FullDAO 锁定后 Owner 紧急暂停权限（None = true，DAO 可通过 EmergencyPauseToggle 关闭）
    #[pallet::storage]
    pub type EmergencyPauseEnabled<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        bool,
    >;

    /// FullDAO 锁定后 Owner 批量取消权限（None = true，DAO 可通过 BatchCancelToggle 关闭）
    #[pallet::storage]
    pub type BatchCancelEnabled<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        bool,
    >;

    // ========== P2: 提案创建冷却期 ==========

    /// P2: 上次创建提案时间 (entity_id, account) → block_number
    #[pallet::storage]
    pub type LastProposalCreatedAt<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        Blake2_128Concat,
        T::AccountId,
        BlockNumberFor<T>,
    >;

    // ========== F5: 投票委托 ==========

    /// 投票委托映射 (entity_id, delegator) → delegate
    #[pallet::storage]
    pub type VoteDelegation<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        Blake2_128Concat,
        T::AccountId,  // delegator
        T::AccountId,  // delegate
    >;

    /// 委托者列表 (entity_id, delegate) → delegators
    #[pallet::storage]
    pub type DelegatedVoters<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        Blake2_128Concat,
        T::AccountId,  // delegate
        BoundedVec<T::AccountId, T::MaxDelegatorsPerDelegate>,
        ValueQuery,
    >;

    // ========== Phase 4.3: 资金保护 ==========

    /// 资金保护配置 (entity_id → FundProtectionConfig)
    #[pallet::storage]
    #[pallet::getter(fn fund_protection_configs)]
    pub type FundProtectionConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        FundProtectionConfigOf<T>,
    >;

    /// 每日支出追踪 (entity_id → (day_number, accumulated_spend))
    ///
    /// day_number = block_number / BLOCKS_PER_DAY，用于按日重置累计支出。
    #[pallet::storage]
    pub type DailySpendTracker<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        (u32, BalanceOf<T>),  // (day_number, accumulated_spend)
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
            /// Phase 4.1: 提案所属领域
            domain: ProposalDomain,
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
        /// F5: 投票权已委托
        VoteDelegated {
            entity_id: u64,
            delegator: T::AccountId,
            delegate: T::AccountId,
        },
        /// F5: 投票委托已撤销
        VoteUndelegated {
            entity_id: u64,
            delegator: T::AccountId,
        },
        /// C3: 提案被 on_idle 自动 finalize（投票期结束未手动调用 finalize_voting）
        ProposalAutoFinalized {
            proposal_id: ProposalId,
            new_status: ProposalStatus,
        },
        /// F1: 投票已修改
        VoteChanged {
            proposal_id: ProposalId,
            voter: T::AccountId,
            old_vote: VoteType,
            new_vote: VoteType,
            weight: BalanceOf<T>,
        },
        /// F6: 治理已暂停
        GovernancePausedEvent {
            entity_id: u64,
        },
        /// F6: 治理已恢复
        GovernanceResumedEvent {
            entity_id: u64,
        },
        /// F6: 批量取消提案
        BatchProposalsCancelled {
            entity_id: u64,
            cancelled_count: u32,
        },
        /// R7-M2: 终态提案部分清理（存储量大需多次调用 cleanup_proposal）
        ProposalPartialCleaned {
            proposal_id: ProposalId,
        },
        /// F5: 提案执行失败（链上 provider 返回错误）
        ProposalExecutionFailed {
            proposal_id: ProposalId,
        },
        /// P2: 紧急恢复 — 治理被强制解锁
        GovernanceForceUnlocked {
            entity_id: u64,
        },
        /// P2: 紧急恢复 — 治理暂停被强制解除
        GovernanceForceResumed {
            entity_id: u64,
        },
        /// Phase 4.1: 资金保护配置已更新
        FundProtectionUpdated {
            entity_id: u64,
            min_treasury_threshold: BalanceOf<T>,
            max_single_spend: BalanceOf<T>,
            max_daily_spend: BalanceOf<T>,
        },
        /// Phase 4.3: 资金预警（金库余额低于阈值或单笔支出超限）
        FundProtectionAlert {
            entity_id: u64,
            alert_type: FundAlertType,
            current_value: BalanceOf<T>,
            threshold: BalanceOf<T>,
        },
    }

    // ==================== 错误 ====================

    #[pallet::error]
    pub enum Error<T> {
        /// R5: 实体不存在
        EntityNotFound,
        /// R5: 不是实体所有者
        NotEntityOwner,
        /// 店铺不属于该实体
        ShopNotFound,
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
        /// S2: 投票期超过最大值
        VotingPeriodTooLong,
        /// C3: 执行延迟低于最小值
        ExecutionDelayTooShort,
        /// S2: 执行延迟超过最大值
        ExecutionDelayTooLong,
        /// FullDAO 需要先发行代币
        TokenNotEnabledForDAO,
        /// 提案 ID 溢出
        ProposalIdOverflow,
        /// L2-R3: 提案未处于终态，不可清理
        ProposalNotTerminal,
        /// F5: 已委托投票权，不可直接投票（需先取消委托）
        VotePowerDelegated,
        /// F5: 已有委托关系
        AlreadyDelegated,
        /// F5: 无委托关系
        NotDelegated,
        /// F5: 不可自我委托
        SelfDelegation,
        /// F5: 委托接收者已达上限
        TooManyDelegators,
        /// R3: 提案类型暂不支持创建（链上执行未实现）
        ProposalTypeNotSupported,
        /// F7: 法定人数阈值不能为 0
        QuorumTooLow,
        /// F7: 通过阈值不能为 0
        PassThresholdTooLow,
        /// BUG-2: 提案门槛低于 MinProposalThreshold
        ProposalThresholdTooLow,
        /// F5: 实体已停用（ban/closed），不允许治理操作
        EntityNotActive,
        /// F2: 委托目标自身已委托他人，不可接受委托
        DelegateAlreadyDelegated,
        /// F6: 治理已暂停
        GovernanceIsPaused,
        /// F6: 治理未暂停
        GovernanceNotPaused,
        /// H2-audit: 该提案没有投票记录（change_vote 时使用）
        NotVoted,
        /// R8: DAO 已关闭 Owner 紧急暂停权限（FullDAO 锁定后）
        EmergencyPauseDisabled,
        /// R8: DAO 已关闭 Owner 批量取消权限（FullDAO 锁定后）
        BatchCancelDisabled,
        /// P2: 代币锁定失败（reserve 不足，无法完成投票）
        TokenLockFailed,
        /// P2: 提案创建冷却期未满（每个用户在每个实体上有创建间隔）
        ProposalCooldownActive,
        /// P2: 当前账户无紧急恢复权限
        NotEmergencyAdmin,
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
        #[pallet::weight(T::WeightInfo::create_proposal())]
        pub fn create_proposal(
            origin: OriginFor<T>,
            entity_id: u64,
            proposal_type: ProposalType<BalanceOf<T>>,
            title: Vec<u8>,
            description_cid: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体存在且活跃
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);

            // F6: 治理暂停检查
            ensure!(!GovernancePaused::<T>::get(entity_id), Error::<T>::GovernanceIsPaused);

            // H1: 检查治理模式，None 模式不允许创建提案
            let gov_config = GovernanceConfigs::<T>::get(entity_id);
            let effective_mode = gov_config.as_ref()
                .map(|c| c.mode)
                .unwrap_or(GovernanceMode::None);
            ensure!(effective_mode != GovernanceMode::None, Error::<T>::GovernanceModeNotAllowed);

            // H2+F2: 验证提案参数有效性（包含业务关联校验）
            Self::validate_proposal_type(entity_id, &proposal_type)?;

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

            // P2: 提案创建冷却期检查
            let cooldown = T::ProposalCooldown::get();
            if cooldown > BlockNumberFor::<T>::default() {
                if let Some(last_created) = LastProposalCreatedAt::<T>::get(entity_id, &who) {
                    let now_block = <frame_system::Pallet<T>>::block_number();
                    ensure!(
                        now_block >= last_created.saturating_add(cooldown),
                        Error::<T>::ProposalCooldownActive
                    );
                }
            }

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
                voter_count: 0,
                snapshot_quorum,
                snapshot_pass,
                snapshot_execution_delay,
                snapshot_total_supply: total_supply,
            };

            // Phase 4.1: 计算提案领域分组
            let domain = proposal.proposal_type.domain();

            // 保存
            Proposals::<T>::insert(proposal_id, proposal);
            entity_proposals.try_push(proposal_id).map_err(|_| Error::<T>::TooManyActiveProposals)?;
            EntityProposals::<T>::insert(entity_id, entity_proposals);
            // L1-fix: checked_add 防止 u64 溢出导致 ID 覆盖
            let next_id = proposal_id.checked_add(1).ok_or(Error::<T>::ProposalIdOverflow)?;
            NextProposalId::<T>::put(next_id);

            // P2: 记录提案创建时间用于冷却期
            LastProposalCreatedAt::<T>::insert(entity_id, &who, now);

            Self::deposit_event(Event::ProposalCreated {
                proposal_id,
                entity_id,
                proposer: who,
                title,
                domain,
            });

            Ok(())
        }

        /// 投票
        ///
        /// # 参数
        /// - `proposal_id`: 提案 ID
        /// - `vote`: 投票类型
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::vote(T::MaxDelegatorsPerDelegate::get()))]
        pub fn vote(
            origin: OriginFor<T>,
            proposal_id: ProposalId,
            vote: VoteType,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 获取提案
            let mut proposal = Proposals::<T>::get(proposal_id)
                .ok_or(Error::<T>::ProposalNotFound)?;

            // F5: 实体活跃检查
            ensure!(T::EntityProvider::is_entity_active(proposal.entity_id), Error::<T>::EntityNotActive);

            // F6: 治理暂停检查
            ensure!(!GovernancePaused::<T>::get(proposal.entity_id), Error::<T>::GovernanceIsPaused);

            // 验证状态
            ensure!(proposal.status == ProposalStatus::Voting, Error::<T>::InvalidProposalStatus);

            // 验证投票期
            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now <= proposal.voting_end, Error::<T>::VotingEnded);

            // F5: 已委托投票权的用户不可直接投票
            ensure!(
                !VoteDelegation::<T>::contains_key(proposal.entity_id, &who),
                Error::<T>::VotePowerDelegated
            );

            // 验证未投过票
            ensure!(
                !VoteRecords::<T>::contains_key(proposal_id, &who),
                Error::<T>::AlreadyVoted
            );

            // R7-H1: 防止委托→代理投票→取消委托→自行投票的双重计票攻击
            // 代理人投票时已为所有委托者插入 VoterTokenLocks 标记
            ensure!(
                !VoterTokenLocks::<T>::contains_key(proposal_id, &who),
                Error::<T>::AlreadyVoted
            );

            // 检查代币类型投票权
            let token_type = T::TokenProvider::get_token_type(proposal.entity_id);
            ensure!(token_type.has_voting_power(), Error::<T>::TokenTypeNoVotingPower);

            // 获取当前投票权重（自身 + 委托者）
            let own_power = Self::calculate_voting_power(proposal.entity_id, &who);
            ensure!(!own_power.is_zero(), Error::<T>::NoVotingPower);
            let delegated_power = Self::calculate_delegated_power(proposal.entity_id, proposal_id, &who);
            let current_balance = own_power.saturating_add(delegated_power);

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
            // F1: 累加独立投票者数量
            proposal.voter_count = proposal.voter_count.saturating_add(1);

            let record = VoteRecord {
                voter: who.clone(),
                vote: vote.clone(),
                weight,
                voted_at: now,
            };

            Proposals::<T>::insert(proposal_id, &proposal);
            VoteRecords::<T>::insert(proposal_id, &who, record);

            // H2: 锁定投票者的原始代币余额，防止投票后转让给其他账户复投
            // P2: reserve 失败时阻断投票（确保代币锁定成功才算有效投票）
            let entity_id = proposal.entity_id;
            let raw_balance = T::TokenProvider::token_balance(entity_id, &who);
            let current_locked = GovernanceLockAmount::<T>::get(entity_id, &who);
            if raw_balance > current_locked {
                let diff = raw_balance.saturating_sub(current_locked);
                T::TokenProvider::reserve(entity_id, &who, diff)
                    .map_err(|_| Error::<T>::TokenLockFailed)?;
                GovernanceLockAmount::<T>::insert(entity_id, &who, raw_balance);
            }
            GovernanceLockCount::<T>::mutate(entity_id, &who, |c| *c = c.saturating_add(1));
            VoterTokenLocks::<T>::insert(proposal_id, &who, ());

            // F5: 锁定委托者的代币（防止委托后转走代币）
            let delegators = DelegatedVoters::<T>::get(entity_id, &who);
            for delegator in delegators.iter() {
                let d_balance = T::TokenProvider::token_balance(entity_id, delegator);
                let d_locked = GovernanceLockAmount::<T>::get(entity_id, delegator);
                if d_balance > d_locked {
                    let diff = d_balance.saturating_sub(d_locked);
                    T::TokenProvider::reserve(entity_id, delegator, diff)
                        .map_err(|_| Error::<T>::TokenLockFailed)?;
                    GovernanceLockAmount::<T>::insert(entity_id, delegator, d_balance);
                }
                GovernanceLockCount::<T>::mutate(entity_id, delegator, |c| *c = c.saturating_add(1));
                VoterTokenLocks::<T>::insert(proposal_id, delegator, ());
            }

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
        #[pallet::weight(T::WeightInfo::finalize_voting())]
        pub fn finalize_voting(
            origin: OriginFor<T>,
            proposal_id: ProposalId,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            let mut proposal = Proposals::<T>::get(proposal_id)
                .ok_or(Error::<T>::ProposalNotFound)?;

            ensure!(T::EntityProvider::is_entity_active(proposal.entity_id), Error::<T>::EntityNotActive);
            ensure!(proposal.status == ProposalStatus::Voting, Error::<T>::InvalidProposalStatus);

            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now > proposal.voting_end, Error::<T>::VotingNotEnded);

            // R2: 使用共享 finalize 逻辑
            let new_status = Self::compute_finalize_result(proposal_id, &mut proposal, now);
            Proposals::<T>::insert(proposal_id, &proposal);
            match new_status {
                ProposalStatus::Passed => Self::deposit_event(Event::ProposalPassed { proposal_id }),
                _ => Self::deposit_event(Event::ProposalFailed { proposal_id }),
            }

            Ok(())
        }

        /// 执行提案
        ///
        /// 任何人都可以调用（执行时间到达后）
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::execute_proposal())]
        pub fn execute_proposal(
            origin: OriginFor<T>,
            proposal_id: ProposalId,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            let mut proposal = Proposals::<T>::get(proposal_id)
                .ok_or(Error::<T>::ProposalNotFound)?;

            // F5: 实体活跃检查
            ensure!(T::EntityProvider::is_entity_active(proposal.entity_id), Error::<T>::EntityNotActive);

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

            // F5: 执行提案 — 捕获执行错误，进入 ExecutionFailed 终态而非回滚
            match Self::do_execute_proposal(&proposal) {
                Ok(()) => {
                    proposal.status = ProposalStatus::Executed;
                    Proposals::<T>::insert(proposal_id, proposal);
                    Self::deposit_event(Event::ProposalExecuted { proposal_id });
                },
                Err(_) => {
                    proposal.status = ProposalStatus::ExecutionFailed;
                    Proposals::<T>::insert(proposal_id, proposal);
                    Self::deposit_event(Event::ProposalExecutionFailed { proposal_id });
                },
            }

            Ok(())
        }

        /// 取消提案
        ///
        /// 提案者或店主可以取消
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::cancel_proposal())]
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
        #[pallet::weight(T::WeightInfo::configure_governance())]
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

            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotEntityOwner);

            // S4: 实体必须活跃
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);

            ensure!(!GovernanceLocked::<T>::get(entity_id), Error::<T>::GovernanceConfigIsLocked);

            if mode == GovernanceMode::FullDAO {
                ensure!(T::TokenProvider::is_token_enabled(entity_id), Error::<T>::TokenNotEnabledForDAO);
            }

            if let Some(q) = quorum_threshold {
                ensure!(q >= 1, Error::<T>::QuorumTooLow);
                ensure!(q <= 100, Error::<T>::InvalidParameter);
            }
            if let Some(p) = pass_threshold {
                ensure!(p >= 1, Error::<T>::PassThresholdTooLow);
                ensure!(p <= 100, Error::<T>::InvalidParameter);
            }
            if let Some(t) = proposal_threshold {
                // BUG-2 修复: 0 = 使用全局默认，>0 必须 >= MinProposalThreshold
                ensure!(t == 0 || t >= T::MinProposalThreshold::get(), Error::<T>::ProposalThresholdTooLow);
                ensure!(t <= 10000, Error::<T>::InvalidParameter);
            }
            if let Some(period) = voting_period {
                ensure!(period >= T::MinVotingPeriod::get(), Error::<T>::VotingPeriodTooShort);
                ensure!(period <= T::MaxVotingPeriod::get(), Error::<T>::VotingPeriodTooLong);
            }
            if let Some(delay) = execution_delay {
                ensure!(delay >= T::MinExecutionDelay::get(), Error::<T>::ExecutionDelayTooShort);
                ensure!(delay <= T::MaxExecutionDelay::get(), Error::<T>::ExecutionDelayTooLong);
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
        /// - None 锁定 = 永久冻结治理配置（实体永不启用 DAO，适用于未发代币实体）
        /// - FullDAO 锁定 = 放弃控制权，仅可通过提案修改治理参数
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::lock_governance())]
        pub fn lock_governance(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotEntityOwner);

            ensure!(!GovernanceLocked::<T>::get(entity_id), Error::<T>::GovernanceAlreadyLocked);

            // None 和 FullDAO 均可锁定：
            // - None 锁定 = 永久冻结，明确"永不启用 DAO"（适用于未发代币实体）
            // - FullDAO 锁定 = 放弃控制权，仅通过提案修改
            GovernanceLocked::<T>::insert(entity_id, true);

            Self::deposit_event(Event::GovernanceConfigLocked { entity_id });
            Ok(())
        }

        /// L2-R3: 清理终态提案（Executed/Failed/Cancelled/Expired）
        ///
        /// 任何人可调用，清理已结束的提案释放存储空间。
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::cleanup_proposal())]
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
                        | ProposalStatus::ExecutionFailed
                ),
                Error::<T>::ProposalNotTerminal
            );

            // F9: 清理 VoteRecords（从 remove_from_active 延迟到此处）
            // M2-R2: 检查 clear_prefix 结果，未完全清理则保留 Proposal 供重试
            let vote_result = VoteRecords::<T>::clear_prefix(proposal_id, 500, None);
            // 清理 VotingPowerSnapshot 残留（如果 on_idle 路径未清理干净）
            let snapshot_result = VotingPowerSnapshot::<T>::clear_prefix(proposal_id, 500, None);
            // L1-R2: 防御性清理 VoterTokenLocks（正常流程中应已为空）
            let lock_result = VoterTokenLocks::<T>::clear_prefix(proposal_id, 500, None);

            // M2-R2: 仅当所有前缀完全清理后才删除 Proposal；否则保留供再次调用
            // MultiRemovalResults.maybe_cursor == None 表示全部清理完毕
            let all_cleared = vote_result.maybe_cursor.is_none()
                && snapshot_result.maybe_cursor.is_none()
                && lock_result.maybe_cursor.is_none();

            if all_cleared {
                Proposals::<T>::remove(proposal_id);
                Self::deposit_event(Event::ProposalCleaned { proposal_id });
            } else {
                Self::deposit_event(Event::ProposalPartialCleaned { proposal_id });
            }

            Ok(())
        }

        /// F5: 委托投票权
        ///
        /// 将自己在某实体的投票权委托给另一个账户。
        /// 委托后不可直接投票，需先取消委托。（Compound 模型）
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::delegate_vote())]
        pub fn delegate_vote(
            origin: OriginFor<T>,
            entity_id: u64,
            delegate: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(who != delegate, Error::<T>::SelfDelegation);

            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);

            // 检查代币已启用
            ensure!(T::TokenProvider::is_token_enabled(entity_id), Error::<T>::TokenNotEnabled);

            // 检查未已有委托
            ensure!(
                !VoteDelegation::<T>::contains_key(entity_id, &who),
                Error::<T>::AlreadyDelegated
            );

            // F2: 检查委托目标自身未委托他人（防止投票权黑洞）
            ensure!(
                !VoteDelegation::<T>::contains_key(entity_id, &delegate),
                Error::<T>::DelegateAlreadyDelegated
            );

            // 添加到委托者列表
            DelegatedVoters::<T>::try_mutate(entity_id, &delegate, |voters| {
                voters.try_push(who.clone()).map_err(|_| Error::<T>::TooManyDelegators)
            })?;
            VoteDelegation::<T>::insert(entity_id, &who, &delegate);

            Self::deposit_event(Event::VoteDelegated {
                entity_id,
                delegator: who,
                delegate,
            });
            Ok(())
        }

        /// F5: 取消投票委托
        ///
        /// 取消对某实体的投票权委托，恢复直接投票能力。
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::undelegate_vote())]
        pub fn undelegate_vote(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let delegate = VoteDelegation::<T>::take(entity_id, &who)
                .ok_or(Error::<T>::NotDelegated)?;

            // 从委托者列表移除
            DelegatedVoters::<T>::mutate(entity_id, &delegate, |voters| {
                voters.retain(|v| v != &who);
            });

            Self::deposit_event(Event::VoteUndelegated {
                entity_id,
                delegator: who,
            });
            Ok(())
        }

        /// 管理员否决提案（需 admin_veto_enabled）
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::veto_proposal())]
        pub fn veto_proposal(
            origin: OriginFor<T>,
            proposal_id: ProposalId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut proposal = Proposals::<T>::get(proposal_id)
                .ok_or(Error::<T>::ProposalNotFound)?;

            let owner = T::EntityProvider::entity_owner(proposal.entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
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

        /// F1: 修改投票
        ///
        /// 已投票的用户可以修改投票类型（Yes/No/Abstain），权重不变。
        /// 仅在投票期内可修改。
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::change_vote())]
        pub fn change_vote(
            origin: OriginFor<T>,
            proposal_id: ProposalId,
            new_vote: VoteType,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut proposal = Proposals::<T>::get(proposal_id)
                .ok_or(Error::<T>::ProposalNotFound)?;

            // F5: 实体活跃检查
            ensure!(T::EntityProvider::is_entity_active(proposal.entity_id), Error::<T>::EntityNotActive);

            // M1-audit: 治理暂停检查（与 vote 一致）
            ensure!(!GovernancePaused::<T>::get(proposal.entity_id), Error::<T>::GovernanceIsPaused);

            // 验证状态
            ensure!(proposal.status == ProposalStatus::Voting, Error::<T>::InvalidProposalStatus);

            // 验证投票期
            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now <= proposal.voting_end, Error::<T>::VotingEnded);

            // 获取旧投票记录
            // H2-audit: 使用 NotVoted 而非 ProposalNotFound
            let old_record = VoteRecords::<T>::get(proposal_id, &who)
                .ok_or(Error::<T>::NotVoted)?;

            let old_vote = old_record.vote.clone();
            let weight = old_record.weight;

            // 如果投票类型相同则无需操作
            if old_vote == new_vote {
                return Ok(());
            }

            // 撤销旧投票权重
            match old_vote {
                VoteType::Yes => proposal.yes_votes = proposal.yes_votes.saturating_sub(weight),
                VoteType::No => proposal.no_votes = proposal.no_votes.saturating_sub(weight),
                VoteType::Abstain => proposal.abstain_votes = proposal.abstain_votes.saturating_sub(weight),
            }

            // 添加新投票权重
            match new_vote {
                VoteType::Yes => proposal.yes_votes = proposal.yes_votes.saturating_add(weight),
                VoteType::No => proposal.no_votes = proposal.no_votes.saturating_add(weight),
                VoteType::Abstain => proposal.abstain_votes = proposal.abstain_votes.saturating_add(weight),
            }

            // 更新记录
            let new_record = VoteRecord {
                voter: who.clone(),
                vote: new_vote.clone(),
                weight,
                voted_at: now,
            };
            Proposals::<T>::insert(proposal_id, &proposal);
            VoteRecords::<T>::insert(proposal_id, &who, new_record);

            Self::deposit_event(Event::VoteChanged {
                proposal_id,
                voter: who,
                old_vote,
                new_vote,
                weight,
            });

            Ok(())
        }

        /// F6: 紧急暂停治理
        ///
        /// 实体所有者可以暂停治理，阻止新提案创建和投票。
        /// 已有提案仍可 finalize/execute/cancel。
        /// R8: FullDAO 锁定后受 EmergencyPauseEnabled 控制，DAO 可关闭此权限。
        #[pallet::call_index(15)]
        #[pallet::weight(T::WeightInfo::pause_governance())]
        pub fn pause_governance(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotEntityOwner);

            // R8: FullDAO 锁定后，检查 DAO 是否仍授权 Owner 紧急暂停
            if GovernanceLocked::<T>::get(entity_id) {
                let mode = GovernanceConfigs::<T>::get(entity_id)
                    .map(|c| c.mode)
                    .unwrap_or(GovernanceMode::None);
                if mode == GovernanceMode::FullDAO {
                    let enabled = EmergencyPauseEnabled::<T>::get(entity_id).unwrap_or(true);
                    ensure!(enabled, Error::<T>::EmergencyPauseDisabled);
                }
            }

            ensure!(!GovernancePaused::<T>::get(entity_id), Error::<T>::GovernanceIsPaused);

            GovernancePaused::<T>::insert(entity_id, true);

            Self::deposit_event(Event::GovernancePausedEvent { entity_id });
            Ok(())
        }

        /// F6: 恢复治理
        #[pallet::call_index(16)]
        #[pallet::weight(T::WeightInfo::resume_governance())]
        pub fn resume_governance(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotEntityOwner);

            ensure!(GovernancePaused::<T>::get(entity_id), Error::<T>::GovernanceNotPaused);

            GovernancePaused::<T>::remove(entity_id);

            Self::deposit_event(Event::GovernanceResumedEvent { entity_id });
            Ok(())
        }

        /// F6: 批量取消所有活跃提案
        ///
        /// 实体所有者在紧急情况下可一次取消所有 Voting 状态提案。
        /// R8: FullDAO 锁定后受 BatchCancelEnabled 控制，DAO 可关闭此权限。
        #[pallet::call_index(17)]
        #[pallet::weight(T::WeightInfo::batch_cancel_proposals(
            T::MaxActiveProposals::get(),
            T::MaxActiveProposals::get().saturating_mul(100),
        ))]
        pub fn batch_cancel_proposals(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotEntityOwner);

            if GovernanceLocked::<T>::get(entity_id) {
                let mode = GovernanceConfigs::<T>::get(entity_id)
                    .map(|c| c.mode)
                    .unwrap_or(GovernanceMode::None);
                if mode == GovernanceMode::FullDAO {
                    let enabled = BatchCancelEnabled::<T>::get(entity_id).unwrap_or(true);
                    ensure!(enabled, Error::<T>::BatchCancelDisabled);
                }
            }

            let active_ids = EntityProposals::<T>::get(entity_id);
            let mut cancelled_count: u32 = 0;

            // R-1 重构: 使用共用 helper 解锁代币 + 清理快照（BUG-3 + BUG-4 修复）
            for &proposal_id in active_ids.iter() {
                if let Some(mut proposal) = Proposals::<T>::get(proposal_id) {
                    if proposal.status == ProposalStatus::Voting {
                        proposal.status = ProposalStatus::Cancelled;
                        Proposals::<T>::insert(proposal_id, &proposal);
                        Self::deposit_event(Event::ProposalCancelled { proposal_id });
                        cancelled_count += 1;
                        Self::unlock_voters_for_proposal(proposal_id, entity_id);
                    }
                }
            }

            // R-2 优化: 所有 Voting 已改为 Cancelled，直接清空活跃列表
            EntityProposals::<T>::remove(entity_id);

            Self::deposit_event(Event::BatchProposalsCancelled {
                entity_id,
                cancelled_count,
            });

            Ok(())
        }

        /// P2: FullDAO 死锁紧急恢复 — 强制解锁治理配置并恢复暂停
        ///
        /// 仅 EmergencyOrigin（通常为 sudo 或链级多签）可调用。
        /// 用于 FullDAO 模式下出现死锁（如所有代币丢失、无法达到法定人数）时的最后手段。
        /// 解锁后 Owner 恢复 configure_governance 权限，可重新配置治理参数。
        #[pallet::call_index(18)]
        #[pallet::weight(T::WeightInfo::configure_governance())]
        pub fn force_unlock_governance(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            T::EmergencyOrigin::ensure_origin(origin)?;

            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);

            if GovernanceLocked::<T>::get(entity_id) {
                GovernanceLocked::<T>::remove(entity_id);
                Self::deposit_event(Event::GovernanceForceUnlocked { entity_id });
            }

            if GovernancePaused::<T>::get(entity_id) {
                GovernancePaused::<T>::remove(entity_id);
                Self::deposit_event(Event::GovernanceForceResumed { entity_id });
            }

            Ok(())
        }
    }

    // ==================== 内部函数 ====================

    impl<T: Config> Pallet<T> {
        /// 验证提案类型参数有效性（basis points ≤ 10000，百分比 ≤ 100，业务关联校验）
        fn validate_proposal_type(entity_id: u64, pt: &ProposalType<BalanceOf<T>>) -> DispatchResult {
            match pt {
                // ==================== 商品管理类 — F2 前置校验 ====================
                ProposalType::PriceChange { product_id, new_price } => {
                    ensure!(!new_price.is_zero(), Error::<T>::InvalidParameter);
                    Self::validate_product_belongs_to_entity(entity_id, *product_id)?;
                },
                ProposalType::ProductDelisting { product_id } => {
                    Self::validate_product_belongs_to_entity(entity_id, *product_id)?;
                },
                ProposalType::InventoryAdjustment { product_id, new_inventory } => {
                    ensure!(*new_inventory <= u32::MAX as u64, Error::<T>::InvalidParameter);
                    Self::validate_product_belongs_to_entity(entity_id, *product_id)?;
                },

                // ==================== 店铺运营类 — F2 前置校验 ====================
                ProposalType::ShopPause { shop_id } | ProposalType::ShopResume { shop_id } => {
                    let entity_shops = T::EntityProvider::entity_shops(entity_id);
                    ensure!(entity_shops.contains(shop_id), Error::<T>::ShopNotFound);
                },
                ProposalType::Promotion { discount_rate, .. } => {
                    ensure!(*discount_rate <= 10000, Error::<T>::InvalidParameter);
                },

                // ==================== 代币经济类 ====================
                ProposalType::TokenConfigChange { reward_rate, exchange_rate } => {
                    if let Some(r) = reward_rate { ensure!(*r <= 10000, Error::<T>::InvalidParameter); }
                    if let Some(e) = exchange_rate { ensure!(*e <= 10000, Error::<T>::InvalidParameter); }
                },
                ProposalType::TokenMint { amount, .. } => {
                    ensure!(!amount.is_zero(), Error::<T>::InvalidParameter);
                },
                ProposalType::TokenBurn { amount } => {
                    ensure!(!amount.is_zero(), Error::<T>::InvalidParameter);
                },
                ProposalType::AirdropDistribution { total_amount, .. } => {
                    ensure!(!total_amount.is_zero(), Error::<T>::InvalidParameter);
                },
                ProposalType::Dividend { rate } => {
                    ensure!(*rate <= 10000, Error::<T>::InvalidParameter);
                },

                // ==================== 财务管理类 ====================
                ProposalType::TreasurySpend { amount, .. } => {
                    ensure!(!amount.is_zero(), Error::<T>::InvalidParameter);
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

                // ==================== 治理参数类 — S2/S3 边界校验 ====================
                ProposalType::VotingPeriodChange { new_period_blocks } => {
                    let period: BlockNumberFor<T> = (*new_period_blocks).into();
                    ensure!(period >= T::MinVotingPeriod::get(), Error::<T>::VotingPeriodTooShort);
                    ensure!(period <= T::MaxVotingPeriod::get(), Error::<T>::VotingPeriodTooLong);
                },
                ProposalType::ExecutionDelayChange { new_delay_blocks } => {
                    let delay: BlockNumberFor<T> = (*new_delay_blocks).into();
                    ensure!(delay >= T::MinExecutionDelay::get(), Error::<T>::ExecutionDelayTooShort);
                    ensure!(delay <= T::MaxExecutionDelay::get(), Error::<T>::ExecutionDelayTooLong);
                },
                ProposalType::QuorumChange { new_quorum } => {
                    ensure!(*new_quorum >= 1, Error::<T>::QuorumTooLow);
                    ensure!(*new_quorum <= 100, Error::<T>::InvalidParameter);
                },
                ProposalType::ProposalThresholdChange { new_threshold } => {
                    // S3: 增加最小值校验
                    ensure!(*new_threshold >= T::MinProposalThreshold::get(), Error::<T>::InvalidParameter);
                    ensure!(*new_threshold <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::PassThresholdChange { new_pass } => {
                    ensure!(*new_pass >= 1, Error::<T>::PassThresholdTooLow);
                    ensure!(*new_pass <= 100, Error::<T>::InvalidParameter);
                },
                ProposalType::AdminVetoToggle { .. } => {},

                // ==================== 返佣配置类 ====================
                ProposalType::CommissionModesChange { modes } => {
                    ensure!(*modes & !0b0000_0011_1111_1111u16 == 0, Error::<T>::InvalidParameter);
                },
                ProposalType::DirectRewardChange { rate } => {
                    ensure!(*rate <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::MultiLevelChange { ref tiers, max_total_rate } => {
                    ensure!(!tiers.is_empty(), Error::<T>::InvalidParameter);
                    ensure!(*max_total_rate > 0 && *max_total_rate <= 10000, Error::<T>::InvalidParameter);
                    for &(rate, _, _, _) in tiers.iter() {
                        ensure!(rate <= 10000, Error::<T>::InvalidParameter);
                    }
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
                ProposalType::MinRepurchaseRateChange { min_rate } => {
                    ensure!(*min_rate <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::FixedAmountChange { amount } => {
                    ensure!(!amount.is_zero(), Error::<T>::InvalidParameter);
                },

                // ==================== 会员等级类 ====================
                ProposalType::AddCustomLevel { discount_rate, commission_bonus, .. } => {
                    ensure!(*discount_rate <= 10000, Error::<T>::InvalidParameter);
                    ensure!(*commission_bonus <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::UpdateCustomLevel { discount_rate, commission_bonus, .. } => {
                    if let Some(dr) = discount_rate {
                        ensure!(*dr <= 10000, Error::<T>::InvalidParameter);
                    }
                    if let Some(cb) = commission_bonus {
                        ensure!(*cb <= 10000, Error::<T>::InvalidParameter);
                    }
                },
                ProposalType::SetUpgradeMode { mode } => {
                    ensure!(*mode <= 2, Error::<T>::InvalidParameter);
                },

                // ==================== 团队业绩类 ====================
                ProposalType::TeamPerformanceChange { ref tiers, max_depth, threshold_mode, .. } => {
                    ensure!(!tiers.is_empty(), Error::<T>::InvalidParameter);
                    ensure!(*max_depth > 0 && *max_depth <= 30, Error::<T>::InvalidParameter);
                    ensure!(*threshold_mode <= 1, Error::<T>::InvalidParameter);
                    for &(_, _, rate) in tiers.iter() {
                        ensure!(rate <= 10000, Error::<T>::InvalidParameter);
                    }
                    for window in tiers.windows(2) {
                        ensure!(window[1].0 > window[0].0, Error::<T>::InvalidParameter);
                    }
                },

                // ==================== 披露管理类 ====================
                ProposalType::DisclosureLevelChange { level, .. } => {
                    ensure!(*level <= 3, Error::<T>::InvalidParameter);
                },

                // ==================== DAO 可控紧急权限 — F3 前置条件 ====================
                ProposalType::EmergencyPauseToggle { .. } | ProposalType::BatchCancelToggle { .. } => {
                    let locked = GovernanceLocked::<T>::get(entity_id);
                    let mode = GovernanceConfigs::<T>::get(entity_id)
                        .map(|c| c.mode)
                        .unwrap_or(GovernanceMode::None);
                    ensure!(locked && mode == GovernanceMode::FullDAO, Error::<T>::GovernanceModeNotAllowed);
                },

                // ==================== R10: 市场管理类 ====================
                ProposalType::PriceProtectionChange { max_price_deviation, max_slippage, circuit_breaker_threshold, .. } => {
                    ensure!(*max_price_deviation <= 10000, Error::<T>::InvalidParameter);
                    ensure!(*max_slippage <= 10000, Error::<T>::InvalidParameter);
                    ensure!(*circuit_breaker_threshold <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::MarketKycChange { min_kyc_level } => {
                    ensure!(*min_kyc_level <= 4, Error::<T>::InvalidParameter);
                },

                // ==================== R10: 单线收益类 ====================
                ProposalType::SingleLineConfigChange { upline_rate, downline_rate, max_upline_levels, max_downline_levels, base_upline_levels, base_downline_levels } => {
                    ensure!(*upline_rate <= 10000, Error::<T>::InvalidParameter);
                    ensure!(*downline_rate <= 10000, Error::<T>::InvalidParameter);
                    ensure!(*base_upline_levels <= *max_upline_levels, Error::<T>::InvalidParameter);
                    ensure!(*base_downline_levels <= *max_downline_levels, Error::<T>::InvalidParameter);
                },

                // ==================== R10: 代币扩展类 ====================
                ProposalType::TokenMaxSupplyChange { new_max_supply } => {
                    ensure!(!new_max_supply.is_zero(), Error::<T>::InvalidParameter);
                },
                ProposalType::TokenTypeChange { new_type } => {
                    ensure!(*new_type <= 3, Error::<T>::InvalidParameter);
                },
                ProposalType::TransferRestrictionChange { restriction, min_receiver_kyc } => {
                    ensure!(*restriction <= 2, Error::<T>::InvalidParameter);
                    ensure!(*min_receiver_kyc <= 4, Error::<T>::InvalidParameter);
                },

                // ==================== R10: 返佣核心配置类 ====================
                ProposalType::CommissionRateChange { new_rate } => {
                    ensure!(*new_rate <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::CreatorRewardRateChange { new_rate } => {
                    ensure!(*new_rate <= 10000, Error::<T>::InvalidParameter);
                },

                // ==================== R10: 推荐人配置类 ====================
                ProposalType::CommissionCapChange { max_per_order, max_total_earned } => {
                    ensure!(!max_per_order.is_zero() || !max_total_earned.is_zero(), Error::<T>::InvalidParameter);
                },

                // ==================== R10: 会员管理类 ====================
                ProposalType::MemberPolicyChange { policy } => {
                    ensure!(*policy <= 3, Error::<T>::InvalidParameter);
                },
                ProposalType::MemberStatsPolicyChange { .. } => {},

                // ==================== R10: KYC 管理类 ====================
                ProposalType::KycRequirementChange { min_level, .. } => {
                    ensure!(*min_level <= 4, Error::<T>::InvalidParameter);
                },

                // ==================== R10: 店铺扩展类 ====================
                ProposalType::PointsConfigChange { reward_rate, exchange_rate, .. } => {
                    ensure!(*reward_rate <= 10000, Error::<T>::InvalidParameter);
                    ensure!(*exchange_rate <= 10000, Error::<T>::InvalidParameter);
                },
                ProposalType::ShopClose { shop_id } => {
                    let entity_shops = T::EntityProvider::entity_shops(entity_id);
                    ensure!(entity_shops.contains(shop_id), Error::<T>::ShopNotFound);
                },
                ProposalType::ShopTypeChange { shop_id, new_type } => {
                    let entity_shops = T::EntityProvider::entity_shops(entity_id);
                    ensure!(entity_shops.contains(shop_id), Error::<T>::ShopNotFound);
                    ensure!(*new_type <= 3, Error::<T>::InvalidParameter);
                },

                // ==================== R10: 商品扩展类 ====================
                ProposalType::ProductVisibilityChange { product_id, visibility } => {
                    Self::validate_product_belongs_to_entity(entity_id, *product_id)?;
                    ensure!(*visibility <= 2, Error::<T>::InvalidParameter);
                },

                // ==================== R10: 披露扩展类 ====================
                // A-5: 允许级别 0-4 (0=None/重置, 1=Warning, 2=Restricted, 3=Suspended, 4=Delisted)
                ProposalType::DisclosurePenaltyChange { level } => {
                    ensure!(*level <= 4, Error::<T>::InvalidParameter);
                },

                // ==================== BUG-5 修复: off-chain 执行类参数校验 ====================
                ProposalType::MarketConfigChange { min_order_amount, order_ttl } => {
                    ensure!(!min_order_amount.is_zero(), Error::<T>::InvalidParameter);
                    ensure!(*order_ttl > 0, Error::<T>::InvalidParameter);
                },
                ProposalType::WithdrawalCooldownChange { nex_cooldown, token_cooldown } => {
                    ensure!(*nex_cooldown > 0 || *token_cooldown > 0, Error::<T>::InvalidParameter);
                },

                // ==================== Phase 4.3: 资金保护类参数校验 ====================
                ProposalType::FundProtectionChange { min_treasury_threshold, max_single_spend, max_daily_spend } => {
                    // 至少设置一项规则
                    ensure!(
                        !min_treasury_threshold.is_zero() || !max_single_spend.is_zero() || !max_daily_spend.is_zero(),
                        Error::<T>::InvalidParameter
                    );
                    // max_daily_spend 必须 >= max_single_spend（如果都设置了）
                    if !max_daily_spend.is_zero() && !max_single_spend.is_zero() {
                        ensure!(*max_daily_spend >= *max_single_spend, Error::<T>::InvalidParameter);
                    }
                },

                _ => {},
            }
            Ok(())
        }

        /// F2: 校验商品是否属于当前实体的某个店铺（S1: 不存在的商品也拒绝）
        fn validate_product_belongs_to_entity(entity_id: u64, product_id: u64) -> DispatchResult {
            let product_shop = T::ProductProvider::product_shop_id(product_id)
                .ok_or(Error::<T>::InvalidParameter)?;
            let entity_shops = T::EntityProvider::entity_shops(entity_id);
            ensure!(entity_shops.contains(&product_shop), Error::<T>::InvalidParameter);
            Ok(())
        }

        /// F5: 计算委托给某用户的投票权重总和
        fn calculate_delegated_power(entity_id: u64, proposal_id: ProposalId, delegate: &T::AccountId) -> BalanceOf<T> {
            let delegators = DelegatedVoters::<T>::get(entity_id, delegate);
            let mut total: BalanceOf<T> = Zero::zero();
            for delegator in delegators.iter() {
                // S1: 同时检查 VoteRecords 和 VoterTokenLocks，防止委托→代投→取消委托→再委托的双重计票
                if !VoteRecords::<T>::contains_key(proposal_id, delegator)
                    && !VoterTokenLocks::<T>::contains_key(proposal_id, delegator)
                {
                    total = total.saturating_add(Self::calculate_voting_power(entity_id, delegator));
                }
            }
            total
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

        /// R-1 重构: 解锁提案所有投票者代币 + 清理快照（共用 helper）
        ///
        /// BUG-3 修复: 同时清理 VotingPowerSnapshot（batch_cancel 原来遗漏）
        /// BUG-4 修复: GovernanceLockCount 使用 mutate 返回值消除双重读取
        fn unlock_voters_for_proposal(proposal_id: ProposalId, entity_id: u64) {
            for (voter, _) in VoterTokenLocks::<T>::drain_prefix(proposal_id) {
                let new_count = GovernanceLockCount::<T>::mutate(entity_id, &voter, |c| {
                    *c = c.saturating_sub(1);
                    *c
                });
                if new_count == 0 {
                    let locked = GovernanceLockAmount::<T>::take(entity_id, &voter);
                    if !locked.is_zero() {
                        T::TokenProvider::unreserve(entity_id, &voter, locked);
                    }
                    GovernanceLockCount::<T>::remove(entity_id, &voter);
                }
            }
            let _ = VotingPowerSnapshot::<T>::clear_prefix(proposal_id, 500, None);
        }

        /// 从活跃提案列表移除，解锁投票者代币，清理投票权快照
        fn remove_from_active(proposal_id: ProposalId, entity_id: u64) {
            EntityProposals::<T>::mutate(entity_id, |proposals| {
                proposals.retain(|&id| id != proposal_id);
            });
            Self::unlock_voters_for_proposal(proposal_id, entity_id);
            // F9: VoteRecords 延迟清理到 cleanup_proposal，保留投票历史供审计
        }

        /// R1-R11: off-chain 执行备注 helper（消除 39 个重复 match arm）
        fn emit_offchain_note(proposal_id: ProposalId, note: &str) -> DispatchResult {
            Self::deposit_event(Event::ProposalExecutionNote {
                proposal_id,
                note: note.as_bytes().to_vec(),
            });
            Ok(())
        }

        /// R2: 共享 finalize 计算逻辑
        ///
        /// 计算投票结果（Passed/Failed），更新 proposal 字段，调用 remove_from_active。
        /// 调用者负责写入存储和发出事件。
        fn compute_finalize_result(
            proposal_id: ProposalId,
            proposal: &mut ProposalOf<T>,
            now: BlockNumberFor<T>,
        ) -> ProposalStatus {
            let total_votes = proposal.yes_votes
                .saturating_add(proposal.no_votes)
                .saturating_add(proposal.abstain_votes);

            let quorum_threshold: BalanceOf<T> = proposal.snapshot_total_supply
                .saturating_mul(proposal.snapshot_quorum.into())
                / 100u128.into();

            let new_status = if total_votes < quorum_threshold {
                ProposalStatus::Failed
            } else {
                let decisive_votes = proposal.yes_votes.saturating_add(proposal.no_votes);
                let pass_threshold: BalanceOf<T> = decisive_votes
                    .saturating_mul(proposal.snapshot_pass.into())
                    / 100u128.into();

                if proposal.yes_votes > pass_threshold {
                    proposal.execution_time = Some(now.saturating_add(proposal.snapshot_execution_delay));
                    ProposalStatus::Passed
                } else {
                    ProposalStatus::Failed
                }
            };

            proposal.status = new_status.clone();
            Self::remove_from_active(proposal_id, proposal.entity_id);
            new_status
        }

        /// on_idle 自动 finalize 过期提案
        fn auto_finalize_proposal(
            proposal_id: ProposalId,
            mut proposal: ProposalOf<T>,
            now: BlockNumberFor<T>,
        ) {
            let new_status = match proposal.status {
                ProposalStatus::Voting => {
                    let result = Self::compute_finalize_result(proposal_id, &mut proposal, now);
                    Proposals::<T>::insert(proposal_id, &proposal);
                    match result {
                        ProposalStatus::Passed => Self::deposit_event(Event::ProposalPassed { proposal_id }),
                        _ => Self::deposit_event(Event::ProposalFailed { proposal_id }),
                    }
                    result
                },
                ProposalStatus::Passed => {
                    proposal.status = ProposalStatus::Expired;
                    Proposals::<T>::insert(proposal_id, &proposal);
                    Self::deposit_event(Event::ProposalExpired { proposal_id });
                    ProposalStatus::Expired
                },
                _ => return,
            };

            Self::deposit_event(Event::ProposalAutoFinalized {
                proposal_id,
                new_status,
            });
        }

        /// 执行提案
        fn do_execute_proposal(proposal: &ProposalOf<T>) -> DispatchResult {
            let entity_id = proposal.entity_id;
            
            match &proposal.proposal_type {
                // ==================== 商品管理类 ====================
                ProposalType::PriceChange { product_id, new_price } => {
                    // F3: 价格变更通过 ProductProvider 链上执行
                    T::ProductProvider::update_price(*product_id, *new_price)
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
                    T::ProductProvider::delist_product(*product_id)
                },
                ProposalType::InventoryAdjustment { product_id, new_inventory } => {
                    // F3: 库存调整通过 ProductProvider 链上执行
                    let inv: u32 = (*new_inventory).saturated_into();
                    T::ProductProvider::set_inventory(*product_id, inv)
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
                    T::TokenProvider::governance_burn(entity_id, *amount)
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
                    // Phase 4.3: 资金保护检查（预警但不阻断）
                    Self::check_fund_protection(entity_id, *amount);
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
                // F1: 执行延迟调整（执行时再次验证最小值）
                ProposalType::ExecutionDelayChange { new_delay_blocks } => {
                    let delay: BlockNumberFor<T> = (*new_delay_blocks).into();
                    ensure!(delay >= T::MinExecutionDelay::get(), Error::<T>::ExecutionDelayTooShort);
                    GovernanceConfigs::<T>::mutate(entity_id, |maybe_config| {
                        let config = maybe_config.get_or_insert_with(GovernanceConfigOf::<T>::default);
                        config.execution_delay = delay;
                    });
                    Ok(())
                },
                // F1: 通过阈值调整
                ProposalType::PassThresholdChange { new_pass } => {
                    GovernanceConfigs::<T>::mutate(entity_id, |maybe_config| {
                        let config = maybe_config.get_or_insert_with(GovernanceConfigOf::<T>::default);
                        config.pass_threshold = *new_pass;
                    });
                    Ok(())
                },
                // F1: 管理员否决权开关
                ProposalType::AdminVetoToggle { enabled } => {
                    GovernanceConfigs::<T>::mutate(entity_id, |maybe_config| {
                        let config = maybe_config.get_or_insert_with(GovernanceConfigOf::<T>::default);
                        config.admin_veto_enabled = *enabled;
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
                ProposalType::MultiLevelChange { ref tiers, max_total_rate } => {
                    T::MultiLevelWriter::set_multi_level_full(
                        entity_id,
                        tiers.to_vec(),
                        *max_total_rate,
                    )
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

                // ==================== 团队业绩返佣配置类（F5）====================
                ProposalType::TeamPerformanceChange { ref tiers, max_depth, allow_stacking, threshold_mode } => {
                    T::TeamWriter::set_team_config(
                        entity_id,
                        tiers.iter().cloned().collect(),
                        *max_depth,
                        *allow_stacking,
                        *threshold_mode,
                    )
                },

                // ==================== 披露管理类（F10）====================
                ProposalType::DisclosureLevelChange { level, insider_trading_control, blackout_period_after } => {
                    use pallet_entity_common::DisclosureLevel;
                    let dl = match level {
                        0 => DisclosureLevel::Basic,
                        1 => DisclosureLevel::Standard,
                        2 => DisclosureLevel::Enhanced,
                        _ => DisclosureLevel::Full,
                    };
                    T::DisclosureProvider::governance_configure_disclosure(
                        entity_id, dl, *insider_trading_control, (*blackout_period_after).into(),
                    )
                },
                ProposalType::DisclosureResetViolations => {
                    T::DisclosureProvider::governance_reset_violations(entity_id)
                },

                // ==================== DAO 可控紧急权限类（R8）====================
                ProposalType::EmergencyPauseToggle { enabled } => {
                    EmergencyPauseEnabled::<T>::insert(entity_id, *enabled);
                    Ok(())
                },
                ProposalType::BatchCancelToggle { enabled } => {
                    BatchCancelEnabled::<T>::insert(entity_id, *enabled);
                    Ok(())
                },

                // ==================== 社区类 ====================
                ProposalType::CommunityEvent { .. } | ProposalType::RuleSuggestion { .. } | ProposalType::General { .. } => {
                    Ok(())
                },

                // ==================== R10: 链上执行（已接入 Provider trait）====================

                // -- 代币扩展类 --
                ProposalType::TokenMaxSupplyChange { new_max_supply } => {
                    T::TokenProvider::governance_set_max_supply(entity_id, *new_max_supply)
                },
                ProposalType::TokenTypeChange { new_type } => {
                    use pallet_entity_common::TokenType as TT;
                    let tt = match new_type {
                        0 => TT::Points,
                        1 => TT::Governance,
                        2 => TT::Equity,
                        _ => TT::Membership,
                    };
                    T::TokenProvider::governance_set_token_type(entity_id, tt)
                },
                ProposalType::TransferRestrictionChange { restriction, min_receiver_kyc } => {
                    T::TokenProvider::governance_set_transfer_restriction(entity_id, *restriction, *min_receiver_kyc)
                },

                // -- 返佣核心配置类 --
                ProposalType::CommissionRateChange { new_rate } => {
                    T::CommissionProvider::governance_set_commission_rate(entity_id, *new_rate)
                },
                ProposalType::CommissionToggle { enabled } => {
                    T::CommissionProvider::governance_toggle_commission(entity_id, *enabled)
                },
                ProposalType::CreatorRewardRateChange { new_rate } => {
                    T::CommissionProvider::set_creator_reward_rate(entity_id, *new_rate)
                },

                // -- 店铺扩展类 --
                ProposalType::ShopClose { shop_id } => {
                    let entity_shops = T::EntityProvider::entity_shops(entity_id);
                    ensure!(entity_shops.contains(shop_id), Error::<T>::ShopNotFound);
                    T::ShopProvider::governance_close_shop(*shop_id)
                },
                ProposalType::ShopTypeChange { shop_id, new_type } => {
                    let entity_shops = T::EntityProvider::entity_shops(entity_id);
                    ensure!(entity_shops.contains(shop_id), Error::<T>::ShopNotFound);
                    use pallet_entity_common::ShopType;
                    let st = match new_type {
                        0 => ShopType::OnlineStore,
                        1 => ShopType::PhysicalStore,
                        2 => ShopType::ServicePoint,
                        _ => ShopType::Virtual,
                    };
                    T::ShopProvider::governance_set_shop_type(*shop_id, st)
                },

                // -- 商品扩展类 --
                ProposalType::ProductVisibilityChange { product_id, visibility } => {
                    use pallet_entity_common::ProductVisibility;
                    let vis = match visibility {
                        0 => ProductVisibility::Public,
                        1 => ProductVisibility::MembersOnly,
                        v => ProductVisibility::LevelGated(*v),
                    };
                    T::ProductProvider::governance_set_visibility(*product_id, vis)
                },

                // -- 披露扩展类 --
                ProposalType::DisclosurePenaltyChange { level } => {
                    T::DisclosureProvider::governance_set_penalty_level(entity_id, *level)
                },

                // ==================== Phase 4.2: 链上执行（通过 XxxGovernancePort）====================

                // -- 市场管理类 --
                ProposalType::MarketConfigChange { min_order_amount, order_ttl } => {
                    T::MarketGovernance::governance_set_market_config(entity_id, *min_order_amount, *order_ttl)
                },
                ProposalType::MarketPause => {
                    T::MarketGovernance::governance_pause_market(entity_id)
                },
                ProposalType::MarketResume => {
                    T::MarketGovernance::governance_resume_market(entity_id)
                },
                ProposalType::MarketClose => {
                    T::MarketGovernance::governance_close_market(entity_id)
                },
                ProposalType::PriceProtectionChange { max_price_deviation, max_slippage, circuit_breaker_threshold, min_trades_for_twap } => {
                    T::MarketGovernance::governance_set_price_protection(entity_id, *max_price_deviation, *max_slippage, *circuit_breaker_threshold, *min_trades_for_twap)
                },
                ProposalType::MarketKycChange { min_kyc_level } => {
                    T::MarketGovernance::governance_set_market_kyc(entity_id, *min_kyc_level)
                },
                ProposalType::CircuitBreakerLift => {
                    T::MarketGovernance::governance_lift_circuit_breaker(entity_id)
                },

                // -- 单线收益类 --
                ProposalType::SingleLineConfigChange { upline_rate, downline_rate, base_upline_levels, base_downline_levels, max_upline_levels, max_downline_levels } => {
                    T::SingleLineGovernance::governance_set_single_line_config(entity_id, *upline_rate, *downline_rate, *base_upline_levels, *base_downline_levels, *max_upline_levels, *max_downline_levels)
                },
                ProposalType::SingleLinePause => {
                    T::SingleLineGovernance::governance_pause_single_line(entity_id)
                },
                ProposalType::SingleLineResume => {
                    T::SingleLineGovernance::governance_resume_single_line(entity_id)
                },

                // -- 代币扩展类 --
                ProposalType::TokenBlacklistManage { account_cid, add } => {
                    T::TokenGovernance::governance_manage_blacklist(entity_id, account_cid.as_slice(), *add)
                },

                // -- 返佣扩展类 --
                ProposalType::WithdrawalCooldownChange { nex_cooldown, token_cooldown } => {
                    T::CommissionGovernance::governance_set_withdrawal_cooldown(entity_id, *nex_cooldown, *token_cooldown)
                },
                ProposalType::TokenWithdrawalConfigChange { enabled } => {
                    T::CommissionGovernance::governance_set_token_withdrawal(entity_id, *enabled)
                },
                ProposalType::WithdrawalPauseToggle { paused } => {
                    T::CommissionGovernance::governance_set_withdrawal_pause(entity_id, *paused)
                },
                ProposalType::ReferrerGuardChange { min_referrer_spent, min_referrer_orders } => {
                    T::CommissionGovernance::governance_set_referrer_guard(entity_id, *min_referrer_spent, *min_referrer_orders)
                },
                ProposalType::CommissionCapChange { max_per_order, max_total_earned } => {
                    T::CommissionGovernance::governance_set_commission_cap(entity_id, *max_per_order, *max_total_earned)
                },
                ProposalType::ReferralValidityChange { validity_blocks, valid_orders } => {
                    T::CommissionGovernance::governance_set_referral_validity(entity_id, *validity_blocks, *valid_orders)
                },
                ProposalType::MultiLevelPause => {
                    T::CommissionGovernance::governance_pause_multi_level(entity_id)
                },
                ProposalType::MultiLevelResume => {
                    T::CommissionGovernance::governance_resume_multi_level(entity_id)
                },
                ProposalType::TeamPerformancePause => {
                    T::CommissionGovernance::governance_pause_team_performance(entity_id)
                },
                ProposalType::TeamPerformanceResume => {
                    T::CommissionGovernance::governance_resume_team_performance(entity_id)
                },

                // -- 会员管理类 --
                ProposalType::MemberPolicyChange { policy } => {
                    T::MemberProvider::set_registration_policy(entity_id, *policy)
                },
                ProposalType::UpgradeRuleToggle { enabled } => {
                    T::MemberProvider::set_upgrade_rule_system_enabled(entity_id, *enabled)
                },
                ProposalType::MemberStatsPolicyChange { qualified_only, .. } => {
                    let policy_bits: u8 = if *qualified_only { 0 } else { 0b0000_0011 };
                    T::MemberProvider::set_stats_policy(entity_id, policy_bits)
                },

                // -- KYC 管理类 --
                ProposalType::KycRequirementChange { min_level, mandatory, grace_period } => {
                    T::KycGovernance::governance_set_kyc_requirement(entity_id, *min_level, *mandatory, *grace_period)
                },
                ProposalType::KycProviderAuthorize { provider_id } => {
                    T::KycGovernance::governance_authorize_kyc_provider(entity_id, *provider_id)
                },
                ProposalType::KycProviderDeauthorize { provider_id } => {
                    T::KycGovernance::governance_deauthorize_kyc_provider(entity_id, *provider_id)
                },

                // -- 店铺扩展类 --
                ProposalType::PointsConfigChange { reward_rate, exchange_rate, transferable } => {
                    T::ShopGovernance::governance_set_points_config(entity_id, *reward_rate, *exchange_rate, *transferable)
                },
                ProposalType::PointsToggle { enabled } => {
                    T::ShopGovernance::governance_toggle_points(entity_id, *enabled)
                },
                ProposalType::ShopPoliciesChange { policies_cid } => {
                    T::ShopGovernance::governance_set_shop_policies(entity_id, policies_cid.as_slice())
                },

                // -- 披露链下类（仍需 CID 链下解析）--
                ProposalType::DisclosureInsiderManage { .. } => Self::emit_offchain_note(proposal.id, "DisclosureInsiderManage approved"),

                // ==================== Phase 4.3: 资金保护 ====================
                ProposalType::FundProtectionChange { min_treasury_threshold, max_single_spend, max_daily_spend } => {
                    FundProtectionConfigs::<T>::insert(entity_id, FundProtectionConfig {
                        min_treasury_threshold: *min_treasury_threshold,
                        max_single_spend: *max_single_spend,
                        max_daily_spend: *max_daily_spend,
                    });
                    Self::deposit_event(Event::FundProtectionUpdated {
                        entity_id,
                        min_treasury_threshold: *min_treasury_threshold,
                        max_single_spend: *max_single_spend,
                        max_daily_spend: *max_daily_spend,
                    });
                    Ok(())
                },
            }
        }

        /// Phase 4.3: 资金保护检查（在 TreasurySpend 等提案执行前调用）
        ///
        /// 不阻断执行，仅发出预警事件。
        fn check_fund_protection(entity_id: u64, spend_amount: BalanceOf<T>) {
            if let Some(config) = FundProtectionConfigs::<T>::get(entity_id) {
                // 检查金库余额阈值
                if !config.min_treasury_threshold.is_zero() {
                    let treasury_balance: BalanceOf<T> = T::TreasuryPort::treasury_balance(entity_id).saturated_into();
                    if treasury_balance < config.min_treasury_threshold {
                        Self::deposit_event(Event::FundProtectionAlert {
                            entity_id,
                            alert_type: FundAlertType::TreasuryBelowThreshold,
                            current_value: treasury_balance,
                            threshold: config.min_treasury_threshold,
                        });
                    }
                }

                // 检查单笔支出限额
                if !config.max_single_spend.is_zero() && spend_amount > config.max_single_spend {
                    Self::deposit_event(Event::FundProtectionAlert {
                        entity_id,
                        alert_type: FundAlertType::SingleSpendExceeded,
                        current_value: spend_amount,
                        threshold: config.max_single_spend,
                    });
                }

                // 检查每日累计支出限额
                if !config.max_daily_spend.is_zero() {
                    let now = <frame_system::Pallet<T>>::block_number();
                    // 按 14400 blocks/day 估算 (6s 出块)
                    let day_number: u32 = now.saturated_into::<u32>() / 14400u32;
                    let (tracked_day, tracked_amount) = DailySpendTracker::<T>::get(entity_id);

                    let new_amount = if tracked_day == day_number {
                        tracked_amount.saturating_add(spend_amount)
                    } else {
                        spend_amount
                    };

                    DailySpendTracker::<T>::insert(entity_id, (day_number, new_amount));

                    if new_amount > config.max_daily_spend {
                        Self::deposit_event(Event::FundProtectionAlert {
                            entity_id,
                            alert_type: FundAlertType::DailySpendExceeded,
                            current_value: new_amount,
                            threshold: config.max_daily_spend,
                        });
                    }
                }
            }
        }
    }
}

// ============================================================================
// GovernanceProvider 实现
// ============================================================================

use pallet_entity_common::GovernanceProvider;

impl<T: Config> GovernanceProvider for Pallet<T> {
    fn governance_mode(entity_id: u64) -> pallet_entity_common::GovernanceMode {
        pallet::GovernanceConfigs::<T>::get(entity_id)
            .map(|c| c.mode)
            .unwrap_or(pallet_entity_common::GovernanceMode::None)
    }

    fn has_active_proposals(entity_id: u64) -> bool {
        !pallet::EntityProposals::<T>::get(entity_id).is_empty()
    }

    fn is_governance_locked(entity_id: u64) -> bool {
        pallet::GovernanceLocked::<T>::get(entity_id)
    }

    fn is_governance_paused(entity_id: u64) -> bool {
        pallet::GovernancePaused::<T>::get(entity_id)
    }
}
