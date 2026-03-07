# pallet-commission-team

> 团队业绩返佣插件 — 基于团队累计销售额的阶梯奖金（NEX + Token 双资产）

## 概述

`pallet-commission-team` 实现 `TEAM_PERFORMANCE`（`0x04`）返佣模式。当买家下单时，沿推荐链向上遍历每个上级，查询其团队统计（团队人数 + 累计销售额），匹配最高达标的阶梯档位，按该档位比例对订单金额计算奖金。

支持 NEX 原生余额与 Token 泛型余额双路径计算，共享同一套阶梯匹配逻辑。

```
                          ┌─────────────────────────────┐
                          │   pallet-commission-core     │
                          │   (调度引擎)                  │
                          └──────────┬──────────────────┘
                                     │ calculate / calculate_token
                          ┌──────────▼──────────────────┐
                          │  pallet-commission-team      │
                          │  ┌───────────────────────┐  │
                          │  │ TeamPerformanceConfig  │  │
                          │  │  tiers[]               │  │
                          │  │  max_depth             │  │
                          │  │  allow_stacking        │  │
                          │  │  threshold_mode        │  │
                          │  └───────────────────────┘  │
                          │                             │
                          │  MemberProvider ──► 推荐链   │
                          │                  ──► 团队统计│
                          │                  ──► 封禁状态│
                          └─────────────────────────────┘
```

## 核心数据结构

### TeamPerformanceTier — 阶梯档位

```rust
pub struct TeamPerformanceTier<Balance> {
    pub sales_threshold: Balance, // 团队累计销售额门槛
    pub min_team_size: u32,       // 团队最小人数门槛（0 = 不限制）
    pub rate: u16,                // 奖金比例（基点，500 = 5%，上限 10000）
}
```

### TeamPerformanceConfig — 团队业绩配置（per-entity）

```rust
pub struct TeamPerformanceConfig<Balance, MaxTiers: Get<u32>> {
    pub tiers: BoundedVec<TeamPerformanceTier<Balance>, MaxTiers>,
    pub max_depth: u8,                    // 推荐链最大遍历深度（1–30）
    pub allow_stacking: bool,             // 是否允许多层叠加发放
    pub threshold_mode: SalesThresholdMode,
}
```

### SalesThresholdMode — 门槛数据源

| 枚举值 | 编码 | 数据来源 | 说明 |
|--------|------|----------|------|
| `Nex` | 0 | `get_member_stats().total_spent` | 使用 NEX 累计消费（u128） |
| `Usdt` | 1 | `get_member_spent_usdt()` | 使用 USDT 累计消费（精度 10^6） |

**默认值：** `max_depth = 5`，`allow_stacking = false`，`threshold_mode = Nex`，`tiers` 为空。

## Storage

| 名称 | 类型 | 键 | 说明 |
|------|------|----|------|
| `TeamPerformanceConfigs` | `StorageMap` | `u64` (entity_id) | 团队业绩阶梯配置 |
| `TeamPerformanceEnabled` | `StorageMap` | `u64` (entity_id) | 启用状态（`ValueQuery`，默认 `false`） |

## Extrinsics

### 配置管理

| call_index | 名称 | 权限 | 说明 |
|------------|------|------|------|
| 0 | `set_team_performance_config` | Owner / Admin | 设置完整配置（覆盖已有配置，自动启用） |
| 1 | `clear_team_performance_config` | Owner / Admin | 清除配置及启用状态 |
| 2 | `update_team_performance_params` | Owner / Admin | 部分更新 max_depth / allow_stacking / threshold_mode |
| 3 | `force_set_team_performance_config` | Root | 强制设置（跳过权限 / 激活 / 锁定检查） |
| 4 | `force_clear_team_performance_config` | Root | 强制清除（不要求配置存在） |

> **Owner / Admin** = Entity Owner 或持有 `COMMISSION_MANAGE` 权限的 Admin。
> 所有非 Root extrinsic 均需通过 `EntityNotActive` + `EntityLocked` 前置检查。

#### set_team_performance_config

