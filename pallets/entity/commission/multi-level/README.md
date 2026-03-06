# pallet-commission-multi-level

多级分销返佣插件 — N 层推荐链 + 三维激活条件 + 总佣金上限。

作为 `pallet-commission-core` 的 `CommissionPlugin` 插件运行，支持 NEX / EntityToken 双轨佣金。

---

## 架构定位

```
pallet-commission-core (调度中心)
  ├── MultiLevelPlugin      ──→ 本模块 (NEX)
  ├── TokenMultiLevelPlugin ──→ 本模块 (Token)
  ├── ReferralPlugin        ──→ pallet-commission-referral
  ├── LevelDiffPlugin       ──→ pallet-commission-level-diff
  └── SingleLinePlugin      ──→ pallet-commission-single-line
```

触发条件：`enabled_modes` 含 `MULTI_LEVEL` 标志位 **且** Entity 已激活 **且** Entity 未暂停 **且** `MultiLevelConfigs` 存在。

---

## 数据结构（8 个）

### MultiLevelTier — 层级配置

```rust
pub struct MultiLevelTier {
    pub rate: u16,              // 佣金比率（基点制，10000 = 100%），0 = 占位层
    pub required_directs: u32,  // 有效直推人数（最大 10000），0 = 无要求
    pub required_team_size: u32,// 最低团队规模（最大 1_000_000），0 = 无要求
    pub required_spent: u128,   // 最低累计消费 USDT（精度 10^6，最大 10^18），0 = 无要求
}
```

### MultiLevelConfig — 多级分销配置

```rust
pub struct MultiLevelConfig<MaxLevels: Get<u32>> {
    pub levels: BoundedVec<MultiLevelTier, MaxLevels>,  // 各层配置，索引 0 = L1
    pub max_total_rate: u16,                            // 佣金总和上限（基点制）
}
```

### MultiLevelStatsData — 个人佣金统计

| 字段 | 类型 | 说明 |
|------|------|------|
| `total_earned` | `u128` | 累计佣金 |
| `total_orders` | `u32` | 参与订单数 |
| `last_commission_block` | `u32` | 最后佣金区块号 |

### EntityStatsData — Entity 级佣金统计

| 字段 | 类型 | 说明 |
|------|------|------|
| `total_distributed` | `u128` | 累计分发佣金 |
| `total_orders` | `u32` | 订单总数 |
| `total_distribution_entries` | `u32` | 累计分发条目数（非去重受益人数） |

### ActivationProgress — 激活进度

| 字段 | 类型 | 说明 |
|------|------|------|
| `level` | `u8` | 层级编号 (1-indexed) |
| `activated` | `bool` | 是否已激活 |
| `directs_current` / `directs_required` | `u32` | 直推当前/要求 |
| `team_current` / `team_required` | `u32` | 团队当前/要求 |
| `spent_current` / `spent_required` | `u128` | 消费当前/要求 |

### ConfigChangeEntry — 审计日志条目

| 字段 | 类型 | 说明 |
|------|------|------|
| `who` | `AccountId` | 操作者（Root 操作使用 `entity_account`） |
| `block_number` | `u32` | 区块号 |
| `change_type` | `ConfigChangeType` | 变更类型 |

### ConfigChangeType — 变更类型枚举（15 个变体）

| 变体 | 来源 |
|------|------|
| `SetConfig` | Owner/Admin 设置 |
| `ClearConfig` | Owner/Admin 清除 |
| `UpdateParams` | 部分更新 |
| `AddTier { index }` | 插入层级 |
| `RemoveTier { index }` | 移除层级 |
| `Pause` / `Resume` | Owner/Admin 暂停/恢复 |
| `ForceSet` / `ForceClear` | Root 强制设置/清除 |
| `ForcePause` / `ForceResume` | Root 强制暂停/恢复 |
| `PendingScheduled` / `PendingApplied` / `PendingCancelled` | 延迟配置生命周期 |
| `PendingAutoApplied` | `on_initialize` 自动应用 |
| `GovernanceSet` / `GovernanceClear` | PlanWriter 治理路径 |

### PendingConfigEntry — 待生效配置

