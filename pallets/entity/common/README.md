# pallet-entity-common

> Entity 模块集群公共类型、Trait 接口和工具库

## 概述

`pallet-entity-common` 是 Entity 子系统的基础依赖，定义了所有 Entity 子模块共享的类型枚举、跨模块 Port/Provider trait 接口、共享错误码和分页工具。

### 特性

- **纯 Rust crate** -- 无链上存储，无 pallet 宏，仅提供类型和 Trait
- **no_std 兼容** -- 支持 WebAssembly 运行时
- **跨模块共享** -- 被 registry、shop、token、member、commission、governance、market、order、review、disclosure、kyc、sale、loyalty、product 等 15+ 个 pallet 引用
- **分层目录结构** -- types/、traits/、errors.rs、pagination.rs 按职责分文件组织

## 模块结构

```
common/src/
    lib.rs            -- 入口：声明子模块 + 全量 re-export
    types/
        mod.rs        -- 领域枚举、DTO、位掩码（EntityType, ShopType, OrderStatus, AdminPermission 等）
    traits/
        mod.rs        -- 跨模块 Port/Provider trait、Null 实现、blanket impl
    errors.rs         -- CommonError 共享错误字符串常量
    pagination.rs     -- PageRequest / PageResponse 标准分页类型
    tests.rs          -- 单元测试
```

所有子模块通过 `pub use` 全量导出，外部 import 路径保持不变：

```rust
use pallet_entity_common::{EntityType, ShopProvider, CommonError, PageRequest};
```

## 核心类型 (types/mod.rs)

### EntityType -- 实体类型

```rust
pub enum EntityType {
    Merchant,         // 商户（默认）
    Enterprise,       // 企业
    DAO,              // 去中心化自治组织
    Community,        // 社区
    Project,          // 项目方
    ServiceProvider,  // 服务提供商
    Fund,             // 基金
    #[deprecated]
    Custom(u8),       // 自定义类型（已废弃：无验证机制，回退到最宽松默认值）
}
```

**辅助方法：**

| 方法 | 说明 |
|------|------|
| `default_governance()` | 返回推荐治理模式（DAO->FullDAO, Enterprise->MultiSig, Fund/Community->Council） |
| `default_token_type()` | 返回推荐代币类型（DAO->Governance, Fund->Share, Enterprise->Equity） |
| `requires_kyc_by_default()` | Enterprise/Fund/Project 默认需要 KYC |
| `suggests_token_type(token)` | 检查代币类型是否为推荐组合 |
| `suggests_governance(mode)` | 检查治理模式是否为推荐组合 |
| `default_transfer_restriction()` | 返回默认转账限制模式 |

### GovernanceMode -- 治理模式

```rust
pub enum GovernanceMode {
    None,       // 无治理（管理员全权控制，默认）
    FullDAO,    // 完全 DAO（所有决策需代币投票）
    MultiSig,   // 多签治理（N-of-M 签名者，适合 Enterprise）
    Council,     // 理事会治理（选举/任命理事会成员投票，适合 Fund/Community）
}
```

### EntityStatus -- 实体状态

```rust
pub enum EntityStatus {
    Pending,      // 待审核（Default，reopen_entity 使用；create_entity 跳过直接 Active）
    Active,       // 正常运营
    Suspended,    // 暂停运营（管理员主动）
    Banned,       // 被封禁（治理处罚，终态）
    Closed,       // 已关闭（终态）
    PendingClose, // 待关闭审批
}
```

**辅助方法：** `is_active()`, `is_terminal()`, `is_pending()`

### TokenType -- 通证类型

```rust
pub enum TokenType {
    Points,       // 积分（默认，消费奖励）
    Governance,   // 治理代币（投票权）
    Equity,       // 股权代币（分红权，需 Enhanced KYC）
    Membership,   // 会员代币（身份凭证，不可转让）
    Share,        // 份额代币（基金份额）
    Bond,         // 债券代币（固定收益）
    Hybrid,       // 混合型（投票权 + 分红权）
}
```

**辅助方法：**

| 方法 | 说明 |
|------|------|
| `has_voting_power()` | Governance/Equity/Hybrid 具有投票权 |
| `has_dividend_rights()` | Equity/Share/Hybrid 具有分红权 |
| `is_transferable_by_default()` | Membership 默认不可转让 |
| `required_kyc_level()` | 返回 (持有者 KYC, 接收方 KYC) 级别 |
| `is_security()` | Equity/Share/Bond 为证券类 |
| `requires_disclosure()` | 证券类需要强制披露 |
| `default_transfer_restriction()` | 默认转账限制模式 |

### TransferRestrictionMode -- 转账限制

