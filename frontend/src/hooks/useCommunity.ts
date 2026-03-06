"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { CommunityConfig, CommunityBinding, ReputationRecord } from "@/lib/types";

export function useCommunityBindings() {
  const [bindings, setBindings] = useState<CommunityBinding[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).groupRobotRegistry.communityBindings.entries();
      const all = entries
        .map(([k, v]: [{ args: [{ toHex: () => string }] }, { toJSON: () => CommunityBinding }]) => {
          const b = v.toJSON();
          if (b) b.communityIdHash = k.args[0].toHex();
          return b;
        })
        .filter(Boolean) as CommunityBinding[];
      setBindings(all);
    } catch { /* ignore */ }
    finally { setIsLoading(false); }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { bindings, isLoading, refetch: fetch };
}

export function useCommunityConfig(communityIdHash: string | null) {
  const [config, setConfig] = useState<CommunityConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!communityIdHash) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).groupRobotCommunity.communityConfigs(communityIdHash);
      setConfig(raw.isNone ? null : (raw.toJSON() as CommunityConfig));
    } catch { /* ignore */ }
    finally { setIsLoading(false); }
  }, [communityIdHash]);

  useEffect(() => { fetch(); }, [fetch]);
  return { config, isLoading, refetch: fetch };
}

export function useCommunityConfigs() {
  const [configs, setConfigs] = useState<(CommunityConfig & { communityIdHash: string })[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).groupRobotCommunity.communityConfigs.entries();
      const all = entries.map(
        ([k, v]: [{ args: [{ toHex: () => string }] }, { toJSON: () => CommunityConfig }]) => {
          const c = v.toJSON();
          return c ? { ...c, communityIdHash: k.args[0].toHex() } : null;
        }
      ).filter(Boolean) as (CommunityConfig & { communityIdHash: string })[];
      setConfigs(all);
    } catch { /* ignore */ }
    finally { setIsLoading(false); }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { configs, isLoading, refetch: fetch };
}

export function useReputation(communityIdHash: string | null, userHash: string | null) {
  const [record, setRecord] = useState<ReputationRecord | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!communityIdHash || !userHash) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).groupRobotCommunity.memberReputation(communityIdHash, userHash);
      setRecord(raw.isNone ? null : (raw.toJSON() as ReputationRecord));
    } catch { /* ignore */ }
    finally { setIsLoading(false); }
  }, [communityIdHash, userHash]);

  useEffect(() => { fetch(); }, [fetch]);
  return { record, isLoading, refetch: fetch };
}

export function useCommunityActions() {
  const { submit, state, reset } = useTx();
  return {
    setNodeRequirement: (communityIdHash: string, requirement: string) =>
      submit("groupRobotCommunity", "setNodeRequirement", [communityIdHash, requirement]),
    updateCommunityConfig: (communityIdHash: string, expectedVersion: number, antiFloodEnabled: boolean, floodLimit: number, warnLimit: number, warnAction: string, welcomeEnabled: boolean, adsEnabled: boolean, language: string) =>
      submit("groupRobotCommunity", "updateCommunityConfig", [communityIdHash, expectedVersion, antiFloodEnabled, floodLimit, warnLimit, warnAction, welcomeEnabled, adsEnabled, language]),
    updateActiveMembers: (communityIdHash: string, activeMembers: number) =>
      submit("groupRobotCommunity", "updateActiveMembers", [communityIdHash, activeMembers]),
    awardReputation: (communityIdHash: string, userHash: string, delta: number) =>
      submit("groupRobotCommunity", "awardReputation", [communityIdHash, userHash, delta]),
    deductReputation: (communityIdHash: string, userHash: string, delta: number) =>
      submit("groupRobotCommunity", "deductReputation", [communityIdHash, userHash, delta]),
    resetReputation: (communityIdHash: string, userHash: string) =>
      submit("groupRobotCommunity", "resetReputation", [communityIdHash, userHash]),
    clearExpiredLogs: (communityIdHash: string, maxAgeBlocks: number) =>
      submit("groupRobotCommunity", "clearExpiredLogs", [communityIdHash, maxAgeBlocks]),
    deleteCommunityConfig: (communityIdHash: string) =>
      submit("groupRobotCommunity", "deleteCommunityConfig", [communityIdHash]),
    banCommunity: (communityIdHash: string) =>
      submit("groupRobotCommunity", "banCommunity", [communityIdHash]),
    unbanCommunity: (communityIdHash: string) =>
      submit("groupRobotCommunity", "unbanCommunity", [communityIdHash]),
    forceRemoveCommunity: (communityIdHash: string) =>
      submit("groupRobotCommunity", "forceRemoveCommunity", [communityIdHash]),
    txState: state,
    resetTx: reset,
  };
}
