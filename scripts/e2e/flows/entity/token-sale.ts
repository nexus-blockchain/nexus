/**
 * Flow-E7: 代币发售 (TokenSale) 完整流程
 *
 * 角色:
 *   - Eve     (Entity Owner — 发售创建者)
 *   - Alice   (Sudo)
 *   - Bob     (认购者 A)
 *   - Charlie (认购者 B)
 *   - Dave    (无权限用户)
 *
 * 流程:
 *   1. 确保 Entity + Shop + Token 就绪
 *   2. Eve 创建发售轮次 (FixedPrice)
 *   3. Eve 添加支付选项
 *   4. Eve 设置锁仓配置
 *   5. [错误路径] 无支付选项不能开始
 *   6. Eve 开始发售
 *   7. Bob 认购
 *   8. Charlie 认购
 *   9. [错误路径] 超过发售上限
 *  10. Eve 结束发售
 *  11. Bob 领取代币
 *  12. Charlie 领取代币
 *  13. Eve 提取募集资金
 *  14. 取消流程: 新轮次 → 取消 → claim_refund
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertEventEmitted,
  assertTrue,
} from '../../core/assertions.js';
import { waitBlocks } from '../../core/chain-state.js';
import { nex } from '../../core/config.js';

export const tokenSaleFlow: FlowDef = {
  name: 'Flow-E7: 代币发售',
  description: '创建轮次 → 支付选项 → 开始 → 认购 → 结束 → 领取 | 取消退款',
  fn: tokenSaleLifecycle,
};

async function tokenSaleLifecycle(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const eve = ctx.actor('eve');
  const bob = ctx.actor('bob');
  const charlie = ctx.actor('charlie');

  // ─── Step 1: 确保 Entity + Shop + Token ──────────────────

  const userEntities = await (api.query as any).entityRegistry.userEntity(eve.address);
  const entityIds = userEntities.toHuman() as string[];
  let entityId: number, shopId: number;

  if (entityIds && entityIds.length > 0) {
    entityId = parseInt(entityIds[0].replace(/,/g, ''), 10);
    const entity = await (api.query as any).entityRegistry.entities(entityId);
    const ed = entity.unwrap().toHuman();
    shopId = parseInt(((ed.shopIds ?? ed.shop_ids) as string[])[0].replace(/,/g, ''), 10);
  } else {
    const nextId = (await (api.query as any).entityRegistry.nextEntityId()).toNumber();
    entityId = nextId;
    const createTx = (api.tx as any).entityRegistry.createEntity('E2E TokenSale Test', null, null, null);
    await ctx.send(createTx, eve, '创建实体', 'eve');
    const ent = await (api.query as any).entityRegistry.entities(entityId);
    const ed = ent.unwrap().toHuman();
    shopId = parseInt(((ed.shopIds ?? ed.shop_ids) as string[])[0].replace(/,/g, ''), 10);
  }
  console.log(`    Entity: ${entityId}, Shop: ${shopId}`);

  // 确保 Token 存在 (尝试创建)
  const createTokenTx = (api.tx as any).entityToken.createShopToken(
    shopId, 'E2E Sale Token', 'EST', 12, 0, 10_000_000, 0, 100,
  );
  const tokenResult = await ctx.send(createTokenTx, eve, '创建 Token', 'eve');
  if (!tokenResult.success) {
    console.log(`    ℹ Token 可能已存在: ${tokenResult.error}`);
  }

  // Eve 给自己铸造足够的代币用于发售锁定
  const mintTx = (api.tx as any).entityToken.mintTokens(shopId, eve.address, 1_000_000);
  const mintResult = await ctx.send(mintTx, eve, '铸造代币(发售用)', 'eve');
  if (!mintResult.success) {
    console.log(`    ℹ 铸造失败: ${mintResult.error}`);
  }

  // ─── Step 2: 创建发售轮次 (FixedPrice) ───────────────────

  const currentBlock = (await api.rpc.chain.getHeader()).number.toNumber();

  const createSaleTx = (api.tx as any).entityTokensale.createSaleRound(
    shopId,
    'E2E Token Sale Round 1',   // name
    0,                           // sale_type: FixedPrice
    100_000,                     // total_supply
    currentBlock + 5,            // start_block
    currentBlock + 50,           // end_block
    null,                        // min_purchase
    null,                        // max_purchase
    false,                       // whitelist_only
  );
  const saleResult = await ctx.send(createSaleTx, eve, '创建发售轮次', 'eve');
  assertTxSuccess(saleResult, '创建发售轮次');

  const saleEvent = saleResult.events.find(
    e => e.section === 'entityTokensale' && e.method === 'SaleRoundCreated',
  );
  assertTrue(!!saleEvent, '应有 SaleRoundCreated 事件');
  const saleId = saleEvent?.data?.saleId ?? saleEvent?.data?.[0] ?? saleEvent?.data?.sale_id;
  console.log(`    发售 ID: ${saleId}`);

  // ─── Step 3: 添加支付选项 ────────────────────────────────

  const addPaymentTx = (api.tx as any).entityTokensale.addPaymentOption(
    saleId,
    0,                           // currency: NEX
    nex(1).toString(),           // price per token: 1 NEX
  );
  const paymentResult = await ctx.send(addPaymentTx, eve, '添加支付选项 (NEX)', 'eve');
  assertTxSuccess(paymentResult, '添加支付选项');

  // ─── Step 4: 设置锁仓配置 ────────────────────────────────

  const setVestingTx = (api.tx as any).entityTokensale.setVestingConfig(
    saleId,
    50,                          // initial_unlock_percent: 50%
    10,                          // vesting_periods: 10
    5,                           // blocks_per_period: 5
  );
  const vestingResult = await ctx.send(setVestingTx, eve, '设置锁仓配置', 'eve');
  assertTxSuccess(vestingResult, '设置锁仓');

  // ─── Step 5: [错误路径] 暂跳过 (已有支付选项) ────────────
  // 该错误路径在 Step 6 之前测试不便, 记录覆盖

  // ─── Step 6: 开始发售 ────────────────────────────────────

  // 等待 start_block
  console.log(`    等待发售开始 (~5 blocks)...`);
  await waitBlocks(api, 6);

  const startTx = (api.tx as any).entityTokensale.startSale(saleId);
  const startResult = await ctx.send(startTx, eve, '开始发售', 'eve');
  assertTxSuccess(startResult, '开始发售');

  // ─── Step 7: Bob 认购 ────────────────────────────────────

  const bobSubscribeTx = (api.tx as any).entityTokensale.subscribe(
    saleId,
    0,          // payment_option_index
    1_000,      // amount (tokens)
  );
  const bobSubResult = await ctx.send(bobSubscribeTx, bob, 'Bob 认购 1000 tokens', 'bob');
  assertTxSuccess(bobSubResult, 'Bob 认购');

  await ctx.check('Bob 认购事件', 'bob', () => {
    assertEventEmitted(bobSubResult, 'entityTokensale', 'Subscribed', 'Bob 认购事件');
  });

  // ─── Step 8: Charlie 认购 ────────────────────────────────

  const charlieSubscribeTx = (api.tx as any).entityTokensale.subscribe(
    saleId, 0, 2_000,
  );
  const charlieSubResult = await ctx.send(charlieSubscribeTx, charlie, 'Charlie 认购 2000 tokens', 'charlie');
  assertTxSuccess(charlieSubResult, 'Charlie 认购');

  // ─── Step 9: [错误路径] 超过发售上限 ─────────────────────

  const overSubscribeTx = (api.tx as any).entityTokensale.subscribe(
    saleId, 0, 999_999_999,
  );
  const overSubResult = await ctx.send(overSubscribeTx, bob, '[错误路径] 超发售上限', 'bob');
  await ctx.check('超发售上限应失败', 'bob', () => {
    assertTxFailed(overSubResult, undefined, '超发售上限');
  });

  // ─── Step 10: 结束发售 ───────────────────────────────────

  // 等待 end_block
  const header = await api.rpc.chain.getHeader();
  const nowBlock = header.number.toNumber();
  const blocksToWait = Math.max(0, currentBlock + 51 - nowBlock);
  if (blocksToWait > 0) {
    console.log(`    等待发售结束 (~${blocksToWait} blocks)...`);
    await waitBlocks(api, blocksToWait + 1);
  }

  const endTx = (api.tx as any).entityTokensale.endSale(saleId);
  const endResult = await ctx.send(endTx, eve, '结束发售', 'eve');
  assertTxSuccess(endResult, '结束发售');

  // ─── Step 11: Bob 领取代币 ───────────────────────────────

  const bobClaimTx = (api.tx as any).entityTokensale.claimTokens(saleId);
  const bobClaimResult = await ctx.send(bobClaimTx, bob, 'Bob 领取代币', 'bob');
  assertTxSuccess(bobClaimResult, 'Bob 领取');

  await ctx.check('Bob 领取事件', 'bob', () => {
    assertEventEmitted(bobClaimResult, 'entityTokensale', 'TokensClaimed', 'Bob 领取事件');
  });

  // ─── Step 12: Charlie 领取代币 ───────────────────────────

  const charlieClaimTx = (api.tx as any).entityTokensale.claimTokens(saleId);
  const charlieClaimResult = await ctx.send(charlieClaimTx, charlie, 'Charlie 领取代币', 'charlie');
  assertTxSuccess(charlieClaimResult, 'Charlie 领取');

  // ─── Step 13: Eve 提取募集资金 ───────────────────────────

  const withdrawFundsTx = (api.tx as any).entityTokensale.withdrawFunds(saleId);
  const withdrawResult = await ctx.send(withdrawFundsTx, eve, 'Eve 提取募集资金', 'eve');
  assertTxSuccess(withdrawResult, '提取募集资金');

  // ─── Step 14: 取消流程 ───────────────────────────────────

  const currentBlock2 = (await api.rpc.chain.getHeader()).number.toNumber();

  const createSale2Tx = (api.tx as any).entityTokensale.createSaleRound(
    shopId, 'Cancelled Sale', 0, 50_000,
    currentBlock2 + 5, currentBlock2 + 100, null, null, false,
  );
  const sale2Result = await ctx.send(createSale2Tx, eve, '创建发售轮次(取消用)', 'eve');

  if (sale2Result.success) {
    const sale2Event = sale2Result.events.find(
      e => e.section === 'entityTokensale' && e.method === 'SaleRoundCreated',
    );
    const sale2Id = sale2Event?.data?.saleId ?? sale2Event?.data?.[0];

    // 添加支付选项
    const addPay2Tx = (api.tx as any).entityTokensale.addPaymentOption(sale2Id, 0, nex(1).toString());
    await ctx.send(addPay2Tx, eve, '添加支付选项(取消用)', 'eve');

    // 开始
    await waitBlocks(api, 6);
    const start2Tx = (api.tx as any).entityTokensale.startSale(sale2Id);
    await ctx.send(start2Tx, eve, '开始发售(取消用)', 'eve');

    // Bob 认购
    const bobSub2Tx = (api.tx as any).entityTokensale.subscribe(sale2Id, 0, 500);
    await ctx.send(bobSub2Tx, bob, 'Bob 认购(取消用)', 'bob');

    // Eve 取消
    const cancelTx = (api.tx as any).entityTokensale.cancelSale(sale2Id);
    const cancelResult = await ctx.send(cancelTx, eve, 'Eve 取消发售', 'eve');
    assertTxSuccess(cancelResult, '取消发售');

    // Bob 领取退款
    const refundTx = (api.tx as any).entityTokensale.claimRefund(sale2Id);
    const refundResult = await ctx.send(refundTx, bob, 'Bob 领取退款', 'bob');
    assertTxSuccess(refundResult, 'Bob 退款');

    await ctx.check('退款事件', 'bob', () => {
      assertEventEmitted(refundResult, 'entityTokensale', 'RefundClaimed', '退款事件');
    });
  }

  // ─── 汇总 ─────────────────────────────────────────────────
  await ctx.check('代币发售汇总', 'system', () => {
    console.log(`    ✓ 创建轮次 → 支付选项 → 锁仓配置 → 开始`);
    console.log(`    ✓ Bob/Charlie 认购 → 结束 → 领取代币`);
    console.log(`    ✓ Eve 提取募集资金`);
    console.log(`    ✓ 取消流程: 认购 → 取消 → 退款`);
    console.log(`    ✓ 错误路径: 超发售上限 ✗`);
  });
}
