# Trading 模块集群

## 模块概述

Nexus 交易系统采用模块化设计，包含以下子模块：

| 模块 | 路径 | 说明 | Runtime Index |
|------|------|------|:---:|
| **pallet-nex-market** | `nex-market/` | NEX/USDT 无做市商 P2P 订单簿 | 56 |
| **pallet-trading-common** | `common/` | 公共 Trait + 工具库（PricingProvider, PriceOracle, ExchangeRateProvider, DepositCalculator） | — |
| **pallet-trading-trc20-verifier** | `trc20-verifier/` | TRC20 USDT 链下验证共享库 | — |

> **历史变更**：`pallet-trading-pricing`、`pallet-trading-credit`、`pallet-trading-maker`、`pallet-trading-p2p` 已于 2026-02-23 废弃删除，由 `pallet-nex-market` 替代。做市商模式已移除，所有用户均可自由挂单/吃单。

---

## 架构设计

### 模块拓扑

```text
┌──────────────────────────────────────────────────────────────────┐
│                          Runtime                                │
│                                                                  │
│  TradingPricingProvider ───→ 1h TWAP → LastTrade → InitialPrice │
│  EntityPricingProvider  ───→ TradingPricingProvider + 陈旧检测    │
│  NexExchangeRateProvider ──→ 汇率 + 置信度评估 (0-100)          │
└──────┬───────────────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────────┐
│                    pallet-nex-market (56)                    │
│                                                              │
│  订单簿 · USDT 交易(OCW) · TWAP 预言机 · 价格保护/熔断      │
│  买家保证金(梯度没收) · 多档判定 · 补付窗口 · seed_liquidity │
└──────┬──────────────────────────────┬───────────────────────┘
       │                              │
       ▼                              ▼
  pallet-trading-common         pallet-trading-trc20-verifier
  (Trait + 共享类型 + 工具)       (TronGrid API 验证)
```

### 跨模块依赖

```text
pallet-nex-market
  ├── Currency (原生 NEX 锁定/转账)
  ├── OCW → pallet-trading-trc20-verifier (TRC20 验证)
  └── 实现 PriceOracle trait（TWAP + LastTradePrice + 陈旧检测）

pallet-trading-common（被外部模块使用）
  ├── pallet-arbitration      → PricingProvider (投诉押金换算)
  ├── pallet-storage-service  → PricingProvider + DepositCalculator (保证金)
  ├── pallet-entity-registry  → PricingProvider (开店初始资金)
  ├── pallet-entity-market    → PricingProvider (商品押金)
  └── pallet-entity-product   → PricingProvider (服务押金)

Runtime 适配器（mod.rs 中实现）:
  TradingPricingProvider   → 优先 1h TWAP → LastTradePrice → initial_price
  EntityPricingProvider    → 委托 TradingPricingProvider + is_price_stale(2400 blocks ≈ 4h)
  NexExchangeRateProvider  → 汇率 + 置信度评估 (0-100)
```

---

## pallet-nex-market（核心交易模块）

无做市商的 NEX/USDT P2P 订单簿。任何人可挂单/吃单，USDT 通过 TRC20 链下支付，OCW 自动验证。

### 交易流程

#### 卖 NEX（卖家锁 NEX，收 USDT）

```text
卖家                      买家                     OCW
  │                        │                        │
  │ place_sell_order ──→ 锁 NEX                     │
  │           reserve_sell_order ──→ 锁买家保证金    │
  │                 链下转 USDT → confirm_payment    │
  │                        │           submit_ocw_result (unsigned)
  │               claim_verification_reward         │
  │                 ← 释放 NEX + 退保证金           │
```

#### 买 NEX（买家挂单，卖家接单）

```text
买家                      卖家                     OCW
  │                        │                        │
  │ place_buy_order ──→ 锁买家保证金                │
  │               accept_buy_order ──→ 锁卖家 NEX   │
  │ 链下转 USDT → confirm_payment                   │
  │                        │           submit_ocw_result
  │               claim_verification_reward         │
```

#### 少付补付流程（50% ~ 99.5%）

```text
OCW 检测到少付 → UnderpaidPending → 补付窗口(2h)
  ├─ 窗口内 OCW 持续扫描 → submit_underpaid_update（更新金额）
  │   └─ 累计 ≥ 99.5% → 升级为 Exact，正常结算
  └─ 窗口到期 → finalize_underpaid（终裁）
      └─ 按最终比例释放 NEX + 梯度没收保证金
```

### 数据结构

#### Order

| 字段 | 类型 | 说明 |
|------|------|------|
| `order_id` | `u64` | 订单 ID |
| `maker` | `AccountId` | 挂单者 |
| `side` | `OrderSide` | `Buy` / `Sell` |
| `nex_amount` | `Balance` | NEX 数量 |
| `filled_amount` | `Balance` | 已成交 NEX 数量 |
| `usdt_price` | `u64` | 每 NEX 的 USDT 单价（精度 10^6） |
| `tron_address` | `Option<TronAddress>` | 卖单=卖家收款地址，买单=买家付款地址 |
| `status` | `OrderStatus` | `Open` / `PartiallyFilled` / `Filled` / `Cancelled` |
| `created_at` | `BlockNumber` | 创建区块 |
| `expires_at` | `BlockNumber` | 过期区块 |
| `buyer_deposit` | `Balance` | 买家预锁定保证金（仅买单，卖单为 0） |
| `deposit_waived` | `bool` | 是否免保证金（仅 seed_liquidity 卖单） |

