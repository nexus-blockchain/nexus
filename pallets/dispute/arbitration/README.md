# pallet-arbitration

> 路径：`pallets/dispute/arbitration/` · Crate: `pallet-arbitration v0.1.0` · Runtime Index: 62

仲裁与投诉统一处理系统。内含两大子系统：**仲裁系统**（资金争议、双向押金、裁决分账）与**投诉系统**（行为投诉、押金防滥用、自动过期归档）。支持 8 个业务域（25 种投诉类型），通过 `ArbitrationRouter` trait 实现域路由解耦。

---

## 目录

- [设计理念](#设计理念)
- [架构概览](#架构概览)
- [业务域](#业务域)
- [Extrinsics](#extrinsics)
- [流程](#流程)
- [存储](#存储)
- [主要类型](#主要类型)
- [事件](#事件)
- [错误](#错误)
- [Trait 接口](#trait-接口)
- [Hooks](#hooks)
- [配置参数](#配置参数)
- [集成示例](#集成示例)
- [测试](#测试)
- [相关模块](#相关模块)

---

## 设计理念

- **域路由架构**：`[u8;8]` 域标识 + `ArbitrationRouter` trait，一套仲裁逻辑服务所有业务 pallet
- **双向押金**：发起方/应诉方各从托管锁定订单金额 `DepositRatioBps`（默认 15%），败诉方罚没 `RejectedSlashBps`（默认 30%）
- **证据 CID 锁定**：仲裁期间自动调用 `CidLockManager` 锁定相关证据，防止删除；裁决后自动解锁
- **信用集成**：裁决结果通过 `CreditUpdater` 反馈到做市商信用系统
- **防膨胀**：三级策略 — 裁决即归档（`ArchivedDispute` ~30B）、投诉延迟归档（`ComplaintArchiveDelayBlocks`）、TTL 自动清理（`ArchiveTtlBlocks`）
- **投诉押金**：通过 `PricingProvider` 实时换算 ≈1 USDT 等值 COS，兜底值 `ComplaintDeposit` 防止恶意投诉
- **分级罚没**：投诉败诉罚没率三级回退 — `DomainPenaltyRates`（治理可调）→ `ComplaintType::penalty_rate()`（按类型差异化）→ `ComplaintSlashBps`（全局兜底）
- **紧急控制**：`set_paused` 全局暂停 + `force_close_*` 强制关闭卡住案件
- **永久封禁**：重度欺诈投诉（如 `OtcTradeFraud`）裁决后自动调用 `Router::ban_account`

---

## 架构概览

```
                         ┌────────────────────────────────────┐
                         │         pallet-arbitration          │
                         │  ┌──────────────┬────────────────┐  │
                         │  │  仲裁子系统    │   投诉子系统    │  │
                         │  │ (资金争议     │ (行为投诉      │  │
                         │  │  双向押金     │  押金防滥用     │  │
                         │  │  裁决分账)    │  自动过期归档)  │  │
                         │  └──────┬───────┴───────┬────────┘  │
                         └────────┼───────────────┼────────────┘
                     ┌────────────┼─────────┐     │
                     ▼            ▼         ▼     ▼
              ┌──────────┐ ┌──────────┐ ┌──────────────────┐
              │  Escrow  │ │ Evidence │ │ trading-common    │
              │ 资金托管  │ │ 证据管理  │ │ PricingProvider  │
              └──────────┘ └──────────┘ └──────────────────┘
                                │
                     ┌──────────┴──────────┐
                     ▼                     ▼
              ┌──────────────┐      ┌──────────────┐
              │ storage-svc  │      │ credit-sys   │
              │ CidLockMgr   │      │ CreditUpdater│
              └──────────────┘      └──────────────┘
```

---

## 业务域

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

---

## Extrinsics

### 仲裁子系统（资金争议）

| call_index | 方法 | 签名者 | 说明 |
|:---:|------|:---:|------|
| 0 | `dispute` | 当事人 | 发起仲裁（旧版，CID 存链） |
| 1 | `arbitrate` | DecisionOrigin | 裁决 → Router 分账 + 押金处理 + CID 解锁 + 信用分 + 归档 |
| 2 | `dispute_with_evidence_id` | 当事人 | 以 evidence_id 引用发起仲裁（校验存在性） |
| 3 | `append_evidence_id` | 当事人 | 追加证据引用（需 `can_dispute` 权限 + 存在性校验） |
| 4 | `dispute_with_two_way_deposit` | 当事人 | **推荐** — 双向押金仲裁，锁发起方 `DepositRatioBps` 押金 |
| 5 | `respond_to_dispute` | 应诉方 | 应诉 — 锁应诉方同额押金 + 提交反驳证据 |
| 20 | `request_default_judgment` | 发起方 | 缺席裁决 — 应诉方超时未应诉，自动 Refund |
| 23 | `settle_dispute` | 任一方 | 纠纷和解 — 释放双方押金，不归档统计 |
| 25 | `dismiss_dispute` | DecisionOrigin | 驳回无效纠纷 — Release + 罚没发起方押金 |
| 28 | `force_close_dispute` | DecisionOrigin | 强制关闭 — 释放所有押金，不罚没 |

### 投诉子系统（行为投诉）

| call_index | 方法 | 签名者 | 说明 |
|:---:|------|:---:|------|
| 10 | `file_complaint` | 投诉人 | 发起投诉（锁 ≈1 USDT 等值 COS 押金） |
| 11 | `respond_to_complaint` | 被投诉人 | 响应/申诉（独立 `response_cid`，不覆盖原始详情） |
| 12 | `withdraw_complaint` | 投诉人 | 撤诉（全额退还押金） |
| 13 | `settle_complaint` | 任一方¹ | 和解（独立 `settlement_cid`，全额退还押金） |
| 14 | `escalate_to_arbitration` | 任一方 | 升级到仲裁委员会（加入待裁决队列） |
| 15 | `resolve_complaint` | DecisionOrigin | 裁决投诉（独立 `resolution_cid`，败诉罚没/胜诉退还） |
| 21 | `supplement_complaint_evidence` | 投诉人 | 补充投诉证据（仅事件，不存链） |
| 22 | `supplement_response_evidence` | 被投诉人 | 补充应诉证据（仅事件，不存链） |
| 24 | `start_mediation` | DecisionOrigin | 启动调解阶段 |
| 26 | `dismiss_complaint` | DecisionOrigin | 驳回无效投诉 — 罚没投诉人押金 |
| 29 | `force_close_complaint` | DecisionOrigin | 强制关闭 — 全额退还押金 |

> ¹ 调解中（Mediating）仅投诉方可和解，防止被投诉方单方面关闭调解绕过仲裁。

### 管理 Extrinsics

| call_index | 方法 | 签名者 | 说明 |
|:---:|------|:---:|------|
| 27 | `set_paused` | DecisionOrigin | 紧急暂停/恢复模块 |
| 30 | `set_domain_penalty_rate` | DecisionOrigin | 动态设置域惩罚比例（`None` 移除覆盖） |

---

## 流程

### 仲裁流程（双向押金模式）

```
1. evidence::commit                          → 发起方提交证据
2. arbitration::dispute_with_two_way_deposit → 发起仲裁，锁发起方押金
3. evidence::commit                          → 应诉方提交证据
4. arbitration::respond_to_dispute           → 应诉，锁应诉方同额押金
5. arbitration::arbitrate                    → 治理裁决
   ├→ Router::apply_decision                     (Escrow 分账)
   ├→ handle_deposits_on_arbitration             (押金罚没/释放)
   ├→ unlock_all_evidence_cids                   (CID 解锁)
   ├→ CreditUpdater::record_maker_dispute_result (信用分)
   └→ archive_and_cleanup                        (归档 + 清理)
```

**替代路径：**

| 路径 | 触发条件 | 行为 |
|------|---------|------|
| 缺席裁决 | 应诉方超时未应诉 | `request_default_judgment` → 自动 Refund |
| 双方和解 | 应诉方已锁定押金 | `settle_dispute` → 全额释放双方押金 |
| 驳回 | 仲裁委员会认定无效 | `dismiss_dispute` → Release + 罚没发起方 |
| 强制关闭 | 案件卡住/异常 | `force_close_dispute` → 释放所有押金 |

### 仲裁押金处理规则

| 裁决 | 发起方（买家）押金 | 应诉方（卖家）押金 |
|------|:---:|:---:|
| Release（卖家胜） | 罚没 `RejectedSlashBps`（30%）→ 国库 | 全额释放 → 托管账户 |
| Refund（买家胜） | 全额释放 → 托管账户 | 罚没 `RejectedSlashBps`（30%）→ 国库 |
| Partial（双方责任） | 罚没 `PartialSlashBps`（50%）→ 国库 | 罚没 `PartialSlashBps`（50%）→ 国库 |

### 投诉状态机

```
                    ┌─ withdraw_complaint ─→ Withdrawn（退还押金）
                    │
Submitted ──────────┤─ (deadline 过期) ────→ Expired（退还押金）
    │               │
    │ respond_to_   │
    │ complaint     │
    ▼               │
Responded ──────────┤─ settle_complaint ──→ ResolvedSettlement（退还押金）
    │               │
    ├─ escalate ────┤─→ Arbitrating ──────→ resolve_complaint
    │               │                        ├ ComplainantWin（退还押金）
    ▼               │                        ├ RespondentWin（罚没押金）
Mediating ──────────┘                        └ Partial（退还押金）
    │
    └─ settle_complaint¹ ──→ ResolvedSettlement（退还押金）

¹ 调解中仅投诉方可和解

其他终态入口:
  dismiss_complaint     → ResolvedRespondentWin（罚没押金）
  force_close_complaint → Withdrawn（退还押金）
```

### 投诉押金罚没规则

投诉败诉（被投诉方胜诉 / 驳回）时，罚没比例按以下优先级确定：

1. **`DomainPenaltyRates`** — 治理动态配置的域级覆盖（`set_domain_penalty_rate`）
2. **`ComplaintType::penalty_rate()`** — 按投诉类型差异化（如 `OtcTradeFraud` = 80%）
3. **`ComplaintSlashBps`** — 全局兜底值

罚没部分转给被投诉方，余额退还投诉人。

| 裁决结果 | 押金处理 | 统计归入 |
|---------|---------|---------|
| 投诉方胜诉（decision=0） | 全额退还 | `complainant_wins` |
| 被投诉方胜诉（decision=1） | 按优先级罚没 → 被投诉方 | `respondent_wins` |
| 部分裁决（decision≥2） | 全额退还 | `complainant_wins` |
| 和解 | 全额退还 | `settlements` |
| 撤诉 / 过期 | 全额退还 | — / `expired_count` |

---

## 存储

### 仲裁存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Disputed` | `DoubleMap<[u8;8], u64, ()>` | 争议登记标记 |
| `EvidenceIds` | `DoubleMap<[u8;8], u64, BoundedVec<u64, MaxEvidence>>` | 证据引用列表 |
| `LockedCidHashes` | `DoubleMap<[u8;8], u64, BoundedVec<Hash, MaxEvidence>>` | 锁定的 CID 哈希 |
| `TwoWayDeposits` | `DoubleMap<[u8;8], u64, TwoWayDepositRecord>` | 双向押金记录 |
| `PendingArbitrationDisputes` | `DoubleMap<[u8;8], u64, ()>` | 待裁决纠纷队列 |

### 投诉存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextComplaintId` | `u64` | 投诉 ID 自增计数器 |
| `Complaints` | `Map<u64, Complaint<T>>` | 活跃投诉主存储 |
| `ComplaintDeposits` | `Map<u64, Balance>` | 投诉押金记录 |
| `UserActiveComplaints` | `Map<AccountId, BoundedVec<u64, 50>>` | 投诉人活跃投诉索引 |
| `RespondentActiveComplaints` | `Map<AccountId, BoundedVec<u64, 50>>` | 被投诉人活跃投诉索引 |
| `ObjectComplaints` | `DoubleMap<[u8;8], u64, BoundedVec<u64, 50>>` | 按业务对象查询投诉索引 |
| `PendingArbitrationComplaints` | `Map<u64, ()>` | 待裁决投诉队列 |
| `DomainStats` | `Map<[u8;8], DomainStatistics>` | 域统计信息 |
| `DomainPenaltyRates` | `Map<[u8;8], u16>` | 域惩罚比例动态配置（bps） |

### 归档存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextArchivedId` | `u64` | 仲裁归档 ID 计数器 |
| `ArchivedDisputes` | `Map<u64, ArchivedDispute>` | 归档仲裁（~30B/条） |
| `ArchivedComplaints` | `Map<u64, ArchivedComplaint>` | 归档投诉（~38B/条） |
| `ArbitrationStats` | `ArbitrationPermanentStats` | 仲裁永久统计 |

### 游标与控制

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `ComplaintExpiryCursor` | `u64` | 投诉过期扫描游标（O(batch)） |
| `ComplaintArchiveCursor` | `u64` | 投诉归档扫描游标 |
| `ArchiveDisputeCleanupCursor` | `u64` | 仲裁归档 TTL 清理游标 |
| `ArchiveComplaintCleanupCursor` | `u64` | 投诉归档 TTL 清理游标 |
| `Paused` | `bool` | 全局暂停开关 |

---

## 主要类型

### Decision

```rust
pub enum Decision {
    Release,      // 全额释放（卖家胜）
    Refund,       // 全额退款（买家胜）
    Partial(u16), // 按比例分配，bps 0–10000
}
```

### HoldReason

```rust
pub enum HoldReason {
    DisputeInitiator,   // 纠纷发起方押金
    DisputeRespondent,  // 应诉方押金
    ComplaintDeposit,   // 投诉押金
}
```

### TwoWayDepositRecord

```rust
pub struct TwoWayDepositRecord<AccountId, Balance, BlockNumber> {
    pub initiator: AccountId,
    pub initiator_deposit: Balance,
    pub respondent: AccountId,
    pub respondent_deposit: Option<Balance>, // 未应诉时 None
    pub response_deadline: BlockNumber,
    pub has_responded: bool,
}
```

### Complaint

```rust
pub struct Complaint<T: Config> {
    pub id: u64,
    pub domain: [u8; 8],
    pub object_id: u64,
    pub complaint_type: ComplaintType,
    pub complainant: T::AccountId,
    pub respondent: T::AccountId,
    pub details_cid: BoundedVec<u8, T::MaxCidLen>,             // 原始投诉详情
    pub response_cid: Option<BoundedVec<u8, T::MaxCidLen>>,    // 被投诉方响应（独立字段）
    pub amount: Option<BalanceOf<T>>,
    pub status: ComplaintStatus,
    pub created_at: BlockNumberFor<T>,
    pub response_deadline: BlockNumberFor<T>,
    pub settlement_cid: Option<BoundedVec<u8, T::MaxCidLen>>,  // 和解详情（独立字段）
    pub resolution_cid: Option<BoundedVec<u8, T::MaxCidLen>>,  // 裁决详情（独立字段）
    pub updated_at: BlockNumberFor<T>,
}
```

### ComplaintStatus

```rust
pub enum ComplaintStatus {
    Submitted,              // 已提交，等待响应
    Responded,              // 已响应/申诉
    Mediating,              // 调解中
    Arbitrating,            // 仲裁中
    ResolvedComplainantWin, // 投诉方胜诉
    ResolvedRespondentWin,  // 被投诉方胜诉
    ResolvedSettlement,     // 和解
    Withdrawn,              // 已撤销
    Expired,                // 已过期
}
```

`is_resolved()` 返回 `true` 的状态：`ResolvedComplainantWin`、`ResolvedRespondentWin`、`ResolvedSettlement`、`Withdrawn`、`Expired`。

### ComplaintType（25 种，覆盖 8 业务域）

| 业务域 | 投诉类型 | `penalty_rate()` | `triggers_permanent_ban()` |
|-------|---------|:---:|:---:|
| OTC | `OtcSellerNotDeliver`, `OtcBuyerFalseClaim`, `OtcPriceDispute` | 3000 (30%) | — |
| OTC | `OtcTradeFraud` | **8000 (80%)** | **YES** |
| 直播 | `LiveIllegalContent` | **5000 (50%)** | — |
| 直播 | `LiveFalseAdvertising`, `LiveHarassment`, `LiveFraud`, `LiveGiftRefund`, `LiveOther` | 3000 (30%) | — |
| 做市商 | `MakerMaliciousOperation` | **5000 (50%)** | — |
| 做市商 | `MakerCreditDefault`, `MakerFalseQuote` | 3000 (30%) | — |
| NFT | `NftSellerNotDeliver`, `NftCounterfeit`, `NftTradeFraud`, `NftAuctionDispute` | 3000 (30%) | — |
| Swap | `SwapMakerNotComplete`, `SwapVerificationTimeout`, `SwapFraud` | 3000 (30%) | — |
| 会员 | `MemberBenefitNotProvided`, `MemberServiceQuality` | 3000 (30%) | — |
| 信用 | `CreditScoreDispute`, `CreditPenaltyAppeal` | 3000 (30%) | — |
| 其他 | `Other` | 3000 (30%) | — |

每种类型绑定三个方法：
- `domain()` — 所属业务域 `[u8;8]`
- `penalty_rate()` — 罚没比例（bps），被 `DomainPenaltyRates` 覆盖时不使用
- `triggers_permanent_ban()` — 投诉方胜诉时是否触发永久封禁

### ArchivedDispute（~30B）

```rust
pub struct ArchivedDispute {
    pub domain: [u8; 8],
    pub object_id: u64,
    pub decision: u8,       // 0=Release, 1=Refund, 2=Partial
    pub partial_bps: u16,
    pub completed_at: u32,
    pub year_month: u16,    // YYMM 格式
}
```

### ArchivedComplaint（~38B）

```rust
pub struct ArchivedComplaint {
    pub id: u64,
    pub domain: [u8; 8],
    pub object_id: u64,
    pub decision: u8,       // 0=投诉方胜, 1=被投诉方胜, 2=和解, 3=撤销, 4=过期
    pub resolved_at: u32,
    pub year_month: u16,
}
```

### ArbitrationPermanentStats

```rust
pub struct ArbitrationPermanentStats {
    pub total_disputes: u64,
    pub release_count: u64,
    pub refund_count: u64,
    pub partial_count: u64,
}
```

### DomainStatistics

```rust
pub struct DomainStatistics {
    pub total_complaints: u64,
    pub resolved_count: u64,
    pub complainant_wins: u64,  // 含投诉方胜诉 + 部分裁决
    pub respondent_wins: u64,
    pub settlements: u64,       // 仅自愿和解（settle_complaint）
    pub expired_count: u64,
}
```

---

## 事件

### 仲裁事件

| 事件 | 字段 | 说明 |
|------|------|------|
| `Disputed` | domain, id | 争议已登记 |
| `Arbitrated` | domain, id, decision, bps | 裁决完成 |
| `DisputeWithDepositInitiated` | domain, id, initiator, respondent, deposit, deadline | 双向押金仲裁发起 |
| `RespondentDepositLocked` | domain, id, respondent, deposit | 应诉方押金锁定 |
| `DepositProcessed` | domain, id, account, released, slashed | 押金处理（罚没/释放） |
| `DefaultJudgment` | domain, id, initiator | 缺席裁决（应诉方超时） |
| `DisputeSettled` | domain, id | 纠纷双方和解 |
| `DisputeDismissed` | domain, id | 纠纷被驳回 |
| `DisputeForceClosed` | domain, id | 纠纷被强制关闭 |

### 投诉事件

| 事件 | 字段 | 说明 |
|------|------|------|
| `ComplaintFiled` | complaint_id, domain, object_id, complainant, respondent, complaint_type | 投诉已提交 |
| `ComplaintResponded` | complaint_id, respondent | 投诉已响应 |
| `ComplaintWithdrawn` | complaint_id | 投诉已撤销 |
| `ComplaintSettled` | complaint_id | 投诉已和解 |
| `ComplaintEscalated` | complaint_id | 投诉已升级到仲裁 |
| `ComplaintResolved` | complaint_id, decision | 投诉裁决完成 |
| `ComplaintExpired` | complaint_id | 投诉已过期 |
| `ComplaintArchived` | complaint_id | 投诉已归档 |
| `ComplaintEvidenceSupplemented` | complaint_id, who, evidence_cid | 补充证据 |
| `ComplaintMediationStarted` | complaint_id | 投诉进入调解阶段 |
| `ComplaintDismissed` | complaint_id | 投诉被驳回 |
| `ComplaintForceClosed` | complaint_id | 投诉被强制关闭 |
| `AccountBanned` | domain, object_id, account | 永久封禁执行 |

### 管理事件

| 事件 | 字段 | 说明 |
|------|------|------|
| `PausedStateChanged` | paused | 模块暂停/恢复 |
| `DomainPenaltyRateUpdated` | domain, rate_bps | 域惩罚比例已更新 |

---

## 错误

| 错误 | 说明 |
|------|------|
| `AlreadyDisputed` | 重复争议登记 / 证据引用列表已满 |
| `NotDisputed` | 案件未登记 / 非当事方 |
| `InsufficientDeposit` | 押金不足或锁定失败 |
| `AlreadyResponded` | 重复应诉 |
| `ResponseDeadlinePassed` | 应诉期已过 |
| `CounterpartyNotFound` | 无法获取对方账户或订单金额 |
| `ComplaintNotFound` | 投诉不存在 |
| `NotAuthorized` | 无权操作 |
| `InvalidComplaintType` | 投诉类型与域不匹配 |
| `InvalidState` | 无效的状态转换 |
| `TooManyComplaints` | 该对象投诉过多 / CID 锁定列表已满 |
| `TooManyActiveComplaints` | 用户活跃投诉超限（上限 50） |
| `EvidenceNotFound` | 引用的 evidence_id 在 pallet-evidence 中不存在 |
| `ResponseDeadlineNotReached` | 应诉期未到，不能申请缺席裁决 |
| `SettlementNotConfirmed` | 纠纷和解需要双方已参与（应诉方已锁定押金） |
| `ModulePaused` | 模块已暂停 |
| `InvalidPenaltyRate` | 惩罚比例超过 10000 bps |

---

## Trait 接口

### ArbitrationRouter（域路由）

```rust
pub trait ArbitrationRouter<AccountId, Balance> {
    /// 检查用户是否有权对指定案件发起仲裁
    fn can_dispute(domain: [u8; 8], who: &AccountId, id: u64) -> bool;
    /// 执行裁决分账（Escrow 操作）
    fn apply_decision(domain: [u8; 8], id: u64, decision: Decision) -> DispatchResult;
    /// 获取仲裁对手方账户
    fn get_counterparty(domain: [u8; 8], initiator: &AccountId, id: u64)
        -> Result<AccountId, DispatchError>;
    /// 获取订单金额（用于计算押金）
    fn get_order_amount(domain: [u8; 8], id: u64) -> Result<Balance, DispatchError>;
    /// 获取做市商 ID（用于信用分更新，默认 None）
    fn get_maker_id(domain: [u8; 8], id: u64) -> Option<u64> { None }
}
```

### CreditUpdater（信用分更新）

```rust
pub trait CreditUpdater {
    fn record_maker_dispute_result(
        maker_id: u64, order_id: u64, maker_win: bool
    ) -> DispatchResult;
}
```

提供空实现 `impl CreditUpdater for ()`，适用于不需要信用集成的场景。

### EvidenceExistenceChecker（证据存在性校验）

```rust
pub trait EvidenceExistenceChecker {
    fn evidence_exists(id: u64) -> bool;
}
```

`dispute_with_evidence_id`、`append_evidence_id`、`dispute_with_two_way_deposit`、`respond_to_dispute` 均在写入前校验引用的 evidence_id 是否存在。

---

## Hooks

### on_idle（剩余权重利用）

每个区块空闲时按优先级依次处理，单项权重 25M ref_time + 读开销 10M ref_time：

| 阶段 | 操作 | 每次上限 | 说明 |
|:---:|------|:---:|------|
| 1 | `expire_old_complaints` | 5 | 扫描 `Submitted` 状态且过期的投诉，退还押金，更新统计 |
| 2 | `archive_old_complaints` | 10 | 归档已解决投诉（延迟 `ComplaintArchiveDelayBlocks`），移除活跃记录 |
| 3 | `cleanup_old_archived_disputes` | 5 | TTL 清理过期归档仲裁（`ArchiveTtlBlocks`，0 禁用） |
| 4 | `cleanup_old_archived_complaints` | 5 | TTL 清理过期归档投诉 |

所有扫描均使用游标推进（O(batch)），避免全表遍历。每阶段仅在剩余权重足够时执行。

---

## 配置参数

### 基础配置

| 参数 | 类型 | 说明 |
|------|------|------|
| `MaxEvidence` | `u32` | 每案最大证据引用数 |
| `MaxCidLen` | `u32` | CID 最大字节长度 |
| `Escrow` | trait | 托管接口（`Escrow<AccountId, Balance>`） |
| `Router` | trait | 域路由（runtime 注入 `ArbitrationRouter`） |
| `DecisionOrigin` | `EnsureOrigin` | 裁决 Origin（治理/委员会） |
| `WeightInfo` | trait | 权重信息（15 个独立函数） |

### 押金配置

| 参数 | 类型 | 说明 |
|------|------|------|
| `Fungible` | trait | Fungible 接口（`Inspect + Mutate + MutateHold`） |
| `RuntimeHoldReason` | enum | 押金锁定原因标识（`From<HoldReason>`） |
| `DepositRatioBps` | `u16` | 仲裁押金比例（bps，1500 = 15%） |
| `ResponseDeadline` | `BlockNumber` | 应诉期限（区块数） |
| `RejectedSlashBps` | `u16` | 败诉罚没比例（bps，3000 = 30%） |
| `PartialSlashBps` | `u16` | 部分胜诉罚没比例（bps，5000 = 50%） |
| `TreasuryAccount` | `AccountId` | 国库账户（接收仲裁罚没） |

### 投诉配置

| 参数 | 类型 | 说明 |
|------|------|------|
| `ComplaintDeposit` | `Balance` | 投诉押金兜底金额（COS 数量） |
| `ComplaintDepositUsd` | `u64` | 投诉押金 USD 价值（精度 10^6，1_000_000 = 1 USDT） |
| `Pricing` | trait | 定价接口（`PricingProvider`，USD→COS 换算） |
| `ComplaintSlashBps` | `u16` | 投诉败诉罚没全局兜底比例（bps） |

### 集成配置

| 参数 | 类型 | 说明 |
|------|------|------|
| `CidLockManager` | trait | CID 锁定管理器（仲裁期间锁定证据） |
| `CreditUpdater` | trait | 信用分更新器（裁决结果反馈信用系统） |
| `EvidenceExists` | trait | 证据存在性检查器（校验 evidence_id） |

### 防膨胀配置

| 参数 | 类型 | 说明 |
|------|------|------|
| `ArchiveTtlBlocks` | `u32` | 归档记录 TTL（区块数，推荐 2_592_000 ≈ 180 天，0 禁用清理） |
| `ComplaintArchiveDelayBlocks` | `BlockNumber` | 投诉归档延迟（区块数，推荐 432_000 ≈ 30 天） |

---

## 集成示例

### 实现 ArbitrationRouter

```rust
impl<T: Config> ArbitrationRouter<T::AccountId, BalanceOf<T>> for Pallet<T> {
    fn can_dispute(domain: [u8; 8], who: &T::AccountId, id: u64) -> bool {
        if domain != *b"otc_ord_" { return false; }
        Self::is_order_participant(id, who)
    }

    fn apply_decision(domain: [u8; 8], id: u64, decision: Decision) -> DispatchResult {
        match decision {
            Decision::Release => T::Escrow::release_all(id, &Self::get_seller(id)?),
            Decision::Refund  => T::Escrow::refund_all(id, &Self::get_buyer(id)?),
            Decision::Partial(bps) => {
                let (seller, buyer) = Self::get_parties(id)?;
                T::Escrow::split_partial(id, &seller, &buyer, bps)
            },
        }
    }

    fn get_counterparty(domain: [u8; 8], initiator: &T::AccountId, id: u64)
        -> Result<T::AccountId, DispatchError> {
        Ok(Self::get_other_party(id, initiator))
    }

    fn get_order_amount(domain: [u8; 8], id: u64) -> Result<BalanceOf<T>, DispatchError> {
        Orders::<T>::get(id).map(|o| o.amount).ok_or(Error::<T>::NotFound.into())
    }

    fn get_maker_id(_domain: [u8; 8], id: u64) -> Option<u64> {
        Orders::<T>::get(id).and_then(|o| o.maker_id)
    }
}
```

### Runtime 配置

```rust
impl pallet_arbitration::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxEvidence = ConstU32<10>;
    type MaxCidLen = ConstU32<64>;
    type Escrow = pallet_escrow::Pallet<Runtime>;
    type Router = OtcArbitrationRouter;
    type DecisionOrigin = EnsureRoot<AccountId>;
    type WeightInfo = pallet_arbitration::weights::SubstrateWeight<Runtime>;
    type Fungible = Balances;
    type RuntimeHoldReason = RuntimeHoldReason;
    type DepositRatioBps = ConstU16<1500>;       // 15%
    type ResponseDeadline = ConstU64<14400>;      // ~1 天
    type RejectedSlashBps = ConstU16<3000>;       // 30%
    type PartialSlashBps = ConstU16<5000>;        // 50%
    type TreasuryAccount = TreasuryAccountId;
    type ComplaintDeposit = ConstU128<100>;
    type ComplaintDepositUsd = ConstU64<1_000_000>; // 1 USDT
    type Pricing = pallet_trading_common::Pallet<Runtime>;
    type ComplaintSlashBps = ConstU16<5000>;      // 50% 兜底
    type CidLockManager = pallet_storage_service::Pallet<Runtime>;
    type CreditUpdater = ();                       // 空实现
    type EvidenceExists = pallet_evidence::Pallet<Runtime>;
    type ArchiveTtlBlocks = ConstU32<2_592_000>;  // ~180 天
    type ComplaintArchiveDelayBlocks = ConstU64<432_000>; // ~30 天
}
```

---

## 测试

测试文件：`src/tests.rs`，共 47 个测试用例。

```bash
cargo test -p pallet-arbitration
```

测试覆盖：

| 类别 | 数量 | 覆盖内容 |
|------|:---:|---------|
| 仲裁基础 | 7 | dispute, arbitrate, dispute_with_evidence_id, dispute_with_two_way_deposit, respond_to_dispute |
| 投诉基础 | 10 | file, respond, withdraw, settle, escalate, resolve (complainant/respondent win) |
| 过期/归档 | 3 | expire_old_complaints, archive_old_complaints, complaint_type_domain_mapping |
| 证据校验 | 2 | respond_to_dispute 拒绝无效证据, dispute_with_two_way_deposit 拒绝无效证据 |
| 暂停控制 | 1 | append_evidence_id 在暂停时被阻止 |
| 域惩罚率 | 4 | 域覆盖 > 类型罚没率 > 全局兜底, 各类型差异化验证 |
| 索引清理 | 7 | withdraw/settle/resolve/dismiss/expire/force_close 立即清理索引 |
| 调解限制 | 2 | 被投诉方不可和解 Mediating 状态, 可和解 Responded 状态 |
| 过期顺序 | 1 | 非单调 deadline 下过期处理正确 |
| 统计归类 | 1 | 部分裁决计入 complainant_wins 而非 settlements |
| 归档安全 | 1 | 索引已清理后归档仍正常工作 |

---

## 相关模块

- [escrow/](../escrow/) — 资金托管（裁决分账接口 `Escrow<AccountId, Balance>`）
- [evidence/](../evidence/) — 证据管理（`evidence_id` 引用 + `EvidenceExistenceChecker`）
- [trading/common/](../../trading/common/) — `PricingProvider`（投诉押金 USD→COS 换算）
- [storage/service/](../../storage/service/) — `CidLockManager`（证据 CID 锁定/解锁）
