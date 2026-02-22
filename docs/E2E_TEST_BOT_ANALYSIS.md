# Nexus 用户视角全流程 E2E 测试机器人 — 可行性与合理性分析

> 日期: 2026-02-22
> 范围: 全链上 Pallet + 离链 GroupRobot 的用户功能流程测试

---

## 1. 现状评估

### 1.1 已有测试基础设施

| 项目 | 位置 | 说明 |
|------|------|------|
| 单元测试 (Rust) | 各 pallet `src/tests.rs` | ~500+ 个 pallet 级单元测试 |
| 链上功能脚本 | `scripts/` | 10 个 TypeScript 脚本 (polkadot-js) |
| 测试工具库 | `scripts/utils/` | api.ts, accounts.ts, helpers.ts |
| 测试运行器 | `scripts/test-all.ts` | 串行执行所有脚本并汇总 |

### 1.2 已有脚本覆盖范围

| 测试脚本 | 覆盖模块 | 状态 |
|----------|----------|------|
| test-pricing.ts | TradingPricing (50) | ✅ 查询接口 |
| test-cny-rate.ts | TradingPricing (50) | ✅ CNY/USDT 汇率 |
| test-maker.ts | TradingMaker (52) | ✅ 做市商申请→审批 |
| test-otc.ts | ~~TradingOtc (53)~~ | ⚠️ 已废弃 (OTC 已合并到 P2P) |
| test-swap.ts | ~~TradingSwap (54)~~ | ⚠️ 已废弃 (Swap 已合并到 P2P) |
| test-referral.ts | AffiliateReferral | ✅ 推荐码+绑定 |
| test-credit.ts | TradingCredit (51) | ✅ 查询 |
| test-escrow.ts | Escrow (60) | ✅ 查询 |
| test-arbitration.ts | Arbitration (64) | ⚠️ 部分 |
| test-chat.ts | Chat 模块 | ⚠️ 部分 |

### 1.3 关键覆盖缺口

**完全未覆盖的 Pallet (17 个):**

| 领域 | 未覆盖 Pallet | pallet_index |
|------|--------------|:------------:|
| **Trading** | TradingP2p (替代 OTC+Swap) | 55 |
| **Entity** | EntityRegistry | 120 |
| | EntityShop | 129 |
| | EntityService | 121 |
| | EntityTransaction | 122 |
| | EntityReview | 123 |
| | EntityToken | 124 |
| | EntityGovernance | 125 |
| | EntityMember | 126 |
| | CommissionCore / Referral / LevelDiff / SingleLine | 127,133-135 |
| | EntityMarket | 128 |
| | EntityDisclosure | 130 |
| | EntityKyc | 131 |
| | EntityTokenSale | 132 |
| **GroupRobot** | Registry / Consensus / Community / Ceremony | 150-153 |
| **Storage** | StorageLifecycle | 65 |
| **治理** | 4 个委员会 (Technical/Arbitration/Treasury/Content) | 70-77 |

**未覆盖的跨模块流程:** 实体创建→店铺→商品→交易→评价→佣金→治理 完整生命周期

---

## 2. "用户视角"测试的定义

### 2.1 测试哲学

与 pallet 单元测试不同，用户视角 E2E 测试关注的是：

```
用户看到什么 → 用户做什么 → 系统响应什么 → 用户状态变成什么
```

核心原则：
- **角色驱动**: 每个测试从具体用户角色出发 (买家、卖家、做市商、实体所有者、节点运营者等)
- **流程连贯**: 模拟完整业务流程 (不是孤立调用单个 extrinsic)
- **状态断言**: 每步操作后验证链上状态变化
- **错误路径**: 模拟用户常见误操作 (余额不足、权限不够、重复操作等)
- **跨模块联动**: 验证模块间数据传递正确性

### 2.2 用户角色矩阵

