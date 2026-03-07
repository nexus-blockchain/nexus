'use client';

import { useEffect, useRef, useCallback } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import type { ApiPromise } from '@polkadot/api';
import type { VoidFn } from '@polkadot/api/types';
import type { EventRecord } from '@polkadot/types/interfaces';
import { useApi } from '@/lib/chain';
import { useEntityDAppStore } from '@/stores/entity-dapp-store';
import type { Notification } from '@/lib/types/models';

// ─── Event Configuration ────────────────────────────────────

export interface EntityEventConfig {
  pallet: string;
  events: string[];
  /** Return true if this event is relevant to the given entityId */
  filter: (event: EventRecord, entityId: number) => boolean;
  /** React Query keys to invalidate when this event fires */
  invalidateKeys: (entityId: number) => unknown[][];
}

/**
 * Attempt to extract an entity_id from the first event data field.
 * Most entity pallet events include entity_id as the first parameter.
 */
function extractEntityIdFromEvent(event: EventRecord): number | null {
  try {
    const data = event.event.data;
    if (data.length > 0) {
      const first = data[0];
      const num = Number(first.toString());
      if (Number.isFinite(num) && num >= 0) return num;
    }
  } catch {
    // ignore parse errors
  }
  return null;
}

function entityIdFilter(event: EventRecord, entityId: number): boolean {
  return extractEntityIdFromEvent(event) === entityId;
}

/**
 * All entity pallet event configurations.
 * Each config defines which pallet/events to watch, how to filter by entityId,
 * and which React Query keys to invalidate on match.
 */
export const ENTITY_EVENT_CONFIGS: EntityEventConfig[] = [
  {
    pallet: 'entityRegistry',
    events: ['EntitySuspendedLowFund', 'FundWarning', 'OwnershipTransferred', 'GovernanceLocked'],
    filter: entityIdFilter,
    invalidateKeys: (eid) => [['entity', eid]],
  },
  {
    pallet: 'entityShop',
    events: ['ShopCreated', 'ShopClosed', 'FundDeposited', 'PointsEnabled'],
    filter: entityIdFilter,
    invalidateKeys: (eid) => [['entity', eid, 'shops']],
  },
  {
    pallet: 'entityToken',
    events: ['TokensMinted', 'DividendDistributed', 'TokenTypeChanged'],
    filter: entityIdFilter,
    invalidateKeys: (eid) => [['entity', eid, 'token']],
  },
  {
    pallet: 'entityOrder',
    events: ['OrderPlaced', 'OrderPaid', 'OrderShipped', 'OrderCompleted', 'RefundRequested'],
    filter: entityIdFilter,
    invalidateKeys: (eid) => [
      ['entity', eid, 'orders'],
      ['entity', eid, 'shops'], // shop orders also need refresh
    ],
  },
  {
    pallet: 'entityMarket',
    events: ['OrderFilled', 'CircuitBreakerTriggered'],
    filter: entityIdFilter,
    invalidateKeys: (eid) => [['entity', eid, 'market']],
  },
  {
    pallet: 'entityMember',
    events: ['MemberRegistered', 'MemberUpgraded', 'MemberBanned'],
    filter: entityIdFilter,
    invalidateKeys: (eid) => [['entity', eid, 'members']],
  },
  {
    pallet: 'entityCommission',
    events: ['CommissionDistributed', 'WithdrawalCompleted', 'PoolRewardClaimed'],
    filter: entityIdFilter,
    invalidateKeys: (eid) => [['entity', eid, 'commission']],
  },
  {
    pallet: 'entityGovernance',
    events: ['ProposalCreated', 'VoteCast', 'ProposalExecuted', 'ProposalVetoed'],
    filter: entityIdFilter,
    invalidateKeys: (eid) => [['entity', eid, 'proposals']],
  },
];


// ─── Event Summary Generation ───────────────────────────────

/**
 * Generate a human-readable summary for a matched event.
 * Returns a non-empty string describing what happened.
 */
