"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { KycRecord } from "@/lib/types";

export function useKycRecords(entityId: number | null) {
  const [records, setRecords] = useState<KycRecord[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityKyc.kycRecords.entries(entityId);
      const results = entries.map(([_k, v]: [unknown, { toJSON: () => KycRecord }]) => v.toJSON());
      setRecords(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { records, isLoading, refetch: fetch };
}

export function useKycActions() {
  const { submit, state, reset } = useTx();
  return {
    submitKyc: (level: string, countryCode: string, dataCid: string) =>
      submit("entityKyc", "submitKyc", [level, countryCode, dataCid]),
    approveKyc: (account: string, level: string, riskScore: number, expiresAt: number) =>
      submit("entityKyc", "approveKyc", [account, level, riskScore, expiresAt]),
    rejectKyc: (account: string, reason: string) =>
      submit("entityKyc", "rejectKyc", [account, reason]),
    revokeKyc: (account: string, reason: string) =>
      submit("entityKyc", "revokeKyc", [account, reason]),
    registerProvider: (provider: string, maxLevel: string) =>
      submit("entityKyc", "registerProvider", [provider, maxLevel]),
    removeProvider: (provider: string) =>
      submit("entityKyc", "removeProvider", [provider]),
    setEntityRequirement: (entityId: number, minLevel: string, maxRiskScore: number) =>
      submit("entityKyc", "setEntityRequirement", [entityId, minLevel, maxRiskScore]),
    updateHighRiskCountries: (countries: string[]) =>
      submit("entityKyc", "updateHighRiskCountries", [countries]),
    expireKyc: (account: string) =>
      submit("entityKyc", "expireKyc", [account]),
    txState: state,
    resetTx: reset,
  };
}