```rust
pub enum TransferRestrictionMode {
    None,         // 无限制（默认）
    Whitelist,    // 白名单模式
    Blacklist,    // 黑名单模式
    KycRequired,  // KYC 模式
    MembersOnly,  // 闭环模式（仅实体成员间）
}
```

**辅助方法：** `try_from_u8(v)` -- 安全转换（未知值返回 `Option::None`）

### Shop 相关类型

#### ShopType

```rust
pub enum ShopType {
    OnlineStore,    // 线上商城（默认）
    PhysicalStore,  // 实体门店
    ServicePoint,   // 服务网点
    Warehouse,      // 仓储/自提点（废弃预告）
    Franchise,      // 加盟店（废弃预告）
    Popup,          // 快闪店/临时店（废弃预告）
    Virtual,        // 虚拟店铺（纯服务）
}
```

**辅助方法：** `requires_location()`, `supports_physical_products()`, `supports_services()`

#### ShopOperatingStatus

```rust
pub enum ShopOperatingStatus {
    Active,       // 营业中（默认）
    Paused,       // 暂停营业
    FundDepleted, // 资金耗尽（自动暂停）
    Closed,       // 已关闭
    Closing,      // 关闭中（宽限期）
    Banned,       // 被治理层封禁
}
```

**辅助方法：** `is_operational()`, `can_resume()`, `is_closed_or_closing()`, `is_banned()`, `is_terminal_or_banned()`

#### EffectiveShopStatus -- 双层状态实时计算

```rust
pub enum EffectiveShopStatus {
    Active,         // 正常营业
    PausedBySelf,   // Shop 自身暂停
    PausedByEntity, // Entity 非 Active 导致不可运营
    FundDepleted,   // Shop 资金耗尽
    Closed,         // Shop 自身关闭
    ClosedByEntity, // Entity 关闭/封禁，强制关闭
    Closing,        // Shop 关闭中（宽限期）
    Banned,         // Shop 被治理层封禁
}
```

**计算逻辑：** `EffectiveShopStatus::compute(entity_status, shop_status)` -- Entity 终态（Banned/Closed）优先；Entity 非 Active 时 Shop 终态显示自身状态，其余显示 PausedByEntity。

**辅助方法：** `is_operational()`, `is_entity_caused()`

#### MemberRegistrationPolicy -- 会员注册策略（位标记）

```rust
pub struct MemberRegistrationPolicy(pub u8);
// OPEN = 0b0000_0000                开放注册
// PURCHASE_REQUIRED = 0b0000_0001   必须先消费
// REFERRAL_REQUIRED = 0b0000_0010   必须有推荐人
// APPROVAL_REQUIRED = 0b0000_0100   需要审批
// KYC_REQUIRED      = 0b0000_1000   注册需 KYC
// KYC_UPGRADE_REQUIRED = 0b0001_0000 升级需 KYC
// ALL_VALID = 0b0001_1111           所有已定义位的并集
```

**辅助方法：** `is_valid()`, `is_open()`, `requires_purchase()`, `requires_referral()`, `requires_approval()`, `requires_kyc()`, `requires_kyc_for_upgrade()`

#### MemberStatsPolicy -- 会员统计策略（位标记）

```rust
pub struct MemberStatsPolicy(pub u8);
// INCLUDE_REPURCHASE_DIRECT = 0b01    直推含复购赠与
// INCLUDE_REPURCHASE_INDIRECT = 0b10  间推含复购赠与
// ALL_VALID = 0b11                    所有已定义位的并集
```

**辅助方法：** `is_valid()`, `include_repurchase_direct()`, `include_repurchase_indirect()`

#### AdminPermission -- 管理员权限位掩码（13 权限）

```rust
pub mod AdminPermission {
    pub const SHOP_MANAGE: u32         = 0b0000_0000_0001;  // Shop 管理
    pub const MEMBER_MANAGE: u32       = 0b0000_0000_0010;  // 会员等级管理
    pub const TOKEN_MANAGE: u32        = 0b0000_0000_0100;  // Token 发售管理
    pub const ADS_MANAGE: u32          = 0b0000_0000_1000;  // 广告管理
    pub const REVIEW_MANAGE: u32       = 0b0000_0001_0000;  // 评论管理
    pub const DISCLOSURE_MANAGE: u32   = 0b0000_0010_0000;  // 披露/公告管理
    pub const ENTITY_MANAGE: u32       = 0b0000_0100_0000;  // 实体管理
    pub const KYC_MANAGE: u32          = 0b0000_1000_0000;  // KYC 要求管理
    pub const GOVERNANCE_MANAGE: u32   = 0b0001_0000_0000;  // 治理提案管理
    pub const ORDER_MANAGE: u32        = 0b0010_0000_0000;  // 订单管理
    pub const COMMISSION_MANAGE: u32   = 0b0100_0000_0000;  // 佣金配置管理
    pub const PRODUCT_MANAGE: u32      = 0b1000_0000_0000;  // 商品管理（独立于 SHOP_MANAGE）
    pub const MARKET_MANAGE: u32       = 0b0001_0000_0000_0000; // 市场/交易管理
    pub const ALL: u32                 = 0xFFFF_FFFF;       // 全部权限（含未来位）
    pub const ALL_DEFINED: u32         = 0b0001_1111_1111_1111; // 所有已定义位的并集
}
```

