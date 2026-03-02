# pallet-entity-registry

> Entity 组织层管理模块 — Entity-Shop 分离架构 | Runtime Index: 120

## 概述

`pallet-entity-registry` 是 Entity-Shop 分离架构的**组织层**模块，负责 Entity（组织实体）的完整生命周期管理。作为 `pallets/entity/` 子系统的基石，有 9 个下游 pallet 通过 `EntityProvider = EntityRegistry` 依赖本模块。

### 核心功能

- **Entity 创建** — 付费即激活，自动创建 Primary Shop
- **金库资金** — 50 USDT 等值 NEX 转入派生账户，支撑运营费用
- **管理员体系** — owner + admins 两层权限，admin 继承到 Shop 管理
- **类型升级** — Merchant → DAO/Enterprise/Community/Project 等
- **治理模式** — None/Advisory/DualTrack/Committee/FullDAO/Tiered
- **官方认证** — 治理授权的 verified 标记
- **双层状态模型** — Entity 暂停/关闭自动抑制 Shop，反向不成立

## 架构

```
┌─────────────────────────────────────────────────────────────────┐
│                   pallet-entity-registry                        │
│                   (组织层 · pallet_index=120)                   │
├─────────────────────────────────────────────────────────────────┤
│  Entity                                                         │
│  ├── id, owner, name, logo_cid, description_cid                │
│  ├── entity_type (Merchant/DAO/Enterprise/...)                  │
│  ├── governance_mode (None/Advisory/DualTrack/...)              │
│  ├── admins: BoundedVec<AccountId, MaxAdmins>                  │
│  ├── verified, metadata_uri                                     │
│  ├── shop_id: u64  ◄── 1:1 关联                                │
│  └── total_sales, total_orders (汇总统计)                       │
├─────────────────────────────────────────────────────────────────┤
│  双层状态模型                                                    │
│                                                                 │
│  EntityStatus ──┐                                               │
│  (组织层独立)    ├──► EffectiveShopStatus (实时计算，不存储)     │
│  ShopOperating ─┘    compute(entity_status, shop_status)        │
│  Status                                                         │
│  (业务层独立)    Entity↓影响Shop  Shop↑不影响Entity             │
└─────────────────────────────────────────────────────────────────┘
         │                              │
         │ EntityProvider trait          │ 创建时调用 ShopProvider
         ▼                              ▼
┌─────────────────────┐    ┌─────────────────────────────────────┐
│  pallet-entity-shop │    │  pallet-entity-{member,token,...}   │
│  (业务层 · 129)     │    │  pallet-commission-core (127)       │
│  • Shop CRUD        │    │  pallet-entity-governance (130)     │
│  • 运营资金         │    │  pallet-entity-disclosure (131)     │
│  • 独立状态管理     │    │  pallet-entity-sale (128)           │
└─────────────────────┘    └─────────────────────────────────────┘
```

## 双层状态模型

Entity 和 Shop 拥有**独立的状态枚举**，Shop 的有效状态在查询时实时计算：

```
Shop 有效状态 = EffectiveShopStatus::compute(EntityStatus, ShopOperatingStatus)
```

### EntityStatus（组织层）

```rust
pub enum EntityStatus {
    Pending,      // 待审核（reopen 后等待治理审批）
    Active,       // 正常运营
    Suspended,    // 暂停（治理/资金不足）
    Banned,       // 封禁
    Closed,       // 已关闭
    PendingClose, // 待关闭审批
}
```

### EffectiveShopStatus（实时计算）

```rust
pub enum EffectiveShopStatus {
    Active,         // 正常营业（Entity Active + Shop Active）
    PausedBySelf,   // Shop 自身暂停
    PausedByEntity, // Entity 非 Active 导致不可运营
    FundDepleted,   // Shop 资金耗尽
    Closed,         // Shop 自身关闭
    ClosedByEntity, // Entity 关闭/封禁，Shop 强制关闭
    Pending,        // 待激活
}
```

### 级联规则

