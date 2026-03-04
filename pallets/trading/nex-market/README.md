# pallet-nex-market

NEX/USDT P2P 订单簿交易市场 — 无做市商模型，任何人可挂单/吃单。

## 概述

`pallet-nex-market` 是 Nexus 链上唯一的 NEX ↔ USDT 交易市场，同时也是全链的价格预言机数据源。

**交易对**: NEX（链上原生代币，精度 10^12）↔ USDT（TRC20 链下支付，精度 10^6）

| 特性 | 说明 |
|------|------|
| **订单簿** | 限价卖单 + 限价买单，支持部分成交 |
| **USDT 通道** | TRC20 链下支付 + OCW 三阶段自动验证 |
| **多档判定** | Exact / Overpaid / Underpaid / SeverelyUnderpaid / Invalid |
| **买家保证金** | 按 USDT 金额计算 NEX 保证金，防不付款风险 |
| **补付窗口** | 少付 50%~99.5% 给予 2h 补付时间，避免网络延迟误判 |
| **TWAP 预言机** | 1h / 24h / 7d 三周期时间加权平均价，`on_idle` 每区块推进 |
| **价格保护** | 挂单/吃单偏离检查 + 7d TWAP 熔断机制 |
| **seed_liquidity** | 瀑布式定价 + 溢价 + 四层防御，冷启动引流 |
| **OCW 验证奖励** | 0.1 NEX/次，激励任何人触发结算 |

---

## 交易流程

### 卖 NEX（卖家锁 NEX，收 USDT）

```text
卖家                         买家                          OCW
  │                           │                             │
  │ place_sell_order           │                             │
  │ ──→ 锁 NEX + 提供 TRON 地址                              │
  │                           │                             │
  │                  reserve_sell_order                      │
  │                  ──→ 提供买家 TRON 地址                   │
  │                  ──→ 锁保证金（或免保证金）                │
  │                  ──→ 创建 UsdtTrade (AwaitingPayment)     │
  │                           │                             │
  │                  链下转 USDT → confirm_payment            │
  │                  ──→ 状态 → AwaitingVerification          │
  │                           │                             │
  │                           │          TronGrid 验证       │
  │                           │     ←── submit_ocw_result    │
  │                           │                             │
  │                  claim_verification_reward               │
  │                  ──→ 释放 NEX 给买家                     │
  │                  ──→ 退还保证金                          │
  │                  ──→ 支付 0.1 NEX 验证奖励               │
```

### 买 NEX（买家挂单，卖家接单）

```text
买家                         卖家                          OCW
  │                           │                             │
  │ place_buy_order            │                             │
  │ ──→ 提供买家 TRON 地址      │                             │
  │ ──→ 预锁保证金             │                             │
  │                           │                             │
  │                  accept_buy_order                        │
  │                  ──→ 提供卖家 TRON 地址                   │
  │                  ──→ 锁卖家 NEX                          │
  │                  ──→ 按比例分配预锁保证金                  │
  │                  ──→ 创建 UsdtTrade (AwaitingPayment)     │
  │                           │                             │
  │ 链下转 USDT → confirm_payment                            │
  │                           │      ←── submit_ocw_result   │
  │                  claim_verification_reward               │
```

### 少付补付流程（50% ~ 99.5%）

```text
OCW 检测到少付
  │
  ▼
submit_ocw_result → UnderpaidPending → 开启补付窗口 (2h)
  │
  ├─ 窗口内 OCW 持续扫描 TronGrid
  │   └─ submit_underpaid_update（仅接受递增金额）
  │       ├─ 累计 ≥ 99.5% → 自动升级为 Exact，回到正常结算
  │       └─ 累计仍 < 99.5% → 等待窗口到期
  │
  └─ 窗口到期 → finalize_underpaid（终裁）
      └─ 按最终比例释放 NEX + 梯度没收保证金
```

### OCW 预检兜底

```text
AwaitingPayment 超过 50% 超时期
  │
  ▼
OCW 预检扫描 → 检测到 USDT 已到账但买家忘记 confirm
  │
  ▼
auto_confirm_payment → 一步完成：确认付款 + 存储验证结果
  ├─ Exact/Overpaid → AwaitingVerification
  ├─ Underpaid → UnderpaidPending（进入补付窗口）
  └─ SeverelyUnderpaid/Invalid → AwaitingVerification（等待 claim 终裁）
```

