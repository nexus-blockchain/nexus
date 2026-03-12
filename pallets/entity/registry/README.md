# pallet-entity-registry

> Entity 组织层管理模块 — Entity-Shop 分离架构 | Runtime Index: 120

## 概述

`pallet-entity-registry` 是 Entity-Shop 分离架构的**组织层**模块，负责 Entity（组织实体）的完整生命周期管理。作为 `pallets/entity/` 子系统的基石，有 18 个下游 pallet 通过 `EntityProvider = EntityRegistry` 依赖本模块。

### 核心功能

- **Entity 创建** — 付费即激活，自动创建 Primary Shop
- **金库资金** — 50 USDT 等值 NEX 转入派生账户，支撑运营费用
- **管理员体系** — owner + admins 两层权限，admin 绑定权限位掩码（13 种细粒度权限）
- **类型升级** — Merchant → DAO/Enterprise/Community/Project 等
- **治理模式** — None/FullDAO/MultiSig/Council
- **官方认证** — 治理授权的 verified 标记
- **双层状态模型** — Entity 暂停/关闭自动抑制 Shop，反向不成立
- **三层暂停区分** — 治理暂停 / 资金不足暂停 / Owner 主动暂停，各有独立标记和恢复路径
- **推荐人系统** — Entity 可绑定推荐人，含反向索引
- **名称唯一性** — 规范化（小写 + trim）后索引，防止重名
- **关闭超时** — 关闭申请超时后任何人可触发自动关闭
- **治理锁定** — GovernanceProvider 检查全局锁定，锁定后所有配置操作被拒绝

## 架构

```
┌──────────────────────────────────────────────────────────────────┐
│                   pallet-entity-registry                         │
│                   (组织层 · pallet_index=120)                    │
├──────────────────────────────────────────────────────────────────┤
│  Entity                                                          │
│  ├── id, owner, name, logo_cid, description_cid, contact_cid   │
│  ├── entity_type (Merchant/DAO/Enterprise/...)                   │
│  ├── governance_mode (None/FullDAO/MultiSig/Council)               │
│  ├── admins: BoundedVec<(AccountId, u32), MaxAdmins>            │
│  ├── verified, metadata_uri                                      │
│  └── primary_shop_id: u64 (1:N 多店铺，0=未创建)                │
│                                                                  │
│  EntitySalesData (独立存储，O(1) 更新)                            │
│  └── total_sales, total_orders                                   │
├──────────────────────────────────────────────────────────────────┤
│  暂停标记                                                        │
│  ├── GovernanceSuspended  — 治理暂停（top_up 不可自动恢复）      │
│  ├── OwnerPaused          — Owner 主动暂停                       │
│  └── SuspensionReasons    — 治理暂停附带原因                     │
└──────────────────────────────────────────────────────────────────┘
         │                              │
         │ EntityProvider trait          │ 创建时调用 ShopProvider
         ▼                              ▼
┌─────────────────────┐    ┌─────────────────────────────────────┐
│  pallet-entity-shop │    │  pallet-entity-{member,token,...}   │
│  (业务层 · 129)     │    │  pallet-commission-core (127)       │
│  • Shop CRUD        │    │  pallet-entity-governance (130)     │
│  • 独立状态管理     │    │  pallet-entity-disclosure (131)     │
└─────────────────────┘    │  pallet-entity-sale (128)           │
                           │  pallet-entity-loyalty (139)        │
                           └─────────────────────────────────────┘
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
    Suspended,    // 暂停（治理/资金不足/Owner 主动）
    Banned,       // 封禁
    Closed,       // 已关闭
    PendingClose, // 待关闭审批
}
```

### 级联规则

| Entity 操作 | 对 Shop 的影响 | 机制 |
|---|---|---|
| `suspend_entity` / `self_pause_entity` | Shop 不可运营 | **查询时计算**（不物理写入 Shop） |
| `resume_entity` / `self_resume_entity` | Shop 恢复其原有独立状态 | **查询时计算** |
| `execute_close_timeout` / `ban_entity` | Shop 强制关闭 | **物理写入**（终态，不可逆） |
| Shop 暂停/关闭 | Entity **不受影响** | 无级联 |

