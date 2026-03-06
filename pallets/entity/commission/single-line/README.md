# pallet-commission-single-line

单线收益插件 — 基于 Entity 级消费注册顺序的上下线返佣。

---

## 核心概念

每个 Entity 维护一条**消费单链**，用户首次消费时按顺序加入。买家从上线（在其之前加入的人）和下线（在其之后加入的人）的消费中获得返佣。

```
消费单链（按首次消费时间排列）：

  ← 上线方向                           下线方向 →
  User1 ── User2 ── User3 ── User4 ── User5 ── User6

  User4 消费 10,000 NEX，upline_rate = 100 (1%), downline_rate = 100 (1%)：

  上线收益 ──────────────────────────────┐
    User3  ← 10,000 × 1% = 100 NEX      │ 向前遍历
    User2  ← 10,000 × 1% = 100 NEX      │ base_upline_levels 层
    User1  ← 10,000 × 1% = 100 NEX      │
  ──────────────────────────────────────┘

  下线收益 ──────────────────────────────┐
    User5  ← 10,000 × 1% = 100 NEX      │ 向后遍历
    User6  ← 10,000 × 1% = 100 NEX      │ base_downline_levels 层
  ──────────────────────────────────────┘
```

**特点：**
- 无需推荐关系，首次消费自动入链（幂等）
- 早期消费者拥有更多下线 → 更多被动收益
- 层数随累计收益动态增长，激励持续消费

---

## 层数计算

```
base = LevelBasedLevels[买家等级] ?? config.base_*_levels
extra = total_earned / level_increment_threshold      （封顶 255）

实际上线层数 = min(base + extra, max_upline_levels)
实际下线层数 = min(base + extra, max_downline_levels)
```

每层佣金 = `order_amount × rate / 10,000`，受 `remaining` 余额封顶。

---

## 分段存储

单链采用分段存储，每段容量 `MaxSingleLineLength`（推荐 200）。段满自动创建新段，总段数受 `MaxSegmentCount` 限制。

```
Segment 0: [User0, User1, ..., User199]    ← 200 accounts
Segment 1: [User200, User201, ..., User399] ← 200 accounts
Segment 2: [User400, User401, ..., User450] ← 51 accounts (partial)

global_index = segment_id × MaxSingleLineLength + local_position
```

跨段遍历由 `process_upline` / `process_downline` 自动处理，按需加载段数据。

---

## 数据结构

### SingleLineConfig

| 字段 | 类型 | 说明 | 约束 |
|------|------|------|------|
| `upline_rate` | `u16` | 上线收益率（基点） | ≤ 1000 (10%) |
| `downline_rate` | `u16` | 下线收益率（基点） | ≤ 1000 (10%) |
| `base_upline_levels` | `u8` | 基础上线层数 | ≤ max_upline_levels |
| `base_downline_levels` | `u8` | 基础下线层数 | ≤ max_downline_levels |
| `level_increment_threshold` | `Balance` | 每增加此收益额增加 1 层 | 0 = 不动态增长 |
| `max_upline_levels` | `u8` | 上线层数上限 | — |
| `max_downline_levels` | `u8` | 下线层数上限 | — |

> **总佣金率约束：** `upline_rate × max_upline_levels + downline_rate × max_downline_levels ≤ MaxTotalRateBps`

### LevelBasedLevels

按会员等级自定义层数，覆盖 `base_*_levels`。设置时校验 ≤ 对应 `max_*_levels`。

```rust
pub struct LevelBasedLevels {
    pub upline_levels: u8,    // 不得超过 config.max_upline_levels
    pub downline_levels: u8,  // 不得超过 config.max_downline_levels
}
```

### PendingConfigChange

延迟生效的配置变更。字段与 `SingleLineConfig` 相同，额外包含 `apply_after: BlockNumber`。

### ConfigChangeLogEntry

每次配置变更自动追加的审计日志，包含完整配置快照和 `block_number`。

### EntitySingleLineStatsData

Entity 级统计（每次佣金计算产生输出时自动更新）。

