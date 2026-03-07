'use client';

import { useEntityQuery } from './use-entity-query';
import { useEntityMutation } from './use-entity-mutation';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useWalletStore } from '@/stores/wallet-store';
import { STALE_TIMES } from '@/lib/chain/constants';
import { OrderStatus, PaymentAsset } from '@/lib/types/enums';
import type { OrderData } from '@/lib/types/models';

// ─── Parser ─────────────────────────────────────────────────

function parseOrderData(raw: unknown): OrderData | null {
  if (!raw || (raw as { isNone?: boolean }).isNone) return null;
  const unwrapped = (raw as { unwrapOr?: (d: null) => unknown }).unwrapOr?.(null) ?? raw;
  if (!unwrapped) return null;
  const obj = unwrapped as Record<string, unknown>;

  return {
    id: Number(obj.id ?? 0),
    shopId: Number(obj.shopId ?? 0),
    productId: Number(obj.productId ?? 0),
    buyer: String(obj.buyer ?? ''),
    quantity: Number(obj.quantity ?? 0),
    paymentAsset: String(obj.paymentAsset ?? 'Native') as PaymentAsset,
    totalAmount: BigInt(String(obj.totalAmount ?? 0)),
    status: String(obj.status ?? 'Created') as OrderStatus,
    escrowId: obj.escrowId != null ? Number(obj.escrowId) : null,
    createdAt: Number(obj.createdAt ?? 0),
    updatedAt: Number(obj.updatedAt ?? 0),
  };
}

// ─── Hook: Shop Orders (seller view) ────────────────────────

export function useShopOrders(shopId: number) {
  const { entityId } = useEntityContext();

  const orderIdsQuery = useEntityQuery<number[]>(
    ['entity', entityId, 'shop', shopId, 'orders'],
    async (api) => {
      const raw = await (api.query as any).entityOrder.shopOrders(shopId);
      if (!raw) return [];
      const arr = raw.toJSON?.() ?? raw;
      return Array.isArray(arr) ? arr.map(Number) : [];
    },
    { staleTime: STALE_TIMES.orders, enabled: shopId > 0 },
  );

  const ordersQuery = useEntityQuery<OrderData[]>(
    ['entity', entityId, 'shop', shopId, 'orders', 'data', orderIdsQuery.data],
    async (api) => {
      const ids = orderIdsQuery.data;
      if (!ids || ids.length === 0) return [];
      const results = await Promise.all(
        ids.map((id) => (api.query as any).entityOrder.orders(id)),
      );
      return results
        .map((raw) => parseOrderData(raw))
        .filter((o): o is OrderData => o !== null);
    },
    {
      staleTime: STALE_TIMES.orders,
      enabled: !!orderIdsQuery.data && orderIdsQuery.data.length > 0,
    },
  );

  // Seller mutations
  const confirmPayment = useEntityMutation('entityOrder', 'confirmPayment', {
    invalidateKeys: [['entity', entityId, 'shop', shopId, 'orders']],
  });
  const confirmShipment = useEntityMutation('entityOrder', 'confirmShipment', {
    invalidateKeys: [['entity', entityId, 'shop', shopId, 'orders']],
  });
  const approveRefund = useEntityMutation('entityOrder', 'approveRefund', {
    invalidateKeys: [['entity', entityId, 'shop', shopId, 'orders']],
  });
  const cancelOrder = useEntityMutation('entityOrder', 'cancelOrder', {
    invalidateKeys: [['entity', entityId, 'shop', shopId, 'orders']],
  });
  const completeOrder = useEntityMutation('entityOrder', 'completeOrder', {
    invalidateKeys: [['entity', entityId, 'shop', shopId, 'orders']],
  });
  const startService = useEntityMutation('entityOrder', 'startService', {
    invalidateKeys: [['entity', entityId, 'shop', shopId, 'orders']],
  });
  const completeService = useEntityMutation('entityOrder', 'completeService', {
    invalidateKeys: [['entity', entityId, 'shop', shopId, 'orders']],
  });

  return {
    orders: ordersQuery.data ?? [],
    isLoading: orderIdsQuery.isLoading || ordersQuery.isLoading,
    error: orderIdsQuery.error || ordersQuery.error,
    confirmPayment,
    confirmShipment,
    approveRefund,
    cancelOrder,
    completeOrder,
    startService,
    completeService,
  };
}

// ─── Hook: Buyer Orders ─────────────────────────────────────

export function useBuyerOrders() {
  const { entityId } = useEntityContext();
  const address = useWalletStore((s) => s.address);

  const orderIdsQuery = useEntityQuery<number[]>(
    ['entity', entityId, 'orders', address],
    async (api) => {
      if (!address) return [];
      const raw = await (api.query as any).entityOrder.buyerOrders(address);
      if (!raw) return [];
      const arr = raw.toJSON?.() ?? raw;
      return Array.isArray(arr) ? arr.map(Number) : [];
    },
    { staleTime: STALE_TIMES.orders, enabled: !!address },
  );

  const ordersQuery = useEntityQuery<OrderData[]>(
    ['entity', entityId, 'orders', address, 'data', orderIdsQuery.data],
    async (api) => {
      const ids = orderIdsQuery.data;
      if (!ids || ids.length === 0) return [];
      const results = await Promise.all(
        ids.map((id) => (api.query as any).entityOrder.orders(id)),
      );
      return results
        .map((raw) => parseOrderData(raw))
        .filter((o): o is OrderData => o !== null);
    },
    {
      staleTime: STALE_TIMES.orders,
      enabled: !!orderIdsQuery.data && orderIdsQuery.data.length > 0,
    },
  );

  // Buyer mutations
  const placeOrder = useEntityMutation('entityOrder', 'placeOrder', {
    invalidateKeys: [['entity', entityId, 'orders', address]],
  });
  const confirmReceipt = useEntityMutation('entityOrder', 'confirmReceipt', {
    invalidateKeys: [['entity', entityId, 'orders', address]],
  });
  const requestRefund = useEntityMutation('entityOrder', 'requestRefund', {
    invalidateKeys: [['entity', entityId, 'orders', address]],
  });
  const cancelOrder = useEntityMutation('entityOrder', 'cancelOrder', {
    invalidateKeys: [['entity', entityId, 'orders', address]],
  });
  const confirmServiceCompletion = useEntityMutation('entityOrder', 'confirmServiceCompletion', {
    invalidateKeys: [['entity', entityId, 'orders', address]],
  });

  return {
    orders: ordersQuery.data ?? [],
    isLoading: orderIdsQuery.isLoading || ordersQuery.isLoading,
    error: orderIdsQuery.error || ordersQuery.error,
    placeOrder,
    confirmReceipt,
    requestRefund,
    cancelOrder,
    confirmServiceCompletion,
  };
}
