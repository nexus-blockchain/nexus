# pallet-commission-core

> 返佣系统核心调度引擎 — 双资产（NEX + Token）配置管理、记账、提现与偿付安全

## 概述

`pallet-commission-core` 是返佣系统的**核心调度引擎**，负责：

- **全局返佣配置**：启用模式（位标志）、佣金率上限、冻结期、创建人收益
- **双资产管线**：NEX 与 Token 并行处理，各自独立配置、独立记账、独立提现
- **返佣记账**：`credit_commission` / `credit_token_commission` 记录并分发佣金
- **取消与退款**：`cancel_commission` / `do_cancel_token_commission` 订单取消时全链路退款
- **提现系统**：四种提现模式 + 三层约束模型 + 自愿复购奖励 + 指定复购目标
- **偿付安全**：提现前验证 Entity 账户可覆盖所有 pending + shopping + pool 承诺
- **插件调度**：5 个 NEX 插件 + 5 个 Token 插件（Referral / MultiLevel / LevelDiff / SingleLine / Team）
- **平台费固定分配**：招商人 `ReferrerShareBps%` + 国库剩余（无招商人时 100% 进国库）
- **KYC/合规守卫**：`ParticipationGuard` trait 在提现和购物余额消费前检查参与权
- **治理控制**：全局暂停、Entity 级暂停、全局佣金率上限、全局最低复购比例、Root 紧急禁用
- **订单归档**：`archive_order_records` 清理已完结订单的全部 per-order 存储

## 架构

```
订单模块 ──→ CommissionProvider::process_commission()     (NEX)
         ──→ TokenCommissionProvider::process_token_commission() (Token)
                          ↓
              ┌─ 平台费无条件分配（无论佣金是否配置）──────────┐
              │  NEX:  有招商人: ReferrerShareBps% → 招商人   │
              │                  剩余 → 国库                  │
              │        无招商人: 100% → 国库                  │
              │  Token: 有招商人: ReferrerShareBps% → 招商人  │
              │         剩余 → UnallocatedTokenPool (留存)    │
              └───────────────────────────────────────────────┘
                          ↓
              core 调度引擎（双来源并行）
              ├── 池 A：平台费 → 招商推荐人奖金（EntityReferral）+ 留存/国库
              └── 池 B：卖家货款/Entity Token × max_commission_rate → 会员返佣
                  ├── 0. CreatorReward（创建人收益，优先扣除）
                  ├── 1. ReferralPlugin.calculate()
                  ├── 2. MultiLevelPlugin.calculate()
                  ├── 3. LevelDiffPlugin.calculate()
                  ├── 4. SingleLinePlugin.calculate()
                  └── 5. TeamPlugin.calculate()
                          ↓ 剩余
                  POOL_REWARD 模式: → UnallocatedPool / UnallocatedTokenPool
                          ↓
              credit_commission() / credit_token_commission() → 记账
                          ↓
              withdraw_commission() / withdraw_token_commission() → 提现 + 分级复购
```

## 平台费分配规则

### NEX 平台费

每笔订单的平台费按**全局固定比例**分配，不受 Entity 配置影响：

| 场景 | 招商人（Referrer） | 国库（Treasury） |
|------|---------------------|------------------|
| 有招商人 | `platform_fee × ReferrerShareBps / 10000` | 剩余部分 |
| 无招商人 | 0 | `platform_fee` (100%) |
| 未配置佣金 | 0 | `platform_fee` (100%) |

- **`ReferrerShareBps`** 为全局常量（runtime 配置 5000 = 50%），不可按 Entity 修改
- 平台费分配在佣金配置检查**之前**执行，确保平台收入不受佣金配置影响
- 取消时：国库部分通过 `OrderTreasuryTransfer` 退回平台账户

### Token 平台费

- Token 平台费率由 `TokenPlatformFeeRate` 全局存储控制（默认 100 bps = 1%）
- 有招商人时：`token_platform_fee × ReferrerShareBps / 10000` → 招商人 Token 佣金
- 剩余部分（Pool A 留存）计入 `UnallocatedTokenPool`，通过 `OrderTokenPlatformRetention` 记录
- 取消时：Pool A 留存从 `UnallocatedTokenPool` 中扣回（M2-R6 审计修复）

## Config

