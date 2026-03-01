// Share Recovery — 5 级恢复链
//
// 优先级:
// 1. 本地密封 share (MRSIGNER/MRENCLAVE 兼容读取)
// 2. Peer share 收集 (K-of-N, 滚动升级)
// 3. Migration Ceremony (从旧版本 Enclave ECDH 传递)
// 4. RA-TLS Provision (DApp 注入, 管理员干预)
// 5. 环境变量 fallback + auto-seal (紧急恢复)

use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};
use zeroize::Zeroizing;

use crate::chain::ChainClient;
use crate::error::{BotError, BotResult};
use crate::tee::enclave_bridge::EnclaveBridge;
use crate::tee::shamir;
use crate::tee::token_vault::TokenVault;

/// Share 恢复结果
#[allow(dead_code)]
pub struct RecoveryResult {
    /// 恢复后的 TokenVault (已注入 Token)
    pub vault: TokenVault,
    /// 恢复的签名密钥 (32 bytes)
    pub signing_key: Zeroizing<[u8; 32]>,
    /// 恢复来源
    pub source: RecoverySource,
}

/// 恢复来源 (5 级)
#[derive(Debug, Clone, PartialEq)]
pub enum RecoverySource {
    /// Level 1: 从本地密封 share 恢复 (K=1, MRSIGNER/MRENCLAVE dual-key)
    LocalShare,
    /// Level 2: 从本地 + peer 收集 share 恢复 (K>1)
    PeerShares { collected: usize, threshold: u8 },
    /// Level 3: 从旧版本 Enclave 迁移恢复 (Migration Ceremony)
    MigrationCeremony { source_endpoint: String },
    /// Level 4: 从 RA-TLS Provision 恢复 (已有机制, 此处标记)
    #[allow(dead_code)]
    RaTlsProvision,
    /// Level 5: 从环境变量 fallback (过渡模式)
    EnvironmentVariable,
}

impl std::fmt::Display for RecoverySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LocalShare => write!(f, "local sealed share"),
            Self::PeerShares { collected, threshold } => {
                write!(f, "{}/{} peer shares", collected, threshold)
            }
            Self::MigrationCeremony { source_endpoint } => {
                write!(f, "migration ceremony from {}", source_endpoint)
            }
            Self::RaTlsProvision => write!(f, "RA-TLS provision"),
            Self::EnvironmentVariable => write!(f, "environment variable (INSECURE)"),
        }
    }
}

/// 恢复配置
pub struct RecoveryConfig {
    /// Shamir 门限 K
    pub threshold: u8,
    /// 平台模式 (需要哪些 Token)
    pub needs_telegram: bool,
    pub needs_discord: bool,
    /// Peer TEE 节点端点 (用于 K>1 收集 share)
    pub peer_endpoints: Vec<String>,
    /// 仪式 hash (用于向 peer 请求对应 share)
    pub ceremony_hash: [u8; 32],
    /// 链客户端 (用于从链上 PeerRegistry 自动发现 peer)
    pub chain_client: Option<Arc<ChainClient>>,
    /// Bot ID Hash (用于链上查询 PeerRegistry)
    pub bot_id_hash: Option<[u8; 32]>,
    /// 旧版本 Enclave 端点 (用于 Migration Ceremony, Level 3)
    pub migration_source: Option<String>,
}

/// 5 级恢复链: 按优先级尝试所有恢复路径
pub async fn recover_token(
    enclave: &Arc<EnclaveBridge>,
    config: &RecoveryConfig,
) -> BotResult<RecoveryResult> {
    // ── Level 1+2: 本地密封 share (+ peer if K>1) ──
    match try_share_recovery(enclave, config).await {
        Ok(mut result) => {
            inject_sealed_api_credentials(enclave, &mut result.vault);
            info!(source = %result.source, "Token 已从 Shamir share 恢复");
            return Ok(result);
        }
        Err(e) => {
            warn!(error = %e, "Level 1/2 share 恢复失败");
        }
    }

    // ── Level 3: Migration Ceremony (从旧版本 Enclave 获取) ──
    if let Some(ref source_endpoint) = config.migration_source {
        info!(endpoint = %source_endpoint, "尝试 Level 3: Migration Ceremony");
        match try_migration_recovery(enclave, config, source_endpoint).await {
            Ok(mut result) => {
                inject_sealed_api_credentials(enclave, &mut result.vault);
                info!(source = %result.source, "Token 已从 Migration Ceremony 恢复");
                return Ok(result);
            }
            Err(e) => {
                warn!(error = %e, "Level 3 Migration Ceremony 失败");
            }
        }
    }

    // ── Level 4: RA-TLS Provision (已由外部机制处理, 此处仅记录) ──
    // RA-TLS Provision 通过 /provision/inject-token 端点异步注入,
    // 不在 startup recovery chain 中同步等待.
    // 如果 provision 已注入 Token, try_share_recovery 已经能读到.

    // ── Level 5: 环境变量 fallback + auto-seal (紧急恢复) ──
    warn!("⚠️ Level 1-3 均失败, 降级到 Level 5: 环境变量 fallback");
    try_env_fallback(enclave, config)
}

