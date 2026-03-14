#!/usr/bin/env tsx

process.env.WS_URL ??= 'wss://202.140.140.202';
process.env.NODE_TLS_REJECT_UNAUTHORIZED ??= '0';

import { chmod, mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady, mnemonicGenerate } from '@polkadot/util-crypto';
import { connectApi, disconnectApi, submitTx } from './e2e/framework/api.js';
import { getDevActors, readFreeBalance } from './e2e/framework/accounts.js';
import { assert, assertTxSuccess } from './e2e/framework/assert.js';
import { formatNex, nex } from './e2e/framework/units.js';
import { readEntity, readEntityIds, resolvePrimaryShopId, waitForNewEntityId } from './e2e/suites/helpers.js';
import { NEXUS_SS58_FORMAT } from './utils/ss58.js';

const fundAmountNex = Number(process.env.NEW_WALLET_FUND_NEX ?? '5000');
const createdAt = new Date();
const walletName = process.env.NEW_WALLET_NAME ?? `entity-owner-${createdAt.getTime()}`;
const entityName = process.env.NEW_ENTITY_NAME ?? `entity-${createdAt.getTime()}`;

async function main(): Promise<void> {
  await cryptoWaitReady();

  const keyring = new Keyring({ type: 'sr25519', ss58Format: NEXUS_SS58_FORMAT });
  const mnemonic = mnemonicGenerate();
  const wallet = keyring.addFromUri(mnemonic, { name: walletName });

  const api = await connectApi(process.env.WS_URL);

  try {
    const actors = await getDevActors();
    const fundAmount = nex(fundAmountNex);

    const balanceBeforeFunding = await readFreeBalance(api, wallet.address);

    const fundingTx = (api.tx.balances as any).transferAllowDeath
      ? (api.tx.balances as any).transferAllowDeath(wallet.address, fundAmount.toString())
      : (api.tx.balances as any).transferKeepAlive(wallet.address, fundAmount.toString());
    const fundingReceipt = await submitTx(api, fundingTx, actors.alice, 'fund generated wallet');
    assertTxSuccess(fundingReceipt, 'fund generated wallet should succeed');

    const balanceAfterFunding = await readFreeBalance(api, wallet.address);
    const beforeEntityIds = await readEntityIds(api, wallet.address);
    const expectedEntityId = Number((await (api.query as any).entityRegistry.nextEntityId()).toString());

    const createEntityTx = (api.tx.entityRegistry as any).createEntity(entityName, null, null, null);
    const createEntityReceipt = await submitTx(api, createEntityTx, wallet, 'create entity with generated wallet');
    assertTxSuccess(createEntityReceipt, 'create entity with generated wallet should succeed');

    const detected = await waitForNewEntityId(api, wallet.address, beforeEntityIds, 10, 1_500);
    assert(detected.entityId != null && detected.entityId > 0, 'failed to detect the entity created by the generated wallet');

    const entityId = detected.entityId;
    const entity = await readEntity(api, entityId);
    const primaryShopId = resolvePrimaryShopId(entity);
    const balanceAfterCreate = await readFreeBalance(api, wallet.address);

    const secretsDir = join(process.cwd(), 'secrets');
    await mkdir(secretsDir, { recursive: true });

    const timestamp = createdAt.toISOString().replace(/[:.]/g, '-');
    const secretFile = join(secretsDir, `generated-wallet-entity-${timestamp}.json`);

    const payload = {
      createdAt: createdAt.toISOString(),
      network: process.env.WS_URL,
      wallet: {
        name: walletName,
        address: wallet.address,
        mnemonic,
        cryptoType: 'sr25519',
        ss58Format: NEXUS_SS58_FORMAT,
      },
      funding: {
        faucet: 'alice',
        faucetAddress: actors.alice.address,
        amountNex: fundAmountNex,
        amountPlanck: fundAmount.toString(),
        txHash: fundingReceipt.txHash,
        blockHash: fundingReceipt.blockHash ?? null,
      },
      entity: {
        name: entityName,
        expectedEntityId,
        entityId,
        primaryShopId,
        txHash: createEntityReceipt.txHash,
        blockHash: createEntityReceipt.blockHash ?? null,
      },
      balances: {
        beforeFunding: {
          planck: balanceBeforeFunding.toString(),
          formatted: formatNex(balanceBeforeFunding),
        },
        afterFunding: {
          planck: balanceAfterFunding.toString(),
          formatted: formatNex(balanceAfterFunding),
        },
        afterCreate: {
          planck: balanceAfterCreate.toString(),
          formatted: formatNex(balanceAfterCreate),
        },
      },
    };

    await writeFile(secretFile, `${JSON.stringify(payload, null, 2)}\n`, { mode: 0o600 });
    await chmod(secretFile, 0o600).catch(() => undefined);

    const output = {
      createdAt: payload.createdAt,
      network: payload.network,
      wallet: {
        name: payload.wallet.name,
        address: payload.wallet.address,
        cryptoType: payload.wallet.cryptoType,
        ss58Format: payload.wallet.ss58Format,
      },
      funding: payload.funding,
      entity: payload.entity,
      balances: payload.balances,
      secretFile,
    };

    console.log(JSON.stringify(output, null, 2));
  } finally {
    await disconnectApi(api);
  }
}

main().catch((error) => {
  console.error(`create wallet + entity failed: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
});
