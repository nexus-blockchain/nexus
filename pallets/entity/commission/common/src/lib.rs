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
}

/// 返佣状态
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum CommissionStatus {
    #[default]
    Pending,
    Distributed,
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
pub struct MemberCommissionStatsData<Balance: Default> {
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

/// 提现模式
///
/// 决定佣金提现时复购比率的确定方式。
/// 无论选择哪种模式，Governance 设定的全局最低复购比率始终生效。
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub enum WithdrawalMode {
    /// 全额提现：不强制复购（Governance 底线仍生效）
    FullWithdrawal,
    /// 固定比率：所有会员统一复购比率
    FixedRate { repurchase_rate: u16 },
    /// 按等级自动决定：通过 default_tier + level_overrides 查表
    LevelBased,
    /// 会员自选：会员提现时指定复购比率，不低于 min_repurchase_rate
    MemberChoice { min_repurchase_rate: u16 },
}

impl Default for WithdrawalMode {
    fn default() -> Self {
        WithdrawalMode::FullWithdrawal
    }
}

// ============================================================================
// 插件输出
// ============================================================================

/// 单条返佣输出（插件计算结果）
#[derive(Clone, RuntimeDebug)]
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
}

// ============================================================================
// MemberProvider Trait（由 member 模块实现）
// ============================================================================

/// 会员服务接口（供返佣插件查询推荐人、等级等，统一使用 entity_id）
pub trait MemberProvider<AccountId> {
    fn is_member(entity_id: u64, account: &AccountId) -> bool;
    fn get_referrer(entity_id: u64, account: &AccountId) -> Option<AccountId>;
    fn get_member_stats(entity_id: u64, account: &AccountId) -> (u32, u32, u128);
    fn uses_custom_levels(entity_id: u64) -> bool;
    fn custom_level_id(entity_id: u64, account: &AccountId) -> u8;
    /// 获取自定义等级的返佣加成（基点），用于 level-diff 无独立配置时的回退
    fn get_level_commission_bonus(entity_id: u64, level_id: u8) -> u16;
    fn auto_register(entity_id: u64, account: &AccountId, referrer: Option<AccountId>) -> Result<(), DispatchError>;
    /// 自动注册会员（entity_id 直达，qualified 控制是否为有效直推）
    fn auto_register_qualified(entity_id: u64, account: &AccountId, referrer: Option<AccountId>, qualified: bool) -> Result<(), DispatchError> {
        let _ = (entity_id, account, referrer, qualified);
        Ok(())
    }
    /// 查询会员是否已激活
    fn is_activated(entity_id: u64, account: &AccountId) -> bool {
        let _ = (entity_id, account);
        true // 默认已激活（无 member 系统时不阻断）
    }

    fn set_custom_levels_enabled(entity_id: u64, enabled: bool) -> Result<(), DispatchError>;
    fn set_upgrade_mode(entity_id: u64, mode: u8) -> Result<(), DispatchError>;
    fn add_custom_level(entity_id: u64, level_id: u8, name: &[u8], threshold: u128, discount_rate: u16, commission_bonus: u16) -> Result<(), DispatchError>;
    fn update_custom_level(entity_id: u64, level_id: u8, name: Option<&[u8]>, threshold: Option<u128>, discount_rate: Option<u16>, commission_bonus: Option<u16>) -> Result<(), DispatchError>;
    fn remove_custom_level(entity_id: u64, level_id: u8) -> Result<(), DispatchError>;
    fn custom_level_count(entity_id: u64) -> u8;
    /// 查询指定等级的会员数量（用于沉淀池奖励 v2 等额分配）
    fn member_count_by_level(entity_id: u64, level_id: u8) -> u32 {
        let _ = (entity_id, level_id);
        0
    }
    /// 查询会员 USDT 累计消费（独立存储 MemberSpentUsdt，精度 10^6）
    fn get_member_spent_usdt(entity_id: u64, account: &AccountId) -> u64 {
        let _ = (entity_id, account);
        0
    }
}

/// 空 MemberProvider 实现
pub struct NullMemberProvider;

impl<AccountId> MemberProvider<AccountId> for NullMemberProvider {
    fn is_member(_: u64, _: &AccountId) -> bool { false }
    fn get_referrer(_: u64, _: &AccountId) -> Option<AccountId> { None }
    fn get_member_stats(_: u64, _: &AccountId) -> (u32, u32, u128) { (0, 0, 0) }
    fn uses_custom_levels(_: u64) -> bool { false }
    fn custom_level_id(_: u64, _: &AccountId) -> u8 { 0 }
    fn get_level_commission_bonus(_: u64, _: u8) -> u16 { 0 }
    fn auto_register(_: u64, _: &AccountId, _: Option<AccountId>) -> Result<(), DispatchError> { Ok(()) }
    fn set_custom_levels_enabled(_: u64, _: bool) -> Result<(), DispatchError> { Ok(()) }
    fn set_upgrade_mode(_: u64, _: u8) -> Result<(), DispatchError> { Ok(()) }
    fn add_custom_level(_: u64, _: u8, _: &[u8], _: u128, _: u16, _: u16) -> Result<(), DispatchError> { Ok(()) }
    fn update_custom_level(_: u64, _: u8, _: Option<&[u8]>, _: Option<u128>, _: Option<u16>, _: Option<u16>) -> Result<(), DispatchError> { Ok(()) }
    fn remove_custom_level(_: u64, _: u8) -> Result<(), DispatchError> { Ok(()) }
    fn custom_level_count(_: u64) -> u8 { 0 }
    fn member_count_by_level(_: u64, _: u8) -> u32 { 0 }
    fn get_member_spent_usdt(_: u64, _: &AccountId) -> u64 { 0 }
}

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
// CommissionPlan — 一键初始化佣金方案
// ============================================================================

/// 佣金方案模板
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub enum CommissionPlan {
    /// 无佣金（关闭所有返佣）
    None,
    /// 直推返佣（推荐人获得订单金额的 rate 基点）
    DirectOnly { rate: u16 },
    /// 多级分销（levels 级，每级 base_rate 基点，逐级递减 20%）
    MultiLevel { levels: u8, base_rate: u16 },
    /// 等级极差（按自定义等级配置比例，单位基点，最多 10 级）
    LevelDiff {
        level_rates: BoundedVec<u16, ConstU32<10>>,
    },
    /// 自定义（仅启用佣金开关，参数后续手动配置）
    Custom,
}

// ============================================================================
// PlanWriter Traits — 插件写入接口
// ============================================================================

/// 推荐链插件写入接口（由 commission-referral 实现）
pub trait ReferralPlanWriter<Balance> {
    /// 设置直推奖励比例
    fn set_direct_rate(entity_id: u64, rate: u16) -> Result<(), DispatchError>;
    /// 设置多级分销（每级比例列表 + 上限比例）
    fn set_multi_level(entity_id: u64, level_rates: Vec<u16>, max_total_rate: u16) -> Result<(), DispatchError>;
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
    fn set_multi_level(_: u64, _: Vec<u16>, _: u16) -> Result<(), DispatchError> { Ok(()) }
    fn set_fixed_amount(_: u64, _: Balance) -> Result<(), DispatchError> { Ok(()) }
    fn set_first_order(_: u64, _: Balance, _: u16, _: bool) -> Result<(), DispatchError> { Ok(()) }
    fn set_repeat_purchase(_: u64, _: u16, _: u32) -> Result<(), DispatchError> { Ok(()) }
    fn clear_config(_: u64) -> Result<(), DispatchError> { Ok(()) }
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
}

/// 空 TeamPlanWriter 实现
impl<Balance> TeamPlanWriter<Balance> for () {
    fn set_team_config(_: u64, _: Vec<(u128, u32, u16)>, _: u8, _: bool, _: u8) -> Result<(), DispatchError> { Ok(()) }
    fn clear_config(_: u64) -> Result<(), DispatchError> { Ok(()) }
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
pub struct MemberTokenCommissionStatsData<TokenBalance: Default> {
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
        token_platform_fee: TokenBalance,
    ) -> Result<(), DispatchError>;

    fn cancel_token_commission(order_id: u64) -> Result<(), DispatchError>;

    fn pending_token_commission(entity_id: u64, account: &AccountId) -> TokenBalance;

    /// 获取 Entity 级 Token 平台费率（bps，0 = 不收费）
    fn token_platform_fee_rate(entity_id: u64) -> u16;
}

/// 空 TokenCommissionProvider 实现
pub struct NullTokenCommissionProvider;

impl<AccountId, TokenBalance: Default> TokenCommissionProvider<AccountId, TokenBalance> for NullTokenCommissionProvider {
    fn process_token_commission(_: u64, _: u64, _: u64, _: &AccountId, _: TokenBalance, _: TokenBalance) -> Result<(), DispatchError> { Ok(()) }
    fn cancel_token_commission(_: u64) -> Result<(), DispatchError> { Ok(()) }
    fn pending_token_commission(_: u64, _: &AccountId) -> TokenBalance { TokenBalance::default() }
    fn token_platform_fee_rate(_: u64) -> u16 { 0 }
}