| Entity 操作 | 对 Shop 的影响 | 机制 |
|---|---|---|
| `suspend_entity` | Shop 不可运营 | **查询时计算**（不物理写入 Shop） |
| `resume_entity` | Shop 恢复其原有独立状态 | **查询时计算**（不物理写入 Shop） |
| `approve_close_entity` | Shop 强制关闭 | **物理写入**（终态，不可逆） |
| `ban_entity` | Shop 强制关闭 | **物理写入**（终态，不可逆） |
| Shop 暂停/关闭 | Entity **不受影响** | 无级联 |

**核心原则**：临时状态（Suspend/Resume）不级联写入，终态（Close/Ban）级联写入。

### 场景示例

```
T1: Entity=Active,  Shop=Active   → 有效状态: Active         ✅
T2: Entity=Active,  Shop=Paused   → 有效状态: PausedBySelf   ❌ manager 暂停
T3: Entity=Suspend, Shop=Paused   → 有效状态: PausedByEntity ❌ Entity 暂停
T4: Entity=Active,  Shop=Paused   → 有效状态: PausedBySelf   ❌ Entity 恢复，Shop 仍暂停 ✅
T5: Entity=Active,  Shop=Active   → 有效状态: Active         ✅ manager 恢复
```

## 金库资金机制

创建 Entity 时，根据实时 NEX/USDT 价格计算 **50 USDT 等值的 NEX** 转入 **Entity 金库派生账户**。

```
地址: PalletId(*b"et/enty/").into_sub_account_truncating(entity_id)
```

### 计算公式

```
NEX 金额 = USDT 金额 × 10^12 / NEX价格
final_fund = clamp(COS金额, MinInitialFundCos, MaxInitialFundCos)
```

### 资金健康状态

| 状态 | 条件 | 行为 |
|------|------|------|
| `Healthy` | 余额 > 预警阈值 | 正常运营 |
| `Warning` | 最低余额 < 余额 ≤ 预警阈值 | 发出 `FundWarning` 事件 |
| `Critical` | 余额 ≤ 最低余额 | 自动暂停 Entity |
| `Depleted` | 余额 = 0 | 资金耗尽 |

### 资金规则

- **不可提取** — 运营期间锁定在派生账户
- **可充值** — owner 随时通过 `top_up_fund` 充值
- **可消费** — 其他模块通过 `deduct_operating_fee` 扣费（IPFS Pin、存储租金等）
- **关闭退还** — 治理审批关闭后全额退还 owner
- **封禁没收** — 可选没收至平台账户

## Runtime 配置

```rust
impl pallet_entity_registry::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MaxEntityNameLength = ConstU32<64>;
    type MaxCidLength = ConstU32<64>;
    type GovernanceOrigin = EnsureRoot<AccountId>;
    type PricingProvider = EntityPricingProvider;
    type InitialFundUsdt = ConstU64<50_000_000>;     // 50 USDT
    type MinInitialFundCos = EntityMinDeposit;
    type MaxInitialFundCos = ConstU128<{ 1000 * UNIT }>;
    type MinOperatingBalance = ConstU128<{ UNIT / 10 }>;
    type FundWarningThreshold = ConstU128<{ UNIT }>;
    type MaxAdmins = ConstU32<10>;
    type MaxEntitiesPerUser = ConstU32<3>;
    type ShopProvider = EntityShop;
    type PlatformAccount = EntityPlatformAccount;
}
```

### 配置参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `Currency` | `Currency` | 货币类型 |
| `MaxEntityNameLength` | `u32` | 名称最大长度 |
| `MaxCidLength` | `u32` | IPFS CID 最大长度 |
| `GovernanceOrigin` | `EnsureOrigin` | 治理 Origin |
| `PricingProvider` | `PricingProvider` | NEX/USDT 定价接口 |
| `InitialFundUsdt` | `u64` | 初始资金 USDT（精度 10^6） |
| `MinInitialFundCos` | `Balance` | 最小初始资金 NEX |
| `MaxInitialFundCos` | `Balance` | 最大初始资金 NEX |
| `MinOperatingBalance` | `Balance` | 最低运营余额（低于自动暂停） |
| `FundWarningThreshold` | `Balance` | 资金预警阈值 |
| `MaxAdmins` | `u32` | 每个 Entity 最大管理员数 |
| `MaxEntitiesPerUser` | `u32` | 每个用户最大 Entity 数 |
| `ShopProvider` | `ShopProvider` | Shop 模块（创建 Primary Shop） |
| `PlatformAccount` | `AccountId` | 平台账户（没收/运营费用接收方） |

