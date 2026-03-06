# pallet-entity-market v2.2.0

> 实体代币 P2P 交易市场模块 | Runtime Index: 126

## 概述

`pallet-entity-market` 实现实体代币的链上 P2P 交易市场。每个 Entity 可独立配置并运营自己的代币市场，支持 **NEX 链上即时结算**，**零手续费**全额转账。

### 核心能力

- **链上原子交换** — NEX ↔ Entity Token 即时结算，零手续费，无链下操作
- **5 种订单类型** — 限价单、市价单、IOC（立即成交或取消）、FOK（全部成交或取消）、Post-Only（仅挂单）
- **自动交叉撮合** — 限价单挂单时自动与对手方价格交叉的订单撮合
- **三周期 TWAP 预言机** — 1h / 24h / 7d 时间加权平均价格，防操纵
- **熔断机制** — 价格偏离 7d TWAP 超阈值自动暂停交易
- **价格偏离保护** — 限价单/改单价格不得偏离参考价过大
- **自吃单防护** — 限价撮合、市价单、直接吃单均跳过自己的订单
- **KYC 准入** — 可设置市场最低 KYC 等级要求
- **内幕交易限制** — 黑窗口期内幕人员禁止交易和改单（集成 DisclosureProvider）
- **过期订单自动清理** — `on_idle` 游标扫描 + 外部触发清理，权重精确追踪
- **市场生命周期** — Active → Paused → Closed，含治理级和 Root 级管控

## 架构

```
┌──────────────────────────────────────────────────────────┐
│                   pallet-entity-market                    │
│                   (pallet_index = 126)                    │
├──────────────────────────────────────────────────────────┤
│  用户交易                                                 │
│  place_sell_order(0)     place_buy_order(1)               │
│  take_order(2)           cancel_order(3)                  │
│  market_buy(12)          market_sell(13)                  │
│  modify_order(30)        batch_cancel_orders(28)          │
│  cancel_all_entity_orders(35)                             │
│  place_ioc_order(38)     place_fok_order(39)              │
│  place_post_only_order(40)                                │
├──────────────────────────────────────────────────────────┤
│  市场管理 (Entity Owner)                                  │
│  configure_market(4)     pause_market(26)                 │
│  resume_market(27)       set_initial_price(17)            │
│  configure_price_protection(15)                           │
│  lift_circuit_breaker(16)                                 │
│  set_kyc_requirement(33) close_market(34)                 │
├──────────────────────────────────────────────────────────┤
│  管理员 (Root / 治理)                                     │
│  force_cancel_order(23)  global_market_pause(32)          │
│  governance_configure_market(36)                          │
│  force_close_market(37)                                   │
├──────────────────────────────────────────────────────────┤
│  维护                                                     │
│  cleanup_expired_orders(29)                               │
├──────────────────────────────────────────────────────────┤
│  TWAP 预言机 (1h / 24h / 7d)                             │
│  异常价格过滤 (±100% 限幅) → 累积器 → 滚动快照           │
├──────────────────────────────────────────────────────────┤
│  on_idle: 游标扫描过期订单 → 退还资产 → 释放名额         │
│  权重: consumed_weight 精确追踪 (ref_time + proof_size)   │
└──────────────────────────────────────────────────────────┘
         │               │               │
         ▼               ▼               ▼
   EntityProvider   TokenProvider   DisclosureProvider
   (实体查询/权限)  (代币余额/锁定)  (内幕交易检查)
         │               │
         ▼               ▼
    KycProvider     PricingProvider
    (KYC 等级)      (NEX/USDT 价格)
```

## 交易流程

链上原子交换，零手续费，全额转账。

