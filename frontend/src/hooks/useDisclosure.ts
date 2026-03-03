"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { DisclosureData, AnnouncementData } from "@/lib/types";

export function useDisclosures(entityId: number | null) {
  const [disclosures, setDisclosures] = useState<DisclosureData[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const ids = await (api.query as any).entityDisclosure.entityDisclosures(entityId);
      const idList = (ids.toJSON() || []) as number[];
      const results = await Promise.all(
        idList.map(async (id: number) => {
          const raw = await (api.query as any).entityDisclosure.disclosures(id);
          return raw.toJSON() as DisclosureData;
        })
      );
      setDisclosures(results.filter(Boolean));
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { disclosures, isLoading, refetch: fetch };
}

export function useAnnouncements(entityId: number | null) {
  const [announcements, setAnnouncements] = useState<AnnouncementData[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const ids = await (api.query as any).entityDisclosure.entityAnnouncements(entityId);
      const idList = (ids.toJSON() || []) as number[];
      const results = await Promise.all(
        idList.map(async (id: number) => {
          const raw = await (api.query as any).entityDisclosure.announcements(id);
          return raw.toJSON() as AnnouncementData;
        })
      );
      setAnnouncements(results.filter(Boolean));
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { announcements, isLoading, refetch: fetch };
}

export function useDisclosureActions() {
  const { submit, state, reset } = useTx();
  return {
    configureDisclosure: (entityId: number, required: boolean, frequency: string) =>
      submit("entityDisclosure", "configureDisclosure", [entityId, required, frequency]),
    publishDisclosure: (entityId: number, disclosureType: string, contentCid: string, materiality: string) =>
      submit("entityDisclosure", "publishDisclosure", [entityId, disclosureType, contentCid, materiality]),
    withdrawDisclosure: (entityId: number, disclosureId: number) =>
      submit("entityDisclosure", "withdrawDisclosure", [entityId, disclosureId]),
    correctDisclosure: (entityId: number, disclosureId: number, contentCid: string) =>
      submit("entityDisclosure", "correctDisclosure", [entityId, disclosureId, contentCid]),
    addInsider: (entityId: number, account: string, role: string) =>
      submit("entityDisclosure", "addInsider", [entityId, account, role]),
    removeInsider: (entityId: number, account: string) =>
      submit("entityDisclosure", "removeInsider", [entityId, account]),
    startBlackout: (entityId: number, endBlock: number) =>
      submit("entityDisclosure", "startBlackout", [entityId, endBlock]),
    endBlackout: (entityId: number) =>
      submit("entityDisclosure", "endBlackout", [entityId]),
    publishAnnouncement: (entityId: number, title: string, contentCid: string, category: string, expiresAt: number | null) =>
      submit("entityDisclosure", "publishAnnouncement", [entityId, title, contentCid, category, expiresAt]),
    updateAnnouncement: (entityId: number, announcementId: number, title?: string, contentCid?: string, category?: string, expiresAt?: number | null) =>
      submit("entityDisclosure", "updateAnnouncement", [entityId, announcementId, title, contentCid, category, expiresAt]),
    withdrawAnnouncement: (entityId: number, announcementId: number) =>
      submit("entityDisclosure", "withdrawAnnouncement", [entityId, announcementId]),
    pinAnnouncement: (entityId: number, announcementId: number | null) =>
      submit("entityDisclosure", "pinAnnouncement", [entityId, announcementId]),
    txState: state,
    resetTx: reset,
  };
}
