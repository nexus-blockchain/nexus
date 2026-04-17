# pallet-entity-tokensale

> NEXUS Entity 代币发售模块 — 多模式发售、实际资金转账、锁仓解锁、KYC/内幕交易防护、Soft Cap、存储清理 | Runtime Index: 132

## 概述

`pallet-entity-tokensale` 实现 Entity 组织的代币公开发售（Token Sale / IEO）功能。Entity owner/admin 可配置多轮发售，支持 5 种发售模式、NEX 支付、灵活锁仓解锁策略、KYC 准入控制、内幕交易防护、Soft Cap 最低募资目标和完整的资金托管流。

## 资金流

```
subscribe:                认购者 NEX ──→ Pallet 托管账户
start_sale:               Entity 代币 ──reserve──→ 锁定
claim_tokens:             Entity 代币 ──repatriate──→ 认购者（初始解锁）
unlock_tokens:            Entity 代币 ──repatriate──→ 认购者（后续解锁）
end_sale:                 未售代币 ──unreserve──→ Entity 账户
end_sale (soft cap 未达): 未售代币 ──unreserve，状态→Cancelled
cancel_sale:              未售代币 ──unreserve──→ Entity 账户
claim_refund:             NEX ──→ 认购者 + Entity 代币 ──unreserve
withdraw_funds:           NEX ──→ Entity 派生账户
reclaim_unclaimed_tokens: 宽限期后未领退款 NEX+代币 → Entity
cleanup_round:            清理存储 + 释放 EntityRounds 槽位
```

## 架构

```
pallet-entity-tokensale (pallet_index = 132)
│
├── 外部依赖
│   ├── EntityProvider        Entity 存在性 / 激活状态 / 权限 / 派生账户 / 锁定检查
│   ├── Currency (NEX)        认购支付 / 退款 / 提取
│   ├── EntityTokenProvider   Entity 代币 reserve / unreserve / repatriate
│   ├── KycChecker            KYC 级别查询 (0-4)
│   └── DisclosureProvider    内幕交易防护（黑窗口期检查）
│
├── 数据结构
│   ├── SaleRound             轮次主体（模式、供应量、锁仓、soft cap）
│   ├── Subscription          认购记录（金额、支付、领取、解锁、退款标记）
│   ├── VestingConfig         锁仓策略（None / Linear / Cliff / Custom）
│   └── PaymentConfig         支付选项（价格、限额、启用状态）
│
├── 独立存储
│   ├── RoundPaymentOptions   支付选项（从 SaleRound 拆出，减少 I/O）
│   ├── RoundWhitelist        白名单 + 个人额度（DoubleMap）
│   ├── WhitelistCount        白名单计数
│   └── ActiveRounds          活跃轮次列表（on_initialize 扫描用）
│
├── 内部函数
│   ├── calculate_payment_amount    checked_mul 防溢出
│   ├── calculate_dutch_price       先除后乘避免 u128 溢出
│   ├── calculate_initial_unlock    基点计算 (bps / 10000)
│   ├── calculate_unlockable        悬崖期 + 线性/阶梯释放
│   └── do_auto_end_sale            on_initialize 自动结束（含 soft cap）
│
├── 查询函数
│   ├── pallet_account              托管账户地址
│   ├── get_current_price           当前价格（荷兰拍实时计算）
│   ├── get_subscription            认购信息
│   ├── get_unlockable_amount       可解锁数量
│   ├── get_sale_statistics         统计（供应/已售/剩余/参与者/募集额/soft cap）
│   └── has_active_sale             实体是否有活跃发售
│
└── Trait 实现
    └── TokenSaleProvider           跨 pallet 查询接口（7 个方法）
```

## 发售模式

| 模式 | 说明 | 价格机制 |
|------|------|----------|
| `FixedPrice` | 固定价格发售（默认） | 恒定价格 |
| `DutchAuction` | 荷兰拍卖 | 从 `start_price` 线性递减到 `end_price`（end_price > 0） |
| `WhitelistAllocation` | 白名单定向分配 | 固定价格，仅白名单可参与，支持个人额度 |
| `FCFS` | 先到先得 | 固定价格，售完即止 |
| `Lottery` | 抽签发售 | 尚未实现（创建时返回 `LotteryNotImplemented`） |

## 发售生命周期

