# TRC20 Verifier（TRC20 交易验证共享库）

## 概述

`pallet-trading-trc20-verifier` 是 TRC20（TRON）交易链下验证的共享库，供 Off-Chain Worker (OCW) 在链下验证 USDT TRC20 转账是否真实完成。

> **注意**：本模块是纯 Rust crate（非 FRAME pallet），无链上存储，仅提供 OCW 可调用的验证函数。

### 主要功能

- **TRC20 交易验证**：通过 TronGrid API 验证 USDT 转账
- **端点健康评分**：动态评估 API 端点可用性，自动排序
- **并行竞速模式**：同时请求多个端点，使用最快响应
- **串行故障转移**：按健康评分依次尝试端点
- **金额匹配判定**：精确匹配、多付、少付、严重不足 4 级判定
- **可配置端点**：支持运行时动态添加/移除 API 端点

---

## 核心 API

### verify_trc20_transaction

完整验证接口，返回详细验证结果。

```rust
pub fn verify_trc20_transaction(
    tx_hash: &[u8],        // TRON 交易哈希
    expected_to: &[u8],    // 预期收款地址
    expected_amount: u64,  // 预期金额（USDT，精度 10^6）
) -> Result<TronTxVerification, &'static str>
```

### verify_trc20_transaction_simple

简化接口，仅返回 `bool`。

```rust
pub fn verify_trc20_transaction_simple(
    tx_hash: &[u8],
    expected_to: &[u8],
    expected_amount: u64,
) -> Result<bool, &'static str>
```

---

## 金额匹配状态

```rust
pub enum AmountStatus {
    Unknown,                          // 未验证
    Exact,                            // 完全匹配（±0.5%）
    Overpaid { excess: u64 },         // 多付
    Underpaid { shortage: u64 },      // 少付（≥50%）
    SeverelyUnderpaid { shortage: u64 }, // 严重不足（<50%）
    Invalid,                          // 金额为零或无法解析
}
```

| 状态 | 条件 | 是否接受 |
|------|------|:---:|
| Exact | `actual ∈ [expected×0.995, expected×1.005]` | ✅ |
| Overpaid | `actual > expected×1.005` | ✅ |
| Underpaid | `actual ∈ [expected×0.5, expected×0.995)` | ❌ |
| SeverelyUnderpaid | `actual < expected×0.5` | ❌ |
| Invalid | `actual = 0` 或解析失败 | ❌ |

---

## 端点健康评分

### 评分公式

```
score = success_rate × 50 + response_speed × 50
```

- **success_rate (0-50)**：`成功次数 / 总次数 × 50`
- **response_speed (0-50)**：`<1000ms → 50分`，`>10000ms → 0分`，线性插值

### 默认端点

| 端点 | 说明 |
|------|------|
| `https://api.trongrid.io` | TronGrid 官方 |
| `https://api.tronstack.io` | TronStack 第三方 |
| `https://apilist.tronscanapi.com` | TronScan |

### 端点管理

```rust
// 添加自定义端点
add_endpoint("https://custom-tron-api.example.com");

// 移除端点
remove_endpoint("https://api.tronstack.io");

// 获取按健康评分排序的端点
let sorted = get_sorted_endpoints();
```

---

## 请求模式

### 并行竞速模式（默认）

同时向所有端点发送请求，使用最快成功响应。超时 5 秒。

### 串行故障转移模式

按健康评分从高到低依次尝试，首个成功即返回。超时 10 秒。

切换方式：

```rust
let mut config = get_endpoint_config();
config.parallel_mode = false; // 关闭并行
save_endpoint_config(&config);
```

---

## 常量配置

| 常量 | 值 | 说明 |
|------|------|------|
| `USDT_CONTRACT` | `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` | USDT TRC20 合约地址（主网） |
| `HTTP_TIMEOUT_MS` | 10,000 | 串行模式超时（ms） |
| `HTTP_TIMEOUT_RACE_MS` | 5,000 | 并行竞速超时（ms） |
| `MIN_CONFIRMATIONS` | 19 | 最小确认数 |
| `HEALTH_DECAY_FACTOR` | 90 | 健康评分衰减因子（%） |

---

## 数据结构

### TronTxVerification

```rust
pub struct TronTxVerification {
    pub tx_hash: Vec<u8>,
    pub is_valid: bool,
    pub from_address: Option<Vec<u8>>,
    pub to_address: Option<Vec<u8>>,
    pub actual_amount: Option<u64>,
    pub expected_amount: Option<u64>,
    pub confirmations: u32,
    pub error: Option<Vec<u8>>,
    pub amount_status: AmountStatus,
}
```

### EndpointHealth

```rust
pub struct EndpointHealth {
    pub success_count: u32,
    pub failure_count: u32,
    pub avg_response_ms: u32,
    pub score: u32,          // 0-100
    pub last_updated: u64,
}
```

---

## 使用方

| 模块 | 用途 |
|------|------|
| `pallet-nex-market` | NEX/USDT 交易 OCW 验证 TRC20 转账 |
| `pallet-entity-market` | Entity Token/USDT 交易 OCW 验证 TRC20 转账 |

---

## 依赖

- `sp-runtime`：OCW HTTP 客户端
- `sp-core`：离线存储
- `sp-io`：时间戳、本地存储
- `codec`：SCALE 编解码

---

## 版本历史

| 版本 | 日期 | 说明 |
|------|------|------|
| v0.1.0 | 2026-02-08 | 从 `pallet-trading-swap/src/ocw.rs` 提取为独立共享库 |

---

## License

Unlicense
