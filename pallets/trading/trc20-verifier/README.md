# pallet-trading-trc20-verifier

TRC20 USDT 链下验证共享库 — 供 OCW 验证 TRON 链上 USDT 转账。

## 概述

`pallet-trading-trc20-verifier` 是纯 Rust crate（非 FRAME pallet，`no_std` 兼容），提供 Off-Chain Worker 可调用的 TRC20 验证函数。无链上存储，所有状态存储在 offchain local storage（PERSISTENT）。

### 核心能力

| 能力 | 说明 |
|------|------|
| **按 tx_hash 验证** | 查询 TronGrid 单笔交易详情，校验收款地址/合约/确认数/金额 |
| **按 from/to/amount 搜索** | 查询收款方 TRC20 转入记录，匹配付款方 + USDT 合约，累计多笔金额 |
| **端点健康评分** | 动态评估 API 端点可用性（成功率 + 响应速度），自动按评分排序 |
| **并行竞速模式** | 同时请求所有端点，使用最快成功响应（5s 超时） |
| **串行故障转移** | 按健康评分依次尝试，首个成功即返回（10s 超时） |
| **多档金额判定** | Exact / Overpaid / Underpaid / SeverelyUnderpaid / Invalid 五级 |
| **安全校验** | USDT 合约地址验证 + 最小确认数(19) + 防 u64 溢出 |

### 使用方

| 模块 | 用途 | 调用的 API |
|------|------|-----------|
| `pallet-nex-market` | NEX/USDT 交易验证（OCW 三阶段） | `verify_trc20_by_transfer` |
| `pallet-entity-market` | Entity Token/USDT 交易验证 | `verify_trc20_by_transfer` |

---

## 核心 API

### verify_trc20_by_transfer（主用 API）

按 `(from, to, amount)` 搜索 TRC20 USDT 转账，**累计同一 from→to 的多笔转账金额**。

```rust
pub fn verify_trc20_by_transfer(
    from_address: &[u8],     // 付款方 TRON 地址（Base58，如 "TBuyer..."）
    to_address: &[u8],       // 收款方 TRON 地址（Base58，如 "TSeller..."）
    expected_amount: u64,    // 预期 USDT 金额（精度 10^6）
    min_timestamp: u64,      // 最早区块时间戳（毫秒），仅搜索此时间之后
) -> Result<TransferSearchResult, &'static str>
```

**查询逻辑**：

```text
1. 构建 TronGrid API URL:
   /v1/accounts/{to}/transactions/trc20
     ?contract_address=TR7NHq...（USDT 合约）
     &only_to=true
     &min_timestamp={min_timestamp}
     &limit=50
     &order_by=block_timestamp,desc

2. 发送 HTTP 请求（并行竞速或串行故障转移）

3. 遍历 data 数组中每个转账条目:
   - 检查 from 地址是否匹配
   - 检查 token_info.address 是否为 USDT 合约（双重保险）
   - 提取 value 并累加到 total_matched_amount
   - 记录最大单笔转账的 tx_hash 和 block_timestamp

4. 返回 TransferSearchResult（含累计金额和匹配状态）
```

### verify_trc20_transaction（按 tx_hash 验证）

通过交易哈希查询单笔交易详情，完整校验链。

```rust
pub fn verify_trc20_transaction(
    tx_hash: &[u8],        // TRON 交易哈希（字节数组）
    expected_to: &[u8],    // 预期收款地址（Base58 字符串字节）
    expected_amount: u64,  // 预期金额（USDT，精度 10^6）
) -> Result<TronTxVerification, &'static str>
```

**校验链**：

```text
1. contractRet == "SUCCESS"       → 否则返回 "Transaction not successful"
2. 包含 USDT_CONTRACT 地址        → 否则返回 "Not a USDT TRC20 transaction"
3. confirmations >= 19            → 否则返回 "Insufficient confirmations"
4. 包含 expected_to 地址          → 否则返回 "Recipient address mismatch"
5. 提取 amount → 多档金额判定     → 仅 Exact/Overpaid 时 is_valid=true
```

