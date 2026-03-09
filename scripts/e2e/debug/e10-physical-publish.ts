/**
 * Minimal repro for the E10 physical product publish step.
 *
 * This bypasses the normal E2E event-fetch path and watches the extrinsic
 * status directly so we can tell whether `publishProduct` itself finalizes.
 */

import { getApi, disconnectApi, getFreeBalance } from '../core/chain-state.js';
import { getDevAccounts } from '../fixtures/accounts.js';
import { nex } from '../core/config.js';

async function main(): Promise<void> {
  const api = await getApi();
  const actors = getDevAccounts();
  const eve = actors.eve;

  console.log('[debug] connected');
  console.log(`[debug] eve=${eve.address}`);
  console.log(`[debug] eve_free_balance=${await getFreeBalance(api, eve.address)}`);

  const { entityId, shopId } = await ensureEntityAndShop(api, eve.address);
  console.log(`[debug] entityId=${entityId} shopId=${shopId}`);

  const productId = (await (api.query as any).entityProduct.nextProductId()).toNumber();
  console.log(`[debug] next physical productId=${productId}`);

  const createTx = (api.tx as any).entityProduct.createProduct(
    shopId,
    `E10-debug-physical-name-${productId}`,
    `E10-debug-physical-images-${productId}`,
    `E10-debug-physical-detail-${productId}`,
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

  const createResult = await watchTx(createTx, eve, `createProduct(${productId})`);
  console.log(`[debug] create_result=${JSON.stringify(createResult)}`);
  await logProductStatus(api, productId, 'after_create');

  const publishTx = (api.tx as any).entityProduct.publishProduct(productId);
  const publishResult = await watchTx(publishTx, eve, `publishProduct(${productId})`);
  console.log(`[debug] publish_result=${JSON.stringify(publishResult)}`);
  await logProductStatus(api, productId, 'after_publish_attempt');

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

async function logProductStatus(api: any, productId: number, label: string): Promise<void> {
  const productOpt = await (api.query as any).entityProduct.products(productId);
  if (productOpt.isNone) {
    console.log(`[debug] ${label}: product ${productId} missing`);
    return;
  }
  const human = productOpt.toHuman();
  const status = (human as any)?.status;
  console.log(`[debug] ${label}: product ${productId} status=${JSON.stringify(status)}`);
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
