# pallet-entity-kyc

> 🔐 Entity KYC/AML 认证模块 — 多级别认证与合规性检查 (Phase 7)

## 概述

`pallet-entity-kyc` 实现用户和实体的 KYC（了解你的客户）和 AML（反洗钱）认证功能，支持多级别认证、多认证提供者、风险评分和高风险国家管理。

### 核心功能

- **5 级认证** — None / Basic / Standard / Enhanced / Institutional
- **4 种提供者类型** — Internal / ThirdParty / Government / Financial
- **风险评分** — 0-100 风险评分系统（未认证用户默认 100）
- **高风险国家** — 可配置最多 50 个 ISO 3166-1 alpha-2 国家代码
- **认证有效期** — 按级别配置，自动过期检查
- **实体 KYC 要求** — 可配置最低级别、强制性、宽限期、风险阈值

## KYC 级别

| 级别 | 要求 | 配置常量 | 可比较 |
|------|------|----------|--------|
| None | 未认证 | - | ✅ (最低) |
| Basic | 邮箱/手机验证 | `BasicKycValidity` | ✅ |
| Standard | 身份证件 | `StandardKycValidity` | ✅ |
| Enhanced | 地址 + 资金来源 | `EnhancedKycValidity` | ✅ |
| Institutional | 企业文件 + 受益人 | `InstitutionalKycValidity` | ✅ (最高) |

> KycLevel 实现 `PartialOrd + Ord`，支持 `>=` 比较。

## 数据结构

### KycRecord — 用户认证记录

```rust
pub struct KycRecord<AccountId, BlockNumber, MaxCidLen> {
    pub account: AccountId,                          // 用户账户
    pub level: KycLevel,                             // 申请级别
    pub status: KycStatus,                           // 当前状态
    pub provider: Option<AccountId>,                 // 审核提供者
    pub data_cid: Option<BoundedVec<u8, MaxCidLen>>, // 认证数据 IPFS CID（加密）
    pub submitted_at: Option<BlockNumber>,           // 提交时间
    pub verified_at: Option<BlockNumber>,            // 审核时间
    pub expires_at: Option<BlockNumber>,             // 过期时间
    pub rejection_reason: Option<RejectionReason>,   // 拒绝原因
    pub rejection_details_cid: Option<BoundedVec<u8, MaxCidLen>>, // 拒绝详情 CID
    pub country_code: Option<[u8; 2]>,               // ISO 3166-1 alpha-2
    pub risk_score: u8,                              // 风险评分 0-100
}
```

### KycProvider — 认证提供者

```rust
pub struct KycProvider<AccountId, MaxNameLen> {
    pub account: AccountId,
    pub name: BoundedVec<u8, MaxNameLen>,
    pub provider_type: ProviderType,    // Internal / ThirdParty / Government / Financial
    pub max_level: KycLevel,            // 支持的最高认证级别
    pub verifications_count: u64,       // 已完成审核数（含批准和拒绝）
}
```

### EntityKycRequirement — 实体 KYC 要求

```rust
pub struct EntityKycRequirement {
    pub min_level: KycLevel,               // 最低 KYC 级别
    pub mandatory: bool,                   // 是否强制要求
    pub grace_period: u32,                 // 宽限期（区块数）
    pub allow_high_risk_countries: bool,   // 是否允许高风险国家
    pub max_risk_score: u8,                // 最大允许风险评分
}
```

### 枚举类型

**KycStatus：** NotSubmitted → Pending → Approved / Rejected / Expired / Revoked

**RejectionReason（8 种）：** UnclearDocument / ExpiredDocument / InformationMismatch / SuspiciousActivity / SanctionedEntity / HighRiskCountry / ForgedDocument / Other

## Runtime 配置

```rust
impl pallet_entity_kyc::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxCidLength = ConstU32<64>;
    type MaxProviderNameLength = ConstU32<64>;
    type MaxProviders = ConstU32<10>;
    type BasicKycValidity = ...;     // ~1 年（区块数）
    type StandardKycValidity = ...;  // ~6 个月
    type EnhancedKycValidity = ...;  // ~1 年
    type InstitutionalKycValidity = ...; // ~1 年
    type AdminOrigin = EnsureRoot<AccountId>;
}
```

## Extrinsics

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 0 | `submit_kyc(level, data_cid, country_code)` | 任意用户 | 提交 KYC 申请（已有 Pending 时拒绝，已批准未过期时仅允许升级） |
| 1 | `approve_kyc(account, risk_score)` | 认证提供者 | 批准 KYC，设置有效期和风险评分 |
| 2 | `reject_kyc(account, reason, details_cid)` | 认证提供者 | 拒绝 KYC，记录原因 |
| 3 | `revoke_kyc(account, reason)` | AdminOrigin | 撤销 Pending/Approved/Expired 的 KYC |
| 4 | `register_provider(account, name, type, max_level)` | AdminOrigin | 注册认证提供者 |
| 5 | `remove_provider(account)` | AdminOrigin | 移除认证提供者 |
| 6 | `set_entity_requirement(entity_id, ...)` | Entity Owner / KYC_MANAGE Admin | 设置实体 KYC 要求 |
| 7 | `update_high_risk_countries(countries)` | AdminOrigin | 更新高风险国家列表（最多 50 个，自动去重） |
| 8 | `expire_kyc(account)` | 任意用户 | 标记已过期的 KYC 记录（需确实已过期） |
| 9 | `cancel_kyc()` | 申请人 | 取消自己的 Pending KYC 申请 |
| 10 | `force_set_entity_requirement(entity_id, ...)` | AdminOrigin | 强制设置实体 KYC 要求（不检查 Entity 存在） |
| 11 | `update_risk_score(account, new_score)` | 认证提供者 | 更新已批准用户的风险评分 |
| 12 | `update_provider(provider, name?, max_level?)` | AdminOrigin | 更新提供者信息（至少提供一个字段） |
| 13 | `suspend_provider(provider)` | AdminOrigin | 暂停认证提供者 |
| 14 | `resume_provider(provider)` | AdminOrigin | 恢复认证提供者 |
| 15 | `force_approve_kyc(account, level, risk_score, country_code)` | AdminOrigin | 强制批准 KYC（数据迁移/特殊豁免） |

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `KycRecords` | `StorageMap<AccountId, KycRecord>` | 用户 KYC 记录 |
| `Providers` | `StorageMap<AccountId, KycProvider>` | 认证提供者 |
| `ProviderCount` | `StorageValue<u32>` | 活跃提供者数量 |
| `EntityRequirements` | `StorageMap<u64, EntityKycRequirement>` | 实体 KYC 要求 |
| `HighRiskCountries` | `StorageValue<BoundedVec<[u8;2]>>` | 高风险国家列表 |

