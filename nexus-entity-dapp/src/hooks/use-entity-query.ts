'use client';

import { useQuery, type UseQueryResult } from '@tanstack/react-query';
import type { ApiPromise } from '@polkadot/api';
import { useApi } from '@/lib/chain';
import { RETRY_CONFIG } from '@/lib/chain/constants';

/**
 * Generic hook for entity-scoped chain queries.
 * Automatically integrates with the shared ApiPromise instance.
 */
export function useEntityQuery<T>(
  queryKey: unknown[],
  queryFn: (api: ApiPromise) => Promise<T>,
  options?: {
    staleTime?: number;
    enabled?: boolean;
    refetchInterval?: number;
  },
): UseQueryResult<T> {
  const { api, isReady } = useApi();

  return useQuery<T>({
    queryKey,
    queryFn: () => {
      if (!api) throw new Error('API not ready');
      return queryFn(api);
    },
    enabled: isReady && !!api && (options?.enabled !== false),
    staleTime: options?.staleTime,
    refetchInterval: options?.refetchInterval,
    retry: RETRY_CONFIG.chainQuery.retry,
    retryDelay: RETRY_CONFIG.chainQuery.retryDelay,
  });
}
