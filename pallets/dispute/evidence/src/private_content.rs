//! 加密内容类型 — 基于 pallet-crypto-common 的类型别名层
//!
//! 核心 struct/enum 定义已迁移至 `pallet-crypto-common`（原始泛型版本），
//! 本模块通过 type alias 将 `pallet-evidence::Config` 的关联类型映射到泛型参数，
//! 保持与旧代码 100% 兼容的 API 和 SCALE 编码。
//!
//! 其他 pallet（kyc、arbitration、order）可直接依赖 `pallet-crypto-common`，
//! 用自身 Config 的关联类型构造同样的类型别名。

use crate::pallet::Config;
use frame_system::pallet_prelude::BlockNumberFor;

// ── 重导出枚举和 trait（无泛型，直接使用） ──
pub use pallet_crypto_common::{EncryptionMethod, KeyType, PrivateContentManager};

// ── 授权用户列表 ──
pub type AuthorizedUsers<T> = frame_support::BoundedVec<
    <T as frame_system::Config>::AccountId,
    <T as Config>::MaxAuthorizedUsers,
>;

// ── 加密密钥包 ──
pub type EncryptedKeyBundles<T> = frame_support::BoundedVec<
    (
        <T as frame_system::Config>::AccountId,
        frame_support::BoundedVec<u8, <T as Config>::MaxKeyLen>,
    ),
    <T as Config>::MaxAuthorizedUsers,
>;

// ── 访问控制策略 ──
pub type AccessPolicy<T> = pallet_crypto_common::AccessPolicy<
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
    <T as Config>::MaxAuthorizedUsers,
>;

// ── 加密内容存储结构 ──
pub type PrivateContent<T> = pallet_crypto_common::PrivateContent<
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
    <T as Config>::MaxCidLen,
    <T as Config>::MaxAuthorizedUsers,
    <T as Config>::MaxKeyLen,
>;

// ── 用户公钥存储 ──
pub type UserPublicKey<T> = pallet_crypto_common::UserPublicKey<
    BlockNumberFor<T>,
    <T as Config>::MaxKeyLen,
>;

// ── 密钥轮换记录 ──
pub type KeyRotationRecord<T> = pallet_crypto_common::KeyRotationRecord<
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
>;