/// 从密封存储加载 Telegram API credentials 到 Vault
fn inject_sealed_api_credentials(enclave: &EnclaveBridge, vault: &mut TokenVault) {
    match enclave.load_api_credentials() {
        Ok(Some((api_id, api_hash))) => {
            vault.set_telegram_api_credentials(api_id, api_hash);
            info!("已从密封存储恢复 Telegram API credentials");
        }
        Ok(None) => {}
        Err(e) => {
            warn!(error = %e, "加载密封 API credentials 失败");
        }
    }
}

/// 从 share 恢复 (K=1 本地, K>1 本地+peer)
async fn try_share_recovery(
    enclave: &Arc<EnclaveBridge>,
    config: &RecoveryConfig,
) -> BotResult<RecoveryResult> {
    let encrypted_share = enclave.load_local_share()?
        .ok_or_else(|| BotError::EnclaveError("No local share found".into()))?;

    let seal_key = enclave.seal_key()?;

    // 解密本地 share
    let local_share = shamir::decrypt_share(&encrypted_share, &seal_key)
        .map_err(|e| BotError::EnclaveError(format!("share decrypt: {}", e)))?;

    let (secrets, source) = if config.threshold <= 1 {
        // K=1: 单个 share 即可恢复
        let secrets = shamir::recover(&[local_share], 1)
            .map_err(|e| BotError::EnclaveError(format!("Shamir recover: {}", e)))?;
        (secrets, RecoverySource::LocalShare)
    } else {
        // K>1: 需要从 peer 收集 K-1 个额外 share
        let needed = (config.threshold - 1) as usize;

        // 如果 peer_endpoints 为空, 尝试从链上 PeerRegistry 自动发现
        let mut endpoints = config.peer_endpoints.clone();
        if endpoints.is_empty() {
            if let Some(discovered) = auto_discover_peers(config, enclave).await? {
                endpoints = discovered;
            }
        }
        if endpoints.is_empty() {
            return Err(BotError::EnclaveError(format!(
                "K={} but no peer_endpoints configured and chain discovery found none", config.threshold
            )));
        }

        info!(
            k = config.threshold, peers = endpoints.len(),
            "K>1 恢复: 从 peer 收集 {} 个额外 share", needed
        );

        let peer_config = crate::tee::peer_client::PeerClientConfig::default();
        let peer_client = crate::tee::peer_client::PeerClient::new(peer_config)?;

        let requester_pk = enclave.public_key_bytes();
        let peer_encrypted = peer_client.collect_shares(
            &endpoints,
            &config.ceremony_hash,
            &requester_pk,
            needed,
        ).await?;

        let receiver_x25519_secret = shamir::ed25519_to_x25519_secret(&enclave.signing_key().to_bytes());
        let mut all_shares = vec![local_share];
        for ecdh_share in &peer_encrypted {
            let share = shamir::decrypt_share_from_sender(
                &ecdh_share.encrypted,
                &receiver_x25519_secret,
                &ecdh_share.ephemeral_pk,
            ).map_err(|e| BotError::EnclaveError(format!(
                "peer share ECDH decrypt failed: {}", e
            )))?;
            all_shares.push(share);
        }

        let total = all_shares.len();
        let secrets = shamir::recover(&all_shares, config.threshold)
            .map_err(|e| BotError::EnclaveError(format!("Shamir recover: {}", e)))?;

        (secrets, RecoverySource::PeerShares {
            collected: total,
            threshold: config.threshold,
        })
    };

    // decode secrets → (signing_key, bot_token)
    let (sk_bytes, bot_token) = shamir::decode_secrets(&secrets)
        .map_err(|e| BotError::EnclaveError(format!("decode secrets: {}", e)))?;

    let mut vault = TokenVault::new();

    if config.needs_telegram {
        vault.set_telegram_token(bot_token.clone());
    }
    if config.needs_discord {
        vault.set_discord_token(bot_token);
    }

    let mut signing_key = Zeroizing::new([0u8; 32]);
    signing_key.copy_from_slice(&sk_bytes);

    Ok(RecoveryResult {
        vault,
        signing_key,
        source,
    })
}

