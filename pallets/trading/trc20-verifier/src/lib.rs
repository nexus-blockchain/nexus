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
pub fn add_endpoint(endpoint: &str) {
    let mut config = get_endpoint_config();
    let endpoint_str = String::from(endpoint);

    if !config.endpoints.contains(&endpoint_str) {
        config.endpoints.push(endpoint_str);
        config.updated_at = current_timestamp_ms();
        save_endpoint_config(&config);
        log::info!(target: "trc20-verifier", "Added endpoint: {}", endpoint);
    }
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

    // 3. 解析响应
    parse_tron_response(&response, expected_to, expected_amount)
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

// ==================== 响应解析 ====================

/// 解析 TronGrid API 响应
fn parse_tron_response(
    response: &[u8],
    expected_to: &[u8],
    expected_amount: u64,
) -> Result<TronTxVerification, &'static str> {
    let response_str = core::str::from_utf8(response)
        .map_err(|_| "Invalid UTF-8 response")?;

    let mut result = TronTxVerification::default();
    result.expected_amount = Some(expected_amount);

    // 检查是否包含成功状态
    // ⚠️ 安全说明 (H4/H5): 字符串包含匹配在 OCW 安全边界内可接受，
    // 但需注意恶意 API 端点可能注入匹配字符串。生产环境应限制可信端点。
    if !response_str.contains("\"contractRet\":\"SUCCESS\"")
        && !response_str.contains("\"contractRet\": \"SUCCESS\"") {
        result.error = Some(b"Transaction not successful".to_vec());
        return Ok(result);
    }

    // 🆕 M6修复: 验证 USDT 合约地址，防止其他 TRC20 代币交易冒充
    if !response_str.contains(USDT_CONTRACT) {
        result.error = Some(b"Not a USDT TRC20 transaction".to_vec());
        return Ok(result);
    }

    // 🆕 L1修复: 检查确认数
    let confirmations = extract_confirmations(response_str).unwrap_or(0);
    result.confirmations = confirmations;
    if confirmations < MIN_CONFIRMATIONS {
        result.error = Some(b"Insufficient confirmations".to_vec());
        return Ok(result);
    }

    // 检查收款地址
    let expected_to_hex = bytes_to_hex(expected_to);
    if !response_str.contains(&expected_to_hex) {
        result.error = Some(b"Recipient address mismatch".to_vec());
        return Ok(result);
    }

    // 提取实际金额
    let actual_amount = extract_amount(response_str);
    result.actual_amount = actual_amount;

    // 计算金额匹配状态
    let (amount_status, is_acceptable) = match actual_amount {
        Some(actual) => {
            let min_exact = expected_amount * 995 / 1000;  // -0.5%
            let max_exact = expected_amount * 1005 / 1000; // +0.5%
            let severe_threshold = expected_amount / 2;    // 50%

            if actual >= min_exact && actual <= max_exact {
                (AmountStatus::Exact, true)
            } else if actual > max_exact {
                let excess = actual.saturating_sub(expected_amount);
                log::info!(target: "trc20-verifier", "Overpaid: expected={}, actual={}, excess={}",
                    expected_amount, actual, excess);
                (AmountStatus::Overpaid { excess }, true)
            } else if actual >= severe_threshold {
                let shortage = expected_amount.saturating_sub(actual);
                log::warn!(target: "trc20-verifier", "Underpaid: expected={}, actual={}, shortage={}",
                    expected_amount, actual, shortage);
                (AmountStatus::Underpaid { shortage }, false)
            } else if actual > 0 {
                let shortage = expected_amount.saturating_sub(actual);
                log::error!(target: "trc20-verifier", "Severely underpaid: expected={}, actual={}",
                    expected_amount, actual);
                (AmountStatus::SeverelyUnderpaid { shortage }, false)
            } else {
                (AmountStatus::Invalid, false)
            }
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

/// 🆕 L1: 从响应中提取确认数
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

/// 从响应中提取金额
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

// ==================== 工具函数 ====================

/// 字节数组转十六进制字符串
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// 十六进制字符串转字节数组
pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, &'static str> {
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

    #[test]
    fn test_amount_status_logic() {
        // exact match
        let expected = 1_000_000u64;
        let actual = 1_000_000u64;
        let min_exact = expected * 995 / 1000;
        let max_exact = expected * 1005 / 1000;
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
}
