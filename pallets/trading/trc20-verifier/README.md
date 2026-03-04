# pallet-trading-trc20-verifier

TRC20 USDT 链下验证共享库 — 供 OCW 验证 TRON 链上 USDT 转账。

## 概述

`pallet-trading-trc20-verifier` 是纯 Rust crate（非 FRAME pallet，`no_std` 兼容），提供 Off-Chain Worker 可调用的 TRC20 验证函数。无链上存储，所有状态存储在 offchain local storage（PERSISTENT）。

### 核心能力

| 能力 | 说明 |
|------|------|
| **按 from/to/amount 搜索** | 查询收款方 TRC20 转入记录，匹配付款方 + USDT 合约，累计多笔金额 |
| **分页支持** | 自动翻页（fingerprint），遍历 >50 笔转账（可配置 max_pages） |
| **按 tx_hash 验证** | 查询 TronGrid 单笔交易详情，校验收款地址/合约/确认数/金额（已废弃，仅向后兼容） |
| **端点健康评分** | 动态评估 API 端点可用性（成功率 + 响应速度），自动按评分排序 + 优先级加成 |
| **并行竞速 / 串行故障转移** | 竞速模式使用最快成功响应；串行模式按评分依次尝试 |
| **API Key 支持** | 每个端点可配置独立 API Key（`TRON-PRO-API-KEY` 头） |
| **速率限制** | 全局请求间隔保护（默认 200ms），防止 API 限流 |
| **响应缓存** | URL 级缓存（默认 TTL 30s），减少重复请求 |
| **多档金额判定** | Exact / Overpaid / Underpaid / SeverelyUnderpaid / Invalid 五级 |
| **验证审计日志** | 记录每次验证的参数、结果、时间戳（可配置保留条数） |
| **可配置参数** | USDT 合约地址、最小确认数、超时、速率限制间隔、缓存 TTL 均可运行时配置 |
| **TronVerifier trait** | 上层 pallet 可注入 Mock 实现，便于单元测试 |

### 使用方

| 模块 | 用途 | 调用的 API |
|------|------|-----------|
| `pallet-nex-market` | NEX/USDT 交易验证（OCW 三阶段） | `verify_trc20_by_transfer` |
| `pallet-entity-market` | Entity Token/USDT 交易验证 | `verify_trc20_by_transfer` |

---

## 核心 API

### verify_trc20_by_transfer（主用 API）

按 `(from, to, amount)` 搜索 TRC20 USDT 转账，**累计同一 from→to 的多笔转账金额**，自动分页。

```rust
pub fn verify_trc20_by_transfer(
    from_address: &[u8],     // 付款方 TRON 地址（Base58）
    to_address: &[u8],       // 收款方 TRON 地址（Base58）
    expected_amount: u64,    // 预期 USDT 金额（精度 10^6）
    min_timestamp: u64,      // 最早区块时间戳（毫秒）
) -> Result<TransferSearchResult, VerificationError>
```

**查询逻辑**：

```text
1. 构建 TronGrid API URL（使用可配置 USDT 合约地址）
2. 分页循环（最多 max_pages 页，默认 3）:
   a. 发送 HTTP 请求（速率限制 + 缓存 + 故障转移）
   b. 解析响应，匹配 from + USDT 合约 + 确认数
   c. 累加匹配金额，记录每笔明细（MatchedTransfer）
   d. 若金额已足够(Exact/Overpaid) → 停止翻页
   e. 提取 fingerprint → 下一页
3. 写入审计日志
4. 返回 TransferSearchResult
```

### TronVerifier trait（H4）

上层 pallet 可通过此 trait 注入 Mock 实现：

```rust
pub trait TronVerifier {
    fn verify_by_transfer(
        from_address: &[u8], to_address: &[u8],
        expected_amount: u64, min_timestamp: u64,
    ) -> Result<TransferSearchResult, VerificationError>;
}

pub struct DefaultTronVerifier;  // 调用真实 TronGrid API
```

### verify_trc20_transaction（已废弃）

通过交易哈希查询单笔交易详情。**建议使用 `verify_trc20_by_transfer` 替代。**

