#![cfg_attr(not(feature = "std"), no_std)]

//! # TRC20 交易验证共享库
//!
//! ## 概述
//! 共享 TRC20 验证逻辑。
//! 可被 `pallet-trading-p2p`、`pallet-entity-market` 等模块复用。
//!
//! ## 功能
//! - TronGrid API 调用验证 TRC20 交易
//! - 端点健康评分与动态排序
//! - 并行请求竞速模式
//! - 金额匹配状态判定
//!
//! ## 版本历史
//! - v0.1.0 (2026-02-08): 提取为共享库

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::format;
use sp_runtime::offchain::{http, Duration};
use sp_core::offchain::StorageKind;
use codec::{Encode, Decode};
use lite_json::{JsonValue, parse_json};

// ==================== 常量配置 ====================

/// 默认 TRON API 端点列表（按优先级排序）
///
/// ⚠️ 注意：所有端点必须是主网端点，不能使用测试网！
pub const DEFAULT_ENDPOINTS: &[&str] = &[
    "https://api.trongrid.io",         // TronGrid 官方
    "https://api.tronstack.io",        // TronStack 第三方
    "https://apilist.tronscanapi.com", // TronScan
];

/// 主端点（用于 URL 构建）
pub const TRONGRID_MAINNET: &str = "https://api.trongrid.io";

/// 官方 USDT TRC20 合约地址 (Mainnet)
pub const USDT_CONTRACT: &str = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";

/// HTTP 请求超时（毫秒）- 串行模式
pub const HTTP_TIMEOUT_MS: u64 = 10_000;

/// HTTP 请求超时（毫秒）- 并行竞速模式（更短）
pub const HTTP_TIMEOUT_RACE_MS: u64 = 5_000;

/// 最小确认数
pub const MIN_CONFIRMATIONS: u32 = 19;

/// 端点健康评分存储键前缀
const ENDPOINT_HEALTH_PREFIX: &[u8] = b"ocw_endpoint_health::";

/// 自定义端点列表存储键
const CUSTOM_ENDPOINTS_KEY: &[u8] = b"ocw_custom_endpoints";

/// 健康评分衰减因子（每次请求后旧分数的权重）
const HEALTH_DECAY_FACTOR: u32 = 90; // 90%

// ==================== 端点健康评分系统 ====================

/// 端点健康状态
#[derive(Debug, Clone, Encode, Decode, Default)]
pub struct EndpointHealth {
    /// 成功次数
    pub success_count: u32,
    /// 失败次数
    pub failure_count: u32,
    /// 平均响应时间（毫秒）
    pub avg_response_ms: u32,
    /// 健康评分 (0-100)
    pub score: u32,
    /// 最后更新时间戳
    pub last_updated: u64,
}

impl EndpointHealth {
    /// 计算健康评分
    ///
    /// 评分公式: score = success_rate * 50 + response_speed * 50
    /// - success_rate: 成功率 (0-50分)
    /// - response_speed: 响应速度分 (0-50分，越快越高)
    pub fn calculate_score(&self) -> u32 {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            return 50; // 默认中等分数
        }

        // 成功率分数 (0-50)
        let success_rate = (self.success_count as u64 * 50 / total as u64) as u32;

        // 响应速度分数 (0-50)
        // 1000ms 以下满分，10000ms 以上 0 分
        let speed_score = if self.avg_response_ms < 1000 {
            50
        } else if self.avg_response_ms > 10000 {
            0
        } else {
            50 - ((self.avg_response_ms - 1000) * 50 / 9000)
        };

        success_rate + speed_score
    }

    /// 记录成功请求
    pub fn record_success(&mut self, response_ms: u32) {
        self.success_count = self.success_count.saturating_add(1);

        // 指数移动平均更新响应时间
        if self.avg_response_ms == 0 {
            self.avg_response_ms = response_ms;
        } else {
            self.avg_response_ms = (self.avg_response_ms * HEALTH_DECAY_FACTOR
                + response_ms * (100 - HEALTH_DECAY_FACTOR)) / 100;
        }

        self.score = self.calculate_score();
        self.last_updated = current_timestamp_ms();
    }

    /// 记录失败请求
    pub fn record_failure(&mut self) {
        self.failure_count = self.failure_count.saturating_add(1);
        self.score = self.calculate_score();
        self.last_updated = current_timestamp_ms();
    }
}

/// 获取当前时间戳（毫秒）
fn current_timestamp_ms() -> u64 {
    sp_io::offchain::timestamp().unix_millis()
}

/// 获取端点健康状态
pub fn get_endpoint_health(endpoint: &str) -> EndpointHealth {
    let key = [ENDPOINT_HEALTH_PREFIX, endpoint.as_bytes()].concat();

    sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, &key)
        .and_then(|data| EndpointHealth::decode(&mut &data[..]).ok())
        .unwrap_or_default()
}

/// 保存端点健康状态
fn save_endpoint_health(endpoint: &str, health: &EndpointHealth) {
    let key = [ENDPOINT_HEALTH_PREFIX, endpoint.as_bytes()].concat();
    sp_io::offchain::local_storage_set(
        StorageKind::PERSISTENT,
        &key,
        &health.encode(),
    );
}

// ==================== 配置化端点管理 ====================

/// 端点配置
#[derive(Debug, Clone, Encode, Decode)]
pub struct EndpointConfig {
    /// 端点 URL 列表
    pub endpoints: Vec<String>,
    /// 是否启用并行竞速模式
    pub parallel_mode: bool,
    /// 最后更新时间
    pub updated_at: u64,
}

impl Default for EndpointConfig {
    fn default() -> Self {
        Self {
            endpoints: DEFAULT_ENDPOINTS.iter().map(|s| String::from(*s)).collect(),
            parallel_mode: true,
            updated_at: 0,
        }
    }
}

/// 获取当前端点配置
pub fn get_endpoint_config() -> EndpointConfig {
    sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, CUSTOM_ENDPOINTS_KEY)
        .and_then(|data| EndpointConfig::decode(&mut &data[..]).ok())
        .unwrap_or_default()
}

/// 保存端点配置
pub fn save_endpoint_config(config: &EndpointConfig) {
    sp_io::offchain::local_storage_set(
        StorageKind::PERSISTENT,
        CUSTOM_ENDPOINTS_KEY,
        &config.encode(),
    );
}

/// 添加自定义端点
///
/// 🆕 H8修复: 添加 URL 格式校验（HTTPS、长度、无空白字符）
pub fn add_endpoint(endpoint: &str) -> Result<(), &'static str> {
    // H8: URL 格式校验
    if !endpoint.starts_with("https://") {
        return Err("Endpoint must use HTTPS");
    }
    if endpoint.len() < 10 || endpoint.len() > 256 {
        return Err("Endpoint URL length must be 10-256 characters");
    }
    if endpoint.bytes().any(|b| b == b' ' || b == b'\t' || b == b'\n' || b == b'\r') {
        return Err("Endpoint URL must not contain whitespace");
    }

    let mut config = get_endpoint_config();
    let endpoint_str = String::from(endpoint);

    if !config.endpoints.contains(&endpoint_str) {
        config.endpoints.push(endpoint_str);
        config.updated_at = current_timestamp_ms();
        save_endpoint_config(&config);
        log::info!(target: "trc20-verifier", "Added endpoint: {}", endpoint);
    }
    Ok(())
}

