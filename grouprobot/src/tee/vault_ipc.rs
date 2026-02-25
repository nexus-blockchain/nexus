// Token Vault IPC 协议 — 主进程与 vault 进程间通信
//
// 协议格式 (length-prefixed binary):
//   [msg_len:4LE][msg_type:1][payload...]
//
// 安全属性:
// - Unix domain socket (仅本机进程间)
// - Token 只在 vault 进程内存中, 主进程只得到拼接后的 URL/Header
// - vault 进程在 Gramine SGX 中运行时, Token 受硬件保护

use std::io;
use std::sync::atomic::{AtomicU64, Ordering};

use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use aes_gcm::aead::Aead;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// ═══════════════════════════════════════════════════════════════
// 请求/响应类型
// ═══════════════════════════════════════════════════════════════

/// IPC 请求
#[derive(Debug, Clone)]
pub enum VaultRequest {
    /// 构建 Telegram API URL: method → "https://api.telegram.org/bot<token>/<method>"
    BuildTgApiUrl { method: String },
    /// 构建 Discord Auth Header → "Bot <token>"
    BuildDcAuthHeader,
    /// 构建 Discord IDENTIFY payload → JSON string
    BuildDcIdentifyPayload { intents: u64 },
    /// 派生 bot_id_hash (SHA256 of TG token)
    DeriveTgBotIdHash,
    /// 健康检查
    Ping,
    /// 安全关闭 (zeroize all tokens)
    Shutdown,
    /// 创建 RA-TLS Provision 会话 (SGX vault 生成 Quote + X25519 密钥对)
    CreateProvisionSession,
    /// 消费 Provision 会话: SGX vault 内 ECDH 解密 + 注入 Token
    ConsumeProvisionSession {
        session_id: [u8; 16],
        ephemeral_pk: [u8; 32],
        ciphertext: Vec<u8>,
        nonce: [u8; 12],
        platform: String,
    },
}

/// IPC 响应
#[derive(Debug, Clone)]
pub enum VaultResponse {
    /// 成功: 返回字符串结果
    Ok(String),
    /// 成功: 返回 32 字节 hash
    OkHash([u8; 32]),
    /// 错误
    Error(String),
    /// Pong (健康检查响应)
    Pong,
    /// 已关闭
    ShutdownAck,
    /// Provision 会话已创建 (SGX Quote + X25519 PK)
    ProvisionSessionCreated {
        session_id: [u8; 16],
        quote: Vec<u8>,
        x25519_pk: [u8; 32],
        tee_measurement: Vec<u8>,
    },
    /// Token 已在 SGX 内解密并注入
    TokenInjected {
        bot_id_hash: Option<[u8; 32]>,
    },
}

// ═══════════════════════════════════════════════════════════════
// 消息类型标识
// ═══════════════════════════════════════════════════════════════

const MSG_BUILD_TG_API_URL: u8 = 1;
const MSG_BUILD_DC_AUTH_HEADER: u8 = 2;
const MSG_BUILD_DC_IDENTIFY_PAYLOAD: u8 = 3;
const MSG_DERIVE_TG_BOT_ID_HASH: u8 = 4;
const MSG_PING: u8 = 10;
const MSG_SHUTDOWN: u8 = 11;
const MSG_CREATE_PROVISION_SESSION: u8 = 20;
const MSG_CONSUME_PROVISION_SESSION: u8 = 21;

const RESP_OK_STRING: u8 = 128;
const RESP_OK_HASH: u8 = 129;
const RESP_ERROR: u8 = 130;
const RESP_PONG: u8 = 131;
const RESP_SHUTDOWN_ACK: u8 = 132;
const RESP_PROVISION_SESSION: u8 = 133;
const RESP_TOKEN_INJECTED: u8 = 134;

/// 最大消息长度 (64KB, 防止恶意超大消息)
const MAX_MSG_LEN: u32 = 65536;

