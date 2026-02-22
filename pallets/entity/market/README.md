# pallet-entity-market v0.8.0

> 实体代币 P2P 交易市场模块 | Runtime Index: 126

## 概述

`pallet-entity-market` 实现实体代币的链上 P2P 交易市场。每个 Shop 可独立配置并运营自己的代币市场，支持 **NEX 链上即时结算** 和 **USDT 链下支付 + OCW 验证** 两种通道。

### 核心能力

- **双通道交易** — NEX（链上原子交换）+ USDT（TRC20 链下支付 + OCW 验证）
- **限价单 + 市价单** — 挂单等待撮合 / 立即以最优价成交（滑点保护）
- **三周期 TWAP 预言机** — 1h / 24h / 7d 时间加权平均价格，防操纵
- **熔断机制** — 价格偏离 7d TWAP 超阈值自动暂停交易
- **买家保证金** — USDT 通道锁定 NEX 保证金，防不付款风险
- **多档金额判定** — OCW 验证实际付款金额，按比例自动处理少付
- **OCW 验证激励** — 任何人可触发验证确认并获取奖励

## 架构

```
┌──────────────────────────────────────────────────────────────────┐
│                     pallet-entity-market                         │
│                     (pallet_index = 126)                         │
├──────────────────┬───────────────────────────────────────────────┤
│                  │                                               │
│  NEX 通道        │  USDT 通道                                   │
│  (链上即时结算)   │  (链下支付 + OCW 验证)                       │
│                  │                                               │
│  place_sell(0)   │  place_usdt_sell(5)   place_usdt_buy(6)      │
│  place_buy(1)    │  reserve_usdt_sell(7)  accept_usdt_buy(8)    │
│  take_order(2)   │  confirm_payment(9)   verify_payment(10)     │
│  cancel(3)       │  process_timeout(11)                         │
│  market_buy(12)  │  submit_ocw_result(18)                       │
│  market_sell(13) │  claim_reward(19)                             │
│                  │                                               │
├──────────────────┴───────────────────────────────────────────────┤
│  价格保护                                                        │
│  configure_price_protection(15)  lift_circuit_breaker(16)        │
│  set_initial_price(17)           configure_market(4)             │
├──────────────────────────────────────────────────────────────────┤
│  TWAP 预言机 (1h / 24h / 7d)                                    │
│  异常价格过滤 (±100% 限幅) → 累积器 → 滚动快照                  │
├──────────────────────────────────────────────────────────────────┤
│  OCW (offchain_worker)                                           │
│  PendingUsdtTrades → TronGrid API 验证 → submit_ocw_result      │
└──────────────────────────────────────────────────────────────────┘
         │                    │                    │
         ▼                    ▼                    ▼
   EntityProvider       ShopProvider        EntityTokenProvider
   (实体查询)           (店铺查询)          (代币余额/锁定/转账)
```

## NEX 通道交易流程

链上原子交换，无需链下操作。

```
Alice (卖家)                                 Bob (买家)
    │ place_sell_order(shop, 1000, 0.1 NEX)      │
    │ → Token 锁定                                │
    │                                              │
    │                    take_order(order_id, None) │
    │                    → NEX 支付                 │
    ▼                                              ▼
┌──────────────────────────────────────────────────┐
│  原子交换                                        │
│  Token: Alice → Bob                              │
│  NEX:   Bob → Alice (扣除手续费)                 │
│  Fee:   → Shop Owner                             │
└──────────────────────────────────────────────────┘
```

## USDT 通道交易流程

两阶段安全模式：先链上锁定，后链下支付。

### 流程 A — 吃 USDT 卖单 (reserve_usdt_sell_order)

```
① Alice 挂 USDT 卖单 (锁定 Token, 提供 TRON 地址)
② Bob  → reserve_usdt_sell_order (锁定 NEX 保证金 + 锁定订单份额)
③ Bob  链下转 USDT → Alice 的 TRON 地址
④ Bob  → confirm_usdt_payment (提交 tron_tx_hash)
⑤ OCW  → submit_ocw_result (验证 TRON 交易 + 多档判定)
⑥ 任何人 → claim_verification_reward (执行结果处理 + 领取奖励)
```

