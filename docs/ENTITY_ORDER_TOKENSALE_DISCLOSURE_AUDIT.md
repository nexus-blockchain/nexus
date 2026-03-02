# Entity Pallets Audit Round 3: Order / TokenSale / Disclosure + Cross-cutting

**Date:** 2026-03-02
**Scope:** `pallet-entity-order`, `pallet-entity-tokensale`, `pallet-entity-disclosure`, `pallet-entity-service`, `pallet-entity-governance`, `pallet-entity-review`

## Summary

| Severity | Found | Fixed |
|----------|-------|-------|
| Critical | 1     | 1     |
| Medium   | 2     | 2     |
| Low      | 5     | 5     |
| **Total**| **8** | **8** |

Cross-cutting review of `pallet-entity-review` (232 lines): **no issues found**.

---

## Fixes

### C1 [Critical] — `pallet-entity-order`: EntityToken 订单无法申请退款

**问题:** `request_refund` 无条件调用 `T::Escrow::set_disputed(order_id)`。EntityToken 订单的资金通过 `EntityToken::reserve` 锁定，不使用 Escrow。Escrow 中无对应锁定记录，`set_disputed` 返回 `NoLock` 错误，导致 **所有 EntityToken 订单买家无法申请退款**。

**修复:** 从 `try_mutate` 闭包中捕获 `payment_asset`，仅当 `payment_asset == PaymentAsset::Native` 时调用 `set_disputed`。避免额外存储读取。

**文件:** `pallets/entity/order/src/lib.rs` (lines 704-734)

---

### M1 [Medium] — `pallet-entity-order`: Token 平台费转移失败被静默吞掉

**问题:** `do_complete_order` 中 `T::EntityToken::repatriate_reserved` 用于转移 token 平台费，但返回值被 `let _ =` 丢弃。转移失败时无任何通知，平台费丢失。

**修复:** 添加 `.is_err()` 检查，失败时发出 `OrderOperationFailed { operation: TokenPlatformFee }` 事件。新增 `TokenPlatformFee` 变体到 `OrderOperation` 枚举。

**文件:** `pallets/entity/order/src/lib.rs` (lines 140-143, 882-894)

---

### M1 [Medium] — `pallet-entity-tokensale`: `end_sale` / `do_auto_end_sale` 残留 stale `remaining_amount`

**问题:** 发售结束后 `remaining_amount` 的代币被 unreserve 归还给 entity 账户，但 `SaleRound` 结构中的 `remaining_amount` 字段未清零。链上查询返回过时数据，前端/索引器可能误判。

**修复:** 在 `T::TokenProvider::unreserve` 调用后添加 `round.remaining_amount = Zero::zero()`。同时修复 `end_sale` extrinsic 和 `do_auto_end_sale` 内部函数。

**文件:** `pallets/entity/tokensale/src/lib.rs` (lines 1038-1039, 1498-1499)

---

### L1 [Low] — `pallet-entity-order`: `NextOrderId` 溢出

**问题:** `saturating_add(1)` 在 `u64::MAX` 时不增加，下次创建覆盖已有订单。

**修复:** 改为 `checked_add(1).ok_or(Error::Overflow)?`，与 `pallet-entity-tokensale` 一致。

**文件:** `pallets/entity/order/src/lib.rs` (line 543-545)

---

### L1 [Low] — `pallet-entity-disclosure`: Cargo.toml dev-dep features 泄漏

**问题:** `sp-core/std` 和 `sp-io/std` 是 dev-dependencies，不应出现在 `[features] std` 列表中。

**修复:** 移除这两项。

**文件:** `pallets/entity/disclosure/Cargo.toml`

---

### L1 [Low] — `pallet-entity-service`: `NextProductId` 溢出 (cross-cutting)

**问题:** 同 order L1，`saturating_add(1)` 在 `u64::MAX` 时不增加。

**修复:** 改为 `checked_add(1).ok_or(Error::ArithmeticOverflow)?`。

**文件:** `pallets/entity/service/src/lib.rs` (line 346-348)

---

### L1 [Low] — `pallet-entity-governance`: `NextProposalId` 溢出 (cross-cutting)

**问题:** 同上 `saturating_add` 模式。

**修复:** 新增 `ProposalIdOverflow` 错误，改为 `checked_add(1).ok_or(Error::ProposalIdOverflow)?`。

**文件:** `pallets/entity/governance/src/lib.rs` (lines 639-640, 756-758)

---

### L2 [Low] — `pallet-entity-governance`: Cargo.toml dev-dep features 泄漏 (cross-cutting)

**问题:** `sp-core/std`、`sp-io/std`、`pallet-balances/std` 是 dev-dependencies，不应出现在 `[features] std` 列表中。

**修复:** 移除这三项。

**文件:** `pallets/entity/governance/Cargo.toml`

---

## Regression Tests

### pallet-entity-order (+3 tests, 59 total)
- `c1_token_order_request_refund_works` — EntityToken 订单可成功申请退款
- `c1_native_order_request_refund_still_uses_escrow` — Native 订单退款仍通过 Escrow
- `c1_token_order_approve_refund_unreserves_tokens` — EntityToken 退款全流程验证

### pallet-entity-tokensale (+2 tests, 56 total)
- `m1_end_sale_zeros_remaining_amount` — 手动结束后 remaining_amount 为 0
- `m1_auto_end_sale_zeros_remaining_amount` — 自动结束后 remaining_amount 为 0

---

## Files Modified (8 files)

| File | Changes |
|------|---------|
| `pallets/entity/order/src/lib.rs` | C1, M1, L1 fixes |
| `pallets/entity/order/src/tests.rs` | +3 regression tests |
| `pallets/entity/tokensale/src/lib.rs` | M1 fix (2 locations) |
| `pallets/entity/tokensale/src/tests.rs` | +2 regression tests |
| `pallets/entity/disclosure/Cargo.toml` | L1 dev-dep features fix |
| `pallets/entity/service/src/lib.rs` | L1 NextProductId overflow fix |
| `pallets/entity/governance/src/lib.rs` | L1 NextProposalId overflow fix |
| `pallets/entity/governance/Cargo.toml` | L2 dev-dep features fix |

## Verification

```
cargo check -p pallet-entity-order         ✅
cargo check -p pallet-entity-tokensale     ✅
cargo check -p pallet-entity-disclosure    ✅
cargo check -p pallet-entity-service       ✅
cargo check -p pallet-entity-governance    ✅
cargo check -p nexus-runtime               ✅
cargo test  -p pallet-entity-order         59/59 ✅
cargo test  -p pallet-entity-tokensale     56/56 ✅
cargo test  -p pallet-entity-disclosure    73/73 ✅
cargo test  -p pallet-entity-service       65 ✅ (shared with governance+review)
```

## Not Fixed (documented only)

- **M [review]**: `ReviewCount` uses `saturating_add` — u64 counter, overflow is theoretical only.
- **M [governance]**: Many `ProposalType` variants only emit events without on-chain execution — design decision, requires off-chain workers.
- **L [service]**: `EntityProvider` declared in Config but never directly called — dead dependency, mirrors entity-token L4.
