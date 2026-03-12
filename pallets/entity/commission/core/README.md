# pallet-commission-core

> 返佣系统核心调度引擎 — 双资产（NEX / Token）配置管理、插件调度、记账、提现与偿付安全

---

## 1. 概述

`pallet-commission-core` 是返佣系统的核心调度引擎，管理从订单产生到佣金提现的完整生命周期。

**核心职责：**

- **双来源调度**：每笔订单同时处理 NEX（平台币）和 Entity Token 两条独立管线
- **平台费分配**：招商人 `ReferrerShareBps%` + 国库/留存（无招商人时 100% 进国库）
- **插件化分佣**：5 个 NEX 插件 + 5 个 Token 插件并行计算，剩余沉入资金池
- **创建人收益**：从 Pool B 佣金预算中优先扣除创建人份额
- **提现系统**：四种提现模式 x 三层约束模型 x 自愿复购奖励 x 指定复购目标
- **偿付安全**：提现前验证 Entity 账户可覆盖所有 pending + shopping + pool 承诺
- **治理控制**：全局暂停、Entity 级暂停、全局佣金率上限、全局最低复购比例、Root 紧急操作
- **KYC 守卫**：`ParticipationGuard` trait 在提现和购物余额消费前检查参与权
- **订单归档**：释放已完结订单的全部 per-order 存储

---

## 2. 代码结构

核心引擎代码拆分为四个文件，按职责分治：

```
commission/core/src/
├── lib.rs          ← Config + Storage + Extrinsics + Trait 实现（主框架）
├── engine.rs       ← 佣金计算引擎：调度 + 记账 + 取消 + 结算
├── settlement.rs   ← 购物余额结算：NEX 委托 Loyalty、Token 直接管理
├── withdraw.rs     ← 提现分配计算 + 权限校验 + Token sweep 辅助
├── runtime_api.rs  ← Runtime API 定义
├── weights.rs      ← 基准测试权重
├── benchmarking.rs ← 基准测试骨架
├── mock.rs         ← 测试 mock
└── tests.rs        ← 单元测试
```

### engine.rs — 佣金计算引擎

| 函数 | 说明 |
|------|------|
| `process_commission` | NEX 佣金调度引擎（双来源：池 A 平台费 + 池 B 卖家货款 -> 5 插件分配 -> 沉淀池） |
| `credit_commission` | NEX 佣金记账（Records / Stats / PendingTotal / LastCredited / OrderIds） |
| `process_token_commission` | Token 佣金调度引擎（含 sweep + 可用额度检查） |
| `credit_token_commission` | Token 佣金纯记账（不转账，Token 在 entity_account 托管直到提现） |
| `cancel_commission` | 取消 NEX 佣金（先转账后更新记录；含 Token 取消） |
| `do_cancel_token_commission` | 取消 Token 佣金（退还 Pool B 沉淀 + Pool A 留存） |
| `do_settle_order_records` | 订单完结：Pending -> Withdrawn，释放 PendingTotal |

### settlement.rs — 购物余额结算

NEX 购物余额已完全迁移至 Loyalty 模块，本文件通过 `T::Loyalty`（`LoyaltyWritePort`）委托操作。Token 购物余额仍由 commission-core 直接管理。

| 函数 | 说明 |
|------|------|
| `do_use_shopping_balance` | 委托 `T::Loyalty::consume_shopping_balance`（纯记账，已废弃，仅保留 trait 兼容） |
| `do_consume_shopping_balance` | 委托 `T::Loyalty::consume_shopping_balance`（记账 + NEX 转账，含 KYC 检查） |
| `do_consume_token_shopping_balance` | Token 购物余额消费（扣减记账 + Token 从 Entity 转入会员钱包，含 ParticipationGuard 检查） |

**NEX vs Token 购物余额归属：**

| 资产 | 存储位置 | 操作方式 |
|------|---------|---------|
| NEX `MemberShoppingBalance` / `ShopShoppingTotal` | **Loyalty 模块** | 通过 `T::Loyalty`（`LoyaltyWritePort`）委托 |
| Token `MemberTokenShoppingBalance` / `TokenShoppingTotal` | **commission-core** | 直接读写本地 Storage |

### withdraw.rs — 提现计算与辅助

| 函数 | 说明 |
|------|------|
| `calc_withdrawal_split` | NEX 提现/复购/奖励分配计算（三层约束模型） |
| `calc_token_withdrawal_split` | Token 提现分配计算（与 NEX 对称，独立配置） |
| `is_pool_reward_locked` | 沉淀池锁定判断（POOL_REWARD 开启或 cooldown 未满） |
| `ensure_owner_or_admin` | 验证 Entity Owner 或 Admin(COMMISSION_MANAGE) 权限 |
| `ensure_entity_owner` | 验证 Entity Owner（仅 Owner，用于资金提取） |
| `sweep_token_free_balance` | 检测 entity_account 外部 Token 转入，归集到沉淀池 |

---

## 3. 架构

```
订单模块 ──> CommissionProvider::process_commission()          (NEX)
         ──> TokenCommissionProvider::process_token_commission() (Token)
                          |
              +─ 池 A：平台费无条件分配 ──────────────────────+
              |  NEX:  有招商人 -> ReferrerShareBps% 给招商人  |
              |                  剩余 -> 国库                  |
              |        无招商人 -> 100% 国库                   |
              |  Token: 有招商人 -> ReferrerShareBps% 给招商人  |
              |         剩余 -> UnallocatedTokenPool（留存）    |
              +───────────────────────────────────────────────+
                          |
              +─ 池 B：会员返佣（卖家货款 x max_commission_rate）+
              |  0. CreatorReward（创建人收益，优先扣除）         |
              |  1. ReferralPlugin.calculate()                  |
              |  2. MultiLevelPlugin.calculate()                |
              |  3. LevelDiffPlugin.calculate()                 |
              |  4. SingleLinePlugin.calculate()                |
              |  5. TeamPlugin.calculate()                      |
              |      | 剩余                                    |
              |  POOL_REWARD 模式 -> 沉淀池                     |
              +────────────────────────────────────────────────+
                          |
              credit_commission / credit_token_commission -> 记账
                          |
              settle_order_commission -> Pending->Withdrawn（订单完结）
                          |
              withdraw_commission / withdraw_token_commission -> 提现 + 分级复购
                  |                                                    |
                  |  NEX 复购 -> T::Loyalty::credit_shopping_balance   |
                  |  Token 复购 -> MemberTokenShoppingBalance（本地）    |
                          |
              archive_order_records -> 释放存储
```

