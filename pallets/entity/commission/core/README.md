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
    type RuntimeEvent: From<Event<Self>> + IsType<...>;
    type Currency: Currency<Self::AccountId>;

    type ShopProvider: ShopProvider<Self::AccountId>;
    type EntityProvider: EntityProvider<Self::AccountId>;
    type MemberProvider: MemberProvider<Self::AccountId>;

    /// 四个返佣插件（均实现 CommissionPlugin trait）
    type ReferralPlugin: CommissionPlugin<Self::AccountId, BalanceOf<Self>>;
    type LevelDiffPlugin: CommissionPlugin<Self::AccountId, BalanceOf<Self>>;
    type SingleLinePlugin: CommissionPlugin<Self::AccountId, BalanceOf<Self>>;
    type TeamPlugin: CommissionPlugin<Self::AccountId, BalanceOf<Self>>;

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
| `MemberCommissionStats` | `DoubleMap<u64, AccountId, MemberCommissionStatsData>` | 会员返佣统计（ValueQuery） |
| `OrderCommissionRecords` | `Map<u64, BoundedVec<CommissionRecord, MaxCommissionRecordsPerOrder>>` | 订单返佣记录（ValueQuery） |
| `ShopCommissionTotals` | `Map<u64, (Balance, u64)>` | Entity 返佣统计 (total_distributed, total_orders)（ValueQuery） |
| `ShopPendingTotal` | `Map<u64, Balance>` | Entity 待提取佣金总额（ValueQuery） |
| `ShopShoppingTotal` | `Map<u64, Balance>` | Entity 购物余额总额（ValueQuery，资金锁定） |
| `WithdrawalConfigs` | `Map<u64, EntityWithdrawalConfig>` | 提现配置（OptionQuery） |
| `MemberShoppingBalance` | `DoubleMap<u64, AccountId, Balance>` | 会员购物余额（ValueQuery） |
| `MemberLastCredited` | `DoubleMap<u64, AccountId, BlockNumber>` | 最后入账区块（ValueQuery，用于冻结期检查） |
| `GlobalMinRepurchaseRate` | `Map<u64, u16>` | Governance 全局最低复购比例（ValueQuery） |
| `OrderTreasuryTransfer` | `Map<u64, Balance>` | 订单平台费转国库金额（ValueQuery，用于取消退款） |

## 核心结构体

### CoreCommissionConfig

```rust
pub struct CoreCommissionConfig {
    pub enabled_modes: CommissionModes,  // 启用的返佣模式位标志
    pub max_commission_rate: u16,        // 会员返佣上限（基点，从卖家货款扣除）
    pub enabled: bool,                   // 是否全局启用
    pub withdrawal_cooldown: u32,        // 提现冻结期（区块数，0 = 无冻结）
}
```

> **注意**：`referrer_share` 字段已移除，招商人分佣比例由全局常量 `ReferrerShareBps` 控制。

### EntityWithdrawalConfig

```rust
pub struct EntityWithdrawalConfig<MaxLevels: Get<u32>> {
    pub mode: WithdrawalMode,                                         // 提现模式
    pub default_tier: WithdrawalTierConfig,                           // LevelBased 默认层配置
    pub level_overrides: BoundedVec<(u8, WithdrawalTierConfig), MaxLevels>, // 按 level_id 覆写
    pub voluntary_bonus_rate: u16,                                    // 自愿多复购奖励（万分比）
    pub enabled: bool,                                                // 是否启用
    pub shopping_balance_generates_commission: bool,                   // 购物余额是否产生佣金
}
```

`WithdrawalTierConfig` 包含 `withdrawal_rate` + `repurchase_rate`，两者之和必须等于 10000。

## Extrinsics

| call_index | 方法 | 权限 | 说明 |
|------------|------|------|------|
| 0 | `set_commission_modes` | Entity Owner | 设置启用的返佣模式位标志 |
| 1 | `set_commission_rate` | Entity Owner | 设置会员返佣上限（≤10000 基点，从卖家货款扣除） |
| 2 | `enable_commission` | Entity Owner | 启用/禁用返佣 |
| 3 | `withdraw_commission` | 会员 | 提取返佣（支持四种提现模式 + 指定复购目标） |
| 4 | `set_withdrawal_config` | Entity Owner | 设置提现配置（含 level_id 唯一性校验） |
| 5 | `use_shopping_balance` | Entity Owner | 使用购物余额支付 |
| 6 | `init_commission_plan` | Entity Owner | 一键初始化佣金方案（None/DirectOnly/MultiLevel/LevelDiff/Custom） |

### set_withdrawal_config 校验规则

