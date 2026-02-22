# Pallet Pricing（动态定价与市场统计模块）

## 📋 模块概述

`pallet-pricing` 是 Nexus 区块链的 **动态定价与市场统计模块**，负责聚合 P2P Buy（USDT→NEX）和 Sell（NEX→USDT）两方向的交易数据，计算 NEX/USD 市场参考价格，并提供完整的市场统计信息。

### 核心特性

- ✅ **双方向价格聚合**：同时聚合 P2P Buy 和 Sell 两方向的价格数据
- ✅ **循环缓冲区设计**：最多存储 10,000 笔订单快照，自动滚动更新
- ✅ **交易量限制**：维护最近累计 1,000,000 NEX 的订单统计
- ✅ **加权平均价格**：基于交易量的加权平均，更准确反映市场情况
- ✅ **简单平均价格**：两个方向均价的简单平均，用于快速参考
- ✅ **冷启动保护**：市场初期使用默认价格，达到阈值后自动退出
- ✅ **价格偏离检查**：防止极端价格订单，保护买卖双方利益
- ✅ **治理可调参数**：冷启动阈值、默认价格可通过治理调整
- ✅ **CNY/USDT 汇率**：通过 OCW 每 24 小时自动获取

---

## � 模块依赖关系

```
┌───────────────────────────────────────────────────────────────────────────────────┐
│                     Pricing 模块数据流向 (v1.4.0 P2P 统一)                        │
├───────────────────────────────────────────────────────────────────────────────────┤
│                                                                                 │
│  ┌────────────────────┐                     ┌──────────────────┐               │
│  │   pallet-p2p         │ ─ report_p2p_trade ─→│  pallet-pricing  │               │
│  │  (Buy: USDT→NEX)    │                     │  ┌────────────┐  │               │
│  │  (Sell: NEX→USDT)    │                     │  │ Buy 聚合    │  │               │
│  └────────────────────┘                     │  └────────────┘  │               │
│         ↑                                   │  ┌────────────┐  │←─ 消费方:     │
│         │                                   │  │ Sell 聚合   │  │  pallet-p2p    │
│         └── get_cos_to_usd_rate() ────────│  └────────────┘  │  pallet-maker  │
│                                             └──────────────────┘  arbitration   │
│                                                                  storage-svc  │
│  ┌──────────────────┐                                              entity-*    │
│  │   Exchange API   │ ─ OCW ─ ocw_submit_exchange_rate ─→ CNY/USDT 汇率         │
│  └──────────────────┘                                                           │
└───────────────────────────────────────────────────────────────────────────────────┘
```

### trait 接口 (PricingProvider)

```rust
pub trait PricingProvider<Balance> {
    /// 获取 NEX/USD 汇率（精度 10^6）
    fn get_cos_to_usd_rate() -> Option<Balance>;
    
    /// 上报 P2P 成交到价格聚合（统一 Buy/Sell 两方向）
    fn report_p2p_trade(timestamp: u64, price_usdt: u64, nex_qty: u128) -> DispatchResult;
}
```

---

## � 主要功能

### 1. 价格聚合管理

#### 添加 Buy 方向成交（`add_buy_trade`）

将 Buy 方向（USDT→NEX）成交添加到价格聚合数据。

**流程：**
1. 输入验证（价格 > 0，数量 > 0，单笔 ≤ 1000万 NEX）
2. 如果累计超过 1,000,000 NEX，删除最旧的订单直到满足限制
3. 添加新订单到循环缓冲区（索引 0-9999）
4. 更新聚合统计数据（总 NEX、总 USDT、订单数）
5. 发出 `BuyTradeAdded` 事件

**调用者：** `pallet-p2p`（通过 runtime bridge 直接调用）

**参数：**
- `timestamp`: 成交时间戳（Unix 毫秒）
- `price_usdt`: USDT 单价（精度 10^6）
- `nex_qty`: NEX 数量（精度 10^12）

#### 添加 Sell 方向成交（`add_sell_trade`）

将 Sell 方向（NEX→USDT）成交添加到价格聚合数据。

**流程：** 与 `add_buy_trade` 相同，但操作 Sell 方向的存储。

