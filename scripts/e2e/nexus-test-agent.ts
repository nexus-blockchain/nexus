#!/usr/bin/env tsx
/**
 * Nexus 全栈测试智能体
 *
 * 统一协调 Cargo 单元测试 + E2E 链上测试 + 覆盖率追踪
 *
 * 用法:
 *   npx tsx e2e/nexus-test-agent.ts                       # 全量运行
 *   npx tsx e2e/nexus-test-agent.ts --mode cargo           # 仅 Cargo 测试
 *   npx tsx e2e/nexus-test-agent.ts --mode e2e             # 仅 E2E 测试
 *   npx tsx e2e/nexus-test-agent.ts --mode coverage        # 仅覆盖率报告
 *   npx tsx e2e/nexus-test-agent.ts --group entity         # 仅 entity 模块群
 *   npx tsx e2e/nexus-test-agent.ts --group grouprobot     # 仅 grouprobot 模块群
 *   npx tsx e2e/nexus-test-agent.ts --priority P0          # 仅 P0 优先级
 *   npx tsx e2e/nexus-test-agent.ts --flow E1 E2 E3        # 仅指定 E2E 流程
 *   npx tsx e2e/nexus-test-agent.ts --pallet pallet-dispute-escrow # 仅指定 pallet
 */

import * as path from 'path';
import * as fs from 'fs';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

import { getApi, disconnectApi } from './core/chain-state.js';
import { runFlows, FlowDef } from './core/test-runner.js';
import { getDevAccounts, fundAccounts } from './fixtures/accounts.js';
import {
  runCargoTests,
  printCargoSummary,
  CargoTestResult,
  ALL_PALLETS,
  PALLET_GROUPS,
} from './core/cargo-runner.js';
import {
  parseTestPlan,
  applyCoverage,
  generateCoverageReport,
  printCoverageReport,
  writeCoverageJSON,
  CoverageMap,
} from './core/coverage-tracker.js';

// ─── Flow 注册表 ───────────────────────────────────────────────

import { makerLifecycleFlow } from './flows/trading/maker-lifecycle.js';
import { p2pBuyFlow } from './flows/trading/p2p-buy.js';
import { p2pSellFlow } from './flows/trading/p2p-sell.js';
import { nexMarketAdminFlow } from './flows/trading/nex-market-admin.js';
import { entityShopFlow } from './flows/entity/entity-shop.js';
// Phase A 新流程
import { orderLifecycleFlow } from './flows/entity/order-lifecycle.js';
import { orderAdminFlow } from './flows/entity/order-admin.js';
import { memberReferralFlow } from './flows/entity/member-referral.js';
import { commissionFlow } from './flows/entity/commission.js';
import { tokenGovernanceFlow } from './flows/entity/token-governance.js';
import { tokenGovernanceAdminFlow } from './flows/entity/token-governance-admin.js';
import { kycFlow } from './flows/entity/kyc.js';
import { tokenSaleFlow } from './flows/entity/token-sale.js';
import { entityMarketFlow } from './flows/entity/entity-market.js';
import { entityDisclosureFlow } from './flows/entity/entity-disclosure.js';
import { nexMarketFlow } from './flows/trading/nex-market.js';
import { disputeFlow } from './flows/dispute/dispute-resolution.js';
import { escrowFlow } from './flows/dispute/escrow.js';
import { arbitrationAppealFlow } from './flows/dispute/arbitration-appeal.js';
import { botLifecycleFlow } from './flows/grouprobot/bot-lifecycle.js';
import { nodeConsensusFlow } from './flows/grouprobot/node-consensus.js';
import { adCampaignFlow } from './flows/grouprobot/ad-campaign.js';
import { adsCorePreferencesFlow } from './flows/grouprobot/ads-core-preferences.js';
import { subscriptionFlow } from './flows/grouprobot/subscription.js';
import { communityFlow } from './flows/grouprobot/community.js';
import { ceremonyFlow } from './flows/grouprobot/ceremony.js';
import { rewardsFlow } from './flows/grouprobot/rewards.js';
import { storageServiceFlow } from './flows/storage/storage-service.js';
import { storageBillingDisputeFlow } from './flows/storage/storage-billing-dispute.js';