// ═══════════════════════════════════════════════════════════════
// 序列化
// ═══════════════════════════════════════════════════════════════

impl VaultRequest {
    /// 序列化为字节
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        match self {
            Self::BuildTgApiUrl { method } => {
                payload.push(MSG_BUILD_TG_API_URL);
                payload.extend_from_slice(method.as_bytes());
            }
            Self::BuildDcAuthHeader => {
                payload.push(MSG_BUILD_DC_AUTH_HEADER);
            }
            Self::BuildDcIdentifyPayload { intents } => {
                payload.push(MSG_BUILD_DC_IDENTIFY_PAYLOAD);
                payload.extend_from_slice(&intents.to_le_bytes());
            }
            Self::DeriveTgBotIdHash => {
                payload.push(MSG_DERIVE_TG_BOT_ID_HASH);
            }
            Self::Ping => {
                payload.push(MSG_PING);
            }
            Self::Shutdown => {
                payload.push(MSG_SHUTDOWN);
            }
            Self::CreateProvisionSession => {
                payload.push(MSG_CREATE_PROVISION_SESSION);
            }
            Self::ConsumeProvisionSession { session_id, ephemeral_pk, ciphertext, nonce, platform } => {
                payload.push(MSG_CONSUME_PROVISION_SESSION);
                payload.extend_from_slice(session_id);
                payload.extend_from_slice(ephemeral_pk);
                payload.extend_from_slice(nonce);
                // ciphertext length (4 bytes LE) + ciphertext
                payload.extend_from_slice(&(ciphertext.len() as u32).to_le_bytes());
                payload.extend_from_slice(ciphertext);
                // platform as UTF-8
                payload.extend_from_slice(platform.as_bytes());
            }
        }
        // 前缀: 4字节小端长度
        let len = payload.len() as u32;
        let mut msg = Vec::with_capacity(4 + payload.len());
        msg.extend_from_slice(&len.to_le_bytes());
        msg.extend(payload);
        msg
    }

    /// 从字节反序列化
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.is_empty() {
            return Err("empty request".into());
        }
        match data[0] {
            MSG_BUILD_TG_API_URL => {
                let method = String::from_utf8(data[1..].to_vec())
                    .map_err(|e| format!("invalid UTF-8: {}", e))?;
                Ok(Self::BuildTgApiUrl { method })
            }
            MSG_BUILD_DC_AUTH_HEADER => Ok(Self::BuildDcAuthHeader),
            MSG_BUILD_DC_IDENTIFY_PAYLOAD => {
                if data.len() < 9 {
                    return Err("intents payload too short".into());
                }
                let intents = u64::from_le_bytes([
                    data[1], data[2], data[3], data[4],
                    data[5], data[6], data[7], data[8],
                ]);
                Ok(Self::BuildDcIdentifyPayload { intents })
            }
            MSG_DERIVE_TG_BOT_ID_HASH => Ok(Self::DeriveTgBotIdHash),
            MSG_PING => Ok(Self::Ping),
            MSG_SHUTDOWN => Ok(Self::Shutdown),
            MSG_CREATE_PROVISION_SESSION => Ok(Self::CreateProvisionSession),
            MSG_CONSUME_PROVISION_SESSION => {
                // layout: [type:1][session_id:16][ephemeral_pk:32][nonce:12][ct_len:4][ciphertext:N][platform:...]
                if data.len() < 1 + 16 + 32 + 12 + 4 {
                    return Err("ConsumeProvisionSession payload too short".into());
                }
                let mut session_id = [0u8; 16];
                session_id.copy_from_slice(&data[1..17]);
                let mut ephemeral_pk = [0u8; 32];
                ephemeral_pk.copy_from_slice(&data[17..49]);
                let mut nonce = [0u8; 12];
                nonce.copy_from_slice(&data[49..61]);
                let ct_len = u32::from_le_bytes([data[61], data[62], data[63], data[64]]) as usize;
                if data.len() < 65 + ct_len {
                    return Err("ciphertext truncated".into());
                }
                let ciphertext = data[65..65 + ct_len].to_vec();
                let platform = String::from_utf8(data[65 + ct_len..].to_vec())
                    .map_err(|e| format!("invalid platform UTF-8: {}", e))?;
                Ok(Self::ConsumeProvisionSession { session_id, ephemeral_pk, ciphertext, nonce, platform })
            }
            other => Err(format!("unknown request type: {}", other)),
        }
    }
}

