# RFC: Entity-Shop 硬分离架构设计

> **状态**: Draft  
> **作者**: Cascade AI  
> **创建日期**: 2026-02-05  
> **最后更新**: 2026-02-05

## 1. 概述

### 1.1 背景

当前 NEXUS 平台的实体管理系统将 Entity（组织）和 Shop（店铺）概念合并为单一结构。这种设计在简单场景下工作良好，但在以下场景存在局限：

- 连锁企业：一个品牌需要管理多个门店
- 多品牌集团：一个组织下有多个独立品牌
- 线上线下融合：同一组织的电商和实体店需要统一管理
- DAO 多业务：去中心化组织需要分离治理层和业务层
- 加盟模式：总部与加盟商的关系管理

### 1.2 目标

设计一套 Entity-Shop 分离架构，实现：

1. **组织层 (Entity)**: 负责治理、代币、KYC、合规
2. **业务层 (Shop)**: 负责商品、订单、会员、返佣
3. **1:N 关系**: 一个 Entity 可创建多个 Shop
4. **资源共享**: Shop 可共享 Entity 的代币、会员体系
5. **独立运营**: 每个 Shop 有独立的运营资金和业务数据

---

## 2. 架构设计

### 2.1 整体架构图

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              NEXUS Platform                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │                     Entity Layer (组织层)                           │ │
│  │  ┌──────────────────────────────────────────────────────────────┐  │ │
│  │  │                    pallet-entity-registry                     │  │ │
│  │  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐           │  │ │
│  │  │  │  Entity A   │  │  Entity B   │  │  Entity C   │           │  │ │
│  │  │  │  (DAO)      │  │ (Enterprise)│  │ (Fund)      │           │  │ │
│  │  │  │             │  │             │  │             │           │  │ │
│  │  │  │ • owner     │  │ • owner     │  │ • owner     │           │  │ │
│  │  │  │ • admins[]  │  │ • admins[]  │  │ • admins[]  │           │  │ │
│  │  │  │ • treasury  │  │ • treasury  │  │ • treasury  │           │  │ │
│  │  │  │ • governance│  │ • governance│  │ • governance│           │  │ │
│  │  │  │ • kyc_level │  │ • kyc_level │  │ • kyc_level │           │  │ │
│  │  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘           │  │ │
│  │  │         │                │                │                   │  │ │
│  │  └─────────┼────────────────┼────────────────┼───────────────────┘  │ │
│  │            │ owns           │ owns           │ owns                 │ │
│  └────────────┼────────────────┼────────────────┼──────────────────────┘ │
│               ▼                ▼                ▼                        │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │                      Shop Layer (业务层)                            │ │
│  │  ┌──────────────────────────────────────────────────────────────┐  │ │
│  │  │                      pallet-entity-shop                       │  │ │
│  │  │                                                               │  │ │
│  │  │  Entity A 的 Shops:                                           │  │ │
│  │  │  ┌─────────┐ ┌─────────┐ ┌─────────┐                         │  │ │
│  │  │  │ Shop A1 │ │ Shop A2 │ │ Shop A3 │                         │  │ │
│  │  │  │ (线上)   │ │ (门店1) │ │ (门店2) │                         │  │ │
│  │  │  └─────────┘ └─────────┘ └─────────┘                         │  │ │
│  │  │                                                               │  │ │
│  │  │  Entity B 的 Shops:                                           │  │ │
│  │  │  ┌─────────┐ ┌─────────┐                                     │  │ │
│  │  │  │ Shop B1 │ │ Shop B2 │                                     │  │ │
│  │  │  │ (品牌A)  │ │ (品牌B) │                                     │  │ │
│  │  │  └─────────┘ └─────────┘                                     │  │ │
│  │  │                                                               │  │ │
│  │  └──────────────────────────────────────────────────────────────┘  │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │                    Supporting Modules (支撑模块)                    │ │
│  │                                                                     │ │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐  │ │
│  │  │   token     │ │ governance  │ │    kyc      │ │    sale     │  │ │
│  │  │ (Entity级)  │ │ (Entity级)  │ │ (Entity级)  │ │ (Entity级)  │  │ │
│  │  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘  │ │
│  │                                                                     │ │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐  │ │
│  │  │   member    │ │ commission  │ │  service    │ │ transaction │  │ │
│  │  │  (Shop级)   │ │  (Shop级)   │ │  (Shop级)   │ │  (Shop级)   │  │ │
│  │  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘  │ │
│  │                                                                     │ │
│  │  ┌─────────────┐ ┌─────────────┐                                   │ │
│  │  │   market    │ │   review    │  (跨层级)                         │ │
│  │  └─────────────┘ └─────────────┘                                   │ │
│  └────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.2 模块职责划分

#### 2.2.1 Entity 层模块 (组织级)

