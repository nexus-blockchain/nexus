# pallet-entity-disclosure

> 实体财务信息披露、内幕交易控制与公告发布模块

## 概述

`pallet-entity-disclosure` 为 NEXUS 平台的实体（企业/店铺）提供链上财务信息披露框架。模块围绕三大核心能力构建：

1. **定期披露** — 按实体设定的级别（Basic → Full），自动计算下次披露截止时间，支持逾期违规追踪
2. **内幕人员管理** — 注册/注销实体内幕人员，五种角色分类，供外部模块查询交易资格
3. **黑窗口期控制** — 披露发布后自动触发交易限制窗口，或由管理员手动管理
4. **公告发布** — 实体发布、更新、撤回公告，支持 8 种分类、可选过期时间、置顶功能

### 架构定位

```
pallet-entity-registry (实体)
        │
        ▼
pallet-entity-disclosure ◄── EntityProvider (entity_owner, entity_exists)
        │
        ├── 披露记录 (DisclosureRecord) ── IPFS CID 引用链下内容
        ├── 内幕人员 (InsiderRecord)     ── 供 market/token 模块查询
        ├── 黑窗口期 (BlackoutPeriods)   ── is_in_blackout() / can_insider_trade()
        └── 公告管理 (AnnouncementRecord) ── 发布/更新/撤回/置顶
```

外部模块通过 `Pallet::<T>::can_insider_trade(entity_id, &account)` 查询账户是否允许交易。

## 披露级别

| 级别 | 典型要求 | 间隔配置常量 | 说明 |
|------|---------|-------------|------|
| **Basic** | 年度简报 | `BasicDisclosureInterval` | 最低要求，适合小型实体 |
| **Standard** | 季度报告 | `StandardDisclosureInterval` | 中等规模实体 |
| **Enhanced** | 月度报告 + 重大事件 | `EnhancedDisclosureInterval` | 大型实体 |
| **Full** | 实时披露 | 间隔 = 0 | 上市级/代币发行实体 |

每次 `publish_disclosure` 后，自动调用 `calculate_next_disclosure` 更新 `next_required_disclosure`。Full 级别间隔为 0，表示无固定周期，需即时披露。

## 披露类型（13 种）

| 类型 | 说明 | 适用级别 |
|------|------|---------|
| `AnnualReport` | 年度财务报告 | Basic+ |
| `QuarterlyReport` | 季度财务报告 | Standard+ |
| `MonthlyReport` | 月度财务报告 | Enhanced+ |
| `MaterialEvent` | 重大事件公告 | Enhanced+ |
| `RelatedPartyTransaction` | 关联交易披露 | Enhanced+ |
| `OwnershipChange` | 股权/代币持有变动 | Standard+ |
| `ManagementChange` | 管理层人事变动 | Standard+ |
| `BusinessChange` | 业务模式/范围变更 | Standard+ |
| `RiskWarning` | 风险提示/预警 | 任意 |
| `DividendAnnouncement` | 分红公告 | Standard+ |
| `TokenIssuance` | 代币发行/增发公告 | Standard+ |
| `Buyback` | 回购计划公告 | Standard+ |
| `Other` | 其他自定义披露 | 任意 |

## 数据结构

### DisclosureRecord

```rust
pub struct DisclosureRecord<AccountId, BlockNumber, MaxCidLen: Get<u32>> {
    pub id: u64,                                        // 自增 ID
    pub entity_id: u64,                                 // 所属实体
    pub disclosure_type: DisclosureType,                // 类型（13 种）
    pub content_cid: BoundedVec<u8, MaxCidLen>,         // 内容 IPFS CID
    pub summary_cid: Option<BoundedVec<u8, MaxCidLen>>, // 摘要 CID（可选）
    pub discloser: AccountId,                           // 发布者
    pub disclosed_at: BlockNumber,                      // 发布区块
    pub status: DisclosureStatus,                       // Pending → Published → Withdrawn / Corrected
    pub previous_id: Option<u64>,                       // 更正链：指向前一版本
    pub verifier: Option<AccountId>,                    // 验证者（预留）
    pub verified_at: Option<BlockNumber>,               // 验证时间（预留）
}
```

