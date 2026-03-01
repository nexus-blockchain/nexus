// Token Vault Server — 独立进程, 持有 Token 并通过 Unix socket 提供 IPC 服务
//
// 安全属性:
// - Token 仅存在于此进程的内存中 (Zeroizing<String>)
// - 主进程通过 Unix socket 发送请求, 得到拼接后的 URL/Header
// - 在 Gramine SGX 中运行时, 此进程的全部内存受硬件加密保护
// - Token 永远不会以原始形式跨越进程边界
//
// 启动模式:
// - 内嵌模式: 由 main.rs spawn, 通过 data_dir/vault.sock 通信
// - 独立模式: 单独运行 (Gramine SGX), 通过指定 socket 路径通信

use std::sync::Arc;

use sha2::{Sha256, Digest};
use tokio::net::UnixListener;
use tokio::sync::RwLock;
use tracing::{info, warn, debug};
use x25519_dalek;

use crate::tee::token_vault::TokenVault;
use crate::tee::vault_ipc::{
    VaultRequest, VaultResponse, IpcCipher,
    read_message, write_message, read_encrypted, write_encrypted,
};

// ═══════════════════════════════════════════════════════════════
// SGX Provision Session — 在 vault 进程 (SGX enclave) 内管理
// ═══════════════════════════════════════════════════════════════

/// 会话超时 (秒)
const PROVISION_SESSION_TIMEOUT_SECS: u64 = 300;
/// 最大并发会话数
const MAX_PROVISION_SESSIONS: usize = 8;

/// 一次 SGX Provision 会话
struct ProvisionSession {
    session_id: [u8; 16],
    x25519_secret: x25519_dalek::StaticSecret,
    x25519_public: x25519_dalek::PublicKey,
    created_at: std::time::Instant,
    used: bool,
}

impl ProvisionSession {
    fn new() -> Self {
        use rand::RngCore;
        let mut session_id = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut session_id);
        let x25519_secret = x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng);
        let x25519_public = x25519_dalek::PublicKey::from(&x25519_secret);
        Self { session_id, x25519_secret, x25519_public, created_at: std::time::Instant::now(), used: false }
    }

    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > std::time::Duration::from_secs(PROVISION_SESSION_TIMEOUT_SECS)
    }
}

/// Vault 服务端
pub struct VaultServer {
    vault: Arc<RwLock<TokenVault>>,
    socket_path: String,
    /// IPC 加密密钥 (None = 明文模式, 向后兼容)
    ipc_key: Option<[u8; 32]>,
    /// SGX Provision 会话 (仅 vault 进程内部使用)
    provision_sessions: Arc<RwLock<Vec<ProvisionSession>>>,
}

#[allow(dead_code)]
impl VaultServer {
    /// 创建 Vault 服务端
    pub fn new(vault: TokenVault, socket_path: String) -> Self {
        Self {
            vault: Arc::new(RwLock::new(vault)),
            socket_path,
            ipc_key: None,
            provision_sessions: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 创建带加密的 Vault 服务端
    pub fn with_encryption(vault: TokenVault, socket_path: String, key: [u8; 32]) -> Self {
        Self {
            vault: Arc::new(RwLock::new(vault)),
            socket_path,
            ipc_key: Some(key),
            provision_sessions: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 运行 IPC 服务 (阻塞)
    pub async fn run(&self) -> std::io::Result<()> {
        // 清理旧 socket 文件
        let _ = std::fs::remove_file(&self.socket_path);

        let listener = UnixListener::bind(&self.socket_path)?;

        // 设置 socket 权限为 0600 (仅 owner 可访问)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(
                &self.socket_path,
                std::fs::Permissions::from_mode(0o600),
            );
        }

        info!(socket = %self.socket_path, "Vault IPC 服务已启动");

        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let vault = self.vault.clone();
                    let ipc_key = self.ipc_key;
                    let sessions = self.provision_sessions.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(vault, stream, ipc_key, sessions).await {
                            debug!(error = %e, "Vault 连接处理结束");
                        }
                    });
                }
                Err(e) => {
                    warn!(error = %e, "Vault accept 失败");
                }
            }
        }
    }

    /// 获取 socket 路径
    pub fn socket_path(&self) -> &str {
        &self.socket_path
    }
}