**辅助方法：** `is_valid(permissions: u32) -> bool` -- 检查是否仅包含已定义位

#### PaymentAsset -- 支付资产类型

```rust
pub enum PaymentAsset {
    Native,       // NEX 原生代币支付（默认）
    EntityToken,  // Entity Token 支付
}
```

### 商品 / 订单类型

| 类型 | 变体 | 说明 |
|------|------|------|
| `ProductStatus` | Draft / OnSale / SoldOut / OffShelf | 商品状态 |
| `ProductVisibility` | Public / MembersOnly / LevelGated(u8) | 商品可见性 |
| `ProductCategory` | Digital / Physical / Service / Subscription / Bundle / Other | 商品类别 |
| `OrderStatus` | Created / Paid / Shipped / Completed / Cancelled / Disputed / Refunded / Expired / Processing / AwaitingConfirmation / PartiallyRefunded | 订单状态（11 个变体，SCALE 编码兼容） |
| `MemberStatus` | Active / Pending / Frozen / Banned / Expired | 会员状态 |
| `DisputeStatus` | None / Submitted / Responded / Mediating / Arbitrating / Resolved / Withdrawn / Expired | 争议状态 |
| `DisputeResolution` | ComplainantWin / RespondentWin / Settlement / PartialSettlement { complainant_share_bps } | 争议裁决（含校验和获赔比例计算） |
| `TokenSaleStatus` | NotStarted / Active / Paused / Ended / Cancelled / Completed | Token 发售状态 |
| `DisclosureLevel` | Basic / Standard / Enhanced / Full | 披露级别 |
| `PriceReliability` | Reliable / Low / Unavailable | 价格可靠性等级 |
| `DividendConfig<Balance, BlockNumber>` | enabled, min_period, last_distribution, accumulated | 分红配置（含迁移提示） |
| `DividendState<Balance, BlockNumber>` | last_distribution, accumulated, total_distributed, round_count | 分红运行时状态（与配置分离） |
| `VestingSchedule` | total, released, start_block, cliff_blocks, vesting_blocks | 锁仓/归属计划（含 `releasable_at` 计算） |

### DTO 结构体

| 结构体 | 说明 |
|--------|------|
| `ProductQueryInfo<Balance>` | 商品聚合查询（shop_id, price, usdt_price, stock, status, category, visibility, min/max_order_quantity） |
| `OrderQueryInfo<AccountId, Balance>` | 订单聚合查询（order_id, entity_id, shop_id, product_id, buyer, seller, quantity, total_amount, token_payment_amount 等 14 字段） |
| `MemberLevelInfo` | 会员等级信息（level_id, name, threshold: u128, discount_rate, commission_bonus） |

## 共享错误码 (errors.rs)

`CommonError` 模块定义 19 个 `&str` 常量，通过 `DispatchError::Other` 使用，统一 13+ 个模块的错误语义：

| 常量 | 值 |
|------|-----|
| `ENTITY_NOT_FOUND` | "EntityNotFound" |
| `ENTITY_NOT_ACTIVE` | "EntityNotActive" |
| `ENTITY_LOCKED` | "EntityLocked" |
| `SHOP_NOT_FOUND` | "ShopNotFound" |
| `SHOP_NOT_ACTIVE` | "ShopNotActive" |
| `PRODUCT_NOT_FOUND` | "ProductNotFound" |
| `ORDER_NOT_FOUND` | "OrderNotFound" |
| `NOT_ENTITY_OWNER` | "NotEntityOwner" |
| `NOT_ENTITY_ADMIN` | "NotEntityAdmin" |
| `NOT_SHOP_MANAGER` | "NotShopManager" |
| `INSUFFICIENT_PERMISSION` | "InsufficientPermission" |
| `MEMBER_NOT_FOUND` | "MemberNotFound" |
| `MEMBER_BANNED` | "MemberBanned" |
| `KYC_REQUIRED` | "KycRequired" |
| `KYC_EXPIRED` | "KycExpired" |
| `TOKEN_NOT_ENABLED` | "TokenNotEnabled" |
| `INSUFFICIENT_BALANCE` | "InsufficientBalance" |
| `EMERGENCY_PAUSED` | "EmergencyPaused" |
| `INVALID_STATUS_TRANSITION` | "InvalidStatusTransition" |
| `PRICE_UNAVAILABLE` | "PriceUnavailable" |

