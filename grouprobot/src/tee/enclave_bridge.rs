use std::path::Path;
use tracing::{info, warn};

use crate::error::{BotError, BotResult};
use crate::tee::sealed_storage::{SealedStorage, SealPolicy};

/// TEE 模式 (三态: TDX / SGX / Software)
#[derive(Debug, Clone, PartialEq)]
pub enum TeeMode {
    /// TDX 硬件模式 (含 TDX+SGX 双证明)
    Tdx,
    /// SGX-Only 硬件模式
    Sgx,
    /// 纯软件模拟 (开发/测试)
    Software,
}

impl TeeMode {
    /// 是否为硬件模式 (TDX 或 SGX)
    pub fn is_hardware(&self) -> bool {
        matches!(self, TeeMode::Tdx | TeeMode::Sgx)
    }
}

impl std::fmt::Display for TeeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TeeMode::Tdx => write!(f, "tdx"),
            TeeMode::Sgx => write!(f, "sgx"),
            TeeMode::Software => write!(f, "software"),
        }
    }
}

/// 需要迁移的密封文件列表
const SEALED_FILES: &[&str] = &[
    "enclave_ed25519.sealed",
    "shamir_share.sealed",
    "ceremony_hash.sealed",
];

/// SGX Enclave 桥接
/// 在 Hardware 模式下调用真实 ecall；Software 模式下使用 AES-GCM 模拟
pub struct EnclaveBridge {
    mode: TeeMode,
    sealed_storage: SealedStorage,
    /// Ed25519 密钥对 (Software 模式在内存中; Hardware 模式在 Enclave 中)
    keypair: ed25519_dalek::SigningKey,
}

impl EnclaveBridge {
    /// 初始化 Enclave 桥接 (使用默认 DualKey 策略)
    pub fn init(data_dir: &str, tee_mode_str: &str) -> BotResult<Self> {
        Self::init_with_policy(data_dir, tee_mode_str, SealPolicy::DualKey)
    }

    /// 初始化 Enclave 桥接 (指定密封策略)
    pub fn init_with_policy(
        data_dir: &str,
        tee_mode_str: &str,
        seal_policy: SealPolicy,
    ) -> BotResult<Self> {
        let mode = Self::detect_mode(tee_mode_str);
        info!(mode = %mode, seal_policy = %seal_policy, "初始化 Enclave 桥接");

        std::fs::create_dir_all(data_dir).ok();
        let sealed_storage = SealedStorage::new_with_policy(
            data_dir, mode.is_hardware(), seal_policy,
        )?;

        // V0 → V1 自动迁移: 旧格式密封文件升级为新格式
        Self::run_migration(&sealed_storage);

        let keypair = Self::load_or_generate_key(&sealed_storage)?;

        if sealed_storage.has_mrsigner_key() {
            info!("MRSIGNER 密钥可用 — 跨版本密封兼容已启用");
        } else if mode.is_hardware() {
            warn!("MRSIGNER 密钥不可用 — 升级时密封文件可能不兼容, 需要 Gramine 配置 sgx.seal_key_derivation");
        }

        Ok(Self {
            mode,
            sealed_storage,
            keypair,
        })
    }

    /// 自动迁移所有已知的密封文件 V0 → V1
    fn run_migration(sealed: &SealedStorage) {
        for file in SEALED_FILES {
            match sealed.migrate_to_v1(file) {
                Ok(true) => info!(file, "密封文件已自动迁移 V0 → V1"),
                Ok(false) => {}
                Err(e) => warn!(file, error = %e, "密封文件迁移失败 (非致命, 将在解封时重试)"),
            }
        }
    }

    fn detect_mode(tee_mode_str: &str) -> TeeMode {
        match tee_mode_str {
            "tdx" | "hardware" => TeeMode::Tdx,
            "sgx" => TeeMode::Sgx,
            "software" => TeeMode::Software,
            _ => Self::auto_detect_hardware(),
        }
    }