impl Drop for VaultServer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// 处理单个客户端连接
async fn handle_connection(
    vault: Arc<RwLock<TokenVault>>,
    stream: tokio::net::UnixStream,
    ipc_key: Option<[u8; 32]>,
    provision_sessions: Arc<RwLock<Vec<ProvisionSession>>>,
) -> std::io::Result<()> {
    let (mut reader, mut writer) = stream.into_split();
    let cipher = ipc_key.map(|k| IpcCipher::new_server(&k));

    loop {
        // 读取请求 (加密或明文)
        let data = match &cipher {
            Some(c) => match read_encrypted(&mut reader, c).await {
                Ok(d) => d,
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
                Err(e) => return Err(e),
            },
            None => match read_message(&mut reader).await {
                Ok(d) => d,
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
                Err(e) => return Err(e),
            },
        };

        let request = match VaultRequest::from_bytes(&data) {
            Ok(r) => r,
            Err(e) => {
                let resp = VaultResponse::Error(format!("parse error: {}", e));
                let resp_payload = resp.to_bytes();
                match &cipher {
                    Some(c) => write_encrypted(&mut writer, &resp_payload[4..], c).await?,
                    None => write_message(&mut writer, &resp_payload).await?,
                }
                continue;
            }
        };

        // 处理请求
        let response = process_request(&vault, &provision_sessions, request).await;

        // 发送响应 (加密或明文)
        let resp_payload = response.to_bytes();
        match &cipher {
            Some(c) => write_encrypted(&mut writer, &resp_payload[4..], c).await?,
            None => write_message(&mut writer, &resp_payload).await?,
        }

        // Shutdown 请求后退出
        if matches!(response, VaultResponse::ShutdownAck) {
            return Ok(());
        }
    }
}

/// 处理单个请求
async fn process_request(
    vault: &Arc<RwLock<TokenVault>>,
    provision_sessions: &Arc<RwLock<Vec<ProvisionSession>>>,
    request: VaultRequest,
) -> VaultResponse {
    match request {
        VaultRequest::BuildTgApiUrl { method } => {
            let v = vault.read().await;
            match v.build_tg_api_url(&method) {
                Ok(url) => VaultResponse::Ok(url.to_string()),
                Err(e) => VaultResponse::Error(format!("{}", e)),
            }
        }
        VaultRequest::BuildDcAuthHeader => {
            let v = vault.read().await;
            match v.build_dc_auth_header() {
                Ok(header) => VaultResponse::Ok(header.to_string()),
                Err(e) => VaultResponse::Error(format!("{}", e)),
            }
        }
        VaultRequest::BuildDcIdentifyPayload { intents } => {
            let v = vault.read().await;
            match v.build_dc_identify_payload(intents) {
                Ok(payload) => VaultResponse::Ok(payload.to_string()),
                Err(e) => VaultResponse::Error(format!("{}", e)),
            }
        }
        VaultRequest::DeriveTgBotIdHash => {
            let v = vault.read().await;
            match v.derive_tg_bot_id_hash() {
                Ok(hash) => VaultResponse::OkHash(hash),
                Err(e) => VaultResponse::Error(format!("{}", e)),
            }
        }
        VaultRequest::Ping => VaultResponse::Pong,
        VaultRequest::Shutdown => {
            let mut v = vault.write().await;
            v.zeroize_all();
            info!("Vault shutdown: tokens zeroized");
            VaultResponse::ShutdownAck
        }
        VaultRequest::CreateProvisionSession => {
            handle_create_provision_session(provision_sessions).await
        }
        VaultRequest::ConsumeProvisionSession { session_id, ephemeral_pk, ciphertext, nonce, platform } => {
            handle_consume_provision_session(
                vault, provision_sessions, session_id, ephemeral_pk, ciphertext, nonce, platform,
            ).await
        }
    }
}

