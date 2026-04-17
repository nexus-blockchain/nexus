# pallet-grouprobot-rewards

节点奖励累积、领取与 Era 奖励记录。从 consensus pallet 拆分而来。

## 概述

本 pallet 负责:

- **节点待领取奖励** — 由 Era 分配或广告收入累加
- **节点累计已领取** — 历史领取总额统计
- **Era 奖励记录** — 每个 Era 的分配明细 (订阅收入、广告收入、通胀铸币、总分配、国库份额)
- **孤儿奖励领取** — 节点退出时自动领取残留奖励 (best-effort)
- **批量领取** — 一次领取多个节点的奖励
- **自定义收款人** — 节点操作者可指定第三方地址接收奖励
- **Owner 分成** — Bot 所有者可设置奖励分成比例，自动拆分
- **奖励分配暂停/恢复** — Root 可暂停/恢复 Era 奖励分配
- **治理工具** — Root 可强制设置/削减待领取奖励、强制修剪 Era 记录

## 数据结构

### EraRewardInfo\<Balance\>

| 字段 | 类型 | 说明 |
|------|------|------|
| `subscription_income` | `Balance` | 本 Era 订阅费收入 |
| `ads_income` | `Balance` | 本 Era 广告收入 |
| `inflation_mint` | `Balance` | 本 Era 通胀铸币量 |
| `total_distributed` | `Balance` | 本 Era 实际分配给节点的总量 |
| `treasury_share` | `Balance` | 本 Era 国库份额 |
| `node_count` | `u32` | 参与分配的节点数 |

### NodeRewardSummary\<Balance\>

| 字段 | 类型 | 说明 |
|------|------|------|
| `pending` | `Balance` | 待领取奖励 |
| `total_earned` | `Balance` | 累计已领取 |

## Config

| 项 | 类型 | 说明 |
|----|------|------|
| `Currency` | `Currency<AccountId>` | 原生货币 |
| `NodeConsensus` | `NodeConsensusProvider<AccountId>` | 节点共识查询 (验证 claim 时的 operator 身份) |
| `BotRegistry` | `BotRegistryProvider<AccountId>` | Bot 注册查询 (验证 bot_owner 身份, 用于 owner 分成) |
| `RewardPoolAccount` | `Get<AccountId>` | 奖励池账户 (订阅费节点份额 + 通胀铸币存放于此) |
| `MaxEraHistory` | `Get<u64>` (constant) | EraRewards 保留窗口 (仅保留最近 N 个 Era 记录) |
| `MaxBatchClaim` | `Get<u32>` (constant) | 批量领取最大节点数 |

## Storage

| 名称 | 类型 | 说明 |
|------|------|------|
| `NodePendingRewards` | `StorageMap<NodeId, Balance>` | 节点待领取奖励 (ValueQuery, 默认 0) |
| `NodeTotalEarned` | `StorageMap<NodeId, Balance>` | 节点累计已领取 (ValueQuery, 默认 0) |
| `EraRewards` | `StorageMap<u64, EraRewardInfo>` | Era 奖励记录 (era → info) |
| `EraCleanupCursor` | `StorageValue<u64>` | 已清理到的 Era 编号 (ValueQuery, 默认 0) |
| `RewardRecipient` | `StorageMap<NodeId, AccountId>` | 自定义收款人 (OptionQuery) |
| `RewardSplitBps` | `StorageMap<BotIdHash, u16>` | Owner 分成基点 (ValueQuery, 默认 0, 最大 5000 = 50%) |
| `OwnerPendingRewards` | `StorageMap<BotIdHash, Balance>` | Owner 待领取奖励 (ValueQuery, 默认 0) |
| `OwnerTotalEarned` | `StorageMap<BotIdHash, Balance>` | Owner 累计已领取 (ValueQuery, 默认 0) |
| `DistributionPaused` | `StorageValue<bool>` | 分配暂停标志 (ValueQuery, 默认 false) |
| `NodeBotSplitBinding` | `StorageMap<NodeId, BotIdHash>` | 节点到 Bot 的分成绑定 (OptionQuery) |

