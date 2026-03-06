//! # Entity Commission (pallet-entity-commission)
//!
//! 返佣系统 re-export wrapper，支持 NEX + Entity Token 双资产全管线返佣。
//!
//! 插件化架构，由以下子模块组成：
//! - `pallet-commission-common` — 共享类型 + CommissionPlugin / TokenCommissionPlugin trait
//! - `pallet-commission-core` — 调度引擎 + 记账 + 提现 + 偿付安全 + 创建人收益
//! - `pallet-commission-referral` — 推荐链返佣（直推/固定金额/首单/复购）
//! - `pallet-commission-multi-level` — 多级分销（N 层 + 三维激活条件）
//! - `pallet-commission-level-diff` — 等级极差返佣（自定义等级体系）
//! - `pallet-commission-single-line` — 单线收益（上线/下线，分段存储）
//! - `pallet-commission-pool-reward` — 沉淀池奖励（周期性等额分配）
//!
//! `pallet-commission-team`（团队业绩阶梯奖金）作为独立 crate 存在，
//! 通过 runtime 层配置 `TeamPlugin` / `TeamWriter` 接入核心引擎，不在本 umbrella crate 内 re-export。

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet_commission_common;
pub use pallet_commission_core;
pub use pallet_commission_referral;
pub use pallet_commission_multi_level;
pub use pallet_commission_level_diff;
pub use pallet_commission_single_line;
pub use pallet_commission_pool_reward;

pub use pallet_commission_common::{
    // NEX 核心类型
    CommissionModes, CommissionOutput, CommissionPlugin, CommissionProvider,
    CommissionRecord, CommissionStatus, CommissionType,
    MemberCommissionStatsData, NullCommissionProvider,
    WithdrawalMode, WithdrawalTierConfig,
    // Token 双资产类型
    TokenCommissionPlugin, TokenCommissionProvider, TokenCommissionRecord,
    MemberTokenCommissionStatsData, NullTokenCommissionProvider,
    TokenTransferProvider, TokenPoolBalanceProvider,
    // PlanWriter traits（供 Governance 写入各插件配置）
    ReferralPlanWriter, MultiLevelPlanWriter, LevelDiffPlanWriter,
    TeamPlanWriter, SingleLinePlanWriter, PoolRewardPlanWriter,
    // Provider traits
    MemberProvider, NullMemberProvider,
    EntityReferrerProvider, PoolBalanceProvider,
    ParticipationGuard,
};
