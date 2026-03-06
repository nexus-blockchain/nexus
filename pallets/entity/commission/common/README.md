# pallet-commission-common

> 返佣系统共享类型与 Trait 定义（纯类型 crate，无 Storage / Extrinsic）

## 定位

`pallet-commission-common` 是返佣插件体系的**接口契约层**，为 6 个返佣插件 + 1 个调度核心提供统一的类型定义和 Trait 接口。所有返佣子模块通过本 crate 通信，不直接依赖彼此。

```
                        ┌──────────────────────┐
                        │ commission-common     │ ◄── 本 crate
                        │ (types + traits)      │
                        └──────────┬───────────┘
                                   │
              ┌────────────────────┼────────────────────┐
              ▼                    ▼                    ▼
       commission-core      6 个插件 pallet      pallet-entity-order
       (调度引擎)           (各实现 Plugin)       (调用 Provider)
```

**依赖关系**：从 `pallet-entity-common` 重新导出 `MemberProvider` / `NullMemberProvider`，消除跨模块重复定义。

## 双来源双管线架构

### 资金来源（双池）

| 池 | 资金来源 | 用途 | 控制方 |
|---|---|---|---|
| **Pool A** | 平台费 × `ReferrerShareBps` | 招商推荐人奖金（`EntityReferral`） | 全局常量（runtime 配置，如 5000 = 50%） |
| **Pool B** | 卖家货款 × `max_commission_rate` | 会员返佣（6 个插件） | Entity Owner 按需配置 |

### 资产管线（双管线）

NEX 原生代币和 Entity Token 两条管线完全对称：

| 维度 | NEX 管线 | Token 管线 |
|------|----------|------------|
| 插件 Trait | `CommissionPlugin` | `TokenCommissionPlugin` |
| 服务 Trait | `CommissionProvider` | `TokenCommissionProvider` |
| 记录类型 | `CommissionRecord`（含 `shop_id`） | `TokenCommissionRecord`（无 `shop_id`） |
| 统计类型 | `MemberCommissionStatsData` | `MemberTokenCommissionStatsData` |
| 沉淀池 | `PoolBalanceProvider` | `TokenPoolBalanceProvider` |
| 插件计算方法 | `calculate()` | `calculate_token()` |

## 类型参考

### CommissionModes — 返佣模式位标志

```rust
pub struct CommissionModes(pub u16);
```

| 常量 | 位值 | 十六进制 | 说明 |
|------|------|----------|------|
| `DIRECT_REWARD` | `0b0000_0001` | `0x001` | 直推奖励 |
| `MULTI_LEVEL` | `0b0000_0010` | `0x002` | 多级分销 |
| `TEAM_PERFORMANCE` | `0b0000_0100` | `0x004` | 团队业绩（阶梯奖金） |
| `LEVEL_DIFF` | `0b0000_1000` | `0x008` | 等级差价 |
| `FIXED_AMOUNT` | `0b0001_0000` | `0x010` | 固定金额 |
| `FIRST_ORDER` | `0b0010_0000` | `0x020` | 首单奖励 |
| `REPEAT_PURCHASE` | `0b0100_0000` | `0x040` | 复购奖励 |
| `SINGLE_LINE_UPLINE` | `0b1000_0000` | `0x080` | 单线上线收益 |
| `SINGLE_LINE_DOWNLINE` | `0b1_0000_0000` | `0x100` | 单线下线收益 |
| `POOL_REWARD` | `0b10_0000_0000` | `0x200` | 沉淀池奖励 |
| `CREATOR_REWARD` | `0b100_0000_0000` | `0x400` | 创建人收益 |

方法：`contains(flag)` / `insert(flag)` / `remove(flag)` / `is_valid()` — 确保无未定义高位。

常量 `ALL_VALID = 0x7FF`（11 位全 1）。

### CommissionType — 返佣类型枚举