impl VaultResponse {
    /// 序列化为字节
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        match self {
            Self::Ok(s) => {
                payload.push(RESP_OK_STRING);
                payload.extend_from_slice(s.as_bytes());
            }
            Self::OkHash(hash) => {
                payload.push(RESP_OK_HASH);
                payload.extend_from_slice(hash);
            }
            Self::Error(e) => {
                payload.push(RESP_ERROR);
                payload.extend_from_slice(e.as_bytes());
            }
            Self::Pong => {
                payload.push(RESP_PONG);
            }
            Self::ShutdownAck => {
                payload.push(RESP_SHUTDOWN_ACK);
            }
            Self::ProvisionSessionCreated { session_id, quote, x25519_pk, tee_measurement } => {
                payload.push(RESP_PROVISION_SESSION);
                payload.extend_from_slice(session_id);
                payload.extend_from_slice(x25519_pk);
                // tee_measurement length (1 byte) + tee_measurement
                payload.push(tee_measurement.len() as u8);
                payload.extend_from_slice(tee_measurement);
                // quote (remaining bytes)
                payload.extend_from_slice(quote);
            }
            Self::TokenInjected { bot_id_hash } => {
                payload.push(RESP_TOKEN_INJECTED);
                match bot_id_hash {
                    Some(h) => {
                        payload.push(1); // has_hash flag
                        payload.extend_from_slice(h);
                    }
                    None => {
                        payload.push(0);
                    }
                }
            }
        }
        let len = payload.len() as u32;
        let mut msg = Vec::with_capacity(4 + payload.len());
        msg.extend_from_slice(&len.to_le_bytes());
        msg.extend(payload);
        msg
    }

    /// 从字节反序列化
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.is_empty() {
            return Err("empty response".into());
        }
        match data[0] {
            RESP_OK_STRING => {
                let s = String::from_utf8(data[1..].to_vec())
                    .map_err(|e| format!("invalid UTF-8: {}", e))?;
                Ok(Self::Ok(s))
            }
            RESP_OK_HASH => {
                if data.len() < 33 {
                    return Err("hash response too short".into());
                }
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&data[1..33]);
                Ok(Self::OkHash(hash))
            }
            RESP_ERROR => {
                let e = String::from_utf8(data[1..].to_vec())
                    .map_err(|e| format!("invalid UTF-8: {}", e))?;
                Ok(Self::Error(e))
            }
            RESP_PONG => Ok(Self::Pong),
            RESP_SHUTDOWN_ACK => Ok(Self::ShutdownAck),
            RESP_PROVISION_SESSION => {
                // layout: [type:1][session_id:16][x25519_pk:32][meas_len:1][measurement:N][quote:...]
                if data.len() < 1 + 16 + 32 + 1 {
                    return Err("ProvisionSession response too short".into());
                }
                let mut session_id = [0u8; 16];
                session_id.copy_from_slice(&data[1..17]);
                let mut x25519_pk = [0u8; 32];
                x25519_pk.copy_from_slice(&data[17..49]);
                let meas_len = data[49] as usize;
                if data.len() < 50 + meas_len {
                    return Err("measurement truncated".into());
                }
                let tee_measurement = data[50..50 + meas_len].to_vec();
                let quote = data[50 + meas_len..].to_vec();
                Ok(Self::ProvisionSessionCreated { session_id, quote, x25519_pk, tee_measurement })
            }
            RESP_TOKEN_INJECTED => {
                if data.len() < 2 {
                    return Err("TokenInjected response too short".into());
                }
                let bot_id_hash = if data[1] == 1 && data.len() >= 34 {
                    let mut h = [0u8; 32];
                    h.copy_from_slice(&data[2..34]);
                    Some(h)
                } else {
                    None
                };
                Ok(Self::TokenInjected { bot_id_hash })
            }
            other => Err(format!("unknown response type: {}", other)),
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// 异步读写帮助函数
// ═══════════════════════════════════════════════════════════════

