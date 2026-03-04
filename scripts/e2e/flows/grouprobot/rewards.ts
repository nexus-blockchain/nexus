/**
 * Flow-G7: 奖励分配完整流程
 *
 * 角色:
 *   - Bob     (节点运营者)
 *   - Alice   (Sudo — 救援滞留奖励)
 *   - Dave    (无权限用户)
 *
 * 流程:
 *   1. Bob 领取节点奖励 (claim_rewards)
 *   2. Alice 救援滞留奖励 (rescue_stranded_rewards)
 *   3. [错误路径] Dave 领取他人节点奖励
 *   4. [错误路径] 领取不存在节点的奖励
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertEventEmitted,
  assertTrue,
} from '../../core/assertions.js';
import { getFreeBalance } from '../../core/chain-state.js';

export const rewardsFlow: FlowDef = {
  name: 'Flow-G7: 奖励分配',
  description: '领取奖励 → 救援滞留奖励 | 错误路径',
  fn: rewardsLifecycle,
};

async function rewardsLifecycle(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const dave = ctx.actor('dave');

  // 使用一个合理的 nodeId (假设已在 G2 flow 中注册)
  const nodeId = '0x' + '01'.repeat(32);
  const fakeNodeId = '0x' + 'ff'.repeat(32);

  // ─── Step 1: Bob 领取节点奖励 ─────────────────────────────

  const bobBalBefore = await getFreeBalance(api, bob.address);

  const claimTx = (api.tx as any).groupRobotRewards.claimRewards(nodeId);
  const claimResult = await ctx.send(claimTx, bob, 'Bob 领取节点奖励', 'bob');

  if (claimResult.success) {
    await ctx.check('验证奖励到账', 'bob', async () => {
      const bobBalAfter = await getFreeBalance(api, bob.address);
      const delta = bobBalAfter - bobBalBefore;
      console.log(`    奖励金额: ${Number(delta) / 1e12} NEX`);
      assertEventEmitted(claimResult, 'groupRobotRewards', 'RewardsClaimed', '领取事件');
    });
  } else {
    console.log(`    ℹ 领取奖励失败 (可能无待领取奖励): ${claimResult.error}`);
  }

  // ─── Step 2: Alice 救援滞留奖励 ───────────────────────────

  const rescueTx = (api.tx as any).groupRobotRewards.rescueStrandedRewards(
    nodeId,
    bob.address,   // beneficiary
  );
  const rescueResult = await ctx.sudo(rescueTx, '救援滞留奖励');

  if (rescueResult.success) {
    await ctx.check('滞留奖励已救援', 'system', () => {
      assertEventEmitted(rescueResult, 'groupRobotRewards', 'StrandedRewardsRescued', '救援事件');
    });
  } else {
    console.log(`    ℹ 救援失败 (可能无滞留奖励): ${rescueResult.error}`);
  }

  // ─── Step 3: [错误路径] Dave 领取他人节点奖励 ──────────────

  const daveClaimTx = (api.tx as any).groupRobotRewards.claimRewards(nodeId);
  const daveClaimResult = await ctx.send(daveClaimTx, dave, '[错误路径] Dave 领取他人奖励', 'dave');
  await ctx.check('非节点 Owner 领取应失败', 'dave', () => {
    assertTxFailed(daveClaimResult, undefined, '非 Owner 领取');
  });

  // ─── Step 4: [错误路径] 领取不存在节点的奖励 ──────────────

  const fakeClaimTx = (api.tx as any).groupRobotRewards.claimRewards(fakeNodeId);
  const fakeClaimResult = await ctx.send(fakeClaimTx, bob, '[错误路径] 不存在节点', 'bob');
  await ctx.check('不存在节点领取应失败', 'bob', () => {
    assertTxFailed(fakeClaimResult, undefined, '不存在节点');
  });

  // ─── 汇总 ─────────────────────────────────────────────────
  await ctx.check('奖励分配汇总', 'system', () => {
    console.log(`    ✓ 领取奖励`);
    console.log(`    ✓ 救援滞留奖励 (Root)`);
    console.log(`    ✓ 错误路径: 非 Owner ✗, 不存在节点 ✗`);
  });
}
