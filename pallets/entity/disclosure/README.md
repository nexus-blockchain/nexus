# pallet-entity-disclosure

> 实体财务信息披露、内幕交易控制、违规追踪、多方签核、渐进式处罚与公告发布模块

## 概述

`pallet-entity-disclosure` 为 NEXUS 平台实体提供链上财务信息披露框架，围绕八大能力构建：

1. **定期披露** — 按级别（Basic → Full）自动计算截止时间；支持草稿工作流
2. **内幕人员管理** — 注册/注销/角色变更，冷静期限制，角色历史追踪，大股东自动注册
3. **黑窗口期控制** — 披露后自动触发交易限制，手动管理，不可缩短已有窗口
4. **违规检测** — 手动举报 + `on_idle` 自动扫描，累计违规达阈值标记高风险
5. **公告发布** — 发布/更新/撤回，8 种分类，可选过期，多置顶
6. **多方审批签核** — 可配置审批角色和人数，草稿需签核后方可发布
7. **渐进式处罚** — None → Warning → Restricted → Suspended → Delisted 自动/手动升级
8. **紧急披露 + 实体状态联动** — 紧急快速通道、实体暂停时截止时间暂停

### 架构定位

```
pallet-entity-registry ──► EntityProvider
        │
        ▼
pallet-entity-disclosure
  ├── 披露记录 + 草稿工作流    ── IPFS CID 引用链下内容
  ├── 多方审批签核 (v0.6)      ── 角色位掩码控制审批资格
  ├── 内幕人员 + 角色历史      ── 供 market/token 模块查询
  ├── 大股东自动注册 (v0.6)    ── token 模块通过 trait 驱动
  ├── 黑窗口期 + 冷静期        ── can_insider_trade()
  ├── 违规追踪 + 高风险标记    ── on_idle 自动检测
  ├── 渐进式处罚 (v0.6)        ── 自动/手动升级，回调下游
  ├── 紧急披露 (v0.6)          ── 跳过审批，加倍黑窗口期
  ├── 实体状态联动 (v0.6)      ── OnEntityStatusChange 暂停/恢复截止
  ├── 内幕交易申报 (v0.6)      ── 内幕人员自主申报交易
  ├── 披露元数据 (v0.6)        ── 报告期间、审计签核
  └── 公告管理 + 多置顶        ── 发布/更新/撤回/过期
```

## 披露级别

| 级别 | 间隔常量 | 说明 |
|------|---------|------|
| **Basic** | `BasicDisclosureInterval` | 年度简报，小型实体 |
| **Standard** | `StandardDisclosureInterval` | 季度报告，中等实体 |
| **Enhanced** | `EnhancedDisclosureInterval` | 月度报告，大型实体 |
| **Full** | 间隔 = 0 | 实时披露，上市级实体 |

管理员只能升级级别，降级需 Root 或治理。

## 披露类型（13 种）

| 类型 | Basic | Standard | Enhanced | Full |
|------|:-----:|:--------:|:--------:|:----:|
| `AnnualReport` / `Other` | ✓ | ✓ | ✓ | ✓ |
| `QuarterlyReport` / `MaterialEvent` / `RiskWarning` | | ✓ | ✓ | ✓ |
| `MonthlyReport` / `RelatedPartyTransaction` / `OwnershipChange` / `ManagementChange` / `BusinessChange` / `DividendAnnouncement` | | | ✓ | ✓ |
| `TokenIssuance` / `Buyback` | | | | ✓ |

## 数据结构

### DisclosureRecord

```rust
pub struct DisclosureRecord<AccountId, BlockNumber, MaxCidLen> {
    pub id: u64,
    pub entity_id: u64,
    pub disclosure_type: DisclosureType,
    pub content_cid: BoundedVec<u8, MaxCidLen>,
    pub summary_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub discloser: AccountId,
    pub disclosed_at: BlockNumber,
    pub status: DisclosureStatus,       // Draft / Published / Withdrawn / Corrected
    pub previous_id: Option<u64>,       // 更正链
}
```

**状态流转：** `Draft → Published → Withdrawn / Corrected`（Draft 可直接删除）

### DisclosureConfig

```rust
pub struct DisclosureConfig<BlockNumber> {
    pub level: DisclosureLevel,
    pub insider_trading_control: bool,
    pub blackout_period_after: BlockNumber,
    pub next_required_disclosure: BlockNumber,
    pub last_disclosure: BlockNumber,
    pub violation_count: u32,               // 重新配置时保留
}
```

