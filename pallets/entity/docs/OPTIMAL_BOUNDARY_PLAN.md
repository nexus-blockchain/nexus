# Entity 模块组最优功能边界 — 综合开发方案

> **日期**: 2026-03-12
> **输入**: ENTITY_MODULE_BOUNDARY_DEEP_AUDIT.md (审计一) + ENTITY_MODULE_BOUNDARY_REAUDIT_20260312.md (审计二)
> **方法**: 双文档交叉分析 + 全量代码验证，取两份审计的交集共识、化解分歧、产出最优路径

---

## 一、两份审计的共识与分歧

### 1.1 完全共识（6 项）

| # | 共识点 | 审计一 | 审计二 |
|---|--------|--------|--------|
| 1 | Token 购物余额必须从 commission/core 迁入 loyalty | P1 优先 | P1 列入 |
| 2 | common/traits/mod.rs (2719 行) 必须分文件 | P3 建议 | P0 建议 |
| 3 | GovernancePort 7 个类型全部为 `()` 空实现，必须接线 | P2 建议 | P2/P3 列入 |
| 4 | order 完成后的 6 个副作用耦合过重，需要解耦 | P4 Hook 化 | 隐含于 Commerce 层整理 |
| 5 | registry 混入资金/治理操作，职责膨胀 | P5 渐进精简 | P1 新增 treasury |
| 6 | governance ProposalType 88 变体过大 | 诊断问题 3 | §3.5 + §4.2 建议压缩 |

### 1.2 关键分歧与裁决

| 分歧点 | 审计一方案 | 审计二方案 | **本文裁决** | 理由 |
|--------|-----------|-----------|-------------|------|
| **资金域** | 渐进精简，registry 内通过 Port 委托 | 新增 treasury/ 顶层 pallet | **不新增 treasury pallet** | 资金分散在 Currency 派生账户中，不是独立存储；新增 pallet 要迁移 3 个模块的资金逻辑 + 改所有 runtime 接线，收益/成本比低。通过 `EntityTreasuryPort` trait 统一接口即可 |
| **dispute** | 保持 order 内部子模块 | 新增独立 dispute/ pallet | **保持 order 内部** | dispute 状态仍写回 Order.status，共享 ExpiryQueue；拆成独立 pallet 需要双向 Port + 存储迁移，当前 7 个函数规模不足以独立 |
| **token 激励策略** | 未提及迁出 | 从 token 迁出 reward_rate/exchange_rate 等到 loyalty | **暂不迁出** | 这些字段是 EntityTokenConfig 结构体的一部分，与资产配置紧耦合；迁出需要拆分 EntityTokenConfig + 修改所有读写路径，风险高收益低。通过 IncentiveStrategyPort 解耦即可 |
| **shop 的 EntityPrimaryShop** | 未单独提及 | 必须删除，收回 registry | **保留 shop 的 EntityPrimaryShop** | registry 和 shop 各有一份是因为 shop 内部需要频繁查询（防关闭/防转让）；删除后 shop 每次操作都要跨 pallet 查询 registry，性能下降。两边已通过 Hook 保持一致 |
| **member 内部拆分** | 未提及 | 拆成 4 个内部子文件 | **纳入 P2 阶段** | 合理但非紧急，member 当前 31 extrinsics 确实需要内部分文件 |

### 1.3 裁决原则

> **最小变更原则**: 不新增顶层 pallet，除非该域有独立存储 + 独立生命周期 + 3 个以上消费方。
> **Port 优先原则**: 边界问题优先通过 trait 接口解耦，而非物理搬迁代码。
> **代码验证原则**: 所有调整基于实际代码现状，不基于设计文档描述。

---

## 二、最优功能边界定义

### 2.1 六层架构

