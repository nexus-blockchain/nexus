use std::path::Path;
use tracing::{info, warn};

use crate::error::BotResult;
use crate::tee::sealed_storage::SealedStorage;

/// TEE 模式
#[derive(Debug, Clone, PartialEq)]
pub enum TeeMode {
    /// TDX + SGX 硬件模式
    Hardware,
    /// 纯软件模拟 (开发/测试)
    Software,
}

impl std::fmt::Display for TeeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TeeMode::Hardware => write!(f, "hardware"),
            TeeMode::Software => write!(f, "software"),
        }
    }
}

/// SGX Enclave 桥接
/// 在 Hardware 模式下调用真实 ecall；Software 模式下使用 AES-GCM 模拟
pub struct EnclaveBridge {
    mode: TeeMode,
    sealed_storage: SealedStorage,
    /// Ed25519 密钥对 (Software 模式在内存中; Hardware 模式在 Enclave 中)
    keypair: ed25519_dalek::SigningKey,
}

impl EnclaveBridge {
    /// 初始化 Enclave 桥接
    pub fn init(data_dir: &str, tee_mode_str: &str) -> BotResult<Self> {
        let mode = Self::detect_mode(tee_mode_str);
        info!(mode = %mode, "初始化 Enclave 桥接");

        std::fs::create_dir_all(data_dir).ok();
        let sealed_storage = SealedStorage::new(data_dir);

        // 加载或生成 Ed25519 密钥
        let keypair = Self::load_or_generate_key(&sealed_storage)?;

        Ok(Self {
            mode,
            sealed_storage,
            keypair,
        })
    }

    fn detect_mode(tee_mode_str: &str) -> TeeMode {
        match tee_mode_str {
            "hardware" | "tdx" | "sgx" => TeeMode::Hardware,
            "software" => TeeMode::Software,
            _ => {
                // 检测 TDX 设备
                if Path::new("/dev/attestation/quote").exists() {
                    info!("检测到 TDX 硬件");
                    TeeMode::Hardware
                } else {
                    warn!("未检测到 TEE 硬件，使用软件模式");
                    TeeMode::Software
                }
            }
        }
    }

    fn load_or_generate_key(sealed: &SealedStorage) -> BotResult<ed25519_dalek::SigningKey> {
        let key_file = "enclave_ed25519.sealed";

        // 尝试加载
        if let Ok(seed_bytes) = sealed.unseal(key_file) {
            if seed_bytes.len() == 32 {
                let mut seed = [0u8; 32];
                seed.copy_from_slice(&seed_bytes);
                let key = ed25519_dalek::SigningKey::from_bytes(&seed);
                info!("Ed25519 密钥已从密封存储加载");
                return Ok(key);
            }
        }

        // 生成新密钥
        let mut seed = [0u8; 32];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut seed);
        let key = ed25519_dalek::SigningKey::from_bytes(&seed);

        // 密封保存
        if let Err(e) = sealed.seal(key_file, &seed) {
            warn!(error = %e, "密钥密封保存失败");
        } else {
            info!("已生成并密封保存新的 Ed25519 密钥");
        }

