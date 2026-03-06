"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { EraRewardInfo, NodeRewardSummary } from "@/lib/types";

export function useNodeRewards(nodeId: string | null) {
  const [summary, setSummary] = useState<NodeRewardSummary | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!nodeId) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const [pendingRaw, totalRaw] = await Promise.all([
        q.groupRobotRewards.nodePendingRewards(nodeId),
        q.groupRobotRewards.nodeTotalEarned(nodeId),
      ]);
      setSummary({
        pending: BigInt(pendingRaw.toString()),
        totalEarned: BigInt(totalRaw.toString()),
      });
    } catch { /* ignore */ }
    finally { setIsLoading(false); }
  }, [nodeId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { summary, isLoading, refetch: fetch };
}

export function useAllNodeRewards(nodeIds: string[]) {
  const [rewards, setRewards] = useState<{ nodeId: string; pending: bigint; totalEarned: bigint }[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (nodeIds.length === 0) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const results = await Promise.all(
        nodeIds.map(async (nodeId) => {
          const [pRaw, tRaw] = await Promise.all([
            q.groupRobotRewards.nodePendingRewards(nodeId),
            q.groupRobotRewards.nodeTotalEarned(nodeId),
          ]);
          return {
            nodeId,
            pending: BigInt(pRaw.toString()),
            totalEarned: BigInt(tRaw.toString()),
          };
        })
      );
      setRewards(results);
    } catch { /* ignore */ }
    finally { setIsLoading(false); }
  }, [nodeIds]);

  useEffect(() => { fetch(); }, [fetch]);
  return { rewards, isLoading, refetch: fetch };
}

export function useEraRewards(era: number | null) {
  const [info, setInfo] = useState<EraRewardInfo | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (era === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).groupRobotRewards.eraRewards(era);
      setInfo(raw.isNone ? null : (raw.toJSON() as EraRewardInfo));
    } catch { /* ignore */ }
    finally { setIsLoading(false); }
  }, [era]);

  useEffect(() => { fetch(); }, [fetch]);
  return { info, isLoading, refetch: fetch };
}

export function useDistributionPaused() {
  const [paused, setPaused] = useState(false);
  const fetch = useCallback(async () => {
    try {
      const api = await getApi();
      const raw = await (api.query as any).groupRobotRewards.distributionPaused();
      setPaused((raw.toJSON() ?? false) as boolean);
    } catch { /* ignore */ }
  }, []);
  useEffect(() => { fetch(); }, [fetch]);
  return { paused, refetch: fetch };
}

export function useOwnerRewards(botIdHash: string | null) {
  const [pending, setPending] = useState<bigint>(BigInt(0));
  const [totalEarned, setTotalEarned] = useState<bigint>(BigInt(0));
  const [splitBps, setSplitBps] = useState(0);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!botIdHash) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const [pRaw, tRaw, sRaw] = await Promise.all([
        q.groupRobotRewards.ownerPendingRewards(botIdHash),
        q.groupRobotRewards.ownerTotalEarned(botIdHash),
        q.groupRobotRewards.rewardSplitBps(botIdHash),
      ]);
      setPending(BigInt(pRaw.toString()));
      setTotalEarned(BigInt(tRaw.toString()));
      setSplitBps((sRaw.toJSON() ?? 0) as number);
    } catch { /* ignore */ }
    finally { setIsLoading(false); }
  }, [botIdHash]);

  useEffect(() => { fetch(); }, [fetch]);
  return { pending, totalEarned, splitBps, isLoading, refetch: fetch };
}

export function useRewardActions() {
  const { submit, state, reset } = useTx();
  return {
    claimRewards: (nodeId: string) =>
      submit("groupRobotRewards", "claimRewards", [nodeId]),
    batchClaimRewards: (nodeIds: string[]) =>
      submit("groupRobotRewards", "batchClaimRewards", [nodeIds]),
    setRewardRecipient: (nodeId: string, recipient: string | null) =>
      submit("groupRobotRewards", "setRewardRecipient", [nodeId, recipient]),
    setRewardSplit: (botIdHash: string, ownerBps: number) =>
      submit("groupRobotRewards", "setRewardSplit", [botIdHash, ownerBps]),
    claimOwnerRewards: (botIdHash: string) =>
      submit("groupRobotRewards", "claimOwnerRewards", [botIdHash]),
    rescueStrandedRewards: (nodeId: string, recipient: string) =>
      submit("groupRobotRewards", "rescueStrandedRewards", [nodeId, recipient]),

    // Admin
    pauseDistribution: () =>
      submit("groupRobotRewards", "pauseDistribution", []),
    resumeDistribution: () =>
      submit("groupRobotRewards", "resumeDistribution", []),
    forceSlashPendingRewards: (nodeId: string, amount: bigint) =>
      submit("groupRobotRewards", "forceSlashPendingRewards", [nodeId, amount]),
    forceSetPendingRewards: (nodeId: string, amount: bigint) =>
      submit("groupRobotRewards", "forceSetPendingRewards", [nodeId, amount]),

    txState: state,
    resetTx: reset,
  };
}
