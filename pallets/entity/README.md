# Entity — 实体商业系统

NEXUS 核心商业领域模块，管理去中心化商业实体的完整生命周期。包含 16 个子模块，覆盖注册、商铺、商品、订单、会员、佣金、治理、合规等全链路。

## 模块架构

```
entity/
├── common/           # 共享类型库 (无 Storage)
├── registry/         # 实体注册与生命周期 [120]
├── shop/             # 商铺管理 [129]
├── product/          # 商品目录 [121]
├── order/            # 订单引擎 [122]
├── review/           # 评价系统 [123]
├── member/           # 会员与推荐 [126]
├── token/            # 实体代币 [124]
├── loyalty/          # 积分与消费余额 [139]
├── commission/       # 佣金引擎 (8 子模块) [127-138]
│   ├── common/       #   共享类型与 Plugin trait
│   ├── core/         #   核心调度引擎 [127]
│   ├── referral/     #   推荐佣金 [133]
│   ├── multi-level/  #   多层级分销 [138]
│   ├── level-diff/   #   等级差价 [134]
│   ├── single-line/  #   单线制 [135]
│   ├── team/         #   团队业绩 [136]
│   └── pool-reward/  #   奖金池 [137]
├── governance/       # DAO 治理 [125]
├── disclosure/       # 信息披露合规 [130]
├── kyc/              # KYC 验证 [131]
├── market/           # 代币交易市场 [128]
└── tokensale/        # 代币预售 [132]
```

## 核心流程

### 实体生命周期

```
创建实体 → 注入运营资金 → 创建商铺 → 上架商品 → 开始交易
    ↓                                              ↓
配置会员等级/佣金模式/KYC要求               订单完成 → Hook 链触发
    ↓                                              ↓
发行实体代币 → 代币预售/交易           会员升级 ← 佣金分配 ← 积分奖励
    ↓
治理提案 ← 信息披露 ← 合规监管
```

### 订单 Hook 链 (Phase 5.3)

订单确认后依次触发:

1. **OrderMemberHook** — 自动注册买家为会员，更新消费统计
2. **OrderShopStatsHook** — 更新商铺订单计数与营收
3. **OrderCommissionHook** — NEX + Token 双路径佣金分配与结算
4. **OrderLoyaltyHook** — 积分/消费余额奖励
5. **OrderCancelHook** — 取消时反向清理佣金

## Entity-Shop 双层架构

```
Entity（组织层，1:N）                       Shop（业务层）
┌──────────────────────────┐             ┌──────────────────────────┐
│ owner + admins (13 权限位)│   1 : N     │ managers                 │
│ entity_type              │────────────►│ shop_type (7 种)         │
│ governance_mode          │             │ operating_fund (独立)    │
│ token    (统一发行)      │             │ products / orders        │
│ member   (统一体系)      │             │ rating / reviews         │
│ commission (统一返佣)    │             │ points (via loyalty)     │
│ kyc      (统一认证)      │             │ location / policies      │
│ disclosure (统一披露)    │             │ closing grace period     │
│ loyalty  (积分+购物余额)  │             │                          │
└──────────────────────────┘             └──────────────────────────┘
```

## 佣金插件体系

CommissionCore 通过 `CommissionPlugin` trait 调度，NEX + Token 双轨并行:

| 插件 | 说明 | 触发条件 |
|------|------|---------|
| Referral | 直推奖励 (固定/百分比/首单/复购) | 有推荐人 |
| MultiLevel | N 层级上线分配 (3D 激活条件) | 上线链存在 |
| LevelDiff | 等级差价补贴 | 等级高于下级 |
| SingleLine | 单线制排队 (上线/下线) | 注册排队 |
| Team | 团队业绩阶梯奖 | 达到销售门槛 |
| PoolReward | 周期性奖金池均分 | 等级达标领取 |

## 关键 Trait 接口

### 数据提供者

| Trait | 实现方 | 说明 |
|-------|--------|------|
| `EntityProvider` | registry | 实体查询、状态、管理员权限、派生账户 |
| `ShopProvider` | shop | Shop 查询、运营资金、统计更新 |
| `ProductProvider` | product | 商品查询、库存管理 |
| `MemberProvider` | member | 会员状态、等级、推荐链 |
| `AssetLedgerPort` | token | 细粒度资产操作 (reserve/unreserve/repatriate) |
| `LoyaltyReadPort` / `LoyaltyWritePort` | loyalty | NEX 消费余额读写 |
| `LoyaltyTokenReadPort` / `LoyaltyTokenWritePort` | loyalty | Token 消费余额读写 |
| `TokenFeeConfigPort` | token | 代币手续费查询 |
| `DisclosureProvider` | disclosure | 黑窗口期、内幕人员检查 |

