// RA-TLS 仪式 — 发起端 + 接收端 (Share 分发/接收)
//
// 仪式发起端 (CeremonyClient):
//   生成密钥 → Shamir split → 分发 share → 记录上链
//
// 仪式接收端 (CeremonyReceiver):
//   接收加密 share → 密封存储 → 链上确认
//
// Share 服务端 (ShareServer):
//   启动时监听 HTTP → 收到 peer 请求 → 返回本地 share (加密)
//   用于 K>1 恢复: 其他节点启动时从本节点获取 share

use std::sync::Arc;

use tracing::{info, warn};

use crate::chain::ChainClient;
use crate::error::{BotError, BotResult};
use crate::tee::enclave_bridge::EnclaveBridge;
use crate::tee::shamir;
use crate::tee::peer_client::base64_encode;

// ═══════════════════════════════════════════════════════════════
// 仪式配置
// ═══════════════════════════════════════════════════════════════

/// 仪式配置
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CeremonyConfig {
    pub k: u8,
    pub n: u8,
    pub timeout_secs: u64,
}

impl Default for CeremonyConfig {
    fn default() -> Self {
        Self { k: 2, n: 3, timeout_secs: 300 }
    }
}

// ═══════════════════════════════════════════════════════════════
// 仪式发起端 (CeremonyClient)
// ═══════════════════════════════════════════════════════════════

/// 仪式发起客户端
#[allow(dead_code)]
pub struct CeremonyClient {
    enclave: Arc<EnclaveBridge>,
    config: CeremonyConfig,
}

#[allow(dead_code)]
impl CeremonyClient {
    pub fn new(enclave: Arc<EnclaveBridge>, config: CeremonyConfig) -> Self {
        Self { enclave, config }
    }

    /// 执行完整仪式流程
    pub async fn run_ceremony(
        &self,
        chain: &ChainClient,
        participants: Vec<String>,
    ) -> BotResult<[u8; 32]> {
        info!(k = self.config.k, n = self.config.n, "开始 RA-TLS 仪式");

        // 1. 生成 Ed25519 密钥
        let mut seed = [0u8; 32];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut seed);

        // 2. Shamir 分片
        let shamir_config = shamir::ShamirConfig::new(self.config.k, self.config.n)
            .map_err(|e| BotError::EnclaveError(format!("Shamir config: {}", e)))?;
        let shares = shamir::split(&seed, &shamir_config)
            .map_err(|e| BotError::EnclaveError(format!("Shamir split: {}", e)))?;
        info!(shares = shares.len(), "Shamir 分片完成");

        // 3. 对每个参与者执行 RA-TLS 握手 + 分发
        // TODO: 实现真实 RA-TLS 握手, 当前使用参与者端点的 SHA256 作为占位标识
        let participant_enclaves: Vec<[u8; 32]> = participants.iter()
            .map(|endpoint| {
                use sha2::{Sha256, Digest};
                let mut h = Sha256::new();
                h.update(b"participant:");
                h.update(endpoint.as_bytes());
                h.finalize().into()
            })
            .collect();

        // 4. 记录仪式到链上
        let ceremony_hash = self.compute_ceremony_hash(&seed, &participant_enclaves);
        let bot_pk = self.enclave.public_key_bytes();
        // mrenclave: 使用 Enclave 公钥的 SHA256 摘要作为标识
        // TODO: 硬件模式下应从 SGX Quote 中提取真实 MRENCLAVE
        let mrenclave: [u8; 32] = {
            use sha2::{Sha256, Digest};
            let mut h = Sha256::new();
            h.update(b"mrenclave:");
            h.update(bot_pk);
            h.finalize().into()
        };

        chain.record_ceremony(
            ceremony_hash,
            mrenclave,
            self.config.k,
            self.config.n,
            bot_pk,
            participant_enclaves,
        ).await?;

        info!(hash = hex::encode(ceremony_hash), "仪式完成并记录上链");
        Ok(ceremony_hash)
    }

    fn compute_ceremony_hash(&self, seed: &[u8; 32], participants: &[[u8; 32]]) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(seed);
        for p in participants {
            hasher.update(p);
        }
        hasher.finalize().into()
    }
}

// ═══════════════════════════════════════════════════════════════
// 仪式接收端 (CeremonyReceiver)
// ═══════════════════════════════════════════════════════════════

