# pallet-trading-common

> 交易系统公共工具库 — 为 Nexus 链上交易生态提供统一的 Trait 接口、共享类型、业务函数和验证工具。

## 概述

`pallet-trading-common` 是一个纯 Rust 工具库（**非 FRAME pallet**），不含链上存储，`no_std` 兼容。它作为交易模块组的共享基础层，被多个 pallet 和 runtime 配置引用，确保跨模块的类型一致性和业务规则统一。

### 设计原则

- **单一数据源** — 付款判定、没收梯度、保证金计算等业务规则只在此 crate 定义一次
- **Trait 抽象** — 通过 `PricingProvider` / `PriceOracle` / `ExchangeRateProvider` 解耦价格数据源与消费方
- **零存储** — 纯函数 + Trait 定义，不引入任何链上存储开销
- **防御性编码** — 所有数值函数使用 `saturating_*` / `checked_*` 防溢出，脱敏函数防 UTF-8 panic

### 架构总览

```text
┌─────────────────────────────────────────────────────────────┐
│                    pallet-trading-common                     │
├──────────────┬──────────────┬──────────────┬────────────────┤
│  traits.rs   │  types.rs    │ validation.rs│   time.rs      │
│  4 Trait 接口 │  5 基础类型   │ TRON 地址校验 │ 区块⇆秒数转换   │
│  + 泛型实现   │  3 共享枚举   │ Base58Check  │ 时间戳预估      │
│              │  3 业务函数   │              │ 可读时间格式    │
├──────────────┴──────────────┴──────────────┴────────────────┤
│  mask.rs (数据脱敏)          │  macros.rs (define_balance_of!)│
└──────────────────────────────┴──────────────────────────────┘
```

### 下游消费方

| 消费方 | 使用的 API |
|--------|-----------|
| **pallet-nex-market** | `is_valid_tron_address`, `compute_payment_ratio_bps`, `calculate_payment_verification_result`, `calculate_deposit_forfeit_rate`, `PaymentVerificationResult`; 实现 `PriceOracle` |
| **pallet-dispute-arbitration** | `PricingProvider`（Config 关联类型） |
| **pallet-storage-service** | `DepositCalculator`（Config 关联类型） |
| **runtime/configs** | 实现 `TradingPricingProvider`（`PricingProvider`）、`NexExchangeRateProvider`（`ExchangeRateProvider`）、`EntityPricingProvider`（组合 `PricingProvider` + `PriceOracle`） |

---

## Trait 接口

### 1. PricingProvider\<Balance\>

NEX/USD 汇率查询的底层接口。

```rust
pub trait PricingProvider<Balance> {
    /// NEX/USD 汇率（精度 10^6），None = 不可用
    fn get_nex_to_usd_rate() -> Option<Balance>;

    /// 上报 P2P 成交到价格聚合
    fn report_p2p_trade(timestamp: u64, price_usdt: u64, nex_qty: u128) -> DispatchResult;
}
```

| 方法 | 精度 | 说明 |
|------|------|------|
| `get_nex_to_usd_rate()` | 10^6 | `1_000_000` = 1 NEX = 1 USD |
| `report_p2p_trade()` | timestamp=ms, price=10^6, qty=10^12 | 喂价到 TWAP 聚合器 |

- **向后兼容**: `report_swap_order()` 已 deprecated，自动转发到 `report_p2p_trade()`
- **Runtime 实现**: `TradingPricingProvider` — 优先 1h TWAP → LastTradePrice → initial_price
- **空实现**: `impl for ()` — 返回 `None` / `Ok(())`

### 2. PriceOracle

链上 TWAP 价格预言机接口 — 由 `pallet-nex-market` 实现。

```rust
pub trait PriceOracle {
    fn get_twap(window: TwapWindow) -> Option<u64>;       // TWAP（精度 10^6）
    fn get_last_trade_price() -> Option<u64>;              // 最新成交价
    fn is_price_stale(max_age_blocks: u32) -> bool;        // 价格是否过时
    fn get_trade_count() -> u64;                           // 累计交易笔数
}
```

**TwapWindow 枚举**（SCALE 编码 + TypeInfo + MaxEncodedLen）：

