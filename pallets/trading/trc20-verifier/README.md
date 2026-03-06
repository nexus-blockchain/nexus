# pallet-trading-trc20-verifier

Off-Chain Worker 可用的 TRC20 USDT 转账验证库。

纯 Rust crate（非 FRAME pallet），`no_std` 兼容。无链上存储，全部状态持久化在 offchain local storage。

---

## 快速集成

```rust
// 1. 上层 pallet 在 OCW 中调用
let result = pallet_trading_trc20_verifier::verify_trc20_by_transfer(
    b"T<buyer_address>",   // 付款方
    b"T<seller_address>",  // 收款方
    10_000_000,            // 10 USDT (精度 10^6)
    1700000000000,         // 最早时间戳 (ms)
)?;

// 2. 判断结果
if result.amount_status.is_acceptable() {
    // 注册 tx_hash 防止重复使用
    pallet_trading_trc20_verifier::register_result_tx_hashes(&result);
    // 执行链上结算...
}
```

**使用方**:

- `pallet-nex-market` — NEX/USDT 交易验证（OCW 三阶段）
- `pallet-entity-market` — Entity Token/USDT 交易验证

上层可通过 `TronVerifier` trait 注入 Mock，测试时无需真实网络。

---

## 架构总览

```text
┌─ 上层 pallet (nex-market / entity-market) ─┐
│  verify_trc20_by_transfer()                 │
│  register_result_tx_hashes()                │
└─────────────────┬───────────────────────────┘
                  │
┌─ trc20-verifier ┼───────────────────────────┐
│                 ▼                           │
│  ┌─ 前置检查 ────────────────────┐          │
│  │ kill switch · 地址校验 · 时间窗 │         │
│  │ OCW 并发锁 (CAS)              │          │
│  └────────────┬──────────────────┘          │
│               ▼                             │
│  ┌─ HTTP 层 ─────────────────────┐          │
│  │ 速率限制 → 缓存 → 请求模式选择  │         │
│  │   ├─ 串行故障转移 (默认)       │          │
│  │   └─ 并行竞速                 │          │
│  │ 端点健康评分 · 熔断隔离 · API Key│         │
│  └────────────┬──────────────────┘          │
│               ▼                             │
│  ┌─ 解析层 ──────────────────────┐          │
│  │ lite-json 解析 TronGrid 响应   │          │
│  │ 合约精确匹配 · 确认数检查       │          │
│  │ 多笔累加 · 分页 (fingerprint)  │          │
│  └────────────┬──────────────────┘          │
│               ▼                             │
│  ┌─ 后置处理 ────────────────────┐          │
│  │ tx_hash 重放过滤 · 金额状态判定 │         │
│  │ 审计日志 · 监控指标             │          │
│  └───────────────────────────────┘          │
└─────────────────────────────────────────────┘
```

---

## 公开 API

### 验证

| 函数 | 说明 |
|------|------|
| `verify_trc20_by_transfer(from, to, amount, min_ts)` | 核心入口 — 搜索 TRC20 转账并返回金额匹配结果 |
| `parse_trc20_transfer_list(response, from, amount, now)` | 解析 TronGrid 响应（供测试/离线分析） |
| `calculate_amount_status(expected, actual, tolerance_bps)` | 五级金额匹配判定 |

### tx_hash 重放防护

| 函数 | 说明 |
|------|------|
| `is_tx_hash_used(tx_hash)` | 查询 tx_hash 是否已注册 |
| `register_used_tx_hash(tx_hash)` | 注册单个 tx_hash |
| `register_result_tx_hashes(result)` | 批量注册结果中所有 tx_hash |

设计原则：`verify_trc20_by_transfer` 内部只**过滤**已使用的 tx_hash，**不自动注册**。上层 pallet 确认接受验证结果后再调用 `register_result_tx_hashes`，避免因上层拒绝导致的误注册。

### 端点管理