/// 移除端点
pub fn remove_endpoint(endpoint: &str) {
    let mut config = get_endpoint_config();
    let endpoint_str = String::from(endpoint);

    if let Some(pos) = config.endpoints.iter().position(|e| e == &endpoint_str) {
        config.endpoints.remove(pos);
        config.updated_at = current_timestamp_ms();
        save_endpoint_config(&config);
        log::info!(target: "trc20-verifier", "Removed endpoint: {}", endpoint);
    }
}

/// 获取按健康评分排序的端点列表
pub fn get_sorted_endpoints() -> Vec<String> {
    let config = get_endpoint_config();
    let mut endpoints_with_scores: Vec<(String, u32)> = config.endpoints
        .iter()
        .map(|e| {
            let health = get_endpoint_health(e);
            (e.clone(), health.score)
        })
        .collect();

    // 按评分降序排序
    endpoints_with_scores.sort_by(|a, b| b.1.cmp(&a.1));

    endpoints_with_scores.into_iter().map(|(e, _)| e).collect()
}

// ==================== TRC20 验证结果 ====================

/// TRC20 交易验证结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TronTxVerification {
    pub tx_hash: Vec<u8>,
    pub is_valid: bool,
    pub from_address: Option<Vec<u8>>,
    pub to_address: Option<Vec<u8>>,
    /// 实际转账金额（从链上读取）
    pub actual_amount: Option<u64>,
    /// 预期金额
    pub expected_amount: Option<u64>,
    pub confirmations: u32,
    pub error: Option<Vec<u8>>,
    /// 金额匹配状态
    pub amount_status: AmountStatus,
}

/// 金额匹配状态
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AmountStatus {
    /// 未知（尚未验证）
    #[default]
    Unknown,
    /// 完全匹配（误差 ±0.5% 以内）
    Exact,
    /// 多付（实际金额 > 预期金额 + 0.5%）
    Overpaid {
        /// 多付金额
        excess: u64,
    },
    /// 少付（实际金额 < 预期金额 - 0.5%）
    Underpaid {
        /// 少付金额
        shortage: u64,
    },
    /// 严重不足（实际金额 < 预期金额的 50%）
    SeverelyUnderpaid {
        shortage: u64,
    },
    /// 金额为零或无法解析
    Invalid,
}

impl Default for TronTxVerification {
    fn default() -> Self {
        Self {
            tx_hash: Vec::new(),
            is_valid: false,
            from_address: None,
            to_address: None,
            actual_amount: None,
            expected_amount: None,
            confirmations: 0,
            error: None,
            amount_status: AmountStatus::Unknown,
        }
    }
}

// ==================== 核心验证 API ====================

/// 验证 TRC20 交易
///
/// ## 参数
/// - `tx_hash`: 交易哈希（字节数组）
/// - `expected_to`: 预期收款地址
/// - `expected_amount`: 预期金额（USDT，精度 10^6）
///
/// ## 返回
/// - `Ok(TronTxVerification)`: 验证结果（含详细状态）
pub fn verify_trc20_transaction(
    tx_hash: &[u8],
    expected_to: &[u8],
    expected_amount: u64,
) -> Result<TronTxVerification, &'static str> {
    // 1. 构建 API URL
    let tx_hash_hex = bytes_to_hex(tx_hash);
    let url = format!("{}/v1/transactions/{}", TRONGRID_MAINNET, tx_hash_hex);

    // 2. 发送 HTTP 请求（带故障转移）
    let response = fetch_url_with_fallback(&url)?;

    // 3. 解析响应（tx_hash 路径暂不检查 from，Phase 2 JSON 重构后可从响应提取）
    parse_tron_response(&response, expected_to, None, expected_amount)
}

/// 简化验证接口：仅返回 bool
pub fn verify_trc20_transaction_simple(
    tx_hash: &[u8],
    expected_to: &[u8],
    expected_amount: u64,
) -> Result<bool, &'static str> {
    let result = verify_trc20_transaction(tx_hash, expected_to, expected_amount)?;
    Ok(result.is_valid)
}

// ==================== 并行请求竞速模式 ====================

/// 并行请求结果
#[allow(dead_code)]
struct RaceResult {
    endpoint: String,
    response: Vec<u8>,
    response_ms: u32,
}

/// 发送 HTTP GET 请求（智能模式选择）
fn fetch_url_with_fallback(url: &str) -> Result<Vec<u8>, &'static str> {
    let config = get_endpoint_config();

    if config.parallel_mode && config.endpoints.len() > 1 {
        fetch_url_parallel_race(url, &config.endpoints)
    } else {
        fetch_url_sequential(url)
    }
}

/// 并行竞速模式：同时请求所有端点，使用最快响应
fn fetch_url_parallel_race(url: &str, endpoints: &[String]) -> Result<Vec<u8>, &'static str> {
    log::info!(target: "trc20-verifier", "Starting parallel race with {} endpoints", endpoints.len());

    let start_time = current_timestamp_ms();

    // 准备所有请求
    let mut pending_requests: Vec<(String, http::PendingRequest)> = Vec::new();
    let timeout = sp_io::offchain::timestamp()
        .add(Duration::from_millis(HTTP_TIMEOUT_RACE_MS));

    for endpoint in endpoints.iter() {
        let target_url = url.replace(TRONGRID_MAINNET, endpoint);

        let request = http::Request::get(&target_url);
        match request.deadline(timeout).send() {
            Ok(pending) => {
                pending_requests.push((endpoint.clone(), pending));
                log::debug!(target: "trc20-verifier", "Sent request to {}", endpoint);
            },
            Err(_) => {
                log::warn!(target: "trc20-verifier", "Failed to send request to {}", endpoint);
                let mut health = get_endpoint_health(endpoint);
                health.record_failure();
                save_endpoint_health(endpoint, &health);
            }
        }
    }

    if pending_requests.is_empty() {
        return Err("Failed to send any requests");
    }

    // 轮询等待第一个成功响应
    let mut winner: Option<RaceResult> = None;
    let mut failed_endpoints: Vec<String> = Vec::new();

    for (endpoint, pending) in pending_requests {
        match pending.try_wait(timeout) {
            Ok(Ok(response)) => {
                let response_ms = (current_timestamp_ms() - start_time) as u32;

                if response.code == 200 {
                    let body = response.body().collect::<Vec<u8>>();
                    if !body.is_empty() {
                        log::info!(target: "trc20-verifier", "Winner: {} ({}ms)", endpoint, response_ms);

                        let mut health = get_endpoint_health(&endpoint);
                        health.record_success(response_ms);
                        save_endpoint_health(&endpoint, &health);

                        winner = Some(RaceResult {
                            endpoint,
                            response: body,
                            response_ms,
                        });
                        break;
                    }
                }

                failed_endpoints.push(endpoint);
            },
            Ok(Err(_)) | Err(_) => {
                failed_endpoints.push(endpoint);
            }
        }
    }

    // 记录失败的端点
    for endpoint in failed_endpoints {
        let mut health = get_endpoint_health(&endpoint);
        health.record_failure();
        save_endpoint_health(&endpoint, &health);
    }

    match winner {
        Some(result) => Ok(result.response),
        None => {
            log::error!(target: "trc20-verifier", "All parallel requests failed");
            Err("All parallel requests failed")
        }
    }
}