### 流程 B — 接受 USDT 买单 (accept_usdt_buy_order)

```
① Bob  挂 USDT 买单
② Alice → accept_usdt_buy_order (锁定 Bob 保证金 + 锁定 Alice Token)
③ Bob  链下转 USDT → Alice 的 TRON 地址
④ Bob  → confirm_usdt_payment
⑤ OCW  → submit_ocw_result
⑥ 任何人 → claim_verification_reward
```

### 超时处理

任何人可调用 `process_usdt_timeout`：退还卖家 Token，按 `DepositForfeitRate` 没收买家保证金归国库。

## 付款金额多档判定

OCW 验证后根据实际付款比例自动处理：

| 比例 | 判定结果 | 处理 |
|------|---------|------|
| ≥ 100.5% | `Overpaid` | ✅ Token 全部释放，保证金退还 |
| 99.5% ~ 100.5% | `Exact` | ✅ Token 全部释放，保证金退还 |
| 50% ~ 99.5% | `Underpaid` | ⚠️ Token 按比例释放，保证金全部没收归国库 |
| < 50% | `SeverelyUnderpaid` | ⚠️ Token 按比例释放，保证金全部没收归国库 |
| = 0 | `Invalid` | ❌ Token 全部退还卖家，保证金全部没收归国库 |

**设计要点**：±0.5% 容差处理汇率波动；少付无需人工仲裁，全自动按比例处理。

## TWAP 价格预言机

三周期时间加权平均价格，防止价格操纵。

```
每次成交 → update_twap_accumulator()
  │
  ├── 异常价格过滤: 偏离上次价格 >100% → 限幅至 ±50%
  ├── 累积价格更新: cumulative += last_price × blocks_elapsed
  ├── 1h 快照: 每 10 分钟滚动更新
  ├── 24h 快照: 每 1 小时滚动更新
  └── 7d 快照: 每 1 天滚动更新
```

**TWAP 计算**: `(current_cumulative - snapshot_cumulative) / block_diff`

**价格偏离检查优先级**:
1. 成交量 ≥ `min_trades_for_twap` → 使用 1h TWAP 作为参考
2. 成交量不足但有 `initial_price` → 使用店主设定的初始价格
3. 都没有 → 跳过检查

**熔断**: 成交价偏离 7d TWAP 超过 `circuit_breaker_threshold` → 暂停交易 `CircuitBreakerDuration` 个区块。

## 数据结构

### TradeOrder

```rust
pub struct TradeOrder<T: Config> {
    pub order_id: u64,
    pub shop_id: u64,
    pub maker: T::AccountId,
    pub side: OrderSide,              // Buy / Sell
    pub order_type: OrderType,        // Limit / Market
    pub channel: PaymentChannel,      // NEX / USDT
    pub token_amount: T::TokenBalance,
    pub filled_amount: T::TokenBalance,
    pub price: BalanceOf<T>,          // NEX 通道: NEX/Token
    pub usdt_price: u64,              // USDT 通道: USDT/Token (精度 10^6)
    pub tron_address: Option<TronAddress>,  // 仅 USDT 卖单
    pub status: OrderStatus,          // Open / PartiallyFilled / Filled / Cancelled / Expired
    pub created_at: BlockNumber,
    pub expires_at: BlockNumber,
}
```

### UsdtTrade

```rust
pub struct UsdtTrade<T: Config> {
    pub trade_id: u64,
    pub order_id: u64,
    pub shop_id: u64,
    pub seller: T::AccountId,
    pub buyer: T::AccountId,
    pub token_amount: T::TokenBalance,
    pub usdt_amount: u64,                    // 精度 10^6
    pub seller_tron_address: TronAddress,    // Base58, 34 字节
    pub tron_tx_hash: Option<TronTxHash>,    // Hex, 64 字节
    pub status: UsdtTradeStatus,             // AwaitingPayment → AwaitingVerification → Completed/Refunded
    pub created_at: BlockNumber,
    pub timeout_at: BlockNumber,
    pub buyer_deposit: BalanceOf<T>,         // NEX 保证金
    pub deposit_status: BuyerDepositStatus,  // None / Locked / Released / Forfeited / PartiallyForfeited
}
```

