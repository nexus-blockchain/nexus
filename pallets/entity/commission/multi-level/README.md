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
    pub required_team_size: u32,// 最低团队规模，0 = 无要求
    pub required_spent: u128,   // 最低累计消费 USDT（精度 10^6），0 = 无要求
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
| `total_beneficiaries` | `u32` | 受益人次数 |

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
| `who` | `AccountId` | 操作者 |
| `block_number` | `u32` | 区块号 |
| `change_type` | `ConfigChangeType` | 变更类型 |

### ConfigChangeType — 变更类型枚举（12 个变体）

`SetConfig` · `ClearConfig` · `UpdateParams` · `AddTier { index }` · `RemoveTier { index }` · `ForceSet` · `ForceClear` · `Pause` · `Resume` · `PendingScheduled` · `PendingApplied` · `PendingCancelled`

### PendingConfigEntry — 待生效配置

| 字段 | 类型 | 说明 |
|------|------|------|
| `config` | `MultiLevelConfigOf<T>` | 待生效的配置 |
| `effective_at` | `u32` | 生效区块号 |
| `scheduled_by` | `AccountId` | 调度者 |

---

## 激活条件

`check_tier_activation` 对推荐人执行三维 **AND** 检查，值为 0 的条件自动跳过：

| 条件 | 数据来源 | 精度 |
|------|----------|------|
| `required_directs` | `MemberProvider::get_member_stats().0` | 有效直推人数 |
| `required_team_size` | `MemberProvider::get_member_stats().1` | 团队人数 |
| `required_spent` | `MemberProvider::get_member_spent_usdt()` | USDT × 10^6 |

> **懒加载 (L1-R3):** 仅在需要时读取 `get_member_stats` / `get_member_spent_usdt`，避免不必要的 DB 读取。

**不满足条件时：** 跳过该层推荐人，遍历继续向上。被跳过的佣金留在 `remaining` 返还 core。

---

## 核心算法 `process_multi_level`

逐层遍历推荐链（buyer → L1 referrer → L2 referrer → ...），每层执行：

1. **rate = 0** → 占位层，跳过并向上移动 referrer
2. **无推荐人** → 终止
3. **循环检测**（`BTreeSet<AccountId>` 含 buyer）→ 命中则终止
4. **非会员** (`is_member` = false) → 跳过，继续下一层
5. **被封禁或未激活** (`is_banned` / `!is_activated`) → 跳过，继续下一层
6. **激活条件不满足** (`check_tier_activation`) → 跳过，继续下一层
7. **计算佣金** `commission = order_amount × rate / 10000`，取 `min(commission, remaining)`
8. **总额上限检查** — 累计超过 `max_total_rate × order_amount / 10000` 时截断最后一笔并终止

> **M1-R6 优化:** `is_member`/`is_banned`（廉价 bool）在 `check_tier_activation`（可能 2 次 DB read）之前执行。

### 终止 vs 跳过

| 情况 | 行为 |
|------|------|
| rate=0 / 非会员 / 被封禁 / 未激活 / 激活条件不满足 | **跳过**，继续 |
| 无推荐人 / 循环检测 / remaining=0 / 超总额上限 | **终止** |

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
| `WeightInfo` | `WeightInfo` | 权重接口（10 个函数） |

`integrity_test` 校验 `MaxMultiLevels ∈ [1, 100]`、`ConfigChangeDelay ≥ 1`。

### Storage（7 项）

| 名称 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `MultiLevelConfigs` | `StorageMap<u64, MultiLevelConfigOf<T>>` | `None` | Entity 多级分销配置 |
| `GlobalPaused` | `StorageMap<u64, bool>` | `false` | 多级分销暂停开关 |
| `MemberMultiLevelStats` | `StorageDoubleMap<u64, AccountId, MultiLevelStatsData>` | `Default` | 个人佣金统计 |
| `EntityMultiLevelStats` | `StorageMap<u64, EntityStatsData>` | `Default` | Entity 级佣金统计 |
| `ConfigChangeLogCount` | `StorageMap<u64, u32>` | `0` | 审计日志计数 |
| `ConfigChangeLogs` | `StorageDoubleMap<u64, u32, ConfigChangeEntry<T>>` | `None` | 审计日志条目 |
| `PendingConfigs` | `StorageMap<u64, PendingConfigEntry<T>>` | `None` | 待生效配置 |