| 字段 | 说明 |
|------|------|
| `total_orders` | 产生过佣金输出的订单数 |
| `total_upline_payouts` | 上线佣金发放次数 |
| `total_downline_payouts` | 下线佣金发放次数 |

---

## Config 常量

| 常量 | 类型 | 说明 | 推荐值 |
|------|------|------|--------|
| `MaxSingleLineLength` | `u32` | 每段最大账户数 | 200 |
| `ConfigChangeDelay` | `BlockNumber` | 配置变更延迟区块数 | 视出块时间而定 |
| `MaxSegmentCount` | `u32` | 单个 Entity 最大段数 | 1000 |
| `MaxTotalRateBps` | `u32` | 总佣金率上限（基点²） | 10000~100000 |

---

## Storage

| 存储项 | 键 | 值 | 说明 |
|--------|-----|-----|------|
| `SingleLineConfigs` | `entity_id` | `SingleLineConfig` | 单线配置 |
| `SingleLineSegments` | `(entity_id, seg_id)` | `BoundedVec<AccountId>` | 分段单链 |
| `SingleLineSegmentCount` | `entity_id` | `u32` | 段数 |
| `SingleLineIndex` | `(entity_id, account)` | `u32` | 全局位置索引 |
| `SingleLineCustomLevelOverrides` | `(entity_id, level_id)` | `LevelBasedLevels` | 等级层数覆盖 |
| `SingleLineEnabled` | `entity_id` | `bool` | 启用状态（默认 true） |
| `PendingConfigChanges` | `entity_id` | `PendingConfigChange` | 待生效配置 |
| `RemovedMembers` | `(entity_id, account)` | `bool` | 逻辑移除标记 |
| `ConfigChangeLogs` | `(entity_id, idx)` | `ConfigChangeLogEntry` | 审计日志 |
| `ConfigChangeLogCount` | `entity_id` | `u32` | 日志计数 |
| `EntitySingleLineStats` | `entity_id` | `EntitySingleLineStatsData` | Entity 统计 |

---

## Extrinsics

### 配置管理

| idx | 方法 | 权限 | 说明 |
|-----|------|------|------|
| 0 | `set_single_line_config` | Owner/Admin | 设置完整配置 + 审计日志 |
| 1 | `clear_single_line_config` | Owner/Admin | 清除配置，级联清理所有 LevelOverrides |
| 2 | `update_single_line_params` | Owner/Admin | 部分更新（仅传需改字段），含交叉校验 |
| 5 | `force_set_single_line_config` | Root | 绕过 Entity 状态检查 |
| 6 | `force_clear_single_line_config` | Root | 绕过 Entity 状态检查 |

### 配置变更延迟

| idx | 方法 | 权限 | 说明 |
|-----|------|------|------|
| 10 | `schedule_config_change` | Owner/Admin | 提交新配置，`ConfigChangeDelay` 区块后可生效 |
| 11 | `apply_pending_config` | **Anyone** | 过延迟期后任何人可触发生效 |
| 12 | `cancel_pending_config` | Owner/Admin | 取消待生效配置 |

流程：`schedule → 等待 N 区块 → apply`（或随时 `cancel`）

### 等级层数管理

| idx | 方法 | 权限 | 说明 |
|-----|------|------|------|
| 3 | `set_level_based_levels` | Owner/Admin | 设置等级自定义层数（校验 ≤ max） |
| 4 | `remove_level_based_levels` | Owner/Admin | 移除指定等级的覆盖 |

### 运维控制

| idx | 方法 | 权限 | 说明 |
|-----|------|------|------|
| 8 | `pause_single_line` | Owner/Admin | 暂停佣金计算（不影响链数据） |
| 9 | `resume_single_line` | Owner/Admin | 恢复佣金计算 |
| 7 | `force_reset_single_line` | Root | 分批清理单链（`limit` 控制每批段数） |
| 13 | `force_remove_from_single_line` | Root | 逻辑移除成员（遍历时跳过） |
| 14 | `force_restore_to_single_line` | Root | 恢复已移除的成员 |

---