/// 从 stream 读取一条 length-prefixed 消息
pub async fn read_message<R: AsyncReadExt + Unpin>(reader: &mut R) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf);
    if len > MAX_MSG_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message too large: {} bytes", len),
        ));
    }
    let mut data = vec![0u8; len as usize];
    reader.read_exact(&mut data).await?;
    Ok(data)
}

/// 向 stream 写入一条消息 (已含 length prefix)
pub async fn write_message<W: AsyncWriteExt + Unpin>(writer: &mut W, msg: &[u8]) -> io::Result<()> {
    writer.write_all(msg).await?;
    writer.flush().await?;
    Ok(())
}

/// 默认 socket 路径
pub fn default_socket_path(data_dir: &str) -> String {
    format!("{}/vault.sock", data_dir)
}

// ═══════════════════════════════════════════════════════════════
// IPC 通道加密 — AES-256-GCM
// ═══════════════════════════════════════════════════════════════

/// IPC 通道加密器
///
/// 使用 AES-256-GCM 加密 Unix socket 上的所有消息,
/// 防止同机 root 用户通过 socat 等工具监听 IPC 流量。
///
/// Nonce 策略: 8 字节方向标识 + 4 字节递增计数器
/// - 发送方向: nonce[0..8] = "CLNT->SV" 或 "SRVR->CL"
/// - 计数器:   nonce[8..12] = send_counter (小端)
pub struct IpcCipher {
    cipher: Aes256Gcm,
    /// 方向前缀 (区分 client→server 和 server→client)
    send_prefix: [u8; 8],
    recv_prefix: [u8; 8],
    send_counter: AtomicU64,
    recv_counter: AtomicU64,
}

/// IPC 加密消息格式:
///   [encrypted_len:4LE][nonce:12][ciphertext+tag:N]
const IPC_NONCE_LEN: usize = 12;

impl IpcCipher {
    /// 创建服务端加密器 (send=SRVR->CL, recv=CLNT->SV)
    pub fn new_server(key: &[u8; 32]) -> Self {
        Self {
            cipher: Aes256Gcm::new(key.into()),
            send_prefix: *b"SRVR->CL",
            recv_prefix: *b"CLNT->SV",
            send_counter: AtomicU64::new(0),
            recv_counter: AtomicU64::new(0),
        }
    }

    /// 创建客户端加密器 (send=CLNT->SV, recv=SRVR->CL)
    pub fn new_client(key: &[u8; 32]) -> Self {
        Self {
            cipher: Aes256Gcm::new(key.into()),
            send_prefix: *b"CLNT->SV",
            recv_prefix: *b"SRVR->CL",
            send_counter: AtomicU64::new(0),
            recv_counter: AtomicU64::new(0),
        }
    }

