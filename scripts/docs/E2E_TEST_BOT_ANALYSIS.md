# Nexus 全栈测试智能体 — 架构与实现文档

> 最后更新: 2026-03-03
> 范围: Cargo 单元测试 + E2E 链上流程测试 + 测试计划覆盖率追踪

---

## 1. 项目概述

Nexus 全栈测试智能体 (`nexus-test-agent`) 是一个统一协调 **Cargo 单元测试**、**E2E 链上测试** 和 **覆盖率追踪** 的测试框架。已从最初的可行性提案发展为完整实现，覆盖 5 大业务领域、15 条 E2E 流程、29 个 Cargo Pallet。

### 1.1 当前实现状态

| 组件 | 状态 | 说明 |
|------|:----:|------|
| 核心框架 (`core/`) | ✅ 已完成 | test-runner, assertions (9 种), reporter, chain-state, config, cargo-runner, coverage-tracker |
| 测试夹具 (`fixtures/`) | ✅ 已完成 | accounts.ts — 角色工厂 + 自动充值 |
| Trading 流程 (T1-T3) | ✅ 已实现 | 做市商、P2P Buy、P2P Sell |
| Entity 流程 (E1-E7) | ✅ 已实现 | 实体/店铺、订单、会员、佣金、Token+治理、KYC、代币发售 |
| Dispute 流程 (D1) | ✅ 已实现 | 证据+投诉+和解+仲裁 |
| GroupRobot 流程 (G1-G3) | ✅ 已实现 | Bot 生命周期、节点共识、广告活动 |
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
│  Phase 2: E2E 链上测试 (15 flows)                     │
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
│   └── accounts.ts                # Dev 账户 (alice..ferdie) + 充值
│
├── flows/                         # 15 条 E2E 流程
│   ├── trading/
│   │   ├── maker-lifecycle.ts     # T1: 做市商完整生命周期
│   │   ├── p2p-buy.ts             # T2: P2P 买入完整流程
│   │   └── p2p-sell.ts            # T3: P2P 卖出完整流程
│   │
│   ├── entity/
│   │   ├── entity-shop.ts         # E1: 实体+店铺创建/管理
│   │   ├── order-lifecycle.ts     # E2: 商品→订单→发货→收货
│   │   ├── member-referral.ts     # E3: 会员注册+推荐+等级
│   │   ├── commission.ts          # E4: 佣金计算+提现+回购
│   │   ├── token-governance.ts    # E5: 代币+治理提案+投票
│   │   ├── kyc.ts                 # E6: KYC 认证完整流程
│   │   └── token-sale.ts          # E7: 代币发售+认购+退款
│   │
│   ├── dispute/
│   │   └── dispute-resolution.ts  # D1: 证据+投诉+和解+仲裁
│   │
│   ├── grouprobot/
│   │   ├── bot-lifecycle.ts       # G1: Bot 注册→TEE→社区→停用
│   │   ├── node-consensus.ts      # G2: 节点+质押+Slash+退出
│   │   └── ad-campaign.ts         # G3: 广告创建→投放→结算
│   │
│   └── storage/
│       └── storage-service.ts     # S1: 运营者+Pin+扣费+Slash
│
├── run-e2e.ts                     # 独立 E2E 运行入口
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
| **Trading** | 1 | nex-market |
| **Dispute** | 3 | escrow, evidence, arbitration |
| **Storage** | 1 | storage-service |
| **GroupRobot** | 7 | grouprobot-registry, grouprobot-consensus, grouprobot-subscription, grouprobot-community, grouprobot-ceremony, grouprobot-rewards, grouprobot-ads |

> **注**: 以下 pallet 存在于代码库但未纳入 `ALL_PALLETS` (无独立 Cargo 测试或为纯类型 crate):
> pallet-entity-common, pallet-entity-commission (虚拟根 crate), pallet-commission-multi-level, pallet-commission-pool-reward, pallet-commission-team,
> pallet-ads-core, pallet-ads-entity, pallet-ads-grouprobot, pallet-ads-primitives,
> pallet-trading-common, pallet-trading-trc20-verifier, pallet-storage-lifecycle,
> pallet-grouprobot-primitives, pallet-crypto-common

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

### 3.8 fixtures/accounts.ts — 角色账户

| 账户 | 典型角色 |
|------|----------|
| `alice` | Sudo / Root 管理员 |
| `bob` | 做市商 / Bot Owner / 广告主 |
| `charlie` | 买家 / 无权限用户 / 社区管理员 |
| `dave` | 卖家 / KYC Provider / 无权限用户 |
| `eve` | 实体所有者 |
| `ferdie` | 额外测试账户 |

