"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { NexOrder, UsdtTrade, NexMarketStats } from "@/lib/types";

export function useNexMarket() {
  const [sellOrders, setSellOrders] = useState<NexOrder[]>([]);
  const [buyOrders, setBuyOrders] = useState<NexOrder[]>([]);
  const [bestAsk, setBestAsk] = useState<number | null>(null);
  const [bestBid, setBestBid] = useState<number | null>(null);
  const [lastTradePrice, setLastTradePrice] = useState<number | null>(null);
  const [marketStats, setMarketStats] = useState<NexMarketStats | null>(null);
  const [isPaused, setIsPaused] = useState(false);
  const [tradingFeeBps, setTradingFeeBps] = useState(0);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;

      const [sellIds, buyIds, askRaw, bidRaw, priceRaw, statsRaw, pausedRaw, feeRaw] =
        await Promise.all([
          q.nexMarket.sellOrders(),
          q.nexMarket.buyOrders(),
          q.nexMarket.bestAsk(),
          q.nexMarket.bestBid(),
          q.nexMarket.lastTradePrice(),
          q.nexMarket.marketStatsStore(),
          q.nexMarket.marketPausedStore(),
          q.nexMarket.tradingFeeBps(),
        ]);

      const sellIdList = (sellIds.toJSON() || []) as number[];
      const buyIdList = (buyIds.toJSON() || []) as number[];

      const [sells, buys] = await Promise.all([
        Promise.all(
          sellIdList.map(async (id: number) => {
            const raw = await q.nexMarket.orders(id);
            return raw.isNone ? null : (raw.toJSON() as NexOrder);
          })
        ),
        Promise.all(
          buyIdList.map(async (id: number) => {
            const raw = await q.nexMarket.orders(id);
            return raw.isNone ? null : (raw.toJSON() as NexOrder);
          })
        ),
      ]);

      setSellOrders(sells.filter(Boolean) as NexOrder[]);
      setBuyOrders(buys.filter(Boolean) as NexOrder[]);
      setBestAsk(askRaw.isNone ? null : (askRaw.toJSON() as number));
      setBestBid(bidRaw.isNone ? null : (bidRaw.toJSON() as number));
      setLastTradePrice(priceRaw.isNone ? null : (priceRaw.toJSON() as number));
      setMarketStats(statsRaw.toJSON() as NexMarketStats);
      setIsPaused(pausedRaw.toJSON() as boolean);
      setTradingFeeBps(feeRaw.toJSON() as number);
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return {
    sellOrders,
    buyOrders,
    bestAsk,
    bestBid,
    lastTradePrice,
    marketStats,
    isPaused,
    tradingFeeBps,
    isLoading,
    refetch: fetch,
  };
}

export function useNexMarketActions() {
  const { submit, state, reset } = useTx();
  return {
    placeSellOrder: (nexAmount: bigint, usdtPrice: number, tronAddress: string) =>
      submit("nexMarket", "placeSellOrder", [nexAmount, usdtPrice, tronAddress]),
    placeBuyOrder: (nexAmount: bigint, usdtPrice: number, buyerTronAddress: string) =>
      submit("nexMarket", "placeBuyOrder", [nexAmount, usdtPrice, buyerTronAddress]),
    cancelOrder: (orderId: number) =>
      submit("nexMarket", "cancelOrder", [orderId]),
    updateOrderPrice: (orderId: number, newPrice: number) =>
      submit("nexMarket", "updateOrderPrice", [orderId, newPrice]),
    reserveSellOrder: (orderId: number, amount: bigint | null, buyerTronAddress: string) =>
      submit("nexMarket", "reserveSellOrder", [orderId, amount, buyerTronAddress]),
    acceptBuyOrder: (orderId: number, amount: bigint | null, tronAddress: string) =>
      submit("nexMarket", "acceptBuyOrder", [orderId, amount, tronAddress]),
    confirmPayment: (tradeId: number) =>
      submit("nexMarket", "confirmPayment", [tradeId]),
    disputeTrade: (tradeId: number, evidenceCid: string) =>
      submit("nexMarket", "disputeTrade", [tradeId, evidenceCid]),
    txState: state,
    resetTx: reset,
  };
}

export function useUserTrades(address: string | null) {
  const [trades, setTrades] = useState<UsdtTrade[]>([]);
  const [tradeIds, setTradeIds] = useState<number[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!address) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;

      const idsRaw = await q.nexMarket.userTrades(address);
      const idList = (idsRaw.toJSON() || []) as number[];
      setTradeIds(idList);

      const results = await Promise.all(
        idList.map(async (id: number) => {
          const raw = await q.nexMarket.usdtTrades(id);
          return raw.isNone ? null : (raw.toJSON() as UsdtTrade);
        })
      );

      setTrades(results.filter(Boolean) as UsdtTrade[]);
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [address]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { trades, tradeIds, isLoading, refetch: fetch };
}
