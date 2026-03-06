"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { AdCampaign, AdPlacement } from "@/lib/types";

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
            return raw.isNone ? null : (raw.toJSON() as AdCampaign);
          })
        );

        setCampaigns(results.filter(Boolean) as AdCampaign[]);
      } else {
        const entries = await q.adsCore.campaigns.entries();
        const all = entries
          .map(([_k, v]: [unknown, { toJSON: () => AdCampaign }]) => v.toJSON())
          .filter(Boolean) as AdCampaign[];
        setCampaigns(all);
      }
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [advertiser]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { campaigns, isLoading, refetch: fetch };
}

export function useCampaignActions() {
  const { submit, state, reset } = useTx();
  return {
    createCampaign: (
      text: string,
      url: string,
      bidPerMille: bigint,
      dailyBudget: bigint,
      totalBudget: bigint,
      deliveryTypes: number,
      expiresInBlocks: number
    ) =>
      submit("adsCore", "createCampaign", [
        text,
        url,
        bidPerMille,
        dailyBudget,
        totalBudget,
        deliveryTypes,
        expiresInBlocks,
      ]),
    fundCampaign: (campaignId: number, amount: bigint) =>
      submit("adsCore", "fundCampaign", [campaignId, amount]),
    pauseCampaign: (campaignId: number) =>
      submit("adsCore", "pauseCampaign", [campaignId]),
    resumeCampaign: (campaignId: number) =>
      submit("adsCore", "resumeCampaign", [campaignId]),
    cancelCampaign: (campaignId: number) =>
      submit("adsCore", "cancelCampaign", [campaignId]),
    claimAdRevenue: (placementId: string, amount: bigint) =>
      submit("adsCore", "claimAdRevenue", [placementId, amount]),
    txState: state,
    resetTx: reset,
  };
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
            return raw.isNone ? null : (raw.toJSON() as AdPlacement);
          })
        );

        setPlacements(results.filter(Boolean) as AdPlacement[]);
      } else {
        const entries = await q.adsEntity.registeredPlacements.entries();
        const all = entries
          .map(([_k, v]: [unknown, { toJSON: () => AdPlacement }]) => v.toJSON())
          .filter(Boolean) as AdPlacement[];
        setPlacements(all);
      }
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [entityId]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { placements, isLoading, refetch: fetch };
}
