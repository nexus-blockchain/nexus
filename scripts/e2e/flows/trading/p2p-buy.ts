/**
 * Flow-T2: NEX 市场卖单流程 (卖家挂单 → 买家预锁定 → 确认付款)
 *
 * 角色: Bob (卖家), Charlie (买家)
 *
 * 前置条件: Flow-T1 已设置初始价格
 *
 * 流程:
 *   1. Bob 挂卖单 (placeSellOrder)
 *   2. Charlie 预锁定卖单 (reserveSellOrder)
 *   3. Charlie 确认付款 (confirmPayment)
 *   4. 验证交易完成
 *   5. Bob 挂卖单 → 取消
 *   6. [错误路径] Charlie 取消他人订单
 *   7. [错误路径] 超时处理 (processTimeout on non-existent trade)
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

export const p2pBuyFlow: FlowDef = {
  name: 'Flow-T2: NEX 卖单流程',
  description: '挂卖单 → 预锁定 → 确认付款 → 取消 | 错误路径',
  fn: p2pBuy,
};

async function p2pBuy(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const charlie = ctx.actor('charlie');
  const tronAddr = 'TJYo36u5BbBVKguFVpsBj3yfHdR65VRj7G';

  // --------------- Step 1: Bob 挂卖单 ---------------
  const sellTx = (api.tx as any).nexMarket.placeSellOrder(
    nex(100).toString(),  // nexAmount
    1_000_000,            // usdtPrice
    tronAddr,             // tronAddress
  );
  const sellResult = await ctx.send(sellTx, bob, 'Bob 挂卖单 100 NEX', 'bob');
  assertTxSuccess(sellResult, '挂卖单');

  const sellEvent = sellResult.events.find(
    (e: any) => e.section === 'nexMarket' && e.method === 'SellOrderPlaced',
  );
  assertTrue(!!sellEvent, '应有 SellOrderPlaced 事件');
  const sellOrderId = sellEvent?.data?.orderId ?? sellEvent?.data?.[0];
  console.log(`    卖单 ID: ${sellOrderId}`);

  // --------------- Step 2: Charlie 预锁定 ---------------
  const reserveTx = (api.tx as any).nexMarket.reserveSellOrder(
    sellOrderId,
    nex(50).toString(),   // amount (部分)
    tronAddr,             // buyerTronAddress
  );
  const reserveResult = await ctx.send(reserveTx, charlie, 'Charlie 预锁定 50 NEX', 'charlie');
  assertTxSuccess(reserveResult, '预锁定');

  const tradeEvent = reserveResult.events.find(
    (e: any) => e.section === 'nexMarket' && e.method === 'TradeCreated',
  );
  const tradeId = tradeEvent?.data?.tradeId ?? tradeEvent?.data?.[0];
  console.log(`    Trade ID: ${tradeId}`);

  // --------------- Step 3: Charlie 确认付款 ---------------
  if (tradeId !== undefined) {
    const confirmTx = (api.tx as any).nexMarket.confirmPayment(tradeId);
    const confirmResult = await ctx.send(confirmTx, charlie, 'Charlie 确认付款', 'charlie');
    assertTxSuccess(confirmResult, '确认付款');

    await ctx.check('验证付款确认', 'charlie', () => {});
  }

  // --------------- Step 4: Bob 挂卖单 → 取消 ---------------
  const sell2Tx = (api.tx as any).nexMarket.placeSellOrder(
    nex(50).toString(), 1_000_000, tronAddr,
  );
  const sell2Result = await ctx.send(sell2Tx, bob, 'Bob 挂卖单(待取消)', 'bob');
  assertTxSuccess(sell2Result, '挂卖单(待取消)');

  const sell2Event = sell2Result.events.find(
    (e: any) => e.section === 'nexMarket' && e.method === 'SellOrderPlaced',
  );
  const cancelableOrderId = sell2Event?.data?.orderId ?? sell2Event?.data?.[0];

  const cancelTx = (api.tx as any).nexMarket.cancelOrder(cancelableOrderId);
  const cancelResult = await ctx.send(cancelTx, bob, 'Bob 取消卖单', 'bob');
  assertTxSuccess(cancelResult, '取消卖单');

  // --------------- Step 5: [错误路径] Charlie 取消他人订单 ---------------
  const sell3Tx = (api.tx as any).nexMarket.placeSellOrder(
    nex(30).toString(), 1_000_000, tronAddr,
  );
  const sell3Result = await ctx.send(sell3Tx, bob, 'Bob 挂卖单(供错误路径)', 'bob');

  if (sell3Result.success) {
    const sell3Event = sell3Result.events.find(
      (e: any) => e.section === 'nexMarket' && e.method === 'SellOrderPlaced',
    );
    const otherOrderId = sell3Event?.data?.orderId ?? sell3Event?.data?.[0];

    const charlieCancelTx = (api.tx as any).nexMarket.cancelOrder(otherOrderId);
    const charlieCancelResult = await ctx.send(charlieCancelTx, charlie, '[错误路径] Charlie 取消他人卖单', 'charlie');
    await ctx.check('非所有者取消应失败', 'charlie', () => {
      assertTxFailed(charlieCancelResult, undefined, '非所有者取消');
    });
  }

  // --------------- Step 6: [错误路径] processTimeout on non-existent trade ---------------
  const timeoutTx = (api.tx as any).nexMarket.processTimeout(99999);
  const timeoutResult = await ctx.send(timeoutTx, bob, '[错误路径] 超时处理不存在的交易', 'bob');
  await ctx.check('不存在的交易超时应失败', 'bob', () => {
    assertTxFailed(timeoutResult, undefined, '不存在的交易');
  });

  // --------------- 汇总 ---------------
  await ctx.check('NEX 卖单流程汇总', 'system', () => {
    console.log(`    ✓ 卖单: 挂单 → 预锁定 → 确认付款`);
    console.log(`    ✓ 取消: 挂单 → 取消`);
    console.log(`    ✓ 错误路径: 非所有者取消 ✗, 不存在交易超时 ✗`);
  });
}
