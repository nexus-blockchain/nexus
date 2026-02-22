use std::path::PathBuf;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use aes_gcm::aead::Aead;
use sha2::{Sha256, Digest};
use tracing::warn;

use crate::error::{BotError, BotResult};

/// AES-256-GCM 密封存储
/// 使用从机器/硬件标识派生的密钥加密本地文件
///
/// Hardware 模式: 密钥绑定 TDX 硬件 (MRENCLAVE/MRTD), 修改代码 → 无法解密
/// Software 模式: 密钥从 HOSTNAME + machine-id 派生 (不安全, 仅开发用)
pub struct SealedStorage {
    data_dir: PathBuf,
    cipher: Aes256Gcm,
}

impl SealedStorage {
    pub fn new(data_dir: &str) -> Self {
        let is_hardware = std::path::Path::new("/dev/attestation/quote").exists();
        let key = if is_hardware {
            Self::derive_hardware_key()
        } else {
            Self::derive_software_key()
        };
        let cipher = Aes256Gcm::new(&key.into());
        Self {
            data_dir: PathBuf::from(data_dir),
            cipher,
        }
    }

    /// Hardware 模式: 从 TDX/SGX 硬件密钥派生
    ///
    /// 使用 Gramine 暴露的 SGX seal key (/dev/attestation/keys/_sgx_mrenclave)
    /// 此密钥由 CPU 微码绑定到 MRENCLAVE, 修改代码后密钥不同 → 无法解密旧数据
    fn derive_hardware_key() -> [u8; 32] {
        // 优先: Gramine SGX MRENCLAVE-bound seal key
        if let Ok(hw_key) = std::fs::read("/dev/attestation/keys/_sgx_mrenclave") {
            if hw_key.len() >= 16 {
                let mut hasher = Sha256::new();
                hasher.update(b"grouprobot-sealed-storage-hw-v1:");
                hasher.update(&hw_key);
                let result = hasher.finalize();
                let mut key = [0u8; 32];
                key.copy_from_slice(&result);
                return key;
            }
        }
        // 回退: 从 TDX quote 的 MRTD 派生
        if let Ok(quote) = std::fs::read("/dev/attestation/quote") {
            if quote.len() >= 232 {
                let mut hasher = Sha256::new();
                hasher.update(b"grouprobot-sealed-storage-mrtd-v1:");
                hasher.update(&quote[184..232]); // MRTD 48 bytes
                let result = hasher.finalize();
                let mut key = [0u8; 32];
                key.copy_from_slice(&result);
                return key;
            }
        }
        warn!("⚠️ 硬件密钥读取失败, 回退到软件模式密钥派生");
        Self::derive_software_key()
    }

    /// Software 模式: 从机器标识派生密钥 (⚠️ 不安全, 仅开发/测试)
    fn derive_software_key() -> [u8; 32] {
        let mut hasher = Sha256::new();
        if let Ok(hostname) = std::env::var("HOSTNAME") {
            hasher.update(hostname.as_bytes());
        }
        hasher.update(b"grouprobot-seal-key-v1");
        if let Ok(machine_id) = std::fs::read_to_string("/etc/machine-id") {
            hasher.update(machine_id.trim().as_bytes());
        }
        let result = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&result);
        key
    }

    /// 密封数据到文件
    pub fn seal(&self, name: &str, data: &[u8]) -> BotResult<()> {
        let path = self.data_dir.join(name);

        // 生成随机 nonce
        let mut nonce_bytes = [0u8; 12];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self.cipher.encrypt(nonce, data)
            .map_err(|e| BotError::EnclaveError(format!("seal encrypt failed: {}", e)))?;

        // 格式: [12 bytes nonce][ciphertext]
        let mut output = Vec::with_capacity(12 + ciphertext.len());
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);

        std::fs::write(&path, &output)
            .map_err(|e| BotError::EnclaveError(format!("seal write failed: {}", e)))?;

        Ok(())
    }

    /// 解封数据
    pub fn unseal(&self, name: &str) -> BotResult<Vec<u8>> {
        let path = self.data_dir.join(name);

        let content = std::fs::read(&path)
            .map_err(|e| BotError::EnclaveError(format!("unseal read failed: {}", e)))?;

        if content.len() < 12 {
            return Err(BotError::EnclaveError("sealed data too short".into()));
        }

        let nonce = Nonce::from_slice(&content[..12]);
        let ciphertext = &content[12..];

        let plaintext = self.cipher.decrypt(nonce, ciphertext)
            .map_err(|e| BotError::EnclaveError(format!("unseal decrypt failed: {}", e)))?;

        Ok(plaintext)
    }

    /// 检查密封文件是否存在
    #[allow(dead_code)]
    pub fn exists(&self, name: &str) -> bool {
        self.data_dir.join(name).exists()
    }

    /// 获取数据目录路径
    pub fn data_dir(&self) -> &str {
        self.data_dir.to_str().unwrap_or(".")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_unseal_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let storage = SealedStorage::new(dir.path().to_str().unwrap());

        let data = b"hello sealed world";
        storage.seal("test.sealed", data).unwrap();
        let recovered = storage.unseal("test.sealed").unwrap();
        assert_eq!(&recovered, data);
    }

    #[test]
    fn unseal_nonexistent_fails() {
        let dir = tempfile::tempdir().unwrap();
        let storage = SealedStorage::new(dir.path().to_str().unwrap());
        assert!(storage.unseal("nonexistent").is_err());
    }

    #[test]
    fn exists_check() {
        let dir = tempfile::tempdir().unwrap();
        let storage = SealedStorage::new(dir.path().to_str().unwrap());
        assert!(!storage.exists("foo"));
        storage.seal("foo", b"bar").unwrap();
        assert!(storage.exists("foo"));
    }

    #[test]
    fn different_data_different_ciphertext() {
        let dir = tempfile::tempdir().unwrap();
        let storage = SealedStorage::new(dir.path().to_str().unwrap());
        storage.seal("a.sealed", b"aaa").unwrap();
        storage.seal("b.sealed", b"bbb").unwrap();
        let a = std::fs::read(dir.path().join("a.sealed")).unwrap();
        let b = std::fs::read(dir.path().join("b.sealed")).unwrap();
        assert_ne!(a, b);
    }
}
