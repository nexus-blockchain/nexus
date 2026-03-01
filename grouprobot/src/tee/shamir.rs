// Shamir 秘密分享 — K-of-N 密钥分片/重组 (GF(256) 完整实现)
//
// 在 GF(256) 上实现 Shamir 门限方案:
// - split(secret, config) → N 个 Share，任意 K 个可重构
// - recover(shares, k) → 原始 secret
//
// 用于 TEE 节点间密钥恢复: BOT_TOKEN + Ed25519 私钥拆分后
// 各节点 SGX seal 自己的 share，启动时互验 Quote 获取 K 份重构。

use rand::RngCore;
use zeroize::Zeroizing;

/// 单个分片
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Share {
    /// 分片 ID (1-255, 在 GF(256) 中的 x 坐标)
    pub id: u8,
    /// 分片数据 (与原始 secret 等长)
    pub data: Vec<u8>,
}

/// 分片配置
#[derive(Clone, Debug)]
pub struct ShamirConfig {
    /// 重构门限 (至少需要 K 个 share)
    pub k: u8,
    /// 总分片数
    pub n: u8,
}

impl ShamirConfig {
    pub fn new(k: u8, n: u8) -> Result<Self, ShamirError> {
        if k == 0 || n == 0 {
            return Err(ShamirError::InvalidParameters("k and n must be > 0".into()));
        }
        if k > n {
            return Err(ShamirError::InvalidParameters("k must be <= n".into()));
        }
        if n > 254 {
            return Err(ShamirError::InvalidParameters("n must be <= 254".into()));
        }
        Ok(Self { k, n })
    }
}

/// Shamir 错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShamirError {
    InvalidParameters(String),
    InsufficientShares { need: u8, have: usize },
    EmptySecret,
    DuplicateShareId(u8),
    InvalidShareId,
    InconsistentShareLengths,
}

impl std::fmt::Display for ShamirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidParameters(msg) => write!(f, "invalid parameters: {}", msg),
            Self::InsufficientShares { need, have } => {
                write!(f, "need {} shares, only have {}", need, have)
            }
            Self::EmptySecret => write!(f, "secret cannot be empty"),
            Self::DuplicateShareId(id) => write!(f, "duplicate share id: {}", id),
            Self::InvalidShareId => write!(f, "share id must be 1-255"),
            Self::InconsistentShareLengths => write!(f, "all shares must have same length"),
        }
    }
}

impl std::error::Error for ShamirError {}

// ═══════════════════════════════════════════════════════════════
// GF(256) 有限域算术 (AES 多项式 x^8 + x^4 + x^3 + x + 1)
// ═══════════════════════════════════════════════════════════════

const GF256_POLY: u16 = 0x11B;

/// GF(256) 乘法
fn gf256_mul(a: u8, b: u8) -> u8 {
    let mut result: u16 = 0;
    let mut a = a as u16;
    let mut b = b as u16;

    for _ in 0..8 {
        if b & 1 != 0 {
            result ^= a;
        }
        let hi = a & 0x80;
        a <<= 1;
        if hi != 0 {
            a ^= GF256_POLY;
        }
        b >>= 1;
    }

    result as u8
}

/// GF(256) 逆元 (费马小定理: a^254 = a^{-1})
fn gf256_inv(a: u8) -> u8 {
    if a == 0 {
        return 0;
    }
    let mut result = a;
    for _ in 0..6 {
        result = gf256_mul(result, result);
        result = gf256_mul(result, a);
    }
    result = gf256_mul(result, result);
    result
}

/// GF(256) 除法
fn gf256_div(a: u8, b: u8) -> u8 {
    gf256_mul(a, gf256_inv(b))
}

// ═══════════════════════════════════════════════════════════════
// Shamir 核心
// ═══════════════════════════════════════════════════════════════

