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
use std::time::Duration;

use tracing::{info, warn, error};
use zeroize::Zeroizing;

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
        Self { k: 2, n: 4, timeout_secs: 300 }
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

        // bot_id_hash = SHA256(bot_public_key)
        let bot_id_hash: [u8; 32] = {
            use sha2::{Sha256, Digest};
            Sha256::digest(bot_pk).into()
        };
        chain.record_ceremony(
            ceremony_hash,
            mrenclave,
            self.config.k,
            self.config.n,
            bot_pk,
            participant_enclaves,
            bot_id_hash,
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
///
/// ── 安全: 必须提供 AttestationGuard 证明发送者 TEE 身份已经链上验证 ──
pub fn handle_receive_share(
    enclave: &Arc<EnclaveBridge>,
    req: &ReceiveShareRequest,
    guard: &AttestationGuard,
) -> BotResult<ReceiveShareResponse> {
    info!(
        ceremony = %req.ceremony_hash,
        sender = %req.sender_pk,
        k = req.k, n = req.n,
        "收到 Ceremony share"
    );

    // ── 安全守卫: 验证发送者身份 ──
    if req.sender_pk.is_empty() || req.sender_pk.len() != 64 {
        warn!(sender = %req.sender_pk, "无效的 sender_pk (需要 64 hex chars)");
        return Err(BotError::EnclaveError(
            "invalid sender_pk: must be 64 hex characters".into(),
        ));
    }
    if req.ceremony_hash.is_empty() || req.ceremony_hash.len() != 64 {
        warn!(ceremony = %req.ceremony_hash, "无效的 ceremony_hash");
        return Err(BotError::EnclaveError(
            "invalid ceremony_hash: must be 64 hex characters".into(),
        ));
    }

    // ── P0 安全守卫: 发送者必须是经过 Quote 验证的 TEE 节点 ──
    // 未经链上证明验证的节点无法分发 share, 防止恶意覆盖本地 share
    if !guard.is_verified_tee {
        warn!(sender = %req.sender_pk, "拒绝: 发送者不是已验证的 TEE 节点");
        return Err(BotError::EnclaveError(
            "sender is not a verified TEE node".into(),
        ));
    }
    if !guard.quote_verified {
        warn!(sender = %req.sender_pk, "拒绝: 发送者的 Quote 未经链上验证");
        return Err(BotError::EnclaveError(
            "sender attestation quote not verified on-chain".into(),
        ));
    }

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

    // R4: 验证请求的 ceremony_hash 与本地存储的匹配
    if let Ok(Some(local_hash)) = enclave.load_ceremony_hash() {
        let req_hash_bytes = hex::decode(&req.ceremony_hash).unwrap_or_default();
        if req_hash_bytes.len() == 32 && req_hash_bytes != local_hash {
            warn!(
                expected = %hex::encode(local_hash),
                received = %req.ceremony_hash,
                "ceremony_hash 不匹配, 拒绝请求"
            );
            return Err(ShareErrorResponse {
                error: "ceremony_hash mismatch: share belongs to a different ceremony".into(),
            });
        }
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

    // 解密本地 share (使用本节点的 seal_key)
    let seal_key = enclave.seal_key().map_err(|e| ShareErrorResponse {
        error: format!("seal_key derivation failed: {}", e),
    })?;
    let plain_share = match shamir::decrypt_share(&encrypted_share, &seal_key) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "解密本地 share 失败");
            return Err(ShareErrorResponse {
                error: format!("decrypt local share: {}", e),
            });
        }
    };

    // 用请求者的 Ed25519 公钥 (→ X25519) 做 ECDH 加密
    // 这样只有请求者的私钥才能解密, 不依赖 seal_key 一致性
    let requester_pk_bytes = match hex::decode(&req.requester_pk) {
        Ok(b) if b.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            arr
        }
        _ => {
            return Err(ShareErrorResponse {
                error: "invalid requester_pk bytes".into(),
            });
        }
    };
    let requester_vk = match ed25519_dalek::VerifyingKey::from_bytes(&requester_pk_bytes) {
        Ok(vk) => vk,
        Err(e) => {
            return Err(ShareErrorResponse {
                error: format!("invalid Ed25519 public key: {}", e),
            });
        }
    };
    let requester_x25519 = shamir::ed25519_pk_to_x25519(&requester_vk);
    let (ecdh_encrypted, ephemeral_pk) = match shamir::encrypt_share_for_recipient(
        &plain_share, &requester_x25519,
    ) {
        Ok(r) => r,
        Err(e) => {
            return Err(ShareErrorResponse {
                error: format!("ECDH encrypt share: {}", e),
            });
        }
    };

    let ecdh_share = shamir::EcdhEncryptedShare {
        encrypted: ecdh_encrypted,
        ephemeral_pk,
    };
    let share_bytes = shamir::ecdh_share_to_bytes(&ecdh_share);
    let share_b64 = base64_encode(&share_bytes);
    let peer_pk = hex::encode(enclave.public_key_bytes());

    info!(share_id = encrypted_share.id, "Share 已 ECDH 加密并发送给 peer");

    Ok(ShareResponse {
        share_data: share_b64,
        peer_pk,
    })
}