/** 全部已注册流程 */
const FLOW_REGISTRY: Record<string, FlowDef> = {
  T1: makerLifecycleFlow,
  T2: p2pBuyFlow,
  T3: p2pSellFlow,
  T4: nexMarketFlow,
  T5: nexMarketAdminFlow,
  E1: entityShopFlow,
  E2: orderLifecycleFlow,
  E3: memberReferralFlow,
  E4: commissionFlow,
  E5: tokenGovernanceFlow,
  E6: kycFlow,
  E7: tokenSaleFlow,
  E8: entityMarketFlow,
  E9: entityDisclosureFlow,
  E10: orderAdminFlow,
  E11: tokenGovernanceAdminFlow,
  D1: disputeFlow,
  D2: escrowFlow,
  D3: arbitrationAppealFlow,
  G1: botLifecycleFlow,
  G2: nodeConsensusFlow,
  G3: adCampaignFlow,
  G4: subscriptionFlow,
  G5: communityFlow,
  G6: ceremonyFlow,
  G7: rewardsFlow,
  A1: adsCorePreferencesFlow,
  S1: storageServiceFlow,
  S2: storageBillingDisputeFlow,
};

/** Flow 分组 */
const FLOW_GROUPS: Record<string, string[]> = {
  trading: ['T1', 'T2', 'T3', 'T4', 'T5'],
  entity: ['E1', 'E2', 'E3', 'E4', 'E5', 'E6', 'E7', 'E8', 'E9', 'E10', 'E11'],
  dispute: ['D1', 'D2', 'D3'],
  grouprobot: ['G1', 'G2', 'G3', 'A1', 'G4', 'G5', 'G6', 'G7'],
  storage: ['S1', 'S2'],
  wave1: ['T5', 'E10', 'E11', 'D3', 'A1', 'S2'],
};

