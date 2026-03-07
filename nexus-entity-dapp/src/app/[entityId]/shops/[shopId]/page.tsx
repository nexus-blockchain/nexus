'use client';

import React, { useState, useCallback } from 'react';
import { useParams } from 'next/navigation';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useShops } from '@/hooks/use-shops';
import { useEntityMutation } from '@/hooks/use-entity-mutation';
import { PermissionGuard } from '@/components/permission-guard';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { AdminPermission } from '@/lib/types/models';
import { ShopType, EffectiveShopStatus } from '@/lib/types/enums';

import { useTranslations } from 'next-intl';
// ─── Constants ──────────────────────────────────────────────

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

function formatNexBalance(balance: bigint): string {
  const divisor = BigInt('1000000000000');
  const whole = balance / divisor;
  const remainder = balance % divisor;
  const decStr = remainder.toString().padStart(12, '0').slice(0, 4);
  const trimmed = decStr.replace(/0+$/, '');
  return trimmed ? `${whole.toLocaleString()}.${trimmed}` : whole.toLocaleString();
}

// ─── Points Config Form ─────────────────────────────────────

function EnablePointsForm({ shopId }: { shopId: number }) {
  const { entityId } = useEntityContext();
  const [rewardRateBps, setRewardRateBps] = useState('100');
  const [exchangeRateBps, setExchangeRateBps] = useState('100');
  const [transferable, setTransferable] = useState(false);

  const enablePoints = useEntityMutation('entityShop', 'enablePoints', {
    invalidateKeys: [['entity', entityId, 'shops']],
  });

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      enablePoints.mutate([shopId, Number(rewardRateBps), Number(exchangeRateBps), transferable]);
    },
    [shopId, rewardRateBps, exchangeRateBps, transferable, enablePoints],
  );

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">启用积分系统</h3>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <div>
          <label htmlFor="reward-rate" className="mb-1 block text-xs text-gray-500">
            奖励比例 (bps, 100 = 1%)
          </label>
          <input
            id="reward-rate"
            type="number"
            min="0"
            value={rewardRateBps}
            onChange={(e) => setRewardRateBps(e.target.value)}
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
        </div>
        <div>
          <label htmlFor="exchange-rate" className="mb-1 block text-xs text-gray-500">
            兑换比例 (bps, 100 = 1%)
          </label>
          <input
            id="exchange-rate"
            type="number"
            min="0"
            value={exchangeRateBps}
            onChange={(e) => setExchangeRateBps(e.target.value)}
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
        </div>
      </div>

      <label className="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
        <input
          type="checkbox"
          checked={transferable}
          onChange={(e) => setTransferable(e.target.checked)}
          className="rounded"
        />
        允许积分转让
      </label>

      <div className="flex items-center gap-3">
        <button
          type="submit"
          disabled={enablePoints.txState.status === 'signing' || enablePoints.txState.status === 'broadcasting'}
          className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
        >
          启用积分
        </button>
        <TxStatusIndicator txState={enablePoints.txState} />
      </div>
    </form>
  );
}

// ─── Update Points Config Form ──────────────────────────────