| 字段 | 类型 | 说明 |
|------|------|------|
| `config` | `MultiLevelConfigOf<T>` | 待生效的配置 |
| `effective_at` | `u32` | 生效区块号 |
| `scheduled_by` | `AccountId` | 调度者 |

---

## 激活条件

`check_tier_activation` 对推荐人执行三维 **AND** 检查，值为 0 的条件自动跳过：

| 条件 | 数据来源 | 上界 |
|------|----------|------|
| `required_directs` | `MemberProvider::get_member_stats().0` | 10,000 |
| `required_team_size` | `MemberProvider::get_member_stats().1` | 1,000,000 |
| `required_spent` | `MemberProvider::get_member_spent_usdt()` | 10^18 |

> **懒加载:** 仅在需要时读取 `get_member_stats` / `get_member_spent_usdt`，避免不必要的 DB 读取。

**不满足条件时：** 跳过该层推荐人，遍历继续向上。被跳过的佣金留在 `remaining` 返还 core。

---

## 核心算法 `process_multi_level`

逐层遍历推荐链（buyer → L1 referrer → L2 referrer → ...），每层执行：

1. **rate = 0** → 占位层，跳过并向上移动 referrer
2. **无推荐人** → 终止
3. **循环检测**（`BTreeSet<AccountId>` 含 buyer）→ 命中则终止
4. **非会员** (`is_member` = false) → 跳过，继续下一层
5. **被封禁 / 未激活 / 已冻结** (`is_banned` / `!is_activated` / `!is_member_active`) → 跳过
6. **激活条件不满足** (`check_tier_activation`) → 跳过，继续下一层
7. **计算佣金** `commission = order_amount × rate / 10000`，取 `min(commission, remaining)`
8. **总额上限检查** — 累计超过 `max_total_rate × order_amount / 10000` 时截断最后一笔并终止

### 终止 vs 跳过

| 情况 | 行为 |
|------|------|
| rate=0 / 非会员 / 被封禁 / 未激活 / 已冻结 / 激活条件不满足 / 佣金精度截断为 0 | **跳过**，继续 |
| 无推荐人 / 循环检测 / remaining=0 / 超总额上限 | **终止** |

---

## 待生效配置自动应用

`on_initialize` 每区块自动检查 `PendingConfigQueue`，对已到达 `effective_at` 的条目执行应用：

| 参数 | 值 | 说明 |
|------|------|------|
| `MAX_AUTO_APPLY` | 5 | 每区块最多检查/应用 5 个条目 |
| `PendingConfigQueue` 容量 | 100 | `BoundedVec<u64, ConstU32<100>>`，满时返回 `PendingQueueFull` |

**处理逻辑：**
- 条目就绪 + Entity 未锁定 → 应用配置 + 记录 `PendingAutoApplied` 审计日志
- 条目就绪 + Entity 已锁定 → 跳过，保留在队列
- 条目未就绪（`current_block < effective_at`）→ 跳过
- 孤立条目（`PendingConfigs` 已被手动删除）→ 自动清理

**Weight 计算：** 按 `checked`（读取开销）和 `applied`（写入开销）分别计费，而非固定值。

---

## 配置示例

### 3 层递减

```
L1: rate=1000 (10%), directs=0    ← 无门槛
L2: rate=500  (5%),  directs=3    ← 需 ≥3 直推
L3: rate=200  (2%),  directs=5    ← 需 ≥5 直推
max_total_rate = 2000 (20%)
```

买家下单 10,000 NEX（Alice → Bob → Carol → Dave）：

| 层级 | 推荐人 | 满足？ | 佣金 | 累计 |
|------|--------|--------|------|------|
| L1 | Bob (5推) | ✅ | 1,000 | 1,000 |
| L2 | Carol (4推) | ✅ | 500 | 1,500 |
| L3 | Dave (6推) | ✅ | 200 | 1,700 |

总佣金 1,700 (17%)，remaining 8,300。若 Carol 仅 1 直推 → 跳过，Dave 仍获 L3，总佣金 1,200。

### 5 层 + max_total_rate 截断

```
L1=800(8%) L2=500(5%) L3=300(3%) L4=200(2%) L5=100(1%)
max_total_rate = 1500 (15%)
```

