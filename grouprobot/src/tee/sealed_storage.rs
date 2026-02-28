use std::path::PathBuf;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use aes_gcm::aead::Aead;
use sha2::{Sha256, Digest};
use tracing::{info, warn, debug};
use crate::error::{BotError, BotResult};

/// 密封策略: 控制密钥绑定到 MRENCLAVE (代码度量) 还是 MRSIGNER (签名者度量)
///
/// - `MrEnclave`: 最高安全性, 代码任何变更导致密钥失效, 旧密封文件不可读
/// - `MrSigner`:  跨版本兼容, 只要 Gramine 签名密钥不变, 升级后仍可解密
/// - `DualKey`:   写入用 MRSIGNER, 读取先尝试 MRSIGNER 再 fallback MRENCLAVE
///                推荐生产环境使用 — 兼顾升级兼容性和安全性
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealPolicy {
    MrEnclave,
    MrSigner,
    DualKey,
}

impl SealPolicy {
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "mrenclave" | "enclave" => Self::MrEnclave,
            "mrsigner" | "signer" => Self::MrSigner,
            "dual" | "dualkey" | "dual_key" => Self::DualKey,
            _ => Self::DualKey,
        }
    }
}

impl std::fmt::Display for SealPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MrEnclave => write!(f, "mrenclave"),
            Self::MrSigner => write!(f, "mrsigner"),
            Self::DualKey => write!(f, "dual"),
        }
    }
}

/// V1 密封格式中的密钥类型标识
const KEY_TYPE_MRENCLAVE: u8 = 0;
const KEY_TYPE_MRSIGNER: u8 = 1;

/// V1 密封格式版本标记 (首字节)
const SEALED_FORMAT_V1: u8 = 0x01;

/// AES-256-GCM 密封存储 (双密钥架构)
///
/// 支持 MRENCLAVE 和 MRSIGNER 两种密钥同时存在:
/// - MRENCLAVE 密钥: 绑定代码度量, 代码变更即失效 (最高安全性)
/// - MRSIGNER 密钥:  绑定签名者, Gramine 签名密钥不变即跨版本兼容
///
/// 文件格式:
/// - V0 (旧格式): `[nonce:12][ciphertext]`  — 仅 MRENCLAVE 密钥
/// - V1 (新格式): `[0x01][key_type:1][nonce:12][ciphertext]` — 标识密钥类型
pub struct SealedStorage {
    data_dir: PathBuf,
    policy: SealPolicy,
    mrenclave_cipher: Option<Aes256Gcm>,
    mrsigner_cipher: Option<Aes256Gcm>,
    software_cipher: Option<Aes256Gcm>,
}

impl SealedStorage {
    /// 兼容旧接口: 等同于 `new_with_policy(data_dir, is_hardware, SealPolicy::DualKey)`
    #[allow(dead_code)]
    pub fn new(data_dir: &str, is_hardware: bool) -> Result<Self, BotError> {
        Self::new_with_policy(data_dir, is_hardware, SealPolicy::DualKey)
    }

    /// 使用指定策略创建密封存储
    pub fn new_with_policy(
        data_dir: &str,
        is_hardware: bool,
        policy: SealPolicy,
    ) -> Result<Self, BotError> {
        let (mrenclave_cipher, mrsigner_cipher, software_cipher) = if is_hardware {
            let mrenclave = Self::derive_hardware_key_mrenclave().ok();
            let mrsigner = Self::derive_hardware_key_mrsigner().ok();

            if mrenclave.is_none() && mrsigner.is_none() {
                return Err(BotError::EnclaveError(
                    "硬件密钥读取失败: MRENCLAVE 和 MRSIGNER seal key 均不可用".into(),
                ));
            }

            (
                mrenclave.map(|k| Aes256Gcm::new(&k.into())),
                mrsigner.map(|k| Aes256Gcm::new(&k.into())),
                None,
            )
        } else {
            let sw_key = Self::derive_software_key();
            (None, None, Some(Aes256Gcm::new(&sw_key.into())))
        };

        Ok(Self {
            data_dir: PathBuf::from(data_dir),
            policy,
            mrenclave_cipher,
            mrsigner_cipher,
            software_cipher,
        })
    }

