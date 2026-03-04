# pallet-commission-core

> 返佣系统核心调度引擎 — 配置管理、记账、提现与偿付安全

## 概述

`pallet-commission-core` 是返佣系统的**核心调度引擎**，负责：

- 全局返佣配置（启用模式、上限、冻结期）
- 返佣记账（`credit_commission`）与取消（`cancel_commission`）
- 提现系统（四种提现模式 + 自愿复购奖励 + 指定复购目标）
- 偿付安全（`ShopPendingTotal` + `ShopShoppingTotal` 资金锁定检查）
- 调度各插件（ReferralPlugin / LevelDiffPlugin / SingleLinePlugin / TeamPlugin）
- **平台费固定分配**：招商人 50% + 国库 50%（无招商人时 100% 进国库）
- **KYC/合规守卫**：通过 `ParticipationGuard` trait 在提现和购物余额消费前检查参与权

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

    /// 四个 NEX 返佣插件（均实现 CommissionPlugin trait）
    type ReferralPlugin: CommissionPlugin<Self::AccountId, BalanceOf<Self>>;
    type LevelDiffPlugin: CommissionPlugin<Self::AccountId, BalanceOf<Self>>;
    type SingleLinePlugin: CommissionPlugin<Self::AccountId, BalanceOf<Self>>;
    type TeamPlugin: CommissionPlugin<Self::AccountId, BalanceOf<Self>>;

    /// 四个 Token 返佣插件（均实现 TokenCommissionPlugin trait）
    type TokenReferralPlugin: TokenCommissionPlugin<Self::AccountId, TokenBalanceOf<Self>>;
    type TokenLevelDiffPlugin: TokenCommissionPlugin<Self::AccountId, TokenBalanceOf<Self>>;
    type TokenSingleLinePlugin: TokenCommissionPlugin<Self::AccountId, TokenBalanceOf<Self>>;
    type TokenTeamPlugin: TokenCommissionPlugin<Self::AccountId, TokenBalanceOf<Self>>;

    /// 招商推荐人查询接口
    type EntityReferrerProvider: EntityReferrerProvider<Self::AccountId>;

    /// 方案写入器（供 Governance 桥接调用）
    type ReferralWriter: ReferralPlanWriter<BalanceOf<Self>>;
    type LevelDiffWriter: LevelDiffPlanWriter;
    type TeamWriter: TeamPlanWriter<BalanceOf<Self>>;
    type PoolRewardWriter: PoolRewardPlanWriter;

    /// 平台账户（用于招商奖金从平台费中扣除）
    type PlatformAccount: Get<Self::AccountId>;

    /// 国库账户（接收平台费中推荐人奖金以外的部分）
    type TreasuryAccount: Get<Self::AccountId>;

    /// 招商推荐人分佣比例（基点，5000 = 平台费的 50%）
    #[pallet::constant]
    type ReferrerShareBps: Get<u16>;

    /// Token 订单平台费率（基点，全局固定，100 = 1%）
    #[pallet::constant]
    type TokenPlatformFeeRate: Get<u16>;

    #[pallet::constant]
    type MaxCommissionRecordsPerOrder: Get<u32>;

    #[pallet::constant]
    type MaxCustomLevels: Get<u32>;

    /// 关闭 POOL_REWARD 后提取沉淀池资金的冷却期（区块数）
    #[pallet::constant]
    type PoolRewardWithdrawCooldown: Get<BlockNumber>;

    /// Entity Token 余额类型
    type TokenBalance: FullCodec + MaxEncodedLen + TypeInfo + Copy + Default
        + Debug + AtLeast32BitUnsigned + From<u32> + Into<u128>;

    /// Token 转账接口（entity_id 级）
    type TokenTransferProvider: TokenTransferProvider<Self::AccountId, TokenBalanceOf<Self>>;

    /// Entity 参与权守卫（KYC / 合规检查）
    /// 默认使用 `()` 允许所有操作（无 KYC 要求）
    type ParticipationGuard: ParticipationGuard<Self::AccountId>;
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
| `MemberTokenCommissionStats` | `DoubleMap<u64, AccountId, MemberTokenCommissionStatsData>` | Token 佣金统计（ValueQuery） |
| `OrderTokenCommissionRecords` | `Map<u64, BoundedVec<TokenCommissionRecord>>` | Token 订单佣金记录（ValueQuery） |
| `TokenPendingTotal` | `Map<u64, TokenBalance>` | Token 待提取佣金总额（ValueQuery） |
| `UnallocatedTokenPool` | `Map<u64, TokenBalance>` | Token 未分配沉淀池（ValueQuery） |
| `OrderTokenUnallocated` | `Map<u64, (u64, u64, TokenBalance)>` | Token 订单沉淀记录（ValueQuery） |
| `MemberTokenShoppingBalance` | `DoubleMap<u64, AccountId, TokenBalance>` | Token 购物余额（ValueQuery） |
| `TokenShoppingTotal` | `Map<u64, TokenBalance>` | Token 购物余额总额（ValueQuery，资金锁定） |
| `TokenWithdrawalConfigs` | `Map<u64, EntityWithdrawalConfig>` | Token 提现配置（OptionQuery） |
| `GlobalMinTokenRepurchaseRate` | `Map<u64, u16>` | Token Governance 全局最低复购比例（ValueQuery） |
| `EntityTokenAccountedBalance` | `Map<u64, TokenBalance>` | Entity Token 已知渠道余额（用于 sweep 检测外部转入） |
| `GlobalMaxCommissionRate` | `Map<u64, u16>` | F15: 全局佣金率上限（ValueQuery，0=无限制） |