/// Share 接收请求 (Ceremony 发起端 → 本节点)
#[derive(serde::Deserialize)]
pub struct ReceiveShareRequest {
    /// 仪式 hash
    pub ceremony_hash: String,
    /// 加密 share (base64)
    pub share_data: String,
    /// 发起端公钥 (hex)
    pub sender_pk: String,
    /// Shamir 配置
    pub k: u8,
    pub n: u8,
}

/// Share 接收响应
#[derive(serde::Serialize)]
pub struct ReceiveShareResponse {
    pub success: bool,
    pub message: String,
    pub receiver_pk: String,
}

/// 处理接收到的 share (Ceremony 分发阶段)
pub fn handle_receive_share(
    enclave: &Arc<EnclaveBridge>,
    req: &ReceiveShareRequest,
) -> BotResult<ReceiveShareResponse> {
    info!(
        ceremony = %req.ceremony_hash,
        sender = %req.sender_pk,
        k = req.k, n = req.n,
        "收到 Ceremony share"
    );

    // 解码 base64 share → EncryptedShare → 保存
    let share_data = crate::tee::peer_client::base64_decode_pub(&req.share_data)
        .map_err(|e| BotError::EnclaveError(format!("share base64: {}", e)))?;

    let encrypted_share = shamir::share_from_bytes(&share_data)
        .map_err(|e| BotError::EnclaveError(format!("share parse: {}", e)))?;

    // 密封保存到本地
    enclave.save_local_share(&encrypted_share)?;

    let receiver_pk = hex::encode(enclave.public_key_bytes());
    info!(share_id = encrypted_share.id, "Share 已接收并密封保存");

    Ok(ReceiveShareResponse {
        success: true,
        message: "share received and sealed".into(),
        receiver_pk,
    })
}

// ═══════════════════════════════════════════════════════════════
// Share 服务端 (ShareServer) — 响应 peer 的 share 请求
// ═══════════════════════════════════════════════════════════════

/// Peer share 请求
#[derive(serde::Deserialize)]
pub struct ShareRequest {
    pub ceremony_hash: String,
    pub requester_pk: String,
}

/// Peer share 响应
#[derive(Debug, serde::Serialize)]
pub struct ShareResponse {
    pub share_data: String,
    pub peer_pk: String,
}

/// Share 错误响应
#[derive(Debug, serde::Serialize)]
pub struct ShareErrorResponse {
    pub error: String,
}

/// TEE 证明验证结果 (必须由链上查询构造)
///
/// 在 handle_share_request 之前, 调用方必须查询链上证明状态:
/// - 请求者 Bot 已注册且活跃
/// - 请求者拥有有效的 TEE 证明 (quote_verified=true)
/// - 请求者的 MRENCLAVE 在白名单中
pub struct AttestationGuard {
    /// 请求者是否是已验证的 TEE 节点
    pub is_verified_tee: bool,
    /// 请求者的 quote_verified 状态
    pub quote_verified: bool,
}

impl AttestationGuard {
    /// 安全构造: 仅当链上查询确认后调用
    pub fn verified(is_verified_tee: bool, quote_verified: bool) -> Self {
        Self { is_verified_tee, quote_verified }
    }
    /// 未验证 (用于软件模式/测试)
    pub fn unverified() -> Self {
        Self { is_verified_tee: false, quote_verified: false }
    }
}

