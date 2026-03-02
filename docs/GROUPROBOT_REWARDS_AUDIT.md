# pallet-grouprobot-rewards 深度审计报告

## 审计范围

| 文件 | 行数 | 说明 |
|------|------|------|
| `pallets/grouprobot/rewards/src/lib.rs` | 262→325 | 核心 pallet 代码 |
| `pallets/grouprobot/rewards/src/mock.rs` | 84 | 测试 mock |
| `pallets/grouprobot/rewards/src/tests.rs` | 141→255 | 单元测试 |
| `pallets/grouprobot/primitives/src/lib.rs` | 709→719 | 新增 OrphanRewardClaimer trait |
| `pallets/grouprobot/consensus/src/lib.rs` | 739→741 | finalize_exit 集成 |
| `pallets/grouprobot/consensus/src/mock.rs` | 230→247 | MockOrphanRewardClaimer |
| `runtime/src/configs/mod.rs` | 1661 | OrphanRewardClaimer 接线 |
| `grouprobot/src/chain/queries.rs` | 684→702 | 链下奖励查询 |
| `grouprobot/src/chain/transactions.rs` | 227→239 | 链下奖励领取 |

## 跨 Pallet 互通分析

### 奖励数据流

```
consensus::on_era_end
  ├─ subscription::settle_era() → subscription_income (90% 直接转运营者, 10% 国库)
  ├─ inflation → rewards::distribute_and_record_era (铸币到 RewardPool, 按权重分配)
  └─ rewards::prune_old_eras()

ads::settle_era_ads
  ├─ Currency::transfer(advertiser → RewardPool, node_share)
  └─ RewardAccruer::accrue_node_reward(node_id, amount)

operator → rewards::claim_rewards(node_id)
  └─ Currency::transfer(RewardPool → operator, pending)
```

### 关键设计决策

1. **subscription node_share (90%)**: 由 subscription pallet 直接转给运营者，不经 RewardPool
2. **inflation**: 由 rewards pallet 铸币到 RewardPool，按节点权重分配
3. **ads node_share**: 由 ads pallet 转入 RewardPool，通过 `accrue_node_reward` 累加到节点
4. **total_pool = inflation only**: 仅通胀参与权重分配，订阅直发

## 发现与修复

### H1 [High] — `prune_old_era_rewards` 每次仅删除 1 条，追赶速度不足

**问题**: 原实现每次调用只删除一条 EraRewards 记录。若因故障/升级跳过了多个 Era 的清理，cursor 需要数百次调用才能追赶 (每 Era 调一次 = 需 365+ Era 才能追上)。在追赶期间，过期存储持续累积。

**修复**: 改为有界循环，每次最多清理 `MAX_PRUNE_PER_CALL = 10` 条。正常运行时每 Era 仅需删 1 条；若有积压，约 37 个 Era 即可清理 365 条积压。

**文件**: `pallets/grouprobot/rewards/src/lib.rs`

### H2 [High] — `claim_rewards` 转账失败时奖励丢失

**问题**: 原代码先清除 `NodePendingRewards`，再执行 `Currency::transfer`。若 RewardPool 余额不足导致转账失败，`NodePendingRewards` 已被清空，奖励永久丢失。

**修复**: 
1. 提取 `do_claim_rewards` 内部函数，先转账后清存储
2. 转账失败返回新错误 `RewardPoolInsufficient`（而非泛化的 transfer error）
3. `NodePendingRewards` 仅在转账成功后才 remove

**文件**: `pallets/grouprobot/rewards/src/lib.rs`

### H3 [High] — `finalize_exit` 删除节点后奖励永久不可领

**问题**: `consensus::finalize_exit` 删除 `Nodes` 存储项（`Nodes::remove`），使 `node_operator()` 返回 `None`。之后 `claim_rewards` 的 `node_operator(&node_id).ok_or(NodeNotFound)?` 永远失败，该节点的 `NodePendingRewards` 变成孤儿数据——既不能领取，也不会被清理。

**修复**: 
1. 在 primitives 新增 `OrphanRewardClaimer<AccountId>` trait
2. rewards pallet 实现该 trait，提供 `try_claim_orphan_rewards` (best-effort: 成功则转账+清存储+事件，失败则 warn log 但不阻断退出)
3. consensus `finalize_exit` 在 `Nodes::remove` 之前调用 `OrphanRewardClaimer::try_claim_orphan_rewards`
4. Runtime 接线: `type OrphanRewardClaimer = pallet_grouprobot_rewards::Pallet<Runtime>`

**文件**: 
- `pallets/grouprobot/primitives/src/lib.rs` (新 trait)
- `pallets/grouprobot/rewards/src/lib.rs` (impl)
- `pallets/grouprobot/consensus/src/lib.rs` (Config + finalize_exit)
- `pallets/grouprobot/consensus/src/mock.rs` (MockOrphanRewardClaimer)
- `runtime/src/configs/mod.rs` (接线)

### M1 [Medium] — `accrue_node_reward` 无事件，链上无审计轨迹

