# pallet-arbitration

> 路径：`pallets/dispute/arbitration/` · Runtime Index: 62

仲裁与投诉统一处理系统。内含两大子系统：**仲裁系统**（资金争议、双向押金、裁决分账）与**投诉系统**（行为投诉、押金防滥用、自动过期）。支持 8 个业务域，通过 `ArbitrationRouter` trait 实现域路由解耦。

## 设计理念

- **域路由架构**：`[u8;8]` 域标识 + `ArbitrationRouter` trait，一套仲裁逻辑服务所有业务 pallet
- **双向押金**：发起方/应诉方各从托管锁定订单金额 15%（`Fungible::hold`），败诉方罚没 30%
- **证据 CID 锁定**：仲裁期间自动调用 `CidLockManager` 锁定相关证据，防止删除；裁决后自动解锁
- **信用集成**：裁决结果通过 `CreditUpdater` 反馈到做市商信用系统
- **防膨胀**：三级策略 — 裁决即归档（`ArchivedDispute` ~30B）、投诉延迟归档、TTL 自动清理
- **投诉押金**：通过 `PricingProvider` 实时换算 ≈1 USDT 等值 COS，防止恶意投诉

## 架构概览

```
                        ┌───────────────────────────────┐
                        │       pallet-arbitration       │
                        │  ┌─────────────┬────────────┐  │
                        │  │ 仲裁子系统   │ 投诉子系统  │  │
                        │  │ (资金争议)   │ (行为投诉)  │  │
                        │  └──────┬──────┴─────┬──────┘  │
                        └────────┼────────────┼──────────┘
                  ┌──────────────┼────────┐   │
                  ▼              ▼        ▼   ▼
           ┌──────────┐  ┌──────────┐  ┌──────────────────┐
           │  Escrow  │  │ Evidence │  │ trading-common    │
           │ 资金托管  │  │ 证据管理  │  │ PricingProvider  │
           └──────────┘  └──────────┘  └──────────────────┘
```

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

## Extrinsics

### 仲裁子系统

| call_index | 方法 | 签名者 | 说明 |
|:---:|------|:---:|------|
| 0 | `dispute` | 当事人 | 发起仲裁（旧版，CID 存链） |
| 1 | `arbitrate` | DecisionOrigin | 裁决 → Router 分账 + 押金处理 + CID 解锁 + 信用分 + 归档 |
| 2 | `dispute_with_evidence_id` | 当事人 | 以 evidence_id 引用发起仲裁 |
| 3 | `append_evidence_id` | 当事人 | 追加证据引用（需 `can_dispute` 权限校验） |
| 4 | `dispute_with_two_way_deposit` | 当事人 | 双向押金仲裁（推荐）— 锁发起方 15% 押金 |
| 5 | `respond_to_dispute` | 应诉方 | 应诉 — 锁应诉方押金 + 提交反驳证据 |

### 投诉子系统

| call_index | 方法 | 签名者 | 说明 |
|:---:|------|:---:|------|
| 10 | `file_complaint` | 投诉人 | 发起投诉（锁 ≈1 USDT 等值 COS 押金） |
| 11 | `respond_to_complaint` | 被投诉人 | 响应/申诉（独立 `response_cid`，不覆盖原始详情） |
| 12 | `withdraw_complaint` | 投诉人 | 撤诉（退还押金） |
| 13 | `settle_complaint` | 任一方 | 和解（独立 `settlement_cid`，退还押金） |
| 14 | `escalate_to_arbitration` | 任一方 | 升级到仲裁委员会 |
| 15 | `resolve_complaint` | DecisionOrigin | 裁决投诉（独立 `resolution_cid`，胜诉退押金/败诉罚没） |

## 流程

### 仲裁流程（双向押金模式）