## 核心结构体

### CoreCommissionConfig

```rust
pub struct CoreCommissionConfig {
    pub enabled_modes: CommissionModes,  // 启用的返佣模式位标志
    pub max_commission_rate: u16,        // 会员返佣上限（基点，从卖家货款扣除）
    pub enabled: bool,                   // 是否全局启用
    pub withdrawal_cooldown: u32,        // NEX 提现冻结期（区块数，0 = 无冻结）
    pub creator_reward_rate: u16,        // 创建人收益比例（基点，0 = 不启用，上限 5000）
    pub token_withdrawal_cooldown: u32,  // F3: Token 提现冻结期（0 = 使用 withdrawal_cooldown）
}
```

> **注意**：`referrer_share` 字段已移除，招商人分佣比例由全局常量 `ReferrerShareBps` 控制。
> **F3**: Token 提现冻结期独立于 NEX，`token_withdrawal_cooldown=0` 时回退到 `withdrawal_cooldown`。

### EntityWithdrawalConfig

```rust
pub struct EntityWithdrawalConfig<MaxLevels: Get<u32>> {
    pub mode: WithdrawalMode,                                         // 提现模式
    pub default_tier: WithdrawalTierConfig,                           // LevelBased 默认层配置
    pub level_overrides: BoundedVec<(u8, WithdrawalTierConfig), MaxLevels>, // 按 level_id 覆写
    pub voluntary_bonus_rate: u16,                                    // 自愿多复购奖励（万分比）
    pub enabled: bool,                                                // 是否启用
}
```

`WithdrawalTierConfig` 包含 `withdrawal_rate` + `repurchase_rate`，两者之和必须等于 10000。

## Extrinsics