```rust
pub enum CommissionType {
    DirectReward,         // 直推奖励
    MultiLevel,           // 多级分销
    TeamPerformance,      // 团队业绩
    LevelDiff,            // 等级差价
    FixedAmount,          // 固定金额
    FirstOrder,           // 首单奖励
    RepeatPurchase,       // 复购奖励
    SingleLineUpline,     // 单线上线
    SingleLineDownline,   // 单线下线
    EntityReferral,       // 招商推荐（Pool A）
    PoolReward,           // 沉淀池奖励
    CreatorReward,        // 创建人收益
}
```

`EntityReferral` 来自 Pool A（平台费），其余来自 Pool B（货款）。

### CommissionStatus — 返佣状态

```rust
pub enum CommissionStatus {
    Pending,       // 默认：待结算
    Distributed,   // [保留] 维持 SCALE 编码索引稳定，生产代码未使用
    Withdrawn,     // 已结算（settle_order_commission 设置）
    Cancelled,     // 已取消
}
```

生命周期：`Pending` → `Withdrawn`（订单完结）或 `Cancelled`（订单取消）。

### 记录类型

**CommissionRecord**（NEX 返佣）：

| 字段 | 类型 | 说明 |
|------|------|------|
| `entity_id` | `u64` | 实体 ID |
| `shop_id` | `u64` | 店铺 ID |
| `order_id` | `u64` | 订单 ID |
| `buyer` | `AccountId` | 买家 |
| `beneficiary` | `AccountId` | 受益人 |
| `amount` | `Balance` | 返佣金额 |
| `commission_type` | `CommissionType` | 返佣类型 |
| `level` | `u8` | 推荐链层级 |
| `status` | `CommissionStatus` | 状态 |
| `created_at` | `BlockNumber` | 创建区块 |

**TokenCommissionRecord**（Token 返佣）：结构与 `CommissionRecord` 相同，但**无 `shop_id`** 字段（Token 佣金不区分店铺）。

### 统计类型

`MemberCommissionStatsData<Balance>` / `MemberTokenCommissionStatsData<TokenBalance>`：

| 字段 | 说明 |
|------|------|
| `total_earned` | 累计获得 |
| `pending` | 待提取 |
| `withdrawn` | 已提现 |
| `repurchased` | 复购消耗 |
| `order_count` | 关联订单数 |

### CommissionOutput — 插件计算输出

```rust
pub struct CommissionOutput<AccountId, Balance> {
    pub beneficiary: AccountId,     // 受益人
    pub amount: Balance,            // 金额
    pub commission_type: CommissionType,  // 类型
    pub level: u8,                  // 层级
}
```

每个插件的 `calculate` / `calculate_token` 返回 `(Vec<CommissionOutput>, new_remaining)`。

### 提现配置

**WithdrawalTierConfig**：

```rust
pub struct WithdrawalTierConfig {
    pub withdrawal_rate: u16,   // 提现比率（万分比）
    pub repurchase_rate: u16,   // 复购比率（万分比）
}
```

- 默认：`withdrawal_rate=10000, repurchase_rate=0`（全额提现）
- 校验：`is_valid()` → `withdrawal_rate + repurchase_rate == 10000`

**WithdrawalMode**：

| 模式 | 说明 |
|------|------|
| `FullWithdrawal`（默认） | 全额提现，不强制复购（Governance 底线仍生效） |
| `FixedRate { repurchase_rate }` | 所有会员统一复购比率 |
| `LevelBased` | 按会员等级查 `default_tier` + `level_overrides` |
| `MemberChoice { min_repurchase_rate }` | 会员自选复购比率，不低于下限 |

## Trait 接口

### 插件接口

两个 trait 签名完全对称，由各返佣插件 pallet 实现：

| Trait | 方法 | 空实现 |
|-------|------|--------|
| `CommissionPlugin<AccountId, Balance>` | `calculate(entity_id, buyer, order_amount, remaining, enabled_modes, is_first_order, buyer_order_count) → (Vec<Output>, remaining)` | `()` |
| `TokenCommissionPlugin<AccountId, TokenBalance>` | `calculate_token(...)` — 签名同上 | `()` |