## 数据结构

### Entity

```rust
pub struct Entity<AccountId, Balance, BlockNumber, MaxNameLen, MaxCidLen, MaxAdmins> {
    pub id: u64,                                      // Entity ID
    pub owner: AccountId,                             // 所有者
    pub name: BoundedVec<u8, MaxNameLen>,             // 名称
    pub logo_cid: Option<BoundedVec<u8, MaxCidLen>>,  // Logo IPFS CID
    pub description_cid: Option<BoundedVec<u8, MaxCidLen>>, // 描述 CID
    pub status: EntityStatus,                         // 实体状态
    pub created_at: BlockNumber,                      // 创建区块
    pub entity_type: EntityType,                      // 类型（默认 Merchant）
    pub admins: BoundedVec<AccountId, MaxAdmins>,     // 管理员列表
    pub governance_mode: GovernanceMode,              // 治理模式（默认 None）
    pub verified: bool,                               // 官方认证
    pub metadata_uri: Option<BoundedVec<u8, MaxCidLen>>, // 元数据 URI
    pub primary_shop_id: u64,                          // Primary Shop ID（0=未创建）
    pub total_sales: Balance,                         // 累计销售额
    pub total_orders: u64,                            // 累计订单数
}
```

### 存储项

| 存储 | 类型 | 说明 |
|------|------|------|
| `NextEntityId` | `StorageValue<u64>` | 自增 Entity ID |
| `Entities` | `StorageMap<u64, Entity>` | Entity 主数据 |
| `UserEntity` | `StorageMap<AccountId, BoundedVec<u64, 3>>` | 用户→Entity 索引 |
| `EntityStats` | `StorageValue<EntityStatistics>` | 全局统计 |
| `EntityCloseRequests` | `StorageMap<u64, BlockNumber>` | 关闭申请时间 |
| `GovernanceSuspended` | `StorageMap<u64, bool>` | 治理暂停标记（区分治理 vs 资金不足暂停） |
| `EntityShops` | `StorageMap<u64, BoundedVec<u64, MaxShopsPerEntity>>` | Entity→Shop 关联索引 |
| `EntityReferrer` | `StorageMap<u64, AccountId>` | Entity 推荐人（招商推荐关系） |
| `EntityReferralCount` | `StorageMap<AccountId, u32>` | 账户招商统计（推荐的 Entity 数量） |

## Extrinsics

### 用户操作

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 0 | `create_entity(name, logo_cid, description_cid, referrer)` | signed | 创建 Entity + 自动创建 Primary Shop，付费即激活，可选推荐人 |
| 1 | `update_entity(entity_id, name, logo_cid, description_cid, metadata_uri)` | owner | 更新名称/Logo/描述/元数据 URI |
| 2 | `request_close_entity(entity_id)` | owner | 申请关闭（Active/Suspended → PendingClose） |
| 3 | `top_up_fund(entity_id, amount)` | owner | 充值金库，仅资金不足暂停可自动恢复（治理暂停不可） |
| 9 | `add_admin(entity_id, new_admin)` | owner | 添加管理员 |
| 10 | `remove_admin(entity_id, admin)` | owner | 移除管理员 |
| 11 | `transfer_ownership(entity_id, new_owner)` | owner | 转移所有权 |
| 15 | `reopen_entity(entity_id)` | owner | 重新开业（Closed → Pending，需重新缴纳押金） |
| 16 | `bind_entity_referrer(entity_id, referrer)` | owner | 补绑推荐人（一次性，Entity 须 Active，推荐人须拥有 Active Entity） |

