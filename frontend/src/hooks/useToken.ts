"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { TokenConfig } from "@/lib/types";

export function useToken(entityId: number | null) {
  const [config, setConfig] = useState<TokenConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).entityToken.shopTokenConfigs(entityId);
      if (raw.isNone) { setConfig(null); } else { setConfig(raw.toJSON() as unknown as TokenConfig); }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { config, isLoading, refetch: fetch };
}

export function useTokenActions() {
  const { submit, state, reset } = useTx();
  return {
    createToken: (entityId: number, name: string, symbol: string, decimals: number, rewardRate: number, exchangeRate: number) =>
      submit("entityToken", "createShopToken", [entityId, name, symbol, decimals, rewardRate, exchangeRate]),
    updateConfig: (entityId: number, rewardRate: number, exchangeRate: number, minRedeem: bigint, maxRedeemPerOrder: bigint, transferable: boolean) =>
      submit("entityToken", "updateTokenConfig", [entityId, rewardRate, exchangeRate, minRedeem, maxRedeemPerOrder, transferable]),
    mintTokens: (entityId: number, to: string, amount: bigint) =>
      submit("entityToken", "mintTokens", [entityId, to, amount]),
    transferTokens: (entityId: number, to: string, amount: bigint) =>
      submit("entityToken", "transferTokens", [entityId, to, amount]),
    configureDividend: (entityId: number, enabled: boolean, interval: number, minAmount: bigint) =>
      submit("entityToken", "configureDividend", [entityId, enabled, interval, minAmount]),
    distributeDividend: (entityId: number, amount: bigint, recipients: string[]) =>
      submit("entityToken", "distributeDividend", [entityId, amount, recipients]),
    claimDividend: (entityId: number) => submit("entityToken", "claimDividend", [entityId]),
    lockTokens: (entityId: number, user: string, amount: bigint, unlockAt: number) =>
      submit("entityToken", "lockTokens", [entityId, user, amount, unlockAt]),
    unlockTokens: (entityId: number) => submit("entityToken", "unlockTokens", [entityId]),
    changeTokenType: (entityId: number, newType: string) =>
      submit("entityToken", "changeTokenType", [entityId, newType]),
    setMaxSupply: (entityId: number, maxSupply: bigint) =>
      submit("entityToken", "setMaxSupply", [entityId, maxSupply]),
    setTransferRestriction: (entityId: number, mode: string) =>
      submit("entityToken", "setTransferRestriction", [entityId, mode]),
    addToWhitelist: (entityId: number, accounts: string[]) =>
      submit("entityToken", "addToWhitelist", [entityId, accounts]),
    removeFromWhitelist: (entityId: number, accounts: string[]) =>
      submit("entityToken", "removeFromWhitelist", [entityId, accounts]),
    addToBlacklist: (entityId: number, accounts: string[]) =>
      submit("entityToken", "addToBlacklist", [entityId, accounts]),
    removeFromBlacklist: (entityId: number, accounts: string[]) =>
      submit("entityToken", "removeFromBlacklist", [entityId, accounts]),
    txState: state,
    resetTx: reset,
  };
}
