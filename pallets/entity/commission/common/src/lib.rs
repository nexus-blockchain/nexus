//! # Commission Common Types
//!
//! Shared types and traits for the commission plugin system.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_runtime::DispatchError;

// ============================================================================
// 返佣模式位标志
// ============================================================================

/// 返佣模式位标志（可多选）
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub struct CommissionModes(pub u16);

impl CommissionModes {
    pub const NONE: u16 = 0b0000_0000;
    pub const DIRECT_REWARD: u16 = 0b0000_0001;
    pub const MULTI_LEVEL: u16 = 0b0000_0010;
    pub const TEAM_PERFORMANCE: u16 = 0b0000_0100;
    pub const LEVEL_DIFF: u16 = 0b0000_1000;
    pub const FIXED_AMOUNT: u16 = 0b0001_0000;
    pub const FIRST_ORDER: u16 = 0b0010_0000;
    pub const REPEAT_PURCHASE: u16 = 0b0100_0000;
    pub const SINGLE_LINE_UPLINE: u16 = 0b1000_0000;
    pub const SINGLE_LINE_DOWNLINE: u16 = 0b1_0000_0000;
    pub const POOL_REWARD: u16 = 0b10_0000_0000;
    pub const CREATOR_REWARD: u16 = 0b100_0000_0000;

    /// 所有已定义模式位的并集（单一事实来源）
    pub const ALL_VALID: u16 =
        Self::DIRECT_REWARD
        | Self::MULTI_LEVEL
        | Self::TEAM_PERFORMANCE
        | Self::LEVEL_DIFF
        | Self::FIXED_AMOUNT
        | Self::FIRST_ORDER
        | Self::REPEAT_PURCHASE
        | Self::SINGLE_LINE_UPLINE
        | Self::SINGLE_LINE_DOWNLINE
        | Self::POOL_REWARD
        | Self::CREATOR_REWARD;

    /// 检查是否仅包含已定义的模式位（无未知高位）
    pub fn is_valid(&self) -> bool {
        self.0 & !Self::ALL_VALID == 0
    }

    pub fn contains(&self, flag: u16) -> bool {
        self.0 & flag != 0
    }

    pub fn insert(&mut self, flag: u16) {
        self.0 |= flag;
    }

    pub fn remove(&mut self, flag: u16) {
        self.0 &= !flag;
    }
}

// ============================================================================
// 返佣类型 / 状态
// ============================================================================

/// 返佣类型
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub enum CommissionType {
    DirectReward,
    MultiLevel,
    TeamPerformance,
    LevelDiff,
    FixedAmount,
    FirstOrder,
    RepeatPurchase,
    SingleLineUpline,
    SingleLineDownline,
    EntityReferral,
    PoolReward,
    CreatorReward,
}

/// 返佣状态
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum CommissionStatus {
    #[default]
    Pending,
    /// [已废弃] 保留以维持 SCALE 编码索引稳定，生产代码从未使用此状态
    Distributed,
    /// 订单已完结，佣金已结算（由 settle_order_commission 设置）
    Withdrawn,
    Cancelled,
}

// ============================================================================
// 返佣记录
// ============================================================================

/// 返佣记录
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct CommissionRecord<AccountId, Balance, BlockNumber> {
    pub entity_id: u64,
    pub shop_id: u64,
    pub order_id: u64,
    pub buyer: AccountId,
    pub beneficiary: AccountId,
    pub amount: Balance,
    pub commission_type: CommissionType,
    pub level: u8,
    pub status: CommissionStatus,
    pub created_at: BlockNumber,
}

/// 会员返佣统计
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub struct MemberCommissionStatsData<Balance> {
    pub total_earned: Balance,
    pub pending: Balance,
    pub withdrawn: Balance,
    pub repurchased: Balance,
    pub order_count: u32,
}

// ============================================================================
// 提现配置
// ============================================================================

/// 分级提现配置
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct WithdrawalTierConfig {
    pub withdrawal_rate: u16,
    pub repurchase_rate: u16,
}

impl Default for WithdrawalTierConfig {
    fn default() -> Self {
        Self {
            withdrawal_rate: 10000,
            repurchase_rate: 0,
        }
    }
}

impl WithdrawalTierConfig {
    /// 校验 withdrawal_rate + repurchase_rate == 10000
    pub fn is_valid(&self) -> bool {
        self.withdrawal_rate.saturating_add(self.repurchase_rate) == 10000
    }
}

/// 提现模式
///
/// 决定佣金提现时复购比率的确定方式。
/// 无论选择哪种模式，Governance 设定的全局最低复购比率始终生效。
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum WithdrawalMode {
    /// 全额提现：不强制复购（Governance 底线仍生效）
    #[default]
    FullWithdrawal,
    /// 固定比率：所有会员统一复购比率
    FixedRate { repurchase_rate: u16 },
    /// 按等级自动决定：通过 default_tier + level_overrides 查表
    LevelBased,
    /// 会员自选：会员提现时指定复购比率，不低于 min_repurchase_rate
    MemberChoice { min_repurchase_rate: u16 },
}

// ============================================================================
// 插件输出
// ============================================================================