```rust
fn set_team_performance_config(
    origin: OriginFor<T>,
    entity_id: u64,
    tiers: BoundedVec<TeamPerformanceTier<BalanceOf<T>>, T::MaxTeamTiers>,
    max_depth: u8,
    allow_stacking: bool,
    threshold_mode: SalesThresholdMode,
) -> DispatchResult
```

全量替换配置。写入后 `TeamPerformanceEnabled` 设为 `true`。

> **注意：** 若之前已暂停，调用此方法会自动取消暂停。如需保留暂停状态仅更新档位，请使用 `add_tier` / `update_tier` / `remove_tier`。

#### update_team_performance_params

部分更新 `max_depth`、`allow_stacking`、`threshold_mode`（不重提 tiers）。至少一个参数非 `None`，否则返回 `NothingToUpdate`。

#### force_set / force_clear

Root 专用。`force_set` 执行参数校验但跳过权限和实体状态检查（可用于预配置或紧急修复）。`force_clear` 仅在配置存在时清除并发出事件。

### 暂停 / 恢复

| call_index | 名称 | 说明 |
|------------|------|------|
| 5 | `pause_team_performance` | 暂停返佣计算（保留配置） |
| 6 | `resume_team_performance` | 恢复返佣计算 |

暂停后 `CommissionPlugin::calculate` 和 `TokenCommissionPlugin::calculate_token` 直接返回空结果。

### 档位 CRUD

| call_index | 名称 | 说明 |
|------------|------|------|
| 7 | `add_tier` | 插入新档位（自动找到正确位置保持升序，重复门槛拒绝） |
| 8 | `update_tier` | 更新指定索引的 rate / min_team_size / sales_threshold |
| 9 | `remove_tier` | 移除指定索引的档位（至少保留 1 个） |

`update_tier` 修改 `sales_threshold` 后会重新校验全部档位的升序约束。

### 参数校验规则

以下规则由 `validate_tiers` 统一执行，extrinsic 和 `TeamPlanWriter` 共享：

| 规则 | 条件 | 错误 |
|------|------|------|
| 档位非空 | `!tiers.is_empty()` | `EmptyTiers` |
| 费率上限 | 每个 `rate <= 10000` | `InvalidRate` |
| 深度范围 | `1 <= max_depth <= 30` | `InvalidMaxDepth` |
| 门槛递增 | `sales_threshold` 严格递增 | `TiersNotAscending` |

## 计算逻辑

### 执行流程

```
buyer 下单
 │
 ├─ CommissionModes 检查 TEAM_PERFORMANCE 位 ─── 未启用 → 跳过
 ├─ TeamPerformanceEnabled 检查 ──────────────── 已暂停 → 跳过
 ├─ 读取 TeamPerformanceConfig ──────────────── 无配置 → 跳过
 │
 └─ 沿推荐链向上遍历（最多 max_depth 层）
      │
      ├─ 循环检测（BTreeSet visited）──────────── 重复 → 终止
      ├─ is_member 检查 ─────────────────────── 非会员 → 跳过
      ├─ is_banned / is_activated /
      │  is_member_active 检查 ──────────────── 不合格 → 跳过
      │
      ├─ 查询团队统计 (team_size, total_spent)
      │  └─ total_spent 来源由 threshold_mode 决定
      │
      ├─ match_tier_with_index → 匹配最高达标档位
      │
      ├─ commission = order_amount × rate / 10000
      ├─ actual = min(commission, remaining)
      ├─ remaining -= actual
      │
      └─ allow_stacking?
           ├─ false → 命中第一个达标上级即停止
           └─ true  → 继续遍历，所有达标上级均可获奖
```

### 阶梯匹配算法

`match_tier_with_index` 对升序排列的档位从低到高遍历：

1. `total_spent < threshold` → **break**（后续档位更高，不可能满足）
2. `min_team_size == 0` 或 `team_size >= min_team_size` → 记录该档位
3. 否则跳过该档位继续（`min_team_size` 不要求单调递增）

返回**最后一个匹配**的 `(tier_index, rate)`。

### 匹配示例

配置 3 档：

| 档位 | sales_threshold | min_team_size | rate |
|------|-----------------|---------------|------|
| 铜牌 | 10,000 | 5 | 200 (2%) |
| 银牌 | 50,000 | 20 | 500 (5%) |
| 金牌 | 200,000 | 50 | 800 (8%) |