## 分页工具 (pagination.rs)

```rust
pub struct PageRequest {
    pub offset: u32,   // 起始偏移量（0-indexed）
    pub limit: u32,    // 每页数量（默认 20）
}

pub struct PageResponse<T> {
    pub items: Vec<T>, // 当前页数据
    pub total: u32,    // 总记录数
    pub has_more: bool, // 是否有更多数据
}
```

**辅助方法：** `PageRequest::capped(max_limit)`, `PageResponse::empty()`, `PageResponse::from_slice(all_items, page)`

## Trait 接口 (traits/mod.rs)

### 一、领域 Provider 接口

#### EntityProvider -- 实体查询接口

被 9+ 个下游 pallet 使用，由 `pallet-entity-registry` 实现。

| 方法 | 必须实现 | 说明 |
|------|---------|------|
| `entity_exists(entity_id)` | 是 | 检查实体是否存在 |
| `is_entity_active(entity_id)` | 是 | 检查实体是否激活 |
| `entity_status(entity_id)` | 是 | 获取实体状态 |
| `entity_owner(entity_id)` | 是 | 获取实体所有者 |
| `entity_account(entity_id)` | 是 | 获取实体派生账户 |
| `update_entity_stats(entity_id, sales, orders)` | 是 | 更新实体统计 |
| `entity_type(entity_id)` | 否 | 获取实体类型（默认 None） |
| `register_shop / unregister_shop` | 否 | Shop 关联管理 |
| `is_entity_admin(entity_id, account, permission)` | 否 | 权限检查 |
| `entity_shops(entity_id)` | 否 | 获取 Entity 下所有 Shop |
| `pause_entity / resume_entity` | 否 | 治理暂停/恢复 |
| `set_governance_mode(entity_id, mode)` | 否 | 设置治理模式 |
| `is_entity_locked(entity_id)` | 否 | 全局锁定检查 |
| `initiate/accept/cancel_ownership_transfer` | 否 | 所有权转移 |
| `pending_ownership_transfer(entity_id)` | 否 | 待转移查询 |
| `entity_name / entity_metadata_cid / entity_description` | 否 | 元数据查询 |

#### ShopProvider -- Shop 查询接口

由 `pallet-entity-shop` 实现，供业务模块查询 Shop 信息。

| 方法 | 必须实现 | 说明 |
|------|---------|------|
| `shop_exists / is_shop_active / shop_entity_id` | 是 | 基础查询 |
| `shop_owner / shop_account / shop_type` | 是 | 属性查询 |
| `is_shop_manager(shop_id, account)` | 是 | 管理员检查 |
| `update_shop_stats / update_shop_rating` | 是 | 统计更新 |
| `deduct_operating_fund / operating_balance` | 是 | 运营资金 |
| `revert_shop_rating` | 否 | 评分回退 |
| `increment/decrement_product_count` | 否 | 商品计数 |
| `create_primary_shop / is_primary_shop` | 否 | 主 Shop 管理 |
| `shop_own_status / effective_status` | 否 | 状态查询 |
| `pause_shop / resume_shop / force_close_shop / force_pause_shop` | 否 | 控制接口 |
| `ban_shop / unban_shop` | 否 | 治理封禁 |
| `governance_close_shop / governance_set_shop_type` | 否 | 治理执行 |

#### ProductProvider -- 商品查询接口

由 `pallet-entity-product` 实现，供订单模块调用。

| 方法 | 必须实现 | 说明 |
|------|---------|------|
| `product_exists / is_product_on_sale / product_shop_id` | 是 | 基础查询 |
| `product_price / product_stock / product_category` | 是 | 属性查询 |
| `deduct_stock / restore_stock / add_sold_count` | 是 | 库存管理 |
| `product_status / product_usdt_price / product_owner` | 否 | 扩展查询 |
| `shop_product_ids / product_visibility` | 否 | 列表/可见性 |
| `product_min/max_order_quantity` | 否 | 订购限制 |
| `get_product_info(product_id)` | 否 | 聚合查询（单次 storage read） |
| `update_price / delist_product / set_inventory` | 否 | 治理调用 |
| `force_unpin_shop_products / force_remove_all_shop_products / force_delist_all_shop_products` | 否 | 级联清理 |
| `governance_set_visibility` | 否 | 治理执行 |

#### OrderProvider -- 订单查询接口

由 `pallet-entity-order` 实现，供评价/佣金/争议等模块调用。

