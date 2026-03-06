# NEXUS 全平台前端设计方案

> 基于 Cosmos 链端 6 大 Pallet 系统（38 个子模块，~600+ Extrinsics）的完整前端架构设计
> 深度分析链端接口，精确映射每个 Extrinsic / Storage / Event

---

## 1. 链端模块全景

| Pallet 系统 | 子模块数 | Extrinsics | 核心能力 |
|-------------|---------|-----------|---------|
| **Entity** (实体商业) | 20 | ~270 | 实体注册、店铺、商品、订单、评价、代币、内部市场、会员、佣金(7种)、治理、披露、KYC、代币发售 |
| **Trading** (NEX 交易) | 3 | ~27 | USDT/NEX P2P OTC 市场、TRC20 链下验证、价格预言机 |
| **Storage** (IPFS 存储) | 2 | ~60 | IPFS Pin 管理、运营商网络、分层存储、计费结算、数据生命周期 |
| **GroupRobot** (机器人) | 7 | ~122 | Bot 注册、TEE 远程证明、社区管理、共识节点、奖励分配、订阅计费 |
| **Dispute** (争议仲裁) | 3 | ~65 | 投诉仲裁、资金托管、证据链管理、隐私内容 |
| **Ads** (广告系统) | 5 | ~79 | 广告活动管理、Entity/GroupRobot 广告位、社区广告质押、收益分配 |
| **合计** | **38** | **~623** | — |

---

## 2. 技术栈

| 层级 | 选型 | 说明 |
|------|------|------|
| **框架** | Next.js 14 (App Router) | SSR + CSR 混合，SEO 友好 |
| **UI 库** | shadcn/ui + Radix UI | 无头组件，完全可定制 |
| **样式** | Tailwind CSS 4 | 原子化 CSS |
| **图标** | Lucide React | 轻量一致的图标集 |
| **状态管理** | Zustand + React Query | 链上状态缓存 + 乐观更新 |
| **链交互** | Polkadot.js API (@polkadot/api) | Substrate RPC + Extrinsic 签名 |
| **钱包** | @polkadot/extension-dapp | Polkadot.js / Talisman / SubWallet |
| **图表** | Recharts | 数据可视化 |
| **表单** | React Hook Form + Zod | 类型安全表单验证 |
| **IPFS** | ipfs-http-client / Pinata SDK | CID 内容上传/解析 |
| **国际化** | next-intl | 中英文切换 |
| **类型** | TypeScript 5 | 全量类型覆盖 |

---

## 3. 整体架构

```
┌──────────────────────────────────────────────────────────────────────┐
│                       Next.js App Router                              │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐             │
│  │  Zustand  │  │  React   │  │ Polkadot │  │   IPFS   │             │
│  │  Store    │  │  Query   │  │  API     │  │  Gateway │             │
│  └─────┬────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘             │
│        │            │              │              │                    │
│  ┌─────┴────────────┴──────────────┴──────────────┴──────────────┐   │
│  │                Chain Adapter Layer (hooks/)                    │   │
│  │                                                               │   │
│  │  Entity:   useEntity / useShop / useToken / useOrder / ...    │   │
│  │  Trading:  useNexMarket / useUsdtTrade                        │   │
│  │  Storage:  useStorageService / useStorageLifecycle             │   │
│  │  Robot:    useBot / useCeremony / useCommunity / useConsensus  │   │
│  │  Dispute:  useArbitration / useEscrow / useEvidence            │   │
│  │  Ads:      useAdCampaign / useAdPlacement / useAdStaking      │   │
│  └──────────────────────┬────────────────────────────────────────┘   │
│                         │                                            │
├─────────────────────────┼────────────────────────────────────────────┤
│              Substrate Node (WebSocket RPC)                          │
│              ws://localhost:9944                                      │
└──────────────────────────────────────────────────────────────────────┘
```

### 3.1 目录结构

```
frontend/
├── src/
│   ├── app/                          # Next.js App Router 页面
│   │   ├── layout.tsx                # 根布局（顶栏 + 底部 Tab/侧栏）
│   │   ├── page.tsx                  # 首页 → 全局仪表盘
│   │   │
│   │   ├── entity/                   # ═══ Entity 商业系统 ═══
│   │   │   ├── settings/             # Entity 设置
│   │   │   ├── admins/               # 管理员权限管理
│   │   │   ├── fund/                 # 运营资金管理
│   │   │   ├── shops/
│   │   │   │   ├── [shopId]/
│   │   │   │   │   ├── products/     # 商品管理
│   │   │   │   │   ├── orders/       # 订单管理
│   │   │   │   │   ├── reviews/      # 评价管理
│   │   │   │   │   └── points/       # 积分系统
│   │   │   │   └── page.tsx          # Shop 列表
│   │   │   ├── token/
│   │   │   │   ├── config/           # 代币配置
│   │   │   │   ├── holders/          # 持有人管理
│   │   │   │   ├── dividend/         # 分红管理
│   │   │   │   ├── transfer/         # 转账限制
│   │   │   │   └── lock/             # 锁仓/Vesting 管理
│   │   │   ├── market/               # Entity 内部代币市场
│   │   │   ├── members/
│   │   │   │   ├── list/             # 会员列表
│   │   │   │   ├── levels/           # 等级管理
│   │   │   │   ├── rules/            # 升级规则引擎
│   │   │   │   └── pending/          # 待审批会员
│   │   │   ├── commission/
│   │   │   │   ├── config/           # 佣金模式配置
│   │   │   │   ├── referral/         # 直推佣金
│   │   │   │   ├── multi-level/      # 多级佣金
│   │   │   │   ├── level-diff/       # 级差佣金
│   │   │   │   ├── single-line/      # 排线佣金
│   │   │   │   ├── team/             # 团队佣金
│   │   │   │   ├── pool-reward/      # 奖池佣金
│   │   │   │   └── withdraw/         # 提现管理
│   │   │   ├── governance/
│   │   │   │   ├── proposals/        # 提案列表
│   │   │   │   ├── vote/             # 投票
│   │   │   │   └── config/           # 治理配置
│   │   │   ├── disclosure/
│   │   │   │   ├── reports/          # 财务披露
│   │   │   │   ├── announcements/    # 公告管理
│   │   │   │   └── insiders/         # 内幕人员管理
│   │   │   ├── kyc/
│   │   │   │   ├── records/          # KYC 记录
│   │   │   │   ├── providers/        # 认证提供者
│   │   │   │   └── settings/         # KYC 要求配置
│   │   │   └── tokensale/
│   │   │       ├── rounds/           # 发售轮次
│   │   │       ├── [roundId]/        # 单轮详情
│   │   │       └── create/           # 创建发售
│   │   │
│   │   ├── trading/                  # ═══ NEX P2P 市场 ═══
│   │   │   ├── orderbook/            # 挂单列表（买单/卖单）
│   │   │   ├── my-orders/            # 我的挂单
│   │   │   ├── my-trades/            # 我的交易
│   │   │   ├── disputes/             # 交易争议
│   │   │   └── settings/             # 市场管理（admin）
│   │   │
│   │   ├── storage/                  # ═══ IPFS 存储 ═══
│   │   │   ├── pins/                 # Pin 管理
│   │   │   ├── operators/            # 运营商管理
│   │   │   ├── billing/              # 计费管理
│   │   │   ├── domains/              # 域名管理
│   │   │   └── lifecycle/            # 数据生命周期
│   │   │
│   │   ├── robot/                    # ═══ GroupRobot ═══
│   │   │   ├── bots/                 # Bot 管理
│   │   │   │   ├── [botId]/
│   │   │   │   │   ├── peers/        # P2P 节点
│   │   │   │   │   ├── attestation/  # TEE 远程证明
│   │   │   │   │   └── ceremony/     # 密钥仪式
│   │   │   │   └── page.tsx
│   │   │   ├── operators/            # 运营商管理
│   │   │   ├── communities/          # 社区管理
│   │   │   ├── nodes/                # 共识节点
│   │   │   ├── rewards/              # 奖励管理
│   │   │   └── subscriptions/        # 订阅管理
│   │   │
│   │   ├── dispute/                  # ═══ 争议仲裁 ═══
│   │   │   ├── complaints/           # 投诉列表
│   │   │   ├── arbitration/          # 仲裁中心
│   │   │   ├── escrow/               # 托管资金
│   │   │   └── evidence/             # 证据管理
│   │   │
│   │   └── ads/                      # ═══ 广告系统 ═══
│   │       ├── campaigns/            # 广告活动
│   │       ├── placements/           # 广告位管理
│   │       ├── staking/              # 社区广告质押
│   │       └── revenue/              # 收益管理
│   │
│   ├── components/
│   │   ├── ui/                       # shadcn/ui 基础组件
│   │   ├── layout/                   # 布局组件
│   │   ├── entity/                   # Entity 业务组件
│   │   ├── trading/                  # Trading 业务组件
│   │   ├── storage/                  # Storage 业务组件
│   │   ├── robot/                    # Robot 业务组件
│   │   ├── dispute/                  # Dispute 业务组件
│   │   ├── ads/                      # Ads 业务组件
│   │   └── shared/                   # 通用组件
│   │
│   ├── hooks/
│   │   ├── useApi.ts                 # Polkadot API 连接
│   │   ├── useWallet.ts              # 钱包连接/签名
│   │   ├── useTx.ts                  # 通用交易提交
│   │   ├── entity/                   # Entity 系列 hooks
│   │   ├── trading/                  # Trading hooks
│   │   ├── storage/                  # Storage hooks
│   │   ├── robot/                    # Robot hooks
│   │   ├── dispute/                  # Dispute hooks
│   │   └── ads/                      # Ads hooks
│   │
│   ├── lib/
│   │   ├── api.ts                    # Polkadot API 单例
│   │   ├── ipfs.ts                   # IPFS 上传/解析
│   │   ├── format.ts                 # 金额/地址/时间格式化
│   │   ├── types.ts                  # 链上类型映射
│   │   └── constants.ts              # 常量定义
│   │
│   ├── stores/
│   │   ├── entity.ts                 # Entity 全局状态
│   │   ├── wallet.ts                 # 钱包状态
│   │   └── ui.ts                     # UI 状态
│   │
│   └── styles/
│       └── globals.css
```

---

## 4. 核心数据类型（链端 → 前端映射）

以下类型定义来自 `pallets/entity/common/src/lib.rs`，前端须精确映射。

### 4.1 实体类型 (EntityType)

```typescript
enum EntityType {
  Merchant = 'Merchant',       // 商户（默认）
  Enterprise = 'Enterprise',   // 企业
  DAO = 'DAO',                 // 去中心化自治组织
  Community = 'Community',     // 社区
  Project = 'Project',         // 项目方
  ServiceProvider = 'ServiceProvider', // 服务提供商
  Fund = 'Fund',               // 基金
  Custom = 'Custom',           // 自定义类型 (u8 子类型)
}
```

### 4.2 实体状态 (EntityStatus)

```typescript
enum EntityStatus {
  Pending = 'Pending',         // 待审核（reopen 时使用，create 跳过直接 Active）
  Active = 'Active',           // 正常运营
  Suspended = 'Suspended',     // 暂停运营（管理员主动）
  Banned = 'Banned',           // 被封禁（治理处罚，不可恢复）
  Closed = 'Closed',           // 已关闭（不可恢复）
  PendingClose = 'PendingClose', // 待关闭审批
}
```

### 4.3 治理模式 (GovernanceMode)

```typescript
enum GovernanceMode {
  None = 'None',     // 无治理（管理员全权控制）
  FullDAO = 'FullDAO', // 完全 DAO（代币投票，可选否决权）
}
```

### 4.4 代币类型 (TokenType)

```typescript
enum TokenType {
  Points = 'Points',           // 积分（消费奖励，默认）
  Governance = 'Governance',   // 治理代币（投票权）
  Equity = 'Equity',           // 股权代币（分红权，证券类）
  Membership = 'Membership',   // 会员代币（会员资格）
  Share = 'Share',             // 份额代币（基金份额，证券类）
  Bond = 'Bond',               // 债券代币（固定收益，证券类）
  Hybrid = 'Hybrid',           // 混合型（u8 子类型）
}
```