| 类型 | 说明 |
|------|------|
| `Currency` | NEX 货币 trait |
| `ShopProvider` | 店铺查询接口 |
| `EntityProvider` | Entity 查询接口 |
| `GovernanceProvider` | 治理查询接口（R8: locked+None 单调递减豁免） |
| `MemberProvider` | 会员查询接口 |
| `EntityReferrerProvider` | 招商推荐人查询 |
| `ReferralPlugin` | NEX 推荐链返佣插件 |
| `MultiLevelPlugin` | NEX 多级分销返佣插件 |
| `LevelDiffPlugin` | NEX 等级极差返佣插件 |
| `SingleLinePlugin` | NEX 单线收益插件 |
| `TeamPlugin` | NEX 团队业绩插件 |
| `TokenReferralPlugin` | Token 推荐链插件 |
| `TokenMultiLevelPlugin` | Token 多级分销插件 |
| `TokenLevelDiffPlugin` | Token 等级极差插件 |
| `TokenSingleLinePlugin` | Token 单线收益插件 |
| `TokenTeamPlugin` | Token 团队业绩插件 |
| `ReferralWriter` | 推荐链方案写入器（Governance） |
| `MultiLevelWriter` | 多级分销方案写入器 |
| `LevelDiffWriter` | 等级极差方案写入器 |
| `TeamWriter` | 团队业绩方案写入器 |
| `PoolRewardWriter` | 沉淀池奖励方案写入器 |
| `PlatformAccount` | 平台账户 |
| `TreasuryAccount` | 国库账户 |
| `TokenBalance` | Entity Token 余额类型 |
| `TokenTransferProvider` | Token 转账接口 |
| `ParticipationGuard` | KYC/合规参与权守卫（默认 `()` 允许全部） |

### 常量

| 常量 | 说明 |
|------|------|
| `ReferrerShareBps` | 招商推荐人分佣比例（基点，5000 = 50%） |
| `MaxCommissionRecordsPerOrder` | 每订单最大返佣记录数 |
| `MaxCustomLevels` | LevelBased 最大自定义等级数 |
| `PoolRewardWithdrawCooldown` | POOL_REWARD 关闭后冷却期（区块数） |
| `MaxWithdrawalRecords` | F6: 每会员提现记录上限 |
| `MaxMemberOrderIds` | F7: 每会员佣金关联订单 ID 上限 |

## Storage

### NEX 存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `CommissionConfigs` | `Map<u64, CoreCommissionConfig>` | Entity 返佣核心配置（OptionQuery） |
| `MemberCommissionStats` | `DoubleMap<u64, AccountId, MemberCommissionStatsData>` | 会员返佣统计 |
| `OrderCommissionRecords` | `Map<u64, BoundedVec<CommissionRecord>>` | 订单返佣记录 |
| `ShopCommissionTotals` | `Map<u64, (Balance, u64)>` | Entity 返佣统计 (total, orders) |
| `ShopPendingTotal` | `Map<u64, Balance>` | Entity 待提取佣金总额 |
| `ShopShoppingTotal` | `Map<u64, Balance>` | Entity 购物余额总额（资金锁定） |
| `WithdrawalConfigs` | `Map<u64, EntityWithdrawalConfig>` | NEX 提现配置（OptionQuery） |
| `MemberShoppingBalance` | `DoubleMap<u64, AccountId, Balance>` | 会员购物余额 |
| `MemberLastCredited` | `DoubleMap<u64, AccountId, BlockNumber>` | NEX 最后入账区块（冻结期检查） |
| `GlobalMinRepurchaseRate` | `Map<u64, u16>` | Governance 全局最低复购比例 |
| `OrderTreasuryTransfer` | `Map<u64, Balance>` | 订单平台费转国库金额（取消退款用） |
| `UnallocatedPool` | `Map<u64, Balance>` | NEX 未分配沉淀资金池 |
| `OrderUnallocated` | `Map<u64, (u64, u64, Balance)>` | 订单沉淀记录 (entity_id, shop_id, amount) |
| `MemberCommissionOrderIds` | `DoubleMap<u64, AccountId, BoundedVec<u64>>` | F19: 会员佣金关联订单 ID |
| `MemberWithdrawalHistory` | `DoubleMap<u64, AccountId, BoundedVec<WithdrawalRecord>>` | F20: 会员提现历史 |