| 模块 | 职责 | 关键功能 |
|------|------|---------|
| **registry** | Entity 生命周期 | 创建、更新、关闭、权限管理 |
| **token** | 组织代币发行 | 发币、分红、锁仓、转账限制 |
| **governance** | 组织治理 | 提案、投票、执行 |
| **kyc** | 合规认证 | KYC/AML、认证提供商管理 |
| **sale** | 代币发售 | IDO、ICO、锁仓计划 |
| **disclosure** | 财务披露 | 定期报告、内幕交易控制 |

#### 2.2.2 Shop 层模块 (业务级)

| 模块 | 职责 | 关键功能 |
|------|------|---------|
| **shop** (新) | Shop 生命周期 | 创建、运营资金、状态管理 |
| **member** | 会员管理 | 等级、推荐关系、积分 |
| **commission** | 返佣管理 | 多模式返佣、分级提现 |
| **service** | 商品/服务 | CRUD、定价、库存 |
| **transaction** | 订单管理 | 下单、托管、退款 |

#### 2.2.3 跨层级模块

| 模块 | 职责 | 说明 |
|------|------|------|
| **market** | 代币交易 | Entity 代币的 P2P 交易 |
| **review** | 评价系统 | Shop 和 Entity 均可评价 |
| **common** | 公共类型 | Trait、枚举、工具函数 |

---

## 3. 数据结构设计

### 3.1 Entity 结构 (组织层)

```rust
/// 实体 (组织层)
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct Entity<AccountId, Balance, BlockNumber, MaxNameLen, MaxCidLen, MaxAdmins, MaxShops> {
    // ========== 基础信息 ==========
    /// 实体 ID (全局唯一)
    pub id: u64,
    /// 创建者/所有者
    pub owner: AccountId,
    /// 实体名称
    pub name: BoundedVec<u8, MaxNameLen>,
    /// Logo IPFS CID
    pub logo_cid: Option<BoundedVec<u8, MaxCidLen>>,
    /// 描述 IPFS CID
    pub description_cid: Option<BoundedVec<u8, MaxCidLen>>,
    /// 元数据 URI
    pub metadata_uri: Option<BoundedVec<u8, MaxCidLen>>,
    
    // ========== 类型与状态 ==========
    /// 实体类型
    pub entity_type: EntityType,
    /// 实体状态
    pub status: EntityStatus,
    /// 是否已验证 (官方认证)
    pub verified: bool,
    
    // ========== 治理相关 ==========
    /// 治理模式
    pub governance_mode: GovernanceMode,
    /// 管理员列表
    pub admins: BoundedVec<AccountId, MaxAdmins>,
    /// 金库账户 (派生账户)
    pub treasury_account: AccountId,
    /// 金库余额快照 (用于快速查询)
    pub treasury_balance: Balance,
    
    // ========== Shop 关联 ==========
    /// 下属 Shop ID 列表
    pub shop_ids: BoundedVec<u64, MaxShops>,
    /// 最大允许 Shop 数量 (可通过治理调整)
    pub max_shops: u32,
    
    // ========== 统计信息 ==========
    /// 创建时间
    pub created_at: BlockNumber,
    /// 最后更新时间
    pub updated_at: BlockNumber,
    /// 累计销售额 (所有 Shop 汇总)
    pub total_sales: Balance,
    /// 累计订单数 (所有 Shop 汇总)
    pub total_orders: u64,
}

/// 实体类型
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum EntityType {
    #[default]
    Merchant,        // 商户 (个体/小微)
    Enterprise,      // 企业
    DAO,             // 去中心化自治组织
    Community,       // 社区
    Project,         // 项目方
    ServiceProvider, // 服务提供商
    Fund,            // 基金
    Cooperative,     // 合作社
    Franchise,       // 特许经营总部
    Custom(u8),      // 自定义
}

/// 实体状态
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum EntityStatus {
    #[default]
    Pending,     // 待审核
    Active,      // 活跃
    Suspended,   // 暂停
    Closing,     // 关闭中
    Closed,      // 已关闭
    Banned,      // 已封禁
}
```

### 3.2 Shop 结构 (业务层)