| call_index | 方法 | 权限 | 说明 |
|------------|------|------|------|
| 0 | `set_commission_modes` | Owner/Admin | 设置启用的返佣模式位标志 |
| 1 | `set_commission_rate` | Owner/Admin | 设置会员返佣上限（≤10000 基点，受 F15 全局上限约束） |
| 2 | `enable_commission` | Owner/Admin | 启用/禁用返佣 |
| 3 | `withdraw_commission` | 会员 | 提取返佣（支持四种提现模式 + 指定复购目标） |
| 4 | `set_withdrawal_config` | Owner/Admin | 设置提现配置（含 level_id 唯一性校验） |
| 5 | `use_shopping_balance` | ~~会员~~ **已禁用** | 购物余额仅可用于购物（下单抵扣），不可直接提取为 NEX → `ShoppingBalanceWithdrawalDisabled` |
| 6 | `init_commission_plan` | ~~Entity Owner~~ **已禁用** | 过度设计，前端改用 `utility.batch` 组合分步 extrinsics → `CommissionPlanDisabled` |
| 8 | `withdraw_token_commission` | 会员 | 提取 Token 佣金（F3: 使用独立 token_withdrawal_cooldown） |
| 10 | `set_token_withdrawal_config` | Owner/Admin | 设置 Token 提现配置（与 NEX 对称，独立存储） |
| 11 | `set_global_min_token_repurchase_rate` | Root | 设置 Token Governance 全局最低复购比例 |
| 12 | `withdraw_entity_funds` | Entity Owner | 提取 Entity NEX 自由余额（需保留 PendingTotal + ShoppingTotal + UnallocatedPool） |
| 13 | `withdraw_entity_token_funds` | Entity Owner | 提取 Entity Token 自由余额（需保留 TokenPendingTotal + TokenShoppingTotal + UnallocatedTokenPool） |
| 14 | `set_creator_reward_rate` | Owner/Admin | 设置创建人收益比例（基点，上限 5000） |
| 15 | `set_token_platform_fee_rate` | Root | 设置 Token 平台费率（基点，上限 1000 = 10%） |
| 16 | `set_global_min_repurchase_rate` | Root | F13: 设置 NEX 全局最低复购比例 |
| 17 | `set_withdrawal_cooldown` | Owner/Admin | F2: 设置 NEX/Token 独立提现冻结期 |
| 18 | `force_disable_entity_commission` | Root | F14: 紧急禁用 Entity 佣金（不可逆） |
| 19 | `set_global_max_commission_rate` | Root | F15: 设置全局佣金率上限（0=无限制） |
| 20 | `clear_commission_config` | Owner/Admin | F4: 清除佣金配置 |
| 21 | `clear_withdrawal_config` | Owner/Admin | F4: 清除 NEX 提现配置 |
| 22 | `clear_token_withdrawal_config` | Owner/Admin | F4: 清除 Token 提现配置 |

> **F1 权限模型**: call_index 0,1,2,4,10,14,17,20,21,22 支持 Owner 或 Admin（需 `COMMISSION_MANAGE` 权限位）。call_index 12,13（资金提取）仅限 Owner。

### set_withdrawal_config 校验规则

- **模式参数**：`FixedRate.repurchase_rate ≤ 10000`，`MemberChoice.min_repurchase_rate ≤ 10000`
- **Tier 配置**：`default_tier` 和每个 `level_overrides` 的 `withdrawal_rate + repurchase_rate == 10000`
- **level_id 唯一性**：`level_overrides` 中不允许重复的 level_id（`DuplicateLevelId` 错误）
- **bonus rate**：`voluntary_bonus_rate ≤ 10000`

### withdraw_commission 流程

1. 确定复购目标账户（自己 or 指定目标）
2. 如果目标不是自己：校验推荐关系 或 自动注册
3. **H1 修复**: auto_register 后验证 target 已成为正式会员（`TargetNotApprovedMember`）
4. **H3 修复**: `ParticipationGuard::can_participate` 检查 target 是否满足 Entity KYC 要求（`TargetParticipationDenied`）
5. `WithdrawalConfig` 启用检查（`WithdrawalConfigNotEnabled`）
6. 冻结期检查（`withdrawal_cooldown` > 0 时）
7. `calc_withdrawal_split` 计算提现/复购/奖励分配
8. 偿付安全检查：`entity_balance - withdrawal ≥ remaining_pending + new_shopping_total`
9. 转账提现部分到用户钱包
10. 复购 + 奖励记入目标账户购物余额

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
- 目标为非会员 → 自动注册（推荐人 = 出资人），注册后验证会员状态
- 目标为已有会员且推荐人是出资人 → 允许
- 目标为已有会员但推荐人非出资人 → `NotDirectReferral` 错误
- APPROVAL_REQUIRED 策略下 target 仅进入 PendingMembers → `TargetNotApprovedMember` 错误
- Entity 配置 mandatory KYC 且 target 未通过 → `TargetParticipationDenied` 错误

### 购物余额使用规则

购物余额**仅可用于购物**（通过 `place_order` 下单抵扣），不可直接提取为 NEX：

| 路径 | 函数 | 行为 | 状态 |
|------|------|------|------|
| 下单抵扣 | `ShoppingBalanceProvider::consume` → `do_consume_shopping_balance` | NEX 从 Entity 转入买家钱包 → Escrow 锁定 | ✅ 允许 |
| 直接提现 | `use_shopping_balance` extrinsic | — | ❌ 已禁用 |
| 纯记账 | `CommissionProvider::use_shopping_balance` → `do_use_shopping_balance` | 仅扣减记账 | ✅ 允许 |

