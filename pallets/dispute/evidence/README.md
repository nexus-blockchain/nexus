# pallet-evidence

> 路径：`pallets/dispute/evidence/` · Runtime Index: 61

统一证据管理系统，提供链上证据提交、IPFS 存储、私密内容加密、访问控制、自动归档等功能，服务于 8 个业务域的争议与仲裁流程。

## 设计理念

- **CID 化存储**：链上仅存 `content_cid`（~64 字节），实际内容存 IPFS，较旧版降低约 74.5% 存储成本
- **双模式提交**：Plain（公开 CID + 自动 Pin）与 Commit（承诺哈希，不暴露明文）
- **低耦合架构**：通过 `EvidenceAuthorizer` trait 与业务 pallet 解耦，runtime 注入实现
- **自动 IPFS Pin**：与 `pallet-storage-service` 集成，证据提交后自动固定 CID
- **三级防膨胀**：证据 90 天自动归档（~50 字节摘要）→ 归档 180 天自动清理 → 索引同步回收
- **双维限频**：账户级（`WindowBlocks` + `MaxPerWindow`）与目标级（`MaxPerSubjectTarget` / `MaxPerSubjectNs`）
- **证据链**：父子追加机制，原证据不可变，补充材料形成可追溯链
- **2 天修改窗口**：提交后 `EvidenceEditWindow` 内可修改清单，窗口关闭后不可变

## 架构

```
┌──────────────────────────────────────────────────────────────┐
│                      pallet-evidence                         │
│                                                              │
│  ┌─────────────────┐  ┌──────────────────┐  ┌────────────┐  │
│  │  Evidence Core   │  │ Private Content  │  │  Archive   │  │
│  │                  │  │                  │  │            │  │
│  │ · commit         │  │ · register_key   │  │ · on_idle  │  │
│  │ · commit_hash    │  │ · store_private  │  │ · 90d归档  │  │
│  │ · link/unlink    │  │ · grant/revoke   │  │ · 180d清理 │  │
│  │ · append         │  │ · rotate_keys    │  │            │  │
│  │ · update_manifest│  │                  │  │            │  │
│  └────────┬─────────┘  └────────┬─────────┘  └─────┬──────┘  │
│           │                     │                   │         │
│  ┌────────┴─────────────────────┴───────────────────┴──────┐  │
│  │              共享基础设施                                  │  │
│  │  · EvidenceAuthorizer · CidValidator · Rate Limiter     │  │
│  └─────────────────────────────────────────────────────────┘  │
└──────────────────────┬───────────────────────────────────────┘
                       │
         ┌─────────────┼─────────────┐
         ▼             ▼             ▼
   ┌───────────┐ ┌──────────┐ ┌───────────┐
   │ storage-  │ │ storage- │ │  media-   │
   │ service   │ │lifecycle │ │  utils    │
   │ IpfsPinner│ │ YYMM转换 │ │ Hash/CID │
   └───────────┘ └──────────┘ └───────────┘
```

## Extrinsics

### 证据提交（Plain / Commit）

| call_index | 方法 | 权重 | 说明 |
|:---:|------|------|------|
| 0 | `commit` | `commit(i,v,d)` | 提交公开证据（imgs/vids/docs CID），自动 IPFS Pin，全局 CID 去重 |
| 1 | `commit_hash` | `commit_hash()` | 提交承诺哈希 `H(ns‖subject_id‖cid_enc‖salt‖ver)`，不暴露明文 CID |
| 11 | `append_evidence` | `commit(i,v,d)` | 追加补充证据，继承父证据的 domain/target_id，形成子证据链 |
| 12 | `update_evidence_manifest` | `commit(i,v,d)` | 在 `EvidenceEditWindow` 内修改已提交的证据清单 |

### 证据链接

| call_index | 方法 | 权重 | 说明 |
|:---:|------|------|------|
| 2 | `link` | `link()` | 按 (domain, target_id) 链接已有证据（复用，不计入配额） |
| 3 | `link_by_ns` | `link_by_ns()` | 按 (ns, subject_id) 链接，需命名空间匹配 |
| 4 | `unlink` | `unlink()` | 取消 (domain, target_id) 链接 |
| 5 | `unlink_by_ns` | `unlink_by_ns()` | 取消 (ns, subject_id) 链接 |

### 私密内容管理

| call_index | 方法 | 权重 | 说明 |
|:---:|------|------|------|
| 6 | `register_public_key` | `register_public_key()` | 注册用户公钥（RSA-2048 / Ed25519 / ECDSA-P256） |
| 7 | `store_private_content` | `store_private_content()` | 存储加密私密内容（CID 必须带加密前缀，含访问策略 + 密钥包） |
| 8 | `grant_access` | `grant_access()` | 授予用户访问权限（仅创建者，更新密钥包） |
| 9 | `revoke_access` | `revoke_access()` | 撤销用户访问权限（不可撤销自己） |
| 10 | `rotate_content_keys` | `rotate_content_keys()` | 轮换加密密钥（O(1) 计数器，记录轮换历史） |