```rust
#[deprecated(note = "Use verify_trc20_by_transfer instead")]
pub fn verify_trc20_transaction(
    tx_hash: &[u8], expected_to: &[u8], expected_amount: u64,
) -> Result<TronTxVerification, VerificationError>
```

---

## 错误类型

### VerificationError

所有公开函数统一使用结构化错误枚举：

```rust
pub enum VerificationError {
    HttpRequestFailed(&'static str),   // HTTP 请求失败
    InvalidJson,                        // JSON 解析失败
    InvalidUtf8,                        // UTF-8 解码失败
    AllEndpointsFailed,                 // 所有端点均失败
    RateLimited,                        // 速率限制中
    InvalidEndpointUrl(&'static str),  // 端点 URL 校验失败
    MaxEndpointsReached,               // 端点数量超限
}
```

---

## 数据结构

### TransferSearchResult

| 字段 | 类型 | 说明 |
|------|------|------|
| `found` | `bool` | 是否找到匹配的转账 |
| `actual_amount` | `Option<u64>` | 匹配转账的累计金额 |
| `tx_hash` | `Option<Vec<u8>>` | 最大单笔转账的交易哈希 |
| `block_timestamp` | `Option<u64>` | 最大单笔转账的区块时间戳（ms） |
| `amount_status` | `AmountStatus` | 金额匹配状态 |
| `error` | `Option<Vec<u8>>` | 错误信息 |
| `matched_transfers` | `Vec<MatchedTransfer>` | 所有匹配转账明细 |
| `remaining_amount` | `Option<u64>` | 还需补付金额（仅少付时有值） |
| `estimated_confirmations` | `Option<u32>` | 估计确认数（基于 block_timestamp） |
| `truncated` | `bool` | 结果是否被截断（分页未完全遍历） |

### MatchedTransfer

```rust
pub struct MatchedTransfer {
    pub tx_hash: Vec<u8>,
    pub amount: u64,
    pub block_timestamp: u64,
}
```

### AmountStatus

五级金额匹配判定：

```rust
pub enum AmountStatus {
    Unknown,                             // 未验证
    Exact,                               // ±0.5% 以内
    Overpaid { excess: u64 },            // > +0.5%
    Underpaid { shortage: u64 },         // 50% ~ -0.5%
    SeverelyUnderpaid { shortage: u64 }, // < 50%
    Invalid,                             // 金额为零或解析失败
}

impl AmountStatus {
    pub fn is_acceptable(&self) -> bool;                // Exact | Overpaid
    pub fn to_verification_result_name(&self) -> &str;  // 与 PaymentVerificationResult 兼容
}
```

| 状态 | 条件 | 上层处理 |
|------|------|---------|
| Exact | `actual ∈ [expected×0.995, expected×1.005]` | 全额结算 |
| Overpaid | `actual > expected × 1.005` | 全额结算 |
| Underpaid | `actual ∈ [expected×0.5, expected×0.995)` | 进入补付窗口 |
| SeverelyUnderpaid | `actual < expected × 0.5` | 按比例释放 + 没收保证金 |
| Invalid | `actual = 0` 或 `expected = 0` | 不释放 + 没收保证金 |

---

## 配置

### VerifierConfig（运行时可配置）

```rust
pub struct VerifierConfig {
    pub usdt_contract: Option<String>,       // 覆盖 USDT 合约地址
    pub min_confirmations: Option<u32>,      // 覆盖最小确认数
    pub rate_limit_interval_ms: u64,         // 请求间隔（默认 200ms）
    pub cache_ttl_ms: u64,                   // 响应缓存 TTL（默认 30s）
    pub max_pages: u32,                      // 分页上限（默认 3）
    pub audit_log_retention: u32,            // 审计日志保留条数（默认 100）
}
```

存储键: `ocw_verifier_config`

### EndpointConfig

```rust
pub struct EndpointConfig {
    pub endpoints: Vec<String>,      // 端点 URL 列表
    pub parallel_mode: bool,         // 是否并行竞速（默认 true）
    pub updated_at: u64,             // 最后更新时间
    pub api_keys: Vec<String>,       // 每个端点的 API Key（索引对应）
    pub timeout_ms: Option<u64>,     // 覆盖串行超时
    pub timeout_race_ms: Option<u64>,// 覆盖竞速超时
    pub priority_boosts: Vec<i32>,   // 端点评分加成（索引对应）
}
```

