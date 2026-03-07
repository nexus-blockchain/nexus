'use client';

import React from 'react';
import { useEscrowStatus, type EscrowStatus } from '@/hooks/use-escrow-status';

const ESCROW_STATUS_CONFIG: Record<EscrowStatus, { label: string; color: string }> = {
  Held: { label: '托管中', color: 'text-blue-700 bg-blue-50' },
  Released: { label: '已释放', color: 'text-green-700 bg-green-50' },
  Refunded: { label: '已退回', color: 'text-yellow-700 bg-yellow-50' },
  Disputed: { label: '争议冻结', color: 'text-red-700 bg-red-50' },
};

export function EscrowStatusSection({ escrowId }: { escrowId: number }) {
  const { escrow, isLoading, isError } = useEscrowStatus(escrowId);

  if (isLoading) {
    return (
      <div className="mt-2 rounded-md border border-gray-100 bg-gray-50 px-3 py-2 text-xs text-gray-400 dark:border-gray-700 dark:bg-gray-800">
        托管状态加载中…
      </div>
    );
  }

  if (isError || !escrow) {
    return (
      <div className="mt-2 rounded-md border border-yellow-100 bg-yellow-50 px-3 py-2 text-xs text-yellow-600 dark:border-yellow-800 dark:bg-yellow-900/20 dark:text-yellow-400">
        ⚠ Escrow 数据暂不可用
      </div>
    );
  }

  const cfg = ESCROW_STATUS_CONFIG[escrow.status];

  return (
    <div className={`mt-2 rounded-md border px-3 py-2 text-xs ${cfg.color} border-current/10`}>
      <span className="font-medium">托管状态:</span> {cfg.label}
      <span className="ml-2 opacity-70">
        (ID: {escrow.id})
      </span>
    </div>
  );
}
