"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { SubscriptionRecord, AdCommitmentRecord, TierFeatureGate } from "@/lib/types";

export function useSubscriptions() {
  const [subscriptions, setSubscriptions] = useState<SubscriptionRecord[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).groupRobotSubscription.subscriptions.entries();
      const all = entries.map(
        ([k, v]: [{ args: [{ toHex: () => string }] }, { toJSON: () => SubscriptionRecord }]) => {
          const s = v.toJSON();
          if (s) s.botIdHash = k.args[0].toHex();
          return s;
        }
      ).filter(Boolean) as SubscriptionRecord[];
      setSubscriptions(all);
    } catch { /* ignore */ }
    finally { setIsLoading(false); }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { subscriptions, isLoading, refetch: fetch };
}

export function useSubscription(botIdHash: string | null) {
  const [subscription, setSubscription] = useState<SubscriptionRecord | null>(null);
  const [escrow, setEscrow] = useState<bigint>(BigInt(0));
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!botIdHash) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const [subRaw, escrowRaw] = await Promise.all([
        q.groupRobotSubscription.subscriptions(botIdHash),
        q.groupRobotSubscription.subscriptionEscrow(botIdHash),
      ]);
      setSubscription(subRaw.isNone ? null : (subRaw.toJSON() as SubscriptionRecord));
      setEscrow(BigInt(escrowRaw.toString()));
    } catch { /* ignore */ }
    finally { setIsLoading(false); }
  }, [botIdHash]);

  useEffect(() => { fetch(); }, [fetch]);
  return { subscription, escrow, isLoading, refetch: fetch };
}

export function useAdCommitments() {
  const [commitments, setCommitments] = useState<AdCommitmentRecord[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).groupRobotSubscription.adCommitments.entries();
      const all = entries.map(
        ([k, v]: [{ args: [{ toHex: () => string }] }, { toJSON: () => AdCommitmentRecord }]) => {
          const c = v.toJSON();
          if (c) c.botIdHash = k.args[0].toHex();
          return c;
        }
      ).filter(Boolean) as AdCommitmentRecord[];
      setCommitments(all);
    } catch { /* ignore */ }
    finally { setIsLoading(false); }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { commitments, isLoading, refetch: fetch };
}

export function useTierFeatureGate(tier: string | null) {
  const [gate, setGate] = useState<TierFeatureGate | null>(null);

  const fetch = useCallback(async () => {
    if (!tier) return;
    try {
      const api = await getApi();
      const raw = await (api.query as any).groupRobotSubscription.tierFeatureGateOverrides(tier);
      setGate(raw.isNone ? null : (raw.toJSON() as TierFeatureGate));
    } catch { /* ignore */ }
  }, [tier]);

  useEffect(() => { fetch(); }, [fetch]);
  return { gate, refetch: fetch };
}

export function useSubscriptionActions() {
  const { submit, state, reset } = useTx();
  return {
    subscribe: (botIdHash: string, tier: string, deposit: bigint) =>
      submit("groupRobotSubscription", "subscribe", [botIdHash, tier, deposit]),
    depositSubscription: (botIdHash: string, amount: bigint) =>
      submit("groupRobotSubscription", "depositSubscription", [botIdHash, amount]),
    cancelSubscription: (botIdHash: string) =>
      submit("groupRobotSubscription", "cancelSubscription", [botIdHash]),
    changeTier: (botIdHash: string, newTier: string) =>
      submit("groupRobotSubscription", "changeTier", [botIdHash, newTier]),
    pauseSubscription: (botIdHash: string) =>
      submit("groupRobotSubscription", "pauseSubscription", [botIdHash]),
    resumeSubscription: (botIdHash: string) =>
      submit("groupRobotSubscription", "resumeSubscription", [botIdHash]),
    withdrawEscrow: (botIdHash: string, amount: bigint) =>
      submit("groupRobotSubscription", "withdrawEscrow", [botIdHash, amount]),

    commitAds: (botIdHash: string, communityIdHash: string, committedAdsPerEra: number) =>
      submit("groupRobotSubscription", "commitAds", [botIdHash, communityIdHash, committedAdsPerEra]),
    cancelAdCommitment: (botIdHash: string) =>
      submit("groupRobotSubscription", "cancelAdCommitment", [botIdHash]),
    updateAdCommitment: (botIdHash: string, newCommittedAdsPerEra: number, newCommunityIdHash: string | null) =>
      submit("groupRobotSubscription", "updateAdCommitment", [botIdHash, newCommittedAdsPerEra, newCommunityIdHash]),

    // Admin
    updateTierFeatureGate: (tier: string, gate: TierFeatureGate) =>
      submit("groupRobotSubscription", "updateTierFeatureGate", [tier, gate]),
    resetTierFeatureGate: (tier: string) =>
      submit("groupRobotSubscription", "resetTierFeatureGate", [tier]),
    updateTierFee: (tier: string, newFee: bigint) =>
      submit("groupRobotSubscription", "updateTierFee", [tier, newFee]),
    forceCancelSubscription: (botIdHash: string) =>
      submit("groupRobotSubscription", "forceCancelSubscription", [botIdHash]),
    forceSuspendSubscription: (botIdHash: string) =>
      submit("groupRobotSubscription", "forceSuspendSubscription", [botIdHash]),
    forceChangeTier: (botIdHash: string, newTier: string) =>
      submit("groupRobotSubscription", "forceChangeTier", [botIdHash, newTier]),

    txState: state,
    resetTx: reset,
  };
}
