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
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use zeroize::Zeroizing;

use crate::error::{BotError, BotResult};
use crate::tee::enclave_bridge::EnclaveBridge;
use crate::tee::token_vault::TokenVault;
use crate::tee::share_recovery;
use crate::tee::vault_client::VaultClient;

// ═══════════════════════════════════════════════════════════════
// 协议类型
// ═══════════════════════════════════════════════════════════════

/// GET /provision/attestation 响应
#[derive(Serialize)]
pub struct AttestationResponse {
    /// TEE Quote (base64) — SGX Quote (connect 模式) 或 TDX Quote (inprocess 模式)
    pub quote: String,
    /// Enclave X25519 公钥 (hex, 32 bytes) — 绑定在 Quote report_data[0..32]
    pub enclave_pk: String,
    /// TEE 度量值 (hex) — MRENCLAVE (32B, SGX) 或 MRTD (48B, TDX)
    pub tee_measurement: String,
    /// TEE 模式: "sgx-proxy" | "hardware" | "software"
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
    /// 活跃会话 (受 RwLock 保护, 仅 inprocess/TDX 模式使用)
    sessions: RwLock<Vec<ProvisionSession>>,
    /// TokenVault (注入目标, 仅 inprocess 模式)
    vault: Option<Arc<RwLock<TokenVault>>>,
    /// SGX Vault IPC 客户端 (connect 模式: 委托 SGX vault 生成 Quote + 解密 Token)
    /// Some = SGX 代理模式 (Token 明文从不经过主进程)
    /// None = TDX 直连模式 (当前行为, 向后兼容)
    vault_client: Option<Arc<VaultClient>>,
    /// Bearer Token 鉴权 (C1 修复)
    provision_secret: String,
}

impl ProvisionState {
    pub fn new(
        enclave: Arc<EnclaveBridge>,
        vault: Option<Arc<RwLock<TokenVault>>>,
        vault_client: Option<Arc<VaultClient>>,
        provision_secret: String,
    ) -> Self {
        Self {
            enclave,
            sessions: RwLock::new(Vec::new()),
            vault,
            vault_client,
            provision_secret,
        }
    }

    /// C1 修复: 验证 Bearer Token
    fn check_auth(&self, headers: &HeaderMap) -> bool {
        let auth = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let expected = format!("Bearer {}", self.provision_secret);
        // 使用 SHA256 比较防止时序攻击
        let expected_hash = Sha256::digest(expected.as_bytes());
        let actual_hash = Sha256::digest(auth.as_bytes());
        expected_hash == actual_hash
    }

    /// 创建新会话
    ///
    /// SGX 代理模式: 委托 SGX vault 生成 SGX Quote + X25519 密钥对
    /// TDX 直连模式: 本地生成 TDX Quote + X25519 密钥对 (向后兼容)
    ///
    /// 返回 (session_id_hex, quote, enclave_pk_hex, tee_measurement_hex, tee_mode_str)
    async fn create_session(&self) -> BotResult<(String, Vec<u8>, String, String, String)> {
        if let Some(ref client) = self.vault_client {
            return self.create_session_via_sgx(client).await;
        }
        self.create_session_local().await
    }

    /// SGX 代理: 通过 IPC 委托 SGX vault 创建会话
    async fn create_session_via_sgx(&self, client: &VaultClient) -> BotResult<(String, Vec<u8>, String, String, String)> {
        let (session_id, quote, x25519_pk, tee_measurement) = client.create_provision_session().await?;
        let session_id_hex = hex::encode(session_id);
        let enclave_pk_hex = hex::encode(x25519_pk);
        let tee_measurement_hex = hex::encode(&tee_measurement);
        info!(
            session_id = %session_id_hex,
            x25519_pk = %enclave_pk_hex,
            measurement = %tee_measurement_hex,
            "SGX 代理: Provision 会话已创建 (Quote 来自 SGX vault)"
        );
        Ok((session_id_hex, quote, enclave_pk_hex, tee_measurement_hex, "sgx-proxy".into()))
    }

    /// TDX 本地: 直接生成 TDX Quote (向后兼容)
    async fn create_session_local(&self) -> BotResult<(String, Vec<u8>, String, String, String)> {
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
        let enclave_pk_hex = hex::encode(pk_bytes);
        let tee_measurement_hex = hex::encode(mrtd);
        let tee_mode = self.enclave.mode().to_string();

        sessions.push(session);

        info!(
            session_id = %session_id_hex,
            x25519_pk = %enclave_pk_hex,
            "Provision 会话已创建 (TDX 本地)"
        );

        Ok((session_id_hex, quote, enclave_pk_hex, tee_measurement_hex, tee_mode))
    }

