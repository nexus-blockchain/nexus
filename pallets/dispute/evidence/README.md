# pallet-evidence

> 路径：`pallets/dispute/evidence/` · Runtime Index: 61

统一证据管理系统，提供链上证据提交、IPFS 存储、私密内容加密、访问控制、证据密封、自动归档等功能，服务于 8 个业务域的争议与仲裁流程。

## 设计理念

- **CID 化存储**：链上仅存 `content_cid`（~64 字节），实际内容存 IPFS，较旧版降低约 74.5% 存储成本
- **双模式提交**：Plain（公开 CID + 自动 Pin + 全局去重）与 Commit-Reveal（承诺哈希，不暴露明文 CID）
- **低耦合架构**：通过 `EvidenceAuthorizer` trait 与业务 pallet 解耦，runtime 注入实现
- **自动 IPFS Pin**：与 `pallet-storage-service` 集成，证据提交后自动固定 CID
- **三级防膨胀**：证据 90 天自动归档（~50 字节摘要）→ 归档 180 天自动清理 → 过期 PendingManifest 清理
- **双维限频**：账户级（`WindowBlocks` + `MaxPerWindow`）与目标级（`MaxPerSubjectTarget` / `MaxPerSubjectNs`）
- **证据链**：父子追加机制，原证据不可变，补充材料形成可追溯链（最多 100 条）
- **2 天修改窗口**：提交后 `EvidenceEditWindow` 内可修改清单，窗口关闭后不可变
- **证据密封**：仲裁冻结期间密封证据，不可修改/撤回/取消链接
- **端到端加密**：私密内容支持 AES-256-GCM / ChaCha20-Poly1305，每用户独立密钥包，支持密钥轮换

## 架构

```
┌──────────────────────────────────────────────────────────────┐
│                      pallet-evidence                         │
│                                                              │
│  ┌──────────────────┐  ┌──────────────────┐  ┌────────────┐ │
│  │  Evidence Core    │  │ Private Content  │  │  Archive   │ │
│  │ · commit (0)      │  │ · register_key   │  │ · on_idle  │ │
│  │ · commit_hash (1) │  │ · store_private  │  │ · 90d归档  │ │
│  │ · link/unlink     │  │ · grant/revoke   │  │ · 180d清理 │ │
│  │ · append (11)     │  │ · rotate_keys    │  │ · PM过期   │ │
│  │ · manifest (12)   │  │ · request_access │  │            │ │
│  │ · reveal (15)     │  │ · update_policy  │  │            │ │
│  │ · seal/unseal     │  │ · delete_content │  │            │ │
│  │ · withdraw (19)   │  │ · cancel_request │  │            │ │
│  │ · force_remove    │  │ · revoke_key     │  │            │ │
│  │ · force_archive   │  │                  │  │            │ │
│  └────────┬──────────┘  └────────┬─────────┘  └─────┬──────┘ │
│           │                      │                   │        │
│  ┌────────┴──────────────────────┴───────────────────┴─────┐  │
│  │              共享基础设施                                  │  │
│  │  · EvidenceAuthorizer · CidValidator · Rate Limiter     │  │
│  │  · EvidenceProvider   · PrivateContentProvider          │  │
│  └─────────────────────────────────────────────────────────┘  │
└──────────────────────┬───────────────────────────────────────┘
                       │
         ┌─────────────┼─────────────┐
         ▼             ▼             ▼
   ┌───────────┐ ┌──────────┐ ┌───────────┐
   │ storage-  │ │ storage- │ │  crypto-  │
   │ service   │ │lifecycle │ │  common   │
   │ IpfsPinner│ │ YYMM转换 │ │ KeyType/  │
   └───────────┘ └──────────┘ │ AccessPol │
                               └───────────┘
```

## Extrinsics（24 个）

### 证据提交（Plain / Commit-Reveal）

| idx | 方法 | 说明 |
|:---:|------|------|
| 0 | `commit` | 提交公开证据（imgs/vids/docs CID），自动 Pin，全局去重，CID 格式验证 |
| 1 | `commit_hash` | 提交承诺哈希 `H(ns‖subject_id‖cid‖salt‖ver)`，不暴露明文 |
| 11 | `append_evidence` | 追加补充证据，继承父证据 domain/target_id，全局去重，配额检查 |
| 12 | `update_evidence_manifest` | 在 EvidenceEditWindow 内修改待处理证据清单 |
| 15 | `reveal_commitment` | 揭示承诺哈希对应的真实 CID，验证哈希，写入 content_cid，自动 Pin |

