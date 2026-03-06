"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { AdCampaign, AdPlacement, CommunityAdStake } from "@/lib/types";

export function useCampaigns(advertiser?: string | null) {
  const [campaigns, setCampaigns] = useState<AdCampaign[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;

      if (advertiser) {
        const idsRaw = await q.adsCore.campaignsByAdvertiser(advertiser);
        const idList = (idsRaw.toJSON() || []) as number[];
        const results = await Promise.all(
          idList.map(async (id: number) => {
            const raw = await q.adsCore.campaigns(id);
            if (raw.isNone) return null;
            const c = raw.toJSON() as AdCampaign;
            c.id = id;
            return c;
          })
        );
        setCampaigns(results.filter(Boolean) as AdCampaign[]);
      } else {
        const entries = await q.adsCore.campaigns.entries();
        const all = entries
          .map(([k, v]: [{ args: [{ toNumber: () => number }] }, { toJSON: () => AdCampaign }]) => {
            const c = v.toJSON();
            if (c) c.id = k.args[0].toNumber();
            return c;
          })
          .filter(Boolean) as AdCampaign[];
        setCampaigns(all);
      }
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [advertiser]);

  useEffect(() => { fetch(); }, [fetch]);
  return { campaigns, isLoading, refetch: fetch };
}

