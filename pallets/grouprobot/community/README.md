# pallet-grouprobot-community

> 路径：`pallets/grouprobot/community/`

社区管理系统，提供群规则配置、节点准入策略、动作日志存证、成员声誉管理功能。

## 设计理念

- **CAS 乐观锁**：`update_community_config` 使用版本号冲突检测，防止并发覆盖
- **双层声誉**：社区本地声誉（`MemberReputation`）+ 全局聚合声誉（`GlobalReputation`）
- **冷却机制**：同一操作者对同一目标的声誉变更受 `ReputationCooldown` 限制
- **批量日志**：`batch_submit_logs` 支持一次提交最多 50 条日志，降低交易成本
- **日志清理**：`clear_expired_logs` 按年龄释放存储空间

## Extrinsics

### 动作日志
| call_index | 方法 | 说明 |
|:---:|------|------|
| 0 | `submit_action_log` | 提交单条动作日志（Ed25519 签名存证） |
| 3 | `batch_submit_logs` | 批量提交日志（最多 50 条） |
| 4 | `clear_expired_logs` | 清理过期日志（按 max_age_blocks） |

### 社区配置
| call_index | 方法 | 说明 |
|:---:|------|------|
| 1 | `set_node_requirement` | 设置节点准入策略（Any/TeeOnly/TeePreferred/MinTee） |
| 2 | `update_community_config` | 更新群规则（CAS 乐观锁，expected_version 校验） |

### 声誉管理
| call_index | 方法 | 说明 |
|:---:|------|------|
| 5 | `award_reputation` | 奖励声誉（+delta，受冷却限制） |
| 6 | `deduct_reputation` | 扣除声誉（-delta，受冷却限制） |
| 7 | `reset_reputation` | 重置用户声誉（清除本地记录，更新全局） |

## 存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `CommunityConfigs` | `Map<CommunityIdHash, CommunityConfig>` | 社区群规则配置 |
| `CommunityNodeRequirement` | `Map<CommunityIdHash, NodeRequirement>` | 节点准入策略（快速查询） |
| `ActionLogs` | `Map<CommunityIdHash, BoundedVec<ActionLog>>` | 动作日志 |
| `LogCount` | `u64` | 日志总数 |
| `MemberReputation` | `DoubleMap<CommunityIdHash, [u8;32], ReputationRecord>` | 社区内用户声誉 |
| `GlobalReputation` | `Map<[u8;32], i64>` | 全局用户声誉（所有社区之和） |
| `ReputationCooldowns` | `NMap<(AccountId, CommunityIdHash, [u8;32]), BlockNumber>` | 声誉变更冷却 |

## 主要类型

### CommunityConfig（群规则配置）
```rust
pub struct CommunityConfig {
    pub node_requirement: NodeRequirement,  // 节点准入策略
    pub anti_flood_enabled: bool,           // 防刷屏开关
    pub flood_limit: u16,                   // 刷屏阈值
    pub warn_limit: u8,                     // 警告次数限制
    pub warn_action: WarnAction,            // 达限动作（Kick/Ban/Mute）
    pub welcome_enabled: bool,              // 欢迎消息开关
    pub version: u32,                       // CAS 版本号
}
```

### ReputationRecord（声誉记录）
```rust
pub struct ReputationRecord<T: Config> {
    pub score: i64,                        // 声誉分数（可负）
    pub awards: u32,                       // 累计奖励次数
    pub deductions: u32,                   // 累计扣分次数
    pub last_updated: BlockNumberFor<T>,   // 最后修改区块
}
```

### ActionLog（动作日志）
```rust
pub struct ActionLog<T: Config> {
    pub community_id_hash: CommunityIdHash,
    pub action_type: ActionType,           // Kick/Ban/Mute/Warn/...
    pub operator: T::AccountId,
    pub target_hash: [u8; 32],
    pub sequence: u64,
    pub message_hash: [u8; 32],
    pub signature: [u8; 64],              // Ed25519 签名
    pub block_number: BlockNumberFor<T>,
}
```

## 错误

| 错误 | 说明 |
|------|------|
| `LogsFull` | 日志数量已满 |
| `SameNodeRequirement` | 准入策略未变更 |
| `ConfigVersionConflict` | CAS 版本冲突 |
| `ConfigNotFound` | 社区配置不存在 |
| `NoLogsToClear` | 无可清理日志 |
| `EmptyBatch` | 批量日志为空 |
| `BatchTooLarge` | 批量日志超过 50 条 |
| `ReputationOnCooldown` | 声誉变更冷却中 |
| `ReputationDeltaTooLarge` | 变更值超过 MaxReputationDelta |
| `ReputationDeltaZero` | 变更值为零 |

## 配置参数

| 参数 | 说明 |
|------|------|
| `MaxLogsPerCommunity` | 每个社区最大日志数 |
| `ReputationCooldown` | 声誉变更冷却区块数 |
| `MaxReputationDelta` | 单次声誉变更最大绝对值 |
| `BotRegistry` | Bot 注册查询（`BotRegistryProvider`） |

## Trait 实现

实现 `CommunityProvider<AccountId>`：
- `get_node_requirement(community_id_hash)` — 获取节点准入策略
- `is_community_bound(community_id_hash)` — 社区是否已配置

实现 `ReputationProvider`：
- `get_reputation(community_id_hash, user_hash)` — 社区本地声誉
- `get_global_reputation(user_hash)` — 全局声誉

## 相关模块

- [primitives/](../primitives/) — ActionType、NodeRequirement、WarnAction
- [registry/](../registry/) — Bot 注册（BotRegistryProvider）
- [consensus/](../consensus/) — 节点共识（查询社区准入策略）
