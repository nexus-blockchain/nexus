"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";

export const COMMISSION_MODE_BITS = {
  NONE:                 0,
  DIRECT_REWARD:        1 << 0,
  MULTI_LEVEL:          1 << 1,
  TEAM_PERFORMANCE:     1 << 2,
  LEVEL_DIFF:           1 << 3,
  FIXED_AMOUNT:         1 << 4,
  FIRST_ORDER:          1 << 5,
  REPEAT_PURCHASE:      1 << 6,
  SINGLE_LINE_UPLINE:   1 << 7,
  SINGLE_LINE_DOWNLINE: 1 << 8,
  POOL_REWARD:          1 << 9,
  CREATOR_REWARD:       1 << 10,
} as const;

export const COMMISSION_MODE_LABELS: Record<number, { name: string; desc: string }> = {
  [COMMISSION_MODE_BITS.DIRECT_REWARD]:        { name: "Direct Reward",        desc: "Direct referral commission on each order" },
  [COMMISSION_MODE_BITS.MULTI_LEVEL]:          { name: "Multi-Level",          desc: "Multi-tier upstream referral chain rewards" },
  [COMMISSION_MODE_BITS.TEAM_PERFORMANCE]:     { name: "Team Performance",     desc: "Bonus based on team sales performance tiers" },
  [COMMISSION_MODE_BITS.LEVEL_DIFF]:           { name: "Level Difference",     desc: "Commission from level gap between referrer and member" },
  [COMMISSION_MODE_BITS.FIXED_AMOUNT]:         { name: "Fixed Amount",         desc: "Fixed token amount per qualifying referral" },
  [COMMISSION_MODE_BITS.FIRST_ORDER]:          { name: "First Order Bonus",    desc: "One-time bonus for a referral's first purchase" },
  [COMMISSION_MODE_BITS.REPEAT_PURCHASE]:      { name: "Repeat Purchase",      desc: "Commission on repeated orders from referred members" },
  [COMMISSION_MODE_BITS.SINGLE_LINE_UPLINE]:   { name: "Single Line (Up)",     desc: "Linear upline chain commission" },
  [COMMISSION_MODE_BITS.SINGLE_LINE_DOWNLINE]: { name: "Single Line (Down)",   desc: "Linear downline chain commission" },
  [COMMISSION_MODE_BITS.POOL_REWARD]:          { name: "Pool Reward",          desc: "Periodic pool distribution to qualifiers" },
  [COMMISSION_MODE_BITS.CREATOR_REWARD]:       { name: "Creator Reward",       desc: "Revenue share for entity creator" },
};

export interface CoreCommissionConfig {
  enabledModes: number;
  maxCommissionRate: number;
  enabled: boolean;
  withdrawalCooldown: number;
  creatorRewardRate: number;
  tokenWithdrawalCooldown: number;
}

export interface CommissionRecord {
  id: number;
  entityId: number;
  beneficiary: string;
  amount: bigint;
  source: string;
  orderId: number;
  status: string;
  createdAt: number;
}

export interface WithdrawableBalance {
  nex: bigint;
  token: bigint;
}

export interface WithdrawalTierConfig {
  withdrawalRate: number;
  repurchaseRate: number;
}

export interface EntityWithdrawalConfig {
  mode: string | { FixedRate?: { repurchaseRate: number }; MemberChoice?: { minRepurchaseRate: number } };
  defaultTier: WithdrawalTierConfig;
  levelOverrides: Array<[number, WithdrawalTierConfig]>;
  voluntaryBonusRate: number;
  enabled: boolean;
}

export interface ReferralConfig {
  directRewardRate?: number;
  fixedAmount?: string;
  firstOrderAmount?: string;
  firstOrderRate?: number;
  firstOrderUseAmount?: boolean;
  repeatPurchaseRate?: number;
  repeatPurchaseMinOrders?: number;
}

export interface MultiLevelTier {
  rate: number;
  requiredDirects: number;
  requiredTeamSize: number;
  requiredSpent: string;
}

export interface MultiLevelConfig {
  tiers: MultiLevelTier[];
  maxTotalRate: number;
  paused: boolean;
}

export interface LevelDiffConfig {
  levelRates: number[];
  maxDepth: number;
}

export interface SingleLineConfig {
  uplineRate: number;
  downlineRate: number;
  baseUplineLevels: number;
  baseDownlineLevels: number;
  levelIncrementThreshold: string;
  maxUplineLevels: number;
  maxDownlineLevels: number;
}