### 4.5 转账限制模式 (TransferRestrictionMode)

```typescript
enum TransferRestrictionMode {
  None = 0,          // 无限制
  Whitelist = 1,     // 白名单模式
  Blacklist = 2,     // 黑名单模式
  KycRequired = 3,   // KYC 模式
  MembersOnly = 4,   // 闭环模式（成员间转账）
}
```

### 4.6 Shop 类型与状态

```typescript
enum ShopType {
  OnlineStore = 'OnlineStore',     // 线上商城（默认）
  PhysicalStore = 'PhysicalStore', // 实体门店
  ServicePoint = 'ServicePoint',   // 服务网点
  Warehouse = 'Warehouse',         // 仓储/自提点
  Franchise = 'Franchise',         // 加盟店
  Popup = 'Popup',                 // 快闪店
  Virtual = 'Virtual',             // 虚拟店铺（纯服务）
}

enum ShopOperatingStatus {
  Active = 'Active',           // 营业中
  Paused = 'Paused',           // 暂停营业
  FundDepleted = 'FundDepleted', // 资金耗尽（自动暂停）
  Closed = 'Closed',           // 已关闭
  Closing = 'Closing',         // 关闭中（宽限期）
  Banned = 'Banned',           // 被治理封禁
}

// EffectiveShopStatus = EntityStatus × ShopOperatingStatus 实时计算
enum EffectiveShopStatus {
  Active, PausedBySelf, PausedByEntity, FundDepleted,
  Closed, ClosedByEntity, Closing, Banned,
}
```

### 4.7 商品类型

```typescript
enum ProductCategory { Digital, Physical, Service, Subscription, Bundle, Other }
enum ProductStatus { Draft, OnSale, SoldOut, OffShelf }
enum ProductVisibility { Public, MembersOnly, LevelGated } // LevelGated(u8)
```

### 4.8 订单状态 (OrderStatus)

```typescript
enum OrderStatus {
  Created, Paid, Shipped, Completed,
  Cancelled, Disputed, Refunded, Expired,
}
enum PaymentAsset { Native, EntityToken }
```

### 4.9 Admin 权限位掩码

```typescript
const AdminPermission = {
  SHOP_MANAGE:       0x001,  // 店铺管理
  MEMBER_MANAGE:     0x002,  // 会员管理
  TOKEN_MANAGE:      0x004,  // 代币发售管理
  ADS_MANAGE:        0x008,  // 广告管理
  REVIEW_MANAGE:     0x010,  // 评论管理
  DISCLOSURE_MANAGE: 0x020,  // 披露/公告管理
  ENTITY_MANAGE:     0x040,  // 实体管理
  KYC_MANAGE:        0x080,  // KYC 要求管理
  GOVERNANCE_MANAGE: 0x100,  // 治理提案管理
  ORDER_MANAGE:      0x200,  // 订单管理
  COMMISSION_MANAGE: 0x400,  // 佣金配置管理
  ALL:               0xFFFFFFFF,
} as const;
```

### 4.10 会员相关

```typescript
// 注册策略（位标记，可组合）
const MemberRegistrationPolicy = {
  OPEN: 0,                     // 开放注册
  PURCHASE_REQUIRED: 0b00001,  // 必须先消费
  REFERRAL_REQUIRED: 0b00010,  // 必须有推荐人
  APPROVAL_REQUIRED: 0b00100,  // 需要审批
  KYC_REQUIRED:      0b01000,  // 需要 KYC
  KYC_UPGRADE_REQUIRED: 0b10000, // 升级需 KYC
} as const;

enum MemberStatus { Active, Pending, Frozen, Banned, Expired }
```

### 4.11 KYC 等级

```typescript
enum KycLevel { None = 0, Basic = 1, Standard = 2, Enhanced = 3, Full = 4 }
enum KycStatus { NotSubmitted = 0, Pending = 1, Approved = 2, Rejected = 3, Expired = 4, Revoked = 5 }
```

---

## 5. Entity 商业系统 — 链端接口深度映射

### 5.1 Entity Registry (pallet-entity-registry)

#### Storage 查询

| Storage | Key | Value | 前端用途 |
|---------|-----|-------|---------|
| `Entities` | `u64` (entity_id) | `EntityInfo<T>` | 实体详情页数据源 |
| `EntityCount` | — | `u64` | 全局统计 |
| `OwnerEntities` | `AccountId` | `BoundedVec<u64>` | 用户拥有的实体列表 |
| `EntityAdmins` | `(u64, AccountId)` | `u32` (permission_bits) | 管理员权限矩阵 |
| `AdminEntities` | `AccountId` | `BoundedVec<u64>` | 用户管理的实体列表 |
| `EntityReferrers` | `AccountId` | `AccountId` | 推荐人关系 |
| `EntityShops` | `u64` (entity_id) | `BoundedVec<u64>` | Entity 下属 Shop 列表 |
| `GovernanceLocked` | `u64` | `bool` | 治理锁定状态 |

#### Extrinsics

| 操作 | Extrinsic | 关键参数 | 权限 |
|------|-----------|---------|------|
| 创建实体 | `entityRegistry.createEntity` | `name, entity_type, governance_mode, shop_name, shop_type` | Signed |
| 更新实体 | `entityRegistry.updateEntity` | `entity_id, name?, logo_cid?, description_cid?, metadata_uri?` | Owner/Admin(ENTITY_MANAGE) |
| 升级类型 | `entityRegistry.upgradeEntityType` | `entity_id, new_type, new_governance?` | Owner |
| 添加管理员 | `entityRegistry.addAdmin` | `entity_id, admin, permissions: u32` | Owner |
| 移除管理员 | `entityRegistry.removeAdmin` | `entity_id, admin` | Owner |
| 更新权限 | `entityRegistry.updateAdminPermissions` | `entity_id, admin, new_permissions` | Owner |
| 充值资金 | `entityRegistry.topUpFund` | `entity_id, amount` | Owner/Admin(ENTITY_MANAGE) |
| 申请关闭 | `entityRegistry.requestCloseEntity` | `entity_id` | Owner |
| 重新开放 | `entityRegistry.reopenEntity` | `entity_id` | Owner (Closed 状态) |
| 转移所有权 | `entityRegistry.transferOwnership` | `entity_id, new_owner` | Owner (不可逆) |
| 绑定推荐人 | `entityRegistry.bindEntityReferrer` | `entity_id, referrer` | Owner (一次性) |
| 暂停实体 | `entityRegistry.suspendEntity` | `entity_id` | Governance |
| 恢复实体 | `entityRegistry.resumeEntity` | `entity_id` | Governance |
| 封禁实体 | `entityRegistry.banEntity` | `entity_id` | Root |
| 锁定治理 | `entityRegistry.lockGovernance` | `entity_id` | Governance (不可逆) |
| 设置治理模式 | `entityRegistry.setGovernanceMode` | `entity_id, mode` | Governance |
| 强制关闭 | `entityRegistry.forceCloseEntity` | `entity_id` | Root |
| 清理关闭 | `entityRegistry.cleanupClosedEntity` | `entity_id` | Signed |

#### Events

`EntityCreated`, `EntityUpdated`, `EntityTypeUpgraded`, `AdminAdded`, `AdminRemoved`, `AdminPermissionsUpdated`, `FundTopUp`, `EntitySuspended`, `EntityResumed`, `EntityBanned`, `EntityCloseRequested`, `EntityClosed`, `EntityReopened`, `OwnershipTransferred`, `GovernanceLocked`, `GovernanceModeChanged`, `ReferrerBound`, `EntitySuspendedLowFund`, `FundWarning`, `OperatingFeeDeducted`

---

### 5.2 Shop 管理 (pallet-entity-shop)

#### Storage 查询

| Storage | Key | Value |
|---------|-----|-------|
| `Shops` | `u64` (shop_id) | `ShopInfo<T>` |
| `ShopCount` | — | `u64` |
| `EntityShopIds` | `u64` (entity_id) | `BoundedVec<u64>` |
| `ShopManagers` | `(u64, AccountId)` | `bool` |
| `ShopOperatingFund` | `u64` | `Balance` |
| `ShopLocations` | `u64` | `ShopLocation<T>` |
| `PrimaryShop` | `u64` (entity_id) | `u64` (shop_id) |

#### Extrinsics

| 操作 | Extrinsic | 关键参数 | 权限 |
|------|-----------|---------|------|
| 创建店铺 | `entityShop.createShop` | `entity_id, name, shop_type, initial_fund` | Owner/Admin(SHOP_MANAGE) |
| 更新店铺 | `entityShop.updateShop` | `shop_id, name?, logo_cid?, description_cid?` | Owner/Manager |
| 暂停店铺 | `entityShop.pauseShop` | `shop_id` | Owner/Manager |
| 恢复店铺 | `entityShop.resumeShop` | `shop_id` | Owner/Manager |
| 关闭店铺 | `entityShop.closeShop` | `shop_id` | Owner (非 Primary) |
| 充值运营资金 | `entityShop.fundOperating` | `shop_id, amount` | Owner/Admin |
| 提取运营资金 | `entityShop.withdrawOperatingFund` | `shop_id, amount` | Owner |
| 设置位置 | `entityShop.setLocation` | `shop_id, latitude, longitude, address_cid, hours_cid` | Owner/Manager |
| 添加管理员 | `entityShop.addManager` | `shop_id, manager` | Owner |
| 移除管理员 | `entityShop.removeManager` | `shop_id, manager` | Owner |
| 启用积分 | `entityShop.enablePoints` | `shop_id, name, symbol` | Owner/Admin |
| 更新积分配置 | `entityShop.updatePointsConfig` | `shop_id, reward_rate_bps, exchange_rate_bps, transferable` | Owner/Admin |
| 禁用积分 | `entityShop.disablePoints` | `shop_id` | Owner |

---

### 5.3 商品管理 (pallet-entity-product)

#### Storage 查询

| Storage | Key | Value |
|---------|-----|-------|
| `Products` | `u64` (product_id) | `ProductInfo<T>` |
| `ShopProducts` | `u64` (shop_id) | `BoundedVec<u64>` |
| `NextProductId` | — | `u64` |

#### Extrinsics

| 操作 | Extrinsic | 关键参数 |
|------|-----------|---------|
| 创建商品 | `entityProduct.createProduct` | `shop_id, name_cid, images_cid, detail_cid, price, usdt_price, stock, category, visibility, min_order_qty, max_order_qty` |
| 更新商品 | `entityProduct.updateProduct` | `product_id, name_cid?, images_cid?, detail_cid?, price?, usdt_price?, stock?` |
| 上架 | `entityProduct.publishProduct` | `product_id` |
| 下架 | `entityProduct.unpublishProduct` | `product_id` |
| 删除 | `entityProduct.deleteProduct` | `product_id` |

**前端字段映射：**

| 字段 | 类型 | 验证 |
|------|------|------|
| 名称 | IPFS CID | 非空 |
| 图片 | IPFS CID | 非空 |
| 详情 | IPFS CID | 非空 |
| NEX 价格 | Balance | > 0 |
| USDT 价格 | u64 (精度 10^6) | ≥ 0 (0=不支持 USDT) |
| 库存 | u32 | 0=无限 |
| 类别 | ProductCategory 枚举 | 必选 |
| 可见性 | ProductVisibility 枚举 | 默认 Public |
| 最小购买量 | u32 | 默认 1 |
| 最大购买量 | u32 | 0=不限 |

---

### 5.4 订单管理 (pallet-entity-order)

#### Storage 查询

| Storage | Key | Value |
|---------|-----|-------|
| `Orders` | `u64` | `OrderInfo<T>` |
| `BuyerOrders` | `AccountId` | `BoundedVec<u64>` |
| `ShopOrders` | `u64` (shop_id) | `BoundedVec<u64>` |
| `NextOrderId` | — | `u64` |

#### Extrinsics

