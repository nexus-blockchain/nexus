'use client';

import { useEntityQuery } from './use-entity-query';
import { useEntityMutation } from './use-entity-mutation';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { STALE_TIMES } from '@/lib/chain/constants';
import { TokenType, TransferRestrictionMode } from '@/lib/types/enums';
import type { TokenConfig } from '@/lib/types/models';

// ─── Parsers ────────────────────────────────────────────────

function parseTokenConfig(raw: unknown, entityId: number): TokenConfig | null {
  if (!raw || (raw as { isNone?: boolean }).isNone) return null;
  const unwrapped = (raw as { unwrapOr?: (d: null) => unknown }).unwrapOr?.(null) ?? raw;
  if (!unwrapped) return null;
  const obj = unwrapped as Record<string, unknown>;

  return {
    entityId,
    name: String(obj.name ?? ''),
    symbol: String(obj.symbol ?? ''),
    decimals: Number(obj.decimals ?? 0),
    tokenType: String(obj.tokenType ?? 'Points') as TokenType,
    totalSupply: BigInt(String(obj.totalSupply ?? 0)),
    maxSupply: BigInt(String(obj.maxSupply ?? 0)),
    transferRestriction: String(obj.transferRestriction ?? 'None') as TransferRestrictionMode,
    holderCount: Number(obj.holderCount ?? 0),
  };
}

// ─── Token Rights Labels ────────────────────────────────────

const TOKEN_RIGHTS_MAP: Record<TokenType, string> = {
  [TokenType.Points]: '消费奖励',
  [TokenType.Governance]: '投票权',
  [TokenType.Equity]: '分红权（证券类）',
  [TokenType.Membership]: '会员资格',
  [TokenType.Share]: '基金份额（证券类）',
  [TokenType.Bond]: '固定收益（证券类）',
  [TokenType.Hybrid]: '混合型',
};

export function getTokenRightsLabel(tokenType: TokenType): string {
  return TOKEN_RIGHTS_MAP[tokenType] ?? tokenType;
}

// ─── Hook ───────────────────────────────────────────────────

export function useEntityToken() {
  const { entityId } = useEntityContext();

  // Query token config
  const tokenConfigQuery = useEntityQuery<TokenConfig | null>(
    ['entity', entityId, 'token'],
    async (api) => {
      const raw = await (api.query as any).entityToken.tokenConfigs(entityId);
      return parseTokenConfig(raw, entityId);
    },
    { staleTime: STALE_TIMES.token },
  );

  // Query holder count
  const holderCountQuery = useEntityQuery<number>(
    ['entity', entityId, 'token', 'holderCount'],
    async (api) => {
      const raw = await (api.query as any).entityToken.tokenHolderCount(entityId);
      return Number(raw?.toString() ?? 0);
    },
    { staleTime: STALE_TIMES.token },
  );

  // Query holders list
  const holdersQuery = useEntityQuery<{ account: string; balance: bigint }[]>(
    ['entity', entityId, 'token', 'holders'],
    async (api) => {
      const raw = await (api.query as any).entityToken.tokenHolders.entries(entityId);
      if (!raw || !Array.isArray(raw)) return [];
      return raw.map(([key, value]: [any, any]) => ({
        account: key.args[1]?.toString() ?? '',
        balance: BigInt(String(value?.toString() ?? 0)),
      }));
    },
    { staleTime: STALE_TIMES.token },
  );

  // Query whitelist
  const whitelistQuery = useEntityQuery<string[]>(
    ['entity', entityId, 'token', 'whitelist'],
    async (api) => {
      const raw = await (api.query as any).entityToken.transferWhitelist.entries(entityId);
      if (!raw || !Array.isArray(raw)) return [];
      return raw.map(([key]: [any]) => key.args[1]?.toString() ?? '');
    },
    { staleTime: STALE_TIMES.token },
  );

  // Query blacklist
  const blacklistQuery = useEntityQuery<string[]>(
    ['entity', entityId, 'token', 'blacklist'],
    async (api) => {
      const raw = await (api.query as any).entityToken.transferBlacklist.entries(entityId);
      if (!raw || !Array.isArray(raw)) return [];
      return raw.map(([key]: [any]) => key.args[1]?.toString() ?? '');
    },
    { staleTime: STALE_TIMES.token },
  );

  // ─── Mutations ──────────────────────────────────────────

  const mintTokens = useEntityMutation('entityToken', 'mintTokens', {
    invalidateKeys: [['entity', entityId, 'token']],
  });

  const burnTokens = useEntityMutation('entityToken', 'burnTokens', {
    invalidateKeys: [['entity', entityId, 'token']],
    confirmDialog: {
      title: '确认销毁代币',
      description: '代币销毁是不可逆操作，销毁后无法恢复。请确认您要销毁指定数量的代币。',
      severity: 'danger',
    },
  });

  const setTransferRestriction = useEntityMutation('entityToken', 'setTransferRestriction', {
    invalidateKeys: [['entity', entityId, 'token']],
  });

  const addToWhitelist = useEntityMutation('entityToken', 'addToWhitelist', {
    invalidateKeys: [['entity', entityId, 'token', 'whitelist']],
  });

  const removeFromWhitelist = useEntityMutation('entityToken', 'removeFromWhitelist', {
    invalidateKeys: [['entity', entityId, 'token', 'whitelist']],
  });

  const addToBlacklist = useEntityMutation('entityToken', 'addToBlacklist', {
    invalidateKeys: [['entity', entityId, 'token', 'blacklist']],
  });

  const removeFromBlacklist = useEntityMutation('entityToken', 'removeFromBlacklist', {
    invalidateKeys: [['entity', entityId, 'token', 'blacklist']],
  });

  return {
    tokenConfig: tokenConfigQuery.data ?? null,
    holderCount: holderCountQuery.data ?? 0,
    holders: holdersQuery.data ?? [],
    whitelist: whitelistQuery.data ?? [],
    blacklist: blacklistQuery.data ?? [],
    isLoading: tokenConfigQuery.isLoading,
    error: tokenConfigQuery.error,
    mintTokens,
    burnTokens,
    setTransferRestriction,
    addToWhitelist,
    removeFromWhitelist,
    addToBlacklist,
    removeFromBlacklist,
  };
}
