/**
 * E2E 测试运行器 — 管理 flow 执行、步骤记录、错误捕获
 */

import { ApiPromise } from '@polkadot/api';
import { KeyringPair } from '@polkadot/keyring/types';
import { TestReporter, FlowResult } from './reporter.js';
import { signAndSend, sudoSend, TxResult } from './chain-state.js';
import { SubmittableExtrinsic } from '@polkadot/api/types';

export interface FlowContext {
  api: ApiPromise;
  reporter: TestReporter;
  /** 按名称获取角色账户 */
  actor: (name: string) => KeyringPair;
  /** 发送交易 (自动记录步骤) */
  send: (
    tx: SubmittableExtrinsic<'promise'>,
    signer: KeyringPair,
    stepName: string,
    actorName?: string,
  ) => Promise<TxResult>;
  /** 通过 Sudo 发送交易 */
  sudo: (
    tx: SubmittableExtrinsic<'promise'>,
    stepName: string,
  ) => Promise<TxResult>;
  /** 记录一个断言步骤 */
  check: (stepName: string, actorName: string, fn: () => Promise<void> | void) => Promise<void>;
}

export type FlowFn = (ctx: FlowContext) => Promise<void>;

export interface FlowDef {
  name: string;
  description: string;
  fn: FlowFn;
}

/**
 * 运行一组 E2E 测试流程
 */
export async function runFlows(
  api: ApiPromise,
  actors: Record<string, KeyringPair>,
  flows: FlowDef[],
): Promise<{ reporter: TestReporter; allPassed: boolean }> {
  const reporter = new TestReporter();

  for (const flow of flows) {
    const result = await runSingleFlow(api, actors, flow, reporter);
    reporter.printFlowResult(result);
  }

  reporter.printSummary();
  return { reporter, allPassed: reporter.allPassed };
}

async function runSingleFlow(
  api: ApiPromise,
  actors: Record<string, KeyringPair>,
  flow: FlowDef,
  reporter: TestReporter,
): Promise<FlowResult> {
  reporter.startFlow(flow.name, flow.description);

  const sudoAccount = actors['alice'] ?? actors['Alice'];

  const ctx: FlowContext = {
    api,
    reporter,

    actor: (name: string) => {
      const account = actors[name] ?? actors[name.toLowerCase()];
      if (!account) throw new Error(`Actor "${name}" not found in actors map`);
      return account;
    },

    send: async (tx, signer, stepName, actorName) => {
      const start = Date.now();
      const result = await signAndSend(api, tx, signer, stepName);
      const duration = Date.now() - start;
      // 错误路径 (expected-fail) steps: don't record here — let ctx.check verify
      const isErrorPath = stepName.includes('[错误路径]') || stepName.includes('[error-path]');
      if (!isErrorPath) {
        reporter.recordStep(
          stepName,
          actorName ?? 'unknown',
          result.success,
          duration,
          result.error,
        );
      }
      return result;
    },

    sudo: async (tx, stepName) => {
      if (!sudoAccount) throw new Error('No sudo account (alice) in actors');
      const start = Date.now();
      const result = await sudoSend(api, tx, sudoAccount, stepName);
      const duration = Date.now() - start;
      reporter.recordStep(stepName, 'sudo(alice)', result.success, duration, result.error);
      return result;
    },

    check: async (stepName, actorName, fn) => {
      const start = Date.now();
      try {
        await fn();
        const duration = Date.now() - start;
        reporter.recordStep(stepName, actorName, true, duration);
      } catch (err: any) {
        const duration = Date.now() - start;
        reporter.recordStep(stepName, actorName, false, duration, err.message);
        throw err;
      }
    },
  };

  try {
    await flow.fn(ctx);
  } catch (err: any) {
    return reporter.endFlow(err.message);
  }

  return reporter.endFlow();
}