| 操作 | Extrinsic | 关键参数 | 角色 |
|------|-----------|---------|------|
| 下单 | `entityOrder.placeOrder` | `shop_id, product_id, quantity, payment_asset, use_shopping_balance, referrer?` | Buyer |
| 支付 | `entityOrder.payOrder` | `order_id` | Buyer |
| 取消 | `entityOrder.cancelOrder` | `order_id` | Buyer (Paid 前) |
| 发货 | `entityOrder.shipOrder` | `order_id, tracking_cid` | Seller |
| 确认收货 | `entityOrder.confirmReceipt` | `order_id` | Buyer |
| 申请退款 | `entityOrder.requestRefund` | `order_id, reason_cid` | Buyer |
| 批准退款 | `entityOrder.approveRefund` | `order_id` | Seller |
| 开始服务 | `entityOrder.startService` | `order_id` | Seller (服务类) |
| 完成服务 | `entityOrder.completeService` | `order_id` | Seller |
| 确认服务 | `entityOrder.confirmService` | `order_id` | Buyer |

**订单流程状态机：**
```
Created → Paid → Shipped → Completed
  ↓         ↓       ↓
Expired  Cancelled  Disputed → Refunded
                       ↓
                    Completed (after service)
```

#### Events

`OrderPlaced`, `OrderPaid`, `OrderCancelled`, `OrderShipped`, `OrderCompleted`, `RefundRequested`, `RefundApproved`, `ServiceStarted`, `ServiceCompleted`, `OrderExpired`

---

### 5.5 评价管理 (pallet-entity-review)

#### Extrinsics

| 操作 | Extrinsic | 参数 | 权限 |
|------|-----------|------|------|
| 提交评价 | `entityReview.submitReview` | `order_id, rating: u8 (1-5), content_cid` | Buyer (订单完成) |
| 开关评价 | `entityReview.setReviewEnabled` | `entity_id, enabled: bool` | Admin(REVIEW_MANAGE) |

---

### 5.6 代币管理 (pallet-entity-token)

#### Storage 查询

| Storage | Key | Value |
|---------|-----|-------|
| `TokenConfigs` | `u64` (entity_id) | `EntityTokenConfig<T>` |
| `TokenBalances` | `(u64, AccountId)` | `Balance` |
| `TokenReserved` | `(u64, AccountId)` | `Balance` |
| `TokenHolderCount` | `u64` | `u32` |
| `Whitelist` | `(u64, AccountId)` | `bool` |
| `Blacklist` | `(u64, AccountId)` | `bool` |
| `TokenLocks` | `(u64, AccountId)` | `VestingSchedule` |
| `DividendClaims` | `(u64, AccountId)` | `Balance` |

#### Extrinsics

| 操作 | Extrinsic | 关键参数 |
|------|-----------|---------|
| 创建代币 | `entityToken.createToken` | `entity_id, name, symbol, decimals, token_type, reward_rate_bps, exchange_rate_bps` |
| 更新配置 | `entityToken.updateTokenConfig` | `entity_id, reward_rate?, exchange_rate?, transferable?` |
| 铸造 | `entityToken.mintTokens` | `entity_id, to, amount` |
| 销毁 | `entityToken.burnTokens` | `entity_id, amount` |
| 转让 | `entityToken.transferTokens` | `entity_id, to, amount` |
| 配置分红 | `entityToken.configureDividend` | `entity_id, enabled, min_period` |
| 发放分红 | `entityToken.distributeDividend` | `entity_id, total_amount` |
| 领取分红 | `entityToken.claimDividend` | `entity_id` |
| 锁仓 | `entityToken.lockTokens` | `entity_id, user, amount, cliff_blocks, vesting_blocks` |
| 解锁 | `entityToken.releaseVesting` | `entity_id` |
| 变更类型 | `entityToken.changeTokenType` | `entity_id, new_type` |
| 设置最大供应 | `entityToken.setMaxSupply` | `entity_id, max_supply` |
| 设置转账限制 | `entityToken.setTransferRestriction` | `entity_id, mode: TransferRestrictionMode` |
| 白名单添加 | `entityToken.addToWhitelist` | `entity_id, account` |
| 白名单移除 | `entityToken.removeFromWhitelist` | `entity_id, account` |
| 黑名单添加 | `entityToken.addToBlacklist` | `entity_id, account` |
| 黑名单移除 | `entityToken.removeFromBlacklist` | `entity_id, account` |

---

### 5.7 内部代币市场 (pallet-entity-market)

Entity 内部的 Token/NEX 交易市场，与 NEX/USDT P2P OTC (trading/nex-market) 独立。

#### Extrinsics

| 操作 | Extrinsic | 关键参数 |
|------|-----------|---------|
| 限价卖出 | `entityMarket.placeSellOrder` | `entity_id, amount, price` |
| 限价买入 | `entityMarket.placeBuyOrder` | `entity_id, amount, price` |
| 吃单 | `entityMarket.takeOrder` | `order_id, amount` |
| 市价买入 | `entityMarket.marketBuy` | `entity_id, amount, max_cost` |
| 市价卖出 | `entityMarket.marketSell` | `entity_id, amount, min_receive` |
| 取消订单 | `entityMarket.cancelOrder` | `order_id` |
| 配置市场 | `entityMarket.configureMarket` | `entity_id, fee_rate, order_ttl, ...` |
| 价格保护 | `entityMarket.configurePriceProtection` | `entity_id, max_deviation_bps, circuit_breaker_*` |
| 设初始价 | `entityMarket.setInitialPrice` | `entity_id, price` |
| 解除熔断 | `entityMarket.liftCircuitBreaker` | `entity_id` |

---

### 5.8 会员管理 (pallet-entity-member)

#### Storage 查询

| Storage | Key | Value |
|---------|-----|-------|
| `Members` | `(u64, AccountId)` | `MemberInfo<T>` |
| `MemberCount` | `u64` (entity_id) | `u32` |
| `MemberReferrals` | `(u64, AccountId)` | `BoundedVec<AccountId>` |
| `LevelSystem` | `u64` | `LevelSystemConfig` |
| `CustomLevels` | `(u64, u8)` | `CustomLevel` |
| `UpgradeRules` | `(u64, u32)` | `UpgradeRule<T>` |
| `PendingMembers` | `(u64, AccountId)` | `PendingMember<T>` |
| `RegistrationPolicy` | `u64` | `MemberRegistrationPolicy` |
| `StatsPolicy` | `u64` | `MemberStatsPolicy` |

#### Extrinsics

| 操作 | Extrinsic | 关键参数 | 权限 |
|------|-----------|---------|------|
| 注册会员 | `entityMember.registerMember` | `entity_id, referrer?` | Signed |
| 审批会员 | `entityMember.approveMember` | `entity_id, account` | Owner/Admin |
| 拒绝会员 | `entityMember.rejectMember` | `entity_id, account` | Owner/Admin |
| 初始化等级系统 | `entityMember.initLevelSystem` | `entity_id, use_custom, upgrade_mode` | Owner/Admin |
| 添加自定义等级 | `entityMember.addCustomLevel` | `entity_id, level_id, name, threshold, discount_rate, commission_bonus` | Owner/Admin |
| 更新等级 | `entityMember.updateCustomLevel` | `entity_id, level_id, ...` | Owner/Admin |
| 删除等级 | `entityMember.removeCustomLevel` | `entity_id, level_id` | Owner/Admin |
| 手动升级 | `entityMember.manualUpgradeMember` | `entity_id, member, target_level_id` | Owner/Admin |
| 设置升级模式 | `entityMember.setUpgradeMode` | `entity_id, mode` | Owner/Admin |
| 设置注册策略 | `entityMember.setMemberPolicy` | `entity_id, policy_bits: u8` | Owner/Admin |
| 设置统计策略 | `entityMember.setStatsPolicy` | `entity_id, policy_bits: u8` | Owner/Admin |
| 添加升级规则 | `entityMember.addUpgradeRule` | `entity_id, trigger, target_level, threshold, max_triggers, stackable, priority` | Owner/Admin |
| 更新规则 | `entityMember.updateUpgradeRule` | `entity_id, rule_id, ...` | Owner/Admin |
| 删除规则 | `entityMember.removeUpgradeRule` | `entity_id, rule_id` | Owner/Admin |
| 启用/禁用规则 | `entityMember.toggleUpgradeRule` | `entity_id, rule_id, enabled` | Owner/Admin |
| 设置冲突策略 | `entityMember.setConflictPolicy` | `entity_id, policy` | Owner/Admin |
| 冻结会员 | `entityMember.freezeMember` | `entity_id, account` | Owner/Admin |
| 解冻会员 | `entityMember.unfreezeMember` | `entity_id, account` | Owner/Admin |
| 封禁会员 | `entityMember.banMember` | `entity_id, account` | Owner/Admin |
| 解封会员 | `entityMember.unbanMember` | `entity_id, account` | Owner/Admin |
| 移除会员 | `entityMember.removeMember` | `entity_id, account` | Owner/Admin |
| 清理过期待审 | `entityMember.cleanupExpiredPending` | `entity_id` | Signed |

**升级触发类型 (UpgradeTrigger)：**

| 触发类型 | 说明 | 对应路径 |
|----------|------|---------|
| `PurchaseProduct` | 购买指定商品 | 订单 |
| `SingleOrder` | 单笔 NEX 消费满额 | 订单 |
| `TotalSpent` | 累计 NEX 消费满额 | 订单 |
| `OrderCount` | 订单数达标 | 订单 |
| `TotalSpentUsdt` | USDT 累计消费达标 | 订单 |
| `SingleOrderUsdt` | USDT 单笔消费达标 | 订单 |
| `ReferralCount` | 直推人数达标 | 推荐 |
| `TeamSize` | 团队总人数达标 | 推荐 |
| `ReferralLevelCount` | 直推中指定等级人数达标 | 推荐 |

---

### 5.9 佣金系统 (pallet-entity-commission)

佣金系统由 **1 个核心 + 6 种模式** 组成，通过 `CommissionModes` 位标记控制启用。

#### 5.9.1 佣金核心 (commission-core)

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| `setCommissionModes` | `entity_id, modes: u32` | 启用/禁用佣金模式（位掩码） |
| `setCommissionRate` | `entity_id, rate_bps` | 设置基础佣金比例 |
| `enableCommission` | `entity_id` | 启用佣金系统 |
| `withdrawCommission` | `entity_id` | 提现 NEX 佣金 |
| `withdrawTokenCommission` | `entity_id` | 提现 Token 佣金 |
| `setWithdrawalConfig` | `entity_id, min_amount, fee_rate, cooldown` | 设置 NEX 提现配置 |
| `setTokenWithdrawalConfig` | `entity_id, ...` | 设置 Token 提现配置 |
| `withdrawEntityFunds` | `entity_id, amount` | Entity 提取资金 |
| `setCreatorRewardRate` | `entity_id, rate_bps` | 设置创始人奖励比例 |
| `setTokenPlatformFeeRate` | `entity_id, rate_bps` | 设置 Token 平台费率 |
| `setWithdrawalCooldown` | `entity_id, blocks` | 设置提现冷却期 |
| `pauseWithdrawals` | `entity_id` | 暂停提现 |
| `archiveOrderRecords` | `entity_id, max_count` | 归档订单佣金记录 |

**核心 Storage：**
`CommissionConfigs`, `MemberCommissionStats`, `OrderCommissionRecords`, `ShopCommissionTotals`, `MemberShoppingBalance`, `WithdrawalConfigs`, `UnallocatedPool`, `MemberTokenCommissionStats`, `OrderTokenCommissionRecords`, `TokenWithdrawalConfigs`, `GlobalCommissionPaused`, `WithdrawalPaused`

#### 5.9.2 直推佣金 (commission-referral)

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| `setDirectRewardConfig` | `entity_id, rate_bps, token_rate_bps` | 直推奖励配置 |
| `setFixedAmountConfig` | `entity_id, fixed_amount, fixed_token` | 固定金额配置 |
| `setFirstOrderConfig` | `entity_id, rate_bps, token_rate_bps` | 首单奖励配置 |
| `setRepeatPurchaseConfig` | `entity_id, rate_bps, token_rate_bps` | 复购奖励配置 |
| `setReferrerGuardConfig` | `entity_id, ...` | 推荐人资格门槛 |
| `setCommissionCapConfig` | `entity_id, max_per_order, max_total` | 佣金上限配置 |
| `setReferralValidityConfig` | `entity_id, validity_blocks` | 推荐关系有效期 |
| `clearReferralConfig` | `entity_id` | 清除配置 |

