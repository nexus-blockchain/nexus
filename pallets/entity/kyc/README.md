# pallet-entity-kyc

> Per-Entity KYC/AML 认证模块 — 用户在每个 Entity 内独立认证

- **Pallet Index**: 131
- **Storage Version**: 1
- **版本**: v1.0.0

## 1. 概述

`pallet-entity-kyc` 实现普通用户在 Entity（组织实体）内的 KYC（Know Your Customer）认证。**同一账户在不同 Entity 拥有完全独立的 KYC 记录、级别和状态**，Entity 之间互不影响。

### 1.1 核心设计理念

每个 Entity 是独立运营的组织实体，独立管理自己的用户认证体系：

- 用户向**特定 Entity** 提交 KYC 申请
- Entity 授权的 **Provider** 或 **Entity Owner/Admin** 审核
- 每个 **(Entity, Account)** 对拥有独立的 KYC 记录
- Entity A 的撤销/过期不影响 Entity B 中同一用户的认证状态
- **认证对象是普通用户**（在 Entity 上下文中），而非认证 Entity 本身

### 1.2 核心能力

| 能力 | 说明 |
|------|------|
| Per-Entity 认证 | 同一用户在不同 Entity 可持有不同级别的 KYC |
| 5 级认证体系 | None / Basic / Standard / Enhanced / Institutional（`Ord` 可比较） |
| 4 种 Provider 类型 | Internal / ThirdParty / Government / Financial（全局注册，per-entity 授权） |
| 双层审核权限 | 全局 Provider（需 Entity 授权 + max_level 约束）或 Entity Owner/Admin（无级别限制） |
| 风险评分 | 0-100 评分系统，per-entity 独立评分 |
| 高风险国家 | 全局列表，最多 50 个 ISO 3166-1 alpha-2 代码 |
| 认证有效期 | 按级别独立配置，链上主动过期标记 |
| 实体 KYC 要求 | 可配置最低级别、强制性、宽限期、风险阈值、高风险国家策略 |
| 审核 SLA | 待审核超时自动拒绝（`PendingKycTimeout`） |
| GDPR 合规 | 数据清除权（`purge_kyc_data`），链上仅存元数据 |
| 操作历史 | per-entity 环形缓冲区记录所有 KYC 操作 |
| 跨模块接口 | 实现 `pallet_entity_common::KycProvider` trait，`entity_id` 参数全面生效 |

## 2. 架构

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                        pallet-entity-kyc (v1.0)                              │
│                       Per-Entity KYC 认证模型                                │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  用户 ─→ 选择 Entity ─→ submit_kyc ─→ Provider/Owner/Admin 审核              │
│                                                                              │
│  Storage 核心:                                                               │
│    KycRecords:    DoubleMap<EntityId, AccountId → KycRecord>                  │
│    KycHistory:    DoubleMap<EntityId, AccountId → BoundedVec<HistoryEntry>>   │
│    Providers:     Map<AccountId → KycProvider>         (全局注册)             │
│    EntityAuthorizedProviders: DoubleMap<EntityId, AccountId → ()>  (per-entity)│
│                                                                              │
└──────┬────────────────────────────────┬──────────────────────────────────────┘
       │                                │
  依赖 EntityProvider               实现 KycProvider trait
       │                            (entity_id 参数全面使用)
       ▼                                │
  pallet-entity-registry         ┌──────┴──────────────────────┐
  (所有权/权限/锁定)              │ 消费方:                     │
                                  │  ├─ pallet-entity-member    │
                                  │  ├─ pallet-entity-token     │
                                  │  ├─ pallet-entity-tokensale │
                                  │  ├─ pallet-entity-market    │
                                  │  └─ pallet-commission-core  │
                                  └─────────────────────────────┘