存储键: `ocw_custom_endpoints`

### 端点管理函数

| 函数 | 说明 |
|------|------|
| `add_endpoint(url)` | 添加端点（HTTPS 校验 + 最大 10 个限制） |
| `remove_endpoint(url)` | 移除端点 |
| `get_sorted_endpoints()` | 按健康评分+优先级降序排列 |
| `reset_endpoint_health(endpoint)` | 重置指定端点健康数据 |
| `get_all_endpoint_diagnostics()` | 批量获取所有端点诊断信息 |

---

## 端点健康评分系统

### EndpointHealth

```rust
pub struct EndpointHealth {
    pub success_count: u32,
    pub failure_count: u32,
    pub avg_response_ms: u32,   // EMA 衰减因子 90%
    pub score: u32,             // 0-100
    pub last_updated: u64,
}
```

### 评分公式

```text
score = success_rate_score(0-50) + speed_score(0-50)
      + priority_boost (EndpointConfig.priority_boosts[i])

success_rate = success / (success + failure) × 50
speed: <1000ms→50, >10000ms→0, 线性插值
初始: 50
```

---

## 速率限制 & 缓存

### 速率限制

全局请求间隔保护，防止 TronGrid 429 限流：

- 默认间隔: 200ms（通过 `VerifierConfig.rate_limit_interval_ms` 配置）
- 存储键: `ocw_last_request_ts`
- 间隔内请求返回 `VerificationError::RateLimited`

### 响应缓存

URL 级响应缓存，减少重复请求：

- 默认 TTL: 30s（通过 `VerifierConfig.cache_ttl_ms` 配置）
- 存储键: `ocw_cache::{url_hash}`
- `fetch_url_with_fallback` 自动检查缓存

---

## 审计日志

每次 `verify_trc20_by_transfer` 调用自动记录：

```rust
pub struct AuditLogEntry {
    pub timestamp: u64,
    pub action: Vec<u8>,
    pub from_address: Vec<u8>,
    pub to_address: Vec<u8>,
    pub expected_amount: u64,
    pub actual_amount: u64,
    pub result_ok: bool,
    pub error_msg: Vec<u8>,
}
```

| 函数 | 说明 |
|------|------|
| `get_audit_logs()` | 获取所有审计日志 |
| `write_audit_log(entry)` | 写入日志（自动截断至 `audit_log_retention`） |

---

## JSON 解析

使用 `lite-json` 结构化解析（`no_std` 兼容），替代旧版字符串匹配。

### 解析工具函数

| 函数 | 说明 |
|------|------|
| `json_find_str(value, key)` | 递归查找字符串字段 |
| `json_find_u64(value, key)` | 递归查找数字字段（支持字符串数字） |
| `json_has_str_value(value, target)` | 检查是否包含完整字符串值（非子串） |
| `json_obj_get(obj, key)` | 对象级字段查找 |
| `json_obj_get_str(obj, key)` | 对象级字符串字段 |
| `json_obj_get_u64(obj, key)` | 对象级数字字段 |

---

## 工具函数

| 函数 | 签名 | 说明 |
|------|------|------|
| `bytes_to_hex` | `&[u8] → String` | 字节转十六进制 |
| `hex_to_bytes` | `&str → Result<Vec<u8>>` | 十六进制转字节（自动去 0x 前缀） |
| `calculate_amount_status` | `(u64, u64) → AmountStatus` | 五级金额判定 |

---

## 常量配置

| 常量 | 值 | 说明 |
|------|------|------|
| `USDT_CONTRACT` | `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` | USDT TRC20 合约地址（默认，可通过 VerifierConfig 覆盖） |
| `TRONGRID_MAINNET` | `https://api.trongrid.io` | 主端点 URL 模板 |
| `HTTP_TIMEOUT_MS` | 10,000 | 串行模式超时（可通过 EndpointConfig 覆盖） |
| `HTTP_TIMEOUT_RACE_MS` | 5,000 | 竞速模式超时（可通过 EndpointConfig 覆盖） |
| `MIN_CONFIRMATIONS` | 19 | 最小确认数（可通过 VerifierConfig 覆盖） |
| `MAX_ENDPOINTS` | 10 | 最大端点数量 |
| `HEALTH_DECAY_FACTOR` | 90 | EMA 衰减因子 |