function UpdatePointsForm({ shopId, currentConfig }: {
  shopId: number;
  currentConfig: { rewardRateBps: number; exchangeRateBps: number; transferable: boolean };
}) {
  const { entityId } = useEntityContext();
  const [rewardRateBps, setRewardRateBps] = useState(String(currentConfig.rewardRateBps));
  const [exchangeRateBps, setExchangeRateBps] = useState(String(currentConfig.exchangeRateBps));
  const [transferable, setTransferable] = useState(currentConfig.transferable);

  const updatePointsConfig = useEntityMutation('entityShop', 'updatePointsConfig', {
    invalidateKeys: [['entity', entityId, 'shops']],
  });

  const disablePoints = useEntityMutation('entityShop', 'disablePoints', {
    invalidateKeys: [['entity', entityId, 'shops']],
  });

  const handleUpdate = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      updatePointsConfig.mutate([shopId, Number(rewardRateBps), Number(exchangeRateBps), transferable]);
    },
    [shopId, rewardRateBps, exchangeRateBps, transferable, updatePointsConfig],
  );

  const handleDisable = useCallback(() => {
    disablePoints.mutate([shopId]);
  }, [shopId, disablePoints]);

  return (
    <div className="space-y-4">
      <div className="rounded-md bg-green-50 px-4 py-3 dark:bg-green-900/20">
        <p className="text-sm font-medium text-green-800 dark:text-green-300">✓ 积分系统已启用</p>
        <p className="mt-1 text-xs text-green-600 dark:text-green-400">
          奖励 {currentConfig.rewardRateBps / 100}% · 兑换 {currentConfig.exchangeRateBps / 100}%
          {currentConfig.transferable ? ' · 可转让' : ' · 不可转让'}
        </p>
      </div>

      <form onSubmit={handleUpdate} className="space-y-4">
        <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">更新积分配置</h3>

        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <div>
            <label htmlFor="update-reward-rate" className="mb-1 block text-xs text-gray-500">
              奖励比例 (bps)
            </label>
            <input
              id="update-reward-rate"
              type="number"
              min="0"
              value={rewardRateBps}
              onChange={(e) => setRewardRateBps(e.target.value)}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              required
            />
          </div>
          <div>
            <label htmlFor="update-exchange-rate" className="mb-1 block text-xs text-gray-500">
              兑换比例 (bps)
            </label>
            <input
              id="update-exchange-rate"
              type="number"
              min="0"
              value={exchangeRateBps}
              onChange={(e) => setExchangeRateBps(e.target.value)}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              required
            />
          </div>
        </div>

        <label className="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
          <input
            type="checkbox"
            checked={transferable}
            onChange={(e) => setTransferable(e.target.checked)}
            className="rounded"
          />
          允许积分转让
        </label>

        <div className="flex items-center gap-3">
          <button
            type="submit"
            disabled={updatePointsConfig.txState.status === 'signing' || updatePointsConfig.txState.status === 'broadcasting'}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            更新配置
          </button>
          <TxStatusIndicator txState={updatePointsConfig.txState} />
        </div>
      </form>

      <hr className="border-gray-200 dark:border-gray-700" />

      <div>
        <h3 className="text-sm font-medium text-red-700 dark:text-red-400">禁用积分</h3>
        <p className="mb-2 text-xs text-gray-500">禁用后积分系统将停止运作。</p>
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={handleDisable}
            disabled={disablePoints.txState.status === 'signing' || disablePoints.txState.status === 'broadcasting'}
            className="rounded-md bg-red-600 px-4 py-2 text-sm text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            禁用积分
          </button>
          <TxStatusIndicator txState={disablePoints.txState} />
        </div>
      </div>
    </div>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function ShopDetailPage() {
  const t = useTranslations('shops');
  const params = useParams();
  const shopId = Number(params.shopId);
  const { isReadOnly, isSuspended } = useEntityContext();
  const { getShop, isLoading, error } = useShops();

  const shop = getShop(shopId);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center p-12 text-sm text-gray-500">
        {t('detail.loading')}
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

  if (!shop) {
    return (
      <div className="flex items-center justify-center p-12 text-sm text-gray-500">
        店铺不存在
      </div>
    );
  }

  const statusCfg = STATUS_CONFIG[shop.effectiveStatus];

  return (
    <div className="mx-auto max-w-3xl space-y-6 p-4 sm:p-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('detail.title')}</h1>

      {/* Shop Info */}
      <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
        <div className="flex items-start justify-between">
          <div>
            <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">{shop.name}</h2>
            <p className="mt-1 text-sm text-gray-500">
              ID: {shop.id} · {SHOP_TYPE_LABELS[shop.shopType as ShopType] ?? shop.shopType}
            </p>
          </div>
          <span className={`inline-flex rounded-full px-2.5 py-0.5 text-xs font-medium ${statusCfg.color}`}>
            {statusCfg.label}
          </span>
        </div>

        <div className="mt-4 rounded-md bg-gray-50 px-4 py-3 dark:bg-gray-800">
          <p className="text-sm text-gray-500 dark:text-gray-400">运营资金</p>
          <p className="mt-1 text-xl font-bold text-gray-900 dark:text-gray-100">
            {formatNexBalance(shop.fundBalance)} <span className="text-sm font-normal text-gray-500">NEX</span>
          </p>
        </div>
      </section>

      {/* Points System Management — requires SHOP_MANAGE */}
      {!isReadOnly && !isSuspended && (
        <PermissionGuard required={AdminPermission.SHOP_MANAGE} fallback={null}>
          <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
            <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">积分系统管理</h2>
            {shop.pointsConfig ? (
              <UpdatePointsForm shopId={shop.id} currentConfig={shop.pointsConfig} />
            ) : (
              <EnablePointsForm shopId={shop.id} />
            )}
          </section>
        </PermissionGuard>
      )}
    </div>
  );
}