`do_consume_shopping_balance` 的安全检查：
1. **H3**: KYC 参与权检查 → `ParticipationRequirementNotMet`
2. 余额充足性 → `InsufficientShoppingBalance`

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
| `do_use_shopping_balance` | 使用购物余额纯记账（供 CommissionProvider 调用，不转 NEX） |
| `do_consume_shopping_balance` | 消费购物余额（扣减记账 + NEX 从 Entity 转入会员钱包，供 ShoppingBalanceProvider 调用） |
| ~~`resolve_entity_id`~~ | L1 修复: 已移除（死代码，未被任何代码路径调用） |
| `ensure_owner_or_admin` | F1: 验证 Entity Owner 或 Admin(COMMISSION_MANAGE) 权限 |
| `ensure_entity_owner` | 验证 Entity Owner（仅 Owner，用于资金提取等敏感操作） |

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
| `CommissionPlanRemoved` | entity_id | [占位] init_commission_plan 已移除 |
| `TieredWithdrawal` | entity_id, account, **repurchase_target**, withdrawn_amount, repurchase_amount, bonus_amount | 分级提现（M3 修复: 含购物余额实际接收账户） |
| `WithdrawalConfigUpdated` | entity_id | 提现配置更新 |
| `ShoppingBalanceUsed` | entity_id, account, amount | 购物余额使用 |
| `CommissionFundsTransferred` | entity_id, shop_id, amount | 佣金资金转入 Entity |
| `PlatformFeeToTreasury` | order_id, amount | 平台费转入国库 |
| `TreasuryRefund` | order_id, amount | 国库退款（订单取消） |
| `CommissionRefundFailed` | entity_id, shop_id, amount | 退款失败（需人工干预） |
| `WithdrawalCooldownNotMet` | entity_id, account, earliest_block | 提现冻结期未满 |
| `GlobalMinTokenRepurchaseRateSet` | entity_id, rate | M1 修复: Token Governance 全局最低复购比例变更 |
| `GlobalMinRepurchaseRateSet` | entity_id, rate | L5 修复: NEX Governance 全局最低复购比例变更 |
| `TokenPlatformFeeRateUpdated` | old_rate, new_rate | Token 平台费率变更 |
| `CommissionForceDisabled` | entity_id | F14: Root 紧急禁用 Entity 佣金 |
| `GlobalMaxCommissionRateSet` | entity_id, rate | F15: 全局佣金率上限变更 |
| `WithdrawalCooldownUpdated` | entity_id, nex_cooldown, token_cooldown | F2: 提现冻结期变更 |
| `CommissionConfigCleared` | entity_id | F4: 佣金配置已清除 |
| `WithdrawalConfigCleared` | entity_id | F4: NEX 提现配置已清除 |
| `TokenWithdrawalConfigCleared` | entity_id | F4: Token 提现配置已清除 |

## Errors

| 错误 | 说明 |
|------|------|
| `ShopNotFound` | 店铺不存在 |
| `EntityNotFound` | 实体不存在 |
| `NotShopOwner` | 不是店主 |
| `NotEntityOwner` | 不是实体所有者 |
| `CommissionNotConfigured` | 返佣未配置 |
| `InsufficientCommission` | 返佣 pending 余额不足（M2 修复: 偿付安全检查已改用 `InsufficientEntityFunds`） |
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
| `TargetNotApprovedMember` | H1: 复购目标未通过审批（APPROVAL_REQUIRED 策略下） |
| `MemberNotActivated` | ~~已废弃~~ 激活机制已移除，保留错误码供兼容 |
| `TargetParticipationDenied` | H3: 复购目标不满足 Entity 参与要求（如 mandatory KYC） |
| `ParticipationRequirementNotMet` | H3: 账户不满足 Entity 参与要求，无法消费购物余额 |
| `ShoppingBalanceWithdrawalDisabled` | 购物余额仅可用于购物，不可直接提取为 NEX |
| `CommissionPlanDisabled` | init_commission_plan 已禁用，请使用 utility.batch 组合分步 extrinsics |
| `InsufficientEntityFunds` | M2 修复: Entity 账户 NEX 偿付能力不足（原复用 InsufficientCommission） |
| `InsufficientEntityTokenFunds` | M2 修复: Entity 账户 Token 偿付能力不足（原复用 InsufficientTokenCommission） |
| `InsufficientTokenCommission` | Token 佣金 pending 余额不足（M2 修复: Token 偿付安全检查已改用 `InsufficientEntityTokenFunds`） |
| `TokenPlatformFeeRateTooHigh` | Token 平台费率超过上限（最大 1000 bps = 10%） |
| `EntityLocked` | 实体已被全局锁定，所有配置操作不可用 |
| `PoolRewardCooldownActive` | POOL_REWARD 关闭后冷却期未满 |
| `NotEntityOwnerOrAdmin` | F1: 调用者既不是 Owner 也不是拥有 COMMISSION_MANAGE 的 Admin |
| `CommissionRateExceedsGlobalMax` | F15: max_commission_rate 超过全局上限 |
| `ConfigNotFound` | F4: 配置不存在，无法清除 |

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