## Extrinsics

| call_index | 名称 | Origin | 说明 |
|------------|------|--------|------|
| 0 | `claim_rewards` | Signed (operator) | 节点操作者领取待领取奖励。支持自定义收款人。 |
| 1 | `rescue_stranded_rewards` | Root | Root 救援已退出节点的滞留奖励。 |
| 2 | `batch_claim_rewards` | Signed (operator) | 批量领取多个节点的奖励 (最多 MaxBatchClaim 个)。跳过非本人节点。 |
| 3 | `set_reward_recipient` | Signed (operator) | 设置/清除节点的自定义收款人地址。 |
| 4 | `force_slash_pending_rewards` | Root | 强制削减节点待领取奖励。 |
| 5 | `set_reward_split` | Signed (bot_owner) | 设置 Bot 的 Owner 分成比例 (0-5000 bps)。 |
| 6 | `claim_owner_rewards` | Signed (bot_owner) | Bot Owner 领取分成奖励。 |
| 7 | `pause_distribution` | Root | 暂停 Era 奖励分配。 |
| 8 | `resume_distribution` | Root | 恢复 Era 奖励分配。 |
| 9 | `force_set_pending_rewards` | Root | 强制设置节点待领取奖励。 |
| 10 | `force_prune_era_rewards` | Root | 强制修剪指定 era 之前的 EraRewards 记录。 |

### claim_rewards 流程

1. 验证调用者为 `node_id` 的操作者 (`NodeConsensus::node_operator`)
2. 检查 `NodePendingRewards > 0`
3. 确定收款人: `RewardRecipient` 自定义地址 或 操作者本人
4. 从 `RewardPoolAccount` 向收款人转账
5. 转账成功后: 清除 `NodePendingRewards`，累加 `NodeTotalEarned`
6. 发出 `RewardsClaimed` 事件 (含 `total_earned_after`)

### batch_claim_rewards 流程

1. 检查列表非空且不超过 `MaxBatchClaim`
2. 遍历 node_ids, 跳过非本人节点 (operator 不匹配) 和无 pending 的节点
3. 对每个有效节点调用 `do_claim_rewards`
4. 至少成功领取 1 个节点, 否则返回 `NoPendingRewards`
5. 发出 `BatchRewardsClaimed` 事件

### Owner 分成流程

1. Bot Owner 通过 `set_reward_split` 设置分成比例 (bps, 0-5000)
2. 外部通过 `bind_node_bot_split` / `unbind_node_bot_split` 绑定节点到 Bot
3. `accrue_node_reward` 时自动拆分: owner_share = amount * bps / 10000
4. owner_share 累加到 `OwnerPendingRewards`, 剩余累加到 `NodePendingRewards`
5. Bot Owner 通过 `claim_owner_rewards` 领取分成

## Events

| 事件 | 字段 | 说明 |
|------|------|------|
| `RewardsClaimed` | `node_id, recipient, amount, total_earned_after` | 节点奖励已领取 |
| `EraCompleted` | `era, total_distributed, era_info` | Era 奖励分配完成 (含完整 era 信息) |
| `RewardAccrued` | `node_id, amount` | 节点奖励已累加 |
| `OrphanRewardsClaimed` | `node_id, operator, amount` | 孤儿奖励已自动领取 |
| `OrphanRewardClaimFailed` | `node_id, amount` | 孤儿奖励领取失败 |
| `BatchRewardsClaimed` | `operator, total_amount, node_count` | 批量领取完成 |
| `RewardRecipientSet` | `node_id, recipient` | 自定义收款人已设置 |
| `RewardRecipientCleared` | `node_id` | 自定义收款人已清除 |
| `PendingRewardsSlashed` | `node_id, amount` | 待领取奖励被削减 |
| `RewardSplitSet` | `bot_id_hash, owner_bps` | Owner 分成比例已设置 |
| `OwnerRewardsClaimed` | `bot_id_hash, owner, amount` | Owner 分成已领取 |
| `DistributionPausedEvent` | (无) | 分配已暂停 |
| `DistributionResumedEvent` | (无) | 分配已恢复 |
| `PendingRewardsForceSet` | `node_id, old_amount, new_amount` | 待领取奖励被强制设置 |
| `EraRewardsForcePruned` | `from_era, to_era` | Era 记录被强制修剪 |