| 方法 | 必须实现 | 说明 |
|------|---------|------|
| `order_exists / order_buyer / order_seller / order_amount / order_shop_id` | 是 | 基础查询 |
| `is_order_completed / is_order_disputed / can_dispute` | 是 | 状态检查 |
| `order_token_amount / order_payment_asset` | 否 | Token 支付查询 |
| `order_completed_at / order_created_at / order_paid_at / order_shipped_at` | 否 | 时间戳查询 |
| `order_status / order_entity_id / order_product_id / order_quantity` | 否 | 扩展查询 |
| `get_order_info(order_id)` | 否 | 聚合查询（单次 storage read） |
| `has_active_orders_for_shop(shop_id)` | 否 | Shop 活跃订单检查 |
| `order_payer / order_fund_account` | 否 | 代付查询 |

#### EntityTokenProvider -- 实体代币接口

由 `pallet-entity-token` 实现，供订单和市场模块调用。

| 方法 | 必须实现 | 说明 |
|------|---------|------|
| `is_token_enabled / token_balance` | 是 | 基础查询 |
| `reward_on_purchase / redeem_for_discount` | 是 | 奖励/折扣 |
| `transfer / reserve / unreserve / repatriate_reserved` | 是 | 资金操作 |
| `get_token_type / total_supply` | 是 | 代币属性 |
| `governance_burn(entity_id, amount)` | 是 | 治理销毁 |
| `token_name / token_symbol / token_decimals` | 否 | 元数据查询 |
| `is_token_transferable / token_holder_count / available_balance` | 否 | 扩展查询 |
| `governance_set_max_supply / governance_set_token_type / governance_set_transfer_restriction` | 否 | 治理执行 |

#### AssetLedgerPort -- 资产账本接口（Phase 3.3）

从 `EntityTokenProvider` 拆出的细粒度 Port，仅包含资产余额查询和 reserve/unreserve/repatriate 等账本操作，供 order 模块管理 Token 支付的资金锁定与结算。

```rust
pub trait AssetLedgerPort<AccountId, Balance> {
    fn is_token_enabled(entity_id: u64) -> bool;
    fn token_balance(entity_id: u64, holder: &AccountId) -> Balance;
    fn reserve(entity_id: u64, who: &AccountId, amount: Balance) -> Result<(), DispatchError>;
    fn unreserve(entity_id: u64, who: &AccountId, amount: Balance) -> Balance;
    fn repatriate_reserved(entity_id: u64, from: &AccountId, to: &AccountId, amount: Balance) -> Result<Balance, DispatchError>;
}
```

**Blanket impl:** 任何实现了 `EntityTokenProvider` 的类型自动满足 `AssetLedgerPort`，pallet-entity-token 无需额外改动。

**空实现：** `NullAssetLedgerPort` 是 `NullEntityTokenProvider` 的类型别名（通过 blanket impl 自动满足）。

### 二、激励系统接口（Phase 2 Loyalty）

#### LoyaltyReadPort -- 激励系统只读查询

```rust
pub trait LoyaltyReadPort<AccountId, Balance> {
    fn is_token_enabled(entity_id: u64) -> bool;           // Token 是否启用
    fn token_discount_balance(entity_id: u64, who: &AccountId) -> Balance; // Token 折扣可用余额
    fn shopping_balance(entity_id: u64, who: &AccountId) -> Balance;       // 购物余额
    fn shopping_total(entity_id: u64) -> Balance;           // Entity 级购物余额总额
}
```

#### LoyaltyWritePort -- 激励系统写入（继承 LoyaltyReadPort）

```rust
pub trait LoyaltyWritePort<AccountId, Balance>: LoyaltyReadPort<AccountId, Balance> {
    fn redeem_for_discount(entity_id: u64, who: &AccountId, tokens: Balance) -> Result<Balance, DispatchError>;
    fn consume_shopping_balance(entity_id: u64, who: &AccountId, amount: Balance) -> Result<(), DispatchError>;
    fn reward_on_purchase(entity_id: u64, who: &AccountId, purchase_amount: Balance) -> Result<Balance, DispatchError>;
    fn credit_shopping_balance(entity_id: u64, who: &AccountId, amount: Balance) -> Result<(), DispatchError>;
}
```

**设计区分：**
- `AssetLedgerPort` -- 资产语义：以 `fund_account()` (payer) 调用
- `LoyaltyWritePort` -- 激励语义：始终以 `&buyer` 调用

#### PointsCleanup -- 积分清理接口

```rust
pub trait PointsCleanup {
    fn cleanup_shop_points(shop_id: u64);  // Shop 关闭时清理全部积分数据
}
```

### 三、佣金/交易接口