| 角色 | 涉及模块 | 典型操作 |
|------|----------|----------|
| **普通用户** | Balances, P2P, Referral | 转账、买 NEX、绑定推荐人 |
| **做市商** | Maker, P2P, Credit | 申请、接单、释放、管理押金 |
| **实体所有者** | EntityRegistry, Shop, Token, Governance | 创建实体、开店、发币、治理 |
| **店铺管理员** | Shop, Service, Transaction | 管理商品、处理订单 |
| **会员/消费者** | Member, Transaction, Review, Commission | 注册会员、下单、评价、提佣 |
| **KYC 审核员** | EntityKyc | 审核 KYC 申请 |
| **投资者** | TokenSale, Market, Disclosure | 参与发售、市场交易、查看披露 |
| **仲裁员** | Arbitration, Evidence | 处理争议、提交证据、裁决 |
| **节点运营者** | GR-Consensus, GR-Registry | 注册节点、质押、领取奖励 |
| **Bot 所有者** | GR-Registry, GR-Community | 注册 Bot、绑定社区、配置群规则 |
| **治理委员** | Collective 委员会 | 提案、投票 |
| **Root 管理员** | Sudo, 全部 | 审批、参数调整、紧急操作 |

---

## 3. 全流程测试范围映射

### 3.1 Trading 领域 (4 条主线)

#### Flow-T1: 做市商完整生命周期
```
锁定押金 → 提交信息 → Root审批 → 激活
→ 接 Buy 订单 → 接 Sell 订单 → 管理信用分
→ 申请提现 → 冷却期 → 执行提现
→ 补充押金 → 暂停服务 → 恢复服务
```
**涉及**: TradingMaker(52), TradingCredit(51), Balances(4)

#### Flow-T2: P2P Buy 完整流程 (用户买 NEX)
```
买家创建订单 → 托管锁定 → 买家法币付款 → 标记已付款
→ 做市商确认 → 释放 NEX → 买家收到 NEX
  [分支] → 买家取消 → 托管退还
  [分支] → 超时 → 自动退还
  [分支] → 争议 → 仲裁 → 裁决
```
**涉及**: TradingP2p(55), Escrow(60), Arbitration(64), Balances(4)

#### Flow-T3: P2P Sell 完整流程 (用户卖 NEX)
```
卖家创建兑换 → NEX 锁定 → 做市商 USDT 转账
→ 提交 TRC20 哈希 → OCW 验证 → 释放 NEX 给做市商
  [分支] → 验证失败 → 重试 / 退还
  [分支] → 做市商超时 → 用户举报 → NEX 退还
```
**涉及**: TradingP2p(55), Escrow(60), TRC20-Verifier

#### Flow-T4: 价格发现
```
查询默认价格 → OTC 交易成交 → 价格聚合更新
→ 冷启动退出 → TWAP 计算
→ CNY/USDT 汇率同步
```
**涉及**: TradingPricing(50)

### 3.2 Entity 领域 (6 条主线)

#### Flow-E1: 实体→店铺创建流程
```
创建实体(Merchant) → 自动创建主店铺 → 实体审批(Root)
→ 创建额外店铺 → 设置店铺运营资金
→ 实体暂停 → 所有店铺 effective 状态变为 PausedByEntity
→ 实体恢复 → 店铺恢复
```
**涉及**: EntityRegistry(120), EntityShop(129)

#### Flow-E2: 商品→订单→评价流程
```
创建商品/服务 → 设置价格 → 上架
→ 会员下单 → 支付(NEX/代币) → 确认收货
→ 提交评价 → 店铺评分更新
→ 取消订单 → 退款
```
**涉及**: EntityService(121), EntityTransaction(122), EntityReview(123), Escrow(60)

#### Flow-E3: 代币发行→市场交易
```
创建实体代币(Governance) → 配置转账限制(KycRequired)
→ 铸造代币 → 分发给会员
→ 发起代币发售(固定价格) → 用户参与
→ 市场挂单(Sell) → 他人接单(Buy) → 成交
→ TWAP 更新 → 配置价格保护
  [分支] → 触发熔断 → 恢复
```
**涉及**: EntityToken(124), EntityTokenSale(132), EntityMarket(128), EntityKyc(131)

#### Flow-E4: 会员→佣金→提现
```
注册会员 → 绑定推荐人 → 会员等级初始化
→ 消费下单 → 触发佣金计算(多模式)
→ 推荐链佣金分发 → 等级差佣金 → 单线佣金
→ 查询佣金余额 → 申请提现(含回购)
→ 会员升级(消费触发) → 等级变化事件
```
**涉及**: EntityMember(126), CommissionCore(127), CommissionReferral(133), CommissionLevelDiff(134), CommissionSingleLine(135)

#### Flow-E5: 治理全流程
```
配置治理模式(DualTrack) → 设置分层阈值
→ 成员创建提案 → 投票(时间加权)
→ 投票期结束 → 自动执行/通过
  [分支] → 管理员否决
  [分支] → 委员会提案
```
**涉及**: EntityGovernance(125), EntityToken(124)

