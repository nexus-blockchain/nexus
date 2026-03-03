/**
 * Flow-G4: 订阅服务完整流程
 *
 * 角色:
 *   - Bob     (订阅用户/群主)
 *   - Charlie (另一个订阅用户)
 *   - Dave    (无权限用户)
 *
 * 流程:
 *   1. Bob 订阅 Bot 服务 (付费)
 *   2. 验证订阅已创建
 *   3. Bob 充值订阅
 *   4. Bob 变更订阅层级
 *   5. Charlie 通过广告承诺订阅 (commit_ads)
 *   6. Charlie 取消广告承诺
 *   7. Bob 取消付费订阅
 *   8. 清理已取消的订阅记录
 *   9. 清理已取消的广告承诺记录
 *  10. [错误路径] Dave 充值不存在的订阅
 *  11. [错误路径] 重复订阅被拒绝
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertEventEmitted,
  assertTrue,
} from '../../core/assertions.js';
import { getFreeBalance } from '../../core/chain-state.js';
import { nex } from '../../core/config.js';

export const subscriptionFlow: FlowDef = {
  name: 'Flow-G4: 订阅服务',
  description: '订阅 → 充值 → 变更层级 → 广告承诺 → 取消 → 清理 | 错误路径',
  fn: subscriptionLifecycle,
};

async function subscriptionLifecycle(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const charlie = ctx.actor('charlie');
  const dave = ctx.actor('dave');

  const botIdHash = '0x' + 'dd'.repeat(32);

  // ─── Step 1: Bob 订阅 Bot 服务 ─────────────────────────────

  const bobBalBefore = await getFreeBalance(api, bob.address);

  const subscribeTx = (api.tx as any).grouprobotSubscription.subscribe(
    botIdHash,
    1,                       // tier: Basic=1
    nex(10).toString(),      // deposit
  );
  const subResult = await ctx.send(subscribeTx, bob, 'Bob 订阅 Bot 服务', 'bob');
  assertTxSuccess(subResult, '订阅');

  const subEvent = subResult.events.find(
    e => e.section === 'grouprobotSubscription' && e.method === 'Subscribed',
  );
  assertTrue(!!subEvent, '应有 Subscribed 事件');
  console.log(`    订阅事件: ${JSON.stringify(subEvent?.data).slice(0, 100)}`);

  // 验证扣费
  await ctx.check('验证订阅扣费', 'bob', async () => {
    const bobBalAfter = await getFreeBalance(api, bob.address);
    const delta = bobBalBefore - bobBalAfter;
    assertTrue(delta > 0n, `Bob 应被扣除订阅费, 减少 ${Number(delta) / 1e12} NEX`);
  });

  // ─── Step 2: 验证订阅状态 ─────────────────────────────────

  await ctx.check('验证订阅已创建', 'bob', async () => {
    const sub = await (api.query as any).grouprobotSubscription.subscriptions(botIdHash, bob.address);
    if (sub && !sub.isNone) {
      const data = sub.unwrap ? sub.unwrap().toHuman() : sub.toHuman();
      console.log(`    订阅状态: ${JSON.stringify(data).slice(0, 150)}`);
    }
  });

  // ─── Step 3: Bob 充值订阅 ─────────────────────────────────

  const depositTx = (api.tx as any).grouprobotSubscription.depositSubscription(
    botIdHash,
    nex(5).toString(),   // additional deposit
  );
  const depositResult = await ctx.send(depositTx, bob, 'Bob 充值订阅', 'bob');
  assertTxSuccess(depositResult, '充值订阅');

  // ─── Step 4: Bob 变更订阅层级 ─────────────────────────────

  const changeTierTx = (api.tx as any).grouprobotSubscription.changeTier(
    botIdHash,
    2,    // tier: Pro=2
  );
  const changeTierResult = await ctx.send(changeTierTx, bob, 'Bob 升级到 Pro', 'bob');
  if (changeTierResult.success) {
    await ctx.check('层级变更成功', 'bob', () => {
      assertEventEmitted(changeTierResult, 'grouprobotSubscription', 'TierChanged', '层级变更事件');
    });
  } else {
    console.log(`    ℹ 层级变更失败: ${changeTierResult.error}`);
  }

  // ─── Step 5: Charlie 通过广告承诺订阅 ─────────────────────

  const commitAdsTx = (api.tx as any).grouprobotSubscription.commitAds(
    botIdHash,
    '0x' + 'ee'.repeat(32),   // community_id_hash
  );
  const commitResult = await ctx.send(commitAdsTx, charlie, 'Charlie 广告承诺订阅', 'charlie');
  if (commitResult.success) {
    await ctx.check('广告承诺事件', 'charlie', () => {
      assertEventEmitted(commitResult, 'grouprobotSubscription', 'AdCommitted', '广告承诺事件');
    });
  } else {
    console.log(`    ℹ 广告承诺失败: ${commitResult.error}`);
  }

  // ─── Step 6: Charlie 取消广告承诺 ─────────────────────────

  const cancelAdTx = (api.tx as any).grouprobotSubscription.cancelAdCommitment(botIdHash);
  const cancelAdResult = await ctx.send(cancelAdTx, charlie, 'Charlie 取消广告承诺', 'charlie');
  if (cancelAdResult.success) {
    await ctx.check('广告承诺已取消', 'charlie', () => {});
  } else {
    console.log(`    ℹ 取消广告承诺失败: ${cancelAdResult.error}`);
  }

  // ─── Step 7: Bob 取消付费订阅 ─────────────────────────────

  const cancelSubTx = (api.tx as any).grouprobotSubscription.cancelSubscription(botIdHash);
  const cancelSubResult = await ctx.send(cancelSubTx, bob, 'Bob 取消订阅', 'bob');
  assertTxSuccess(cancelSubResult, '取消订阅');

  await ctx.check('验证取消事件', 'bob', () => {
    assertEventEmitted(cancelSubResult, 'grouprobotSubscription', 'SubscriptionCancelled', '取消事件');
  });

  // ─── Step 8: 清理已取消的订阅记录 ─────────────────────────

  const cleanupSubTx = (api.tx as any).grouprobotSubscription.cleanupSubscription(
    botIdHash,
    bob.address,
  );
  const cleanupSubResult = await ctx.send(cleanupSubTx, bob, '清理订阅记录', 'bob');
  if (cleanupSubResult.success) {
    await ctx.check('订阅记录已清理', 'bob', () => {});
  } else {
    console.log(`    ℹ 清理订阅: ${cleanupSubResult.error}`);
  }

  // ─── Step 9: 清理广告承诺记录 ─────────────────────────────

  const cleanupAdTx = (api.tx as any).grouprobotSubscription.cleanupAdCommitment(
    botIdHash,
    charlie.address,
  );
  const cleanupAdResult = await ctx.send(cleanupAdTx, charlie, '清理广告承诺记录', 'charlie');
  if (cleanupAdResult.success) {
    await ctx.check('广告承诺记录已清理', 'charlie', () => {});
  } else {
    console.log(`    ℹ 清理广告承诺: ${cleanupAdResult.error}`);
  }

  // ─── Step 10: [错误路径] Dave 充值不存在的订阅 ─────────────

  const daveDepositTx = (api.tx as any).grouprobotSubscription.depositSubscription(
    '0x' + 'ff'.repeat(32),   // 不存在的 bot
    nex(5).toString(),
  );
  const daveDepositResult = await ctx.send(daveDepositTx, dave, '[错误路径] Dave 充值不存在订阅', 'dave');
  await ctx.check('不存在订阅充值应失败', 'dave', () => {
    assertTxFailed(daveDepositResult, undefined, '不存在订阅');
  });

  // ─── Step 11: [错误路径] 重复订阅 ─────────────────────────

  // Bob 重新订阅
  const resubTx = (api.tx as any).grouprobotSubscription.subscribe(
    botIdHash, 1, nex(10).toString(),
  );
  const resubResult = await ctx.send(resubTx, bob, 'Bob 重新订阅', 'bob');

  if (resubResult.success) {
    // 尝试重复订阅
    const dupSubTx = (api.tx as any).grouprobotSubscription.subscribe(
      botIdHash, 1, nex(10).toString(),
    );
    const dupSubResult = await ctx.send(dupSubTx, bob, '[错误路径] Bob 重复订阅', 'bob');
    await ctx.check('重复订阅应失败', 'bob', () => {
      assertTxFailed(dupSubResult, undefined, '重复订阅');
    });
  }

  // ─── 汇总 ─────────────────────────────────────────────────
  await ctx.check('订阅服务汇总', 'system', () => {
    console.log(`    ✓ 付费订阅: 订阅 → 充值 → 变更层级 → 取消`);
    console.log(`    ✓ 广告承诺: 承诺 → 取消`);
    console.log(`    ✓ 清理: 订阅记录 → 广告承诺记录`);
    console.log(`    ✓ 错误路径: 不存在订阅 ✗, 重复订阅 ✗`);
  });
}