/// 串行故障转移模式：按健康评分依次尝试端点
fn fetch_url_sequential(url: &str) -> Result<Vec<u8>, &'static str> {
    let sorted_endpoints = get_sorted_endpoints();
    let mut last_error = "No endpoints available";

    log::info!(target: "trc20-verifier", "Sequential mode with {} endpoints (sorted by health)",
        sorted_endpoints.len());

    for (idx, endpoint) in sorted_endpoints.iter().enumerate() {
        let target_url = url.replace(TRONGRID_MAINNET, endpoint);
        let start_time = current_timestamp_ms();

        log::debug!(target: "trc20-verifier", "Trying endpoint {} ({}/{})",
            endpoint, idx + 1, sorted_endpoints.len());

        match fetch_url(&target_url) {
            Ok(response) => {
                let response_ms = (current_timestamp_ms() - start_time) as u32;

                let mut health = get_endpoint_health(endpoint);
                health.record_success(response_ms);
                save_endpoint_health(endpoint, &health);

                if idx > 0 {
                    log::info!(target: "trc20-verifier", "Fallback endpoint {} succeeded ({}ms)",
                        endpoint, response_ms);
                }
                return Ok(response);
            },
            Err(e) => {
                let mut health = get_endpoint_health(endpoint);
                health.record_failure();
                save_endpoint_health(endpoint, &health);

                log::warn!(target: "trc20-verifier", "Endpoint {} failed: {}", endpoint, e);
                last_error = e;
            }
        }
    }

    log::error!(target: "trc20-verifier", "All {} endpoints failed", sorted_endpoints.len());
    Err(last_error)
}

/// 发送 HTTP GET 请求
fn fetch_url(url: &str) -> Result<Vec<u8>, &'static str> {
    log::debug!(target: "trc20-verifier", "Fetching URL: {}", url);

    let request = http::Request::get(url);

    let timeout = sp_io::offchain::timestamp()
        .add(Duration::from_millis(HTTP_TIMEOUT_MS));

    let pending = request
        .deadline(timeout)
        .send()
        .map_err(|_| "Failed to send HTTP request")?;

    let response = pending
        .try_wait(timeout)
        .map_err(|_| "HTTP request timeout")?
        .map_err(|_| "HTTP request failed")?;

    if response.code != 200 {
        log::warn!(target: "trc20-verifier", "HTTP response code: {}", response.code);
        return Err("Non-200 HTTP response");
    }

    let body = response.body().collect::<Vec<u8>>();

    if body.is_empty() {
        return Err("Empty response body");
    }

    log::debug!(target: "trc20-verifier", "Received {} bytes", body.len());
    Ok(body)
}

// ==================== JSON 结构化解析辅助 (C1+C2+M5 修复) ====================

/// 比较 lite-json 的 char slice key 与 &str
fn json_key_eq(key: &[char], name: &str) -> bool {
    key.len() == name.len() && key.iter().zip(name.chars()).all(|(a, b)| *a == b)
}

/// 将 lite-json 的 char slice 转为 String
fn json_chars_to_string(chars: &[char]) -> String {
    chars.iter().collect()
}

/// 从 JSON 对象字段列表中按 key 获取值引用
fn json_obj_get<'a>(obj: &'a [(Vec<char>, JsonValue)], key: &str) -> Option<&'a JsonValue> {
    obj.iter().find(|(k, _)| json_key_eq(k, key)).map(|(_, v)| v)
}

/// 获取 JSON 对象中的字符串字段
fn json_obj_get_str(obj: &[(Vec<char>, JsonValue)], key: &str) -> Option<String> {
    match json_obj_get(obj, key)? {
        JsonValue::String(chars) => Some(json_chars_to_string(chars)),
        _ => None,
    }
}

/// 获取 JSON 对象中的 u64 数字字段（兼容字符串格式数字如 "50000000"）
fn json_obj_get_u64(obj: &[(Vec<char>, JsonValue)], key: &str) -> Option<u64> {
    match json_obj_get(obj, key)? {
        JsonValue::Number(n) => {
            if n.negative { return None; }
            Some(n.integer)
        },
        JsonValue::String(chars) => {
            let s: String = chars.iter().collect();
            s.parse::<u64>().ok()
        },
        _ => None,
    }
}

/// 获取 JSON 对象中的嵌套对象
#[allow(dead_code)]
fn json_obj_get_object<'a>(obj: &'a [(Vec<char>, JsonValue)], key: &str) -> Option<&'a [(Vec<char>, JsonValue)]> {
    match json_obj_get(obj, key)? {
        JsonValue::Object(inner) => Some(inner.as_slice()),
        _ => None,
    }
}

/// 在 JSON 树中递归搜索指定 key 的字符串值
fn json_find_str(value: &JsonValue, key: &str) -> Option<String> {
    match value {
        JsonValue::Object(obj) => {
            if let Some(s) = json_obj_get_str(obj, key) {
                return Some(s);
            }
            for (_, v) in obj.iter() {
                if let Some(s) = json_find_str(v, key) {
                    return Some(s);
                }
            }
            None
        },
        JsonValue::Array(arr) => {
            for v in arr {
                if let Some(s) = json_find_str(v, key) {
                    return Some(s);
                }
            }
            None
        },
        _ => None,
    }
}

/// 在 JSON 树中递归搜索指定 key 的 u64 数字值
fn json_find_u64(value: &JsonValue, key: &str) -> Option<u64> {
    match value {
        JsonValue::Object(obj) => {
            if let Some(n) = json_obj_get_u64(obj, key) {
                return Some(n);
            }
            for (_, v) in obj.iter() {
                if let Some(n) = json_find_u64(v, key) {
                    return Some(n);
                }
            }
            None
        },
        JsonValue::Array(arr) => {
            for v in arr {
                if let Some(n) = json_find_u64(v, key) {
                    return Some(n);
                }
            }
            None
        },
        _ => None,
    }
}

/// 检查 JSON 树中是否有任何字符串值完全等于 target
fn json_has_str_value(value: &JsonValue, target: &str) -> bool {
    match value {
        JsonValue::String(chars) => json_chars_to_string(chars) == target,
        JsonValue::Object(obj) => obj.iter().any(|(_, v)| json_has_str_value(v, target)),
        JsonValue::Array(arr) => arr.iter().any(|v| json_has_str_value(v, target)),
        _ => false,
    }
}

// ==================== 响应解析 ====================

