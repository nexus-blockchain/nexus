// Token 安全容器 — Token 永不以原始 String 形式暴露到容器外部
//
// 安全属性:
// - Token 使用 Zeroizing<String> 包装, drop 时内存自动清零
// - 不实现 Debug/Display, 防止日志泄露
// - build_* 方法返回 Zeroizing<String>, 调用方用完即清零
// - 不提供 get_token() 方法, Token 永远不以原始形式暴露
//
// Software 模式: Token 在 Zeroizing<String> 中 (TDX 内存, 微秒级存在)
// Hardware 模式 (未来): Token 在 SGX Enclave 中, 通过 ecall 拼接

use tracing::debug;
use zeroize::Zeroizing;

use crate::error::{BotError, BotResult};
use crate::tee::mem_security;

/// Token 安全容器
pub struct TokenVault {
    /// Telegram Bot Token (Zeroizing 包装, drop 时自动清零)
    tg_token: Option<Zeroizing<String>>,
    /// Discord Bot Token (Zeroizing 包装)
    dc_token: Option<Zeroizing<String>>,
    /// Telegram API ID (Local Bot API Server 用, Zeroizing 包装)
    tg_api_id: Option<Zeroizing<String>>,
    /// Telegram API Hash (Local Bot API Server 用, Zeroizing 包装)
    tg_api_hash: Option<Zeroizing<String>>,
    /// Telegram API Base URL (默认 https://api.telegram.org, Local Server 时改为 http://127.0.0.1:8081)
    tg_api_base_url: Option<String>,
}

impl TokenVault {
    /// 创建空 Vault
    pub fn new() -> Self {
        Self {
            tg_token: None,
            dc_token: None,
            tg_api_id: None,
            tg_api_hash: None,
            tg_api_base_url: None,
        }
    }

    /// 注入 Telegram Token (仅调用一次)
    ///
    /// 接受 String 或 Zeroizing<String>, 内部统一以 Zeroizing 保护
    pub fn set_telegram_token(&mut self, token: impl Into<Zeroizing<String>>) {
        let z: Zeroizing<String> = token.into();
        // mlock: 锁定 Token 内存页, 防止被 swap 到磁盘
        if mem_security::mlock_bytes(z.as_bytes()) {
            debug!("Telegram token memory locked (mlock)");
        }
        self.tg_token = Some(z);
    }

    /// 注入 Discord Token (仅调用一次)
    ///
    /// 接受 String 或 Zeroizing<String>, 内部统一以 Zeroizing 保护
    pub fn set_discord_token(&mut self, token: impl Into<Zeroizing<String>>) {
        let z: Zeroizing<String> = token.into();
        if mem_security::mlock_bytes(z.as_bytes()) {
            debug!("Discord token memory locked (mlock)");
        }
        self.dc_token = Some(z);
    }

    /// 注入 Telegram API credentials (api_id + api_hash, Local Bot API Server 用)
    ///
    /// api_id/api_hash 比 bot token 更敏感 — 泄露可影响开发者账号全部 bot
    pub fn set_telegram_api_credentials(
        &mut self,
        api_id: impl Into<Zeroizing<String>>,
        api_hash: impl Into<Zeroizing<String>>,
    ) {
        let id: Zeroizing<String> = api_id.into();
        let hash: Zeroizing<String> = api_hash.into();
        if mem_security::mlock_bytes(id.as_bytes()) {
            debug!("Telegram API ID memory locked (mlock)");
        }
        if mem_security::mlock_bytes(hash.as_bytes()) {
            debug!("Telegram API hash memory locked (mlock)");
        }
        self.tg_api_id = Some(id);
        self.tg_api_hash = Some(hash);
    }

    /// 设置 Telegram API Base URL (Local Bot API Server 地址)
    ///
    /// 默认 https://api.telegram.org; 部署 Local Server 时改为 http://127.0.0.1:8081
    pub fn set_tg_api_base_url(&mut self, url: String) {
        self.tg_api_base_url = Some(url);
    }

