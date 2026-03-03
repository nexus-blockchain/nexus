"use client";

import { useState, useCallback } from "react";
import { getApi } from "./useApi";
import { useWalletStore } from "@/stores/wallet";

type TxStatus = "idle" | "signing" | "broadcasting" | "inBlock" | "finalized" | "error";

interface TxState {
  status: TxStatus;
  txHash: string | null;
  blockHash: string | null;
  error: string | null;
}

interface UseTxReturn {
  submit: (palletName: string, callName: string, params: unknown[]) => Promise<void>;
  state: TxState;
  reset: () => void;
}

const initialState: TxState = {
  status: "idle",
  txHash: null,
  blockHash: null,
  error: null,
};

export function useTx(): UseTxReturn {
  const [state, setState] = useState<TxState>(initialState);
  const address = useWalletStore((s) => s.address);

  const submit = useCallback(
    async (palletName: string, callName: string, params: unknown[]) => {
      if (!address) {
        setState({ ...initialState, status: "error", error: "Wallet not connected" });
        return;
      }

      setState({ ...initialState, status: "signing" });

      try {
        const api = await getApi();
        const { web3FromAddress } = await import("@polkadot/extension-dapp");
        const injector = await web3FromAddress(address);

        const pallet = (api.tx as Record<string, Record<string, (...args: unknown[]) => unknown>>)[palletName];
        if (!pallet || !pallet[callName]) {
          throw new Error(`Extrinsic ${palletName}.${callName} not found`);
        }

        const extrinsic = pallet[callName](...params) as {
          signAndSend: (
            address: string,
            options: { signer: unknown },
            callback: (result: {
              status: { isInBlock: boolean; isFinalized: boolean; asInBlock?: { toString: () => string }; asFinalized?: { toString: () => string } };
              txHash: { toString: () => string };
              dispatchError?: unknown;
            }) => void
          ) => Promise<() => void>;
        };

        setState((prev) => ({ ...prev, status: "broadcasting" }));

        await new Promise<void>((resolve, reject) => {
          extrinsic
            .signAndSend(address, { signer: injector.signer }, (result) => {
              const { status, txHash, dispatchError } = result;

              if (dispatchError) {
                setState({
                  status: "error",
                  txHash: txHash.toString(),
                  blockHash: null,
                  error: String(dispatchError),
                });
                reject(new Error(String(dispatchError)));
                return;
              }

              if (status.isInBlock) {
                setState({
                  status: "inBlock",
                  txHash: txHash.toString(),
                  blockHash: status.asInBlock?.toString() || null,
                  error: null,
                });
              }

              if (status.isFinalized) {
                setState({
                  status: "finalized",
                  txHash: txHash.toString(),
                  blockHash: status.asFinalized?.toString() || null,
                  error: null,
                });
                resolve();
              }
            })
            .catch(reject);
        });
      } catch (err) {
        setState((prev) => ({
          ...prev,
          status: "error",
          error: err instanceof Error ? err.message : "Transaction failed",
        }));
      }
    },
    [address]
  );

  const reset = useCallback(() => {
    setState(initialState);
  }, []);

  return { submit, state, reset };
}
