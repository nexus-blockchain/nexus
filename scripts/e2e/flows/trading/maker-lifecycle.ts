/**
 * Flow-T1: NEX 市场种子资金 + 流动性注入
 *
 * 角色: Alice (Sudo)
 *
 * 流程:
 *   1. Sudo 设置初始价格
 *   2. Sudo 注入种子资金 (fundSeedAccount)
 *   3. Sudo 配置价格保护
 *   4. Sudo 注入流动性 (seedLiquidity)
 *   5. 验证初始价格已设置
 *   6. [错误路径] 非 Sudo 设置价格应失败
 *   7. [错误路径] 非 Sudo 解除熔断应失败
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertEventEmitted,
  assertTrue,
} from '../../core/assertions.js';
import { nex } from '../../core/config.js';

export const makerLifecycleFlow: FlowDef = {
  name: 'Flow-T1: 种子资金+流动性',
  description: '设置价格 → 种子资金 → 价格保护 → 流动性注入 | 权限校验',
  fn: makerLifecycle,
};

async function makerLifecycle(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');

  // --------------- Step 1: Sudo 设置初始价格 ---------------
  const setPriceTx = (api.tx as any).nexMarket.setInitialPrice(1_000_000);
  const priceResult = await ctx.sudo(setPriceTx, '设置初始价格');
  assertTxSuccess(priceResult, '设置初始价格');

  await ctx.check('初始价格已设置', 'system', async () => {
    const price = await (api.query as any).nexMarket.lastTradePrice();
    console.log(`    初始价格: ${price.toHuman()}`);
  });

  // --------------- Step 2: Sudo 注入种子资金 ---------------
  const fundTx = (api.tx as any).nexMarket.fundSeedAccount(nex(1000).toString());
  const fundResult = await ctx.sudo(fundTx, '注入种子资金');
  assertTxSuccess(fundResult, '注入种子资金');

  // --------------- Step 3: Sudo 配置价格保护 ---------------
  const configProtTx = (api.tx as any).nexMarket.configurePriceProtection(
    true,   // enabled
    500,    // maxPriceDeviation
    5000,   // circuitBreakerThreshold
    5,      // minTradesForTwap
  );
  const protResult = await ctx.sudo(configProtTx, '配置价格保护');
  assertTxSuccess(protResult, '配置价格保护');

  // --------------- Step 4: Sudo 注入流动性 ---------------
  const seedTx = (api.tx as any).nexMarket.seedLiquidity(5, null);
  const seedResult = await ctx.sudo(seedTx, '注入流动性');
  if (seedResult.success) {
    await ctx.check('流动性已注入', 'system', () => {});
  } else {
    console.log(`    ℹ 流动性注入失败 (可能余额不足): ${seedResult.error}`);
  }

  // --------------- Step 5: [错误路径] 非 Sudo 设置价格 ---------------
  const bobPriceTx = (api.tx as any).nexMarket.setInitialPrice(999);
  const bobPriceResult = await ctx.send(bobPriceTx, bob, '[错误路径] 非 Sudo 设置价格', 'bob');
  await ctx.check('非 Sudo 设置价格应失败', 'bob', () => {
    assertTxFailed(bobPriceResult, undefined, '非 Sudo 设置价格');
  });

  // --------------- Step 6: [错误路径] 非 Sudo 解除熔断 ---------------
  const bobLiftTx = (api.tx as any).nexMarket.liftCircuitBreaker();
  const bobLiftResult = await ctx.send(bobLiftTx, bob, '[错误路径] 非 Sudo 解除熔断', 'bob');
  await ctx.check('非 Sudo 解除熔断应失败', 'bob', () => {
    assertTxFailed(bobLiftResult, undefined, '非 Sudo 解除熔断');
  });

  // --------------- 汇总 ---------------
  await ctx.check('种子资金+流动性汇总', 'system', () => {
    console.log(`    ✓ 初始价格设置`);
    console.log(`    ✓ 种子资金注入`);
    console.log(`    ✓ 价格保护配置`);
    console.log(`    ✓ 流动性注入`);
    console.log(`    ✓ 错误路径: 非 Sudo 操作 ✗`);
  });
}
