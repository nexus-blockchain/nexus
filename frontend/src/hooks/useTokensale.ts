"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { SaleRound } from "@/lib/types";

export function useSaleRounds(entityId: number | null) {
  const [rounds, setRounds] = useState<SaleRound[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const ids = await (api.query as any).entityTokensale.entitySaleRounds(entityId);
      const idList = (ids.toJSON() || []) as number[];
      const results = await Promise.all(
        idList.map(async (id: number) => {
          const raw = await (api.query as any).entityTokensale.saleRounds(id);
          return raw.toJSON() as SaleRound;
        })
      );
      setRounds(results.filter(Boolean));
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { rounds, isLoading, refetch: fetch };
}

export function useTokensaleActions() {
  const { submit, state, reset } = useTx();
  return {
    createSaleRound: (entityId: number, mode: string, totalAmount: bigint, price: bigint, startBlock: number, endBlock: number) =>
      submit("entityTokensale", "createSaleRound", [entityId, mode, totalAmount, price, startBlock, endBlock]),
    addPaymentOption: (roundId: number, assetType: string, price: bigint) =>
      submit("entityTokensale", "addPaymentOption", [roundId, assetType, price]),
    setVestingConfig: (roundId: number, initialUnlockPct: number, vestingBlocks: number) =>
      submit("entityTokensale", "setVestingConfig", [roundId, initialUnlockPct, vestingBlocks]),
    configureDutchAuction: (roundId: number, startPrice: bigint, endPrice: bigint, decayRate: number) =>
      submit("entityTokensale", "configureDutchAuction", [roundId, startPrice, endPrice, decayRate]),
    addToWhitelist: (roundId: number, accounts: string[]) =>
      submit("entityTokensale", "addToWhitelist", [roundId, accounts]),
    startSale: (roundId: number) => submit("entityTokensale", "startSale", [roundId]),
    subscribe: (roundId: number, amount: bigint, paymentAsset: string) =>
      submit("entityTokensale", "subscribe", [roundId, amount, paymentAsset]),
    endSale: (roundId: number) => submit("entityTokensale", "endSale", [roundId]),
    claimTokens: (roundId: number) => submit("entityTokensale", "claimTokens", [roundId]),
    unlockTokens: (roundId: number) => submit("entityTokensale", "unlockTokens", [roundId]),
    cancelSale: (roundId: number) => submit("entityTokensale", "cancelSale", [roundId]),
    claimRefund: (roundId: number) => submit("entityTokensale", "claimRefund", [roundId]),
    withdrawFunds: (roundId: number) => submit("entityTokensale", "withdrawFunds", [roundId]),
    reclaimUnclaimed: (roundId: number) => submit("entityTokensale", "reclaimUnclaimedTokens", [roundId]),
    txState: state,
    resetTx: reset,
  };
}
