"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { DisclosureData, AnnouncementData, DisclosureConfigData, InsiderRecord } from "@/lib/types";

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
          if (raw.isNone) return null;
          return { id, ...raw.toJSON() } as DisclosureData;
        })
      );
      setDisclosures(results.filter(Boolean) as DisclosureData[]);
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
          if (raw.isNone) return null;
          return { id, ...raw.toJSON() } as AnnouncementData;
        })
      );
      setAnnouncements(results.filter(Boolean) as AnnouncementData[]);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { announcements, isLoading, refetch: fetch };
}

export function useDisclosureConfig(entityId: number | null) {
  const [config, setConfig] = useState<DisclosureConfigData | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).entityDisclosure.disclosureConfigs(entityId);
      if (raw && !raw.isNone) {
        setConfig(raw.toJSON() as unknown as DisclosureConfigData);
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { config, isLoading, refetch: fetch };
}

export function useInsiders(entityId: number | null) {
  const [insiders, setInsiders] = useState<InsiderRecord[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).entityDisclosure.insiders(entityId);
      if (raw && !raw.isNone) {
        setInsiders((raw.toJSON() || []) as InsiderRecord[]);
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { insiders, isLoading, refetch: fetch };
}

export function useBlackout(entityId: number | null) {
  const [blackout, setBlackout] = useState<{ start: number; end: number } | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).entityDisclosure.blackoutPeriods(entityId);
      if (raw && !raw.isNone) {
        const data = raw.toJSON() as [number, number];
        setBlackout({ start: data[0], end: data[1] });
      } else {
        setBlackout(null);
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { blackout, isLoading, refetch: fetch };
}

export function useDisclosureActions() {
  const { submit, state, reset } = useTx();
  return {
    configureDisclosure: (entityId: number, level: string, insiderTradingControl: boolean, blackoutAfter: number) =>
      submit("entityDisclosure", "configureDisclosure", [entityId, level, insiderTradingControl, blackoutAfter]),
    publishDisclosure: (entityId: number, disclosureType: string, contentCid: string, summaryCid: string | null) =>
      submit("entityDisclosure", "publishDisclosure", [entityId, disclosureType, contentCid, summaryCid]),
    withdrawDisclosure: (disclosureId: number) =>
      submit("entityDisclosure", "withdrawDisclosure", [disclosureId]),
    correctDisclosure: (oldDisclosureId: number, contentCid: string, summaryCid: string | null) =>
      submit("entityDisclosure", "correctDisclosure", [oldDisclosureId, contentCid, summaryCid]),

    createDraftDisclosure: (entityId: number, disclosureType: string, contentCid: string, summaryCid: string | null) =>
      submit("entityDisclosure", "createDraftDisclosure", [entityId, disclosureType, contentCid, summaryCid]),
    updateDraft: (disclosureId: number, contentCid: string, summaryCid: string | null) =>
      submit("entityDisclosure", "updateDraft", [disclosureId, contentCid, summaryCid]),
    deleteDraft: (disclosureId: number) =>
      submit("entityDisclosure", "deleteDraft", [disclosureId]),
    publishDraft: (disclosureId: number) =>
      submit("entityDisclosure", "publishDraft", [disclosureId]),

    addInsider: (entityId: number, account: string, role: string) =>
      submit("entityDisclosure", "addInsider", [entityId, account, role]),
    removeInsider: (entityId: number, account: string) =>
      submit("entityDisclosure", "removeInsider", [entityId, account]),
    updateInsiderRole: (entityId: number, account: string, newRole: string) =>
      submit("entityDisclosure", "updateInsiderRole", [entityId, account, newRole]),
    batchAddInsiders: (entityId: number, insiders: Array<[string, string]>) =>
      submit("entityDisclosure", "batchAddInsiders", [entityId, insiders]),
    batchRemoveInsiders: (entityId: number, accounts: string[]) =>
      submit("entityDisclosure", "batchRemoveInsiders", [entityId, accounts]),

    startBlackout: (entityId: number, duration: number) =>
      submit("entityDisclosure", "startBlackout", [entityId, duration]),
    endBlackout: (entityId: number) =>
      submit("entityDisclosure", "endBlackout", [entityId]),
    expireBlackout: (entityId: number) =>
      submit("entityDisclosure", "expireBlackout", [entityId]),

    reportViolation: (entityId: number, violationType: string) =>
      submit("entityDisclosure", "reportDisclosureViolation", [entityId, violationType]),
    resetViolationCount: (entityId: number) =>
      submit("entityDisclosure", "resetViolationCount", [entityId]),

    publishAnnouncement: (entityId: number, category: string, title: string, contentCid: string, expiresAt: number | null) =>
      submit("entityDisclosure", "publishAnnouncement", [entityId, category, title, contentCid, expiresAt]),
    updateAnnouncement: (announcementId: number, title: string | null, contentCid: string | null, category: string | null, expiresAt: number | null) =>
      submit("entityDisclosure", "updateAnnouncement", [announcementId, title, contentCid, category, expiresAt]),
    withdrawAnnouncement: (announcementId: number) =>
      submit("entityDisclosure", "withdrawAnnouncement", [announcementId]),
    pinAnnouncement: (entityId: number, announcementId: number) =>
      submit("entityDisclosure", "pinAnnouncement", [entityId, announcementId]),
    unpinAnnouncement: (entityId: number, announcementId: number) =>
      submit("entityDisclosure", "unpinAnnouncement", [entityId, announcementId]),
    expireAnnouncement: (announcementId: number) =>
      submit("entityDisclosure", "expireAnnouncement", [announcementId]),

    txState: state,
    resetTx: reset,
  };
}
