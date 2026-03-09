/**
 * Minimal repro for the E10 cleanupBuyerOrders path.
 *
 * This version avoids the long create/publish path:
 * - reuse existing on-sale service + physical products from Eve's shop
 * - use an isolated buyer account so BuyerOrders is easy to inspect
 * - keep only the shortest path to one terminal order + one active order
 */

import { getApi, disconnectApi, getFreeBalance } from '../core/chain-state.js';
import { createFlowAccounts, getDevAccounts } from '../fixtures/accounts.js';
import { nex } from '../core/config.js';

async function main(): Promise<void> {
  const api = await getApi();
  const shared = getDevAccounts();
  const actors = createFlowAccounts(`E10Cleanup${Date.now()}`, ['Bob']);
  const alice = actors.alice;
  const buyer = actors.bob;
  const eve = shared.eve;

  console.log('[debug] connected');
  console.log(`[debug] alice=${alice.address}`);
  console.log(`[debug] eve=${eve.address}`);
  console.log(`[debug] buyer=${buyer.address}`);

  console.log(`[debug] fund_buyer_result=${JSON.stringify(await watchTx(
    api.tx.balances.transferKeepAlive(buyer.address, nex(50_000).toString()),
    alice,
    `fundBuyer(${buyer.address})`,
  ))}`);
  console.log(`[debug] buyer_free_balance=${await getFreeBalance(api, buyer.address)}`);

  const { entityId, shopId } = await ensureEntityAndShop(api, eve.address);
  console.log(`[debug] entityId=${entityId} shopId=${shopId}`);

  const serviceProductId = await findReusableProduct(api, shopId, 'Service');
  const physicalProductId = await findReusableProduct(api, shopId, 'Physical');
  console.log(`[debug] service_product_id=${serviceProductId}`);
  console.log(`[debug] physical_product_id=${physicalProductId}`);

  const completedOrderId = (await (api.query as any).entityTransaction.nextOrderId()).toNumber();
  console.log(`[debug] completed_order_id=${completedOrderId}`);
  console.log(`[debug] place_service_order_result=${JSON.stringify(await watchTx(
    placeOrderTx(api, serviceProductId, null),
    buyer,
    `placeOrder(${completedOrderId})`,
  ))}`);
  console.log(`[debug] start_service_result=${JSON.stringify(await watchTx(
    (api.tx as any).entityTransaction.startService(completedOrderId),
    eve,
    `startService(${completedOrderId})`,
  ))}`);
  console.log(`[debug] complete_service_result=${JSON.stringify(await watchTx(
    (api.tx as any).entityTransaction.completeService(completedOrderId),
    eve,
    `completeService(${completedOrderId})`,
  ))}`);
  console.log(`[debug] confirm_service_result=${JSON.stringify(await watchTx(
    (api.tx as any).entityTransaction.confirmService(completedOrderId),
    buyer,
    `confirmService(${completedOrderId})`,
  ))}`);
  await logOrder(api, completedOrderId, 'after_completed_service_order');

  const paidOrderId = (await (api.query as any).entityTransaction.nextOrderId()).toNumber();
  console.log(`[debug] paid_order_id=${paidOrderId}`);
  console.log(`[debug] place_physical_order_result=${JSON.stringify(await watchTx(
    placeOrderTx(api, physicalProductId, `e10-debug-cleanup-${physicalProductId}`),
    buyer,
    `placeOrder(${paidOrderId})`,
  ))}`);
  await logOrder(api, paidOrderId, 'after_paid_physical_order');

  console.log(`[debug] buyer_orders_before_cleanup=${JSON.stringify((await (api.query as any).entityTransaction.buyerOrders(buyer.address)).toHuman())}`);
  const cleanupResult = await watchTx(
    (api.tx as any).entityTransaction.cleanupBuyerOrders(),
    buyer,
    'cleanupBuyerOrders()',
  );
  console.log(`[debug] cleanup_buyer_orders_result=${JSON.stringify(cleanupResult)}`);
  console.log(`[debug] buyer_orders_after_cleanup=${JSON.stringify((await (api.query as any).entityTransaction.buyerOrders(buyer.address)).toHuman())}`);
  await logOrder(api, completedOrderId, 'completed_order_after_cleanup');
  await logOrder(api, paidOrderId, 'paid_order_after_cleanup');

  await disconnectApi();
}

function placeOrderTx(api: any, productId: number, shippingCid: string | null): any {
  return (api.tx as any).entityTransaction.placeOrder(
    productId,
    1,
    shippingCid,
    null,
    null,
    null,
    null,
    null,
  );
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

async function findReusableProduct(
  api: any,
  shopId: number,
  category: 'Service' | 'Physical',
): Promise<number> {
  const nextProductId = (await (api.query as any).entityProduct.nextProductId()).toNumber();

  for (let productId = nextProductId - 1; productId >= 1; productId -= 1) {
    const productOpt = await (api.query as any).entityProduct.products(productId);
    if (productOpt.isNone) continue;

    const product = productOpt.toHuman() as Record<string, string>;
    if (parseHumanInt(product.shopId) !== shopId) continue;
    if (product.category !== category) continue;
    if (product.status !== 'OnSale') continue;
    if (category === 'Physical' && parseHumanInt(product.stock) <= 0) continue;

    return productId;
  }

  throw new Error(`No reusable on-sale ${category} product found for shop ${shopId}`);
}

function parseHumanInt(value: unknown): number {
  return parseInt(String(value ?? '0').replace(/,/g, ''), 10);
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

      if (status.isUsurped) {
        clearTimeout(timeout);
        if (unsub) unsub();
        resolve({
          ok: false,
          stage: 'usurped',
          label,
          replacedBy: status.asUsurped.toHex(),
        });
        return;
      }

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
