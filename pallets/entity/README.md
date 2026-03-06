# Entity 模块套件

> NEXUS 平台核心业务模块 — 覆盖实体注册、多店铺运营、通证经济、订单交易、DAO 治理、多模式返佣、KYC 合规与代币发售全链路

## 概述

Entity 模块套件是 NEXUS 平台的核心业务层，采用 **Entity（组织层）+ Shop（业务层）** 双层架构，围绕链上实体的完整生命周期构建。套件由 14 个顶层子模块（含 commission 下的 8 个子模块）组成，共提供 **270+ 链上可调用函数（extrinsics）**，覆盖从实体注册到代币发售的全业务场景。

所有子模块通过 `pallet-entity-common` 定义的 Trait 接口实现松耦合，遵循开闭原则：新增下游模块无需修改上游。

## 架构总览

```
pallets/entity/
├── common/              # 公共类型、Trait 接口、分页、空实现（纯 Rust crate，无链上状储）
│
│  ┌─ 组织层 ─────────────────────────────────────────────────────────────┐
├── registry/            # Entity 注册、生命周期、多管理员权限位掩码、推荐人
│  └──────────────────────────────────────────────────────────────────────┘
│
│  ┌─ 业务层 ─────────────────────────────────────────────────────────────┐
├── shop/                # Shop 管理、运营资金、积分系统（TTL/上限/兑换）
├── product/             # 商品 CRUD、押金机制、批量操作、IPFS 元数据固定
├── order/               # 订单全流程（NEX/Token 双资产）、Escrow 托管、超时队列
├── review/              # 评价系统（提交/回复/修改/删除、商品评分索引）
│  └──────────────────────────────────────────────────────────────────────┘
│
│  ┌─ 通证经济层 ─────────────────────────────────────────────────────────┐
├── token/               # 7 种代币类型、分红、锁仓、转账限制、冻结/销毁
├── market/              # P2P 交易市场（限价/市价/IOC/FOK）、TWAP、熔断机制
├── tokensale/           # 代币发售（5 种模式）、多轮次、Vesting 解锁、软硬上限
│  └──────────────────────────────────────────────────────────────────────┘
│
│  ┌─ 会员与佣金层 ───────────────────────────────────────────────────────┐
├── member/              # 会员注册策略、自定义等级、升级规则引擎、批量审批
├── commission/          # 多模式返佣（8 子模块：core + common + 6 佣金插件）
│   ├── common/          #   公共类型、CommissionPlugin trait、11 种佣金模式
│   ├── core/            #   佣金调度引擎、NEX/Token 双管线、提现与复购分成
│   ├── referral/        #   直推返佣
│   ├── multi-level/     #   多级分销
│   ├── single-line/     #   单线上/下返佣
│   ├── pool-reward/     #   资金池奖励
│   ├── level-diff/      #   级差返佣
│   └── team/            #   团队业绩返佣
│  └──────────────────────────────────────────────────────────────────────┘
│
│  ┌─ 治理与合规层 ───────────────────────────────────────────────────────┐
├── governance/          # DAO 治理（40+ 提案类型、代币投票、委托、否决权）
├── disclosure/          # 财务披露（4 级）、内幕交易管控、公告管理、草稿流程
└── kyc/                 # KYC/AML 5 级认证、Provider 管理、风险评分、GDPR 清除
   └──────────────────────────────────────────────────────────────────────┘
```

## 子模块速查