| 窗口 | 抗操纵能力 | 典型用途 |
|------|-----------|---------|
| `OneHour` | 低 | 实时价格参考、保证金计算 |
| `OneDay` | 中 | 过渡期定价 |
| `OneWeek` | 高 | 成熟期基准价 |

### 3. ExchangeRateProvider

高级封装 — 聚合 TWAP + 陈旧检测 + 置信度评估。

```rust
pub trait ExchangeRateProvider {
    fn get_nex_usdt_rate() -> Option<u64>;                 // 汇率（精度 10^6）
    fn price_confidence() -> u8;                           // 置信度 0-100
    fn is_rate_reliable() -> bool {                        // 默认: ≥ 30 即可信
        Self::price_confidence() >= 30
    }
}
```

**置信度评估**：

| 区间 | 含义 | 数据来源 |
|------|------|----------|
| 90-100 | 高可信 | TWAP 可用 + 高交易量（≥100 笔） |
| 60-89 | 中可信 | TWAP 或 LastTradePrice 可用 |
| 30-59 | 低可信 | 仅 initial_price（冷启动期） |
| 0-29 | 不可信 | 价格过时或不可用 |

**Trait 层次关系**：

```text
ExchangeRateProvider    ← 高级聚合，对外统一接口
  ├── PricingProvider   ← 底层汇率 + 喂价
  └── PriceOracle       ← TWAP 窗口 + 陈旧检测
        └── pallet-nex-market  ← 唯一数据源
```

- **Runtime 实现**: `NexExchangeRateProvider`
- **空实现**: `impl for ()` — 返回 `None` / `0` / `false`

### 4. DepositCalculator\<Balance\>

USD→NEX 保证金动态计算。

```rust
pub trait DepositCalculator<Balance> {
    fn calculate_deposit(usd_amount: u64, fallback: Balance) -> Balance;
}
```

**计算公式**：

```
nex_amount = usd_amount × 10^12 / rate

usd_amount : 精度 10^6（5_000_000 = 5 USDT）
rate       : 精度 10^6（来自 PricingProvider）
结果       : 精度 10^12（NEX 标准精度 UNIT）
异常       : 汇率 None 或 0 → 返回 fallback
```

**泛型实现**: `DepositCalculatorImpl<P: PricingProvider<Balance>, Balance>`

```rust
// runtime 中配置
type DepositCalculator = DepositCalculatorImpl<TradingPricingProvider, Balance>;
```

---

## 共享类型

### 基础类型

| 类型 | 底层 | 说明 |
|------|------|------|
| `TronAddress` | `BoundedVec<u8, 34>` | TRON Base58 地址（`T` 开头，34 字符） |
| `TronTxHash` | `BoundedVec<u8, 64>` | TRON 交易哈希（hex 编码） |
| `TxHash` | `BoundedVec<u8, 128>` | 通用链交易哈希 |
| `MomentOf` | `u64` | Unix 秒时间戳 |
| `Cid` | `BoundedVec<u8, 64>` | IPFS 内容标识符 |

### 交易状态枚举

**UsdtTradeStatus** — USDT 交易生命周期：

```text
AwaitingPayment → AwaitingVerification → Completed
                                       → Disputed
                                       → UnderpaidPending → Completed / Refunded
                → Cancelled
                → Refunded
```

**BuyerDepositStatus** — 买家保证金生命周期（默认 `None`）：

```text
None → Locked → Released           (正常完成)
              → Forfeited          (超时/违约)
              → PartiallyForfeited (少付)
```

### 付款判定

**PaymentVerificationResult** — 多档判定（basis points 精度）：

| 变体 | 条件 (bps) | 含义 |
|------|-----------|------|
| `Overpaid` | ratio ≥ 10050 | 多付 ≥ 100.5% |
| `Exact` | 9950 ≤ ratio < 10050 | 精确 99.5%~100.5% |
| `Underpaid` | 5000 ≤ ratio < 9950 | 少付 50%~99.5% |
| `SeverelyUnderpaid` | ratio < 5000 | 严重少付 < 50% |
| `Invalid` | expected=0 或 actual=0 | 无效输入 |