```
                          ┌─ add_payment_option (至少一个)
                          ├─ set_vesting_config
create_sale_round ──→ [NotStarted] ──┤
(Entity owner/admin)      ├─ configure_dutch_auction (荷兰拍卖)
                          ├─ add_to_whitelist (白名单模式，支持个人额度)
                          ├─ update_sale_round (可选更新参数)
                          ├─ remove_payment_option / remove_from_whitelist
                          └─ cancel_sale (取消，不释放代币)
                                │
                         start_sale (reserve Entity 代币)
                                │
                                ▼
                           [Active] ←── subscribe / increase_subscription
                          /   │   \          时间窗口 + KYC + 内幕检查 + 白名单
                  end_sale  pause  cancel_sale
                  (soft cap) _sale  (释放未售)
                      │       │         │
                      │       ▼         │
                      │   [Paused]      │
                      │    / │  \       │
                      │ resume end cancel
                      │ _sale _sale _sale
                      │   │    │     │
                      │   ▼    │     │
                      │[Active]│     │
                      │        │     │
                      ▼        ▼     ▼
                  [Ended]  [Ended] [Cancelled]
                      │              │
               claim_tokens    claim_refund / force_batch_refund
               (代币→用户)     (NEX→用户 + 释放代币)
                      │              │
               unlock_tokens    reclaim_unclaimed_tokens
               (悬崖期后)       (宽限期后→Completed)
                      │
               withdraw_funds (NEX→Entity)
                      │
               cleanup_round (清理存储 + 释放 EntityRounds 槽位)
```

### 轮次状态 (RoundStatus)

| 状态 | 说明 | 可转入 |
|------|------|--------|
| `NotStarted` | 已创建，配置阶段 | start_sale→Active, cancel_sale→Cancelled |
| `Active` | 发售进行中，可认购 | end_sale→Ended/Cancelled, pause_sale→Paused, cancel_sale→Cancelled |
| `Paused` | 已暂停（不可认购，可恢复） | resume_sale→Active, end_sale→Ended, cancel_sale→Cancelled |
| `Ended` | 已结束（可领取代币/提取资金） | — |
| `Cancelled` | 已取消（认购者可退款） | reclaim_unclaimed_tokens→Completed |
| `Completed` | 退款宽限期后回收完成 | — |

### Soft Cap 机制

当 `soft_cap > 0` 且 `end_sale` / `on_initialize` 自动结束时，检查募集额是否达到 soft cap：
- **达标**: 正常结束 → `Ended`
- **未达标**: 自动转为 `Cancelled`，仅释放未售代币；已售代币保留 reserved，由 `claim_refund` 逐个释放

## 数据结构

### SaleRound

```rust
pub struct SaleRound<AccountId, Balance, BlockNumber> {
    pub id: u64,
    pub entity_id: u64,
    pub mode: SaleMode,
    pub status: RoundStatus,
    pub total_supply: Balance,
    pub sold_amount: Balance,
    pub remaining_amount: Balance,
    pub participants_count: u32,
    pub payment_options_count: u32,             // 实际数据在 RoundPaymentOptions
    pub vesting_config: VestingConfig<BlockNumber>,
    pub kyc_required: bool,
    pub min_kyc_level: u8,                     // 0-4
    pub start_block: BlockNumber,
    pub end_block: BlockNumber,
    pub dutch_start_price: Option<Balance>,
    pub dutch_end_price: Option<Balance>,
    pub creator: AccountId,
    pub created_at: BlockNumber,
    pub funds_withdrawn: bool,
    pub cancelled_at: Option<BlockNumber>,
    pub total_refunded_tokens: Balance,
    pub total_refunded_nex: Balance,
    pub soft_cap: Balance,                     // 最低募资目标（0 = 无 soft cap）
}
```

### Subscription

```rust
pub struct Subscription<AccountId, Balance, BlockNumber, AssetId> {
    pub subscriber: AccountId,
    pub round_id: u64,
    pub amount: Balance,                       // 认购 Entity 代币数量
    pub payment_asset: Option<AssetId>,        // None = 原生 NEX
    pub payment_amount: Balance,               // 实际支付 NEX
    pub subscribed_at: BlockNumber,
    pub claimed: bool,                         // 是否已领取初始解锁
    pub unlocked_amount: Balance,              // 累计已解锁
    pub refunded: bool,                        // 是否已退款
}
```

### VestingConfig / PaymentConfig

```rust
pub struct VestingConfig<BlockNumber> {
    pub vesting_type: VestingType,             // None / Linear / Cliff / Custom
    pub initial_unlock_bps: u16,               // 基点（10000 = 100%）
    pub cliff_duration: BlockNumber,           // 悬崖期（须 <= total_duration）
    pub total_duration: BlockNumber,           // 总解锁期
    pub unlock_interval: BlockNumber,          // 阶梯解锁间隔
}

pub struct PaymentConfig<AssetId, Balance> {
    pub asset_id: Option<AssetId>,             // None = NEX
    pub price: Balance,                        // 单价（须 > 0）
    pub min_purchase: Balance,                 // 最小购买量（须 > 0）
    pub max_purchase_per_account: Balance,     // 每人最大（须 >= min）
    pub enabled: bool,
}
```

## Config 配置

