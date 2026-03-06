# pallet-commission-single-line

> 单线收益插件 — 基于消费注册顺序的上下线收益

## 概述

`pallet-commission-single-line` 是返佣系统的**单线收益插件**，基于 Entity 级消费顺序形成一条单链，每个用户都有唯一的上线（在你之前消费的人）和下线（在你之后消费的人）。

- **上线收益** (SingleLineUpline) — 向前遍历，从上线的消费中获益
- **下线收益** (SingleLineDownline) — 向后遍历，从下线的消费中获益
- **层数动态增长** — 消费越多，可遍历的层数越多

## 单线原理

```
消费单链（按首次消费顺序）：
User1 → User2 → User3 → User4 → User5 → User6 → ...

User4 消费 1000 NEX，配置 upline_rate=10(0.1%), downline_rate=10(0.1%)：

上线收益（向前遍历）：
├── User3 → 1000 × 0.1% = 1 NEX
├── User2 → 1000 × 0.1% = 1 NEX
└── User1 → 1000 × 0.1% = 1 NEX

下线收益（向后遍历）：
├── User5 → 1000 × 0.1% = 1 NEX
└── User6 → 1000 × 0.1% = 1 NEX
```

特点：
- 无需推荐关系，只要消费就自动进入单链
- 早期消费者拥有更多下线，获得更多被动收益
- 层数随累计收益动态增长，激励持续消费

## 数据结构

### SingleLineConfig — 单线收益配置（可自定义）

每个 Entity 可通过 `set_single_line_config` 自定义全部参数，无硬编码默认值约束。

```rust
pub struct SingleLineConfig<Balance> {
    pub upline_rate: u16,              // 上线收益率（基点，10 = 0.1%，上限 1000）
    pub downline_rate: u16,            // 下线收益率（基点，上限 1000）
    pub base_upline_levels: u8,        // 基础上线层数（可自定义，建议 10）
    pub base_downline_levels: u8,      // 基础下线层数（可自定义，建议 15）
    pub level_increment_threshold: Balance, // 每增加此收益额，增加 1 层（可自定义）
    pub max_upline_levels: u8,         // 最大上线层数（可自定义，建议 150）
    pub max_downline_levels: u8,       // 最大下线层数（可自定义，建议 200）
}
```

### LevelBasedLevels — 按会员等级自定义层数

可通过 `set_level_based_levels` 为不同会员等级设定独立的基础层数，替代 `SingleLineConfig` 中的 `base_upline_levels` / `base_downline_levels`。

```rust
pub struct LevelBasedLevels {
    pub upline_levels: u8,   // 该等级的上线层数
    pub downline_levels: u8, // 该等级的下线层数
}
```

优先级：等级覆盖 > config 基础值 > 默认值

### 层数动态增长

```
base = LevelBasedLevels(buyer等级) ?? SingleLineConfig.base_*_levels

实际上线层数 = min(base_upline + extra_levels, max_upline_levels)
实际下线层数 = min(base_downline + extra_levels, max_downline_levels)

extra_levels = total_earned / level_increment_threshold
```

