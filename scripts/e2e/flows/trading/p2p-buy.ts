/**
 * Flow-T2: P2P Buy 完整流程 (用户买 NEX)
 *
 * 角色: Bob (做市商), Charlie (买家), Alice (Sudo)
 *
 * 前置条件: Bob 已是 Active 做市商 (依赖 Flow-T1)
 *
 * 流程:
 *   1. 查询 Charlie 余额
 *   2. Charlie 创建 Buy 订单 (购买 100 NEX)
 *   3. 验证订单状态 + 托管锁定
 *   4. Charlie 标记已付款
 *   5. Bob(做市商) 释放 NEX
 *   6. 验证 Charlie 收到 NEX
 *   7. [错误路径] 已完成订单不能重复释放
 *   8. [分支] 创建另一个订单 → 买家取消
 *   9. [分支] 创建另一个订单 → 发起争议
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertStorageField,
  assertEventEmitted,
  assertTrue,
  assertEqual,
} from '../../core/assertions.js';
import { getFreeBalance } from '../../core/chain-state.js';
import { nex } from '../../core/config.js';
import { blake2AsHex } from '@polkadot/util-crypto';

export const p2pBuyFlow: FlowDef = {
  name: 'Flow-T2: P2P Buy 流程',
  description: '创建 Buy 订单 → 付款 → 释放 NEX + 取消 + 争议分支',
  fn: p2pBuy,
};

async function p2pBuy(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const charlie = ctx.actor('charlie');

  // 确认 Bob 是活跃做市商
  let makerId: number;
  await ctx.check('确认做市商状态', 'bob', async () => {
    const bobMakerId = await (api.query as any).tradingMaker.accountToMaker(bob.address);
    assertTrue(bobMakerId.isSome, 'Bob 应是做市商');
    makerId = bobMakerId.unwrap().toNumber();
    const app = await (api.query as any).tradingMaker.makerApplications(makerId);
    assertTrue(app.isSome, '做市商记录应存在');
    const status = app.unwrap().status.toString();
    assertEqual(status, 'Active', 'Bob 应是 Active 状态');
  });
  // 获取 makerId (在 check 外部也需要)
  const bobMakerIdRaw = await (api.query as any).tradingMaker.accountToMaker(bob.address);
  makerId = bobMakerIdRaw.unwrap().toNumber();

  // ============ 主流程: 创建 → 付款 → 释放 ============

  // --------------- Step 1: 查询余额 ---------------
  const charlieBalanceBefore = await getFreeBalance(api, charlie.address);
  await ctx.check('查询 Charlie 初始余额', 'charlie', () => {
    console.log(`    Charlie 余额: ${Number(charlieBalanceBefore) / 1e12} NEX`);
  });

  // --------------- Step 2: 创建 Buy 订单 ---------------
  const nextOrderId = await (api.query as any).tradingP2p.nextBuyOrderId();
  const orderId = nextOrderId.toNumber();

  const paymentCommit = blake2AsHex(`payment:${charlie.address}:${Date.now()}`);
  const contactCommit = blake2AsHex(`contact:wechat_charlie:${Date.now()}`);

  const createTx = (api.tx as any).tradingP2p.createBuyOrder(
    makerId,
    nex(100).toString(),
    paymentCommit,
    contactCommit,
  );
  const createResult = await ctx.send(createTx, charlie, '创建 Buy 订单 (100 NEX)', 'charlie');
  assertTxSuccess(createResult, '创建 Buy 订单');
  assertEventEmitted(createResult, 'tradingP2p', 'BuyOrderCreated', '创建订单事件');

  // --------------- Step 3: 验证订单状态 ---------------
  await ctx.check('验证订单已创建', 'charlie', async () => {
    await assertStorageField(
      api, 'tradingP2p', 'buyOrders', [orderId],
      'state', 'Created', '订单状态应为 Created',
    );
  });

  // --------------- Step 4: 标记已付款 ---------------
  const tronTxHash = `${Date.now().toString(16)}abcdef1234567890`;
  const markPaidTx = (api.tx as any).tradingP2p.markPaid(
    orderId,
    tronTxHash,
  );
  const paidResult = await ctx.send(markPaidTx, charlie, '标记已付款', 'charlie');
  assertTxSuccess(paidResult, '标记付款');

  await ctx.check('验证订单状态为 Paid', 'charlie', async () => {
    await assertStorageField(
      api, 'tradingP2p', 'buyOrders', [orderId],
      'state', 'Paid', '订单状态应为 Paid',
    );
  });

  // --------------- Step 5: 做市商释放 NEX ---------------
  const releaseTx = (api.tx as any).tradingP2p.releaseNex(orderId);
  const releaseResult = await ctx.send(releaseTx, bob, '释放 NEX', 'bob');
  assertTxSuccess(releaseResult, '释放 NEX');

  // --------------- Step 6: 验证最终状态 ---------------
  await ctx.check('验证订单完成 + Charlie 收到 NEX', 'charlie', async () => {
    await assertStorageField(
      api, 'tradingP2p', 'buyOrders', [orderId],
      'state', 'Released', '订单状态应为 Released',
    );

    const charlieBalanceAfter = await getFreeBalance(api, charlie.address);
    // Charlie 应增加约 100 NEX (减去手续费)
    const delta = charlieBalanceAfter - charlieBalanceBefore;
    assertTrue(delta > nex(90), `Charlie 应收到约 100 NEX, 实际增加 ${Number(delta) / 1e12}`);
  });

  // --------------- Step 7: 错误路径 — 重复释放 ---------------
  const dupRelease = (api.tx as any).tradingP2p.releaseNex(orderId);
  const dupResult = await ctx.send(dupRelease, bob, '[错误路径] 重复释放 NEX', 'bob');
  await ctx.check('重复释放应失败', 'bob', () => {
    assertTxFailed(dupResult, undefined, '重复释放');
  });

  // ============ 分支: 取消订单 ============

  const nextOrderId2 = await (api.query as any).tradingP2p.nextBuyOrderId();
  const cancelOrderId = nextOrderId2.toNumber();

  const payCommit2 = blake2AsHex(`payment:cancel:${Date.now()}`);
  const contCommit2 = blake2AsHex(`contact:cancel:${Date.now()}`);

  const createTx2 = (api.tx as any).tradingP2p.createBuyOrder(
    makerId,
    nex(50).toString(),
    payCommit2,
    contCommit2,
  );
  const create2 = await ctx.send(createTx2, charlie, '创建待取消订单 (50 NEX)', 'charlie');
  assertTxSuccess(create2, '创建待取消订单');

  const cancelTx = (api.tx as any).tradingP2p.cancelBuyOrder(cancelOrderId);
  const cancelResult = await ctx.send(cancelTx, charlie, '取消 Buy 订单', 'charlie');
  assertTxSuccess(cancelResult, '取消订单');

  await ctx.check('验证取消后订单状态', 'charlie', async () => {
    await assertStorageField(
      api, 'tradingP2p', 'buyOrders', [cancelOrderId],
      'state', 'Cancelled', '订单状态应为 Cancelled',
    );
  });

  // ============ 分支: 争议订单 ============

  const nextOrderId3 = await (api.query as any).tradingP2p.nextBuyOrderId();
  const disputeOrderId = nextOrderId3.toNumber();

  const payCommit3 = blake2AsHex(`payment:dispute:${Date.now()}`);
  const contCommit3 = blake2AsHex(`contact:dispute:${Date.now()}`);

  const createTx3 = (api.tx as any).tradingP2p.createBuyOrder(
    makerId,
    nex(30).toString(),
    payCommit3,
    contCommit3,
  );
  const create3 = await ctx.send(createTx3, charlie, '创建待争议订单 (30 NEX)', 'charlie');
  assertTxSuccess(create3, '创建待争议订单');

  // 先标记付款 (争议需要在 Paid 之后)
  const markPaid3 = (api.tx as any).tradingP2p.markPaid(disputeOrderId, null);
  const paid3 = await ctx.send(markPaid3, charlie, '争议订单标记付款', 'charlie');
  assertTxSuccess(paid3, '争议订单标记付款');

  const disputeTx = (api.tx as any).tradingP2p.disputeBuyOrder(disputeOrderId);
  const disputeResult = await ctx.send(disputeTx, charlie, '发起 Buy 争议', 'charlie');
  assertTxSuccess(disputeResult, '发起争议');

  await ctx.check('验证争议状态', 'charlie', async () => {
    await assertStorageField(
      api, 'tradingP2p', 'buyOrders', [disputeOrderId],
      'state', 'Disputed', '订单状态应为 Disputed',
    );
  });

  // --------------- 汇总 ---------------
  await ctx.check('P2P Buy 流程汇总', 'system', () => {
    console.log(`    主流程订单 #${orderId}: Created → Paid → Released ✓`);
    console.log(`    取消分支订单 #${cancelOrderId}: Created → Cancelled ✓`);
    console.log(`    争议分支订单 #${disputeOrderId}: Created → Paid → Disputed ✓`);
  });
}
