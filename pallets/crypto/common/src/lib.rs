//! # pallet-crypto-common
//!
//! NEXUS 加密内容公共类型库，供多个 pallet 共享：
//! - `PrivateContent` — 加密内容存储结构
//! - `AccessPolicy` — 访问控制策略
//! - `UserPublicKey` — 用户公钥存储
//! - `KeyRotationRecord` — 密钥轮换记录
//! - `EncryptionMethod` — 加密方法枚举
//! - `KeyType` — 密钥类型枚举
//!
//! ## 设计原则
//!
//! 所有 struct/enum 使用 **原始泛型参数**（AccountId, BlockNumber, MaxCidLen 等），
//! 不依赖任何特定 pallet 的 Config trait。各 pallet 通过 type alias 映射自身 Config：
//!
//! ```rust,ignore
//! // 在 pallet-evidence 的 private_content.rs 中：
//! pub type PrivateContentOf<T> = pallet_crypto_common::PrivateContent<
//!     <T as frame_system::Config>::AccountId,
//!     BlockNumberFor<T>,
//!     <T as Config>::MaxCidLen,
//!     <T as Config>::MaxAuthorizedUsers,
//!     <T as Config>::MaxKeyLen,
//! >;
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use codec::{Decode, Encode, MaxEncodedLen, DecodeWithMemTracking};
use frame_support::{
    pallet_prelude::*,
    BoundedVec,
    CloneNoBound, PartialEqNoBound, EqNoBound, RuntimeDebugNoBound,
};
use scale_info::TypeInfo;
use sp_core::{ConstU32, H256};
use sp_runtime::RuntimeDebug;

// ============================================================
// EncryptionMethod — 替代原始 u8
// ============================================================

/// 加密方法
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum EncryptionMethod {
    /// 未加密（公开内容）
    None = 0,
    /// AES-256-GCM（推荐，高性能 AEAD）
    Aes256Gcm = 1,
    /// ChaCha20-Poly1305（移动端友好 AEAD）
    ChaCha20Poly1305 = 2,
    /// XChaCha20-Poly1305（扩展 nonce，防重放更安全）
    XChaCha20Poly1305 = 3,
}

impl Default for EncryptionMethod {
    fn default() -> Self {
        Self::None
    }
}

impl EncryptionMethod {
    /// 从原始 u8 转换（兼容旧 pallet-evidence 存储）
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::None,
            1 => Self::Aes256Gcm,
            2 => Self::ChaCha20Poly1305,
            3 => Self::XChaCha20Poly1305,
            _ => Self::None,
        }
    }

    /// 转换为 u8（兼容旧存储）
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// 是否为加密方法
    pub fn is_encrypted(&self) -> bool {
        !matches!(self, Self::None)
    }
}

// ============================================================
// KeyType — 替代原始 u8
// ============================================================

/// 密钥类型
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum KeyType {
    /// RSA-2048（DER 格式，270-512 字节）
    Rsa2048 = 1,
    /// Ed25519（32 字节）
    Ed25519 = 2,
    /// ECDSA-P256（33 或 65 字节）
    EcdsaP256 = 3,
}

impl KeyType {
    /// 从原始 u8 转换
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Rsa2048),
            2 => Some(Self::Ed25519),
            3 => Some(Self::EcdsaP256),
            _ => None,
        }
    }

    /// 转换为 u8
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// 验证密钥数据长度是否合法
    pub fn validate_key_len(&self, len: usize) -> bool {
        match self {
            Self::Rsa2048 => (270..=512).contains(&len),
            Self::Ed25519 => len == 32,
            Self::EcdsaP256 => len == 33 || len == 65,
        }
    }
}

// ============================================================
// AccessPolicy — 访问控制策略（原始泛型）
// ============================================================

/// 访问控制策略
///
/// 定义谁可以解密加密内容。所有需要加密的 pallet（evidence、arbitration、kyc、order）
/// 均使用此枚举管理访问权限。
///
/// 泛型参数：
/// - `AccountId` — 账户类型
/// - `BlockNumber` — 区块号类型
/// - `MaxAuthorizedUsers` — 最大授权用户数上限
#[derive(
    Encode, Decode, DecodeWithMemTracking,
    CloneNoBound, PartialEqNoBound, EqNoBound, RuntimeDebugNoBound,
    TypeInfo, MaxEncodedLen,
)]
#[codec(encode_bound())]
#[codec(decode_bound())]
#[codec(mel_bound(
    AccountId: MaxEncodedLen,
    BlockNumber: MaxEncodedLen,
))]
#[scale_info(skip_type_params(MaxAuthorizedUsers))]
pub enum AccessPolicy<
    AccountId: Encode + Decode + Clone + PartialEq + Eq + core::fmt::Debug + TypeInfo + MaxEncodedLen,
    BlockNumber: Encode + Decode + Clone + PartialEq + Eq + core::fmt::Debug + TypeInfo + MaxEncodedLen,
    MaxAuthorizedUsers: Get<u32>,
> {
    /// 仅创建者可访问
    OwnerOnly,
    /// 指定用户列表
    SharedWith(BoundedVec<AccountId, MaxAuthorizedUsers>),
    /// 定时访问（到期后自动撤销）
    TimeboxedAccess {
        users: BoundedVec<AccountId, MaxAuthorizedUsers>,
        expires_at: BlockNumber,
    },
    /// 治理控制（由治理提案决定访问权限）
    GovernanceControlled,
    /// 基于角色的访问（扩展用，role 标识符最长 32 字节）
    RoleBased(BoundedVec<u8, ConstU32<32>>),
}

// ============================================================
// PrivateContent — 加密内容存储结构（原始泛型）
// ============================================================