```
┌─────────────────────────────────────────────────────────────────────┐
│                      ENTITY MODULE GROUP                            │
│                                                                     │
│  ┌─── FOUNDATION (共享基础，无状态) ──────────────────────────────┐ │
│  │  common/                                                       │ │
│  │  ├── types/     纯数据类型 (DTO, Enum, Bitmask)               │ │
│  │  ├── traits/    跨域 Port 接口 (按角色分 7 个文件)            │ │
│  │  ├── errors.rs  共享错误常量                                   │ │
│  │  └── pagination.rs 分页工具                                    │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                     │
│  ┌─── ORGANIZATION (身份与组织) ──────────────────────────────────┐ │
│  │  registry/     Entity 生命周期 + Admin + 资金 + 主店           │ │
│  │  member/       会员注册 + 推荐链 + 层级 + 升级规则            │ │
│  │  kyc/          实名认证 + Provider + 风险评分                  │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                     │
│  ┌─── COMMERCE (商业交易) ────────────────────────────────────────┐ │
│  │  shop/         Shop 运营 + Manager + 资金 + 统计               │ │
│  │  product/      产品目录 + 库存 + 发布控制                      │ │
│  │  order/        订单状态机 + 结算 + 退款争议 + 超时自动化       │ │
│  │  review/       订单评价 + 评分聚合                             │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                     │
│  ┌─── INCENTIVE (激励与分润) ─────────────────────────────────────┐ │
│  │  commission/core     佣金引擎 + 提现 (NEX+Token 双管道)       │ │
│  │  commission/plugins  6 个计算插件                               │ │
│  │  loyalty/            积分 + 购物余额 (NEX+Token 统一)         │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                     │
│  ┌─── CAPITAL (Token 资产) ───────────────────────────────────────┐ │
│  │  token/        Entity Token 管理 + 分红 + 锁仓 + 限制         │ │
│  │  tokensale/    Token 发售 + Vesting                            │ │
│  │  market/       Token 交易 (Order Book DEX)                     │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                     │
│  ┌─── COMPLIANCE (治理与合规) ────────────────────────────────────┐ │
│  │  governance/   提案投票 + Port 执行 + 委托 + 资金保护          │ │
│  │  disclosure/   财务披露 + 内幕控制 + 公告                      │ │
│  └────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 每个模块的精确职责边界

#### common/ — 共享基础层 (无状态)

| 拥有 | 不拥有 |
|------|--------|
| 跨域共享 DTO (EntityStatus, ShopType, OrderStatus, ProductCategory ...) | 任何业务逻辑 |
| 跨域 Port trait (Provider / Handler / Hook / Guard / Port) | 只被单模块使用的本地 trait |
| 共享错误常量 (CommonError) | 业务 pallet 的 Error enum |
| 分页工具 (PageRequest / PageResponse) | — |

#### registry/ — Entity 注册中心

| 拥有 | 不拥有 |
|------|--------|
| Entity 创建 / 关闭 / 重开 / 状态迁移 | 订单、产品等下游生命周期 |
| Owner / Admin 权限管理 (add/remove/update/resign) | — |
| Entity 运营资金 (top_up_fund + 自动暂停/恢复 + 扣费) | Shop 运营资金 |
| 主店标识 (primary_shop_id in Entity struct) | Shop 具体创建/管理 |
| Entity → Shop 映射 (EntityShops) | — |
| 推荐人绑定 (EntityReferrer) | 推荐链/多级关系 (属 member) |
| 治理操作入口 (suspend/resume/ban/unban) | 提案投票逻辑 (属 governance) |
| Entity 类型 / 验证状态 | — |

#### shop/ — Shop 运营管理

| 拥有 | 不拥有 |
|------|--------|
| Shop 创建 / 更新 / 暂停 / 恢复 / 关闭 (两阶段) | Entity 生命周期 |
| Shop Manager 管理 | Admin 权限 (属 registry) |
| Shop 运营资金 (fund_operating / withdraw_operating_fund / 派生账户) | Entity 运营资金 |
| EntityPrimaryShop 缓存 (防关闭/防转让的快速查询) | 主店设置 (属 registry) |
| Shop 类型 / 位置 / 策略 | — |
| Ban 状态管理 (ban_shop / unban_shop) | — |

#### product/ — 产品目录

| 拥有 | 不拥有 |
|------|--------|
| 产品 CRUD (create/update/delete) | 订单中的产品快照 |
| 发布 / 下架控制 (含批量) | 库存扣减 (由 order 触发) |
| 产品押金 / 强制下架 | — |
| 产品统计 (ProductStats) | — |

#### order/ — 订单交易引擎

| 拥有 | 不拥有 |
|------|--------|
| 订单状态机: 下单 → 发货 → 确认 → 完成 | 佣金计算 (触发 CommissionHandler) |
| 支付双轨: NEX (Escrow) + Token (Reserve/Repatriate) | 积分发放 (触发 LoyaltyWritePort) |
| 退款/争议子流程 (dispute.rs 内 7 个函数) | 会员注册/升级 (触发 MemberProvider) |
| 超时自动化 (on_idle + ExpiryQueue) | Shop 统计更新 (触发 ShopProvider) |
| 代付 (place_order_for) | — |
| 清理 (cleanup_buyer/shop/payer_orders) | — |
| 平台费率 (PlatformFeeRate) | — |

#### commission/core — 佣金引擎

| 拥有 | 不拥有 |
|------|--------|
| NEX + Token 佣金分发引擎 (5 plugin 调度) | 具体计算算法 (属 plugins) |
| 佣金配置 (CommissionConfigs / Modes / Rate) | — |
| NEX + Token 提现系统 (配置 + 历史 + 冷却) | — |
| Entity 资金提取 (withdraw_entity_funds / withdraw_entity_token_funds) | — |
| **Token 购物余额 (待迁出)**: MemberTokenShoppingBalance, TokenShoppingTotal | NEX 购物余额 (已属 loyalty) |
| 沉淀池 (UnallocatedPool / TokenPool) | — |
| 治理控制 (GlobalPause / MaxRate / MinRepurchase) | — |
| Creator Reward | — |

#### commission/plugins — 佣金计算插件 (6 个)

| Plugin | Pallet Index | 职责 |
|--------|-------------|------|
| referral | 133 | 直推奖 |
| multi_level | 138 | 多级分销 |
| level_diff | 134 | 级差奖 |
| single_line | 135 | 单线排队 |
| team | 136 | 团队绩效 |
| pool_reward | 137 | 沉淀池分红 |

每个 plugin 只做计算 + 写入自己的 Plan 配置，不做资金操作。

#### loyalty/ — 积分与购物余额

| 拥有 | 不拥有 |
|------|--------|
| Shop 积分系统 (6 个存储项: Config/Balance/TotalSupply/TTL/ExpiresAt/MaxSupply) | Token 资产管理 |
| NEX 购物余额 (MemberShoppingBalance / ShopShoppingTotal) | Token 购物余额 (**待接收**) |
| 积分 CRUD (enable/disable/issue/burn/transfer/redeem/expire) | 佣金计算 |
| PointsCleanup (Shop 关闭时清理积分) | — |

#### token/ — Entity Token 资产

| 拥有 | 不拥有 |
|------|--------|
| pallet-assets 桥接: 创建/Mint/Burn/Transfer | Token 公开发售 (属 tokensale) |
| 7 种 TokenType (Points/Governance/Equity/Membership/Share/Bond/Hybrid) | Token 交易撮合 (属 market) |
| 分红 (Dividend) | 佣金分发 |
| 锁仓/解锁 (Lock/Unlock/Vesting) | — |
| 转让限制 (Whitelist/Blacklist/KYC/MembersOnly) | — |
| 内幕交易黑盒期控制 | — |
| Token 激励参数 (reward_rate/exchange_rate/min_redeem 等) | — |
| AssetLedgerPort (blanket impl from EntityTokenProvider) | — |

#### tokensale/ — Token 发售

| 拥有 | 不拥有 |
|------|--------|
| 多模式发售 (固定价/荷兰拍/白名单/先到先得/抽签) | Token 资产操作 (通过 EntityTokenProvider) |
| Sale-round Vesting (线性/悬崖/自定义) | 资产级 Lock/Vesting (属 token) |
| KYC 集成 / 退款 (软顶) | — |

#### market/ — Token 交易市场

| 拥有 | 不拥有 |
|------|--------|
| Order Book DEX (限价/市价/IOC/FOK/Post-only) | Token 资产操作 (通过 EntityTokenProvider) |
| 熔断器 (价格偏差保护) | — |
| TWAP 计算 | — |
| KYC 门控 / 内幕交易控制 | — |

#### member/ — 会员体系

| 拥有 | 不拥有 |
|------|--------|
| 会员注册 / 审批 / 封禁 / 移除 | Entity Admin (属 registry) |
| 推荐关系 (Referral tree) | Entity 推荐人 (属 registry) |
| 自定义层级系统 (Tier) | — |
| 基于消费/活动的升级规则 | — |
| 注册策略 (Open/ReferralRequired/Approval/KYC) | — |

#### review/ — 订单评价

| 拥有 | 不拥有 |
|------|--------|
| 评价 (1-5 星 + IPFS 内容) | 仲裁/争议 (属 order/dispute.rs) |
| 卖家回复 | — |
| 评价编辑 (单次窗口) | — |
| 产品评分聚合 | — |

#### governance/ — 治理引擎

| 拥有 | 不拥有 |
|------|--------|
| 提案生命周期 (create → vote → finalize → execute) | 具体业务操作 (通过 GovernancePort 分发) |
| 88 ProposalType + 14 ProposalDomain 分类 | — |
| 投票 + 委托 + 否决 | — |
| 时间加权投票 + 闪电贷防护 | — |
| on_idle 自动 finalize + expire | — |
| FundProtectionConfig (非阻塞告警) | — |

#### disclosure/ — 合规披露

| 拥有 | 不拥有 |
|------|--------|
| 多级财务披露 (Basic/Standard/Enhanced/Full) | 提案投票 (属 governance) |
| 草稿 → 发布 → 更正工作流 | — |
| 内幕人管理 + 交易黑盒期 | — |
| 违规追踪 / 处罚 | — |
| 公告系统 (置顶/过期) | — |

#### kyc/ — 实名认证

| 拥有 | 不拥有 |
|------|--------|
| per-Entity KYC 记录 | 会员注册 (属 member) |
| 5 级认证 (None→Basic→Standard→Enhanced→Institutional) | — |
| Provider 授权 / 管理 | — |
| 风险评分 / 国家限制 | — |

---

## 三、模块间依赖关系

### 3.1 数据流向图

```
                          ┌──────────┐
                          │  common  │ ← 所有模块依赖 (纯类型+接口)
                          └────┬─────┘
                               │
              ┌────────────────┼───────────────────┐
              ▼                ▼                    ▼
        ┌──────────┐    ┌──────────┐         ┌──────────┐
        │ registry │    │   kyc    │         │disclosure│
        └────┬─────┘    └────┬─────┘         └────┬─────┘
             │               │                    │
     ┌───────┼───────┐      │              ┌─────┘
     ▼       ▼       ▼      ▼              ▼
