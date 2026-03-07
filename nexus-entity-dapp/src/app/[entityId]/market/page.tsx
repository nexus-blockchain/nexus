'use client';

import React, { useState, useCallback, useMemo } from 'react';
import dynamic from 'next/dynamic';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { useEntityMarket } from '@/hooks/use-entity-market';
import type { MarketOrder } from '@/lib/types/models';

import { useTranslations } from 'next-intl';
// Lazy-load chart component
const MarketChart = dynamic(() => import('@/components/market-chart'), {
  ssr: false,
  loading: () => <div className="flex h-48 items-center justify-center text-sm text-gray-400">加载图表中…</div>,
});

// ─── Helpers ────────────────────────────────────────────────

function formatAmount(amount: bigint): string {
  return amount.toLocaleString();
}

function isTxBusy(m: { txState: { status: string } }): boolean {
  return m.txState.status === 'signing' || m.txState.status === 'broadcasting';
}

// ─── Market Stats Display ───────────────────────────────────

function MarketStatsSection() {
  const { stats, priceProtection } = useEntityMarket();

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">市场统计</h2>

      {stats?.circuitBreakerActive && (
        <div className="mb-4 rounded-md bg-red-50 p-3 text-sm text-red-800 dark:bg-red-900/30 dark:text-red-300">
          ⚠️ 熔断机制已触发，当前暂停交易
        </div>
      )}

      <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
        <div>
          <p className="text-xs text-gray-500">TWAP 价格</p>
          <p className="text-sm font-medium text-gray-900 dark:text-gray-100">{stats ? formatAmount(stats.twapPrice) : '—'}</p>
        </div>
        <div>
          <p className="text-xs text-gray-500">最新价格</p>
          <p className="text-sm font-medium text-gray-900 dark:text-gray-100">{stats ? formatAmount(stats.lastPrice) : '—'}</p>
        </div>
        <div>
          <p className="text-xs text-gray-500">24h 成交量</p>
          <p className="text-sm font-medium text-gray-900 dark:text-gray-100">{stats ? formatAmount(stats.volume24h) : '—'}</p>
        </div>
        <div>
          <p className="text-xs text-gray-500">熔断状态</p>
          <span
            className={`inline-block rounded-full px-2 py-0.5 text-xs font-medium ${
              stats?.circuitBreakerActive
                ? 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300'
                : 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300'
            }`}
          >
            {stats?.circuitBreakerActive ? '已触发' : '正常'}
          </span>
        </div>
      </div>

      {priceProtection && (
        <div className="mt-4 border-t border-gray-100 pt-4 dark:border-gray-700">
          <h3 className="mb-2 text-sm font-medium text-gray-700 dark:text-gray-300">价格保护配置</h3>
          <div className="grid grid-cols-3 gap-4 text-sm">
            <div>
              <p className="text-xs text-gray-500">最大偏差 (bps)</p>
              <p className="text-gray-900 dark:text-gray-100">{priceProtection.maxDeviationBps}</p>
            </div>
            <div>
              <p className="text-xs text-gray-500">熔断阈值</p>
              <p className="text-gray-900 dark:text-gray-100">{priceProtection.circuitBreakerThreshold}</p>
            </div>
            <div>
              <p className="text-xs text-gray-500">熔断持续时间</p>
              <p className="text-gray-900 dark:text-gray-100">{priceProtection.circuitBreakerDuration} 区块</p>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}

// ─── Order Book Display ─────────────────────────────────────

function OrderBookSection() {
  const { orders, cancelOrder, takeOrder } = useEntityMarket();
  const { isReadOnly, isSuspended } = useEntityContext();
  const { stats } = useEntityMarket();
  const tradingDisabled = isReadOnly || isSuspended || !!stats?.circuitBreakerActive;

  const [takeAmount, setTakeAmount] = useState<Record<number, string>>({});

  const { buyOrders, sellOrders } = useMemo(() => {
    const buys = orders
      .filter((o) => o.side === 'Buy')
      .sort((a, b) => (b.price > a.price ? 1 : b.price < a.price ? -1 : 0));
    const sells = orders
      .filter((o) => o.side === 'Sell')
      .sort((a, b) => (a.price > b.price ? 1 : a.price < b.price ? -1 : 0));

    // Compute cumulative depth
    let cumBuy = BigInt(0);
    const buyWithDepth = buys.map((o) => { cumBuy += o.amount - o.filled; return { ...o, cumulative: cumBuy }; });
    let cumSell = BigInt(0);
    const sellWithDepth = sells.map((o) => { cumSell += o.amount - o.filled; return { ...o, cumulative: cumSell }; });

    return { buyOrders: buyWithDepth, sellOrders: sellWithDepth };
  }, [orders]);

  const handleCancel = useCallback((orderId: number) => {
    const { entityId } = orders.find((o) => o.id === orderId) ?? { entityId: 0 };
    cancelOrder.mutate([entityId, orderId]);
  }, [orders, cancelOrder]);

  const handleTake = useCallback((order: MarketOrder) => {
    const amt = takeAmount[order.id];
    if (!amt?.trim()) return;
    takeOrder.mutate([order.entityId, order.id, amt.trim()]);
    setTakeAmount((prev) => ({ ...prev, [order.id]: '' }));
  }, [takeAmount, takeOrder]);

  const renderOrderRow = (o: MarketOrder & { cumulative: bigint }) => (
    <tr key={o.id} className="border-b last:border-0 dark:border-gray-700">
      <td className="py-2 pr-4 text-gray-900 dark:text-gray-100">{formatAmount(o.price)}</td>
      <td className="py-2 pr-4 text-gray-900 dark:text-gray-100">{formatAmount(o.amount - o.filled)}</td>
      <td className="py-2 pr-4 text-gray-500">{formatAmount(o.cumulative)}</td>
      <td className="py-2 text-right">
        {!tradingDisabled && (
          <div className="flex items-center justify-end gap-2">
            <input
              type="text"
              inputMode="decimal"
              value={takeAmount[o.id] ?? ''}
              onChange={(e) => setTakeAmount((prev) => ({ ...prev, [o.id]: e.target.value }))}
              placeholder="数量"
              className="w-20 rounded-md border border-gray-300 px-2 py-1 text-xs dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            />
            <button
              type="button"
              onClick={() => handleTake(o)}
              disabled={isTxBusy(takeOrder)}
              className="rounded-md bg-blue-600 px-2 py-1 text-xs text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              吃单
            </button>
            <button
              type="button"
              onClick={() => handleCancel(o.id)}
              disabled={isTxBusy(cancelOrder)}
              className="rounded-md bg-gray-500 px-2 py-1 text-xs text-white hover:bg-gray-600 disabled:cursor-not-allowed disabled:opacity-50"
            >
              撤单
            </button>
          </div>
        )}
      </td>
    </tr>
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">订单簿</h2>
      <div className="grid gap-6 md:grid-cols-2">
        {/* Sell side */}
        <div>
          <h3 className="mb-2 text-sm font-medium text-red-600">卖单</h3>
          {sellOrders.length === 0 ? (
            <p className="text-sm text-gray-400">暂无卖单</p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-left text-sm">
                <thead>
                  <tr className="border-b text-xs text-gray-500 dark:border-gray-700">
                    <th className="pb-2 pr-4">价格</th>
                    <th className="pb-2 pr-4">数量</th>
                    <th className="pb-2 pr-4">累计深度</th>
                    <th className="pb-2 text-right">操作</th>
                  </tr>
                </thead>
                <tbody>{sellOrders.map(renderOrderRow)}</tbody>
              </table>
            </div>
          )}
        </div>
        {/* Buy side */}
        <div>
          <h3 className="mb-2 text-sm font-medium text-green-600">买单</h3>
          {buyOrders.length === 0 ? (
            <p className="text-sm text-gray-400">暂无买单</p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-left text-sm">
                <thead>
                  <tr className="border-b text-xs text-gray-500 dark:border-gray-700">
                    <th className="pb-2 pr-4">价格</th>
                    <th className="pb-2 pr-4">数量</th>
                    <th className="pb-2 pr-4">累计深度</th>
                    <th className="pb-2 text-right">操作</th>
                  </tr>
                </thead>
                <tbody>{buyOrders.map(renderOrderRow)}</tbody>
              </table>
            </div>
          )}
        </div>
      </div>
      <div className="mt-3 flex items-center gap-3">
        <TxStatusIndicator txState={cancelOrder.txState} />
        <TxStatusIndicator txState={takeOrder.txState} />
      </div>
    </section>
  );
}

// ─── Limit Trade Forms ──────────────────────────────────────

function LimitTradeSection() {
  const { entityId, isReadOnly, isSuspended } = useEntityContext();
  const { placeBuyOrder, placeSellOrder, stats } = useEntityMarket();
  const tradingDisabled = isReadOnly || isSuspended || !!stats?.circuitBreakerActive;

  const [buyPrice, setBuyPrice] = useState('');
  const [buyAmount, setBuyAmount] = useState('');
  const [sellPrice, setSellPrice] = useState('');
  const [sellAmount, setSellAmount] = useState('');

  const handleBuy = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!buyPrice.trim() || !buyAmount.trim()) return;
      placeBuyOrder.mutate([entityId, buyPrice.trim(), buyAmount.trim()]);
      setBuyPrice('');
      setBuyAmount('');
    },
    [entityId, buyPrice, buyAmount, placeBuyOrder],
  );

  const handleSell = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!sellPrice.trim() || !sellAmount.trim()) return;
      placeSellOrder.mutate([entityId, sellPrice.trim(), sellAmount.trim()]);
      setSellPrice('');
      setSellAmount('');
    },
    [entityId, sellPrice, sellAmount, placeSellOrder],
  );

  if (tradingDisabled) return null;

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">限价交易</h2>
      <div className="grid gap-6 md:grid-cols-2">
        {/* Limit Buy */}
        <form onSubmit={handleBuy} className="space-y-3">
          <h3 className="text-sm font-medium text-green-600">限价买入</h3>
          <input
            type="text"
            inputMode="decimal"
            value={buyPrice}
            onChange={(e) => setBuyPrice(e.target.value)}
            placeholder="价格"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
          <input
            type="text"
            inputMode="decimal"
            value={buyAmount}
            onChange={(e) => setBuyAmount(e.target.value)}
            placeholder="数量"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
          <div className="flex items-center gap-3">
            <button
              type="submit"
              disabled={isTxBusy(placeBuyOrder)}
              className="rounded-md bg-green-600 px-4 py-2 text-sm text-white hover:bg-green-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              买入
            </button>
            <TxStatusIndicator txState={placeBuyOrder.txState} />
          </div>
        </form>
        {/* Limit Sell */}
        <form onSubmit={handleSell} className="space-y-3">
          <h3 className="text-sm font-medium text-red-600">限价卖出</h3>
          <input
            type="text"
            inputMode="decimal"
            value={sellPrice}
            onChange={(e) => setSellPrice(e.target.value)}
            placeholder="价格"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
          <input
            type="text"
            inputMode="decimal"
            value={sellAmount}
            onChange={(e) => setSellAmount(e.target.value)}
            placeholder="数量"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
          <div className="flex items-center gap-3">
            <button
              type="submit"
              disabled={isTxBusy(placeSellOrder)}
              className="rounded-md bg-red-600 px-4 py-2 text-sm text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              卖出
            </button>
            <TxStatusIndicator txState={placeSellOrder.txState} />
          </div>
        </form>
      </div>
    </section>
  );
}