**核心原则**：临时状态不级联写入，终态级联写入。

## 金库资金机制

创建 Entity 时，根据实时 NEX/USDT 价格计算 **50 USDT 等值的 NEX** 转入 **Entity 金库派生账户**。

```
地址: PalletId(*b"et/enty/").into_sub_account_truncating(entity_id)
公式: NEX = USDT × 10^12 / NEX价格, clamp(NEX, MinInitialFundCos, MaxInitialFundCos)
```

若价格过时（`is_price_stale()`），使用保守兜底值 `MinInitialFundCos`。

| 状态 | 条件 | 行为 |
|------|------|------|
| `Healthy` | 余额 > 预警阈值 | 正常运营 |
| `Warning` | 最低余额 < 余额 ≤ 预警阈值 | 发出 `FundWarning` 事件 |
| `Critical` | 0 < 余额 ≤ 最低余额 | 自动暂停 Entity |
| `Depleted` | 余额 = 0 | 资金耗尽 |

**资金规则**：不可提取 · 可充值(`top_up_fund`) · 可消费(`deduct_operating_fee`→`PlatformAccount`) · 关闭退还 · 封禁可没收

## Runtime 配置

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
| `MaxShopsPerEntity` | `u32` | 每个 Entity 最大 Shop 数 |
| `PlatformAccount` | `AccountId` | 平台账户（没收/运营费用接收方） |
| `GovernanceProvider` | `GovernanceProvider` | 治理查询提供者（全局实体锁定检查） |
| `CloseRequestTimeout` | `BlockNumber` | 关闭申请超时区块数 |
| `MaxReferralsPerReferrer` | `u32` | 每个推荐人最大推荐实体数量 |
| `OnEntityStatusChange` | `OnEntityStatusChange` | Entity 状态变更级联通知（暂停/封禁/关闭/恢复时通知下游模块） |
| `OrderProvider` | `OrderProvider` | 订单查询提供者（关闭前置检查） |
| `TokenSaleProvider` | `TokenSaleProvider` | 代币发售查询提供者（关闭前置检查） |
| `DisputeQueryProvider` | `DisputeQueryProvider` | 争议查询提供者（关闭前置检查） |
| `MarketProvider` | `MarketProvider` | 市场交易查询提供者（关闭前置检查） |
| `StoragePin` | `StoragePin` | IPFS Pin 管理接口（元数据 CID 持久化） |

### integrity_test 约束

- `MaxEntityNameLength`, `MaxCidLength`, `MaxAdmins`, `MaxEntitiesPerUser`, `MaxShopsPerEntity`, `MaxReferralsPerReferrer` 均 > 0
- `MinOperatingBalance ≤ FundWarningThreshold`
- `MinOperatingBalance ≤ MinInitialFundCos`
- `MinInitialFundCos ≤ MaxInitialFundCos`
- `CloseRequestTimeout > 0`（防止绕过治理审批）

## 数据结构

### Entity

```rust
pub struct Entity<AccountId, BlockNumber, MaxNameLen, MaxCidLen, MaxAdmins> {
    pub id: u64,
    pub owner: AccountId,
    pub name: BoundedVec<u8, MaxNameLen>,
    pub logo_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub description_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub status: EntityStatus,
    pub created_at: BlockNumber,
    pub entity_type: EntityType,                         // 默认 Merchant
    pub admins: BoundedVec<(AccountId, u32), MaxAdmins>, // (账户, 权限位掩码)
    pub governance_mode: GovernanceMode,                 // 默认 None
    pub verified: bool,
    pub metadata_uri: Option<BoundedVec<u8, MaxCidLen>>,
    pub contact_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub primary_shop_id: u64,                            // 0 = 未创建
}
```

### AdminPermission（权限位掩码，定义在 pallet-entity-common）

