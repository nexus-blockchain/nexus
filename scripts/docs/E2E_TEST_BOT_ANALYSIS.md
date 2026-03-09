# Nexus 全栈测试智能体 — 架构与实现文档

> 最后更新: 2026-03-08
> 范围: Cargo 单元测试 + E2E 链上流程测试 + 测试计划覆盖率追踪

---

## 1. 项目概述

Nexus 全栈测试智能体 (`nexus-test-agent`) 是一个统一协调 **Cargo 单元测试**、**E2E 链上测试** 和 **覆盖率追踪** 的测试框架。已从最初的可行性提案发展为完整实现，覆盖 5 大业务领域、23 条 E2E 流程、29 个 Cargo Pallet。

### 1.1 当前实现状态

| 组件 | 状态 | 说明 |
|------|:----:|------|
| 核心框架 (`core/`) | ✅ 已完成 | test-runner, assertions (9 种), reporter, chain-state, config, cargo-runner, coverage-tracker |
| 测试夹具 (`fixtures/`) | ✅ 已完成 | accounts.ts — 角色工厂 + 自动充值; bootstrap.ts — 开发链引导 (初始价格等) |
| Trading 流程 (T1-T4) | ✅ 已实现 | 种子资金+流动性、NEX 卖单、NEX 买单、NEX 市场 (DEX) |
| Entity 流程 (E1-E9) | ✅ 已实现 | 实体/店铺、订单、会员、佣金、Token+治理、KYC、代币发售、实体市场、信息披露 |
| Dispute 流程 (D1-D2) | ✅ 已实现 | 证据+投诉+和解+仲裁、托管 (Escrow) |
| GroupRobot 流程 (G1-G7) | ✅ 已实现 | Bot 生命周期、节点共识、广告活动、订阅服务、社区管理、仪式验证、奖励分配 |
| Storage 流程 (S1) | ✅ 已实现 | 运营者+用户+Pin+扣费+Slash |
| 覆盖率追踪 | ✅ 已完成 | 解析 NEXUS_TEST_PLAN*.md，映射 Flow → 测试用例 ID |
| 综合报告 | ✅ 已完成 | JSON 报告 (cargo + e2e + coverage) |
| CI 集成 | ⏳ 待实现 | GitHub Actions workflow |

### 1.2 测试层次架构

```
┌─────────────────────────────────────────────────────┐
│  nexus-test-agent.ts (统一入口)                       │
│  协调 Cargo + E2E + Coverage 三阶段                   │
├─────────────────────────────────────────────────────┤
│  Phase 1: Cargo 单元测试 (29 pallets)                 │
│  cargo-runner.ts → 逐 pallet 执行 cargo test          │
├─────────────────────────────────────────────────────┤
│  Phase 2: E2E 链上测试 (23 flows)                     │
│  test-runner.ts → 连接节点 → 角色账户 → 逐 flow 执行    │
├─────────────────────────────────────────────────────┤
│  Phase 3: 覆盖率追踪                                  │
│  coverage-tracker.ts → 解析测试计划 → 映射 → 报告       │
└─────────────────────────────────────────────────────┘
```

---

## 2. 项目结构

```
scripts/e2e/
├── core/                          # 核心框架 (7 模块)
│   ├── test-runner.ts             # FlowContext + 流程执行引擎
│   ├── assertions.ts              # 链上断言库 (9 种断言)
│   ├── chain-state.ts             # API 连接 + 交易签发 + Storage 查询
│   ├── config.ts                  # 环境配置 + 代币精度
│   ├── reporter.ts                # 结果收集 + JSON 报告
│   ├── cargo-runner.ts            # Cargo test 执行 + 解析 (29 pallets)
│   └── coverage-tracker.ts        # 测试计划解析 + 覆盖映射
│
├── fixtures/                      # 测试夹具
│   ├── accounts.ts                # Dev 账户 (alice..ferdie) + 充值
│   └── bootstrap.ts               # 开发链引导 (设置 NEX/USDT 初始价格等)
│
├── flows/                         # 23 条 E2E 流程
│   ├── trading/
│   │   ├── maker-lifecycle.ts     # T1: 做市商完整生命周期
│   │   ├── p2p-buy.ts             # T2: P2P 买入完整流程
│   │   ├── p2p-sell.ts            # T3: P2P 卖出完整流程
│   │   └── nex-market.ts          # T4: NEX 市场 (DEX) 完整流程
│   │
│   ├── entity/
│   │   ├── entity-shop.ts         # E1: 实体+店铺创建/管理
│   │   ├── order-lifecycle.ts     # E2: 商品→订单→发货→收货
│   │   ├── member-referral.ts     # E3: 会员注册+推荐+等级
│   │   ├── commission.ts          # E4: 佣金计算+提现+回购
│   │   ├── token-governance.ts    # E5: 代币+治理提案+投票
│   │   ├── kyc.ts                 # E6: KYC 认证完整流程
│   │   ├── token-sale.ts          # E7: 代币发售+认购+退款
│   │   ├── entity-market.ts       # E8: 实体市场交易 (挂单/吃单/市价)
│   │   └── entity-disclosure.ts   # E9: 信息披露+公告+内幕人员+黑窗口
│   │
│   ├── dispute/
│   │   ├── dispute-resolution.ts  # D1: 证据+投诉+和解+仲裁
│   │   └── escrow.ts              # D2: 托管锁定/释放/退款/争议仲裁
│   │
│   ├── grouprobot/
│   │   ├── bot-lifecycle.ts       # G1: Bot 注册→TEE→社区→停用
│   │   ├── node-consensus.ts      # G2: 节点+质押+Slash+退出
│   │   ├── ad-campaign.ts         # G3: 广告创建→投放→结算
│   │   ├── subscription.ts        # G4: 订阅→充值→变更→广告承诺→取消
│   │   ├── community.ts           # G5: 行为日志→节点策略→声誉管理
│   │   ├── ceremony.ts            # G6: Enclave 审批→仪式记录→强制重做
│   │   └── rewards.ts             # G7: 奖励领取→救援滞留奖励
│   │
│   └── storage/
│       └── storage-service.ts     # S1: 运营者+Pin+扣费+Slash
│
├── run-e2e.ts                     # 独立 E2E 运行入口 (3 个 Phase, 全部 23 流程)
├── nexus-test-agent.ts            # 统一测试智能体 (Cargo+E2E+Coverage)
└── test-coverage-check.ts         # 快速覆盖率检查 (无需链)
```

---

## 3. 核心框架

### 3.1 test-runner.ts — 流程执行引擎

定义 `FlowDef` 接口和 `FlowContext` 执行上下文:

```typescript
interface FlowDef {
  name: string;           // 流程名称 (如 "Flow-T1: 做市商生命周期")
  description: string;    // 简要描述
  fn: FlowFn;             // (ctx: FlowContext) => Promise<void>
}

interface FlowContext {
  api: ApiPromise;                           // Polkadot.js API
  reporter: TestReporter;                    // 报告器实例
  actor(name: string): KeyringPair;          // 获取角色账户
  send(tx, signer, stepName, actorName?): Promise<TxResult>;  // 签名发送交易 (自动记录步骤)
  sudo(tx, stepName): Promise<TxResult>;     // 通过 Alice Sudo 发送
  check(stepName, actorName, fn): Promise<void>;  // 断言检查步骤
}
```

- **runFlows(api, actors, flows)**: 串行执行多条流程，返回 `{ reporter, allPassed }`
- **runSingleFlow(api, actors, flow, reporter)**: 执行单条流程，自动 catch 错误

### 3.2 assertions.ts — 9 种链上断言

| 断言函数 | 用途 |
|----------|------|
| `assertTxSuccess(result, context?)` | 交易执行成功 |
| `assertTxFailed(result, expectedError?, context?)` | 交易执行失败 (可匹配错误名) |
| `assertEventEmitted(result, section, method, context?)` | 事件已发射 |
| `assertBalanceChange(api, addr, balanceBefore, expectedDelta, toleranceBps?, context?)` | 余额变化 (带容差，默认 1% / 100bps) |
| `assertStorageExists(api, pallet, storage, keys, context?)` | Storage 条目存在 |
| `assertStorageEmpty(api, pallet, storage, keys, context?)` | Storage 条目不存在 |
| `assertStorageField(api, pallet, storage, keys, fieldPath, expected, context?)` | Storage 字段值匹配 (支持嵌套路径 `a.b.c`) |
| `assertEqual(actual, expected, context?)` | 通用相等断言 |
| `assertTrue(value, context?)` | 通用真值断言 |

