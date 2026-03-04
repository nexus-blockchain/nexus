#![cfg_attr(not(feature = "std"), no_std)]

//! # TRC20 交易验证共享库 (v0.3.0)
//!
//! ## 概述
//! 共享 TRC20 验证逻辑。
//! 可被 `pallet-nex-market`、`pallet-entity-market` 等模块复用。
//!
//! ## 功能
//! - TronGrid API 调用验证 TRC20 交易
//! - 端点健康评分与动态排序
//! - 并行请求竞速模式 + API Key 支持
//! - 金额匹配状态判定
//! - 请求速率限制与响应缓存
//! - 分页查询 (>50 笔转账)
//! - TronVerifier trait 抽象 (上层可 Mock)
//! - 验证审计日志
//!
//! ## 版本历史
//! - v0.3.0 (2026-03-04): 全面增强 — VerificationError 枚举、API Key、速率限制、
//!   响应缓存、分页、TronVerifier trait、审计日志、可配置参数
//! - v0.2.0 (2026-02-23): 新增 verify_trc20_by_transfer
//! - v0.1.0 (2026-02-08): 提取为共享库

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::format;
use sp_runtime::offchain::{http, Duration};
use sp_core::offchain::StorageKind;
use codec::{Encode, Decode};
use lite_json::{JsonValue, parse_json};

// ==================== 错误类型 (H5) ====================

/// 验证错误枚举 — 取代 `&'static str`，上层 pallet 可 match 具体错误做差异化处理
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationError {
    /// HTTP 请求发送失败
    HttpSendFailed,
    /// HTTP 请求超时
    HttpTimeout,
    /// 非 200 HTTP 响应
    HttpBadStatus(u16),
    /// 空响应体
    EmptyResponse,
    /// 所有端点均失败（并行或串行）
    AllEndpointsFailed,
    /// 无可用端点
    NoEndpoints,
    /// 响应非法 UTF-8
    InvalidUtf8,
    /// 响应非法 JSON
    InvalidJson,
    /// 请求被速率限制
    RateLimited,
    /// 端点 URL 格式错误
    InvalidEndpointUrl(&'static str),
    /// 端点数量已达上限
    MaxEndpointsReached,
}

impl VerificationError {
    /// 转换为向后兼容的静态字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HttpSendFailed => "Failed to send HTTP request",
            Self::HttpTimeout => "HTTP request timeout",
            Self::HttpBadStatus(_) => "Non-200 HTTP response",
            Self::EmptyResponse => "Empty response body",
            Self::AllEndpointsFailed => "All endpoints failed",
            Self::NoEndpoints => "No endpoints available",
            Self::InvalidUtf8 => "Invalid UTF-8 response",
            Self::InvalidJson => "Invalid JSON response",
            Self::RateLimited => "Rate limited",
            Self::InvalidEndpointUrl(reason) => reason,
            Self::MaxEndpointsReached => "Maximum endpoints reached",
        }
    }
}

impl core::fmt::Display for VerificationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::HttpBadStatus(code) => write!(f, "Non-200 HTTP response ({})", code),
            other => write!(f, "{}", other.as_str()),
        }
    }
}

/// 向后兼容: VerificationError → &'static str
impl From<VerificationError> for &'static str {
    fn from(e: VerificationError) -> Self {
        e.as_str()
    }
}

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

/// 端点数量上限 (M3)
const MAX_ENDPOINTS: usize = 10;

/// 速率限制存储键 (H2)
const RATE_LIMIT_KEY: &[u8] = b"ocw_rate_limit_last_req";

/// 响应缓存存储键前缀 (M1)
const RESPONSE_CACHE_PREFIX: &[u8] = b"ocw_resp_cache::";

/// 验证器配置存储键 (H3)
const VERIFIER_CONFIG_KEY: &[u8] = b"ocw_verifier_config";

/// 审计日志存储键前缀 (M7)
const AUDIT_LOG_PREFIX: &[u8] = b"ocw_audit_log::";

/// 审计日志计数器键 (M7)
const AUDIT_LOG_COUNTER_KEY: &[u8] = b"ocw_audit_log_counter";

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

/// 端点配置 (H1+L1+L2 增强)
#[derive(Debug, Clone, Encode, Decode)]
pub struct EndpointConfig {
    /// 端点 URL 列表
    pub endpoints: Vec<String>,
    /// 是否启用并行竞速模式
    pub parallel_mode: bool,
    /// 最后更新时间
    pub updated_at: u64,
    /// API Key 映射: (endpoint_prefix, api_key) (H1)
    pub api_keys: Vec<(String, String)>,
    /// 串行模式 HTTP 超时（毫秒）(L1)
    pub timeout_ms: u64,
    /// 并行竞速 HTTP 超时（毫秒）(L1)
    pub timeout_race_ms: u64,
    /// 端点优先级加成: (endpoint, boost) (L2)
    pub priority_boosts: Vec<(String, u32)>,
}

impl Default for EndpointConfig {
    fn default() -> Self {
        Self {
            endpoints: DEFAULT_ENDPOINTS.iter().map(|s| String::from(*s)).collect(),
            parallel_mode: true,
            updated_at: 0,
            api_keys: Vec::new(),
            timeout_ms: HTTP_TIMEOUT_MS,
            timeout_race_ms: HTTP_TIMEOUT_RACE_MS,
            priority_boosts: Vec::new(),
        }
    }
}

/// 验证器可配置参数 (H3)
#[derive(Debug, Clone, Encode, Decode)]
pub struct VerifierConfig {
    /// USDT TRC20 合约地址（覆盖默认常量）
    pub usdt_contract: String,
    /// 最小确认数
    pub min_confirmations: u32,
    /// 速率限制间隔（毫秒，0=禁用）(H2)
    pub rate_limit_interval_ms: u64,
    /// 响应缓存 TTL（毫秒，0=禁用）(M1)
    pub cache_ttl_ms: u64,
    /// 最大分页数 (M2)
    pub max_pages: u32,
    /// 审计日志保留条数 (M7)
    pub audit_log_retention: u32,
    /// 最后更新时间
    pub updated_at: u64,
}

impl Default for VerifierConfig {
    fn default() -> Self {
        Self {
            usdt_contract: String::from(USDT_CONTRACT),
            min_confirmations: MIN_CONFIRMATIONS,
            rate_limit_interval_ms: 200,  // 200ms 默认间隔
            cache_ttl_ms: 30_000,         // 30s 默认缓存
            max_pages: 3,                 // 最多 3 页
            audit_log_retention: 100,     // 保留最近 100 条
            updated_at: 0,
        }
    }
}

/// 获取验证器配置
pub fn get_verifier_config() -> VerifierConfig {
    sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, VERIFIER_CONFIG_KEY)
        .and_then(|data| VerifierConfig::decode(&mut &data[..]).ok())
        .unwrap_or_default()
}