#### Flow-E6: 合规流程
```
申请 KYC(Basic) → 审核通过 → 升级到 Standard
→ 发布财务披露 → 进入黑出期 → 内幕人员登记
→ 黑出期结束 → 恢复交易权限
  [分支] → 高风险国家拦截
```
**涉及**: EntityKyc(131), EntityDisclosure(130)

### 3.3 GroupRobot 领域 (3 条主线)

#### Flow-G1: Bot 注册→节点运营
```
注册 Bot → 提交 TEE 证明(MRTD/MRENCLAVE)
→ Root 审批 MRTD → 证明验证通过
→ 注册节点 → 质押 → 节点激活
→ 订阅服务(Pro) → 存入订阅金
→ Era 结束 → 奖励分配(TEE 加权)
→ 领取奖励
  [分支] → 证明过期 → 刷新
  [分支] → 节点下线 → 退出冷却期 → 退出
```
**涉及**: GR-Registry(150), GR-Consensus(151)

#### Flow-G2: 社区配置→动作日志
```
绑定 Bot 到社区 → 配置群规则(CAS 版本)
→ 提交动作日志 → 批量提交
→ 设置节点需求(TeeOnly)
→ 清理过期日志
```
**涉及**: GR-Community(152), GR-Registry(150)

#### Flow-G3: Ceremony 流程
```
审批 Enclave → 记录 Ceremony(Shamir 参数)
→ 查询活跃 Ceremony → 标记序列已处理
  [分支] → Ceremony 过期 → 强制重新 Ceremony
  [分支] → 撤销 Ceremony
```
**涉及**: GR-Ceremony(153), GR-Consensus(151)

### 3.4 争议解决领域 (1 条主线)

#### Flow-D1: 仲裁全流程
```
交易争议 → 发起仲裁 → 提交证据(IPFS CID)
→ 对方提交证据 → 仲裁员查看
→ 仲裁裁决(Release/Refund/Partial)
→ 执行裁决 → 资金释放/退还
```
**涉及**: Arbitration(64), Evidence(63), Escrow(60), StorageService(62)

### 3.5 存储领域 (1 条主线)

#### Flow-S1: IPFS 存储生命周期
```
PIN CID → 查询 PIN 状态 → 生命周期续期
→ Unpin → 确认删除
```
**涉及**: StorageService(62), StorageLifecycle(65)

---

## 4. 技术架构设计

### 4.1 推荐架构

```
scripts/
├── e2e/                           # E2E 测试根目录
│   ├── core/                      # 核心框架
│   │   ├── test-runner.ts         # 测试运行器 (并行/串行控制)
│   │   ├── chain-state.ts         # 链状态快照/恢复
│   │   ├── assertions.ts          # 链上状态断言库
│   │   ├── reporter.ts            # 测试报告生成 (JSON/HTML)
│   │   └── config.ts              # 测试环境配置
│   │
│   ├── fixtures/                  # 测试夹具 (预置数据)
│   │   ├── accounts.ts            # 角色账户工厂
│   │   ├── entity-factory.ts      # 实体/店铺预创建
│   │   └── maker-factory.ts       # 做市商预创建
│   │
│   ├── flows/                     # 按用户流程组织
│   │   ├── trading/
│   │   │   ├── maker-lifecycle.ts     # Flow-T1
│   │   │   ├── p2p-buy.ts            # Flow-T2
│   │   │   ├── p2p-sell.ts           # Flow-T3
│   │   │   └── price-discovery.ts    # Flow-T4
│   │   │
│   │   ├── entity/
│   │   │   ├── entity-shop.ts        # Flow-E1
│   │   │   ├── order-review.ts       # Flow-E2
│   │   │   ├── token-market.ts       # Flow-E3
│   │   │   ├── member-commission.ts  # Flow-E4
│   │   │   ├── governance.ts         # Flow-E5
│   │   │   └── compliance.ts         # Flow-E6
│   │   │
│   │   ├── grouprobot/
│   │   │   ├── bot-node.ts           # Flow-G1
│   │   │   ├── community-config.ts   # Flow-G2
│   │   │   └── ceremony.ts           # Flow-G3
│   │   │
│   │   ├── dispute/
│   │   │   └── arbitration.ts        # Flow-D1
│   │   │
│   │   └── storage/
│   │       └── ipfs-lifecycle.ts     # Flow-S1
│   │
│   ├── scenarios/                 # 复合场景 (跨领域)
│   │   ├── new-user-journey.ts    # 新用户: 注册→首购→绑定推荐人→成为会员
│   │   ├── merchant-onboarding.ts # 商户入驻: 创建实体→开店→上架→首单
│   │   ├── investor-flow.ts       # 投资者: KYC→参与发售→市场交易→分红
│   │   └── full-commerce.ts       # 完整商业: 建立实体→运营→治理→退出
│   │
│   └── stress/                    # 压力测试 (可选)
│       ├── concurrent-orders.ts   # 并发订单
│       └── mass-registration.ts   # 批量注册
│
├── utils/                         # 现有工具 (复用)
│   ├── api.ts
│   ├── accounts.ts
│   └── helpers.ts
│
└── package.json                   # 更新依赖
```