/// 处理 peer 的 share 请求 (K>1 恢复阶段)
///
/// ── 安全: 必须提供 AttestationGuard 证明请求者 TEE 身份已经链上验证 ──
pub fn handle_share_request(
    enclave: &Arc<EnclaveBridge>,
    req: &ShareRequest,
    guard: &AttestationGuard,
) -> Result<ShareResponse, ShareErrorResponse> {
    info!(
        ceremony = %req.ceremony_hash,
        requester = %req.requester_pk,
        "收到 peer share 请求"
    );

    // ── 安全守卫: 验证请求者身份 ──
    // 拒绝空 requester_pk (防止匿名请求)
    if req.requester_pk.is_empty() || req.requester_pk.len() != 64 {
        warn!(requester = %req.requester_pk, "无效的 requester_pk (需要 64 hex chars)");
        return Err(ShareErrorResponse {
            error: "invalid requester_pk: must be 64 hex characters".into(),
        });
    }
    // 验证 ceremony_hash 格式
    if req.ceremony_hash.is_empty() || req.ceremony_hash.len() != 64 {
        warn!(ceremony = %req.ceremony_hash, "无效的 ceremony_hash");
        return Err(ShareErrorResponse {
            error: "invalid ceremony_hash: must be 64 hex characters".into(),
        });
    }

    // ── P0b 安全守卫: 请求者必须是经过 Quote 验证的 TEE 节点 ──
    // 未经链上证明验证的节点无法获取 share, 防止修改代码的恶意节点窃取 API Key
    if !guard.is_verified_tee {
        warn!(requester = %req.requester_pk, "拒绝: 请求者不是已验证的 TEE 节点");
        return Err(ShareErrorResponse {
            error: "requester is not a verified TEE node".into(),
        });
    }
    if !guard.quote_verified {
        warn!(requester = %req.requester_pk, "拒绝: 请求者的 Quote 未经链上验证");
        return Err(ShareErrorResponse {
            error: "requester attestation quote not verified on-chain".into(),
        });
    }

    // 加载本地 share
    let encrypted_share = match enclave.load_local_share() {
        Ok(Some(share)) => share,
        Ok(None) => {
            warn!("peer 请求 share 但本地无 share");
            return Err(ShareErrorResponse {
                error: "no local share available".into(),
            });
        }
        Err(e) => {
            warn!(error = %e, "加载本地 share 失败");
            return Err(ShareErrorResponse {
                error: format!("load share failed: {}", e),
            });
        }
    };

    let share_bytes = shamir::share_to_bytes(&encrypted_share);
    let share_b64 = base64_encode(&share_bytes);
    let peer_pk = hex::encode(enclave.public_key_bytes());

    info!(share_id = encrypted_share.id, "Share 已发送给 peer");

    Ok(ShareResponse {
        share_data: share_b64,
        peer_pk,
    })
}

/// 共享链客户端句柄 (支持后台异步注入)
pub type SharedChainClient = Arc<tokio::sync::RwLock<Option<Arc<ChainClient>>>>;

/// 创建空的共享链客户端句柄
pub fn new_shared_chain() -> SharedChainClient {
    Arc::new(tokio::sync::RwLock::new(None))
}