**状态流转：**

```
   ┌─── Pending (默认初始值，当前 publish 直接进 Published)
   │
Published ──┬── Withdrawn  (撤回)
             └── Corrected  (被新版本取代)
```

### DisclosureConfig

```rust
pub struct DisclosureConfig<BlockNumber> {
    pub level: DisclosureLevel,              // 当前披露级别
    pub insider_trading_control: bool,       // 是否启用内幕交易控制
    pub blackout_period_before: BlockNumber, // 披露前黑窗口期长度
    pub blackout_period_after: BlockNumber,  // 披露后黑窗口期长度
    pub next_required_disclosure: BlockNumber, // 下次必须披露的截止区块
    pub last_disclosure: BlockNumber,        // 上次披露区块
    pub violation_count: u32,                // 累计违规次数
}
```

### InsiderRecord

```rust
pub struct InsiderRecord<AccountId, BlockNumber> {
    pub account: AccountId,    // 内幕人员账户
    pub role: InsiderRole,     // 角色
    pub added_at: BlockNumber, // 注册时间
    pub active: bool,          // 是否活跃（remove 时设为 false）
}
```

**InsiderRole：** `Owner` | `Admin` | `Auditor` | `Advisor` | `MajorHolder`

### ViolationType

| 类型 | 说明 |
|------|------|
| `LateDisclosure` | 逾期未披露 |
| `BlackoutTrading` | 黑窗口期内交易 |
| `UndisclosedMaterialEvent` | 未披露重大事件 |

### AnnouncementRecord

```rust
pub struct AnnouncementRecord<AccountId, BlockNumber, MaxCidLen, MaxTitleLen> {
    pub id: u64,                                    // 自增 ID
    pub entity_id: u64,                             // 所属实体
    pub category: AnnouncementCategory,             // 分类（8 种）
    pub title: BoundedVec<u8, MaxTitleLen>,          // 标题
    pub content_cid: BoundedVec<u8, MaxCidLen>,     // 内容 IPFS CID
    pub publisher: AccountId,                       // 发布者
    pub published_at: BlockNumber,                  // 发布区块
    pub expires_at: Option<BlockNumber>,            // 过期时间（None = 永不过期）
    pub status: AnnouncementStatus,                 // Active / Withdrawn / Expired
    pub is_pinned: bool,                            // 是否置顶
}
```

**AnnouncementCategory（8 种）：** `General` | `Promotion` | `SystemUpdate` | `Event` | `Policy` | `Partnership` | `Product` | `Other`

**状态流转：**

```
Active ───┬─── Withdrawn  (撤回)
           └─── Expired    (过期)
```

## Extrinsics

| # | 函数 | 权限 | 说明 |
|---|------|------|------|
| 0 | `configure_disclosure(entity_id, level, insider_control, blackout_before, blackout_after)` | Entity Owner | 初始化或更新实体披露配置，自动计算 `next_required_disclosure` |
| 1 | `publish_disclosure(entity_id, type, content_cid, summary_cid?)` | Entity Owner | 发布披露记录；更新配置中的 `last_disclosure` 和 `next_required_disclosure`；若启用内幕控制且 `blackout_after > 0`，自动开启黑窗口期 |
| 2 | `withdraw_disclosure(disclosure_id)` | Owner 或 Discloser | 撤回已发布的披露（状态 → `Withdrawn`） |
| 3 | `correct_disclosure(old_id, content_cid, summary_cid?)` | Entity Owner | 创建更正版本（新记录 `previous_id` 指向旧 ID），旧记录状态 → `Corrected` |
| 4 | `add_insider(entity_id, account, role)` | Entity Owner | 添加内幕人员，检查重复（仅检查 `active=true` 的记录） |
| 5 | `remove_insider(entity_id, account)` | Entity Owner | 软删除内幕人员（`active=false`），保留历史记录 |
| 6 | `start_blackout(entity_id, duration)` | Entity Owner | 手动开启黑窗口期，设置 `(now, now + duration)` |
| 7 | `end_blackout(entity_id)` | Entity Owner | 手动提前结束黑窗口期（删除存储记录） |
| 8 | `publish_announcement(entity_id, category, title, content_cid, expires_at?)` | Entity Owner | 发布公告，支持可选过期时间 |
| 9 | `update_announcement(id, title?, content_cid?, category?, expires_at?)` | Entity Owner | 更新公告内容（仅 Active 状态），支持部分更新 |
| 10 | `withdraw_announcement(announcement_id)` | Owner 或 Publisher | 撤回公告（Active → Withdrawn），自动清除置顶 |
| 11 | `pin_announcement(entity_id, announcement_id?)` | Entity Owner | 置顶/取消置顶公告（Some=置顶，None=取消），每实体最多一个 |

