# pallet-grouprobot-consensus

> 路径：`pallets/grouprobot/consensus/`

GroupRobot 节点共识模块 — 节点质押准入、TEE 可信计算验证、Era 经济编排、Equivocation 举报惩罚、消息序列去重。

> **注意**: 订阅管理和奖励领取已拆分到独立模块 `pallet-grouprobot-subscription` 和 `pallet-grouprobot-rewards`。

---

## 目录

- [设计理念](#设计理念)
- [架构概览](#架构概览)
- [Extrinsics](#extrinsics)
- [存储](#存储)
- [事件](#事件)
- [错误](#错误)
- [核心类型](#核心类型)
- [Era 经济模型](#era-经济模型)
- [Config 配置](#config-配置)
- [Hooks](#hooks)
- [Trait 实现](#trait-实现)
- [公共查询方法](#公共查询方法)
- [内部函数](#内部函数)
- [测试覆盖](#测试覆盖)
- [相关模块](#相关模块)

---

## 设计理念

### 节点生命周期

```
                  register_node
                       │
                       ▼
         ┌──────── Active ◄──────────┐
         │             │             │
   force_suspend   request_exit   reinstate_node
         │             │         force_reinstate
         ▼             ▼             │
    Suspended ──►  Exiting      Suspended
         │             │
   request_exit   finalize_exit
         │             │
         ▼             ▼
      Exiting      [已移除]
         │
    finalize_exit
         │
         ▼
      [已移除]
```

- **质押准入** — 注册时锁定最低质押（`MinStake`），退出需经冷却期
- **质押灵活性** — 运营者可随时补充质押（`increase_stake`），Slash 后可补质押恢复
- **TEE 专属奖励** — 仅通过 Registry 证明验证的 TEE 节点参与 Era 奖励分配，非 TEE 节点权重为 0
- **TEE 自动降级** — Era 结束时检查绑定 Bot 的证明有效性，过期自动降级并清理 `NodeBotBinding`
- **Bot 绑定灵活性** — 运营者可主动解绑 Bot（`unbind_bot`）后重新绑定其他 Bot

### Equivocation 惩罚机制

- 同序列双签举报（ed25519 签名链上验证）→ Root 审核执行 Slash
- Slash 罚没质押百分比 + 暂停节点 + 重置 TEE 状态 + 清理 Bot 绑定
- 举报者按可配置百分比（`ReporterRewardPct`，bps）获得奖励

### 消息去重

- `ProcessedSequences` 存储已处理消息序列号，防止重复处理
- `on_initialize` 游标式自动清理过期记录，避免存储膨胀

### Era 编排

- `on_era_end` 委托外部模块完成完整结算流程：
  SubscriptionSettler → TEE 降级检查 → RewardDistributor → PeerUptimeRecorder → Era 推进

---

## 架构概览

```
┌─────────────────────────────────────────────────┐
│           pallet-grouprobot-consensus           │
│                                                 │
│  ┌──────────┐ ┌────────────┐ ┌──────────────┐  │
│  │ 节点管理  │ │ Equivocation│ │  消息去重    │  │
│  │ 注册/退出 │ │  举报/Slash │ │ Sequence TTL │  │
│  │ 质押/恢复 │ │  清理/奖励  │ │  自动清理    │  │
│  └──────────┘ └────────────┘ └──────────────┘  │
│                                                 │
│  ┌──────────────── Era 编排 ─────────────────┐  │
│  │ on_era_end:                               │  │
│  │  1. SubscriptionSettler::settle_era()     │  │
│  │  2. TEE 证明有效性检查 + 自动降级         │  │
│  │  3. compute_node_weight() → 权重向量      │  │
│  │  4. RewardDistributor::distribute()       │  │
│  │  5. PeerUptimeRecorder::record()          │  │
│  │  6. Era 推进 + prune_old_eras()           │  │
│  └───────────────────────────────────────────┘  │
│                                                 │
│  依赖 Trait:                                    │
│  ├── BotRegistryProvider (Bot/TEE 查询)         │
│  ├── SubscriptionSettler (订阅费结算)           │
│  ├── SubscriptionProvider (Tier 层级查询)       │
│  ├── EraRewardDistributor (奖励分配)            │
│  ├── PeerUptimeRecorder (心跳快照)              │
│  └── OrphanRewardClaimer (退出领取残留奖励)     │
│                                                 │
│  对外提供:                                      │
│  └── NodeConsensusProvider (节点状态查询)        │
└─────────────────────────────────────────────────┘
```

---

## Extrinsics

### 节点管理（Signed）

| call_index | 方法 | 说明 |
|:---:|------|------|
| 0 | `register_node(node_id, stake)` | 注册节点 + 质押锁定（reserve），加入 ActiveNodeList |
| 1 | `request_exit(node_id)` | 申请退出（Active/Suspended 均可），进入冷却期 |
| 2 | `finalize_exit(node_id)` | 冷却期满后完成退出：领取残留奖励 → 退还质押 → 清理所有存储 |
| 11 | `verify_node_tee(node_id, bot_id_hash)` | 通过 BotRegistry 证明验证 TEE 状态，仅 Active 节点可调用 |
| 14 | `increase_stake(node_id, amount)` | 运营者补充质押（Active/Suspended 均可） |
| 15 | `reinstate_node(node_id)` | 运营者恢复 Suspended 节点为 Active（需 stake ≥ MinStake） |
| 18 | `unbind_bot(node_id)` | 解除 Bot 绑定 + 重置 TEE 状态为 false |
| 19 | `replace_operator(node_id, new_operator)` | 转移运营权：unreserve 旧操作者 → reserve 新操作者，重置 TEE 状态 |

### Equivocation（混合 Origin）

| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 3 | `report_equivocation(node_id, seq, msg_hash_a, sig_a, msg_hash_b, sig_b)` | Signed | 举报双签，ed25519 链上验签（NodeId = 公钥），两组消息必须不同 |
| 4 | `slash_equivocation(node_id, seq)` | Root | 执行 Slash：罚没质押 + 暂停节点 + 重置 TEE + 举报者奖励 |
| 13 | `cleanup_resolved_equivocation(node_id, seq)` | Signed | 清理已解决的单条记录 |
| 23 | `batch_cleanup_equivocations(items)` | Signed | 批量清理已解决记录（BoundedVec，上限 MaxActiveNodes） |

### 消息去重（Signed）

| call_index | 方法 | 说明 |
|:---:|------|------|
| 10 | `mark_sequence_processed(bot_id_hash, seq)` | Tier gate（Free 不可用） + Bot Owner/Operator 授权，幂等（重复返回 SequenceDuplicate 事件） |

### 治理操作（Root）

| call_index | 方法 | 说明 |
|:---:|------|------|
| 12 | `set_tee_reward_params(tee_multiplier, sgx_bonus)` | TEE 奖励参数（bps），tee_multiplier ≤ 50000，sgx_bonus ≤ 10000 |
| 16 | `force_suspend_node(node_id)` | 暂停 Active 节点 + 重置 TEE + 清理绑定 |
| 17 | `force_remove_node(node_id)` | 强制移除节点 + 全额 Slash 质押 + 清理所有存储 |
| 20 | `set_slash_percentage(new_pct)` | 运行时 Slash 百分比（1-100，0=恢复 Config 默认值） |
| 21 | `set_reporter_reward_pct(pct)` | 举报者奖励百分比（bps，0-5000，0=关闭奖励） |
| 22 | `force_reinstate_node(node_id)` | 强制恢复 Suspended 节点（无需质押校验） |
| 24 | `force_era_end()` | 手动触发 Era 结束结算（无需等待 EraLength） |

---

## 存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Nodes` | `Map<NodeId → ProjectNode>` | 节点完整信息 |
| `OperatorNodes` | `Map<AccountId → NodeId>` | 操作者到节点的反向索引（1:1） |
| `ActiveNodeList` | `BoundedVec<NodeId, MaxActiveNodes>` | 活跃节点列表（参与 Era 分配） |
| `ExitRequests` | `Map<NodeId → BlockNumber>` | 退出冷却起始区块 |
| `ProcessedSequences` | `DoubleMap<BotIdHash, u64 → BlockNumber>` | 已处理消息序列（含记录时间，用于 TTL 过期） |
| `NodeBotBinding` | `Map<NodeId → BotIdHash>` | 节点与 Bot 的 TEE 绑定关系 |
| `EquivocationRecords` | `DoubleMap<NodeId, u64 → EquivocationRecord>` | 双签证据记录 |
| `CurrentEra` | `u64` | 当前 Era 编号（ValueQuery，默认 0） |
| `EraStartBlock` | `BlockNumber` | 当前 Era 起始区块（ValueQuery） |
| `TeeRewardMultiplier` | `u32` | TEE 奖励倍数（bps，0=使用默认 10000=1.0x） |
| `SgxEnclaveBonus` | `u32` | SGX 双证明额外奖励（bps，叠加到 TEE 倍数） |
| `SlashPercentageOverride` | `Option<u32>` | 运行时 Slash 百分比覆盖（None=使用 Config 默认值） |
| `ReporterRewardPct` | `u32` | 举报者奖励百分比（bps，ValueQuery，默认 0=关闭） |

---

## 事件

| 事件 | 字段 | 触发场景 |
|------|------|----------|
| `NodeRegistered` | node_id, operator, stake | `register_node` 成功 |
| `ExitRequested` | node_id | `request_exit` 成功 |
| `ExitFinalized` | node_id, stake_returned | `finalize_exit` 完成退还质押 |
| `EquivocationReported` | node_id, reporter, sequence | `report_equivocation` 成功 |
| `NodeSlashed` | node_id, amount | `slash_equivocation` 执行罚没 |
| `ReporterRewarded` | reporter, amount | Slash 时举报者获得奖励（pct > 0） |
| `SequenceProcessed` | bot_id_hash, sequence | 新序列标记成功 |
| `SequenceDuplicate` | bot_id_hash, sequence | 重复序列（幂等，不失败） |
| `NodeTeeStatusChanged` | node_id, is_tee | TEE 状态变更（验证/降级/解绑） |
| `EraCompleted` | era, total_distributed | Era 结算完成 |
| `TeeRewardParamsUpdated` | tee_multiplier, sgx_bonus | `set_tee_reward_params` 成功 |
| `StakeIncreased` | node_id, added, new_total | `increase_stake` 成功 |
| `NodeReinstated` | node_id | `reinstate_node` 恢复为 Active |
| `NodeForceSuspended` | node_id | `force_suspend_node` 治理暂停 |
| `NodeForceRemoved` | node_id, stake_slashed | `force_remove_node` 治理移除 |
| `BotUnbound` | node_id | `unbind_bot` 解除绑定 |
| `OperatorReplaced` | node_id, old_operator, new_operator | `replace_operator` 转移运营权 |
| `SlashPercentageUpdated` | new_pct | `set_slash_percentage` 更新 |
| `ReporterRewardPctUpdated` | new_pct | `set_reporter_reward_pct` 更新 |
| `NodeForceReinstated` | node_id | `force_reinstate_node` 治理恢复 |
| `EraForceEnded` | era | `force_era_end` 手动触发 |

---

## 错误

| 错误 | 说明 | 触发 extrinsic |
|------|------|----------------|
| `NodeAlreadyRegistered` | 节点 ID 或操作者已注册 | register_node |
| `NodeNotFound` | 节点不存在 | 多处 |
| `NotOperator` | 调用者不是节点操作者 | 节点管理类 |
| `InsufficientStake` | 质押不足（< MinStake 或 amount=0） | register_node, increase_stake, reinstate_node |
| `MaxNodesReached` | ActiveNodeList 已满 | register_node, reinstate_node, force_reinstate_node |
| `NodeNotActive` | 节点非 Active 状态 | request_exit, verify_node_tee, force_suspend_node |
| `AlreadyExiting` | 已有退出请求 | request_exit |
| `CooldownNotComplete` | 冷却期未满 | finalize_exit |
| `NotExiting` | 节点不在 Exiting 状态 | finalize_exit |
| `BotNotRegistered` | Bot 未注册或非活跃 | verify_node_tee |
| `BotOwnerMismatch` | Bot 所有者 ≠ 节点操作者 | verify_node_tee |
| `AttestationNotValid` | Bot TEE 证明无效或已过期 | verify_node_tee |
| `AlreadyTeeVerified` | 节点已是 TEE 节点 | verify_node_tee |
| `FreeTierNotAllowed` | Free 层级不允许此操作 | mark_sequence_processed |
| `NotBotOperator` | 调用者非 Bot Owner/Operator | mark_sequence_processed |
| `EquivocationAlreadyReported` | 该序列已被举报 | report_equivocation |
| `InvalidEquivocationEvidence` | 证据无效（相同哈希/签名/签名验证失败） | report_equivocation |
| `EquivocationNotFound` | 双签记录不存在 | slash_equivocation, cleanup |
| `EquivocationAlreadyResolved` | 已解决，不可重复 Slash | slash_equivocation |
| `EquivocationNotResolved` | 未解决，不可清理 | cleanup_resolved_equivocation |
| `NodeNotSuspended` | 节点非 Suspended 状态 | reinstate_node, force_reinstate_node |
| `NoBotBinding` | 节点未绑定 Bot | unbind_bot |
| `NewOperatorAlreadyHasNode` | 新操作者已拥有节点 | replace_operator |
| `InvalidSlashPercentage` | 百分比超出 0-100 范围 | set_slash_percentage |
| `InvalidReporterRewardPct` | 百分比超出 0-5000 范围 | set_reporter_reward_pct |
| `InvalidTeeRewardParams` | tee_multiplier > 50000 或 sgx_bonus > 10000 | set_tee_reward_params |
| `NothingToCleanup` | 批量清理无有效记录 | batch_cleanup_equivocations |
| `EraNotReady` | Era 未到结束条件（保留，force_era_end 不受此限制） | — |

---

## 核心类型

### ProjectNode — 节点信息

```rust
pub struct ProjectNode<T: Config> {
    pub operator: T::AccountId,      // 节点操作者（质押持有者）
    pub node_id: NodeId,             // 节点 ID（ed25519 公钥，[u8; 32]）
    pub status: NodeStatus,          // Active / Suspended / Exiting
    pub stake: BalanceOf<T>,         // 当前质押余额
    pub registered_at: BlockNumberFor<T>,  // 注册区块
    pub is_tee_node: bool,           // 是否通过 TEE 验证
}
```

### EquivocationRecord — 双签证据

```rust
pub struct EquivocationRecord<T: Config> {
    pub node_id: NodeId,             // 被举报节点
    pub sequence: u64,               // 消息序列号
    pub msg_hash_a: [u8; 32],       // 第一条消息哈希
    pub signature_a: [u8; 64],      // 第一条 ed25519 签名
    pub msg_hash_b: [u8; 32],       // 第二条消息哈希（必须 ≠ msg_hash_a）
    pub signature_b: [u8; 64],      // 第二条 ed25519 签名（必须 ≠ signature_a）
    pub reporter: T::AccountId,      // 举报者
    pub reported_at: BlockNumberFor<T>,  // 举报区块
    pub resolved: bool,              // 是否已被 Slash 处理
}
```

### NodeStatus（来自 primitives）

```
Active     — 正常运行，参与 Era 分配
Suspended  — 被暂停（Slash/治理），可补质押恢复或退出
Exiting    — 退出冷却中，等待 finalize_exit
```

---

## Era 经济模型

```
每 Era（EraLength 个区块）触发 on_era_end:

1. 订阅结算
   └─ SubscriptionSettler::settle_era() → EraSettlementResult
      ├─ 收取活跃订阅费（余额不足则逐 Era 降级）
      ├─ node_share → 已由 subscription pallet 直接分配给运营者
      └─ treasury_share → 国库

2. TEE 证明有效性检查
   └─ 遍历活跃节点，查 NodeBotBinding → BotRegistry 验证
      └─ 过期 → is_tee_node = false + 清理绑定 + 发出事件

3. 节点权重计算
   ├─ 非 TEE 节点: weight = 0（不参与分配）
   ├─ TEE 节点: weight = BASE_NODE_WEIGHT × tee_multiplier / 10000
   └─ SGX 双证明: weight = BASE_NODE_WEIGHT × (tee_multiplier + sgx_bonus) / 10000
   （BASE_NODE_WEIGHT = 500,000 固定常量）
   （tee_multiplier 默认 10000 = 1.0x，0 视为 10000）

4. 奖励分配
   └─ RewardDistributor::distribute_and_record(era, 通胀, 权重向量, ...)
      └─ 可分配总额 = InflationPerEra（订阅收入已直接分配，不参与权重分配）

5. Uptime 快照
   └─ PeerUptimeRecorder::record_era_uptime(era)

6. Era 推进
   ├─ CurrentEra += 1, EraStartBlock = now
   └─ RewardDistributor::prune_old_eras(era)
```

> 无活跃节点时仍执行订阅结算 + Uptime 快照 + Era 推进（跳过权重计算和奖励分配）。

---

## Config 配置

### 常量（`#[pallet::constant]`）

| 参数 | 类型 | 说明 | integrity_test 校验 |
|------|------|------|---------------------|
| `MaxActiveNodes` | `u32` | 最大活跃节点数（ActiveNodeList 上限） | > 0 |
| `MinStake` | `BalanceOf` | 最小质押额 | > 0 |
| `ExitCooldownPeriod` | `BlockNumber` | 退出冷却期（区块数） | > 0 |
| `EraLength` | `BlockNumber` | Era 长度（区块数） | > 0 |
| `InflationPerEra` | `BalanceOf` | 每 Era 通胀铸币量 | — |
| `SlashPercentage` | `u32` | 默认 Slash 百分比（如 10 = 10%） | ∈ 1..=100 |
| `SequenceTtlBlocks` | `BlockNumber` | ProcessedSequences 过期 TTL | — |
| `MaxSequenceCleanupPerBlock` | `u32` | 每块最多清理的过期序列数 | — |

### 依赖 Trait

| 参数 | Trait | 说明 |
|------|-------|------|
| `Currency` | `ReservableCurrency` | 质押 reserve/unreserve、Slash |
| `BotRegistry` | `BotRegistryProvider<AccountId>` | Bot 注册/TEE/证明查询 |
| `SubscriptionSettler` | `SubscriptionSettler` | Era 结束时订阅费结算 |
| `RewardDistributor` | `EraRewardDistributor` | Era 奖励按权重分配 + 历史清理 |
| `Subscription` | `SubscriptionProvider` | 订阅层级查询（Tier gate） |
| `PeerUptimeRecorder` | `PeerUptimeRecorder` | Era 结束时 Peer 心跳快照 |
| `OrphanRewardClaimer` | `OrphanRewardClaimer<AccountId>` | 节点退出时领取残留奖励 |

---

## Hooks

### `on_initialize(n)`

每个区块执行，包含两个阶段：

1. **防膨胀清理** — 游标式清理过期 `ProcessedSequences`
   - 扫描上限 = `MaxSequenceCleanupPerBlock × 3`（防全表扫描）
   - 清理上限 = `MaxSequenceCleanupPerBlock`
   - 过期条件: `now - recorded_block > SequenceTtlBlocks`
   - Weight: 动态计算（5M base + 5M×scanned + 10M×cleaned ref_time）

2. **Era 边界检测** — `n - era_start >= EraLength` 时触发 `on_era_end(n)`
   - 首次运行时初始化 `EraStartBlock`

### `integrity_test`

启动时校验 Config 常量有效性：
- `MinStake > 0`
- `EraLength > 0`
- `SlashPercentage ∈ 1..=100`
- `ExitCooldownPeriod > 0`
- `MaxActiveNodes > 0`

---

## Trait 实现

### `NodeConsensusProvider<AccountId>`

供其他模块（ads、subscription 等）查询节点状态：

| 方法 | 返回 | 说明 |
|------|------|------|
| `is_node_active(node_id)` | `bool` | 节点是否为 Active 状态 |
| `node_operator(node_id)` | `Option<AccountId>` | 获取节点操作者 |
| `is_tee_node_by_operator(operator)` | `bool` | 操作者是否运行 TEE 节点 |

---

## 公共查询方法

| 方法 | 返回 | 说明 |
|------|------|------|
| `is_sequence_processed(bot_id_hash, seq)` | `bool` | 序列是否已处理 |
| `effective_slash_percentage()` | `u32` | 有效 Slash 百分比（运行时覆盖 > Config 默认值） |
| `active_node_count()` | `u32` | 当前活跃节点数 |
| `current_era()` | `u64` | 当前 Era 编号 |
| `era_blocks_remaining()` | `BlockNumber` | 距下一 Era 结束的剩余区块数 |

---

## 内部函数

| 函数 | 说明 |
|------|------|
| `on_era_end(now)` | Era 完整编排流程（订阅结算 → TEE 检查 → 权重计算 → 奖励分配 → Uptime → 推进） |
| `compute_node_weight(node, node_id)` | 计算单个节点权重（非 TEE=0，TEE=base×factor/10000，SGX 叠加 bonus） |
| `cleanup_expired_sequences(now)` | 游标式清理过期 ProcessedSequences，返回动态 Weight |

---

## 测试覆盖

当前测试数：**80**

| 分类 | 测试数 | 覆盖范围 |
|------|:------:|----------|
| 节点注册 | 4 | 成功、重复节点、重复操作者、质押不足 |
| 退出流程 | 2 | 完整流程、非操作者拒绝 |
| Equivocation 举报 | 2 | 举报成功、Slash 执行 |
| 消息去重 | 2 | 标记成功、重复检测 |
| TEE 验证 | 4 | 成功、重复验证、非 TEE Bot、所有者不匹配 |
| Era 结算 | 3 | 正常结算、无节点、非 TEE 零权重 |
| TEE 奖励参数 | 1 | Root 权限校验 |
| 过期清理 | 2 | 过期清理、新鲜保留 |
| Trait 实现 | 1 | NodeConsensusProvider 三个方法 |
| Tier 层级 | 2 | is_paid 逻辑、默认 Free |
| Tier Gate | 2 | Free 拒绝、Paid 通过 |
| 审计回归 (R1) | 12 | H1 证据校验、H2 授权、H4 绑定清理、H5 暂停退出、H6 参数校验、H8 TEE 重置、R1 Slash/清理/TEE/M5 |
| P1 补充质押 | 4 | 成功、非操作者、零金额、Suspended |
| P2 节点恢复 | 4 | 成功、非 Suspended、质押不足、Slash 后补充恢复 |
| P3 治理暂停 | 3 | 成功、非 Root、非 Active |
| P4 治理移除 | 2 | 成功、非 Root |
| P5 举报奖励 | 5 | 设置/校验/Root、Slash 奖励、零百分比 |
| P7 解绑 Bot | 3 | 成功、无绑定、解绑重绑 |
| P8 运营权转移 | 3 | 成功、新操作者已有节点、质押转移 |
| P9 Slash 百分比 | 3 | 设置/恢复默认、超范围、运行时生效 |
| P10 治理恢复 | 2 | 成功、非 Root |
| P12 批量清理/查询 | 4 | 批量清理、空清理、节点数查询、Era 查询 |
| P13 手动 Era | 2 | 成功、非 Root |
| P14 签名验证 | 3 | 无效签名拒绝、有效签名通过、单签名无效 |

---

## 相关模块

| 模块 | 路径 | 关系 |
|------|------|------|
| **primitives** | `../primitives/` | NodeStatus、NodeId、BalanceOf、BotRegistryProvider 等基础类型和 Trait 定义 |
| **registry** | `../registry/` | Bot 注册管理，实现 `BotRegistryProvider` + `PeerUptimeRecorder` |
| **subscription** | `../subscription/` | 订阅管理，实现 `SubscriptionSettler` + `SubscriptionProvider` |
| **rewards** | `../rewards/` | 奖励分配，实现 `EraRewardDistributor` + `OrphanRewardClaimer` |
| **community** | `../community/` | 社区管理（独立模块，无直接依赖） |