**调用者：** `pallet-p2p`（通过 `report_p2p_trade` trait 方法）

**上报时机：**
| 路径 | 函数 | 是否上报 |
|------|------|---------|
| P2P 正常完成 | 订单完成回调 | ✅ 上报 `nex_amount` |
| 部分付款接受 | 部分成交回调 | ⚠️ 应上报实际成交量 |
| 超时/取消/退款 | - | ❌ 不上报（未成交） |

**参数：** 与 `add_buy_trade` 相同

---

### 2. 价格查询接口

#### 获取 NEX 市场参考价格（`get_memo_reference_price`）

获取 NEX/USD 市场参考价格（简单平均 + 冷启动保护）。

**算法：**
- **冷启动阶段**：如果两个方向交易量都未达阈值，返回默认价格
- **正常阶段**：
  - 如果两个方向都有数据：`(Buy 均价 + Sell 均价) / 2`
  - 如果只有一个方向有数据：使用该方向的均价
  - 如果都无数据：返回默认价格（兜底）

**返回：** `u64`（USDT/NEX 价格，精度 10^6）

**用途：**
- 前端显示参考价格
- 价格偏离度计算
- 简单的市场概览

#### 获取 NEX 市场价格（`get_cos_market_price_weighted`）

获取 NEX/USD 市场价格（加权平均 + 冷启动保护）。

**算法：**
- **冷启动阶段**：如果两个方向交易量都未达阈值，返回默认价格
- **正常阶段**：加权平均 = `(Buy 总 USDT + Sell 总 USDT) / (Buy 总 NEX + Sell 总 NEX)`

**优点：**
- 考虑交易量权重，更准确反映市场情况
- 大交易量市场的价格权重更高
- 符合市值加权指数的计算方式
- 冷启动保护避免初期价格为 0 或被操纵

**返回：** `u64`（USDT/NEX 价格，精度 10^6）

**用途：**
- 资产估值（钱包总值计算）
- 清算价格参考
- 市场指数计算

#### 获取市场统计信息（`get_market_stats`）

获取完整的 NEX 市场统计信息。

**返回：** `MarketStats` 结构，包含：
- Buy 和 Sell 各自的均价
- 加权平均价格和简单平均价格
- 各方向的交易量和订单数
- 总交易量

**用途：**
- 市场概况 Dashboard
- 价格比较和分析
- 交易量统计
- API 查询接口

---

### 3. 价格偏离检查

#### 检查价格偏离（`check_price_deviation`）

检查订单价格是否在允许的偏离范围内。

**逻辑：**
1. 获取当前市场加权平均价格作为基准价格
2. 验证基准价格有效（> 0）
3. 计算订单价格与基准价格的偏离率（绝对值，单位：bps）
4. 检查偏离率是否超过 `MaxPriceDeviation` 配置的限制

**示例：**
- 基准价格：1.0 USDT/NEX（1,000,000）
- `MaxPriceDeviation`：2000 bps（20%）
- 允许范围：0.8 ~ 1.2 USDT/NEX
- 订单价格 1.1 USDT/NEX → 偏离 10% → 通过 ✅
- 订单价格 1.5 USDT/NEX → 偏离 50% → 拒绝 ❌

**用途：**
- P2P Buy 订单创建时的价格合理性检查
- P2P Sell 订单创建时的价格合理性检查
- 防止极端价格订单，保护买卖双方

---

### 4. 冷启动机制

#### 冷启动保护

为避免市场初期价格为 0 或被操纵，本模块实现了冷启动保护机制。

**机制：**
1. **冷启动阶段**：
   - 如果 Buy 和 Sell 两方向的交易量都低于 `ColdStartThreshold`（默认 1 亿 NEX）
   - 返回 `DefaultPrice`（默认 0.000001 USDT/NEX）
   
2. **退出冷启动**：
   - 当任一方向交易量达到阈值时，自动退出冷启动
   - 设置 `ColdStartExited = true`（单向锁定，不可回退）
   - 发出 `ColdStartExited` 事件
   
3. **正常阶段**：
   - 使用实际市场数据计算价格
   - 不再使用默认价格

#### 治理调整冷启动参数（`set_cold_start_params`）

治理可在冷启动期间调整参数。