```rust
impl pallet_entity_tokensale::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;                      // NEX 支付/退款
    type AssetId = u64;
    type EntityProvider = EntityRegistry;           // Entity 权限/账户/锁定
    type TokenProvider = EntityToken;               // Entity 代币操作
    type KycChecker = TokenSaleKycBridge;           // KYC 级别查询
    type DisclosureProvider = EntityDisclosure;     // 内幕交易防护
    type MaxPaymentOptions = ConstU32<5>;
    type MaxWhitelistSize = ConstU32<1000>;
    type MaxRoundsHistory = ConstU32<50>;
    type MaxSubscriptionsPerRound = ConstU32<10000>;
    type MaxActiveRounds = ConstU32<100>;
    type RefundGracePeriod = ConstU64<14400>;       // ~48h
    type MaxBatchRefund = ConstU32<50>;
}
```

| 参数 | 说明 |
|------|------|
| `Currency` | NEX 货币（认购收款、退款、提取） |
| `EntityProvider` | Entity 存在性/激活状态/权限/派生账户/锁定检查 |
| `TokenProvider` | Entity 代币 reserve/unreserve/repatriate |
| `KycChecker` | KYC 级别查询（0-4） |
| `DisclosureProvider` | 内幕交易防护（黑窗口期禁止认购） |
| `MaxPaymentOptions` | 每轮最大支付选项数 |
| `MaxWhitelistSize` | 每轮白名单最大容量（add_to_whitelist 输入上限） |
| `MaxRoundsHistory` | 每 Entity 最大历史轮次数（cleanup_round 释放槽位） |
| `MaxSubscriptionsPerRound` | 每轮最大参与者数 |
| `MaxActiveRounds` | on_initialize 扫描上限（活跃轮次列表） |
| `RefundGracePeriod` | 退款宽限期（取消后多少区块内可退款） |
| `MaxBatchRefund` | force_batch_refund 每批最大退款数 |

## 存储项

| 存储 | 类型 | 说明 |
|------|------|------|
| `NextRoundId` | `StorageValue<u64>` | 自增轮次 ID（checked_add 防溢出） |
| `SaleRounds` | `StorageMap<u64, SaleRound>` | 轮次主表 |
| `EntityRounds` | `StorageMap<u64, BoundedVec<u64, MaxRoundsHistory>>` | Entity → 轮次索引（cleanup_round 释放槽位） |
| `Subscriptions` | `StorageDoubleMap<u64, AccountId, Subscription>` | 认购记录 |
| `RoundParticipants` | `StorageMap<u64, BoundedVec<AccountId, MaxSubscriptionsPerRound>>` | 参与者列表 |
| `RaisedFunds` | `StorageDoubleMap<u64, Option<AssetId>, Balance>` | 已募集金额（按资产分账） |
| `RoundPaymentOptions` | `StorageMap<u64, BoundedVec<PaymentConfig, MaxPaymentOptions>>` | 支付选项（从 SaleRound 拆出） |
| `ActiveRounds` | `StorageValue<BoundedVec<u64, MaxActiveRounds>>` | 活跃轮次 ID 列表（on_initialize 扫描） |
| `RoundWhitelist` | `StorageDoubleMap<u64, AccountId, Option<Balance>>` | 白名单 + 个人额度（None=默认限额） |
| `WhitelistCount` | `StorageMap<u64, u32>` | 白名单计数 |

## Extrinsics

