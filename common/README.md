# Common Libraries

共享工具库模块组，包含非 pallet 的公共类型和工具函数，供多个 pallet 共享使用。

## 模块结构

```
common/
├── crypto/     # 加密内容公共类型 (pallet-crypto-common)
└── media/      # 媒体处理工具 (nexus-media-common)
```

## 模块说明

### crypto (加密内容公共类型)

**导出内容**：共享数据结构和 trait，供多个 pallet 共享加密内容存储、访问控制和密钥管理。

- `PrivateContent` — 加密内容存储结构
- `AccessPolicy` — 访问控制策略（OwnerOnly / SharedWith / TimeboxedAccess / GovernanceControlled / RoleBased）
- `UserPublicKey` — 用户公钥存储（RSA-2048 / Ed25519 / ECDSA-P256）
- `EncryptionMethod` / `KeyType` / `ContentStatus` — 枚举类型
- `PrivateContentManager` / `PrivateContentProvider` / `KeyManager` — trait 接口

### media (媒体处理工具)

**导出内容**：媒体格式验证和哈希计算工具函数。

- `IpfsHelper` — IPFS CID 格式验证、内容完整性校验
- `HashHelper` — Blake2-256 承诺哈希计算
- `MediaError` — 错误类型

## 设计原则

- **非 pallet**：不包含 extrinsic、storage、event，不编译为独立 runtime 模块
- **no_std 兼容**：支持 WASM runtime 环境
- **纯泛型**：不依赖特定 pallet 的 Config trait