**权限：** Root（治理投票）

**参数：**
- `threshold`: 可选，新的冷启动阈值（NEX 数量，精度 10^12）
- `default_price`: 可选，新的默认价格（USDT/NEX，精度 10^6）

**限制：**
- 只能在冷启动期间调整（`ColdStartExited = false`）
- 一旦退出冷启动，无法再调整这些参数

#### 治理紧急重置冷启动（`reset_cold_start`）

在极端市场条件下，允许治理重新进入冷启动状态。

**权限：** Root（治理投票）

**参数：**
- `reason`: 重置原因（最多 256 字节，用于审计和追溯）

**使用场景：**
- 市场崩盘，价格长期失真
- 系统维护，需要暂停市场定价
- 数据异常，需要重新校准

**效果：**
- 将 `ColdStartExited` 设置为 `false`
- 系统将重新使用 `DefaultPrice` 直到市场恢复
- 发出 `ColdStartReset` 事件

**安全考虑：**
- 仅限 Root 权限（通常需要治理投票）
- 不清理历史数据，保留市场记录
- 可多次调用，适应复杂市场环境

---

## 📊 核心数据结构

### OrderSnapshot（订单快照）

```rust
pub struct OrderSnapshot {
    pub timestamp: u64,     // 订单时间戳（Unix 毫秒）
    pub price_usdt: u64,    // USDT 单价（精度 10^6）
    pub cos_qty: u128,     // NEX 数量（精度 10^12）
}
```

### PriceAggregateData（价格聚合数据）

```rust
pub struct PriceAggregateData {
    pub total_cos: u128,      // 累计 NEX 数量（精度 10^12）
    pub total_usdt: u128,      // 累计 USDT 金额（精度 10^6）
    pub order_count: u32,      // 订单数量
    pub oldest_index: u32,     // 最旧订单索引（循环缓冲区指针，0-9999）
    pub newest_index: u32,     // 最新订单索引（循环缓冲区指针，0-9999）
}
```

### MarketStats（市场统计信息）

```rust
pub struct MarketStats {
    pub buy_price: u64,            // Buy 方向均价（精度 10^6）
    pub sell_price: u64,           // Sell 方向均价（精度 10^6）
    pub weighted_price: u64,       // 加权平均价格（精度 10^6）
    pub simple_avg_price: u64,     // 简单平均价格（精度 10^6）
    pub buy_volume: u128,          // Buy 方向交易量（精度 10^12）
    pub sell_volume: u128,         // Sell 方向交易量（精度 10^12）
    pub total_volume: u128,        // 总交易量（精度 10^12）
    pub buy_order_count: u32,      // Buy 方向订单数
    pub sell_order_count: u32,     // Sell 方向订单数
}
```

---

## 🔐 存储结构

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `BuyPriceAggregate` | `PriceAggregateData` | Buy 方向价格聚合数据 |
| `BuyOrderRingBuffer` | `Map<u32, OrderSnapshot>` | Buy 方向订单历史循环缓冲区（0-9999） |
| `SellPriceAggregate` | `PriceAggregateData` | Sell 方向价格聚合数据 |
| `SellOrderRingBuffer` | `Map<u32, OrderSnapshot>` | Sell 方向订单历史循环缓冲区（0-9999） |
| `ColdStartThreshold` | `u128` | 冷启动阈值（默认 1 亿 NEX） |
| `DefaultPrice` | `u64` | 默认价格（默认 0.000001 USDT/NEX） |
| `ColdStartExited` | `bool` | 冷启动退出标记（单向锁定） |

---

## 🎯 事件（Events）

```rust
pub enum Event<T: Config> {
    /// Buy 方向成交添加到价格聚合
    BuyTradeAdded {
        timestamp: u64,
        price_usdt: u64,
        nex_qty: u128,
        new_avg_price: u64,
    },
    
    /// Sell 方向成交添加到价格聚合
    SellTradeAdded {
        timestamp: u64,
        price_usdt: u64,
        nex_qty: u128,
        new_avg_price: u64,
    },
    
    /// 冷启动参数更新事件
    ColdStartParamsUpdated {
        threshold: Option<u128>,
        default_price: Option<u64>,
    },
    
    /// 冷启动退出事件（标志性事件，市场进入正常定价阶段）
    ColdStartExited {
        final_threshold: u128,
        buy_volume: u128,
        sell_volume: u128,
        market_price: u64,
    },
    
    /// 冷启动重置事件（治理紧急恢复机制）
    ColdStartReset {
        reason: BoundedVec<u8, ConstU32<256>>,
    },
}
```

