'use client';

import React from 'react';

/**
 * Displayed when an order is in Disputed status.
 * Provides guidance on the arbitration process.
 */
export function DisputeNotice() {
  return (
    <div className="mt-2 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-700 dark:border-red-800 dark:bg-red-900/20 dark:text-red-400">
      <p className="font-medium">⚠ 该订单已进入争议状态</p>
      <p className="mt-1 opacity-80">
        争议将由平台仲裁员处理。请保留相关交易凭证，等待仲裁结果。
        如需提交补充材料，请通过平台争议中心操作。
      </p>
    </div>
  );
}
