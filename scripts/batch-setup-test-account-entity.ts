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

interface AccountSetupResult {
  accountIndex: number;
  ownerAddress: string;
  balanceBefore: string;
  balanceAfter?: string;
  entityId?: number;
  primaryShopId?: number | null;
  createdShopId?: number;
  tokenAssetId?: number;
  tokenMetadata?: unknown;
  success: boolean;
  error?: string;
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

async function setupOne(
  api: Awaited<ReturnType<typeof getApi>>,
  keyring: Keyring,
  account: AccountInfo,
  shopFund: number,
): Promise<AccountSetupResult> {
  const owner = keyring.addFromMnemonic(account.mnemonic);
  const balanceBefore = await api.query.system.account(owner.address);

  const result: AccountSetupResult = {
    accountIndex: account.index,
    ownerAddress: owner.address,
    balanceBefore: balanceBefore.data.free.toString(),
    success: false,
  };

  const entityId = (await (api.query as any).entityRegistry.nextEntityId()).toNumber();
  const createEntityTx = (api.tx as any).entityRegistry.createEntity(
    `Batch Entity ${entityId}`,
    null,
    `batch-entity-${entityId}`,
    null,
  );
  const entityResult = await signAndSend(api, createEntityTx, owner, `account #${account.index} create entity #${entityId}`);
  if (!entityResult.success) {
    result.error = `create entity failed: ${entityResult.error}`;
    return result;
  }

  const entityShopIds = (await (api.query as any).entityRegistry.entityShops(entityId)).toHuman() as unknown[];
  const primaryShopId = entityShopIds.length > 0 ? parseId(entityShopIds[0]) : null;

  const createdShopId = (await (api.query as any).entityShop.nextShopId()).toNumber();
  const createShopTx = (api.tx as any).entityShop.createShop(
    entityId,
    `Batch Shop ${createdShopId}`,
    'OnlineStore',
    toNexWei(shopFund),
  );
  const shopResult = await signAndSend(api, createShopTx, owner, `account #${account.index} create shop #${createdShopId}`);
  if (!shopResult.success) {
    result.entityId = entityId;
    result.primaryShopId = primaryShopId;
    result.error = `create shop failed: ${shopResult.error}`;
    return result;
  }

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
  const tokenResult = await signAndSend(api, createTokenTx, owner, `account #${account.index} create token for entity #${entityId}`);
  if (!tokenResult.success) {
    result.entityId = entityId;
    result.primaryShopId = primaryShopId;
    result.createdShopId = createdShopId;
    result.tokenAssetId = tokenAssetId;
    result.error = `create token failed: ${tokenResult.error}`;
    return result;
  }

  const tokenMetadata = await (api.query as any).entityToken.entityTokenMetadata(entityId);
  const tokenMetadataHuman = tokenMetadata.toHuman();
  if (tokenMetadataHuman === null) {
    result.entityId = entityId;
    result.primaryShopId = primaryShopId;
    result.createdShopId = createdShopId;
    result.tokenAssetId = tokenAssetId;
    result.error = 'create token finished but token metadata is still missing on-chain';
    return result;
  }

  const balanceAfter = await api.query.system.account(owner.address);

  result.entityId = entityId;
  result.primaryShopId = primaryShopId;
  result.createdShopId = createdShopId;
  result.tokenAssetId = tokenAssetId;
  result.tokenMetadata = tokenMetadataHuman;
  result.balanceAfter = balanceAfter.data.free.toString();
  result.success = true;

  return result;
}

async function main() {
  await cryptoWaitReady();

  const args = process.argv.slice(2);
  const startIndex = args[0] ? parseInt(args[0], 10) : 2;
  const endIndex = args[1] ? parseInt(args[1], 10) : 20;
  const shopFund = args[2] ? parseFloat(args[2]) : 50;

  if (!Number.isInteger(startIndex) || !Number.isInteger(endIndex) || startIndex <= 0 || endIndex < startIndex) {
    logError('startIndex/endIndex are invalid');
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
    process.exitCode = 1;
    return;
  }

  const accounts = JSON.parse(fs.readFileSync(accountsFile, 'utf-8')) as AccountInfo[];
  const selected = accounts.filter((account) => account.index >= startIndex && account.index <= endIndex);

  if (selected.length === 0) {
    logError(`No accounts found in range ${startIndex}-${endIndex}`);
    process.exitCode = 1;
    return;
  }

  logSection(`Batch setup entity/shop/token for accounts #${startIndex}-${endIndex}`);
  console.log(`   Accounts: ${selected.length}`);
  console.log(`   Shop fund: ${shopFund} NEX`);

  const keyring = new Keyring({ type: 'sr25519', ss58Format: NEXUS_SS58_FORMAT });
  const api = await getApi();

  const results: AccountSetupResult[] = [];

  try {
    for (const account of selected) {
      logStep(account.index, `Setup account #${account.index}`);
      console.log(`   Address: ${account.address}`);

      try {
        const result = await setupOne(api, keyring, account, shopFund);
        results.push(result);

        if (result.success) {
          console.log(`   ✅ Entity #${result.entityId}, primary shop #${result.primaryShopId}, extra shop #${result.createdShopId}, token #${result.tokenAssetId}`);
          console.log(`   Balance: ${formatNex(result.balanceBefore)} -> ${formatNex(result.balanceAfter ?? '0')}`);
        } else {
          console.log(`   ❌ ${result.error}`);
        }
      } catch (error: any) {
        const failed: AccountSetupResult = {
          accountIndex: account.index,
          ownerAddress: account.address,
          balanceBefore: '0',
          success: false,
          error: error.message,
        };
        results.push(failed);
        console.log(`   ❌ Exception: ${error.message}`);
      }
    }

    const summary = {
      timestamp: new Date().toISOString(),
      startIndex,
      endIndex,
      shopFund,
      total: results.length,
      successCount: results.filter((item) => item.success).length,
      failCount: results.filter((item) => !item.success).length,
      results,
    };

    const summaryFile = path.join(process.cwd(), 'entity-setup-batch-result.json');
    fs.writeFileSync(summaryFile, JSON.stringify(summary, null, 2), 'utf-8');

    logSection('Batch setup finished');
    console.log(`   Summary file: ${summaryFile}`);
    console.log(`   Success: ${summary.successCount}`);
    console.log(`   Failed: ${summary.failCount}`);

    if (summary.failCount === 0) {
      logSuccess('All accounts completed entity/shop/token setup');
    } else {
      logError(`${summary.failCount} accounts failed during batch setup`);
    }
  } finally {
    await disconnectApi();
  }
}

main().catch((error: Error) => {
  logError(error.message);
  console.error(error);
  process.exitCode = 1;
});
