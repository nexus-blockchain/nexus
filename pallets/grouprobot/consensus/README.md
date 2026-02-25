# pallet-grouprobot-consensus

> 路径：`pallets/grouprobot/consensus/`

节点共识系统，提供节点质押、TEE 加权奖励分配、消息去重、订阅管理、Equivocation 举报与 Slash。

## 设计理念

- **质押准入**：节点注册时锁定最低质押（`MinStake`），退出需经冷却期
- **TEE 加权奖励**：TEE 节点获得 `TeeRewardMultiplier` 倍奖励权重
- **Era 经济模型**：每 Era 收取订阅费 + 铸币通胀，按权重分配给活跃节点
- **Equivocation 惩罚**：同序列双签举报 → Root 执行 Slash（质押百分比罚没）
- **消息去重**：`ProcessedSequences` 防止重复处理同一消息

## Extrinsics

### 节点管理
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 0 | `register_node` | Signed | 注册节点 + 质押锁定 |
| 1 | `request_exit` | Signed | 申请退出（进入冷却期） |
| 2 | `finalize_exit` | Signed | 完成退出 + 退还质押 |
| 11 | `set_node_tee_status` | Signed | 设置节点 TEE 状态 |

### Equivocation
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 3 | `report_equivocation` | Signed | 举报双签（同序列不同消息+签名） |
| 4 | `slash_equivocation` | Root | 执行 Slash（罚没质押百分比，暂停节点） |

### 订阅管理
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 5 | `subscribe` | Signed | 订阅 Bot 服务（Basic/Pro/Enterprise） |
| 6 | `deposit_subscription` | Signed | 充值订阅（PastDue/Suspended 自动恢复） |
| 7 | `cancel_subscription` | Signed | 取消订阅（退还 Escrow 余额） |
| 8 | `change_tier` | Signed | 变更订阅层级 |

### 奖励与去重
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 9 | `claim_rewards` | Signed | 领取节点奖励（铸币给操作者） |
| 10 | `mark_sequence_processed` | Signed | 标记消息序列已处理（去重） |

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

### 订阅
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Subscriptions` | `Map<BotIdHash, Subscription>` | 订阅信息 |
| `SubscriptionEscrow` | `Map<BotIdHash, Balance>` | 订阅预存余额 |

### Equivocation
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `EquivocationRecords` | `DoubleMap<NodeId, u64, EquivocationRecord>` | 双签记录 |

### Era 经济
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `CurrentEra` | `u64` | 当前 Era |
| `EraStartBlock` | `BlockNumber` | Era 起始区块 |
| `NodePendingRewards` | `Map<NodeId, Balance>` | 待领取奖励 |
| `NodeTotalEarned` | `Map<NodeId, Balance>` | 累计已领取 |
| `TeeRewardMultiplier` | `u32` | TEE 奖励倍数（bps, 15000=1.5x） |
| `SgxEnclaveBonus` | `u32` | SGX 双证明额外奖励（bps） |
| `EraRewards` | `Map<u64, EraRewardInfo>` | Era 奖励记录 |

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

### Subscription（订阅信息）
```rust
pub struct Subscription<T: Config> {
    pub owner: T::AccountId,
    pub bot_id_hash: BotIdHash,
    pub tier: SubscriptionTier,     // Basic/Pro/Enterprise
    pub fee_per_era: BalanceOf<T>,
    pub started_at: BlockNumberFor<T>,
    pub paid_until_era: u64,
    pub status: SubscriptionStatus, // Active/PastDue/Suspended/Cancelled
}
```

### EraRewardInfo（Era 奖励信息）
```rust
pub struct EraRewardInfo<Balance> {
    pub subscription_income: Balance,   // 订阅收入
    pub inflation_mint: Balance,        // 通胀铸币
    pub total_distributed: Balance,     // 分配给节点的总额
    pub treasury_share: Balance,        // 国库份额
    pub node_count: u32,                // 活跃节点数
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
5. 按权重分配：weight = reputation × 100 × tee_factor / 10000
   - 普通节点 tee_factor = 10000 (1.0x)
   - TEE 节点 tee_factor = TeeRewardMultiplier (默认 15000 = 1.5x)
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
| `SubscriptionAlreadyExists` | 订阅已存在 |
| `SubscriptionNotFound` | 订阅不存在 |
| `SubscriptionAlreadyCancelled` | 订阅已取消 |
| `SameTier` | 层级未变更 |
| `InsufficientDeposit` | 预存不足 |
| `NoPendingRewards` | 无待领取奖励 |
| `EquivocationAlreadyReported` | 已举报 |
| `SequenceAlreadyProcessed` | 序列已处理 |
| `SameTeeStatus` | TEE 状态未变更 |

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
| `BasicFeePerEra` | Basic 层级每 Era 费用 |
| `ProFeePerEra` | Pro 层级每 Era 费用 |
| `EnterpriseFeePerEra` | Enterprise 层级每 Era 费用 |
| `BotRegistry` | Bot 注册查询（`BotRegistryProvider`） |

## Hooks

- **`on_initialize`**：检测 Era 边界（`n - era_start >= EraLength`），触发 `on_era_end` 执行订阅扣费 + 奖励分配

## Trait 实现

实现 `NodeConsensusProvider<AccountId>`：
- `is_node_active(node_id)` — 节点是否活跃
- `node_operator(node_id)` — 获取操作者
- `is_tee_node_by_operator(operator)` — 操作者是否运行 TEE 节点

## 相关模块

- [primitives/](../primitives/) — NodeStatus、SubscriptionTier、BotRegistryProvider
- [registry/](../registry/) — Bot 注册（BotRegistryProvider 实现）
- [community/](../community/) — 社区管理（CommunityProvider）