```
Alice (卖家)                                 Bob (买家)
    │ place_sell_order(entity, 1000, 100)        │
    │ → Token 锁定 (reserved)                    │
    │ → 自动撮合价格交叉的买单                     │
    │                                              │
    │                    take_order(order_id, None) │
    │                    → NEX 支付                 │
    ▼                                              ▼
┌──────────────────────────────────────────────────┐
│  原子交换（零手续费）                              │
│  Token: Alice(reserved) → Bob(free)               │
│  NEX:   Bob(free) → Alice(free)                   │
└──────────────────────────────────────────────────┘
    │
    ▼
on_trade_completed → TWAP 更新 → 熔断检查
```

## 订单类型

| 类型 | 枚举 | 说明 |
|------|------|------|
| **Limit** | `OrderType::Limit` | 限价单，挂单等待撮合或自动交叉撮合 |
| **Market** | `OrderType::Market` | 市价单，立即以最优价成交（滑点保护） |
| **IOC** | `OrderType::ImmediateOrCancel` | 立即成交或取消，部分成交后剩余自动退还 |
| **FOK** | `OrderType::FillOrKill` | 全部成交或全部取消，不接受部分成交 |
| **PostOnly** | `OrderType::PostOnly` | 仅挂单，若会立即撮合则拒绝 |

## TWAP 价格预言机

三周期时间加权平均价格，防止价格操纵。

```
每次成交 → on_trade_completed()
  │
  ├── update_twap_accumulator()
  │     ├── 异常价格过滤: 偏离上次价格 >100% → 限幅至 ±50%
  │     ├── 累积价格更新: cumulative += last_price × blocks_elapsed
  │     ├── 1h 快照: 每 10 分钟滚动更新
  │     ├── 24h 快照: 每 1 小时滚动更新
  │     └── 7d 快照: 每 1 天滚动更新
  │
  ├── update_last_trade_price()
  ├── emit TwapUpdated event
  └── check_circuit_breaker() → 偏离 7d TWAP 超阈值 → 触发熔断
```

**TWAP 计算**: `(current_cumulative - snapshot_cumulative) / block_diff`

**价格偏离检查优先级** (`check_price_deviation`):
1. 成交量 ≥ `min_trades_for_twap` 且三周期快照充足 → 使用 1h TWAP
2. 成交量不足但有 `initial_price` → 使用实体所有者设定的初始价格
3. 都没有 → 跳过检查

**熔断**: 成交价偏离 7d TWAP 超过 `circuit_breaker_threshold` → 暂停交易 `CircuitBreakerDuration` 个区块。所有交易入口（限价、市价、IOC、FOK、PostOnly、take_order）均强制检查熔断状态。

## 数据结构

### TradeOrder

```rust
pub struct TradeOrder<T: Config> {
    pub order_id: u64,
    pub entity_id: u64,
    pub maker: T::AccountId,
    pub side: OrderSide,              // Buy | Sell
    pub order_type: OrderType,        // Limit | Market | ImmediateOrCancel | FillOrKill | PostOnly
    pub token_amount: T::TokenBalance,
    pub filled_amount: T::TokenBalance,
    pub price: BalanceOf<T>,          // NEX per Token
    pub status: OrderStatus,          // Open | PartiallyFilled | Filled | Cancelled | Expired
    pub created_at: BlockNumber,
    pub expires_at: BlockNumber,
}
```

### MarketConfig

```rust
pub struct MarketConfig {
    pub nex_enabled: bool,        // 启用 NEX 交易
    pub min_order_amount: u128,   // 最小订单 Token 数量
    pub order_ttl: u32,           // 订单有效期 (区块数, ≥10)
    pub paused: bool,             // 实体级暂停开关
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
    pub circuit_breaker_active: bool,     // 熔断是否激活
    pub circuit_breaker_until: u32,       // 熔断到期区块
    pub initial_price: Option<Balance>,   // 冷启动参考价格
}
```

### TradeRecord

```rust
pub struct TradeRecord<T: Config> {
    pub trade_id: u64,
    pub order_id: u64,
    pub entity_id: u64,
    pub maker: T::AccountId,
    pub taker: T::AccountId,
    pub side: OrderSide,              // 从 taker 视角
    pub token_amount: T::TokenBalance,
    pub price: BalanceOf<T>,
    pub nex_amount: BalanceOf<T>,
    pub block_number: BlockNumberFor<T>,
}
```

