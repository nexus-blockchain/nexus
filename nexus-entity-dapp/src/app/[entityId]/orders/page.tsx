'use client';

import React, { useState, useCallback } from 'react';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useBuyerOrders } from '@/hooks/use-orders';
import { useCurrentBlock } from '@/hooks/use-current-block';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { EscrowStatusSection } from '@/components/order/escrow-status-section';
import { DisputeNotice } from '@/components/order/dispute-notice';
import { OrderTimeoutWarning } from '@/components/order/order-timeout-warning';
import { OrderStatus, PaymentAsset, ProductCategory } from '@/lib/types/enums';
import { getValidOrderTransitions } from '@/lib/utils';
import type { OrderData } from '@/lib/types/models';

import { useTranslations } from 'next-intl';
// ─── Constants ──────────────────────────────────────────────

const ORDER_STATUS_CONFIG: Record<OrderStatus, { label: string; color: string }> = {
  [OrderStatus.Created]: { label: '已创建', color: 'bg-gray-100 text-gray-800' },
  [OrderStatus.Paid]: { label: '已支付', color: 'bg-blue-100 text-blue-800' },
  [OrderStatus.Shipped]: { label: '已发货', color: 'bg-indigo-100 text-indigo-800' },
  [OrderStatus.ServiceStarted]: { label: '服务中', color: 'bg-cyan-100 text-cyan-800' },
  [OrderStatus.ServiceCompleted]: { label: '服务完成', color: 'bg-teal-100 text-teal-800' },
  [OrderStatus.Confirmed]: { label: '已确认', color: 'bg-emerald-100 text-emerald-800' },
  [OrderStatus.Completed]: { label: '已完成', color: 'bg-green-100 text-green-800' },
  [OrderStatus.RefundRequested]: { label: '退款申请中', color: 'bg-orange-100 text-orange-800' },
  [OrderStatus.Refunded]: { label: '已退款', color: 'bg-yellow-100 text-yellow-800' },
  [OrderStatus.Disputed]: { label: '争议中', color: 'bg-red-100 text-red-800' },
  [OrderStatus.Cancelled]: { label: '已取消', color: 'bg-gray-100 text-gray-600' },
  [OrderStatus.Expired]: { label: '已过期', color: 'bg-gray-100 text-gray-500' },
};

const PAYMENT_LABELS: Record<PaymentAsset, string> = {
  [PaymentAsset.Native]: 'NEX',
  [PaymentAsset.EntityToken]: 'Entity Token',
};

/** 买家可执行的操作：目标状态 → { 按钮标签, 颜色 } */
const BUYER_TRANSITION_CONFIG: Partial<Record<OrderStatus, { label: string; color: string }>> = {
  [OrderStatus.Completed]: { label: '确认收货', color: 'bg-green-600 hover:bg-green-700' },
  [OrderStatus.Confirmed]: { label: '确认服务完成', color: 'bg-emerald-600 hover:bg-emerald-700' },
};

function formatNexBalance(balance: bigint): string {
  const divisor = BigInt('1000000000000');
  const whole = balance / divisor;
  const remainder = balance % divisor;
  const decStr = remainder.toString().padStart(12, '0').slice(0, 4);
  const trimmed = decStr.replace(/0+$/, '');
  return trimmed ? `${whole.toLocaleString()}.${trimmed}` : whole.toLocaleString();
}

// ─── Place Order Form ───────────────────────────────────────