| 常量 | 位值 | 说明 |
|------|------|------|
| `SHOP_MANAGE` | `0x001` | Shop 管理 |
| `MEMBER_MANAGE` | `0x002` | 会员等级管理 |
| `TOKEN_MANAGE` | `0x004` | Token 发售管理 |
| `ADS_MANAGE` | `0x008` | 广告管理 |
| `REVIEW_MANAGE` | `0x010` | 评论管理 |
| `DISCLOSURE_MANAGE` | `0x020` | 披露/公告管理 |
| `ENTITY_MANAGE` | `0x040` | 实体信息管理 |
| `KYC_MANAGE` | `0x080` | KYC 要求管理 |
| `GOVERNANCE_MANAGE` | `0x100` | 治理提案管理 |
| `ORDER_MANAGE` | `0x200` | 订单管理 |
| `COMMISSION_MANAGE` | `0x400` | 佣金配置管理 |
| `PRODUCT_MANAGE` | `0x800` | 商品管理（定价、库存、上下架） |
| `MARKET_MANAGE` | `0x1000` | 市场/交易管理（挂单、交易对配置） |

Owner 天然拥有全部权限。`AdminPermission::is_valid(perm)` 检查是否仅包含已定义位。

## 存储项

| 存储 | 类型 | 说明 |
|------|------|------|
| `NextEntityId` | `StorageValue<u64>` | 自增 Entity ID（从 1 开始） |
| `Entities` | `StorageMap<u64, Entity>` | Entity 主数据 |
| `UserEntity` | `StorageMap<AccountId, BoundedVec<u64>>` | 用户→Entity 索引 |
| `EntityStats` | `StorageValue<EntityStatistics>` | 全局统计 |
| `EntityCloseRequests` | `StorageMap<u64, BlockNumber>` | 关闭申请时间戳 |
| `GovernanceSuspended` | `StorageMap<u64, bool>` | 治理暂停标记 |
| `OwnerPaused` | `StorageMap<u64, bool>` | Owner 主动暂停标记 |
| `EntityReferrer` | `StorageMap<u64, AccountId>` | Entity 推荐人 |
| `ReferrerEntities` | `StorageMap<AccountId, BoundedVec<u64>>` | 推荐人反向索引 |
| `EntitySales` | `StorageMap<u64, EntitySalesData>` | 销售统计（独立存储） |
| `EntityShops` | `StorageMap<u64, BoundedVec<u64>>` | Entity→Shop 索引（1:N） |
| `EntityNameIndex` | `StorageMap<BoundedVec<u8>, u64>` | 名称唯一性索引（normalized → id） |
| `SuspensionReasons` | `StorageMap<u64, BoundedVec<u8, 256>>` | 治理暂停原因 |

## Extrinsics

### 用户操作

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 0 | `create_entity(name, logo_cid, description_cid, referrer)` | signed | 创建 Entity + Primary Shop，付费即激活 |
| 1 | `update_entity(entity_id, name, logo_cid, description_cid, metadata_uri, contact_cid)` | owner/ENTITY_MANAGE | 更新实体信息 |
| 2 | `request_close_entity(entity_id)` | owner | 申请关闭（Active/Suspended → PendingClose） |
| 3 | `top_up_fund(entity_id, amount)` | owner/ENTITY_MANAGE | 充值金库，仅资金不足暂停可自动恢复 |
| 9 | `add_admin(entity_id, new_admin, permissions)` | owner | 添加管理员（指定权限位掩码） |
| 10 | `remove_admin(entity_id, admin)` | owner | 移除管理员 |
| 11 | `transfer_ownership(entity_id, new_owner)` | owner | 转移所有权 |
| 12 | `upgrade_entity_type(entity_id, new_type, new_governance)` | governance/owner | 类型升级 |
| 15 | `reopen_entity(entity_id)` | owner | 重开（Closed → Active，重新缴纳押金，付费即激活） |
| 16 | `bind_entity_referrer(entity_id, referrer)` | owner | 补绑推荐人（一次性） |
| 17 | `update_admin_permissions(entity_id, admin, new_permissions)` | owner | 更新管理员权限 |
| 20 | `cancel_close_request(entity_id)` | owner | 撤销关闭申请 |
| 21 | `resign_admin(entity_id)` | admin 自身 | 管理员主动辞职 |
| 22 | `set_primary_shop(entity_id, shop_id)` | owner/ENTITY_MANAGE | 设置 Primary Shop |
| 23 | `self_pause_entity(entity_id)` | owner | Owner 主动暂停实体 |
| 24 | `self_resume_entity(entity_id)` | owner | Owner 恢复主动暂停的实体 |