### DailyStats

```rust
pub struct DailyStats<Balance> {
    pub open_price: Balance,      // 开盘价
    pub high_price: Balance,      // 最高价
    pub low_price: Balance,       // 最低价
    pub close_price: Balance,     // 收盘价（最新成交价）
    pub volume_nex: u128,         // 24h 成交量 (NEX)
    pub trade_count: u32,         // 24h 成交笔数
    pub period_start: u32,        // 统计起始区块
}
```

## Extrinsics

### 用户交易 (signed)

| Index | 函数 | 说明 |
|-------|------|------|
| 0 | `place_sell_order(entity_id, token_amount, price)` | 卖单（锁定 Token，自动交叉撮合） |
| 1 | `place_buy_order(entity_id, token_amount, price)` | 买单（锁定 NEX，自动交叉撮合） |
| 2 | `take_order(order_id, amount)` | 直接吃单（原子交换，零手续费） |
| 3 | `cancel_order(order_id)` | 取消自己的订单（退还锁定资产） |
| 12 | `market_buy(entity_id, token_amount, max_cost)` | 市价买入（`max_cost` 滑点保护） |
| 13 | `market_sell(entity_id, token_amount, min_receive)` | 市价卖出（`min_receive` 滑点保护） |
| 28 | `batch_cancel_orders(order_ids: BoundedVec<u64, 50>)` | 批量取消自己的订单（≤50 笔，解码阶段限制） |
| 30 | `modify_order(order_id, new_price, new_amount)` | 改价/减量（仅 Open 状态，含市场状态/价格偏离/内幕交易检查） |
| 35 | `cancel_all_entity_orders(entity_id)` | 取消自己在指定实体的所有活跃订单 |
| 38 | `place_ioc_order(entity_id, side, token_amount, price)` | IOC 订单（立即成交或取消） |
| 39 | `place_fok_order(entity_id, side, token_amount, price)` | FOK 订单（全部成交或全部取消） |
| 40 | `place_post_only_order(entity_id, side, token_amount, price)` | Post-Only 订单（仅挂单，拒绝立即撮合） |

### 市场管理 (Entity Owner)

| Index | 函数 | 说明 |
|-------|------|------|
| 4 | `configure_market(entity_id, nex_enabled, min_order_amount, order_ttl)` | 配置市场参数 |
| 15 | `configure_price_protection(entity_id, enabled, max_deviation, max_slippage, threshold, min_trades)` | 配置价格保护 |
| 16 | `lift_circuit_breaker(entity_id)` | 熔断到期后手动解除 |
| 17 | `set_initial_price(entity_id, initial_price)` | TWAP 冷启动参考价（仅首次） |
| 26 | `pause_market(entity_id)` | 暂停实体市场 |
| 27 | `resume_market(entity_id)` | 恢复实体市场（须已暂停） |
| 33 | `set_kyc_requirement(entity_id, min_kyc_level)` | 设置市场最低 KYC 等级 |
| 34 | `close_market(entity_id)` | 永久关闭市场（取消所有订单，退还资产） |

### 管理员 (Root / 治理)

| Index | 函数 | 说明 |
|-------|------|------|
| 23 | `force_cancel_order(order_id)` | 强制取消任意订单 |
| 32 | `global_market_pause(paused)` | 全局市场暂停/恢复 |
| 36 | `governance_configure_market(entity_id, ...)` | 治理级市场配置（绕过 owner 检查） |
| 37 | `force_close_market(entity_id)` | 强制关闭市场 |

### 维护 (任何人可调用)

| Index | 函数 | 说明 |
|-------|------|------|
| 29 | `cleanup_expired_orders(entity_id, max_count)` | 手动清理过期订单（≤100 笔） |

## 存储

