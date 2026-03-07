'use client';

import { useEntityQuery } from './use-entity-query';

/** Escrow status as returned by pallet-dispute-escrow */
export type EscrowStatus = 'Held' | 'Released' | 'Refunded' | 'Disputed';

export interface EscrowData {
  id: number;
  status: EscrowStatus;
  amount: bigint;
  depositor: string;
  beneficiary: string;
}

function parseEscrowData(raw: unknown, escrowId: number): EscrowData | null {
  if (!raw || (raw as { isNone?: boolean }).isNone) return null;
  const unwrapped = (raw as { unwrapOr?: (d: null) => unknown }).unwrapOr?.(null) ?? raw;
  if (!unwrapped) return null;
  const obj = unwrapped as Record<string, unknown>;
  return {
    id: escrowId,
    status: String(obj.status ?? 'Held') as EscrowStatus,
    amount: BigInt(String(obj.amount ?? 0)),
    depositor: String(obj.depositor ?? ''),
    beneficiary: String(obj.beneficiary ?? ''),
  };
}

/**
 * Query escrow status from pallet-dispute-escrow.
 * Returns null with graceful degradation on failure.
 */
export function useEscrowStatus(escrowId: number | null) {
  const query = useEntityQuery<EscrowData | null>(
    ['escrow', escrowId],
    async (api) => {
      if (escrowId == null) return null;
      const raw = await (api.query as any).disputeEscrow.escrows(escrowId);
      return parseEscrowData(raw, escrowId);
    },
    { staleTime: 15_000, enabled: escrowId != null },
  );

  return {
    escrow: query.data ?? null,
    isLoading: query.isLoading,
    isError: query.isError,
  };
}