---

## 安全设计

### 交易验证安全

| 防护 | 说明 |
|------|------|
| **合约地址校验** | lite-json 精确字符串匹配（非子串），防其他 TRC20 冒充 |
| **最小确认数** | 默认 ≥ 19，防 TRON 链重组回滚 |
| **Base58 地址匹配** | 直接 UTF-8 字符串匹配，避免 hex 编码永不匹配 |
| **u128 中间计算** | 金额阈值防 u64 溢出，极端值 `.min(u64::MAX)` 防截断回绕 |
| **expected=0 → Invalid** | 与 pallet-trading-common 语义一致 |
| **双重合约过滤** | URL 参数 + 响应体内 token_info.address 双重校验 |

### 端点安全

| 防护 | 说明 |
|------|------|
| **HTTPS 强制** | `add_endpoint` 拒绝非 HTTPS URL |
| **URL 校验** | 长度 10-256，无空白字符 |
| **最大端点数** | 限制 10 个，防存储膨胀 |
| **故障隔离** | 单端点失败不影响其他端点 |
| **速率限制** | 全局请求间隔防 API 限流 |

---

## 测试

```bash
cargo test -p pallet-trading-trc20-verifier
```

覆盖范围（45 个测试）：

**基础工具** — `bytes_to_hex`, `hex_to_bytes`（含 0x 前缀），边界值

**端点健康** — 评分公式（默认/高成功率/慢响应）

**JSON 解析 (lite-json)** — `json_find_str`, `json_has_str_value` 精确匹配, `extract_amount`, `extract_json_string_value`, `extract_json_number`, `find_matching_brace`

**转账搜索** — 精确匹配 / 多付 / 少付 / 严重少付 / from 不匹配 / 空数组 / API failure / 多笔累加 / 非 USDT 合约忽略 / JSON 字符串中的 `{}` 特殊字符

**审计回归** — H1 Base58 匹配, H6 发送方校验, H7 expected=0, H8 端点校验, C1 精确字段匹配, C2 字段名提取, M2 大金额不溢出, M5 lite-json 替代, M8 极端值防回绕, L1 最大单笔跟踪, L2 expected=0, L6 0x 前缀

---

## 依赖

| crate | 用途 |
|-------|------|
| `codec` (SCALE) | 端点健康/配置/审计日志序列化 |
| `sp-runtime` | OCW HTTP 客户端 |
| `sp-core` | offchain StorageKind |
| `sp-io` | 时间戳 / 本地存储读写 |
| `sp-std` | `no_std` 兼容 |
| `log` | OCW 日志 |
| `lite-json` | `no_std` JSON 结构化解析 |

---

## 版本历史

| 版本 | 日期 | 说明 |
|------|------|------|
| v0.3.0 | 2026-03-04 | **重大增强**: VerificationError 枚举统一错误处理, VerifierConfig 运行时配置(合约/确认数/速率/缓存/分页/审计), EndpointConfig 增强(API Key/超时/优先级), TronVerifier trait 抽象, 分页支持(fingerprint), 速率限制+响应缓存, 审计日志, TransferSearchResult 增强(matched_transfers/remaining_amount/estimated_confirmations/truncated), AmountStatus.is_acceptable()+to_verification_result_name(), 端点健康重置+批量诊断, lite-json 结构化解析, 端点最大数量限制(10), 废弃 verify_trc20_transaction |
| v0.2.0 | 2026-02-23 | 新增 `verify_trc20_by_transfer`、`TransferSearchResult`、`parse_trc20_transfer_list`、JSON 工具函数 |
| v0.1.0 | 2026-02-08 | 从 `pallet-trading-swap/src/ocw.rs` 提取为独立共享库 |

---

**License**: Unlicense