- `getDevAccounts()`: 返回开发链预置 6 个账户 (`//Alice` … `//Ferdie`)
- `createFlowAccounts(flowPrefix, roles)`: 为指定 Flow 创建隔离账户 (URI: `//{prefix}/{role}`)
- `fundAccounts(api, accounts, amountNex=100_000)`: 从 Alice 批量转账 NEX 给目标账户

---

## 4. E2E 流程详解

### 4.1 Trading 领域

#### Flow-T1: 做市商完整生命周期 (`maker-lifecycle.ts`)

**角色**: Bob (做市商申请人), Charlie (无权限用户), Alice (Sudo/审批)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | 查询 Bob 初始状态 (不应是 Maker) | `accountToMaker` 查询 |
| 2 | Bob 锁定押金 (`tradingMaker.lockDeposit`) | `assertTxSuccess` + `assertEventEmitted(DepositLocked)` + 验证状态 `DepositLocked` |
| 3 | Bob 提交申请信息 (`tradingMaker.submitInfo`) | `assertTxSuccess` + 验证状态 `PendingReview` |
| 4 | Alice Sudo 审批 (`tradingMaker.approveMaker`) | `assertTxSuccess` |
| 5 | 验证做市商状态为 Active | `assertStorageField(status, Active)` |
| 6 | **[错误]** 重复锁定押金 | `assertTxFailed` |
| 7 | **[错误]** Charlie (非做市商) 取消做市商 (`cancelMaker`) | `assertTxFailed` |
| 8 | 查询做市商详细信息 | `assertTrue` + 打印 ID/状态/押金 |

**涉及 Pallet**: TradingMaker, Balances

---

#### Flow-T2: P2P Buy 完整流程 (`p2p-buy.ts`)

**角色**: Bob (做市商), Charlie (买家), Alice (Sudo)

**前置条件**: Bob 已是 Active 做市商 (依赖 Flow-T1)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | 确认 Bob 做市商状态 + 查询 Charlie 余额 | `assertTrue(isSome)` + `assertEqual(Active)` |
| 2 | Charlie 创建 Buy 订单 (`tradingP2p.createBuyOrder`, 100 NEX) | `assertTxSuccess` + `assertEventEmitted(BuyOrderCreated)` |
| 3 | 验证订单状态 = Created | `assertStorageField(state, Created)` |
| 4 | Charlie 标记已付款 (`tradingP2p.markPaid`) | `assertTxSuccess` + 验证状态 `Paid` |
| 5 | Bob 释放 NEX (`tradingP2p.releaseNex`) | `assertTxSuccess` |
| 6 | 验证订单 Released + Charlie 收到约 100 NEX | `assertStorageField(state, Released)` + `assertTrue(delta > 90 NEX)` |
| 7 | **[错误]** 重复释放 | `assertTxFailed` |
| 8 | **[分支]** 创建新订单 → Charlie 取消 (`cancelBuyOrder`) | `assertTxSuccess` + 验证状态 `Cancelled` |
| 9 | **[分支]** 创建新订单 → 付款 → Charlie 争议 (`disputeBuyOrder`) | `assertTxSuccess` + 验证状态 `Disputed` |

**涉及 Pallet**: TradingP2p, TradingMaker, Balances

---

#### Flow-T3: P2P Sell 完整流程 (`p2p-sell.ts`)

**角色**: Bob (做市商), Dave (卖家), Charlie (无权限用户), Alice (Sudo/验证确认)

**前置条件**: Bob 已是 Active 做市商 (依赖 Flow-T1)

| Step | 操作 | 断言 |
|:----:|------|------|
| 1 | Dave 创建 Sell 订单 (`tradingP2p.createSellOrder`, 200 NEX) | `assertTxSuccess` |
| 2 | 验证订单已创建 + Dave NEX 被锁定 (减少约 200 NEX) | `assertTrue(isSome)` + `assertTrue(delta > 199 NEX)` |
| 3 | Bob 提交 TRC20 交易哈希 (`tradingP2p.markSellComplete`) | `assertTxSuccess` |
| 4 | Sudo 确认验证 (`tradingP2p.confirmSellVerification`) | `assertTxSuccess` (若 VerificationOrigin 非 Sudo 则记录跳过) |
| 5 | 验证 Sell 订单最终状态 | Storage 检查 |
| 6 | **[分支]** 创建新订单 → Dave 举报 (`tradingP2p.reportSell`) | `assertTxSuccess` (可能需等待超时) |
| 7 | **[错误]** Charlie (非卖家) 举报 | `assertTxFailed` |

**涉及 Pallet**: TradingP2p, TradingMaker, Balances

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
npx tsx scripts/e2e/nexus-test-agent.ts --pallet pallet-escrow pallet-entity-token

# 按优先级过滤覆盖率报告
npx tsx scripts/e2e/nexus-test-agent.ts --priority P0