### 治理操作

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| ~~4~~ | ~~`approve_entity`~~ | — | 已移除（付费即激活，reopen/unban 直接 Active） |
| ~~5~~ | ~~`approve_close_entity`~~ | — | 已移除（关闭统一走超时机制 `execute_close_timeout`） |
| 6 | `suspend_entity(entity_id, reason)` | governance | 暂停（可附原因，不级联写入 Shop） |
| 7 | `resume_entity(entity_id)` | governance | 恢复（需资金充足） |
| 8 | `ban_entity(entity_id, confiscate_fund, reason)` | governance | 封禁，可没收资金，级联关闭 Shop |
| ~~13~~ | ~~`change_governance_mode`~~ | — | 已移除（统一由 governance pallet 管理） |
| 14 | `verify_entity(entity_id)` | governance | 官方认证 |
| 18 | `unban_entity(entity_id)` | governance | 解除封禁（Banned → Active，直接激活） |
| 19 | `unverify_entity(entity_id)` | governance | 撤销认证 |
| 25 | `force_transfer_ownership(entity_id, new_owner)` | governance | 强制转移所有权 |
| 26 | `reject_close_request(entity_id)` | governance | 拒绝关闭申请（PendingClose → 恢复） |

### 公共操作

| Index | 函数 | 权限 | 说明 |
|-------|------|------|------|
| 27 | `execute_close_timeout(entity_id)` | signed (任何人) | 执行超时关闭申请 |

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
| `ShopRemovedFromEntity { entity_id, shop_id }` | Shop 已移除 |
| `EntityUpdated { entity_id }` | 信息已更新 |
| `EntityStatusChanged { entity_id, status }` | 状态已变更 |
| `FundToppedUp { entity_id, amount, new_balance }` | 金库已充值 |
| `OperatingFeeDeducted { entity_id, fee, fee_type, remaining_balance }` | 运营费已扣除 |
| `FundWarning { entity_id, current_balance, warning_threshold }` | 资金预警 |
| `EntitySuspendedLowFund { entity_id, current_balance, minimum_balance }` | 资金不足暂停 |
| `EntityResumedAfterFunding { entity_id }` | 充值后恢复 |
| `EntityCloseRequested { entity_id }` | 申请关闭 |
| `EntityClosed { entity_id, fund_refunded }` | 已关闭（资金退还） |
| `EntityBanned { entity_id, fund_confiscated, reason }` | 已封禁（可附原因） |
| `EntityUnbanned { entity_id }` | 已解除封禁 |
| `FundConfiscated { entity_id, amount }` | 资金已没收 |
| `FundRefundFailed { entity_id, amount }` | 资金退还失败（需人工干预） |
| `AdminAdded { entity_id, admin, permissions }` | 管理员已添加（含权限） |
| `AdminRemoved { entity_id, admin }` | 管理员已移除 |
| `AdminPermissionsUpdated { entity_id, admin, old_permissions, new_permissions }` | 权限已更新 |
| `AdminResigned { entity_id, admin }` | 管理员辞职 |
| `EntityTypeUpgraded { entity_id, old_type, new_type }` | 类型已升级 |
| `GovernanceModeChanged { entity_id, old_mode, new_mode }` | 治理模式已变更 |
| `EntityVerified { entity_id }` | 已认证 |
| `EntityUnverified { entity_id }` | 认证已撤销 |
| `EntityReopened { entity_id, owner, initial_fund }` | 重新开业申请 |
| `OwnershipTransferred { entity_id, old_owner, new_owner }` | 所有权已转移 |
| `OwnershipForceTransferred { entity_id, old_owner, new_owner }` | 治理强制转移 |
| `EntityReferrerBound { entity_id, referrer }` | 推荐人已绑定 |
| `ShopCascadeFailed { entity_id, shop_id }` | Shop 级联操作失败（需人工干预） |
| `CloseRequestCancelled { entity_id }` | 关闭申请已撤销 |
| `CloseRequestRejected { entity_id }` | 治理拒绝关闭申请 |
| `CloseRequestAutoExecuted { entity_id, fund_refunded }` | 超时自动关闭 |
| `PrimaryShopChanged { entity_id, old_shop_id, new_shop_id }` | Primary Shop 已变更 |
| `EntityOwnerPaused { entity_id }` | Owner 主动暂停 |
| `EntityOwnerResumed { entity_id }` | Owner 主动恢复 |
| `EntitySuspendedWithReason { entity_id, reason }` | 治理暂停（附原因） |