/// SGX vault 创建 Provision 会话: 生成 X25519 密钥对 + TEE Quote
async fn handle_create_provision_session(
    sessions: &Arc<RwLock<Vec<ProvisionSession>>>,
) -> VaultResponse {
    let mut sessions = sessions.write().await;

    // 清理过期会话
    sessions.retain(|s| !s.is_expired());

    if sessions.len() >= MAX_PROVISION_SESSIONS {
        return VaultResponse::Error("too many active provision sessions".into());
    }

    let session = ProvisionSession::new();
    let session_id = session.session_id;
    let pk_bytes = session.x25519_public.to_bytes();

    // 计算 pk_hash → 绑定到 report_data
    let mut hasher = Sha256::new();
    hasher.update(b"ra-tls-provision-v1:");
    hasher.update(pk_bytes);
    let pk_hash: [u8; 32] = hasher.finalize().into();

    // 生成 TEE Quote (SGX 或 TDX, 取决于运行环境)
    let (quote, tee_measurement) = match generate_tee_quote(&pk_hash) {
        Ok(r) => r,
        Err(e) => return VaultResponse::Error(format!("quote generation failed: {}", e)),
    };

    info!(
        session_id = %hex::encode(session_id),
        x25519_pk = %hex::encode(pk_bytes),
        measurement = %hex::encode(&tee_measurement),
        "SGX vault: Provision 会话已创建"
    );

    sessions.push(session);

    VaultResponse::ProvisionSessionCreated {
        session_id,
        quote,
        x25519_pk: pk_bytes,
        tee_measurement,
    }
}

/// SGX vault 消费 Provision 会话: ECDH 解密 + 注入 Token
async fn handle_consume_provision_session(
    vault: &Arc<RwLock<TokenVault>>,
    sessions: &Arc<RwLock<Vec<ProvisionSession>>>,
    session_id: [u8; 16],
    ephemeral_pk: [u8; 32],
    ciphertext: Vec<u8>,
    nonce: [u8; 12],
    platform: String,
) -> VaultResponse {
    let mut sessions = sessions.write().await;
    let session_id_hex = hex::encode(session_id);

    // 查找匹配的会话
    let idx = match sessions.iter().position(|s| s.session_id == session_id) {
        Some(i) => i,
        None => return VaultResponse::Error("invalid or expired session_id".into()),
    };

    if sessions[idx].is_expired() {
        sessions.remove(idx);
        return VaultResponse::Error("session expired".into());
    }

    if sessions[idx].used {
        sessions.remove(idx);
        return VaultResponse::Error("session already used".into());
    }

    sessions[idx].used = true;

    // ECDH → shared secret → KDF → AES key
    let dapp_pk = x25519_dalek::PublicKey::from(ephemeral_pk);
    let shared_secret = sessions[idx].x25519_secret.diffie_hellman(&dapp_pk);

    let mut hasher = Sha256::new();
    hasher.update(shared_secret.as_bytes());
    hasher.update(b"ra-tls-token-provision-v1");
    let aes_key: [u8; 32] = hasher.finalize().into();

    // AES-256-GCM 解密 (在 SGX enclave 内完成)
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
    let cipher = match Aes256Gcm::new_from_slice(&aes_key) {
        Ok(c) => c,
        Err(_) => {
            sessions.remove(idx);
            return VaultResponse::Error("AES key init failed".into());
        }
    };
    let gcm_nonce = Nonce::from_slice(&nonce);
    let plaintext = match cipher.decrypt(gcm_nonce, ciphertext.as_ref()) {
        Ok(pt) => pt,
        Err(_) => {
            warn!(session = %session_id_hex, "SGX vault: Token 解密失败 (密钥不匹配或数据被篡改)");
            sessions.remove(idx);
            return VaultResponse::Error("AES-GCM decryption failed".into());
        }
    };

    let token = match String::from_utf8(plaintext) {
        Ok(t) => t,
        Err(_) => {
            sessions.remove(idx);
            return VaultResponse::Error("decrypted data is not valid UTF-8".into());
        }
    };

    // 销毁会话
    sessions.remove(idx);

    // 注入 Token 到 vault (在 SGX enclave 内, Token 永不离开)
    let mut v = vault.write().await;
    let mut bot_id_hash = None;
    match platform.as_str() {
        "telegram" => {
            // 计算 bot_id_hash
            let hash: [u8; 32] = Sha256::digest(token.as_bytes()).into();
            bot_id_hash = Some(hash);
            v.set_telegram_token(token);
            info!(session = %session_id_hex, "SGX vault: Telegram Token 已注入");
        }
        "discord" => {
            v.set_discord_token(token);
            info!(session = %session_id_hex, "SGX vault: Discord Token 已注入");
        }
        _ => {
            return VaultResponse::Error(format!("unsupported platform: {}", platform));
        }
    }

    VaultResponse::TokenInjected { bot_id_hash }
}

