#!/usr/bin/env tsx
/**
 * Nexus 业务 Pallet 压力测试 — 测真实业务操作的 block 吞吐上限
 *
 * 测试场景:
 *   A) create_entity   — 最重: 名称唯一性检查 + 资金转账 + 自动创建 shop + 多表写入
 *   B) top_up_fund      — 中等: 读 entity + 转账到金库
 *   C) remark           — 对照组 (最轻)
 *
 * 用法:
 *   npx tsx e2e/stress-test-biz.ts                            # 全部场景各跑 30s
 *   npx tsx e2e/stress-test-biz.ts --scenario entity          # 只跑 create_entity
 *   npx tsx e2e/stress-test-biz.ts --scenario topup           # 只跑 top_up_fund
 *   npx tsx e2e/stress-test-biz.ts --scenario remark          # 对照组
 *   npx tsx e2e/stress-test-biz.ts --duration 60 --concurrency 100
 */

import { ApiPromise, WsProvider } from '@polkadot/api';
import { Keyring } from '@polkadot/keyring';
import { KeyringPair } from '@polkadot/keyring/types';
import { defaultConfig, nex } from './core/config.js';
import { NEXUS_SS58_FORMAT } from '../utils/ss58.js';
import * as fs from 'fs';

// ── Config ──────────────────────────────────────────────────

interface BizStressConfig {
  wsUrl: string;
  durationSec: number;
  concurrency: number;
  scenarios: string[];  // 'entity' | 'topup' | 'remark'
}

interface BlockStat {
  blockNum: number;
  userTxCount: number;
  timestampMs: number;
}

interface ScenarioResult {
  scenario: string;
  elapsedSec: number;
  blocksObserved: number;
  poolSubmitted: number;
  poolAccepted: number;
  poolRejected: number;
  peakTxPerBlock: number;
  avgTxPerBlock: number;
  peakTps: number;
  effectiveTps: number;
  blockInterval: number;
  blocks: BlockStat[];
  errors: Record<string, number>;
}

// ── Helpers ─────────────────────────────────────────────────

function parseArgs(): BizStressConfig {
  const args = process.argv.slice(2);
  const get = (flag: string, fallback: string): string => {
    const idx = args.indexOf(flag);
    return idx !== -1 && args[idx + 1] ? args[idx + 1] : fallback;
  };

  const scenarioArg = get('--scenario', 'all');
  const scenarios = scenarioArg === 'all'
    ? ['remark', 'topup', 'entity']
    : [scenarioArg];

  return {
    wsUrl: process.env.WS_URL ?? defaultConfig.wsUrl,
    durationSec: parseInt(get('--duration', '30'), 10),
    concurrency: parseInt(get('--concurrency', '100'), 10),
    scenarios,
  };
}

function createSenderPool(count: number, prefix: string): KeyringPair[] {
  const keyring = new Keyring({ type: 'sr25519', ss58Format: NEXUS_SS58_FORMAT });
  return Array.from({ length: count }, (_, i) =>
    keyring.addFromUri(`//BizStress/${prefix}${i}`),
  );
}

async function getAccountNonce(api: ApiPromise, address: string): Promise<number> {
  const account = await api.query.system.account(address);
  return (account as any).nonce.toNumber();
}

async function fundSenders(
  api: ApiPromise,
  senders: KeyringPair[],
  alice: KeyringPair,
  amountNex: number,
): Promise<void> {
  const amount = nex(amountNex).toString();
  let currentNonce = await getAccountNonce(api, alice.address);

  const pending: Promise<void>[] = [];
  for (const sender of senders) {
    const n = currentNonce++;
    const p = new Promise<void>((resolve, reject) => {
      const t = setTimeout(() => reject(new Error('Fund timeout')), 180_000);
      api.tx.balances
        .transferKeepAlive(sender.address, amount)
        .signAndSend(alice, { nonce: n }, ({ status }: any) => {
          if (status.isInBlock || status.isFinalized) { clearTimeout(t); resolve(); }
        })
        .catch((err: Error) => { clearTimeout(t); reject(err); });
    });
    pending.push(p);
  }
  await Promise.all(pending);
}

