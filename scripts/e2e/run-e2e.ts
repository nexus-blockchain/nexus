#!/usr/bin/env tsx
/**
 * Nexus E2E 测试主入口
 *
 * 用法:
 *   npx tsx e2e/run-e2e.ts                    # 运行全部流程
 *   npx tsx e2e/run-e2e.ts --phase 1          # 只运行 Phase 1
 *   npx tsx e2e/run-e2e.ts --phase 2          # 只运行 Phase 2
 *   npx tsx e2e/run-e2e.ts --phase 3          # 只运行 Phase 3
 *   npx tsx e2e/run-e2e.ts --flow T1          # 只运行 Flow-T1
 *   npx tsx e2e/run-e2e.ts --flow T2 T3       # 运行 T2 + T3
 *   npx tsx e2e/run-e2e.ts --flow E1 E2 E3    # 运行多个 Entity 流程
 */

import { getApi, disconnectApi } from './core/chain-state.js';
import { runFlows, FlowDef } from './core/test-runner.js';
import { getDevAccounts, fundAccounts } from './fixtures/accounts.js';
import { bootstrapDevChain } from './fixtures/bootstrap.js';

// ── Trading flows ────────────────────────────────────────────
import { makerLifecycleFlow } from './flows/trading/maker-lifecycle.js';
import { p2pBuyFlow } from './flows/trading/p2p-buy.js';
import { p2pSellFlow } from './flows/trading/p2p-sell.js';
import { nexMarketFlow } from './flows/trading/nex-market.js';
import { nexMarketAdminFlow } from './flows/trading/nex-market-admin.js';

// ── Entity flows ─────────────────────────────────────────────
import { entityShopFlow } from './flows/entity/entity-shop.js';
import { orderLifecycleFlow } from './flows/entity/order-lifecycle.js';
import { memberReferralFlow } from './flows/entity/member-referral.js';
import { commissionFlow } from './flows/entity/commission.js';
import { tokenGovernanceFlow } from './flows/entity/token-governance.js';
import { kycFlow } from './flows/entity/kyc.js';
import { tokenSaleFlow } from './flows/entity/token-sale.js';
import { entityDisclosureFlow } from './flows/entity/entity-disclosure.js';
import { orderAdminFlow } from './flows/entity/order-admin.js';
import { tokenGovernanceAdminFlow } from './flows/entity/token-governance-admin.js';

// ── Dispute flows ────────────────────────────────────────────
import { disputeFlow } from './flows/dispute/dispute-resolution.js';
import { escrowFlow } from './flows/dispute/escrow.js';
import { arbitrationAppealFlow } from './flows/dispute/arbitration-appeal.js';

// ── GroupRobot flows ─────────────────────────────────────────
import { botLifecycleFlow } from './flows/grouprobot/bot-lifecycle.js';
import { nodeConsensusFlow } from './flows/grouprobot/node-consensus.js';
import { adCampaignFlow } from './flows/grouprobot/ad-campaign.js';
import { adsCorePreferencesFlow } from './flows/grouprobot/ads-core-preferences.js';
import { subscriptionFlow } from './flows/grouprobot/subscription.js';
import { communityFlow } from './flows/grouprobot/community.js';
import { ceremonyFlow } from './flows/grouprobot/ceremony.js';
import { rewardsFlow } from './flows/grouprobot/rewards.js';

// ── Storage flows ────────────────────────────────────────────
import { storageServiceFlow } from './flows/storage/storage-service.js';
import { storageBillingDisputeFlow } from './flows/storage/storage-billing-dispute.js';

// ═════════════════════════════════════════════════════════════
// Phase 分组
// ═════════════════════════════════════════════════════════════

/** Phase 1: 核心交易 + 实体基础 (按依赖顺序) */
const PHASE1_FLOWS: FlowDef[] = [
  makerLifecycleFlow,  // T1: 做市商 (无依赖)
  p2pBuyFlow,          // T2: P2P Buy (依赖 T1)
  p2pSellFlow,         // T3: P2P Sell (依赖 T1)
  entityShopFlow,      // E1: 实体→店铺 (无依赖)
];

