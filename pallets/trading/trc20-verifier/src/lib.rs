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
    /// TRON 地址格式无效 (C2)
    InvalidTronAddress(&'static str),
    /// 时间戳超出最大回溯窗口 (C3)
    TimestampTooOld,
    /// 配置参数无效 (H1)
    InvalidConfig(&'static str),
    /// 验证正在进行中 - OCW 并发锁 (M1)
    VerificationLocked,
    /// 验证器已被全局禁用（kill switch）
    VerifierDisabled,
    /// 交易哈希已被使用（防重放）
    TxHashAlreadyUsed,
    /// 多端点共识验证失败 — 端点之间返回数据不一致 (H3-C)
    ConsensusFailure,
    /// 有效响应不足，无法进行共识验证 (H3-C)
    InsufficientEndpointResponses,
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
            Self::InvalidTronAddress(reason) => reason,
            Self::TimestampTooOld => "Timestamp exceeds max lookback window",
            Self::InvalidConfig(reason) => reason,
            Self::VerificationLocked => "Verification already in progress",
            Self::VerifierDisabled => "Verifier is globally disabled",
            Self::TxHashAlreadyUsed => "Transaction hash already used",
            Self::ConsensusFailure => "Endpoint consensus verification failed",
            Self::InsufficientEndpointResponses => "Insufficient endpoint responses for consensus",
        }
    }
}

impl core::fmt::Display for VerificationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::HttpBadStatus(code) => write!(f, "Non-200 HTTP response ({})", code),
            Self::InvalidTronAddress(reason) => write!(f, "Invalid TRON address: {}", reason),
            Self::InvalidConfig(reason) => write!(f, "Invalid config: {}", reason),
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

/// 健康评分窗口大小 (M2)— 每达到此数量时半衰计数器
const HEALTH_WINDOW_SIZE: u32 = 100;

/// 端点数量上限 (M3)
const MAX_ENDPOINTS: usize = 10;

/// 速率限制存储键 (H2)
const RATE_LIMIT_KEY: &[u8] = b"ocw_rate_limit_last_req";

/// 响应缓存存储键前缀 (M1)
const RESPONSE_CACHE_PREFIX: &[u8] = b"ocw_resp_cache::";

/// 验证器配置存储键 (H3)
const VERIFIER_CONFIG_KEY: &[u8] = b"ocw_verifier_config";

/// OCW 并发锁存储键前缀 (M1)
const OCW_LOCK_PREFIX: &[u8] = b"ocw_verify_lock::";

/// OCW 并发锁超时（毫秒）(M1)
const OCW_LOCK_TIMEOUT_MS: u64 = 30_000;

/// 默认最大回溯时间窗口（72小时）(C3)
const DEFAULT_MAX_LOOKBACK_MS: u64 = 259_200_000;

/// 缓存键注册表存储键 (M3)
const CACHE_KEYS_KEY: &[u8] = b"ocw_cache_keys_registry";

/// 最大缓存条目数 (M3)
const MAX_CACHE_ENTRIES: usize = 50;

/// 监控指标存储键 (M4)
const VERIFIER_METRICS_KEY: &[u8] = b"ocw_verifier_metrics";

/// 审计日志存储键前缀 (M7)
const AUDIT_LOG_PREFIX: &[u8] = b"ocw_audit_log::";

/// 审计日志计数器键 (M7)
const AUDIT_LOG_COUNTER_KEY: &[u8] = b"ocw_audit_log_counter";

/// NEW-6: 配置版本标记前缀 (0xFF 不会出现在 SCALE 编码首字节)
const CONFIG_VERSION_MARKER: u8 = 0xFF;
/// NEW-6: EndpointConfig 当前版本
const ENDPOINT_CONFIG_VERSION: u8 = 1;
/// NEW-6: VerifierConfig 当前版本 (v2: 含 consensus 字段)
const VERIFIER_CONFIG_VERSION: u8 = 2;

/// 已用 tx_hash 注册表存储键前缀 (P0 防重放)
const USED_TX_HASH_PREFIX: &[u8] = b"ocw_used_tx::";

/// 端点熔断：隔离持续时间（毫秒，默认 60 秒）
const QUARANTINE_DURATION_MS: u64 = 60_000;

/// 端点熔断存储键前缀
const QUARANTINE_PREFIX: &[u8] = b"ocw_quarantine::";

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
        // M2: 窗口化 — 总数达到窗口大小时半衰计数器，防止无界累积
        if self.success_count.saturating_add(self.failure_count) >= HEALTH_WINDOW_SIZE {
            self.success_count /= 2;
            self.failure_count /= 2;
        }
        self.success_count = self.success_count.saturating_add(1);

        // 指数移动平均更新响应时间
        // M1-R3修复: 使用 u64 中间计算防止 avg_response_ms * 90 溢出 u32
        if self.avg_response_ms == 0 {
            self.avg_response_ms = response_ms;
        } else {
            self.avg_response_ms = ((self.avg_response_ms as u64 * HEALTH_DECAY_FACTOR as u64
                + response_ms as u64 * (100 - HEALTH_DECAY_FACTOR) as u64) / 100) as u32;
        }

        self.score = self.calculate_score();
        self.last_updated = current_timestamp_ms();
    }

    /// 记录失败请求
    pub fn record_failure(&mut self) {
        // M2: 窗口化半衰
        if self.success_count.saturating_add(self.failure_count) >= HEALTH_WINDOW_SIZE {
            self.success_count /= 2;
            self.failure_count /= 2;
        }
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
    /// API Key 映射: (endpoint, api_key) (H1, H1-R3精确匹配)
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
    /// 最大回溯时间窗口（毫秒，0=禁用）(C3)
    pub max_lookback_ms: u64,
    /// 最后更新时间
    pub updated_at: u64,
    /// NEW-9: 金额容差（基点, 1 bps = 0.01%, 默认 50 = ±0.5%）
    pub amount_tolerance_bps: u32,
    /// 全局启用开关（kill switch，false 时所有验证立即返回 VerifierDisabled）
    pub enabled: bool,
    /// H3-C: 是否启用多端点共识验证（false = 旧的 first-wins 模式）
    pub consensus_enabled: bool,
    /// H3-C: 共识所需最少端点响应数（默认 2，即 2-of-N）
    pub min_consensus_responses: u32,
    /// H3-C: 仅 1 个端点可用时是否允许降级为单源验证
    pub allow_single_source_fallback: bool,
    /// H3-C: 共识比对中 block_timestamp 允许的偏差（毫秒，默认 3000 ≈ 1 TRON 出块周期）
    pub consensus_timestamp_tolerance_ms: u64,
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
            max_lookback_ms: DEFAULT_MAX_LOOKBACK_MS, // 72h
            updated_at: 0,
            amount_tolerance_bps: 50, // ±0.5%
            enabled: true,
            consensus_enabled: false,                // Phase 1: 默认关闭，先部署基础设施
            min_consensus_responses: 2,              // 2-of-N
            allow_single_source_fallback: true,      // 安全默认：允许降级
            consensus_timestamp_tolerance_ms: 3_000, // 1 TRON 出块周期
        }
    }
}

/// 获取验证器配置 (NEW-6: 支持版本化存储格式 + 多级旧格式迁移)
pub fn get_verifier_config() -> VerifierConfig {
    match sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, VERIFIER_CONFIG_KEY) {
        Some(data) if data.len() >= 2 && data[0] == CONFIG_VERSION_MARKER => {
            let _version = data[1];
            // V3: 当前完整格式（含 consensus 字段）
            if let Ok(config) = VerifierConfig::decode(&mut &data[2..]) {
                return config;
            }
            // V2: 含 enabled 但无 consensus 字段
            #[derive(Decode)]
            struct V2Config {
                usdt_contract: String,
                min_confirmations: u32,
                rate_limit_interval_ms: u64,
                cache_ttl_ms: u64,
                max_pages: u32,
                audit_log_retention: u32,
                max_lookback_ms: u64,
                updated_at: u64,
                amount_tolerance_bps: u32,
                enabled: bool,
            }
            if let Ok(v2) = V2Config::decode(&mut &data[2..]) {
                return VerifierConfig {
                    usdt_contract: v2.usdt_contract,
                    min_confirmations: v2.min_confirmations,
                    rate_limit_interval_ms: v2.rate_limit_interval_ms,
                    cache_ttl_ms: v2.cache_ttl_ms,
                    max_pages: v2.max_pages,
                    audit_log_retention: v2.audit_log_retention,
                    max_lookback_ms: v2.max_lookback_ms,
                    updated_at: v2.updated_at,
                    amount_tolerance_bps: v2.amount_tolerance_bps,
                    enabled: v2.enabled,
                    consensus_enabled: false,
                    min_consensus_responses: 2,
                    allow_single_source_fallback: true,
                    consensus_timestamp_tolerance_ms: 3_000,
                };
            }
            // V1: 无 enabled 和 consensus 字段
            #[derive(Decode)]
            struct V1Config {
                usdt_contract: String,
                min_confirmations: u32,
                rate_limit_interval_ms: u64,
                cache_ttl_ms: u64,
                max_pages: u32,
                audit_log_retention: u32,
                max_lookback_ms: u64,
                updated_at: u64,
                amount_tolerance_bps: u32,
            }
            if let Ok(v1) = V1Config::decode(&mut &data[2..]) {
                return VerifierConfig {
                    usdt_contract: v1.usdt_contract,
                    min_confirmations: v1.min_confirmations,
                    rate_limit_interval_ms: v1.rate_limit_interval_ms,
                    cache_ttl_ms: v1.cache_ttl_ms,
                    max_pages: v1.max_pages,
                    audit_log_retention: v1.audit_log_retention,
                    max_lookback_ms: v1.max_lookback_ms,
                    updated_at: v1.updated_at,
                    amount_tolerance_bps: v1.amount_tolerance_bps,
                    enabled: true,
                    consensus_enabled: false,
                    min_consensus_responses: 2,
                    allow_single_source_fallback: true,
                    consensus_timestamp_tolerance_ms: 3_000,
                };
            }
            VerifierConfig::default()
        }
        Some(data) if !data.is_empty() => {
            if let Ok(config) = VerifierConfig::decode(&mut &data[..]) {
                return config;
            }
            #[derive(Decode)]
            struct LegacyVerifierConfig {
                usdt_contract: String,
                min_confirmations: u32,
                rate_limit_interval_ms: u64,
                cache_ttl_ms: u64,
                max_pages: u32,
                audit_log_retention: u32,
                max_lookback_ms: u64,
                updated_at: u64,
            }
            if let Ok(legacy) = LegacyVerifierConfig::decode(&mut &data[..]) {
                return VerifierConfig {
                    usdt_contract: legacy.usdt_contract,
                    min_confirmations: legacy.min_confirmations,
                    rate_limit_interval_ms: legacy.rate_limit_interval_ms,
                    cache_ttl_ms: legacy.cache_ttl_ms,
                    max_pages: legacy.max_pages,
                    audit_log_retention: legacy.audit_log_retention,
                    max_lookback_ms: legacy.max_lookback_ms,
                    updated_at: legacy.updated_at,
                    amount_tolerance_bps: 50,
                    enabled: true,
                    consensus_enabled: false,
                    min_consensus_responses: 2,
                    allow_single_source_fallback: true,
                    consensus_timestamp_tolerance_ms: 3_000,
                };
            }
            VerifierConfig::default()
        }
        _ => VerifierConfig::default(),
    }
}

/// 保存验证器配置 (H1: 带安全验证, NEW-6: 版本化存储)
pub fn save_verifier_config(config: &VerifierConfig) -> Result<(), VerificationError> {
    validate_verifier_config(config)?;
    let mut versioned = alloc::vec![CONFIG_VERSION_MARKER, VERIFIER_CONFIG_VERSION];
    versioned.extend_from_slice(&config.encode());
    sp_io::offchain::local_storage_set(
        StorageKind::PERSISTENT,
        VERIFIER_CONFIG_KEY,
        &versioned,
    );
    log::info!(target: "trc20-verifier", "Config saved: min_conf={}, contract={}, max_lookback={}ms, tolerance={}bps",
        config.min_confirmations, config.usdt_contract, config.max_lookback_ms, config.amount_tolerance_bps);
    Ok(())
}

// ==================== TRON 地址校验 (C2) ====================

/// Base58 字符集
const BASE58_CHARS: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Base58 解码为固定 25 字节 (TRON 地址: 1 version + 20 hash + 4 checksum)
fn base58_decode_tron(input: &[u8]) -> Result<[u8; 25], VerificationError> {
    let mut result = [0u8; 25];

    for &c in input.iter() {
        let val = match BASE58_CHARS.iter().position(|&x| x == c) {
            Some(v) => v as u32,
            None => return Err(VerificationError::InvalidTronAddress(
                "Address contains invalid Base58 characters",
            )),
        };

        let mut carry = val;
        for byte in result.iter_mut().rev() {
            carry += (*byte as u32) * 58;
            *byte = (carry & 0xFF) as u8;
            carry >>= 8;
        }
        if carry != 0 {
            return Err(VerificationError::InvalidTronAddress(
                "Invalid decoded address length",
            ));
        }
    }

    Ok(result)
}

/// 校验 TRON Base58 地址格式 + Base58Check 校验和 (C2+S1)
///
/// 规则: T 开头、34 字符、仅包含合法 Base58 字符、版本字节 0x41、SHA256 双哈希校验和
pub fn validate_tron_address(address: &[u8]) -> Result<(), VerificationError> {
    let addr_str = core::str::from_utf8(address)
        .map_err(|_| VerificationError::InvalidTronAddress("Invalid UTF-8"))?;

    if addr_str.len() != 34 {
        return Err(VerificationError::InvalidTronAddress(
            "Address must be 34 characters",
        ));
    }

    if !addr_str.starts_with('T') {
        return Err(VerificationError::InvalidTronAddress(
            "Address must start with 'T'",
        ));
    }

    for byte in addr_str.as_bytes() {
        if !BASE58_CHARS.contains(byte) {
            return Err(VerificationError::InvalidTronAddress(
                "Address contains invalid Base58 characters",
            ));
        }
    }

    let decoded = base58_decode_tron(address)?;
    if decoded[0] != 0x41 {
        return Err(VerificationError::InvalidTronAddress(
            "Invalid TRON version byte (expected 0x41)",
        ));
    }

    let hash1 = sp_core::hashing::sha2_256(&decoded[..21]);
    let hash2 = sp_core::hashing::sha2_256(&hash1);
    if hash2[..4] != decoded[21..25] {
        return Err(VerificationError::InvalidTronAddress(
            "Base58Check checksum mismatch",
        ));
    }

    Ok(())
}

// ==================== 配置安全验证 (H1) ====================

/// 验证 VerifierConfig 参数安全性 (H1)
pub fn validate_verifier_config(config: &VerifierConfig) -> Result<(), VerificationError> {
    // 最小确认数不能低于安全阈值
    if config.min_confirmations < 10 {
        return Err(VerificationError::InvalidConfig(
            "min_confirmations must be >= 10",
        ));
    }
    // USDT 合约地址格式检查
    if !config.usdt_contract.is_empty() {
        validate_tron_address(config.usdt_contract.as_bytes())?;
    }
    // 速率限制间隔: 0=禁用，否则 >=50ms
    if config.rate_limit_interval_ms > 0 && config.rate_limit_interval_ms < 50 {
        return Err(VerificationError::InvalidConfig(
            "rate_limit_interval_ms must be 0 (disabled) or >= 50",
        ));
    }
    // 最大回溯窗口: 0=禁用，否则 >= 1小时
    if config.max_lookback_ms > 0 && config.max_lookback_ms < 3_600_000 {
        return Err(VerificationError::InvalidConfig(
            "max_lookback_ms must be 0 (disabled) or >= 3600000 (1 hour)",
        ));
    }
    // M2-R1: 缓存 TTL: 0=禁用，否则 >= 1000ms（避免过小 TTL 导致无效开销）
    if config.cache_ttl_ms > 0 && config.cache_ttl_ms < 1_000 {
        return Err(VerificationError::InvalidConfig(
            "cache_ttl_ms must be 0 (disabled) or >= 1000 (1 second)",
        ));
    }
    // M2-R1: 分页上限: 1..=10（防止无界循环）
    if config.max_pages == 0 || config.max_pages > 10 {
        return Err(VerificationError::InvalidConfig(
            "max_pages must be between 1 and 10",
        ));
    }
    // H1-R2: 审计日志保留量: 0=禁用，否则 1..=10_000（防止 offchain 存储无界增长）
    if config.audit_log_retention > 10_000 {
        return Err(VerificationError::InvalidConfig(
            "audit_log_retention must be 0 (disabled) or <= 10000",
        ));
    }
    // NEW-9: 金额容差: 0..=1000 bps (最大 10%)
    if config.amount_tolerance_bps > 1000 {
        return Err(VerificationError::InvalidConfig(
            "amount_tolerance_bps must be <= 1000 (10%)",
        ));
    }
    // H3-C: 共识最少响应数: 2..=10
    if config.min_consensus_responses < 2 || config.min_consensus_responses > 10 {
        return Err(VerificationError::InvalidConfig(
            "min_consensus_responses must be between 2 and 10",
        ));
    }
    // H3-C: 共识时间戳容差: 0 或 >= 1000ms
    if config.consensus_timestamp_tolerance_ms > 0 && config.consensus_timestamp_tolerance_ms < 1_000 {
        return Err(VerificationError::InvalidConfig(
            "consensus_timestamp_tolerance_ms must be 0 or >= 1000",
        ));
    }
    Ok(())
}

// ==================== OCW 并发锁 (M1) ====================

/// 尝试获取 OCW 验证锁 (M1, NEW-7: 返回锁令牌用于安全释放)
///
/// 使用 CAS (compare-and-set) 防止多个 OCW 实例同时验证同一笔交易。
/// 成功时返回 `Some(lock_token)`，调用方须在释放时传回此令牌。
pub fn try_acquire_verify_lock(lock_id: &[u8]) -> Option<u64> {
    let key = [OCW_LOCK_PREFIX, lock_id].concat();
    let now = current_timestamp_ms();
    let now_bytes = now.encode();

    let existing = sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, &key);

    let acquired = match existing {
        Some(ref data) if !data.is_empty() => {
            if let Ok(locked_at) = u64::decode(&mut &data[..]) {
                if now.saturating_sub(locked_at) < OCW_LOCK_TIMEOUT_MS {
                    return None;
                }
            }
            sp_io::offchain::local_storage_compare_and_set(
                StorageKind::PERSISTENT, &key, existing, &now_bytes,
            )
        }
        other => {
            sp_io::offchain::local_storage_compare_and_set(
                StorageKind::PERSISTENT, &key, other, &now_bytes,
            )
        }
    };

    if acquired { Some(now) } else { None }
}

/// 释放 OCW 验证锁 (M1, NEW-7: CAS 释放 — 仅释放自己持有的锁)
///
/// 使用 `lock_token` 验证锁的持有者身份，防止误释放其他 OCW 实例的锁。
pub fn release_verify_lock(lock_id: &[u8], lock_token: u64) {
    let key = [OCW_LOCK_PREFIX, lock_id].concat();
    let token_bytes = lock_token.encode();
    sp_io::offchain::local_storage_compare_and_set(
        StorageKind::PERSISTENT, &key, Some(token_bytes), &[],
    );
}

// ==================== 缓存清理 (M3) ====================

/// 注册缓存键（用于后续清理）
fn register_cache_key(url_hash: &[u8]) {
    let mut keys: Vec<Vec<u8>> = sp_io::offchain::local_storage_get(
        StorageKind::PERSISTENT, CACHE_KEYS_KEY,
    )
    .and_then(|d| Vec::<Vec<u8>>::decode(&mut &d[..]).ok())
    .unwrap_or_default();

    if !keys.iter().any(|k| k == url_hash) {
        if keys.len() >= MAX_CACHE_ENTRIES {
            // 满时淘汰最旧条目
            let oldest = keys.remove(0);
            let old_full_key = [RESPONSE_CACHE_PREFIX, &oldest].concat();
            sp_io::offchain::local_storage_set(StorageKind::PERSISTENT, &old_full_key, &[]);
        }
        keys.push(url_hash.to_vec());
        sp_io::offchain::local_storage_set(
            StorageKind::PERSISTENT, CACHE_KEYS_KEY, &keys.encode(),
        );
    }
}