/** Flow → 覆盖的测试计划用例 ID */
const COVERAGE_MAP: CoverageMap = {
  'Flow-E1: 实体→店铺创建': [
    'ER-001', 'ER-007', 'ER-009', 'ER-011', 'ER-012', 'ER-015', 'ER-016',
    'ER-013', 'ER-014', 'SH-001',
  ],
  'Flow-T1: 种子资金+流动性': [
    'NM-001', 'NM-024', 'NM-029',
  ],
  'Flow-T2: NEX 卖单流程': [
    'NM-007', 'NM-008', 'NM-009', 'NM-010',
  ],
  'Flow-T3: NEX 买单流程': [
    'NM-001', 'NM-002', 'NM-003', 'NM-005', 'NM-006',
  ],
  'Flow-E2: 订单生命周期': [
    'OD-001', 'OD-004', 'OD-005', 'OD-006', 'OD-007', 'OD-012',
    'OD-013', 'OD-014', 'OD-015',
    'SV-001', 'SV-003',
  ],
  'Flow-E3: 会员推荐裂变': [
    'MB-001', 'MB-002', 'MB-003', 'MB-004', 'MB-005', 'MB-007', 'MB-008',
    'MB-011', 'MB-012', 'MB-015',
  ],
  'Flow-D1: 争议解决': [
    'EV-001', 'AR-008', 'AR-009', 'AR-010', 'AR-011', 'AR-012', 'AR-013',
  ],
  'Flow-G1: Bot 生命周期': [
    'GR-001', 'GR-002', 'GR-003', 'GR-004', 'GR-006',
  ],
  'Flow-E5: Token+治理': [
    'TK-001', 'TK-005', 'TK-007', 'TK-011', 'TK-012', 'TK-014',
    'GV-001', 'GV-003', 'GV-004', 'GV-006', 'GV-009', 'GV-011',
  ],
  'Flow-E6: KYC 认证': [
    'KY-001', 'KY-002', 'KY-003', 'KY-004', 'KY-007', 'KY-009', 'KY-010', 'KY-011',
  ],
  'Flow-E4: 佣金返佣': [
    'CM-001', 'CM-002', 'CM-003', 'CM-004', 'CM-009', 'CM-010', 'CM-013',
    'CM-016', 'CM-021',
  ],
  'Flow-E7: 代币发售': [
    'TS-001', 'TS-003', 'TS-004', 'TS-006', 'TS-007', 'TS-009', 'TS-010',
    'TS-012', 'TS-014', 'TS-015', 'TS-016', 'TS-017', 'TS-018',
  ],
  'Flow-G2: 节点共识': [
    'CN-001', 'CN-002', 'CN-003', 'CN-004', 'CN-005', 'CN-006',
    'CN-008', 'CN-009', 'CN-010', 'CN-011',
  ],
  'Flow-G3: 广告活动': [
    'AD-001', 'AD-002', 'AD-003', 'AD-004', 'AD-005', 'AD-007', 'AD-008',
    'AD-009', 'AD-010', 'AD-011', 'AD-012', 'AD-013', 'AD-014',
    'AD-018', 'AD-019', 'AD-020', 'AD-021', 'AD-022',
  ],
  'Flow-S1: 存储服务': [
    'SS-001', 'SS-002', 'SS-003', 'SS-004', 'SS-005', 'SS-006',
    'SS-008', 'SS-010', 'SS-013', 'SS-015', 'SS-016', 'SS-017',
    'SS-018', 'SS-020', 'SS-022',
  ],
  'Flow-E8: 实体市场': [
    'EM-001', 'EM-002', 'EM-003', 'EM-004', 'EM-005', 'EM-006',
    'EM-007', 'EM-008', 'EM-009', 'EM-010', 'EM-013',
  ],
  'Flow-E9: 信息披露': [
    'DC-001', 'DC-002', 'DC-003', 'DC-005', 'DC-006', 'DC-008',
    'DC-009', 'DC-011', 'DC-012',
  ],
  'Flow-T4: NEX 市场': [
    'NM-001', 'NM-002', 'NM-003', 'NM-005', 'NM-006', 'NM-007',
    'NM-008', 'NM-011', 'NM-013', 'NM-014', 'NM-024', 'NM-029',
  ],
  // 这批增量流主要覆盖 2026-03-09 新增接口，其中一部分在旧测试计划里还没有 case id。
  'Flow-T5: NEX Market 管理/争议': [],
  'Flow-E10: 订单治理/维护': [
    'OD-008', 'OD-010', 'OD-017',
  ],
  'Flow-E11: Token/Governance 管理': [
    'TK-003',
  ],
  'Flow-D2: 托管': [
    'ES-001', 'ES-002', 'ES-003', 'ES-004', 'ES-005', 'ES-007',
    'ES-008', 'ES-009', 'ES-010', 'ES-011',
  ],
  'Flow-D3: 仲裁申诉': [
    'AR-007', 'AR-008', 'AR-011', 'AR-024',
  ],
  'Flow-G4: 订阅服务': [
    'SB-001', 'SB-002', 'SB-003', 'SB-004', 'SB-007', 'SB-008',
    'SB-009',
  ],
  'Flow-G5: 社区管理': [
    'GC-001', 'GC-006', 'GC-007', 'GC-008', 'GC-010', 'GC-012',
    'GC-013', 'GC-014', 'GC-016',
  ],
  'Flow-G6: 仪式验证': [
    'CE-001', 'CE-005', 'CE-007', 'CE-008', 'CE-009', 'CE-011',
  ],
  'Flow-G7: 奖励分配': [
    'RW-001', 'RW-002',
  ],
  'Flow-A1: Ads Core 偏好/确认': [
    'AC-001', 'AC-005', 'AC-022', 'AC-023', 'AC-026',
    'AC-027', 'AC-029', 'AC-032', 'AG-001', 'AG-012',
  ],
  'Flow-S2: 存储计费/Slash 争议': [
    'SS-001', 'SS-002', 'SS-012',
  ],
};

// ─── CLI 参数解析 ───────────────────────────────────────────────

interface AgentOptions {
  mode: 'all' | 'cargo' | 'e2e' | 'coverage';
  groups: string[];
  flows: string[];
  pallets: string[];
  priority?: string;
  verbose: boolean;
  reportDir: string;
}

