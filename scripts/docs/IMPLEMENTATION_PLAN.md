# Nexus 测试实施方案

> 基于 NEXUS_TEST_PLAN*.md (测试计划) + E2E_TEST_BOT_ANALYSIS.md (测试框架) 综合规划
> 制定日期: 2026-03-06

---

## 1. 现状概览

### 1.1 资产盘点

| 维度 | 已有 | 缺口 |
|------|------|------|
| **测试计划** | 3 份文档, ~686 测试用例 (P0: 131, P1: 333, P2: 77 + Part1 ~145) | 覆盖率映射仅 204 条, 缺口 ~480+ |
| **E2E 框架** | core/ 7 模块 + 23 条 Flow + 9 种断言 | 无 tsconfig.json, 无 CI E2E job |
| **Cargo 测试** | 29 pallets 已纳入 ALL_PALLETS | CI 已跑 cargo test, 但未与 E2E 联动 |
| **E2E 覆盖** | 26 pallets 直接覆盖, ~170+ extrinsics | EntityReview 无 E2E 流程 |
| **CI/CD** | ci.yml: build + clippy + test + docker | 无 E2E job, 无覆盖率报告上传 |
| **开发链** | ci.yml 有 run-node job (block 检测) | 无 E2E 跑后门诊断 |

### 1.2 覆盖率差距分析

```
测试计划用例总数:  ~830+
Coverage Map 映射: 204 条 (~24.5%)
────────────────────────────────────
未映射 P0 用例:    估计 ~80+
未映射 P1 用例:    估计 ~280+
未映射 P2 用例:    估计 ~60+
```

覆盖缺口主要来自:
1. **每条 Flow 只覆盖核心路径**, 测试计划中的大量负向/边界/安全用例未被 E2E 涵盖
2. **测试计划新增接口** (Part1 的 `unban_entity`, `force_transfer_ownership` 等) 未在现有 Flow 中测试
3. **跨模块集成测试** (Part3 Section 28-31) 完全没有 E2E 覆盖

---

## 2. 实施原则

| 原则 | 说明 |
|------|------|
| **P0 优先** | 先确保 131 条 P0 用例全部有测试覆盖 (Cargo 或 E2E) |
| **Cargo 兜底** | 单一 extrinsic 的参数边界/权限测试优先用 Cargo unit test 覆盖 |
| **E2E 覆盖流程** | 多步骤跨 pallet 业务流程用 E2E Flow 覆盖 |
| **增量映射** | 每完成一批测试, 同步更新 COVERAGE_MAP |
| **CI 闭环** | 每个 PR 跑 P0 E2E, nightly 跑全量 |

---

## 3. 分阶段实施计划

### Phase 0: 基础设施加固 (第 1 周)

> 目标: 让现有 23 条 E2E Flow 能可靠运行, CI 能自动执行

#### 0.1 补齐 tsconfig.json

```
scripts/
└── tsconfig.json   ← 新增
```

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "Node16",
    "esModuleInterop": true,
    "strict": true,
    "outDir": "./dist",
    "rootDir": "./e2e",
    "declaration": false,
    "sourceMap": true
  },
  "include": ["e2e/**/*.ts"],
  "exclude": ["node_modules", "dist"]
}
```

#### 0.2 本地验证流程

```bash
# Step 1: 启动 dev 链 (instant-seal 模式)
./target/release/nexus-node --dev --tmp --rpc-cors=all \
  --sealing=instant

# Step 2: 安装依赖
cd scripts && npm install

# Step 3: 冒烟测试 — Phase 1 (4 流程, 最快)
npm run e2e:phase1

# Step 4: 全量 E2E (23 流程)
npm run e2e

# Step 5: 覆盖率报告
npm run agent -- --mode coverage
```

#### 0.3 CI E2E Job

在 `.github/workflows/ci.yml` 新增 job:

```yaml
e2e-test:
  name: E2E Tests
  runs-on: ubuntu-latest
  needs: [build]
  steps:
    - uses: actions/checkout@v4
    - name: Download nexus-node
      uses: actions/download-artifact@v4
      with:
        name: nexus-node
    - name: Start dev chain
      run: |
        chmod +x nexus-node
        ./nexus-node --dev --tmp --sealing=instant &
        sleep 10
    - name: Install E2E deps
      run: cd scripts && npm ci
    - name: Run P0 E2E (PR gate)
      run: cd scripts && npm run agent -- --mode e2e --priority P0
    - name: Upload report
      if: always()
      uses: actions/upload-artifact@v4
      with:
        name: e2e-report
        path: scripts/e2e-reports/