#### UsdtTrade

| 字段 | 类型 | 说明 |
|------|------|------|
| `trade_id` | `u64` | 交易 ID |
| `order_id` | `u64` | 关联订单 ID |
| `seller` | `AccountId` | 卖 NEX 方（收 USDT） |
| `buyer` | `AccountId` | 买 NEX 方（付 USDT） |
| `nex_amount` | `Balance` | NEX 数量 |
| `usdt_amount` | `u64` | USDT 金额（精度 10^6） |
| `seller_tron_address` | `TronAddress` | 卖家 TRON 收款地址 |
| `buyer_tron_address` | `Option<TronAddress>` | 买家 TRON 付款地址（OCW 匹配用） |
| `status` | `UsdtTradeStatus` | `AwaitingPayment` / `AwaitingVerification` / `UnderpaidPending` / `Completed` / `Refunded` |
| `created_at` | `BlockNumber` | 创建区块 |
| `timeout_at` | `BlockNumber` | 超时区块 |
| `buyer_deposit` | `Balance` | 买家保证金 |
| `deposit_status` | `BuyerDepositStatus` | `None` / `Locked` / `Released` / `Forfeited` |
| `first_verified_at` | `Option<BlockNumber>` | 首次 OCW 验证时间 |
| `first_actual_amount` | `Option<u64>` | 首次检测金额 |
| `underpaid_deadline` | `Option<BlockNumber>` | 少付补付截止区块 |

#### 辅助结构

| 类型 | 说明 |
|------|------|
| `MarketStats` | 市场统计：`total_orders`, `total_trades`, `total_volume_usdt` |
| `PriceSnapshot` | 价格快照：`cumulative_price` (u128), `block_number` (u32) |
| `TwapAccumulator` | TWAP 累积器：三周期快照 + `last_price` + `trade_count` |
| `PriceProtectionConfig` | 价格保护配置：偏离阈值、熔断阈值、初始价格等 |

### Extrinsics

| # | 调用 | 权限 | 参数 | 说明 |
|---|------|------|------|------|
| 0 | `place_sell_order` | 签名 | `nex_amount`, `usdt_price`, `tron_address` | 挂卖单：锁 NEX，提供 TRON 收款地址，校验价格偏离 |
| 1 | `place_buy_order` | 签名 | `nex_amount`, `usdt_price`, `buyer_tron_address` | 挂买单：预锁保证金，提供 TRON 付款地址，校验价格偏离 |
| 2 | `cancel_order` | Owner | `order_id` | 取消订单：退还锁定的 NEX（卖单）或保证金（买单） |
| 3 | `reserve_sell_order` | 签名 | `order_id`, `amount?`, `buyer_tron_address` | 买家吃卖单：锁保证金（或免保证金），创建 UsdtTrade |
| 4 | `accept_buy_order` | 签名 | `order_id`, `amount?`, `tron_address` | 卖家接买单：锁 NEX，按比例分配预锁保证金，创建 UsdtTrade |
| 5 | `confirm_payment` | 买家 | `trade_id` | 声明已付款：从 AwaitingPayment → AwaitingVerification |
| 6 | `process_timeout` | 任何人 | `trade_id` | 处理超时：分阶段策略（AwaitingPayment / AwaitingVerification + 宽限期 / UnderpaidPending） |
| 7 | `submit_ocw_result` | Unsigned | `trade_id`, `actual_amount` | OCW 提交验证结果：Exact/Overpaid 存储、Underpaid 进补付窗口、其余直接终裁 |
| 8 | `claim_verification_reward` | 任何人 | `trade_id` | 领取 OCW 验证奖励（触发 NEX 释放 + 保证金退还 + TWAP 更新） |
| 9 | `configure_price_protection` | MarketAdmin | `enabled`, `max_deviation`, `cb_threshold`, `min_trades` | 配置价格保护参数（偏离上限、熔断阈值、TWAP 最小成交数） |
| 10 | `set_initial_price` | MarketAdmin | `initial_price` | 设置初始基准价格（TWAP 冷启动，初始化累积器） |
| 11 | `lift_circuit_breaker` | MarketAdmin | — | 手动解除已过期的熔断 |
| 13 | `fund_seed_account` | MarketAdmin | `amount` | 国库 → 种子账户注资（补充 seed_liquidity 资金） |
| 14 | `seed_liquidity` | MarketAdmin | `order_count`, `usdt_override?` | 批量挂免保证金卖单（瀑布式定价 + 溢价，冷启动引流） |
| 15 | `auto_confirm_payment` | Unsigned | `trade_id`, `actual_amount` | OCW 预检：买家忘记 confirm 时自动确认 + 验证（一步到位） |
| 16 | `submit_underpaid_update` | Unsigned | `trade_id`, `new_actual_amount` | OCW 补付窗口内更新累计金额（仅接受递增，达 99.5% 自动升级） |
| 17 | `finalize_underpaid` | 任何人 | `trade_id` | 补付窗口到期后终裁：按最终比例释放 NEX + 梯度没收保证金 |