全部合格：L1=800, L2=500, L3=**200**（截断），L4/L5 不执行。总佣金 1,500。

---

## Pallet API

### Config（6 个关联类型）

| 关联类型 | 约束 | 说明 |
|----------|------|------|
| `RuntimeEvent` | `From<Event<Self>>` | 事件类型 |
| `MemberProvider` | `MemberProvider<AccountId>` | 推荐链 + 统计 + USDT 消费数据 |
| `EntityProvider` | `EntityProvider<AccountId>` | 实体查询（Owner/Admin/锁定/激活状态） |
| `MaxMultiLevels` | `Get<u32>`，1 ≤ x ≤ 100 | 最大层级数（默认 15） |
| `ConfigChangeDelay` | `Get<u32>`，≥ 1 | 配置延迟生效区块数 |
| `WeightInfo` | `WeightInfo` | 权重接口（13 个函数） |

`integrity_test` 校验 `MaxMultiLevels ∈ [1, 100]`、`ConfigChangeDelay ≥ 1`。

`STORAGE_VERSION = 2`

### Storage（8 项）

| 名称 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `MultiLevelConfigs` | `StorageMap<u64, MultiLevelConfigOf<T>>` | `None` | Entity 多级分销配置 |
| `GlobalPaused` | `StorageMap<u64, bool>` | `false` | 多级分销暂停开关 |
| `MemberMultiLevelStats` | `StorageDoubleMap<u64, AccountId, MultiLevelStatsData>` | `Default` | 个人佣金统计 |
| `EntityMultiLevelStats` | `StorageMap<u64, EntityStatsData>` | `Default` | Entity 级佣金统计 |
| `ConfigChangeLogCount` | `StorageMap<u64, u32>` | `0` | 审计日志计数 |
| `ConfigChangeLogs` | `StorageDoubleMap<u64, u32, ConfigChangeEntry<T>>` | `None` | 审计日志条目（环形缓冲，MAX=1000） |
| `PendingConfigs` | `StorageMap<u64, PendingConfigEntry<T>>` | `None` | 待生效配置 |
| `PendingConfigQueue` | `StorageValue<BoundedVec<u64, ConstU32<100>>>` | `[]` | 待生效配置队列（供 `on_initialize` 遍历） |

### Extrinsics（15 个，call_index 0–14）

| idx | 名称 | Origin | EntityLocked | 说明 |
|-----|------|--------|:---:|------|
| 0 | `set_multi_level_config` | Owner/Admin | ✅ | 设置完整多级分销配置 |
| 1 | `clear_multi_level_config` | Owner/Admin | ✅ | 清除配置（需已存在） |
| 2 | `force_set_multi_level_config` | Root | — | 强制设置配置 |
| 3 | `force_clear_multi_level_config` | Root | — | 强制清除配置（幂等） |
| 4 | `update_multi_level_params` | Owner/Admin | ✅ | 部分更新 max_total_rate 和/或指定层 |
| 5 | `add_tier` | Owner/Admin | ✅ | 在指定位置插入新层级 |
| 6 | `remove_tier` | Owner/Admin | ✅ | 移除层级（至少保留 1 层） |
| 7 | `pause_multi_level` | Owner/Admin | — | 暂停多级分销 |
| 8 | `resume_multi_level` | Owner/Admin | — | 恢复多级分销（未暂停时返回 `MultiLevelNotPaused`） |
| 9 | `schedule_config_change` | Owner/Admin | ✅ | 调度延迟生效配置（加入 `PendingConfigQueue`） |
| 10 | `apply_pending_config` | **任何人** | ✅ | 手动应用已到期的待生效配置 |
| 11 | `cancel_pending_config` | Owner/Admin | — | 取消待生效配置 |
| 12 | `force_pause_multi_level` | Root | — | 强制暂停（紧急响应） |
| 13 | `force_resume_multi_level` | Root | — | 强制恢复 |
| 14 | `force_cleanup_entity` | Root | — | 清理 Entity 全部存储（参数 `member_count_hint` 仅影响 weight） |

