use subxt::{OnlineClient, SubstrateConfig};
use subxt_signer::sr25519::Keypair;
use tokio::sync::mpsc;
use tracing::{info, warn};
use zeroize::Zeroize;

use crate::chain::types::PendingActionLog;
use crate::error::{BotError, BotResult};

/// Substrate 链客户端
pub struct ChainClient {
    api: OnlineClient<SubstrateConfig>,
    signer: Keypair,
    /// 动作日志队列发送端
    log_sender: mpsc::Sender<PendingActionLog>,
    /// 动作日志队列接收端 (由 batcher 持有)
    log_receiver: Option<mpsc::Receiver<PendingActionLog>>,
}

impl ChainClient {
    /// 连接到链
    pub async fn connect(rpc_url: &str, signer: Keypair) -> BotResult<Self> {
        info!(url = rpc_url, "正在连接到 Substrate 链...");
        let api = OnlineClient::<SubstrateConfig>::from_url(rpc_url)
            .await
            .map_err(|e| BotError::ChainConnection(format!("{}", e)))?;

        let chain = api.genesis_hash();
        let runtime_version = api.runtime_version();
        info!(
            genesis = ?chain,
            spec_version = runtime_version.spec_version,
            "链连接成功"
        );

        let (tx, rx) = mpsc::channel(1024);

        Ok(Self {
            api,
            signer,
            log_sender: tx,
            log_receiver: Some(rx),
        })
    }

    /// 获取 subxt API 引用
    pub fn api(&self) -> &OnlineClient<SubstrateConfig> {
        &self.api
    }

    /// 获取签名者引用
    pub fn signer(&self) -> &Keypair {
        &self.signer
    }

    /// 获取日志发送端克隆
    pub fn log_sender(&self) -> mpsc::Sender<PendingActionLog> {
        self.log_sender.clone()
    }

    /// 取出日志接收端 (仅一次)
    pub fn take_log_receiver(&mut self) -> Option<mpsc::Receiver<PendingActionLog>> {
        self.log_receiver.take()
    }
}

/// 加载或生成节点签名密钥
pub fn load_or_generate_signer(data_dir: &str, seed: Option<&str>) -> Keypair {
    // 如果提供了种子
    if let Some(seed_hex) = seed {
        match hex::decode(seed_hex.strip_prefix("0x").unwrap_or(seed_hex)) {
            Ok(mut bytes) => {
                if bytes.len() == 32 {
                    let mut s = [0u8; 32];
                    s.copy_from_slice(&bytes);
                    bytes.zeroize();
                    let result = Keypair::from_secret_key(s);
                    s.zeroize();
                    if let Ok(kp) = result {
                        info!("链上签名密钥从种子加载");
                        return kp;
                    }
                    warn!("提供的 CHAIN_SIGNER_SEED 无法生成密钥对，回退到文件/生成");
                } else {
                    warn!(len = bytes.len(), "提供的 CHAIN_SIGNER_SEED 长度不是 32 字节，回退到文件/生成");
                    bytes.zeroize();
                }
            }
            Err(e) => {
                warn!(error = %e, "提供的 CHAIN_SIGNER_SEED 不是有效 hex，回退到文件/生成");
            }
        }
    }

    // 尝试从文件加载
    let key_path = std::path::Path::new(data_dir).join("chain_signer.key");
    if key_path.exists() {
        if let Ok(mut seed_bytes) = std::fs::read(&key_path) {
            if seed_bytes.len() == 32 {
                let mut seed = [0u8; 32];
                seed.copy_from_slice(&seed_bytes);
                seed_bytes.zeroize();
                let result = Keypair::from_secret_key(seed);
                seed.zeroize();
                if let Ok(kp) = result {
                    info!("链上签名密钥已从文件加载");
                    return kp;
                }
            } else {
                seed_bytes.zeroize();
            }
        }
    }

    // 生成新密钥
    let mut seed = [0u8; 32];
    use rand::RngCore;
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let kp = Keypair::from_secret_key(seed).expect("有效的随机种子");

    std::fs::create_dir_all(data_dir).ok();
    if let Err(e) = std::fs::write(&key_path, &seed) {
        warn!(error = %e, "链上签名密钥持久化失败");
    } else {
        // 限制密钥文件权限为 0o600 (仅 owner 可读写)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600));
        }
        info!("已生成并保存新的链上签名密钥");
    }
    seed.zeroize();
    kp
}