// ═══════════════════════════════════════════════════════════════
// Level 3: Migration Ceremony — 从旧版本 Enclave 获取密钥
// ═══════════════════════════════════════════════════════════════

/// Migration 导出响应 (旧版本 Enclave 返回)
#[derive(serde::Deserialize)]
struct MigrationExportResponse {
    /// ECDH 加密的 secret (base64)
    encrypted_secret: String,
    /// 临时 X25519 公钥 (hex, 用于 ECDH 解密)
    ephemeral_pk: String,
    /// 旧版本的 Ed25519 公钥 (hex)
    #[allow(dead_code)]
    source_pk: String,
}

/// 从旧版本 Enclave 请求密钥迁移
async fn try_migration_recovery(
    enclave: &Arc<EnclaveBridge>,
    config: &RecoveryConfig,
    source_endpoint: &str,
) -> BotResult<RecoveryResult> {
    let url = format!(
        "{}/migration/export-secret",
        source_endpoint.trim_end_matches('/')
    );

    let my_pk = enclave.public_key_bytes();
    let my_pk_hex = hex::encode(my_pk);

    info!(url = %url, requester_pk = %my_pk_hex, "发起 Migration Ceremony 请求");

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| BotError::EnclaveError(format!("http client: {}", e)))?;

    let req_body = serde_json::json!({
        "requester_pk": my_pk_hex,
    });

    let resp = http.post(&url)
        .json(&req_body)
        .send()
        .await
        .map_err(|e| BotError::EnclaveError(format!("migration request failed: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(BotError::EnclaveError(format!(
            "migration export failed: {} — {}", status, body
        )));
    }

    let export_resp: MigrationExportResponse = resp.json().await
        .map_err(|e| BotError::EnclaveError(format!("migration response parse: {}", e)))?;

    // ECDH 解密: 用本节点的 Ed25519 私钥 → X25519 解密
    let receiver_x25519 = shamir::ed25519_to_x25519_secret(&enclave.signing_key().to_bytes());

    let ephemeral_pk_bytes = hex::decode(&export_resp.ephemeral_pk)
        .map_err(|e| BotError::EnclaveError(format!("ephemeral_pk hex: {}", e)))?;
    if ephemeral_pk_bytes.len() != 32 {
        return Err(BotError::EnclaveError("ephemeral_pk must be 32 bytes".into()));
    }
    let mut ephemeral_pk = [0u8; 32];
    ephemeral_pk.copy_from_slice(&ephemeral_pk_bytes);

    let encrypted_bytes = crate::tee::peer_client::base64_decode_pub(&export_resp.encrypted_secret)
        .map_err(|e| BotError::EnclaveError(format!("encrypted_secret base64: {}", e)))?;

    let secret_bytes = shamir::ecdh_decrypt_raw(&encrypted_bytes, &receiver_x25519, &ephemeral_pk)
        .map_err(|e| BotError::EnclaveError(format!("migration ECDH decrypt: {}", e)))?;

    // 解码: secret_bytes = encode_secrets(bot_token, signing_key)
    let (sk_bytes, bot_token) = shamir::decode_secrets(&secret_bytes)
        .map_err(|e| BotError::EnclaveError(format!("migration decode secrets: {}", e)))?;

    let mut vault = TokenVault::new();
    if config.needs_telegram {
        vault.set_telegram_token(bot_token.clone());
    }
    if config.needs_discord {
        vault.set_discord_token(bot_token.clone());
    }

    let mut signing_key = Zeroizing::new([0u8; 32]);
    signing_key.copy_from_slice(&sk_bytes);

    // 用迁移的密钥替换当前自动生成的密钥 (保持链上身份)
    // replace_signing_key 会用当前 SealPolicy 重新密封
    // 注意: enclave 被 Arc 包裹, 需要获取可变引用的方式处理
    // 这里通过 auto-seal 机制保存: 创建 K=1 share 并密封
    auto_seal_token(enclave, &bot_token, &sk_bytes)?;

    info!(
        source = %source_endpoint,
        signing_key_pk = %hex::encode(ed25519_dalek::SigningKey::from_bytes(&sk_bytes).verifying_key().to_bytes()),
        "Migration Ceremony 成功: secret 已恢复并密封"
    );

    Ok(RecoveryResult {
        vault,
        signing_key,
        source: RecoverySource::MigrationCeremony {
            source_endpoint: source_endpoint.to_string(),
        },
    })
}

