'use client';

import { useEntityQuery } from './use-entity-query';
import { useEntityMutation } from './use-entity-mutation';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { STALE_TIMES } from '@/lib/chain/constants';
import type { SaleRound, DutchAuctionConfig, VestingConfig } from '@/lib/types/models';

// ─── Parsers ────────────────────────────────────────────────

function parseSaleRoundEntries(rawEntries: [any, any][]): SaleRound[] {
  if (!rawEntries || !Array.isArray(rawEntries)) return [];
  return rawEntries.map(([key, value]) => {
    const obj = value?.toJSON?.() ?? value;
    const vc = obj.vestingConfig ?? obj.vesting_config ?? null;
    const da = obj.dutchAuction ?? obj.dutch_auction ?? null;
    return {
      id: Number(key.args?.[1]?.toString() ?? obj.id ?? 0),
      entityId: Number(key.args?.[0]?.toString() ?? obj.entityId ?? obj.entity_id ?? 0),
      name: String(obj.name ?? ''),
      totalSupply: BigInt(String(obj.totalSupply ?? obj.total_supply ?? 0)),
      price: BigInt(String(obj.price ?? 0)),
      startBlock: Number(obj.startBlock ?? obj.start_block ?? 0),
      endBlock: Number(obj.endBlock ?? obj.end_block ?? 0),
      minPurchase: BigInt(String(obj.minPurchase ?? obj.min_purchase ?? 0)),
      maxPurchase: BigInt(String(obj.maxPurchase ?? obj.max_purchase ?? 0)),
      softCap: BigInt(String(obj.softCap ?? obj.soft_cap ?? 0)),
      hardCap: BigInt(String(obj.hardCap ?? obj.hard_cap ?? 0)),
      totalRaised: BigInt(String(obj.totalRaised ?? obj.total_raised ?? 0)),
      participantCount: Number(obj.participantCount ?? obj.participant_count ?? 0),
      vestingConfig: vc ? {
        cliffBlocks: Number(vc.cliffBlocks ?? vc.cliff_blocks ?? 0),
        vestingBlocks: Number(vc.vestingBlocks ?? vc.vesting_blocks ?? 0),
      } : null,
      dutchAuction: da ? {
        startPrice: BigInt(String(da.startPrice ?? da.start_price ?? 0)),
        endPrice: BigInt(String(da.endPrice ?? da.end_price ?? 0)),
        decayBlocks: Number(da.decayBlocks ?? da.decay_blocks ?? 0),
      } : null,
      status: String(obj.status ?? 'Created'),
    };
  });
}

function parseWhitelistEntries(rawEntries: [any, any][]): string[] {
  if (!rawEntries || !Array.isArray(rawEntries)) return [];
  return rawEntries.map(([key]) => String(key.args?.[2]?.toString() ?? ''));
}

// ─── Helpers ────────────────────────────────────────────────

/**
 * Compute current Dutch auction price via linear interpolation.
 */
export function computeDutchAuctionPrice(config: DutchAuctionConfig, elapsedBlocks: number): bigint {
  if (elapsedBlocks <= 0) return config.startPrice;
  if (elapsedBlocks >= config.decayBlocks) return config.endPrice;
  const priceDiff = config.startPrice - config.endPrice;
  const decay = (priceDiff * BigInt(elapsedBlocks)) / BigInt(config.decayBlocks);
  return config.startPrice - decay;
}

// ─── Hook ───────────────────────────────────────────────────

export function useTokensale() {
  const { entityId } = useEntityContext();

  const invalidateKeys = [['entity', entityId, 'tokensale']];

  // ─── Queries ──────────────────────────────────────────

  const saleRoundsQuery = useEntityQuery<SaleRound[]>(
    ['entity', entityId, 'tokensale'],
    async (api) => {
      const raw = await (api.query as any).entityTokensale.saleRounds.entries(entityId);
      return parseSaleRoundEntries(raw);
    },
    { staleTime: STALE_TIMES.token },
  );

  const useSubscription = (roundId: number, account: string | null) =>
    useEntityQuery<{ amount: bigint; claimed: boolean } | null>(
      ['entity', entityId, 'tokensale', roundId, 'subscriptions', account],
      async (api) => {
        if (!account) return null;
        const raw = await (api.query as any).entityTokensale.subscriptions(entityId, roundId, account);
        if (!raw || (raw as any).isNone) return null;
        const obj = (raw as any).unwrapOr?.(null) ?? raw;
        if (!obj) return null;
        const data = obj.toJSON?.() ?? obj;
        return {
          amount: BigInt(String(data.amount ?? 0)),
          claimed: Boolean(data.claimed),
        };
      },
      { staleTime: STALE_TIMES.token, enabled: !!account },
    );

  const useWhitelist = (roundId: number) =>
    useEntityQuery<string[]>(
      ['entity', entityId, 'tokensale', roundId, 'whitelist'],
      async (api) => {
        const raw = await (api.query as any).entityTokensale.whitelists.entries(entityId, roundId);
        return parseWhitelistEntries(raw);
      },
      { staleTime: STALE_TIMES.token },
    );

  // ─── Mutations ──────────────────────────────────────────

  const createSaleRound = useEntityMutation('entityTokensale', 'createSaleRound', { invalidateKeys });
  const startSaleRound = useEntityMutation('entityTokensale', 'startSaleRound', { invalidateKeys });
  const subscribe = useEntityMutation('entityTokensale', 'subscribe', { invalidateKeys });
  const increaseSubscription = useEntityMutation('entityTokensale', 'increaseSubscription', { invalidateKeys });
  const endSaleRound = useEntityMutation('entityTokensale', 'endSaleRound', { invalidateKeys });
  const claimTokens = useEntityMutation('entityTokensale', 'claimTokens', { invalidateKeys });
  const unlockTokens = useEntityMutation('entityTokensale', 'unlockTokens', { invalidateKeys });
  const claimRefund = useEntityMutation('entityTokensale', 'claimRefund', { invalidateKeys });
  const addToWhitelist = useEntityMutation('entityTokensale', 'addToWhitelist', { invalidateKeys });
  const removeFromWhitelist = useEntityMutation('entityTokensale', 'removeFromWhitelist', { invalidateKeys });
  const configureDutchAuction = useEntityMutation('entityTokensale', 'configureDutchAuction', { invalidateKeys });
  const configureVesting = useEntityMutation('entityTokensale', 'configureVesting', { invalidateKeys });

  return {
    saleRounds: saleRoundsQuery.data ?? [],
    isLoading: saleRoundsQuery.isLoading,
    error: saleRoundsQuery.error,
    useSubscription,
    useWhitelist,
    createSaleRound,
    startSaleRound,
    subscribe,
    increaseSubscription,
    endSaleRound,
    claimTokens,
    unlockTokens,
    claimRefund,
    addToWhitelist,
    removeFromWhitelist,
    configureDutchAuction,
    configureVesting,
  };
}