┌────────┐┌──────┐┌──────┐┌──────┐   ┌─────────┐
│  shop  ││member││token ││market│   │tokensale│
└───┬────┘└──┬───┘└──┬───┘└──────┘   └─────────┘
    │        │       │
    ▼        │       ▼
┌────────┐   │  ┌─────────┐
│product │   │  │ loyalty │ ← NEX + Token 购物余额 + 积分
└───┬────┘   │  └────┬────┘
    │        │       │
    ▼        ▼       ▼
┌─────────────────────────┐
│         order           │ ← 核心交易引擎
└────────────┬────────────┘
             │ CommissionHandler::on_order_completed
             ▼
┌─────────────────────────┐
│   commission (core)     │ ← 佣金分发引擎
│   + 6 plugins           │
└────────────┬────────────┘
             │
             ▼
┌─────────────────────────┐
│      governance         │ ← 通过 GovernancePort 操作所有下游
└─────────────────────────┘

┌──────────┐
│  review  │ ← 叶子节点 (依赖 order + shop)
└──────────┘
```

### 3.2 Trait 依赖矩阵 (提供方 → 消费方)

```
EntityProvider (registry) →
  shop, product, order, token, tokensale, market, member,
  commission, loyalty, governance, review, kyc, disclosure

ShopProvider (shop) →
  registry, product, order, commission, loyalty, governance

