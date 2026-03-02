# pallet-grouprobot-subscription 深度审计报告 (Round 2)

## 审计范围

| 文件 | 行数 | 说明 |
|------|------|------|
| `pallets/grouprobot/subscription/src/lib.rs` | 734→739 | 核心 pallet 代码 |
| `pallets/grouprobot/subscription/src/mock.rs` | 148 | 测试 mock |
| `pallets/grouprobot/subscription/src/tests.rs` | 732→871 | 单元测试 |
| `pallets/grouprobot/rewards/src/lib.rs` | 325 | 奖励 pallet (交叉审计) |
| `pallets/grouprobot/rewards/src/tests.rs` | 255 | 奖励测试 (交叉审计) |
| `pallets/grouprobot/primitives/src/lib.rs` | 719 | 共享类型/trait |
| `pallets/grouprobot/consensus/src/lib.rs` | on_era_end | Era 编排 (交叉审计) |
| `runtime/src/configs/mod.rs` | 1680-1721 | Runtime 配置 |

## 跨 Pallet 互通分析

### 订阅费数据流

```
consensus::on_era_end
  ├─ subscription::settle_era()
  │   ├─ settle_era_subscriptions() → 游标分页遍历 Subscriptions
  │   │   ├─ escrow >= fee: unreserve → 90% 直转运营者, 10% 转国库
  │   │   └─ escrow < fee: Active→PastDue→Suspended 降级
  │   ├─ settle_ad_commitments() → 游标分页遍历 AdCommitments
  │   │   ├─ delivered >= committed: 重置 underdelivery_eras
  │   │   └─ delivered < committed: underdelivery_eras++, 超限→Cancelled
  │   └─ return subscription_income (u128)
  │
  ├─ total_pool = inflation (订阅 node_share 已直发, 不参与权重分配) ✅
  └─ rewards::distribute_and_record(era, total_pool=inflation, ...)
```

### 关键设计确认

1. **total_pool = inflation only**: consensus 正确传入 `let total_pool = inflation;`，订阅收入不参与权重分配 ✅
2. **rewards pallet 无新发现**: 前轮 H1-H3, M1, L1-L2 修复均正确，本轮确认 18/18 测试通过 ✅
3. **90/10 拆分**: 运营者 90%, 国库 10% (代码注释一致)

## 发现与修复

### C1 [Critical] — settle 游标跳过未处理条目

**问题**: `settle_era_subscriptions` 和 `settle_ad_commitments` 在分页中断时 (`settled >= max_settle`)，将 cursor 设为当前**未处理**条目的 key：

```rust
if settled >= max_settle {
    last_key = Some(bot_hash);  // ← BUG: 未处理的 key
    break;
}
```

Substrate `StorageMap::iter_from(raw_key)` 从给定 key **之后**开始迭代。因此 cursor 指向未处理 key 时，该条目在下次迭代中被永久跳过。每个分页边界跳过一条订阅/承诺。

**影响**: 大量订阅时，每 Era 结算漏扣一条订阅费；广告承诺达标检查漏检一条。

**修复**: break 时不更新 `last_key`，cursor 保持为最后已处理的 key。

**文件**: `pallets/grouprobot/subscription/src/lib.rs` (两处)

### H1 [High] — SubscriptionFeeCollected 事件在转账失败时仍发出

**问题**: `settle_era_subscriptions` 中 `SubscriptionFeeCollected` 事件在 `if node_ok && treasury_ok` 检查**之外**发出。即使两笔转账都失败（log::warn），仍会发出"费用已收取"事件，误导链上索引器和前端。

**修复**: 将事件移入 `if node_ok && treasury_ok` 成功分支内。

**文件**: `pallets/grouprobot/subscription/src/lib.rs`

### M1 [Medium] — deposit_subscription 任意金额即可重新激活

