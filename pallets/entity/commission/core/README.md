# pallet-commission-core

> 返佣系统核心调度引擎 — 配置管理、记账、提现与偿付安全

## 概述

`pallet-commission-core` 是返佣系统的**核心调度引擎**，负责：

- 全局返佣配置（启用模式、上限、冻结期）
- 返佣记账（`credit_commission`）与取消（`cancel_commission`）
- 提现系统（四种提现模式 + 自愿复购奖励 + 指定复购目标）
- 偿付安全（`ShopPendingTotal` + `ShopShoppingTotal` 资金锁定检查）
- 一键初始化佣金方案（`init_commission_plan`）
- 调度各插件（ReferralPlugin / LevelDiffPlugin / SingleLinePlugin / TeamPlugin）
- **平台费固定分配**：招商人 50% + 国库 50%（无招商人时 100% 进国库）

## 架构

```
订单模块 → CommissionProvider::process_commission()
                    ↓
            ┌─ 平台费无条件分配（无论佣金是否配置）─┐
            │  有招商人: 50% → 招商人, 50% → 国库   │
            │  无招商人: 100% → 国库                 │
            └────────────────────────────────────────┘
                    ↓
            core 调度引擎（双来源并行）
            ├── 池 A：平台费 × ReferrerShareBps → 招商推荐人奖金（EntityReferral）
            └── 池 B：卖家货款 × max_commission_rate → 会员返佣
                ├── 1. ReferralPlugin.calculate()
                ├── 2. LevelDiffPlugin.calculate()
                ├── 3. SingleLinePlugin.calculate()
                └── 4. TeamPlugin.calculate()
                        ↓
            credit_commission() → 记账 + 资金转移
                        ↓
            withdraw_commission() → 提现 + 分级复购
```

## 平台费分配规则

每笔订单的平台费按**全局固定比例**分配，不受 Entity 配置影响：

| 场景 | 招商人（Referrer） | 国库（Treasury） |
|------|---------------------|------------------|
| 有招商人 | `platform_fee × ReferrerShareBps / 10000` (50%) | 剩余部分 (50%) |
| 无招商人 | 0 | `platform_fee` (100%) |
| 未配置佣金 | 0 | `platform_fee` (100%) |

- **`ReferrerShareBps`** 为全局常量（runtime 配置 5000 = 50%），不可按 Entity 修改
- 平台费分配在佣金配置检查**之前**执行，确保平台收入不受佣金配置影响

## Config

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type Currency: Currency<Self::AccountId>;
    type ShopProvider: ShopProvider<Self::AccountId>;
    type EntityProvider: EntityProvider<Self::AccountId>;
    type MemberProvider: MemberProvider<Self::AccountId>;

    /// 四个返佣插件（均实现 CommissionPlugin trait）
    type ReferralPlugin: CommissionPlugin<...>;
    type LevelDiffPlugin: CommissionPlugin<...>;
    type SingleLinePlugin: CommissionPlugin<...>;
    type TeamPlugin: CommissionPlugin<...>;

    /// 招商推荐人查询接口
    type EntityReferrerProvider: EntityReferrerProvider<Self::AccountId>;

    /// 方案写入器（供 init_commission_plan 使用）
    type ReferralWriter: ReferralPlanWriter<BalanceOf<Self>>;
    type LevelDiffWriter: LevelDiffPlanWriter;
    type TeamWriter: TeamPlanWriter<BalanceOf<Self>>;

    /// 平台账户（用于招商奖金从平台费中扣除）
    type PlatformAccount: Get<Self::AccountId>;

    /// 国库账户（接收平台费中推荐人奖金以外的部分）
    type TreasuryAccount: Get<Self::AccountId>;

    /// 招商推荐人分佣比例（基点，5000 = 平台费的 50%）
    /// 全局固定：平台费 = referrer 50% + 国库 50%
    #[pallet::constant]
    type ReferrerShareBps: Get<u16>;

    #[pallet::constant]
    type MaxCommissionRecordsPerOrder: Get<u32>;
    #[pallet::constant]
    type MaxCustomLevels: Get<u32>;
}
```

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `CommissionConfigs` | `Map<u64, CoreCommissionConfig>` | Entity 返佣核心配置 |
| `MemberCommissionStats` | `DoubleMap<u64, AccountId, Stats>` | 会员返佣统计 |
| `OrderCommissionRecords` | `Map<u64, BoundedVec<Record>>` | 订单返佣记录 |
| `ShopCommissionTotals` | `Map<u64, (Balance, u64)>` | Entity 返佣统计 |
| `ShopPendingTotal` | `Map<u64, Balance>` | Entity 待提取佣金总额 |
| `ShopShoppingTotal` | `Map<u64, Balance>` | Entity 购物余额总额 |
| `WithdrawalConfigs` | `Map<u64, EntityWithdrawalConfig>` | 提现配置 |
| `MemberShoppingBalance` | `DoubleMap<u64, AccountId, Balance>` | 会员购物余额 |
| `MemberLastCredited` | `DoubleMap<u64, AccountId, BlockNumber>` | 最后入账区块 |
| `GlobalMinRepurchaseRate` | `Map<u64, u16>` | Governance 全局最低复购比例 |
| `OrderTreasuryTransfer` | `Map<u64, Balance>` | 订单平台费转国库金额（用于取消退款） |

## CoreCommissionConfig

```rust
pub struct CoreCommissionConfig {
    pub enabled_modes: CommissionModes,  // 启用的返佣模式位标志
    pub max_commission_rate: u16,        // 会员返佣上限（从卖家货款扣除）
    pub enabled: bool,                   // 是否启用返佣
    pub withdrawal_cooldown: u32,        // 提现冻结期
}
```

> **注意**：`referrer_share` 字段已移除，招商人分佣比例由全局常量 `ReferrerShareBps` 控制。

## Extrinsics

| call_index | 方法 | 权限 | 说明 |
|------------|------|------|------|
| 0 | `set_commission_modes` | Entity Owner | 设置启用的返佣模式位标志 |
| 1 | `set_commission_rate` | Entity Owner | 设置会员返佣上限（从卖家货款扣除） |
| 2 | `enable_commission` | Entity Owner | 启用/禁用返佣 |
| 3 | `withdraw_commission` | 会员 | 提取返佣（支持四种提现模式） |
| 4 | `set_withdrawal_config` | Entity Owner | 设置提现配置 |
| 5 | `use_shopping_balance` | Entity Owner | 使用购物余额支付 |
| 6 | `init_commission_plan` | Entity Owner | 一键初始化佣金方案 |

> `set_referrer_share` extrinsic 已移除，招商人分佣比例由全局常量 `ReferrerShareBps` 固定。

## 提现系统

### 四种提现模式

| 模式 | 行为 |
|------|------|
| `FullWithdrawal` | 全额提现，Governance 底线仍生效 |
| `FixedRate` | 所有会员统一复购比率 |
| `LevelBased` | 按 level_id 查 default_tier / level_overrides |
| `MemberChoice` | 会员自选复购比率，不低于 min_repurchase_rate |

### 三层约束模型

```
Governance 底线（强制）
    ↓ max()