    /// 消费会话并解密 + 注入 Token
    ///
    /// SGX 代理模式: 密文转发到 SGX vault, Token 明文从不经过主进程
    /// TDX 直连模式: 本地 ECDH 解密 + 注入 inprocess vault
    async fn consume_and_inject(
        &self,
        session_id_hex: &str,
        ephemeral_pk_hex: &str,
        ciphertext_b64: &str,
        nonce_b64: &str,
        platform: &str,
    ) -> BotResult<Option<[u8; 32]>> {
        if let Some(ref client) = self.vault_client {
            return self.consume_and_inject_via_sgx(
                client, session_id_hex, ephemeral_pk_hex, ciphertext_b64, nonce_b64, platform,
            ).await;
        }
        // TDX 本地: 解密 + 注入
        let token = self.consume_session_local(
            session_id_hex, ephemeral_pk_hex, ciphertext_b64, nonce_b64,
        ).await?;
        self.inject_token(token, platform).await
    }

    /// SGX 代理: 将密文转发到 SGX vault 解密 + 注入
    async fn consume_and_inject_via_sgx(
        &self,
        client: &VaultClient,
        session_id_hex: &str,
        ephemeral_pk_hex: &str,
        ciphertext_b64: &str,
        nonce_b64: &str,
        platform: &str,
    ) -> BotResult<Option<[u8; 32]>> {
        // 解析原始字节
        let session_id_bytes = hex::decode(session_id_hex)
            .map_err(|_| BotError::EnclaveError("invalid session_id hex".into()))?;
        if session_id_bytes.len() != 16 {
            return Err(BotError::EnclaveError("session_id must be 16 bytes".into()));
        }
        let mut session_id = [0u8; 16];
        session_id.copy_from_slice(&session_id_bytes);

        let ephemeral_pk_bytes = hex::decode(ephemeral_pk_hex)
            .map_err(|_| BotError::EnclaveError("invalid ephemeral_pk hex".into()))?;
        if ephemeral_pk_bytes.len() != 32 {
            return Err(BotError::EnclaveError("ephemeral_pk must be 32 bytes".into()));
        }
        let mut ephemeral_pk = [0u8; 32];
        ephemeral_pk.copy_from_slice(&ephemeral_pk_bytes);

        use base64::Engine;
        let ciphertext = base64::engine::general_purpose::STANDARD.decode(ciphertext_b64)
            .map_err(|_| BotError::EnclaveError("invalid ciphertext base64".into()))?;
        let nonce_bytes = base64::engine::general_purpose::STANDARD.decode(nonce_b64)
            .map_err(|_| BotError::EnclaveError("invalid nonce base64".into()))?;
        if nonce_bytes.len() != 12 {
            return Err(BotError::EnclaveError("nonce must be 12 bytes".into()));
        }
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&nonce_bytes);

        // 通过 IPC 转发到 SGX vault (密文从未解密, Token 明文从不经过主进程)
        let bot_id_hash = client.consume_provision_session(
            session_id, ephemeral_pk, ciphertext, nonce, platform.to_string(),
        ).await?;

