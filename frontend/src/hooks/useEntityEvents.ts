"use client";

import { useState, useEffect, useCallback, useRef } from "react";
import { getApi } from "./useApi";

export interface EntityEvent {
  id: string;
  section: string;
  method: string;
  data: Record<string, unknown>;
  blockNumber: number;
  timestamp: number;
}

const RELEVANT_SECTIONS = [
  "entityRegistry",
  "entityShop",
  "entityProduct",
  "entityOrder",
  "entityReview",
  "entityToken",
  "entityMarket",
  "entityMember",
  "entityGovernance",
  "entityDisclosure",
  "entityKyc",
  "entityTokensale",
  "commissionCore",
];

export function useEntityEvents(entityId: number | null, maxEvents = 50) {
  const [events, setEvents] = useState<EntityEvent[]>([]);
  const [isSubscribed, setIsSubscribed] = useState(false);
  const unsubRef = useRef<(() => void) | null>(null);
  const eventIdCounter = useRef(0);

  const subscribe = useCallback(async () => {
    if (!entityId) return;

    try {
      const api = await getApi();
      const unsub = await api.query.system.events((rawEvents: Array<{
        event: { section: string; method: string; data: { toJSON: () => unknown[] } };
        phase: { isApplyExtrinsic: boolean };
      }>) => {
        const newEvents: EntityEvent[] = [];

        rawEvents.forEach((record) => {
          const { event } = record;
          const section = event.section.toString();

          if (RELEVANT_SECTIONS.some((s) => section.toLowerCase().includes(s.toLowerCase()))) {
            const data = event.data.toJSON() as unknown[];
            const entityIdInEvent = data[0];
            if (entityIdInEvent !== undefined && Number(entityIdInEvent) === entityId) {
              eventIdCounter.current += 1;
              newEvents.push({
                id: `evt-${eventIdCounter.current}`,
                section,
                method: event.method.toString(),
                data: Object.fromEntries(data.map((d, i) => [`arg${i}`, d])),
                blockNumber: 0,
                timestamp: Date.now(),
              });
            }
          }
        });

        if (newEvents.length > 0) {
          setEvents((prev) => [...newEvents, ...prev].slice(0, maxEvents));
        }
      });

      unsubRef.current = unsub as unknown as () => void;
      setIsSubscribed(true);
    } catch {
      setIsSubscribed(false);
    }
  }, [entityId, maxEvents]);

  const unsubscribe = useCallback(() => {
    if (unsubRef.current) {
      unsubRef.current();
      unsubRef.current = null;
    }
    setIsSubscribed(false);
  }, []);

  useEffect(() => {
    subscribe();
    return () => unsubscribe();
  }, [subscribe, unsubscribe]);

  const clearEvents = useCallback(() => setEvents([]), []);

  return { events, isSubscribed, clearEvents, unsubscribe, resubscribe: subscribe };
}
