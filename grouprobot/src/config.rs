use crate::error::{BotError, BotResult};
use serde::Deserialize;

/// 平台模式
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlatformMode {
    Telegram,
    Discord,
    Both,
}

impl PlatformMode {
    pub fn needs_telegram(&self) -> bool {
        matches!(self, PlatformMode::Telegram | PlatformMode::Both)
    }

    pub fn needs_discord(&self) -> bool {
        matches!(self, PlatformMode::Discord | PlatformMode::Both)
    }
}

/// Discord 配置
#[derive(Debug, Clone)]
pub struct DiscordConfig {
    pub application_id: String,
    pub intents: u64,
}

/// Bot 配置
///
/// ⚠️ 手动实现 Debug: webhook_secret / chain_signer_seed 等敏感字段已脱敏
#[derive(Clone)]
pub struct BotConfig {
    // 平台
    pub platform: PlatformMode,
    pub bot_id_hash: [u8; 32],

    // Discord
    pub discord: Option<DiscordConfig>,

    // Webhook
    pub webhook_port: u16,
    pub webhook_url: String,
    pub webhook_secret: String,

    // 链上
    pub chain_rpc: String,
    pub chain_signer_seed: Option<String>,

    // TEE
    pub tee_mode: String,
    pub data_dir: String,

    // Vault 进程模式: "inprocess" | "spawn" | "connect"
    pub vault_mode: String,
    /// Vault Unix socket 路径 (用于 spawn/connect 模式)
    pub vault_socket: String,

    // Shamir Share 恢复
    /// Shamir 门限 K (需要多少个 share 才能恢复)
    pub shamir_threshold: u8,
    /// Peer TEE 节点端点列表 (RA-TLS, 用于收集 K-1 个 share)
    pub peer_endpoints: Vec<String>,
    /// Ceremony share 接收端口 (0 = 禁用)
    pub ceremony_port: u16,

    // 性能
    pub webhook_rate_limit: u32,
    pub execute_rate_limit: u32,
    pub chain_log_batch_interval: u64,
    pub chain_log_batch_size: usize,

    // 监控
    pub metrics_enabled: bool,
    pub log_level: String,
}

impl std::fmt::Debug for BotConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BotConfig")
            .field("platform", &self.platform)
            .field("bot_id_hash", &hex::encode(self.bot_id_hash))
            .field("webhook_port", &self.webhook_port)
            .field("webhook_url", &self.webhook_url)
            .field("webhook_secret", &if self.webhook_secret.is_empty() { "<empty>" } else { "<REDACTED>" })
            .field("chain_rpc", &self.chain_rpc)
            .field("chain_signer_seed", &if self.chain_signer_seed.is_some() { "<REDACTED>" } else { "<none>" })
            .field("tee_mode", &self.tee_mode)
            .field("data_dir", &self.data_dir)
            .field("vault_mode", &self.vault_mode)
            .field("shamir_threshold", &self.shamir_threshold)
            .field("peer_endpoints", &self.peer_endpoints.len())
            .field("ceremony_port", &self.ceremony_port)
            .field("metrics_enabled", &self.metrics_enabled)
            .field("log_level", &self.log_level)
            .finish()
    }
}

