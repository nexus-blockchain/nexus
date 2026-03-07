'use client';

import React, { useState, useCallback } from 'react';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useShops } from '@/hooks/use-shops';
import { PermissionGuard } from '@/components/permission-guard';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { AdminPermission } from '@/lib/types/models';
import { ShopType, EffectiveShopStatus } from '@/lib/types/enums';
import type { ShopData } from '@/lib/types/models';
import { isFundWarning } from '@/lib/utils/fund-warning';

import { useTranslations } from 'next-intl';
// ─── Constants ──────────────────────────────────────────────

/** Fund warning threshold: 1 NEX (10^12) */
const FUND_WARNING_THRESHOLD = BigInt('1000000000000');

const SHOP_TYPE_LABELS: Record<ShopType, string> = {
  [ShopType.OnlineStore]: '线上商店',
  [ShopType.PhysicalStore]: '实体店铺',
  [ShopType.ServicePoint]: '服务网点',
  [ShopType.Warehouse]: '仓储',
  [ShopType.Franchise]: '加盟店',
  [ShopType.Popup]: '快闪店',
  [ShopType.Virtual]: '虚拟店铺',
};

const STATUS_CONFIG: Record<EffectiveShopStatus, { label: string; color: string }> = {
  [EffectiveShopStatus.Active]: { label: '运营中', color: 'bg-green-100 text-green-800' },
  [EffectiveShopStatus.PausedBySelf]: { label: '自行暂停', color: 'bg-yellow-100 text-yellow-800' },
  [EffectiveShopStatus.PausedByEntity]: { label: 'Entity 暂停', color: 'bg-orange-100 text-orange-800' },
  [EffectiveShopStatus.FundDepleted]: { label: '资金耗尽', color: 'bg-red-100 text-red-800' },
  [EffectiveShopStatus.Closed]: { label: '已关闭', color: 'bg-gray-100 text-gray-800' },
  [EffectiveShopStatus.ClosedByEntity]: { label: 'Entity 关闭', color: 'bg-gray-100 text-gray-800' },
  [EffectiveShopStatus.Closing]: { label: '关闭中', color: 'bg-gray-100 text-gray-600' },
  [EffectiveShopStatus.Banned]: { label: '已封禁', color: 'bg-red-100 text-red-900' },
};

// ─── Helpers ────────────────────────────────────────────────

function formatNexBalance(balance: bigint): string {
  const divisor = BigInt('1000000000000');
  const whole = balance / divisor;
  const remainder = balance % divisor;
  const decStr = remainder.toString().padStart(12, '0').slice(0, 4);
  const trimmed = decStr.replace(/0+$/, '');
  return trimmed ? `${whole.toLocaleString()}.${trimmed}` : whole.toLocaleString();
}

// ─── Shop Card ──────────────────────────────────────────────

function ShopCard({ shop, onTopUp }: { shop: ShopData; onTopUp: (shopId: number) => void }) {
  const statusCfg = STATUS_CONFIG[shop.effectiveStatus];
  const showFundWarning =
    shop.effectiveStatus === EffectiveShopStatus.FundDepleted ||
    isFundWarning(shop.fundBalance, FUND_WARNING_THRESHOLD);

  return (
    <div className="rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-900">
      <div className="flex items-start justify-between">
        <div>
          <h3 className="text-base font-semibold text-gray-900 dark:text-gray-100">
            {shop.name}
          </h3>
          <p className="mt-1 text-xs text-gray-500">
            ID: {shop.id} · {SHOP_TYPE_LABELS[shop.shopType as ShopType] ?? shop.shopType}
          </p>
        </div>
        <span className={`inline-flex rounded-full px-2.5 py-0.5 text-xs font-medium ${statusCfg.color}`}>
          {statusCfg.label}
        </span>
      </div>

      <div className="mt-3 text-sm text-gray-700 dark:text-gray-300">
        运营资金: <span className="font-medium">{formatNexBalance(shop.fundBalance)} NEX</span>
      </div>

      {showFundWarning && (
        <div className="mt-3 flex items-center justify-between rounded-md bg-red-50 px-3 py-2 dark:bg-red-900/20">
          <span className="text-xs text-red-700 dark:text-red-400">
            ⚠️ 资金不足，请及时充值以维持店铺运营
          </span>
          <button
            type="button"
            onClick={() => onTopUp(shop.id)}
            className="ml-2 rounded bg-red-600 px-2.5 py-1 text-xs text-white hover:bg-red-700"
          >
            快捷充值
          </button>
        </div>
      )}

      {shop.pointsConfig && (
        <div className="mt-2 text-xs text-gray-500">
          积分: 奖励 {shop.pointsConfig.rewardRateBps / 100}% · 兑换 {shop.pointsConfig.exchangeRateBps / 100}%
          {shop.pointsConfig.transferable ? ' · 可转让' : ''}
        </div>
      )}
    </div>
  );
}

// ─── Create Shop Form ───────────────────────────────────────