async function getBlockUserTxCount(api: ApiPromise, blockHash: string): Promise<number> {
  try {
    const provider = (api as any)?._rpcCore?.provider;
    if (provider && typeof provider.send === 'function') {
      const raw = await provider.send('chain_getBlock', [blockHash]);
      const exts = raw?.block?.extrinsics;
      if (Array.isArray(exts)) return Math.max(0, exts.length - 1);
    }
  } catch { /* fallback */ }
  try {
    const apiAt = await api.at(blockHash);
    const events = await apiAt.query.system.events();
    let maxIdx = -1;
    for (const r of events as any) {
      if (r.phase?.isApplyExtrinsic) {
        const idx = r.phase.asApplyExtrinsic.toNumber();
        if (idx > maxIdx) maxIdx = idx;
      }
    }
    return Math.max(0, maxIdx);
  } catch { return 0; }
}

// ── Scenario: remark (baseline) ─────────────────────────────

async function firehoseRemark(
  api: ApiPromise,
  sender: KeyringPair,
  idx: number,
  shouldStop: () => boolean,
  outcomes: { ok: number; err: number; errors: Record<string, number> },
): Promise<void> {
  let nonce = await getAccountNonce(api, sender.address);
  let seq = 0;
  while (!shouldStop()) {
    try {
      await api.tx.system.remark(`biz-remark-${idx}-${seq++}`)
        .signAndSend(sender, { nonce: nonce++ });
      outcomes.ok++;
    } catch (e: any) {
      outcomes.err++;
      const k = (e.message ?? '').substring(0, 60);
      outcomes.errors[k] = (outcomes.errors[k] || 0) + 1;
    }
  }
}

// ── Scenario: create_entity ─────────────────────────────────
// MaxEntitiesPerUser=3，每个 sender 最多创建 3 个 entity
// 超出后会报错，但 tx 仍然消耗 block weight

async function firehoseCreateEntity(
  api: ApiPromise,
  sender: KeyringPair,
  idx: number,
  shouldStop: () => boolean,
  outcomes: { ok: number; err: number; errors: Record<string, number> },
): Promise<void> {
  let nonce = await getAccountNonce(api, sender.address);
  let seq = 0;
  while (!shouldStop()) {
    try {
      const name = `BizTest-${idx}-${seq++}-${Date.now()}`;
      await (api.tx as any).entityRegistry.createEntity(
        name,     // name
        null,     // logo_cid
        null,     // description_cid
        null,     // referrer
      ).signAndSend(sender, { nonce: nonce++ });
      outcomes.ok++;
    } catch (e: any) {
      outcomes.err++;
      const k = (e.message ?? '').substring(0, 60);
      outcomes.errors[k] = (outcomes.errors[k] || 0) + 1;
    }
  }
}

// ── Scenario: top_up_fund ───────────────────────────────────
// 先为每个 sender 创建一个 entity，然后反复 top_up_fund

async function setupEntitiesForTopUp(
  api: ApiPromise,
  senders: KeyringPair[],
): Promise<Map<string, number>> {
  // 每个 sender 创建 1 个 entity，返回 sender.address → entity_id
  const entityMap = new Map<string, number>();

  for (const sender of senders) {
    try {
      const nonce = await getAccountNonce(api, sender.address);
      const name = `TopUp-${sender.address.substring(0, 8)}-${Date.now()}`;

      await new Promise<void>((resolve, reject) => {
        const t = setTimeout(() => reject(new Error('create timeout')), 60_000);
        (api.tx as any).entityRegistry.createEntity(name, null, null, null)
          .signAndSend(sender, { nonce }, ({ status, events }: any) => {
            if (status.isInBlock || status.isFinalized) {
              clearTimeout(t);
              // 从事件中提取 entity_id
              if (events) {
                for (const record of events) {
                  const { event } = record;
                  if (event.section === 'entityRegistry' && event.method === 'EntityCreated') {
                    const entityId = event.data[0]?.toNumber?.() ?? event.data[0];
                    if (typeof entityId === 'number') {
                      entityMap.set(sender.address, entityId);
                    }
                  }
                }
              }
              resolve();
            }
          })
          .catch((err: Error) => { clearTimeout(t); reject(err); });
      });
    } catch (e) {
      // 如果创建失败（比如之前已有 entity），查询现有的
      try {
        const userEntities = await (api.query as any).entityRegistry.userEntity(sender.address);
        const ids = userEntities.toJSON();
        if (Array.isArray(ids) && ids.length > 0) {
          entityMap.set(sender.address, ids[0]);
        }
      } catch { /* skip */ }
    }
  }

  return entityMap;
}

