# pallet-trading-common

交易系统公共工具库 — 共享 Trait 接口、类型定义、业务函数和验证工具。

## 概述

`pallet-trading-common` 是纯 Rust crate（非 FRAME pallet，`no_std` 兼容），无链上存储，被 Trading 模块组及多个外部 pallet 共享引用。

### 模块结构

```text
src/
├── lib.rs          # 入口，重新导出所有公共 API
├── traits.rs       # 共享 Trait（PricingProvider, PriceOracle, ExchangeRateProvider, DepositCalculator）
├── types.rs        # 共享类型 + 业务函数（多档判定、没收梯度、付款比例计算）
├── mask.rs         # 数据脱敏（姓名、身份证、生日）
├── validation.rs   # TRON 地址 Base58Check 校验
├── time.rs         # 区块数 ↔ 秒数转换
└── macros.rs       # 公共宏（define_balance_of!）
```

---

## Trait 接口（traits.rs）

### PricingProvider\<Balance\>

NEX/USD 底层汇率查询 — 被多个外部 pallet 引用。

```rust
pub trait PricingProvider<Balance> {
    fn get_cos_to_usd_rate() -> Option<Balance>;
    fn report_p2p_trade(timestamp: u64, price_usdt: u64, nex_qty: u128) -> DispatchResult;
}
```

| 方法 | 说明 |
|------|------|
| `get_cos_to_usd_rate()` | 获取 NEX/USD 汇率（精度 10^6，如 `1_000_000` = 1 NEX = 1 USD），`None` 表示不可用 |
| `report_p2p_trade()` | 上报 P2P 成交（timestamp=Unix ms, price_usdt=精度 10^6, nex_qty=精度 10^12） |

**向后兼容**：`report_swap_order()`（已 deprecated）转发到 `report_p2p_trade()`。

**Runtime 实现**：`TradingPricingProvider` — 优先 1h TWAP → LastTradePrice → initial_price

**消费方**：

| 模块 | 用途 |
|------|------|
| pallet-arbitration | 投诉押金 USD 换算 |
| pallet-storage-service | 运营者保证金 USD 换算 |
| pallet-entity-registry | 开店初始资金 |
| pallet-entity-market | 商品押金 |
| pallet-entity-product | 服务押金 |

**空实现**：`impl PricingProvider<Balance> for ()` — 返回 `None` / `Ok(())`，用于测试和 Mock。

---

### PriceOracle

NEX/USDT 链上 TWAP 价格预言机 — 由 `pallet-nex-market` 直接实现。

```rust
pub trait PriceOracle {
    fn get_twap(window: TwapWindow) -> Option<u64>;
    fn get_last_trade_price() -> Option<u64>;
    fn is_price_stale(max_age_blocks: u32) -> bool;
    fn get_trade_count() -> u64;
}
```

| 方法 | 返回 | 说明 |
|------|------|------|
| `get_twap(window)` | `Option<u64>` | 指定窗口的 TWAP（精度 10^6） |
| `get_last_trade_price()` | `Option<u64>` | 最新成交价 |
| `is_price_stale(max_age)` | `bool` | 超过 `max_age` 区块未更新则为 true |
| `get_trade_count()` | `u64` | 累计交易数（判断数据可信度） |

**TwapWindow 枚举**：

| 窗口 | 实际精度 | 抗操纵能力 | 用途 |
|------|---------|-----------|------|
| `OneHour` | ~10min | 低 | 实时价格参考 |
| `OneDay` | ~1-2h | 中 | 过渡期定价 |
| `OneWeek` | ~24-48h | 高 | 成熟期基准价 |

**空实现**：返回 `None` / `true` / `0`。

---

### ExchangeRateProvider

统一兑换比率接口 — 聚合 TWAP + 陈旧检测 + 置信度评估的高级封装。

```rust
pub trait ExchangeRateProvider {
    fn get_nex_usdt_rate() -> Option<u64>;
    fn price_confidence() -> u8;
    fn is_rate_reliable() -> bool { Self::price_confidence() >= 30 }
}
```

| 方法 | 返回 | 说明 |
|------|------|------|
| `get_nex_usdt_rate()` | `Option<u64>` | NEX/USDT 汇率（精度 10^6），内部优先级：1h TWAP → LastTrade → initial_price |
| `price_confidence()` | `u8` | 价格置信度 (0-100) |
| `is_rate_reliable()` | `bool` | 置信度 ≥ 30 即可信（默认实现，可覆盖） |

**置信度等级**：

| 区间 | 含义 | 数据来源 |
|------|------|----------|
| 90-100 | 高可信 | TWAP 可用 + 高交易量（≥100笔） |
| 60-89 | 中可信 | TWAP 或 LastTradePrice 可用 |
| 30-59 | 低可信 | 仅 initial_price（冷启动期） |
| 0-29 | 不可信 | 价格过时或不可用 |