    // ═══════════════════════════════════════════════════════════════
    // 密钥派生
    // ═══════════════════════════════════════════════════════════════

    /// MRENCLAVE-bound 硬件密钥 (代码度量绑定, 代码变更即失效)
    fn derive_hardware_key_mrenclave() -> Result<[u8; 32], BotError> {
        if let Ok(hw_key) = std::fs::read("/dev/attestation/keys/_sgx_mrenclave") {
            if hw_key.len() >= 16 {
                let mut hasher = Sha256::new();
                hasher.update(b"grouprobot-sealed-storage-hw-v1:");
                hasher.update(&hw_key);
                let result = hasher.finalize();
                let mut key = [0u8; 32];
                key.copy_from_slice(&result);
                return Ok(key);
            }
        }
        if let Ok(quote) = std::fs::read("/dev/attestation/quote") {
            if quote.len() >= 232 {
                let mut hasher = Sha256::new();
                hasher.update(b"grouprobot-sealed-storage-mrtd-v1:");
                hasher.update(&quote[184..232]);
                let result = hasher.finalize();
                let mut key = [0u8; 32];
                key.copy_from_slice(&result);
                return Ok(key);
            }
        }
        Err(BotError::EnclaveError(
            "MRENCLAVE seal key 不可用: 无法从 SGX seal key 或 TDX MRTD 派生".into(),
        ))
    }

    /// MRSIGNER-bound 硬件密钥 (签名者绑定, 跨版本兼容)
    fn derive_hardware_key_mrsigner() -> Result<[u8; 32], BotError> {
        if let Ok(hw_key) = std::fs::read("/dev/attestation/keys/_sgx_mrsigner") {
            if hw_key.len() >= 16 {
                let mut hasher = Sha256::new();
                hasher.update(b"grouprobot-sealed-storage-hw-mrsigner-v1:");
                hasher.update(&hw_key);
                let result = hasher.finalize();
                let mut key = [0u8; 32];
                key.copy_from_slice(&result);
                return Ok(key);
            }
        }
        Err(BotError::EnclaveError(
            "MRSIGNER seal key 不可用: /dev/attestation/keys/_sgx_mrsigner 无法读取".into(),
        ))
    }

    /// Software 模式密钥 (不安全, 仅开发/测试)
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

    // ═══════════════════════════════════════════════════════════════
    // 密封 (写入)
    // ═══════════════════════════════════════════════════════════════

    /// 密封数据到文件 (根据 policy 选择密钥, 使用 V1 格式)
    pub fn seal(&self, name: &str, data: &[u8]) -> BotResult<()> {
        let cipher = self.write_cipher()?;
        let key_type = self.write_key_type();

        let mut nonce_bytes = [0u8; 12];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, data)
            .map_err(|e| BotError::EnclaveError(format!("seal encrypt failed: {}", e)))?;

