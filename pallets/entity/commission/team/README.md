# pallet-commission-team

> 团队业绩返佣插件 — 基于团队累计销售额的阶梯奖金（NEX + Token 双资产）

## 概述

`pallet-commission-team` 是返佣插件系统的**团队业绩插件**，实现 `TEAM_PERFORMANCE`（`0x04`）返佣模式。当买家下单时，沿推荐链向上遍历，对每个上级查询其团队统计（`team_size`、`total_spent`），匹配最高达标的阶梯档位，按该档位比例对当前订单金额计算奖金。

同时提供 **Token 多资产版本**（`TokenCommissionPlugin`），阶梯匹配逻辑与 NEX 版完全一致，仅佣金金额计算使用泛型余额类型。

## 核心数据结构

### TeamPerformanceTier — 阶梯档位

```rust
pub struct TeamPerformanceTier<Balance> {
    pub sales_threshold: Balance, // 团队累计销售额门槛
    pub min_team_size: u32,       // 团队最小人数门槛（0 = 不限制）
    pub rate: u16,                // 奖金比例（基点，500 = 5%）
}
```

### TeamPerformanceConfig — 团队业绩配置（per-entity）

```rust
pub struct TeamPerformanceConfig<Balance, MaxTiers: Get<u32>> {
    pub tiers: BoundedVec<TeamPerformanceTier<Balance>, MaxTiers>, // 按 sales_threshold 升序
    pub max_depth: u8,        // 沿推荐链向上最大遍历深度（1-30）
    pub allow_stacking: bool, // 是否允许多层叠加
    pub threshold_mode: SalesThresholdMode, // 门槛数据源模式
}
```

### SalesThresholdMode — 门槛数据源模式

```rust
pub enum SalesThresholdMode {
    Nex = 0,  // 使用 get_member_stats 返回的 total_spent（NEX Balance 转 u128）
    Usdt = 1, // 使用 get_member_spent_usdt 返回的 USDT 累计（精度 10^6）
}
```

**Default:** `max_depth = 5`，`allow_stacking = false`，`threshold_mode = Nex`，`tiers` 为空

## Storage

| 名称 | 类型 | 键 | 说明 |
|------|------|----|------|
| `TeamPerformanceConfigs` | `StorageMap` | `u64` (entity_id) | 团队业绩配置 |

## Extrinsics

| call_index | 名称 | 权限 | 说明 |
|------------|------|------|------|
| 0 | `set_team_performance_config` | Root | 设置团队业绩返佣配置 |

### set_team_performance_config (call_index 0)

```rust
fn set_team_performance_config(
    origin: OriginFor<T>,          // Root only
    entity_id: u64,
    tiers: BoundedVec<TeamPerformanceTier<BalanceOf<T>>, T::MaxTeamTiers>,
    max_depth: u8,
    allow_stacking: bool,
    threshold_mode: SalesThresholdMode,
) -> DispatchResult
```

**校验（`validate_config` 共享方法，extrinsic 和 PlanWriter 统一调用）：**

| 规则 | 条件 | 错误 |
|------|------|------|
| 档位非空 | `!tiers.is_empty()` | `EmptyTiers` |
| 费率上限 | 每个 `rate <= 10000` | `InvalidRate` |
| 深度范围 | `1 <= max_depth <= 30` | `InvalidMaxDepth` |
| 门槛递增 | `sales_threshold` 严格递增 | `TiersNotAscending` |

## 计算逻辑

### 流程

```
buyer 下单
  → 读取 entity_id 的 TeamPerformanceConfig
  → 沿推荐链向上遍历（最多 max_depth 层）
    → 跳过未激活会员（is_activated 检查）
    → 对每个激活上级查询 (team_size, total_spent)
    → total_spent 来源由 threshold_mode 决定（Nex=NEX 累计, Usdt=USDT 累计）
    → match_tier: 匹配最高达标阶梯档位
    → commission = order_amount × rate / 10000
    → actual = min(commission, remaining)
    → remaining -= actual

allow_stacking = false → 命中第一个达标上级即停止
allow_stacking = true  → 所有达标上级均可获奖（叠加发放）
```

### 阶梯匹配算法 (`match_tier`)

档位按 `sales_threshold` 升序排列，从低到高遍历：

1. 若 `total_spent < threshold` → **break**（后续档位门槛更高，不可能满足）
2. 若 `min_team_size == 0` 或 `team_size >= min_team_size` → 记录该档位 rate
3. 否则跳过该档位继续检查下一档（`min_team_size` 不保证单调递增）

最终返回最后一个匹配的 rate。

### 示例

配置 3 档：

