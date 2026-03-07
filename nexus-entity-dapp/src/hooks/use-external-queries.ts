'use client';

import { useQuery, type UseQueryResult } from '@tanstack/react-query';
import type { ApiPromise } from '@polkadot/api';
import { useApi } from '@/lib/chain';
import { STALE_TIMES, RETRY_CONFIG } from '@/lib/chain/constants';

// ─── Types ──────────────────────────────────────────────────

export interface NexBalance {
  free: bigint;
  reserved: bigint;
  frozen: bigint;
}

export interface EscrowStatus {
  escrowId: number;
  amount: bigint;
  status: string;
  depositor: string;
  beneficiary: string;
}

export interface AssetInfo {
  assetId: number;
  owner: string;
  supply: bigint;
  deposit: bigint;
  minBalance: bigint;
  isSufficient: boolean;
  accounts: number;
  isFrozen: boolean;
}

export interface StoragePinStatus {
  cid: string;
  pinned: boolean;
  size: number;
  fee: bigint;
  expiry: number | null;
}

/** Result shape for external queries with graceful degradation */
export interface ExternalQueryResult<T> {
  data: T | null;
  isLoading: boolean;
  isUnavailable: boolean;
  error: Error | null;
}

// ─── Parsers ────────────────────────────────────────────────

function parseAccountInfo(raw: unknown): NexBalance {
  const obj = raw as Record<string, unknown>;
  const data = (obj?.data ?? obj) as Record<string, unknown>;
  return {
    free: BigInt(String(data?.free ?? 0)),
    reserved: BigInt(String(data?.reserved ?? 0)),
    frozen: BigInt(String(data?.frozen ?? data?.miscFrozen ?? 0)),
  };
}

function parseEscrowData(raw: unknown): EscrowStatus | null {
  if (!raw || (raw as { isNone?: boolean }).isNone) return null;
  const unwrapped = (raw as { unwrapOr?: (d: null) => unknown }).unwrapOr?.(null) ?? raw;
  if (!unwrapped) return null;
  const obj = unwrapped as Record<string, unknown>;
  return {
    escrowId: Number(obj.escrowId ?? obj.id ?? 0),
    amount: BigInt(String(obj.amount ?? 0)),
    status: String(obj.status ?? 'Unknown'),
    depositor: String(obj.depositor ?? ''),
    beneficiary: String(obj.beneficiary ?? ''),
  };
}

function parseAssetData(raw: unknown): AssetInfo | null {
  if (!raw || (raw as { isNone?: boolean }).isNone) return null;
  const unwrapped = (raw as { unwrapOr?: (d: null) => unknown }).unwrapOr?.(null) ?? raw;
  if (!unwrapped) return null;
  const obj = unwrapped as Record<string, unknown>;
  return {
    assetId: Number(obj.assetId ?? obj.id ?? 0),
    owner: String(obj.owner ?? ''),
    supply: BigInt(String(obj.supply ?? 0)),
    deposit: BigInt(String(obj.deposit ?? 0)),
    minBalance: BigInt(String(obj.minBalance ?? 0)),
    isSufficient: Boolean(obj.isSufficient),
    accounts: Number(obj.accounts ?? 0),
    isFrozen: Boolean(obj.isFrozen),
  };
}

function parseStoragePinData(raw: unknown): StoragePinStatus | null {
  if (!raw || (raw as { isNone?: boolean }).isNone) return null;
  const unwrapped = (raw as { unwrapOr?: (d: null) => unknown }).unwrapOr?.(null) ?? raw;
  if (!unwrapped) return null;
  const obj = unwrapped as Record<string, unknown>;
  return {
    cid: String(obj.cid ?? ''),
    pinned: Boolean(obj.pinned),
    size: Number(obj.size ?? 0),
    fee: BigInt(String(obj.fee ?? 0)),
    expiry: obj.expiry != null ? Number(obj.expiry) : null,
  };
}

// ─── Helper ─────────────────────────────────────────────────

/**
 * Wraps a React Query result into an ExternalQueryResult with graceful degradation.
 * On error, returns { data: null, isUnavailable: true } instead of throwing.
 */