| 函数 | 说明 |
|------|------|
| `add_endpoint(url)` | 添加端点 |
| `remove_endpoint(url)` | 移除端点（同时清理 api_key、priority_boost） |
| `set_api_key(endpoint, key)` | 设置端点 API Key |
| `set_endpoint_priority_boost(endpoint, boost)` | 设置评分加成 |
| `get_sorted_endpoints()` | 获取按评分降序排列的端点（跳过熔断中的） |
| `get_endpoint_health(endpoint)` | 获取单个端点健康状态 |
| `get_all_endpoint_health()` | 获取所有端点健康状态 |
| `reset_endpoint_health(endpoint)` | 重置端点评分 |
| `is_endpoint_quarantined(endpoint)` | 检查端点是否处于熔断隔离 |

### 配置

| 函数 | 说明 |
|------|------|
| `get_verifier_config()` | 读取验证器配置（支持版本化迁移） |
| `save_verifier_config(config)` | 保存配置（自动校验参数范围） |
| `get_endpoint_config()` | 读取端点配置 |
| `save_endpoint_config(config)` | 保存端点配置（自动校验 HTTPS/SSRF/长度） |
| `validate_tron_address(address)` | 校验 TRON Base58Check 地址 |

### OCW 并发控制

| 函数 | 说明 |
|------|------|
| `try_acquire_verify_lock(lock_id) → Option<token>` | CAS 获取锁，返回令牌 |
| `release_verify_lock(lock_id, token)` | 令牌验证释放，防误释放 |

### 缓存 & 监控

| 函数 | 说明 |
|------|------|
| `cleanup_expired_cache()` | 清理过期缓存 |
| `get_verifier_metrics()` | 获取累计指标 |
| `reset_verifier_metrics()` | 重置指标 |
| `get_recent_audit_logs(max_count)` | 获取最近 N 条审计日志 |

### 工具

| 函数 | 说明 |
|------|------|
| `bytes_to_hex(bytes) → String` | 字节 → 十六进制 |
| `hex_to_bytes(hex) → Result<Vec<u8>>` | 十六进制 → 字节（自动去 `0x`） |

### Trait

```rust
pub trait TronVerifier {
    fn verify_by_transfer(from, to, amount, min_ts) -> Result<TransferSearchResult, VerificationError>;
}

pub struct DefaultTronVerifier;  // 调用真实 TronGrid API
```

---

## 数据结构

### TransferSearchResult

```rust
pub struct TransferSearchResult {
    pub found: bool,                               // 是否找到匹配转账
    pub actual_amount: Option<u64>,                 // 累计匹配金额
    pub tx_hash: Option<Vec<u8>>,                   // 最大单笔的 tx_hash
    pub block_timestamp: Option<u64>,               // 最大单笔的时间戳 (ms)
    pub amount_status: AmountStatus,                // 金额匹配状态
    pub error: Option<Vec<u8>>,                     // 错误信息
    pub matched_transfers: Vec<MatchedTransfer>,    // 所有匹配明细
    pub remaining_amount: Option<u64>,              // 还需补付金额（少付时）
    pub estimated_confirmations: Option<u32>,       // 估计确认数
    pub truncated: bool,                            // 分页未遍历完
}
```

### MatchedTransfer

```rust
pub struct MatchedTransfer {
    pub tx_hash: Vec<u8>,
    pub amount: u64,
    pub block_timestamp: u64,
    pub estimated_confirmations: Option<u32>,
}
```

### AmountStatus

五级金额匹配（容差可配置，默认 ±0.5%）：

```rust
pub enum AmountStatus {
    Unknown,
    Exact,
    Overpaid { excess: u64 },
    Underpaid { shortage: u64 },
    SeverelyUnderpaid { shortage: u64 },
    Invalid,
}
```

| 状态 | 条件（默认 50 bps） | 上层处理 |
|------|------|---------|
| Exact | `expected × 0.995 ≤ actual ≤ expected × 1.005` | 全额结算 |
| Overpaid | `actual > expected × 1.005` | 全额结算 |
| Underpaid | `expected × 0.5 ≤ actual < expected × 0.995` | 补付窗口 |
| SeverelyUnderpaid | `actual < expected × 0.5` | 按比例释放 + 没收保证金 |
| Invalid | `actual = 0` 或 `expected = 0` | 不释放 + 没收保证金 |

辅助方法：`is_acceptable()` → Exact 或 Overpaid；`to_verification_result_name()` → 与 `pallet-trading-common` 兼容的枚举名。

### VerificationError