#### 5.9.3 多级佣金 (commission-multi-level)

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| `setMultiLevelConfig` | `entity_id, max_depth, rates[]` | 设置多级分配比例 |
| `addTier` | `entity_id, depth, rate_bps, token_rate_bps` | 添加层级 |
| `removeTier` | `entity_id, depth` | 移除层级 |
| `updateMultiLevelParams` | `entity_id, ...` | 更新参数 |
| `pauseMultiLevel` / `resumeMultiLevel` | `entity_id` | 暂停/恢复 |
| `scheduleConfigChange` | `entity_id, ...` | 预定配置变更（延时生效） |

#### 5.9.4 级差佣金 (commission-level-diff)

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| `setLevelDiffConfig` | `entity_id, level_rates[]` | 设置各等级费率 |
| `updateLevelDiffConfig` | `entity_id, ...` | 更新配置 |
| `clearLevelDiffConfig` | `entity_id` | 清除配置 |

#### 5.9.5 排线佣金 (commission-single-line)

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| `setSingleLineConfig` | `entity_id, segment_size, rate_bps, mode` | 排线配置 |
| `setLevelBasedLevels` | `entity_id, level_rates[]` | 等级差异化配置 |
| `pauseSingleLine` / `resumeSingleLine` | `entity_id` | 暂停/恢复 |

#### 5.9.6 团队佣金 (commission-team)

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| `setTeamPerformanceConfig` | `entity_id, tiers[]` | 团队业绩层级配置 |
| `addTier` | `entity_id, threshold, rate_bps` | 添加业绩层级 |
| `updateTier` / `removeTier` | `entity_id, tier_id, ...` | 更新/删除层级 |
| `pauseTeamPerformance` / `resumeTeamPerformance` | `entity_id` | 暂停/恢复 |

#### 5.9.7 奖池佣金 (commission-pool-reward)

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| `setPoolRewardConfig` | `entity_id, pool_rate_bps, round_blocks, min_participants` | 奖池配置 |
| `claimPoolReward` | `entity_id` | 领取奖池奖励 |
| `forceNewRound` | `entity_id` | 强制开始新一轮 |
| `setTokenPoolEnabled` | `entity_id, enabled` | 启用 Token 奖池 |
| `pausePoolReward` / `resumePoolReward` | `entity_id` | 暂停/恢复 |

---

### 5.10 DAO 治理 (pallet-entity-governance)

#### Extrinsics

| 操作 | Extrinsic | 关键参数 |
|------|-----------|---------|
| 创建提案 | `entityGovernance.createProposal` | `entity_id, proposal_type, title, description_cid, execution_params?` |
| 投票 | `entityGovernance.vote` | `proposal_id, vote_type: Approve/Reject/Abstain` |
| 结束投票 | `entityGovernance.finalizeVoting` | `proposal_id` |
| 执行提案 | `entityGovernance.executeProposal` | `proposal_id` |
| 取消提案 | `entityGovernance.cancelProposal` | `proposal_id` |
| 否决提案 | `entityGovernance.vetoProposal` | `proposal_id` |
| 配置治理 | `entityGovernance.configureGovernance` | `entity_id, quorum_pct, pass_threshold, voting_period, execution_delay, veto_enabled` |
| 锁定治理 | `entityGovernance.lockGovernance` | `entity_id` |
| 清理提案 | `entityGovernance.cleanupProposal` | `proposal_id` |

**ProposalType 分类（~41 种）：**

| 分类 | 提案类型 |
|------|---------|
| 实体管理 | `UpdateEntity`, `UpgradeEntityType`, `TransferOwnership`, `CloseEntity`, `SuspendEntity`, `ResumeEntity` |
| 店铺管理 | `CreateShop`, `CloseShop`, `PauseShop`, `BanShop`, `UnbanShop` |
| 商品管理 | `UpdatePrice`, `DelistProduct`, `SetInventory` |
| 代币管理 | `MintTokens`, `BurnTokens`, `ChangeTokenType`, `SetMaxSupply`, `SetTransferRestriction`, `ConfigureDividend` |
| 市场管理 | `ConfigureMarket`, `SetInitialPrice`, `LiftCircuitBreaker`, `ConfigurePriceProtection` |
| 会员管理 | `SetRegistrationPolicy`, `SetStatsPolicy`, `AddCustomLevel`, `SetUpgradeMode`, `BanMember`, `RemoveMember` |
| 佣金管理 | `SetCommissionRate`, `SetCommissionModes`, `UpdateWithdrawalConfig` |
| 披露管理 | `ConfigureDisclosure`, `ResetViolations` |
| 治理管理 | `UpdateGovernanceConfig`, `LockGovernance` |

---

### 5.11 财务披露与公告 (pallet-entity-disclosure)

#### Extrinsics

| 操作 | Extrinsic | 关键参数 | 权限 |
|------|-----------|---------|------|
| 配置披露 | `entityDisclosure.configureDisclosure` | `entity_id, level: DisclosureLevel, insider_trading_control, blackout_period_after` | Owner/Admin |
| 发布披露 | `entityDisclosure.publishDisclosure` | `entity_id, title_cid, content_cid, disclosure_type, is_material` | Owner/Admin |
| 创建草稿 | `entityDisclosure.createDraftDisclosure` | `entity_id, ...` | Owner/Admin |
| 更新草稿 | `entityDisclosure.updateDraft` | `disclosure_id, ...` | Owner/Admin |
| 发布草稿 | `entityDisclosure.publishDraft` | `disclosure_id` | Owner/Admin |
| 撤回披露 | `entityDisclosure.withdrawDisclosure` | `disclosure_id` | Owner/Admin |
| 更正披露 | `entityDisclosure.correctDisclosure` | `old_id, ...correction_params` | Owner/Admin |
| 添加内幕人员 | `entityDisclosure.addInsider` | `entity_id, account, role: InsiderRole` | Owner/Admin |
| 更新角色 | `entityDisclosure.updateInsiderRole` | `entity_id, account, new_role` | Owner/Admin |
| 移除内幕人员 | `entityDisclosure.removeInsider` | `entity_id, account` | Owner/Admin |
| 批量添加 | `entityDisclosure.batchAddInsiders` | `entity_id, insiders[]` | Owner/Admin |
| 开始黑窗口 | `entityDisclosure.startBlackout` | `entity_id, end_block` | Owner/Admin |
| 结束黑窗口 | `entityDisclosure.endBlackout` | `entity_id` | Owner/Admin |
| 发布公告 | `entityDisclosure.publishAnnouncement` | `entity_id, title_cid, content_cid, category, expires_at` | Owner/Admin |
| 更新公告 | `entityDisclosure.updateAnnouncement` | `announcement_id, ...` | Owner/Admin |
| 撤回公告 | `entityDisclosure.withdrawAnnouncement` | `announcement_id` | Owner/Admin |
| 置顶公告 | `entityDisclosure.pinAnnouncement` | `entity_id, announcement_id` | Owner/Admin |
| 过期公告 | `entityDisclosure.expireAnnouncement` | `announcement_id` | Signed |

**InsiderRole 枚举：** `Owner(0)`, `Admin(1)`, `Auditor(2)`, `Advisor(3)`, `MajorHolder(4)`

**DisclosureLevel 枚举：** `Basic` → `Standard` → `Enhanced` → `Full`

---

### 5.12 KYC 管理 (pallet-entity-kyc)

#### Extrinsics

| 操作 | Extrinsic | 关键参数 | 权限 |
|------|-----------|---------|------|
| 提交 KYC | `entityKyc.submitKyc` | `level: KycLevel, data_cid, country_code?: [u8;2]` | User |
| 批准 | `entityKyc.approveKyc` | `account, level, notes` | Provider |
| 拒绝 | `entityKyc.rejectKyc` | `account, reason` | Provider |
| 撤销 | `entityKyc.revokeKyc` | `account, reason` | Admin |
| 注册 Provider | `entityKyc.registerProvider` | `provider_account, name, api_endpoint` | Root |
| 移除 Provider | `entityKyc.removeProvider` | `provider_account` | Root |
| 设置要求 | `entityKyc.setEntityRequirement` | `entity_id, min_level, max_risk_score` | Admin(KYC_MANAGE) |
| 移除要求 | `entityKyc.removeEntityRequirement` | `entity_id` | Admin |
| 高风险国家 | `entityKyc.updateHighRiskCountries` | `countries: Vec<[u8;2]>` | Root |
| 过期标记 | `entityKyc.expireKyc` | `account` | Signed |
| 取消申请 | `entityKyc.cancelKyc` | — | User |
| 更新风险分 | `entityKyc.updateRiskScore` | `account, score: u8` | Provider |
| 续期 | `entityKyc.renewKyc` | `account, new_data_cid` | Provider |
| 更新数据 | `entityKyc.updateKycData` | `new_data_cid` | User |
| 清除数据 | `entityKyc.purgeKycData` | — | User |
| 批量撤销 | `entityKyc.batchRevokeByProvider` | `provider, accounts[]` | Provider |

---

### 5.13 代币发售 (pallet-entity-tokensale)

#### Extrinsics

| 操作 | Extrinsic | 关键参数 |
|------|-----------|---------|
| 创建轮次 | `entityTokensale.createSaleRound` | `entity_id, name, total_supply, price, start_block, end_block, min_buy, max_buy_per_user, soft_cap, hard_cap` |
| 添加支付选项 | `entityTokensale.addPaymentOption` | `round_id, asset_id, price` |
| 设置 Vesting | `entityTokensale.setVestingConfig` | `round_id, cliff_blocks, vesting_blocks` |
| 配置荷兰拍 | `entityTokensale.configureDutchAuction` | `round_id, start_price, end_price, decay_blocks` |
| 白名单管理 | `entityTokensale.addToWhitelist` / `removeFromWhitelist` | `round_id, accounts[]` |
| 开始发售 | `entityTokensale.startSale` | `round_id` |
| 认购 | `entityTokensale.subscribe` | `round_id, amount, payment_asset_id` |
| 增加认购 | `entityTokensale.increaseSubscription` | `round_id, additional_amount` |
| 结束发售 | `entityTokensale.endSale` | `round_id` |
| 领取代币 | `entityTokensale.claimTokens` | `round_id` |
| 解锁代币 | `entityTokensale.unlockTokens` | `round_id` |
| 取消发售 | `entityTokensale.cancelSale` | `round_id` |
| 领取退款 | `entityTokensale.claimRefund` | `round_id` |
| 提取资金 | `entityTokensale.withdrawFunds` | `round_id` |
| 延长发售 | `entityTokensale.extendSale` | `round_id, new_end_block` |
| 暂停/恢复 | `entityTokensale.pauseSale` / `resumeSale` | `round_id` |
| 更新轮次 | `entityTokensale.updateSaleRound` | `round_id, ...` |
| 清理轮次 | `entityTokensale.cleanupRound` | `round_id` |

---

## 6. NEX P2P OTC 市场 — 链端接口 (pallet-nex-market)

独立于 Entity 的全局 NEX/USDT P2P 交易市场，使用 TRC20 链下支付 + OCW 验证。

### 6.1 核心交易流程

```
卖家挂单(NEX锁定) → 买家预留/接单 → 买家支付USDT(链下TRC20)
                                        ↓
                              提交txHash → OCW验证TRC20交易 → 自动确认/争议
                                                              ↓
                                                        NEX释放给买家 + 交易费
```

### 6.2 Storage 查询

