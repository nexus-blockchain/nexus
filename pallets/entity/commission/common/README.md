# pallet-commission-common

> 返佣系统共享类型与 Trait 定义

## 概述

`pallet-commission-common` 是返佣插件系统的**纯类型定义 crate**，不包含 Storage 或 Extrinsic。为所有返佣子模块提供统一的类型、枚举、Trait 接口和空实现。

**依赖关系：** 本 crate 从 `pallet-entity-common` 重新导出 `MemberProvider` 和 `NullMemberProvider`，消除跨模块重复定义。

## 双来源架构

佣金系统采用双来源并行架构，两个池独立计算、互不干扰：

| 池 | 资金来源 | 用途 | 控制方 |
|---|---|---|---|
| **池 A** | 平台费 × `ReferrerShareBps` | 招商推荐人奖金（`EntityReferral`） | 全局常量（runtime 5000 = 50%） |
| **池 B** | 卖家货款 × `max_commission_rate` | 会员返佣（6 个插件） | Entity Owner |

NEX 和 Entity Token 两条管线完全对称——每个插件同时实现 `CommissionPlugin`（NEX）和 `TokenCommissionPlugin`（Token），由 core 分别调度。

## 核心类型

### CommissionModes — 返佣模式位标志

```rust
pub struct CommissionModes(pub u16);
```

| 常量 | 值 | 说明 |
|------|-----|------|
| `DIRECT_REWARD` | `0x001` | 直推奖励 |
| `MULTI_LEVEL` | `0x002` | 多级分销 |
| `TEAM_PERFORMANCE` | `0x004` | 团队业绩（阶梯奖金） |
| `LEVEL_DIFF` | `0x008` | 等级差价 |
| `FIXED_AMOUNT` | `0x010` | 固定金额 |
| `FIRST_ORDER` | `0x020` | 首单奖励 |
| `REPEAT_PURCHASE` | `0x040` | 复购奖励 |
| `SINGLE_LINE_UPLINE` | `0x080` | 单线上线收益 |
| `SINGLE_LINE_DOWNLINE` | `0x100` | 单线下线收益 |
| `POOL_REWARD` | `0x200` | 沉淀池奖励 |
| `CREATOR_REWARD` | `0x400` | 创建人收益 |

方法：`contains(flag)`, `insert(flag)`, `remove(flag)`, `is_valid()`, 常量 `ALL_VALID`

### CommissionType — 返佣类型

```
DirectReward | MultiLevel | TeamPerformance | LevelDiff | FixedAmount |
FirstOrder | RepeatPurchase | SingleLineUpline | SingleLineDownline |
EntityReferral | PoolReward | CreatorReward
```

### CommissionStatus — 返佣状态

`Pending`（默认） → `Distributed` / `Withdrawn` / `Cancelled`

### CommissionRecord — NEX 返佣记录

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

### TokenCommissionRecord — Token 返佣记录

与 `CommissionRecord` 对称，**无 `shop_id`**（Token 佣金不区分 Shop）：

```rust
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
```

### 统计类型

| 类型 | 用途 | 字段 |
|------|------|------|
| `MemberCommissionStatsData<Balance>` | NEX 佣金统计 | `total_earned`, `pending`, `withdrawn`, `repurchased`, `order_count` |
| `MemberTokenCommissionStatsData<TokenBalance>` | Token 佣金统计 | 同上 |

### CommissionOutput — 插件输出

```rust
pub struct CommissionOutput<AccountId, Balance> {
    pub beneficiary: AccountId,
    pub amount: Balance,
    pub commission_type: CommissionType,
    pub level: u8,
}
```

### WithdrawalTierConfig — 分级提现配置

```rust
pub struct WithdrawalTierConfig {
    pub withdrawal_rate: u16,  // 万分比
    pub repurchase_rate: u16,  // 万分比
}
```

- 默认值：`withdrawal_rate = 10000`, `repurchase_rate = 0`（全额提现）
- 校验：`is_valid()` — 确保 `withdrawal_rate + repurchase_rate == 10000`

### WithdrawalMode — 提现模式