| 上级 | total_spent | team_size | 结果 |
|------|-------------|-----------|------|
| A | 80,000 | 30 | 银牌 ✓（rate=500） |
| B | 300,000 | 15 | 金牌 team_size 不足 → 银牌也不足 → 铜牌满足（rate=200） |
| C | 5,000 | 100 | 未达铜牌门槛 → 无匹配 |

### NEX 路径 vs Token 路径

两条路径共享同一个 `match_tier_with_index` 算法和相同的会员过滤逻辑，仅佣金金额计算使用不同的余额类型：

| | NEX 路径 | Token 路径 |
|---|---|---|
| Trait | `CommissionPlugin` | `TokenCommissionPlugin` |
| 金额类型 | `BalanceOf<T>` | 泛型 `TB: AtLeast32BitUnsigned` |
| 事件 | `TeamCommissionAwarded`（含 amount） | `TokenTeamTierMatched`（不含 amount） |
| 金额事件 | 自身 event 包含 | 由 core 的 `TokenCommissionDistributed` 记录 |

## Trait 实现

| Trait | 来源 | 说明 |
|-------|------|------|
| `CommissionPlugin<AccountId, Balance>` | pallet-commission-common | NEX 返佣计算，由 core 调度引擎调用 |
| `TokenCommissionPlugin<AccountId, TB>` | pallet-commission-common | Token 多资产返佣计算 |
| `TeamPlanWriter<Balance>` | pallet-commission-common | 方案写入接口，供 core 批量配置 |

### TeamPlanWriter 接口

```rust
fn set_team_config(
    entity_id: u64,
    tiers: Vec<(u128, u32, u16)>,  // (threshold, min_team_size, rate)
    max_depth: u8,
    allow_stacking: bool,
    threshold_mode: u8,            // 0=Nex, 1=Usdt（其他值拒绝）
) -> Result<(), DispatchError>

fn clear_config(entity_id: u64) -> Result<(), DispatchError>
```

`set_team_config` 执行与 extrinsic 一致的参数校验，写入后发出 `TeamPerformanceConfigUpdated` 事件。`clear_config` 仅在配置存在时发出 `TeamPerformanceConfigCleared` 事件。

### 查询函数

```rust
/// 查询会员当前匹配档位 → (tier_index, rate, next_threshold, next_team_size)
fn get_matched_tier_for_account(entity_id: u64, account: &AccountId)
    -> Option<(u8, u16, Option<Balance>, Option<u32>)>

/// 查询团队业绩状态 → (config_exists, is_enabled)
fn get_team_performance_status(entity_id: u64) -> (bool, bool)
```

