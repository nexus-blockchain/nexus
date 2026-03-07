'use client';

import { useEntityQuery } from './use-entity-query';
import { useEntityMutation } from './use-entity-mutation';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { STALE_TIMES } from '@/lib/chain/constants';
import { DisclosureLevel, DisclosureStatus, InsiderRole } from '@/lib/types/enums';

// ─── Interfaces ─────────────────────────────────────────────

interface DisclosureData {
  id: number;
  entityId: number;
  title: string;
  contentCid: string;
  level: DisclosureLevel;
  status: DisclosureStatus;
  createdAt: number;
}

interface InsiderData {
  account: string;
  role: InsiderRole;
  addedAt: number;
}

interface AnnouncementData {
  id: number;
  title: string;
  contentCid: string;
  category: string;
  pinned: boolean;
  expiresAt: number | null;
  createdAt: number;
}

// ─── Parsers ────────────────────────────────────────────────

function parseDisclosureEntries(rawEntries: [any, any][]): DisclosureData[] {
  if (!rawEntries || !Array.isArray(rawEntries)) return [];
  return rawEntries.map(([key, value]) => {
    const obj = value?.toJSON?.() ?? value;
    return {
      id: Number(key.args?.[1]?.toString() ?? obj.id ?? 0),
      entityId: Number(key.args?.[0]?.toString() ?? obj.entityId ?? obj.entity_id ?? 0),
      title: String(obj.title ?? ''),
      contentCid: String(obj.contentCid ?? obj.content_cid ?? ''),
      level: String(obj.level ?? 'Basic') as DisclosureLevel,
      status: String(obj.status ?? 'Draft') as DisclosureStatus,
      createdAt: Number(obj.createdAt ?? obj.created_at ?? 0),
    };
  });
}

function parseInsiderEntries(rawEntries: [any, any][]): InsiderData[] {
  if (!rawEntries || !Array.isArray(rawEntries)) return [];
  return rawEntries.map(([key, value]) => {
    const obj = value?.toJSON?.() ?? value;
    return {
      account: String(key.args?.[1]?.toString() ?? obj.account ?? ''),
      role: String(obj.role ?? 'Admin') as InsiderRole,
      addedAt: Number(obj.addedAt ?? obj.added_at ?? 0),
    };
  });
}

function parseAnnouncementEntries(rawEntries: [any, any][]): AnnouncementData[] {
  if (!rawEntries || !Array.isArray(rawEntries)) return [];
  return rawEntries.map(([key, value]) => {
    const obj = value?.toJSON?.() ?? value;
    return {
      id: Number(key.args?.[1]?.toString() ?? obj.id ?? 0),
      title: String(obj.title ?? ''),
      contentCid: String(obj.contentCid ?? obj.content_cid ?? ''),
      category: String(obj.category ?? ''),
      pinned: Boolean(obj.pinned),
      expiresAt: obj.expiresAt ?? obj.expires_at ?? null,
      createdAt: Number(obj.createdAt ?? obj.created_at ?? 0),
    };
  });
}

function parseBlackoutPeriod(raw: unknown): { start: number; end: number } | null {
  if (!raw || (raw as { isNone?: boolean }).isNone) return null;
  const unwrapped = (raw as { unwrapOr?: (d: null) => unknown }).unwrapOr?.(null) ?? raw;
  if (!unwrapped) return null;
  const obj = (unwrapped as any).toJSON?.() ?? unwrapped;
  return { start: Number(obj.start ?? 0), end: Number(obj.end ?? 0) };
}

// ─── Hook ───────────────────────────────────────────────────

