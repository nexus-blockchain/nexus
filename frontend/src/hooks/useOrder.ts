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

export function useUserOrders(account: string | null) {
  const [orders, setOrders] = useState<OrderData[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!account) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityOrder.buyerOrders.entries(account);
      const results = entries.map(([_key, val]: [unknown, { toJSON: () => OrderData }]) => val.toJSON());
      setOrders(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [account]);

  useEffect(() => { fetch(); }, [fetch]);
  return { orders, isLoading, refetch: fetch };
}

export function useOrderActions() {
  const { submit, state, reset } = useTx();
  return {
    placeOrder: (
      shopId: number,
      productId: number,
      quantity: number,
      paymentAsset: string,
      shippingCid: string,
      useTokens: bigint | null,
      useShoppingBalance: bigint | null,
    ) =>
      submit("entityOrder", "placeOrder", [
        shopId, productId, quantity,
        paymentAsset, shippingCid,
        useTokens, useShoppingBalance,
      ]),
    cancelOrder: (orderId: number) =>
      submit("entityOrder", "cancelOrder", [orderId]),
    payOrder: (orderId: number) =>
      submit("entityOrder", "payOrder", [orderId]),
    shipOrder: (orderId: number, trackingCid: string) =>
      submit("entityOrder", "shipOrder", [orderId, trackingCid]),
    confirmReceipt: (orderId: number) =>
      submit("entityOrder", "confirmReceipt", [orderId]),
    requestRefund: (orderId: number, reasonCid: string) =>
      submit("entityOrder", "requestRefund", [orderId, reasonCid]),
    approveRefund: (orderId: number) =>
      submit("entityOrder", "approveRefund", [orderId]),
    disputeOrder: (orderId: number, reasonCid: string) =>
      submit("entityOrder", "disputeOrder", [orderId, reasonCid]),
    startService: (orderId: number) =>
      submit("entityOrder", "startService", [orderId]),
    completeService: (orderId: number) =>
      submit("entityOrder", "completeService", [orderId]),
    confirmService: (orderId: number) =>
      submit("entityOrder", "confirmService", [orderId]),
    txState: state,
    resetTx: reset,
  };
}
