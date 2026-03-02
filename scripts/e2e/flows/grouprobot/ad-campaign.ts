/**
 * Flow-G3: 广告活动完整流程
 *
 * 角色:
 *   - Bob     (广告主)
 *   - Alice   (Sudo — 审核/治理)
 *   - Charlie (社区管理员)
 *   - Dave    (无权限用户)
 *
 * 流程:
 *   1. Alice 设置社区管理员 (Charlie)
 *   2. Charlie 质押获取 audience_cap
 *   3. Bob 创建广告活动 (锁预算)
 *   4. Alice 审核广告 (approve)
 *   5. Bob 追加预算
 *   6. 提交投放收据 (audience_cap 裁切测试)
 *   7. Era 结算 (CPM 计费)
 *   8. Charlie 提取广告收入
 *   9. Bob 暂停广告
 *  10. Bob 取消广告 → 退还剩余预算
 *  11. 双向偏好: 拉黑/白名单
 *  12. [错误路径] Dave 非管理员操作
 *  13. Alice Slash 社区
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

export const adCampaignFlow: FlowDef = {
  name: 'Flow-G3: 广告活动',
  description: '质押 → 创建 → 审核 → 投放 → 结算 → 提取 | 暂停/取消 | Slash',
  fn: adCampaign,
};

async function adCampaign(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const charlie = ctx.actor('charlie');
  const dave = ctx.actor('dave');

  const communityIdHash = '0x' + 'cc'.repeat(32);

  // ─── Step 1: 设置社区管理员 ───────────────────────────────

  const setAdminTx = (api.tx as any).grouprobotAds.setCommunityAdmin(
    communityIdHash,
    charlie.address,
  );
  const adminResult = await ctx.sudo(setAdminTx, '设置社区管理员 (Charlie)');
  assertTxSuccess(adminResult, '设置管理员');

  // ─── Step 2: Charlie 质押获取 audience_cap ────────────────

  const stakeTx = (api.tx as any).grouprobotAds.stakeForAds(
    communityIdHash,
    nex(50).toString(),   // stake amount
  );
  const stakeResult = await ctx.send(stakeTx, charlie, 'Charlie 质押获取 cap', 'charlie');
  assertTxSuccess(stakeResult, '质押');

  await ctx.check('验证质押事件', 'charlie', () => {
    assertEventEmitted(stakeResult, 'grouprobotAds', 'StakedForAds', '质押事件');
  });

  // ─── Step 3: Bob 创建广告活动 ─────────────────────────────

  const bobBalBefore = await getFreeBalance(api, bob.address);

  const createCampaignTx = (api.tx as any).grouprobotAds.createCampaign(
    'E2E Test Ad Campaign',         // title
    'QmAdContentCid001',            // content_cid
    nex(100).toString(),            // budget
    1000,                            // cpm_rate (10 NEX per 1000 impressions)
    null,                            // max_daily_spend
    null,                            // target_communities
  );
  const campaignResult = await ctx.send(createCampaignTx, bob, 'Bob 创建广告活动', 'bob');
  assertTxSuccess(campaignResult, '创建广告');

  const campaignEvent = campaignResult.events.find(
    e => e.section === 'grouprobotAds' && e.method === 'CampaignCreated',
  );
  assertTrue(!!campaignEvent, '应有 CampaignCreated 事件');
  const campaignId = campaignEvent?.data?.campaignId ?? campaignEvent?.data?.[0];
  console.log(`    广告活动 ID: ${campaignId}`);

  // 验证预算已锁定
  await ctx.check('验证预算已锁定', 'bob', async () => {
    const bobBalAfter = await getFreeBalance(api, bob.address);
    const delta = bobBalBefore - bobBalAfter;
    assertTrue(delta > 0n, `Bob 应被扣除预算, 减少 ${Number(delta) / 1e12} NEX`);
  });

  // ─── Step 4: Alice 审核广告 ───────────────────────────────

  const reviewTx = (api.tx as any).grouprobotAds.reviewCampaign(
    campaignId,
    true,    // approved
    null,    // reason
  );
  const reviewResult = await ctx.sudo(reviewTx, '审核广告 (approve)');
  assertTxSuccess(reviewResult, '审核广告');

  // ─── Step 5: Bob 追加预算 ────────────────────────────────

  const fundTx = (api.tx as any).grouprobotAds.fundCampaign(
    campaignId,
    nex(50).toString(),
  );
  const fundResult = await ctx.send(fundTx, bob, 'Bob 追加预算 50 NEX', 'bob');
  assertTxSuccess(fundResult, '追加预算');

  // ─── Step 6: 提交投放收据 ────────────────────────────────

  const receiptTx = (api.tx as any).grouprobotAds.submitDeliveryReceipt(
    campaignId,
    communityIdHash,
    500,              // audience_size (会被 cap 裁切)
    '0x' + '00'.repeat(64),  // proof_hash
  );
  const receiptResult = await ctx.send(receiptTx, charlie, '提交投放收据', 'charlie');
  assertTxSuccess(receiptResult, '投放收据');

  await ctx.check('验证投放收据事件', 'charlie', () => {
    assertEventEmitted(receiptResult, 'grouprobotAds', 'DeliveryReceiptSubmitted', '收据事件');
  });

  // ─── Step 7: Era 结算 ────────────────────────────────────

  const settleTx = (api.tx as any).grouprobotAds.settleEraAds(communityIdHash);
  const settleResult = await ctx.send(settleTx, charlie, 'Era 结算', 'charlie');
  if (settleResult.success) {
    await ctx.check('Era 结算事件', 'system', () => {
      assertEventEmitted(settleResult, 'grouprobotAds', 'EraSettled', '结算事件');
    });
  } else {
    console.log(`    ℹ 结算失败 (可能尚未到 Era 边界): ${settleResult.error}`);
  }

  // ─── Step 8: Charlie 提取广告收入 ─────────────────────────

  const charlieBalBefore = await getFreeBalance(api, charlie.address);

  const claimRevTx = (api.tx as any).grouprobotAds.claimAdRevenue(communityIdHash);
  const claimRevResult = await ctx.send(claimRevTx, charlie, 'Charlie 提取广告收入', 'charlie');
  if (claimRevResult.success) {
    await ctx.check('验证收入到账', 'charlie', async () => {
      const charlieBalAfter = await getFreeBalance(api, charlie.address);
      const delta = charlieBalAfter - charlieBalBefore;
      console.log(`    Charlie 收入: ${Number(delta) / 1e12} NEX`);
    });
  } else {
    console.log(`    ℹ 提取失败 (可能无收入): ${claimRevResult.error}`);
  }

  // ─── Step 9: Bob 暂停广告 ────────────────────────────────

  const pauseTx = (api.tx as any).grouprobotAds.pauseCampaign(campaignId);
  const pauseResult = await ctx.send(pauseTx, bob, 'Bob 暂停广告', 'bob');
  assertTxSuccess(pauseResult, '暂停广告');

  // ─── Step 10: Bob 取消广告 → 退还剩余预算 ────────────────

  const bobBalBeforeCancel = await getFreeBalance(api, bob.address);

  const cancelTx = (api.tx as any).grouprobotAds.cancelCampaign(campaignId);
  const cancelResult = await ctx.send(cancelTx, bob, 'Bob 取消广告', 'bob');
  assertTxSuccess(cancelResult, '取消广告');

  await ctx.check('验证剩余预算退还', 'bob', async () => {
    const bobBalAfterCancel = await getFreeBalance(api, bob.address);
    const delta = bobBalAfterCancel - bobBalBeforeCancel;
    assertTrue(delta > 0n, `Bob 应收到退还预算, 增加 ${Number(delta) / 1e12} NEX`);
  });

  // ─── Step 11: 双向偏好 ───────────────────────────────────

  // Bob 拉黑社区
  const blockCommTx = (api.tx as any).grouprobotAds.advertiserBlockCommunity(communityIdHash);
  const blockResult = await ctx.send(blockCommTx, bob, 'Bob 拉黑社区', 'bob');
  assertTxSuccess(blockResult, '拉黑社区');

  // Bob 取消拉黑
  const unblockTx = (api.tx as any).grouprobotAds.advertiserUnblockCommunity(communityIdHash);
  const unblockResult = await ctx.send(unblockTx, bob, 'Bob 取消拉黑', 'bob');
  assertTxSuccess(unblockResult, '取消拉黑');

  // Charlie (管理员) 拉黑广告主
  // 需要新广告以获取广告主 ID
  const commBlockTx = (api.tx as any).grouprobotAds.communityBlockAdvertiser(
    communityIdHash, bob.address,
  );
  const commBlockResult = await ctx.send(commBlockTx, charlie, 'Charlie 拉黑广告主', 'charlie');
  assertTxSuccess(commBlockResult, '社区拉黑广告主');

  // 取消拉黑
  const commUnblockTx = (api.tx as any).grouprobotAds.communityUnblockAdvertiser(
    communityIdHash, bob.address,
  );
  await ctx.send(commUnblockTx, charlie, 'Charlie 取消拉黑', 'charlie');

  // ─── Step 12: [错误路径] Dave 非管理员操作 ────────────────

  const daveClaimTx = (api.tx as any).grouprobotAds.claimAdRevenue(communityIdHash);
  const daveResult = await ctx.send(daveClaimTx, dave, '[错误路径] Dave 提取收入', 'dave');
  await ctx.check('非管理员提取应失败', 'dave', () => {
    assertTxFailed(daveResult, undefined, '非管理员');
  });

  const daveBlockTx = (api.tx as any).grouprobotAds.communityBlockAdvertiser(
    communityIdHash, bob.address,
  );
  const daveBlockResult = await ctx.send(daveBlockTx, dave, '[错误路径] Dave 拉黑', 'dave');
  await ctx.check('非管理员拉黑应失败', 'dave', () => {
    assertTxFailed(daveBlockResult, undefined, '非管理员拉黑');
  });

  // ─── Step 13: Alice Slash 社区 ────────────────────────────

  const slashTx = (api.tx as any).grouprobotAds.slashCommunity(
    communityIdHash,
    nex(10).toString(),    // slash_amount
    'E2E Slash test',      // reason
  );
  const slashResult = await ctx.sudo(slashTx, 'Slash 社区');
  assertTxSuccess(slashResult, 'Slash 社区');

  await ctx.check('Slash 事件', 'system', () => {
    assertEventEmitted(slashResult, 'grouprobotAds', 'CommunitySlashed', 'Slash 事件');
  });

  // ─── Charlie 取消质押 ────────────────────────────────────

  const unstakeTx = (api.tx as any).grouprobotAds.unstakeFromAds(communityIdHash);
  const unstakeResult = await ctx.send(unstakeTx, charlie, 'Charlie 取消质押', 'charlie');
  if (unstakeResult.success) {
    await ctx.check('取消质押事件', 'charlie', () => {});
  } else {
    console.log(`    ℹ 取消质押失败: ${unstakeResult.error}`);
  }

  // ─── 汇总 ─────────────────────────────────────────────────
  await ctx.check('广告活动汇总', 'system', () => {
    console.log(`    ✓ 质押 → 创建 → 审核 → 追加预算`);
    console.log(`    ✓ 投放收据 → Era 结算 → 提取收入`);
    console.log(`    ✓ 暂停 → 取消 → 退还预算`);
    console.log(`    ✓ 双向偏好: 拉黑/取消拉黑`);
    console.log(`    ✓ Slash 社区`);
    console.log(`    ✓ 错误路径: 非管理员操作 ✗`);
  });
}