### 治理操作

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 4 | `approve_entity(entity_id)` | governance | 审批激活 Pending 实体 |
| 5 | `approve_close_entity(entity_id)` | governance | 审批关闭，退还资金，级联关闭 Shop |
| 6 | `suspend_entity(entity_id)` | governance | 暂停（不级联写入 Shop） |
| 7 | `resume_entity(entity_id)` | governance | 恢复（需资金充足，不级联写入 Shop） |
| 8 | `ban_entity(entity_id, confiscate_fund)` | governance | 封禁（仅 Active/Suspended/PendingClose），可没收资金，级联关闭 Shop |
| 12 | `upgrade_entity_type(entity_id, new_type, new_governance)` | governance/owner | 类型升级（治理可任意升级，owner 受路径限制） |
| ~~13~~ | ~~`change_governance_mode`~~ | — | 已移除（统一由 `pallet-entity-governance::configure_governance` 管理） |
| 14 | `verify_entity(entity_id)` | governance | 官方认证 |

### 类型升级规则

| 当前类型 | Owner 可升级为 | 治理可升级为 |
|----------|---------|---------|
| Merchant | 任何类型 | 任何类型 |
| Community | DAO | 任何类型 |
| Project | DAO, Enterprise | 任何类型 |
| DAO / Enterprise / 其他 | ❌ 不可 | 任何类型 |

## Events

| 事件 | 说明 |
|------|------|
| `EntityCreated { entity_id, owner, treasury_account, initial_fund }` | Entity 已创建 |
| `ShopAddedToEntity { entity_id, shop_id }` | Shop 已关联 |
| `EntityUpdated { entity_id }` | 信息已更新 |
| `EntityStatusChanged { entity_id, status }` | 状态已变更 |
| `FundToppedUp { entity_id, amount, new_balance }` | 金库已充值 |
| `OperatingFeeDeducted { entity_id, fee, fee_type, remaining_balance }` | 运营费已扣除 |
| `FundWarning { entity_id, current_balance, warning_threshold }` | 资金预警 |
| `EntitySuspendedLowFund { entity_id, current_balance, minimum_balance }` | 资金不足暂停 |
| `EntityResumedAfterFunding { entity_id }` | 充值后恢复 |
| `EntityCloseRequested { entity_id }` | 申请关闭 |
| `EntityClosed { entity_id, fund_refunded }` | 已关闭（资金退还） |
| `EntityBanned { entity_id, fund_confiscated }` | 已封禁 |
| `FundConfiscated { entity_id, amount }` | 资金已没收 |
| `AdminAdded { entity_id, admin }` | 管理员已添加 |
| `AdminRemoved { entity_id, admin }` | 管理员已移除 |
| `EntityTypeUpgraded { entity_id, old_type, new_type }` | 类型已升级 |
| `GovernanceModeChanged { entity_id, old_mode, new_mode }` | 治理模式已变更 |
| `EntityVerified { entity_id }` | 已认证 |
| `EntityReopened { entity_id, owner, initial_fund }` | 重新开业申请 |
| `OwnershipTransferred { entity_id, old_owner, new_owner }` | 所有权已转移 |
| `EntityReferrerBound { entity_id, referrer }` | Entity 推荐人已绑定 |

## Errors

| 错误 | 说明 |
|------|------|
| `EntityNotFound` | 实体不存在 |
| `MaxEntitiesReached` | 用户实体数量已达上限（3） |
| `NotEntityOwner` | 不是实体所有者 |
| `InsufficientOperatingFund` | 运营资金不足 |
| `InvalidEntityStatus` | 无效的实体状态（Banned/Closed 实体拒绝修改操作） |
| `NameEmpty` | 名称为空 |
| `NameTooLong` | 名称过长 |
| `CidTooLong` | CID 过长 |
| `PriceUnavailable` | 价格不可用 |
| `ArithmeticOverflow` | 算术溢出 |
| `InsufficientBalanceForInitialFund` | 余额不足以支付初始资金 |
| `NotAdmin` | 不是管理员 |
| `AdminAlreadyExists` | 管理员已存在 |
| `AdminNotFound` | 管理员不存在 |
| `MaxAdminsReached` | 管理员数量已达上限（10） |
| `CannotRemoveOwner` | 不能移除所有者 |
| `DAORequiresGovernance` | DAO 类型需要治理模式 |
| `InvalidEntityTypeUpgrade` | 无效的类型升级 |
| `ZeroAmount` | 充值金额为零 |
| `AlreadyVerified` | 实体已验证（幂等拦截） |
| `ShopAlreadyRegistered` | Shop 已注册在此 Entity |
| `EntityNotActive` | 实体状态不允许此操作 |
| `SameEntityType` | 类型未变化 |
| `SameGovernanceMode` | 治理模式未变化 |
| `ReferrerAlreadyBound` | 推荐人已绑定（不可更改） |
| `InvalidReferrer` | 无效推荐人（推荐人未拥有 Active Entity） |
| `SelfReferral` | 不能推荐自己 |

