# pallet-dispute-arbitration

> 路径：`pallets/dispute/arbitration/` · Crate: `pallet-dispute-arbitration v0.2.0` · StorageVersion: 1

仲裁与投诉统一处理系统。内含两大子系统：**仲裁系统**（资金争议、双向押金、裁决分账）与**投诉系统**（行为投诉、押金防滥用、申诉机制、自动升级、冷却期、证据链上追踪）。支持 9 个业务域（26 种投诉类型），通过 `ArbitrationRouter` trait 实现域路由解耦。

---

## 目录

- [设计理念](#设计理念)
- [代码组织](#代码组织)
- [业务域](#业务域)
- [Extrinsics](#extrinsics)
- [状态机](#状态机)
- [流程](#流程)
- [存储](#存储)
- [主要类型](#主要类型)
- [事件](#事件)
- [错误](#错误)
- [Trait 接口](#trait-接口)
- [Runtime API](#runtime-api)
- [Hooks](#hooks)
- [配置参数](#配置参数)
- [前端集成](#前端集成)
- [集成示例](#集成示例)
- [测试](#测试)
- [版本历史](#版本历史)
- [相关模块](#相关模块)

---

## 设计理念

- **域路由架构**：`[u8;8]` 域标识 + `ArbitrationRouter` trait，一套仲裁逻辑服务所有业务 pallet
- **双向押金**：发起方/应诉方各从托管锁定订单金额 `DepositRatioBps`（默认 15%），败诉方罚没 `RejectedSlashBps`（默认 30%）
- **形式化状态机**：所有状态转换通过 `state_machine::can_transition()` 统一验证，防止不一致
- **申诉机制**：败诉方在 `AppealWindowBlocks` 内可提起申诉，缴纳 2x 市场价押金，治理最终裁决
- **自动升级**：Responded 状态超过 `AutoEscalateBlocks` 自动升级到 Arbitrating；Submitted 超过响应截止后允许手动升级
- **冷却期**：投诉解决后对同一 `(domain, object_id)` 启用冷却期，防止骚扰式投诉
- **证据链上追踪**：补充证据存入 `ComplaintEvidenceCids` 存储，完整审计链
- **证据 CID 锁定**：仲裁期间自动调用 `CidLockManager` 锁定相关证据，防止删除
- **防膨胀**：三级策略 — 裁决即归档（`ArchivedDispute` ~30B）、投诉延迟归档（`ComplaintArchiveDelayBlocks`）、TTL 自动清理（`ArchiveTtlBlocks`），所有游标环形回绕
- **投诉押金**：通过 `PricingProvider` 实时换算 ≈1 USDT 等值 NEX，兜底值 `ComplaintDeposit`
- **分级罚没**：投诉败诉罚没率三级回退 — `DomainPenaltyRates`（治理可调）→ `ComplaintType::penalty_rate()`（按类型差异化）→ `ComplaintSlashBps`（全局兜底）
- **资金容错**：押金 slash/release 失败时发出 `DepositOperationFailed` 事件 + 兜底释放，防止资金永久锁定
- **紧急控制**：`set_paused` 全局暂停 + `force_close_*` 强制关闭卡住案件
- **封禁请求**：重度欺诈投诉（如 `OtcTradeFraud`）裁决后通过事件通知外部系统执行封禁
- **存储迁移**：`on_runtime_upgrade` 框架就绪，版本检查 + 自动升级

---

## 代码组织

```
pallets/dispute/arbitration/src/
  lib.rs           -- pallet 框架、Config、Event、Error、22 个 Extrinsics、Hooks、Runtime API helpers
  types.rs         -- 所有类型定义 (Decision, ComplaintType×26, ComplaintStatus×10, 归档结构体, 统计结构体)
  runtime_api.rs   -- Runtime API 声明 (3 个 API) + 返回类型 (ComplaintSummary, ComplaintDetail)
  state_machine.rs -- 状态转换表 (15 条规则) + 验证函数 (can_transition) + 3 个单元测试
  dispute.rs       -- Dispute 子系统 helpers (deposit handling, evidence locking, archive)
  complaint.rs     -- Complaint 子系统 helpers (slash with fallback, refund with event, active count)
  cleanup.rs       -- on_idle 5 阶段处理 (expire, auto-escalate, archive, cleanup×2)，环形游标
  weights.rs       -- 15 个权重函数 (手工估算 + SubstrateWeight<T> / () 双实现)
  benchmarking.rs  -- 15 个 benchmark (frame_benchmarking::v2)
  mock.rs          -- 测试 mock runtime (5 个 mock trait 实现)
  tests.rs         -- 68 个测试用例
```

---

## 业务域

| 域常量 | 字面量 | 对应 Pallet | 业务 | 投诉类型数 | 状态 |
|--------|--------|-------------|------|:---:|:---:|
| `ENTITY_ORDER` | `entorder` | `entity/order` | 商城订单争议 | 4 | **已集成** |
| `OTC_ORDER` | `otc_ord_` | `trading/nex-market` | OTC 法币交易 | 4 | 已定义 |
| `MAKER` | `maker___` | `trading/nex-market` | 做市商 | 3 | 已定义 |
| `NFT_TRADE` | `nft_trd_` | `trading/nex-market` | NFT 交易（预留） | 4 | 预留 |
| `SWAP` | `swap____` | `trading/nex-market` | Swap 交换（预留） | 3 | 预留 |
| `MEMBER` | `member__` | `entity/member` | 会员 | 2 | 已定义 |
| `ADS` | `ads_____` | `ads/core` | 广告 | 3 | 已定义 |
| `TOKNSALE` | `toknsale` | `entity/tokensale` | 代币发售 | 2 | 已定义 |
| `OTHER` | `other___` | — | 通用 | 1 | 已定义 |

> **已集成**：Runtime `UnifiedArbitrationRouter` 已实现完整路由（`can_dispute` / `apply_decision` / `get_counterparty` / `get_order_amount`）。
> **已定义**：域常量和 `ComplaintType` 已定义，Router 使用默认行为（`can_dispute → true`，`apply_decision → Ok(())`）。
> **预留**：域常量和类型已定义，待对应 Pallet 上线后集成。

---

## Extrinsics

### 仲裁子系统（资金争议）

| call_index | 方法 | 签名者 | 说明 |
|:---:|------|:---:|------|
| ~~0~~ | ~~`dispute`~~ | — | **已废弃**，返回 `Deprecated` |
| 1 | `arbitrate` | DecisionOrigin | 裁决（0=Release, 1=Refund, 2=Partial），无效 code 返回 `InvalidDecisionCode`，bps 自动 `.min(10_000)` |
| ~~2~~ | ~~`dispute_with_evidence_id`~~ | — | **已废弃**，返回 `Deprecated` |
| 3 | `append_evidence_id` | 当事人 | 追加证据引用（需 `can_dispute` 权限 + 存在性校验） |
| 4 | `dispute_with_two_way_deposit` | 当事人 | **推荐** — 双向押金仲裁，锁发起方 `DepositRatioBps` 押金 |
| 5 | `respond_to_dispute` | 应诉方 | 应诉 — 锁应诉方同额押金 + 提交反驳证据 |
| 20 | `request_default_judgment` | 发起方 | 缺席裁决 — 应诉方超时未应诉，自动 Refund |
| 23 | `settle_dispute` | 任一方 | 纠纷和解 — 释放双方押金（需双方已参与） |
| 25 | `dismiss_dispute` | DecisionOrigin | 驳回无效纠纷 — Release + 罚没发起方押金 |
| 28 | `force_close_dispute` | DecisionOrigin | 强制关闭 — 释放所有押金，不罚没 |

### 投诉子系统（行为投诉）

| call_index | 方法 | 签名者 | 说明 |
|:---:|------|:---:|------|
| 10 | `file_complaint` | 投诉人 | 发起投诉（锁 ≈1 USDT 等值押金，含冷却期/频率检查） |
| 11 | `respond_to_complaint` | 被投诉人 | 响应（独立 `response_cid`，需在截止前） |
| 12 | `withdraw_complaint` | 投诉人 | 撤诉（全额退还押金） |
| 13 | `settle_complaint` | 投诉人 | 和解（仅投诉方可发起 + 设置冷却期） |
| 14 | `escalate_to_arbitration` | 任一方 | 升级到仲裁（Submitted 状态需响应截止后才可升级） |
| 15 | `resolve_complaint` | DecisionOrigin | 裁决投诉（0=投诉方胜, 1=被投诉方胜, ≥2=部分裁决） |
| 21 | `supplement_complaint_evidence` | 投诉人 | 补充证据（存入链上 `ComplaintEvidenceCids`） |
| 22 | `supplement_response_evidence` | 被投诉人 | 补充反驳证据（存入链上） |
| 24 | `start_mediation` | DecisionOrigin | 启动调解阶段 |
| 26 | `dismiss_complaint` | DecisionOrigin | 驳回无效投诉 — 罚没投诉人押金 |
| 29 | `force_close_complaint` | DecisionOrigin | 强制关闭 — 全额退还押金 |
| 31 | `appeal` | 败诉方 | 申诉 — 缴纳 2x 市场价押金，在 `AppealWindowBlocks` 内提出 |
| 32 | `resolve_appeal` | DecisionOrigin | 申诉裁决 — 最终裁决，不可再上诉 |

### 管理 Extrinsics

| call_index | 方法 | 签名者 | 说明 |
|:---:|------|:---:|------|
| 27 | `set_paused` | DecisionOrigin | 紧急暂停/恢复模块 |
| 30 | `set_domain_penalty_rate` | DecisionOrigin | 动态设置域惩罚比例（`None` 移除覆盖，上限 10000 bps） |

---

## 状态机

所有转换通过 `state_machine::can_transition()` 统一验证（15 条规则）。

```
                    ┌─ withdraw_complaint ─→ Withdrawn（退还押金）
                    │
Submitted ──────────┤─ respond_to_complaint → Responded
    │               │─ (deadline 过期) ────→ Expired（退还押金）
    │               │─ escalate (deadline 后) → Arbitrating（手动升级）
    │               │
    ▼               │
Responded ──────────┤─ settle_complaint ──→ ResolvedSettlement（退还押金 + 冷却期）
    │               │─ withdraw_complaint → Withdrawn（退还押金）
    │               │─ (auto_escalate) ──→ Arbitrating（on_idle 自动升级）
    ├─ escalate ────┤
    │               │
    ▼               │
Mediating ──────────┤─ settle_complaint ──→ ResolvedSettlement（退还押金 + 冷却期）
    │               │─ (max_lifetime) ───→ Expired
    └─ escalate ────┘
                    │
                    ▼
            Arbitrating ──────→ resolve_complaint
                │               ├ decision=0 → ComplainantWin（退还押金 + 冷却期）
                │               ├ decision=1 → RespondentWin（罚没押金 + 冷却期）
                │               └ decision≥2 → ComplainantWin（退还押金 + 冷却期，部分裁决）
                │
                └─ (max_lifetime) → Expired

    ┌─────────────────────────────────────────────┐
    │             申诉机制 (Appeal)                │
    │                                             │
    │  ComplainantWin ─┐                          │
    │                  ├─ appeal (败诉方) ─→ Appealed
    │  RespondentWin ──┘   (2x 市场价押金)   │    │
    │                                        ▼    │
    │                              resolve_appeal │
    │                         ├ ComplainantWin     │
    │                         └ RespondentWin      │
    │                    (申诉成功退还，失败罚没)   │
    └─────────────────────────────────────────────┘

终态（可归档）: Withdrawn, Expired, ResolvedSettlement,
                ResolvedComplainantWin, ResolvedRespondentWin

治理强制入口:
  dismiss_complaint     → ResolvedRespondentWin（罚没押金）
  force_close_complaint → Withdrawn（退还押金）
```

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
   └→ archive_and_cleanup                        (归档 + 清理 5 项存储)
```

**替代路径：**

| 路径 | 触发条件 | 行为 |
|------|---------|------|
| 缺席裁决 | 应诉方超时未应诉 | `request_default_judgment` → 自动 Refund + 归档 |
| 双方和解 | 应诉方已锁定押金 | `settle_dispute` → 全额释放双方押金 + 清理 |
| 驳回 | 仲裁委员会认定无效 | `dismiss_dispute` → Release + 罚没发起方 |
| 强制关闭 | 案件卡住/异常 | `force_close_dispute` → 释放所有押金 |

### 投诉押金规则

**押金计算**（`file_complaint` 和 `appeal`）：

```
base_deposit = max(
    ComplaintDepositUsd / nex_to_usd_rate,  // 实时 USD 定价
    ComplaintDeposit                         // 兜底最低值
)
appeal_deposit = base_deposit × 2           // 申诉加倍
```

**罚没规则**（投诉败诉时）：

罚没比例按以下优先级确定：
1. **`DomainPenaltyRates`** — 治理动态配置的域级覆盖
2. **`ComplaintType::penalty_rate()`** — 按投诉类型差异化（如 `OtcTradeFraud` = 80%）
3. **`ComplaintSlashBps`** — 全局兜底值

罚没部分转给被投诉方，余额退还投诉人。

| 裁决结果 | 押金处理 | 统计归入 |
|---------|---------|---------|
| 投诉方胜诉（decision=0） | 全额退还 | `complainant_wins` |
| 被投诉方胜诉（decision=1） | 按优先级罚没 → 被投诉方 | `respondent_wins` |
| 部分裁决（decision≥2） | 全额退还 | `complainant_wins` |
| 和解 | 全额退还 + 设置冷却期 | `settlements` |
| 撤诉 | 全额退还 | — |
| 过期 | 全额退还 | `expired_count` |
| 驳回 | 按优先级罚没 → 被投诉方 | `respondent_wins` |

**资金容错**：slash/release 失败时发出 `DepositOperationFailed { complaint_id, operation, amount }` 事件（operation: 0=slash, 1=return, 2=refund），slash 失败后尝试 release 兜底。

---

## 存储

### 仲裁存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Disputed` | `DoubleMap<[u8;8], u64, ()>` | 争议登记标记 |
| `EvidenceIds` | `DoubleMap<[u8;8], u64, BoundedVec<u64>>` | 证据引用列表 |
| `LockedCidHashes` | `DoubleMap<[u8;8], u64, BoundedVec<Hash>>` | 锁定的 CID 哈希 |
| `TwoWayDeposits` | `DoubleMap<[u8;8], u64, TwoWayDepositRecord>` | 双向押金记录 |
| `PendingArbitrationDisputes` | `DoubleMap<[u8;8], u64, ()>` | 待裁决纠纷队列 |

### 投诉存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextComplaintId` | `u64` | 投诉 ID 自增计数器 |
| `Complaints` | `Map<u64, Complaint<T>>` | 活跃投诉主存储 |
| `ComplaintDeposits` | `Map<u64, Balance>` | 投诉押金记录（resolve 后清除） |
| `ActiveComplaintCount` | `Map<AccountId, u32>` | 用户活跃投诉计数器 |
| `ComplaintEvidenceCids` | `Map<u64, BoundedVec<BoundedVec<u8>>>` | 投诉证据 CID 链上审计链 |
| `ComplaintCooldown` | `DoubleMap<[u8;8], u64, BlockNumber>` | 投诉冷却期到期时间 |
| `PendingArbitrationComplaints` | `Map<u64, ()>` | 待裁决投诉队列 |
| `DomainStats` | `Map<[u8;8], DomainStatistics>` | 域统计信息 |
| `DomainPenaltyRates` | `Map<[u8;8], u16>` | 域惩罚比例动态配置 |

### 归档存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextArchivedId` | `u64` | 仲裁归档 ID 计数器 |
| `ArchivedDisputes` | `Map<u64, ArchivedDispute>` | 归档仲裁（~30B） |
| `ArchivedComplaints` | `Map<u64, ArchivedComplaint>` | 归档投诉（~30B） |
| `ArbitrationStats` | `ArbitrationPermanentStats` | 仲裁永久统计 |

### 游标与控制

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `ComplaintExpiryCursor` | `u64` | 投诉过期扫描游标（环形回绕） |
| `ComplaintArchiveCursor` | `u64` | 投诉归档扫描游标（环形回绕） |
| `AutoEscalateCursor` | `u64` | 自动升级扫描游标（环形回绕） |
| `ArchiveDisputeCleanupCursor` | `u64` | 仲裁归档 TTL 清理游标（环形回绕） |
| `ArchiveComplaintCleanupCursor` | `u64` | 投诉归档 TTL 清理游标（环形回绕） |
| `Paused` | `bool` | 全局暂停开关 |

---

## 主要类型

### Decision

```rust
pub enum Decision {
    Release,      // 全额释放（卖家胜）
    Refund,       // 全额退款（买家胜）
    Partial(u16), // 按比例分配，bps 0–10000（自动 .min(10_000)）
}
```

### ComplaintStatus

```rust
pub enum ComplaintStatus {
    Submitted,              // 已提交，等待响应
    Responded,              // 已响应
    Mediating,              // 调解中
    Arbitrating,            // 仲裁中
    ResolvedComplainantWin, // 投诉方胜诉
    ResolvedRespondentWin,  // 被投诉方胜诉
    ResolvedSettlement,     // 和解
    Withdrawn,              // 已撤销
    Expired,                // 已过期
    Appealed,               // 申诉中
}
```

- `is_resolved()` → `true`：`ResolvedComplainantWin/RespondentWin/Settlement/Withdrawn/Expired`
- `is_active()` → `true`：`Submitted/Responded/Mediating/Arbitrating/Appealed`

### Complaint

```rust
pub struct Complaint<T: Config> {
    pub id: u64,
    pub domain: [u8; 8],
    pub object_id: u64,
    pub complaint_type: ComplaintType,
    pub complainant: T::AccountId,
    pub respondent: T::AccountId,
    pub details_cid: BoundedVec<u8, T::MaxCidLen>,
    pub response_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
    pub amount: Option<BalanceOf<T>>,
    pub status: ComplaintStatus,
    pub created_at: BlockNumberFor<T>,
    pub response_deadline: BlockNumberFor<T>,
    pub settlement_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
    pub resolution_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
    pub appeal_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
    pub appellant: Option<T::AccountId>,
    pub updated_at: BlockNumberFor<T>,
}
```

### TwoWayDepositRecord

```rust
pub struct TwoWayDepositRecord<AccountId, Balance, BlockNumber> {
    pub initiator: AccountId,
    pub initiator_deposit: Balance,
    pub respondent: AccountId,
    pub respondent_deposit: Option<Balance>,
    pub response_deadline: BlockNumber,
    pub has_responded: bool,
}
```

### ComplaintType（26 种，覆盖 9 业务域）

| 业务域 | 投诉类型 | `penalty_rate()` | `triggers_permanent_ban()` |
|-------|---------|:---:|:---:|
| 商城订单 | `EntityOrderNotDeliver`, `EntityOrderFalseClaim`, `EntityOrderQuality` | 3000 (30%) | — |
| 商城订单 | `EntityOrderFraud` | **8000 (80%)** | **YES** |
| OTC | `OtcSellerNotDeliver`, `OtcBuyerFalseClaim`, `OtcPriceDispute` | 3000 (30%) | — |
| OTC | `OtcTradeFraud` | **8000 (80%)** | **YES** |
| 做市商 | `MakerMaliciousOperation` | **5000 (50%)** | — |
| 做市商 | 其余 2 种 | 3000 (30%) | — |
| NFT | 4 种 | 3000 (30%) | — |
| Swap | 3 种 | 3000 (30%) | — |
| 会员 | 2 种 | 3000 (30%) | — |
| 广告 | `AdsFraudClick` | **5000 (50%)** | — |
| 广告 | 其余 2 种 | 3000 (30%) | — |
| 代币发售 | 2 种 | 3000 (30%) | — |
| 其他 | `Other` | 3000 (30%) | — |

### HoldReason

```rust
pub enum HoldReason {
    DisputeInitiator,   // 仲裁发起方押金
    DisputeRespondent,  // 仲裁应诉方押金
    ComplaintDeposit,   // 投诉/申诉押金
}
```

---

## 事件

### 仲裁事件

| 事件 | 说明 |
|------|------|
| `Disputed { domain, id }` | 争议已登记（已废弃路径） |
| `Arbitrated { domain, id, decision, bps }` | 裁决完成 |
| `DisputeWithDepositInitiated { domain, id, initiator, respondent, deposit, deadline }` | 双向押金仲裁发起 |
| `RespondentDepositLocked { domain, id, respondent, deposit }` | 应诉方押金锁定 |
| `DepositProcessed { domain, id, account, released, slashed }` | 押金处理完成 |
| `DefaultJudgment { domain, id, initiator }` | 缺席裁决 |
| `DisputeSettled { domain, id }` | 纠纷和解 |
| `DisputeDismissed { domain, id }` | 纠纷被驳回 |
| `DisputeForceClosed { domain, id }` | 纠纷被强制关闭 |

### 投诉事件

| 事件 | 说明 |
|------|------|
| `ComplaintFiled { complaint_id, domain, object_id, complainant, respondent, complaint_type }` | 投诉已提交 |
| `ComplaintResponded { complaint_id, respondent }` | 投诉已响应 |
| `ComplaintWithdrawn { complaint_id }` | 投诉已撤销 |
| `ComplaintSettled { complaint_id }` | 投诉已和解 |
| `ComplaintEscalated { complaint_id }` | 投诉已升级到仲裁 |
| `ComplaintResolved { complaint_id, decision }` | 投诉裁决完成 |
| `ComplaintExpired { complaint_id }` | 投诉已过期 |
| `ComplaintArchived { complaint_id }` | 投诉已归档 |
| `ComplaintAutoEscalated { complaint_id }` | 投诉自动升级到仲裁 |
| `ComplaintEvidenceSupplemented { complaint_id, who, evidence_cid }` | 补充证据（已存链） |
| `ComplaintMediationStarted { complaint_id }` | 投诉进入调解 |
| `ComplaintDismissed { complaint_id }` | 投诉被驳回 |
| `ComplaintForceClosed { complaint_id }` | 投诉被强制关闭 |
| `AppealFiled { complaint_id, appellant }` | 申诉已提出 |
| `AppealResolved { complaint_id, decision }` | 申诉裁决完成 |
| `AccountBanRequested { domain, object_id, account }` | 封禁请求（通过事件通知外部系统） |
| `DepositOperationFailed { complaint_id, operation, amount }` | 押金操作失败（0=slash, 1=return, 2=refund） |

### 管理事件

| 事件 | 说明 |
|------|------|
| `PausedStateChanged { paused }` | 模块暂停/恢复 |
| `DomainPenaltyRateUpdated { domain, rate_bps }` | 域惩罚比例已更新 |

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
| `NotAuthorized` | 无权操作（含自我投诉/仲裁检查） |
| `InvalidComplaintType` | 投诉类型与域不匹配 |
| `InvalidState` | 无效的状态转换（由状态机验证） |
| `TooManyComplaints` | CID 锁定/证据列表已满 |
| `TooManyActiveComplaints` | 用户活跃投诉超限（`MaxActivePerUser`） |
| `EvidenceNotFound` | 引用的 evidence_id 不存在 |
| `ResponseDeadlineNotReached` | 应诉期未到（缺席裁决/Submitted 升级） |
| `SettlementNotConfirmed` | 纠纷和解需要双方已参与 |
| `ModulePaused` | 模块已暂停 |
| `InvalidPenaltyRate` | 惩罚比例超过 10000 bps |
| `Deprecated` | 已废弃的接口（call_index 0, 2） |
| `CooldownActive` | 冷却期未结束 |
| `AppealWindowClosed` | 申诉窗口已关闭 |
| `CannotAppeal` | 无法申诉（胜诉方或非当事方） |
| `InvalidDecisionCode` | 无效裁决代码（必须为 0/1/2） |

---

## Trait 接口

### ArbitrationRouter（域路由）

```rust
pub trait ArbitrationRouter<AccountId, Balance> {
    fn can_dispute(domain: [u8; 8], who: &AccountId, id: u64) -> bool;
    fn apply_decision(domain: [u8; 8], id: u64, decision: Decision) -> DispatchResult;
    fn get_counterparty(domain: [u8; 8], initiator: &AccountId, id: u64)
        -> Result<AccountId, DispatchError>;
    fn get_order_amount(domain: [u8; 8], id: u64) -> Result<Balance, DispatchError>;
}
```

### EvidenceExistenceChecker（证据存在性校验）

```rust
pub trait EvidenceExistenceChecker {
    fn evidence_exists(id: u64) -> bool;
}
```

---

## Runtime API

`runtime_api.rs` 声明 3 个 RPC 查询接口，扫描上限 `MAX_API_SCAN = 10,000`，避免 RPC 超时。

| API | 参数 | 返回值 | 说明 |
|-----|------|--------|------|
| `get_complaints_by_status` | `status, offset, limit` | `Vec<ComplaintSummary>` | 按状态分页查询（limit 上限 100，扫描上限 10,000） |
| `get_user_complaints` | `account` | `Vec<u64>` | 用户关联的活跃投诉 ID（扫描上限 10,000） |
| `get_complaint_detail` | `complaint_id` | `Option<ComplaintDetail>` | 聚合查询：投诉 + 押金 + 证据 CID（一次调用替代 3 次 storage query） |

### 返回类型

```rust
pub struct ComplaintSummary<AccountId, Balance> {
    pub id: u64,
    pub domain: [u8; 8],
    pub object_id: u64,
    pub complaint_type: ComplaintType,
    pub complainant: AccountId,
    pub respondent: AccountId,
    pub amount: Option<Balance>,
    pub status: ComplaintStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

pub struct ComplaintDetail<AccountId, Balance> {
    // ComplaintSummary 所有字段 +
    pub details_cid: Vec<u8>,
    pub response_cid: Option<Vec<u8>>,
    pub response_deadline: u64,
    pub settlement_cid: Option<Vec<u8>>,
    pub resolution_cid: Option<Vec<u8>>,
    pub appeal_cid: Option<Vec<u8>>,
    pub appellant: Option<AccountId>,
    pub deposit: Option<Balance>,       // 来自 ComplaintDeposits
    pub evidence_cids: Vec<Vec<u8>>,    // 来自 ComplaintEvidenceCids
}
```

---

## Hooks

### on_runtime_upgrade

版本检查 + 自动升级框架。当 `on_chain_version < STORAGE_VERSION` 时执行迁移并更新版本号。

### on_idle（剩余权重利用，5 阶段）

每个区块空闲时按优先级依次处理，所有游标到达末尾后自动回绕到 0：

| 阶段 | 操作 | 每次上限 | 说明 |
|:---:|------|:---:|------|
| 1 | `expire_old_complaints` | 5 | Submitted 超过 deadline 或活跃投诉超过 `ComplaintMaxLifetimeBlocks` → Expired |
| 2 | `auto_escalate_stale_complaints` | 3 | Responded 超过 `AutoEscalateBlocks` → Arbitrating |
| 3 | `archive_old_complaints` | 10 | 已解决投诉超过 `max(ComplaintArchiveDelayBlocks, AppealWindowBlocks)` → 归档 + 清理关联存储 |
| 4 | `cleanup_old_archived_disputes` | 5 | 归档仲裁超过 `ArchiveTtlBlocks` → 删除 |
| 5 | `cleanup_old_archived_complaints` | 5 | 归档投诉超过 `ArchiveTtlBlocks` → 删除 |

权重预算：每阶段独立计算 `cursor_overhead + scan × per_item_read + process × per_item_write`，仅在剩余权重充足时执行。

---

## 配置参数

### 核心配置

| 参数 | 类型 | 说明 |
|------|------|------|
| `MaxEvidence` | `u32` | 每案最大证据引用数 |
| `MaxCidLen` | `u32` | CID 最大字节长度 |
| `Escrow` | trait | 托管接口 |
| `Router` | trait | 域路由（runtime 注入） |
| `DecisionOrigin` | `EnsureOrigin` | 裁决 Origin |
| `WeightInfo` | trait | 权重信息（15 个函数） |

### 押金配置

| 参数 | 类型 | 说明 |
|------|------|------|
| `Fungible` | trait | `Inspect + Mutate + MutateHold` |
| `RuntimeHoldReason` | enum | 押金锁定原因标识（3 种） |
| `DepositRatioBps` | `u16` | 仲裁押金比例（1500 = 15%） |
| `ResponseDeadline` | `BlockNumber` | 应诉期限 |
| `RejectedSlashBps` | `u16` | 败诉罚没（3000 = 30%） |
| `PartialSlashBps` | `u16` | 部分胜诉罚没（5000 = 50%） |
| `TreasuryAccount` | `AccountId` | 国库账户 |
| `ComplaintDeposit` | `Balance` | 投诉押金兜底金额 |
| `ComplaintDepositUsd` | `u64` | 投诉押金 USD 价值（精度 10^6） |
| `Pricing` | trait | `PricingProvider`（USD→NEX 换算） |
| `ComplaintSlashBps` | `u16` | 投诉败诉罚没全局兜底 |

### 时间参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `ComplaintMaxLifetimeBlocks` | `BlockNumber` | 投诉最大活跃时长 |
| `AppealWindowBlocks` | `BlockNumber` | 申诉时间窗口 |
| `AutoEscalateBlocks` | `BlockNumber` | 自动升级超时 |
| `ArchiveTtlBlocks` | `u32` | 归档记录 TTL（0 禁用清理） |
| `ComplaintArchiveDelayBlocks` | `BlockNumber` | 投诉归档延迟 |

### 限制参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `MaxActivePerUser` | `u32` | 每用户最大活跃投诉数 |

### 集成配置

| 参数 | 类型 | 说明 |
|------|------|------|
| `CidLockManager` | trait | CID 锁定管理器 |
| `EvidenceExists` | trait | 证据存在性检查器 |

---

## 前端集成

### Hooks（`useArbitration.ts`，共 17 个）

**查询 Hooks：**

| Hook | 数据源 | 返回值 |
|------|--------|--------|
| `useComplaints(status?, offset, limit)` | Runtime API / Storage | 投诉列表 |
| `useComplaint(id)` | Storage `complaints(id)` | 单条投诉 |
| `useUserComplaints(address)` | Runtime API `getUserComplaints` | 用户活跃投诉 ID |
| `useComplaintDetail(id)` | Runtime API `getComplaintDetail` | 聚合详情（投诉+押金+证据） |
| `useArbitrationStats()` | Storage `arbitrationStats` | 全局统计 |
| `useDomainStats(domain)` | Storage `domainStats` | 域统计 |
| `useArbitrationPaused()` | Storage `paused` | 暂停状态 |
| `useEvidenceIds(domain, id)` | Storage `evidenceIds` | 争议证据 ID 列表 |
| `useLockedCidHashes(domain, id)` | Storage `lockedCidHashes` | 锁定的 CID 哈希 |
| `useTwoWayDeposit(domain, id)` | Storage `twoWayDeposits` | 双方押金状态 |
| `useArchivedDispute(archivedId)` | Storage `archivedDisputes` | 归档争议 |
| `useArchivedComplaint(complaintId)` | Storage `archivedComplaints` | 归档投诉 |
| `usePendingArbitrationDisputes()` | Storage entries | 待仲裁争议列表 |
| `usePendingArbitrationComplaints()` | Storage entries | 待仲裁投诉列表 |
| `useComplaintCooldown(domain, objectId)` | Storage `complaintCooldown` | 冷却期到期区块 |
| `useDomainPenaltyRate(domain)` | Storage `domainPenaltyRates` | 域罚没率 |

**操作 Hook：**

`useArbitrationActions()` 提供 22 个操作方法（11 投诉 + 8 争议 + 2 管理 + 1 上诉）。

### 推荐查询策略

| 场景 | 推荐接口 |
|------|---------|
| 投诉列表（按状态筛选） | Runtime API `getComplaintsByStatus` |
| 投诉详情页 | Runtime API `getComplaintDetail` |
| 我的投诉 | Runtime API `getUserComplaints` → 批量 `getComplaintDetail` |
| 争议详情页 | `useTwoWayDeposit` + `useEvidenceIds` + `useLockedCidHashes` |
| 管理员仪表盘 | `usePendingArbitrationDisputes` + `usePendingArbitrationComplaints` |
| 投诉按钮冷却 | `useComplaintCooldown` |
| 全局/域统计 | `useArbitrationStats` + `useDomainStats` |

---

## 集成示例

### 实现 ArbitrationRouter

```rust
pub struct OrderArbitrationRouter;

impl ArbitrationRouter<AccountId, Balance> for OrderArbitrationRouter {
    fn can_dispute(domain: [u8; 8], who: &AccountId, id: u64) -> bool {
        if domain != *b"otc_ord_" { return false; }
        Orders::is_participant(id, who)
    }

    fn apply_decision(domain: [u8; 8], id: u64, decision: Decision) -> DispatchResult {
        let (buyer, seller) = Orders::get_parties(id)?;
        match decision {
            Decision::Release => Escrow::release_all(id, &seller),
            Decision::Refund  => Escrow::refund_all(id, &buyer),
            Decision::Partial(bps) => Escrow::split_partial(id, &seller, &buyer, bps),
        }
    }

    fn get_counterparty(_domain: [u8; 8], initiator: &AccountId, id: u64)
        -> Result<AccountId, DispatchError> {
        Orders::get_other_party(id, initiator)
    }

    fn get_order_amount(_domain: [u8; 8], id: u64) -> Result<Balance, DispatchError> {
        Orders::get_amount(id)
    }
}
```

### Runtime 配置

```rust
impl pallet_dispute_arbitration::pallet::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxEvidence = ConstU32<20>;
    type MaxCidLen = ConstU32<64>;
    type Escrow = pallet_dispute_escrow::Pallet<Runtime>;
    type Router = OrderArbitrationRouter;
    type DecisionOrigin = EnsureProportionAtLeast<AccountId, Council, 2, 3>;
    type WeightInfo = pallet_dispute_arbitration::weights::SubstrateWeight<Runtime>;
    type Fungible = Balances;
    type RuntimeHoldReason = RuntimeHoldReason;
    type DepositRatioBps = ConstU16<1500>;
    type ResponseDeadline = ConstU32<{ 7 * DAYS }>;
    type RejectedSlashBps = ConstU16<3000>;
    type PartialSlashBps = ConstU16<5000>;
    type ComplaintDeposit = ConstU128<{ UNIT / 10 }>;
    type ComplaintDepositUsd = ConstU64<1_000_000>;
    type Pricing = TradingPricingProvider;
    type ComplaintSlashBps = ConstU16<5000>;
    type TreasuryAccount = TreasuryAccountId;
    type CidLockManager = pallet_storage_service::Pallet<Runtime>;
    type ArchiveTtlBlocks = ConstU32<2_592_000>;
    type ComplaintArchiveDelayBlocks = ConstU32<432_000>;
    type ComplaintMaxLifetimeBlocks = ConstU32<1_296_000>;
    type EvidenceExists = EvidenceExistenceCheckerImpl;
    type AppealWindowBlocks = ConstU32<{ 3 * DAYS }>;
    type AutoEscalateBlocks = ConstU32<{ 14 * DAYS }>;
    type MaxActivePerUser = ConstU32<50>;
}
```

---

## 测试

测试文件：`src/tests.rs`（68 个测试）+ `src/state_machine.rs`（3 个内联测试）

```bash
cargo test -p pallet-dispute-arbitration
# test result: ok. 68 passed; 0 failed
```

| 类别 | 数量 | 覆盖内容 |
|------|:---:|---------|
| 仲裁基础 | 10 | dispute_deprecated×2, arbitrate (release/refund/partial/invalid_code/default_bps) |
| 争议交互 | 8 | respond_to_dispute (正常/错误方/超时), settle_dispute (正常/未响应), dismiss_dispute, force_close_dispute, request_default_judgment (正常/未超时) |
| 投诉基础 | 10 | file, respond (正常/超时/非被告), withdraw, settle, escalate (正常/Submitted 超时后/Submitted 未超时) |
| 投诉裁决 | 4 | resolve_complaint (complainant_win/respondent_win_slash), start_mediation (正常/无效状态) |
| 状态机 | 3 | valid/invalid transitions, status predicates |
| 申诉 | 4 | 败诉方申诉, 胜诉方被拒, 窗口过期, resolve_appeal (含投诉方为上诉方) |
| 自动升级 | 1 | auto_escalate_stale_responded |
| 过期/归档 | 4 | expire_submitted, expire_responded, expire_arbitrating, archive |
| 证据追踪 | 1 | supplement_evidence 存入链上 |
| 冷却期 | 1 | cooldown 阻止重复投诉, 冷却后允许 |
| 频率限制 | 1 | MaxActivePerUser 限制 |
| 域惩罚率 | 2 | 域覆盖, 按类型差异化 |
| 统计归类 | 1 | 部分裁决计入 complainant_wins |
| 暂停控制 | 1 | paused_blocks_operations |
| 强制关闭/驳回 | 2 | force_close_complaint, dismiss_complaint (slash + stats) |
| 安全检查 | 4 | self-complaint rejected, not_authorized, deadline checks |
| 废弃接口 | 2 | call_index 0, 2 返回 Deprecated |

### Benchmarking

`src/benchmarking.rs` 使用 `frame_benchmarking::v2` 覆盖 15 个 extrinsic：

`dispute`, `arbitrate`, `file_complaint`, `respond_to_complaint`, `withdraw_complaint`, `settle_complaint`, `escalate_to_arbitration`, `request_default_judgment`, `supplement_evidence`, `settle_dispute`, `start_mediation`, `dismiss_dispute`, `dismiss_complaint`, `force_close_dispute`, `force_close_complaint`

---

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-01 | 初始版本：双向押金仲裁 + 投诉系统 |
| v0.2.0 | 2026-03 | 深度重构：模块拆分、形式化状态机、申诉机制、自动升级、冷却期、证据链上追踪、存储优化、废弃 call_index 0/2 |
| v0.2.1 | 2026-03 | 生产就绪修复：上诉押金基于市场价、decision_code 校验 + `InvalidDecisionCode` 错误、bps 上限校验、Runtime API 扫描上限 10K、押金操作失败事件 + 兜底释放、归档游标环形回绕、Submitted 直升 Arbitrating（deadline 后）、`on_runtime_upgrade` 框架、benchmarking 重写覆盖 15 个 extrinsic、争议子系统 6 个 extrinsic 测试补全（68 测试）、前端 17 个 Hook 全覆盖 |

---

## 相关模块

- [escrow/](../escrow/) — 资金托管（裁决分账接口 `Escrow<AccountId, Balance>`）
- [evidence/](../evidence/) — 证据管理（`evidence_id` 引用 + `EvidenceExistenceChecker`）
- [trading/common/](../../trading/common/) — `PricingProvider`（投诉押金 USD→NEX 换算）
- [storage/service/](../../storage/service/) — `CidLockManager`（证据 CID 锁定/解锁）
- [storage/lifecycle/](../../storage/lifecycle/) — `block_to_year_month`（归档时间戳）
