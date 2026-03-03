"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { OrderData } from "@/lib/types";

export function useOrders(shopId: number | null) {
  const [orders, setOrders] = useState<OrderData[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (shopId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityOrder.shopOrders.entries(shopId);
      const results = entries.map(([_key, val]: [unknown, { toJSON: () => OrderData }]) => val.toJSON());
      setOrders(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [shopId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { orders, isLoading, refetch: fetch };
}

export function useOrderActions() {
  const { submit, state, reset } = useTx();
  return {
    placeOrder: (shopId: number, productId: number, quantity: number, shippingCid: string, useTokens: bigint | null, useShoppingBalance: bigint | null) =>
      submit("entityOrder", "placeOrder", [shopId, productId, quantity, shippingCid, useTokens, useShoppingBalance]),
    cancelOrder: (orderId: number) => submit("entityOrder", "cancelOrder", [orderId]),
    shipOrder: (orderId: number, trackingCid: string) =>
      submit("entityOrder", "shipOrder", [orderId, trackingCid]),
    confirmReceipt: (orderId: number) => submit("entityOrder", "confirmReceipt", [orderId]),
    requestRefund: (orderId: number, reasonCid: string) =>
      submit("entityOrder", "requestRefund", [orderId, reasonCid]),
    approveRefund: (orderId: number) => submit("entityOrder", "approveRefund", [orderId]),
    startService: (orderId: number) => submit("entityOrder", "startService", [orderId]),
    completeService: (orderId: number) => submit("entityOrder", "completeService", [orderId]),
    confirmService: (orderId: number) => submit("entityOrder", "confirmService", [orderId]),
    txState: state,
    resetTx: reset,
  };
}
