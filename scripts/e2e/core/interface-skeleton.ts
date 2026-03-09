/**
 * Flow skeleton helpers: 用最小可运行断言先把接口面挂到测试框架中。
 */

import { assertTrue } from './assertions.js';
import { FlowContext } from './test-runner.js';

export async function assertTxMethodsExist(
  ctx: FlowContext,
  apiSection: string,
  methods: string[],
  stepName = `检查 ${apiSection} 接口存在性`,
): Promise<void> {
  await ctx.check(stepName, 'system', () => {
    const txRoot = ctx.api.tx as Record<string, Record<string, unknown> | undefined>;
    const palletTx = txRoot[apiSection];
    assertTrue(!!palletTx, `api.tx.${apiSection} 应存在`);

    for (const method of methods) {
      assertTrue(
        typeof palletTx?.[method] === 'function',
        `api.tx.${apiSection}.${method} 应存在`,
      );
    }
  });
}

export async function printSkeletonChecklist(
  ctx: FlowContext,
  stepName: string,
  owner: string,
  items: string[],
): Promise<void> {
  await ctx.check(stepName, owner, () => {
    for (const item of items) {
      console.log(`    - ${item}`);
    }
  });
}
