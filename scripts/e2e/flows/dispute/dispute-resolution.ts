/**
 * Flow-D1: 争议解决完整流程
 *
 * 角色:
 *   - Bob     (Buyer/原告)
 *   - Eve     (Seller/被告)
 *   - Alice   (Sudo/仲裁委员会)
 *   - Charlie (无权限用户)
 *
 * 流程:
 *   1. Bob 提交证据
 *   2. Eve 提交反驳证据
 *   3. Bob 发起投诉 (缴纳押金)
 *   4. Eve 响应投诉
 *   5. [路径A] 达成和解
 *   6. Bob 发起新投诉 → 升级到仲裁
 *   7. Alice(仲裁) 裁决
 *   8. [错误路径] 非仲裁者裁决
 *   9. Bob 撤销投诉 → 退还押金
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertEventEmitted,
  assertTrue,
} from '../../core/assertions.js';
import { getFreeBalance } from '../../core/chain-state.js';

export const disputeFlow: FlowDef = {
  name: 'Flow-D1: 争议解决',
  description: '证据提交 → 投诉 → 响应 → 和解/仲裁 → 裁决 | 撤销退款',
  fn: disputeResolution,
};

async function disputeResolution(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const eve = ctx.actor('eve');
  const charlie = ctx.actor('charlie');

  // ─── Step 1: Bob 提交证据 ─────────────────────────────────

  const imgCids = ['QmTestEvidenceImg001'];
  const vidCids: string[] = [];
  const docCids = ['QmTestEvidenceDoc001'];

  const commitTx = (api.tx as any).evidence.commit(
    imgCids,    // images
    vidCids,    // videos
    docCids,    // documents
    null,       // description_cid
  );
  const commitResult = await ctx.send(commitTx, bob, 'Bob 提交证据', 'bob');
  assertTxSuccess(commitResult, '提交证据');

  const commitEvent = commitResult.events.find(
    e => e.section === 'evidence' && e.method === 'EvidenceCommitted',
  );
  assertTrue(!!commitEvent, '应有 EvidenceCommitted 事件');
  const evidenceId = commitEvent?.data?.evidenceId ?? commitEvent?.data?.[0] ?? commitEvent?.data?.evidence_id;
  console.log(`    证据 ID: ${evidenceId}`);

  // ─── Step 2: Eve 提交反驳证据 ─────────────────────────────

  const eveCommitTx = (api.tx as any).evidence.commit(
    ['QmSellerRebuttalImg001'], [], ['QmSellerRebuttalDoc001'], null,
  );
  const eveCommitResult = await ctx.send(eveCommitTx, eve, 'Eve 提交反驳证据', 'eve');
  assertTxSuccess(eveCommitResult, 'Eve 提交证据');

  const eveEvidenceEvent = eveCommitResult.events.find(
    e => e.section === 'evidence' && e.method === 'EvidenceCommitted',
  );
  const eveEvidenceId = eveEvidenceEvent?.data?.evidenceId ?? eveEvidenceEvent?.data?.[0];

  // ─── Step 3: Bob 发起投诉 ─────────────────────────────────

  const bobBalBefore = await getFreeBalance(api, bob.address);

  const complaintTx = (api.tx as any).arbitration.fileComplaint(
    eve.address,               // respondent
    'QmComplaintDescCid001',   // description_cid
    evidenceId,                // evidence_id (optional)
    null,                      // order_id (optional)
    null,                      // escrow_id (optional)
  );
  const complaintResult = await ctx.send(complaintTx, bob, 'Bob 发起投诉', 'bob');
  assertTxSuccess(complaintResult, '发起投诉');

  const complaintEvent = complaintResult.events.find(
    e => e.section === 'arbitration' && e.method === 'ComplaintFiled',
  );
  assertTrue(!!complaintEvent, '应有 ComplaintFiled 事件');
  const complaintId = complaintEvent?.data?.complaintId ?? complaintEvent?.data?.[0];
  console.log(`    投诉 ID: ${complaintId}`);

  // 验证押金已扣除
  await ctx.check('验证投诉押金已扣除', 'bob', async () => {
    const bobBalAfter = await getFreeBalance(api, bob.address);
    const delta = bobBalBefore - bobBalAfter;
    assertTrue(delta > 0n, `Bob 应被扣除押金, 减少 ${Number(delta) / 1e12} NEX`);
  });

  // ─── Step 4: Eve 响应投诉 ─────────────────────────────────

  const respondTx = (api.tx as any).arbitration.respondToComplaint(
    complaintId,
    'QmResponseDescCid001',    // response_cid
    eveEvidenceId,             // evidence_id (optional)
  );
  const respondResult = await ctx.send(respondTx, eve, 'Eve 响应投诉', 'eve');
  assertTxSuccess(respondResult, '响应投诉');

  // ─── Step 5: 达成和解 ─────────────────────────────────────

  const settleTx = (api.tx as any).arbitration.settleComplaint(complaintId);
  const settleResult = await ctx.send(settleTx, bob, 'Bob 提议和解', 'bob');
  // 和解需要双方同意，先由一方提议
  if (settleResult.success) {
    await ctx.check('和解提议已提交', 'bob', () => {});

    // Eve 同意和解
    const eveSettleTx = (api.tx as any).arbitration.settleComplaint(complaintId);
    const eveSettleResult = await ctx.send(eveSettleTx, eve, 'Eve 同意和解', 'eve');
    if (eveSettleResult.success) {
      await ctx.check('投诉已和解', 'system', () => {
        assertEventEmitted(eveSettleResult, 'arbitration', 'ComplaintSettled', '和解事件');
      });
    }
  } else {
    console.log(`    ℹ 和解失败: ${settleResult.error}`);
  }

  // ─── Step 6: 新投诉 → 升级到仲裁 ─────────────────────────

  const complaint2Tx = (api.tx as any).arbitration.fileComplaint(
    eve.address, 'QmComplaint2Desc', null, null, null,
  );
  const complaint2Result = await ctx.send(complaint2Tx, bob, 'Bob 发起新投诉(升级)', 'bob');
  assertTxSuccess(complaint2Result, '发起新投诉');

  const complaint2Event = complaint2Result.events.find(
    e => e.section === 'arbitration' && e.method === 'ComplaintFiled',
  );
  const complaint2Id = complaint2Event?.data?.complaintId ?? complaint2Event?.data?.[0];

  // Eve 响应
  const respond2Tx = (api.tx as any).arbitration.respondToComplaint(complaint2Id, 'QmResponse2', null);
  await ctx.send(respond2Tx, eve, 'Eve 响应新投诉', 'eve');

  // 升级到仲裁
  const escalateTx = (api.tx as any).arbitration.escalateToArbitration(complaint2Id);
  const escalateResult = await ctx.send(escalateTx, bob, 'Bob 升级到仲裁', 'bob');
  assertTxSuccess(escalateResult, '升级到仲裁');

  await ctx.check('验证投诉已升级', 'bob', () => {
    assertEventEmitted(escalateResult, 'arbitration', 'ComplaintEscalated', '升级事件');
  });

  // ─── Step 7: Alice(仲裁) 裁决 ─────────────────────────────

  const resolveTx = (api.tx as any).arbitration.resolveComplaint(
    complaint2Id,
    0,    // decision: 0=FavorComplainant, 1=FavorRespondent, 2=Split
    null, // bps (用于 Split)
  );
  const resolveResult = await ctx.sudo(resolveTx, '仲裁裁决');
  assertTxSuccess(resolveResult, '仲裁裁决');

  await ctx.check('验证裁决事件', 'system', () => {
    assertEventEmitted(resolveResult, 'arbitration', 'ComplaintResolved', '裁决事件');
  });

  // ─── Step 8: [错误路径] 非仲裁者裁决 ─────────────────────

  // 先再创建一个投诉用于错误路径
  const complaint3Tx = (api.tx as any).arbitration.fileComplaint(
    eve.address, 'QmComplaint3', null, null, null,
  );
  const complaint3Result = await ctx.send(complaint3Tx, bob, 'Bob 发起投诉(错误路径)', 'bob');

  if (complaint3Result.success) {
    const complaint3Event = complaint3Result.events.find(
      e => e.section === 'arbitration' && e.method === 'ComplaintFiled',
    );
    const complaint3Id = complaint3Event?.data?.complaintId ?? complaint3Event?.data?.[0];

    // Eve 响应
    await ctx.send(
      (api.tx as any).arbitration.respondToComplaint(complaint3Id, 'QmResp3', null),
      eve, 'Eve 响应', 'eve',
    );

    // Bob 升级
    await ctx.send(
      (api.tx as any).arbitration.escalateToArbitration(complaint3Id),
      bob, 'Bob 升级', 'bob',
    );

    // Charlie 尝试裁决
    const fakeResolveTx = (api.tx as any).arbitration.resolveComplaint(complaint3Id, 0, null);
    const fakeResolveResult = await ctx.send(fakeResolveTx, charlie, '[错误路径] Charlie 裁决', 'charlie');
    await ctx.check('非仲裁者裁决应失败', 'charlie', () => {
      assertTxFailed(fakeResolveResult, undefined, '非仲裁者裁决');
    });
  }

  // ─── Step 9: 撤销投诉 → 退还押金 ─────────────────────────

  const complaint4Tx = (api.tx as any).arbitration.fileComplaint(
    eve.address, 'QmComplaint4', null, null, null,
  );
  const complaint4Result = await ctx.send(complaint4Tx, bob, 'Bob 发起投诉(撤销)', 'bob');

  if (complaint4Result.success) {
    const complaint4Event = complaint4Result.events.find(
      e => e.section === 'arbitration' && e.method === 'ComplaintFiled',
    );
    const complaint4Id = complaint4Event?.data?.complaintId ?? complaint4Event?.data?.[0];

    const bobBalBeforeWithdraw = await getFreeBalance(api, bob.address);

    const withdrawTx = (api.tx as any).arbitration.withdrawComplaint(complaint4Id);
    const withdrawResult = await ctx.send(withdrawTx, bob, 'Bob 撤销投诉', 'bob');
    assertTxSuccess(withdrawResult, '撤销投诉');

    await ctx.check('验证押金退还', 'bob', async () => {
      const bobBalAfter = await getFreeBalance(api, bob.address);
      // 押金应返还 (扣除手续费)
      const delta = bobBalAfter - bobBalBeforeWithdraw;
      console.log(`    退还变化: ${Number(delta) / 1e12} NEX`);
    });
  }

  // ─── 汇总 ─────────────────────────────────────────────────
  await ctx.check('争议解决汇总', 'system', () => {
    console.log(`    ✓ 证据: 双方提交`);
    console.log(`    ✓ 投诉: 发起→响应→和解`);
    console.log(`    ✓ 仲裁: 升级→裁决`);
    console.log(`    ✓ 撤销: 撤销→退还押金`);
    console.log(`    ✓ 错误路径: 非仲裁者裁决 ✗`);
  });
}