    /// 自动检测硬件 TEE 类型
    ///
    /// 检测顺序:
    /// 1. /dev/attestation/quote 存在 → 读取 version 字段
    ///    - version=4 → TDX
    ///    - version=3 → SGX
    /// 2. /dev/sgx_enclave 存在 → SGX
    /// 3. 否则 → Software
    fn auto_detect_hardware() -> TeeMode {
        if Path::new("/dev/attestation/quote").exists() {
            if let Ok(quote) = std::fs::read("/dev/attestation/quote") {
                if quote.len() >= 2 {
                    let version = u16::from_le_bytes([quote[0], quote[1]]);
                    return match version {
                        4 => {
                            info!("自动检测: TDX Quote v4");
                            TeeMode::Tdx
                        }
                        3 => {
                            info!("自动检测: SGX Quote v3");
                            TeeMode::Sgx
                        }
                        _ => {
                            info!(version, "自动检测: 未知 Quote 版本, 默认 TDX");
                            TeeMode::Tdx
                        }
                    };
                }
            }
            info!("自动检测: /dev/attestation/quote 存在但无法读取, 默认 TDX");
            TeeMode::Tdx
        } else if Path::new("/dev/sgx_enclave").exists() {
            info!("自动检测: SGX enclave 设备");
            TeeMode::Sgx
        } else {
            warn!("未检测到 TEE 硬件，使用软件模式");
            TeeMode::Software
        }
    }

    fn load_or_generate_key(sealed: &SealedStorage) -> BotResult<ed25519_dalek::SigningKey> {
        let key_file = "enclave_ed25519.sealed";

        if let Ok(seed_bytes) = sealed.unseal(key_file) {
            if seed_bytes.len() == 32 {
                let mut seed = [0u8; 32];
                seed.copy_from_slice(&seed_bytes);
                let key = ed25519_dalek::SigningKey::from_bytes(&seed);
                info!("Ed25519 密钥已从密封存储加载");
                return Ok(key);
            }
        }

        let mut seed = [0u8; 32];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut seed);
        let key = ed25519_dalek::SigningKey::from_bytes(&seed);

        sealed.seal(key_file, &seed).map_err(|e| {
            BotError::EnclaveError(format!("密钥密封保存失败, 中止启动以防密钥丢失: {}", e))
        })?;
        info!("已生成并密封保存新的 Ed25519 密钥");

