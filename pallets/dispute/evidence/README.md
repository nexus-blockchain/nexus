# pallet-dispute-evidence

链上证据管理模块。支持 CID 化存储、双模式提交（Plain / Commit-Reveal）、IPFS 自动 Pin、加密内容管理、
证据追加链、密封冻结（仲裁隔离）、速率限制、全局 CID 去重、押金机制、自动归档与清理。

---

## 设计原则

| 原则 | 说明 |
|------|------|
| CID 化 | 链上仅存 `content_cid`（~64B），实际媒体在 IPFS |
| 低耦合 | 通过 `EvidenceAuthorizer` / `EvidenceSealAuthorizer` 与 runtime 解耦 |
| 押金机制 | 提交证据预留押金，撤回/归档退还，force_remove 没收 |
| 双权限 | 提交权限 (`Authorizer`) 与密封权限 (`SealAuthorizer`) 隔离 |
| 速率限制 | `WindowBlocks + MaxPerWindow` 滑窗 + `MaxPerSubjectTarget/Ns` 配额 |
| 链接限数 | 单证据最大 `MaxLinksPerEvidence` 链接目标 |
| 承诺超时 | `CommitRevealDeadline` 内未揭示则 `on_idle` 自动清理 |

---

## 架构

```
┌─────────────────── pallet-dispute-evidence ───────────────────┐
│                                                        │
│  Extrinsics (24)                                       │
│  ├── commit (Plain 提交, 自定义 ns)                     │
│  ├── commit_hash (Commit-Reveal 提交)                   │
│  ├── commit_v2 (单一 manifest CID 模式)                 │
│  ├── link / link_by_ns (链接到目标)                      │
│  ├── unlink / unlink_by_ns (取消链接)                    │
│  ├── append_evidence (所有者追加补充材料)                  │
│  ├── reveal_commitment (揭示承诺内容)                     │
│  ├── seal_evidence / unseal_evidence (SealAuthorizer)    │
│  ├── withdraw_evidence (撤回+退押金+清索引)               │
│  ├── force_remove_evidence (Root 强制移除+没收押金)        │
│  ├── force_archive_evidence (Root 强制归档+退押金)         │
│  └── 加密内容管理 (9个, 含 revoke_public_key/cancel_req) │
│                                                         │
│  on_idle                                                │
│  ├── archive_old_evidences (游标归档+unpin+退押金)         │
│  ├── cleanup_old_archives (清理过期归档)                   │
│  ├── cleanup_unrevealed_commitments (清理超时承诺)         │
│  └── cleanup_expired_access_requests (游标清理过期请求)    │
│                                                         │
│  Traits (输出)                                           │
│  ├── EvidenceProvider<AccountId>                         │
│  └── PrivateContentProvider<AccountId>                   │
│                                                         │
│  Traits (输入, runtime 注入)                              │
│  ├── EvidenceAuthorizer<AccountId>                       │
│  ├── EvidenceSealAuthorizer<AccountId>                    │
│  ├── StoragePin (IPFS pin/unpin)                         │
│  └── Currency (ReservableCurrency, 押金)                  │
└─────────────────────────────────────────────────────────┘
```

---

## Extrinsics

### 证据提交

| # | 名称 | 签名 | 说明 |
|---|------|------|------|
| 0 | `commit` | signed | Plain 模式提交。需传入 `ns`。自动推断 content_type。预留押金。同步写 Target 和 Ns 索引。 |
| 1 | `commit_hash` | signed | Commit-Reveal 模式。先提交哈希承诺，后续 reveal 揭示。预留押金。content_type 默认 Document。 |
| 24 | `commit_v2` | signed | 单一 manifest CID 模式。直接传入 IPFS manifest CID 和内容类型，替代旧版 commit。 |
| 11 | `append_evidence` | signed | 向已有证据追加补充材料。仅原始所有者可调用。预留押金。受 `MaxSupplements` 限制。 |
| 15 | `reveal_commitment` | signed | 揭示之前的 commit_hash 承诺内容。 |

### 链接管理

| # | 名称 | 签名 | 说明 |
|---|------|------|------|
| 2 | `link` | signed | 按 (domain, target_id) 链接证据。受 `MaxLinksPerEvidence` 限制，检查重复。 |
| 3 | `link_by_ns` | signed | 按 (ns, subject_id) 链接证据。受 `MaxLinksPerEvidence` 限制，检查重复。 |
| 4 | `unlink` | signed | 取消 (domain, target_id) 链接。递减 LinkCount。密封证据不可取消。 |
| 5 | `unlink_by_ns` | signed | 取消 (ns, subject_id) 链接。递减 LinkCount。密封证据不可取消。 |

### 生命周期管理