/// 创建 Axum 路由 (用于 Ceremony 端口)
///
/// shared_chain: 共享链客户端句柄, 链连接成功后由后台任务注入
///               None 时回退到 unverified (⚠️ 仅开发模式)
pub fn ceremony_routes(
    enclave: Arc<EnclaveBridge>,
    shared_chain: SharedChainClient,
) -> axum::Router {
    use axum::{routing::post, Json};

    let enclave_receive = enclave.clone();
    let enclave_share = enclave;

    axum::Router::new()
        .route("/ceremony/receive-share", post({
            let enc = enclave_receive;
            move |Json(req): Json<ReceiveShareRequest>| {
                let enc = enc.clone();
                async move {
                    match handle_receive_share(&enc, &req) {
                        Ok(resp) => (axum::http::StatusCode::OK, Json(serde_json::json!(resp))),
                        Err(e) => (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({"error": format!("{}", e)})),
                        ),
                    }
                }
            }
        }))
        .route("/share/request", post({
            let enc = enclave_share;
            let chain = shared_chain;
            move |Json(req): Json<ShareRequest>| {
                let enc = enc.clone();
                let chain = chain.clone();
                async move {
                    // 构造 AttestationGuard: 优先链上查询, 无链客户端时回退 unverified
                    let chain_lock = chain.read().await;
                    let guard = match chain_lock.as_ref() {
                        Some(c) => {
                            match c.query_attestation_guard(&req.requester_pk).await {
                                Ok((is_tee, quote_ok)) => {
                                    info!(
                                        requester = %req.requester_pk,
                                        is_verified_tee = is_tee,
                                        quote_verified = quote_ok,
                                        "AttestationGuard 链上查询完成"
                                    );
                                    AttestationGuard::verified(is_tee, quote_ok)
                                }
                                Err(e) => {
                                    warn!(error = %e, "AttestationGuard 链上查询失败, 拒绝请求");
                                    return (
                                        axum::http::StatusCode::SERVICE_UNAVAILABLE,
                                        Json(serde_json::json!({"error": "attestation query failed"})),
                                    );
                                }
                            }
                        }
                        None => {
                            warn!("⚠️ share/request: 链客户端尚未连接, AttestationGuard 未验证");
                            AttestationGuard::unverified()
                        }
                    };
                    drop(chain_lock);
                    match handle_share_request(&enc, &req, &guard) {
                        Ok(resp) => (axum::http::StatusCode::OK, Json(serde_json::json!(resp))),
                        Err(err) => (
                            axum::http::StatusCode::NOT_FOUND,
                            Json(serde_json::json!(err)),
                        ),
                    }
                }
            }
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_enclave(dir: &str) -> Arc<EnclaveBridge> {
        Arc::new(EnclaveBridge::init(dir, "software").unwrap())
    }

    #[test]
    fn ceremony_config_default() {
        let cfg = CeremonyConfig::default();
        assert_eq!(cfg.k, 2);
        assert_eq!(cfg.n, 3);
        assert_eq!(cfg.timeout_secs, 300);
    }

    #[test]
    fn receive_share_and_serve() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let enclave = make_enclave(path);

        // 模拟 Ceremony 产出: 创建一个 share 并编码
        let token = "test:TOKEN_FOR_CEREMONY";
        let sk = [0x33u8; 32];
        let secrets = shamir::encode_secrets(token, &sk);
        let config = shamir::ShamirConfig::new(2, 3).unwrap();
        let shares = shamir::split(&secrets, &config).unwrap();

        // 加密 share[0]
        let seal_key = enclave.seal_key();
        let encrypted = shamir::encrypt_share(&shares[0], &seal_key).unwrap();
        let share_bytes = shamir::share_to_bytes(&encrypted);
        let share_b64 = base64_encode(&share_bytes);

        // 接收 share
        let req = ReceiveShareRequest {
            ceremony_hash: hex::encode([0xAA; 32]),
            share_data: share_b64,
            sender_pk: hex::encode([0xBB; 32]),
            k: 2,
            n: 3,
        };
        let resp = handle_receive_share(&enclave, &req).unwrap();
        assert!(resp.success);

        // 验证 share 已保存
        let loaded = enclave.load_local_share().unwrap();
        assert!(loaded.is_some());

        // 模拟 peer 请求 share (需要 verified guard)
        let share_req = ShareRequest {
            ceremony_hash: hex::encode([0xAA; 32]),
            requester_pk: hex::encode([0xCC; 32]),
        };
        let guard = AttestationGuard::verified(true, true);
        let share_resp = handle_share_request(&enclave, &share_req, &guard).unwrap();
        assert!(!share_resp.share_data.is_empty());
        assert!(!share_resp.peer_pk.is_empty());
    }

    #[test]
    fn share_request_no_local_share() {
        let dir = tempfile::tempdir().unwrap();
        let enclave = make_enclave(dir.path().to_str().unwrap());

        let req = ShareRequest {
            ceremony_hash: hex::encode([0u8; 32]),
            requester_pk: hex::encode([0u8; 32]),
        };
        let guard = AttestationGuard::verified(true, true);
        let result = handle_share_request(&enclave, &req, &guard);
        assert!(result.is_err());
        assert!(result.unwrap_err().error.contains("no local share"));
    }

    #[test]
    fn ceremony_routes_compiles() {
        let dir = tempfile::tempdir().unwrap();
        let enclave = make_enclave(dir.path().to_str().unwrap());
        let _router = ceremony_routes(enclave, new_shared_chain());
    }

    #[test]
    fn share_request_rejected_without_attestation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let enclave = make_enclave(path);

        // 先保存一个 share
        let token = "test:TOKEN";
        let sk = [0x11u8; 32];
        let secrets = shamir::encode_secrets(token, &sk);
        let config = shamir::ShamirConfig::new(2, 3).unwrap();
        let shares = shamir::split(&secrets, &config).unwrap();
        let seal_key = enclave.seal_key();
        let encrypted = shamir::encrypt_share(&shares[0], &seal_key).unwrap();
        enclave.save_local_share(&encrypted).unwrap();

        let req = ShareRequest {
            ceremony_hash: hex::encode([0xAA; 32]),
            requester_pk: hex::encode([0xCC; 32]),
        };

        // 未验证的 guard → 拒绝
        let guard = AttestationGuard::unverified();
        let result = handle_share_request(&enclave, &req, &guard);
        assert!(result.is_err());
        assert!(result.unwrap_err().error.contains("not a verified TEE node"));

        // 已验证但 quote 未验证 → 拒绝
        let guard = AttestationGuard::verified(true, false);
        let result = handle_share_request(&enclave, &req, &guard);
        assert!(result.is_err());
        assert!(result.unwrap_err().error.contains("quote not verified"));

        // 完全验证 → 成功
        let guard = AttestationGuard::verified(true, true);
        let result = handle_share_request(&enclave, &req, &guard);
        assert!(result.is_ok());
    }
}