```
1. evidence::commit                          → 发起方提交证据
2. arbitration::dispute_with_two_way_deposit → 发起仲裁，锁发起方押金 15%
3. evidence::commit                          → 应诉方提交证据
4. arbitration::respond_to_dispute           → 应诉，锁应诉方押金 15%
5. arbitration::arbitrate                    → 治理裁决
   ├→ Router::apply_decision                     (Escrow 分账)
   ├→ handle_deposits_on_arbitration             (押金罚没/释放)
   ├→ unlock_all_evidence_cids                   (CID 解锁)
   ├→ CreditUpdater::record_maker_dispute_result (信用分)
   └→ archive_and_cleanup                        (归档 + 清理)
```

### 押金处理规则

| 裁决 | 发起方（买家）押金 | 应诉方（卖家）押金 |
|------|:---:|:---:|
| Release（卖家胜） | 罚没 `RejectedSlashBps`（30%） | 全额释放 |
| Refund（买家胜） | 全额释放 | 罚没 `RejectedSlashBps`（30%） |
| Partial（双方责任） | 罚没 `PartialSlashBps`（50%） | 罚没 `PartialSlashBps`（50%） |

罚没部分转入国库（`TreasuryAccount`），释放部分回到托管账户。

### 投诉状态机

```
Submitted ──→ Responded ──→ Mediating ──→ Arbitrating ──→ Resolved
    │              │             │                          ├ ComplainantWin
    │              │             │                          ├ RespondentWin
    │              ├→ settle_complaint → ResolvedSettlement  └ Settlement
    │              └→ escalate_to_arbitration ─────────↗
    ├→ withdraw_complaint → Withdrawn
    └→ (deadline 过期)    → Expired
```

### 投诉押金处理规则

| 裁决 | 押金处理 |
|------|---------|
| 投诉方胜诉 | 全额退还 |
| 被投诉方胜诉 | 罚没 `ComplaintSlashBps` 转给被投诉方，余额退还 |
| 和解 / 撤诉 / 过期 | 全额退还 |

## 存储

### 仲裁存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Disputed` | `DoubleMap<[u8;8], u64, ()>` | 争议登记标记 |
| `EvidenceIds` | `DoubleMap<[u8;8], u64, BoundedVec<u64>>` | 证据引用列表 |
| `LockedCidHashes` | `DoubleMap<[u8;8], u64, BoundedVec<Hash>>` | 锁定的 CID 哈希 |
| `TwoWayDeposits` | `DoubleMap<[u8;8], u64, TwoWayDepositRecord>` | 双向押金记录 |

### 归档存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextArchivedId` | `u64` | 归档 ID 计数器 |
| `ArchivedDisputes` | `Map<u64, ArchivedDispute>` | 归档仲裁（~30B/条） |
| `ArbitrationStats` | `ArbitrationPermanentStats` | 永久统计（总数/Release/Refund/Partial） |

### 投诉存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextComplaintId` | `u64` | 投诉 ID 计数器 |
| `Complaints` | `Map<u64, Complaint<T>>` | 活跃投诉主存储 |
| `ArchivedComplaints` | `Map<u64, ArchivedComplaint>` | 归档投诉（~38B/条） |
| `UserActiveComplaints` | `Map<AccountId, BoundedVec<u64, 50>>` | 用户活跃投诉索引 |
| `ComplaintDeposits` | `Map<u64, Balance>` | 投诉押金记录 |
| `DomainStats` | `Map<[u8;8], DomainStatistics>` | 域统计 |

### 游标与清理

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `ComplaintArchiveCursor` | `u64` | 投诉归档扫描游标 |
| `ComplaintExpiryCursor` | `u64` | 投诉过期扫描游标（O(batch)） |
| `ArchiveDisputeCleanupCursor` | `u64` | 仲裁归档 TTL 清理游标 |
| `ArchiveComplaintCleanupCursor` | `u64` | 投诉归档 TTL 清理游标 |

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
    pub details_cid: BoundedVec<u8, T::MaxCidLen>,
    pub response_cid: Option<BoundedVec<u8, T::MaxCidLen>>,    // 被投诉方响应
    pub amount: Option<BalanceOf<T>>,
    pub status: ComplaintStatus,
    pub created_at: BlockNumberFor<T>,
    pub response_deadline: BlockNumberFor<T>,
    pub settlement_cid: Option<BoundedVec<u8, T::MaxCidLen>>,  // 和解详情
    pub resolution_cid: Option<BoundedVec<u8, T::MaxCidLen>>,  // 裁决详情
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