/// 将 secret 拆分为 N 个 share，任意 K 个可重构
pub fn split(secret: &[u8], config: &ShamirConfig) -> Result<Vec<Share>, ShamirError> {
    if secret.is_empty() {
        return Err(ShamirError::EmptySecret);
    }

    let k = config.k;
    let n = config.n;

    let mut rng = rand::rngs::OsRng;
    let mut shares: Vec<Share> = (1..=n)
        .map(|id| Share {
            id,
            data: vec![0u8; secret.len()],
        })
        .collect();

    // 对 secret 的每个字节独立构造 k-1 次多项式
    for (byte_idx, &secret_byte) in secret.iter().enumerate() {
        let mut coefficients = vec![0u8; k as usize];
        coefficients[0] = secret_byte;
        rng.fill_bytes(&mut coefficients[1..]);

        for share in shares.iter_mut() {
            let x = share.id;
            let mut y = 0u8;
            let mut x_pow = 1u8;

            for &coeff in &coefficients {
                y ^= gf256_mul(coeff, x_pow);
                x_pow = gf256_mul(x_pow, x);
            }

            share.data[byte_idx] = y;
        }
    }

    Ok(shares)
}

/// 从 K 个 share 重构 secret (拉格朗日插值)
pub fn recover(shares: &[Share], k: u8) -> Result<Vec<u8>, ShamirError> {
    if shares.len() < k as usize {
        return Err(ShamirError::InsufficientShares {
            need: k,
            have: shares.len(),
        });
    }

    if shares.is_empty() {
        return Err(ShamirError::InsufficientShares { need: k, have: 0 });
    }

    // 检查 share id 有效性和唯一性
    let mut seen_ids = std::collections::HashSet::new();
    for share in shares.iter().take(k as usize) {
        if share.id == 0 {
            return Err(ShamirError::InvalidShareId);
        }
        if !seen_ids.insert(share.id) {
            return Err(ShamirError::DuplicateShareId(share.id));
        }
    }

    // 检查 share 长度一致
    let secret_len = shares[0].data.len();
    for share in shares.iter().take(k as usize) {
        if share.data.len() != secret_len {
            return Err(ShamirError::InconsistentShareLengths);
        }
    }

    let used_shares = &shares[..k as usize];
    let mut secret = vec![0u8; secret_len];

    // 拉格朗日插值求 f(0)
    for (byte_idx, secret_byte) in secret.iter_mut().enumerate() {
        let mut value = 0u8;

        for (i, share_i) in used_shares.iter().enumerate() {
            let xi = share_i.id;
            let yi = share_i.data[byte_idx];

            // 计算拉格朗日基 L_i(0) = ∏_{j≠i} (0-x_j)/(x_i-x_j)
            let mut basis = 1u8;
            for (j, share_j) in used_shares.iter().enumerate() {
                if i == j {
                    continue;
                }
                let xj = share_j.id;
                basis = gf256_mul(basis, gf256_div(xj, xi ^ xj));
            }

            value ^= gf256_mul(yi, basis);
        }

        *secret_byte = value;
    }

    Ok(secret)
}

// ═══════════════════════════════════════════════════════════════
// 分片加密 (AES-256-GCM)
// ═══════════════════════════════════════════════════════════════

/// 加密的分片 (传输/存储用)
#[derive(Clone, Debug)]
pub struct EncryptedShare {
    /// 分片 ID
    pub id: u8,
    /// 加密后的数据 (AES-GCM)
    pub ciphertext: Vec<u8>,
    /// AES-GCM nonce
    pub nonce: [u8; 12],
}

/// 用 AES-256-GCM 加密分片
pub fn encrypt_share(share: &Share, key: &[u8; 32]) -> Result<EncryptedShare, ShamirError> {
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
    use aes_gcm::aead::Aead;

    let cipher = Aes256Gcm::new(key.into());
    let mut nonce_bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, share.data.as_ref())
        .map_err(|_| ShamirError::InvalidParameters("encryption failed".into()))?;

    Ok(EncryptedShare {
        id: share.id,
        ciphertext,
        nonce: nonce_bytes,
    })
}

/// 解密分片
pub fn decrypt_share(encrypted: &EncryptedShare, key: &[u8; 32]) -> Result<Share, ShamirError> {
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
    use aes_gcm::aead::Aead;

    let cipher = Aes256Gcm::new(key.into());
    let nonce = Nonce::from_slice(&encrypted.nonce);

    let plaintext = cipher
        .decrypt(nonce, encrypted.ciphertext.as_ref())
        .map_err(|_| ShamirError::InvalidParameters("decryption failed".into()))?;

    Ok(Share {
        id: encrypted.id,
        data: plaintext,
    })
}