## EntityProvider Trait

本模块实现 `EntityProvider` trait，供 9 个下游 pallet 调用：

```rust
pub trait EntityProvider<AccountId> {
    fn entity_exists(entity_id: u64) -> bool;
    fn is_entity_active(entity_id: u64) -> bool;
    fn entity_status(entity_id: u64) -> Option<EntityStatus>;
    fn entity_owner(entity_id: u64) -> Option<AccountId>;
    fn entity_account(entity_id: u64) -> AccountId;
    fn update_entity_stats(entity_id, sales_amount, order_count) -> DispatchResult;
    fn update_entity_rating(entity_id, rating) -> DispatchResult;
    fn register_shop(entity_id, shop_id) -> DispatchResult;
    fn unregister_shop(entity_id, shop_id) -> DispatchResult;
    fn is_entity_admin(entity_id, account) -> bool;
    fn entity_shops(entity_id) -> Vec<u64>;
}
```

## 辅助函数

```rust
impl<T: Config> Pallet<T> {
    /// 获取 Entity 金库派生账户
    pub fn entity_treasury_account(entity_id: u64) -> T::AccountId;
    /// 计算初始运营资金（USDT 等值 NEX）
    pub fn calculate_initial_fund() -> Result<BalanceOf<T>, DispatchError>;
    /// 获取资金健康状态
    pub fn get_fund_health(balance: BalanceOf<T>) -> FundHealth;
    /// 获取金库资金余额
    pub fn get_entity_fund_balance(entity_id: u64) -> BalanceOf<T>;
    /// 扣除运营费用（供其他模块调用）
    pub fn deduct_operating_fee(entity_id, fee, fee_type) -> DispatchResult;
    /// 获取当前初始资金金额（供前端查询）
    pub fn get_current_initial_fund() -> Result<BalanceOf<T>, DispatchError>;
    /// 获取初始资金计算详情
    pub fn get_initial_fund_details() -> (u64, u64, u128);
    /// 检查是否是管理员（owner 或 admins）
    pub fn is_admin(entity_id: u64, who: &T::AccountId) -> bool;
    /// 确保调用者是管理员
    pub fn ensure_admin(entity_id: u64, who: &T::AccountId) -> DispatchResult;
    /// 验证实体类型升级规则
    pub fn validate_entity_type_upgrade(current, new) -> DispatchResult;
    /// 检查账户是否拥有至少一个 Active Entity
    pub fn has_active_entity(account: &T::AccountId) -> bool;
}
```

## Entity 生命周期

```
路径 A: 新建（付费即激活）           路径 B: 重开（需治理审批）
═══════════════════════             ═══════════════════════
create_entity                       reopen_entity (Closed → Pending)
  │ 缴纳 50 USDT 押金                │ 重新缴纳押金
  │ 自动创建 Primary Shop            │
  ▼                                  ▼
Active ◄─────────────────────────── approve_entity (Pending → Active)
  │    │                             │ 同时 resume_shop 恢复 Shop
  │    │                             │
  │    ▼              ▲
  │  Suspended ───► top_up_fund()（充值恢复）
  │  (治理暂停/资金不足)
  │
  ├──► PendingClose ──► Closed ──► 可走路径 B 重开
  │    (owner 申请)      (治理审批，退还资金)
  │
  └──► Banned (治理封禁，可没收资金)
```

### 两条激活路径的设计意图