| # | 名称 | Origin | 说明 |
|---|------|--------|------|
| 16 | `seal_evidence` | signed | 密封证据（仲裁冻结）。需 `SealAuthorizer` 授权。 |
| 17 | `unseal_evidence` | signed | 解封证据。需 `SealAuthorizer` 授权。 |
| 18 | `force_remove_evidence` | Root | 强制移除违规内容。没收押金。调用 unpin。 |
| 19 | `withdraw_evidence` | signed | 所有者主动撤回。退还押金。完整清理所有索引（Target/Ns/CidHash/Commit/Parent-Children/LinkCount）。调用 unpin。 |
| 21 | `force_archive_evidence` | Root | 强制归档。退还押金。调用 unpin。 |

### 加密内容管理

| # | 名称 | 签名 | 说明 |
|---|------|------|------|
| 6 | `register_public_key` | signed | 注册用户公钥 |
| 7 | `store_private_content` | signed | 存储加密内容 |
| 8 | `grant_access` | signed | 授权用户解密（自动清除待处理请求） |
| 9 | `revoke_access` | signed | 撤销解密授权 |
| 10 | `rotate_content_keys` | signed | 轮换加密密钥（历史仅通过事件记录） |
| 13 | `request_access` | signed | 请求解密访问（受 `MaxPendingRequestsPerContent` 限制） |
| 14 | `update_access_policy` | signed | 更新访问策略 |
| 20 | `delete_private_content` | signed | 删除加密内容。调用 unpin 释放 IPFS 存储。 |
| 22 | `revoke_public_key` | signed | 撤销公钥 |
| 23 | `cancel_access_request` | signed | 取消访问请求 |