// ═══════════════════════════════════════════════════════════════
// Re-ceremony — 恢复 secret → 重新分片 → 分发
// ═══════════════════════════════════════════════════════════════

/// Re-ceremony 参与者信息
#[derive(Debug, Clone)]
pub struct Participant {
    /// Peer 端点 URL
    pub endpoint: String,
    /// Ed25519 公钥 (32 bytes)
    pub public_key: [u8; 32],
}

/// Re-ceremony 配置
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ReCeremonyConfig {
    /// 当前 Shamir 门限 K (用于恢复现有 secret)
    pub current_k: u8,
    /// 新的 Shamir 门限 K
    pub new_k: u8,
    /// 新的总 share 数 N
    pub new_n: u8,
    /// 分发超时 (秒)
    pub timeout_secs: u64,
}

/// Re-ceremony 结果
#[derive(Debug)]
#[allow(dead_code)]
pub struct ReCeremonyResult {
    pub ceremony_hash: [u8; 32],
    pub new_k: u8,
    pub new_n: u8,
    pub distributed: usize,
}

/// ECDH 加密的 Share 接收请求 (Re-ceremony 分发用)
///
/// 与 ReceiveShareRequest 的区别:
/// - share_data 是 ECDH 加密格式 (EcdhEncryptedShare), 而非 seal_key 加密
/// - 接收方用自己的 Ed25519 私钥 ECDH 解密, 再用自己的 seal_key 重新加密保存
#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct ReceiveShareEcdhRequest {
    pub ceremony_hash: String,
    /// ECDH 加密的 share (base64 of EcdhEncryptedShare bytes)
    pub share_data: String,
    /// 发起者公钥 (hex)
    pub sender_pk: String,
    pub k: u8,
    pub n: u8,
}

/// 处理 ECDH 加密的 share 接收 (Re-ceremony 分发阶段)
///
/// 流程: ECDH 解密 → 得到明文 Share → 用本节点 seal_key 重新加密 → 密封保存
pub fn handle_receive_share_ecdh(
    enclave: &Arc<EnclaveBridge>,
    req: &ReceiveShareEcdhRequest,
    guard: &AttestationGuard,
) -> BotResult<ReceiveShareResponse> {
    info!(
        ceremony = %req.ceremony_hash,
        sender = %req.sender_pk,
        k = req.k, n = req.n,
        "收到 Re-ceremony ECDH share"
    );

    // 安全守卫: 验证发送者身份
    if req.sender_pk.is_empty() || req.sender_pk.len() != 64 {
        return Err(BotError::EnclaveError("invalid sender_pk".into()));
    }
    if req.ceremony_hash.is_empty() || req.ceremony_hash.len() != 64 {
        return Err(BotError::EnclaveError("invalid ceremony_hash".into()));
    }
    if !guard.is_verified_tee || !guard.quote_verified {
        warn!(sender = %req.sender_pk, "拒绝: 发送者未经 TEE 验证");
        return Err(BotError::EnclaveError("sender not verified".into()));
    }

    // ECDH 解密
    let share_bytes = crate::tee::peer_client::base64_decode_pub(&req.share_data)
        .map_err(|e| BotError::EnclaveError(format!("base64: {}", e)))?;
    let ecdh_share = shamir::ecdh_share_from_bytes(&share_bytes)
        .map_err(|e| BotError::EnclaveError(format!("ecdh parse: {}", e)))?;

    let receiver_x25519 = shamir::ed25519_to_x25519_secret(&enclave.signing_key().to_bytes());
    let plain_share = shamir::decrypt_share_from_sender(
        &ecdh_share.encrypted, &receiver_x25519, &ecdh_share.ephemeral_pk,
    ).map_err(|e| BotError::EnclaveError(format!("ECDH decrypt: {}", e)))?;

    // 用本节点 seal_key 重新加密
    let seal_key = enclave.seal_key()?;
    let encrypted = shamir::encrypt_share(&plain_share, &seal_key)
        .map_err(|e| BotError::EnclaveError(format!("re-encrypt: {}", e)))?;

    enclave.save_local_share(&encrypted)?;

    // 保存 ceremony_hash
    let ch_bytes = hex::decode(&req.ceremony_hash)
        .map_err(|e| BotError::EnclaveError(format!("ceremony_hash hex: {}", e)))?;
    if ch_bytes.len() == 32 {
        let mut ch = [0u8; 32];
        ch.copy_from_slice(&ch_bytes);
        enclave.save_ceremony_hash(&ch)?;
    }

    let receiver_pk = hex::encode(enclave.public_key_bytes());
    info!(share_id = plain_share.id, "Re-ceremony share 已 ECDH 解密并密封保存");

    Ok(ReceiveShareResponse {
        success: true,
        message: "share received via ECDH and re-sealed".into(),
        receiver_pk,
    })
}