### ComplaintType（25 种，覆盖 8 业务域）

| 业务域 | 投诉类型 | penalty_rate | permanent_ban |
|-------|---------|:---:|:---:|
| OTC | `OtcSellerNotDeliver`, `OtcBuyerFalseClaim`, `OtcPriceDispute` | 3000 | — |
| OTC | `OtcTradeFraud` | 8000 | YES |
| 直播 | `LiveIllegalContent` | 5000 | — |
| 直播 | `LiveFalseAdvertising`, `LiveHarassment`, `LiveFraud`, `LiveGiftRefund`, `LiveOther` | 3000 | — |
| 做市商 | `MakerMaliciousOperation` | 5000 | — |
| 做市商 | `MakerCreditDefault`, `MakerFalseQuote` | 3000 | — |
| NFT | `NftSellerNotDeliver`, `NftCounterfeit`, `NftTradeFraud`, `NftAuctionDispute` | 3000 | — |
| Swap | `SwapMakerNotComplete`, `SwapVerificationTimeout`, `SwapFraud` | 3000 | — |
| 会员 | `MemberBenefitNotProvided`, `MemberServiceQuality` | 3000 | — |
| 信用 | `CreditScoreDispute`, `CreditPenaltyAppeal` | 3000 | — |
| 其他 | `Other` | 3000 | — |

