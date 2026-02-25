# pallet-escrow

> 路径：`pallets/dispute/escrow/` · Runtime Index: 60

资金托管系统，提供安全的资金锁定、释放、退款和分账功能，作为交易、仲裁等业务的底层基础设施。

## 设计理念

- **安全优先**：外部调用需 `AuthorizedOrigin`，内部 trait 供其他 pallet 调用
- **原子操作**：所有资金操作为原子事务，失败自动回滚
- **状态一致**：托管状态（Locked→Disputed→Closed）与实际余额保持一致
- **已关闭保护**：`AlreadyClosed` 的托管不允许重新打开
- **可扩展**：通过 `ExpiryPolicy` 支持自定义到期策略

## 托管状态

| 状态 | 值 | 说明 |
|------|---|------|
| Locked | 0 | 资金已锁定，可正常操作 |
| Disputed | 1 | 争议中，仅允许仲裁操作（`release`/`refund`/`release_split` 返回 `DisputeActive`） |
| Resolved | 2 | 仲裁完成 |
| Closed | 3 | 资金已全部转出，不可重新打开 |

## Extrinsics

### 基础操作
| call_index | 方法 | 说明 |
|:---:|------|------|
| 0 | `lock` | 锁定资金到托管（拒绝 AlreadyClosed） |
| 1 | `release` | 全额释放给收款人（拒绝 DisputeActive） |
| 2 | `refund` | 全额退款给付款人（拒绝 DisputeActive） |
| 3 | `lock_with_nonce` | 幂等锁定（nonce 严格递增，防重放） |
| 4 | `release_split` | 分账释放（`BoundedVec<(AccountId, Balance), MaxSplitEntries>`） |

### 争议处理
| call_index | 方法 | 说明 |
|:---:|------|------|
| 5 | `dispute` | 进入争议状态 |
| 6 | `apply_decision_release_all` | 裁决：全额释放（先解除 Disputed 再 release） |
| 7 | `apply_decision_refund_all` | 裁决：全额退款 |
| 8 | `apply_decision_partial_bps` | 裁决：按 bps 比例分账（0-10000） |

### 管理操作
| call_index | 方法 | 说明 |
|:---:|------|------|
| 9 | `set_pause` | 设置全局暂停（AdminOrigin） |
| 10 | `schedule_expiry` | 安排到期处理（写入 `ExpiringAt` 索引） |
| 11 | `cancel_expiry` | 取消到期处理 |

## 存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Locked` | `Map<u64, Balance>` | 托管余额 |
| `LockStateOf` | `Map<u64, u8>` | 托管状态（0/1/2/3） |
| `LockNonces` | `Map<u64, u64>` | 幂等 nonce |
| `Paused` | `bool` | 全局暂停开关 |
| `ExpiryOf` | `Map<u64, BlockNumber>` | 到期区块 |
| `ExpiringAt` | `Map<BlockNumber, BoundedVec<u64>>` | 按区块索引的到期项（O(1) 查询） |

## 错误

| 错误 | 说明 |
|------|------|
| `Insufficient` | 余额不足 |
| `NoLock` | 托管不存在 |
| `DisputeActive` | 争议状态禁止操作 |
| `AlreadyClosed` | 已关闭的托管不允许重新打开 |
| `GloballyPaused` | 全局暂停中 |
| `ExpiringAtFull` | 到期队列已满 |

## Trait 接口

### Escrow（供其他 pallet 调用）
```rust
pub trait Escrow<AccountId, Balance> {
    fn lock_from(payer: &AccountId, id: u64, amount: Balance) -> DispatchResult;
    fn transfer_from_escrow(id: u64, to: &AccountId, amount: Balance) -> DispatchResult;
    fn release_all(id: u64, to: &AccountId) -> DispatchResult;
    fn refund_all(id: u64, to: &AccountId) -> DispatchResult;
    fn amount_of(id: u64) -> Balance;
    fn escrow_account() -> AccountId;
    fn split_partial(id: u64, release_to: &AccountId, refund_to: &AccountId, bps: u16) -> DispatchResult;
}
```

### ExpiryPolicy（到期策略）
```rust
pub trait ExpiryPolicy<AccountId, BlockNumber> {
    fn on_expire(id: u64) -> Result<ExpiryAction<AccountId>, DispatchError>;
    fn now() -> BlockNumber;
}

pub enum ExpiryAction<AccountId> {
    ReleaseAll(AccountId),
    RefundAll(AccountId),
    NoAction,
}
```

## 配置参数

| 参数 | 说明 |
|------|------|
| `EscrowPalletId` | 托管模块 PalletId（派生托管账户） |
| `AuthorizedOrigin` | 授权操作 Origin（白名单） |
| `AdminOrigin` | 管理员 Origin（治理/应急） |
| `MaxExpiringPerBlock` | 每块最多到期项（防区块超重） |
| `MaxSplitEntries` | `release_split` 最大分账条目数 |
| `ExpiryPolicy` | 到期处理策略（runtime 注入） |
| `WeightInfo` | 权重信息 |

## 集成示例

```rust
// 业务 pallet 使用 Escrow trait
impl<T: Config> Pallet<T> {
    fn create_order(buyer: &T::AccountId, amount: Balance) -> DispatchResult {
        let order_id = Self::next_order_id();
        T::Escrow::lock_from(buyer, order_id, amount)?;
        Ok(())
    }
    
    fn complete_order(order_id: u64, seller: &T::AccountId) -> DispatchResult {
        T::Escrow::release_all(order_id, seller)
    }
}
```

## 测试

```bash
cargo test -p pallet-escrow    # 26 个单元测试
```

## 相关模块

- [arbitration/](../arbitration/) — 仲裁系统（调用裁决接口）
- [evidence/](../evidence/) — 证据管理