    /// 获取 API credentials 引用 (用于启动 Local Bot API Server sidecar)
    #[allow(dead_code)]
    pub fn tg_api_credentials(&self) -> Option<(&str, &str)> {
        match (&self.tg_api_id, &self.tg_api_hash) {
            (Some(id), Some(hash)) => Some((id.as_str(), hash.as_str())),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn has_tg_api_credentials(&self) -> bool {
        self.tg_api_id.is_some() && self.tg_api_hash.is_some()
    }

    /// 构建 Telegram API URL (Token 在内部拼接, 返回 Zeroizing<String>)
    ///
    /// base URL 可配置: 默认为 Telegram 官方, 部署 Local Server 时自动切换
    pub fn build_tg_api_url(&self, method: &str) -> BotResult<Zeroizing<String>> {
        let token = self.tg_token.as_ref().ok_or_else(|| {
            BotError::EnclaveError("Telegram token not set in vault".into())
        })?;
        let base = self.tg_api_base_url.as_deref()
            .unwrap_or("https://api.telegram.org");
        Ok(Zeroizing::new(format!(
            "{}/bot{}/{}",
            base,
            token.as_str(),
            method
        )))
    }

    /// 构建 Discord Authorization header 值: "Bot <token>"
    ///
    /// 返回值 drop 时自动清零堆内存
    pub fn build_dc_auth_header(&self) -> BotResult<Zeroizing<String>> {
        let token = self.dc_token.as_ref().ok_or_else(|| {
            BotError::EnclaveError("Discord token not set in vault".into())
        })?;
        Ok(Zeroizing::new(format!("Bot {}", token.as_str())))
    }

    /// 构建 Discord Gateway IDENTIFY payload JSON
    ///
    /// 返回值 drop 时自动清零堆内存
    pub fn build_dc_identify_payload(&self, intents: u64) -> BotResult<Zeroizing<String>> {
        let token = self.dc_token.as_ref().ok_or_else(|| {
            BotError::EnclaveError("Discord token not set in vault".into())
        })?;
        let payload = serde_json::json!({
            "op": 2,
            "d": {
                "token": token.as_str(),
                "intents": intents,
                "properties": {
                    "os": "linux",
                    "browser": "grouprobot",
                    "device": "grouprobot"
                }
            }
        });
        Ok(Zeroizing::new(payload.to_string()))
    }

    /// 从 Telegram token 派生 bot_id_hash (SHA256)
    pub fn derive_tg_bot_id_hash(&self) -> BotResult<[u8; 32]> {
        let token = self.tg_token.as_ref().ok_or_else(|| {
            BotError::EnclaveError("Telegram token not set in vault".into())
        })?;
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        Ok(hash)
    }

    /// 安全清零所有 Token 和 credentials
    pub fn zeroize_all(&mut self) {
        // munlock before drop (Zeroizing 会清零内容, 然后 munlock 释放页锁)
        if let Some(ref t) = self.tg_token {
            mem_security::munlock_bytes(t.as_bytes());
        }
        if let Some(ref t) = self.dc_token {
            mem_security::munlock_bytes(t.as_bytes());
        }
        if let Some(ref t) = self.tg_api_id {
            mem_security::munlock_bytes(t.as_bytes());
        }
        if let Some(ref t) = self.tg_api_hash {
            mem_security::munlock_bytes(t.as_bytes());
        }
        self.tg_token = None;
        self.dc_token = None;
        self.tg_api_id = None;
        self.tg_api_hash = None;
        self.tg_api_base_url = None;
    }

    /// 检查 Telegram token 是否已设置
    #[allow(dead_code)]
    pub fn has_telegram_token(&self) -> bool {
        self.tg_token.is_some()
    }

    /// 检查 Discord token 是否已设置
    #[allow(dead_code)]
    pub fn has_discord_token(&self) -> bool {
        self.dc_token.is_some()
    }
}

// 显式不实现 Debug — 防止 Token 通过 {:?} 泄露到日志
// 显式不实现 Clone — 防止 Token 被复制

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_tg_api_url_default_base() {
        let mut vault = TokenVault::new();
        vault.set_telegram_token("123456:ABCDEF".to_string());
        let url = vault.build_tg_api_url("sendMessage").unwrap();
        assert_eq!(
            url.as_str(),
            "https://api.telegram.org/bot123456:ABCDEF/sendMessage"
        );
    }

    #[test]
    fn build_tg_api_url_custom_base() {
        let mut vault = TokenVault::new();
        vault.set_telegram_token("123456:ABCDEF".to_string());
        vault.set_tg_api_base_url("http://127.0.0.1:8081".into());
        let url = vault.build_tg_api_url("sendMessage").unwrap();
        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:8081/bot123456:ABCDEF/sendMessage"
        );
    }