### Token 存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `MemberTokenCommissionStats` | `DoubleMap<u64, AccountId, MemberTokenCommissionStatsData>` | Token 佣金统计 |
| `OrderTokenCommissionRecords` | `Map<u64, BoundedVec<TokenCommissionRecord>>` | Token 订单佣金记录 |
| `TokenPendingTotal` | `Map<u64, TokenBalance>` | Token 待提取佣金总额 |
| `UnallocatedTokenPool` | `Map<u64, TokenBalance>` | Token 未分配沉淀池 |
| `OrderTokenUnallocated` | `Map<u64, (u64, u64, TokenBalance)>` | Token 订单沉淀记录 |
| `OrderTokenPlatformRetention` | `Map<u64, (u64, TokenBalance)>` | M2-R6: 订单 Pool A 留存（cancel 回退用） |
| `TokenPlatformFeeRate` | `StorageValue<u16>` | Token 平台费率（默认 100 bps = 1%） |
| `MemberTokenShoppingBalance` | `DoubleMap<u64, AccountId, TokenBalance>` | Token 购物余额 |
| `TokenShoppingTotal` | `Map<u64, TokenBalance>` | Token 购物余额总额（资金锁定） |
| `TokenWithdrawalConfigs` | `Map<u64, EntityWithdrawalConfig>` | Token 提现配置（OptionQuery） |
| `MemberTokenLastCredited` | `DoubleMap<u64, AccountId, BlockNumber>` | Token 最后入账区块（独立冻结期） |
| `GlobalMinTokenRepurchaseRate` | `Map<u64, u16>` | Token Governance 全局最低复购比例 |
| `EntityTokenAccountedBalance` | `Map<u64, TokenBalance>` | Entity Token 已知渠道余额（sweep 检测） |
| `MemberTokenCommissionOrderIds` | `DoubleMap<u64, AccountId, BoundedVec<u64>>` | F19: Token 佣金关联订单 ID |
| `MemberTokenWithdrawalHistory` | `DoubleMap<u64, AccountId, BoundedVec<WithdrawalRecord>>` | F20: Token 提现历史 |

### 治理/全局存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `PoolRewardDisabledAt` | `Map<u64, BlockNumber>` | POOL_REWARD 关闭时间戳（cooldown 计算，OptionQuery） |
| `GlobalMaxCommissionRate` | `Map<u64, u16>` | F15: 全局佣金率上限（0=无限制） |
| `GlobalMaxTokenCommissionRate` | `Map<u64, u16>` | F16: 全局 Token 佣金率上限 |
| `GlobalCommissionPaused` | `StorageValue<bool>` | F17: 全局佣金紧急暂停开关 |
| `WithdrawalPaused` | `Map<u64, bool>` | F18: Entity 级提现暂停开关 |

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

### WithdrawalRecord

```rust
pub struct WithdrawalRecord<Balance, BlockNumber> {
    pub total_amount: Balance,   // 提现总额（withdrawal + repurchase + bonus）
    pub withdrawn: Balance,      // 到手金额
    pub repurchased: Balance,    // 复购金额
    pub bonus: Balance,          // 自愿多复购奖励
    pub block_number: BlockNumber,
}
```

## Extrinsics

