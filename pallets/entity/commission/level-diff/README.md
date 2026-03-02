# pallet-commission-level-diff

> 等级极差返佣插件 — 基于会员等级差价的返佣计算

## 概述

`pallet-commission-level-diff` 是返佣系统的**等级极差插件**，沿推荐链向上遍历，高等级推荐人获得与下级的等级差价返佣。使用自定义等级体系：

- **自定义等级体系** — Entity 自定义等级 + 对应返佣率（`CustomLevelDiffConfig`）
- 无独立配置时回退到 `MemberProvider::get_level_commission_bonus()`

## 等级极差原理

```
推荐链：买家 → A(level0,3%) → B(level1,6%) → C(level2,9%) → D(level2,9%) → E(level4,15%)

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
- 推荐链有环时自动中断（循环检测）

## 数据结构

### CustomLevelDiffConfig — 等级极差配置

```rust
pub struct CustomLevelDiffConfig<MaxLevels> {
    pub level_rates: BoundedVec<u16, MaxLevels>,  // 各等级返佣率（按 level_id 顺序，基点）
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
| `CustomLevelDiffConfigs` | `Map<u64, CustomLevelDiffConfig>` | 等级极差配置（entity_id → config） |

## Extrinsics

| call_index | 方法 | 权限 | 说明 |
|------------|------|------|------|
| 1 | `set_level_diff_config` | Root | 设置等级极差配置（自定义等级体系） |

> `level_rates` 不可为空，每个 rate `<= 10000` 基点。`max_depth` 范围 `1-20`。

## 计算逻辑

```
CommissionPlugin::calculate()
    ↓ (LEVEL_DIFF 位启用时)
process_level_diff(entity_id, buyer, order_amount, remaining)
    ├── 读取 CustomLevelDiffConfig（无配置时回退 commission_bonus）
    ├── 沿推荐链向上遍历（最大 max_depth 层，含循环检测）
    ├── 查询每个推荐人的等级率（配置优先，越界回退 MemberProvider）
    ├── 计算差价 = referrer_rate - prev_rate
    ├── 差价 > 0 → 计算返佣 = order_amount × diff_rate / 10000
    └── 从 remaining 扣除，额度耗尽或链结束则退出
```

## Token 多资产支持

提供与 NEX 版对称的 Token 计算函数（泛型 `TB: AtLeast32BitUnsigned`）：

- `process_level_diff_token` — 复用同一份 `CustomLevelDiffConfig` 的 rate 配置
- 阶梯匹配逻辑与 NEX 版完全一致（基于 `MemberProvider` 的 `custom_level_id` / `get_level_commission_bonus`）
- 仅佣金金额计算使用泛型 TB

## Trait 实现

- **`CommissionPlugin`** — NEX 返佣计算，由 core 调度引擎调用
- **`TokenCommissionPlugin`** — Token 多资产返佣计算
- **`LevelDiffPlanWriter`** — 供 core 的 `init_commission_plan` 写入配置，`clear_config` 清除配置

## Events

| 事件 | 说明 |
|------|------|
| `LevelDiffConfigUpdated` | 等级极差配置更新（extrinsic + trait 路径均触发） |

## Errors

| 错误 | 说明 |
|------|------|
| `InvalidRate` | 返佣率超过 10000 基点 |
| `InvalidMaxDepth` | max_depth 不在 1-20 范围内 |
| `EmptyLevelRates` | level_rates 为空 |

## 审计记录

### Round 1 已修复

| ID | 级别 | 描述 |
|----|------|------|
| H1 | High | `LevelDiffPlanWriter::set_global_rates` trait 方法无 rate 校验。修复: 添加 `ensure!(rate <= 10000)` |
| M1 | Medium | `process_level_diff` 无条件读取两套配置。修复: 合并为统一的 `CustomLevelDiffConfig` |

### Round 2 已修复

| ID | 级别 | 描述 |
|----|------|------|
| H1 | High | `process_level_diff` / `process_level_diff_token` 推荐链无循环检测 — 若推荐链有环则 while 循环无限执行，耗尽 block weight。修复: 添加 `BTreeSet<AccountId>` visited 集合，重复访问时立即 break |
| H2 | High | `set_level_diff_config` 允许空 `level_rates` — 写入无意义空配置。修复: 添加 `ensure!(!level_rates.is_empty(), EmptyLevelRates)` |
| M1 | Medium | `LevelDiffPlanWriter::set_level_rates` trait 路径不发出事件 — governance 修改配置无链上通知。修复: 添加 `deposit_event(LevelDiffConfigUpdated)` |

### 记录但未修复

| ID | 级别 | 描述 |
|----|------|------|
| H3 | High | `CommissionProvider::set_level_diff_config` trait 方法不传 `max_depth`，用 `level_rates.len() as u8` 作为 depth，与 extrinsic 行为不一致（设计决策：trait 路径 depth 自动推导合理） |
| M2 | Medium | `process_level_diff_token` 与 NEX 版完全重复（~50行维护风险） |
| M3 | Medium | Extrinsic 权重硬编码 `Weight::from_parts(45_000_000, 4_000)`，无 WeightInfo trait |
| M4 | Medium | `init_commission_plan` LevelDiff 启用了无用的 `DIRECT_REWARD` 位标志（referral config 已 clear） |
| L1 | Low | 无等级率单调递增校验（无害但易误配） |

## 依赖

```toml
[dependencies]
pallet-entity-common = { path = "../../common" }
pallet-commission-common = { path = "../common" }
```