| 档位 | sales_threshold | min_team_size | rate |
|------|-----------------|---------------|------|
| 铜牌 | 10,000 | 5 | 200 (2%) |
| 银牌 | 50,000 | 20 | 500 (5%) |
| 金牌 | 200,000 | 50 | 800 (8%) |

- 上级 A：`total_spent=80,000, team_size=30` → 匹配银牌，rate=500
- 上级 B：`total_spent=300,000, team_size=15` → 金牌 team_size 不足，回退银牌 team_size 也不足，但铜牌满足 → rate=200
- 上级 C：`total_spent=5,000` → 未达铜牌门槛，不匹配

## 与其他返佣模式的区别

| | LEVEL_DIFF | TEAM_PERFORMANCE |
|---|---|---|
| **依据** | 个人会员等级 | 团队累计销售额 + 团队人数 |
| **计算** | 上下级费率差额 | 固定阶梯比例 × 订单金额 |
| **数据来源** | `member_level()` / `custom_level_id()` | `get_member_stats()` |
| **未激活处理** | 跳过继续遍历 | 跳过继续遍历 |
| **典型场景** | 代理商等级体系 | 团队销售目标达标奖金 |

## Trait 实现

| Trait | 来源 | 说明 |
|-------|------|------|
| `CommissionPlugin` | `pallet-commission-common` | NEX 返佣计算，由 core 调度引擎调用 |
| `TokenCommissionPlugin` | `pallet-commission-common` | Token 多资产返佣计算（泛型 `TB: AtLeast32BitUnsigned`） |
| `TeamPlanWriter` | `pallet-commission-common` | 由 core 的 `init_commission_plan` 调用，写入/清除配置 |

### TeamPlanWriter 接口

```rust
fn set_team_config(
    entity_id: u64,
    tiers: Vec<(u128, u32, u16)>,  // (threshold, min_team_size, rate)
    max_depth: u8,
    allow_stacking: bool,
    threshold_mode: u8,            // 0=Nex, 1=Usdt
) -> Result<(), DispatchError>

fn clear_config(entity_id: u64) -> Result<(), DispatchError>
```

`set_team_config` 直接写入存储（无 `validate_config` 校验），调用方需确保参数正确。

## Config

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    type Currency: Currency<Self::AccountId>;
    type MemberProvider: MemberProvider<Self::AccountId>;

    /// 最大阶梯档位数
    #[pallet::constant]
    type MaxTeamTiers: Get<u32>;
}
```

## Runtime 配置示例

```rust
impl pallet_commission_team::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MemberProvider = EntityMemberProvider;
    type MaxTeamTiers = ConstU32<10>;
}
```

## Events

| 事件 | 字段 | 说明 |
|------|------|------|
| `TeamPerformanceConfigUpdated` | `entity_id: u64` | 团队业绩配置已创建或更新 |

## Errors

| 错误 | 触发条件 |
|------|----------|
| `InvalidRate` | 任一档位 `rate > 10000` |
| `EmptyTiers` | 档位列表为空 |
| `InvalidMaxDepth` | `max_depth` 为 0 或超过 30 |
| `TiersNotAscending` | `sales_threshold` 未严格递增 |

## 依赖

```toml
[dependencies]
pallet-entity-common = { path = "../../common", default-features = false }
pallet-commission-common = { path = "../common", default-features = false }

[dev-dependencies]
pallet-balances = { workspace = true, features = ["std"] }
sp-core = { workspace = true, features = ["std"] }
sp-io = { workspace = true, features = ["std"] }
```

## 测试覆盖

共 **23 个**单元测试（代码内嵌 `#[cfg(test)] mod tests`）：

| 分类 | 数量 | 覆盖内容 |
|------|------|----------|
| Extrinsic 校验 | 7 | 正常写入、空档位、非法费率、深度 0/31、门槛未递增、非 Root 权限 |
| Plugin 计算 | 6 | 无配置、模式未启用、单档非叠加、多档叠加、团队人数过滤、remaining 封顶 |
| 遍历深度 | 1 | max_depth 截断 |
| PlanWriter | 1 | set_team_config + clear_config |
| 审计回归 | 6 | H1（PlanWriter 校验）×4、H2（非单调 team_size 匹配）、M1（未激活跳过） |
| 自动生成 | 2 | genesis_config、runtime_integrity |

## 已知限制

| 编号 | 级别 | 说明 |
|------|------|------|
| L1 | Low | extrinsic 硬编码 Weight，未接入 WeightInfo benchmark 框架 |
| L2 | Low | PlanWriter `set_team_config` / `clear_config` 不发出事件 |
