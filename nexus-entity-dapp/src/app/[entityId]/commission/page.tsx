'use client';

import React, { useState, useCallback } from 'react';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { PermissionGuard } from '@/components/permission-guard';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { AdminPermission } from '@/lib/types/models';
import { CommissionPlugin } from '@/lib/types/enums';
import { useCommission } from '@/hooks/use-commission';
import { useWalletStore } from '@/stores/wallet-store';

import { useTranslations } from 'next-intl';
// ─── Helpers ────────────────────────────────────────────────

function isTxBusy(m: { txState: { status: string } }): boolean {
  return m.txState.status === 'signing' || m.txState.status === 'broadcasting';
}

const PLUGIN_LIST: { key: CommissionPlugin; label: string; bit: number }[] = [
  { key: CommissionPlugin.Referral, label: '直推奖励', bit: 0x001 },
  { key: CommissionPlugin.MultiLevel, label: '多级分销', bit: 0x002 },
  { key: CommissionPlugin.LevelDiff, label: '等级差价', bit: 0x008 },
  { key: CommissionPlugin.SingleLine, label: '单线收益', bit: 0x080 },
  { key: CommissionPlugin.Team, label: '团队业绩', bit: 0x004 },
  { key: CommissionPlugin.PoolReward, label: '沉淀池奖励', bit: 0x200 },
];

// ─── Overview Section ───────────────────────────────────────

function OverviewSection() {
  const { entityId } = useEntityContext();
  const { config, setCommissionRate } = useCommission();
  const [rate, setRate] = useState('');

  const handleSetRate = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!rate.trim()) return;
      setCommissionRate.mutate([entityId, Number(rate)]);
      setRate('');
    },
    [entityId, rate, setCommissionRate],
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">佣金系统概览</h2>
      <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
        <div>
          <p className="text-xs text-gray-500">启用状态</p>
          <span
            className={`inline-block rounded-full px-2 py-0.5 text-xs font-medium ${
              config?.enabled
                ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300'
                : 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300'
            }`}
          >
            {config?.enabled ? '已启用' : '未启用'}
          </span>
        </div>
        <div>
          <p className="text-xs text-gray-500">基础费率 (bps)</p>
          <p className="text-sm font-medium text-gray-900 dark:text-gray-100">{config?.baseRate ?? 0}</p>
        </div>
        <div>
          <p className="text-xs text-gray-500">启用模式位</p>
          <p className="font-mono text-sm font-medium text-gray-900 dark:text-gray-100">
            0x{(config?.enabledModes ?? 0).toString(16).toUpperCase().padStart(3, '0')}
          </p>
        </div>
        <div>
          <p className="text-xs text-gray-500">提现状态</p>
          <span
            className={`inline-block rounded-full px-2 py-0.5 text-xs font-medium ${
              config?.withdrawalPaused
                ? 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300'
                : 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300'
            }`}
          >
            {config?.withdrawalPaused ? '已暂停' : '正常'}
          </span>
        </div>
      </div>

      <PermissionGuard required={AdminPermission.COMMISSION_MANAGE} fallback={null}>
        <form onSubmit={handleSetRate} className="mt-4 flex items-center gap-3 border-t border-gray-100 pt-4 dark:border-gray-700">
          <input
            type="number"
            value={rate}
            onChange={(e) => setRate(e.target.value)}
            placeholder="新费率 (bps)"
            className="w-40 rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
          />
          <button
            type="submit"
            disabled={isTxBusy(setCommissionRate)}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            设置费率
          </button>
          <TxStatusIndicator txState={setCommissionRate.txState} />
        </form>
      </PermissionGuard>
    </section>
  );
}

// ─── Plugin Config Section ──────────────────────────────────

function PluginSection() {
  const { entityId } = useEntityContext();
  const { config, enablePlugin, disablePlugin } = useCommission();
  const enabledModes = config?.enabledModes ?? 0;

  const handleToggle = useCallback(
    (plugin: CommissionPlugin, bit: number) => {
      const isEnabled = (enabledModes & bit) !== 0;
      if (isEnabled) {
        disablePlugin.mutate([entityId, plugin]);
      } else {
        enablePlugin.mutate([entityId, plugin]);
      }
    },
    [entityId, enabledModes, enablePlugin, disablePlugin],
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">插件配置</h2>
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {PLUGIN_LIST.map(({ key, label, bit }) => {
          const isEnabled = (enabledModes & bit) !== 0;
          return (
            <div
              key={key}
              className="flex items-center justify-between rounded-md border border-gray-100 px-4 py-3 dark:border-gray-700"
            >
              <div>
                <p className="text-sm font-medium text-gray-900 dark:text-gray-100">{label}</p>
                <p className="text-xs text-gray-500">{key}</p>
              </div>
              <button
                type="button"
                onClick={() => handleToggle(key, bit)}
                disabled={isTxBusy(enablePlugin) || isTxBusy(disablePlugin)}
                className={`rounded-md px-3 py-1 text-xs text-white disabled:cursor-not-allowed disabled:opacity-50 ${
                  isEnabled
                    ? 'bg-red-500 hover:bg-red-600'
                    : 'bg-green-600 hover:bg-green-700'
                }`}
              >
                {isEnabled ? '禁用' : '启用'}
              </button>
            </div>
          );
        })}
      </div>
      <div className="mt-3 flex items-center gap-3">
        <TxStatusIndicator txState={enablePlugin.txState} />
        <TxStatusIndicator txState={disablePlugin.txState} />
      </div>
    </section>
  );
}

