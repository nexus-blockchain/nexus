# pallet-entity-market v2.0.0

> 实体代币 P2P 交易市场模块 | Runtime Index: 126

## 概述

`pallet-entity-market` 实现实体代币的链上 P2P 交易市场。每个 Entity 可独立配置并运营自己的代币市场，支持 **NEX 链上即时结算**。

### 核心能力

- **链上原子交换** — NEX ↔ Entity Token 即时结算
- **限价单 + 市价单** — 挂单等待撮合 / 立即以最优价成交（滑点保护）
- **自动交叉撮合** — 挂单时自动与对手方价格交叉的订单撮合
- **三周期 TWAP 预言机** — 1h / 24h / 7d 时间加权平均价格，防操纵
- **熔断机制** — 价格偏离 7d TWAP 超阈值自动暂停交易
- **价格偏离保护** — 限价单/改单价格不得偏离参考价过大
- **内幕交易限制** — 黑窗口期内幕人员禁止交易（集成 DisclosureProvider）
- **过期订单自动清理** — `on_idle` 自动清理 + 外部触发清理

## 架构

```
┌──────────────────────────────────────────────────────┐
│              pallet-entity-market                      │
│              (pallet_index = 126)                      │
├──────────────────────────────────────────────────────┤
│  交易                                                 │
│  place_sell_order(0)    place_buy_order(1)            │
│  take_order(2)          cancel_order(3)               │
│  market_buy(12)         market_sell(13)               │
│  modify_order(30)       batch_cancel_orders(28)       │
├──────────────────────────────────────────────────────┤
│  市场管理 (Entity Owner)                              │
│  configure_market(4)    pause_market(26)              │
│  resume_market(27)      set_initial_price(17)         │
│  configure_price_protection(15)                       │
│  lift_circuit_breaker(16)                             │
├──────────────────────────────────────────────────────┤
│  管理员 (Root)                                        │
│  force_cancel_order(23) global_market_pause(32)       │
├──────────────────────────────────────────────────────┤
│  维护                                                 │
│  cleanup_expired_orders(29)                           │
├──────────────────────────────────────────────────────┤
│  TWAP 预言机 (1h / 24h / 7d)                         │
│  异常价格过滤 (±100% 限幅) → 累积器 → 滚动快照       │
└──────────────────────────────────────────────────────┘
         │                              │
         ▼                              ▼
   EntityProvider              EntityTokenProvider
   (实体查询/权限)              (代币余额/锁定/转账)
```

## 交易流程

链上原子交换，无需链下操作。

```
Alice (卖家)                                 Bob (买家)
    │ place_sell_order(entity, 1000, 100)        │
    │ → Token 锁定                                │
    │ → 自动撮合价格交叉的买单                     │
    │                                              │
    │                    take_order(order_id, None) │
    │                    → NEX 支付                 │
    ▼                                              ▼
┌──────────────────────────────────────────────────┐
│  原子交换                                        │
│  Token: Alice → Bob                              │
│  NEX:   Bob → Alice (扣除手续费)                 │
│  Fee:   → Entity Owner                           │
└──────────────────────────────────────────────────┘
```

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
1. 成交量 ≥ `min_trades_for_twap` → 使用 1h TWAP
2. 成交量不足但有 `initial_price` → 使用实体所有者设定的初始价格
3. 都没有 → 跳过检查

**熔断**: 成交价偏离 7d TWAP 超过 `circuit_breaker_threshold` → 暂停交易 `CircuitBreakerDuration` 个区块。

## 数据结构

### TradeOrder

```rust
pub struct TradeOrder<T: Config> {
    pub order_id: u64,
    pub entity_id: u64,
    pub maker: T::AccountId,
    pub side: OrderSide,              // Buy / Sell
    pub order_type: OrderType,        // Limit / Market
    pub token_amount: T::TokenBalance,
    pub filled_amount: T::TokenBalance,
    pub price: BalanceOf<T>,          // NEX per Token
    pub status: OrderStatus,          // Open / PartiallyFilled / Filled / Cancelled / Expired
    pub created_at: BlockNumber,
    pub expires_at: BlockNumber,
}
```

### MarketConfig

