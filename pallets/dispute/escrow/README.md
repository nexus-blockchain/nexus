# pallet-escrow

> 路径：`pallets/dispute/escrow/` · Runtime Index: 60

资金托管系统，提供安全的资金锁定、释放、退款和分账功能，作为交易、仲裁等业务的底层基础设施。

## 设计理念

- **安全优先**：外部调用需 `AuthorizedOrigin`，内部 trait 供其他 pallet 调用
- **原子操作**：所有资金操作为原子事务，失败自动回滚
- **状态一致**：托管状态与实际余额严格同步，不存在"有余额无状态"或反向不一致
- **已关闭保护**：`Closed` 的托管不允许重新打开，防止资金重入
- **ED 安全**：最后一笔转账使用 `AllowDeath`，避免 Existential Deposit dust 永久卡住
- **可扩展**：通过 `ExpiryPolicy` trait 支持 runtime 注入自定义到期策略

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
                  │  │  apply_decision_*
                  ▼  ▼
          ┌──────────┐     ┌──────────┐
          │ Disputed │────▶│  Closed  │
          │  (1)     │     │  (3)     │
          └──────────┘     └──────────┘
               │                 ▲
               │                 │
               └─── apply_decision_* ──┘
                    (先解除为 0，再执行资金操作，最终写入 3)

  Resolved(2) 仅在 set_resolved() 后短暂出现，
  apply_decision 系列直接 0 → 3 跳过该中间态。
```

**状态约束**

| 状态 | 值 | 允许的操作 | 禁止的操作 |
|------|----|-----------|-----------|
| Locked | 0 | lock / release / refund / release_split / dispute / schedule_expiry | — |
| Disputed | 1 | apply_decision_* / set_resolved | lock / release / refund / release_split / transfer |
| Resolved | 2 | release / refund（理论上，实际由 apply_decision 一步到位） | — |
| Closed | 3 | — | 所有写操作（`AlreadyClosed`） |

## Extrinsics

### 基础操作
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 0 | `lock` | Auth | 从 payer 锁定资金到托管（拒绝 Closed / Disputed） |
| 1 | `release` | Auth | 全额释放给收款人（拒绝 Disputed）→ Closed |
| 2 | `refund` | Auth | 全额退款给付款人（拒绝 Disputed）→ Closed |
| 3 | `lock_with_nonce` | Auth | 幂等锁定（nonce 严格递增，重复 nonce 静默忽略） |
| 4 | `release_split` | Auth | 分账释放，`BoundedVec<(AccountId, Balance), MaxSplitEntries>` |

### 争议处理
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 5 | `dispute` | Auth | 进入争议状态（Locked → Disputed） |
| 6 | `apply_decision_release_all` | Auth | 裁决：全额释放（解除 Disputed → release_all → Closed） |
| 7 | `apply_decision_refund_all` | Auth | 裁决：全额退款（解除 Disputed → refund_all → Closed） |
| 8 | `apply_decision_partial_bps` | Auth | 裁决：按 bps 比例分账（0–10000，余额退 refund_to → Closed） |

### 管理操作
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 9 | `set_pause` | Admin | 全局暂停开关（应急止血） |
| 10 | `schedule_expiry` | Auth | 安排到期处理（写入 `ExpiringAt` 索引，Disputed 时静默跳过） |
| 11 | `cancel_expiry` | Auth | 取消到期处理（从索引中移除） |

> **Origin 说明**：Auth = `AuthorizedOrigin \| Root`，Admin = `AdminOrigin`

## 存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Locked` | `Map<u64, Balance>` | 托管余额，余额归零时 `remove` |
| `LockStateOf` | `Map<u64, u8>` | 状态码：0=Locked / 1=Disputed / 2=Resolved / 3=Closed |
| `LockNonces` | `Map<u64, u64>` | 幂等 nonce（`lock_with_nonce` 专用） |
| `Paused` | `ValueQuery<bool>` | 全局暂停开关 |
| `ExpiryOf` | `Map<u64, BlockNumber>` | 到期区块号 |
| `ExpiringAt` | `Map<BlockNumber, BoundedVec<u64, MaxExpiringPerBlock>>` | 按区块索引的到期项（`on_initialize` O(1) 查询） |

## 事件

| 事件 | 字段 | 触发时机 |
|------|------|---------|
| `Locked` | `id, amount` | `lock_from` 成功 |
| `Transfered` | `id, to, amount, remaining` | `transfer_from_escrow` 每笔划转 |
| `Released` | `id, to, amount` | `release_all` 全额释放 |
| `Refunded` | `id, to, amount` | `refund_all` 全额退款 |
| `Disputed` | `id, reason` | `dispute` / `set_disputed` |
| `DecisionApplied` | `id, decision(0/1/2)` | 仲裁裁决执行完成 |
| `ExpiryScheduled` | `id, at` | 到期调度注册 |
| `Expired` | `id, action(0/1/2)` | `on_initialize` 到期处理（0=Release / 1=Refund / 2=Noop） |
| `PartialSplit` | `id, release_to, release_amount, refund_to, refund_amount` | `split_partial` 按比例分账 |