/// 解析 TronGrid API 响应
///
/// ## 参数
/// - `expected_from`: 可选，预期发送方地址（Base58）。提供时检查发送方匹配。
///
/// 🆕 C1+C2 修复: 使用 lite-json 结构化解析替代 contains() 子串匹配。
/// - contractRet 按字段精确匹配（不再被响应中其他文本干扰）
/// - USDT 合约按完整字符串值匹配（不再匹配子串）
/// - 金额按字段名提取（不再匹配第一个 "amount:" 出现位置）
fn parse_tron_response(
    response: &[u8],
    expected_to: &[u8],
    expected_from: Option<&[u8]>,
    expected_amount: u64,
) -> Result<TronTxVerification, &'static str> {
    let response_str = core::str::from_utf8(response)
        .map_err(|_| "Invalid UTF-8 response")?;

    let mut result = TronTxVerification::default();
    result.expected_amount = Some(expected_amount);

    // C1修复: 使用 lite-json 结构化解析
    let json_value = parse_json(response_str)
        .map_err(|_| "Invalid JSON response")?;

    // 1. 检查交易成功状态 — 按字段名精确查找 contractRet
    let contract_ret = json_find_str(&json_value, "contractRet");
    if contract_ret.as_deref() != Some("SUCCESS") {
        result.error = Some(b"Transaction not successful".to_vec());
        return Ok(result);
    }

    // 2. 验证 USDT 合约地址 — 检查 JSON 树中是否有完整字符串值等于 USDT 合约
    if !json_has_str_value(&json_value, USDT_CONTRACT) {
        result.error = Some(b"Not a USDT TRC20 transaction".to_vec());
        return Ok(result);
    }

    // 3. 检查确认数 — 按字段名精确提取
    let confirmations = json_find_u64(&json_value, "confirmations")
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0);
    result.confirmations = confirmations;
    if confirmations < MIN_CONFIRMATIONS {
        result.error = Some(b"Insufficient confirmations".to_vec());
        return Ok(result);
    }

    // 4. H6修复: 检查发送方地址
    if let Some(expected_from_bytes) = expected_from {
        let expected_from_str = core::str::from_utf8(expected_from_bytes)
            .map_err(|_| "Invalid expected_from UTF-8")?;
        // 搜索 owner_address（TronGrid v1 标准字段）或 from
        let from_addr = json_find_str(&json_value, "owner_address")
            .or_else(|| json_find_str(&json_value, "from"));
        if from_addr.as_deref() != Some(expected_from_str) {
            // 回退: 检查是否有任何字符串值匹配（兼容不同 API 格式）
            if !json_has_str_value(&json_value, expected_from_str) {
                result.error = Some(b"Sender address mismatch".to_vec());
                return Ok(result);
            }
        }
    }

    // 5. 检查收款地址 — 按字段名精确查找
    let expected_to_str = core::str::from_utf8(expected_to)
        .map_err(|_| "Invalid expected_to UTF-8")?;
    let to_addr = json_find_str(&json_value, "to_address")
        .or_else(|| json_find_str(&json_value, "to"));
    if to_addr.as_deref() != Some(expected_to_str) {
        // 回退: 检查是否有任何字符串值匹配（兼容不同 API 格式）
        if !json_has_str_value(&json_value, expected_to_str) {
            result.error = Some(b"Recipient address mismatch".to_vec());
            return Ok(result);
        }
    }

    // 6. C2修复: 按字段名精确提取金额（不再匹配首个 "amount:" 子串位置）
    let actual_amount = json_find_u64(&json_value, "amount");
    result.actual_amount = actual_amount;

    // 🆕 H7修复: 统一使用 calculate_amount_status，消除重复逻辑
    // 修复前内联版本缺少 expected==0 → Invalid 保护
    let (amount_status, is_acceptable) = match actual_amount {
        Some(actual) => {
            let status = calculate_amount_status(expected_amount, actual);
            let acceptable = matches!(status, AmountStatus::Exact | AmountStatus::Overpaid { .. });
            match &status {
                AmountStatus::Overpaid { excess } =>
                    log::info!(target: "trc20-verifier", "Overpaid: expected={}, actual={}, excess={}",
                        expected_amount, actual, excess),
                AmountStatus::Underpaid { shortage } =>
                    log::warn!(target: "trc20-verifier", "Underpaid: expected={}, actual={}, shortage={}",
                        expected_amount, actual, shortage),
                AmountStatus::SeverelyUnderpaid { .. } =>
                    log::error!(target: "trc20-verifier", "Severely underpaid: expected={}, actual={}",
                        expected_amount, actual),
                _ => {}
            }
            (status, acceptable)
        },
        None => {
            log::error!(target: "trc20-verifier", "Failed to extract amount from response");
            (AmountStatus::Invalid, false)
        }
    };

    result.amount_status = amount_status.clone();

    if !is_acceptable {
        let error_msg = match &amount_status {
            AmountStatus::Underpaid { shortage } =>
                format!("Underpaid by {} (expected {}, got {})",
                    shortage, expected_amount, actual_amount.unwrap_or(0)),
            AmountStatus::SeverelyUnderpaid { shortage } =>
                format!("Severely underpaid by {} (possible fraud)", shortage),
            AmountStatus::Invalid =>
                "Invalid or zero amount".to_string(),
            _ => "Amount mismatch".to_string(),
        };
        result.error = Some(error_msg.into_bytes());
        return Ok(result);
    }

    result.is_valid = true;

    Ok(result)
}

/// 🆕 L1: 从响应中提取确认数（C1修复后仅测试使用）
#[cfg(test)]
fn extract_confirmations(response: &str) -> Option<u32> {
    let patterns = ["\"confirmations\":", "\"confirmations\": "];

    for pattern in patterns {
        if let Some(start) = response.find(pattern) {
            let after_key = &response[start + pattern.len()..];
            let trimmed = after_key.trim_start();
            let num_str: String = trimmed.chars()
                .take_while(|c| c.is_numeric())
                .collect();
            if !num_str.is_empty() {
                if let Ok(count) = num_str.parse::<u32>() {
                    return Some(count);
                }
            }
        }
    }
    None
}

/// 从响应中提取金额（C2修复后仅测试使用）
#[cfg(test)]
fn extract_amount(response: &str) -> Option<u64> {
    let patterns = ["\"amount\":", "\"amount\": "];

    for pattern in patterns {
        if let Some(start) = response.find(pattern) {
            let after_key = &response[start + pattern.len()..];
            let trimmed = after_key.trim_start();
            let num_str: String = trimmed.chars()
                .take_while(|c| c.is_numeric())
                .collect();
            if !num_str.is_empty() {
                if let Ok(amount) = num_str.parse::<u64>() {
                    return Some(amount);
                }
            }
        }
    }
    None
}

// ==================== 按 (from, to, amount) 搜索验证 ====================

/// TRC20 转账搜索结果
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransferSearchResult {
    /// 是否找到匹配的转账
    pub found: bool,
    /// 匹配转账的实际金额（USDT 精度 10^6）
    pub actual_amount: Option<u64>,
    /// 匹配转账的交易哈希
    pub tx_hash: Option<Vec<u8>>,
    /// 匹配转账的区块时间戳（毫秒）
    pub block_timestamp: Option<u64>,
    /// 金额匹配状态
    pub amount_status: AmountStatus,
    /// 错误信息
    pub error: Option<Vec<u8>>,
}

/// 按 (from, to, amount) 搜索并验证 TRC20 USDT 转账
///
/// ## 参数
/// - `from_address`: 付款方 TRON 地址（Base58，如 "T1234..."）
/// - `to_address`: 收款方 TRON 地址（Base58，如 "T5678..."）
/// - `expected_amount`: 预期 USDT 金额（精度 10^6）
/// - `min_timestamp`: 最早区块时间戳（毫秒），仅搜索此时间之后的转账
///
/// ## 返回
/// - `Ok(TransferSearchResult)`: 搜索结果（含匹配金额和状态）
/// - `Err`: HTTP 请求失败
///
/// ## 查询逻辑
/// 调用 TronGrid API 获取收款方的 TRC20 转入记录，
/// 在结果中查找 from 匹配且合约为 USDT 的转账。
pub fn verify_trc20_by_transfer(
    from_address: &[u8],
    to_address: &[u8],
    expected_amount: u64,
    min_timestamp: u64,
) -> Result<TransferSearchResult, &'static str> {
    let from_str = core::str::from_utf8(from_address).map_err(|_| "Invalid from_address UTF-8")?;
    let to_str = core::str::from_utf8(to_address).map_err(|_| "Invalid to_address UTF-8")?;

    // 构建 TronGrid TRC20 转账查询 URL
    let url = format!(
        "{}/v1/accounts/{}/transactions/trc20?contract_address={}&only_to=true&min_timestamp={}&limit=50&order_by=block_timestamp,desc",
        TRONGRID_MAINNET, to_str, USDT_CONTRACT, min_timestamp
    );

    log::info!(target: "trc20-verifier",
        "Searching TRC20 transfers: to={}, from={}, amount={}, since={}",
        to_str, from_str, expected_amount, min_timestamp);

    // 发送 HTTP 请求（带故障转移）
    let response = fetch_url_with_fallback(&url)?;

    // 🆕 H5修复: 获取当前时间传给解析函数，用于过滤未确认转账
    let now_ms = sp_io::offchain::timestamp().unix_millis();

    // 解析转账列表
    parse_trc20_transfer_list(&response, from_str, expected_amount, now_ms)
}