/// 单条返佣输出（插件计算结果）
#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub struct CommissionOutput<AccountId, Balance> {
    pub beneficiary: AccountId,
    pub amount: Balance,
    pub commission_type: CommissionType,
    pub level: u8,
}

// ============================================================================
// CommissionPlugin Trait
// ============================================================================

/// 返佣插件接口
///
/// 每个返佣模式实现此 trait，由 core 调度引擎调用。
/// `calculate` 接收订单上下文和剩余可分配额度，返回返佣输出列表和更新后的剩余额度。
pub trait CommissionPlugin<AccountId, Balance> {
    /// 计算返佣
    ///
    /// # 参数
    /// - `entity_id`: 实体 ID（用于插件配置查询和 MemberProvider 查询推荐链）
    /// - `buyer`: 买家账户
    /// - `order_amount`: 订单金额
    /// - `remaining`: 剩余可分配额度
    /// - `enabled_modes`: 启用的返佣模式位标志
    /// - `is_first_order`: 是否首单
    /// - `buyer_order_count`: 买家订单数
    ///
    /// # 返回
    /// `(outputs, new_remaining)` — 返佣输出列表和剩余额度
    fn calculate(
        entity_id: u64,
        buyer: &AccountId,
        order_amount: Balance,
        remaining: Balance,
        enabled_modes: CommissionModes,
        is_first_order: bool,
        buyer_order_count: u32,
    ) -> (Vec<CommissionOutput<AccountId, Balance>>, Balance);
}

/// 空插件实现（占位）
impl<AccountId, Balance> CommissionPlugin<AccountId, Balance> for () {
    fn calculate(
        _entity_id: u64,
        _buyer: &AccountId,
        _order_amount: Balance,
        remaining: Balance,
        _enabled_modes: CommissionModes,
        _is_first_order: bool,
        _buyer_order_count: u32,
    ) -> (Vec<CommissionOutput<AccountId, Balance>>, Balance) {
        (Vec::new(), remaining)
    }
}

// ============================================================================
// CommissionProvider Trait（供外部模块调用）
// ============================================================================

/// 返佣服务接口（所有方法统一使用 entity_id）
pub trait CommissionProvider<AccountId, Balance> {
    fn process_commission(
        entity_id: u64,
        shop_id: u64,
        order_id: u64,
        buyer: &AccountId,
        order_amount: Balance,
        available_pool: Balance,
        platform_fee: Balance,
    ) -> Result<(), DispatchError>;

    fn cancel_commission(order_id: u64) -> Result<(), DispatchError>;

    fn pending_commission(entity_id: u64, account: &AccountId) -> Balance;

    fn set_commission_modes(entity_id: u64, modes: u16) -> Result<(), DispatchError>;

    fn set_direct_reward_rate(entity_id: u64, rate: u16) -> Result<(), DispatchError>;

    fn set_level_diff_config(
        entity_id: u64,
        level_rates: Vec<u16>,
    ) -> Result<(), DispatchError>;

    fn set_fixed_amount(entity_id: u64, amount: Balance) -> Result<(), DispatchError>;

    fn set_first_order_config(
        entity_id: u64,
        amount: Balance,
        rate: u16,
        use_amount: bool,
    ) -> Result<(), DispatchError>;

    fn set_repeat_purchase_config(entity_id: u64, rate: u16, min_orders: u32) -> Result<(), DispatchError>;

    fn set_withdrawal_config_by_governance(
        entity_id: u64,
        enabled: bool,
    ) -> Result<(), DispatchError>;

    fn shopping_balance(entity_id: u64, account: &AccountId) -> Balance;

    /// 使用购物余额（由订单模块调用）
    fn use_shopping_balance(entity_id: u64, account: &AccountId, amount: Balance) -> Result<(), DispatchError>;

    /// 设置全局最低复购比例（由 Governance 调用，万分比）
    fn set_min_repurchase_rate(entity_id: u64, rate: u16) -> Result<(), DispatchError>;

    /// 设置创建人收益比例（基点，从 Pool B 预算中优先扣除）
    fn set_creator_reward_rate(entity_id: u64, rate: u16) -> Result<(), DispatchError>;

    /// 订单完结时结算佣金记录（Pending → Withdrawn）
    ///
    /// 由订单模块在订单完结（确认收货/超时完成）时调用，
    /// 标记佣金记录为已结算，使其可以被 archive_order_records 归档。
    fn settle_order_commission(order_id: u64) -> Result<(), DispatchError>;

    // ==================== R10: 治理提案链上执行接口 ====================

    /// 设置最大返佣比率（治理提案执行）
    fn governance_set_commission_rate(entity_id: u64, rate: u16) -> Result<(), DispatchError> {
        let _ = (entity_id, rate);
        Ok(())
    }

    /// 返佣总开关（治理提案执行）
    fn governance_toggle_commission(entity_id: u64, enabled: bool) -> Result<(), DispatchError> {
        let _ = (entity_id, enabled);
        Ok(())
    }
}

/// 空 CommissionProvider 实现
pub struct NullCommissionProvider;

