'use client';

import React, { useCallback } from 'react';
import { useParams } from 'next/navigation';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useShopOrders } from '@/hooks/use-orders';
import { useCurrentBlock } from '@/hooks/use-current-block';
import { PermissionGuard } from '@/components/permission-guard';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { EscrowStatusSection } from '@/components/order/escrow-status-section';
import { DisputeNotice } from '@/components/order/dispute-notice';
import { OrderTimeoutWarning } from '@/components/order/order-timeout-warning';
import { AdminPermission } from '@/lib/types/models';
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

/** 卖家可执行的操作：目标状态 → { 按钮标签, 颜色 } */
const SELLER_TRANSITION_CONFIG: Partial<Record<OrderStatus, { label: string; color: string }>> = {
  [OrderStatus.Shipped]: { label: '确认发货', color: 'bg-indigo-600 hover:bg-indigo-700' },
  [OrderStatus.Completed]: { label: '完成订单', color: 'bg-green-600 hover:bg-green-700' },
  [OrderStatus.ServiceStarted]: { label: '开始服务', color: 'bg-cyan-600 hover:bg-cyan-700' },
  [OrderStatus.ServiceCompleted]: { label: '完成服务', color: 'bg-teal-600 hover:bg-teal-700' },
};

function formatNexBalance(balance: bigint): string {
  const divisor = BigInt('1000000000000');
  const whole = balance / divisor;
  const remainder = balance % divisor;
  const decStr = remainder.toString().padStart(12, '0').slice(0, 4);
  const trimmed = decStr.replace(/0+$/, '');
  return trimmed ? `${whole.toLocaleString()}.${trimmed}` : whole.toLocaleString();
}

// ─── Seller Order Card ──────────────────────────────────────

function SellerOrderCard({ order, shopId, currentBlock }: { order: OrderData; shopId: number; currentBlock: number }) {
  const { isReadOnly, isSuspended } = useEntityContext();
  const {
    confirmShipment, approveRefund, cancelOrder,
    completeOrder, startService, completeService,
  } = useShopOrders(shopId);
  const statusCfg = ORDER_STATUS_CONFIG[order.status];
  const canAct = !isReadOnly && !isSuspended;

  const category = order.productCategory ?? ProductCategory.Physical;
  const validTransitions = getValidOrderTransitions(category, order.status);

  const handleTransition = useCallback((target: OrderStatus) => {
    switch (target) {
      case OrderStatus.Shipped:
        confirmShipment.mutate([order.id]);
        break;
      case OrderStatus.Completed:
        completeOrder.mutate([order.id]);
        break;
      case OrderStatus.ServiceStarted:
        startService.mutate([order.id]);
        break;
      case OrderStatus.ServiceCompleted:
        completeService.mutate([order.id]);
        break;
    }
  }, [order.id, confirmShipment, completeOrder, startService, completeService]);

  const handleApproveRefund = useCallback(() => {
    approveRefund.mutate([order.id]);
  }, [order.id, approveRefund]);

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
            商品 #{order.productId} · 数量 {order.quantity}
          </p>
        </div>
        <span className={`inline-flex rounded-full px-2 py-0.5 text-xs font-medium ${statusCfg.color}`}>
          {statusCfg.label}
        </span>
      </div>

      <div className="mt-3 space-y-1 text-sm text-gray-600 dark:text-gray-400">
        <p>买家: {order.buyer.slice(0, 8)}…{order.buyer.slice(-6)}</p>
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

      {/* Seller actions — driven by order flow transitions */}
      {canAct && (
        <PermissionGuard required={AdminPermission.ORDER_MANAGE} fallback={null}>
          <div className="mt-3 flex flex-wrap gap-2">
            {validTransitions.map((target) => {
              const cfg = SELLER_TRANSITION_CONFIG[target];
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
            {order.status === OrderStatus.RefundRequested && (
              <button
                onClick={handleApproveRefund}
                className="rounded-md bg-orange-600 px-3 py-1 text-xs text-white hover:bg-orange-700"
              >
                批准退款
              </button>
            )}
            {(order.status === OrderStatus.Created || order.status === OrderStatus.Paid) && (
              <button
                onClick={handleCancel}
                className="rounded-md bg-gray-600 px-3 py-1 text-xs text-white hover:bg-gray-700"
              >
                取消订单
              </button>
            )}
          </div>
          <div className="mt-2">
            <TxStatusIndicator txState={confirmShipment.txState} />
            <TxStatusIndicator txState={completeOrder.txState} />
            <TxStatusIndicator txState={startService.txState} />
            <TxStatusIndicator txState={completeService.txState} />
            <TxStatusIndicator txState={approveRefund.txState} />
            <TxStatusIndicator txState={cancelOrder.txState} />
          </div>
        </PermissionGuard>
      )}
    </div>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function ShopOrdersPage() {
  const t = useTranslations('shops');
  const params = useParams();
  const shopId = Number(params.shopId);
  const { orders, isLoading, error } = useShopOrders(shopId);
  const currentBlock = useCurrentBlock();

  if (isLoading) {
    return (
      <div className="flex items-center justify-center p-12 text-sm text-gray-500">
        {t('orders.loading')}
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
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('orders.title')}</h1>

      {orders.length === 0 ? (
        <div className="rounded-lg border border-dashed border-gray-300 p-8 text-center text-sm text-gray-500 dark:border-gray-600">
          暂无订单
        </div>
      ) : (
        <div className="space-y-4">
          {orders.map((order) => (
            <SellerOrderCard key={order.id} order={order} shopId={shopId} currentBlock={currentBlock} />
          ))}
        </div>
      )}
    </div>
  );
}
