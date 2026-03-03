# pallet-grouprobot-rewards

节点奖励累积、领取与 Era 奖励记录。从 consensus pallet 拆分而来。

## 概述

本 pallet 负责:

- **节点待领取奖励** — 由 Era 分配或广告收入累加
- **节点累计已领取** — 历史领取总额统计
- **Era 奖励记录** — 每个 Era 的分配明细 (订阅收入、通胀铸币、总分配、国库份额)
- **孤儿奖励领取** — 节点退出时自动领取残留奖励 (best-effort)

## 数据结构

### EraRewardInfo\<Balance\>

| 字段 | 类型 | 说明 |
|------|------|------|
| `subscription_income` | `Balance` | 本 Era 订阅费收入 |
| `inflation_mint` | `Balance` | 本 Era 通胀铸币量 |
| `total_distributed` | `Balance` | 本 Era 实际分配给节点的总量 |
| `treasury_share` | `Balance` | 本 Era 国库份额 |
| `node_count` | `u32` | 参与分配的节点数 |

## Config

| 项 | 类型 | 说明 |
|----|------|------|
| `Currency` | `Currency<AccountId>` | 原生货币 |
| `NodeConsensus` | `NodeConsensusProvider<AccountId>` | 节点共识查询 (验证 claim 时的 operator 身份) |
| `RewardPoolAccount` | `Get<AccountId>` | 奖励池账户 (订阅费节点份额 + 通胀铸币存放于此) |
| `MaxEraHistory` | `Get<u64>` (constant) | EraRewards 保留窗口 (仅保留最近 N 个 Era 记录) |

## Storage

| 名称 | 类型 | 说明 |
|------|------|------|
| `NodePendingRewards` | `StorageMap<NodeId, Balance>` | 节点待领取奖励 (ValueQuery, 默认 0) |
| `NodeTotalEarned` | `StorageMap<NodeId, Balance>` | 节点累计已领取 (ValueQuery, 默认 0) |
| `EraRewards` | `StorageMap<u64, EraRewardInfo>` | Era 奖励记录 (era → info) |
| `EraCleanupCursor` | `StorageValue<u64>` | 已清理到的 Era 编号 (ValueQuery, 默认 0) |

## Extrinsics

| call_index | 名称 | 签名 | 说明 |
|------------|------|------|------|
| 0 | `claim_rewards` | `origin, node_id: NodeId` | 节点操作者领取待领取奖励。先转账后清存储，转账失败不丢奖励。 |
| 1 | `rescue_stranded_rewards` | `origin (Root), node_id: NodeId, recipient: AccountId` | Root 救援已退出节点的滞留奖励 (orphan claim 失败后的恢复手段)。仅允许 node_operator 返回 None 的节点。 |

### claim_rewards 流程

1. 验证调用者为 `node_id` 的操作者 (`NodeConsensus::node_operator`)
2. 检查 `NodePendingRewards > 0`
3. 从 `RewardPoolAccount` 向操作者转账
4. 转账成功后: 清除 `NodePendingRewards`，累加 `NodeTotalEarned`
5. 发出 `RewardsClaimed` 事件

## Events

| 事件 | 字段 | 说明 |
|------|------|------|
| `RewardsClaimed` | `node_id, recipient, amount` | 节点奖励已领取 (含收款人地址) |
| `EraCompleted` | `era, total_distributed` | Era 奖励分配完成 |
| `RewardAccrued` | `node_id, amount` | 节点奖励已累加 (ads 或 Era 分配) |
| `OrphanRewardsClaimed` | `node_id, operator, amount` | 节点退出时残留奖励已自动领取 |
| `OrphanRewardClaimFailed` | `node_id, amount` | 孤儿奖励领取失败 (奖励池不足, 需 Root rescue) |

## Errors

| 错误 | 说明 |
|------|------|
| `NodeNotFound` | 节点不存在 |
| `NotOperator` | 不是节点操作者 |
| `NoPendingRewards` | 无待领取奖励 |
| `RewardPoolInsufficient` | 奖励池余额不足 |
| `NodeStillActive` | 节点仍活跃, 应使用 claim_rewards (rescue_stranded_rewards 专用) |

## 内部函数

| 函数 | 说明 |
|------|------|
| `do_claim_rewards(node_id, recipient)` | 内部领取逻辑 — 先转账后清存储 |
| `try_claim_orphan_rewards(node_id, operator)` | 节点退出时 best-effort 自动领取残留奖励，失败仅 log::warn |
| `distribute_and_record_era(...)` | 按权重向节点分配奖励 + 铸币通胀 + 记录 Era 信息 |
| `prune_old_era_rewards(current_era)` | 清理过期 EraRewards，每次最多清理 10 条 (有界) |

### distribute_and_record_era 流程

1. 将通胀部分 `deposit_creating` 铸币到奖励池
2. 按 `node_weights` 权重比例分配 `total_pool` 到各节点的 `NodePendingRewards`
3. 写入 `EraRewards` 记录
4. 发出 `EraCompleted` 事件

## Trait 实现

### RewardAccruer

```rust
fn accrue_node_reward(node_id: &NodeId, amount: u128);
```

向节点累加待领取奖励，发出 `RewardAccrued` 事件。供 ads pallet 等外部模块写入节点奖励。

### OrphanRewardClaimer\<AccountId\>

```rust
fn try_claim_orphan_rewards(node_id: &NodeId, operator: &AccountId);
```

节点退出时由 consensus pallet 的 `finalize_exit` 调用，best-effort 领取残留奖励。