### 业务函数

```rust
/// 多档判定入口（expected=0 或 actual=0 → Invalid）
pub fn calculate_payment_verification_result(expected: u64, actual: u64) -> PaymentVerificationResult;

/// 付款比例（bps, 10000=100%）— u32 防超付截断，u128 中间计算防溢出
pub fn compute_payment_ratio_bps(expected: u64, actual: u64) -> u32;

/// 保证金梯度没收比例
pub fn calculate_deposit_forfeit_rate(payment_ratio: u32) -> u16;
```

**没收梯度**：

| 付款比例 | 没收比例 | 说明 |
|---------|---------|------|
| ≥ 99.5% | 0% | 视为足额 |
| 95%~99.5% | 20% | 轻微少付 |
| 80%~95% | 50% | 明显少付 |
| < 80% | 100% | 严重少付，全额没收 |

---

## 工具模块

### TRON 地址校验 (validation.rs)

```rust
pub fn is_valid_tron_address(address: &[u8]) -> bool
```

完整 Base58Check 六步校验：

1. 长度 == 34 字节
2. 首字节 == `T`
3. 全部字符 ∈ Base58 字符集
4. Base58 解码 → 25 字节
5. 首字节 == `0x41`（TRON 主网）
6. `SHA256(SHA256(payload[0..21]))[0..4] == checksum[21..25]`

内置零依赖 `base58_decode()` 实现。

### 数据脱敏 (mask.rs)

三个函数均对**非 ASCII 输入安全**（返回全掩码，不会 panic）：

| 函数 | 输入 → 输出示例 |
|------|---------------|
| `mask_name("张三")` | `"×三"` |
| `mask_name("李四五")` | `"李×五"` |
| `mask_id_card("110101199001011234")` | `"1101**********1234"` |
| `mask_birthday("1990-01-01")` | `"1990-xx-xx"` |

### 时间转换 (time.rs)

**常量**（基于 6 秒出块时间）：

| 常量 | 值 |
|------|-----|
| `DEFAULT_BLOCK_TIME_SECS` | 6 |
| `BLOCKS_PER_MINUTE` | 10 |
| `BLOCKS_PER_HOUR` | 600 |
| `BLOCKS_PER_DAY` | 14400 |

**函数**：

| 函数 | 说明 | 示例 |
|------|------|------|
| `blocks_to_seconds(blocks)` | 区块 → 秒（`saturating_mul`） | `100 → 600` |
| `seconds_to_blocks(secs)` | 秒 → 区块（向上取整） | `7 → 2` |
| `estimate_timestamp_from_block(target, current, now_ts)` | 预估目标区块时间戳 | 支持过去/未来 |
| `estimate_remaining_seconds(target, current)` | 剩余秒数（已过期 → 0） | `(1100, 1000) → 600` |
| `format_duration(secs)` | 人类可读格式 | `5400 → "1h 30m"` |

### 公共宏 (macros.rs)

```rust
pallet_trading_common::define_balance_of!();
// 展开为: type BalanceOf<T> = <<T as Config>::Currency as Currency<...>>::Balance;
```

---

## 快速接入

### Cargo.toml

```toml
[dependencies]
pallet-trading-common = { path = "../common", default-features = false }

[features]
std = ["pallet-trading-common/std"]
```

### Pallet Config

```rust
use pallet_trading_common::{PricingProvider, DepositCalculator};

#[pallet::config]
pub trait Config: frame_system::Config {
    type Pricing: PricingProvider<BalanceOf<Self>>;
    type DepositCalc: DepositCalculator<BalanceOf<Self>>;
}
```

### 业务函数调用

```rust
use pallet_trading_common::{
    calculate_payment_verification_result, compute_payment_ratio_bps,
    calculate_deposit_forfeit_rate, PaymentVerificationResult,
};

let result = calculate_payment_verification_result(10_000_000, 9_500_000); // Underpaid
let ratio  = compute_payment_ratio_bps(10_000_000, 9_500_000);            // 9500
let forfeit = calculate_deposit_forfeit_rate(ratio);                       // 2000 (20%)
```

### 工具函数调用