| # | 调用 | 权限 | 前置状态 | 说明 |
|---|------|------|----------|------|
| 0 | `create_sale_round` | Entity owner/admin | — | 创建轮次（Entity 存在+激活+未锁定+soft_cap） |
| 1 | `add_payment_option` | 创建者 | NotStarted | 添加支付选项（price>0, max>=min, 去重asset_id） |
| 2 | `set_vesting_config` | 创建者 | NotStarted | 锁仓配置（bps≤10000, total≥cliff） |
| 3 | `configure_dutch_auction` | 创建者 | NotStarted + DutchAuction | 价格曲线（start>end>0） |
| 4 | `add_to_whitelist` | 创建者 | NotStarted | 白名单+个人额度（BoundedVec 输入） |
| 5 | `start_sale` | 创建者 | NotStarted | reserve Entity 代币 + 需≥1支付选项 |
| 6 | `subscribe` | signed | Active | NEX→托管（时间窗口+KYC+内幕+白名单） |
| 7 | `end_sale` | 创建者 | Active/Paused | 释放未售（须≥end_block或售罄，soft cap检查） |
| 8 | `claim_tokens` | 认购者 | Ended | Entity 代币→用户（初始解锁） |
| 9 | `unlock_tokens` | 认购者 | Ended + 已claimed | Entity 代币→用户（后续解锁） |
| 10 | `cancel_sale` | 创建者 | NotStarted/Active/Paused | 释放未售代币→Cancelled |
| 11 | `claim_refund` | 认购者 | Cancelled | NEX退还 + 释放对应代币 |
| 12 | `withdraw_funds` | 创建者 | Ended/Completed | NEX→Entity 派生账户 |
| 13 | `reclaim_unclaimed_tokens` | 创建者 | Cancelled + 宽限期后 | 回收未领退款→Completed |
| 14 | `force_cancel_sale` | Root | NotStarted/Active/Paused | 治理强制取消 |
| 15 | `force_end_sale` | Root | Active/Paused | 治理强制结束 |
| 16 | `force_refund` | Root | Cancelled | 治理强制退款指定认购者 |
| 17 | `force_withdraw_funds` | Root | Ended/Completed | 治理强制提取资金 |
| 18 | `update_sale_round` | 创建者 | NotStarted | 更新轮次参数（可选字段） |
| 19 | `increase_subscription` | 已认购者 | Active | 追加认购量（内幕+KYC+限额检查） |
| 20 | `remove_from_whitelist` | 创建者 | NotStarted | 从白名单移除地址 |
| 21 | `remove_payment_option` | 创建者 | NotStarted | 按索引移除支付选项 |
| 22 | `extend_sale` | 创建者 | Active/Paused | 延长发售时间（仅延长不缩短） |
| 23 | `pause_sale` | 创建者 | Active | 暂停发售（Active→Paused） |
| 24 | `resume_sale` | 创建者 | Paused | 恢复发售（Paused→Active） |
| 25 | `cleanup_round` | 创建者 | Ended/Completed + 已提取 | 清理存储 + 释放 EntityRounds 槽位 |
| 26 | `force_batch_refund` | Root | Cancelled | 批量强制退款（跳过已退/失败项） |

### subscribe 详细流程

1. 验证轮次状态为 `Active`
2. 验证 `remaining_amount >= amount`
3. **时间窗口校验**：`now ∈ [start_block, end_block]`
4. 验证未重复认购
5. **内幕交易防护**：`DisclosureProvider::can_insider_trade` 检查黑窗口期
6. **KYC 校验**：`kyc_required` 时检查 `KycChecker::kyc_level >= min_kyc_level`
7. **白名单校验**：WhitelistAllocation 模式查 `RoundWhitelist` 存储
8. **预检参与者容量**：`RoundParticipants.len() < MaxSubscriptionsPerRound`
9. 查找匹配支付选项（asset_id + enabled）
10. 验证购买量 `[min_purchase, effective_max]`（白名单个人额度优先）
11. **checked_mul** 计算支付金额（溢出返回 ArithmeticOverflow）
12. **Currency::transfer** 从用户到 Pallet 托管账户
13. 创建认购记录，更新轮次数据，更新募集统计

### 荷兰拍卖价格

```
price_range = start_price - end_price
quotient = price_range / total_duration
remainder = price_range % total_duration
price_drop = quotient × elapsed + (remainder × elapsed / total_duration)
current_price = max(start_price - price_drop, end_price)
```

先除后乘避免 u128 溢出，余数单独处理保留精度。`now ≤ start` 返回 `start_price`，`now ≥ end` 返回 `end_price`。

## 锁仓解锁

```
subscribe ──→ claim_tokens (初始解锁) ──→ [悬崖期] ──→ unlock_tokens (线性/阶梯释放)
```

| 类型 | 说明 |
|------|------|
| `None` | 无锁仓，claim 即全额 |
| `Linear` | 初始解锁 + 悬崖期后连续线性释放 |
| `Cliff` | 悬崖期结束后按 `unlock_interval` 阶梯释放 |
| `Custom` | 自定义（预留） |

```
initial_unlock = total × initial_unlock_bps / 10000
vesting_amount = total × (10000 - initial_unlock_bps) / 10000

# Linear: 连续线性
effective_elapsed = elapsed_since_cliff

# Cliff: 按 unlock_interval 阶梯取整
effective_elapsed = (elapsed_since_cliff / unlock_interval) × unlock_interval

unlocked = vesting_amount × effective_elapsed / vesting_duration
new_unlock = (initial_unlock + unlocked) - already_unlocked
```

## Events