- **模式参数**：`FixedRate.repurchase_rate ≤ 10000`，`MemberChoice.min_repurchase_rate ≤ 10000`
- **Tier 配置**：`default_tier` 和每个 `level_overrides` 的 `withdrawal_rate + repurchase_rate == 10000`
- **level_id 唯一性**：`level_overrides` 中不允许重复的 level_id（`DuplicateLevelId` 错误）
- **bonus rate**：`voluntary_bonus_rate ≤ 10000`

### withdraw_commission 流程

1. 确定复购目标账户（自己 or 指定目标）
2. 如果目标不是自己：校验推荐关系 或 自动注册
3. 冻结期检查（`withdrawal_cooldown` > 0 时）
4. `calc_withdrawal_split` 计算提现/复购/奖励分配
5. 偿付安全检查：`entity_balance - withdrawal ≥ remaining_pending + new_shopping_total`
6. 转账提现部分到用户钱包
7. 复购 + 奖励记入目标账户购物余额

## 提现系统

### 四种提现模式

| 模式 | 行为 |
|------|------|
| `FullWithdrawal` | 不强制复购，Governance 底线仍生效 |
| `FixedRate { repurchase_rate }` | 所有会员统一复购比率 |
| `LevelBased` | 通过 `MemberProvider::custom_level_id_by_entity` 获取会员等级，查 `level_overrides` 匹配，未匹配时回退 `default_tier` |
| `MemberChoice { min_repurchase_rate }` | 会员提现时自选比率，不低于 min_repurchase_rate |

### LevelBased 模式详解

`calc_withdrawal_split` 中 LevelBased 分支的工作流：

1. 调用 `T::MemberProvider::custom_level_id_by_entity(entity_id, who)` 获取会员有效等级
   - 该调用会检查 `MemberLevelExpiry`，过期时自动重算等级
2. 在 `config.level_overrides` 中线性查找匹配的 `level_id`
3. 找到 → 使用对应 `WithdrawalTierConfig`
4. 未找到 → 回退到 `config.default_tier`（设计意图：兜底配置）

### 三层约束模型

```
Governance 底线（GlobalMinRepurchaseRate，强制）
    ↓ max()
Entity 模式设定（FullWithdrawal / FixedRate / LevelBased / MemberChoice）
    ↓ max()
会员选择（MemberChoice 模式下的 requested_rate）
    ↓
最终复购比率（final_repurchase_rate，≤ 10000）
```

### 自愿多复购奖励

超出强制最低线的部分 × `voluntary_bonus_rate` 额外计入购物余额：

```
bonus = (repurchase - mandatory_repurchase) × voluntary_bonus_rate / 10000
```

### 指定复购目标

`withdraw_commission` 的 `repurchase_target` 参数：
- `None` → 复购到自己的购物余额
- 目标为非会员 → 自动注册（推荐人 = 出资人）
- 目标为已有会员且推荐人是出资人 → 允许
- 目标为已有会员但推荐人非出资人 → `NotDirectReferral` 错误

### 偿付安全

提现前验证 Entity 账户有足够资金覆盖所有承诺：

```
entity_balance - withdrawal ≥ (old_pending - total_amount) + (old_shopping + repurchase + bonus)
```

## 内部函数

| 函数 | 说明 |
|------|------|
| `process_commission` | 调度引擎：双来源架构处理订单返佣（平台费 + 卖家货款） |
| `credit_commission` | 记录并发放返佣（写入 Records/Stats/PendingTotal/LastCredited） |
| `cancel_commission` | 取消订单返佣（H2 审计修复：先转账后更新记录，防止转账失败但记录已取消） |
| `calc_withdrawal_split` | 计算提现/复购/奖励分配（三层约束模型） |
| `do_use_shopping_balance` | 使用购物余额内部实现（供 extrinsic 和 CommissionProvider 调用） |
| `resolve_entity_id` | 从 shop_id 解析 entity_id |
| `ensure_entity_owner` | 验证 Entity 所有者权限 |

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
    seller 货款 × max_commission_rate → seller --transfer--> Entity 账户
    取消：Entity 账户 --transfer--> seller（H2 审计修复：仅转账成功的记录标记取消）

  提现：
    Entity 账户 --transfer(KeepAlive)--> 会员钱包（提现部分）
                   --credit-->  会员购物余额（复购 + 奖励）
                   ShopShoppingTotal 同步增加（资金锁定）
