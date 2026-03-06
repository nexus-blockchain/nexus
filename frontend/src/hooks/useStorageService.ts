"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type {
  PinInfo, StorageOperator, BillingParams,
  GlobalHealthStats, DomainConfig, TierConfig,
  OperatorSlaData,
} from "@/lib/types";

export function useStoragePins(owner: string | null) {
  const [pins, setPins] = useState<PinInfo[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!owner) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const indexRaw = await q.storageService.ownerPinIndex(owner);
      const hashList = (indexRaw.toJSON() || []) as string[];

      const results = await Promise.all(
        hashList.map(async (cidHash: string) => {
          const [metaRaw, stateRaw, tierRaw, registryRaw] = await Promise.all([
            q.storageService.pinMeta(cidHash),
            q.storageService.pinStateOf(cidHash),
            q.storageService.cidTier(cidHash),
            q.storageService.cidRegistry(cidHash),
          ]);
          const meta = metaRaw.isNone ? null : metaRaw.toJSON();
          if (!meta) return null;
          const state = stateRaw.isNone ? null : stateRaw.toJSON();
          const tier = tierRaw.isNone ? null : tierRaw.toJSON();
          const cid = registryRaw.isNone ? null : registryRaw.toJSON();
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
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [owner]);

  useEffect(() => { fetch(); }, [fetch]);
  return { pins, isLoading, refetch: fetch };
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
          const [opRaw, bondRaw, usedRaw, pinCountRaw, rewardsRaw, healthRaw] = await Promise.all([
            q.storageService.operators(account),
            q.storageService.operatorBond(account),
            q.storageService.operatorUsedBytes(account),
            q.storageService.operatorPinCount(account),
            q.storageService.operatorRewards(account),
            q.storageService.operatorPinStats(account),
          ]);
          if (opRaw.isNone) return null;
          const op = opRaw.toJSON() as Record<string, unknown>;
          return {
            ...op,
            account,
            bond: BigInt(String(bondRaw?.toJSON?.() ?? 0)),
            usedBytes: Number(usedRaw?.toJSON?.() ?? 0),
            pinCount: Number(pinCountRaw?.toJSON?.() ?? 0),
            rewards: BigInt(String(rewardsRaw?.toJSON?.() ?? 0)),
            healthScore: healthRaw?.isNone ? 0 : Number((healthRaw.toJSON() as Record<string, unknown>).healthScore ?? 0),
          } as StorageOperator;
        })
      );
      setOperators(results.filter(Boolean) as StorageOperator[]);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { operators, isLoading, refetch: fetch };
}

export function useBillingParams() {
  const [params, setParams] = useState<BillingParams | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const [priceRaw, periodRaw, graceRaw, maxChargeRaw, minReserveRaw, pausedRaw] = await Promise.all([
        q.storageService.pricePerGiBWeek(),
        q.storageService.billingPeriodBlocks(),
        q.storageService.graceBlocks(),
        q.storageService.maxChargePerBlock(),
        q.storageService.subjectMinReserve(),
        q.storageService.billingPaused(),
      ]);
      setParams({
        pricePerGibWeek: BigInt(String(priceRaw?.toJSON?.() ?? 0)),
        periodBlocks: Number(periodRaw?.toJSON?.() ?? 0),
        graceBlocks: Number(graceRaw?.toJSON?.() ?? 0),
        maxChargePerBlock: Number(maxChargeRaw?.toJSON?.() ?? 0),
        subjectMinReserve: BigInt(String(minReserveRaw?.toJSON?.() ?? 0)),
        paused: pausedRaw?.toJSON?.() ?? false,
      });
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { params, isLoading, refetch: fetch };
}

export function useUserFundingBalance(address: string | null) {
  const [balance, setBalance] = useState<bigint>(BigInt(0));
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!address) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).storageService.userFundingBalance(address);
      setBalance(BigInt(String(raw?.toJSON?.() ?? 0)));
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [address]);

  useEffect(() => { fetch(); }, [fetch]);
  return { balance, isLoading, refetch: fetch };
}

export function useHealthStats() {
  const [stats, setStats] = useState<GlobalHealthStats | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).storageService.healthCheckStats();
      if (raw && !raw.isNone) {
        setStats(raw.toJSON() as unknown as GlobalHealthStats);
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { stats, isLoading, refetch: fetch };
}

export function useRegisteredDomains() {
  const [domains, setDomains] = useState<Array<{ name: string; config: DomainConfig; priority: number }>>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const entries = await q.storageService.registeredDomains.entries();
      const results = await Promise.all(
        entries.map(async ([key, v]: [{ args: [{ toJSON: () => string }] }, { toJSON: () => DomainConfig }]) => {
          const name = String(key.args[0].toJSON());
          const config = v.toJSON() as unknown as DomainConfig;
          let priority = 0;
          try {
            const pRaw = await q.storageService.domainPriority(key.args[0]);
            priority = Number(pRaw?.toJSON?.() ?? 0);
          } catch { /* ignore */ }
          return { name, config, priority };
        })
      );
      setDomains(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { domains, isLoading, refetch: fetch };
}

export function useTierConfigs() {
  const [configs, setConfigs] = useState<Record<string, TierConfig>>({});
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const entries = await q.storageService.pinTierConfig.entries();
      const result: Record<string, TierConfig> = {};
      for (const [key, v] of entries) {
        const tier = String(key.args[0].toJSON());
        result[tier] = v.toJSON() as unknown as TierConfig;
      }
      setConfigs(result);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { configs, isLoading, refetch: fetch };
}

export function useOperatorSla(account: string | null) {
  const [sla, setSla] = useState<OperatorSlaData | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!account) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).storageService.operatorSla(account);
      if (raw && !raw.isNone) {
        setSla(raw.toJSON() as unknown as OperatorSlaData);
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [account]);

  useEffect(() => { fetch(); }, [fetch]);
  return { sla, isLoading, refetch: fetch };
}