### EraRewardDistributor

```rust
fn distribute_and_record(era, total_pool, subscription_income, inflation, treasury_share, node_weights, node_count) -> u128;
fn prune_old_eras(current_era);
```

由 consensus pallet 的 `on_era_end` 调用，执行 Era 奖励分配和历史清理。

## 跨 Pallet 依赖

```
consensus ──on_era_end──► rewards (EraRewardDistributor)
consensus ──finalize_exit──► rewards (OrphanRewardClaimer)
ads ──settle_era_ads──► rewards (RewardAccruer)
rewards ──claim──► NodeConsensus (验证 operator)
```

## 测试

共 28 个用户测试 (30 total, 含 2 auto-gen):

- `claim_rewards_works` — 正常领取
- `claim_rewards_fails_no_pending` — 无待领取奖励
- `claim_rewards_fails_not_operator` — 节点不存在
- `claim_rewards_fails_wrong_operator` — 非该节点操作者
- `accrue_node_reward_works` — 累加奖励
- `distribute_and_record_era_works` — 等权重 Era 分配
- `distribute_with_unequal_weights` — 不等权重分配
- `era_reward_distributor_trait_works` — EraRewardDistributor trait 调用
- `prune_old_era_rewards_works` — 正常清理
- `h1_prune_batch_bounded_at_10` — 每次最多清理 10 条
- `h2_claim_fails_insufficient_pool_preserves_pending` — 奖励池不足时保留 pending
- `h3_try_claim_orphan_rewards_works` — 孤儿奖励正常领取
- `h3_try_claim_orphan_no_pending_is_noop` — 无待领取时无操作
- `h3_try_claim_orphan_insufficient_pool_preserves_pending` — 孤儿奖励转账失败保留 pending
- `h3_orphan_reward_claimer_trait_works` — OrphanRewardClaimer trait 调用
- `m1_accrue_node_reward_emits_event` — 累加奖励发出事件
- `m1_r2_distribute_emits_reward_accrued_per_node` — Era 分配逐节点 RewardAccrued 事件
- `m2_r2_rescue_stranded_rewards_works` — Root 救援滞留奖励
- `m2_r2_rescue_rejects_non_root` — 非 Root 拒绝
- `m2_r2_rescue_rejects_active_node` — 活跃节点拒绝 rescue
- `m2_r2_rescue_rejects_no_pending` — 无滞留奖励拒绝
- `distribute_zero_inflation_no_mint` — 零通胀不铸币
- `distribute_empty_weights_zero_distributed` — 空权重零分配
- `accrue_zero_amount_is_noop` — 零金额无操作无事件
- `m1_r3_claim_event_includes_recipient` — 领取事件含收款人
- `m1_r3_rescue_event_includes_recipient` — 救援事件含收款人
- `m2_r3_orphan_claim_failure_emits_event` — 孤儿领取失败发射事件
- `m3_r3_distribute_era_skips_duplicate` — 重复 era 分配被跳过

## 审计历史

| 轮次 | 修复项 | 说明 |
|------|--------|------|
| Round 1 | H1 | `prune_old_era_rewards` 每次仅删 1 条 → 有界循环 MAX_PRUNE_PER_CALL=10 |
| Round 1 | H2 | `claim_rewards` 先清存储后转账 → 先转账后清存储，新增 RewardPoolInsufficient 错误 |
| Round 1 | H3 | `finalize_exit` 删除 Nodes 后奖励不可领 → OrphanRewardClaimer trait + try_claim_orphan_rewards |
| Round 1 | M1 | `accrue_node_reward` 无事件 → 新增 RewardAccrued 事件 |
| Round 1 | L1 | 链下代码无奖励查询/领取 → 新增 off-chain query/claim 函数 |
| Round 1 | L2 | `deposit_creating` 返回值 `let _` → `let _imbalance` + 注释 |
| Round 2 | M1-R2 | `distribute_and_record_era` 逐节点发出 `RewardAccrued` 事件, 与 ads 路径一致 |
| Round 2 | M2-R2 | 新增 `rescue_stranded_rewards` Root extrinsic (call_index 1) + `NodeStillActive` 错误 |
| Round 2 | L1-R2 | 移除死依赖 `sp-core` |
| Round 2 | L2-R2 | `try-runtime` feature 补充 `sp-runtime/try-runtime` + `frame-system/try-runtime` 传播 |
| Round 3 | M1-R3 | `RewardsClaimed` 事件缺 `recipient` 字段 → 添加 `recipient: AccountId` 字段, 链上可追踪 rescue 收款人 |
| Round 3 | M2-R3 | `try_claim_orphan_rewards` 失败时无链上事件 → 新增 `OrphanRewardClaimFailed` 事件 |
| Round 3 | M3-R3 | `distribute_and_record_era` 无 era 去重保护 → 添加 `EraRewards::contains_key` 前置检查, 防止重复铸币+重复分配 |

### 记录但未修复

| ID | 严重级别 | 说明 |
|----|---------|------|
| M2 | Medium | `EraRewardInfo.subscription_income` 语义混淆 (记录总收入, 但 node_share 已由 subscription 直接分配) |
| M3-R2 | Medium | `NodeTotalEarned` 无清理机制 — 退出节点历史统计永久保留 (设计决策, 48 bytes/node) |
| L3 | Low | 硬编码 Weight, 无 WeightInfo trait |
| L3-R2 | Low | `distribute_and_record_era` 整数除法截断灰尘滞留 RewardPool (数学特性) |

## License

Apache-2.0