Entity 模式设定
    ↓ max()
会员选择（MemberChoice 模式）
    ↓
最终复购比率
```

### 自愿多复购奖励

超出强制最低线的部分 × `voluntary_bonus_rate` 额外计入购物余额。

### 指定复购目标

`withdraw_commission` 支持 `repurchase_target` 参数：
- 目标为非会员：自动注册，推荐人 = 出资人
- 目标为已有会员：推荐人必须是出资人，否则拒绝

### 偿付安全

提现前验证：`entity_balance - withdrawal >= remaining_pending + new_shopping_total`

## 资金流向

```
双来源并行（每笔订单同时处理）：

  平台费分配（无条件执行，保障平台收入）：
    ┌─ 有招商人 ─────────────────────────────────────────────┐
    │  平台费 × 50%  → 平台账户 --transfer--> Entity 账户     │
    │  平台费 × 50%  → 平台账户 --transfer--> 国库            │
    └────────────────────────────────────────────────────────┘
    ┌─ 无招商人 ─────────────────────────────────────────────┐
    │  平台费 × 100% → 平台账户 --transfer--> 国库            │
    └────────────────────────────────────────────────────────┘
    取消：Entity 账户 --transfer--> 平台账户（招商奖金退回）
          国库 --transfer(AllowDeath)--> 平台账户（国库部分退回）

  池 B（会员返佣）：
    卖家货款 × max_commission_rate → seller --transfer--> Entity 账户
    取消：Entity 账户 --transfer--> seller

  提现：
    Entity 账户 --transfer--> 会员钱包（提现部分）
                   --credit-->  会员购物余额（复购部分 + 奖励）
```

## Events

| 事件 | 说明 |
|------|------|
| `CommissionConfigUpdated` | 配置更新 |
| `CommissionModesUpdated` | 模式更新 |
| `CommissionDistributed` | 返佣发放（含 entity_id, shop_id, order_id, beneficiary, amount, type, level） |
| `CommissionWithdrawn` | 返佣提取 |
| `CommissionCancelled` | 返佣取消（含 refund_succeeded / refund_failed 计数） |
| `CommissionPlanInitialized` | 佣金方案初始化 |
| `TieredWithdrawal` | 分级提现（含 withdrawn, repurchase, bonus 三部分金额） |
| `WithdrawalConfigUpdated` | 提现配置更新 |
| `ShoppingBalanceUsed` | 购物余额使用 |
| `CommissionFundsTransferred` | 佣金资金转入 Entity（来源: seller 或 平台） |
| `PlatformFeeToTreasury` | 平台费转入国库（有招商人时 50%，无招商人时 100%） |
| `TreasuryRefund` | 国库退款（订单取消时平台费退回平台账户） |
| `CommissionRefundFailed` | 退款失败（需人工干预） |
| `WithdrawalCooldownNotMet` | 提现冻结期未满 |

## Errors

| 错误 | 说明 |
|------|------|
| `ShopNotFound` | 店铺不存在 |
| `EntityNotFound` | 实体不存在 |
| `NotShopOwner` | 不是店主 |
| `NotEntityOwner` | 不是实体所有者 |
| `CommissionNotConfigured` | 返佣未配置 |
| `InsufficientCommission` | 返佣余额不足 |
| `InvalidCommissionRate` | 无效的返佣率 |
| `RecordsFull` | 订单返佣记录已满 |
| `Overflow` | 溢出 |
| `WithdrawalConfigNotEnabled` | 提现配置未启用 |
| `InvalidWithdrawalConfig` | 无效的提现配置 |
| `InsufficientShoppingBalance` | 购物余额不足 |
| `WithdrawalCooldownNotMet` | 提现冻结期未满 |
| `NotDirectReferral` | 复购目标不是出资人的直推下线 |
| `AutoRegisterFailed` | 自动注册会员失败 |
| `ZeroWithdrawalAmount` | 提现金额为 0 |

## Trait 实现

- **`CommissionProvider`** — 外部接口，接收 shop_id，内部解析 entity_id
- **`CommissionFundGuard`** — 佣金资金已转入 Entity 账户，Shop 的 protected_funds 始终为 0

## 依赖

```toml
[dependencies]
pallet-entity-common = { path = "../../common" }
pallet-commission-common = { path = "../common" }
```
