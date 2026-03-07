'use client';

import React from 'react';
import { OrderStatus } from '@/lib/types/enums';

/** Default order timeout: ~7 days at 6s block time = 100800 blocks */
const ORDER_TIMEOUT_BLOCKS = 100_800;
/** Warning threshold: show warning when < 20% time remaining */
const WARNING_THRESHOLD = 0.2;

interface OrderTimeoutWarningProps {
  updatedAt: number;
  currentBlock: number;
  status: OrderStatus;
}

/** Statuses where timeout is relevant (pending action) */
const TIMEOUT_RELEVANT_STATUSES = new Set<OrderStatus>([
  OrderStatus.Created,
  OrderStatus.Paid,
  OrderStatus.Shipped,
  OrderStatus.ServiceStarted,
  OrderStatus.ServiceCompleted,
  OrderStatus.RefundRequested,
]);

/**
 * Shows a timeout warning when an order is approaching expiry.
 * Only displayed for orders in active (non-terminal) statuses.
 */
export function OrderTimeoutWarning({ updatedAt, currentBlock, status }: OrderTimeoutWarningProps) {
  if (!TIMEOUT_RELEVANT_STATUSES.has(status)) return null;
  if (currentBlock <= 0 || updatedAt <= 0) return null;

  const expiryBlock = updatedAt + ORDER_TIMEOUT_BLOCKS;
  const remaining = expiryBlock - currentBlock;

  if (remaining <= 0) {
    return (
      <div className="mt-2 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-700 dark:border-red-800 dark:bg-red-900/20 dark:text-red-400">
        ⏰ 该订单已超时，请及时处理或取消。
      </div>
    );
  }

  const progress = remaining / ORDER_TIMEOUT_BLOCKS;
  if (progress > WARNING_THRESHOLD) return null;

  const hoursRemaining = Math.round((remaining * 6) / 3600);

  return (
    <div className="mt-2 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-700 dark:border-amber-800 dark:bg-amber-900/20 dark:text-amber-400">
      ⏰ 订单即将超时，剩余约 {hoursRemaining > 0 ? `${hoursRemaining} 小时` : '不足 1 小时'}，请尽快处理。
    </div>
  );
}