## 存储

| 存储项 | 键 | 值 | 说明 |
|--------|----|----|------|
| `NextDisclosureId` | — | `u64` | 全局自增 ID |
| `Disclosures` | `disclosure_id` | `DisclosureRecord` | 所有披露记录 |
| `DisclosureConfigs` | `entity_id` | `DisclosureConfig` | 实体级披露配置 |
| `EntityDisclosures` | `entity_id` | `BoundedVec<u64, MaxDisclosureHistory>` | 实体关联的披露 ID 列表 |
| `Insiders` | `entity_id` | `BoundedVec<InsiderRecord, MaxInsiders>` | 内幕人员列表 |
| `BlackoutPeriods` | `entity_id` | `(BlockNumber, BlockNumber)` | 黑窗口期起止区块 |
| `NextAnnouncementId` | — | `u64` | 公告全局自增 ID |
| `Announcements` | `announcement_id` | `AnnouncementRecord` | 公告记录 |
| `EntityAnnouncements` | `entity_id` | `BoundedVec<u64, MaxAnnouncementHistory>` | 实体关联公告 ID 列表 |
| `PinnedAnnouncement` | `entity_id` | `u64` | 实体置顶公告 ID（最多一个） |

## 事件

| 事件 | 字段 | 触发时机 |
|------|------|---------|
| `DisclosurePublished` | `disclosure_id, entity_id, disclosure_type, discloser` | `publish_disclosure` |
| `DisclosureWithdrawn` | `disclosure_id, entity_id` | `withdraw_disclosure` |
| `DisclosureCorrected` | `old_disclosure_id, new_disclosure_id, entity_id` | `correct_disclosure` |
| `DisclosureConfigUpdated` | `entity_id, level` | `configure_disclosure` |
| `InsiderAdded` | `entity_id, account, role` | `add_insider` |
| `InsiderRemoved` | `entity_id, account` | `remove_insider` |
| `BlackoutStarted` | `entity_id, start_block, end_block` | `start_blackout` 或 `publish_disclosure` 自动触发 |
| `BlackoutEnded` | `entity_id` | `end_blackout` |
| `DisclosureViolation` | `entity_id, violation_type` | 违规检测时发出 |
| `AnnouncementPublished` | `announcement_id, entity_id, category, publisher` | `publish_announcement` |
| `AnnouncementUpdated` | `announcement_id, entity_id` | `update_announcement` |
| `AnnouncementWithdrawn` | `announcement_id, entity_id` | `withdraw_announcement` |
| `AnnouncementPinned` | `entity_id, announcement_id` | `pin_announcement(Some)` |
| `AnnouncementUnpinned` | `entity_id` | `pin_announcement(None)` |

## 错误