function PlaceOrderForm() {
  const { isReadOnly, isSuspended } = useEntityContext();
  const { placeOrder } = useBuyerOrders();

  const [form, setForm] = useState({
    shopId: '',
    productId: '',
    quantity: '1',
    paymentAsset: PaymentAsset.Native as PaymentAsset,
    useShoppingBalance: false,
    referrer: '',
  });
  const [errors, setErrors] = useState<Record<string, string>>({});

  const setField = useCallback((key: string, value: string | boolean) => {
    setForm((prev) => ({ ...prev, [key]: value }));
    setErrors((prev) => {
      const next = { ...prev };
      delete next[key];
      return next;
    });
  }, []);

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      const fieldErrors: Record<string, string> = {};

      const shopId = Number(form.shopId);
      if (!Number.isInteger(shopId) || shopId <= 0) fieldErrors.shopId = '请输入有效的店铺 ID';

      const productId = Number(form.productId);
      if (!Number.isInteger(productId) || productId <= 0) fieldErrors.productId = '请输入有效的商品 ID';

      const quantity = Number(form.quantity);
      if (!Number.isInteger(quantity) || quantity < 1) fieldErrors.quantity = '数量必须 >= 1';

      if (Object.keys(fieldErrors).length > 0) {
        setErrors(fieldErrors);
        return;
      }

      const referrer = form.referrer.trim() || null;

      placeOrder.mutate([
        shopId,
        productId,
        quantity,
        form.paymentAsset,
        form.useShoppingBalance,
        referrer,
      ]);
    },
    [form, placeOrder],
  );

  if (isReadOnly || isSuspended) return null;

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">下单</h2>
      <form onSubmit={handleSubmit} className="space-y-4">
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
          <div>
            <label htmlFor="order-shop-id" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              店铺 ID
            </label>
            <input
              id="order-shop-id"
              type="number"
              min="1"
              value={form.shopId}
              onChange={(e) => setField('shopId', e.target.value)}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              required
            />
            {errors.shopId && <p className="mt-1 text-xs text-red-500">{errors.shopId}</p>}
          </div>
          <div>
            <label htmlFor="order-product-id" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              商品 ID
            </label>
            <input
              id="order-product-id"
              type="number"
              min="1"
              value={form.productId}
              onChange={(e) => setField('productId', e.target.value)}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              required
            />
            {errors.productId && <p className="mt-1 text-xs text-red-500">{errors.productId}</p>}
          </div>
          <div>
            <label htmlFor="order-quantity" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              数量
            </label>
            <input
              id="order-quantity"
              type="number"
              min="1"
              value={form.quantity}
              onChange={(e) => setField('quantity', e.target.value)}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              required
            />
            {errors.quantity && <p className="mt-1 text-xs text-red-500">{errors.quantity}</p>}
          </div>
        </div>

        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <div>
            <label htmlFor="payment-asset" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              支付方式
            </label>
            <select
              id="payment-asset"
              value={form.paymentAsset}
              onChange={(e) => setField('paymentAsset', e.target.value)}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            >
              <option value={PaymentAsset.Native}>NEX (原生代币)</option>
              <option value={PaymentAsset.EntityToken}>Entity Token</option>
            </select>
          </div>
          <div>
            <label htmlFor="order-referrer" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              推荐人地址 (可选)
            </label>
            <input
              id="order-referrer"
              type="text"
              value={form.referrer}
              onChange={(e) => setField('referrer', e.target.value)}
              placeholder="留空则无推荐人"
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            />
          </div>
        </div>

        <label className="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
          <input
            type="checkbox"
            checked={form.useShoppingBalance}
            onChange={(e) => setField('useShoppingBalance', e.target.checked)}
            className="rounded"
          />
          使用购物余额
        </label>

        <div className="flex items-center gap-3">
          <button
            type="submit"
            disabled={placeOrder.txState.status === 'signing' || placeOrder.txState.status === 'broadcasting'}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            提交订单
          </button>
          <TxStatusIndicator txState={placeOrder.txState} />
        </div>
      </form>
    </section>
  );
}

// ─── Buyer Order Card ───────────────────────────────────────