| 模式 | 说明 |
|------|------|
| `FullWithdrawal`（默认） | 全额提现，不强制复购（Governance 底线仍生效） |
| `FixedRate { repurchase_rate }` | 所有会员统一复购比率 |
| `LevelBased` | 按会员等级通过 `default_tier` + `level_overrides` 查表 |
| `MemberChoice { min_repurchase_rate }` | 会员自选复购比率，不低于 `min_repurchase_rate` |

## Trait 接口

### 插件接口

| Trait | 用途 | 空实现 |
|-------|------|--------|
| `CommissionPlugin<AccountId, Balance>` | NEX 返佣计算 | `()` |
| `TokenCommissionPlugin<AccountId, TokenBalance>` | Token 返佣计算 | `()` |

两者签名一致：`calculate` / `calculate_token` 接收订单上下文（`entity_id`, `buyer`, `order_amount`, `remaining`, `enabled_modes`, `is_first_order`, `buyer_order_count`），返回 `(Vec<CommissionOutput>, new_remaining)`。

### 服务接口

#### CommissionProvider — NEX 佣金服务（空实现：`NullCommissionProvider`）

| 方法 | 说明 |
|------|------|
| `process_commission(entity_id, shop_id, order_id, buyer, order_amount, available_pool, platform_fee)` | 处理订单返佣 |
| `cancel_commission(order_id)` | 取消订单返佣 |
| `pending_commission(entity_id, account)` | 查询待提取返佣 |
| `set_commission_modes(entity_id, modes)` | 设置返佣模式 |
| `set_direct_reward_rate(entity_id, rate)` | 设置直推奖励比例 |
| `set_level_diff_config(entity_id, level_rates)` | 设置等级差价配置 |
| `set_fixed_amount(entity_id, amount)` | 设置固定金额奖励 |
| `set_first_order_config(entity_id, amount, rate, use_amount)` | 设置首单奖励 |
| `set_repeat_purchase_config(entity_id, rate, min_orders)` | 设置复购奖励 |
| `set_withdrawal_config_by_governance(entity_id, enabled)` | 治理设置提现开关 |
| `shopping_balance(entity_id, account)` | 查询购物余额 |
| `use_shopping_balance(entity_id, account, amount)` | 使用购物余额 |
| `set_min_repurchase_rate(entity_id, rate)` | 设置全局最低复购比例 |
| `set_creator_reward_rate(entity_id, rate)` | 设置创建人收益比例（bps，从 Pool B 预扣） |

#### TokenCommissionProvider — Token 佣金服务（空实现：`NullTokenCommissionProvider`）

| 方法 | 说明 |
|------|------|
| `process_token_commission(entity_id, shop_id, order_id, buyer, token_order_amount, token_available_pool, token_platform_fee)` | 处理 Token 订单返佣 |
| `cancel_token_commission(order_id)` | 取消 Token 订单返佣 |
| `pending_token_commission(entity_id, account)` | 查询待提取 Token 返佣 |
| `token_platform_fee_rate(entity_id)` | 获取 Entity 级 Token 平台费率（bps） |

### 查询接口

| Trait | 空实现 | 说明 |
|-------|--------|------|
| `EntityReferrerProvider<AccountId>` | `()` | `entity_referrer(entity_id)` — 获取 Entity 的招商推荐人 |
| `MemberProvider<AccountId>` | `NullMemberProvider` | 从 `pallet-entity-common` 重新导出，提供推荐人/等级/统计等查询 |
| `ParticipationGuard<AccountId>` | `()` | `can_participate(entity_id, account)` — KYC/合规检查守卫 |

`ParticipationGuard` 在 `withdraw_commission`、`claim_pool_reward`、`do_consume_shopping_balance` 中调用，确保 target 账户满足 Entity 的参与要求。默认允许所有操作。

### PlanWriter Traits — 插件方案写入接口