ProductProvider (product) →
  order, shop, governance

OrderProvider (order) →
  registry, review, governance

EntityTokenProvider / AssetLedgerPort (token) →
  order, tokensale, market, commission, loyalty, governance

MemberProvider (member) →
  order, commission, governance, token

GovernanceProvider (governance) →
  registry, commission

LoyaltyWritePort + LoyaltyReadPort (loyalty) →
  order, commission

CommissionFundGuard (commission) →
  shop, loyalty

KycProvider (kyc) →
  token, tokensale, market, commission (via ParticipationGuard)

DisclosureProvider (disclosure) →
  token, tokensale, market, governance
```

### 3.3 订单完成数据流

```
用户确认收货 → order::do_complete_order()
  │
  ├─1→ Escrow::release() / Token::repatriate()           [资金转移 - 核心]
  ├─2→ ShopProvider::update_shop_stats()                   [Shop 统计]
  ├─3→ CommissionHandler::on_order_completed()              [佣金分发]
  │      └→ 6 plugins 顺序调度 → 积分/购物余额
  ├─4→ MemberProvider::auto_register()                     [会员自动注册]
  ├─5→ MemberProvider::update_spent() + check_upgrade()    [消费累积/升级]
  └─6→ Loyalty::reward_on_purchase()                       [积分奖励]