impl<AccountId, Balance: Default> CommissionProvider<AccountId, Balance> for NullCommissionProvider {
    fn process_commission(_: u64, _: u64, _: u64, _: &AccountId, _: Balance, _: Balance, _: Balance) -> Result<(), DispatchError> { Ok(()) }
    fn cancel_commission(_: u64) -> Result<(), DispatchError> { Ok(()) }
    fn pending_commission(_: u64, _: &AccountId) -> Balance { Balance::default() }
    fn set_commission_modes(_: u64, _: u16) -> Result<(), DispatchError> { Ok(()) }
    fn set_direct_reward_rate(_: u64, _: u16) -> Result<(), DispatchError> { Ok(()) }
    fn set_level_diff_config(_: u64, _: Vec<u16>) -> Result<(), DispatchError> { Ok(()) }
    fn set_fixed_amount(_: u64, _: Balance) -> Result<(), DispatchError> { Ok(()) }
    fn set_first_order_config(_: u64, _: Balance, _: u16, _: bool) -> Result<(), DispatchError> { Ok(()) }
    fn set_repeat_purchase_config(_: u64, _: u16, _: u32) -> Result<(), DispatchError> { Ok(()) }
    fn set_withdrawal_config_by_governance(_: u64, _: bool) -> Result<(), DispatchError> { Ok(()) }
    fn shopping_balance(_: u64, _: &AccountId) -> Balance { Balance::default() }
    fn use_shopping_balance(_: u64, _: &AccountId, _: Balance) -> Result<(), DispatchError> { Ok(()) }
    fn set_min_repurchase_rate(_: u64, _: u16) -> Result<(), DispatchError> { Ok(()) }
    fn set_creator_reward_rate(_: u64, _: u16) -> Result<(), DispatchError> { Ok(()) }
    fn settle_order_commission(_: u64) -> Result<(), DispatchError> { Ok(()) }
}

// ============================================================================
// MemberProvider Trait — 从 pallet-entity-common 统一导出
// ============================================================================

/// 从 `pallet-entity-common` 统一导出，消除重复定义。
pub use pallet_entity_common::{MemberProvider, NullMemberProvider};

// ============================================================================
// EntityReferrerProvider — 招商推荐人查询接口
// ============================================================================

/// 招商推荐人查询接口（供 commission-core 查询 Entity 级推荐人）
pub trait EntityReferrerProvider<AccountId> {
    /// 获取 Entity 的招商推荐人
    fn entity_referrer(entity_id: u64) -> Option<AccountId>;
}

/// 空 EntityReferrerProvider 实现
impl<AccountId> EntityReferrerProvider<AccountId> for () {
    fn entity_referrer(_entity_id: u64) -> Option<AccountId> { None }
}

// ============================================================================
// PlanWriter Traits — 插件写入接口
// ============================================================================

/// 推荐链插件写入接口（由 commission-referral 实现）
pub trait ReferralPlanWriter<Balance> {
    /// 设置直推奖励比例
    fn set_direct_rate(entity_id: u64, rate: u16) -> Result<(), DispatchError>;
    /// 设置固定金额奖励
    fn set_fixed_amount(entity_id: u64, amount: Balance) -> Result<(), DispatchError>;
    /// 设置首单奖励
    fn set_first_order(entity_id: u64, amount: Balance, rate: u16, use_amount: bool) -> Result<(), DispatchError>;
    /// 设置复购奖励
    fn set_repeat_purchase(entity_id: u64, rate: u16, min_orders: u32) -> Result<(), DispatchError>;
    /// 清除全部推荐链配置
    fn clear_config(entity_id: u64) -> Result<(), DispatchError>;
}

/// 空 ReferralPlanWriter 实现
impl<Balance> ReferralPlanWriter<Balance> for () {
    fn set_direct_rate(_: u64, _: u16) -> Result<(), DispatchError> { Ok(()) }
    fn set_fixed_amount(_: u64, _: Balance) -> Result<(), DispatchError> { Ok(()) }
    fn set_first_order(_: u64, _: Balance, _: u16, _: bool) -> Result<(), DispatchError> { Ok(()) }
    fn set_repeat_purchase(_: u64, _: u16, _: u32) -> Result<(), DispatchError> { Ok(()) }
    fn clear_config(_: u64) -> Result<(), DispatchError> { Ok(()) }
}

/// 多级分销插件写入接口（由 commission-multi-level 实现）
pub trait MultiLevelPlanWriter {
    /// 设置多级分销（每级比例列表 + 上限比例，激活条件全为 0）
    fn set_multi_level(entity_id: u64, level_rates: Vec<u16>, max_total_rate: u16) -> Result<(), DispatchError>;
    /// F7: 设置多级分销（含完整激活条件）
    ///
    /// tiers: Vec<(rate, required_directs, required_team_size, required_spent)>
    fn set_multi_level_full(
        entity_id: u64,
        tiers: Vec<(u16, u32, u32, u128)>,
        max_total_rate: u16,
    ) -> Result<(), DispatchError>;
    /// 清除多级分销配置
    fn clear_multi_level_config(entity_id: u64) -> Result<(), DispatchError>;
    /// 暂停多级分销（治理调用）
    fn governance_pause(entity_id: u64) -> Result<(), DispatchError> {
        let _ = entity_id;
        Ok(())
    }
    /// 恢复多级分销（治理调用）
    fn governance_resume(entity_id: u64) -> Result<(), DispatchError> {
        let _ = entity_id;
        Ok(())
    }
}