```rust
use pallet_trading_common::{
    is_valid_tron_address, mask_name,
    blocks_to_seconds, format_duration, BLOCKS_PER_HOUR,
};

assert!(is_valid_tron_address(b"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"));
assert_eq!(mask_name("张三"), "×三".as_bytes());
assert_eq!(blocks_to_seconds(BLOCKS_PER_HOUR), 3600);
assert_eq!(format_duration(5400), b"1h 30m");
```

---

## 完整导出清单

| 类别 | 项目 |
|------|------|
| **Trait** | `PricingProvider`, `PriceOracle`, `TwapWindow`, `ExchangeRateProvider`, `DepositCalculator`, `DepositCalculatorImpl` |
| **类型** | `TronAddress`, `TronTxHash`, `TxHash`, `MomentOf`, `Cid`, `UsdtTradeStatus`, `BuyerDepositStatus`, `PaymentVerificationResult` |
| **业务函数** | `calculate_payment_verification_result`, `compute_payment_ratio_bps`, `calculate_deposit_forfeit_rate` |
| **工具函数** | `mask_name`, `mask_id_card`, `mask_birthday`, `is_valid_tron_address`, `blocks_to_seconds`, `seconds_to_blocks`, `estimate_timestamp_from_block`, `estimate_remaining_seconds`, `format_duration` |
| **常量** | `DEFAULT_BLOCK_TIME_SECS`, `BLOCKS_PER_MINUTE`, `BLOCKS_PER_HOUR`, `BLOCKS_PER_DAY` |
| **宏** | `define_balance_of!` |

---

## 测试

```bash
cargo test -p pallet-trading-common    # 54 个单元测试
```

| 模块 | 测试数 | 覆盖要点 |
|------|--------|---------|
| types.rs | 20 | 付款比例计算（正常/超付/u64::MAX 防溢出）、多档判定边界、无效输入、梯度没收四档 |
| traits.rs | 11 | DepositCalculator（汇率正常/None/零/u64::MAX 极值/零金额）、PriceOracle 空实现、ExchangeRateProvider 置信度阈值 |
| time.rs | 10 | 双向转换、时间戳预估、剩余秒数、格式化、常量、u64::MAX 饱和保护 |
| validation.rs | 6 | 地址格式校验、Base58Check 校验和、解码、空地址/全T/非 ASCII 边界 |
| mask.rs | 7 | 脱敏规则（姓名/身份证/生日）、非 ASCII + emoji panic 防护 |

---

## 依赖

| crate | 用途 |
|-------|------|
| `codec` (SCALE) | Encode / Decode / DecodeWithMemTracking |
| `scale-info` | TypeInfo 元数据 |
| `sp-core` | `hashing::sha2_256`（Base58Check） |
| `sp-std` | `no_std` 兼容 |
| `sp-runtime` | `DispatchResult`, `AtLeast32BitUnsigned`, `RuntimeDebug` |
| `frame-support` | `BoundedVec`, `ConstU32`, `MaxEncodedLen`, `Currency` |

---

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.8.1 | 2026-03-05 | R2 审计: 过时文档修正, cos→nex 变量名, mask_birthday 冗余清理, 校验和切片比较, +10 边界测试 (54 total) |
| v0.8.0 | 2026-03-04 | R1 审计: get_cos_to_usd_rate→get_nex_to_usd_rate, mask panic 防护, TwapWindow SCALE derives, +4 回归测试 (44 total) |
| v0.7.0 | 2026-02-26 | ExchangeRateProvider（置信度接口） |
| v0.6.0 | 2026-02-23 | PriceOracle + TwapWindow; 共享枚举 + 多档判定 + 梯度没收函数 |
| v0.5.0 | 2026-02-08 | report_swap_order→report_p2p_trade; define_balance_of! 宏; M7 精度修复 |
| v0.4.0 | 2026-01-18 | 统一公共类型和 Trait |
| v0.3.0 | 2026-01-18 | 时间转换工具 |
| v0.2.0 | 2026-01-18 | MakerCreditInterface（后废弃） |
| v0.1.0 | — | 初始版本 |

---

**License**: Unlicense