**问题**: `deposit_subscription` 在 PastDue/Suspended 状态下，存入任意非零金额即重新激活为 Active。恶意用户可存入 1 unit 获得瞬时付费层级权益（直到下次 Era 结算再次降级）。

**修复**: 仅在充值后 escrow 总额 >= fee_per_era 时才重新激活：

```rust
let new_escrow = SubscriptionEscrow::<T>::get(&bot_id_hash);
if new_escrow >= sub.fee_per_era {
    // 重新激活
}
```

**文件**: `pallets/grouprobot/subscription/src/lib.rs`

### L1 [Low] — commit_ads 未检查运营者

**问题**: `subscribe` 要求 `bot_operator().is_some()`，但 `commit_ads` 不检查。广告承诺可为无运营者的 Bot 创建，造成不一致。

**修复**: 添加 `ensure!(T::BotRegistry::bot_operator(&bot_id_hash).is_some(), Error::<T>::BotHasNoOperator)`。

**文件**: `pallets/grouprobot/subscription/src/lib.rs`

## 记录但未修复

### M2 [Medium] — 已取消条目消耗分页配额

两个 settle 函数对 `Cancelled` 条目执行 `continue` 但仍计入 `settled`。随着取消的订阅/承诺累积，有效结算吞吐量下降。需要垃圾回收机制清理已取消记录。

### M3 [Medium] — 无垃圾回收机制

已取消的 `Subscriptions` 和 `AdCommitments` 永久留在存储中，无清理路径。长期运行后存储膨胀并拖慢迭代。

### L2 [Low] — 硬编码 90/10 拆分比例

运营者/国库拆分比例硬编码为 90/10。建议改为 Config 常量以便治理调整。

### L3 [Low] — 硬编码 Weight

所有 6 个 extrinsic 使用硬编码 Weight，无 WeightInfo/benchmark。需引入完整 benchmark 框架。

## 新增测试 (5 个, 43 total, was 38)

| 测试 | 覆盖 |
|------|------|
| `c1_settle_cursor_stores_last_processed_key` | C1: 游标正确清除 |
| `h1_no_event_on_transfer_failure` | H1: 转账成功才发事件 |
| `m1_deposit_does_not_reactivate_if_escrow_below_fee` | M1: 余额不足不重新激活 |
| `m1_deposit_reactivates_when_escrow_covers_fee` | M1: 余额充足才激活 |
| `l1_commit_ads_fails_no_operator` | L1: 无运营者拒绝广告承诺 |

## 验证

```
cargo test -p pallet-grouprobot-subscription  → 43/43 ✅
cargo test -p pallet-grouprobot-rewards       → 18/18 ✅
cargo check -p pallet-grouprobot-subscription ✅
cargo check -p pallet-grouprobot-rewards      ✅
cargo check -p nexus-runtime                  ✅
```

## 修改文件汇总

| 文件 | 变更 |
|------|------|
| `pallets/grouprobot/subscription/src/lib.rs` | C1, H1, M1, L1 修复 |
| `pallets/grouprobot/subscription/src/tests.rs` | +5 回归测试 (43 total) |

## 发现汇总

| ID | 严重性 | 状态 | 描述 |
|----|--------|------|------|
| C1 | Critical | ✅ 已修复 | settle 游标跳过未处理条目 |
| H1 | High | ✅ 已修复 | 转账失败仍发出 FeeCollected 事件 |
| M1 | Medium | ✅ 已修复 | 任意金额重新激活订阅 |
| M2 | Medium | 📋 记录 | 已取消条目消耗分页配额 |
| M3 | Medium | 📋 记录 | 无垃圾回收机制 |
| L1 | Low | ✅ 已修复 | commit_ads 未检查运营者 |
| L2 | Low | 📋 记录 | 硬编码拆分比例 |
| L3 | Low | 📋 记录 | 硬编码 Weight |

**总计: 8 项发现, 4 项已修复 (1 Critical, 1 High, 1 Medium, 1 Low)**