### Extrinsics（12 个，call_index 0–11）

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
| 8 | `resume_multi_level` | Owner/Admin | — | 恢复多级分销 |
| 9 | `schedule_config_change` | Owner/Admin | ✅ | 调度延迟生效配置 |
| 10 | `apply_pending_config` | **任何人** | ✅ | 应用已到期的待生效配置（M1-R9） |
| 11 | `cancel_pending_config` | Owner/Admin | — | 取消待生效配置 |

**权限模型：**
- **Owner/Admin** — Entity Owner 或持有 `COMMISSION_MANAGE` 权限的 Admin
- **Root** — `force_*` 系列无视权限和锁定
- **任何人** — `apply_pending_config` 仅需到达生效区块

**校验规则：** levels 非空，每层 `rate ≤ 10000`，`0 < max_total_rate ≤ 10000`。

### Events（13 个）

| 事件 | 触发点 | 说明 |
|------|--------|------|
| `MultiLevelConfigUpdated` | set / force_set / PlanWriter | 配置已更新 |
| `MultiLevelConfigCleared` | clear / force_clear / PlanWriter | 配置已清除 |
| `TierUpdated` | update_multi_level_params | 单层配置已更新 |
| `MaxTotalRateUpdated` | update_multi_level_params | max_total_rate 已更新 |
| `TierInserted` | add_tier | 层级已插入 |
| `TierRemoved` | remove_tier | 层级已移除 |
| `MultiLevelPaused` | pause_multi_level | 已暂停 |
| `MultiLevelResumed` | resume_multi_level | 已恢复 |
| `RatesSumExceedsMax` | set / force_set / apply_pending | rates 总和超过 max_total_rate 警告（rates_sum: u32） |
| `ConfigDetailedChange` | set / force_set / apply_pending | 新旧配置对比（levels 数/max_rate） |
| `PendingConfigScheduled` | schedule_config_change | 待生效配置已调度 |
| `PendingConfigApplied` | apply_pending_config | 待生效配置已应用 |
| `PendingConfigCancelled` | cancel_pending_config | 待生效配置已取消 |

### Errors（13 个）

| 错误 | 触发条件 |
|------|----------|
| `InvalidRate` | rate > 10000 或 max_total_rate 为 0 或 > 10000 |
| `EmptyLevels` | levels 数组为空 |
| `EntityNotFound` | entity_id 对应的实体不存在 |
| `NotEntityOwnerOrAdmin` | 非 Owner 且无 COMMISSION_MANAGE 权限 |
| `ConfigNotFound` | 配置不存在（clear/update/add/remove） |
| `EntityLocked` | 实体已被全局锁定 |
| `NothingToUpdate` | update 全 None / resume 时未暂停 |
| `TierIndexOutOfBounds` | tier_index 越界 |
| `TierLimitExceeded` | 添加后超 MaxMultiLevels |
| `MultiLevelIsPaused` | 已暂停，不可重复暂停 |
| `PendingConfigExists` | 已有待生效配置 |
| `NoPendingConfig` | 无待生效配置 |
| `PendingConfigNotReady` | 当前区块 < effective_at |

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
| `clear_multi_level_config(entity_id)` | 清除配置 |

所有方法均校验参数并 emit 事件。

### 查询辅助函数

