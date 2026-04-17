# Dispute 模块组

> 路径：`pallets/dispute/`

NEXUS 争议解决基础设施，由三个独立 Substrate pallet 组成，提供资金托管、证据管理、仲裁裁决功能，支持 8 个业务域的资金争议与行为投诉处理。

## 设计理念

- **域路由解耦**：`[u8;8]` 域标识 + `ArbitrationRouter` trait，一套仲裁逻辑服务所有业务 pallet
- **三层分离**：资金（Escrow）、证据（Evidence）、决策（Arbitration）各自独立，仅通过 trait 接口互操作
- **双向押金博弈**：发起方/应诉方各锁订单金额 15%，败诉方罚没 30%，抑制恶意仲裁
- **三级防膨胀**：裁决即归档（~30B）→ 延迟归档已解决记录 → TTL 自动清理
- **证据 CID 锁定**：仲裁期间自动锁定证据 CID 防删除，裁决后自动解锁
- **信用闭环**：裁决结果通过 `CreditUpdater` 反馈做市商信用系统

## 目录结构

```
pallets/dispute/
├── escrow/       # 资金托管 (pallet-dispute-escrow, index 60)
├── evidence/     # 证据管理 (pallet-dispute-evidence, index 61)
└── arbitration/  # 仲裁裁决 + 统一投诉 (pallet-dispute-arbitration, index 62)
```

## 子模块概览

| 模块 | 职责 | Extrinsics | 存储项 | Runtime Index |
|------|------|:---:|:---:|:---:|
| **escrow** | 资金锁定、释放、退款、分账、到期策略 | 12 | 6 | 60 |
| **evidence** | 证据提交、IPFS Pin、私密内容、访问控制、自动归档 | 13 | 22 | 61 |
| **arbitration** | 仲裁（双向押金）+ 统一投诉（25 种类型） | 12 | 14 | 62 |

## 架构与依赖

```
                     ┌──────────────────────────────┐
                     │       pallet-dispute-arbitration       │
                     │  ┌────────────┬─────────────┐  │
                     │  │ 仲裁子系统  │  投诉子系统  │  │
                     │  │ (资金争议)  │  (行为投诉)  │  │
                     │  └─────┬──────┴──────┬──────┘  │
                     └───────┼─────────────┼──────────┘
               ┌─────────────┼─────────┐   │
               ▼             ▼         ▼   ▼
        ┌──────────┐  ┌──────────┐  ┌──────────────────┐
        │  Escrow  │  │ Evidence │  │ trading-common    │
        │ 资金托管  │  │ 证据管理  │  │ PricingProvider  │
        │          │  │          │  │ CidLockManager   │
        └──────────┘  └──────────┘  └──────────────────┘
```

**Trait 依赖链**

| 消费方 | 依赖 Trait | 提供方 |
|--------|-----------|--------|
| Arbitration | `Escrow<AccountId, Balance>` | pallet-dispute-escrow |
| Arbitration | `EvidenceExistenceChecker` | pallet-dispute-evidence（runtime 桥接） |
| Arbitration | `ArbitrationRouter` | runtime 注入（per-domain 路由） |
| Arbitration | `PricingProvider` | pallet-trading-common |
| Arbitration | `CidLockManager` | pallet-storage-service |
| Arbitration | `CreditUpdater` | runtime 注入 |
| Evidence | `EvidenceAuthorizer` | runtime 注入 |
| Evidence | `IpfsPinner` | pallet-storage-service |

## 业务域（8 字节标识）

| 域常量 | 字面量 | 业务 | 投诉类型数 |
|--------|--------|------|:---:|
| `OTC_ORDER` | `otc_ord_` | OTC 法币交易 | 4 |
| `LIVESTREAM` | `livstrm_` | 直播 | 6 |
| `MAKER` | `maker___` | 做市商 | 3 |
| `NFT_TRADE` | `nft_trd_` | NFT 交易 | 4 |
| `SWAP` | `swap____` | Swap 交换 | 3 |
| `MEMBER` | `member__` | 会员 | 2 |
| `CREDIT` | `credit__` | 信用系统 | 2 |
| `OTHER` | `other___` | 其他 | 1 |

## 两大子系统

### 仲裁系统（资金争议，双向押金）

**完整流程**

```
1. evidence::commit                          → 发起方提交证据
2. arbitration::dispute_with_two_way_deposit → 发起仲裁，Fungible::hold 锁发起方押金 15%
3. evidence::commit                          → 应诉方提交证据
4. arbitration::respond_to_dispute           → 应诉，锁应诉方押金 15%
5. arbitration::arbitrate                    → 治理裁决
   ├→ Router::apply_decision                     (Escrow 分账)
   ├→ handle_deposits_on_arbitration             (押金罚没/释放)
   ├→ unlock_all_evidence_cids                   (CID 解锁)
   ├→ CreditUpdater::record_maker_dispute_result (信用分)
   └→ archive_and_cleanup                        (归档 + 清理)
```