### 治理端口 (7 个)

`MarketGovernancePort` · `CommissionGovernancePort` · `SingleLineGovernancePort` · `KycGovernancePort` · `ShopGovernancePort` · `TokenGovernancePort` · `TreasuryPort`

### Hook 通知

| Trait | 触发方 | 说明 |
|-------|--------|------|
| `OnOrderCompleted` | order | 订单完成触发佣金/积分/会员更新 |
| `OnOrderCancelled` | order | 订单取消触发佣金反转 |
| `OnEntityStatusChange` | registry | 实体状态级联通知 |
| `PointsCleanup` | shop | Shop 关闭清理积分 |

## 状态机

**Entity:** `Active ⇄ Suspended ⇄ PendingClose → Closed` / `Active → Banned → Pending`

**Order:** `Paid → Shipped → Completed` / `Paid → Cancelled` / `Shipped → Disputed → Refunded/Completed`

## 子模块速查

| 模块 | Crate | Pallet 索引 | 核心能力 |
|------|-------|------------|----------|
| [common](./common/) | `pallet-entity-common` | — | 类型、38+ Trait、分页、空实现 |
| [registry](./registry/) | `pallet-entity-registry` | 120 | Entity 创建/更新/关闭、运营资金、权限位管理 |
| [shop](./shop/) | `pallet-entity-shop` | 129 | Shop CRUD、运营资金、宽限期关闭、Manager |
| [product](./product/) | `pallet-entity-product` | 121 | 商品 CRUD/上下架、押金托管、变体管理 |
| [order](./order/) | `pallet-entity-order` | 122 | NEX/Token 双资产下单、Hook 驱动副作用 |
| [review](./review/) | `pallet-entity-review` | 123 | 评价提交/修改/删除、商家回复 |
| [member](./member/) | `pallet-entity-member` | 126 | 会员注册、推荐链、等级升级规则引擎 |
| [token](./token/) | `pallet-entity-token` | 124 | 7 种代币类型、铸造/分红/锁仓/转账限制 |
| [loyalty](./loyalty/) | `pallet-entity-loyalty` | 139 | 积分系统 + NEX/Token 双消费余额 |
| [commission](./commission/) | `pallet-commission-*` | 127-138 | 插件化佣金 (7 插件)、NEX/Token 双管线 |
| [governance](./governance/) | `pallet-entity-governance` | 125 | 14 提案域、代币加权投票、资金保护 |
| [disclosure](./disclosure/) | `pallet-entity-disclosure` | 130 | 4 级披露、内幕管控、审批工作流、渐进处罚 |
| [kyc](./kyc/) | `pallet-entity-kyc` | 131 | 5 级 KYC、Provider 管理、风险评分 |
| [market](./market/) | `pallet-entity-market` | 128 | P2P 交易 (5 种订单)、TWAP 预言机、熔断 |
| [tokensale](./tokensale/) | `pallet-entity-tokensale` | 132 | 5 种发售模式、Vesting 解锁、白名单 |

## 测试覆盖

| 模块 | 测试数 | 状态 |
|------|--------|------|
| commission-core | 208 | PASS |
| order | 231 | PASS |
| disclosure | 258 | PASS |
| governance | 226 | PASS |
| market | 151 | PASS |
| token | 148 | PASS |
| shop | 124 | PASS |

## 运行测试

```bash
# 全部 Entity 模块
cargo test -p pallet-entity-common -p pallet-entity-registry -p pallet-entity-shop \
  -p pallet-entity-product -p pallet-entity-order -p pallet-entity-review \
  -p pallet-entity-member -p pallet-entity-token -p pallet-entity-loyalty \
  -p pallet-entity-governance -p pallet-entity-disclosure -p pallet-entity-kyc \
  -p pallet-entity-market -p pallet-entity-tokensale

# 佣金子模块
cargo test -p pallet-commission-core -p pallet-commission-referral \
  -p pallet-commission-multi-level -p pallet-commission-single-line \
  -p pallet-commission-pool-reward -p pallet-commission-level-diff \
  -p pallet-commission-team
```
