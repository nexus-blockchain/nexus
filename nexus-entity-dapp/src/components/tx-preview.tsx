'use client';

import React from 'react';

export interface TxPreviewProps {
  pallet: string;
  call: string;
  params: Record<string, unknown>;
  estimatedFee?: string;
}

/**
 * Displays a summary of a pending transaction: pallet, call, parameters, and estimated fee.
 */
export function TxPreview({ pallet, call, params, estimatedFee }: TxPreviewProps) {
  const paramEntries = Object.entries(params);

  return (
    <div className="rounded-lg border border-gray-200 bg-gray-50 p-4 text-sm" data-testid="tx-preview">
      <h3 className="mb-3 font-semibold text-gray-900">Transaction Preview</h3>

      <dl className="space-y-2">
        <div className="flex justify-between">
          <dt className="text-gray-500">Pallet</dt>
          <dd className="font-mono text-gray-900" data-testid="tx-preview-pallet">{pallet}</dd>
        </div>

        <div className="flex justify-between">
          <dt className="text-gray-500">Call</dt>
          <dd className="font-mono text-gray-900" data-testid="tx-preview-call">{call}</dd>
        </div>

        {paramEntries.length > 0 && (
          <div>
            <dt className="mb-1 text-gray-500">Parameters</dt>
            <dd>
              <ul className="space-y-1 rounded-md bg-white p-2" data-testid="tx-preview-params">
                {paramEntries.map(([key, value]) => (
                  <li key={key} className="flex justify-between gap-2">
                    <span className="text-gray-500">{key}</span>
                    <span className="truncate font-mono text-gray-900">{formatParamValue(value)}</span>
                  </li>
                ))}
              </ul>
            </dd>
          </div>
        )}

        <div className="flex justify-between border-t border-gray-200 pt-2">
          <dt className="text-gray-500">Estimated Fee</dt>
          <dd className="font-mono text-gray-900" data-testid="tx-preview-fee">
            {estimatedFee ?? '—'}
          </dd>
        </div>
      </dl>
    </div>
  );
}

function formatParamValue(value: unknown): string {
  if (value === null || value === undefined) return '—';
  if (typeof value === 'bigint') return value.toString();
  if (typeof value === 'object') return JSON.stringify(value);
  return String(value);
}