| 模块 | Crate | 核心能力 | Extrinsics |
|------|-------|----------|------------|
| [common](./common/) | `pallet-entity-common` | 公共枚举、Trait 接口、分页类型、空实现 | — |
| [registry](./registry/) | `pallet-entity-registry` | Entity 创建/更新/关闭、运营资金、管理员权限、所有权转移、推荐人 | 28 |
| [shop](./shop/) | `pallet-entity-shop` | Shop CRUD、运营资金/积分系统（TTL/上限/兑换）、宽限期关闭、封禁/解封 | 33 |
| [product](./product/) | `pallet-entity-product` | 商品 CRUD/上下架、押金托管、批量操作、IPFS Pin、强制下架 | 9 |
| [order](./order/) | `pallet-entity-order` | NEX/Token 双资产下单、Escrow 托管、发货/收货/退款、服务类订单、超时自动处理 | 19 |
| [review](./review/) | `pallet-entity-review` | 评价提交/修改/删除、商家回复、商品评分索引、评价开关 | 5 |
| [token](./token/) | `pallet-entity-token` | 7 种代币类型、铸造/销毁、分红、锁仓、转账限制（白/黑名单/KYC/Members）、冻结 | 25 |
| [market](./market/) | `pallet-entity-market` | P2P 挂单（Limit/Market/IOC/FOK/PostOnly）、TWAP 预言机、熔断器、KYC 门槛 | 24+ |
| [tokensale](./tokensale/) | `pallet-entity-tokensale` | 5 种发售模式、多轮次、Vesting 线性解锁、白名单配额、软上限退款 | 27 |
| [member](./member/) | `pallet-entity-member` | 5 种注册策略、自定义等级、升级规则引擎（多条件）、批量审批、封禁/移除 | 33 |
| [commission](./commission/) | `pallet-entity-commission-*` | 11 种佣金模式、NEX/Token 双管线、提现复购分成、偿付保护 | 27 (core) |
| [governance](./governance/) | `pallet-entity-governance` | 40+ 提案类型、代币加权投票、时间加权乘数、委托、否决权、自动终结 | 18 |
| [disclosure](./disclosure/) | `pallet-entity-disclosure` | 4 级披露、黑窗口期、内幕人员管控、公告系统、草稿流程、违规追踪、多方审批、渐进式处罚、紧急披露 | 39 |
| [kyc](./kyc/) | `pallet-entity-kyc` | 5 级 KYC、Provider 授权、风险评分、国家限制、GDPR 清除、超时处理 | 25 |

## 核心类型

### EntityType — 实体类型

```
Merchant       商户（默认）     → 治理: None,    代币: Points,     转账限制: None,       KYC: ❌
Enterprise     企业             → 治理: FullDAO, 代币: Equity,     转账限制: Whitelist,   KYC: ✅
DAO            自治组织         → 治理: FullDAO, 代币: Governance, 转账限制: None,        KYC: ❌
Community      社区             → 治理: None,    代币: Membership, 转账限制: None,        KYC: ❌
Project        项目方           → 治理: FullDAO, 代币: Share,      转账限制: KycRequired, KYC: ✅
ServiceProvider 服务提供商      → 治理: None,    代币: Points,     转账限制: None,        KYC: ❌
Fund           基金             → 治理: FullDAO, 代币: Share,      转账限制: Whitelist,   KYC: ✅
Custom(u8)     自定义           → 治理: None,    代币: Points,     转账限制: None,        KYC: ❌
```

### TokenType — 通证权益矩阵

| 类型 | 投票权 | 分红权 | 可转让 | 默认 KYC | 默认转账限制 | 证券类 |
|------|--------|--------|--------|----------|-------------|--------|
| `Points` | ❌ | ❌ | ✅ | (0,0) | None | ❌ |
| `Governance` | ✅ | ❌ | ✅ | (2,2) | KycRequired | ❌ |
| `Equity` | ✅ | ✅ | ✅ | (3,3) | Whitelist | ✅ |
| `Membership` | ❌ | ❌ | ❌ | (1,1) | MembersOnly | ❌ |
| `Share` | ❌ | ✅ | ✅ | (2,2) | KycRequired | ✅ |
| `Bond` | ❌ | ✅ | ✅ | (2,2) | KycRequired | ✅ |
| `Hybrid(u8)` | ✅ | ✅ | ✅ | (2,2) | None | ❌ |

### 状态机

**Entity 状态（`EntityStatus`）：**

```
create_entity ──► Active ◄──── approve_entity ◄── Pending ◄── reopen_entity
                    │                                ▲
                    ├── self_pause ──► Suspended ─────┘ (self_resume / governance resume)
                    ├── request_close ──► PendingClose ──► Closed (approve / timeout)
                    │                        │
                    │                        └── cancel_close ──► Active
                    └── ban_entity ──► Banned ──► unban ──► Pending
```

**Shop 有效状态（`EffectiveShopStatus`）：**

由 `EntityStatus × ShopOperatingStatus` 实时计算，Entity 终态优先级最高。

```
Active | PausedBySelf | PausedByEntity | FundDepleted | Closed | ClosedByEntity | Closing | Banned
```

**订单流程（`OrderStatus`）：**

```
place_order ──► Paid ──┬── ship_order ──► Shipped ──┬── confirm_receipt ──► Completed
                       │                            ├── 超时自动确认 ──────► Completed
                       │                            └── request_refund ──► Disputed ──► Refunded/Completed
                       ├── cancel_order ──► Cancelled
                       ├── 发货超时 ──► Refunded
                       └── (数字商品) ──► Completed (立即)
```

