import { ApiPromise } from '@polkadot/api';
import { KeyringPair } from '@polkadot/keyring/types';
import { SubmittableExtrinsic } from '@polkadot/api/types';

export interface TxResult {
  success: boolean;
  blockHash?: string;
  txHash?: string;
  events?: any[];
  error?: string;
}

export async function signAndSend(
  api: ApiPromise,
  tx: SubmittableExtrinsic<'promise'>,
  signer: KeyringPair,
  description: string
): Promise<TxResult> {
  console.log(`\n📤 发送交易: ${description}`);

  const txHash = tx.hash.toHex();

  return new Promise((resolve) => {
    tx.signAndSend(signer, async ({ status, events: callbackEvents, dispatchError, txIndex }) => {
      if (status.isInBlock) {
        console.log(`📦 已入块: ${status.asInBlock.toHex()}`);
      }

      if (!status.isFinalized) {
        return;
      }

      const blockHash = status.asFinalized.toHex();
      console.log(`✅ 已确认: ${blockHash}`);

      if (dispatchError) {
        const errorMessage = decodeDispatchError(api, dispatchError);
        console.log(`❌ 交易失败: ${errorMessage}`);
        resolve({
          success: false,
          blockHash,
          txHash,
          error: errorMessage,
        });
        return;
      }

      try {
        const callbackTxIndex = txIndex ?? callbackEvents?.find(
          ({ phase }) => phase.isApplyExtrinsic,
        )?.phase.asApplyExtrinsic.toNumber();

        const finalizedEvents = await withTimeout(
          getExtrinsicEvents(api, blockHash, callbackTxIndex),
          15_000,
          `Timeout while fetching finalized events for ${description}`,
        );

        const failedEvent = finalizedEvents.find(
          ({ event }) => event.section === 'system' && event.method === 'ExtrinsicFailed',
        );

        if (failedEvent) {
          const errorMessage = decodeExtrinsicFailedEvent(api, failedEvent.event.data[0]);
          console.log(`❌ 交易失败: ${errorMessage}`);
          resolve({
            success: false,
            blockHash,
            txHash,
            error: errorMessage,
          });
          return;
        }

        const relevantEvents = finalizedEvents
          .filter(({ event }) =>
            event.section !== 'system' &&
            event.section !== 'transactionPayment'
          )
          .map(({ event }) => ({
            section: event.section,
            method: event.method,
            data: event.data.toHuman(),
          }));

        if (relevantEvents.length > 0) {
          console.log('📋 事件:');
          relevantEvents.forEach(e => {
            console.log(`   - ${e.section}.${e.method}:`, e.data);
          });
        }

        resolve({
          success: true,
          blockHash,
          txHash,
          events: relevantEvents,
        });
      } catch (error: any) {
        console.log(`⚠️  无法读取最终区块事件: ${error.message}`);

        const failedEvent = callbackEvents?.find(
          ({ event }) => event.section === 'system' && event.method === 'ExtrinsicFailed',
        );
        if (failedEvent) {
          const errorMessage = decodeExtrinsicFailedEvent(api, failedEvent.event.data[0]);
          console.log(`❌ 交易失败: ${errorMessage}`);
          resolve({
            success: false,
            blockHash,
            txHash,
            error: errorMessage,
          });
          return;
        }

        const relevantCallbackEvents = (callbackEvents ?? [])
          .filter(({ event }) =>
            event.section !== 'system' &&
            event.section !== 'transactionPayment'
          )
          .map(({ event }) => ({
            section: event.section,
            method: event.method,
            data: event.data.toHuman(),
          }));

        resolve({
          success: true,
          blockHash,
          txHash,
          error: `[warn] cannot fetch block events: ${error.message}`,
          events: relevantCallbackEvents,
        });
      }
    }).catch((error) => {
      console.log(`❌ 发送失败: ${error.message}`);
      resolve({
        success: false,
        error: error.message,
      });
    });
  });
}