        Ok(key)
    }

    /// 替换签名密钥 (从迁移恢复时使用, 保持跨版本身份连续性)
    ///
    /// 将旧版本的 Ed25519 seed 导入并用当前策略重新密封, 使节点在升级后
    /// 保持相同的链上身份 (public_key 不变)
    pub fn replace_signing_key(&mut self, seed: [u8; 32]) -> BotResult<()> {
        let old_pk = hex::encode(self.public_key_bytes());
        self.keypair = ed25519_dalek::SigningKey::from_bytes(&seed);
        let new_pk = hex::encode(self.public_key_bytes());

        self.sealed_storage.seal("enclave_ed25519.sealed", &seed).map_err(|e| {
            BotError::EnclaveError(format!("密钥替换密封失败: {}", e))
        })?;

        info!(
            old_pk = %old_pk,
            new_pk = %new_pk,
            "Ed25519 密钥已从迁移恢复并重新密封 (身份已保持)"
        );
        Ok(())
    }

    pub fn mode(&self) -> &TeeMode {
        &self.mode
    }

    /// 是否拥有 MRSIGNER 密钥 (用于判断跨版本升级兼容能力)
    pub fn has_mrsigner_key(&self) -> bool {
        self.sealed_storage.has_mrsigner_key()
    }

    /// 获取 Ed25519 签名密钥引用 (用于 ECDH share 加密)
    pub fn signing_key(&self) -> &ed25519_dalek::SigningKey {
        &self.keypair
    }

    pub fn public_key(&self) -> ed25519_dalek::VerifyingKey {
        self.keypair.verifying_key()
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.public_key().to_bytes()
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key_bytes())
    }

    /// 签名 (Software: 直接签; Hardware: ecall_sign)
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        use ed25519_dalek::Signer;
        let sig = self.keypair.sign(message);
        sig.to_bytes()
    }

    pub fn verify(&self, message: &[u8], signature: &[u8; 64]) -> bool {
        use ed25519_dalek::Verifier;
        let sig = ed25519_dalek::Signature::from_bytes(signature);
        self.public_key().verify(message, &sig).is_ok()
    }

    pub fn seal(&self, name: &str, data: &[u8]) -> BotResult<()> {
        self.sealed_storage.seal(name, data)
    }

    pub fn unseal(&self, name: &str) -> BotResult<Vec<u8>> {
        self.sealed_storage.unseal(name)
    }

    /// 获取密封存储密钥 (用于 Shamir share 加解密)
    ///
    /// Hardware 模式: 从 TDX configentry 派生, 绑定到 TD 实例 + MRTD
    /// Software 模式: 从 data_dir 路径派生 (不安全, 仅用于开发)
    pub fn seal_key(&self) -> BotResult<[u8; 32]> {
        use sha2::{Sha256, Digest};
        match self.mode {
            TeeMode::Tdx => {
                let hw_entropy = Self::read_tdx_seal_entropy()?;
                let mut hasher = Sha256::new();
                hasher.update(b"grouprobot-shamir-seal-hw:");
                hasher.update(hw_entropy);
                hasher.update(self.sealed_storage.data_dir().as_bytes());
                let result = hasher.finalize();
                let mut key = [0u8; 32];
                key.copy_from_slice(&result);
                Ok(key)
            }
            TeeMode::Sgx => {
                let hw_entropy = Self::read_sgx_seal_entropy()?;
                let mut hasher = Sha256::new();
                hasher.update(b"grouprobot-shamir-seal-sgx:");
                hasher.update(hw_entropy);
                hasher.update(self.sealed_storage.data_dir().as_bytes());
                let result = hasher.finalize();
                let mut key = [0u8; 32];
                key.copy_from_slice(&result);
                Ok(key)
            }
            TeeMode::Software => {
                let mut hasher = Sha256::new();
                hasher.update(b"grouprobot-shamir-seal-sw-v2:");
                hasher.update(self.sealed_storage.data_dir().as_bytes());
                if let Ok(hostname) = std::env::var("HOSTNAME") {
                    hasher.update(hostname.as_bytes());
                }
                if let Ok(machine_id) = std::fs::read_to_string("/etc/machine-id") {
                    hasher.update(machine_id.trim().as_bytes());
                }
                let result = hasher.finalize();
                let mut key = [0u8; 32];
                key.copy_from_slice(&result);
                Ok(key)
            }
        }
    }

    /// 读取 TDX 硬件密封熵
    fn read_tdx_seal_entropy() -> BotResult<[u8; 64]> {
        if let Ok(data) = std::fs::read("/dev/attestation/keys/_sgx_mrenclave") {
            if data.len() >= 16 {
                use sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                hasher.update(b"tdx-seal-from-sgx-key:");
                hasher.update(&data);
                let h1: [u8; 32] = hasher.finalize().into();
                let mut entropy = [0u8; 64];
                entropy[..32].copy_from_slice(&h1);
                let mut hasher2 = Sha256::new();
                hasher2.update(h1);
                hasher2.update(b"seal-entropy-extend");
                let h2: [u8; 32] = hasher2.finalize().into();
                entropy[32..].copy_from_slice(&h2);
                return Ok(entropy);
            }
        }
        if let Ok(quote) = std::fs::read("/dev/attestation/quote") {
            if quote.len() >= 232 {
                use sha2::{Sha256, Digest};
                let mrtd = &quote[184..232];
                let mut hasher = Sha256::new();
                hasher.update(b"tdx-seal-from-mrtd:");
                hasher.update(mrtd);
                let h: [u8; 32] = hasher.finalize().into();
                let mut entropy = [0u8; 64];
                entropy[..32].copy_from_slice(&h);
                entropy[32..48].copy_from_slice(&mrtd[..16]);
                return Ok(entropy);
            }
        }
        Err(BotError::EnclaveError(
            "无法读取 TDX 硬件熵: SGX seal key 和 MRTD 均不可用, 拒绝使用零值熵".into(),
        ))
    }

    /// 读取 SGX-Only 硬件密封熵
    fn read_sgx_seal_entropy() -> BotResult<[u8; 64]> {
        if let Ok(data) = std::fs::read("/dev/attestation/keys/_sgx_mrenclave") {
            if data.len() >= 16 {
                use sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                hasher.update(b"sgx-seal-from-mrenclave-key:");
                hasher.update(&data);
                let h1: [u8; 32] = hasher.finalize().into();
                let mut entropy = [0u8; 64];
                entropy[..32].copy_from_slice(&h1);
                let mut hasher2 = Sha256::new();
                hasher2.update(h1);
                hasher2.update(b"sgx-seal-entropy-extend");
                let h2: [u8; 32] = hasher2.finalize().into();
                entropy[32..].copy_from_slice(&h2);
                return Ok(entropy);
            }
        }
        if let Ok(quote) = std::fs::read("/dev/attestation/quote") {
            if quote.len() >= 144 {
                use sha2::{Sha256, Digest};
                let mrenclave = &quote[112..144];
                let mut hasher = Sha256::new();
                hasher.update(b"sgx-seal-from-mrenclave:");
                hasher.update(mrenclave);
                let h: [u8; 32] = hasher.finalize().into();
                let mut entropy = [0u8; 64];
                entropy[..32].copy_from_slice(&h);
                entropy[32..64].copy_from_slice(&h);
                return Ok(entropy);
            }
        }
        Err(BotError::EnclaveError(
            "无法读取 SGX 硬件熵: seal key 和 MRENCLAVE 均不可用".into(),
        ))
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

    /// 保存 ceremony hash (与 share 关联, 用于验证 peer 请求的 ceremony_hash)
    pub fn save_ceremony_hash(&self, hash: &[u8; 32]) -> BotResult<()> {
        self.sealed_storage.seal("ceremony_hash.sealed", hash)?;
        info!(ceremony = %hex::encode(hash), "Ceremony hash 已密封保存");
        Ok(())
    }

    /// 加载 ceremony hash
    pub fn load_ceremony_hash(&self) -> BotResult<Option<[u8; 32]>> {
        match self.sealed_storage.unseal("ceremony_hash.sealed") {
            Ok(data) if data.len() == 32 => {
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&data);
                Ok(Some(hash))
            }
            _ => Ok(None),
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
    fn init_with_all_policies() {
        for policy in [SealPolicy::MrEnclave, SealPolicy::MrSigner, SealPolicy::DualKey] {
            let dir = tempfile::tempdir().unwrap();
            let bridge = EnclaveBridge::init_with_policy(
                dir.path().to_str().unwrap(), "software", policy,
            ).unwrap();
            assert_eq!(*bridge.mode(), TeeMode::Software);
            assert_eq!(bridge.public_key_bytes().len(), 32);
        }
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
    fn key_persistence_across_policies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();

        // 用 MrEnclave 策略创建密钥
        let pk1 = {
            let bridge = EnclaveBridge::init_with_policy(path, "software", SealPolicy::MrEnclave).unwrap();
            bridge.public_key_bytes()
        };
        // 切换到 DualKey 策略, 密钥应保持 (V0 向后兼容)
        let pk2 = {
            let bridge = EnclaveBridge::init_with_policy(path, "software", SealPolicy::DualKey).unwrap();
            bridge.public_key_bytes()
        };
        assert_eq!(pk1, pk2, "切换策略后密钥应保持不变 (V0 兼容)");
    }

    #[test]
    fn replace_signing_key_preserves_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();

        let mut bridge = EnclaveBridge::init(path, "software").unwrap();
        let original_pk = bridge.public_key_bytes();

        let migration_seed = [0x42u8; 32];
        let expected_key = ed25519_dalek::SigningKey::from_bytes(&migration_seed);
        let expected_pk = expected_key.verifying_key().to_bytes();

        bridge.replace_signing_key(migration_seed).unwrap();
        assert_eq!(bridge.public_key_bytes(), expected_pk);
        assert_ne!(bridge.public_key_bytes(), original_pk);

        // 重启后应加载替换后的密钥
        let bridge2 = EnclaveBridge::init(path, "software").unwrap();
        assert_eq!(bridge2.public_key_bytes(), expected_pk);
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