| Storage | Key | Value | 说明 |
|---------|-----|-------|------|
| `Orders` | `u64` | `Order<T>` | 挂单信息 |
| `SellOrders` / `BuyOrders` | — | `BoundedVec<u64>` | 活跃买卖单 |
| `UserOrders` | `AccountId` | `BoundedVec<u64>` | 用户挂单 |
| `UsdtTrades` | `u64` | `UsdtTrade<T>` | 交易详情 |
| `PendingUsdtTrades` | — | `BoundedVec<u64>` | 待验证交易 |
| `AwaitingPaymentTrades` | — | `BoundedVec<u64>` | 待支付交易 |
| `PendingUnderpaidTrades` | — | `BoundedVec<u64>` | 欠付处理中 |
| `BestAsk` / `BestBid` | — | `u64` | 最优价 |
| `LastTradePrice` | — | `u64` | 最新成交价 |
| `MarketStatsStore` | — | `MarketStats` | 总量/笔数/最新价 |
| `TwapAccumulatorStore` | — | `TwapAccumulator` | TWAP 价格累计 |
| `PriceProtectionStore` | — | `PriceProtectionConfig` | 价格保护配置 |
| `MarketPausedStore` | — | `bool` | 市场暂停标记 |
| `TradingFeeBps` | — | `u16` | 交易手续费(bps) |
| `DepositExchangeRate` | — | `u64` | 保证金汇率 |
| `TradeDisputeStore` | `u64` | `TradeDispute<T>` | 交易争议 |
| `UserTrades` | `AccountId` | `BoundedVec<u64>` | 用户交易记录 |

### 6.3 Extrinsics

| 操作 | Extrinsic | 关键参数 | 权限 |
|------|-----------|---------|------|
| 卖单 | `nexMarket.placeSellOrder` | `nex_amount, usdt_price, tron_address, ttl_blocks?` | Signed |
| 买单 | `nexMarket.placeBuyOrder` | `nex_amount, usdt_price, ttl_blocks?` | Signed |
| 取消挂单 | `nexMarket.cancelOrder` | `order_id` | OrderOwner |
| 更新价格 | `nexMarket.updateOrderPrice` | `order_id, new_price` | OrderOwner |
| 预留卖单 | `nexMarket.reserveSellOrder` | `order_id, take_amount` | Signed (买家) |
| 接受买单 | `nexMarket.acceptBuyOrder` | `order_id, take_amount, tron_address` | Signed (卖家) |
| 确认支付 | `nexMarket.confirmPayment` | `trade_id, tx_hash` | Buyer |
| 处理超时 | `nexMarket.processTimeout` | `trade_id` | Signed |
| 处理欠付 | `nexMarket.finalizeUnderpaid` | `trade_id` | Signed |
| 领取验证奖励 | `nexMarket.claimVerificationReward` | — | Signed |
| 发起争议 | `nexMarket.disputeTrade` | `trade_id, evidence_cid` | TradeParticipant |
| 解决争议 | `nexMarket.resolveDispute` | `trade_id, resolution: DisputeResolution` | MarketAdmin |
| 配置价格保护 | `nexMarket.configurePriceProtection` | `config: PriceProtectionConfig` | MarketAdmin |
| 设初始价 | `nexMarket.setInitialPrice` | `price` | MarketAdmin |
| 解除熔断 | `nexMarket.liftCircuitBreaker` | — | MarketAdmin |
| 设交易费 | `nexMarket.setTradingFee` | `fee_bps` | MarketAdmin |
| 更新汇率 | `nexMarket.updateDepositExchangeRate` | `new_rate` | MarketAdmin |
| 暂停市场 | `nexMarket.forcePauseMarket` | — | MarketAdmin |
| 恢复市场 | `nexMarket.forceResumeMarket` | — | MarketAdmin |
| 强制结算 | `nexMarket.forceSettleTrade` | `trade_id, actual_amount, resolution` | MarketAdmin |
| 强制取消 | `nexMarket.forceCancelTrade` | `trade_id` | MarketAdmin |
| 种子流动性 | `nexMarket.seedLiquidity` | `usdt_amount` | Signed |
| 注资种子账户 | `nexMarket.fundSeedAccount` | `amount` | Signed |

### 6.4 关键数据类型

```typescript
enum OrderSide { Sell, Buy }
enum OrderStatus { Open, PartiallyFilled, Filled, Cancelled, Expired }
enum UsdtTradeStatus { AwaitingPayment, AwaitingVerification, Completed, Disputed, Cancelled, Refunded, UnderpaidPending }
enum BuyerDepositStatus { None, Locked, Released, Forfeited, PartiallyForfeited }
enum DisputeResolution { ReleaseToBuyer, RefundToSeller }

interface PriceProtectionConfig {
  max_deviation_bps: number;      // 最大偏差(bps)
  circuit_breaker_threshold: number; // 熔断阈值
  circuit_breaker_duration: number;  // 熔断持续时间(blocks)
}
```

### 6.5 Events

`OrderCreated`, `OrderCancelled`, `OrderPriceUpdated`, `UsdtTradeCreated`, `UsdtPaymentSubmitted`, `UsdtTradeCompleted`, `UsdtTradeRefunded`, `BuyerDepositLocked`, `BuyerDepositReleased`, `BuyerDepositForfeited`, `TwapUpdated`, `CircuitBreakerTriggered`, `CircuitBreakerLifted`, `PriceProtectionConfigured`, `UnderpaidDetected`, `UnderpaidFinalized`, `MarketPaused`, `MarketResumed`, `TradeForceSettled`, `TradeForceCancelled`, `TradeDisputed`, `DisputeResolved`, `TradingFeeUpdated`, `TradingFeeCharged`, `LiquiditySeeded`

---

## 7. IPFS 存储服务 — 链端接口

### 7.1 Storage Service (pallet-storage-service)

#### 关键 Storage

| Storage | Key | Value | 说明 |
|---------|-----|-------|------|
| `Operators` | `AccountId` | `OperatorInfo<T>` | 存储运营商信息 |
| `OperatorBond` | `AccountId` | `Balance` | 运营商质押 |
| `PinMeta` | `Hash` | `PinMetadata` | Pin 元数据 |
| `PinStateOf` | `Hash` | `u8` (0=Requested,1=Pinning,2=Pinned,3=Degraded,4=Failed) | Pin 状态 |
| `PinAssignments` | `Hash` | `BoundedVec<AccountId>` | Pin 分配节点 |
| `CidRegistry` | `Hash` | `BoundedVec<u8>` | CID 注册表 |
| `CidTier` | `Hash` | `PinTier` | CID 存储层级 |
| `PinBilling` | `Hash` | `(BlockNumber, u128, u8)` | 计费信息 |
| `OwnerPinIndex` | `AccountId` | `BoundedVec<Hash>` | 所有者 Pin 索引 |
| `UserFundingBalance` | `AccountId` | `Balance` | 用户存储余额 |
| `BillingPaused` | — | `bool` | 计费暂停 |
| `PricePerGiBWeek` | — | `u128` | 每 GiB/周价格 |
| `RegisteredDomains` | `BoundedVec<u8>` | `DomainConfig` | 注册域名 |

#### 用户侧 Extrinsics

| 操作 | Extrinsic | 关键参数 |
|------|-----------|---------|
| 请求 Pin | `storageService.requestPinForSubject` | `subject_id, cid, size_bytes, tier?: PinTier` |
| 请求 Unpin | `storageService.requestUnpin` | `cid` |
| 批量 Unpin | `storageService.batchUnpin` | `cids: Vec` |
| 续期 Pin | `storageService.renewPin` | `cid_hash, periods` |
| 升级层级 | `storageService.upgradePinTier` | `cid_hash, new_tier` |
| 充值余额 | `storageService.fundUserAccount` | `target_user, amount` |
| 充值 IPFS 池 | `storageService.fundIpfsPool` | `amount` |

#### 运营商 Extrinsics

| 操作 | Extrinsic | 关键参数 |
|------|-----------|---------|
| 加入运营商 | `storageService.joinOperator` | `peer_id, capacity_gib, endpoint_hash, cert_fingerprint?, bond` |
| 更新信息 | `storageService.updateOperator` | `peer_id?, capacity_gib?, endpoint_hash?` |
| 退出 | `storageService.leaveOperator` | — |
| 暂停 | `storageService.pauseOperator` | — |
| 恢复 | `storageService.resumeOperator` | — |
| 报告 Pin | `storageService.markPinned` | `cid_hash, replicas` |
| 报告失败 | `storageService.markPinFailed` | `cid_hash, code` |
| 健康探测 | `storageService.reportProbe` | `ok: bool` |
| 领取奖励 | `storageService.operatorClaimRewards` | — |
| 追加质押 | `storageService.topUpBond` | `amount` |
| 减少质押 | `storageService.reduceBond` | `amount` |

#### 关键类型

```typescript
enum PinTier { Critical, Standard, Temporary }
enum HealthStatus { Unknown, Healthy, Degraded, Critical, Offline }
enum OperatorLayer { Core, Community, External }
enum SubjectType { Evidence, OtcOrder, Complaint, MakerProduct, NftAsset, Chat, Livestream, General, Custom }
```

### 7.2 Storage Lifecycle (pallet-storage-lifecycle)

| 操作 | Extrinsic | 说明 | 权限 |
|------|-----------|------|------|
| 设置归档配置 | `storageLifecycle.setArchiveConfig` | `config: ArchiveConfig` | Root |
| 暂停/恢复归档 | `storageLifecycle.pauseArchival` / `resumeArchival` | — | Root |
| 设置归档策略 | `storageLifecycle.setArchivePolicy` | `data_type, policy` | Root |
| 强制归档 | `storageLifecycle.forceArchive` | `data_type, data_ids, target_level` | Root |
| 保护免清除 | `storageLifecycle.protectFromPurge` | `data_type, data_id` | Root |
| 延长活跃期 | `storageLifecycle.extendActivePeriod` | `data_type, data_id, extend_blocks` | Root |
| 从归档恢复 | `storageLifecycle.restoreFromArchive` | `data_type, data_id` | Root |

```typescript
enum ArchiveLevel { Active, ArchivedL1, ArchivedL2, Purged }
```

---

## 8. GroupRobot 机器人系统 — 链端接口

### 8.1 Bot 注册 (grouprobot-registry)

#### 关键 Storage

| Storage | Key | Value |
|---------|-----|-------|
| `Bots` | `[u8;32]` (bot_id_hash) | `BotInfo<T>` |
| `OwnerBots` | `AccountId` | `BoundedVec<BotIdHash>` |
| `CommunityBindings` | `[u8;32]` (community_hash) | `CommunityBinding<T>` |
| `Attestations` / `AttestationsV2` | `BotIdHash` | `AttestationRecord<T>` |
| `Operators` | `(AccountId, Platform)` | `OperatorInfo<T>` |
| `PeerRegistry` | `BotIdHash` | `BoundedVec<PeerEndpoint<T>>` |

#### 核心 Extrinsics (45)

| 分类 | 操作 | Extrinsic |
|------|------|-----------|
| Bot 管理 | 注册 Bot | `grouprobotRegistry.registerBot(bot_id_hash, public_key)` |
| | 更新公钥 | `grouprobotRegistry.updatePublicKey(bot_id_hash, new_key)` |
| | 注销 Bot | `grouprobotRegistry.deactivateBot(bot_id_hash)` |
| | 转移所有权 | `grouprobotRegistry.transferBotOwnership(bot_id_hash, new_owner)` |
| 社区绑定 | 绑定社区 | `grouprobotRegistry.bindCommunity(bot_id_hash, community_id_hash, platform)` |
| | 解绑社区 | `grouprobotRegistry.unbindCommunity(community_id_hash)` |
| 用户平台绑定 | 绑定 | `grouprobotRegistry.bindUserPlatform(platform, platform_user_id_hash)` |
| | 解绑 | `grouprobotRegistry.unbindUserPlatform(platform)` |
| TEE 远程证明 | 提交证明 | `grouprobotRegistry.submitTeeAttestation(bot_id_hash, quote_raw, ...)` |
| | 刷新证明 | `grouprobotRegistry.refreshAttestation(bot_id_hash, ...)` |
| | DCAP 证明 | `grouprobotRegistry.submitDcapAttestation(bot_id_hash, tdx_quote_raw, ...)` |
| | SGX 证明 | `grouprobotRegistry.submitSgxAttestation(bot_id_hash, sgx_quote_raw, ...)` |
| P2P 节点 | 注册节点 | `grouprobotRegistry.registerPeer(bot_id_hash, peer_public_key, endpoint)` |
| | 注销节点 | `grouprobotRegistry.deregisterPeer(bot_id_hash, peer_public_key)` |
| | 心跳 | `grouprobotRegistry.heartbeatPeer(bot_id_hash, peer_public_key)` |
| 运营商 | 注册 | `grouprobotRegistry.registerOperator(platform, platform_app_hash, name, contact)` |
| | 更新 | `grouprobotRegistry.updateOperator(platform, name, contact)` |
| | 注销 | `grouprobotRegistry.deregisterOperator(platform)` |
| | 分配 Bot | `grouprobotRegistry.assignBotToOperator(bot_id_hash, platform)` |
| Root 管理 | 审批 MRTD | `grouprobotRegistry.approveMrtd(mrtd, version)` |
| | 吊销 MRTD | `grouprobotRegistry.revokeMrtd(mrtd)` |
| | 暂停 Bot | `grouprobotRegistry.suspendBot(bot_id_hash)` |
| | 恢复 Bot | `grouprobotRegistry.reactivateBot(bot_id_hash)` |

