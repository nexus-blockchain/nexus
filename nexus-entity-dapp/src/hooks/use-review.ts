'use client';

import { useEntityQuery } from './use-entity-query';
import { useEntityMutation } from './use-entity-mutation';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { STALE_TIMES } from '@/lib/chain/constants';

// ─── Types ──────────────────────────────────────────────────

interface ReviewData {
  id: number;
  entityId: number;
  orderId: number;
  reviewer: string;
  rating: number;
  contentCid: string;
  createdAt: number;
}

// ─── Parsers ────────────────────────────────────────────────

function parseReviewEntries(rawEntries: [any, any][]): ReviewData[] {
  if (!rawEntries || !Array.isArray(rawEntries)) return [];
  return rawEntries.map(([key, value]) => {
    const obj = value?.toJSON?.() ?? value;
    return {
      id: Number(key.args?.[1]?.toString() ?? obj.id ?? 0),
      entityId: Number(key.args?.[0]?.toString() ?? obj.entityId ?? obj.entity_id ?? 0),
      orderId: Number(obj.orderId ?? obj.order_id ?? 0),
      reviewer: String(obj.reviewer ?? ''),
      rating: Number(obj.rating ?? 0),
      contentCid: String(obj.contentCid ?? obj.content_cid ?? ''),
      createdAt: Number(obj.createdAt ?? obj.created_at ?? 0),
    };
  });
}

// ─── Helpers ────────────────────────────────────────────────

/** Returns true only for integers 1-5. */
export function validateRating(value: number): boolean {
  return Number.isInteger(value) && value >= 1 && value <= 5;
}

// ─── Hook ───────────────────────────────────────────────────

export function useReview() {
  const { entityId } = useEntityContext();

  const invalidateKeys = [['entity', entityId, 'reviews']];

  // ─── Queries ──────────────────────────────────────────

  const reviewsQuery = useEntityQuery<ReviewData[]>(
    ['entity', entityId, 'reviews'],
    async (api) => {
      const raw = await (api.query as any).entityReview.reviews.entries(entityId);
      return parseReviewEntries(raw);
    },
    { staleTime: STALE_TIMES.token },
  );

  const reviewEnabledQuery = useEntityQuery<boolean>(
    ['entity', entityId, 'reviews', 'enabled'],
    async (api) => {
      const raw = await (api.query as any).entityReview.reviewEnabled(entityId);
      if (raw === undefined || raw === null) return true;
      const val = (raw as any).unwrapOr?.(true) ?? raw;
      return Boolean(val?.toJSON?.() ?? val);
    },
    { staleTime: STALE_TIMES.token },
  );

  // ─── Mutations ──────────────────────────────────────────

  const submitReview = useEntityMutation('entityReview', 'submitReview', { invalidateKeys });
  const setReviewEnabled = useEntityMutation('entityReview', 'setReviewEnabled', { invalidateKeys });

  return {
    reviews: reviewsQuery.data ?? [],
    reviewEnabled: reviewEnabledQuery.data ?? true,
    isLoading: reviewsQuery.isLoading,
    error: reviewsQuery.error,
    submitReview,
    setReviewEnabled,
  };
}
