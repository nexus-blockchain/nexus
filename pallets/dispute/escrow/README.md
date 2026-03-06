# pallet-escrow

> 路径：`pallets/dispute/escrow/` · Runtime Index: 60 · 版本：v0.6.0

资金托管系统，提供安全的资金锁定、释放、退款和分账功能，作为交易、仲裁等业务的底层基础设施。

## 设计理念

- **安全优先**：外部调用需 `AuthorizedOrigin`，内部 trait 供其他 pallet 调用
- **原子操作**：所有资金操作为原子事务，失败自动回滚
- **状态一致**：托管状态与实际余额严格同步，不存在"有余额无状态"或反向不一致
- **已关闭保护**：`Closed` 的托管不允许重新打开，防止资金重入
- **ED 安全**：最后一笔转账使用 `AllowDeath`，避免 Existential Deposit dust 永久卡住；多笔分账场景中前 N-1 笔用 `KeepAlive`，最后一笔用 `AllowDeath`
- **零金额防护**：所有资金操作拒绝零金额输入，防止幻影事件和冗余存储写入
- **Observer 通知**：所有资金操作（含 trait 层的 `split_partial`、`apply_decision_partial_bps`、`on_initialize` 到期）均通知 `EscrowObserver`，确保下游业务模块状态同步
- **付款人记录**：`PayerOf` 存储记录首次锁定的付款人，供 `ExpiryPolicy` 默认退款兜底
- **到期自清理**：`release_all` / `refund_all` / `split_partial` 等关闭路径自动清理到期调度（ExpiryOf + ExpiringAt），避免残留
- **可扩展**：通过 `ExpiryPolicy`、`EscrowObserver` trait 支持 runtime 注入自定义策略

## 状态机

```text
              lock / lock_with_nonce
                    │
                    ▼
             ┌──────────┐
             │  Locked   │ ◀─── set_resolved()
             │  (0)      │
             └────┬──┬───┘
                  │  │
      dispute()   │  │  release / refund / release_split
                  │  │  release_partial / refund_partial
                  ▼  ▼
          ┌──────────┐     ┌──────────┐
          │ Disputed │────▶│  Closed  │
          │  (1)     │     │  (3)     │
          └──────────┘     └──────────┘
               │                 ▲
               │                 │
               └─── apply_decision_* ──┘
               └─── force_release / force_refund ──┘
               └─── 争议超时 (on_initialize) ──┘
                    (apply_decision: 先解除为 0，再执行资金操作，最终写入 3)
                    (force: 直接绕过状态机，清理争议时间戳和到期索引)
                    (超时: MaxDisputeDuration 后自动通过 ExpiryPolicy 处理)
```

**状态约束**

| 状态 | 值 | 允许的操作 | 禁止的操作 |
|------|----|-----------|-----------|
| Locked | 0 | lock / release / refund / release_split / dispute / partial / schedule_expiry / split_partial | — |
| Disputed | 1 | apply_decision_* / set_resolved / force_release / force_refund | lock / lock_with_nonce / release / refund / release_split / release_partial / refund_partial / transfer_from_escrow / split_partial |
| Closed | 3 | cleanup_closed | 所有写操作（`AlreadyClosed`） |

## Extrinsics（17 个）

### 基础操作（call_index 0–4）

| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 0 | `lock` | Auth | 从 payer 锁定资金到托管（拒绝 Closed / Disputed / 零金额） |
| 1 | `release` | Auth | 全额释放给收款人（拒绝 Disputed）→ Closed，触发 Observer |
| 2 | `refund` | Auth | 全额退款给付款人（拒绝 Disputed）→ Closed，触发 Observer |
| 3 | `lock_with_nonce` | Auth | 幂等锁定（nonce 严格递增，重复 nonce 静默忽略；拒绝 Closed / Disputed） |
| 4 | `release_split` | Auth | 分账释放 `BoundedVec<(AccountId, Balance), MaxSplitEntries>`（拒绝 Disputed / Closed），逐笔触发 Observer |

### 争议处理（call_index 5–8）

| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 5 | `dispute` | Auth | 进入争议状态（Locked → Disputed），记录 `DisputedAt` 时间戳，支持 BoundedVec 争议详情 |
| 6 | `apply_decision_release_all` | Auth | 裁决全额释放（要求 Disputed → 解除 → release_all → Closed），清理 `DisputedAt` + `ExpiryOf` / `ExpiringAt` |
| 7 | `apply_decision_refund_all` | Auth | 裁决全额退款（要求 Disputed → 解除 → refund_all → Closed），清理 `DisputedAt` + `ExpiryOf` / `ExpiringAt` |
| 8 | `apply_decision_partial_bps` | Auth | 裁决按 bps 比例分账（0–10000），复用 `split_partial` 保证计算一致性，清理 `DisputedAt` + `ExpiryOf` / `ExpiringAt` |

### 管理操作（call_index 9–13, 16）

| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 9 | `set_pause` | Admin | 全局暂停开关（应急止血），发出 `PauseToggled` 事件 |
| 10 | `schedule_expiry` | Auth | 安排到期处理（要求托管存在且非 Closed，Disputed 时静默跳过），已有到期先从旧索引移除 |
| 11 | `cancel_expiry` | Auth | 取消到期处理（从 `ExpiringAt` 和 `ExpiryOf` 中移除） |
| 12 | `force_release` | Admin | 应急强制释放（绕过状态机），清理 `DisputedAt` + `ExpiryOf` / `ExpiringAt` |
| 13 | `force_refund` | Admin | 应急强制退款（绕过状态机），清理 `DisputedAt` + `ExpiryOf` / `ExpiringAt` |
| 16 | `cleanup_closed` | Signed | 批量清理已关闭的托管记录（含 `PayerOf`），跳过非 Closed 项（容错） |

### 部分操作（call_index 14–15）

| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 14 | `refund_partial` | Auth | 部分退款（拒绝 Disputed / Paused / 零金额），余额归零时自动 Closed，触发 Observer |
| 15 | `release_partial` | Auth | 部分释放（拒绝 Disputed / Paused / 零金额），余额归零时自动 Closed，触发 Observer |

> **Origin 说明**：Auth = `AuthorizedOrigin | Root`（Runtime 配置为 `EnsureRoot`，业务 pallet 通过 trait 调用），Admin = `AdminOrigin`，Signed = `ensure_signed`

## 存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Locked` | `Map<u64, Balance>` | 托管余额，余额归零时 `remove` |
| `LockStateOf` | `Map<u64, u8>` | 状态码：0=Locked / 1=Disputed / 3=Closed |
| `LockNonces` | `Map<u64, u64>` | 幂等 nonce（`lock_with_nonce` 专用） |
| `Paused` | `ValueQuery<bool>` | 全局暂停开关 |
| `ExpiryOf` | `Map<u64, BlockNumber>` | 到期区块号 |
| `ExpiringAt` | `Map<BlockNumber, BoundedVec<u64, MaxExpiringPerBlock>>` | 按区块索引的到期项（`on_initialize` O(1) 查询） |
| `DisputedAt` | `Map<u64, BlockNumber>` | 争议时间戳（dispute 时写入，set_resolved / apply_decision_* / force_* 时清除） |
| `PayerOf` | `Map<u64, AccountId>` | 付款人记录（首次 lock_from 写入，ExpiryPolicy 退款兜底目标） |

## 事件

| 事件 | 字段 | 触发时机 |
|------|------|---------|
| `Locked` | `id, payer, amount` | `lock_from` 成功 |
| `Transferred` | `id, to, amount, remaining` | `transfer_from_escrow` / `release_split` 每笔划转 |
| `Released` | `id, to, amount` | `release_all` 全额释放 |
| `Refunded` | `id, to, amount` | `refund_all` 全额退款 |
| `PartialRefunded` | `id, to, amount, remaining` | `refund_partial` 部分退款 |
| `PartialReleased` | `id, to, amount, remaining` | `release_partial` 部分释放 |
| `Disputed` | `id, reason, detail, at` | `dispute` / `set_disputed`（detail 为 BoundedVec） |
| `DecisionApplied` | `id, decision(0/1/2)` | 仲裁裁决执行完成（0=Release / 1=Refund / 2=PartialBps） |
| `ExpiryScheduled` | `id, at` | 到期调度注册 |
| `Expired` | `id, action(0/1/2)` | `on_initialize` 到期处理（0=Release / 1=Refund / 2=Noop） |
| `PartialSplit` | `id, release_to, release_amount, refund_to, refund_amount` | `split_partial` 按比例分账 |
| `PauseToggled` | `paused` | `set_pause` 全局暂停切换 |
| `ForceAction` | `id, action, to, amount` | `force_release`(action=0) / `force_refund`(action=1) 强制操作 |
| `Cleaned` | `ids: Vec<u64>` | `cleanup_closed` 清理记录 |

