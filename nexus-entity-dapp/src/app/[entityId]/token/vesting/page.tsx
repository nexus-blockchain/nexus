'use client';

import React, { useState, useCallback } from 'react';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useEntityQuery } from '@/hooks/use-entity-query';
import { useEntityMutation } from '@/hooks/use-entity-mutation';
import { PermissionGuard } from '@/components/permission-guard';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { AdminPermission } from '@/lib/types/models';
import { STALE_TIMES } from '@/lib/chain/constants';

import { useTranslations } from 'next-intl';
// ─── Types ──────────────────────────────────────────────────

interface VestingEntry {
  account: string;
  amount: bigint;
  cliffBlocks: number;
  vestingBlocks: number;
  startBlock: number;
}

// ─── Helpers ────────────────────────────────────────────────

function formatAmount(amount: bigint): string {
  return amount.toLocaleString();
}

function shortenAddress(addr: string): string {
  if (addr.length <= 12) return addr;
  return `${addr.slice(0, 6)}…${addr.slice(-6)}`;
}

function parseVestingEntries(rawEntries: [any, any][]): VestingEntry[] {
  const results: VestingEntry[] = [];
  for (const [storageKey, value] of rawEntries) {
    const keys = storageKey.args ?? storageKey.toHuman?.()?.args ?? [];
    const account = String(keys[keys.length - 1] ?? '');
    const arr = value.toJSON?.() ?? value;
    const schedules = Array.isArray(arr) ? arr : [arr];
    for (const s of schedules) {
      if (!s) continue;
      results.push({
        account,
        amount: BigInt(String(s.amount ?? s.locked ?? 0)),
        cliffBlocks: Number(s.cliffBlocks ?? s.cliff_blocks ?? 0),
        vestingBlocks: Number(s.vestingBlocks ?? s.vesting_blocks ?? 0),
        startBlock: Number(s.startBlock ?? s.start_block ?? 0),
      });
    }
  }
  return results;
}

// ─── Lock Tokens Form ───────────────────────────────────────

function LockTokensSection() {
  const { entityId, isReadOnly, isSuspended } = useEntityContext();
  const [account, setAccount] = useState('');
  const [amount, setAmount] = useState('');
  const [cliffBlocks, setCliffBlocks] = useState('');
  const [vestingBlocks, setVestingBlocks] = useState('');

  const lockTokens = useEntityMutation('entityToken', 'lockTokens', {
    invalidateKeys: [['entity', entityId, 'token', 'vesting']],
  });

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!account.trim() || !amount.trim() || !cliffBlocks.trim() || !vestingBlocks.trim()) return;
      lockTokens.mutate([entityId, account.trim(), amount.trim(), cliffBlocks.trim(), vestingBlocks.trim()]);
      setAccount('');
      setAmount('');
      setCliffBlocks('');
      setVestingBlocks('');
    },
    [entityId, account, amount, cliffBlocks, vestingBlocks, lockTokens],
  );

  if (isReadOnly || isSuspended) return null;

  return (
    <PermissionGuard required={AdminPermission.TOKEN_MANAGE} fallback={null}>
      <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
        <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">锁定代币</h2>
        <p className="mb-3 text-xs text-gray-500 dark:text-gray-400">
          每个账户最多 10 条锁仓记录
        </p>
        <form onSubmit={handleSubmit} className="space-y-3">
          <input
            type="text"
            value={account}
            onChange={(e) => setAccount(e.target.value)}
            placeholder="目标账户地址"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
          <input
            type="text"
            inputMode="decimal"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            placeholder="锁定数量"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
          <div className="grid gap-3 sm:grid-cols-2">
            <input
              type="text"
              inputMode="numeric"
              value={cliffBlocks}
              onChange={(e) => setCliffBlocks(e.target.value)}
              placeholder="悬崖期 (区块数)"
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              required
            />
            <input
              type="text"
              inputMode="numeric"
              value={vestingBlocks}
              onChange={(e) => setVestingBlocks(e.target.value)}
              placeholder="释放期 (区块数)"
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              required
            />
          </div>
          <div className="flex items-center gap-3">
            <button
              type="submit"
              disabled={lockTokens.txState.status === 'signing' || lockTokens.txState.status === 'broadcasting'}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              锁定
            </button>
            <TxStatusIndicator txState={lockTokens.txState} />
          </div>
        </form>
      </section>
    </PermissionGuard>
  );
}