/// 空 MultiLevelPlanWriter 实现
impl MultiLevelPlanWriter for () {
    fn set_multi_level(_: u64, _: Vec<u16>, _: u16) -> Result<(), DispatchError> { Ok(()) }
    fn set_multi_level_full(_: u64, _: Vec<(u16, u32, u32, u128)>, _: u16) -> Result<(), DispatchError> { Ok(()) }
    fn clear_multi_level_config(_: u64) -> Result<(), DispatchError> { Ok(()) }
}

/// 等级极差插件写入接口（由 commission-level-diff 实现）
pub trait LevelDiffPlanWriter {
    /// 设置自定义等级极差比例（level_rates: 每个自定义等级对应的 bps）
    fn set_level_rates(entity_id: u64, level_rates: Vec<u16>, max_depth: u8) -> Result<(), DispatchError>;
    /// 清除等级极差配置
    fn clear_config(entity_id: u64) -> Result<(), DispatchError>;
}

/// 空 LevelDiffPlanWriter 实现
impl LevelDiffPlanWriter for () {
    fn set_level_rates(_: u64, _: Vec<u16>, _: u8) -> Result<(), DispatchError> { Ok(()) }
    fn clear_config(_: u64) -> Result<(), DispatchError> { Ok(()) }
}

/// 团队业绩插件写入接口（由 commission-team 实现）
pub trait TeamPlanWriter<Balance> {
    /// 设置团队业绩阶梯配置
    ///
    /// tiers: Vec<(sales_threshold_u128, min_team_size, rate_bps)>
    /// threshold_mode: 0=Nex, 1=Usdt
    fn set_team_config(entity_id: u64, tiers: Vec<(u128, u32, u16)>, max_depth: u8, allow_stacking: bool, threshold_mode: u8) -> Result<(), DispatchError>;
    /// 清除团队业绩配置
    fn clear_config(entity_id: u64) -> Result<(), DispatchError>;
    /// 暂停团队业绩返佣（治理调用）
    fn governance_pause(entity_id: u64) -> Result<(), DispatchError> {
        let _ = entity_id;
        Ok(())
    }
    /// 恢复团队业绩返佣（治理调用）
    fn governance_resume(entity_id: u64) -> Result<(), DispatchError> {
        let _ = entity_id;
        Ok(())
    }
}

/// 空 TeamPlanWriter 实现
impl<Balance> TeamPlanWriter<Balance> for () {
    fn set_team_config(_: u64, _: Vec<(u128, u32, u16)>, _: u8, _: bool, _: u8) -> Result<(), DispatchError> { Ok(()) }
    fn clear_config(_: u64) -> Result<(), DispatchError> { Ok(()) }
}

/// 单线收益插件写入接口（由 commission-single-line 实现）
pub trait SingleLinePlanWriter {
    /// 设置单线收益配置
    ///
    /// rates: (upline_rate_bps, downline_rate_bps), max 1000 each
    /// base_levels: (base_upline, base_downline)
    /// max_levels: (max_upline, max_downline)
    /// level_increment_threshold: u128 encoded threshold
    #[allow(clippy::too_many_arguments)]
    fn set_single_line_config(
        entity_id: u64,
        upline_rate: u16,
        downline_rate: u16,
        base_upline_levels: u8,
        base_downline_levels: u8,
        level_increment_threshold: u128,
        max_upline_levels: u8,
        max_downline_levels: u8,
    ) -> Result<(), DispatchError>;
    /// 清除单线收益配置
    fn clear_config(entity_id: u64) -> Result<(), DispatchError>;
    /// 设置按等级自定义层数覆盖
    fn set_level_based_levels(
        entity_id: u64,
        level_id: u8,
        upline_levels: u8,
        downline_levels: u8,
    ) -> Result<(), DispatchError>;
    /// 清除指定等级的层数覆盖
    fn clear_level_overrides(entity_id: u64, level_id: u8) -> Result<(), DispatchError>;
}

/// 空 SingleLinePlanWriter 实现
impl SingleLinePlanWriter for () {
    fn set_single_line_config(_: u64, _: u16, _: u16, _: u8, _: u8, _: u128, _: u8, _: u8) -> Result<(), DispatchError> { Ok(()) }
    fn clear_config(_: u64) -> Result<(), DispatchError> { Ok(()) }
    fn set_level_based_levels(_: u64, _: u8, _: u8, _: u8) -> Result<(), DispatchError> { Ok(()) }
    fn clear_level_overrides(_: u64, _: u8) -> Result<(), DispatchError> { Ok(()) }
}

/// 沉淀池奖励插件写入接口（由 commission-pool-reward 实现）
///
/// v2: 周期性等额分配模型——level_ratios sum=10000，round_duration 为区块数
pub trait PoolRewardPlanWriter {
    /// 设置沉淀池奖励配置
    ///
    /// level_ratios: Vec<(level_id, ratio_bps)>, sum must equal 10000
    /// round_duration: 轮次持续区块数
    fn set_pool_reward_config(entity_id: u64, level_ratios: Vec<(u8, u16)>, round_duration: u32) -> Result<(), DispatchError>;
    /// 清除沉淀池奖励配置
    fn clear_config(entity_id: u64) -> Result<(), DispatchError>;
    /// 设置 Entity Token 池奖励是否启用（默认 no-op）
    fn set_token_pool_enabled(entity_id: u64, enabled: bool) -> Result<(), DispatchError> {
        let _ = (entity_id, enabled);
        Ok(())
    }
}