```rust
/// 店铺 (业务层)
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct Shop<AccountId, Balance, BlockNumber, MaxNameLen, MaxCidLen, MaxManagers> {
    // ========== 基础信息 ==========
    /// Shop ID (全局唯一)
    pub id: u64,
    /// 所属 Entity ID
    pub entity_id: u64,
    /// Shop 名称
    pub name: BoundedVec<u8, MaxNameLen>,
    /// Logo IPFS CID
    pub logo_cid: Option<BoundedVec<u8, MaxCidLen>>,
    /// 描述 IPFS CID
    pub description_cid: Option<BoundedVec<u8, MaxCidLen>>,
    
    // ========== 类型与状态 ==========
    /// Shop 类型
    pub shop_type: ShopType,
    /// Shop 状态
    pub status: ShopStatus,
    /// 是否为主 Shop (每个 Entity 有且仅有一个)
    pub is_primary: bool,
    
    // ========== 管理权限 ==========
    /// Shop 管理员 (Entity admin 自动有权限)
    pub managers: BoundedVec<AccountId, MaxManagers>,
    /// 客服账户
    pub customer_service: Option<AccountId>,
    
    // ========== 运营资金 ==========
    /// 运营账户 (派生账户)
    pub operating_account: AccountId,
    /// 初始运营资金
    pub initial_fund: Balance,
    /// 当前运营余额快照
    pub operating_balance: Balance,
    
    // ========== 会员配置 ==========
    /// 会员体系模式
    pub member_mode: MemberMode,
    /// 是否启用 Shop 级积分 (独立于 Entity 代币)
    pub points_enabled: bool,
    /// 积分配置 (如果启用)
    pub points_config: Option<PointsConfig>,
    
    // ========== 业务配置 ==========
    /// 返佣配置 ID (指向 commission 模块)
    pub commission_config_id: Option<u64>,
    /// 支持的支付方式
    pub payment_methods: PaymentMethods,
    
    // ========== 地理信息 (实体店) ==========
    /// 地理位置 (经度, 纬度) * 10^6
    pub location: Option<(i64, i64)>,
    /// 地址信息 CID
    pub address_cid: Option<BoundedVec<u8, MaxCidLen>>,
    /// 营业时间 CID
    pub business_hours_cid: Option<BoundedVec<u8, MaxCidLen>>,
    
    // ========== 统计信息 ==========
    /// 创建时间
    pub created_at: BlockNumber,
    /// 商品/服务数量
    pub product_count: u32,
    /// 累计销售额
    pub total_sales: Balance,
    /// 累计订单数
    pub total_orders: u32,
    /// 评分 (0-500 = 0.0-5.0)
    pub rating: u16,
    /// 评价数量
    pub rating_count: u32,
}

/// Shop 类型
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum ShopType {
    #[default]
    OnlineStore,     // 线上商城
    PhysicalStore,   // 实体门店
    ServicePoint,    // 服务网点
    Warehouse,       // 仓储/自提点
    Franchise,       // 加盟店
    Popup,           // 快闪店/临时店
    Virtual,         // 虚拟店铺 (纯服务)
}

/// Shop 状态
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum ShopStatus {
    #[default]
    Pending,         // 待激活
    Active,          // 营业中
    Paused,          // 暂停营业
    FundDepleted,    // 资金耗尽
    Closing,         // 关闭中
    Closed,          // 已关闭
}

/// 会员体系模式
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum MemberMode {
    #[default]
    Inherit,         // 继承 Entity 会员体系
    Independent,     // 独立会员体系
    Hybrid,          // 混合模式 (Entity + Shop 双重会员)
}

/// 积分配置
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct PointsConfig {
    /// 积分名称
    pub name: BoundedVec<u8, ConstU32<32>>,
    /// 积分符号
    pub symbol: BoundedVec<u8, ConstU32<8>>,
    /// 购物返积分比例 (基点)
    pub reward_rate: u16,
    /// 积分兑换比例 (基点)
    pub exchange_rate: u16,
    /// 积分是否可转让
    pub transferable: bool,
}
```

### 3.3 关联关系

```rust
// Storage Maps 设计

/// Entity 存储 entity_id -> Entity
#[pallet::storage]
pub type Entities<T: Config> = StorageMap<_, Blake2_128Concat, u64, EntityOf<T>>;

/// Shop 存储 shop_id -> Shop
#[pallet::storage]
pub type Shops<T: Config> = StorageMap<_, Blake2_128Concat, u64, ShopOf<T>>;

/// Entity -> Shops 索引 entity_id -> Vec<shop_id>
#[pallet::storage]
pub type EntityShops<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    u64,
    BoundedVec<u64, T::MaxShopsPerEntity>,
    ValueQuery,
>;

/// Shop -> Entity 反向索引 shop_id -> entity_id
#[pallet::storage]
pub type ShopEntity<T: Config> = StorageMap<_, Blake2_128Concat, u64, u64>;

/// Owner -> Entities 索引 account -> Vec<entity_id>
#[pallet::storage]
pub type OwnerEntities<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    BoundedVec<u64, T::MaxEntitiesPerOwner>,
    ValueQuery,
>;
```

---

## 4. Trait 接口设计

### 4.1 EntityProvider (组织层)