| call_index | 方法 | 权限 | 说明 |
|------------|------|------|------|
| 0 | `set_commission_modes` | Owner/Admin | 设置启用的返佣模式位标志；跟踪 POOL_REWARD 开关 |
| 1 | `set_commission_rate` | Owner/Admin | 设置会员返佣上限（≤10000，受 F15 全局上限约束） |
| 2 | `enable_commission` | Owner/Admin | 启用/禁用返佣；跟踪 POOL_REWARD 实际激活状态 |
| 3 | `withdraw_commission` | 会员 | 提取 NEX 返佣（四种提现模式 + 指定复购目标） |
| 4 | `set_withdrawal_config` | Owner/Admin | 设置 NEX 提现配置（含 level_id 唯一性校验） |
| 5 | `use_shopping_balance` | ~~会员~~ **已禁用** | → `ShoppingBalanceWithdrawalDisabled` |
| 6 | `init_commission_plan` | ~~Owner~~ **已禁用** | → `CommissionPlanDisabled` |
| 8 | `withdraw_token_commission` | 会员 | 提取 Token 佣金（F3: 独立 token_withdrawal_cooldown） |
| 10 | `set_token_withdrawal_config` | Owner/Admin | 设置 Token 提现配置（独立存储） |
| 11 | `set_global_min_token_repurchase_rate` | Root | Token Governance 全局最低复购比例 |
| 12 | `withdraw_entity_funds` | Entity Owner | 提取 Entity NEX 自由余额（保留 Pending+Shopping+Pool） |
| 13 | `withdraw_entity_token_funds` | Entity Owner | 提取 Entity Token 自由余额（保留 TokenPending+Shopping+Pool） |
| 14 | `set_creator_reward_rate` | Owner/Admin | 设置创建人收益比例（基点，上限 5000）；R8: None+Locked 时仅允许降低 |
| 15 | `set_token_platform_fee_rate` | Root | 设置 Token 平台费率（基点，上限 1000 = 10%） |
| 16 | `set_global_min_repurchase_rate` | Root | F13: NEX 全局最低复购比例 |
| 17 | `set_withdrawal_cooldown` | Owner/Admin | F2: NEX/Token 独立提现冻结期 |
| 18 | `force_disable_entity_commission` | Root | F14: 紧急禁用 Entity 佣金 |
| 19 | `set_global_max_commission_rate` | Root | F15: 全局佣金率上限（0=无限制） |
| 20 | `clear_commission_config` | Owner/Admin | F4: 清除佣金配置 |
| 21 | `clear_withdrawal_config` | Owner/Admin | F4: 清除 NEX 提现配置 |
| 22 | `clear_token_withdrawal_config` | Owner/Admin | F4: 清除 Token 提现配置 |
| 23 | `set_global_max_token_commission_rate` | Root | F16: 全局 Token 佣金率上限（0=无限制） |
| 24 | `force_global_pause` | Root | F17: 全局佣金紧急暂停/恢复 |
| 25 | `pause_withdrawals` | Owner/Admin | F18: Entity 级提现暂停/恢复 |
| 26 | `archive_order_records` | Owner/Admin | F21: 归档已完结订单佣金记录（释放存储） |

> **F1 权限模型**: call_index 0,1,2,4,10,14,17,20-22,25 支持 Owner 或 Admin（需 `COMMISSION_MANAGE` 权限位）。call_index 12,13（资金提取）仅限 Owner。call_index 11,15,16,18,19,23,24 仅限 Root。

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

### NEX 管线

| 函数 | 说明 |
|------|------|
| `process_commission` | 调度引擎：双来源架构处理订单 NEX 返佣（平台费 + 卖家货款） |
| `credit_commission` | 记录并发放 NEX 返佣（Records/Stats/PendingTotal/LastCredited/OrderIds） |
| `cancel_commission` | 取消订单 NEX 返佣（先转账后更新记录；含 Token 取消） |
| `calc_withdrawal_split` | 计算 NEX 提现/复购/奖励分配（三层约束模型） |
| `do_use_shopping_balance` | NEX 购物余额纯记账（供 CommissionProvider 调用） |
| `do_consume_shopping_balance` | 消费 NEX 购物余额（记账 + 转账，含 ParticipationGuard 检查） |

### Token 管线

| 函数 | 说明 |
|------|------|
| `process_token_commission` | Token 调度引擎：双源架构处理 Token 返佣（含 sweep + 可用额度检查） |
| `credit_token_commission` | Token 佣金纯记账（不转账，Token 托管在 entity_account） |
| `do_cancel_token_commission` | 取消 Token 佣金（退还 Pool B 沉淀 + Pool A 留存） |
| `calc_token_withdrawal_split` | 计算 Token 提现/复购/奖励分配（与 NEX 对称） |
| `do_consume_token_shopping_balance` | 消费 Token 购物余额（记账 + Token 转账） |
| `sweep_token_free_balance` | 检测 entity_account 外部 Token 转入，更新 EntityTokenAccountedBalance |

### 辅助函数

| 函数 | 说明 |
|------|------|
| `ensure_owner_or_admin` | F1: 验证 Entity Owner 或 Admin(COMMISSION_MANAGE) 权限 |
| `ensure_entity_owner` | 验证 Entity Owner（仅 Owner，用于资金提取） |
| `is_pool_reward_locked` | 判断沉淀池是否因 POOL_REWARD 状态或 cooldown 而锁定 |

## 资金流向