| Trait | 说明 |
|-------|------|
| `CommissionFundGuard` | 佣金资金保护：`protected_funds(entity_id) -> u128` |
| `OrderCommissionHandler<AccountId, Balance>` | 订单佣金处理：`on_order_completed` / `on_order_cancelled` |
| `TokenOrderCommissionHandler<AccountId>` | Token 订单佣金处理（u128，含 `token_platform_fee_rate`） |
| `ShoppingBalanceProvider<AccountId, Balance>` | 购物余额查询/消费 |

### 四、其他 Provider 接口

| Trait | 说明 |
|-------|------|
| `EntityTokenPriceProvider` | Token 价格查询（NEX/USDT）、置信度、可靠性等级 |
| `PricingProvider` | NEX/USDT 全局定价接口 |
| `FeeConfigProvider` | 手续费配置查询（platform/entity/token 三级费率） |
| `KycProvider<AccountId>` | KYC 状态查询（级别、过期、参与资格） |
| `GovernanceProvider` | 治理状态查询（模式、活跃提案、锁定） |
| `DisclosureProvider<AccountId>` | 披露查询（黑窗口期、内幕人员、违规、处罚） |
| `DisclosureReadProvider<AccountId>` | 披露只读子集（新模块优先使用） |
| `DisclosureWriteProvider<AccountId>` | 披露治理写入子集 |
| `ReviewProvider<AccountId>` | 评价查询（Shop/Product 评分、评价数） |
| `MarketProvider<AccountId, Balance>` | 市场查询（交易对、交易量、买卖盘） |
| `TokenSaleProvider<Balance>` | Token 发售查询 |
| `VestingProvider<AccountId>` | 锁仓/归属查询和释放 |
| `DividendProvider<AccountId, Balance>` | 分红查询和领取 |
| `EmergencyProvider` | 紧急暂停检查 |
| `DisputeQueryProvider<AccountId>` | 争议状态查询 |
| `MemberProvider<AccountId>` | 统一会员服务（合并原两个重复定义） |
| `MemberQueryProvider<AccountId>` | 会员只读查询子集 |
| `MemberWriteProvider<AccountId>` | 会员写入子集 |

### 五、资金规则 Port 接口

| Trait | 说明 |
|-------|------|
| `EntityTreasuryPort` | Entity 资金库查询（treasury_balance, is_treasury_sufficient） |
| `ShopFundPort` | Shop 运营资金查询/扣减 |
| `FundProtectionPort` | 已承诺资金保护查询 |

### 六、治理执行 Port 接口（Phase 4）

供 governance 模块在提案通过后直接调用下游模块执行，替代链下 off-chain 执行。

| Trait | 泛型参数 | 方法数 | 说明 |
|-------|---------|--------|------|
| `MarketGovernancePort<Balance>` | Balance | 7 | 市场配置/暂停/关闭/价格保护/KYC/熔断 |
| `CommissionGovernancePort<Balance>` | Balance | 10 | 提现冷却期/Token提现/推荐人门槛/返佣上限/推荐有效期/多级暂停/团队暂停 |
| `SingleLineGovernancePort` | 无 | 3 | 单线收益配置/暂停/恢复 |
| `KycGovernancePort` | 无 | 3 | KYC等级要求/提供者授权/取消授权 |
| `ShopGovernancePort` | 无 | 3 | 积分配置/积分开关/店铺政策 |
| `TokenGovernancePort<AccountId>` | AccountId | 1 | 代币黑名单管理 |

### 七、事件回调接口

| Trait | 说明 |
|-------|------|
| `OnEntityStatusChange` | Entity 状态变更通知（suspended/banned/resumed/closed） |
| `OnOrderStatusChange<AccountId, Balance>` | 订单状态变更通知 |
| `OnKycStatusChange<AccountId>` | KYC 状态变更通知 |
| `OnDisclosureViolation` | 披露违规回调 |
| `OnMemberRemoved<AccountId>` | 会员移除回调（支持元组组合，宏生成 A-H 最多 8 个） |

### 八、Blanket Impl 桥接

| 源 Trait | 目标 Trait | 说明 |
|----------|-----------|------|
| `EntityTokenProvider` | `AssetLedgerPort` | 零改动迁移：pallet-entity-token 自动满足 AssetLedgerPort |
| `DisclosureProvider` | `DisclosureReadProvider` | 已实现 DisclosureProvider 的类型自动实现只读子集 |
| `DisclosureProvider` | `DisclosureWriteProvider` | 已实现 DisclosureProvider 的类型自动实现写入子集 |
| `MemberProvider` | `MemberQueryProvider` | 已实现 MemberProvider 的类型自动实现只读子集 |
| `MemberProvider` | `MemberWriteProvider` | 已实现 MemberProvider 的类型自动实现写入子集 |

## 测试用空实现（Null 实现）

