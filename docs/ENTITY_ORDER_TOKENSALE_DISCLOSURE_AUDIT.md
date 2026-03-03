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

---

# Entity TokenSale Deep Audit Round 4

**Date:** 2026-03-04
**Scope:** `pallet-entity-tokensale` — deep audit of extrinsics, arithmetic, cross-pallet interactions, storage consistency

## Summary

| Severity | Found | Fixed |
|----------|-------|-------|
| High     | 1     | 1     |
| Medium   | 3     | 3     |
| Low      | 2     | 1     |
| **Total**| **6** | **5** |

---

## Fixes

### H1-deep [High] — `claim_tokens` / `unlock_tokens` 忽略 `repatriate_reserved` 返回值

**问题:** `claim_tokens` 和 `unlock_tokens` 调用 `T::TokenProvider::repatriate_reserved` 后仅传播 `Err`（通过 `?`），但丢弃 `Ok(actual)` 返回值。`repatriate_reserved` 返回实际转移量，可能小于请求量（例如跨 pallet reserve 竞争导致 entity_account 的 reserved 不足）。此时 `sub.unlocked_amount` 被记录为请求量而非实际量，产生 **幻影代币记账**——认购者的解锁记录高于实际收到的代币。

**影响:** 后续 `unlock_tokens` 基于膨胀的 `unlocked_amount` 计算剩余可解锁量，可能少发代币或完全跳过解锁。

**修复:** 捕获 `repatriate_reserved` 返回的 `actual` 值，`ensure!(actual == requested, IncompleteUnreserve)`。

**文件:** `pallets/entity/tokensale/src/lib.rs` — `claim_tokens` (line ~1084) + `unlock_tokens` (line ~1138)

---

### M1-deep [Medium] — `cancel_sale` 不清零 `remaining_amount`

**问题:** Round 3 的 M1 修复了 `end_sale` 和 `do_auto_end_sale` 中 unreserve 后 `remaining_amount` 未清零的问题，但 **`cancel_sale` 遗漏了同样的修复**。Active 状态的轮次被取消后，`remaining_amount` 仍保留原始未售数量，链上查询返回过时数据。

**修复:** 在 `cancel_sale` 的 Active 分支 unreserve 后添加 `round.remaining_amount = Zero::zero()`。

**文件:** `pallets/entity/tokensale/src/lib.rs` (line ~1185)

---

### M2-deep [Medium] — `calculate_dutch_price` 溢出可导致价格低于 `end_price`

**问题:** 荷兰拍卖价格计算中 `price_range.saturating_mul(elapsed)` 在极端 price_range 值下溢出 u128，`saturating_mul` 饱和到 `u128::MAX`。随后 `u128::MAX / total_duration` 得到巨大的 `price_drop`，`start_price.saturating_sub(price_drop)` 饱和到 0。最终返回的价格可能远低于 `end_price`，甚至为 0。

**修复:** 在 `saturating_sub` 后添加 `.max(end_u128)` 钳位，确保计算价格不低于终止价格。

**文件:** `pallets/entity/tokensale/src/lib.rs` — `calculate_dutch_price` (line ~1412)

---

### M3-deep [Medium] — 4 处 `unreserve` 返回值被忽略

**问题:** `unreserve` 返回 deficit（未能 unreserve 的金额）。以下 4 处调用丢弃返回值，unreserve 不完整时无任何通知：
1. `end_sale` — 释放未售代币
2. `cancel_sale` — 释放未售代币
3. `do_auto_end_sale` — 自动结束释放未售代币
4. `reclaim_unclaimed_tokens` — 释放未领取代币

仅 `claim_refund` 正确检查了 deficit。

**修复:** 捕获 deficit，非零时通过 `log::warn!` 记录。添加 `log` 依赖到 Cargo.toml。

**文件:** `pallets/entity/tokensale/src/lib.rs` (4 locations) + `Cargo.toml`

---

### L1-deep [Low] — `Subscription.last_unlock_at` 死字段（记录不修复）

**问题:** `last_unlock_at` 在 `subscribe` 中初始化为 `now`，在 `unlock_tokens` 中更新为当前块号，但 **从未被任何计算逻辑读取**。`calculate_unlockable` 使用 `subscribed_at` 作为锁仓起点，不依赖 `last_unlock_at`。

**原因不修复:** 移除字段需要存储迁移。字段无功能影响，仅占用少量存储。

---

### L2-deep [Low] — Cargo.toml 缺失依赖和 feature flags

**问题:**
- 缺少 `log` 依赖（M3-deep 所需）
- `runtime-benchmarks` 缺少 `sp-runtime/runtime-benchmarks`
- `try-runtime` 缺少 `sp-runtime/try-runtime`

**修复:** 添加 `log = { workspace = true }` 依赖，`log/std` 到 std features，以及缺失的 sp-runtime feature 传播。

**文件:** `pallets/entity/tokensale/Cargo.toml`

---

## Regression Tests (+7 tests, 63 total)

| Test | Validates |
|------|-----------|
| `h1_deep_claim_tokens_rejects_partial_repatriation` | claim_tokens 在 reserve 不足时返回 IncompleteUnreserve |
| `h1_deep_unlock_tokens_rejects_partial_repatriation` | unlock_tokens 在 reserve 不足时返回 IncompleteUnreserve |
| `h1_deep_claim_and_unlock_full_amount_succeeds` | 正常全额 claim + unlock 带锁仓成功 |
| `m1_deep_cancel_sale_zeros_remaining_amount` | Active 取消后 remaining_amount 归零 |
| `m1_deep_cancel_not_started_keeps_remaining` | NotStarted 取消保持原始 remaining_amount |
| `m2_deep_dutch_price_clamped_to_end_price` | 极端 price_range 下价格 >= end_price |
| `m2_deep_dutch_price_reaches_end_price_at_end` | 正常递减在结束时刻等于 end_price |

## Files Modified (4 files)

| File | Changes |
|------|---------|
| `pallets/entity/tokensale/src/lib.rs` | H1-deep (2), M1-deep, M2-deep, M3-deep (4) |
| `pallets/entity/tokensale/src/mock.rs` | +set_reserved/get_reserved test helpers |
| `pallets/entity/tokensale/src/tests.rs` | +7 regression tests |
| `pallets/entity/tokensale/Cargo.toml` | log dep + L2 feature flags |

## Verification

```
cargo test  -p pallet-entity-tokensale     63/63 ✅
cargo check -p nexus-runtime               ✅
```

## Not Fixed (documented only)

- **L1-deep [tokensale]**: `Subscription.last_unlock_at` dead field — 需存储迁移，无功能影响。
