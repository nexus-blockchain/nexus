//! Domain-specific governance execution port traits
//!
//! These ports allow the governance pallet to execute proposals by delegating
//! to downstream pallets without direct dependency on their concrete types.
//!
//! MarketGovernancePort, CommissionGovernancePort, SingleLineGovernancePort,
//! KycGovernancePort, ShopGovernancePort, TokenGovernancePort.

use sp_runtime::DispatchError;

/// 市场治理执行接口
///
/// 供 governance 模块在提案通过后调用，替代链下 off-chain 执行。
pub trait MarketGovernancePort<Balance> {
    /// 变更市场配置
    fn governance_set_market_config(entity_id: u64, min_order_amount: Balance, order_ttl: u32) -> Result<(), DispatchError>;
    /// 暂停市场交易
    fn governance_pause_market(entity_id: u64) -> Result<(), DispatchError>;
    /// 恢复市场交易
    fn governance_resume_market(entity_id: u64) -> Result<(), DispatchError>;
    /// 永久关闭市场（不可逆）
    fn governance_close_market(entity_id: u64) -> Result<(), DispatchError>;
    /// 变更价格保护参数
    fn governance_set_price_protection(entity_id: u64, max_price_deviation: u16, max_slippage: u16, circuit_breaker_threshold: u16, min_trades_for_twap: u32) -> Result<(), DispatchError>;
    /// 变更市场 KYC 要求
    fn governance_set_market_kyc(entity_id: u64, min_kyc_level: u8) -> Result<(), DispatchError>;
    /// 解除熔断
    fn governance_lift_circuit_breaker(entity_id: u64) -> Result<(), DispatchError>;
}

/// 空 MarketGovernancePort 实现（fail-closed: 未接线时拒绝执行）
impl<Balance> MarketGovernancePort<Balance> for () {
    fn governance_set_market_config(_: u64, _: Balance, _: u32) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_pause_market(_: u64) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_resume_market(_: u64) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_close_market(_: u64) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_set_price_protection(_: u64, _: u16, _: u16, _: u16, _: u32) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_set_market_kyc(_: u64, _: u8) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_lift_circuit_breaker(_: u64) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
}

/// 返佣治理执行接口（补充 CommissionProvider 中缺失的治理方法）
///
/// 涵盖提现冷却期、推荐人门槛、佣金上限等 CommissionProvider 不含的治理配置。
pub trait CommissionGovernancePort<Balance> {
    /// 设置提现冷却期
    fn governance_set_withdrawal_cooldown(entity_id: u64, nex_cooldown: u32, token_cooldown: u32) -> Result<(), DispatchError>;
    /// 设置代币提现配置
    fn governance_set_token_withdrawal(entity_id: u64, enabled: bool) -> Result<(), DispatchError>;
    /// 暂停/恢复提现
    fn governance_set_withdrawal_pause(entity_id: u64, paused: bool) -> Result<(), DispatchError>;
    /// 设置推荐人资格门槛
    fn governance_set_referrer_guard(entity_id: u64, min_referrer_spent: Balance, min_referrer_orders: u32) -> Result<(), DispatchError>;
    /// 设置返佣上限
    fn governance_set_commission_cap(entity_id: u64, max_per_order: Balance, max_total_earned: Balance) -> Result<(), DispatchError>;
    /// 设置推荐有效期
    fn governance_set_referral_validity(entity_id: u64, validity_blocks: u32, valid_orders: u32) -> Result<(), DispatchError>;
    /// 暂停多级分销
    fn governance_pause_multi_level(entity_id: u64) -> Result<(), DispatchError>;
    /// 恢复多级分销
    fn governance_resume_multi_level(entity_id: u64) -> Result<(), DispatchError>;
    /// 暂停团队业绩返佣
    fn governance_pause_team_performance(entity_id: u64) -> Result<(), DispatchError>;
    /// 恢复团队业绩返佣
    fn governance_resume_team_performance(entity_id: u64) -> Result<(), DispatchError>;
}

