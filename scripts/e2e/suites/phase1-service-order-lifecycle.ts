import { submitTx } from '../framework/api.js';
import { assert, assertEvent, assertTxSuccess } from '../framework/assert.js';
import { readObjectField } from '../framework/codec.js';
import { TestSuite } from '../framework/types.js';
import { nex } from '../framework/units.js';
import {
  asBigInt,
  asOptionalNumber,
  createAndPublishProduct,
  decodeStatus,
  readMember,
  readNextOrderId,
  readOrder,
  readShop,
  setupFreshEntity,
  setupMembers,
} from './helpers.js';

const SERVICE_PRICE = nex(10);

export const phase1ServiceOrderLifecycleSuite: TestSuite = {
  id: 'phase1-service-order-lifecycle',
  title: 'Phase 1 / S1-01 service order lifecycle',
  description: 'Create a service product and verify placeOrder → startService → completeService → confirmService updates order, member, and shop state.',
  tags: ['phase1', 'order', 'service', 'entity'],
  async run(ctx) {
    const seller = ctx.actors.ferdie;
    const buyer = ctx.actors.bob;
    const tx = ctx.api.tx as any;

    await ctx.step('fund seller and buyer accounts', async () => {
      await ctx.ensureFundsFor(['ferdie', 'bob'], 25_000);
    });

    const setup = await ctx.step('create a fresh entity, activate buyer membership, and publish a service product', async () => {
      const { entityId, shopId } = await setupFreshEntity(ctx.api, seller, nex(2_500));
      await setupMembers(ctx.api, seller, shopId, entityId, [buyer]);
      const productId = await createAndPublishProduct(ctx.api, seller, shopId, {
        price: SERVICE_PRICE,
        category: 'Service',
      });

      const memberBefore = await readMember(ctx.api, entityId, buyer.address);
      const shopBefore = await readShop(ctx.api, shopId);
      ctx.note(`entityId=${entityId} shopId=${shopId} productId=${productId}`);
      return { entityId, shopId, productId, memberBefore, shopBefore };
    });

    const orderId = await ctx.step('buyer places the service order', async () => {
      const nextOrderId = await readNextOrderId(ctx.api);
      const receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.placeOrder(
          setup.productId,
          1,
          null,
          null,
          null,
          null,
          null,
          null,
        ),
        buyer,
        'place service order',
      );
      assertTxSuccess(receipt, 'place service order should succeed');

      const order = await readOrder(ctx.api, nextOrderId);
      const status = decodeStatus(order, 'status').toLowerCase();
      assert(status.includes('paid'), `service order should start in Paid state, got ${status}`);
      return nextOrderId;
    });

    await ctx.step('seller starts the service', async () => {
      const receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.startService(orderId),
        seller,
        'start service',
      );
      assertTxSuccess(receipt, 'start service should succeed');
      assertEvent(receipt, 'entityTransaction', 'ServiceStarted', 'start service should emit ServiceStarted');

      const order = await readOrder(ctx.api, orderId);
      const status = decodeStatus(order, 'status').toLowerCase();
      assert(status.includes('shipped'), `started service order should move to Shipped-like state, got ${status}`);
      assert(readObjectField(order.json, 'serviceStartedAt', 'service_started_at') != null, 'service_started_at should be populated');
    });

    await ctx.step('seller marks the service as completed', async () => {
      const receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.completeService(orderId),
        seller,
        'complete service',
      );
      assertTxSuccess(receipt, 'complete service should succeed');
      assertEvent(receipt, 'entityTransaction', 'ServiceCompleted', 'complete service should emit ServiceCompleted');

      const order = await readOrder(ctx.api, orderId);
      const status = decodeStatus(order, 'status').toLowerCase();
      assert(status.includes('shipped'), `completed service order should remain confirmable, got ${status}`);
      assert(readObjectField(order.json, 'serviceCompletedAt', 'service_completed_at') != null, 'service_completed_at should be populated');
    });

    await ctx.step('buyer confirms service completion and hooks update storage', async () => {
      const receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.confirmService(orderId),
        buyer,
        'confirm service',
      );
      assertTxSuccess(receipt, 'confirm service should succeed');
      assertEvent(receipt, 'entityTransaction', 'OrderCompleted', 'confirm service should emit OrderCompleted');

      const order = await readOrder(ctx.api, orderId);
      const memberAfter = await readMember(ctx.api, setup.entityId, buyer.address);
      const shopAfter = await readShop(ctx.api, setup.shopId);

      const orderStatus = decodeStatus(order, 'status').toLowerCase();
      assert(orderStatus.includes('completed'), `service order should be completed, got ${orderStatus}`);
      assert(readObjectField(order.json, 'completedAt', 'completed_at') != null, 'completed_at should be populated');

      const spentBefore = asOptionalNumber(readObjectField(setup.memberBefore.json, 'totalSpent', 'total_spent')) ?? 0;
      const spentAfter = asOptionalNumber(readObjectField(memberAfter.json, 'totalSpent', 'total_spent')) ?? 0;
      assert(spentAfter > spentBefore, `member total_spent should increase after service completion (${spentBefore} -> ${spentAfter})`);

      const totalOrdersBefore = asOptionalNumber(readObjectField(setup.shopBefore.json, 'totalOrders', 'total_orders')) ?? 0;
      const totalOrdersAfter = asOptionalNumber(readObjectField(shopAfter.json, 'totalOrders', 'total_orders')) ?? 0;
      assert(totalOrdersAfter > totalOrdersBefore, `shop total_orders should increase (${totalOrdersBefore} -> ${totalOrdersAfter})`);

      const salesBefore = asBigInt(readObjectField(setup.shopBefore.json, 'totalSales', 'total_sales') ?? 0);
      const salesAfter = asBigInt(readObjectField(shopAfter.json, 'totalSales', 'total_sales') ?? 0);
      assert(salesAfter > salesBefore, `shop total_sales should increase (${salesBefore} -> ${salesAfter})`);

      ctx.note(`orderId=${orderId} memberSpent=${spentAfter} shopTotalOrders=${totalOrdersAfter}`);
    });
  },
};