```rust
pub struct MarketConfig {
    pub nex_enabled: bool,        // 启用 NEX 交易
    pub fee_rate: u16,            // 手续费率 (bps, 100 = 1%)
    pub min_order_amount: u128,   // 最小订单 Token 数量
    pub order_ttl: u32,           // 订单有效期 (区块数)
    pub paused: bool,             // 实体级暂停开关（由 pause/resume_market 控制）
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
    pub circuit_breaker_active: bool,
    pub circuit_breaker_until: u32,
    pub initial_price: Option<Balance>,   // 冷启动参考价格
}
```

## Extrinsics

### 用户交易

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 0 | `place_sell_order(entity_id, token_amount, price)` | signed | 卖单（锁定 Token，自动交叉撮合） |
| 1 | `place_buy_order(entity_id, token_amount, price)` | signed | 买单（锁定 NEX，自动交叉撮合） |
| 2 | `take_order(order_id, amount)` | signed | 吃单（原子交换，收手续费） |
| 3 | `cancel_order(order_id)` | maker | 取消订单（退还锁定资产） |
| 12 | `market_buy(entity_id, token_amount, max_cost)` | signed | 市价买（滑点保护） |
| 13 | `market_sell(entity_id, token_amount, min_receive)` | signed | 市价卖（滑点保护） |
| 28 | `batch_cancel_orders(order_ids)` | maker | 批量取消（≤50） |
| 30 | `modify_order(order_id, new_price, new_amount)` | maker | 改价/减量（含价格偏离检查） |

### 市场管理 (Entity Owner)

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 4 | `configure_market(entity_id, nex_enabled, fee_rate, min_order_amount, order_ttl)` | entity owner | 配置市场参数（不影响 paused 状态） |
| 15 | `configure_price_protection(entity_id, ...)` | entity owner | 配置偏离阈值/滑点/熔断/TWAP |
| 16 | `lift_circuit_breaker(entity_id)` | entity owner | 熔断到期后手动解除 |
| 17 | `set_initial_price(entity_id, initial_price)` | entity owner | TWAP 冷启动参考价格 |
| 26 | `pause_market(entity_id)` | entity owner | 暂停实体市场 |
| 27 | `resume_market(entity_id)` | entity owner | 恢复实体市场 |

### 管理员 (Root)

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 23 | `force_cancel_order(order_id)` | root | 强制取消任意订单 |
| 32 | `global_market_pause(paused)` | root | 全局市场暂停/恢复 |

### 维护

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 29 | `cleanup_expired_orders(entity_id, max_count)` | signed (any) | 清理过期订单（≤100） |

## 存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextOrderId` | `StorageValue<u64>` | 自增订单 ID |
| `Orders` | `StorageMap<u64, TradeOrder>` | 订单主数据 |
| `EntitySellOrders` | `StorageMap<u64, BoundedVec<u64, 1000>>` | 实体卖单索引 |
| `EntityBuyOrders` | `StorageMap<u64, BoundedVec<u64, 1000>>` | 实体买单索引 |
| `UserOrders` | `StorageMap<AccountId, BoundedVec<u64, 100>>` | 用户订单索引 |
| `MarketConfigs` | `StorageMap<u64, MarketConfig>` | 实体市场配置 |
| `MarketStatsStorage` | `StorageMap<u64, MarketStats>` | 市场统计 (订单数/成交量/手续费) |
| `BestAsk` | `StorageMap<u64, Balance>` | 实体最优卖价 |
| `BestBid` | `StorageMap<u64, Balance>` | 实体最优买价 |
| `LastTradePrice` | `StorageMap<u64, Balance>` | 最新成交价 |
| `TwapAccumulators` | `StorageMap<u64, TwapAccumulator>` | TWAP 累积器 (三周期快照) |
| `PriceProtection` | `StorageMap<u64, PriceProtectionConfig>` | 价格保护配置 |
| `GlobalMarketPaused` | `StorageValue<bool>` | 全局市场暂停开关 |

## Events

