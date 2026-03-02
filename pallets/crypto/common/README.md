# pallet-crypto-common

NEXUS 加密内容公共类型库，供多个 pallet 共享加密内容存储、访问控制和密钥管理的核心数据结构。

## 导出类型

| 类型 | 说明 |
|------|------|
| `PrivateContent<AccountId, BlockNumber, MaxCidLen, MaxAuthorizedUsers, MaxKeyLen>` | 加密内容存储结构（链上元数据 + IPFS CID） |
| `AccessPolicy<AccountId, BlockNumber, MaxAuthorizedUsers>` | 访问控制策略（OwnerOnly / SharedWith / TimeboxedAccess / GovernanceControlled / RoleBased） |
| `UserPublicKey<BlockNumber, MaxKeyLen>` | 用户公钥存储（RSA-2048 / Ed25519 / ECDSA-P256） |
| `KeyRotationRecord<AccountId, BlockNumber>` | 密钥轮换审计记录 |
| `EncryptionMethod` | 加密方法枚举（None / AES-256-GCM / ChaCha20-Poly1305 / XChaCha20-Poly1305） |
| `KeyType` | 密钥类型枚举（RSA-2048 / Ed25519 / ECDSA-P256） |
| `PrivateContentManager<AccountId>` | 加密内容管理 trait 接口 |

## 设计原则

所有 struct/enum 使用 **原始泛型参数**（`AccountId`, `BlockNumber`, `MaxCidLen` 等），不依赖任何特定 pallet 的 Config trait。

各业务 pallet 通过 **type alias** 映射自身 Config 的关联类型：

```rust
// pallets/dispute/evidence/src/private_content.rs
pub type PrivateContentOf<T> = pallet_crypto_common::PrivateContent<
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
    <T as Config>::MaxCidLen,
    <T as Config>::MaxAuthorizedUsers,
    <T as Config>::MaxKeyLen,
>;
```

这种设计避免了 Rust orphan rule 和 Substrate pallet macro 的兼容性问题，同时保证 SCALE 编码完全一致（存储向后兼容）。

## 使用方

| Pallet | 用途 |
|--------|------|
| `pallet-evidence` | 证据加密存储、访问控制、密钥轮换 |
| `pallet-arbitration` | （规划中）仲裁加密证据 |
| `pallet-kyc` | （规划中）KYC 加密文档 |

## 依赖

- `frame-support` ≥ 45.0.0
- `frame-system` ≥ 45.0.0
- `sp-core`, `sp-runtime`
- `parity-scale-codec`, `scale-info`
