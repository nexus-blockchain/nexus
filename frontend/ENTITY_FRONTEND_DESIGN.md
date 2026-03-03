# NEXUS Entity 前端设计方案

> 针对单个 Entity 的完整前端管理界面，基于 `pallets/entity` 15 个子模块的链上能力设计

## 1. 技术栈

| 层级 | 选型 | 说明 |
|------|------|------|
| **框架** | Next.js 14 (App Router) | SSR + CSR 混合，SEO 友好 |
| **UI 库** | shadcn/ui + Radix UI | 无头组件，完全可定制 |
| **样式** | Tailwind CSS 4 | 原子化 CSS |
| **图标** | Lucide React | 轻量一致的图标集 |
| **状态管理** | Zustand + React Query | 链上状态缓存 + 乐观更新 |
| **链交互** | Polkadot.js API (@polkadot/api) | Substrate RPC + Extrinsic 签名 |
| **钱包** | @polkadot/extension-dapp | 浏览器扩展钱包（Polkadot.js / Talisman / SubWallet） |
| **图表** | Recharts | 数据可视化（销售/代币/佣金） |
| **表单** | React Hook Form + Zod | 类型安全表单验证 |
| **IPFS** | ipfs-http-client / Pinata SDK | CID 内容上传/解析 |
| **国际化** | next-intl | 中英文切换 |
| **类型** | TypeScript 5 | 全量类型覆盖 |