export function useStorageActions() {
  const { submit, state, reset } = useTx();
  return {
    requestPinForSubject: (subjectId: number, cid: string, sizeBytes: number, tier: string | null) =>
      submit("storageService", "requestPinForSubject", [subjectId, cid, sizeBytes, tier]),
    requestUnpin: (cid: string) =>
      submit("storageService", "requestUnpin", [cid]),
    batchUnpin: (cids: string[]) =>
      submit("storageService", "batchUnpin", [cids]),
    renewPin: (cidHash: string, periods: number) =>
      submit("storageService", "renewPin", [cidHash, periods]),
    upgradePinTier: (cidHash: string, newTier: string) =>
      submit("storageService", "upgradePinTier", [cidHash, newTier]),

    fundUserAccount: (targetUser: string, amount: bigint) =>
      submit("storageService", "fundUserAccount", [targetUser, amount]),
    fundSubjectAccount: (subjectId: number, amount: bigint) =>
      submit("storageService", "fundSubjectAccount", [subjectId, amount]),
    fundIpfsPool: (amount: bigint) =>
      submit("storageService", "fundIpfsPool", [amount]),

    joinOperator: (peerId: string, capacityGib: number, endpointHash: string, certFingerprint: string | null, bond: bigint) =>
      submit("storageService", "joinOperator", [peerId, capacityGib, endpointHash, certFingerprint, bond]),
    updateOperator: (peerId: string | null, capacityGib: number | null, endpointHash: string | null, certFingerprint: string | null) =>
      submit("storageService", "updateOperator", [peerId, capacityGib, endpointHash, certFingerprint]),
    leaveOperator: () =>
      submit("storageService", "leaveOperator", []),
    pauseOperator: () =>
      submit("storageService", "pauseOperator", []),
    resumeOperator: () =>
      submit("storageService", "resumeOperator", []),
    topUpBond: (amount: bigint) =>
      submit("storageService", "topUpBond", [amount]),
    reduceBond: (amount: bigint) =>
      submit("storageService", "reduceBond", [amount]),
    operatorClaimRewards: () =>
      submit("storageService", "operatorClaimRewards", []),

    setBillingParams: (pricePerGibWeek: bigint | null, periodBlocks: number | null, graceBlocks: number | null, maxChargePerBlock: number | null, subjectMinReserve: bigint | null, paused: boolean | null) =>
      submit("storageService", "setBillingParams", [pricePerGibWeek, periodBlocks, graceBlocks, maxChargePerBlock, subjectMinReserve, paused]),
    chargeDue: (limit: number) =>
      submit("storageService", "chargeDue", [limit]),
    distributeToOperators: (maxAmount: bigint) =>
      submit("storageService", "distributeToOperators", [maxAmount]),
    emergencyPauseBilling: () =>
      submit("storageService", "emergencyPauseBilling", []),
    resumeBilling: () =>
      submit("storageService", "resumeBilling", []),

    setReplicasConfig: (l0: number | null, l1: number | null, l2: number | null, l3: number | null, minThreshold: number | null) =>
      submit("storageService", "setReplicasConfig", [l0, l1, l2, l3, minThreshold]),
    updateTierConfig: (tier: string, config: TierConfig) =>
      submit("storageService", "updateTierConfig", [tier, config]),

    registerDomain: (domain: string, subjectTypeId: number, defaultTier: string, autoPinEnabled: boolean) =>
      submit("storageService", "registerDomain", [domain, subjectTypeId, defaultTier, autoPinEnabled]),
    updateDomainConfig: (domain: string, autoPinEnabled: boolean | null, defaultTier: string | null, subjectTypeId: number | null) =>
      submit("storageService", "updateDomainConfig", [domain, autoPinEnabled, defaultTier, subjectTypeId]),
    setDomainPriority: (domain: string, priority: number) =>
      submit("storageService", "setDomainPriority", [domain, priority]),

    setOperatorStatus: (who: string, status: number) =>
      submit("storageService", "setOperatorStatus", [who, status]),
    slashOperator: (who: string, amount: bigint) =>
      submit("storageService", "slashOperator", [who, amount]),
    setOperatorLayer: (operator: string, layer: string, priority: number | null) =>
      submit("storageService", "setOperatorLayer", [operator, layer, priority]),
    migrateOperatorPins: (from: string, to: string, maxPins: number) =>
      submit("storageService", "migrateOperatorPins", [from, to, maxPins]),

    governanceForceUnpin: (cid: string, reason: string) =>
      submit("storageService", "governanceForceUnpin", [cid, reason]),
    cleanupExpiredCids: (limit: number) =>
      submit("storageService", "cleanupExpiredCids", [limit]),
    cleanupExpiredLocks: (maxCount: number) =>
      submit("storageService", "cleanupExpiredLocks", [maxCount]),

    txState: state,
    resetTx: reset,
  };
}