## 权限模型

```
Owner/Admin（需 COMMISSION_MANAGE 权限）
├── 配置 CRUD（0, 1, 2）
├── 等级层数管理（3, 4）
├── 暂停/恢复（8, 9）
├── 调度/取消配置变更（10, 12）
│
│   全部受 EntityActive + EntityNotLocked 守卫

Root（Sudo/治理）
├── 强制配置（5, 6）         ← 无 Entity 状态检查
├── 分批重置（7）
├── 成员移除/恢复（13, 14）

Anyone
└── 应用到期配置（11）       ← 仅检查延迟期
```

---

## 佣金计算流程

```
CommissionPlugin::calculate(entity_id, buyer, order_amount, remaining, modes)
│
├── 前置检查
│   ├── Entity 是否活跃？
│   ├── 单线是否启用？
│   └── 配置是否存在？
│
├── 预读 buyer_in_chain（避免末尾重复读取）
│
├── 计算有效层数
│   └── effective_base_levels() → 等级覆盖 ?? config.base
│
├── SINGLE_LINE_UPLINE 启用时
│   └── process_upline()
│       ├── 从 buyer 位置向前遍历
│       ├── 跳过：banned / unactivated / inactive / removed 成员
│       ├── 佣金 = order_amount × rate / 10000，受 remaining 封顶
│       └── 跨段边界时自动加载新段
│
├── SINGLE_LINE_DOWNLINE 启用时
│   └── process_downline()（同理向后遍历）
│
├── 更新 EntitySingleLineStats
│
└── buyer 未在链中 → 自动加入（幂等）
    └── 段满 → 创建新段（受 MaxSegmentCount 限制）
```

---

## 成员过滤

遍历时以下成员被跳过（消耗 depth 但不发佣金）：

| 条件 | 来源 |
|------|------|
| `is_banned` | MemberProvider |
| `!is_activated` | MemberProvider |
| `!is_member_active` | MemberProvider |
| `RemovedMembers` | 本模块 `force_remove` |

---

## Query 辅助函数

| 函数 | 返回 | 说明 |
|------|------|------|
| `single_line_length(entity_id)` | `u32` | 单链总长度 |
| `single_line_remaining_capacity(entity_id)` | `u32` | 当前段剩余容量 |
| `user_position(entity_id, account)` | `Option<u32>` | 用户全局位置 |
| `user_effective_levels(entity_id, account)` | `Option<(u8, u8)>` | 有效上线/下线层数 |
| `is_single_line_enabled(entity_id)` | `bool` | 是否启用 |
| `preview_single_line_commission(entity_id, buyer, amount)` | `Vec<CommissionOutput>` | 预览佣金分配（无副作用） |

---

## Events

| 事件 | 触发时机 |
|------|----------|
| `SingleLineConfigUpdated` | 配置设置/更新/应用 |
| `SingleLineConfigCleared` | 配置清除 |
| `AddedToSingleLine` | 用户加入单链 |
| `SingleLineJoinFailed` | 加入失败（段数达上限） |
| `LevelBasedLevelsUpdated` | 等级覆盖设置 |
| `LevelBasedLevelsRemoved` | 等级覆盖移除 |
| `SingleLinePaused` | 暂停 |
| `SingleLineResumed` | 恢复 |
| `SingleLineReset` | 本批次清除完成（含 removed_count） |
| `SingleLineResetCompleted` | 全部段清除完成 |
| `NewSegmentCreated` | 新段创建 |
| `AllLevelOverridesCleared` | 所有等级覆盖被级联清除 |
| `ConfigChangeScheduled` | 配置变更已调度 |
| `PendingConfigApplied` | 待生效配置已应用 |
| `PendingConfigCancelled` | 待生效配置已取消 |
| `MemberRemovedFromSingleLine` | 成员逻辑移除 |
| `MemberRestoredToSingleLine` | 成员恢复 |

---

## Errors

