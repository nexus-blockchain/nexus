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
  └── pallet-entity-service   → PricingProvider (服务押金)

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
  │                 reserve_sell_order ──→ 锁保证金  │
  │                 链下转 USDT → confirm_payment    │
  │                        │           submit_ocw_result (unsigned)
  │               claim_verification_reward         │
  │                 ← 释放 NEX + 退保证金           │
```

#### 买 NEX（买家挂单，卖家接单）

```text
买家                      卖家                     OCW
  │                        │                        │
  │ place_buy_order        │                        │
  │               accept_buy_order ──→ 锁 NEX + 锁买家保证金
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

### Extrinsics

| # | 调用 | 权限 | 说明 |
|---|------|------|------|
| 0 | `place_sell_order` | 签名 | 挂卖单（锁 NEX，提供 TRON 收款地址） |
| 1 | `place_buy_order` | 签名 | 挂买单（声明意向，无链上锁定） |
| 2 | `cancel_order` | Owner | 取消订单（退还锁定） |
| 3 | `reserve_sell_order` | 签名 | 买家吃卖单（锁保证金，创建 UsdtTrade） |
| 4 | `accept_buy_order` | 签名 | 卖家接买单（锁 NEX + 锁买家保证金） |
| 5 | `confirm_payment` | 买家 | 提交 TRON tx hash，声明已付款 |
| 6 | `process_timeout` | 任何人 | 处理超时（AwaitingPayment / AwaitingVerification） |
| 7 | `submit_ocw_result` | Unsigned | OCW 提交 TRC20 验证结果 |
| 8 | `claim_verification_reward` | 任何人 | 领取 OCW 验证奖励（触发结算） |
| 9 | `configure_price_protection` | MarketAdmin | 配置价格保护参数 |
| 10 | `set_initial_price` | MarketAdmin | 设置初始基准价格（TWAP 冷启动） |
| 11 | `lift_circuit_breaker` | MarketAdmin | 手动解除熔断 |
| 13 | `fund_seed_account` | MarketAdmin | 国库 → 种子账户注资 |
| 14 | `seed_liquidity` | MarketAdmin | 批量挂免保证金卖单（冷启动引流） |
| 15 | `auto_confirm_payment` | Unsigned | OCW 预检：买家忘记 confirm 时自动确认 |
| 16 | `submit_underpaid_update` | Unsigned | OCW 补付窗口内更新累计金额 |
| 17 | `finalize_underpaid` | 任何人 | 补付窗口到期后终裁 |

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

### OCW 三阶段工作流

`on_idle` 每区块刷新 TWAP 累积器，`offchain_worker` 执行链下验证：

| 阶段 | 扫描队列 | 触发条件 | 动作 |
|------|---------|---------|------|
| 1. 正常验证 | `PendingUsdtTrades` | AwaitingVerification | TronGrid 验证 → `submit_ocw_result` |
| 2. 补付扫描 | `PendingUnderpaidTrades` | UnderpaidPending | 检查新转账 → `submit_underpaid_update` |
| 3. 预检兜底 | `AwaitingPaymentTrades` | AwaitingPayment 超 50% 超时期 | 检测到账 → `auto_confirm_payment` |

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
| `PendingUsdtTrades` | `BoundedVec<u64, 100>` | OCW 待验证队列 |
| `AwaitingPaymentTrades` | `BoundedVec<u64, 100>` | 待付款跟踪队列（OCW 预检） |
| `PendingUnderpaidTrades` | `BoundedVec<u64, 100>` | 少付补付跟踪队列 |
| `OcwVerificationResults` | `Map<u64, (Result, u64)>` | OCW 验证结果 |
| `BestAsk` / `BestBid` | `Option<u64>` | 最优买卖价 |
| `LastTradePrice` | `Option<u64>` | 最新成交价 |
| `MarketStatsStore` | `MarketStats` | 累计统计（订单数/交易数/USDT 成交量） |
| `TwapAccumulatorStore` | `TwapAccumulator` | TWAP 累积器（三周期快照） |
| `PriceProtectionStore` | `PriceProtectionConfig` | 价格保护 + 熔断配置 |
| `CompletedBuyers` | `Map<AccountId, bool>` | 已完成首单的买家（L3 Sybil 防御） |
| `ActiveWaivedTrades` | `Map<AccountId, u32>` | 活跃免保证金交易数（L2 防 grief） |
| `CumulativeSeedUsdtSold` | `ValueQuery<u64>` | seed 累计成交 USDT 总额（审计对账） |

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

