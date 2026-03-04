"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";

export interface CommissionConfig {
  entityId: number;
  referralEnabled: boolean;
  levelDiffEnabled: boolean;
  singleLineEnabled: boolean;
  poolRewardEnabled: boolean;
  referralRate: number;
  maxDepth: number;
}

export interface CommissionRecord {
  id: number;
  entityId: number;
  beneficiary: string;
  amount: bigint;
  source: string;
  orderId: number;
  status: string;
  createdAt: number;
}

export interface WithdrawableBalance {
  nex: bigint;
  token: bigint;
}

export function useCommissionConfig(entityId: number | null) {
  const [config, setConfig] = useState<CommissionConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).commissionCore.entityCommissionConfig(entityId);
      if (raw.isNone) { setConfig(null); } else { setConfig(raw.toJSON() as unknown as CommissionConfig); }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { config, isLoading, refetch: fetch };
}

export function useCommissionRecords(entityId: number | null) {
  const [records, setRecords] = useState<CommissionRecord[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).commissionCore.commissionRecords.entries(entityId);
      const results = entries.map(([_k, v]: [unknown, { toJSON: () => CommissionRecord }]) => v.toJSON());
      setRecords(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { records, isLoading, refetch: fetch };
}

export function useWithdrawable(entityId: number | null, account: string | null) {
  const [balance, setBalance] = useState<WithdrawableBalance>({ nex: BigInt(0), token: BigInt(0) });
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null || !account) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).commissionCore.withdrawableBalance(entityId, account);
      if (!raw.isNone) {
        const data = raw.toJSON() as { nex: string; token: string };
        setBalance({ nex: BigInt(data.nex || 0), token: BigInt(data.token || 0) });
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId, account]);

  useEffect(() => { fetch(); }, [fetch]);
  return { balance, isLoading, refetch: fetch };
}

export function useCommissionActions() {
  const { submit, state, reset } = useTx();
  return {
    configureCommission: (entityId: number, referralEnabled: boolean, levelDiffEnabled: boolean, singleLineEnabled: boolean, poolRewardEnabled: boolean) =>
      submit("commissionCore", "configureCommission", [entityId, referralEnabled, levelDiffEnabled, singleLineEnabled, poolRewardEnabled]),
    setReferralRate: (entityId: number, rate: number, maxDepth: number) =>
      submit("commissionReferral", "setReferralRate", [entityId, rate, maxDepth]),
    withdrawCommission: (entityId: number, assetType: string) =>
      submit("commissionCore", "withdrawCommission", [entityId, assetType]),
    cancelCommission: (entityId: number, recordId: number) =>
      submit("commissionCore", "cancelCommission", [entityId, recordId]),
    txState: state,
    resetTx: reset,
  };
}