## Events

| 事件 | 说明 |
|------|------|
| `KycSubmitted` | KYC 已提交 |
| `KycApproved` | KYC 已通过（含 expires_at） |
| `KycRejected` | KYC 已拒绝（含 reason） |
| `KycExpired` | KYC 已过期 |
| `KycRevoked` | KYC 已撤销 |
| `KycCancelled` | KYC 申请已取消 |
| `KycForceApproved` | KYC 已强制批准 |
| `ProviderRegistered` | 提供者已注册 |
| `ProviderRemoved` | 提供者已移除 |
| `ProviderUpdated` | 提供者已更新 |
| `ProviderSuspended` | 提供者已暂停 |
| `ProviderResumed` | 提供者已恢复 |
| `RiskScoreUpdated` | 风险评分已更新 |
| `EntityRequirementSet` | 实体 KYC 要求已设置 |
| `HighRiskCountriesUpdated` | 高风险国家已更新 |

## Errors

| 错误 | 说明 |
|------|------|
| `KycNotFound` | KYC 记录不存在 |
| `KycAlreadyPending` | 已有待审核的 KYC |
| `KycAlreadyApproved` | KYC 已通过 |
| `ProviderNotFound` | 提供者不存在 |
| `ProviderAlreadyExists` | 提供者已存在 |
| `CidTooLong` / `NameTooLong` | 长度超限 |
| `MaxProvidersReached` | 达到最大提供者数量 |
| `InvalidKycStatus` / `InvalidKycLevel` | 状态/级别无效 |
| `InsufficientKycLevel` | KYC 级别不满足要求 |
| `HighRiskCountry` | 高风险国家 |
| `RiskScoreTooHigh` | 风险评分过高 |
| `KycExpired` | KYC 已过期 |
| `ProviderLevelNotSupported` | 提供者不支持此级别 |
| `TooManyCountries` | 高风险国家列表超出 50 上限 |
| `InvalidRiskScore` | 风险评分超出 0-100 范围 |
| `EmptyProviderName` | 提供者名称为空 |
| `EmptyDataCid` | KYC 数据 CID 为空 |
| `InvalidCountryCode` | 国家代码格式无效 |
| `SelfApprovalNotAllowed` | 不允许自我审批 |
| `KycNotExpired` | KYC 尚未过期（expire_kyc 调用时） |
| `NotEntityOwnerOrAdmin` | 非 Entity Owner 或授权管理员 |
| `EntityNotFound` | Entity 不存在 |
| `ProviderIsSuspended` | 提供者已被暂停 |
| `ProviderNotSuspended` | 提供者未被暂停 |
| `NothingToUpdate` | 未提供任何更新字段 |

## 辅助函数

```rust
impl<T: Config> Pallet<T> {
    /// 获取 KYC 有效期（按级别不同）
    pub fn get_validity_period(level: KycLevel) -> BlockNumber;
    /// 检查用户是否满足 KYC 要求（含过期检查）
    pub fn meets_kyc_requirement(account: &AccountId, min_level: KycLevel) -> bool;
    /// 获取用户当前 KYC 级别（仅 Approved 状态）
    pub fn get_kyc_level(account: &AccountId) -> KycLevel;
    /// 检查用户是否来自高风险国家
    pub fn is_high_risk_country(account: &AccountId) -> bool;
    /// 综合检查用户能否参与实体活动（级别+国家+风险+过期）
    pub fn can_participate_in_entity(account: &AccountId, entity_id: u64) -> bool;
    /// 获取用户风险评分（未认证返回 100）
    pub fn get_risk_score(account: &AccountId) -> u8;
}
```

## 隐私说明

- KYC 数据通过 IPFS CID 引用，实际数据加密存储在链下
- 链上只存储认证状态、级别、风险评分等元数据
- 符合 GDPR 数据最小化原则

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-02-03 | Phase 7 初始版本 |
| v0.2.0 | 2026-03 | Round 1-3 审计修复: 空 CID 校验、自我审批阻止、风险评分验证、国家代码格式校验、Expired 状态可撤销 |
| v0.3.0 | 2026-03 | Round 4 审计: M1(高风险国家去重) M2(拒绝计入审核数) M3(expire_kyc extrinsic) L2(README同步) L3(Cargo features) |
| v0.4.0 | 2026-03 | P0-P2 功能: Entity Owner 设置 KYC 要求、cancel_kyc、upgrade KYC、force_approve、provider 管理(suspend/resume/update)、update_risk_score、KycLevel as_u8/try_from_u8 |
| v0.5.0 | 2026-03 | Round 6 审计: M1(get_risk_score 状态+过期检查) L1(update_provider 拒绝 no-op) L2-L5(补充测试覆盖) L6(README 同步) |