### NEX 资金流

```
  池 A（平台费 → 国库 + 招商人）：
    平台账户 --transfer--> 国库（treasury_portion）
    平台账户 --transfer--> Entity 账户（referrer_quota → credit_commission）
    取消: Entity 账户 → 平台账户 | 国库 → 平台账户

  池 B（卖家货款 → 会员返佣 + 沉淀池）：
    seller --transfer--> Entity 账户（插件分配 + POOL_REWARD 剩余）
    取消: Entity 账户 → seller（仅转账成功的记录标记取消）

  提现：
    Entity 账户 --transfer(KeepAlive)--> 会员钱包（提现部分）
    复购 + 奖励 → MemberShoppingBalance（记账，资金留在 Entity）
```

### Token 资金流

```
  池 A（Token 平台费 → 招商人 + 留存）：
    token_platform_fee sweep → EntityTokenAccountedBalance
    referrer_quota → credit_token_commission（纯记账）
    pool_a_retention → UnallocatedTokenPool（记账）
    取消: pool_a_retention 从 UnallocatedTokenPool 扣回

  池 B（Entity Token → 会员返佣 + 沉淀池）：
    entity_token_balance - committed → 可用额度 → 插件分配（纯记账）
    剩余 → UnallocatedTokenPool（POOL_REWARD 模式）
    取消: token_transfer Entity → seller

  提现：
    Entity 账户 --token_transfer--> 会员钱包（提现部分）
    复购 + 奖励 → MemberTokenShoppingBalance（记账）
```

## Events

### NEX 事件

| 事件 | 说明 |
|------|------|
| `CommissionConfigUpdated { entity_id }` | 核心配置更新 |
| `CommissionModesUpdated { entity_id, modes }` | 返佣模式更新 |
| `CommissionDistributed { entity_id, order_id, beneficiary, amount, commission_type, level }` | 返佣发放 |
| `CommissionWithdrawn { entity_id, account, amount }` | 返佣提取 |
| `CommissionCancelled { order_id, refund_succeeded, refund_failed }` | 返佣取消（含成功/失败计数） |
| `CommissionPlanRemoved { entity_id }` | [占位] init_commission_plan 已移除 |
| `TieredWithdrawal { entity_id, account, repurchase_target, withdrawn_amount, repurchase_amount, bonus_amount }` | 分级提现（含复购目标） |
| `WithdrawalConfigUpdated { entity_id }` | NEX 提现配置更新 |
| `ShoppingBalanceUsed { entity_id, account, amount }` | 购物余额使用 |
| `CommissionFundsTransferred { entity_id, shop_id, amount }` | 佣金资金转入 Entity |
| `PlatformFeeToTreasury { order_id, amount }` | 平台费转入国库 |
| `TreasuryRefund { order_id, amount }` | 国库退款（订单取消） |
| `CommissionRefundFailed { entity_id, shop_id, amount }` | 退款失败（需人工干预） |
| `UnallocatedCommissionPooled { entity_id, order_id, amount }` | 未分配佣金转入沉淀池 |
| `PoolRewardDistributed { entity_id, order_id, total_distributed }` | 沉淀池奖励发放 |
| `UnallocatedPoolRefunded { entity_id, order_id, amount }` | 沉淀池退还卖家（取消） |
| `EntityFundsWithdrawn { entity_id, to, amount }` | Owner 提取 Entity NEX 余额 |
| `WithdrawalCooldownNotMet { entity_id, account, earliest_block }` | 提现冻结期未满 |

### Token 事件

| 事件 | 说明 |
|------|------|
| `TokenCommissionDistributed { entity_id, order_id, beneficiary, amount, commission_type, level }` | Token 佣金分发 |
| `TokenCommissionWithdrawn { entity_id, account, amount }` | Token 佣金提现 |
| `TokenCommissionCancelled { order_id, cancelled_count }` | Token 佣金取消 |
| `TokenTieredWithdrawal { entity_id, account, repurchase_target, withdrawn_amount, repurchase_amount, bonus_amount }` | Token 分层提现 |
| `TokenWithdrawalConfigUpdated { entity_id }` | Token 提现配置更新 |
| `TokenShoppingBalanceUsed { entity_id, account, amount }` | Token 购物余额使用 |
| `TokenUnallocatedPooled { entity_id, order_id, amount }` | Token 沉淀池入账 |
| `TokenUnallocatedPoolRefunded { entity_id, order_id, amount }` | Token 沉淀池退还 |
| `EntityTokenFundsWithdrawn { entity_id, to, amount }` | Owner 提取 Entity Token 余额 |

