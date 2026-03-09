/**
 * Flow-T5: NEX Market 管理/争议回归
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertEventEmitted,
  assertTxFailed,
  assertTxSuccess,
  assertTrue,
} from '../../core/assertions.js';
import { nex } from '../../core/config.js';

export const nexMarketAdminFlow: FlowDef = {
  name: 'Flow-T5: NEX Market 管理/争议',
  description: '订单改量 + 用户封禁/解封 + 争议补证失败路径 + 批量强制操作',
  fn: runNexMarketAdminFlow,
};

async function runNexMarketAdminFlow(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');

  const tronAddr = 'T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb';

  const protectTx = (api.tx as any).nexMarket.configurePriceProtection(true, 500, 5000, 5);
  const protectResult = await ctx.sudo(protectTx, '[错误路径] Root 尝试配置价格保护 (T5)');
  await ctx.check('Root 无法调用 MarketAdminOrigin: configure_price_protection', 'sudo(alice)', () => {
    assertTxFailed(protectResult, 'BadOrigin', 'configure_price_protection');
  });

  const setPriceTx = (api.tx as any).nexMarket.setInitialPrice(1_000_000);
  const setPriceResult = await ctx.sudo(setPriceTx, '[错误路径] Root 尝试设置初始价格 (T5)');
  await ctx.check('Root 无法调用 MarketAdminOrigin: set_initial_price', 'sudo(alice)', () => {
    assertTxFailed(setPriceResult, 'BadOrigin', 'set_initial_price');
  });

  const placeSellTx = (api.tx as any).nexMarket.placeSellOrder(
    nex(120).toString(),
    1,
    tronAddr,
    null,
  );
  const placeSellResult = await ctx.send(placeSellTx, bob, 'Bob 挂卖单 (T5)', 'bob');
  assertTxSuccess(placeSellResult, '挂卖单');

  const orderEvent = placeSellResult.events.find(
    e => e.section === 'nexMarket' && e.method === 'OrderCreated',
  );
  assertTrue(!!orderEvent, '应产生 OrderCreated');
  const orderId = orderEvent?.data?.orderId ?? orderEvent?.data?.[0];

  const updateAmountTx = (api.tx as any).nexMarket.updateOrderAmount(
    orderId,
    nex(100).toString(),
  );
  const updateAmountResult = await ctx.send(updateAmountTx, bob, 'Bob 修改订单数量', 'bob');
  assertTxSuccess(updateAmountResult, '修改订单数量');
  await ctx.check('订单数量修改事件', 'bob', () => {
    assertEventEmitted(updateAmountResult, 'nexMarket', 'OrderAmountUpdated', '改量事件');
  });

  const banTx = (api.tx as any).nexMarket.banUser(bob.address);
  const banResult = await ctx.sudo(banTx, '[错误路径] Root 尝试封禁 Bob');
  await ctx.check('Root 无法调用 MarketAdminOrigin: ban_user', 'sudo(alice)', () => {
    assertTxFailed(banResult, 'BadOrigin', 'ban_user');
  });

  const unbanTx = (api.tx as any).nexMarket.unbanUser(bob.address);
  const unbanResult = await ctx.sudo(unbanTx, '[错误路径] Root 尝试解封 Bob');
  await ctx.check('Root 无法调用 MarketAdminOrigin: unban_user', 'sudo(alice)', () => {
    assertTxFailed(unbanResult, 'BadOrigin', 'unban_user');
  });

  const fakeTradeId = 9_999_991;
  const counterEvidenceTx = (api.tx as any).nexMarket.submitCounterEvidence(
    fakeTradeId,
    'QmT5CounterEvidence',
  );
  const counterEvidenceResult = await ctx.send(
    counterEvidenceTx,
    bob,
    '[错误路径] 不存在交易提交反驳证据',
    'bob',
  );
  await ctx.check('不存在交易提交反驳证据失败', 'bob', () => {
    assertTxFailed(counterEvidenceResult, 'UsdtTradeNotFound', 'counter evidence');
  });

  const sellerConfirmTx = (api.tx as any).nexMarket.sellerConfirmReceived(fakeTradeId);
  const sellerConfirmResult = await ctx.send(
    sellerConfirmTx,
    bob,
    '[错误路径] 不存在交易卖家确认收款',
    'bob',
  );
  await ctx.check('不存在交易卖家确认收款失败', 'bob', () => {
    assertTxFailed(sellerConfirmResult, 'UsdtTradeNotFound', 'seller confirm');
  });

  const batchForceSettleTx = (api.tx as any).nexMarket.batchForceSettle(
    [fakeTradeId],
    1_000_000,
    'ReleaseToBuyer',
  );
  const batchForceSettleResult = await ctx.sudo(batchForceSettleTx, '[错误路径] Root 尝试批量强制结算');
  await ctx.check('Root 无法调用 MarketAdminOrigin: batch_force_settle', 'sudo(alice)', () => {
    assertTxFailed(batchForceSettleResult, 'BadOrigin', 'batch_force_settle');
  });

  const batchForceCancelTx = (api.tx as any).nexMarket.batchForceCancel([fakeTradeId]);
  const batchForceCancelResult = await ctx.sudo(batchForceCancelTx, '[错误路径] Root 尝试批量强制取消');
  await ctx.check('Root 无法调用 MarketAdminOrigin: batch_force_cancel', 'sudo(alice)', () => {
    assertTxFailed(batchForceCancelResult, 'BadOrigin', 'batch_force_cancel');
  });

  const cleanupTx = (api.tx as any).nexMarket.cancelOrder(orderId);
  const cleanupResult = await ctx.send(cleanupTx, bob, 'Bob 取消 T5 测试订单', 'bob');
  if (cleanupResult.success) {
    await ctx.check('测试订单已取消', 'bob', () => {
      assertEventEmitted(cleanupResult, 'nexMarket', 'OrderCancelled', '清理订单');
    });
  } else {
    await ctx.check('测试订单清理失败已记录', 'bob', () => {
      console.log(`    ℹ T5 清理订单失败: ${cleanupResult.error}`);
    });
  }
}
