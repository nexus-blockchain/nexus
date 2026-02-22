// Share Recovery — 启动时从 Shamir Share 恢复 Token + 签名密钥
//
// 恢复流程:
// 1. 尝试加载本地密封 share
// 2. K=1: 单个 share 直接恢复
//    K>1: 本地 share + 从 peer 收集 K-1 个 share → Shamir recover
// 3. decode_secrets → (signing_key, bot_token)
// 4. 注入 TokenVault
//
// Fallback: 无 share 时从环境变量加载 (过渡模式, 带告警)

use std::sync::Arc;

use tracing::{info, warn};
use zeroize::Zeroizing;

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

/// 恢复来源
#[derive(Debug, Clone, PartialEq)]
pub enum RecoverySource {
    /// 从本地密封 share 恢复 (K=1)
    LocalShare,
    /// 从本地 + peer 收集 share 恢复 (K>1)
    PeerShares { collected: usize, threshold: u8 },
    /// 从环境变量 fallback (过渡模式)
    EnvironmentVariable,
}

impl std::fmt::Display for RecoverySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LocalShare => write!(f, "local sealed share"),
            Self::PeerShares { collected, threshold } => {
                write!(f, "{}/{} peer shares", collected, threshold)
            }
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
}

/// 尝试从密封 share 恢复 Token
///
/// 优先级:
/// 1. 本地密封 share (+ peer shares if K>1) → Shamir recover → decode_secrets
/// 2. 环境变量 fallback (过渡模式, 带告警)
pub async fn recover_token(
    enclave: &Arc<EnclaveBridge>,
    config: &RecoveryConfig,
) -> BotResult<RecoveryResult> {
    // 尝试从 share 恢复
    match try_share_recovery(enclave, config).await {
        Ok(result) => {
            info!(source = %result.source, "Token 已从 Shamir share 恢复");
            return Ok(result);
        }
        Err(e) => {
            warn!(error = %e, "Share 恢复失败, 尝试环境变量 fallback");
        }
    }

    // Fallback: 环境变量 (过渡模式) + auto-seal
    try_env_fallback(enclave, config)
}