### 核心存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextOrderId` | `StorageValue<u64>` | 自增订单 ID |
| `Orders` | `StorageMap<u64 → TradeOrder>` | 订单主数据 |
| `EntitySellOrders` | `StorageMap<u64 → BoundedVec<u64, 1000>>` | 实体卖单 ID 索引 |
| `EntityBuyOrders` | `StorageMap<u64 → BoundedVec<u64, 1000>>` | 实体买单 ID 索引 |
| `UserOrders` | `StorageMap<AccountId → BoundedVec<u64, 100>>` | 用户活跃订单索引 |
| `MarketConfigs` | `StorageMap<u64 → MarketConfig>` | 实体市场配置 |

### 价格与 TWAP

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `BestAsk` | `StorageMap<u64 → Balance>` | 实体最优卖价缓存 |
| `BestBid` | `StorageMap<u64 → Balance>` | 实体最优买价缓存 |
| `LastTradePrice` | `StorageMap<u64 → Balance>` | 最新成交价 |
| `TwapAccumulators` | `StorageMap<u64 → TwapAccumulator>` | TWAP 累积器 |
| `PriceProtection` | `StorageMap<u64 → PriceProtectionConfig>` | 价格保护配置 |

### 交易历史与统计

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextTradeId` | `StorageValue<u64>` | 自增成交 ID |
| `TradeRecords` | `StorageMap<u64 → TradeRecord>` | 成交记录主数据 |
| `UserTradeHistory` | `StorageMap<AccountId → BoundedVec<u64>>` | 用户成交历史（环形覆盖） |
| `EntityTradeHistory` | `StorageMap<u64 → BoundedVec<u64>>` | 实体成交历史（环形覆盖） |
| `UserOrderHistory` | `StorageMap<AccountId → BoundedVec<u64>>` | 用户已完结订单历史（环形覆盖） |
| `MarketStatsStorage` | `StorageMap<u64 → MarketStats>` | 实体市场统计 |
| `EntityDailyStats` | `StorageMap<u64 → DailyStats>` | 实体日 K 线统计 |
| `GlobalStats` | `StorageValue<MarketStats>` | 全局累计统计 |

### 系统状态

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `GlobalMarketPaused` | `StorageValue<bool>` | 全局市场暂停开关 |
| `MarketStatusStorage` | `StorageMap<u64 → MarketStatus>` | 市场状态 (Active/Paused/Closed) |
| `MarketKycRequirement` | `StorageMap<u64 → u8>` | 市场最低 KYC 等级 |
| `OnIdleCursor` | `StorageValue<u64>` | on_idle 过期订单扫描游标 |

## Events

| 事件 | 字段 | 说明 |
|------|------|------|
| `OrderCreated` | order_id, entity_id, maker, side, token_amount, price | 订单已创建 |
| `OrderFilled` | order_id, entity_id, maker, taker, filled_amount, total_next | 订单已成交 |
| `OrderCancelled` | order_id, entity_id | 订单已取消 |
| `OrderModified` | order_id, new_price, new_amount | 订单已修改 |
| `OrderForceCancelled` | order_id | Root 强制取消 |
| `MarketConfigured` | entity_id | 市场配置已更新 |
| `MarketOrderExecuted` | entity_id, trader, side, filled_amount, total_next | 市价单已执行 |
| `TradeExecuted` | trade_id, order_id, entity_id, maker, taker, side, token_amount, price, nex_amount | 成交记录 |
| `TwapUpdated` | entity_id, new_price, twap_1h, twap_24h, twap_7d | TWAP 已更新 |
| `CircuitBreakerTriggered` | entity_id, current_price, twap_7d, deviation_bps, until_block | 熔断已触发 |
| `CircuitBreakerLifted` | entity_id | 熔断已解除 |
| `PriceProtectionConfigured` | entity_id, enabled, max_deviation, max_slippage | 价格保护已配置 |
| `InitialPriceSet` | entity_id, initial_price | 初始价格已设置 |
| `MarketPausedEvent` | entity_id | 实体市场已暂停 |
| `MarketResumedEvent` | entity_id | 实体市场已恢复 |
| `MarketClosed` | entity_id, orders_cancelled | 市场已永久关闭 |
| `MarketForceClosed` | entity_id, orders_cancelled | Root 强制关闭 |
| `AllEntityOrdersCancelled` | entity_id, user, cancelled_count | 用户实体订单全部取消 |
| `KycRequirementSet` | entity_id, min_kyc_level | KYC 准入等级已设置 |
| `ExpiredOrdersCleaned` | entity_id, count, cleaner | 过期订单已清理 |
| `GlobalMarketPauseToggled` | paused | 全局暂停状态变更 |
| `BatchOrdersCancelled` | cancelled_count, failed_count | 批量取消完成 |

## Errors

| 错误 | 说明 |
|------|------|
| `EntityNotFound` | 实体不存在 |
| `NotEntityOwner` | 不是实体所有者 |
| `TokenNotEnabled` | 实体代币未启用 |
| `MarketNotEnabled` | 市场未配置/启用 |
| `OrderNotFound` | 订单不存在 |
| `NotOrderOwner` | 不是订单所有者 |
| `OrderClosed` | 订单已关闭 (Filled/Cancelled/Expired) |
| `InsufficientBalance` | NEX 余额不足 |
| `InsufficientTokenBalance` | Token 余额不足 |
| `AmountTooSmall` | 数量为零或过小 |
| `AmountExceedsAvailable` | 数量超过订单可用 |
| `ZeroPrice` | 价格为零 |
| `OrderBookFull` | 订单簿已满（1000/边） |
| `UserOrdersFull` | 用户订单数已满（100） |
| `CannotTakeOwnOrder` | 不能吃自己的单 |
| `ArithmeticOverflow` | 算术溢出 |
| `OrderSideMismatch` | 订单方向不匹配 |
| `NoOrdersAvailable` | 没有可用订单 |
| `SlippageExceeded` | 滑点超限 |
| `PriceDeviationTooHigh` | 价格偏离参考价过大 |
| `MarketCircuitBreakerActive` | 市场处于熔断状态 |
| `InsufficientTwapData` | TWAP 数据不足 |
| `InvalidBasisPoints` | 基点参数无效 (>10000) |
| `EntityNotActive` | 实体未激活 (Banned/Closed) |
| `OrderTtlTooShort` | 订单 TTL 过短 (<10) |
| `InsiderTradingRestricted` | 内幕人员黑窗口期禁止交易 |
| `EntityLocked` | 实体已被全局锁定 |
| `MarketPaused` | 实体市场已暂停 |
| `GlobalMarketPausedError` | 全局市场已暂停 |
| `OrderAmountBelowMinimum` | 订单数量低于最小值 |
| `CircuitBreakerNotActive` | 熔断未激活（lift 时检查） |
| `ModifyAmountExceedsOriginal` | 修改后数量不得超过原始 |
| `InvalidOrderStatus` | 订单状态无效 |
| `TooManyOrders` | 批量操作数量过多 |
| `InsufficientKycLevel` | KYC 等级不足 |
| `MarketAlreadyClosed` | 市场已永久关闭 |
| `FokNotFullyFillable` | FOK 订单无法全部成交 |
| `PostOnlyWouldMatch` | Post-Only 订单会立即撮合 |
| `InitialPriceAlreadySet` | 初始价格不可重复设置（已有真实成交） |
| `MarketNotPaused` | 市场未暂停（resume 时检查） |

## Runtime 配置

```rust
impl pallet_entity_market::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type Balance = u128;
    type TokenBalance = u128;
    type EntityProvider = EntityRegistry;        // 实体查询/权限
    type TokenProvider = EntityToken;            // 代币余额/锁定/转账
    type DisclosureProvider = EntityDisclosure;  // 内幕交易检查
    type KycProvider = EntityKyc;               // KYC 等级查询
    type PricingProvider = PriceOracle;         // NEX/USDT 价格
    type DefaultOrderTTL = ConstU32<100800>;    // 7 天 (7×24×600)
    type MaxActiveOrdersPerUser = ConstU32<100>;
    type BlocksPerHour = ConstU32<600>;
    type BlocksPerDay = ConstU32<14400>;        // 24×600
    type BlocksPerWeek = ConstU32<100800>;      // 7×24×600
    type CircuitBreakerDuration = ConstU32<600>;// 1h
    type MaxTradeHistoryPerUser = ConstU32<200>;
    type MaxOrderHistoryPerUser = ConstU32<200>;
    type WeightInfo = pallet_entity_market::weights::SubstrateWeight<Runtime>;
}
```

### integrity_test 校验

测试环境下自动校验以下约束：

- `DefaultOrderTTL >= 10`
- `BlocksPerHour > 0`
- `BlocksPerDay > BlocksPerHour`
- `BlocksPerWeek > BlocksPerDay`
- `CircuitBreakerDuration > 0`

## 查询接口

```rust
impl<T: Config> Pallet<T> {
    // 订单簿
    pub fn get_order_book_depth(entity_id: u64, depth: u32) -> OrderBookDepth;
    pub fn get_order_book_snapshot(entity_id: u64) -> (Vec<(Balance, TokenBalance)>, Vec<(Balance, TokenBalance)>);
    pub fn get_sell_orders(entity_id: u64) -> Vec<TradeOrder>;
    pub fn get_buy_orders(entity_id: u64) -> Vec<TradeOrder>;
    pub fn get_user_orders(user: &AccountId) -> Vec<TradeOrder>;

