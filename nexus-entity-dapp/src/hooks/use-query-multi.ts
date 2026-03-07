'use client';

import { useQuery, type UseQueryResult } from '@tanstack/react-query';
import type { ApiPromise } from '@polkadot/api';
import { useApi } from '@/lib/chain';

/**
 * Hook for batch chain queries using api.queryMulti()
 */
export function useQueryMulti<T extends unknown[]>(
  queryKey: unknown[],
  queries: (api: ApiPromise) => [any, ...any[]][],
  transform: (results: any[]) => T,
  options?: { staleTime?: number; enabled?: boolean },
): UseQueryResult<T> {
  const { api, isReady } = useApi();

  return useQuery<T>({
    queryKey,
    queryFn: async () => {
      if (!api) throw new Error('API not ready');
      const queryDefs = queries(api);
      const results = await api.queryMulti(queryDefs as any);
      return transform(results as any[]);
    },
    enabled: isReady && !!api && (options?.enabled !== false),
    staleTime: options?.staleTime,
  });
}
