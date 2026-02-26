# pallet-commission-referral

> 推荐链返佣插件 — 直推、多级分销、固定金额、首单奖励、复购奖励

## 概述

`pallet-commission-referral` 是返佣系统的**推荐链插件**，基于会员推荐关系链计算返佣，包含 5 种返佣模式：

- **直推奖励** (DirectReward) — 直接推荐人获得比例返佣
- **多级分销** (MultiLevel) — N 层推荐链 + 激活条件
- **固定金额** (FixedAmount) — 每单固定金额返佣
- **首单奖励** (FirstOrder) — 新用户首单额外奖励
- **复购奖励** (RepeatPurchase) — 达到最低订单数后的复购返佣

## 数据结构

### ReferralConfig — 推荐链返佣总配置（per-entity）

```rust
pub struct ReferralConfig<Balance, MaxLevels> {
    pub direct_reward: DirectRewardConfig,
    pub multi_level: MultiLevelConfig<MaxLevels>,
    pub fixed_amount: FixedAmountConfig<Balance>,
    pub first_order: FirstOrderConfig<Balance>,
    pub repeat_purchase: RepeatPurchaseConfig,
}
```

### 各模式配置

```rust
/// 直推奖励 — 推荐人获得 rate 基点的返佣
pub struct DirectRewardConfig {
    pub rate: u16,  // 基点，500 = 5%
}

/// 多级分销层级 — 每层独立的返佣率和激活条件
pub struct MultiLevelTier {
    pub rate: u16,              // 返佣率（基点）
    pub required_directs: u32,  // 激活所需直推人数（0=无条件）
    pub required_team_size: u32,// 激活所需团队人数（0=无条件）
    pub required_spent: u128,   // 激活所需消费金额（0=无条件）
}

/// 多级分销配置
pub struct MultiLevelConfig<MaxLevels> {
    pub levels: BoundedVec<MultiLevelTier, MaxLevels>,
    pub max_total_rate: u16,    // 多级分销总返佣上限（基点）
}

/// 固定金额 — 每单固定金额
pub struct FixedAmountConfig<Balance> {
    pub amount: Balance,
}

/// 首单奖励 — 固定金额或比例
pub struct FirstOrderConfig<Balance> {
    pub amount: Balance,
    pub rate: u16,
    pub use_amount: bool,  // true=使用固定金额, false=使用比例
}

/// 复购奖励 — 达到最低订单数后按比例返佣
pub struct RepeatPurchaseConfig {
    pub rate: u16,
    pub min_orders: u32,
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
    type MaxMultiLevels: Get<u32>;
}
```

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `ReferralConfigs` | `Map<u64, ReferralConfig>` | 推荐链返佣配置（entity_id → config） |

## Extrinsics

| call_index | 方法 | 权限 | 说明 |
|------------|------|------|------|
| 0 | `set_direct_reward_config` | Root | 设置直推奖励率 |
| 1 | `set_multi_level_config` | Root | 设置多级分销配置 |
| 2 | `set_fixed_amount_config` | Root | 设置固定金额 |
| 3 | `set_first_order_config` | Root | 设置首单奖励 |
| 4 | `set_repeat_purchase_config` | Root | 设置复购奖励 |

> Extrinsics 使用 `ensure_root` 权限。正常使用通过 core pallet 的 `CommissionProvider` trait 或 `init_commission_plan` 进行配置。

## 计算逻辑

### 插件调度顺序

```
CommissionPlugin::calculate()
├── 1. 直推奖励（DIRECT_REWARD 位启用时）
├── 2. 多级分销（MULTI_LEVEL 位启用时）
├── 3. 固定金额（FIXED_AMOUNT 位启用时）
├── 4. 首单奖励（FIRST_ORDER 位启用且 is_first_order 时）
└── 5. 复购奖励（REPEAT_PURCHASE 位启用时）
```

### 多级分销激活条件

每层独立检查推荐人的激活条件：

```
Layer 1: rate=5%, 无条件
Layer 2: rate=3%, 需 1 直推
Layer 3: rate=2%, 需 3 直推
Layer 4: rate=1%, 需 5 直推 + 10 团队
Layer 5: rate=1%, 需 10 直推 + 30 团队
```

- 推荐人未激活 → 跳过该层，继续遍历上级
- `max_total_rate` 限制多级分销总返佣
- 每种模式从 `remaining` 额度中扣除，避免超发

## Trait 实现

- **`CommissionPlugin`** — 由 core 调度引擎调用，配置按 `entity_id` 查询，推荐链按 `shop_id` 查询
- **`ReferralPlanWriter`** — 供 core 的 `init_commission_plan` 写入配置

## Events

| 事件 | 说明 |
|------|------|
| `ReferralConfigUpdated` | 推荐链配置更新（entity_id） |

## Errors

| 错误 | 说明 |
|------|------|
| `InvalidRate` | 返佣率超过 10000 基点 |

## 依赖

```toml
[dependencies]
pallet-entity-common = { path = "../../common" }
pallet-commission-common = { path = "../common" }
```
