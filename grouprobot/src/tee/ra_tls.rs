// RA-TLS 端到端加密 Token 注入 — 管理员 DApp → TEE Enclave
//
// 安全属性:
// - Enclave 生成临时 X25519 密钥对, 公钥绑定在 TDX Quote 的 report_data 中
// - DApp 验证 Quote (MRTD 白名单) → 用 Enclave PK 加密 Token
// - 中间代理 / CDN / 反向代理只能看到密文
// - Enclave 用 X25519 私钥解密 → 注入 TokenVault + auto-seal
//
// 端点:
//   GET  /provision/attestation   → { quote, enclave_pk, mrtd }
//   POST /provision/inject-token  → { ephemeral_pk, ciphertext, nonce, platform }
//
// 生命周期:
//   1. DApp 调用 GET /provision/attestation 获取 Quote + Enclave PK
//   2. DApp 端验证 Quote (链上 MRTD 白名单 / Intel PCCS)
//   3. DApp 用 Enclave PK + 临时密钥 ECDH → AES-256-GCM 加密 Token
//   4. DApp 调用 POST /provision/inject-token 提交密文
//   5. Enclave 解密 → 注入 TokenVault → auto-seal 为 Shamir share
//   6. 临时密钥对销毁, 后续启动从 sealed share 恢复

use std::sync::Arc;

use axum::{routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use zeroize::Zeroizing;

use crate::error::{BotError, BotResult};
use crate::tee::enclave_bridge::EnclaveBridge;
use crate::tee::token_vault::TokenVault;
use crate::tee::share_recovery;

// ═══════════════════════════════════════════════════════════════
// 协议类型
// ═══════════════════════════════════════════════════════════════

/// GET /provision/attestation 响应
#[derive(Serialize)]
pub struct AttestationResponse {
    /// TDX Quote (base64)
    pub quote: String,
    /// Enclave X25519 公钥 (hex, 32 bytes) — 绑定在 Quote report_data[0..32]
    pub enclave_pk: String,
    /// MRTD (hex, 48 bytes) — DApp 可用于链上白名单验证
    pub mrtd: String,
    /// TEE 模式
    pub tee_mode: String,
    /// 会话 ID (hex) — 用于关联后续 inject 请求
    pub session_id: String,
}

/// POST /provision/inject-token 请求
#[derive(Deserialize)]
pub struct InjectTokenRequest {
    /// DApp 临时 X25519 公钥 (hex, 32 bytes)
    pub ephemeral_pk: String,
    /// AES-256-GCM 加密后的 Token (base64)
    pub ciphertext: String,
    /// AES-256-GCM nonce (base64, 12 bytes)
    pub nonce: String,
    /// 平台: "telegram" | "discord"
    pub platform: String,
    /// 会话 ID (hex) — 必须匹配 attestation 阶段返回的 session_id
    pub session_id: String,
}

/// POST /provision/inject-token 响应
#[derive(Serialize)]
pub struct InjectTokenResponse {
    pub success: bool,
    pub message: String,
    /// Bot ID Hash (hex) — 仅 Telegram, 用于链上注册
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bot_id_hash: Option<String>,
}

/// 错误响应
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

// ═══════════════════════════════════════════════════════════════
// ProvisionSession — 临时会话状态
// ═══════════════════════════════════════════════════════════════

/// 一次 Provision 会话 (GET attestation → POST inject 之间)
struct ProvisionSession {
    /// 会话 ID (随机 16 bytes)
    session_id: [u8; 16],
    /// 临时 X25519 私钥 (会话结束后 zeroize)
    x25519_secret: x25519_dalek::StaticSecret,
    /// 临时 X25519 公钥
    x25519_public: x25519_dalek::PublicKey,
    /// 创建时间
    created_at: std::time::Instant,
    /// 是否已使用 (一次性)
    used: bool,
}

impl ProvisionSession {
    fn new() -> Self {
        use rand::RngCore;
        let mut session_id = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut session_id);

        let x25519_secret = x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng);
        let x25519_public = x25519_dalek::PublicKey::from(&x25519_secret);

        Self {
            session_id,
            x25519_secret,
            x25519_public,
            created_at: std::time::Instant::now(),
            used: false,
        }
    }

    fn session_id_hex(&self) -> String {
        hex::encode(self.session_id)
    }

    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > std::time::Duration::from_secs(SESSION_TIMEOUT_SECS)
    }
}