/// 解析 TronGrid TRC20 转账列表响应，搜索匹配的转账
///
/// TronGrid `/v1/accounts/{addr}/transactions/trc20` 响应格式:
/// ```json
/// {
///   "data": [
///     {
///       "transaction_id": "abc123...",
///       "from": "TBuyerAddr...",
///       "to": "TSellerAddr...",
///       "value": "50000000",
///       "block_timestamp": 1700000000000,
///       "type": "Transfer",
///       "token_info": { "address": "TR7NHq..." }
///     }
///   ],
///   "success": true
/// }
/// ```
/// ## 参数
/// - `now_ms`: 当前时间（毫秒），用于 H5 确认数近似检查。传 0 跳过检查。
///
/// 🆕 C1+M5 修复: 使用 lite-json 结构化解析替代 find_matching_brace + contains()。
/// - from 地址按字段精确匹配（不再格式化 pattern 做子串搜索）
/// - USDT 合约按完整字符串值匹配（不再子串匹配）
/// - M5: 不再使用 find_matching_brace（不能正确处理 JSON 字符串中的 `{}`）
pub fn parse_trc20_transfer_list(
    response: &[u8],
    expected_from: &str,
    expected_amount: u64,
    now_ms: u64,
) -> Result<TransferSearchResult, &'static str> {
    let response_str = core::str::from_utf8(response)
        .map_err(|_| "Invalid UTF-8 response")?;

    let mut result = TransferSearchResult::default();

    // C1修复: 使用 lite-json 结构化解析
    let json_value = parse_json(response_str)
        .map_err(|_| "Invalid JSON response")?;

    let root_obj = match json_value.as_object() {
        Some(obj) => obj,
        None => {
            result.error = Some(b"Response is not a JSON object".to_vec());
            return Ok(result);
        }
    };

    // 检查 API 成功标志 — 按字段精确匹配
    match json_obj_get(root_obj, "success") {
        Some(JsonValue::Boolean(true)) => {},
        _ => {
            result.error = Some(b"API returned failure".to_vec());
            return Ok(result);
        }
    }

    // 获取 data 数组
    let data_array = match json_obj_get(root_obj, "data") {
        Some(JsonValue::Array(arr)) => arr,
        _ => {
            result.error = Some(b"No data array in response".to_vec());
            return Ok(result);
        }
    };

    // 累计匹配金额
    let mut total_matched_amount: u64 = 0;
    let mut max_single_amount: u64 = 0;
    let mut best_tx_hash: Option<Vec<u8>> = None;
    let mut best_timestamp: Option<u64> = None;

    // 遍历 data 数组中的每个转账条目
    for entry_value in data_array.iter() {
        let entry_obj = match entry_value.as_object() {
            Some(obj) => obj,
            None => continue,
        };

        // 检查 from 地址是否匹配 — 按字段精确比较（C1修复）
        let from_addr = json_obj_get_str(entry_obj, "from");
        if from_addr.as_deref() != Some(expected_from) {
            continue;
        }

        // 检查是 USDT 合约 — 在整个 entry 中搜索完整字符串值（C1修复）
        if !json_has_str_value(entry_value, USDT_CONTRACT) {
            continue;
        }

        // H5修复: 检查确认数（用 block_timestamp 近似）
        // now_ms=0 时跳过检查（用于单元测试）
        if now_ms > 0 {
            if let Some(ts) = json_obj_get_u64(entry_obj, "block_timestamp") {
                let min_age_ms = (MIN_CONFIRMATIONS as u64).saturating_mul(3000).saturating_mul(2);
                if ts > 0 && now_ms.saturating_sub(ts) < min_age_ms {
                    log::warn!(target: "trc20-verifier",
                        "Skipping too-recent transfer (ts={}, age={}ms < {}ms)",
                        ts, now_ms.saturating_sub(ts), min_age_ms);
                    continue;
                }
            }
        }

        // 提取 value（TronGrid 返回字符串格式: "value":"50000000"）
        if let Some(amount) = json_obj_get_u64(entry_obj, "value") {
            if amount > 0 {
                total_matched_amount = total_matched_amount.saturating_add(amount);

                // L1修复: 记录最大单笔的交易信息
                if best_tx_hash.is_none() || amount > max_single_amount {
                    max_single_amount = amount;
                    best_tx_hash = json_obj_get_str(entry_obj, "transaction_id")
                        .map(|s| s.into_bytes());
                    best_timestamp = json_obj_get_u64(entry_obj, "block_timestamp");
                }

                log::info!(target: "trc20-verifier",
                    "Found matching transfer: value={}, running_total={}", amount, total_matched_amount);
            }
        }
    }

    if total_matched_amount == 0 {
        result.error = Some(b"No matching transfer found".to_vec());
        result.amount_status = AmountStatus::Invalid;
        return Ok(result);
    }

    result.found = true;
    result.actual_amount = Some(total_matched_amount);
    result.tx_hash = best_tx_hash;
    result.block_timestamp = best_timestamp;

    // 计算金额匹配状态
    result.amount_status = calculate_amount_status(expected_amount, total_matched_amount);

    Ok(result)
}