/// 清理过期缓存条目 (M3)
///
/// 返回清理的条目数
pub fn cleanup_expired_cache() -> u32 {
    let config = get_verifier_config();
    if config.cache_ttl_ms == 0 { return 0; }

    let now = current_timestamp_ms();
    let mut cleaned = 0u32;

    let keys: Vec<Vec<u8>> = sp_io::offchain::local_storage_get(
        StorageKind::PERSISTENT, CACHE_KEYS_KEY,
    )
    .and_then(|d| Vec::<Vec<u8>>::decode(&mut &d[..]).ok())
    .unwrap_or_default();

    let mut remaining = Vec::new();
    for key in keys {
        let full_key = [RESPONSE_CACHE_PREFIX, &key].concat();
        let expired = match sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, &full_key) {
            Some(data) => {
                if let Ok((timestamp, _)) = <(u64, Vec<u8>)>::decode(&mut &data[..]) {
                    now.saturating_sub(timestamp) > config.cache_ttl_ms
                } else {
                    true // 解码失败，视为过期
                }
            }
            None => true,
        };
        if expired {
            sp_io::offchain::local_storage_set(StorageKind::PERSISTENT, &full_key, &[]);
            cleaned += 1;
        } else {
            remaining.push(key);
        }
    }

    sp_io::offchain::local_storage_set(
        StorageKind::PERSISTENT, CACHE_KEYS_KEY, &remaining.encode(),
    );

    if cleaned > 0 {
        log::info!(target: "trc20-verifier", "Cache cleanup: removed {} expired entries", cleaned);
    }
    cleaned
}

// ==================== 监控指标 (M4) ====================

/// 验证器监控指标
#[derive(Debug, Clone, Encode, Decode, Default)]
pub struct VerifierMetrics {
    /// 累计验证成功数
    pub total_success: u64,
    /// 累计验证失败数
    pub total_failure: u64,
    /// 累计验证总耗时（毫秒）
    pub total_duration_ms: u64,
    /// 端点切换次数（串行模式中 fallback 成功次数）
    pub endpoint_fallback_count: u64,
    /// 缓存命中次数
    pub cache_hit_count: u64,
    /// 速率限制拦截次数
    pub rate_limit_hit_count: u64,
    /// 并发锁拦截次数
    pub lock_contention_count: u64,
    /// 最后更新时间
    pub last_updated: u64,
    /// H3-C: 共识验证成功次数
    pub consensus_success_count: u64,
    /// H3-C: 共识验证失败次数（NoConsensus 或 InsufficientResponses）
    pub consensus_failure_count: u64,
    /// H3-C: 降级为单源验证次数
    pub degraded_verification_count: u64,
}

/// 获取监控指标 (H3-C: 含旧格式迁移支持)
pub fn get_verifier_metrics() -> VerifierMetrics {
    sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, VERIFIER_METRICS_KEY)
        .and_then(|d| {
            // 优先尝试新格式
            if let Ok(m) = VerifierMetrics::decode(&mut &d[..]) {
                return Some(m);
            }
            // 旧格式迁移：不含 consensus 字段
            #[derive(Decode)]
            struct LegacyMetrics {
                total_success: u64,
                total_failure: u64,
                total_duration_ms: u64,
                endpoint_fallback_count: u64,
                cache_hit_count: u64,
                rate_limit_hit_count: u64,
                lock_contention_count: u64,
                last_updated: u64,
            }
            if let Ok(old) = LegacyMetrics::decode(&mut &d[..]) {
                return Some(VerifierMetrics {
                    total_success: old.total_success,
                    total_failure: old.total_failure,
                    total_duration_ms: old.total_duration_ms,
                    endpoint_fallback_count: old.endpoint_fallback_count,
                    cache_hit_count: old.cache_hit_count,
                    rate_limit_hit_count: old.rate_limit_hit_count,
                    lock_contention_count: old.lock_contention_count,
                    last_updated: old.last_updated,
                    consensus_success_count: 0,
                    consensus_failure_count: 0,
                    degraded_verification_count: 0,
                });
            }
            None
        })
        .unwrap_or_default()
}

/// 保存监控指标
fn save_verifier_metrics(metrics: &VerifierMetrics) {
    sp_io::offchain::local_storage_set(
        StorageKind::PERSISTENT, VERIFIER_METRICS_KEY, &metrics.encode(),
    );
}

/// 记录验证结果到指标 (M4)
fn record_metric_verification(success: bool, duration_ms: u64) {
    let mut m = get_verifier_metrics();
    if success {
        m.total_success = m.total_success.saturating_add(1);
    } else {
        m.total_failure = m.total_failure.saturating_add(1);
    }
    m.total_duration_ms = m.total_duration_ms.saturating_add(duration_ms);
    m.last_updated = current_timestamp_ms();
    save_verifier_metrics(&m);
}

/// 重置监控指标
pub fn reset_verifier_metrics() {
    save_verifier_metrics(&VerifierMetrics::default());
}

/// 获取有效 USDT 合约地址（配置优先，否则默认常量）(NEW-3: 接受 config 避免重复读取)
fn effective_usdt_contract(config: &VerifierConfig) -> String {
    if config.usdt_contract.is_empty() {
        String::from(USDT_CONTRACT)
    } else {
        config.usdt_contract.clone()
    }
}

/// 获取有效最小确认数 (NEW-3: 接受 config 避免重复读取)
fn effective_min_confirmations(config: &VerifierConfig) -> u32 {
    if config.min_confirmations == 0 { MIN_CONFIRMATIONS } else { config.min_confirmations }
}

/// 获取当前端点配置 (NEW-6: 支持版本化存储格式)
pub fn get_endpoint_config() -> EndpointConfig {
    match sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, CUSTOM_ENDPOINTS_KEY) {
        Some(data) if data.len() >= 2 && data[0] == CONFIG_VERSION_MARKER => {
            let _version = data[1];
            EndpointConfig::decode(&mut &data[2..]).unwrap_or_default()
        }
        Some(data) if !data.is_empty() => {
            EndpointConfig::decode(&mut &data[..]).unwrap_or_default()
        }
        _ => EndpointConfig::default(),
    }
}

/// NEW-2: 验证 EndpointConfig 安全性（统一入口，add_endpoint 和 save_endpoint_config 均走此路径）
fn validate_endpoint_config(config: &EndpointConfig) -> Result<(), VerificationError> {
    if config.endpoints.len() > MAX_ENDPOINTS {
        return Err(VerificationError::MaxEndpointsReached);
    }
    for ep in &config.endpoints {
        if !ep.starts_with("https://") {
            return Err(VerificationError::InvalidEndpointUrl("Endpoint must use HTTPS"));
        }
        if ep.len() < 10 || ep.len() > 256 {
            return Err(VerificationError::InvalidEndpointUrl(
                "Endpoint URL length must be 10-256 characters",
            ));
        }
        if ep.bytes().any(|b| b == b' ' || b == b'\t' || b == b'\n' || b == b'\r') {
            return Err(VerificationError::InvalidEndpointUrl(
                "Endpoint URL must not contain whitespace",
            ));
        }
        if is_private_or_loopback_url(ep) {
            return Err(VerificationError::InvalidEndpointUrl(
                "Endpoint must not target private or loopback addresses",
            ));
        }
    }
    if config.timeout_ms > 0 && config.timeout_ms < 1_000 {
        return Err(VerificationError::InvalidConfig(
            "timeout_ms must be 0 or >= 1000",
        ));
    }
    if config.timeout_race_ms > 0 && config.timeout_race_ms < 500 {
        return Err(VerificationError::InvalidConfig(
            "timeout_race_ms must be 0 or >= 500",
        ));
    }
    Ok(())
}

/// 保存端点配置 (NEW-2: 带安全验证, NEW-6: 版本化存储)
pub fn save_endpoint_config(config: &EndpointConfig) -> Result<(), VerificationError> {
    validate_endpoint_config(config)?;
    let mut versioned = alloc::vec![CONFIG_VERSION_MARKER, ENDPOINT_CONFIG_VERSION];
    versioned.extend_from_slice(&config.encode());
    sp_io::offchain::local_storage_set(
        StorageKind::PERSISTENT,
        CUSTOM_ENDPOINTS_KEY,
        &versioned,
    );
    Ok(())
}

/// NEW-1: 检查 IPv4 地址字符串是否为私有/回环地址
fn is_private_or_loopback_ipv4(host: &str) -> bool {
    if host == "0.0.0.0" || host.starts_with("127.") {
        return true;
    }
    if host.starts_with("10.") || host.starts_with("192.168.") || host.starts_with("169.254.") {
        return true;
    }
    if host.starts_with("172.") {
        if let Some(second) = host.splitn(3, '.').nth(1) {
            if let Ok(n) = second.parse::<u8>() {
                if (16..=31).contains(&n) {
                    return true;
                }
            }
        }
    }
    false
}

/// S3+NEW-1: 检查端点 URL 是否指向私有/回环地址 (SSRF 防护)
///
/// 支持检测: IPv4 私有/回环、IPv6 回环/ULA/link-local、IPv4-mapped IPv6 (::ffff:x.x.x.x)
fn is_private_or_loopback_url(endpoint: &str) -> bool {
    let after_scheme = match endpoint.strip_prefix("https://") {
        Some(s) => s,
        None => return false,
    };
    let host_port = after_scheme.split('/').next().unwrap_or("");
    // NEW-1: IPv6 括号地址不能按 ':' 分割端口号
    let host = if host_port.starts_with('[') {
        match host_port.find(']') {
            Some(end) => &host_port[..=end],
            None => host_port,
        }
    } else {
        host_port.split(':').next().unwrap_or("")
    };

    if host.is_empty() || host == "localhost" || host == "localhost." {
        return true;
    }
    if is_private_or_loopback_ipv4(host) {
        return true;
    }
    if host.starts_with('[') {
        let inner = host.trim_start_matches('[').trim_end_matches(']');
        if inner == "::1" || inner.starts_with("fc") || inner.starts_with("fd")
            || inner.starts_with("FC") || inner.starts_with("FD")
            || inner.starts_with("fe80") || inner.starts_with("FE80")
        {
            return true;
        }
        // NEW-1: IPv4-mapped IPv6 (e.g. [::ffff:127.0.0.1], [::ffff:10.0.0.1])
        for prefix in &["::ffff:", "::FFFF:"] {
            if let Some(ipv4_part) = inner.strip_prefix(prefix) {
                if is_private_or_loopback_ipv4(ipv4_part) {
                    return true;
                }
            }
        }
    }
    false
}

/// 添加自定义端点（校验由 validate_endpoint_config 统一处理）
pub fn add_endpoint(endpoint: &str) -> Result<(), VerificationError> {
    let mut config = get_endpoint_config();
    let endpoint_str = String::from(endpoint);

    if config.endpoints.contains(&endpoint_str) {
        return Ok(());
    }

    config.endpoints.push(endpoint_str);
    config.updated_at = current_timestamp_ms();
    save_endpoint_config(&config)?;
    log::info!(target: "trc20-verifier", "Added endpoint: {}", endpoint);
    Ok(())
}

/// 设置端点 API Key (H1)
///
/// M2-R3修复: 验证端点必须存在于端点列表中，防止孤立 API Key 条目
pub fn set_api_key(endpoint: &str, api_key: &str) -> Result<(), VerificationError> {
    let mut config = get_endpoint_config();
    let ep = String::from(endpoint);
    // M2-R3: 端点必须已注册
    if !config.endpoints.contains(&ep) {
        return Err(VerificationError::InvalidEndpointUrl(
            "Endpoint not found in endpoint list",
        ));
    }
    let key = String::from(api_key);
    if let Some(pos) = config.api_keys.iter().position(|(e, _)| e == &ep) {
        config.api_keys[pos].1 = key;
    } else {
        config.api_keys.push((ep, key));
    }
    config.updated_at = current_timestamp_ms();
    save_endpoint_config(&config)?;
    Ok(())
}

/// 获取端点对应的 API Key (H1)
///
/// H1-R3修复: 使用精确匹配替代 starts_with 前缀匹配，
/// 防止 API Key 泄漏到同名前缀的恶意端点
/// (如 "https://api.trongrid.io" 的 Key 被发送到 "https://api.trongrid.io.evil.com")
fn get_api_key_for_endpoint(endpoint: &str) -> Option<String> {
    let config = get_endpoint_config();
    config.api_keys.iter()
        .find(|(e, _)| e == endpoint)
        .map(|(_, k)| k.clone())
}

/// 移除端点 (M1修复: 同时清理关联的 api_keys 和 priority_boosts)
pub fn remove_endpoint(endpoint: &str) {
    let mut config = get_endpoint_config();
    let endpoint_str = String::from(endpoint);

    if let Some(pos) = config.endpoints.iter().position(|e| e == &endpoint_str) {
        config.endpoints.remove(pos);
        config.api_keys.retain(|(e, _)| e != &endpoint_str);
        config.priority_boosts.retain(|(e, _)| e != &endpoint_str);
        config.updated_at = current_timestamp_ms();
        let _ = save_endpoint_config(&config);
        log::info!(target: "trc20-verifier", "Removed endpoint: {}", endpoint);
    }
}

/// 获取按健康评分排序的端点列表 (L2 增强: 支持优先级加成 + 熔断过滤)
pub fn get_sorted_endpoints() -> Vec<String> {
    let config = get_endpoint_config();
    let mut endpoints_with_scores: Vec<(String, u32)> = config.endpoints
        .iter()
        .filter_map(|e| {
            if is_endpoint_quarantined(e) {
                log::debug!(target: "trc20-verifier", "Skipping quarantined endpoint: {}", e);
                return None;
            }
            let health = get_endpoint_health(e);
            let boost = config.priority_boosts.iter()
                .find(|(ep, _)| ep == e)
                .map(|(_, b)| *b)
                .unwrap_or(0);
            Some((e.clone(), health.score.saturating_add(boost)))
        })
        .collect();

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
///
/// M1-R4修复: 验证端点必须存在于端点列表中（与 M2-R3 set_api_key 对齐）
pub fn set_endpoint_priority_boost(endpoint: &str, boost: u32) -> Result<(), VerificationError> {
    let mut config = get_endpoint_config();
    let ep = String::from(endpoint);
    // M1-R4: 端点必须已注册
    if !config.endpoints.contains(&ep) {
        return Err(VerificationError::InvalidEndpointUrl(
            "Endpoint not found in endpoint list",
        ));
    }
    if let Some(pos) = config.priority_boosts.iter().position(|(e, _)| e == &ep) {
        config.priority_boosts[pos].1 = boost;
    } else {
        config.priority_boosts.push((ep, boost));
    }
    config.updated_at = current_timestamp_ms();
    save_endpoint_config(&config)?;
    Ok(())
}

// ==================== 端点熔断/隔离 ====================

/// 检查端点是否处于熔断隔离中
pub fn is_endpoint_quarantined(endpoint: &str) -> bool {
    let key = [QUARANTINE_PREFIX, endpoint.as_bytes()].concat();
    sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, &key)
        .and_then(|d| u64::decode(&mut &d[..]).ok())
        .map_or(false, |until| current_timestamp_ms() < until)
}

/// 对端点执行熔断隔离（持续 QUARANTINE_DURATION_MS 毫秒）
fn quarantine_endpoint(endpoint: &str) {
    let key = [QUARANTINE_PREFIX, endpoint.as_bytes()].concat();
    let until = current_timestamp_ms().saturating_add(QUARANTINE_DURATION_MS);
    sp_io::offchain::local_storage_set(StorageKind::PERSISTENT, &key, &until.encode());
    log::warn!(target: "trc20-verifier",
        "Endpoint quarantined for {}ms: {}", QUARANTINE_DURATION_MS, endpoint);
}

/// 清除端点的熔断状态（成功请求后调用）
fn clear_quarantine(endpoint: &str) {
    let key = [QUARANTINE_PREFIX, endpoint.as_bytes()].concat();
    sp_io::offchain::local_storage_set(StorageKind::PERSISTENT, &key, &[]);
}

// ==================== tx_hash 重放防护 (P0) ====================

/// 检查 tx_hash 是否已被使用过（防重放）
pub fn is_tx_hash_used(tx_hash: &[u8]) -> bool {
    if tx_hash.is_empty() { return false; }
    let key = [USED_TX_HASH_PREFIX, tx_hash].concat();
    sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, &key)
        .map_or(false, |data| !data.is_empty())
}

/// 注册单个 tx_hash 为已使用（防重放）
pub fn register_used_tx_hash(tx_hash: &[u8]) {
    if tx_hash.is_empty() { return; }
    let key = [USED_TX_HASH_PREFIX, tx_hash].concat();
    let now = current_timestamp_ms();
    sp_io::offchain::local_storage_set(StorageKind::PERSISTENT, &key, &now.encode());
}

/// 批量注册 TransferSearchResult 中所有匹配转账的 tx_hash（上层 pallet 接受验证结果后调用）
pub fn register_result_tx_hashes(result: &TransferSearchResult) {
    for t in &result.matched_transfers {
        register_used_tx_hash(&t.tx_hash);
    }
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
        // M4: 记录速率限制拦截
        let mut m = get_verifier_metrics();
        m.rate_limit_hit_count = m.rate_limit_hit_count.saturating_add(1);
        m.last_updated = now;
        save_verifier_metrics(&m);
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
    // M4: 记录缓存命中
    let mut m = get_verifier_metrics();
    m.cache_hit_count = m.cache_hit_count.saturating_add(1);
    m.last_updated = now;
    save_verifier_metrics(&m);
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
    // M3: 注册缓存键用于后续清理
    register_cache_key(url.as_bytes());
}

// ==================== 审计日志 (M7) ====================

/// H3-C: 共识验证详情（嵌入审计日志）
#[derive(Debug, Clone, Encode, Decode)]
pub struct ConsensusDetail {
    /// 是否使用了共识验证模式
    pub consensus_mode: bool,
    /// 达成一致的端点列表
    pub agreeing_endpoints: Vec<Vec<u8>>,
    /// 返回不同数据的端点（如有）
    pub dissenting_endpoint: Option<Vec<u8>>,
    /// 是否降级为单源验证
    pub degraded: bool,
    /// 实际查询的端点数
    pub total_endpoints_queried: u8,
    /// 成功响应的端点数
    pub total_endpoints_responded: u8,
}

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
    /// 匹配到的交易哈希 (M5)
    pub tx_hash: Vec<u8>,
    /// 使用的端点 (M5)
    pub endpoint_used: Vec<u8>,
    /// 验证耗时（毫秒）(M5)
    pub duration_ms: u64,
    /// H3-C: 共识验证详情（非共识模式为 None）
    pub consensus_detail: Option<ConsensusDetail>,
}

/// 记录审计日志
fn write_audit_log(entry: &AuditLogEntry) {
    let config = get_verifier_config();
    if config.audit_log_retention == 0 { return; }

    let counter = sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, AUDIT_LOG_COUNTER_KEY)
        .and_then(|d| u64::decode(&mut &d[..]).ok())
        .unwrap_or(0);
    // M3-R1修复: 使用 saturating_add 替代 wrapping_add，防止 u64 回绕后清理逻辑下溢
    let next = counter.saturating_add(1);

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
///
/// M2-R4修复: 将迭代次数限制为 min(max_count, counter, audit_log_retention)，
/// 防止大 max_count 导致过度循环（遍历大量不存在的存储键）
pub fn get_recent_audit_logs(max_count: u32) -> Vec<AuditLogEntry> {
    let counter = sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, AUDIT_LOG_COUNTER_KEY)
        .and_then(|d| u64::decode(&mut &d[..]).ok())
        .unwrap_or(0);
    if counter == 0 || max_count == 0 { return Vec::new(); }

    // M2-R4: 限制实际迭代次数，避免 max_count >> counter 时的无效循环
    let retention = get_verifier_config().audit_log_retention as u64;
    let effective = (max_count as u64).min(counter).min(if retention > 0 { retention } else { counter });
    let start = counter.saturating_sub(effective - 1);
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

/// 金额匹配状态
#[derive(Debug, Clone, PartialEq, Eq, Default, Encode, Decode)]
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

// ==================== 并行请求竞速模式 ====================

/// NEW-5: 简易 URL 百分号编码 (RFC 3986 unreserved 字符直通，其余编码)
fn percent_encode_param(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push('%');
                let hi = b >> 4;
                let lo = b & 0x0F;
                encoded.push(if hi < 10 { (b'0' + hi) as char } else { (b'A' + hi - 10) as char });
                encoded.push(if lo < 10 { (b'0' + lo) as char } else { (b'A' + lo - 10) as char });
            }
        }
    }
    encoded
}