```

Nightly 全量:

```yaml
# .github/workflows/nightly-e2e.yml
on:
  schedule:
    - cron: '0 2 * * *'    # 每天凌晨 2 点
  workflow_dispatch:

jobs:
  full-e2e:
    # ... 与上面类似, 但运行全量
    - run: cd scripts && npm run agent
```

#### 0.4 交付物

| 交付 | 文件/产物 |
|------|----------|
| tsconfig.json | `scripts/tsconfig.json` |
| CI E2E job | `.github/workflows/ci.yml` 追加 |
| Nightly workflow | `.github/workflows/nightly-e2e.yml` 新增 |
| 本地运行文档 | 本文 Section 3 Phase 0.2 |

---

### Phase 1: P0 用例全覆盖 (第 2-3 周)

> 目标: 131 条 P0 用例 100% 有测试覆盖 (Cargo 或 E2E)

#### 1.1 P0 用例清单与覆盖策略

**Part 1 — Entity 核心 (Section 1-7)**

| 用例 ID | 描述 | 覆盖方式 | 状态 |
|---------|------|----------|------|
| ER-001 | create_entity 付费激活 | E2E (E1) | ✅ 已覆盖 |
| ER-005 | 余额不足被拒绝 | Cargo | 待补 |
| ER-012 | 治理审批 Pending→Active | E2E (E1) | ✅ 已覆盖 |
| ER-013 | 非治理 Origin 无法审批 | Cargo | 待补 |
| SH-001 | create_shop | E2E (E1) | ✅ 已覆盖 |
| SH-002 | 实体未激活创建被拒绝 | Cargo | 待补 |
| SH-033 | 提取超可用余额被拒绝 | Cargo | 待补 |
| SV-001 | create_product | E2E (E2) | ✅ 已覆盖 |
| SV-004 | publish/unpublish | E2E (E2) | ✅ 已覆盖 |
| OD-001 | NEX 支付下单 | E2E (E2) | ✅ 已覆盖 |
| OD-002 | shopping_balance 抵扣 | Cargo | 待补 |
| OD-004 | 商品已下架/余额不足 | Cargo | 待补 |
| OD-005 | 买家取消退款 | E2E (E2) | ✅ 已覆盖 |
| OD-006 | 发货→确认→释放 | E2E (E2) | ✅ 已覆盖 |
| OD-018 | 订单触发双路佣金 | E2E (E4) | ✅ 部分覆盖 |
| OD-019 | 下单自动注册会员 | Cargo | 待补 |
| OD-020 | 取消触发 cancel_commission | Cargo | 待补 |
| TK-001 | create_shop_token 7 种类型 | E2E (E5) | ✅ 部分覆盖 |
| TK-003 | mint_tokens | E2E (E5) | ✅ 已覆盖 |
| TK-004 | transfer_tokens | E2E (E5) | ✅ 已覆盖 |
| TK-020 | 全局暂停时操作被拒绝 | Cargo | 待补 |
| TK-021 | 冻结时转账被拒绝 | Cargo | 待补 |
| GV-002 | create_proposal | E2E (E5) | ✅ 已覆盖 |
| GV-003 | 持有不足被拒绝 | E2E (E5) | ✅ 已覆盖 |
| GV-004 | vote 投票 | E2E (E5) | ✅ 已覆盖 |
| GV-006 | execute_proposal | E2E (E5) | ✅ 已覆盖 |

**Part 2 — 会员/佣金/交易/争议**

| 用例 ID | 描述 | 覆盖方式 | 状态 |
|---------|------|----------|------|
| MB-* P0 | 会员注册/绑定推荐 | E2E (E3) | ✅ 大部分覆盖 |
| CM-* P0 | 佣金初始化/分配/提现 | E2E (E4) | ✅ 部分覆盖 |
| DC-* P0 | 信息披露发布/撤回 | E2E (E9) | ✅ 已覆盖 |
| KY-* P0 | KYC 提交/批准/撤销 | E2E (E6) | ✅ 已覆盖 |
| TS-* P0 | 代币发售/认购/领取 | E2E (E7) | ✅ 已覆盖 |
| NM-* P0 | NEX 市场挂单/成交 | E2E (T4) | ✅ 已覆盖 |
| EM-* P0 | 实体市场交易 | E2E (E8) | ✅ 已覆盖 |
| ES-* P0 | 托管锁定/释放 | E2E (D2) | ✅ 已覆盖 |
| EV-* P0 | 证据提交 | E2E (D1) | ✅ 已覆盖 |

**Part 3 — GroupRobot/Storage/Ads/集成**

| 用例 ID | 描述 | 覆盖方式 | 状态 |
|---------|------|----------|------|
| AR-* P0 | 仲裁流程 | E2E (D1) | ✅ 部分覆盖 |
| SS-* P0 | 存储服务全流程 | E2E (S1) | ✅ 已覆盖 |
| SL-* P0 | 存储生命周期 | Cargo | 待补 |
| GR-* P0 | Bot 注册/管理 | E2E (G1) | ✅ 已覆盖 |
| CN-* P0 | 节点注册/质押/退出 | E2E (G2) | ✅ 已覆盖 |
| SB-* P0 | 订阅/取消 | E2E (G4) | ✅ 已覆盖 |
| GC-* P0 | 社区管理 | E2E (G5) | ✅ 已覆盖 |
| CE-* P0 | 仪式记录/撤销 | E2E (G6) | ✅ 已覆盖 |
| RW-* P0 | 奖励领取 | E2E (G7) | ✅ 已覆盖 |
| AD-* P0 | 广告创建/结算 | E2E (G3) | ✅ 已覆盖 |
| INT-* P0 | 跨模块集成 | 待新建 | 待补 |
| SEC-* P0 | 安全性测试 | 待新建 | 待补 |

#### 1.2 Cargo 补充任务清单

未被 E2E 覆盖的 P0 用例, 需在对应 pallet 的 `tests/` 中补充:

| Pallet | 待补用例 | 工作量估计 |
|--------|---------|:---------:|
| pallet-entity-registry | ER-005 (余额不足), ER-013 (非治理审批) | 0.5d |
| pallet-entity-shop | SH-002 (未激活创建), SH-033 (超额提取) | 0.5d |
| pallet-entity-order | OD-002 (shopping_balance), OD-004 (下架/余额不足), OD-019 (自动注册), OD-020 (cancel_commission) | 1d |
| pallet-entity-token | TK-020 (全局暂停), TK-021 (冻结转账) | 0.5d |
| pallet-storage-lifecycle | SL-* P0 全部 | 1d |
| 跨模块集成 (Cargo) | INT-* P0 | 1.5d |
| **总计** | | **~5d** |

#### 1.3 COVERAGE_MAP 更新

每完成一批 Cargo/E2E 测试, 同步更新 `nexus-test-agent.ts` 中的 `COVERAGE_MAP`:

```typescript
const COVERAGE_MAP: CoverageMap = {
  // ... 现有 23 条 Flow 映射 ...
  
  // 新增: Cargo 测试映射 (标记 Cargo 覆盖的用例)
  'Cargo: pallet-entity-registry': [
    'ER-005', 'ER-013',
  ],
  'Cargo: pallet-entity-shop': [
    'SH-002', 'SH-033',
  ],
  // ...
};
```

#### 1.4 交付物

| 交付 | 验收标准 |
|------|----------|
| P0 Cargo 补充 | 新增 Cargo test 全部通过 |
| COVERAGE_MAP 更新 | `npm run agent -- --mode coverage --priority P0` 覆盖率 100% |
| 覆盖率报告 | P0 用例全绿 |

---

### Phase 2: 流程增强 + P1 覆盖 (第 4-6 周)

> 目标: 补全 EntityReview E2E 流程, 增强现有流程, P1 覆盖率 > 70%

#### 2.1 新增 E2E Flow: E10 EntityReview

```
flows/entity/entity-review.ts    ← 新增
```

| Step | 操作 |
|:----:|------|
| 1 | 确保 Entity + Shop + Product + 已完成订单就绪 |
| 2 | Bob 评价订单 (`entityReview.submitReview`) |
| 3 | Eve 回复评价 (`entityReview.replyToReview`) |
| 4 | Bob 编辑评价 (`entityReview.editReview`) |
| 5 | Root 删除评价 (`entityReview.removeReview`) |
| 6 | Eve 关闭评价 (`entityReview.setReviewEnabled`, false) |
| 7 | [错误] 非买家评价 |
| 8 | [错误] 重复评价 |

注册为 E10, 加入 `run-e2e.ts` Phase 2, 更新 COVERAGE_MAP.

#### 2.2 现有 Flow 增强

针对测试计划中 P1 但现有 Flow 未涉及的 extrinsics, 增强现有 Flow:

| Flow | 增强内容 | 新增覆盖 |
|------|---------|----------|
| E1 | 增加 unban_entity, cancel_close_request, self_pause/resume, resign_admin | ER-026~031 |
| E1 | 增加 force_transfer_ownership | ER-032 |
| E2 | 增加 seller_cancel_order, update_shipping_address, update_tracking | OD-013~016 |
| E5 | 增加 delegate_vote, admin_veto | GV-005, GV-008 |
| E5 | 增加 force_disable_token, force_freeze, burn_tokens | TK-012~016 |
| E8 | 增加更多边界场景 (空订单簿市价交易) | EM-* P1 |
| S1 | 增加生命周期集成 (pin→续期→到期→清理) | SL-* P1 |

每个 Flow 增强约 0.5-1d, 总计约 **5d**.

#### 2.3 Cargo P1 补充

| Pallet 群 | 待补 P1 数 | 工作量 |
|-----------|:---------:|:------:|
| Entity Registry/Shop/Product | ~15 | 2d |
| Entity Order/Review | ~10 | 1.5d |
| Entity Token/Governance | ~12 | 1.5d |
| Commission (Core + 5 插件) | ~20 | 2d |
| Entity Member/KYC/TokenSale | ~15 | 2d |
| Trading (NEX Market) | ~10 | 1d |
| Entity Market/Disclosure | ~10 | 1d |
| Escrow/Evidence/Arbitration | ~10 | 1d |
| GroupRobot (6 pallets) | ~20 | 2d |
| Storage/Ads | ~10 | 1d |
| **总计** | **~132** | **~15d** |

#### 2.4 流程隔离增强

将各 Flow 从共享 dev 账户迁移到 `createFlowAccounts()`:

```typescript
// 改前:
const bob = ctx.actor('bob');