## Entity-Shop 双层架构

```
Entity（组织层，1:N）                       Shop（业务层）
┌──────────────────────────┐             ┌──────────────────────────┐
│ owner + admins (位掩码)  │   1 : N     │ managers                 │
│ entity_type              │────────────►│ shop_type (7 种)         │
│ governance_mode          │             │ operating_fund (独立)    │
│ token    (统一发行)      │             │ products / orders        │
│ member   (统一体系)      │             │ rating / reviews         │
│ commission (统一返佣)    │             │ points (独立积分系统)    │
│ kyc      (统一认证)      │             │ location / policies      │
│ disclosure (统一披露)    │             │ closing grace period     │
└──────────────────────────┘             └──────────────────────────┘
```

- 每个 Entity 有且仅有一个 **Primary Shop**（创建 Entity 时自动创建，不可关闭）
- Entity 管理组织层面的代币、会员、佣金、治理和合规（全部 Shop 共享）
- Shop 管理独立的商品目录、订单、评分、运营资金和积分
- Shop 关闭采用**宽限期机制**（Closing → 期满 → Closed），期间可完成已有订单

## 跨模块 Trait 接口

所有子模块通过 `pallet-entity-common` 定义的 Trait 接口解耦，运行时通过关联类型桥接。

### 数据提供者

| Trait | 实现方 | 消费方 | 说明 |
|-------|--------|--------|------|
| `EntityProvider` | registry | shop, token, governance, member, commission, market, product, order, disclosure, kyc, tokensale | 实体查询、状态、管理员权限、派生账户 |
| `ShopProvider` | shop | product, order, review, commission | Shop 查询、运营资金、统计更新、积分 |
| `ProductProvider` | product | order | 商品查询、库存管理、价格 |
| `OrderProvider` | order | review, commission | 订单查询、状态检查、时间戳 |
| `EntityTokenProvider` | token | order, market | 代币余额、奖励、锁定/解锁、铸造 |
| `EntityTokenPriceProvider` | market | commission, tokensale | Token 价格（TWAP）、置信度评估 |
| `PricingProvider` | trading | registry, product, order | NEX/USDT 加权平均价格 |
| `MemberProvider` | member | commission, governance, order | 会员状态、等级、推荐链、消费统计 |
| `KycProvider` | kyc | token, member, tokensale | KYC 级别、过期检查 |
| `GovernanceProvider` | governance | registry | 治理模式、活跃提案、锁定状态 |
| `DisclosureProvider` | disclosure | token, market | 黑窗口期、内幕人员检查 |
| `TokenSaleProvider` | tokensale | governance | 发售轮次状态查询 |
| `FeeConfigProvider` | — | order, commission | 手续费配置查询 |
| `VestingProvider` | token | tokensale | 锁仓余额、释放、计划详情 |
| `DividendProvider` | token | governance | 分红查询、领取 |

### 事件通知

| Trait | 触发方 | 响应方 | 说明 |
|-------|--------|--------|------|
| `OnEntityStatusChange` | registry | shop, token, market | Entity 暂停/封禁/关闭/恢复时级联通知 |
| `OnOrderStatusChange` | order | commission, member | 订单状态变更时触发佣金/会员更新 |
| `OnKycStatusChange` | kyc | token, order | KYC 状态变更时通知下游 |

### 操作接口

| Trait | 调用方 | 实现方 | 说明 |
|-------|--------|--------|------|
| `CommissionFundGuard` | shop | commission | 查询已承诺佣金资金，防止运营扣费侵占 |
| `OrderCommissionHandler` | order | commission | NEX 订单完成/取消时触发佣金 |
| `TokenOrderCommissionHandler` | order | commission | Token 订单完成/取消时触发 Token 佣金 |
| `ShoppingBalanceProvider` | order | commission | 购物余额查询与消费 |
| `OrderMemberHandler` | order | member | 订单完成时自动注册会员 + 更新消费 |
| `EmergencyProvider` | — | — | 全局紧急暂停机制 |

## 管理员权限位掩码

Entity Admin 采用 `u32` 位掩码控制细粒度权限，Owner 天然拥有全部权限：