| 事件 | 字段 | 说明 |
|------|------|------|
| `OrderCreated` | order_id, entity_id, maker, side, token_amount, price | 订单已创建 |
| `OrderFilled` | order_id, entity_id, taker, filled_amount, total_next, fee | 订单已成交 |
| `OrderCancelled` | order_id, entity_id | 订单已取消 |
| `MarketConfigured` | entity_id | 市场配置已更新 |
| `MarketOrderExecuted` | entity_id, trader, side, filled_amount, total_next, total_fee | 市价单已执行 |
| `TwapUpdated` | entity_id, new_price, twap_1h, twap_24h, twap_7d | TWAP 已更新 |
| `CircuitBreakerTriggered` | entity_id, current_price, twap_7d, deviation_bps, until_block | 熔断已触发 |
| `CircuitBreakerLifted` | entity_id | 熔断已解除 |
| `PriceProtectionConfigured` | entity_id, enabled, max_deviation, max_slippage | 价格保护已配置 |
| `InitialPriceSet` | entity_id, initial_price | 初始价格已设置 |
| `MarketPausedEvent` | entity_id | 实体市场已暂停 |
| `MarketResumedEvent` | entity_id | 实体市场已恢复 |
| `OrderForceCancelled` | order_id | Root 强制取消订单 |
| `OrderModified` | order_id, new_price, new_amount | 订单已修改 |
| `ExpiredOrdersCleaned` | entity_id, count, cleaner | 过期订单已清理 |
| `GlobalMarketPauseToggled` | paused | 全局暂停状态变更 |
| `BatchOrdersCancelled` | cancelled_count, failed_count | 批量取消完成 |

## Errors

| 错误 | 说明 |
|------|------|
| `EntityNotFound` | 实体不存在 |
| `NotEntityOwner` | 不是实体所有者 |
| `TokenNotEnabled` | 实体代币未启用 |
| `MarketNotEnabled` | 市场未启用 |
| `OrderNotFound` | 订单不存在 |
| `NotOrderOwner` | 不是订单所有者 |
| `OrderClosed` | 订单已关闭 |
| `InsufficientBalance` | NEX 余额不足 |
| `InsufficientTokenBalance` | Token 余额不足 |
| `AmountTooSmall` | 数量为零或过小 |
| `AmountExceedsAvailable` | 数量超过可用 |
| `ZeroPrice` | 价格为零 |
| `OrderBookFull` | 订单簿已满（1000/边） |
| `UserOrdersFull` | 用户订单数已满（100） |
| `CannotTakeOwnOrder` | 不能吃自己的单 |
| `ArithmeticOverflow` | 算术溢出 |
| `OrderSideMismatch` | 订单方向不匹配 |
| `NoOrdersAvailable` | 没有可用订单（市价单） |
| `SlippageExceeded` | 滑点超限 |
| `PriceDeviationTooHigh` | 价格偏离参考价过大 |
| `MarketCircuitBreakerActive` | 市场处于熔断状态 |
| `InsufficientTwapData` | TWAP 数据不足 |
| `InvalidFeeRate` | 手续费率无效（>50%） |
| `InvalidBasisPoints` | 基点参数无效（>10000） |
| `EntityNotActive` | 实体未激活（Banned/Closed） |
| `OrderTtlTooShort` | 订单 TTL 过短（<10） |
| `InsiderTradingRestricted` | 内幕人员黑窗口期禁止交易 |
| `EntityLocked` | 实体已被全局锁定 |
| `MarketPaused` | 实体市场已暂停 |
| `GlobalMarketPausedError` | 全局市场已暂停 |
| `OrderAmountBelowMinimum` | 订单数量低于最小值 |
| `CircuitBreakerNotActive` | 熔断未激活 |
| `ModifyAmountExceedsOriginal` | 修改后数量超过原始 |
| `InvalidOrderStatus` | 订单状态无效 |
| `TooManyOrders` | 批量操作数量过多 |

## Runtime 配置

```rust
impl pallet_entity_market::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type Balance = u128;
    type TokenBalance = u128;
    type EntityProvider = EntityRegistry;
    type TokenProvider = EntityToken;
    type DefaultOrderTTL = ConstU32<{ 7 * 24 * 600 }>;  // 7 天
    type MaxActiveOrdersPerUser = ConstU32<100>;
    type DefaultFeeRate = ConstU16<30>;              // 0.3%
    type BlocksPerHour = ConstU32<600>;
    type BlocksPerDay = ConstU32<{ 24 * 600 }>;
    type BlocksPerWeek = ConstU32<{ 7 * 24 * 600 }>;
    type CircuitBreakerDuration = ConstU32<600>;     // 1h
    type DisclosureProvider = EntityDisclosure;
}
```