// ═══════════════════════════════════════════════════════════════
// Secrets 编解码 (bot_token + signing_key → 字节序列)
// ═══════════════════════════════════════════════════════════════

/// 编码 secrets (signing_key + bot_token) 为字节序列
///
/// 格式: [sk_len:4LE][signing_key:32B][token_len:4LE][bot_token:N]
pub fn encode_secrets(bot_token: &str, signing_key: &[u8; 32]) -> Vec<u8> {
    let token_bytes = bot_token.as_bytes();
    let mut buf = Vec::with_capacity(4 + 32 + 4 + token_bytes.len());
    buf.extend_from_slice(&32u32.to_le_bytes());
    buf.extend_from_slice(signing_key);
    buf.extend_from_slice(&(token_bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(token_bytes);
    buf
}

/// 解码 secrets → (signing_key, bot_token)
///
/// bot_token 使用 Zeroizing<String> 包裹, 防止明文残留在堆内存
pub fn decode_secrets(data: &[u8]) -> Result<([u8; 32], Zeroizing<String>), ShamirError> {
    if data.len() < 4 + 32 + 4 {
        return Err(ShamirError::InvalidParameters("secrets data too short".into()));
    }
    let sk_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if sk_len != 32 || data.len() < 4 + 32 + 4 {
        return Err(ShamirError::InvalidParameters("invalid signing key length".into()));
    }
    let mut signing_key = [0u8; 32];
    signing_key.copy_from_slice(&data[4..36]);
    let token_len = u32::from_le_bytes([data[36], data[37], data[38], data[39]]) as usize;
    if data.len() < 40 + token_len {
        return Err(ShamirError::InvalidParameters("secrets data truncated".into()));
    }
    let bot_token = Zeroizing::new(
        String::from_utf8(data[40..40 + token_len].to_vec())
            .map_err(|e| ShamirError::InvalidParameters(format!("invalid UTF-8: {}", e)))?
    );
    Ok((signing_key, bot_token))
}

// ═══════════════════════════════════════════════════════════════
// EncryptedShare 二进制序列化
// ═══════════════════════════════════════════════════════════════

/// 将 EncryptedShare 序列化为字节
pub fn share_to_bytes(share: &EncryptedShare) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.push(share.id);
    buf.extend_from_slice(&share.nonce);
    buf.extend_from_slice(&(share.ciphertext.len() as u32).to_le_bytes());
    buf.extend_from_slice(&share.ciphertext);
    buf
}

/// 从字节反序列化 EncryptedShare
pub fn share_from_bytes(data: &[u8]) -> Result<EncryptedShare, ShamirError> {
    if data.len() < 1 + 12 + 4 {
        return Err(ShamirError::InvalidParameters("share data too short".into()));
    }
    let id = data[0];
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&data[1..13]);
    let ct_len = u32::from_le_bytes([data[13], data[14], data[15], data[16]]) as usize;
    if data.len() < 17 + ct_len {
        return Err(ShamirError::InvalidParameters("share data truncated".into()));
    }
    let ciphertext = data[17..17 + ct_len].to_vec();
    Ok(EncryptedShare { id, ciphertext, nonce })
}

// ═══════════════════════════════════════════════════════════════
// X25519 ECDH 公钥加密 (接收方公钥加密 Share)
// ═══════════════════════════════════════════════════════════════

/// Ed25519 签名密钥 → X25519 静态密钥 (用于 ECDH)
///
/// ed25519 签名密钥的前 32 bytes 经 SHA-512 → clamped → X25519 secret
/// 这与 libsodium crypto_sign_ed25519_sk_to_curve25519 兼容
#[allow(dead_code)]
pub fn ed25519_to_x25519_secret(ed_secret: &[u8; 32]) -> x25519_dalek::StaticSecret {
    use sha2::{Sha512, Digest};
    let mut hasher = Sha512::new();
    hasher.update(ed_secret);
    let hash = hasher.finalize();
    let mut scalar = [0u8; 32];
    scalar.copy_from_slice(&hash[..32]);
    // clamp
    scalar[0] &= 248;
    scalar[31] &= 127;
    scalar[31] |= 64;
    x25519_dalek::StaticSecret::from(scalar)
}