```

## Events

| 事件 | 字段 | 说明 |
|------|------|------|
| `CommissionConfigUpdated` | entity_id | 核心配置更新 |
| `CommissionModesUpdated` | entity_id, modes | 返佣模式更新 |
| `CommissionDistributed` | entity_id, shop_id, order_id, beneficiary, amount, commission_type, level | 返佣发放 |
| `CommissionWithdrawn` | entity_id, account, amount | 返佣提取 |
| `CommissionCancelled` | order_id, refund_succeeded, refund_failed | 返佣取消（CC-M1 审计修复：含成功/失败计数） |
| `CommissionPlanInitialized` | entity_id, plan | 佣金方案初始化 |
| `TieredWithdrawal` | entity_id, account, withdrawn_amount, repurchase_amount, bonus_amount | 分级提现（三部分金额） |
| `WithdrawalConfigUpdated` | entity_id | 提现配置更新 |
| `ShoppingBalanceUsed` | entity_id, account, amount | 购物余额使用 |
| `CommissionFundsTransferred` | entity_id, shop_id, amount | 佣金资金转入 Entity |
| `PlatformFeeToTreasury` | order_id, amount | 平台费转入国库 |
| `TreasuryRefund` | order_id, amount | 国库退款（订单取消） |
| `CommissionRefundFailed` | entity_id, shop_id, amount | 退款失败（需人工干预） |
| `WithdrawalCooldownNotMet` | entity_id, account, earliest_block | 提现冻结期未满 |

## Errors

| 错误 | 说明 |
|------|------|
| `ShopNotFound` | 店铺不存在 |
| `EntityNotFound` | 实体不存在 |
| `NotShopOwner` | 不是店主 |
| `NotEntityOwner` | 不是实体所有者 |
| `CommissionNotConfigured` | 返佣未配置 |
| `InsufficientCommission` | 返佣余额不足 / 偿付安全检查未通过 |
| `InvalidCommissionRate` | 无效的返佣率（> 10000） |
| `RecordsFull` | 订单返佣记录达到 MaxCommissionRecordsPerOrder 上限 |
| `Overflow` | 数值溢出 |
| `WithdrawalConfigNotEnabled` | 提现配置未启用 |
| `InvalidWithdrawalConfig` | 无效的提现配置（tier 比率之和 ≠ 10000 / bonus_rate > 10000 / 模式参数越界） |
| `InsufficientShoppingBalance` | 购物余额不足 |
| `WithdrawalCooldownNotMet` | 提现冻结期未满 |
| `NotDirectReferral` | 复购目标不是出资人的直推下线 |
| `AutoRegisterFailed` | 自动注册会员失败 |
| `ZeroWithdrawalAmount` | 提现金额为 0 |
| `DuplicateLevelId` | LevelBased 配置中 level_overrides 存在重复的 level_id |

## Trait 实现

### CommissionProvider

外部接口，接收 `shop_id`，内部解析 `entity_id`。提供以下方法：

| 方法 | 说明 |
|------|------|
| `process_commission` | 处理订单返佣（双来源） |
| `cancel_commission` | 取消订单返佣 |
| `pending_commission` | 查询会员待提取佣金 |
| `shopping_balance` | 查询会员购物余额 |
| `use_shopping_balance` | 使用购物余额 |
| `set_commission_modes` | 设置返佣模式（Governance 调用） |
| `set_direct_reward_rate` | 设置直推奖励比率 |
| `set_level_diff_config` | 设置等级极差配置 |
| `set_fixed_amount` | 设置固定金额奖励 |
| `set_first_order_config` | 设置首单奖励配置 |
| `set_repeat_purchase_config` | 设置复购奖励配置 |
| `set_withdrawal_config_by_governance` | Governance 设置提现配置 |
| `set_min_repurchase_rate` | 设置 Governance 全局最低复购比例 |

### CommissionFundGuard

佣金资金已转入 Entity 账户，Shop 的 `protected_funds` 始终返回 0。

## 审计修复记录

| 编号 | 级别 | 修复 | 说明 |
|------|------|------|------|
| H2 | High | `cancel_commission` | 先尝试转账，成功后才取消记录和更新统计，防止转账失败但记录已标记 Cancelled |
| CC-M1 | Medium | `CommissionCancelled` 事件 | 增加 refund_succeeded / refund_failed 计数 |
| C1 | Critical | `withdraw_commission` | 偿付安全检查计入 repurchase + bonus 对 ShopShoppingTotal 的增量 |
| M1 | Low | `set_withdrawal_config` | level_overrides 添加 level_id 唯一性校验（DuplicateLevelId 错误） |

## 测试覆盖

19 个测试（`cargo test -p pallet-commission-core`）：

- **set_commission_rate**: works / rejects_invalid / rejects_non_owner
- **process_commission（平台费分配）**: referrer_gets_half / dual_source / referrer_skipped_no_referrer / referrer_skipped_zero_fee / referrer_capped / referrer_stats
- **process_commission（国库）**: 50_50_split / full_to_treasury / no_transfer_zero / capped_by_balance
- **cancel_commission**: refunds_all（双来源 + 国库退款）
- **未配置佣金**: treasury_receives_even_without_config
- **init_commission_plan**: works
- **set_withdrawal_config**: m1_rejects_duplicate_level_id（M1 审计回归测试）

## 依赖

```toml
[dependencies]
pallet-entity-common = { path = "../../common" }
pallet-commission-common = { path = "../common" }
```
