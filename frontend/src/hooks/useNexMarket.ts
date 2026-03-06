"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { NexOrder, UsdtTrade, NexMarketStats, TwapData, PriceProtectionConfig, TradeDispute } from "@/lib/types";

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

      const parseOrder = (data: Record<string, unknown>): NexOrder => ({
        ...data,
        nexAmount: BigInt(String(data.nexAmount || 0)),
        filledAmount: BigInt(String(data.filledAmount || 0)),
        buyerDeposit: BigInt(String(data.buyerDeposit || 0)),
      } as NexOrder);

      const [sells, buys] = await Promise.all([
        Promise.all(
          sellIdList.map(async (id: number) => {
            const raw = await q.nexMarket.orders(id);
            return raw.isNone ? null : parseOrder(raw.toJSON());
          })
        ),
        Promise.all(
          buyIdList.map(async (id: number) => {
            const raw = await q.nexMarket.orders(id);
            return raw.isNone ? null : parseOrder(raw.toJSON());
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

  useEffect(() => { fetch(); }, [fetch]);

  return {
    sellOrders, buyOrders, bestAsk, bestBid, lastTradePrice,
    marketStats, isPaused, tradingFeeBps, isLoading, refetch: fetch,
  };
}

export function useUserOrders(address: string | null) {
  const [orders, setOrders] = useState<NexOrder[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!address) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const idsRaw = await q.nexMarket.userOrders(address);
      const idList = (idsRaw.toJSON() || []) as number[];
      const results = await Promise.all(
        idList.map(async (id: number) => {
          const raw = await q.nexMarket.orders(id);
          if (raw.isNone) return null;
          const data = raw.toJSON() as Record<string, unknown>;
          return {
            ...data,
            nexAmount: BigInt(String(data.nexAmount || 0)),
            filledAmount: BigInt(String(data.filledAmount || 0)),
            buyerDeposit: BigInt(String(data.buyerDeposit || 0)),
          } as NexOrder;
        })
      );
      setOrders(results.filter(Boolean) as NexOrder[]);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [address]);

  useEffect(() => { fetch(); }, [fetch]);
  return { orders, isLoading, refetch: fetch };
}

export function useUserTrades(address: string | null) {
  const [trades, setTrades] = useState<UsdtTrade[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!address) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const idsRaw = await q.nexMarket.userTrades(address);
      const idList = (idsRaw.toJSON() || []) as number[];
      const results = await Promise.all(
        idList.map(async (id: number) => {
          const raw = await q.nexMarket.usdtTrades(id);
          if (raw.isNone) return null;
          const data = raw.toJSON() as Record<string, unknown>;
          return {
            ...data,
            tradeId: id,
            nexAmount: BigInt(String(data.nexAmount || 0)),
            buyerDeposit: BigInt(String(data.buyerDeposit || 0)),
          } as UsdtTrade;
        })
      );
      setTrades(results.filter(Boolean) as UsdtTrade[]);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [address]);

  useEffect(() => { fetch(); }, [fetch]);
  return { trades, isLoading, refetch: fetch };
}

export function useTwap() {
  const [twap, setTwap] = useState<TwapData | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).nexMarket.twapAccumulatorStore();
      if (raw && !raw.isNone) {
        setTwap(raw.toJSON() as unknown as TwapData);
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { twap, isLoading, refetch: fetch };
}

export function usePriceProtection() {
  const [config, setConfig] = useState<PriceProtectionConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).nexMarket.priceProtectionStore();
      if (raw && !raw.isNone) {
        setConfig(raw.toJSON() as unknown as PriceProtectionConfig);
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { config, isLoading, refetch: fetch };
}

export function usePendingTrades() {
  const [trades, setTrades] = useState<UsdtTrade[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const [pendingIds, awaitingIds, underpaidIds] = await Promise.all([
        q.nexMarket.pendingUsdtTrades(),
        q.nexMarket.awaitingPaymentTrades(),
        q.nexMarket.pendingUnderpaidTrades(),
      ]);
      const allIds = [
        ...(pendingIds.toJSON() || []) as number[],
        ...(awaitingIds.toJSON() || []) as number[],
        ...(underpaidIds.toJSON() || []) as number[],
      ];
      const unique = [...new Set(allIds)];
      const results = await Promise.all(
        unique.map(async (id) => {
          const raw = await q.nexMarket.usdtTrades(id);
          if (raw.isNone) return null;
          const data = raw.toJSON() as Record<string, unknown>;
          return {
            ...data, tradeId: id,
            nexAmount: BigInt(String(data.nexAmount || 0)),
            buyerDeposit: BigInt(String(data.buyerDeposit || 0)),
          } as UsdtTrade;
        })
      );
      setTrades(results.filter(Boolean) as UsdtTrade[]);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { trades, isLoading, refetch: fetch };
}

export function useTradeDispute(tradeId: number | null) {
  const [dispute, setDispute] = useState<TradeDispute | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (tradeId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).nexMarket.tradeDisputeStore(tradeId);
      if (raw && !raw.isNone) {
        setDispute(raw.toJSON() as unknown as TradeDispute);
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [tradeId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { dispute, isLoading, refetch: fetch };
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
    processTimeout: (tradeId: number) =>
      submit("nexMarket", "processTimeout", [tradeId]),
    claimVerificationReward: (tradeId: number) =>
      submit("nexMarket", "claimVerificationReward", [tradeId]),
    finalizeUnderpaid: (tradeId: number) =>
      submit("nexMarket", "finalizeUnderpaid", [tradeId]),
    disputeTrade: (tradeId: number, evidenceCid: string) =>
      submit("nexMarket", "disputeTrade", [tradeId, evidenceCid]),

    configurePriceProtection: (enabled: boolean, maxDeviation: number, cbThreshold: number, minTrades: number) =>
      submit("nexMarket", "configurePriceProtection", [enabled, maxDeviation, cbThreshold, minTrades]),
    setInitialPrice: (initialPrice: number) =>
      submit("nexMarket", "setInitialPrice", [initialPrice]),
    liftCircuitBreaker: () =>
      submit("nexMarket", "liftCircuitBreaker", []),
    setTradingFee: (feeBps: number) =>
      submit("nexMarket", "setTradingFee", [feeBps]),
    forcePauseMarket: () =>
      submit("nexMarket", "forcePauseMarket", []),
    forceResumeMarket: () =>
      submit("nexMarket", "forceResumeMarket", []),
    forceSettleTrade: (tradeId: number, actualAmount: number, resolution: string) =>
      submit("nexMarket", "forceSettleTrade", [tradeId, actualAmount, resolution]),
    forceCancelTrade: (tradeId: number) =>
      submit("nexMarket", "forceCancelTrade", [tradeId]),
    resolveDispute: (tradeId: number, resolution: string) =>
      submit("nexMarket", "resolveDispute", [tradeId, resolution]),
    updateDepositExchangeRate: (newRate: number) =>
      submit("nexMarket", "updateDepositExchangeRate", [newRate]),
    fundSeedAccount: (amount: bigint) =>
      submit("nexMarket", "fundSeedAccount", [amount]),
    seedLiquidity: (orderCount: number, usdtOverride: number | null) =>
      submit("nexMarket", "seedLiquidity", [orderCount, usdtOverride]),

    txState: state,
    resetTx: reset,
  };
}
