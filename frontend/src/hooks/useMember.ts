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
      const results = entries.map(([_k, v]: [unknown, { toJSON: () => MemberData }]) => v.toJSON());
      setMembers(results);
      setCount(results.length);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { members, count, isLoading, refetch: fetch };
}

export function useLevels(shopId: number | null) {
  const [levels, setLevels] = useState<LevelData[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (shopId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityMember.customLevels.entries(shopId);
      const results = entries.map(([_k, v]: [unknown, { toJSON: () => LevelData }]) => v.toJSON());
      setLevels(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [shopId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { levels, isLoading, refetch: fetch };
}

export function useUpgradeRules(shopId: number | null) {
  const [rules, setRules] = useState<UpgradeRule[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (shopId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityMember.upgradeRules.entries(shopId);
      const results = entries.map(([_k, v]: [unknown, { toJSON: () => UpgradeRule }]) => v.toJSON());
      setRules(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [shopId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { rules, isLoading, refetch: fetch };
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
    updateCustomLevel: (shopId: number, levelId: number, threshold?: number, discountRate?: number, commissionBonus?: number) =>
      submit("entityMember", "updateCustomLevel", [shopId, levelId, threshold, discountRate, commissionBonus]),
    removeCustomLevel: (shopId: number, levelId: number) =>
      submit("entityMember", "removeCustomLevel", [shopId, levelId]),
    manualUpgrade: (shopId: number, member: string, targetLevelId: number) =>
      submit("entityMember", "manualUpgradeMember", [shopId, member, targetLevelId]),
    setUpgradeMode: (shopId: number, mode: string) =>
      submit("entityMember", "setUpgradeMode", [shopId, mode]),
    setMemberPolicy: (shopId: number, policyBits: number) =>
      submit("entityMember", "setMemberPolicy", [shopId, policyBits]),
    approveMember: (shopId: number, account: string) =>
      submit("entityMember", "approveMember", [shopId, account]),
    rejectMember: (shopId: number, account: string) =>
      submit("entityMember", "rejectMember", [shopId, account]),
    addUpgradeRule: (shopId: number, trigger: string, targetLevelId: number, threshold: bigint, priority: number, stackable: boolean, maxTriggers: number) =>
      submit("entityMember", "addUpgradeRule", [shopId, trigger, targetLevelId, threshold, priority, stackable, maxTriggers]),
    updateUpgradeRule: (shopId: number, ruleId: number, enabled: boolean, priority: number) =>
      submit("entityMember", "updateUpgradeRule", [shopId, ruleId, enabled, priority]),
    removeUpgradeRule: (shopId: number, ruleId: number) =>
      submit("entityMember", "removeUpgradeRule", [shopId, ruleId]),
    txState: state,
    resetTx: reset,
  };
}
