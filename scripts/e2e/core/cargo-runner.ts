/**
 * Cargo Test 运行器 — 执行 Rust 单元测试并解析结果
 */

import { spawn } from 'child_process';
import * as path from 'path';

export interface CargoTestResult {
  pallet: string;
  passed: number;
  failed: number;
  ignored: number;
  duration: number;
  output: string;
  success: boolean;
  errors: string[];
}

/** 所有可测试的 pallet 包名 */
export const ALL_PALLETS = [
  // Entity 模块群
  'pallet-entity-registry',
  'pallet-entity-shop',
  'pallet-entity-product',
  'pallet-entity-order',
  'pallet-entity-review',
  'pallet-entity-token',
  'pallet-entity-governance',
  'pallet-entity-member',
  'pallet-entity-market',
  'pallet-entity-disclosure',
  'pallet-entity-kyc',
  'pallet-entity-tokensale',
  // Commission 模块群
  'pallet-commission-common',
  'pallet-commission-core',
  'pallet-commission-referral',
  'pallet-commission-level-diff',
  'pallet-commission-single-line',
  // Trading 模块群
  'pallet-nex-market',
  // Dispute 模块群
  'pallet-dispute-escrow',
  'pallet-dispute-evidence',
  'pallet-dispute-arbitration',
  // Storage 模块群
  'pallet-storage-service',
  // GroupRobot 模块群
  'pallet-grouprobot-registry',
  'pallet-grouprobot-consensus',
  'pallet-grouprobot-subscription',
  'pallet-grouprobot-community',
  'pallet-grouprobot-ceremony',
  'pallet-grouprobot-rewards',
  'pallet-grouprobot-ads',
] as const;

export type PalletName = typeof ALL_PALLETS[number];

/** 按模块群分组 */
export const PALLET_GROUPS: Record<string, readonly PalletName[]> = {
  entity: ALL_PALLETS.filter(p => p.startsWith('pallet-entity-')),
  commission: ALL_PALLETS.filter(p => p.startsWith('pallet-commission-')),
  trading: ['pallet-nex-market'],
  dispute: ['pallet-dispute-escrow', 'pallet-dispute-evidence', 'pallet-dispute-arbitration'],
  storage: ['pallet-storage-service'],
  grouprobot: ALL_PALLETS.filter(p => p.startsWith('pallet-grouprobot-')),
};

/**
 * 运行单个 pallet 的 cargo test
 */
export function runCargoTest(
  pallet: string,
  projectRoot: string,
  filter?: string,
): Promise<CargoTestResult> {
  return new Promise((resolve) => {
    const args = ['test', '-p', pallet, '--', '--color=never'];
    if (filter) args.push('--test-threads=1', filter);

    const start = Date.now();
    let output = '';

    const child = spawn('cargo', args, {
      cwd: projectRoot,
      env: { ...process.env, RUST_BACKTRACE: '1' },
    });

    child.stdout.on('data', (data: Buffer) => { output += data.toString(); });
    child.stderr.on('data', (data: Buffer) => { output += data.toString(); });

    child.on('close', (code) => {
      const duration = Date.now() - start;
      const { passed, failed, ignored, errors } = parseCargoOutput(output);

      resolve({
        pallet,
        passed,
        failed,
        ignored,
        duration,
        output,
        success: code === 0,
        errors,
      });
    });

    child.on('error', (err) => {
      resolve({
        pallet,
        passed: 0,
        failed: 0,
        ignored: 0,
        duration: Date.now() - start,
        output: err.message,
        success: false,
        errors: [err.message],
      });
    });
  });
}

/**
 * 批量运行多个 pallet 的测试
 */
export async function runCargoTests(
  pallets: readonly string[],
  projectRoot: string,
  opts?: { concurrency?: number; filter?: string; onResult?: (r: CargoTestResult) => void },
): Promise<CargoTestResult[]> {
  const concurrency = opts?.concurrency ?? 1; // 默认串行，Rust 编译占资源
  const results: CargoTestResult[] = [];
  const queue = [...pallets];

  async function worker() {
    while (queue.length > 0) {
      const pallet = queue.shift()!;
      const result = await runCargoTest(pallet, projectRoot, opts?.filter);
      results.push(result);
      opts?.onResult?.(result);
    }
  }

  const workers = Array.from({ length: Math.min(concurrency, queue.length) }, () => worker());
  await Promise.all(workers);

  return results;
}

/** 解析 cargo test 输出 */
function parseCargoOutput(output: string): {
  passed: number; failed: number; ignored: number; errors: string[];
} {
  let passed = 0, failed = 0, ignored = 0;
  const errors: string[] = [];

  // 匹配: test result: ok. 42 passed; 0 failed; 0 ignored; ...
  const resultMatch = output.match(
    /test result: (?:ok|FAILED)\.\s+(\d+) passed;\s+(\d+) failed;\s+(\d+) ignored/
  );
  if (resultMatch) {
    passed = parseInt(resultMatch[1], 10);
    failed = parseInt(resultMatch[2], 10);
    ignored = parseInt(resultMatch[3], 10);
  }

  // 收集失败的测试名
  const failedTests = output.matchAll(/---- (\S+) stdout ----/g);
  for (const m of failedTests) {
    errors.push(m[1]);
  }

  return { passed, failed, ignored, errors };
}

/** 打印 cargo test 汇总 */
export function printCargoSummary(results: CargoTestResult[]): void {
  const totalPassed = results.reduce((s, r) => s + r.passed, 0);
  const totalFailed = results.reduce((s, r) => s + r.failed, 0);
  const totalIgnored = results.reduce((s, r) => s + r.ignored, 0);
  const totalTime = results.reduce((s, r) => s + r.duration, 0);

  console.log('\n' + '─'.repeat(70));
  console.log('  Cargo Test 汇总');
  console.log('─'.repeat(70));

  for (const r of results) {
    const icon = r.success ? '✅' : '❌';
    const stats = `${r.passed}/${r.passed + r.failed}`;
    const dur = `${(r.duration / 1000).toFixed(1)}s`;
    console.log(`  ${icon} ${r.pallet.padEnd(38)} ${stats.padEnd(10)} ${dur}`);
    if (r.errors.length > 0) {
      for (const e of r.errors) {
        console.log(`     ↳ FAIL: ${e}`);
      }
    }
  }

  console.log('─'.repeat(70));
  console.log(`  合计: ${totalPassed} passed, ${totalFailed} failed, ${totalIgnored} ignored (${(totalTime / 1000).toFixed(1)}s)`);
  console.log('─'.repeat(70));
}