### 服务接口

#### CommissionProvider — NEX 佣金服务

空实现：`NullCommissionProvider`

| 方法 | 说明 |
|------|------|
| `process_commission(entity_id, shop_id, order_id, buyer, order_amount, available_pool, platform_fee)` | 处理订单返佣 |
| `cancel_commission(order_id)` | 取消订单返佣 |
| `settle_order_commission(order_id)` | 订单完结时结算佣金（Pending → Withdrawn） |
| `pending_commission(entity_id, account)` | 查询待提取返佣 |
| `shopping_balance(entity_id, account)` | 查询购物余额 |
| `use_shopping_balance(entity_id, account, amount)` | 使用购物余额 |
| `set_commission_modes(entity_id, modes)` | 设置返佣模式 |
| `set_direct_reward_rate(entity_id, rate)` | 设置直推奖励比例 |
| `set_level_diff_config(entity_id, level_rates)` | 设置等级差价配置 |
| `set_fixed_amount(entity_id, amount)` | 设置固定金额奖励 |
| `set_first_order_config(entity_id, amount, rate, use_amount)` | 设置首单奖励 |
| `set_repeat_purchase_config(entity_id, rate, min_orders)` | 设置复购奖励 |
| `set_creator_reward_rate(entity_id, rate)` | 设置创建人收益比例（bps） |
| `set_withdrawal_config_by_governance(entity_id, enabled)` | 治理设置提现开关 |
| `set_min_repurchase_rate(entity_id, rate)` | 设置全局最低复购比例 |
| `governance_set_commission_rate(entity_id, rate)` | 治理设置最大返佣比率（默认 `Ok(())`） |
| `governance_toggle_commission(entity_id, enabled)` | 治理返佣总开关（默认 `Ok(())`） |

#### TokenCommissionProvider — Token 佣金服务

空实现：`NullTokenCommissionProvider`

| 方法 | 说明 |
|------|------|
| `process_token_commission(entity_id, shop_id, order_id, buyer, token_order_amount, token_available_pool, token_platform_fee)` | 处理 Token 订单返佣 |
| `cancel_token_commission(order_id)` | 取消 Token 订单返佣 |
| `pending_token_commission(entity_id, account)` | 查询待提取 Token 返佣 |
| `token_platform_fee_rate(entity_id)` | 获取 Entity 级 Token 平台费率（bps，0=不收费） |

### 查询 / 守卫接口

| Trait | 空实现 | 方法 | 说明 |
|-------|--------|------|------|
| `EntityReferrerProvider<AccountId>` | `()` | `entity_referrer(entity_id) → Option<AccountId>` | Entity 级招商推荐人 |
| `ParticipationGuard<AccountId>` | `()`（默认允许） | `can_participate(entity_id, account) → bool` | 提现/领奖前 KYC 合规检查 |

`ParticipationGuard` 在 `withdraw_commission`、`claim_pool_reward`、`do_consume_shopping_balance` 三个场景调用。

### 沉淀池 / 转账接口

| Trait | 空实现 | 方法 | 用途 |
|-------|--------|------|------|
| `PoolBalanceProvider<Balance>` | `()` | `pool_balance` / `deduct_pool` | NEX 沉淀池余额读写 |
| `TokenPoolBalanceProvider<TokenBalance>` | `()` | `token_pool_balance` / `deduct_token_pool` | Token 沉淀池余额读写 |
| `TokenTransferProvider<AccountId, TokenBalance>` | `()` | `token_balance_of` / `token_transfer` | Entity Token 转账（entity_id 级） |

### PlanWriter Traits — 插件方案写入接口

每个返佣插件暴露一个 PlanWriter trait，供 commission-core 或 governance 模块写入配置：