**押金处理规则**

| 裁决 | 发起方押金 | 应诉方押金 |
|------|:---:|:---:|
| Release（卖家胜） | 罚没 30%（`RejectedSlashBps`） | 全额释放 |
| Refund（买家胜） | 全额释放 | 罚没 30%（`RejectedSlashBps`） |
| Partial（双方责任） | 罚没 50%（`PartialSlashBps`） | 罚没 50%（`PartialSlashBps`） |

罚没部分转入国库（`TreasuryAccount`），释放部分回到托管账户。

### 投诉系统（行为投诉，25 种类型）

**完整流程**

```
1. arbitration::file_complaint             → 发起投诉（锁 ≈1 USDT 等值 COS 押金）
2. arbitration::respond_to_complaint       → 被投诉方响应/申诉
3. arbitration::settle_complaint           → 和解（或）
   arbitration::escalate_to_arbitration    → 升级到仲裁委员会
4. arbitration::resolve_complaint          → 治理裁决
```

**状态机**

```
Submitted ──→ Responded ──→ Mediating ──→ Arbitrating ──→ Resolved
    │              │             │                          ├ ComplainantWin
    │              │             │                          ├ RespondentWin
    │              ├→ settle     → ResolvedSettlement       └ Settlement
    │              └→ escalate ─────────────────────↗
    ├→ withdraw → Withdrawn
    └→ (deadline 过期) → Expired
```

**押金处理规则**

| 结果 | 押金处理 |
|------|---------|
| 投诉方胜诉 | 全额退还 |
| 被投诉方胜诉 | 罚没 `ComplaintSlashBps` 转给被投诉方，余额退还 |
| 和解 / 撤诉 / 过期 | 全额退还 |

## Extrinsics 全览

### Escrow（12 个）

| idx | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 0 | `lock` | Auth | 锁定资金到托管（拒绝 Closed / Disputed） |
| 1 | `release` | Auth | 全额释放 → Closed |
| 2 | `refund` | Auth | 全额退款 → Closed |
| 3 | `lock_with_nonce` | Auth | 幂等锁定（nonce 递增，重复静默忽略） |
| 4 | `release_split` | Auth | 分账释放（BoundedVec 条目） |
| 5 | `dispute` | Auth | Locked → Disputed |
| 6 | `apply_decision_release_all` | Auth | 裁决：全额释放 → Closed |
| 7 | `apply_decision_refund_all` | Auth | 裁决：全额退款 → Closed |
| 8 | `apply_decision_partial_bps` | Auth | 裁决：按 bps 比例分账 → Closed |
| 9 | `set_pause` | Admin | 全局暂停开关 |
| 10 | `schedule_expiry` | Auth | 安排到期处理 |
| 11 | `cancel_expiry` | Auth | 取消到期处理 |

### Evidence（13 个）

| idx | 方法 | 说明 |
|:---:|------|------|
| 0 | `commit` | 提交公开证据（CID），自动 IPFS Pin |
| 1 | `commit_hash` | 提交承诺哈希（不暴露明文 CID） |
| 2 | `link` | 按 (domain, target_id) 链接已有证据 |
| 3 | `link_by_ns` | 按 (ns, subject_id) 链接 |
| 4 | `unlink` | 取消 (domain, target_id) 链接 |
| 5 | `unlink_by_ns` | 取消 (ns, subject_id) 链接 |
| 6 | `register_public_key` | 注册用户公钥（RSA-2048 / Ed25519 / ECDSA-P256） |
| 7 | `store_private_content` | 存储加密私密内容（含访问策略 + 密钥包） |
| 8 | `grant_access` | 授予用户私密内容访问权限 |
| 9 | `revoke_access` | 撤销用户访问权限 |
| 10 | `rotate_content_keys` | 轮换加密密钥（O(1) 计数器） |
| 11 | `append_evidence` | 追加补充证据，形成子证据链 |
| 12 | `update_evidence_manifest` | 在 2 天修改窗口内更新清单 |

### Arbitration（12 个）

