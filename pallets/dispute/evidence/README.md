# pallet-evidence

> 路径：`pallets/dispute/evidence/` · Runtime Index: 61

统一证据管理系统，提供链上证据提交、IPFS 存储、私密内容加密、访问控制等功能。

## 设计理念

- **CID 化设计**：链上仅存 `content_cid`，实际内容存 IPFS，降低 74.5% 存储成本
- **双模式支持**：Plain（公开 CID）+ Commit（承诺哈希）
- **低耦合架构**：通过 `EvidenceAuthorizer` trait 实现模块解耦
- **自动 IPFS Pin**：与 `pallet-storage-service` 集成
- **存储归档**：90 天自动归档，降低约 75% 存储（`ArchivedEvidence` ~50 字节）
- **限频控制**：账户级（`WindowBlocks`+`MaxPerWindow`）+ 目标级（`MaxPerSubjectTarget`）

## Extrinsics

### 证据提交
| call_index | 方法 | 说明 |
|:---:|------|------|
| 0 | `commit` | 提交公开证据（imgs/vids/docs CID，自动 IPFS Pin） |
| 1 | `commit_hash` | 提交承诺哈希（Commit 模式，H(ns‖subject_id‖cid_enc‖salt‖ver)） |
| 11 | `append_evidence` | 追加补充证据（创建子证据链） |
| 12 | `update_evidence_manifest` | 修改窗口内更新证据清单 |

### 证据链接
| call_index | 方法 | 说明 |
|:---:|------|------|
| 2 | `link` | 链接已有证据到目标（domain, target_id） |
| 3 | `link_by_ns` | 按命名空间链接（ns, subject_id） |
| 4 | `unlink` | 取消链接 |
| 5 | `unlink_by_ns` | 按命名空间取消链接 |

### 私密内容管理
| call_index | 方法 | 说明 |
|:---:|------|------|
| 6 | `register_public_key` | 注册用户公钥（加密密钥包用） |
| 7 | `store_private_content` | 存储加密私密内容 |
| 8 | `grant_access` | 授予访问权限 |
| 9 | `revoke_access` | 撤销访问权限 |
| 10 | `rotate_content_keys` | 轮换加密密钥（O(1) 计数器） |

## 存储

### 核心存储
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextEvidenceId` | `u64` | 证据 ID 计数器 |
| `Evidences` | `Map<u64, Evidence>` | 证据主存储 |
| `EvidenceByTarget` | `DoubleMap<(u8,u64), u64, ()>` | 按 (domain, target_id) 索引 |
| `EvidenceByNs` | `DoubleMap<([u8;8],u64), u64, ()>` | 按命名空间索引 |
| `EvidenceCountByTarget` | `Map<(u8,u64), u32>` | 按目标计数 |
| `EvidenceCountByNs` | `Map<([u8;8],u64), u32>` | 按命名空间计数 |
| `CommitIndex` | `Map<H256, u64>` | 承诺哈希去重索引 |
| `CidHashIndex` | `Map<H256, u64>` | CID 全局去重索引 |
| `AccountWindows` | `Map<AccountId, WindowInfo>` | 账户级限频窗口 |

### 证据追加链
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `EvidenceParent` | `Map<u64, u64>` | 子证据 → 父证据 |
| `EvidenceChildren` | `Map<u64, BoundedVec<u64>>` | 父证据 → 子证据列表 |
| `PendingManifests` | `Map<u64, PendingManifest>` | 待处理清单 |
| `PendingManifestQueue` | `BoundedVec<u64>` | 待处理队列 |

### 私密内容存储
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextPrivateContentId` | `u64` | 私密内容 ID 计数器 |
| `PrivateContents` | `Map<u64, PrivateContent>` | 私密内容主存储 |
| `PrivateContentByCid` | `Map<CID, u64>` | 按 CID 索引 |
| `PrivateContentBySubject` | `DoubleMap<(u8,u64), u64, ()>` | 按主体索引 |
| `UserPublicKeys` | `Map<AccountId, UserPublicKey>` | 用户公钥 |
| `KeyRotationHistory` | `DoubleMap<u64, u32, ...>` | 密钥轮换历史 |
| `KeyRotationCounter` | `Map<u64, u32>` | 轮换计数器（O(1)） |

### 归档
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `ArchivedEvidences` | `Map<u64, ArchivedEvidence>` | 归档摘要（~50 字节） |
| `EvidenceArchiveCursor` | `u64` | 归档扫描游标 |
| `ArchiveStats` | `ArchiveStatistics` | 归档统计 |

## 主要类型

### Evidence（CID 化证据记录）
```rust
pub struct Evidence<AccountId, BlockNumber, MaxContentCidLen, MaxSchemeLen> {
    pub id: u64,
    pub owner: AccountId,
    pub domain: u8,
    pub target_id: u64,
    pub content_cid: BoundedVec<u8, MaxContentCidLen>,
    pub content_type: ContentType,
    pub encryption_scheme: Option<BoundedVec<u8, MaxSchemeLen>>,
    pub created_at: BlockNumber,
    pub commit: Option<H256>,
    pub ns: Option<[u8; 8]>,
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

## Trait 接口

### EvidenceAuthorizer（权限验证）
```rust
pub trait EvidenceAuthorizer<AccountId> {
    /// 校验账户是否在给定命名空间下被授权提交/链接证据
    fn is_authorized(ns: [u8; 8], who: &AccountId) -> bool;
}
```

### EvidenceProvider（只读查询）
```rust
pub trait EvidenceProvider<AccountId> {
    fn get(id: u64) -> Option<()>;
}
```

## 配置参数

| 参数 | 说明 |
|------|------|
| `MaxContentCidLen` | 内容 CID 最大长度（建议 64） |
| `MaxSchemeLen` | 加密方案描述最大长度（建议 32） |
| `MaxCidLen` | CID 最大长度（旧版兼容） |
| `MaxImg` / `MaxVid` / `MaxDoc` | 图片/视频/文档最大数量 |
| `MaxMemoLen` | 备注最大长度 |
| `MaxAuthorizedUsers` | 最大授权用户数 |
| `MaxKeyLen` | 密钥最大长度 |
| `MaxPerSubjectTarget` | 每目标最大证据数 |
| `MaxPerSubjectNs` | 每命名空间最大证据数 |
| `WindowBlocks` | 限频窗口（区块数） |
| `MaxPerWindow` | 每窗口最大提交数 |
| `EnableGlobalCidDedup` | 是否启用全局 CID 去重 |
| `MaxListLen` | 列表最大长度 |
| `EvidenceEditWindow` | 修改窗口（区块数） |
| `DefaultStoragePrice` | IPFS 存储单价 |
| `IpfsPinner` | IPFS Pin 提供者（runtime 注入） |
| `Authorizer` | 权限验证（runtime 注入） |
| `WeightInfo` | 权重信息 |

## IPFS 内容格式

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
  }
}
```

## 集成示例

```rust
// 在业务 pallet 中实现 EvidenceAuthorizer
impl<T: Config> EvidenceAuthorizer<T::AccountId> for Pallet<T> {
    fn is_authorized(ns: [u8; 8], who: &T::AccountId) -> bool {
        // 验证是否为授权参与方
        Self::is_participant(ns, who)
    }
}
```

## 相关模块

- [storage/service/](../../storage/service/) — IPFS 存储服务（IpfsPinner）
- [arbitration/](../arbitration/) — 仲裁系统（引用证据 ID）
