// Peer Client — RA-TLS 连接 peer TEE 节点, 交换 Shamir Share
//
// 用于 K>1 恢复场景:
// - 本地已有 1 个 sealed share
// - 需要从 K-1 个 peer 收集额外 share
// - 每个 peer 连接: RA-TLS 验证 Quote → 请求 share → 接收加密 share
//
// 协议 (HTTP/JSON, RA-TLS 加密传输层):
//   POST /share/request  { ceremony_hash, requester_pk }
//   → 200 { encrypted_share (base64) }
//
// 当前版本: HTTPS 客户端 (TLS, 未验证 SGX Quote — 需配合 Gramine RA-TLS)
// 未来版本: 集成 Intel SGX 远程证明验证

use std::time::Duration;

use futures_util::StreamExt;
use tracing::{info, warn, debug};

use crate::error::{BotError, BotResult};

/// Share 请求
#[derive(serde::Serialize)]
struct ShareRequest {
    /// 仪式 hash (标识哪次 Ceremony 产出的 share)
    ceremony_hash: String,
    /// 请求者公钥 (hex, 用于 peer 验证身份)
    requester_pk: String,
}

/// Share 响应
#[derive(serde::Deserialize)]
struct ShareResponse {
    /// 加密 share 的二进制 (base64)
    share_data: String,
    /// peer 公钥 (hex)
    peer_pk: String,
}

/// Share 请求错误响应
#[derive(serde::Deserialize)]
struct ShareErrorResponse {
    error: String,
}

/// Peer 客户端配置
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PeerClientConfig {
    /// 连接超时 (秒)
    pub connect_timeout_secs: u64,
    /// 请求超时 (秒)
    pub request_timeout_secs: u64,
    /// 最大重试次数 (每个 peer)
    pub max_retries: u32,
    /// 重试基础间隔 (秒, 指数退避)
    pub retry_base_secs: u64,
}

impl Default for PeerClientConfig {
    fn default() -> Self {
        Self {
            connect_timeout_secs: 10,
            request_timeout_secs: 30,
            max_retries: 5,
            retry_base_secs: 2,
        }
    }
}

/// Peer 客户端 — 连接 peer TEE 节点获取 share
pub struct PeerClient {
    http: reqwest::Client,
    config: PeerClientConfig,
}

impl PeerClient {
    /// 创建 peer 客户端
    pub fn new(config: PeerClientConfig) -> BotResult<Self> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(config.connect_timeout_secs))
            .timeout(Duration::from_secs(config.request_timeout_secs))
            // RA-TLS: 未来替换为自定义 TLS verifier (验证 SGX Quote)
            // 当前: 使用系统 CA (适用于 Gramine 内运行的 peer)
            .danger_accept_invalid_certs(false)
            .build()
            .map_err(|e| BotError::EnclaveError(format!("peer client build: {}", e)))?;

        Ok(Self { http, config })
    }

    /// 从单个 peer 请求 share
    #[allow(dead_code)]
    pub async fn request_share(
        &self,
        endpoint: &str,
        ceremony_hash: &[u8; 32],
        requester_pk: &[u8; 32],
    ) -> BotResult<crate::tee::shamir::EcdhEncryptedShare> {
        let url = format!("{}/share/request", endpoint.trim_end_matches('/'));
        let req_body = ShareRequest {
            ceremony_hash: hex::encode(ceremony_hash),
            requester_pk: hex::encode(requester_pk),
        };

        let mut last_error = String::new();

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                let delay = self.config.retry_base_secs * (1u64 << (attempt - 1).min(5));
                debug!(attempt, delay_secs = delay, endpoint, "重试请求 share");
                tokio::time::sleep(Duration::from_secs(delay)).await;
            }

            match self.try_request_share(&url, &req_body).await {
                Ok(share) => {
                    info!(
                        endpoint, share_id = share.encrypted.id, attempt,
                        "成功从 peer 获取 ECDH share"
                    );
                    return Ok(share);
                }
                Err(e) => {
                    last_error = format!("{}", e);
                    warn!(
                        endpoint, attempt, error = %e,
                        "请求 share 失败"
                    );
                }
            }
        }

        Err(BotError::EnclaveError(format!(
            "peer {} unreachable after {} retries: {}",
            endpoint, self.config.max_retries, last_error
        )))
    }

    /// 单次请求尝试
    async fn try_request_share(
        &self,
        url: &str,
        body: &ShareRequest,
    ) -> BotResult<crate::tee::shamir::EcdhEncryptedShare> {
        let resp = self.http.post(url)
            .json(body)
            .send()
            .await
            .map_err(|e| BotError::EnclaveError(format!("peer request: {}", e)))?;

        let status = resp.status();
        if !status.is_success() {
            let error_body: ShareErrorResponse = resp.json().await
                .unwrap_or(ShareErrorResponse { error: format!("HTTP {}", status) });
            return Err(BotError::EnclaveError(format!(
                "peer responded {}: {}", status, error_body.error
            )));
        }

        let share_resp: ShareResponse = resp.json().await
            .map_err(|e| BotError::EnclaveError(format!("peer response parse: {}", e)))?;

        // 解码 base64 share data (ECDH 加密格式: [32 ephemeral_pk][EncryptedShare])
        let share_bytes = base64_decode(&share_resp.share_data)
            .map_err(|e| BotError::EnclaveError(format!("share base64 decode: {}", e)))?;

        let ecdh_share = crate::tee::shamir::ecdh_share_from_bytes(&share_bytes)
            .map_err(|e| BotError::EnclaveError(format!("ecdh share parse: {}", e)))?;

        debug!(peer_pk = %share_resp.peer_pk, share_id = ecdh_share.encrypted.id, "ECDH share 解码成功");
        Ok(ecdh_share)
    }

    /// 从多个 peer 收集 shares (并行请求, first-K-of-N 早返回)
    ///
    /// 使用 FuturesUnordered 实现真正的先到先得: 任意 K-1 个 peer 响应即返回,
    /// 无需等待慢速/超时的 peer。每个 peer 请求包含指数退避重试。
    pub async fn collect_shares(
        &self,
        endpoints: &[String],
        ceremony_hash: &[u8; 32],
        requester_pk: &[u8; 32],
        needed: usize,
    ) -> BotResult<Vec<crate::tee::shamir::EcdhEncryptedShare>> {
        if endpoints.is_empty() {
            return Err(BotError::EnclaveError("no peer endpoints configured".into()));
        }

        info!(
            peers = endpoints.len(), needed,
            "开始从 peer 收集 Shamir shares"
        );

        // 并行请求所有 peer (每个含重试), 用 FuturesUnordered 先到先得
        let mut futures = futures_util::stream::FuturesUnordered::new();
        for (idx, endpoint) in endpoints.iter().enumerate() {
            let client = self.http.clone();
            let config = self.config.clone();
            let ep = endpoint.clone();
            let ch = *ceremony_hash;
            let pk = *requester_pk;

            futures.push(tokio::spawn(async move {
                let peer_client = PeerClient { http: client, config };
                let result = peer_client.request_share(&ep, &ch, &pk).await;
                (idx, result)
            }));
        }

        let mut collected = Vec::new();
        let mut errors = Vec::new();

        while let Some(join_result) = futures.next().await {
            match join_result {
                Ok((idx, Ok(share))) => {
                    collected.push(share);
                    if collected.len() >= needed {
                        info!(collected = collected.len(), "已收集足够 shares");
                        return Ok(collected);
                    }
                }
                Ok((idx, Err(e))) => {
                    errors.push(format!("peer[{}]: {}", idx, e));
                }
                Err(e) => {
                    errors.push(format!("peer task join: {}", e));
                }
            }
        }

        Err(BotError::EnclaveError(format!(
            "collected {}/{} shares, errors: [{}]",
            collected.len(), needed, errors.join("; ")
        )))
    }
}