/** Phase 2: 实体扩展 + 交易/治理/争议增量 + 托管 */
const PHASE2_FLOWS: FlowDef[] = [
  orderLifecycleFlow,     // E2: 订单生命周期
  memberReferralFlow,     // E3: 会员推荐
  commissionFlow,         // E4: 佣金返佣
  tokenGovernanceFlow,    // E5: Token+治理
  kycFlow,                // E6: KYC 认证
  tokenSaleFlow,          // E7: 代币发售
  entityDisclosureFlow,   // E9: 信息披露
  orderAdminFlow,         // E10: 订单治理/维护
  tokenGovernanceAdminFlow, // E11: Token/Governance 管理
  nexMarketFlow,          // T4: NEX 市场 (DEX)
  nexMarketAdminFlow,     // T5: NEX Market 管理/争议
  disputeFlow,            // D1: 争议解决
  escrowFlow,             // D2: 托管
  arbitrationAppealFlow,  // D3: 仲裁申诉
];

/** Phase 3: GroupRobot/Ads + 存储 */
const PHASE3_FLOWS: FlowDef[] = [
  botLifecycleFlow,    // G1: Bot 生命周期
  nodeConsensusFlow,   // G2: 节点共识
  adCampaignFlow,      // G3: 广告活动
  adsCorePreferencesFlow, // A1: Ads Core 偏好/确认
  subscriptionFlow,    // G4: 订阅服务
  communityFlow,       // G5: 社区管理
  ceremonyFlow,        // G6: 仪式验证
  rewardsFlow,         // G7: 奖励分配
  storageServiceFlow,  // S1: 存储服务
  storageBillingDisputeFlow, // S2: 存储计费/Slash 争议
];

/** 全部流程 */
const ALL_FLOWS: FlowDef[] = [
  ...PHASE1_FLOWS,
  ...PHASE2_FLOWS,
  ...PHASE3_FLOWS,
];

const PHASE_MAP: Record<string, FlowDef[]> = {
  '1': PHASE1_FLOWS,
  '2': PHASE2_FLOWS,
  '3': PHASE3_FLOWS,
};

const FLOW_MAP: Record<string, FlowDef> = {
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

function parseArgs(): { flows: FlowDef[]; label: string } {
  const args = process.argv.slice(2);

  // --phase N
  const phaseIdx = args.indexOf('--phase');
  if (phaseIdx !== -1) {
    const phaseNum = args[phaseIdx + 1];
    const phaseFlows = PHASE_MAP[phaseNum];
    if (!phaseFlows) {
      console.error(`未知 Phase: ${phaseNum}. 可用: 1, 2, 3`);
      process.exit(1);
    }
    return { flows: phaseFlows, label: `Phase ${phaseNum}` };
  }

  // --flow X Y Z
  const flowIdx = args.indexOf('--flow');
  if (flowIdx !== -1) {
    const flowNames = args.slice(flowIdx + 1).filter((a) => !a.startsWith('--'));
    if (flowNames.length === 0) {
      console.log(`可用流程: ${Object.keys(FLOW_MAP).join(', ')}`);
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
    return { flows: selected, label: `自选 (${flowNames.join(', ')})` };
  }

  return { flows: ALL_FLOWS, label: '全部 (Phase 1+2+3)' };
}

async function main() {
  console.log('\n' + '='.repeat(70));
  console.log('  Nexus E2E 测试');
  console.log('='.repeat(70));

  const { flows, label } = parseArgs();
  console.log(`\n📋 运行范围: ${label}`);
  console.log(`   流程数: ${flows.length}`);
  console.log(`   流程: ${flows.map((f) => f.name).join(', ')}`);

  // 连接链
  const api = await getApi();
  const chain = await api.rpc.system.chain();
  const version = await api.rpc.system.version();
  console.log(`\n🔗 已连接: ${chain} (${version})\n`);

  // 准备账户
  const actors = getDevAccounts();

  // 确保测试账户有余额
  console.log('💰 检查/补充测试账户余额...');
  await fundAccounts(api, actors, 500_000);

  // 引导链状态 (设置初始价格等)
  await bootstrapDevChain(api, actors.alice);

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