```rust
pub enum VerificationError {
    HttpSendFailed,                    // HTTP 发送失败
    HttpTimeout,                       // HTTP 超时
    HttpBadStatus(u16),                // 非 200 状态码
    EmptyResponse,                     // 空响应体
    InvalidUtf8,                       // UTF-8 解码失败
    InvalidJson,                       // JSON 解析失败
    AllEndpointsFailed,                // 所有端点均失败
    NoEndpoints,                       // 无可用端点
    RateLimited,                       // 速率限制
    InvalidEndpointUrl(&'static str),  // URL 校验失败
    MaxEndpointsReached,               // 端点数超限
    InvalidTronAddress(&'static str),  // 地址格式无效
    TimestampTooOld,                   // 超出回溯窗口
    InvalidConfig(&'static str),       // 配置参数无效
    VerificationLocked,                // OCW 并发锁冲突
    VerifierDisabled,                  // 全局 kill switch 禁用
    TxHashAlreadyUsed,                 // tx_hash 已被使用
}
```

实现 `Display`、`as_str()`、`From<VerificationError> for &'static str`。

---

## 配置参数

### VerifierConfig

| 字段 | 类型 | 默认值 | 校验规则 |
|------|------|--------|----------|
| `usdt_contract` | `String` | `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` | 合法 TRON 地址 |
| `min_confirmations` | `u32` | 19 | ≥ 10 |
| `rate_limit_interval_ms` | `u64` | 200 | 0（禁用）或 ≥ 50 |
| `cache_ttl_ms` | `u64` | 30,000 | 0（禁用）或 ≥ 1,000 |
| `max_pages` | `u32` | 3 | 1–10 |
| `audit_log_retention` | `u32` | 100 | 0（禁用）或 ≤ 10,000 |
| `max_lookback_ms` | `u64` | 259,200,000 (72h) | 0（禁用）或 ≥ 3,600,000 (1h) |
| `amount_tolerance_bps` | `u32` | 50 (±0.5%) | 0–1,000 (最大 10%) |
| `enabled` | `bool` | `true` | — |
| `updated_at` | `u64` | 0 | 自动写入 |

存储键 `ocw_verifier_config`，版本化格式（`0xFF + version + SCALE`），支持从旧格式自动迁移。

### EndpointConfig

| 字段 | 类型 | 默认值 | 校验规则 |
|------|------|--------|----------|
| `endpoints` | `Vec<String>` | 3 个默认端点 | HTTPS、长度 10–256、无空白、非私有地址、≤ 10 个 |
| `parallel_mode` | `bool` | `true` | — |
| `api_keys` | `Vec<(String, String)>` | `[]` | 端点必须已注册 |
| `timeout_ms` | `u64` | 10,000 | 0 或 ≥ 1,000 |
| `timeout_race_ms` | `u64` | 5,000 | 0 或 ≥ 500 |
| `priority_boosts` | `Vec<(String, u32)>` | `[]` | 端点必须已注册 |
| `updated_at` | `u64` | 0 | 自动写入 |

存储键 `ocw_custom_endpoints`，版本化格式。

---

## 内部机制

### 端点健康评分

每次 HTTP 请求后更新端点的 `EndpointHealth`：

```text
score = success_rate_score(0–50) + speed_score(0–50) + priority_boost

success_rate_score = success_count / (success + failure) × 50
speed_score = <1000ms → 50,  >10000ms → 0,  中间线性插值

初始评分: 50
窗口化半衰: success + failure 达 100 时两者各除 2
响应时间: EMA 衰减（90% 旧值 + 10% 新值，u64 中间计算防溢出）
```

### 端点熔断

| 触发条件 | 行为 | 恢复 |
|---------|------|------|
| 请求后 score < 15 | 隔离 60 秒 | 超时自动恢复 |
| 成功请求 | 立即清除隔离 | — |

隔离期间 `get_sorted_endpoints()` 和并行竞速自动跳过。

### 请求模式

| 模式 | 选择条件 | 行为 |
|------|---------|------|
| 串行故障转移 | `parallel_mode = false` 或仅 1 端点 | 按评分顺序依次尝试 |
| 并行竞速 | `parallel_mode = true` 且 > 1 端点 | 同时请求所有端点，取最快成功响应 |

两种模式均：API Key 注入、健康评分更新、熔断触发/清除。