```

---

## 四、调整计划与实施步骤

### Phase 5.1 — 基础收口 (低风险，高收益)

#### 任务 A: common/traits 分文件

**目标**: 将 2719 行的 `traits/mod.rs` 拆分为 7 个语义文件

**当前状态**:
```
common/src/traits/
└── mod.rs          ← 2719 行，所有 50+ trait 混在一起
```

**目标状态**:
```
common/src/traits/
├── mod.rs              ← 仅 re-export (~50 行)
├── core.rs             ← EntityProvider, ShopProvider, ProductProvider, OrderProvider (~400 行)
├── asset.rs            ← EntityTokenProvider, AssetLedgerPort, PricingProvider,
│                          EntityTokenPriceProvider, TokenTransferProvider (~350 行)
├── member.rs           ← MemberProvider, MemberQueryProvider, KycProvider,
│                          EntityReferrerProvider, ParticipationGuard (~300 行)
├── incentive.rs        ← CommissionHandler, CommissionFundGuard, CommissionProvider,
│                          LoyaltyReadPort, LoyaltyWritePort, PoolBalanceProvider (~350 行)
├── governance_ports.rs ← MarketGovernancePort, CommissionGovernancePort, SingleLineGovernancePort,
│                          KycGovernancePort, ShopGovernancePort, TokenGovernancePort,
│                          EntityTreasuryPort, GovernanceProvider (~350 行)
├── hooks.rs            ← OnEntityStatusChange, OnOrderStatusChange, OnKycStatusChange,
│                          StoragePin, PointsCleanup (~200 行)
└── null_impls.rs       ← 所有 Null* / () impl 集中 (~500 行)
```

**变更规则**:
- 纯文件移动 + re-export，不改任何 trait 签名
- `mod.rs` 使用 `pub use core::*; pub use asset::*; ...` 保持外部 import 路径不变
- 同时统一重复的 KYC trait：member/token/tokensale 中的本地 `KycChecker`/`KycLevelProvider` 改为 import `common::KycProvider`

**验收标准**:
- `cargo build --release` 零错误零警告
- 所有现有测试通过
- 外部 import 路径不变 (通过 re-export)

---

#### 任务 B: Token 购物余额迁入 loyalty

**目标**: 将 commission/core 中的 Token 购物余额统一到 loyalty，消除概念分裂

**当前状态**:
```
commission/core/src/lib.rs:
  Line 515: MemberTokenShoppingBalance<T>  (DoubleMap)
  Line 526: TokenShoppingTotal<T>          (Map)
  settlement.rs: do_credit_token_shopping_balance() / do_consume_token_shopping_balance()