| 事件 | 字段 | 触发时机 |
|------|------|----------|
| `SaleRoundCreated` | round_id, entity_id, mode, total_supply | create_sale_round |
| `PaymentOptionAdded` | round_id, asset_id | add_payment_option |
| `VestingConfigSet` | round_id | set_vesting_config |
| `DutchAuctionConfigured` | round_id | configure_dutch_auction |
| `SaleRoundStarted` | round_id | start_sale |
| `SaleRoundEnded` | round_id, sold_amount, participants_count | end_sale |
| `SaleRoundCancelled` | round_id | cancel_sale |
| `Subscribed` | round_id, subscriber, amount, payment_amount | subscribe |
| `TokensClaimed` | round_id, subscriber, amount | claim_tokens |
| `TokensUnlocked` | round_id, subscriber, amount | unlock_tokens |
| `WhitelistUpdated` | round_id, count | add_to_whitelist |
| `FundsWithdrawn` | round_id, recipient, amount | withdraw_funds |
| `RefundClaimed` | round_id, subscriber, amount | claim_refund |
| `ExpiredRefundsReclaimed` | round_id, tokens_reclaimed, nex_reclaimed | reclaim_unclaimed_tokens |
| `SaleRoundForceCancelled` | round_id | force_cancel_sale |
| `SaleRoundForceEnded` | round_id, sold_amount, participants_count | force_end_sale |
| `ForceRefundIssued` | round_id, subscriber, amount | force_refund |
| `ForceFundsWithdrawn` | round_id, recipient, amount | force_withdraw_funds |
| `SaleRoundUpdated` | round_id | update_sale_round |
| `SubscriptionIncreased` | round_id, subscriber, additional_amount, additional_payment | increase_subscription |
| `WhitelistRemoved` | round_id, removed_count | remove_from_whitelist |
| `PaymentOptionRemoved` | round_id, index | remove_payment_option |
| `SaleExtended` | round_id, new_end_block | extend_sale |
| `SaleRoundPaused` | round_id | pause_sale |
| `SaleRoundResumed` | round_id | resume_sale |
| `SaleAutoEnded` | round_id, sold_amount, participants_count | on_initialize 自动结束 |
| `SoftCapNotMet` | round_id, raised, soft_cap | end_sale / on_initialize soft cap 未达标 |
| `RoundStorageCleaned` | round_id, subscriptions_removed | cleanup_round |
| `ForceBatchRefundIssued` | round_id, refunded_count, total_nex | force_batch_refund |

## Errors

| 错误 | 说明 |
|------|------|
| `RoundNotFound` | 轮次不存在 |
| `RoundNotStarted` | 轮次未开始 |
| `RoundEnded` | 轮次已结束 |
| `RoundCancelled` | 轮次已取消 |
| `SoldOut` | 剩余不足 |
| `InvalidRoundStatus` | 无效的轮次状态 |
| `InsufficientBalance` | 余额不足 |
| `ExceedsPurchaseLimit` | 超过购买限额 |
| `BelowMinPurchase` | 低于最小购买量 |
| `NotInWhitelist` | 白名单模式下不在名单 |
| `InsufficientKycLevel` | KYC 级别 < min_kyc_level |
| `InvalidPaymentAsset` | 无效支付资产（当前仅支持 NEX） |
| `AlreadySubscribed` | 重复认购 |
| `NotSubscribed` | 未认购 |
| `AlreadyClaimed` | 重复领取 |
| `NoTokensToUnlock` | 无可解锁代币 |
| `CliffNotReached` | 悬崖期未到 |
| `Unauthorized` | 非 Entity owner/admin |
| `WhitelistFull` | 白名单已满 |
| `RoundsHistoryFull` | 轮次历史已满（cleanup_round 释放槽位） |
| `ParticipantsFull` | 参与者已满 |
| `PaymentOptionsFull` | 支付选项已满 |
| `InvalidDutchAuctionConfig` | 荷兰拍卖配置无效（start ≤ end 或 end = 0） |
| `InvalidVestingConfig` | 锁仓配置无效（bps > 10000） |
| `EntityNotFound` | Entity 不存在 |
| `EntityNotActive` | Entity 未激活 |
| `InvalidTotalSupply` | total_supply = 0 |
| `InvalidTimeWindow` | end_block ≤ start_block |
| `InvalidPrice` | price = 0 或 min_purchase = 0 |
| `InvalidVestingDuration` | cliff > total |
| `SaleNotInTimeWindow` | 认购不在时间窗口；或提前结束发售 |
| `ArithmeticOverflow` | amount × price 溢出 u128 |
| `NoPaymentOptions` | start_sale 时无支付选项 |
| `InvalidKycLevel` | min_kyc_level > 4 |
| `InsufficientTokenSupply` | Entity 代币余额不足 |
| `FundsAlreadyWithdrawn` | 重复提取 |
| `SaleNotCancelled` | 退款需 Cancelled 状态 |
| `AlreadyRefunded` | 重复退款 |
| `InvalidPurchaseLimits` | max < min |
| `RefundPeriodNotExpired` | 退款宽限期未到期 |
| `DutchAuctionNotConfigured` | 荷兰拍卖未配置价格曲线 |
| `ActiveRoundsFull` | 活跃轮次已满 |
| `RoundIdOverflow` | 轮次 ID 溢出（u64） |
| `StartBlockInPast` | start_block 在过去 |
| `IncompleteUnreserve` | Entity 代币 unreserve 不完整 |
| `DuplicatePaymentOption` | 重复的支付选项（相同 asset_id） |
| `EntityLocked` | 实体已被全局锁定 |
| `NoUpdateProvided` | update_sale_round 所有参数均为 None |
| `PaymentOptionNotFound` | 支付选项索引不存在 |
| `InvalidExtension` | 新结束时间必须大于当前结束时间 |
| `SaleNotPaused` | 发售未处于暂停状态 |
| `InsiderTradingBlocked` | 内幕人员在黑窗口期禁止认购 |
| `RoundNotCleanable` | 轮次不可清理（未到终态或资金未提取） |
| `EmptyBatch` | 批量操作列表为空 |
| `LotteryNotImplemented` | Lottery 模式尚未实现 |
| `SoftCapNotMet` | Soft cap 未达标，发售已自动取消 |

