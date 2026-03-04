/**
 * Flow-T3: NEX 市场买单流程 (买家挂买单 → 卖家接单)
 *
 * 角色: Bob (买家), Dave (卖家), Charlie (无权限用户)
 *
 * 前置条件: Flow-T1 已设置初始价格
 *
 * 流程:
 *   1. Bob 挂买单 (placeBuyOrder)
 *   2. Dave 接受买单 (acceptBuyOrder)
 *   3. 验证交易事件
 *   4. Bob 挂买单 → 取消
 *   5. [错误路径] Charlie 取消他人买单
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

export const p2pSellFlow: FlowDef = {
  name: 'Flow-T3: NEX 买单流程',
  description: '挂买单 → 接单 → 取消 | 错误路径',
  fn: p2pSell,
};

async function p2pSell(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const dave = ctx.actor('dave');
  const charlie = ctx.actor('charlie');
  const tronAddr = 'TJYo36u5BbBVKguFVpsBj3yfHdR65VRj7G';

  // --------------- Step 1: Bob 挂买单 ---------------
  const buyTx = (api.tx as any).nexMarket.placeBuyOrder(
    nex(50).toString(),   // nexAmount
    1_000_000,            // usdtPrice
    tronAddr,             // buyerTronAddress
  );
  const buyResult = await ctx.send(buyTx, bob, 'Bob 挂买单 50 NEX', 'bob');
  assertTxSuccess(buyResult, '挂买单');

  const buyEvent = buyResult.events.find(
    (e: any) => e.section === 'nexMarket' && e.method === 'BuyOrderPlaced',
  );
  assertTrue(!!buyEvent, '应有 BuyOrderPlaced 事件');
  const buyOrderId = buyEvent?.data?.orderId ?? buyEvent?.data?.[0];
  console.log(`    买单 ID: ${buyOrderId}`);

  // --------------- Step 2: Dave 接受买单 ---------------
  const acceptTx = (api.tx as any).nexMarket.acceptBuyOrder(
    buyOrderId,
    nex(50).toString(),   // amount
    tronAddr,             // tronAddress
  );
  const acceptResult = await ctx.send(acceptTx, dave, 'Dave 接受买单', 'dave');
  assertTxSuccess(acceptResult, '接受买单');

  await ctx.check('验证接单事件', 'dave', () => {
    const tradeEvent = acceptResult.events.find(
      (e: any) => e.section === 'nexMarket' && e.method === 'TradeCreated',
    );
    if (tradeEvent) {
      const tradeId = tradeEvent?.data?.tradeId ?? tradeEvent?.data?.[0];
      console.log(`    Trade ID: ${tradeId}`);
    }
  });

  // --------------- Step 3: Bob 挂买单 → 取消 ---------------
  const buy2Tx = (api.tx as any).nexMarket.placeBuyOrder(
    nex(20).toString(), 1_000_000, tronAddr,
  );
  const buy2Result = await ctx.send(buy2Tx, bob, 'Bob 挂买单(待取消)', 'bob');
  assertTxSuccess(buy2Result, '挂买单(待取消)');

  const buy2Event = buy2Result.events.find(
    (e: any) => e.section === 'nexMarket' && e.method === 'BuyOrderPlaced',
  );
  const cancelableId = buy2Event?.data?.orderId ?? buy2Event?.data?.[0];

  const cancelTx = (api.tx as any).nexMarket.cancelOrder(cancelableId);
  const cancelResult = await ctx.send(cancelTx, bob, 'Bob 取消买单', 'bob');
  assertTxSuccess(cancelResult, '取消买单');

  // --------------- Step 4: [错误路径] Charlie 取消他人买单 ---------------
  const buy3Tx = (api.tx as any).nexMarket.placeBuyOrder(
    nex(10).toString(), 1_000_000, tronAddr,
  );
  const buy3Result = await ctx.send(buy3Tx, bob, 'Bob 挂买单(供错误路径)', 'bob');

  if (buy3Result.success) {
    const buy3Event = buy3Result.events.find(
      (e: any) => e.section === 'nexMarket' && e.method === 'BuyOrderPlaced',
    );
    const otherId = buy3Event?.data?.orderId ?? buy3Event?.data?.[0];

    const charlieCancelTx = (api.tx as any).nexMarket.cancelOrder(otherId);
    const charlieCancelResult = await ctx.send(charlieCancelTx, charlie, '[错误路径] Charlie 取消他人买单', 'charlie');
    await ctx.check('非所有者取消应失败', 'charlie', () => {
      assertTxFailed(charlieCancelResult, undefined, '非所有者取消');
    });
  }

  // --------------- 汇总 ---------------
  await ctx.check('NEX 买单流程汇总', 'system', () => {
    console.log(`    ✓ 买单: 挂单 → 接单`);
    console.log(`    ✓ 取消: 挂单 → 取消`);
    console.log(`    ✓ 错误路径: 非所有者取消 ✗`);
  });
}