## Errors

| 错误 | 说明 |
|------|------|
| `EntityNotFound` | 实体不存在 |
| `MaxEntitiesReached` | 用户实体数量已达上限 |
| `NotEntityOwner` | 不是实体所有者 |
| `InsufficientOperatingFund` | 运营资金不足 |
| `InvalidEntityStatus` | 无效的实体状态 |
| `NameEmpty` | 名称为空 |
| `NameTooLong` | 名称过长 |
| `InvalidName` | 名称内容无效（非 UTF-8 或含控制字符） |
| `NameAlreadyTaken` | 名称已被其他实体使用 |
| `CidTooLong` | CID 过长 |
| `PriceUnavailable` | 价格不可用 |
| `ArithmeticOverflow` | 算术溢出 |
| `InsufficientBalanceForInitialFund` | 余额不足以支付初始资金 |
| `NotAdmin` | 不是管理员 |
| `AdminAlreadyExists` | 管理员已存在 |
| `AdminNotFound` | 管理员不存在 |
| `MaxAdminsReached` | 管理员数量已达上限 |
| `CannotRemoveOwner` | 不能移除所有者 |
| `DAORequiresGovernance` | DAO 类型需要治理模式 |
| `InvalidEntityTypeUpgrade` | 无效的类型升级 |
| `ShopLimitReached` | Entity Shop 数量已达上限 |
| `ShopNotRegistered` | Shop 未注册在此 Entity |
| `ShopAlreadyRegistered` | Shop 已注册在此 Entity |
| `ShopNotInEntity` | Shop 不属于此 Entity |
| `AlreadyPrimaryShop` | 已是当前 Primary Shop |
| `ZeroAmount` | 充值金额为零 |
| `AlreadyVerified` | 实体已验证 |
| `NotVerified` | 实体未认证（无法撤销） |
| `EntityNotActive` | 实体状态不允许此操作 |
| `SameEntityType` | 类型未变化 |
| `ReferrerAlreadyBound` | 推荐人已绑定（不可更改） |
| `InvalidReferrer` | 无效推荐人（未拥有 Active Entity） |
| `SelfReferral` | 不能推荐自己 |
| `ReferrerIndexFull` | 推荐人反向索引已满 |
| `SameOwner` | 不能转移给自己 |
| `InvalidPermissions` | 无效权限值（不可为 0 或含未定义位） |
| `NotAdminCaller` | 调用者不是此实体的管理员 |
| `AlreadyOwnerPaused` | Entity 已被 Owner 暂停 |
| `NotOwnerPaused` | Entity 未被 Owner 暂停 |
| `EntityLocked` | 实体已被全局锁定（治理锁） |
| `CloseRequestNotExpired` | 关闭申请尚未超时 |
| `HasActiveProposals` | 存在活跃治理提案，不允许关闭 |
| `HasActiveOrders` | 存在未完成订单，不允许关闭 |
| `HasActiveDisputes` | 存在活跃争议，不允许关闭 |
| `HasActiveTokenSale` | 存在活跃代币发售，不允许关闭 |
| `HasActiveMarket` | 存在活跃市场交易，不允许关闭 |

## EntityProvider Trait

本模块实现 `EntityProvider` trait（定义在 `pallet-entity-common`），供下游 pallet 调用：

