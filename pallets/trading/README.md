# Trading 模块集群

## 模块概述

Nexus 交易系统采用模块化设计，包含以下子模块：

| 模块 | 路径 | 说明 | Runtime Index |
|------|------|------|:---:|
| **pallet-nex-market** | `nex-market/` | NEX/USDT 无做市商 P2P 订单簿 | 56 |
| **pallet-trading-common** | `common/` | 公共 Trait 定义（PricingProvider, DepositCalculator） | — |
| **pallet-trading-trc20-verifier** | `trc20-verifier/` | TRC20 交易链下验证共享库 | — |

> **历史变更**：`pallet-trading-pricing`、`pallet-trading-credit`、`pallet-trading-maker`、`pallet-trading-p2p` 已于 2026-02-23 废弃删除，由 `pallet-nex-market` 替代。做市商模式已移除，所有用户均可自由挂单/吃单。

---

## 架构设计

```text
┌───────────────────────────────────────────┐
│              Runtime (index)              │
│                                           │
│           NexMarket(56)                   │
└──────────────┬────────────────────────────┘
               │
               ▼
┌──────────────────────────────────────────┐
│          pallet-nex-market               │
│  NEX/USDT 订单簿（无做市商）              │
│                                          │
│  卖单: 锁 NEX → 买家付 USDT → OCW 验证   │
│  买单: 声明意向 → 卖家接单 → USDT 验证    │
│                                          │
│  TWAP 预言机 · 熔断机制 · 买家保证金      │
│  多档金额判定 · OCW 验证奖励              │
└──────────────┬───────────────────────────┘
               │
     ┌─────────┼─────────┐
     ▼                   ▼
pallet-trading     pallet-trading
  -common            -trc20-verifier
  PricingProvider    TRC20 链下验证
  DepositCalculator
```

### 模块依赖关系

```text
pallet-nex-market
  ├── Currency (原生 NEX 锁定/转账)
  └── OCW → pallet-trading-trc20-verifier (TRC20 验证)

pallet-trading-common（被外部模块使用）
  ├── pallet-arbitration → PricingProvider (投诉押金换算)
  ├── pallet-storage-service → PricingProvider + DepositCalculator (保证金计算)
  └── pallet-entity-* → PricingProvider (Entity 定价)

TradingPricingProvider（runtime 适配器）
  └── 读取 pallet-nex-market::LastTradePrice / PriceProtectionStore
```

---

## pallet-nex-market（核心交易模块）

无做市商的 NEX/USDT 订单簿交易市场。任何人可挂单/吃单。

### 交易流程

#### 卖 NEX（卖家锁 NEX，收 USDT）

```
1. place_sell_order(nex_amount, usdt_price, tron_address) → NEX 被 reserve
2. 买家 reserve_sell_order(order_id) → 锁定买家保证金，创建 UsdtTrade
3. 买家链下转 USDT → confirm_payment(trade_id, tx_hash)
4. OCW 验证 → submit_ocw_result(trade_id, actual_amount)
5. claim_verification_reward(trade_id) → 多档判定处理
```

#### 买 NEX（买家挂单，卖家接单）

```
1. place_buy_order(nex_amount, usdt_price) → 无链上锁定
2. 卖家 accept_buy_order(order_id, tron_address) → 锁 NEX + 买家保证金
3-5. 同上（买家转 USDT → OCW 验证 → 结算）
```

### Extrinsics

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
| 9 | `configure_price_protection` | 配置价格保护（MarketAdmin） |
| 10 | `set_initial_price` | 设置初始价格（MarketAdmin） |
| 11 | `lift_circuit_breaker` | 解除熔断（MarketAdmin） |
| 12 | `seed_liquidity` | 注入流动性种子（MarketAdmin） |

### 多档判定逻辑

| 实际金额 | 结果 | 处理 |
|----------|------|------|
| ≥ 100.5% | Overpaid | 全额释放 NEX，退还保证金 |
| 99.5% ~ 100.5% | Exact | 全额释放 NEX，退还保证金 |
| 50% ~ 99.5% | Underpaid | 按比例释放 NEX，没收保证金 |
| < 50% | SeverelyUnderpaid | 按比例释放 NEX，没收保证金 |
| = 0 | Invalid | 不释放 NEX，没收保证金 |