```typescript
enum Platform { Telegram, Discord, Slack, Matrix, Farcaster }
enum BotStatus { Active, Suspended, Deactivated }
enum TeeType { Tdx, Sgx, TdxPlusSgx }
```

### 8.2 密钥仪式 (grouprobot-ceremony) — 11 Extrinsics

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| 记录仪式 | `grouprobotCeremony.recordCeremony` | 记录分布式密钥生成仪式 |
| 吊销仪式 | `grouprobotCeremony.revokeCeremony` | Root 吊销 |
| 所有者吊销 | `grouprobotCeremony.ownerRevokeCeremony` | 所有者吊销 |
| 续期 | `grouprobotCeremony.renewCeremony` | 延长有效期 |
| 强制重新仪式 | `grouprobotCeremony.forceReCeremony` | Root 强制 |

### 8.3 社区管理 (grouprobot-community) — 16 Extrinsics

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| 提交行为日志 | `grouprobotCommunity.submitActionLog` | 记录社区管理操作 |
| 批量提交 | `grouprobotCommunity.batchSubmitLogs` | 批量操作日志 |
| 设置节点要求 | `grouprobotCommunity.setNodeRequirement` | 最低 TEE 要求 |
| 更新配置 | `grouprobotCommunity.updateCommunityConfig` | 社区配置（防刷/欢迎/广告/语言） |
| 声望奖励 | `grouprobotCommunity.awardReputation` | 增加成员声望 |
| 声望扣减 | `grouprobotCommunity.deductReputation` | 扣减成员声望 |
| 封禁社区 | `grouprobotCommunity.banCommunity` | Root 封禁 |

### 8.4 共识节点 (grouprobot-consensus) — 18 Extrinsics

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| 注册节点 | `grouprobotConsensus.registerNode` | 注册共识节点 + 质押 |
| 增加质押 | `grouprobotConsensus.increaseStake` | 追加质押 |
| 请求退出 | `grouprobotConsensus.requestExit` | 申请退出 |
| 完成退出 | `grouprobotConsensus.finalizeExit` | 解锁质押 |
| 报告异常 | `grouprobotConsensus.reportEquivocation` | 举报双签/作恶 |
| 验证 TEE | `grouprobotConsensus.verifyNodeTee` | 验证节点 TEE 状态 |

### 8.5 奖励分配 (grouprobot-rewards) — 11 Extrinsics

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| 领取奖励 | `grouprobotRewards.claimRewards` | 节点领取 Era 奖励 |
| 批量领取 | `grouprobotRewards.batchClaimRewards` | 批量领取 |
| 设置接收方 | `grouprobotRewards.setRewardRecipient` | 设置奖励接收地址 |
| 设置分配比例 | `grouprobotRewards.setRewardSplit` | 设置 Bot/Owner 分成 |
| 领取 Owner 奖励 | `grouprobotRewards.claimOwnerRewards` | Owner 领取份额 |

### 8.6 订阅管理 (grouprobot-subscription) — 21 Extrinsics

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| 订阅 | `grouprobotSubscription.subscribe` | `bot_id_hash, tier` |
| 充值 | `grouprobotSubscription.depositSubscription` | 预付费充值 |
| 取消订阅 | `grouprobotSubscription.cancelSubscription` | 取消 |
| 变更套餐 | `grouprobotSubscription.changeTier` | 升降级 |
| 暂停/恢复 | `grouprobotSubscription.pauseSubscription` / `resumeSubscription` | 暂停/恢复 |
| 承诺广告 | `grouprobotSubscription.commitAds` | 承诺广告投放量 |
| 取消承诺 | `grouprobotSubscription.cancelAdCommitment` | 取消广告承诺 |
| 提取托管 | `grouprobotSubscription.withdrawEscrow` | 提取订阅余额 |

```typescript
enum SubscriptionTier { Free, Basic, Pro, Enterprise }
enum SubscriptionStatus { Active, PastDue, Suspended, Cancelled, Paused }
```

---

## 9. 争议仲裁系统 — 链端接口

### 9.1 仲裁 (pallet-dispute-arbitration)

#### 核心 Extrinsics

| 操作 | Extrinsic | 关键参数 | 权限 |
|------|-----------|---------|------|
| 提交投诉 | `disputeArbitration.fileComplaint` | `respondent, complaint_type, evidence_id, object_id?` | Signed |
| 响应投诉 | `disputeArbitration.respondToComplaint` | `complaint_id, evidence_id` | Respondent |
| 撤回投诉 | `disputeArbitration.withdrawComplaint` | `complaint_id` | Complainant |
| 和解 | `disputeArbitration.settleComplaint` | `complaint_id` | Both parties |
| 升级仲裁 | `disputeArbitration.escalateToArbitration` | `complaint_id` | Signed |
| 裁决 | `disputeArbitration.resolveComplaint` | `complaint_id, decision: Decision` | Arbitrator |
| 追加证据 | `disputeArbitration.supplementComplaintEvidence` | `complaint_id, evidence_id` | Complainant |
| 追加答辩 | `disputeArbitration.supplementResponseEvidence` | `complaint_id, evidence_id` | Respondent |
| 缺席判决 | `disputeArbitration.requestDefaultJudgment` | `complaint_id` | Complainant |
| 开始调解 | `disputeArbitration.startMediation` | `complaint_id` | Arbitrator |
| 驳回 | `disputeArbitration.dismissComplaint` | `complaint_id` | Arbitrator |

```typescript
enum Decision { Release, Refund, Partial } // Partial(u16 bps)
enum ComplaintType {
  OtcFraud, OtcQuality, OtcDelivery,
  LivestreamContent, LivestreamPayment,
  MakerQuality, MakerDescription,
  NftCopyright, NftAuthenticity,
  SwapSettlement, MemberReputation, CreditFraud, Other
}
enum ComplaintStatus {
  Submitted, Responded, Mediating, Arbitrating,
  ResolvedComplainantWin, ResolvedRespondentWin, ResolvedSettlement,
  Withdrawn, Expired
}
```

### 9.2 资金托管 (pallet-dispute-escrow)

| 操作 | Extrinsic | 说明 | 权限 |
|------|-----------|------|------|
| 锁定 | `disputeEscrow.lock` | `from, to, amount` | Authorized |
| 释放 | `disputeEscrow.release` | `from, to` | Authorized |
| 退款 | `disputeEscrow.refund` | `from, to` | Authorized |
| 部分释放 | `disputeEscrow.releasePartial` | `from, to, amount` | Authorized |
| 部分退款 | `disputeEscrow.refundPartial` | `from, to, amount` | Authorized |
| 分拆释放 | `disputeEscrow.releaseSplit` | `from, to, splits[]` | Authorized |
| 争议标记 | `disputeEscrow.dispute` | `from, to` | Authorized |
| 执行裁决(全释) | `disputeEscrow.applyDecisionReleaseAll` | `from, to` | Admin |
| 执行裁决(全退) | `disputeEscrow.applyDecisionRefundAll` | `from, to` | Admin |
| 执行裁决(比例) | `disputeEscrow.applyDecisionPartialBps` | `from, to, release_bps` | Admin |
| Token 锁定 | `disputeEscrow.tokenLock` | `from, to, amount, reason` | Authorized |
| Token 释放/退款 | `disputeEscrow.tokenRelease` / `tokenRefund` | `from, to` | Authorized |
| 设置到期 | `disputeEscrow.scheduleExpiry` | `from, to, at_block` | Authorized |

### 9.3 证据管理 (pallet-dispute-evidence)

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| 提交证据 | `disputeEvidence.commit` | `target, ns, content_cid, images[], videos[], docs[], memo, content_type` |
| 哈希承诺 | `disputeEvidence.commitHash` | `target, ns, commitment: H256` |
| 揭示承诺 | `disputeEvidence.revealCommitment` | `evidence_id, content_cid, salt` |
| 追加证据 | `disputeEvidence.appendEvidence` | `parent_id, content_cid, images[], ...` |
| 关联证据 | `disputeEvidence.link` / `linkByNs` | `evidence_id, target/ns` |
| 封存证据 | `disputeEvidence.sealEvidence` | `evidence_id` (不可再修改) |
| 解封证据 | `disputeEvidence.unsealEvidence` | `evidence_id` |
| 撤回证据 | `disputeEvidence.withdrawEvidence` | `evidence_id` |
| 注册公钥 | `disputeEvidence.registerPublicKey` | `key_type, public_key` |
| 存储隐私内容 | `disputeEvidence.storePrivateContent` | `cid, encrypted_key, authorized_users[]` |
| 授权访问 | `disputeEvidence.grantAccess` | `content_id, user, encrypted_key` |
| 撤销访问 | `disputeEvidence.revokeAccess` | `content_id, user` |
| 轮换密钥 | `disputeEvidence.rotateContentKeys` | `content_id, new_encrypted_keys[]` |
| 请求访问 | `disputeEvidence.requestAccess` | `content_id` |

---

## 10. 广告系统 — 链端接口

### 10.1 广告核心 (pallet-ads-core) — 50 Extrinsics

#### 广告主操作

| 操作 | Extrinsic | 关键参数 |
|------|-----------|---------|
| 注册广告主 | `adsCore.registerAdvertiser` | 注册为广告主 |
| 创建活动 | `adsCore.createCampaign` | `text, url, bid_per_mille, bid_per_click, campaign_type, daily_budget, total_budget, delivery_types, expires_at` |
| 充值活动 | `adsCore.fundCampaign` | `campaign_id, amount` |
| 暂停活动 | `adsCore.pauseCampaign` | `campaign_id` |
| 恢复活动 | `adsCore.resumeCampaign` | `campaign_id` |
| 取消活动 | `adsCore.cancelCampaign` | `campaign_id` |
| 更新活动 | `adsCore.updateCampaign` | `campaign_id, ...` |
| 延长到期 | `adsCore.extendCampaignExpiry` | `campaign_id, new_expiry` |
| 设定目标 | `adsCore.setCampaignTargets` | `campaign_id, targets` |
| 举报活动 | `adsCore.reportApprovedCampaign` | `campaign_id` |
| 重新提审 | `adsCore.resubmitCampaign` | `campaign_id` |
| 领取推荐收益 | `adsCore.claimReferralEarnings` | — |

#### 广告位操作

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| 领取广告收入 | `adsCore.claimAdRevenue` | 广告位领取收入 |
| 设置投放类型 | `adsCore.setPlacementDeliveryTypes` | 支持的投放类型 |
| 设置需审批 | `adsCore.setPlacementApprovalRequired` | 需广告位审批 |
| 审批活动 | `adsCore.approveCampaignForPlacement` | 审批某活动 |
| 拒绝活动 | `adsCore.rejectCampaignForPlacement` | 拒绝某活动 |
| 屏蔽广告主 | `adsCore.placementBlockAdvertiser` | 屏蔽某广告主 |

#### 投放验证

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| 提交展示回执 | `adsCore.submitDeliveryReceipt` | 提交 CPM 投放回执 |
| 提交点击回执 | `adsCore.submitClickReceipt` | 提交 CPC 点击回执 |
| 确认回执 | `adsCore.confirmReceipt` | 广告主确认 |
| 争议回执 | `adsCore.disputeReceipt` | 广告主争议 |
| 结算 Era 广告 | `adsCore.settleEraAds` | 结算一个 Era 的广告 |