/// 保存验证器配置
pub fn save_verifier_config(config: &VerifierConfig) {
    sp_io::offchain::local_storage_set(
        StorageKind::PERSISTENT,
        VERIFIER_CONFIG_KEY,
        &config.encode(),
    );
}

/// 获取有效 USDT 合约地址（配置优先，否则默认常量）
fn effective_usdt_contract() -> String {
    let config = get_verifier_config();
    if config.usdt_contract.is_empty() {
        String::from(USDT_CONTRACT)
    } else {
        config.usdt_contract
    }
}

/// 获取有效最小确认数
fn effective_min_confirmations() -> u32 {
    let config = get_verifier_config();
    if config.min_confirmations == 0 { MIN_CONFIRMATIONS } else { config.min_confirmations }
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
/// H8修复: URL 格式校验  
/// M3修复: 端点数量上限检查
pub fn add_endpoint(endpoint: &str) -> Result<(), VerificationError> {
    if !endpoint.starts_with("https://") {
        return Err(VerificationError::InvalidEndpointUrl("Endpoint must use HTTPS"));
    }
    if endpoint.len() < 10 || endpoint.len() > 256 {
        return Err(VerificationError::InvalidEndpointUrl("Endpoint URL length must be 10-256 characters"));
    }
    if endpoint.bytes().any(|b| b == b' ' || b == b'\t' || b == b'\n' || b == b'\r') {
        return Err(VerificationError::InvalidEndpointUrl("Endpoint URL must not contain whitespace"));
    }

    let mut config = get_endpoint_config();
    let endpoint_str = String::from(endpoint);

    if !config.endpoints.contains(&endpoint_str) {
        // M3: 端点数量上限
        if config.endpoints.len() >= MAX_ENDPOINTS {
            return Err(VerificationError::MaxEndpointsReached);
        }
        config.endpoints.push(endpoint_str);
        config.updated_at = current_timestamp_ms();
        save_endpoint_config(&config);
        log::info!(target: "trc20-verifier", "Added endpoint: {}", endpoint);
    }
    Ok(())
}

/// 设置端点 API Key (H1)
pub fn set_api_key(endpoint: &str, api_key: &str) {
    let mut config = get_endpoint_config();
    let ep = String::from(endpoint);
    let key = String::from(api_key);
    if let Some(pos) = config.api_keys.iter().position(|(e, _)| e == &ep) {
        config.api_keys[pos].1 = key;
    } else {
        config.api_keys.push((ep, key));
    }
    config.updated_at = current_timestamp_ms();
    save_endpoint_config(&config);
}

/// 获取端点对应的 API Key (H1)
fn get_api_key_for_endpoint(endpoint: &str) -> Option<String> {
    let config = get_endpoint_config();
    config.api_keys.iter()
        .find(|(e, _)| endpoint.starts_with(e.as_str()))
        .map(|(_, k)| k.clone())
}

/// 移除端点 (M1修复: 同时清理关联的 api_keys 和 priority_boosts)
pub fn remove_endpoint(endpoint: &str) {
    let mut config = get_endpoint_config();
    let endpoint_str = String::from(endpoint);

    if let Some(pos) = config.endpoints.iter().position(|e| e == &endpoint_str) {
        config.endpoints.remove(pos);
        // M1修复: 清理关联的 API key
        config.api_keys.retain(|(e, _)| e != &endpoint_str);
        // M1修复: 清理关联的优先级加成
        config.priority_boosts.retain(|(e, _)| e != &endpoint_str);
        config.updated_at = current_timestamp_ms();
        save_endpoint_config(&config);
        log::info!(target: "trc20-verifier", "Removed endpoint: {}", endpoint);
    }
}

/// 获取按健康评分排序的端点列表 (L2 增强: 支持优先级加成)
pub fn get_sorted_endpoints() -> Vec<String> {
    let config = get_endpoint_config();
    let mut endpoints_with_scores: Vec<(String, u32)> = config.endpoints
        .iter()
        .map(|e| {
            let health = get_endpoint_health(e);
            let boost = config.priority_boosts.iter()
                .find(|(ep, _)| ep == e)
                .map(|(_, b)| *b)
                .unwrap_or(0);
            (e.clone(), health.score.saturating_add(boost))
        })
        .collect();

    // 按评分降序排序
    endpoints_with_scores.sort_by(|a, b| b.1.cmp(&a.1));

    endpoints_with_scores.into_iter().map(|(e, _)| e).collect()
}

/// 重置端点健康评分 (M4)
pub fn reset_endpoint_health(endpoint: &str) {
    save_endpoint_health(endpoint, &EndpointHealth::default());
    log::info!(target: "trc20-verifier", "Reset health for endpoint: {}", endpoint);
}

/// 获取所有端点健康状态 (M5)
pub fn get_all_endpoint_health() -> Vec<(String, EndpointHealth)> {
    let config = get_endpoint_config();
    config.endpoints.iter()
        .map(|e| (e.clone(), get_endpoint_health(e)))
        .collect()
}

/// 设置端点优先级加成 (L2)
pub fn set_endpoint_priority_boost(endpoint: &str, boost: u32) {
    let mut config = get_endpoint_config();
    let ep = String::from(endpoint);
    if let Some(pos) = config.priority_boosts.iter().position(|(e, _)| e == &ep) {
        config.priority_boosts[pos].1 = boost;
    } else {
        config.priority_boosts.push((ep, boost));
    }
    config.updated_at = current_timestamp_ms();
    save_endpoint_config(&config);
}

// ==================== 速率限制 (H2) ====================

/// 检查速率限制，通过则更新时间戳
fn check_rate_limit() -> Result<(), VerificationError> {
    let config = get_verifier_config();
    if config.rate_limit_interval_ms == 0 {
        return Ok(());
    }
    let now = current_timestamp_ms();
    let last = sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, RATE_LIMIT_KEY)
        .and_then(|d| u64::decode(&mut &d[..]).ok())
        .unwrap_or(0);
    if now.saturating_sub(last) < config.rate_limit_interval_ms {
        return Err(VerificationError::RateLimited);
    }
    sp_io::offchain::local_storage_set(
        StorageKind::PERSISTENT, RATE_LIMIT_KEY, &now.encode(),
    );
    Ok(())
}

// ==================== 响应缓存 (M1) ====================

/// 获取缓存的响应
fn get_cached_response(url: &str) -> Option<Vec<u8>> {
    let config = get_verifier_config();
    if config.cache_ttl_ms == 0 { return None; }
    let key = [RESPONSE_CACHE_PREFIX, url.as_bytes()].concat();
    let data = sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, &key)?;
    let (timestamp, response) = <(u64, Vec<u8>)>::decode(&mut &data[..]).ok()?;
    let now = current_timestamp_ms();
    if now.saturating_sub(timestamp) > config.cache_ttl_ms {
        return None;
    }
    log::debug!(target: "trc20-verifier", "Cache hit for URL (age={}ms)", now.saturating_sub(timestamp));
    Some(response)
}