## 2. 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                    Next.js App Router                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
│  │  Zustand  │  │  React   │  │ Polkadot │  │   IPFS   │   │
│  │  Store    │  │  Query   │  │  API     │  │  Gateway │   │
│  └─────┬────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘   │
│        │            │              │              │          │
│  ┌─────┴────────────┴──────────────┴──────────────┴─────┐   │
│  │              Chain Adapter Layer (hooks/)             │   │
│  │  useEntity / useShop / useToken / useOrder / ...     │   │
│  └──────────────────────┬───────────────────────────────┘   │
│                         │                                    │
├─────────────────────────┼────────────────────────────────────┤
│              Substrate Node (WebSocket RPC)                  │
│              ws://localhost:9944                             │
└─────────────────────────────────────────────────────────────┘
```

### 2.1 目录结构

```
frontend/
├── public/
│   └── locales/               # i18n 翻译文件
├── src/
│   ├── app/                   # Next.js App Router 页面
│   │   ├── layout.tsx         # 根布局（侧边栏 + 顶栏）
│   │   ├── page.tsx           # 首页 → Entity 仪表盘
│   │   ├── entity/
│   │   │   ├── settings/      # Entity 基本设置
│   │   │   ├── admins/        # 管理员权限管理
│   │   │   └── fund/          # 运营资金管理
│   │   ├── shops/
│   │   │   ├── [shopId]/      # 单个 Shop 详情
│   │   │   │   ├── products/  # 商品管理
│   │   │   │   ├── orders/    # 订单管理
│   │   │   │   ├── reviews/   # 评价管理
│   │   │   │   └── points/    # 积分系统
│   │   │   └── page.tsx       # Shop 列表
│   │   ├── token/
│   │   │   ├── config/        # 代币配置
│   │   │   ├── holders/       # 持有人管理
│   │   │   ├── dividend/      # 分红管理
│   │   │   ├── transfer/      # 转账限制
│   │   │   └── lock/          # 锁仓管理
│   │   ├── market/
│   │   │   ├── orderbook/     # 订单簿（NEX 市场）
│   │   │   ├── usdt/          # USDT OTC 市场
│   │   │   └── settings/      # 市场配置
│   │   ├── members/
│   │   │   ├── list/          # 会员列表
│   │   │   ├── levels/        # 等级管理
│   │   │   ├── rules/         # 升级规则
│   │   │   └── pending/       # 待审批会员
│   │   ├── commission/
│   │   │   ├── config/        # 佣金配置
│   │   │   ├── withdraw/      # 提现管理
│   │   │   └── pool/          # 奖池管理
│   │   ├── governance/
│   │   │   ├── proposals/     # 提案列表
│   │   │   ├── vote/          # 投票
│   │   │   └── config/        # 治理配置
│   │   ├── disclosure/
│   │   │   ├── reports/       # 财务披露
│   │   │   ├── announcements/ # 公告管理
│   │   │   └── insiders/      # 内幕人员管理
│   │   ├── kyc/
│   │   │   ├── records/       # KYC 记录
│   │   │   ├── providers/     # 认证提供者
│   │   │   └── settings/      # KYC 要求配置
│   │   └── tokensale/
│   │       ├── rounds/        # 发售轮次
│   │       ├── [roundId]/     # 单轮详情
│   │       └── create/        # 创建发售
│   ├── components/
│   │   ├── ui/                # shadcn/ui 基础组件
│   │   ├── layout/            # 布局组件（Sidebar, Header, Breadcrumb）
│   │   ├── entity/            # Entity 专用组件
│   │   ├── shop/              # Shop 专用组件
│   │   ├── token/             # Token 专用组件
│   │   ├── market/            # Market 专用组件
│   │   └── shared/            # 通用业务组件（StatusBadge, CidDisplay, TxButton）
│   ├── hooks/
│   │   ├── useApi.ts          # Polkadot API 连接管理
│   │   ├── useWallet.ts       # 钱包连接/签名
│   │   ├── useEntity.ts       # Entity 查询/操作
│   │   ├── useShop.ts         # Shop 查询/操作
│   │   ├── useToken.ts        # Token 查询/操作
│   │   ├── useMarket.ts       # Market 查询/操作
│   │   ├── useMember.ts       # Member 查询/操作
│   │   ├── useCommission.ts   # Commission 查询/操作
│   │   ├── useGovernance.ts   # Governance 查询/操作
│   │   ├── useDisclosure.ts   # Disclosure 查询/操作
│   │   ├── useKyc.ts          # KYC 查询/操作
│   │   ├── useTokensale.ts    # Tokensale 查询/操作
│   │   ├── useOrder.ts        # Order 查询/操作
│   │   ├── useReview.ts       # Review 查询/操作
│   │   └── useTx.ts           # 通用交易提交 + 状态追踪
│   ├── lib/
│   │   ├── api.ts             # Polkadot API 单例
│   │   ├── ipfs.ts            # IPFS 上传/解析工具
│   │   ├── format.ts          # 金额/地址/时间格式化
│   │   ├── types.ts           # 链上类型映射
│   │   └── constants.ts       # 常量定义
│   ├── stores/
│   │   ├── entity.ts          # Entity 全局状态
│   │   ├── wallet.ts          # 钱包状态
│   │   └── ui.ts              # UI 状态（侧边栏折叠、主题等）
│   └── styles/
│       └── globals.css        # Tailwind 全局样式
├── .env.local                 # 环境变量（RPC 端点、IPFS 网关）
├── next.config.js
├── tailwind.config.ts
├── tsconfig.json
└── package.json
```

## 3. 页面设计

### 3.1 全局布局

```
┌─────────────────────────────────────────────────────────────────┐
│ [Logo] NEXUS Entity Manager    [Entity: MyShop ▾] [🔔] [👤 0x...] │
├──────────┬──────────────────────────────────────────────────────┤
│          │                                                      │
│ 📊 仪表盘 │              主内容区域                               │
│          │                                                      │
│ 🏢 Entity │  ┌──────────────────────────────────────────────┐   │
│   设置    │  │                                              │   │
│   管理员  │  │        当前页面内容                            │   │
│   资金    │  │                                              │   │
│          │  │                                              │   │
│ 🏪 Shop  │  │                                              │   │
│   商品    │  │                                              │   │
│   订单    │  │                                              │   │
│   评价    │  │                                              │   │
│          │  └──────────────────────────────────────────────┘   │
│ 🪙 Token  │                                                      │
│ 📈 Market │                                                      │
│ 👥 会员   │                                                      │
│ 💰 佣金   │                                                      │
│ 🗳️ 治理   │                                                      │
│ 📋 披露   │                                                      │
│ 🔐 KYC   │                                                      │
│ 🎯 发售   │                                                      │
├──────────┴──────────────────────────────────────────────────────┤
│ [Chain: Nexus Testnet] [Block: #12,345] [Finalized: #12,340]   │
└─────────────────────────────────────────────────────────────────┘
```

**顶栏功能：**
- Entity 选择器（当前用户拥有/管理的 Entity 切换）
- 通知中心（链上事件监听：资金预警、订单状态、提案投票等）
- 钱包连接/断开，账户余额显示

**侧边栏：**
- 折叠/展开，移动端抽屉模式
- 根据 Entity 类型动态显示菜单（如 Merchant 不显示"治理"，DAO 不显示"商品"）
- 根据用户权限隐藏无权访问的模块（AdminPermission 位掩码）

### 3.2 仪表盘 (Dashboard)

首页全局概览，卡片式布局：

```
┌──────────────────────────────────────────────────────────────┐
│                      Entity 仪表盘                            │
├──────────┬──────────┬──────────┬──────────┬─────────────────┤
│  状态     │  类型     │  资金健康  │  Shop 数  │  会员数        │
│  🟢 Active│ Merchant │  🟢 健康   │   3      │  1,234         │
├──────────┴──────────┴──────────┴──────────┴─────────────────┤
│                                                              │
│  ┌─ 运营资金 ──────────────┐  ┌─ 销售概览 ─────────────────┐ │
│  │ 金库余额: 1,250 NEX     │  │ 今日订单: 23    ↑12%       │ │
│  │ 预警线: 500 NEX         │  │ 今日销售: 450 NEX ↑8%      │ │
│  │ [████████░░] 75%        │  │ 累计订单: 5,678            │ │
│  │ [充值]                  │  │ 累计销售: 123,456 NEX      │ │
│  └─────────────────────────┘  └────────────────────────────┘ │
│                                                              │
│  ┌─ 代币概览 ──────────────┐  ┌─ 近期活动 ─────────────────┐ │
│  │ 通证: MyToken (MYT)    │  │ • 新订单 #5679 (2分钟前)   │ │
│  │ 类型: Points            │  │ • 会员升级 Silver (5分钟前) │ │
│  │ 总供应: 1,000,000      │  │ • 提案 #12 通过 (1小时前)  │ │
│  │ 持有人: 456             │  │ • 佣金结算 50 NEX (3小时前) │ │
│  └─────────────────────────┘  └────────────────────────────┘ │
│                                                              │
│  ┌─ 7日销售趋势 ──────────────────────────────────────────┐  │
│  │  [Recharts 折线图]                                      │  │
│  └─────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

**数据来源映射：**

| 卡片 | 链上存储 / RPC |
|------|---------------|
| 状态/类型 | `EntityRegistry.Entities(entity_id)` → `status`, `entity_type` |
| 资金余额 | `System.Account(treasury_account)` → `free` |
| 资金健康 | 对比 `MinOperatingBalance` 和 `FundWarningThreshold` 常量 |
| Shop 数 | `EntityShop.EntityShops(entity_id)` → `len()` |
| 会员数 | `EntityMember.EntityMemberCount(entity_id)` |
| 销售额/订单 | `EntityRegistry.Entities(entity_id)` → `total_sales`, `total_orders` |
| 代币信息 | `EntityToken.ShopTokenConfigs(entity_id)` |
| 近期活动 | 订阅 `system.events` 过滤 Entity 相关事件 |

### 3.3 Entity 管理

#### 3.3.1 Entity 设置页

| 字段 | 类型 | 来源 | 可编辑 | 对应 Extrinsic |
|------|------|------|--------|---------------|
| 名称 | text | `entity.name` | ✅ | `registry.update_entity` |
| Logo | IPFS CID | `entity.logo_cid` | ✅ | `registry.update_entity` |
| 描述 | IPFS CID | `entity.description_cid` | ✅ | `registry.update_entity` |
| 元数据 URI | IPFS CID | `entity.metadata_uri` | ✅ | `registry.update_entity` |
| 实体类型 | select | `entity.entity_type` | ✅ (升级) | `registry.upgrade_entity_type` |
| 治理模式 | badge | `entity.governance_mode` | ✅ | `governance.configure_governance` |
| 已验证 | badge | `entity.verified` | ❌ (治理) | — |
| 状态 | badge | `entity.status` | ❌ | — |
| 创建时间 | date | `entity.created_at` | ❌ | — |
| 推荐人 | address | `EntityReferrers(entity_owner)` | ✅ (一次) | `registry.bind_entity_referrer` |

**操作按钮：**
- **申请关闭** → `registry.request_close_entity` （仅 Owner）
- **重新开放** → `registry.reopen_entity` （仅 Closed 状态）
- **转移所有权** → `registry.transfer_ownership` （Modal 确认，不可逆）

#### 3.3.2 管理员权限管理

权限位掩码定义（AdminPermission）：

| 位 | 权限 | 说明 |
|----|------|------|
| `0x01` | SHOP_MANAGE | 店铺管理 |
| `0x02` | PRODUCT_MANAGE | 商品管理 |
| `0x04` | ORDER_MANAGE | 订单管理 |
| `0x08` | MEMBER_MANAGE | 会员管理 |
| `0x10` | TOKEN_MANAGE | 代币管理 |
| `0x20` | GOVERNANCE_MANAGE | 治理管理 |
| `0x40` | FINANCE_MANAGE | 资金管理 |
| `0x80` | DISCLOSURE_MANAGE | 披露管理 |

**UI：** 表格 + 复选框矩阵

```
┌─ 管理员列表 ─────────────────────────────────────────────────┐
│                                                              │
│  地址          │ 店铺 │ 商品 │ 订单 │ 会员 │ 代币 │ 治理 │ 操作 │
│  0xAbc...123  │  ☑  │  ☑  │  ☑  │  ☐  │  ☐  │  ☐  │ [编辑][删除] │
│  0xDef...456  │  ☑  │  ☑  │  ☑  │  ☑  │  ☑  │  ☑  │ [编辑][删除] │
│                                                              │
│  [+ 添加管理员]                                               │
└──────────────────────────────────────────────────────────────┘
```

**Extrinsics：**
- `registry.add_admin(entity_id, admin, permissions)`
- `registry.remove_admin(entity_id, admin)`
- `registry.update_admin_permissions(entity_id, admin, new_permissions)`

#### 3.3.3 运营资金管理

```
┌─ 运营资金 ─────────────────────────────────────────────────────┐
│                                                                │
│  金库账户: 5EYCAe5hvej...（派生账户）          [复制]            │
│                                                                │
│  ┌──────────────────────────────────┐                          │
│  │ 当前余额      1,250.00 NEX      │                          │
│  │ 最低余额       100.00 NEX      │                          │
│  │ 预警阈值       500.00 NEX      │                          │
│  │ 健康状态      🟢 Healthy        │                          │
│  │ [████████████░░░] 75%           │                          │
│  └──────────────────────────────────┘                          │
│                                                                │
│  充值金额: [________] NEX   [充值]                              │
│                                                                │
│  ┌─ 费用记录 ──────────────────────────────────────────────┐   │
│  │ 时间        │ 类型      │ 金额     │ 余额              │   │
│  │ 2026-03-03 │ IpfsPin   │ -5 NEX  │ 1,250 NEX         │   │
│  │ 2026-03-02 │ Storage   │ -2 NEX  │ 1,255 NEX         │   │
│  │ 2026-03-01 │ 充值      │ +500 NEX│ 1,257 NEX         │   │
│  └─────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────┘
```

**Extrinsic：** `registry.top_up_fund(entity_id, amount)`

### 3.4 Shop 管理

#### 3.4.1 Shop 列表

```
┌─ 我的店铺 ──────────────────────────────────── [+ 创建店铺] ─┐
│                                                              │
│  ┌─ 主店铺 ★ ────────────────┐  ┌─ 分店 A ────────────────┐ │
│  │ MyShop Prime              │  │ Branch Store A           │ │
│  │ 🟢 Active                 │  │ 🟡 Paused               │ │
│  │ 商品: 45  订单: 1,234     │  │ 商品: 12  订单: 456     │ │
│  │ 评分: ⭐ 4.7 (892评价)    │  │ 评分: ⭐ 4.2 (231评价)  │ │
│  │ 资金: 800 NEX             │  │ 资金: 200 NEX           │ │
│  │ [管理] [暂停]             │  │ [管理] [恢复] [关闭]    │ │
│  └───────────────────────────┘  └──────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

**Shop 操作对应 Extrinsics：**

| 操作 | Extrinsic |
|------|-----------|
| 创建店铺 | `shop.create_shop(entity_id, name, shop_type, initial_fund)` |
| 更新信息 | `shop.update_shop(shop_id, name, logo_cid, ...)` |
| 暂停/恢复 | `shop.pause_shop(shop_id)` / `shop.resume_shop(shop_id)` |
| 关闭 | `shop.close_shop(shop_id)` |
| 充值资金 | `shop.fund_operating(shop_id, amount)` |
| 提取资金 | `shop.withdraw_operating_fund(shop_id, amount)` |
| 设置位置 | `shop.set_location(shop_id, latitude, longitude, address_cid, hours_cid)` |
| 添加/移除管理员 | `shop.add_manager(shop_id, manager)` / `shop.remove_manager(...)` |

#### 3.4.2 商品管理 (Service)

**列表视图：** 表格 + 卡片切换，支持按状态过滤（Draft/OnSale/OffShelf/SoldOut）

**商品创建表单：**

| 字段 | 类型 | 验证 |
|------|------|------|
| 名称 CID | IPFS 上传 | 非空 |
| 图片 CID | IPFS 上传 | 非空 |
| 详情 CID | IPFS 上传 | 非空 |
| NEX 价格 | number | > 0 |
| USDT 价格 | number | ≥ 0 (0=不支持 USDT) |
| 库存 | number | 0=无限 |
| 是否数字商品 | toggle | — |

**Extrinsics：**

| 操作 | Extrinsic |
|------|-----------|
| 创建 | `service.create_product(shop_id, name_cid, images_cid, detail_cid, price, usdt_price, stock, is_digital)` |
| 更新 | `service.update_product(product_id, ...)` |
| 上架 | `service.publish_product(product_id)` |
| 下架 | `service.unpublish_product(product_id)` |
| 删除 | `service.delete_product(product_id)` |

#### 3.4.3 订单管理 (Order)

**订单看板视图（Kanban）：**

```
  待付款      │  待发货      │  已发货      │  已完成      │  退款中
 ┌────────┐  │ ┌────────┐  │ ┌────────┐  │ ┌────────┐  │ ┌────────┐
 │ #5680  │  │ │ #5678  │  │ │ #5675  │  │ │ #5670  │  │ │ #5672  │
 │ 50 NEX │  │ │ 30 NEX │  │ │ 80 NEX │  │ │ 25 NEX │  │ │ 15 NEX │
 │ 买家:.. │  │ │ [发货] │  │ │ 物流:..│  │ │ ⭐ 4.5  │  │ │ [同意] │
 └────────┘  │ └────────┘  │ └────────┘  │ └────────┘  │ │ [拒绝] │
             │             │             │             │ └────────┘
```

**卖家操作 Extrinsics：**

| 操作 | Extrinsic | 条件 |
|------|-----------|------|
| 发货 | `order.ship_order(order_id, tracking_cid)` | 已付款 |
| 开始服务 | `order.start_service(order_id)` | 服务类订单 |
| 完成服务 | `order.complete_service(order_id)` | 服务进行中 |
| 同意退款 | `order.approve_refund(order_id)` | 退款申请中 |

**买家操作 Extrinsics：**

| 操作 | Extrinsic | 条件 |
|------|-----------|------|
| 下单 | `order.place_order(shop_id, product_id, quantity, ...)` | — |
| 取消 | `order.cancel_order(order_id)` | 待发货，非数字商品 |
| 确认收货 | `order.confirm_receipt(order_id)` | 已发货 |
| 确认服务 | `order.confirm_service(order_id)` | 服务已完成 |
| 申请退款 | `order.request_refund(order_id, reason_cid)` | 已收货 |

#### 3.4.4 评价管理 (Review)

```
┌─ 评价概览 ──────────────────────────────────────────────────┐
│  总评分: ⭐ 4.7    评价数: 892    [开启评价 ✅]              │
├─────────────────────────────────────────────────────────────┤
│  ⭐⭐⭐⭐⭐  ████████████████  68%                           │
│  ⭐⭐⭐⭐    ████████          22%                           │
│  ⭐⭐⭐      ███               7%                            │
│  ⭐⭐        █                 2%                            │
│  ⭐          ░                 1%                            │
├─────────────────────────────────────────────────────────────┤
│  评价列表 (按时间倒序)                                       │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ 0xAbc... │ ⭐⭐⭐⭐⭐ │ 订单 #5670 │ "非常满意..."     │ │
│  │ 0xDef... │ ⭐⭐⭐⭐   │ 订单 #5665 │ "还不错..."       │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

**Extrinsics：**
- `review.submit_review(order_id, rating, content_cid)`
- `review.set_review_enabled(entity_id, enabled)` （管理员）

#### 3.4.5 积分系统 (Shop Points)

**配置面板：**

| 字段 | 说明 | Extrinsic |
|------|------|-----------|
| 积分名称 | 如 "优惠积分" | `shop.enable_points` |
| 积分符号 | 如 "PTS" | `shop.enable_points` |
| 返积分比例 | 基点，500=5% | `shop.update_points_config` |
| 兑换比例 | 基点，1000=10% | `shop.update_points_config` |
| 可转让 | toggle | `shop.update_points_config` |
| 禁用积分 | — | `shop.disable_points` |

### 3.5 代币管理 (Token)

#### 3.5.1 代币配置

```
┌─ 代币信息 ──────────────────────────────────────────────────┐
│                                                              │
│  名称: MyToken        符号: MYT        精度: 12             │
│  类型: Points ▾       最大供应: 1,000,000                   │
│  转账限制: None ▾     可转让: ✅                             │
│                                                              │
│  ┌─ 统计 ──────────┐  ┌─ 分红配置 ─────────────────────┐   │
│  │ 总供应: 500,000 │  │ 是否启用: ✅                    │   │
│  │ 持有人: 456     │  │ 分红间隔: 7 天                  │   │
│  │ 最大供应: 1M    │  │ 最低分红额: 100 NEX             │   │
│  └─────────────────┘  │ [配置分红] [发放分红]           │   │
│                       └─────────────────────────────────┘   │
│  [铸造代币] [更新配置] [变更类型] [设置最大供应]            │
└──────────────────────────────────────────────────────────────┘
```

**代币 Extrinsics 映射：**

| 操作 | Extrinsic |
|------|-----------|
| 创建代币 | `token.create_shop_token(entity_id, name, symbol, decimals, reward_rate, exchange_rate)` |
| 更新配置 | `token.update_token_config(entity_id, reward_rate, exchange_rate, ...)` |
| 铸造 | `token.mint_tokens(entity_id, to, amount)` |
| 转让 | `token.transfer_tokens(entity_id, to, amount)` |
| 配置分红 | `token.configure_dividend(entity_id, ...)` |
| 发放分红 | `token.distribute_dividend(entity_id, amount, recipients)` |
| 领取分红 | `token.claim_dividend(entity_id)` |
| 锁仓 | `token.lock_tokens(entity_id, user, amount, unlock_at)` |
| 解锁 | `token.unlock_tokens(entity_id)` |
| 变更类型 | `token.change_token_type(entity_id, new_type)` |
| 设置最大供应 | `token.set_max_supply(entity_id, max_supply)` |
| 设置转账限制 | `token.set_transfer_restriction(entity_id, mode)` |
| 白名单管理 | `token.add_to_whitelist` / `remove_from_whitelist` |
| 黑名单管理 | `token.add_to_blacklist` / `remove_from_blacklist` |

#### 3.5.2 转账限制管理

根据 `TransferRestrictionMode` 动态渲染：

| 模式 | UI 组件 |
|------|---------|
| None | 无额外 UI |
| Whitelist | 白名单地址列表 + 添加/移除 |
| Blacklist | 黑名单地址列表 + 添加/移除 |
| KycRequired | 显示最低 KYC 级别选择器 |
| MembersOnly | 仅提示信息 |

### 3.6 代币市场 (Market)

#### 3.6.1 NEX 订单簿

```
┌─ NEX/MYT 交易市场 ─────────────────────────────────────────┐
│                                                              │
│  最新价: 0.5 NEX  │  24h 量: 12,500  │  TWAP: 0.48 NEX    │
│                                                              │
│  ┌─ 卖单 (Ask) ──────┐  ┌─ 买单 (Bid) ──────┐             │
│  │ 0.55  │  500     │  │ 0.48  │  800     │             │
│  │ 0.53  │  1,200   │  │ 0.47  │  1,500   │             │
│  │ 0.52  │  300     │  │ 0.45  │  2,000   │             │
│  │ 0.51  │  2,500   │  │ 0.44  │  500     │             │
│  └────────────────────┘  └────────────────────┘             │
│                                                              │
│  ┌─ 下单 ─────────────────────────────────────────────┐     │
│  │ [限价买入] [限价卖出] [市价买入] [市价卖出]        │     │
│  │                                                     │     │
│  │ 价格: [______] NEX   数量: [______] MYT            │     │
│  │ 总额: 0 NEX          滑点保护: [5]%                 │     │
│  │                                                     │     │
│  │ [确认下单]                                          │     │
│  └─────────────────────────────────────────────────────┘     │
│                                                              │
│  ┌─ 我的挂单 ──────────────────────────────────────────┐    │
│  │ #123 │ 卖出 │ 100 MYT @ 0.55 │ Open │ [取消]      │    │
│  │ #120 │ 买入 │ 200 MYT @ 0.45 │ Partial │ [取消]   │    │
│  └──────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────┘
```

**市场 Extrinsics：**

| 操作 | Extrinsic |
|------|-----------|
| 限价卖出 | `market.place_sell_order(entity_id, amount, price)` |
| 限价买入 | `market.place_buy_order(entity_id, amount, price)` |
| 吃单 | `market.take_order(order_id, amount)` |
| 市价买入 | `market.market_buy(entity_id, amount, max_cost)` |
| 市价卖出 | `market.market_sell(entity_id, amount, min_receive)` |
| 取消订单 | `market.cancel_order(order_id)` |
| 配置市场 | `market.configure_market(entity_id, fee_rate, order_ttl, ...)` |
| 价格保护 | `market.configure_price_protection(entity_id, ...)` |
| 设初始价 | `market.set_initial_price(entity_id, price)` |
| 解除熔断 | `market.lift_circuit_breaker(entity_id)` |

#### 3.6.2 USDT OTC 市场

独立页签，支持 USDT/TRON 链下支付流程：

```
下单 → 锁定Token → 支付USDT(链下) → 提交txHash → OCW验证 → 完成
```

**USDT 市场 Extrinsics：**
- `market.place_usdt_sell_order` / `place_usdt_buy_order`
- `market.reserve_usdt_sell_order` / `accept_usdt_buy_order`
- `market.confirm_usdt_payment`
- `market.process_usdt_timeout`
- `market.claim_verification_reward`

### 3.7 会员管理 (Member)

#### 3.7.1 会员列表

```
┌─ 会员管理 ──────────────────────── 总数: 1,234 ─── [策略设置] ─┐
│                                                                 │
│  搜索: [________________]  等级: [全部 ▾]  状态: [全部 ▾]       │
│                                                                 │
│  地址          │ 等级     │ 消费(USDT) │ 推荐人     │ 注册时间   │
│  0xAbc...123  │ 🥇 Gold  │ $2,500    │ 0xDef..   │ 2026-01   │
│  0xDef...456  │ 🥈 Silver│ $1,200    │ 0xAbc..   │ 2026-02   │
│  0xGhi...789  │ Normal   │ $300      │ —         │ 2026-03   │
│                                                                 │
│  [手动升级] [导出]                                               │
└─────────────────────────────────────────────────────────────────┘
```

#### 3.7.2 等级管理

| 操作 | Extrinsic |
|------|-----------|
| 初始化等级系统 | `member.init_level_system(shop_id, use_custom, upgrade_mode)` |
| 添加自定义等级 | `member.add_custom_level(shop_id, name, threshold, discount_rate, commission_bonus)` |
| 更新等级 | `member.update_custom_level(shop_id, level_id, ...)` |
| 删除等级 | `member.remove_custom_level(shop_id, level_id)` |
| 手动升级 | `member.manual_upgrade_member(shop_id, member, target_level_id)` |
| 设置升级模式 | `member.set_upgrade_mode(shop_id, mode)` |

#### 3.7.3 升级规则引擎

```
┌─ 升级规则 ────────────────────────────── [+ 添加规则] ─────┐
│                                                             │
│  冲突策略: [优先级最高 ▾]    系统状态: [✅ 已启用]           │
│                                                             │
│  ┌─ 规则 #1: 消费升级 ──────────────────── 优先级: 1 ─┐   │
│  │ 触发: PurchaseProduct                               │   │
│  │ 目标等级: Gold    阈值: $2,000                      │   │
│  │ 已触发: 23/100次  可叠加: ❌  状态: ✅ 启用          │   │
│  │ [编辑] [禁用] [删除]                                │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─ 规则 #2: 推荐升级 ──────────────────── 优先级: 2 ─┐   │
│  │ 触发: ReferralCount                                 │   │
│  │ 目标等级: Silver  阈值: 10人                        │   │
│  │ 已触发: 5/∞次   可叠加: ✅  状态: ✅ 启用           │   │
│  │ [编辑] [禁用] [删除]                                │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

**规则触发类型 (UpgradeTrigger)：**

| 触发类型 | 说明 | 路径 |
|----------|------|------|
| `PurchaseProduct` | 购买指定商品 | 订单路径 |
| `SingleOrder` | 单笔消费满额 | 订单路径 |
| `TotalSpent` | 累计消费满额 | 订单路径 |
| `OrderCount` | 订单数达标 | 订单路径 |
| `TotalSpentUsdt` | USDT 累计消费达标 | 订单路径 |
| `SingleOrderUsdt` | USDT 单笔消费达标 | 订单路径 |
| `ReferralCount` | 直推人数达标 | 推荐路径 |
| `TeamSize` | 团队总人数达标 | 推荐路径 |
| `ReferralLevelCount` | 直推中达指定等级人数 | 推荐路径 |

#### 3.7.4 会员注册策略

| 策略位 | 说明 |
|--------|------|
| `0` | 开放注册 |
| `1` | 需要购买（下单即注册） |
| `2` | 需要推荐人 |
| `4` | 需要审批 |

**Extrinsics：**
- `member.set_member_policy(shop_id, policy_bits)`
- `member.approve_member(shop_id, account)` / `reject_member(shop_id, account)`
- `member.cleanup_expired_pending(entity_id)`

### 3.8 佣金管理 (Commission)

佣金系统由 7 个子模块组成，前端按功能整合：

#### 3.8.1 佣金配置

**佣金模式：**

| 模式 | 子模块 | 说明 |
|------|--------|------|
| 直推佣金 | `commission-referral` | 直接推荐人获得 |
| 多级佣金 | `commission-multi-level` | 多层级瀑布分配 |
| 级差佣金 | `commission-level-diff` | 按等级差分配 |
| 排线佣金 | `commission-single-line` | 单线排列分配 |
| 团队佣金 | `commission-team` | 按团队业绩分配 |
| 奖池佣金 | `commission-pool-reward` | 沉淀池+轮次分配 |

**配置页面：** 由 `commission-core` 统一管理，配置后各子模块按 `CommissionModes` 标志位决定启用

#### 3.8.2 提现管理

```
┌─ 佣金提现 ──────────────────────────────────────────────────┐
│                                                              │
│  NEX 可提现: 1,250 NEX    Token 可提现: 5,000 MYT           │
│  购物余额: 300 NEX        待结算: 50 NEX                    │
│                                                              │
│  [提现 NEX]  [提现 Token]  [使用购物余额下单]               │
│                                                              │
│  ┌─ 提现记录 ──────────────────────────────────────────┐    │
│  │ 时间       │ 类型  │ 金额      │ 手续费   │ 状态    │    │
│  │ 2026-03-03│ NEX  │ 500 NEX  │ 25 NEX  │ ✅ 完成  │    │
│  │ 2026-03-01│ Token│ 1000 MYT │ 0       │ ✅ 完成  │    │
│  └──────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────┘
```

### 3.9 DAO 治理 (Governance)

#### 3.9.1 提案列表

```
┌─ 治理提案 ──────────── 模式: FullDAO ──── [+ 创建提案] ────┐
│                                                              │
│  状态过滤: [全部] [投票中] [待执行] [已通过] [已失败]       │
│                                                              │
│  ┌─ #12 修改佣金比例 ──────────────────── 投票中 ──────┐   │
│  │ 类型: UpdateCommission                              │   │
│  │ 提案人: 0xAbc...     创建: 2026-03-01              │   │
│  │ 截止: 2026-03-08     法定人数: 30%                  │   │
│  │                                                     │   │
│  │ 赞成: ████████░░ 72%    反对: ██░░░░░░░░ 18%       │   │
│  │ 弃权: █░░░░░░░░░ 10%                               │   │
│  │                                                     │   │
│  │ [👍 赞成] [👎 反对] [🤷 弃权]                       │   │
│  └─────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

**提案类型 (ProposalType)：** 约 41 种，按分类展示

**Governance Extrinsics：**

| 操作 | Extrinsic |
|------|-----------|
| 创建提案 | `governance.create_proposal(entity_id, proposal_type, title, description_cid)` |
| 投票 | `governance.vote(proposal_id, vote_type)` |
| 结束投票 | `governance.finalize_voting(proposal_id)` |
| 执行提案 | `governance.execute_proposal(proposal_id)` |
| 取消提案 | `governance.cancel_proposal(proposal_id)` |
| 否决提案 | `governance.veto_proposal(proposal_id)` |
| 配置治理 | `governance.configure_governance(entity_id, ...)` |
| 锁定治理 | `governance.lock_governance(entity_id)` |
| 清理提案 | `governance.cleanup_proposal(proposal_id)` |

### 3.10 财务披露与公告 (Disclosure)

#### 3.10.1 财务披露

```
┌─ 财务披露 ──────────────────── [发布披露] [配置] ──────────┐
│                                                             │
│  黑窗口期: 🔴 生效中 (至 Block #15,000)                    │
│  内幕人员: 3 人                                             │
│                                                             │
│  ┌─ #5 2026-Q1 财务报告 ──────────── Active ──────────┐   │
│  │ 类型: QuarterlyReport                               │   │
│  │ 发布: 2026-03-01    披露窗口: 至 2026-03-31        │   │
│  │ 重要程度: Material                                  │   │
│  │ [查看] [更正] [撤回]                                │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

#### 3.10.2 公告管理

| 操作 | Extrinsic |
|------|-----------|
| 发布公告 | `disclosure.publish_announcement(entity_id, title, content_cid, category, expires_at)` |
| 更新公告 | `disclosure.update_announcement(entity_id, id, ...)` |
| 撤回公告 | `disclosure.withdraw_announcement(entity_id, id)` |
| 置顶公告 | `disclosure.pin_announcement(entity_id, announcement_id)` |
| 过期公告 | `disclosure.expire_announcement(entity_id, id)` |
| 清理历史 | `disclosure.cleanup_announcement_history(entity_id)` |

#### 3.10.3 内幕人员与黑窗口

| 操作 | Extrinsic |
|------|-----------|
| 添加内幕人员 | `disclosure.add_insider(entity_id, account, role)` |
| 移除内幕人员 | `disclosure.remove_insider(entity_id, account)` |
| 开始黑窗口 | `disclosure.start_blackout(entity_id, end_block)` |
| 结束黑窗口 | `disclosure.end_blackout(entity_id)` |

### 3.11 KYC 管理

```
┌─ KYC 设置 ──────────────────────────────────────────────────┐
│                                                              │
│  实体 KYC 要求:                                              │
│  最低级别: [Standard ▾]    最大风险分: [70 ___]              │
│  [保存设置]                                                  │
│                                                              │
│  高风险国家: KP, IR, SY  [编辑列表]                         │
│                                                              │
│  ┌─ 认证提供者 ──────────────────────────── [+ 注册] ─┐    │
│  │ 0xProv1... │ 最高级别: Enhanced │ 验证数: 156     │    │
│  │ 0xProv2... │ 最高级别: Standard │ 验证数: 89      │    │
│  └────────────────────────────────────────────────────┘    │
│                                                              │
│  ┌─ KYC 记录 ──────────────────────────────────────────┐   │
│  │ 账户       │ 级别     │ 状态     │ 过期时间          │   │
│  │ 0xAbc...  │ Standard│ ✅ 通过  │ 2027-03-01        │   │
│  │ 0xDef...  │ Basic   │ ⏳ 待审  │ —                 │   │
│  │ 0xGhi...  │ Enhanced│ ❌ 拒绝  │ —                 │   │
│  └──────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

**KYC Extrinsics：**

| 操作 | Extrinsic | 角色 |
|------|-----------|------|
| 提交 KYC | `kyc.submit_kyc(level, country_code, data_cid)` | 用户 |
| 批准 | `kyc.approve_kyc(account, level, risk_score, expires_at)` | Provider |
| 拒绝 | `kyc.reject_kyc(account, reason)` | Provider |
| 撤销 | `kyc.revoke_kyc(account, reason)` | Admin |
| 过期标记 | `kyc.expire_kyc(account)` | 任何人 |
| 注册 Provider | `kyc.register_provider(provider, max_level)` | Root |
| 设置要求 | `kyc.set_entity_requirement(entity_id, min_level, max_risk_score)` | Admin |
| 高风险国家 | `kyc.update_high_risk_countries(countries)` | Root |

### 3.12 代币发售 (Tokensale)

```
┌─ 代币发售 ──────────────────────────────── [创建发售] ─────┐
│                                                              │
│  ┌─ Round #3: Series A ─────── 进行中 ────────────────┐    │
│  │                                                     │    │
│  │ 模式: FixedPrice    价格: 0.5 NEX/MYT              │    │
│  │ 总量: 100,000 MYT   已售: 67,500 MYT               │    │
│  │ 开始: Block #10,000  结束: Block #20,000            │    │
│  │                                                     │    │
│  │ [████████████████████░░░░░░░░] 67.5%               │    │
│  │                                                     │    │
│  │ 支付方式: NEX ✅  USDT ✅                           │    │
│  │ Vesting: 20% 立即 + 80% 线性 180天                  │    │
│  │ 白名单: ✅ 已启用 (56 地址)                         │    │
│  │                                                     │    │
│  │ [认购] [添加白名单] [结束发售] [取消发售]           │    │
│  └─────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────┘
```

**Tokensale Extrinsics：**

| 操作 | Extrinsic |
|------|-----------|
| 创建轮次 | `tokensale.create_sale_round(entity_id, mode, total_amount, price, start_block, end_block, ...)` |
| 添加支付选项 | `tokensale.add_payment_option(round_id, asset_type, price)` |
| 设置 Vesting | `tokensale.set_vesting_config(round_id, initial_unlock_pct, vesting_blocks)` |
| 配置荷兰拍 | `tokensale.configure_dutch_auction(round_id, start_price, end_price, decay_rate)` |
| 添加白名单 | `tokensale.add_to_whitelist(round_id, accounts)` |
| 开始发售 | `tokensale.start_sale(round_id)` |
| 认购 | `tokensale.subscribe(round_id, amount, payment_asset)` |
| 结束发售 | `tokensale.end_sale(round_id)` |
| 领取代币 | `tokensale.claim_tokens(round_id)` |
| 解锁代币 | `tokensale.unlock_tokens(round_id)` |
| 取消发售 | `tokensale.cancel_sale(round_id)` |
| 领取退款 | `tokensale.claim_refund(round_id)` |
| 提取资金 | `tokensale.withdraw_funds(round_id)` |
| 回收未领 | `tokensale.reclaim_unclaimed_tokens(round_id)` |

## 4. 链上交互层设计 (Hooks)

### 4.1 核心 Hook: `useApi`

```typescript
// hooks/useApi.ts
import { ApiPromise, WsProvider } from '@polkadot/api';

interface ApiState {
  api: ApiPromise | null;
  isConnected: boolean;
  chainInfo: { name: string; bestBlock: number; finalizedBlock: number };
}

export function useApi(): ApiState;
```

### 4.2 交易提交 Hook: `useTx`

```typescript
// hooks/useTx.ts
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

### 4.3 Entity 查询 Hook 示例: `useEntity`

```typescript
// hooks/useEntity.ts
interface EntityData {
  id: number;
  owner: string;
  name: string;
  logoCid: string | null;
  descriptionCid: string | null;
  status: 'Pending' | 'Active' | 'Suspended' | 'Banned' | 'Closed';
  entityType: EntityType;
  governanceMode: 'None' | 'FullDAO';
  verified: boolean;
  admins: Array<{ address: string; permissions: number }>;
  primaryShopId: number;
  totalSales: bigint;
  totalOrders: number;
  fundBalance: bigint;
  fundHealth: 'Healthy' | 'Warning' | 'Critical' | 'Depleted';
}

export function useEntity(entityId: number): {
  data: EntityData | null;
  isLoading: boolean;
  error: Error | null;
  refetch: () => void;
};

export function useEntityActions(entityId: number): {
  updateEntity: (params: UpdateEntityParams) => Promise<void>;
  topUpFund: (amount: bigint) => Promise<void>;
  addAdmin: (admin: string, permissions: number) => Promise<void>;
  removeAdmin: (admin: string) => Promise<void>;
  transferOwnership: (newOwner: string) => Promise<void>;
  requestClose: () => Promise<void>;
  reopenEntity: () => Promise<void>;
  upgradeType: (newType: EntityType, newGovernance?: GovernanceMode) => Promise<void>;
  bindReferrer: (referrer: string) => Promise<void>;
};
```

### 4.4 事件订阅 Hook: `useEntityEvents`

```typescript
// hooks/useEntityEvents.ts
export function useEntityEvents(entityId: number): {
  events: EntityEvent[];
  subscribe: () => () => void;  // 返回 unsubscribe
};
```

监听事件列表（各 pallet 关键事件）：

| Pallet | 事件 |
|--------|------|
| Registry | `EntityCreated`, `EntitySuspendedLowFund`, `FundWarning`, `OperatingFeeDeducted` |
| Shop | `ShopCreated`, `ShopClosed`, `FundDeposited` |
| Token | `TokenCreated`, `TokensMinted`, `DividendDistributed` |
| Order | `OrderPlaced`, `OrderShipped`, `OrderCompleted`, `RefundRequested` |
| Market | `OrderPlaced`, `OrderFilled`, `CircuitBreakerTriggered` |
| Member | `MemberRegistered`, `MemberUpgraded`, `MemberLevelExpired` |
| Governance | `ProposalCreated`, `VoteCast`, `ProposalExecuted` |
| Disclosure | `DisclosurePublished`, `AnnouncementPublished`, `BlackoutStarted` |
| KYC | `KycApproved`, `KycRejected`, `KycExpired` |
| Tokensale | `SaleStarted`, `Subscribed`, `SaleEnded` |

## 5. 状态管理

### 5.1 Zustand Store

```typescript
// stores/entity.ts
interface EntityStore {
  // 当前选中的 Entity
  currentEntityId: number | null;
  setCurrentEntityId: (id: number) => void;

  // 用户拥有/管理的 Entity 列表
  userEntities: EntitySummary[];
  loadUserEntities: (account: string) => Promise<void>;

  // 权限缓存
  permissions: Record<number, number>;  // entityId → permission bits
  hasPermission: (entityId: number, required: number) => boolean;
}
```

### 5.2 React Query 缓存策略

| 查询 | staleTime | 缓存策略 |
|------|-----------|---------|
| Entity 基本信息 | 30s | `['entity', entityId]` |
| Shop 列表 | 30s | `['shops', entityId]` |
| 商品列表 | 15s | `['products', shopId]` |
| 订单列表 | 10s | `['orders', shopId, status]` |
| 代币信息 | 60s | `['token', entityId]` |
| 市场订单簿 | 5s | `['orderbook', entityId]` |
| 会员列表 | 30s | `['members', entityId, page]` |
| 提案列表 | 30s | `['proposals', entityId]` |
| KYC 记录 | 60s | `['kyc', entityId]` |

**乐观更新：** 对于 `update_entity`、`update_product` 等修改操作，在 Extrinsic 提交后立即更新本地缓存，Finalized 后刷新确认。

## 6. IPFS 集成

### 6.1 内容上传流程

```
用户选择文件 → 前端压缩/预览 → 上传到 IPFS (Pinata/Infura)
                                      ↓
                              获取 CID (bafk...)
                                      ↓
                              CID 写入链上 Extrinsic
```

### 6.2 CID 解析组件

```typescript
// components/shared/CidDisplay.tsx
// 自动检测 CID 类型（图片/JSON/文本），渲染对应组件
// 使用 IPFS Gateway 获取内容: https://gateway.pinata.cloud/ipfs/{cid}
```

## 7. 权限控制

### 7.1 路由级权限

```typescript
// middleware.ts (Next.js)
// 1. 检查钱包是否连接
// 2. 检查当前账户是否为 Entity owner/admin
// 3. 根据 AdminPermission 位掩码控制页面访问
```

### 7.2 组件级权限

```typescript
// components/shared/RequirePermission.tsx
interface Props {
  entityId: number;
  permission: number;  // AdminPermission 位掩码
  children: React.ReactNode;
  fallback?: React.ReactNode;
}

// 用法:
// <RequirePermission entityId={1} permission={PRODUCT_MANAGE}>
//   <CreateProductButton />
// </RequirePermission>
```

### 7.3 根据 EntityType 动态菜单

| EntityType | 显示模块 | 隐藏模块 |
|------------|---------|---------|
| Merchant | 全部 | — |
| Enterprise | 全部 | — |
| DAO | 治理(主导), 代币, 市场, 会员, 披露 | 商品, 订单 (可选) |
| Community | 会员, 代币, 公告 | 商品, 订单, KYC (可选) |
| Project | 代币, 发售, 治理, 披露, KYC | — |
| Fund | 代币, 治理, 市场, 披露, KYC | 商品, 订单 |

## 8. 响应式设计

| 断点 | 布局 |
|------|------|
| `< 640px` (Mobile) | 底部 Tab 导航，全屏内容 |
| `640-1024px` (Tablet) | 侧边栏抽屉，可收起 |
| `> 1024px` (Desktop) | 固定侧边栏 + 主内容 |

## 9. 性能优化

- **链上查询批量化：** 使用 `api.queryMulti()` 批量查询，减少 WebSocket 往返
- **订阅优化：** 仪表盘使用 `api.query.*.entries()` 一次性加载，配合事件增量更新
- **分页：** 会员列表、订单列表使用 cursor-based 分页（链上 StorageMap iteration）
- **图片懒加载：** IPFS 内容使用 `IntersectionObserver` 懒加载
- **代码分割：** 按路由动态 `import()`，市场图表按需加载
- **SSR 限制：** 链上数据全部 CSR，仅布局/静态内容 SSR

## 10. 环境变量

```env
# .env.local
NEXT_PUBLIC_WS_ENDPOINT=ws://localhost:9944
NEXT_PUBLIC_IPFS_GATEWAY=https://gateway.pinata.cloud/ipfs
NEXT_PUBLIC_PINATA_API_KEY=your_api_key
NEXT_PUBLIC_PINATA_SECRET=your_secret
NEXT_PUBLIC_CHAIN_NAME=Nexus Testnet
```

## 11. 开发计划

### Phase 1: 基础框架 (1-2 周)
- [ ] Next.js 项目初始化 + Tailwind + shadcn/ui
- [ ] Polkadot API 连接 + 钱包集成
- [ ] 全局布局（侧边栏 + 顶栏 + 面包屑）
- [ ] Entity 选择器 + 基本状态管理
- [ ] IPFS 上传/解析工具
- [ ] 通用组件：TxButton, StatusBadge, CidDisplay, AddressDisplay

### Phase 2: 核心管理 (2-3 周)
- [ ] 仪表盘 + Entity 设置/管理员/资金
- [ ] Shop 管理（创建/编辑/暂停/关闭）
- [ ] 商品管理（CRUD + 状态管理）
- [ ] 订单管理（看板视图 + 操作流程）
- [ ] 评价管理

### Phase 3: 通证经济 (2-3 周)
- [ ] 代币配置 + 持有人管理
- [ ] 分红管理 + 锁仓管理
- [ ] 转账限制（白名单/黑名单）
- [ ] NEX 订单簿市场
- [ ] USDT OTC 市场

### Phase 4: 会员与佣金 (2 周)
- [ ] 会员列表 + 等级管理
- [ ] 升级规则引擎配置
- [ ] 佣金配置 + 提现管理
- [ ] 积分系统

### Phase 5: 治理与合规 (2 周)
- [ ] DAO 治理提案 + 投票
- [ ] 财务披露 + 公告管理
- [ ] KYC 管理
- [ ] 代币发售 + Vesting

### Phase 6: 打磨 (1-2 周)
- [ ] 通知中心（链上事件实时推送）
- [ ] 国际化 (中/英)
- [ ] 响应式适配
- [ ] 性能优化 + E2E 测试
- [ ] 部署配置

**预计总工期：10-14 周**

## 12. Pallet-Extrinsic 完整映射表

| Pallet | 模块 | Extrinsic 数量 | 面向 |
|--------|------|---------------|------|
| `pallet-entity-registry` | 实体注册 | 18 | Owner/Admin/Governance |
| `pallet-entity-shop` | 店铺管理 | 14 | Owner/Admin/Manager |
| `pallet-entity-service` | 商品管理 | 5 | Owner/Admin |
| `pallet-entity-order` | 订单管理 | 9 | Buyer/Seller |
| `pallet-entity-review` | 评价管理 | 2 | Buyer/Admin |
| `pallet-entity-token` | 代币管理 | 16 | Owner/Admin/User |
| `pallet-entity-market` | 代币市场 | 22 | Trader/Owner/OCW |
| `pallet-entity-member` | 会员管理 | 23 | Owner/Admin/User |
| `pallet-entity-commission-*` | 佣金系统 | ~30 | Owner/Admin/User |
| `pallet-entity-governance` | DAO 治理 | 9 | Owner/Admin/Voter |
| `pallet-entity-disclosure` | 财务披露 | 15 | Owner/Admin |
| `pallet-entity-kyc` | KYC 认证 | 9 | User/Provider/Admin |
| `pallet-entity-tokensale` | 代币发售 | 14 | Owner/Admin/Subscriber |
| **合计** | | **~186** | |