```rust
/// Entity 提供者接口
pub trait EntityProvider<AccountId> {
    // ========== 查询方法 ==========
    /// Entity 是否存在
    fn entity_exists(entity_id: u64) -> bool;
    
    /// Entity 是否活跃
    fn is_entity_active(entity_id: u64) -> bool;
    
    /// 获取 Entity 所有者
    fn entity_owner(entity_id: u64) -> Option<AccountId>;
    
    /// 获取 Entity 金库账户
    fn entity_treasury(entity_id: u64) -> Option<AccountId>;
    
    /// 获取 Entity 类型
    fn entity_type(entity_id: u64) -> Option<EntityType>;
    
    /// 获取 Entity 治理模式
    fn governance_mode(entity_id: u64) -> Option<GovernanceMode>;
    
    /// 检查是否为 Entity 管理员
    fn is_entity_admin(entity_id: u64, account: &AccountId) -> bool;
    
    /// 获取 Entity 下所有 Shop IDs
    fn entity_shops(entity_id: u64) -> Vec<u64>;
    
    // ========== 写入方法 ==========
    /// 更新 Entity 统计
    fn update_entity_stats(entity_id: u64, sales: u128, orders: u64);
    
    /// 暂停 Entity
    fn pause_entity(entity_id: u64) -> DispatchResult;
    
    /// 恢复 Entity
    fn resume_entity(entity_id: u64) -> DispatchResult;
}
```

### 4.2 ShopProvider (业务层)

```rust
/// Shop 提供者接口
pub trait ShopProvider<AccountId> {
    // ========== 查询方法 ==========
    /// Shop 是否存在
    fn shop_exists(shop_id: u64) -> bool;
    
    /// Shop 是否活跃
    fn is_shop_active(shop_id: u64) -> bool;
    
    /// 获取 Shop 所属 Entity ID
    fn shop_entity(shop_id: u64) -> Option<u64>;
    
    /// 获取 Shop 运营账户
    fn shop_account(shop_id: u64) -> Option<AccountId>;
    
    /// 获取 Shop 类型
    fn shop_type(shop_id: u64) -> Option<ShopType>;
    
    /// 检查是否为 Shop 管理员
    fn is_shop_manager(shop_id: u64, account: &AccountId) -> bool;
    
    /// 获取 Shop 会员模式
    fn member_mode(shop_id: u64) -> MemberMode;
    
    // ========== 写入方法 ==========
    /// 更新 Shop 统计
    fn update_shop_stats(shop_id: u64, sales: u128, orders: u32);
    
    /// 暂停 Shop
    fn pause_shop(shop_id: u64) -> DispatchResult;
    
    /// 恢复 Shop
    fn resume_shop(shop_id: u64) -> DispatchResult;
    
    /// 扣减运营资金
    fn deduct_operating_fund(shop_id: u64, amount: u128, fee_type: FeeType) -> DispatchResult;
}
```

### 4.3 跨层级查询

```rust
/// 统一查询接口 (向后兼容)
pub trait UnifiedProvider<AccountId> {
    /// 通过 Shop ID 获取 Entity 所有者 (常用快捷方法)
    fn shop_owner(shop_id: u64) -> Option<AccountId> {
        Self::shop_entity(shop_id)
            .and_then(|entity_id| Self::entity_owner(entity_id))
    }
    
    /// 检查账户是否有 Shop 管理权限 (含 Entity admin)
    fn can_manage_shop(shop_id: u64, account: &AccountId) -> bool {
        if Self::is_shop_manager(shop_id, account) {
            return true;
        }
        Self::shop_entity(shop_id)
            .map(|entity_id| Self::is_entity_admin(entity_id, account))
            .unwrap_or(false)
    }
}
```

---

## 5. 模块交互设计

### 5.1 会员体系

```
┌─────────────────────────────────────────────────────────────┐
│                      会员体系架构                            │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  MemberMode::Inherit (继承模式)                              │
│  ┌────────────────────────────────────────────────────┐     │
│  │  Entity A                                          │     │
│  │  └── 会员体系 (所有 Shop 共享)                     │     │
│  │       ├── Shop A1 的会员 → Entity A 会员           │     │
│  │       ├── Shop A2 的会员 → Entity A 会员           │     │
│  │       └── Shop A3 的会员 → Entity A 会员           │     │
│  └────────────────────────────────────────────────────┘     │
│                                                              │
│  MemberMode::Independent (独立模式)                          │
│  ┌────────────────────────────────────────────────────┐     │
│  │  Entity B                                          │     │
│  │  ├── Shop B1 会员体系 (独立)                       │     │
│  │  └── Shop B2 会员体系 (独立)                       │     │
│  └────────────────────────────────────────────────────┘     │
│                                                              │
│  MemberMode::Hybrid (混合模式)                               │
│  ┌────────────────────────────────────────────────────┐     │
│  │  Entity C                                          │     │
│  │  └── Entity 会员 + Shop 会员 (双重身份)            │     │
│  │       ├── Entity 会员: 全局权益 (代币分红等)       │     │
│  │       └── Shop 会员: 门店权益 (折扣、积分等)       │     │
│  └────────────────────────────────────────────────────┘     │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### 5.2 代币与积分

```
┌─────────────────────────────────────────────────────────────┐
│                     代币/积分架构                            │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  Entity 代币 (pallet-entity-token)                          │
│  ┌────────────────────────────────────────────────────┐     │
│  │  • 由 Entity 发行                                  │     │
│  │  • 支持 Governance/Equity/Share 等类型            │     │
│  │  • 可在 Market 交易                               │     │
│  │  • 所有 Shop 可使用 (作为支付方式)                │     │
│  └────────────────────────────────────────────────────┘     │
│                                                              │
│  Shop 积分 (pallet-entity-shop 内置)                        │
│  ┌────────────────────────────────────────────────────┐     │
│  │  • 由 Shop 发行 (可选)                            │     │
│  │  • 仅限 Points 类型                               │     │
│  │  • 仅限 Shop 内使用                               │     │
│  │  • 不可在 Market 交易                             │     │
│  │  • 可设置兑换为 Entity 代币                       │     │
│  └────────────────────────────────────────────────────┘     │
│                                                              │
│  兑换关系:                                                   │
│  Shop 积分 ──(exchange_rate)──> Entity 代币                 │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### 5.3 返佣流转