### MarketConfig

```rust
pub struct MarketConfig<Balance> {
    pub cos_enabled: bool,        // 启用 NEX 交易
    pub usdt_enabled: bool,       // 启用 USDT 交易
    pub fee_rate: u16,            // 手续费率 (bps, 100 = 1%)
    pub min_order_amount: u128,   // 最小订单 Token 数量
    pub order_ttl: u32,           // 订单有效期 (区块数)
    pub usdt_timeout: u32,        // USDT 交易超时 (区块数)
    pub fee_recipient: Option<Balance>,  // 手续费接收方 (None = Shop Owner)
}
```

### PriceProtectionConfig

```rust
pub struct PriceProtectionConfig<Balance> {
    pub enabled: bool,                    // 默认 true
    pub max_price_deviation: u16,         // 限价单最大偏离 (bps, 默认 2000 = 20%)
    pub max_slippage: u16,                // 市价单最大滑点 (bps, 默认 500 = 5%)
    pub circuit_breaker_threshold: u16,   // 熔断阈值 (bps, 默认 5000 = 50%)
    pub min_trades_for_twap: u64,         // 启用 TWAP 的最小成交数 (默认 100)
    pub circuit_breaker_active: bool,     // 是否处于熔断
    pub circuit_breaker_until: u32,       // 熔断结束区块
    pub initial_price: Option<Balance>,   // 冷启动参考价格
}
```

## Extrinsics

### 用户交易

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 0 | `place_sell_order(shop_id, token_amount, price)` | signed | NEX 卖单（锁定 Token） |
| 1 | `place_buy_order(shop_id, token_amount, price)` | signed | NEX 买单（锁定 NEX） |
| 2 | `take_order(order_id, amount)` | signed | 吃单（原子交换，收手续费） |
| 3 | `cancel_order(order_id)` | maker | 取消订单（退还锁定资产） |
| 12 | `market_buy(shop_id, token_amount, max_cost)` | signed | 市价买（滑点保护） |
| 13 | `market_sell(shop_id, token_amount, min_receive)` | signed | 市价卖（滑点保护） |

### USDT 通道

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 5 | `place_usdt_sell_order(shop_id, amount, usdt_price, tron_addr)` | signed | 挂 USDT 卖单（锁定 Token） |
| 6 | `place_usdt_buy_order(shop_id, amount, usdt_price)` | signed | 挂 USDT 买单 |
| 7 | `reserve_usdt_sell_order(order_id, amount)` | signed (buyer) | 预锁定卖单（锁定保证金 + 份额） |
| 8 | `accept_usdt_buy_order(order_id, amount, tron_addr)` | signed (seller) | 接受买单（锁定保证金 + Token） |
| 9 | `confirm_usdt_payment(trade_id, tron_tx_hash)` | buyer | 提交链下支付凭证（64 字节 hex） |
| 10 | `verify_usdt_payment(trade_id, verified, actual_amount)` | none (OCW) | OCW 验证（ValidateUnsigned） |
| 11 | `process_usdt_timeout(trade_id)` | signed (any) | 处理超时（退 Token，没收保证金） |

### OCW 激励

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 18 | `submit_ocw_result(trade_id, actual_amount)` | none (OCW) | 提交验证结果 + 多档判定 |
| 19 | `claim_verification_reward(trade_id)` | signed (any) | 执行验证结果 + 领取奖励 |

### 市场管理 (Shop Owner)

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 4 | `configure_market(shop_id, ...)` | shop owner | 配置双通道/手续费/TTL/超时 |
| 15 | `configure_price_protection(shop_id, ...)` | shop owner | 配置偏离阈值/滑点/熔断/TWAP |
| 16 | `lift_circuit_breaker(shop_id)` | shop owner | 熔断到期后手动解除 |
| 17 | `set_initial_price(shop_id, initial_price)` | shop owner | TWAP 冷启动参考价格 |

