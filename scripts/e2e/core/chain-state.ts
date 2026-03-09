/**
 * 链状态管理 — 连接、查询、交易提交
 */

import { ApiPromise, WsProvider } from '@polkadot/api';
import { KeyringPair } from '@polkadot/keyring/types';
import { SubmittableExtrinsic } from '@polkadot/api/types';
import { defaultConfig } from './config.js';

let apiInstance: ApiPromise | null = null;
const EVENT_FETCH_TIMEOUT_MS = Number(process.env.E2E_EVENT_FETCH_TIMEOUT_MS ?? 15_000);
const TRACE_TX_ENABLED = process.env.E2E_TRACE_TX === '1';
const TRACE_TX_FILTER = process.env.E2E_TRACE_TX_FILTER ?? '';

export interface TxResult {
  success: boolean;
  blockHash?: string;
  txHash?: string;
  events: TxEvent[];
  error?: string;
}

export interface TxEvent {
  section: string;
  method: string;
  data: any;
}

function shouldTraceTx(description?: string): boolean {
  if (!TRACE_TX_ENABLED) return false;
  if (!TRACE_TX_FILTER) return true;
  return (description ?? '').includes(TRACE_TX_FILTER);
}

function traceTx(description: string | undefined, message: string): void {
  console.log(`[tx-trace] ${description ?? 'unknown'} :: ${message}`);
}

/** 获取或创建 API 连接 */
export async function getApi(wsUrl?: string): Promise<ApiPromise> {
  if (apiInstance && apiInstance.isConnected) return apiInstance;
  const url = wsUrl ?? defaultConfig.wsUrl;
  const provider = new WsProvider(url);
  apiInstance = await ApiPromise.create({ provider });
  return apiInstance;
}

/** 断开连接 */
export async function disconnectApi(): Promise<void> {
  if (apiInstance) {
    await apiInstance.disconnect();
    apiInstance = null;
  }
}

/** 签名并发送交易，等待 finalized，从区块手动拉取事件 */
export async function signAndSend(
  api: ApiPromise,
  tx: SubmittableExtrinsic<'promise'>,
  signer: KeyringPair,
  description?: string,
): Promise<TxResult> {
  return signAndSendWithRetry(api, tx, signer, description, 0);
}

async function signAndSendWithRetry(
  api: ApiPromise,
  tx: SubmittableExtrinsic<'promise'>,
  signer: KeyringPair,
  description: string | undefined,
  attempt: number,
): Promise<TxResult> {
  const txHash = tx.hash.toHex();
  const traceEnabled = shouldTraceTx(description);
  const submitStartedAt = Date.now();

  if (traceEnabled) {
    traceTx(
      description,
      `submit_start attempt=${attempt} signer=${signer.address} txHash=${txHash}`,
    );
  }

  // Phase 1: synchronous callback — only captures blockHash + txIndex
  const inclusion = await new Promise<{ blockHash: string; txIndex?: number } | { error: string }>((resolve) => {
    const timeout = setTimeout(() => {
      if (traceEnabled) {
        traceTx(description, `submit_timeout afterMs=${Date.now() - submitStartedAt}`);
      }
      resolve({ error: `Timeout: ${description}` });
    }, defaultConfig.txTimeout);

    tx.signAndSend(signer, ({ status, txIndex }) => {
      if (traceEnabled) {
        traceTx(
          description,
          `callback status=${status.type} txIndex=${txIndex ?? 'n/a'} afterMs=${Date.now() - submitStartedAt}`,
        );
      }
      if (!status.isFinalized) return;
      clearTimeout(timeout);
      resolve({ blockHash: status.asFinalized.toHex(), txIndex });
    }).catch((error: Error) => {
      clearTimeout(timeout);
      if (traceEnabled) {
        traceTx(description, `submit_error afterMs=${Date.now() - submitStartedAt} error=${error.message}`);
      }
      resolve({ error: error.message });
    });
  });

  if ('error' in inclusion) {
    if (traceEnabled) {
      traceTx(description, `submit_result error=${inclusion.error}`);
    }
    if (attempt < 2 && inclusion.error.includes('Priority is too low')) {
      if (traceEnabled) {
        traceTx(description, `retry_after_priority_low nextAttempt=${attempt + 1}`);
      }
      await waitBlocks(api, 1);
      return signAndSendWithRetry(api, tx, signer, description, attempt + 1);
    }
    return { success: false, events: [], error: inclusion.error };
  }

  const { blockHash, txIndex } = inclusion;

  if (traceEnabled) {
    traceTx(
      description,
      `submit_finalized blockHash=${blockHash} txIndex=${txIndex ?? 'n/a'} afterMs=${Date.now() - submitStartedAt}`,
    );
  }

  // Phase 2: async event fetching from the finalized block
  // (Polkadot.js v12 callback returns empty events for extrinsic v5 runtimes)
  try {
    if (traceEnabled) {
      traceTx(description, `event_fetch_start timeoutMs=${EVENT_FETCH_TIMEOUT_MS}`);
    }
    const allEvents = await withTimeout(
      getEventsAtBlock(api, blockHash),
      EVENT_FETCH_TIMEOUT_MS,
      `Timeout while fetching finalized events for ${description ?? txHash}`,
    );

    if (traceEnabled) {
      traceTx(description, `event_fetch_done totalEvents=${allEvents.length}`);
    }

    // Determine our extrinsic index
    let extIdx: number | undefined = txIndex;
    let extIdxSource = 'callback';
    if (extIdx === undefined || extIdx === null) {
      // Heuristic: highest extrinsic index = our user tx (inherents have lower indices)
      let maxIdx = -1;
      for (const record of allEvents) {
        if (record.phase.isApplyExtrinsic) {
          const idx = record.phase.asApplyExtrinsic.toNumber();
          if (idx > maxIdx) maxIdx = idx;
        }
      }
      extIdx = maxIdx >= 0 ? maxIdx : undefined;
      extIdxSource = 'heuristic';
    }

    if (traceEnabled) {
      traceTx(description, `event_fetch_ext_idx selected=${extIdx ?? 'n/a'} source=${extIdxSource}`);
    }

    // Collect events for our extrinsic
    const ourRecords = extIdx !== undefined
      ? allEvents.filter((r: any) =>
          r.phase.isApplyExtrinsic && r.phase.asApplyExtrinsic.eq(extIdx))
      : [];

    if (traceEnabled) {
      traceTx(description, `event_fetch_records matched=${ourRecords.length}`);
    }

    // Check for ExtrinsicFailed
    const failedRecord = ourRecords.find(
      (r: any) => r.event.section === 'system' && r.event.method === 'ExtrinsicFailed',
    );

    if (failedRecord) {
      const errData = failedRecord.event.data[0];
      let errorMessage = 'Unknown dispatch error';
      try {
        if (errData.isModule) {
          const decoded = api.registry.findMetaError(errData.asModule);
          errorMessage = `${decoded.section}.${decoded.name}: ${decoded.docs.join(' ')}`;
        } else {
          errorMessage = errData.toHuman ? JSON.stringify(errData.toHuman()) : errData.toString();
        }
      } catch { /* keep fallback message */ }
      if (traceEnabled) {
        traceTx(description, `event_fetch_failed error=${errorMessage}`);
      }
      return { success: false, blockHash, txHash, events: [], error: errorMessage };
    }

    // Extract non-system events
    const txEvents: TxEvent[] = ourRecords
      .filter((r: any) =>
        r.event.section !== 'system' && r.event.section !== 'transactionPayment',
      )
      .map((r: any) => ({
        section: r.event.section as string,
        method: r.event.method as string,
        data: r.event.data.toHuman(),
      }));

    if (traceEnabled) {
      traceTx(
        description,
        `event_fetch_success eventCount=${txEvents.length} events=${txEvents.map((e) => `${e.section}.${e.method}`).join(',') || 'none'}`,
      );
    }

    return { success: true, blockHash, txHash, events: txEvents };
  } catch (fetchErr: any) {
    if (traceEnabled) {
      traceTx(description, `event_fetch_error error=${fetchErr.message}`);
    }
    return {
      success: true,
      blockHash,
      txHash,
      events: [],
      error: `[warn] cannot fetch block events: ${fetchErr.message}`,
    };
  }
}