```
┌─────────────────────────────────────────────────────────────┐
│                       返佣流转                               │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  订单支付流程:                                               │
│                                                              │
│  买家 ──支付──> Shop 运营账户                                │
│                     │                                        │
│                     ├──(平台手续费)──> 平台金库              │
│                     │                                        │
│                     ├──(Entity分成)──> Entity 金库           │
│                     │                                        │
│                     ├──(返佣池)──> 返佣分配                  │
│                     │      │                                 │
│                     │      ├── 直推奖励                      │
│                     │      ├── 多级分销                      │
│                     │      ├── 级差返佣                      │
│                     │      └── ...                           │
│                     │                                        │
│                     └──(卖家收入)──> 卖家账户                │
│                                                              │
│  返佣归属:                                                   │
│  • MemberMode::Inherit  → 返佣在 Entity 层统一管理          │
│  • MemberMode::Independent → 返佣在 Shop 层独立管理         │
│  • MemberMode::Hybrid → 可配置分配比例                      │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 6. Extrinsics 设计

### 6.1 Entity 管理

```rust
// pallet-entity-registry

/// 创建 Entity
#[pallet::call_index(0)]
pub fn create_entity(
    origin: OriginFor<T>,
    name: BoundedVec<u8, T::MaxNameLen>,
    entity_type: EntityType,
    governance_mode: GovernanceMode,
    logo_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
    description_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
) -> DispatchResult;

/// 更新 Entity 信息
#[pallet::call_index(1)]
pub fn update_entity(
    origin: OriginFor<T>,
    entity_id: u64,
    name: Option<BoundedVec<u8, T::MaxNameLen>>,
    logo_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
    description_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
    metadata_uri: Option<BoundedVec<u8, T::MaxCidLen>>,
) -> DispatchResult;

/// 添加 Entity 管理员
#[pallet::call_index(2)]
pub fn add_entity_admin(
    origin: OriginFor<T>,
    entity_id: u64,
    admin: T::AccountId,
) -> DispatchResult;

/// 移除 Entity 管理员
#[pallet::call_index(3)]
pub fn remove_entity_admin(
    origin: OriginFor<T>,
    entity_id: u64,
    admin: T::AccountId,
) -> DispatchResult;

/// 向 Entity 金库充值
#[pallet::call_index(4)]
pub fn fund_entity_treasury(
    origin: OriginFor<T>,
    entity_id: u64,
    amount: BalanceOf<T>,
) -> DispatchResult;

/// 更新 Entity 治理模式 (需通过治理)
#[pallet::call_index(5)]
pub fn update_governance_mode(
    origin: OriginFor<T>,
    entity_id: u64,
    governance_mode: GovernanceMode,
) -> DispatchResult;
```

### 6.2 Shop 管理

```rust
// pallet-entity-shop (新模块)

/// 创建 Shop
#[pallet::call_index(0)]
pub fn create_shop(
    origin: OriginFor<T>,
    entity_id: u64,
    name: BoundedVec<u8, T::MaxNameLen>,
    shop_type: ShopType,
    member_mode: MemberMode,
    logo_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
    description_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
    initial_fund: BalanceOf<T>,
) -> DispatchResult;

/// 更新 Shop 信息
#[pallet::call_index(1)]
pub fn update_shop(
    origin: OriginFor<T>,
    shop_id: u64,
    name: Option<BoundedVec<u8, T::MaxNameLen>>,
    logo_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
    description_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
) -> DispatchResult;

/// 添加 Shop 管理员
#[pallet::call_index(2)]
pub fn add_shop_manager(
    origin: OriginFor<T>,
    shop_id: u64,
    manager: T::AccountId,
) -> DispatchResult;

/// 移除 Shop 管理员
#[pallet::call_index(3)]
pub fn remove_shop_manager(
    origin: OriginFor<T>,
    shop_id: u64,
    manager: T::AccountId,
) -> DispatchResult;