## ParticipationGuard Trait

KYC/合规检查的泛型接口，在 `withdraw_commission` 和 `do_consume_shopping_balance` 中调用：

```rust
pub trait ParticipationGuard<AccountId> {
    fn can_participate(entity_id: u64, account: &AccountId) -> bool;
}

/// 默认空实现（无 KYC 系统时使用，所有账户均允许）
impl<AccountId> ParticipationGuard<AccountId> for () {
    fn can_participate(_: u64, _: &AccountId) -> bool { true }
}
```

Runtime 通过 `KycParticipationGuard` 桥接 `pallet-entity-kyc::can_participate_in_entity`：
- Entity 未配置 `EntityRequirements` 或 `mandatory=false` → 返回 `true`（允许所有）
- Entity 配置 `mandatory=true` → 检查账户 KYC 状态、级别、国家、风险评分、过期

## 审计修复记录

| 编号 | 级别 | 修复 | 说明 |
|------|------|------|------|
| H2 | High | `cancel_commission` | 先尝试转账，成功后才取消记录和更新统计，防止转账失败但记录已标记 Cancelled |
| CC-M1 | Medium | `CommissionCancelled` 事件 | 增加 refund_succeeded / refund_failed 计数 |
| C1 | Critical | `withdraw_commission` | 偿付安全检查计入 repurchase + bonus 对 ShopShoppingTotal 的增量 |
| M1 | Low | `set_withdrawal_config` | level_overrides 添加 level_id 唯一性校验（DuplicateLevelId 错误） |
| H1-rep | High | `withdraw_commission` | APPROVAL_REQUIRED 策略下 auto_register 后验证 target 会员状态（`TargetNotApprovedMember`） |
| H1-audit | High | `withdraw_commission` | WithdrawalConfig 未启用时拒绝提现（`WithdrawalConfigNotEnabled`） |
| H3-rep | High | `withdraw_commission` / `do_consume_shopping_balance` | 引入 `ParticipationGuard` trait 检查 Entity KYC 参与要求 |
| H3-stats | High | `withdraw_commission` | `stats.repurchased` 含 bonus（修复统计不完整） |
| M2-rep | ~~Removed~~ | `do_consume_shopping_balance` | ~~激活检查已移除（过度设计）~~ KYC 参与权检查已足够 |
| M3 | Medium | `TieredWithdrawal` 事件 | 新增 `repurchase_target` 字段 |
| — | — | `use_shopping_balance` extrinsic | 禁用直接提现，购物余额仅可用于购物 |
| M1-R4 | Medium | `set_global_min_token_repurchase_rate` | 新增 `GlobalMinTokenRepurchaseRateSet` 事件，Governance 比例变更可审计 |
| M2-R4 | Medium | `withdraw_commission` / `withdraw_token_commission` | 偿付安全检查改用 `InsufficientEntityFunds` / `InsufficientEntityTokenFunds`，与 pending 不足区分 |
| M3-R4 | Medium | `cancel_commission` | 消除 Token 取消逻辑重复，复用 `do_cancel_token_commission` |
| M4-R4 | Medium | `CommissionProvider::set_commission_modes` | 使用 `CommissionModes::is_valid()`（单一事实来源）替代手动掩码 |
| L1-R4 | Low | `resolve_entity_id` | 移除死代码 |
| L5-R4 | Low | `CommissionProvider::set_min_repurchase_rate` | 新增 `GlobalMinRepurchaseRateSet` 事件 |

