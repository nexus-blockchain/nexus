import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';
import * as fs from 'fs';
import * as path from 'path';
import { getApi, disconnectApi } from './utils/api.js';
import { NEXUS_SS58_FORMAT } from './utils/ss58.js';
import {
  logError,
  logInfo,
  logSection,
  logStep,
  logSuccess,
  signAndSend,
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
  error?: string | null;
}

interface BatchSummary {
  timestamp: string;
  startIndex: number;
  endIndex: number;
  shopFund: number;
  total: number;
  successCount: number;
  failCount: number;
  results: AccountSetupResult[];
  repairedAt?: string;
}

interface RepairItem {
  accountIndex: number;
  entityId: number;
  ownerAddress: string;
  tokenAssetId: number;
  action: 'skipped' | 'created' | 'failed';
  tokenMetadata?: unknown;
  error?: string;
}

function isMissingToken(result: AccountSetupResult): result is AccountSetupResult & {
  entityId: number;
} {
  return typeof result.entityId === 'number' && result.tokenMetadata === null;
}

async function main() {
  await cryptoWaitReady();

  const batchFile = path.join(process.cwd(), 'entity-setup-batch-result.json');
  const accountsFile = path.join(process.cwd(), 'test-accounts.json');

  if (!fs.existsSync(batchFile)) {
    logError(`Missing file: ${batchFile}`);
    process.exitCode = 1;
    return;
  }

  if (!fs.existsSync(accountsFile)) {
    logError(`Missing file: ${accountsFile}`);
    process.exitCode = 1;
    return;
  }

  const summary = JSON.parse(fs.readFileSync(batchFile, 'utf-8')) as BatchSummary;
  const accounts = JSON.parse(fs.readFileSync(accountsFile, 'utf-8')) as AccountInfo[];
  const accountByIndex = new Map(accounts.map((account) => [account.index, account]));
  const targets = summary.results.filter(isMissingToken);

  if (targets.length === 0) {
    logInfo('No missing token metadata found in entity-setup-batch-result.json');
    return;
  }

  logSection('Repair missing entity tokens');
  console.log(`   Targets: ${targets.length}`);
  console.log(`   Account indexes: ${targets.map((item) => item.accountIndex).join(', ')}`);

  const keyring = new Keyring({ type: 'sr25519', ss58Format: NEXUS_SS58_FORMAT });
  const api = await getApi();
  const repairItems: RepairItem[] = [];

  try {
    for (const target of targets) {
      const account = accountByIndex.get(target.accountIndex);
      const tokenAssetId = target.tokenAssetId ?? (1_000_000 + target.entityId);

      logStep(target.accountIndex, `Repair account #${target.accountIndex} entity #${target.entityId}`);

      if (!account) {
        const error = `missing account #${target.accountIndex} in test-accounts.json`;
        target.success = false;
        target.error = error;
        repairItems.push({
          accountIndex: target.accountIndex,
          entityId: target.entityId,
          ownerAddress: target.ownerAddress,
          tokenAssetId,
          action: 'failed',
          error,
        });
        console.log(`   ❌ ${error}`);
        continue;
      }

      const owner = keyring.addFromMnemonic(account.mnemonic);
      const existingMetadata = await (api.query as any).entityToken.entityTokenMetadata(target.entityId);
      const existingMetadataHuman = existingMetadata.toHuman();

      if (existingMetadataHuman !== null) {
        target.ownerAddress = owner.address;
        target.tokenAssetId = tokenAssetId;
        target.tokenMetadata = existingMetadataHuman;
        target.success = true;
        target.error = null;
        repairItems.push({
          accountIndex: target.accountIndex,
          entityId: target.entityId,
          ownerAddress: owner.address,
          tokenAssetId,
          action: 'skipped',
          tokenMetadata: existingMetadataHuman,
        });
        console.log('   ℹ️  Token already exists, refreshed local result');
        continue;
      }

      const tokenSymbol = `BT${target.entityId}`.slice(0, 8);
      const createTokenTx = (api.tx as any).entityToken.createShopToken(
        target.entityId,
        `Batch Token ${target.entityId}`,
        tokenSymbol,
        12,
        0,
        100,
      );

      const txResult = await signAndSend(
        api,
        createTokenTx,
        owner,
        `repair token for entity #${target.entityId}`,
      );

      if (!txResult.success) {
        const error = `create token failed: ${txResult.error}`;
        target.ownerAddress = owner.address;
        target.tokenAssetId = tokenAssetId;
        target.success = false;
        target.error = error;
        repairItems.push({
          accountIndex: target.accountIndex,
          entityId: target.entityId,
          ownerAddress: owner.address,
          tokenAssetId,
          action: 'failed',
          error,
        });
        console.log(`   ❌ ${error}`);
        continue;
      }

      const repairedMetadata = await (api.query as any).entityToken.entityTokenMetadata(target.entityId);
      const repairedMetadataHuman = repairedMetadata.toHuman();

      if (repairedMetadataHuman === null) {
        const error = 'create token finished but token metadata is still missing on-chain';
        target.ownerAddress = owner.address;
        target.tokenAssetId = tokenAssetId;
        target.success = false;
        target.error = error;
        repairItems.push({
          accountIndex: target.accountIndex,
          entityId: target.entityId,
          ownerAddress: owner.address,
          tokenAssetId,
          action: 'failed',
          error,
        });
        console.log(`   ❌ ${error}`);
        continue;
      }

      target.ownerAddress = owner.address;
      target.tokenAssetId = tokenAssetId;
      target.tokenMetadata = repairedMetadataHuman;
      target.success = true;
      target.error = null;
      repairItems.push({
        accountIndex: target.accountIndex,
        entityId: target.entityId,
        ownerAddress: owner.address,
        tokenAssetId,
        action: 'created',
        tokenMetadata: repairedMetadataHuman,
      });
      console.log(`   ✅ Token #${tokenAssetId} repaired`);
    }

    summary.timestamp = new Date().toISOString();
    summary.repairedAt = summary.timestamp;
    summary.total = summary.results.length;
    summary.successCount = summary.results.filter((item) => item.success).length;
    summary.failCount = summary.results.filter((item) => !item.success).length;

    fs.writeFileSync(batchFile, JSON.stringify(summary, null, 2), 'utf-8');

    const repairFile = path.join(process.cwd(), 'entity-token-repair-result.json');
    fs.writeFileSync(
      repairFile,
      JSON.stringify(
        {
          timestamp: summary.timestamp,
          total: repairItems.length,
          createdCount: repairItems.filter((item) => item.action === 'created').length,
          skippedCount: repairItems.filter((item) => item.action === 'skipped').length,
          failedCount: repairItems.filter((item) => item.action === 'failed').length,
          items: repairItems,
        },
        null,
        2,
      ),
      'utf-8',
    );

    logSection('Repair finished');
    console.log(`   Batch file: ${batchFile}`);
    console.log(`   Repair file: ${repairFile}`);
    console.log(`   Created: ${repairItems.filter((item) => item.action === 'created').length}`);
    console.log(`   Skipped: ${repairItems.filter((item) => item.action === 'skipped').length}`);
    console.log(`   Failed: ${repairItems.filter((item) => item.action === 'failed').length}`);

    if (summary.failCount === 0) {
      logSuccess('All batch accounts now have entity/shop/token data');
    } else {
      logError(`${summary.failCount} batch accounts still have unresolved issues`);
      process.exitCode = 1;
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