### 存储概览

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
| `CompletedBuyers` | `Map<AccountId, bool>` | 已完成首笔交易的买家（L3 Sybil 防御） |
| `ActiveWaivedTrades` | `Map<AccountId, u32>` | 买家活跃免保证金交易数（L2 防 grief） |
| `CumulativeSeedUsdtSold` | `u64` | seed_liquidity 累计成交 USDT 总额（审计对账用） |
| `CumulativeSeedNexLocked` | `Balance` | seed_liquidity 累计注入 NEX 总量（总预算上限检查） |

### 配置参数

```rust
impl pallet_nex_market::Config for Runtime {
    type Currency = Balances;
    type WeightInfo = ();
    type DefaultOrderTTL = ConstU32<{ 24 * HOURS }>;
    type MaxActiveOrdersPerUser = ConstU32<100>;
    type FeeRate = ConstU16<100>;                   // 1%
    type UsdtTimeout = ConstU32<{ 12 * HOURS }>;
    type BlocksPerHour = ConstU32<{ 1 * HOURS }>;
    type BlocksPerDay = ConstU32<{ 24 * HOURS }>;
    type BlocksPerWeek = ConstU32<{ 7 * DAYS }>;
    type CircuitBreakerDuration = ConstU32<{ 1 * HOURS }>;
    type VerificationReward = ConstU128<{ UNIT / 10 }>;     // 0.1 NEX
    type RewardSource = NexMarketRewardSource;
    type BuyerDepositRate = ConstU16<1000>;                  // 10%
    type MinBuyerDeposit = ConstU128<{ 10 * UNIT }>;         // 10 NEX
    type DepositForfeitRate = ConstU16<10000>;               // 100%
    type UsdtToNexRate = ConstU64<10_000_000_000>;           // 1 USDT = 10 NEX
    type TreasuryAccount = NexMarketTreasuryAccount;
    type SeedLiquidityAccount = NexMarketSeedAccount; // 独立种子账户
    type FeeRecipient = NexMarketFeeRecipient;
    type MarketAdminOrigin = EnsureProportionAtLeast<     // TreasuryCouncil 2/3 多数
        AccountId, TreasuryCollectiveInstance, 2, 3
    >;
    type FirstOrderTimeout = ConstU32<{ 1 * HOURS }>;    // 免保证金短超时 1h
    type MaxFirstOrderAmount = ConstU128<{ 100 * UNIT }>; // 免保证金单笔上限 100 NEX
    type MaxWaivedSeedOrders = ConstU32<20>;              // seed_liquidity 单次最多 20 笔
    type SeedPricePremiumBps = ConstU16<2000>;             // seed 溢价 20%
    type MaxSeedTotalNex = ConstU128<{ 10_000 * UNIT }>;   // 种子总预算 10,000 NEX
}
```

### 测试

```bash
cargo test -p pallet-nex-market    # 43 个单元测试
```

---

## pallet-trading-common（公共 Trait 库）

纯 Rust crate（非 FRAME pallet），提供跨模块共享的 Trait 接口和工具函数。

### 保留的 Trait

| Trait | 说明 | 使用者 |
|-------|------|--------|
| `PricingProvider<Balance>` | NEX/USD 汇率查询 | arbitration, storage-service, entity-* |
| `DepositCalculator<Balance>` | 统一保证金计算（基于 USD 价值） | storage-service |
| `DepositCalculatorImpl` | DepositCalculator 的默认实现 | runtime 配置 |

### 保留的工具

| 模块 | 说明 |
|------|------|
| `types.rs` | TronAddress, MomentOf, Cid, TxHash |
| `mask.rs` | 数据脱敏函数（姓名、身份证、生日） |
| `validation.rs` | TRON 地址格式验证 |
| `time.rs` | 区块数 ↔ 秒数转换工具 |

详见 [common/README.md](common/README.md)。

---

## pallet-trading-trc20-verifier（TRC20 验证库）

纯 Rust crate，供 OCW 在链下验证 USDT TRC20 转账。

### 核心功能

- **TRC20 交易验证**：通过 TronGrid API 验证 USDT 转账
- **端点健康评分**：动态评估 API 端点可用性
- **并行竞速 / 串行故障转移**：多端点请求策略
- **金额匹配判定**：Exact / Overpaid / Underpaid / SeverelyUnderpaid / Invalid

详见 [trc20-verifier/README.md](trc20-verifier/README.md)。

---

## 前端调用示例

### 卖 NEX（NEX → USDT）

