#!/usr/bin/env tsx
/**
 * Nexus 压力测试 v2 — 探测单 block 吞吐上限
 *
 * 策略:
 *   1. 创建 N 个 sender，各自用递增 nonce 尽可能快地提交 tx（不等确认）
 *   2. 订阅 newHeads，每出一个 block 就查询该 block 包含了多少笔 extrinsic
 *   3. 持续 D 秒后停止提交，等 pool 清空，输出 per-block 统计
 *
 * 用法:
 *   npx tsx e2e/stress-test.ts --duration 30 --concurrency 50
 *   npx tsx e2e/stress-test.ts --duration 60 --concurrency 100 --type remark
 *   npx tsx e2e/stress-test.ts --total 500 --concurrency 50
 */

import { ApiPromise, WsProvider } from '@polkadot/api';
import { Keyring } from '@polkadot/keyring';
import { KeyringPair } from '@polkadot/keyring/types';
import { defaultConfig, nex } from './core/config.js';
import { NEXUS_SS58_FORMAT } from '../utils/ss58.js';
import * as fs from 'fs';

// ── Types ───────────────────────────────────────────────────

interface StressConfig {
  wsUrl: string;
  totalTx: number;
  concurrency: number;
  txType: 'remark' | 'transfer' | 'mixed';
  durationSec: number | null;
}

interface BlockStat {
  blockNum: number;
  blockHash: string;
  extrinsicCount: number;   // 总 extrinsic (含 inherent)
  userTxCount: number;       // 用户提交的 tx (减去 inherent)
  timestampMs: number;
}

interface TxOutcome {
  submitted: boolean;
  error?: string;
}

interface StressReport {
  config: StressConfig;
  startTime: string;
  endTime: string;
  elapsedSec: number;
  totalSubmitted: number;
  totalAccepted: number;
  totalRejected: number;
  rejectRate: number;
  blocks: BlockStat[];
  peakTxPerBlock: number;
  avgTxPerBlock: number;
  effectiveTps: number;
  peakTps: number;
  errors: Record<string, number>;
}

// ── Helpers ─────────────────────────────────────────────────

function parseArgs(): StressConfig {
  const args = process.argv.slice(2);
  const get = (flag: string, fallback: string): string => {
    const idx = args.indexOf(flag);
    return idx !== -1 && args[idx + 1] ? args[idx + 1] : fallback;
  };

  return {
    wsUrl: process.env.WS_URL ?? defaultConfig.wsUrl,
    totalTx: parseInt(get('--total', '0'), 10),
    concurrency: parseInt(get('--concurrency', '50'), 10),
    txType: get('--type', 'remark') as StressConfig['txType'],
    durationSec: args.includes('--duration')
      ? parseInt(get('--duration', '30'), 10)
      : (!args.includes('--total') ? 30 : null),
  };
}

// ── Account Pool ────────────────────────────────────────────

function createSenderPool(count: number): KeyringPair[] {
  const keyring = new Keyring({ type: 'sr25519', ss58Format: NEXUS_SS58_FORMAT });
  return Array.from({ length: count }, (_, i) =>
    keyring.addFromUri(`//StressV2/S${i}`),
  );
}

async function getAccountNonce(api: ApiPromise, address: string): Promise<number> {
  const account = await api.query.system.account(address);
  return (account as any).nonce.toNumber();
}

async function fundSenderPool(
  api: ApiPromise,
  senders: KeyringPair[],
  alice: KeyringPair,
): Promise<void> {
  const amount = nex(50_000).toString();
  let nonce = await getAccountNonce(api, alice.address);

  const pending: Promise<void>[] = [];
  for (const sender of senders) {
    const n = nonce++;
    const p = new Promise<void>((resolve, reject) => {
      const t = setTimeout(() => reject(new Error('Fund timeout')), 180_000);
      api.tx.balances
        .transferKeepAlive(sender.address, amount)
        .signAndSend(alice, { nonce: n }, ({ status }: any) => {
          if (status.isInBlock || status.isFinalized) {
            clearTimeout(t);
            resolve();
          }
        })
        .catch((err: Error) => { clearTimeout(t); reject(err); });
    });
    pending.push(p);
  }

  await Promise.all(pending);
}