    #[test]
    fn build_dc_auth_header() {
        let mut vault = TokenVault::new();
        vault.set_discord_token("my-dc-token".to_string());
        let header = vault.build_dc_auth_header().unwrap();
        assert_eq!(header.as_str(), "Bot my-dc-token");
    }

    #[test]
    fn build_dc_identify_payload() {
        let mut vault = TokenVault::new();
        vault.set_discord_token("dc-token-123".to_string());
        let payload = vault.build_dc_identify_payload(33281).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(parsed["op"], 2);
        assert_eq!(parsed["d"]["token"], "dc-token-123");
        assert_eq!(parsed["d"]["intents"], 33281);
    }

    #[test]
    fn no_token_returns_error() {
        let vault = TokenVault::new();
        assert!(vault.build_tg_api_url("test").is_err());
        assert!(vault.build_dc_auth_header().is_err());
        assert!(vault.build_dc_identify_payload(0).is_err());
    }

    #[test]
    fn zeroize_all_clears_tokens() {
        let mut vault = TokenVault::new();
        vault.set_telegram_token("tg-token".to_string());
        vault.set_discord_token("dc-token".to_string());
        assert!(vault.has_telegram_token());
        assert!(vault.has_discord_token());

        vault.zeroize_all();
        assert!(!vault.has_telegram_token());
        assert!(!vault.has_discord_token());
        assert!(vault.build_tg_api_url("test").is_err());
    }

    #[test]
    fn derive_bot_id_hash() {
        let mut vault = TokenVault::new();
        vault.set_telegram_token("123456:ABCDEF".to_string());
        let hash = vault.derive_tg_bot_id_hash().unwrap();
        assert_eq!(hash.len(), 32);
        assert_ne!(hash, [0u8; 32]);

        // 确定性
        let hash2 = vault.derive_tg_bot_id_hash().unwrap();
        assert_eq!(hash, hash2);
    }

    #[test]
    fn set_and_get_api_credentials() {
        let mut vault = TokenVault::new();
        assert!(!vault.has_tg_api_credentials());
        assert!(vault.tg_api_credentials().is_none());

        vault.set_telegram_api_credentials("12345".to_string(), "abc123hash".to_string());
        assert!(vault.has_tg_api_credentials());
        let (id, hash) = vault.tg_api_credentials().unwrap();
        assert_eq!(id, "12345");
        assert_eq!(hash, "abc123hash");
    }

    #[test]
    fn zeroize_all_clears_api_credentials() {
        let mut vault = TokenVault::new();
        vault.set_telegram_api_credentials("12345".to_string(), "abc123hash".to_string());
        vault.set_tg_api_base_url("http://localhost:8081".into());
        assert!(vault.has_tg_api_credentials());

        vault.zeroize_all();
        assert!(!vault.has_tg_api_credentials());
        assert!(vault.tg_api_credentials().is_none());
    }
}
