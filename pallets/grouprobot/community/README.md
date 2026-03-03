# pallet-grouprobot-community

> 路径：`pallets/grouprobot/community/`

社区管理系统，提供群规则配置、节点准入策略、动作日志存证、成员声誉管理功能。

## 设计理念

- **CAS 乐观锁**：`update_community_config` 使用版本号冲突检测，防止并发覆盖
- **双层声誉**：社区本地声誉（`MemberReputation`）+ 全局聚合声誉（`GlobalReputation`）
- **冷却机制**：同一操作者对同一目标的声誉变更受 `ReputationCooldown` 限制
- **批量日志**：`batch_submit_logs` 支持一次提交最多 50 条日志，降低交易成本
- **日志清理**：`clear_expired_logs` 按年龄释放存储空间
- **Ed25519 签名验证**：动作日志提交需 Bot TEE 节点的 Ed25519 签名
- **Sequence 单调递增**：日志 sequence 严格递增，防止重放攻击
- **Tier 订阅门控**：日志提交和清理受订阅层级限制

## Extrinsics

### 动作日志
| call_index | 方法 | 说明 |
|:---:|------|------|
| 0 | `submit_action_log` | 提交单条动作日志（Bot owner + Ed25519 签名 + sequence 递增） |
| 3 | `batch_submit_logs` | 批量提交日志（最多 MaxBatchSize 条，weight 按数量缩放） |
| 4 | `clear_expired_logs` | 清理过期日志（按 max_age_blocks，受层级保留期限制） |

### 社区配置
| call_index | 方法 | 说明 |
|:---:|------|------|
| 1 | `set_node_requirement` | 设置节点准入策略（Any/TeeOnly/TeePreferred/MinTee） |
| 2 | `update_community_config` | 更新群规则（CAS 乐观锁，language 须 ISO 639-1 小写） |
| 8 | `update_active_members` | Bot 更新社区活跃成员数（供广告 CPM 计费） |

### 存储清理
| call_index | 方法 | 说明 |
|:---:|------|------|
| 9 | `cleanup_expired_cooldowns` | 清理已过期的冷却条目（任何人可调用，释放 storage） |

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
| `LastSequence` | `Map<CommunityIdHash, Option<u64>>` | 社区最后提交的日志 sequence |
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
    pub ads_enabled: bool,                  // 是否接受广告投放
    pub active_members: u32,                // 活跃成员数（Bot 定期更新）
    pub language: [u8; 2],                  // 社区语言（ISO 639-1 小写）
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
| `NoLogsToClear` | 无可清理日志 |
| `EmptyBatch` | 批量日志为空 |
| `BatchTooLarge` | 批量日志超过 50 条 |
| `ReputationOnCooldown` | 声誉变更冷却中 |
| `ReputationDeltaTooLarge` | 变更值超过 MaxReputationDelta |
| `InvalidMaxAge` | max_age_blocks 不能为零 |
| `ReputationDeltaZero` | 变更值为零 |
| `CommunityNotFound` | 社区配置不存在（需先 update_community_config） |
| `FreeTierNotAllowed` | Free 层级不允许使用此功能 |
| `RetentionPeriodNotExpired` | 日志未超过层级保留期限 |
| `InvalidSignature` | Ed25519 签名验证失败 |
| `BotPublicKeyNotFound` | Bot 未注册或 public_key 不可用 |
| `NotBotOwner` | 调用者不是该社区绑定 Bot 的 owner |
| `InvalidLanguageCode` | 语言代码无效（须为 ASCII 小写字母） |
| `SequenceNotMonotonic` | 日志 sequence 必须严格递增 |
| `BotNotActive` | Bot 未激活（停用/banned） |
| `CooldownNotExpired` | 冷却条目尚未过期 |
| `CooldownNotFound` | 冷却条目不存在 |

## 事件

| 事件 | 说明 |
|------|------|
| `ActionLogSubmitted` | 单条动作日志已提交（含 community_id_hash, action_type, operator, sequence） |
| `BatchLogsSubmitted` | 批量日志已提交（含 community_id_hash, count） |
| `NodeRequirementUpdated` | 节点准入策略已更新（含 requirement） |
| `CommunityConfigUpdated` | 社区配置已更新（含 version） |
| `ExpiredLogsCleared` | 过期日志已清理（含 cleared 数量） |
| `ReputationAwarded` | 声誉已奖励（含 community_id_hash, user_hash, delta, new_score, operator） |
| `ReputationDeducted` | 声誉已扣除（含 community_id_hash, user_hash, delta, new_score, operator） |
| `ReputationReset` | 声誉已重置（含 community_id_hash, user_hash, old_score） |
| `ActiveMembersUpdated` | 活跃成员数已更新（含 community_id_hash, active_members） |
| `CooldownCleaned` | 过期冷却条目已清理（含 community_id_hash, user_hash, operator） |

## 配置参数

| 参数 | 说明 |
|------|------|
| `MaxLogsPerCommunity` | 每个社区最大日志数 |
| `ReputationCooldown` | 声誉变更冷却区块数 |
| `MaxReputationDelta` | 单次声誉变更最大绝对值 |
| `BotRegistry` | Bot 注册查询（`BotRegistryProvider`） |
| `MaxBatchSize` | 批量提交日志最大条数 |
| `BlocksPerDay` | 每日区块数（用于日志保留期计算，6s/block = 14400） |
| `Subscription` | 订阅层级查询（`SubscriptionProvider`） |

