# pallet-grouprobot-subscription

订阅管理、Escrow 预存余额与层级费用结算。从 consensus pallet 拆分而来。

## 概述

本 pallet 负责:

- **付费订阅** — Bot 所有者通过预存 Escrow 订阅 Basic/Pro/Enterprise 层级
- **广告承诺订阅** — 群主承诺社区接受 N 条广告/Era，换取对应层级权益 (无需付费)
- **有效层级** — 综合付费订阅与广告承诺，取较高者作为 Bot 实际层级
- **Era 结算** — 游标分页扣除订阅费 (90% 运营者 + 10% 国库)，检查广告承诺达标情况

## 订阅层级

| 层级 | 付费 | max_rules | log_retention_days | forced_ads_per_day | can_disable_ads | tee_access |
|------|------|-----------|--------------------|--------------------|-----------------|------------|
| Free | — | 3 | 7 | 2 | ✗ | ✗ |
| Basic | BasicFeePerEra | 10 | 30 | 0 | ✓ | ✗ |
| Pro | ProFeePerEra | 50 | 90 | 0 | ✓ | ✓ |
| Enterprise | EnterpriseFeePerEra | ∞ | 永久 | 0 | ✓ | ✓ |

## 获取层级的两种路径

1. **付费订阅** (`subscribe`) — Bot 所有者预存 Escrow，每 Era 自动扣费
2. **广告承诺** (`commit_ads`) — 群主承诺每 Era 接受 ≥ N 条广告，由阈值映射层级:
   - `committed_ads_per_era ≥ AdEnterpriseThreshold` → Enterprise
   - `committed_ads_per_era ≥ AdProThreshold` → Pro
   - `committed_ads_per_era ≥ AdBasicThreshold` → Basic
   - 否则 → Free (不允许创建)

`effective_tier(bot_id_hash)` 取两种路径的较高者。

## 数据结构

### SubscriptionRecord\<T\>

| 字段 | 类型 | 说明 |
|------|------|------|
| `owner` | `AccountId` | 订阅所有者 |
| `bot_id_hash` | `BotIdHash` | Bot 标识 |
| `tier` | `SubscriptionTier` | 订阅层级 |
| `fee_per_era` | `Balance` | 每 Era 费用 |
| `started_at` | `BlockNumber` | 开始区块 |
| `status` | `SubscriptionStatus` | 状态 (Active / PastDue / Suspended / Cancelled) |

### AdCommitmentRecord\<T\>

| 字段 | 类型 | 说明 |
|------|------|------|
| `owner` | `AccountId` | 承诺所有者 |
| `bot_id_hash` | `BotIdHash` | Bot 标识 |
| `community_id_hash` | `CommunityIdHash` | 社区标识 |
| `committed_ads_per_era` | `u32` | 每 Era 承诺接受的广告数 |
| `effective_tier` | `SubscriptionTier` | 对应的权益层级 |
| `underdelivery_eras` | `u8` | 连续未达标 Era 数 |
| `status` | `AdCommitmentStatus` | 状态 (Active / Underdelivery / Cancelled) |
| `started_at` | `BlockNumber` | 开始区块 |

### 状态机

**SubscriptionStatus:**

```
Active ──(escrow不足)──► PastDue ──(仍不足)──► Suspended (→ FreeTierFallback)
  │                        │                        │
  │ (充值≥fee)             │ (充值≥fee)              │ (充值≥fee)
  │ ◄─────────────────────┘ ◄──────────────────────┘
  │
  └──(cancel)──► Cancelled
```

**AdCommitmentStatus:**

```
Active ──(未达标)──► Underdelivery ──(连续超限)──► Cancelled (→ Downgraded)
  │                     │
  │ (达标)              │ (达标)
  │ ◄─────────────────┘
  │
  └──(cancel)──► Cancelled
```

## Config