### 4.2 核心框架设计

#### 测试用例结构

```typescript
interface E2ETestFlow {
  name: string;
  description: string;
  roles: string[];             // 涉及角色
  pallets: string[];           // 涉及 pallet
  prerequisites: string[];     // 前置流程
  steps: E2EStep[];
  cleanup?: () => Promise<void>;
}

interface E2EStep {
  name: string;
  actor: string;               // 执行角色
  action: () => Promise<any>;  // 链上操作
  assertions: Assertion[];     // 状态断言
  errorPath?: {                // 错误路径测试
    action: () => Promise<any>;
    expectedError: string;
  };
}
```

#### 链上断言库

```typescript
// 余额变化断言
await assertBalanceChange(account, expectedDelta);

// 存储值断言
await assertStorageValue('tradingP2p', 'buyOrders', [orderId], expected);

// 事件断言
await assertEventEmitted('TradingP2p', 'OrderCreated', { orderId, buyer });

// 状态机断言
await assertStateTransition('tradingP2p', 'buyOrders', orderId, 'state', 'Paid');
```

### 4.3 技术栈选择

| 组件 | 选择 | 理由 |
|------|------|------|
| 语言 | TypeScript | 复用现有 scripts/utils，polkadot-js 生态成熟 |
| 链交互 | @polkadot/api v12 | 已在用，metadata 自动生成类型 |
| 测试框架 | vitest | 比 jest 更快，原生 ESM，TypeScript 零配置 |
| 报告 | 自定义 JSON + HTML | 链上测试需要自定义格式 |
| CI | GitHub Actions | 已有 .github/workflows/ |

---

## 5. 可行性分析

### 5.1 技术可行性 — ✅ 高

| 维度 | 评估 | 说明 |
|------|:----:|------|
| **链交互能力** | ✅ | polkadot-js API 可调用任意 extrinsic + 查询任意 storage |
| **类型安全** | ✅ | metadata v15 自动生成类型，无需手写 codec |
| **状态断言** | ✅ | 可读取任意 storage item 进行精确断言 |
| **事件监听** | ✅ | 可订阅 block events 验证事件发射 |
| **角色模拟** | ✅ | Keyring 支持任意数量测试账户 |
| **Sudo 调用** | ✅ | 开发节点 Alice = Sudo，可模拟管理员操作 |
| **OCW 模拟** | ⚠️ | Off-chain Worker 无法直接触发，需 Sudo 模拟确认 |
| **时间推进** | ⚠️ | 开发节点 6s/block，Era/冷却期需调整参数或等待 |
| **并发测试** | ⚠️ | 需要 nonce 管理，避免交易冲突 |

**OCW 限制的解决方案:**
- TRC20 验证 → Sudo 调用 `confirmVerification` 模拟
- 价格聚合 → 直接查询 storage 验证中间状态
- 证明过期扫描 → 等待足够 block 或 sudo 设置区块号

**时间推进的解决方案:**
- 开发节点使用较短的 Era/冷却期参数
- 或使用 `api.rpc.engine.createBlock()` 快速出块 (需 manual-seal)

### 5.2 工程可行性 — ✅ 中高

| 维度 | 评估 | 说明 |
|------|:----:|------|
| **现有基础** | ✅ | scripts/utils/ 工具库可直接复用 |
| **模块独立性** | ✅ | 每条流程可独立开发和测试 |
| **增量交付** | ✅ | 按优先级逐步覆盖，无需一次全部完成 |
| **维护成本** | ⚠️ | pallet 接口变更需同步更新测试 |
| **环境依赖** | ⚠️ | 需要运行本地 dev 节点 |