---

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextEvidenceId` | `u64` | 全局自增 ID |
| `Evidences` | `Map<u64, Evidence>` | 核心证据记录 |
| `EvidenceByTarget` | `DoubleMap<(u8,u64), u64, ()>` | domain+target → id 索引 |
| `EvidenceByNs` | `DoubleMap<([u8;8],u64), u64, ()>` | ns+subject → id 索引 |
| `EvidenceStatuses` | `Map<u64, EvidenceStatus>` | 统一状态（Active/Sealed/Withdrawn/Removed） |
| `EvidenceCountByTarget` | `Map<(u8,u64), u32>` | 配额计数 |
| `EvidenceCountByNs` | `Map<([u8;8],u64), u32>` | 配额计数 |
| `EvidenceLinkCount` | `Map<u64, u32>` | 每条证据的链接计数 |
| `EvidenceDeposits` | `Map<u64, Balance>` | 押金记录 |
| `CommitIndex` | `Map<H256, u64>` | 承诺 hash → evidence_id |
| `CidHashIndex` | `Map<H256, u64>` | CID hash → id（全局去重） |
| `AccountWindows` | `Map<AccountId, (BlockNumber, u32)>` | 速率限制窗口 |
| `EvidenceParent` | `Map<u64, u64>` | 子→父关系 |
| `EvidenceChildren` | `Map<u64, Vec<u64>>` | 父→子列表 |
| `ArchivedEvidences` | `Map<u64, ArchivedEvidence>` | 归档摘要 |
| `ArchiveStats` | `ArchiveStatistics` | 归档统计 |
| `EvidenceArchiveCursor` | `u64` | 归档游标 |
| `ArchiveCleanupCursor` | `u64` | 归档清理游标 |
| `CommitCleanupCursor` | `u64` | 承诺超时清理游标（独立于归档游标） |
| `AccessRequestCount` | `Map<u64, u32>` | 每个内容的待处理请求计数 |
| `AccessRequestCleanupCursor` | `u64` | 访问请求清理游标 |
| `EvidenceByOwner` | `DoubleMap<AccountId, u64, ()>` | 按所有者索引证据 |
| `PrivateContentDeposits` | `Map<u64, Balance>` | 私密内容押金记录 |
| 加密内容存储 (5项) | — | PrivateContents, UserPublicKeys, AccessRequests, PrivateContentByCid, PrivateContentBySubject |

---

## Config

| 参数 | 类型 | 说明 |
|------|------|------|
| `MaxContentCidLen` | `u32` | CID 最大长度（建议 64），与 `MaxCidLen` 保持一致 |
| `MaxCidLen` | `u32` | 媒体 CID 最大长度（向后兼容，值应与 MaxContentCidLen 相同） |
| `MaxSchemeLen` | `u32` | 加密方案描述最大长度 |
| `Authorizer` | trait | 证据提交/链接权限验证 |
| `SealAuthorizer` | trait | 密封/解封权限验证（仲裁角色隔离） |
| `Currency` | trait | `ReservableCurrency`，用于押金 |
| `EvidenceDeposit` | `Balance` | 每条证据押金金额 |
| `CommitRevealDeadline` | `BlockNumber` | 承诺揭示截止期限（默认 ~7天） |
| `MaxLinksPerEvidence` | `u32` | 单条证据最大链接数 |
| `MaxSupplements` | `u32` | 每条父证据最大追加补充材料数 |
| `MaxPendingRequestsPerContent` | `u32` | 每个内容最大待处理访问请求数 |
| `MaxPerSubjectTarget` | `u32` | 每 target 最大证据数 |
| `MaxPerSubjectNs` | `u32` | 每 ns 最大证据数 |
| `WindowBlocks` | `BlockNumber` | 速率限制滑窗大小 |
| `MaxPerWindow` | `u32` | 滑窗内最大提交数 |
| `EnableGlobalCidDedup` | `bool` | 是否启用全局 CID 去重 |
| `StoragePin` | trait | IPFS Pin/Unpin 接口 |
| `PrivateContentDeposit` | `Balance` | 私密内容押金金额 |
| `AccessRequestTtlBlocks` | `BlockNumber` | 访问请求 TTL（过期后 on_idle 自动清理） |
| `MaxReasonLen` | `u32` | 密封/强制操作原因最大长度 |
| `ArchiveTtlBlocks` | `u32` | 归档记录 TTL |
| `ArchiveDelayBlocks` | `u32` | 证据归档延迟 |

---

## Events

共 26 个事件，覆盖证据生命周期、加密内容管理、押金操作、承诺超时。

关键新增事件：
- `EvidenceDepositReserved` / `EvidenceDepositRefunded` / `EvidenceDepositSlashed` — 押金三态
- `CommitmentExpired` — 承诺超时清理

---

## Errors

共 31 个错误码。

关键新增：
- `TooManyLinks` — 超过 `MaxLinksPerEvidence`
- `DuplicateLink` — 链接已存在（防重复）
- `TooManyAccessRequests` — 超过 `MaxPendingRequestsPerContent`
- `InsufficientDeposit` — 押金余额不足

---

## on_idle Hook

每个空闲区块最多执行四个清理任务（受剩余权重限制）：

1. **archive_old_evidences** — 游标遍历，归档过期证据，unpin CID，退还押金
2. **cleanup_old_archives** — 清理超过 `ArchiveTtlBlocks` 的归档记录
3. **cleanup_unrevealed_commitments** — 独立游标 (`CommitCleanupCursor`) 清理超过 `CommitRevealDeadline` 的未揭示承诺，同步清理 Target/Ns 索引
4. **cleanup_expired_access_requests** — 游标式 (`AccessRequestCleanupCursor`) 清理超过 `AccessRequestTtlBlocks` 的过期访问请求

---

## 安全特性

| 特性 | 说明 |
|------|------|
| 权限隔离 | 提交权限 (`Authorizer`) 与密封权限 (`SealAuthorizer`) 完全隔离 |
| 押金机制 | `Currency::reserve` 预留，防垃圾攻击。撤回退还，违规没收 |
| 速率限制 | 滑窗 + 配额双重限制 |
| 链接限数 | `MaxLinksPerEvidence` 防膨胀，重复链接检测 |
| 链接对称 | `unlink` 同步递减 `EvidenceLinkCount`，与 `link` 对称 |
| 承诺超时 | 未揭示承诺自动清理（独立游标 `CommitCleanupCursor`），释放配额 |
| 所有者保护 | `append_evidence` 仅允许原始所有者追加 |
| IPFS 同步 | 移除/归档/撤回/删除私密内容时调用 `unpin` 释放存储 |
| 撤回完整清理 | `withdraw_evidence` 清理所有索引（Target/Ns/CidHash/Commit/Parent-Children/LinkCount） |
| 追加押金 | `append_evidence` 同样预留押金，与 `commit` 对齐 |
| 访问请求限数 | `MaxPendingRequestsPerContent` 防请求泛滥 |

---

## 集成

### 输出 Traits

- **`EvidenceProvider<AccountId>`** — 查询证据存在性、详情
- **`PrivateContentProvider<AccountId>`** — 查询加密内容、解密密钥
- **`EvidenceSealAuthorizer<AccountId>`** — 密封权限接口（runtime 实现）

### 消费方

- `pallet-dispute-arbitration` — 通过 `EvidenceExistenceChecker` 验证证据存在性
- `pallet-dispute-escrow` — 争议流程中引用证据

### 依赖

- `pallet-storage-service` — `StoragePin` trait (pin/unpin)
- `pallet-crypto-common` — 加密内容共享类型
- `pallet-balances` — `ReservableCurrency`（押金）
- `media-utils` — CID 验证、哈希计算

---

## 权重

全部 24 个 extrinsic 均有对应的 `WeightInfo` 实现和专属 benchmark。
手写权重位于 `weights.rs`，基准测试位于 `benchmarking.rs`（v2 宏）。
`seal_evidence` / `unseal_evidence` 权重参数化，包含子级级联开销。
生产环境应使用 `frame-benchmarking` 自动生成。
