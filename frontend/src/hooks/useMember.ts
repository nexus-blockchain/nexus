"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { MemberData, LevelData, UpgradeRule } from "@/lib/types";

export function useMembers(entityId: number | null) {
  const [members, setMembers] = useState<MemberData[]>([]);
  const [count, setCount] = useState(0);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityMember.entityMembers.entries(entityId);
      const results = entries.map(([key, v]: [{ args: [unknown, { toString: () => string }] }, { toJSON: () => Record<string, unknown> }]) => ({
        account: key.args[1].toString(),
        ...v.toJSON(),
      })) as MemberData[];
      setMembers(results);
      setCount(results.length);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { members, count, isLoading, refetch: fetch };
}

export function useLevels(entityId: number | null) {
  const [levels, setLevels] = useState<LevelData[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).entityMember.entityLevelSystems(entityId);
      if (raw && !raw.isNone) {
        const data = raw.toJSON();
        if (data?.customLevels) {
          setLevels(data.customLevels as LevelData[]);
        }
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { levels, isLoading, refetch: fetch };
}

export interface LevelSystemInfo {
  useCustom: boolean;
  upgradeMode: string;
  customLevels: LevelData[];
}

export function useLevelSystem(entityId: number | null) {
  const [system, setSystem] = useState<LevelSystemInfo | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).entityMember.entityLevelSystems(entityId);
      if (raw && !raw.isNone) {
        setSystem(raw.toJSON() as unknown as LevelSystemInfo);
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { system, isLoading, refetch: fetch };
}

export interface UpgradeRuleSystem {
  enabled: boolean;
  conflictStrategy: string;
  rules: UpgradeRule[];
  nextRuleId: number;
}

export function useUpgradeRuleSystem(entityId: number | null) {
  const [ruleSystem, setRuleSystem] = useState<UpgradeRuleSystem | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).entityMember.entityUpgradeRules(entityId);
      if (raw && !raw.isNone) {
        setRuleSystem(raw.toJSON() as unknown as UpgradeRuleSystem);
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { ruleSystem, isLoading, refetch: fetch };
}

export function useUpgradeRules(entityId: number | null) {
  const { ruleSystem, isLoading, refetch } = useUpgradeRuleSystem(entityId);
  return { rules: ruleSystem?.rules ?? [], isLoading, refetch };
}

export function useMemberPolicy(entityId: number | null) {
  const [policyBits, setPolicyBits] = useState<number | null>(null);
  const [statsPolicyBits, setStatsPolicyBits] = useState<number | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const [rawPolicy, rawStats] = await Promise.all([
        q.entityMember.entityMemberPolicy(entityId),
        q.entityMember.entityMemberStatsPolicy?.(entityId),
      ]);
      if (rawPolicy && !rawPolicy.isNone) {
        const val = rawPolicy.toJSON();
        setPolicyBits(typeof val === "number" ? val : (val as number));
      }
      if (rawStats && !rawStats.isNone) {
        const val = rawStats.toJSON();
        setStatsPolicyBits(typeof val === "number" ? val : (val as number));
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { policyBits, statsPolicyBits, isLoading, refetch: fetch };
}

export function useMemberCount(entityId: number | null) {
  const [count, setCount] = useState(0);
  const [bannedCount, setBannedCount] = useState(0);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const [rawCount, rawBanned] = await Promise.all([
        q.entityMember.memberCount(entityId),
        q.entityMember.bannedMemberCount(entityId),
      ]);
      setCount(rawCount?.toJSON?.() ?? 0);
      setBannedCount(rawBanned?.toJSON?.() ?? 0);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { count, bannedCount, isLoading, refetch: fetch };
}

export function usePendingMembers(entityId: number | null) {
  const [pending, setPending] = useState<Array<{ account: string; referrer: string | null; appliedAt: number }>>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityMember.pendingMembers.entries(entityId);
      const results = entries.map(([key, val]: [{ args: [unknown, { toString: () => string }] }, { toJSON: () => [string | null, number] }]) => {
        const data = val.toJSON();
        return {
          account: key.args[1].toString(),
          referrer: Array.isArray(data) ? data[0] : null,
          appliedAt: Array.isArray(data) ? data[1] : 0,
        };
      });
      setPending(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { pending, isLoading, refetch: fetch };
}

export function useMemberActions() {
  const { submit, state, reset } = useTx();
  return {
    registerMember: (shopId: number, referrer: string | null) =>
      submit("entityMember", "registerMember", [shopId, referrer]),
    bindReferrer: (shopId: number, referrer: string) =>
      submit("entityMember", "bindReferrer", [shopId, referrer]),

    initLevelSystem: (shopId: number, useCustom: boolean, upgradeMode: string) =>
      submit("entityMember", "initLevelSystem", [shopId, useCustom, upgradeMode]),
    addCustomLevel: (shopId: number, name: string, threshold: number, discountRate: number, commissionBonus: number) =>
      submit("entityMember", "addCustomLevel", [shopId, name, threshold, discountRate, commissionBonus]),
    updateCustomLevel: (shopId: number, levelId: number, name?: string | null, threshold?: number | null, discountRate?: number | null, commissionBonus?: number | null) =>
      submit("entityMember", "updateCustomLevel", [shopId, levelId, name, threshold, discountRate, commissionBonus]),
    removeCustomLevel: (shopId: number, levelId: number) =>
      submit("entityMember", "removeCustomLevel", [shopId, levelId]),
    resetLevelSystem: (shopId: number) =>
      submit("entityMember", "resetLevelSystem", [shopId]),
    setUseCustomLevels: (shopId: number, useCustom: boolean) =>
      submit("entityMember", "setUseCustomLevels", [shopId, useCustom]),

    manualUpgrade: (shopId: number, member: string, targetLevelId: number) =>
      submit("entityMember", "manualSetMemberLevel", [shopId, member, targetLevelId]),
    setUpgradeMode: (shopId: number, mode: string) =>
      submit("entityMember", "setUpgradeMode", [shopId, mode]),

    initUpgradeRuleSystem: (shopId: number, conflictStrategy: string) =>
      submit("entityMember", "initUpgradeRuleSystem", [shopId, conflictStrategy]),
    addUpgradeRule: (shopId: number, name: string, trigger: Record<string, unknown>, targetLevelId: number, duration: number | null, priority: number, stackable: boolean, maxTriggers: number | null) =>
      submit("entityMember", "addUpgradeRule", [shopId, name, trigger, targetLevelId, duration, priority, stackable, maxTriggers]),
    updateUpgradeRule: (shopId: number, ruleId: number, enabled: boolean | null, priority: number | null) =>
      submit("entityMember", "updateUpgradeRule", [shopId, ruleId, enabled, priority]),
    removeUpgradeRule: (shopId: number, ruleId: number) =>
      submit("entityMember", "removeUpgradeRule", [shopId, ruleId]),
    setUpgradeRuleSystemEnabled: (shopId: number, enabled: boolean) =>
      submit("entityMember", "setUpgradeRuleSystemEnabled", [shopId, enabled]),
    setConflictStrategy: (shopId: number, strategy: string) =>
      submit("entityMember", "setConflictStrategy", [shopId, strategy]),
    resetUpgradeRuleSystem: (shopId: number) =>
      submit("entityMember", "resetUpgradeRuleSystem", [shopId]),

    setMemberPolicy: (shopId: number, policyBits: number) =>
      submit("entityMember", "setMemberPolicy", [shopId, policyBits]),
    setMemberStatsPolicy: (shopId: number, policyBits: number) =>
      submit("entityMember", "setMemberStatsPolicy", [shopId, policyBits]),

    approveMember: (shopId: number, account: string) =>
      submit("entityMember", "approveMember", [shopId, account]),
    rejectMember: (shopId: number, account: string) =>
      submit("entityMember", "rejectMember", [shopId, account]),
    batchApproveMembers: (shopId: number, accounts: string[]) =>
      submit("entityMember", "batchApproveMembers", [shopId, accounts]),
    batchRejectMembers: (shopId: number, accounts: string[]) =>
      submit("entityMember", "batchRejectMembers", [shopId, accounts]),

    banMember: (shopId: number, account: string, reason: string | null) =>
      submit("entityMember", "banMember", [shopId, account, reason]),
    unbanMember: (shopId: number, account: string) =>
      submit("entityMember", "unbanMember", [shopId, account]),
    removeMember: (shopId: number, account: string) =>
      submit("entityMember", "removeMember", [shopId, account]),
    activateMember: (shopId: number, account: string) =>
      submit("entityMember", "activateMember", [shopId, account]),
    deactivateMember: (shopId: number, account: string) =>
      submit("entityMember", "deactivateMember", [shopId, account]),

    cancelPendingMember: (shopId: number) =>
      submit("entityMember", "cancelPendingMember", [shopId]),
    cleanupExpiredPending: (entityId: number, maxClean: number) =>
      submit("entityMember", "cleanupExpiredPending", [entityId, maxClean]),

    leaveEntity: (entityId: number) =>
      submit("entityMember", "leaveEntity", [entityId]),

    txState: state,
    resetTx: reset,
  };
}