// ── Block Observer ──────────────────────────────────────────

async function getBlockExtrinsicCount(
  api: ApiPromise,
  blockHash: string,
): Promise<{ total: number; user: number }> {
  // 使用 raw RPC 获取区块，避免 extrinsic v5 解码问题
  try {
    const provider = (api as any)?._rpcCore?.provider;
    if (provider && typeof provider.send === 'function') {
      const raw = await provider.send('chain_getBlock', [blockHash]);
      const extrinsics = raw?.block?.extrinsics;
      if (Array.isArray(extrinsics)) {
        // inherent 通常有 1-2 个 (timestamp, 可能还有 aura)
        const total = extrinsics.length;
        const user = Math.max(0, total - 1); // 减去 timestamp inherent
        return { total, user };
      }
    }
  } catch { /* fallback */ }

  // Fallback: 通过事件计数
  try {
    const apiAt = await api.at(blockHash);
    const events = await apiAt.query.system.events();
    let maxExtIdx = -1;
    for (const record of events as any) {
      if (record.phase?.isApplyExtrinsic) {
        const idx = record.phase.asApplyExtrinsic.toNumber();
        if (idx > maxExtIdx) maxExtIdx = idx;
      }
    }
    const total = maxExtIdx + 1;
    return { total, user: Math.max(0, total - 1) };
  } catch {
    return { total: 0, user: 0 };
  }
}

// ── Fire-and-Forget Sender ──────────────────────────────────

async function firehoseSender(
  api: ApiPromise,
  senderIdx: number,
  sender: KeyringPair,
  allSenders: KeyringPair[],
  txType: StressConfig['txType'],
  shouldStop: () => boolean,
  outcomes: TxOutcome[],
): Promise<void> {
  let nonce = await getAccountNonce(api, sender.address);
  let txCount = 0;

  while (!shouldStop()) {
    const currentNonce = nonce++;
    try {
      const tx = txType === 'transfer'
        ? api.tx.balances.transferKeepAlive(
            allSenders[(senderIdx + 1) % allSenders.length].address,
            nex(1).toString(),
          )
        : txType === 'mixed' && txCount % 2 === 1
          ? api.tx.balances.transferKeepAlive(
              allSenders[(senderIdx + 1) % allSenders.length].address,
              nex(1).toString(),
            )
          : api.tx.system.remark(`s${senderIdx}-${txCount}-${Date.now()}`);

      // Fire-and-forget: 只等提交到 pool，不等 inBlock
      await tx.signAndSend(sender, { nonce: currentNonce });
      outcomes.push({ submitted: true });
    } catch (err: any) {
      outcomes.push({ submitted: false, error: err.message ?? String(err) });
      // 如果 nonce 被拒，不递增（回退）
      // 但大部分情况下 pool 满是 1010 错误，nonce 没被消耗
    }
    txCount++;
  }
}

// ── Main ────────────────────────────────────────────────────