### 多档判定 & 保证金没收

#### 付款金额判定

| 实际 / 应付比例 | 判定结果 | NEX 释放 | 保证金 |
|----------------|----------|----------|--------|
| ≥ 100.5% | Overpaid | 全额释放 | 退还 |
| 99.5% ~ 100.5% | Exact | 全额释放 | 退还 |
| 50% ~ 99.5% | Underpaid | 进入补付窗口 | 待定 |
| < 50% | SeverelyUnderpaid | 按比例释放 | 没收 |
| = 0 | Invalid | 不释放 | 没收 |

#### 保证金梯度没收（补付终裁）

| 最终付款比例 | 没收比例 |
|-------------|---------|
| ≥ 99.5% | 0% |
| 95% ~ 99.5% | 20% |
| 80% ~ 95% | 50% |
| < 80% | 100% |

> 比例计算使用 `compute_payment_ratio_bps()` 返回 u32（basis points），防止超付 6.55 倍时 u16 截断导致误判。

### OCW 三阶段工作流

`on_idle` 每区块刷新 TWAP 累积器 + 🆕 过期订单 GC，`offchain_worker` 执行链下验证：

| 阶段 | 扫描队列 | 触发条件 | 动作 |
|------|---------|---------|------|
| 1. 正常验证 | `PendingUsdtTrades` | AwaitingVerification | TronGrid 验证 → `submit_ocw_result` |
| 2. 补付扫描 | `PendingUnderpaidTrades` | UnderpaidPending | 检查新转账 → `submit_underpaid_update` |
| 3. 预检兜底 | `AwaitingPaymentTrades` | AwaitingPayment 超 50% 超时期 | 检测到账 → `auto_confirm_payment` |

#### 🆕 H2 — 可配置队列容量

三个 OCW 跟踪队列（`PendingUsdtTrades`、`AwaitingPaymentTrades`、`PendingUnderpaidTrades`）容量从硬编码 `ConstU32<100>` 改为 Config trait 参数，Runtime 可按需调整而无需修改 pallet 代码：

- `MaxPendingTrades` — OCW 待验证队列上限
- `MaxAwaitingPaymentTrades` — 待付款跟踪队列上限
- `MaxUnderpaidTrades` — 少付补付跟踪队列上限

#### 🆕 H3 — 过期订单自动 GC

`on_idle` 在 TWAP 刷新后执行有界过期订单清理：

- 每区块最多处理 `MaxExpiredOrdersPerBlock` 个过期订单（避免单块重量超标）
- 过期卖单：退还未成交 NEX（`unreserve`），状态标记 `Expired`，从 `SellOrders` + `UserOrders` 移除
- 过期买单：退还预锁保证金（`unreserve`），状态标记 `Expired`，从 `BuyOrders` + `UserOrders` 移除
- 清理后自动刷新 `BestAsk` / `BestBid`
- 权重感知：尊重 `remaining_weight` 预算，不会超额消耗

#### 🆕 H4 — 增量最优价格维护

`update_best_prices()` O(N) 全扫描重构为三个函数：

| 函数 | 复杂度 | 调用场景 |
|------|--------|---------|
| `update_best_price_on_new_order(price, side)` | O(1) | `place_sell_order` / `place_buy_order` |
| `update_best_price_on_remove(price, side)` | 摊还 O(1)，最坏 O(N) | `cancel_order`（仅当取消的是当前最优价订单时重扫） |
| `refresh_best_prices()` | O(N) | `seed_liquidity`（批量操作）/ `on_idle` GC 后 |

> 绝大多数操作为 O(1)，仅在取消恰好是最优价的订单时触发单侧 O(N) 重扫。`place_sell_order` / `place_buy_order` 权重从 80M 降至 50M ref_time。

#### Unsigned 交易安全

三个 unsigned extrinsic（`submit_ocw_result`、`auto_confirm_payment`、`submit_underpaid_update`）均拒绝 `TransactionSource::External`，仅接受 Local / InBlock 来源，防止外部用户注入伪造结果。

**C3 — TxHash 防重放：** `submit_ocw_result` 和 `auto_confirm_payment` 接受 `Option<TxHash>` 参数。提交时检查 `UsedTxHashes` 存储，已使用的 tx_hash 会被拒绝（防止同一笔 TRON 交易用于多笔 nex-market 交易的支付证明）。

**C4 — actual_amount 安全边界：** `validate_unsigned` 对三个 unsigned extrinsic 均检查 `actual_amount <= trade.usdt_amount × 10`（10 倍上限），防止恶意节点注入极端金额强制 Overpaid/Invalid 判定。`submit_underpaid_update` 额外检查单调递增（`new_actual_amount > previous_amount`），在交易池阶段即过滤无效更新。

**M3 — propagate(false)：** 所有 unsigned 交易设置 `propagate(false)`，仅接受本地 OCW 产生的交易，不广播给 P2P 网络中的其他节点。

#### 🆕 Phase 6 — 剩余 Medium/Low 修复

**M6 — ID 溢出保护：** `NextOrderId` 和 `NextUsdtTradeId` 在 `u64::MAX` 时 `saturating_add(1)` 会静默保持不变，导致新旧订单/交易共享同一 ID（数据覆盖）。修复：读取后立即 `ensure!(id < u64::MAX, ArithmeticOverflow)`。