async function firehoseTopUpFund(
  api: ApiPromise,
  sender: KeyringPair,
  entityId: number,
  shouldStop: () => boolean,
  outcomes: { ok: number; err: number; errors: Record<string, number> },
): Promise<void> {
  let nonce = await getAccountNonce(api, sender.address);
  const amount = nex(10).toString();
  while (!shouldStop()) {
    try {
      await (api.tx as any).entityRegistry.topUpFund(entityId, amount)
        .signAndSend(sender, { nonce: nonce++ });
      outcomes.ok++;
    } catch (e: any) {
      outcomes.err++;
      const k = (e.message ?? '').substring(0, 60);
      outcomes.errors[k] = (outcomes.errors[k] || 0) + 1;
    }
  }
}

// ── Run single scenario ─────────────────────────────────────

async function runScenario(
  api: ApiPromise,
  scenario: string,
  senders: KeyringPair[],
  durationSec: number,
  entityMap?: Map<string, number>,
): Promise<ScenarioResult> {
  const blockStats: BlockStat[] = [];
  const baseHeader = await api.rpc.chain.getHeader();
  const baseBlock = baseHeader.number.toNumber();

  // 订阅新 block
  const unsubPromise = api.rpc.chain.subscribeNewHeads(async (header) => {
    const blockNum = header.number.toNumber();
    if (blockNum <= baseBlock) return;
    const userTx = await getBlockUserTxCount(api, header.hash.toHex());
    blockStats.push({ blockNum, userTxCount: userTx, timestampMs: Date.now() });
  });

  const outcomes = { ok: 0, err: 0, errors: {} as Record<string, number> };
  let stopped = false;
  const shouldStop = () => stopped;

  const startTime = Date.now();

  // 进度
  const progress = setInterval(() => {
    const elapsed = (Date.now() - startTime) / 1000;
    const recent = blockStats.filter((b) => b.blockNum > baseBlock);
    const peak = recent.length > 0 ? Math.max(...recent.map((b) => b.userTxCount)) : 0;
    process.stdout.write(
      `\r    [${elapsed.toFixed(0)}s] pool=${outcomes.ok + outcomes.err} ok=${outcomes.ok} err=${outcomes.err} blocks=${recent.length} peak=${peak}  `,
    );
  }, 1000);

  // 启动 workers
  const workers = senders.map((sender, i) => {
    switch (scenario) {
      case 'entity':
        return firehoseCreateEntity(api, sender, i, shouldStop, outcomes);
      case 'topup':
        const eid = entityMap?.get(sender.address);
        if (eid === undefined) return Promise.resolve(); // 无 entity 跳过
        return firehoseTopUpFund(api, sender, eid, shouldStop, outcomes);
      default: // remark
        return firehoseRemark(api, sender, i, shouldStop, outcomes);
    }
  });

  // 定时停止
  setTimeout(() => { stopped = true; }, durationSec * 1000);
  await Promise.all(workers);
  clearInterval(progress);

  // drain
  process.stdout.write('\n    Draining...');
  await new Promise<void>((resolve) => {
    let seen = 0;
    const unsub = api.rpc.chain.subscribeNewHeads(() => {
      seen++;
      if (seen >= 3) { unsub.then((u) => u()); resolve(); }
    });
  });
  console.log(' done');

  const unsub = await unsubPromise;
  unsub();

  const elapsed = (Date.now() - startTime) / 1000;
  const testBlocks = blockStats.filter((b) => b.blockNum > baseBlock);
  const userTxCounts = testBlocks.map((b) => b.userTxCount);
  const peak = userTxCounts.length > 0 ? Math.max(...userTxCounts) : 0;
  const total = userTxCounts.reduce((a, b) => a + b, 0);
  const avg = userTxCounts.length > 0
    ? Math.round((total / userTxCounts.length) * 100) / 100
    : 0;

  let blockInterval = 6;
  if (testBlocks.length >= 2) {
    const intervals: number[] = [];
    for (let i = 1; i < testBlocks.length; i++) {
      intervals.push((testBlocks[i].timestampMs - testBlocks[i - 1].timestampMs) / 1000);
    }
    blockInterval = Math.round((intervals.reduce((a, b) => a + b, 0) / intervals.length) * 100) / 100;
  }

  return {
    scenario,
    elapsedSec: Math.round(elapsed * 100) / 100,
    blocksObserved: testBlocks.length,
    poolSubmitted: outcomes.ok + outcomes.err,
    poolAccepted: outcomes.ok,
    poolRejected: outcomes.err,
    peakTxPerBlock: peak,
    avgTxPerBlock: avg,
    peakTps: blockInterval > 0 ? Math.round((peak / blockInterval) * 100) / 100 : 0,
    effectiveTps: blockInterval > 0 ? Math.round((avg / blockInterval) * 100) / 100 : 0,
    blockInterval,
    blocks: testBlocks,
    errors: outcomes.errors,
  };
}