/// Ed25519 验证密钥 (公钥) → X25519 公钥
///
/// 使用 curve25519 的 Edwards → Montgomery 转换
#[allow(dead_code)]
pub fn ed25519_pk_to_x25519(ed_pk: &ed25519_dalek::VerifyingKey) -> x25519_dalek::PublicKey {
    let ep = ed_pk.to_montgomery();
    x25519_dalek::PublicKey::from(ep.to_bytes())
}

/// 用接收方公钥加密 Share (X25519 ECDH + AES-256-GCM)
///
/// 发送方生成临时密钥对 → ECDH(ephemeral_secret, receiver_pk) → 对称密钥 → 加密
/// 返回: (EncryptedShare, ephemeral_public_key)
#[allow(dead_code)]
pub fn encrypt_share_for_recipient(
    share: &Share,
    receiver_x25519_pk: &x25519_dalek::PublicKey,
) -> Result<(EncryptedShare, [u8; 32]), ShamirError> {
    use sha2::{Sha256, Digest};

    // 生成临时 X25519 密钥对
    let ephemeral_secret = x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng);
    let ephemeral_public = x25519_dalek::PublicKey::from(&ephemeral_secret);

    // ECDH → shared secret
    let shared_secret = ephemeral_secret.diffie_hellman(receiver_x25519_pk);

    // KDF: SHA256(shared_secret || "grouprobot-share-encrypt-v1")
    let mut hasher = Sha256::new();
    hasher.update(shared_secret.as_bytes());
    hasher.update(b"grouprobot-share-encrypt-v1");
    let derived_key: [u8; 32] = hasher.finalize().into();

    // AES-256-GCM 加密
    let encrypted = encrypt_share(share, &derived_key)?;

    Ok((encrypted, ephemeral_public.to_bytes()))
}

/// 用自己的私钥解密 Share (X25519 ECDH + AES-256-GCM)
///
/// receiver_secret: 接收方的 X25519 私钥
/// ephemeral_pk: 发送方的临时公钥 (随 EncryptedShare 一起传输)
#[allow(dead_code)]
pub fn decrypt_share_from_sender(
    encrypted: &EncryptedShare,
    receiver_secret: &x25519_dalek::StaticSecret,
    ephemeral_pk: &[u8; 32],
) -> Result<Share, ShamirError> {
    use sha2::{Sha256, Digest};

    let sender_pk = x25519_dalek::PublicKey::from(*ephemeral_pk);
    let shared_secret = receiver_secret.diffie_hellman(&sender_pk);

    // KDF: 同 encrypt 侧一致
    let mut hasher = Sha256::new();
    hasher.update(shared_secret.as_bytes());
    hasher.update(b"grouprobot-share-encrypt-v1");
    let derived_key: [u8; 32] = hasher.finalize().into();

    decrypt_share(encrypted, &derived_key)
}

// ═══════════════════════════════════════════════════════════════
// Raw ECDH 加解密 (用于 Migration Ceremony, 操作任意字节)
// ═══════════════════════════════════════════════════════════════

/// ECDH 加密任意字节 (Migration Ceremony 用)
///
/// 生成临时 X25519 密钥对 → ECDH → AES-256-GCM 加密
/// 返回: (ciphertext, ephemeral_public_key)
#[allow(dead_code)]
pub fn ecdh_encrypt_raw(
    plaintext: &[u8],
    receiver_x25519_pk: &x25519_dalek::PublicKey,
) -> Result<(Vec<u8>, [u8; 32]), ShamirError> {
    use sha2::{Sha256, Digest};
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};

    let ephemeral_secret = x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng);
    let ephemeral_public = x25519_dalek::PublicKey::from(&ephemeral_secret);
    let shared_secret = ephemeral_secret.diffie_hellman(receiver_x25519_pk);

    let mut hasher = Sha256::new();
    hasher.update(shared_secret.as_bytes());
    hasher.update(b"grouprobot-migration-ecdh-v1");
    let derived_key: [u8; 32] = hasher.finalize().into();

    let cipher = Aes256Gcm::new(&derived_key.into());
    let mut nonce_bytes = [0u8; 12];
    use rand::RngCore;
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, plaintext)
        .map_err(|e| ShamirError::InvalidParameters(format!("ECDH encrypt: {}", e)))?;

    // [nonce:12][ciphertext]
    let mut output = Vec::with_capacity(12 + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);

    Ok((output, ephemeral_public.to_bytes()))
}

