# pallet-arbitration

> 路径：`pallets/dispute/arbitration/` · Runtime Index: 62

仲裁争议处理系统，提供争议登记、证据管理、仲裁裁决、双向押金、统一投诉等功能，支持 8 个业务域。

## 设计理念

- **域路由架构**：8 字节域标识，多业务统一仲裁
- **双向押金**：发起方/应诉方各从托管锁定订单金额的 15%（`Fungible::hold`）
- **两大子系统**：仲裁系统（资金争议）+ 投诉系统（行为投诉）
- **CID 锁定**：仲裁期间自动锁定相关证据 CID，防止删除
- **信用集成**：裁决结果通过 `CreditUpdater` 反馈到做市商信用系统

## Extrinsics

### 仲裁相关
| call_index | 方法 | 说明 |
|:---:|------|------|
| 0 | `dispute` | 发起仲裁（旧版，CID 存链） |
| 1 | `arbitrate` | 治理裁决（DecisionOrigin）→ 分账 + 押金 + CID 解锁 + 信用分 + 归档 |
| 2 | `dispute_with_evidence_id` | 带 evidence_id 引用发起仲裁 |
| 3 | `append_evidence_id` | 追加证据引用（需权限校验） |
| 4 | `dispute_with_two_way_deposit` | 双向押金仲裁（推荐）— 锁发起方 15% 押金 |
| 5 | `respond_to_dispute` | 应诉 — 锁应诉方押金 + 提交反驳证据 |

### 投诉相关
| call_index | 方法 | 说明 |
|:---:|------|------|
| 10 | `file_complaint` | 发起投诉（锁 ~1 USDT 押金，Pricing 换算） |
| 11 | `respond_to_complaint` | 被投诉方响应/申诉 |
| 12 | `withdraw_complaint` | 撤诉（退还押金） |
| 13 | `settle_complaint` | 和解（退还押金） |
| 14 | `escalate_to_arbitration` | 升级到仲裁委员会 |
| 15 | `resolve_complaint` | 治理裁决投诉（DecisionOrigin）— 胜诉退押金/败诉罚没 |

## 存储

### 仲裁存储
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Disputed` | `DoubleMap<[u8;8], u64, ()>` | 争议登记 |
| `EvidenceIds` | `DoubleMap<[u8;8], u64, BoundedVec<u64>>` | 证据引用列表 |
| `LockedCidHashes` | `DoubleMap<[u8;8], u64, BoundedVec<Hash>>` | 锁定的 CID 哈希 |
| `TwoWayDeposits` | `DoubleMap<[u8;8], u64, TwoWayDepositRecord>` | 双向押金记录 |

### 归档存储
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextArchivedId` | `u64` | 归档 ID 计数器 |
| `ArchivedDisputes` | `Map<u64, ArchivedDispute>` | 归档仲裁记录 |
| `ArbitrationStats` | `ArbitrationPermanentStats` | 永久统计 |