| 函数 | 返回值 | 说明 |
|------|--------|------|
| `get_activation_status(entity_id, account)` | `Vec<bool>` | 各层级激活状态 |
| `get_activation_progress(entity_id, account)` | `Vec<ActivationProgress>` | 激活进度（含当前值与要求值） |
| `preview_commission(entity_id, buyer, amount)` | `Vec<(AccountId, u128, u8)>` | 预览佣金分配（不扣款） |
| `is_paused(entity_id)` | `bool` | 是否暂停 |
| `update_stats(entity_id, outputs)` | — | 更新个人 + Entity 级统计 |

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
| 单层佣金精度截断为 0 | 跳过该层继续（L1-R9），remaining=0 时终止 |
| 累计超 max_total_rate | 截断最后一笔 |
| NEX / Token 隔离 | 泛型参数 `B`，独立 trait 调用 |
| 暂停 / Entity 未激活 | `calculate` / `preview_commission` 返空 |

---

## 权重（10 个函数）

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

---

## 测试覆盖（119 个）

### Extrinsic 测试（46 个）

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
| EntityLocked 回归 | 5 | set/clear/add/remove/update 锁定拒绝 |
| ConfigNotFound 回归 | 2 | add_tier/remove_tier 无配置拒绝 |
| is_paused 查询 | 1 | 暂停/恢复/查询状态 |
| R9 回归 | 2 | apply_pending 锁定拒绝、未锁定正常 |

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
| R5 边界 | 3 | rate=0 占位层、链短于配置、TokenCommissionPlugin |
| apply_pending 事件 | 1 | 应用触发 ConfigDetailedChange + RatesSumExceedsMax |
| R8 回归 | 3 | 冻结推荐人跳过、正常推荐人不受影响、preview 未激活返空 |
| R9 回归 | 2 | 小额佣金截断跳过（非终止）、remaining=0 正确终止 |

### PlanWriter 测试（10 个）

| 分类 | 数量 | 覆盖内容 |
|------|:---:|------|
| set_multi_level | 5 | 创建、rate 校验（multi/level）、层数上限、清除 |
| set_multi_level_full | 3 | 含激活条件、空 tiers 拒绝、无效 rate 拒绝 |
| R9 回归 | 2 | clear 无配置不发事件、clear 有配置发事件 |

### 功能测试（24 个）

| 分类 | 数量 | 覆盖内容 |
|------|:---:|------|
| 审计日志 F2 | 5 | set/clear/pause-resume/add-remove/update 审计 |
| rates 警告 F4 | 2 | 超限事件、未超限无事件 |
| 激活进度 F5 | 2 | 无配置返空、含数据返回进度 |
| 佣金统计 F6/F13 | 3 | 统计更新、累加、空输出无操作 |
| 详细变更 F7 | 2 | 首次设置旧值零、覆写显示旧值 |
| 预览 F8 | 3 | 正常预览、无配置返空、暂停返空 |
| 激活状态 F11 | 3 | 无配置/全通过/部分通过 |
| R2 回归 | 6 | PlanWriter 事件、空 levels/零 rate 拒绝 |
| R7 回归 | 2 | force_set DetailedChange、cancel_pending 审计 |
| R9 回归 | 2 | rates_sum u32 不饱和、审计日志环形缓冲 |

---

## 审计修复历史

### Round 1

| ID | 级别 | 描述 |
|----|------|------|
| H1 | High | `required_spent` 误用 NEX Balance — 改用 `get_member_spent_usdt()` |
| H2 | High | PlanWriter 缺 rate 校验 |
| H3 | High | PlanWriter 超层数静默清空 — 改返回 `TooManyLevels` |
| M1 | Medium | 硬编码 Weight → WeightInfo trait |
| M2 | Medium | 激活条件零测试 → +5 回归测试 |
| L1 | Low | try-runtime feature 缺 sp-runtime |

### Round 2

| ID | 级别 | 描述 |
|----|------|------|
| M1-R2 | Medium | PlanWriter `set_multi_level` 不 emit 事件 |
| M2-R2 | Medium | PlanWriter `clear_multi_level_config` 无事件 |
| L1-R2 | Low | `set_multi_level_config` 接受空 levels |
| L2-R2 | Low | `max_total_rate = 0` 静默禁用佣金 |

### Round 3