---

## 4. 平台费分配

### NEX 平台费

每笔订单的平台费按全局固定比例分配，不受 Entity 佣金配置影响：

| 场景 | 招商人（Referrer） | 国库（Treasury） |
|------|---------------------|------------------|
| 有招商人 | `platform_fee x ReferrerShareBps / 10000` | 剩余 |
| 无招商人 | 0 | 100% |

- `ReferrerShareBps` 为全局 runtime 常量（典型值 5000 = 50%），不可按 Entity 修改
- 平台费分配在佣金配置检查之前执行，确保平台收入不受佣金配置影响
- 取消时：国库部分通过 `OrderTreasuryTransfer` 退回平台账户

### Token 平台费

- Token 平台费率由 `TokenPlatformFeeRate` 全局存储控制（默认 100 bps = 1%）
- 有招商人：`token_platform_fee x ReferrerShareBps / 10000` -> 招商人 Token 佣金
- 剩余部分（Pool A 留存）计入 `UnallocatedTokenPool`，通过 `OrderTokenPlatformRetention` 记录
- 取消时：Pool A 留存从 `UnallocatedTokenPool` 中扣回

---

## 5. 提现系统

### 四种提现模式

| 模式 | 行为 |
|------|------|
| `FullWithdrawal` | 不强制复购，Governance 底线仍生效 |
| `FixedRate { repurchase_rate }` | 所有会员统一复购比率 |
| `LevelBased` | 按会员等级查 `level_overrides`，未匹配回退 `default_tier` |
| `MemberChoice { min_repurchase_rate }` | 会员自选比率，不低于 `min_repurchase_rate` |

### 三层约束模型

```
Governance 底线（GlobalMinRepurchaseRate，强制）
    | max()
Entity 模式设定（FullWithdrawal / FixedRate / LevelBased / MemberChoice）
    | max()
会员选择（MemberChoice 模式下的 requested_rate）
    |
最终复购比率（final_repurchase_rate，<= 10000）
```

### 自愿多复购奖励

超出强制最低线的部分 x `voluntary_bonus_rate` 额外计入购物余额：

```
bonus = (repurchase - mandatory_repurchase) x voluntary_bonus_rate / 10000
```

### 指定复购目标

`withdraw_commission` 的 `repurchase_target` 参数：

| 场景 | 行为 |
|------|------|
| `None` | 复购到自己的购物余额 |
| 目标为非会员 | 自动注册（推荐人 = 出资人），注册后验证会员状态 |
| 目标为直推下线 | 允许 |
| 目标为非直推会员 | `NotDirectReferral` 错误 |
| APPROVAL_REQUIRED 策略下 target 待审批 | `TargetNotApprovedMember` 错误 |
| Entity 配置 mandatory KYC 且 target 未通过 | `TargetParticipationDenied` 错误 |

### 偿付安全

提现前验证 Entity 账户有足够资金覆盖所有承诺：

```
entity_balance - withdrawal >= (old_pending - total_amount) + Loyalty::shopping_total(entity_id) + repurchase + bonus
```

注意：NEX 购物余额总量通过 `T::Loyalty::shopping_total()` 从 Loyalty 模块读取，不再由 commission-core 直接存储。

### 购物余额使用规则

购物余额仅可用于购物（通过 `place_order` 下单抵扣），不可直接提取为 NEX：

| 路径 | 行为 | 状态 |
|------|------|------|
| 下单抵扣（`ShoppingBalanceProvider::consume`） | NEX 从 Entity 转入 Escrow | 可用 |
| 直接提现（`use_shopping_balance` extrinsic） | — | 已禁用 |
| 纯记账（`CommissionProvider::use_shopping_balance`） | 委托 T::Loyalty | 可用 |

---

## 6. Config

### 关联类型（33 项）