/// 设置响应缓存
fn set_cached_response(url: &str, response: &[u8]) {
    let config = get_verifier_config();
    if config.cache_ttl_ms == 0 { return; }
    let key = [RESPONSE_CACHE_PREFIX, url.as_bytes()].concat();
    let now = current_timestamp_ms();
    let data = (now, response.to_vec()).encode();
    sp_io::offchain::local_storage_set(StorageKind::PERSISTENT, &key, &data);
}

// ==================== 审计日志 (M7) ====================

/// 审计日志条目
#[derive(Debug, Clone, Encode, Decode)]
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

/// 记录审计日志
fn write_audit_log(entry: &AuditLogEntry) {
    let config = get_verifier_config();
    if config.audit_log_retention == 0 { return; }

    let counter = sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, AUDIT_LOG_COUNTER_KEY)
        .and_then(|d| u64::decode(&mut &d[..]).ok())
        .unwrap_or(0);
    let next = counter.wrapping_add(1);

    let key = [AUDIT_LOG_PREFIX, &next.encode()].concat();
    sp_io::offchain::local_storage_set(StorageKind::PERSISTENT, &key, &entry.encode());
    sp_io::offchain::local_storage_set(StorageKind::PERSISTENT, AUDIT_LOG_COUNTER_KEY, &next.encode());

    // 清理超出保留量的旧日志
    if next > config.audit_log_retention as u64 {
        let old_id = next - config.audit_log_retention as u64;
        let old_key = [AUDIT_LOG_PREFIX, &old_id.encode()].concat();
        sp_io::offchain::local_storage_set(StorageKind::PERSISTENT, &old_key, &[]);
    }
}

/// 获取最近的审计日志 (M7)
pub fn get_recent_audit_logs(max_count: u32) -> Vec<AuditLogEntry> {
    let counter = sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, AUDIT_LOG_COUNTER_KEY)
        .and_then(|d| u64::decode(&mut &d[..]).ok())
        .unwrap_or(0);
    if counter == 0 || max_count == 0 { return Vec::new(); }

    let start = counter.saturating_sub(max_count as u64 - 1);
    let mut logs = Vec::new();
    for id in start..=counter {
        let key = [AUDIT_LOG_PREFIX, &id.encode()].concat();
        if let Some(data) = sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, &key) {
            if let Ok(entry) = AuditLogEntry::decode(&mut &data[..]) {
                logs.push(entry);
            }
        }
    }
    logs
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

/// 验证 TRC20 交易（按 tx_hash）
///
/// ⚠️ D2: 当前无实际消费方使用此接口，推荐使用 `verify_trc20_by_transfer`。
/// 未来版本可能移除。
#[deprecated(note = "No active consumers. Use verify_trc20_by_transfer instead.")]
pub fn verify_trc20_transaction(
    tx_hash: &[u8],
    expected_to: &[u8],
    expected_amount: u64,
) -> Result<TronTxVerification, VerificationError> {
    // H3修复: 移除冗余 check_rate_limit()，fetch_url_with_fallback 内部已包含速率限制
    let tx_hash_hex = bytes_to_hex(tx_hash);
    let url = format!("{}/v1/transactions/{}", TRONGRID_MAINNET, tx_hash_hex);

    let response = fetch_url_with_fallback(&url)?;

    parse_tron_response(&response, expected_to, None, expected_amount)
}

/// 简化验证接口：仅返回 bool
///
/// ⚠️ D2: 同 verify_trc20_transaction，推荐使用 `verify_trc20_by_transfer`。
#[deprecated(note = "No active consumers. Use verify_trc20_by_transfer instead.")]
#[allow(deprecated)]
pub fn verify_trc20_transaction_simple(
    tx_hash: &[u8],
    expected_to: &[u8],
    expected_amount: u64,
) -> Result<bool, VerificationError> {
    let result = verify_trc20_transaction(tx_hash, expected_to, expected_amount)?;
    Ok(result.is_valid)
}

// ==================== 并行请求竞速模式 ====================

/// 发送 HTTP GET 请求（智能模式选择 + H2 速率限制 + M1 缓存）
fn fetch_url_with_fallback(url: &str) -> Result<Vec<u8>, VerificationError> {
    // M1: 检查缓存
    if let Some(cached) = get_cached_response(url) {
        return Ok(cached);
    }

    // H2: 速率限制
    check_rate_limit()?;

    let config = get_endpoint_config();

    let result = if config.parallel_mode && config.endpoints.len() > 1 {
        fetch_url_parallel_race(url, &config)
    } else {
        fetch_url_sequential(url, &config)
    };

    // M1: 成功时写入缓存
    if let Ok(ref body) = result {
        set_cached_response(url, body);
    }

    result
}

/// 并行竞速模式：同时请求所有端点，使用最快响应 (H1 API Key + L1 可配置超时)
fn fetch_url_parallel_race(url: &str, config: &EndpointConfig) -> Result<Vec<u8>, VerificationError> {
    let endpoints = &config.endpoints;
    log::info!(target: "trc20-verifier", "Starting parallel race with {} endpoints", endpoints.len());

    let start_time = current_timestamp_ms();

    let mut pending_requests: Vec<(String, http::PendingRequest)> = Vec::new();
    let timeout = sp_io::offchain::timestamp()
        .add(Duration::from_millis(config.timeout_race_ms));

    for endpoint in endpoints.iter() {
        let target_url = url.replace(TRONGRID_MAINNET, endpoint);

        // H1: 添加 API Key header
        let mut request = http::Request::get(&target_url);
        if let Some(api_key) = get_api_key_for_endpoint(endpoint) {
            request = request.add_header("TRON-PRO-API-KEY", &api_key);
        }

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
        return Err(VerificationError::AllEndpointsFailed);
    }

    let mut winner_response: Option<Vec<u8>> = None;
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

                        winner_response = Some(body);
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

    for endpoint in failed_endpoints {
        let mut health = get_endpoint_health(&endpoint);
        health.record_failure();
        save_endpoint_health(&endpoint, &health);
    }

    match winner_response {
        Some(body) => Ok(body),
        None => {
            log::error!(target: "trc20-verifier", "All parallel requests failed");
            Err(VerificationError::AllEndpointsFailed)
        }
    }
}

