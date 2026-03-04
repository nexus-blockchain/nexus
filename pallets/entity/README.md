# Entity 模块 (pallet-entity)

> NEXUS 实体管理系统 — 支持多类型实体、Entity-Shop 双层架构、通证发行、DAO 治理、多模式返佣、KYC 合规与代币发售

## 概述

Entity 模块是 NEXUS 平台的核心业务模块套件，提供通用实体管理能力。采用 **Entity（组织层）+ Shop（业务层）** 双层架构，支持商户、企业、投资基金、DAO、社区等多种实体类型。模块集群由 15 个子模块组成，覆盖实体注册、店铺运营、通证经济、会员体系、佣金分配、订单交易、DAO 治理、合规披露和代币发售全链路。

## 模块架构

```
pallets/entity/
├── common/              # 公共类型和跨模块 Trait（纯 Rust crate，无链上存储）
│
│  ┌─ 组织层 ─────────────────────────────────────────────┐
├── registry/            # 实体注册、生命周期、多管理员权限
│  └──────────────────────────────────────────────────────┘
│
│  ┌─ 业务层 ─────────────────────────────────────────────┐
├── shop/                # Shop 管理、运营资金、双层状态
├── product/             # 商品/服务 CRUD、库存管理
├── order/               # 订单全流程（下单→发货→收货→退款）
├── review/              # 评价系统（订单完成后评价）
│  └──────────────────────────────────────────────────────┘
│
│  ┌─ 通证经济层 ─────────────────────────────────────────┐
├── token/               # Entity 通证（7 种类型、分红、锁仓、转账限制）
├── market/              # P2P 代币交易市场（NEX/USDT 双市场、TWAP 预言机）
├── tokensale/           # 代币发售（固定价格/荷兰拍、多资产支付、Vesting）
│  └──────────────────────────────────────────────────────┘
│
│  ┌─ 会员与佣金层 ───────────────────────────────────────┐
├── member/              # 会员注册、等级系统、升级规则引擎
├── commission/          # 多模式返佣（8 个子模块：core + common + 6 佣金插件）
│  └──────────────────────────────────────────────────────┘
│
│  ┌─ 治理与合规层 ───────────────────────────────────────┐
├── governance/          # DAO 治理（提案、投票、委托、执行）
├── disclosure/          # 财务披露、公告管理、内幕交易控制
└── kyc/                 # KYC/AML 四级认证（None/Basic/Standard/Enhanced）
   └──────────────────────────────────────────────────────┘
```

## 子模块概览

| 模块 | Crate | 说明 | 代码行数 | 测试数 |
|------|-------|------|----------|--------|
| [common](./common/README.md) | `pallet-entity-common` | 公共类型、Trait 接口、空实现 | ~1,686 | — |
| [registry](./registry/README.md) | `pallet-entity-registry` | 实体注册、状态管理、多管理员权限位掩码 | ~2,311 | 168 |
| [shop](./shop/README.md) | `pallet-entity-shop` | Shop 管理、运营资金、积分系统 | ~2,039 | 143 |
| [token](./token/README.md) | `pallet-entity-token` | 通证创建/铸造/分红/锁仓/转账限制 | ~1,799 | 102 |
| [governance](./governance/README.md) | `pallet-entity-governance` | 多模式治理、提案、委托投票、代币锁定 | ~1,901 | 115 |
| [member](./member/README.md) | `pallet-entity-member` | 会员注册策略、自定义等级、升级规则引擎 | ~3,282 | 131 |
| [commission](./commission/README.md) | `pallet-entity-commission-*` | 8 子模块返佣体系（core + common + 6 插件） | ~9,313 | 450 |
| [market](./market/README.md) | `pallet-entity-market` | NEX/USDT 双市场、限价/市价单、TWAP | ~4,897 | 82 |
| [product](./product/README.md) | `pallet-entity-product` | 商品 CRUD、库存管理、NEX/USDT 定价 | ~1,113 | 96 |
| [order](./order/README.md) | `pallet-entity-order` | 订单全流程、NEX/Token 双支付、佣金触发 | ~1,699 | 118 |
| [review](./review/README.md) | `pallet-entity-review` | 评价提交、评分聚合、Root 删除 | ~388 | 61 |
| [disclosure](./disclosure/README.md) | `pallet-entity-disclosure` | 财务披露、公告管理、Blackout 窗口 | ~2,080 | 155 |
| [kyc](./kyc/README.md) | `pallet-entity-kyc` | KYC/AML 四级认证、Provider 管理 | ~1,183 | 106 |
| [tokensale](./tokensale/README.md) | `pallet-entity-tokensale` | 代币发售轮次、多资产支付、Vesting 解锁 | ~2,094 | 107 |

## 核心类型

### EntityType — 实体类型

| 变体 | 说明 | 默认治理 | 默认代币 | 默认转账限制 | 默认需 KYC |
|------|------|----------|----------|-------------|-----------|
| `Merchant` | 商户（默认） | None | Points | None | ❌ |
| `Enterprise` | 企业 | FullDAO | Equity | Whitelist | ✅ |
| `DAO` | 去中心化自治组织 | FullDAO | Governance | None | ❌ |
| `Community` | 社区 | None | Membership | None | ❌ |
| `Project` | 项目方 | FullDAO | Share | KycRequired | ✅ |
| `ServiceProvider` | 服务提供商 | None | Points | None | ❌ |
| `Fund` | 基金 | FullDAO | Share | Whitelist | ✅ |
| `Custom(u8)` | 自定义类型 | None | Points | None | ❌ |