### InsiderRecord / InsiderRoleChangeRecord

```rust
pub struct InsiderRecord<AccountId, BlockNumber> {
    pub account: AccountId,
    pub role: InsiderRole,      // Owner | Admin | Auditor | Advisor | MajorHolder
    pub added_at: BlockNumber,
}

pub struct InsiderRoleChangeRecord<BlockNumber> {
    pub old_role: Option<InsiderRole>,  // None = 初始添加
    pub new_role: InsiderRole,
    pub changed_at: BlockNumber,
}
```

移除使用 `swap_remove` 硬删除，之后进入冷静期（`InsiderCooldownPeriod`）。

### v0.6 新增数据结构

```rust
/// 审计状态
pub enum AuditStatus { NotRequired, Pending, Approved, Rejected }

/// 渐进式处罚级别
pub enum PenaltyLevel { None, Warning, Restricted, Suspended, Delisted }

/// 多方签核配置
pub struct ApprovalConfig {
    pub required_approvals: u32,
    pub allowed_roles: u8,     // 位掩码: Owner=0x01 Admin=0x02 Auditor=0x04 Advisor=0x08 MajorHolder=0x10
}

/// 披露扩展元数据（分离存储）
pub struct DisclosureMetadata<BlockNumber> {
    pub period_start: Option<BlockNumber>,
    pub period_end: Option<BlockNumber>,
    pub audit_status: AuditStatus,
    pub is_emergency: bool,
}

/// 内幕人员交易申报
pub struct InsiderTransactionReport<AccountId, BlockNumber> {
    pub account: AccountId,
    pub transaction_type: InsiderTransactionType,  // Buy|Sell|Transfer|Pledge|Gift
    pub token_amount: u128,
    pub reported_at: BlockNumber,
    pub transaction_block: BlockNumber,
}

/// 财务年度配置
pub struct FiscalYearConfig<BlockNumber> {
    pub year_start_block: BlockNumber,
    pub year_length: BlockNumber,
}
```

### AnnouncementRecord

```rust
pub struct AnnouncementRecord<AccountId, BlockNumber, MaxCidLen, MaxTitleLen> {
    pub id: u64,
    pub entity_id: u64,
    pub category: AnnouncementCategory,  // General|Promotion|SystemUpdate|Event|Policy|Partnership|Product|Other
    pub title: BoundedVec<u8, MaxTitleLen>,
    pub content_cid: BoundedVec<u8, MaxCidLen>,
    pub publisher: AccountId,
    pub published_at: BlockNumber,
    pub expires_at: Option<BlockNumber>,
    pub status: AnnouncementStatus,      // Active / Withdrawn / Expired
    pub is_pinned: bool,
}
```

### ViolationType

`LateDisclosure` | `BlackoutTrading` | `UndisclosedMaterialEvent`

## Extrinsics（39 个）

### 披露管理

| # | 函数 | 权限 | 说明 |
|---|------|------|------|
| 0 | `configure_disclosure` | Admin | 配置披露设置；不可降级；保留 violation_count |
| 1 | `publish_disclosure` | Admin | 直接发布；更新配置；自动触发黑窗口期 |
| 2 | `withdraw_disclosure` | Admin/Discloser | Published → Withdrawn |
| 3 | `correct_disclosure` | Admin | 创建更正版本；触发黑窗口期 |

### 草稿工作流

| # | 函数 | 权限 | 说明 |
|---|------|------|------|
| 18 | `create_draft_disclosure` | Admin | 创建草稿；不触发黑窗口 |
| 19 | `update_draft` | Admin | 更新草稿内容 |
| 20 | `delete_draft` | Admin | 删除草稿 |
| 21 | `publish_draft` | Admin | Draft → Published；**检查审批要求**；触发黑窗口 |

### 多方审批签核 (v0.6)

| # | 函数 | 权限 | 说明 |
|---|------|------|------|
| 28 | `configure_approval_requirements` | Admin | 配置审批人数和允许角色 |
| 29 | `approve_disclosure` | 内幕人员(角色匹配) | 审批草稿 |
| 30 | `reject_disclosure` | 内幕人员(角色匹配) | 拒绝并重置全部审批 |

### 内幕人员管理