---

## ❌ 错误（Errors）

| 错误 | 说明 |
|------|------|
| `ColdStartAlreadyExited` | 冷启动已退出，无法再调整冷启动参数 |
| `PriceDeviationTooLarge` | 价格偏离过大，超出允许的最大偏离范围 |
| `InvalidBasePrice` | 基准价格无效（为 0 或获取失败） |
| `ColdStartNotExited` | 冷启动未退出，无法重置 |

---

## 🔧 Runtime 配置

```rust
impl pallet_pricing::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    
    // 最大价格偏离（基点，bps）
    // 2000 bps = 20%，表示订单价格不能超过基准价格的 ±20%
    type MaxPriceDeviation = ConstU16<2000>;
}

// 在 construct_runtime! 中添加
construct_runtime! {
    pub struct Runtime {
        // ... 其他模块
        Pricing: pallet_pricing,
    }
}
```

---

## 📱 前端调用示例

### 1. 查询市场价格

```typescript
import { ApiPromise } from '@polkadot/api';

// 获取市场参考价格（简单平均）
async function getReferencePrice(api: ApiPromise) {
  const price = await api.query.pricing.getRemarkablePrice();
  console.log('NEX 市场参考价格:', price.toNumber() / 1_000_000, 'USDT');
}

// 获取市场价格（加权平均）
async function getMarketPrice(api: ApiPromise) {
  const price = await api.query.pricing.getCosMarketPriceWeighted();
  console.log('NEX 市场价格:', price.toNumber() / 1_000_000, 'USDT');
}
```

### 2. 查询市场统计

```typescript
// 获取完整市场统计
async function getMarketStats(api: ApiPromise) {
  const stats = await api.query.pricing.marketStats();
  
  console.log('市场统计:', {
    buyPrice: stats.buyPrice.toNumber() / 1_000_000,
    sellPrice: stats.sellPrice.toNumber() / 1_000_000,
    weightedPrice: stats.weightedPrice.toNumber() / 1_000_000,
    simpleAvgPrice: stats.simpleAvgPrice.toNumber() / 1_000_000,
    buyVolume: stats.buyVolume.toString(),
    sellVolume: stats.sellVolume.toString(),
    totalVolume: stats.totalVolume.toString(),
    buyOrderCount: stats.buyOrderCount.toNumber(),
    sellOrderCount: stats.sellOrderCount.toNumber(),
  });
}
```

### 3. 查询聚合数据

```typescript
// 获取 Buy 方向聚合数据
async function getBuyStats(api: ApiPromise) {
  const aggregate = await api.query.pricing.buyAggregate();
  
  console.log('Buy 方向聚合数据:', {
    totalNex: aggregate.totalNex.toString(),
    totalUsdt: aggregate.totalUsdt.toString(),
    orderCount: aggregate.orderCount.toNumber(),
    oldestIndex: aggregate.oldestIndex.toNumber(),
    newestIndex: aggregate.newestIndex.toNumber(),
  });
  
  // 计算均价
  const avgPrice = aggregate.totalNex.isZero() 
    ? 0 
    : aggregate.totalUsdt.mul(1_000_000_000_000).div(aggregate.totalNex).toNumber();
  console.log('Buy 方向均价:', avgPrice / 1_000_000, 'USDT');
}

// 获取 Sell 方向聚合数据
async function getSellStats(api: ApiPromise) {
  const aggregate = await api.query.pricing.sellAggregate();
  // 类似 Buy 方向的处理
}
```

### 4. 查询冷启动状态

```typescript
// 查询冷启动状态
async function getColdStartStatus(api: ApiPromise) {
  const exited = await api.query.pricing.coldStartExited();
  const threshold = await api.query.pricing.coldStartThreshold();
  const defaultPrice = await api.query.pricing.defaultPrice();
  
  console.log('冷启动状态:', {
    exited: exited.isTrue,
    threshold: threshold.toString(),
    defaultPrice: defaultPrice.toNumber() / 1_000_000,
  });
}
```