export function toExternalResult<T>(query: UseQueryResult<T>): ExternalQueryResult<T> {
  return {
    data: query.data ?? null,
    isLoading: query.isLoading,
    isUnavailable: query.isError,
    error: query.error instanceof Error ? query.error : query.error ? new Error(String(query.error)) : null,
  };
}

// ─── Hooks ──────────────────────────────────────────────────

/**
 * Query user NEX balance via pallet-balances (system.account).
 * Gracefully degrades on failure.
 *
 * Validates: Requirements 23.1, 23.5
 */
export function useNexBalance(address: string | null | undefined): ExternalQueryResult<NexBalance> {
  const { api, isReady } = useApi();

  const query = useQuery<NexBalance>({
    queryKey: ['external', 'balances', address],
    queryFn: async () => {
      if (!api) throw new Error('API not ready');
      const raw = await (api.query as any).system.account(address);
      return parseAccountInfo(raw);
    },
    enabled: isReady && !!api && !!address,
    staleTime: STALE_TIMES.entity,
    retry: RETRY_CONFIG.chainQuery.retry,
    retryDelay: RETRY_CONFIG.chainQuery.retryDelay,
  });

  return toExternalResult(query);
}

/**
 * Query escrow status via pallet-dispute-escrow.
 * Gracefully degrades on failure.
 *
 * Validates: Requirements 23.2, 23.5
 */
export function useEscrowStatus(escrowId: number | null | undefined): ExternalQueryResult<EscrowStatus> {
  const { api, isReady } = useApi();

  const query = useQuery<EscrowStatus>({
    queryKey: ['external', 'escrow', escrowId],
    queryFn: async () => {
      if (!api) throw new Error('API not ready');
      const raw = await (api.query as any).disputeEscrow.escrows(escrowId);
      const parsed = parseEscrowData(raw);
      if (!parsed) throw new Error('Escrow not found');
      return parsed;
    },
    enabled: isReady && !!api && escrowId != null,
    staleTime: STALE_TIMES.orders,
    retry: RETRY_CONFIG.chainQuery.retry,
    retryDelay: RETRY_CONFIG.chainQuery.retryDelay,
  });

  return toExternalResult(query);
}

/**
 * Query asset info via pallet-assets.
 * Gracefully degrades on failure.
 *
 * Validates: Requirements 23.3, 23.5
 */
export function useAssetInfo(assetId: number | null | undefined): ExternalQueryResult<AssetInfo> {
  const { api, isReady } = useApi();

  const query = useQuery<AssetInfo>({
    queryKey: ['external', 'assets', assetId],
    queryFn: async () => {
      if (!api) throw new Error('API not ready');
      const raw = await (api.query as any).assets.asset(assetId);
      const parsed = parseAssetData(raw);
      if (!parsed) throw new Error('Asset not found');
      return parsed;
    },
    enabled: isReady && !!api && assetId != null,
    staleTime: STALE_TIMES.token,
    retry: RETRY_CONFIG.chainQuery.retry,
    retryDelay: RETRY_CONFIG.chainQuery.retryDelay,
  });

  return toExternalResult(query);
}

/**
 * Query IPFS pin status via pallet-storage-service.
 * Gracefully degrades on failure.
 *
 * Validates: Requirements 23.4, 23.5
 */
export function useStoragePinStatus(cid: string | null | undefined): ExternalQueryResult<StoragePinStatus> {
  const { api, isReady } = useApi();

  const query = useQuery<StoragePinStatus>({
    queryKey: ['external', 'storage', cid],
    queryFn: async () => {
      if (!api) throw new Error('API not ready');
      const raw = await (api.query as any).storageService.pins(cid);
      const parsed = parseStoragePinData(raw);
      if (!parsed) throw new Error('Pin not found');
      return parsed;
    },
    enabled: isReady && !!api && !!cid,
    staleTime: STALE_TIMES.entity,
    retry: RETRY_CONFIG.chainQuery.retry,
    retryDelay: RETRY_CONFIG.chainQuery.retryDelay,
  });

  return toExternalResult(query);
}
