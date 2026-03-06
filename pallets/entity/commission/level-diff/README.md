# pallet-commission-level-diff

> 等级极差返佣插件 — 基于推荐链上下级等级差价的佣金计算

## 概述

`pallet-commission-level-diff` 是 NEXUS 返佣系统的**等级极差插件**。它沿推荐链向上遍历，当上级推荐人等级高于已遍历的最高等级时，按等级差价计算佣金。同时支持 NEX 原生代币和 Token 多资产两条路径。

本插件作为 `pallet-commission-core` 的插件组件运行，通过 `CommissionPlugin` / `TokenCommissionPlugin` trait 接入核心调度管线。

## 等级极差原理

```
推荐链：买家 → A(level0, 3%) → B(level1, 6%) → C(level2, 9%) → D(level2, 9%) → E(level4, 15%)

订单金额 10000 bps：
├── A: 等级率  3%, prev= 0%, diff= 3% → order × 3%  = 佣金
├── B: 等级率  6%, prev= 3%, diff= 3% → order × 3%  = 佣金
├── C: 等级率  9%, prev= 6%, diff= 3% → order × 3%  = 佣金
├── D: 等级率  9%, prev= 9%, diff= 0% → 跳过（无差价）
└── E: 等级率 15%, prev= 9%, diff= 6% → order × 6%  = 佣金
```

**核心规则：**

- 仅当推荐人等级率 **严格大于** 已遍历的最高等级率时才产生差价佣金
- 相同或更低等级的推荐人不获得差价返佣
- 佣金从 `remaining` 池扣除，额度耗尽后提前退出
- 推荐链有环时通过 `BTreeSet` 循环检测自动中断

## 数据结构

```rust
/// 等级极差配置（统一使用自定义等级体系）
pub struct CustomLevelDiffConfig<MaxLevels: Get<u32>> {
    /// 各等级返佣率（bps），索引 = custom_level_id，弱单调递增
    pub level_rates: BoundedVec<u16, MaxLevels>,
    /// 最大推荐链遍历深度（1-20）
    pub max_depth: u8,
}
```

- `level_rates[i]` 对应 `custom_level_id = i` 的返佣率（基点，1 bps = 0.01%）
- 未配置时回退到 `MemberProvider::get_level_commission_bonus(entity_id, level_id)`
- `level_id` 越界时同样回退到 `MemberProvider`

