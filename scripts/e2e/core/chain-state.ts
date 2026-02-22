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

/** 签名并发送交易，等待 finalized */
export async function signAndSend(
  api: ApiPromise,
  tx: SubmittableExtrinsic<'promise'>,
  signer: KeyringPair,
  description?: string,
): Promise<TxResult> {
  return new Promise((resolve) => {
    const timeout = setTimeout(() => {
      resolve({ success: false, events: [], error: `Timeout: ${description}` });
    }, defaultConfig.txTimeout);

    tx.signAndSend(signer, ({ status, events, dispatchError }) => {
      if (!status.isFinalized) return;
      clearTimeout(timeout);

      const blockHash = status.asFinalized.toHex();

      if (dispatchError) {
        let errorMessage = 'Unknown error';
        if (dispatchError.isModule) {
          const decoded = api.registry.findMetaError(dispatchError.asModule);
          errorMessage = `${decoded.section}.${decoded.name}: ${decoded.docs.join(' ')}`;
        } else {
          errorMessage = dispatchError.toString();
        }
        resolve({ success: false, blockHash, events: [], error: errorMessage });
        return;
      }

      const txEvents: TxEvent[] = events
        .filter(({ event }) =>
          !event.section.includes('system') &&
          !event.section.includes('transactionPayment')
        )
        .map(({ event }) => ({
          section: event.section,
          method: event.method,
          data: event.data.toHuman(),
        }));

      resolve({
        success: true,
        blockHash,
        txHash: tx.hash.toHex(),
        events: txEvents,
      });
    }).catch((error: Error) => {
      clearTimeout(timeout);
      resolve({ success: false, events: [], error: error.message });
    });
  });
}

/** 通过 Sudo 发送交易 */
export async function sudoSend(
  api: ApiPromise,
  tx: SubmittableExtrinsic<'promise'>,
  sudoAccount: KeyringPair,
  description?: string,
): Promise<TxResult> {
  const sudoTx = api.tx.sudo.sudo(tx);
  return signAndSend(api, sudoTx, sudoAccount, `sudo: ${description}`);
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