### 证据链接

| idx | 方法 | 说明 |
|:---:|------|------|
| 2 | `link` | 按 (domain, target_id) 链接已有证据，检查密封和状态 |
| 3 | `link_by_ns` | 按 (ns, subject_id) 链接，需命名空间匹配，检查密封和状态 |
| 4 | `unlink` | 取消 (domain, target_id) 链接，密封证据不可取消 |
| 5 | `unlink_by_ns` | 取消 (ns, subject_id) 链接，密封证据不可取消 |

### 证据生命周期

| idx | 方法 | 说明 |
|:---:|------|------|
| 16 | `seal_evidence` | 密封证据（仲裁冻结），密封后不可修改/撤回/取消链接 |
| 17 | `unseal_evidence` | 解封证据，恢复 Active 状态 |
| 18 | `force_remove_evidence` | **Root** 强制移除违规证据，清理所有索引/密封/父子关系 |
| 19 | `withdraw_evidence` | 所有者撤回证据，标记 Withdrawn |
| 21 | `force_archive_evidence` | **Root** 强制归档（不等 90 天），创建摘要并清理原始记录 |

### 私密内容管理

| idx | 方法 | 说明 |
|:---:|------|------|
| 6 | `register_public_key` | 注册用户公钥，支持 KeyType 枚举 |
| 7 | `store_private_content` | 存储加密私密内容（CID 必须带加密前缀，含策略 + 密钥包） |
| 8 | `grant_access` | 授予访问权限（仅创建者），更新密钥包，自动清除待处理请求 |
| 9 | `revoke_access` | 撤销访问权限（不可撤销自己），验证用户在列表中 |
| 10 | `rotate_content_keys` | 轮换加密密钥，O(1) 计数器，记录轮换历史 |
| 13 | `request_access` | 请求访问加密内容（需已注册公钥） |
| 14 | `update_access_policy` | 更新访问策略（仅创建者） |
| 20 | `delete_private_content` | 删除私密内容（GDPR），需先 revoke 所有非创建者 |
| 22 | `revoke_public_key` | 撤销用户公钥（密钥泄露时） |
| 23 | `cancel_access_request` | 取消自己的待处理访问请求 |

## 存储（30 项）

### 证据核心

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextEvidenceId` | `StorageValue<u64>` | 证据 ID 自增计数器 |
| `Evidences` | `StorageMap<u64, Evidence>` | 证据主存储 |
| `EvidenceByTarget` | `StorageDoubleMap<(u8,u64), u64, ()>` | (domain, target_id) → evidence_id |
| `EvidenceByNs` | `StorageDoubleMap<([u8;8],u64), u64, ()>` | (ns, subject_id) → evidence_id |
| `EvidenceCountByTarget` | `StorageMap<(u8,u64), u32>` | 按 target 计数（限额） |
| `EvidenceCountByNs` | `StorageMap<([u8;8],u64), u32>` | 按 ns 计数（限额） |
| `CommitIndex` | `StorageMap<H256, u64>` | 承诺哈希 → evidence_id（reveal/archive 时清理） |
| `CidHashIndex` | `StorageMap<H256, u64>` | blake2_256(cid) → evidence_id 全局去重 |
| `AccountWindows` | `StorageMap<AccountId, WindowInfo>` | 限频窗口 |

### 证据状态与密封

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `EvidenceStatuses` | `StorageMap<u64, EvidenceStatus>` | 证据状态（默认 Active） |
| `SealedEvidences` | `StorageMap<u64, AccountId>` | 已密封证据 → 密封者 |

### 证据追加链

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `EvidenceParent` | `StorageMap<u64, u64>` | 子 → 父映射 |
| `EvidenceChildren` | `StorageMap<u64, BoundedVec<u64, 100>>` | 父 → 子列表 |

### 待处理清单

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `PendingManifests` | `StorageMap<u64, PendingManifest>` | evidence_id → 待处理清单 |
| `PendingManifestQueue` | `StorageValue<BoundedVec<u64>>` | 待处理队列（OCW） |
| `PendingManifestCleanupCursor` | `StorageValue<u64>` | 过期清理游标 |

### 私密内容

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextPrivateContentId` | `StorageValue<u64>` | ID 计数器 |
| `PrivateContents` | `StorageMap<u64, PrivateContent>` | 主存储 |
| `PrivateContentByCid` | `StorageMap<BoundedVec<u8>, u64>` | CID 去重索引 |
| `PrivateContentBySubject` | `StorageDoubleMap<([u8;8],u64), u64, ()>` | 主体索引 |
| `UserPublicKeys` | `StorageMap<AccountId, UserPublicKey>` | 用户公钥 |
| `AccessRequests` | `StorageDoubleMap<u64, AccountId, BlockNumber>` | 访问请求 |
| `KeyRotationHistory` | `StorageDoubleMap<u64, u32, KeyRotationRecord>` | 轮换记录 |
| `KeyRotationCounter` | `StorageMap<u64, u32>` | 轮换计数（O(1)） |