| 结构体 / 类型别名 | 说明 |
|-------------------|------|
| `NullEntityProvider` | 空实体提供者 |
| `NullShopProvider` | 空 Shop 提供者 |
| `NullProductProvider` | 空商品提供者 |
| `NullOrderProvider` | 空订单提供者 |
| `NullEntityTokenProvider` | 空代币提供者 |
| `NullAssetLedgerPort` | = `NullEntityTokenProvider`（通过 blanket impl 自动满足 AssetLedgerPort） |
| `NullPricingProvider` | 空定价提供者（返回默认价格 1） |
| `NullFeeConfigProvider` | 空手续费配置提供者（platform_fee_rate = 100bps） |
| `NullKycProvider` | 空 KYC 提供者 |
| `NullGovernanceProvider` | 空治理提供者 |
| `NullDisclosureProvider` | 空披露提供者 |
| `NullDisclosureReadProvider` | = `NullDisclosureProvider` 的类型别名 |
| `NullDisclosureWriteProvider` | = `NullDisclosureProvider` 的类型别名 |
| `NullMemberProvider` | 空会员服务提供者 |
| `NullMemberQueryProvider` | = `NullMemberProvider` 的类型别名 |
| `NullMemberWriteProvider` | = `NullMemberProvider` 的类型别名 |
| `NullLoyaltyProvider` | 空 Loyalty 提供者（LoyaltyReadPort + LoyaltyWritePort） |
| `NullReviewProvider` | 空评价提供者 |
| `NullMarketProvider` | 空市场提供者 |
| `NullTokenSaleProvider` | 空 Token Sale 提供者 |
| `NullVestingProvider` | 空锁仓提供者 |
| `NullDividendProvider` | 空分红提供者 |
| `NullEmergencyProvider` | 空紧急暂停提供者 |
| `NullDisputeQueryProvider` | 空争议查询提供者 |
| `()` impl | EntityTokenPriceProvider, CommissionFundGuard, OrderCommissionHandler, TokenOrderCommissionHandler, ShoppingBalanceProvider, OnEntityStatusChange, OnOrderStatusChange, OnKycStatusChange, OnDisclosureViolation, OnMemberRemoved, EntityTreasuryPort, ShopFundPort, FundProtectionPort, PointsCleanup, 6 个 GovernancePort |

所有空实现对查询类方法返回 `false`/`None`/`Default`，对写入类方法返回 `Ok(())`。

## 使用方式

```toml
[dependencies]
pallet-entity-common = { workspace = true }
```

```rust
use pallet_entity_common::{
    // 类型
    EntityType, GovernanceMode, EntityStatus, TokenType,
    TransferRestrictionMode, ShopType, ShopOperatingStatus,
    EffectiveShopStatus, MemberRegistrationPolicy,
    MemberStatsPolicy, AdminPermission, PaymentAsset,
    ProductStatus, ProductVisibility, ProductCategory,
    OrderStatus, MemberStatus, DisputeStatus, DisputeResolution,
    TokenSaleStatus, DisclosureLevel, PriceReliability,
    DividendConfig, DividendState, VestingSchedule,
    ProductQueryInfo, OrderQueryInfo, MemberLevelInfo,

    // Provider trait
    EntityProvider, ShopProvider, ProductProvider,
    OrderProvider, EntityTokenProvider, EntityTokenPriceProvider,
    PricingProvider, FeeConfigProvider, CommissionFundGuard,
    OrderCommissionHandler, TokenOrderCommissionHandler,
    ShoppingBalanceProvider, KycProvider, GovernanceProvider,
    DisclosureProvider, DisclosureReadProvider, DisclosureWriteProvider,
    ReviewProvider, MarketProvider, TokenSaleProvider,
    VestingProvider, DividendProvider, EmergencyProvider,
    DisputeQueryProvider, MemberProvider, MemberQueryProvider,
    MemberWriteProvider,

    // Port trait
    AssetLedgerPort, LoyaltyReadPort, LoyaltyWritePort, PointsCleanup,
    EntityTreasuryPort, ShopFundPort, FundProtectionPort,
    MarketGovernancePort, CommissionGovernancePort,
    SingleLineGovernancePort, KycGovernancePort,
    ShopGovernancePort, TokenGovernancePort,

    // 事件回调
    OnEntityStatusChange, OnOrderStatusChange,
    OnKycStatusChange, OnDisclosureViolation, OnMemberRemoved,

    // 空实现
    NullEntityProvider, NullShopProvider, NullProductProvider,
    NullOrderProvider, NullEntityTokenProvider, NullAssetLedgerPort,
    NullPricingProvider, NullMemberProvider, NullLoyaltyProvider,

    // 错误码
    CommonError,

    // 分页
    PageRequest, PageResponse,
};
```

## 依赖关系图

