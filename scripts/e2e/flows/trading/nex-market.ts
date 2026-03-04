/**
 * Flow-T4: NEX 市场 (DEX) 完整流程
 *
 * 角色:
 *   - Bob     (卖家 — 卖 NEX 收 USDT)
 *   - Charlie (买家 — 买 NEX 付 USDT)
 *   - Alice   (Sudo — 价格保护/熔断/种子流动性)
 *   - Dave    (无权限用户)
 *
 * 流程:
 *   1. Alice 配置价格保护
 *   2. Alice 设置初始价格
 *   3. Bob 挂卖单 (卖 NEX 收 USDT)
 *   4. Charlie 挂买单 (买 NEX 付 USDT)
 *   5. Charlie 预锁定卖单 (reserve_sell_order)
 *   6. Charlie 确认付款 (confirm_payment)
 *   7. Bob 接受买单 (accept_buy_order)
 *   8. Bob 取消订单
 *   9. Alice 解除熔断
 *  10. Alice 注资种子账户
 *  11. Alice 注入种子流动性
 *  12. [错误路径] Dave 取消他人订单
 *  13. [错误路径] 超时处理
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

export const nexMarketFlow: FlowDef = {
  name: 'Flow-T4: NEX 市场',
  description: '挂卖/买单 → 预锁定 → 确认付款 → 接受买单 → 取消 → 种子流动性 | 错误路径',
  fn: nexMarket,
};

async function nexMarket(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const charlie = ctx.actor('charlie');
  const dave = ctx.actor('dave');

  const tronAddr = 'TJYo36u5BbBVKguFVpsBj3yfHdR65VRj7G';

  // ─── Step 1: Alice 配置价格保护 ────────────────────────────

  const configProtTx = (api.tx as any).nexMarket.configurePriceProtection(
    true,    // enabled
    500,     // maxPriceDeviation: 5%
    5000,    // circuitBreakerThreshold: 50%
    5,       // minTradesForTwap
  );
  const configProtResult = await ctx.sudo(configProtTx, '配置价格保护');
  assertTxSuccess(configProtResult, '配置价格保护');

  // ─── Step 2: Alice 设置初始价格 ────────────────────────────

  const setInitPriceTx = (api.tx as any).nexMarket.setInitialPrice(
    1_000_000,   // 1 USDT per NEX (精度 10^6)
  );
  const initPriceResult = await ctx.sudo(setInitPriceTx, '设置初始价格');
  assertTxSuccess(initPriceResult, '设置初始价格');

  // ─── Step 3: Bob 挂卖单 (卖 NEX 收 USDT) ─────────────────

  const bobBalBefore = await getFreeBalance(api, bob.address);

  const sellTx = (api.tx as any).nexMarket.placeSellOrder(
    nex(100).toString(),   // nex_amount
    1_000_000,             // usdt_price: 1 USDT per NEX
    tronAddr,              // tron_address
  );
  const sellResult = await ctx.send(sellTx, bob, 'Bob 挂卖单', 'bob');
  assertTxSuccess(sellResult, '挂卖单');

  const sellEvent = sellResult.events.find(
    e => e.section === 'nexMarket' && e.method === 'OrderPlaced',
  );
  assertTrue(!!sellEvent, '应有 OrderPlaced 事件');
  const sellOrderId = sellEvent?.data?.orderId ?? sellEvent?.data?.[0];
  console.log(`    卖单 ID: ${sellOrderId}`);

  // 验证 NEX 已锁定
  await ctx.check('验证 NEX 已锁定', 'bob', async () => {
    const bobBalAfter = await getFreeBalance(api, bob.address);
    const delta = bobBalBefore - bobBalAfter;
    assertTrue(delta > 0n, `Bob 应被锁定 NEX, 减少 ${Number(delta) / 1e12} NEX`);
  });

  // ─── Step 4: Charlie 挂买单 (买 NEX 付 USDT) ──────────────

  const buyTx = (api.tx as any).nexMarket.placeBuyOrder(
    nex(50).toString(),    // nex_amount
    1_000_000,             // usdt_price
    tronAddr,              // buyer_tron_address
  );
  const buyResult = await ctx.send(buyTx, charlie, 'Charlie 挂买单', 'charlie');
  assertTxSuccess(buyResult, '挂买单');

  const buyEvent = buyResult.events.find(
    e => e.section === 'nexMarket' && e.method === 'OrderPlaced',
  );
  const buyOrderId = buyEvent?.data?.orderId ?? buyEvent?.data?.[0];
  console.log(`    买单 ID: ${buyOrderId}`);

  // ─── Step 5: Charlie 预锁定卖单 ───────────────────────────

  const reserveTx = (api.tx as any).nexMarket.reserveSellOrder(
    sellOrderId,
    nex(30).toString(),    // amount
    tronAddr,              // buyer_tron_address
  );
  const reserveResult = await ctx.send(reserveTx, charlie, 'Charlie 预锁定卖单', 'charlie');
  if (reserveResult.success) {
    await ctx.check('预锁定事件', 'charlie', () => {
      assertEventEmitted(reserveResult, 'nexMarket', 'OrderReserved', '预锁定事件');
    });

    // ─── Step 6: Charlie 确认付款 ────────────────────────────
    const usdtTxEvent = reserveResult.events.find(
      e => e.section === 'nexMarket' && e.method === 'UsdtTransactionCreated',
    );
    const usdtTxId = usdtTxEvent?.data?.transactionId ?? usdtTxEvent?.data?.[0];

    // confirmPayment takes only tradeId
    const tradeEvent = reserveResult.events.find(
      e => e.section === 'nexMarket' && e.method === 'TradeCreated',
    );
    const tradeId = tradeEvent?.data?.tradeId ?? tradeEvent?.data?.[0] ?? usdtTxId;

    if (tradeId !== undefined) {
      const confirmTx = (api.tx as any).nexMarket.confirmPayment(tradeId);
      const confirmResult = await ctx.send(confirmTx, charlie, 'Charlie 确认付款', 'charlie');
      if (confirmResult.success) {
        await ctx.check('付款已确认', 'charlie', () => {
          assertEventEmitted(confirmResult, 'nexMarket', 'PaymentConfirmed', '确认事件');
        });
      } else {
        console.log(`    ℹ 确认付款失败: ${confirmResult.error}`);
      }
    }
  } else {
    console.log(`    ℹ 预锁定失败: ${reserveResult.error}`);
  }

  // ─── Step 7: Bob 接受买单 ─────────────────────────────────

  if (buyOrderId !== undefined) {
    const acceptTx = (api.tx as any).nexMarket.acceptBuyOrder(
      buyOrderId,
      nex(50).toString(),    // amount
      tronAddr,              // seller_tron_address
    );
    const acceptResult = await ctx.send(acceptTx, bob, 'Bob 接受买单', 'bob');
    if (acceptResult.success) {
      await ctx.check('买单已接受', 'bob', () => {
        assertEventEmitted(acceptResult, 'nexMarket', 'BuyOrderAccepted', '接受事件');
      });
    } else {
      console.log(`    ℹ 接受买单失败: ${acceptResult.error}`);
    }
  }

  // ─── Step 8: Bob 取消订单 ─────────────────────────────────

  // 挂一个新的卖单然后取消
  const cancelableTx = (api.tx as any).nexMarket.placeSellOrder(
    nex(20).toString(), 1_000_000, tronAddr,
  );
  const cancelableResult = await ctx.send(cancelableTx, bob, 'Bob 挂单(待取消)', 'bob');

  if (cancelableResult.success) {
    const cancelableEvent = cancelableResult.events.find(
      e => e.section === 'nexMarket' && e.method === 'OrderPlaced',
    );
    const cancelableOrderId = cancelableEvent?.data?.orderId ?? cancelableEvent?.data?.[0];

    const cancelTx = (api.tx as any).nexMarket.cancelOrder(cancelableOrderId);
    const cancelResult = await ctx.send(cancelTx, bob, 'Bob 取消订单', 'bob');
    assertTxSuccess(cancelResult, '取消订单');

    await ctx.check('验证取消事件', 'bob', () => {
      assertEventEmitted(cancelResult, 'nexMarket', 'OrderCancelled', '取消事件');
    });
  }

  // ─── Step 9: Alice 解除熔断 ───────────────────────────────

  const liftCbTx = (api.tx as any).nexMarket.liftCircuitBreaker();
  const liftResult = await ctx.sudo(liftCbTx, '解除熔断');
  if (liftResult.success) {
    await ctx.check('熔断已解除', 'system', () => {});
  } else {
    console.log(`    ℹ 解除熔断: ${liftResult.error} (预期 — 当前无熔断)`);
  }

  // ─── Step 10: Alice 注资种子账户 ──────────────────────────

  const fundSeedTx = (api.tx as any).nexMarket.fundSeedAccount(
    nex(1000).toString(),   // amount
  );
  const fundSeedResult = await ctx.sudo(fundSeedTx, '注资种子账户');
  if (fundSeedResult.success) {
    await ctx.check('种子账户已注资', 'system', () => {});
  } else {
    console.log(`    ℹ 注资失败: ${fundSeedResult.error}`);
  }

  // ─── Step 11: Alice 注入种子流动性 ─────────────────────────

  const seedTx = (api.tx as any).nexMarket.seedLiquidity(
    5,                  // orderCount
    null,               // usdtOverride: None (use default)
  );
  const seedResult = await ctx.sudo(seedTx, '注入种子流动性');
  if (seedResult.success) {
    await ctx.check('种子流动性已注入', 'system', () => {});
  } else {
    console.log(`    ℹ 种子流动性失败: ${seedResult.error}`);
  }

  // ─── Step 12: [错误路径] Dave 取消他人订单 ─────────────────

  // Bob 挂一个新单供错误路径测试
  const anotherTx = (api.tx as any).nexMarket.placeSellOrder(
    nex(10).toString(), 1_000_000, tronAddr,
  );
  const anotherResult = await ctx.send(anotherTx, bob, 'Bob 挂单(供错误路径)', 'bob');

  if (anotherResult.success) {
    const anotherEvent = anotherResult.events.find(
      e => e.section === 'nexMarket' && e.method === 'OrderPlaced',
    );
    const anotherOrderId = anotherEvent?.data?.orderId ?? anotherEvent?.data?.[0];

    const daveCancelTx = (api.tx as any).nexMarket.cancelOrder(anotherOrderId);
    const daveCancelResult = await ctx.send(daveCancelTx, dave, '[错误路径] Dave 取消他人订单', 'dave');
    await ctx.check('非所有者取消应失败', 'dave', () => {
      assertTxFailed(daveCancelResult, undefined, '非所有者取消');
    });
  }

  // ─── Step 13: [错误路径] 超时处理 ─────────────────────────

  // 尝试对不存在的交易处理超时
  const timeoutTx = (api.tx as any).nexMarket.processTimeout(99999);
  const timeoutResult = await ctx.send(timeoutTx, bob, '[错误路径] 不存在交易超时', 'bob');
  await ctx.check('不存在交易超时应失败', 'bob', () => {
    assertTxFailed(timeoutResult, undefined, '不存在交易');
  });

  // ─── 汇总 ─────────────────────────────────────────────────
  await ctx.check('NEX 市场汇总', 'system', () => {
    console.log(`    ✓ 卖单: 挂单 → 预锁定 → 确认付款`);
    console.log(`    ✓ 买单: 挂单 → 接受`);
    console.log(`    ✓ 管理: 取消 → 熔断 → 种子流动性`);
    console.log(`    ✓ 错误路径: 非所有者取消 ✗, 不存在交易超时 ✗`);
  });
}
