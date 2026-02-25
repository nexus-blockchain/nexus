// Token Vault Client — 通过 Unix socket IPC 连接 vault 进程
//
// 替代直连 TokenVault, 提供相同的 build_* 接口
// Token 永远不在主进程内存中出现 (仅存在于 vault 进程)
//
// 使用方式:
// - VaultClient::connect(socket_path) → 连接到 vault 进程
// - client.build_tg_api_url("sendMessage") → 通过 IPC 获取拼接后的 URL
//
// 连接池: 使用持久连接, 支持自动重连


use tokio::net::UnixStream;
use tokio::sync::Mutex;
use tracing::{warn, debug};
use zeroize::Zeroizing;

use crate::error::{BotError, BotResult};
use crate::tee::vault_ipc::{
    VaultRequest, VaultResponse, IpcCipher,
    read_message, write_message, read_encrypted, write_encrypted,
};

/// Vault IPC 客户端
pub struct VaultClient {
    socket_path: String,
    conn: Mutex<Option<VaultConnection>>,
    /// IPC 加密密钥 (None = 明文模式)
    ipc_key: Option<[u8; 32]>,
}

/// 持久化连接
struct VaultConnection {
    reader: tokio::net::unix::OwnedReadHalf,
    writer: tokio::net::unix::OwnedWriteHalf,
    cipher: Option<IpcCipher>,
}

#[allow(dead_code)]
impl VaultClient {
    /// 连接到 vault 服务端 (明文模式)
    pub async fn connect(socket_path: &str) -> BotResult<Self> {
        let client = Self {
            socket_path: socket_path.to_string(),
            conn: Mutex::new(None),
            ipc_key: None,
        };
        client.ensure_connected().await?;
        Ok(client)
    }

    /// 连接到 vault 服务端 (加密模式)
    pub async fn connect_encrypted(socket_path: &str, key: [u8; 32]) -> BotResult<Self> {
        let client = Self {
            socket_path: socket_path.to_string(),
            conn: Mutex::new(None),
            ipc_key: Some(key),
        };
        client.ensure_connected().await?;
        Ok(client)
    }

    /// 确保连接可用 (自动重连)
    async fn ensure_connected(&self) -> BotResult<()> {
        let mut conn = self.conn.lock().await;
        if conn.is_none() {
            let stream = UnixStream::connect(&self.socket_path).await
                .map_err(|e| BotError::EnclaveError(format!("vault connect: {}", e)))?;
            let (reader, writer) = stream.into_split();
            let cipher = self.ipc_key.map(|k| IpcCipher::new_client(&k));
            *conn = Some(VaultConnection { reader, writer, cipher });
            debug!(socket = %self.socket_path, encrypted = self.ipc_key.is_some(), "Vault IPC 连接已建立");
        }
        Ok(())
    }

    /// 发送请求并接收响应
    async fn request(&self, req: VaultRequest) -> BotResult<VaultResponse> {
        let mut retries = 0;
        // 每次重连时需要新建 cipher (重置 nonce 计数器)
        loop {
            self.ensure_connected().await?;
            let mut conn_guard = self.conn.lock().await;
            if let Some(ref mut conn) = *conn_guard {
                // 发送请求
                let write_result = match (&self.ipc_key, &mut conn.cipher) {
                    (Some(_), Some(ref c)) => {
                        let payload = req.to_bytes();
                        write_encrypted(&mut conn.writer, &payload[4..], c).await
                    }
                    _ => {
                        let msg = req.to_bytes();
                        write_message(&mut conn.writer, &msg).await
                    }
                };
                if let Err(e) = write_result {
                    warn!(error = %e, "Vault IPC 写入失败, 重连");
                    *conn_guard = None;
                    drop(conn_guard);
                    retries += 1;
                    if retries > 2 {
                        return Err(BotError::EnclaveError(format!("vault write failed after retries: {}", e)));
                    }
                    continue;
                }
                // 接收响应
                let read_result = match (&self.ipc_key, &mut conn.cipher) {
                    (Some(_), Some(ref c)) => read_encrypted(&mut conn.reader, c).await,
                    _ => read_message(&mut conn.reader).await,
                };
                match read_result {
                    Ok(data) => {
                        let resp = VaultResponse::from_bytes(&data)
                            .map_err(|e| BotError::EnclaveError(format!("vault response parse: {}", e)))?;
                        return Ok(resp);
                    }
                    Err(e) => {
                        warn!(error = %e, "Vault IPC 读取失败, 重连");
                        *conn_guard = None;
                        drop(conn_guard);
                        retries += 1;
                        if retries > 2 {
                            return Err(BotError::EnclaveError(format!("vault read failed after retries: {}", e)));
                        }
                        continue;
                    }
                }
            } else {
                return Err(BotError::EnclaveError("vault connection lost".into()));
            }
        }
    }