```

### 2.1 Provider 授权模型

```
              全局注册 (AdminOrigin)          per-Entity 授权 (Owner/Admin)
                     │                               │
    ┌────────────────┴────────────────┐              │
    │  Providers: Map<AccountId>     │              │
    │  (全局 Provider 注册表)         │              │
    └────────────────┬────────────────┘              │
                     │                               │
                     ▼                               ▼
    ┌─────────────────────────────────────────────────────────┐
    │  EntityAuthorizedProviders: DoubleMap<EntityId, AccId>   │
    │  Entity Owner 授权哪些全局 Provider 可以审核自己的用户     │
    └────────────────────────┬────────────────────────────────┘
                             │
                             ▼ approve / reject / renew / update_risk_score
    ┌─────────────────────────────────────────────────────────┐
    │  路径 A: 全局 Provider + Entity 授权 → 受 max_level 约束│
    │  路径 B: Entity Owner/Admin (KYC_MANAGE) → 无级别限制   │
    └─────────────────────────────────────────────────────────┘
```

**审核权限判定逻辑** (`ensure_can_review`):

1. 检查调用者是否为已注册 Provider → 未暂停 + 已被该 Entity 授权 → 返回 `Ok(Some(max_level))`
2. 检查调用者是否为 Entity Owner 或 `KYC_MANAGE` Admin → 返回 `Ok(None)`（无级别限制）
3. 两者都不满足 → 返回 `NotEntityOwnerOrAdmin` 错误

### 2.2 依赖 Trait

| Trait | 来源 | 用途 |
|-------|------|------|
| `EntityProvider<AccountId>` | pallet-entity-common | 实体存在性、所有权、管理员权限（KYC_MANAGE）、锁定状态查询 |
| `OnKycStatusChange<AccountId>` | pallet-entity-common | KYC 状态变更下游通知（含 entity_id） |

### 2.3 对外提供 Trait

| Trait | 消费方 | 方法签名 |
|-------|--------|----------|
| `KycProvider<AccountId>` | entity-member / entity-token / entity-tokensale / entity-market / commission-core | `kyc_level(entity_id, account) → u8` |
| | | `is_kyc_approved(entity_id, account) → bool` |
| | | `is_kyc_expired(entity_id, account) → bool` |
| | | `can_participate(entity_id, account) → bool` |
| | | `meets_kyc_requirement(entity_id, account, level) → bool` |
| | | `kyc_expires_at(entity_id, account) → u64` |

### 2.4 Runtime 桥接

| 桥接 Struct | 消费 Pallet | Trait | 方法 |
|-------------|-------------|-------|------|
| `MemberKycBridge` | pallet-entity-member | `KycChecker` | `is_kyc_passed(entity_id, account)` |
| `KycParticipationGuard` | pallet-commission-core / pool-reward | `ParticipationGuard` | `can_participate(entity_id, account)` |
| `TokenSaleKycBridge` | pallet-entity-tokensale | `KycChecker` | `kyc_level(entity_id, account)` |
| `TokenKycBridge` | pallet-entity-token | `KycLevelProvider` | `get_kyc_level(entity_id, account)` / `meets_kyc_requirement(entity_id, account, min_level)` |
| `EntityKyc` | pallet-entity-market | `KycProvider` (直接) | `kyc_level(entity_id, account)` |

## 3. KYC 级别

| 级别 | 编码 | 典型要求 | 有效期配置 |
|------|------|----------|------------|
| None | 0 | 未认证 | — |
| Basic | 1 | 邮箱/手机验证 | `BasicKycValidity` |
| Standard | 2 | 身份证件核验 | `StandardKycValidity` |
| Enhanced | 3 | 地址证明 + 资金来源 | `EnhancedKycValidity` |
| Institutional | 4 | 企业文件 + 受益人披露 | `InstitutionalKycValidity` |

级别实现 `Ord`，可直接用 `>=` 比较。`as_u8()` 转为 `0-4` 整数值，`try_from_u8()` 反向转换。

## 4. KYC 状态

| 状态 | 编码 | 说明 |
|------|------|------|
| NotSubmitted | 0 | 未提交（初始态，或 cancel 后删除记录） |
| Pending | 1 | 待审核 |
| Approved | 2 | 已批准（有有效期） |
| Rejected | 3 | 已拒绝 |
| Expired | 4 | 已过期（需主动标记） |
| Revoked | 5 | 已撤销（管理员操作） |

### 4.1 状态机

```
                ┌──────────────┐
                │ NotSubmitted │ ◄─── cancel_kyc (删除记录)
                └──────┬───────┘
                       │ submit_kyc(entity_id, level, data_cid, country_code)
                       ▼
                ┌──────────────┐ ──── timeout_pending_kyc ────┐
                │   Pending    │                               │
                └──┬───┬───┬───┘                               │
       approve_kyc │   │   │ reject_kyc                        │
                   │   │   ▼                                   ▼
                   │   │  ┌──────────┐                  ┌──────────┐
                   │   │  │ Rejected │ ◄────────────────│(Rejected)│
                   │   │  └──────────┘                  └──────────┘
                   │   │ revoke_kyc
                   ▼   ▼
              ┌──────────┐     expire_kyc     ┌─────────┐
              │ Approved │ ──────────────────► │ Expired │
              └────┬─────┘                    └────┬────┘
                   │                               │
                   │ revoke_kyc          revoke_kyc │
                   ▼                               ▼
              ┌─────────┐                    ┌─────────┐
              │ Revoked │                    │ Revoked │
              └─────────┘                    └─────────┘

  续期 (renew_kyc): Approved|Expired → Approved
  升级提交 (submit_kyc level>current): Approved → Pending
  强制批准 (force_approve_kyc): 任意状态 → Approved