/// 会话超时 (秒)
const SESSION_TIMEOUT_SECS: u64 = 300; // 5 分钟
/// 最大并发会话数
const MAX_SESSIONS: usize = 8;

// ═══════════════════════════════════════════════════════════════
// ProvisionState — 共享状态
// ═══════════════════════════════════════════════════════════════

/// Provision 端点共享状态
pub struct ProvisionState {
    enclave: Arc<EnclaveBridge>,
    /// 活跃会话 (受 RwLock 保护)
    sessions: RwLock<Vec<ProvisionSession>>,
    /// TokenVault (注入目标, 仅 inprocess 模式)
    /// None = vault 在外部进程中 (需要通过 VaultClient IPC 注入)
    vault: Option<Arc<RwLock<TokenVault>>>,
}

impl ProvisionState {
    pub fn new(enclave: Arc<EnclaveBridge>, vault: Option<Arc<RwLock<TokenVault>>>) -> Self {
        Self {
            enclave,
            sessions: RwLock::new(Vec::new()),
            vault,
        }
    }

    /// 创建新会话, 返回 (session, Quote, MRTD)
    async fn create_session(&self) -> BotResult<(String, Vec<u8>, [u8; 48])> {
        let mut sessions = self.sessions.write().await;

        // 清理过期会话
        sessions.retain(|s| !s.is_expired());

        // 限制并发会话数
        if sessions.len() >= MAX_SESSIONS {
            return Err(BotError::EnclaveError(
                "too many active provision sessions (max 8)".into()
            ));
        }

        let session = ProvisionSession::new();
        let session_id_hex = session.session_id_hex();
        let pk_bytes = session.x25519_public.to_bytes();

        // 将 X25519 PK 绑定到 report_data → 生成 TDX Quote
        // report_data[0..32] = SHA256(x25519_pk)
        // 这样 DApp 可以验证: Quote.report_data 确实绑定了这个公钥
        let mut hasher = Sha256::new();
        hasher.update(b"ra-tls-provision-v1:");
        hasher.update(pk_bytes);
        let pk_hash: [u8; 32] = hasher.finalize().into();

        let quote = self.generate_provision_quote(&pk_hash)?;
        let mrtd = Self::extract_mrtd_from_quote(&quote);

        sessions.push(session);

        info!(
            session_id = %session_id_hex,
            x25519_pk = %hex::encode(pk_bytes),
            "Provision 会话已创建"
        );

        Ok((session_id_hex, quote, mrtd))
    }

    /// 消费会话并解密 Token
    async fn consume_session(
        &self,
        session_id_hex: &str,
        ephemeral_pk_hex: &str,
        ciphertext_b64: &str,
        nonce_b64: &str,
    ) -> BotResult<Zeroizing<String>> {
        let mut sessions = self.sessions.write().await;

        // 查找匹配的会话
        let idx = sessions.iter().position(|s| s.session_id_hex() == session_id_hex)
            .ok_or_else(|| BotError::EnclaveError("invalid or expired session_id".into()))?;

        // 检查过期
        if sessions[idx].is_expired() {
            sessions.remove(idx);
            return Err(BotError::EnclaveError("session expired".into()));
        }

        // 检查一次性使用
        if sessions[idx].used {
            sessions.remove(idx);
            return Err(BotError::EnclaveError("session already used".into()));
        }

        // 标记已使用
        sessions[idx].used = true;

        // 解析 DApp 临时公钥
        let ephemeral_pk_bytes = hex::decode(ephemeral_pk_hex)
            .map_err(|_| BotError::EnclaveError("invalid ephemeral_pk hex".into()))?;
        if ephemeral_pk_bytes.len() != 32 {
            sessions.remove(idx);
            return Err(BotError::EnclaveError("ephemeral_pk must be 32 bytes".into()));
        }
        let mut epk = [0u8; 32];
        epk.copy_from_slice(&ephemeral_pk_bytes);
        let dapp_pk = x25519_dalek::PublicKey::from(epk);

        // ECDH → shared secret → KDF → AES key
        let shared_secret = sessions[idx].x25519_secret.diffie_hellman(&dapp_pk);

        let mut hasher = Sha256::new();
        hasher.update(shared_secret.as_bytes());
        hasher.update(b"ra-tls-token-provision-v1");
        let aes_key: [u8; 32] = hasher.finalize().into();

        // 解码 ciphertext + nonce
        use base64::Engine;
        let ciphertext = base64::engine::general_purpose::STANDARD.decode(ciphertext_b64)
            .map_err(|_| BotError::EnclaveError("invalid ciphertext base64".into()))?;
        let nonce_bytes = base64::engine::general_purpose::STANDARD.decode(nonce_b64)
            .map_err(|_| BotError::EnclaveError("invalid nonce base64".into()))?;
        if nonce_bytes.len() != 12 {
            sessions.remove(idx);
            return Err(BotError::EnclaveError("nonce must be 12 bytes".into()));
        }

        // AES-256-GCM 解密
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
        let cipher = Aes256Gcm::new_from_slice(&aes_key)
            .map_err(|_| BotError::EnclaveError("AES key init failed".into()))?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let plaintext = cipher.decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| {
                warn!("Token 解密失败 — 可能是密钥不匹配或数据被篡改");
                BotError::EnclaveError("AES-GCM decryption failed (wrong key or tampered data)".into())
            })?;