## Config

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<...>;
    type Currency: Currency<Self::AccountId>;
    /// 查询买家累计收益（从 core 的 MemberCommissionStats 读取）
    type StatsProvider: SingleLineStatsProvider<Self::AccountId, BalanceOf<Self>>;
    /// 查询买家会员等级 ID（可选，用于按等级自定义层数）
    type MemberLevelProvider: SingleLineMemberLevelProvider<Self::AccountId>;
    /// 实体查询接口（权限校验）
    type EntityProvider: EntityProvider<Self::AccountId>;
    /// 会员查询接口（is_banned 检查）
    type MemberProvider: MemberProvider<Self::AccountId>;
    #[pallet::constant]
    type MaxSingleLineLength: Get<u32>;
}
```

### SingleLineStatsProvider Trait

```rust
pub trait SingleLineStatsProvider<AccountId, Balance> {
    fn get_member_stats(entity_id: u64, account: &AccountId) -> MemberCommissionStatsData<Balance>;
}
```

由 core pallet 实现，提供会员累计收益数据用于动态层数计算。

### SingleLineMemberLevelProvider Trait

```rust
pub trait SingleLineMemberLevelProvider<AccountId> {
    fn custom_level_id(entity_id: u64, account: &AccountId) -> u8;
}
```

返回买家的有效自定义等级 ID（考虑过期回退），用于查找 `SingleLineCustomLevelOverrides`。`()` 空实现返回 `0`（不区分等级）。

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `SingleLineConfigs` | `Map<u64, SingleLineConfig>` | 单线收益配置（entity_id → config） |
| `SingleLineSegments` | `DoubleMap<u64, u32, BoundedVec<AccountId>>` | 分段消费单链（entity_id, segment_id → 按序账户列表） |
| `SingleLineSegmentCount` | `Map<u64, u32>` | 单链段数（entity_id → 段计数） |
| `SingleLineIndex` | `DoubleMap<u64, AccountId, u32>` | 用户在单链中的全局位置索引 |
| `SingleLineCustomLevelOverrides` | `DoubleMap<u64, u8, LevelBasedLevels>` | 按等级自定义层数（entity_id, level_id → 层数） |
| `SingleLineEnabled` | `Map<u64, bool>` | 单线收益启用状态（默认 true） |

## Extrinsics

| call_index | 方法 | 权限 | 说明 |
|------------|------|------|------|
| 0 | `set_single_line_config` | Owner/Admin | 设置单线收益配置 |
| 1 | `clear_single_line_config` | Owner/Admin | 清除单线收益配置（级联清理 LevelOverrides） |
| 2 | `update_single_line_params` | Owner/Admin | 部分更新配置参数 |
| 3 | `set_level_based_levels` | Owner/Admin | 设置按等级自定义层数 |
| 4 | `remove_level_based_levels` | Owner/Admin | 移除按等级自定义层数 |
| 5 | `force_set_single_line_config` | Root | 强制设置单线收益配置（无权限检查） |
| 6 | `force_clear_single_line_config` | Root | 强制清除单线收益配置（级联清理 LevelOverrides） |
| 7 | `force_reset_single_line` | Root | 强制重置单链数据（清除所有段和索引） |
| 8 | `pause_single_line` | Owner/Admin | 暂停单线收益计算 |
| 9 | `resume_single_line` | Owner/Admin | 恢复单线收益计算 |

> `upline_rate` 和 `downline_rate` 上限为 1000 基点（10%），建议设置 5-10 基点（0.05%-0.1%）避免资金压力。

### 权限模型

- **Owner/Admin**: Entity 所有者或拥有 `COMMISSION_MANAGE` 权限的管理员可调用 set/clear/update/set_level/remove_level
- **Root**: `force_set` 和 `force_clear` 仅限 Root 调用，无 entity 权限检查
- 被封禁会员（`is_banned`）在佣金计算时自动跳过（消耗层数但不发放）

## 计算逻辑

```
CommissionPlugin::calculate()
    ↓ (SINGLE_LINE_UPLINE / SINGLE_LINE_DOWNLINE 位启用时)
    ├── process_upline(): 从 buyer 位置向前遍历单链
    ├── process_downline(): 从 buyer 位置向后遍历单链
    └── 首次消费时自动加入单链（add_to_single_line）