function BuyerOrderCard({ order, currentBlock }: { order: OrderData; currentBlock: number }) {
  const { isReadOnly, isSuspended } = useEntityContext();
  const { confirmReceipt, requestRefund, cancelOrder, confirmServiceCompletion } = useBuyerOrders();
  const statusCfg = ORDER_STATUS_CONFIG[order.status];
  const canAct = !isReadOnly && !isSuspended;

  const category = order.productCategory ?? ProductCategory.Physical;
  const validTransitions = getValidOrderTransitions(category, order.status);

  const handleTransition = useCallback((target: OrderStatus) => {
    switch (target) {
      case OrderStatus.Completed:
        confirmReceipt.mutate([order.id]);
        break;
      case OrderStatus.Confirmed:
        confirmServiceCompletion.mutate([order.id]);
        break;
    }
  }, [order.id, confirmReceipt, confirmServiceCompletion]);

  const handleRequestRefund = useCallback(() => {
    requestRefund.mutate([order.id]);
  }, [order.id, requestRefund]);

  const handleCancel = useCallback(() => {
    cancelOrder.mutate([order.id]);
  }, [order.id, cancelOrder]);

  return (
    <div className="rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-900">
      <div className="flex items-start justify-between">
        <div>
          <p className="text-sm font-medium text-gray-900 dark:text-gray-100">
            订单 #{order.id}
          </p>
          <p className="mt-0.5 text-xs text-gray-500">
            店铺 #{order.shopId} · 商品 #{order.productId} · 数量 {order.quantity}
          </p>
        </div>
        <span className={`inline-flex rounded-full px-2 py-0.5 text-xs font-medium ${statusCfg.color}`}>
          {statusCfg.label}
        </span>
      </div>

      <div className="mt-3 space-y-1 text-sm text-gray-600 dark:text-gray-400">
        <p>金额: {formatNexBalance(order.totalAmount)} {PAYMENT_LABELS[order.paymentAsset]}</p>
        {order.escrowId != null && <p>托管 ID: {order.escrowId}</p>}
      </div>

      {/* Escrow status display */}
      {order.escrowId != null && <EscrowStatusSection escrowId={order.escrowId} />}

      {/* Dispute notice */}
      {order.status === OrderStatus.Disputed && <DisputeNotice />}

      {/* Timeout warning */}
      <OrderTimeoutWarning
        updatedAt={order.updatedAt}
        currentBlock={currentBlock}
        status={order.status}
      />

      {/* Buyer actions — driven by order flow transitions */}
      {canAct && (
        <div className="mt-3 flex flex-wrap gap-2">
          {validTransitions.map((target) => {
            const cfg = BUYER_TRANSITION_CONFIG[target];
            if (!cfg) return null;
            return (
              <button
                key={target}
                onClick={() => handleTransition(target)}
                className={`rounded-md px-3 py-1 text-xs text-white ${cfg.color}`}
              >
                {cfg.label}
              </button>
            );
          })}
          {(order.status === OrderStatus.Paid || order.status === OrderStatus.Shipped) && (
            <button
              onClick={handleRequestRefund}
              className="rounded-md bg-orange-600 px-3 py-1 text-xs text-white hover:bg-orange-700"
            >
              申请退款
            </button>
          )}
          {order.status === OrderStatus.Created && (
            <button
              onClick={handleCancel}
              className="rounded-md bg-gray-600 px-3 py-1 text-xs text-white hover:bg-gray-700"
            >
              取消订单
            </button>
          )}
        </div>
      )}

      {canAct && (
        <div className="mt-2">
          <TxStatusIndicator txState={confirmReceipt.txState} />
          <TxStatusIndicator txState={confirmServiceCompletion.txState} />
          <TxStatusIndicator txState={requestRefund.txState} />
          <TxStatusIndicator txState={cancelOrder.txState} />
        </div>
      )}
    </div>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function BuyerOrdersPage() {
  const t = useTranslations('orders');
  const { orders, isLoading, error } = useBuyerOrders();
  const currentBlock = useCurrentBlock();

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

      <PlaceOrderForm />

      {orders.length === 0 ? (
        <div className="rounded-lg border border-dashed border-gray-300 p-8 text-center text-sm text-gray-500 dark:border-gray-600">
          暂无订单
        </div>
      ) : (
        <div className="space-y-4">
          {orders.map((order) => (
            <BuyerOrderCard key={order.id} order={order} currentBlock={currentBlock} />
          ))}
        </div>
      )}
    </div>
  );
}
