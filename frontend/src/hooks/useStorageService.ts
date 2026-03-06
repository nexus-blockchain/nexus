"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { PinInfo, StorageOperator } from "@/lib/types";

export function useStoragePins(owner: string | null) {
  const [pins, setPins] = useState<PinInfo[]>([]);
  const [cidHashes, setCidHashes] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!owner) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;

      const indexRaw = await q.storageService.ownerPinIndex(owner);
      const hashList = (indexRaw.toJSON() || []) as string[];
      setCidHashes(hashList);

      const results = await Promise.all(
        hashList.map(async (cidHash: string) => {
          const [metaRaw, stateRaw, tierRaw, registryRaw] = await Promise.all([
            q.storageService.pinMeta(cidHash),
            q.storageService.pinStateOf(cidHash),
            q.storageService.cidTier(cidHash),
            q.storageService.cidRegistry(cidHash),
          ]);

          const meta = metaRaw.isNone ? null : metaRaw.toJSON();
          const state = stateRaw.isNone ? null : stateRaw.toJSON();
          const tier = tierRaw.isNone ? null : tierRaw.toJSON();
          const cid = registryRaw.isNone ? null : registryRaw.toJSON();

          if (!meta) return null;

          return {
            ...meta,
            cidHash,
            cid: cid ?? "",
            state: String(state ?? "Unknown"),
            tier: String(tier ?? "Standard"),
            owner,
          } as PinInfo;
        })
      );

      setPins(results.filter(Boolean) as PinInfo[]);
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [owner]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { pins, cidHashes, isLoading, refetch: fetch };
}

export function useStorageOperators() {
  const [operators, setOperators] = useState<StorageOperator[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;

      const indexRaw = await q.storageService.activeOperatorIndex();
      const accountIds = (indexRaw.toJSON() || []) as string[];

      const results = await Promise.all(
        accountIds.map(async (account: string) => {
          const [opRaw, bondRaw] = await Promise.all([
            q.storageService.operators(account),
            q.storageService.operatorBond(account),
          ]);

          if (opRaw.isNone) return null;

          const op = opRaw.toJSON() as Record<string, unknown>;
          const bond = bondRaw.isNone ? BigInt(0) : BigInt(String(bondRaw.toJSON()));

          return {
            ...op,
            account,
            bond,
          } as StorageOperator;
        })
      );

      setOperators(results.filter(Boolean) as StorageOperator[]);
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { operators, isLoading, refetch: fetch };
}

export function useStorageActions() {
  const { submit, state, reset } = useTx();
  return {
    requestPinForSubject: (cidHash: string, subjectTypeId: number, subjectId: number, tier: string) =>
      submit("storageService", "requestPinForSubject", [cidHash, subjectTypeId, subjectId, tier]),
    requestUnpin: (cidHash: string) =>
      submit("storageService", "requestUnpin", [cidHash]),
    renewPin: (cidHash: string) =>
      submit("storageService", "renewPin", [cidHash]),
    upgradePinTier: (cidHash: string, newTier: string) =>
      submit("storageService", "upgradePinTier", [cidHash, newTier]),
    fundUserAccount: (amount: bigint) =>
      submit("storageService", "fundUserAccount", [amount]),
    joinOperator: (peerId: string, capacityGib: number, endpointHash: string) =>
      submit("storageService", "joinOperator", [peerId, capacityGib, endpointHash]),
    leaveOperator: () =>
      submit("storageService", "leaveOperator", []),
    txState: state,
    resetTx: reset,
  };
}