loyalty/src/lib.rs:
  Line 185: MemberShoppingBalance<T>       (DoubleMap) ← NEX
  Line 174: ShopShoppingTotal<T>           (Map)       ← NEX
```

**目标状态**:
```
loyalty/src/lib.rs:
  MemberShoppingBalance<T>          ← NEX (已有)
  ShopShoppingTotal<T>              ← NEX (已有)
  + MemberTokenShoppingBalance<T>   ← Token (迁入)
  + TokenShoppingTotal<T>           ← Token (迁入)
  + credit_token_shopping_balance()
  + consume_token_shopping_balance()

commission/core/src/settlement.rs:
  do_credit_token_shopping_balance()  → T::Loyalty::credit_token_shopping_balance()
  do_consume_token_shopping_balance() → T::Loyalty::consume_token_shopping_balance()
```

**变更清单**:

| 文件 | 变更 |
|------|------|
| `common/src/traits/mod.rs` (→ incentive.rs) | `LoyaltyWritePort` 新增 `credit_token_shopping_balance()` / `consume_token_shopping_balance()` |
| `loyalty/src/lib.rs` | 新增 2 个 Token 存储项 + 实现方法 |
| `commission/core/src/lib.rs` | 删除 `MemberTokenShoppingBalance` / `TokenShoppingTotal` 存储声明 |
| `commission/core/src/settlement.rs` | 替换直接存储操作为 `T::Loyalty::*` 调用 |
| `runtime/src/configs/mod.rs` | `LoyaltyBridge` 新增 Token 购物余额方法转发 |
| `commission/core/src/mock.rs` | `MockLoyaltyProvider` 新增 Token 方法的 mock |

**存储迁移**: 需要 `on_runtime_upgrade` 将 commission/core 中的两个 StorageMap 数据搬到 loyalty 的新存储项中。

**验收标准**:
- commission-core 208 测试全过
- loyalty 测试覆盖新增 Token 购物余额功能
- `cargo build --release` 零错误零警告

---

### Phase 5.2 — GovernancePort 完全启用 (中等复杂度)

**目标**: 让 governance 的 execute_proposal 100% 通过 Port 分发，不再直接调用 Provider 方法

**当前状态** (runtime/src/configs/mod.rs):
```rust
type MarketGovernance = ();       // Line 1733
type CommissionGovernance = ();   // Line 1734
type SingleLineGovernance = ();   // Line 1735
type KycGovernance = ();          // Line 1736
type ShopGovernance = ();         // Line 1737
type TokenGovernance = ();        // Line 1738
type TreasuryPort = ();           // Line 1740
```

**目标状态**:
```rust
type MarketGovernance = EntityMarket;
type CommissionGovernance = CommissionCore;
type SingleLineGovernance = CommissionSingleLine;
type KycGovernance = EntityKyc;
type ShopGovernance = EntityShop;
type TokenGovernance = EntityToken;
type TreasuryPort = EntityRegistry;  // registry 实现 EntityTreasuryPort
```

**实施步骤 (逐个 Port)**:

| 步骤 | Port | 目标 Pallet | 需要实现的方法 |
|------|------|------------|---------------|
| 5.2.1 | `ShopGovernancePort` | EntityShop | 积分相关治理操作 |
| 5.2.2 | `TokenGovernancePort` | EntityToken | 黑名单/限制相关治理操作 |
| 5.2.3 | `MarketGovernancePort` | EntityMarket | 市场暂停/恢复/配置 |
| 5.2.4 | `CommissionGovernancePort` | CommissionCore | 佣金暂停/费率上限 |
| 5.2.5 | `SingleLineGovernancePort` | CommissionSingleLine | 单线暂停/恢复/配置 |
| 5.2.6 | `KycGovernancePort` | EntityKyc | Provider 授权/取消 |
| 5.2.7 | `EntityTreasuryPort` | EntityRegistry | 资金保护/支出限额 |

**每个 Port 的实施模式**:
```rust
// 1. 在目标 pallet 中新增 impl block
impl<T: Config> ShopGovernancePort for Pallet<T> {
    fn governance_update_points_config(...) -> DispatchResult {
        // 大部分方法已在 Provider trait 中有对应实现
        // 新增 impl 转发调用即可
    }
}