**与其他 Trait 的关系**：

```text
ExchangeRateProvider (高级封装，带置信度)
  └── 内部组合 PricingProvider + PriceOracle
        └── 数据源 → pallet-nex-market
```

**Runtime 实现**：`NexExchangeRateProvider`

**空实现**：返回 `None` / `0` / `false`。

---

### DepositCalculator\<Balance\>

统一保证金计算接口 — 基于 USD 金额和实时汇率动态计算 NEX 保证金。

```rust
pub trait DepositCalculator<Balance> {
    fn calculate_deposit(usd_amount: u64, fallback: Balance) -> Balance;
}
```

**计算公式**：

```text
nex_amount = usd_amount × 10^12 / rate

其中：
  usd_amount  精度 10^6（如 5_000_000 = 5 USDT）
  rate        精度 10^6（NEX/USD 汇率）
  结果        精度 10^12（NEX 标准精度）

汇率不可用或为 0 时 → 返回 fallback
```

**泛型实现**：`DepositCalculatorImpl<P, Balance>`

```rust
type DepositCalc = DepositCalculatorImpl<TradingPricingProvider, Balance>;
let deposit = DepositCalc::calculate_deposit(5_000_000, fallback); // 5 USDT 等值
```

要求 `P: PricingProvider<Balance>`, `Balance: AtLeast32BitUnsigned + Copy + TryFrom<u128> + Into<u128>`。

**空实现**：`impl DepositCalculator<Balance> for ()` — 始终返回 fallback。

---

## 共享类型（types.rs）

### 基础类型

| 类型 | 定义 | 说明 |
|------|------|------|
| `TronAddress` | `BoundedVec<u8, 34>` | TRON Base58 地址（以 `T` 开头） |
| `TronTxHash` | `BoundedVec<u8, 64>` | TRON 交易哈希（64 字节 hex） |
| `TxHash` | `BoundedVec<u8, 128>` | 通用交易哈希 |
| `MomentOf` | `u64` | Unix 秒时间戳 |
| `Cid` | `BoundedVec<u8, 64>` | IPFS 内容标识符 |

### 共享枚举

#### UsdtTradeStatus

USDT 交易状态（entity-market / nex-market 共享），SCALE 编码兼容。

| 变体 | 说明 |
|------|------|
| `AwaitingPayment` | 等待买家支付 USDT |
| `AwaitingVerification` | 等待 OCW 验证 |
| `Completed` | 已完成 |
| `Disputed` | 争议中 |
| `Cancelled` | 已取消 |
| `Refunded` | 已退款（超时） |
| `UnderpaidPending` | 少付等待补付（窗口内） |

#### BuyerDepositStatus

买家保证金状态（entity-market / nex-market 共享），默认值 `None`。

| 变体 | 说明 |
|------|------|
| `None` | 无保证金（默认） |
| `Locked` | 已锁定 |
| `Released` | 已退还（交易完成） |
| `Forfeited` | 已没收（超时/违约） |
| `PartiallyForfeited` | 部分没收（少付场景） |

#### PaymentVerificationResult

付款金额多档判定结果（entity-market / nex-market 共享）。

| 变体 | 条件 (bps) | 说明 |
|------|-----------|------|
| `Overpaid` | ratio ≥ 10050 | 多付（≥100.5%） |
| `Exact` | 9950 ≤ ratio < 10050 | 精确（99.5%~100.5%） |
| `Underpaid` | 5000 ≤ ratio < 9950 | 少付（50%~99.5%） |
| `SeverelyUnderpaid` | ratio < 5000 | 严重少付（<50%） |
| `Invalid` | expected=0 或 actual=0 | 无效 |

### 业务函数

#### calculate_payment_verification_result

```rust
pub fn calculate_payment_verification_result(
    expected_amount: u64,
    actual_amount: u64,
) -> PaymentVerificationResult
```

多档判定入口。`expected=0` 或 `actual=0` 均返回 `Invalid`（M2 修复）。

#### compute_payment_ratio_bps

```rust
pub fn compute_payment_ratio_bps(expected_amount: u64, actual_amount: u64) -> u32
```

计算付款比例（basis points，10000 = 100%）。返回 **u32** 防止超付 >6.55 倍时 u16 截断导致误判（H1 修复）。

```text
ratio = actual × 10000 / expected    （u128 中间计算，min(u32::MAX)）
expected=0 → 返回 0
```

> 下游 pallet 应统一使用此函数，禁止自行 `as u16`。

#### calculate_deposit_forfeit_rate

```rust
pub fn calculate_deposit_forfeit_rate(payment_ratio: u32) -> u16
```

保证金梯度没收比例（entity-market / nex-market 共享）。