## 权限模型

所有 Entity owner/创建者操作均检查 `EntityLocked` 状态。Root 操作不检查锁定。

| 操作 | 调用方 | 前置条件 |
|------|--------|----------|
| `create_sale_round` | Entity owner/admin | Entity 存在+激活+未锁定 |
| `add_payment_option` | 轮次创建者 | NotStarted + Entity 未锁定 |
| `set_vesting_config` | 轮次创建者 | NotStarted + Entity 未锁定 |
| `configure_dutch_auction` | 轮次创建者 | NotStarted + DutchAuction + Entity 未锁定 |
| `add_to_whitelist` | 轮次创建者 | NotStarted + Entity 未锁定 |
| `start_sale` | 轮次创建者 | NotStarted + ≥1 支付选项 + Entity 未锁定 |
| `subscribe` | signed | Active + 时间窗口 + KYC + 内幕检查 + 白名单 |
| `end_sale` | 轮次创建者 | Active/Paused + (now≥end_block 或售罄) + Entity 未锁定 |
| `claim_tokens` | 认购者 | Ended + 未 claimed |
| `unlock_tokens` | 认购者 | Ended + 已 claimed + 悬崖期后 |
| `cancel_sale` | 轮次创建者 | NotStarted/Active/Paused + Entity 未锁定 |
| `claim_refund` | 认购者 | Cancelled + 未退款 |
| `withdraw_funds` | 轮次创建者 | Ended/Completed + 未提取 + Entity 未锁定 |
| `reclaim_unclaimed_tokens` | 轮次创建者 | Cancelled + 宽限期后 + Entity 未锁定 |
| `force_cancel_sale` | Root | NotStarted/Active/Paused |
| `force_end_sale` | Root | Active/Paused |
| `force_refund` | Root | Cancelled + 指定认购者未退款 |
| `force_withdraw_funds` | Root | Ended/Completed + 未提取 |
| `update_sale_round` | 轮次创建者 | NotStarted + Entity 未锁定 |
| `increase_subscription` | 已认购者 | Active + 时间窗口 + 内幕检查 + 限额内 |
| `remove_from_whitelist` | 轮次创建者 | NotStarted + Entity 未锁定 |
| `remove_payment_option` | 轮次创建者 | NotStarted + Entity 未锁定 |
| `extend_sale` | 轮次创建者 | Active/Paused + Entity 未锁定 |
| `pause_sale` | 轮次创建者 | Active + Entity 未锁定 |
| `resume_sale` | 轮次创建者 | Paused + Entity 未锁定 |
| `cleanup_round` | 轮次创建者 | Ended/Completed + funds_withdrawn |
| `force_batch_refund` | Root | Cancelled |

## 测试

```bash
cargo test -p pallet-entity-tokensale
# 146 tests passed
```

**按类别分组（146 个）：**

### 基础功能 (49)

- 创建/配置/启动/认购/结束/领取/取消/退款/提取/回收全流程
- 输入校验（zero supply, bad time, kyc level, price, limits, overflow）
- 白名单独立存储 + 去重 + 状态检查
- 荷兰拍卖配置 + price=0 允许/拒绝
- Cliff/Linear 解锁计算验证
- on_initialize 自动结束（单轮/多轮/未过期跳过）
- 支付选项独立存储验证
- 退款宽限期回收流程

### 审计回归 (27)

- **C1**: reclaim 后阻止 withdraw 双重提取
- **H2**: Completed 状态拒绝 claim_tokens
- **H3**: 拒绝非 None asset_id
- **M1**: end_sale/auto_end/cancel_sale 清零 remaining_amount
- **M2**: Ended 状态限制 unlock_tokens
- **L2**: NextRoundId 溢出检测
- **L3**: 过去 start_block 拒绝
- **L5**: 荷兰拍卖 end_price=0 拒绝
- **H1-deep**: claim/unlock 不完整转移检测
- **M1-deep**: cancel NotStarted 保持 remaining
- **M2-deep**: 荷兰拍卖价格钳制 + 精确递减
- **M1-R5**: 重复 asset_id 拒绝