## Config

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<...>;
    /// 原生代币（NEX）操作接口
    type Currency: Currency<Self::AccountId>;
    /// 会员信息查询（推荐人、等级、封禁/激活/冻结状态）
    type MemberProvider: MemberProvider<Self::AccountId>;
    /// 实体查询（存在性、活跃状态、Owner/Admin 权限、治理锁）
    type EntityProvider: EntityProvider<Self::AccountId>;
    /// 最大自定义等级数（runtime 常量，当前 = 10）
    #[pallet::constant]
    type MaxCustomLevels: Get<u32>;
}
```

**Runtime 配置示例（nexus-runtime）：**

```rust
impl pallet_commission_level_diff::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MemberProvider = EntityMemberProvider;
    type EntityProvider = EntityRegistry;
    type MaxCustomLevels = ConstU32<10>;
}
```

## Storage

| 存储项 | 键 | 值 | 说明 |
|--------|------|------|------|
| `CustomLevelDiffConfigs` | `entity_id: u64` | `CustomLevelDiffConfig` | 每个实体的等级极差配置 |

**Storage Version:** `1`

## Extrinsics

| call_index | 方法 | 权限 | 说明 |
|------------|------|------|------|
| 1 | `set_level_diff_config(entity_id, level_rates, max_depth)` | Owner / Admin(COMMISSION_MANAGE) | 设置等级极差配置（覆盖） |
| 2 | `clear_level_diff_config(entity_id)` | Owner / Admin(COMMISSION_MANAGE) | 清除等级极差配置 |
| 3 | `force_set_level_diff_config(entity_id, level_rates, max_depth)` | Root | 强制设置（紧急覆写） |
| 4 | `force_clear_level_diff_config(entity_id)` | Root | 强制清除 |
| 5 | `update_level_diff_config(entity_id, level_rates?, max_depth?)` | Owner / Admin(COMMISSION_MANAGE) | 部分更新（None 保留原值） |

### 参数校验

| 规则 | 错误 |
|------|------|
| `level_rates` 不可为空 | `EmptyLevelRates` |
| 每个 rate ≤ 10000 bps | `InvalidRate` |
| rates 弱单调递增（`rates[i+1] >= rates[i]`） | `RatesNotMonotonic` |
| `max_depth` 范围 1-20 | `InvalidMaxDepth` |

### 权限保护

| Extrinsic | entity_exists | is_entity_active | is_entity_locked | Owner/Admin |
|-----------|:---:|:---:|:---:|:---:|
| `set_level_diff_config` | ✅（via ensure_owner_or_admin） | ✅ | ✅ | ✅ |
| `clear_level_diff_config` | ✅ | ✅ | ✅ | ✅ |
| `update_level_diff_config` | ✅ | ✅ | ✅ | ✅ |
| `force_set_level_diff_config` | ✅ | — | — | Root |
| `force_clear_level_diff_config` | ✅ | — | — | Root |

## 计算逻辑

### NEX 路径 — `process_level_diff`

```
CommissionPlugin::calculate(entity_id, buyer, order_amount, remaining, enabled_modes, ...)
    │
    ├── enabled_modes 不含 LEVEL_DIFF → 直接返回
    │
    └── process_level_diff(entity_id, buyer, order_amount, &mut remaining, &mut outputs)
            │
            ├── 读取 CustomLevelDiffConfig（无则 max_depth 默认 10）
            ├── prev_rate = 0, visited = BTreeSet::new()
            │
            └── while referrer = get_referrer(buyer → 上级):
                    ├── 循环检测: visited 已含 → break
                    ├── level > max_depth → break
                    ├── remaining == 0 → break
                    ├── !is_member → skip（非会员推荐人）
                    ├── is_banned || !is_activated → skip（封禁/未激活）
                    ├── !is_member_active → skip（冻结/暂停）
                    ├── referrer_rate = config.level_rates[level_id] 或回退 commission_bonus
                    ├── referrer_rate <= prev_rate → skip（无差价）
                    ├── diff_rate = referrer_rate - prev_rate
                    ├── commission = order_amount × diff_rate / 10000
                    ├── actual = min(commission, remaining)
                    ├── remaining -= actual
                    ├── outputs.push(CommissionOutput { beneficiary, amount, LevelDiff, level })
                    ├── 发射 LevelDiffCommissionDetail 事件
                    └── prev_rate = referrer_rate
```

### Token 路径 — `process_level_diff_token<TB>`

逻辑与 NEX 版完全对称，差异：

- 金额类型为泛型 `TB: AtLeast32BitUnsigned + Copy + Into<u128>`
- 发射 `LevelDiffTokenCommissionDetail` 事件（`token_amount: u128`）
- 复用同一份 `CustomLevelDiffConfig`

### 推荐人过滤链（NEX 与 Token 一致）

```
is_member? ──no──→ skip
    │yes
is_banned? || !is_activated? ──yes──→ skip
    │no
is_member_active? ──no──→ skip
    │yes