**M7 — UsedTxHashes TTL 清理：** `UsedTxHashes` 存储随交易量无限增长。修复：值类型改为 `(trade_id, inserted_at_block)`，`on_idle` 中 cursor-based 有界遍历，超过 `TxHashTtlBlocks`（默认 7 天）的条目自动清理。每区块最多处理 10 条，不会超额消耗权重。

**M4 — deviation_bps 截断修复：** `check_price_deviation` 和 `check_circuit_breaker` 中 `as u16` 强转在极端偏离（>655%）时会截断回绕为小值，导致本应触发保护的大偏离被放行。修复：使用 `.min(u16::MAX as u128)` saturating cast。

**M2 — 双读存储优化：** `process_full_payment` 中两次 `Orders::get(order_id)`（seed USDT 累计 + TWAP 更新）合并为一次缓存读取。

**L5 — 清理未使用错误变量：** 移除 `AmountExceedsAvailable`、`DepositNotWaived`、`OrderIndexRestoreFailed` 三个从未引用的错误码。

### 存储概览

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextOrderId` | `ValueQuery<u64>` | 订单 ID 计数器 |
| `Orders` | `Map<u64, Order>` | 订单映射 |
| `SellOrders` | `BoundedVec<u64, 1000>` | 卖单索引 |
| `BuyOrders` | `BoundedVec<u64, 1000>` | 买单索引 |
| `UserOrders` | `Map<AccountId, BoundedVec<u64, 100>>` | 用户订单索引 |
| `NextUsdtTradeId` | `ValueQuery<u64>` | USDT 交易 ID 计数器 |
| `UsdtTrades` | `Map<u64, UsdtTrade>` | USDT 交易映射 |
| `PendingUsdtTrades` | `BoundedVec<u64, MaxPendingTrades>` | OCW 待验证队列（🆕 H2 可配置） |
| `AwaitingPaymentTrades` | `BoundedVec<u64, MaxAwaitingPaymentTrades>` | 待付款跟踪队列（🆕 H2 可配置） |
| `PendingUnderpaidTrades` | `BoundedVec<u64, MaxUnderpaidTrades>` | 少付补付跟踪队列（🆕 H2 可配置） |
| `OcwVerificationResults` | `Map<u64, (PaymentVerificationResult, u64)>` | OCW 验证结果 |
| `BestAsk` / `BestBid` | `Option<u64>` | 最优买卖价 |
| `LastTradePrice` | `Option<u64>` | 最新成交价 |
| `MarketStatsStore` | `MarketStats` | 累计统计（订单数/交易数/USDT 成交量） |
| `TwapAccumulatorStore` | `TwapAccumulator` | TWAP 累积器（三周期快照） |
| `PriceProtectionStore` | `PriceProtectionConfig` | 价格保护 + 熔断配置 |
| `CompletedBuyers` | `Map<AccountId, bool>` | 已完成首单的买家（L3 Sybil 防御） |
| `UsedTxHashes` | `Map<TxHash, (u64, BlockNumber)>` | 🆕 M7: 已使用 tx_hash → (trade_id, 插入区块号) |
| `TxHashGcCursor` | `Option<TxHash>` | 🆕 M7: tx_hash GC 游标 |
| `ActiveWaivedTrades` | `Map<AccountId, u32>` | 活跃免保证金交易数（L2 防 grief） |
| `CumulativeSeedUsdtSold` | `ValueQuery<u64>` | seed 累计成交 USDT 总额（审计对账） |

### Events

| 事件 | 字段 | 触发场景 |
|------|------|---------|
| `OrderCreated` | `order_id`, `maker`, `side`, `nex_amount`, `usdt_price` | 创建订单（sell/buy/seed） |
| `OrderCancelled` | `order_id` | 取消订单 |
| `UsdtTradeCreated` | `trade_id`, `order_id`, `seller`, `buyer`, `nex_amount`, `usdt_amount` | 创建 USDT 交易 |
| `UsdtPaymentSubmitted` | `trade_id` | 买家确认付款 |
| `UsdtTradeCompleted` | `trade_id`, `order_id` | 交易完成结算 |
| `UsdtTradeVerificationFailed` | `trade_id`, `reason` | OCW 验证失败 |
| `UsdtTradeRefunded` | `trade_id` | AwaitingPayment 超时退款 |
| `VerificationTimeoutRefunded` | `trade_id`, `buyer`, `seller`, `usdt_amount` | AwaitingVerification 宽限期后超时退款 |
| `TwapUpdated` | `new_price`, `twap_1h`, `twap_24h`, `twap_7d` | 成交后更新 TWAP |
| `CircuitBreakerTriggered` | `current_price`, `twap_7d`, `deviation_bps`, `until_block` | 价格偏离触发熔断 |
| `CircuitBreakerLifted` | — | 熔断解除 |
| `PriceProtectionConfigured` | `enabled`, `max_deviation` | 价格保护参数变更 |
| `InitialPriceSet` | `initial_price` | 设置初始价格 |
| `OcwResultSubmitted` | `trade_id`, `verification_result`, `actual_amount` | OCW 提交验证结果 |
| `UnderpaidAutoProcessed` | `trade_id`, `expected_amount`, `actual_amount`, `payment_ratio`, `nex_released`, `deposit_forfeited` | 少付自动处理 |
| `BuyerDepositLocked` | `trade_id`, `buyer`, `deposit` | 保证金锁定 |
| `BuyerDepositReleased` | `trade_id`, `buyer`, `deposit` | 保证金退还 |
| `BuyerDepositForfeited` | `trade_id`, `buyer`, `forfeited`, `to_treasury` | 保证金没收（转入国库） |
| `VerificationRewardClaimed` | `trade_id`, `claimer`, `reward`, `reward_paid` | 领取验证奖励 |
| `LiquiditySeeded` | `order_count`, `total_nex`, `source` | seed_liquidity 批量挂单 |
| `SeedAccountFunded` | `amount`, `treasury`, `seed_account` | 国库 → 种子账户注资 |
| `WaivedDepositTradeCreated` | `trade_id`, `buyer`, `nex_amount` | 免保证金交易创建 |
| `AutoPaymentDetected` | `trade_id`, `actual_amount` | OCW 预检检测到 USDT 到账 |
| `UnderpaidDetected` | `trade_id`, `expected_amount`, `actual_amount`, `payment_ratio`, `deadline` | 少付进入补付窗口 |
| `UnderpaidAmountUpdated` | `trade_id`, `previous_amount`, `new_amount` | 补付窗口内金额更新 |
| `UnderpaidFinalized` | `trade_id`, `final_amount`, `payment_ratio`, `deposit_forfeit_rate` | 补付终裁完成 |

### Errors

| 错误 | 说明 |
|------|------|
| `OrderNotFound` | 订单不存在 |
| `NotOrderOwner` | 不是订单所有者 |
| `OrderClosed` | 订单已关闭（非 Open/PartiallyFilled） |
| `InsufficientBalance` | NEX 余额不足 |
| `AmountTooSmall` | 数量过小或为零 |
| `AmountExceedsAvailable` | 数量超过可用余量 |
| `ZeroPrice` | USDT 单价为零 |
| `OrderBookFull` | 订单簿已满（卖单/买单各 1000 上限） |
| `UserOrdersFull` | 用户订单数已满（100 上限） |
| `CannotTakeOwnOrder` | 不能吃自己的单 |
| `ArithmeticOverflow` | 算术溢出 |
| `OrderSideMismatch` | 订单方向不匹配 |
| `InvalidTronAddress` | TRON 地址无效（Base58Check 校验失败） |
| `UsdtTradeNotFound` | USDT 交易不存在 |
| `NotTradeParticipant` | 不是交易参与者 |
| `InvalidTradeStatus` | 交易状态不允许当前操作 |
| `TradeTimeout` | 交易已超时 |
| `PendingQueueFull` | 待验证队列已满（100 上限） |
| `PriceDeviationTooHigh` | 挂单/吃单价格偏离 TWAP 超过阈值 |
| `MarketCircuitBreakerActive` | 市场熔断中，暂停交易 |
| `OcwResultNotFound` | OCW 验证结果不存在 |
| `InsufficientDepositBalance` | 买家保证金余额不足 |
| `InvalidBasisPoints` | 基点参数超过 10000 |
| `FirstOrderLimitReached` | 免保证金首单超限（每账户仅 1 笔活跃） |
| `FirstOrderAmountTooLarge` | 首单金额超过上限 |
| `BuyerAlreadyCompleted` | 该买家已完成过交易，不再享受免保证金 |
| `DepositNotWaived` | 订单不支持免保证金 |
| `TooManySeedOrders` | seed_liquidity 单次订单数超限 |
| `NoPriceReference` | 无可用基准价格（需先 `set_initial_price`） |
| `StillInGracePeriod` | 仍在验证宽限期内，不允许超时 |
| `UnderpaidGraceNotExpired` | 补付窗口尚未到期 |
| `NotUnderpaidPending` | 交易不在 UnderpaidPending 状态 |
| `OrderExpired` | 订单已过期 |
| `CircuitBreakerNotActive` | 熔断未激活，无需解除 |
| `AwaitingPaymentQueueFull` | 待付款跟踪队列已满（100 上限） |
| `UnderpaidQueueFull` | 少付跟踪队列已满（100 上限） |
| `OrderIndexRestoreFailed` | 订单索引恢复失败（订单簿满） |

### 配置参数（Runtime）

```rust
impl pallet_nex_market::Config for Runtime {
    type Currency = Balances;
    type WeightInfo = ();