## 错误

| 错误 | 说明 |
|------|------|
| `Insufficient` | 余额不足 / bps 超出 10000 |
| `NoLock` | 托管不存在或余额为零 |
| `DisputeActive` | 争议状态下禁止常规资金操作 |
| `AlreadyClosed` | 已关闭的托管不允许重新打开 |
| `GloballyPaused` | 全局暂停中 |
| `ExpiringAtFull` | 到期队列已满（达到 `MaxExpiringPerBlock`） |
| `NotInDispute` | `set_resolved` 要求当前状态为 Disputed(1) |

## Trait 接口

### Escrow（供其他 pallet 内部调用）

```rust
pub trait Escrow<AccountId, Balance> {
    fn escrow_account() -> AccountId;
    fn lock_from(payer: &AccountId, id: u64, amount: Balance) -> DispatchResult;
    fn transfer_from_escrow(id: u64, to: &AccountId, amount: Balance) -> DispatchResult;
    fn release_all(id: u64, to: &AccountId) -> DispatchResult;
    fn refund_all(id: u64, to: &AccountId) -> DispatchResult;
    fn amount_of(id: u64) -> Balance;
    fn split_partial(id: u64, release_to: &AccountId, refund_to: &AccountId, bps: u16) -> DispatchResult;
    fn set_disputed(id: u64) -> DispatchResult;
    fn set_resolved(id: u64) -> DispatchResult;
}
```

| 方法 | 状态约束 | 说明 |
|------|---------|------|
| `lock_from` | ≠ Closed | 从付款人向托管划转，累加到 `Locked[id]` |
| `transfer_from_escrow` | ≠ Disputed, ≠ Closed | 部分划转（可多次分账），余额归零时删除键 |
| `release_all` | ≠ Disputed, ≠ Closed | 全额释放 → 设置 Closed |
| `refund_all` | ≠ Disputed, ≠ Closed | 全额退款 → 设置 Closed |
| `amount_of` | — | 查询当前托管余额 |
| `split_partial` | ≠ Closed, bps ≤ 10000 | 按比例分账：bps/10000 给 release_to → 设置 Closed |
| `set_disputed` | ≠ Closed, 余额 > 0 | 标记为 Disputed(1) |
| `set_resolved` | == Disputed(1) | 解除争议 → Locked(0) |

### ExpiryPolicy（到期策略，由 runtime 注入）

```rust
pub trait ExpiryPolicy<AccountId, BlockNumber> {
    fn on_expire(id: u64) -> Result<ExpiryAction<AccountId>, DispatchError>;
    fn now() -> BlockNumber;
}

pub enum ExpiryAction<AccountId> {
    ReleaseAll(AccountId),
    RefundAll(AccountId),
    Noop,
}
```

## Hooks

### on_initialize

每块执行，通过 `ExpiringAt` 索引获取当前块到期项（O(1)），逐项处理：

1. **Disputed 项**：跳过，重新调度到 14400 块后（约 24h @ 6s/block）
2. **正常项**：调用 `ExpiryPolicy::on_expire` → 执行 `release_all` / `refund_all` / Noop
3. **失败处理**：转账失败仅记录 `log::warn`，不 panic

权重估算：基础 5M ref_time + 每项 50M ref_time（含 ~3 reads + 4 writes）

## 配置参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `EscrowPalletId` | `PalletId` | 托管账户派生种子（`py/escro`） |
| `Currency` | `Currency<AccountId>` | 原生货币接口 |
| `AuthorizedOrigin` | `EnsureOrigin` | 授权操作白名单 |
| `AdminOrigin` | `EnsureOrigin` | 管理员（治理/应急） |
| `MaxExpiringPerBlock` | `u32` | 每块最多处理到期项（防区块超重） |
| `MaxSplitEntries` | `u32` | `release_split` 最大分账条目数 |
| `ExpiryPolicy` | `ExpiryPolicy` | 到期策略（runtime 注入） |
| `WeightInfo` | `WeightInfo` | 权重信息（12 个 extrinsic） |

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

## 测试

```bash
cargo test -p pallet-escrow    # 26 个单元测试
```

覆盖范围：基础锁定释放、争议状态阻断、已关闭防重入、AllowDeath 小额安全、仲裁裁决状态正确性、全局暂停、BoundedVec 分账、幂等 nonce、到期调度与取消、on_initialize 到期处理及争议跳过。

## 相关模块

- [arbitration/](../arbitration/) — 仲裁系统（调用裁决接口）
- [evidence/](../evidence/) — 证据管理
