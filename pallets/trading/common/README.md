# Trading Common（交易公共工具库）

## 概述

`pallet-trading-common` 是交易系统的公共工具库，提供共享类型定义、数据脱敏函数、验证工具和 Trait 接口。

### 特点

- ✅ **纯 Rust crate**：无链上存储，仅提供工具函数和类型定义
- ✅ **跨模块共享**：可被 P2P、Maker、Credit、Pricing 等多个 pallet 引用
- ✅ **no_std 兼容**：支持 WebAssembly 运行时环境

## 模块结构

```
pallets/trading/common/
├── src/
│   ├── lib.rs          # 模块入口，重新导出公共 API
│   ├── types.rs        # 公共类型定义
│   ├── traits.rs       # 公共 Trait 接口
│   ├── mask.rs         # 数据脱敏函数
│   ├── validation.rs   # 验证函数
│   └── time.rs         # 时间转换工具
└── Cargo.toml
```

## 类型定义

### TronAddress

TRON 地址类型，固定 34 字节。

```rust
pub type TronAddress = BoundedVec<u8, ConstU32<34>>;
```

**说明**：
- TRC20 地址以 `T` 开头，长度固定为 34 字符
- 用于 P2P Buy 订单收款地址和 Sell 兑换地址

**使用者**：
- `pallet-trading-p2p`：做市商收款地址 / 用户 USDT 接收地址
- `pallet-trading-maker`：做市商注册地址

### MomentOf

时间戳类型，Unix 秒。

```rust
pub type MomentOf = u64;
```

**说明**：
- 用于 P2P 订单的时间字段
- 精度为秒（非毫秒）

### Cid

IPFS CID 类型，最大 64 字节。

```rust
pub type Cid = BoundedVec<u8, ConstU32<64>>;
```

**说明**：
- 用于存储 IPFS 内容标识符
- 如做市商的公开/私密资料

### TxHash

交易哈希类型，最大 128 字节。

```rust
pub type TxHash = BoundedVec<u8, ConstU32<128>>;
```

**说明**：
- 用于存储 TRON TRC20 交易哈希
- P2P Sell 侧使用

### MakerApplicationInfo

做市商申请信息（简化版，用于跨模块传递）。

```rust
pub struct MakerApplicationInfo<AccountId, Balance> {
    pub account: AccountId,        // 做市商账户
    pub tron_address: TronAddress, // TRON 收款地址
    pub is_active: bool,           // 是否激活
}
```

## Trait 接口

### PricingProvider

定价服务接口，提供 NEX/USD 实时汇率查询功能。

```rust
pub trait PricingProvider<Balance> {
    /// 获取 NEX/USD 汇率（精度 10^6）
    /// 返回 Some(rate) 表示当前汇率，None 表示价格不可用
    fn get_cos_to_usd_rate() -> Option<Balance>;
    
    /// 上报 P2P 成交到价格聚合
    fn report_p2p_trade(
        timestamp: u64,    // 交易时间戳（Unix 毫秒）
        price_usdt: u64,   // USDT 单价（精度 10^6）
        cos_qty: u128,    // NEX 数量（精度 10^12）
    ) -> DispatchResult;
}
```

**使用者**：
- `pallet-trading-p2p`：计算 Buy/Sell 订单金额
- `pallet-trading-maker`：计算押金价值

**实现者**：
- `pallet-trading-pricing`：提供聚合价格

### MakerInterface

做市商接口，提供做市商信息查询功能。

```rust
pub trait MakerInterface<AccountId, Balance> {
    /// 查询做市商申请信息
    fn get_maker_application(maker_id: u64) -> Option<MakerApplicationInfo<AccountId, Balance>>;
    
    /// 检查做市商是否激活
    fn is_maker_active(maker_id: u64) -> bool;
    
    /// 获取做市商 ID（通过账户）
    fn get_maker_id(who: &AccountId) -> Option<u64>;
    
    /// 获取做市商押金的 USD 价值（精度 10^6）
    fn get_deposit_usd_value(maker_id: u64) -> Result<u64, DispatchError>;
}
```

**使用者**：
- `pallet-trading-p2p`：验证做市商和获取收款地址

**实现者**：
- `pallet-trading-maker`：提供做市商管理

### MakerCreditInterface

做市商信用接口，提供信用分管理功能。

```rust
pub trait MakerCreditInterface {
    /// 记录做市商订单完成（提升信用分）
    fn record_maker_order_completed(
        maker_id: u64,
        order_id: u64,
        response_time_seconds: u32,
    ) -> DispatchResult;
    
    /// 记录做市商订单超时（降低信用分）
    fn record_maker_order_timeout(
        maker_id: u64,
        order_id: u64,
    ) -> DispatchResult;
    
    /// 记录做市商争议结果
    fn record_maker_dispute_result(
        maker_id: u64,
        order_id: u64,
        maker_win: bool,  // true = 做市商胜诉
    ) -> DispatchResult;
}
```

**使用者**：
- `pallet-trading-p2p`：Buy/Sell 订单完成/超时/争议时调用

