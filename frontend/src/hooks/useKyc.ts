"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { KycRecord, KycProviderData, EntityKycRequirement } from "@/lib/types";

export function useKycRecords(entityId: number | null) {
  const [records, setRecords] = useState<KycRecord[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityKyc.kycRecords.entries(entityId);
      const results = entries.map(([key, v]: [{ args: [unknown, { toString: () => string }] }, { toJSON: () => Record<string, unknown> }]) => ({
        account: key.args[1].toString(),
        ...v.toJSON(),
      })) as KycRecord[];
      setRecords(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { records, isLoading, refetch: fetch };
}

export function useKycProviders() {
  const [providers, setProviders] = useState<KycProviderData[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityKyc.providers.entries();
      const results = entries.map(([key, v]: [{ args: [{ toString: () => string }] }, { toJSON: () => Record<string, unknown> }]) => ({
        account: key.args[0].toString(),
        ...v.toJSON(),
      })) as KycProviderData[];
      setProviders(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { providers, isLoading, refetch: fetch };
}

export function useEntityKycRequirement(entityId: number | null) {
  const [requirement, setRequirement] = useState<EntityKycRequirement | null>(null);
  const [pendingCount, setPendingCount] = useState(0);
  const [approvedCount, setApprovedCount] = useState(0);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const [rawReq, rawPending, rawApproved] = await Promise.all([
        q.entityKyc.entityRequirements(entityId),
        q.entityKyc.pendingKycCount(entityId),
        q.entityKyc.approvedKycCount(entityId),
      ]);
      if (rawReq && !rawReq.isNone) {
        setRequirement(rawReq.toJSON() as unknown as EntityKycRequirement);
      }
      setPendingCount(rawPending?.toJSON?.() ?? 0);
      setApprovedCount(rawApproved?.toJSON?.() ?? 0);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { requirement, pendingCount, approvedCount, isLoading, refetch: fetch };
}

export function useAuthorizedProviders(entityId: number | null) {
  const [authorized, setAuthorized] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityKyc.entityAuthorizedProviders.entries(entityId);
      const accounts = entries.map(([key]: [{ args: [unknown, { toString: () => string }] }]) => key.args[1].toString());
      setAuthorized(accounts);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { authorized, isLoading, refetch: fetch };
}

export function useKycActions() {
  const { submit, state, reset } = useTx();
  return {
    submitKyc: (entityId: number, level: string, dataCid: string, countryCode: string) =>
      submit("entityKyc", "submitKyc", [entityId, level, dataCid, countryCode]),
    approveKyc: (entityId: number, account: string, riskScore: number) =>
      submit("entityKyc", "approveKyc", [entityId, account, riskScore]),
    rejectKyc: (entityId: number, account: string, reason: string, detailsCid: string | null) =>
      submit("entityKyc", "rejectKyc", [entityId, account, reason, detailsCid]),
    revokeKyc: (entityId: number, account: string, reason: string) =>
      submit("entityKyc", "revokeKyc", [entityId, account, reason]),
    expireKyc: (entityId: number, account: string) =>
      submit("entityKyc", "expireKyc", [entityId, account]),
    renewKyc: (entityId: number, account: string) =>
      submit("entityKyc", "renewKyc", [entityId, account]),
    cancelKyc: (entityId: number) =>
      submit("entityKyc", "cancelKyc", [entityId]),
    updateKycData: (entityId: number, newDataCid: string) =>
      submit("entityKyc", "updateKycData", [entityId, newDataCid]),
    purgeKycData: (entityId: number) =>
      submit("entityKyc", "purgeKycData", [entityId]),
    forceApproveKyc: (entityId: number, account: string, level: string, riskScore: number, countryCode: string) =>
      submit("entityKyc", "forceApproveKyc", [entityId, account, level, riskScore, countryCode]),
    updateRiskScore: (entityId: number, account: string, newScore: number) =>
      submit("entityKyc", "updateRiskScore", [entityId, account, newScore]),
    timeoutPendingKyc: (entityId: number, account: string) =>
      submit("entityKyc", "timeoutPendingKyc", [entityId, account]),

    registerProvider: (providerAccount: string, name: string, providerType: string, maxLevel: string) =>
      submit("entityKyc", "registerProvider", [providerAccount, name, providerType, maxLevel]),
    removeProvider: (providerAccount: string) =>
      submit("entityKyc", "removeProvider", [providerAccount]),
    updateProvider: (providerAccount: string, name: string | null, maxLevel: string | null) =>
      submit("entityKyc", "updateProvider", [providerAccount, name, maxLevel]),
    suspendProvider: (providerAccount: string) =>
      submit("entityKyc", "suspendProvider", [providerAccount]),
    resumeProvider: (providerAccount: string) =>
      submit("entityKyc", "resumeProvider", [providerAccount]),
    authorizeProvider: (entityId: number, providerAccount: string) =>
      submit("entityKyc", "authorizeProvider", [entityId, providerAccount]),
    deauthorizeProvider: (entityId: number, providerAccount: string) =>
      submit("entityKyc", "deauthorizeProvider", [entityId, providerAccount]),

    setEntityRequirement: (entityId: number, minLevel: string, mandatory: boolean, gracePeriod: number, allowHighRisk: boolean, maxRiskScore: number) =>
      submit("entityKyc", "setEntityRequirement", [entityId, minLevel, mandatory, gracePeriod, allowHighRisk, maxRiskScore]),
    removeEntityRequirement: (entityId: number) =>
      submit("entityKyc", "removeEntityRequirement", [entityId]),
    updateHighRiskCountries: (countries: string[]) =>
      submit("entityKyc", "updateHighRiskCountries", [countries]),

    txState: state,
    resetTx: reset,
  };
}