### 投诉存储
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextComplaintId` | `u64` | 投诉 ID 计数器 |
| `Complaints` | `Map<u64, Complaint>` | 活跃投诉 |
| `ArchivedComplaints` | `Map<u64, ArchivedComplaint>` | 归档投诉 |
| `UserActiveComplaints` | `Map<AccountId, BoundedVec<u64, 50>>` | 用户活跃投诉索引 |
| `ComplaintDeposits` | `Map<u64, Balance>` | 投诉押金记录 |
| `DomainStats` | `Map<[u8;8], DomainStatistics>` | 域统计 |
| `ComplaintArchiveCursor` | `u64` | 归档游标 |
| `ComplaintExpiryCursor` | `u64` | 过期扫描游标（O(batch)） |

## 主要类型

### Decision（裁决类型）
```rust
pub enum Decision {
    Release,      // 全额释放（卖家胜）
    Refund,       // 全额退款（买家胜）
    Partial(u16), // 按比例分配，bps=0-10000
}
```

### HoldReason（押金锁定原因）
```rust
pub enum HoldReason {
    DisputeInitiator,   // 纠纷发起方押金
    DisputeRespondent,  // 应诉方押金
    ComplaintDeposit,   // 投诉押金
}
```

### ComplaintStatus（投诉状态）
```rust
pub enum ComplaintStatus {
    Submitted,              // 已提交
    Responded,              // 已响应
    Mediating,              // 调解中
    Arbitrating,            // 仲裁中
    ResolvedComplainantWin, // 投诉方胜
    ResolvedRespondentWin,  // 被投诉方胜
    ResolvedSettlement,     // 和解
    Withdrawn,              // 已撤销
    Expired,                // 已过期
}
```

### ComplaintType（25 种，覆盖 8 业务域）

| 业务域 | 投诉类型 |
|-------|---------|
| OTC | OtcSellerNotDeliver, OtcBuyerFalseClaim, OtcTradeFraud, OtcPriceDispute |
| 直播 | LiveIllegalContent, LiveFalseAdvertising, LiveHarassment, LiveFraud, LiveGiftRefund, LiveOther |
| 做市商 | MakerCreditDefault, MakerMaliciousOperation, MakerFalseQuote |
| NFT | NftSellerNotDeliver, NftCounterfeit, NftTradeFraud, NftAuctionDispute |
| Swap | SwapMakerNotComplete, SwapVerificationTimeout, SwapFraud |
| 会员 | MemberBenefitNotProvided, MemberServiceQuality |
| 信用 | CreditScoreDispute, CreditPenaltyAppeal |
| 其他 | Other |

每种类型绑定：`domain()` 所属域、`penalty_rate()` 惩罚比例、`triggers_permanent_ban()` 是否永久封禁。

## 错误

| 错误 | 说明 |
|------|------|
| `AlreadyDisputed` | 重复争议登记 |
| `NotDisputed` | 案件未登记 |
| `InsufficientDeposit` | 押金不足 |
| `AlreadyResponded` | 已经应诉 |
| `ResponseDeadlinePassed` | 应诉期已过 |
| `CounterpartyNotFound` | 无法获取对方账户 |
| `ComplaintNotFound` | 投诉不存在 |
| `NotAuthorized` | 无权操作 |
| `InvalidComplaintType` | 投诉类型与域不匹配 |
| `InvalidState` | 无效的状态转换 |
| `TooManyComplaints` | 该对象投诉过多 |
| `TooManyActiveComplaints` | 用户活跃投诉超限 |

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

## 配置参数

| 参数 | 说明 |
|------|------|
| `MaxEvidence` | 每案最大证据引用数 |
| `MaxCidLen` | CID 最大长度 |
| `Escrow` | 托管接口 |
| `Router` | 域路由（runtime 注入） |
| `DecisionOrigin` | 仲裁决策 Origin（治理/委员会） |
| `Fungible` | Fungible 接口（`Inspect` + `Mutate` + `MutateHold`） |
| `RuntimeHoldReason` | 押金锁定原因标识 |
| `DepositRatioBps` | 押金比例（bps, 1500=15%） |
| `ResponseDeadline` | 应诉期限（区块数） |
| `RejectedSlashBps` | 败诉罚没比例（bps, 3000=30%） |
| `PartialSlashBps` | 部分胜诉罚没（bps, 5000=50%） |
| `ComplaintDeposit` | 投诉押金兜底金额 |
| `ComplaintDepositUsd` | 投诉押金 USD 价值（精度 10^6） |
| `Pricing` | 定价接口（`PricingProvider`，换算押金） |
| `ComplaintSlashBps` | 投诉败诉罚没比例 |
| `TreasuryAccount` | 国库账户 |
| `CidLockManager` | CID 锁定管理器 |
| `CreditUpdater` | 信用分更新器 |
| `WeightInfo` | 权重信息（7 个独立函数） |

## 集成示例

```rust
// 业务 pallet 实现 ArbitrationRouter
impl<T: Config> ArbitrationRouter<T::AccountId, BalanceOf<T>> for Pallet<T> {
    fn can_dispute(domain: [u8; 8], who: &T::AccountId, id: u64) -> bool {
        if domain != *b"otc_ord_" { return false; }
        Self::is_order_participant(id, who)
    }
    
    fn apply_decision(domain: [u8; 8], id: u64, decision: Decision) -> DispatchResult {
        // 按裁决执行放款/退款
        Ok(())
    }
    
    fn get_counterparty(domain: [u8; 8], initiator: &T::AccountId, id: u64)
        -> Result<T::AccountId, DispatchError> {
        // 返回对方账户
        Ok(Self::get_other_party(id, initiator))
    }
    
    fn get_order_amount(domain: [u8; 8], id: u64) -> Result<BalanceOf<T>, DispatchError> {
        Orders::<T>::get(id).map(|o| o.amount).ok_or(Error::<T>::NotFound.into())
    }
}
```

## 相关模块

- [escrow/](../escrow/) — 资金托管（裁决接口）
- [evidence/](../evidence/) — 证据管理（证据引用）
- [trading/common/](../../trading/common/) — PricingProvider（投诉押金换算）