## 错误

| 错误 | 说明 |
|------|------|
| `Insufficient` | 余额不足 / 金额为零 / bps 超出 10000 |
| `NoLock` | 托管不存在或余额为零 |
| `DisputeActive` | 争议状态下禁止常规资金操作（lock / release / refund / split 等） |
| `AlreadyClosed` | 已关闭的托管不允许重新打开或再次操作 |
| `GloballyPaused` | 全局暂停中，变更操作被拒绝 |
| `ExpiringAtFull` | 目标块到期队列已满（达到 `MaxExpiringPerBlock`） |
| `NotInDispute` | `set_resolved` / `apply_decision_*` 要求当前状态为 Disputed(1) |

## Trait 接口

### `Escrow<AccountId, Balance>`（供其他 pallet 内部调用）

```rust
pub trait Escrow<AccountId, Balance> {
    fn escrow_account() -> AccountId;
    fn lock_from(payer: &AccountId, id: u64, amount: Balance) -> DispatchResult;
    fn transfer_from_escrow(id: u64, to: &AccountId, amount: Balance) -> DispatchResult;
    fn release_all(id: u64, to: &AccountId) -> DispatchResult;
    fn refund_all(id: u64, to: &AccountId) -> DispatchResult;
    fn refund_partial(id: u64, to: &AccountId, amount: Balance) -> DispatchResult;
    fn release_partial(id: u64, to: &AccountId, amount: Balance) -> DispatchResult;
    fn amount_of(id: u64) -> Balance;
    fn split_partial(id: u64, release_to: &AccountId, refund_to: &AccountId, bps: u16) -> DispatchResult;
    fn set_disputed(id: u64) -> DispatchResult;
    fn set_resolved(id: u64) -> DispatchResult;
}
```

| 方法 | 状态约束 | 说明 |
|------|---------|------|
| `lock_from` | ≠ Closed, ≠ Disputed, amount > 0 | 从付款人向托管划转，累加到 `Locked[id]`，首次写入 `PayerOf` |
| `transfer_from_escrow` | ≠ Disputed, ≠ Closed | 部分划转（可多次分账），余额归零时删除键。**注意：不触发 Observer** |
| `release_all` | ≠ Disputed, ≠ Closed | 全额释放 → Closed，触发 `Observer::on_released` |
| `refund_all` | ≠ Disputed, ≠ Closed | 全额退款 → Closed，触发 `Observer::on_refunded` |
| `refund_partial` | ≠ Disputed, ≠ Closed, amount > 0 | 部分退款，余额归零时 Closed，触发 `Observer::on_refunded` |
| `release_partial` | ≠ Disputed, ≠ Closed, amount > 0 | 部分释放，余额归零时 Closed，触发 `Observer::on_released` |
| `amount_of` | — | 查询当前托管余额 |
| `split_partial` | ≠ Disputed, ≠ Closed, bps ≤ 10000 | 按比例分账 → Closed（第一笔 KeepAlive，最后 AllowDeath），触发 `Observer::on_released` + `Observer::on_refunded` |
| `set_disputed` | ≠ Closed, ≠ Disputed, 余额 > 0 | 标记 Disputed(1)，记录时间戳，触发 `Observer::on_disputed` |
| `set_resolved` | == Disputed(1) | 解除争议 → Locked(0)，清除时间戳 |

### `EscrowObserver<AccountId, Balance>`（状态变更观察者）

```rust
pub trait EscrowObserver<AccountId, Balance> {
    fn on_released(id: u64, to: &AccountId, amount: Balance);
    fn on_refunded(id: u64, to: &AccountId, amount: Balance);
    fn on_expired(id: u64, action: u8);
    fn on_disputed(id: u64);
    fn on_force_action(id: u64, action: u8);
}
```

默认提供 `()` 空实现。业务模块可实现此 trait 以同步托管状态变更。

### `ExpiryPolicy<AccountId, BlockNumber>`（到期策略）

```rust
pub trait ExpiryPolicy<AccountId, BlockNumber> {
    fn on_expire(id: u64) -> Result<ExpiryAction<AccountId>, DispatchError>;
}

pub enum ExpiryAction<AccountId> {
    ReleaseAll(AccountId),
    RefundAll(AccountId),
    Noop,
}
```

## Hooks

### `on_initialize`