export interface TeamPerformanceTier {
  salesThreshold: string;
  minTeamSize: number;
  rate: number;
}

export interface TeamPerformanceConfig {
  tiers: TeamPerformanceTier[];
  maxDepth: number;
  allowStacking: boolean;
  thresholdMode: string;
  paused: boolean;
}

export interface PoolRewardConfig {
  levelRatios: Array<[number, number]>;
  roundDuration: number;
}

export function useCommissionConfig(entityId: number | null) {
  const [config, setConfig] = useState<CoreCommissionConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).commissionCore.commissionConfigs(entityId);
      if (raw && !raw.isNone) {
        const data = raw.toJSON() as Record<string, unknown>;
        setConfig({
          enabledModes: Number(data.enabledModes ?? 0),
          maxCommissionRate: Number(data.maxCommissionRate ?? 0),
          enabled: Boolean(data.enabled),
          withdrawalCooldown: Number(data.withdrawalCooldown ?? 0),
          creatorRewardRate: Number(data.creatorRewardRate ?? 0),
          tokenWithdrawalCooldown: Number(data.tokenWithdrawalCooldown ?? 0),
        });
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { config, isLoading, refetch: fetch };
}

export function useCommissionRecords(entityId: number | null) {
  const [records, setRecords] = useState<CommissionRecord[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).commissionCore.orderCommissionRecords.entries();
      const results: CommissionRecord[] = [];
      for (const [_k, v] of entries) {
        const arr = v.toJSON() as CommissionRecord[];
        if (Array.isArray(arr)) {
          for (const r of arr) {
            if (Number(r.entityId) === entityId) results.push(r);
          }
        }
      }
      setRecords(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { records, isLoading, refetch: fetch };
}

export function useWithdrawable(entityId: number | null, account: string | null) {
  const [balance, setBalance] = useState<WithdrawableBalance>({ nex: BigInt(0), token: BigInt(0) });
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null || !account) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const [rawNex, rawToken] = await Promise.all([
        q.commissionCore.memberCommissionStats([entityId, account]),
        q.commissionCore.memberTokenCommissionStats?.([entityId, account]),
      ]);
      let nex = BigInt(0);
      let token = BigInt(0);
      if (rawNex && !rawNex.isNone) {
        const data = rawNex.toJSON() as Record<string, string>;
        nex = BigInt(data.totalEarned || 0) - BigInt(data.totalWithdrawn || 0);
      }
      if (rawToken && !rawToken.isNone) {
        const data = rawToken.toJSON() as Record<string, string>;
        token = BigInt(data.totalEarned || 0) - BigInt(data.totalWithdrawn || 0);
      }
      setBalance({ nex: nex > 0 ? nex : BigInt(0), token: token > 0 ? token : BigInt(0) });
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId, account]);

  useEffect(() => { fetch(); }, [fetch]);
  return { balance, isLoading, refetch: fetch };
}

export function useWithdrawalConfig(entityId: number | null) {
  const [config, setConfig] = useState<EntityWithdrawalConfig | null>(null);
  const [tokenConfig, setTokenConfig] = useState<EntityWithdrawalConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const [rawNex, rawToken] = await Promise.all([
        q.commissionCore.withdrawalConfigs(entityId),
        q.commissionCore.tokenWithdrawalConfigs?.(entityId),
      ]);
      if (rawNex && !rawNex.isNone) setConfig(rawNex.toJSON() as unknown as EntityWithdrawalConfig);
      if (rawToken && !rawToken.isNone) setTokenConfig(rawToken.toJSON() as unknown as EntityWithdrawalConfig);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { config, tokenConfig, isLoading, refetch: fetch };
}

export function useReferralConfig(entityId: number | null) {
  const [config, setConfig] = useState<ReferralConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).commissionReferral.referralConfigs(entityId);
      if (raw && !raw.isNone) setConfig(raw.toJSON() as unknown as ReferralConfig);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { config, isLoading, refetch: fetch };
}

export function useMultiLevelConfig(entityId: number | null) {
  const [config, setConfig] = useState<MultiLevelConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).commissionMultiLevel.multiLevelConfigs(entityId);
      if (raw && !raw.isNone) setConfig(raw.toJSON() as unknown as MultiLevelConfig);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { config, isLoading, refetch: fetch };
}

