# pallet-grouprobot-consensus

> 路径：`pallets/grouprobot/consensus/`

节点共识系统，提供节点质押、TEE 加权奖励分配、消息去重、Era 编排、Equivocation 举报与 Slash。

> **注意**: 订阅管理和奖励领取已拆分到 `pallet-grouprobot-subscription` 和 `pallet-grouprobot-rewards`。

## 设计理念

- **质押准入**：节点注册时锁定最低质押（`MinStake`），退出需经冷却期（Suspended 节点也可退出）
- **TEE 专属奖励**：仅 TEE 节点参与 Era 奖励分配，非 TEE 节点权重为 0
- **TEE 自动降级**：Era 结束时检查绑定 Bot 的证明有效性，过期自动降级 + 清理 NodeBotBinding
- **Era 编排**：`on_era_end` 委托 SubscriptionSettler → RewardDistributor → PeerUptimeRecorder 完成结算
- **Equivocation 惩罚**：同序列双签举报 → Root 执行 Slash（质押百分比罚没 + 暂停节点 + 重置 TEE 状态）
- **消息去重**：`ProcessedSequences` 防止重复处理，自动过期清理避免存储膨胀
- **孤儿奖励**：节点退出时通过 `OrphanRewardClaimer` 自动领取残留奖励

## Extrinsics

### 节点管理
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 0 | `register_node` | Signed | 注册节点 + 质押锁定 |
| 1 | `request_exit` | Signed | 申请退出（Active/Suspended 均可，进入冷却期） |
| 2 | `finalize_exit` | Signed | 完成退出 + 领取残留奖励 + 退还质押 + 清理 NodeBotBinding |
| 11 | `verify_node_tee` | Signed | 通过 Registry 证明验证节点 TEE 状态 |

### Equivocation
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 3 | `report_equivocation` | Signed | 举报双签（同序列不同消息+签名，需两组证据不同） |
| 4 | `slash_equivocation` | Root | 执行 Slash（罚没质押百分比 + 暂停节点 + 重置 TEE 状态 + 清理 Bot 绑定） |

### 去重与治理
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 10 | `mark_sequence_processed` | Signed | 标记消息序列已处理（Tier gate + Bot 所有者或操作者授权） |
| 12 | `set_tee_reward_params` | Root | 设置 TEE 奖励参数 |
| 13 | `cleanup_resolved_equivocation` | Signed | 清理已解决的 Equivocation 记录（任何人可调用） |

## 存储

### 节点
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Nodes` | `Map<NodeId, ProjectNode>` | 节点信息 |
| `OperatorNodes` | `Map<AccountId, NodeId>` | 操作者 → 节点 ID |
| `ActiveNodeList` | `BoundedVec<NodeId>` | 活跃节点列表 |
| `ExitRequests` | `Map<NodeId, BlockNumber>` | 退出请求（冷却起始区块） |

### 消息去重
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `ProcessedSequences` | `DoubleMap<BotIdHash, u64, BlockNumber>` | 已处理的消息序列 |

### TEE 绑定
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NodeBotBinding` | `Map<NodeId, BotIdHash>` | 节点→Bot TEE 绑定 |

### Equivocation
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `EquivocationRecords` | `DoubleMap<NodeId, u64, EquivocationRecord>` | 双签记录 |

### Era 经济
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `CurrentEra` | `u64` | 当前 Era |
| `EraStartBlock` | `BlockNumber` | Era 起始区块 |
| `TeeRewardMultiplier` | `u32` | TEE 奖励倍数（bps, 0=默认10000=1.0x, 15000=1.5x） |
| `SgxEnclaveBonus` | `u32` | SGX 双证明额外奖励（bps） |

## 事件

| 事件 | 说明 |
|------|------|
| `NodeRegistered` | 节点注册成功（含 node_id, operator, stake） |
| `ExitRequested` | 节点申请退出 |
| `ExitFinalized` | 节点完成退出（含退还质押金额） |
| `EquivocationReported` | 双签已举报（含 node_id, reporter, sequence） |
| `NodeSlashed` | 节点已被 Slash（含实际罚没金额） |
| `SequenceProcessed` | 消息序列已标记处理 |
| `SequenceDuplicate` | 检测到重复序列（幂等返回，不失败） |
| `NodeTeeStatusChanged` | 节点 TEE 状态变更（含 is_tee 标志） |
| `EraCompleted` | Era 完成（含 era 编号和分配总额） |
| `TeeRewardParamsUpdated` | TEE 奖励参数已更新（含 tee_multiplier, sgx_bonus） |