    /// 构建 Telegram API URL (通过 IPC)
    pub async fn build_tg_api_url(&self, method: &str) -> BotResult<Zeroizing<String>> {
        let resp = self.request(VaultRequest::BuildTgApiUrl { method: method.into() }).await?;
        match resp {
            VaultResponse::Ok(url) => Ok(Zeroizing::new(url)),
            VaultResponse::Error(e) => Err(BotError::EnclaveError(e)),
            other => Err(BotError::EnclaveError(format!("unexpected response: {:?}", other))),
        }
    }

    /// 构建 Discord Auth Header (通过 IPC)
    pub async fn build_dc_auth_header(&self) -> BotResult<Zeroizing<String>> {
        let resp = self.request(VaultRequest::BuildDcAuthHeader).await?;
        match resp {
            VaultResponse::Ok(header) => Ok(Zeroizing::new(header)),
            VaultResponse::Error(e) => Err(BotError::EnclaveError(e)),
            other => Err(BotError::EnclaveError(format!("unexpected response: {:?}", other))),
        }
    }

    /// 构建 Discord IDENTIFY payload (通过 IPC)
    pub async fn build_dc_identify_payload(&self, intents: u64) -> BotResult<Zeroizing<String>> {
        let resp = self.request(VaultRequest::BuildDcIdentifyPayload { intents }).await?;
        match resp {
            VaultResponse::Ok(payload) => Ok(Zeroizing::new(payload)),
            VaultResponse::Error(e) => Err(BotError::EnclaveError(e)),
            other => Err(BotError::EnclaveError(format!("unexpected response: {:?}", other))),
        }
    }

    /// 派生 bot_id_hash (通过 IPC)
    pub async fn derive_tg_bot_id_hash(&self) -> BotResult<[u8; 32]> {
        let resp = self.request(VaultRequest::DeriveTgBotIdHash).await?;
        match resp {
            VaultResponse::OkHash(hash) => Ok(hash),
            VaultResponse::Error(e) => Err(BotError::EnclaveError(e)),
            other => Err(BotError::EnclaveError(format!("unexpected response: {:?}", other))),
        }
    }

    /// 健康检查
    pub async fn ping(&self) -> BotResult<()> {
        let resp = self.request(VaultRequest::Ping).await?;
        match resp {
            VaultResponse::Pong => Ok(()),
            VaultResponse::Error(e) => Err(BotError::EnclaveError(e)),
            other => Err(BotError::EnclaveError(format!("unexpected ping response: {:?}", other))),
        }
    }

    /// 安全关闭 vault (zeroize all tokens)
    pub async fn shutdown(&self) -> BotResult<()> {
        let resp = self.request(VaultRequest::Shutdown).await?;
        match resp {
            VaultResponse::ShutdownAck => Ok(()),
            _ => Ok(()), // 即使响应异常也不报错
        }
    }

    /// 创建 SGX Provision 会话 (通过 IPC 委托 SGX vault)
    ///
    /// 返回 (session_id, quote, x25519_pk, tee_measurement)
    pub async fn create_provision_session(&self) -> BotResult<(
        [u8; 16], Vec<u8>, [u8; 32], Vec<u8>,
    )> {
        let resp = self.request(VaultRequest::CreateProvisionSession).await?;
        match resp {
            VaultResponse::ProvisionSessionCreated { session_id, quote, x25519_pk, tee_measurement } => {
                Ok((session_id, quote, x25519_pk, tee_measurement))
            }
            VaultResponse::Error(e) => Err(BotError::EnclaveError(e)),
            other => Err(BotError::EnclaveError(format!("unexpected provision response: {:?}", other))),
        }
    }