function CreateShopForm() {
  const { entityId } = useEntityContext();
  const { createShop } = useShops();

  const [name, setName] = useState('');
  const [shopType, setShopType] = useState<ShopType>(ShopType.OnlineStore);
  const [initialFund, setInitialFund] = useState('');

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      const trimmedName = name.trim();
      if (!trimmedName) return;

      // Convert NEX amount to chain balance (12 decimals)
      const parts = initialFund.trim().split('.');
      const whole = parts[0] ?? '0';
      const frac = (parts[1] ?? '').padEnd(12, '0').slice(0, 12);
      const rawFund = BigInt(whole) * BigInt('1000000000000') + BigInt(frac);

      createShop.mutate([entityId, trimmedName, shopType, rawFund.toString()]);
      setName('');
      setInitialFund('');
    },
    [entityId, name, shopType, initialFund, createShop],
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">创建店铺</h2>
      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label htmlFor="shop-name" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
            店铺名称
          </label>
          <input
            id="shop-name"
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="输入店铺名称"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
        </div>

        <div>
          <label htmlFor="shop-type" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
            店铺类型
          </label>
          <select
            id="shop-type"
            value={shopType}
            onChange={(e) => setShopType(e.target.value as ShopType)}
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
          >
            {Object.values(ShopType).map((t) => (
              <option key={t} value={t}>
                {SHOP_TYPE_LABELS[t]}
              </option>
            ))}
          </select>
        </div>

        <div>
          <label htmlFor="initial-fund" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
            初始运营资金 (NEX)
          </label>
          <input
            id="initial-fund"
            type="text"
            inputMode="decimal"
            value={initialFund}
            onChange={(e) => setInitialFund(e.target.value)}
            placeholder="0.0"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
        </div>

        <div className="flex items-center gap-3">
          <button
            type="submit"
            disabled={createShop.txState.status === 'signing' || createShop.txState.status === 'broadcasting'}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            创建店铺
          </button>
          <TxStatusIndicator txState={createShop.txState} />
        </div>
      </form>
    </section>
  );
}

// ─── Top-Up Dialog ──────────────────────────────────────────

function TopUpDialog({
  shopId,
  onClose,
}: {
  shopId: number;
  onClose: () => void;
}) {
  const [amount, setAmount] = useState('');
  const { depositFund } = useShops();

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      const parts = amount.trim().split('.');
      const whole = parts[0] ?? '0';
      const frac = (parts[1] ?? '').padEnd(12, '0').slice(0, 12);
      const rawAmount = BigInt(whole) * BigInt('1000000000000') + BigInt(frac);
      depositFund.mutate([shopId, rawAmount.toString()]);
      setAmount('');
      onClose();
    },
    [shopId, amount, depositFund, onClose],
  );

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="w-full max-w-sm rounded-lg bg-white p-6 shadow-lg dark:bg-gray-900">
        <h3 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">
          充值运营资金 (Shop #{shopId})
        </h3>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label htmlFor="topup-shop-amount" className="mb-1 block text-sm text-gray-700 dark:text-gray-300">
              充值金额 (NEX)
            </label>
            <input
              id="topup-shop-amount"
              type="text"
              inputMode="decimal"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              placeholder="0.0"
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              required
              autoFocus
            />
          </div>
          <div className="flex justify-end gap-2">
            <button
              type="button"
              onClick={onClose}
              className="rounded-md border border-gray-300 px-4 py-2 text-sm text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:text-gray-300 dark:hover:bg-gray-800"
            >
              取消
            </button>
            <button
              type="submit"
              disabled={!amount.trim()}
              className="rounded-md bg-green-600 px-4 py-2 text-sm text-white hover:bg-green-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              充值
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function ShopsPage() {
  const t = useTranslations('shops');
  const { isReadOnly, isSuspended } = useEntityContext();
  const { shops, isLoading, error } = useShops();
  const [topUpShopId, setTopUpShopId] = useState<number | null>(null);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center p-12 text-sm text-gray-500">
        {t('loading')}
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center p-12 text-sm text-red-500">
        加载失败: {String(error)}
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-4xl space-y-6 p-4 sm:p-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('title')}</h1>

      {/* Create shop form — requires SHOP_MANAGE permission, not in readonly/suspended */}
      {!isReadOnly && !isSuspended && (
        <PermissionGuard required={AdminPermission.SHOP_MANAGE} fallback={null}>
          <CreateShopForm />
        </PermissionGuard>
      )}

      {/* Shop list */}
      {shops.length === 0 ? (
        <div className="rounded-lg border border-dashed border-gray-300 p-8 text-center text-sm text-gray-500 dark:border-gray-600">
          暂无店铺
        </div>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2">
          {shops.map((shop) => (
            <ShopCard key={shop.id} shop={shop} onTopUp={setTopUpShopId} />
          ))}
        </div>
      )}

      {/* Top-up dialog */}
      {topUpShopId !== null && (
        <TopUpDialog shopId={topUpShopId} onClose={() => setTopUpShopId(null)} />
      )}
    </div>
  );
}