/// 向 Shop 充值运营资金
#[pallet::call_index(4)]
pub fn fund_shop_operating(
    origin: OriginFor<T>,
    shop_id: u64,
    amount: BalanceOf<T>,
) -> DispatchResult;

/// 暂停 Shop
#[pallet::call_index(5)]
pub fn pause_shop(
    origin: OriginFor<T>,
    shop_id: u64,
) -> DispatchResult;

/// 恢复 Shop
#[pallet::call_index(6)]
pub fn resume_shop(
    origin: OriginFor<T>,
    shop_id: u64,
) -> DispatchResult;

/// 设置 Shop 位置信息 (实体店)
#[pallet::call_index(7)]
pub fn set_shop_location(
    origin: OriginFor<T>,
    shop_id: u64,
    location: Option<(i64, i64)>,
    address_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
    business_hours_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
) -> DispatchResult;

/// 启用 Shop 积分
#[pallet::call_index(8)]
pub fn enable_shop_points(
    origin: OriginFor<T>,
    shop_id: u64,
    config: PointsConfig,
) -> DispatchResult;

/// 关闭 Shop
#[pallet::call_index(9)]
pub fn close_shop(
    origin: OriginFor<T>,
    shop_id: u64,
) -> DispatchResult;
```

---

## 7. 开发说明

> **注意**: 主网尚未上线，无需数据迁移。直接实现新架构即可。

### 7.1 开发策略

1. **新建模块**: 创建 `pallet-entity-shop` 作为业务层模块
2. **重构现有模块**: 修改 `pallet-entity-registry` 为组织层模块
3. **更新依赖**: 各业务模块改为依赖 `ShopProvider` trait

### 7.2 ID 分配策略

```rust
// Entity 和 Shop 使用独立的 ID 序列
#[pallet::storage]
pub type NextEntityId<T> = StorageValue<_, u64, ValueQuery>;

#[pallet::storage]
pub type NextShopId<T> = StorageValue<_, u64, ValueQuery>;
```

### 7.3 Primary Shop 自动创建

```rust
/// 创建 Entity 时自动创建 Primary Shop
pub fn create_entity(...) -> DispatchResult {
    // 1. 创建 Entity
    let entity_id = Self::next_entity_id();
    let entity = Entity { id: entity_id, ... };
    Entities::<T>::insert(entity_id, entity);
    
    // 2. 自动创建 Primary Shop
    let shop_id = Self::next_shop_id();
    let shop = Shop {
        id: shop_id,
        entity_id,
        is_primary: true,
        ..Default::default()
    };
    Shops::<T>::insert(shop_id, shop);
    
    // 3. 建立关联
    EntityShops::<T>::mutate(entity_id, |shops| shops.try_push(shop_id));
    
    Ok(())
}
```

---

## 8. 实施计划

### 8.1 Phase 1: 基础架构 (Week 1-2)

#### Step 1.1: 更新 pallet-entity-common (Day 1)

```
pallets/entity/common/src/lib.rs
├── 新增 ShopType 枚举
├── 新增 ShopStatus 枚举  
├── 新增 MemberMode 枚举
├── 新增 ShopProvider trait
└── 新增 NullShopProvider 空实现
```

**任务清单:**
- [x] 定义 `ShopType` 枚举 (OnlineStore, PhysicalStore, ServicePoint, Franchise, Popup, Virtual) ✅
- [x] 定义 `ShopOperatingStatus` 枚举 (Pending, Active, Paused, FundDepleted, Closing, Closed) ✅
- [x] 定义 `MemberMode` 枚举 (Inherit, Independent, Hybrid) ✅
- [x] 定义 `ShopProvider` trait (shop_exists, is_shop_active, shop_entity, shop_account, etc.) ✅
- [x] 实现 `NullShopProvider` 空实现 ✅

#### Step 1.2: 创建 pallet-entity-shop 模块 (Day 2-4)

```
pallets/entity/shop/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs          # 主模块
    ├── types.rs        # Shop 结构体、PointsConfig
    ├── weights.rs      # 权重定义
    ├── mock.rs         # 测试 mock
    └── tests.rs        # 单元测试