| 付款比例 (bps) | 没收比例 (bps) |
|---------------|---------------|
| ≥ 9950 (99.5%) | 0 (0%) |
| 9500~9949 (95%~99.5%) | 2000 (20%) |
| 8000~9499 (80%~95%) | 5000 (50%) |
| < 8000 (<80%) | 10000 (100%) |

---

## 数据脱敏（mask.rs）

### mask_name

```rust
pub fn mask_name(full_name: &str) -> Vec<u8>
```

| 字符数 | 规则 | 示例 |
|--------|------|------|
| 0 | 返回空 | `""` → `""` |
| 1 | 替换为 `×` | `"李"` → `"×"` |
| 2 | 前 `×` + 保留末字 | `"张三"` → `"×三"` |
| 3 | 保留首末 + 中间 `×` | `"李四五"` → `"李×五"` |
| 4+ | 保留首末 + 中间 `×` | `"王二麻子"` → `"王×子"` |

### mask_id_card

```rust
pub fn mask_id_card(id_card: &str) -> Vec<u8>
```

| 长度 | 规则 | 示例 |
|------|------|------|
| 18 位 | 前 4 + 10×`*` + 后 4 | `"110101199001011234"` → `"1101**********1234"` |
| 15 位 | 前 4 + 7×`*` + 后 4 | `"110101900101123"` → `"1101*******1123"` |
| < 8 位 | 全部 `*` 替换 | `"1234567"` → `"*******"` |

### mask_birthday

```rust
pub fn mask_birthday(birthday: &str) -> Vec<u8>
```

| 格式 | 规则 | 示例 |
|------|------|------|
| YYYY-MM-DD | 保留年份 + `xx-xx` | `"1990-01-01"` → `"1990-xx-xx"` |
| < 4 字符 | 全部替换 | `"123"` → `"****-xx-xx"` |

---

## TRON 地址校验（validation.rs）

### is_valid_tron_address

```rust
pub fn is_valid_tron_address(address: &[u8]) -> bool
```

完整 Base58Check 校验链：

```text
1. 长度 == 34 字节
2. 首字节 == 'T'
3. 所有字节 ∈ Base58 字符集（1-9, A-H, J-N, P-Z, a-k, m-z）
4. Base58 解码 → 25 字节
5. 首字节 == 0x41（TRON 主网前缀）
6. SHA256(SHA256(payload[0..21]))[0..4] == checksum[21..25]
```

内部 `base58_decode()` 为无外部依赖实现，支持前导 `1` → `0x00` 映射。

---

## 时间转换工具（time.rs）

### 常量

| 常量 | 值 | 说明 |
|------|------|------|
| `DEFAULT_BLOCK_TIME_SECS` | 6 | 默认区块时间（秒） |
| `BLOCKS_PER_MINUTE` | 10 | 1 分钟的区块数 |
| `BLOCKS_PER_HOUR` | 600 | 1 小时的区块数 |
| `BLOCKS_PER_DAY` | 14400 | 1 天的区块数 |

### 函数

| 函数 | 签名 | 说明 |
|------|------|------|
| `blocks_to_seconds` | `(u64) → u64` | 区块数 → 秒数（`blocks × 6`） |
| `seconds_to_blocks` | `(u64) → u64` | 秒数 → 区块数（向上取整） |
| `estimate_timestamp_from_block` | `(target, current, timestamp) → u64` | 预估目标区块的 Unix 时间戳（支持过去/未来） |
| `estimate_remaining_seconds` | `(target, current) → u64` | 计算剩余秒数（已过期返回 0） |
| `format_duration` | `(u64) → Vec<u8>` | 格式化为可读字符串 |

**format_duration 输出示例**：

| 输入 (秒) | 输出 |
|-----------|------|
| 0~59 | `< 1m` |
| 300 | `5m` |
| 3600 | `1h` |
| 5400 | `1h 30m` |
| 86400 | `1d` |
| 90000 | `1d 1h` |

---

## 公共宏（macros.rs）

### define_balance_of!

在 pallet 中快速定义 `BalanceOf<T>` 类型别名：

```rust
pallet_trading_common::define_balance_of!();
// 展开为：
// pub type BalanceOf<T> = <<T as Config>::Currency as Currency<...>>::Balance;
```

---

## 使用示例

### 引入依赖

```toml
[dependencies]
pallet-trading-common = { path = "../common", default-features = false }

[features]
std = ["pallet-trading-common/std"]
```

### 在 pallet Config 中使用 Trait

```rust
use pallet_trading_common::{PricingProvider, DepositCalculator};

#[pallet::config]
pub trait Config: frame_system::Config {
    type Pricing: PricingProvider<BalanceOf<Self>>;
    type DepositCalc: DepositCalculator<BalanceOf<Self>>;
}
```

### 使用业务函数