### 治理事件

| 事件 | 说明 |
|------|------|
| `GlobalMinRepurchaseRateSet { entity_id, rate }` | NEX Governance 最低复购比例变更 |
| `GlobalMinTokenRepurchaseRateSet { entity_id, rate }` | Token Governance 最低复购比例变更 |
| `TokenPlatformFeeRateUpdated { old_rate, new_rate }` | Token 平台费率变更 |
| `CommissionForceDisabled { entity_id }` | F14: Root 紧急禁用 Entity 佣金 |
| `GlobalMaxCommissionRateSet { entity_id, rate }` | F15: 全局佣金率上限变更 |
| `GlobalMaxTokenCommissionRateSet { entity_id, rate }` | F16: 全局 Token 佣金率上限变更 |
| `WithdrawalCooldownUpdated { entity_id, nex_cooldown, token_cooldown }` | F2: 提现冻结期变更 |
| `CommissionConfigCleared { entity_id }` | F4: 佣金配置已清除 |
| `WithdrawalConfigCleared { entity_id }` | F4: NEX 提现配置已清除 |
| `TokenWithdrawalConfigCleared { entity_id }` | F4: Token 提现配置已清除 |
| `GlobalCommissionPauseToggled { paused }` | F17: 全局暂停/恢复 |
| `WithdrawalPauseToggled { entity_id, paused }` | F18: Entity 级提现暂停/恢复 |
| `OrderRecordsArchived { order_id }` | F21: 订单记录已归档 |

## Errors

| 错误 | 说明 |
|------|------|
| `ShopNotFound` | 店铺不存在 |
| `EntityNotFound` | 实体不存在 |
| `NotShopOwner` | 不是店主 |
| `NotEntityOwner` | 不是实体所有者 |
| `CommissionNotConfigured` | 返佣未配置 |
| `InsufficientCommission` | NEX 返佣 pending 余额不足 |
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
| `TargetNotApprovedMember` | 复购目标未通过审批（APPROVAL_REQUIRED 策略下） |
| `MemberNotActivated` | ~~已废弃~~ 保留错误码供兼容 |
| `TargetParticipationDenied` | 复购目标不满足 Entity 参与要求（如 mandatory KYC） |
| `ParticipationRequirementNotMet` | 账户不满足 Entity 参与要求，无法消费购物余额 |
| `ShoppingBalanceWithdrawalDisabled` | 购物余额仅可用于购物，不可直接提取为 NEX |
| `InsufficientUnallocatedPool` | 沉淀资金池余额不足 |
| `InsufficientTokenCommission` | Token 佣金 pending 余额不足 |
| `TokenTransferFailed` | Token 转账失败 |
| `InsufficientEntityFunds` | Entity 账户 NEX 偿付能力不足 |
| `InsufficientEntityTokenFunds` | Entity 账户 Token 偿付能力不足 |
| `PoolRewardCooldownActive` | POOL_REWARD 关闭后冷却期未满 |
| `CommissionPlanDisabled` | init_commission_plan 已禁用 |
| `TokenPlatformFeeRateTooHigh` | Token 平台费率超过上限（最大 1000 bps = 10%） |
| `EntityLocked` | 实体已被全局锁定 |
| `LockedOnlyDecreaseAllowed` | R8: 锁定状态下仅允许降低（无币实体单调递减豁免） |
| `NotEntityOwnerOrAdmin` | F1: 既不是 Owner 也不是 COMMISSION_MANAGE Admin |
| `CommissionRateExceedsGlobalMax` | F15: max_commission_rate 超过全局上限 |
| `ConfigNotFound` | F4: 配置不存在，无法清除 |
| `TokenCommissionRateExceedsGlobalMax` | F16: Token max_commission_rate 超过全局 Token 上限 |
| `GlobalCommissionPaused` | F17: 全局佣金紧急暂停中 |
| `WithdrawalPausedByOwner` | F18: Entity 级提现已暂停 |
| `EntityNotActive` | F4+: Entity 未处于活跃状态 |
| `OrderRecordsNotFound` | F21: 订单记录不存在或已归档 |
| `OrderRecordsNotFinalized` | F21: 订单佣金记录中存在未完结记录（Pending），不可归档 |