---

## Extrinsics

### 用户操作

| # | 调用 | 权限 | 参数 | 说明 |
|---|------|------|------|------|
| 0 | `place_sell_order` | 签名 | `nex_amount`, `usdt_price`, `tron_address` | 挂卖单：reserve 锁定 NEX，校验 TRON 地址（Base58Check），校验价格偏离 |
| 1 | `place_buy_order` | 签名 | `nex_amount`, `usdt_price`, `buyer_tron_address` | 挂买单：预锁保证金，校验 TRON 地址，校验价格偏离 |
| 2 | `cancel_order` | Owner | `order_id` | 取消订单：退还未成交的锁定 NEX（卖单）或剩余保证金（买单） |
| 3 | `reserve_sell_order` | 签名 | `order_id`, `amount?`, `buyer_tron_address` | 买家吃卖单：校验过期/价格偏离/熔断，锁保证金或免保证金（L2/L3 检查），创建 UsdtTrade |
| 4 | `accept_buy_order` | 签名 | `order_id`, `amount?`, `tron_address` | 卖家接买单：校验过期/价格偏离/熔断，锁卖家 NEX，按比例分配预锁保证金，创建 UsdtTrade |
| 5 | `confirm_payment` | 买家 | `trade_id` | 确认已付款：从 AwaitingPayment → AwaitingVerification，加入 OCW 待验证队列 |
| 6 | `process_timeout` | 任何人 | `trade_id` | 分阶段超时处理（详见下方） |
| 8 | `claim_verification_reward` | 任何人 | `trade_id` | 领取验证奖励 + 根据 OCW 结果结算（释放 NEX / 退保证金 / 没收保证金） |
| 17 | `finalize_underpaid` | 任何人 | `trade_id` | 补付窗口到期后终裁：按最终比例释放 NEX + 梯度没收保证金 |

### OCW 操作（Unsigned）

| # | 调用 | 参数 | 说明 |
|---|------|------|------|
| 7 | `submit_ocw_result` | `trade_id`, `actual_amount` | 正常验证结果：Exact/Overpaid 存储、Underpaid 进补付窗口、SeverelyUnderpaid/Invalid 直接存储 |
| 15 | `auto_confirm_payment` | `trade_id`, `actual_amount` | 预检兜底：一步完成确认 + 验证，少付进补付窗口 |
| 16 | `submit_underpaid_update` | `trade_id`, `new_actual_amount` | 补付窗口内更新累计金额（仅接受递增，达 99.5% 自动升级回 Exact） |

> 三个 unsigned extrinsic 均拒绝 `TransactionSource::External`，仅接受 Local / InBlock 来源，防止外部用户注入伪造结果。

### 管理操作（MarketAdmin = TreasuryCouncil 2/3）

| # | 调用 | 参数 | 说明 |
|---|------|------|------|
| 9 | `configure_price_protection` | `enabled`, `max_deviation`, `cb_threshold`, `min_trades` | 配置价格保护（偏离上限、熔断阈值、TWAP 最小成交数） |
| 10 | `set_initial_price` | `initial_price` | 设置初始基准价格（冷启动，初始化 TWAP 累积器 + LastTradePrice） |
| 11 | `lift_circuit_breaker` | — | 手动解除已到期的熔断 |
| 13 | `fund_seed_account` | `amount` | 国库 → 种子账户转账（补充 seed_liquidity 资金） |
| 14 | `seed_liquidity` | `order_count`, `usdt_override?` | 批量挂免保证金卖单（瀑布式定价 + 溢价） |

---

## process_timeout 分阶段策略

| 交易状态 | 超时条件 | 处理 |
|----------|---------|------|
| **AwaitingPayment** | `now > timeout_at` | 退还卖家 NEX，没收买家保证金（DepositForfeitRate），回滚订单 |
| **AwaitingVerification** | `now > timeout_at + VerificationGracePeriod` | 先检查 OCW 结果：有结果按正常结算，无结果退款 + 没收 + 发出 `VerificationTimeoutRefunded` 事件供链下仲裁 |
| **UnderpaidPending** | `now > underpaid_deadline` | 读取最终 OCW 金额做终裁（等效 `finalize_underpaid`），无结果走通用超时退款 |