/// 加密内容存储结构
///
/// 链上存储加密内容的元数据（CID、哈希、密钥包），实际加密数据存储在 IPFS。
/// 任何需要加密 IPFS 内容的 pallet 均可复用此结构。
///
/// 泛型参数：
/// - `AccountId` — 账户类型
/// - `BlockNumber` — 区块号类型
/// - `MaxCidLen` — CID 最大字节数
/// - `MaxAuthorizedUsers` — 最大授权用户数
/// - `MaxKeyLen` — 加密密钥最大字节数
#[derive(
    Encode, Decode, DecodeWithMemTracking,
    CloneNoBound, PartialEqNoBound, EqNoBound, RuntimeDebugNoBound,
    TypeInfo, MaxEncodedLen,
)]
#[codec(encode_bound())]
#[codec(decode_bound())]
#[codec(mel_bound(
    AccountId: MaxEncodedLen,
    BlockNumber: MaxEncodedLen,
))]
#[scale_info(skip_type_params(MaxCidLen, MaxAuthorizedUsers, MaxKeyLen))]
pub struct PrivateContent<
    AccountId: Encode + Decode + Clone + PartialEq + Eq + core::fmt::Debug + TypeInfo + MaxEncodedLen,
    BlockNumber: Encode + Decode + Clone + PartialEq + Eq + core::fmt::Debug + TypeInfo + MaxEncodedLen,
    MaxCidLen: Get<u32>,
    MaxAuthorizedUsers: Get<u32>,
    MaxKeyLen: Get<u32>,
> {
    /// 内容ID（由使用方 pallet 分配）
    pub id: u64,
    /// 命名空间（8 字节，用于隔离不同业务域）
    pub ns: [u8; 8],
    /// 业务主体ID（如 evidence_id、complaint_id、order_id）
    pub subject_id: u64,
    /// IPFS CID（明文存储，便于索引和去重）
    pub cid: BoundedVec<u8, MaxCidLen>,
    /// 原始内容的哈希（用于验证完整性，SHA-256）
    pub content_hash: H256,
    /// 加密方法标识 (1=AES-256-GCM, 2=ChaCha20-Poly1305, etc.)
    pub encryption_method: u8,
    /// 创建者
    pub creator: AccountId,
    /// 访问控制策略
    pub access_policy: AccessPolicy<AccountId, BlockNumber, MaxAuthorizedUsers>,
    /// 每个授权用户的加密密钥包
    pub encrypted_keys: BoundedVec<(AccountId, BoundedVec<u8, MaxKeyLen>), MaxAuthorizedUsers>,
    /// 创建时间
    pub created_at: BlockNumber,
    /// 最后更新时间
    pub updated_at: BlockNumber,
}

// ============================================================
// UserPublicKey — 用户公钥存储（原始泛型）
// ============================================================

/// 用户公钥存储
///
/// 用于非对称加密密钥分发：创建者用接收方公钥加密对称密钥，
/// 接收方用私钥解密对称密钥，再用对称密钥解密内容。
#[derive(
    Encode, Decode, DecodeWithMemTracking,
    CloneNoBound, PartialEqNoBound, EqNoBound, RuntimeDebugNoBound,
    TypeInfo, MaxEncodedLen,
)]
#[codec(encode_bound())]
#[codec(decode_bound())]
#[codec(mel_bound(BlockNumber: MaxEncodedLen))]
#[scale_info(skip_type_params(MaxKeyLen))]
pub struct UserPublicKey<
    BlockNumber: Encode + Decode + Clone + PartialEq + Eq + core::fmt::Debug + TypeInfo + MaxEncodedLen,
    MaxKeyLen: Get<u32>,
> {
    /// 公钥数据（DER/原始格式）
    pub key_data: BoundedVec<u8, MaxKeyLen>,
    /// 密钥类型（1=RSA-2048, 2=Ed25519, 3=ECDSA-P256）
    pub key_type: u8,
    /// 注册时间（区块号）
    pub registered_at: BlockNumber,
}

// ============================================================
// KeyRotationRecord — 密钥轮换记录（原始泛型）
// ============================================================

/// 密钥轮换记录
///
/// 当加密内容的对称密钥需要更换时（如用户被撤销访问权限后），
/// 记录每次轮换的元数据，便于审计和追踪。
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct KeyRotationRecord<AccountId, BlockNumber> {
    /// 内容ID
    pub content_id: u64,
    /// 轮换批次（递增）
    pub rotation_round: u32,
    /// 轮换时间
    pub rotated_at: BlockNumber,
    /// 轮换者
    pub rotated_by: AccountId,
}

// ============================================================
// PrivateContentManager trait — 加密内容管理接口
// ============================================================

/// 加密内容管理接口
///
/// 供业务 pallet 通过 trait 间接操作加密内容，
/// 无需直接依赖 pallet-evidence 的存储。
pub trait PrivateContentManager<AccountId> {
    /// 存储加密内容，返回 content_id
    fn store_private_content(
        creator: AccountId,
        ns: [u8; 8],
        subject_id: u64,
        cid: Vec<u8>,
        content_hash: H256,
        encryption_method: u8,
    ) -> Result<u64, sp_runtime::DispatchError>;

    /// 授权用户访问
    fn grant_access(
        grantor: AccountId,
        content_id: u64,
        user: AccountId,
        encrypted_key: Vec<u8>,
    ) -> Result<(), sp_runtime::DispatchError>;

    /// 撤销用户访问
    fn revoke_access(
        revoker: AccountId,
        content_id: u64,
        user: AccountId,
    ) -> Result<(), sp_runtime::DispatchError>;

    /// 检查用户是否有访问权限
    fn has_access(
        content_id: u64,
        user: &AccountId,
    ) -> bool;
}