| 项 | 类型 | 说明 |
|----|------|------|
| `Currency` | `ReservableCurrency<AccountId>` | 原生货币 (支持 reserve/unreserve) |
| `BotRegistry` | `BotRegistryProvider<AccountId>` | Bot 注册查询 |
| `BasicFeePerEra` | `Get<Balance>` (constant) | Basic 层级每 Era 费用 |
| `ProFeePerEra` | `Get<Balance>` (constant) | Pro 层级每 Era 费用 |
| `EnterpriseFeePerEra` | `Get<Balance>` (constant) | Enterprise 层级每 Era 费用 |
| `TreasuryAccount` | `Get<AccountId>` | 国库账户 (订阅费 10% 转入) |
| `RewardPoolAccount` | `Get<AccountId>` | 奖励池账户 (订阅费 90% 节点份额转入) |
| `MaxSubscriptionSettlePerEra` | `Get<u32>` (constant) | 每次 Era 结算最多处理的订阅数 (游标分页) |
| `EraLength` | `Get<BlockNumber>` (constant) | Era 长度 (区块数，用于按比例计算取消费用) |
| `EraStartBlockProvider` | `Get<BlockNumber>` | Era 起始区块查询 |
| `CurrentEraProvider` | `Get<u64>` | 当前 Era 编号查询 |
| `AdDelivery` | `AdDeliveryProvider` | 广告投放计数查询 |
| `AdBasicThreshold` | `Get<u32>` (constant) | Basic 层级最低广告数/Era |
| `AdProThreshold` | `Get<u32>` (constant) | Pro 层级最低广告数/Era |
| `AdEnterpriseThreshold` | `Get<u32>` (constant) | Enterprise 层级最低广告数/Era |
| `MaxUnderdeliveryEras` | `Get<u8>` (constant) | 连续未达标最大 Era 数 (超过则降级) |

## Storage

| 名称 | 类型 | 说明 |
|------|------|------|
| `Subscriptions` | `StorageMap<BotIdHash, SubscriptionRecord>` | 付费订阅表 |
| `SubscriptionEscrow` | `StorageMap<BotIdHash, Balance>` | 订阅 Escrow 预存余额 (ValueQuery) |
| `SubscriptionSettleCursor` | `StorageValue<BotIdHash>` | 订阅结算游标 (None=从头开始) |
| `SubscriptionSettlePending` | `StorageValue<bool>` | 当前 Era 是否有待结算订阅 (ValueQuery) |
| `AdCommitments` | `StorageMap<BotIdHash, AdCommitmentRecord>` | 广告承诺订阅表 |
| `AdCommitmentSettleCursor` | `StorageValue<BotIdHash>` | 广告承诺结算游标 |

## Extrinsics

| call_index | 名称 | 签名 | 说明 |
|------------|------|------|------|
| 0 | `subscribe` | `origin, bot_id_hash, tier, deposit` | 创建付费订阅，预存 Escrow |
| 1 | `deposit_subscription` | `origin, bot_id_hash, amount` | 充值订阅 Escrow |
| 2 | `cancel_subscription` | `origin, bot_id_hash` | 取消订阅，按比例退款 |
| 3 | `change_tier` | `origin, bot_id_hash, new_tier` | 变更订阅层级 |
| 4 | `commit_ads` | `origin, bot_id_hash, community_id_hash, committed_ads_per_era` | 创建广告承诺订阅 |
| 5 | `cancel_ad_commitment` | `origin, bot_id_hash` | 取消广告承诺订阅 |
| 6 | `cleanup_subscription` | `origin, bot_id_hash` | 清理已取消的付费订阅记录 (任何人可调用) |
| 7 | `cleanup_ad_commitment` | `origin, bot_id_hash` | 清理已取消的广告承诺记录 (任何人可调用) |

### subscribe (call_index 0)

1. 验证调用者为 Bot 所有者，Bot 已激活且已分配运营者
2. 层级不能为 Free，存款 ≥ 层级费用
3. Reserve 存款到 Escrow
4. 创建 SubscriptionRecord (status=Active)

### deposit_subscription (call_index 1)

1. 验证调用者为订阅所有者，金额非零，订阅未取消
2. Reserve 金额并累加到 Escrow
3. 若订阅处于 PastDue/Suspended 且充值后 Escrow ≥ fee_per_era，则重新激活为 Active

### cancel_subscription (call_index 2)

1. 验证调用者为 Bot 所有者，订阅未取消
2. 设置状态为 Cancelled
3. 按比例计算当期已使用费用: `fee_per_era × blocks_used / era_length`
4. Unreserve 全部 Escrow，按比例费用转入国库，剩余退还

### change_tier (call_index 3)

1. 验证调用者为 Bot 所有者，新层级非 Free
2. 订阅状态必须为 Active 或 PastDue
3. M1-R4: 升级到更贵层级时验证 Escrow ≥ 新层级费用
4. 更新 tier 和 fee_per_era

### commit_ads (call_index 4)

1. 验证调用者为 Bot 所有者，Bot 已激活且已分配运营者
2. 承诺数量 → 层级映射，层级必须 ≥ Basic
3. 创建 AdCommitmentRecord (status=Active)

### cancel_ad_commitment (call_index 5)