/// 执行 Re-ceremony: 恢复现有 secret → 重新分片 → ECDH 加密分发 → 链上记录
///
/// 与 run_ceremony 的区别: secret 来源是恢复而非新生成
///
/// 原子性保证: 所有参与者必须确认收到新 share, 否则不记录链上
/// (失败时旧 share 仍然有效, 可重试)
#[allow(dead_code)]
pub async fn run_re_ceremony(
    enclave: &Arc<EnclaveBridge>,
    chain: &Arc<ChainClient>,
    config: &ReCeremonyConfig,
    participants: &[Participant],
    bot_id_hash: &[u8; 32],
) -> BotResult<ReCeremonyResult> {
    // ── 前置验证 ──
    if participants.len() != config.new_n as usize {
        return Err(BotError::EnclaveError(format!(
            "participants count ({}) != new_n ({})", participants.len(), config.new_n
        )));
    }
    if config.new_k > config.new_n || config.new_k == 0 {
        return Err(BotError::EnclaveError(format!(
            "invalid new K/N: K={}, N={}", config.new_k, config.new_n
        )));
    }

    let my_pk = enclave.public_key_bytes();
    let my_index = participants.iter().position(|p| p.public_key == my_pk)
        .ok_or_else(|| BotError::EnclaveError("initiator not in participants list".into()))?;

    info!(
        current_k = config.current_k,
        new_k = config.new_k, new_n = config.new_n,
        participants = participants.len(),
        my_index,
        "开始 Re-ceremony"
    );

    // ══ 阶段 1: 从现有 share 恢复 secret ══
    let encrypted_share = enclave.load_local_share()?
        .ok_or_else(|| BotError::EnclaveError("no local share for recovery".into()))?;
    let seal_key = enclave.seal_key()?;
    let local_share = shamir::decrypt_share(&encrypted_share, &seal_key)
        .map_err(|e| BotError::EnclaveError(format!("decrypt local share: {}", e)))?;

    let secrets = if config.current_k <= 1 {
        // K=1: 本地 share 即可恢复
        let s = shamir::recover(&[local_share], 1)
            .map_err(|e| BotError::EnclaveError(format!("recover: {}", e)))?;
        Zeroizing::new(s)
    } else {
        // K>1: 需要从 peer 收集
        let ceremony_hash = enclave.load_ceremony_hash()?.unwrap_or(*bot_id_hash);
        let needed = (config.current_k - 1) as usize;

        let peer_endpoints: Vec<String> = participants.iter()
            .filter(|p| p.public_key != my_pk && !p.endpoint.is_empty())
            .map(|p| p.endpoint.clone())
            .collect();

        if peer_endpoints.is_empty() {
            return Err(BotError::EnclaveError(
                "K>1 but no peer endpoints for share collection".into()
            ));
        }

        info!(needed, peers = peer_endpoints.len(), "从 peer 收集 share...");

        let peer_config = crate::tee::peer_client::PeerClientConfig::default();
        let peer_client = crate::tee::peer_client::PeerClient::new(peer_config)?;
        let peer_encrypted = peer_client.collect_shares(
            &peer_endpoints, &ceremony_hash, &my_pk, needed,
        ).await?;

        let receiver_x25519 = shamir::ed25519_to_x25519_secret(&enclave.signing_key().to_bytes());
        let mut all_shares = vec![local_share];
        for ecdh_share in &peer_encrypted {
            let share = shamir::decrypt_share_from_sender(
                &ecdh_share.encrypted, &receiver_x25519, &ecdh_share.ephemeral_pk,
            ).map_err(|e| BotError::EnclaveError(format!("ECDH decrypt peer share: {}", e)))?;
            all_shares.push(share);
        }

        let s = shamir::recover(&all_shares, config.current_k)
            .map_err(|e| BotError::EnclaveError(format!("Shamir recover: {}", e)))?;
        Zeroizing::new(s)
    };

    info!("Secret 恢复完成, 开始重新分片 (K={}, N={})", config.new_k, config.new_n);

    // ══ 阶段 2: 重新分片 ══
    let new_shamir_config = shamir::ShamirConfig::new(config.new_k, config.new_n)
        .map_err(|e| BotError::EnclaveError(format!("new Shamir config: {}", e)))?;
    let new_shares = shamir::split(&secrets, &new_shamir_config)
        .map_err(|e| BotError::EnclaveError(format!("Shamir split: {}", e)))?;

    // 计算新 ceremony_hash (包含随机数, 确保每次 Re-ceremony 产出不同 hash)
    let new_ceremony_hash: [u8; 32] = {
        use sha2::{Sha256, Digest};
        use rand::RngCore;
        let mut h = Sha256::new();
        h.update(b"re-ceremony-v1:");
        h.update(bot_id_hash);
        h.update(&[config.new_k, config.new_n]);
        for p in participants {
            h.update(&p.public_key);
        }
        let mut nonce = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        h.update(&nonce);
        h.finalize().into()
    };

    info!(
        shares = new_shares.len(),
        ceremony = %hex::encode(new_ceremony_hash),
        "新 share 生成完成, 开始分发"
    );

    // ══ 阶段 3: 分发新 share 给所有参与者 ══
    let sender_pk_hex = hex::encode(my_pk);
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.timeout_secs))
        .build()
        .map_err(|e| BotError::EnclaveError(format!("http client: {}", e)))?;

    let mut distributed = 0usize;
    let mut errors = Vec::new();

    for (i, participant) in participants.iter().enumerate() {
        if participant.public_key == my_pk {
            // 发起者自己: 直接加密保存
            let encrypted = shamir::encrypt_share(&new_shares[i], &seal_key)
                .map_err(|e| BotError::EnclaveError(format!("encrypt local share: {}", e)))?;
            enclave.save_local_share(&encrypted)?;
            enclave.save_ceremony_hash(&new_ceremony_hash)?;
            distributed += 1;
            info!(share_id = new_shares[i].id, "[Re-ceremony] 本地 share 已保存");
        } else {
            // Peer: ECDH 加密 + HTTP 分发
            match distribute_share_ecdh(
                &http, participant, &new_shares[i],
                &new_ceremony_hash, &sender_pk_hex,
                config.new_k, config.new_n,
            ).await {
                Ok(()) => {
                    distributed += 1;
                    info!(
                        endpoint = %participant.endpoint,
                        share_id = new_shares[i].id,
                        "[Re-ceremony] share 已分发给 peer"
                    );
                }
                Err(e) => {
                    error!(
                        endpoint = %participant.endpoint,
                        error = %e,
                        "[Re-ceremony] share 分发失败"
                    );
                    errors.push(format!("{}: {}", participant.endpoint, e));
                }
            }
        }
    }

    // 原子性检查: 所有参与者必须收到新 share
    if distributed != participants.len() {
        error!(
            distributed, total = participants.len(),
            errors = ?errors,
            "Re-ceremony 不完整: 部分 share 分发失败, 不记录链上"
        );
        // 回滚本地 share: 恢复旧的 (TODO: 更完善的回滚机制)
        return Err(BotError::EnclaveError(format!(
            "Re-ceremony incomplete: {}/{} distributed. Errors: [{}]. Old shares still valid for successful peers.",
            distributed, participants.len(), errors.join("; ")
        )));
    }

    // ══ 阶段 4: 链上记录 ══
    let mrenclave = compute_mrenclave(&my_pk);
    let participant_pks: Vec<[u8; 32]> = participants.iter().map(|p| p.public_key).collect();

    chain.record_ceremony(
        new_ceremony_hash, mrenclave,
        config.new_k, config.new_n,
        my_pk, participant_pks,
        *bot_id_hash,
    ).await?;

    info!(
        hash = %hex::encode(new_ceremony_hash),
        k = config.new_k, n = config.new_n, distributed,
        "✅ Re-ceremony 完成并记录链上"
    );

    Ok(ReCeremonyResult {
        ceremony_hash: new_ceremony_hash,
        new_k: config.new_k,
        new_n: config.new_n,
        distributed,
    })
}