// ─── Market Trade Forms ─────────────────────────────────────

function MarketTradeSection() {
  const { entityId, isReadOnly, isSuspended } = useEntityContext();
  const { marketBuy, marketSell, stats } = useEntityMarket();
  const tradingDisabled = isReadOnly || isSuspended || !!stats?.circuitBreakerActive;

  const [buyAmount, setBuyAmount] = useState('');
  const [maxCost, setMaxCost] = useState('');
  const [sellAmount, setSellAmount] = useState('');
  const [minReceive, setMinReceive] = useState('');

  const handleMarketBuy = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!buyAmount.trim() || !maxCost.trim()) return;
      marketBuy.mutate([entityId, buyAmount.trim(), maxCost.trim()]);
      setBuyAmount('');
      setMaxCost('');
    },
    [entityId, buyAmount, maxCost, marketBuy],
  );

  const handleMarketSell = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!sellAmount.trim() || !minReceive.trim()) return;
      marketSell.mutate([entityId, sellAmount.trim(), minReceive.trim()]);
      setSellAmount('');
      setMinReceive('');
    },
    [entityId, sellAmount, minReceive, marketSell],
  );

  if (tradingDisabled) return null;

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">市价交易</h2>
      <div className="grid gap-6 md:grid-cols-2">
        {/* Market Buy */}
        <form onSubmit={handleMarketBuy} className="space-y-3">
          <h3 className="text-sm font-medium text-green-600">市价买入</h3>
          <input
            type="text"
            inputMode="decimal"
            value={buyAmount}
            onChange={(e) => setBuyAmount(e.target.value)}
            placeholder="买入数量"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
          <input
            type="text"
            inputMode="decimal"
            value={maxCost}
            onChange={(e) => setMaxCost(e.target.value)}
            placeholder="最大花费（滑点保护）"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
          <div className="flex items-center gap-3">
            <button
              type="submit"
              disabled={isTxBusy(marketBuy)}
              className="rounded-md bg-green-600 px-4 py-2 text-sm text-white hover:bg-green-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              市价买入
            </button>
            <TxStatusIndicator txState={marketBuy.txState} />
          </div>
        </form>
        {/* Market Sell */}
        <form onSubmit={handleMarketSell} className="space-y-3">
          <h3 className="text-sm font-medium text-red-600">市价卖出</h3>
          <input
            type="text"
            inputMode="decimal"
            value={sellAmount}
            onChange={(e) => setSellAmount(e.target.value)}
            placeholder="卖出数量"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
          <input
            type="text"
            inputMode="decimal"
            value={minReceive}
            onChange={(e) => setMinReceive(e.target.value)}
            placeholder="最少收到（滑点保护）"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
          <div className="flex items-center gap-3">
            <button
              type="submit"
              disabled={isTxBusy(marketSell)}
              className="rounded-md bg-red-600 px-4 py-2 text-sm text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              市价卖出
            </button>
            <TxStatusIndicator txState={marketSell.txState} />
          </div>
        </form>
      </div>
    </section>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function MarketPage() {
  const t = useTranslations('market');
  const { isLoading, error, stats } = useEntityMarket();

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
    <div className="mx-auto max-w-5xl space-y-6 p-4 sm:p-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('title')}</h1>

      {stats?.circuitBreakerActive && (
        <div className="rounded-md border border-red-300 bg-red-50 p-4 text-sm text-red-800 dark:border-red-700 dark:bg-red-900/30 dark:text-red-300">
          ⚠️ 熔断机制已触发，交易功能暂时关闭。请等待熔断期结束后再进行交易。
        </div>
      )}

      <MarketStatsSection />
      <MarketChart />
      <OrderBookSection />
      <LimitTradeSection />
      <MarketTradeSection />
    </div>
  );
}