    /// 加密消息 payload
    fn encrypt(&self, plaintext: &[u8]) -> io::Result<Vec<u8>> {
        let counter = self.send_counter.fetch_add(1, Ordering::SeqCst);
        let nonce_bytes = self.build_nonce(&self.send_prefix, counter);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self.cipher.encrypt(nonce, plaintext)
            .map_err(|e| io::Error::other(format!("IPC encrypt: {}", e)))?;

        // [nonce:12][ciphertext+tag]
        let mut result = Vec::with_capacity(IPC_NONCE_LEN + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend(ciphertext);
        Ok(result)
    }

    /// 解密消息
    fn decrypt(&self, encrypted: &[u8]) -> io::Result<Vec<u8>> {
        if encrypted.len() < IPC_NONCE_LEN + 16 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "encrypted IPC message too short",
            ));
        }

        let nonce_bytes = &encrypted[..IPC_NONCE_LEN];
        let ciphertext = &encrypted[IPC_NONCE_LEN..];

        // 验证 nonce 前缀匹配预期方向
        if nonce_bytes[..8] != self.recv_prefix {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "IPC nonce direction mismatch",
            ));
        }

        // 验证计数器单调递增 (防重放)
        let counter = u32::from_le_bytes([
            nonce_bytes[8], nonce_bytes[9], nonce_bytes[10], nonce_bytes[11],
        ]) as u64;
        let expected = self.recv_counter.fetch_add(1, Ordering::SeqCst);
        if counter != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("IPC nonce counter mismatch: got {}, expected {}", counter, expected),
            ));
        }

        let nonce = Nonce::from_slice(nonce_bytes);
        self.cipher.decrypt(nonce, ciphertext)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("IPC decrypt: {}", e)))
    }

    fn build_nonce(&self, prefix: &[u8; 8], counter: u64) -> [u8; 12] {
        let mut nonce = [0u8; 12];
        nonce[..8].copy_from_slice(prefix);
        nonce[8..12].copy_from_slice(&(counter as u32).to_le_bytes());
        nonce
    }
}

/// 写入加密消息: [encrypted_len:4LE][nonce:12][ciphertext+tag]
pub async fn write_encrypted<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    payload: &[u8],
    cipher: &IpcCipher,
) -> io::Result<()> {
    let encrypted = cipher.encrypt(payload)?;
    let len = encrypted.len() as u32;
    writer.write_all(&len.to_le_bytes()).await?;
    writer.write_all(&encrypted).await?;
    writer.flush().await?;
    Ok(())
}

/// 读取并解密消息
pub async fn read_encrypted<R: AsyncReadExt + Unpin>(
    reader: &mut R,
    cipher: &IpcCipher,
) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf);
    if len > MAX_MSG_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("encrypted IPC message too large: {} bytes", len),
        ));
    }
    let mut encrypted = vec![0u8; len as usize];
    reader.read_exact(&mut encrypted).await?;
    cipher.decrypt(&encrypted)
}

// ═══════════════════════════════════════════════════════════════
// IPC 密钥文件管理
// ═══════════════════════════════════════════════════════════════

const IPC_KEY_FILENAME: &str = "ipc_session.key";