### 3.3 chain-state.ts — 链上交互层

| 函数 | 用途 |
|------|------|
| `getApi(wsUrl?)` | 建立 WS 连接 (默认 `ws://127.0.0.1:9944`，支持 `WS_URL` 环境变量) |
| `disconnectApi()` | 关闭连接 |
| `signAndSend(api, tx, signer, description?)` | 签名发送，等待 finalized，返回 `TxResult {success, blockHash?, txHash?, events, error?}` |
| `sudoSend(api, tx, sudoAccount, description?)` | 通过指定 Sudo 账户发送 |
| `queryStorage(api, pallet, storage, ...keys)` | 查询任意 Storage |
| `getFreeBalance(api, address)` | 获取可用余额 (`bigint`) |
| `waitBlocks(api, count)` | 等待 n 个区块 (subscribeNewHeads) |

### 3.4 config.ts — 环境与代币精度

```typescript
interface E2EConfig {
  wsUrl: string;         // 默认 ws://127.0.0.1:9944，支持 WS_URL 环境变量
  txTimeout: number;     // 默认 60000 ms
  verbose: boolean;      // VERBOSE=true 开启详细日志
  NEX_DECIMALS: bigint;  // 1_000_000_000_000n (10^12)
  USDT_DECIMALS: bigint; // 1_000_000n (10^6)
}
```

| 函数 | 用途 |
|------|------|
| `nex(n)` | `n * 10^12` → 链上 Balance |
| `usdt(n)` | `n * 10^6` → 链上 Balance |
| `formatNex(raw)` | 原始值 → 可读 NEX 字符串 |
| `formatUsdt(raw)` | 原始值 → 可读 USDT 字符串 |

### 3.5 reporter.ts — 测试报告器

`TestReporter` 类:
- `startFlow(name, description)`: 开始记录一条流程
- `recordStep(name, actor, passed, duration, error?)`: 记录步骤结果
- `endFlow(error?)`: 结束当前流程，返回 `FlowResult`
- `printFlowResult(flow)`: 终端打印单条流程详细结果
- `printSummary()`: 打印汇总 (流程数/步骤数/耗时)
- `toJSON()`: 输出 JSON 报告 (含 timestamp + summary + flows)
- `allPassed`: getter，判断是否全部通过

### 3.6 cargo-runner.ts — Cargo 测试执行器

覆盖 **29 个 Pallet**，按模块群组织:

| 模块群 | Pallet 数 | Pallet 列表 |
|--------|:---------:|-------------|
| **Entity** | 12 | entity-registry, entity-shop, entity-product, entity-order, entity-review, entity-token, entity-governance, entity-member, entity-market, entity-disclosure, entity-kyc, entity-tokensale |
| **Commission** | 5 | commission-common, commission-core, commission-referral, commission-level-diff, commission-single-line |
| **Trading** | 1 | pallet-nex-market（无 TradingMaker/TradingP2p，全部通过 NexMarket） |
| **Dispute** | 3 | escrow, evidence, arbitration |
| **Storage** | 1 | storage-service |
| **GroupRobot** | 7 | grouprobot-registry, grouprobot-consensus, grouprobot-subscription, grouprobot-community, grouprobot-ceremony, grouprobot-rewards；广告通过 pallet-ads-grouprobot（非 pallet-grouprobot-ads） |

