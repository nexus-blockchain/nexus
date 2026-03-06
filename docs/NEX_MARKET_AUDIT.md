# pallet-nex-market 深度审计报告

**日期:** 2026-03-05 (R1), 2026-03-06 (R2)  
**范围:** `pallets/trading/nex-market/src/lib.rs`, `mock.rs`, `weights.rs`, `pallets/trading/common/src/*`  
**版本:** lib.rs 3926 行 (R2后), tests.rs 3281 行 (R2后), weights.rs 95 行

---

## 审计概要

| 严重度 | R1 数量 | R2 数量 | 已修复 |
|--------|---------|---------|--------|
| Critical | 1 | 0 | 1 ✅ |
| High | 2 | 1 | 3 ✅ |
| Medium | 2 | 2 | 4 ✅ |
| Low | 2 | 1 | 1 ✅ (R2) |

---

## Round 1 修复详情

### C1: `resolve_dispute` 静默丢弃国库转账失败 (Critical)

**位置:** `lib.rs` — `resolve_dispute` extrinsic  
**问题:** `let _ = T::Currency::transfer(...)` 静默忽略转账结果。当国库余额不足时，买家得到 0 NEX，但争议被标记为 `ResolvedForBuyer`。管理员认为买家已补偿，实际上没有。  
**修复:** 改用 `.map_err(|_| Error::<T>::InsufficientBalance)?` 传播错误。转账失败时争议解决操作回滚，争议保持 Open 状态。  
**测试:** `c1_resolve_dispute_fails_when_treasury_underfunded`

### H1: `update_order_price` 买单价格变更不重算保证金 (High)

**位置:** `lib.rs` — `update_order_price` extrinsic  
**问题:** 买单保证金在 `place_buy_order` 时根据原始价格计算并锁定。价格变更后保证金不调整：
- 价格上涨 → 保证金不足，保护力度低于预期
- 价格下跌 → 多余资金被锁定

**修复:** 价格变更时重新计算保证金。差额通过 `reserve`/`unreserve` 调整。卖单不受影响（卖家锁的是 NEX 数量，与价格无关）。  
**测试:** `h1_update_buy_order_price_adjusts_deposit_upward`, `h1_update_buy_order_price_adjusts_deposit_downward`, `h1_update_sell_order_price_no_deposit_change`

### H2: `rollback_order_filled_amount` 将已过期订单重新加入订单簿 (High)

**位置:** `lib.rs` — `rollback_order_filled_amount` 内部函数  
**问题:** 当交易超时回滚时，Filled 订单恢复为 Open/PartiallyFilled 并重新加入订单簿，但不检查订单是否已过期。已过期订单重新出现在订单簿中，虽然吃单方有过期检查不会成交，但会污染 `BestAsk`/`BestBid` 价格直到 GC 清理。  
**修复:** 回滚前检查 `now > order.expires_at`，过期订单标记为 `Expired` 状态，不加回订单簿。  
**测试:** `h2_rollback_skips_expired_order`

### M1: `reserve_sell_order`/`accept_buy_order` 无最低吃单量检查 (Medium)

**位置:** `lib.rs` — `reserve_sell_order`, `accept_buy_order`  
**问题:** 挂单有 `MinOrderNexAmount` 检查，但吃单只检查 `!is_zero()`。用户可提交极小吃单量（如 1 unit），每笔都需要 OCW 链上验证，浪费验证资源。  
**修复:** 吃单量也需满足 `MinOrderNexAmount`。当订单剩余可用量本身低于最低限额时放宽检查（允许清扫尾单）。  
**测试:** `m1_reserve_sell_order_rejects_micro_fill`, `m1_accept_buy_order_rejects_micro_fill`, `m1_reserve_sell_order_allows_tail_fill_below_minimum`

### M2: USDT 金额计算使用不安全 `as u64` 强转 (Medium)

**位置:** `lib.rs` — `place_buy_order`, `reserve_sell_order`, `accept_buy_order` 三处  
**问题:** `checked_div(...)? as u64` — u128 除法结果直接强转 u64。极端 `nex_amount` 值下结果可能超过 `u64::MAX`，导致静默截断，USDT 金额被严重低估。  
**修复:** 三处均改用 `u64::try_from(...).map_err(|_| Error::<T>::ArithmeticOverflow)?`，溢出时返回错误而非截断。

---

## Round 2 修复详情

### H1-R2: `rollback_order_filled_amount` 覆写 Cancelled 订单状态为 Open/PartiallyFilled (High)

**位置:** `lib.rs` — `rollback_order_filled_amount` 内部函数  
**问题:** 交易超时回滚时，`rollback_order_filled_amount` 无条件将 `filled_amount < nex_amount` 的订单状态设为 Open/PartiallyFilled。当订单已被取消（Cancelled）后，其活跃交易超时触发回滚，订单状态被覆写为 Open，但订单不在订单簿/用户索引中 — 产生幽灵订单。  
**场景:**
1. Alice 挂 100 NEX 卖单
2. Bob 部分吃 50 NEX → PartiallyFilled
3. Alice 取消订单 → Cancelled，未成交的 50 NEX unreserve
4. Bob 交易超时 → rollback 将 filled_amount 从 50 减为 0 → status 被错误设为 Open
5. 存储中出现 status=Open 但不在 SellOrders 索引中的幽灵记录

