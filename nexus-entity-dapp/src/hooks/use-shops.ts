'use client';

import { useCallback } from 'react';
import { useEntityQuery } from './use-entity-query';
import { useEntityMutation } from './use-entity-mutation';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { computeEffectiveShopStatus } from '@/lib/utils/shop-status';
import { STALE_TIMES } from '@/lib/chain/constants';
import { ShopOperatingStatus } from '@/lib/types/enums';
import type { ShopData, PointsConfig } from '@/lib/types/models';

// ─── Parsers ────────────────────────────────────────────────

function parseShopData(raw: unknown, entityStatus: string): ShopData | null {
  if (!raw || (raw as { isNone?: boolean }).isNone) return null;
  const unwrapped = (raw as { unwrapOr?: (d: null) => unknown }).unwrapOr?.(null) ?? raw;
  if (!unwrapped) return null;
  const obj = unwrapped as Record<string, unknown>;

  const operatingStatus = String(obj.operatingStatus ?? 'Active') as ShopOperatingStatus;
  const fundBalance = BigInt(String(obj.fundBalance ?? 0));
  const fundDepleted = fundBalance <= BigInt(0);

  return {
    id: Number(obj.id ?? 0),
    entityId: Number(obj.entityId ?? 0),
    name: String(obj.name ?? ''),
    shopType: String(obj.shopType ?? 'OnlineStore') as ShopData['shopType'],
    operatingStatus,
    effectiveStatus: computeEffectiveShopStatus(
      entityStatus as any,
      operatingStatus,
      fundDepleted,
    ),
    fundBalance,
    pointsConfig: obj.pointsConfig ? parsePointsConfig(obj.pointsConfig) : null,
  };
}

function parsePointsConfig(raw: unknown): PointsConfig | null {
  if (!raw || (raw as { isNone?: boolean }).isNone) return null;
  const obj = (raw as Record<string, unknown>);
  return {
    rewardRateBps: Number(obj.rewardRateBps ?? 0),
    exchangeRateBps: Number(obj.exchangeRateBps ?? 0),
    transferable: Boolean(obj.transferable),
  };
}

// ─── Hook ───────────────────────────────────────────────────

export function useShops() {
  const { entityId, entity } = useEntityContext();
  const entityStatus = entity?.status ?? 'Active';

  // Query shop IDs for this entity
  const shopIdsQuery = useEntityQuery<number[]>(
    ['entity', entityId, 'shops'],
    async (api) => {
      const raw = await (api.query as any).entityShop.entityShopIds(entityId);
      if (!raw) return [];
      const arr = raw.toJSON?.() ?? raw;
      return Array.isArray(arr) ? arr.map(Number) : [];
    },
    { staleTime: STALE_TIMES.shops },
  );

  // Query full shop data for each shop ID
  const shopsQuery = useEntityQuery<ShopData[]>(
    ['entity', entityId, 'shops', 'data', shopIdsQuery.data],
    async (api) => {
      const ids = shopIdsQuery.data;
      if (!ids || ids.length === 0) return [];

      const results = await Promise.all(
        ids.map((id) => (api.query as any).entityShop.shops(id)),
      );

      return results
        .map((raw) => parseShopData(raw, entityStatus))
        .filter((s): s is ShopData => s !== null);
    },
    {
      staleTime: STALE_TIMES.shops,
      enabled: !!shopIdsQuery.data && shopIdsQuery.data.length > 0,
    },
  );

  // Query single shop
  const getShop = useCallback(
    (shopId: number) => {
      return shopsQuery.data?.find((s) => s.id === shopId) ?? null;
    },
    [shopsQuery.data],
  );

  // Mutations
  const createShop = useEntityMutation('entityShop', 'createShop', {
    invalidateKeys: [['entity', entityId, 'shops']],
  });

  const closeShop = useEntityMutation('entityShop', 'closeShop', {
    invalidateKeys: [['entity', entityId, 'shops']],
  });

  const pauseShop = useEntityMutation('entityShop', 'pauseShop', {
    invalidateKeys: [['entity', entityId, 'shops']],
  });

  const resumeShop = useEntityMutation('entityShop', 'resumeShop', {
    invalidateKeys: [['entity', entityId, 'shops']],
  });

  const depositFund = useEntityMutation('entityShop', 'depositFund', {
    invalidateKeys: [['entity', entityId, 'shops']],
  });

  return {
    shops: shopsQuery.data ?? [],
    isLoading: shopIdsQuery.isLoading || shopsQuery.isLoading,
    error: shopIdsQuery.error || shopsQuery.error,
    getShop,
    createShop,
    closeShop,
    pauseShop,
    resumeShop,
    depositFund,
  };
}