// 2. 在 governance/src/lib.rs 的 execute_proposal 中
// 将直接 Provider 调用替换为 Port 调用:
//   Before: T::ShopProvider::update_points_config(...)
//   After:  T::ShopGovernance::governance_update_points_config(...)

// 3. 在 runtime 中更新类型绑定
```

**验收标准**:
- governance 16 测试全过
- 每个下游 pallet 测试不受影响
- execute_proposal 中不再有直接 Provider 方法调用
- `cargo build --release` 零错误零警告

---

### Phase 5.3 — Order 副作用 Hook 化 (高复杂度，可选)

**目标**: order 模块只关心订单状态机 + 资金转移，所有"完成后副作用"由 Hook 链处理

**方案**:

```rust
// common/src/traits/hooks.rs — 新增组合 Hook trait
pub trait OnOrderCompleted<AccountId, Balance> {
    fn on_completed(info: &OrderCompletionInfo<AccountId, Balance>) -> DispatchResult;
}

/// OrderCompletionInfo 包含:
/// - order_id, entity_id, shop_id
/// - buyer, payer, total_amount
/// - product_snapshots, referrer
/// - payment_type (NEX/Token)

// 为 tuple 自动实现 (Substrate 标准模式):
impl<A, B, C, D, Acc, Bal> OnOrderCompleted<Acc, Bal> for (A, B, C, D)
where
    A: OnOrderCompleted<Acc, Bal>,
    B: OnOrderCompleted<Acc, Bal>,
    C: OnOrderCompleted<Acc, Bal>,
    D: OnOrderCompleted<Acc, Bal>,
{ ... }
```

**Runtime 配线**:
```rust
// runtime/src/configs/mod.rs
type OnOrderCompleted = (
    ShopStatsBridge,          // → shop 统计更新
    CommissionBridge,         // → 佣金分发
    MemberAutoRegisterBridge, // → 会员注册/升级
    LoyaltyBridge,            // → 积分奖励
);
```

**变更清单**:

| 文件 | 变更 |
|------|------|
| `common/src/traits/hooks.rs` | 新增 `OnOrderCompleted` trait + `OrderCompletionInfo` DTO |
| `order/src/lib.rs` Config | 新增 `type OnOrderCompleted: OnOrderCompleted<...>` |
| `order/src/lib.rs` do_complete_order | 替换 6 处直接调用为 `T::OnOrderCompleted::on_completed(&info)` |
| `runtime/src/configs/mod.rs` | 新增 4 个 Bridge impl + 配线 |
| 各 Bridge | 转发到现有 Provider 方法 |
| `order/src/mock.rs` | Mock OnOrderCompleted |

**收益**:
- order 模块从 12 个外部 trait 依赖降至 ~6 个
- 新增副作用只需在 runtime 配线中追加，不改 order 代码
- 副作用执行顺序在 runtime 层面可控

---

### Phase 5.4 — 模块内部分文件 (低风险，可并行)

#### member/ 内部拆分

```
member/src/
├── lib.rs              ← #[pallet] 宏 + 存储 + extrinsic 入口
├── registration.rs     ← do_register / do_approve / do_reject / do_ban / do_remove
├── referral.rs         ← do_bind_referrer / get_referral_chain / team_size
├── level.rs            ← add_custom_level / remove_level / set_upgrade_mode
└── upgrade_rule.rs     ← add_upgrade_rule / check_rules / do_auto_upgrade
```

#### market/ 内部拆分

```
market/src/
├── lib.rs              ← #[pallet] 宏 + 存储 + extrinsic 入口
├── engine.rs           ← place_order / cancel_order / modify_order + 撮合核心
├── orderbook.rs        ← 价格档位管理 / 深度快照
├── oracle.rs           ← TWAP 计算 / 价格快照
└── risk.rs             ← 熔断器 / KYC 门控 / 黑盒期 / 价格保护
```

---

## 五、实施路径与时间线

```
Phase 5.1 (1~2 天) — 并行执行:
  ├─ 任务 A: common/traits 分文件 (纯重构)
  └─ 任务 B: Token 购物余额迁入 loyalty