---

## 多档判定 & 保证金没收

### 付款金额判定

| 实际 / 应付比例 | 判定结果 | NEX 释放 | 保证金 |
|----------------|----------|----------|--------|
| ≥ 100.5% | Overpaid | 全额释放 | 退还 |
| 99.5% ~ 100.5% | Exact | 全额释放 | 退还 |
| 50% ~ 99.5% | Underpaid | 进入补付窗口(2h) | 待定 |
| < 50% | SeverelyUnderpaid | 按比例释放 | 没收 |
| = 0 | Invalid | 不释放（NEX 退还卖家） | 没收 |

### 保证金梯度没收（补付终裁 / 少付结算）

| 最终付款比例 | 没收比例 |
|-------------|---------|
| ≥ 99.5% | 0% |
| 95% ~ 99.5% | 20% |
| 80% ~ 95% | 50% |
| < 80% | 100% |

### 少付 NEX 释放计算

```text
nex_to_release = nex_amount × payment_ratio / 10000
nex_to_refund  = nex_amount - nex_to_release  (退还卖家)
```

> 比例计算使用 `compute_payment_ratio_bps()` 返回 u32，防止超付 >6.55 倍时 u16 截断导致误判。

---

## OCW 三阶段工作流

### Hooks

- **`on_idle`**：每区块推进 TWAP 累积器（用 `last_price` 填充空白区间）
- **`offchain_worker`**：执行链下三阶段验证

### 三阶段验证

| 阶段 | 扫描队列 | 触发条件 | API 调用 | 链上动作 |
|------|---------|---------|---------|---------|
| 1. 正常验证 | `PendingUsdtTrades` | AwaitingVerification | `verify_trc20_by_transfer` (回溯 24h) | → offchain storage → sidecar → `submit_ocw_result` |
| 2. 补付扫描 | `PendingUnderpaidTrades` | UnderpaidPending | `verify_trc20_by_transfer` (回溯 48h) | → offchain storage → sidecar → `submit_underpaid_update` |
| 3. 预检兜底 | `AwaitingPaymentTrades` | AwaitingPayment + 超 50% 超时期 | `verify_trc20_by_transfer` (回溯 24h) | → offchain storage → sidecar → `auto_confirm_payment` |

### OCW ↔ Sidecar 通信

OCW 将验证结果写入 offchain local storage（PERSISTENT），外部 sidecar 服务读取后提交 unsigned 交易：

| 存储键前缀 | 值 | 对应 extrinsic |
|-----------|------|---------------|
| `nex_market_ocw::{trade_id}` | `(bool, u64)` | `submit_ocw_result` |
| `nex_market_auto::{trade_id}` | `(bool, u64)` | `auto_confirm_payment` |
| `nex_market_undp::{trade_id}` | `(bool, u64)` | `submit_underpaid_update` |

---

## TWAP 预言机

### 累积器机制

每笔成交调用 `on_trade_completed(trade_price)` 更新累积器：

1. **异常价格过滤**：偏离 > 100% 时钳制到 ±50%（`last_price × 1.5` 或 `last_price / 2`）
2. **累积价格推进**：`cumulative += last_price × blocks_elapsed`
3. **快照推进**：按时间间隔更新 hour/day/week snapshot

### 快照推进间隔

| 快照 | 推进间隔 | 说明 |
|------|---------|------|
| `hour_snapshot` | `BlocksPerHour / 6` | ~10min |
| `day_snapshot` | `BlocksPerHour` | ~1h |
| `week_snapshot` | `BlocksPerDay` | ~24h |

### TWAP 计算

```text
twap = (current_cumulative - snapshot.cumulative_price) / (current_block - snapshot.block_number)
```

### 对外接口

本模块实现 `pallet_trading_common::PriceOracle` trait：

| 方法 | 说明 |
|------|------|
| `get_twap(window)` | 获取 1h/24h/7d TWAP（精度 10^6） |
| `get_last_trade_price()` | 获取最新成交价 |
| `is_price_stale(max_age)` | 价格是否超过指定区块数未更新 |
| `get_trade_count()` | 累计交易数 |

---

## 价格保护

### 偏离检查（挂单 + 吃单）