| idx | 方法 | 签名者 | 说明 |
|:---:|------|:---:|------|
| 0 | `dispute` | 当事人 | 发起仲裁（旧版） |
| 1 | `arbitrate` | DecisionOrigin | 裁决 → 分账 + 押金 + CID 解锁 + 信用分 + 归档 |
| 2 | `dispute_with_evidence_id` | 当事人 | 以 evidence_id 引用发起仲裁 |
| 3 | `append_evidence_id` | 当事人 | 追加证据引用（需 `can_dispute` 校验） |
| 4 | `dispute_with_two_way_deposit` | 当事人 | 双向押金仲裁（推荐） |
| 5 | `respond_to_dispute` | 应诉方 | 应诉 — 锁应诉方押金 + 提交反驳证据 |
| 10 | `file_complaint` | 投诉人 | 发起投诉（锁 ≈1 USDT 等值 COS） |
| 11 | `respond_to_complaint` | 被投诉人 | 响应/申诉 |
| 12 | `withdraw_complaint` | 投诉人 | 撤诉（退还押金） |
| 13 | `settle_complaint` | 任一方 | 和解（退还押金） |
| 14 | `escalate_to_arbitration` | 任一方 | 升级到仲裁委员会 |
| 15 | `resolve_complaint` | DecisionOrigin | 裁决投诉（胜诉退押金/败诉罚没） |

## Hooks

| Pallet | Hook | 说明 |
|--------|------|------|
| Escrow | `on_initialize` | 按块处理到期托管：Disputed 跳过重调度 24h，正常项执行 ExpiryPolicy |
| Evidence | `on_idle` | 归档 90 天旧证据（每次 ≤10）+ 清理 180 天旧归档（每次 ≤5） |
| Arbitration | `on_idle` | 4 阶段：过期投诉（≤5）→ 归档投诉（≤10）→ 清理仲裁归档（≤5）→ 清理投诉归档（≤5） |

## 核心 Trait 接口

### Escrow（资金操作，供业务 pallet 调用）

```rust
pub trait Escrow<AccountId, Balance> {
    fn escrow_account() -> AccountId;
    fn lock_from(payer: &AccountId, id: u64, amount: Balance) -> DispatchResult;
    fn transfer_from_escrow(id: u64, to: &AccountId, amount: Balance) -> DispatchResult;
    fn release_all(id: u64, to: &AccountId) -> DispatchResult;
    fn refund_all(id: u64, to: &AccountId) -> DispatchResult;
    fn amount_of(id: u64) -> Balance;
    fn split_partial(id: u64, release_to: &AccountId, refund_to: &AccountId, bps: u16) -> DispatchResult;
    fn set_disputed(id: u64) -> DispatchResult;
    fn set_resolved(id: u64) -> DispatchResult;
}
```

### ArbitrationRouter（域路由，runtime 注入）

```rust
pub trait ArbitrationRouter<AccountId, Balance> {
    fn can_dispute(domain: [u8; 8], who: &AccountId, id: u64) -> bool;
    fn apply_decision(domain: [u8; 8], id: u64, decision: Decision) -> DispatchResult;
    fn get_counterparty(domain: [u8; 8], initiator: &AccountId, id: u64) -> Result<AccountId, DispatchError>;
    fn get_order_amount(domain: [u8; 8], id: u64) -> Result<Balance, DispatchError>;
    fn get_maker_id(domain: [u8; 8], id: u64) -> Option<u64> { None }
}
```

### EvidenceAuthorizer / EvidenceExistenceChecker

```rust
pub trait EvidenceAuthorizer<AccountId> {
    fn is_authorized(ns: [u8; 8], who: &AccountId) -> bool;
}

pub trait EvidenceExistenceChecker {
    fn evidence_exists(id: u64) -> bool;
}
```

## 安全审计修复

| 编号 | 模块 | 修复内容 |
|------|------|---------|
| EH3 | escrow | `lock`/`lock_with_nonce` 拒绝已关闭的托管 |
| EM1 | escrow | `schedule_expiry` 使用 `ExpiringAtFull` 错误码 |
| EM2 | escrow | `release`/`refund`/`release_split` 返回 `DisputeActive` |
| VC1 | evidence | 5 种类型添加 `DecodeWithMemTracking` |
| VC2 | evidence | 5 个 extrinsic 使用正确 WeightInfo |
| VH1/VH2 | evidence | `commit`/`append_evidence` 添加 CID 边界验证 |
| VM1 | evidence | 密钥轮换使用 O(1) `KeyRotationCounter` |
| VM2 | evidence | 归档清理同步回收关联索引 |
| AC1 | arbitration | 3 种类型添加 `DecodeWithMemTracking` |
| AC2 | arbitration | 7 个 weight 函数独立化 |
| AH1 | arbitration | `append_evidence_id` 增加 `can_dispute` 权限校验 |
| AH4 | arbitration | 投诉过期使用游标扫描（O(batch)） |
| AH5/AH6/AH7 | arbitration | 撤诉/和解/过期时退还投诉押金 |

## 相关文档

- [escrow/README.md](escrow/README.md) — 资金托管：状态机、Trait 接口、到期策略
- [evidence/README.md](evidence/README.md) — 证据管理：CID 化存储、私密内容、归档策略
- [arbitration/README.md](arbitration/README.md) — 仲裁 + 投诉：双向押金、25 种投诉类型、域路由