## 主要类型

### ProjectNode（节点信息）
```rust
pub struct ProjectNode<T: Config> {
    pub operator: T::AccountId,
    pub node_id: NodeId,
    pub status: NodeStatus,         // Active/Probation/Suspended/Exiting
    pub reputation: u32,            // 初始 5000
    pub uptime_blocks: u64,
    pub stake: BalanceOf<T>,
    pub registered_at: BlockNumberFor<T>,
    pub last_active: BlockNumberFor<T>,
    pub is_tee_node: bool,
}
```

### EquivocationRecord（双签证据）
```rust
pub struct EquivocationRecord<T: Config> {
    pub node_id: NodeId,
    pub sequence: u64,
    pub msg_hash_a: [u8; 32],
    pub signature_a: [u8; 64],
    pub msg_hash_b: [u8; 32],
    pub signature_b: [u8; 64],
    pub reporter: T::AccountId,
    pub reported_at: BlockNumberFor<T>,
    pub resolved: bool,
}
```

## Era 经济模型

```
每 Era (EraLength 个区块):
1. 收取活跃订阅费 → subscription_income
   - 余额不足 → Active→PastDue→Suspended（逐 Era 降级）
2. 拆分订阅收入：90% 节点 + 10% 国库
3. 铸币通胀：InflationPerEra
4. 可分配总额 = 仅通胀（节点份额已由 subscription pallet 直接分配）
5. 按权重分配（仅 TEE 节点参与）：
   - 非 TEE 节点 weight = 0（不参与分配）
   - TEE 节点 weight = reputation × 100 × TeeRewardMultiplier / 10000
   - SGX 双证明 weight = reputation × 100 × (TeeRewardMultiplier + SgxEnclaveBonus) / 10000
```

## 错误

| 错误 | 说明 |
|------|------|
| `NodeAlreadyRegistered` | 节点或操作者已注册 |
| `NodeNotFound` | 节点不存在 |
| `NotOperator` | 不是节点操作者 |
| `InsufficientStake` | 质押不足 |
| `MaxNodesReached` | 活跃节点数已满 |
| `NodeNotActive` | 节点非活跃 |
| `AlreadyExiting` | 节点已在退出中 |
| `CooldownNotComplete` | 冷却期未到 |
| `NotExiting` | 节点不在退出状态 |
| `BotNotRegistered` | Bot 未注册 |
| `NotBotOwner` | 不是 Bot 所有者 |
| `EquivocationAlreadyReported` | 已举报 |
| `SequenceAlreadyProcessed` | 序列已处理 |
| `BotOwnerMismatch` | Bot 所有者与操作者不匹配 |
| `AttestationNotValid` | TEE 证明无效或已过期 |
| `AlreadyTeeVerified` | 节点已是 TEE 节点 |
| `FreeTierNotAllowed` | Free 层级不允许此功能 |
| `InvalidEquivocationEvidence` | 双签证据无效（相同哈希或签名） |
| `EquivocationNotFound` | 双签记录不存在 |
| `NotBotOperator` | 调用者不是 Bot 操作者或所有者 |
| `InvalidTeeRewardParams` | TEE 奖励参数超出允许范围 |
| `EquivocationAlreadyResolved` | Equivocation 已被解决（不可重复 Slash） |
| `EquivocationNotResolved` | Equivocation 尚未解决（不可清理） |

> **Note:** `NotBotOwner` 已在 Round 1.1 中移除（死错误码，从未使用）。SCALE 索引已变更。

## 配置参数

| 参数 | 说明 |
|------|------|
| `Currency` | ReservableCurrency（质押/Slash） |
| `MaxActiveNodes` | 最大活跃节点数 |
| `MinStake` | 最小质押额 |
| `ExitCooldownPeriod` | 退出冷却期（区块数） |
| `EraLength` | Era 长度（区块数） |
| `InflationPerEra` | 每 Era 通胀铸币量 |
| `SlashPercentage` | Slash 百分比（如 10 = 10%） |
| `BotRegistry` | Bot 注册查询（`BotRegistryProvider`） |
| `SequenceTtlBlocks` | ProcessedSequences 过期区块数 |
| `MaxSequenceCleanupPerBlock` | 每块最多清理的过期序列数 |
| `SubscriptionSettler` | 订阅结算（`SubscriptionSettler`） |
| `RewardDistributor` | 奖励分配（`EraRewardDistributor`） |
| `Subscription` | 订阅层级查询（`SubscriptionProvider`） |
| `PeerUptimeRecorder` | Peer Uptime 记录（`PeerUptimeRecorder`） |
| `OrphanRewardClaimer` | 节点退出时领取残留奖励（`OrphanRewardClaimer`） |