## Config

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<...>;
    type Currency: Currency<Self::AccountId>;

    /// 会员查询（推荐链 / 团队统计 / 封禁状态）
    type MemberProvider: MemberProvider<Self::AccountId>;

    /// 实体查询（权限校验 / Owner-Admin 判断 / 激活-锁定状态）
    type EntityProvider: EntityProvider<Self::AccountId>;

    /// 最大阶梯档位数（runtime 常量，须在 1–255 范围内）
    #[pallet::constant]
    type MaxTeamTiers: Get<u32>;
}
```

### Runtime 配置示例

```rust
impl pallet_commission_team::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MemberProvider = EntityMemberProvider;
    type EntityProvider = EntityRegistry;
    type MaxTeamTiers = ConstU32<10>;
}
```

### 完整性校验

`integrity_test` 在编译期断言：
- `MaxTeamTiers > 0`
- `MaxTeamTiers <= 255`（tier 索引使用 `u8` 存储）

## Events

| 事件 | 字段 | 触发场景 |
|------|------|----------|
| `TeamPerformanceConfigUpdated` | `entity_id, tier_count, max_depth, allow_stacking, threshold_mode` | set_config / update_params / force_set / PlanWriter |
| `TeamPerformanceConfigCleared` | `entity_id` | clear_config / force_clear / PlanWriter（仅配置存在时） |
| `TeamPerformancePaused` | `entity_id` | pause |
| `TeamPerformanceResumed` | `entity_id` | resume |
| `TeamTierAdded` | `entity_id, tier_index` | add_tier |
| `TeamTierUpdated` | `entity_id, tier_index` | update_tier |
| `TeamTierRemoved` | `entity_id, tier_index` | remove_tier |
| `TeamCommissionAwarded` | `entity_id, beneficiary, tier_index, rate, amount, depth` | NEX 路径佣金发放 |
| `TokenTeamTierMatched` | `entity_id, beneficiary, tier_index, rate, depth` | Token 路径阶梯匹配 |

## Errors

| 错误 | 触发条件 |
|------|----------|
| `InvalidRate` | 任一档位 `rate > 10000` |
| `EmptyTiers` | 档位列表为空，或 `remove_tier` 试图删除最后一个档位 |
| `InvalidMaxDepth` | `max_depth` 为 0 或超过 30 |
| `TiersNotAscending` | `sales_threshold` 未严格递增，或 `add_tier` 门槛重复 |
| `EntityNotFound` | entity_id 对应的实体不存在 |
| `NotEntityOwnerOrAdmin` | 调用者非 Owner 且无 `COMMISSION_MANAGE` 权限 |
| `ConfigNotFound` | 清除 / 更新 / 暂停 / 恢复 / 档位操作时配置不存在 |
| `NothingToUpdate` | `update_params` 或 `update_tier` 所有参数为 None |
| `EntityLocked` | 实体已被全局锁定 |
| `EntityNotActive` | 实体未激活（暂停/封禁） |
| `TeamPerformanceIsPaused` | 重复暂停 |
| `TeamPerformanceNotPaused` | 未暂停时恢复 |
| `TierIndexOutOfBounds` | 档位索引越界 |
| `TierLimitReached` | 档位数达 `MaxTeamTiers` 上限 |

## 安全模型

### 权限层级

```
Root ──── force_set / force_clear（跳过所有业务检查，仅校验参数）
  │
Owner / Admin(COMMISSION_MANAGE)
  │
  ├─ EntityNotActive 检查
  ├─ EntityLocked 检查
  └─ 业务校验（validate_tiers / ConfigNotFound / ...）
```

### 推荐链遍历安全

| 防护 | 说明 |
|------|------|
| 循环检测 | `BTreeSet<AccountId>` 记录已访问节点，重复即终止 |
| 深度限制 | `max_depth`（1–30）硬性截断 |
| 非会员跳过 | `is_member` 检查，防止已移除会员残留推荐链 |
| 封禁跳过 | `is_banned` 检查 |
| 未激活跳过 | `is_activated` 检查 |
| 冻结跳过 | `is_member_active` 检查 |
| 余额封顶 | `actual = min(commission, remaining)` |
| 饱和算术 | `saturating_mul` / `saturating_sub` 防溢出 |

被跳过的会员仍消耗 depth 槽位（与 multi-level / referral 设计一致，防止深度滥用）。

## 与其他返佣模式对比

| | LEVEL_DIFF | TEAM_PERFORMANCE |
|---|---|---|
| **依据** | 个人会员等级 | 团队累计销售额 + 团队人数 |
| **计算** | 上下级费率差额 | 固定阶梯比例 × 订单金额 |
| **数据来源** | `member_level()` / `custom_level_id()` | `get_member_stats()` / `get_member_spent_usdt()` |
| **封禁处理** | 跳过继续遍历 | 跳过继续遍历 |
| **典型场景** | 代理商等级体系 | 团队销售目标达标奖金 |

## 依赖

```toml
[dependencies]
pallet-entity-common = { path = "../../common", default-features = false }
pallet-commission-common = { path = "../common", default-features = false }

[dev-dependencies]
pallet-balances = { workspace = true, features = ["std"] }
sp-io = { workspace = true, features = ["std"] }
```

## 已知限制

| 编号 | 级别 | 说明 |
|------|------|------|
| L1 | ~~Low~~ | ~~extrinsic 硬编码 Weight，未接入 WeightInfo benchmark 框架~~ **已修复：已接入 WeightInfo trait + benchmarking 框架** |
| L2 | Low | `set_team_performance_config` 覆盖配置时无条件重新启用（若需保留暂停状态仅更新档位，应使用 `add_tier` / `update_tier` / `remove_tier`） |
| L3 | Low | `force_set` (Root) 不检查实体是否存在，可为不存在的 entity_id 创建配置 |

## 测试覆盖

共 **87 个**单元测试（代码内嵌 `#[cfg(test)] mod tests`）：

