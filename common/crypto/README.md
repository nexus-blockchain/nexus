# pallet-crypto-common

NEXUS 加密内容公共类型库，供多个 pallet 共享加密内容存储、访问控制和密钥管理的核心数据结构。

## 导出类型

| 类型 | 说明 |
|------|------|
| `PrivateContent<AccountId, BlockNumber, MaxCidLen, MaxAuthorizedUsers, MaxKeyLen>` | 加密内容存储结构（链上元数据 + IPFS CID + 生命周期状态） |
| `AccessPolicy<AccountId, BlockNumber, MaxAuthorizedUsers>` | 访问控制策略（OwnerOnly / SharedWith / TimeboxedAccess / GovernanceControlled / RoleBased），含 `is_authorized()` / `is_expired()` 辅助方法 |
| `UserPublicKey<BlockNumber, MaxKeyLen>` | 用户公钥存储（RSA-2048 / Ed25519 / ECDSA-P256） |
| `KeyRotationRecord<AccountId, BlockNumber>` | 密钥轮换审计记录 |
| `EncryptionMethod` | 加密方法枚举（None / AES-256-GCM / ChaCha20-Poly1305 / XChaCha20-Poly1305），含 `from_u8()` / `as_u8()` / `is_encrypted()` |
| `KeyType` | 密钥类型枚举（RSA-2048 / Ed25519 / ECDSA-P256），含 `from_u8()` / `as_u8()` / `validate_key_len()` |
| `ContentStatus` | 内容生命周期状态（Active / Frozen / Archived / Purged），含 `is_mutable()` / `is_readable()` |

## 导出 Trait

| Trait | 说明 |
|-------|------|
| `PrivateContentManager<AccountId>` | 加密内容写入管理接口：store / grant / revoke / update_content / rotate_keys / force_grant / force_revoke / freeze / unfreeze |
| `PrivateContentProvider<AccountId>` | 加密内容只读查询接口：can_access / get_encrypted_key / get_decryption_info / get_content_status / get_content_creator |
| `KeyManager<AccountId>` | 用户公钥管理接口：register / update / revoke / has_public_key / get_key_type / get_key_data |

## 公共 Helper

| 函数 | 说明 |
|------|------|
| `validate_cid(cid: &[u8]) -> bool` | CID 格式基本验证（非空、长度 ≤ 128、ASCII 可打印字符） |

## SCALE 向后兼容

所有枚举变体均使用 `#[codec(index = N)]` 显式标注，确保：
- `EncryptionMethod` 编码值与原始 `u8` 一致（0/1/2/3）
- `KeyType` 编码值与原始 `u8` 一致（1/2/3）
- `ContentStatus` 编码值（0/1/2/3）
- 新增变体不会影响已有存储数据的解码

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

## 单元测试

30 个测试覆盖：
- `EncryptionMethod`: from_u8 / as_u8 / is_encrypted / default / SCALE 编码向后兼容 / 从原始 u8 解码
- `KeyType`: from_u8 / as_u8 / SCALE 编码向后兼容 / validate_key_len / 从原始 u8 解码 / index=0 解码失败
- `ContentStatus`: default / is_mutable / is_readable / SCALE 编码
- `AccessPolicy`: OwnerOnly / SharedWith / TimeboxedAccess（未过期/已过期/边界值）/ GovernanceControlled / RoleBased / 非 Timeboxed 不过期 / SCALE roundtrip
- `validate_cid`: 空 / 有效 / 过长 / 控制字符 / 高位字节

## 依赖

- `frame-support` ≥ 45.0.0
- `frame-system` ≥ 45.0.0
- `sp-core`, `sp-runtime`
- `parity-scale-codec`, `scale-info`

## 版本历史

| 版本 | 变更 |
|------|------|
| v0.2.0 | 枚举统一（EncryptionMethod/KeyType 替代 u8）、ContentStatus 生命周期、AccessPolicy::is_authorized()/is_expired()、PrivateContentProvider/KeyManager trait、validate_cid helper、30 单元测试 |
| v0.1.0 | 初始版本：PrivateContent / AccessPolicy / UserPublicKey / KeyRotationRecord / EncryptionMethod / KeyType / PrivateContentManager |