    // ── 订单参数 ──
    type DefaultOrderTTL = ConstU32<{ 24 * HOURS }>;         // 订单有效期 24h
    type MaxActiveOrdersPerUser = ConstU32<100>;

    // ── USDT 交易 ──
    type UsdtTimeout = ConstU32<{ 12 * HOURS }>;             // USDT 付款超时 12h
    type VerificationGracePeriod = ConstU32<{ 1 * HOURS }>;  // OCW 验证宽限期 1h
    type UnderpaidGracePeriod = ConstU32<{ 2 * HOURS }>;     // 少付补付窗口 2h

    // ── TWAP 时间参数 ──
    type BlocksPerHour = ConstU32<{ 1 * HOURS }>;
    type BlocksPerDay = ConstU32<{ 24 * HOURS }>;
    type BlocksPerWeek = ConstU32<{ 7 * DAYS }>;

    // ── 价格保护 ──
    type CircuitBreakerDuration = ConstU32<{ 1 * HOURS }>;   // 熔断持续 1h

    // ── OCW 奖励 ──
    type VerificationReward = ConstU128<{ UNIT / 10 }>;      // 0.1 NEX/次
    type RewardSource = NexMarketRewardSource;                // PalletId: nxm/rwds

    // ── 买家保证金 ──
    type BuyerDepositRate = ConstU16<1000>;                   // 10%
    type MinBuyerDeposit = ConstU128<{ 10 * UNIT }>;          // 最低 10 NEX
    type DepositForfeitRate = ConstU16<10000>;                // 默认没收比例 100%
    type UsdtToNexRate = ConstU64<10_000_000_000>;            // 1 USDT = 10 NEX