### 5.3 资源评估

| 流程 | 预计复杂度 | 开发工时 | 优先级 |
|------|:----------:|:--------:|:------:|
| **核心框架** (runner/assertions/reporter) | 中 | 2天 | P0 |
| **Flow-T1** 做市商生命周期 | 低 | 0.5天 | P0 |
| **Flow-T2** P2P Buy | 中 | 1天 | P0 |
| **Flow-T3** P2P Sell | 中 | 1天 | P0 |
| **Flow-T4** 价格发现 | 低 | 0.5天 | P1 |
| **Flow-E1** 实体→店铺 | 中 | 1天 | P0 |
| **Flow-E2** 订单→评价 | 高 | 1.5天 | P1 |
| **Flow-E3** 代币→市场 | 高 | 2天 | P1 |
| **Flow-E4** 会员→佣金 | 高 | 2天 | P1 |
| **Flow-E5** 治理 | 中 | 1天 | P2 |
| **Flow-E6** 合规 | 中 | 1天 | P2 |
| **Flow-G1** Bot→节点 | 中 | 1天 | P1 |
| **Flow-G2** 社区配置 | 低 | 0.5天 | P1 |
| **Flow-G3** Ceremony | 中 | 0.5天 | P2 |
| **Flow-D1** 仲裁 | 中 | 1天 | P1 |
| **Flow-S1** 存储 | 低 | 0.5天 | P2 |
| **复合场景** (4个) | 高 | 3天 | P2 |
| **CI 集成** | 中 | 1天 | P1 |
| **合计** | — | **~21天** | — |

### 5.4 按优先级分批交付

| 批次 | 内容 | 工时 | 覆盖 Pallet 数 |
|:----:|------|:----:|:--------------:|
| **Phase 1** | 框架 + Trading 流程 (T1-T3) + 实体基础 (E1) | ~5.5天 | 7 |
| **Phase 2** | Entity 进阶 (E2-E4) + GroupRobot (G1-G2) + 仲裁 | ~7天 | 18 |
| **Phase 3** | 合规 (E5-E6) + Ceremony + 存储 + 复合场景 + CI | ~8.5天 | 全部 ~30 |

---

## 6. 合理性分析

### 6.1 ROI 评估

#### 直接收益

| 收益 | 说明 |
|------|------|
| **回归测试自动化** | 每次 pallet 修改后自动验证全流程，替代手动测试 |
| **接口兼容性守护** | 检测 extrinsic 签名、storage 格式变更导致的前端/离链破坏 |
| **跨模块 Bug 发现** | 单元测试无法覆盖的模块间状态传递错误 |
| **文档即代码** | 测试流程本身就是最准确的"用户操作手册" |
| **CI/CD 门禁** | merge 前自动验证，避免带 Bug 上线 |

#### 间接收益

| 收益 | 说明 |
|------|------|
| **开发信心** | 重构 pallet 后一键验证全流程无回归 |
| **前端开发加速** | 前端团队可参考测试脚本理解 API 调用方式和参数 |
| **审计辅助** | 安全审计方可以运行 E2E 测试验证修复 |
| **新成员 onboarding** | 新开发者通过测试脚本快速理解业务流程 |

### 6.2 风险与代价

| 风险 | 严重性 | 缓解措施 |
|------|:------:|----------|
| **维护成本** | 中 | pallet 接口变更时需同步更新测试。→ 框架层抽象调用，减少单点修改面 |
| **假阳性** | 低 | 链上状态依赖执行顺序。→ 每次测试前重置 dev 节点或使用独立账户 |
| **测试速度** | 中 | 链上交易需等待 finality (6s/block)。→ 使用 instant-seal 模式或 batch 交易 |
| **OCW 不可控** | 低 | 部分流程依赖 OCW。→ Sudo 模拟替代 |
| **环境隔离** | 低 | 测试间状态污染。→ 每个 flow 使用独立账户命名空间 |

### 6.3 与现有测试的关系

```
┌─────────────────────────────────────────┐
│          E2E 测试 (用户视角)              │  ← 新建
│  完整流程 · 跨模块 · 多角色 · 状态断言    │
├─────────────────────────────────────────┤
│       现有 scripts/ (功能验证)            │  ← 逐步迁移/替代
│  单模块 · 单步骤 · 手动验证               │
├─────────────────────────────────────────┤
│       Rust 单元测试 (pallet 内部)         │  ← 保留
│  Mock runtime · 边界条件 · 错误路径       │
└─────────────────────────────────────────┘
```

