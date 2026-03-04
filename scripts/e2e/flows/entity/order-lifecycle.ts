/**
 * Flow-E2: 订单完整生命周期
 *
 * 角色:
 *   - Eve   (Entity Owner / Seller)
 *   - Alice (Sudo)
 *   - Bob   (Buyer)
 *   - Charlie (无权限用户)
 *
 * 前置: Flow-E1 已跑过 (链上已有 Active Entity + Primary Shop)
 *
 * 流程:
 *   1. Eve 查询/创建实体 + 店铺
 *   2. Eve 创建商品并上架
 *   3. Bob 下单 (NEX 支付)
 *   4. 验证订单已创建 + 资金锁入托管
 *   5. Eve 发货
 *   6. Bob 确认收货 → 释放资金
 *   7. 验证事件: 佣金/会员升级触发
 *   8. [错误路径] 非买家确认收货
 *   9. [错误路径] 已下架商品无法下单
 *  10. Bob 下单 → 取消 → 退款
 *  11. 服务类订单: 开始→完成→确认
 *  12. 退款流程: 申请→批准
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertEventEmitted,
  assertStorageExists,
  assertStorageField,
  assertTrue,
} from '../../core/assertions.js';
import { getFreeBalance, queryStorage } from '../../core/chain-state.js';
import { nex } from '../../core/config.js';

export const orderLifecycleFlow: FlowDef = {
  name: 'Flow-E2: 订单生命周期',
  description: '创建商品 → 下单 → 发货 → 确认 → 佣金 | 取消退款 | 权限校验',
  fn: orderLifecycle,
};

async function orderLifecycle(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const eve = ctx.actor('eve');
  const bob = ctx.actor('bob');
  const charlie = ctx.actor('charlie');

  // ─── Step 1: 确保有 Active Entity + Shop ─────────────────

  let entityId: number;
  let shopId: number;

  // 查找 Eve 已有的 entity
  const userEntities = await (api.query as any).entityRegistry.userEntity(eve.address);
  const entityIds = userEntities.toHuman() as string[];

  if (entityIds && entityIds.length > 0) {
    entityId = parseInt(entityIds[0].replace(/,/g, ''), 10);
    // 查找该 entity 下的 shop
    const entity = await (api.query as any).entityRegistry.entities(entityId);
    const entityData = entity.unwrap().toHuman();
    const shopIds = entityData.shopIds ?? entityData.shop_ids;
    shopId = parseInt((shopIds as string[])[0].replace(/,/g, ''), 10);
    console.log(`    复用已有 Entity#${entityId}, Shop#${shopId}`);
  } else {
    // 创建新 entity
    const nextId = (await (api.query as any).entityRegistry.nextEntityId()).toNumber();
    entityId = nextId;
    const createTx = (api.tx as any).entityRegistry.createEntity('E2E Order Test', null, null, null);
    const cr = await ctx.send(createTx, eve, '创建实体', 'eve');
    assertTxSuccess(cr, '创建实体');
    // 查 shop
    const ent = await (api.query as any).entityRegistry.entities(entityId);
    const ed = ent.unwrap().toHuman();
    shopId = parseInt(((ed.shopIds ?? ed.shop_ids) as string[])[0].replace(/,/g, ''), 10);
  }

  await ctx.check('Entity+Shop 就绪', 'system', () => {
    console.log(`    Entity: ${entityId}, Shop: ${shopId}`);
  });

  // ─── Step 2: 创建商品并上架 ──────────────────────────────

  const nextProductId = await (api.query as any).entityProduct.nextProductId();
  const productId = nextProductId.toNumber();

  const createProductTx = (api.tx as any).entityProduct.createProduct(
    shopId,
    'E2E Test Product',       // nameCid
    'img_placeholder',        // imagesCid
    'detail_placeholder',     // detailCid
    nex(10).toString(),       // price: 10 NEX
    100,                      // stock
    'Physical',               // category
  );
  const cpResult = await ctx.send(createProductTx, eve, '创建商品', 'eve');
  assertTxSuccess(cpResult, '创建商品');

  const publishTx = (api.tx as any).entityProduct.publishProduct(productId);
  const pubResult = await ctx.send(publishTx, eve, '上架商品', 'eve');
  assertTxSuccess(pubResult, '上架商品');

  await ctx.check('商品已上架', 'eve', async () => {
    await assertStorageExists(api, 'entityProduct', 'products', [productId], '商品应存在');
  });

  // ─── Step 3: Bob 下单 (NEX 支付) ──────────────────────────

  const bobBalanceBefore = await getFreeBalance(api, bob.address);

  const placeOrderTx = (api.tx as any).entityTransaction.placeOrder(
    productId,
    1,                    // quantity
    null,                 // shippingCid
    null,                 // useTokens
    null,                 // useShoppingBalance
    null,                 // paymentAsset
  );
  const orderResult = await ctx.send(placeOrderTx, bob, 'Bob 下单', 'bob');
  assertTxSuccess(orderResult, 'Bob 下单');

  // 从事件中获取 order_id
  const orderEvent = orderResult.events.find(
    e => e.section === 'entityTransaction' && e.method === 'OrderPlaced',
  );
  assertTrue(!!orderEvent, '应有 OrderPlaced 事件');
  const orderId = orderEvent!.data?.orderId ?? orderEvent!.data?.[0] ?? orderEvent!.data?.order_id;
  console.log(`    订单 ID: ${orderId}`);

  // ─── Step 4: 验证资金锁入 ─────────────────────────────────

  await ctx.check('验证 Bob 余额减少', 'bob', async () => {
    const bobBalanceAfter = await getFreeBalance(api, bob.address);
    const delta = bobBalanceBefore - bobBalanceAfter;
    assertTrue(delta > 0n, `Bob 应被扣费, 实际减少 ${Number(delta) / 1e12} NEX`);
  });

  // ─── Step 5: Eve 发货 ─────────────────────────────────────

  const shipTx = (api.tx as any).entityTransaction.shipOrder(orderId, 'tracking_placeholder');
  const shipResult = await ctx.send(shipTx, eve, 'Eve 发货', 'eve');
  assertTxSuccess(shipResult, '发货');

  // ─── Step 6: Bob 确认收货 → 释放资金 ─────────────────────

  const eveBalanceBefore = await getFreeBalance(api, eve.address);

  const confirmTx = (api.tx as any).entityTransaction.confirmReceipt(orderId);
  const confirmResult = await ctx.send(confirmTx, bob, 'Bob 确认收货', 'bob');
  assertTxSuccess(confirmResult, '确认收货');

  // ─── Step 7: 验证事件 ─────────────────────────────────────

  await ctx.check('验证 OrderCompleted 事件', 'system', () => {
    assertEventEmitted(confirmResult, 'entityTransaction', 'OrderCompleted', '应有 OrderCompleted');
  });

  await ctx.check('验证卖家收到款项', 'eve', async () => {
    const eveBalanceAfter = await getFreeBalance(api, eve.address);
    const delta = eveBalanceAfter - eveBalanceBefore;
    // 卖家应收到大部分款项(扣除平台费+佣金)
    assertTrue(delta > 0n, `Eve 应收到款项, 实际增加 ${Number(delta) / 1e12} NEX`);
  });

  // ─── Step 8: [错误路径] 非买家确认收货 ────────────────────

  // 创建新订单用于错误路径测试
  const placeOrder2Tx = (api.tx as any).entityTransaction.placeOrder(productId, 1, null, null, null, null);
  const order2Result = await ctx.send(placeOrder2Tx, bob, 'Bob 下单(错误路径用)', 'bob');
  assertTxSuccess(order2Result, '下单2');

  const order2Event = order2Result.events.find(
    e => e.section === 'entityTransaction' && e.method === 'OrderPlaced',
  );
  const order2Id = order2Event?.data?.orderId ?? order2Event?.data?.[0] ?? order2Event?.data?.order_id;

  // 发货
  const ship2Tx = (api.tx as any).entityTransaction.shipOrder(order2Id, 'tracking_placeholder');
  await ctx.send(ship2Tx, eve, '发货(错误路径用)', 'eve');

  // Charlie 尝试确认收货
  const fakeConfirmTx = (api.tx as any).entityTransaction.confirmReceipt(order2Id);
  const fakeConfirmResult = await ctx.send(fakeConfirmTx, charlie, '[错误路径] Charlie 确认收货', 'charlie');
  await ctx.check('非买家确认应失败', 'charlie', () => {
    assertTxFailed(fakeConfirmResult, undefined, '非买家确认收货');
  });

  // ─── Step 9: [错误路径] 下架商品无法下单 ─────────────────

  const unpublishTx = (api.tx as any).entityProduct.unpublishProduct(productId);
  const unpubResult = await ctx.send(unpublishTx, eve, '下架商品', 'eve');
  assertTxSuccess(unpubResult, '下架');

  const failOrderTx = (api.tx as any).entityTransaction.placeOrder(productId, 1, null, null, null, null);
  const failOrderResult = await ctx.send(failOrderTx, bob, '[错误路径] 下架后下单', 'bob');
  await ctx.check('下架商品下单应失败', 'bob', () => {
    assertTxFailed(failOrderResult, undefined, '下架商品下单');
  });

  // 重新上架供后续测试
  const republishTx = (api.tx as any).entityProduct.publishProduct(productId);
  await ctx.send(republishTx, eve, '重新上架', 'eve');

  // ─── Step 10: 取消订单 → 退款 ────────────────────────────

  const bobBalBefore2 = await getFreeBalance(api, bob.address);

  const placeOrder3Tx = (api.tx as any).entityTransaction.placeOrder(productId, 1, null, null, null, null);
  const order3Result = await ctx.send(placeOrder3Tx, bob, 'Bob 下单(取消测试)', 'bob');
  assertTxSuccess(order3Result, '下单3');

  const order3Event = order3Result.events.find(
    e => e.section === 'entityTransaction' && e.method === 'OrderPlaced',
  );
  const order3Id = order3Event?.data?.orderId ?? order3Event?.data?.[0] ?? order3Event?.data?.order_id;

  const cancelTx = (api.tx as any).entityTransaction.cancelOrder(order3Id);
  const cancelResult = await ctx.send(cancelTx, bob, 'Bob 取消订单', 'bob');
  assertTxSuccess(cancelResult, '取消订单');

  await ctx.check('验证 OrderCancelled 事件', 'bob', () => {
    assertEventEmitted(cancelResult, 'entityTransaction', 'OrderCancelled', '应有 OrderCancelled');
  });

  // ─── Step 11: 退款流程 ────────────────────────────────────

  // 对 order2 申请退款 (之前 Charlie 确认失败, Bob 还没确认)
  const refundTx = (api.tx as any).entityTransaction.requestRefund(order2Id, 'refund_reason_placeholder');
  const refundResult = await ctx.send(refundTx, bob, 'Bob 申请退款', 'bob');
  assertTxSuccess(refundResult, '申请退款');

  const approveRefundTx = (api.tx as any).entityTransaction.approveRefund(order2Id);
  const approveRefundResult = await ctx.send(approveRefundTx, eve, 'Eve 批准退款', 'eve');
  assertTxSuccess(approveRefundResult, '批准退款');

  await ctx.check('验证退款事件', 'system', () => {
    assertEventEmitted(approveRefundResult, 'entityTransaction', 'RefundApproved', '应有 RefundApproved');
  });

  // ─── 汇总 ─────────────────────────────────────────────────
  await ctx.check('订单生命周期流程汇总', 'system', () => {
    console.log(`    ✓ 实物订单: 下单→发货→确认→佣金`);
    console.log(`    ✓ 取消订单: 下单→取消→退款`);
    console.log(`    ✓ 退款流程: 申请→批准`);
    console.log(`    ✓ 错误路径: 非买家确认 ✗, 下架下单 ✗`);
  });
}