// 改后:
const accounts = createFlowAccounts('E8', ['seller', 'buyer', 'unauthorized']);
const seller = accounts.seller;
const buyer = accounts.buyer;
```

逐 Flow 改造, 约 **2d**.

#### 2.5 交付物

| 交付 | 验收标准 |
|------|----------|
| E10 Flow | flows/entity/entity-review.ts + 注册 + COVERAGE_MAP |
| Flow 增强 (7 个) | 各 Flow 新增步骤通过 |
| Cargo P1 | `cargo test` 全部通过 |
| 流程隔离 | 23 条 Flow 全部使用隔离账户 |
| 覆盖率 | P1 覆盖率 > 70% |

---

### Phase 3: 高级场景 + CI 完善 (第 7-9 周)

> 目标: 跨模块集成, 安全性, 性能测试, CI 完善

#### 3.1 跨模块集成 E2E 流程

基于 NEXUS_TEST_PLAN_PART3 Section 28-29, 新增复合 E2E 流程:

| Flow ID | 名称 | 覆盖场景 |
|---------|------|----------|
| X1 | 新用户旅程 | 注册 → 绑定推荐 → 下单 → 评价 → 佣金触发 → 提现 |
| X2 | 商户入驻 | 实体创建 → 店铺 → 商品 → KYC → Token 发行 → 市场上架 |
| X3 | Bot 完整运营 | Bot 注册 → 节点共识 → 社区绑定 → 订阅 → 广告 → 奖励 |
| X4 | 争议全流程 | 下单 → 托管锁定 → 争议 → 证据 → 仲裁 → 执行 |

每条约 2d, 总计 **8d**.

#### 3.2 安全性测试

基于 NEXUS_TEST_PLAN_PART3 Section 31:

| 分类 | 测试内容 | 实现方式 |
|------|---------|----------|
| 重入保护 | 同一区块内重复操作 | Cargo (mock) |
| 整数溢出 | 大额 Balance 乘法 | Cargo |
| 权限绕过 | 全 pallet 非 Origin 调用 | Cargo (已有, 需补全) |
| 跨 pallet 状态一致性 | 实体关闭后 Shop/Token/Order 行为 | E2E (X5) |
| DoS 防护 | 批量操作 gas 限制 | Cargo + E2E |

约 **5d**.

#### 3.3 性能与边界测试

基于 NEXUS_TEST_PLAN_PART3 Section 30:

| 场景 | 方法 | 工作量 |
|------|------|:------:|
| 100 并发订单 | E2E batch (TypeScript Promise.all) | 1d |
| MaxProductsPerShop 边界 | Cargo | 0.5d |
| MaxEntitiesPerUser 边界 | Cargo | 0.5d |
| 大 Vec 参数 (batch_submit_logs) | Cargo + E2E | 0.5d |
| 长 Era 奖励计算 | Cargo | 0.5d |
| **总计** | | **3d** |

#### 3.4 CI 增强

```yaml
# PR 检查 (快速, ~5min)
e2e-pr:
  - cargo test (P0 pallets only)
  - E2E Phase 1 (T1-T3, E1)
  - Coverage check (P0 must be 100%)

