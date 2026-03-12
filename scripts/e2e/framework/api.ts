import { ApiPromise, WsProvider } from '@polkadot/api';
import { SubmittableExtrinsic } from '@polkadot/api/types';
import { KeyringPair } from '@polkadot/keyring/types';
import { EventRecord } from '@polkadot/types/interfaces';
import { codecToJson } from './codec.js';
import { ChainSnapshot } from './types.js';

export interface TxEvent {
  section: string;
  method: string;
  data: unknown;
}

export interface TxReceipt {
  label: string;
  success: boolean;
  txHash: string;
  blockHash?: string;
  extrinsicIndex?: number;
  events: TxEvent[];
  error?: string;
}

function toNumberMaybe(value: any): number | undefined {
  if (value == null) {
    return undefined;
  }
  if (typeof value === 'number') {
    return value;
  }
  if (typeof value.toNumber === 'function') {
    return value.toNumber();
  }
  return undefined;
}

function decodeDispatchError(api: ApiPromise, dispatchError: any): string {
  try {
    if (dispatchError?.isModule) {
      const decoded = api.registry.findMetaError(dispatchError.asModule);
      return `${decoded.section}.${decoded.name}: ${decoded.docs.join(' ')}`;
    }
  } catch {
    // fall through
  }

  if (typeof dispatchError?.toString === 'function') {
    return dispatchError.toString();
  }

  return 'Unknown dispatch error';
}

export async function connectApi(wsUrl: string = process.env.WS_URL ?? 'ws://127.0.0.1:9944'): Promise<ApiPromise> {
  const provider = new WsProvider(wsUrl);
  return ApiPromise.create({ provider });
}

export async function disconnectApi(api: ApiPromise): Promise<void> {
  await api.disconnect();
}

export async function captureChainSnapshot(api: ApiPromise): Promise<ChainSnapshot> {
  const [chain, nodeName, nodeVersion] = await Promise.all([
    api.rpc.system.chain(),
    api.rpc.system.name(),
    api.rpc.system.version(),
  ]);

  return {
    chain: chain.toString(),
    nodeName: nodeName.toString(),
    nodeVersion: nodeVersion.toString(),
    specName: api.runtimeVersion.specName.toString(),
    specVersion: api.runtimeVersion.specVersion.toNumber(),
  };
}

export async function submitTx(
  api: ApiPromise,
  tx: SubmittableExtrinsic<'promise'>,
  signer: KeyringPair,
  label: string,
): Promise<TxReceipt> {
  const txHash = tx.hash.toHex();
  const timeoutMs = Number(process.env.E2E_TX_TIMEOUT_MS ?? 90_000);

  const inclusion = await new Promise<{ blockHash: string; txIndex?: number } | { error: string }>((resolve) => {
    let settled = false;
    let unsubscribe: undefined | (() => void);

    const finish = (result: { blockHash: string; txIndex?: number } | { error: string }) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timeout);
      if (unsubscribe) {
        try {
          unsubscribe();
        } catch {
          // ignore unsubscribe errors
        }
      }
      resolve(result);
    };

    const timeout = setTimeout(() => {
      finish({ error: `Timed out while waiting for finalized status: ${label}` });
    }, timeoutMs);

    tx.signAndSend(signer, (result: any) => {
      if (!result.status?.isFinalized) {
        return;
      }
      finish({
        blockHash: result.status.asFinalized.toHex(),
        txIndex: toNumberMaybe(result.txIndex),
      });
    }).then((unsub) => {
      unsubscribe = unsub;
      if (settled) {
        try {
          unsubscribe();
        } catch {
          // ignore unsubscribe errors
        }
      }
    }).catch((error: Error) => {
      finish({ error: error.message });
    });
  });

  if ('error' in inclusion) {
    return {
      label,
      success: false,
      txHash,
      events: [],
      error: inclusion.error,
    };
  }

  const { blockHash } = inclusion;
  const [signedBlock, allEventsCodec] = await Promise.all([
    api.rpc.chain.getBlock(blockHash),
    api.query.system.events.at(blockHash),
  ]);
  const allEvents = Array.from(allEventsCodec as unknown as Iterable<EventRecord>);

  let extrinsicIndex = inclusion.txIndex;
  if (extrinsicIndex == null) {
    extrinsicIndex = signedBlock.block.extrinsics.findIndex((extrinsic) => extrinsic.hash.toHex() === txHash);
    if (extrinsicIndex < 0) {
      extrinsicIndex = undefined;
    }
  }

  const records = extrinsicIndex == null
    ? []
    : allEvents.filter((record) =>
        record.phase.isApplyExtrinsic && record.phase.asApplyExtrinsic.toNumber() === extrinsicIndex,
      );

  const failed = records.find(
    (record) => record.event.section === 'system' && record.event.method === 'ExtrinsicFailed',
  );

  if (failed) {
    return {
      label,
      success: false,
      txHash,
      blockHash,
      extrinsicIndex,
      events: [],
      error: decodeDispatchError(api, failed.event.data[0]),
    };
  }

  const events: TxEvent[] = records
    .filter((record) => record.event.section !== 'system' && record.event.section !== 'transactionPayment')
    .map((record) => ({
      section: record.event.section as string,
      method: record.event.method as string,
      data: codecToJson(record.event.data),
    }));

  return {
    label,
    success: true,
    txHash,
    blockHash,
    extrinsicIndex,
    events,
  };
}