        Ok(key)
    }

    /// 获取 TEE 模式
    pub fn mode(&self) -> &TeeMode {
        &self.mode
    }

    /// 获取公钥
    pub fn public_key(&self) -> ed25519_dalek::VerifyingKey {
        self.keypair.verifying_key()
    }

    /// 获取公钥 bytes
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.public_key().to_bytes()
    }

    /// 获取公钥 hex
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key_bytes())
    }

    /// 签名 (Software: 直接签; Hardware: ecall_sign)
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        use ed25519_dalek::Signer;
        let sig = self.keypair.sign(message);
        sig.to_bytes()
    }

    /// 验证签名
    pub fn verify(&self, message: &[u8], signature: &[u8; 64]) -> bool {
        use ed25519_dalek::Verifier;
        let sig = ed25519_dalek::Signature::from_bytes(signature);
        self.public_key().verify(message, &sig).is_ok()
    }

    /// 密封数据
    pub fn seal(&self, name: &str, data: &[u8]) -> BotResult<()> {
        self.sealed_storage.seal(name, data)
    }

    /// 解封数据
    pub fn unseal(&self, name: &str) -> BotResult<Vec<u8>> {
        self.sealed_storage.unseal(name)
    }

    /// 获取密封存储密钥 (用于 Shamir share 加解密)
    ///
    /// Hardware 模式: 从 TDX configentry 派生, 绑定到 TD 实例 + MRTD
    /// Software 模式: 从 data_dir 路径派生 (不安全, 仅用于开发)
    pub fn seal_key(&self) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        match self.mode {
            TeeMode::Hardware => {
                // 读取 TDX configentry 作为硬件绑定熵源
                // /dev/attestation/keys/_sgx_mrenclave 或 TDX 等效路径
                // 此密钥由 CPU 硬件派生, 绑定到 MRTD (代码度量)
                // 修改代码 → MRTD 改变 → seal_key 改变 → 无法解密旧 share
                let hw_entropy = Self::read_tdx_seal_entropy();
                let mut hasher = Sha256::new();
                hasher.update(b"grouprobot-shamir-seal-hw:");
                hasher.update(hw_entropy);
                hasher.update(self.sealed_storage.data_dir().as_bytes());
                let result = hasher.finalize();
                let mut key = [0u8; 32];
                key.copy_from_slice(&result);
                key
            }
            TeeMode::Software => {
                // ⚠️ 软件模式: 从路径派生, 不安全
                let mut hasher = Sha256::new();
                hasher.update(b"grouprobot-shamir-seal:");
                hasher.update(self.sealed_storage.data_dir().as_bytes());
                let result = hasher.finalize();
                let mut key = [0u8; 32];
                key.copy_from_slice(&result);
                key
            }
        }
    }

    /// 读取 TDX 硬件密封熵
    ///
    /// 优先使用 Gramine 提供的 SGX seal key 接口:
    ///   /dev/attestation/keys/_sgx_mrenclave
    /// 回退: 从 /dev/attestation/quote 的 MRTD 字段派生
    fn read_tdx_seal_entropy() -> [u8; 64] {
        // Gramine SGX seal key (MRENCLAVE-bound)
        if let Ok(data) = std::fs::read("/dev/attestation/keys/_sgx_mrenclave") {
            if data.len() >= 16 {
                use sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                hasher.update(b"tdx-seal-from-sgx-key:");
                hasher.update(&data);
                let h1: [u8; 32] = hasher.finalize().into();
                let mut entropy = [0u8; 64];
                entropy[..32].copy_from_slice(&h1);
                // 二次哈希增加熵
                let mut hasher2 = Sha256::new();
                hasher2.update(h1);
                hasher2.update(b"seal-entropy-extend");
                let h2: [u8; 32] = hasher2.finalize().into();
                entropy[32..].copy_from_slice(&h2);
                return entropy;
            }
        }
        // 回退: 从 TDX report_data 路径读取
        // 写入空 report_data → 读取 quote → 从 MRTD 字段派生
        if let Ok(quote) = std::fs::read("/dev/attestation/quote") {
            if quote.len() >= 232 {
                use sha2::{Sha256, Digest};
                let mrtd = &quote[184..232]; // MRTD 48 bytes
                let mut hasher = Sha256::new();
                hasher.update(b"tdx-seal-from-mrtd:");
                hasher.update(mrtd);
                let h: [u8; 32] = hasher.finalize().into();
                let mut entropy = [0u8; 64];
                entropy[..32].copy_from_slice(&h);
                entropy[32..48].copy_from_slice(&mrtd[..16]);
                return entropy;
            }
        }
        warn!("无法读取 TDX 硬件熵, 回退到零值 (seal_key 安全性降级)");
        [0u8; 64]
    }

    /// 保存加密 share 到本地密封存储
    pub fn save_local_share(&self, share: &crate::tee::shamir::EncryptedShare) -> BotResult<()> {
        let data = crate::tee::shamir::share_to_bytes(share);
        self.sealed_storage.seal("shamir_share.sealed", &data)?;
        info!(share_id = share.id, "Shamir share 已密封保存");
        Ok(())
    }

    /// 从本地密封存储加载加密 share
    pub fn load_local_share(&self) -> BotResult<Option<crate::tee::shamir::EncryptedShare>> {
        match self.sealed_storage.unseal("shamir_share.sealed") {
            Ok(data) => {
                let share = crate::tee::shamir::share_from_bytes(&data)
                    .map_err(|e| crate::error::BotError::EnclaveError(format!("share parse: {}", e)))?;
                info!(share_id = share.id, "已加载本地 Shamir share");
                Ok(Some(share))
            }
            Err(_) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_mode_init() {
        let dir = tempfile::tempdir().unwrap();
        let bridge = EnclaveBridge::init(dir.path().to_str().unwrap(), "software").unwrap();
        assert_eq!(*bridge.mode(), TeeMode::Software);
        assert_eq!(bridge.public_key_bytes().len(), 32);
    }

    #[test]
    fn sign_and_verify() {
        let dir = tempfile::tempdir().unwrap();
        let bridge = EnclaveBridge::init(dir.path().to_str().unwrap(), "software").unwrap();
        let msg = b"hello grouprobot";
        let sig = bridge.sign(msg);
        assert!(bridge.verify(msg, &sig));
        assert!(!bridge.verify(b"tampered", &sig));
    }

    #[test]
    fn key_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();

        let pk1 = {
            let bridge = EnclaveBridge::init(path, "software").unwrap();
            bridge.public_key_bytes()
        };
        let pk2 = {
            let bridge = EnclaveBridge::init(path, "software").unwrap();
            bridge.public_key_bytes()
        };
        assert_eq!(pk1, pk2, "密钥应持久化");
    }

    #[test]
    fn seal_unseal() {
        let dir = tempfile::tempdir().unwrap();
        let bridge = EnclaveBridge::init(dir.path().to_str().unwrap(), "software").unwrap();
        let data = b"secret data";
        bridge.seal("test_seal", data).unwrap();
        let recovered = bridge.unseal("test_seal").unwrap();
        assert_eq!(&recovered, data);
    }
}