| | 路径 A: 新建 | 路径 B: 重开 |
|---|---|---|
| **入口** | `create_entity` (call_index 0) | `reopen_entity` (call_index 15) → `approve_entity` (call_index 4) |
| **初始状态** | 直接 `Active` | `Pending` → 治理审批后 `Active` |
| **信任锚点** | 押金（经济担保） | 押金 + 治理审批（双重担保） |
| **适用场景** | 无历史记录的新 Entity | 曾被关闭/封禁，有风险记录 |
| **Shop 创建** | `create_entity` 内自动创建 | `approve_entity` 内恢复已有 Shop |

> **注意**：`approve_entity` 当前**仅**由 `reopen_entity` 产生的 `Pending` 状态触发。
> `create_entity` 跳过 `Pending` 直接进入 `Active`。

### 未来扩展：新建审批模式

若需要新建 Entity 也经过审批流程，需修改以下三处：

1. `create_entity` 初始状态改为 `EntityStatus::Pending`
2. `active_entities` 统计递增延后到 `approve_entity`
3. Primary Shop 创建延后到 `approve_entity`（避免 Pending 期间产生可运营的 Shop）

## 安全机制

- **派生账户隔离** — `PalletId(*b"et/enty/")` + entity_id，每个 Entity 独立金库
- **资金不可提取** — 运营期间锁定，仅关闭退还或封禁没收
- **资金健康监控** — 低于阈值自动预警/暂停
- **每用户上限** — `MaxEntitiesPerUser = 3`，防止刷 Entity
- **双层状态隔离** — Entity 暂停不物理修改 Shop，避免状态覆盖

## 已知技术债

| 项目 | 状态 | 说明 |
|------|------|------|
| Weight benchmarking | � 占位 | 所有 extrinsic 使用硬编码占位值（60M~200M ref_time, 4k~10k proof_size），生产前需基于 `frame_benchmarking` 重新计算 |

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-01-31 | 从 pallet-mall 拆分 |
| v0.2.0 | 2026-02-01 | 实现 USDT 等值 NEX 金库机制 |
| v0.3.0 | 2026-02-03 | 重构为 Entity，支持多种实体类型和治理模式 |
| v0.4.0 | 2026-02-05 | Entity-Shop 分离架构，1:1 关联，Primary Shop |
| v0.5.0 | 2026-02-07 | 多实体支持，UserEntity BoundedVec |
| v0.6.0 | 2026-02-08 | 双层状态模型，EffectiveShopStatus 实时计算 |
| v0.6.1 | 2026-02-08 | 移除 treasury_fund 字段，治理可任意升级类型，reopen 防御性去重 |
| v0.6.2 | 2026-02-08 | 安全审计修复：ban 状态限制、GovernanceSuspended 防绕过、Banned/Closed 拒绝修改、NameEmpty 错误、清理未用代码 |
| v0.7.0 | 2026-02-09 | 深度审计: DecodeWithMemTracking、Weight 修正、ban 没收事件修复、admin/transfer 状态检查、top_up_fund 零值拦截、update_entity 签名变更(metadata_uri 替换 customer_service)、unregister_shop 验证、verify_entity 幂等、新增 mock.rs+tests.rs (74 tests) |
| v0.8.0 | 2026-02-26 | Entity 推荐人（招商）: create_entity 增加 referrer 参数、bind_entity_referrer extrinsic (call_index 16)、EntityReferrer/EntityReferralCount 存储、has_active_entity 辅助函数 (105 tests) |
| v0.9.0 | 2026-03-05 | 深度审计 Round 5: H1 NextEntityId 溢出保护、H2 approve_entity/approve_close_entity 清除 GovernanceSuspended 遗留标记、M1 metadata_uri 空值转 None、M2 set_governance_mode 拒绝 Banned/Closed、L3 README 补充 EntityShops 存储项 (114 tests) |

## 相关模块

- [pallet-entity-common](../common/) — 共享类型 + Trait 接口
- [pallet-entity-shop](../shop/) — Shop 业务层管理
- [pallet-entity-member](../member/) — 会员体系
- [pallet-entity-token](../token/) — 实体代币
- [pallet-entity-governance](../governance/) — 治理模块
- [pallet-commission-core](../../commission/core/) — 佣金核心