        // 转为 UTF-8 String
        let token = String::from_utf8(plaintext)
            .map_err(|_| BotError::EnclaveError("decrypted data is not valid UTF-8".into()))?;

        // 销毁会话 (包括 X25519 私钥)
        sessions.remove(idx);

        info!(session_id = %session_id_hex, "Token 解密成功, 会话已销毁");

        Ok(Zeroizing::new(token))
    }

    /// 生成绑定 provision PK 的 TDX Quote
    fn generate_provision_quote(&self, pk_hash: &[u8; 32]) -> BotResult<Vec<u8>> {
        use crate::tee::enclave_bridge::TeeMode;
        match self.enclave.mode() {
            TeeMode::Hardware => {
                // 写入 report_data → 读取 TDX Quote
                let mut report_data = [0u8; 64];
                report_data[..32].copy_from_slice(pk_hash);
                // report_data[32..64] = 0 (无 nonce, provision 不需要)

                std::fs::write("/dev/attestation/user_report_data", &report_data)
                    .map_err(|e| BotError::EnclaveError(format!("write report_data: {}", e)))?;
                let quote = std::fs::read("/dev/attestation/quote")
                    .map_err(|e| BotError::EnclaveError(format!("read quote: {}", e)))?;
                Ok(quote)
            }
            TeeMode::Software => {
                // 软件模式: 模拟 Quote (包含 pk_hash 在 report_data 位置)
                // 前端开发/测试用, 生产环境必须是 Hardware 模式
                let mut mock_quote = vec![0u8; 256];
                // 模拟 header (4 bytes version)
                mock_quote[0..4].copy_from_slice(&4u32.to_le_bytes());
                // 模拟 report_data (offset 568 in real TDX Quote, 此处简化为 offset 48)
                if mock_quote.len() >= 80 {
                    mock_quote[48..80].copy_from_slice(pk_hash);
                }
                // 模拟 MRTD (offset 184 in real TDX Quote, 此处简化为 offset 80)
                // 填充固定值用于测试
                for i in 80..128 {
                    mock_quote[i] = 0xAA;
                }
                Ok(mock_quote)
            }
        }
    }

    /// 从 Quote 提取 MRTD (48 bytes)
    fn extract_mrtd_from_quote(quote: &[u8]) -> [u8; 48] {
        let mut mrtd = [0u8; 48];
        // TDX Quote v4: Body starts at offset 48, MRTD at Body + 136 = offset 184
        if quote.len() >= 232 {
            mrtd.copy_from_slice(&quote[184..232]);
        }
        mrtd
    }

    /// 注入 Token 到 Vault + auto-seal
    async fn inject_token(
        &self,
        token: Zeroizing<String>,
        platform: &str,
    ) -> BotResult<Option<[u8; 32]>> {
        let mut bot_id_hash = None;

        if let Some(ref vault) = self.vault {
            // inprocess 模式: 直接注入
            let mut v = vault.write().await;
            match platform {
                "telegram" => {
                    v.set_telegram_token(token.to_string());
                    // 计算 bot_id_hash
                    let mut hasher = Sha256::new();
                    hasher.update(token.as_bytes());
                    let hash: [u8; 32] = hasher.finalize().into();
                    bot_id_hash = Some(hash);
                    info!("Telegram Token 已注入 TokenVault");
                }
                "discord" => {
                    v.set_discord_token(token.to_string());
                    info!("Discord Token 已注入 TokenVault");
                }
                _ => {
                    return Err(BotError::EnclaveError(
                        format!("unsupported platform: {}", platform)
                    ));
                }
            }
        } else {
            // IPC 模式: 通过 VaultClient 注入 (未来扩展)
            // 当前返回错误, 需要在 VaultIPC 协议中增加 SetToken 请求类型
            return Err(BotError::EnclaveError(
                "IPC vault mode: inject via provision not yet supported (use inprocess or spawn)".into()
            ));
        }

        // Auto-seal: 将 Token 保存为 Shamir share (后续启动无需再次注入)
        let signing_key = [0u8; 32]; // 临时签名密钥, Ceremony 会覆盖
        if let Err(e) = share_recovery::create_and_save_share(
            &self.enclave, token.as_str(), &signing_key, 1, 1, 0,
        ) {
            warn!(error = %e, "Auto-seal 失败 (Token 已注入但未持久化)");
        } else {
            info!("Token 已 auto-seal 为 Shamir share (后续启动从 share 恢复)");
        }

        // 清除环境变量中可能存在的旧 Token
        std::env::remove_var("BOT_TOKEN");
        std::env::remove_var("DISCORD_BOT_TOKEN");

        Ok(bot_id_hash)
    }
}