/// ECDH 解密任意字节 (Migration Ceremony 用)
///
/// data = [nonce:12][ciphertext]
#[allow(dead_code)]
pub fn ecdh_decrypt_raw(
    data: &[u8],
    receiver_secret: &x25519_dalek::StaticSecret,
    ephemeral_pk: &[u8; 32],
) -> Result<Vec<u8>, ShamirError> {
    use sha2::{Sha256, Digest};
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};

    if data.len() < 13 {
        return Err(ShamirError::InvalidParameters("ECDH encrypted data too short".into()));
    }

    let sender_pk = x25519_dalek::PublicKey::from(*ephemeral_pk);
    let shared_secret = receiver_secret.diffie_hellman(&sender_pk);

    let mut hasher = Sha256::new();
    hasher.update(shared_secret.as_bytes());
    hasher.update(b"grouprobot-migration-ecdh-v1");
    let derived_key: [u8; 32] = hasher.finalize().into();

    let cipher = Aes256Gcm::new(&derived_key.into());
    let nonce = Nonce::from_slice(&data[..12]);
    let ciphertext = &data[12..];

    cipher.decrypt(nonce, ciphertext)
        .map_err(|e| ShamirError::InvalidParameters(format!("ECDH decrypt: {}", e)))
}

/// 带 ephemeral_pk 的加密 share (传输格式)
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct EcdhEncryptedShare {
    /// 加密的 share
    pub encrypted: EncryptedShare,
    /// 发送方临时公钥 (32 bytes, X25519)
    pub ephemeral_pk: [u8; 32],
}

/// 序列化 EcdhEncryptedShare → bytes
#[allow(dead_code)]
pub fn ecdh_share_to_bytes(share: &EcdhEncryptedShare) -> Vec<u8> {
    let inner = share_to_bytes(&share.encrypted);
    let mut buf = Vec::with_capacity(32 + inner.len());
    buf.extend_from_slice(&share.ephemeral_pk);
    buf.extend_from_slice(&inner);
    buf
}