每种类型绑定：`domain()` 所属域、`penalty_rate()` 惩罚比例（bps）、`triggers_permanent_ban()` 是否永久封禁。

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
    pub complainant_wins: u64,
    pub respondent_wins: u64,
    pub settlements: u64,
    pub expired_count: u64,
}
```

## 事件

### 仲裁事件

| 事件 | 字段 | 说明 |
|------|------|------|
| `Disputed` | domain, id | 争议已登记 |
| `Arbitrated` | domain, id, decision, bps | 裁决完成 |
| `DisputeWithDepositInitiated` | domain, id, initiator, respondent, deposit, deadline | 双向押金仲裁发起 |
| `RespondentDepositLocked` | domain, id, respondent, deposit | 应诉方押金锁定 |
| `DepositProcessed` | domain, id, account, released, slashed | 押金处理（罚没/释放） |

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

## 错误

| 错误 | 说明 |
|------|------|
| `AlreadyDisputed` | 重复争议登记 / 证据列表已满 |
| `NotDisputed` | 案件未登记 / 非当事方 |
| `InsufficientDeposit` | 押金不足或锁定失败 |
| `AlreadyResponded` | 重复应诉 |
| `ResponseDeadlinePassed` | 应诉期已过 |
| `CounterpartyNotFound` | 无法获取对方账户或订单金额 |
| `ComplaintNotFound` | 投诉不存在 |
| `NotAuthorized` | 无权操作 |
| `InvalidComplaintType` | 投诉类型与域不匹配 |
| `InvalidState` | 无效的状态转换 |
| `TooManyComplaints` | 该对象投诉过多 |
| `TooManyActiveComplaints` | 用户活跃投诉超限（上限 50） |
| `EvidenceNotFound` | 引用的 evidence_id 在 pallet-evidence 中不存在 |

## Trait 接口

### ArbitrationRouter（域路由）

```rust
pub trait ArbitrationRouter<AccountId, Balance> {
    fn can_dispute(domain: [u8; 8], who: &AccountId, id: u64) -> bool;
    fn apply_decision(domain: [u8; 8], id: u64, decision: Decision) -> DispatchResult;
    fn get_counterparty(domain: [u8; 8], initiator: &AccountId, id: u64) -> Result<AccountId, DispatchError>;
    fn get_order_amount(domain: [u8; 8], id: u64) -> Result<Balance, DispatchError>;
    fn get_maker_id(domain: [u8; 8], id: u64) -> Option<u64> { None }
}
```

### CreditUpdater（信用分更新）

```rust
pub trait CreditUpdater {
    fn record_maker_dispute_result(maker_id: u64, order_id: u64, maker_win: bool) -> DispatchResult;
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

## Hooks

### on_idle（剩余权重利用）

每个区块空闲时按优先级依次处理，单项权重 25M ref_time：

| 阶段 | 操作 | 每次上限 |
|:---:|------|:---:|
| 1 | `expire_old_complaints` — 扫描过期投诉，退还押金 | 5 |
| 2 | `archive_old_complaints` — 归档已解决投诉（延迟 `ComplaintArchiveDelayBlocks`） | 10 |
| 3 | `cleanup_old_archived_disputes` — TTL 清理归档仲裁 | 5 |
| 4 | `cleanup_old_archived_complaints` — TTL 清理归档投诉 | 5 |

所有扫描均使用游标推进（O(batch)），避免全表遍历。

## 配置参数

### 基础配置

| 参数 | 说明 |
|------|------|
| `MaxEvidence` | 每案最大证据引用数 |
| `MaxCidLen` | CID 最大长度 |
| `Escrow` | 托管接口（`Escrow<AccountId, Balance>`） |
| `Router` | 域路由（runtime 注入 `ArbitrationRouter`） |
| `DecisionOrigin` | 裁决 Origin（治理/委员会） |
| `WeightInfo` | 权重信息（7 个独立函数） |

### 押金配置

| 参数 | 说明 |
|------|------|
| `Fungible` | Fungible 接口（`Inspect` + `Mutate` + `MutateHold`） |
| `RuntimeHoldReason` | 押金锁定原因标识（`From<HoldReason>`） |
| `DepositRatioBps` | 仲裁押金比例（bps，1500 = 15%） |
| `ResponseDeadline` | 应诉期限（区块数） |
| `RejectedSlashBps` | 败诉罚没比例（bps，3000 = 30%） |
| `PartialSlashBps` | 部分胜诉罚没比例（bps，5000 = 50%） |
| `TreasuryAccount` | 国库账户（接收罚没） |

### 投诉配置

| 参数 | 说明 |
|------|------|
| `ComplaintDeposit` | 投诉押金兜底金额（COS 数量） |
| `ComplaintDepositUsd` | 投诉押金 USD 价值（精度 10^6，1_000_000 = 1 USDT） |
| `Pricing` | 定价接口（`PricingProvider`，换算押金） |
| `ComplaintSlashBps` | 投诉败诉罚没比例（bps） |

### 集成配置

| 参数 | 说明 |
|------|------|
| `CidLockManager` | CID 锁定管理器（仲裁期间锁定证据） |
| `CreditUpdater` | 信用分更新器（裁决结果反馈信用系统） |
| `EvidenceExists` | 证据存在性检查器（校验 evidence_id） |

### 防膨胀配置

| 参数 | 说明 |
|------|------|
| `ArchiveTtlBlocks` | 归档记录 TTL（区块数，默认 2_592_000 ≈ 180 天，0 禁用清理） |
| `ComplaintArchiveDelayBlocks` | 投诉归档延迟（区块数，默认 432_000 ≈ 30 天） |

## 集成示例

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

## 相关模块

- [escrow/](../escrow/) — 资金托管（裁决分账接口）
- [evidence/](../evidence/) — 证据管理（evidence_id 引用）
- [trading/common/](../../trading/common/) — PricingProvider（投诉押金换算）
- [storage/service/](../../storage/service/) — CidLockManager（证据 CID 锁定）
