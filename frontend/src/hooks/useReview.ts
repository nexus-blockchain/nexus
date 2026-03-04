"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";

export interface ReviewData {
  id: number;
  orderId: number;
  shopId: number;
  reviewer: string;
  rating: number;
  contentCid: string;
  status: string;
  createdAt: number;
}

export function useReviews(shopId: number | null) {
  const [reviews, setReviews] = useState<ReviewData[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (shopId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityOrder.shopReviews.entries(shopId);
      const results = entries.map(([_k, v]: [unknown, { toJSON: () => ReviewData }]) => v.toJSON());
      setReviews(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [shopId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { reviews, isLoading, refetch: fetch };
}

export function useReviewActions() {
  const { submit, state, reset } = useTx();
  return {
    submitReview: (orderId: number, rating: number, contentCid: string) =>
      submit("entityOrder", "submitReview", [orderId, rating, contentCid]),
    respondToReview: (reviewId: number, responseCid: string) =>
      submit("entityOrder", "respondToReview", [reviewId, responseCid]),
    flagReview: (reviewId: number, reason: string) =>
      submit("entityOrder", "flagReview", [reviewId, reason]),
    txState: state,
    resetTx: reset,
  };
}
