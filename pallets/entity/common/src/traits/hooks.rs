//! Event notification traits and lifecycle callbacks
//!
//! Hook traits for cross-module event propagation (Entity status change, Order status change,
//! KYC status change, Member removal, Disclosure violation, Points cleanup).

extern crate alloc;

use super::super::types::*;

// ============================================================================
// P5: Entity 状态变更级联通知
// ============================================================================

/// Entity 状态变更通知接口
///
/// 供下游模块（Shop、Token、Market、Ads 等）在 Entity 状态变更时做出响应。
/// 当 Entity 被暂停/封禁/关闭/恢复时，级联通知所有关联模块执行清理逻辑。
pub trait OnEntityStatusChange {
    /// Entity 被暂停时触发
    fn on_entity_suspended(entity_id: u64);
    /// Entity 被封禁时触发
    fn on_entity_banned(entity_id: u64);
    /// Entity 恢复运营时触发
    fn on_entity_resumed(entity_id: u64);
    /// Entity 被关闭时触发
    fn on_entity_closed(entity_id: u64);
}

/// 空 Entity 状态变更通知（测试用或无下游模块时）
impl OnEntityStatusChange for () {
    fn on_entity_suspended(_entity_id: u64) {}
    fn on_entity_banned(_entity_id: u64) {}
    fn on_entity_resumed(_entity_id: u64) {}
    fn on_entity_closed(_entity_id: u64) {}
}

// ============================================================================
// P2: 订单状态变更通知
// ============================================================================

/// 订单状态变更通知接口
///
/// 供下游模块（佣金、会员、物流、保险等）在订单状态变更时做出响应。
/// 实现开闭原则：新增下游模块无需修改 order pallet。
pub trait OnOrderStatusChange<AccountId, Balance> {
    /// 订单状态变更时触发
    fn on_order_status_changed(
        order_id: u64,
        entity_id: u64,
        shop_id: u64,
        buyer: &AccountId,
        amount: Balance,
        old_status: &OrderStatus,
        new_status: &OrderStatus,
    );
}

impl<AccountId, Balance> OnOrderStatusChange<AccountId, Balance> for () {
    fn on_order_status_changed(
        _order_id: u64,
        _entity_id: u64,
        _shop_id: u64,
        _buyer: &AccountId,
        _amount: Balance,
        _old_status: &OrderStatus,
        _new_status: &OrderStatus,
    ) {}
}

// ============================================================================
// KYC 状态变更通知
// ============================================================================

/// KYC 状态变更通知接口
///
/// 供下游模块（订单、交易等）在 KYC 状态变更时做出响应。
/// old_status / new_status 使用 u8 编码避免跨 pallet 类型依赖:
/// 0=NotSubmitted, 1=Pending, 2=Approved, 3=Rejected, 4=Expired, 5=Revoked
pub trait OnKycStatusChange<AccountId> {
    fn on_kyc_status_changed(entity_id: u64, account: &AccountId, old_status: u8, new_status: u8, level: u8);
}

impl<AccountId> OnKycStatusChange<AccountId> for () {
    fn on_kyc_status_changed(_entity_id: u64, _account: &AccountId, _old_status: u8, _new_status: u8, _level: u8) {}
}

// ============================================================================
// 披露违规回调
// ============================================================================

/// 披露违规回调 — 供下游模块（token/market）响应披露违规事件
pub trait OnDisclosureViolation {
    /// 违规达到阈值或处罚升级时调用
    fn on_violation_threshold_reached(entity_id: u64, violation_count: u32, penalty_level: u8);
}

impl OnDisclosureViolation for () {
    fn on_violation_threshold_reached(_: u64, _: u32, _: u8) {}
}

// ============================================================================
// 会员生命周期回调
// ============================================================================

/// 会员移除回调（通知依赖方清理关联存储）
///
/// 由 `pallet-entity-member` 在 `do_remove_member` 末尾调用。
/// 各佣金插件实现此 trait 以清理各自的 per-user 存储（如 claim 记录等）。
///
/// 支持通过元组组合多个实现：`type OnMemberRemoved = (PluginA, PluginB);`
pub trait OnMemberRemoved<AccountId> {
    fn on_member_removed(entity_id: u64, account: &AccountId);
}