// ─── Vesting Entries Table ──────────────────────────────────

function VestingEntriesTable({ entries }: { entries: VestingEntry[] }) {
  const { entityId, isReadOnly, isSuspended } = useEntityContext();

  const releaseVestedTokens = useEntityMutation('entityToken', 'releaseVestedTokens', {
    invalidateKeys: [['entity', entityId, 'token', 'vesting']],
  });

  const handleRelease = useCallback(
    (account: string) => {
      releaseVestedTokens.mutate([entityId, account]);
    },
    [entityId, releaseVestedTokens],
  );

  const isTxBusy =
    releaseVestedTokens.txState.status === 'signing' ||
    releaseVestedTokens.txState.status === 'broadcasting';

  // Group entries by account
  const grouped = entries.reduce<Record<string, VestingEntry[]>>((acc, entry) => {
    (acc[entry.account] ??= []).push(entry);
    return acc;
  }, {});

  if (entries.length === 0) {
    return (
      <div className="rounded-lg border border-dashed border-gray-300 p-6 text-center text-sm text-gray-500 dark:border-gray-600">
        暂无锁仓记录
      </div>
    );
  }

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">
        锁仓记录 ({entries.length})
      </h2>

      <div className="space-y-4">
        {Object.entries(grouped).map(([account, accountEntries]) => (
          <div key={account} className="rounded-md border border-gray-100 bg-gray-50 p-4 dark:border-gray-700 dark:bg-gray-800">
            <div className="mb-2 flex items-center justify-between">
              <span className="font-mono text-xs text-gray-700 dark:text-gray-300">
                {shortenAddress(account)}
              </span>
              <div className="flex items-center gap-2">
                <span className="text-xs text-gray-500">{accountEntries.length}/10 条</span>
                {!isReadOnly && !isSuspended && (
                  <PermissionGuard required={AdminPermission.TOKEN_MANAGE} fallback={null}>
                    <button
                      type="button"
                      onClick={() => handleRelease(account)}
                      disabled={isTxBusy}
                      className="rounded-md bg-green-600 px-3 py-1 text-xs text-white hover:bg-green-700 disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      释放
                    </button>
                  </PermissionGuard>
                )}
              </div>
            </div>

            <div className="overflow-x-auto">
              <table className="w-full text-left text-sm">
                <thead>
                  <tr className="border-b text-xs text-gray-500 dark:border-gray-600">
                    <th className="pb-1 pr-3">数量</th>
                    <th className="pb-1 pr-3">悬崖期</th>
                    <th className="pb-1 pr-3">释放期</th>
                    <th className="pb-1">起始区块</th>
                  </tr>
                </thead>
                <tbody>
                  {accountEntries.map((entry, idx) => (
                    <tr key={idx} className="border-b last:border-0 dark:border-gray-600">
                      <td className="py-1.5 pr-3 text-gray-900 dark:text-gray-100">{formatAmount(entry.amount)}</td>
                      <td className="py-1.5 pr-3 text-gray-700 dark:text-gray-300">{entry.cliffBlocks} 块</td>
                      <td className="py-1.5 pr-3 text-gray-700 dark:text-gray-300">{entry.vestingBlocks} 块</td>
                      <td className="py-1.5 text-gray-700 dark:text-gray-300">{entry.startBlock}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        ))}
      </div>

      <div className="mt-3">
        <TxStatusIndicator txState={releaseVestedTokens.txState} />
      </div>
    </section>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function VestingPage() {
  const t = useTranslations('token');
  const { entityId } = useEntityContext();

  const vestingQuery = useEntityQuery<VestingEntry[]>(
    ['entity', entityId, 'token', 'vesting'],
    async (api) => {
      const rawEntries = await (api.query as any).entityToken.vestingSchedules.entries(entityId);
      return parseVestingEntries(rawEntries);
    },
    { staleTime: STALE_TIMES.token },
  );

  if (vestingQuery.isLoading) {
    return (
      <div className="flex items-center justify-center p-12 text-sm text-gray-500">
        {t('vesting.loading')}
      </div>
    );
  }

  if (vestingQuery.error) {
    return (
      <div className="flex items-center justify-center p-12 text-sm text-red-500">
        加载失败: {String(vestingQuery.error)}
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-4xl space-y-6 p-4 sm:p-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('vesting.title')}</h1>
      <LockTokensSection />
      <VestingEntriesTable entries={vestingQuery.data ?? []} />
    </div>
  );
}