- **Rust 单元测试**: 保留，继续覆盖 pallet 内部逻辑和边界条件
- **现有 scripts/**: 逐步迁移到 E2E 框架中 (test-otc.ts, test-swap.ts 已废弃，优先迁移)
- **E2E 测试**: 新建，覆盖用户完整操作路径和跨模块联动

### 6.4 同类项目对标

| 项目 | E2E 测试方案 | 覆盖范围 |
|------|-------------|----------|
| Acala | chopsticks + polkadot-js | DEX/Staking/Governance 流程 |
| Moonbeam | ethers.js E2E | EVM 合约 + Substrate 桥接 |
| Astar | subxt + TypeScript | dApps Staking 流程 |
| **Nexus (当前)** | 10 个零散脚本 | ~30% 模块覆盖 |
| **Nexus (目标)** | 结构化 E2E 框架 | ~95% 模块 + 跨模块 |

---

## 7. 建议方案

### 7.1 推荐: 分批增量实施

**不建议**一次性开发全部 15+ 条流程。推荐按 Phase 1→2→3 分批交付：

- **Phase 1 (5.5天)**: 框架 + Trading + 实体基础 — 立即验证 P2P 替代 OTC/Swap 后的正确性
- **Phase 2 (7天)**: Entity 进阶 + GroupRobot — 覆盖主营业务闭环
- **Phase 3 (8.5天)**: 合规 + 复合场景 + CI — 完善覆盖率，集成到 CI pipeline

### 7.2 关键决策点

| 决策 | 推荐 | 理由 |
|------|------|------|
| **测试框架** | vitest | 原生 ESM，零配置 TS，并行执行 |
| **出块模式** | instant-seal | E2E 测试用 instant-seal 避免 6s 等待 |
| **状态隔离** | 每 flow 独立账户 | 避免账户命名空间使用 flow 前缀 (如 `//T2/Buyer`) |
| **CI 触发** | PR + nightly | PR 运行 P0 流程 (~5分钟), nightly 运行全部 |
| **报告格式** | JSON + GitHub Summary | CI 中自动生成摘要 |

### 7.3 现有脚本处置

| 脚本 | 处置 |
|------|------|
| test-otc.ts | ❌ 删除 (pallet 已移除) |
| test-swap.ts | ❌ 删除 (pallet 已移除) |
| test-maker.ts | 🔄 迁移到 flows/trading/maker-lifecycle.ts |
| test-pricing.ts | 🔄 迁移到 flows/trading/price-discovery.ts |
| test-referral.ts | 🔄 合并到 flows/entity/member-commission.ts |
| test-credit.ts | 🔄 合并到 flows/trading/maker-lifecycle.ts |
| test-escrow.ts | 🔄 合并到 flows/trading/p2p-buy.ts |
| test-arbitration.ts | 🔄 迁移到 flows/dispute/arbitration.ts |
| test-chat.ts | 🔄 保留 (Chat 模块独立) |
| test-cny-rate.ts | 🔄 合并到 flows/trading/price-discovery.ts |

---

## 8. 结论

### 可行性: ✅ 完全可行

- 技术栈成熟 (polkadot-js + TypeScript)
- 现有工具库可复用
- 增量交付无风险

### 合理性: ✅ 强烈推荐

- **17 个 Pallet 完全未覆盖**，存在显著的测试盲区
- **OTC/Swap → P2P 迁移**后现有测试已失效，急需更新
- 项目已进入多模块联动阶段，单元测试不足以保证跨模块正确性
- 投入 ~21 人天，换取长期自动化回归能力，ROI 显著

### 建议立即启动 Phase 1

优先级最高的工作:
1. 搭建 E2E 核心框架 (runner + assertions + reporter)
2. 实现 P2P Buy/Sell 流程测试 (替代废弃的 test-otc.ts / test-swap.ts)
3. 实现实体→店铺创建流程测试 (覆盖 Entity 核心链路)

---

## 附录: 测试数量预估

| 领域 | 流程数 | 预计测试用例数 |
|------|:------:|:-------------:|
| Trading | 4 | ~35 |
| Entity | 6 | ~60 |
| GroupRobot | 3 | ~25 |
| Dispute | 1 | ~10 |
| Storage | 1 | ~5 |
| 复合场景 | 4 | ~20 |
| **合计** | **19** | **~155** |
