# pallet-entity-common

> 📦 Entity 模块集群公共类型和 Trait 定义库

## 概述

`pallet-entity-common` 是 Entity 子系统的基础依赖，定义了所有 Entity 子模块共享的类型枚举、跨模块 Trait 接口和工具函数。

### 特性

- **纯 Rust crate** — 无链上存储，无 pallet 宏，仅提供类型和 Trait
- **no_std 兼容** — 支持 WebAssembly 运行时
- **跨模块共享** — 被 registry、shop、token、member、commission、governance、market、service、order、review、disclosure、kyc、sale 等 13+ 个 pallet 引用

## 核心类型

### EntityType — 实体类型

```rust
pub enum EntityType {
    Merchant,         // 商户（默认）
    Enterprise,       // 企业
    DAO,              // 去中心化自治组织
    Community,        // 社区
    Project,          // 项目方
    ServiceProvider,  // 服务提供商
    Fund,             // 基金
    Custom(u8),       // 自定义类型
}
```

**辅助方法：**

| 方法 | 说明 |
|------|------|
| `default_governance()` | 返回推荐治理模式（如 DAO→FullDAO, Merchant→None） |
| `default_token_type()` | 返回推荐代币类型（如 DAO→Governance, Fund→Share） |
| `requires_kyc_by_default()` | Enterprise/Fund/Project 默认需要 KYC |
| `suggests_token_type(token)` | 检查代币类型是否为推荐组合 |
| `suggests_governance(mode)` | 检查治理模式是否为推荐组合 |
| `default_transfer_restriction()` | 返回默认转账限制模式 |

### GovernanceMode — 治理模式

```rust
pub enum GovernanceMode {
    None,       // 无治理（管理员全权控制，默认）
    FullDAO,    // 完全 DAO（所有决策需代币投票）
}
```

### EntityStatus — 实体状态

```rust
pub enum EntityStatus {
    Pending,      // 待审核（reopen 后等待治理审批）
    Active,       // 正常运营（默认）
    Suspended,    // 暂停运营
    Banned,       // 被封禁
    Closed,       // 已关闭
    PendingClose, // 待关闭审批
}
```

### TokenType — 通证类型