async function main() {
  const config = parseArgs();
  // 如果只给了 --total 没给 --duration，用 total 模式
  const isTimeBased = config.durationSec !== null && config.durationSec > 0;
  const isTotalBased = !isTimeBased && config.totalTx > 0;

  if (!isTimeBased && !isTotalBased) {
    // 默认 30 秒
    config.durationSec = 30;
  }

  console.log('\n' + '='.repeat(65));
  console.log('  Nexus Stress Test v2 — Block Throughput Profiler');
  console.log('='.repeat(65));

  const provider = new WsProvider(config.wsUrl);
  const api = await ApiPromise.create({ provider });
  const chain = await api.rpc.system.chain();
  const version = await api.rpc.system.version();
  console.log(`\n  Chain: ${chain} (${version})`);

  // Create & fund senders
  const senderCount = config.concurrency;
  console.log(`  Creating ${senderCount} sender accounts...`);
  const senders = createSenderPool(senderCount);

  const keyring = new Keyring({ type: 'sr25519', ss58Format: NEXUS_SS58_FORMAT });
  const alice = keyring.addFromUri('//Alice');
  console.log('  Funding sender accounts...');
  await fundSenderPool(api, senders, alice);
  console.log('  All senders funded.\n');

  // ── Phase 1: 获取 baseline block number ──
  const baseHeader = await api.rpc.chain.getHeader();
  const baseBlock = baseHeader.number.toNumber();
  console.log(`  Baseline block: #${baseBlock}`);

  // ── Phase 2: Subscribe to new blocks ──
  const blockStats: BlockStat[] = [];
  const unsubPromise = api.rpc.chain.subscribeNewHeads(async (header) => {
    const blockNum = header.number.toNumber();
    const blockHash = header.hash.toHex();
    const { total, user } = await getBlockExtrinsicCount(api, blockHash);
    blockStats.push({
      blockNum,
      blockHash,
      extrinsicCount: total,
      userTxCount: user,
      timestampMs: Date.now(),
    });
  });

  // ── Phase 3: Fire tx as fast as possible ──
  const outcomes: TxOutcome[] = [];
  let stopped = false;
  const shouldStop = () => stopped;

  const startTime = new Date();
  let totalLimit = isTotalBased ? config.totalTx : Infinity;
  let submittedCount = 0;

  // 对 total 模式，用一个计数器做停止条件
  const totalShouldStop = () => {
    if (isTotalBased) return outcomes.length >= totalLimit;
    return stopped;
  };

  console.log(
    `  Mode: ${isTimeBased || (!isTotalBased) ? `${config.durationSec}s duration` : `${config.totalTx} tx limit`}`,
  );
  console.log(`  Concurrency: ${senderCount} senders (fire-and-forget)`);
  console.log(`  Tx type: ${config.txType}`);
  console.log('');

  // 进度打印
  const progressInterval = setInterval(() => {
    const elapsed = (Date.now() - startTime.getTime()) / 1000;
    const accepted = outcomes.filter((o) => o.submitted).length;
    const rejected = outcomes.length - accepted;
    const recentBlocks = blockStats.filter((b) => b.blockNum > baseBlock);
    const peakTx = recentBlocks.length > 0
      ? Math.max(...recentBlocks.map((b) => b.userTxCount))
      : 0;
    process.stdout.write(
      `\r  [${elapsed.toFixed(0)}s] pool_submitted=${outcomes.length} accepted=${accepted} rejected=${rejected} blocks=${recentBlocks.length} peak_tx/block=${peakTx}  `,
    );
  }, 1000);

  // 启动所有 sender
  const workers = senders.map((sender, i) =>
    firehoseSender(api, i, sender, senders, config.txType, totalShouldStop, outcomes),
  );

  // 定时停止
  if (isTimeBased || !isTotalBased) {
    const dur = config.durationSec ?? 30;
    setTimeout(() => { stopped = true; }, dur * 1000);
  }

  await Promise.all(workers);
  stopped = true;
  clearInterval(progressInterval);

  // 等几个 block 让 pool 中剩余 tx 被打包
  console.log('\n\n  Draining tx pool (waiting 4 blocks)...');
  await new Promise<void>((resolve) => {
    let seen = 0;
    const unsub = api.rpc.chain.subscribeNewHeads(() => {
      seen++;
      if (seen >= 4) {
        unsub.then((u) => u());
        resolve();
      }
    });
  });

  // 停止订阅
  const unsub = await unsubPromise;
  unsub();

  const endTime = new Date();
  const elapsedSec = (endTime.getTime() - startTime.getTime()) / 1000;

  // ── 统计 ──
  const testBlocks = blockStats.filter((b) => b.blockNum > baseBlock);
  const accepted = outcomes.filter((o) => o.submitted).length;
  const rejected = outcomes.length - accepted;
  const errors: Record<string, number> = {};
  for (const o of outcomes) {
    if (!o.submitted && o.error) {
      // 截取关键部分
      const key = o.error.length > 80 ? o.error.substring(0, 80) : o.error;
      errors[key] = (errors[key] || 0) + 1;
    }
  }

  const userTxCounts = testBlocks.map((b) => b.userTxCount);
  const peakTxPerBlock = userTxCounts.length > 0 ? Math.max(...userTxCounts) : 0;
  const totalUserTx = userTxCounts.reduce((a, b) => a + b, 0);
  const avgTxPerBlock = userTxCounts.length > 0
    ? Math.round((totalUserTx / userTxCounts.length) * 100) / 100
    : 0;

  // 计算 block 间隔来得出真实 TPS
  let blockIntervalSec = 6; // default
  if (testBlocks.length >= 2) {
    const intervals: number[] = [];
    for (let i = 1; i < testBlocks.length; i++) {
      intervals.push((testBlocks[i].timestampMs - testBlocks[i - 1].timestampMs) / 1000);
    }
    blockIntervalSec = intervals.reduce((a, b) => a + b, 0) / intervals.length;
  }

  const effectiveTps = blockIntervalSec > 0
    ? Math.round((avgTxPerBlock / blockIntervalSec) * 100) / 100
    : 0;
  const peakTps = blockIntervalSec > 0
    ? Math.round((peakTxPerBlock / blockIntervalSec) * 100) / 100
    : 0;

  const report: StressReport = {
    config,
    startTime: startTime.toISOString(),
    endTime: endTime.toISOString(),
    elapsedSec: Math.round(elapsedSec * 100) / 100,
    totalSubmitted: outcomes.length,
    totalAccepted: accepted,
    totalRejected: rejected,
    rejectRate: outcomes.length > 0
      ? Math.round((rejected / outcomes.length) * 10000) / 100
      : 0,
    blocks: testBlocks,
    peakTxPerBlock,
    avgTxPerBlock,
    effectiveTps,
    peakTps,
    errors,
  };

  // Print
  console.log('\n' + '='.repeat(65));
  console.log('  Results');
  console.log('='.repeat(65));
  console.log(`  Elapsed:            ${report.elapsedSec}s`);
  console.log(`  Block interval:     ${blockIntervalSec.toFixed(2)}s`);
  console.log(`  Blocks observed:    ${testBlocks.length}`);
  console.log('');
  console.log(`  Pool submitted:     ${report.totalSubmitted}`);
  console.log(`  Pool accepted:      ${report.totalAccepted}`);
  console.log(`  Pool rejected:      ${report.totalRejected} (${report.rejectRate}%)`);
  console.log('');
  console.log(`  Peak tx/block:      ${report.peakTxPerBlock}`);
  console.log(`  Avg  tx/block:      ${report.avgTxPerBlock}`);
  console.log(`  Effective TPS:      ${report.effectiveTps}`);
  console.log(`  Peak TPS:           ${report.peakTps}`);
  console.log('');

  console.log('  Per-block breakdown:');
  console.log('  ' + '-'.repeat(50));
  console.log('  Block#     | Total ext | User tx | ');
  console.log('  ' + '-'.repeat(50));
  for (const b of testBlocks) {
    const bar = '\u2588'.repeat(Math.min(b.userTxCount, 60));
    console.log(
      `  #${String(b.blockNum).padStart(8)} |    ${String(b.extrinsicCount).padStart(5)} |  ${String(b.userTxCount).padStart(5)} | ${bar}`,
    );
  }
  console.log('  ' + '-'.repeat(50));

  if (Object.keys(report.errors).length > 0) {
    console.log('\n  Pool errors:');
    for (const [msg, count] of Object.entries(report.errors).sort((a, b) => b[1] - a[1]).slice(0, 10)) {
      console.log(`    [${count}x] ${msg}`);
    }
  }
  console.log('');

  // Write report
  const reportPath = `stress-report-${Date.now()}.json`;
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2), 'utf-8');
  console.log(`  Report: ${reportPath}`);

  await api.disconnect();
  process.exit(report.rejectRate > 50 ? 1 : 0);
}

main().catch((err) => {
  console.error('Fatal error:', err);
  process.exit(1);
});