### 归档

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `ArchivedEvidences` | `StorageMap<u64, ArchivedEvidence>` | 归档摘要（~50 字节） |
| `EvidenceArchiveCursor` | `StorageValue<u64>` | 归档扫描游标 |
| `ArchiveStats` | `StorageValue<ArchiveStatistics>` | 全局统计 |
| `ArchiveCleanupCursor` | `StorageValue<u64>` | 清理游标 |

## 事件（26 个）

### 证据核心

| 事件 | 说明 |
|------|------|
| `EvidenceCommitted { id, domain, target_id, owner }` | 公开证据提交 |
| `EvidenceCommittedV2 { id, ns, subject_id, owner }` | 按命名空间提交 |
| `EvidenceLinked / EvidenceLinkedV2` | 证据已链接 |
| `EvidenceUnlinked / EvidenceUnlinkedV2` | 证据已取消链接 |
| `EvidenceAppended { id, parent_id, domain, target_id, owner }` | 补充证据追加 |
| `EvidenceManifestUpdated { evidence_id, owner }` | 清单已更新 |
| `EvidenceManifestConfirmed { evidence_id, manifest_cid }` | 清单已确认 |
| `EvidenceThrottled(AccountId, u8)` | 限频触发（1=Rate, 2=Quota） |
| `EvidenceQuotaReached(u8, u64)` | 配额上限（0=target, 1=ns） |

### 证据生命周期

| 事件 | 说明 |
|------|------|
| `CommitmentRevealed { id, ns, subject_id, content_cid, owner }` | 承诺已揭示 |
| `EvidenceSealed / EvidenceUnsealed` | 密封 / 解封 |
| `EvidenceWithdrawn { id, owner }` | 已撤回 |
| `EvidenceForceRemoved { id, domain, target_id }` | Root 强制移除 |
| `EvidenceForceArchived { id, domain, target_id }` | Root 强制归档 |
| `EvidenceArchived { id, domain, target_id }` | 自动归档 |

### 私密内容

| 事件 | 说明 |
|------|------|
| `PublicKeyRegistered { user, key_type }` | 公钥注册 |
| `PublicKeyRevoked { user }` | 公钥撤销 |
| `PrivateContentStored { content_id, ns, subject_id, cid, creator }` | 内容存储 |
| `PrivateContentDeleted { content_id, deleted_by }` | 内容删除 |
| `AccessGranted / AccessRevoked` | 权限授予 / 撤销 |
| `AccessRequested / AccessRequestCancelled` | 请求 / 取消请求 |
| `AccessPolicyUpdated { content_id, updated_by }` | 策略更新 |
| `KeysRotated { content_id, rotation_round, rotated_by }` | 密钥轮换 |

## 错误（31 个）

