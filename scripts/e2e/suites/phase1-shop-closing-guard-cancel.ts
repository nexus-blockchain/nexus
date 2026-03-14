import type { ApiPromise } from '@polkadot/api';
import type { TxReceipt } from '../framework/api.js';
import { submitTx } from '../framework/api.js';
import { assert, assertEvent, assertTxSuccess } from '../framework/assert.js';
import { codecToJson } from '../framework/codec.js';
import { TestSuite } from '../framework/types.js';
import { nex } from '../framework/units.js';
import {
  asOptionalNumber,
  bytes,
  createAndPublishProduct,
  decodeStatus,
  readNextOrderId,
  readNextProductId,
  readOrder,
  readShop,
  setupFreshEntity,
  setupMembers,
} from './helpers.js';

const PRODUCT_PRICE = nex(10);
const SECONDARY_SHOP_INITIAL_FUND = nex(500);
const CLOSING_TOP_UP = nex(50);

function uniqueSuffix(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function assertTxFailureIncludes(receipt: TxReceipt, keyword: string, message: string): void {
  assert(!receipt.success, `${message}: expected tx failure`);
  assert(
    (receipt.error ?? '').includes(keyword),
    `${message}: expected error to include "${keyword}", got "${receipt.error ?? ''}"`,
  );
}

async function readShopClosingAt(api: ApiPromise, shopId: number): Promise<number | undefined> {
  const value = await (api.query as any).entityShop.shopClosingAt(shopId);
  if ((value as any)?.isSome) {
    return Number((value as any).unwrap().toString());
  }
  return asOptionalNumber(codecToJson(value));
}

function buildCreateProductTx(api: ApiPromise, shopId: number) {
  const suffix = uniqueSuffix();
  return (api.tx as any).entityProduct.createProduct(
    shopId,
    bytes(`closing-prod-${suffix}`),
    bytes(`closing-img-${suffix}`),
    bytes(`closing-detail-${suffix}`),
    PRODUCT_PRICE.toString(),
    0,
    0,
    'Digital',
    0,
    bytes(''),
    bytes(''),
    0,
    0,
    'Public',
  );
}

export const phase1ShopClosingGuardCancelSuite: TestSuite = {
  id: 'phase1-shop-closing-guard-cancel',
  title: 'Phase 1 / S1-06a shop closing guards + cancel substitute',
  description: 'Substitute for blocked S1-06: verify closeShop enters Closing, blocks new business ops, still allows fundOperating, and cancelCloseShop restores activity.',
  tags: ['phase1', 'shop', 'closing', 'substitute', 'entity'],
  async run(ctx) {
    const seller = ctx.actors.ferdie;
    const buyer = ctx.actors.bob;
    const tx = ctx.api.tx as any;
    const query = ctx.api.query as any;

    await ctx.step('fund seller and buyer accounts', async () => {
      await ctx.ensureFundsFor(['ferdie', 'bob'], 25_000);
    });

    const setup = await ctx.step('create a fresh entity with a closeable secondary shop, member buyer, and published product', async () => {
      const { entityId, shopId: primaryShopId } = await setupFreshEntity(ctx.api, seller, nex(2_500));
      const closeableShopId = Number((await query.entityShop.nextShopId()).toString());

      const createShopReceipt = await submitTx(
        ctx.api,
        tx.entityShop.createShop(
          entityId,
          bytes(`closing-shop-${uniqueSuffix()}`),
          'OnlineStore',
          SECONDARY_SHOP_INITIAL_FUND.toString(),
        ),
        seller,
        'create closeable secondary shop',
      );
      assertTxSuccess(createShopReceipt, 'createShop should succeed for the closeable secondary shop');

      await setupMembers(ctx.api, seller, closeableShopId, entityId, [buyer]);
      const productId = await createAndPublishProduct(ctx.api, seller, closeableShopId, {
        price: PRODUCT_PRICE,
        category: 'Digital',
      });

      ctx.note(`entityId=${entityId} primaryShopId=${primaryShopId} closeableShopId=${closeableShopId} productId=${productId}`);
      return { entityId, primaryShopId, shopId: closeableShopId, productId };
    });

    await ctx.step('owner closes the secondary shop and it enters the Closing grace state', async () => {
      const gracePeriod = Number((ctx.api.consts as any).entityShop.shopClosingGracePeriod.toString());
      const receipt = await submitTx(
        ctx.api,
        tx.entityShop.closeShop(setup.shopId),
        seller,
        'close secondary shop',
      );
      assertTxSuccess(receipt, 'closeShop should succeed for a non-primary shop without active orders');
      assertEvent(receipt, 'entityShop', 'ShopClosing', 'closeShop should emit ShopClosing');

      const shop = await readShop(ctx.api, setup.shopId);
      const closingAt = await readShopClosingAt(ctx.api, setup.shopId);
      const status = decodeStatus(shop, 'status').toLowerCase();

      assert(status.includes('closing'), `shop should enter Closing state, got ${status}`);
      assert(closingAt != null, 'shopClosingAt should be populated after closeShop');
      ctx.note(`closingAt=${closingAt} gracePeriodBlocks=${gracePeriod}`);
    });

    await ctx.step('closing-state guards block new business ops but still allow owner top-ups', async () => {
      const beforeOrderId = await readNextOrderId(ctx.api);
      const placeOrderReceipt = await submitTx(
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
        'place order while shop is closing',
      );
      assertTxFailureIncludes(placeOrderReceipt, 'ShopInactive', 'closing shop should reject new orders');
      const afterOrderId = await readNextOrderId(ctx.api);
      assert(afterOrderId === beforeOrderId, `failed order should not advance nextOrderId (${beforeOrderId} -> ${afterOrderId})`);

      const beforeProductId = await readNextProductId(ctx.api);
      const createProductReceipt = await submitTx(
        ctx.api,
        buildCreateProductTx(ctx.api, setup.shopId),
        seller,
        'create product while shop is closing',
      );
      assertTxFailureIncludes(createProductReceipt, 'ShopNotActive', 'closing shop should reject new product creation');
      const afterProductId = await readNextProductId(ctx.api);
      assert(afterProductId === beforeProductId, `failed product creation should not advance nextProductId (${beforeProductId} -> ${afterProductId})`);

      const withdrawReceipt = await submitTx(
        ctx.api,
        tx.entityShop.withdrawOperatingFund(setup.shopId, nex(1).toString()),
        seller,
        'withdraw operating fund while shop is closing',
      );
      assertTxFailureIncludes(withdrawReceipt, 'ShopAlreadyClosing', 'withdrawOperatingFund should reject a Closing shop');

      const topUpReceipt = await submitTx(
        ctx.api,
        tx.entityShop.fundOperating(setup.shopId, CLOSING_TOP_UP.toString()),
        seller,
        'top up operating fund while shop is closing',
      );
      assertTxSuccess(topUpReceipt, 'fundOperating should still succeed during the Closing grace period');
      assertEvent(topUpReceipt, 'entityShop', 'OperatingFundDeposited', 'fundOperating during Closing should emit OperatingFundDeposited');

      const shopAfterTopUp = await readShop(ctx.api, setup.shopId);
      const statusAfterTopUp = decodeStatus(shopAfterTopUp, 'status').toLowerCase();
      assert(statusAfterTopUp.includes('closing'), `shop should remain Closing after top-up, got ${statusAfterTopUp}`);
    });

    await ctx.step('owner cancels the close and business operations resume', async () => {
      const cancelReceipt = await submitTx(
        ctx.api,
        tx.entityShop.cancelCloseShop(setup.shopId),
        seller,
        'cancel shop close',
      );
      assertTxSuccess(cancelReceipt, 'cancelCloseShop should succeed for the entity owner');
      assertEvent(cancelReceipt, 'entityShop', 'ShopClosingCancelled', 'cancelCloseShop should emit ShopClosingCancelled');

      const closingAtAfterCancel = await readShopClosingAt(ctx.api, setup.shopId);
      const shopAfterCancel = await readShop(ctx.api, setup.shopId);
      const statusAfterCancel = decodeStatus(shopAfterCancel, 'status').toLowerCase();

      assert(closingAtAfterCancel == null, 'shopClosingAt should be cleared after cancelCloseShop');
      assert(statusAfterCancel.includes('active'), `shop should return to Active after cancelCloseShop, got ${statusAfterCancel}`);

      const nextOrderId = await readNextOrderId(ctx.api);
      const resumedOrderReceipt = await submitTx(
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
        'place order after canceling shop close',
      );
      assertTxSuccess(resumedOrderReceipt, 'placeOrder should succeed again after cancelCloseShop restores activity');
      assertEvent(resumedOrderReceipt, 'entityTransaction', 'OrderCompleted', 'digital order should auto-complete once the shop is Active again');

      const order = await readOrder(ctx.api, nextOrderId);
      const orderStatus = decodeStatus(order, 'status').toLowerCase();
      assert(orderStatus.includes('completed'), `resumed order should complete successfully, got ${orderStatus}`);
    });
  },
};
