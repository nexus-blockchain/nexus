"use client";

import { useState, useEffect, useCallback } from "react";
import { ApiPromise, WsProvider } from "@polkadot/api";

interface ChainInfo {
  name: string;
  bestBlock: number;
  finalizedBlock: number;
}

interface ApiState {
  api: ApiPromise | null;
  isConnected: boolean;
  isConnecting: boolean;
  chainInfo: ChainInfo;
  error: string | null;
}

let apiInstance: ApiPromise | null = null;
let apiPromise: Promise<ApiPromise> | null = null;

function getWsEndpoint(): string {
  return process.env.NEXT_PUBLIC_WS_ENDPOINT || "ws://localhost:9944";
}

async function getApi(): Promise<ApiPromise> {
  if (apiInstance && apiInstance.isConnected) return apiInstance;
  if (apiPromise) return apiPromise;

  apiPromise = ApiPromise.create({
    provider: new WsProvider(getWsEndpoint()),
  }).then((api) => {
    apiInstance = api;
    return api;
  });

  return apiPromise;
}

export function useApi(): ApiState {
  const [state, setState] = useState<ApiState>({
    api: null,
    isConnected: false,
    isConnecting: true,
    chainInfo: { name: "", bestBlock: 0, finalizedBlock: 0 },
    error: null,
  });

  useEffect(() => {
    let cancelled = false;

    const connect = async () => {
      try {
        const api = await getApi();
        if (cancelled) return;

        const [chain, bestHeader, finalizedHash] = await Promise.all([
          api.rpc.system.chain(),
          api.rpc.chain.getHeader(),
          api.rpc.chain.getFinalizedHead(),
        ]);

        const finalizedHeader = await api.rpc.chain.getHeader(finalizedHash);

        setState({
          api,
          isConnected: true,
          isConnecting: false,
          chainInfo: {
            name: chain.toString(),
            bestBlock: bestHeader.number.toNumber(),
            finalizedBlock: finalizedHeader.number.toNumber(),
          },
          error: null,
        });

        await api.rpc.chain.subscribeNewHeads((header) => {
          if (!cancelled) {
            setState((prev) => ({
              ...prev,
              chainInfo: {
                ...prev.chainInfo,
                bestBlock: header.number.toNumber(),
              },
            }));
          }
        });
      } catch (err) {
        if (!cancelled) {
          setState((prev) => ({
            ...prev,
            isConnecting: false,
            error: err instanceof Error ? err.message : "Connection failed",
          }));
        }
      }
    };

    connect();

    return () => {
      cancelled = true;
    };
  }, []);

  return state;
}

export { getApi };