每块执行，通过 `ExpiringAt` 索引获取当前块到期项（O(1)），逐项处理：

1. **Disputed 项（未超时）**：重新调度到 14400 块后（约 24h @ 6s/block），如果目标块已满则尝试相邻块（最多 10 次）；若全部失败则清理 `ExpiryOf` 防止残留
2. **Disputed 项（已超时）**：`DisputedAt + MaxDisputeDuration ≤ 当前块` 时，清除争议状态，回落到正常到期处理流程（安全网，防止资金永久锁死）
3. **正常项**：调用 `ExpiryPolicy::on_expire` → 执行 `release_all` / `refund_all` / Noop → 清理 `ExpiryOf` → 通知 `Observer::on_expired`
4. **失败处理**：转账失败仅记录 `log::warn`，不 panic

### 争议超时安全网

- `dispute` / `set_disputed` 触发时，若当前无 `ExpiryOf`，自动调度 `now + MaxDisputeDuration` 到期
- 确保每个争议都有超时兜底，不会因仲裁方失联导致资金永久锁死

**权重估算**：基础 5M ref_time (1K proof_size) + 每项 50M ref_time (3.5K proof_size)

## 配置参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `EscrowPalletId` | `PalletId` | 托管账户派生种子（`py/escro`） |
| `Currency` | `Currency<AccountId>` | 原生货币接口 |
| `AuthorizedOrigin` | `EnsureOrigin` | 授权操作白名单（外部调用入口） |
| `AdminOrigin` | `EnsureOrigin` | 管理员（治理/应急操作） |
| `MaxExpiringPerBlock` | `u32` | 每块最多处理到期项（防区块超重） |
| `MaxSplitEntries` | `u32` | `release_split` 最大分账条目数 |
| `ExpiryPolicy` | `ExpiryPolicy` | 到期策略（runtime 注入，默认 `RefundAll(payer)` 兜底） |
| `MaxReasonLen` | `u32` | 争议原因最大长度（BoundedVec 上限） |
| `TokenHandler` | `TokenEscrowHandler` | Entity Token 托管处理器 |
| `Observer` | `EscrowObserver` | 托管状态变更观察者 |
| `MaxCleanupPerCall` | `u32` | `cleanup_closed` 每次最大清理条目数 |
| `MaxDisputeDuration` | `BlockNumber` | 争议最大持续时间，超时后自动通过 ExpiryPolicy 处理（默认 100800 ≈ 7天） |
| `WeightInfo` | `WeightInfo` | 权重信息（覆盖全部 20 个 extrinsic） |

## 集成示例

```rust
// 业务 pallet 通过 Escrow trait 调用
impl<T: Config> Pallet<T> {
    fn create_order(buyer: &T::AccountId, order_id: u64, amount: Balance) -> DispatchResult {
        T::Escrow::lock_from(buyer, order_id, amount)?;
        Ok(())
    }

    fn complete_order(order_id: u64, seller: &T::AccountId) -> DispatchResult {
        T::Escrow::release_all(order_id, seller)
    }

    fn cancel_order(order_id: u64, buyer: &T::AccountId) -> DispatchResult {
        T::Escrow::refund_all(order_id, buyer)
    }

    fn partial_delivery(order_id: u64, seller: &T::AccountId, amount: Balance) -> DispatchResult {
        T::Escrow::release_partial(order_id, seller, amount)
    }

    fn raise_dispute(order_id: u64) -> DispatchResult {
        T::Escrow::set_disputed(order_id)
    }

    fn resolve_with_split(
        order_id: u64,
        seller: &T::AccountId,
        buyer: &T::AccountId,
        seller_bps: u16,
    ) -> DispatchResult {
        T::Escrow::set_resolved(order_id)?;
        T::Escrow::split_partial(order_id, seller, buyer, seller_bps)
    }
}
```

## 安全审计记录

经过 4 轮深度审计，已修复全部发现问题：