# Nightly 全量 (~30min)
e2e-nightly:
  - cargo test (全部 29 pallets)
  - E2E 全量 (23 flows + X1-X4)
  - Coverage report → Slack/邮件通知
  - Upload coverage JSON as artifact

# Weekly 深度 (~1h)
e2e-weekly:
  - 全量 + 性能测试
  - HTML 覆盖率报告生成
```

#### 3.5 交付物

| 交付 | 验收标准 |
|------|----------|
| X1-X4 复合流程 | 全部通过 |
| 安全性测试 | 所有安全用例通过 |
| 性能测试 | 边界场景通过 |
| CI PR gate | PR 自动阻塞 P0 失败 |
| Nightly 报告 | 每日报告可查 |
| 覆盖率 | P0: 100%, P1: >85%, P2: >50% |

---

### Phase 4: P2 + 持续维护 (第 10 周起)

> 目标: 长尾用例, 报告优化, 文档维护

#### 4.1 P2 用例补全

| 领域 | 估计工作量 |
|------|:---------:|
| Entity (升级类型, 验证, 重开, 积分 TTL) | 2d |
| Commission (多级/团队/池奖励边界) | 2d |
| GroupRobot (Ads 高级场景) | 1d |
| Storage Lifecycle (存储过期边界) | 1d |
| **总计** | **6d** |

#### 4.2 HTML 覆盖率报告

扩展 `coverage-tracker.ts`, 输出带颜色进度条的 HTML 报告, 支持按模块/优先级/Flow 维度展示.

#### 4.3 维护机制

| 机制 | 说明 |
|------|------|
| **新 extrinsic → 测试** | 每次新增 pallet extrinsic, 必须同步更新 NEXUS_TEST_PLAN 和添加对应测试 |
| **COVERAGE_MAP 自动化** | 考虑从 Cargo test 名称自动生成映射, 减少手动维护 |
| **回归保护** | 任何 P0 测试失败, PR 自动阻塞 |
| **季度审计** | 每季度审查覆盖率报告, 确保新功能都有测试 |

---

## 4. 执行步骤清单 (快速参考)

### Step 1: 环境准备

```bash
# 1.1 编译 nexus-node (release)
cargo build --release -p nexus-node

