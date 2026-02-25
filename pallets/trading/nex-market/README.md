# pallet-nex-market

NEX/USDT P2P 交易市场模块 — 无做市商订单簿模型。

## 概述

`pallet-nex-market` 是一个去中心化的 NEX ↔ USDT 交易市场，任何人均可挂单/吃单，无需做市商。
基于 `pallet-entity-market` 的成熟模式，针对全局单一交易对（NEX/USDT）进行了简化和优化。

### 核心特性

| 特性 | 说明 |
|------|------|
| **订单簿** | 限价卖单 + 限价买单，按价格优先排序 |
| **USDT 通道** | TRC20 链下支付 + OCW 自动验证 |
| **多档判定** | Exact / Overpaid / Underpaid / SeverelyUnderpaid / Invalid |
| **买家保证金** | 防止不付款风险，超时/少付自动没收 |
| **TWAP 预言机** | 1h / 24h / 7d 三周期时间加权平均价格 |
| **价格保护** | 限价单偏离检查 + 熔断机制 |
| **OCW 验证奖励** | 激励节点参与 USDT 交易验证 |

## 交易流程

### 卖 NEX（卖家锁 NEX，收 USDT）

```
1. 卖家调用 place_sell_order(nex_amount, usdt_price, tron_address)
   → NEX 被 reserve 锁定
2. 买家调用 reserve_sell_order(order_id, amount?)
   → 锁定买家保证金 → 创建 UsdtTrade
3. 买家链下转 USDT 到卖家 TRON 地址
4. 买家调用 confirm_payment(trade_id, tron_tx_hash)
   → 状态变更为 AwaitingVerification
5. OCW 验证 TRON 交易 → submit_ocw_result(trade_id, actual_amount)
6. 任何人调用 claim_verification_reward(trade_id)
   → 根据多档判定处理：释放 NEX / 退还保证金 / 没收保证金
```

### 买 NEX（买家挂单，卖家接单）

```
1. 买家调用 place_buy_order(nex_amount, usdt_price)
   → 无链上锁定
2. 卖家调用 accept_buy_order(order_id, amount?, tron_address)
   → 锁定卖家 NEX + 锁定买家保证金 → 创建 UsdtTrade
3-6. 同上（买家转 USDT → OCW 验证 → 结算）
```

## Extrinsics

| # | 调用 | 说明 |
|---|------|------|
| 0 | `place_sell_order` | 挂卖单（锁 NEX） |
| 1 | `place_buy_order` | 挂买单（声明意向） |
| 2 | `cancel_order` | 取消订单（退还锁定） |
| 3 | `reserve_sell_order` | 买家吃卖单（锁保证金） |
| 4 | `accept_buy_order` | 卖家接买单（锁 NEX + 保证金） |
| 5 | `confirm_payment` | 买家提交 TRON tx hash |
| 6 | `process_timeout` | 处理超时交易 |
| 7 | `submit_ocw_result` | OCW 提交验证结果（unsigned） |
| 8 | `claim_verification_reward` | 领取验证奖励 |
| 9 | `configure_price_protection` | 配置价格保护（Root） |
| 10 | `set_initial_price` | 设置初始价格（Root） |
| 11 | `lift_circuit_breaker` | 解除熔断（Root） |

## 存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextOrderId` | `u64` | 订单 ID 计数器 |
| `Orders` | `Map<u64, Order>` | 订单映射 |
| `SellOrders` | `BoundedVec<u64>` | 卖单索引 |
| `BuyOrders` | `BoundedVec<u64>` | 买单索引 |
| `UserOrders` | `Map<AccountId, BoundedVec<u64>>` | 用户订单索引 |
| `NextUsdtTradeId` | `u64` | USDT 交易 ID 计数器 |
| `UsdtTrades` | `Map<u64, UsdtTrade>` | USDT 交易映射 |
| `PendingUsdtTrades` | `BoundedVec<u64>` | OCW 待验证队列 |
| `OcwVerificationResults` | `Map<u64, (Result, u64)>` | OCW 验证结果 |
| `BestAsk` / `BestBid` | `Option<u64>` | 最优价格 |
| `LastTradePrice` | `Option<u64>` | 最新成交价 |
| `MarketStatsStore` | `MarketStats` | 市场统计 |
| `TwapAccumulatorStore` | `TwapAccumulator` | TWAP 累积器 |
| `PriceProtectionStore` | `PriceProtectionConfig` | 价格保护配置 |

## 多档判定逻辑

| 实际金额 | 结果 | 处理 |
|----------|------|------|
| ≥ 100.5% | Overpaid | 全额释放 NEX，退还保证金 |
| 99.5% ~ 100.5% | Exact | 全额释放 NEX，退还保证金 |
| 50% ~ 99.5% | Underpaid | 按比例释放 NEX，没收保证金 |
| < 50% | SeverelyUnderpaid | 按比例释放 NEX，没收保证金 |
| = 0 | Invalid | 不释放 NEX，没收保证金 |

## 价格精度

- **NEX**: 10^12（链上原生代币精度）
- **USDT**: 10^6（TRC20 标准精度）
- **usdt_price**: USDT per NEX，精度 10^6（例: 500_000 = 0.5 USDT/NEX）

## Config 参数

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `DefaultOrderTTL` | `u32` | 24h | 订单有效期 |
| `MaxActiveOrdersPerUser` | `u32` | 100 | 每用户最大活跃订单 |
| `FeeRate` | `u16` | 100 (1%) | 手续费率 bps |
| `UsdtTimeout` | `u32` | 12h | USDT 交易超时 |
| `BuyerDepositRate` | `u16` | 1000 (10%) | 保证金比例 bps |
| `MinBuyerDeposit` | `Balance` | 10 NEX | 最低保证金 |
| `DepositForfeitRate` | `u16` | 10000 (100%) | 没收比例 bps |
| `UsdtToNexRate` | `u64` | 10B | 保证金换算汇率 |
| `VerificationReward` | `Balance` | 0.1 NEX | OCW 验证奖励 |
| `CircuitBreakerDuration` | `u32` | 1h | 熔断持续时间 |

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

## 测试

```bash
cargo test -p pallet-nex-market
```

28 个测试覆盖：
- 卖单 / 买单创建与取消
- reserve_sell_order / accept_buy_order
- confirm_payment 流程
- 完整交易流程（精确付款）
- 少付 / 严重少付自动处理
- 超时退款 + 保证金没收
- 价格保护 + 偏离检查
- 最优价格更新
- 多档判定逻辑
- 市场统计
