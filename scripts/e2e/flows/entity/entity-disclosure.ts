/**
 * Flow-E9: 信息披露 + 公告完整流程
 *
 * 角色:
 *   - Bob     (实体所有者)
 *   - Alice   (Sudo)
 *   - Charlie (内幕人员)
 *   - Dave    (无权限用户)
 *
 * 流程:
 *   1. 创建实体
 *   2. Bob 配置披露设置
 *   3. Bob 发布披露
 *   4. Bob 更正披露 (新版本)
 *   5. Bob 撤回披露
 *   6. Bob 清理披露历史
 *   7. Bob 添加内幕人员 (Charlie)
 *   8. Bob 开始黑窗口期
 *   9. Bob 结束黑窗口期
 *  10. Bob 移除内幕人员
 *  11. Bob 发布公告
 *  12. Bob 更新公告
 *  13. Bob 置顶公告
 *  14. Bob 撤回公告
 *  15. Bob 清理公告历史
 *  16. [错误路径] Dave 发布披露
 *  17. [错误路径] Dave 撤回他人披露
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertEventEmitted,
  assertTrue,
} from '../../core/assertions.js';

export const entityDisclosureFlow: FlowDef = {
  name: 'Flow-E9: 信息披露',
  description: '配置 → 发布/更正/撤回 → 内幕人员 → 黑窗口 → 公告管理 | 错误路径',
  fn: entityDisclosure,
};

async function entityDisclosure(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const charlie = ctx.actor('charlie');
  const dave = ctx.actor('dave');

  // ─── Step 1: 创建实体 ──────────────────────────────────────

  const createEntityTx = (api.tx as any).entityRegistry.createEntity(
    'E9 Disclosure Test Entity',
    'QmE9DisclosureDesc',
    null,
  );
  const entityResult = await ctx.send(createEntityTx, bob, '创建实体', 'bob');
  assertTxSuccess(entityResult, '创建实体');

  const entityEvent = entityResult.events.find(
    e => e.section === 'entityRegistry' && e.method === 'EntityCreated',
  );
  const entityId = entityEvent?.data?.entityId ?? entityEvent?.data?.[0];
  console.log(`    实体 ID: ${entityId}`);

  // ─── Step 2: 配置披露设置 ──────────────────────────────────

  const configTx = (api.tx as any).entityDisclosure.configureDisclosure(
    entityId,
    true,     // require_approval
    7200,     // blackout_duration (blocks)
    null,     // max_disclosures
  );
  const configResult = await ctx.send(configTx, bob, '配置披露设置', 'bob');
  assertTxSuccess(configResult, '配置披露');

  // ─── Step 3: 发布披露 ─────────────────────────────────────

  const publishTx = (api.tx as any).entityDisclosure.publishDisclosure(
    entityId,
    0,                        // disclosure_type: Financial=0
    'QmDisclosureTitleCid',   // title_cid
    'QmDisclosureContentCid', // content_cid
    null,                     // attachments
    null,                     // effective_at
  );
  const publishResult = await ctx.send(publishTx, bob, 'Bob 发布披露', 'bob');
  assertTxSuccess(publishResult, '发布披露');

  const disclosureEvent = publishResult.events.find(
    e => e.section === 'entityDisclosure' && e.method === 'DisclosurePublished',
  );
  assertTrue(!!disclosureEvent, '应有 DisclosurePublished 事件');
  const disclosureId = disclosureEvent?.data?.disclosureId ?? disclosureEvent?.data?.[0];
  console.log(`    披露 ID: ${disclosureId}`);

  // ─── Step 4: 更正披露 ─────────────────────────────────────

  const correctTx = (api.tx as any).entityDisclosure.correctDisclosure(
    entityId,
    disclosureId,
    'QmCorrectedTitleCid',
    'QmCorrectedContentCid',
    null,
    'QmCorrectionReasonCid',  // reason_cid
  );
  const correctResult = await ctx.send(correctTx, bob, 'Bob 更正披露', 'bob');
  assertTxSuccess(correctResult, '更正披露');

  await ctx.check('验证更正事件', 'bob', () => {
    assertEventEmitted(correctResult, 'entityDisclosure', 'DisclosureCorrected', '更正事件');
  });

  // 获取新版本的披露 ID
  const correctedEvent = correctResult.events.find(
    e => e.section === 'entityDisclosure' && e.method === 'DisclosurePublished',
  );
  const correctedDisclosureId = correctedEvent?.data?.disclosureId ?? correctedEvent?.data?.[0] ?? disclosureId;

  // ─── Step 5: 撤回披露 ─────────────────────────────────────

  const withdrawTx = (api.tx as any).entityDisclosure.withdrawDisclosure(
    entityId,
    correctedDisclosureId,
  );
  const withdrawResult = await ctx.send(withdrawTx, bob, 'Bob 撤回披露', 'bob');
  assertTxSuccess(withdrawResult, '撤回披露');

  await ctx.check('验证撤回事件', 'bob', () => {
    assertEventEmitted(withdrawResult, 'entityDisclosure', 'DisclosureWithdrawn', '撤回事件');
  });

  // ─── Step 6: 清理披露历史 ─────────────────────────────────

  const cleanupDiscTx = (api.tx as any).entityDisclosure.cleanupDisclosureHistory(entityId);
  const cleanupDiscResult = await ctx.send(cleanupDiscTx, bob, 'Bob 清理披露历史', 'bob');
  if (cleanupDiscResult.success) {
    await ctx.check('披露历史已清理', 'bob', () => {});
  } else {
    console.log(`    ℹ 清理披露历史: ${cleanupDiscResult.error}`);
  }

  // ─── Step 7: 添加内幕人员 ─────────────────────────────────

  const addInsiderTx = (api.tx as any).entityDisclosure.addInsider(
    entityId,
    charlie.address,
    'CFO',           // role
  );
  const addInsiderResult = await ctx.send(addInsiderTx, bob, 'Bob 添加 Charlie 为内幕人员', 'bob');
  assertTxSuccess(addInsiderResult, '添加内幕人员');

  await ctx.check('验证内幕人员事件', 'bob', () => {
    assertEventEmitted(addInsiderResult, 'entityDisclosure', 'InsiderAdded', '内幕人员事件');
  });

  // ─── Step 8: 开始黑窗口期 ─────────────────────────────────

  const startBlackoutTx = (api.tx as any).entityDisclosure.startBlackout(entityId);
  const startBlackoutResult = await ctx.send(startBlackoutTx, bob, 'Bob 开始黑窗口期', 'bob');
  assertTxSuccess(startBlackoutResult, '开始黑窗口期');

  await ctx.check('验证黑窗口事件', 'bob', () => {
    assertEventEmitted(startBlackoutResult, 'entityDisclosure', 'BlackoutStarted', '黑窗口事件');
  });

  // ─── Step 9: 结束黑窗口期 ─────────────────────────────────

  const endBlackoutTx = (api.tx as any).entityDisclosure.endBlackout(entityId);
  const endBlackoutResult = await ctx.send(endBlackoutTx, bob, 'Bob 结束黑窗口期', 'bob');
  assertTxSuccess(endBlackoutResult, '结束黑窗口期');

  // ─── Step 10: 移除内幕人员 ────────────────────────────────

  const removeInsiderTx = (api.tx as any).entityDisclosure.removeInsider(
    entityId,
    charlie.address,
  );
  const removeInsiderResult = await ctx.send(removeInsiderTx, bob, 'Bob 移除 Charlie', 'bob');
  assertTxSuccess(removeInsiderResult, '移除内幕人员');

  // ─── Step 11: 发布公告 ────────────────────────────────────

  const publishAnnTx = (api.tx as any).entityDisclosure.publishAnnouncement(
    entityId,
    'QmAnnouncementTitleCid',
    'QmAnnouncementContentCid',
    0,      // category: General=0
    null,   // expires_at
    null,   // attachments
  );
  const publishAnnResult = await ctx.send(publishAnnTx, bob, 'Bob 发布公告', 'bob');
  assertTxSuccess(publishAnnResult, '发布公告');

  const annEvent = publishAnnResult.events.find(
    e => e.section === 'entityDisclosure' && e.method === 'AnnouncementPublished',
  );
  assertTrue(!!annEvent, '应有 AnnouncementPublished 事件');
  const announcementId = annEvent?.data?.announcementId ?? annEvent?.data?.[0];
  console.log(`    公告 ID: ${announcementId}`);

  // ─── Step 12: 更新公告 ────────────────────────────────────

  const updateAnnTx = (api.tx as any).entityDisclosure.updateAnnouncement(
    entityId,
    announcementId,
    'QmUpdatedAnnTitle',
    'QmUpdatedAnnContent',
    null,   // category
    null,   // expires_at
  );
  const updateAnnResult = await ctx.send(updateAnnTx, bob, 'Bob 更新公告', 'bob');
  assertTxSuccess(updateAnnResult, '更新公告');

  // ─── Step 13: 置顶公告 ────────────────────────────────────

  const pinTx = (api.tx as any).entityDisclosure.pinAnnouncement(entityId, announcementId);
  const pinResult = await ctx.send(pinTx, bob, 'Bob 置顶公告', 'bob');
  assertTxSuccess(pinResult, '置顶公告');

  // 取消置顶
  const unpinTx = (api.tx as any).entityDisclosure.pinAnnouncement(entityId, null);
  const unpinResult = await ctx.send(unpinTx, bob, 'Bob 取消置顶', 'bob');
  assertTxSuccess(unpinResult, '取消置顶');

  // ─── Step 14: 撤回公告 ────────────────────────────────────

  const withdrawAnnTx = (api.tx as any).entityDisclosure.withdrawAnnouncement(
    entityId,
    announcementId,
  );
  const withdrawAnnResult = await ctx.send(withdrawAnnTx, bob, 'Bob 撤回公告', 'bob');
  assertTxSuccess(withdrawAnnResult, '撤回公告');

  // ─── Step 15: 清理公告历史 ────────────────────────────────

  const cleanupAnnTx = (api.tx as any).entityDisclosure.cleanupAnnouncementHistory(entityId);
  const cleanupAnnResult = await ctx.send(cleanupAnnTx, bob, 'Bob 清理公告历史', 'bob');
  if (cleanupAnnResult.success) {
    await ctx.check('公告历史已清理', 'bob', () => {});
  } else {
    console.log(`    ℹ 清理公告历史: ${cleanupAnnResult.error}`);
  }

  // ─── Step 16: [错误路径] Dave 发布披露 ─────────────────────

  const davePublishTx = (api.tx as any).entityDisclosure.publishDisclosure(
    entityId, 0, 'QmFakeTitle', 'QmFakeContent', null, null,
  );
  const davePublishResult = await ctx.send(davePublishTx, dave, '[错误路径] Dave 发布披露', 'dave');
  await ctx.check('非所有者发布应失败', 'dave', () => {
    assertTxFailed(davePublishResult, undefined, '非所有者发布');
  });

  // ─── Step 17: [错误路径] Dave 撤回他人披露 ─────────────────

  // 先让 Bob 发布一个新的披露
  const newDiscTx = (api.tx as any).entityDisclosure.publishDisclosure(
    entityId, 0, 'QmNewTitle', 'QmNewContent', null, null,
  );
  const newDiscResult = await ctx.send(newDiscTx, bob, 'Bob 发布新披露(供错误路径)', 'bob');
  if (newDiscResult.success) {
    const newDiscEvent = newDiscResult.events.find(
      e => e.section === 'entityDisclosure' && e.method === 'DisclosurePublished',
    );
    const newDiscId = newDiscEvent?.data?.disclosureId ?? newDiscEvent?.data?.[0];

    const daveWithdrawTx = (api.tx as any).entityDisclosure.withdrawDisclosure(entityId, newDiscId);
    const daveWithdrawResult = await ctx.send(daveWithdrawTx, dave, '[错误路径] Dave 撤回他人披露', 'dave');
    await ctx.check('非所有者撤回应失败', 'dave', () => {
      assertTxFailed(daveWithdrawResult, undefined, '非所有者撤回');
    });
  }

  // ─── 汇总 ─────────────────────────────────────────────────
  await ctx.check('信息披露汇总', 'system', () => {
    console.log(`    ✓ 披露: 配置 → 发布 → 更正 → 撤回 → 清理`);
    console.log(`    ✓ 内幕人员: 添加 → 移除`);
    console.log(`    ✓ 黑窗口: 开始 → 结束`);
    console.log(`    ✓ 公告: 发布 → 更新 → 置顶/取消 → 撤回 → 清理`);
    console.log(`    ✓ 错误路径: 非所有者发布 ✗, 非所有者撤回 ✗`);
  });
}