/// 找到匹配的 `}` 括号（支持一层嵌套）（M5修复后仅测试使用）
#[cfg(test)]
fn find_matching_brace(s: &str, open_pos: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut depth: u32 = 0;
    for i in open_pos..bytes.len() {
        match bytes[i] {
            b'{' => depth = depth.saturating_add(1),
            b'}' => {
                depth = match depth.checked_sub(1) {
                    Some(d) => d,
                    None => return None, // L3: 防止恶意响应导致下溢
                };
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// 从 JSON 片段中提取字符串字段值（C1修复后仅测试使用）
/// 匹配 `"key":"value"` 或 `"key": "value"` 格式
#[cfg(test)]
fn extract_json_string_value<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let patterns = [
        format!("\"{}\":\"", key),
        format!("\"{}\": \"", key),
    ];

    for pattern in &patterns {
        if let Some(start) = json.find(pattern.as_str()) {
            let value_start = start + pattern.len();
            if let Some(end) = json[value_start..].find('"') {
                return Some(&json[value_start..value_start + end]);
            }
        }
    }
    None
}

/// 从 JSON 片段中提取数字字段值（C1修复后仅测试使用）
/// 匹配 `"key":12345` 或 `"key": 12345` 格式
#[cfg(test)]
fn extract_json_number(json: &str, key: &str) -> Option<u64> {
    let patterns = [
        format!("\"{}\":", key),
        format!("\"{}\": ", key),
    ];

    for pattern in &patterns {
        if let Some(start) = json.find(pattern.as_str()) {
            let after_key = &json[start + pattern.len()..];
            let trimmed = after_key.trim_start();
            // 跳过引号（如果是字符串数字）
            let trimmed = trimmed.trim_start_matches('"');
            let num_str: String = trimmed.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if !num_str.is_empty() {
                return num_str.parse::<u64>().ok();
            }
        }
    }
    None
}

/// 计算金额匹配状态
pub fn calculate_amount_status(expected: u64, actual: u64) -> AmountStatus {
    if actual == 0 {
        return AmountStatus::Invalid;
    }
    // L2修复: expected==0 应返回 Invalid（与 pallet-trading-common 统一语义）
    if expected == 0 {
        return AmountStatus::Invalid;
    }

    // M2修复: 使用 u128 中间计算防止大金额乘法溢出
    // 🆕 M8修复: 用 min(u64::MAX) 替代裸 as u64 截断，防止极端值回绕
    let min_exact = (expected as u128 * 995 / 1000).min(u64::MAX as u128) as u64;  // -0.5%
    let max_exact = (expected as u128 * 1005 / 1000).min(u64::MAX as u128) as u64; // +0.5%
    let severe_threshold = expected / 2;    // 50%

    if actual >= min_exact && actual <= max_exact {
        AmountStatus::Exact
    } else if actual > max_exact {
        AmountStatus::Overpaid { excess: actual.saturating_sub(expected) }
    } else if actual >= severe_threshold {
        AmountStatus::Underpaid { shortage: expected.saturating_sub(actual) }
    } else if actual > 0 {
        AmountStatus::SeverelyUnderpaid { shortage: expected.saturating_sub(actual) }
    } else {
        AmountStatus::Invalid
    }
}

// ==================== 工具函数 ====================

/// 字节数组转十六进制字符串
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// 十六进制字符串转字节数组
///
/// 🆕 L6修复: 自动去除 0x/0X 前缀
pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, &'static str> {
    let hex = if hex.starts_with("0x") || hex.starts_with("0X") {
        &hex[2..]
    } else {
        hex
    };

    if hex.len() % 2 != 0 {
        return Err("Invalid hex length");
    }

    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| "Invalid hex"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_to_hex() {
        let bytes = [0x12, 0x34, 0xab, 0xcd];
        assert_eq!(bytes_to_hex(&bytes), "1234abcd");
    }

    #[test]
    fn test_hex_to_bytes() {
        let hex = "1234abcd";
        let bytes = hex_to_bytes(hex).unwrap();
        assert_eq!(bytes, vec![0x12, 0x34, 0xab, 0xcd]);
    }

    #[test]
    fn test_hex_to_bytes_invalid() {
        assert!(hex_to_bytes("123").is_err()); // odd length
        assert!(hex_to_bytes("zzzz").is_err()); // invalid hex chars
    }

    #[test]
    fn test_endpoint_health_score() {
        let mut health = EndpointHealth::default();
        assert_eq!(health.calculate_score(), 50); // default

        health.success_count = 9;
        health.failure_count = 1;
        health.avg_response_ms = 500;
        // success_rate = 9/10 * 50 = 45, speed = 50 (< 1000ms)
        assert_eq!(health.calculate_score(), 95);

        health.avg_response_ms = 5500;
        // success_rate = 45, speed = 50 - (4500 * 50 / 9000) = 50 - 25 = 25
        assert_eq!(health.calculate_score(), 70);
    }

    #[test]
    fn test_extract_amount() {
        assert_eq!(extract_amount(r#""amount":1000000"#), Some(1000000));
        assert_eq!(extract_amount(r#""amount": 2500000"#), Some(2500000));
        assert_eq!(extract_amount(r#"no amount here"#), None);
        assert_eq!(extract_amount(r#""amount":0"#), Some(0));
    }

    // ==================== 转账搜索解析测试 ====================

    #[test]
    fn test_find_matching_brace() {
        assert_eq!(find_matching_brace("{abc}", 0), Some(4));
        assert_eq!(find_matching_brace("{a{b}c}", 0), Some(6));
        assert_eq!(find_matching_brace("[{a},{b}]", 1), Some(3));
        assert_eq!(find_matching_brace("{", 0), None);
    }

    #[test]
    fn test_extract_json_string_value() {
        let json = r#"{"from":"TBuyerAddr","to":"TSellerAddr","value":"50000000"}"#;
        assert_eq!(extract_json_string_value(json, "from"), Some("TBuyerAddr"));
        assert_eq!(extract_json_string_value(json, "to"), Some("TSellerAddr"));
        assert_eq!(extract_json_string_value(json, "value"), Some("50000000"));
        assert_eq!(extract_json_string_value(json, "missing"), None);
    }

    #[test]
    fn test_extract_json_string_value_with_spaces() {
        let json = r#"{"from": "TBuyerAddr", "value": "50000000"}"#;
        assert_eq!(extract_json_string_value(json, "from"), Some("TBuyerAddr"));
        assert_eq!(extract_json_string_value(json, "value"), Some("50000000"));
    }

    #[test]
    fn test_extract_json_number() {
        let json = r#"{"block_timestamp":1700000000000,"confirmations":20}"#;
        assert_eq!(extract_json_number(json, "block_timestamp"), Some(1700000000000));
        assert_eq!(extract_json_number(json, "confirmations"), Some(20));
        assert_eq!(extract_json_number(json, "missing"), None);
    }

    #[test]
    fn test_extract_json_number_string_format() {
        // TronGrid 有时把数字放在引号里
        let json = r#"{"block_timestamp":"1700000000000"}"#;
        assert_eq!(extract_json_number(json, "block_timestamp"), Some(1700000000000));
    }

    #[test]
    fn test_parse_transfer_list_exact_match() {
        let response = br#"{"data":[{"transaction_id":"abc123","from":"TBuyerXXX","to":"TSellerYYY","value":"10000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
        let result = parse_trc20_transfer_list(response, "TBuyerXXX", 10_000_000, 0).unwrap();
        assert!(result.found);
        assert_eq!(result.actual_amount, Some(10_000_000));
        assert_eq!(result.amount_status, AmountStatus::Exact);
        assert_eq!(result.tx_hash, Some(b"abc123".to_vec()));
    }

    #[test]
    fn test_parse_transfer_list_overpaid() {
        let response = br#"{"data":[{"transaction_id":"tx1","from":"TBuyer","to":"TSeller","value":"15000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
        let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
        assert!(result.found);
        assert_eq!(result.actual_amount, Some(15_000_000));
        assert_eq!(result.amount_status, AmountStatus::Overpaid { excess: 5_000_000 });
    }

    #[test]
    fn test_parse_transfer_list_underpaid() {
        let response = br#"{"data":[{"transaction_id":"tx1","from":"TBuyer","to":"TSeller","value":"7000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
        let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
        assert!(result.found);
        assert_eq!(result.actual_amount, Some(7_000_000));
        assert_eq!(result.amount_status, AmountStatus::Underpaid { shortage: 3_000_000 });
    }

    #[test]
    fn test_parse_transfer_list_severely_underpaid() {
        let response = br#"{"data":[{"transaction_id":"tx1","from":"TBuyer","to":"TSeller","value":"3000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
        let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
        assert!(result.found);
        assert_eq!(result.actual_amount, Some(3_000_000));
        assert_eq!(result.amount_status, AmountStatus::SeverelyUnderpaid { shortage: 7_000_000 });
    }

    #[test]
    fn test_parse_transfer_list_no_match() {
        let response = br#"{"data":[{"transaction_id":"tx1","from":"TWrongBuyer","to":"TSeller","value":"10000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
        let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
        assert!(!result.found);
        assert_eq!(result.amount_status, AmountStatus::Invalid);
    }

    #[test]
    fn test_parse_transfer_list_empty_data() {
        let response = br#"{"data":[],"success":true}"#;
        let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
        assert!(!result.found);
    }

    #[test]
    fn test_parse_transfer_list_api_failure() {
        let response = br#"{"success":false,"error":"rate limit"}"#;
        let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
        assert!(!result.found);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_parse_transfer_list_multi_entry_accumulates() {
        // 同一 from 有两笔转账，金额应累加
        let response = br#"{"data":[{"transaction_id":"tx1","from":"TBuyer","to":"TSeller","value":"5000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}},{"transaction_id":"tx2","from":"TBuyer","to":"TSeller","value":"6000000","block_timestamp":1700000001000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
        let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
        assert!(result.found);
        assert_eq!(result.actual_amount, Some(11_000_000)); // 5M + 6M = 11M
        assert_eq!(result.amount_status, AmountStatus::Overpaid { excess: 1_000_000 });
    }

    #[test]
    fn test_parse_transfer_list_wrong_contract_ignored() {
        // 非 USDT 合约的转账应被忽略
        let response = br#"{"data":[{"transaction_id":"tx1","from":"TBuyer","to":"TSeller","value":"10000000","block_timestamp":1700000000000,"token_info":{"address":"TFakeContractAddress"}}],"success":true}"#;
        let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
        assert!(!result.found);
    }

    #[test]
    fn test_calculate_amount_status() {
        assert_eq!(calculate_amount_status(1_000_000, 1_000_000), AmountStatus::Exact);
        assert_eq!(calculate_amount_status(1_000_000, 1_004_000), AmountStatus::Exact); // within 0.5%
        assert_eq!(calculate_amount_status(1_000_000, 1_010_000), AmountStatus::Overpaid { excess: 10_000 });
        assert_eq!(calculate_amount_status(1_000_000, 800_000), AmountStatus::Underpaid { shortage: 200_000 });
        assert_eq!(calculate_amount_status(1_000_000, 400_000), AmountStatus::SeverelyUnderpaid { shortage: 600_000 });
        assert_eq!(calculate_amount_status(1_000_000, 0), AmountStatus::Invalid);
    }

    #[test]
    fn test_amount_status_logic() {
        // exact match
        let expected = 1_000_000u64;
        let actual = 1_000_000u64;
        let min_exact = (expected as u128 * 995 / 1000) as u64;
        let max_exact = (expected as u128 * 1005 / 1000) as u64;
        assert!(actual >= min_exact && actual <= max_exact);

        // overpaid
        let actual_over = 1_010_000u64;
        assert!(actual_over > max_exact);

        // underpaid
        let actual_under = 900_000u64;
        assert!(actual_under < min_exact);
        assert!(actual_under >= expected / 2);

        // severely underpaid
        let actual_severe = 400_000u64;
        assert!(actual_severe < expected / 2);
    }

    // ==================== 审计回归测试 ====================

    #[test]
    fn h1_parse_tron_response_base58_address_match() {
        // H1修复: expected_to 是 Base58 字符串字节，应直接匹配响应中的 Base58 地址
        // 修复前: bytes_to_hex("TSeller...") → "5453656c6c65722e2e2e" → 永远不匹配
        // 修复后: 直接用 "TSeller..." 匹配
        let response = br#"{"contractRet":"SUCCESS","contract_address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","to_address":"TSellerAddr12345678901234567890","confirmations":100,"amount":10000000}"#;
        let expected_to = b"TSellerAddr12345678901234567890";
        let result = parse_tron_response(response, expected_to, None, 10_000_000).unwrap();
        assert!(result.is_valid);
        assert_eq!(result.amount_status, AmountStatus::Exact);
    }

    #[test]
    fn h1_parse_tron_response_address_mismatch() {
        let response = br#"{"contractRet":"SUCCESS","contract_address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","to_address":"TSellerAddr12345678901234567890","confirmations":100,"amount":10000000}"#;
        let wrong_to = b"TWrongAddr123456789012345678901";
        let result = parse_tron_response(response, wrong_to, None, 10_000_000).unwrap();
        assert!(!result.is_valid);
        assert_eq!(result.error, Some(b"Recipient address mismatch".to_vec()));
    }

    #[test]
    fn m2_calculate_amount_status_large_amount_no_overflow() {
        // M2修复: 大金额不应溢出
        // 修复前: 10^16 * 1005 溢出 u64（u64::MAX ≈ 1.84×10^19）
        // 修复后: 使用 u128 中间计算
        let large_amount: u64 = 10_000_000_000_000_000; // 10^16 ($10 billion USDT)
        let result = calculate_amount_status(large_amount, large_amount);
        assert_eq!(result, AmountStatus::Exact);

        // 确认边界正确
        let slightly_over = (large_amount as u128 * 1006 / 1000) as u64;
        let result2 = calculate_amount_status(large_amount, slightly_over);
        assert!(matches!(result2, AmountStatus::Overpaid { .. }));
    }

    #[test]
    fn l1_best_tx_hash_tracks_largest_single() {
        // L1修复: best_tx_hash 应指向最大单笔转账，不是"大于前序总和"
        // 反例: [6M, 5M, 7M] — 7M 是最大单笔，修复前 best 停留在 6M
        let response = br#"{"data":[{"transaction_id":"tx_6m","from":"TBuyer","to":"TSeller","value":"6000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}},{"transaction_id":"tx_5m","from":"TBuyer","to":"TSeller","value":"5000000","block_timestamp":1700000001000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}},{"transaction_id":"tx_7m","from":"TBuyer","to":"TSeller","value":"7000000","block_timestamp":1700000002000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
        let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
        assert!(result.found);
        assert_eq!(result.actual_amount, Some(18_000_000)); // 6M+5M+7M
        // best_tx_hash 应为最大单笔 7M 的 tx
        assert_eq!(result.tx_hash, Some(b"tx_7m".to_vec()));
    }

    #[test]
    fn l2_calculate_amount_status_expected_zero_returns_invalid() {
        // L2修复: expected==0 返回 Invalid（与 pallet-trading-common 一致）
        // 修复前返回 Overpaid
        assert_eq!(calculate_amount_status(0, 1_000_000), AmountStatus::Invalid);
        assert_eq!(calculate_amount_status(0, 0), AmountStatus::Invalid);
    }

    #[test]
    fn h1_parse_tron_response_insufficient_confirmations() {
        let response = br#"{"contractRet":"SUCCESS","contract_address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","to_address":"TSellerXXX","confirmations":5,"amount":10000000}"#;
        let result = parse_tron_response(response, b"TSellerXXX", None, 10_000_000).unwrap();
        assert!(!result.is_valid);
        assert_eq!(result.confirmations, 5);
        assert_eq!(result.error, Some(b"Insufficient confirmations".to_vec()));
    }

    #[test]
    fn h1_parse_tron_response_not_usdt_contract() {
        let response = br#"{"contractRet":"SUCCESS","contract_address":"TFakeContract","to_address":"TSellerXXX","confirmations":100,"amount":10000000}"#;
        let result = parse_tron_response(response, b"TSellerXXX", None, 10_000_000).unwrap();
        assert!(!result.is_valid);
        assert_eq!(result.error, Some(b"Not a USDT TRC20 transaction".to_vec()));
    }

    #[test]
    fn h1_parse_tron_response_tx_not_successful() {
        let response = br#"{"contractRet":"REVERT","to_address":"TSellerXXX"}"#;
        let result = parse_tron_response(response, b"TSellerXXX", None, 10_000_000).unwrap();
        assert!(!result.is_valid);
        assert_eq!(result.error, Some(b"Transaction not successful".to_vec()));
    }

    // ==================== Phase 1 新增回归测试 ====================

    #[test]
    fn h6_parse_tron_response_sender_address_check() {
        let response = br#"{"contractRet":"SUCCESS","contract_address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","from":"TBuyerAddr1234567890","to_address":"TSellerAddr12345678901234567890","confirmations":100,"amount":10000000}"#;
        // 提供正确的 from 地址 → 通过
        let result = parse_tron_response(
            response, b"TSellerAddr12345678901234567890", Some(b"TBuyerAddr1234567890"), 10_000_000
        ).unwrap();
        assert!(result.is_valid);
    }

    #[test]
    fn h6_parse_tron_response_sender_mismatch() {
        let response = br#"{"contractRet":"SUCCESS","contract_address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","from":"TBuyerAddr1234567890","to_address":"TSellerAddr12345678901234567890","confirmations":100,"amount":10000000}"#;
        // 提供错误的 from 地址 → 拒绝
        let result = parse_tron_response(
            response, b"TSellerAddr12345678901234567890", Some(b"TWrongSender1234567890"), 10_000_000
        ).unwrap();
        assert!(!result.is_valid);
        assert_eq!(result.error, Some(b"Sender address mismatch".to_vec()));
    }

    #[test]
    fn h6_parse_tron_response_no_from_check_when_none() {
        let response = br#"{"contractRet":"SUCCESS","contract_address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","from":"TRandomSender","to_address":"TSellerAddr12345678901234567890","confirmations":100,"amount":10000000}"#;
        // expected_from=None → 不检查发送方（向后兼容）
        let result = parse_tron_response(
            response, b"TSellerAddr12345678901234567890", None, 10_000_000
        ).unwrap();
        assert!(result.is_valid);
    }

    #[test]
    fn h7_parse_tron_response_expected_zero_returns_invalid() {
        // H7修复: 内联逻辑现在统一使用 calculate_amount_status
        // expected_amount=0 应返回 Invalid（修复前返回 Overpaid）
        let response = br#"{"contractRet":"SUCCESS","contract_address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","to_address":"TSellerAddr12345678901234567890","confirmations":100,"amount":10000000}"#;
        let result = parse_tron_response(
            response, b"TSellerAddr12345678901234567890", None, 0
        ).unwrap();
        assert!(!result.is_valid);
        assert_eq!(result.amount_status, AmountStatus::Invalid);
    }

    #[test]
    fn h8_add_endpoint_rejects_http() {
        assert_eq!(add_endpoint("http://api.example.com"), Err("Endpoint must use HTTPS"));
    }

    #[test]
    fn h8_add_endpoint_rejects_short_url() {
        assert_eq!(add_endpoint("https://x"), Err("Endpoint URL length must be 10-256 characters"));
    }

    #[test]
    fn h8_add_endpoint_rejects_whitespace() {
        assert_eq!(add_endpoint("https://api.example .com"), Err("Endpoint URL must not contain whitespace"));
    }

    #[test]
    fn l6_hex_to_bytes_handles_0x_prefix() {
        // L6修复: 0x 前缀应被自动去除
        assert_eq!(hex_to_bytes("0x1234abcd").unwrap(), vec![0x12, 0x34, 0xab, 0xcd]);
        assert_eq!(hex_to_bytes("0X1234ABCD").unwrap(), vec![0x12, 0x34, 0xab, 0xcd]);
        // 无前缀仍然正常工作
        assert_eq!(hex_to_bytes("1234abcd").unwrap(), vec![0x12, 0x34, 0xab, 0xcd]);
    }

    // ==================== Phase 2 新增回归测试 (C1+C2+M5) ====================

    #[test]
    fn c1_json_helpers_basic() {
        use lite_json::parse_json;
        let json_str = r#"{"name":"Alice","age":30,"nested":{"key":"val"}}"#;
        let val = parse_json(json_str).unwrap();
        assert_eq!(json_find_str(&val, "name"), Some("Alice".into()));
        assert_eq!(json_find_u64(&val, "age"), Some(30));
        assert_eq!(json_find_str(&val, "key"), Some("val".into()));
        assert_eq!(json_find_str(&val, "missing"), None);
    }

    #[test]
    fn c1_json_has_str_value_exact_match() {
        use lite_json::parse_json;
        // json_has_str_value should only match complete string values, not substrings
        let json_str = r#"{"addr":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","other":"TR7NHq"}"#;
        let val = parse_json(json_str).unwrap();
        assert!(json_has_str_value(&val, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"));
        assert!(json_has_str_value(&val, "TR7NHq")); // exact match of "other" field
        assert!(!json_has_str_value(&val, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6")); // prefix, not exact
    }

    #[test]
    fn c1_parse_tron_response_rejects_invalid_json() {
        let response = b"this is not json";
        let result = parse_tron_response(response, b"TSeller", None, 10_000_000);
        assert_eq!(result, Err("Invalid JSON response"));
    }

    #[test]
    fn c1_parse_transfer_list_rejects_invalid_json() {
        let response = b"not json at all";
        let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0);
        assert_eq!(result, Err("Invalid JSON response"));
    }

    #[test]
    fn c2_amount_extracted_by_field_name() {
        // C2修复: amount 按字段名提取，不再匹配首个子串
        // 即使有嵌套 amount 字段，也能正确提取顶层 amount
        let response = br#"{"contractRet":"SUCCESS","contract_address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","to_address":"TSellerXXX","confirmations":100,"amount":5000000}"#;
        let result = parse_tron_response(response, b"TSellerXXX", None, 5_000_000).unwrap();
        assert!(result.is_valid);
        assert_eq!(result.actual_amount, Some(5_000_000));
        assert_eq!(result.amount_status, AmountStatus::Exact);
    }

    #[test]
    fn m5_transfer_list_handles_braces_in_strings() {
        // M5修复: 旧 find_matching_brace 无法处理字符串中的 {}
        // 新 lite-json 解析器正确处理任意 JSON
        let response = br#"{"data":[{"transaction_id":"tx{special}","from":"TBuyer","to":"TSeller","value":"10000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
        let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
        assert!(result.found);
        assert_eq!(result.actual_amount, Some(10_000_000));
        assert_eq!(result.tx_hash, Some(b"tx{special}".to_vec()));
    }

    #[test]
    fn c1_transfer_list_from_exact_match() {
        // C1修复: from 必须精确匹配字段值，不能是子串
        // 旧代码用 contains("\"from\":\"TBuyer\"") 可能被 "TBuyerExtra" 包含
        let response = br#"{"data":[{"transaction_id":"tx1","from":"TBuyerExtra","to":"TSeller","value":"10000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
        let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
        // "TBuyerExtra" != "TBuyer" → should NOT match
        assert!(!result.found);
    }

    #[test]
    fn m8_calculate_amount_status_extreme_expected_no_wrap() {
        // M8修复: expected 接近 u64::MAX 时 max_exact 不应回绕
        let huge = u64::MAX; // 18_446_744_073_709_551_615
        // 修复前: (u64::MAX as u128 * 1005 / 1000) as u64 会截断
        // 修复后: .min(u64::MAX as u128) 阻止截断
        let result = calculate_amount_status(huge, huge);
        // huge * 995/1000 < huge < huge * 1005/1000 (capped at u64::MAX) → Exact
        assert_eq!(result, AmountStatus::Exact);
    }
}