## Errors

| 错误 | 说明 |
|------|------|
| `NodeNotFound` | 节点不存在 |
| `NotOperator` | 不是节点操作者 |
| `NoPendingRewards` | 无待领取奖励 |
| `RewardPoolInsufficient` | 奖励池余额不足 |
| `NodeStillActive` | 节点仍活跃 (rescue_stranded_rewards 专用) |
| `EmptyBatchList` | 批量列表为空 |
| `TooManyNodes` | 超过 MaxBatchClaim 限制 |
| `InvalidSplitBps` | 分成比例超过 5000 (50%) |
| `NotBotOwner` | 不是 Bot 所有者 |
| `NoOwnerPendingRewards` | Owner 无待领取分成 |
| `DistributionIsPaused` | 分配已暂停 (pause 时重复调用) |
| `DistributionNotPaused` | 分配未暂停 (resume 时) |
| `SlashExceedsPending` | 削减金额超过待领取 |
| `NothingToPrune` | 无可修剪的 Era 记录 |

## 内部函数

| 函数 | 说明 |
|------|------|
| `do_claim_rewards(node_id, recipient)` | 内部领取逻辑 — 先转账后清存储, 支持自定义收款人 |
| `try_claim_orphan_rewards(node_id, operator)` | best-effort 自动领取残留奖励 |
| `distribute_and_record_era(...)` | 按权重分配奖励 + 铸币通胀 + Owner 拆分 + 记录 Era 信息 |
| `prune_old_era_rewards(current_era)` | 清理过期 EraRewards，每次最多 10 条 |
| `accrue_with_split(node_id, amount)` | 按 Owner 分成比例拆分奖励累加 |
| `bind_node_bot_split(node_id, bot_id_hash)` | 绑定节点到 Bot (分成用) |
| `unbind_node_bot_split(node_id)` | 解除节点分成绑定 |

### distribute_and_record_era 流程

1. 检查 `DistributionPaused` → 若暂停则返回 0
2. 检查 era 去重 (`EraRewards::contains_key`)
3. 将通胀部分 `deposit_creating` 铸币到奖励池
4. 按 `node_weights` 权重分配 `total_pool` 到各节点 (含 Owner 拆分)
5. 写入 `EraRewards` 记录 (含 `ads_income` 字段)
6. 发出 `EraCompleted` 事件 (含完整 `era_info`)

## 查询辅助函数

| 函数 | 返回类型 | 说明 |
|------|---------|------|
| `pending_rewards(node_id)` | `Balance` | 节点待领取奖励 |
| `node_reward_summary(node_id)` | `NodeRewardSummary` | 节点奖励汇总 (pending + total_earned) |
| `reward_pool_balance()` | `Balance` | 奖励池当前余额 |
| `owner_pending(bot_id_hash)` | `Balance` | Owner 待领取分成 |

## Trait 实现

### RewardAccruer

```rust
fn accrue_node_reward(node_id: &NodeId, amount: u128);
```

向节点累加待领取奖励 (含 Owner 拆分)，发出 `RewardAccrued` 事件。

### OrphanRewardClaimer\<AccountId\>

```rust
fn try_claim_orphan_rewards(node_id: &NodeId, operator: &AccountId);
```

节点退出时由 consensus pallet 的 `finalize_exit` 调用。

### EraRewardDistributor