**权限模型：**
- **Owner/Admin** — Entity Owner 或持有 `COMMISSION_MANAGE` 权限的 Admin
- **Root** — `force_*` 系列无视权限和锁定
- **任何人** — `apply_pending_config` 仅需到达生效区块 + Entity 未锁定

**校验规则（`validate_config` + `validate_tier`）：**
- `levels` 非空，每层 `rate ≤ 10000`
- `required_directs ≤ 10000`，`required_team_size ≤ 1_000_000`，`required_spent ≤ 10^18`
- `0 < max_total_rate ≤ 10000`

### Events（15 个）

| 事件 | 触发点 | 说明 |
|------|--------|------|
| `MultiLevelConfigUpdated` | set / force_set / PlanWriter | 配置已更新 |
| `MultiLevelConfigCleared` | clear / force_clear / PlanWriter | 配置已清除 |
| `TierUpdated` | update_multi_level_params | 单层配置已更新 |
| `MaxTotalRateUpdated` | update_multi_level_params | max_total_rate 已更新 |
| `TierInserted` | add_tier | 层级已插入 |
| `TierRemoved` | remove_tier | 层级已移除 |
| `MultiLevelPaused` | pause / force_pause | 已暂停 |
| `MultiLevelResumed` | resume / force_resume | 已恢复 |
| `RatesSumExceedsMax` | set / force_set / apply / PlanWriter | rates 总和超过 max_total_rate 警告（`rates_sum: u32`） |
| `ConfigDetailedChange` | set / force_set / apply / PlanWriter | 新旧配置对比（levels 数/max_rate） |
| `PendingConfigScheduled` | schedule_config_change | 待生效配置已调度 |
| `PendingConfigApplied` | apply / on_initialize | 待生效配置已应用 |
| `PendingConfigCancelled` | cancel_pending_config | 待生效配置已取消 |
| `MultiLevelCommissionDistributed` | update_stats | 佣金分发汇总（`total_amount`, `beneficiary_count`） |
| `EntityStorageCleaned` | force_cleanup_entity | Entity 存储已清理 |

### Errors（16 个）

| 错误 | 触发条件 |
|------|----------|
| `InvalidRate` | rate > 10000 或 max_total_rate 为 0 或 > 10000 |
| `EmptyLevels` | levels 数组为空 |
| `EntityNotFound` | entity_id 对应的实体不存在 |
| `NotEntityOwnerOrAdmin` | 非 Owner 且无 COMMISSION_MANAGE 权限 |
| `ConfigNotFound` | 配置不存在（clear/update/add/remove） |
| `EntityLocked` | 实体已被全局锁定 |
| `NothingToUpdate` | update 全 None |
| `TierIndexOutOfBounds` | tier_index 越界 |
| `TierLimitExceeded` | 添加后超 MaxMultiLevels |
| `MultiLevelIsPaused` | 已暂停，不可重复暂停 |
| `MultiLevelNotPaused` | 未暂停，不可恢复 |
| `PendingConfigExists` | 已有待生效配置 |
| `NoPendingConfig` | 无待生效配置 |
| `PendingConfigNotReady` | 当前区块 < effective_at |
| `InvalidDirects` | required_directs > 10000 |
| `InvalidTeamSize` | required_team_size > 1_000_000 |
| `InvalidSpent` | required_spent > 10^18 |
| `PendingQueueFull` | PendingConfigQueue 已满（容量 100） |

---

## Trait 实现

### CommissionPlugin / TokenCommissionPlugin

供 `pallet-commission-core` 通过 `type MultiLevelPlugin` / `type TokenMultiLevelPlugin` 调用。共用 `process_multi_level` 泛型算法，仅 Balance 类型不同。

计算前置检查链：`enabled_modes` 含 `MULTI_LEVEL` → Entity 已激活 → Entity 未暂停 → 配置存在 → `process_multi_level`。

### MultiLevelPlanWriter（治理路径）

| 方法 | 说明 |
|------|------|
| `set_multi_level(entity_id, rates, max_total_rate)` | 设置仅含 rate 的配置（激活条件全为 0） |
| `set_multi_level_full(entity_id, tiers, max_total_rate)` | 设置含完整激活条件的配置 |
| `clear_multi_level_config(entity_id)` | 清除配置（幂等） |