## 查询接口

```rust
impl<T: Config> Pallet<T> {
    pub fn get_order_book_depth(entity_id: u64, depth: u32) -> OrderBookDepth;
    pub fn get_market_summary(entity_id: u64) -> MarketSummary;
    pub fn get_best_prices(entity_id: u64) -> (Option<Balance>, Option<Balance>);
    pub fn get_spread(entity_id: u64) -> Option<Balance>;
    pub fn calculate_twap(entity_id: u64, period: TwapPeriod) -> Option<Balance>;
    pub fn get_order_book_snapshot(entity_id: u64) -> (Vec<(Balance, TokenBalance)>, Vec<(Balance, TokenBalance)>);
    pub fn get_sell_orders(entity_id: u64) -> Vec<TradeOrder>;
    pub fn get_buy_orders(entity_id: u64) -> Vec<TradeOrder>;
    pub fn get_user_orders(user: &AccountId) -> Vec<TradeOrder>;
}
```

## EntityTokenPriceProvider

本模块实现 `EntityTokenPriceProvider` trait，供其他模块查询代币价格：

- **`get_token_price(entity_id)`** — 优先级: 1h TWAP → LastTradePrice → initial_price
- **`get_token_price_usdt(entity_id)`** — 已移除，始终返回 `None`
- **`token_price_confidence(entity_id)`** — 0~95 置信度分数
- **`is_token_price_stale(entity_id, max_age_blocks)`** — 价格是否过时

## 安全机制

- **原子交换** — 单笔交易内完成 Token 和 NEX 的双向转移
- **价格偏离检查** — 限价单/改单价格不得偏离 TWAP/初始价格超过 `max_price_deviation`
- **异常价格过滤** — TWAP 累积时偏离上次价格 >100% 的成交价被限幅至 ±50%
- **熔断机制** — 价格偏离 7d TWAP 超阈值自动暂停交易
- **滑点保护** — 市价单 `max_cost` / `min_receive` 防止不利成交
- **自吃单防护** — 禁止自己吃自己的单
- **内幕交易限制** — 黑窗口期内幕人员禁止交易
- **过期订单过滤** — 撮合时过滤已过期但尚未清理的订单

## 已知技术债

| 项目 | 状态 | 说明 |
|------|------|------|
| Weight benchmarking | 🟡 占位 | 所有 extrinsic 使用硬编码占位值 |
| 订单簿排序 | 🟡 O(N log N) | 每次撮合全量排序，大订单簿时性能待优化 |

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v2.0.0 | 2026-03-04 | **USDT 通道移除**: 删除所有 USDT 交易、OCW 验证、保证金机制；删除死代码 (MarketOperation, DailyTradeStats, MarketSummaryStorage)；`total_fees_cos`→`total_fees_nex`；`configure_market` 保留 paused 状态；`modify_order` 增加价格偏离检查；`cleanup_expired_orders` 移除 owner 付费奖励；`OrderFilled`/`OrderCancelled` 事件增加 entity_id |
| v1.2.0 | 2026-03-04 | 功能扩展: 新增 extrinsics (pause/resume, batch_cancel, cleanup, modify, force_cancel, global_pause) |
| v1.1.0 | 2026-02-26 | EntityTokenPriceProvider 实现 |
| v1.0.0 | 2026-02-24 | 架构重构: shop_id → entity_id |
| v0.5.0 | 2026-02-01 | 三周期 TWAP 预言机 + 熔断 |
| v0.3.0 | 2026-02-01 | 市价单 + 滑点保护 |
| v0.1.0 | 2026-02-01 | NEX 限价单 (place_sell/buy, take, cancel) |

## 相关模块

- [pallet-entity-common](../common/) — 共享 Trait 接口（EntityProvider, EntityTokenProvider, DisclosureProvider）
- [pallet-entity-registry](../registry/) — 实体管理（EntityProvider 实现方）
- [pallet-entity-token](../token/) — 实体代币（EntityTokenProvider 实现方）