| 类型 | trait 约束 | 说明 |
|------|-----------|------|
| `Currency` | `Currency<AccountId>` | NEX 货币 |
| `WeightInfo` | `WeightInfo` | 权重信息 |
| `ShopProvider` | `ShopProvider<AccountId>` | 店铺查询 |
| `EntityProvider` | `EntityProvider<AccountId>` | Entity 查询 |
| `GovernanceProvider` | `GovernanceProvider` | 治理查询（R8: locked+None 单调递减豁免） |
| `MemberProvider` | `MemberProvider<AccountId>` | 会员查询 |
| `ReferralPlugin` | `CommissionPlugin` | NEX 推荐链插件 |
| `MultiLevelPlugin` | `CommissionPlugin` | NEX 多级分销插件 |
| `LevelDiffPlugin` | `CommissionPlugin` | NEX 等级极差插件 |
| `SingleLinePlugin` | `CommissionPlugin` | NEX 单线收益插件 |
| `TeamPlugin` | `CommissionPlugin` | NEX 团队业绩插件 |
| `EntityReferrerProvider` | `EntityReferrerProvider<AccountId>` | 招商推荐人查询 |
| `ReferralWriter` | `ReferralPlanWriter` | 推荐链方案写入器 |
| `MultiLevelWriter` | `MultiLevelPlanWriter` | 多级分销方案写入器 |
| `LevelDiffWriter` | `LevelDiffPlanWriter` | 等级极差方案写入器 |
| `TeamWriter` | `TeamPlanWriter` | 团队业绩方案写入器 |
| `PoolRewardWriter` | `PoolRewardPlanWriter` | 沉淀池奖励方案写入器 |
| `PlatformAccount` | `Get<AccountId>` | 平台账户 |
| `TreasuryAccount` | `Get<AccountId>` | 国库账户 |
| `ParticipationGuard` | `ParticipationGuard<AccountId>` | KYC/合规参与权守卫 |
| `TokenBalance` | `FullCodec + AtLeast32BitUnsigned + ...` | Entity Token 余额类型 |
| `TokenReferralPlugin` | `TokenCommissionPlugin` | Token 推荐链插件 |
| `TokenMultiLevelPlugin` | `TokenCommissionPlugin` | Token 多级分销插件 |
| `TokenLevelDiffPlugin` | `TokenCommissionPlugin` | Token 等级极差插件 |
| `TokenSingleLinePlugin` | `TokenCommissionPlugin` | Token 单线收益插件 |
| `TokenTeamPlugin` | `TokenCommissionPlugin` | Token 团队业绩插件 |
| `TokenTransferProvider` | `TokenTransferProvider<AccountId, TokenBalance>` | Token 转账接口 |
| `MultiLevelQuery` | `MultiLevelQueryProvider` | 多级分销查询（供 Runtime API） |
| `TeamQuery` | `TeamQueryProvider` | 团队业绩查询（供 Runtime API） |
| `SingleLineQuery` | `SingleLineQueryProvider` | 单线收益查询（供 Runtime API） |
| `PoolRewardQuery` | `PoolRewardQueryProvider` | 沉淀池奖励查询（供 Runtime API） |
| `ReferralQuery` | `ReferralQueryProvider` | 推荐链返佣查询（供 Runtime API） |
| `Loyalty` | `LoyaltyWritePort<AccountId, Balance>` | **Loyalty 模块接口（NEX 购物余额读写委托）** |

### 常量（6 项）

| 常量 | 类型 | 说明 |
|------|------|------|
| `ReferrerShareBps` | `u16` | 招商推荐人分佣比例（基点，5000 = 50%） |
| `MaxCommissionRecordsPerOrder` | `u32` | 每订单最大返佣记录数 |
| `MaxCustomLevels` | `u32` | LevelBased 最大自定义等级数 |
| `PoolRewardWithdrawCooldown` | `BlockNumber` | POOL_REWARD 关闭后冷却期（区块数） |
| `MaxWithdrawalRecords` | `u32` | 每会员提现记录上限 |
| `MaxMemberOrderIds` | `u32` | 每会员佣金关联订单 ID 上限 |

---

## 7. 核心结构体

### CoreCommissionConfig

```rust
pub struct CoreCommissionConfig {
    pub enabled_modes: CommissionModes,  // 启用的返佣模式位标志
    pub max_commission_rate: u16,        // 会员返佣上限（基点）
    pub enabled: bool,                   // 全局启用开关
    pub withdrawal_cooldown: u32,        // NEX 提现冻结期（区块数，0 = 无冻结）
    pub creator_reward_rate: u16,        // 创建人收益比例（基点，上限 5000）
    pub token_withdrawal_cooldown: u32,  // Token 提现冻结期（0 = 回退到 withdrawal_cooldown）
}
```

### EntityWithdrawalConfig

```rust
pub struct EntityWithdrawalConfig<MaxLevels: Get<u32>> {
    pub mode: WithdrawalMode,
    pub default_tier: WithdrawalTierConfig,           // LevelBased 默认层
    pub level_overrides: BoundedVec<(u8, WithdrawalTierConfig), MaxLevels>,
    pub voluntary_bonus_rate: u16,                    // 自愿多复购奖励（万分比）
    pub enabled: bool,
}
```

`WithdrawalTierConfig` 包含 `withdrawal_rate` + `repurchase_rate`，两者之和 = 10000。