    // ── 账户 ──
    type TreasuryAccount = NexMarketTreasuryAccount;          // PalletId: nxm/trsy
    type SeedLiquidityAccount = NexMarketSeedAccount;         // PalletId: nxm/seed

    // ── 治理 ──
    type MarketAdminOrigin = EnsureProportionAtLeast<         // TreasuryCouncil 2/3
        AccountId, TreasuryCollectiveInstance, 2, 3
    >;

    // ── seed_liquidity 参数 ──
    type FirstOrderTimeout = ConstU32<{ 1 * HOURS }>;        // 免保证金短超时 1h
    type MaxFirstOrderAmount = ConstU128<{ 100 * UNIT }>;    // 免保证金单笔上限 100 NEX
    type MaxWaivedSeedOrders = ConstU32<20>;                  // 单次最多 20 笔
    type SeedPricePremiumBps = ConstU16<2000>;                // 溢价 20%
    type SeedOrderUsdtAmount = ConstU64<10_000_000>;          // 固定 10 USDT/笔
    type SeedTronAddress = NexMarketSeedTronAddr;             // 固定 TRON 收款地址
}
```

### 测试

```bash
cargo test -p pallet-nex-market    # 61 个单元测试
```

---

## pallet-trading-common（公共 Trait + 工具库）

纯 Rust crate（非 FRAME pallet，`no_std` 兼容），提供跨模块共享的 Trait 接口、共享类型和工具函数。

### Trait 接口

| Trait | 方法 | 说明 | 消费方 |
|-------|------|------|--------|
| `PricingProvider<Balance>` | `get_cos_to_usd_rate()`, `report_p2p_trade()` | NEX/USD 底层汇率查询（精度 10^6） | arbitration, storage-service, entity-* |
| `PriceOracle` | `get_twap()`, `get_last_trade_price()`, `is_price_stale()`, `get_trade_count()` | TWAP 预言机 + 陈旧检测 + 交易量 | Runtime 桥接 |
| `ExchangeRateProvider` | `get_nex_usdt_rate()`, `price_confidence()`, `is_rate_reliable()` | 带置信度(0-100)的统一兑换比率 | 佣金换算、打赏定价等 |
| `DepositCalculator<Balance>` | `calculate_deposit(usd_amount, fallback)` | USD 等值 NEX 保证金计算（`usd * 10^12 / rate`） | storage-service |
| `DepositCalculatorImpl<P, B>` | — | DepositCalculator 泛型默认实现，基于 PricingProvider | Runtime 配置 |

#### 价格置信度等级（ExchangeRateProvider）

| 置信度 | 数据来源 | 含义 |
|--------|---------|------|
| 90-100 | TWAP + 高交易量(≥100笔) | 可充分信赖 |
| 60-89 | TWAP 或 LastTradePrice | 一般可信 |
| 30-59 | 仅 initial_price（冷启动） | 谨慎使用 |
| 0-29 | 过时或不可用 | 应使用兜底值 |

#### TWAP 查询窗口（TwapWindow）

| 窗口 | 实际精度 | 抗操纵能力 |
|------|---------|-----------|
| `OneHour` | ~10min | 低 |
| `OneDay` | ~1-2h | 中 |
| `OneWeek` | ~24-48h | 高（最抗操纵） |

### 共享类型（types.rs）

| 类型 | 定义 | 说明 |
|------|------|------|
| `TronAddress` | `BoundedVec<u8, 34>` | TRON Base58 地址 |
| `TronTxHash` | `BoundedVec<u8, 64>` | TRON 交易哈希 |
| `TxHash` | `BoundedVec<u8, 128>` | 通用交易哈希 |
| `MomentOf` | `u64` | Unix 秒时间戳 |
| `Cid` | `BoundedVec<u8, 64>` | IPFS CID |
| `UsdtTradeStatus` | enum | 共享状态枚举（`AwaitingPayment` / `AwaitingVerification` / `Completed` / `Disputed` / `Cancelled` / `Refunded` / `UnderpaidPending`） |
| `BuyerDepositStatus` | enum | 保证金状态（`None` / `Locked` / `Released` / `Forfeited` / `PartiallyForfeited`） |
| `PaymentVerificationResult` | enum | 多档判定结果（`Exact` / `Overpaid` / `Underpaid` / `SeverelyUnderpaid` / `Invalid`） |

### 共享工具函数

| 函数 | 签名 | 说明 |
|------|------|------|
| `calculate_payment_verification_result` | `(expected: u64, actual: u64) → PaymentVerificationResult` | 多档判定（含 expected=0 的 Invalid 边界处理） |
| `compute_payment_ratio_bps` | `(expected: u64, actual: u64) → u32` | 付款比例（basis points），返回 u32 防 u16 截断 |
| `calculate_deposit_forfeit_rate` | `(ratio: u32) → u16` | 保证金梯度没收比例（bps） |

### 工具模块

| 模块 | 导出 | 说明 |
|------|------|------|
| `mask.rs` | `mask_name`, `mask_id_card`, `mask_birthday` | 数据脱敏 |
| `validation.rs` | `is_valid_tron_address` | TRON 地址 Base58Check 校验 |
| `time.rs` | `blocks_to_seconds`, `seconds_to_blocks`, `estimate_timestamp_from_block`, `estimate_remaining_seconds`, `format_duration` | 区块数 ↔ 秒数转换（默认 6s/block） |
| `macros.rs` | — | 公共宏定义 |

详见 [common/README.md](common/README.md)。

---

## pallet-trading-trc20-verifier（TRC20 验证库）

纯 Rust crate（`no_std` 兼容），供 OCW 在链下验证 USDT TRC20 转账。

### 核心功能

- **TRC20 交易验证**：通过 TronGrid API 验证 USDT 转账（合约 `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t`）
- **端点健康评分**：动态评估 API 端点可用性，自动排序（衰减因子 90%）
- **多策略请求**：并行竞速（5s 超时）/ 串行故障转移（10s 超时）
- **最小确认数**：19 confirmations 防止链重组
- **金额匹配判定**：返回 `PaymentVerificationResult` 枚举

### 默认端点

| 端点 | 说明 |
|------|------|
| `api.trongrid.io` | TronGrid 官方（主端点） |
| `api.tronstack.io` | TronStack 第三方 |
| `apilist.tronscanapi.com` | TronScan |

### 端点健康评分

```text
EndpointHealth {
    success_count: u32,     // 成功次数
    failure_count: u32,     // 失败次数
    avg_response_ms: u32,   // 平均响应时间
    score: u32,             // 健康评分 (0-100)
    last_updated: u64,      // 最后更新时间戳
}
```

评分按衰减因子 90% 加权更新，OCW 按评分降序选择端点。

详见 [trc20-verifier/README.md](trc20-verifier/README.md)。

---

## 价格服务架构

### 三层价格接口

```text
                    ┌─────────────────────────────────┐
                    │        ExchangeRateProvider      │  高级封装
                    │   get_nex_usdt_rate()            │
                    │   price_confidence() → 0-100     │
                    │   is_rate_reliable() → bool      │
                    └───────────────┬─────────────────┘
                                    │
                    ┌───────────────┴─────────────────┐
                    │         PricingProvider          │  底层汇率
                    │   get_cos_to_usd_rate()          │
                    │   report_p2p_trade()             │
                    └───────────────┬─────────────────┘
                                    │
                    ┌───────────────┴─────────────────┐
                    │          PriceOracle             │  TWAP 数据源
                    │   get_twap(window)               │
                    │   get_last_trade_price()         │
                    │   is_price_stale(max_age)        │
                    │   get_trade_count()              │
                    └─────────────────────────────────┘