function decodeDispatchError(api: ApiPromise, dispatchError: any): string {
  if (dispatchError?.isModule) {
    const decoded = api.registry.findMetaError(dispatchError.asModule);
    return `${decoded.section}.${decoded.name}: ${decoded.docs.join(' ')}`;
  }

  return dispatchError?.toString?.() ?? 'Unknown error';
}

function decodeExtrinsicFailedEvent(api: ApiPromise, dispatchError: any): string {
  try {
    if (dispatchError?.isModule) {
      const decoded = api.registry.findMetaError(dispatchError.asModule);
      return `${decoded.section}.${decoded.name}: ${decoded.docs.join(' ')}`;
    }

    if (dispatchError?.toHuman) {
      return JSON.stringify(dispatchError.toHuman());
    }

    return dispatchError?.toString?.() ?? 'Unknown dispatch error';
  } catch {
    return dispatchError?.toString?.() ?? 'Unknown dispatch error';
  }
}

async function getExtrinsicEvents(
  api: ApiPromise,
  blockHash: string,
  txIndex?: number,
): Promise<any[]> {
  const allEvents = await getEventsAtBlock(api, blockHash);
  let extrinsicIndex = txIndex;

  if (extrinsicIndex === undefined || extrinsicIndex === null) {
    let maxIndex = -1;
    for (const record of allEvents) {
      if (record.phase.isApplyExtrinsic) {
        const index = record.phase.asApplyExtrinsic.toNumber();
        if (index > maxIndex) {
          maxIndex = index;
        }
      }
    }

    extrinsicIndex = maxIndex >= 0 ? maxIndex : undefined;
  }

  if (extrinsicIndex === undefined || extrinsicIndex === null) {
    throw new Error('Cannot determine extrinsic index in finalized block');
  }

  return allEvents.filter((record: any) =>
    record.phase.isApplyExtrinsic && record.phase.asApplyExtrinsic.eq(extrinsicIndex),
  );
}

async function getEventsAtBlock(api: ApiPromise, blockHash: string): Promise<any[]> {
  const eventsQuery = (api.query as any)?.system?.events;
  if (eventsQuery?.at) {
    return await eventsQuery.at(blockHash) as unknown as any[];
  }

  const apiAt = await api.at(blockHash);
  return await apiAt.query.system.events() as unknown as any[];
}

async function withTimeout<T>(promise: Promise<T>, timeoutMs: number, message: string): Promise<T> {
  return await new Promise<T>((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(message)), timeoutMs);
    promise.then(
      (value) => {
        clearTimeout(timeout);
        resolve(value);
      },
      (error) => {
        clearTimeout(timeout);
        reject(error);
      },
    );
  });
}

export function formatNex(amount: bigint | string | number): string {
  const value = BigInt(amount);
  // 链上精度：1 NEX = 1e12 最小单位（12位小数）
  const nex = Number(value) / 1e12;
  return `${nex.toLocaleString()} NEX`;
}

export function formatUsdt(amount: number): string {
  const usdt = amount / 1e6;
  if (usdt < 0.01 && usdt > 0) {
    return `$${usdt.toFixed(6)} USDT`;
  }
  return `$${usdt.toFixed(2)} USDT`;
}

export function toNexWei(nex: number): string {
  // 链上精度：1 NEX = 1e12 最小单位（12位小数）
  return (BigInt(Math.floor(nex * 1e12))).toString();
}

export function toUsdtWei(usdt: number): string {
  return Math.floor(usdt * 1e6).toString();
}

export function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

export function logSection(title: string): void {
  console.log('\n' + '='.repeat(60));
  console.log(`  ${title}`);
  console.log('='.repeat(60));
}

export function logStep(step: number, description: string): void {
  console.log(`\n📌 步骤 ${step}: ${description}`);
}

export function logSuccess(message: string): void {
  console.log(`✅ ${message}`);
}

export function logError(message: string): void {
  console.log(`❌ ${message}`);
}

export function logInfo(message: string): void {
  console.log(`ℹ️  ${message}`);
}

export function logQuery(name: string, value: any): void {
  console.log(`📊 ${name}:`, typeof value === 'object' ? JSON.stringify(value, null, 2) : value);
}