# 1.2 启动 dev 链 (instant-seal 模式, 出块立即)
./target/release/nexus-node --dev --tmp \
  --rpc-cors=all --sealing=instant

# 1.3 验证链运行
curl -s -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"system_health"}' \
  http://127.0.0.1:9944 | jq .
```

### Step 2: E2E 框架准备

```bash
# 2.1 安装依赖
cd scripts && npm install

# 2.2 类型检查 (需先创建 tsconfig.json)
npx tsc --noEmit

# 2.3 冒烟测试 (Phase 1 — 4 流程)
npm run e2e:phase1
```

### Step 3: 分阶段运行

```bash
# 3.1 仅 Cargo 测试 (无需链节点)
npm run agent -- --mode cargo

# 3.2 E2E Phase 1 (核心交易 + 实体基础)
npm run e2e:phase1

# 3.3 E2E Phase 2 (实体扩展 + 争议)
npm run e2e:phase2

# 3.4 E2E Phase 3 (GroupRobot + Storage)
npm run e2e:phase3

# 3.5 全量运行 (Cargo + E2E + Coverage)
npm run agent

# 3.6 查看覆盖率
npm run agent -- --mode coverage
npm run agent -- --mode coverage --priority P0

# 3.7 指定流程 debug
npm run e2e -- --flow T4 E8 D2