```

**任务清单:**
- [x] 创建模块目录结构和 Cargo.toml ✅
- [x] 定义 `Shop` 结构体 (id, entity_id, name, shop_type, status, managers, etc.) ✅
- [x] 定义 `PointsConfig` 结构体 (name, symbol, reward_rate, exchange_rate, transferable) ✅
- [x] 定义 Storage Maps: ✅
  - `Shops`: shop_id -> Shop
  - `ShopEntity`: shop_id -> entity_id (反向索引)
  - `NextShopId`: u64
  - `ShopPointsBalances`: (shop_id, account) -> balance (Shop 积分)
- [x] 定义 Events 和 Errors ✅
- [x] 实现 `ShopProvider` trait for `Pallet<T>` ✅

#### Step 1.3: 实现 Shop Extrinsics (Day 5-7)

**任务清单:**
- [x] `create_shop(entity_id, name, shop_type, member_mode, initial_fund)` - 创建 Shop ✅
- [x] `update_shop(shop_id, name, logo_cid, description_cid)` - 更新 Shop 信息 ✅
- [x] `add_manager(shop_id, manager)` - 添加管理员 ✅
- [x] `remove_manager(shop_id, manager)` - 移除管理员 ✅
- [x] `fund_operating(shop_id, amount)` - 充值运营资金 ✅
- [x] `pause_shop(shop_id)` / `resume_shop(shop_id)` - 暂停/恢复 ✅
- [x] `set_location(shop_id, location, address_cid)` - 设置位置 (实体店) ✅
- [x] `enable_points(shop_id, config)` - 启用 Shop 积分 ✅
- [x] `close_shop(shop_id)` - 关闭 Shop ✅

---

### 8.2 Phase 2: 重构 Registry 模块 (Week 2-3)

#### Step 2.1: 拆分 Entity 结构 (Day 8-9)

**当前 Entity 结构:**
```rust
// 需要移除的字段 (迁移到 Shop)
- product_count      // -> Shop.product_count
- total_sales        // -> Shop.total_sales (汇总到 Entity)
- total_orders       // -> Shop.total_orders (汇总到 Entity)
- rating             // -> Shop.rating
- rating_count       // -> Shop.rating_count
```

**新增字段:**
```rust
// Entity 新增
+ shop_ids: BoundedVec<u64, MaxShops>  // 下属 Shop 列表
+ max_shops: u32                        // 最大 Shop 数量
```

**任务清单:**
- [x] 更新 `Entity` 结构体，移除业务字段 ✅
- [x] 添加 `shop_ids` 和 `primary_shop_id` 字段 ✅
- [x] 更新 `EntityOf` 类型别名 ✅
- [x] 添加 `EntityShops` Storage Map: entity_id -> Vec<shop_id> ✅

#### Step 2.2: 更新 Entity 创建流程 (Day 10-11)

**任务清单:**
- [x] 修改 `create_shop` extrinsic: ✅
  - 创建 Entity 后自动分配 Primary Shop ID
  - 设置 `primary_shop_id`
  - 建立 Entity-Shop 关联
- [ ] 添加 `set_max_shops(entity_id, max)` extrinsic (治理调用)
- [ ] 添加权限检查: Entity admin 自动成为所有 Shop 的管理员

#### Step 2.3: 更新 EntityProvider trait (Day 12)

**任务清单:**
- [ ] 添加 `entity_shops(entity_id) -> Vec<u64>` 方法
- [ ] 添加 `primary_shop(entity_id) -> Option<u64>` 方法
- [ ] 更新统计汇总逻辑 (从 Shops 聚合到 Entity)

---

### 8.3 Phase 3: 业务模块适配 (Week 3-4)

#### Step 3.1: 更新 member 模块 (Day 13-15)

**核心变更:** 支持三种 MemberMode

```rust
MemberMode::Inherit      // 会员数据存储在 Entity 级别
MemberMode::Independent  // 会员数据存储在 Shop 级别
MemberMode::Hybrid       // 双层会员体系
```

**任务清单:**
- [x] 添加 `MemberScope` 枚举 (Entity, Shop) ✅
- [x] 添加 `ShopMembers` Storage 支持 Shop 级别会员 ✅
- [x] 添加 `ShopMemberCount` Storage ✅
- [x] 添加 `ShopProvider` 到 Config trait ✅
- [ ] 更新 `register_member` 逻辑:
  - Inherit: 注册到 Entity
  - Independent: 注册到 Shop
  - Hybrid: 同时注册到 Entity 和 Shop
- [ ] 更新 `MemberProvider` trait 方法签名

#### Step 3.2: 更新 commission 模块 (Day 16-18)

**核心变更:** 返佣配置和计算基于 Shop

**任务清单:**
- [x] 添加 `ShopProvider` 到 Config trait ✅
- [ ] 修改 `EntityCommissionConfigs` -> 基于 shop_id
- [ ] 添加 Entity 级别返佣汇总:
  - `EntityCommissionTotals`: entity_id -> total
- [ ] 更新返佣计算逻辑:
  - 订单在 Shop 产生
  - 返佣按 Shop 配置计算
  - 汇总到 Entity 统计
- [ ] 支持跨 Shop 返佣 (可选配置)

#### Step 3.3: 更新 service 模块 (Day 19-20)

**核心变更:** 商品/服务归属到 Shop

**任务清单:**
- [x] 确认 `Products` Storage 已使用 shop_id ✅
- [x] 添加 `ShopProvider` 到 Config trait ✅
- [ ] 添加 Shop 活跃状态检查
- [ ] 更新权限检查: Shop manager 或 Entity admin

#### Step 3.4: 更新 transaction 模块 (Day 21-22)

**核心变更:** 订单关联到 Shop

**任务清单:**
- [x] 确认 `Orders` Storage 已使用 shop_id ✅
- [x] 添加 `ShopProvider` 到 Config trait ✅
- [ ] 订单支付流向 Shop 运营账户
- [ ] 更新统计: Shop 级别 + Entity 汇总
- [ ] 添加 `shop_id` 到订单事件

#### Step 3.5: 更新其他模块 (Day 23-24)

**token 模块:**
- [ ] 确认代币归属 Entity (不变)
- [ ] Shop 可使用 Entity 代币作为支付方式

**governance 模块:**
- [ ] 确认治理在 Entity 级别 (不变)
- [ ] 添加 Shop 相关治理提案类型 (可选)

**review 模块:**
- [ ] 支持 Shop 级别评价
- [ ] 评分汇总到 Entity

---

### 8.4 Phase 4: 测试与文档 (Week 5)

#### Step 4.1: 单元测试 (Day 25-26)

**任务清单:**
- [ ] pallet-entity-shop 单元测试 (>80% 覆盖率)
- [ ] pallet-entity-registry 更新后的单元测试
- [ ] 跨模块调用测试

#### Step 4.2: 集成测试 (Day 27-28)

**测试场景:**
- [ ] 创建 Entity + Primary Shop
- [ ] 创建多个 Shop
- [ ] 不同 MemberMode 下的会员注册
- [ ] 跨 Shop 返佣计算
- [ ] Shop 暂停/关闭对业务的影响

#### Step 4.3: 文档更新 (Day 29-30)

**任务清单:**
- [x] 更新 README.md 文件 ✅
- [ ] 更新 API 文档
- [ ] 编写使用示例
- [ ] 更新架构图

---

### 8.5 里程碑检查点

| 里程碑 | 预计完成 | 验收标准 |
|--------|---------|---------|
| M1: Shop 模块可用 | Week 1 末 | Shop CRUD 通过测试 |
| M2: Entity-Shop 关联 | Week 2 末 | 创建 Entity 自动创建 Shop |
| M3: 会员模块适配 | Week 3 末 | 三种 MemberMode 可用 |
| M4: 全模块适配 | Week 4 末 | 所有模块编译通过 |
| M5: 测试完成 | Week 5 末 | 测试覆盖率 >80% |

---

### 8.6 依赖关系图

```
Week 1                    Week 2                    Week 3-4                 Week 5
──────                    ──────                    ────────                 ──────