### Root 治理 (12)

- force_cancel_sale / force_end_sale / force_refund / force_withdraw_funds
- 各函数 origin 检查 + 状态校验

### 功能增强 P1-P3 (25)

- update_sale_round（参数更新/校验/状态检查）
- increase_subscription（追加认购/限额/售罄检查）
- remove_from_whitelist（移除/不存在跳过/状态检查）
- remove_payment_option（移除/索引校验/状态检查）
- extend_sale（延长/缩短拒绝/Paused 可延长）
- pause_sale / resume_sale（状态转换/状态检查）
- Paused 状态下 subscribe 拒绝 / cancel / end / force_end / force_cancel / on_initialize 跳过

### Soft Cap (4)

- soft cap 达标正常结束
- soft cap 未达标自动取消
- on_initialize soft cap 自动取消
- zero soft cap 正常结束

### 内幕交易防护 (3)

- insider 被 subscribe 阻止
- 非 insider 可正常认购
- insider 被 increase_subscription 阻止

### 白名单个人额度 (2)

- 个人额度限制生效
- None 额度使用默认限额

### 发售统计 (2)

- get_sale_statistics 返回正确数据
- 不存在轮次返回 None

### TokenSaleProvider trait (2)

- active_sale_round 返回活跃轮次
- 不存在轮次返回 None

### 存储清理 F8 (4)

- cleanup_round 清理全部存储
- 拒绝活跃轮次清理
- 拒绝非创建者清理
- 拒绝资金未提取清理

### 批量强制退款 F9 (5)

- force_batch_refund 正常退款
- 拒绝非 root
- 拒绝非 Cancelled 状态
- 拒绝空批量
- 跳过已退款项

### EntityLocked (1)

- Entity 锁定时拒绝 create_sale_round

### 活跃发售检查 F12 (4)

- has_active_sale 返回 true
- 无活跃发售返回 false
- 结束后返回 false
- Paused 仍返回 true

### 审计回归 R6 (3)

- H1-R6: claim_refund 在 end_sale soft cap 失败后正常工作
- H1-R6: claim_refund 在 auto_end soft cap 失败后正常工作
- H1-R6: reclaim 在 soft cap 取消后正常工作

### 审计回归 R7 (3)

- M1-R7: cleanup_round 释放 EntityRounds 槽位
- M1-R7: cleanup 后允许创建新轮次
- L1-R7: force_batch_refund 部分 unreserve 时重新锁定代币

## Hooks

### on_initialize

每个区块自动扫描 `ActiveRounds` 列表，结束所有 `now > end_block` 且状态为 `Active` 的轮次（跳过 `Paused`）。包含 Soft Cap 检查。

### integrity_test

`#[cfg(test)]` 模式下校验所有 Config 常量 > 0：`MaxPaymentOptions`, `MaxWhitelistSize`, `MaxRoundsHistory`, `MaxSubscriptionsPerRound`, `MaxActiveRounds`, `MaxBatchRefund`。

## TokenSaleProvider Trait

跨 pallet 查询接口（`pallet_entity_common::TokenSaleProvider`）：

| 方法 | 返回 | 说明 |
|------|------|------|
| `active_sale_round(entity_id)` | `Option<u64>` | 实体活跃轮次 ID（含 Paused） |
| `sale_round_status(round_id)` | `Option<TokenSaleStatus>` | 轮次状态映射 |
| `sold_amount(round_id)` | `Option<Balance>` | 已售数量 |
| `remaining_amount(round_id)` | `Option<Balance>` | 剩余数量 |
| `participants_count(round_id)` | `Option<u32>` | 参与者数 |
| `sale_total_supply(round_id)` | `Option<Balance>` | 总供应量 |
| `sale_entity_id(round_id)` | `Option<u64>` | 轮次所属 Entity |

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-02-03 | Phase 8 初始版本 |
| v0.1.1 | 2026-02-09 | 模块文件夹从 `sale` 重命名为 `tokensale` |
| v0.2.0 | 2026-02-09 | 深度审计修复（20 项） |
| v0.3.0 | 2026-02-23 | 二次深度审计（4 项修复 + 5 新测试） |
| v0.4.0 | 2026-02-26 | 三次审计（3 项修复 + 3 新测试） |
| v0.5.0 | 2026-03 | 功能增强（11 新 extrinsics + Paused + 59 新测试） |
| v0.6.0 | 2026-03 | 审计 R5-R6（F2-F12 功能 + soft cap + 内幕防护 + 存储清理 + 批量退款） |
| v0.7.0 | 2026-03 | 审计 R7（M1-R7 cleanup_round 释放 EntityRounds 槽位 + L1-R7 force_batch_refund 回滚 + 3 回归测试） |

