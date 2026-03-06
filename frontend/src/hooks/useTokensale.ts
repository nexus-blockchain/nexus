"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { SaleRound, PaymentOptionConfig, SubscriptionData } from "@/lib/types";

export function useSaleRounds(entityId: number | null) {
  const [rounds, setRounds] = useState<SaleRound[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const ids = await (api.query as any).entityTokensale.entityRounds(entityId);
      const idList = (ids.toJSON() || []) as number[];
      const results = await Promise.all(
        idList.map(async (id: number) => {
          const raw = await (api.query as any).entityTokensale.saleRounds(id);
          if (raw.isNone) return null;
          const data = raw.toJSON() as Record<string, unknown>;
          return {
            ...data,
            id,
            totalSupply: BigInt(String(data.totalSupply || 0)),
            soldAmount: BigInt(String(data.soldAmount || 0)),
            remainingAmount: BigInt(String(data.remainingAmount || 0)),
            dutchStartPrice: data.dutchStartPrice ? BigInt(String(data.dutchStartPrice)) : null,
            dutchEndPrice: data.dutchEndPrice ? BigInt(String(data.dutchEndPrice)) : null,
            totalRefundedTokens: BigInt(String(data.totalRefundedTokens || 0)),
            totalRefundedNex: BigInt(String(data.totalRefundedNex || 0)),
            softCap: BigInt(String(data.softCap || 0)),
          } as SaleRound;
        })
      );
      setRounds(results.filter(Boolean) as SaleRound[]);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { rounds, isLoading, refetch: fetch };
}

export function useRoundPaymentOptions(roundId: number | null) {
  const [options, setOptions] = useState<PaymentOptionConfig[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (roundId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).entityTokensale.roundPaymentOptions(roundId);
      if (raw && !raw.isNone) {
        const data = (raw.toJSON() || []) as Array<Record<string, unknown>>;
        setOptions(data.map((d) => ({
          ...d,
          price: BigInt(String(d.price || 0)),
          minPurchase: BigInt(String(d.minPurchase || 0)),
          maxPurchasePerAccount: BigInt(String(d.maxPurchasePerAccount || 0)),
        })) as PaymentOptionConfig[]);
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [roundId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { options, isLoading, refetch: fetch };
}

export function useSubscription(roundId: number | null, account: string | null) {
  const [subscription, setSubscription] = useState<SubscriptionData | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (roundId === null || !account) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).entityTokensale.subscriptions(roundId, account);
      if (raw && !raw.isNone) {
        const data = raw.toJSON() as Record<string, unknown>;
        setSubscription({
          ...data,
          amount: BigInt(String(data.amount || 0)),
          paymentAmount: BigInt(String(data.paymentAmount || 0)),
          unlockedAmount: BigInt(String(data.unlockedAmount || 0)),
        } as SubscriptionData);
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [roundId, account]);

  useEffect(() => { fetch(); }, [fetch]);
  return { subscription, isLoading, refetch: fetch };
}

export function useRoundWhitelist(roundId: number | null) {
  const [whitelist, setWhitelist] = useState<Array<{ account: string; allocation: bigint | null }>>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (roundId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityTokensale.roundWhitelist.entries(roundId);
      const results = entries.map(([key, v]: [{ args: [unknown, { toString: () => string }] }, { toJSON: () => unknown }]) => ({
        account: key.args[1].toString(),
        allocation: v.toJSON() ? BigInt(String(v.toJSON())) : null,
      }));
      setWhitelist(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [roundId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { whitelist, isLoading, refetch: fetch };
}

export function useTokensaleActions() {
  const { submit, state, reset } = useTx();
  return {
    createSaleRound: (entityId: number, mode: string, totalSupply: bigint, startBlock: number, endBlock: number, kycRequired: boolean, minKycLevel: number, softCap: bigint) =>
      submit("entityTokensale", "createSaleRound", [entityId, mode, totalSupply, startBlock, endBlock, kycRequired, minKycLevel, softCap]),
    updateSaleRound: (roundId: number, totalSupply: bigint | null, startBlock: number | null, endBlock: number | null, kycRequired: boolean | null, minKycLevel: number | null) =>
      submit("entityTokensale", "updateSaleRound", [roundId, totalSupply, startBlock, endBlock, kycRequired, minKycLevel]),
    addPaymentOption: (roundId: number, assetId: number | null, price: bigint, minPurchase: bigint, maxPurchasePerAccount: bigint) =>
      submit("entityTokensale", "addPaymentOption", [roundId, assetId, price, minPurchase, maxPurchasePerAccount]),
    removePaymentOption: (roundId: number, index: number) =>
      submit("entityTokensale", "removePaymentOption", [roundId, index]),
    setVestingConfig: (roundId: number, vestingType: string, initialUnlockBps: number, cliffDuration: number, totalDuration: number, unlockInterval: number) =>
      submit("entityTokensale", "setVestingConfig", [roundId, vestingType, initialUnlockBps, cliffDuration, totalDuration, unlockInterval]),
    configureDutchAuction: (roundId: number, startPrice: bigint, endPrice: bigint) =>
      submit("entityTokensale", "configureDutchAuction", [roundId, startPrice, endPrice]),
    addToWhitelist: (roundId: number, accounts: Array<[string, bigint | null]>) =>
      submit("entityTokensale", "addToWhitelist", [roundId, accounts]),
    removeFromWhitelist: (roundId: number, accounts: string[]) =>
      submit("entityTokensale", "removeFromWhitelist", [roundId, accounts]),
    startSale: (roundId: number) =>
      submit("entityTokensale", "startSale", [roundId]),
    subscribe: (roundId: number, amount: bigint, paymentAsset: number | null) =>
      submit("entityTokensale", "subscribe", [roundId, amount, paymentAsset]),
    increaseSubscription: (roundId: number, additionalAmount: bigint, paymentAsset: number | null) =>
      submit("entityTokensale", "increaseSubscription", [roundId, additionalAmount, paymentAsset]),
    endSale: (roundId: number) =>
      submit("entityTokensale", "endSale", [roundId]),
    claimTokens: (roundId: number) =>
      submit("entityTokensale", "claimTokens", [roundId]),
    unlockTokens: (roundId: number) =>
      submit("entityTokensale", "unlockTokens", [roundId]),
    cancelSale: (roundId: number) =>
      submit("entityTokensale", "cancelSale", [roundId]),
    claimRefund: (roundId: number) =>
      submit("entityTokensale", "claimRefund", [roundId]),
    withdrawFunds: (roundId: number) =>
      submit("entityTokensale", "withdrawFunds", [roundId]),
    reclaimUnclaimed: (roundId: number) =>
      submit("entityTokensale", "reclaimUnclaimedTokens", [roundId]),
    extendSale: (roundId: number, newEndBlock: number) =>
      submit("entityTokensale", "extendSale", [roundId, newEndBlock]),
    pauseSale: (roundId: number) =>
      submit("entityTokensale", "pauseSale", [roundId]),
    resumeSale: (roundId: number) =>
      submit("entityTokensale", "resumeSale", [roundId]),
    cleanupRound: (roundId: number) =>
      submit("entityTokensale", "cleanupRound", [roundId]),
    txState: state,
    resetTx: reset,
  };
}
