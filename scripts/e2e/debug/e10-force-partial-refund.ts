/**
 * Minimal repro for the E10 forcePartialRefund path.
 *
 * This bypasses the normal E2E event-fetch path and watches extrinsic
 * statuses directly. It reproduces:
 * 1) physical product create/publish
 * 2) physical order place
 * 3) sudo forcePartialRefund on the paid order
 */

import { getApi, disconnectApi, getFreeBalance } from '../core/chain-state.js';
import { getDevAccounts } from '../fixtures/accounts.js';
import { nex } from '../core/config.js';

async function main(): Promise<void> {
  const api = await getApi();
  const actors = getDevAccounts();
  const alice = actors.alice;
  const eve = actors.eve;
  const bob = actors.bob;

  console.log('[debug] connected');
  console.log(`[debug] alice=${alice.address}`);
  console.log(`[debug] eve=${eve.address}`);
  console.log(`[debug] bob=${bob.address}`);
  console.log(`[debug] eve_free_balance=${await getFreeBalance(api, eve.address)}`);
  console.log(`[debug] bob_free_balance=${await getFreeBalance(api, bob.address)}`);

  const { entityId, shopId } = await ensureEntityAndShop(api, eve.address);
  console.log(`[debug] entityId=${entityId} shopId=${shopId}`);

  const productId = (await (api.query as any).entityProduct.nextProductId()).toNumber();
  console.log(`[debug] next physical productId=${productId}`);

  const createTx = (api.tx as any).entityProduct.createProduct(
    shopId,
    `E10-debug-partial-refund-name-${productId}`,
    `E10-debug-partial-refund-images-${productId}`,
    `E10-debug-partial-refund-detail-${productId}`,
    nex(7).toString(),
    0,
    100,
    'Physical',
    0,
    '',
    '',
    1,
    0,
    'Public',
  );
  console.log(`[debug] create_result=${JSON.stringify(await watchTx(createTx, eve, `createProduct(${productId})`))}`);

  const publishTx = (api.tx as any).entityProduct.publishProduct(productId);
  console.log(`[debug] publish_result=${JSON.stringify(await watchTx(publishTx, eve, `publishProduct(${productId})`))}`);
  await logProduct(api, productId, 'after_publish');

  const orderId = (await (api.query as any).entityTransaction.nextOrderId()).toNumber();
  console.log(`[debug] order_id=${orderId}`);

  const placeOrderTx = (api.tx as any).entityTransaction.placeOrder(
    productId,
    1,
    `e10-debug-force-refund-${productId}`,
    null,
    null,
    null,
    null,
    null,
  );
  console.log(`[debug] place_order_result=${JSON.stringify(await watchTx(placeOrderTx, bob, `placeOrder(${orderId})`))}`);
  await logOrder(api, orderId, 'after_place_order');
  await logProduct(api, productId, 'after_place_order');

  const bobBefore = await getFreeBalance(api, bob.address);
  const forcePartialRefundTx = api.tx.sudo.sudo(
    (api.tx as any).entityTransaction.forcePartialRefund(orderId, 5000, null),
  );
  console.log(`[debug] force_partial_refund_result=${JSON.stringify(await watchTx(forcePartialRefundTx, alice, `sudo(forcePartialRefund(${orderId}))`))}`);
  const bobAfter = await getFreeBalance(api, bob.address);
  console.log(`[debug] bob_delta=${(bobAfter - bobBefore).toString()}`);
  await logOrder(api, orderId, 'after_force_partial_refund');
  await logProduct(api, productId, 'after_force_partial_refund');

  await disconnectApi();
}

async function ensureEntityAndShop(api: any, ownerAddress: string): Promise<{ entityId: number; shopId: number }> {
  const userEntities = await (api.query as any).entityRegistry.userEntity(ownerAddress);
  const entityIds = userEntities.toHuman() as string[];

  if (entityIds && entityIds.length > 0) {
    for (const rawEntityId of entityIds) {
      const entityId = parseInt(rawEntityId.replace(/,/g, ''), 10);
      const shopIdsRaw = await (api.query as any).entityRegistry.entityShops(entityId);
      const shopIds = shopIdsRaw.toHuman() as string[];
      if (shopIds && shopIds.length > 0) {
        return {
          entityId,
          shopId: parseInt(shopIds[0].replace(/,/g, ''), 10),
        };
      }
    }
  }

  throw new Error('No existing entity/shop found for Eve');
}

async function logOrder(api: any, orderId: number, label: string): Promise<void> {
  const orderOpt = await (api.query as any).entityTransaction.orders(orderId);
  if (orderOpt.isNone) {
    console.log(`[debug] ${label}: order ${orderId} missing`);
    return;
  }
  console.log(`[debug] ${label}: order=${JSON.stringify(orderOpt.toHuman())}`);
}

async function logProduct(api: any, productId: number, label: string): Promise<void> {
  const productOpt = await (api.query as any).entityProduct.products(productId);
  if (productOpt.isNone) {
    console.log(`[debug] ${label}: product ${productId} missing`);
    return;
  }
  console.log(`[debug] ${label}: product=${JSON.stringify(productOpt.toHuman())}`);
}

async function watchTx(tx: any, signer: any, label: string): Promise<Record<string, unknown>> {
  return new Promise((resolve) => {
    let unsub: (() => void) | undefined;
    const timeout = setTimeout(() => {
      if (unsub) unsub();
      resolve({ ok: false, stage: 'timeout', label });
    }, 90_000);

    tx.signAndSend(signer, (result: any) => {
      const status = result.status;
      console.log(`[debug] ${label} status=${status.type}`);

      if (result.dispatchError) {
        clearTimeout(timeout);
        if (unsub) unsub();
        resolve({
          ok: false,
          stage: 'dispatch_error',
          label,
          error: formatDispatchError(result.dispatchError),
        });
        return;
      }

      if (status.isInBlock) {
        console.log(`[debug] ${label} inBlock=${status.asInBlock.toHex()}`);
      }

      if (status.isFinalized) {
        clearTimeout(timeout);
        if (unsub) unsub();
        resolve({
          ok: true,
          stage: 'finalized',
          label,
          blockHash: status.asFinalized.toHex(),
          eventCount: Array.isArray(result.events) ? result.events.length : 0,
        });
      }
    }).then((u: () => void) => {
      unsub = u;
    }).catch((error: Error) => {
      clearTimeout(timeout);
      if (unsub) unsub();
      resolve({ ok: false, stage: 'submit_error', label, error: error.message });
    });
  });
}

function formatDispatchError(dispatchError: any): string {
  if (dispatchError.isModule) {
    return dispatchError.asModule.toString();
  }
  return dispatchError.toString();
}

main().catch(async (error) => {
  console.error('[debug] fatal', error);
  await disconnectApi();
  process.exit(1);
});
