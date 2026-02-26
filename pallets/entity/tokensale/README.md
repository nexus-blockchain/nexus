# pallet-entity-tokensale

> NEXUS Entity 代币发售模块 — 多模式公开发售、实际资金转账、锁仓解锁、KYC 集成 | Runtime Index: 132

## 概述

`pallet-entity-tokensale` 实现 Entity 组织的代币公开发售（Token Sale / IEO）功能。Entity owner/admin 可配置多轮发售，支持 5 种发售模式、NEX 支付、灵活的锁仓解锁策略、KYC 准入控制和完整的资金托管流。

## 资金流

```
subscribe:      认购者 NEX ──→ Pallet 托管账户
start_sale:     Entity 代币 ──reserve──→ 锁定
claim_tokens:   Entity 代币 ──repatriate──→ 认购者（初始解锁）
unlock_tokens:  Entity 代币 ──repatriate──→ 认购者（后续解锁）
end_sale:       未售代币 ──unreserve──→ Entity 账户
cancel_sale:    未售代币 ──unreserve──→ Entity 账户
claim_refund:   NEX ──→ 认购者 + Entity 代币 ──unreserve
withdraw_funds: NEX ──→ Entity 派生账户
```

## 架构

```
pallet-entity-tokensale (pallet_index = 132)
│
├── 外部依赖
│   ├── EntityProvider       Entity 存在性 / 权限 / 派生账户
│   ├── Currency (NEX)       认购支付 / 退款 / 提取
│   ├── EntityTokenProvider  Entity 代币 reserve / unreserve / repatriate
│   └── KycChecker           KYC 级别查询
│
├── 数据结构
│   ├── SaleRound            轮次主体（模式、供应量、支付、锁仓）
│   ├── Subscription         认购记录（金额、支付、领取、解锁、退款标记）
│   ├── VestingConfig        锁仓策略
│   └── PaymentConfig        支付选项
│
├── 独立存储
│   ├── RoundWhitelist       白名单（DoubleMap，避免大 struct 加载）
│   └── WhitelistCount       白名单计数
│
├── 内部函数
│   ├── calculate_payment_amount    checked_mul 防溢出
│   ├── calculate_dutch_price       线性递减（SaturatedConversion）
│   ├── calculate_initial_unlock    基点计算
│   └── calculate_unlockable        悬崖期 + 线性/阶梯释放
│
└── 查询函数
    ├── pallet_account              托管账户地址
    ├── get_current_price           当前价格
    ├── get_subscription            认购信息
    └── get_unlockable_amount       可解锁数量
```

## 发售模式

| 模式 | 说明 | 价格机制 |
|------|------|----------|
| `FixedPrice` | 固定价格发售（默认） | 恒定价格 |
| `DutchAuction` | 荷兰拍卖 | 从 `start_price` 线性递减到 `end_price` |
| `WhitelistAllocation` | 白名单定向分配 | 固定价格，仅白名单可参与 |
| `FCFS` | 先到先得 | 固定价格，售完即止 |
| `Lottery` | 抽签发售 | 固定价格，随机分配（预留） |

## 发售生命周期

```
                          ┌─ add_payment_option (至少一个)
                          ├─ set_vesting_config
create_sale_round ──→ [NotStarted] ──┤
(Entity owner/admin)      ├─ configure_dutch_auction (荷兰拍卖)
                          └─ add_to_whitelist (白名单模式)
                                │
                         start_sale (锁定 Entity 代币)
                                │
                                ▼
                           [Active] ←── subscribe (NEX → 托管)
                            │     │            │
                     end_sale  cancel_sale   时间窗口校验
                     (释放未售)  (释放未售)    + KYC 校验
                            │         │
                            ▼         ▼
                        [Ended]  [Cancelled]
                            │         │
                     claim_tokens  claim_refund
                     (代币→用户)   (NEX→用户 + 释放代币)
                            │
                     unlock_tokens (悬崖期后)
                            │
                     withdraw_funds (NEX→Entity)
```

### 轮次状态 (RoundStatus)