| # | 函数 | 权限 | 说明 |
|---|------|------|------|
| 4 | `add_insider` | Admin | 添加；记录初始角色历史 |
| 5 | `remove_insider` | Admin | 硬删除；启动冷静期 |
| 22 | `update_insider_role` | Admin | 更新角色；记录变更历史 |
| 24 | `batch_add_insiders` | Admin | 批量添加（原子性） |
| 25 | `batch_remove_insiders` | Admin | 批量移除（原子性）；启动冷静期 |

### 黑窗口期

| # | 函数 | 权限 | 说明 |
|---|------|------|------|
| 6 | `start_blackout` | Admin | 手动开启；不可缩短已有窗口 |
| 7 | `end_blackout` | Admin | 手动提前结束 |
| 27 | `expire_blackout` | 任何人 | 清理已过期的存储 |

### 违规管理

| # | 函数 | 权限 | 说明 |
|---|------|------|------|
| 15 | `report_disclosure_violation` | 任何人 | 举报违规；同周期去重；达阈值标记高风险 |
| 26 | `reset_violation_count` | Root | 重置违规 + 清除高风险标记 |

### 渐进式处罚 (v0.6)

| # | 函数 | 权限 | 说明 |
|---|------|------|------|
| 34 | `escalate_penalty` | Root | 手动升级处罚级别（仅升不降） |
| 35 | `reset_penalty` | Root | 重置处罚级别为 None |

### 紧急披露 (v0.6)

| # | 函数 | 权限 | 说明 |
|---|------|------|------|
| 31 | `publish_emergency_disclosure` | Admin | 跳过审批，触发加倍黑窗口期 |

### 内幕人员交易申报 (v0.6)

| # | 函数 | 权限 | 说明 |
|---|------|------|------|
| 32 | `report_insider_transaction` | 内幕人员/冷静期 | 自主申报交易（买/卖/转/质押/赠送） |

### 披露元数据与审计 (v0.6)

| # | 函数 | 权限 | 说明 |
|---|------|------|------|
| 33 | `configure_fiscal_year` | Admin | 配置财务年度起始和周期 |
| 37 | `set_disclosure_metadata` | Admin | 设置报告期间、审计要求 |
| 38 | `audit_disclosure` | Auditor | 审计员签核/拒绝披露 |

### 公告管理

| # | 函数 | 权限 | 说明 |
|---|------|------|------|
| 8 | `publish_announcement` | Admin | 发布公告 |
| 9 | `update_announcement` | Admin | 部分更新；不可更新已过期公告 |
| 10 | `withdraw_announcement` | Admin/Publisher | 撤回；自动清除置顶 |
| 11 | `pin_announcement` | Admin | 置顶（幂等） |
| 23 | `unpin_announcement` | Admin | 取消置顶 |
| 12 | `expire_announcement` | 任何人 | 标记已过期公告 |

### 强制/清理

| # | 函数 | 权限 | 说明 |
|---|------|------|------|
| 16 | `force_configure_disclosure` | Root | 强制配置（可降级） |
| 13 | `cleanup_disclosure_history` | 任何人 | 移除已终态披露 ID，释放容量 |
| 14 | `cleanup_announcement_history` | 任何人 | 移除已终态公告 ID |
| 17 | `cleanup_entity_disclosure` | 任何人 | 清理已关闭/已封禁实体全部存储 |
| 36 | `cleanup_expired_cooldowns` | 任何人 | 清理已过期冷静期记录 (v0.6) |

> **权限：** "Admin" = 持有 `DISCLOSURE_MANAGE` 权限。大部分写操作要求实体 Active 且未锁定。

## 渐进式处罚框架 (v0.6)

违规达阈值后自动升级处罚级别：

| 违规次数 | 处罚级别 | 效果 |
|---------|---------|------|
| < threshold/2 | None | 无处罚 |
| ≥ threshold/2 | Warning | 记录警告事件 |
| ≥ threshold | Restricted | 标记高风险，`is_penalty_active()` 返回 true |
| ≥ threshold×2 | Suspended | 通过 `OnDisclosureViolation` 回调通知下游 |
| ≥ threshold×3 | Delisted | 最高级别处罚 |

Root 可通过 `escalate_penalty` / `reset_penalty` 手动管理。

## 多方审批工作流 (v0.6)