### 功能扩展记录

| 编号 | 说明 |
|------|------|
| F1 | Admin 权限支持 — `ensure_owner_or_admin` + `COMMISSION_MANAGE` 权限位，配置 extrinsics 支持 Owner/Admin，资金提取仅 Owner |
| F2 | `set_withdrawal_cooldown` 独立 extrinsic (call_index 17) — NEX/Token 冻结期独立配置 |
| F3 | Token 独立冻结期 — `token_withdrawal_cooldown` 字段，0 = 回退到 `withdrawal_cooldown` |
| F4 | `clear_commission_config` / `clear_withdrawal_config` / `clear_token_withdrawal_config` (call_index 20-22) |
| F13 | NEX 全局最低复购比例 `set_global_min_repurchase_rate` (call_index 16) — 与 Token 版对称 |
| F14 | `force_disable_entity_commission` (call_index 18) — Root 紧急禁用 Entity 佣金 |
| F15 | `GlobalMaxCommissionRate` storage + `set_global_max_commission_rate` (call_index 19) — 治理佣金率上限 |

## 测试覆盖

144 个测试（`cargo test -p pallet-commission-core`）：

- **set_commission_rate**: works / rejects_invalid / rejects_non_owner
- **process_commission（平台费分配）**: referrer_gets_half / dual_source / referrer_skipped_no_referrer / referrer_skipped_zero_fee / referrer_capped / referrer_stats
- **process_commission（国库）**: 50_50_split / full_to_treasury / no_transfer_zero / capped_by_balance
- **cancel_commission**: refunds_all（双来源 + 国库退款）
- **未配置佣金**: treasury_receives_even_without_config
- **init_commission_plan**: is_disabled（验证已禁用）
- **set_withdrawal_config**: m1_rejects_duplicate_level_id
- **提现审计**: h1_withdraw_blocked_when_config_disabled / h1_withdraw_allowed_when_no_config
- **统计修复**: h3_repurchased_includes_bonus
- **事件修复**: m3_event_includes_repurchase_target
- **提现模式**: fixed_rate_withdrawal_split_works / governance_floor_enforced_in_full_withdrawal_mode
- **H3 KYC**: h3_withdraw_blocked_when_target_participation_denied / h3_consume_shopping_balance_blocked_when_participation_denied / h3_self_withdraw_not_checked_by_participation_guard
- **购物余额**: use_shopping_balance_extrinsic_always_rejected
- **Round 4 回归**: m1_set_global_min_token_repurchase_rate_emits_event / l5_set_min_repurchase_rate_via_trait_emits_event / m2_withdraw_commission_solvency_uses_entity_funds_error / m2_withdraw_token_commission_solvency_uses_entity_token_funds_error / m3_cancel_commission_still_cancels_token_records / m4_trait_set_commission_modes_uses_is_valid
- **F1 Admin 权限**: f1_admin_can_set_commission_modes / f1_admin_can_set_commission_rate / f1_admin_can_enable_commission / f1_admin_without_permission_rejected / f1_non_owner_non_admin_rejected / f1_owner_still_works / f1_admin_cannot_withdraw_entity_funds
- **F13 NEX 全局最低复购**: f13_set_global_min_repurchase_rate_works / f13_rejects_non_root / f13_rejects_over_10000
- **F2 提现冻结期**: f2_set_withdrawal_cooldown_works / f2_set_withdrawal_cooldown_admin_works
- **F3 Token 独立冻结期**: f3_token_uses_independent_cooldown / f3_token_fallback_to_nex_cooldown_when_zero
- **F14 Root 紧急暂停**: f14_force_disable_works / f14_force_disable_rejects_non_root / f14_force_disable_creates_config_if_absent
- **F15 全局佣金率上限**: f15_set_global_max_commission_rate_works / f15_set_commission_rate_blocked_by_global_max / f15_zero_global_max_means_no_limit / f15_rejects_non_root
- **F4 清除配置**: f4_clear_commission_config_works / f4_clear_commission_config_rejects_absent / f4_clear_withdrawal_config_works / f4_clear_token_withdrawal_config_works / f4_clear_config_admin_works

## 依赖

```toml
[dependencies]
pallet-entity-common = { path = "../../common" }
pallet-commission-common = { path = "../common" }
```