export function useDisclosure() {
  const { entityId } = useEntityContext();

  const invalidateKeys = [['entity', entityId, 'disclosure']];

  // ─── Queries ──────────────────────────────────────────

  const disclosuresQuery = useEntityQuery<DisclosureData[]>(
    ['entity', entityId, 'disclosure'],
    async (api) => {
      const raw = await (api.query as any).entityDisclosure.disclosures.entries(entityId);
      return parseDisclosureEntries(raw);
    },
    { staleTime: STALE_TIMES.members },
  );

  const disclosureLevelQuery = useEntityQuery<DisclosureLevel>(
    ['entity', entityId, 'disclosure', 'level'],
    async (api) => {
      const raw = await (api.query as any).entityDisclosure.disclosureLevel(entityId);
      return String(raw?.toString() ?? 'Basic') as DisclosureLevel;
    },
    { staleTime: STALE_TIMES.members },
  );

  const insidersQuery = useEntityQuery<InsiderData[]>(
    ['entity', entityId, 'disclosure', 'insiders'],
    async (api) => {
      const raw = await (api.query as any).entityDisclosure.insiders.entries(entityId);
      return parseInsiderEntries(raw);
    },
    { staleTime: STALE_TIMES.members },
  );

  const blackoutQuery = useEntityQuery<{ start: number; end: number } | null>(
    ['entity', entityId, 'disclosure', 'blackout'],
    async (api) => {
      const raw = await (api.query as any).entityDisclosure.blackoutPeriod(entityId);
      return parseBlackoutPeriod(raw);
    },
    { staleTime: STALE_TIMES.members },
  );

  const announcementsQuery = useEntityQuery<AnnouncementData[]>(
    ['entity', entityId, 'disclosure', 'announcements'],
    async (api) => {
      const raw = await (api.query as any).entityDisclosure.announcements.entries(entityId);
      return parseAnnouncementEntries(raw);
    },
    { staleTime: STALE_TIMES.members },
  );

  // ─── Mutations ──────────────────────────────────────────

  const createDraftDisclosure = useEntityMutation('entityDisclosure', 'createDraftDisclosure', { invalidateKeys });
  const updateDraft = useEntityMutation('entityDisclosure', 'updateDraft', { invalidateKeys });
  const publishDraft = useEntityMutation('entityDisclosure', 'publishDraft', { invalidateKeys });
  const withdrawDisclosure = useEntityMutation('entityDisclosure', 'withdrawDisclosure', { invalidateKeys });
  const correctDisclosure = useEntityMutation('entityDisclosure', 'correctDisclosure', { invalidateKeys });
  const addInsider = useEntityMutation('entityDisclosure', 'addInsider', { invalidateKeys });
  const updateInsiderRole = useEntityMutation('entityDisclosure', 'updateInsiderRole', { invalidateKeys });
  const removeInsider = useEntityMutation('entityDisclosure', 'removeInsider', { invalidateKeys });
  const setDisclosureLevel = useEntityMutation('entityDisclosure', 'setDisclosureLevel', { invalidateKeys });
  const publishAnnouncement = useEntityMutation('entityDisclosure', 'publishAnnouncement', { invalidateKeys });
  const updateAnnouncement = useEntityMutation('entityDisclosure', 'updateAnnouncement', { invalidateKeys });
  const withdrawAnnouncement = useEntityMutation('entityDisclosure', 'withdrawAnnouncement', { invalidateKeys });
  const pinAnnouncement = useEntityMutation('entityDisclosure', 'pinAnnouncement', { invalidateKeys });

  return {
    // Query data
    disclosures: disclosuresQuery.data ?? [],
    disclosureLevel: disclosureLevelQuery.data ?? DisclosureLevel.Basic,
    insiders: insidersQuery.data ?? [],
    blackout: blackoutQuery.data ?? null,
    announcements: announcementsQuery.data ?? [],
    isLoading: disclosuresQuery.isLoading,
    error: disclosuresQuery.error,
    // Mutations
    createDraftDisclosure,
    updateDraft,
    publishDraft,
    withdrawDisclosure,
    correctDisclosure,
    addInsider,
    updateInsiderRole,
    removeInsider,
    setDisclosureLevel,
    publishAnnouncement,
    updateAnnouncement,
    withdrawAnnouncement,
    pinAnnouncement,
  };
}
