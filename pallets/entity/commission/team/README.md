# pallet-commission-team

> 团队业绩返佣插件 — 基于团队累计销售额的阶梯奖金

## 概述

`pallet-commission-team` 是返佣插件系统的**团队业绩插件**，实现 `TEAM_PERFORMANCE`（`0x04`）返佣模式。当买家下单时，沿推荐链向上遍历，对每个上级查询其团队统计（team_size, total_spent），匹配最高达标的阶梯档位，按该档位比例对当前订单金额计算奖金。

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
pub struct TeamPerformanceConfig<Balance, MaxTiers> {
    pub tiers: BoundedVec<TeamPerformanceTier<Balance>, MaxTiers>,
    pub max_depth: u8,        // 沿推荐链向上最大遍历深度（1-30）
    pub allow_stacking: bool, // 是否允许多层叠加
}
```

## Storage

| 名称 | 类型 | 说明 |
|------|------|------|
| `TeamPerformanceConfigs` | `StorageMap<u64, TeamPerformanceConfig>` | entity_id → 团队业绩配置 |

## Extrinsics

### set_team_performance_config (call_index 0)

设置团队业绩返佣配置。

```rust
fn set_team_performance_config(
    origin: OriginFor<T>,          // Root
    entity_id: u64,
    tiers: BoundedVec<TeamPerformanceTier<BalanceOf<T>>, T::MaxTeamTiers>,
    max_depth: u8,                 // 1-30
    allow_stacking: bool,
) -> DispatchResult
```

**权限：** Root

**校验：**
- `tiers` 非空
- 每个 `rate <= 10000`
- `max_depth` 在 1-30 范围内
- `sales_threshold` 严格递增

## 计算逻辑

```
buyer 下单 → 沿推荐链向上遍历（最多 max_depth 层）
  → 对每个上级查询 (team_size, total_spent)
  → 匹配最高达标阶梯档位
  → commission = order_amount × rate / 10000
  → 受 remaining 可用额度约束

allow_stacking = false: 仅最近一个达标上级获奖（命中即停）
allow_stacking = true:  所有达标上级均可获奖（叠加发放）
```

## 与其他模式的区别

| | LEVEL_DIFF | TEAM_PERFORMANCE |
|---|---|---|
| 依据 | 个人会员等级 | 团队累计销售额 + 团队人数 |
| 计算 | 上下级费率差额 | 固定阶梯比例 |
| 数据来源 | `member_level()` / `custom_level_id()` | `get_member_stats()` |
| 场景 | 代理商等级体系 | 团队销售目标达标奖金 |

## Trait 实现

| Trait | 说明 |
|-------|------|
| `CommissionPlugin` | 由 core 调度引擎调用，计算团队业绩返佣 |
| `TeamPlanWriter` | 由 core 的 `init_commission_plan` 调用，写入/清除配置 |

## Config

```rust
pub trait Config: frame_system::Config {
    type RuntimeEvent: ...;
    type Currency: Currency<Self::AccountId>;
    type MemberProvider: MemberProvider<Self::AccountId>;
    type MaxTeamTiers: Get<u32>;  // 最大阶梯档位数
}
```

## Runtime 配置

```rust
impl pallet_commission_team::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MemberProvider = EntityMemberProvider;
    type MaxTeamTiers = ConstU32<10>;
}
```

## Events

| 事件 | 说明 |
|------|------|
| `TeamPerformanceConfigUpdated` | 团队业绩配置已更新 |

## Errors

| 错误 | 说明 |
|------|------|
| `InvalidRate` | 费率超过 10000 基点 |
| `EmptyTiers` | 档位数为 0 |
| `InvalidMaxDepth` | 遍历深度无效（需 1-30） |
| `TiersNotAscending` | 阶梯门槛未严格递增 |

## 依赖

```toml
[dependencies]
pallet-entity-common = { path = "../../common", default-features = false }
pallet-commission-common = { path = "../common", default-features = false }
```