```typescript
import { ApiPromise } from '@polkadot/api';

// 挂卖单
await api.tx.nexMarket.placeSellOrder(nexAmount, usdtPrice, tronAddress).signAndSend(seller);

// 查询卖单列表
const sellOrders = await api.query.nexMarket.sellOrders();

// 买家吃卖单
await api.tx.nexMarket.reserveSellOrder(orderId).signAndSend(buyer);

// 买家链下转 USDT 后提交 tx hash
await api.tx.nexMarket.confirmPayment(tradeId, tronTxHash).signAndSend(buyer);

// 任何人领取验证奖励（触发结算）
await api.tx.nexMarket.claimVerificationReward(tradeId).signAndSend(anyone);
```

### 买 NEX（USDT → NEX）

```typescript
// 挂买单
await api.tx.nexMarket.placeBuyOrder(nexAmount, usdtPrice).signAndSend(buyer);

// 卖家接买单
await api.tx.nexMarket.acceptBuyOrder(orderId, tronAddress).signAndSend(seller);

// 后续同卖单流程（confirm_payment → OCW → claim_reward）
```

### 管理操作（TreasuryCouncil 2/3 多数审批）

```typescript
// 注入流动性种子（市场冷启动）
const orders = [
  [100 * UNIT, 500_000, tronAddress],  // 100 NEX @ 0.5 USDT
  [200 * UNIT, 600_000, tronAddress],  // 200 NEX @ 0.6 USDT
];
await api.tx.council.propose(threshold, api.tx.nexMarket.seedLiquidity(orders), lengthBound);

// 设置初始价格
await api.tx.council.propose(threshold, api.tx.nexMarket.setInitialPrice(price), lengthBound);

// 配置价格保护
await api.tx.council.propose(threshold, api.tx.nexMarket.configurePriceProtection(...), lengthBound);

// 解除熔断
await api.tx.council.propose(threshold, api.tx.nexMarket.liftCircuitBreaker(), lengthBound);
```

> **注意**：所有管理操作已从 `Root` 迁移至 `MarketAdminOrigin`（TreasuryCouncil 2/3 多数），无需 sudo 权限。

---

## 安全设计

- **买家保证金**：USDT 通道锁定 NEX 保证金，超时/少付自动没收
- **防重放**：TRON tx hash 唯一性检查，防止重复提交
- **TWAP 预言机**：三周期（1h/24h/7d）时间加权平均价格，防操纵
- **熔断机制**：价格偏离 7d TWAP 超阈值自动暂停交易
- **多档判定**：OCW 验证实际付款金额，自动按比例处理
- **OCW 验证奖励**：激励任何人触发验证确认
- **委员会治理**：管理操作由 TreasuryCouncil 2/3 多数审批，无需 Root
- **免保证金首单（seed_liquidity）**：多层防御机制
  - **L0 定价**：保护性瀑布式基准价 + 20% 溢价下限
    - 成熟期（≥100 笔）：7d TWAP
    - 过渡期（≥30 笔）：max(24h TWAP, InitialPrice)（只涨不跌）
    - 冷启动（<30 笔）：InitialPrice 兗底
  - **L1 总量**：种子总预算 10,000 NEX，独立种子账户与国库分离
  - **L2 防 Grief**：单笔上限 100 NEX、每账户最多 1 笔活跃、短超时 1h
  - **L3 防 Sybil**：完成首笔交易后标记 `CompletedBuyers`，不再享受免保证金
  - **审计**：`CumulativeSeedUsdtSold` / `CumulativeSeedNexLocked` 链上对账

---

## 版本历史

| 版本 | 日期 | 说明 |
|------|------|------|
| v2.2.0 | 2026-02-24 | 保护性瀑布式 seed 定价（7d TWAP + 溢价下限 + 总预算）+ 独立种子账户 |
| v2.1.0 | 2026-02-24 | seed_liquidity 流动性注入 + 委员会治理 + L2/L3 防 grief 防 Sybil |
| v2.0.0 | 2026-02-23 | 架构重构：删除 pricing/credit/maker/p2p，新增 pallet-nex-market 替代 |
| v1.1.0 | 2026-02-23 | pallet-trading-p2p 新增 Instant Buy（已废弃） |
| v1.0.0 | 2026-02-08 | pallet-trading-p2p 合并 OTC + Swap（已废弃） |
| v0.1.0 | 2025-11-03 | 初始版本，拆分为 maker/otc/swap/common |

---

**License**: Unlicense