// ═══════════════════════════════════════════════════════════════
// Axum 路由
// ═══════════════════════════════════════════════════════════════

/// 创建 RA-TLS Provision 路由
///
/// enclave: TEE Enclave 桥接
/// vault: TokenVault (inprocess 模式), None = IPC 模式
pub fn provision_routes(
    enclave: Arc<EnclaveBridge>,
    vault: Option<Arc<RwLock<TokenVault>>>,
) -> Router {
    let state = Arc::new(ProvisionState::new(enclave, vault));

    Router::new()
        .route("/provision/attestation", get({
            let st = state.clone();
            move || {
                let st = st.clone();
                async move { handle_attestation(st).await }
            }
        }))
        .route("/provision/inject-token", post({
            let st = state.clone();
            move |Json(req): Json<InjectTokenRequest>| {
                let st = st.clone();
                async move { handle_inject_token(st, req).await }
            }
        }))
}

/// GET /provision/attestation
///
/// 返回 TDX Quote + Enclave X25519 公钥
/// DApp 用此公钥加密 Token, 中间代理无法解密
async fn handle_attestation(
    state: Arc<ProvisionState>,
) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    match state.create_session().await {
        Ok((session_id, quote, mrtd)) => {
            // 找到刚创建的会话获取公钥
            let sessions = state.sessions.read().await;
            let session = sessions.iter().find(|s| s.session_id_hex() == session_id);
            let enclave_pk = match session {
                Some(s) => hex::encode(s.x25519_public.to_bytes()),
                None => {
                    return (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!(ErrorResponse { error: "session lost".into() })),
                    );
                }
            };
            drop(sessions);

            use base64::Engine;
            let resp = AttestationResponse {
                quote: base64::engine::general_purpose::STANDARD.encode(&quote),
                enclave_pk,
                mrtd: hex::encode(mrtd),
                tee_mode: state.enclave.mode().to_string(),
                session_id,
            };
            (axum::http::StatusCode::OK, Json(serde_json::json!(resp)))
        }
        Err(e) => {
            error!(error = %e, "创建 provision 会话失败");
            (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!(ErrorResponse { error: format!("{}", e) })),
            )
        }
    }
}