```text
check_price_deviation(usdt_price):
  1. 检查熔断是否激活 → MarketCircuitBreakerActive
  2. 获取参考价：TWAP 数据充足 → 1h TWAP，否则 → initial_price
  3. 计算偏离 bps = |price - ref_price| × 10000 / ref_price
  4. 偏离 > max_price_deviation → PriceDeviationTooHigh
```

### 熔断机制

每笔成交后检查 `check_circuit_breaker(trade_price)`：
- 条件：`|trade_price - 7d TWAP| / 7d TWAP > circuit_breaker_threshold`
- 触发：暂停所有交易 `CircuitBreakerDuration` 个区块
- 解除：自动到期 或 `lift_circuit_breaker`（MarketAdmin 手动解除已到期的熔断）

### TWAP 数据充足性判定

需同时满足：
- `trade_count ≥ min_trades_for_twap`
- 三个快照间距各自 ≥ 对应周期（`BlocksPerHour` / `BlocksPerDay` / `BlocksPerWeek`）

---

## seed_liquidity 流动性引导

### 定价策略（保护性瀑布式）

```text
ref_price = get_seed_reference_price()
seed_price = ref_price × (1 + SeedPricePremiumBps / 10000)
nex_per_order = usdt_amount × 10^12 / seed_price
```

| 市场阶段 | 条件 | 基准价来源 | 保护方向 |
|----------|------|-----------|---------|
| 成熟期 | `trade_count ≥ min_trades_for_twap` | 7d TWAP | 真实市场价 |
| 过渡期 | `trade_count ≥ 30` | `max(24h TWAP, InitialPrice)` | 只涨不跌 |
| 冷启动 | `trade_count < 30` | InitialPrice | 人为兜底 |

### 四层防御

| 层级 | 机制 | 参数 |
|------|------|------|
| **L0 定价** | 瀑布式基准价 + 溢价下限 | 20% 溢价 |
| **L1 资金隔离** | 独立种子账户（`nxm/seed`），需 `fund_seed_account` 显式注资 | — |
| **L2 防 Grief** | 单笔上限 + 每账户仅 1 笔活跃免保证金 + 短超时 | 100 NEX / 1h |
| **L3 防 Sybil** | 完成首单后标记 `CompletedBuyers`，不再免保证金 | — |
| **审计** | `CumulativeSeedUsdtSold` 链上累计对账 | — |

---

## 数据结构

### Order

| 字段 | 类型 | 说明 |
|------|------|------|
| `order_id` | `u64` | 订单 ID |
| `maker` | `AccountId` | 挂单者 |
| `side` | `OrderSide` | `Buy` / `Sell` |
| `nex_amount` | `Balance` | NEX 数量（精度 10^12） |
| `filled_amount` | `Balance` | 已成交 NEX 数量 |
| `usdt_price` | `u64` | 每 NEX 的 USDT 单价（精度 10^6） |
| `tron_address` | `Option<TronAddress>` | 卖单 = 卖家收款地址，买单 = 买家付款地址 |
| `status` | `OrderStatus` | `Open` / `PartiallyFilled` / `Filled` / `Cancelled` |
| `created_at` | `BlockNumber` | 创建区块 |
| `expires_at` | `BlockNumber` | 过期区块（DefaultOrderTTL） |
| `buyer_deposit` | `Balance` | 预锁保证金（仅买单，卖单为 0） |
| `deposit_waived` | `bool` | 是否免保证金（仅 seed_liquidity 卖单） |

### UsdtTrade

| 字段 | 类型 | 说明 |
|------|------|------|
| `trade_id` | `u64` | 交易 ID |
| `order_id` | `u64` | 关联订单 |
| `seller` / `buyer` | `AccountId` | 卖方（收 USDT）/ 买方（付 USDT） |
| `nex_amount` | `Balance` | NEX 数量 |
| `usdt_amount` | `u64` | 应付 USDT 金额（精度 10^6） |
| `seller_tron_address` | `TronAddress` | 卖家 TRON 收款地址 |
| `buyer_tron_address` | `Option<TronAddress>` | 买家 TRON 付款地址（OCW 按 from/to/amount 匹配） |
| `status` | `UsdtTradeStatus` | `AwaitingPayment` → `AwaitingVerification` → `UnderpaidPending` → `Completed` / `Refunded` |
| `created_at` | `BlockNumber` | 创建区块 |
| `timeout_at` | `BlockNumber` | 超时区块（普通=UsdtTimeout，免保证金=FirstOrderTimeout） |
| `buyer_deposit` | `Balance` | 买家保证金 |
| `deposit_status` | `BuyerDepositStatus` | `None` / `Locked` / `Released` / `Forfeited` |
| `first_verified_at` | `Option<BlockNumber>` | 首次 OCW 验证区块（少付场景） |
| `first_actual_amount` | `Option<u64>` | 首次检测金额 |
| `underpaid_deadline` | `Option<BlockNumber>` | 补付截止区块 |