export function useLevelDiffConfig(entityId: number | null) {
  const [config, setConfig] = useState<LevelDiffConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).commissionLevelDiff.customLevelDiffConfigs(entityId);
      if (raw && !raw.isNone) setConfig(raw.toJSON() as unknown as LevelDiffConfig);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { config, isLoading, refetch: fetch };
}

export function useSingleLineConfig(entityId: number | null) {
  const [config, setConfig] = useState<SingleLineConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).commissionSingleLine.singleLineConfigs(entityId);
      if (raw && !raw.isNone) setConfig(raw.toJSON() as unknown as SingleLineConfig);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { config, isLoading, refetch: fetch };
}

export function useTeamConfig(entityId: number | null) {
  const [config, setConfig] = useState<TeamPerformanceConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).commissionTeam.teamPerformanceConfigs(entityId);
      if (raw && !raw.isNone) setConfig(raw.toJSON() as unknown as TeamPerformanceConfig);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { config, isLoading, refetch: fetch };
}

export function usePoolRewardConfig(entityId: number | null) {
  const [config, setConfig] = useState<PoolRewardConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).commissionPoolReward.poolRewardConfigs(entityId);
      if (raw && !raw.isNone) setConfig(raw.toJSON() as unknown as PoolRewardConfig);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { config, isLoading, refetch: fetch };
}