继续计算差价
```

## Trait 实现

### CommissionPlugin（NEX）

```rust
impl CommissionPlugin<AccountId, Balance> for Pallet<T>
```

由 `pallet-commission-core` 在 NEX 调度管线中调用。仅当 `enabled_modes` 包含 `LEVEL_DIFF` 位时执行。

### TokenCommissionPlugin（Token 多资产）

```rust
impl<TB> TokenCommissionPlugin<AccountId, TB> for Pallet<T>
where TB: AtLeast32BitUnsigned + Copy + Default + Debug + Into<u128>
```

由 `pallet-commission-core` 在 Token 调度管线中调用。`Into<u128>` 约束用于事件中的金额序列化。

### LevelDiffPlanWriter

```rust
impl LevelDiffPlanWriter for Pallet<T>
```

供 `pallet-commission-core` 通过治理路径写入/清除配置：

| 方法 | 说明 | 校验 |
|------|------|------|
| `set_level_rates(entity_id, level_rates, max_depth)` | 写入配置 | entity_exists + 空检查 + rate ≤ 10000 + 单调递增 + depth 1-20 + MaxCustomLevels |
| `clear_config(entity_id)` | 清除配置 | 幻影事件守卫（不存在时不发射事件） |

## Events

| 事件 | 字段 | 触发时机 |
|------|------|----------|
| `LevelDiffConfigUpdated` | `entity_id: u64, levels_count: u32` | 配置设置/更新成功 |
| `LevelDiffConfigCleared` | `entity_id: u64` | 配置清除成功 |
| `LevelDiffCommissionDetail` | `entity_id, beneficiary, referrer_rate, prev_rate, diff_rate, amount: Balance, level` | NEX 路径每笔极差佣金计算明细 |
| `LevelDiffTokenCommissionDetail` | `entity_id, beneficiary, referrer_rate, prev_rate, diff_rate, token_amount: u128, level` | Token 路径每笔极差佣金计算明细 |

## Errors

| 错误 | 说明 |
|------|------|
| `InvalidRate` | 返佣率超过 10000 基点 |
| `InvalidMaxDepth` | max_depth 不在 1-20 范围内 |
| `EmptyLevelRates` | level_rates 为空 |
| `EntityNotFound` | entity_id 对应的实体不存在 |
| `NotEntityOwnerOrAdmin` | 调用者非 Entity Owner 且非 Admin(COMMISSION_MANAGE) |
| `EntityLocked` | 实体已被治理锁定 |
| `ConfigNotFound` | 清除/更新时配置不存在 |
| `EntityNotActive` | 实体未激活 |
| `RatesNotMonotonic` | 等级率未弱单调递增 |

## Hooks

### integrity_test（std only）

运行时常量合理性检查：`MaxCustomLevels > 0`。

## 安全机制

| 机制 | 说明 |
|------|------|
| 循环检测 | `BTreeSet<AccountId>` 防止推荐链有环时无限循环 |
| 额度耗尽退出 | `remaining == 0` 时立即 break |
| 封禁检查 | `is_banned` → 跳过 |
| 激活检查 | `is_activated` → 未激活跳过 |
| 冻结检查 | `is_member_active` → 冻结/暂停跳过 |
| 非会员检查 | `is_member` → 非会员跳过 |
| 治理锁 | `is_entity_locked` → signed extrinsic 拒绝 |
| 实体活跃 | `is_entity_active` → signed extrinsic 拒绝 |
| 实体存在 | `entity_exists` → 所有 extrinsic 拒绝（防孤立存储） |
| 率值校验 | rate ≤ 10000, 非空, 弱单调递增 |
| 深度校验 | max_depth 1-20 |
| 幻影事件守卫 | `clear_config` trait 路径配置不存在时不发射事件 |

## 测试覆盖

共 **76** 个单元测试，覆盖：

- 基础计算：差价计算、额度耗尽、相同等级跳过、空推荐链
- 配置管理：设置/清除/更新、参数校验（rates、depth、空、单调性）
- 权限模型：Owner、Admin、Root、非授权拒绝
- 安全守卫：循环检测、封禁/未激活/冻结跳过、非会员跳过、实体锁定/未激活/不存在
- Token 路径：Token 计算、Token 事件、Token remaining 耗尽
- Trait 路径：LevelDiffPlanWriter 事件发射、幻影事件守卫、实体存在性校验
- 事件验证：CommissionDetail 字段、ConfigUpdated levels_count、TokenCommissionDetail

## 依赖

```toml
[dependencies]
codec = { features = ["derive"] }
scale-info = { features = ["derive"] }
frame-support = { workspace = true }
frame-system = { workspace = true }
sp-runtime = { workspace = true }
pallet-commission-common = { path = "../common", default-features = false }
pallet-entity-common = { path = "../../common", default-features = false }