```
configure_approval_requirements(entity_id, required=2, roles=Auditor|Owner)
  │
  ▼
create_draft_disclosure(...)
  │
  ├── approve_disclosure(by Auditor A)  ← count=1
  ├── approve_disclosure(by Auditor B)  ← count=2 ≥ required
  │
  ▼
publish_draft(...)  ← 自动检查审批是否满足
```

- 任何审批人调用 `reject_disclosure` 会重置全部审批
- `publish_emergency_disclosure` 跳过审批要求

## OnEntityStatusChange (v0.6)

实体暂停/封禁/恢复/关闭时自动联动：

| 事件 | 响应 |
|------|------|
| `on_entity_suspended` | 暂停披露截止计时（保存剩余区块） |
| `on_entity_banned` | 暂停披露截止计时 |
| `on_entity_resumed` | 恢复截止计时（基于剩余区块重新计算） |
| `on_entity_closed` | 清除暂停记录 |

`on_idle` 自动扫描会跳过已暂停和非 Active 的实体，不产生无意义违规。

## 大股东自动注册 (v0.6)

通过 `DisclosureProvider::register_major_holder` / `deregister_major_holder` 接口，token 模块在持仓变动时自动注册/注销大股东为内幕人员：

- 超过 `MajorHolderThreshold`（5%）→ 自动添加 `InsiderRole::MajorHolder`
- 低于阈值 → 自动移除并进入冷静期
- 已配置披露的实体才生效

## on_idle 自动违规检测

- 每批最多扫描 **10** 个实体，使用 skip-count 游标避免重复扫描
- `ViolationRecords` 去重：同一逾期周期不重复计数
- 权重同时检查 `ref_time` 和 `proof_size`，跳过的实体也计入权重
- 违规达 `ViolationThreshold` 时自动标记高风险 + 自动升级处罚级别
- v0.6: 跳过已暂停截止时间的实体和非 Active 实体