impl<AccountId> OnMemberRemoved<AccountId> for () {
    fn on_member_removed(_: u64, _: &AccountId) {}
}

macro_rules! impl_on_member_removed_tuple {
    ( $first:ident $(, $rest:ident )* ) => {
        impl<AccountId, $first: OnMemberRemoved<AccountId>, $( $rest: OnMemberRemoved<AccountId> ),*>
            OnMemberRemoved<AccountId> for ($first, $( $rest ),*)
        {
            fn on_member_removed(entity_id: u64, account: &AccountId) {
                $first::on_member_removed(entity_id, account);
                $( $rest::on_member_removed(entity_id, account); )*
            }
        }
        impl_on_member_removed_tuple!( $( $rest ),* );
    };
    () => {};
}

impl_on_member_removed_tuple!(A, B, C, D, E, F, G, H);

// ============================================================================
// 积分清理接口
// ============================================================================

/// 积分清理接口（Shop 关闭时由 loyalty 模块执行）
pub trait PointsCleanup {
    /// 清理指定 Shop 的全部积分数据
    fn cleanup_shop_points(shop_id: u64);
}

impl PointsCleanup for () {
    fn cleanup_shop_points(_shop_id: u64) {}
}

// ============================================================================
// Phase 5.3: 订单完成/取消 Hook — 副作用链式调用
// ============================================================================

/// 订单完成 Hook — 所有"完成后副作用"由 Hook 链依次执行
///
/// 支持元组组合：`type OnOrderCompleted = (MemberHook, ShopHook, CommissionHook, LoyaltyHook);`
/// 每个 Hook 实现是 best-effort，单个 Hook 失败不影响后续执行。
pub trait OnOrderCompleted<AccountId, Balance> {
    fn on_completed(info: &OrderCompletionInfo<AccountId, Balance>);
}

impl<AccountId, Balance> OnOrderCompleted<AccountId, Balance> for () {
    fn on_completed(_info: &OrderCompletionInfo<AccountId, Balance>) {}
}

/// 订单取消 Hook — 取消/退款后的清理工作
///
/// 支持元组组合：`type OnOrderCancelled = (CommissionCancelHook, ...);`
pub trait OnOrderCancelled {
    fn on_cancelled(info: &OrderCancellationInfo);
}

impl OnOrderCancelled for () {
    fn on_cancelled(_info: &OrderCancellationInfo) {}
}

// Tuple impls for OnOrderCompleted (up to 6 elements)
macro_rules! impl_on_order_completed_tuple {
    ( $first:ident $(, $rest:ident )* ) => {
        impl<AccountId, Balance, $first: OnOrderCompleted<AccountId, Balance>, $( $rest: OnOrderCompleted<AccountId, Balance> ),*>
            OnOrderCompleted<AccountId, Balance> for ($first, $( $rest ),*)
        {
            fn on_completed(info: &OrderCompletionInfo<AccountId, Balance>) {
                $first::on_completed(info);
                $( $rest::on_completed(info); )*
            }
        }
        impl_on_order_completed_tuple!( $( $rest ),* );
    };
    () => {};
}

impl_on_order_completed_tuple!(A, B, C, D, E, F);

// Tuple impls for OnOrderCancelled (up to 6 elements)
macro_rules! impl_on_order_cancelled_tuple {
    ( $first:ident $(, $rest:ident )* ) => {
        impl<$first: OnOrderCancelled, $( $rest: OnOrderCancelled ),*>
            OnOrderCancelled for ($first, $( $rest ),*)
        {
            fn on_cancelled(info: &OrderCancellationInfo) {
                $first::on_cancelled(info);
                $( $rest::on_cancelled(info); )*
            }
        }
        impl_on_order_cancelled_tuple!( $( $rest ),* );
    };
    () => {};
}

impl_on_order_cancelled_tuple!(A, B, C, D, E, F);
