"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { MarketOrder } from "@/lib/types";

export function useOrderbook(entityId: number | null) {
  const [buyOrders, setBuyOrders] = useState<MarketOrder[]>([]);
  const [sellOrders, setSellOrders] = useState<MarketOrder[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityMarket.orders.entries();
      const all = entries
        .map(([_k, v]: [unknown, { toJSON: () => MarketOrder }]) => v.toJSON())
        .filter((o: MarketOrder) => o.entityId === entityId && o.status === "Open");
      setBuyOrders(all.filter((o: MarketOrder) => o.side === "Buy").sort((a: MarketOrder, b: MarketOrder) => Number(b.price) - Number(a.price)));
      setSellOrders(all.filter((o: MarketOrder) => o.side === "Sell").sort((a: MarketOrder, b: MarketOrder) => Number(a.price) - Number(b.price)));
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { buyOrders, sellOrders, isLoading, refetch: fetch };
}

export function useMarketActions() {
  const { submit, state, reset } = useTx();
  return {
    placeSellOrder: (entityId: number, amount: bigint, price: bigint) =>
      submit("entityMarket", "placeSellOrder", [entityId, amount, price]),
    placeBuyOrder: (entityId: number, amount: bigint, price: bigint) =>
      submit("entityMarket", "placeBuyOrder", [entityId, amount, price]),
    takeOrder: (orderId: number, amount: bigint | null) =>
      submit("entityMarket", "takeOrder", [orderId, amount]),
    cancelOrder: (orderId: number) => submit("entityMarket", "cancelOrder", [orderId]),
    marketBuy: (entityId: number, amount: bigint, maxCost: bigint) =>
      submit("entityMarket", "marketBuy", [entityId, amount, maxCost]),
    marketSell: (entityId: number, amount: bigint, minReceive: bigint) =>
      submit("entityMarket", "marketSell", [entityId, amount, minReceive]),
    configureMarket: (entityId: number, feeRate: number, orderTtl: number, minOrderAmount: bigint) =>
      submit("entityMarket", "configureMarket", [entityId, feeRate, orderTtl, minOrderAmount]),
    configurePriceProtection: (entityId: number, maxSlippage: number, cbThreshold: number, minTrades: number) =>
      submit("entityMarket", "configurePriceProtection", [entityId, maxSlippage, cbThreshold, minTrades]),
    setInitialPrice: (entityId: number, price: bigint) =>
      submit("entityMarket", "setInitialPrice", [entityId, price]),
    liftCircuitBreaker: (entityId: number) =>
      submit("entityMarket", "liftCircuitBreaker", [entityId]),
    placeUsdtSellOrder: (entityId: number, amount: bigint, usdtPrice: number, tronAddress: string) =>
      submit("entityMarket", "placeUsdtSellOrder", [entityId, amount, usdtPrice, tronAddress]),
    placeUsdtBuyOrder: (entityId: number, amount: bigint, usdtPrice: number) =>
      submit("entityMarket", "placeUsdtBuyOrder", [entityId, amount, usdtPrice]),
    confirmUsdtPayment: (tradeId: number, txHash: string) =>
      submit("entityMarket", "confirmUsdtPayment", [tradeId, txHash]),
    txState: state,
    resetTx: reset,
  };
}
