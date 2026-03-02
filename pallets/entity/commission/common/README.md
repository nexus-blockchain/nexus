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
| `POOL_REWARD` | `0x200` | 沉淀池奖励 |

方法：`contains(flag)`, `insert(flag)`, `remove(flag)`, `is_valid()`, 常量 `ALL_VALID`

### 双来源架构

佣金系统采用双来源并行架构，两个池独立计算、互不干扰：

| 池 | 资金来源 | 用途 | 控制方 |
|---|---|---|---|
| 池 A | 平台费 × `ReferrerShareBps` | 招商推荐人奖金（EntityReferral） | 全局常量（runtime 5000=50%） |
| 池 B | 卖家货款 × `max_commission_rate` | 会员返佣（4 个插件） | Entity Owner |

### CommissionType — 返佣类型

`DirectReward`, `MultiLevel`, `TeamPerformance`, `LevelDiff`, `FixedAmount`, `FirstOrder`, `RepeatPurchase`, `SingleLineUpline`, `SingleLineDownline`, `EntityReferral`, `PoolReward`

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

### MemberCommissionStatsData — 会员返佣统计

```rust
pub struct MemberCommissionStatsData<Balance: Default> {
    pub total_earned: Balance,
    pub pending: Balance,
    pub withdrawn: Balance,
    pub repurchased: Balance,
    pub order_count: u32,
}
```

### WithdrawalTierConfig — 分级提现配置

```rust
pub struct WithdrawalTierConfig {
    pub withdrawal_rate: u16,  // 万分比
    pub repurchase_rate: u16,  // 万分比
}
```

校验方法：`is_valid()` — 确保 `withdrawal_rate + repurchase_rate == 10000`

### WithdrawalMode — 提现模式

| 模式 | 说明 |
|------|------|
| `FullWithdrawal` | 全额提现，不强制复购（Governance 底线仍生效） |
| `FixedRate { repurchase_rate }` | 所有会员统一复购比率 |
| `LevelBased` | 按会员等级查表确定复购比率 |
| `MemberChoice { min_repurchase_rate }` | 会员自选复购比率，不低于 `min_repurchase_rate` |

## Trait 定义

### CommissionPlugin — 返佣插件接口（NEX）

每个返佣模式实现此 trait，由 core 调度引擎调用：

```rust
pub trait CommissionPlugin<AccountId, Balance> {
    fn calculate(
        entity_id: u64, buyer: &AccountId,
        order_amount: Balance, remaining: Balance,
        enabled_modes: CommissionModes,
        is_first_order: bool, buyer_order_count: u32,
    ) -> (Vec<CommissionOutput<AccountId, Balance>>, Balance);
}
```

空实现：`()`

### TokenCommissionPlugin — Token 返佣插件接口

与 `CommissionPlugin` 对称，Balance 替换为 TokenBalance：

```rust
pub trait TokenCommissionPlugin<AccountId, TokenBalance> {
    fn calculate_token(...) -> (Vec<CommissionOutput<AccountId, TokenBalance>>, TokenBalance);
}
```

空实现：`()`

### CommissionProvider — 外部服务接口（NEX）

供订单模块等外部调用：

- `process_commission()` — 处理订单返佣
- `cancel_commission()` — 取消订单返佣
- `pending_commission()` — 查询待提取返佣
- `set_commission_modes()` / `set_direct_reward_rate()` / `set_level_diff_config()` 等配置写入
- `shopping_balance()` / `use_shopping_balance()` — 购物余额查询与使用
- `set_min_repurchase_rate()` — 设置全局最低复购比例

空实现：`NullCommissionProvider`

### TokenCommissionProvider — Token 佣金服务接口

供 Transaction 模块调用 Token 佣金计算：

- `process_token_commission()` / `cancel_token_commission()` / `pending_token_commission()`
- `token_platform_fee_rate()` — Entity 级 Token 平台费率

空实现：`NullTokenCommissionProvider`

### MemberProvider — 会员服务接口

供返佣插件查询推荐人、等级等：

- `is_member()`, `get_referrer()`, `get_member_stats()`
- `uses_custom_levels()`, `custom_level_id()`, `get_level_commission_bonus()`
- `auto_register()`, `auto_register_qualified()`, `is_activated()`
- `get_member_spent_usdt()`, `member_count_by_level()`
- 治理写入：`set_custom_levels_enabled()`, `set_upgrade_mode()`, `add_custom_level()`, `update_custom_level()`, `remove_custom_level()`, `custom_level_count()`

空实现：`NullMemberProvider`

### EntityReferrerProvider — 招商推荐人查询接口

供 commission-core 查询 Entity 级招商推荐人：

- `entity_referrer(entity_id)` — 获取 Entity 的招商推荐人账户

空实现：`()`

### PlanWriter Traits — 方案写入接口

| Trait | 实现者 | 方法 |
|-------|--------|------|
| `ReferralPlanWriter` | commission-referral | `set_direct_rate`, `set_multi_level`, `set_fixed_amount`, `set_first_order`, `set_repeat_purchase`, `clear_config` |
| `LevelDiffPlanWriter` | commission-level-diff | `set_level_rates`, `clear_config` |
| `TeamPlanWriter` | commission-team | `set_team_config` (含 `threshold_mode`), `clear_config` |
| `PoolRewardPlanWriter` | commission-pool-reward | `set_pool_reward_config`, `clear_config`, `set_token_pool_enabled` |

### Token 扩展类型

```rust
/// Token 佣金记录（无 shop_id —— Token 佣金不区分 Shop）
pub struct TokenCommissionRecord<AccountId, TokenBalance, BlockNumber> {
    pub entity_id: u64,
    pub order_id: u64,
    pub buyer: AccountId,
    pub beneficiary: AccountId,
    pub amount: TokenBalance,
    pub commission_type: CommissionType,
    pub level: u8,
    pub status: CommissionStatus,
    pub created_at: BlockNumber,
}

/// Token 佣金统计
pub struct MemberTokenCommissionStatsData<TokenBalance: Default> {
    pub total_earned: TokenBalance,
    pub pending: TokenBalance,
    pub withdrawn: TokenBalance,
    pub repurchased: TokenBalance,
    pub order_count: u32,
}
```

### TokenCommissionProvider — Token 佣金服务接口

供 Transaction 模块调用 Token 佣金计算：

- `process_token_commission()` / `cancel_token_commission()` / `pending_token_commission()`
- `token_platform_fee_rate()` — Entity 级 Token 平台费率（bps，0 = 不收费）

空实现：`NullTokenCommissionProvider`

### 余额读写接口

| Trait | 用途 |
|-------|------|
| `PoolBalanceProvider` | NEX 沉淀池余额读写（core → pool-reward） |
| `TokenPoolBalanceProvider` | Token 沉淀池余额读写（core → pool-reward） |
| `TokenTransferProvider` | Entity Token 转账接口（entity_id 级） |

## 依赖

```toml
[dependencies]
codec = { features = ["derive"], workspace = true }
scale-info = { features = ["derive"], workspace = true }
frame-support = { workspace = true }
sp-runtime = { workspace = true }
```

## 版本历史

- **v0.3.0**: 移除 `CommissionPlan` 枚举（过度设计，前端改用 `utility.batch` 组合分步 extrinsics）
- **v0.2.0**: H1 位标志校验 (`ALL_VALID`/`is_valid`), M1 移除死依赖, M2 `WithdrawalTierConfig::is_valid()`, L1 `CommissionOutput` 添加 PartialEq/Eq
- **v0.1.0**: 初始版本
