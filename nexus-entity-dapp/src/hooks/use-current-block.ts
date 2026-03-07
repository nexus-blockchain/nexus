'use client';

import { useEntityQuery } from './use-entity-query';

/**
 * Returns the current best block number.
 * Refreshes every 6 seconds (1 block time).
 */
export function useCurrentBlock(): number {
  const { data } = useEntityQuery<number>(
    ['system', 'currentBlock'],
    async (api) => {
      const header = await api.rpc.chain.getHeader();
      return header.number.toNumber();
    },
    { staleTime: 6_000, refetchInterval: 6_000 },
  );
  return data ?? 0;
}