# 输出目录 (默认 ./e2e-reports)
npx tsx scripts/e2e/nexus-test-agent.ts --report-dir ./reports

# 详细日志
npx tsx scripts/e2e/nexus-test-agent.ts --verbose

# 显示帮助
npx tsx scripts/e2e/nexus-test-agent.ts --help
```

### 5.2 run-e2e.ts — 独立 E2E 运行 (Phase 1)

仅包含 Phase 1 流程 (T1-T3, E1)，使用 `--flow` 标志:

```bash
# 运行全部 Phase 1 流程
npx tsx scripts/e2e/run-e2e.ts

# 运行指定流程 (--flow 标志)
npx tsx scripts/e2e/run-e2e.ts --flow T1 T2 E1
```

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
| E1 | 实体+店铺 | entity | `flows/entity/entity-shop.ts` |
| E2 | 订单生命周期 | entity | `flows/entity/order-lifecycle.ts` |
| E3 | 会员+推荐 | entity | `flows/entity/member-referral.ts` |
| E4 | 佣金+提现 | entity | `flows/entity/commission.ts` |
| E5 | Token+治理 | entity | `flows/entity/token-governance.ts` |
| E6 | KYC 认证 | entity | `flows/entity/kyc.ts` |
| E7 | 代币发售 | entity | `flows/entity/token-sale.ts` |
| D1 | 争议解决 | dispute | `flows/dispute/dispute-resolution.ts` |
| G1 | Bot 生命周期 | grouprobot | `flows/grouprobot/bot-lifecycle.ts` |
| G2 | 节点共识 | grouprobot | `flows/grouprobot/node-consensus.ts` |
| G3 | 广告活动 | grouprobot | `flows/grouprobot/ad-campaign.ts` |
| S1 | 存储服务 | storage | `flows/storage/storage-service.ts` |

---

## 7. Pallet 覆盖矩阵

### 7.1 E2E 流程覆盖的 Pallet

| Pallet | E2E 流程 | 覆盖 extrinsics |
|--------|:--------:|-----------------|
| TradingMaker | T1 | lockDeposit, submitInfo, approveMaker, cancelMaker |
| TradingP2p | T2, T3 | createBuyOrder, createSellOrder, markPaid, releaseNex, cancelBuyOrder, disputeBuyOrder, markSellComplete, confirmSellVerification, reportSell |
| EntityRegistry | E1-E7 | createEntity, updateEntity, suspendEntity, resumeEntity, requestCloseEntity, approveCloseEntity |
| EntityShop | E1, E2, E5, E7 | (自动创建 + 查询) |
| EntityProduct | E2 | createProduct, publishProduct |
| EntityTransaction | E2, E4 | placeOrder, shipOrder, confirmReceipt, cancelOrder |
| EntityToken | E5, E7 | createShopToken, mintTokens, transferTokens, setTransferRestriction, addToWhitelist, lockTokens, unlockTokens |
| EntityGovernance | E5 | configureGovernance, createProposal, vote, finalizeVoting, executeProposal |
| EntityMember | E3, E4 | registerMember, bindReferrer, initCustomLevels, manualUpgrade |
| CommissionCore | E4 | initCommissionPlan, setMode, enableCommission, setWithdrawalConfig |
| EntityKyc | E6 | registerProvider, submitKyc, approveKyc, setEntityRequirement, revokeKyc, updateHighRiskCountries |
| EntityTokenSale | E7 | createSaleRound, addPaymentOption, setVestingConfig, startSale, subscribe, endSale, claimTokens, withdrawFunds, cancelSale, claimRefund |
| Evidence | D1 | commit |
| Arbitration | D1, T2 | fileComplaint, respondToComplaint, settleComplaint, escalateToArbitration, resolveComplaint, withdrawComplaint |
| GroupRobotRegistry | G1 | registerBot, updatePublicKey, approveMrtd, submitAttestation, bindCommunity, unbindCommunity, deactivateBot |
| GroupRobotConsensus | G2 | registerNode, markSequenceProcessed, verifyNodeTee, setTeeRewardParams, reportEquivocation, slashEquivocation, requestExit, finalizeExit |
| GroupRobotAds | G3 | setCommunityAdmin, stakeForAds, createCampaign, reviewCampaign, fundCampaign, submitDeliveryReceipt, settleEraAds, claimAdRevenue, pauseCampaign, cancelCampaign, advertiserBlockCommunity, communityBlockAdvertiser, slashCommunity |
| StorageService | S1 | setBillingParams, joinOperator, updateOperator, fundUserAccount, requestPinForSubject, markPinned, chargeDue, operatorClaimRewards, pauseOperator, resumeOperator, distributeToOperators, slashOperator, leaveOperator |

### 7.2 仅 Cargo 覆盖 (E2E 未直接测试)

以下 pallet 在 `ALL_PALLETS` 中有 Cargo 测试，但无独立 E2E 流程直接调用:

| Pallet | 说明 |
|--------|------|
| CommissionCommon | 通用佣金工具库 (通过 E4 间接覆盖) |
| CommissionReferral | 推荐链佣金 (通过 E4 间接覆盖) |
| CommissionLevelDiff | 等级差佣金 (通过 E4 间接覆盖) |
| CommissionSingleLine | 单线佣金 (通过 E4 间接覆盖) |
| GroupRobotSubscription | 订阅结算 (通过 G2 Era 结算间接覆盖) |
| GroupRobotCommunity | 社区配置/动作日志 |
| GroupRobotCeremony | Shamir 仪式 |
| GroupRobotRewards | 奖励分配 |
| NexMarket | 交易对市场 |
| EntityMarket | 代币市场交易 |
| EntityDisclosure | 财务披露 |
| EntityReview | 评价 |
| Escrow | 资金托管 (通过 T2/T3/D1 间接覆盖) |

> **未纳入 `ALL_PALLETS` 的 crate** (纯类型/工具/虚拟根):
> pallet-entity-common, pallet-entity-commission, pallet-commission-multi-level, pallet-commission-pool-reward, pallet-commission-team,
> pallet-ads-core, pallet-ads-entity, pallet-ads-grouprobot, pallet-ads-primitives,
> pallet-trading-common, pallet-trading-trc20-verifier, pallet-storage-lifecycle,
> pallet-grouprobot-primitives, pallet-crypto-common

---

## 8. 角色矩阵

| 角色 | 使用账户 | 参与流程 | 典型操作 |
|------|----------|----------|----------|
| **Sudo/Root** | alice | 全部 | 审批, Slash, 参数设置, 紧急操作 |
| **做市商** | bob | T1, T2, T3 | 锁定押金, 提交信息, 释放 NEX, 提交 TRC20 哈希 |
| **买家** | charlie | T2 | 创建 Buy 订单, 标记付款, 取消, 争议 |
| **卖家** | dave | T3 | 创建 Sell 订单, 举报 |
| **实体所有者** | eve | E1-E7 | 实体管理, 发币, 发售, 佣金配置 |
| **买家/会员** | bob | E2, E4, E7, D1, S1 | 下单, 认购, 投诉, Pin 文件 |
| **无权限用户** | charlie/dave | 全部 (错误路径) | 越权操作验证 |
| **Bot Owner** | bob | G1 | 注册/停用 Bot, TEE 证明, 绑定社区 |
| **节点运营者** | bob | G2 | 注册节点, 质押, 退出 |
| **社区管理员** | charlie | G3 | 质押, 投放收据, 提取收入 |
| **广告主** | bob | G3 | 创建/管理广告活动 |
| **存储运营者** | charlie | S1 | 加入/退出运营者 |
| **KYC Provider** | dave | E6 | 批准 KYC |
| **推荐人** | charlie | E3, E4 | 被绑定为推荐人 |
| **Token 持有者** | bob | E5 | 创建提案, 投票 |
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
| **状态隔离** | 流程间共享链状态可能互相影响 | 各流程使用独立 entityId/shopId |

### 9.2 待实现项

| 优先级 | 项目 | 说明 |
|:------:|------|------|
| P0 | CI 集成 | GitHub Actions: PR 运行 P0 流程, nightly 全量 |
| P1 | EntityReview 流程 | 订单完成后评价 + 店铺评分 |
| P1 | EntityMarket 流程 | 代币市场挂单/成交 |
| P1 | EntityDisclosure 流程 | 财务披露 + 黑出期 |
| P1 | Community 流程 | 社区配置 + 动作日志 |
| P1 | Ceremony 流程 | Shamir 仪式 + 过期处理 |
| P2 | 复合场景 | 新用户旅程, 商户入驻, 投资者流程 |
| P2 | 压力测试 | 并发订单, 批量注册 |
| P2 | HTML 报告 | 可视化测试报告 |

---

## 10. 统计汇总

| 指标 | 数值 |
|------|:----:|
| E2E 流程数 | 15 |
| Cargo 覆盖 Pallet 数 | 29 |
| E2E 直接覆盖 Pallet 数 | 18 |
| E2E 间接覆盖 Pallet 数 | 6 |
| 仅 Cargo 覆盖 Pallet 数 | 7 |
| 未纳入 ALL_PALLETS 的 crate 数 | 14 |
| 覆盖 extrinsic 数 (E2E) | ~90+ |
| 错误路径测试数 | ~30+ |
| 业务领域 | 5 (Trading, Entity, Dispute, GroupRobot, Storage) |
| 角色类型 | 16 |
| 断言类型 | 9 |
