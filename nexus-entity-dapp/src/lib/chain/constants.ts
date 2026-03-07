/** React Query staleTime configuration per module (in ms) */
export const STALE_TIMES = {
  entity:     30_000,   // 30s
  shops:      30_000,   // 30s
  products:   15_000,   // 15s
  orders:     10_000,   // 10s
  token:      60_000,   // 60s
  orderBook:   5_000,   // 5s
  members:    30_000,   // 30s
  proposals:  30_000,   // 30s
} as const;

/** React Query retry configuration */
export const RETRY_CONFIG = {
  chainQuery: {
    retry: 3,
    retryDelay: (attempt: number) => Math.min(1000 * 2 ** attempt, 10000),
  },
  ipfsContent: {
    retry: 2,
    retryDelay: 2000,
  },
} as const;

/** Dangerous operations that require confirmation dialog */
export const DANGEROUS_OPERATIONS = [
  'transferOwnership',
  'requestCloseEntity',
  'lockGovernance',
  'banMember',
  'burnTokens',
] as const;