// ═══════════════════════════════════════════════════════════════
// Base64 帮助函数 (避免添加额外依赖)
// ═══════════════════════════════════════════════════════════════

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base64 编码
pub fn base64_encode(data: &[u8]) -> String {
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(BASE64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(BASE64_CHARS[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(BASE64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(BASE64_CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

/// Base64 解码 (公开接口, 供 ceremony.rs 等模块复用)
pub fn base64_decode_pub(input: &str) -> Result<Vec<u8>, String> {
    base64_decode(input)
}

/// Base64 解码 (内部使用)
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    let input = input.trim_end_matches('=');
    let mut result = Vec::new();

    let decode_char = |c: u8| -> Result<u32, String> {
        match c {
            b'A'..=b'Z' => Ok((c - b'A') as u32),
            b'a'..=b'z' => Ok((c - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((c - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(format!("invalid base64 char: {}", c as char)),
        }
    };

    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let remaining = bytes.len() - i;
        if remaining >= 4 {
            let a = decode_char(bytes[i])?;
            let b = decode_char(bytes[i + 1])?;
            let c = decode_char(bytes[i + 2])?;
            let d = decode_char(bytes[i + 3])?;
            let triple = (a << 18) | (b << 12) | (c << 6) | d;
            result.push(((triple >> 16) & 0xFF) as u8);
            result.push(((triple >> 8) & 0xFF) as u8);
            result.push((triple & 0xFF) as u8);
            i += 4;
        } else if remaining == 3 {
            let a = decode_char(bytes[i])?;
            let b = decode_char(bytes[i + 1])?;
            let c = decode_char(bytes[i + 2])?;
            let triple = (a << 18) | (b << 12) | (c << 6);
            result.push(((triple >> 16) & 0xFF) as u8);
            result.push(((triple >> 8) & 0xFF) as u8);
            i += 3;
        } else if remaining == 2 {
            let a = decode_char(bytes[i])?;
            let b = decode_char(bytes[i + 1])?;
            let triple = (a << 18) | (b << 12);
            result.push(((triple >> 16) & 0xFF) as u8);
            i += 2;
        } else {
            break;
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_roundtrip() {
        let data = b"hello world, this is a test of base64!";
        let encoded = base64_encode(data);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn base64_empty() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_decode("").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn base64_padding() {
        assert_eq!(base64_encode(b"a"), "YQ==");
        assert_eq!(base64_encode(b"ab"), "YWI=");
        assert_eq!(base64_encode(b"abc"), "YWJj");
        assert_eq!(base64_decode("YQ==").unwrap(), b"a");
        assert_eq!(base64_decode("YWI=").unwrap(), b"ab");
        assert_eq!(base64_decode("YWJj").unwrap(), b"abc");
    }

    #[test]
    fn base64_binary_data() {
        let data: Vec<u8> = (0..=255).collect();
        let encoded = base64_encode(&data);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn peer_client_config_default() {
        let cfg = PeerClientConfig::default();
        assert_eq!(cfg.connect_timeout_secs, 10);
        assert_eq!(cfg.request_timeout_secs, 30);
        assert_eq!(cfg.max_retries, 5);
        assert_eq!(cfg.retry_base_secs, 2);
    }

    #[test]
    fn share_request_serialization() {
        let req = ShareRequest {
            ceremony_hash: hex::encode([0xAB; 32]),
            requester_pk: hex::encode([0xCD; 32]),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("ceremony_hash"));
        assert!(json.contains("requester_pk"));
    }
}