```rust
pub enum TokenType {
    Points,       // 积分（默认，消费奖励）
    Governance,   // 治理代币（投票权）
    Equity,       // 股权代币（分红权，需 Enhanced KYC）
    Membership,   // 会员代币（身份凭证，不可转让）
    Share,        // 份额代币（基金份额）
    Bond,         // 债券代币（固定收益）
    Hybrid(u8),   // 混合型（多种权益）
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

### TransferRestrictionMode — 转账限制

```rust
pub enum TransferRestrictionMode {
    None,         // 无限制（默认）
    Whitelist,    // 白名单模式
    Blacklist,    // 黑名单模式
    KycRequired,  // KYC 模式
    MembersOnly,  // 闭环模式（仅实体成员间）
}
```

**辅助方法：**

| 方法 | 说明 |
|------|------|
| `from_u8(v)` | 从 u8 转换（未知值回退到 None） |
| `try_from_u8(v)` | 安全转换（未知值返回 `Option::None`） |

### Shop 相关类型

#### ShopType

```rust
pub enum ShopType {
    OnlineStore,    // 线上商城（默认）
    PhysicalStore,  // 实体门店
    ServicePoint,   // 服务网点
    Warehouse,      // 仓储/自提点
    Franchise,      // 加盟店
    Popup,          // 快闪店/临时店
    Virtual,        // 虚拟店铺（纯服务）
}
```

#### ShopOperatingStatus

```rust
pub enum ShopOperatingStatus {
    Pending,      // 待激活（默认）
    Active,       // 营业中
    Paused,       // 暂停营业
    FundDepleted, // 资金耗尽
    Closing,      // 关闭中
    Closed,       // 已关闭
}
```

#### EffectiveShopStatus — 双层状态实时计算

```rust
pub enum EffectiveShopStatus {
    Active,         // 正常营业
    PausedBySelf,   // Shop 自身暂停
    PausedByEntity, // Entity 非 Active 导致不可运营
    FundDepleted,   // Shop 资金耗尽
    Closed,         // Shop 自身关闭（含 Closing 终态）
    ClosedByEntity, // Entity 关闭/封禁，强制关闭
    Pending,        // 待激活
}
```

**计算逻辑：** `EffectiveShopStatus::compute(entity_status, shop_status)` — Entity 终态（Banned/Closed）优先；Entity 非 Active 时 Shop Closed/Closing 显示 Closed，其余显示 PausedByEntity。

#### MemberMode — 会员体系模式

```rust
pub enum MemberMode {
    Inherit,  // 继承模式（Entity 级别，所有 Shop 共享，默认且唯一）
}
```

#### MemberRegistrationPolicy — 会员注册策略（位标记）

```rust
pub struct MemberRegistrationPolicy(pub u8);
// OPEN = 0b0000_0000           开放注册
// PURCHASE_REQUIRED = 0b001    必须先消费
// REFERRAL_REQUIRED = 0b010    必须有推荐人
// APPROVAL_REQUIRED = 0b100    需要审批
// ALL_VALID = 0b111            所有已定义位的并集
```

**辅助方法：** `is_valid()`, `is_open()`, `requires_purchase()`, `requires_referral()`, `requires_approval()`

#### MemberStatsPolicy — 会员统计策略（位标记）

```rust
pub struct MemberStatsPolicy(pub u8);
// INCLUDE_REPURCHASE_DIRECT = 0b01    直推含复购赠与
// INCLUDE_REPURCHASE_INDIRECT = 0b10  间推含复购赠与
// ALL_VALID = 0b11                    所有已定义位的并集
```

**辅助方法：** `is_valid()`, `include_repurchase_direct()`, `include_repurchase_indirect()`

#### AdminPermission — 管理员权限位掩码

```rust
pub mod AdminPermission {
    pub const SHOP_MANAGE: u32       = 0b0000_0001;
    pub const MEMBER_MANAGE: u32     = 0b0000_0010;
    pub const TOKEN_MANAGE: u32      = 0b0000_0100;
    pub const ADS_MANAGE: u32        = 0b0000_1000;
    pub const REVIEW_MANAGE: u32     = 0b0001_0000;
    pub const DISCLOSURE_MANAGE: u32 = 0b0010_0000;
    pub const ALL: u32               = 0xFFFF_FFFF;  // 全部权限（含未来位）
    pub const ALL_DEFINED: u32       = 0b0011_1111;  // 所有已定义位的并集
}
```

**辅助方法：** `is_valid(permissions: u32) -> bool` — 检查是否仅包含已定义位

#### PaymentAsset — 支付资产类型

```rust
pub enum PaymentAsset {
    Native,       // NEX 原生代币支付（默认）
    EntityToken,  // Entity Token 支付
}
```

### 商品 / 订单类型

| 类型 | 说明 |
|------|------|
| `ProductStatus` | Draft / OnSale / SoldOut / OffShelf |
| `ProductCategory` | Digital / Physical / Service / Other |
| `OrderStatus` | Created / Paid / Shipped / Completed / Cancelled / Disputed / Refunded / Expired |
| `DividendConfig<Balance, BlockNumber>` | 分红配置（启用、周期、累计待分配金额） |

## Trait 接口

### EntityProvider — 实体查询接口

被 9+ 个下游 pallet 使用，由 `pallet-entity-registry` 实现。

```rust
pub trait EntityProvider<AccountId> {
    fn entity_exists(entity_id: u64) -> bool;
    fn is_entity_active(entity_id: u64) -> bool;
    fn entity_status(entity_id: u64) -> Option<EntityStatus>;
    fn entity_owner(entity_id: u64) -> Option<AccountId>;
    fn entity_account(entity_id: u64) -> AccountId;
    fn update_entity_stats(entity_id: u64, sales: u128, orders: u32) -> DispatchResult;
    fn update_entity_rating(entity_id: u64, rating: u8) -> DispatchResult;
    fn register_shop(entity_id: u64, shop_id: u64) -> DispatchResult;
    fn unregister_shop(entity_id: u64, shop_id: u64) -> DispatchResult;
    fn is_entity_admin(entity_id: u64, account: &AccountId, required_permission: u32) -> bool;
    fn entity_shops(entity_id: u64) -> Vec<u64>;
    fn pause_entity(entity_id: u64) -> DispatchResult;
    fn resume_entity(entity_id: u64) -> DispatchResult;
    fn set_governance_mode(entity_id: u64, mode: GovernanceMode) -> DispatchResult;
}
```

### ShopProvider — Shop 查询接口

由 `pallet-entity-shop` 实现，供业务模块查询 Shop 信息。

```rust
pub trait ShopProvider<AccountId> {
    fn shop_exists(shop_id: u64) -> bool;
    fn is_shop_active(shop_id: u64) -> bool;
    fn shop_entity_id(shop_id: u64) -> Option<u64>;
    fn shop_owner(shop_id: u64) -> Option<AccountId>;
    fn shop_account(shop_id: u64) -> AccountId;
    fn shop_type(shop_id: u64) -> Option<ShopType>;
    fn is_shop_manager(shop_id: u64, account: &AccountId) -> bool;
    fn update_shop_stats(shop_id: u64, sales: u128, orders: u32) -> DispatchResult;
    fn update_shop_rating(shop_id: u64, rating: u8) -> DispatchResult;
    fn deduct_operating_fund(shop_id: u64, amount: u128) -> DispatchResult;
    fn operating_balance(shop_id: u64) -> u128;
    fn create_primary_shop(entity_id: u64, name: Vec<u8>, shop_type: ShopType) -> Result<u64, _>;
    fn is_primary_shop(shop_id: u64) -> bool;
    fn shop_own_status(shop_id: u64) -> Option<ShopOperatingStatus>;
    fn effective_status(shop_id: u64) -> Option<EffectiveShopStatus>;
    fn pause_shop(shop_id: u64) -> DispatchResult;
    fn resume_shop(shop_id: u64) -> DispatchResult;
    fn force_close_shop(shop_id: u64) -> DispatchResult;
}
```

### ProductProvider — 商品查询接口

由 `pallet-entity-service` 实现，供订单模块调用。

```rust
pub trait ProductProvider<AccountId, Balance> {
    fn product_exists(product_id: u64) -> bool;
    fn is_product_on_sale(product_id: u64) -> bool;
    fn product_shop_id(product_id: u64) -> Option<u64>;
    fn product_price(product_id: u64) -> Option<Balance>;
    fn product_stock(product_id: u64) -> Option<u32>;
    fn product_category(product_id: u64) -> Option<ProductCategory>;
    fn deduct_stock(product_id: u64, quantity: u32) -> DispatchResult;
    fn restore_stock(product_id: u64, quantity: u32) -> DispatchResult;
    fn add_sold_count(product_id: u64, quantity: u32) -> DispatchResult;
    fn update_price(product_id: u64, new_price: Balance) -> DispatchResult;
    fn delist_product(product_id: u64) -> DispatchResult;
    fn set_inventory(product_id: u64, new_inventory: u32) -> DispatchResult;
}
```

### OrderProvider — 订单查询接口

由 `pallet-entity-order` 实现，供评价/佣金/争议等模块调用。

```rust
pub trait OrderProvider<AccountId, Balance> {
    fn order_exists(order_id: u64) -> bool;
    fn order_buyer(order_id: u64) -> Option<AccountId>;
    fn order_seller(order_id: u64) -> Option<AccountId>;
    fn order_amount(order_id: u64) -> Option<Balance>;
    fn order_shop_id(order_id: u64) -> Option<u64>;
    fn is_order_completed(order_id: u64) -> bool;
    fn is_order_disputed(order_id: u64) -> bool;
    fn can_dispute(order_id: u64, who: &AccountId) -> bool;
    fn order_token_amount(order_id: u64) -> Option<u128>;
    fn order_payment_asset(order_id: u64) -> Option<PaymentAsset>;
    fn order_completed_at(order_id: u64) -> Option<u64>;
}
```

### EntityTokenProvider — 实体代币接口

由 `pallet-entity-token` 实现，供订单和市场模块调用。

```rust
pub trait EntityTokenProvider<AccountId, Balance> {
    fn is_token_enabled(entity_id: u64) -> bool;
    fn token_balance(entity_id: u64, holder: &AccountId) -> Balance;
    fn reward_on_purchase(entity_id: u64, buyer: &AccountId, purchase_amount: Balance) -> Result<Balance, DispatchError>;
    fn redeem_for_discount(entity_id: u64, buyer: &AccountId, tokens: Balance) -> Result<Balance, DispatchError>;
    fn transfer(entity_id: u64, from: &AccountId, to: &AccountId, amount: Balance) -> Result<(), DispatchError>;
    fn reserve(entity_id: u64, who: &AccountId, amount: Balance) -> Result<(), DispatchError>;
    fn unreserve(entity_id: u64, who: &AccountId, amount: Balance) -> Balance;
    fn repatriate_reserved(entity_id: u64, from: &AccountId, to: &AccountId, amount: Balance) -> Result<Balance, DispatchError>;
    fn get_token_type(entity_id: u64) -> TokenType;
    fn total_supply(entity_id: u64) -> Balance;
}
```

### EntityTokenPriceProvider — 实体代币价格查询

```rust
pub trait EntityTokenPriceProvider {
    type Balance;
    fn get_token_price(entity_id: u64) -> Option<Self::Balance>;   // NEX per Token
    fn get_token_price_usdt(entity_id: u64) -> Option<u64>;        // USDT per Token
    fn token_price_confidence(entity_id: u64) -> u8;               // 0-100
    fn is_token_price_stale(entity_id: u64, max_age_blocks: u32) -> bool;
    fn is_token_price_reliable(entity_id: u64) -> bool;            // 默认 confidence >= 30
}
```

### PricingProvider — NEX/USDT 定价接口

由 `pallet-trading-pricing` 实现，供 Entity/Shop 模块计算 USDT 等值 NEX。

```rust
pub trait PricingProvider {
    fn get_nex_usdt_price() -> u64;   // 精度 10^6，返回 0 表示不可用
    fn is_price_stale() -> bool;      // 默认 false
}
```

### CommissionFundGuard — 佣金资金保护

由 `pallet-entity-commission` 实现，防止运营扣费侵占用户佣金。

```rust
pub trait CommissionFundGuard {
    fn protected_funds(entity_id: u64) -> u128;
}
```

### OrderCommissionHandler — 订单佣金处理

由 `pallet-entity-commission` 实现，订单完成/取消时触发佣金计算。

```rust
pub trait OrderCommissionHandler<AccountId, Balance> {
    fn on_order_completed(entity_id: u64, shop_id: u64, order_id: u64, buyer: &AccountId, order_amount: Balance, platform_fee: Balance) -> DispatchResult;
    fn on_order_cancelled(order_id: u64) -> DispatchResult;
}
```

### TokenOrderCommissionHandler — Token 订单佣金处理

```rust
pub trait TokenOrderCommissionHandler<AccountId> {
    fn on_token_order_completed(entity_id: u64, shop_id: u64, order_id: u64, buyer: &AccountId, token_amount: u128, token_platform_fee: u128) -> DispatchResult;
    fn on_token_order_cancelled(order_id: u64) -> DispatchResult;
    fn token_platform_fee_rate(entity_id: u64) -> u16;  // bps
    fn entity_account(entity_id: u64) -> AccountId;
}
```

### ShoppingBalanceProvider — 购物余额接口

```rust
pub trait ShoppingBalanceProvider<AccountId, Balance> {
    fn shopping_balance(entity_id: u64, account: &AccountId) -> Balance;
    fn consume_shopping_balance(entity_id: u64, account: &AccountId, amount: Balance) -> DispatchResult;
}
```

### OrderMemberHandler — 订单会员处理

```rust
pub trait OrderMemberHandler<AccountId, Balance> {
    fn auto_register(entity_id: u64, account: &AccountId, referrer: Option<AccountId>) -> DispatchResult;
    fn update_spent(entity_id: u64, account: &AccountId, amount: Balance, amount_usdt: u64) -> DispatchResult;
    fn check_order_upgrade_rules(entity_id: u64, buyer: &AccountId, product_id: u64, order_amount: Balance, amount_usdt: u64) -> DispatchResult;
}
```

## 测试用空实现

| 结构体 / 类型 | 说明 |
|---------------|------|
| `NullEntityProvider` | 空实体提供者 |
| `NullShopProvider` | 空 Shop 提供者 |
| `NullProductProvider` | 空商品提供者 |
| `NullOrderProvider` | 空订单提供者 |
| `NullEntityTokenProvider` | 空代币提供者 |
| `NullPricingProvider` | 空定价提供者（返回默认价格 1） |
| `()` impl for `EntityTokenPriceProvider` | 空价格提供者 |
| `()` impl for `CommissionFundGuard` | 空佣金保护 |
| `()` impl for `OrderCommissionHandler` | 空佣金处理 |
| `()` impl for `TokenOrderCommissionHandler` | 空 Token 佣金处理 |
| `()` impl for `ShoppingBalanceProvider` | 空购物余额 |
| `()` impl for `OrderMemberHandler` | 空会员处理 |

所有空实现对查询类方法返回 `false`/`None`/`Default`，对写入类方法返回 `Ok(())`。

## 使用方式

```toml
[dependencies]
pallet-entity-common = { workspace = true }
```

```rust
use pallet_entity_common::{
    EntityType, GovernanceMode, EntityStatus, TokenType,
    TransferRestrictionMode, ShopType, ShopOperatingStatus,
    EffectiveShopStatus, MemberMode, MemberRegistrationPolicy,
    MemberStatsPolicy, AdminPermission, PaymentAsset,
    EntityProvider, ShopProvider, ProductProvider,
    OrderProvider, EntityTokenProvider, EntityTokenPriceProvider,
    PricingProvider, CommissionFundGuard, OrderCommissionHandler,
    TokenOrderCommissionHandler, ShoppingBalanceProvider,
    OrderMemberHandler,
};
```

## 依赖关系图

```
pallet-entity-common (纯类型 crate)
    │
    ├─► pallet-entity-registry   (EntityProvider 实现)
    ├─► pallet-entity-shop       (ShopProvider 实现)
    ├─► pallet-entity-token      (EntityTokenProvider 实现)
    ├─► pallet-entity-service    (ProductProvider 实现)
    ├─► pallet-entity-order      (OrderProvider 实现)
    ├─► pallet-entity-member     (MemberProvider 消费方)
    ├─► pallet-entity-commission (CommissionFundGuard / OrderCommissionHandler 实现)
    ├─► pallet-entity-governance (消费方)
    ├─► pallet-entity-market     (消费方)
    ├─► pallet-entity-review     (消费方)
    ├─► pallet-entity-disclosure (消费方)
    ├─► pallet-entity-kyc        (消费方)
    └─► pallet-entity-sale       (消费方)
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
| v0.6.1 | 2026-03 | 审计修复: M1(Closing 一致性)、M2(AdminPermission::ALL_DEFINED + is_valid)、M3(Policy is_valid)、L2(try_from_u8)、L3(Cargo 特性传播) |
| v0.6.2 | 2026-03 | 审计 Round 2: README 同步更新 — GovernanceMode/MemberMode 枚举、EntityProvider/ShopProvider/OrderProvider trait 签名修正 |
