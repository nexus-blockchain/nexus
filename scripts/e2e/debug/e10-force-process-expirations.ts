/**
 * Minimal repro for the E10 forceProcessExpirations path.
 *
 * Strategy:
 * - reuse Eve's existing shop + on-sale physical product
 * - create one isolated Paid order so ExpiryQueue definitely contains a target block
 * - first call forceProcessExpirations(currentHead) to mirror E10's empty-path shape
 * - then call forceProcessExpirations(expiryBlock) and inspect queue/order state directly
 */

import { disconnectApi, getApi, getFreeBalance } from '../core/chain-state.js';
import { createFlowAccounts, getDevAccounts } from '../fixtures/accounts.js';
import { nex } from '../core/config.js';

async function main(): Promise<void> {
  const api = await getApi();
  const shared = getDevAccounts();
  const actors = createFlowAccounts(`E10Expiry${Date.now()}`, ['Bob']);
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

  const physicalProductId = await findReusableProduct(api, shopId, 'Physical');
  console.log(`[debug] physical_product_id=${physicalProductId}`);

  const orderId = (await (api.query as any).entityTransaction.nextOrderId()).toNumber();
  console.log(`[debug] paid_order_id=${orderId}`);
  console.log(`[debug] place_physical_order_result=${JSON.stringify(await watchTx(
    placeOrderTx(api, physicalProductId, `e10-debug-expiry-${physicalProductId}`),
    buyer,
    `placeOrder(${orderId})`,
  ))}`);
  const orderBefore = await getOrder(api, orderId);
  console.log(`[debug] order_before_force=${JSON.stringify(orderBefore)}`);

  const headBefore = (await api.rpc.chain.getHeader()).number.toNumber();
  const shipTimeout = (api.consts as any).entityTransaction.shipTimeout.toNumber();
  const expiryBlock = parseHumanInt(orderBefore?.createdAt) + shipTimeout;
  console.log(`[debug] current_head=${headBefore}`);
  console.log(`[debug] ship_timeout=${shipTimeout}`);
  console.log(`[debug] expiry_block=${expiryBlock}`);
  console.log(`[debug] expiry_queue_target_before=${JSON.stringify(await getExpiryQueue(api, expiryBlock))}`);

  const emptyPathResult = await watchTx(
    api.tx.sudo.sudo((api.tx as any).entityTransaction.forceProcessExpirations(headBefore)),
    alice,
    `sudo(forceProcessExpirations(${headBefore}))`,
  );
  console.log(`[debug] force_process_expirations_current_head_result=${JSON.stringify(emptyPathResult)}`);
  console.log(`[debug] expiry_queue_target_after_current_head=${JSON.stringify(await getExpiryQueue(api, expiryBlock))}`);
  console.log(`[debug] order_after_current_head_force=${JSON.stringify(await getOrder(api, orderId))}`);

  const targetPathResult = await watchTx(
    api.tx.sudo.sudo((api.tx as any).entityTransaction.forceProcessExpirations(expiryBlock)),
    alice,
    `sudo(forceProcessExpirations(${expiryBlock}))`,
  );
  console.log(`[debug] force_process_expirations_target_result=${JSON.stringify(targetPathResult)}`);
  console.log(`[debug] expiry_queue_target_after_force=${JSON.stringify(await getExpiryQueue(api, expiryBlock))}`);
  console.log(`[debug] order_after_target_force=${JSON.stringify(await getOrder(api, orderId))}`);

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
  category: 'Physical',
): Promise<number> {
  const nextProductId = (await (api.query as any).entityProduct.nextProductId()).toNumber();

  for (let productId = nextProductId - 1; productId >= 1; productId -= 1) {
    const productOpt = await (api.query as any).entityProduct.products(productId);
    if (productOpt.isNone) continue;

    const product = productOpt.toHuman() as Record<string, string>;
    if (parseHumanInt(product.shopId) !== shopId) continue;
    if (product.category !== category) continue;
    if (product.status !== 'OnSale') continue;
    if (parseHumanInt(product.stock) <= 0) continue;

    return productId;
  }

  throw new Error(`No reusable on-sale ${category} product found for shop ${shopId}`);
}

async function getOrder(api: any, orderId: number): Promise<Record<string, unknown> | null> {
  const orderOpt = await (api.query as any).entityTransaction.orders(orderId);
  if (orderOpt.isNone) return null;
  return orderOpt.toHuman() as Record<string, unknown>;
}

async function getExpiryQueue(api: any, blockNumber: number): Promise<string[]> {
  const queue = await (api.query as any).entityTransaction.expiryQueue(blockNumber);
  return (queue.toHuman() as string[]) ?? [];
}

function parseHumanInt(value: unknown): number {
  return parseInt(String(value ?? '0').replace(/,/g, ''), 10);
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