| 权限 | 位值 | 说明 |
|------|------|------|
| `SHOP_MANAGE` | `0x001` | Shop/产品管理 |
| `MEMBER_MANAGE` | `0x002` | 会员等级/审批 |
| `TOKEN_MANAGE` | `0x004` | Token 发售管理 |
| `ADS_MANAGE` | `0x008` | 广告位管理 |
| `REVIEW_MANAGE` | `0x010` | 评价系统开关 |
| `DISCLOSURE_MANAGE` | `0x020` | 披露/公告管理 |
| `ENTITY_MANAGE` | `0x040` | 实体信息/资金 |
| `KYC_MANAGE` | `0x080` | KYC 要求配置 |
| `GOVERNANCE_MANAGE` | `0x100` | 治理提案管理 |
| `ORDER_MANAGE` | `0x200` | 退款审批/争议 |
| `COMMISSION_MANAGE` | `0x400` | 返佣配置 |

## 佣金系统

佣金模块采用**插件化架构**，通过 `CommissionPlugin` trait 支持 11 种可组合的佣金模式：

```
CommissionModes (位标记，可自由组合):
  Direct           直推返佣
  MultiLevel       多级分销（最多 N 层）
  TeamPerformance  团队业绩返佣
  LevelDiff        级差返佣
  Fixed            固定金额返佣
  FirstOrder       首单特殊返佣
  RepeatPurchase   复购返佣
  SingleLineUp     单线上级返佣
  SingleLineDown   单线下级返佣
  PoolReward       资金池奖励
  CreatorReward    Entity 创建者奖励
```

佣金核心引擎（`commission-core`）支持 **NEX 和 Token 双资产管线**，提现时强制复购分成（可配置比例），通过 `CommissionFundGuard` 保护已承诺资金不被运营扣费侵占。

## 治理提案类型

`pallet-entity-governance` 支持 **40+ 种提案类型**，覆盖实体管理的各个方面：

| 类别 | 提案类型 |
|------|---------|
| **实体管理** | PauseEntity, ResumeEntity, CloseEntity, UpgradeEntityType, TransferOwnership |
| **Shop 管理** | PauseShop, ResumeShop, CloseShop, BanShop, UnbanShop |
| **代币经济** | MintTokens, BurnTokens, SetMaxSupply, ChangeTokenType, SetTransferRestriction, ConfigureDividend |
| **商品管理** | UpdateProductPrice, DelistProduct, SetProductInventory |
| **会员管理** | SetCustomLevels, SetUpgradeMode, AddCustomLevel, SetRegistrationPolicy, BanMember |
| **佣金配置** | SetCommissionModes, SetCommissionRate, ConfigureReferral, ConfigureMultiLevel |
| **合规披露** | ConfigureDisclosure, ResetViolations |
| **通用** | Custom(u16) 自定义提案 |

投票采用**代币加权 + 持有时间乘数**，支持委托投票（Compound 模型）和管理员否决权。

## 快速开始

### 依赖配置

```toml
[dependencies]
pallet-entity-common = { workspace = true }
pallet-entity-registry = { workspace = true }
pallet-entity-shop = { workspace = true }
pallet-entity-token = { workspace = true }
pallet-entity-governance = { workspace = true }
pallet-entity-member = { workspace = true }
pallet-entity-commission = { workspace = true }
pallet-entity-market = { workspace = true }
pallet-entity-product = { workspace = true }
pallet-entity-order = { workspace = true }
pallet-entity-review = { workspace = true }
pallet-entity-disclosure = { workspace = true }
pallet-entity-kyc = { workspace = true }
pallet-entity-tokensale = { workspace = true }
```

### 运行测试

```bash
# 核心模块
cargo test -p pallet-entity-common
cargo test -p pallet-entity-registry
cargo test -p pallet-entity-shop
cargo test -p pallet-entity-token
cargo test -p pallet-entity-governance
cargo test -p pallet-entity-member

# 业务模块
cargo test -p pallet-entity-product
cargo test -p pallet-entity-order
cargo test -p pallet-entity-review
cargo test -p pallet-entity-market
cargo test -p pallet-entity-tokensale
cargo test -p pallet-entity-disclosure
cargo test -p pallet-entity-kyc

# 佣金子模块
cargo test -p pallet-entity-commission-core
cargo test -p pallet-entity-commission-referral
cargo test -p pallet-entity-commission-multi-level
cargo test -p pallet-entity-commission-single-line
cargo test -p pallet-entity-commission-pool-reward
```

## 许可证

MIT License