## 存储

### 证据核心

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextEvidenceId` | `u64` | 证据 ID 自增计数器 |
| `Evidences` | `Map<u64, Evidence>` | 证据主存储 |
| `EvidenceByTarget` | `DoubleMap<(u8, u64), u64, ()>` | 按 (domain, target_id) → evidence_id 索引 |
| `EvidenceByNs` | `DoubleMap<([u8;8], u64), u64, ()>` | 按 (ns, subject_id) → evidence_id 索引 |
| `EvidenceCountByTarget` | `Map<(u8, u64), u32>` | 按 (domain, target_id) 计数（限额用） |
| `EvidenceCountByNs` | `Map<([u8;8], u64), u32>` | 按 (ns, subject_id) 计数（限额用） |
| `CommitIndex` | `Map<H256, u64>` | 承诺哈希 → evidence_id 唯一索引（防重复提交） |
| `CidHashIndex` | `Map<H256, u64>` | `blake2_256(cid)` → evidence_id 全局去重索引（可选） |
| `AccountWindows` | `Map<AccountId, WindowInfo>` | 账户限频窗口（window_start + count） |

### 证据追加链

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `EvidenceParent` | `Map<u64, u64>` | 子证据 → 父证据映射 |
| `EvidenceChildren` | `Map<u64, BoundedVec<u64, 100>>` | 父证据 → 子证据列表（最多 100 条） |

### 待处理清单

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `PendingManifests` | `Map<u64, PendingManifest>` | evidence_id → 待处理清单 |
| `PendingManifestQueue` | `BoundedVec<u64, MaxListLen>` | 待处理队列（OCW 消费） |

### 私密内容

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextPrivateContentId` | `u64` | 私密内容 ID 计数器 |
| `PrivateContents` | `Map<u64, PrivateContent>` | 私密内容主存储 |
| `PrivateContentByCid` | `Map<BoundedVec<u8>, u64>` | CID → content_id 去重索引 |
| `PrivateContentBySubject` | `DoubleMap<([u8;8], u64), u64, ()>` | (ns, subject_id) → content_id 索引 |
| `UserPublicKeys` | `Map<AccountId, UserPublicKey>` | 用户公钥（加密密钥包用） |
| `KeyRotationHistory` | `DoubleMap<u64, u32, KeyRotationRecord>` | (content_id, round) → 轮换记录 |
| `KeyRotationCounter` | `Map<u64, u32>` | content_id → 轮换次数（O(1)，避免 iter_prefix 扫描） |

### 归档

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `ArchivedEvidences` | `Map<u64, ArchivedEvidence>` | 归档摘要（~50 字节/条） |
| `EvidenceArchiveCursor` | `u64` | 归档扫描游标 |
| `ArchiveStats` | `ArchiveStatistics` | 全局归档统计（总数 + 节省字节 + 最后归档块） |
| `ArchiveCleanupCursor` | `u64` | 归档清理扫描游标 |

## 主要类型

### Evidence（CID 化证据记录）

```rust
pub struct Evidence<AccountId, BlockNumber, MaxContentCidLen, MaxSchemeLen> {
    pub id: u64,
    pub domain: u8,                                           // 业务域（1=Evidence, 2=OtcOrder, ...）
    pub target_id: u64,                                       // 目标 ID（如订单号）
    pub owner: AccountId,
    pub content_cid: BoundedVec<u8, MaxContentCidLen>,        // IPFS 内容 CID
    pub content_type: ContentType,                            // 内容类型标识
    pub created_at: BlockNumber,
    pub is_encrypted: bool,                                   // 是否加密
    pub encryption_scheme: Option<BoundedVec<u8, MaxSchemeLen>>, // 加密算法（如 "aes256-gcm"）
    pub commit: Option<H256>,                                 // 承诺哈希（Commit 模式）
    pub ns: Option<[u8; 8]>,                                  // 命名空间
}
```

### ContentType

```rust
pub enum ContentType {
    Image,     // 图片
    Video,     // 视频
    Document,  // 文档
    Mixed,     // 混合
    Text,      // 纯文本
}
```

### ArchivedEvidence（~50 字节归档摘要）

```rust
pub struct ArchivedEvidence {
    pub id: u64,
    pub domain: u8,
    pub target_id: u64,
    pub content_hash: H256,   // blake2_256(content_cid)
    pub content_type: u8,     // ContentType 编码值
    pub created_at: u32,
    pub archived_at: u32,
    pub year_month: u16,      // YYMM 格式，便于按月统计
}
```