**修复:** 回滚前检查订单状态。Cancelled/Expired 订单仅回退 `filled_amount`，不改变状态，不重新入簿。  
**测试:** `h1r2_rollback_preserves_cancelled_order_status`, `h1r2_rollback_preserves_expired_order_status_non_filled`

### M1-R2: `process_full_payment` 手续费 `repatriate_reserved` 静默丢弃失败 (Medium)

**位置:** `lib.rs` — `process_full_payment` 内部函数  
**问题:** `let _ = T::Currency::repatriate_reserved(...)` 丢弃返回值。若手续费转账失败或部分转账：
- 手续费仍 reserved 在卖家账户（永久冻结）
- 买家只收到 `nex_amount - fee_amount`（减少了 fee 但无人收到）

**修复:** 处理 `repatriate_reserved` 返回值，使用实际转账金额 `actually_charged` 计算 `nex_to_buyer`。失败时 `actually_charged = 0`，买家收到完整 `nex_amount`。  
**测试:** `m1r2_fee_actually_charged_equals_nex_deducted`

### M2-R2: `rollback_order_filled_amount` 重新入簿后不刷新 BestAsk/BestBid (Medium)

**位置:** `lib.rs` — `rollback_order_filled_amount` 内部函数  
**问题:** 订单从 Filled 恢复并重新加入订单簿后，不调用 `update_best_price_on_new_order`。最优价格过时，直到下次 GC 或新订单操作才刷新。  
**修复:** 重新入簿成功后调用 `update_best_price_on_new_order(order.usdt_price, order.side)`。  
**测试:** `m2r2_rollback_refreshes_best_ask`

### L1-R2: `get_order_depth` 冗余 `expires_at` 检查 (Low)

**位置:** `lib.rs` — `get_order_depth` 公共查询函数  
**问题:** `if now > order.expires_at { continue; }` — 但上游 `get_sell_order_list()` 和 `get_buy_order_list()` 已过滤过期订单。属于死代码。  
**修复:** 移除冗余检查和未使用的 `now` 变量。  
**测试:** `l1r2_order_depth_excludes_expired_orders`

---

## 记录但未修复

### L1-R1: `process_underpaid` 不收取交易手续费

**位置:** `lib.rs` — `process_underpaid`  
**描述:** `process_full_payment` 收取 `TradingFeeBps` 交易手续费，但 `process_underpaid` 不收取。理论上买家可通过轻微欠付（如 99%）来规避手续费。但考虑到欠付买家已受到保证金没收惩罚，且收到的 NEX 按比例减少，额外收费可能构成双重惩罚。**建议:** 设计决策，保持现状或在 `process_underpaid` 中对交付的 NEX 部分收取费用。

### L2-R1: `update_order_price` 允许修改已过期但未被 GC 清理的订单

**位置:** `lib.rs` — `update_order_price`  
**描述:** 仅检查 `OrderStatus::Open || PartiallyFilled`，不检查 `expires_at`。已过期但状态仍为 Open 的订单可被修改价格（虽然不能被吃单）。**影响极低。**

---

## 修改文件

| 文件 | 修改内容 |
|------|----------|
| `pallets/trading/nex-market/src/lib.rs` | R1: C1, H1, H2, M1, M2; R2: H1-R2, M1-R2, M2-R2, L1-R2 |
| `pallets/trading/nex-market/src/tests.rs` | R1: +9 回归测试; R2: +6 回归测试 (125 total) |

## 新增测试

### Round 1 (+9)

| 测试名 | 验证内容 |
|--------|----------|
| `c1_resolve_dispute_fails_when_treasury_underfunded` | 国库不足时争议解决失败并保持 Open |
| `h1_update_buy_order_price_adjusts_deposit_upward` | 价格上涨时保证金增加 |
| `h1_update_buy_order_price_adjusts_deposit_downward` | 价格下跌时保证金减少 |
| `h1_update_sell_order_price_no_deposit_change` | 卖单价格变更不影响 reserved |
| `h2_rollback_skips_expired_order` | 过期订单回滚时标记 Expired 不加回订单簿 |
| `m1_reserve_sell_order_rejects_micro_fill` | 微量吃卖单被拒 |
| `m1_accept_buy_order_rejects_micro_fill` | 微量吃买单被拒 |
| `m1_reserve_sell_order_allows_tail_fill_below_minimum` | 尾单低于最低限额时允许 |

### Round 2 (+6)

| 测试名 | 验证内容 |
|--------|----------|
| `h1r2_rollback_preserves_cancelled_order_status` | Cancelled 订单回滚后状态不被覆写 |
| `h1r2_rollback_preserves_expired_order_status_non_filled` | Expired (非 Filled) 订单回滚后状态不被覆写 |
| `m1r2_fee_actually_charged_equals_nex_deducted` | 手续费实际扣除量 = 国库实际收到量 |
| `m2r2_rollback_refreshes_best_ask` | 订单重新入簿后 BestAsk 刷新 |
| `l1r2_order_depth_excludes_expired_orders` | 深度图不含过期订单 |

## 验证

### Round 1
- `cargo test -p pallet-nex-market`: **120/120 ✅**
- `cargo check -p pallet-nex-market`: ✅

### Round 2
- `cargo test -p pallet-nex-market`: **125/125 ✅**
- `cargo check -p pallet-nex-market`: ✅