Phase 5.2 (3~5 天) — 逐个 Port 串行:
  └─ 7 个 GovernancePort 逐一实现 + 接线

Phase 5.3 (2~3 天) — 可选:
  └─ Order 副作用 Hook 化

Phase 5.4 (1~2 天) — 可与 5.2/5.3 并行:
  ├─ member/ 内部分文件
  └─ market/ 内部分文件
```

---

## 六、风险控制

| 风险 | 缓解措施 |
|------|---------|
| 存储迁移 (Token 购物余额) 丢数据 | 编写 try-runtime 测试验证迁移前后数据一致 |
| GovernancePort 接线后提案执行失败 | 每接一个 Port 就跑 governance 全量测试 + 新增 Port 级单元测试 |
| common 分文件后外部 import 路径断裂 | mod.rs 保持 `pub use *` re-export，CI 全量编译验证 |
| Order Hook 化后副作用执行顺序变化 | Hook tuple 顺序与现有代码调用顺序一致，保证行为等价 |

---

## 七、最终模块统计 (调整后预期)

| 模块 | 存储项 | Extrinsics | Pallet Index | 变化 |
|------|--------|-----------|-------------|------|
| common | 0 | 0 | — | traits 分 7 文件 |
| registry | 13 | 25 | 120 | 不变 |
| shop | 7 | 20 | 129 | 不变 |
| product | 5 | 10 | 121 | 不变 |
| order | 8 | 25 | 122 | Config 新增 OnOrderCompleted (Phase 5.3) |
| review | 9 | 5 | 123 | 不变 |
| commission/core | **36** (-2) | **27** | 127 | 迁出 Token 购物余额 2 存储项 |
| commission/plugins | 各 3-5 | 各 2-5 | 133-138 | 不变 |
| loyalty | **10** (+2) | 10 | 139 | 接收 Token 购物余额 2 存储项 |
| token | 12+ | 34 | 124 | 实现 TokenGovernancePort |
| tokensale | 10 | 26+ | 132 | 不变 |
| market | 21 | 27+ | 128 | 实现 MarketGovernancePort |
| member | 16 | 32 | 126 | 内部分 4 文件 |
| governance | 22 | 16 | 125 | execute_proposal 100% 走 Port |
| disclosure | 23 | 41 | 130 | 不变 |
| kyc | 11+ | 25 | 131 | 实现 KycGovernancePort |

---

## 八、本文档与两份审计的关系

| 采纳来源 | 采纳内容 |
|---------|---------|
| 审计一 (DEEP_AUDIT) | 6 层架构划分、P1-P5 优先级框架、调整 1-5 的详细变更清单、职责矩阵 |
| 审计二 (REAUDIT) | 6 个越界点诊断、"第一轮重构只做了一半"判断、KYC trait 统一建议、member/market 内部拆分 |
| 本文独立裁决 | 不新增 treasury/dispute pallet、不迁 token 激励参数、保留 shop 的 EntityPrimaryShop |

> **核心理念**: 边界问题优先通过 **Port trait 解耦**，而非物理搬迁代码。
> 只在概念分裂明确且迁移成本可控时才做存储迁移 (如 Token 购物余额)。
> 不做预防性重构，不做假设性抽象。