┌─────────────┐
│   common    │
│ ShopType    │
│ ShopProvider│
└──────┬──────┘
       │
       ▼
┌─────────────┐     ┌─────────────┐
│    shop     │────▶│  registry   │
│ Shop struct │     │ Entity 1:N  │
│ CRUD ops    │     │ Primary Shop│
└─────────────┘     └──────┬──────┘
                           │
                           ▼
                    ┌─────────────┐     ┌─────────────┐
                    │   member    │────▶│   tests     │
                    │ MemberMode  │     │   docs      │
                    └──────┬──────┘     └─────────────┘
                           │
                    ┌──────┴──────┐
                    ▼             ▼
             ┌───────────┐ ┌───────────┐
             │commission │ │service    │
             │transaction│ │review     │
             └───────────┘ └───────────┘
```

---

## 9. 风险评估

| 风险 | 等级 | 缓解措施 |
|------|------|---------|
| 开发周期长 | � 中 | 分阶段实施，MVP 优先 |
| 接口设计不合理 | 🟡 中 | 充分设计评审，预留扩展点 |
| 性能下降 | 🟡 中 | 增加索引，优化查询路径 |
| 复杂度增加 | 🟡 中 | 完善文档，代码审查 |
| 前端改动大 | 🟡 中 | 分阶段发布，API 版本控制 |

---

## 10. 附录

### 10.1 术语表

| 术语 | 定义 |
|------|------|
| **Entity** | 组织实体，代表法人或 DAO，负责治理、代币、合规 |
| **Shop** | 业务单元，代表具体经营场所或线上店铺 |
| **Primary Shop** | 每个 Entity 默认的主 Shop，创建 Entity 时自动创建 |
| **Treasury** | Entity 金库账户，用于治理资金、代币池 |
| **Operating Account** | Shop 运营账户，用于日常经营费用 |

### 10.2 参考资料

- [Polkadot SDK Runtime Storage](https://paritytech.github.io/polkadot-sdk/master/frame_support/storage/index.html)
- [Multi-tenant SaaS Architecture](https://docs.microsoft.com/en-us/azure/architecture/guide/multitenant/overview)
- [DAO Organizational Structures](https://ethereum.org/en/dao/)

---

**文档结束**