```rust
pub trait EntityProvider<AccountId> {
    // 查询
    fn entity_exists(entity_id: u64) -> bool;
    fn is_entity_active(entity_id: u64) -> bool;
    fn entity_status(entity_id: u64) -> Option<EntityStatus>;
    fn entity_owner(entity_id: u64) -> Option<AccountId>;
    fn entity_account(entity_id: u64) -> AccountId;
    fn entity_type(entity_id: u64) -> Option<EntityType>;
    fn entity_shops(entity_id: u64) -> Vec<u64>;
    fn entity_name(entity_id: u64) -> Vec<u8>;
    fn entity_metadata_cid(entity_id: u64) -> Option<Vec<u8>>;
    fn entity_description(entity_id: u64) -> Vec<u8>;
    // 权限
    fn is_entity_admin(entity_id: u64, account: &AccountId, required_permission: u32) -> bool;
    fn is_entity_locked(entity_id: u64) -> bool;
    // 写入
    fn update_entity_stats(entity_id: u64, sales_amount: u128, order_count: u32) -> DispatchResult;
    fn register_shop(entity_id: u64, shop_id: u64) -> DispatchResult;
    fn unregister_shop(entity_id: u64, shop_id: u64) -> DispatchResult;
    fn pause_entity(entity_id: u64) -> DispatchResult;
    fn resume_entity(entity_id: u64) -> DispatchResult;
    fn set_governance_mode(entity_id: u64, mode: GovernanceMode) -> DispatchResult;
}
```

## Runtime API

```rust
pub trait EntityRegistryApi<AccountId, Balance> {
    fn get_entity(entity_id: u64) -> Option<EntityInfo<AccountId, Balance>>;
    fn get_entities_by_owner(account: AccountId) -> Vec<u64>;
    fn get_entity_fund_info(entity_id: u64) -> Option<EntityFundInfo<Balance>>;
    fn get_verified_entities(offset: u32, limit: u32) -> Vec<u64>;
    fn get_entity_by_name(name: Vec<u8>) -> Option<u64>;
    fn get_entity_admins(entity_id: u64) -> Vec<(AccountId, u32)>;
    fn get_entity_suspension_reason(entity_id: u64) -> Option<Vec<u8>>;
    fn get_entity_sales(entity_id: u64) -> Option<(Balance, u64)>;
    fn get_entity_referrer(entity_id: u64) -> Option<AccountId>;
    fn get_referrer_entities(account: AccountId) -> Vec<u64>;
    fn get_entities_by_status(status: EntityStatus, offset: u32, limit: u32) -> Vec<u64>;
    fn check_admin_permission(entity_id: u64, account: AccountId) -> Option<u32>;
}
```

## 辅助函数

```rust
impl<T: Config> Pallet<T> {
    pub fn entity_treasury_account(entity_id: u64) -> T::AccountId;
    pub fn calculate_initial_fund() -> Result<BalanceOf<T>, DispatchError>;
    pub fn get_fund_health(balance: BalanceOf<T>) -> FundHealth;
    pub fn get_entity_fund_balance(entity_id: u64) -> BalanceOf<T>;
    pub fn deduct_operating_fee(entity_id, fee, fee_type) -> DispatchResult;
    pub fn get_current_initial_fund() -> Result<BalanceOf<T>, DispatchError>;
    pub fn get_initial_fund_details() -> (u64, u64, u128);
    pub fn normalize_entity_name(name: &[u8]) -> Result<BoundedVec<u8>, DispatchError>;
    pub fn has_permission(entity_id, who, required: u32) -> bool;
    pub fn is_admin(entity_id, who) -> bool;
    pub fn ensure_permission(entity_id, who, required: u32) -> DispatchResult;
    pub fn validate_entity_type_upgrade(current, new) -> DispatchResult;
    pub fn has_active_entity(account: &T::AccountId) -> bool;
    pub fn has_operable_entity(account: &T::AccountId) -> bool;
    pub fn ensure_entity_operable(status: &EntityStatus) -> DispatchResult;
    pub fn get_entity_type(entity_id) -> Option<EntityType>;
    pub fn get_governance_mode(entity_id) -> Option<GovernanceMode>;
    pub fn is_verified(entity_id) -> bool;
    pub fn get_admins(entity_id) -> Vec<(T::AccountId, u32)>;
}
```