| 错误 | 说明 |
|------|------|
| `NotAuthorized` | 命名空间或账户未被授权 |
| `NotFound` | 目标证据不存在 |
| `InvalidCidFormat` | CID 为空/非 UTF-8/不符 IPFS 规范/媒体全空 |
| `DuplicateCid` | 提交内 CID 重复 |
| `DuplicateCidGlobal` | 全局 CID 去重命中 |
| `CommitAlreadyExists` | 承诺哈希已被占用 |
| `CommitNotFound` | reveal 时承诺不存在 |
| `CommitMismatch` | 揭示数据与承诺不匹配 |
| `AlreadyRevealed` | 不可重复揭示 |
| `NamespaceMismatch` | 命名空间不匹配 |
| `RateLimited` | 窗口内达到提交上限 |
| `TooManyForSubject` | 目标已达最大证据数 |
| `TooManyImages / TooManyVideos / TooManyDocs` | 媒体数量超限 |
| `ParentEvidenceNotFound` | 父证据不存在 |
| `TooManySupplements` | 补充超 100 条 |
| `CannotAppendToArchived` | 不能追加到已归档证据 |
| `PendingManifestNotFound` | 待处理清单不存在 |
| `EditWindowExpired` | 修改窗口已过期 |
| `PendingQueueFull` | 待处理队列已满 |
| `EvidenceSealed` | 证据已密封 |
| `EvidenceNotSealed` | 证据未密封（解封时） |
| `EvidenceWithdrawn` | 证据已撤回 |
| `InvalidEvidenceStatus` | 状态不允许此操作 |
| `PrivateContentNotFound` | 私密内容不存在 |
| `PublicKeyNotRegistered` | 公钥未注册 |
| `AccessDenied` | 无权操作 |
| `CidAlreadyExists` | 私密内容 CID 已存在 |
| `TooManyAuthorizedUsers` | 授权用户超限 |
| `InvalidEncryptedKey` | 密钥格式不合法 |
| `ContentHasActiveUsers` | 删除前需先 revoke 所有用户 |
| `AlreadyRequested / AlreadyAuthorized / SelfAccessRequest / AccessRequestNotFound` | 访问请求相关 |

## Hooks

```
on_idle(now, remaining_weight)
│
├── archive_old_evidences(max=10)
│   ├── 游标: EvidenceArchiveCursor → NextEvidenceId
│   ├── 跳过已密封证据
│   ├── 条件: now - created_at >= ArchiveDelayBlocks (90天)
│   ├── 操作: ArchivedEvidence + 清理全部索引/CommitIndex/父子关系/密封/状态
│   ├── 扫描开销: 10M × 10 = 100M ref_time
│   └── 每项归档: 30M ref_time + 2500 proof_size
│
├── cleanup_old_archives(max=5)
│   ├── 游标: ArchiveCleanupCursor
│   ├── 条件: now - archived_at > ArchiveTtlBlocks (180天, 0=不清理)
│   └── 遇到未过期记录即停止
│
└── cleanup_expired_pending_manifests(max=5)
    ├── 游标: PendingManifestCleanupCursor
    └── 条件: now > created_at + EvidenceEditWindow
```

## Trait 接口

### EvidenceAuthorizer（权限验证，runtime 注入）

```rust
pub trait EvidenceAuthorizer<AccountId> {
    fn is_authorized(ns: [u8; 8], who: &AccountId) -> bool;
}
```

### EvidenceProvider（只读查询，供其他 pallet 使用）

```rust
pub trait EvidenceProvider<AccountId> {
    fn get(id: u64) -> Option<EvidenceInfo<AccountId>>;
    fn exists(id: u64) -> bool;
    fn get_status(id: u64) -> Option<EvidenceStatus>;
    fn is_active(id: u64) -> bool;  // 默认实现
}
```

### PrivateContentProvider（定义于 pallet-crypto-common）

```rust
pub trait PrivateContentProvider<AccountId> {
    fn can_access(content_id: u64, user: &AccountId) -> bool;
    fn get_encrypted_key(content_id: u64, user: &AccountId) -> Option<Vec<u8>>;
    fn get_decryption_info(content_id: u64, user: &AccountId)
        -> Option<(Vec<u8>, H256, EncryptionMethod, Vec<u8>)>;
    fn get_content_status(content_id: u64) -> Option<ContentStatus>;
    fn get_content_creator(content_id: u64) -> Option<AccountId>;
}
```

## 只读查询方法

| 方法 | 说明 |
|------|------|
| `list_ids_by_target(domain, target_id, start_id, limit)` | 按 target 分页列出 evidence_id |
| `list_ids_by_ns(ns, subject_id, start_id, limit)` | 按 ns 分页列出 evidence_id |
| `count_by_target / count_by_ns` | 获取证据数量 |
| `can_access_private_content(content_id, user)` | 检查访问权限 |
| `get_encrypted_key_for_user(content_id, user)` | 获取用户密钥包 |
| `get_private_content_by_cid(cid)` | 通过 CID 查找内容 |
| `get_private_content_ids_by_subject(ns, subject_id)` | 获取主体下所有内容 ID |
| `get_decryption_info(content_id, user)` | 解密信息 (cid, hash, method, key) |
| `get_content_metadata(content_id)` | 公开元数据（不含密钥） |
| `list_access_requests(content_id)` | 待处理访问请求 |
| `get_user_public_key(user)` | 用户公钥 |
| `compute_evidence_commitment / verify_evidence_commitment` | 承诺哈希计算/验证 |