| Trait | 说明 | 消费方 |
|-------|------|--------|
| `PricingProvider<Balance>` | NEX/USD 底层汇率查询（精度 10^6） | arbitration, storage-service, entity-* |
| `PriceOracle` | TWAP 预言机（1h/24h/7d）+ 陈旧检测 + 交易量 | Runtime 桥接 |
| `ExchangeRateProvider` | 带置信度(0-100)的统一兑换比率 | 佣金换算、打赏定价等 |
| `DepositCalculator<Balance>` | 统一保证金计算（基于 USD 价值） | storage-service |
| `DepositCalculatorImpl<P, B>` | DepositCalculator 泛型默认实现 | Runtime 配置 |

#### 价格置信度等级（ExchangeRateProvider）

| 置信度 | 数据来源 | 含义 |
|--------|---------|------|
| 90-100 | TWAP + 高交易量(≥100笔) | 可充分信赖 |
| 60-89 | TWAP 或 LastTradePrice | 一般可信 |
| 30-59 | 仅 initial_price（冷启动） | 谨慎使用 |
| 0-29 | 过时或不可用 | 应使用兜底值 |

### 共享类型（types.rs）

| 类型 | 说明 |
|------|------|
| `TronAddress` | BoundedVec<u8, 34>，TRON Base58 地址 |
| `TronTxHash` | BoundedVec<u8, 64>，TRON 交易哈希 |
| `MomentOf` | u64，Unix 秒时间戳 |
| `Cid` | BoundedVec<u8, 64>，IPFS CID |
| `UsdtTradeStatus` | 共享状态枚举（nex-market / entity-market） |
| `BuyerDepositStatus` | 保证金状态枚举（None/Locked/Released/Forfeited/PartiallyForfeited） |
| `PaymentVerificationResult` | 多档判定结果（Exact/Overpaid/Underpaid/SeverelyUnderpaid/Invalid） |

### 共享工具函数

| 函数 | 说明 |
|------|------|
| `calculate_payment_verification_result(expected, actual)` | 多档判定（u32 ratio，防 u16 截断） |
| `compute_payment_ratio_bps(expected, actual)` | 付款比例（basis points，u32 返回） |
| `calculate_deposit_forfeit_rate(ratio)` | 保证金梯度没收比例 |

### 工具模块

| 模块 | 说明 |
|------|------|
| `mask.rs` | 数据脱敏（`mask_name`, `mask_id_card`, `mask_birthday`） |
| `validation.rs` | TRON 地址格式验证（`is_valid_tron_address`） |
| `time.rs` | 区块数 ↔ 秒数转换（`blocks_to_seconds`, `seconds_to_blocks`, `format_duration`） |
| `macros.rs` | 公共宏定义 |

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
const stats = await api.query.nexMarket.marketStats();           // 市场统计
const twap = await api.query.nexMarket.twapAccumulator();        // TWAP 累积器
```

> **注意**：所有管理操作由 `MarketAdminOrigin`（TreasuryCouncil 2/3 多数）审批，无需 sudo 权限。

---

## 安全设计

### 交易安全

- **买家保证金**：USDT 通道锁定 NEX 保证金，超时/少付自动梯度没收
- **补付窗口**：少付 50%-99.5% 给予 2h 补付时间，避免因网络延迟误判
- **防重放**：TRON tx hash 唯一性检查，防止重复提交
- **OCW 验证奖励**：0.1 NEX/次，激励任何人触发验证确认
- **预检兜底**：OCW 自动检测买家忘记 `confirm_payment` 的情况

### 价格安全

- **TWAP 预言机**：三周期（1h/24h/7d）时间加权平均价格，平滑单笔极端成交
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