### TokenType — 通证权益矩阵

| 类型 | 投票权 | 分红权 | 可转让 | KYC 级别 | 默认转账限制 | 证券类 |
|------|--------|--------|--------|----------|-------------|--------|
| `Points` | ❌ | ❌ | ✅ | None (0,0) | None | ❌ |
| `Governance` | ✅ | ❌ | ✅ | Standard (2,2) | KycRequired | ❌ |
| `Equity` | ✅ | ✅ | ✅ | Enhanced (3,3) | Whitelist | ✅ |
| `Membership` | ❌ | ❌ | ❌ | Basic (1,1) | MembersOnly | ❌ |
| `Share` | ❌ | ✅ | ✅ | Standard (2,2) | KycRequired | ✅ |
| `Bond` | ❌ | ✅ | ✅ | Standard (2,2) | KycRequired | ✅ |
| `Hybrid(u8)` | ✅ | ✅ | ✅ | Standard (2,2) | None | ❌ |

### GovernanceMode — 治理模式

| 模式 | 说明 |
|------|------|
| `None` | 无治理（管理员全权控制，默认） |
| `FullDAO` | 完全 DAO（所有决策需代币投票） |

### TransferRestrictionMode — 转账限制

| 模式 | 说明 |
|------|------|
| `None` | 无限制（默认） |
| `Whitelist` | 仅白名单地址可接收 |
| `Blacklist` | 黑名单地址禁止接收 |
| `KycRequired` | 接收方需满足 KYC 要求 |
| `MembersOnly` | 仅实体成员间可转账 |

## 跨模块 Trait 接口

Entity 子模块通过 `pallet-entity-common` 定义的 Trait 接口实现松耦合：

| Trait | 实现方 | 消费方 | 说明 |
|-------|--------|--------|------|
| `EntityProvider` | registry | shop, token, governance, member, commission, market, product, order, disclosure, kyc, tokensale | 实体查询、状态、管理员权限 |
| `ShopProvider` | shop | product, order, review, commission | Shop 查询、运营资金、统计更新 |
| `ProductProvider` | product | order | 商品查询、库存管理 |
| `OrderProvider` | order | review, commission | 订单查询、状态检查 |
| `EntityTokenProvider` | token | order, market | 代币余额、奖励、锁定/解锁 |
| `EntityTokenPriceProvider` | market | commission, tokensale | Token 价格查询（TWAP） |
| `PricingProvider` | trading/pricing | registry, product | NEX/USDT 定价 |
| `MemberProvider` | member | commission, governance, order | 会员查询、等级、推荐链 |
| `KycProvider` | kyc | token, member, tokensale | KYC 级别查询 |
| `GovernanceProvider` | governance | registry | 治理状态查询 |
| `DisclosureProvider` | disclosure | token, market | 黑窗口期、内幕人员检查 |
| `CommissionFundGuard` | commission | shop | 佣金资金保护 |
| `OrderCommissionHandler` | commission | order | NEX 订单佣金触发 |
| `TokenOrderCommissionHandler` | commission | order | Token 订单佣金触发 |
| `ShoppingBalanceProvider` | commission | order | 购物余额抵扣 |
| `OrderMemberHandler` | member | order | 自动注册、消费更新 |

## Entity-Shop 双层架构

```
Entity（组织层）                    Shop（业务层）
┌─────────────────────┐         ┌─────────────────────┐
│ owner / admins      │ 1:N     │ managers             │
│ entity_type         │────────►│ shop_type            │
│ governance_mode     │         │ operating_fund       │
│ token (统一)        │         │ products / orders    │
│ member (统一)       │         │ rating / reviews     │
│ commission (统一)   │         │ points (独立)        │
└─────────────────────┘         └─────────────────────┘
```

- **Entity** 管理组织层面的代币、会员、佣金和治理（全部 Shop 共享）
- **Shop** 管理独立的商品、订单、评分和运营资金
- 每个 Entity 有且仅有一个 **Primary Shop**（创建 Entity 时自动创建）
- Shop 的有效状态由 `EffectiveShopStatus::compute(entity_status, shop_status)` 实时计算

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

### 测试

```bash
# 测试所有 Entity 子模块
cargo test -p pallet-entity-common
cargo test -p pallet-entity-registry
cargo test -p pallet-entity-shop
cargo test -p pallet-entity-token
cargo test -p pallet-entity-governance
cargo test -p pallet-entity-member
cargo test -p pallet-entity-commission-core
cargo test -p pallet-entity-commission-referral
cargo test -p pallet-entity-commission-multi-level
cargo test -p pallet-entity-commission-single-line
cargo test -p pallet-entity-commission-pool-reward
cargo test -p pallet-entity-market
cargo test -p pallet-entity-product
cargo test -p pallet-entity-order
cargo test -p pallet-entity-review
cargo test -p pallet-entity-disclosure
cargo test -p pallet-entity-kyc
cargo test -p pallet-entity-tokensale
```

## 许可证

MIT License
