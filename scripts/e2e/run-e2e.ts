#!/usr/bin/env tsx
/**
 * Nexus E2E 测试主入口
 *
 * 用法:
 *   npx tsx e2e/run-e2e.ts              # 运行全部 Phase 1 流程
 *   npx tsx e2e/run-e2e.ts --flow T1    # 只运行 Flow-T1
 *   npx tsx e2e/run-e2e.ts --flow T2 T3 # 运行 T2 + T3
 */

import { getApi, disconnectApi } from './core/chain-state.js';
import { runFlows, FlowDef } from './core/test-runner.js';
import { getDevAccounts, fundAccounts } from './fixtures/accounts.js';

// Flow 导入
import { makerLifecycleFlow } from './flows/trading/maker-lifecycle.js';
import { p2pBuyFlow } from './flows/trading/p2p-buy.js';
import { p2pSellFlow } from './flows/trading/p2p-sell.js';
import { entityShopFlow } from './flows/entity/entity-shop.js';

/** 全部 Phase 1 流程 (按依赖顺序) */
const ALL_FLOWS: FlowDef[] = [
  makerLifecycleFlow,  // T1: 做市商 (无依赖)
  p2pBuyFlow,          // T2: P2P Buy (依赖 T1)
  p2pSellFlow,         // T3: P2P Sell (依赖 T1)
  entityShopFlow,      // E1: 实体→店铺 (无依赖)
];

const FLOW_MAP: Record<string, FlowDef> = {
  T1: makerLifecycleFlow,
  T2: p2pBuyFlow,
  T3: p2pSellFlow,
  E1: entityShopFlow,
};

function parseArgs(): FlowDef[] {
  const args = process.argv.slice(2);
  const flowIdx = args.indexOf('--flow');

  if (flowIdx === -1) return ALL_FLOWS;

  const flowNames = args.slice(flowIdx + 1).filter((a) => !a.startsWith('--'));
  if (flowNames.length === 0) {
    console.log('可用流程: T1, T2, T3, E1');
    process.exit(0);
  }

  const selected: FlowDef[] = [];
  for (const name of flowNames) {
    const flow = FLOW_MAP[name.toUpperCase()];
    if (!flow) {
      console.error(`未知流程: ${name}. 可用: ${Object.keys(FLOW_MAP).join(', ')}`);
      process.exit(1);
    }
    selected.push(flow);
  }
  return selected;
}

async function main() {
  console.log('\n' + '='.repeat(70));
  console.log('  Nexus E2E 测试 — Phase 1');
  console.log('='.repeat(70));

  const flows = parseArgs();
  console.log(`\n📋 待运行流程: ${flows.map((f) => f.name).join(', ')}`);

  // 连接链
  const api = await getApi();
  const chain = await api.rpc.system.chain();
  const version = await api.rpc.system.version();
  console.log(`🔗 已连接: ${chain} (${version})\n`);

  // 准备账户
  const actors = getDevAccounts();

  // 确保测试账户有余额
  console.log('💰 检查/补充测试账户余额...');
  await fundAccounts(api, actors, 500_000);

  // 运行流程
  const { reporter, allPassed } = await runFlows(api, actors, flows);

  // 写出 JSON 报告
  const reportPath = `e2e-report-${Date.now()}.json`;
  const fs = await import('fs');
  fs.writeFileSync(reportPath, reporter.toJSON(), 'utf-8');
  console.log(`\n📄 JSON 报告: ${reportPath}`);

  // 断开连接
  await disconnectApi();

  process.exit(allPassed ? 0 : 1);
}

main().catch((err) => {
  console.error('❌ 未捕获的错误:', err);
  process.exit(1);
});