[dev-dependencies]
pallet-balances = { workspace = true, features = ["std"] }
sp-io = { workspace = true, features = ["std"] }
```

## 审计记录

### Round 1-2（v0.2.0）

| ID | 级别 | 描述 |
|----|------|------|
| H1-R1 | High | trait 方法无 rate 校验 → 添加 `ensure!(rate <= 10000)` |
| M1-R1 | Medium | 无条件读取两套配置 → 合并为统一 `CustomLevelDiffConfig` |
| H1-R2 | High | 推荐链无循环检测 → `BTreeSet` visited 集合 |
| H2-R2 | High | 允许空 `level_rates` → `EmptyLevelRates` 校验 |
| M1-R2 | Medium | trait 路径不发事件 → 添加 `LevelDiffConfigUpdated` |

### Round 3（v0.3.0）

| ID | 级别 | 描述 |
|----|------|------|
| M1-R3 | Medium | `clear_config` 不发事件 → 新增 `LevelDiffConfigCleared` 事件 |
| M2-R3 | Medium | trait 路径不检查空 rates → 添加校验 |
| L1-R3 | Low | Cargo features 传播缺失 → 已添加 |

### Round 4（v0.4.0）

| ID | 级别 | 描述 |
|----|------|------|
| M1-R4 | Medium | 死依赖移除 |
| M2-R4 | Medium | Token 路径零测试覆盖 → 新增 4 个 Token 测试 |

### Round 5（v0.5.0）— 权限模型重构

| ID | 级别 | 描述 |
|----|------|------|
| X1 | High | `set_level_diff_config` 改为 signed + Owner/Admin 权限 |
| X2 | High | 新增 `force_set_level_diff_config` (Root) |
| X3 | High | 新增 `clear_level_diff_config` (signed) |
| X4 | High | 新增 `force_clear_level_diff_config` (Root) |
| X5 | High | Config 新增 `EntityProvider` |
| X6 | Medium | 日常 extrinsic 添加 `is_entity_locked` 检查 |
| X8 | High | 添加 `is_banned` + `is_activated` 检查 |
| X10 | Low | trait `clear_config` 幻影事件守卫 |

### Round 6（v0.6.0）— 功能增强

| ID | 描述 |
|----|------|
| F1 | `is_entity_active` 检查 |
| F2 | `update_level_diff_config` 部分更新 extrinsic |
| F3 | 等级率弱单调递增校验 (`RatesNotMonotonic`) |
| F4 | `is_member` 非会员跳过 |
| F5 | `LevelDiffCommissionDetail` 明细事件 |
| F6 | trait 路径 `entity_exists` 校验 |
| F7 | `integrity_test` 运行时常量校验 |
| F8 | 事件包含 `levels_count` |

### Round 7（v0.7.0）— 审计修复

| ID | 级别 | 描述 | 状态 |
|----|------|------|------|
| M1-R7 | Medium | Token 路径不发射佣金明细事件 → 新增 `LevelDiffTokenCommissionDetail` | ✅ 已修复 |
| M2-R7 | Medium | 缺少 `is_member_active` 检查（与 referral 不一致） → NEX/Token 均添加 | ✅ 已修复 |
| M3-R7 | Medium | core pallet trait 路径不校验单调递增 | 📝 记录（core 范围） |
| L1-R7 | Low | force extrinsic 不检查 `entity_exists` → 已添加 | ✅ 已修复 |
| L2-R7 | Low | Token remaining 耗尽无专门测试 → 已添加 | ✅ 已修复 |

### Round 8（v0.8.0）— 审计修复

| ID | 级别 | 描述 | 状态 |
|----|------|------|------|
| M1-R8 | Medium | `CommissionPlugin::calculate` / `TokenCommissionPlugin::calculate_token` 不检查 `is_entity_active`（与 referral/multi-level 不一致） → 已添加 | ✅ 已修复 |
| L1-R8 | Low | 未使用的 `sp-std` 依赖 → 已移除 | ✅ 已修复 |
| L2-R8 | Low | Cargo.toml 缺少 `pallet-commission-common` 的 `runtime-benchmarks` / `try-runtime` feature 传播 → 已添加 | ✅ 已修复 |

### 记录但未修复

| ID | 级别 | 描述 |
|----|------|------|
| H3 | High | core trait 方法不传 `max_depth`，用 `level_rates.len() as u8`（设计决策） |
| M2 | Medium | `process_level_diff_token` 与 NEX 版代码重复（~50 行维护风险） |
| M3 | Medium | Extrinsic 权重硬编码，无 WeightInfo trait |
| M3-R7 | Medium | core pallet `set_level_diff_config` 不校验单调递增（core 范围） |

## 版本历史

| 版本 | 变更 | 测试数 |
|------|------|:------:|
| v0.1.0 | 初始实现 | — |
| v0.2.0 | Round 1-2 审计修复（循环检测、空 rates、事件） | — |
| v0.3.0 | Round 3 审计修复（clear 事件、trait 空检查、Cargo features） | 26 |
| v0.4.0 | Round 4 审计修复（死依赖、Token 测试） | 30 |
| v0.5.0 | Round 5 权限模型重构（Owner/Admin/Root、is_banned、幻影事件守卫） | 46 |
| v0.6.0 | Round 6 功能增强（is_entity_active、update、单调校验、is_member、CommissionDetail、integrity_test） | 68 |
| v0.7.0 | Round 7 审计修复（Token 事件、is_member_active、force entity_exists） | 74 |
| v0.8.0 | Round 8 审计修复（插件路径 is_entity_active、移除 sp-std、Cargo feature 传播） | 76 |