```

### Runtime 适配器

| 适配器 | 接口 | 价格优先级 | 消费方 |
|--------|------|-----------|--------|
| `TradingPricingProvider` | `PricingProvider<Balance>` | 1h TWAP → LastTrade → initial_price | 全局底层 |
| `EntityPricingProvider` | `entity_common::PricingProvider` | 同上 + `is_price_stale(2400)` | entity-registry 初始资金 |
| `NexExchangeRateProvider` | `ExchangeRateProvider` | 同上 + 置信度评估 | 佣金/打赏等 |

### 陈旧保护机制

`EntityPricingProvider::is_price_stale()` 检测超过 2400 区块（~4 小时 @6s/block）无交易：

```text
entity-registry::calculate_initial_fund()
  │
  ├─ is_price_stale() == true → 返回 MinInitialFundCos（保守兜底）
  └─ is_price_stale() == false → 正常计算 usdt_amount × 10^12 / price
```

---

## 前端调用示例

### 卖 NEX（NEX → USDT）

```typescript
import { ApiPromise } from '@polkadot/api';

// 挂卖单（提供 TRON 收款地址）
await api.tx.nexMarket.placeSellOrder(nexAmount, usdtPrice, tronAddress).signAndSend(seller);

// 查询卖单列表
const sellOrders = await api.query.nexMarket.sellOrders();

// 买家吃卖单（提供买家 TRON 付款地址）
await api.tx.nexMarket.reserveSellOrder(orderId, null, buyerTronAddress).signAndSend(buyer);

// 买家链下转 USDT 后确认付款
await api.tx.nexMarket.confirmPayment(tradeId).signAndSend(buyer);