/// 串行故障转移模式 (H1 API Key + L1 可配置超时)
fn fetch_url_sequential(url: &str, config: &EndpointConfig) -> Result<Vec<u8>, VerificationError> {
    let sorted_endpoints = get_sorted_endpoints();
    let mut last_error = VerificationError::NoEndpoints;

    log::info!(target: "trc20-verifier", "Sequential mode with {} endpoints (sorted by health)",
        sorted_endpoints.len());

    for (idx, endpoint) in sorted_endpoints.iter().enumerate() {
        let target_url = url.replace(TRONGRID_MAINNET, endpoint);
        let start_time = current_timestamp_ms();

        log::debug!(target: "trc20-verifier", "Trying endpoint {} ({}/{})",
            endpoint, idx + 1, sorted_endpoints.len());

        match fetch_url_with_key(&target_url, endpoint, config.timeout_ms) {
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

/// 发送 HTTP GET 请求 (H1: 支持 API Key, L1: 可配置超时)
fn fetch_url_with_key(url: &str, endpoint: &str, timeout_ms: u64) -> Result<Vec<u8>, VerificationError> {
    log::debug!(target: "trc20-verifier", "Fetching URL: {}", url);

    let mut request = http::Request::get(url);

    // H1: 添加 API Key header
    if let Some(api_key) = get_api_key_for_endpoint(endpoint) {
        request = request.add_header("TRON-PRO-API-KEY", &api_key);
    }

    let timeout = sp_io::offchain::timestamp()
        .add(Duration::from_millis(timeout_ms));

    let pending = request
        .deadline(timeout)
        .send()
        .map_err(|_| VerificationError::HttpSendFailed)?;

    let response = pending
        .try_wait(timeout)
        .map_err(|_| VerificationError::HttpTimeout)?
        .map_err(|_| VerificationError::HttpSendFailed)?;

    if response.code != 200 {
        log::warn!(target: "trc20-verifier", "HTTP response code: {}", response.code);
        return Err(VerificationError::HttpBadStatus(response.code));
    }

    let body = response.body().collect::<Vec<u8>>();

    if body.is_empty() {
        return Err(VerificationError::EmptyResponse);
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
) -> Result<TronTxVerification, VerificationError> {
    let response_str = core::str::from_utf8(response)
        .map_err(|_| VerificationError::InvalidUtf8)?;

    let mut result = TronTxVerification::default();
    result.expected_amount = Some(expected_amount);

    let json_value = parse_json(response_str)
        .map_err(|_| VerificationError::InvalidJson)?;

    // 1. 检查交易成功状态
    let contract_ret = json_find_str(&json_value, "contractRet");
    if contract_ret.as_deref() != Some("SUCCESS") {
        result.error = Some(b"Transaction not successful".to_vec());
        return Ok(result);
    }

    // 2. H3: 使用可配置 USDT 合约地址
    let contract = effective_usdt_contract();
    if !json_has_str_value(&json_value, &contract) {
        result.error = Some(b"Not a USDT TRC20 transaction".to_vec());
        return Ok(result);
    }

    // 3. H3: 使用可配置最小确认数
    let min_conf = effective_min_confirmations();
    let confirmations = json_find_u64(&json_value, "confirmations")
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0);
    result.confirmations = confirmations;
    if confirmations < min_conf {
        result.error = Some(b"Insufficient confirmations".to_vec());
        return Ok(result);
    }

    // 4. 检查发送方地址
    if let Some(expected_from_bytes) = expected_from {
        let expected_from_str = core::str::from_utf8(expected_from_bytes)
            .map_err(|_| VerificationError::InvalidUtf8)?;
        let from_addr = json_find_str(&json_value, "owner_address")
            .or_else(|| json_find_str(&json_value, "from"));
        if from_addr.as_deref() != Some(expected_from_str) {
            if !json_has_str_value(&json_value, expected_from_str) {
                result.error = Some(b"Sender address mismatch".to_vec());
                return Ok(result);
            }
        }
    }

    // 5. 检查收款地址
    let expected_to_str = core::str::from_utf8(expected_to)
        .map_err(|_| VerificationError::InvalidUtf8)?;
    let to_addr = json_find_str(&json_value, "to_address")
        .or_else(|| json_find_str(&json_value, "to"));
    if to_addr.as_deref() != Some(expected_to_str) {
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

/// 单笔匹配转账明细 (M6)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedTransfer {
    pub tx_hash: Vec<u8>,
    pub amount: u64,
    pub block_timestamp: u64,
}

/// TRC20 转账搜索结果 (M6+L3+L4 增强)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransferSearchResult {
    /// 是否找到匹配的转账
    pub found: bool,
    /// 匹配转账的实际金额（USDT 精度 10^6）
    pub actual_amount: Option<u64>,
    /// 最大单笔转账的交易哈希
    pub tx_hash: Option<Vec<u8>>,
    /// 最大单笔转账的区块时间戳（毫秒）
    pub block_timestamp: Option<u64>,
    /// 金额匹配状态
    pub amount_status: AmountStatus,
    /// 错误信息
    pub error: Option<Vec<u8>>,
    /// 所有匹配转账明细 (M6)
    pub matched_transfers: Vec<MatchedTransfer>,
    /// 还需补付金额 (L3)，仅在少付时有值
    pub remaining_amount: Option<u64>,
    /// 估计确认数 (L4)
    pub estimated_confirmations: Option<u32>,
    /// 结果是否被截断（分页未完全遍历）(M2)
    pub truncated: bool,
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
) -> Result<TransferSearchResult, VerificationError> {
    let from_str = core::str::from_utf8(from_address).map_err(|_| VerificationError::InvalidUtf8)?;
    let to_str = core::str::from_utf8(to_address).map_err(|_| VerificationError::InvalidUtf8)?;

    // H3: 使用可配置 USDT 合约地址
    let contract = effective_usdt_contract();
    let verifier_config = get_verifier_config();

    log::info!(target: "trc20-verifier",
        "Searching TRC20 transfers: to={}, from={}, amount={}, since={}",
        to_str, from_str, expected_amount, min_timestamp);

    let now_ms = sp_io::offchain::timestamp().unix_millis();

    // M2: 分页循环
    let mut combined = TransferSearchResult::default();
    let mut fingerprint: Option<String> = None;
    let mut page = 0u32;

    loop {
        // 构建 URL，带可选 fingerprint 分页参数
        let url = if let Some(ref fp) = fingerprint {
            format!(
                "{}/v1/accounts/{}/transactions/trc20?contract_address={}&only_to=true&min_timestamp={}&limit=50&order_by=block_timestamp,desc&fingerprint={}",
                TRONGRID_MAINNET, to_str, contract, min_timestamp, fp
            )
        } else {
            format!(
                "{}/v1/accounts/{}/transactions/trc20?contract_address={}&only_to=true&min_timestamp={}&limit=50&order_by=block_timestamp,desc",
                TRONGRID_MAINNET, to_str, contract, min_timestamp
            )
        };

        let response = fetch_url_with_fallback(&url)?;
        let (page_result, next_fp) = parse_trc20_transfer_list_paged(&response, from_str, expected_amount, now_ms, &contract)?;

        // 合并分页结果 (H2: 传入 expected_amount 以正确计算累计 amount_status)
        merge_transfer_results(&mut combined, &page_result, expected_amount);

        page += 1;

        // 停止分页条件: 已找到足够金额 / 无更多页 / 达到上限
        let enough = matches!(combined.amount_status, AmountStatus::Exact | AmountStatus::Overpaid { .. });
        if enough || next_fp.is_none() || page >= verifier_config.max_pages {
            if !enough && next_fp.is_some() && page >= verifier_config.max_pages {
                combined.truncated = true;
            }
            break;
        }
        fingerprint = next_fp;
    }

    // M7: 写入审计日志
    write_audit_log(&AuditLogEntry {
        timestamp: now_ms,
        action: b"verify_trc20_by_transfer".to_vec(),
        from_address: from_address.to_vec(),
        to_address: to_address.to_vec(),
        expected_amount,
        actual_amount: combined.actual_amount.unwrap_or(0),
        result_ok: combined.found,
        error_msg: combined.error.clone().unwrap_or_default(),
    });

    Ok(combined)
}

/// 合并分页结果 (M2, H2修复: 使用累计总额重新计算 amount_status)
fn merge_transfer_results(combined: &mut TransferSearchResult, page: &TransferSearchResult, expected_amount: u64) {
    // 累加匹配转账
    combined.matched_transfers.extend(page.matched_transfers.clone());

    // 累加金额
    let prev = combined.actual_amount.unwrap_or(0);
    let page_amt = page.actual_amount.unwrap_or(0);
    let total = prev.saturating_add(page_amt);
    if total > 0 {
        combined.found = true;
        combined.actual_amount = Some(total);
    }

    // 跟踪最大单笔
    if let Some(ref pt) = page.tx_hash {
        let page_max = page.matched_transfers.iter().map(|t| t.amount).max().unwrap_or(0);
        let cur_max = combined.matched_transfers.iter()
            .filter(|t| !page.matched_transfers.contains(t))
            .map(|t| t.amount).max().unwrap_or(0);
        if page_max >= cur_max {
            combined.tx_hash = Some(pt.clone());
            combined.block_timestamp = page.block_timestamp;
        }
    }

    // 更新确认数估计
    if page.estimated_confirmations.is_some() {
        combined.estimated_confirmations = page.estimated_confirmations;
    }

    // H2修复: 使用累计总额重新计算 amount_status（而非使用单页状态）
    if total > 0 {
        combined.amount_status = calculate_amount_status(expected_amount, total);
        // 重新计算还需补付金额
        match &combined.amount_status {
            AmountStatus::Underpaid { shortage } | AmountStatus::SeverelyUnderpaid { shortage } => {
                combined.remaining_amount = Some(*shortage);
            },
            _ => {
                combined.remaining_amount = None;
            }
        }
    }

    // 继承错误（仅当合并后仍未找到）
    if !combined.found {
        combined.error = page.error.clone();
        combined.amount_status = page.amount_status.clone();
    }
}

/// 解析 TronGrid TRC20 转账列表响应 (向后兼容包装)
pub fn parse_trc20_transfer_list(
    response: &[u8],
    expected_from: &str,
    expected_amount: u64,
    now_ms: u64,
) -> Result<TransferSearchResult, VerificationError> {
    let contract = effective_usdt_contract();
    let (result, _) = parse_trc20_transfer_list_paged(response, expected_from, expected_amount, now_ms, &contract)?;
    Ok(result)
}

/// 解析 TronGrid TRC20 转账列表响应（带分页支持）
///
/// 返回: (TransferSearchResult, Option<next_fingerprint>)
fn parse_trc20_transfer_list_paged(
    response: &[u8],
    expected_from: &str,
    expected_amount: u64,
    now_ms: u64,
    usdt_contract: &str,
) -> Result<(TransferSearchResult, Option<String>), VerificationError> {
    let response_str = core::str::from_utf8(response)
        .map_err(|_| VerificationError::InvalidUtf8)?;

    let mut result = TransferSearchResult::default();

    let json_value = parse_json(response_str)
        .map_err(|_| VerificationError::InvalidJson)?;

    let root_obj = match json_value.as_object() {
        Some(obj) => obj,
        None => {
            result.error = Some(b"Response is not a JSON object".to_vec());
            return Ok((result, None));
        }
    };

    match json_obj_get(root_obj, "success") {
        Some(JsonValue::Boolean(true)) => {},
        _ => {
            result.error = Some(b"API returned failure".to_vec());
            return Ok((result, None));
        }
    }

    let data_array = match json_obj_get(root_obj, "data") {
        Some(JsonValue::Array(arr)) => arr,
        _ => {
            result.error = Some(b"No data array in response".to_vec());
            return Ok((result, None));
        }
    };

    // M2: 提取分页 fingerprint
    let next_fingerprint = json_obj_get(root_obj, "meta")
        .and_then(|m| m.as_object())
        .and_then(|meta| json_obj_get_str(meta, "fingerprint"));

    // H3: 使用可配置确认数
    let min_conf = effective_min_confirmations();

    let mut total_matched_amount: u64 = 0;
    let mut max_single_amount: u64 = 0;
    let mut best_tx_hash: Option<Vec<u8>> = None;
    let mut best_timestamp: Option<u64> = None;

    for entry_value in data_array.iter() {
        let entry_obj = match entry_value.as_object() {
            Some(obj) => obj,
            None => continue,
        };

        let from_addr = json_obj_get_str(entry_obj, "from");
        if from_addr.as_deref() != Some(expected_from) {
            continue;
        }

        // H3: 使用可配置合约地址
        if !json_has_str_value(entry_value, usdt_contract) {
            continue;
        }

        // 确认数近似检查 (now_ms=0 跳过)
        let mut est_conf: Option<u32> = None;
        if now_ms > 0 {
            if let Some(ts) = json_obj_get_u64(entry_obj, "block_timestamp") {
                let age_ms = now_ms.saturating_sub(ts);
                let min_age_ms = (min_conf as u64).saturating_mul(3000).saturating_mul(2);
                if ts > 0 && age_ms < min_age_ms {
                    log::warn!(target: "trc20-verifier",
                        "Skipping too-recent transfer (ts={}, age={}ms < {}ms)",
                        ts, age_ms, min_age_ms);
                    continue;
                }
                // L4: 估计确认数 (TRON 约 3s/block)
                est_conf = Some((age_ms / 3000) as u32);
            }
        }

        if let Some(amount) = json_obj_get_u64(entry_obj, "value") {
            if amount > 0 {
                total_matched_amount = total_matched_amount.saturating_add(amount);

                let tx_hash_bytes = json_obj_get_str(entry_obj, "transaction_id")
                    .map(|s| s.into_bytes());
                let ts = json_obj_get_u64(entry_obj, "block_timestamp");

                // M6: 记录每笔匹配转账明细
                result.matched_transfers.push(MatchedTransfer {
                    tx_hash: tx_hash_bytes.clone().unwrap_or_default(),
                    amount,
                    block_timestamp: ts.unwrap_or(0),
                });

                if best_tx_hash.is_none() || amount > max_single_amount {
                    max_single_amount = amount;
                    best_tx_hash = tx_hash_bytes;
                    best_timestamp = ts;
                    // L4: 保存最大笔的确认数估计
                    result.estimated_confirmations = est_conf;
                }

                log::info!(target: "trc20-verifier",
                    "Found matching transfer: value={}, running_total={}", amount, total_matched_amount);
            }
        }
    }

    if total_matched_amount == 0 {
        result.error = Some(b"No matching transfer found".to_vec());
        result.amount_status = AmountStatus::Invalid;
        return Ok((result, next_fingerprint));
    }

    result.found = true;
    result.actual_amount = Some(total_matched_amount);
    result.tx_hash = best_tx_hash;
    result.block_timestamp = best_timestamp;

    result.amount_status = calculate_amount_status(expected_amount, total_matched_amount);

    // L3: 计算还需补付金额
    match &result.amount_status {
        AmountStatus::Underpaid { shortage } | AmountStatus::SeverelyUnderpaid { shortage } => {
            result.remaining_amount = Some(*shortage);
        },
        _ => {}
    }

    Ok((result, next_fingerprint))
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

// ==================== TronVerifier trait 抽象 (H4) ====================

/// TronVerifier trait — 上层 pallet 可通过此 trait 注入 Mock 实现
pub trait TronVerifier {
    /// 按 (from, to, amount, min_timestamp) 搜索验证 TRC20 USDT 转账
    fn verify_by_transfer(
        from_address: &[u8],
        to_address: &[u8],
        expected_amount: u64,
        min_timestamp: u64,
    ) -> Result<TransferSearchResult, VerificationError>;
}

/// 默认实现: 调用真实 TronGrid API
pub struct DefaultTronVerifier;

impl TronVerifier for DefaultTronVerifier {
    fn verify_by_transfer(
        from_address: &[u8],
        to_address: &[u8],
        expected_amount: u64,
        min_timestamp: u64,
    ) -> Result<TransferSearchResult, VerificationError> {
        verify_trc20_by_transfer(from_address, to_address, expected_amount, min_timestamp)
    }
}

// ==================== M8: AmountStatus ↔ PaymentVerificationResult 兼容 ====================

impl AmountStatus {
    /// 转换为与 `pallet-trading-common::PaymentVerificationResult` 兼容的名称
    /// 上层 pallet 可用此映射到对应枚举变体
    pub fn to_verification_result_name(&self) -> &'static str {
        match self {
            AmountStatus::Exact => "Exact",
            AmountStatus::Overpaid { .. } => "Overpaid",
            AmountStatus::Underpaid { .. } => "Underpaid",
            AmountStatus::SeverelyUnderpaid { .. } => "SeverelyUnderpaid",
            AmountStatus::Invalid | AmountStatus::Unknown => "Invalid",
        }
    }

    /// 是否可接受（Exact 或 Overpaid）
    pub fn is_acceptable(&self) -> bool {
        matches!(self, AmountStatus::Exact | AmountStatus::Overpaid { .. })
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

    /// 提供 offchain 存储环境的测试辅助宏
    /// parse_tron_response / parse_trc20_transfer_list / add_endpoint
    /// 内部调用 sp_io::offchain::local_storage_get，需要 Externalities 环境
    fn with_offchain_ext<R>(f: impl FnOnce() -> R) -> R {
        let (offchain, _state) = sp_core::offchain::testing::TestOffchainExt::new();
        let mut ext = sp_io::TestExternalities::default();
        ext.register_extension(sp_core::offchain::OffchainDbExt::new(offchain.clone()));
        ext.register_extension(sp_core::offchain::OffchainWorkerExt::new(offchain));
        ext.execute_with(f)
    }

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
        with_offchain_ext(|| {
            let response = br#"{"data":[{"transaction_id":"abc123","from":"TBuyerXXX","to":"TSellerYYY","value":"10000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
            let result = parse_trc20_transfer_list(response, "TBuyerXXX", 10_000_000, 0).unwrap();
            assert!(result.found);
            assert_eq!(result.actual_amount, Some(10_000_000));
            assert_eq!(result.amount_status, AmountStatus::Exact);
            assert_eq!(result.tx_hash, Some(b"abc123".to_vec()));
        });
    }

    #[test]
    fn test_parse_transfer_list_overpaid() {
        with_offchain_ext(|| {
            let response = br#"{"data":[{"transaction_id":"tx1","from":"TBuyer","to":"TSeller","value":"15000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
            let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
            assert!(result.found);
            assert_eq!(result.actual_amount, Some(15_000_000));
            assert_eq!(result.amount_status, AmountStatus::Overpaid { excess: 5_000_000 });
        });
    }

    #[test]
    fn test_parse_transfer_list_underpaid() {
        with_offchain_ext(|| {
            let response = br#"{"data":[{"transaction_id":"tx1","from":"TBuyer","to":"TSeller","value":"7000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
            let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
            assert!(result.found);
            assert_eq!(result.actual_amount, Some(7_000_000));
            assert_eq!(result.amount_status, AmountStatus::Underpaid { shortage: 3_000_000 });
        });
    }

    #[test]
    fn test_parse_transfer_list_severely_underpaid() {
        with_offchain_ext(|| {
            let response = br#"{"data":[{"transaction_id":"tx1","from":"TBuyer","to":"TSeller","value":"3000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
            let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
            assert!(result.found);
            assert_eq!(result.actual_amount, Some(3_000_000));
            assert_eq!(result.amount_status, AmountStatus::SeverelyUnderpaid { shortage: 7_000_000 });
        });
    }

    #[test]
    fn test_parse_transfer_list_no_match() {
        with_offchain_ext(|| {
            let response = br#"{"data":[{"transaction_id":"tx1","from":"TWrongBuyer","to":"TSeller","value":"10000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
            let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
            assert!(!result.found);
            assert_eq!(result.amount_status, AmountStatus::Invalid);
        });
    }

    #[test]
    fn test_parse_transfer_list_empty_data() {
        with_offchain_ext(|| {
            let response = br#"{"data":[],"success":true}"#;
            let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
            assert!(!result.found);
        });
    }

    #[test]
    fn test_parse_transfer_list_api_failure() {
        with_offchain_ext(|| {
            let response = br#"{"success":false,"error":"rate limit"}"#;
            let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
            assert!(!result.found);
            assert!(result.error.is_some());
        });
    }

    #[test]
    fn test_parse_transfer_list_multi_entry_accumulates() {
        with_offchain_ext(|| {
            let response = br#"{"data":[{"transaction_id":"tx1","from":"TBuyer","to":"TSeller","value":"5000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}},{"transaction_id":"tx2","from":"TBuyer","to":"TSeller","value":"6000000","block_timestamp":1700000001000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
            let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
            assert!(result.found);
            assert_eq!(result.actual_amount, Some(11_000_000));
            assert_eq!(result.amount_status, AmountStatus::Overpaid { excess: 1_000_000 });
        });
    }

    #[test]
    fn test_parse_transfer_list_wrong_contract_ignored() {
        with_offchain_ext(|| {
            let response = br#"{"data":[{"transaction_id":"tx1","from":"TBuyer","to":"TSeller","value":"10000000","block_timestamp":1700000000000,"token_info":{"address":"TFakeContractAddress"}}],"success":true}"#;
            let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
            assert!(!result.found);
        });
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
        with_offchain_ext(|| {
            let response = br#"{"contractRet":"SUCCESS","contract_address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","to_address":"TSellerAddr12345678901234567890","confirmations":100,"amount":10000000}"#;
            let expected_to = b"TSellerAddr12345678901234567890";
            let result = parse_tron_response(response, expected_to, None, 10_000_000).unwrap();
            assert!(result.is_valid);
            assert_eq!(result.amount_status, AmountStatus::Exact);
        });
    }

    #[test]
    fn h1_parse_tron_response_address_mismatch() {
        with_offchain_ext(|| {
            let response = br#"{"contractRet":"SUCCESS","contract_address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","to_address":"TSellerAddr12345678901234567890","confirmations":100,"amount":10000000}"#;
            let wrong_to = b"TWrongAddr123456789012345678901";
            let result = parse_tron_response(response, wrong_to, None, 10_000_000).unwrap();
            assert!(!result.is_valid);
            assert_eq!(result.error, Some(b"Recipient address mismatch".to_vec()));
        });
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
        with_offchain_ext(|| {
            let response = br#"{"data":[{"transaction_id":"tx_6m","from":"TBuyer","to":"TSeller","value":"6000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}},{"transaction_id":"tx_5m","from":"TBuyer","to":"TSeller","value":"5000000","block_timestamp":1700000001000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}},{"transaction_id":"tx_7m","from":"TBuyer","to":"TSeller","value":"7000000","block_timestamp":1700000002000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
            let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
            assert!(result.found);
            assert_eq!(result.actual_amount, Some(18_000_000));
            assert_eq!(result.tx_hash, Some(b"tx_7m".to_vec()));
        });
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
        with_offchain_ext(|| {
            let response = br#"{"contractRet":"SUCCESS","contract_address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","to_address":"TSellerXXX","confirmations":5,"amount":10000000}"#;
            let result = parse_tron_response(response, b"TSellerXXX", None, 10_000_000).unwrap();
            assert!(!result.is_valid);
            assert_eq!(result.confirmations, 5);
            assert_eq!(result.error, Some(b"Insufficient confirmations".to_vec()));
        });
    }

    #[test]
    fn h1_parse_tron_response_not_usdt_contract() {
        with_offchain_ext(|| {
            let response = br#"{"contractRet":"SUCCESS","contract_address":"TFakeContract","to_address":"TSellerXXX","confirmations":100,"amount":10000000}"#;
            let result = parse_tron_response(response, b"TSellerXXX", None, 10_000_000).unwrap();
            assert!(!result.is_valid);
            assert_eq!(result.error, Some(b"Not a USDT TRC20 transaction".to_vec()));
        });
    }

    #[test]
    fn h1_parse_tron_response_tx_not_successful() {
        with_offchain_ext(|| {
            let response = br#"{"contractRet":"REVERT","to_address":"TSellerXXX"}"#;
            let result = parse_tron_response(response, b"TSellerXXX", None, 10_000_000).unwrap();
            assert!(!result.is_valid);
            assert_eq!(result.error, Some(b"Transaction not successful".to_vec()));
        });
    }

    // ==================== Phase 1 新增回归测试 ====================

    #[test]
    fn h6_parse_tron_response_sender_address_check() {
        with_offchain_ext(|| {
            let response = br#"{"contractRet":"SUCCESS","contract_address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","from":"TBuyerAddr1234567890","to_address":"TSellerAddr12345678901234567890","confirmations":100,"amount":10000000}"#;
            let result = parse_tron_response(
                response, b"TSellerAddr12345678901234567890", Some(b"TBuyerAddr1234567890"), 10_000_000
            ).unwrap();
            assert!(result.is_valid);
        });
    }

    #[test]
    fn h6_parse_tron_response_sender_mismatch() {
        with_offchain_ext(|| {
            let response = br#"{"contractRet":"SUCCESS","contract_address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","from":"TBuyerAddr1234567890","to_address":"TSellerAddr12345678901234567890","confirmations":100,"amount":10000000}"#;
            let result = parse_tron_response(
                response, b"TSellerAddr12345678901234567890", Some(b"TWrongSender1234567890"), 10_000_000
            ).unwrap();
            assert!(!result.is_valid);
            assert_eq!(result.error, Some(b"Sender address mismatch".to_vec()));
        });
    }

    #[test]
    fn h6_parse_tron_response_no_from_check_when_none() {
        with_offchain_ext(|| {
            let response = br#"{"contractRet":"SUCCESS","contract_address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","from":"TRandomSender","to_address":"TSellerAddr12345678901234567890","confirmations":100,"amount":10000000}"#;
            let result = parse_tron_response(
                response, b"TSellerAddr12345678901234567890", None, 10_000_000
            ).unwrap();
            assert!(result.is_valid);
        });
    }

    #[test]
    fn h7_parse_tron_response_expected_zero_returns_invalid() {
        with_offchain_ext(|| {
            let response = br#"{"contractRet":"SUCCESS","contract_address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","to_address":"TSellerAddr12345678901234567890","confirmations":100,"amount":10000000}"#;
            let result = parse_tron_response(
                response, b"TSellerAddr12345678901234567890", None, 0
            ).unwrap();
            assert!(!result.is_valid);
            assert_eq!(result.amount_status, AmountStatus::Invalid);
        });
    }

    #[test]
    fn h8_add_endpoint_rejects_http() {
        // URL format validation happens before any offchain storage access
        assert_eq!(add_endpoint("http://api.example.com"), Err(VerificationError::InvalidEndpointUrl("Endpoint must use HTTPS")));
    }

    #[test]
    fn h8_add_endpoint_rejects_short_url() {
        assert_eq!(add_endpoint("https://x"), Err(VerificationError::InvalidEndpointUrl("Endpoint URL length must be 10-256 characters")));
    }

    #[test]
    fn h8_add_endpoint_rejects_whitespace() {
        assert_eq!(add_endpoint("https://api.example .com"), Err(VerificationError::InvalidEndpointUrl("Endpoint URL must not contain whitespace")));
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
        with_offchain_ext(|| {
            let response = b"this is not json";
            let result = parse_tron_response(response, b"TSeller", None, 10_000_000);
            assert_eq!(result, Err(VerificationError::InvalidJson));
        });
    }

    #[test]
    fn c1_parse_transfer_list_rejects_invalid_json() {
        with_offchain_ext(|| {
            let response = b"not json at all";
            let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0);
            assert_eq!(result, Err(VerificationError::InvalidJson));
        });
    }

    #[test]
    fn c2_amount_extracted_by_field_name() {
        with_offchain_ext(|| {
            let response = br#"{"contractRet":"SUCCESS","contract_address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","to_address":"TSellerXXX","confirmations":100,"amount":5000000}"#;
            let result = parse_tron_response(response, b"TSellerXXX", None, 5_000_000).unwrap();
            assert!(result.is_valid);
            assert_eq!(result.actual_amount, Some(5_000_000));
            assert_eq!(result.amount_status, AmountStatus::Exact);
        });
    }

    #[test]
    fn m5_transfer_list_handles_braces_in_strings() {
        with_offchain_ext(|| {
            let response = br#"{"data":[{"transaction_id":"tx{special}","from":"TBuyer","to":"TSeller","value":"10000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
            let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
            assert!(result.found);
            assert_eq!(result.actual_amount, Some(10_000_000));
            assert_eq!(result.tx_hash, Some(b"tx{special}".to_vec()));
        });
    }

    #[test]
    fn c1_transfer_list_from_exact_match() {
        with_offchain_ext(|| {
            let response = br#"{"data":[{"transaction_id":"tx1","from":"TBuyerExtra","to":"TSeller","value":"10000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
            let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
            assert!(!result.found);
        });
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

    // ==================== Phase 3 新增回归测试 (H1+H2+H3+M1) ====================

    #[test]
    fn h1_get_recent_audit_logs_zero_count_no_panic() {
        with_offchain_ext(|| {
            // H1修复: max_count==0 应返回空 Vec，不应 panic
            let logs = get_recent_audit_logs(0);
            assert!(logs.is_empty());
        });
    }

    #[test]
    fn h1_get_recent_audit_logs_zero_counter_returns_empty() {
        with_offchain_ext(|| {
            // counter==0 且 max_count>0 也应安全返回空
            let logs = get_recent_audit_logs(10);
            assert!(logs.is_empty());
        });
    }

    #[test]
    fn h2_merge_transfer_results_cumulative_amount_status() {
        // H2修复: 多页合并后 amount_status 应基于累计总额重新计算
        // 场景: expected=10M, 第一页5M(Underpaid), 第二页6M → 累计11M(Overpaid)
        let mut combined = TransferSearchResult::default();

        let page1 = TransferSearchResult {
            found: true,
            actual_amount: Some(5_000_000),
            amount_status: AmountStatus::SeverelyUnderpaid { shortage: 5_000_000 },
            remaining_amount: Some(5_000_000),
            matched_transfers: vec![MatchedTransfer {
                tx_hash: b"tx1".to_vec(),
                amount: 5_000_000,
                block_timestamp: 1700000000000,
            }],
            tx_hash: Some(b"tx1".to_vec()),
            block_timestamp: Some(1700000000000),
            ..Default::default()
        };

        let page2 = TransferSearchResult {
            found: true,
            actual_amount: Some(6_000_000),
            amount_status: AmountStatus::Underpaid { shortage: 4_000_000 },
            remaining_amount: Some(4_000_000),
            matched_transfers: vec![MatchedTransfer {
                tx_hash: b"tx2".to_vec(),
                amount: 6_000_000,
                block_timestamp: 1700000001000,
            }],
            tx_hash: Some(b"tx2".to_vec()),
            block_timestamp: Some(1700000001000),
            ..Default::default()
        };

        let expected_amount = 10_000_000u64;
        merge_transfer_results(&mut combined, &page1, expected_amount);
        merge_transfer_results(&mut combined, &page2, expected_amount);

        assert!(combined.found);
        assert_eq!(combined.actual_amount, Some(11_000_000));
        // 修复前: amount_status 会是 page2 的 Underpaid (基于 page2 单页 6M vs 10M)
        // 修复后: amount_status 基于累计 11M vs 10M = Overpaid
        assert_eq!(combined.amount_status, AmountStatus::Overpaid { excess: 1_000_000 });
        assert_eq!(combined.remaining_amount, None); // Overpaid 无需补付
    }

    #[test]
    fn h2_merge_single_page_status_unchanged() {
        // 单页场景: 合并行为与修复前一致
        let mut combined = TransferSearchResult::default();
        let page = TransferSearchResult {
            found: true,
            actual_amount: Some(10_000_000),
            amount_status: AmountStatus::Exact,
            matched_transfers: vec![MatchedTransfer {
                tx_hash: b"tx1".to_vec(),
                amount: 10_000_000,
                block_timestamp: 1700000000000,
            }],
            tx_hash: Some(b"tx1".to_vec()),
            block_timestamp: Some(1700000000000),
            ..Default::default()
        };
        merge_transfer_results(&mut combined, &page, 10_000_000);
        assert_eq!(combined.amount_status, AmountStatus::Exact);
    }

    #[test]
    fn m1_remove_endpoint_cleans_api_key_and_priority() {
        with_offchain_ext(|| {
            // 添加端点
            let ep = "https://api.trongrid.io";
            add_endpoint(ep).unwrap();
            set_api_key(ep, "my-key");
            set_endpoint_priority_boost(ep, 10);

            // 验证已添加
            let config = get_endpoint_config();
            assert!(config.api_keys.iter().any(|(e, _)| e == ep));
            assert!(config.priority_boosts.iter().any(|(e, _)| e == ep));

            // 移除端点
            remove_endpoint(ep);

            // M1修复: api_keys 和 priority_boosts 也应被清理
            let config2 = get_endpoint_config();
            assert!(!config2.endpoints.contains(&String::from(ep)));
            assert!(!config2.api_keys.iter().any(|(e, _)| e == ep));
            assert!(!config2.priority_boosts.iter().any(|(e, _)| e == ep));
        });
    }

    #[test]
    fn m1_remove_nonexistent_endpoint_is_noop() {
        with_offchain_ext(|| {
            let config_before = get_endpoint_config();
            remove_endpoint("https://nonexistent.example.com");
            let config_after = get_endpoint_config();
            assert_eq!(config_before.endpoints.len(), config_after.endpoints.len());
        });
    }
}