| Trait | 实现者 | 空实现 | 方法 |
|-------|--------|--------|------|
| `ReferralPlanWriter<Balance>` | commission-referral | `()` | `set_direct_rate` / `set_fixed_amount` / `set_first_order` / `set_repeat_purchase` / `clear_config` |
| `MultiLevelPlanWriter` | commission-multi-level | `()` | `set_multi_level` / `set_multi_level_full` / `clear_multi_level_config` |
| `LevelDiffPlanWriter` | commission-level-diff | `()` | `set_level_rates` / `clear_config` |
| `TeamPlanWriter<Balance>` | commission-team | `()` | `set_team_config` / `clear_config` |
| `SingleLinePlanWriter` | commission-single-line | `()` | `set_single_line_config` / `clear_config` / `set_level_based_levels` / `clear_level_overrides` |
| `PoolRewardPlanWriter` | commission-pool-reward | `()` | `set_pool_reward_config` / `clear_config` / `set_token_pool_enabled` |

#### PlanWriter 参数说明

**`set_multi_level_full`**：`tiers: Vec<(rate, required_directs, required_team_size, required_spent)>` — 含完整激活条件的多级分销配置。

**`set_team_config`**：`tiers: Vec<(sales_threshold_u128, min_team_size, rate_bps)>`，`threshold_mode`: 0=Nex / 1=Usdt。

**`set_pool_reward_config`**：`level_ratios: Vec<(level_id, ratio_bps)>` — 各等级分配比例之和必须等于 10000；`round_duration`: 轮次持续区块数。

**`set_single_line_config`**：`upline_rate`/`downline_rate` 各最大 1000 bps；`level_increment_threshold`: 消费达标后增加可分配层数。

### 重新导出

```rust
pub use pallet_entity_common::{MemberProvider, NullMemberProvider};
```

从 `pallet-entity-common` 统一导出会员服务接口，消除原先 commission-common 与 entity-common 重复定义 MemberProvider 的问题。

## 依赖

```toml
[dependencies]
codec = { features = ["derive"], workspace = true }
scale-info = { features = ["derive"], workspace = true }
frame-support = { workspace = true }
sp-runtime = { workspace = true }
pallet-entity-common = { path = "../../common", default-features = false }
```

Feature 传播：`std` / `runtime-benchmarks` / `try-runtime` 均正确传播至 `pallet-entity-common`。

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.6.0 | 2026-03 | README 重新设计：全面同步 674 行源码，重构文档结构（架构图 + 双管线对比 + 分类 Trait 参考表），补充 `settle_order_commission` / `governance_set_commission_rate` / `governance_toggle_commission` 方法文档，明确 CommissionStatus 生命周期和 Distributed 保留语义 |
| v0.5.1 | 2026-03 | 深度审计 Round 3 — L1 Cargo.toml 补充 `pallet-entity-common/runtime-benchmarks` 和 `pallet-entity-common/try-runtime` feature 传播 |
| v0.5.0 | 2026-03 | 新增 `CREATOR_REWARD` 模式位 / `CreatorReward` 类型 / `set_creator_reward_rate`；新增 `SingleLinePlanWriter` / `ParticipationGuard` trait；`MultiLevelPlanWriter` 新增 `set_multi_level_full`；`MemberProvider` 改为从 `pallet-entity-common` 重新导出 |
| v0.4.0 | 2026-03 | PlanWriter 表修正，移除重复 `TokenCommissionProvider` 章节，Cargo.toml feature 传播 |
| v0.3.0 | 2026-03 | 移除 `CommissionPlan` 枚举（过度设计，前端改用 `utility.batch` 组合分步 extrinsics） |
| v0.2.0 | 2026-03 | 位标志校验（`ALL_VALID`/`is_valid`），移除死依赖，`WithdrawalTierConfig::is_valid()`，`CommissionOutput` 添加 PartialEq/Eq |
| v0.1.0 | 2026-03 | 初始版本 |