// 任何人领取验证奖励（触发结算）
await api.tx.nexMarket.claimVerificationReward(tradeId).signAndSend(anyone);
```

### 买 NEX（USDT → NEX）

```typescript
// 挂买单（提供买家 TRON 付款地址，自动锁保证金）
await api.tx.nexMarket.placeBuyOrder(nexAmount, usdtPrice, buyerTronAddress).signAndSend(buyer);

// 卖家接买单（提供卖家 TRON 收款地址）
await api.tx.nexMarket.acceptBuyOrder(orderId, null, sellerTronAddress).signAndSend(seller);

// 后续同卖单流程（confirm_payment → OCW → claim_reward）
```

### 管理操作（TreasuryCouncil 2/3 多数审批）

```typescript
// 国库注资种子账户
await api.tx.council.propose(
  threshold, api.tx.nexMarket.fundSeedAccount(amount), lengthBound
);

// 注入流动性种子（批量挂单，可选 USDT 金额覆盖）
await api.tx.council.propose(
  threshold, api.tx.nexMarket.seedLiquidity(orderCount, usdtOverride), lengthBound
);

// 设置初始价格（冷启动）
await api.tx.council.propose(
  threshold, api.tx.nexMarket.setInitialPrice(price), lengthBound
);

// 配置价格保护
await api.tx.council.propose(
  threshold, api.tx.nexMarket.configurePriceProtection(enabled, maxDeviation, cbThreshold, minTrades), lengthBound
);

// 解除熔断
await api.tx.council.propose(
  threshold, api.tx.nexMarket.liftCircuitBreaker(), lengthBound
);
```

### 查询价格（RPC）

```typescript
const lastPrice = await api.query.nexMarket.lastTradePrice();   // 最新成交价
const bestAsk = await api.query.nexMarket.bestAsk();             // 最优卖价
const bestBid = await api.query.nexMarket.bestBid();             // 最优买价
const stats = await api.query.nexMarket.marketStatsStore();      // 市场统计
const twap = await api.query.nexMarket.twapAccumulatorStore();   // TWAP 累积器
```

> **注意**：所有管理操作由 `MarketAdminOrigin`（TreasuryCouncil 2/3 多数）审批，无需 sudo 权限。

---

## 安全设计

### 交易安全

- **买家保证金**：USDT 通道锁定 NEX 保证金，超时/少付自动梯度没收
- **补付窗口**：少付 50%-99.5% 给予 2h 补付时间，避免因网络延迟误判
- **OCW 验证奖励**：0.1 NEX/次，激励任何人触发验证确认
- **预检兜底**：OCW 自动检测买家忘记 `confirm_payment` 的情况（超 50% 超时期才扫描，节省 API 配额）
- **Unsigned 安全**：三个 unsigned extrinsic 均拒绝 External 来源，仅接受 Local/InBlock

### 价格安全

- **TWAP 预言机**：三周期（1h/24h/7d）时间加权平均价格，`on_idle` 每区块推进累积器
- **价格偏离检查**：挂单和吃单均校验价格偏离（默认最大 20%）
- **熔断机制**：价格偏离 7d TWAP 超阈值（默认 50%）自动暂停交易 1h
- **陈旧保护**：超过 4h 无交易时，entity 初始资金计算使用保守兜底值
- **置信度评估**：消费方可通过 `price_confidence()` 判断价格可信度

### seed_liquidity 多层防御

| 层级 | 机制 | 参数 |
|------|------|------|
| **L0 定价** | 保护性瀑布式基准价 + 溢价下限 | 20% 溢价 |
| | 成熟期(≥100笔): 7d TWAP | |
| | 过渡期(≥30笔): max(24h TWAP, InitialPrice) | |
| | 冷启动(<30笔): InitialPrice 兜底 | |
| **L1 资金隔离** | 独立种子账户，需 `fund_seed_account` 注资 | PalletId: nxm/seed |
| **L2 防 Grief** | 单笔上限 + 每账户最多 1 笔活跃 + 短超时 | 100 NEX / 1h |
| **L3 防 Sybil** | 完成首单后标记 `CompletedBuyers` | 不再免保证金 |
| **审计** | `CumulativeSeedUsdtSold` 链上对账 | — |

---

## 版本历史

| 版本 | 日期 | 说明 |
|------|------|------|
| v2.4.0 | 2026-02-26 | 补付窗口(underpaid grace)、OCW 预检(auto_confirm)、fund_seed_account |
| v2.3.0 | 2026-02-26 | 价格服务升级：TWAP 优先 + 陈旧保护 + ExchangeRateProvider 统一接口 |
| v2.2.0 | 2026-02-24 | 保护性瀑布式 seed 定价（7d TWAP + 溢价下限）+ 独立种子账户 |
| v2.1.0 | 2026-02-24 | seed_liquidity 流动性注入 + 委员会治理 + L2/L3 防 grief 防 Sybil |
| v2.0.0 | 2026-02-23 | 架构重构：删除 pricing/credit/maker/p2p，新增 pallet-nex-market 替代 |
| v1.0.0 | 2026-02-08 | pallet-trading-p2p 合并 OTC + Swap（已废弃） |
| v0.1.0 | 2025-11-03 | 初始版本，拆分为 maker/otc/swap/common |

---

**License**: Unlicense