## 存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextOrderId` | `StorageValue<u64>` | 自增订单 ID |
| `Orders` | `StorageMap<u64, TradeOrder>` | 订单主数据 |
| `ShopSellOrders` | `StorageMap<u64, BoundedVec<u64, 1000>>` | 店铺卖单索引 |
| `ShopBuyOrders` | `StorageMap<u64, BoundedVec<u64, 1000>>` | 店铺买单索引 |
| `UserOrders` | `StorageMap<AccountId, BoundedVec<u64, 100>>` | 用户订单索引 |
| `MarketConfigs` | `StorageMap<u64, MarketConfig>` | 店铺市场配置 |
| `MarketStatsStorage` | `StorageMap<u64, MarketStats>` | 市场统计 (订单数/成交量/手续费) |
| `NextUsdtTradeId` | `StorageValue<u64>` | 自增 USDT 交易 ID |
| `UsdtTrades` | `StorageMap<u64, UsdtTrade>` | USDT 交易记录 |
| `PendingUsdtTrades` | `StorageValue<BoundedVec<u64, 100>>` | OCW 待验证队列 |
| `OcwVerificationResults` | `StorageMap<u64, (PaymentVerificationResult, u64)>` | OCW 验证结果 |
| `BestAsk` | `StorageMap<u64, Balance>` | 店铺最优卖价 |
| `BestBid` | `StorageMap<u64, Balance>` | 店铺最优买价 |
| `LastTradePrice` | `StorageMap<u64, Balance>` | 最新成交价 |
| `MarketSummaryStorage` | `StorageMap<u64, MarketSummary>` | 市场摘要 |
| `TwapAccumulators` | `StorageMap<u64, TwapAccumulator>` | TWAP 累积器 (三周期快照) |
| `PriceProtection` | `StorageMap<u64, PriceProtectionConfig>` | 价格保护配置 |

## Events

| 事件 | 字段 | 说明 |
|------|------|------|
| `OrderCreated` | order_id, shop_id, maker, side, token_amount, price | 订单已创建 |
| `OrderFilled` | order_id, taker, filled_amount, total_next, fee | 订单已成交 |
| `OrderCancelled` | order_id | 订单已取消 |
| `MarketConfigured` | shop_id | 市场配置已更新 |
| `UsdtSellOrderCreated` | order_id, shop_id, maker, token_amount, usdt_price, tron_address | USDT 卖单 |
| `UsdtBuyOrderCreated` | order_id, shop_id, maker, token_amount, usdt_price | USDT 买单 |
| `UsdtTradeCreated` | trade_id, order_id, seller, buyer, token_amount, usdt_amount | USDT 交易已创建 |
| `UsdtPaymentSubmitted` | trade_id, tron_tx_hash | 支付凭证已提交 |
| `UsdtTradeCompleted` | trade_id, order_id | USDT 交易已完成 |
| `UsdtTradeVerificationFailed` | trade_id, reason | 验证失败 |
| `UsdtTradeRefunded` | trade_id | 超时退款 |
| `MarketOrderExecuted` | shop_id, trader, side, filled_amount, total_next, total_fee | 市价单已执行 |
| `TwapUpdated` | shop_id, new_price, twap_1h, twap_24h, twap_7d | TWAP 已更新 |
| `CircuitBreakerTriggered` | shop_id, current_price, twap_7d, deviation_bps, until_block | 熔断已触发 |
| `CircuitBreakerLifted` | shop_id | 熔断已解除 |
| `PriceProtectionConfigured` | shop_id, enabled, max_deviation, max_slippage | 价格保护已配置 |
| `InitialPriceSet` | shop_id, initial_price | 初始价格已设置 |
| `OcwResultSubmitted` | trade_id, verification_result, actual_amount | OCW 结果已提交 |
| `VerificationRewardClaimed` | trade_id, claimer, reward | 验证奖励已领取 |
| `BuyerDepositLocked` | trade_id, buyer, deposit | 保证金已锁定 |
| `BuyerDepositReleased` | trade_id, buyer, deposit | 保证金已退还 |
| `BuyerDepositForfeited` | trade_id, buyer, forfeited, to_treasury | 保证金已没收 |
| `UnderpaidAutoProcessed` | trade_id, expected, actual, ratio, token_released, deposit_forfeited | 少付自动处理 |