**问题**: ads pallet 通过 `RewardAccruer::accrue_node_reward` 累加奖励到节点，但该操作不发出任何事件。链上索引器/浏览器无法追踪奖励累加来源和时间。

**修复**: 新增 `RewardAccrued { node_id, amount }` 事件，每次累加时发出。

**文件**: `pallets/grouprobot/rewards/src/lib.rs`

### L1 [Low] — 链下代码无奖励查询和领取功能

**问题**: `grouprobot/src/chain/queries.rs` 和 `transactions.rs` 完全没有奖励相关的查询或交易。节点运营者无法通过 Bot 查询待领奖励或触发领取。

**修复**: 
- `queries.rs`: 新增 `query_pending_rewards(node_id)` 和 `query_total_earned(node_id)`
- `transactions.rs`: 新增 `claim_rewards(node_id)`

**文件**: `grouprobot/src/chain/queries.rs`, `grouprobot/src/chain/transactions.rs`

### L2 [Low] — `deposit_creating` 返回值 `let _ =` 掩盖意图

**问题**: `deposit_creating` 返回 `PositiveImbalance`，`let _ =` 丢弃是正确行为（drop 时自增 TotalIssuance），但代码意图不清晰。

**修复**: 改为 `let _imbalance =` + 注释说明 drop 行为。

**文件**: `pallets/grouprobot/rewards/src/lib.rs`

## 记录但未修复

### M2 [Medium] — `EraRewardInfo.subscription_income` 语义混淆

`EraRewardInfo` 记录 `subscription_income` (全部订阅收入)，但 `total_distributed` 仅是 inflation 的加权分配结果。90% 的 subscription node_share 已由 subscription pallet 直接转给运营者，不在 `total_distributed` 中体现。链上分析工具可能误以为 `total_distributed` 包含了所有奖励。建议添加文档或重命名字段为 `inflation_distributed`。

### M3 [Medium] — subscription 转账失败静默吞掉 (已在 subscription 审计修复)

`settle_era_subscriptions` 中 `let _ = Currency::transfer(...)` 静默吞掉 node_share 和 treasury_share 转账失败。此问题已在 subscription pallet 审计中修复 (H1)。

### L3 [Low] — 硬编码 Weight

`claim_rewards` 使用硬编码 `Weight::from_parts(40_000_000, 6_000)`，无 WeightInfo/benchmark。需引入完整 benchmark 框架。

## 新增错误

| 错误 | 说明 |
|------|------|
| `RewardPoolInsufficient` | 奖励池余额不足以完成转账 |

## 新增事件

| 事件 | 说明 |
|------|------|
| `RewardAccrued { node_id, amount }` | 节点奖励已累加 |
| `OrphanRewardsClaimed { node_id, operator, amount }` | 节点退出时残留奖励已自动领取 |

## 新增 Trait

| Trait | 说明 |
|------|------|
| `OrphanRewardClaimer<AccountId>` | 节点退出时领取残留奖励 (primitives) |

## 新增测试 (8 个, 18 total, was 10)

| 测试 | 覆盖 |
|------|------|
| `h1_prune_batch_bounded_at_10` | H1: 批量清理上限 10 条/次 |
| `h2_claim_fails_insufficient_pool_preserves_pending` | H2: 转账失败不丢奖励 |
| `h3_try_claim_orphan_rewards_works` | H3: 孤儿奖励正常领取 |
| `h3_try_claim_orphan_no_pending_is_noop` | H3: 无待领为空操作 |
| `h3_try_claim_orphan_insufficient_pool_preserves_pending` | H3: 池不足时保留待领 |
| `h3_orphan_reward_claimer_trait_works` | H3: trait 接口测试 |
| `m1_accrue_node_reward_emits_event` | M1: 累加事件发出 |
| `prune_old_era_rewards_works` (更新) | H1: 批量清理 5 条 |

## 验证

```
cargo test -p pallet-grouprobot-rewards   → 18/18 ✅
cargo test -p pallet-grouprobot-consensus → 38/38 ✅
cargo check -p pallet-grouprobot-rewards  ✅
cargo check -p pallet-grouprobot-consensus ✅
cargo check -p pallet-grouprobot-primitives ✅
cargo check -p grouprobot (off-chain)     ✅
```

Runtime (`nexus-runtime`) 有预存在的 `pallet-entity-order` 编译错误 (非本次修改引起)。

## 修改文件汇总

| 文件 | 变更 |
|------|------|
| `pallets/grouprobot/rewards/src/lib.rs` | H1, H2, H3, M1, L2 修复 |
| `pallets/grouprobot/rewards/src/tests.rs` | +8 测试 (18 total) |
| `pallets/grouprobot/primitives/src/lib.rs` | 新增 OrphanRewardClaimer trait |
| `pallets/grouprobot/consensus/src/lib.rs` | H3: Config + finalize_exit |
| `pallets/grouprobot/consensus/src/mock.rs` | MockOrphanRewardClaimer |
| `runtime/src/configs/mod.rs` | OrphanRewardClaimer 接线 |
| `grouprobot/src/chain/queries.rs` | L1: 奖励查询 |
| `grouprobot/src/chain/transactions.rs` | L1: 奖励领取 |
