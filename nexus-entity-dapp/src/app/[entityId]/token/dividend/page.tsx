'use client';

import React, { useState, useCallback } from 'react';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useEntityQuery } from '@/hooks/use-entity-query';
import { useEntityMutation } from '@/hooks/use-entity-mutation';
import { PermissionGuard } from '@/components/permission-guard';
import { TxConfirmDialog } from '@/components/tx-confirm-dialog';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { AdminPermission } from '@/lib/types/models';
import { useWalletStore } from '@/stores/wallet-store';
import { STALE_TIMES } from '@/lib/chain/constants';

import { useTranslations } from 'next-intl';
// ─── Types ──────────────────────────────────────────────────

interface DividendConfig {
  dividendId: number;
  totalAmount: bigint;
  snapshotBlock: number;
  distributed: boolean;
}

interface ClaimStatus {
  claimed: boolean;
  amount: bigint;
}

// ─── Helpers ────────────────────────────────────────────────

function formatAmount(amount: bigint): string {
  return amount.toLocaleString();
}

function parseDividendConfigs(raw: unknown): DividendConfig[] {
  if (!raw) return [];
  const arr = Array.isArray(raw) ? raw : (raw as any).toArray?.() ?? [];
  return arr.map((item: any, idx: number) => {
    const obj = item.toJSON?.() ?? item;
    return {
      dividendId: Number(obj.dividendId ?? obj.dividend_id ?? idx),
      totalAmount: BigInt(String(obj.totalAmount ?? obj.total_amount ?? 0)),
      snapshotBlock: Number(obj.snapshotBlock ?? obj.snapshot_block ?? 0),
      distributed: Boolean(obj.distributed),
    };
  });
}

function parseClaimStatus(raw: unknown): ClaimStatus | null {
  if (!raw || (raw as any).isNone) return null;
  const unwrapped = (raw as any).unwrapOr?.(null) ?? raw;
  if (!unwrapped) return null;
  const obj = (unwrapped as any).toJSON?.() ?? unwrapped;
  return {
    claimed: Boolean(obj.claimed),
    amount: BigInt(String(obj.amount ?? 0)),
  };
}

// ─── Configure Dividend ─────────────────────────────────────

function ConfigureDividendSection() {
  const { entityId, isReadOnly, isSuspended } = useEntityContext();
  const [totalAmount, setTotalAmount] = useState('');
  const [snapshotBlock, setSnapshotBlock] = useState('');

  const configureDividend = useEntityMutation('entityToken', 'configureDividend', {
    invalidateKeys: [['entity', entityId, 'token', 'dividend']],
  });

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!totalAmount.trim() || !snapshotBlock.trim()) return;
      configureDividend.mutate([entityId, totalAmount.trim(), snapshotBlock.trim()]);
      setTotalAmount('');
      setSnapshotBlock('');
    },
    [entityId, totalAmount, snapshotBlock, configureDividend],
  );

  if (isReadOnly || isSuspended) return null;

  return (
    <PermissionGuard required={AdminPermission.TOKEN_MANAGE} fallback={null}>
      <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
        <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">配置分红</h2>
        <form onSubmit={handleSubmit} className="space-y-3">
          <input
            type="text"
            inputMode="decimal"
            value={totalAmount}
            onChange={(e) => setTotalAmount(e.target.value)}
            placeholder="分红总额"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
          <input
            type="text"
            inputMode="numeric"
            value={snapshotBlock}
            onChange={(e) => setSnapshotBlock(e.target.value)}
            placeholder="快照区块高度"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
          <div className="flex items-center gap-3">
            <button
              type="submit"
              disabled={configureDividend.txState.status === 'signing' || configureDividend.txState.status === 'broadcasting'}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              创建分红
            </button>
            <TxStatusIndicator txState={configureDividend.txState} />
          </div>
        </form>
      </section>
    </PermissionGuard>
  );
}

// ─── Dividend List with Distribute & Claim ──────────────────