## Trait 实现

### CommissionProvider

外部接口，供 Governance 和订单模块调用。提供以下方法：

| 方法 | 说明 |
|------|------|
| `process_commission` | 处理订单 NEX 返佣（双来源） |
| `cancel_commission` | 取消订单 NEX 返佣 |
| `pending_commission` | 查询会员待提取 NEX 佣金 |
| `shopping_balance` | 查询会员购物余额 |
| `use_shopping_balance` | 使用购物余额（纯记账） |
| `set_commission_modes` | 设置返佣模式（含 POOL_REWARD 跟踪） |
| `set_direct_reward_rate` | 设置直推奖励比率 |
| `set_level_diff_config` | 设置等级极差配置 |
| `set_fixed_amount` | 设置固定金额奖励 |
| `set_first_order_config` | 设置首单奖励配置 |
| `set_repeat_purchase_config` | 设置复购奖励配置 |
| `set_withdrawal_config_by_governance` | Governance 设置提现配置 |
| `set_min_repurchase_rate` | 设置 Governance 全局最低复购比例 |
| `set_creator_reward_rate` | 设置创建人收益比例（≤5000） |

### TokenCommissionProvider

Token 佣金外部接口：

| 方法 | 说明 |
|------|------|
| `process_token_commission` | 处理订单 Token 返佣 |
| `cancel_token_commission` | 取消订单 Token 返佣 |
| `pending_token_commission` | 查询会员待提取 Token 佣金 |
| `token_platform_fee_rate` | 查询 Token 平台费率 |

### PoolBalanceProvider / TokenPoolBalanceProvider

供 pool-reward v2 访问沉淀池余额：

| 方法 | 说明 |
|------|------|
| `pool_balance` / `token_pool_balance` | 查询沉淀池余额 |
| `deduct_pool` / `deduct_token_pool` | 扣减沉淀池余额（余额不足时返回错误） |

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

| 编号 | 级别 | 修复位置 | 说明 |
|------|------|----------|------|
| C1 | Critical | `withdraw_commission` | 偿付安全检查计入 repurchase + bonus 对 ShopShoppingTotal 的增量 |
| H1 | High | `withdraw_commission` | WithdrawalConfig 未启用时拒绝提现；auto_register 后验证 target 会员状态 |
| H2 | High | `cancel_commission` | 先尝试转账，成功后才取消记录，防止转账失败但记录已标记 Cancelled |
| H3 | High | `withdraw_*` / `do_consume_*` | 引入 `ParticipationGuard` trait 检查 Entity KYC 参与要求 |
| CC-M1 | Medium | `CommissionCancelled` 事件 | 增加 refund_succeeded / refund_failed 计数 |
| M1 | Low | `set_withdrawal_config` | level_overrides 添加 level_id 唯一性校验 |
| M3 | Medium | `TieredWithdrawal` 事件 | 新增 `repurchase_target` 字段 |
| M1-R4 | Medium | `set_global_min_token_repurchase_rate` | 新增 `GlobalMinTokenRepurchaseRateSet` 事件 |
| M2-R4 | Medium | `withdraw_*` | 偿付安全改用 `InsufficientEntityFunds` / `InsufficientEntityTokenFunds` |
| M3-R4 | Medium | `cancel_commission` | 消除 Token 取消重复，复用 `do_cancel_token_commission` |
| M4-R4 | Medium | `CommissionProvider::set_commission_modes` | 使用 `CommissionModes::is_valid()` 替代手动掩码 |
| L5-R4 | Low | `CommissionProvider::set_min_repurchase_rate` | 新增 `GlobalMinRepurchaseRateSet` 事件 |
| H1-R5 | High | `enable_commission` / `force_disable` / `clear_config` | POOL_REWARD 状态跟踪与 cooldown 一致性 |
| M1-R5 | Medium | `archive_order_records` | 验证订单记录 entity_id 防止跨实体越权归档 |
| M2-R5 | Medium | `process_token_commission` | 未配置佣金时先 sweep 再优雅返回 |
| M1-R6 | Medium | `set_commission_modes` / trait impl | 仅当 commission 已启用时才清除 POOL_REWARD cooldown，防止 toggle 绕过 |
| M2-R6 | Medium | `process_token_commission` / `do_cancel_token_commission` | 新增 `OrderTokenPlatformRetention` 存储，cancel 时回退 Pool A 留存 |
| L1-R7 | Low | `Cargo.toml` | 移除未使用的 `sp-std` 依赖（已使用 `extern crate alloc` 替代） |
| L2-R7 | Low | `Cargo.toml` | 移除未使用的 `sp-core` dev-dependency |