> **注**: 以下 pallet 存在于代码库但未纳入 `ALL_PALLETS` (无独立 Cargo 测试或为纯类型 crate):
> pallet-entity-common, pallet-entity-commission (虚拟根 crate), pallet-commission-multi-level, pallet-commission-pool-reward, pallet-commission-team,
> pallet-ads-core, pallet-ads-entity, pallet-ads-grouprobot, pallet-ads-primitives,
> pallet-trading-common, pallet-trading-trc20-verifier, pallet-storage-lifecycle,
> pallet-grouprobot-primitives, pallet-crypto-common
>
> **Runtime 新增**: pallet_authorship(#7), pallet_session(#8), Historical(#9), pallet_offences(#10)，用于 GRANDPA equivocation。SS58 前缀已改为 273（地址以 X 开头）。

功能:
- `runCargoTest(pallet, projectRoot, filter?)`: 逐 pallet 运行 `cargo test -p <pallet>`
- `runCargoTests(pallets, projectRoot, opts?)`: 批量运行，支持并发 (`concurrency` 默认 1)、过滤和回调
- 解析 stdout: 提取 pass/fail/ignored 计数和失败测试名
- `printCargoSummary(results)`: 打印所有 pallet 汇总

### 3.7 coverage-tracker.ts — 覆盖率追踪

| 函数 | 用途 |
|------|------|
| `parseTestPlan(planDir)` | 解析 `NEXUS_TEST_PLAN*.md` (3 个文件)，提取 `TestCase[]` (ID + 模块 + 角色 + 类型 + 优先级) |
| `applyCoverage(cases, coverageMap)` | 将 `CoverageMap { flowName: caseId[] }` 映射到测试用例 |
| `generateCoverageReport(cases)` | 按模块/优先级统计覆盖率，输出未覆盖 P0/P1 列表 |
| `printCoverageReport(report)` | 终端打印覆盖率报告 (含进度条) |
| `writeCoverageJSON(report, path)` | 写入 JSON 覆盖率文件 |

### 3.8 fixtures/ — 测试夹具

#### 3.8.1 accounts.ts — 角色账户

| 账户 | 典型角色 |
|------|----------|
| `alice` | Sudo / Root 管理员 |
| `bob` | 做市商 / Bot Owner / 广告主 / 卖家 / 节点运营者 |
| `charlie` | 买家 / 社区管理员 / 内幕人员 / 认购者 |
| `dave` | 卖家 / KYC Provider / 无权限用户 |
| `eve` | 实体所有者 |
| `ferdie` | 额外测试账户 |

- `getDevAccounts()`: 返回开发链预置 6 个账户 (`//Alice` … `//Ferdie`)
- `createFlowAccounts(flowPrefix, roles)`: 为指定 Flow 创建隔离账户 (URI: `//{prefix}/{role}`)
- `fundAccounts(api, accounts, amountNex=100_000)`: 从 Alice 批量转账 NEX 给目标账户

#### 3.8.2 bootstrap.ts — 开发链引导

在运行 E2E 之前设置必要的链上状态:

- `bootstrapDevChain(api, sudoAccount)`: 运行所有引导步骤
- `ensureInitialPrice(api, sudoAccount, priceU64?)`: 通过 `sudo(system.setStorage)` 直接写入 `NexMarket.LastTradePrice`

> **说明**: `nexMarket.setInitialPrice` 需要 `MarketAdminOrigin` (council)，在 dev 链上使用 `setStorage` 更便捷。默认价格 `10_000_000_000n` (1 USDT ≈ 10 NEX)。

---

## 4. E2E 流程详解

### 4.1 Trading 领域

> **说明**: 当前 Runtime 仅包含 `pallet_nex_market`，无 TradingMaker/TradingP2p。T1-T4 全部通过 NexMarket 实现。

#### Flow-T1: 种子资金+流动性 (`maker-lifecycle.ts`)

**角色**: Alice (Sudo), Bob (无权限用户 — 错误路径)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Sudo 设置初始价格 (`nexMarket.setInitialPrice`) | `assertTxSuccess` |
| 2 | Sudo 注入种子资金 (`nexMarket.fundSeedAccount`) | `assertTxSuccess` |
| 3 | Sudo 配置价格保护 (`nexMarket.configurePriceProtection`) | `assertTxSuccess` |
| 4 | Sudo 注入流动性 (`nexMarket.seedLiquidity`) | 成功/失败记录 |
| 5 | 验证初始价格已设置 | Storage 查询 |
| 6 | **[错误]** Bob 非 Sudo 设置价格 | `assertTxFailed` |
| 7 | **[错误]** Bob 非 Sudo 解除熔断 | `assertTxFailed` |

**涉及 Pallet**: NexMarket, Balances

---

#### Flow-T2: NEX 卖单流程 (`p2p-buy.ts`)

**角色**: Bob (卖家), Charlie (买家)

**前置条件**: Flow-T1 已设置初始价格

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Bob 挂卖单 (`nexMarket.placeSellOrder`, 100 NEX) | `assertTxSuccess` + `assertEventEmitted(SellOrderPlaced)` |
| 2 | Charlie 预锁定卖单 (`nexMarket.reserveSellOrder`, 50 NEX) | `assertTxSuccess` |
| 3 | Charlie 确认付款 (`nexMarket.confirmPayment`) | `assertTxSuccess` |
| 4 | 验证交易完成 | Event 检查 |
| 5 | Bob 挂卖单 → 取消 | `assertTxSuccess` |
| 6 | **[错误]** Charlie 取消他人订单 | `assertTxFailed` |
| 7 | **[错误]** 超时处理 (processTimeout on non-existent trade) | `assertTxFailed` |

**涉及 Pallet**: NexMarket, Balances

---

#### Flow-T3: NEX 买单流程 (`p2p-sell.ts`)

**角色**: Bob (买家), Dave (卖家), Charlie (无权限用户)

**前置条件**: Flow-T1 已设置初始价格

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Bob 挂买单 (`nexMarket.placeBuyOrder`, 50 NEX) | `assertTxSuccess` + `assertEventEmitted(BuyOrderPlaced)` |
| 2 | Dave 接受买单 (`nexMarket.acceptBuyOrder`) | `assertTxSuccess` |
| 3 | 验证交易事件 | Event 检查 |
| 4 | Bob 挂买单 → 取消 | `assertTxSuccess` |
| 5 | **[错误]** Charlie 取消他人买单 | `assertTxFailed` |

**涉及 Pallet**: NexMarket, Balances

---

#### Flow-T4: NEX 市场 (DEX) 完整流程 (`nex-market.ts`)

**角色**: Bob (卖家), Charlie (买家), Alice (Sudo), Dave (无权限用户)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Alice 配置价格保护 (`nexMarket.configurePriceProtection`) | `assertTxSuccess` |
| 2 | Alice 设置初始价格 (`nexMarket.setInitialPrice`) | `assertTxSuccess` |
| 3 | Bob 挂卖单 (`nexMarket.placeSellOrder`, 100 NEX, 1 USDT/NEX) | `assertTxSuccess` + `assertTrue(OrderPlaced)` + 验证 NEX 锁定 |
| 4 | Charlie 挂买单 (`nexMarket.placeBuyOrder`, 50 NEX) | `assertTxSuccess` + `assertTrue(OrderPlaced)` |
| 5 | Charlie 预锁定卖单 (`nexMarket.reserveSellOrder`, 30 NEX) | `assertEventEmitted(OrderReserved)` |
| 6 | Charlie 确认付款 (`nexMarket.confirmPayment`) | `assertEventEmitted(PaymentConfirmed)` |
| 7 | Bob 接受买单 (`nexMarket.acceptBuyOrder`) | `assertEventEmitted(BuyOrderAccepted)` |
| 8 | Bob 取消订单 (`nexMarket.cancelOrder`) | `assertTxSuccess` + `assertEventEmitted(OrderCancelled)` |
| 9 | Alice 解除熔断 (`nexMarket.liftCircuitBreaker`) | 成功/失败记录 |
| 10 | Alice 注资种子账户 (`nexMarket.fundSeedAccount`) | 成功/失败记录 |
| 11 | Alice 注入种子流动性 (`nexMarket.seedLiquidity`) | 成功/失败记录 |
| 12 | **[错误]** Dave 取消他人订单 | `assertTxFailed` |
| 13 | **[错误]** 不存在交易超时 (`nexMarket.processTimeout`) | `assertTxFailed` |

**涉及 Pallet**: NexMarket, Balances

---

### 4.2 Entity 领域

#### Flow-E1: 实体+店铺创建/管理 (`entity-shop.ts`)

**角色**: Eve (实体所有者), Alice (Sudo/治理审批), Charlie (无权限)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Eve 创建实体 (`entityRegistry.createEntity`) | `assertTxSuccess` |
| 2 | 验证实体已创建 (Active) + UserEntity 记录 + 金库资金扣除 | `assertStorageExists` + `assertStorageField(status, Active)` + `assertTrue(delta > 0)` |
| 3 | 查询实体详情 (ID/名称/类型/状态/所有者) | Storage 字段打印 |
| 4 | Eve 更新实体 (`entityRegistry.updateEntity`) | `assertTxSuccess` + `assertStorageField(name, Updated)` |
| 5 | Sudo 暂停实体 (`entityRegistry.suspendEntity`) | `assertTxSuccess` |
| 6 | 验证状态为 Suspended | `assertStorageField(status, Suspended)` |
| 7 | Sudo 恢复实体 (`entityRegistry.resumeEntity`) | `assertTxSuccess` |
| 8 | 验证状态恢复为 Active | `assertStorageField(status, Active)` |
| 9 | **[错误]** Charlie 更新 Eve 的实体 | `assertTxFailed(NotEntityOwner)` |
| 10 | **[错误]** 空名称创建实体 | `assertTxFailed(NameEmpty)` |
| 11 | Eve 申请关闭实体 (`entityRegistry.requestCloseEntity`) | `assertTxSuccess` + 验证状态 `PendingClose` |
| 12 | Sudo 审批关闭 (`entityRegistry.approveCloseEntity`) | `assertTxSuccess` + 验证状态 `Closed` |

**涉及 Pallet**: EntityRegistry, Balances

---

#### Flow-E2: 商品→订单完整生命周期 (`order-lifecycle.ts`)

**角色**: Eve (卖家/实体主), Bob (买家), Alice (Sudo)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | 确保实体+店铺已就绪 | Storage 查询 |
| 2 | Eve 创建商品 (`entityProduct.createProduct`) | `assertTxSuccess` |
| 3 | Eve 上架商品 (`entityProduct.publishProduct`) | `assertTxSuccess` |
| 4 | Bob 下单 (`entityTransaction.placeOrder`) | `assertTxSuccess` + `assertEventEmitted(OrderPlaced)` |
| 5 | 验证资金锁定 | 余额变化 |
| 6 | Eve 发货 (`entityTransaction.shipOrder`) | `assertTxSuccess` |
| 7 | Bob 确认收货 (`entityTransaction.confirmReceipt`) | `assertTxSuccess` + `assertEventEmitted(OrderCompleted)` |
| 8 | 验证付款事件 | Event 检查 |
| 9 | **[错误]** Charlie 确认收货 (未授权) | `assertTxFailed` |
| 10 | **[错误]** 购买未上架商品 | `assertTxFailed` |
| 11 | 订单取消+退款流程 | `assertTxSuccess` |

**涉及 Pallet**: EntityRegistry, EntityShop, EntityProduct, EntityTransaction, Balances

---

#### Flow-E3: 会员注册+推荐+等级 (`member-referral.ts`)

**角色**: Eve (实体主), Bob (会员), Charlie (推荐人), Alice (Sudo)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Eve 确保实体就绪 | Storage 查询 |
| 2 | Bob 注册会员 (`entityMember.registerMember`) | `assertTxSuccess` + `assertEventEmitted(MemberRegistered)` |
| 3 | Bob 绑定推荐人 Charlie (`entityMember.bindReferrer`) | `assertTxSuccess` |
| 4 | Eve 初始化自定义等级体系 | `assertTxSuccess` |
| 5 | Eve 手动升级 Bob 等级 | `assertTxSuccess` |
| 6 | Eve 批准会员策略 | `assertTxSuccess` |
| 7 | **[错误]** 重复绑定推荐人 | `assertTxFailed` |
| 8 | **[错误]** 自己推荐自己 | `assertTxFailed` |

**涉及 Pallet**: EntityMember, EntityRegistry

---

#### Flow-E4: 佣金计算+提现+回购 (`commission.ts`)

**角色**: Eve (实体主), Bob (买家/会员), Charlie (推荐人), Alice (Sudo)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Eve 初始化佣金计划 (`commissionCore.initCommissionPlan`) | `assertTxSuccess` |
| 2 | Eve 设置佣金模式 | `assertTxSuccess` |
| 3 | Eve 启用佣金 | `assertTxSuccess` |
| 4 | Eve 设置提现配置 (含回购比例) | `assertTxSuccess` |
| 5 | Bob/Charlie 注册会员 + 绑定推荐 | `assertTxSuccess` |
| 6 | Bob 下单触发佣金计算 | 验证佣金事件 |
| 7 | Charlie 提取佣金 | `assertTxSuccess` + 余额变化 |
| 8 | **[错误]** 禁用购物余额时操作 | `assertTxFailed` |
| 9 | **[错误]** 超额提取 | `assertTxFailed` |
| 10 | Eve 提取实体资金 | `assertTxSuccess` |
| 11 | Eve 设置佣金比率 | `assertTxSuccess` |

**涉及 Pallet**: CommissionCore, CommissionReferral, EntityMember, EntityTransaction, EntityRegistry

---

#### Flow-E5: Token+治理提案+投票 (`token-governance.ts`)

**角色**: Eve (实体主), Bob (Token 持有者/投票者), Charlie (无权限), Alice (Sudo)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | 确保 Entity + Shop 就绪 | Storage 查询 |
| 2 | Eve 创建 Governance Token (`entityToken.createShopToken`) | `assertEventEmitted(ShopTokenCreated)` |
| 3 | Eve 铸造代币给 Bob (`entityToken.mintTokens`) | `assertTxSuccess` |
| 4 | Bob 转让部分给 Charlie | `assertTxSuccess` |
| 5 | **[错误]** 超 max_supply 铸造 | `assertTxFailed` |
| 6 | Eve 配置治理模式 FullDAO (`entityGovernance.configureGovernance`) | `assertTxSuccess` |
| 7 | Bob 创建提案 (`entityGovernance.createProposal`) | `assertEventEmitted(ProposalCreated)` |
| 8 | **[错误]** Charlie 持有不足创建提案 | `assertTxFailed` |
| 9 | Bob 投票 (`entityGovernance.vote`) | `assertTxSuccess` |
| 10 | 等待投票期结束 → 结束投票 (`finalizeVoting`) | `assertEventEmitted(VotingFinalized)` |
| 11 | 等待执行延迟 → 执行提案 (`executeProposal`) | `assertEventEmitted(ProposalExecuted)` |
| 12 | Eve 设置转账限制 Whitelist | `assertTxSuccess` |
| 13 | Bob (白名单内) 转账成功 | `assertTxSuccess` |
| 14 | **[错误]** Charlie (非白名单) 转账失败 | `assertTxFailed` |
| 15 | Eve 锁仓 + 等待 + 解锁 | `assertTxSuccess` |

**涉及 Pallet**: EntityToken, EntityGovernance, EntityRegistry, EntityShop

---

#### Flow-E6: KYC 认证完整流程 (`kyc.ts`)

**角色**: Eve (实体主), Bob (申请人), Dave (Provider), Charlie (无权限), Alice (Sudo)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Sudo 注册 KYC Provider Dave | `assertEventEmitted(ProviderRegistered)` |
| 2 | Bob 提交 KYC Basic (`entityKyc.submitKyc`) | `assertTxSuccess` + `assertEventEmitted(KycSubmitted)` |
| 3 | Dave 批准 KYC (`entityKyc.approveKyc`) | `assertTxSuccess` |
| 4 | 验证 KYC 状态 (Approved, risk_score) | Storage 查询 |
| 5 | **[错误]** 空 data_cid 提交 | `assertTxFailed(EmptyDataCid)` |
| 6 | **[错误]** 非法国家代码 | `assertTxFailed` |
| 7 | Eve 设置实体 KYC 要求 (min_level, max_risk_score) | `assertTxSuccess` |
| 8 | **[错误]** max_risk_score > 100 | `assertTxFailed` |
| 9 | Sudo 撤销 KYC (`entityKyc.revokeKyc`) | `assertTxSuccess` + `assertEventEmitted(KycRevoked)` |
| 10 | Sudo 更新高风险国家列表 | `assertTxSuccess` |

**涉及 Pallet**: EntityKyc, EntityRegistry

---

#### Flow-E7: 代币发售+认购+退款 (`token-sale.ts`)

**角色**: Eve (发售创建者), Bob (认购者A), Charlie (认购者B), Alice (Sudo)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | 确保 Entity + Shop + Token 就绪 | Storage 查询 |
| 2 | Eve 创建发售轮次 FixedPrice (`entityTokensale.createSaleRound`) | `assertTxSuccess` + `assertEventEmitted(SaleRoundCreated)` |
| 3 | Eve 添加支付选项 NEX | `assertTxSuccess` |
| 4 | Eve 设置锁仓配置 (50% 初始解锁, 10 periods) | `assertTxSuccess` |
| 5 | 等待 start_block → Eve 开始发售 | `assertTxSuccess` |
| 6 | Bob 认购 1000 tokens | `assertTxSuccess` + `assertEventEmitted(Subscribed)` |
| 7 | Charlie 认购 2000 tokens | `assertTxSuccess` |
| 8 | **[错误]** 超过发售上限认购 | `assertTxFailed` |
| 9 | 等待 end_block → Eve 结束发售 | `assertTxSuccess` |
| 10 | Bob 领取代币 | `assertTxSuccess` + `assertEventEmitted(TokensClaimed)` |
| 11 | Charlie 领取代币 | `assertTxSuccess` |
| 12 | Eve 提取募集资金 (`withdrawFunds`) | `assertTxSuccess` |
| 13 | 取消流程: 新轮次 → Bob 认购 → Eve 取消 | `assertTxSuccess` |
| 14 | Bob 领取退款 (`claimRefund`) | `assertTxSuccess` + `assertEventEmitted(RefundClaimed)` |

**涉及 Pallet**: EntityTokenSale, EntityToken, EntityRegistry, EntityShop

---

#### Flow-E8: 实体市场交易完整流程 (`entity-market.ts`)

**角色**: Bob (卖家 — Token 持有者), Charlie (买家), Alice (Sudo — 实体/Token 创建), Dave (无权限用户)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | 创建实体 + 代币 + 铸造给 Bob/Charlie | `assertTxSuccess` |
| 2 | Bob 配置市场参数 (`entityMarket.configureMarket`) | `assertTxSuccess` |
| 3 | Bob 设置初始价格 (`entityMarket.setInitialPrice`) | `assertTxSuccess` |
| 4 | Bob 配置价格保护 (`entityMarket.configurePriceProtection`) | `assertTxSuccess` |
| 5 | Bob 挂 NEX 卖单 (`entityMarket.placeSellOrder`) | `assertTxSuccess` + `assertTrue(OrderPlaced)` |
| 6 | Charlie 吃卖单 (`entityMarket.takeOrder`, 部分成交) | `assertTxSuccess` + `assertEventEmitted(OrderFilled)` |
| 7 | Bob 挂 NEX 买单 (`entityMarket.placeBuyOrder`) | `assertTxSuccess` |
| 8 | Charlie 吃买单 (全部成交) | `assertTxSuccess` |
| 9 | Charlie 市价买入 (`entityMarket.marketBuy`) | 成功/失败记录 |
| 10 | Charlie 市价卖出 (`entityMarket.marketSell`) | 成功/失败记录 |
| 11 | Bob 取消订单 (`entityMarket.cancelOrder`) | `assertTxSuccess` + `assertEventEmitted(OrderCancelled)` |
| 12 | USDT 卖单流程: 挂 USDT 卖单 → Charlie 预锁定 → 确认支付 | `assertTxSuccess` + Event 验证 |
| 13 | **[错误]** Dave 取消他人订单 | `assertTxFailed` |
| 14 | **[错误]** 熔断测试 | 成功/失败记录 |

**涉及 Pallet**: EntityMarket, EntityRegistry, EntityToken, Balances

---

#### Flow-E9: 信息披露+公告完整流程 (`entity-disclosure.ts`)

**角色**: Bob (实体所有者), Alice (Sudo), Charlie (内幕人员), Dave (无权限用户)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | 创建实体 | `assertTxSuccess` |
| 2 | Bob 配置披露设置 (`entityDisclosure.configureDisclosure`) | `assertTxSuccess` |
| 3 | Bob 发布披露 (`entityDisclosure.publishDisclosure`, Financial) | `assertTxSuccess` + `assertTrue(DisclosurePublished)` |
| 4 | Bob 更正披露 (`entityDisclosure.correctDisclosure`) | `assertTxSuccess` + `assertEventEmitted(DisclosureCorrected)` |
| 5 | Bob 撤回披露 (`entityDisclosure.withdrawDisclosure`) | `assertTxSuccess` + `assertEventEmitted(DisclosureWithdrawn)` |
| 6 | Bob 清理披露历史 (`entityDisclosure.cleanupDisclosureHistory`) | 成功/失败记录 |
| 7 | Bob 添加内幕人员 Charlie (`entityDisclosure.addInsider`) | `assertTxSuccess` + `assertEventEmitted(InsiderAdded)` |
| 8 | Bob 开始黑窗口期 (`entityDisclosure.startBlackout`) | `assertTxSuccess` + `assertEventEmitted(BlackoutStarted)` |
| 9 | Bob 结束黑窗口期 (`entityDisclosure.endBlackout`) | `assertTxSuccess` |
| 10 | Bob 移除内幕人员 (`entityDisclosure.removeInsider`) | `assertTxSuccess` |
| 11 | Bob 发布公告 (`entityDisclosure.publishAnnouncement`) | `assertTxSuccess` + `assertTrue(AnnouncementPublished)` |
| 12 | Bob 更新公告 (`entityDisclosure.updateAnnouncement`) | `assertTxSuccess` |
| 13 | Bob 置顶/取消置顶 (`entityDisclosure.pinAnnouncement`) | `assertTxSuccess` |
| 14 | Bob 撤回公告 (`entityDisclosure.withdrawAnnouncement`) | `assertTxSuccess` |
| 15 | Bob 清理公告历史 (`entityDisclosure.cleanupAnnouncementHistory`) | 成功/失败记录 |
| 16 | **[错误]** Dave 发布披露 | `assertTxFailed` |
| 17 | **[错误]** Dave 撤回他人披露 | `assertTxFailed` |

**涉及 Pallet**: EntityDisclosure, EntityRegistry

---

### 4.3 Dispute 领域

#### Flow-D1: 争议解决完整流程 (`dispute-resolution.ts`)

**角色**: Bob (原告), Eve (被告), Charlie (无权限), Alice (Sudo/仲裁委员会)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Bob 提交证据 (`evidence.commit`) | `assertTxSuccess` + `assertEventEmitted(EvidenceCommitted)` |
| 2 | Eve 提交反驳证据 | `assertTxSuccess` |
| 3 | Bob 发起投诉 (`arbitration.fileComplaint`) | `assertTxSuccess` + `assertEventEmitted(ComplaintFiled)` + 验证押金扣除 |
| 4 | Eve 响应投诉 (`respondToComplaint`) | `assertTxSuccess` |
| 5 | 双方和解 (`settleComplaint`) | `assertEventEmitted(ComplaintSettled)` |
| 6 | Bob 发起新投诉 → 升级到仲裁 (`escalateToArbitration`) | `assertEventEmitted(ComplaintEscalated)` |
| 7 | Sudo 仲裁裁决 (`resolveComplaint`, FavorComplainant) | `assertTxSuccess` + `assertEventEmitted(ComplaintResolved)` |
| 8 | **[错误]** Charlie 尝试裁决 (非仲裁者) | `assertTxFailed` |
| 9 | Bob 撤销投诉 → 验证押金退还 | `assertTxSuccess` + 余额变化 |

**涉及 Pallet**: Evidence, Arbitration, Balances

---

#### Flow-D2: 托管 (Escrow) 完整流程 (`escrow.ts`)

**角色**: Bob (付款人), Charlie (收款人), Alice (Sudo/AuthorizedOrigin — 仲裁/暂停), Dave (无权限用户)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Bob 锁定资金 (`escrow.lock`, 50 NEX) | `assertTxSuccess` + `assertEventEmitted(Locked)` + 验证余额减少 |
| 2 | 验证托管状态 | Storage 查询打印 |
| 3 | Bob nonce 幂等锁定 (`escrow.lockWithNonce`, nonce=1) | `assertTxSuccess` + 重复 nonce 忽略 |
| 4 | 释放资金给 Charlie (`escrow.release`) | `assertTxSuccess` + 验证 Charlie 余额增加 |
| 5 | 锁定 → 退款 (`escrow.refund`) | `assertTxSuccess` + 验证退款到账 |
| 6 | 锁定 → 分账释放 (`escrow.releaseSplit`, 60/40) | `assertTxSuccess` + `assertEventEmitted(Released)` |
| 7 | 锁定 → 争议 → 仲裁全额释放 (`escrow.applyDecisionReleaseAll`) | `assertTxSuccess` + `assertEventEmitted(Disputed)` |
| 8 | 锁定 → 争议 → 仲裁全额退款 (`escrow.applyDecisionRefundAll`) | `assertTxSuccess` |
| 9 | 锁定 → 争议 → 仲裁部分释放 (`escrow.applyDecisionPartialBps`, 70/30) | 成功/失败记录 |
| 10 | Alice 设置全局暂停 → 验证锁定被拒绝 → 取消暂停 | `assertTxSuccess` |
| 11 | 安排到期 → 取消到期 (`escrow.scheduleExpiry` / `cancelExpiry`) | `assertTxSuccess` |
| 12 | **[错误]** Dave 释放他人托管 | `assertTxFailed` |
| 13 | **[错误]** 争议状态下释放被拒绝 | `assertTxFailed` |

**涉及 Pallet**: Escrow, Balances

---

### 4.4 GroupRobot 领域

#### Flow-G1: Bot 完整生命周期 (`bot-lifecycle.ts`)

**角色**: Bob (Bot Owner), Charlie (无权限), Alice (Sudo)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Bob 注册 Bot (`grouprobotRegistry.registerBot`) | `assertTxSuccess` + `assertEventEmitted(BotRegistered)` |
| 2 | 验证 Bot 已创建 (ownerBots 查询) | `assertTrue(bots.length > 0)` |
| 3 | Bob 更换公钥 (`updatePublicKey`) | `assertTxSuccess` + `assertEventEmitted(PublicKeyUpdated)` |
| 4 | Sudo 审批 MRTD (`approveMrtd`) | `assertTxSuccess` |
| 5 | Bob 提交 TEE 证明 (`submitAttestation`, 软件模式) | 成功/失败记录 |
| 6 | Bob 绑定社区 (`bindCommunity`) | `assertTxSuccess` + `assertEventEmitted(CommunityBound)` |
| 7 | **[错误]** Charlie 绑定社区到 Bob 的 Bot | `assertTxFailed` |
| 8 | Bob 解绑社区 | `assertTxSuccess` |
| 9 | Bob 停用 Bot (`deactivateBot`) | `assertTxSuccess` + `assertEventEmitted(BotDeactivated)` |
| 10 | **[错误]** 停用后绑定社区 | `assertTxFailed` |

**涉及 Pallet**: GroupRobotRegistry

---

#### Flow-G2: 节点共识完整流程 (`node-consensus.ts`)

**角色**: Bob (节点运营者), Charlie (举报者), Dave (无权限), Alice (Sudo)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Bob 注册节点+质押 100 NEX (`registerNode`) | `assertTxSuccess` + `assertEventEmitted(NodeRegistered)` + 验证质押扣除 |
| 2 | **[错误]** Dave 质押不足注册 | `assertTxFailed` |
| 3 | 验证节点已注册 (nodes 查询) | Storage 查询 |
| 4 | Bob 标记消息序列已处理 (`markSequenceProcessed`) | `assertTxSuccess` |
| 5 | **[错误]** 重复序列标记 | `assertTxFailed` |
| 6 | **[错误]** Dave Free tier 标记 | `assertTxFailed` |
| 7 | Bob 验证节点 TEE (`verifyNodeTee`) | 成功/失败记录 |
| 8 | Sudo 设置 TEE 奖励参数 (`setTeeRewardParams`) | `assertTxSuccess` |
| 9 | Charlie 举报 Equivocation | `assertEventEmitted(EquivocationReported)` |
| 10 | Sudo Slash (`slashEquivocation`) | `assertEventEmitted(NodeSlashed)` |
| 11 | Bob 申请退出 (`requestExit`) | `assertTxSuccess` |
| 12 | 等待冷却期 → Bob 完成退出 + 验证质押退还 | `assertTxSuccess` + 余额变化 |

**涉及 Pallet**: GroupRobotConsensus, Balances

---

#### Flow-G3: 广告活动完整流程 (`ad-campaign.ts`)

**角色**: Bob (广告主), Charlie (社区管理员), Dave (无权限), Alice (Sudo)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Sudo 设置社区管理员 Charlie | `assertTxSuccess` |
| 2 | Charlie 质押 50 NEX 获取 audience_cap | `assertTxSuccess` + `assertEventEmitted(StakedForAds)` |
| 3 | Bob 创建广告活动 (预算 100 NEX, CPM 1000) | `assertTxSuccess` + `assertEventEmitted(CampaignCreated)` + 验证预算锁定 |
| 4 | Sudo 审核广告 (approve) | `assertTxSuccess` |
| 5 | Bob 追加预算 50 NEX | `assertTxSuccess` |
| 6 | Charlie 提交投放收据 (500 audience, cap 裁切) | `assertTxSuccess` + `assertEventEmitted(DeliveryReceiptSubmitted)` |
| 7 | Era 结算 CPM 计费 (`settleEraAds`) | `assertEventEmitted(EraSettled)` |
| 8 | Charlie 提取广告收入 | 余额变化验证 |
| 9 | Bob 暂停广告 | `assertTxSuccess` |
| 10 | Bob 取消广告 → 退还剩余预算 | `assertTxSuccess` + 余额增加 |
| 11 | 双向偏好: 广告主拉黑/取消拉黑社区, 社区拉黑/取消拉黑广告主 | 全部 `assertTxSuccess` |
| 12 | **[错误]** Dave 非管理员提取收入 | `assertTxFailed` |
| 13 | **[错误]** Dave 非管理员拉黑 | `assertTxFailed` |
| 14 | Sudo Slash 社区 10 NEX | `assertTxSuccess` + `assertEventEmitted(CommunitySlashed)` |
| 15 | Charlie 取消质押 | 记录结果 |

**涉及 Pallet**: GroupRobotAds, Balances

---

#### Flow-G4: 订阅服务完整流程 (`subscription.ts`)

**角色**: Bob (订阅用户/群主), Charlie (广告承诺用户), Dave (无权限用户)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Bob 订阅 Bot 服务 (`groupRobotSubscription.subscribe`, Basic) | `assertTxSuccess` + `assertTrue(Subscribed)` + 验证扣费 |
| 2 | 验证订阅已创建 | Storage 查询 |
| 3 | Bob 充值订阅 (`groupRobotSubscription.depositSubscription`) | `assertTxSuccess` |
| 4 | Bob 变更订阅层级 (`groupRobotSubscription.changeTier`, Pro) | `assertEventEmitted(TierChanged)` |
| 5 | Charlie 通过广告承诺订阅 (`groupRobotSubscription.commitAds`) | `assertEventEmitted(AdCommitted)` |
| 6 | Charlie 取消广告承诺 (`groupRobotSubscription.cancelAdCommitment`) | 成功/失败记录 |
| 7 | Bob 取消付费订阅 (`groupRobotSubscription.cancelSubscription`) | `assertTxSuccess` + `assertEventEmitted(SubscriptionCancelled)` |
| 8 | 清理已取消的订阅记录 (`groupRobotSubscription.cleanupSubscription`) | 成功/失败记录 |
| 9 | 清理广告承诺记录 (`groupRobotSubscription.cleanupAdCommitment`) | 成功/失败记录 |
| 10 | **[错误]** Dave 充值不存在的订阅 | `assertTxFailed` |
| 11 | **[错误]** 重复订阅被拒绝 | `assertTxFailed` |

**涉及 Pallet**: GroupRobotSubscription, Balances

---

#### Flow-G5: 社区管理完整流程 (`community.ts`)

**角色**: Bob (Bot Owner / 社区管理者), Charlie (社区成员), Dave (无权限用户)

**前置条件**: Bot 已注册并绑定社区 (流程内自动完成)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Bob 提交行为日志 (`groupRobotCommunity.submitActionLog`, Kick) | `assertEventEmitted(ActionLogSubmitted)` |
| 2 | Bob 批量提交行为日志 (`groupRobotCommunity.batchSubmitLogs`, Ban+Mute) | 成功/失败记录 |
| 3 | Bob 设置节点准入策略 (`groupRobotCommunity.setNodeRequirement`, MinNodes: 1) | 成功/失败记录 |
| 4 | Bob 更新社区配置 (`groupRobotCommunity.updateCommunityConfig`, CAS 乐观锁) | `assertEventEmitted(CommunityConfigUpdated)` |
| 5 | Bob 奖励 Charlie 声誉 (`groupRobotCommunity.awardReputation`, +100) | `assertEventEmitted(ReputationAwarded)` |
| 6 | Bob 扣减 Charlie 声誉 (`groupRobotCommunity.deductReputation`, -50) | `assertEventEmitted(ReputationDeducted)` |
| 7 | Bob 重置 Charlie 声誉 (`groupRobotCommunity.resetReputation`) | 成功/失败记录 |
| 8 | Bob 更新活跃成员数 (`groupRobotCommunity.updateActiveMembers`, 42) | 成功/失败记录 |
| 9 | Bob 清理过期日志 (`groupRobotCommunity.clearExpiredLogs`) | 成功/失败记录 |
| 10 | 清理过期冷却 (`groupRobotCommunity.cleanupExpiredCooldowns`) | 成功/失败记录 |
| 11 | **[错误]** Dave 提交行为日志 (非 Bot Owner) | `assertTxFailed` |
| 12 | **[错误]** Dave 奖励声誉 (非管理者) | `assertTxFailed` |

**涉及 Pallet**: GroupRobotCommunity, GroupRobotRegistry

---

#### Flow-G6: 仪式验证完整流程 (`ceremony.ts`)

**角色**: Bob (Bot Owner / 仪式发起者), Alice (Sudo — Enclave 审批 / 强制 re-ceremony), Dave (无权限用户)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Alice 审批 Ceremony Enclave (`groupRobotCeremony.approveCeremonyEnclave`) | `assertTxSuccess` + `assertEventEmitted(CeremonyEnclaveApproved)` |
| 2 | Bob 记录仪式 (`groupRobotCeremony.recordCeremony`, k=3, n=5) | `assertTxSuccess` + `assertTrue(CeremonyRecorded)` |
| 3 | 验证仪式已记录 | Storage 查询 |
| 4 | Alice 强制 re-ceremony (`groupRobotCeremony.forceReCeremony`) | `assertTxSuccess` + `assertEventEmitted(ReCeremonyForced)` |
| 5 | Bob 重新记录仪式 | 成功/失败记录 |
| 6 | Alice 撤销仪式 (`groupRobotCeremony.revokeCeremony`) | `assertEventEmitted(CeremonyRevoked)` |
| 7 | 清理终态仪式记录 (`groupRobotCeremony.cleanupCeremony`) | 成功/失败记录 |
| 8 | Alice 移除 Enclave (`groupRobotCeremony.removeCeremonyEnclave`) | `assertTxSuccess` |
| 9 | **[错误]** Dave 记录仪式 (非 Bot Owner) | `assertTxFailed` |
| 10 | **[错误]** 未审批 Enclave 记录仪式 | `assertTxFailed` |

**涉及 Pallet**: GroupRobotCeremony

---

#### Flow-G7: 奖励分配完整流程 (`rewards.ts`)

**角色**: Bob (节点运营者), Alice (Sudo — 救援滞留奖励), Dave (无权限用户)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Bob 领取节点奖励 (`groupRobotRewards.claimRewards`) | `assertEventEmitted(RewardsClaimed)` + 余额变化 |
| 2 | Alice 救援滞留奖励 (`groupRobotRewards.rescueStrandedRewards`) | `assertEventEmitted(StrandedRewardsRescued)` |
| 3 | **[错误]** Dave 领取他人节点奖励 | `assertTxFailed` |
| 4 | **[错误]** 领取不存在节点的奖励 | `assertTxFailed` |

**涉及 Pallet**: GroupRobotRewards, Balances

---

### 4.5 Storage 领域

#### Flow-S1: 存储服务完整流程 (`storage-service.ts`)

**角色**: Bob (存储用户), Charlie (运营者), Dave (无权限), Alice (Sudo)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Sudo 设置计费参数 (`setBillingParams`) | `assertTxSuccess` |
| 2 | Charlie 加入运营者 (质押 50 NEX) | `assertTxSuccess` + 保证金扣除 |
| 3 | Charlie 更新运营者信息 (capacity, endpoint) | `assertTxSuccess` |
| 4 | Bob 充值用户账户 20 NEX | `assertTxSuccess` + `assertEventEmitted(UserFunded)` |
| 5 | Bob 请求 Pin 文件 (1 MB) | `assertTxSuccess` + `assertEventEmitted(PinRequested)` |
| 6 | **[错误]** Dave 余额不足 Pin 100 GB | `assertTxFailed` |
| 7 | Sudo 标记 Pin 成功 (`markPinned`) | `assertEventEmitted(FilePinned)` |
| 8 | Sudo 处理到期扣费 (`chargeDue`) | 记录结果 |
| 9 | Charlie 领取奖励 | 记录结果 |
| 10 | Charlie 暂停/恢复运营者 | 双向 `assertTxSuccess` |
| 11 | Sudo 分配资金给运营者 | 记录结果 |
| 12 | Sudo Slash 运营者 5 NEX | `assertEventEmitted(OperatorSlashed)` |
| 13 | Charlie 退出运营者 → 验证保证金退还 | `assertTxSuccess` + 余额变化 |
| 14 | **[错误]** Dave 非运营者更新/暂停 | `assertTxFailed` |

**涉及 Pallet**: StorageService, Balances

---

## 5. 入口点与使用方式

### 5.1 nexus-test-agent.ts — 统一入口

```bash
# 运行全部 (cargo + e2e + coverage)
npx tsx scripts/e2e/nexus-test-agent.ts

# 仅 cargo 测试
npx tsx scripts/e2e/nexus-test-agent.ts --mode cargo

# 仅 E2E 链上测试
npx tsx scripts/e2e/nexus-test-agent.ts --mode e2e

# 仅覆盖率检查
npx tsx scripts/e2e/nexus-test-agent.ts --mode coverage

# 指定模块群 (--group, 空格分隔)
npx tsx scripts/e2e/nexus-test-agent.ts --group entity grouprobot

# 指定流程 (--flow, 空格分隔, 自动转大写)
npx tsx scripts/e2e/nexus-test-agent.ts --flow E1 E2 D1

# 指定 pallet (--pallet, 空格分隔)
npx tsx scripts/e2e/nexus-test-agent.ts --pallet pallet-dispute-escrow pallet-entity-token

# 按优先级过滤覆盖率报告
npx tsx scripts/e2e/nexus-test-agent.ts --priority P0

# 输出目录 (默认 ./e2e-reports)
npx tsx scripts/e2e/nexus-test-agent.ts --report-dir ./reports

# 详细日志
npx tsx scripts/e2e/nexus-test-agent.ts --verbose

# 显示帮助
npx tsx scripts/e2e/nexus-test-agent.ts --help
```

### 5.2 run-e2e.ts — 独立 E2E 运行 (Phase 1+2+3, 全部 23 流程)

支持 3 个 Phase 分组和指定 Flow:

- **Phase 1**: 核心交易 + 实体基础 (T1-T3, E1) — 4 流程
- **Phase 2**: 实体扩展 + 争议 + 托管 (E2-E9, T4, D1-D2) — 11 流程
- **Phase 3**: GroupRobot + 存储 (G1-G7, S1) — 8 流程

```bash
# 运行全部 23 流程 (Phase 1+2+3)
npx tsx scripts/e2e/run-e2e.ts

# 运行指定 Phase
npx tsx scripts/e2e/run-e2e.ts --phase 1
npx tsx scripts/e2e/run-e2e.ts --phase 2
npx tsx scripts/e2e/run-e2e.ts --phase 3

# 运行指定流程 (--flow 标志, 空格分隔)
npx tsx scripts/e2e/run-e2e.ts --flow T1 T2 E1
npx tsx scripts/e2e/run-e2e.ts --flow G4 G5 G6 G7
```

> **注**: `run-e2e.ts` 额外调用 `bootstrapDevChain()` 引导开发链状态 (设置初始价格)，而 `nexus-test-agent.ts` 不执行引导。

### 5.3 test-coverage-check.ts — 快速覆盖率

```bash
# 无需运行链节点, 纯覆盖率映射检查
npx tsx scripts/e2e/test-coverage-check.ts
```

---

## 6. 流程注册与分组

nexus-test-agent 中注册的所有流程及其分组:

| Flow ID | 名称 | 分组 | 文件 |
|:-------:|------|:----:|------|
| T1 | 做市商生命周期 | trading | `flows/trading/maker-lifecycle.ts` |
| T2 | P2P Buy | trading | `flows/trading/p2p-buy.ts` |
| T3 | P2P Sell | trading | `flows/trading/p2p-sell.ts` |
| T4 | NEX 市场 (DEX) | trading | `flows/trading/nex-market.ts` |
| E1 | 实体+店铺 | entity | `flows/entity/entity-shop.ts` |
| E2 | 订单生命周期 | entity | `flows/entity/order-lifecycle.ts` |
| E3 | 会员+推荐 | entity | `flows/entity/member-referral.ts` |
| E4 | 佣金+提现 | entity | `flows/entity/commission.ts` |
| E5 | Token+治理 | entity | `flows/entity/token-governance.ts` |
| E6 | KYC 认证 | entity | `flows/entity/kyc.ts` |
| E7 | 代币发售 | entity | `flows/entity/token-sale.ts` |
| E8 | 实体市场 | entity | `flows/entity/entity-market.ts` |
| E9 | 信息披露 | entity | `flows/entity/entity-disclosure.ts` |
| D1 | 争议解决 | dispute | `flows/dispute/dispute-resolution.ts` |
| D2 | 托管 | dispute | `flows/dispute/escrow.ts` |
| G1 | Bot 生命周期 | grouprobot | `flows/grouprobot/bot-lifecycle.ts` |
| G2 | 节点共识 | grouprobot | `flows/grouprobot/node-consensus.ts` |
| G3 | 广告活动 | grouprobot | `flows/grouprobot/ad-campaign.ts` |
| G4 | 订阅服务 | grouprobot | `flows/grouprobot/subscription.ts` |
| G5 | 社区管理 | grouprobot | `flows/grouprobot/community.ts` |
| G6 | 仪式验证 | grouprobot | `flows/grouprobot/ceremony.ts` |
| G7 | 奖励分配 | grouprobot | `flows/grouprobot/rewards.ts` |
| S1 | 存储服务 | storage | `flows/storage/storage-service.ts` |

### 6.1 覆盖率映射 (COVERAGE_MAP)

nexus-test-agent 中定义了 23 条 Flow 到测试计划用例 ID 的映射:

| Flow | 映射用例数 | 用例 ID 前缀 |
|:----:|:---------:|:------------|
| T1 | 1 | NM |
| T2 | 4 | NM |
| T3 | 5 | NM |
| T4 | 12 | NM |
| E1 | 10 | ER, SH |
| E2 | 11 | OD, SV |
| E3 | 10 | MB |
| E4 | 9 | CM |
| E5 | 12 | TK, GV |
| E6 | 8 | KY |
| E7 | 13 | TS |
| E8 | 11 | EM |
| E9 | 9 | DC |
| D1 | 7 | EV, AR |
| D2 | 10 | ES |
| G1 | 5 | GR |
| G2 | 10 | CN |
| G3 | 18 | AD |
| G4 | 7 | SB |
| G5 | 9 | GC |
| G6 | 6 | CE |
| G7 | 2 | RW |
| S1 | 15 | SS |

---

## 7. Pallet 覆盖矩阵

### 7.1 E2E 流程覆盖的 Pallet

| Pallet | E2E 流程 | 覆盖 extrinsics |
|--------|:--------:|-----------------|
| NexMarket | T1, T2, T3, T4 | configurePriceProtection, setInitialPrice, placeSellOrder, placeBuyOrder, reserveSellOrder, confirmPayment, acceptBuyOrder, cancelOrder, liftCircuitBreaker, fundSeedAccount, seedLiquidity, processTimeout |
| EntityRegistry | E1-E9 | createEntity, updateEntity, suspendEntity, resumeEntity, requestCloseEntity, approveCloseEntity |
| EntityShop | E1, E2, E5, E7 | (自动创建 + 查询) |
| EntityProduct | E2 | createProduct, publishProduct |
| EntityTransaction | E2, E4 | placeOrder, shipOrder, confirmReceipt, cancelOrder |
| EntityToken | E5, E7, E8 | createShopToken, mintTokens, transferTokens, setTransferRestriction, addToWhitelist, lockTokens, unlockTokens |
| EntityGovernance | E5 | configureGovernance, createProposal, vote, finalizeVoting, executeProposal |
| EntityMember | E3, E4 | registerMember, bindReferrer, initCustomLevels, manualUpgrade |
| CommissionCore | E4 | initCommissionPlan, setMode, enableCommission, setWithdrawalConfig |
| EntityKyc | E6 | registerProvider, submitKyc, approveKyc, setEntityRequirement, revokeKyc, updateHighRiskCountries |
| EntityTokenSale | E7 | createSaleRound, addPaymentOption, setVestingConfig, startSale, subscribe, endSale, claimTokens, withdrawFunds, cancelSale, claimRefund |
| EntityMarket | E8 | configureMarket, setInitialPrice, configurePriceProtection, placeSellOrder, placeBuyOrder, takeOrder, marketBuy, marketSell, cancelOrder, placeUsdtSellOrder, reserveUsdtSellOrder, confirmUsdtPayment, liftCircuitBreaker |
| EntityDisclosure | E9 | configureDisclosure, publishDisclosure, correctDisclosure, withdrawDisclosure, cleanupDisclosureHistory, addInsider, startBlackout, endBlackout, removeInsider, publishAnnouncement, updateAnnouncement, pinAnnouncement, withdrawAnnouncement, cleanupAnnouncementHistory |
| Evidence | D1 | commit |
| Arbitration | D1, T2 | fileComplaint, respondToComplaint, settleComplaint, escalateToArbitration, resolveComplaint, withdrawComplaint |
| Escrow | D2 | lock, lockWithNonce, release, refund, releaseSplit, dispute, applyDecisionReleaseAll, applyDecisionRefundAll, applyDecisionPartialBps, setPause, scheduleExpiry, cancelExpiry |
| GroupRobotRegistry | G1, G5 | registerBot, updatePublicKey, approveMrtd, submitAttestation, bindCommunity, unbindCommunity, deactivateBot |
| GroupRobotConsensus | G2 | registerNode, markSequenceProcessed, verifyNodeTee, setTeeRewardParams, reportEquivocation, slashEquivocation, requestExit, finalizeExit |
| GroupRobotAds | G3 | setCommunityAdmin, stakeForAds, createCampaign, reviewCampaign, fundCampaign, submitDeliveryReceipt, settleEraAds, claimAdRevenue, pauseCampaign, cancelCampaign, advertiserBlockCommunity, communityBlockAdvertiser, slashCommunity |
| GroupRobotSubscription | G4 | subscribe, depositSubscription, changeTier, commitAds, cancelAdCommitment, cancelSubscription, cleanupSubscription, cleanupAdCommitment |
| GroupRobotCommunity | G5 | submitActionLog, batchSubmitLogs, setNodeRequirement, updateCommunityConfig, awardReputation, deductReputation, resetReputation, updateActiveMembers, clearExpiredLogs, cleanupExpiredCooldowns |
| GroupRobotCeremony | G6 | approveCeremonyEnclave, recordCeremony, forceReCeremony, revokeCeremony, cleanupCeremony, removeCeremonyEnclave |
| GroupRobotRewards | G7 | claimRewards, rescueStrandedRewards |
| StorageService | S1 | setBillingParams, joinOperator, updateOperator, fundUserAccount, requestPinForSubject, markPinned, chargeDue, operatorClaimRewards, pauseOperator, resumeOperator, distributeToOperators, slashOperator, leaveOperator |

### 7.2 仅 Cargo 覆盖 (E2E 未直接测试)

以下 pallet 在 `ALL_PALLETS` 中有 Cargo 测试，但无独立 E2E 流程直接调用:

| Pallet | 说明 |
|--------|------|
| CommissionCommon | 通用佣金工具库 (通过 E4 间接覆盖) |
| CommissionReferral | 推荐链佣金 (通过 E4 间接覆盖) |
| CommissionLevelDiff | 等级差佣金 (通过 E4 间接覆盖) |
| CommissionSingleLine | 单线佣金 (通过 E4 间接覆盖) |
| EntityReview | 评价 |

> **未纳入 `ALL_PALLETS` 的 crate** (纯类型/工具/虚拟根):
> pallet-entity-common, pallet-entity-commission, pallet-commission-multi-level, pallet-commission-pool-reward, pallet-commission-team,
> pallet-ads-core, pallet-ads-entity, pallet-ads-grouprobot, pallet-ads-primitives,
> pallet-trading-common, pallet-trading-trc20-verifier, pallet-storage-lifecycle,
> pallet-grouprobot-primitives, pallet-crypto-common

---

## 8. 角色矩阵

| 角色 | 使用账户 | 参与流程 | 典型操作 |
|------|----------|----------|----------|
| **Sudo/Root** | alice | 全部 | 审批, Slash, 参数设置, 紧急操作, Enclave 管理, 暂停/恢复 |
| **卖家 (NEX)** | bob | T2, T4 | 挂卖单, 取消订单, 接受买单 |
| **买家 (NEX)** | charlie | T4 | 挂买单, 预锁定卖单, 确认付款 |
| **实体所有者** | eve / bob | E1-E9 | 实体管理, 发币, 发售, 佣金配置, 市场管理, 披露管理 |
| **买家/会员** | bob | E2, E4, E7, D1, S1 | 下单, 认购, 投诉, Pin 文件 |
| **无权限用户** | charlie/dave | 全部 (错误路径) | 越权操作验证 |
| **Bot Owner** | bob | G1, G5, G6 | 注册/停用 Bot, TEE 证明, 绑定社区, 仪式记录 |
| **节点运营者** | bob | G2, G7 | 注册节点, 质押, 退出, 领取奖励 |
| **社区管理员** | charlie | G3, G5 | 质押, 投放收据, 提取收入, 行为日志 |
| **广告主** | bob | G3 | 创建/管理广告活动 |
| **订阅用户** | bob | G4 | 订阅, 充值, 变更层级, 取消 |
| **广告承诺用户** | charlie | G4 | 广告承诺订阅, 取消承诺 |
| **付款人/收款人** | bob / charlie | D2 | 锁定托管, 释放/退款 |
| **内幕人员** | charlie | E9 | 被添加/移除为内幕人员 |
| **存储运营者** | charlie | S1 | 加入/退出运营者 |
| **KYC Provider** | dave | E6 | 批准 KYC |
| **推荐人** | charlie | E3, E4 | 被绑定为推荐人 |
| **Token 持有者** | bob | E5, E8 | 创建提案, 投票, 市场交易 |
| **认购者** | bob, charlie | E7 | 认购代币, 领取/退款 |

---

## 9. 已知限制与待办

### 9.1 技术限制

| 限制 | 说明 | 缓解方式 |
|------|------|----------|
| **OCW 不可直接触发** | TRC20 验证、价格聚合等依赖 Off-chain Worker | Sudo 模拟确认 (`confirmVerification`) |
| **TEE 证明格式** | 软件模式 TEE 可能需特定 Quote 格式 | 记录结果而非 assert |
| **Era 边界依赖** | 某些操作需等待 Era 结束 | `waitBlocks()` + 短 Era 配置 |
| **出块等待** | 默认 6s/block，治理/发售流程等待时间长 | 使用 instant-seal 或 manual-seal |
| **状态隔离** | 流程间共享链状态可能互相影响 | 各流程使用独立 entityId/shopId/botId |
| **账户复用** | 多 Flow 复用 Bob/Charlie 等 dev 账户 | 建议使用 `createFlowAccounts()` 创建隔离账户 |
| **签名格式依赖** | G5/G6 等流程使用模拟签名，可能与链上验证逻辑不匹配 | 使用 mock 签名 + 记录而非 assert |

### 9.2 待实现项

| 优先级 | 项目 | 说明 |
|:------:|------|------|
| P0 | CI 集成 | GitHub Actions: PR 运行 P0 流程, nightly 全量 |
| P1 | EntityReview 流程 | 订单完成后评价 + 店铺评分 (仅剩未实现的 E2E 流程) |
| P1 | 流程隔离增强 | 各 Flow 使用 `createFlowAccounts()` 避免状态污染 |
| P1 | Flow 前置条件验证 | 每条流程开始前检查链状态是否满足前置条件 |
| P2 | 复合场景 | 新用户旅程, 商户入驻, 投资者流程 |
| P2 | 压力测试 | 并发订单, 批量注册 |
| P2 | HTML 报告 | 可视化测试报告 |
| P2 | 超时灵活配置 | 支持按 Flow 自定义 `txTimeout`，长流程使用更大超时 |

---

## 10. 统计汇总

| 指标 | 数值 |
|------|:----:|
| E2E 流程数 | 23 |
| Cargo 覆盖 Pallet 数 | 29 |
| E2E 直接覆盖 Pallet 数 | 26 |
| E2E 间接覆盖 Pallet 数 | 4 (CommissionCommon/Referral/LevelDiff/SingleLine) |
| 仅 Cargo 覆盖 Pallet 数 | 1 (EntityReview) |
| 未纳入 ALL_PALLETS 的 crate 数 | 14 |
| 覆盖 extrinsic 数 (E2E) | ~170+ |
| 错误路径测试数 | ~45+ |
| 业务领域 | 5 (Trading, Entity, Dispute, GroupRobot, Storage) |
| 角色类型 | 19 |
| 断言类型 | 9 |
| Coverage Map 映射用例总数 | 204 |