1. 验证调用者为 Bot 所有者，承诺未取消
2. 设置状态为 Cancelled

### cleanup_subscription (call_index 6)

1. 任何签名账户均可调用
2. 验证订阅存在且状态为 Cancelled
3. 从存储中移除订阅记录，L2-R4 防御性清理 Escrow 残留，释放存储空间

### cleanup_ad_commitment (call_index 7)

1. 任何签名账户均可调用
2. 验证广告承诺存在且状态为 Cancelled
3. 从存储中移除承诺记录，释放存储空间

## Events

| 事件 | 字段 | 说明 |
|------|------|------|
| `Subscribed` | `bot_id_hash, tier, owner` | 新订阅已创建 |
| `FreeTierFallback` | `bot_id_hash` | 订阅降级为 Free (Suspended) |
| `SubscriptionDeposited` | `bot_id_hash, amount` | Escrow 已充值 |
| `SubscriptionCancelled` | `bot_id_hash` | 订阅已取消 |
| `TierChanged` | `bot_id_hash, old_tier, new_tier` | 层级已变更 |
| `SubscriptionFeeCollected` | `bot_id_hash, amount` | 订阅费已收取 |
| `SubscriptionCancelledWithProration` | `bot_id_hash, prorated_fee, refunded` | 取消订阅按比例扣费 |
| `AdCommitted` | `bot_id_hash, community_id_hash, committed_ads_per_era, tier` | 广告承诺已创建 |
| `AdCommitmentCancelled` | `bot_id_hash` | 广告承诺已取消 |
| `AdCommitmentFulfilled` | `bot_id_hash, delivered, committed` | 广告承诺达标 |
| `AdCommitmentUnderdelivered` | `bot_id_hash, delivered, committed, consecutive` | 广告承诺未达标 |
| `AdCommitmentDowngraded` | `bot_id_hash` | 广告承诺因连续未达标降级 |
| `SubscriptionCleaned` | `bot_id_hash` | 已取消的付费订阅记录已清理 |
| `AdCommitmentCleaned` | `bot_id_hash` | 已取消的广告承诺记录已清理 |

## Errors

| 错误 | 说明 |
|------|------|
| `BotNotRegistered` | Bot 未注册 |
| `NotBotOwner` | 不是 Bot 所有者 |
| `SubscriptionAlreadyExists` | 订阅已存在 |
| `SubscriptionNotFound` | 订阅不存在 |
| `SubscriptionAlreadyCancelled` | 订阅已取消 |
| `SameTier` | 层级相同 |
| `InsufficientDeposit` | 预存不足 |
| `CannotSubscribeFree` | Free 层级无需订阅 |
| `SubscriptionFeeTransferFailed` | 订阅费转账到国库失败 |
| `NotSubscriptionOwner` | 仅订阅 Owner 可充值 |
| `BotHasNoOperator` | Bot 未分配运营者 |
| `AdCommitmentAlreadyExists` | 广告承诺已存在 |
| `AdCommitmentNotFound` | 广告承诺不存在 |
| `AdCommitmentAlreadyCancelled` | 广告承诺已取消 |
| `CommitmentBelowMinimum` | 承诺广告数未达 Basic 阈值 |
| `ZeroDepositAmount` | 充值金额不能为零 |
| `SubscriptionNotActive` | 订阅未处于活跃状态 |
| `SubscriptionNotTerminal` | 订阅未处于终态 (仅 Cancelled 可清理) |
| `AdCommitmentNotTerminal` | 广告承诺未处于终态 (仅 Cancelled 可清理) |

## Era 结算流程

### settle_era_subscriptions

由 consensus pallet 的 `on_era_end` 通过 `SubscriptionSettler::settle_era()` 调用。

1. 从游标位置遍历 `Subscriptions`，每次最多处理 `MaxSubscriptionSettlePerEra` 条
2. 跳过 Cancelled 订阅
3. Escrow ≥ fee_per_era:
   - Unreserve 费用，尝试 90% 转给 Bot 运营者 + 10% 转给国库
   - 仅双方转账均成功时扣减 Escrow、计入 subscription_income 并发出事件
   - 部分失败时：回收未转出金额 (re-reserve)，仅扣减实际转出额
4. Escrow < fee_per_era:
   - Active → PastDue
   - PastDue → Suspended (发出 FreeTierFallback 事件)
5. 未处理完则设置游标继续下次结算

### settle_ad_commitments

与订阅结算同次调用，使用独立游标分页。