```

## 5. 数据结构

### 5.1 KycRecord — 用户在特定 Entity 的认证记录

```rust
pub struct KycRecord<AccountId, BlockNumber, MaxCidLen: Get<u32>> {
    pub account: AccountId,
    pub level: KycLevel,
    pub status: KycStatus,
    pub provider: Option<AccountId>,          // 审核该记录的 Provider
    pub data_cid: Option<BoundedVec<u8, MaxCidLen>>,  // IPFS CID (链下数据)
    pub submitted_at: Option<BlockNumber>,
    pub verified_at: Option<BlockNumber>,
    pub expires_at: Option<BlockNumber>,
    pub rejection_reason: Option<RejectionReason>,
    pub rejection_details_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub country_code: Option<[u8; 2]>,        // ISO 3166-1 alpha-2
    pub risk_score: u8,                       // 0-100
}
```

### 5.2 KycProvider — 认证提供者（全局注册）

```rust
pub struct KycProvider<AccountId, MaxNameLen: Get<u32>> {
    pub account: AccountId,
    pub name: BoundedVec<u8, MaxNameLen>,
    pub provider_type: ProviderType,     // Internal | ThirdParty | Government | Financial
    pub max_level: KycLevel,             // 可审核的最高级别
    pub verifications_count: u64,        // 累计审核次数
    pub suspended: bool,                 // 是否被暂停
}
```

### 5.3 EntityKycRequirement — 实体 KYC 准入要求

```rust
pub struct EntityKycRequirement {
    pub min_level: KycLevel,             // 最低 KYC 级别
    pub mandatory: bool,                 // 是否强制 (false 时所有人可参与)
    pub grace_period: u32,               // 过期后宽限区块数
    pub allow_high_risk_countries: bool,  // 是否允许高风险国家用户
    pub max_risk_score: u8,              // 最大允许风险评分
}
```

### 5.4 KycHistoryEntry — 操作历史条目

```rust
pub struct KycHistoryEntry<BlockNumber> {
    pub action: KycAction,    // Submitted|Approved|Rejected|Revoked|Expired|Renewed|Cancelled|DataUpdated|DataPurged|ForceApproved|TimedOut
    pub level: KycLevel,
    pub block_number: BlockNumber,
}
```

### 5.5 RejectionReason — 拒绝原因枚举

```rust
pub enum RejectionReason {
    UnclearDocument,       // 文档不清晰
    ExpiredDocument,       // 文档已过期
    InformationMismatch,   // 信息不匹配
    SuspiciousActivity,    // 可疑活动
    SanctionedEntity,      // 被制裁实体
    HighRiskCountry,       // 高风险国家
    ForgedDocument,        // 伪造文档
    Other,                 // 其他
}
```

## 6. Storage（9 项）

| 存储项 | 类型 | 作用域 | 说明 |
|--------|------|--------|------|
| `KycRecords` | `DoubleMap<u64, AccountId → KycRecord>` | per-entity | 用户在特定 Entity 的 KYC 记录 |
| `Providers` | `Map<AccountId → KycProvider>` | 全局 | 认证提供者注册表 |
| `ProviderCount` | `Value<u32>` | 全局 | 活跃提供者数量 |
| `EntityAuthorizedProviders` | `DoubleMap<u64, AccountId → ()>` | per-entity | Entity 授权的 Provider 列表 |
| `EntityRequirements` | `Map<u64 → EntityKycRequirement>` | per-entity | 实体 KYC 准入要求配置 |
| `HighRiskCountries` | `Value<BoundedVec<[u8;2], 50>>` | 全局 | 高风险国家列表 |
| `KycHistory` | `DoubleMap<u64, AccountId → BoundedVec<HistoryEntry>>` | per-entity | 操作历史（环形缓冲区） |
| `PendingKycCount` | `Map<u64 → u32>` | per-entity | 待审核 KYC 数量 |
| `ApprovedKycCount` | `Map<u64 → u32>` | per-entity | 已批准 KYC 数量 |

## 7. Config 配置

| 配置项 | 类型 | 说明 |
|--------|------|------|
| `MaxCidLength` | `u32` | IPFS CID 最大长度 |
| `MaxProviderNameLength` | `u32` | Provider 名称最大长度 |
| `MaxProviders` | `u32` | 全局最大 Provider 数量 |
| `BasicKycValidity` | `BlockNumber` | Basic 级别有效期（区块数） |
| `StandardKycValidity` | `BlockNumber` | Standard 级别有效期 |
| `EnhancedKycValidity` | `BlockNumber` | Enhanced 级别有效期 |
| `InstitutionalKycValidity` | `BlockNumber` | Institutional 级别有效期 |
| `MaxHistoryEntries` | `u32` | 每用户每 Entity 最大历史条目数 |
| `PendingKycTimeout` | `BlockNumber` | 待审核超时区块数 |
| `AdminOrigin` | `EnsureOrigin` | 管理员权限来源（Root/Council） |
| `EntityProvider` | `EntityProvider<AccountId>` | 实体信息查询接口 |
| `OnKycStatusChange` | `OnKycStatusChange<AccountId>` | 状态变更通知回调 |

## 8. Extrinsics（25 个）

### 8.1 用户操作

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 0 | `submit_kyc(entity_id, level, data_cid, country_code)` | Signed | 向指定 Entity 提交 KYC 申请 |
| 9 | `cancel_kyc(entity_id)` | Signed (本人) | 取消自己在指定 Entity 的 Pending KYC（删除记录） |
| 17 | `update_kyc_data(entity_id, new_data_cid)` | Signed (本人) | 更新 Pending 状态的 KYC 数据（补充/替换材料） |
| 18 | `purge_kyc_data(entity_id)` | Signed (本人) | GDPR 数据删除权（仅 Rejected/Revoked/Expired 状态） |

### 8.2 Provider / Entity Owner/Admin 审核操作

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 1 | `approve_kyc(entity_id, account, risk_score)` | `ensure_can_review` | 批准 KYC（Provider 受 max_level 约束） |
| 2 | `reject_kyc(entity_id, account, reason, details_cid?)` | `ensure_can_review` | 拒绝 KYC，附拒绝原因和可选详情 CID |
| 11 | `update_risk_score(entity_id, account, new_score)` | `ensure_can_review` | 更新已批准用户的风险评分（0-100） |
| 16 | `renew_kyc(entity_id, account)` | `ensure_can_review` | 续期 KYC（Approved/Expired → Approved） |

### 8.3 Entity Owner/Admin 管理操作

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 6 | `set_entity_requirement(entity_id, min_level, mandatory, grace_period, allow_high_risk, max_risk_score)` | Owner/Admin | 设置实体 KYC 准入要求 |
| 19 | `remove_entity_requirement(entity_id)` | Owner/Admin | 移除实体 KYC 准入要求 |
| 23 | `authorize_provider(entity_id, provider)` | Owner/Admin | 授权全局 Provider 为该 Entity 审核 KYC |
| 24 | `deauthorize_provider(entity_id, provider)` | Owner/Admin | 撤销 Provider 对该 Entity 的审核授权 |

### 8.4 公共操作（任何人可调用）

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 8 | `expire_kyc(entity_id, account)` | Signed (任何人) | 标记已过期的 KYC（`now > expires_at`） |
| 20 | `timeout_pending_kyc(entity_id, account)` | Signed (任何人) | 超时待审核 KYC → Rejected（`now > submitted_at + timeout`） |

### 8.5 AdminOrigin 管理操作

| Index | 函数 | 说明 |
|-------|------|------|
| 3 | `revoke_kyc(entity_id, account, reason)` | 撤销 KYC（Pending/Approved/Expired → Revoked） |
| 4 | `register_provider(account, name, type, max_level)` | 注册全局认证提供者 |
| 5 | `remove_provider(account)` | 移除认证提供者 |
| 7 | `update_high_risk_countries(countries)` | 更新高风险国家列表（去重排序） |
| 10 | `force_set_entity_requirement(entity_id, ...)` | 强制设置实体 KYC 要求（跳过 Entity 存在性检查） |
| 12 | `update_provider(provider, name?, max_level?)` | 更新提供者信息 |
| 13 | `suspend_provider(provider)` | 暂停认证提供者 |
| 14 | `resume_provider(provider)` | 恢复认证提供者 |
| 15 | `force_approve_kyc(entity_id, account, level, risk_score, country_code)` | 强制批准 KYC（数据迁移/特殊豁免） |
| 21 | `batch_revoke_by_provider(entity_id, provider, accounts, reason)` | 批量撤销指定 Provider 在指定 Entity 中批准的 KYC（最多 100 个） |
| 22 | `force_remove_provider(provider)` | 强制移除提供者 |

## 9. Events（24 个）

| 事件 | 字段 | 触发时机 |
|------|------|----------|
| `KycSubmitted` | entity_id, account, level | submit_kyc |
| `KycApproved` | entity_id, account, level, provider, expires_at | approve_kyc |
| `KycRejected` | entity_id, account, level, reason | reject_kyc |
| `KycExpired` | entity_id, account | expire_kyc |
| `KycRevoked` | entity_id, account, reason | revoke_kyc |
| `KycCancelled` | entity_id, account | cancel_kyc |
| `KycRenewed` | entity_id, account, level, expires_at | renew_kyc |
| `KycForceApproved` | entity_id, account, level, expires_at | force_approve_kyc |
| `KycDataUpdated` | entity_id, account | update_kyc_data |
| `KycDataPurged` | entity_id, account | purge_kyc_data |
| `RiskScoreUpdated` | entity_id, account, old_score, new_score | update_risk_score |
| `PendingKycTimedOut` | entity_id, account | timeout_pending_kyc |
| `ProviderRegistered` | provider, name, provider_type | register_provider |
| `ProviderRemoved` | provider | remove_provider |
| `ProviderUpdated` | provider | update_provider |
| `ProviderSuspended` | provider | suspend_provider |
| `ProviderResumed` | provider | resume_provider |
| `ProviderForceRemoved` | provider | force_remove_provider |
| `ProviderAuthorized` | entity_id, provider | authorize_provider |
| `ProviderDeauthorized` | entity_id, provider | deauthorize_provider |
| `ProviderKycsRevoked` | entity_id, provider, count, reason | batch_revoke_by_provider |
| `EntityRequirementSet` | entity_id, min_level | set_entity_requirement / force_set |
| `EntityRequirementRemoved` | entity_id | remove_entity_requirement |
| `HighRiskCountriesUpdated` | count | update_high_risk_countries |

## 10. Errors（32 个）

| 错误 | 说明 |
|------|------|
| `KycNotFound` | 指定 Entity 下无该用户的 KYC 记录 |
| `KycAlreadyPending` | 该 Entity 下已有待审核的 KYC |
| `KycAlreadyApproved` | 该 Entity 下已有相同或更高级别的有效 KYC |
| `ProviderNotFound` | Provider 未注册 |
| `ProviderAlreadyExists` | Provider 已注册 |
| `CidTooLong` | IPFS CID 超过 MaxCidLength |
| `NameTooLong` | Provider 名称超过 MaxProviderNameLength |
| `MaxProvidersReached` | 全局 Provider 数量已达上限 |
| `InvalidKycStatus` | 当前 KYC 状态不允许该操作 |
| `InvalidKycLevel` | KYC 级别无效（如提交 None） |
| `ProviderLevelNotSupported` | 用户申请的级别超过 Provider 的 max_level |
| `TooManyCountries` | 高风险国家列表超过 50 个 |
| `InvalidRiskScore` | 风险评分超过 100 |
| `EmptyProviderName` | Provider 名称为空 |
| `EmptyDataCid` | 数据 CID 为空 |
| `InvalidCountryCode` | 国家代码格式不合法（需两个大写字母） |
| `SelfApprovalNotAllowed` | 不允许自我审批 |
| `KycNotExpired` | KYC 尚未过期（`now <= expires_at`） |
| `NotEntityOwnerOrAdmin` | 调用者非 Entity Owner 且无 KYC_MANAGE 权限 |
| `EntityNotFound` | Entity 不存在 |
| `ProviderIsSuspended` | Provider 已被暂停 |
| `ProviderNotSuspended` | Provider 未被暂停（resume 时检查） |
| `NothingToUpdate` | 无更新内容 |
| `EntityLocked` | Entity 已锁定 |
| `KycNotRenewable` | KYC 状态不可续期（仅 Approved/Expired 可续） |
| `KycDataCannotBePurged` | KYC 数据不可清除（仅 Rejected/Revoked/Expired 可清除） |
| `RequirementNotFound` | Entity 未配置 KYC 要求 |
| `PendingNotTimedOut` | Pending KYC 尚未超时 |
| `ProviderMismatch` | 批量撤销时 Provider 不匹配 |
| `EmptyAccountList` | 批量撤销账户列表为空 |
| `ProviderNotAuthorized` | Provider 未被该 Entity 授权 |
| `ProviderAlreadyAuthorized` | Provider 已被该 Entity 授权 |

## 11. 公开辅助函数

```rust
impl<T: Config> Pallet<T> {
    // ── 有效期 ──
    pub fn get_validity_period(level: KycLevel) -> BlockNumber;