所有方法均：
- 校验 `EntityLocked`
- 复用 `validate_config` / `validate_tier` 校验参数
- 触发 `RatesSumExceedsMax` 警告检查
- 发出 `ConfigDetailedChange` 事件（含新旧 levels 数和 max_rate 对比）
- 记录 `GovernanceSet` / `GovernanceClear` 审计日志（使用 `entity_account` 标识）

### 公开辅助函数

| 函数 | 返回值 | 说明 |
|------|--------|------|
| `validate_tier(tier)` | `DispatchResult` | 校验单层 tier 参数上界 |
| `validate_config(levels, max_total_rate)` | `DispatchResult` | 校验完整配置参数 |
| `check_rates_sum_warning(entity_id, config)` | — | rates 总和超限时发出警告事件 |
| `record_change_log(entity_id, who, change_type)` | — | 写入审计日志（环形缓冲） |
| `update_stats(entity_id, outputs)` | — | 更新个人 + Entity 级统计，发出 `MultiLevelCommissionDistributed` |
| `get_activation_status(entity_id, account)` | `Vec<bool>` | 各层级激活状态 |
| `get_activation_progress(entity_id, account)` | `Vec<ActivationProgress>` | 激活进度（含当前值与要求值） |
| `get_recent_change_logs(entity_id, limit)` | `Vec<ConfigChangeEntry>` | 最近审计日志（逆序，最多 limit 条） |
| `preview_commission(entity_id, buyer, amount)` | `Vec<(AccountId, u128, u8)>` | 预览佣金分配（不扣款） |
| `is_paused(entity_id)` | `bool` | 是否暂停 |
| `cleanup_entity_storage(entity_id)` | — | 清理 Entity 全部 7 项存储 + 队列 |

---

## 边界安全

| 情况 | 处理 |
|------|------|
| 空 levels | `process_multi_level` 直接返回 |
| 链短于配置层数 | 无推荐人 → break，已分佣保留 |
| 全部不合格 | 佣金 = 0，remaining 不变 |
| 环形推荐链 | `BTreeSet<AccountId>` visited → break |
| level_idx > 255 | `.min(255) as u8` |
| 单层佣金 > remaining | `min(commission, remaining)` |
| 单层佣金精度截断为 0 | 跳过该层继续，remaining=0 时终止 |
| 累计超 max_total_rate | 截断最后一笔 |
| NEX / Token 隔离 | 泛型参数 `B`，独立 trait 调用 |
| 暂停 / Entity 未激活 | `calculate` / `preview_commission` 返空 |
| 审计日志无限增长 | 环形缓冲（MAX=1000），slot = count % 1000 |
| PendingConfigQueue 满 | `try_push` 失败 → `PendingQueueFull` |
| on_initialize 开销可控 | 每区块最多检查 5 个条目 |

---

## 权重（13 个函数）

| 函数 | ref_time | proof_size | 备注 |
|------|----------|------------|------|
| `set_multi_level_config(n)` | 35M + 2M×n | 3K | n = levels 数量 |
| `clear_multi_level_config` | 35M | 3K | |
| `update_multi_level_params` | 40M | 4K | |
| `add_tier` | 40M | 4K | |
| `remove_tier` | 38M | 4K | |
| `pause_multi_level` | 30M | 3K | |
| `resume_multi_level` | 30M | 3K | |
| `schedule_config_change(n)` | 40M + 2M×n | 4K | n = levels 数量 |
| `apply_pending_config` | 45M | 5K | |
| `cancel_pending_config` | 30M | 3K | |
| `force_pause_multi_level` | 25M | 2K | |
| `force_resume_multi_level` | 25M | 2K | |
| `force_cleanup_entity(m)` | 50M + 1M×m | 5K + 500×m | m = member_count_hint |

---

## Benchmarking

`benchmarking.rs` 使用 `frame_benchmarking::v2` 宏，覆盖 5 个 Root extrinsics：

| Benchmark | 参数 | 说明 |
|-----------|------|------|
| `force_set_multi_level_config` | `l: Linear<1, 15>` | 层级数缩放 |
| `force_clear_multi_level_config` | — | 含前置 seed |
| `force_pause_multi_level` | — | |
| `force_resume_multi_level` | — | 含前置暂停 |
| `force_cleanup_entity` | `m: Linear<0, 1000>` | member_count_hint 缩放 |