function DividendList({ dividends }: { dividends: DividendConfig[] }) {
  const { entityId, isReadOnly, isSuspended } = useEntityContext();
  const address = useWalletStore((s) => s.address);

  const distributeDividend = useEntityMutation('entityToken', 'distributeDividend', {
    invalidateKeys: [['entity', entityId, 'token', 'dividend']],
  });

  const claimDividend = useEntityMutation('entityToken', 'claimDividend', {
    invalidateKeys: [['entity', entityId, 'token', 'dividend']],
  });

  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [showDistributeConfirm, setShowDistributeConfirm] = useState(false);

  // Query claim status for selected dividend
  const claimStatusQuery = useEntityQuery<ClaimStatus | null>(
    ['entity', entityId, 'token', 'dividend', 'claim', selectedId, address],
    async (api) => {
      if (selectedId === null || !address) return null;
      const raw = await (api.query as any).entityToken.dividendClaims(entityId, selectedId, address);
      return parseClaimStatus(raw);
    },
    {
      staleTime: STALE_TIMES.token,
      enabled: selectedId !== null && !!address,
    },
  );

  const handleDistribute = useCallback(
    (dividendId: number) => {
      setSelectedId(dividendId);
      setShowDistributeConfirm(true);
    },
    [],
  );

  const handleDistributeConfirm = useCallback(() => {
    if (selectedId === null) return;
    distributeDividend.mutate([entityId, selectedId]);
    setShowDistributeConfirm(false);
  }, [entityId, selectedId, distributeDividend]);

  const handleClaim = useCallback(
    (dividendId: number) => {
      claimDividend.mutate([entityId, dividendId]);
    },
    [entityId, claimDividend],
  );

  if (dividends.length === 0) {
    return (
      <div className="rounded-lg border border-dashed border-gray-300 p-6 text-center text-sm text-gray-500 dark:border-gray-600">
        暂无分红记录
      </div>
    );
  }

  const isTxBusy = (m: ReturnType<typeof useEntityMutation>) =>
    m.txState.status === 'signing' || m.txState.status === 'broadcasting';

  return (
    <>
      <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
        <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">
          分红列表 ({dividends.length})
        </h2>
        <div className="overflow-x-auto">
          <table className="w-full text-left text-sm">
            <thead>
              <tr className="border-b text-xs text-gray-500 dark:border-gray-700">
                <th className="pb-2 pr-4">ID</th>
                <th className="pb-2 pr-4">总额</th>
                <th className="pb-2 pr-4">快照区块</th>
                <th className="pb-2 pr-4">状态</th>
                <th className="pb-2 text-right">操作</th>
              </tr>
            </thead>
            <tbody>
              {dividends.map((d) => (
                <tr key={d.dividendId} className="border-b last:border-0 dark:border-gray-700">
                  <td className="py-2 pr-4 text-gray-900 dark:text-gray-100">{d.dividendId}</td>
                  <td className="py-2 pr-4 text-gray-900 dark:text-gray-100">{formatAmount(d.totalAmount)}</td>
                  <td className="py-2 pr-4 text-gray-700 dark:text-gray-300">{d.snapshotBlock}</td>
                  <td className="py-2 pr-4">
                    <span
                      className={`rounded-full px-2 py-0.5 text-xs font-medium ${
                        d.distributed
                          ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300'
                          : 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-300'
                      }`}
                    >
                      {d.distributed ? '已分发' : '待分发'}
                    </span>
                  </td>
                  <td className="py-2 text-right">
                    <div className="flex items-center justify-end gap-2">
                      {!d.distributed && !isReadOnly && !isSuspended && (
                        <PermissionGuard required={AdminPermission.TOKEN_MANAGE} fallback={null}>
                          <button
                            type="button"
                            onClick={() => handleDistribute(d.dividendId)}
                            disabled={isTxBusy(distributeDividend)}
                            className="rounded-md bg-blue-600 px-3 py-1 text-xs text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
                          >
                            分发
                          </button>
                        </PermissionGuard>
                      )}
                      {d.distributed && address && (
                        <button
                          type="button"
                          onClick={() => {
                            setSelectedId(d.dividendId);
                            handleClaim(d.dividendId);
                          }}
                          disabled={isTxBusy(claimDividend) || claimStatusQuery.data?.claimed === true}
                          className="rounded-md bg-green-600 px-3 py-1 text-xs text-white hover:bg-green-700 disabled:cursor-not-allowed disabled:opacity-50"
                        >
                          {claimStatusQuery.data?.claimed ? '已领取' : '领取'}
                        </button>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="mt-3 flex items-center gap-3">
          <TxStatusIndicator txState={distributeDividend.txState} />
          <TxStatusIndicator txState={claimDividend.txState} />
        </div>
      </section>

      <TxConfirmDialog
        open={showDistributeConfirm}
        onClose={() => setShowDistributeConfirm(false)}
        onConfirm={handleDistributeConfirm}
        config={{
          title: '确认分发分红',
          description: '分发后将按快照区块的持仓比例向所有持有人分配分红，此操作不可撤销。',
          severity: 'warning',
        }}
      />
    </>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function DividendPage() {
  const t = useTranslations('token');
  const { entityId } = useEntityContext();

  const dividendsQuery = useEntityQuery<DividendConfig[]>(
    ['entity', entityId, 'token', 'dividend'],
    async (api) => {
      const raw = await (api.query as any).entityToken.dividendConfigs(entityId);
      return parseDividendConfigs(raw);
    },
    { staleTime: STALE_TIMES.token },
  );

  if (dividendsQuery.isLoading) {
    return (
      <div className="flex items-center justify-center p-12 text-sm text-gray-500">
        {t('dividend.loading')}
      </div>
    );
  }

  if (dividendsQuery.error) {
    return (
      <div className="flex items-center justify-center p-12 text-sm text-red-500">
        加载失败: {String(dividendsQuery.error)}
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-4xl space-y-6 p-4 sm:p-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('dividend.title')}</h1>
      <ConfigureDividendSection />
      <DividendList dividends={dividendsQuery.data ?? []} />
    </div>
  );
}