### verify_trc20_transaction_simple

简化接口，仅返回 `bool`（`is_valid`）。

```rust
pub fn verify_trc20_transaction_simple(
    tx_hash: &[u8],
    expected_to: &[u8],
    expected_amount: u64,
) -> Result<bool, &'static str>
```

---

## 数据结构

### TransferSearchResult

`verify_trc20_by_transfer` 的返回值。

| 字段 | 类型 | 说明 |
|------|------|------|
| `found` | `bool` | 是否找到匹配的转账 |
| `actual_amount` | `Option<u64>` | 匹配转账的累计金额（多笔累加） |
| `tx_hash` | `Option<Vec<u8>>` | 最大单笔转账的交易哈希 |
| `block_timestamp` | `Option<u64>` | 最大单笔转账的区块时间戳（ms） |
| `amount_status` | `AmountStatus` | 金额匹配状态 |
| `error` | `Option<Vec<u8>>` | 错误信息 |

### TronTxVerification

`verify_trc20_transaction` 的返回值。

| 字段 | 类型 | 说明 |
|------|------|------|
| `tx_hash` | `Vec<u8>` | 交易哈希 |
| `is_valid` | `bool` | 验证是否通过（仅 Exact/Overpaid 为 true） |
| `from_address` | `Option<Vec<u8>>` | 付款方地址 |
| `to_address` | `Option<Vec<u8>>` | 收款方地址 |
| `actual_amount` | `Option<u64>` | 实际转账金额 |
| `expected_amount` | `Option<u64>` | 预期金额 |
| `confirmations` | `u32` | 确认数 |
| `error` | `Option<Vec<u8>>` | 错误信息 |
| `amount_status` | `AmountStatus` | 金额匹配状态 |

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
```

| 状态 | 条件 | 上层处理 |
|------|------|---------|
| Exact | `actual ∈ [expected×0.995, expected×1.005]` | 全额结算 |
| Overpaid | `actual > expected × 1.005` | 全额结算 |
| Underpaid | `actual ∈ [expected×0.5, expected×0.995)` | 进入补付窗口 |
| SeverelyUnderpaid | `actual < expected × 0.5` | 按比例释放 + 没收保证金 |
| Invalid | `actual = 0` 或 `expected = 0` | 不释放 + 没收保证金 |

> 金额阈值计算使用 u128 中间值防止大金额乘法溢出（M2 修复）。`expected=0` 返回 Invalid（L2 修复，与 pallet-trading-common 语义统一）。

---

## 端点健康评分系统

### EndpointHealth

```rust
pub struct EndpointHealth {
    pub success_count: u32,     // 累计成功次数
    pub failure_count: u32,     // 累计失败次数
    pub avg_response_ms: u32,   // 指数移动平均响应时间（衰减因子 90%）
    pub score: u32,             // 健康评分 (0-100)
    pub last_updated: u64,      // 最后更新时间戳（ms）
}
```

### 评分公式

```text
score = success_rate_score + speed_score

success_rate_score (0-50):
  = success_count / (success_count + failure_count) × 50

speed_score (0-50):
  avg_response_ms < 1000ms  → 50
  avg_response_ms > 10000ms → 0
  其他 → 线性插值: 50 - (avg_ms - 1000) × 50 / 9000

初始评分（无请求记录）: 50
```

### 响应时间更新（EMA）

```text
avg_response_ms = old_avg × 0.9 + new_response × 0.1
```

### 存储

评分数据存储在 offchain local storage（PERSISTENT），键格式 `ocw_endpoint_health::{endpoint_url}`。

---

## 请求模式

### 并行竞速模式（默认）

同时向所有端点发送 HTTP GET 请求，使用**第一个成功响应**（HTTP 200 + 非空 body），超时 5s。

```text
                   ┌─→ api.trongrid.io    ──→ 200 OK (350ms) ✓ 使用此响应
Parallel Race ─────┼─→ api.tronstack.io   ──→ 超时
                   └─→ apilist.tronscan... ──→ 200 OK (800ms)