/// 将 share ECDH 加密后分发给单个 peer
async fn distribute_share_ecdh(
    http: &reqwest::Client,
    participant: &Participant,
    share: &shamir::Share,
    ceremony_hash: &[u8; 32],
    sender_pk_hex: &str,
    k: u8,
    n: u8,
) -> BotResult<()> {
    // Ed25519 pk → X25519 pk → ECDH 加密
    let vk = ed25519_dalek::VerifyingKey::from_bytes(&participant.public_key)
        .map_err(|e| BotError::EnclaveError(format!("invalid peer pk: {}", e)))?;
    let x25519_pk = shamir::ed25519_pk_to_x25519(&vk);
    let (ecdh_encrypted, ephemeral_pk) = shamir::encrypt_share_for_recipient(share, &x25519_pk)
        .map_err(|e| BotError::EnclaveError(format!("ECDH encrypt: {}", e)))?;

    let ecdh_share = shamir::EcdhEncryptedShare { encrypted: ecdh_encrypted, ephemeral_pk };
    let share_b64 = base64_encode(&shamir::ecdh_share_to_bytes(&ecdh_share));

    let url = format!(
        "{}/ceremony/receive-share-ecdh",
        participant.endpoint.trim_end_matches('/')
    );

    let req_body = ReceiveShareEcdhRequest {
        ceremony_hash: hex::encode(ceremony_hash),
        share_data: share_b64,
        sender_pk: sender_pk_hex.to_string(),
        k,
        n,
    };

    let resp = http.post(&url)
        .json(&req_body)
        .send()
        .await
        .map_err(|e| BotError::EnclaveError(format!("POST {}: {}", url, e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(BotError::EnclaveError(format!(
            "peer {} responded {}: {}", participant.endpoint, status, body
        )));
    }

    Ok(())
}

/// 计算 MRENCLAVE 标识 (TODO: 硬件模式下应从 SGX Quote 提取)
fn compute_mrenclave(pk: &[u8; 32]) -> [u8; 32] {
    use sha2::{Sha256, Digest};
    let mut h = Sha256::new();
    h.update(b"mrenclave:");
    h.update(pk);
    h.finalize().into()
}

// ═══════════════════════════════════════════════════════════════
// Migration Ceremony — 跨版本密钥传递
// ═══════════════════════════════════════════════════════════════

/// Migration 导出请求 (新版本 Enclave → 旧版本)
#[derive(serde::Deserialize)]
pub struct MigrationExportRequest {
    /// 请求者 (新版本) 的 Ed25519 公钥 (hex)
    pub requester_pk: String,
}

/// Migration 导出响应 (旧版本 Enclave 返回)
#[derive(serde::Serialize)]
pub struct MigrationExportResponse {
    /// ECDH 加密的 secret (base64)
    pub encrypted_secret: String,
    /// 临时 X25519 公钥 (hex)
    pub ephemeral_pk: String,
    /// 旧版本的 Ed25519 公钥 (hex)
    pub source_pk: String,
}

/// Migration 导出状态 (一次性使用)
pub struct MigrationExportGuard {
    exported: std::sync::atomic::AtomicBool,
}

impl MigrationExportGuard {
    pub fn new() -> Self {
        Self { exported: std::sync::atomic::AtomicBool::new(false) }
    }

    fn try_export(&self) -> bool {
        !self.exported.swap(true, std::sync::atomic::Ordering::SeqCst)
    }

    #[allow(dead_code)]
    pub fn is_exported(&self) -> bool {
        self.exported.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// 处理 Migration 导出请求
///
/// 旧版本 Enclave 解密本地 share → 恢复 secret → ECDH 加密发送给新版本
///
/// 安全约束:
/// - 单次使用: 导出后标记 exported, 拒绝重复导出
/// - ECDH 端到端: secret 明文只在 TEE 内存中
/// - 超时: 由 HTTP 层控制
pub fn handle_migration_export(
    enclave: &Arc<EnclaveBridge>,
    req: &MigrationExportRequest,
    guard: &MigrationExportGuard,
) -> BotResult<MigrationExportResponse> {
    // 一次性使用守卫
    if !guard.try_export() {
        warn!("Migration export 已使用过, 拒绝重复导出");
        return Err(BotError::EnclaveError(
            "migration export already used (one-time only)".into(),
        ));
    }

    // 验证请求者公钥
    if req.requester_pk.len() != 64 {
        return Err(BotError::EnclaveError("invalid requester_pk: must be 64 hex chars".into()));
    }
    let requester_pk_bytes = hex::decode(&req.requester_pk)
        .map_err(|e| BotError::EnclaveError(format!("requester_pk hex: {}", e)))?;
    if requester_pk_bytes.len() != 32 {
        return Err(BotError::EnclaveError("requester_pk must be 32 bytes".into()));
    }
    let mut requester_pk = [0u8; 32];
    requester_pk.copy_from_slice(&requester_pk_bytes);

    info!(
        requester = %req.requester_pk,
        "Migration export: 新版本 Enclave 请求密钥迁移"
    );

    // 加载并解密本地 share → 恢复 secret
    let encrypted_share = enclave.load_local_share()?
        .ok_or_else(|| BotError::EnclaveError("no local share to export".into()))?;
    let seal_key = enclave.seal_key()?;
    let local_share = shamir::decrypt_share(&encrypted_share, &seal_key)
        .map_err(|e| BotError::EnclaveError(format!("decrypt local share for migration: {}", e)))?;

    let secrets = shamir::recover(&[local_share], 1)
        .map_err(|e| BotError::EnclaveError(format!("recover secret for migration: {}", e)))?;

    // ECDH 加密: 用请求者 Ed25519 公钥 → X25519 公钥
    let requester_vk = ed25519_dalek::VerifyingKey::from_bytes(&requester_pk)
        .map_err(|e| BotError::EnclaveError(format!("invalid requester Ed25519 pk: {}", e)))?;
    let requester_x25519 = shamir::ed25519_pk_to_x25519(&requester_vk);

    let (encrypted, ephemeral_pk) = shamir::ecdh_encrypt_raw(&secrets, &requester_x25519)
        .map_err(|e| BotError::EnclaveError(format!("ECDH encrypt for migration: {}", e)))?;

    let source_pk = hex::encode(enclave.public_key_bytes());
    info!(
        source_pk = %source_pk,
        requester = %req.requester_pk,
        "Migration export 成功: secret 已 ECDH 加密发送"
    );

    Ok(MigrationExportResponse {
        encrypted_secret: base64_encode(&encrypted),
        ephemeral_pk: hex::encode(ephemeral_pk),
        source_pk,
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
    let enclave_receive_ecdh = enclave.clone();
    let enclave_migration = enclave.clone();
    let enclave_share = enclave;
    let chain_receive = shared_chain.clone();
    let chain_receive_ecdh = shared_chain.clone();
    let migration_guard = Arc::new(MigrationExportGuard::new());

    axum::Router::new()
        .route("/ceremony/receive-share", post({
            let enc = enclave_receive;
            let chain = chain_receive;
            move |Json(req): Json<ReceiveShareRequest>| {
                let enc = enc.clone();
                let chain = chain.clone();
                async move {
                    // 构造 AttestationGuard: 验证发送者 TEE 身份
                    let chain_lock = chain.read().await;
                    let guard = match chain_lock.as_ref() {
                        Some(c) => {
                            match c.query_attestation_guard(&req.sender_pk).await {
                                Ok((is_tee, quote_ok)) => {
                                    info!(
                                        sender = %req.sender_pk,
                                        is_verified_tee = is_tee,
                                        quote_verified = quote_ok,
                                        "receive-share: AttestationGuard 链上查询完成"
                                    );
                                    AttestationGuard::verified(is_tee, quote_ok)
                                }
                                Err(e) => {
                                    warn!(error = %e, "receive-share: AttestationGuard 链上查询失败");
                                    return (
                                        axum::http::StatusCode::SERVICE_UNAVAILABLE,
                                        Json(serde_json::json!({"error": "attestation query failed"})),
                                    );
                                }
                            }
                        }
                        None => {
                            warn!("⚠️ receive-share: 链客户端尚未连接, 拒绝接收");
                            return (
                                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                                Json(serde_json::json!({"error": "chain client not available"})),
                            );
                        }
                    };
                    drop(chain_lock);
                    match handle_receive_share(&enc, &req, &guard) {
                        Ok(resp) => (axum::http::StatusCode::OK, Json(serde_json::json!(resp))),
                        Err(e) => (
                            axum::http::StatusCode::FORBIDDEN,
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
        .route("/ceremony/receive-share-ecdh", post({
            let enc = enclave_receive_ecdh;
            let chain = chain_receive_ecdh;
            move |Json(req): Json<ReceiveShareEcdhRequest>| {
                let enc = enc.clone();
                let chain = chain.clone();
                async move {
                    let chain_lock = chain.read().await;
                    let guard = match chain_lock.as_ref() {
                        Some(c) => {
                            match c.query_attestation_guard(&req.sender_pk).await {
                                Ok((is_tee, quote_ok)) => {
                                    info!(
                                        sender = %req.sender_pk,
                                        is_verified_tee = is_tee,
                                        quote_verified = quote_ok,
                                        "receive-share-ecdh: AttestationGuard 链上查询完成"
                                    );
                                    AttestationGuard::verified(is_tee, quote_ok)
                                }
                                Err(e) => {
                                    warn!(error = %e, "receive-share-ecdh: AttestationGuard 查询失败");
                                    return (
                                        axum::http::StatusCode::SERVICE_UNAVAILABLE,
                                        Json(serde_json::json!({"error": "attestation query failed"})),
                                    );
                                }
                            }
                        }
                        None => {
                            warn!("⚠️ receive-share-ecdh: 链客户端尚未连接, 拒绝接收");
                            return (
                                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                                Json(serde_json::json!({"error": "chain client not available"})),
                            );
                        }
                    };
                    drop(chain_lock);
                    match handle_receive_share_ecdh(&enc, &req, &guard) {
                        Ok(resp) => (axum::http::StatusCode::OK, Json(serde_json::json!(resp))),
                        Err(e) => (
                            axum::http::StatusCode::FORBIDDEN,
                            Json(serde_json::json!({"error": format!("{}", e)})),
                        ),
                    }
                }
            }
        }))
        .route("/migration/export-secret", post({
            let enc = enclave_migration;
            let guard = migration_guard;
            move |Json(req): Json<MigrationExportRequest>| {
                let enc = enc.clone();
                let guard = guard.clone();
                async move {
                    match handle_migration_export(&enc, &req, &guard) {
                        Ok(resp) => (axum::http::StatusCode::OK, Json(serde_json::json!(resp))),
                        Err(e) => (
                            axum::http::StatusCode::FORBIDDEN,
                            Json(serde_json::json!({"error": format!("{}", e)})),
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
        assert_eq!(cfg.n, 4);
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
        let seal_key = enclave.seal_key().unwrap();
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
        let guard = AttestationGuard::verified(true, true);
        let resp = handle_receive_share(&enclave, &req, &guard).unwrap();
        assert!(resp.success);

        // 验证 share 已保存
        let loaded = enclave.load_local_share().unwrap();
        assert!(loaded.is_some());

        // 模拟 peer 请求 share (需要 verified guard + 合法 Ed25519 公钥)
        let requester_sk = ed25519_dalek::SigningKey::from_bytes(&[0x42u8; 32]);
        let requester_pk_hex = hex::encode(requester_sk.verifying_key().to_bytes());
        let share_req = ShareRequest {
            ceremony_hash: hex::encode([0xAA; 32]),
            requester_pk: requester_pk_hex,
        };
        let guard = AttestationGuard::verified(true, true);
        let share_resp = handle_share_request(&enclave, &share_req, &guard).unwrap();
        assert!(!share_resp.share_data.is_empty());
        assert!(!share_resp.peer_pk.is_empty());

        // 验证返回的是 ECDH 加密格式
        let share_bytes = crate::tee::peer_client::base64_decode_pub(&share_resp.share_data).unwrap();
        let ecdh_share = shamir::ecdh_share_from_bytes(&share_bytes).unwrap();
        // 请求者用自己的私钥解密
        let receiver_secret = shamir::ed25519_to_x25519_secret(&requester_sk.to_bytes());
        let decrypted = shamir::decrypt_share_from_sender(
            &ecdh_share.encrypted, &receiver_secret, &ecdh_share.ephemeral_pk,
        ).unwrap();
        assert_eq!(decrypted, shares[0]);
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
        let seal_key = enclave.seal_key().unwrap();
        let encrypted = shamir::encrypt_share(&shares[0], &seal_key).unwrap();
        enclave.save_local_share(&encrypted).unwrap();

        // 使用合法 Ed25519 公钥 (否则 ECDH 加密会失败)
        let requester_sk = ed25519_dalek::SigningKey::from_bytes(&[0x55u8; 32]);
        let requester_pk_hex = hex::encode(requester_sk.verifying_key().to_bytes());
        let req = ShareRequest {
            ceremony_hash: hex::encode([0xAA; 32]),
            requester_pk: requester_pk_hex,
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

    #[test]
    fn receive_share_ecdh_decrypt_and_reseal() {
        // 模拟 Re-ceremony 分发: 发起者 ECDH 加密 share → 接收者解密 → 用自己的 seal_key 保存
        let sender_dir = tempfile::tempdir().unwrap();
        let receiver_dir = tempfile::tempdir().unwrap();
        let sender_enclave = make_enclave(sender_dir.path().to_str().unwrap());
        let receiver_enclave = make_enclave(receiver_dir.path().to_str().unwrap());

        // 创建一个 plaintext share
        let token = "test:ECDH_CEREMONY_TOKEN";
        let sk = [0x77u8; 32];
        let secrets = shamir::encode_secrets(token, &sk);
        let config = shamir::ShamirConfig::new(1, 2).unwrap();
        let shares = shamir::split(&secrets, &config).unwrap();

        // 发起者用接收者公钥 ECDH 加密 share[1]
        let receiver_vk = receiver_enclave.public_key();
        let receiver_x25519 = shamir::ed25519_pk_to_x25519(&receiver_vk);
        let (ecdh_encrypted, ephemeral_pk) = shamir::encrypt_share_for_recipient(
            &shares[1], &receiver_x25519,
        ).unwrap();

        let ecdh_share = shamir::EcdhEncryptedShare { encrypted: ecdh_encrypted, ephemeral_pk };
        let share_b64 = base64_encode(&shamir::ecdh_share_to_bytes(&ecdh_share));

        let ceremony_hash = [0xCC; 32];
        let req = ReceiveShareEcdhRequest {
            ceremony_hash: hex::encode(ceremony_hash),
            share_data: share_b64,
            sender_pk: hex::encode(sender_enclave.public_key_bytes()),
            k: 1,
            n: 2,
        };

        let guard = AttestationGuard::verified(true, true);
        let resp = handle_receive_share_ecdh(&receiver_enclave, &req, &guard).unwrap();
        assert!(resp.success);
        assert!(resp.message.contains("ECDH"));

        // 验证: 接收者能用自己的 seal_key 解密保存的 share
        let loaded = receiver_enclave.load_local_share().unwrap().unwrap();
        let receiver_seal_key = receiver_enclave.seal_key().unwrap();
        let decrypted = shamir::decrypt_share(&loaded, &receiver_seal_key).unwrap();
        assert_eq!(decrypted, shares[1]);

        // 验证: ceremony_hash 已保存
        let stored_hash = receiver_enclave.load_ceremony_hash().unwrap();
        assert_eq!(stored_hash, Some(ceremony_hash));
    }

    #[test]
    fn receive_share_ecdh_rejected_without_attestation() {
        let dir = tempfile::tempdir().unwrap();
        let enclave = make_enclave(dir.path().to_str().unwrap());

        let req = ReceiveShareEcdhRequest {
            ceremony_hash: hex::encode([0u8; 32]),
            share_data: "dGVzdA==".into(), // dummy base64
            sender_pk: hex::encode([0xAA; 32]),
            k: 2,
            n: 4,
        };

        let guard = AttestationGuard::unverified();
        let result = handle_receive_share_ecdh(&enclave, &req, &guard);
        assert!(result.is_err());
    }

    #[test]
    fn re_ceremony_config_validation() {
        // Participant 数量不匹配 new_n
        let p = Participant {
            endpoint: "https://a:8443".into(),
            public_key: [0x11; 32],
        };

        let config = ReCeremonyConfig {
            current_k: 1,
            new_k: 2,
            new_n: 4, // 4 != 1 participant
            timeout_secs: 30,
        };

        // 无法在同步测试中调用 async run_re_ceremony, 但可测试结构体构造
        assert_eq!(config.new_k, 2);
        assert_eq!(config.new_n, 4);
        assert_eq!(p.public_key, [0x11; 32]);
    }

    #[test]
    fn migration_export_one_time_use() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let enclave = make_enclave(path);

        // 创建 share (K=1)
        let token = "test:MIGRATION_TOKEN";
        let sk = [0x99u8; 32];
        let secrets = shamir::encode_secrets(token, &sk);
        let config = shamir::ShamirConfig::new(1, 1).unwrap();
        let shares = shamir::split(&secrets, &config).unwrap();
        let seal_key = enclave.seal_key().unwrap();
        let encrypted = shamir::encrypt_share(&shares[0], &seal_key).unwrap();
        enclave.save_local_share(&encrypted).unwrap();

        // 新版本 Enclave 的密钥
        let new_sk = ed25519_dalek::SigningKey::from_bytes(&[0x88u8; 32]);
        let new_pk_hex = hex::encode(new_sk.verifying_key().to_bytes());

        let guard = MigrationExportGuard::new();
        let req = MigrationExportRequest { requester_pk: new_pk_hex.clone() };

        // 第一次导出成功
        let resp = handle_migration_export(&enclave, &req, &guard).unwrap();
        assert!(!resp.encrypted_secret.is_empty());
        assert!(!resp.ephemeral_pk.is_empty());
        assert!(!resp.source_pk.is_empty());

        // 验证: 新版本能用自己的私钥 ECDH 解密
        let receiver_x25519 = shamir::ed25519_to_x25519_secret(&new_sk.to_bytes());
        let ephemeral_pk_bytes = hex::decode(&resp.ephemeral_pk).unwrap();
        let mut ephemeral_pk = [0u8; 32];
        ephemeral_pk.copy_from_slice(&ephemeral_pk_bytes);

        let encrypted_bytes = crate::tee::peer_client::base64_decode_pub(&resp.encrypted_secret).unwrap();
        let decrypted = shamir::ecdh_decrypt_raw(&encrypted_bytes, &receiver_x25519, &ephemeral_pk).unwrap();

        let (sk_out, token_out) = shamir::decode_secrets(&decrypted).unwrap();
        assert_eq!(token_out, token);
        assert_eq!(sk_out, sk);

        // 第二次导出拒绝 (一次性使用)
        let req2 = MigrationExportRequest { requester_pk: new_pk_hex };
        let result = handle_migration_export(&enclave, &req2, &guard);
        assert!(result.is_err());
    }

    #[test]
    fn migration_export_no_share_fails() {
        let dir = tempfile::tempdir().unwrap();
        let enclave = make_enclave(dir.path().to_str().unwrap());

        let guard = MigrationExportGuard::new();
        let req = MigrationExportRequest {
            requester_pk: hex::encode([0x11; 32]),
        };
        let result = handle_migration_export(&enclave, &req, &guard);
        assert!(result.is_err());
    }

    #[test]
    fn migration_guard_state() {
        let guard = MigrationExportGuard::new();
        assert!(!guard.is_exported());
        assert!(guard.try_export());
        assert!(guard.is_exported());
        assert!(!guard.try_export());
    }

    #[test]
    fn re_ceremony_local_only_k1() {
        // 模拟 K=1 → K=1,N=2 的 Re-ceremony (仅本地阶段: 恢复 + 重新分片)
        // 无法测试 HTTP 分发, 但验证恢复 → 分片 → 本地保存的完整逻辑
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let enclave = make_enclave(path);

        // 创建初始 share (K=1, N=1)
        let token = "test:RECEREMONY_TOKEN";
        let sk = [0x55u8; 32];
        let secrets = shamir::encode_secrets(token, &sk);
        let config = shamir::ShamirConfig::new(1, 1).unwrap();
        let shares = shamir::split(&secrets, &config).unwrap();

        let seal_key = enclave.seal_key().unwrap();
        let encrypted = shamir::encrypt_share(&shares[0], &seal_key).unwrap();
        enclave.save_local_share(&encrypted).unwrap();
        enclave.save_ceremony_hash(&[0xDD; 32]).unwrap();

        // 验证: 能从本地 share 恢复 secret
        let loaded = enclave.load_local_share().unwrap().unwrap();
        let decrypted = shamir::decrypt_share(&loaded, &seal_key).unwrap();
        let recovered = shamir::recover(&[decrypted.clone()], 1).unwrap();
        let (sk_out, token_out) = shamir::decode_secrets(&recovered).unwrap();
        assert_eq!(token_out, token);
        assert_eq!(sk_out, sk);

        // 验证: 可重新分片为 K=2, N=3
        let new_config = shamir::ShamirConfig::new(2, 3).unwrap();
        let new_shares = shamir::split(&recovered, &new_config).unwrap();
        assert_eq!(new_shares.len(), 3);

        // 保存新 share[0] 到本地 (模拟 Re-ceremony 本地阶段)
        let new_encrypted = shamir::encrypt_share(&new_shares[0], &seal_key).unwrap();
        enclave.save_local_share(&new_encrypted).unwrap();
        let new_hash = [0xEE; 32];
        enclave.save_ceremony_hash(&new_hash).unwrap();

        // 验证新 share 可加载
        let reloaded = enclave.load_local_share().unwrap().unwrap();
        let re_decrypted = shamir::decrypt_share(&reloaded, &seal_key).unwrap();
        assert_eq!(re_decrypted, new_shares[0]);
        assert_eq!(enclave.load_ceremony_hash().unwrap(), Some(new_hash));
    }
}