/// 反序列化 bytes → EcdhEncryptedShare
#[allow(dead_code)]
pub fn ecdh_share_from_bytes(data: &[u8]) -> Result<EcdhEncryptedShare, ShamirError> {
    if data.len() < 32 + 1 + 12 + 4 {
        return Err(ShamirError::InvalidParameters("ecdh share data too short".into()));
    }
    let mut ephemeral_pk = [0u8; 32];
    ephemeral_pk.copy_from_slice(&data[..32]);
    let encrypted = share_from_bytes(&data[32..])?;
    Ok(EcdhEncryptedShare { encrypted, ephemeral_pk })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gf256_mul_identity() {
        assert_eq!(gf256_mul(1, 1), 1);
        assert_eq!(gf256_mul(0, 42), 0);
        assert_eq!(gf256_mul(42, 0), 0);
        assert_eq!(gf256_mul(1, 42), 42);
    }

    #[test]
    fn gf256_inv_all() {
        for a in 1..=255u8 {
            let inv = gf256_inv(a);
            assert_eq!(gf256_mul(a, inv), 1, "inv({}) failed", a);
        }
    }

    #[test]
    fn split_recover_2_of_3() {
        let secret = b"hello world secret key material!";
        let config = ShamirConfig::new(2, 3).unwrap();
        let shares = split(secret, &config).unwrap();
        assert_eq!(shares.len(), 3);

        for combo in &[[0, 1], [0, 2], [1, 2]] {
            let selected: Vec<Share> = combo.iter().map(|&i| shares[i].clone()).collect();
            let recovered = recover(&selected, 2).unwrap();
            assert_eq!(&recovered, secret.as_slice(), "combo {:?} failed", combo);
        }
    }

    #[test]
    fn split_recover_3_of_5() {
        let secret = b"this is a longer secret that we want to split into five shares";
        let config = ShamirConfig::new(3, 5).unwrap();
        let shares = split(secret, &config).unwrap();
        assert_eq!(shares.len(), 5);

        let combos = vec![
            vec![0, 1, 2], vec![0, 1, 3], vec![0, 1, 4],
            vec![0, 2, 3], vec![0, 2, 4], vec![0, 3, 4],
            vec![1, 2, 3], vec![1, 2, 4], vec![1, 3, 4],
            vec![2, 3, 4],
        ];
        for combo in combos {
            let selected: Vec<Share> = combo.iter().map(|&i| shares[i].clone()).collect();
            let recovered = recover(&selected, 3).unwrap();
            assert_eq!(&recovered, secret.as_slice(), "combo {:?} failed", combo);
        }
    }

    #[test]
    fn insufficient_shares_fail() {
        let secret = b"test secret";
        let config = ShamirConfig::new(3, 5).unwrap();
        let shares = split(secret, &config).unwrap();
        let selected = vec![shares[0].clone(), shares[1].clone()];
        assert!(matches!(recover(&selected, 3), Err(ShamirError::InsufficientShares { .. })));
    }

    #[test]
    fn wrong_shares_fail() {
        let config = ShamirConfig::new(2, 3).unwrap();
        let shares = split(b"test secret", &config).unwrap();
        let other_shares = split(b"other secre", &config).unwrap();
        let mixed = vec![shares[0].clone(), other_shares[1].clone()];
        let recovered = recover(&mixed, 2).unwrap();
        assert_ne!(&recovered, b"test secret".as_slice());
    }

    #[test]
    fn single_byte_secret() {
        let config = ShamirConfig::new(2, 3).unwrap();
        let shares = split(&[42u8], &config).unwrap();
        let recovered = recover(&[shares[0].clone(), shares[2].clone()], 2).unwrap();
        assert_eq!(recovered, vec![42u8]);
    }

    #[test]
    fn large_secret() {
        let mut secret = vec![0u8; 1024];
        rand::rngs::OsRng.fill_bytes(&mut secret);
        let config = ShamirConfig::new(2, 3).unwrap();
        let shares = split(&secret, &config).unwrap();
        let recovered = recover(&shares[..2], 2).unwrap();
        assert_eq!(recovered, secret);
    }

    #[test]
    fn config_validation() {
        assert!(ShamirConfig::new(0, 3).is_err());
        assert!(ShamirConfig::new(3, 0).is_err());
        assert!(ShamirConfig::new(5, 3).is_err());
        assert!(ShamirConfig::new(2, 255).is_err());
        assert!(ShamirConfig::new(2, 254).is_ok());
        assert!(ShamirConfig::new(1, 1).is_ok());
    }

    #[test]
    fn empty_secret_fails() {
        let config = ShamirConfig::new(2, 3).unwrap();
        assert!(matches!(split(&[], &config), Err(ShamirError::EmptySecret)));
    }

    #[test]
    fn duplicate_share_id_fails() {
        let share = Share { id: 1, data: vec![42] };
        assert!(matches!(recover(&[share.clone(), share], 2), Err(ShamirError::DuplicateShareId(1))));
    }

    #[test]
    fn share_ids_are_1_to_n() {
        let config = ShamirConfig::new(2, 5).unwrap();
        let shares = split(b"test", &config).unwrap();
        for (i, share) in shares.iter().enumerate() {
            assert_eq!(share.id, (i + 1) as u8);
        }
    }

    #[test]
    fn encrypt_decrypt_share() {
        let config = ShamirConfig::new(2, 3).unwrap();
        let shares = split(b"secret key material", &config).unwrap();
        let key = [0xABu8; 32];
        let encrypted = encrypt_share(&shares[0], &key).unwrap();
        assert_ne!(encrypted.ciphertext, shares[0].data);
        let decrypted = decrypt_share(&encrypted, &key).unwrap();
        assert_eq!(decrypted, shares[0]);
    }

    #[test]
    fn encrypt_wrong_key_fails() {
        let config = ShamirConfig::new(2, 3).unwrap();
        let shares = split(b"secret key material", &config).unwrap();
        let encrypted = encrypt_share(&shares[0], &[0xABu8; 32]).unwrap();
        assert!(decrypt_share(&encrypted, &[0xCDu8; 32]).is_err());
    }

    #[test]
    fn bot_token_split_recover() {
        let bot_token = b"123456:ABCdefGHIjklMNOpqrSTUvwxYZ-_0123456789";
        let config = ShamirConfig::new(2, 3).unwrap();
        let shares = split(bot_token, &config).unwrap();
        let recovered = recover(&[shares[1].clone(), shares[2].clone()], 2).unwrap();
        assert_eq!(recovered.as_slice(), bot_token.as_slice());
    }

    #[test]
    fn encode_decode_secrets_roundtrip() {
        let token = "123456:ABCDEF_token";
        let sk = [0x42u8; 32];
        let encoded = encode_secrets(token, &sk);
        let (recovered_sk, recovered_token) = decode_secrets(&encoded).unwrap();
        assert_eq!(recovered_sk, sk);
        assert_eq!(recovered_token.as_str(), token);
    }

    #[test]
    fn decode_secrets_too_short() {
        assert!(decode_secrets(&[0u8; 10]).is_err());
    }

    #[test]
    fn encode_split_recover_decode() {
        let token = "bot_token:SECRET123";
        let sk = [0xFFu8; 32];
        let secrets = encode_secrets(token, &sk);

        let config = ShamirConfig::new(2, 3).unwrap();
        let shares = split(&secrets, &config).unwrap();
        let recovered = recover(&[shares[0].clone(), shares[2].clone()], 2).unwrap();
        let (dec_sk, dec_token) = decode_secrets(&recovered).unwrap();
        assert_eq!(dec_sk, sk);
        assert_eq!(dec_token.as_str(), token);
    }

    #[test]
    fn share_serde_roundtrip() {
        let config = ShamirConfig::new(2, 3).unwrap();
        let shares = split(b"test data", &config).unwrap();
        let key = [0xAAu8; 32];
        let encrypted = encrypt_share(&shares[0], &key).unwrap();

        let bytes = share_to_bytes(&encrypted);
        let recovered = share_from_bytes(&bytes).unwrap();
        assert_eq!(recovered.id, encrypted.id);
        assert_eq!(recovered.nonce, encrypted.nonce);
        assert_eq!(recovered.ciphertext, encrypted.ciphertext);
    }

    #[test]
    fn share_serde_too_short() {
        assert!(share_from_bytes(&[0u8; 5]).is_err());
    }

    // ── X25519 ECDH tests ──

    #[test]
    fn ecdh_encrypt_decrypt_roundtrip() {
        // 模拟接收方: Ed25519 密钥 → X25519
        let ed_secret = ed25519_dalek::SigningKey::from_bytes(&[0x42u8; 32]);
        let ed_pk = ed_secret.verifying_key();
        let x_secret = ed25519_to_x25519_secret(&[0x42u8; 32]);
        let x_pk = ed25519_pk_to_x25519(&ed_pk);

        // 创建 share
        let config = ShamirConfig::new(2, 3).unwrap();
        let shares = split(b"ecdh test secret", &config).unwrap();

        // 发送方: 用接收方公钥加密
        let (encrypted, ephemeral_pk) = encrypt_share_for_recipient(&shares[0], &x_pk).unwrap();

        // 接收方: 用私钥解密
        let decrypted = decrypt_share_from_sender(&encrypted, &x_secret, &ephemeral_pk).unwrap();
        assert_eq!(decrypted, shares[0]);
    }

    #[test]
    fn ecdh_wrong_receiver_fails() {
        let ed_secret = ed25519_dalek::SigningKey::from_bytes(&[0x42u8; 32]);
        let ed_pk = ed_secret.verifying_key();
        let x_pk = ed25519_pk_to_x25519(&ed_pk);

        // 另一个接收方的密钥
        let wrong_secret = ed25519_to_x25519_secret(&[0xAA; 32]);

        let config = ShamirConfig::new(2, 3).unwrap();
        let shares = split(b"ecdh wrong key test", &config).unwrap();

        let (encrypted, ephemeral_pk) = encrypt_share_for_recipient(&shares[0], &x_pk).unwrap();

        // 错误接收方解密失败
        assert!(decrypt_share_from_sender(&encrypted, &wrong_secret, &ephemeral_pk).is_err());
    }

    #[test]
    fn ecdh_share_serde_roundtrip() {
        let ed_secret = ed25519_dalek::SigningKey::from_bytes(&[0x55u8; 32]);
        let ed_pk = ed_secret.verifying_key();
        let x_pk = ed25519_pk_to_x25519(&ed_pk);

        let config = ShamirConfig::new(2, 3).unwrap();
        let shares = split(b"serde test", &config).unwrap();

        let (encrypted, ephemeral_pk) = encrypt_share_for_recipient(&shares[0], &x_pk).unwrap();
        let ecdh_share = EcdhEncryptedShare { encrypted, ephemeral_pk };

        // 序列化 → 反序列化
        let bytes = ecdh_share_to_bytes(&ecdh_share);
        let recovered = ecdh_share_from_bytes(&bytes).unwrap();
        assert_eq!(recovered.ephemeral_pk, ecdh_share.ephemeral_pk);
        assert_eq!(recovered.encrypted.id, ecdh_share.encrypted.id);
        assert_eq!(recovered.encrypted.nonce, ecdh_share.encrypted.nonce);
        assert_eq!(recovered.encrypted.ciphertext, ecdh_share.encrypted.ciphertext);
    }

    #[test]
    fn ecdh_share_serde_too_short() {
        assert!(ecdh_share_from_bytes(&[0u8; 10]).is_err());
    }

    #[test]
    fn ed25519_to_x25519_deterministic() {
        let s1 = ed25519_to_x25519_secret(&[0x99u8; 32]);
        let s2 = ed25519_to_x25519_secret(&[0x99u8; 32]);
        let pk1 = x25519_dalek::PublicKey::from(&s1);
        let pk2 = x25519_dalek::PublicKey::from(&s2);
        assert_eq!(pk1.as_bytes(), pk2.as_bytes());
    }

    #[test]
    fn ecdh_raw_roundtrip() {
        let receiver_ed = ed25519_dalek::SigningKey::from_bytes(&[0x77u8; 32]);
        let receiver_x_secret = ed25519_to_x25519_secret(&receiver_ed.to_bytes());
        let receiver_x_pk = x25519_dalek::PublicKey::from(&receiver_x_secret);

        let plaintext = b"migration secret payload: sk + token";
        let (encrypted, ephemeral_pk) = ecdh_encrypt_raw(plaintext, &receiver_x_pk).unwrap();

        let decrypted = ecdh_decrypt_raw(&encrypted, &receiver_x_secret, &ephemeral_pk).unwrap();
        assert_eq!(&decrypted, plaintext);
    }

    #[test]
    fn ecdh_raw_wrong_key_fails() {
        let receiver_ed = ed25519_dalek::SigningKey::from_bytes(&[0x77u8; 32]);
        let receiver_x_secret = ed25519_to_x25519_secret(&receiver_ed.to_bytes());
        let receiver_x_pk = x25519_dalek::PublicKey::from(&receiver_x_secret);

        let (encrypted, ephemeral_pk) = ecdh_encrypt_raw(b"secret", &receiver_x_pk).unwrap();

        let wrong_secret = ed25519_to_x25519_secret(&[0xAA; 32]);
        assert!(ecdh_decrypt_raw(&encrypted, &wrong_secret, &ephemeral_pk).is_err());
    }

    #[test]
    fn ecdh_raw_too_short_fails() {
        let secret = ed25519_to_x25519_secret(&[0x11; 32]);
        assert!(ecdh_decrypt_raw(&[0u8; 5], &secret, &[0u8; 32]).is_err());
    }
}
