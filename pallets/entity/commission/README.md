# pallet-entity-commission

> 💰 Entity 返佣管理模块 - 支持多选返佣模式

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Substrate](https://img.shields.io/badge/Substrate-polkadot--sdk-blue)](https://github.com/paritytech/polkadot-sdk)

## 📖 概述

`pallet-entity-commission` 是 Entity 商城系统的返佣管理模块，支持**多选返佣模式**，店铺可同时启用多种返佣方式，返佣按顺序叠加计算。

### 核心功能

- 💰 **多选返佣模式** - 店铺可同时启用多种返佣方式
- 🔗 **直推奖励** - 直接推荐人获得返佣
- 📊 **多级分销** - 支持 N 层推荐人返佣 + 激活条件
- ⭐ **等级差价** - 高等级会员获得与下级的等级差价
- 💵 **固定金额** - 每单固定金额返佣
- 🎁 **首单奖励** - 新用户首单额外奖励
- 🔄 **复购奖励** - 复购用户额外奖励
- 📈 **单线收益** - 基于全局注册顺序的上下线收益
- 🏆 **团队业绩** - 团队累计销售额阶梯奖金
- ⚙️ **灵活配置** - 店主可自定义各模式参数

## 🎯 返佣模式（可多选）

| 模式 | 位标志 | 说明 |
|------|--------|------|
| `DIRECT_REWARD` | 0x01 | 直推奖励 |
| `MULTI_LEVEL` | 0x02 | 多级分销（N层+激活条件） |
| `TEAM_PERFORMANCE` | 0x04 | 团队业绩（阶梯奖金） |
| `LEVEL_DIFF` | 0x08 | 等级差价 |
| `FIXED_AMOUNT` | 0x10 | 固定金额 |
| `FIRST_ORDER` | 0x20 | 首单奖励 |
| `REPEAT_PURCHASE` | 0x40 | 复购奖励 |
| `SINGLE_LINE_UPLINE` | 0x80 | 单线上线收益 |
| `SINGLE_LINE_DOWNLINE` | 0x100 | 单线下线收益 |
| `ENTITY_REFERRAL` | - | 招商推荐人奖金（从平台费扣除，由 core 内置处理） |

## 💡 多选返佣示例

```
店铺配置：
├── ✅ 直推奖励: 5%
├── ✅ 多级分销: 5层配置（带激活条件）
│   ├── L1: 5%，无条件
│   ├── L2: 3%，需1直推
│   ├── L3: 2%，需3直推
│   ├── L4: 1%，需5直推+10团队
│   └── L5: 1%，需10直推+30团队
├── ✅ 等级差价: 启用
└── 最大返佣上限: 15%

买家 David 消费 1000 NEX：
推荐链：Alice(10直推) → Bob(5直推) → Carol(1直推) → Eve(0直推) → David

返佣计算（叠加，受上限约束）：
├── 直推奖励: Eve 获得 1000 × 5% = 50 NEX
├── 多级分销:
│   ├── Eve (L1, 无条件): 1000 × 5% = 50 NEX
│   ├── Carol (L2, 1直推✓): 1000 × 3% = 30 NEX
│   ├── Bob (L3, 3直推✓): 1000 × 2% = 20 NEX
│   ├── Alice (L4, 5直推✓): 1000 × 1% = 10 NEX
│   └── (L5 无人或未激活)
└── 总多级分销: 110 NEX（在15%上限=150 NEX内）

总返佣：160 NEX（受限于可用返佣池）
```

## 📦 安装

### Cargo.toml

```toml
[dependencies]
pallet-entity-commission = { path = "pallets/entity/commission", default-features = false }

[features]
std = [
    "pallet-entity-commission/std",
]
```

## ⚙️ Runtime 配置

```rust
parameter_types! {
    /// 最大返佣记录数（每订单）
    pub const MaxCommissionRecordsPerOrder: u32 = 20;
}

impl pallet_entity_commission::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type ShopProvider = EntityShop;
    type MemberProvider = EntityMember;
    type MaxCommissionRecordsPerOrder = MaxCommissionRecordsPerOrder;
    type MaxSingleLineLength = ConstU32<10000>;
    type MaxMultiLevels = ConstU32<15>;  // 最大 15 层多级分销
}
```

## 📊 数据结构

### CommissionModes - 返佣模式位标志

```rust
pub struct CommissionModes(pub u16);

impl CommissionModes {
    pub const NONE: u16 = 0b0000_0000;
    pub const DIRECT_REWARD: u16 = 0b0000_0001;
    pub const MULTI_LEVEL: u16 = 0b0000_0010;
    pub const TEAM_PERFORMANCE: u16 = 0b0000_0100;
    pub const LEVEL_DIFF: u16 = 0b0000_1000;
    pub const FIXED_AMOUNT: u16 = 0b0001_0000;
    pub const FIRST_ORDER: u16 = 0b0010_0000;
    pub const REPEAT_PURCHASE: u16 = 0b0100_0000;
}
```

### EntityCommissionConfig - 实体返佣配置

```rust
pub struct EntityCommissionConfig<Balance> {
    pub enabled_modes: CommissionModes,   // 启用的模式（位标志）
    pub max_commission_rate: u16,         // 会员返佣上限（卖家货款扣除）
    pub enabled: bool,                    // 是否全局启用
    pub direct_reward: DirectRewardConfig,
    pub multi_level: MultiLevelConfig,
    pub team_performance: TeamPerformanceConfig,
    pub level_diff: LevelDiffConfig,
    pub fixed_amount: FixedAmountConfig<Balance>,
    pub first_order: FirstOrderConfig<Balance>,
    pub repeat_purchase: RepeatPurchaseConfig,
}
```

### 各模式配置

```rust
// 直推奖励
pub struct DirectRewardConfig {
    pub rate: u16,  // 返佣率（基点）
}

// 多级分销层级配置
pub struct MultiLevelTier {
    pub rate: u16,              // 返佣率（基点）
    pub required_directs: u32,  // 激活所需直推人数（0=无条件）
    pub required_team_size: u32,// 激活所需团队人数（0=无条件）
    pub required_spent: u128,   // 激活所需消费金额（0=无条件）
}

// 多级分销配置（支持 N 层 + 激活条件）
pub struct MultiLevelConfig<MaxLevels> {
    pub levels: BoundedVec<MultiLevelTier, MaxLevels>,
    pub max_total_rate: u16,    // 最大返佣比例上限（基点）
}

// 等级差价（全局等级体系）
pub struct LevelDiffConfig {
    pub normal_rate: u16,
    pub silver_rate: u16,
    pub gold_rate: u16,
    pub platinum_rate: u16,
    pub diamond_rate: u16,
}

// 自定义等级极差配置（方案 B）
pub struct CustomLevelDiffConfig<MaxLevels> {
    pub level_rates: BoundedVec<u16, MaxLevels>,  // 各等级返佣率（按 level_id 顺序）
    pub max_depth: u8,                             // 最大遍历层级
}

// 固定金额
pub struct FixedAmountConfig<Balance> {
    pub amount: Balance,
}

// 首单奖励
pub struct FirstOrderConfig<Balance> {
    pub amount: Balance,
    pub rate: u16,
    pub use_amount: bool,
}

// 复购奖励
pub struct RepeatPurchaseConfig {
    pub rate: u16,
    pub min_orders: u32,
}
```

## 🔧 Extrinsics

### 1. set_commission_modes

设置启用的返佣模式（多选）。

```rust
fn set_commission_modes(
    origin: OriginFor<T>,
    shop_id: u64,
    modes: CommissionModes,
) -> DispatchResult
```

**权限：** 仅店主

**示例：** 启用直推 + 三级分销
```rust
let modes = CommissionModes(
    CommissionModes::DIRECT_REWARD | CommissionModes::MULTI_LEVEL
);
```

### 2. set_direct_reward_config

设置直推奖励配置。

```rust
fn set_direct_reward_config(
    origin: OriginFor<T>,
    shop_id: u64,
    rate: u16,  // 基点，500 = 5%
) -> DispatchResult
```

### 3. set_multi_level_config

设置多级分销配置（支持 N 层 + 激活条件）。

```rust
fn set_multi_level_config(
    origin: OriginFor<T>,
    shop_id: u64,
    levels: BoundedVec<MultiLevelTier, T::MaxMultiLevels>,
    max_total_rate: u16,
) -> DispatchResult
```

**示例：** 配置 5 层多级分销
```rust
let levels = vec![
    MultiLevelTier { rate: 500, required_directs: 0, required_team_size: 0, required_spent: 0 },
    MultiLevelTier { rate: 300, required_directs: 1, required_team_size: 0, required_spent: 0 },
    MultiLevelTier { rate: 200, required_directs: 3, required_team_size: 0, required_spent: 0 },
    MultiLevelTier { rate: 100, required_directs: 5, required_team_size: 10, required_spent: 0 },
    MultiLevelTier { rate: 100, required_directs: 10, required_team_size: 30, required_spent: 0 },
];
```

### 4. set_level_diff_config

设置等级差价配置（全局等级体系）。

```rust
fn set_level_diff_config(
    origin: OriginFor<T>,
    shop_id: u64,
    normal_rate: u16,
    silver_rate: u16,
    gold_rate: u16,
    platinum_rate: u16,
    diamond_rate: u16,
) -> DispatchResult
```

### 11. set_custom_level_diff_config

设置自定义等级极差配置（方案 B，独立于等级定义）。

```rust
fn set_custom_level_diff_config(
    origin: OriginFor<T>,
    shop_id: u64,
    level_rates: BoundedVec<u16, T::MaxCustomLevels>,  // 各等级返佣率
    max_depth: u8,                                      // 最大遍历层级
) -> DispatchResult
```

**示例：** 配置 4 个自定义等级的极差返佣率
```rust
// 等级 0: 3%, 等级 1: 6%, 等级 2: 9%, 等级 3: 15%
let level_rates = vec![300, 600, 900, 1500];
EntityCommission::set_custom_level_diff_config(origin, shop_id, level_rates.try_into().unwrap(), 10)?;
```

### 5. set_fixed_amount_config

设置固定金额配置。

```rust
fn set_fixed_amount_config(
    origin: OriginFor<T>,
    shop_id: u64,
    amount: BalanceOf<T>,
) -> DispatchResult
```

### 6. set_first_order_config

设置首单奖励配置。

```rust
fn set_first_order_config(
    origin: OriginFor<T>,
    shop_id: u64,
    amount: BalanceOf<T>,
    rate: u16,
    use_amount: bool,
) -> DispatchResult
```

### 7. set_repeat_purchase_config

设置复购奖励配置。

```rust
fn set_repeat_purchase_config(
    origin: OriginFor<T>,
    shop_id: u64,
    rate: u16,
    min_orders: u32,
) -> DispatchResult
```

### 8. set_commission_rate

设置会员返佣上限（从卖家货款扣除）。

**权限：** Entity Owner

```rust
fn set_commission_rate(
    origin: OriginFor<T>,
    shop_id: u64,
    max_rate: u16,
) -> DispatchResult
```

### 9. enable_commission

启用/禁用返佣。

```rust
fn enable_commission(
    origin: OriginFor<T>,
    shop_id: u64,
    enabled: bool,
) -> DispatchResult
```

### 10. withdraw_commission

提取返佣。

```rust
fn withdraw_commission(
    origin: OriginFor<T>,
    shop_id: u64,
    amount: Option<BalanceOf<T>>,
) -> DispatchResult
```

**权限：** 会员

## 📡 Events

| 事件 | 说明 | 字段 |
|------|------|------|
| `CommissionConfigUpdated` | 返佣配置更新 | `shop_id` |
| `CommissionModesUpdated` | 返佣模式更新 | `shop_id`, `modes` |
| `CommissionDistributed` | 返佣发放 | `shop_id`, `order_id`, `beneficiary`, `amount`, `commission_type`, `level` |
| `CommissionWithdrawn` | 返佣提取 | `shop_id`, `account`, `amount` |
| `CommissionCancelled` | 返佣取消 | `order_id` |

## ❌ Errors

| 错误 | 说明 |
|------|------|
| `ShopNotFound` | 店铺不存在 |
| `NotShopOwner` | 不是店主 |
| `CommissionNotConfigured` | 返佣未配置 |
| `InsufficientCommission` | 返佣余额不足 |
| `InvalidCommissionRate` | 无效的返佣率 |
| `RecordsFull` | 记录数已满 |

## 🔌 CommissionProvider Trait

本模块实现了 `CommissionProvider` trait，供订单模块调用：

```rust
pub trait CommissionProvider<AccountId, Balance> {
    /// 处理订单返佣
    fn process_commission(
        shop_id: u64,
        order_id: u64,
        buyer: &AccountId,
        order_amount: Balance,
        available_pool: Balance,
    ) -> DispatchResult;

    /// 取消订单返佣
    fn cancel_commission(order_id: u64) -> DispatchResult;

    /// 获取待提取返佣
    fn pending_commission(shop_id: u64, account: &AccountId) -> Balance;
}
```

## 🔗 与订单模块集成

在 `pallet-entity-order` 的 `do_complete_order` 中调用：

```rust
// 发放返佣（双来源：平台费→招商奖金，卖家货款→会员返佣）
T::CommissionHandler::on_order_completed(
    order.shop_id,
    order_id,
    &order.buyer,
    order.total_amount,
    order.platform_fee,
)?;
```

在订单取消/退款时调用：

```rust
// 取消返佣
T::CommissionProvider::cancel_commission(order_id)?;
```

## 📈 返佣模式组合推荐

| 店铺类型 | 推荐组合 | 说明 |
|----------|----------|------|
| **社交电商** | 直推 + 三级分销 | 激励分享和裂变 |
| **代理商体系** | 等级差价 + 团队业绩 | 激励代理升级 |
| **拉新活动** | 直推 + 首单奖励 + 固定金额 | 快速拉新 |
| **复购型店铺** | 直推 + 复购奖励 | 提高复购率 |
| **全功能分销** | 全部启用 | 大型分销体系 |
| **被动收益型** | 单线上线 + 单线下线 | 无需推荐也能获益 |

## 🔗 单线收益模式

### 概念说明

单线收益基于**店铺消费顺序**形成一条单链，每个用户都有唯一的上线（在你之前消费的人）和下线（在你之后消费的人）。

```
店铺消费单链（按首次消费顺序）：
User1 → User2 → User3 → User4 → User5 → ...

对于 User3：
├── 上线：User2, User1（在 User3 之前消费）
└── 下线：User4, User5（在 User3 之后消费）
```

### 配置参数

```rust
pub struct SingleLineConfig<Balance> {
    pub upline_rate: u16,              // 上线收益率（基点，10 = 0.1%）
    pub downline_rate: u16,            // 下线收益率（基点，10 = 0.1%）
    pub base_upline_levels: u8,        // 基础上线层数（默认 10）
    pub base_downline_levels: u8,      // 基础下线层数（默认 15）
    pub level_increment_threshold: Balance, // 每增加此消费额，增加 1 层
    pub max_upline_levels: u8,         // 最大上线层数（默认 20）
    pub max_downline_levels: u8,       // 最大下线层数（默认 30）
}
```

### 收益计算示例

```
用户 User5 消费，累计消费 100 USDT，可获取 12 层上线、18 层下线

上线收益（向前遍历）：
├── User4 累计消费 200 USDT → 200 × 0.1% = 0.2 USDT
├── User3 累计消费 150 USDT → 150 × 0.1% = 0.15 USDT
├── User2 累计消费 100 USDT → 100 × 0.1% = 0.1 USDT
└── User1 累计消费 300 USDT → 300 × 0.1% = 0.3 USDT

下线收益（向后遍历）：
├── User6 累计消费 50 USDT → 50 × 0.1% = 0.05 USDT
├── User7 累计消费 80 USDT → 80 × 0.1% = 0.08 USDT
└── ...
```

### 特点

- ✅ **无需推荐** - 只要消费就自动进入单链
- ✅ **被动收益** - 后续有人消费，你就有下线收益
- ✅ **激励早期用户** - 早期消费者有更多下线
- ✅ **消费越多层数越多** - 激励持续消费
- ⚠️ **比例较低** - 建议 0.05%-0.1%，避免资金压力

## 🏆 团队业绩模式

### 概念说明

团队业绩返佣基于推荐链上级的**团队累计销售额**，按阶梯比例发放奖金。与等级差价不同，团队业绩关注的是团队整体表现而非个人等级。

### 配置参数

```rust
pub struct TeamPerformanceTier<Balance> {
    pub sales_threshold: Balance,  // 团队累计销售额门槛
    pub min_team_size: u32,        // 团队最小人数门槛（0=不限制）
    pub rate: u16,                 // 奖金比例（基点，500 = 5%）
}

pub struct TeamPerformanceConfig<Balance, MaxTiers> {
    pub tiers: BoundedVec<TeamPerformanceTier<Balance>, MaxTiers>,
    pub max_depth: u8,        // 沿推荐链向上最大遍历深度（1-30）
    pub allow_stacking: bool, // 是否允许多层叠加
}
```

### 计算示例

```
配置：
├── Tier 1: 团队销售 ≥ 3000, 团队 ≥ 5人, rate = 2%
└── Tier 2: 团队销售 ≥ 10000, 团队 ≥ 20人, rate = 5%

推荐链: Alice(团队50人,销售20000) → Bob(团队10人,销售5000) → Carol → David(buyer)

David 消费 10000 NEX, allow_stacking=false:
└── Bob 达标 Tier1 (5000≥3000, 10≥5): 10000 × 2% = 200 NEX（最近达标上级）

David 消费 10000 NEX, allow_stacking=true:
├── Bob 达标 Tier1: 10000 × 2% = 200 NEX
└── Alice 达标 Tier2: 10000 × 5% = 500 NEX
```

### 与等级差价的区别

| | 等级差价 (LEVEL_DIFF) | 团队业绩 (TEAM_PERFORMANCE) |
|---|---|---|
| 依据 | 个人会员等级 | 团队累计销售额 + 团队人数 |
| 计算 | 上下级费率差额 | 固定阶梯比例 |
| 场景 | 代理商等级体系 | 团队销售目标达标奖金 |

## 🔒 安全机制

1. **返佣上限** - `max_commission_rate` 限制总返佣不超过可用池的比例
2. **优先级顺序** - 按固定顺序计算，先到先得
3. **剩余池管理** - 每种模式从剩余池中扣除，避免超发
4. **延迟发放** - 返佣记录在 `pending`，需手动提取
5. **取消机制** - 订单退款时可取消未提取的返佣

## 📝 注意事项

1. **模式可叠加** - 同一订单可触发多种返佣模式
2. **返佣池有限** - 总返佣受限于 `available_pool`（ShopRevenue = 卖家货款，PlatformFee = 平台费）
3. **需手动提取** - 返佣记录后需调用 `withdraw_commission` 提取
4. **店主配置** - 店主需先配置并启用返佣功能

## 💰 分级提现机制

### 概念说明

分级提现允许店铺根据会员等级设置不同的提现比例，部分返佣自动转入购物余额用于复购。

### 数据结构

```rust
// 单个等级的提现配置
pub struct WithdrawalTierConfig {
    pub withdrawal_rate: u16,   // 提现比例（基点，6000 = 60%）
    pub repurchase_rate: u16,   // 复购比例（基点，4000 = 40%）
}

// 实体提现配置
pub struct EntityWithdrawalConfig<MaxLevels> {
    pub tier_configs: BoundedVec<WithdrawalTierConfig, MaxLevels>,
    pub enabled: bool,
    pub shopping_balance_generates_commission: bool,
}
```

### 配置示例

```rust
// 设置 4 个等级的分级提现配置
let tier_configs = vec![
    WithdrawalTierConfig { withdrawal_rate: 6000, repurchase_rate: 4000 },  // 等级0: 60%提现, 40%复购
    WithdrawalTierConfig { withdrawal_rate: 7000, repurchase_rate: 3000 },  // 等级1: 70%提现, 30%复购
    WithdrawalTierConfig { withdrawal_rate: 8000, repurchase_rate: 2000 },  // 等级2: 80%提现, 20%复购
    WithdrawalTierConfig { withdrawal_rate: 9000, repurchase_rate: 1000 },  // 等级3: 90%提现, 10%复购
];

EntityCommission::set_withdrawal_config(
    origin,
    shop_id,
    tier_configs.try_into().unwrap(),
    true,   // 启用分级提现
    false,  // 购物余额消费不产生返佣
)?;
```

### 提现流程

```
用户待提取返佣: 100 NEX
用户等级: 等级1 (70%提现, 30%复购)

提现结果:
├── 70 NEX → 用户钱包余额（可自由使用）
└── 30 NEX → 用户购物余额（仅限店铺消费）
```

### 购物余额使用

购物余额仅限在该店铺消费，可通过 `use_shopping_balance` 扣除：

```rust
EntityCommission::use_shopping_balance(origin, shop_id, amount)?;
```

### Extrinsics

#### set_withdrawal_config

设置分级提现配置。

```rust
fn set_withdrawal_config(
    origin: OriginFor<T>,
    shop_id: u64,
    tier_configs: BoundedVec<WithdrawalTierConfig, T::MaxCustomLevels>,
    enabled: bool,
    shopping_balance_generates_commission: bool,
) -> DispatchResult
```

#### use_shopping_balance

使用购物余额支付。

```rust
fn use_shopping_balance(
    origin: OriginFor<T>,
    shop_id: u64,
    amount: BalanceOf<T>,
) -> DispatchResult
```
