# pallet-commission-level-diff

> 等级极差返佣插件 — 基于会员等级差价的返佣计算

## 概述

`pallet-commission-level-diff` 是返佣系统的**等级极差插件**，沿推荐链向上遍历，高等级推荐人获得与下级的等级差价返佣。支持两种等级体系：

- **全局等级体系** — Normal / Silver / Gold / Platinum / Diamond 五级固定体系
- **自定义等级体系** — Entity 自定义等级 + 对应返佣率

## 等级极差原理

```
推荐链：买家 → A(Normal,3%) → B(Silver,6%) → C(Gold,9%) → D(Gold,9%) → E(Diamond,15%)

订单金额 1000 NEX：
├── A: 等级率 3%, prev=0%, 差价=3% → 1000 × 3% = 30 NEX
├── B: 等级率 6%, prev=3%, 差价=3% → 1000 × 3% = 30 NEX
├── C: 等级率 9%, prev=6%, 差价=3% → 1000 × 3% = 30 NEX
├── D: 等级率 9%, prev=9%, 差价=0% → 跳过（无差价）
└── E: 等级率 15%, prev=9%, 差价=6% → 1000 × 6% = 60 NEX
```

核心规则：
- 仅当推荐人等级率 > 已遍历的最高等级率时才产生差价
- 相同等级的推荐人不获得差价返佣
- 额度耗尽后提前退出

## 数据结构

### LevelDiffConfig — 全局等级差价配置

```rust
pub struct LevelDiffConfig {
    pub normal_rate: u16,    // Normal 等级返佣率（基点）
    pub silver_rate: u16,
    pub gold_rate: u16,
    pub platinum_rate: u16,
    pub diamond_rate: u16,
}
```

### CustomLevelDiffConfig — 自定义等级极差配置

```rust
pub struct CustomLevelDiffConfig<MaxLevels> {
    pub level_rates: BoundedVec<u16, MaxLevels>,  // 各等级返佣率（按 level_id 顺序）
    pub max_depth: u8,                             // 最大遍历层级（1-20）
}
```

## Config

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<...>;
    type Currency: Currency<Self::AccountId>;
    type MemberProvider: MemberProvider<Self::AccountId>;
    #[pallet::constant]
    type MaxCustomLevels: Get<u32>;
}
```

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `LevelDiffConfigs` | `Map<u64, LevelDiffConfig>` | 全局等级差价配置（entity_id → config） |
| `CustomLevelDiffConfigs` | `Map<u64, CustomLevelDiffConfig>` | 自定义等级极差配置（entity_id → config） |

## Extrinsics

| call_index | 方法 | 权限 | 说明 |
|------------|------|------|------|
| 0 | `set_level_diff_config` | Root | 设置全局等级差价配置（5 级固定体系） |
| 1 | `set_custom_level_diff_config` | Root | 设置自定义等级极差配置 |

> 所有返佣率参数校验 `<= 10000` 基点。`max_depth` 范围 `1-20`。

## 计算逻辑

```
CommissionPlugin::calculate()
    ↓ (LEVEL_DIFF 位启用时)
process_level_diff(entity_id, shop_id, buyer, order_amount, remaining)
    ├── 判断使用全局等级还是自定义等级（MemberProvider::uses_custom_levels）
    ├── 沿推荐链向上遍历（最大 max_depth 层）
    ├── 查询每个推荐人的等级率
    ├── 计算差价 = referrer_rate - prev_rate
    ├── 差价 > 0 → 计算返佣 = order_amount × diff_rate / 10000
    └── 从 remaining 扣除，额度耗尽或链结束则退出
```

## Trait 实现

- **`CommissionPlugin`** — 由 core 调度引擎调用
- **`LevelDiffPlanWriter`** — 供 core 的 `init_commission_plan` 写入全局等级配置，`clear_config` 清除全局+自定义配置

## Events

| 事件 | 说明 |
|------|------|
| `LevelDiffConfigUpdated` | 全局等级差价配置更新 |
| `CustomLevelDiffConfigUpdated` | 自定义等级极差配置更新 |

## Errors

| 错误 | 说明 |
|------|------|
| `InvalidRate` | 返佣率超过 10000 基点 |
| `InvalidMaxDepth` | max_depth 不在 1-20 范围内 |

## 依赖

```toml
[dependencies]
pallet-entity-common = { path = "../../common" }
pallet-commission-common = { path = "../common" }
```