### 5. 治理调整冷启动参数

```typescript
import { Keyring } from '@polkadot/keyring';

// 治理调整冷启动参数
async function setColdStartParams(
  api: ApiPromise,
  sudoAccount: KeyringPair,
  threshold?: string,
  defaultPrice?: number
) {
  const tx = api.tx.pricing.setColdStartParams(
    threshold || null,
    defaultPrice ? defaultPrice * 1_000_000 : null
  );
  
  // 需要 Root 权限
  const sudoTx = api.tx.sudo.sudo(tx);
  await sudoTx.signAndSend(sudoAccount);
}
```

### 6. 治理紧急重置冷启动

```typescript
// 治理紧急重置冷启动
async function resetColdStart(
  api: ApiPromise,
  sudoAccount: KeyringPair,
  reason: string
) {
  const reasonBytes = new TextEncoder().encode(reason);
  
  const tx = api.tx.pricing.resetColdStart(reasonBytes);
  const sudoTx = api.tx.sudo.sudo(tx);
  await sudoTx.signAndSend(sudoAccount);
}
```

---

## 🧮 价格计算详解

### 1. 单方向均价计算

```
Buy/Sell 均价 = (总 USDT / 总 NEX)
             = total_usdt / (total_nex / 10^12)
             = (total_usdt * 10^12) / total_nex
```

**示例：**
- 总 USDT：1000（精度 10^6）= 0.001 USDT
- 总 NEX：1,000,000,000,000（精度 10^12）= 1 NEX
- 均价 = (1000 * 10^12) / 1,000,000,000,000 = 1,000,000（精度 10^6）= 1 USDT/NEX

### 2. 加权平均价格计算

```
加权平均价格 = (Buy 总 USDT + Sell 总 USDT) / (Buy 总 NEX + Sell 总 NEX)
```

**示例：**
- Buy 总 USDT：1000（0.001 USDT）
- Buy 总 NEX：1,000,000,000,000（1 NEX）
- Sell 总 USDT：2000（0.002 USDT）
- Sell 总 NEX：1,000,000,000,000（1 NEX）
- 加权平均 = (1000 + 2000) * 10^12 / (1,000,000,000,000 + 1,000,000,000,000) = 1,500,000（1.5 USDT/NEX）

### 3. 简单平均价格计算

```
简单平均价格 = (Buy 均价 + Sell 均价) / 2
```

**示例：**
- Buy 均价：1,000,000（1 USDT/NEX）
- Sell 均价：2,000,000（2 USDT/NEX）
- 简单平均 = (1,000,000 + 2,000,000) / 2 = 1,500,000（1.5 USDT/NEX）

### 4. 价格偏离计算

```
偏离率（bps）= |订单价格 - 基准价格| / 基准价格 × 10000
```

**示例：**
- 基准价格：1,000,000（1 USDT/NEX）
- 订单价格：1,200,000（1.2 USDT/NEX）
- 偏离率 = (1,200,000 - 1,000,000) / 1,000,000 × 10000 = 2000 bps = 20%

---

## 🛡️ 安全考虑

### 1. 冷启动保护

- ✅ **默认价格锚点**：避免市场初期价格为 0 或被操纵
- ✅ **单向锁定**：一旦退出冷启动，不可回退（除非治理重置）
- ✅ **治理可调**：冷启动参数可通过治理调整

### 2. 循环缓冲区

- ✅ **自动滚动**：最多存储 10,000 笔订单，自动删除最旧的
- ✅ **交易量限制**：维护最近累计 1,000,000 NEX 的订单
- ✅ **防止存储膨胀**：存储空间固定，不会无限增长

### 3. 价格偏离检查

- ✅ **极端价格保护**：防止恶意或错误的极端价格订单
- ✅ **可配置阈值**：`MaxPriceDeviation` 可通过 Runtime 配置调整
- ✅ **双向保护**：溢价和折价都受限

### 4. 计算溢出保护