### PrivateContent

```rust
pub struct PrivateContent<T: Config> {
    pub id: u64,
    pub ns: [u8; 8],
    pub subject_id: u64,
    pub cid: BoundedVec<u8, T::MaxCidLen>,
    pub content_hash: H256,                        // 原始内容哈希
    pub encryption_method: u8,                     // 1=AES-256-GCM, 2=ChaCha20-Poly1305
    pub creator: AccountId,
    pub access_policy: AccessPolicy<T>,
    pub encrypted_keys: EncryptedKeyBundles<T>,    // 每用户加密密钥包
    pub created_at: BlockNumber,
    pub updated_at: BlockNumber,
}
```

### AccessPolicy（5 种访问策略）

```rust
pub enum AccessPolicy<T: Config> {
    OwnerOnly,                                          // 仅创建者
    SharedWith(BoundedVec<AccountId, MaxAuthorizedUsers>), // 指定用户列表
    TimeboxedAccess { users, expires_at: BlockNumber },  // 定时访问（到期自动撤销）
    GovernanceControlled,                                // 治理控制
    RoleBased(BoundedVec<u8, 32>),                       // 基于角色（扩展）
}
```

### ManifestStatus

```rust
pub enum ManifestStatus {
    Pending,    // 待处理（可修改）
    Processing, // 处理中（OCW 已获取）
    Confirmed,  // 已确认
    Failed,     // 处理失败
}
```

## 错误

| 错误 | 说明 |
|------|------|
| `NotAuthorized` | 命名空间或账户未被授权 |
| `NotFound` | 目标证据不存在 |
| `InvalidCidFormat` | CID 为空、非 UTF-8 或不符合 IPFS 规范 |
| `DuplicateCid` | 同一提交内存在重复 CID |
| `DuplicateCidGlobal` | 全局 CID 去重命中（Plain 模式，`EnableGlobalCidDedup=true`） |
| `CommitAlreadyExists` | 承诺哈希已被占用 |
| `NamespaceMismatch` | 证据命名空间与操作命名空间不匹配 |
| `RateLimited` | 账户在窗口内达到提交上限 |
| `TooManyForSubject` | 目标/命名空间已达最大证据数 |
| `TooManyImages` | 图片数量超过 `MaxImg` |
| `TooManyVideos` | 视频数量超过 `MaxVid` |
| `TooManyDocs` | 文档数量超过 `MaxDoc` |
| `ParentEvidenceNotFound` | 追加时父证据不存在 |
| `TooManySupplements` | 补充证据超过 100 条上限 |
| `CannotAppendToArchived` | 不能追加到已归档的证据 |
| `PendingManifestNotFound` | 待处理清单不存在 |
| `EditWindowExpired` | 修改窗口已过期 |
| `PendingQueueFull` | 待处理队列已满 |
| `PrivateContentNotFound` | 私密内容不存在 |
| `PublicKeyNotRegistered` | 用户公钥未注册 |
| `AccessDenied` | 无权访问/操作此内容 |
| `CidAlreadyExists` | 私密内容 CID 已存在 |
| `TooManyAuthorizedUsers` | 授权用户数量超过 `MaxAuthorizedUsers` |
| `InvalidEncryptedKey` | 加密密钥格式/长度不合法 |
| `UnsupportedKeyType` | 密钥类型不在 1-3 范围内 |

## Hooks

```
on_idle(now, remaining_weight)
│
├── archive_old_evidences(max=10)          ← 需 remaining > 300M ref_time
│   │  游标扫描: EvidenceArchiveCursor → NextEvidenceId
│   │  条件: now - created_at >= ArchiveDelayBlocks (默认90天)
│   │  操作: 创建 ArchivedEvidence + 清理 Evidences + 二级索引 + CidHashIndex
│   │
│   └── 每项 ≈ 30M ref_time + 2500 proof_size
│
└── cleanup_old_archives(max=5)            ← 需 remaining > 150M ref_time
    │  游标扫描: ArchiveCleanupCursor
    │  条件: now - archived_at > ArchiveTtlBlocks (默认180天)
    │  操作: 删除 ArchivedEvidences 条目
    │
    └── 遇到未过期记录即停止（时间单调递增）
```

## Trait 接口

### EvidenceAuthorizer（权限验证，runtime 注入）

```rust
pub trait EvidenceAuthorizer<AccountId> {
    fn is_authorized(ns: [u8; 8], who: &AccountId) -> bool;
}
```

### EvidenceProvider（只读查询）

```rust
pub trait EvidenceProvider<AccountId> {
    fn get(id: u64) -> Option<()>;
}
```

### PrivateContentProvider（私密内容查询）

