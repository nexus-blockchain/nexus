/**
 * Minimal repro for the E10 service confirmation path.
 *
 * This bypasses the normal E2E event-fetch path and watches the extrinsic
 * status directly so we can tell whether `confirmService` itself finalizes.
 */

import { getApi, disconnectApi, getFreeBalance } from '../core/chain-state.js';
import { getDevAccounts } from '../fixtures/accounts.js';
import { nex } from '../core/config.js';

async function main(): Promise<void> {
  const api = await getApi();
  const actors = getDevAccounts();
  const eve = actors.eve;
  const bob = actors.bob;

  console.log('[debug] connected');
  console.log(`[debug] eve=${eve.address}`);
  console.log(`[debug] bob=${bob.address}`);
  console.log(`[debug] eve_free_balance=${await getFreeBalance(api, eve.address)}`);
  console.log(`[debug] bob_free_balance=${await getFreeBalance(api, bob.address)}`);

  const { entityId, shopId } = await ensureEntityAndShop(api, eve.address);
  console.log(`[debug] entityId=${entityId} shopId=${shopId}`);

  const productId = (await (api.query as any).entityProduct.nextProductId()).toNumber();
  console.log(`[debug] next service productId=${productId}`);

  const createTx = (api.tx as any).entityProduct.createProduct(
    shopId,
    `E10-debug-service-name-${productId}`,
    `E10-debug-service-images-${productId}`,
    `E10-debug-service-detail-${productId}`,
    nex(5).toString(),
    0,
    100,
    'Service',
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

  const orderId = (await (api.query as any).entityTransaction.nextOrderId()).toNumber();
  console.log(`[debug] next service orderId=${orderId}`);

  const placeOrderTx = (api.tx as any).entityTransaction.placeOrder(
    productId,
    1,
    null,
    null,
    null,
    null,
    null,
    null,
  );
  console.log(`[debug] place_order_result=${JSON.stringify(await watchTx(placeOrderTx, bob, `placeOrder(${orderId})`))}`);
  await logOrder(api, orderId, 'after_place_order');

  const startServiceTx = (api.tx as any).entityTransaction.startService(orderId);
  console.log(`[debug] start_service_result=${JSON.stringify(await watchTx(startServiceTx, eve, `startService(${orderId})`))}`);
  await logOrder(api, orderId, 'after_start_service');

  const completeServiceTx = (api.tx as any).entityTransaction.completeService(orderId);
  console.log(`[debug] complete_service_result=${JSON.stringify(await watchTx(completeServiceTx, eve, `completeService(${orderId})`))}`);
  await logOrder(api, orderId, 'after_complete_service');

  const confirmServiceTx = (api.tx as any).entityTransaction.confirmService(orderId);
  console.log(`[debug] confirm_service_result=${JSON.stringify(await watchTx(confirmServiceTx, bob, `confirmService(${orderId})`))}`);
  await logOrder(api, orderId, 'after_confirm_service');

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