---

## 测试覆盖（167 个）

### Extrinsic 测试（56 个）

| 分类 | 数量 | 覆盖内容 |
|------|:---:|------|
| set_multi_level_config | 7 | Owner/Admin 设置、权限校验、rate 校验、Entity 不存在 |
| clear_multi_level_config | 3 | Owner 清除、不存在拒绝、权限拒绝 |
| force_set / force_clear | 5 | Root 操作、非 Root 拒绝、幂等清除 |
| update_multi_level_params | 7 | rate/tier/both 更新、NothingToUpdate/ConfigNotFound/OOB/InvalidRate |
| add_tier | 4 | 末尾追加、开头插入、超限拒绝、索引越界 |
| remove_tier | 3 | 移除中间层、最后一层拒绝、索引越界 |
| pause / resume | 5 | 暂停/恢复、重复暂停拒绝、未暂停恢复拒绝、Admin 操作 |
| schedule / apply / cancel | 6 | 调度/应用/取消、重复调度/未到期/无待生效拒绝 |
| force_pause / force_resume | 6 | Root 操作、非 Root 拒绝、已暂停/未暂停错误 |
| force_cleanup_entity | 3 | 全部存储清理、非 Root 拒绝、member_count_hint |
| EntityLocked 回归 | 5 | set/clear/add/remove/update 锁定拒绝 |
| is_paused 查询 | 1 | 暂停/恢复/查询状态 |
| apply_pending 锁定 | 2 | 锁定拒绝、未锁定正常 |

### 佣金计算测试（26 个）

| 分类 | 数量 | 覆盖内容 |
|------|:---:|------|
| 基础计算 | 4 | 3 层基础、总额截断、激活条件、循环检测 |
| 模式/配置边界 | 2 | 标志未启用返空、无配置返空 |
| 激活条件回归 | 3 | USDT vs NEX Balance、USDT 充足、三条件组合 |
| is_banned | 3 | 封禁跳过、非封禁正常、全封禁返空 |
| is_member | 2 | 非会员跳过、全部非会员返空 |
| Entity 激活 | 2 | 未激活跳过、激活正常 |
| 暂停跳过 | 1 | calculate 暂停返空 |
| 边界场景 | 3 | rate=0 占位层、链短于配置、TokenCommissionPlugin |
| apply_pending 事件 | 1 | 应用触发 ConfigDetailedChange + RatesSumExceedsMax |
| 冻结/未激活 | 3 | 冻结推荐人跳过、正常推荐人不受影响、preview 未激活返空 |
| 精度边界 | 2 | 小额佣金截断跳过（非终止）、remaining=0 正确终止 |

### PlanWriter 测试（18 个）

| 分类 | 数量 | 覆盖内容 |
|------|:---:|------|
| set_multi_level | 5 | 创建、rate 校验（multi/level）、层数上限、清除 |
| set_multi_level_full | 3 | 含激活条件、空 tiers 拒绝、无效 rate 拒绝 |
| validate_config 复用 | 2 | set 和 full 路径的校验一致性 |
| 审计日志 | 4 | GovernanceSet（set/full）、GovernanceClear、无配置不写日志 |
| ConfigDetailedChange | 3 | 新建/覆写/清除事件 |
| EntityLocked | 3 | set/full/clear 锁定拒绝（含未锁定正常） |
| 边界 | 2 | 无效 team_size/spent 拒绝（via validate_config） |
| 幂等 | 2 | clear 无配置不发事件、clear 有配置发事件 |

### 功能测试（32 个）

| 分类 | 数量 | 覆盖内容 |
|------|:---:|------|
| 审计日志 | 7 | set/clear/pause-resume/add-remove/update/force_set/force_clear 审计 |
| rates 警告 | 5 | 超限事件、未超限无事件、update/add/remove 警告 |
| 激活进度 | 2 | 无配置返空、含数据返回进度 |
| 佣金统计 | 3 | 统计更新、累加、空输出无操作 |
| 详细变更 | 2 | 首次设置旧值零、覆写显示旧值 |
| 预览 | 3 | 正常预览、无配置返空、暂停返空 |
| 激活状态 | 3 | 无配置/全通过/部分通过 |
| 最近日志 | 3 | 空返回、逆序、limit 限制 |
| tier 上界校验 | 5 | team_size/spent 越界拒绝（set/add/update/PlanWriter）、合法值通过 |
| 环形缓冲 | 1 | 日志回绕覆盖正确性 |
| 统计事件 | 1 | MultiLevelCommissionDistributed 事件 |