/// 从 share 恢复 (K=1 本地, K>1 本地+peer)
async fn try_share_recovery(
    enclave: &Arc<EnclaveBridge>,
    config: &RecoveryConfig,
) -> BotResult<RecoveryResult> {
    let encrypted_share = enclave.load_local_share()?
        .ok_or_else(|| BotError::EnclaveError("No local share found".into()))?;

    let seal_key = enclave.seal_key();

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
        if config.peer_endpoints.is_empty() {
            return Err(BotError::EnclaveError(format!(
                "K={} but no peer_endpoints configured", config.threshold
            )));
        }

        info!(
            k = config.threshold, peers = config.peer_endpoints.len(),
            "K>1 恢复: 从 peer 收集 {} 个额外 share", needed
        );

        let peer_config = crate::tee::peer_client::PeerClientConfig::default();
        let peer_client = crate::tee::peer_client::PeerClient::new(peer_config)?;

        let requester_pk = enclave.public_key_bytes();
        let peer_encrypted = peer_client.collect_shares(
            &config.peer_endpoints,
            &config.ceremony_hash,
            &requester_pk,
            needed,
        ).await?;

        // 解密 peer shares
        //
        // ⚠️ 注意: 当前设计中, 所有 share 使用 ceremony 发起端的 seal_key 加密。
        // Peer 节点将 EncryptedShare 原样存储 (外层由 sealed_storage 保护)。
        // 因此所有参与节点必须使用相同的 ceremony 加密密钥才能互相解密 share。
        //
        // 如果各节点的 seal_key 不同 (生产环境中大概率如此), 需要:
        // 1. 使用 ceremony 级别的共享密钥加密 share, 或
        // 2. 使用接收方公钥加密 share (推荐), 或
        // 3. 依赖 RA-TLS 传输层加密, share 本身不加密
        //
        // TODO: 实现基于接收方公钥的 share 加密方案
        let mut all_shares = vec![local_share];
        for es in &peer_encrypted {
            let share = shamir::decrypt_share(es, &seal_key)
                .map_err(|e| BotError::EnclaveError(format!(
                    "peer share decrypt failed (possible key mismatch — \
                     see share_recovery.rs H2 comment): {}", e
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

    // ⚠️ 安全: 使用 Zeroizing 包裹环境变量中读取的 token, 确保 drop 时清零
    // 避免 token.clone() 产生未清零的 String 副本
    let mut primary_token: Option<Zeroizing<String>>;

    if config.needs_telegram {
        let token = std::env::var("BOT_TOKEN")
            .map_err(|_| BotError::Config("BOT_TOKEN required (no share available)".into()))?;
        primary_token = Some(Zeroizing::new(token.clone()));
        vault.set_telegram_token(token);
    } else {
        primary_token = None;
    }

    if config.needs_discord {
        let token = std::env::var("DISCORD_BOT_TOKEN")
            .map_err(|_| BotError::Config("DISCORD_BOT_TOKEN required (no share available)".into()))?;
        if primary_token.is_none() {
            primary_token = Some(Zeroizing::new(token.clone()));
        }
        vault.set_discord_token(token);
    }

    // 实际签名密钥由 EnclaveBridge 管理, 这里存零值表示不覆盖
    let signing_key = Zeroizing::new([0u8; 32]);

    // ── Auto-seal: 将 Token 自动保存为 Shamir share ──
    // 下次启动时 recover_token() 会直接从 share 恢复, 不再需要环境变量
    if let Some(ref token) = primary_token {
        match auto_seal_token(enclave, token.as_str(), &signing_key) {
            Ok(()) => {
                info!("✅ Token 已自动密封为 Shamir share (下次启动将从 share 恢复)");
                warn!("⚠️  请在下次部署时移除 BOT_TOKEN / DISCORD_BOT_TOKEN 环境变量");
            }
            Err(e) => {
                warn!(error = %e, "auto-seal 失败, 下次启动仍需环境变量");
            }
        }
    }
    // primary_token (Zeroizing<String>) 在此作用域结束后自动清零

    // ⚠️ 安全: 清除环境变量中的 token 明文, 防止通过 /proc/<pid>/environ 读取
    // 注意: std::env::remove_var 仅清除进程内 env, 不影响父进程
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
    // 检查是否已有 share (避免覆盖 Ceremony 产出的 share)
    if let Ok(Some(_)) = enclave.load_local_share() {
        info!("本地已有 share, 跳过 auto-seal");
        return Ok(());
    }

    // K=1, N=1: 单节点 auto-seal (过渡用途)
    create_and_save_share(enclave, token, signing_key, 1, 1, 0)?;
    Ok(())
}

/// 执行 Ceremony 后保存 share (由 Ceremony 端点调用)
///
/// 将 bot_token + signing_key 编码为 secrets, 分片, 加密并保存本地 share
pub fn create_and_save_share(
    enclave: &Arc<EnclaveBridge>,
    bot_token: &str,
    signing_key: &[u8; 32],
    k: u8,
    n: u8,
    local_share_index: usize,
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

    let seal_key = enclave.seal_key();
    let encrypted = shamir::encrypt_share(&shares[local_share_index], &seal_key)
        .map_err(|e| BotError::EnclaveError(format!("encrypt share: {}", e)))?;

    enclave.save_local_share(&encrypted)?;

    info!(
        k = k, n = n, share_id = shares[local_share_index].id,
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
        }
    }

    #[tokio::test]
    async fn create_and_recover_share() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let enclave = make_enclave(path);

        let token = "123456:ABCDEF_token";
        let sk = [0x42u8; 32];
        create_and_save_share(&enclave, token, &sk, 1, 1, 0).unwrap();

        let config = default_config(1, true, false);
        let result = recover_token(&enclave, &config).await.unwrap();
        assert_eq!(result.source, RecoverySource::LocalShare);
        assert!(result.vault.has_telegram_token());

        let url = result.vault.build_tg_api_url("getMe").unwrap();
        assert!(url.contains("123456:ABCDEF_token"));
        assert_eq!(&*result.signing_key, &sk);
    }

    #[test]
    fn no_share_falls_back_to_env() {
        // 直接测试 try_env_fallback (同步, 避免与 async 测试的 env var 竞争)
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let enclave = make_enclave(path);

        std::env::set_var("BOT_TOKEN", "env_test_token:123");

        let config = default_config(1, true, false);
        let result = try_env_fallback(&enclave, &config).unwrap();
        assert_eq!(result.source, RecoverySource::EnvironmentVariable);
        assert!(result.vault.has_telegram_token());

        // ── 附带测试 auto-seal → share 恢复 (在同一线程内顺序执行, 无竞争) ──
        // auto_seal 应该已经保存了 share
        assert!(enclave.load_local_share().unwrap().is_some());

        // 清除 env, 从 sealed share 恢复
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
        let result = create_and_save_share(&enclave, "token", &[0u8; 32], 2, 3, 5);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn encode_split_save_load_recover_decode() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let enclave = make_enclave(path);

        let token = "my_bot:secret_token_789";
        let sk = [0xABu8; 32];
        create_and_save_share(&enclave, token, &sk, 1, 3, 0).unwrap();

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
        create_and_save_share(&enclave, token, &sk, 2, 3, 0).unwrap();

        // K=2 but no peer endpoints → try_share_recovery must fail
        let config = RecoveryConfig {
            threshold: 2,
            needs_telegram: true,
            needs_discord: false,
            peer_endpoints: vec![], // no peers!
            ceremony_hash: [0u8; 32],
        };
        let result = try_share_recovery(&enclave, &config).await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.err().unwrap());
        assert!(err_msg.contains("no peer_endpoints"), "expected peer_endpoints error, got: {}", err_msg);
    }
}