### 功能扩展记录

| 编号 | 说明 |
|------|------|
| F1 | Admin 权限支持 — `ensure_owner_or_admin` + `COMMISSION_MANAGE` 权限位 |
| F2 | `set_withdrawal_cooldown` (call_index 17) — NEX/Token 冻结期独立配置 |
| F3 | Token 独立冻结期 — `token_withdrawal_cooldown` 字段，0 = 回退到 `withdrawal_cooldown` |
| F4 | `clear_commission_config` / `clear_withdrawal_config` / `clear_token_withdrawal_config` (call_index 20-22) |
| F6 | `MaxWithdrawalRecords` — 每会员提现记录上限 |
| F7 | `MaxMemberOrderIds` — 每会员佣金关联订单 ID 上限 |
| F13 | `set_global_min_repurchase_rate` (call_index 16) — NEX 全局最低复购比例 |
| F14 | `force_disable_entity_commission` (call_index 18) — Root 紧急禁用 Entity 佣金 |
| F15 | `GlobalMaxCommissionRate` + `set_global_max_commission_rate` (call_index 19) — 治理佣金率上限 |
| F16 | `GlobalMaxTokenCommissionRate` + `set_global_max_token_commission_rate` (call_index 23) — Token 佣金率上限 |
| F17 | `GlobalCommissionPaused` + `force_global_pause` (call_index 24) — 全局紧急暂停 |
| F18 | `WithdrawalPaused` + `pause_withdrawals` (call_index 25) — Entity 级提现暂停 |
| F19 | `MemberCommissionOrderIds` / `MemberTokenCommissionOrderIds` — 佣金关联订单 ID 索引 |
| F20 | `MemberWithdrawalHistory` / `MemberTokenWithdrawalHistory` — 提现历史 |
| F21 | `archive_order_records` (call_index 26) — 归档已完结订单记录释放存储 |
| R8 | `set_creator_reward_rate` — 无币实体（None+Locked）单调递减豁免：锁定后仅允许降低 creator_reward_rate |

## 测试覆盖

190 个测试（`cargo test -p pallet-commission-core`），主要覆盖领域：

- **基础配置**: set_commission_rate / set_commission_modes / enable_commission
- **平台费分配**: referrer_gets_half / dual_source / treasury 50-50 split / full_to_treasury
- **cancel_commission**: 双来源 + 国库退款 + Token 取消
- **提现模式**: FullWithdrawal / FixedRate / LevelBased / MemberChoice
- **三层约束**: Governance 底线 / Entity 设定 / 会员选择
- **KYC/合规 (H3)**: ParticipationGuard 提现阻止 / 购物余额消费阻止
- **POOL_REWARD**: 沉淀池入账 / cooldown 跟踪 / 跨路径一致性
- **Token 管线**: process / credit / cancel / withdraw / shopping balance
- **Admin 权限 (F1)**: Owner/Admin 配置 / 资金提取仅 Owner
- **F2-F4**: 独立冻结期 / 配置清除
- **F13-F15**: 全局最低复购 / Root 紧急禁用 / 全局佣金率上限
- **F16-F18**: Token 佣金率上限 / 全局暂停 / Entity 级暂停
- **F19-F21**: 订单 ID 索引 / 提现历史 / 订单记录归档
- **审计回归**: R4-R6 所有修复项对应的回归测试
- **R8 单调递减**: None+Locked 允许降低 / 禁止增加 / 禁止相同值 / FullDAO+Locked 全部禁止 / 未锁定自由设置

## 依赖

```toml
[dependencies]
pallet-entity-common = { path = "../../common" }
pallet-commission-common = { path = "../common" }
sp-runtime = { workspace = true }
log = { workspace = true }

[dev-dependencies]
pallet-balances = { workspace = true, features = ["std"] }
sp-io = { workspace = true, features = ["std"] }
```