```

- 成功端点记录 `record_success(response_ms)`
- 失败/超时端点记录 `record_failure()`

### 串行故障转移模式

按健康评分**降序**依次尝试，首个成功即返回，超时 10s。

```text
Sequential: trongrid(score=95) → 失败 → tronstack(score=70) → 200 OK ✓
```

### 切换模式

```rust
let mut config = get_endpoint_config();
config.parallel_mode = false;  // 关闭并行，启用串行
save_endpoint_config(&config);
```

---

## 端点管理

### EndpointConfig

```rust
pub struct EndpointConfig {
    pub endpoints: Vec<String>,  // 端点 URL 列表
    pub parallel_mode: bool,     // 是否并行竞速（默认 true）
    pub updated_at: u64,         // 最后更新时间
}
```

存储在 offchain local storage，键 `ocw_custom_endpoints`。

### 默认端点

| 端点 | 说明 |
|------|------|
| `https://api.trongrid.io` | TronGrid 官方（主端点，用于 URL 构建模板） |
| `https://api.tronstack.io` | TronStack 第三方 |
| `https://apilist.tronscanapi.com` | TronScan |

### 管理函数

| 函数 | 说明 |
|------|------|
| `get_endpoint_config()` | 获取当前端点配置（无配置时返回默认值） |
| `save_endpoint_config(config)` | 保存端点配置 |
| `add_endpoint(url)` | 添加自定义端点（去重） |
| `remove_endpoint(url)` | 移除端点 |
| `get_sorted_endpoints()` | 获取按健康评分降序排列的端点列表 |
| `get_endpoint_health(endpoint)` | 获取指定端点的健康状态 |

---

## JSON 解析

本库使用轻量级字符串匹配解析 TronGrid JSON 响应（`no_std` 兼容，无需 serde/json crate）。

### 解析工具函数

| 函数 | 说明 |
|------|------|
| `extract_json_string_value(json, key)` | 提取字符串字段（支持 `"key":"value"` 和 `"key": "value"`） |
| `extract_json_number(json, key)` | 提取数字字段（支持数字和引号包裹的数字） |
| `extract_amount(response)` | 提取 `"amount"` 字段 |
| `extract_confirmations(response)` | 提取 `"confirmations"` 字段 |
| `find_matching_brace(s, pos)` | 找到匹配的 `}` 括号（支持一层嵌套，防下溢） |

### 转账列表解析逻辑

`parse_trc20_transfer_list` 解析 TronGrid `/v1/accounts/{addr}/transactions/trc20` 响应：

```text
1. 检查 "success":true
2. 定位 "data":[ 数组
3. 遍历每个 {} 条目（find_matching_brace 支持嵌套 token_info）:
   a. 匹配 "from":"expected_from"
   b. 匹配 USDT_CONTRACT 地址
   c. 提取 "value" 并累加到 total_matched_amount
   d. 跟踪最大单笔的 tx_hash / block_timestamp
4. 计算 calculate_amount_status(expected, total)
```

> **安全说明**：字符串包含匹配在 OCW 安全边界内可接受，但需限制可信端点以防恶意 API 注入匹配字符串。

---

## 工具函数

| 函数 | 签名 | 说明 |
|------|------|------|
| `bytes_to_hex(bytes)` | `&[u8] → String` | 字节数组转十六进制字符串 |
| `hex_to_bytes(hex)` | `&str → Result<Vec<u8>, &str>` | 十六进制字符串转字节数组 |
| `calculate_amount_status(expected, actual)` | `(u64, u64) → AmountStatus` | 五级金额匹配判定（公开 API） |

---

## 常量配置