## Errors

| 错误 | 说明 |
|------|------|
| `ShopNotFound` | 店铺不存在 |
| `NotShopOwner` | 不是店主 |
| `TokenNotEnabled` | 店铺代币未启用 |
| `MarketNotEnabled` | NEX 市场未启用 |
| `UsdtMarketNotEnabled` | USDT 市场未启用（需 `configure_market` 开启） |
| `OrderNotFound` | 订单不存在 |
| `NotOrderOwner` | 不是订单所有者 |
| `OrderClosed` | 订单已关闭（Filled/Cancelled/Expired） |
| `InsufficientBalance` | NEX 余额不足 |
| `InsufficientTokenBalance` | Token 余额不足 |
| `InsufficientDepositBalance` | 买家保证金余额不足 |
| `AmountTooSmall` | 数量为零或过小 |
| `AmountExceedsAvailable` | 数量超过可用 |
| `ZeroPrice` | 价格为零 |
| `OrderBookFull` | 订单簿已满（1000/边） |
| `UserOrdersFull` | 用户订单数已满（100） |
| `CannotTakeOwnOrder` | 不能吃自己的单 |
| `ArithmeticOverflow` | 算术溢出 |
| `OrderSideMismatch` | 订单方向不匹配 |
| `ChannelMismatch` | 支付通道不匹配 |
| `InvalidTronAddress` | TRON 地址无效（需 34 字节 Base58, T 开头） |
| `InvalidTxHash` | 交易哈希无效（需 64 字节 hex） |
| `UsdtTradeNotFound` | USDT 交易不存在 |
| `NotTradeParticipant` | 不是交易参与者 |
| `InvalidTradeStatus` | 交易状态无效 |
| `TradeTimeout` | 交易已超时 |
| `PendingQueueFull` | 待验证队列已满（100） |
| `NoOrdersAvailable` | 没有可用订单（市价单） |
| `SlippageExceeded` | 滑点超限 |
| `PriceDeviationTooHigh` | 价格偏离参考价过大 |
| `MarketCircuitBreakerActive` | 市场处于熔断状态 |
| `OcwResultNotFound` | OCW 验证结果不存在 |
| `InsufficientTwapData` | TWAP 数据不足 |

## Runtime 配置

```rust
impl pallet_entity_market::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type Balance = u128;
    type TokenBalance = u128;
    type EntityProvider = EntityRegistry;
    type ShopProvider = EntityShop;
    type TokenProvider = EntityToken;
    type DefaultOrderTTL = ConstU32<14400>;          // 24h
    type MaxActiveOrdersPerUser = ConstU32<100>;
    type DefaultFeeRate = ConstU16<100>;              // 1%
    type DefaultUsdtTimeout = ConstU32<7200>;         // 12h
    type BlocksPerHour = ConstU32<600>;
    type BlocksPerDay = ConstU32<14400>;
    type BlocksPerWeek = ConstU32<100800>;
    type CircuitBreakerDuration = ConstU32<600>;      // 1h
    type VerificationReward = ConstU128<100_000_000_000>;  // 0.1 NEX
    type RewardSource = TreasuryAccountId;
    type BuyerDepositRate = ConstU16<1000>;           // 10%
    type MinBuyerDeposit = ConstU128<{ 10 * UNIT }>;  // 10 NEX
    type DepositForfeitRate = ConstU16<10000>;        // 100%
    type UsdtToNexRate = ConstU64<10_000_000_000>;
    type TreasuryAccount = TreasuryAccountId;
}
```

## 查询接口