**实现者**：
- `pallet-trading-credit`：提供信用分管理

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
| 15位 | 前4位 + 7个`*` + 后4位 | `"110101900101123"` → `"1101*******0123"` |
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

**示例**：
```rust
// 有效地址
is_valid_tron_address(b"TYASr5UV6HEcXatwdFQfmLVUqQQQMUxHLS"); // true

// 无效地址
is_valid_tron_address(b"AYASr5UV6HEcXatwdFQfmLVUqQQQMUxHLS"); // false - 不是T开头
is_valid_tron_address(b"TYASr5UV6HEcXatwdFQfmLVUqQQQMUxHL0"); // false - 包含0（非Base58）
```

### 时间转换工具

#### 常量

```rust
pub const DEFAULT_BLOCK_TIME_SECS: u64 = 6;  // 默认区块时间（秒）
pub const BLOCKS_PER_MINUTE: u64 = 10;       // 1分钟的区块数
pub const BLOCKS_PER_HOUR: u64 = 600;        // 1小时的区块数
pub const BLOCKS_PER_DAY: u64 = 14400;       // 1天的区块数
```

#### blocks_to_seconds

区块数转换为秒数。

```rust
pub fn blocks_to_seconds(blocks: u64) -> u64
```

**示例**：
```rust
blocks_to_seconds(100);  // 600 秒 = 10 分钟
blocks_to_seconds(600);  // 3600 秒 = 1 小时
```

#### seconds_to_blocks

秒数转换为区块数（向上取整）。

```rust
pub fn seconds_to_blocks(seconds: u64) -> u64
```

**示例**：
```rust
seconds_to_blocks(60);    // 10 块
seconds_to_blocks(3600);  // 600 块 = 1 小时
```

#### estimate_timestamp_from_block

根据区块号预估 Unix 时间戳。

```rust
pub fn estimate_timestamp_from_block(
    target_block: u64,
    current_block: u64,
    current_timestamp: u64,
) -> u64
```

**示例**：
```rust
let future_ts = estimate_timestamp_from_block(
    12345,      // 目标区块
    12000,      // 当前区块
    1705500000, // 当前时间戳
);
// 返回: 1705502070 (当前时间 + 345块 × 6秒)
```

#### estimate_remaining_seconds

计算剩余秒数。

```rust
pub fn estimate_remaining_seconds(target_block: u64, current_block: u64) -> u64
```

**示例**：
```rust
estimate_remaining_seconds(1100, 1000);  // 600 秒
estimate_remaining_seconds(900, 1000);   // 0（已过期）
```

#### format_duration

格式化时间间隔为可读字符串。

```rust
pub fn format_duration(seconds: u64) -> Vec<u8>
```

**输出示例**：
| 输入（秒） | 输出 |
|-----------|------|
| 0 | `< 1m` |
| 59 | `< 1m` |
| 60 | `1m` |
| 300 | `5m` |
| 3600 | `1h` |
| 5400 | `1h 30m` |
| 86400 | `1d` |
| 90000 | `1d 1h` |

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

### 使用类型定义

```rust
use pallet_trading_common::{TronAddress, MomentOf, Cid};

// 定义存储
#[pallet::storage]
pub type MakerAddress<T> = StorageMap<_, Blake2_128Concat, u64, TronAddress>;
```

### 使用 Trait 接口

```rust
use pallet_trading_common::{PricingProvider, MakerInterface};

#[pallet::config]
pub trait Config: frame_system::Config {
    type PricingProvider: PricingProvider<BalanceOf<Self>>;
    type MakerProvider: MakerInterface<Self::AccountId, BalanceOf<Self>>;
}

// 在 dispatchable 中使用
impl<T: Config> Pallet<T> {
    fn calculate_cos_amount(usd_amount: u64) -> Option<BalanceOf<T>> {
        let rate = T::PricingProvider::get_cos_to_usd_rate()?;
        // 计算逻辑...
        Some(cos_amount)
    }
}
```

### 使用工具函数

```rust
use pallet_trading_common::{
    mask_name, mask_id_card, mask_birthday,
    is_valid_tron_address,
    blocks_to_seconds, estimate_remaining_seconds,
};

// 数据脱敏
let masked = mask_name("张三");  // "×三"

// 地址验证
if !is_valid_tron_address(&tron_address) {
    return Err(Error::<T>::InvalidTronAddress.into());
}

// 时间计算
let remaining = estimate_remaining_seconds(timeout_block, current_block);
```

## 版本历史

| 版本 | 日期 | 变更内容 |
|------|------|----------|
| v0.1.0 | - | 初始版本 |
| v0.2.0 | 2026-01-18 | 添加统一的 MakerCreditInterface trait |
| v0.3.0 | 2026-01-18 | 添加时间转换工具函数 |
| v0.4.0 | 2026-01-18 | 统一公共类型和 Trait 定义 |
| v0.5.0 | 2026-02-08 | 适配 P2P 统一模型：report_swap_order → report_p2p_trade |

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
