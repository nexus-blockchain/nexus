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

### SingleLineConfig — 单线收益配置

```rust
pub struct SingleLineConfig<Balance> {
    pub upline_rate: u16,              // 上线收益率（基点，10 = 0.1%）
    pub downline_rate: u16,            // 下线收益率（基点）
    pub base_upline_levels: u8,        // 基础上线层数（默认 10）
    pub base_downline_levels: u8,      // 基础下线层数（默认 15）
    pub level_increment_threshold: Balance, // 每增加此收益额，增加 1 层
    pub max_upline_levels: u8,         // 最大上线层数（默认 20）
    pub max_downline_levels: u8,       // 最大下线层数（默认 30）
}
```

### 层数动态增长

```
实际上线层数 = min(base_upline_levels + extra_levels, max_upline_levels)
实际下线层数 = min(base_downline_levels + extra_levels, max_downline_levels)

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
    #[pallet::constant]
    type MaxSingleLineLength: Get<u32>;
}
```

### SingleLineStatsProvider Trait

```rust
pub trait SingleLineStatsProvider<AccountId, Balance> {
    fn get_member_stats(shop_id: u64, account: &AccountId) -> MemberCommissionStatsData<Balance>;
}
```

由 core pallet 实现，提供会员累计收益数据用于动态层数计算。

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `SingleLineConfigs` | `Map<u64, SingleLineConfig>` | 单线收益配置（entity_id → config） |
| `SingleLines` | `Map<u64, BoundedVec<AccountId>>` | 消费单链（entity_id → 按序账户列表） |
| `SingleLineIndex` | `DoubleMap<u64, AccountId, u32>` | 用户在单链中的位置索引 |

## Extrinsics

| call_index | 方法 | 权限 | 说明 |
|------------|------|------|------|
| 0 | `set_single_line_config` | Root | 设置单线收益配置 |

> `upline_rate` 和 `downline_rate` 上限为 1000 基点（10%），建议设置 5-10 基点（0.05%-0.1%）避免资金压力。

## 计算逻辑

```
CommissionPlugin::calculate()
    ↓ (SINGLE_LINE_UPLINE / SINGLE_LINE_DOWNLINE 位启用时)
    ├── process_upline(): 从 buyer 位置向前遍历单链
    ├── process_downline(): 从 buyer 位置向后遍历单链
    └── 首次消费时自动加入单链（add_to_single_line）
```

### 加入单链

- 首次消费（`is_first_order = true`）时自动调用 `add_to_single_line`
- 已在单链中的用户不会重复加入
- 单链满（`MaxSingleLineLength`）时发出 `SingleLineJoinFailed` 事件

## Trait 实现

- **`CommissionPlugin`** — 由 core 调度引擎调用，配置和单链均按 `entity_id` 查询（跨店共享单链）

## Events

| 事件 | 说明 |
|------|------|
| `SingleLineConfigUpdated` | 单线收益配置更新 |
| `AddedToSingleLine` | 用户加入单链（entity_id, account, index） |
| `SingleLineJoinFailed` | 单链加入失败（链已满） |

## Errors

| 错误 | 说明 |
|------|------|
| `InvalidRate` | 收益率超过 1000 基点 |
| `SingleLineFull` | 消费单链已满 |

## 依赖

```toml
[dependencies]
pallet-entity-common = { path = "../../common" }
pallet-commission-common = { path = "../common" }
```