export function generateEventSummary(pallet: string, eventName: string): string {
  const summaries: Record<string, Record<string, string>> = {
    entityRegistry: {
      EntitySuspendedLowFund: 'Entity suspended due to low funds',
      FundWarning: 'Entity fund balance is below warning threshold',
      OwnershipTransferred: 'Entity ownership has been transferred',
      GovernanceLocked: 'Entity governance has been locked',
    },
    entityShop: {
      ShopCreated: 'A new shop has been created',
      ShopClosed: 'A shop has been closed',
      FundDeposited: 'Funds deposited to shop',
      PointsEnabled: 'Points system enabled for shop',
    },
    entityToken: {
      TokensMinted: 'New tokens have been minted',
      DividendDistributed: 'Dividend has been distributed',
      TokenTypeChanged: 'Token type has been changed',
    },
    entityOrder: {
      OrderPlaced: 'A new order has been placed',
      OrderPaid: 'An order has been paid',
      OrderShipped: 'An order has been shipped',
      OrderCompleted: 'An order has been completed',
      RefundRequested: 'A refund has been requested',
    },
    entityMarket: {
      OrderFilled: 'A market order has been filled',
      CircuitBreakerTriggered: 'Market circuit breaker has been triggered',
    },
    entityMember: {
      MemberRegistered: 'A new member has registered',
      MemberUpgraded: 'A member has been upgraded',
      MemberBanned: 'A member has been banned',
    },
    entityCommission: {
      CommissionDistributed: 'Commission has been distributed',
      WithdrawalCompleted: 'A withdrawal has been completed',
      PoolRewardClaimed: 'Pool reward has been claimed',
    },
    entityGovernance: {
      ProposalCreated: 'A new governance proposal has been created',
      VoteCast: 'A vote has been cast on a proposal',
      ProposalExecuted: 'A governance proposal has been executed',
      ProposalVetoed: 'A governance proposal has been vetoed',
    },
  };

  return summaries[pallet]?.[eventName] ?? `${pallet}.${eventName}`;
}

// ─── Event Filtering ────────────────────────────────────────

/**
 * Filter a chain event against all entity event configs for a given entityId.
 * Returns the matching config and event name, or null if no match.
 */
export function filterEntityEvent(
  event: EventRecord,
  entityId: number,
): { config: EntityEventConfig; eventName: string } | null {
  const section = event.event.section;
  const method = event.event.method;

  for (const config of ENTITY_EVENT_CONFIGS) {
    if (config.pallet === section && config.events.includes(method)) {
      if (config.filter(event, entityId)) {
        return { config, eventName: method };
      }
    }
  }
  return null;
}

// ─── Hook ───────────────────────────────────────────────────

/**
 * Subscribe to chain events relevant to the given entityId.
 * Automatically filters events by pallet and entity_id, generates notifications,
 * and invalidates related React Query caches.
 */
export function useEntityEvents(entityId: number) {
  const { api, isReady } = useApi();
  const queryClient = useQueryClient();
  const unsubRef = useRef<VoidFn | null>(null);

  const addNotification = useEntityDAppStore((s) => s.addNotification);
  const notifications = useEntityDAppStore((s) => s.notifications);
  const unreadCount = useEntityDAppStore((s) => s.unreadCount);
  const markNotificationRead = useEntityDAppStore((s) => s.markNotificationRead);
  const clearAllNotifications = useEntityDAppStore((s) => s.clearAllNotifications);

  const handleEvents = useCallback(
    (events: EventRecord[]) => {
      for (const record of events) {
        const match = filterEntityEvent(record, entityId);
        if (!match) continue;

        const { config, eventName } = match;

        // Generate notification
        const notification: Notification = {
          id: `${Date.now()}-${config.pallet}-${eventName}-${Math.random().toString(36).slice(2, 8)}`,
          pallet: config.pallet,
          event: eventName,
          summary: generateEventSummary(config.pallet, eventName),
          timestamp: Date.now(),
          read: false,
        };
        addNotification(notification);

        // Invalidate related queries to refresh page data
        const keysToInvalidate = config.invalidateKeys(entityId);
        for (const key of keysToInvalidate) {
          queryClient.invalidateQueries({ queryKey: key });
        }
      }
    },
    [entityId, addNotification, queryClient],
  );

  useEffect(() => {
    if (!api || !isReady) return;

    let cancelled = false;

    const subscribe = async () => {
      try {
        const unsub = await (api as ApiPromise).query.system.events(
          ((eventRecords: any) => {
            if (cancelled) return;
            const records: EventRecord[] = Array.isArray(eventRecords)
              ? eventRecords
              : [...eventRecords];
            handleEvents(records);
          }) as any,
        ) as unknown as VoidFn;
        if (!cancelled) {
          unsubRef.current = unsub;
        } else {
          unsub();
        }
      } catch {
        // Subscription failed — will retry on reconnect via ApiProvider
      }
    };

    subscribe();

    return () => {
      cancelled = true;
      unsubRef.current?.();
      unsubRef.current = null;
    };
  }, [api, isReady, handleEvents]);

  return {
    notifications,
    unreadCount,
    markAsRead: markNotificationRead,
    clearAll: clearAllNotifications,
  };
}
