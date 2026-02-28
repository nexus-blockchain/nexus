//! # Entity Commission (pallet-entity-commission)
//!
//! 返佣系统 re-export wrapper。
//!
//! 插件化架构，由以下子模块组成：
//! - `pallet-commission-common` — 共享类型 + CommissionPlugin trait
//! - `pallet-commission-core` — 调度引擎 + 记账 + 提现 + 偿付安全
//! - `pallet-commission-referral` — 推荐链返佣（Direct/Multi/Fixed/First/Repeat）
//! - `pallet-commission-level-diff` — 等级极差返佣
//! - `pallet-commission-single-line` — 单线收益（上线/下线）
//! - `pallet-commission-pool-reward` — 沉淀池奖励（未分配佣金回馈高级别会员）

#![cfg_attr(not(feature = "std"), no_std)]

// Re-export all sub-crates for backward compatibility
pub use pallet_commission_common;
pub use pallet_commission_core;
pub use pallet_commission_referral;
pub use pallet_commission_level_diff;
pub use pallet_commission_single_line;
pub use pallet_commission_pool_reward;

// Re-export commonly used traits and types at crate root
pub use pallet_commission_common::{
    CommissionModes, CommissionOutput, CommissionPlugin, CommissionPlan, CommissionProvider,
    CommissionRecord, CommissionStatus, CommissionType,
    LevelDiffPlanWriter, MemberCommissionStatsData, MemberProvider,
    NullCommissionProvider, NullMemberProvider, PoolRewardPlanWriter, ReferralPlanWriter,
    WithdrawalTierConfig,
};