```rust
pub trait PrivateContentProvider<AccountId> {
    fn can_access(content_id: u64, user: &AccountId) -> bool;
    fn get_decryption_key(content_id: u64, user: &AccountId) -> Option<Vec<u8>>;
}
```

## 配置参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `MaxContentCidLen` | `Get<u32>` | IPFS 内容 CID 最大长度（建议 64） |
| `MaxSchemeLen` | `Get<u32>` | 加密方案描述最大长度（建议 32） |
| `MaxCidLen` | `Get<u32>` | 媒体 CID 最大长度（旧版兼容） |
| `MaxImg` / `MaxVid` / `MaxDoc` | `Get<u32>` | 图片/视频/文档最大数量 |
| `MaxMemoLen` | `Get<u32>` | 备注最大长度 |
| `MaxAuthorizedUsers` | `Get<u32>` | 私密内容最大授权用户数 |
| `MaxKeyLen` | `Get<u32>` | 密钥最大长度 |
| `MaxPerSubjectTarget` | `Get<u32>` | 每 (domain, target_id) 最大证据数 |
| `MaxPerSubjectNs` | `Get<u32>` | 每 (ns, subject_id) 最大证据数 |
| `WindowBlocks` | `Get<BlockNumber>` | 限频窗口长度（区块数） |
| `MaxPerWindow` | `Get<u32>` | 每窗口最大提交数 |
| `EnableGlobalCidDedup` | `Get<bool>` | 是否启用全局 CID 去重 |
| `MaxListLen` | `Get<u32>` | 列表/队列最大长度 |
| `EvidenceEditWindow` | `Get<BlockNumber>` | 修改窗口（区块数，28800 ≈ 2 天） |
| `ArchiveDelayBlocks` | `Get<u32>` | 归档延迟（1_296_000 ≈ 90 天） |
| `ArchiveTtlBlocks` | `Get<u32>` | 归档记录 TTL（2_592_000 ≈ 180 天，0=不清理） |
| `DefaultStoragePrice` | `Get<Balance>` | IPFS 存储单价（每副本每月） |
| `EvidenceNsBytes` | `Get<[u8; 8]>` | 默认命名空间 |
| `IpfsPinner` | `IpfsPinner<AccountId, Balance>` | IPFS Pin 提供者（runtime 注入） |
| `Authorizer` | `EvidenceAuthorizer<AccountId>` | 权限验证（runtime 注入） |
| `WeightInfo` | `WeightInfo` | 权重信息 |

## CID 加密验证

`cid_validator` 模块通过前缀识别加密 CID，私密内容要求 CID 必须携带加密前缀：

| 前缀 | 类型 |
|------|------|
| `enc-` | 通用加密 |
| `sealed-` | 密封加密 |
| `priv-` | 私有加密 |
| `encrypted-` | 完整前缀 |

## IPFS 内容格式

`content_cid` 指向 IPFS 上的 JSON 文件：

```json
{
  "version": "1.0",
  "evidence_id": 123,
  "domain": 2,
  "target_id": 456,
  "content": {
    "images": ["QmXxx1", "QmXxx2"],
    "videos": ["QmYyy1"],
    "documents": ["QmZzz1"],
    "memo": "文字说明"
  },
  "metadata": {
    "created_at": 1234567890,
    "owner": "5Grwva...",
    "encryption": {
      "enabled": true,
      "scheme": "aes256-gcm",
      "key_bundles": { "..." }
    }
  }
}
```

## 集成示例

```rust
// 在业务 pallet 中实现 EvidenceAuthorizer
impl<T: Config> EvidenceAuthorizer<T::AccountId> for Pallet<T> {
    fn is_authorized(ns: [u8; 8], who: &T::AccountId) -> bool {
        Self::is_participant(ns, who)
    }
}
```

## 安全审计修复

| 编号 | 修复内容 |
|------|---------|
| VC1 | `ContentType` 等 5 种类型添加 `DecodeWithMemTracking` |
| VC2 | 5 个 extrinsic 使用独立 WeightInfo 函数 |
| VH1/VH2 | `commit` / `append_evidence` 添加 CID 边界验证与非空检查 |
| VM1 | 密钥轮换使用 O(1) `KeyRotationCounter` 替代 `iter_prefix` |
| VM2 | 归档时同步清理 `EvidenceByTarget` / `EvidenceByNs` / `CidHashIndex` |
| L-4 | 新增 `cid_validator` 模块，私密内容强制加密 CID 前缀 |

## 相关模块

- [storage/service/](../../storage/service/) — IPFS 存储服务（`IpfsPinner`）
- [storage/lifecycle/](../../storage/lifecycle/) — 存储生命周期（`block_to_year_month`）
- [arbitration/](../arbitration/) — 仲裁系统（引用 evidence_id）
- [escrow/](../escrow/) — 资金托管