## 公共查询方法

- `get_node_requirement(community_id_hash)` — 获取节点准入策略
- `is_community_configured(community_id_hash)` — 社区是否已配置
- `log_count_for(community_id_hash)` — 获取社区日志数

## 内部 Helper 函数

- `verify_action_log_signature(...)` — 重建签名消息并验证 Ed25519 签名
- `ensure_bot_owner(who, community_id_hash)` — 检查调用者是否为社区绑定 Bot 的 owner
- `check_cooldown(operator, community_id_hash, user_hash)` — 检查声誉变更冷却
- `set_cooldown(operator, community_id_hash, user_hash)` — 设置冷却时间戳
- `ensure_sequence_monotonic(community_id_hash, sequence)` — 确保 sequence 严格递增
- `ensure_active_bot_owner(who, community_id_hash)` — ensure_bot_owner + is_bot_active 组合检查
- `do_modify_reputation(who, community_id_hash, user_hash, delta, is_award)` — award/deduct 共享逻辑

## 相关模块

- [primitives/](../primitives/) — ActionType、ConfigUpdateAction、NodeRequirement、WarnAction、SubscriptionTier、TierFeatureGate
- [registry/](../registry/) — Bot 注册（BotRegistryProvider 实现，查询 bot_owner/bot_public_key）
- [subscription/](../subscription/) — 订阅管理（SubscriptionProvider 实现，tier gate 查询）
- [consensus/](../consensus/) — 节点共识（查询社区准入策略）

## 审计历史

### Round 1 (2026-03)

**发现 8 项，修复 8 项:**

| ID | 级别 | 描述 | 状态 |
|:---:|:---:|------|:---:|
| H1 | High | `batch_submit_logs` 固定 weight 不随批量大小缩放 | ✅ 修复 |
| M1 | Medium | `update_community_config` 双重读取 CommunityConfigs | ✅ 修复 |
| M2 | Medium | `language` 字段无格式验证 | ✅ 修复 |
| M3 | Medium | `submit_action_log`/`batch_submit_logs` 不要求 Bot owner | ✅ 修复 |
| M4 | Medium | `sequence` 无单调递增校验（可重放） | ✅ 修复 |
| L2 | Low | Cargo.toml 死依赖 `log` | ✅ 修复 |
| L3 | Low | `try-runtime` feature 不完整 | ✅ 修复 |
| L4 | Low | 死错误码 `ConfigNotFound` | ✅ 修复 |

**记录未修复:**
- L6: `clear_expired_logs` 对 Free 层级路径为死代码（订阅降级场景合理）
- L7: 所有 extrinsic weight 硬编码，无 WeightInfo trait（需完整 benchmark 框架）

**测试:** 30 → 48 tests, cargo test 48/48 ✅

### Round 2 (2026-03)

**发现 3 项，修复 3 项:**

| ID | 级别 | 描述 | 状态 |
|:---:|:---:|------|:---:|
| M1-R2 | Medium | 6 个 Bot 操作 extrinsic 不检查 `is_bot_active` — 停用 Bot 仍可操作 | ✅ 修复 |
| M2-R2 | Medium | `ReputationCooldowns` 存储无清理机制 — 无界增长 | ✅ 修复 |
| L1-R2 | Low | `award_reputation` / `deduct_reputation` 近完全重复代码 | ✅ 修复 |

**新增 extrinsic (1):** `cleanup_expired_cooldowns` (call_index 9)
**新增错误 (2):** `BotNotActive`, `CooldownNotExpired`
**新增事件 (1):** `CooldownCleaned`
**新增 helper (2):** `ensure_active_bot_owner`, `do_modify_reputation`

**记录未修复:**
- L2-R2: 所有 extrinsic weight 硬编码，无 WeightInfo trait（需完整 benchmark 框架）

**测试:** 48 → 59 tests, cargo test 59/59 ✅

### Round 3 (2026-03)

**发现 5 项，修复 5 项:**

| ID | 级别 | 描述 | 状态 |
|:---:|:---:|------|:---:|
| M1-R3 | Medium | `cleanup_expired_cooldowns` 复用 `NoLogsToClear` 错误码 — 语义混淆 | ✅ 修复 |
| M2-R3 | Medium | `batch_submit_logs` 硬编码批量上限 50 — 应为可配置常量 | ✅ 修复 |
| M3-R3 | Medium | `clear_expired_logs` 硬编码 14400 blocks/day — 区块时间变更时计算错误 | ✅ 修复 |
| L1-R3 | Low | 缺少 `integrity_test` — Config 参数无编译期校验 | ✅ 修复 |
| L2-R3 | Low | Enterprise 层级永久保留路径无测试覆盖 | ✅ 修复 |

**新增 Config 参数 (2):** `MaxBatchSize`, `BlocksPerDay`
**新增错误 (1):** `CooldownNotFound`
**新增 hooks (1):** `integrity_test` (校验 MaxLogsPerCommunity, MaxReputationDelta, MaxBatchSize, BlocksPerDay > 0)

**记录未修复:**
- L2-R2: 所有 extrinsic weight 硬编码，无 WeightInfo trait（需完整 benchmark 框架）

**测试:** 59 → 64 tests, cargo test 64/64 ✅