### CommissionRecord

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
    pub status: CommissionStatus,   // Pending -> Withdrawn -> (archive)
    pub created_at: BlockNumber,
}
```

### CommissionStatus

```rust
pub enum CommissionStatus {
    Pending,      // 初始状态
    Distributed,  // [已废弃] 保留 SCALE 编码索引
    Withdrawn,    // 订单完结，佣金已结算
    Cancelled,    // 订单取消
}
```

### CommissionType

```rust
pub enum CommissionType {
    DirectReferral, MultiLevel, TeamPerformance, LevelDiff,
    FixedAmount, FirstOrder, RepeatPurchase, SingleLineUpline,
    SingleLineDownline, EntityReferral, PoolReward, CreatorReward,
}
```

### WithdrawalRecord

```rust
pub struct WithdrawalRecord<Balance, BlockNumber> {
    pub total_amount: Balance,    // 提现总额
    pub withdrawn: Balance,       // 到手金额
    pub repurchased: Balance,     // 复购金额
    pub bonus: Balance,           // 自愿多复购奖励
    pub block_number: BlockNumber,
}
```

### MemberCommissionStatsData / MemberTokenCommissionStatsData

```rust
pub struct MemberCommissionStatsData<Balance> {
    pub total_earned: Balance,
    pub pending: Balance,
    pub withdrawn: Balance,
    pub repurchased: Balance,
    pub order_count: u32,
}
```

---

## 8. Storage（37 项）

> NEX 购物余额存储（`MemberShoppingBalance`、`ShopShoppingTotal`）已迁移至 Loyalty 模块，commission-core 通过 `T::Loyalty` Port 读写。

### NEX 存储（11 项）

| 存储项 | 类型 | Key | Value | Query |
|--------|------|-----|-------|-------|
| `CommissionConfigs` | `StorageMap` | `entity_id` | `CoreCommissionConfig` | OptionQuery |
| `MemberCommissionStats` | `StorageDoubleMap` | `entity_id, AccountId` | `MemberCommissionStatsData` | ValueQuery |
| `OrderCommissionRecords` | `StorageMap` | `order_id` | `BoundedVec<CommissionRecord>` | ValueQuery |
| `ShopCommissionTotals` | `StorageMap` | `entity_id` | `(Balance, u64)` | ValueQuery |
| `ShopPendingTotal` | `StorageMap` | `entity_id` | `Balance` | ValueQuery |
| `WithdrawalConfigs` | `StorageMap` | `entity_id` | `EntityWithdrawalConfig` | OptionQuery |
| `MemberLastCredited` | `StorageDoubleMap` | `entity_id, AccountId` | `BlockNumber` | ValueQuery |
| `GlobalMinRepurchaseRate` | `StorageMap` | `entity_id` | `u16` | ValueQuery |
| `OrderTreasuryTransfer` | `StorageMap` | `order_id` | `Balance` | ValueQuery |
| `UnallocatedPool` | `StorageMap` | `entity_id` | `Balance` | ValueQuery |
| `OrderUnallocated` | `StorageMap` | `order_id` | `(u64, u64, Balance)` | ValueQuery |

### Token 存储（13 项）

| 存储项 | 类型 | Key | Value | Query |
|--------|------|-----|-------|-------|
| `MemberTokenCommissionStats` | `StorageDoubleMap` | `entity_id, AccountId` | `MemberTokenCommissionStatsData` | ValueQuery |
| `OrderTokenCommissionRecords` | `StorageMap` | `order_id` | `BoundedVec<TokenCommissionRecord>` | ValueQuery |
| `TokenPendingTotal` | `StorageMap` | `entity_id` | `TokenBalance` | ValueQuery |
| `UnallocatedTokenPool` | `StorageMap` | `entity_id` | `TokenBalance` | ValueQuery |
| `OrderTokenUnallocated` | `StorageMap` | `order_id` | `(u64, u64, TokenBalance)` | ValueQuery |
| `OrderTokenPlatformRetention` | `StorageMap` | `order_id` | `(u64, TokenBalance)` | ValueQuery |
| `TokenPlatformFeeRate` | `StorageValue` | -- | `u16` | ValueQuery (默认 100) |
| `MemberTokenShoppingBalance` | `StorageDoubleMap` | `entity_id, AccountId` | `TokenBalance` | ValueQuery |
| `TokenShoppingTotal` | `StorageMap` | `entity_id` | `TokenBalance` | ValueQuery |
| `TokenWithdrawalConfigs` | `StorageMap` | `entity_id` | `EntityWithdrawalConfig` | OptionQuery |
| `MemberTokenLastCredited` | `StorageDoubleMap` | `entity_id, AccountId` | `BlockNumber` | ValueQuery |
| `GlobalMinTokenRepurchaseRate` | `StorageMap` | `entity_id` | `u16` | ValueQuery |
| `EntityTokenAccountedBalance` | `StorageMap` | `entity_id` | `TokenBalance` | OptionQuery |

### 治理 / 控制存储（13 项）

| 存储项 | 类型 | Key | Value | Query |
|--------|------|-----|-------|-------|
| `ReferrerEarnedByBuyer` | `StorageNMap` | `(entity_id, referrer, buyer)` | `Balance` | ValueQuery |
| `PoolRewardDisabledAt` | `StorageMap` | `entity_id` | `BlockNumber` | OptionQuery |
| `GlobalMaxCommissionRate` | `StorageMap` | `entity_id` | `u16` | ValueQuery |
| `GlobalMaxTokenCommissionRate` | `StorageMap` | `entity_id` | `u16` | ValueQuery |
| `GlobalCommissionPaused` | `StorageValue` | -- | `bool` | ValueQuery |
| `WithdrawalPaused` | `StorageMap` | `entity_id` | `bool` | ValueQuery |
| `MinWithdrawalInterval` | `StorageMap` | `entity_id` | `u32` | ValueQuery |
| `MemberCommissionOrderIds` | `StorageDoubleMap` | `entity_id, AccountId` | `BoundedVec<u64>` | ValueQuery |
| `MemberTokenCommissionOrderIds` | `StorageDoubleMap` | `entity_id, AccountId` | `BoundedVec<u64>` | ValueQuery |
| `MemberWithdrawalHistory` | `StorageDoubleMap` | `entity_id, AccountId` | `BoundedVec<WithdrawalRecord>` | ValueQuery |
| `MemberTokenWithdrawalHistory` | `StorageDoubleMap` | `entity_id, AccountId` | `BoundedVec<WithdrawalRecord>` | ValueQuery |
| `MemberLastWithdrawn` | `StorageDoubleMap` | `entity_id, AccountId` | `BlockNumber` | ValueQuery |
| `MemberTokenLastWithdrawn` | `StorageDoubleMap` | `entity_id, AccountId` | `BlockNumber` | ValueQuery |

---

## 9. Extrinsics（28 项）

### 配置管理

| idx | 方法 | 权限 | 说明 |
|-----|------|------|------|
| 0 | `set_commission_modes` | Owner/Admin | 设置返佣模式位标志；跟踪 POOL_REWARD 开关 |
| 1 | `set_commission_rate` | Owner/Admin | 设置 `max_commission_rate`（<=10000，受全局 NEX + Token 上限约束） |
| 2 | `enable_commission` | Owner/Admin | 启用/禁用返佣；跟踪 POOL_REWARD 实际激活状态 |
| 14 | `set_creator_reward_rate` | Owner/Admin | 设置创建人收益比例（上限 5000 bps）；R8: None+Locked 仅允许降低 |
| 17 | `set_withdrawal_cooldown` | Owner/Admin | 设置 NEX/Token 独立提现冻结期 |
| 20 | `clear_commission_config` | Owner/Admin | 清除佣金配置 |
| 29 | `set_min_withdrawal_interval` | Owner/Admin | 设置最小提现间隔（区块数，基于上次提现时间） |

### 提现配置

| idx | 方法 | 权限 | 说明 |
|-----|------|------|------|
| 4 | `set_withdrawal_config` | Owner/Admin | 设置 NEX 提现配置（含 level_id 唯一性校验） |
| 10 | `set_token_withdrawal_config` | Owner/Admin | 设置 Token 提现配置（独立存储） |
| 21 | `clear_withdrawal_config` | Owner/Admin | 清除 NEX 提现配置 |
| 22 | `clear_token_withdrawal_config` | Owner/Admin | 清除 Token 提现配置 |
| 25 | `pause_withdrawals` | Owner/Admin | Entity 级提现暂停/恢复 |

### 会员操作

| idx | 方法 | 权限 | 说明 |
|-----|------|------|------|
| 3 | `withdraw_commission` | 会员 | 提取 NEX 返佣（四种模式 + 指定复购目标 + 偿付安全检查） |
| 8 | `withdraw_token_commission` | 会员 | 提取 Token 佣金（独立 `token_withdrawal_cooldown`） |
| 5 | `use_shopping_balance` | -- | **[已禁用]** -> `ShoppingBalanceWithdrawalDisabled` |
| 6 | `init_commission_plan` | -- | **[已禁用]** -> `CommissionPlanDisabled` |

### 资金管理

| idx | 方法 | 权限 | 说明 |
|-----|------|------|------|
| 12 | `withdraw_entity_funds` | Owner | 提取 Entity NEX 自由余额（保留 Pending+Shopping+Pool） |
| 13 | `withdraw_entity_token_funds` | Owner | 提取 Entity Token 自由余额 |
| 26 | `archive_order_records` | Owner/Admin | 归档已完结订单佣金记录（释放 per-order 存储） |
| 28 | `retry_cancel_commission` | Root | 重试失败的订单退款（cancel_commission 幂等重放） |

### Root 治理

| idx | 方法 | 权限 | 说明 |
|-----|------|------|------|
| 11 | `set_global_min_token_repurchase_rate` | Root | Token 全局最低复购比例 |
| 15 | `set_token_platform_fee_rate` | Root | Token 平台费率（上限 1000 bps = 10%） |
| 16 | `set_global_min_repurchase_rate` | Root | NEX 全局最低复购比例 |
| 18 | `force_disable_entity_commission` | Root | 紧急禁用 Entity 佣金 |
| 19 | `set_global_max_commission_rate` | Root | 全局 NEX 佣金率上限（0 = 无限制） |
| 23 | `set_global_max_token_commission_rate` | Root | 全局 Token 佣金率上限（0 = 无限制） |
| 24 | `force_global_pause` | Root | 全局佣金紧急暂停/恢复 |
| 27 | `force_enable_entity_commission` | Root | 重新启用 Entity 佣金（与 force_disable 对称） |

> **权限模型**：idx 0,1,2,4,10,14,17,20-22,25,26,29 支持 Owner 或 Admin（需 `COMMISSION_MANAGE` 权限位）。idx 12,13（资金提取）仅限 Owner。idx 11,15,16,18,19,23,24,27,28 仅限 Root。

---

## 10. Events（43 项）

### NEX 事件

| 事件 | 字段 |
|------|------|
| `CommissionConfigUpdated` | `entity_id` |
| `CommissionModesUpdated` | `entity_id, modes` |
| `CommissionDistributed` | `entity_id, order_id, beneficiary, amount, commission_type, level` |
| `CommissionWithdrawn` | `entity_id, account, amount` -- **[已废弃]** |
| `CommissionCancelled` | `order_id, refund_succeeded, refund_failed` |
| `CommissionPlanRemoved` | `entity_id` -- **[已废弃]** |
| `WithdrawalCooldownNotMet` | `entity_id, account, earliest_block` |
| `TieredWithdrawal` | `entity_id, account, repurchase_target, withdrawn_amount, repurchase_amount, bonus_amount` |
| `WithdrawalConfigUpdated` | `entity_id` |
| `ShoppingBalanceUsed` | `entity_id, account, amount` |
| `CommissionFundsTransferred` | `entity_id, shop_id, amount` |
| `PlatformFeeToTreasury` | `order_id, amount` |
| `TreasuryRefund` | `order_id, amount` |
| `CommissionRefundFailed` | `entity_id, shop_id, amount` |
| `UnallocatedCommissionPooled` | `entity_id, order_id, amount` |
| `PoolRewardDistributed` | `entity_id, order_id, total_distributed` |
| `UnallocatedPoolRefunded` | `entity_id, order_id, amount` |
| `EntityFundsWithdrawn` | `entity_id, to, amount` |

### Token 事件

| 事件 | 字段 |
|------|------|
| `TokenCommissionDistributed` | `entity_id, order_id, beneficiary, amount, commission_type, level` |
| `TokenCommissionWithdrawn` | `entity_id, account, amount` -- **[已废弃]** |
| `TokenCommissionCancelled` | `order_id, cancelled_count` |
| `TokenTieredWithdrawal` | `entity_id, account, repurchase_target, withdrawn_amount, repurchase_amount, bonus_amount` |
| `TokenWithdrawalConfigUpdated` | `entity_id` |
| `TokenShoppingBalanceUsed` | `entity_id, account, amount` |
| `TokenUnallocatedPooled` | `entity_id, order_id, amount` |
| `TokenUnallocatedPoolRefunded` | `entity_id, order_id, amount` |
| `EntityTokenFundsWithdrawn` | `entity_id, to, amount` |

### 治理事件

| 事件 | 字段 |
|------|------|
| `GlobalMinRepurchaseRateSet` | `entity_id, rate` |
| `GlobalMinTokenRepurchaseRateSet` | `entity_id, rate` |
| `TokenPlatformFeeRateUpdated` | `old_rate, new_rate` |
| `CommissionForceDisabled` | `entity_id` |
| `CommissionForceEnabled` | `entity_id` |
| `GlobalMaxCommissionRateSet` | `entity_id, rate` |
| `GlobalMaxTokenCommissionRateSet` | `entity_id, rate` |
| `WithdrawalCooldownUpdated` | `entity_id, nex_cooldown, token_cooldown` |
| `CommissionConfigCleared` | `entity_id` |
| `WithdrawalConfigCleared` | `entity_id` |
| `TokenWithdrawalConfigCleared` | `entity_id` |
| `GlobalCommissionPauseToggled` | `paused` |
| `WithdrawalPauseToggled` | `entity_id, paused` |
| `OrderRecordsArchived` | `order_id` |
| `OrderRecordsSettled` | `order_id` |
| `MinWithdrawalIntervalUpdated` | `entity_id, interval` |

---

## 11. Errors（42 项）

| 错误 | 说明 |
|------|------|
| `ShopNotFound` | 店铺不存在 |
| `EntityNotFound` | 实体不存在 |
| `NotShopOwner` | 不是店主 |
| `NotEntityOwner` | 不是实体所有者 |
| `CommissionNotConfigured` | 返佣未配置 |
| `InsufficientCommission` | NEX pending 余额不足 |
| `InvalidCommissionRate` | 无效返佣率（> 10000） |
| `RecordsFull` | 订单返佣记录达上限 |
| `Overflow` | 数值溢出 |
| `WithdrawalConfigNotEnabled` | 提现配置未启用 |
| `InvalidWithdrawalConfig` | 无效提现配置（tier 比率之和 != 10000 / bonus_rate > 10000 / 模式参数越界） |
| `InsufficientShoppingBalance` | 购物余额不足 |
| `WithdrawalCooldownNotMet` | 提现冻结期未满 |
| `NotDirectReferral` | 复购目标不是出资人的直推下线 |
| `AutoRegisterFailed` | 自动注册会员失败 |
| `ZeroWithdrawalAmount` | 提现金额为 0 |
| `DuplicateLevelId` | LevelBased 配置中 level_id 重复 |
| `TargetNotApprovedMember` | 复购目标未通过审批 |
| `MemberNotActivated` | **[已废弃]** 保留错误码索引 |
| `TargetParticipationDenied` | 复购目标不满足 Entity 参与要求 |
| `ParticipationRequirementNotMet` | 账户不满足参与要求（消费购物余额时） |
| `ShoppingBalanceWithdrawalDisabled` | 购物余额仅可用于购物，不可提取 |
| `InsufficientUnallocatedPool` | 沉淀资金池余额不足 |
| `InsufficientTokenCommission` | Token pending 余额不足 |
| `TokenTransferFailed` | Token 转账失败 |
| `InsufficientEntityFunds` | Entity NEX 偿付能力不足 |
| `InsufficientEntityTokenFunds` | Entity Token 偿付能力不足 |
| `PoolRewardCooldownActive` | POOL_REWARD 关闭后冷却期未满 |
| `CommissionPlanDisabled` | init_commission_plan 已禁用 |
| `TokenPlatformFeeRateTooHigh` | Token 平台费率超上限（最大 1000 bps） |
| `EntityLocked` | 实体已被全局锁定 |
| `LockedOnlyDecreaseAllowed` | 锁定状态下仅允许降低（单调递减豁免） |
| `NotEntityOwnerOrAdmin` | 既不是 Owner 也不是 COMMISSION_MANAGE Admin |
| `CommissionRateExceedsGlobalMax` | NEX max_commission_rate 超过全局上限 |
| `ConfigNotFound` | 配置不存在，无法清除 |
| `TokenCommissionRateExceedsGlobalMax` | Token max_commission_rate 超过全局上限 |
| `GlobalCommissionPaused` | 全局佣金紧急暂停中 |
| `WithdrawalPausedByOwner` | Entity 级提现已暂停 |
| `EntityNotActive` | Entity 未处于活跃状态 |
| `OrderRecordsNotFound` | 订单记录不存在或已归档 |
| `OrderRecordsNotFinalized` | 订单记录中存在 Pending，不可归档 |
| `WithdrawalIntervalNotMet` | 提现间隔未满足 |

---

## 12. Trait 实现

### CommissionProvider（15 方法）

供 Governance 和订单模块调用的 NEX 佣金外部接口：

| 方法 | 说明 |
|------|------|
| `process_commission` | 处理订单 NEX 返佣（双来源调度） |
| `cancel_commission` | 取消订单 NEX 返佣 |
| `pending_commission` | 查询会员待提取 NEX 佣金 |
| `shopping_balance` | 查询会员购物余额（委托 `T::Loyalty::shopping_balance`） |
| `use_shopping_balance` | 使用购物余额（委托 `T::Loyalty::consume_shopping_balance`） |
| `set_commission_modes` | 设置返佣模式（含 POOL_REWARD 跟踪） |
| `set_direct_reward_rate` | 设置直推奖励比率 |
| `set_level_diff_config` | 设置等级极差配置 |
| `set_fixed_amount` | 设置固定金额奖励 |
| `set_first_order_config` | 设置首单奖励配置 |
| `set_repeat_purchase_config` | 设置复购奖励配置 |
| `set_withdrawal_config_by_governance` | Governance 设置提现配置 |
| `set_min_repurchase_rate` | 设置全局最低复购比例 |
| `set_creator_reward_rate` | 设置创建人收益比例（<= 5000） |
| `settle_order_commission` | 订单完结时结算佣金（Pending -> Withdrawn） |

### TokenCommissionProvider（4 方法）

| 方法 | 说明 |
|------|------|
| `process_token_commission` | 处理订单 Token 返佣 |
| `cancel_token_commission` | 取消订单 Token 返佣 |
| `pending_token_commission` | 查询会员待提取 Token 佣金 |
| `token_platform_fee_rate` | 查询 Token 平台费率 |

### PoolBalanceProvider / TokenPoolBalanceProvider

供 pool-reward 模块访问沉淀池余额：

| 方法 | 说明 |
|------|------|
| `pool_balance` / `token_pool_balance` | 查询沉淀池余额 |
| `deduct_pool` / `deduct_token_pool` | 扣减沉淀池余额（不足时返回错误） |

### CommissionFundGuard

佣金资金已转入 Entity 账户，`protected_funds` 始终返回 0。

---

## 13. 资金流向

### NEX 资金流

```
池 A（平台费 -> 国库 + 招商人）：
  平台账户 ──transfer──> 国库（treasury_portion）
  平台账户 ──transfer──> Entity 账户（referrer_quota -> credit_commission）
  取消: Entity -> 平台账户 | 国库 -> 平台账户