| 错误 | 触发条件 |
|------|----------|
| `InvalidRate` | rate > 1000 |
| `InvalidLevels` | upline_levels 和 downline_levels 同时为 0 |
| `BaseLevelsExceedMax` | base > max |
| `RatesTooHigh` | 总佣金率超 MaxTotalRateBps |
| `LevelOverrideExceedsMax` | 等级覆盖层数 > config max |
| `EntityNotFound` | entity_owner 返回 None |
| `NotEntityOwnerOrAdmin` | 非 Owner 且无 COMMISSION_MANAGE |
| `ConfigNotFound` | 清除/更新时配置不存在 |
| `NothingToUpdate` | update_params 所有参数均为 None |
| `EntityLocked` | Entity 已锁定 |
| `EntityNotActive` | Entity 未激活 |
| `SingleLineIsPaused` | 重复暂停 |
| `SingleLineNotPaused` | 未暂停时恢复 |
| `PendingConfigAlreadyExists` | 已有待生效配置 |
| `PendingConfigNotFound` | 无待生效配置 |
| `PendingConfigNotReady` | 延迟期未到 |
| `MemberNotInSingleLine` | 移除/恢复时成员不在链中 |
| `MaxSegmentCountReached` | 段数达上限 |

---

## Trait 实现

| Trait | 说明 |
|-------|------|
| `CommissionPlugin<AccountId, Balance>` | NEX 返佣计算入口 |
| `TokenCommissionPlugin<AccountId, TB>` | Token 多资产返佣计算 |
| `SingleLinePlanWriter` | 治理集成（复用 `validate_config` 校验） |

---

## 分批重置

`force_reset_single_line(entity_id, limit)` 从末尾段开始逆序清理：

```
调用 1: limit=1 → 清理 Segment 2 (最后段), 发射 SingleLineReset
调用 2: limit=1 → 清理 Segment 1, 发射 SingleLineReset
调用 3: limit=1 → 清理 Segment 0, 发射 SingleLineReset + SingleLineResetCompleted
```

- 大链分多次调用，不阻塞区块
- `limit = u32::MAX` 一次性全清（仅适用于小链）
- 每段清理包括：删除段数据 + 移除所有成员的 `SingleLineIndex`

---

## 安全机制

| 机制 | 说明 |
|------|------|
| **rate 上限** | 单项 ≤ 1000 基点（10%） |
| **总佣金率上限** | rate × levels 总和 ≤ MaxTotalRateBps |
| **base ≤ max** | 基础层数不得超过最大层数 |
| **等级覆盖校验** | set_level_based_levels 时 ≤ config max |
| **运行时 clamp** | 有效层数 = min(base + extra, max) |
| **extra_levels 封顶** | .min(255) 防 u8 溢出 |
| **saturating 算术** | 全部使用饱和运算 |
| **段数上限** | MaxSegmentCount 防无限存储增长 |
| **配置变更延迟** | 防止瞬时费率操控 |
| **成员逻辑移除** | force_remove 不破坏链结构 |
| **Entity 状态守卫** | 所有管理操作检查 active + not locked |
| **WeightInfo** | 所有 extrinsic 使用 T::WeightInfo 计算 weight |
| **StorageVersion** | v2，含 integrity_test |

---

## 依赖

| Crate | 用途 |
|-------|------|
| `frame-support` / `frame-system` | Substrate 框架 |
| `sp-runtime` | 运行时原语 |
| `pallet-entity-common` | EntityProvider / AdminPermission |
| `pallet-commission-common` | CommissionPlugin / MemberProvider / CommissionOutput |

---

## 测试

163 个单元测试，覆盖：
- 配置 CRUD + 边界值 + 权限拒绝
- 佣金计算（上线/下线/动态层数/remaining 封顶/跨段遍历）
- 成员过滤（banned / unactivated / frozen / removed）
- 配置变更延迟（schedule / apply / cancel / 过早 apply）
- 分批重置（部分/完整/空链）
- 成员移除与恢复
- 等级覆盖（设置/移除/校验超限/运行时 clamp）
- Plugin 集成（NEX + Token）
- PlanWriter 集成
- 审计日志 + Entity 统计
- 段自动扩展 + 段数上限