function parseArgs(): AgentOptions {
  const args = process.argv.slice(2);
  const opts: AgentOptions = {
    mode: 'all',
    groups: [],
    flows: [],
    pallets: [],
    verbose: false,
    reportDir: path.resolve(process.cwd(), 'e2e-reports'),
  };

  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case '--mode':
        opts.mode = args[++i] as AgentOptions['mode'];
        break;
      case '--group':
        while (i + 1 < args.length && !args[i + 1].startsWith('--')) {
          opts.groups.push(args[++i]);
        }
        break;
      case '--flow':
        while (i + 1 < args.length && !args[i + 1].startsWith('--')) {
          opts.flows.push(args[++i].toUpperCase());
        }
        break;
      case '--pallet':
        while (i + 1 < args.length && !args[i + 1].startsWith('--')) {
          opts.pallets.push(args[++i]);
        }
        break;
      case '--priority':
        opts.priority = args[++i];
        break;
      case '--verbose':
        opts.verbose = true;
        break;
      case '--report-dir':
        opts.reportDir = path.resolve(args[++i]);
        break;
      case '--help':
        printHelp();
        process.exit(0);
    }
  }

  return opts;
}

function printHelp(): void {
  console.log(`
Nexus 全栈测试智能体

用法: npx tsx e2e/nexus-test-agent.ts [选项]

选项:
  --mode <all|cargo|e2e|coverage>  运行模式 (默认: all)
  --group <name> [name...]         按模块群筛选 (entity/commission/trading/dispute/storage/grouprobot)
  --flow <id> [id...]              指定 E2E 流程 (T1-T4/E1-E9/D1-D2/G1-G7/S1)
  --pallet <name> [name...]        指定 cargo test pallet
  --priority <P0|P1|P2>            按优先级筛选覆盖率报告
  --verbose                        详细输出
  --report-dir <path>              报告输出目录
  --help                           显示帮助

示例:
  npx tsx e2e/nexus-test-agent.ts                         # 全量
  npx tsx e2e/nexus-test-agent.ts --mode cargo --group entity
  npx tsx e2e/nexus-test-agent.ts --mode e2e --flow E1
  npx tsx e2e/nexus-test-agent.ts --mode coverage --priority P0
`);
}

// ─── 主逻辑 ───────────────────────────────────────────────────