## 存储（23 项）

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextDisclosureId` | `Value<u64>` | 全局自增披露 ID |
| `Disclosures` | `Map<u64 → DisclosureRecord>` | 披露记录 |
| `DisclosureConfigs` | `Map<u64 → DisclosureConfig>` | 实体披露配置 |
| `EntityDisclosures` | `Map<u64 → BoundedVec<u64>>` | 实体披露 ID 列表 |
| `Insiders` | `Map<u64 → BoundedVec<InsiderRecord>>` | 内幕人员列表 |
| `BlackoutPeriods` | `Map<u64 → (Block, Block)>` | 黑窗口期起止 |
| `NextAnnouncementId` | `Value<u64>` | 全局自增公告 ID |
| `Announcements` | `Map<u64 → AnnouncementRecord>` | 公告记录 |
| `EntityAnnouncements` | `Map<u64 → BoundedVec<u64>>` | 实体公告 ID 列表 |
| `ViolationRecords` | `DoubleMap<u64, Block → bool>` | 违规去重 |
| `PinnedAnnouncements` | `Map<u64 → BoundedVec<u64>>` | 置顶公告列表 |
| `InsiderRoleHistory` | `DoubleMap<u64, Account → BoundedVec>` | 角色变更历史 |
| `RemovedInsiders` | `DoubleMap<u64, Account → Block>` | 冷静期截止 |
| `AutoViolationCursor` | `Value<u32>` | on_idle 跳过计数 |
| `HighRiskEntities` | `Map<u64 → bool>` | 高风险标记 |
| `ApprovalConfigs` | `Map<u64 → ApprovalConfig>` | 审批配置 (v0.6) |
| `DisclosureApprovals` | `DoubleMap<u64, Account → bool>` | 审批记录 (v0.6) |
| `DisclosureApprovalCounts` | `Map<u64 → u32>` | 审批计数 (v0.6) |
| `InsiderTransactionReports` | `DoubleMap<u64, Account → BoundedVec>` | 交易申报 (v0.6) |
| `EntityPenalties` | `Map<u64 → PenaltyLevel>` | 处罚级别 (v0.6) |
| `FiscalYearConfigs` | `Map<u64 → FiscalYearConfig>` | 财务年度 (v0.6) |
| `PausedDeadlines` | `Map<u64 → (Block, Block)>` | 暂停截止 (v0.6) |
| `DisclosureMetadataStore` | `Map<u64 → DisclosureMetadata>` | 扩展元数据 (v0.6) |

## DisclosureProvider Trait

供外部模块调用：

```rust
fn is_in_blackout(entity_id) -> bool;
fn is_insider(entity_id, account) -> bool;
fn can_insider_trade(entity_id, account) -> bool;
fn get_disclosure_level(entity_id) -> DisclosureLevel;
fn is_disclosure_overdue(entity_id) -> bool;
fn get_violation_count(entity_id) -> u32;
fn get_insider_role(entity_id, account) -> Option<u8>;
fn is_disclosure_configured(entity_id) -> bool;
fn is_high_risk(entity_id) -> bool;
fn governance_configure_disclosure(...) -> DispatchResult;
fn governance_reset_violations(entity_id) -> DispatchResult;
// v0.6 新增
fn register_major_holder(entity_id, account) -> DispatchResult;
fn deregister_major_holder(entity_id, account) -> DispatchResult;
fn get_penalty_level(entity_id) -> u8;
fn is_penalty_active(entity_id) -> bool;
```

## OnDisclosureViolation Trait (v0.6)

```rust
pub trait OnDisclosureViolation {
    fn on_violation_threshold_reached(entity_id: u64, violation_count: u32, penalty_level: u8);
}
```

## Runtime 配置

```rust
impl pallet_entity_disclosure::Config for Runtime {
    type EntityProvider = EntityRegistry;
    type MaxCidLength = ConstU32<64>;
    type MaxInsiders = ConstU32<50>;
    type MaxDisclosureHistory = ConstU32<100>;
    type BasicDisclosureInterval = ...;       // ~1 年
    type StandardDisclosureInterval = ...;    // ~3 个月
    type EnhancedDisclosureInterval = ...;    // ~1 个月
    type MaxBlackoutDuration = ...;           // ~3 天
    type MaxAnnouncementHistory = ConstU32<200>;
    type MaxTitleLength = ConstU32<128>;
    type MaxPinnedAnnouncements = ConstU32<5>;
    type MaxInsiderRoleHistory = ConstU32<20>;
    type InsiderCooldownPeriod = ...;         // ~1 天
    type MajorHolderThreshold = ConstU32<500>; // 5%
    type ViolationThreshold = ConstU32<3>;
    // v0.6 新增
    type MaxApprovers = ConstU32<10>;
    type MaxInsiderTransactionHistory = ConstU32<50>;
    type EmergencyBlackoutMultiplier = ConstU32<3>;
    type OnDisclosureViolation = ();
}
```

## 依赖

| Crate | 用途 |
|-------|------|
| `pallet-entity-common` | `EntityProvider`、`DisclosureProvider`、`DisclosureLevel`、`AdminPermission`、`EntityStatus`、`OnEntityStatusChange`、`OnDisclosureViolation` |
| `frame-support` / `frame-system` | Substrate FRAME 框架 |
| `sp-runtime` | `Saturating`、`Zero`、`SaturatedConversion` |

## 测试

```bash
cargo test -p pallet-entity-disclosure
```

234 个测试，覆盖：配置、发布、草稿工作流、多方审批、内幕人员管理、大股东自动注册、黑窗口期、违规检测、渐进式处罚、紧急披露、内幕交易申报、审计签核、财务年度、实体状态联动、公告管理、权限控制、存储清理、边界条件。

## 版本历史

| 版本 | 变更 |
|------|------|
| v0.1.0 | 初始版本：8 extrinsic (0-7)、13 种披露类型、5 种内幕角色 |
| v0.2.0 | 公告功能：+4 extrinsic (8-11)、AnnouncementRecord、置顶 |
| v0.3.0 | 违规追踪：report_violation (15)、force_configure (16)、cleanup (13-14, 17)、on_idle 自动检测 |
| v0.4.0 | 草稿工作流：draft extrinsics (18-21)、update_insider_role (22)、unpin (23)、InsiderRoleHistory |
| v0.5.0 | 批量操作：batch insiders (24-25)、reset_violation (26)、expire_blackout (27)、HighRiskEntities、冷静期、DisclosureProvider trait |
| v0.5.1 | 审计修复 Round 1-2：on_idle 游标改为 skip-count、proof_size 权重、governance blackout 验证、feature 传播 |
| v0.6.0 | 深度增强：+11 extrinsic (28-38)、多方审批签核、大股东自动注册、渐进式处罚、紧急披露、内幕交易申报、审计签核、财务年度、实体状态联动(OnEntityStatusChange)、OnDisclosureViolation trait、8 新存储项 |