## Entity 生命周期

```
路径 A: 新建（付费即激活）           路径 B: 重开（付费即激活）
═══════════════════════             ═══════════════════════
create_entity                       reopen_entity (Closed → Active)
  │ 缴纳 50 USDT 押金                │ 重新缴纳押金
  │ 自动创建 Primary Shop            │
  ▼                                  ▼
Active ◄─────────────────────────── Active（付费即激活，无需治理审批）
  │
  ├──► Suspended ◄── suspend_entity (治理) / self_pause_entity (Owner)
  │       │              / 资金不足 (deduct_operating_fee 自动触发)
  │       │
  │       ├── resume_entity (治理恢复)
  │       ├── self_resume_entity (Owner 恢复，仅 OwnerPaused)
  │       └── top_up_fund (资金恢复，仅非治理暂停/非 Owner 暂停)
  │
  ├──► PendingClose ──► Closed ──► 可走路径 B 重开
  │    │ (owner 申请)      (超时自动关闭，退还资金)
  │    ├── cancel_close_request (owner 撤销)
  │    └── reject_close_request (治理拒绝)
  │
  └──► Banned ──► unban_entity (治理) ──► Active（直接激活）
       (治理封禁，可没收资金，级联关闭 Shop)
```

### 两条激活路径

| | 路径 A: 新建 | 路径 B: 重开 |
|---|---|---|
| **入口** | `create_entity` | `reopen_entity` |
| **初始状态** | 直接 `Active` | 直接 `Active` |
| **信任锚点** | 押金 | 押金（付费即激活） |
| **Shop** | 自动创建 | 恢复已有 Shop |

## 安全机制

- **派生账户隔离** — `PalletId(*b"et/enty/")` + entity_id，每个 Entity 独立金库
- **资金不可提取** — 运营期间锁定，仅关闭退还或封禁没收
- **资金健康监控** — 低于阈值自动预警/暂停
- **每用户上限** — `MaxEntitiesPerUser`，防止刷 Entity
- **双层状态隔离** — Entity 暂停不物理修改 Shop，避免状态覆盖
- **治理暂停防绕过** — `GovernanceSuspended` 标记防止 `top_up_fund` 自动恢复治理暂停的实体
- **Owner 暂停隔离** — `OwnerPaused` 标记防止与治理暂停混淆
- **名称唯一性** — normalized 索引防重名，Banned/Closed 时释放名称
- **治理锁定** — `GovernanceProvider::is_governance_locked` 锁定后拒绝所有配置操作
- **NextEntityId 溢出保护** — 创建前检查 u64 溢出
- **关闭超时** — `CloseRequestTimeout > 0` 强制，防止绕过治理审批
- **权限位校验** — `AdminPermission::is_valid` 拒绝未定义位（add_admin/update_admin_permissions 双重校验）
- **状态变更通知** — `OnEntityStatusChange` 回调在暂停/封禁/关闭/恢复时通知下游模块（disclosure 等）
- **unban 资金检查** — `unban_entity` 检查金库余额 ≥ `MinOperatingBalance`，防止零资金激活
- **推荐人非终态校验** — 推荐人允许 Suspended/Pending 等非终态 Entity 的 owner，仅拒绝 Banned/Closed

## 存储迁移

**v0 → v1**（`StorageVersion::new(1)`）：
- 构建 `EntityNameIndex`（遍历所有非 Banned/Closed 实体，规范化名称后建立索引）
- `contact_cid` 字段默认 `None`，无需数据迁移

## 已知技术债