// ─── Withdrawal Config Section ──────────────────────────────

function WithdrawalSection() {
  const { entityId } = useEntityContext();
  const { config, configureWithdrawal, pauseWithdrawal, resumeWithdrawal } = useCommission();

  const [minAmount, setMinAmount] = useState('');
  const [feeRate, setFeeRate] = useState('');
  const [cooldown, setCooldown] = useState('');

  const handleConfigure = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!minAmount.trim() || !feeRate.trim() || !cooldown.trim()) return;
      configureWithdrawal.mutate([entityId, minAmount.trim(), Number(feeRate), Number(cooldown)]);
      setMinAmount('');
      setFeeRate('');
      setCooldown('');
    },
    [entityId, minAmount, feeRate, cooldown, configureWithdrawal],
  );

  const wc = config?.withdrawalConfig;

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">提现管理</h2>
        {config?.withdrawalPaused && (
          <span className="rounded-full bg-red-100 px-2 py-0.5 text-xs font-medium text-red-800 dark:bg-red-900/30 dark:text-red-300">
            提现已暂停
          </span>
        )}
      </div>

      {wc && (
        <div className="mb-4 grid grid-cols-3 gap-4">
          <div>
            <p className="text-xs text-gray-500">最低提现额</p>
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100">{wc.minAmount.toString()}</p>
          </div>
          <div>
            <p className="text-xs text-gray-500">手续费率 (bps)</p>
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100">{wc.feeRate}</p>
          </div>
          <div>
            <p className="text-xs text-gray-500">冷却期 (区块)</p>
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100">{wc.cooldown}</p>
          </div>
        </div>
      )}

      <form onSubmit={handleConfigure} className="space-y-3 border-t border-gray-100 pt-4 dark:border-gray-700">
        <div className="grid grid-cols-3 gap-3">
          <input
            type="text"
            inputMode="decimal"
            value={minAmount}
            onChange={(e) => setMinAmount(e.target.value)}
            placeholder="最低提现额"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
          />
          <input
            type="number"
            value={feeRate}
            onChange={(e) => setFeeRate(e.target.value)}
            placeholder="手续费率 (bps)"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
          />
          <input
            type="number"
            value={cooldown}
            onChange={(e) => setCooldown(e.target.value)}
            placeholder="冷却期 (区块)"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
          />
        </div>
        <div className="flex items-center gap-3">
          <button
            type="submit"
            disabled={isTxBusy(configureWithdrawal)}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            更新提现配置
          </button>
          <button
            type="button"
            onClick={() =>
              config?.withdrawalPaused
                ? resumeWithdrawal.mutate([entityId])
                : pauseWithdrawal.mutate([entityId])
            }
            disabled={isTxBusy(pauseWithdrawal) || isTxBusy(resumeWithdrawal)}
            className={`rounded-md px-4 py-2 text-sm text-white disabled:cursor-not-allowed disabled:opacity-50 ${
              config?.withdrawalPaused
                ? 'bg-green-600 hover:bg-green-700'
                : 'bg-red-500 hover:bg-red-600'
            }`}
          >
            {config?.withdrawalPaused ? '恢复提现' : '暂停提现'}
          </button>
          <TxStatusIndicator txState={configureWithdrawal.txState} />
          <TxStatusIndicator txState={pauseWithdrawal.txState} />
          <TxStatusIndicator txState={resumeWithdrawal.txState} />
        </div>
      </form>
    </section>
  );
}

// ─── Withdraw Section (NEX + Token dual channel) ────────────