- ✅ **饱和运算**：使用 `saturating_*` 方法防止溢出
- ✅ **检查除零**：计算均价前验证分母不为 0
- ✅ **精度转换**：谨慎处理 `u64` 和 `u128` 之间的转换

---

## 📊 循环缓冲区详解

### 设计原理

```text
循环缓冲区索引：0 ───► 9999 ───► 0（循环）
                   ▲           │
                   │           │
              oldest_index   newest_index
```

### 添加订单流程

```text
初始状态：
- oldest_index = 0
- newest_index = 0
- order_count = 0

添加第 1 笔订单：
- 写入索引 0
- newest_index = 0
- order_count = 1

添加第 2 笔订单：
- 写入索引 1
- newest_index = 1
- order_count = 2

...

添加第 10,001 笔订单：
- 累计 NEX 超过 1,000,000 限制
- 删除索引 0 的订单
- oldest_index = 1
- 写入索引 1（覆盖）
- newest_index = 1
- order_count = 10000
```

### 限制机制

```rust
// 当累计 NEX 超过 1,000,000 时
while new_total > limit && agg.order_count > 0 {
    // 删除最旧的订单
    let oldest = BuyOrderRingBuffer::<T>::take(agg.oldest_index);
    // 从聚合数据中减去
    agg.total_nex -= oldest.nex_qty;
    agg.total_usdt -= oldest.nex_qty / 10^12 * oldest.price_usdt;
    agg.order_count -= 1;
    // 移动最旧索引
    agg.oldest_index = (agg.oldest_index + 1) % 10000;
}
```

---

## 🌐 CNY/USDT 汇率 (Offchain Worker)

### 功能说明

通过 Offchain Worker 每 24 小时自动从外部 API 获取 CNY/USD 汇率。

**API 来源：** `https://api.exchangerate-api.com/v4/latest/USD`

### 汇率存储结构

```rust
pub struct ExchangeRateData {
    /// CNY/USD 汇率（精度 10^6，如 7.2345 → 7_234_500）
    pub cny_rate: u64,
    /// 更新时间戳（Unix 秒）
    pub updated_at: u64,
}
```

### 汇率验证

- 汇率范围限制：5.0 ~ 10.0 CNY/USD
- 超出范围的汇率将被拒绝

### 相关事件

```rust
ExchangeRateUpdated {
    cny_rate: u64,
    updated_at: u64,
    block_number: BlockNumberFor<T>,
}
```

---

## 📚 相关模块

| 模块 | 调用方向 | 接口 |
|------|---------|------|
| `pallet-p2p` | → Pricing | `add_buy_trade()` / `add_sell_trade()` |
| `pallet-p2p` | ← Pricing | `get_cos_to_usd_rate()` |
| `pallet-maker` | ← Pricing | `get_cos_to_usd_rate()` |
| `pallet-arbitration` | ← Pricing | `get_cos_to_usd_rate()` |
| `pallet-storage-service` | ← Pricing | `DepositCalculatorImpl` |
| `pallet-entity-*` | ← Pricing | `EntityPricingProvider` bridge |
| `pallet-common` | 定义 | `PricingProvider` trait |

---

## 🚀 版本历史

| 版本 | 日期 | 说明 |
|------|------|------|
| v1.0.0 | 2025-11-04 | 初始版本，支持双市场价格聚合和冷启动保护 |
| v1.1.0 | 2025-11-04 | 添加治理紧急重置冷启动功能（M-3 修复） |
| v1.2.0 | 2026-01-18 | 添加 CNY/USDT 汇率 OCW 功能 |
| v1.3.0 | 2026-02-04 | 优化冷启动检查，避免重复触发事件 |
| v1.4.0 | 2026-02-08 | 适配 P2P 统一模型：OTC→Buy, Bridge→Sell, report_swap_order→report_p2p_trade |

---

## ⚠️ 已知问题 (待修复)

### P2P 部分付款路径未上报

**问题描述：**
- 部分付款接受路径完成交易但未上报 Pricing

**影响：**
- 价格聚合数据可能缺少部分成交量
- VWAP 计算可能存在偏差

**建议修复：**
在部分付款接受函数中添加 `T::Pricing::report_p2p_trade()` 调用，按实际成交的 NEX 数量上报。