| 项目 | 状态 | 说明 |
|------|------|------|
| Runtime API 实现接线 | ✅ 已完成 | runtime_api.rs 已声明 12 个 API + helpers.rs 已实现，已在 runtime/src/apis.rs 的 `impl_runtime_apis!` 中接线 |
| Benchmark 权重 | ⏳ 待完成 | benchmarking.rs 已定义 25 个 benchmark，需运行生成真实权重替换 SubstrateWeight 手写值 |

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-01-31 | 从 pallet-mall 拆分 |
| v0.2.0 | 2026-02-01 | 实现 USDT 等值 NEX 金库机制 |
| v0.3.0 | 2026-02-03 | 重构为 Entity，支持多种实体类型和治理模式 |
| v0.4.0 | 2026-02-05 | Entity-Shop 分离架构，1:1 关联，Primary Shop |
| v0.5.0 | 2026-02-07 | 多实体支持，UserEntity BoundedVec，1:N 多店铺 |
| v0.6.0 | 2026-02-08 | 双层状态模型，EffectiveShopStatus 实时计算 |
| v0.6.1 | 2026-02-08 | 移除 treasury_fund 字段，治理可任意升级类型，reopen 防御性去重 |
| v0.6.2 | 2026-02-08 | 安全审计: ban 状态限制、GovernanceSuspended 防绕过、Banned/Closed 拒绝修改 |
| v0.7.0 | 2026-02-09 | 深度审计: DecodeWithMemTracking、Weight 修正、ban 没收事件修复、admin/transfer 状态检查、verify_entity 幂等、新增 mock.rs+tests.rs |
| v0.8.0 | 2026-02-26 | Entity 推荐人（招商）: referrer 参数、bind_entity_referrer、EntityReferrer 存储、has_active_entity |
| v0.9.0 | 2026-03-05 | 深度审计 Round 5: NextEntityId 溢出保护、approve 清除 GovernanceSuspended 遗留、metadata_uri 空值转 None |
| v1.0.0 | — | Phase 5+6: OwnerPaused/self_pause/self_resume、set_primary_shop、cancel_close_request、resign_admin、force_transfer_ownership、reject_close_request、execute_close_timeout、unban_entity、unverify_entity、update_admin_permissions、EntityNameIndex 唯一性索引、SuspensionReasons、ReferrerEntities 反向索引、EntitySales 独立存储、contact_cid 字段、GovernanceProvider 治理锁定、CloseRequestTimeout 超时机制 |
| v1.1.0 | 2026-03-06 | 上线前审查: P0-AdminPermission::is_valid 校验、P0-OnEntityStatusChange 回调接入（暂停/封禁/关闭/恢复 10 处调用点）、P1-Runtime API 补充（名称查询/admin 权限/暂停原因/销售统计/推荐关系/按状态过滤 共 8 个新接口）、P1-approve_entity 金库余额检查（防 unban 后零资金激活）、P1-关闭逻辑提取 do_finalize_close（消除 90% 重复代码）、P2-推荐人校验放宽（非终态即可）、P2-ensure_entity_operable 统一状态前置检查、P2-force_transfer_ownership 设计决策文档化、P3-is_admin 语义文档化 |
| v1.2.0 | 2026-03-07 | P0-ban 保留 EntityShops 关联列表（修复 unban 后 Shop 无法恢复的 bug）、P0-close 保留 EntityShops 关联列表（修复 reopen 后 Shop 无法恢复的 bug）、P2-移除废弃的 RuntimeEvent 关联类型（polkadot-sdk PR #7229） |
| v1.3.0 | 2026-03-12 | 移除 approve_entity/approve_close_entity（付费即激活 + 超时关闭统一）、新增关闭前置检查 Provider（OrderProvider/TokenSaleProvider/DisputeQueryProvider/MarketProvider）、AdminPermission 新增 PRODUCT_MANAGE + MARKET_MANAGE（13 权限位）、GovernanceMode 更新为 None/FullDAO/MultiSig/Council、StoragePin 集成 |

## 相关模块

- [pallet-entity-common](../common/) — 共享类型 + Trait 接口（EntityProvider, AdminPermission 等）
- [pallet-entity-shop](../shop/) — Shop 业务层管理
- [pallet-entity-member](../member/) — 会员体系
- [pallet-entity-token](../token/) — 实体代币
- [pallet-entity-governance](../governance/) — 治理模块
- [pallet-entity-loyalty](../loyalty/) — 积分与购物余额
- [pallet-commission-core](../../commission/core/) — 佣金核心