| 状态 | 说明 |
|------|------|
| `NotStarted` | 已创建，配置阶段 |
| `WhitelistOpen` | 白名单注册中（预留） |
| `Active` | 发售进行中，可认购 |
| `SoldOut` | 已售罄 |
| `Ended` | 创建者手动结束 |
| `Cancelled` | 已取消（认购者可退款） |
| `Settling` | 结算中（预留） |
| `Completed` | 已完成 |

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
    pub funds_withdrawn: bool,                 // v0.2.0 新增
    pub cancelled_at: Option<BlockNumber>,      // v0.3.0 新增
    pub total_refunded_tokens: Balance,         // v0.3.0 新增
    pub total_refunded_nex: Balance,            // v0.3.0 新增
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
    pub last_unlock_at: BlockNumber,
    pub refunded: bool,                        // v0.2.0 新增：是否已退款
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
    type EntityProvider = EntityRegistry;           // Entity 权限/账户
    type TokenProvider = EntityToken;               // Entity 代币操作
    type KycChecker = TokenSaleKycBridge;           // KYC 级别查询
    type MaxPaymentOptions = ConstU32<5>;
    type MaxWhitelistSize = ConstU32<1000>;
    type MaxRoundsHistory = ConstU32<50>;
    type MaxSubscriptionsPerRound = ConstU32<10000>;
}
```

| 参数 | 说明 |
|------|------|
| `Currency` | NEX 货币（认购收款、退款、提取） |
| `EntityProvider` | Entity 存在性/激活状态/权限/派生账户 |
| `TokenProvider` | Entity 代币 reserve/unreserve/repatriate |
| `KycChecker` | KYC 级别查询（0-4） |
| `MaxPaymentOptions` | 每轮最大支付选项数 |
| `MaxWhitelistSize` | 每轮白名单最大容量 |
| `MaxRoundsHistory` | 每 Entity 最大历史轮次数 |
| `MaxSubscriptionsPerRound` | 每轮最大参与者数 |

## 存储项

| 存储 | 类型 | 说明 |
|------|------|------|
| `NextRoundId` | `StorageValue<u64>` | 自增轮次 ID |
| `SaleRounds` | `StorageMap<u64, SaleRound>` | 轮次主表 |
| `EntityRounds` | `StorageMap<u64, BoundedVec<u64>>` | Entity → 轮次索引 |
| `Subscriptions` | `StorageDoubleMap<u64, AccountId, Subscription>` | 认购记录 |
| `RoundParticipants` | `StorageMap<u64, BoundedVec<AccountId>>` | 参与者列表 |
| `RaisedFunds` | `StorageDoubleMap<u64, Option<AssetId>, Balance>` | 已募集金额 |
| `RoundWhitelist` | `StorageDoubleMap<u64, AccountId, bool>` | 白名单（v0.2.0 独立） |
| `WhitelistCount` | `StorageMap<u64, u32>` | 白名单计数 |

## Extrinsics

| # | 调用 | 权限 | 前置状态 | 说明 |
|---|------|------|----------|------|
| 0 | `create_sale_round` | Entity owner/admin | — | 创建轮次（校验 Entity 存在+激活+权限） |
| 1 | `add_payment_option` | 创建者 | NotStarted | 添加支付选项（price > 0, max >= min） |
| 2 | `set_vesting_config` | 创建者 | NotStarted | 锁仓配置（total >= cliff） |
| 3 | `configure_dutch_auction` | 创建者 | NotStarted + DutchAuction | 价格曲线（start > end） |
| 4 | `add_to_whitelist` | 创建者 | NotStarted | 独立存储白名单（BoundedVec 输入） |
| 5 | `start_sale` | 创建者 | NotStarted | 锁定 Entity 代币 + 需 ≥1 支付选项 |
| 6 | `subscribe` | signed | Active | NEX → 托管（时间窗口+KYC+白名单校验） |
| 7 | `end_sale` | 创建者 | Active | 释放未售代币（须 ≥ end_block 或已售罄） |
| 8 | `claim_tokens` | 认购者 | Ended | Entity 代币 → 用户（初始解锁） |
| 9 | `unlock_tokens` | 认购者 | 已 claimed | Entity 代币 → 用户（后续解锁） |
| 10 | `cancel_sale` | 创建者 | NotStarted/Active | 释放未售代币 → Cancelled |
| 11 | `claim_refund` | 认购者 | Cancelled | NEX 退还 + 释放对应代币 |
| 12 | `withdraw_funds` | 创建者 | Ended/Completed | NEX → Entity 派生账户 |

### subscribe 详细流程

1. 验证轮次状态为 `Active`
2. 验证 `remaining_amount >= amount`
3. **时间窗口校验**：`now ∈ [start_block, end_block]`
4. 验证未重复认购
5. **KYC 校验**：`kyc_required` 时检查 `KycChecker::kyc_level >= min_kyc_level`
6. **白名单校验**：WhitelistAllocation 模式查 `RoundWhitelist` 存储
7. 查找匹配支付选项
8. 验证购买量 `[min_purchase, max_purchase_per_account]`
9. **checked_mul** 计算支付金额（溢出返回 ArithmeticOverflow）
10. **Currency::transfer** 从用户到 Pallet 托管账户
11. 创建认购记录，更新轮次数据，更新募集统计

### 荷兰拍卖价格

```
current_price = start_price - (start_price - end_price) × elapsed / total_duration
```

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

## Errors

| 错误 | 说明 |
|------|------|
| `EntityNotFound` | Entity 不存在 |
| `EntityNotActive` | Entity 未激活 |
| `Unauthorized` | 非 Entity owner/admin |
| `InvalidTotalSupply` | total_supply = 0 |
| `InvalidTimeWindow` | end_block <= start_block |
| `InvalidKycLevel` | min_kyc_level > 4 |
| `InvalidPrice` | price = 0 或 min_purchase = 0 |
| `InvalidPurchaseLimits` | max < min |
| `InvalidVestingDuration` | cliff > total |
| `InvalidVestingConfig` | initial_unlock_bps > 10000 |
| `InvalidDutchAuctionConfig` | start_price <= end_price |
| `NoPaymentOptions` | start_sale 时无支付选项 |
| `InsufficientTokenSupply` | Entity 代币余额不足 |
| `SaleNotInTimeWindow` | 认购时不在 [start, end]；或提前结束发售 |
| `InsufficientKycLevel` | KYC 级别 < min_kyc_level |
| `NotInWhitelist` | 白名单模式下不在名单 |
| `ArithmeticOverflow` | amount × price 溢出 u128 |
| `SoldOut` | 剩余不足 |
| `AlreadySubscribed` | 重复认购 |
| `AlreadyClaimed` | 重复领取 |
| `AlreadyRefunded` | 重复退款 |
| `SaleNotCancelled` | 退款需 Cancelled 状态 |
| `FundsAlreadyWithdrawn` | 重复提取 |

## 权限模型

| 操作 | 调用方 | 前置条件 |
|------|--------|----------|
| `create_sale_round` | Entity owner/admin | Entity 存在且激活 |
| `add_payment_option` | 轮次创建者 | NotStarted |
| `set_vesting_config` | 轮次创建者 | NotStarted |
| `configure_dutch_auction` | 轮次创建者 | NotStarted + DutchAuction |
| `add_to_whitelist` | 轮次创建者 | NotStarted |
| `start_sale` | 轮次创建者 | NotStarted + ≥1 支付选项 |
| `subscribe` | signed | Active + 时间窗口 + KYC + 白名单 |
| `end_sale` | 轮次创建者 | Active + (now ≥ end_block 或 已售罄) |
| `claim_tokens` | 认购者 | Ended + 未 claimed |
| `unlock_tokens` | 认购者 | 已 claimed + 悬崖期后 |
| `cancel_sale` | 轮次创建者 | NotStarted/Active |
| `claim_refund` | 认购者 | Cancelled + 未退款 |
| `withdraw_funds` | 轮次创建者 | Ended/Completed + 未提取 |

## 测试

```bash
cargo test -p pallet-entity-tokensale
# 49 tests passed
```

| 测试 | 覆盖 |
|------|------|
| `create_sale_round_works` | 创建轮次、状态、funds_withdrawn |
| `create_sale_round_rejects_invalid_entity` | Entity 不存在 |
| `create_sale_round_rejects_non_owner` | 非 owner/admin |
| `create_sale_round_rejects_zero_supply` | total_supply = 0 |
| `create_sale_round_rejects_bad_time_window` | end <= start |
| `create_sale_round_rejects_invalid_kyc_level` | level > 4 |
| `add_payment_option_works` | 添加支付选项 |
| `add_payment_option_rejects_zero_price` | price = 0 |
| `add_payment_option_rejects_bad_limits` | max < min |
| `set_vesting_config_works` | 锁仓配置 |
| `set_vesting_config_rejects_cliff_gt_total` | cliff > total |
| `configure_dutch_auction_requires_not_started` | 荷兰拍卖配置 |
| `whitelist_uses_separate_storage` | 独立白名单存储+去重 |
| `whitelist_rejects_non_not_started` | Active 状态不可添加 |
| `start_sale_requires_payment_options` | 无选项时拒绝 |
| `start_sale_locks_entity_tokens` | 锁定代币 |
| `subscribe_transfers_nex` | NEX 实际转账到托管 |
| `subscribe_rejects_outside_time_window` | 时间窗口校验 |
| `subscribe_rejects_duplicate` | 重复认购 |
| `subscribe_checks_kyc` | KYC 级别检查 |
| `subscribe_checks_whitelist` | 白名单模式 |
| `end_sale_releases_unsold_tokens` | 释放未售代币 |
| `claim_tokens_distributes_entity_tokens` | 代币分发 |
| `claim_tokens_rejects_double_claim` | 重复领取 |
| `cancel_and_refund_works` | 取消+退款全流程 |
| `claim_refund_rejects_non_cancelled` | 非 Cancelled 退款 |
| `withdraw_funds_works` | NEX 提取到 Entity |
| `calculate_initial_unlock_works` | 20% 初始解锁 |
| `calculate_initial_unlock_no_vesting_returns_total` | 无锁仓全额 |
| `subscribe_rejects_overflow` | checked_mul 溢出保护 |
| `end_sale_rejects_premature_end` | 提前结束发售被拒绝 |
| `end_sale_allows_when_sold_out` | 售罄时可提前结束 |
| `end_sale_allows_after_end_block` | 超过 end_block 后正常结束 |
| `cliff_vesting_unlock_interval_step_function` | Cliff 阶梯解锁各阶段验证 |
| `linear_vesting_continuous_unlock` | Linear 连续线性不受 interval 影响 |
| `on_initialize_auto_ends_expired_sale` | on_initialize 自动结束过期发售 |
| `on_initialize_does_not_end_before_expiry` | 未过期不结束 |
| `on_initialize_handles_multiple_rounds` | 多轮次部分过期处理 |
| `payment_options_stored_separately` | 支付选项独立存储验证 |
| `reclaim_unclaimed_tokens_after_grace_period` | 退款宽限期回收流程 |
| `reclaim_rejects_non_creator` | 非创建者不能回收 |
| `dutch_auction_allows_zero_price_in_payment_option` | 荷兰拍卖允许 price=0 |
| `non_dutch_rejects_zero_price` | 非荷兰拍卖拒绝 price=0 |
| `dutch_auction_start_requires_configure` | 荷兰拍卖需先配置 |
| `c1_reclaim_blocks_subsequent_withdraw` | C1: reclaim 后阻止 withdraw 双重提取 |
| `h2_claim_tokens_rejects_completed_from_cancel` | H2: Completed 状态拒绝 claim_tokens |
| `h3_add_payment_option_rejects_non_none_asset_id` | H3: 拒绝非 None asset_id |

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-02-03 | Phase 8 初始版本 |
| v0.1.1 | 2026-02-09 | 模块文件夹从 `sale` 重命名为 `tokensale`，更新 README |
| v0.2.0 | 2026-02-09 | 深度审计修复（20 项），详见下方 |
| v0.3.0 | 2026-02-23 | 二次深度审计（4 项修复 + 5 新测试），详见下方 |
| v0.4.0 | 2026-02-26 | 三次审计（3 项修复 + 3 新测试），详见下方 |

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

- **C1**: `add_to_whitelist` 参数从 `Vec<T::AccountId>` 改为 `BoundedVec<T::AccountId, T::MaxWhitelistSize>`，防止无界输入 DoS
- **H1**: `subscribe` 在 NEX 转账前预检 `RoundParticipants` 容量（fail-fast），避免浪费计算
- **H2**: `end_sale` 新增时间窗口强制检查：`now >= end_block || remaining_amount == 0`，防止创建者提前截止损害参与者权益
- **M1**: `calculate_unlockable` 实现 `VestingType::Cliff` 的 `unlock_interval` 阶梯解锁（原实现忽略 interval，全部按线性处理）
- 新增 5 个测试覆盖上述修复
- 已有 4 个测试适配 H2 变更（end_sale 前需推进到 end_block）

### v0.4.0 三次审计（3 项修复 + 3 新测试）

- **C1**: `reclaim_unclaimed_tokens` 未设置 `funds_withdrawn = true` → 允许 `withdraw_funds` 双重提取，跨轮资金污染。修复: reclaim 时同步标记 `funds_withdrawn = true`
- **H2**: `claim_tokens` 允许 `Completed` 状态（来自 cancel→reclaim 路径，代币已 unreserve）→ 认购者拿不到代币但状态标记已领取。修复: 仅允许 `Ended` 状态
- **H3**: `add_payment_option` 允许 `asset_id = Some(x)` → `RaisedFunds` 键不一致，`withdraw_funds` 只读 `None` 键导致资金永久锁定。修复: 强制 `asset_id = None`
- **M4**: Mock `repatriate_reserved` 返回值语义与真实 pallet 不一致（返回差额 vs 实际量）。修复: 改为返回 `Ok(actual)`
- 新增 3 个回归测试

**已知设计局限（标记未修）：**
- L1: `EntityTokenProvider::repatriate_reserved` 返回值语义（实际量 vs 差额）与 Substrate `ReservableCurrency` 惯例不同，跨 pallet 影响范围较广（market 等也使用），暂不修改
- L2: `RoundStatus` 枚举中 `WhitelistOpen`、`SoldOut`、`Settling` 三个值为预留，从未使用

## 许可证

MIT License