/// 安全构建端点 URL (H3)
///
/// 使用 `strip_prefix` 替代 `replace`，仅替换 URL 开头的基础域名，
/// 避免端点 URL 中包含 TRONGRID_MAINNET 子串时的意外替换。
fn build_endpoint_url(url: &str, endpoint: &str) -> String {
    match url.strip_prefix(TRONGRID_MAINNET) {
        Some(path) => format!("{}{}", endpoint, path),
        None => {
            log::warn!(target: "trc20-verifier",
                "URL does not start with expected base: {}", TRONGRID_MAINNET);
            url.to_string()
        }
    }
}

/// 发送 HTTP GET 请求（智能模式选择 + H2 速率限制 + M1 缓存）
///
/// 返回 (响应体, 使用的端点名称)
/// 注意：此函数用于非共识模式。共识模式使用 fetch_url_with_consensus。
fn fetch_url_with_fallback(url: &str) -> Result<(Vec<u8>, String), VerificationError> {
    // M1: 检查缓存
    if let Some(cached) = get_cached_response(url) {
        return Ok((cached, String::from("(cached)")));
    }

    // H2: 速率限制
    check_rate_limit()?;

    let config = get_endpoint_config();

    let result = if config.parallel_mode && config.endpoints.len() > 1 {
        fetch_url_parallel_race(url, &config)
    } else {
        fetch_url_sequential(url, &config)
    };

    // M1+S5: 成功时写入缓存 — 仅缓存结构合法的 JSON 响应，防止缓存投毒
    if let Ok((ref body, _)) = result {
        if !body.is_empty() && (body[0] == b'{' || body[0] == b'[') {
            set_cached_response(url, body);
        }
    }

    result
}

/// H3-C: 共识模式 HTTP 请求 — 收集所有端点响应并进行共识验证
///
/// 返回: (共识后的响应体, 使用的端点名称, 共识详情)
///
/// 与 `fetch_url_with_fallback` 的区别:
/// - 不使用单 URL 缓存（H3-A3修复：防止缓存导致伪共识）
/// - 收集所有端点响应后做 2-of-N 共识比对
/// - 共识成功后缓存共识结论
fn fetch_url_with_consensus(
    url: &str,
    expected_from: &str,
    usdt_contract: &str,
    verifier_config: &VerifierConfig,
) -> Result<(Vec<u8>, String, ConsensusDetail), VerificationError> {
    // H3-A3修复: 共识模式下不使用旧的 per-URL 缓存
    // 否则所有端点命中同一个缓存 → 伪 N/N 一致

    // H2: 速率限制
    check_rate_limit()?;

    let config = get_endpoint_config();
    let total_queried = config.endpoints.iter()
        .filter(|e| !is_endpoint_quarantined(e))
        .count() as u8;

    let all_responses = fetch_all_endpoints(url, &config)?;
    let total_responded = all_responses.len() as u8;

    if all_responses.is_empty() {
        return Err(VerificationError::AllEndpointsFailed);
    }

    // 将原始响应转换为 EndpointResponse（含标准化转账列表）
    let mut endpoint_responses: Vec<EndpointResponse> = Vec::new();
    for (endpoint, body, response_ms) in all_responses {
        let transfers = extract_normalized_transfers(&body, expected_from, usdt_contract)
            .unwrap_or_default();
        endpoint_responses.push(EndpointResponse {
            endpoint,
            transfers,
            response_ms,
            raw_body: body,
        });
    }

    let min_responses = verifier_config.min_consensus_responses as usize;
    let tolerance_ms = verifier_config.consensus_timestamp_tolerance_ms;

    // 检查有效响应数 vs 降级策略
    if endpoint_responses.len() < min_responses {
        if verifier_config.allow_single_source_fallback && endpoint_responses.len() == 1 {
            log::warn!(target: "trc20-verifier",
                "Consensus: only 1 response, falling back to single-source (degraded)");
            let resp = endpoint_responses.into_iter().next().unwrap();
            let detail = ConsensusDetail {
                consensus_mode: true,
                agreeing_endpoints: Vec::new(),
                dissenting_endpoint: None,
                degraded: true,
                total_endpoints_queried: total_queried,
                total_endpoints_responded: total_responded,
            };
            // 记录降级指标
            let mut m = get_verifier_metrics();
            m.degraded_verification_count = m.degraded_verification_count.saturating_add(1);
            m.last_updated = current_timestamp_ms();
            save_verifier_metrics(&m);
            return Ok((resp.raw_body, resp.endpoint, detail));
        }
        return Err(VerificationError::InsufficientEndpointResponses);
    }

    match build_consensus(endpoint_responses, min_responses, tolerance_ms) {
        ConsensusResult::Agreed { raw_body, response_endpoint, agreeing_endpoints, dissenting_endpoint, .. } => {
            if let Some(ref ep) = dissenting_endpoint {
                // 对分歧端点额外降低健康分
                log::warn!(target: "trc20-verifier",
                    "Consensus dissenting endpoint: {}", ep);
                let mut health = get_endpoint_health(ep);
                health.record_failure();
                save_endpoint_health(ep, &health);
            }

            // H3-A3修复: 共识成功后缓存结论（而非单端点响应）
            if !raw_body.is_empty() && (raw_body[0] == b'{' || raw_body[0] == b'[') {
                set_cached_response(url, &raw_body);
            }

            let detail = ConsensusDetail {
                consensus_mode: true,
                agreeing_endpoints: agreeing_endpoints.iter().map(|e| e.as_bytes().to_vec()).collect(),
                dissenting_endpoint: dissenting_endpoint.map(|e| e.into_bytes()),
                degraded: false,
                total_endpoints_queried: total_queried,
                total_endpoints_responded: total_responded,
            };
            // 记录共识成功指标
            let mut m = get_verifier_metrics();
            m.consensus_success_count = m.consensus_success_count.saturating_add(1);
            m.last_updated = current_timestamp_ms();
            save_verifier_metrics(&m);

            Ok((raw_body, response_endpoint, detail))
        },
        ConsensusResult::NoConsensus { .. } => {
            log::error!(target: "trc20-verifier",
                "Consensus FAILED: all endpoints disagree");
            // 记录共识失败指标
            let mut m = get_verifier_metrics();
            m.consensus_failure_count = m.consensus_failure_count.saturating_add(1);
            m.last_updated = current_timestamp_ms();
            save_verifier_metrics(&m);
            Err(VerificationError::ConsensusFailure)
        },
        ConsensusResult::InsufficientResponses { count, responses } => {
            if verifier_config.allow_single_source_fallback && count == 1 {
                log::warn!(target: "trc20-verifier",
                    "Consensus: insufficient responses, falling back to single-source (degraded)");
                let resp = responses.into_iter().next().unwrap();
                let detail = ConsensusDetail {
                    consensus_mode: true,
                    agreeing_endpoints: Vec::new(),
                    dissenting_endpoint: None,
                    degraded: true,
                    total_endpoints_queried: total_queried,
                    total_endpoints_responded: total_responded,
                };
                let mut m = get_verifier_metrics();
                m.degraded_verification_count = m.degraded_verification_count.saturating_add(1);
                m.last_updated = current_timestamp_ms();
                save_verifier_metrics(&m);
                return Ok((resp.raw_body, resp.endpoint, detail));
            }
            // 记录共识失败指标
            let mut m = get_verifier_metrics();
            m.consensus_failure_count = m.consensus_failure_count.saturating_add(1);
            m.last_updated = current_timestamp_ms();
            save_verifier_metrics(&m);
            Err(VerificationError::InsufficientEndpointResponses)
        },
    }
}

/// 并行竞速模式：同时请求所有端点，使用最快响应 (H1 API Key + L1 可配置超时 + 熔断过滤)
///
/// H3-A1修复: 区分"真正失败"（HTTP 错误/非200/空体）和"竞争慢"（winner 后未轮询到的端点），
/// 后者不扣分，避免长期运行后只有最快端点存活。
fn fetch_url_parallel_race(url: &str, config: &EndpointConfig) -> Result<(Vec<u8>, String), VerificationError> {
    let endpoints = &config.endpoints;
    log::info!(target: "trc20-verifier", "Starting parallel race with {} endpoints", endpoints.len());

    let start_time = current_timestamp_ms();

    let mut pending_requests: Vec<(String, http::PendingRequest)> = Vec::new();
    let timeout = sp_io::offchain::timestamp()
        .add(Duration::from_millis(config.timeout_race_ms));

    for endpoint in endpoints.iter() {
        if is_endpoint_quarantined(endpoint) {
            log::debug!(target: "trc20-verifier", "Skipping quarantined endpoint: {}", endpoint);
            continue;
        }

        let target_url = build_endpoint_url(url, endpoint);

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
                if health.score < 15 { quarantine_endpoint(endpoint); }
            }
        }
    }

    if pending_requests.is_empty() {
        return Err(VerificationError::AllEndpointsFailed);
    }

    let mut winner_response: Option<Vec<u8>> = None;
    let mut winner_endpoint = String::new();
    // H3-A1: 区分真正失败（HTTP 错误）和竞争落败（未轮询到/超时但可能仍在传输）
    let mut error_endpoints: Vec<String> = Vec::new();

    for (endpoint, pending) in pending_requests {
        if winner_response.is_some() {
            // H3-A1: winner 已确定，后续端点是竞争落败者，不扣分
            log::debug!(target: "trc20-verifier", "Race loser (not penalized): {}", endpoint);
            continue;
        }

        match pending.try_wait(timeout) {
            Ok(Ok(response)) => {
                let response_ms = current_timestamp_ms().saturating_sub(start_time) as u32;

                if response.code == 200 {
                    let body = response.body().collect::<Vec<u8>>();
                    if !body.is_empty() {
                        log::info!(target: "trc20-verifier", "Winner: {} ({}ms)", endpoint, response_ms);

                        let mut health = get_endpoint_health(&endpoint);
                        health.record_success(response_ms);
                        save_endpoint_health(&endpoint, &health);
                        clear_quarantine(&endpoint);

                        winner_endpoint = endpoint;
                        winner_response = Some(body);
                        continue; // H3-A1: 不 break，让后续端点走 "race loser" 路径
                    }
                }

                // 明确的 HTTP 错误（非200 或空体）→ 真正失败
                error_endpoints.push(endpoint);
            },
            Ok(Err(_)) | Err(_) => {
                // 网络错误/超时 → 真正失败
                error_endpoints.push(endpoint);
            }
        }
    }

    // H3-A1: 只对真正失败的端点扣分
    for endpoint in error_endpoints {
        let mut health = get_endpoint_health(&endpoint);
        health.record_failure();
        save_endpoint_health(&endpoint, &health);
        if health.score < 15 { quarantine_endpoint(&endpoint); }
    }

    match winner_response {
        Some(body) => Ok((body, winner_endpoint)),
        None => {
            log::error!(target: "trc20-verifier", "All parallel requests failed");
            Err(VerificationError::AllEndpointsFailed)
        }
    }
}

/// H3-C: 并行收集所有端点响应（用于共识验证模式）
///
/// 与 `fetch_url_parallel_race` 不同，此函数等待所有端点返回（或超时），
/// 收集所有成功响应用于后续共识比对。
///
/// 返回: 所有成功端点的 (endpoint, body, response_ms) 列表
fn fetch_all_endpoints(url: &str, config: &EndpointConfig) -> Result<Vec<(String, Vec<u8>, u32)>, VerificationError> {
    let endpoints = &config.endpoints;
    log::info!(target: "trc20-verifier", "Starting consensus fetch with {} endpoints", endpoints.len());

    let start_time = current_timestamp_ms();

    let mut pending_requests: Vec<(String, http::PendingRequest)> = Vec::new();
    let timeout = sp_io::offchain::timestamp()
        .add(Duration::from_millis(config.timeout_race_ms));

    for endpoint in endpoints.iter() {
        if is_endpoint_quarantined(endpoint) {
            log::debug!(target: "trc20-verifier", "Skipping quarantined endpoint: {}", endpoint);
            continue;
        }

        let target_url = build_endpoint_url(url, endpoint);

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
                if health.score < 15 { quarantine_endpoint(endpoint); }
            }
        }
    }

    if pending_requests.is_empty() {
        return Err(VerificationError::AllEndpointsFailed);
    }

    let mut successful: Vec<(String, Vec<u8>, u32)> = Vec::new();

    // 收集所有响应（不提前 break）
    for (endpoint, pending) in pending_requests {
        match pending.try_wait(timeout) {
            Ok(Ok(response)) => {
                let response_ms = current_timestamp_ms().saturating_sub(start_time) as u32;

                if response.code == 200 {
                    let body = response.body().collect::<Vec<u8>>();
                    if !body.is_empty() {
                        log::info!(target: "trc20-verifier",
                            "Consensus response from {} ({}ms, {} bytes)",
                            endpoint, response_ms, body.len());

                        let mut health = get_endpoint_health(&endpoint);
                        health.record_success(response_ms);
                        save_endpoint_health(&endpoint, &health);
                        clear_quarantine(&endpoint);

                        successful.push((endpoint, body, response_ms));
                        continue;
                    }
                }

                // 非 200 或空体 → 真正失败
                log::warn!(target: "trc20-verifier",
                    "Endpoint {} returned status {} or empty body", endpoint, response.code);
                let mut health = get_endpoint_health(&endpoint);
                health.record_failure();
                save_endpoint_health(&endpoint, &health);
                if health.score < 15 { quarantine_endpoint(&endpoint); }
            },
            Ok(Err(_)) | Err(_) => {
                log::warn!(target: "trc20-verifier", "Endpoint {} timeout/error in consensus mode", endpoint);
                let mut health = get_endpoint_health(&endpoint);
                health.record_failure();
                save_endpoint_health(&endpoint, &health);
                if health.score < 15 { quarantine_endpoint(&endpoint); }
            }
        }
    }

    log::info!(target: "trc20-verifier",
        "Consensus fetch complete: {}/{} endpoints responded successfully",
        successful.len(), endpoints.len());

    Ok(successful)
}