```rust
fn distribute_and_record(era, total_pool, subscription_income, ads_income, inflation, treasury_share, node_weights, node_count) -> u128;
fn prune_old_eras(current_era);
```

由 consensus pallet 的 `on_era_end` 调用。

## integrity_test

运行时完整性检查:
- `MaxEraHistory > 0`
- `MaxBatchClaim > 0`

## 跨 Pallet 依赖

```
consensus ──on_era_end──► rewards (EraRewardDistributor)
consensus ──finalize_exit──► rewards (OrphanRewardClaimer)
ads ──settle_era_ads──► rewards (RewardAccruer)
rewards ──claim──► NodeConsensus (验证 operator)
rewards ──owner_split──► BotRegistry (验证 bot_owner)
```

## 测试

共 60 个用户测试 (62 total, 含 2 auto-gen):

### 基础功能
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
- `m1_r2_distribute_emits_reward_accrued_per_node` — Era 分配逐节点事件
- `m2_r2_rescue_stranded_rewards_works` — Root 救援滞留奖励
- `m2_r2_rescue_rejects_non_root` / `m2_r2_rescue_rejects_active_node` / `m2_r2_rescue_rejects_no_pending`
- `distribute_zero_inflation_no_mint` — 零通胀不铸币
- `distribute_empty_weights_zero_distributed` — 空权重零分配
- `accrue_zero_amount_is_noop` — 零金额无操作
- `m1_r3_claim_event_includes_recipient` — 领取事件含收款人 + total_earned_after
- `m1_r3_rescue_event_includes_recipient` — 救援事件含收款人 + total_earned_after
- `m2_r3_orphan_claim_failure_emits_event` — 孤儿领取失败事件
- `m3_r3_distribute_era_skips_duplicate` — 重复 era 被跳过

### P0: 批量领取
- `batch_claim_rewards_works` — 批量领取, 跳过非本人节点
- `batch_claim_empty_list_fails` — 空列表拒绝
- `batch_claim_too_many_nodes_fails` — 超限拒绝
- `batch_claim_no_pending_fails` — 无可领取拒绝
- `batch_claim_respects_custom_recipient` — 批量领取使用自定义收款人

### P0: 自定义收款人
- `set_reward_recipient_works` — 设置收款人 + 验证领取到自定义地址
- `clear_reward_recipient_works` — 清除收款人
- `set_reward_recipient_not_operator_fails` — 非操作者拒绝

### P0: 强制削减
- `force_slash_pending_rewards_works` — 正常削减
- `force_slash_exceeds_pending_fails` — 超额拒绝
- `force_slash_rejects_non_root` — 非 Root 拒绝

### P1: Owner 分成
- `set_reward_split_works` — 设置分成比例
- `set_reward_split_invalid_bps_fails` — 超限拒绝
- `set_reward_split_not_owner_fails` — 非 Owner 拒绝
- `owner_split_accrues_correctly` — 累加时正确拆分 (20% → owner, 80% → operator)
- `owner_split_zero_bps_no_split` — 0 bps 不拆分
- `claim_owner_rewards_works` — Owner 领取分成
- `claim_owner_rewards_no_pending_fails` — 无 pending 拒绝
- `claim_owner_rewards_not_owner_fails` — 非 Owner 拒绝
- `era_distribution_with_owner_split` — Era 分配含 Owner 拆分

### P1: 暂停/恢复分配
- `pause_distribution_works` — 暂停成功
- `pause_already_paused_fails` — 重复暂停拒绝
- `resume_distribution_works` — 恢复成功
- `resume_not_paused_fails` — 未暂停时恢复拒绝
- `distribution_skipped_when_paused` — 暂停后 distribute 返回 0

### P1: 强制设置
- `force_set_pending_rewards_works` — 强制设置
- `force_set_pending_rejects_non_root` — 非 Root 拒绝

### P2: 强制修剪
- `force_prune_era_rewards_works` — 强制修剪
- `force_prune_nothing_fails` — 无可修剪拒绝
- `force_prune_capped_at_max` — 单次上限 MAX_FORCE_PRUNE=100 验证