| 错误 | 说明 |
|------|------|
| `EntityNotFound` | `EntityProvider::entity_owner` 返回 `None` |
| `NotAdmin` | 调用者不是实体所有者 |
| `DisclosureNotFound` | `disclosure_id` 不存在 |
| `CidTooLong` | CID 超过 `MaxCidLength` |
| `HistoryFull` | `EntityDisclosures` 达到 `MaxDisclosureHistory` 上限 |
| `InsiderExists` | 目标账户已是活跃内幕人员 |
| `InsiderNotFound` | 目标账户不在活跃内幕人员列表中 |
| `InsidersFull` | `Insiders` 达到 `MaxInsiders` 上限 |
| `InBlackoutPeriod` | 当前处于黑窗口期 |
| `InvalidDisclosureStatus` | 披露状态不允许该操作（如撤回非 Published 的记录） |
| `InsufficientDisclosureLevel` | 披露级别不满足要求 |
| `DisclosureIntervalNotReached` | 披露间隔未到 |
| `AnnouncementNotFound` | `announcement_id` 不存在 |
| `AnnouncementHistoryFull` | `EntityAnnouncements` 达到 `MaxAnnouncementHistory` 上限 |
| `AnnouncementNotActive` | 公告状态不是 Active |
| `EmptyTitle` | 公告标题为空 |
| `TitleTooLong` | 标题超过 `MaxTitleLength` |
| `InvalidExpiry` | 过期时间早于当前区块 |

## 公开查询接口

```rust
// 计算指定级别下一次必须披露的区块号
Pallet::<T>::calculate_next_disclosure(level, now) -> BlockNumber

// 实体当前是否处于黑窗口期
Pallet::<T>::is_in_blackout(entity_id) -> bool

// 账户是否是该实体的活跃内幕人员
Pallet::<T>::is_insider(entity_id, &account) -> bool

// 内幕人员是否可以交易（非内幕人员始终返回 true）
// 逻辑：非内幕 → true; 未启用控制 → true; 不在黑窗口 → true
Pallet::<T>::can_insider_trade(entity_id, &account) -> bool

// 获取实体披露级别（无配置时返回 Basic）
Pallet::<T>::get_disclosure_level(entity_id) -> DisclosureLevel

// 检查实体是否逾期未披露
Pallet::<T>::is_disclosure_overdue(entity_id) -> bool

// 检查公告是否已过期
Pallet::<T>::is_announcement_expired(announcement_id) -> bool

// 获取实体置顶公告 ID
Pallet::<T>::get_pinned_announcement(entity_id) -> Option<u64>
```

## Runtime 配置

```rust
impl pallet_entity_disclosure::Config for Runtime {
    type EntityProvider = EntityRegistry;        // EntityProvider trait 实现
    type MaxCidLength = ConstU32<64>;            // IPFS CID 最大字节数
    type MaxInsiders = ConstU32<50>;             // 每实体最大内幕人员数
    type MaxDisclosureHistory = ConstU32<100>;   // 每实体最大披露历史数
    type BasicDisclosureInterval = ...;          // Basic 级别间隔（~1 年区块数）
    type StandardDisclosureInterval = ...;       // Standard 级别间隔（~3 个月）
    type EnhancedDisclosureInterval = ...;       // Enhanced 级别间隔（~1 个月）
    type MajorHolderThreshold = ConstU32<500>;   // 大股东阈值（基点，500 = 5%）
    type MaxBlackoutDuration = ...;              // 黑窗口期最大时长
    type MaxAnnouncementHistory = ConstU32<200>; // 每实体最大公告数
    type MaxTitleLength = ConstU32<128>;         // 公告标题最大字节数
}
```

## 依赖

| Crate | 用途 |
|-------|------|
| `pallet-entity-common` | `EntityProvider` trait（`entity_owner`） |
| `frame-support` / `frame-system` | Substrate 框架 |
| `sp-runtime` | `Saturating`, `Zero` |

## 测试

```bash
cargo test -p pallet-entity-disclosure
```

覆盖：配置披露、发布披露、添加内幕人员、黑窗口期管理、公告发布/更新/撤回/置顶。73 个测试。

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-02-03 | Phase 6 初始版本：8 个 extrinsic、6 个 helper、13 种披露类型、5 种内幕角色 |
| v0.2.0 | 2026-02-26 | 公告发布功能：+4 extrinsic (8-11)、+4 storage、+5 event、+6 error、+2 helper、+2 Config 常量、+42 测试 |