1. 遍历 `AdCommitments`，复用 `MaxSubscriptionSettlePerEra` 限制
2. 跳过 Cancelled 承诺
3. 查询 `AdDelivery::era_delivery_count` 与承诺数比较:
   - **达标**: 重置 `underdelivery_eras = 0`，发出 `AdCommitmentFulfilled`
   - **未达标**: `underdelivery_eras += 1`
     - 超过 `MaxUnderdeliveryEras` → 降级为 Cancelled，发出 `AdCommitmentDowngraded`
     - 否则 → 状态设为 Underdelivery，发出 `AdCommitmentUnderdelivered`
4. 重置社区投放计数

## Trait 实现

### SubscriptionProvider

```rust
fn effective_tier(bot_id_hash: &BotIdHash) -> SubscriptionTier;
fn effective_feature_gate(bot_id_hash: &BotIdHash) -> TierFeatureGate;
```

供外部 pallet (ads, community 等) 查询 Bot 的有效层级和功能限制。

### SubscriptionSettler

```rust
fn settle_era() -> EraSettlementResult;
```

由 consensus pallet 的 `on_era_end` 调用。执行订阅费结算 + 广告承诺检查，返回 `EraSettlementResult { total_income, treasury_share }`。

## 跨 Pallet 依赖

```
consensus ──on_era_end──► subscription (SubscriptionSettler)
subscription ──settle──► BotRegistry (查询 operator)
subscription ──settle──► AdDelivery (查询投放次数)
ads/community ──查询──► subscription (SubscriptionProvider)
```

## 测试

共 49 个单元测试，覆盖:

- 订阅 CRUD (subscribe / deposit / cancel / change_tier)
- 广告承诺 CRUD (commit_ads / cancel_ad_commitment)
- Era 结算 (游标分页、费用扣除、降级逻辑)
- 广告承诺达标/未达标/降级
- 有效层级计算 (付费 vs 广告承诺取高)
- 边界条件 (零金额、重复操作、权限校验)
- 清理 extrinsics (cleanup_subscription / cleanup_ad_commitment)
- 审计回归测试 (C1 cursor fix, H1 event fix, M1 reactivation, H1-R3 underdelivery, M2-R3 cleanup, etc.)

## 审计历史

| 轮次 | 修复项 | 说明 |
|------|--------|------|
| Round 1 | H1 | `settle_era_subscriptions` 转账失败仍发事件 → 移入成功分支 |
| Round 1 | H2 | `change_tier` 无状态检查，已取消/暂停可"复活" → 仅允许 Active/PastDue |
| Round 1 | H3 | `cancel_subscription` 按比例费用转账静默吞错 → 传播错误 |
| Round 1 | H4 | `deposit_subscription` 接受零金额 → ZeroDepositAmount 检查 |
| Round 2 | C1 | 分页 cursor 设为未处理 key，iter_from 跳过一条记录 → break 时不更新 last_key |
| Round 2 | M1 | 任意非零金额即可重新激活 PastDue/Suspended → 仅在 escrow ≥ fee 时激活 |
| Round 2 | L1 | `commit_ads` 不检查 bot_operator → 添加检查 |
| — | M5 | `settle_ad_commitments` 无分页 → 新增 AdCommitmentSettleCursor 游标分页 |
| Round 3 | H1-R3 | `effective_tier` 忽略 Underdelivery 广告承诺，首次未达标即失去层级 → 包含 Underdelivery 状态 |
| Round 3 | M1-R3 | `settle_era_subscriptions` 在转账前扣减 escrow，失败时余额泄漏 → 移至转账成功后，部分失败回收 |
| Round 3 | M2-R3 | 已取消的 Subscriptions/AdCommitments 永不清理 → 新增 cleanup_subscription / cleanup_ad_commitment extrinsics |
| Round 3 | L1-R3 | 死 `SubscriptionCancelled` 事件 (保留以避免 SCALE 索引变更) |
| Round 3 | L2-R3 | Cargo.toml 死 `sp-core` 依赖 → 移除 |
| Round 3 | L3-R3 | `try-runtime` feature 缺 `sp-runtime/try-runtime` → 补全 |

## 相关模块

- [primitives/](../primitives/) — SubscriptionTier、SubscriptionStatus、AdCommitmentStatus、SubscriptionProvider、SubscriptionSettler、EraSettlementResult
- [registry/](../registry/) — Bot 注册（BotRegistryProvider 实现，查询 bot_owner/bot_operator）
- [consensus/](../consensus/) — Era 编排（调用 SubscriptionSettler::settle_era）
- [rewards/](../rewards/) — 奖励分配（消费订阅收入数据）

## License

Apache-2.0