/// 空 PoolRewardPlanWriter 实现
impl PoolRewardPlanWriter for () {
    fn set_pool_reward_config(_: u64, _: Vec<(u8, u16)>, _: u32) -> Result<(), DispatchError> { Ok(()) }
    fn clear_config(_: u64) -> Result<(), DispatchError> { Ok(()) }
    fn set_token_pool_enabled(_: u64, _: bool) -> Result<(), DispatchError> { Ok(()) }
}

// ============================================================================
// PoolBalanceProvider — 沉淀池余额读写接口
// ============================================================================

/// 沉淀池余额读写接口（由 commission-core 实现，供 pool-reward 访问）
pub trait PoolBalanceProvider<Balance> {
    /// 查询沉淀池余额
    fn pool_balance(entity_id: u64) -> Balance;
    /// 从沉淀池扣减指定金额
    fn deduct_pool(entity_id: u64, amount: Balance) -> Result<(), DispatchError>;
}

/// 空 PoolBalanceProvider 实现
impl<Balance: Default> PoolBalanceProvider<Balance> for () {
    fn pool_balance(_: u64) -> Balance { Balance::default() }
    fn deduct_pool(_: u64, _: Balance) -> Result<(), DispatchError> { Ok(()) }
}

// ============================================================================
// Token 多资产扩展（方案 A: 全插件管线多资产化）
// ============================================================================

/// Token 佣金插件接口（与 CommissionPlugin 对称，Balance → TokenBalance）
///
/// 每个返佣插件额外实现此 trait，由 core 的 Token 调度管线调用。
/// 签名与 `CommissionPlugin` 完全一致，仅类型语义不同。
pub trait TokenCommissionPlugin<AccountId, TokenBalance> {
    fn calculate_token(
        entity_id: u64,
        buyer: &AccountId,
        order_amount: TokenBalance,
        remaining: TokenBalance,
        enabled_modes: CommissionModes,
        is_first_order: bool,
        buyer_order_count: u32,
    ) -> (Vec<CommissionOutput<AccountId, TokenBalance>>, TokenBalance);
}

/// 空 TokenCommissionPlugin 实现
impl<AccountId, TokenBalance> TokenCommissionPlugin<AccountId, TokenBalance> for () {
    fn calculate_token(
        _: u64, _: &AccountId, _: TokenBalance, remaining: TokenBalance,
        _: CommissionModes, _: bool, _: u32,
    ) -> (Vec<CommissionOutput<AccountId, TokenBalance>>, TokenBalance) {
        (Vec::new(), remaining)
    }
}

/// Token 佣金记录（与 CommissionRecord 对称，无 shop_id —— Token 佣金不区分 Shop）
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct TokenCommissionRecord<AccountId, TokenBalance, BlockNumber> {
    pub entity_id: u64,
    pub order_id: u64,
    pub buyer: AccountId,
    pub beneficiary: AccountId,
    pub amount: TokenBalance,
    pub commission_type: CommissionType,
    pub level: u8,
    pub status: CommissionStatus,
    pub created_at: BlockNumber,
}

/// Token 佣金统计（含复购分流统计）
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub struct MemberTokenCommissionStatsData<TokenBalance> {
    pub total_earned: TokenBalance,
    pub pending: TokenBalance,
    pub withdrawn: TokenBalance,
    pub repurchased: TokenBalance,
    pub order_count: u32,
}

// ============================================================================
// TokenTransferProvider — Token 转账接口（entity_id 级）
// ============================================================================

/// Token 转账接口（entity_id 级，简化 fungibles 接口）
///
/// 由 runtime 实现（委托 EntityTokenProvider 或 pallet-assets）。
/// commission-core 通过此 trait 执行 Token 提现和余额查询。
pub trait TokenTransferProvider<AccountId, TokenBalance> {
    /// 获取指定账户在某 Entity 下的可用 Token 余额
    fn token_balance_of(entity_id: u64, who: &AccountId) -> TokenBalance;

    /// Token 转账: from → to（entity_id 级）
    fn token_transfer(
        entity_id: u64,
        from: &AccountId,
        to: &AccountId,
        amount: TokenBalance,
    ) -> Result<(), DispatchError>;
}

/// 空 TokenTransferProvider 实现
impl<AccountId, TokenBalance: Default> TokenTransferProvider<AccountId, TokenBalance> for () {
    fn token_balance_of(_: u64, _: &AccountId) -> TokenBalance { TokenBalance::default() }
    fn token_transfer(_: u64, _: &AccountId, _: &AccountId, _: TokenBalance) -> Result<(), DispatchError> { Ok(()) }
}

// ============================================================================
// TokenPoolBalanceProvider — Token 沉淀池余额读写接口
// ============================================================================

/// Token 沉淀池余额读写接口（由 commission-core 实现，供 pool-reward 访问）
pub trait TokenPoolBalanceProvider<TokenBalance> {
    fn token_pool_balance(entity_id: u64) -> TokenBalance;
    fn deduct_token_pool(entity_id: u64, amount: TokenBalance) -> Result<(), DispatchError>;
}