    // ── 查询（全部 per-entity）──
    pub fn get_kyc_level(entity_id: u64, account: &AccountId) -> KycLevel;
    pub fn get_risk_score(entity_id: u64, account: &AccountId) -> u8;
    pub fn get_kyc_stats(entity_id: u64) -> (u32, u32);   // (pending, approved)
    pub fn get_kyc_history(entity_id: u64, account: &AccountId) -> Vec<KycHistoryEntry>;

    // ── 检查（全部 per-entity）──
    pub fn meets_kyc_requirement(entity_id: u64, account: &AccountId, min_level: KycLevel) -> bool;
    pub fn is_high_risk_country(entity_id: u64, account: &AccountId) -> bool;
    pub fn can_participate_in_entity(account: &AccountId, entity_id: u64) -> bool;
    pub fn check_account_compliance(entity_id: u64, account: &AccountId, req: &EntityKycRequirement) -> bool;
}
```

**合规检查逻辑** (`can_participate_in_entity`):

- Entity 未配置 `EntityRequirements` → `true`（所有人可参与）
- `mandatory == false` → `true`
- `mandatory == true` → 检查:
  - KYC 状态必须为 Approved 或 Expired
  - `level >= min_level`
  - 高风险国家策略
  - `risk_score <= max_risk_score`
  - 过期时间 + 宽限期

## 12. 版本历史

| 版本 | StorageVersion | 变更 |
|------|---------------|------|
| v0.1.0 ~ v0.7.0 | 0 | 全局单记录模型（已废弃） |
| v1.0.0 | 1 | **Per-Entity KYC 重设计**: StorageDoubleMap、所有 extrinsic 增加 entity_id、Provider 授权模型（authorize/deauthorize）、Entity Owner/Admin 可直接审核、KycProvider trait entity_id 参数全面生效、OnKycStatusChange 增加 entity_id、KycLevelProvider (pallet-entity-token) 增加 entity_id |