/// 空 CommissionGovernancePort 实现（fail-closed: 未接线时拒绝执行）
impl<Balance> CommissionGovernancePort<Balance> for () {
    fn governance_set_withdrawal_cooldown(_: u64, _: u32, _: u32) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_set_token_withdrawal(_: u64, _: bool) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_set_withdrawal_pause(_: u64, _: bool) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_set_referrer_guard(_: u64, _: Balance, _: u32) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_set_commission_cap(_: u64, _: Balance, _: Balance) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_set_referral_validity(_: u64, _: u32, _: u32) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_pause_multi_level(_: u64) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_resume_multi_level(_: u64) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_pause_team_performance(_: u64) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_resume_team_performance(_: u64) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
}

/// 单线收益治理执行接口
pub trait SingleLineGovernancePort {
    /// 设置单线收益配置
    fn governance_set_single_line_config(entity_id: u64, upline_rate: u16, downline_rate: u16, base_upline_levels: u8, base_downline_levels: u8, max_upline_levels: u8, max_downline_levels: u8) -> Result<(), DispatchError>;
    /// 暂停单线收益
    fn governance_pause_single_line(entity_id: u64) -> Result<(), DispatchError>;
    /// 恢复单线收益
    fn governance_resume_single_line(entity_id: u64) -> Result<(), DispatchError>;
}

/// 空 SingleLineGovernancePort 实现（fail-closed: 未接线时拒绝执行）
impl SingleLineGovernancePort for () {
    fn governance_set_single_line_config(_: u64, _: u16, _: u16, _: u8, _: u8, _: u8, _: u8) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_pause_single_line(_: u64) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_resume_single_line(_: u64) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
}

/// KYC 治理执行接口
pub trait KycGovernancePort {
    /// 设置 KYC 等级要求
    fn governance_set_kyc_requirement(entity_id: u64, min_level: u8, mandatory: bool, grace_period: u32) -> Result<(), DispatchError>;
    /// 授权 KYC 提供者
    fn governance_authorize_kyc_provider(entity_id: u64, provider_id: u64) -> Result<(), DispatchError>;
    /// 取消 KYC 提供者授权
    fn governance_deauthorize_kyc_provider(entity_id: u64, provider_id: u64) -> Result<(), DispatchError>;
}

/// 空 KycGovernancePort 实现（fail-closed: 未接线时拒绝执行）
impl KycGovernancePort for () {
    fn governance_set_kyc_requirement(_: u64, _: u8, _: bool, _: u32) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_authorize_kyc_provider(_: u64, _: u64) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_deauthorize_kyc_provider(_: u64, _: u64) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
}

/// 店铺治理扩展执行接口（补充 ShopProvider 中缺失的治理方法）
pub trait ShopGovernancePort {
    /// 变更积分配置
    fn governance_set_points_config(entity_id: u64, reward_rate: u16, exchange_rate: u16, transferable: bool) -> Result<(), DispatchError>;
    /// 积分系统开关
    fn governance_toggle_points(entity_id: u64, enabled: bool) -> Result<(), DispatchError>;
    /// 变更店铺政策
    fn governance_set_shop_policies(entity_id: u64, policies_cid: &[u8]) -> Result<(), DispatchError>;
}

/// 空 ShopGovernancePort 实现（fail-closed: 未接线时拒绝执行）
impl ShopGovernancePort for () {
    fn governance_set_points_config(_: u64, _: u16, _: u16, _: bool) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_toggle_points(_: u64, _: bool) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
    fn governance_set_shop_policies(_: u64, _: &[u8]) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
}

/// 代币治理扩展执行接口（补充 EntityTokenProvider 中缺失的治理方法）
pub trait TokenGovernancePort<AccountId> {
    /// 代币黑名单管理（account_cid 链下解析，链上标记 add/remove）
    fn governance_manage_blacklist(entity_id: u64, account_cid: &[u8], add: bool) -> Result<(), DispatchError>;
}

/// 空 TokenGovernancePort 实现（fail-closed: 未接线时拒绝执行）
impl<AccountId> TokenGovernancePort<AccountId> for () {
    fn governance_manage_blacklist(_: u64, _: &[u8], _: bool) -> Result<(), DispatchError> { Err(DispatchError::Other("not implemented")) }
}