// ── Main ────────────────────────────────────────────────────

async function main() {
  const config = parseArgs();

  console.log('\n' + '='.repeat(65));
  console.log('  Nexus Business Pallet Stress Test');
  console.log('='.repeat(65));

  const provider = new WsProvider(config.wsUrl);
  const api = await ApiPromise.create({ provider });
  const chain = await api.rpc.system.chain();
  const version = await api.rpc.system.version();
  console.log(`\n  Chain: ${chain} (${version})`);
  console.log(`  Duration: ${config.durationSec}s per scenario`);
  console.log(`  Concurrency: ${config.concurrency} senders`);
  console.log(`  Scenarios: ${config.scenarios.join(', ')}`);

  const keyring = new Keyring({ type: 'sr25519', ss58Format: NEXUS_SS58_FORMAT });
  const alice = keyring.addFromUri('//Alice');

  const results: ScenarioResult[] = [];

  for (const scenario of config.scenarios) {
    console.log(`\n${'─'.repeat(65)}`);
    console.log(`  Scenario: ${scenario.toUpperCase()}`);
    console.log(`${'─'.repeat(65)}`);

    // 每个场景用不同的 sender 前缀，避免 nonce / entity 冲突
    const prefix = `${scenario}-`;
    const senders = createSenderPool(config.concurrency, prefix);

    console.log(`  Funding ${senders.length} senders...`);
    // create_entity 需要 50 USDT 等值 NEX，给多点
    const fundAmount = scenario === 'entity' ? 500_000 : 100_000;
    await fundSenders(api, senders, alice, fundAmount);
    console.log('  Funded.');

    let entityMap: Map<string, number> | undefined;
    if (scenario === 'topup') {
      console.log('  Setting up entities for top_up_fund...');
      entityMap = await setupEntitiesForTopUp(api, senders);
      console.log(`  Created ${entityMap.size} entities.`);
    }

    console.log(`  Running ${scenario} for ${config.durationSec}s...`);
    const result = await runScenario(api, scenario, senders, config.durationSec, entityMap);
    results.push(result);
  }

  // ── 汇总 ──
  console.log('\n\n' + '='.repeat(65));
  console.log('  COMPARISON RESULTS');
  console.log('='.repeat(65));
  console.log('');
  console.log(
    '  ' +
    'Scenario'.padEnd(15) +
    'Peak/blk'.padStart(10) +
    'Avg/blk'.padStart(10) +
    'Peak TPS'.padStart(10) +
    'Eff TPS'.padStart(10) +
    'Interval'.padStart(10) +
    'Reject%'.padStart(10),
  );
  console.log('  ' + '─'.repeat(75));

  for (const r of results) {
    console.log(
      '  ' +
      r.scenario.padEnd(15) +
      String(r.peakTxPerBlock).padStart(10) +
      String(r.avgTxPerBlock).padStart(10) +
      String(r.peakTps).padStart(10) +
      String(r.effectiveTps).padStart(10) +
      `${r.blockInterval}s`.padStart(10) +
      `${Math.round(r.poolRejected / Math.max(1, r.poolSubmitted) * 100)}%`.padStart(10),
    );
  }
  console.log('  ' + '─'.repeat(75));

  // Per-scenario block breakdown
  for (const r of results) {
    console.log(`\n  [${r.scenario}] Per-block:`);
    for (const b of r.blocks) {
      const bar = '\u2588'.repeat(Math.min(Math.round(b.userTxCount / 50), 60));
      console.log(`    #${String(b.blockNum).padStart(7)} | ${String(b.userTxCount).padStart(5)} tx | ${bar}`);
    }
    if (Object.keys(r.errors).length > 0) {
      console.log(`  Errors:`);
      for (const [msg, count] of Object.entries(r.errors).sort((a, b) => b[1] - a[1]).slice(0, 3)) {
        console.log(`    [${count}x] ${msg}`);
      }
    }
  }

  console.log('');

  // Write report
  const reportPath = `biz-stress-report-${Date.now()}.json`;
  fs.writeFileSync(reportPath, JSON.stringify(results, null, 2), 'utf-8');
  console.log(`  Report: ${reportPath}`);

  await api.disconnect();
  process.exit(0);
}

main().catch((err) => {
  console.error('Fatal error:', err);
  process.exit(1);
});