池 B（卖家货款 -> 会员返佣 + 沉淀池）：
  seller ──transfer──> Entity 账户（插件分配 + POOL_REWARD 剩余）
  取消: Entity -> seller（仅转账成功的记录标记取消）

提现：
  Entity ──transfer(KeepAlive)──> 会员钱包（提现部分）
  复购 + 奖励 -> T::Loyalty::credit_shopping_balance（Loyalty 记账，资金留在 Entity）
```

### Token 资金流

```
池 A（Token 平台费 -> 招商人 + 留存）：
  token_platform_fee sweep -> EntityTokenAccountedBalance
  referrer_quota -> credit_token_commission（纯记账）
  pool_a_retention -> UnallocatedTokenPool（记账）
  取消: pool_a_retention 从 UnallocatedTokenPool 扣回

池 B（Entity Token -> 会员返佣 + 沉淀池）：
  entity_token_balance - committed -> 可用额度 -> 插件分配（纯记账）
  剩余 -> UnallocatedTokenPool（POOL_REWARD 模式）
  取消: token_transfer Entity -> seller

提现：
  Entity ──token_transfer──> 会员钱包（提现部分）
  复购 + 奖励 -> MemberTokenShoppingBalance（记账）
```

---

## 14. Loyalty 集成

### 背景

Phase 2 模块边界重构将 NEX 购物余额存储从 commission-core 迁移至独立的 Loyalty 模块。commission-core 不再直接持有 `MemberShoppingBalance` 和 `ShopShoppingTotal` 两项 NEX 存储。

### 接口

Config 新增 `type Loyalty: LoyaltyWritePort<AccountId, Balance>`，提供以下能力：

| 方法 | 说明 | 调用位置 |
|------|------|---------|
| `shopping_balance(entity_id, account)` | 查询 NEX 购物余额 | CommissionProvider trait、偿付安全检查 |
| `shopping_total(entity_id)` | 查询 Entity 级 NEX 购物余额总量 | 偿付安全检查（withdraw_entity_funds） |
| `credit_shopping_balance(entity_id, account, amount)` | 增加 NEX 购物余额 | 提现复购（withdraw_commission） |
| `consume_shopping_balance(entity_id, account, amount)` | 消费 NEX 购物余额 | settlement.rs 委托调用 |

### Token 购物余额

Token 购物余额（`MemberTokenShoppingBalance`、`TokenShoppingTotal`）仍由 commission-core 直接管理，未迁移。Token 侧的记账、消费、偿付安全检查均使用本地存储。

---

## 15. ParticipationGuard Trait

KYC/合规检查的泛型接口，在 `withdraw_commission` 和 `do_consume_shopping_balance` 中调用：

```rust
pub trait ParticipationGuard<AccountId> {
    fn can_participate(entity_id: u64, account: &AccountId) -> bool;
}

