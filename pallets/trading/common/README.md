# Trading Common（交易公共工具库）

## 概述

`pallet-trading-common` 是交易系统的公共工具库，提供共享 Trait 接口、类型定义、数据脱敏函数和验证工具。

### 特点

- ✅ **纯 Rust crate**：无链上存储，仅提供工具函数和类型定义
- ✅ **跨模块共享**：被 arbitration、storage-service、entity-* 等多个 pallet 引用
- ✅ **no_std 兼容**：支持 WebAssembly 运行时环境

## 模块结构

```
pallets/trading/common/
├── src/
│   ├── lib.rs          # 模块入口，重新导出公共 API
│   ├── types.rs        # 公共类型定义
│   ├── traits.rs       # 公共 Trait 接口（PricingProvider, PriceOracle, ExchangeRateProvider, DepositCalculator）
│   ├── mask.rs         # 数据脱敏函数
│   ├── validation.rs   # 验证函数
│   └── time.rs         # 时间转换工具
└── Cargo.toml
```

## Trait 接口

### PricingProvider

NEX/USD 汇率查询接口。

```rust
pub trait PricingProvider<Balance> {
    /// 获取 NEX/USD 汇率（精度 10^6）
    /// 返回 Some(rate) 表示当前汇率，None 表示价格不可用
    fn get_cos_to_usd_rate() -> Option<Balance>;
    
    /// 上报 P2P 成交到价格聚合（nex-market 内部自行更新 TWAP，此方法可空实现）
    fn report_p2p_trade(
        timestamp: u64,
        price_usdt: u64,
        nex_qty: u128,
    ) -> sp_runtime::DispatchResult;
}
```

**使用者**：
- `pallet-arbitration`：投诉押金 USD 换算
- `pallet-storage-service`：保证金 USD 换算
- `pallet-entity-*`：Entity 定价

**Runtime 实现**：
- `TradingPricingProvider`：优先 1h TWAP → LastTradePrice → initial_price（治理设定）

### PriceOracle

NEX/USDT 链上 TWAP 价格预言机接口。

```rust
pub trait PriceOracle {
    /// 获取指定窗口的 TWAP（精度 10^6 = 1 USDT）
    fn get_twap(window: TwapWindow) -> Option<u64>;
    /// 获取最新成交价
    fn get_last_trade_price() -> Option<u64>;
    /// 价格数据是否过时（超过 max_age_blocks 个区块未更新）
    fn is_price_stale(max_age_blocks: u32) -> bool;
    /// 获取累计交易数（用于判断数据可信度）
    fn get_trade_count() -> u64;
}
```

**TWAP 窗口**：
- `TwapWindow::OneHour` — ~10min 实际窗口
- `TwapWindow::OneDay` — ~1-2h 实际窗口
- `TwapWindow::OneWeek` — ~24-48h 实际窗口（最抗操纵）

**Runtime 实现**：
- `pallet_nex_market::Pallet<Runtime>` 直接实现此 trait

### ExchangeRateProvider（v0.7.0）

统一兑换比率接口 — 聚合 TWAP + 陈旧检测 + 置信度评估。

```rust
pub trait ExchangeRateProvider {
    /// 获取 NEX/USDT 兑换比率（精度 10^6）
    fn get_nex_usdt_rate() -> Option<u64>;
    /// 价格置信度 (0-100)
    fn price_confidence() -> u8;
    /// 价格是否可信赖（置信度 >= 30）
    fn is_rate_reliable() -> bool;
}
```

**置信度等级**：

| 区间 | 含义 | 数据来源 |
|------|------|----------|
| 90-100 | 高可信 | TWAP 可用 + 高交易量（≥100笔） |
| 60-89 | 中可信 | TWAP 或 LastTradePrice 可用 |
| 30-59 | 低可信 | 仅 initial_price（冷启动期） |
| 0-29 | 不可信 | 价格过时或不可用 |

**Runtime 实现**：
- `NexExchangeRateProvider`：组合 `TradingPricingProvider` + `PriceOracle` 的置信度评估

### DepositCalculator

统一保证金计算接口。

```rust
pub trait DepositCalculator<Balance> {
    /// 根据 USD 金额和汇率计算保证金（NEX）
    /// 如汇率不可用则返回 fallback 金额
    fn calculate_deposit(usd_amount: u64, fallback: Balance) -> Balance;
}
```

**使用者**：
- `pallet-storage-service`：存储操作员保证金计算

**默认实现**：
- `DepositCalculatorImpl<P, Balance>`：基于 `PricingProvider<Balance>` 自动换算

## 类型定义

### TronAddress

TRON 地址类型，固定 34 字节。

```rust
pub type TronAddress = BoundedVec<u8, ConstU32<34>>;
```

### MomentOf

时间戳类型，Unix 秒。

```rust
pub type MomentOf = u64;
```

### Cid

IPFS CID 类型，最大 64 字节。

```rust
pub type Cid = BoundedVec<u8, ConstU32<64>>;
```

### TxHash

交易哈希类型，最大 128 字节。

```rust
pub type TxHash = BoundedVec<u8, ConstU32<128>>;
```

## 工具函数

