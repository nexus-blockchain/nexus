import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';
import * as fs from 'fs';
import * as path from 'path';
import { getApi, disconnectApi } from './utils/api.js';
import { NEXUS_SS58_FORMAT } from './utils/ss58.js';
import {
  formatNex,
  logError,
  logSection,
  logStep,
  logSuccess,
  signAndSend,
  toNexWei,
} from './utils/helpers.js';

interface AccountInfo {
  index: number;
  mnemonic: string;
  address: string;
  publicKey: string;
}

interface SetupSummary {
  timestamp: string;
  accountIndex: number;
  ownerAddress: string;
  entityId: number;
  primaryShopId: number | null;
  createdShopId: number;
  tokenAssetId: number;
  tokenMetadata: unknown;
  balanceBefore: string;
  balanceAfter: string;
}

function parseId(value: unknown): number | null {
  if (value === null || value === undefined) {
    return null;
  }
  if (typeof value === 'number') {
    return value;
  }
  const text = String(value).replace(/,/g, '').trim();
  if (!text) {
    return null;
  }
  const parsed = Number(text);
  return Number.isFinite(parsed) ? parsed : null;
}

async function main() {
  await cryptoWaitReady();

  const args = process.argv.slice(2);
  const accountIndex = args[0] ? parseInt(args[0], 10) : 1;
  const shopFund = args[1] ? parseFloat(args[1]) : 50;

  if (!Number.isInteger(accountIndex) || accountIndex <= 0) {
    logError('accountIndex must be a positive integer');
    process.exitCode = 1;
    return;
  }

  if (Number.isNaN(shopFund) || shopFund < 0) {
    logError('shopFund must be a non-negative number');
    process.exitCode = 1;
    return;
  }

  const accountsFile = path.join(process.cwd(), 'test-accounts.json');
  if (!fs.existsSync(accountsFile)) {
    logError(`Missing file: ${accountsFile}`);
    console.log('Run create-test-accounts.ts first.');
    process.exitCode = 1;
    return;
  }

  const accounts = JSON.parse(fs.readFileSync(accountsFile, 'utf-8')) as AccountInfo[];
  const selected = accounts[accountIndex - 1];
  if (!selected) {
    logError(`Account #${accountIndex} not found in ${accountsFile}`);
    process.exitCode = 1;
    return;
  }

  logSection(`Setup entity/shop/token from account #${accountIndex}`);

  const keyring = new Keyring({ type: 'sr25519', ss58Format: NEXUS_SS58_FORMAT });
  const owner = keyring.addFromMnemonic(selected.mnemonic);
  const api = await getApi();

  try {
    logStep(1, 'Inspect selected account');
    const balanceBefore = await api.query.system.account(owner.address);
    console.log(`   Address: ${owner.address}`);
    console.log(`   Free balance: ${formatNex(balanceBefore.data.free.toString())}`);

    logStep(2, 'Create entity');
    const entityId = (await (api.query as any).entityRegistry.nextEntityId()).toNumber();
    const createEntityTx = (api.tx as any).entityRegistry.createEntity(
      `Batch Entity ${entityId}`,
      null,
      `batch-entity-${entityId}`,
      null,
    );
    const entityResult = await signAndSend(api, createEntityTx, owner, `create entity #${entityId}`);
    if (!entityResult.success) {
      throw new Error(`create entity failed: ${entityResult.error}`);
    }

    const entityShopIds = (await (api.query as any).entityRegistry.entityShops(entityId)).toHuman() as unknown[];
    const primaryShopId = entityShopIds.length > 0 ? parseId(entityShopIds[0]) : null;
    console.log(`   Entity ID: ${entityId}`);
    console.log(`   Primary shop ID: ${primaryShopId ?? 'n/a'}`);

    logStep(3, 'Create an extra shop');
    const createdShopId = (await (api.query as any).entityShop.nextShopId()).toNumber();
    const createShopTx = (api.tx as any).entityShop.createShop(
      entityId,
      `Batch Shop ${createdShopId}`,
      'OnlineStore',
      toNexWei(shopFund),
    );
    const shopResult = await signAndSend(api, createShopTx, owner, `create shop #${createdShopId}`);
    if (!shopResult.success) {
      throw new Error(`create shop failed: ${shopResult.error}`);
    }

    const createdShop = await (api.query as any).entityShop.shops(createdShopId);
    console.log(`   Created shop ID: ${createdShopId}`);
    console.log(`   Shop data: ${JSON.stringify(createdShop.toHuman())}`);

    logStep(4, 'Create token');
    const tokenAssetId = 1_000_000 + entityId;
    const tokenSymbol = `BT${entityId}`.slice(0, 8);
    const createTokenTx = (api.tx as any).entityToken.createShopToken(
      entityId,
      `Batch Token ${entityId}`,
      tokenSymbol,
      12,
      0,
      100,
    );
    const tokenResult = await signAndSend(api, createTokenTx, owner, `create token for entity #${entityId}`);
    if (!tokenResult.success) {
      throw new Error(`create token failed: ${tokenResult.error}`);
    }

    const tokenMetadata = await (api.query as any).entityToken.entityTokenMetadata(entityId);
    const tokenMetadataHuman = tokenMetadata.toHuman();
    if (tokenMetadataHuman === null) {
      throw new Error('create token finished but token metadata is still missing on-chain');
    }
    console.log(`   Token asset ID: ${tokenAssetId}`);
    console.log(`   Token metadata: ${JSON.stringify(tokenMetadataHuman)}`);

    logStep(5, 'Write summary');
    const balanceAfter = await api.query.system.account(owner.address);
    const summary: SetupSummary = {
      timestamp: new Date().toISOString(),
      accountIndex,
      ownerAddress: owner.address,
      entityId,
      primaryShopId,
      createdShopId,
      tokenAssetId,
      tokenMetadata: tokenMetadataHuman,
      balanceBefore: balanceBefore.data.free.toString(),
      balanceAfter: balanceAfter.data.free.toString(),
    };

    const summaryFile = path.join(process.cwd(), 'entity-setup-result.json');
    fs.writeFileSync(summaryFile, JSON.stringify(summary, null, 2), 'utf-8');
    console.log(`   Summary file: ${summaryFile}`);
    console.log(`   Balance after: ${formatNex(balanceAfter.data.free.toString())}`);

    logSuccess('Entity/shop/token setup completed');
  } finally {
    await disconnectApi();
  }
}

main().catch((error: Error) => {
  logError(error.message);
  console.error(error);
  process.exitCode = 1;
});