## Hooks

- **`on_initialize`**：
  1. **防膨胀清理**：游标式清理过期 `ProcessedSequences`（扫描上限 = `MaxSequenceCleanupPerBlock × 3`，清理上限 = `MaxSequenceCleanupPerBlock`）
  2. **Era 边界检测**：`n - era_start >= EraLength` 时触发 `on_era_end`

- **`on_era_end`** 编排流程：
  1. **订阅结算**：委托 `SubscriptionSettler::settle_era()` 收取订阅费（无节点时仍执行）
  2. **TEE 验证降级**：遍历活跃节点，检查 `NodeBotBinding` 绑定 Bot 的证明有效性，过期则 `is_tee_node = false` + 清理绑定
  3. **权重计算**：非 TEE 权重 = 0，TEE 权重 = reputation × 100 × (tee_factor + sgx_bonus) / 10000
  4. **奖励分配**：委托 `RewardDistributor::distribute_and_record()` 铸币 + 按权重分配
  5. **Uptime 快照**：委托 `PeerUptimeRecorder::record_era_uptime()` 快照心跳计数
  6. **清理**：递增 Era，委托 `RewardDistributor::prune_old_eras()` 清理过期记录

## Trait 实现

### NodeConsensusProvider\<AccountId\>

供其他模块查询节点状态：
- `is_node_active(node_id)` — 节点是否活跃
- `node_operator(node_id)` — 获取操作者
- `is_tee_node_by_operator(operator)` — 操作者是否运行 TEE 节点

## 公共查询方法

- `is_sequence_processed(bot_id_hash, sequence)` — 序列是否已处理

## 测试覆盖

当前测试数：44

## 审计历史

### Round 1 (Mar 2026)

| ID | 严重级 | 描述 | 状态 |
|---|---|---|---|
| H1 | High | `slash_equivocation` 不检查 `resolved` — 可重复 Slash | ✅ 已修复 |
| M1 | Medium | `report_equivocation` 不验证签名有效性 | 记录 |
| M2 | Medium | `EquivocationRecords` 无清理机制 | ✅ 已修复 |
| M3 | Medium | `verify_node_tee` 不检查节点活跃状态 | ✅ 已修复 |
| M4 | Medium | `uptime_blocks`/`last_active`/`reputation` 死字段/静态字段 | 记录 |
| M5 | Medium | `on_era_end` 无节点时跳过订阅结算+uptime+pruning | ✅ 已修复 |
| L1 | Low | `log`+`sp-core` 死依赖 | ✅ 已修复 |
| L2 | Low | `try-runtime` feature 缺失传播 | ✅ 已修复 |
| L3 | Low | `NotBotOwner` 死错误码 | ✅ R1.1 已修复 |
| L4 | Low | `treasury_share` 硬编码 10% 耦合 | ✅ R1.1 已修复 |

### Round 1.1 (Mar 2026)

| ID | 严重级 | 描述 | 状态 |
|---|---|---|---|
| L3 | Low | 移除死错误码 `NotBotOwner` | ✅ 已修复 |
| L4 | Low | `SubscriptionSettler::settle_era()` 返回 `EraSettlementResult` 含 `treasury_share`，消除硬编码 | ✅ 已修复 |

## 相关模块

- [primitives/](../primitives/) — NodeStatus、NodeId、BalanceOf、BotRegistryProvider 等
- [registry/](../registry/) — Bot 注册（BotRegistryProvider + PeerUptimeRecorder 实现）
- [subscription/](../subscription/) — 订阅管理（SubscriptionSettler + SubscriptionProvider 实现）
- [rewards/](../rewards/) — 奖励分配（EraRewardDistributor + OrphanRewardClaimer 实现）
- [community/](../community/) — 社区管理