impl BotConfig {
    pub fn from_env() -> BotResult<Self> {
        let platform_str = std::env::var("PLATFORM").unwrap_or_else(|_| "telegram".into());
        let platform = match platform_str.to_lowercase().as_str() {
            "telegram" => PlatformMode::Telegram,
            "discord" => PlatformMode::Discord,
            "both" => PlatformMode::Both,
            other => return Err(BotError::Config(format!("Unknown platform: {}", other))),
        };

        let bot_id_hash = Self::parse_bot_id_hash()?;

        let discord = if platform.needs_discord() {
            Some(DiscordConfig {
                application_id: std::env::var("DISCORD_APPLICATION_ID")
                    .map_err(|_| BotError::Config("DISCORD_APPLICATION_ID required".into()))?,
                intents: std::env::var("DISCORD_INTENTS")
                    .unwrap_or_else(|_| "33281".into())
                    .parse()
                    .unwrap_or(33281),
            })
        } else {
            None
        };

        Ok(Self {
            platform,
            bot_id_hash,
            discord,
            webhook_port: std::env::var("WEBHOOK_PORT")
                .unwrap_or_else(|_| "3000".into())
                .parse()
                .unwrap_or(3000),
            webhook_url: std::env::var("WEBHOOK_URL").unwrap_or_default(),
            webhook_secret: std::env::var("WEBHOOK_SECRET").unwrap_or_default(),
            chain_rpc: std::env::var("CHAIN_RPC")
                .unwrap_or_else(|_| "ws://127.0.0.1:9944".into()),
            chain_signer_seed: std::env::var("CHAIN_SIGNER_SEED").ok(),
            tee_mode: std::env::var("TEE_MODE").unwrap_or_else(|_| "auto".into()),
            data_dir: std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".into()),
            vault_mode: std::env::var("VAULT_MODE").unwrap_or_else(|_| "inprocess".into()),
            vault_socket: std::env::var("VAULT_SOCKET").unwrap_or_default(),
            shamir_threshold: std::env::var("SHAMIR_THRESHOLD")
                .unwrap_or_else(|_| "1".into())
                .parse()
                .unwrap_or(1),
            peer_endpoints: std::env::var("PEER_ENDPOINTS")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.trim().to_string())
                .collect(),
            ceremony_port: std::env::var("CEREMONY_PORT")
                .unwrap_or_else(|_| "0".into())
                .parse()
                .unwrap_or(0),
            webhook_rate_limit: std::env::var("WEBHOOK_RATE_LIMIT")
                .unwrap_or_else(|_| "200".into())
                .parse()
                .unwrap_or(200),
            execute_rate_limit: std::env::var("EXECUTE_RATE_LIMIT")
                .unwrap_or_else(|_| "100".into())
                .parse()
                .unwrap_or(100),
            chain_log_batch_interval: std::env::var("CHAIN_LOG_BATCH_INTERVAL")
                .unwrap_or_else(|_| "6".into())
                .parse()
                .unwrap_or(6),
            chain_log_batch_size: std::env::var("CHAIN_LOG_BATCH_SIZE")
                .unwrap_or_else(|_| "50".into())
                .parse()
                .unwrap_or(50),
            metrics_enabled: std::env::var("METRICS_ENABLED")
                .unwrap_or_else(|_| "true".into())
                .parse()
                .unwrap_or(true),
            log_level: std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into()),
        })
    }

    fn parse_bot_id_hash() -> BotResult<[u8; 32]> {
        if let Ok(hash_str) = std::env::var("BOT_ID_HASH") {
            let stripped = hash_str.strip_prefix("0x").unwrap_or(&hash_str);
            let bytes = hex::decode(stripped)
                .map_err(|e| BotError::Config(format!("Invalid BOT_ID_HASH: {}", e)))?;
            if bytes.len() != 32 {
                return Err(BotError::Config(format!(
                    "BOT_ID_HASH must be 32 bytes, got {}",
                    bytes.len()
                )));
            }
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&bytes);
            Ok(hash)
        } else if let Ok(token) = std::env::var("BOT_TOKEN") {
            // 过渡: 从 BOT_TOKEN 派生 (将来移除)
            use sha2::{Sha256, Digest};
            use zeroize::Zeroize;
            let mut token_buf = token;
            let mut hasher = Sha256::new();
            hasher.update(token_buf.as_bytes());
            let result = hasher.finalize();
            token_buf.zeroize(); // 立即清零 token 明文
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&result);
            Ok(hash)
        } else {
            // 无 BOT_ID_HASH 也无 BOT_TOKEN: 使用零值, ShareRecovery 恢复后可覆盖
            Ok([0u8; 32])
        }
    }

    pub fn bot_id_hash_hex(&self) -> String {
        hex::encode(self.bot_id_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_mode_telegram() {
        let p = PlatformMode::Telegram;
        assert!(p.needs_telegram());
        assert!(!p.needs_discord());
    }

    #[test]
    fn platform_mode_both() {
        let p = PlatformMode::Both;
        assert!(p.needs_telegram());
        assert!(p.needs_discord());
    }

    #[test]
    fn debug_redacts_secrets() {
        let cfg = BotConfig {
            platform: PlatformMode::Telegram,
            bot_id_hash: [0xAB; 32],
            discord: None,
            webhook_port: 3000,
            webhook_url: "https://example.com/webhook".into(),
            webhook_secret: "super_secret_value_123".into(),
            chain_rpc: "ws://127.0.0.1:9944".into(),
            chain_signer_seed: Some("0xdeadbeef1234567890".into()),
            tee_mode: "software".into(),
            data_dir: "./data".into(),
            vault_mode: "inprocess".into(),
            vault_socket: String::new(),
            shamir_threshold: 1,
            peer_endpoints: vec![],
            ceremony_port: 0,
            webhook_rate_limit: 200,
            execute_rate_limit: 100,
            chain_log_batch_interval: 6,
            chain_log_batch_size: 50,
            metrics_enabled: true,
            log_level: "info".into(),
        };
        let debug_output = format!("{:?}", cfg);
        // webhook_secret 和 chain_signer_seed 的实际值不应出现
        assert!(!debug_output.contains("super_secret_value_123"),
            "webhook_secret leaked in Debug output: {}", debug_output);
        assert!(!debug_output.contains("deadbeef1234567890"),
            "chain_signer_seed leaked in Debug output: {}", debug_output);
        // 应显示 <REDACTED>
        assert!(debug_output.contains("<REDACTED>"));
    }
}
