# pallet-commission-common

> 返佣系统共享类型与 Trait 定义

## 概述

`pallet-commission-common` 是返佣插件系统的**纯类型定义 crate**，不包含 Storage 或 Extrinsic。为所有返佣子模块提供统一的类型、枚举、Trait 接口和空实现。

## 核心类型

### CommissionModes — 返佣模式位标志

```rust
pub struct CommissionModes(pub u16);
```

| 常量 | 位标志 | 说明 |
|------|--------|------|
| `DIRECT_REWARD` | `0x01` | 直推奖励 |
| `MULTI_LEVEL` | `0x02` | 多级分销 |
| `TEAM_PERFORMANCE` | `0x04` | 团队业绩（阶梯奖金） |
| `LEVEL_DIFF` | `0x08` | 等级差价 |
| `FIXED_AMOUNT` | `0x10` | 固定金额 |
| `FIRST_ORDER` | `0x20` | 首单奖励 |
| `REPEAT_PURCHASE` | `0x40` | 复购奖励 |
| `SINGLE_LINE_UPLINE` | `0x80` | 单线上线收益 |
| `SINGLE_LINE_DOWNLINE` | `0x100` | 单线下线收益 |

方法：`contains(flag)`, `insert(flag)`, `remove(flag)`

### 双来源架构

佣金系统采用双来源并行架构，两个池独立计算、互不干扰：

| 池 | 资金来源 | 用途 | 控制方 |
|---|---|---|---|
| 池 A | 平台费 × `ReferrerShareBps` | 招商推荐人奖金（EntityReferral） | 全局常量（runtime 5000=50%） |
| 池 B | 卖家货款 × `max_commission_rate` | 会员返佣（4 个插件） | Entity Owner |

### CommissionType — 返佣类型

`DirectReward`, `MultiLevel`, `TeamPerformance`, `LevelDiff`, `FixedAmount`, `FirstOrder`, `RepeatPurchase`, `SingleLineUpline`, `SingleLineDownline`, `EntityReferral`

### CommissionStatus — 返佣状态

`Pending` → `Distributed` / `Withdrawn` / `Cancelled`

### CommissionRecord — 返佣记录

```rust
pub struct CommissionRecord<AccountId, Balance, BlockNumber> {
    pub entity_id: u64,
    pub shop_id: u64,
    pub order_id: u64,
    pub buyer: AccountId,
    pub beneficiary: AccountId,
    pub amount: Balance,
    pub commission_type: CommissionType,
    pub level: u8,
    pub status: CommissionStatus,
    pub created_at: BlockNumber,
}
```

### WithdrawalMode — 提现模式

| 模式 | 说明 |
|------|------|
| `FullWithdrawal` | 全额提现，不强制复购（Governance 底线仍生效） |
| `FixedRate` | 所有会员统一复购比率 |
| `LevelBased` | 按会员等级查表确定复购比率 |
| `MemberChoice` | 会员自选复购比率，不低于 `min_repurchase_rate` |

### CommissionPlan — 佣金方案模板

| 方案 | 说明 |
|------|------|
| `None` | 关闭所有返佣 |
| `DirectOnly { rate }` | 仅直推返佣 |
| `MultiLevel { levels, base_rate }` | 多级分销（逐级递减 20%） |
| `LevelDiff { normal, silver, gold, platinum, diamond }` | 等级极差 |
| `Custom` | 自定义（仅开启开关，参数后续配置） |

## Trait 定义

### CommissionPlugin — 返佣插件接口

每个返佣模式实现此 trait，由 core 调度引擎调用：

```rust
pub trait CommissionPlugin<AccountId, Balance> {
    fn calculate(
        entity_id: u64, shop_id: u64, buyer: &AccountId,
        order_amount: Balance, remaining: Balance,
        enabled_modes: CommissionModes,
        is_first_order: bool, buyer_order_count: u32,
    ) -> (Vec<CommissionOutput<AccountId, Balance>>, Balance);
}
```

### CommissionProvider — 外部服务接口

供订单模块等外部调用：

- `process_commission()` — 处理订单返佣
- `cancel_commission()` — 取消订单返佣
- `pending_commission()` — 查询待提取返佣
- `set_commission_modes()` / `set_direct_reward_rate()` / `set_level_diff_config()` 等配置写入
- `use_shopping_balance()` — 使用购物余额
- `set_min_repurchase_rate()` — 设置全局最低复购比例

空实现：`NullCommissionProvider`

### MemberProvider — 会员服务接口

供返佣插件查询推荐人、等级等：

- `is_member()`, `get_referrer()`, `member_level()`, `get_member_stats()`
- `uses_custom_levels()`, `custom_level_id()`, `auto_register()`
- `get_level_commission_bonus()` — 查询自定义等级的 commission_bonus 字段
- 治理写入：`set_custom_levels_enabled()`, `add_custom_level()`, `update_custom_level()`, `remove_custom_level()` 等

空实现：`NullMemberProvider`

### EntityReferrerProvider — 招商推荐人查询接口

供 commission-core 查询 Entity 级招商推荐人：

- `entity_referrer(entity_id)` — 获取 Entity 的招商推荐人账户

空实现：`()`

### PlanWriter Traits — 方案写入接口

| Trait | 实现者 | 方法 |
|-------|--------|------|
| `ReferralPlanWriter` | commission-referral | `set_direct_rate`, `set_multi_level`, `set_fixed_amount`, `set_first_order`, `set_repeat_purchase`, `clear_config` |
| `LevelDiffPlanWriter` | commission-level-diff | `set_global_rates`, `clear_config` |
| `TeamPlanWriter` | commission-team | `set_team_config`, `clear_config` |
| `EntityReferrerProvider` | entity-registry (runtime bridge) | `entity_referrer` |

## 依赖

```toml
[dependencies]
pallet-entity-common = { path = "../../common", default-features = false }
```