export function useCommissionActions() {
  const { submit, state, reset } = useTx();
  return {
    setCommissionModes: (entityId: number, modes: number) =>
      submit("commissionCore", "setCommissionModes", [entityId, modes]),
    setCommissionRate: (entityId: number, rate: number) =>
      submit("commissionCore", "setCommissionRate", [entityId, rate]),
    enableCommission: (entityId: number, enabled: boolean) =>
      submit("commissionCore", "enableCommission", [entityId, enabled]),
    setWithdrawalCooldown: (entityId: number, nexCooldown: number, tokenCooldown: number) =>
      submit("commissionCore", "setWithdrawalCooldown", [entityId, nexCooldown, tokenCooldown]),
    setCreatorRewardRate: (entityId: number, rate: number) =>
      submit("commissionCore", "setCreatorRewardRate", [entityId, rate]),

    withdrawCommission: (entityId: number, amount: bigint | null, repurchaseRate: number | null, repurchaseTarget: string | null) =>
      submit("commissionCore", "withdrawCommission", [entityId, amount, repurchaseRate, repurchaseTarget]),
    withdrawTokenCommission: (entityId: number, amount: bigint | null, repurchaseRate: number | null, repurchaseTarget: string | null) =>
      submit("commissionCore", "withdrawTokenCommission", [entityId, amount, repurchaseRate, repurchaseTarget]),

    setWithdrawalConfig: (entityId: number, mode: unknown, defaultTier: WithdrawalTierConfig, levelOverrides: Array<[number, WithdrawalTierConfig]>, voluntaryBonusRate: number, enabled: boolean) =>
      submit("commissionCore", "setWithdrawalConfig", [entityId, mode, defaultTier, levelOverrides, voluntaryBonusRate, enabled]),
    setTokenWithdrawalConfig: (entityId: number, mode: unknown, defaultTier: WithdrawalTierConfig, levelOverrides: Array<[number, WithdrawalTierConfig]>, voluntaryBonusRate: number, enabled: boolean) =>
      submit("commissionCore", "setTokenWithdrawalConfig", [entityId, mode, defaultTier, levelOverrides, voluntaryBonusRate, enabled]),

    pauseWithdrawals: (entityId: number, paused: boolean) =>
      submit("commissionCore", "pauseWithdrawals", [entityId, paused]),

    setDirectRewardConfig: (entityId: number, rate: number) =>
      submit("commissionReferral", "setDirectRewardConfig", [entityId, rate]),
    setFixedAmountConfig: (entityId: number, amount: bigint) =>
      submit("commissionReferral", "setFixedAmountConfig", [entityId, amount]),
    setFirstOrderConfig: (entityId: number, amount: bigint, rate: number, useAmount: boolean) =>
      submit("commissionReferral", "setFirstOrderConfig", [entityId, amount, rate, useAmount]),
    setRepeatPurchaseConfig: (entityId: number, rate: number, minOrders: number) =>
      submit("commissionReferral", "setRepeatPurchaseConfig", [entityId, rate, minOrders]),
    clearReferralConfig: (entityId: number) =>
      submit("commissionReferral", "clearReferralConfig", [entityId]),
    setReferrerGuardConfig: (entityId: number, minSpent: bigint, minOrders: number) =>
      submit("commissionReferral", "setReferrerGuardConfig", [entityId, minSpent, minOrders]),
    setCommissionCapConfig: (entityId: number, maxPerOrder: bigint, maxTotalEarned: bigint) =>
      submit("commissionReferral", "setCommissionCapConfig", [entityId, maxPerOrder, maxTotalEarned]),

    setMultiLevelConfig: (entityId: number, levels: MultiLevelTier[], maxTotalRate: number) =>
      submit("commissionMultiLevel", "setMultiLevelConfig", [entityId, levels, maxTotalRate]),
    clearMultiLevelConfig: (entityId: number) =>
      submit("commissionMultiLevel", "clearMultiLevelConfig", [entityId]),
    addMultiLevelTier: (entityId: number, index: number, tier: MultiLevelTier) =>
      submit("commissionMultiLevel", "addTier", [entityId, index, tier]),
    removeMultiLevelTier: (entityId: number, index: number) =>
      submit("commissionMultiLevel", "removeTier", [entityId, index]),
    pauseMultiLevel: (entityId: number) =>
      submit("commissionMultiLevel", "pauseMultiLevel", [entityId]),
    resumeMultiLevel: (entityId: number) =>
      submit("commissionMultiLevel", "resumeMultiLevel", [entityId]),

    setLevelDiffConfig: (entityId: number, levelRates: number[], maxDepth: number) =>
      submit("commissionLevelDiff", "setLevelDiffConfig", [entityId, levelRates, maxDepth]),
    clearLevelDiffConfig: (entityId: number) =>
      submit("commissionLevelDiff", "clearLevelDiffConfig", [entityId]),

    setSingleLineConfig: (entityId: number, uplineRate: number, downlineRate: number, baseUpline: number, baseDownline: number, threshold: bigint, maxUpline: number, maxDownline: number) =>
      submit("commissionSingleLine", "setSingleLineConfig", [entityId, uplineRate, downlineRate, baseUpline, baseDownline, threshold, maxUpline, maxDownline]),
    clearSingleLineConfig: (entityId: number) =>
      submit("commissionSingleLine", "clearSingleLineConfig", [entityId]),
    pauseSingleLine: (entityId: number) =>
      submit("commissionSingleLine", "pauseSingleLine", [entityId]),
    resumeSingleLine: (entityId: number) =>
      submit("commissionSingleLine", "resumeSingleLine", [entityId]),

    setTeamPerformanceConfig: (entityId: number, tiers: Array<{ salesThreshold: bigint; minTeamSize: number; rate: number }>, maxDepth: number, allowStacking: boolean, thresholdMode: string) =>
      submit("commissionTeam", "setTeamPerformanceConfig", [entityId, tiers, maxDepth, allowStacking, thresholdMode]),
    clearTeamPerformanceConfig: (entityId: number) =>
      submit("commissionTeam", "clearTeamPerformanceConfig", [entityId]),
    pauseTeamPerformance: (entityId: number) =>
      submit("commissionTeam", "pauseTeamPerformance", [entityId]),
    resumeTeamPerformance: (entityId: number) =>
      submit("commissionTeam", "resumeTeamPerformance", [entityId]),

    setPoolRewardConfig: (entityId: number, levelRatios: Array<[number, number]>, roundDuration: number) =>
      submit("commissionPoolReward", "setPoolRewardConfig", [entityId, levelRatios, roundDuration]),
    clearPoolRewardConfig: (entityId: number) =>
      submit("commissionPoolReward", "clearPoolRewardConfig", [entityId]),
    claimPoolReward: (entityId: number) =>
      submit("commissionPoolReward", "claimPoolReward", [entityId]),
    setTokenPoolEnabled: (entityId: number, enabled: boolean) =>
      submit("commissionPoolReward", "setTokenPoolEnabled", [entityId, enabled]),
    pausePoolReward: (entityId: number) =>
      submit("commissionPoolReward", "pausePoolReward", [entityId]),
    resumePoolReward: (entityId: number) =>
      submit("commissionPoolReward", "resumePoolReward", [entityId]),

    txState: state,
    resetTx: reset,
  };
}
