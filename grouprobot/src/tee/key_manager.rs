use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use sha2::{Sha256, Digest};
use tracing::{info, warn};

use crate::error::{BotError, BotResult};
use crate::tee::enclave_bridge::EnclaveBridge;

/// Ed25519 密钥管理器 (通过 EnclaveBridge 签名)
pub struct KeyManager {
    enclave: Arc<EnclaveBridge>,
    bot_id_hash: [u8; 32],
}

impl KeyManager {
    pub fn new(enclave: Arc<EnclaveBridge>, bot_id_hash: [u8; 32]) -> Self {
        Self { enclave, bot_id_hash }
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.enclave.public_key_bytes()
    }

    pub fn public_key_hex(&self) -> String {
        self.enclave.public_key_hex()
    }

    pub fn bot_id_hash(&self) -> [u8; 32] {
        self.bot_id_hash
    }

    /// 签名消息 (返回签名 + 消息哈希)
    pub fn sign_message(
        &self,
        bot_id_hash: &[u8; 32],
        sequence: u64,
        timestamp: u64,
        payload: &[u8],
    ) -> ([u8; 64], [u8; 32]) {
        // 构造待签名内容: bot_id_hash || sequence || timestamp || SHA256(payload)
        let mut hasher = Sha256::new();
        hasher.update(payload);
        let payload_hash: [u8; 32] = hasher.finalize().into();

        let mut message = Vec::with_capacity(32 + 8 + 8 + 32);
        message.extend_from_slice(bot_id_hash);
        message.extend_from_slice(&sequence.to_le_bytes());
        message.extend_from_slice(&timestamp.to_le_bytes());
        message.extend_from_slice(&payload_hash);

        let signature = self.enclave.sign(&message);

        // 计算 message_hash
        let mut msg_hasher = Sha256::new();
        msg_hasher.update(&message);
        let message_hash: [u8; 32] = msg_hasher.finalize().into();

        (signature, message_hash)
    }

    /// 验证签名
    pub fn verify_signature(&self, message: &[u8], signature: &[u8; 64]) -> bool {
        self.enclave.verify(message, signature)
    }

    /// P5-fix: 签名广告投放收据 (返回 64 字节 Ed25519 签名)
    pub fn sign_receipt(&self, message: &[u8]) -> [u8; 64] {
        self.enclave.sign(message)
    }
}

/// 序列号管理器 (原子递增 + 持久化)
pub struct SequenceManager {
    current: AtomicU64,
    data_dir: String,
}

impl SequenceManager {
    pub fn load_or_init(data_dir: &str) -> BotResult<Self> {
        let path = std::path::Path::new(data_dir).join("sequence.dat");
        let current = if path.exists() {
            let bytes = std::fs::read(&path)
                .map_err(|e| BotError::Config(format!("read sequence: {}", e)))?;
            if bytes.len() == 8 {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&bytes);
                u64::from_le_bytes(buf)
            } else {
                0
            }
        } else {
            0
        };

        info!(current_seq = current, "序列号管理器就绪");

        Ok(Self {
            current: AtomicU64::new(current),
            data_dir: data_dir.to_string(),
        })
    }

    pub fn current(&self) -> u64 {
        self.current.load(Ordering::Relaxed)
    }

    pub fn next(&self) -> BotResult<u64> {
        let seq = self.current.fetch_add(1, Ordering::SeqCst) + 1;
        if let Err(e) = self.persist(seq) {
            // 持久化失败 → 回退 atomic counter, 防止重启后序列号重复
            self.current.fetch_sub(1, Ordering::SeqCst);
            return Err(e);
        }
        Ok(seq)
    }

    fn persist(&self, seq: u64) -> BotResult<()> {
        let path = std::path::Path::new(&self.data_dir).join("sequence.dat");
        std::fs::write(&path, seq.to_le_bytes())
            .map_err(|e| {
                warn!(error = %e, "序列号持久化失败");
                BotError::EnclaveError(format!("sequence persist failed: {}", e))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_increment() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = SequenceManager::load_or_init(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(mgr.current(), 0);
        assert_eq!(mgr.next().unwrap(), 1);
        assert_eq!(mgr.next().unwrap(), 2);
        assert_eq!(mgr.current(), 2);
    }

    #[test]
    fn sequence_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        {
            let mgr = SequenceManager::load_or_init(path).unwrap();
            mgr.next().unwrap();
            mgr.next().unwrap();
            mgr.next().unwrap();
        }
        let mgr = SequenceManager::load_or_init(path).unwrap();
        assert_eq!(mgr.current(), 3);
    }

    #[test]
    fn key_manager_sign_verify() {
        let dir = tempfile::tempdir().unwrap();
        let enclave = Arc::new(
            EnclaveBridge::init(dir.path().to_str().unwrap(), "software").unwrap()
        );
        let bot_hash = [1u8; 32];
        let km = KeyManager::new(enclave, bot_hash);

        let (sig, msg_hash) = km.sign_message(&bot_hash, 1, 1000, b"test payload");
        assert_eq!(sig.len(), 64);
        assert_eq!(msg_hash.len(), 32);
    }
}