```rust
use pallet_trading_common::{
    calculate_payment_verification_result,
    compute_payment_ratio_bps,
    calculate_deposit_forfeit_rate,
    PaymentVerificationResult,
};

let result = calculate_payment_verification_result(10_000_000, 9_500_000);
// → Underpaid

let ratio = compute_payment_ratio_bps(10_000_000, 9_500_000);
// → 9500 (95%)

let forfeit = calculate_deposit_forfeit_rate(ratio);
// → 2000 (20%)
```

### 使用工具函数

```rust
use pallet_trading_common::{
    is_valid_tron_address, mask_name,
    blocks_to_seconds, estimate_remaining_seconds, format_duration,
};

let valid = is_valid_tron_address(b"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"); // true
let masked = mask_name("张三");          // "×三"
let secs = blocks_to_seconds(100);       // 600
let remaining = estimate_remaining_seconds(1100, 1000); // 600
let display = format_duration(5400);     // "1h 30m"
```

---

## 完整导出清单

### 类型

`TronAddress`, `TronTxHash`, `MomentOf`, `Cid`, `TxHash`, `UsdtTradeStatus`, `BuyerDepositStatus`, `PaymentVerificationResult`

### Trait

`PricingProvider`, `DepositCalculator`, `DepositCalculatorImpl`, `PriceOracle`, `TwapWindow`, `ExchangeRateProvider`

### 业务函数

`calculate_payment_verification_result`, `compute_payment_ratio_bps`, `calculate_deposit_forfeit_rate`

### 工具函数

`mask_name`, `mask_id_card`, `mask_birthday`, `is_valid_tron_address`, `blocks_to_seconds`, `seconds_to_blocks`, `estimate_timestamp_from_block`, `estimate_remaining_seconds`, `format_duration`

### 常量

`DEFAULT_BLOCK_TIME_SECS`

### 宏

`define_balance_of!`

---

## 测试

```bash
cargo test -p pallet-trading-common    # 40 个单元测试
```

覆盖范围：

| 模块 | 测试数 | 覆盖内容 |
|------|--------|---------|
| types.rs | 20 | compute_payment_ratio_bps（正常/7x 超付/100x/u64::MAX 防溢出）、多档判定边界（Overpaid/Exact/Underpaid/SeverelyUnderpaid）、expected=0/actual=0/both=0 边界、梯度没收（四档 + u32 大值） |
| traits.rs | 8 | DepositCalculator（正常汇率/无汇率/零汇率/空实现/多金额）、PricingProvider 空实现、ExchangeRateProvider 空实现 + 置信度阈值 |
| time.rs | 6 | blocks↔seconds 双向转换、estimate_timestamp 过去/未来/当前、remaining 过期处理、format_duration 全格式、常量 |
| validation.rs | 3 | TRON 地址格式（长度/首字/Base58 字符集）、Base58Check 校验和（篡改检测）、解码长度和前缀 |
| mask.rs | 3 | 姓名脱敏（0-4+ 字符）、身份证脱敏（18/15/<8 位）、生日脱敏 |

---

## 依赖

| crate | 用途 |
|-------|------|
| `codec` (SCALE) | 枚举/结构体编解码（Encode, Decode, DecodeWithMemTracking） |
| `scale-info` | TypeInfo 类型元数据（Runtime 元数据生成） |
| `sp-core` | `hashing::sha2_256`（Base58Check 校验和） |
| `sp-std` | `no_std` 兼容 Vec/预导入 |
| `sp-runtime` | `DispatchResult`、`traits::AtLeast32BitUnsigned`、`RuntimeDebug` |
| `frame-support` | `BoundedVec`、`ConstU32`、`MaxEncodedLen`、`Currency` trait |

---

## 版本历史

| 版本 | 日期 | 变更内容 |
|------|------|----------|
| v0.7.0 | 2026-02-26 | 新增 ExchangeRateProvider（带置信度的统一兑换比率接口） |
| v0.6.0 | 2026-02-23 | 新增 PriceOracle + TwapWindow；新增 UsdtTradeStatus/BuyerDepositStatus/PaymentVerificationResult 共享枚举；新增多档判定 + 比例计算 + 梯度没收共享函数；移除废弃 Trait |
| v0.5.0 | 2026-02-08 | report_swap_order → report_p2p_trade；新增 define_balance_of! 宏；DepositCalculatorImpl M7 精度修复（10^12） |
| v0.4.0 | 2026-01-18 | 统一公共类型和 Trait 定义 |
| v0.3.0 | 2026-01-18 | 新增时间转换工具（blocks_to_seconds 等） |
| v0.2.0 | 2026-01-18 | 新增 MakerCreditInterface trait（后废弃） |
| v0.1.0 | — | 初始版本，从 OTC/Swap/Maker 提取共享类型 |

---

**License**: Unlicense