| 轮次 | 发现 | 修复 | 关键修复 |
|------|------|------|---------|
| R1 | 3H + 3M + 4L | 全部 | C1(状态检查) · C2(Closed防重入) · H1(裁决需Disputed) · H2(重调度重试) · H3(AllowDeath) |
| R2 | 1H + 4M + 1L | 全部 | H1(split KeepAlive/AllowDeath) · M1(lock_from争议检查) · M2(cleanup ExpiringAt) · M4(DisputedAt清理) |
| R3 | 4M + 3L | 全部 | M1(零金额防护) · M2(force清理到期索引) · M3(split Observer通知) · M4(partial_bps Observer) · L1(release_split Closed检查) · L2(重调度失败清理ExpiryOf) · L3(benchmarking重写) |
| R4 | 3H + 2M + 2L | 全部 | H1(StorageVersion) · H2(争议超时安全网) · H3(Benchmark precondition) · M1(Event拼写) · M2(Resolved幽灵态) · L1(ExpiryPolicy::now废弃) · L2(前端canForce逻辑) |
| R5 | 3H + 4M + 3L | 全部 | H1(AuthorizedOrigin→EnsureRoot) · H2(Observer::on_expired) · H3(apply_decision 清理expiry) · M1(PayerOf+ExpiryPolicy退款兜底) · M3(Token死代码标注) · M4(前端ValueQuery检测) · L1(NotClosed移除) · L2(Resolved注释) · L3(schedule_expiry验证) |

## 测试

```bash
cargo test -p pallet-escrow    # 84 个单元测试
```

**覆盖范围**（84 个测试）：

- **基础操作**：锁定/释放/退款、零金额拒绝、AllowDeath 小额安全
- **幂等 nonce**：重复 nonce 静默忽略、递增 nonce 累加
- **争议流程**：争议阻断 release/refund/lock/release_split、重复 dispute 防护、set_disputed/set_resolved trait 调用
- **仲裁裁决**：apply_decision_* 要求 Disputed 状态、全额释放/退款/按比例分账、DisputedAt 清理
- **已关闭保护**：lock/lock_with_nonce/release_split 拒绝 Closed 托管、double release 拒绝
- **部分操作**：refund_partial/release_partial 正常流程 + 争议阻断 + 零金额拒绝 + 余额归零自动关闭
- **强制操作**：force_release/force_refund 绕过争议 + 需要 Admin Origin + 清理到期索引
- **到期调度**：schedule/cancel_expiry、on_initialize 到期处理、争议项重调度（含失败清理）
- **分账**：release_split BoundedVec 正常/超额、split_partial 争议检查 + Observer 通知 + ED 安全
- **全局暂停**：set_pause 阻断操作 + 事件发出
- **存储清理**：cleanup_closed 正常/容错跳过非 Closed + ExpiringAt 索引清理
- **Observer 通知**：split_partial 双向通知、apply_decision_partial_bps 释放通知、release_split 逐笔通知
- **争议超时**：自动调度 expiry、保留已有 expiry、trait 层自动调度、超时强制执行 ExpiryPolicy、未超时重调度
- **R5 修复**：apply_decision_* 清理 expiry 索引、PayerOf 记录/不覆盖/cleanup 清理、schedule_expiry 拒绝不存在/已关闭

## 相关模块

- [arbitration/](../arbitration/) — 仲裁系统（调用 `apply_decision_*` 裁决接口）
- [evidence/](../evidence/) — 证据管理（争议举证）

## 版本历史

| 版本 | 说明 |
|------|------|
| v0.1.0 | 初始实现：基础锁定/释放/退款 + 到期调度 |
| v0.2.0 | R1 审计修复 + 功能增强（部分操作 · 争议详情 · 强制操作 · Token 托管 · Observer · cleanup · 全局暂停） |
| v0.2.1 | R2 审计修复（split KeepAlive · lock_from 争议检查 · cleanup ExpiringAt · DisputedAt 清理） |
| v0.3.0 | R3 审计修复（零金额防护 · force 清理到期索引 · split/partial Observer 通知 · release_split Closed 检查 · 重调度失败清理 · benchmarking 完全重写覆盖 20 extrinsic） |
| v0.4.0 | R4 上线审计（StorageVersion · 争议超时安全网 MaxDisputeDuration · Benchmark precondition 重写 · Event::Transferred 拼写修正 · Resolved(2) 幽灵态清理 · ExpiryPolicy::now/is_noop 废弃代码移除 · 前端 canForce 修复） |
| v0.5.0 | R5 安全加固（AuthorizedOrigin→EnsureRoot 堵死公开调用 · PayerOf 存储+DefaultExpiryPolicy 退款兜底 · Observer::on_expired 补全 · apply_decision 裁决清理 expiry 残留 · remove_expiry_schedule 提取消重复 · schedule_expiry 验证存在性 · NotClosed 死代码移除 · Token 死代码标注 · 前端 ValueQuery 检测修复） |