    // 市场信息
    pub fn get_market_summary(entity_id: u64) -> MarketSummary;
    pub fn get_best_prices(entity_id: u64) -> (Option<Balance>, Option<Balance>);
    pub fn get_spread(entity_id: u64) -> Option<Balance>;
    pub fn get_market_status(entity_id: u64) -> MarketStatus;
    pub fn get_kyc_requirement(entity_id: u64) -> u8;

    // TWAP
    pub fn calculate_twap(entity_id: u64, period: TwapPeriod) -> Option<Balance>;

    // 分页历史查询
    pub fn get_user_trade_history(user: &AccountId, page: u32, page_size: u32) -> Vec<TradeRecord>;
    pub fn get_entity_trade_history(entity_id: u64, page: u32, page_size: u32) -> Vec<TradeRecord>;
    pub fn get_user_order_history(user: &AccountId, page: u32, page_size: u32) -> Vec<TradeOrder>;

    // 统计
    pub fn get_daily_stats(entity_id: u64) -> DailyStats;
    pub fn get_global_stats() -> MarketStats;
}
```

## EntityTokenPriceProvider

本模块实现 `EntityTokenPriceProvider` trait，供其他模块查询代币价格：

| 函数 | 说明 |
|------|------|
| `get_token_price(entity_id)` | 优先级: 1h TWAP → LastTradePrice → initial_price |
| `get_token_price_usdt(entity_id)` | Token → NEX → USDT 间接换算 (精度 10^6) |
| `token_price_confidence(entity_id)` | 置信度 0~95: TWAP+活跃=95, TWAP=80, LastTrade=65, initial_price=35, stale≤25 |
| `is_token_price_stale(entity_id, max_age)` | 最后成交距今是否超过 `max_age` 个区块 |

## 安全机制

| 机制 | 说明 |
|------|------|
| **原子交换** | 单笔交易内完成 Token 和 NEX 的双向转移 |
| **价格偏离检查** | 限价单/改单价格不得偏离 TWAP/初始价格超过 `max_price_deviation` |
| **异常价格过滤** | TWAP 累积时偏离上次价格 >100% 的成交价被限幅至 ±50% |
| **熔断机制** | 所有交易入口均强制检查；偏离 7d TWAP 超阈值自动暂停 |
| **滑点保护** | 市价单 `max_cost` / `min_receive` 防止不利成交 |
| **自吃单防护** | `do_cross_match`、`do_market_buy`、`do_market_sell`、`take_order` 四处均跳过自己的订单 |
| **KYC 准入** | 可配置市场最低 KYC 等级，不达标禁止交易 |
| **内幕交易限制** | 黑窗口期内幕人员禁止交易和改单（DisclosureProvider） |
| **改单安全** | `modify_order` 强制检查市场状态 + 内幕交易限制，暂停期间禁止改价 |
| **过期订单过滤** | 撮合前 `get_sorted_orders` 过滤已过期但尚未清理的订单 |
| **on_idle 权重精确** | `consumed_weight` 累加器追踪实际消耗（ref_time + proof_size），防止区块超重 |

## on_idle 自动清理

```
每个区块 on_idle:
  ├── 从 OnIdleCursor 位置开始扫描（每批 200 个 ID）
  ├── 同时检查 ref_time 和 proof_size 预算
  ├── 每块最多清理 20 个过期订单
  ├── 退还锁定资产 (Token/NEX unreserve)
  ├── 更新订单状态为 Expired
  ├── 从订单簿和用户索引中移除
  ├── 添加到已完结订单历史
  ├── 更新受影响实体的 BestAsk/BestBid 缓存
  ├── consumed_weight 精确追踪每次扫描和清理的开销
  └── 游标前进，到达末尾归零循环