# 3.8 按模块群运行
npm run e2e:trading
npm run e2e:entity
npm run e2e:dispute
npm run e2e:grouprobot
npm run e2e:storage
```

### Step 4: 排查失败

```bash
# 4.1 查看详细日志
VERBOSE=true npm run e2e -- --flow E1

# 4.2 查看 JSON 报告
cat e2e-reports/agent-report-*.json | jq '.e2e.report'

# 4.3 检查链状态
# 使用 polkadot.js apps: https://polkadot.js.org/apps/?rpc=ws://127.0.0.1:9944

# 4.4 重置链 (清除所有状态)
# 重启 nexus-node 带 --tmp 即可

# 4.5 单独运行某条 Flow 的对应 Cargo 测试
cargo test -p pallet-entity-registry -- test_create_entity
```

### Step 5: 更新覆盖率映射

```bash
# 5.1 修改 COVERAGE_MAP (nexus-test-agent.ts)
# 5.2 验证映射正确性
npm run agent -- --mode coverage
# 5.3 查看未覆盖 P0
npm run agent -- --mode coverage --priority P0
```

---

## 5. 时间线与里程碑

```
Week 1          Phase 0: 基础设施
                ├── tsconfig.json
                ├── CI E2E job
                ├── 本地全量验证
                └── 验收: 23 流程全绿 ✓

Week 2-3        Phase 1: P0 全覆盖
                ├── Cargo P0 补充 (~5d)
                ├── COVERAGE_MAP P0 同步
                └── 验收: P0 覆盖率 100% ✓

Week 4-6        Phase 2: 流程增强 + P1
                ├── E10 EntityReview Flow
                ├── 7 个 Flow 增强
                ├── Cargo P1 补充 (~15d)
                ├── 流程隔离改造
                └── 验收: P1 覆盖率 > 70% ✓

Week 7-9        Phase 3: 高级场景
                ├── X1-X4 跨模块集成
                ├── 安全性测试
                ├── 性能边界测试
                ├── CI 完善 (PR gate + nightly)
                └── 验收: P0=100%, P1>85% ✓

Week 10+        Phase 4: 长尾维护
                ├── P2 用例补全
                ├── HTML 报告
                └── 持续维护机制
```

---

## 6. 风险与应对

| 风险 | 影响 | 概率 | 应对 |
|------|------|:----:|------|
| dev 链 instant-seal 与生产行为不一致 | E2E 通过但生产失败 | 中 | Phase 3 增加 manual-seal 测试 |
| Cargo test 编译时间长 (29 pallets) | CI 超时 | 高 | 按模块群分组并行 + 缓存 target/ |
| 流程间状态泄漏 | 间歇性失败 | 高 | Phase 2 实施账户隔离 |
| 新 pallet 添加未同步测试计划 | 覆盖率下降 | 中 | CI 检查 ALL_PALLETS 与计划一致性 |
| E2E 运行时间过长 | 阻塞 PR | 中 | PR 仅跑 Phase 1, nightly 全量 |
| TEE/OCW 模拟不精确 | 假阳性 | 低 | 标记为 soft assertion, 不阻塞 |

---

## 7. 资源需求

| 角色 | 职责 | 投入 |
|------|------|:----:|
| Substrate 开发 | Cargo test 编写, pallet 边界修复 | ~20d |
| E2E 测试工程师 | Flow 编写/增强, 覆盖率映射 | ~25d |
| DevOps | CI/CD pipeline, 链节点管理 | ~5d |
| **总计** | | **~50d (约 2.5 人月)** |