### 速率限制

全局请求间隔，间隔内返回 `RateLimited`。默认 200ms，设 0 禁用。

### 响应缓存

URL 级缓存。仅缓存以 `{` 或 `[` 开头的合法 JSON 响应（防缓存投毒）。容量限制 50 条，满时淘汰最旧。

### OCW 并发锁

以 `from:to` 为 lock_id，CAS 获取/释放。超时 30 秒自动过期。令牌机制防止误释放他人持有的锁。

### 审计日志

每次 `verify_trc20_by_transfer` 自动记录：

```rust
pub struct AuditLogEntry {
    pub timestamp: u64,          // 验证时间
    pub action: Vec<u8>,         // "verify_trc20_by_transfer"
    pub from_address: Vec<u8>,
    pub to_address: Vec<u8>,
    pub expected_amount: u64,
    pub actual_amount: u64,
    pub result_ok: bool,
    pub error_msg: Vec<u8>,
    pub tx_hash: Vec<u8>,        // 匹配到的 tx_hash
    pub endpoint_used: Vec<u8>,  // 实际响应的端点 URL
    pub duration_ms: u64,        // 验证耗时
}
```

### 监控指标

```rust
pub struct VerifierMetrics {
    pub total_success: u64,
    pub total_failure: u64,
    pub total_duration_ms: u64,
    pub endpoint_fallback_count: u64,
    pub cache_hit_count: u64,
    pub rate_limit_hit_count: u64,
    pub lock_contention_count: u64,
    pub last_updated: u64,
}
```

---

## 安全设计

### 交易安全

| 机制 | 说明 |
|------|------|
| tx_hash 重放防护 | 已使用的 tx_hash 自动过滤，上层显式注册 |
| 全局 kill switch | `enabled = false` → 所有验证立即返回 `VerifierDisabled` |
| 合约精确匹配 | 检查 `token_info.address` 字段，非全树搜索 |
| 最小确认数 | 默认 ≥ 19，基于时间戳估算（TRON ~3s/block） |
| Base58Check | 地址校验含 SHA256 双哈希校验和 |
| u128 中间计算 | 金额阈值运算防 u64 溢出，`.min(u64::MAX)` 防截断回绕 |
| expected=0 → Invalid | 与 `pallet-trading-common` 语义一致 |
| found 时 tx_hash 非空保证 | found=true 但 tx_hash 为空时降级为 found=false |

### 端点安全

| 机制 | 说明 |
|------|------|
| HTTPS 强制 | 拒绝非 HTTPS URL |
| SSRF 防护 | 拒绝 localhost、127.x、10.x、172.16–31.x、192.168.x、169.254.x、IPv6 回环/ULA/link-local、`::ffff:` 映射 |
| URL 校验 | 长度 10–256，无空白字符，`validate_endpoint_config` 统一入口 |
| API Key 精确匹配 | `==` 而非 `starts_with`，防止 Key 泄漏到同名前缀的恶意端点 |
| 端点数量限制 | 最多 10 个 |
| 端点熔断 | score < 15 自动隔离 60s |
| OCW 并发锁 | CAS + 令牌，30s 超时，防并发验证和误释放 |
| 配置参数校验 | `save_verifier_config` / `save_endpoint_config` 校验所有参数范围 |
| 缓存投毒防护 | 仅缓存以 `{`/`[` 开头的 JSON 响应 |

---

## 常量

| 常量 | 值 | 说明 |
|------|------|------|
| `USDT_CONTRACT` | `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` | 默认 USDT 合约（可覆盖） |
| `DEFAULT_ENDPOINTS` | 3 个 | TronGrid / TronStack / TronScan |
| `TRONGRID_MAINNET` | `https://api.trongrid.io` | URL 构建基础 |
| `HTTP_TIMEOUT_MS` | 10,000 | 串行超时（可覆盖） |
| `HTTP_TIMEOUT_RACE_MS` | 5,000 | 竞速超时（可覆盖） |
| `MIN_CONFIRMATIONS` | 19 | 最小确认数（可覆盖） |
| `MAX_ENDPOINTS` | 10 | 端点数上限 |
| `HEALTH_DECAY_FACTOR` | 90 | EMA 衰减因子 |
| `HEALTH_WINDOW_SIZE` | 100 | 半衰窗口 |
| `QUARANTINE_DURATION_MS` | 60,000 | 熔断隔离时间 |
| `OCW_LOCK_TIMEOUT_MS` | 30,000 | 并发锁超时 |
| `MAX_CACHE_ENTRIES` | 50 | 缓存容量 |
| `DEFAULT_MAX_LOOKBACK_MS` | 259,200,000 | 默认回溯窗口 (72h) |