```
pallet-entity-common (纯类型 crate)
    |
    |-- types/     领域枚举、DTO、位掩码
    |-- traits/    跨模块 Port/Provider trait
    |-- errors     CommonError 共享错误码
    |-- pagination PageRequest/PageResponse
    |
    +---> pallet-entity-registry     (EntityProvider 实现)
    +---> pallet-entity-shop         (ShopProvider 实现)
    +---> pallet-entity-token        (EntityTokenProvider 实现 + AssetLedgerPort blanket)
    +---> pallet-entity-product      (ProductProvider 实现)
    +---> pallet-entity-order        (OrderProvider 实现, AssetLedgerPort 消费方)
    +---> pallet-entity-member       (MemberProvider 实现)
    +---> pallet-entity-loyalty      (LoyaltyReadPort + LoyaltyWritePort + PointsCleanup 实现)
    +---> pallet-entity-commission   (CommissionFundGuard / OrderCommissionHandler / CommissionGovernancePort 实现)
    +---> pallet-entity-governance   (GovernanceProvider 实现, 6 个 GovernancePort 消费方)
    +---> pallet-entity-market       (MarketProvider / MarketGovernancePort 实现)
    +---> pallet-entity-review       (ReviewProvider 实现)
    +---> pallet-entity-disclosure   (DisclosureProvider 实现)
    +---> pallet-entity-kyc          (KycProvider / KycGovernancePort 实现)
    +---> pallet-entity-sale         (TokenSaleProvider 实现)
```

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-01-31 | 初始版本 |
| v0.2.0 | 2026-02-01 | Phase 2-4: 扩展 EntityType、TokenType、GovernanceMode |
| v0.3.0 | 2026-02-05 | Entity-Shop 分离架构：ShopType、ShopOperatingStatus、EffectiveShopStatus、MemberMode |
| v0.4.0 | 2026-02-07 | 新增 MemberRegistrationPolicy、CommissionFundGuard、OrderCommissionHandler |
| v0.5.0 | 2026-02-08 | 新增 TransferRestrictionMode、DividendConfig、TokenType 辅助方法 |
| v0.6.0 | 2026-03 | 新增 AdminPermission 位掩码、EntityTokenPriceProvider、TokenOrderCommissionHandler、ShoppingBalanceProvider、OrderMemberHandler、MemberStatsPolicy、PaymentAsset |
| v0.6.1 | 2026-03 | 审计修复: M1(Closing一致性)、M2(AdminPermission::ALL_DEFINED+is_valid)、M3(Policy is_valid)、L2(try_from_u8)、L3(Cargo特性传播) |
| v0.6.2 | 2026-03 | 审计 Round 2: README 同步更新 |
| v0.7.0 | 2026-03 | 用户角色分析重构: 删除 MemberMode 死代码、EntityProvider::entity_type()、AdminPermission 新增 GOVERNANCE/ORDER/COMMISSION_MANAGE、ProductCategory 新增 Subscription/Bundle、新增 KycProvider/GovernanceProvider trait |
| v0.8.0 | 2026-03 | MemberProvider trait 统一: 合并重复定义到 common，新增 MemberLevelInfo 和 NullMemberProvider |
| v0.9.0 | 2026-03 | 深度审计全量实现: CommonError 共享错误码、ReviewProvider/MarketProvider trait、GovernanceMode 新增 MultiSig/Council、AdminPermission 新增 PRODUCT_MANAGE/MARKET_MANAGE (13权限)、OrderStatus 新增 Processing/AwaitingConfirmation/PartiallyRefunded、DisputeResolution 新增 PartialSettlement、DividendState 拆分、PriceReliability 枚举 |
| v0.9.1 | 2026-03 | **Phase 1 重构**: 代码拆分为 types/、traits/、errors.rs、pagination.rs 目录结构；**Phase 2 Loyalty**: 新增 LoyaltyReadPort + LoyaltyWritePort + PointsCleanup 接口和 NullLoyaltyProvider；**Phase 3.3 AssetLedgerPort**: 新增 AssetLedgerPort 细粒度 Port + EntityTokenProvider blanket impl + NullAssetLedgerPort；**Phase 4 治理 Port**: 新增 MarketGovernancePort、CommissionGovernancePort、SingleLineGovernancePort、KycGovernancePort、ShopGovernancePort、TokenGovernancePort 六个治理执行接口；MemberProvider/DisclosureProvider 拆分 Query/Write 子 trait + blanket impl 桥接；新增 EntityTreasuryPort、ShopFundPort、FundProtectionPort 资金规则 Port；新增 OnEntityStatusChange、OnOrderStatusChange、OnKycStatusChange、OnDisclosureViolation、OnMemberRemoved 事件回调；新增 DisputeQueryProvider、TokenSaleProvider、VestingProvider、DividendProvider、EmergencyProvider、FeeConfigProvider 等接口 |