        // V1 格式: [0x01][key_type:1][nonce:12][ciphertext]
        let mut output = Vec::with_capacity(2 + 12 + ciphertext.len());
        output.push(SEALED_FORMAT_V1);
        output.push(key_type);
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);

        let path = self.data_dir.join(name);
        std::fs::write(&path, &output)
            .map_err(|e| BotError::EnclaveError(format!("seal write failed: {}", e)))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&path, perms)
                .map_err(|e| BotError::EnclaveError(format!("seal chmod failed: {}", e)))?;
        }

        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════
    // 解封 (读取, 含版本自动检测)
    // ═══════════════════════════════════════════════════════════════

    /// 解封数据 (自动检测 V0/V1 格式, 多密钥 fallback)
    pub fn unseal(&self, name: &str) -> BotResult<Vec<u8>> {
        let path = self.data_dir.join(name);
        let content = std::fs::read(&path)
            .map_err(|e| BotError::EnclaveError(format!("unseal read failed: {}", e)))?;

        if content.len() < 14 {
            return Err(BotError::EnclaveError("sealed data too short".into()));
        }

        if content[0] == SEALED_FORMAT_V1 && content.len() >= 14 {
            self.unseal_v1(&content)
        } else {
            self.unseal_v0(&content, name)
        }
    }

    /// 解封 V1 格式: [0x01][key_type:1][nonce:12][ciphertext]
    fn unseal_v1(&self, content: &[u8]) -> BotResult<Vec<u8>> {
        let key_type = content[1];
        let nonce = Nonce::from_slice(&content[2..14]);
        let ciphertext = &content[14..];

        let cipher = match key_type {
            KEY_TYPE_MRENCLAVE => self.mrenclave_cipher.as_ref()
                .or(self.software_cipher.as_ref())
                .ok_or_else(|| BotError::EnclaveError(
                    "V1 MRENCLAVE 密钥不可用 (代码已更新?)".into(),
                ))?,
            KEY_TYPE_MRSIGNER => self.mrsigner_cipher.as_ref()
                .or(self.software_cipher.as_ref())
                .ok_or_else(|| BotError::EnclaveError(
                    "V1 MRSIGNER 密钥不可用".into(),
                ))?,
            _ => return Err(BotError::EnclaveError(
                format!("未知 V1 key_type: {}", key_type),
            )),
        };

        cipher.decrypt(nonce, ciphertext)
            .map_err(|e| BotError::EnclaveError(format!("unseal V1 decrypt failed: {}", e)))
    }

    /// 解封 V0 格式: [nonce:12][ciphertext] — 多密钥 fallback 尝试
    fn unseal_v0(&self, content: &[u8], name: &str) -> BotResult<Vec<u8>> {
        let nonce = Nonce::from_slice(&content[..12]);
        let ciphertext = &content[12..];

        // 按优先级尝试所有可用密钥
        let ciphers: Vec<(&str, &Aes256Gcm)> = [
            ("mrenclave", self.mrenclave_cipher.as_ref()),
            ("mrsigner", self.mrsigner_cipher.as_ref()),
            ("software", self.software_cipher.as_ref()),
        ]
            .into_iter()
            .filter_map(|(label, c)| c.map(|c| (label, c)))
            .collect();

        for (label, cipher) in &ciphers {
            if let Ok(plaintext) = cipher.decrypt(nonce, ciphertext) {
                debug!(file = name, key = label, "V0 文件用 {} 密钥解密成功", label);
                return Ok(plaintext);
            }
        }

        Err(BotError::EnclaveError(format!(
            "unseal V0 failed: 所有可用密钥均无法解密 '{}' (代码已更新?)", name
        )))
    }

    // ═══════════════════════════════════════════════════════════════
    // V0 → V1 迁移
    // ═══════════════════════════════════════════════════════════════

    /// 将 V0 格式文件迁移到 V1 格式 (使用当前 write policy 的密钥重新密封)
    ///
    /// 返回 `Ok(true)` 表示已迁移, `Ok(false)` 表示无需迁移 (已是 V1 或不存在)
    pub fn migrate_to_v1(&self, name: &str) -> BotResult<bool> {
        let path = self.data_dir.join(name);
        if !path.exists() {
            return Ok(false);
        }

        let content = std::fs::read(&path)
            .map_err(|e| BotError::EnclaveError(format!("migrate read failed: {}", e)))?;

        if content.len() < 14 {
            return Ok(false);
        }

        // 已经是 V1 格式
        if content[0] == SEALED_FORMAT_V1 {
            return Ok(false);
        }

        // V0 格式: 尝试解密
        let plaintext = self.unseal_v0(&content, name)?;

        // 备份旧文件
        let backup_path = self.data_dir.join(format!("{}.v0.bak", name));
        if let Err(e) = std::fs::copy(&path, &backup_path) {
            warn!(file = name, error = %e, "V0 备份失败 (继续迁移)");
        }

        // 用当前策略重新密封为 V1
        self.seal(name, &plaintext)?;

        info!(file = name, policy = %self.policy, "密封文件已从 V0 迁移到 V1");
        Ok(true)
    }

    // ═══════════════════════════════════════════════════════════════
    // 辅助方法
    // ═══════════════════════════════════════════════════════════════

    /// 根据策略选择写入密钥
    fn write_cipher(&self) -> BotResult<&Aes256Gcm> {
        if let Some(ref sw) = self.software_cipher {
            return Ok(sw);
        }
        match self.policy {
            SealPolicy::MrEnclave => self.mrenclave_cipher.as_ref(),
            SealPolicy::MrSigner | SealPolicy::DualKey => {
                self.mrsigner_cipher.as_ref().or(self.mrenclave_cipher.as_ref())
            }
        }
        .ok_or_else(|| BotError::EnclaveError("无可用密封密钥".into()))
    }

    /// 根据策略返回写入的 key_type 标识
    fn write_key_type(&self) -> u8 {
        if self.software_cipher.is_some() {
            return KEY_TYPE_MRENCLAVE;
        }
        match self.policy {
            SealPolicy::MrEnclave => KEY_TYPE_MRENCLAVE,
            SealPolicy::MrSigner | SealPolicy::DualKey => {
                if self.mrsigner_cipher.is_some() {
                    KEY_TYPE_MRSIGNER
                } else {
                    KEY_TYPE_MRENCLAVE
                }
            }
        }
    }

    pub fn exists(&self, name: &str) -> bool {
        self.data_dir.join(name).exists()
    }

    pub fn data_dir(&self) -> &str {
        self.data_dir.to_str().unwrap_or(".")
    }

    #[allow(dead_code)]
    pub fn policy(&self) -> SealPolicy {
        self.policy
    }

    /// 是否拥有 MRSIGNER 密钥 (用于判断跨版本兼容能力)
    pub fn has_mrsigner_key(&self) -> bool {
        self.mrsigner_cipher.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_storage(dir: &str) -> SealedStorage {
        SealedStorage::new_with_policy(dir, false, SealPolicy::DualKey).unwrap()
    }

    #[test]
    fn seal_unseal_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let storage = make_storage(dir.path().to_str().unwrap());

        let data = b"hello sealed world";
        storage.seal("test.sealed", data).unwrap();
        let recovered = storage.unseal("test.sealed").unwrap();
        assert_eq!(&recovered, data);
    }

    #[test]
    fn seal_produces_v1_format() {
        let dir = tempfile::tempdir().unwrap();
        let storage = make_storage(dir.path().to_str().unwrap());

        storage.seal("v1test.sealed", b"payload").unwrap();

        let raw = std::fs::read(dir.path().join("v1test.sealed")).unwrap();
        assert_eq!(raw[0], SEALED_FORMAT_V1, "首字节应为 V1 标记");
        assert!(raw[1] == KEY_TYPE_MRENCLAVE || raw[1] == KEY_TYPE_MRSIGNER);
        assert!(raw.len() > 14);
    }

    #[test]
    fn v0_format_still_readable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let storage = make_storage(path);

        // 手工写入 V0 格式: [nonce:12][ciphertext]
        let cipher = storage.write_cipher().unwrap();
        let mut nonce_bytes = [0u8; 12];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, b"v0 data" as &[u8]).unwrap();

        let mut v0_content = Vec::new();
        v0_content.extend_from_slice(&nonce_bytes);
        v0_content.extend_from_slice(&ciphertext);
        std::fs::write(dir.path().join("legacy.sealed"), &v0_content).unwrap();

        let recovered = storage.unseal("legacy.sealed").unwrap();
        assert_eq!(&recovered, b"v0 data");
    }

    #[test]
    fn migrate_v0_to_v1() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let storage = make_storage(path);

        // 写 V0 格式
        let cipher = storage.write_cipher().unwrap();
        let mut nonce_bytes = [0u8; 12];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, b"migrate me" as &[u8]).unwrap();

        let mut v0_content = Vec::new();
        v0_content.extend_from_slice(&nonce_bytes);
        v0_content.extend_from_slice(&ciphertext);
        std::fs::write(dir.path().join("old.sealed"), &v0_content).unwrap();

        // 迁移
        assert!(storage.migrate_to_v1("old.sealed").unwrap());

        // 确认 V1 格式
        let raw = std::fs::read(dir.path().join("old.sealed")).unwrap();
        assert_eq!(raw[0], SEALED_FORMAT_V1);

        // 确认数据完整
        let recovered = storage.unseal("old.sealed").unwrap();
        assert_eq!(&recovered, b"migrate me");

        // 再次迁移应返回 false (已是 V1)
        assert!(!storage.migrate_to_v1("old.sealed").unwrap());
    }

    #[test]
    fn migrate_creates_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let storage = make_storage(path);

        // 写 V0
        let cipher = storage.write_cipher().unwrap();
        let mut nonce_bytes = [0u8; 12];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ct = cipher.encrypt(nonce, b"bak test" as &[u8]).unwrap();
        let mut v0 = Vec::new();
        v0.extend_from_slice(&nonce_bytes);
        v0.extend_from_slice(&ct);
        let original = v0.clone();
        std::fs::write(dir.path().join("bak.sealed"), &v0).unwrap();

        storage.migrate_to_v1("bak.sealed").unwrap();

        let backup = std::fs::read(dir.path().join("bak.sealed.v0.bak")).unwrap();
        assert_eq!(backup, original, "备份应保留原始 V0 内容");
    }

    #[test]
    fn unseal_nonexistent_fails() {
        let dir = tempfile::tempdir().unwrap();
        let storage = make_storage(dir.path().to_str().unwrap());
        assert!(storage.unseal("nonexistent").is_err());
    }

    #[test]
    fn exists_check() {
        let dir = tempfile::tempdir().unwrap();
        let storage = make_storage(dir.path().to_str().unwrap());
        assert!(!storage.exists("foo"));
        storage.seal("foo", b"bar").unwrap();
        assert!(storage.exists("foo"));
    }

    #[test]
    fn different_data_different_ciphertext() {
        let dir = tempfile::tempdir().unwrap();
        let storage = make_storage(dir.path().to_str().unwrap());
        storage.seal("a.sealed", b"aaa").unwrap();
        storage.seal("b.sealed", b"bbb").unwrap();
        let a = std::fs::read(dir.path().join("a.sealed")).unwrap();
        let b = std::fs::read(dir.path().join("b.sealed")).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn seal_policy_from_str() {
        assert_eq!(SealPolicy::from_str_lossy("mrenclave"), SealPolicy::MrEnclave);
        assert_eq!(SealPolicy::from_str_lossy("mrsigner"), SealPolicy::MrSigner);
        assert_eq!(SealPolicy::from_str_lossy("dual"), SealPolicy::DualKey);
        assert_eq!(SealPolicy::from_str_lossy("unknown"), SealPolicy::DualKey);
    }

    #[test]
    fn migrate_nonexistent_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        let storage = make_storage(dir.path().to_str().unwrap());
        assert!(!storage.migrate_to_v1("nope.sealed").unwrap());
    }

    #[test]
    fn all_policies_roundtrip() {
        for policy in [SealPolicy::MrEnclave, SealPolicy::MrSigner, SealPolicy::DualKey] {
            let dir = tempfile::tempdir().unwrap();
            let storage = SealedStorage::new_with_policy(
                dir.path().to_str().unwrap(), false, policy,
            ).unwrap();

            let data = b"test data for each policy";
            storage.seal("test.sealed", data).unwrap();
            let recovered = storage.unseal("test.sealed").unwrap();
            assert_eq!(&recovered, data, "policy {:?} roundtrip failed", policy);
        }
    }
}