### 辅助结构

| 类型 | 字段 | 说明 |
|------|------|------|
| `MarketStats` | `total_orders`, `total_trades`, `total_volume_usdt` | 累计市场统计 |
| `TwapAccumulator` | `current_cumulative`, `current_block`, `last_price`, `trade_count`, `{hour,day,week}_snapshot`, `last_{hour,day,week}_update` | TWAP 累积器 |
| `PriceSnapshot` | `cumulative_price` (u128), `block_number` (u32) | 价格快照 |
| `PriceProtectionConfig` | `enabled`, `max_price_deviation`, `circuit_breaker_threshold`, `min_trades_for_twap`, `circuit_breaker_active`, `circuit_breaker_until`, `initial_price` | 价格保护配置（默认 20% 偏离 / 50% 熔断 / 100 笔启用 TWAP） |

---

## 存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextOrderId` | `ValueQuery<u64>` | 订单 ID 计数器 |
| `Orders` | `Map<u64, Order>` | 订单映射 |
| `SellOrders` | `BoundedVec<u64, 1000>` | 卖单索引 |
| `BuyOrders` | `BoundedVec<u64, 1000>` | 买单索引 |
| `UserOrders` | `Map<AccountId, BoundedVec<u64, 100>>` | 用户订单索引 |
| `NextUsdtTradeId` | `ValueQuery<u64>` | USDT 交易 ID 计数器 |
| `UsdtTrades` | `Map<u64, UsdtTrade>` | USDT 交易映射 |
| `PendingUsdtTrades` | `BoundedVec<u64, 100>` | OCW 待验证队列（AwaitingVerification） |
| `AwaitingPaymentTrades` | `BoundedVec<u64, 100>` | 待付款跟踪队列（OCW 预检用） |
| `PendingUnderpaidTrades` | `BoundedVec<u64, 100>` | 少付补付跟踪队列 |
| `OcwVerificationResults` | `Map<u64, (PaymentVerificationResult, u64)>` | OCW 验证结果 |
| `BestAsk` / `BestBid` | `Option<u64>` | 最优卖价 / 买价 |
| `LastTradePrice` | `Option<u64>` | 最新成交价 |
| `MarketStatsStore` | `MarketStats` | 累计市场统计 |
| `TwapAccumulatorStore` | `Option<TwapAccumulator>` | TWAP 累积器 |
| `PriceProtectionStore` | `Option<PriceProtectionConfig>` | 价格保护 + 熔断配置 |
| `CompletedBuyers` | `Map<AccountId, bool>` | 已完成首单的买家（L3 Sybil 防御） |
| `ActiveWaivedTrades` | `Map<AccountId, u32>` | 活跃免保证金交易数（L2 防 grief） |
| `CumulativeSeedUsdtSold` | `ValueQuery<u64>` | seed 累计成交 USDT 总额（审计） |

---

## Events