### P1: 查询辅助
- `query_helpers_work` — pending_rewards / node_reward_summary / reward_pool_balance / owner_pending

### 绑定/解绑
- `bind_unbind_node_bot_split_works` — bind + unbind 验证

## 审计历史

| 轮次 | 修复项 | 说明 |
|------|--------|------|
| Round 1 | H1 | `prune_old_era_rewards` 每次仅删 1 条 → 有界循环 MAX_PRUNE_PER_CALL=10 |
| Round 1 | H2 | `claim_rewards` 先清存储后转账 → 先转账后清存储，新增 RewardPoolInsufficient 错误 |
| Round 1 | H3 | `finalize_exit` 删除 Nodes 后奖励不可领 → OrphanRewardClaimer trait + try_claim_orphan_rewards |
| Round 1 | M1 | `accrue_node_reward` 无事件 → 新增 RewardAccrued 事件 |
| Round 1 | L1 | 链下代码无奖励查询/领取 → 新增 off-chain query/claim 函数 |
| Round 1 | L2 | `deposit_creating` 返回值 `let _` → `let _imbalance` + 注释 |
| Round 2 | M1-R2 | `distribute_and_record_era` 逐节点发出 `RewardAccrued` 事件 |
| Round 2 | M2-R2 | 新增 `rescue_stranded_rewards` Root extrinsic (call_index 1) |
| Round 2 | L1-R2 | 移除死依赖 `sp-core` |
| Round 2 | L2-R2 | `try-runtime` feature 补充传播 |
| Round 3 | M1-R3 | `RewardsClaimed` 事件添加 `recipient` 字段 |
| Round 3 | M2-R3 | 新增 `OrphanRewardClaimFailed` 事件 |
| Round 3 | M3-R3 | `distribute_and_record_era` 添加 era 去重保护 |
| Round 4 | P0 | 批量领取 `batch_claim_rewards` (call_index 2) |
| Round 4 | P0 | 自定义收款人 `set_reward_recipient` (call_index 3) |
| Round 4 | P0 | 强制削减 `force_slash_pending_rewards` (call_index 4) |
| Round 4 | P1 | Owner 分成 `set_reward_split` + `claim_owner_rewards` (call_index 5,6) |
| Round 4 | P1 | 暂停/恢复分配 `pause_distribution` / `resume_distribution` (call_index 7,8) |
| Round 4 | P1 | 强制设置 `force_set_pending_rewards` (call_index 9) |
| Round 4 | P1 | 查询辅助函数 + integrity_test |
| Round 4 | P1 | `EraRewardInfo` 新增 `ads_income` 字段, 修复语义混淆 |
| Round 4 | P2 | 强制修剪 `force_prune_era_rewards` (call_index 10) |
| Round 4 | P2 | `RewardsClaimed` 新增 `total_earned_after`, `EraCompleted` 新增 `era_info` |
| Round 4 | P2 | `NodeBotSplitBinding` + `bind/unbind_node_bot_split` 辅助函数 |
| Round 5 | L3-fix | 新增 `WeightInfo` trait + `weights.rs` + `benchmarking.rs`, 替换全部硬编码 Weight |
| Round 5 | L4-fix | `force_prune_era_rewards` 新增 `MAX_FORCE_PRUNE=100` 上界, 防止无界循环 |
| Round 5 | L5-fix | Clippy 清理: 修复 14 个 `needless_borrows_for_generic_args` + 1 个 `too_many_arguments` |

### 记录但未修复

| ID | 严重级别 | 说明 |
|----|---------|------|
| M3-R2 | Medium | `NodeTotalEarned` 无清理机制 — 退出节点历史统计永久保留 (设计决策, 48 bytes/node) |
| L3-R2 | Low | `distribute_and_record_era` 整数除法截断灰尘滞留 RewardPool (数学特性) |

## License

Apache-2.0