### PendingConfigQueue 测试（12 个）

| 分类 | 数量 | 覆盖内容 |
|------|:---:|------|
| 队列管理 | 3 | schedule 入队、cancel 出队、manual apply 出队 |
| on_initialize | 4 | 自动应用、跳过锁定 Entity、多 Entity 同时生效、孤立条目清理 |
| MAX_AUTO_APPLY | 1 | 8 个条目分两批处理（5+3） |
| PendingQueueFull | 1 | 101 个条目时报错 |
| force_cleanup | 3 | 清除队列条目、幂等性、member_count_hint |

---

## 审计修复历史

### Round 1–9（历史）

共 9 轮审计，修复 30+ 个问题，涵盖安全漏洞（USDT 误用、缺权限检查）、设计缺陷（PlanWriter 事件/校验缺失）、边界安全（精度截断、环形缓冲）、权重准确性等。所有发现均已修复。

### Round 10

| ID | 级别 | 描述 |
|----|------|------|
| R10-#1 | Medium | `force_set/clear` 缺审计日志 — 补充 `ForceSet`/`ForceClear` + `entity_account` |
| R10-#2 | Medium | `update/add/remove` 缺 rates_sum 警告 — 补充 `check_rates_sum_warning` |
| R10-#3 | Medium | tier 参数缺上界校验 — 新增 `validate_tier`（team_size ≤ 1M, spent ≤ 10^18） |
| R10-#4 | Medium | 缺存储清理机制 — 新增 `force_cleanup_entity` |
| R10-#6 | Medium | 缺 Root 紧急暂停 — 新增 `force_pause/resume_multi_level` |
| R10-#7 | Low | `update_stats` 不发事件 — 新增 `MultiLevelCommissionDistributed` |
| R10-#8 | Low | `total_beneficiaries` 命名误导 — 改为 `total_distribution_entries` |
| R10-#9 | Low | 缺审计日志查询 — 新增 `get_recent_change_logs` |
| R10-#10 | Low | PlanWriter 缺 `EntityLocked` 检查 — 补充 |
| R10-#14 | Medium | 待生效配置无自动应用 — 新增 `on_initialize` + `PendingConfigQueue` |

### Round 10 复查

| ID | 级别 | 描述 |
|----|------|------|
| A-1 | Bug | `on_initialize` weight 只计 applied 不计 checked — 修复为分别计费 |
| A-2 | Low | `ConfigChangeType::ForceCleanup` 死代码 — 已移除 |
| A-3 | Low | `resume_multi_level` 错误类型不一致 — 统一为 `MultiLevelNotPaused` |

### Round 11

| ID | 级别 | 描述 |
|----|------|------|
| B-1 | Medium | `force_cleanup_entity` weight 固定 — 参数化 `member_count_hint` |
| B-2 | Medium | PlanWriter 重复校验逻辑 — 复用 `validate_config`/`validate_tier` |
| B-3 | Low | PlanWriter 无审计日志 — 新增 `GovernanceSet`/`GovernanceClear` |
| B-4 | Low | PlanWriter 缺 `ConfigDetailedChange` 事件 — 补充 |
| B-5 | Low | `update_stats` 为 `pub(crate)` 不可被外部 crate 调用 — 提升为 `pub` |

所有发现均已修复 ✅

---

## 依赖

```
pallet-commission-multi-level
├── pallet-commission-common  (CommissionPlugin, MemberProvider, MultiLevelPlanWriter)
├── pallet-entity-common      (EntityProvider, AdminPermission)
├── frame-support / frame-system / sp-runtime
├── frame-benchmarking        (optional, runtime-benchmarks)
└── codec / scale-info
```

## License

MIT