/// 从链上 PeerRegistry 自动发现 peer 端点
async fn auto_discover_peers(
    config: &RecoveryConfig,
    enclave: &Arc<EnclaveBridge>,
) -> BotResult<Option<Vec<String>>> {
    let chain = match config.chain_client.as_ref() {
        Some(c) => c,
        None => return Ok(None),
    };
    let bot_id_hash = match config.bot_id_hash.as_ref() {
        Some(h) => h,
        None => return Ok(None),
    };

    info!("尝试从链上 PeerRegistry 自动发现 peer...");
    let peers = chain.query_peer_registry(bot_id_hash).await?;

    if peers.is_empty() {
        warn!("链上 PeerRegistry 无注册 peer");
        return Ok(None);
    }

    let my_pk = enclave.public_key_bytes();
    let endpoints: Vec<String> = peers.iter()
        .filter(|p| p.public_key != my_pk)
        .filter(|p| !p.endpoint.is_empty())
        .map(|p| p.endpoint.clone())
        .collect();

    if endpoints.is_empty() {
        warn!(total_peers = peers.len(), "链上有 peer 但排除自己后无可用端点");
        return Ok(None);
    }

    info!(
        discovered = endpoints.len(),
        total = peers.len(),
        "从链上 PeerRegistry 发现 {} 个 peer 端点", endpoints.len()
    );
    Ok(Some(endpoints))
}

/// 环境变量 fallback (过渡模式)
///
/// 首次使用环境变量加载后, 自动创建 Shamir share 并密封保存,
/// 这样下次启动就能直接从 share 恢复, 无需再依赖环境变量。
fn try_env_fallback(
    enclave: &Arc<EnclaveBridge>,
    config: &RecoveryConfig,
) -> BotResult<RecoveryResult> {
    warn!("⚠️  使用环境变量加载 Token (不安全, 仅过渡用途)");
    warn!("⚠️  请尽快执行 Ceremony 生成 Shamir share");

    let mut vault = TokenVault::new();

    let mut primary_token: Option<Zeroizing<String>>;

    if config.needs_telegram {
        let token = Zeroizing::new(std::env::var("BOT_TOKEN")
            .map_err(|_| BotError::Config("BOT_TOKEN required (no share available)".into()))?);
        primary_token = Some(token.clone());
        vault.set_telegram_token(token);
    } else {
        primary_token = None;
    }

    if config.needs_discord {
        let token = Zeroizing::new(std::env::var("DISCORD_BOT_TOKEN")
            .map_err(|_| BotError::Config("DISCORD_BOT_TOKEN required (no share available)".into()))?);
        if primary_token.is_none() {
            primary_token = Some(token.clone());
        }
        vault.set_discord_token(token);
    }

    let mut signing_key = Zeroizing::new([0u8; 32]);
    signing_key.copy_from_slice(&enclave.signing_key().to_bytes());

    // ── Auto-seal: 将 Token 自动保存为 Shamir share ──
    if let Some(ref token) = primary_token {
        match auto_seal_token(enclave, token.as_str(), &signing_key) {
            Ok(()) => {
                info!("Token 已自动密封为 Shamir share (下次启动将从 share 恢复)");
                warn!("⚠️  请在下次部署时移除 BOT_TOKEN / DISCORD_BOT_TOKEN 环境变量");
            }
            Err(e) => {
                warn!(error = %e, "auto-seal 失败, 下次启动仍需环境变量");
            }
        }
    }

    // ── Telegram API credentials (Local Bot API Server, 独立于 bot token) ──
    if let (Ok(api_id), Ok(api_hash)) = (std::env::var("TG_API_ID"), std::env::var("TG_API_HASH")) {
        let api_id_z = Zeroizing::new(api_id);
        let api_hash_z = Zeroizing::new(api_hash);
        if let Err(e) = enclave.save_api_credentials(&api_id_z, &api_hash_z) {
            warn!(error = %e, "API credentials auto-seal 失败");
        } else {
            info!("Telegram API credentials 已密封保存 (下次启动自动加载)");
        }
        vault.set_telegram_api_credentials(api_id_z, api_hash_z);
        std::env::remove_var("TG_API_ID");
        std::env::remove_var("TG_API_HASH");
    } else {
        inject_sealed_api_credentials(enclave, &mut vault);
    }

    // 清除环境变量中的 token 明文
    if config.needs_telegram {
        std::env::remove_var("BOT_TOKEN");
    }
    if config.needs_discord {
        std::env::remove_var("DISCORD_BOT_TOKEN");
    }

    Ok(RecoveryResult {
        vault,
        signing_key,
        source: RecoverySource::EnvironmentVariable,
    })
}

