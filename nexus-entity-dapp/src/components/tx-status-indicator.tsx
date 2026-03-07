'use client';

import React from 'react';
import type { TxState } from '@/lib/types/models';

export interface TxStatusIndicatorProps {
  txState: TxState;
}

/**
 * Real-time transaction status display.
 * Hidden when status is 'idle'; otherwise shows status with appropriate color.
 */
export function TxStatusIndicator({ txState }: TxStatusIndicatorProps) {
  if (txState.status === 'idle') return null;

  const { label, color } = statusConfig[txState.status];

  return (
    <div className={`flex items-center gap-2 rounded-md px-3 py-2 text-sm ${color}`} data-testid="tx-status">
      <span data-testid="tx-status-label">{resolveLabel(label, txState)}</span>
    </div>
  );
}

const statusConfig: Record<Exclude<TxState['status'], 'idle'>, { label: string; color: string }> = {
  signing: {
    label: 'Waiting for signature...',
    color: 'bg-yellow-50 text-yellow-800',
  },
  broadcasting: {
    label: 'Broadcasting...',
    color: 'bg-blue-50 text-blue-800',
  },
  inBlock: {
    label: 'Included in block #{blockNumber}',
    color: 'bg-blue-50 text-blue-800',
  },
  finalized: {
    label: 'Finalized ✓',
    color: 'bg-green-50 text-green-800',
  },
  error: {
    label: '{error}',
    color: 'bg-red-50 text-red-800',
  },
};

function resolveLabel(template: string, txState: TxState): string {
  let result = template;
  if (txState.blockNumber !== null) {
    result = result.replace('{blockNumber}', String(txState.blockNumber));
  }
  if (txState.error !== null) {
    result = result.replace('{error}', txState.error);
  }
  if (txState.status === 'finalized' && txState.hash) {
    return `${result} (${txState.hash.slice(0, 10)}…)`;
  }
  return result;
}