| 常量 | 值 | 说明 |
|------|------|------|
| `USDT_CONTRACT` | `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` | USDT TRC20 合约地址（主网） |
| `TRONGRID_MAINNET` | `https://api.trongrid.io` | 主端点（URL 构建模板） |
| `HTTP_TIMEOUT_MS` | 10,000 | 串行模式单请求超时（ms） |
| `HTTP_TIMEOUT_RACE_MS` | 5,000 | 并行竞速模式超时（ms） |
| `MIN_CONFIRMATIONS` | 19 | 最小确认数（防链重组） |
| `HEALTH_DECAY_FACTOR` | 90 | 响应时间 EMA 衰减因子（90% 旧 + 10% 新） |
| `ENDPOINT_HEALTH_PREFIX` | `ocw_endpoint_health::` | 端点健康数据存储键前缀 |
| `CUSTOM_ENDPOINTS_KEY` | `ocw_custom_endpoints` | 端点配置存储键 |

---

## 安全设计

### 交易验证安全

| 防护 | 说明 |
|------|------|
| **合约地址校验** | 每次验证均检查 USDT 合约地址 `TR7NHq...`，防止其他 TRC20 代币冒充（M6 修复） |
| **最小确认数** | 要求 ≥ 19 confirmations，防止 TRON 链重组导致回滚（L1 修复） |
| **Base58 地址匹配** | `expected_to` 直接按 UTF-8 字符串匹配，避免 hex 编码导致永不匹配（H1 修复） |
| **u128 中间计算** | 金额阈值计算使用 u128 防止 `expected × 1005` 溢出 u64（M2 修复） |
| **expected=0 处理** | `calculate_amount_status(0, any)` 返回 Invalid，与 pallet-trading-common 语义一致（L2 修复） |
| **括号匹配防下溢** | `find_matching_brace` 使用 `checked_sub` 防止恶意响应导致 depth 下溢（L3 修复） |
| **双重合约过滤** | 转账搜索：URL 参数 `contract_address` + 响应体内 `token_info.address` 双重校验 |

### 端点安全

| 防护 | 说明 |
|------|------|
| **主网限定** | 所有默认端点为 TRON 主网，代码注释标注禁止使用测试网 |
| **故障隔离** | 请求失败不影响其他端点，评分自动降级 |
| **评分衰减** | 新请求结果仅占 10% 权重（EMA），防止单次异常翻转评分 |

---

## 测试

```bash
cargo test -p pallet-trading-trc20-verifier
```

覆盖范围（27 个测试）：

**基础工具**
- `bytes_to_hex` / `hex_to_bytes` 正确性和边界

**端点健康**
- 评分公式（默认/高成功率/慢响应）

**JSON 解析**
- `extract_amount` 多格式
- `extract_json_string_value` 有/无空格格式
- `extract_json_number` 数字和字符串数字格式
- `find_matching_brace` 嵌套和异常

**转账搜索**
- 精确匹配 / 多付 / 少付 / 严重少付
- from 地址不匹配 → 不找到
- 空 data 数组
- API 返回 failure
- 多笔转账累加（5M + 6M = 11M）
- 非 USDT 合约忽略

**审计回归**
- H1: Base58 地址直接匹配（非 hex 编码）
- H1: 地址不匹配 / 确认数不足 / 非 USDT 合约 / 交易失败
- M2: 大金额（10^16 USDT）不溢出
- L1: `best_tx_hash` 跟踪最大单笔（非前序总和）
- L2: `expected=0` 返回 Invalid

---

## 依赖

| crate | 用途 |
|-------|------|
| `codec` (SCALE) | 端点健康/配置序列化 |
| `sp-runtime` | OCW HTTP 客户端（`offchain::http`） |
| `sp-core` | offchain `StorageKind` |
| `sp-io` | 时间戳 / 本地存储读写 |
| `sp-std` | `no_std` 兼容 |
| `log` | OCW 日志输出 |

---

## 版本历史

| 版本 | 日期 | 说明 |
|------|------|------|
| v0.2.0 | 2026-02-23 | 新增 `verify_trc20_by_transfer`（按 from/to 搜索 + 累加）、`TransferSearchResult`、`parse_trc20_transfer_list`、JSON 解析工具函数 |
| v0.1.0 | 2026-02-08 | 从 `pallet-trading-swap/src/ocw.rs` 提取为独立共享库 |

---

**License**: Unlicense