```rust
impl<T: Config> Pallet<T> {
    /// 获取订单簿深度（每边 N 档，聚合同价位）
    pub fn get_order_book_depth(shop_id: u64, depth: u32) -> OrderBookDepth;
    /// 获取市场摘要 (best_ask, best_bid, last_price, volumes)
    pub fn get_market_summary(shop_id: u64) -> MarketSummary;
    /// 获取最优买卖价
    pub fn get_best_prices(shop_id: u64) -> (Option<Balance>, Option<Balance>);
    /// 获取买卖价差
    pub fn get_spread(shop_id: u64) -> Option<Balance>;
    /// 计算指定周期的 TWAP
    pub fn calculate_twap(shop_id: u64, period: TwapPeriod) -> Option<Balance>;
    /// 获取订单簿快照（简化版，20 档）
    pub fn get_order_book_snapshot(shop_id: u64) -> (Vec<(Balance, TokenBalance)>, Vec<(Balance, TokenBalance)>);
    /// 获取店铺卖单/买单列表
    pub fn get_sell_orders(shop_id: u64) -> Vec<TradeOrder>;
    pub fn get_buy_orders(shop_id: u64) -> Vec<TradeOrder>;
    /// 获取用户订单列表
    pub fn get_user_orders(user: &AccountId) -> Vec<TradeOrder>;
}
```

## 安全机制

- **原子交换** — NEX 通道在单笔交易内完成 Token 和 NEX 的双向转移
- **两阶段锁定** — USDT 通道先链上锁定份额/保证金，后链下支付
- **NEX 保证金** — 防止 USDT 买家不付款（`MinBuyerDeposit` + `DepositForfeitRate`）
- **ValidateUnsigned** — OCW 提交限制：交易存在 + AwaitingVerification 状态 + 无重复结果
- **价格偏离检查** — 限价单价格不得偏离 TWAP/初始价格超过 `max_price_deviation`
- **异常价格过滤** — TWAP 累积时偏离上次价格 >100% 的成交价被限幅至 ±50%
- **熔断机制** — 价格偏离 7d TWAP 超阈值自动暂停交易
- **滑点保护** — 市价单 `max_cost` / `min_receive` 防止不利成交
- **自吃单防护** — `CannotTakeOwnOrder` 禁止自己吃自己的单

## 已知技术债

| 项目 | 状态 | 说明 |
|------|------|------|
| Weight benchmarking | 🟡 占位 | 所有 extrinsic 使用硬编码占位值（20k~150k ref_time, proof_size=0） |
| Token 实际锁定 | 🟡 简化 | NEX 卖单的 Token 锁定通过注释标记，需接入 TokenProvider::reserve |
| 24h 高低价/成交量 | 🟡 TODO | `MarketSummary` 中的 high_24h / low_24h / volume_24h 返回 0 |
| 订单过期清理 | 🟡 未实现 | 过期订单未自动清理，需 on_idle 或外部触发 |
| mock.rs + tests.rs | 🔴 无 | 无单元测试覆盖 |

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-02-01 | NEX 通道限价单（place_sell/buy, take, cancel） |
| v0.2.0 | 2026-02-01 | USDT 通道 + OCW 验证（TRC20 交易验证） |
| v0.3.0 | 2026-02-01 | 市价单支持（market_buy, market_sell + 滑点保护） |
| v0.4.0 | 2026-02-01 | 订单簿深度优化（价格聚合, BestAsk/BestBid 缓存） |
| v0.5.0 | 2026-02-01 | 三周期 TWAP 预言机（1h/24h/7d + 异常过滤 + 熔断） |
| v0.6.0 | 2026-02-04 | OCW 验证激励（submit_ocw_result + claim_verification_reward + ValidateUnsigned） |
| v0.7.0 | 2026-02-04 | 买家保证金机制（NEX reserve + forfeit + release） |
| v0.8.0 | 2026-02-04 | 付款金额多档判定（5 级结果 + 自动按比例处理） |

## 相关模块

- [pallet-entity-common](../common/) — 共享类型 + Trait 接口（EntityProvider, ShopProvider, EntityTokenProvider）
- [pallet-entity-registry](../registry/) — 实体管理（EntityProvider 实现方）
- [pallet-entity-shop](../shop/) — 店铺管理（ShopProvider 实现方）
- [pallet-entity-token](../token/) — 实体代币（EntityTokenProvider 实现方, reserve/unreserve/repatriate）