async function getEventsAtBlock(api: ApiPromise, blockHash: string): Promise<any[]> {
  const eventsQuery = (api.query as any)?.system?.events;
  if (eventsQuery?.at) {
    return await eventsQuery.at(blockHash);
  }

  const apiAt = await api.at(blockHash);
  return await apiAt.query.system.events();
}

async function withTimeout<T>(promise: Promise<T>, timeoutMs: number, message: string): Promise<T> {
  return await new Promise<T>((resolve, reject) => {
    const timeout = setTimeout(() => {
      reject(new Error(message));
    }, timeoutMs);

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

/** 通过 Sudo 发送交易，检查 Sudid 事件确认内部调用结果 */
export async function sudoSend(
  api: ApiPromise,
  tx: SubmittableExtrinsic<'promise'>,
  sudoAccount: KeyringPair,
  description?: string,
): Promise<TxResult> {
  const sudoTx = api.tx.sudo.sudo(tx);
  const result = await signAndSend(api, sudoTx, sudoAccount, `sudo: ${description}`);

  // Even if the outer extrinsic succeeded, the inner call may have failed.
  // Check the sudo.Sudid event for the inner dispatch result.
  if (result.success) {
    const sudidEvent = result.events.find(
      (e) => e.section === 'sudo' && e.method === 'Sudid',
    );
    if (sudidEvent) {
      const sudoResult = sudidEvent.data?.sudoResult ?? sudidEvent.data?.[0];
      if (sudoResult && typeof sudoResult === 'object' && 'Err' in sudoResult) {
        return {
          ...result,
          success: false,
          error: `sudo inner call failed: ${JSON.stringify(sudoResult.Err)}`,
        };
      }
    }
  }

  return result;
}

/** 查询 storage 值 */
export async function queryStorage(
  api: ApiPromise,
  pallet: string,
  storage: string,
  ...keys: any[]
): Promise<any> {
  const query = (api.query as any)[pallet]?.[storage];
  if (!query) throw new Error(`Storage not found: ${pallet}.${storage}`);
  return query(...keys);
}

/** 获取账户余额 (free) */
export async function getFreeBalance(api: ApiPromise, address: string): Promise<bigint> {
  const account = await api.query.system.account(address);
  return BigInt((account as any).data.free.toString());
}

/** 等待指定数量的区块 */
export async function waitBlocks(api: ApiPromise, count: number): Promise<void> {
  const start = (await api.rpc.chain.getHeader()).number.toNumber();
  const target = start + count;
  return new Promise((resolve) => {
    const unsub = api.rpc.chain.subscribeNewHeads((header) => {
      if (header.number.toNumber() >= target) {
        unsub.then((u) => u());
        resolve();
      }
    });
  });
}