| ID | 级别 | 描述 |
|----|------|------|
| L1-R3 | Low | `check_tier_activation` 不必要的 DB read — 懒加载优化 |
| L2-R3 | Low | Extrinsic 文档注释未反映 R2 校验 |

### Round 4

| ID | 级别 | 描述 |
|----|------|------|
| H1-R4 | High | `process_multi_level` 缺 `is_activated` 检查 |
| M1-R4 | Medium | Cargo.toml 缺 feature 传播 |

### Round 5

| ID | 级别 | 描述 |
|----|------|------|
| L1-R5 | Low | 死 dev-dependency `pallet-balances` |
| L2-R5 | Low | `rate=0` 占位层无测试覆盖 |
| L3-R5 | Low | 链短于配置层数无测试覆盖 |
| L4-R5 | Low | `TokenCommissionPlugin` 无测试覆盖 |

### Round 6

| ID | 级别 | 描述 |
|----|------|------|
| M1-R6 | Medium | `process_multi_level` 检查顺序次优 — 重排 is_member/is_banned 在前 |
| L1-R6 | Low | 死错误码 `EntityNotActive` 已移除 |
| L2-R6 | Low | 缺 update/add/remove 专用权重 |
| L3-R6 | Low | README 过时 |
| L4-R6 | Low | 缺 EntityLocked 回归测试 |
| L5-R6 | Low | 缺 ConfigNotFound 回归测试 |

### Round 7

| ID | 级别 | 描述 |
|----|------|------|
| M1-R7 | Medium | `force_set` 不发出 `ConfigDetailedChange` 事件 |
| M2-R7 | Medium | `get_activation_progress` 冗余 DB read — 预加载优化 |
| M3-R7 | Medium | `cancel_pending_config` 不记录审计日志 — 新增 `PendingCancelled` |
| L1–L4-R7 | Low | 4 个 extrinsic 使用错误的权重函数 |
| L5-R7 | Low | README 严重过时 |
| L6-R7 | Low | Cargo.toml 缺 pallet-commission-common feature 传播 |

所有发现均已修复 ✅

### Round 8

| ID | 级别 | 描述 |
|----|------|------|
| M1-R8 | Medium | `process_multi_level` 缺 `is_member_active` 检查 — 冻结/暂停推荐人仍获佣。修复: 添加检查 |
| M2-R8 | Medium | `preview_commission` 缺 `is_entity_active` 检查 — 与 `calculate()` 行为不一致。修复: 添加检查 |
| L1-R8 | Low | `sp-std` 依赖未使用（代码使用 `extern crate alloc`）。修复: 已移除 |
| L2-R8 | Low | `ConfigChangeType::ForceSet`/`ForceClear` 死代码 — `force_*` extrinsic 无 AccountId 无法记录审计日志。记录未修复 |

所有发现均已修复（L2-R8 记录未修复） ✅

### Round 9

| ID | 级别 | 描述 |
|----|------|------|
| M1-R9 | Medium | `apply_pending_config` 缺 `EntityLocked` 检查 — 锁定实体可被绕过。修复: 添加检查 |
| L1-R9 | Low | `process_multi_level` 小额订单 `actual.is_zero() → break` — 精度截断应跳过而非终止。修复: 区分 remaining=0（break）与 commission=0（continue） |
| L2-R9 | Low | `check_rates_sum_warning` 中 `rates_sum` 为 u16 饱和 — 事件报告值不准确。修复: 改为 u32 |
| L3-R9 | Low | `PlanWriter::clear_multi_level_config` 对不存在的配置发送 `Cleared` 事件。修复: 仅在配置存在时发事件 |
| L4-R9 | Low | `ConfigChangeLogs` 无限增长无清理。修复: 环形缓冲（MAX=1000），slot = count % 1000 |

所有发现均已修复 ✅

---

## 依赖

```
pallet-commission-multi-level
├── pallet-commission-common  (CommissionPlugin, MemberProvider, MultiLevelPlanWriter)
├── pallet-entity-common      (EntityProvider, AdminPermission)
├── frame-support / frame-system / sp-runtime
└── codec / scale-info
```

## License

MIT