### v0.2.0 审计修复详情

- **C1**: 4 个 struct 添加 `DecodeWithMemTracking`
- **H1-H2**: 输入校验（total_supply > 0, end > start）
- **H3**: 集成 `EntityProvider` 验证 Entity 存在/激活/权限
- **H4-H5**: 集成 `Currency` + `EntityTokenProvider` 实现实际资金转账
- **H6-H7**: 支付选项校验（price > 0, max >= min, cliff <= total）
- **H8**: 集成 `KycChecker` 做 KYC 级别检查
- **H9**: subscribe 时间窗口校验 `[start_block, end_block]`
- **H10**: `checked_mul` 替代 `saturating_mul` 防溢出
- **M1-M2**: Weight 修正（ref_time 200M+, proof_size > 0）
- **M3**: `configure_dutch_auction` 添加 NotStarted 状态检查
- **M4**: 新增 `claim_refund`(11) 处理取消后退款
- **M5**: `add_to_whitelist` 添加 NotStarted 状态检查
- **L1**: `start_sale` 校验 ≥1 支付选项
- **L2**: `min_kyc_level <= 4` 校验
- **L3**: 白名单拆分为 `RoundWhitelist` + `WhitelistCount` 独立存储
- **L4**: 配置变更操作添加事件通知
- 新增 `withdraw_funds`(12) 提取募集 NEX
- SaleRound 新增 `funds_withdrawn` 字段
- Subscription 新增 `refunded` 字段

### v0.3.0 二次深度审计

- **C1**: `add_to_whitelist` 参数改为 `BoundedVec`，防无界输入 DoS
- **H1**: `subscribe` NEX 转账前预检参与者容量（fail-fast）
- **H2**: `end_sale` 强制检查 `now >= end_block || remaining_amount == 0`
- **M1**: `calculate_unlockable` 实现 Cliff 阶梯解锁（原忽略 interval）
- 新增 5 个测试 + 4 个已有测试适配

### v0.4.0 三次审计

- **C1**: `reclaim_unclaimed_tokens` 标记 `funds_withdrawn = true`，防双重提取
- **H2**: `claim_tokens` 仅允许 `Ended` 状态（拒绝 Completed）
- **H3**: `add_payment_option` 强制 `asset_id = None`（防键不一致）
- 新增 3 个回归测试

### v0.5.0 功能增强

**新增 RoundStatus:** `Paused`（Active→Paused→Active 可恢复）

**新增 Extrinsics (11):**
- Root 治理: `force_cancel_sale`(14), `force_end_sale`(15), `force_refund`(16), `force_withdraw_funds`(17)
- Owner: `update_sale_round`(18), `remove_from_whitelist`(20), `remove_payment_option`(21), `extend_sale`(22), `pause_sale`(23), `resume_sale`(24)
- Subscriber: `increase_subscription`(19)

**新增 Events (13), Errors (4), 测试 59 个** (108 total)

### v0.6.0 审计 R5-R6

**新增功能:**
- **F2**: Soft Cap 最低募资目标（`soft_cap` 字段 + `SoftCapNotMet` 事件/错误）
- **F4**: 内幕交易防护（`DisclosureProvider` + `InsiderTradingBlocked` 错误）
- **F5**: 白名单个人额度（`RoundWhitelist` 值改为 `Option<Balance>`）
- **F6**: 发售统计查询 `get_sale_statistics`
- **F7**: `TokenSaleProvider` trait 实现
- **F8**: 存储清理 `cleanup_round`(25)（清理认购/白名单/支付/募集/EntityRounds）
- **F9**: 批量强制退款 `force_batch_refund`(26)（跳过已退/失败项 + 统计更新）
- **F10**: `integrity_test` 配置参数校验
- **F11**: Lottery 模式拒绝（`LotteryNotImplemented`）
- **F12**: 活跃发售检查 `has_active_sale`

**审计修复:**
- **M1-R5**: `add_payment_option` 检查重复 `asset_id`（`DuplicatePaymentOption`）
- **H1-R6**: Soft cap 失败时仅释放未售代币，已售代币由 `claim_refund` 逐个释放
- **M1-R6**: Mock `new_test_ext()` 清理 thread-local 状态

**新增测试: 38 个** (146 total)

### v0.7.0 审计 R7

- **M1-R7**: `cleanup_round` 新增 `EntityRounds::mutate` 移除 round_id，释放 `MaxRoundsHistory` 槽位
- **L1-R7**: `force_batch_refund` 部分 unreserve 时重新 reserve 已释放代币，保持状态一致
- 新增 3 个回归测试（cleanup 槽位释放 + 新轮次创建 + 部分 unreserve 回滚）

## 许可证

MIT License