/// 生成 TEE Quote, 返回 (quote_bytes, tee_measurement)
///
/// 在 Gramine SGX 中: /dev/attestation/quote 生成 SGX Quote, 提取 MRENCLAVE (32 bytes)
/// 在 TDX VM 中: 生成 TDX Quote, 提取 MRTD (48 bytes)
/// 在软件模式: 返回模拟 Quote
fn generate_tee_quote(pk_hash: &[u8; 32]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let mut report_data = [0u8; 64];
    report_data[..32].copy_from_slice(pk_hash);

    if std::path::Path::new("/dev/attestation/quote").exists() {
        // 硬件模式: 写入 report_data → 读取 Quote
        std::fs::write("/dev/attestation/user_report_data", &report_data)
            .map_err(|e| format!("write report_data: {}", e))?;
        let quote = std::fs::read("/dev/attestation/quote")
            .map_err(|e| format!("read quote: {}", e))?;

        // 检测 Quote 类型: SGX Quote Header 的 attestation_type
        // TDX Quote v4: version=4 at offset 0, TEE type at offset 4
        // SGX Quote v3: version=3 at offset 0
        let measurement = if quote.len() >= 6 {
            let version = u16::from_le_bytes([quote[0], quote[1]]);
            let att_key_type = u16::from_le_bytes([quote[2], quote[3]]);
            if version == 4 && quote.len() >= 232 {
                // TDX Quote v4: MRTD at offset 184 (48 bytes)
                info!("Vault quote: TDX v4 (MRTD)");
                quote[184..232].to_vec()
            } else if version == 3 && quote.len() >= 144 {
                // SGX Quote v3: MRENCLAVE at Report Body offset 112 (32 bytes)
                // Quote v3 header = 48 bytes, Report Body starts at 48
                // MRENCLAVE at Report Body + 64 = offset 48+64 = 112
                info!(att_key_type, "Vault quote: SGX v3 (MRENCLAVE)");
                quote[112..144].to_vec()
            } else {
                warn!(version, len = quote.len(), "Unknown quote format, returning empty measurement");
                vec![]
            }
        } else {
            vec![]
        };

        Ok((quote, measurement))
    } else {
        // 软件模式: 模拟 Quote
        let mut mock_quote = vec![0u8; 256];
        mock_quote[0..2].copy_from_slice(&3u16.to_le_bytes()); // version=3 (SGX)
        // 模拟 report_data 位置 (简化)
        if mock_quote.len() >= 80 {
            mock_quote[48..80].copy_from_slice(pk_hash);
        }
        // 模拟 MRENCLAVE (offset 112, 32 bytes)
        let mock_mrenclave = [0xBBu8; 32];
        mock_quote[112..144].copy_from_slice(&mock_mrenclave);
        Ok((mock_quote, mock_mrenclave.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tee::vault_ipc;

    async fn setup_server_client() -> (String, tokio::task::JoinHandle<()>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_str().unwrap().to_string();
        let sock_path = vault_ipc::default_socket_path(&dir_path);

        let mut vault = TokenVault::new();
        vault.set_telegram_token("test_tg:TOKEN123".to_string());
        vault.set_discord_token("test_dc_TOKEN456".to_string());

        let server = VaultServer::new(vault, sock_path.clone());
        let handle = tokio::spawn(async move {
            let _ = server.run().await;
        });

        // 等待 server 启动
        for _ in 0..50 {
            if std::path::Path::new(&sock_path).exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        (sock_path, handle, dir)
    }

    #[tokio::test]
    async fn server_ping() {
        let (sock_path, handle, _dir) = setup_server_client().await;

        let stream = tokio::net::UnixStream::connect(&sock_path).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        let req = VaultRequest::Ping;
        write_message(&mut writer, &req.to_bytes()).await.unwrap();

        let data = read_message(&mut reader).await.unwrap();
        let resp = VaultResponse::from_bytes(&data).unwrap();
        assert!(matches!(resp, VaultResponse::Pong));

        handle.abort();
    }

    #[tokio::test]
    async fn server_build_tg_api_url() {
        let (sock_path, handle, _dir) = setup_server_client().await;

        let stream = tokio::net::UnixStream::connect(&sock_path).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        let req = VaultRequest::BuildTgApiUrl { method: "getMe".into() };
        write_message(&mut writer, &req.to_bytes()).await.unwrap();

        let data = read_message(&mut reader).await.unwrap();
        let resp = VaultResponse::from_bytes(&data).unwrap();
        match resp {
            VaultResponse::Ok(url) => {
                assert!(url.contains("test_tg:TOKEN123"));
                assert!(url.contains("/getMe"));
            }
            _ => panic!("expected Ok, got {:?}", resp),
        }

        handle.abort();
    }

    #[tokio::test]
    async fn server_build_dc_auth_header() {
        let (sock_path, handle, _dir) = setup_server_client().await;

        let stream = tokio::net::UnixStream::connect(&sock_path).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        let req = VaultRequest::BuildDcAuthHeader;
        write_message(&mut writer, &req.to_bytes()).await.unwrap();

        let data = read_message(&mut reader).await.unwrap();
        let resp = VaultResponse::from_bytes(&data).unwrap();
        match resp {
            VaultResponse::Ok(header) => assert_eq!(header, "Bot test_dc_TOKEN456"),
            _ => panic!("expected Ok"),
        }

        handle.abort();
    }

    #[tokio::test]
    async fn server_derive_hash() {
        let (sock_path, handle, _dir) = setup_server_client().await;

        let stream = tokio::net::UnixStream::connect(&sock_path).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        let req = VaultRequest::DeriveTgBotIdHash;
        write_message(&mut writer, &req.to_bytes()).await.unwrap();

        let data = read_message(&mut reader).await.unwrap();
        let resp = VaultResponse::from_bytes(&data).unwrap();
        match resp {
            VaultResponse::OkHash(hash) => {
                assert_ne!(hash, [0u8; 32]);
                assert_eq!(hash.len(), 32);
            }
            _ => panic!("expected OkHash"),
        }

        handle.abort();
    }

    #[tokio::test]
    async fn server_shutdown_zeroizes() {
        let (sock_path, handle, _dir) = setup_server_client().await;

        let stream = tokio::net::UnixStream::connect(&sock_path).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        // Shutdown
        let req = VaultRequest::Shutdown;
        write_message(&mut writer, &req.to_bytes()).await.unwrap();

        let data = read_message(&mut reader).await.unwrap();
        let resp = VaultResponse::from_bytes(&data).unwrap();
        assert!(matches!(resp, VaultResponse::ShutdownAck));

        handle.abort();
    }

    #[tokio::test]
    async fn server_create_provision_session() {
        let (sock_path, handle, _dir) = setup_server_client().await;

        let stream = tokio::net::UnixStream::connect(&sock_path).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        let req = VaultRequest::CreateProvisionSession;
        write_message(&mut writer, &req.to_bytes()).await.unwrap();

        let data = read_message(&mut reader).await.unwrap();
        let resp = VaultResponse::from_bytes(&data).unwrap();
        match resp {
            VaultResponse::ProvisionSessionCreated { session_id, quote, x25519_pk, tee_measurement } => {
                assert_ne!(session_id, [0u8; 16]);
                assert!(!quote.is_empty());
                assert_ne!(x25519_pk, [0u8; 32]);
                // 软件模式: 模拟 MRENCLAVE (32 bytes)
                assert_eq!(tee_measurement.len(), 32);
            }
            other => panic!("expected ProvisionSessionCreated, got {:?}", other),
        }

        handle.abort();
    }

    #[tokio::test]
    async fn server_sgx_provision_roundtrip() {
        use sha2::{Sha256, Digest};
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};

        let (sock_path, handle, _dir) = setup_server_client().await;

        let stream = tokio::net::UnixStream::connect(&sock_path).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        // Step 1: 创建 provision 会话
        write_message(&mut writer, &VaultRequest::CreateProvisionSession.to_bytes()).await.unwrap();
        let data = read_message(&mut reader).await.unwrap();
        let (session_id, x25519_pk) = match VaultResponse::from_bytes(&data).unwrap() {
            VaultResponse::ProvisionSessionCreated { session_id, x25519_pk, .. } => (session_id, x25519_pk),
            other => panic!("expected ProvisionSessionCreated, got {:?}", other),
        };

        // Step 2: DApp 端 ECDH 加密 Token
        let token = "123456789:ABCdefGHIjklMNOpqrSTUvwxYZ";
        let dapp_secret = x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng);
        let dapp_public = x25519_dalek::PublicKey::from(&dapp_secret);
        let enclave_pk = x25519_dalek::PublicKey::from(x25519_pk);
        let shared_secret = dapp_secret.diffie_hellman(&enclave_pk);

        let mut hasher = Sha256::new();
        hasher.update(shared_secret.as_bytes());
        hasher.update(b"ra-tls-token-provision-v1");
        let aes_key: [u8; 32] = hasher.finalize().into();

        let cipher = Aes256Gcm::new_from_slice(&aes_key).unwrap();
        let mut nonce_bytes = [0u8; 12];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, token.as_bytes()).unwrap();

        // Step 3: 发送加密 Token 到 vault
        let req = VaultRequest::ConsumeProvisionSession {
            session_id,
            ephemeral_pk: dapp_public.to_bytes(),
            ciphertext,
            nonce: nonce_bytes,
            platform: "telegram".into(),
        };
        write_message(&mut writer, &req.to_bytes()).await.unwrap();

        let data = read_message(&mut reader).await.unwrap();
        match VaultResponse::from_bytes(&data).unwrap() {
            VaultResponse::TokenInjected { bot_id_hash } => {
                // Telegram 应该返回 bot_id_hash
                assert!(bot_id_hash.is_some());
                let expected_hash: [u8; 32] = Sha256::digest(token.as_bytes()).into();
                assert_eq!(bot_id_hash.unwrap(), expected_hash);
            }
            other => panic!("expected TokenInjected, got {:?}", other),
        }

        // Step 4: 验证 Token 可用 — 构建 TG API URL
        let req = VaultRequest::BuildTgApiUrl { method: "getMe".into() };
        write_message(&mut writer, &req.to_bytes()).await.unwrap();
        let data = read_message(&mut reader).await.unwrap();
        match VaultResponse::from_bytes(&data).unwrap() {
            VaultResponse::Ok(url) => {
                assert!(url.contains("123456789:ABCdefGHIjklMNOpqrSTUvwxYZ"));
                assert!(url.contains("/getMe"));
            }
            other => panic!("expected Ok, got {:?}", other),
        }

        handle.abort();
    }

    #[tokio::test]
    async fn server_provision_session_one_time_use() {
        use sha2::{Sha256, Digest};
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};

        let (sock_path, handle, _dir) = setup_server_client().await;

        let stream = tokio::net::UnixStream::connect(&sock_path).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        // 创建会话
        write_message(&mut writer, &VaultRequest::CreateProvisionSession.to_bytes()).await.unwrap();
        let data = read_message(&mut reader).await.unwrap();
        let (session_id, x25519_pk) = match VaultResponse::from_bytes(&data).unwrap() {
            VaultResponse::ProvisionSessionCreated { session_id, x25519_pk, .. } => (session_id, x25519_pk),
            other => panic!("unexpected: {:?}", other),
        };

        // DApp 加密
        let dapp_secret = x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng);
        let dapp_public = x25519_dalek::PublicKey::from(&dapp_secret);
        let shared = dapp_secret.diffie_hellman(&x25519_dalek::PublicKey::from(x25519_pk));
        let mut h = Sha256::new(); h.update(shared.as_bytes()); h.update(b"ra-tls-token-provision-v1");
        let aes_key: [u8; 32] = h.finalize().into();
        let cipher = Aes256Gcm::new_from_slice(&aes_key).unwrap();
        let mut nonce_bytes = [0u8; 12];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let ct = cipher.encrypt(Nonce::from_slice(&nonce_bytes), b"test:token".as_ref()).unwrap();

        let consume_req = VaultRequest::ConsumeProvisionSession {
            session_id,
            ephemeral_pk: dapp_public.to_bytes(),
            ciphertext: ct.clone(),
            nonce: nonce_bytes,
            platform: "telegram".into(),
        };

        // 第一次: 成功
        write_message(&mut writer, &consume_req.to_bytes()).await.unwrap();
        let data = read_message(&mut reader).await.unwrap();
        assert!(matches!(VaultResponse::from_bytes(&data).unwrap(), VaultResponse::TokenInjected { .. }));

        // 第二次: 失败 (session consumed)
        write_message(&mut writer, &consume_req.to_bytes()).await.unwrap();
        let data = read_message(&mut reader).await.unwrap();
        assert!(matches!(VaultResponse::from_bytes(&data).unwrap(), VaultResponse::Error(_)));

        handle.abort();
    }

    #[tokio::test]
    async fn server_multiple_requests_one_connection() {
        let (sock_path, handle, _dir) = setup_server_client().await;

        let stream = tokio::net::UnixStream::connect(&sock_path).await.unwrap();
        let (mut reader, mut writer) = stream.into_split();

        // Request 1: Ping
        write_message(&mut writer, &VaultRequest::Ping.to_bytes()).await.unwrap();
        let data = read_message(&mut reader).await.unwrap();
        assert!(matches!(VaultResponse::from_bytes(&data).unwrap(), VaultResponse::Pong));

        // Request 2: TG URL
        let req = VaultRequest::BuildTgApiUrl { method: "sendMessage".into() };
        write_message(&mut writer, &req.to_bytes()).await.unwrap();
        let data = read_message(&mut reader).await.unwrap();
        match VaultResponse::from_bytes(&data).unwrap() {
            VaultResponse::Ok(url) => assert!(url.contains("sendMessage")),
            other => panic!("expected Ok, got {:?}", other),
        }

        // Request 3: DC Auth
        write_message(&mut writer, &VaultRequest::BuildDcAuthHeader.to_bytes()).await.unwrap();
        let data = read_message(&mut reader).await.unwrap();
        match VaultResponse::from_bytes(&data).unwrap() {
            VaultResponse::Ok(h) => assert!(h.starts_with("Bot ")),
            other => panic!("expected Ok, got {:?}", other),
        }

        handle.abort();
    }
}
