/**
 * 链状态管理 — 连接、查询、交易提交
 */

import { ApiPromise, WsProvider } from '@polkadot/api';
import { KeyringPair } from '@polkadot/keyring/types';
import { SubmittableExtrinsic } from '@polkadot/api/types';
import { defaultConfig } from './config.js';

let apiInstance: ApiPromise | null = null;

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
  const txHash = tx.hash.toHex();

  // Phase 1: synchronous callback — only captures blockHash + txIndex
  const inclusion = await new Promise<{ blockHash: string; txIndex?: number } | { error: string }>((resolve) => {
    const timeout = setTimeout(() => {
      resolve({ error: `Timeout: ${description}` });
    }, defaultConfig.txTimeout);

    tx.signAndSend(signer, ({ status, txIndex }) => {
      if (!status.isFinalized) return;
      clearTimeout(timeout);
      resolve({ blockHash: status.asFinalized.toHex(), txIndex });
    }).catch((error: Error) => {
      clearTimeout(timeout);
      resolve({ error: error.message });
    });
  });

  if ('error' in inclusion) {
    return { success: false, events: [], error: inclusion.error };
  }

  const { blockHash, txIndex } = inclusion;

  // Phase 2: async event fetching from the finalized block
  // (Polkadot.js v12 callback returns empty events for extrinsic v5 runtimes)
  try {
    const apiAt = await api.at(blockHash);
    const allEvents: any = await apiAt.query.system.events();

    // Determine our extrinsic index
    let extIdx: number | undefined = txIndex;
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
    }

    // Collect events for our extrinsic
    const ourRecords = extIdx !== undefined
      ? allEvents.filter((r: any) =>
          r.phase.isApplyExtrinsic && r.phase.asApplyExtrinsic.eq(extIdx))
      : [];

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

    return { success: true, blockHash, txHash, events: txEvents };
  } catch (fetchErr: any) {
    return {
      success: true,
      blockHash,
      txHash,
      events: [],
      error: `[warn] cannot fetch block events: ${fetchErr.message}`,
    };
  }
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