## 主要类型

### Evidence

```rust
pub struct Evidence<AccountId, BlockNumber, MaxContentCidLen, MaxSchemeLen> {
    pub id: u64,
    pub domain: u8,                                           // 业务域
    pub target_id: u64,
    pub owner: AccountId,
    pub content_cid: BoundedVec<u8, MaxContentCidLen>,        // IPFS CID
    pub content_type: ContentType,                            // Image/Video/Document/Mixed/Text
    pub created_at: BlockNumber,
    pub is_encrypted: bool,
    pub encryption_scheme: Option<BoundedVec<u8, MaxSchemeLen>>,
    pub commit: Option<H256>,                                 // 揭示后清除
    pub ns: Option<[u8; 8]>,
}
```

### EvidenceStatus / ContentType

```rust
pub enum EvidenceStatus { Active, Withdrawn, Sealed, Removed }
pub enum ContentType { Image, Video, Document, Mixed, Text }
```

### ArchivedEvidence（~50 字节）

```rust
pub struct ArchivedEvidence {
    pub id: u64, pub domain: u8, pub target_id: u64,
    pub content_hash: H256,  // blake2_256(content_cid)
    pub content_type: u8, pub created_at: u32, pub archived_at: u32,
    pub year_month: u16,     // YYMM 格式
}
```

### AccessPolicy（5 种，定义于 pallet-crypto-common）

```rust
pub enum AccessPolicy<AccountId, BlockNumber, MaxAuthorizedUsers> {
    OwnerOnly,
    SharedWith(BoundedVec<AccountId, MaxAuthorizedUsers>),
    TimeboxedAccess { users, expires_at: BlockNumber },
    GovernanceControlled,      // 委托给 Authorizer
    RoleBased(BoundedVec<u8, ConstU32<32>>),
}
```

### EvidenceInfo（跨 pallet 轻量查询）

```rust
pub struct EvidenceInfo<AccountId> {
    pub id: u64, pub owner: AccountId, pub domain: u8,
    pub target_id: u64, pub is_encrypted: bool, pub status: EvidenceStatus,
}
```

## 配置参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `MaxContentCidLen` | `Get<u32>` | IPFS CID 最大长度（建议 64） |
| `MaxSchemeLen` | `Get<u32>` | 加密方案描述最大长度（建议 32） |
| `MaxCidLen` | `Get<u32>` | 媒体 CID 最大长度 |
| `MaxImg` / `MaxVid` / `MaxDoc` | `Get<u32>` | 图片/视频/文档最大数量 |
| `MaxMemoLen` | `Get<u32>` | 备注最大长度 |
| `MaxAuthorizedUsers` | `Get<u32>` | 私密内容最大授权用户数 |
| `MaxKeyLen` | `Get<u32>` | 密钥最大长度 |
| `MaxPerSubjectTarget` | `Get<u32>` | 每 (domain, target_id) 最大证据数 |
| `MaxPerSubjectNs` | `Get<u32>` | 每 (ns, subject_id) 最大证据数 |
| `WindowBlocks` | `Get<BlockNumber>` | 限频窗口（区块数） |
| `MaxPerWindow` | `Get<u32>` | 每窗口最大提交数 |
| `EnableGlobalCidDedup` | `Get<bool>` | 是否启用全局 CID 去重 |
| `MaxListLen` | `Get<u32>` | 列表/队列最大长度 |
| `EvidenceEditWindow` | `Get<BlockNumber>` | 修改窗口（28800 ≈ 2 天） |
| `ArchiveDelayBlocks` | `Get<u32>` | 归档延迟（1_296_000 ≈ 90 天） |
| `ArchiveTtlBlocks` | `Get<u32>` | 归档 TTL（2_592_000 ≈ 180 天，0=不清理） |
| `DefaultStoragePrice` | `Get<Balance>` | IPFS 存储单价 |
| `EvidenceNsBytes` | `Get<[u8; 8]>` | 默认命名空间 |
| `IpfsPinner` | `IpfsPinner<AccountId, Balance>` | IPFS Pin 提供者 |
| `Authorizer` | `EvidenceAuthorizer<AccountId>` | 权限验证 |
| `Balance` | `AtLeast32BitUnsigned + ...` | 余额类型 |
| `WeightInfo` | `WeightInfo` | 权重信息（21 个函数） |