```typescript
enum CampaignStatus { Active, Paused, Exhausted, Expired, Cancelled, Suspended, UnderReview }
enum CampaignType { Cpm, Cpc, Fixed, Private }
enum AdReviewStatus { Pending, Approved, Rejected, Flagged }
```

### 10.2 Entity 广告位 (pallet-ads-entity) — 9 Extrinsics

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| 注册实体广告位 | `adsEntity.registerEntityPlacement` | Entity 级别广告位 |
| 注册店铺广告位 | `adsEntity.registerShopPlacement` | Shop 级别广告位 |
| 注销广告位 | `adsEntity.deregisterPlacement` | 注销 |
| 启用/禁用 | `adsEntity.setPlacementActive` | 开关广告位 |
| 设置展示上限 | `adsEntity.setImpressionCap` | 每日展示上限 |
| 设置点击上限 | `adsEntity.setClickCap` | 每日点击上限 |
| 设置分成比例 | `adsEntity.setEntityAdShare` | Entity 广告分成(bps) |

### 10.3 社区广告质押 (pallet-ads-grouprobot) — 20 Extrinsics

| 操作 | Extrinsic | 说明 |
|------|-----------|------|
| 质押参与广告 | `adsGrouprobot.stakeForAds` | 社区质押获得广告曝光 |
| 解除质押 | `adsGrouprobot.unstakeForAds` | 解除质押 |
| 提取解绑 | `adsGrouprobot.withdrawUnbonded` | 提取已解绑质押 |
| 领取质押收益 | `adsGrouprobot.claimStakerReward` | 领取质押奖励 |
| 设置社区管理员 | `adsGrouprobot.setCommunityAdmin` | 社区广告管理员 |
| 管理员暂停广告 | `adsGrouprobot.adminPauseAds` | 社区管理员暂停 |
| 管理员恢复广告 | `adsGrouprobot.adminResumeAds` | 社区管理员恢复 |
| 报告受众 | `adsGrouprobot.reportNodeAudience` | 节点报告社区受众规模 |
| 设置 Bot 广告开关 | `adsGrouprobot.setBotAdsEnabled` | Bot 所有者控制广告 |

---

## 11. 事件订阅（全平台关键事件清单）

前端通过 `api.query.system.events()` 订阅，按 pallet 过滤。

| Pallet 系统 | 关键事件 |
|-------------|---------|
| **Entity Registry** | `EntityCreated`, `EntitySuspendedLowFund`, `FundWarning`, `OwnershipTransferred`, `GovernanceLocked` |
| **Entity Shop** | `ShopCreated`, `ShopClosed`, `FundDeposited`, `PointsEnabled` |
| **Entity Token** | `TokenCreated`, `TokensMinted`, `DividendDistributed`, `TokenTypeChanged` |
| **Entity Order** | `OrderPlaced`, `OrderPaid`, `OrderShipped`, `OrderCompleted`, `RefundRequested` |
| **Entity Market** | `OrderPlaced`, `OrderFilled`, `CircuitBreakerTriggered` |
| **Entity Member** | `MemberRegistered`, `MemberUpgraded`, `MemberBanned`, `LevelExpired` |
| **Entity Commission** | `CommissionDistributed`, `WithdrawalCompleted`, `PoolRewardClaimed` |
| **Entity Governance** | `ProposalCreated`, `VoteCast`, `ProposalExecuted`, `ProposalVetoed` |
| **Entity Disclosure** | `DisclosurePublished`, `AnnouncementPublished`, `BlackoutStarted` |
| **Entity KYC** | `KycApproved`, `KycRejected`, `KycExpired` |
| **Entity Tokensale** | `SaleStarted`, `Subscribed`, `SaleEnded`, `TokensClaimed` |
| **NEX Market** | `OrderCreated`, `UsdtTradeCreated`, `UsdtTradeCompleted`, `CircuitBreakerTriggered`, `TradeDisputed` |
| **Storage** | `PinRequested`, `PinMarkedPinned`, `PinExpired`, `OperatorJoined`, `HealthDegraded` |
| **GroupRobot** | `BotRegistered`, `CommunityBound`, `AttestationSubmitted`, `NodeRegistered`, `RewardsClaimed`, `Subscribed` |
| **Dispute** | `ComplaintFiled`, `ComplaintResolved`, `EscrowLocked`, `EscrowReleased`, `EvidenceCommitted` |
| **Ads** | `CampaignCreated`, `DeliveryReceiptSubmitted`, `AdReveneClaimed`, `AdStaked`, `PlacementRegistered` |

---

## 12. 链上交互层设计 (Hooks)

### 12.1 核心 Hook: `useApi`

```typescript
interface ApiState {
  api: ApiPromise | null;
  isConnected: boolean;
  chainInfo: {
    name: string;
    bestBlock: number;
    finalizedBlock: number;
  };
}
export function useApi(): ApiState;
```

### 12.2 交易提交 Hook: `useTx`

```typescript
interface TxState {
  status: 'idle' | 'signing' | 'broadcasting' | 'inBlock' | 'finalized' | 'error';
  txHash: string | null;
  blockHash: string | null;
  error: string | null;
}
interface UseTxReturn {
  submit: (extrinsic: SubmittableExtrinsic) => Promise<void>;
  state: TxState;
  reset: () => void;
}
export function useTx(): UseTxReturn;
```

### 12.3 Entity 查询 Hook: `useEntity`

```typescript
interface EntityData {
  id: number;
  owner: string;
  name: string;
  logoCid: string | null;
  descriptionCid: string | null;
  metadataUri: string | null;
  status: EntityStatus;
  entityType: EntityType;
  governanceMode: GovernanceMode;
  verified: boolean;
  governanceLocked: boolean;
  admins: Array<{ address: string; permissions: number }>;
  shopIds: number[];
  primaryShopId: number;
  totalSales: bigint;
  totalOrders: number;
  fundBalance: bigint;
  createdAt: number;
}

export function useEntity(entityId: number): {
  data: EntityData | null;
  isLoading: boolean;
  error: Error | null;
  refetch: () => void;
};

export function useEntityActions(entityId: number): {
  updateEntity: (params: Partial<EntityData>) => Promise<void>;
  topUpFund: (amount: bigint) => Promise<void>;
  addAdmin: (admin: string, permissions: number) => Promise<void>;
  removeAdmin: (admin: string) => Promise<void>;
  updateAdminPermissions: (admin: string, newPerms: number) => Promise<void>;
  transferOwnership: (newOwner: string) => Promise<void>;
  requestClose: () => Promise<void>;
  reopenEntity: () => Promise<void>;
  upgradeType: (newType: EntityType, newGovernance?: GovernanceMode) => Promise<void>;
  bindReferrer: (referrer: string) => Promise<void>;
};
```

### 12.4 NEX Market Hook: `useNexMarket`

```typescript
export function useNexMarket(): {
  sellOrders: Order[];
  buyOrders: Order[];
  bestAsk: number;
  bestBid: number;
  lastPrice: number;
  marketStats: MarketStats;
  isMarketPaused: boolean;
  tradingFeeBps: number;
};

export function useNexMarketActions(): {
  placeSellOrder: (amount: bigint, usdtPrice: number, tronAddress: string, ttl?: number) => Promise<void>;
  placeBuyOrder: (amount: bigint, usdtPrice: number, ttl?: number) => Promise<void>;
  cancelOrder: (orderId: number) => Promise<void>;
  reserveSellOrder: (orderId: number, takeAmount: bigint) => Promise<void>;
  acceptBuyOrder: (orderId: number, takeAmount: bigint, tronAddress: string) => Promise<void>;
  confirmPayment: (tradeId: number, txHash: string) => Promise<void>;
  disputeTrade: (tradeId: number, evidenceCid: string) => Promise<void>;
};
```

### 12.5 Bot 管理 Hook: `useBot`

```typescript
export function useBot(botIdHash: string): {
  data: BotInfo | null;
  attestation: AttestationRecord | null;
  peers: PeerEndpoint[];
  communities: CommunityBinding[];
};

export function useBotActions(): {
  registerBot: (botIdHash: string, publicKey: string) => Promise<void>;
  updatePublicKey: (botIdHash: string, newKey: string) => Promise<void>;
  deactivateBot: (botIdHash: string) => Promise<void>;
  bindCommunity: (botIdHash: string, communityHash: string, platform: Platform) => Promise<void>;
  submitTeeAttestation: (botIdHash: string, quoteRaw: Uint8Array, ...) => Promise<void>;
  registerPeer: (botIdHash: string, peerKey: string, endpoint: string) => Promise<void>;
};
```

---

## 13. 状态管理

### 13.1 Zustand Store

```typescript
// stores/entity.ts
interface EntityStore {
  currentEntityId: number | null;
  setCurrentEntityId: (id: number) => void;
  userEntities: EntitySummary[];
  loadUserEntities: (account: string) => Promise<void>;
  permissions: Record<number, number>;
  hasPermission: (entityId: number, required: number) => boolean;
}
```

### 13.2 React Query 缓存策略

| 查询 | staleTime | 缓存 Key |
|------|-----------|---------|
| Entity 基本信息 | 30s | `['entity', entityId]` |
| Shop 列表 | 30s | `['shops', entityId]` |
| 商品列表 | 15s | `['products', shopId]` |
| 订单列表 | 10s | `['orders', shopId, status]` |
| 代币信息 | 60s | `['token', entityId]` |
| Entity 市场订单簿 | 5s | `['entityMarket', entityId]` |
| NEX/USDT 挂单 | 3s | `['nexMarket', 'orders']` |
| NEX/USDT 交易 | 5s | `['nexMarket', 'trades']` |
| 会员列表 | 30s | `['members', entityId, page]` |
| 提案列表 | 30s | `['proposals', entityId]` |
| KYC 记录 | 60s | `['kyc', entityId]` |
| Bot 列表 | 30s | `['bots', owner]` |
| 社区配置 | 60s | `['community', communityHash]` |
| 投诉列表 | 15s | `['complaints', status]` |
| 广告活动 | 30s | `['campaigns', advertiser]` |
| IPFS Pin | 60s | `['pins', owner]` |
| 运营商列表 | 60s | `['operators']` |

---

## 14. 权限控制

### 14.1 AdminPermission 矩阵

```typescript
function hasPermission(entityId: number, account: string, required: number): boolean {
  // 1. Owner 拥有全部权限
  if (isOwner(entityId, account)) return true;
  // 2. 查询 EntityAdmins 存储获取 permission_bits
  const bits = getAdminPermissions(entityId, account);
  return (bits & required) === required;
}
```

### 14.2 根据 EntityType 动态菜单

| EntityType | 主要模块 | 可选/隐藏模块 |
|------------|---------|-------------|
| Merchant | 店铺、商品、订单、会员、佣金、代币 | 治理(可选)、披露(可选) |
| Enterprise | 全部 | — |
| DAO | 治理(主导)、代币、市场、会员、披露 | 商品(可选)、订单(可选) |
| Community | 会员、代币、公告 | 商品(可选)、KYC(可选) |
| Project | 代币、发售、治理、披露、KYC | — |
| ServiceProvider | 店铺(服务)、订单、会员、佣金 | — |
| Fund | 代币、治理、市场、披露、KYC | 商品、订单 |

---

## 15. 页面设计要点

### 15.1 全局布局

```
┌──────────────────────────────────────────────────────────────────────┐
│ [Logo] NEXUS Platform     [Entity: MyShop ▾] [🔔] [👤 0x...] [🌐] │
├─────────┬────────────────────────────────────────────────────────────┤
│         │                                                            │
│ 📊 仪表盘│              主内容区域                                     │
│         │                                                            │
│ 🏢 Entity│  ┌──────────────────────────────────────────────────┐     │
│ 🏪 Shop  │  │                                                  │     │
│ 🪙 Token │  │          当前页面内容                              │     │
│ 📈 Market│  │                                                  │     │
│ 👥 会员  │  └──────────────────────────────────────────────────┘     │
│ 💰 佣金  │                                                            │
│ 🗳️ 治理  │                                                            │
│ 📋 披露  │                                                            │
│ 🔐 KYC  │                                                            │
│ 🎯 发售  │                                                            │
│ ─────── │                                                            │
│ 💱 交易  │  (NEX/USDT P2P)                                           │
│ 📦 存储  │  (IPFS)                                                    │
│ 🤖 机器人│  (GroupRobot)                                              │
│ ⚖️ 仲裁  │  (Dispute)                                                 │
│ 📢 广告  │  (Ads)                                                     │
├─────────┴────────────────────────────────────────────────────────────┤
│ [Chain: Nexus Testnet] [Block: #12,345] [Finalized: #12,340]        │
└──────────────────────────────────────────────────────────────────────┘
```