export function useCampaign(campaignId: number | null) {
  const [campaign, setCampaign] = useState<AdCampaign | null>(null);
  const [escrow, setEscrow] = useState<bigint>(BigInt(0));
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (campaignId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const raw = await q.adsCore.campaigns(campaignId);
      if (raw.isNone) { setCampaign(null); return; }
      const c = raw.toJSON() as AdCampaign;
      c.id = campaignId;
      setCampaign(c);
      const escrowRaw = await q.adsCore.campaignEscrow(campaignId);
      setEscrow(BigInt(escrowRaw.toString()));
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [campaignId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { campaign, escrow, isLoading, refetch: fetch };
}

export function usePlacements(entityId?: number | null) {
  const [placements, setPlacements] = useState<AdPlacement[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;

      if (entityId !== undefined && entityId !== null) {
        const idsRaw = await q.adsEntity.entityPlacementIds(entityId);
        const placementIds = (idsRaw.toJSON() || []) as string[];
        const results = await Promise.all(
          placementIds.map(async (pid: string) => {
            const raw = await q.adsEntity.registeredPlacements(pid);
            if (raw.isNone) return null;
            const p = raw.toJSON() as AdPlacement;
            p.placementId = pid;
            return p;
          })
        );
        setPlacements(results.filter(Boolean) as AdPlacement[]);
      } else {
        const entries = await q.adsEntity.registeredPlacements.entries();
        const all = entries
          .map(([k, v]: [{ args: [{ toHex: () => string }] }, { toJSON: () => AdPlacement }]) => {
            const p = v.toJSON();
            if (p) p.placementId = k.args[0].toHex();
            return p;
          })
          .filter(Boolean) as AdPlacement[];
        setPlacements(all);
      }
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { placements, isLoading, refetch: fetch };
}

export function usePlacementRevenue(placementId: string | null) {
  const [total, setTotal] = useState<bigint>(BigInt(0));
  const [claimable, setClaimable] = useState<bigint>(BigInt(0));
  const [eraRevenue, setEraRevenue] = useState<bigint>(BigInt(0));
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!placementId) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const [tRaw, cRaw, eRaw] = await Promise.all([
        q.adsCore.placementTotalRevenue(placementId),
        q.adsCore.placementClaimable(placementId),
        q.adsCore.eraAdRevenue(placementId),
      ]);
      setTotal(BigInt(tRaw.toString()));
      setClaimable(BigInt(cRaw.toString()));
      setEraRevenue(BigInt(eRaw.toString()));
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [placementId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { total, claimable, eraRevenue, isLoading, refetch: fetch };
}

export function useCampaignActions() {
  const { submit, state, reset } = useTx();
  return {
    createCampaign: (text: string, url: string, bidPerMille: bigint, dailyBudget: bigint, totalBudget: bigint, deliveryTypes: number, expiresAt: number, targets: string[] | null, campaignType: string, bidPerClick: bigint) =>
      submit("adsCore", "createCampaign", [text, url, bidPerMille, dailyBudget, totalBudget, deliveryTypes, expiresAt, targets, campaignType, bidPerClick]),
    updateCampaign: (campaignId: number, text: string | null, url: string | null, bidPerMille: bigint | null, dailyBudget: bigint | null, deliveryTypes: number | null, bidPerClick: bigint | null) =>
      submit("adsCore", "updateCampaign", [campaignId, text, url, bidPerMille, dailyBudget, deliveryTypes, bidPerClick]),
    fundCampaign: (campaignId: number, amount: bigint) =>
      submit("adsCore", "fundCampaign", [campaignId, amount]),
    pauseCampaign: (campaignId: number) =>
      submit("adsCore", "pauseCampaign", [campaignId]),
    resumeCampaign: (campaignId: number) =>
      submit("adsCore", "resumeCampaign", [campaignId]),
    cancelCampaign: (campaignId: number) =>
      submit("adsCore", "cancelCampaign", [campaignId]),
    expireCampaign: (campaignId: number) =>
      submit("adsCore", "expireCampaign", [campaignId]),
    resubmitCampaign: (campaignId: number, text: string, url: string, totalBudget: bigint) =>
      submit("adsCore", "resubmitCampaign", [campaignId, text, url, totalBudget]),
    extendCampaignExpiry: (campaignId: number, newExpiresAt: number) =>
      submit("adsCore", "extendCampaignExpiry", [campaignId, newExpiresAt]),
    setCampaignTargets: (campaignId: number, targets: string[]) =>
      submit("adsCore", "setCampaignTargets", [campaignId, targets]),
    clearCampaignTargets: (campaignId: number) =>
      submit("adsCore", "clearCampaignTargets", [campaignId]),
    setCampaignMultiplier: (campaignId: number, multiplierBps: number) =>
      submit("adsCore", "setCampaignMultiplier", [campaignId, multiplierBps]),

    // Review / Admin
    reviewCampaign: (campaignId: number, approved: boolean) =>
      submit("adsCore", "reviewCampaign", [campaignId, approved]),
    flagCampaign: (campaignId: number) =>
      submit("adsCore", "flagCampaign", [campaignId]),
    suspendCampaign: (campaignId: number) =>
      submit("adsCore", "suspendCampaign", [campaignId]),
    unsuspendCampaign: (campaignId: number) =>
      submit("adsCore", "unsuspendCampaign", [campaignId]),
    forceCancelCampaign: (campaignId: number) =>
      submit("adsCore", "forceCancelCampaign", [campaignId]),
    reportApprovedCampaign: (campaignId: number) =>
      submit("adsCore", "reportApprovedCampaign", [campaignId]),

    // Revenue
    claimAdRevenue: (placementId: string, amount: bigint) =>
      submit("adsCore", "claimAdRevenue", [placementId, amount]),
    settleEraAds: (placementId: string) =>
      submit("adsCore", "settleEraAds", [placementId]),
    forceSettleEraAds: (placementId: string) =>
      submit("adsCore", "forceSettleEraAds", [placementId]),

    // Delivery
    submitDeliveryReceipt: (campaignId: number, placementId: string, audienceSize: number) =>
      submit("adsCore", "submitDeliveryReceipt", [campaignId, placementId, audienceSize]),
    submitClickReceipt: (campaignId: number, placementId: string, clickCount: number, verifiedClicks: number) =>
      submit("adsCore", "submitClickReceipt", [campaignId, placementId, clickCount, verifiedClicks]),
    confirmReceipt: (campaignId: number, placementId: string, receiptIndex: number) =>
      submit("adsCore", "confirmReceipt", [campaignId, placementId, receiptIndex]),
    disputeReceipt: (campaignId: number, placementId: string, receiptIndex: number) =>
      submit("adsCore", "disputeReceipt", [campaignId, placementId, receiptIndex]),

    // Placement
    flagPlacement: (placementId: string) =>
      submit("adsCore", "flagPlacement", [placementId]),
    slashPlacement: (placementId: string, reporter: string) =>
      submit("adsCore", "slashPlacement", [placementId, reporter]),
    unbanPlacement: (placementId: string) =>
      submit("adsCore", "unbanPlacement", [placementId]),
    resetSlashCount: (placementId: string) =>
      submit("adsCore", "resetSlashCount", [placementId]),
    clearPlacementFlags: (placementId: string) =>
      submit("adsCore", "clearPlacementFlags", [placementId]),
    setPlacementMultiplier: (placementId: string, multiplierBps: number) =>
      submit("adsCore", "setPlacementMultiplier", [placementId, multiplierBps]),
    setPlacementApprovalRequired: (placementId: string, required: boolean) =>
      submit("adsCore", "setPlacementApprovalRequired", [placementId, required]),
    approveCampaignForPlacement: (placementId: string, campaignId: number) =>
      submit("adsCore", "approveCampaignForPlacement", [placementId, campaignId]),
    rejectCampaignForPlacement: (placementId: string, campaignId: number) =>
      submit("adsCore", "rejectCampaignForPlacement", [placementId, campaignId]),
    setPlacementDeliveryTypes: (placementId: string, deliveryTypes: number) =>
      submit("adsCore", "setPlacementDeliveryTypes", [placementId, deliveryTypes]),

    // Advertiser block/prefer
    advertiserBlockPlacement: (placementId: string) =>
      submit("adsCore", "advertiserBlockPlacement", [placementId]),
    advertiserUnblockPlacement: (placementId: string) =>
      submit("adsCore", "advertiserUnblockPlacement", [placementId]),
    advertiserPreferPlacement: (placementId: string) =>
      submit("adsCore", "advertiserPreferPlacement", [placementId]),
    advertiserUnpreferPlacement: (placementId: string) =>
      submit("adsCore", "advertiserUnpreferPlacement", [placementId]),
    placementBlockAdvertiser: (placementId: string, advertiser: string) =>
      submit("adsCore", "placementBlockAdvertiser", [placementId, advertiser]),
    placementUnblockAdvertiser: (placementId: string, advertiser: string) =>
      submit("adsCore", "placementUnblockAdvertiser", [placementId, advertiser]),

    // Referrer
    registerAdvertiser: (referrer: string) =>
      submit("adsCore", "registerAdvertiser", [referrer]),
    claimReferralEarnings: () =>
      submit("adsCore", "claimReferralEarnings", []),
    cleanupCampaign: (campaignId: number) =>
      submit("adsCore", "cleanupCampaign", [campaignId]),

    txState: state,
    resetTx: reset,
  };
}

export function usePlacementActions() {
  const { submit, state, reset } = useTx();
  return {
    registerEntityPlacement: (entityId: number) =>
      submit("adsEntity", "registerEntityPlacement", [entityId]),
    registerShopPlacement: (entityId: number, shopId: number) =>
      submit("adsEntity", "registerShopPlacement", [entityId, shopId]),
    deregisterPlacement: (placementId: string) =>
      submit("adsEntity", "deregisterPlacement", [placementId]),
    setPlacementActive: (placementId: string, active: boolean) =>
      submit("adsEntity", "setPlacementActive", [placementId, active]),
    setImpressionCap: (placementId: string, dailyCap: number) =>
      submit("adsEntity", "setImpressionCap", [placementId, dailyCap]),
    setClickCap: (placementId: string, dailyCap: number) =>
      submit("adsEntity", "setClickCap", [placementId, dailyCap]),
    setEntityAdShare: (entityId: number, shareBps: number) =>
      submit("adsEntity", "setEntityAdShare", [entityId, shareBps]),
    banEntity: (entityId: number) =>
      submit("adsEntity", "banEntity", [entityId]),
    unbanEntity: (entityId: number) =>
      submit("adsEntity", "unbanEntity", [entityId]),
    txState: state,
    resetTx: reset,
  };
}

export function useAdGrouprobotActions() {
  const { submit, state, reset } = useTx();
  return {
    stakeForAds: (communityIdHash: string, amount: bigint) =>
      submit("adsGrouprobot", "stakeForAds", [communityIdHash, amount]),
    unstakeForAds: (communityIdHash: string, amount: bigint) =>
      submit("adsGrouprobot", "unstakeForAds", [communityIdHash, amount]),
    withdrawUnbonded: (communityIdHash: string) =>
      submit("adsGrouprobot", "withdrawUnbonded", [communityIdHash]),
    claimStakerReward: (communityIdHash: string) =>
      submit("adsGrouprobot", "claimStakerReward", [communityIdHash]),
    setCommunityAdmin: (communityIdHash: string, newAdmin: string) =>
      submit("adsGrouprobot", "setCommunityAdmin", [communityIdHash, newAdmin]),
    resignCommunityAdmin: (communityIdHash: string) =>
      submit("adsGrouprobot", "resignCommunityAdmin", [communityIdHash]),
    adminPauseAds: (communityIdHash: string) =>
      submit("adsGrouprobot", "adminPauseAds", [communityIdHash]),
    adminResumeAds: (communityIdHash: string) =>
      submit("adsGrouprobot", "adminResumeAds", [communityIdHash]),
    setGlobalAdsPause: (paused: boolean) =>
      submit("adsGrouprobot", "setGlobalAdsPause", [paused]),
    setBotAdsEnabled: (communityIdHash: string, disabled: boolean) =>
      submit("adsGrouprobot", "setBotAdsEnabled", [communityIdHash, disabled]),
    setStakeTiers: (tiers: [bigint, number][]) =>
      submit("adsGrouprobot", "setStakeTiers", [tiers]),
    slashCommunity: (communityIdHash: string) =>
      submit("adsGrouprobot", "slashCommunity", [communityIdHash]),
    forceUnstake: (communityIdHash: string) =>
      submit("adsGrouprobot", "forceUnstake", [communityIdHash]),
    txState: state,
    resetTx: reset,
  };
}

export function useCommunityAdStakes() {
  const [stakes, setStakes] = useState<CommunityAdStake[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const entries = await q.adsGrouprobot.communityAdStake.entries();
      const all = entries.map(
        ([k, v]: [{ args: [{ toHex: () => string }] }, { toString: () => string }]) => ({
          communityIdHash: k.args[0].toHex(),
          totalStake: BigInt(v.toString()),
          stakerCount: 0,
          audienceCap: 0,
          adminPaused: false,
        } as CommunityAdStake)
      );
      setStakes(all);
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { stakes, isLoading, refetch: fetch };
}