/// 空 TokenPoolBalanceProvider 实现
impl<TokenBalance: Default> TokenPoolBalanceProvider<TokenBalance> for () {
    fn token_pool_balance(_: u64) -> TokenBalance { TokenBalance::default() }
    fn deduct_token_pool(_: u64, _: TokenBalance) -> Result<(), DispatchError> { Ok(()) }
}

// ============================================================================
// TokenCommissionProvider — Token 佣金服务接口（供 transaction 模块调用）
// ============================================================================

/// Token 佣金服务接口（全插件 Token 管线对外入口）
pub trait TokenCommissionProvider<AccountId, TokenBalance> {
    fn process_token_commission(
        entity_id: u64,
        shop_id: u64,
        order_id: u64,
        buyer: &AccountId,
        token_order_amount: TokenBalance,
        token_available_pool: TokenBalance,
        token_platform_fee: TokenBalance,
    ) -> Result<(), DispatchError>;

    fn cancel_token_commission(order_id: u64) -> Result<(), DispatchError>;

    fn pending_token_commission(entity_id: u64, account: &AccountId) -> TokenBalance;

    /// 获取 Entity 级 Token 平台费率（bps，0 = 不收费）
    fn token_platform_fee_rate(entity_id: u64) -> u16;
}

// ============================================================================
// ParticipationGuard — Entity 参与权守卫（KYC / 合规检查）
// ============================================================================

/// Entity 参与权守卫（KYC / 合规检查接口）
///
/// 在 `withdraw_commission`、`claim_pool_reward`、`do_consume_shopping_balance` 中调用，
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

/// 空 TokenCommissionProvider 实现
pub struct NullTokenCommissionProvider;

impl<AccountId, TokenBalance: Default> TokenCommissionProvider<AccountId, TokenBalance> for NullTokenCommissionProvider {
    fn process_token_commission(_: u64, _: u64, _: u64, _: &AccountId, _: TokenBalance, _: TokenBalance, _: TokenBalance) -> Result<(), DispatchError> { Ok(()) }
    fn cancel_token_commission(_: u64) -> Result<(), DispatchError> { Ok(()) }
    fn pending_token_commission(_: u64, _: &AccountId) -> TokenBalance { TokenBalance::default() }
    fn token_platform_fee_rate(_: u64) -> u16 { 0 }
}

// ============================================================================
// QueryProvider Traits — 子模块查询接口（供 Runtime API 聚合层使用）
// ============================================================================

/// 多级分销查询接口
pub trait MultiLevelQueryProvider<AccountId> {
    /// 激活进度（每层级的当前值 vs 要求值）
    fn activation_progress(entity_id: u64, account: &AccountId) -> Vec<MultiLevelActivationInfo>;
    /// 是否暂停
    fn is_paused(entity_id: u64) -> bool;
    /// 会员多级佣金统计
    fn member_stats(entity_id: u64, account: &AccountId) -> Option<MultiLevelMemberStats>;
}

/// 激活进度（Runtime API 可编解码版本，不依赖 Config）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct MultiLevelActivationInfo {
    pub level: u8,
    pub activated: bool,
    pub directs_current: u32,
    pub directs_required: u32,
    pub team_current: u32,
    pub team_required: u32,
    pub spent_current: u128,
    pub spent_required: u128,
}

/// 多级佣金会员统计
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, Default)]
pub struct MultiLevelMemberStats {
    pub total_earned: u128,
    pub total_orders: u32,
    pub last_commission_block: u32,
}

/// 空 MultiLevelQueryProvider 实现
impl<AccountId> MultiLevelQueryProvider<AccountId> for () {
    fn activation_progress(_: u64, _: &AccountId) -> Vec<MultiLevelActivationInfo> { Vec::new() }
    fn is_paused(_: u64) -> bool { false }
    fn member_stats(_: u64, _: &AccountId) -> Option<MultiLevelMemberStats> { None }
}

/// 团队业绩查询接口
pub trait TeamQueryProvider<AccountId, Balance> {
    /// 查询会员匹配的阶梯档位
    /// 返回 (tier_index, rate_bps, next_threshold, next_min_team_size)
    fn matched_tier(entity_id: u64, account: &AccountId) -> Option<TeamTierInfo<Balance>>;
    /// 查询团队业绩模块状态 (config_exists, is_enabled)
    fn status(entity_id: u64) -> (bool, bool);
}

/// 团队阶梯快照
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct TeamTierInfo<Balance> {
    pub tier_index: u8,
    pub rate: u16,
    pub next_threshold: Option<Balance>,
    pub next_min_team_size: Option<u32>,
}

/// 空 TeamQueryProvider 实现
impl<AccountId, Balance> TeamQueryProvider<AccountId, Balance> for () {
    fn matched_tier(_: u64, _: &AccountId) -> Option<TeamTierInfo<Balance>> { None }
    fn status(_: u64) -> (bool, bool) { (false, false) }
}

