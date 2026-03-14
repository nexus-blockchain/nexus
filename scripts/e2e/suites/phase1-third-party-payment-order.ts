import { readFreeBalance } from '../framework/accounts.js';
import { submitTx } from '../framework/api.js';
import { assert, assertEvent, assertEqual, assertTxSuccess } from '../framework/assert.js';
import { codecToJson, readObjectField } from '../framework/codec.js';
import { TestSuite } from '../framework/types.js';
import { nex } from '../framework/units.js';
import {
  asOptionalNumber,
  bytes,
  createAndPublishProduct,
  decodeStatus,
  readMember,
  readNextOrderId,
  readOrder,
  setupFreshEntity,
  setupMembers,
} from './helpers.js';

async function readPayerOrders(api: any, address: string): Promise<number[]> {
  const value = await api.query.entityTransaction.payerOrders(address);
  const json = codecToJson<unknown[]>(value);
  return Array.isArray(json) ? json.map((item) => Number(item)) : [];
}

const PHYSICAL_PRICE = nex(20);

export const phase1ThirdPartyPaymentOrderSuite: TestSuite = {
  id: 'phase1-third-party-payment-order',
  title: 'Phase 1 / S1-02 third-party payment order',
  description: 'Verify placeOrderFor stores payer metadata, charges the payer, updates the buyer on completion, and allows cleanupPayerOrders.',
  tags: ['phase1', 'order', 'payer', 'entity'],
  async run(ctx) {
    const seller = ctx.actors.ferdie;
    const buyer = ctx.actors.charlie;
    const payer = ctx.actors.dave;
    const tx = ctx.api.tx as any;

    await ctx.step('fund seller, buyer, and payer accounts', async () => {
      await ctx.ensureFundsFor(['ferdie', 'charlie', 'dave'], 25_000);
    });

    const setup = await ctx.step('create a fresh entity, activate buyer membership, and publish a physical product', async () => {
      const { entityId, shopId } = await setupFreshEntity(ctx.api, seller, nex(2_500));
      await setupMembers(ctx.api, seller, shopId, entityId, [buyer]);
      const productId = await createAndPublishProduct(ctx.api, seller, shopId, {
        price: PHYSICAL_PRICE,
        category: 'Physical',
        stock: 10,
      });
      const memberBefore = await readMember(ctx.api, entityId, buyer.address);
      ctx.note(`entityId=${entityId} shopId=${shopId} productId=${productId}`);
      return { entityId, shopId, productId, memberBefore };
    });

    const placed = await ctx.step('payer places a third-party order for the buyer', async () => {
      const nextOrderId = await readNextOrderId(ctx.api);
      const payerBalanceBefore = await readFreeBalance(ctx.api, payer.address);
      const buyerBalanceBefore = await readFreeBalance(ctx.api, buyer.address);

      const receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.placeOrderFor(
          buyer.address,
          setup.productId,
          1,
          bytes(`ship-${Date.now()}`),
          null,
          null,
          null,
          null,
          null,
        ),
        payer,
        'place order for buyer',
      );
      assertTxSuccess(receipt, 'placeOrderFor should succeed');

      const order = await readOrder(ctx.api, nextOrderId);
      const payerBalanceAfter = await readFreeBalance(ctx.api, payer.address);
      const buyerBalanceAfter = await readFreeBalance(ctx.api, buyer.address);
      const payerOrders = await readPayerOrders(ctx.api, payer.address);
      const status = decodeStatus(order, 'status').toLowerCase();

      assert(status.includes('paid'), `third-party order should start in Paid state, got ${status}`);
      assertEqual(String(readObjectField(order.json, 'buyer')), buyer.address, 'order buyer should match requested buyer');
      assertEqual(String(readObjectField(order.json, 'payer')), payer.address, 'order payer should match payer signer');
      assert(payerBalanceAfter < payerBalanceBefore, 'payer free balance should decrease after funding the order');
      assert(buyerBalanceAfter === buyerBalanceBefore, 'buyer free balance should not change when a third party pays');
      assert(payerOrders.includes(nextOrderId), `payer order index should contain order ${nextOrderId}`);

      return { orderId: nextOrderId };
    });

    await ctx.step('seller ships the order and buyer confirms receipt', async () => {
      const shipReceipt = await submitTx(
        ctx.api,
        tx.entityTransaction.shipOrder(placed.orderId, bytes(`track-${Date.now()}`)),
        seller,
        'ship third-party order',
      );
      assertTxSuccess(shipReceipt, 'shipOrder should succeed for third-party order');
      assertEvent(shipReceipt, 'entityTransaction', 'OrderShipped', 'shipOrder should emit OrderShipped');

      const confirmReceipt = await submitTx(
        ctx.api,
        tx.entityTransaction.confirmReceipt(placed.orderId),
        buyer,
        'confirm third-party order receipt',
      );
      assertTxSuccess(confirmReceipt, 'confirmReceipt should succeed for third-party order');
      assertEvent(confirmReceipt, 'entityTransaction', 'OrderCompleted', 'confirmReceipt should emit OrderCompleted');

      const order = await readOrder(ctx.api, placed.orderId);
      const memberAfter = await readMember(ctx.api, setup.entityId, buyer.address);
      const status = decodeStatus(order, 'status').toLowerCase();
      const spentBefore = asOptionalNumber(readObjectField(setup.memberBefore.json, 'totalSpent', 'total_spent')) ?? 0;
      const spentAfter = asOptionalNumber(readObjectField(memberAfter.json, 'totalSpent', 'total_spent')) ?? 0;

      assert(status.includes('completed'), `third-party order should complete after buyer confirmation, got ${status}`);
      assert(spentAfter > spentBefore, `buyer total_spent should increase after completion (${spentBefore} -> ${spentAfter})`);
    });

    await ctx.step('payer cleans up the completed payer-order index', async () => {
      const beforeCleanup = await readPayerOrders(ctx.api, payer.address);
      assert(beforeCleanup.includes(placed.orderId), `payer order index should still contain ${placed.orderId} before cleanup`);

      const receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.cleanupPayerOrders(),
        payer,
        'cleanup payer orders',
      );
      assertTxSuccess(receipt, 'cleanupPayerOrders should succeed');
      assertEvent(receipt, 'entityTransaction', 'PayerOrdersCleaned', 'cleanupPayerOrders should emit PayerOrdersCleaned');

      const afterCleanup = await readPayerOrders(ctx.api, payer.address);
      assert(!afterCleanup.includes(placed.orderId), `payer order index should remove completed order ${placed.orderId}`);
      ctx.note(`orderId=${placed.orderId} payerOrdersBefore=${beforeCleanup.length} payerOrdersAfter=${afterCleanup.length}`);
    });
  },
};