## CID 加密验证

`cid_validator` 模块通过前缀识别加密 CID，`store_private_content` 强制要求加密前缀，
然后剥离前缀后验证底层 IPFS CID 格式：

| 前缀 | 类型 |
|------|------|
| `enc-` | 通用加密 |
| `sealed-` | 密封加密 |
| `priv-` | 私有加密 |
| `encrypted-` | 完整前缀 |

## 权重（WeightInfo trait，21 个函数）

所有 extrinsic 使用独立权重函数，精确计算 reads/writes/proof_size。
两套实现：`SubstrateWeight<T>`（生产）与 `()`（测试 / 默认 RocksDbWeight）。

关键权重参考：
- `commit(i,v,d)`: 8M + 2M×N ref_time, 2+N reads, 5+N writes
- `force_remove_evidence`: 80M ref_time, 3 reads, 13 writes
- `force_archive_evidence`: 80M ref_time, 3 reads, 14 writes
- `store_private_content`: 80M ref_time, 5 reads, 4 writes

## 集成示例

```rust
// 在业务 pallet 中实现 EvidenceAuthorizer
impl<T: Config> EvidenceAuthorizer<T::AccountId> for Pallet<T> {
    fn is_authorized(ns: [u8; 8], who: &T::AccountId) -> bool {
        Self::is_participant(ns, who)
    }
}
```

## 安全审计修复历史

### Phase 1-2（初始审计）

| 编号 | 修复内容 |
|------|---------|
| VC1 | `ContentType` 等 5 种类型添加 `DecodeWithMemTracking` |
| VC2 | 5 个 extrinsic 使用独立 WeightInfo 函数（替代硬编码 10_000） |
| VH1/VH2 | `commit` / `append_evidence` 添加 imgs/vids/docs 边界验证与非空检查 |
| VH3 | `update_evidence_manifest` 的 vids/docs 边界验证 |
| VM1 | 密钥轮换使用 O(1) `KeyRotationCounter` 替代 `iter_prefix` O(N) |
| VM2 | 归档时同步清理 EvidenceByTarget / EvidenceByNs / CidHashIndex 等索引 |
| L-4 | 新增 `cid_validator` 模块，私密内容强制加密 CID 前缀 |

### Round 3

| 编号 | 修复内容 |
|------|---------|
| H1-R3 | `CommitIndex` 在 reveal/archive/force_archive 后清理（防存储泄漏） |
| M1-R3 | `append_evidence` 全局 CID 去重 + CidHashIndex 注册 |
| M2-R3 | `link`/`link_by_ns` 添加密封和状态检查（与 unlink 对称） |
| M3-R3 | `archive_old_evidences` 清理子证据父引用 + 自身 children 列表 |
| M4-R3 | `on_idle` 权重补充扫描开销（per_scan_weight） |
| M5-R3 | force_remove/force_archive/delete_private_content 独立权重函数 |

### Round 4

| 编号 | 修复内容 |
|------|---------|
| H1-R4 | `force_archive_evidence` 清理子证据父引用 + 自身 children 列表 |
| M1-R4 | `link`/`link_by_ns` 权重 reads 修正（1/2→3） |
| M2-R4 | `reveal_commitment` 权重 writes 修正（2→3，含 CommitIndex） |
| M3-R4 | `append_evidence` 添加 MaxPerSubjectTarget 配额检查 |
| M4-R4 | `rotate_content_keys` 权重 writes 修正（2→3，含 KeyRotationCounter） |

### Round 5

| 编号 | 修复内容 |
|------|---------|
| M1-R5 | `store_private_content` 权重 writes 3→4（含 PrivateContentBySubject） |
| M2-R5 | `grant_access` 权重 writes 1→2（含 AccessRequests::remove） |
| M3-R5 | `force_archive_evidence` 权重 reads 2→3（含 EvidenceChildren::get） |

### 测试覆盖

共 46 个单元/回归测试，覆盖所有 extrinsic 和关键审计修复。

## 相关模块

- [storage/service/](../../storage/service/) — IPFS 存储服务（`IpfsPinner`）
- [storage/lifecycle/](../../storage/lifecycle/) — 存储生命周期（`block_to_year_month`）
- [crypto/](../../common/crypto/) — 加密类型（`pallet-crypto-common`）
- [arbitration/](../arbitration/) — 仲裁系统（引用 evidence_id）
- [escrow/](../escrow/) — 资金托管