---

## Offchain 存储键

| 存储键 | 用途 |
|--------|------|
| `ocw_verifier_config` | VerifierConfig（版本化） |
| `ocw_custom_endpoints` | EndpointConfig（版本化） |
| `ocw_endpoint_health::{endpoint}` | 端点健康评分 |
| `ocw_quarantine::{endpoint}` | 端点熔断到期时间 |
| `ocw_used_tx::{tx_hash}` | 已使用的 tx_hash |
| `ocw_verify_lock::{from:to}` | OCW 并发锁 |
| `ocw_rate_limit_last_req` | 上次请求时间戳 |
| `ocw_resp_cache::{url}` | URL 响应缓存 |
| `ocw_cache_keys_registry` | 缓存键注册表 |
| `ocw_verifier_metrics` | 监控指标 |
| `ocw_audit_log::{id}` | 审计日志条目 |
| `ocw_audit_log_counter` | 审计日志计数器 |

---

## 测试

```bash
cargo test -p pallet-trading-trc20-verifier
```

128 个测试，覆盖：

- **基础工具** — hex 转换、边界值
- **端点健康** — 评分公式、窗口半衰、EMA 溢出保护、移除清理
- **转账解析** — 精确匹配 / 多付 / 少付 / 严重少付 / from 不匹配 / 空数组 / 多笔累加 / 非 USDT 忽略 / JSON 特殊字符
- **配置校验** — 各参数的边界条件、USDT 合约地址校验、金额容差
- **OCW 并发锁** — 获取 / 释放 / 不同 ID 独立 / 令牌验证
- **监控指标** — 默认值 / 记录 / 重置
- **缓存** — 注册+清理 / 容量淘汰
- **tx_hash 重放** — 默认未使用 / 注册+检查 / 空 hash 安全 / 批量注册
- **Kill switch** — 默认启用 / 禁用错误 / 旧格式迁移
- **端点熔断** — 默认不隔离 / 隔离+检查 / 清除 / 排序跳过
- **安全回归** — SSRF、API Key 精确匹配、版本化迁移、合约精确匹配、分页去重

---

## 依赖

| crate | 用途 |
|-------|------|
| `codec` (SCALE) | 结构序列化/反序列化 |
| `sp-runtime` | OCW HTTP 客户端 |
| `sp-core` | offchain StorageKind、SHA256 哈希 |
| `sp-io` | 时间戳 / offchain local storage |
| `log` | OCW 日志 |
| `lite-json` | `no_std` JSON 结构化解析 |

---

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| **v0.4.0** | 2026-03-06 | 上线前审计加固: tx_hash 重放防护, 全局 kill switch, 端点熔断/隔离, endpoint_used 审计修复, 废弃代码清理 (~400行), URL 校验统一入口 |
| v0.3.4 | 2026-03 | 审计 R4: priority_boost 端点存在校验, audit_log 迭代限制, 分页 tx_hash 去重, token_info.address 精确匹配 |
| v0.3.3 | 2026-03 | 审计 R3: API Key 精确匹配, EMA u64 中间计算, set_api_key 端点验证 |
| v0.3.2 | 2026-03 | 审计 R2: audit_log_retention 校验, saturating_sub, est_conf 截断保护 |
| v0.3.1 | 2026-03 | 审计 R1: OCW 锁泄漏修复, 配置校验增强, 审计计数器 saturating_add |
| v0.3.0 | 2026-03-04 | 重大增强: VerificationError 枚举, 运行时配置, API Key, 速率限制, 缓存, 分页, TronVerifier trait, 审计日志, lite-json |
| v0.2.0 | 2026-02-23 | 新增 verify_trc20_by_transfer |
| v0.1.0 | 2026-02-08 | 从 pallet-trading-swap/src/ocw.rs 提取 |

---

**License**: Unlicense