| 事件 | 字段 | 触发场景 |
|------|------|---------|
| `OrderCreated` | `order_id`, `maker`, `side`, `nex_amount`, `usdt_price` | place_sell_order / place_buy_order / seed_liquidity |
| `OrderCancelled` | `order_id` | cancel_order |
| `UsdtTradeCreated` | `trade_id`, `order_id`, `seller`, `buyer`, `nex_amount`, `usdt_amount` | reserve_sell_order / accept_buy_order |
| `UsdtPaymentSubmitted` | `trade_id` | confirm_payment |
| `UsdtTradeCompleted` | `trade_id`, `order_id` | 全额付款结算 |
| `UsdtTradeVerificationFailed` | `trade_id`, `reason` | OCW 验证失败 |
| `UsdtTradeRefunded` | `trade_id` | AwaitingPayment 超时退款 |
| `VerificationTimeoutRefunded` | `trade_id`, `buyer`, `seller`, `usdt_amount` | AwaitingVerification 宽限期后超时退款 |
| `TwapUpdated` | `new_price`, `twap_1h`, `twap_24h`, `twap_7d` | 成交后 TWAP 更新 |
| `CircuitBreakerTriggered` | `current_price`, `twap_7d`, `deviation_bps`, `until_block` | 价格偏离触发熔断 |
| `CircuitBreakerLifted` | — | 熔断解除 |
| `PriceProtectionConfigured` | `enabled`, `max_deviation` | configure_price_protection |
| `InitialPriceSet` | `initial_price` | set_initial_price |
| `OcwResultSubmitted` | `trade_id`, `verification_result`, `actual_amount` | submit_ocw_result（Exact/Overpaid/SeverelyUnderpaid/Invalid） |
| `UnderpaidDetected` | `trade_id`, `expected_amount`, `actual_amount`, `payment_ratio`, `deadline` | 少付进入补付窗口 |
| `UnderpaidAmountUpdated` | `trade_id`, `previous_amount`, `new_amount` | submit_underpaid_update |
| `UnderpaidFinalized` | `trade_id`, `final_amount`, `payment_ratio`, `deposit_forfeit_rate` | finalize_underpaid |
| `UnderpaidAutoProcessed` | `trade_id`, `expected_amount`, `actual_amount`, `payment_ratio`, `nex_released`, `deposit_forfeited` | 少付结算（process_underpaid） |
| `BuyerDepositLocked` | `trade_id`, `buyer`, `deposit` | reserve_sell_order / accept_buy_order |
| `BuyerDepositReleased` | `trade_id`, `buyer`, `deposit` | 全额结算退还 |
| `BuyerDepositForfeited` | `trade_id`, `buyer`, `forfeited`, `to_treasury` | 超时/少付没收（转入国库） |
| `VerificationRewardClaimed` | `trade_id`, `claimer`, `reward`, `reward_paid` | claim_verification_reward |
| `LiquiditySeeded` | `order_count`, `total_nex`, `source` | seed_liquidity |
| `SeedAccountFunded` | `amount`, `treasury`, `seed_account` | fund_seed_account |
| `WaivedDepositTradeCreated` | `trade_id`, `buyer`, `nex_amount` | 免保证金交易创建 |
| `AutoPaymentDetected` | `trade_id`, `actual_amount` | auto_confirm_payment |

---

## Errors

| 错误 | 说明 |
|------|------|
| `OrderNotFound` | 订单不存在 |
| `NotOrderOwner` | 不是订单所有者 |
| `OrderClosed` | 订单已关闭（非 Open/PartiallyFilled） |
| `OrderExpired` | 订单已过期 |
| `InsufficientBalance` | NEX 余额不足 |
| `AmountTooSmall` | 数量为零或过小 |
| `AmountExceedsAvailable` | 数量超过剩余可用 |
| `ZeroPrice` | USDT 单价为零 |
| `OrderBookFull` | 订单簿已满（卖/买各 1000 上限） |
| `UserOrdersFull` | 用户订单数已满（100 上限） |
| `CannotTakeOwnOrder` | 不能吃自己的单 |
| `ArithmeticOverflow` | 算术溢出 |
| `OrderSideMismatch` | 订单方向不匹配 |
| `InvalidTronAddress` | TRON 地址校验失败 |
| `UsdtTradeNotFound` | USDT 交易不存在 |
| `NotTradeParticipant` | 不是交易参与者 |
| `InvalidTradeStatus` | 当前状态不允许该操作 |
| `TradeTimeout` | 交易已超时 |
| `PendingQueueFull` | 待验证队列已满（100） |
| `AwaitingPaymentQueueFull` | 待付款队列已满（100） |
| `UnderpaidQueueFull` | 补付队列已满（100） |
| `PriceDeviationTooHigh` | 价格偏离超过阈值 |
| `MarketCircuitBreakerActive` | 市场熔断中 |
| `OcwResultNotFound` | OCW 验证结果不存在 |
| `InsufficientDepositBalance` | 保证金余额不足 |
| `InvalidBasisPoints` | 基点参数超过 10000 |
| `FirstOrderLimitReached` | 免保证金交易：每账户仅 1 笔活跃 |
| `FirstOrderAmountTooLarge` | 免保证金交易：金额超上限 |
| `BuyerAlreadyCompleted` | 买家已完成过交易，不再免保证金 |
| `DepositNotWaived` | 订单不支持免保证金 |
| `TooManySeedOrders` | seed_liquidity 单次订单数超限 |
| `NoPriceReference` | 无可用基准价格（需先 set_initial_price） |
| `StillInGracePeriod` | 仍在验证宽限期内 |
| `UnderpaidGraceNotExpired` | 补付窗口尚未到期 |
| `NotUnderpaidPending` | 交易不在 UnderpaidPending 状态 |
| `CircuitBreakerNotActive` | 熔断未激活，无需解除 |
| `OrderIndexRestoreFailed` | 回滚后订单簿满，无法恢复索引 |