### 15.2 NEX/USDT 交易页面

```
┌─ NEX/USDT P2P 市场 ─────────────────────────────────────────────────┐
│                                                                      │
│ 最新价: $0.05  │  24h量: 125,000 NEX  │  TWAP: $0.048  │ 费率: 1% │
│ 状态: 🟢 运营中  │  熔断保护: ✅ 正常                                  │
│                                                                      │
│ ┌─ 卖单 (Ask) ────────────┐  ┌─ 买单 (Bid) ───────────────┐       │
│ │ $0.055 │ 5,000 NEX     │  │ $0.048 │ 8,000 NEX         │       │
│ │ $0.053 │ 12,000 NEX    │  │ $0.047 │ 15,000 NEX        │       │
│ │ $0.052 │ 3,000 NEX     │  │ $0.045 │ 20,000 NEX        │       │
│ └─────────────────────────┘  └─────────────────────────────┘       │
│                                                                      │
│ ┌─ 挂单 ─────────────────────────────────────────────────────────┐  │
│ │ [卖出 NEX]  [买入 NEX]                                         │  │
│ │                                                                 │  │
│ │ 数量: [________] NEX   USDT价格: [________]                    │  │
│ │ TRON 地址: [________________________________] (卖单必填)       │  │
│ │ 有效期: [72h ▾]                                                 │  │
│ │                                                                 │  │
│ │ [确认挂单]                                                      │  │
│ └─────────────────────────────────────────────────────────────────┘  │
│                                                                      │
│ ┌─ 我的交易 ──────────────────────────────────────────────────────┐ │
│ │ #T5 │ 买入 │ 10,000 NEX @ $0.05 │ AwaitingPayment │ [支付]    │ │
│ │ #T3 │ 卖出 │ 5,000 NEX @ $0.052 │ Completed ✅    │           │ │
│ └──────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────┘
```

### 15.3 Bot 管理页面

```
┌─ GroupRobot 管理 ────────────────────────────────── [+ 注册 Bot] ────┐
│                                                                       │
│ ┌─ MyBot-001 ──────────────── 🟢 Active ── TEE: TDX ✅ ─────────┐  │
│ │ Bot ID: 0xabcd...1234                                           │  │
│ │ 公钥: 0x5678...                          证明过期: 2026-06-01   │  │
│ │ 社区: 3 │ 节点: 2 │ 订阅: Pro                                   │  │
│ │                                                                  │  │
│ │ ┌─ 社区绑定 ─────────────────────────────────────────────────┐ │  │
│ │ │ Telegram: @my_community  │ Discord: MyServer#123           │ │  │
│ │ └────────────────────────────────────────────────────────────┘ │  │
│ │                                                                  │  │
│ │ ┌─ P2P 节点 ────────────────────────────────────────────────┐  │  │
│ │ │ Node-A │ 🟢 Active │ ws://node-a.example.com │ 心跳: 2m  │  │  │
│ │ │ Node-B │ 🟢 Active │ ws://node-b.example.com │ 心跳: 5m  │  │  │
│ │ └───────────────────────────────────────────────────────────┘  │  │
│ │                                                                  │  │
│ │ [更新公钥] [刷新证明] [管理社区] [管理节点] [注销]              │  │
│ └──────────────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────────────┘
```

---

## 16. 性能优化

- **链上查询批量化：** 使用 `api.queryMulti()` 批量查询，减少 WebSocket 往返
- **订阅优化：** 仪表盘使用 `api.query.*.entries()` 一次性加载，配合事件增量更新
- **分页：** 会员/订单/投诉列表使用 `PageRequest { offset, limit }` 标准分页（链端 runtime_api 支持）
- **图片懒加载：** IPFS 内容使用 `IntersectionObserver` 懒加载
- **代码分割：** 按路由动态 `import()`，市场图表按需加载
- **SSR 限制：** 链上数据全部 CSR，仅布局/静态内容 SSR
- **WebSocket 复用：** 所有 pallet 查询共享同一 WebSocket 连接

---

## 17. 环境变量

```env
NEXT_PUBLIC_WS_ENDPOINT=ws://localhost:9944
NEXT_PUBLIC_IPFS_GATEWAY=https://gateway.pinata.cloud/ipfs
NEXT_PUBLIC_PINATA_API_KEY=your_api_key
NEXT_PUBLIC_PINATA_SECRET=your_secret
NEXT_PUBLIC_CHAIN_NAME=Nexus Testnet
```

---

## 18. 开发计划

### Phase 1: 基础框架 (2 周)
- [ ] Next.js 项目初始化 + Tailwind + shadcn/ui
- [ ] Polkadot API 连接 + 钱包集成
- [ ] 全局布局（六大系统侧栏 + 顶栏 + 面包屑）
- [ ] Entity 选择器 + 基本状态管理
- [ ] IPFS 上传/解析工具
- [ ] 通用组件：TxButton, StatusBadge, CidDisplay, AddressDisplay, PermissionGuard

### Phase 2: Entity 核心 (3 周)
- [ ] 仪表盘 + Entity 设置/管理员/资金
- [ ] Shop 管理（创建/编辑/暂停/关闭/积分）
- [ ] 商品管理（CRUD + 状态管理 + 可见性）
- [ ] 订单管理（看板视图 + 全流程操作 + 支付资产选择）
- [ ] 评价管理

### Phase 3: 通证经济 (3 周)
- [ ] 代币配置 + 类型管理（7 种类型）
- [ ] 持有人管理 + 分红管理
- [ ] 锁仓/Vesting 管理
- [ ] 转账限制（5 种模式：None/Whitelist/Blacklist/KycRequired/MembersOnly）
- [ ] Entity 内部 Token/NEX 市场

### Phase 4: 会员与佣金 (3 周)
- [ ] 会员列表 + 5 种状态管理
- [ ] 等级系统（自定义等级）
- [ ] 升级规则引擎（9 种触发类型）
- [ ] 注册策略配置（5 种位标记组合）
- [ ] 佣金核心 + 6 种佣金模式配置页面
- [ ] 提现管理（NEX + Token 双通道）

### Phase 5: 治理与合规 (2 周)
- [ ] DAO 治理提案（~41 种类型）+ 投票
- [ ] 财务披露 + 草稿流程 + 公告管理
- [ ] 内幕人员管理 + 黑窗口期
- [ ] KYC 管理（5 级 + Provider 管理）
- [ ] 代币发售 + Vesting + 荷兰拍

### Phase 6: NEX P2P 市场 (2 周)
- [ ] 挂单列表（买单/卖单）+ 最优价/TWAP 展示
- [ ] 挂单创建 + TRON 地址管理
- [ ] 交易流程（预留→支付→验证→完成）
- [ ] 欠付处理 + 超时处理
- [ ] 交易争议 + 价格保护/熔断展示

### Phase 7: GroupRobot (2 周)
- [ ] Bot 注册/管理 + 公钥更新
- [ ] TEE 远程证明管理（TDX/SGX/DCAP）
- [ ] 社区绑定 + 配置管理
- [ ] P2P 节点注册/心跳/管理
- [ ] 运营商管理
- [ ] 共识节点 + 质押/退出
- [ ] 订阅管理 + 套餐变更
- [ ] 奖励领取 + 分配比例设置

### Phase 8: 争议与广告 (2 周)
- [ ] 投诉提交 + 证据上传
- [ ] 投诉响应 + 证据追加
- [ ] 仲裁流程（调解→升级→裁决）
- [ ] 托管资金展示
- [ ] 广告活动管理（CPM/CPC/Fixed/Private）
- [ ] 广告位注册（Entity/Shop/Community）
- [ ] 社区广告质押 + 收益领取
- [ ] 投放验证 + 结算

### Phase 9: IPFS 存储 (1 周)
- [ ] Pin 管理（请求/续期/升级/Unpin）
- [ ] 运营商面板（加入/退出/质押/奖励）
- [ ] 计费管理 + 余额充值
- [ ] 域名管理
- [ ] 数据生命周期展示

### Phase 10: 打磨 (2 周)
- [ ] 通知中心（全平台事件实时推送）
- [ ] 国际化 (中/英)
- [ ] 响应式适配（Mobile/Tablet/Desktop）
- [ ] 性能优化 + E2E 测试
- [ ] 部署配置

**预计总工期：20-22 周**

---

## 19. Pallet-Extrinsic 完整统计表

| Pallet | 模块 | Extrinsic 数 | 面向角色 |
|--------|------|-------------|---------|
| `pallet-entity-registry` | 实体注册 | ~18 | Owner/Admin/Governance/Root |
| `pallet-entity-shop` | 店铺管理 | ~14 | Owner/Admin/Manager |
| `pallet-entity-product` | 商品管理 | ~5 | Owner/Admin |
| `pallet-entity-order` | 订单管理 | ~10 | Buyer/Seller |
| `pallet-entity-review` | 评价管理 | ~2 | Buyer/Admin |
| `pallet-entity-token` | 代币管理 | ~17 | Owner/Admin/User |
| `pallet-entity-market` | 内部市场 | ~10 | Trader/Owner |
| `pallet-entity-member` | 会员管理 | ~23 | Owner/Admin/User |
| `pallet-entity-commission-core` | 佣金核心 | ~25 | Owner/Admin |
| `pallet-entity-commission-referral` | 直推佣金 | ~14 | Owner/Admin |
| `pallet-entity-commission-multi-level` | 多级佣金 | ~12 | Owner/Admin |
| `pallet-entity-commission-level-diff` | 级差佣金 | ~5 | Owner/Admin |
| `pallet-entity-commission-single-line` | 排线佣金 | ~10 | Owner/Admin |
| `pallet-entity-commission-team` | 团队佣金 | ~10 | Owner/Admin |
| `pallet-entity-commission-pool-reward` | 奖池佣金 | ~12 | Owner/Admin/User |
| `pallet-entity-governance` | DAO 治理 | ~9 | Owner/Admin/Voter |
| `pallet-entity-disclosure` | 财务披露 | ~28 | Owner/Admin |
| `pallet-entity-kyc` | KYC 认证 | ~23 | User/Provider/Admin/Root |
| `pallet-entity-tokensale` | 代币发售 | ~27 | Owner/Admin/Subscriber |
| `pallet-nex-market` | NEX/USDT 市场 | ~27 | Trader/MarketAdmin/OCW |
| `pallet-storage-service` | IPFS 存储 | ~51 | User/Operator/Governance/OCW |
| `pallet-storage-lifecycle` | 数据生命周期 | ~9 | Root |
| `pallet-grouprobot-registry` | Bot 注册 | ~45 | Owner/Root |
| `pallet-grouprobot-ceremony` | 密钥仪式 | ~11 | Owner/Root |
| `pallet-grouprobot-community` | 社区管理 | ~16 | Owner/Root |
| `pallet-grouprobot-consensus` | 共识节点 | ~18 | Operator/Root |
| `pallet-grouprobot-rewards` | 奖励分配 | ~11 | Node/Owner |
| `pallet-grouprobot-subscription` | 订阅管理 | ~21 | Owner/Root |
| `pallet-dispute-arbitration` | 争议仲裁 | ~23 | User/Arbitrator/Root |
| `pallet-dispute-escrow` | 资金托管 | ~20 | Authorized/Admin |
| `pallet-dispute-evidence` | 证据管理 | ~22 | User/Root |
| `pallet-ads-core` | 广告核心 | ~50 | Advertiser/Placement/Root |
| `pallet-ads-entity` | Entity 广告位 | ~9 | Owner/Admin/Root |
| `pallet-ads-grouprobot` | 社区广告质押 | ~20 | Staker/Admin/Root |
| **合计** | **34 模块** | **~623** | — |