### 数据脱敏函数

#### mask_name

姓名脱敏函数。

```rust
pub fn mask_name(full_name: &str) -> Vec<u8>
```

**脱敏规则**：
| 字符数 | 规则 | 示例 |
|--------|------|------|
| 0 | 返回空 | `""` → `""` |
| 1 | 返回 `×` | `"李"` → `"×"` |
| 2 | 前×，保留后 | `"张三"` → `"×三"` |
| 3 | 前后保留，中间× | `"李四五"` → `"李×五"` |
| 4+ | 前1后1，中间× | `"王二麻子"` → `"王×子"` |

#### mask_id_card

身份证号脱敏函数。

```rust
pub fn mask_id_card(id_card: &str) -> Vec<u8>
```

**脱敏规则**：
| 长度 | 规则 | 示例 |
|------|------|------|
| 18位 | 前4位 + 10个`*` + 后4位 | `"110101199001011234"` → `"1101**********1234"` |
| 15位 | 前4位 + 7个`*` + 后4位 | `"110101900101123"` → `"1101*******1123"` |
| <8位 | 全部用`*`替换 | `"1234567"` → `"*******"` |

#### mask_birthday

生日脱敏函数。

```rust
pub fn mask_birthday(birthday: &str) -> Vec<u8>
```

**脱敏规则**：
| 格式 | 规则 | 示例 |
|------|------|------|
| YYYY-MM-DD | 保留年份，月日用xx替换 | `"1990-01-01"` → `"1990-xx-xx"` |
| <4字符 | 全部替换 | `"123"` → `"****-xx-xx"` |

### 验证函数

#### is_valid_tron_address

验证 TRON 地址格式。

```rust
pub fn is_valid_tron_address(address: &[u8]) -> bool
```

**验证规则**：
- 长度：34 字符
- 开头：`T`
- 编码：Base58（字符集：`1-9, A-H, J-N, P-Z, a-k, m-z`）
- 校验和：Base58Check（SHA256 双哈希后 4 字节校验）

### 时间转换工具

#### 常量

```rust
pub const DEFAULT_BLOCK_TIME_SECS: u64 = 6;  // 默认区块时间（秒）
pub const BLOCKS_PER_MINUTE: u64 = 10;       // 1分钟的区块数
pub const BLOCKS_PER_HOUR: u64 = 600;        // 1小时的区块数
pub const BLOCKS_PER_DAY: u64 = 14400;       // 1天的区块数
```

#### 函数

| 函数 | 说明 |
|------|------|
| `blocks_to_seconds(blocks)` | 区块数 → 秒数 |
| `seconds_to_blocks(seconds)` | 秒数 → 区块数（向上取整） |
| `estimate_timestamp_from_block(target, current, timestamp)` | 预估目标区块的 Unix 时间戳 |
| `estimate_remaining_seconds(target, current)` | 计算剩余秒数 |
| `format_duration(seconds)` | 格式化为可读字符串（如 `1h 30m`） |

## 使用示例

### 在 Pallet 中引用

```toml
# Cargo.toml
[dependencies]
pallet-trading-common = { path = "../common", default-features = false }

[features]
std = [
    "pallet-trading-common/std",
]
```

### 使用 Trait 接口

```rust
use pallet_trading_common::PricingProvider;

#[pallet::config]
pub trait Config: frame_system::Config {
    type Pricing: PricingProvider<BalanceOf<Self>>;
}

impl<T: Config> Pallet<T> {
    fn get_nex_price() -> Option<BalanceOf<T>> {
        T::Pricing::get_cos_to_usd_rate()
    }
}
```

### 使用工具函数

```rust
use pallet_trading_common::{
    mask_name, is_valid_tron_address,
    blocks_to_seconds, estimate_remaining_seconds,
};

let masked = mask_name("张三");  // "×三"
let valid = is_valid_tron_address(b"TYASr5UV6HEcXatwdFQfmLVUqQQQMUxHLS"); // true
let remaining = estimate_remaining_seconds(1100, 1000);  // 600 秒
```

## 版本历史

| 版本 | 日期 | 变更内容 |
|------|------|----------|
| v0.7.0 | 2026-02-26 | 新增 ExchangeRateProvider（带置信度的统一兑换比率接口） |
| v0.6.0 | 2026-02-23 | 新增 PriceOracle（TWAP 预言机接口）；移除 MakerInterface 等废弃 Trait |
| v0.5.0 | 2026-02-08 | 适配 P2P 统一模型：report_swap_order → report_p2p_trade |
| v0.4.0 | 2026-01-18 | 统一公共类型和 Trait 定义 |
| v0.3.0 | 2026-01-18 | 添加时间转换工具函数 |
| v0.2.0 | 2026-01-18 | 添加统一的 MakerCreditInterface trait |
| v0.1.0 | - | 初始版本 |

## 依赖关系

```
pallet-trading-common
├── codec (SCALE 编解码)
├── scale-info (类型元数据)
├── sp-core (Substrate 核心)
├── sp-std (no_std 标准库)
├── sp-runtime (运行时工具)
└── frame-support (FRAME 支持)
```

## 许可证

Unlicense