impl<AccountId> ParticipationGuard<AccountId> for () {
    fn can_participate(_: u64, _: &AccountId) -> bool { true }
}
```

Runtime 通过 `KycParticipationGuard` 桥接 `pallet-entity-kyc::can_participate_in_entity`：
- Entity 未配置 `EntityRequirements` 或 `mandatory=false` -> 允许所有
- Entity 配置 `mandatory=true` -> 检查账户 KYC 状态、级别、国家、风险评分、过期

---

## 16. 审计修复记录

### 安全审计

| 编号 | 级别 | 修复位置 | 说明 |
|------|------|----------|------|
| C1 | Critical | `withdraw_commission` | 偿付安全检查计入 repurchase + bonus 对购物余额总量的增量 |
| H1 | High | `withdraw_commission` | WithdrawalConfig 未启用时拒绝；auto_register 后验证 target 会员状态 |
| H2 | High | `cancel_commission` | 先尝试转账，成功后才取消记录 |
| H3 | High | `withdraw_*` / `do_consume_*` | 引入 ParticipationGuard 检查 Entity KYC 参与要求 |
| CC-M1 | Medium | `CommissionCancelled` 事件 | 增加 refund_succeeded / refund_failed 计数 |
| M1 | Low | `set_withdrawal_config` | level_overrides 添加 level_id 唯一性校验 |
| M3 | Medium | `TieredWithdrawal` 事件 | 新增 repurchase_target 字段 |

### 迭代审计

| 编号 | 说明 |
|------|------|
| M1-R4 | `set_global_min_token_repurchase_rate` 新增事件 |
| M2-R4 | 偿付安全改用 `InsufficientEntityFunds` / `InsufficientEntityTokenFunds` |
| M3-R4 | `cancel_commission` 消除 Token 取消重复，复用 `do_cancel_token_commission` |
| M4-R4 | `CommissionModes::is_valid()` 替代手动掩码校验 |
| L5-R4 | `set_min_repurchase_rate` 新增事件 |
| H1-R5 | POOL_REWARD 状态跟踪与 cooldown 一致性 |
| M1-R5 | `archive_order_records` 验证订单 entity_id 防止跨实体越权 |
| M2-R5 | `process_token_commission` 未配置佣金时先 sweep 再返回 |
| M1-R6 | 仅当 commission 已启用时才清除 POOL_REWARD cooldown |
| M2-R6 | 新增 `OrderTokenPlatformRetention` 存储，cancel 时回退 Pool A 留存 |
| L1-R7 | 移除未使用的 `sp-std` 依赖 |
| L2-R7 | 移除未使用的 `sp-core` dev-dependency |

### 深度审计修复（Phase 2）

| ID | 级别 | 修复内容 |
|----|------|----------|
| BUG-1 | Critical | `settle_order_commission` -- Pending -> Withdrawn 生命周期，使 archive 可正常归档 |
| BUG-2 | High | `set_commission_rate` -- 同时校验 GlobalMaxTokenCommissionRate |
| BUG-3 | High | `force_enable_entity_commission` -- 与 force_disable 对称的 Root 恢复路径 |
| BUG-4 | Medium | 标记 `CommissionWithdrawn` / `TokenCommissionWithdrawn` 事件为已废弃 |
| MISSING-2 | High | `retry_cancel_commission` -- Root 重试失败退款 |
| MISSING-3 | Medium | `set_min_withdrawal_interval` + `MemberLastWithdrawn` -- 基于时间的提现频率限制 |
| R-1 | -- | 标记 `CommissionStatus::Distributed` 为已废弃 |
| R-4 | -- | 标记 `MemberNotActivated` 错误为已废弃 |
| R-11 | -- | `set_commission_modes` 消除 `CommissionConfigs` 双重存储读取 |
| MISSING-1 | -- | 添加 `benchmarking.rs` 基准测试骨架框架 |

### 功能扩展

| 编号 | 说明 |
|------|------|
| F1 | Admin 权限支持 -- `ensure_owner_or_admin` + `COMMISSION_MANAGE` 权限位 |
| F2 | `set_withdrawal_cooldown` -- NEX/Token 冻结期独立配置 |
| F3 | Token 独立冻结期 -- `token_withdrawal_cooldown` 字段 |
| F4 | `clear_commission_config` / `clear_withdrawal_config` / `clear_token_withdrawal_config` |
| F6 | `MaxWithdrawalRecords` -- 每会员提现记录上限 |
| F7 | `MaxMemberOrderIds` -- 每会员佣金关联订单 ID 上限 |
| F13 | `set_global_min_repurchase_rate` -- NEX 全局最低复购比例 |
| F14 | `force_disable_entity_commission` -- Root 紧急禁用 |
| F15 | `GlobalMaxCommissionRate` -- 治理佣金率上限 |
| F16 | `GlobalMaxTokenCommissionRate` -- Token 佣金率上限 |
| F17 | `GlobalCommissionPaused` -- 全局紧急暂停 |
| F18 | `WithdrawalPaused` -- Entity 级提现暂停 |
| F19 | `MemberCommissionOrderIds` -- 佣金关联订单 ID 索引 |
| F20 | `MemberWithdrawalHistory` -- 提现历史 |
| F21 | `archive_order_records` -- 归档已完结订单释放存储 |
| R8 | `set_creator_reward_rate` -- 无币实体（None+Locked）单调递减豁免 |

---

## 17. 测试覆盖

208 个测试（`cargo test -p pallet-commission-core`），覆盖领域：

| 领域 | 覆盖内容 |
|------|---------|
| 基础配置 | set_commission_rate / set_commission_modes / enable_commission |
| 平台费分配 | referrer 50-50 split / dual_source / full_to_treasury |
| cancel_commission | 双来源 + 国库退款 + Token 取消 |
| 提现模式 | FullWithdrawal / FixedRate / LevelBased / MemberChoice |
| 三层约束 | Governance 底线 / Entity 设定 / 会员选择 |
| KYC/合规 | ParticipationGuard 提现阻止 / 购物余额消费阻止 |
| POOL_REWARD | 沉淀池入账 / cooldown 跟踪 / 跨路径一致性 |
| Token 管线 | process / credit / cancel / withdraw / shopping balance |
| Admin 权限 | Owner/Admin 配置 / 资金提取仅 Owner |
| F2-F4 | 独立冻结期 / 配置清除 |
| F13-F18 | 全局最低复购 / Root 禁用 / 佣金率上限 / 全局暂停 / Entity 暂停 |
| F19-F21 | 订单 ID 索引 / 提现历史 / 订单记录归档 |
| BUG-1 settle | Pending->Withdrawn / 保留 Cancelled / settle+archive 链路 / Token 结算 |
| BUG-2 | Token 全局上限阻止 / 双上限取小值 |
| BUG-3 | force_enable 禁用->启用->幂等 / 非 Root 拒绝 |
| MISSING-2 | retry_cancel 幂等 / 非 Root 拒绝 |
| MISSING-3 | 设置间隔 / 权限校验 / NEX+Token 频率限制 |
| R8 | None+Locked 允许降低 / 禁止增加 / FullDAO+Locked 全部禁止 |
| R-11 | 合并后 PoolReward 跟踪一致性 |
| 审计回归 | R4-R6 所有修复项对应回归测试 |

---

## 18. 依赖

```toml
[dependencies]
pallet-entity-common = { path = "../../common" }
pallet-commission-common = { path = "../common" }
sp-runtime = { workspace = true }
log = { workspace = true }
frame-benchmarking = { workspace = true, optional = true }

[dev-dependencies]
pallet-balances = { workspace = true, features = ["std"] }
sp-io = { workspace = true, features = ["std"] }
```