---

## 对外价格服务

本模块是全链唯一的 NEX/USDT 价格数据源，通过三层 Trait 向外部模块提供价格：

```text
┌──────────────────────────────────────────────┐
│       ExchangeRateProvider (高级封装)          │
│  get_nex_usdt_rate() + price_confidence()     │
│  is_rate_reliable() (默认阈值 30)             │
├──────────────┬───────────────────────────────┤
│ PricingProvider │      PriceOracle             │
│ (底层汇率查询)  │ (底层 TWAP 预言机)            │
│ get_cos_to_usd  │ get_twap / get_last_trade    │
│ report_p2p_trade│ is_price_stale / trade_count  │
└──────────────┴───────────────────────────────┘
                       │
           ┌───────────┴───────────┐
           │   pallet-nex-market    │
           │  TWAP 累积器 + 订单簿   │
           └───────────────────────┘
```

### 价格优先级

```text
1h TWAP → LastTradePrice → initial_price（治理设定）
```

### Runtime 适配器

| 适配器 | 接口 | 价格策略 | 消费方 |
|--------|------|---------|--------|
| `TradingPricingProvider` | `PricingProvider<Balance>` | 1h TWAP → LastTrade → initial_price | 全局底层 |
| `EntityPricingProvider` | `entity_common::PricingProvider` | 同上 + `is_price_stale(2400)` | entity-registry 初始资金 |
| `NexExchangeRateProvider` | `ExchangeRateProvider` | 同上 + 置信度评估(0-100) | 佣金/打赏等 |

### 消费方

| 模块 | 用途 | 接口 |
|------|------|------|
| entity-registry | 开店初始资金 | `entity_common::PricingProvider` |
| entity-product | 服务押金 | `entity_common::PricingProvider` |
| storage-service | 运营者保证金 | `DepositCalculatorImpl` |
| arbitration | 投诉押金换算 | `trading_common::PricingProvider` |

### 陈旧保护

超过 2400 区块（~4h @6s/block）无交易时，entity 初始资金计算自动回退到保守兜底值。

---

## 公共查询接口

| 方法 | 返回 | 说明 |
|------|------|------|
| `get_sell_order_list()` | `Vec<Order>` | 活跃且未过期的卖单列表 |
| `get_buy_order_list()` | `Vec<Order>` | 活跃且未过期的买单列表 |
| `get_user_order_list(user)` | `Vec<Order>` | 指定用户的所有订单 |
| `get_best_prices()` | `(Option<u64>, Option<u64>)` | (最优卖价, 最优买价) |

---

## 精度说明

| 数据 | 精度 | 示例 |
|------|------|------|
| NEX 数量 | 10^12 | `1_000_000_000_000` = 1 NEX |
| USDT 金额 | 10^6 | `1_000_000` = 1 USDT |
| usdt_price | 10^6 (USDT per NEX) | `500_000` = 0.5 USDT/NEX |
| USDT 金额计算 | `nex_amount × usdt_price / 10^12` | — |
| 保证金计算 | `usdt_amount × rate/10000 × UsdtToNexRate/10^6` | — |

> 前端精度转换：`price / 1_000_000` → USDT 显示，`amount / 1_000_000_000_000` → NEX 显示。polkadot.js `formatBalance()` 自动读取 `tokenDecimals=12`。

