"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { TokenConfig, VestingSchedule } from "@/lib/types";

export function useToken(entityId: number | null) {
  const [config, setConfig] = useState<TokenConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).entityToken.shopTokenConfigs(entityId);
      if (raw.isNone) {
        setConfig(null);
      } else {
        const json = raw.toJSON() as Record<string, unknown>;
        setConfig({
          ...json,
          maxSupply: BigInt(String(json.maxSupply || 0)),
          totalSupply: BigInt(String(json.totalSupply || 0)),
          minRedeem: BigInt(String(json.minRedeem || 0)),
          maxRedeemPerOrder: BigInt(String(json.maxRedeemPerOrder || 0)),
          dividendConfig: {
            enabled: Boolean((json.dividendConfig as any)?.enabled),
            minPeriod: Number((json.dividendConfig as any)?.minPeriod || 0),
            lastDistribution: Number((json.dividendConfig as any)?.lastDistribution || 0),
            accumulated: BigInt(String((json.dividendConfig as any)?.accumulated || 0)),
          },
        } as unknown as TokenConfig);
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { config, isLoading, refetch: fetch };
}

interface LockEntry {
  account: string;
  amount: bigint;
  unlockAt: number;
}

export function useTokenLocks(entityId: number | null) {
  const [locks, setLocks] = useState<LockEntry[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityToken.tokenLocks?.entries(entityId);
      if (entries) {
        const results: LockEntry[] = entries.map(
          ([key, val]: [{ args: [unknown, { toString: () => string }] }, { toJSON: () => { amount: string; unlockAt: number } }]) => {
            const data = val.toJSON();
            return {
              account: key.args[1].toString(),
              amount: BigInt(String(data.amount || 0)),
              unlockAt: Number(data.unlockAt || 0),
            };
          }
        );
        results.sort((a, b) => a.unlockAt - b.unlockAt);
        setLocks(results);
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { locks, isLoading, refetch: fetch };
}

export function useVestingSchedule(entityId: number | null, account: string | null) {
  const [schedule, setSchedule] = useState<VestingSchedule | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null || !account) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).entityToken.vestingSchedules?.(entityId, account);
      if (raw && !raw.isNone) {
        const json = raw.toJSON() as Record<string, unknown>;
        setSchedule({
          total: BigInt(String(json.total || 0)),
          released: BigInt(String(json.released || 0)),
          startBlock: Number(json.startBlock || 0),
          cliffBlocks: Number(json.cliffBlocks || 0),
          vestingBlocks: Number(json.vestingBlocks || 0),
        });
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId, account]);

  useEffect(() => { fetch(); }, [fetch]);
  return { schedule, isLoading, refetch: fetch };
}

interface WhiteBlackEntry {
  account: string;
}

export function useTokenWhitelist(entityId: number | null) {
  const [list, setList] = useState<WhiteBlackEntry[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityToken.transferWhitelist?.entries(entityId);
      if (entries) {
        setList(entries.map(([key]: [{ args: [unknown, { toString: () => string }] }]) => ({
          account: key.args[1].toString(),
        })));
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { list, isLoading, refetch: fetch };
}

export function useTokenBlacklist(entityId: number | null) {
  const [list, setList] = useState<WhiteBlackEntry[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityToken.transferBlacklist?.entries(entityId);
      if (entries) {
        setList(entries.map(([key]: [{ args: [unknown, { toString: () => string }] }]) => ({
          account: key.args[1].toString(),
        })));
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { list, isLoading, refetch: fetch };
}

export function useTokenActions() {
  const { submit, state, reset } = useTx();
  return {
    createToken: (entityId: number, name: string, symbol: string, decimals: number, tokenType: string, rewardRate: number, exchangeRate: number) =>
      submit("entityToken", "createShopToken", [entityId, name, symbol, decimals, tokenType, rewardRate, exchangeRate]),
    updateConfig: (entityId: number, rewardRate: number, exchangeRate: number, minRedeem: bigint, maxRedeemPerOrder: bigint, transferable: boolean) =>
      submit("entityToken", "updateTokenConfig", [entityId, rewardRate, exchangeRate, minRedeem, maxRedeemPerOrder, transferable]),
    mintTokens: (entityId: number, to: string, amount: bigint) =>
      submit("entityToken", "mintTokens", [entityId, to, amount]),
    transferTokens: (entityId: number, to: string, amount: bigint) =>
      submit("entityToken", "transferTokens", [entityId, to, amount]),
    configureDividend: (entityId: number, enabled: boolean, minPeriod: number, minAmount: bigint) =>
      submit("entityToken", "configureDividend", [entityId, enabled, minPeriod, minAmount]),
    distributeDividend: (entityId: number, amount: bigint, recipients: string[]) =>
      submit("entityToken", "distributeDividend", [entityId, amount, recipients]),
    claimDividend: (entityId: number) => submit("entityToken", "claimDividend", [entityId]),
    lockTokens: (entityId: number, user: string, amount: bigint, unlockAt: number) =>
      submit("entityToken", "lockTokens", [entityId, user, amount, unlockAt]),
    unlockTokens: (entityId: number) => submit("entityToken", "unlockTokens", [entityId]),
    createVesting: (entityId: number, beneficiary: string, total: bigint, startBlock: number, cliffBlocks: number, vestingBlocks: number) =>
      submit("entityToken", "createVestingSchedule", [entityId, beneficiary, total, startBlock, cliffBlocks, vestingBlocks]),
    releaseVested: (entityId: number) => submit("entityToken", "releaseVestedTokens", [entityId]),
    changeTokenType: (entityId: number, newType: string) =>
      submit("entityToken", "changeTokenType", [entityId, newType]),
    setMaxSupply: (entityId: number, maxSupply: bigint) =>
      submit("entityToken", "setMaxSupply", [entityId, maxSupply]),
    setTransferRestriction: (entityId: number, mode: string) =>
      submit("entityToken", "setTransferRestriction", [entityId, mode]),
    setMinReceiverKyc: (entityId: number, level: number) =>
      submit("entityToken", "setMinReceiverKyc", [entityId, level]),
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