/// POST /provision/inject-token
///
/// DApp 提交 ECDH 加密的 Token
/// Enclave 解密 → 注入 TokenVault → auto-seal
async fn handle_inject_token(
    state: Arc<ProvisionState>,
    req: InjectTokenRequest,
) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    // 验证平台
    if req.platform != "telegram" && req.platform != "discord" {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!(ErrorResponse {
                error: "platform must be 'telegram' or 'discord'".into()
            })),
        );
    }

    // 验证 session_id 格式
    if req.session_id.len() != 32 {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!(ErrorResponse {
                error: "session_id must be 32 hex characters".into()
            })),
        );
    }

    // 解密 Token
    let token = match state.consume_session(
        &req.session_id,
        &req.ephemeral_pk,
        &req.ciphertext,
        &req.nonce,
    ).await {
        Ok(t) => t,
        Err(e) => {
            warn!(error = %e, session = %req.session_id, "Token 注入失败");
            return (
                axum::http::StatusCode::FORBIDDEN,
                Json(serde_json::json!(ErrorResponse { error: format!("{}", e) })),
            );
        }
    };

    // 基本 Token 格式验证
    if token.is_empty() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!(ErrorResponse { error: "token is empty".into() })),
        );
    }
    if req.platform == "telegram" && !token.contains(':') {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!(ErrorResponse {
                error: "invalid Telegram token format (expected 'id:secret')".into()
            })),
        );
    }

    // 注入 TokenVault + auto-seal
    match state.inject_token(token, &req.platform).await {
        Ok(bot_id_hash) => {
            let resp = InjectTokenResponse {
                success: true,
                message: format!("{} token injected and sealed", req.platform),
                bot_id_hash: bot_id_hash.map(|h| hex::encode(h)),
            };
            info!(platform = %req.platform, "✅ Token 已通过 RA-TLS 安全注入");
            (axum::http::StatusCode::OK, Json(serde_json::json!(resp)))
        }
        Err(e) => {
            error!(error = %e, platform = %req.platform, "Token 注入 Vault 失败");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!(ErrorResponse { error: format!("{}", e) })),
            )
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};

    fn make_test_enclave() -> Arc<EnclaveBridge> {
        let dir = tempfile::tempdir().unwrap();
        Arc::new(EnclaveBridge::init(dir.path().to_str().unwrap(), "software").unwrap())
    }

    fn make_test_state() -> (Arc<ProvisionState>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let enclave = Arc::new(
            EnclaveBridge::init(dir.path().to_str().unwrap(), "software").unwrap()
        );
        let vault = Arc::new(RwLock::new(TokenVault::new()));
        let state = Arc::new(ProvisionState::new(enclave, Some(vault)));
        (state, dir)
    }

    /// 模拟 DApp 端加密 Token 的过程
    fn dapp_encrypt_token(
        enclave_pk_hex: &str,
        token: &str,
    ) -> (String, String, String) {
        // DApp 生成临时 X25519 密钥对
        let dapp_secret = x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng);
        let dapp_public = x25519_dalek::PublicKey::from(&dapp_secret);

        // 解析 Enclave PK
        let enclave_pk_bytes = hex::decode(enclave_pk_hex).unwrap();
        let mut epk = [0u8; 32];
        epk.copy_from_slice(&enclave_pk_bytes);
        let enclave_pk = x25519_dalek::PublicKey::from(epk);

        // ECDH → shared secret → KDF
        let shared_secret = dapp_secret.diffie_hellman(&enclave_pk);
        let mut hasher = Sha256::new();
        hasher.update(shared_secret.as_bytes());
        hasher.update(b"ra-tls-token-provision-v1");
        let aes_key: [u8; 32] = hasher.finalize().into();

        // AES-256-GCM 加密
        let cipher = Aes256Gcm::new_from_slice(&aes_key).unwrap();
        let mut nonce_bytes = [0u8; 12];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, token.as_bytes()).unwrap();

        use base64::Engine;
        let ephemeral_pk_hex = hex::encode(dapp_public.to_bytes());
        let ciphertext_b64 = base64::engine::general_purpose::STANDARD.encode(&ciphertext);
        let nonce_b64 = base64::engine::general_purpose::STANDARD.encode(&nonce_bytes);

        (ephemeral_pk_hex, ciphertext_b64, nonce_b64)
    }

    #[tokio::test]
    async fn create_session_returns_valid_data() {
        let (state, _dir) = make_test_state();
        let (session_id, quote, _mrtd) = state.create_session().await.unwrap();

        assert_eq!(session_id.len(), 32); // 16 bytes hex
        assert!(!quote.is_empty());
        assert_eq!(state.sessions.read().await.len(), 1);
    }

    #[tokio::test]
    async fn full_provision_roundtrip_telegram() {
        let (state, _dir) = make_test_state();
        let token = "123456789:ABCdefGHIjklMNOpqrSTUvwxYZ";

        // Step 1: 创建会话
        let (session_id, _quote, _mrtd) = state.create_session().await.unwrap();

        // 获取 enclave PK
        let enclave_pk_hex = {
            let sessions = state.sessions.read().await;
            let s = sessions.iter().find(|s| s.session_id_hex() == session_id).unwrap();
            hex::encode(s.x25519_public.to_bytes())
        };

        // Step 2: DApp 端加密 Token
        let (ephemeral_pk, ciphertext, nonce) = dapp_encrypt_token(&enclave_pk_hex, token);

        // Step 3: 注入
        let decrypted = state.consume_session(
            &session_id, &ephemeral_pk, &ciphertext, &nonce,
        ).await.unwrap();

        assert_eq!(decrypted.as_str(), token);

        // Step 4: 注入 Vault
        let hash = state.inject_token(decrypted, "telegram").await.unwrap();
        assert!(hash.is_some());

        // 验证 Vault 中有 Token
        let vault = state.vault.as_ref().unwrap().read().await;
        assert!(vault.has_telegram_token());
    }

    #[tokio::test]
    async fn full_provision_roundtrip_discord() {
        let (state, _dir) = make_test_state();
        let token = "MTIzNDU2Nzg5MDEyMzQ1Njc4OQ.Gg1234.abcdefghijklmnop";

        let (session_id, _quote, _mrtd) = state.create_session().await.unwrap();
        let enclave_pk_hex = {
            let sessions = state.sessions.read().await;
            let s = sessions.iter().find(|s| s.session_id_hex() == session_id).unwrap();
            hex::encode(s.x25519_public.to_bytes())
        };

        let (ephemeral_pk, ciphertext, nonce) = dapp_encrypt_token(&enclave_pk_hex, token);

        let decrypted = state.consume_session(
            &session_id, &ephemeral_pk, &ciphertext, &nonce,
        ).await.unwrap();

        let hash = state.inject_token(decrypted, "discord").await.unwrap();
        assert!(hash.is_none()); // Discord 无 bot_id_hash

        let vault = state.vault.as_ref().unwrap().read().await;
        assert!(vault.has_discord_token());
    }

    #[tokio::test]
    async fn session_one_time_use() {
        let (state, _dir) = make_test_state();
        let token = "test:token";

        let (session_id, _, _) = state.create_session().await.unwrap();
        let enclave_pk_hex = {
            let sessions = state.sessions.read().await;
            hex::encode(sessions[0].x25519_public.to_bytes())
        };

        let (ephemeral_pk, ciphertext, nonce) = dapp_encrypt_token(&enclave_pk_hex, token);

        // 第一次使用成功
        let _ = state.consume_session(&session_id, &ephemeral_pk, &ciphertext, &nonce).await.unwrap();

        // 第二次使用失败 (会话已销毁)
        let result = state.consume_session(&session_id, &ephemeral_pk, &ciphertext, &nonce).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn invalid_session_id_rejected() {
        let (state, _dir) = make_test_state();
        let result = state.consume_session(
            "00000000000000000000000000000000",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "AAAA",
            "AAAAAAAAAAAA",
        ).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn wrong_key_decryption_fails() {
        let (state, _dir) = make_test_state();
        let token = "test:token";

        let (session_id, _, _) = state.create_session().await.unwrap();

        // 使用完全不同的密钥加密 (不是 Enclave PK)
        let wrong_secret = x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng);
        let wrong_pk = x25519_dalek::PublicKey::from(&wrong_secret);
        let (ephemeral_pk, ciphertext, nonce) = dapp_encrypt_token(
            &hex::encode(wrong_pk.to_bytes()), token,
        );

        let result = state.consume_session(&session_id, &ephemeral_pk, &ciphertext, &nonce).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn max_sessions_enforced() {
        let (state, _dir) = make_test_state();
        for _ in 0..MAX_SESSIONS {
            state.create_session().await.unwrap();
        }
        // 第 9 个应该失败
        let result = state.create_session().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn invalid_platform_format_check() {
        let (state, _dir) = make_test_state();
        let token = Zeroizing::new("invalid_format_no_colon".to_string());
        // inject_token 本身不检查格式, 但 handle_inject_token 会
        // 这里直接测试 inject_token 成功 (格式检查在 handler 层)
        let result = state.inject_token(token, "telegram").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn unsupported_platform_rejected() {
        let (state, _dir) = make_test_state();
        let token = Zeroizing::new("test:token".to_string());
        let result = state.inject_token(token, "whatsapp").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn axum_routes_compile() {
        // 确保路由可以正常构建
        let enclave = make_test_enclave();
        let vault = Arc::new(RwLock::new(TokenVault::new()));
        let _router = provision_routes(enclave, Some(vault));
    }
}