/// 单线收益查询接口
pub trait SingleLineQueryProvider<AccountId> {
    /// 全局排位
    fn position(entity_id: u64, account: &AccountId) -> Option<u32>;
    /// 有效搜索层数 (upline_levels, downline_levels)
    fn effective_levels(entity_id: u64, account: &AccountId) -> Option<(u8, u8)>;
    /// 单线是否启用
    fn is_enabled(entity_id: u64) -> bool;
    /// 排队总长度
    fn queue_length(entity_id: u64) -> u32;
}

/// 空 SingleLineQueryProvider 实现
impl<AccountId> SingleLineQueryProvider<AccountId> for () {
    fn position(_: u64, _: &AccountId) -> Option<u32> { None }
    fn effective_levels(_: u64, _: &AccountId) -> Option<(u8, u8)> { None }
    fn is_enabled(_: u64) -> bool { false }
    fn queue_length(_: u64) -> u32 { 0 }
}

/// 沉淀池奖励查询接口
pub trait PoolRewardQueryProvider<AccountId, Balance, TokenBalance> {
    /// 可领取金额 (nex, token)
    fn claimable(entity_id: u64, account: &AccountId) -> (Balance, TokenBalance);
    /// 是否暂停
    fn is_paused(entity_id: u64) -> bool;
    /// 当前轮次 ID
    fn current_round_id(entity_id: u64) -> u64;
}

/// 空 PoolRewardQueryProvider 实现
impl<AccountId, Balance: Default, TokenBalance: Default> PoolRewardQueryProvider<AccountId, Balance, TokenBalance> for () {
    fn claimable(_: u64, _: &AccountId) -> (Balance, TokenBalance) { (Balance::default(), TokenBalance::default()) }
    fn is_paused(_: u64) -> bool { false }
    fn current_round_id(_: u64) -> u64 { 0 }
}

/// 推荐链返佣查询接口
pub trait ReferralQueryProvider<AccountId, Balance> {
    /// 推荐人累计获佣
    fn referrer_total_earned(entity_id: u64, account: &AccountId) -> Balance;
    /// 返佣上限配置 (max_per_order, max_total_earned)；None 表示未配置
    fn cap_config(entity_id: u64) -> Option<(Balance, Balance)>;
}

/// 空 ReferralQueryProvider 实现
impl<AccountId, Balance: Default> ReferralQueryProvider<AccountId, Balance> for () {
    fn referrer_total_earned(_: u64, _: &AccountId) -> Balance { Balance::default() }
    fn cap_config(_: u64) -> Option<(Balance, Balance)> { None }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- CommissionModes ----

    #[test]
    fn modes_default_is_none() {
        let m = CommissionModes::default();
        assert_eq!(m.0, CommissionModes::NONE);
        assert!(m.is_valid());
    }

    #[test]
    fn modes_single_flag() {
        let m = CommissionModes(CommissionModes::DIRECT_REWARD);
        assert!(m.contains(CommissionModes::DIRECT_REWARD));
        assert!(!m.contains(CommissionModes::MULTI_LEVEL));
        assert!(m.is_valid());
    }

    #[test]
    fn modes_combined_flags() {
        let m = CommissionModes(
            CommissionModes::DIRECT_REWARD
                | CommissionModes::POOL_REWARD
                | CommissionModes::CREATOR_REWARD,
        );
        assert!(m.contains(CommissionModes::DIRECT_REWARD));
        assert!(m.contains(CommissionModes::POOL_REWARD));
        assert!(m.contains(CommissionModes::CREATOR_REWARD));
        assert!(!m.contains(CommissionModes::TEAM_PERFORMANCE));
        assert!(m.is_valid());
    }

    #[test]
    fn modes_all_valid_bits() {
        let m = CommissionModes(CommissionModes::ALL_VALID);
        assert!(m.is_valid());
        // 11 个模式位全部包含
        assert!(m.contains(CommissionModes::DIRECT_REWARD));
        assert!(m.contains(CommissionModes::MULTI_LEVEL));
        assert!(m.contains(CommissionModes::TEAM_PERFORMANCE));
        assert!(m.contains(CommissionModes::LEVEL_DIFF));
        assert!(m.contains(CommissionModes::FIXED_AMOUNT));
        assert!(m.contains(CommissionModes::FIRST_ORDER));
        assert!(m.contains(CommissionModes::REPEAT_PURCHASE));
        assert!(m.contains(CommissionModes::SINGLE_LINE_UPLINE));
        assert!(m.contains(CommissionModes::SINGLE_LINE_DOWNLINE));
        assert!(m.contains(CommissionModes::POOL_REWARD));
        assert!(m.contains(CommissionModes::CREATOR_REWARD));
    }

    #[test]
    fn modes_invalid_high_bit() {
        // 设置一个未定义的高位
        let m = CommissionModes(0b1000_0000_0000);
        assert!(!m.is_valid());
    }

    #[test]
    fn modes_mixed_valid_and_invalid() {
        let m = CommissionModes(CommissionModes::DIRECT_REWARD | 0b1000_0000_0000);
        assert!(!m.is_valid());
    }