---

## Config 参数

| 参数 | 类型 | Runtime 值 | 说明 |
|------|------|-----------|------|
| **订单** | | | |
| `DefaultOrderTTL` | `u32` | 24h | 订单有效期（区块数） |
| `MaxActiveOrdersPerUser` | `u32` | 100 | 每用户最大活跃订单 |
| **USDT 交易** | | | |
| `UsdtTimeout` | `u32` | 12h | 付款超时（区块数） |
| `VerificationGracePeriod` | `u32` | 1h | AwaitingVerification 超时宽限期 |
| `UnderpaidGracePeriod` | `u32` | 2h | 少付补付窗口 |
| **TWAP** | | | |
| `BlocksPerHour` | `u32` | 600 | 1h 区块数 |
| `BlocksPerDay` | `u32` | 14400 | 24h 区块数 |
| `BlocksPerWeek` | `u32` | 100800 | 7d 区块数 |
| **价格保护** | | | |
| `CircuitBreakerDuration` | `u32` | 1h | 熔断持续时间 |
| **OCW 奖励** | | | |
| `VerificationReward` | `Balance` | 0.1 NEX | 验证奖励 |
| `RewardSource` | `AccountId` | nxm/rwds | 奖励来源账户 |
| **保证金** | | | |
| `BuyerDepositRate` | `u16` | 1000 (10%) | 保证金比例 bps |
| `MinBuyerDeposit` | `Balance` | 10 NEX | 最低保证金 |
| `DepositForfeitRate` | `u16` | 10000 (100%) | 超时没收比例 bps |
| `UsdtToNexRate` | `u64` | 10_000_000_000 | 保证金换算汇率（1 USDT = 10 NEX） |
| **账户** | | | |
| `TreasuryAccount` | `AccountId` | nxm/trsy | 国库账户 |
| `SeedLiquidityAccount` | `AccountId` | nxm/seed | 种子账户 |
| **治理** | | | |
| `MarketAdminOrigin` | `EnsureOrigin` | TreasuryCouncil 2/3 | 管理操作审批 |
| **seed_liquidity** | | | |
| `FirstOrderTimeout` | `u32` | 1h | 免保证金短超时 |
| `MaxFirstOrderAmount` | `Balance` | 100 NEX | 免保证金单笔上限 |
| `MaxWaivedSeedOrders` | `u32` | 20 | 单次最多挂单数 |
| `SeedPricePremiumBps` | `u16` | 2000 (20%) | 溢价比例 |
| `SeedOrderUsdtAmount` | `u64` | 10_000_000 | 每笔固定 10 USDT |
| `SeedTronAddress` | `[u8; 34]` | 固定地址 | 种子 TRON 收款地址 |

---

## 测试

```bash
cargo test -p pallet-nex-market    # 61 个单元测试
```

覆盖范围：
- 卖单/买单创建、取消、过期
- reserve_sell_order / accept_buy_order（含部分成交）
- confirm_payment 流程
- 完整交易结算（Exact / Overpaid）
- 少付处理（Underpaid / SeverelyUnderpaid / Invalid）
- 补付窗口（submit_underpaid_update / finalize_underpaid）
- 超时退款 + 保证金没收（三阶段）
- 价格偏离检查 + 熔断触发/解除
- TWAP 累积器更新
- 最优价格维护
- seed_liquidity 多层防御（L0-L3）
- 订单回滚（filled_amount 恢复 + 索引恢复）
- 多档判定逻辑边界
- 市场统计更新

---

## 与 pallet-entity-market 的区别

| 维度 | entity-market | nex-market |
|------|---------------|------------|
| 交易对 | Entity Token ↔ NEX/USDT | NEX ↔ USDT |
| 市场数量 | 多个（每店铺一个） | 单一全局市场 |
| shop_id | 必须 | 无 |
| NEX 通道 | 原子交换 | 无（NEX 是标的物） |
| USDT 通道 | ✅ | ✅ |
| TokenProvider | 需要 | 不需要 |
| 做市商 | 不需要 | 不需要 |
| TWAP 预言机 | 无 | ✅ 全链价格数据源 |
| seed_liquidity | 无 | ✅ 冷启动引流 |

---

**License**: Unlicense