```

### 加入单链

- 未在链中的用户每次消费都自动尝试加入（`!SingleLineIndex::contains_key`）
- 已在单链中的用户不会重复加入（幂等）
- 段满时自动创建新段（`NewSegmentCreated` 事件），无需人工干预

## Token 多资产支持

`process_upline` / `process_downline` 已泛型化为 `<B: AtLeast32BitUnsigned + Copy>`，NEX 和 Token 共用同一实现：

- `do_calculate<B>` 统一分发，`calculate` / `calculate_token` 各委托一行
- `extra_levels` 计算仍基于 NEX `total_earned`（通过 `StatsProvider`），不使用 Token 收益
- 单链维护（`add_to_single_line`）在 NEX 版和 Token 版的 `calculate`/`calculate_token` 中均触发（幂等，不重复加入）

## Trait 实现

- **`CommissionPlugin`** — 由 core 调度引擎调用，配置和单链均按 `entity_id` 查询（跨店共享单链）
- **`TokenCommissionPlugin`** — Token 多资产返佣计算
- **`SingleLinePlanWriter`** — 治理集成接口，支持通过提案设置/清除配置

## Events

| 事件 | 说明 |
|------|------|
| `SingleLineConfigUpdated` | 单线收益配置更新 |
| `SingleLineConfigCleared` | 单线收益配置已清除 |
| `AddedToSingleLine` | 用户加入单链（entity_id, account, index） |
| `SingleLineJoinFailed` | 单链加入失败（段满自动扩展后仍失败，理论上不应触发） |
| `LevelBasedLevelsUpdated` | 按等级自定义层数已更新 |
| `LevelBasedLevelsRemoved` | 按等级自定义层数已移除 |
| `SingleLinePaused` | 单线收益已暂停 |
| `SingleLineResumed` | 单线收益已恢复 |
| `SingleLineReset` | 单链数据已重置（entity_id, removed_count） |
| `NewSegmentCreated` | 新段已创建（entity_id, segment_id） |
| `AllLevelOverridesCleared` | 所有等级层数覆盖已清除 |

## Errors

| 错误 | 说明 |
|------|------|
| `InvalidRate` | 收益率超过 1000 基点 |
| `InvalidLevels` | upline_levels 和 downline_levels 不能同时为 0 |
| `BaseLevelsExceedMax` | base_upline_levels > max_upline_levels 或 base_downline_levels > max_downline_levels |
| `EntityNotFound` | 实体不存在 |
| `NotEntityOwnerOrAdmin` | 调用者非 Entity Owner 或 Admin |
| `ConfigNotFound` | 配置不存在（clear/update 时） |
| `NothingToUpdate` | update_single_line_params 无参数待更新 |
| `EntityLocked` | 实体已锁定，不允许修改 |
| `EntityNotActive` | 实体未激活 |
| `SingleLineIsPaused` | 单线收益已暂停（重复暂停时） |
| `SingleLineNotPaused` | 单线收益未暂停（未暂停时恢复） |

## 审计记录

| ID | 级别 | 描述 |
|----|------|------|
| C2 | Critical | `process_upline`/`process_downline` 佣金计算使用 `beneficiary.total_earned * rate`（累计值，无限增长）。修复: 改为 `order_amount * rate / 10000`，添加 `order_amount` 参数 |
| H4 | High | `calc_extra_levels` 中 `(earned/threshold) as u8` 可溢出。修复: 添加 `.min(255)` |
| L1-R2 | Low | README trait 签名/存储名与代码不一致。修复: 同步 |
| L2-R2 | Low | Cargo.toml 缺 `sp-runtime/runtime-benchmarks` 和 `sp-runtime/try-runtime` feature 传播。修复: 已添加 |
| L3-R2 | Low | `process_downline`/`process_downline_token` 中 `buyer_index + i` 无限溢出检查。修复: 改用 `saturating_add` |
| M1-R2 | Medium | `AddedToSingleLine` 事件已定义但 `add_to_single_line` 从未发射。修复: 成功加入后发射事件 |
| M2-R2 | Medium | `set_single_line_config` 不校验 `base_levels <= max_levels`，允许不合逻辑配置。修复: 添加 `BaseLevelsExceedMax` 校验 |
| M3-R2 | Medium | `SingleLines` 在启用 upline+downline 时被 `process_upline` 和 `process_downline` 各读取一次（需签名变更）。修复: `calculate`/`calculate_token` 预读取一次，传 `&line` 引用给 process 函数（R3 已实现） |
| M1-R3 | Medium | 同 M3-R2 的实施轮次: NEX + Token 共 4 个 process 函数签名均增加 `line: &[T::AccountId]` 参数，消除冗余存储读取。R4 改为分段存储后按需加载段 |
| L5-R3 | Low | Token 版 `_token` 函数与 NEX 版逻辑大量重复（~170 行）。修复: `process_upline`/`process_downline` 泛型化为 `<B: AtLeast32BitUnsigned + Copy>`，删除 `process_upline_token`/`process_downline_token`；新增 `do_calculate<B>` 统一分发，`calculate`/`calculate_token` 各委托一行 |
| M1-R4 | Medium | `AllLevelOverridesCleared` 事件已定义但 `do_clear_all_level_overrides` 从未发射。修复: 清除后发射事件 |
| M2-R4 | Medium | `calc_extra_levels` 有死 `else` 分支 — `threshold.is_zero()` 提前返回使 `threshold_u128 > 0` 检查恒为 true。修复: 移除死分支 |
| M3-R4 | Medium | `pause_single_line`/`resume_single_line` 不检查 `EntityLocked`，与其他 extrinsic 不一致。修复: 添加 EntityLocked 检查 |
| L1-R4 | Low | mock.rs 中 `NON_MEMBERS`/`set_non_member` 为死代码（随 `join_single_line` 删除）。修复: 已移除 |
| L2-R4 | Low | README 过时（存储/Extrinsics/Events/Errors 表均不准确）。修复: 全面同步 |
| L3-R4 | Low | `SingleLineJoinFailed` 事件注释称「需人工干预」但段满自动扩展使其不可达。修复: 更新注释 |

### P0/P1 重构记录

| ID | 级别 | 描述 |
|----|------|------|
| P0-权限 | 重构 | 权限下放: set/set_level/remove_level 从 Root 改为 Owner/Admin signed origin，新增 ensure_owner_or_admin 检查 |
| P0-clear | 新增 | `clear_single_line_config` (Owner/Admin) + `force_clear_single_line_config` (Root) |
| P0-force | 新增 | `force_set_single_line_config` (Root) — 无权限检查的配置设置 |
| P0-ban | 新增 | process_upline/process_downline 跳过 is_banned 受益人（消耗层数但不发佣金） |
| P1-update | 新增 | `update_single_line_params` 部分更新（upline_rate/downline_rate/threshold） |
| P1-writer | 新增 | `SingleLinePlanWriter` trait 实现，支持治理提案设置/清除配置 |

### Round 5（v0.5.0）— 审计修复

| ID | 级别 | 描述 | 状态 |
|----|------|------|------|
| M1-R5 | Medium | `process_upline`/`process_downline` 缺少 `is_member_active` 检查（与 referral/multi-level 不一致） → 已添加 | ✅ 已修复 |
| L1-R5 | Low | 未使用的 `sp-std` 依赖 → 已移除 | ✅ 已修复 |
| L2-R5 | Low | 未使用的 `sp-core` dev-dependency → 已移除 | ✅ 已修复 |

### 记录但未修复

| ID | 级别 | 描述 |
|----|------|------|
| L4-D | Low | extrinsic 硬编码 Weight，无 WeightInfo trait |

## 测试覆盖

共 **140** 个单元测试。

## 依赖

```toml
[dependencies]
codec = { features = ["derive"] }
scale-info = { features = ["derive"] }
frame-support = { workspace = true }
frame-system = { workspace = true }
sp-runtime = { workspace = true }
pallet-entity-common = { path = "../../common" }
pallet-commission-common = { path = "../common" }