    #[test]
    fn modes_insert_and_remove() {
        let mut m = CommissionModes::default();
        assert!(!m.contains(CommissionModes::MULTI_LEVEL));

        m.insert(CommissionModes::MULTI_LEVEL);
        assert!(m.contains(CommissionModes::MULTI_LEVEL));

        m.insert(CommissionModes::FIRST_ORDER);
        assert!(m.contains(CommissionModes::FIRST_ORDER));
        assert!(m.contains(CommissionModes::MULTI_LEVEL));

        m.remove(CommissionModes::MULTI_LEVEL);
        assert!(!m.contains(CommissionModes::MULTI_LEVEL));
        assert!(m.contains(CommissionModes::FIRST_ORDER));
        assert!(m.is_valid());
    }

    // ---- WithdrawalTierConfig ----

    #[test]
    fn withdrawal_tier_default_is_valid() {
        let tier = WithdrawalTierConfig::default();
        assert_eq!(tier.withdrawal_rate, 10000);
        assert_eq!(tier.repurchase_rate, 0);
        assert!(tier.is_valid());
    }

    #[test]
    fn withdrawal_tier_valid_split() {
        let tier = WithdrawalTierConfig {
            withdrawal_rate: 7000,
            repurchase_rate: 3000,
        };
        assert!(tier.is_valid());
    }

    #[test]
    fn withdrawal_tier_invalid_sum() {
        let tier = WithdrawalTierConfig {
            withdrawal_rate: 5000,
            repurchase_rate: 4000,
        };
        assert!(!tier.is_valid());
    }

    #[test]
    fn withdrawal_tier_overflow_saturates() {
        // u16::MAX + u16::MAX 会 saturate 到 u16::MAX，不等于 10000
        let tier = WithdrawalTierConfig {
            withdrawal_rate: u16::MAX,
            repurchase_rate: u16::MAX,
        };
        assert!(!tier.is_valid());
    }

    // ---- CommissionStatus default ----

    #[test]
    fn commission_status_default_is_pending() {
        assert_eq!(CommissionStatus::default(), CommissionStatus::Pending);
    }

    // ---- WithdrawalMode default ----

    #[test]
    fn withdrawal_mode_default_is_full() {
        assert_eq!(WithdrawalMode::default(), WithdrawalMode::FullWithdrawal);
    }

    // ---- 空实现 ----

    #[test]
    fn null_commission_plugin_returns_empty() {
        let (outputs, remaining) =
            <() as CommissionPlugin<u64, u128>>::calculate(1, &42, 1000, 500, CommissionModes::default(), false, 0);
        assert!(outputs.is_empty());
        assert_eq!(remaining, 500);
    }

    #[test]
    fn null_token_commission_plugin_returns_empty() {
        let (outputs, remaining) =
            <() as TokenCommissionPlugin<u64, u128>>::calculate_token(1, &42, 1000, 500, CommissionModes::default(), false, 0);
        assert!(outputs.is_empty());
        assert_eq!(remaining, 500);
    }

    #[test]
    fn null_commission_provider_noop() {
        type P = NullCommissionProvider;
        assert!(<P as CommissionProvider<u64, u128>>::process_commission(1, 1, 1, &42, 100, 50, 10).is_ok());
        assert!(<P as CommissionProvider<u64, u128>>::cancel_commission(1).is_ok());
        assert_eq!(<P as CommissionProvider<u64, u128>>::pending_commission(1, &42), 0u128);
        assert_eq!(<P as CommissionProvider<u64, u128>>::shopping_balance(1, &42), 0u128);
        assert!(<P as CommissionProvider<u64, u128>>::settle_order_commission(1).is_ok());
    }

    #[test]
    fn null_entity_referrer_provider() {
        assert_eq!(<() as EntityReferrerProvider<u64>>::entity_referrer(1), None);
    }

    #[test]
    fn null_participation_guard_allows_all() {
        assert!(<() as ParticipationGuard<u64>>::can_participate(1, &42));
    }

    #[test]
    fn null_pool_balance_provider() {
        assert_eq!(<() as PoolBalanceProvider<u128>>::pool_balance(1), 0);
        assert!(<() as PoolBalanceProvider<u128>>::deduct_pool(1, 100).is_ok());
    }

    #[test]
    fn null_token_pool_balance_provider() {
        assert_eq!(<() as TokenPoolBalanceProvider<u128>>::token_pool_balance(1), 0);
        assert!(<() as TokenPoolBalanceProvider<u128>>::deduct_token_pool(1, 100).is_ok());
    }

    #[test]
    fn null_token_transfer_provider() {
        assert_eq!(<() as TokenTransferProvider<u64, u128>>::token_balance_of(1, &42), 0);
        assert!(<() as TokenTransferProvider<u64, u128>>::token_transfer(1, &42, &43, 100).is_ok());
    }

    #[test]
    fn null_token_commission_provider() {
        assert!(<NullTokenCommissionProvider as TokenCommissionProvider<u64, u128>>::process_token_commission(1, 1, 1, &42, 100, 50, 10).is_ok());
        assert!(<NullTokenCommissionProvider as TokenCommissionProvider<u64, u128>>::cancel_token_commission(1).is_ok());
        assert_eq!(<NullTokenCommissionProvider as TokenCommissionProvider<u64, u128>>::pending_token_commission(1, &42), 0u128);
        assert_eq!(<NullTokenCommissionProvider as TokenCommissionProvider<u64, u128>>::token_platform_fee_rate(1), 0);
    }
}