/// 确保 IPC 密钥存在: 有则加载, 无则生成
pub fn ensure_ipc_key(data_dir: &str) -> io::Result<[u8; 32]> {
    let path = format!("{}/{}", data_dir, IPC_KEY_FILENAME);
    if let Ok(data) = std::fs::read(&path) {
        if data.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&data);
            return Ok(key);
        }
        // 密钥文件损坏, 重新生成
    }
    // 生成新密钥
    let mut key = [0u8; 32];
    use rand::RngCore;
    rand::rngs::OsRng.fill_bytes(&mut key);

    // 写入文件 (0o600 权限)
    std::fs::create_dir_all(data_dir)?;
    std::fs::write(&path, key)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrip_tg_api_url() {
        let req = VaultRequest::BuildTgApiUrl { method: "sendMessage".into() };
        let bytes = req.to_bytes();
        // skip 4-byte length prefix
        let parsed = VaultRequest::from_bytes(&bytes[4..]).unwrap();
        match parsed {
            VaultRequest::BuildTgApiUrl { method } => assert_eq!(method, "sendMessage"),
            _ => panic!("wrong type"),
        }
    }

    #[test]
    fn request_roundtrip_dc_identify() {
        let req = VaultRequest::BuildDcIdentifyPayload { intents: 33281 };
        let bytes = req.to_bytes();
        let parsed = VaultRequest::from_bytes(&bytes[4..]).unwrap();
        match parsed {
            VaultRequest::BuildDcIdentifyPayload { intents } => assert_eq!(intents, 33281),
            _ => panic!("wrong type"),
        }
    }

    #[test]
    fn request_roundtrip_simple() {
        for req in [VaultRequest::BuildDcAuthHeader, VaultRequest::DeriveTgBotIdHash,
                     VaultRequest::Ping, VaultRequest::Shutdown] {
            let bytes = req.to_bytes();
            let parsed = VaultRequest::from_bytes(&bytes[4..]).unwrap();
            // just check no error
            assert!(format!("{:?}", parsed).len() > 0);
        }
    }

    #[test]
    fn response_roundtrip_ok_string() {
        let resp = VaultResponse::Ok("https://api.telegram.org/botXXX/send".into());
        let bytes = resp.to_bytes();
        let parsed = VaultResponse::from_bytes(&bytes[4..]).unwrap();
        match parsed {
            VaultResponse::Ok(s) => assert!(s.contains("telegram")),
            _ => panic!("wrong type"),
        }
    }

    #[test]
    fn response_roundtrip_ok_hash() {
        let hash = [0xAB; 32];
        let resp = VaultResponse::OkHash(hash);
        let bytes = resp.to_bytes();
        let parsed = VaultResponse::from_bytes(&bytes[4..]).unwrap();
        match parsed {
            VaultResponse::OkHash(h) => assert_eq!(h, hash),
            _ => panic!("wrong type"),
        }
    }

    #[test]
    fn response_roundtrip_error() {
        let resp = VaultResponse::Error("token not set".into());
        let bytes = resp.to_bytes();
        let parsed = VaultResponse::from_bytes(&bytes[4..]).unwrap();
        match parsed {
            VaultResponse::Error(e) => assert_eq!(e, "token not set"),
            _ => panic!("wrong type"),
        }
    }

    #[test]
    fn response_roundtrip_pong_shutdown() {
        for resp in [VaultResponse::Pong, VaultResponse::ShutdownAck] {
            let bytes = resp.to_bytes();
            let parsed = VaultResponse::from_bytes(&bytes[4..]).unwrap();
            assert!(format!("{:?}", parsed).len() > 0);
        }
    }

    #[test]
    fn empty_request_error() {
        assert!(VaultRequest::from_bytes(&[]).is_err());
    }

    #[test]
    fn unknown_request_type_error() {
        assert!(VaultRequest::from_bytes(&[255]).is_err());
    }

    #[tokio::test]
    async fn read_write_message_roundtrip() {
        let req = VaultRequest::BuildTgApiUrl { method: "getMe".into() };
        let msg_bytes = req.to_bytes();

        // 模拟 stream: 用 tokio duplex
        let (mut client, mut server) = tokio::io::duplex(4096);

        // 写入
        write_message(&mut client, &msg_bytes).await.unwrap();
        drop(client); // 关闭写端

        // 读取
        let data = read_message(&mut server).await.unwrap();
        let parsed = VaultRequest::from_bytes(&data).unwrap();
        match parsed {
            VaultRequest::BuildTgApiUrl { method } => assert_eq!(method, "getMe"),
            _ => panic!("wrong type"),
        }
    }

    // ── IpcCipher tests ──

    #[test]
    fn ipc_cipher_encrypt_decrypt_roundtrip() {
        let key = [0x42u8; 32];
        let server_cipher = IpcCipher::new_server(&key);
        let client_cipher = IpcCipher::new_client(&key);

        // client → server
        let plaintext = b"hello vault";
        let encrypted = client_cipher.encrypt(plaintext).unwrap();
        let decrypted = server_cipher.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);

        // server → client
        let response = b"https://api.telegram.org/botTOKEN/sendMessage";
        let encrypted = server_cipher.encrypt(response).unwrap();
        let decrypted = client_cipher.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, response);
    }

    #[test]
    fn ipc_cipher_direction_mismatch_rejected() {
        let key = [0x42u8; 32];
        let server_cipher = IpcCipher::new_server(&key);
        // server encrypts with SRVR->CL prefix
        let encrypted = server_cipher.encrypt(b"test").unwrap();
        // another server tries to decrypt (expects CLNT->SV prefix) → mismatch
        let server2 = IpcCipher::new_server(&key);
        assert!(server2.decrypt(&encrypted).is_err());
    }

    #[test]
    fn ipc_cipher_replay_rejected() {
        let key = [0x42u8; 32];
        let client = IpcCipher::new_client(&key);
        let server = IpcCipher::new_server(&key);

        let enc1 = client.encrypt(b"msg1").unwrap();
        let enc2 = client.encrypt(b"msg2").unwrap();

        // 正常顺序解密
        server.decrypt(&enc1).unwrap();
        server.decrypt(&enc2).unwrap();

        // 重放 enc1 → counter mismatch (expected 2, got 0)
        assert!(server.decrypt(&enc1).is_err());
    }

    #[test]
    fn ipc_cipher_wrong_key_rejected() {
        let key1 = [0x42u8; 32];
        let key2 = [0x99u8; 32];
        let client = IpcCipher::new_client(&key1);
        let server = IpcCipher::new_server(&key2);

        let encrypted = client.encrypt(b"secret").unwrap();
        assert!(server.decrypt(&encrypted).is_err());
    }

    #[tokio::test]
    async fn encrypted_read_write_roundtrip() {
        let key = [0xABu8; 32];
        let client_cipher = IpcCipher::new_client(&key);
        let server_cipher = IpcCipher::new_server(&key);

        let (mut client_w, mut server_r) = tokio::io::duplex(4096);

        // client 发送加密请求
        let req = VaultRequest::BuildTgApiUrl { method: "getMe".into() };
        let payload = req.to_bytes();
        // 只加密 payload 部分 (跳过 4 字节长度前缀)
        write_encrypted(&mut client_w, &payload[4..], &client_cipher).await.unwrap();
        drop(client_w);

        // server 解密读取
        let data = read_encrypted(&mut server_r, &server_cipher).await.unwrap();
        let parsed = VaultRequest::from_bytes(&data).unwrap();
        match parsed {
            VaultRequest::BuildTgApiUrl { method } => assert_eq!(method, "getMe"),
            _ => panic!("wrong type"),
        }
    }

    #[tokio::test]
    async fn encrypted_bidirectional_roundtrip() {
        let key = [0xCDu8; 32];
        let client_cipher = IpcCipher::new_client(&key);
        let server_cipher = IpcCipher::new_server(&key);

        let (mut c2s_w, mut c2s_r) = tokio::io::duplex(4096);
        let (mut s2c_w, mut s2c_r) = tokio::io::duplex(4096);

        // client → server: 请求
        let req = VaultRequest::Ping;
        let payload = req.to_bytes();
        write_encrypted(&mut c2s_w, &payload[4..], &client_cipher).await.unwrap();

        let data = read_encrypted(&mut c2s_r, &server_cipher).await.unwrap();
        assert!(matches!(VaultRequest::from_bytes(&data).unwrap(), VaultRequest::Ping));

        // server → client: 响应
        let resp = VaultResponse::Pong;
        let resp_payload = resp.to_bytes();
        write_encrypted(&mut s2c_w, &resp_payload[4..], &server_cipher).await.unwrap();

        let resp_data = read_encrypted(&mut s2c_r, &client_cipher).await.unwrap();
        assert!(matches!(VaultResponse::from_bytes(&resp_data).unwrap(), VaultResponse::Pong));
    }

    #[test]
    fn ensure_ipc_key_creates_and_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();

        // 首次: 生成
        let key1 = ensure_ipc_key(path).unwrap();
        assert_ne!(key1, [0u8; 32]);

        // 再次: 加载相同密钥
        let key2 = ensure_ipc_key(path).unwrap();
        assert_eq!(key1, key2);
    }
}
