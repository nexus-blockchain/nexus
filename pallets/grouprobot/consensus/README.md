# pallet-grouprobot-consensus

> 路径：`pallets/grouprobot/consensus/`

节点共识系统，提供节点质押、TEE 加权奖励分配、消息去重、Era 编排、Equivocation 举报与 Slash。

> **注意**: 订阅管理和奖励领取已拆分到 `pallet-grouprobot-subscription` 和 `pallet-grouprobot-rewards`。

## 设计理念

- **质押准入**：节点注册时锁定最低质押（`MinStake`），退出需经冷却期
- **TEE 专属奖励**：仅 TEE 节点参与 Era 奖励分配，非 TEE 节点权重为 0
- **Era 经济模型**：每 Era 收取订阅费 + 铸币通胀，按 TEE 权重分配给活跃 TEE 节点
- **Equivocation 惩罚**：同序列双签举报 → Root 执行 Slash（质押百分比罚没）
- **消息去重**：`ProcessedSequences` 防止重复处理同一消息

## Extrinsics

### 节点管理
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 0 | `register_node` | Signed | 注册节点 + 质押锁定 |
| 1 | `request_exit` | Signed | 申请退出（进入冷却期） |
| 2 | `finalize_exit` | Signed | 完成退出 + 退还质押 |
| 11 | `verify_node_tee` | Signed | 通过 Registry 证明验证节点 TEE 状态 |

### Equivocation
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 3 | `report_equivocation` | Signed | 举报双签（同序列不同消息+签名，需证据不同） |
| 4 | `slash_equivocation` | Root | 执行 Slash（罚没质押百分比，暂停节点） |

### 去重与治理
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 10 | `mark_sequence_processed` | Signed | 标记消息序列已处理（需 Bot 授权） |
| 12 | `set_tee_reward_params` | Root | 设置 TEE 奖励参数 |

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
2. 拆分订阅收入：80% 节点 + 10% 国库 + 10% Agent
3. 铸币通胀：InflationPerEra
4. 可分配总额 = 节点份额 + 通胀
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

## Hooks

- **`on_initialize`**：
  1. 清理过期 `ProcessedSequences`（游标式，每块最多 `MaxSequenceCleanupPerBlock` 条）
  2. 检测 Era 边界（`n - era_start >= EraLength`），触发 `on_era_end`
  3. `on_era_end`: 结算订阅 → TEE 验证降级 → 权重计算 → 奖励分配 → Uptime 快照

## Trait 实现

实现 `NodeConsensusProvider<AccountId>`：
- `is_node_active(node_id)` — 节点是否活跃
- `node_operator(node_id)` — 获取操作者
- `is_tee_node_by_operator(operator)` — 操作者是否运行 TEE 节点

## 相关模块

- [primitives/](../primitives/) — NodeStatus、SubscriptionTier、BotRegistryProvider
- [registry/](../registry/) — Bot 注册（BotRegistryProvider 实现）
- [community/](../community/) — 社区管理（CommunityProvider）