| Trait | 实现者 | 空实现 | 方法 |
|-------|--------|--------|------|
| `ReferralPlanWriter<Balance>` | commission-referral | `()` | `set_direct_rate`, `set_fixed_amount`, `set_first_order`, `set_repeat_purchase`, `clear_config` |
| `MultiLevelPlanWriter` | commission-multi-level | `()` | `set_multi_level(entity_id, level_rates, max_total_rate)`, `set_multi_level_full(entity_id, tiers, max_total_rate)`, `clear_multi_level_config` |
| `LevelDiffPlanWriter` | commission-level-diff | `()` | `set_level_rates(entity_id, level_rates, max_depth)`, `clear_config` |
| `TeamPlanWriter<Balance>` | commission-team | `()` | `set_team_config(entity_id, tiers, max_depth, allow_stacking, threshold_mode)`, `clear_config` |
| `SingleLinePlanWriter` | commission-single-line | `()` | `set_single_line_config(entity_id, upline_rate, downline_rate, base_upline_levels, base_downline_levels, level_increment_threshold, max_upline_levels, max_downline_levels)`, `clear_config`, `set_level_based_levels(entity_id, level_id, upline_levels, downline_levels)`, `clear_level_overrides(entity_id, level_id)` |
| `PoolRewardPlanWriter` | commission-pool-reward | `()` | `set_pool_reward_config(entity_id, level_ratios, round_duration)`, `clear_config`, `set_token_pool_enabled(entity_id, enabled)` |

**`set_multi_level_full` 参数说明：** `tiers: Vec<(rate, required_directs, required_team_size, required_spent)>` — 含完整激活条件的多级分销配置。

**`set_team_config` 参数说明：** `tiers: Vec<(sales_threshold_u128, min_team_size, rate_bps)>`, `threshold_mode`: 0=Nex / 1=Usdt。

**`set_pool_reward_config` 参数说明：** `level_ratios: Vec<(level_id, ratio_bps)>` — sum 必须等于 10000；`round_duration`: 轮次持续区块数。

### 余额读写接口

| Trait | 空实现 | 方法 | 用途 |
|-------|--------|------|------|
| `PoolBalanceProvider<Balance>` | `()` | `pool_balance`, `deduct_pool` | NEX 沉淀池余额读写（core → pool-reward） |
| `TokenPoolBalanceProvider<TokenBalance>` | `()` | `token_pool_balance`, `deduct_token_pool` | Token 沉淀池余额读写（core → pool-reward） |
| `TokenTransferProvider<AccountId, TokenBalance>` | `()` | `token_balance_of`, `token_transfer` | Entity Token 转账接口（entity_id 级，供 core 执行 Token 提现） |

## 依赖

```toml
[dependencies]
codec = { features = ["derive"], workspace = true }
scale-info = { features = ["derive"], workspace = true }
frame-support = { workspace = true }
sp-runtime = { workspace = true }
pallet-entity-common = { path = "../../common", default-features = false }
```

## 版本历史

- **v0.5.1**: 深度审计 Round 3 — L1 Cargo.toml 补充 `pallet-entity-common/runtime-benchmarks` 和 `pallet-entity-common/try-runtime` feature 传播
- **v0.5.0**: README 全面重写同步最新代码。新增 `CREATOR_REWARD` 模式位 / `CreatorReward` 类型 / `set_creator_reward_rate`；新增 `SingleLinePlanWriter` trait；新增 `ParticipationGuard` trait；`MultiLevelPlanWriter` 新增 `set_multi_level_full`；`MemberProvider` 改为从 `pallet-entity-common` 重新导出
- **v0.4.0**: M1 README PlanWriter 表修正（`set_multi_level` 归属 `MultiLevelPlanWriter`），L1 移除重复 `TokenCommissionProvider` 章节，L2 Cargo.toml 补充 `runtime-benchmarks`/`try-runtime` feature 传播
- **v0.3.0**: 移除 `CommissionPlan` 枚举（过度设计，前端改用 `utility.batch` 组合分步 extrinsics）
- **v0.2.0**: H1 位标志校验（`ALL_VALID`/`is_valid`），M1 移除死依赖，M2 `WithdrawalTierConfig::is_valid()`，L1 `CommissionOutput` 添加 PartialEq/Eq
- **v0.1.0**: 初始版本