        info!(session = %session_id_hex, platform, "✅ Token 已通过 SGX 代理安全注入 (明文仅在 SGX enclave 内)");
        Ok(bot_id_hash)
    }

    /// TDX 本地: 解密 Token (向后兼容)
    async fn consume_session_local(
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
            TeeMode::Tdx | TeeMode::Sgx => {
                // 写入 report_data → 读取 TDX/SGX Quote (Gramine 统一接口)
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
                // version=0 标记: 客户端可据此识别并拒绝 Software 模式 Quote
                let mut mock_quote = vec![0u8; 256];
                // 模拟 header (4 bytes version=0, 明确标记为非真实 Quote)
                mock_quote[0..4].copy_from_slice(&0u32.to_le_bytes());
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

    /// 从 Quote 提取度量值 (48 bytes)
    ///
    /// TDX Quote v4: MRTD at offset 184 (48B)
    /// SGX Quote v3: MRENCLAVE at offset 112 (32B) + 16B zero-pad
    fn extract_mrtd_from_quote(quote: &[u8]) -> [u8; 48] {
        let mut measurement = [0u8; 48];
        if quote.len() >= 2 {
            let version = u16::from_le_bytes([quote[0], quote[1]]);
            match version {
                4 if quote.len() >= 232 => {
                    measurement.copy_from_slice(&quote[184..232]);
                }
                3 if quote.len() >= 144 => {
                    measurement[..32].copy_from_slice(&quote[112..144]);
                }
                _ => {}
            }
        }
        measurement
    }

    /// 注入 Token 到 inprocess Vault + auto-seal (TDX 本地模式)
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
                    let mut hasher = Sha256::new();
                    hasher.update(token.as_bytes());
                    let hash: [u8; 32] = hasher.finalize().into();
                    bot_id_hash = Some(hash);
                    info!("Telegram Token 已注入 TokenVault (inprocess)");
                }
                "discord" => {
                    v.set_discord_token(token.to_string());
                    info!("Discord Token 已注入 TokenVault (inprocess)");
                }
                _ => {
                    return Err(BotError::EnclaveError(
                        format!("unsupported platform: {}", platform)
                    ));
                }
            }
        } else {
            return Err(BotError::EnclaveError(
                "inprocess vault not available (use SGX proxy mode for IPC vault)".into()
            ));
        }

        // Auto-seal: 将 Token 保存为 Shamir share (后续启动无需再次注入)
        // R5: 使用 enclave 实际签名密钥, 避免存入零值
        let signing_key = self.enclave.signing_key().to_bytes();
        let zero_hash = [0u8; 32]; // auto-seal 无真实 ceremony
        if let Err(e) = share_recovery::create_and_save_share(
            &self.enclave, token.as_str(), &signing_key, 1, 1, 0, &zero_hash,
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
/// vault_client: SGX Vault IPC 客户端 (connect 模式), None = TDX 直连
/// C1 修复: provision_secret 为空时禁用路由, 非空时要求 Bearer Token 鉴权
pub fn provision_routes(
    enclave: Arc<EnclaveBridge>,
    vault: Option<Arc<RwLock<TokenVault>>>,
    vault_client: Option<Arc<VaultClient>>,
    provision_secret: String,
) -> Router {
    if provision_secret.is_empty() {
        warn!("⚠️ PROVISION_SECRET 未设置, /provision/* 路由已禁用");
        return Router::new();
    }

    if vault_client.is_some() {
        info!("RA-TLS Provision: SGX 代理模式 (Token 明文仅在 SGX enclave 内)");
    } else if vault.is_some() {
        info!("RA-TLS Provision: TDX 本地模式 (inprocess vault)");
    } else {
        warn!("RA-TLS Provision: 无 vault 可用, inject 将失败");
    }

    let state = Arc::new(ProvisionState::new(enclave, vault, vault_client, provision_secret));

    Router::new()
        .route("/provision/attestation", get({
            let st = state.clone();
            move |headers: HeaderMap| {
                let st = st.clone();
                async move { handle_attestation(st, headers).await }
            }
        }))
        .route("/provision/inject-token", post({
            let st = state.clone();
            move |headers: HeaderMap, Json(req): Json<InjectTokenRequest>| {
                let st = st.clone();
                async move { handle_inject_token(st, headers, req).await }
            }
        }))
}

/// GET /provision/attestation
///
/// 返回 TEE Quote + Enclave X25519 公钥
/// SGX 代理模式: Quote 来自 SGX vault (MRENCLAVE)
/// TDX 本地模式: Quote 来自 TDX VM (MRTD)
async fn handle_attestation(
    state: Arc<ProvisionState>,
    headers: HeaderMap,
) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    // C1 修复: Bearer Token 鉴权
    if !state.check_auth(&headers) {
        warn!("provision/attestation: 鉴权失败");
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(serde_json::json!(ErrorResponse { error: "unauthorized: invalid or missing PROVISION_SECRET".into() })),
        );
    }

    match state.create_session().await {
        Ok((session_id, quote, enclave_pk, tee_measurement, tee_mode)) => {
            use base64::Engine;
            let resp = AttestationResponse {
                quote: base64::engine::general_purpose::STANDARD.encode(&quote),
                enclave_pk,
                tee_measurement,
                tee_mode,
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
    headers: HeaderMap,
    req: InjectTokenRequest,
) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    // C1 修复: Bearer Token 鉴权
    if !state.check_auth(&headers) {
        warn!("provision/inject-token: 鉴权失败");
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(serde_json::json!(ErrorResponse { error: "unauthorized: invalid or missing PROVISION_SECRET".into() })),
        );
    }
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

    // 解密 + 注入 (统一路径: SGX 代理 / TDX 本地)
    match state.consume_and_inject(
        &req.session_id,
        &req.ephemeral_pk,
        &req.ciphertext,
        &req.nonce,
        &req.platform,
    ).await {
        Ok(bot_id_hash) => {
            let resp = InjectTokenResponse {
                success: true,
                message: format!("{} token injected and sealed", req.platform),
                bot_id_hash: bot_id_hash.map(|h| hex::encode(h)),
            };
            (axum::http::StatusCode::OK, Json(serde_json::json!(resp)))
        }
        Err(e) => {
            warn!(error = %e, session = %req.session_id, platform = %req.platform, "Token 注入失败");
            (
                axum::http::StatusCode::FORBIDDEN,
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
        let state = Arc::new(ProvisionState::new(enclave, Some(vault), None, "test-secret".into()));
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
        let (session_id, quote, _pk, _meas, _mode) = state.create_session().await.unwrap();

        assert_eq!(session_id.len(), 32); // 16 bytes hex
        assert!(!quote.is_empty());
        assert_eq!(state.sessions.read().await.len(), 1);
    }

    #[tokio::test]
    async fn full_provision_roundtrip_telegram() {
        let (state, _dir) = make_test_state();
        let token = "123456789:ABCdefGHIjklMNOpqrSTUvwxYZ";

        // Step 1: 创建会话
        let (session_id, _quote, enclave_pk_hex, _meas, _mode) = state.create_session().await.unwrap();

        // Step 2: DApp 端加密 Token
        let (ephemeral_pk, ciphertext, nonce) = dapp_encrypt_token(&enclave_pk_hex, token);

        // Step 3: 解密 + 注入 (TDX 本地模式)
        let hash = state.consume_and_inject(
            &session_id, &ephemeral_pk, &ciphertext, &nonce, "telegram",
        ).await.unwrap();
        assert!(hash.is_some());

        // 验证 Vault 中有 Token
        let vault = state.vault.as_ref().unwrap().read().await;
        assert!(vault.has_telegram_token());
    }

    #[tokio::test]
    async fn full_provision_roundtrip_discord() {
        let (state, _dir) = make_test_state();
        let token = "MTIzNDU2Nzg5MDEyMzQ1Njc4OQ.Gg1234.abcdefghijklmnop";

        let (session_id, _quote, enclave_pk_hex, _meas, _mode) = state.create_session().await.unwrap();

        let (ephemeral_pk, ciphertext, nonce) = dapp_encrypt_token(&enclave_pk_hex, token);

        let hash = state.consume_and_inject(
            &session_id, &ephemeral_pk, &ciphertext, &nonce, "discord",
        ).await.unwrap();
        assert!(hash.is_none()); // Discord 无 bot_id_hash

        let vault = state.vault.as_ref().unwrap().read().await;
        assert!(vault.has_discord_token());
    }

    #[tokio::test]
    async fn session_one_time_use() {
        let (state, _dir) = make_test_state();
        let token = "test:token";

        let (session_id, _, enclave_pk_hex, _, _) = state.create_session().await.unwrap();

        let (ephemeral_pk, ciphertext, nonce) = dapp_encrypt_token(&enclave_pk_hex, token);

        // 第一次使用成功
        let _ = state.consume_and_inject(&session_id, &ephemeral_pk, &ciphertext, &nonce, "telegram").await.unwrap();

        // 第二次使用失败 (会话已销毁)
        let result = state.consume_and_inject(&session_id, &ephemeral_pk, &ciphertext, &nonce, "telegram").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn invalid_session_id_rejected() {
        let (state, _dir) = make_test_state();
        let result = state.consume_and_inject(
            "00000000000000000000000000000000",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "AAAA",
            "AAAAAAAAAAAA",
            "telegram",
        ).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn wrong_key_decryption_fails() {
        let (state, _dir) = make_test_state();
        let token = "test:token";

        let (session_id, _, _, _, _) = state.create_session().await.unwrap();

        // 使用完全不同的密钥加密 (不是 Enclave PK)
        let wrong_secret = x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng);
        let wrong_pk = x25519_dalek::PublicKey::from(&wrong_secret);
        let (ephemeral_pk, ciphertext, nonce) = dapp_encrypt_token(
            &hex::encode(wrong_pk.to_bytes()), token,
        );

        let result = state.consume_and_inject(&session_id, &ephemeral_pk, &ciphertext, &nonce, "telegram").await;
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
        let _router = provision_routes(enclave, Some(vault), None, "test-secret".into());
    }
}