/// 自动将 token 密封为 K=1 Shamir share
fn auto_seal_token(
    enclave: &Arc<EnclaveBridge>,
    token: &str,
    signing_key: &[u8; 32],
) -> BotResult<()> {
    if let Ok(Some(_)) = enclave.load_local_share() {
        info!("本地已有 share, 跳过 auto-seal");
        return Ok(());
    }

    let zero_hash = [0u8; 32];
    create_and_save_share(enclave, token, signing_key, 1, 1, 0, &zero_hash)?;
    Ok(())
}

/// 执行 Ceremony 后保存 share (由 Ceremony 端点调用)
pub fn create_and_save_share(
    enclave: &Arc<EnclaveBridge>,
    bot_token: &str,
    signing_key: &[u8; 32],
    k: u8,
    n: u8,
    local_share_index: usize,
    ceremony_hash: &[u8; 32],
) -> BotResult<()> {
    let secrets = shamir::encode_secrets(bot_token, signing_key);

    let config = shamir::ShamirConfig::new(k, n)
        .map_err(|e| BotError::EnclaveError(format!("Shamir config: {}", e)))?;
    let shares = shamir::split(&secrets, &config)
        .map_err(|e| BotError::EnclaveError(format!("Shamir split: {}", e)))?;

    if local_share_index >= shares.len() {
        return Err(BotError::EnclaveError(format!(
            "share index {} out of range (n={})", local_share_index, n
        )));
    }

    let seal_key = enclave.seal_key()?;
    let encrypted = shamir::encrypt_share(&shares[local_share_index], &seal_key)
        .map_err(|e| BotError::EnclaveError(format!("encrypt share: {}", e)))?;

    enclave.save_local_share(&encrypted)?;
    enclave.save_ceremony_hash(ceremony_hash)?;

    info!(
        k = k, n = n, share_id = shares[local_share_index].id,
        ceremony = %hex::encode(ceremony_hash),
        "Ceremony share 已创建并密封保存"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_enclave(dir: &str) -> Arc<EnclaveBridge> {
        Arc::new(EnclaveBridge::init(dir, "software").unwrap())
    }

    fn default_config(threshold: u8, tg: bool, dc: bool) -> RecoveryConfig {
        RecoveryConfig {
            threshold,
            needs_telegram: tg,
            needs_discord: dc,
            peer_endpoints: vec![],
            ceremony_hash: [0u8; 32],
            chain_client: None,
            bot_id_hash: None,
            migration_source: None,
        }
    }

    #[tokio::test]
    async fn create_and_recover_share() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let enclave = make_enclave(path);

        let token = "123456:ABCDEF_token";
        let sk = [0x42u8; 32];
        let ch = [0xAA; 32];
        create_and_save_share(&enclave, token, &sk, 1, 1, 0, &ch).unwrap();

        let config = default_config(1, true, false);
        let result = recover_token(&enclave, &config).await.unwrap();
        assert_eq!(result.source, RecoverySource::LocalShare);
        assert!(result.vault.has_telegram_token());

        let url = result.vault.build_tg_api_url("getMe").unwrap();
        assert!(url.contains("123456:ABCDEF_token"));
        assert_eq!(&*result.signing_key, &sk);
        assert_eq!(enclave.load_ceremony_hash().unwrap(), Some(ch));
    }

    #[test]
    fn no_share_falls_back_to_env() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let enclave = make_enclave(path);

        std::env::set_var("BOT_TOKEN", "env_test_token:123");

        let config = default_config(1, true, false);
        let result = try_env_fallback(&enclave, &config).unwrap();
        assert_eq!(result.source, RecoverySource::EnvironmentVariable);
        assert!(result.vault.has_telegram_token());

        assert!(enclave.load_local_share().unwrap().is_some());

        std::env::remove_var("BOT_TOKEN");
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let r2 = rt.block_on(recover_token(&enclave, &config)).unwrap();
        assert_eq!(r2.source, RecoverySource::LocalShare);
        assert!(r2.vault.has_telegram_token());

        let url = r2.vault.build_tg_api_url("getMe").unwrap();
        assert!(url.contains("env_test_token:123"));
    }

    #[test]
    fn create_share_invalid_index() {
        let dir = tempfile::tempdir().unwrap();
        let enclave = make_enclave(dir.path().to_str().unwrap());
        let result = create_and_save_share(&enclave, "token", &[0u8; 32], 2, 3, 5, &[0u8; 32]);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn encode_split_save_load_recover_decode() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let enclave = make_enclave(path);

        let token = "my_bot:secret_token_789";
        let sk = [0xABu8; 32];
        create_and_save_share(&enclave, token, &sk, 1, 3, 0, &[0u8; 32]).unwrap();

        let config = default_config(1, true, true);
        let result = recover_token(&enclave, &config).await.unwrap();
        assert_eq!(result.source, RecoverySource::LocalShare);
        assert!(result.vault.has_telegram_token());
        assert!(result.vault.has_discord_token());
        assert_eq!(&*result.signing_key, &sk);
    }

    #[test]
    fn recovery_source_display() {
        assert_eq!(format!("{}", RecoverySource::LocalShare), "local sealed share");
        assert_eq!(
            format!("{}", RecoverySource::PeerShares { collected: 2, threshold: 3 }),
            "2/3 peer shares"
        );
        assert_eq!(
            format!("{}", RecoverySource::MigrationCeremony {
                source_endpoint: "https://old:3000".into(),
            }),
            "migration ceremony from https://old:3000"
        );
        assert_eq!(
            format!("{}", RecoverySource::EnvironmentVariable),
            "environment variable (INSECURE)"
        );
    }

    #[tokio::test]
    async fn k_gt_1_no_peers_fails_share_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let enclave = make_enclave(path);

        let token = "test:TOKEN";
        let sk = [0x11u8; 32];
        create_and_save_share(&enclave, token, &sk, 2, 3, 0, &[0u8; 32]).unwrap();

        let config = RecoveryConfig {
            threshold: 2,
            needs_telegram: true,
            needs_discord: false,
            peer_endpoints: vec![],
            ceremony_hash: [0u8; 32],
            chain_client: None,
            bot_id_hash: None,
            migration_source: None,
        };
        let result = try_share_recovery(&enclave, &config).await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.err().unwrap());
        assert!(err_msg.contains("no peer_endpoints"), "expected peer_endpoints error, got: {}", err_msg);
    }

    #[tokio::test]
    async fn migration_recovery_unreachable_endpoint_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let enclave = make_enclave(path);

        let config = default_config(1, true, false);
        let result = try_migration_recovery(
            &enclave, &config, "http://127.0.0.1:59999",
        ).await;
        assert!(result.is_err(), "unreachable migration source should fail");
        let err = format!("{}", result.err().unwrap());
        assert!(
            err.contains("migration request failed") || err.contains("connection"),
            "expected connection error, got: {}", err
        );
    }

    #[test]
    fn recovery_source_migration_eq() {
        let a = RecoverySource::MigrationCeremony {
            source_endpoint: "https://a:3000".into(),
        };
        let b = RecoverySource::MigrationCeremony {
            source_endpoint: "https://a:3000".into(),
        };
        assert_eq!(a, b);
    }
}