    /// 消费 SGX Provision 会话: 将密文转发到 SGX vault 解密 + 注入
    ///
    /// Token 明文从不经过主进程, 仅在 SGX enclave 内解密
    pub async fn consume_provision_session(
        &self,
        session_id: [u8; 16],
        ephemeral_pk: [u8; 32],
        ciphertext: Vec<u8>,
        nonce: [u8; 12],
        platform: String,
    ) -> BotResult<Option<[u8; 32]>> {
        let resp = self.request(VaultRequest::ConsumeProvisionSession {
            session_id, ephemeral_pk, ciphertext, nonce, platform,
        }).await?;
        match resp {
            VaultResponse::TokenInjected { bot_id_hash } => Ok(bot_id_hash),
            VaultResponse::Error(e) => Err(BotError::EnclaveError(e)),
            other => Err(BotError::EnclaveError(format!("unexpected inject response: {:?}", other))),
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// VaultProvider trait — 统一 in-process 和 IPC 两种模式
// ═══════════════════════════════════════════════════════════════

/// Token Vault 提供者 trait
///
/// 统一 TokenVault (in-process) 和 VaultClient (IPC) 两种模式
/// 所有 Executor/Gateway 通过此 trait 访问 Token
#[async_trait::async_trait]
pub trait VaultProvider: Send + Sync {
    async fn build_tg_api_url(&self, method: &str) -> BotResult<Zeroizing<String>>;
    async fn build_dc_auth_header(&self) -> BotResult<Zeroizing<String>>;
    async fn build_dc_identify_payload(&self, intents: u64) -> BotResult<Zeroizing<String>>;
}

/// In-process TokenVault 实现 VaultProvider
#[async_trait::async_trait]
impl VaultProvider for crate::tee::token_vault::TokenVault {
    async fn build_tg_api_url(&self, method: &str) -> BotResult<Zeroizing<String>> {
        self.build_tg_api_url(method)
    }
    async fn build_dc_auth_header(&self) -> BotResult<Zeroizing<String>> {
        self.build_dc_auth_header()
    }
    async fn build_dc_identify_payload(&self, intents: u64) -> BotResult<Zeroizing<String>> {
        self.build_dc_identify_payload(intents)
    }
}

/// In-process Arc<RwLock<TokenVault>> 实现 VaultProvider
/// 用于 inprocess 模式: provision 路由需要写访问, executor 需要读访问
#[async_trait::async_trait]
impl VaultProvider for tokio::sync::RwLock<crate::tee::token_vault::TokenVault> {
    async fn build_tg_api_url(&self, method: &str) -> BotResult<Zeroizing<String>> {
        self.read().await.build_tg_api_url(method)
    }
    async fn build_dc_auth_header(&self) -> BotResult<Zeroizing<String>> {
        self.read().await.build_dc_auth_header()
    }
    async fn build_dc_identify_payload(&self, intents: u64) -> BotResult<Zeroizing<String>> {
        self.read().await.build_dc_identify_payload(intents)
    }
}

/// IPC VaultClient 实现 VaultProvider
#[async_trait::async_trait]
impl VaultProvider for VaultClient {
    async fn build_tg_api_url(&self, method: &str) -> BotResult<Zeroizing<String>> {
        self.build_tg_api_url(method).await
    }
    async fn build_dc_auth_header(&self) -> BotResult<Zeroizing<String>> {
        self.build_dc_auth_header().await
    }
    async fn build_dc_identify_payload(&self, intents: u64) -> BotResult<Zeroizing<String>> {
        self.build_dc_identify_payload(intents).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::tee::token_vault::TokenVault;
    use crate::tee::vault_server::VaultServer;
    use crate::tee::vault_ipc;

    async fn setup() -> (String, tokio::task::JoinHandle<()>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_str().unwrap().to_string();

        let sock_path = vault_ipc::default_socket_path(&dir_path);
        let mut vault = TokenVault::new();
        vault.set_telegram_token("client_test:TG_TOKEN".into());
        vault.set_discord_token("client_test_DC_TOKEN".into());

        let server = VaultServer::new(vault, sock_path.clone());
        let handle = tokio::spawn(async move {
            let _ = server.run().await;
        });

        // 等待 server
        for _ in 0..50 {
            if std::path::Path::new(&sock_path).exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        (sock_path, handle, dir)
    }

    #[tokio::test]
    async fn client_build_tg_api_url() {
        let (sock_path, handle, _dir) = setup().await;
        let client = VaultClient::connect(&sock_path).await.unwrap();

        let url = client.build_tg_api_url("getMe").await.unwrap();
        assert!(url.contains("client_test:TG_TOKEN"));
        assert!(url.contains("/getMe"));

        handle.abort();
    }

    #[tokio::test]
    async fn client_build_dc_auth_header() {
        let (sock_path, handle, _dir) = setup().await;
        let client = VaultClient::connect(&sock_path).await.unwrap();

        let header = client.build_dc_auth_header().await.unwrap();
        assert_eq!(header.as_str(), "Bot client_test_DC_TOKEN");

        handle.abort();
    }

    #[tokio::test]
    async fn client_ping() {
        let (sock_path, handle, _dir) = setup().await;
        let client = VaultClient::connect(&sock_path).await.unwrap();

        client.ping().await.unwrap();

        handle.abort();
    }

    #[tokio::test]
    async fn client_derive_hash() {
        let (sock_path, handle, _dir) = setup().await;
        let client = VaultClient::connect(&sock_path).await.unwrap();

        let hash = client.derive_tg_bot_id_hash().await.unwrap();
        assert_ne!(hash, [0u8; 32]);

        handle.abort();
    }

    #[tokio::test]
    async fn vault_provider_trait_in_process() {
        let mut vault = TokenVault::new();
        vault.set_telegram_token("trait_test:TOKEN".into());

        let provider: Arc<dyn VaultProvider> = Arc::new(vault);
        let url = provider.build_tg_api_url("test").await.unwrap();
        assert!(url.contains("trait_test:TOKEN"));
    }

    #[tokio::test]
    async fn vault_provider_trait_ipc() {
        let (sock_path, handle, _dir) = setup().await;
        let client = VaultClient::connect(&sock_path).await.unwrap();

        let provider: Arc<dyn VaultProvider> = Arc::new(client);
        let url = provider.build_tg_api_url("test").await.unwrap();
        assert!(url.contains("client_test:TG_TOKEN"));

        handle.abort();
    }

    // ── Encrypted IPC E2E test ──

    async fn setup_encrypted() -> (String, [u8; 32], tokio::task::JoinHandle<()>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_str().unwrap().to_string();

        let sock_path = vault_ipc::default_socket_path(&dir_path);
        let ipc_key = vault_ipc::ensure_ipc_key(&dir_path).unwrap();

        let mut vault = TokenVault::new();
        vault.set_telegram_token("enc_test:SECRET_TG".into());
        vault.set_discord_token("enc_test_SECRET_DC".into());

        let server = VaultServer::with_encryption(vault, sock_path.clone(), ipc_key);
        let handle = tokio::spawn(async move {
            let _ = server.run().await;
        });

        for _ in 0..50 {
            if std::path::Path::new(&sock_path).exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        (sock_path, ipc_key, handle, dir)
    }

    #[tokio::test]
    async fn encrypted_ipc_e2e() {
        let (sock_path, ipc_key, handle, _dir) = setup_encrypted().await;
        let client = VaultClient::connect_encrypted(&sock_path, ipc_key).await.unwrap();

        // Ping
        client.ping().await.unwrap();

        // TG URL
        let url = client.build_tg_api_url("getMe").await.unwrap();
        assert!(url.contains("enc_test:SECRET_TG"));
        assert!(url.contains("/getMe"));

        // DC Auth
        let header = client.build_dc_auth_header().await.unwrap();
        assert_eq!(header.as_str(), "Bot enc_test_SECRET_DC");

        // Hash
        let hash = client.derive_tg_bot_id_hash().await.unwrap();
        assert_ne!(hash, [0u8; 32]);

        handle.abort();
    }

    #[tokio::test]
    async fn encrypted_client_plain_server_rejected() {
        // 加密客户端连接到明文服务端 → 应该失败
        let (sock_path, handle, _dir) = setup().await; // 明文 server
        let key = [0x42u8; 32];
        let client = VaultClient::connect_encrypted(&sock_path, key).await.unwrap();

        // 请求应该失败 (server 无法解析加密数据)
        let result = client.ping().await;
        assert!(result.is_err());

        handle.abort();
    }
}