| 分类 | 数量 | 覆盖内容 |
|------|------|----------|
| Extrinsic 权限与校验 | 10 | Owner/Admin 设置、非 Owner 拒绝、无权限 Admin 拒绝、Entity 不存在、空档位、非法费率、深度 0/31、门槛未递增、相等门槛 |
| clear_config | 2 | Owner 清除、配置不存在拒绝 |
| force_set / force_clear | 4 | Root 设置/清除、非 Root 拒绝 |
| update_params | 4 | 部分更新、全 None 拒绝、配置不存在、非法深度 |
| Plugin 计算（NEX） | 6 | 无配置、模式位未启用、单档非叠加、多档叠加、团队人数过滤、remaining 封顶 |
| 遍历深度 | 1 | max_depth 截断 |
| PlanWriter | 1 | set_team_config + clear_config |
| 非单调 team_size | 1 | 跳过 team_size 不足的低档位，匹配更高档位 |
| PlanWriter 校验 | 4 | 空档位 / 非法费率 / 非法深度 / 非升序门槛 拒绝 |
| 循环检测 | 2 | 环形推荐链、自引用推荐 |
| PlanWriter 事件 | 1 | set + clear 事件发射 |
| 封禁会员跳过 | 2 | 非叠加 / 叠加模式 |
| 幻影事件防护 | 3 | force_clear 无配置不发事件、有配置发事件、PlanWriter clear 无配置不发事件 |
| threshold_mode 校验 | 1 | PlanWriter 拒绝无效 mode (2/255) |
| Extrinsic 事件验证 | 4 | set / clear / update / force_set 事件 |
| EntityLocked | 3 | set / clear / update 锁定拒绝 |
| EntityNotActive | 5 | set / clear / update / add_tier / pause 未激活拒绝 |
| 暂停 / 恢复 | 3 | pause+resume+幂等检查、暂停跳过计算、clear 移除 enabled |
| 档位 CRUD | 7 | add 中间/末尾/重复拒绝、update 费率/越界/升序违反、remove 正常/最后一个拒绝 |
| 查询函数 | 3 | 当前匹配档位、最高档 next=None、状态查询 |
| 事件信息增强 | 2 | 配置事件字段、佣金发放事件 |
| USDT 模式 | 2 | USDT 模式匹配、NEX spent 被忽略 |
| 非会员跳过 | 3 | NEX / 叠加 / Token 路径 |
| Token 路径一致性 | 1 | match_tier_with_index 与 NEX 路径结果一致 |
| 冻结会员跳过 | 3 | NEX / 叠加 / Token 路径 |
| Token 事件 | 2 | TokenTeamTierMatched 单次 / 叠加 |
| 边界行为回归 | 3 | set_config 覆盖暂停状态、force_set 非存在实体、integrity_test |
| 自动生成 | 3 | genesis_config、runtime_integrity 等 |

## 审计历史

| 轮次 | 修复内容 | 测试数 |
|------|----------|--------|
| R1 | match_tier 非单调 team_size 修复、PlanWriter 参数校验 | 24 |
| R2（深度审计） | 循环检测、未激活跳过、PlanWriter 事件 | 29 |
| v0.2.0 | 权限下放 Owner/Admin、clear/force/banned、部分更新 | 42 |
| R3（深度审计） | 幻影事件修复、Cargo features、PlanWriter threshold_mode 校验 | 51 |
| R4（深度审计） | is_member 检查、冗余 storage read 消除、Token 路径 match_tier 一致性 | 79 |
| R5（深度审计） | is_member_active 检查（NEX+Token）、死依赖清理 | 82 |
| R6（深度审计） | Token 路径 `TokenTeamTierMatched` 事件、integrity_test `MaxTeamTiers<=255` | 87 |