async function main(): Promise<void> {
  const opts = parseArgs();
  const projectRoot = path.resolve(__dirname, '../..');
  const planDir = path.resolve(projectRoot, 'scripts/docs');
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);

  console.log('\n' + '╔' + '═'.repeat(68) + '╗');
  console.log('║' + '  Nexus 全栈测试智能体'.padEnd(68) + '║');
  console.log('╚' + '═'.repeat(68) + '╝');
  console.log(`  模式: ${opts.mode}`);
  if (opts.groups.length) console.log(`  模块群: ${opts.groups.join(', ')}`);
  if (opts.flows.length) console.log(`  流程: ${opts.flows.join(', ')}`);
  if (opts.pallets.length) console.log(`  Pallets: ${opts.pallets.join(', ')}`);
  if (opts.priority) console.log(`  优先级: ${opts.priority}`);

  // 确保报告目录
  if (!fs.existsSync(opts.reportDir)) {
    fs.mkdirSync(opts.reportDir, { recursive: true });
  }

  const report: AgentReport = {
    timestamp: new Date().toISOString(),
    mode: opts.mode,
    cargo: null,
    e2e: null,
    coverage: null,
  };

  // ─── Phase 1: Cargo 单元测试 ───
  if (opts.mode === 'all' || opts.mode === 'cargo') {
    console.log('\n' + '─'.repeat(70));
    console.log('  Phase 1: Cargo 单元测试');
    console.log('─'.repeat(70));

    const pallets = resolvePallets(opts);
    console.log(`  待测 Pallet: ${pallets.length}个\n`);

    const results = await runCargoTests(pallets, projectRoot, {
      onResult: (r) => {
        const icon = r.success ? '✅' : '❌';
        console.log(`  ${icon} ${r.pallet}: ${r.passed} passed, ${r.failed} failed (${(r.duration / 1000).toFixed(1)}s)`);
      },
    });

    printCargoSummary(results);
    report.cargo = results;
  }

  // ─── Phase 2: E2E 链上测试 ───
  if (opts.mode === 'all' || opts.mode === 'e2e') {
    console.log('\n' + '─'.repeat(70));
    console.log('  Phase 2: E2E 链上测试');
    console.log('─'.repeat(70));

    const flows = resolveFlows(opts);
    if (flows.length === 0) {
      console.log('  ℹ 无匹配的 E2E 流程\n');
    } else {
      console.log(`  待运行流程: ${flows.map(f => f.name).join(', ')}\n`);

      try {
        const api = await getApi();
        const chain = await api.rpc.system.chain();
        console.log(`  🔗 已连接: ${chain}\n`);

        const actors = getDevAccounts();
        await fundAccounts(api, actors, 500_000);

        const { reporter, allPassed } = await runFlows(api, actors, flows);

        report.e2e = {
          flows: flows.map(f => f.name),
          passed: allPassed,
          report: JSON.parse(reporter.toJSON()),
        };

        await disconnectApi();
      } catch (err: any) {
        console.error(`  ❌ E2E 连接失败: ${err.message}`);
        console.error('  确保本地节点运行在 ws://127.0.0.1:9944');
        report.e2e = { flows: [], passed: false, report: null, error: err.message };
      }
    }
  }

  // ─── Phase 3: 覆盖率报告 ───
  if (opts.mode === 'all' || opts.mode === 'coverage') {
    console.log('\n' + '─'.repeat(70));
    console.log('  Phase 3: 测试计划覆盖率');
    console.log('─'.repeat(70));

    const cases = parseTestPlan(planDir);
    if (cases.length === 0) {
      console.log('  ⚠ 未找到测试计划文件或无法解析用例');
    } else {
      applyCoverage(cases, COVERAGE_MAP);

      // 如果指定优先级，过滤
      const filtered = opts.priority
        ? cases.filter(c => c.priority === opts.priority)
        : cases;

      const coverageReport = generateCoverageReport(filtered);
      printCoverageReport(coverageReport);

      const coveragePath = path.join(opts.reportDir, `coverage-${timestamp}.json`);
      writeCoverageJSON(coverageReport, coveragePath);
      console.log(`  📄 覆盖率报告: ${coveragePath}`);

      report.coverage = coverageReport;
    }
  }

  // ─── 写出综合报告 ───
  const reportPath = path.join(opts.reportDir, `agent-report-${timestamp}.json`);
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2), 'utf-8');
  console.log(`\n📄 综合报告: ${reportPath}`);

  // ─── 退出码 ───
  const cargoOk = !report.cargo || report.cargo.every(r => r.success);
  const e2eOk = !report.e2e || report.e2e.passed;

  if (cargoOk && e2eOk) {
    console.log('\n✅ 全部测试通过\n');
    process.exit(0);
  } else {
    console.log('\n❌ 存在失败的测试\n');
    process.exit(1);
  }
}

// ─── 辅助函数 ───────────────────────────────────────────────────

interface AgentReport {
  timestamp: string;
  mode: string;
  cargo: CargoTestResult[] | null;
  e2e: { flows: string[]; passed: boolean; report: any; error?: string } | null;
  coverage: any;
}

function resolvePallets(opts: AgentOptions): string[] {
  if (opts.pallets.length > 0) return opts.pallets;

  if (opts.groups.length > 0) {
    const pallets: string[] = [];
    for (const g of opts.groups) {
      const group = PALLET_GROUPS[g];
      if (group) pallets.push(...group);
      else console.warn(`  ⚠ 未知模块群: ${g}`);
    }
    return pallets;
  }

  return [...ALL_PALLETS];
}

function resolveFlows(opts: AgentOptions): FlowDef[] {
  if (opts.flows.length > 0) {
    return opts.flows
      .map(id => FLOW_REGISTRY[id])
      .filter((f): f is FlowDef => {
        if (!f) return false;
        return true;
      });
  }

  if (opts.groups.length > 0) {
    const flowIds: string[] = [];
    for (const g of opts.groups) {
      const ids = FLOW_GROUPS[g];
      if (ids) flowIds.push(...ids);
    }
    return flowIds
      .map(id => FLOW_REGISTRY[id])
      .filter((f): f is FlowDef => !!f);
  }

  // 默认全部
  return Object.values(FLOW_REGISTRY);
}

// ─── 入口 ───────────────────────────────────────────────────────

main().catch((err) => {
  console.error('❌ 未捕获的错误:', err);
  process.exit(1);
});