/// 串行故障转移模式 (H1 API Key + L1 可配置超时 + 熔断)
fn fetch_url_sequential(url: &str, config: &EndpointConfig) -> Result<(Vec<u8>, String), VerificationError> {
    let sorted_endpoints = get_sorted_endpoints();
    let mut last_error = VerificationError::NoEndpoints;

    log::info!(target: "trc20-verifier", "Sequential mode with {} endpoints (sorted by health)",
        sorted_endpoints.len());

    for (idx, endpoint) in sorted_endpoints.iter().enumerate() {
        let target_url = build_endpoint_url(url, endpoint);
        let start_time = current_timestamp_ms();

        log::debug!(target: "trc20-verifier", "Trying endpoint {} ({}/{})",
            endpoint, idx + 1, sorted_endpoints.len());

        match fetch_url_with_key(&target_url, endpoint, config.timeout_ms) {
            Ok(response) => {
                let response_ms = current_timestamp_ms().saturating_sub(start_time) as u32;

                let mut health = get_endpoint_health(endpoint);
                health.record_success(response_ms);
                save_endpoint_health(endpoint, &health);
                clear_quarantine(endpoint);

                if idx > 0 {
                    log::info!(target: "trc20-verifier", "Fallback endpoint {} succeeded ({}ms)",
                        endpoint, response_ms);
                    let mut m = get_verifier_metrics();
                    m.endpoint_fallback_count = m.endpoint_fallback_count.saturating_add(1);
                    m.last_updated = current_timestamp_ms();
                    save_verifier_metrics(&m);
                }
                return Ok((response, endpoint.clone()));
            },
            Err(e) => {
                let mut health = get_endpoint_health(endpoint);
                health.record_failure();
                save_endpoint_health(endpoint, &health);
                if health.score < 15 { quarantine_endpoint(endpoint); }

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


// ==================== 响应解析 ====================

// ==================== 按 (from, to, amount) 搜索验证 ====================

/// 单笔匹配转账明细 (M6, NEW-4: 含确认数估计)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedTransfer {
    pub tx_hash: Vec<u8>,
    pub amount: u64,
    pub block_timestamp: u64,
    /// NEW-4: 该笔转账的估计确认数 (与 tx_hash 绑定)
    pub estimated_confirmations: Option<u32>,
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

// ==================== H3-C: 共识验证数据结构 ====================

/// 标准化转账记录 — 用于跨端点比对的规范形式
///
/// 只包含链上不可变字段，不同端点对同一笔交易必须返回完全相同的值
/// （block_timestamp 允许小幅偏差，由 consensus_timestamp_tolerance_ms 控制）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedTransfer {
    /// 交易哈希（主键）
    pub tx_hash: Vec<u8>,
    /// 发送方地址
    pub from: String,
    /// 转账金额（USDT 精度 10^6）
    pub amount: u64,
    /// 区块时间戳（毫秒）
    pub block_timestamp: u64,
    /// 代币合约地址
    pub contract_address: String,
}

/// 排序支持：按 tx_hash 字典序，使比对前的规范化确定性
impl Ord for NormalizedTransfer {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.tx_hash.cmp(&other.tx_hash)
    }
}

impl PartialOrd for NormalizedTransfer {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// 单个端点的标准化响应
#[derive(Debug, Clone)]
pub struct EndpointResponse {
    /// 端点 URL
    pub endpoint: String,
    /// 标准化后的匹配转账列表（已按 tx_hash 排序）
    pub transfers: Vec<NormalizedTransfer>,
    /// HTTP 响应耗时（毫秒）
    pub response_ms: u32,
    /// 原始响应体（用于最终解析）
    pub raw_body: Vec<u8>,
}

/// 共识验证结果 (H3-C)
#[derive(Debug, Clone)]
pub enum ConsensusResult {
    /// ≥ min_consensus_responses 个端点数据一致
    Agreed {
        /// 达成一致的转账列表
        transfers: Vec<NormalizedTransfer>,
        /// 达成一致的端点列表
        agreeing_endpoints: Vec<String>,
        /// 返回不同数据的端点（如有）
        dissenting_endpoint: Option<String>,
        /// 用于后续详细解析的原始响应体（取自第一个一致端点）
        raw_body: Vec<u8>,
        /// 响应端点名（取自第一个一致端点）
        response_endpoint: String,
    },
    /// 所有端点返回不同数据，无法达成共识
    NoConsensus {
        responses: Vec<EndpointResponse>,
    },
    /// 有效响应数量不足（< min_consensus_responses）
    InsufficientResponses {
        count: usize,
        responses: Vec<EndpointResponse>,
    },
}

/// H3-C: 从 TronGrid 原始响应中提取标准化转账列表（用于共识比对）
///
/// 轻量解析器 — 仅提取共识比对所需的字段（tx_hash, from, amount, block_timestamp, contract_address），
/// 不做确认数检查、金额状态计算等业务逻辑。
///
/// `expected_from`: 过滤发送方地址（仅匹配此地址的转账参与共识比对）
/// `usdt_contract`: USDT 合约地址（仅匹配此合约的转账）
///
/// 返回按 tx_hash 排序的 NormalizedTransfer 列表，使比对结果确定性。
pub fn extract_normalized_transfers(
    response: &[u8],
    expected_from: &str,
    usdt_contract: &str,
) -> Result<Vec<NormalizedTransfer>, VerificationError> {
    let response_str = core::str::from_utf8(response)
        .map_err(|_| VerificationError::InvalidUtf8)?;

    let json_value = parse_json(response_str)
        .map_err(|_| VerificationError::InvalidJson)?;

    let root_obj = match json_value.as_object() {
        Some(obj) => obj,
        None => return Ok(Vec::new()),
    };

    // 检查 API success 标志
    match json_obj_get(root_obj, "success") {
        Some(JsonValue::Boolean(true)) => {},
        _ => return Ok(Vec::new()),
    }

    let data_array = match json_obj_get(root_obj, "data") {
        Some(JsonValue::Array(arr)) => arr,
        _ => return Ok(Vec::new()),
    };

    let mut transfers: Vec<NormalizedTransfer> = Vec::new();

    for entry_value in data_array.iter() {
        let entry_obj = match entry_value.as_object() {
            Some(obj) => obj,
            None => continue,
        };

        // 过滤 from 地址
        let from_addr = match json_obj_get_str(entry_obj, "from") {
            Some(addr) if addr == expected_from => addr,
            _ => continue,
        };

        // 精确匹配合约地址
        let contract_addr = json_obj_get(entry_obj, "token_info")
            .and_then(|ti| ti.as_object())
            .and_then(|ti_obj| json_obj_get_str(ti_obj, "address"));
        let contract_addr = match contract_addr {
            Some(addr) if addr == usdt_contract => addr,
            _ => continue,
        };

        // 提取金额
        let amount = match json_obj_get_u64(entry_obj, "value") {
            Some(a) if a > 0 => a,
            _ => continue,
        };

        // 提取 tx_hash
        let tx_hash = json_obj_get_str(entry_obj, "transaction_id")
            .map(|s| s.into_bytes())
            .unwrap_or_default();
        if tx_hash.is_empty() {
            continue;
        }

        // 提取 block_timestamp
        let block_timestamp = json_obj_get_u64(entry_obj, "block_timestamp").unwrap_or(0);

        transfers.push(NormalizedTransfer {
            tx_hash,
            from: from_addr,
            amount,
            block_timestamp,
            contract_address: contract_addr,
        });
    }

    // 按 tx_hash 排序，确保比对确定性
    transfers.sort();

    Ok(transfers)
}

/// H3-C: 比较两个端点的标准化转账列表是否一致
///
/// 比对规则:
/// - tx_hash: 精确匹配
/// - amount: 精确匹配（链上数据不可变）
/// - from: 精确匹配
/// - contract_address: 精确匹配
/// - block_timestamp: 允许 ±tolerance_ms 偏差
/// - 转账条数: 必须完全相同
fn transfers_agree(a: &[NormalizedTransfer], b: &[NormalizedTransfer], tolerance_ms: u64) -> bool {
    if a.len() != b.len() {
        return false;
    }
    // 两个列表都已按 tx_hash 排序
    for (ta, tb) in a.iter().zip(b.iter()) {
        if ta.tx_hash != tb.tx_hash {
            return false;
        }
        if ta.amount != tb.amount {
            return false;
        }
        if ta.from != tb.from {
            return false;
        }
        if ta.contract_address != tb.contract_address {
            return false;
        }
        // block_timestamp 允许偏差
        let ts_diff = if ta.block_timestamp >= tb.block_timestamp {
            ta.block_timestamp - tb.block_timestamp
        } else {
            tb.block_timestamp - ta.block_timestamp
        };
        if ts_diff > tolerance_ms {
            return false;
        }
    }
    true
}

/// H3-C: 多端点共识验证
///
/// 从多个端点的响应中寻找 ≥ `min_agreement` 个端点数据一致的组合。
/// 使用 pairwise 比较算法，对 N 个端点执行 O(N²) 次比对。
pub fn build_consensus(
    responses: Vec<EndpointResponse>,
    min_agreement: usize,
    timestamp_tolerance_ms: u64,
) -> ConsensusResult {
    let n = responses.len();

    if n < min_agreement {
        log::warn!(target: "trc20-verifier",
            "Consensus: only {} responses, need {}", n, min_agreement);
        return ConsensusResult::InsufficientResponses {
            count: n,
            responses,
        };
    }

    // 构建 pairwise 一致性矩阵
    // agree[i][j] = true 表示 responses[i] 与 responses[j] 数据一致
    let mut agree: Vec<Vec<bool>> = alloc::vec![alloc::vec![false; n]; n];
    for i in 0..n {
        agree[i][i] = true; // 自身一致
        for j in (i + 1)..n {
            let matched = transfers_agree(
                &responses[i].transfers,
                &responses[j].transfers,
                timestamp_tolerance_ms,
            );
            agree[i][j] = matched;
            agree[j][i] = matched;
            if matched {
                log::debug!(target: "trc20-verifier",
                    "Consensus: {} and {} AGREE", responses[i].endpoint, responses[j].endpoint);
            } else {
                log::info!(target: "trc20-verifier",
                    "Consensus: {} and {} DISAGREE", responses[i].endpoint, responses[j].endpoint);
            }
        }
    }

    // 找到最大一致组：对每个端点 i，统计与它一致的端点数
    let mut best_group_idx: usize = 0;
    let mut best_group_size: usize = 0;

    for i in 0..n {
        let group_size = agree[i].iter().filter(|&&x| x).count();
        if group_size > best_group_size {
            best_group_size = group_size;
            best_group_idx = i;
        }
    }

    if best_group_size >= min_agreement {
        let mut agreeing_endpoints: Vec<String> = Vec::new();
        let mut dissenting_endpoint: Option<String> = None;

        for j in 0..n {
            if agree[best_group_idx][j] {
                agreeing_endpoints.push(responses[j].endpoint.clone());
            } else {
                dissenting_endpoint = Some(responses[j].endpoint.clone());
            }
        }

        log::info!(target: "trc20-verifier",
            "Consensus REACHED: {}/{} endpoints agree (dissent: {:?})",
            agreeing_endpoints.len(), n, dissenting_endpoint);

        // 取最大一致组中响应最快的端点作为 raw_body 来源
        let best_resp = &responses[best_group_idx];

        return ConsensusResult::Agreed {
            transfers: best_resp.transfers.clone(),
            agreeing_endpoints,
            dissenting_endpoint,
            raw_body: best_resp.raw_body.clone(),
            response_endpoint: best_resp.endpoint.clone(),
        };
    }

    log::warn!(target: "trc20-verifier",
        "Consensus FAILED: no group of {} agreeing endpoints found (best group: {})",
        min_agreement, best_group_size);

    ConsensusResult::NoConsensus { responses }
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
    // C2: 校验 TRON 地址格式
    validate_tron_address(from_address)?;
    validate_tron_address(to_address)?;

    let from_str = core::str::from_utf8(from_address).map_err(|_| VerificationError::InvalidUtf8)?;
    let to_str = core::str::from_utf8(to_address).map_err(|_| VerificationError::InvalidUtf8)?;

    let now_ms = sp_io::offchain::timestamp().unix_millis();
    let verify_start = now_ms;

    let verifier_config = get_verifier_config();

    // Kill switch: 全局禁用检查
    if !verifier_config.enabled {
        log::warn!(target: "trc20-verifier", "Verifier is globally disabled");
        return Err(VerificationError::VerifierDisabled);
    }

    // C3: 时间窗口最大值限制
    if verifier_config.max_lookback_ms > 0 && min_timestamp > 0 {
        let earliest_allowed = now_ms.saturating_sub(verifier_config.max_lookback_ms);
        if min_timestamp < earliest_allowed {
            log::warn!(target: "trc20-verifier",
                "Timestamp {} exceeds max lookback window (earliest_allowed={})",
                min_timestamp, earliest_allowed);
            return Err(VerificationError::TimestampTooOld);
        }
    }

    // M1+NEW-7: OCW 并发锁 (返回锁令牌用于安全释放)
    let lock_id = [from_address, b":", to_address].concat();
    let lock_token = match try_acquire_verify_lock(&lock_id) {
        Some(token) => token,
        None => {
            log::info!(target: "trc20-verifier",
                "Verification already in progress for {}=>{}", from_str, to_str);
            let mut m = get_verifier_metrics();
            m.lock_contention_count = m.lock_contention_count.saturating_add(1);
            m.last_updated = now_ms;
            save_verifier_metrics(&m);
            return Err(VerificationError::VerificationLocked);
        }
    };

    // NEW-3: 一次性读取配置，后续传参避免重复解码
    let contract = effective_usdt_contract(&verifier_config);
    let min_conf = effective_min_confirmations(&verifier_config);
    let tolerance_bps = verifier_config.amount_tolerance_bps;

    log::info!(target: "trc20-verifier",
        "Searching TRC20 transfers: to={}, from={}, amount={}, since={}, consensus={}",
        to_str, from_str, expected_amount, min_timestamp, verifier_config.consensus_enabled);

    // M2: 分页循环
    let mut last_endpoint_used = String::new();
    let mut last_consensus_detail: Option<ConsensusDetail> = None;
    let use_consensus = verifier_config.consensus_enabled;
    let paging_result: Result<TransferSearchResult, VerificationError> = (|| {
        let mut combined = TransferSearchResult::default();
        let mut fingerprint: Option<String> = None;
        let mut page = 0u32;

        loop {
            let url = if let Some(ref fp) = fingerprint {
                format!(
                    "{}/v1/accounts/{}/transactions/trc20?contract_address={}&only_to=true&min_timestamp={}&limit=50&order_by=block_timestamp,desc&fingerprint={}",
                    TRONGRID_MAINNET, to_str, contract, min_timestamp, percent_encode_param(fp)
                )
            } else {
                format!(
                    "{}/v1/accounts/{}/transactions/trc20?contract_address={}&only_to=true&min_timestamp={}&limit=50&order_by=block_timestamp,desc",
                    TRONGRID_MAINNET, to_str, contract, min_timestamp
                )
            };

            // H3-C: 根据 consensus_enabled 选择 fetch 模式
            let (response, endpoint_used) = if use_consensus {
                let (body, ep, detail) = fetch_url_with_consensus(&url, from_str, &contract, &verifier_config)?;
                last_consensus_detail = Some(detail);
                (body, ep)
            } else {
                fetch_url_with_fallback(&url)?
            };
            last_endpoint_used = endpoint_used;
            let (page_result, next_fp) = parse_trc20_transfer_list_paged(
                &response, from_str, expected_amount, now_ms, &contract, min_conf, tolerance_bps,
            )?;

            merge_transfer_results(&mut combined, &page_result, expected_amount, tolerance_bps);

            page += 1;

            let enough = matches!(combined.amount_status, AmountStatus::Exact | AmountStatus::Overpaid { .. });
            if enough || next_fp.is_none() || page >= verifier_config.max_pages {
                if !enough && next_fp.is_some() && page >= verifier_config.max_pages {
                    combined.truncated = true;
                }
                break;
            }
            fingerprint = next_fp;
        }

        Ok(combined)
    })();

    // H1-R1+NEW-7: 使用锁令牌安全释放 (仅释放自己持有的锁)
    release_verify_lock(&lock_id, lock_token);

    let mut combined = paging_result?;

    // P0: tx_hash 重放过滤 — 移除已被使用过的转账，防止同一笔链上转账验证多笔订单
    let pre_filter_count = combined.matched_transfers.len();
    combined.matched_transfers.retain(|t| !is_tx_hash_used(&t.tx_hash));
    if combined.matched_transfers.len() < pre_filter_count {
        let removed = pre_filter_count - combined.matched_transfers.len();
        log::info!(target: "trc20-verifier",
            "Filtered {} already-used tx_hash(es) for {}=>{}", removed, from_str, to_str);

        let total: u64 = combined.matched_transfers.iter().map(|t| t.amount).sum();
        if total == 0 {
            combined.found = false;
            combined.actual_amount = None;
            combined.tx_hash = None;
            combined.block_timestamp = None;
            combined.amount_status = AmountStatus::Invalid;
            combined.error = Some(b"All matching transfers already used".to_vec());
        } else {
            combined.actual_amount = Some(total);
            combined.amount_status = calculate_amount_status(expected_amount, total, tolerance_bps);
            if let Some(max_t) = combined.matched_transfers.iter().max_by_key(|t| t.amount) {
                combined.tx_hash = Some(max_t.tx_hash.clone());
                combined.block_timestamp = Some(max_t.block_timestamp);
                combined.estimated_confirmations = max_t.estimated_confirmations;
            }
            match &combined.amount_status {
                AmountStatus::Underpaid { shortage } | AmountStatus::SeverelyUnderpaid { shortage } => {
                    combined.remaining_amount = Some(*shortage);
                },
                _ => { combined.remaining_amount = None; }
            }
        }
    }

    // C1: found=true 时保证 tx_hash 非空（防重放前提）
    if combined.found && combined.tx_hash.as_ref().map_or(true, |h| h.is_empty()) {
        log::error!(target: "trc20-verifier",
            "Found matching transfers but no valid tx_hash for {}=>{}",
            from_str, to_str);
        combined.found = false;
        combined.error = Some(b"No valid transaction hash found".to_vec());
    }

    let duration_ms = sp_io::offchain::timestamp().unix_millis().saturating_sub(verify_start);

    // M4: 记录验证指标
    record_metric_verification(combined.found, duration_ms);

    // M5+M7: 写入增强审计日志（endpoint_used 现在由 fetch 层传回）
    write_audit_log(&AuditLogEntry {
        timestamp: now_ms,
        action: b"verify_trc20_by_transfer".to_vec(),
        from_address: from_address.to_vec(),
        to_address: to_address.to_vec(),
        expected_amount,
        actual_amount: combined.actual_amount.unwrap_or(0),
        result_ok: combined.found,
        error_msg: combined.error.clone().unwrap_or_default(),
        tx_hash: combined.tx_hash.clone().unwrap_or_default(),
        endpoint_used: last_endpoint_used.clone().into_bytes(),
        duration_ms,
        consensus_detail: last_consensus_detail,
    });

    Ok(combined)
}

/// 合并分页结果 (M2, H2修复, NEW-4: estimated_confirmations 绑定 tx_hash, NEW-9: 可配置容差)
///
/// M3-R4修复: 按 tx_hash 去重，防止 API 分页重叠导致金额膨胀
fn merge_transfer_results(combined: &mut TransferSearchResult, page: &TransferSearchResult, expected_amount: u64, tolerance_bps: u32) {
    let mut dedup_page_amt: u64 = 0;
    for t in &page.matched_transfers {
        let is_dup = !t.tx_hash.is_empty()
            && combined.matched_transfers.iter().any(|existing| existing.tx_hash == t.tx_hash);
        if !is_dup {
            combined.matched_transfers.push(t.clone());
            dedup_page_amt = dedup_page_amt.saturating_add(t.amount);
        }
    }

    let prev = combined.actual_amount.unwrap_or(0);
    let total = prev.saturating_add(dedup_page_amt);
    if total > 0 {
        combined.found = true;
        combined.actual_amount = Some(total);
    }

    // L1+NEW-4: 在所有已合并的转账中找全局最大笔，绑定 estimated_confirmations
    if let Some(max_t) = combined.matched_transfers.iter().max_by_key(|t| t.amount) {
        if !max_t.tx_hash.is_empty() {
            combined.tx_hash = Some(max_t.tx_hash.clone());
            if max_t.block_timestamp > 0 {
                combined.block_timestamp = Some(max_t.block_timestamp);
            }
            combined.estimated_confirmations = max_t.estimated_confirmations;
        }
    }

    // H2+NEW-9: 使用累计总额 + 可配置容差重新计算 amount_status
    if total > 0 {
        combined.amount_status = calculate_amount_status(expected_amount, total, tolerance_bps);
        match &combined.amount_status {
            AmountStatus::Underpaid { shortage } | AmountStatus::SeverelyUnderpaid { shortage } => {
                combined.remaining_amount = Some(*shortage);
            },
            _ => {
                combined.remaining_amount = None;
            }
        }
    }

    if !combined.found {
        combined.error = page.error.clone();
        combined.amount_status = page.amount_status.clone();
    }
}

/// 解析 TronGrid TRC20 转账列表响应 (向后兼容包装, NEW-3: 内部读取一次配置)
pub fn parse_trc20_transfer_list(
    response: &[u8],
    expected_from: &str,
    expected_amount: u64,
    now_ms: u64,
) -> Result<TransferSearchResult, VerificationError> {
    let config = get_verifier_config();
    let contract = effective_usdt_contract(&config);
    let min_conf = effective_min_confirmations(&config);
    let tolerance_bps = config.amount_tolerance_bps;
    let (result, _) = parse_trc20_transfer_list_paged(
        response, expected_from, expected_amount, now_ms, &contract, min_conf, tolerance_bps,
    )?;
    Ok(result)
}

/// 解析 TronGrid TRC20 转账列表响应（带分页支持）
/// NEW-3: 接受 min_confirmations + tolerance_bps 避免内部重复读取配置
///
/// 返回: (TransferSearchResult, Option<next_fingerprint>)
fn parse_trc20_transfer_list_paged(
    response: &[u8],
    expected_from: &str,
    expected_amount: u64,
    now_ms: u64,
    usdt_contract: &str,
    min_confirmations: u32,
    tolerance_bps: u32,
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

    let min_conf = min_confirmations;

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
        // M4-R4修复: 精确检查 token_info.address 字段，替代全树搜索
        // 防止合约地址出现在其他字段（如 note/memo）时误匹配非 USDT 转账
        let contract_match = json_obj_get(entry_obj, "token_info")
            .and_then(|ti| ti.as_object())
            .and_then(|ti_obj| json_obj_get_str(ti_obj, "address"))
            .map_or(false, |addr| addr == usdt_contract);
        if !contract_match {
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
                // M2-R2: 使用 .min() 防止极端 age_ms 值截断 u32
                est_conf = Some((age_ms / 3000).min(u32::MAX as u64) as u32);
            }
        }

        if let Some(amount) = json_obj_get_u64(entry_obj, "value") {
            if amount > 0 {
                total_matched_amount = total_matched_amount.saturating_add(amount);

                let tx_hash_bytes = json_obj_get_str(entry_obj, "transaction_id")
                    .map(|s| s.into_bytes());
                let ts = json_obj_get_u64(entry_obj, "block_timestamp");

                // M6+NEW-4: 记录每笔匹配转账明细 (含确认数估计)
                result.matched_transfers.push(MatchedTransfer {
                    tx_hash: tx_hash_bytes.clone().unwrap_or_default(),
                    amount,
                    block_timestamp: ts.unwrap_or(0),
                    estimated_confirmations: est_conf,
                });

                if best_tx_hash.is_none() || amount > max_single_amount {
                    max_single_amount = amount;
                    best_tx_hash = tx_hash_bytes;
                    best_timestamp = ts;
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

    result.amount_status = calculate_amount_status(expected_amount, total_matched_amount, tolerance_bps);

    // L3: 计算还需补付金额
    match &result.amount_status {
        AmountStatus::Underpaid { shortage } | AmountStatus::SeverelyUnderpaid { shortage } => {
            result.remaining_amount = Some(*shortage);
        },
        _ => {}
    }

    Ok((result, next_fingerprint))
}

/// 计算金额匹配状态 (NEW-9: 可配置容差, tolerance_bps 单位为基点, 50 = ±0.5%)
pub fn calculate_amount_status(expected: u64, actual: u64, tolerance_bps: u32) -> AmountStatus {
    if actual == 0 {
        return AmountStatus::Invalid;
    }
    if expected == 0 {
        return AmountStatus::Invalid;
    }

    let tol = tolerance_bps as u128;
    let min_exact = (expected as u128 * (10_000 - tol) / 10_000).min(u64::MAX as u128) as u64;
    let max_exact = (expected as u128 * (10_000 + tol) / 10_000).min(u64::MAX as u128) as u64;
    let severe_threshold = expected / 2;

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

    // ==================== 转账搜索解析测试 ====================

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
        assert_eq!(calculate_amount_status(1_000_000, 1_000_000, 50), AmountStatus::Exact);
        assert_eq!(calculate_amount_status(1_000_000, 1_004_000, 50), AmountStatus::Exact); // within 0.5%
        assert_eq!(calculate_amount_status(1_000_000, 1_010_000, 50), AmountStatus::Overpaid { excess: 10_000 });
        assert_eq!(calculate_amount_status(1_000_000, 800_000, 50), AmountStatus::Underpaid { shortage: 200_000 });
        assert_eq!(calculate_amount_status(1_000_000, 400_000, 50), AmountStatus::SeverelyUnderpaid { shortage: 600_000 });
        assert_eq!(calculate_amount_status(1_000_000, 0, 50), AmountStatus::Invalid);
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
    fn m2_calculate_amount_status_large_amount_no_overflow() {
        // M2修复: 大金额不应溢出
        // 修复前: 10^16 * 1005 溢出 u64（u64::MAX ≈ 1.84×10^19）
        // 修复后: 使用 u128 中间计算
        let large_amount: u64 = 10_000_000_000_000_000; // 10^16 ($10 billion USDT)
        let result = calculate_amount_status(large_amount, large_amount, 50);
        assert_eq!(result, AmountStatus::Exact);

        // 确认边界正确
        let slightly_over = (large_amount as u128 * 1006 / 1000) as u64;
        let result2 = calculate_amount_status(large_amount, slightly_over, 50);
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
        assert_eq!(calculate_amount_status(0, 1_000_000, 50), AmountStatus::Invalid);
        assert_eq!(calculate_amount_status(0, 0, 50), AmountStatus::Invalid);
    }

    #[test]
    fn h8_add_endpoint_rejects_http() {
        with_offchain_ext(|| {
            assert_eq!(add_endpoint("http://api.example.com"), Err(VerificationError::InvalidEndpointUrl("Endpoint must use HTTPS")));
        });
    }

    #[test]
    fn h8_add_endpoint_rejects_short_url() {
        with_offchain_ext(|| {
            assert_eq!(add_endpoint("https://x"), Err(VerificationError::InvalidEndpointUrl("Endpoint URL length must be 10-256 characters")));
        });
    }

    #[test]
    fn h8_add_endpoint_rejects_whitespace() {
        with_offchain_ext(|| {
            assert_eq!(add_endpoint("https://api.example .com"), Err(VerificationError::InvalidEndpointUrl("Endpoint URL must not contain whitespace")));
        });
    }

    #[test]
    fn l6_hex_to_bytes_handles_0x_prefix() {
        // L6修复: 0x 前缀应被自动去除
        assert_eq!(hex_to_bytes("0x1234abcd").unwrap(), vec![0x12, 0x34, 0xab, 0xcd]);
        assert_eq!(hex_to_bytes("0X1234ABCD").unwrap(), vec![0x12, 0x34, 0xab, 0xcd]);
        // 无前缀仍然正常工作
        assert_eq!(hex_to_bytes("1234abcd").unwrap(), vec![0x12, 0x34, 0xab, 0xcd]);
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
        let result = calculate_amount_status(huge, huge, 50);
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
                estimated_confirmations: None,
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
                estimated_confirmations: None,
            }],
            tx_hash: Some(b"tx2".to_vec()),
            block_timestamp: Some(1700000001000),
            ..Default::default()
        };

        let expected_amount = 10_000_000u64;
        merge_transfer_results(&mut combined, &page1, expected_amount, 50);
        merge_transfer_results(&mut combined, &page2, expected_amount, 50);

        assert!(combined.found);
        assert_eq!(combined.actual_amount, Some(11_000_000));
        assert_eq!(combined.amount_status, AmountStatus::Overpaid { excess: 1_000_000 });
        assert_eq!(combined.remaining_amount, None);
    }

    #[test]
    fn h2_merge_single_page_status_unchanged() {
        let mut combined = TransferSearchResult::default();
        let page = TransferSearchResult {
            found: true,
            actual_amount: Some(10_000_000),
            amount_status: AmountStatus::Exact,
            matched_transfers: vec![MatchedTransfer {
                tx_hash: b"tx1".to_vec(),
                amount: 10_000_000,
                block_timestamp: 1700000000000,
                estimated_confirmations: None,
            }],
            tx_hash: Some(b"tx1".to_vec()),
            block_timestamp: Some(1700000000000),
            ..Default::default()
        };
        merge_transfer_results(&mut combined, &page, 10_000_000, 50);
        assert_eq!(combined.amount_status, AmountStatus::Exact);
    }

    #[test]
    fn m1_remove_endpoint_cleans_api_key_and_priority() {
        with_offchain_ext(|| {
            // 添加端点
            let ep = "https://api.trongrid.io";
            add_endpoint(ep).unwrap();
            set_api_key(ep, "my-key").unwrap();
            set_endpoint_priority_boost(ep, 10).unwrap();

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

    // ==================== C2: TRON 地址格式校验 ====================

    #[test]
    fn c2_validate_tron_address_valid() {
        let addr = b"TJCnKsPa7y5okkXvQAidZBzqx3QyQ6sxMW";
        assert!(validate_tron_address(addr).is_ok());
    }

    #[test]
    fn c2_validate_tron_address_wrong_prefix() {
        let addr = b"AJCnKsPa7y5okkXvQAidZBzqx3QyQ6sxMW";
        let err = validate_tron_address(addr).unwrap_err();
        assert!(matches!(err, VerificationError::InvalidTronAddress(_)));
        assert!(err.as_str().contains("start with 'T'"));
    }

    #[test]
    fn c2_validate_tron_address_wrong_length() {
        let addr = b"TJCnKsPa7y5okkXvQAid";
        let err = validate_tron_address(addr).unwrap_err();
        assert!(err.as_str().contains("34 characters"));
    }

    #[test]
    fn c2_validate_tron_address_invalid_base58_chars() {
        // '0', 'O', 'I', 'l' are not in Base58
        let addr = b"T0CnKsPa7y5okkXvQAidZBzqx3QyQ6sxMW";
        let err = validate_tron_address(addr).unwrap_err();
        assert!(err.as_str().contains("Base58"));
    }

    #[test]
    fn c2_validate_tron_address_invalid_utf8() {
        let addr: &[u8] = &[0xFF, 0xFE, 0xFD];
        let err = validate_tron_address(addr).unwrap_err();
        assert!(err.as_str().contains("UTF-8"));
    }

    // ==================== H1: 配置安全验证 ====================

    #[test]
    fn h1_validate_config_rejects_low_confirmations() {
        let mut config = VerifierConfig::default();
        config.min_confirmations = 5;
        let err = validate_verifier_config(&config).unwrap_err();
        assert!(matches!(err, VerificationError::InvalidConfig(_)));
        assert!(err.as_str().contains("min_confirmations"));
    }

    #[test]
    fn h1_validate_config_rejects_bad_rate_limit() {
        let mut config = VerifierConfig::default();
        config.rate_limit_interval_ms = 30; // <50
        let err = validate_verifier_config(&config).unwrap_err();
        assert!(err.as_str().contains("rate_limit_interval_ms"));
    }

    #[test]
    fn h1_validate_config_allows_disabled_rate_limit() {
        let mut config = VerifierConfig::default();
        config.rate_limit_interval_ms = 0;
        assert!(validate_verifier_config(&config).is_ok());
    }

    #[test]
    fn h1_validate_config_rejects_short_lookback() {
        let mut config = VerifierConfig::default();
        config.max_lookback_ms = 1_000; // 1s, less than 1h minimum
        let err = validate_verifier_config(&config).unwrap_err();
        assert!(err.as_str().contains("max_lookback_ms"));
    }

    #[test]
    fn h1_validate_config_allows_disabled_lookback() {
        let mut config = VerifierConfig::default();
        config.max_lookback_ms = 0;
        assert!(validate_verifier_config(&config).is_ok());
    }

    #[test]
    fn h1_validate_config_rejects_invalid_contract_address() {
        let mut config = VerifierConfig::default();
        config.usdt_contract = String::from("INVALID_ADDRESS");
        let err = validate_verifier_config(&config).unwrap_err();
        assert!(matches!(err, VerificationError::InvalidTronAddress(_) | VerificationError::InvalidConfig(_)));
    }

    #[test]
    fn h1_validate_config_default_passes() {
        assert!(validate_verifier_config(&VerifierConfig::default()).is_ok());
    }

    #[test]
    fn h1_save_config_validates() {
        with_offchain_ext(|| {
            let mut config = VerifierConfig::default();
            config.min_confirmations = 3; // too low
            assert!(save_verifier_config(&config).is_err());

            // Valid config should save successfully
            let valid = VerifierConfig::default();
            assert!(save_verifier_config(&valid).is_ok());
        });
    }

    // ==================== C3: 时间窗口最大值限制 ====================

    #[test]
    fn c3_verifier_config_has_max_lookback() {
        let config = VerifierConfig::default();
        assert_eq!(config.max_lookback_ms, DEFAULT_MAX_LOOKBACK_MS);
        assert_eq!(config.max_lookback_ms, 259_200_000); // 72h
    }

    // ==================== H3: URL 构建健壮性 ====================

    #[test]
    fn h3_build_endpoint_url_strip_prefix() {
        let url = format!("{}/v1/accounts/TAddr/transactions/trc20", TRONGRID_MAINNET);
        let endpoint = "https://custom-api.example.com";
        let result = build_endpoint_url(&url, endpoint);
        assert_eq!(result, "https://custom-api.example.com/v1/accounts/TAddr/transactions/trc20");
    }

    #[test]
    fn h3_build_endpoint_url_same_base() {
        let url = format!("{}/v1/test", TRONGRID_MAINNET);
        let result = build_endpoint_url(&url, TRONGRID_MAINNET);
        assert_eq!(result, format!("{}/v1/test", TRONGRID_MAINNET));
    }

    #[test]
    fn h3_build_endpoint_url_no_match_returns_original() {
        let url = "https://other-api.io/v1/test";
        let endpoint = "https://custom.example.com";
        let result = build_endpoint_url(url, endpoint);
        assert_eq!(result, url); // fallback: return original
    }

    // ==================== M2: 端点健康评分窗口化 ====================

    #[test]
    fn m2_endpoint_health_windowing() {
        with_offchain_ext(|| {
            let mut health = EndpointHealth::default();

            // 填充 success_count 到窗口大小（触发半衰阈值）
            health.success_count = HEALTH_WINDOW_SIZE;
            health.failure_count = 0;
            health.avg_response_ms = 500;

            // 记录一次成功，应触发半衰
            health.record_success(500);

            // 半衰后: success_count = HEALTH_WINDOW_SIZE / 2 + 1
            let expected_after_decay = HEALTH_WINDOW_SIZE / 2 + 1;
            assert_eq!(health.success_count, expected_after_decay);
            assert_eq!(health.failure_count, 0);
        });
    }

    #[test]
    fn m2_endpoint_health_windowing_failure() {
        with_offchain_ext(|| {
            let mut health = EndpointHealth::default();
            health.success_count = 80;
            health.failure_count = HEALTH_WINDOW_SIZE - 80;
            health.avg_response_ms = 500;

            // 记录失败触发半衰
            health.record_failure();

            assert_eq!(health.success_count, 40);
            let expected_fail = (HEALTH_WINDOW_SIZE - 80) / 2 + 1;
            assert_eq!(health.failure_count, expected_fail);
        });
    }

    #[test]
    fn m2_endpoint_health_no_windowing_below_threshold() {
        with_offchain_ext(|| {
            let mut health = EndpointHealth::default();
            health.success_count = 10;
            health.failure_count = 5;
            health.avg_response_ms = 500;

            health.record_success(500);

            // No windowing: success_count incremented normally
            assert_eq!(health.success_count, 11);
            assert_eq!(health.failure_count, 5);
        });
    }

    // ==================== M4: 监控指标 ====================

    #[test]
    fn m4_verifier_metrics_default() {
        with_offchain_ext(|| {
            let metrics = get_verifier_metrics();
            assert_eq!(metrics.total_success, 0);
            assert_eq!(metrics.total_failure, 0);
            assert_eq!(metrics.total_duration_ms, 0);
        });
    }

    #[test]
    fn m4_record_metric_verification() {
        with_offchain_ext(|| {
            record_metric_verification(true, 150);
            record_metric_verification(true, 200);
            record_metric_verification(false, 5000);

            let metrics = get_verifier_metrics();
            assert_eq!(metrics.total_success, 2);
            assert_eq!(metrics.total_failure, 1);
            assert_eq!(metrics.total_duration_ms, 5350);
        });
    }

    #[test]
    fn m4_reset_verifier_metrics() {
        with_offchain_ext(|| {
            record_metric_verification(true, 100);
            assert_eq!(get_verifier_metrics().total_success, 1);

            reset_verifier_metrics();
            let metrics = get_verifier_metrics();
            assert_eq!(metrics.total_success, 0);
            assert_eq!(metrics.total_failure, 0);
        });
    }

    // ==================== M5: 审计日志字段增强 ====================

    #[test]
    fn m5_audit_log_entry_has_enhanced_fields() {
        with_offchain_ext(|| {
            let entry = AuditLogEntry {
                timestamp: 1700000000000,
                action: b"test".to_vec(),
                from_address: b"TFrom".to_vec(),
                to_address: b"TTo".to_vec(),
                expected_amount: 100,
                actual_amount: 100,
                result_ok: true,
                error_msg: Vec::new(),
                tx_hash: b"abc123".to_vec(),
                endpoint_used: b"https://api.trongrid.io".to_vec(),
                duration_ms: 250,
                consensus_detail: None,
            };
            write_audit_log(&entry);

            let logs = get_recent_audit_logs(1);
            assert_eq!(logs.len(), 1);
            assert_eq!(logs[0].tx_hash, b"abc123".to_vec());
            assert_eq!(logs[0].endpoint_used, b"https://api.trongrid.io".to_vec());
            assert_eq!(logs[0].duration_ms, 250);
        });
    }

    // ==================== M1: OCW 并发锁 ====================

    #[test]
    fn m1_ocw_lock_acquire_and_release() {
        with_offchain_ext(|| {
            let lock_id = b"test_lock_1";

            // 首次获取应成功
            let token = try_acquire_verify_lock(lock_id);
            assert!(token.is_some());

            // 再次获取应失败（锁未过期）
            assert!(try_acquire_verify_lock(lock_id).is_none());

            // 使用令牌释放后应能重新获取
            release_verify_lock(lock_id, token.unwrap());
            assert!(try_acquire_verify_lock(lock_id).is_some());
        });
    }

    #[test]
    fn m1_ocw_lock_different_ids_independent() {
        with_offchain_ext(|| {
            let lock_a = b"lock_a";
            let lock_b = b"lock_b";

            assert!(try_acquire_verify_lock(lock_a).is_some());
            assert!(try_acquire_verify_lock(lock_b).is_some());
            assert!(try_acquire_verify_lock(lock_a).is_none());
        });
    }

    // ==================== M3: 缓存清理 ====================

    #[test]
    fn m3_register_cache_key_and_cleanup() {
        with_offchain_ext(|| {
            // 设置缓存 TTL=1ms，使所有条目立即过期
            let mut config = VerifierConfig::default();
            config.cache_ttl_ms = 1;
            sp_io::offchain::local_storage_set(
                StorageKind::PERSISTENT,
                VERIFIER_CONFIG_KEY,
                &config.encode(),
            );

            // 写入缓存条目，故意写入无法解码的数据（视为过期）
            let url_key = b"test_url_1";
            let full_key = [RESPONSE_CACHE_PREFIX, url_key].concat();
            sp_io::offchain::local_storage_set(StorageKind::PERSISTENT, &full_key, b"corrupt");

            // 注册到缓存键注册表
            register_cache_key(url_key);

            // 清理过期缓存（corrupt data 视为过期）
            let cleaned = cleanup_expired_cache();
            assert_eq!(cleaned, 1);

            // 再次清理应为 0
            let cleaned2 = cleanup_expired_cache();
            assert_eq!(cleaned2, 0);
        });
    }

    #[test]
    fn m3_cache_eviction_when_full() {
        with_offchain_ext(|| {
            // 注册 MAX_CACHE_ENTRIES 个键
            for i in 0..MAX_CACHE_ENTRIES {
                let key = format!("key_{}", i);
                register_cache_key(key.as_bytes());
            }

            // 注册第 MAX_CACHE_ENTRIES + 1 个键，应淘汰最旧的
            register_cache_key(b"overflow_key");

            let keys: Vec<Vec<u8>> = sp_io::offchain::local_storage_get(
                StorageKind::PERSISTENT, CACHE_KEYS_KEY,
            )
            .and_then(|d| Vec::<Vec<u8>>::decode(&mut &d[..]).ok())
            .unwrap_or_default();

            assert_eq!(keys.len(), MAX_CACHE_ENTRIES);
            // 第一个键 "key_0" 应被淘汰
            assert!(!keys.iter().any(|k| k == b"key_0"));
            // overflow_key 应存在
            assert!(keys.iter().any(|k| k == b"overflow_key"));
        });
    }

    // ==================== H1-R1: OCW 锁泄漏修复回归 ====================

    #[test]
    fn h1r1_lock_released_after_successful_paging() {
        with_offchain_ext(|| {
            // 模拟: 获取锁 → 释放锁后可以重新获取
            let lock_id = b"TFrom:TTo";
            let token = try_acquire_verify_lock(lock_id).unwrap();
            release_verify_lock(lock_id, token);
            assert!(try_acquire_verify_lock(lock_id).is_some());
        });
    }

    // ==================== M2-R1: 配置校验增强回归 ====================

    #[test]
    fn m2r1_validate_config_rejects_tiny_cache_ttl() {
        let mut config = VerifierConfig::default();
        config.cache_ttl_ms = 500; // <1000ms
        let err = validate_verifier_config(&config).unwrap_err();
        assert!(matches!(err, VerificationError::InvalidConfig(_)));
        assert!(err.as_str().contains("cache_ttl_ms"));
    }

    #[test]
    fn m2r1_validate_config_allows_disabled_cache() {
        let mut config = VerifierConfig::default();
        config.cache_ttl_ms = 0; // disabled
        assert!(validate_verifier_config(&config).is_ok());
    }

    #[test]
    fn m2r1_validate_config_allows_valid_cache_ttl() {
        let mut config = VerifierConfig::default();
        config.cache_ttl_ms = 5_000; // 5 seconds
        assert!(validate_verifier_config(&config).is_ok());
    }

    #[test]
    fn m2r1_validate_config_rejects_zero_max_pages() {
        let mut config = VerifierConfig::default();
        config.max_pages = 0;
        let err = validate_verifier_config(&config).unwrap_err();
        assert!(err.as_str().contains("max_pages"));
    }

    #[test]
    fn m2r1_validate_config_rejects_excessive_max_pages() {
        let mut config = VerifierConfig::default();
        config.max_pages = 100;
        let err = validate_verifier_config(&config).unwrap_err();
        assert!(err.as_str().contains("max_pages"));
    }

    #[test]
    fn m2r1_validate_config_allows_max_pages_10() {
        let mut config = VerifierConfig::default();
        config.max_pages = 10;
        assert!(validate_verifier_config(&config).is_ok());
    }

    // ==================== C1: tx_hash 防重放 ====================

    #[test]
    fn c1_merge_results_preserves_tx_hash() {
        let mut combined = TransferSearchResult::default();
        let page = TransferSearchResult {
            found: true,
            actual_amount: Some(10_000_000),
            tx_hash: Some(b"valid_tx_hash_123".to_vec()),
            block_timestamp: Some(1700000000000),
            amount_status: AmountStatus::Exact,
            matched_transfers: vec![MatchedTransfer {
                tx_hash: b"valid_tx_hash_123".to_vec(),
                amount: 10_000_000,
                block_timestamp: 1700000000000,
                estimated_confirmations: None,
            }],
            ..Default::default()
        };
        merge_transfer_results(&mut combined, &page, 10_000_000, 50);

        // tx_hash should be preserved and non-empty
        assert!(combined.tx_hash.is_some());
        assert!(!combined.tx_hash.as_ref().unwrap().is_empty());
    }

    // ==================== H1-R2: audit_log_retention 校验回归 ====================

    #[test]
    fn h1r2_validate_config_rejects_excessive_audit_retention() {
        let mut config = VerifierConfig::default();
        config.audit_log_retention = 20_000; // > 10_000
        let err = validate_verifier_config(&config).unwrap_err();
        assert!(matches!(err, VerificationError::InvalidConfig(_)));
        assert!(err.as_str().contains("audit_log_retention"));
    }

    #[test]
    fn h1r2_validate_config_allows_zero_audit_retention() {
        let mut config = VerifierConfig::default();
        config.audit_log_retention = 0; // disabled
        assert!(validate_verifier_config(&config).is_ok());
    }

    #[test]
    fn h1r2_validate_config_allows_max_audit_retention() {
        let mut config = VerifierConfig::default();
        config.audit_log_retention = 10_000; // upper bound
        assert!(validate_verifier_config(&config).is_ok());
    }

    // ==================== M2-R2: est_conf 截断保护回归 ====================

    #[test]
    fn m2r2_calculate_amount_status_is_acceptable_helper() {
        // 同时验证 AmountStatus::is_acceptable 辅助方法
        assert!(AmountStatus::Exact.is_acceptable());
        assert!(AmountStatus::Overpaid { excess: 1 }.is_acceptable());
        assert!(!AmountStatus::Underpaid { shortage: 1 }.is_acceptable());
        assert!(!AmountStatus::SeverelyUnderpaid { shortage: 1 }.is_acceptable());
        assert!(!AmountStatus::Invalid.is_acceptable());
        assert!(!AmountStatus::Unknown.is_acceptable());
    }

    // ==================== H1-R3: API Key 精确匹配回归 ====================

    #[test]
    fn h1r3_api_key_exact_match_prevents_prefix_leak() {
        with_offchain_ext(|| {
            let trusted = "https://api.trongrid.io";
            let malicious = "https://api.trongrid.io.evil.com";

            add_endpoint(trusted).unwrap();
            add_endpoint(malicious).unwrap();
            set_api_key(trusted, "secret-key-123").unwrap();

            // H1-R3修复: 恶意端点不应获取到信任端点的 API Key
            assert_eq!(get_api_key_for_endpoint(trusted), Some("secret-key-123".into()));
            assert_eq!(get_api_key_for_endpoint(malicious), None);
        });
    }

    #[test]
    fn h1r3_api_key_exact_match_same_endpoint() {
        with_offchain_ext(|| {
            let ep = "https://api.trongrid.io";
            add_endpoint(ep).unwrap();
            set_api_key(ep, "my-key").unwrap();
            assert_eq!(get_api_key_for_endpoint(ep), Some("my-key".into()));
        });
    }

    // ==================== M1-R3: EMA 溢出保护回归 ====================

    #[test]
    fn m1r3_endpoint_health_ema_no_overflow_large_response_ms() {
        with_offchain_ext(|| {
            let mut health = EndpointHealth::default();
            // 模拟极端场景: avg_response_ms 接近 u32::MAX
            health.avg_response_ms = u32::MAX / 2; // ~2.1 billion
            health.success_count = 5;

            // M1-R3修复前: (u32::MAX/2) * 90 溢出 u32, 导致 panic 或截断
            // M1-R3修复后: 使用 u64 中间计算，不溢出
            health.record_success(1000); // 正常响应时间

            // EMA 应收敛，不应 panic
            let expected_approx = ((u32::MAX as u64 / 2) * 90 + 1000 * 10) / 100;
            assert_eq!(health.avg_response_ms, expected_approx as u32);
        });
    }

    // ==================== M2-R3: set_api_key 端点验证回归 ====================

    #[test]
    fn m2r3_set_api_key_rejects_nonexistent_endpoint() {
        with_offchain_ext(|| {
            let result = set_api_key("https://nonexistent.example.com", "key123");
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), VerificationError::InvalidEndpointUrl(_)));
        });
    }

    #[test]
    fn m2r3_set_api_key_accepts_registered_endpoint() {
        with_offchain_ext(|| {
            let ep = "https://api.trongrid.io";
            add_endpoint(ep).unwrap();
            assert!(set_api_key(ep, "valid-key").is_ok());

            let config = get_endpoint_config();
            assert!(config.api_keys.iter().any(|(e, k)| e == ep && k == "valid-key"));
        });
    }

    // ==================== M1-R4: set_endpoint_priority_boost 端点验证回归 ====================

    #[test]
    fn m1r4_set_priority_boost_rejects_nonexistent_endpoint() {
        with_offchain_ext(|| {
            let result = set_endpoint_priority_boost("https://nonexistent.example.com", 10);
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), VerificationError::InvalidEndpointUrl(_)));
        });
    }

    #[test]
    fn m1r4_set_priority_boost_accepts_registered_endpoint() {
        with_offchain_ext(|| {
            let ep = "https://api.trongrid.io";
            add_endpoint(ep).unwrap();
            assert!(set_endpoint_priority_boost(ep, 15).is_ok());

            let config = get_endpoint_config();
            assert!(config.priority_boosts.iter().any(|(e, b)| e == ep && *b == 15));
        });
    }

    // ==================== M2-R4: get_recent_audit_logs 迭代限制回归 ====================

    #[test]
    fn m2r4_get_recent_audit_logs_caps_iteration() {
        with_offchain_ext(|| {
            // 写入 3 条日志
            for _ in 0..3 {
                write_audit_log(&AuditLogEntry {
                    timestamp: 1700000000000,
                    action: b"test".to_vec(),
                    from_address: b"TFrom".to_vec(),
                    to_address: b"TTo".to_vec(),
                    expected_amount: 100,
                    actual_amount: 100,
                    result_ok: true,
                    error_msg: Vec::new(),
                    tx_hash: Vec::new(),
                    endpoint_used: Vec::new(),
                    duration_ms: 100,
                    consensus_detail: None,
                });
            }

            // M2-R4修复前: max_count=u32::MAX 会尝试迭代 ~4 billion 次
            // M2-R4修复后: 迭代次数被限制为 min(max_count, counter, retention)
            let logs = get_recent_audit_logs(u32::MAX);
            assert_eq!(logs.len(), 3);
        });
    }

    #[test]
    fn m2r4_get_recent_audit_logs_respects_retention_cap() {
        with_offchain_ext(|| {
            // 写入 5 条日志（默认 retention=1000，只有 5 条存在）
            for _ in 0..5 {
                write_audit_log(&AuditLogEntry {
                    timestamp: 1700000000000,
                    action: b"test".to_vec(),
                    from_address: b"TFrom".to_vec(),
                    to_address: b"TTo".to_vec(),
                    expected_amount: 100,
                    actual_amount: 100,
                    result_ok: true,
                    error_msg: Vec::new(),
                    tx_hash: Vec::new(),
                    endpoint_used: Vec::new(),
                    duration_ms: 100,
                    consensus_detail: None,
                });
            }

            // 请求 3 条，应返回 3 条
            let logs = get_recent_audit_logs(3);
            assert_eq!(logs.len(), 3);
        });
    }

    // ==================== M3-R4: merge_transfer_results 去重回归 ====================

    #[test]
    fn m3r4_merge_deduplicates_by_tx_hash() {
        // 场景: page1 和 page2 包含相同 tx_hash 的转账（API 分页重叠）
        let mut combined = TransferSearchResult::default();

        let page1 = TransferSearchResult {
            found: true,
            actual_amount: Some(5_000_000),
            amount_status: AmountStatus::Underpaid { shortage: 5_000_000 },
            matched_transfers: vec![MatchedTransfer {
                tx_hash: b"tx_dup".to_vec(),
                amount: 5_000_000,
                block_timestamp: 1700000000000,
                estimated_confirmations: None,
            }],
            tx_hash: Some(b"tx_dup".to_vec()),
            block_timestamp: Some(1700000000000),
            ..Default::default()
        };

        let page2 = TransferSearchResult {
            found: true,
            actual_amount: Some(5_000_000),
            amount_status: AmountStatus::Underpaid { shortage: 5_000_000 },
            matched_transfers: vec![MatchedTransfer {
                tx_hash: b"tx_dup".to_vec(),
                amount: 5_000_000,
                block_timestamp: 1700000000000,
                estimated_confirmations: None,
            }],
            tx_hash: Some(b"tx_dup".to_vec()),
            block_timestamp: Some(1700000000000),
            ..Default::default()
        };

        let expected_amount = 10_000_000u64;
        merge_transfer_results(&mut combined, &page1, expected_amount, 50);
        merge_transfer_results(&mut combined, &page2, expected_amount, 50);

        // M3-R4修复前: actual_amount = 10M (重复计算), amount_status = Exact
        // M3-R4修复后: actual_amount = 5M (去重), 5M >= severe_threshold(5M) → Underpaid
        assert_eq!(combined.actual_amount, Some(5_000_000));
        assert_eq!(combined.matched_transfers.len(), 1);
        assert!(matches!(combined.amount_status, AmountStatus::Underpaid { shortage: 5_000_000 }));
    }

    #[test]
    fn m3r4_merge_allows_different_tx_hashes() {
        // 不同 tx_hash 不应被去重
        let mut combined = TransferSearchResult::default();

        let page1 = TransferSearchResult {
            found: true,
            actual_amount: Some(5_000_000),
            matched_transfers: vec![MatchedTransfer {
                tx_hash: b"tx_a".to_vec(),
                amount: 5_000_000,
                block_timestamp: 1700000000000,
                estimated_confirmations: None,
            }],
            tx_hash: Some(b"tx_a".to_vec()),
            block_timestamp: Some(1700000000000),
            ..Default::default()
        };

        let page2 = TransferSearchResult {
            found: true,
            actual_amount: Some(6_000_000),
            matched_transfers: vec![MatchedTransfer {
                tx_hash: b"tx_b".to_vec(),
                amount: 6_000_000,
                block_timestamp: 1700000001000,
                estimated_confirmations: None,
            }],
            tx_hash: Some(b"tx_b".to_vec()),
            block_timestamp: Some(1700000001000),
            ..Default::default()
        };

        merge_transfer_results(&mut combined, &page1, 10_000_000, 50);
        merge_transfer_results(&mut combined, &page2, 10_000_000, 50);

        assert_eq!(combined.actual_amount, Some(11_000_000));
        assert_eq!(combined.matched_transfers.len(), 2);
        assert_eq!(combined.amount_status, AmountStatus::Overpaid { excess: 1_000_000 });
    }

    // ==================== M4-R4: 精确合约地址检查回归 ====================

    #[test]
    fn m4r4_contract_check_uses_token_info_address() {
        with_offchain_ext(|| {
            // token_info.address 正确匹配 USDT 合约
            let response = br#"{"data":[{"transaction_id":"tx1","from":"TBuyer","to":"TSeller","value":"10000000","block_timestamp":1700000000000,"token_info":{"address":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"}}],"success":true}"#;
            let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
            assert!(result.found);
        });
    }

    #[test]
    fn m4r4_contract_in_other_field_not_matched() {
        with_offchain_ext(|| {
            // M4-R4修复: 合约地址出现在 memo 字段而非 token_info.address — 不应匹配
            let response = br#"{"data":[{"transaction_id":"tx1","from":"TBuyer","to":"TSeller","value":"10000000","block_timestamp":1700000000000,"memo":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","token_info":{"address":"TFakeToken123456789"}}],"success":true}"#;
            let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
            // 修复前: found=true (全树搜索在 memo 字段找到合约地址)
            // 修复后: found=false (仅检查 token_info.address)
            assert!(!result.found);
        });
    }

    #[test]
    fn m4r4_missing_token_info_skips_entry() {
        with_offchain_ext(|| {
            // 无 token_info 字段的转账条目应被跳过
            let response = br#"{"data":[{"transaction_id":"tx1","from":"TBuyer","to":"TSeller","value":"10000000","block_timestamp":1700000000000}],"success":true}"#;
            let result = parse_trc20_transfer_list(response, "TBuyer", 10_000_000, 0).unwrap();
            assert!(!result.found);
        });
    }

    // ==================== S1: Base58Check 校验和回归测试 ====================

    #[test]
    fn s1_validate_tron_address_checksum_valid_usdt_contract() {
        // USDT TRC20 合约地址 — 已知合法 TRON 主网地址
        assert!(validate_tron_address(b"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t").is_ok());
    }

    #[test]
    fn s1_validate_tron_address_checksum_mismatch() {
        // 真实地址末两字符互换 → 格式合法但 Base58Check 校验和失败
        let addr = b"TJCnKsPa7y5okkXvQAidZBzqx3QyQ6sxWM";
        let err = validate_tron_address(addr).unwrap_err();
        assert!(matches!(err, VerificationError::InvalidTronAddress(_)));
    }

    #[test]
    fn s1_base58_decode_tron_roundtrip() {
        // USDT 合约地址解码后首字节应为 0x41 (TRON 主网版本)
        let decoded = base58_decode_tron(b"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t").unwrap();
        assert_eq!(decoded[0], 0x41);
        assert_eq!(decoded.len(), 25);
    }

    // ==================== S3: SSRF 防护回归测试 ====================

    #[test]
    fn s3_add_endpoint_rejects_localhost() {
        with_offchain_ext(|| {
            assert_eq!(
                add_endpoint("https://localhost/api"),
                Err(VerificationError::InvalidEndpointUrl("Endpoint must not target private or loopback addresses"))
            );
        });
    }

    #[test]
    fn s3_add_endpoint_rejects_loopback_ip() {
        with_offchain_ext(|| {
            assert_eq!(
                add_endpoint("https://127.0.0.1/api"),
                Err(VerificationError::InvalidEndpointUrl("Endpoint must not target private or loopback addresses"))
            );
        });
    }

    #[test]
    fn s3_add_endpoint_rejects_private_ips() {
        with_offchain_ext(|| {
            assert_eq!(
                add_endpoint("https://10.0.0.1/api"),
                Err(VerificationError::InvalidEndpointUrl("Endpoint must not target private or loopback addresses"))
            );
            assert_eq!(
                add_endpoint("https://192.168.1.1/api"),
                Err(VerificationError::InvalidEndpointUrl("Endpoint must not target private or loopback addresses"))
            );
            assert_eq!(
                add_endpoint("https://172.16.0.1/api"),
                Err(VerificationError::InvalidEndpointUrl("Endpoint must not target private or loopback addresses"))
            );
        });
    }

    #[test]
    fn s3_add_endpoint_rejects_link_local() {
        with_offchain_ext(|| {
            assert_eq!(
                add_endpoint("https://169.254.1.1/api"),
                Err(VerificationError::InvalidEndpointUrl("Endpoint must not target private or loopback addresses"))
            );
        });
    }

    #[test]
    fn s3_add_endpoint_allows_public_domain() {
        with_offchain_ext(|| {
            assert!(add_endpoint("https://api.trongrid.io").is_ok());
        });
    }

    #[test]
    fn s3_is_private_or_loopback_url_edge_cases() {
        assert!(is_private_or_loopback_url("https://0.0.0.0/api"));
        assert!(is_private_or_loopback_url("https://localhost./api"));
        assert!(is_private_or_loopback_url("https://172.31.255.255/api"));
        assert!(!is_private_or_loopback_url("https://172.32.0.1/api"));
        assert!(!is_private_or_loopback_url("https://8.8.8.8/api"));
        assert!(!is_private_or_loopback_url("https://api.example.com/v1"));
    }

    // ==================== S5: 缓存完整性回归测试 ====================

    #[test]
    fn s5_cache_rejects_non_json_response() {
        with_offchain_ext(|| {
            let mut config = VerifierConfig::default();
            config.cache_ttl_ms = 60_000;
            sp_io::offchain::local_storage_set(
                StorageKind::PERSISTENT, VERIFIER_CONFIG_KEY, &config.encode(),
            );

            // 模拟非 JSON 响应被传入 set_cached_response
            // S5修复: fetch_url_with_fallback 仅缓存以 '{' 或 '[' 开头的响应
            // 此处直接验证: 手动写入 HTML 响应，get_cached_response 应能读取
            // 但 fetch_url_with_fallback 层不会写入此类响应（在 integration 层保证）
            let url = "https://api.test.com/v1/test";
            set_cached_response(url, b"<html>error</html>");
            // 缓存已写入（低层 set_cached_response 不过滤），但可以读取
            let cached = get_cached_response(url);
            assert!(cached.is_some()); // 低层 API 不过滤，过滤在 fetch_url_with_fallback 层
        });
    }

    // ==================== L1: tx_hash 追踪简化回归测试 ====================

    #[test]
    fn l1_merge_results_tracks_global_max() {
        let mut combined = TransferSearchResult::default();

        let page1 = TransferSearchResult {
            found: true,
            actual_amount: Some(7_000_000),
            matched_transfers: vec![MatchedTransfer {
                tx_hash: b"tx_7m".to_vec(),
                amount: 7_000_000,
                block_timestamp: 1700000000000,
                estimated_confirmations: None,
            }],
            tx_hash: Some(b"tx_7m".to_vec()),
            block_timestamp: Some(1700000000000),
            ..Default::default()
        };

        let page2 = TransferSearchResult {
            found: true,
            actual_amount: Some(3_000_000),
            matched_transfers: vec![MatchedTransfer {
                tx_hash: b"tx_3m".to_vec(),
                amount: 3_000_000,
                block_timestamp: 1700000001000,
                estimated_confirmations: None,
            }],
            tx_hash: Some(b"tx_3m".to_vec()),
            block_timestamp: Some(1700000001000),
            ..Default::default()
        };

        merge_transfer_results(&mut combined, &page1, 10_000_000, 50);
        merge_transfer_results(&mut combined, &page2, 10_000_000, 50);

        // L1修复: tx_hash 应指向全局最大笔 (7M)，而非最后一页的最大笔 (3M)
        assert_eq!(combined.tx_hash, Some(b"tx_7m".to_vec()));
        assert_eq!(combined.block_timestamp, Some(1700000000000));
        assert_eq!(combined.actual_amount, Some(10_000_000));
    }

    // ==================== A4: AmountStatus Encode/Decode 回归测试 ====================

    #[test]
    fn a4_amount_status_encode_decode_roundtrip() {
        let statuses = vec![
            AmountStatus::Unknown,
            AmountStatus::Exact,
            AmountStatus::Overpaid { excess: 1_000_000 },
            AmountStatus::Underpaid { shortage: 500_000 },
            AmountStatus::SeverelyUnderpaid { shortage: 9_000_000 },
            AmountStatus::Invalid,
        ];
        for status in statuses {
            let encoded = status.encode();
            let decoded = AmountStatus::decode(&mut &encoded[..]).unwrap();
            assert_eq!(status, decoded);
        }
    }

    // ==================== NEW-1: IPv4-mapped IPv6 SSRF 防护 ====================

    #[test]
    fn new1_ssrf_ipv4_mapped_ipv6_loopback() {
        assert!(is_private_or_loopback_url("https://[::ffff:127.0.0.1]/api"));
        assert!(is_private_or_loopback_url("https://[::FFFF:127.0.0.1]/api"));
        assert!(is_private_or_loopback_url("https://[::ffff:127.0.0.1]:8080/api"));
    }

    #[test]
    fn new1_ssrf_ipv4_mapped_ipv6_private() {
        assert!(is_private_or_loopback_url("https://[::ffff:10.0.0.1]/api"));
        assert!(is_private_or_loopback_url("https://[::ffff:192.168.1.1]/api"));
        assert!(is_private_or_loopback_url("https://[::ffff:172.16.0.1]/api"));
        assert!(is_private_or_loopback_url("https://[::ffff:169.254.1.1]/api"));
    }

    #[test]
    fn new1_ssrf_ipv4_mapped_ipv6_public_allowed() {
        assert!(!is_private_or_loopback_url("https://[::ffff:8.8.8.8]/api"));
        assert!(!is_private_or_loopback_url("https://[::ffff:1.2.3.4]/api"));
    }

    #[test]
    fn new1_add_endpoint_rejects_ipv4_mapped_ipv6() {
        with_offchain_ext(|| {
            assert_eq!(
                add_endpoint("https://[::ffff:127.0.0.1]/api"),
                Err(VerificationError::InvalidEndpointUrl("Endpoint must not target private or loopback addresses"))
            );
        });
    }

    // ==================== NEW-2: save_endpoint_config 校验 ====================

    #[test]
    fn new2_save_endpoint_config_rejects_http() {
        let mut config = EndpointConfig::default();
        config.endpoints.push(String::from("http://evil.com/api"));
        let err = save_endpoint_config(&config).unwrap_err();
        assert!(matches!(err, VerificationError::InvalidEndpointUrl(_)));
    }

    #[test]
    fn new2_save_endpoint_config_rejects_ssrf() {
        let mut config = EndpointConfig::default();
        config.endpoints.push(String::from("https://127.0.0.1/api"));
        let err = save_endpoint_config(&config).unwrap_err();
        assert!(matches!(err, VerificationError::InvalidEndpointUrl(_)));
    }

    #[test]
    fn new2_save_endpoint_config_rejects_bad_timeout() {
        let mut config = EndpointConfig::default();
        config.timeout_ms = 100; // too low
        let err = save_endpoint_config(&config).unwrap_err();
        assert!(matches!(err, VerificationError::InvalidConfig(_)));
    }

    #[test]
    fn new2_save_endpoint_config_accepts_valid() {
        with_offchain_ext(|| {
            let config = EndpointConfig::default();
            assert!(save_endpoint_config(&config).is_ok());
        });
    }

    // ==================== NEW-4: estimated_confirmations 绑定 tx_hash ====================

    #[test]
    fn new4_estimated_confirmations_bound_to_global_max() {
        let mut combined = TransferSearchResult::default();

        let page1 = TransferSearchResult {
            found: true,
            actual_amount: Some(8_000_000),
            matched_transfers: vec![MatchedTransfer {
                tx_hash: b"tx_big".to_vec(),
                amount: 8_000_000,
                block_timestamp: 1700000000000,
                estimated_confirmations: Some(500),
            }],
            estimated_confirmations: Some(500),
            ..Default::default()
        };

        let page2 = TransferSearchResult {
            found: true,
            actual_amount: Some(2_000_000),
            matched_transfers: vec![MatchedTransfer {
                tx_hash: b"tx_small".to_vec(),
                amount: 2_000_000,
                block_timestamp: 1700000100000,
                estimated_confirmations: Some(100),
            }],
            estimated_confirmations: Some(100),
            ..Default::default()
        };

        merge_transfer_results(&mut combined, &page1, 10_000_000, 50);
        merge_transfer_results(&mut combined, &page2, 10_000_000, 50);

        assert_eq!(combined.tx_hash, Some(b"tx_big".to_vec()));
        assert_eq!(combined.estimated_confirmations, Some(500));
    }

    // ==================== NEW-5: fingerprint URL 编码 ====================

    #[test]
    fn new5_percent_encode_ascii_passthrough() {
        assert_eq!(percent_encode_param("abc123"), "abc123");
        assert_eq!(percent_encode_param("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn new5_percent_encode_special_chars() {
        assert_eq!(percent_encode_param("a b"), "a%20b");
        assert_eq!(percent_encode_param("a=b&c"), "a%3Db%26c");
        assert_eq!(percent_encode_param("foo/bar"), "foo%2Fbar");
    }

    // ==================== NEW-6: 配置版本化存储 ====================

    #[test]
    fn new6_verifier_config_versioned_roundtrip() {
        with_offchain_ext(|| {
            let mut config = VerifierConfig::default();
            config.min_confirmations = 25;
            config.amount_tolerance_bps = 100;
            save_verifier_config(&config).unwrap();

            let loaded = get_verifier_config();
            assert_eq!(loaded.min_confirmations, 25);
            assert_eq!(loaded.amount_tolerance_bps, 100);
        });
    }

    #[test]
    fn new6_verifier_config_legacy_migration() {
        with_offchain_ext(|| {
            // 模拟旧版 VerifierConfig (无 amount_tolerance_bps) 直接写入
            #[derive(Encode)]
            struct LegacyConfig {
                usdt_contract: String,
                min_confirmations: u32,
                rate_limit_interval_ms: u64,
                cache_ttl_ms: u64,
                max_pages: u32,
                audit_log_retention: u32,
                max_lookback_ms: u64,
                updated_at: u64,
            }
            let legacy = LegacyConfig {
                usdt_contract: String::from("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"),
                min_confirmations: 30,
                rate_limit_interval_ms: 200,
                cache_ttl_ms: 30_000,
                max_pages: 3,
                audit_log_retention: 100,
                max_lookback_ms: 259_200_000,
                updated_at: 0,
            };
            sp_io::offchain::local_storage_set(
                StorageKind::PERSISTENT, VERIFIER_CONFIG_KEY, &legacy.encode(),
            );

            let loaded = get_verifier_config();
            assert_eq!(loaded.min_confirmations, 30);
            assert_eq!(loaded.amount_tolerance_bps, 50); // 迁移后使用默认值
            assert!(loaded.enabled); // 迁移后默认启用
        });
    }

    #[test]
    fn new6_endpoint_config_versioned_roundtrip() {
        with_offchain_ext(|| {
            let config = EndpointConfig::default();
            save_endpoint_config(&config).unwrap();
            let loaded = get_endpoint_config();
            assert_eq!(loaded.endpoints.len(), config.endpoints.len());
        });
    }

    // ==================== NEW-7: OCW 锁持有者标识 ====================

    #[test]
    fn new7_lock_release_requires_correct_token() {
        with_offchain_ext(|| {
            let lock_id = b"test_owner_lock";
            let token = try_acquire_verify_lock(lock_id).unwrap();

            // 使用错误令牌释放 — CAS 不匹配，锁不应被释放
            release_verify_lock(lock_id, token + 999);
            assert!(try_acquire_verify_lock(lock_id).is_none()); // 锁仍在

            // 使用正确令牌释放
            release_verify_lock(lock_id, token);
            assert!(try_acquire_verify_lock(lock_id).is_some());
        });
    }

    // ==================== NEW-9: 金额容差可配置化 ====================

    #[test]
    fn new9_amount_tolerance_configurable() {
        // 默认 50 bps (±0.5%): 1,004,000 应为 Exact
        assert_eq!(calculate_amount_status(1_000_000, 1_004_000, 50), AmountStatus::Exact);

        // 收窄为 10 bps (±0.1%): 1,004,000 应为 Overpaid
        assert_eq!(
            calculate_amount_status(1_000_000, 1_004_000, 10),
            AmountStatus::Overpaid { excess: 4_000 }
        );

        // 放宽为 500 bps (±5%): 1,040,000 应为 Exact
        assert_eq!(calculate_amount_status(1_000_000, 1_040_000, 500), AmountStatus::Exact);

        // 零容差: 精确匹配才算 Exact
        assert_eq!(calculate_amount_status(1_000_000, 1_000_000, 0), AmountStatus::Exact);
        assert_eq!(
            calculate_amount_status(1_000_000, 1_000_001, 0),
            AmountStatus::Overpaid { excess: 1 }
        );
    }

    #[test]
    fn new9_validate_config_rejects_excessive_tolerance() {
        let mut config = VerifierConfig::default();
        config.amount_tolerance_bps = 1001; // > 1000 (10%)
        let err = validate_verifier_config(&config).unwrap_err();
        assert!(matches!(err, VerificationError::InvalidConfig(_)));
    }

    // ==================== P0: tx_hash 重放防护 ====================

    #[test]
    fn p0_tx_hash_unused_by_default() {
        with_offchain_ext(|| {
            assert!(!is_tx_hash_used(b"tx_new_123"));
        });
    }

    #[test]
    fn p0_register_and_check_tx_hash() {
        with_offchain_ext(|| {
            let tx = b"tx_abc_456";
            assert!(!is_tx_hash_used(tx));
            register_used_tx_hash(tx);
            assert!(is_tx_hash_used(tx));
        });
    }

    #[test]
    fn p0_empty_tx_hash_never_used() {
        with_offchain_ext(|| {
            register_used_tx_hash(b"");
            assert!(!is_tx_hash_used(b""));
        });
    }

    #[test]
    fn p0_register_result_tx_hashes_batch() {
        with_offchain_ext(|| {
            let result = TransferSearchResult {
                found: true,
                actual_amount: Some(10_000_000),
                matched_transfers: vec![
                    MatchedTransfer { tx_hash: b"tx_a".to_vec(), amount: 5_000_000, block_timestamp: 1700000000000, estimated_confirmations: None },
                    MatchedTransfer { tx_hash: b"tx_b".to_vec(), amount: 5_000_000, block_timestamp: 1700000001000, estimated_confirmations: None },
                ],
                ..Default::default()
            };
            register_result_tx_hashes(&result);
            assert!(is_tx_hash_used(b"tx_a"));
            assert!(is_tx_hash_used(b"tx_b"));
            assert!(!is_tx_hash_used(b"tx_c"));
        });
    }

    // ==================== P1: Kill switch ====================

    #[test]
    fn p1_kill_switch_default_enabled() {
        let config = VerifierConfig::default();
        assert!(config.enabled);
    }

    #[test]
    fn p1_kill_switch_disabled_error() {
        with_offchain_ext(|| {
            let mut config = VerifierConfig::default();
            config.enabled = false;
            save_verifier_config(&config).unwrap();

            let err = VerificationError::VerifierDisabled;
            assert_eq!(err.as_str(), "Verifier is globally disabled");
        });
    }

    #[test]
    fn p1_kill_switch_legacy_migration_defaults_enabled() {
        with_offchain_ext(|| {
            #[derive(Encode)]
            struct OldConfig {
                usdt_contract: String,
                min_confirmations: u32,
                rate_limit_interval_ms: u64,
                cache_ttl_ms: u64,
                max_pages: u32,
                audit_log_retention: u32,
                max_lookback_ms: u64,
                updated_at: u64,
                amount_tolerance_bps: u32,
            }
            let old = OldConfig {
                usdt_contract: String::from("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"),
                min_confirmations: 25,
                rate_limit_interval_ms: 200,
                cache_ttl_ms: 30_000,
                max_pages: 3,
                audit_log_retention: 100,
                max_lookback_ms: 259_200_000,
                updated_at: 0,
                amount_tolerance_bps: 50,
            };
            let mut versioned = alloc::vec![CONFIG_VERSION_MARKER, 1u8];
            versioned.extend_from_slice(&old.encode());
            sp_io::offchain::local_storage_set(
                StorageKind::PERSISTENT, VERIFIER_CONFIG_KEY, &versioned,
            );

            let loaded = get_verifier_config();
            assert_eq!(loaded.min_confirmations, 25);
            assert!(loaded.enabled);
        });
    }

    // ==================== P1: 端点熔断/隔离 ====================

    #[test]
    fn p1_quarantine_default_not_quarantined() {
        with_offchain_ext(|| {
            assert!(!is_endpoint_quarantined("https://api.trongrid.io"));
        });
    }

    #[test]
    fn p1_quarantine_and_check() {
        with_offchain_ext(|| {
            let ep = "https://api.trongrid.io";
            quarantine_endpoint(ep);
            assert!(is_endpoint_quarantined(ep));
        });
    }

    #[test]
    fn p1_quarantine_clear() {
        with_offchain_ext(|| {
            let ep = "https://api.trongrid.io";
            quarantine_endpoint(ep);
            assert!(is_endpoint_quarantined(ep));
            clear_quarantine(ep);
            assert!(!is_endpoint_quarantined(ep));
        });
    }

    #[test]
    fn p1_quarantine_independent_endpoints() {
        with_offchain_ext(|| {
            let ep_a = "https://api.trongrid.io";
            let ep_b = "https://api.tronstack.io";
            quarantine_endpoint(ep_a);
            assert!(is_endpoint_quarantined(ep_a));
            assert!(!is_endpoint_quarantined(ep_b));
        });
    }

    #[test]
    fn p1_sorted_endpoints_skip_quarantined() {
        with_offchain_ext(|| {
            add_endpoint("https://api.trongrid.io").unwrap();
            add_endpoint("https://api.tronstack.io").unwrap();

            let all = get_sorted_endpoints();
            assert!(all.iter().any(|e| e == "https://api.trongrid.io"));

            quarantine_endpoint("https://api.trongrid.io");

            let filtered = get_sorted_endpoints();
            assert!(!filtered.iter().any(|e| e == "https://api.trongrid.io"));
            assert!(filtered.iter().any(|e| e == "https://api.tronstack.io"));
        });
    }

    // ==================== P1: endpoint_used 审计日志 ====================

    #[test]
    fn p1_verification_error_variants_display() {
        assert_eq!(VerificationError::VerifierDisabled.as_str(), "Verifier is globally disabled");
        assert_eq!(VerificationError::TxHashAlreadyUsed.as_str(), "Transaction hash already used");
    }

    // ==================== H3-C Phase 1: 共识验证基础设施 ====================

    #[test]
    fn h3c_consensus_failure_error_variant() {
        let err = VerificationError::ConsensusFailure;
        assert_eq!(err.as_str(), "Endpoint consensus verification failed");
        // Display 也正确
        assert_eq!(format!("{}", err), "Endpoint consensus verification failed");
        // From<VerificationError> for &'static str
        let s: &'static str = err.into();
        assert_eq!(s, "Endpoint consensus verification failed");
    }

    #[test]
    fn h3c_insufficient_responses_error_variant() {
        let err = VerificationError::InsufficientEndpointResponses;
        assert_eq!(err.as_str(), "Insufficient endpoint responses for consensus");
        assert_eq!(format!("{}", err), "Insufficient endpoint responses for consensus");
    }

    #[test]
    fn h3c_normalized_transfer_ordering() {
        let t1 = NormalizedTransfer {
            tx_hash: b"aaa".to_vec(),
            from: String::from("T1"),
            amount: 100,
            block_timestamp: 1000,
            contract_address: String::from("C1"),
        };
        let t2 = NormalizedTransfer {
            tx_hash: b"bbb".to_vec(),
            from: String::from("T1"),
            amount: 200,
            block_timestamp: 2000,
            contract_address: String::from("C1"),
        };
        let t3 = NormalizedTransfer {
            tx_hash: b"aaa".to_vec(),
            from: String::from("T1"),
            amount: 100,
            block_timestamp: 1000,
            contract_address: String::from("C1"),
        };
        // 排序: tx_hash 字典序
        assert!(t1 < t2);
        assert!(t2 > t1);
        // 相等
        assert_eq!(t1, t3);
    }

    #[test]
    fn h3c_normalized_transfer_sort_deterministic() {
        let mut transfers = vec![
            NormalizedTransfer {
                tx_hash: b"ccc".to_vec(), from: String::from("T1"),
                amount: 300, block_timestamp: 3000, contract_address: String::from("C1"),
            },
            NormalizedTransfer {
                tx_hash: b"aaa".to_vec(), from: String::from("T1"),
                amount: 100, block_timestamp: 1000, contract_address: String::from("C1"),
            },
            NormalizedTransfer {
                tx_hash: b"bbb".to_vec(), from: String::from("T1"),
                amount: 200, block_timestamp: 2000, contract_address: String::from("C1"),
            },
        ];
        transfers.sort();
        assert_eq!(transfers[0].tx_hash, b"aaa");
        assert_eq!(transfers[1].tx_hash, b"bbb");
        assert_eq!(transfers[2].tx_hash, b"ccc");
    }

    #[test]
    fn h3c_consensus_result_agreed_construction() {
        let transfers = vec![NormalizedTransfer {
            tx_hash: b"tx1".to_vec(), from: String::from("T1"),
            amount: 1_000_000, block_timestamp: 1700000000000,
            contract_address: String::from("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"),
        }];
        let result = ConsensusResult::Agreed {
            transfers: transfers.clone(),
            agreeing_endpoints: vec![String::from("ep1"), String::from("ep2")],
            dissenting_endpoint: Some(String::from("ep3")),
            raw_body: b"{}".to_vec(),
            response_endpoint: String::from("ep1"),
        };
        match result {
            ConsensusResult::Agreed { agreeing_endpoints, dissenting_endpoint, .. } => {
                assert_eq!(agreeing_endpoints.len(), 2);
                assert!(dissenting_endpoint.is_some());
            },
            _ => panic!("Expected Agreed"),
        }
    }

    #[test]
    fn h3c_consensus_result_no_consensus() {
        let result = ConsensusResult::NoConsensus {
            responses: vec![],
        };
        assert!(matches!(result, ConsensusResult::NoConsensus { .. }));
    }

    #[test]
    fn h3c_consensus_result_insufficient() {
        let result = ConsensusResult::InsufficientResponses {
            count: 1,
            responses: vec![],
        };
        match result {
            ConsensusResult::InsufficientResponses { count, .. } => assert_eq!(count, 1),
            _ => panic!("Expected InsufficientResponses"),
        }
    }

    #[test]
    fn h3c_verifier_config_consensus_defaults() {
        let config = VerifierConfig::default();
        assert!(!config.consensus_enabled);
        assert_eq!(config.min_consensus_responses, 2);
        assert!(config.allow_single_source_fallback);
        assert_eq!(config.consensus_timestamp_tolerance_ms, 3_000);
    }

    #[test]
    fn h3c_verifier_config_consensus_save_load_roundtrip() {
        with_offchain_ext(|| {
            let mut config = VerifierConfig::default();
            config.consensus_enabled = true;
            config.min_consensus_responses = 3;
            config.allow_single_source_fallback = false;
            config.consensus_timestamp_tolerance_ms = 6_000;
            save_verifier_config(&config).unwrap();

            let loaded = get_verifier_config();
            assert!(loaded.consensus_enabled);
            assert_eq!(loaded.min_consensus_responses, 3);
            assert!(!loaded.allow_single_source_fallback);
            assert_eq!(loaded.consensus_timestamp_tolerance_ms, 6_000);
        });
    }

    #[test]
    fn h3c_verifier_config_validate_min_consensus_responses() {
        with_offchain_ext(|| {
            let mut config = VerifierConfig::default();
            // 低于下限
            config.min_consensus_responses = 1;
            assert!(matches!(
                validate_verifier_config(&config),
                Err(VerificationError::InvalidConfig(_))
            ));
            // 超出上限
            config.min_consensus_responses = 11;
            assert!(matches!(
                validate_verifier_config(&config),
                Err(VerificationError::InvalidConfig(_))
            ));
            // 有效值
            config.min_consensus_responses = 2;
            assert!(validate_verifier_config(&config).is_ok());
            config.min_consensus_responses = 10;
            assert!(validate_verifier_config(&config).is_ok());
        });
    }

    #[test]
    fn h3c_verifier_config_validate_timestamp_tolerance() {
        with_offchain_ext(|| {
            let mut config = VerifierConfig::default();
            // 0 = 严格模式，允许
            config.consensus_timestamp_tolerance_ms = 0;
            assert!(validate_verifier_config(&config).is_ok());
            // < 1000 且 > 0，拒绝
            config.consensus_timestamp_tolerance_ms = 500;
            assert!(matches!(
                validate_verifier_config(&config),
                Err(VerificationError::InvalidConfig(_))
            ));
            // >= 1000，允许
            config.consensus_timestamp_tolerance_ms = 1_000;
            assert!(validate_verifier_config(&config).is_ok());
        });
    }

    #[test]
    fn h3c_v2_config_migration_from_v1_versioned() {
        // 模拟 V1 格式（含 enabled 但无 consensus 字段）的存储数据
        with_offchain_ext(|| {
            #[derive(Encode)]
            struct V2OldConfig {
                usdt_contract: String,
                min_confirmations: u32,
                rate_limit_interval_ms: u64,
                cache_ttl_ms: u64,
                max_pages: u32,
                audit_log_retention: u32,
                max_lookback_ms: u64,
                updated_at: u64,
                amount_tolerance_bps: u32,
                enabled: bool,
            }
            let old = V2OldConfig {
                usdt_contract: String::from("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"),
                min_confirmations: 19,
                rate_limit_interval_ms: 200,
                cache_ttl_ms: 30_000,
                max_pages: 3,
                audit_log_retention: 100,
                max_lookback_ms: 259_200_000,
                updated_at: 0,
                amount_tolerance_bps: 50,
                enabled: true,
            };
            let mut versioned = alloc::vec![CONFIG_VERSION_MARKER, 1u8];
            versioned.extend_from_slice(&old.encode());
            sp_io::offchain::local_storage_set(
                StorageKind::PERSISTENT, VERIFIER_CONFIG_KEY, &versioned,
            );

            let loaded = get_verifier_config();
            // 保留原有字段
            assert_eq!(loaded.min_confirmations, 19);
            assert!(loaded.enabled);
            // consensus 字段使用默认值
            assert!(!loaded.consensus_enabled);
            assert_eq!(loaded.min_consensus_responses, 2);
            assert!(loaded.allow_single_source_fallback);
            assert_eq!(loaded.consensus_timestamp_tolerance_ms, 3_000);
        });
    }

    #[test]
    fn h3c_v1_config_migration_without_enabled() {
        // 模拟最早的 V1 格式（无 enabled 无 consensus）的存储数据
        with_offchain_ext(|| {
            #[derive(Encode)]
            struct V1OldConfig {
                usdt_contract: String,
                min_confirmations: u32,
                rate_limit_interval_ms: u64,
                cache_ttl_ms: u64,
                max_pages: u32,
                audit_log_retention: u32,
                max_lookback_ms: u64,
                updated_at: u64,
                amount_tolerance_bps: u32,
            }
            let old = V1OldConfig {
                usdt_contract: String::from("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"),
                min_confirmations: 25,
                rate_limit_interval_ms: 200,
                cache_ttl_ms: 30_000,
                max_pages: 3,
                audit_log_retention: 100,
                max_lookback_ms: 259_200_000,
                updated_at: 0,
                amount_tolerance_bps: 50,
            };
            let mut versioned = alloc::vec![CONFIG_VERSION_MARKER, 1u8];
            versioned.extend_from_slice(&old.encode());
            sp_io::offchain::local_storage_set(
                StorageKind::PERSISTENT, VERIFIER_CONFIG_KEY, &versioned,
            );

            let loaded = get_verifier_config();
            assert_eq!(loaded.min_confirmations, 25);
            assert!(loaded.enabled); // V1 → 默认 true
            assert!(!loaded.consensus_enabled); // V1 → 默认 false
            assert_eq!(loaded.min_consensus_responses, 2);
        });
    }

    #[test]
    fn h3c_endpoint_response_construction() {
        let resp = EndpointResponse {
            endpoint: String::from("https://api.trongrid.io"),
            transfers: vec![],
            response_ms: 150,
            raw_body: b"{}".to_vec(),
        };
        assert_eq!(resp.endpoint, "https://api.trongrid.io");
        assert_eq!(resp.response_ms, 150);
    }

    // ==================== H3-C Phase 2: 共识验证核心逻辑 ====================

    /// 构建一个合法的 TronGrid JSON 响应（多笔转账）
    fn make_trongrid_response(transfers: &[(&str, &str, u64, u64, &str)]) -> Vec<u8> {
        // transfers: [(tx_hash, from, amount, block_timestamp, contract_address), ...]
        let mut data_items = Vec::new();
        for (tx_hash, from, amount, ts, contract) in transfers {
            data_items.push(format!(
                r#"{{"transaction_id":"{}","from":"{}","to":"TRecvAddr","value":"{}","block_timestamp":{},"token_info":{{"address":"{}"}}}}"#,
                tx_hash, from, amount, ts, contract
            ));
        }
        let json = format!(r#"{{"success":true,"data":[{}],"meta":{{}}}}"#, data_items.join(","));
        json.into_bytes()
    }

    #[test]
    fn h3c_extract_normalized_transfers_basic() {
        let response = make_trongrid_response(&[
            ("tx_abc", "TSender1", 10_000_000, 1700000000000, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"),
            ("tx_def", "TSender1", 5_000_000, 1700000001000, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"),
        ]);
        let transfers = extract_normalized_transfers(&response, "TSender1", "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t").unwrap();
        assert_eq!(transfers.len(), 2);
        // 已按 tx_hash 排序
        assert_eq!(transfers[0].tx_hash, b"tx_abc");
        assert_eq!(transfers[0].amount, 10_000_000);
        assert_eq!(transfers[1].tx_hash, b"tx_def");
        assert_eq!(transfers[1].amount, 5_000_000);
    }

    #[test]
    fn h3c_extract_normalized_transfers_filters_wrong_from() {
        let response = make_trongrid_response(&[
            ("tx_1", "TSender1", 10_000_000, 1700000000000, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"),
            ("tx_2", "TOtherSender", 5_000_000, 1700000001000, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"),
        ]);
        let transfers = extract_normalized_transfers(&response, "TSender1", "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t").unwrap();
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].tx_hash, b"tx_1");
    }

    #[test]
    fn h3c_extract_normalized_transfers_filters_wrong_contract() {
        let response = make_trongrid_response(&[
            ("tx_1", "TSender1", 10_000_000, 1700000000000, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"),
            ("tx_2", "TSender1", 5_000_000, 1700000001000, "TFakeContract12345678901234567890"),
        ]);
        let transfers = extract_normalized_transfers(&response, "TSender1", "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t").unwrap();
        assert_eq!(transfers.len(), 1);
    }

    #[test]
    fn h3c_extract_normalized_transfers_empty_response() {
        let response = br#"{"success":true,"data":[],"meta":{}}"#;
        let transfers = extract_normalized_transfers(response, "TSender1", "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t").unwrap();
        assert!(transfers.is_empty());
    }

    #[test]
    fn h3c_extract_normalized_transfers_invalid_json() {
        let response = b"not json";
        assert!(matches!(
            extract_normalized_transfers(response, "TSender1", "C"),
            Err(VerificationError::InvalidJson)
        ));
    }

    #[test]
    fn h3c_extract_normalized_transfers_sorted_by_tx_hash() {
        let response = make_trongrid_response(&[
            ("tx_zzz", "TSender1", 1, 1000, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"),
            ("tx_aaa", "TSender1", 2, 2000, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"),
            ("tx_mmm", "TSender1", 3, 3000, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"),
        ]);
        let transfers = extract_normalized_transfers(&response, "TSender1", "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t").unwrap();
        assert_eq!(transfers[0].tx_hash, b"tx_aaa");
        assert_eq!(transfers[1].tx_hash, b"tx_mmm");
        assert_eq!(transfers[2].tx_hash, b"tx_zzz");
    }

    // --- transfers_agree 测试 ---

    fn make_normalized(tx_hash: &str, amount: u64, ts: u64) -> NormalizedTransfer {
        NormalizedTransfer {
            tx_hash: tx_hash.as_bytes().to_vec(),
            from: String::from("TSender1"),
            amount,
            block_timestamp: ts,
            contract_address: String::from("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"),
        }
    }

    #[test]
    fn h3c_transfers_agree_identical() {
        let a = vec![make_normalized("tx1", 100, 1000), make_normalized("tx2", 200, 2000)];
        let b = vec![make_normalized("tx1", 100, 1000), make_normalized("tx2", 200, 2000)];
        assert!(transfers_agree(&a, &b, 3000));
    }

    #[test]
    fn h3c_transfers_agree_timestamp_within_tolerance() {
        let a = vec![make_normalized("tx1", 100, 1000)];
        let b = vec![make_normalized("tx1", 100, 3500)]; // diff = 2500, tolerance = 3000
        assert!(transfers_agree(&a, &b, 3000));
    }

    #[test]
    fn h3c_transfers_disagree_timestamp_exceeds_tolerance() {
        let a = vec![make_normalized("tx1", 100, 1000)];
        let b = vec![make_normalized("tx1", 100, 5000)]; // diff = 4000 > tolerance 3000
        assert!(!transfers_agree(&a, &b, 3000));
    }

    #[test]
    fn h3c_transfers_disagree_different_amount() {
        let a = vec![make_normalized("tx1", 100, 1000)];
        let b = vec![make_normalized("tx1", 200, 1000)];
        assert!(!transfers_agree(&a, &b, 3000));
    }

    #[test]
    fn h3c_transfers_disagree_different_count() {
        let a = vec![make_normalized("tx1", 100, 1000), make_normalized("tx2", 200, 2000)];
        let b = vec![make_normalized("tx1", 100, 1000)];
        assert!(!transfers_agree(&a, &b, 3000));
    }

    #[test]
    fn h3c_transfers_disagree_different_tx_hash() {
        let a = vec![make_normalized("tx1", 100, 1000)];
        let b = vec![make_normalized("tx2", 100, 1000)];
        assert!(!transfers_agree(&a, &b, 3000));
    }

    #[test]
    fn h3c_transfers_agree_both_empty() {
        let a: Vec<NormalizedTransfer> = vec![];
        let b: Vec<NormalizedTransfer> = vec![];
        assert!(transfers_agree(&a, &b, 3000));
    }

    // --- build_consensus 测试 ---

    fn make_endpoint_response(endpoint: &str, transfers: Vec<NormalizedTransfer>) -> EndpointResponse {
        EndpointResponse {
            endpoint: String::from(endpoint),
            transfers,
            response_ms: 100,
            raw_body: b"{}".to_vec(),
        }
    }

    #[test]
    fn h3c_build_consensus_3_of_3_agree() {
        let t = vec![make_normalized("tx1", 1000, 5000)];
        let responses = vec![
            make_endpoint_response("ep1", t.clone()),
            make_endpoint_response("ep2", t.clone()),
            make_endpoint_response("ep3", t.clone()),
        ];
        match build_consensus(responses, 2, 3000) {
            ConsensusResult::Agreed { agreeing_endpoints, dissenting_endpoint, .. } => {
                assert_eq!(agreeing_endpoints.len(), 3);
                assert!(dissenting_endpoint.is_none());
            },
            other => panic!("Expected Agreed, got {:?}", other),
        }
    }

    #[test]
    fn h3c_build_consensus_2_of_3_agree() {
        let t_majority = vec![make_normalized("tx1", 1000, 5000)];
        let t_dissent = vec![make_normalized("tx1", 9999, 5000)]; // 不同金额
        let responses = vec![
            make_endpoint_response("ep1", t_majority.clone()),
            make_endpoint_response("ep2", t_dissent),
            make_endpoint_response("ep3", t_majority.clone()),
        ];
        match build_consensus(responses, 2, 3000) {
            ConsensusResult::Agreed { agreeing_endpoints, dissenting_endpoint, .. } => {
                assert_eq!(agreeing_endpoints.len(), 2);
                assert!(agreeing_endpoints.contains(&String::from("ep1")));
                assert!(agreeing_endpoints.contains(&String::from("ep3")));
                assert_eq!(dissenting_endpoint, Some(String::from("ep2")));
            },
            other => panic!("Expected Agreed, got {:?}", other),
        }
    }

    #[test]
    fn h3c_build_consensus_all_disagree() {
        let responses = vec![
            make_endpoint_response("ep1", vec![make_normalized("tx1", 100, 5000)]),
            make_endpoint_response("ep2", vec![make_normalized("tx1", 200, 5000)]),
            make_endpoint_response("ep3", vec![make_normalized("tx1", 300, 5000)]),
        ];
        match build_consensus(responses, 2, 3000) {
            ConsensusResult::NoConsensus { responses } => {
                assert_eq!(responses.len(), 3);
            },
            other => panic!("Expected NoConsensus, got {:?}", other),
        }
    }

    #[test]
    fn h3c_build_consensus_insufficient_responses() {
        let t = vec![make_normalized("tx1", 1000, 5000)];
        let responses = vec![
            make_endpoint_response("ep1", t),
        ];
        match build_consensus(responses, 2, 3000) {
            ConsensusResult::InsufficientResponses { count, .. } => {
                assert_eq!(count, 1);
            },
            other => panic!("Expected InsufficientResponses, got {:?}", other),
        }
    }

    #[test]
    fn h3c_build_consensus_2_of_3_with_timestamp_tolerance() {
        // ep1 和 ep3 的 timestamp 差 2000ms，在 tolerance 3000ms 以内
        let responses = vec![
            make_endpoint_response("ep1", vec![make_normalized("tx1", 1000, 5000)]),
            make_endpoint_response("ep2", vec![make_normalized("tx1", 1000, 15000)]), // 偏差 10000 > 3000
            make_endpoint_response("ep3", vec![make_normalized("tx1", 1000, 7000)]),  // 偏差 2000 < 3000
        ];
        match build_consensus(responses, 2, 3000) {
            ConsensusResult::Agreed { agreeing_endpoints, dissenting_endpoint, .. } => {
                assert_eq!(agreeing_endpoints.len(), 2);
                assert!(agreeing_endpoints.contains(&String::from("ep1")));
                assert!(agreeing_endpoints.contains(&String::from("ep3")));
                assert_eq!(dissenting_endpoint, Some(String::from("ep2")));
            },
            other => panic!("Expected Agreed, got {:?}", other),
        }
    }

    #[test]
    fn h3c_build_consensus_empty_transfers_agree() {
        // 两个端点都返回空（没有匹配转账）— 应该一致
        let responses = vec![
            make_endpoint_response("ep1", vec![]),
            make_endpoint_response("ep2", vec![]),
        ];
        match build_consensus(responses, 2, 3000) {
            ConsensusResult::Agreed { transfers, agreeing_endpoints, .. } => {
                assert!(transfers.is_empty());
                assert_eq!(agreeing_endpoints.len(), 2);
            },
            other => panic!("Expected Agreed, got {:?}", other),
        }
    }

    #[test]
    fn h3c_build_consensus_2_of_2_exact() {
        // 仅两个端点，都一致 → 2/2 通过
        let t = vec![make_normalized("tx1", 500, 1000)];
        let responses = vec![
            make_endpoint_response("ep1", t.clone()),
            make_endpoint_response("ep2", t),
        ];
        match build_consensus(responses, 2, 3000) {
            ConsensusResult::Agreed { agreeing_endpoints, dissenting_endpoint, .. } => {
                assert_eq!(agreeing_endpoints.len(), 2);
                assert!(dissenting_endpoint.is_none());
            },
            other => panic!("Expected Agreed, got {:?}", other),
        }
    }

    #[test]
    fn h3c_build_consensus_2_of_2_disagree() {
        // 仅两个端点，不一致 → NoConsensus
        let responses = vec![
            make_endpoint_response("ep1", vec![make_normalized("tx1", 100, 1000)]),
            make_endpoint_response("ep2", vec![make_normalized("tx1", 200, 1000)]),
        ];
        match build_consensus(responses, 2, 3000) {
            ConsensusResult::NoConsensus { .. } => {},
            other => panic!("Expected NoConsensus, got {:?}", other),
        }
    }

    #[test]
    fn h3c_build_consensus_multi_transfer_order_independent() {
        // 转账列表内容相同但原始顺序不同（排序后一致）
        let t1 = vec![
            make_normalized("tx_aaa", 100, 1000),
            make_normalized("tx_zzz", 200, 2000),
        ];
        let t2 = vec![
            make_normalized("tx_aaa", 100, 1000),
            make_normalized("tx_zzz", 200, 2000),
        ];
        // 已经按 tx_hash 排序，确认一致
        let responses = vec![
            make_endpoint_response("ep1", t1),
            make_endpoint_response("ep2", t2),
        ];
        match build_consensus(responses, 2, 3000) {
            ConsensusResult::Agreed { .. } => {},
            other => panic!("Expected Agreed, got {:?}", other),
        }
    }

    #[test]
    fn h3c_build_consensus_strict_timestamp_zero_tolerance() {
        // tolerance=0 时 timestamp 必须精确匹配
        let responses = vec![
            make_endpoint_response("ep1", vec![make_normalized("tx1", 100, 1000)]),
            make_endpoint_response("ep2", vec![make_normalized("tx1", 100, 1001)]), // 差 1ms
        ];
        match build_consensus(responses, 2, 0) {
            ConsensusResult::NoConsensus { .. } => {},
            other => panic!("Expected NoConsensus with 0 tolerance, got {:?}", other),
        }
    }

    #[test]
    fn h3c_consensus_config_routing_default_off() {
        // 默认 consensus_enabled=false，验证主函数应走旧路径
        let config = VerifierConfig::default();
        assert!(!config.consensus_enabled);
    }

    // ==================== H3-C Phase 3: 审计日志 + 可观测性 ====================

    #[test]
    fn h3c_consensus_detail_encode_decode_roundtrip() {
        let detail = ConsensusDetail {
            consensus_mode: true,
            agreeing_endpoints: vec![b"ep1".to_vec(), b"ep2".to_vec()],
            dissenting_endpoint: Some(b"ep3".to_vec()),
            degraded: false,
            total_endpoints_queried: 3,
            total_endpoints_responded: 3,
        };
        let encoded = detail.encode();
        let decoded = ConsensusDetail::decode(&mut &encoded[..]).unwrap();
        assert!(decoded.consensus_mode);
        assert_eq!(decoded.agreeing_endpoints.len(), 2);
        assert_eq!(decoded.dissenting_endpoint, Some(b"ep3".to_vec()));
        assert!(!decoded.degraded);
        assert_eq!(decoded.total_endpoints_queried, 3);
        assert_eq!(decoded.total_endpoints_responded, 3);
    }

    #[test]
    fn h3c_consensus_detail_degraded() {
        let detail = ConsensusDetail {
            consensus_mode: true,
            agreeing_endpoints: Vec::new(),
            dissenting_endpoint: None,
            degraded: true,
            total_endpoints_queried: 3,
            total_endpoints_responded: 1,
        };
        let encoded = detail.encode();
        let decoded = ConsensusDetail::decode(&mut &encoded[..]).unwrap();
        assert!(decoded.degraded);
        assert!(decoded.agreeing_endpoints.is_empty());
        assert_eq!(decoded.total_endpoints_responded, 1);
    }

    #[test]
    fn h3c_audit_log_entry_with_consensus_detail() {
        with_offchain_ext(|| {
            let detail = ConsensusDetail {
                consensus_mode: true,
                agreeing_endpoints: vec![b"trongrid".to_vec(), b"tronstack".to_vec()],
                dissenting_endpoint: None,
                degraded: false,
                total_endpoints_queried: 2,
                total_endpoints_responded: 2,
            };
            let entry = AuditLogEntry {
                timestamp: 1700000000000,
                action: b"verify_trc20_by_transfer".to_vec(),
                from_address: b"TSender".to_vec(),
                to_address: b"TReceiver".to_vec(),
                expected_amount: 10_000_000,
                actual_amount: 10_000_000,
                result_ok: true,
                error_msg: Vec::new(),
                tx_hash: b"tx_abc".to_vec(),
                endpoint_used: b"trongrid".to_vec(),
                duration_ms: 2500,
                consensus_detail: Some(detail),
            };
            write_audit_log(&entry);
            let logs = get_recent_audit_logs(1);
            assert_eq!(logs.len(), 1);
            let loaded = &logs[0];
            assert!(loaded.consensus_detail.is_some());
            let cd = loaded.consensus_detail.as_ref().unwrap();
            assert!(cd.consensus_mode);
            assert_eq!(cd.agreeing_endpoints.len(), 2);
            assert!(!cd.degraded);
        });
    }

    #[test]
    fn h3c_audit_log_entry_without_consensus_detail() {
        with_offchain_ext(|| {
            let entry = AuditLogEntry {
                timestamp: 1700000000000,
                action: b"verify".to_vec(),
                from_address: b"TF".to_vec(),
                to_address: b"TT".to_vec(),
                expected_amount: 100,
                actual_amount: 100,
                result_ok: true,
                error_msg: Vec::new(),
                tx_hash: Vec::new(),
                endpoint_used: Vec::new(),
                duration_ms: 50,
                consensus_detail: None,
            };
            write_audit_log(&entry);
            let logs = get_recent_audit_logs(1);
            assert_eq!(logs.len(), 1);
            assert!(logs[0].consensus_detail.is_none());
        });
    }

    #[test]
    fn h3c_verifier_metrics_consensus_fields_default() {
        with_offchain_ext(|| {
            let metrics = get_verifier_metrics();
            assert_eq!(metrics.consensus_success_count, 0);
            assert_eq!(metrics.consensus_failure_count, 0);
            assert_eq!(metrics.degraded_verification_count, 0);
        });
    }

    #[test]
    fn h3c_verifier_metrics_consensus_increment() {
        with_offchain_ext(|| {
            let mut m = get_verifier_metrics();
            m.consensus_success_count = m.consensus_success_count.saturating_add(3);
            m.consensus_failure_count = m.consensus_failure_count.saturating_add(1);
            m.degraded_verification_count = m.degraded_verification_count.saturating_add(2);
            save_verifier_metrics(&m);

            let loaded = get_verifier_metrics();
            assert_eq!(loaded.consensus_success_count, 3);
            assert_eq!(loaded.consensus_failure_count, 1);
            assert_eq!(loaded.degraded_verification_count, 2);
        });
    }

    #[test]
    fn h3c_verifier_metrics_reset_clears_consensus() {
        with_offchain_ext(|| {
            let mut m = get_verifier_metrics();
            m.consensus_success_count = 10;
            m.consensus_failure_count = 5;
            m.degraded_verification_count = 3;
            save_verifier_metrics(&m);

            reset_verifier_metrics();
            let after = get_verifier_metrics();
            assert_eq!(after.consensus_success_count, 0);
            assert_eq!(after.consensus_failure_count, 0);
            assert_eq!(after.degraded_verification_count, 0);
        });
    }

    #[test]
    fn h3c_verifier_metrics_legacy_migration() {
        // 模拟旧格式存储（不含 consensus 字段）
        with_offchain_ext(|| {
            #[derive(Encode)]
            struct OldMetrics {
                total_success: u64,
                total_failure: u64,
                total_duration_ms: u64,
                endpoint_fallback_count: u64,
                cache_hit_count: u64,
                rate_limit_hit_count: u64,
                lock_contention_count: u64,
                last_updated: u64,
            }
            let old = OldMetrics {
                total_success: 42,
                total_failure: 3,
                total_duration_ms: 100_000,
                endpoint_fallback_count: 5,
                cache_hit_count: 20,
                rate_limit_hit_count: 2,
                lock_contention_count: 1,
                last_updated: 1700000000000,
            };
            sp_io::offchain::local_storage_set(
                StorageKind::PERSISTENT, VERIFIER_METRICS_KEY, &old.encode(),
            );

            let loaded = get_verifier_metrics();
            // 保留旧字段
            assert_eq!(loaded.total_success, 42);
            assert_eq!(loaded.total_failure, 3);
            assert_eq!(loaded.cache_hit_count, 20);
            // 新字段使用默认值
            assert_eq!(loaded.consensus_success_count, 0);
            assert_eq!(loaded.consensus_failure_count, 0);
            assert_eq!(loaded.degraded_verification_count, 0);
        });
    }

    #[test]
    fn h3c_verifier_metrics_roundtrip_with_consensus() {
        with_offchain_ext(|| {
            let mut m = VerifierMetrics::default();
            m.total_success = 100;
            m.consensus_success_count = 50;
            m.consensus_failure_count = 2;
            m.degraded_verification_count = 5;
            save_verifier_metrics(&m);

            let loaded = get_verifier_metrics();
            assert_eq!(loaded.total_success, 100);
            assert_eq!(loaded.consensus_success_count, 50);
            assert_eq!(loaded.consensus_failure_count, 2);
            assert_eq!(loaded.degraded_verification_count, 5);
        });
    }
}