function WithdrawSection() {
  const { entityId } = useEntityContext();
  const { config, withdrawNex, withdrawToken } = useCommission();
  const [nexAmount, setNexAmount] = useState('');
  const [tokenAmount, setTokenAmount] = useState('');
  const paused = config?.withdrawalPaused ?? false;

  const handleWithdrawNex = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!nexAmount.trim()) return;
      withdrawNex.mutate([entityId, nexAmount.trim()]);
      setNexAmount('');
    },
    [entityId, nexAmount, withdrawNex],
  );

  const handleWithdrawToken = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!tokenAmount.trim()) return;
      withdrawToken.mutate([entityId, tokenAmount.trim()]);
      setTokenAmount('');
    },
    [entityId, tokenAmount, withdrawToken],
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">佣金提现</h2>
      {paused && (
        <div className="mb-4 rounded-md bg-red-50 p-3 text-sm text-red-800 dark:bg-red-900/30 dark:text-red-300">
          ⚠️ 提现功能已暂停，请联系管理员
        </div>
      )}
      <div className="grid gap-6 md:grid-cols-2">
        <form onSubmit={handleWithdrawNex} className="space-y-3">
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">NEX 提现</h3>
          <input
            type="text"
            inputMode="decimal"
            value={nexAmount}
            onChange={(e) => setNexAmount(e.target.value)}
            placeholder="提现金额"
            disabled={paused}
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
          />
          <div className="flex items-center gap-3">
            <button
              type="submit"
              disabled={paused || isTxBusy(withdrawNex)}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              提取 NEX
            </button>
            <TxStatusIndicator txState={withdrawNex.txState} />
          </div>
        </form>
        <form onSubmit={handleWithdrawToken} className="space-y-3">
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">Token 提现</h3>
          <input
            type="text"
            inputMode="decimal"
            value={tokenAmount}
            onChange={(e) => setTokenAmount(e.target.value)}
            placeholder="提现金额"
            disabled={paused}
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
          />
          <div className="flex items-center gap-3">
            <button
              type="submit"
              disabled={paused || isTxBusy(withdrawToken)}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              提取 Token
            </button>
            <TxStatusIndicator txState={withdrawToken.txState} />
          </div>
        </form>
      </div>
    </section>
  );
}

// ─── Member Commission Stats ────────────────────────────────

function MemberStatsSection() {
  const address = useWalletStore((s) => s.address);
  const { useMemberCommission } = useCommission();
  const { data } = useMemberCommission(address);

  if (!address) {
    return (
      <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
        <h2 className="mb-2 text-lg font-semibold text-gray-900 dark:text-gray-100">我的佣金</h2>
        <p className="text-sm text-gray-400">请先连接钱包</p>
      </section>
    );
  }

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">我的佣金</h2>
      <div className="grid grid-cols-2 gap-4">
        <div>
          <p className="text-xs text-gray-500">NEX 累计收益</p>
          <p className="text-sm font-medium text-gray-900 dark:text-gray-100">{data?.nexEarned.toString() ?? '0'}</p>
        </div>
        <div>
          <p className="text-xs text-gray-500">Token 累计收益</p>
          <p className="text-sm font-medium text-gray-900 dark:text-gray-100">{data?.tokenEarned.toString() ?? '0'}</p>
        </div>
      </div>
    </section>
  );
}

// ─── Order Commission Records ───────────────────────────────

function OrderCommissionsSection() {
  const { orderCommissions } = useCommission();

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">订单佣金记录</h2>
      {orderCommissions.length === 0 ? (
        <p className="py-8 text-center text-sm text-gray-400">暂无佣金记录</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-left text-sm">
            <thead>
              <tr className="border-b text-xs text-gray-500 dark:border-gray-700">
                <th className="pb-2 pr-4">订单 ID</th>
                <th className="pb-2 pr-4">金额</th>
                <th className="pb-2">插件类型</th>
              </tr>
            </thead>
            <tbody>
              {orderCommissions.map((oc) => (
                <tr key={`${oc.orderId}-${oc.plugin}`} className="border-b last:border-0 dark:border-gray-700">
                  <td className="py-2 pr-4 text-gray-900 dark:text-gray-100">#{oc.orderId}</td>
                  <td className="py-2 pr-4 text-gray-900 dark:text-gray-100">{oc.amount.toString()}</td>
                  <td className="py-2 text-gray-500">{oc.plugin}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function CommissionPage() {
  const t = useTranslations('commission');
  const { isLoading, error } = useCommission();

  if (isLoading) {
    return <div className="flex items-center justify-center p-12 text-sm text-gray-500">{t('loading')}</div>;
  }

  if (error) {
    return <div className="flex items-center justify-center p-12 text-sm text-red-500">加载失败: {String(error)}</div>;
  }

  return (
    <div className="mx-auto max-w-5xl space-y-6 p-4 sm:p-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('title')}</h1>

      <OverviewSection />
      <MemberStatsSection />

      <PermissionGuard required={AdminPermission.COMMISSION_MANAGE} fallback={null}>
        <PluginSection />
        <WithdrawalSection />
      </PermissionGuard>

      <WithdrawSection />
      <OrderCommissionsSection />
    </div>
  );
}