```

## 已知技术债

| 项目 | 状态 | 说明 |
|------|------|------|
| Weight benchmarking | 🟡 占位 | 所有 extrinsic 使用硬编码占位值 |
| 订单簿排序 | 🟡 O(N log N) | 每次撮合全量排序，大订单簿时性能待优化 |

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v2.2.0 | 2026-03-05 | **审计 Round 9**: on_idle 权重精确追踪 (consumed_weight 累加器)；on_idle proof_size 维度检查；modify_order 增加市场状态 + 内幕交易检查；batch_cancel_orders 改用 BoundedVec 解码阶段限制；移除所有手续费相关代码和配置 |
| v2.1.0 | 2026-03-05 | **审计 Round 7+8**: 自吃单防护 (market_buy/sell)；熔断器全入口强制检查；on_idle 游标扫描替代 last-1000；on_idle proof_size 权重；resume_market 对称性检查 |
| v2.0.0 | 2026-03-04 | **USDT 通道移除**: 删除所有 USDT 交易/OCW/保证金；新增 IOC/FOK/PostOnly 订单；close_market/force_close_market；cancel_all_entity_orders；KYC 准入；治理级配置；日统计/全局统计；分页查询；成交历史 |
| v1.2.0 | 2026-03-04 | 功能扩展: pause/resume, batch_cancel, cleanup, modify, force_cancel, global_pause |
| v1.1.0 | 2026-02-26 | EntityTokenPriceProvider 实现 (含 USDT 间接换算) |
| v1.0.0 | 2026-02-24 | 架构重构: shop_id → entity_id |
| v0.5.0 | 2026-02-01 | 三周期 TWAP 预言机 + 熔断 |
| v0.3.0 | 2026-02-01 | 市价单 + 滑点保护 |
| v0.1.0 | 2026-02-01 | NEX 限价单 (place_sell/buy, take, cancel) |

## 相关模块

- [pallet-entity-common](../common/) — 共享 Trait 接口（EntityProvider, EntityTokenProvider, DisclosureProvider, KycProvider, PricingProvider）
- [pallet-entity-registry](../registry/) — 实体管理（EntityProvider 实现方）
- [pallet-entity-token](../token/) — 实体代币（EntityTokenProvider 实现方）
- [pallet-entity-disclosure](../disclosure/) — 信息披露（DisclosureProvider 实现方）
- [pallet-entity-kyc](../kyc/) — KYC 管理（KycProvider 实现方）
