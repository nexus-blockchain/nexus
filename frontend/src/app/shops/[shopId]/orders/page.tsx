"use client";

import { use, useState } from "react";
import { useOrders, useOrderActions } from "@/hooks/useOrder";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { formatBalance } from "@/lib/utils";
import {
  ArrowLeft,
  ShoppingCart,
  Clock,
  CreditCard,
  Truck,
  CheckCircle,
  XCircle,
  AlertTriangle,
  RotateCcw,
  LayoutGrid,
  List,
  Coins,
  Banknote,
} from "lucide-react";
import Link from "next/link";
import type { OrderData } from "@/lib/types";

const KANBAN_COLUMNS = [
  { key: "Created", label: "Created", icon: Clock, color: "border-t-blue-500", bg: "bg-blue-50 dark:bg-blue-950/30" },
  { key: "Paid", label: "Paid", icon: CreditCard, color: "border-t-cyan-500", bg: "bg-cyan-50 dark:bg-cyan-950/30" },
  { key: "Shipped", label: "Shipped", icon: Truck, color: "border-t-indigo-500", bg: "bg-indigo-50 dark:bg-indigo-950/30" },
  { key: "Completed", label: "Completed", icon: CheckCircle, color: "border-t-green-500", bg: "bg-green-50 dark:bg-green-950/30" },
  { key: "Cancelled", label: "Cancelled", icon: XCircle, color: "border-t-gray-400", bg: "bg-gray-50 dark:bg-gray-900/30" },
  { key: "Disputed", label: "Disputed", icon: AlertTriangle, color: "border-t-red-500", bg: "bg-red-50 dark:bg-red-950/30" },
  { key: "Refunded", label: "Refunded", icon: RotateCcw, color: "border-t-purple-500", bg: "bg-purple-50 dark:bg-purple-950/30" },
  { key: "Expired", label: "Expired", icon: Clock, color: "border-t-gray-300", bg: "bg-gray-50 dark:bg-gray-900/30" },
] as const;

function PaymentBadge({ asset }: { asset: string }) {
  if (asset === "EntityToken") {
    return <Badge variant="secondary" className="gap-1 text-[10px]"><Coins className="h-2.5 w-2.5" />Token</Badge>;
  }
  return <Badge variant="outline" className="gap-1 text-[10px]"><Banknote className="h-2.5 w-2.5" />NEX</Badge>;
}

function OrderCard({ order, shopId, actions }: { order: OrderData; shopId: number; actions: ReturnType<typeof useOrderActions> }) {
  return (
    <Card className="transition-shadow hover:shadow-md">
      <CardContent className="p-3 space-y-2">
        <div className="flex items-center justify-between">
          <Link href={`/shops/${shopId}/orders/${order.id}`} className="font-semibold text-sm hover:text-primary transition-colors">
            #{order.id}
          </Link>
          <PaymentBadge asset={order.paymentAsset} />
        </div>

        <div className="text-xs text-muted-foreground space-y-0.5">
          <div className="flex justify-between">
            <span>Product #{order.productId}</span>
            <span>×{order.quantity}</span>
          </div>
          <div className="flex justify-between font-medium text-foreground">
            <span>Total</span>
            <span>{formatBalance(order.totalPrice)} NEX</span>
          </div>
        </div>

        <div className="flex items-center gap-1 text-xs text-muted-foreground">
          <AddressDisplay address={order.buyer} chars={4} />
        </div>

        {order.tokenDiscount > 0 && (
          <div className="text-[10px] text-green-600 dark:text-green-400">Token discount: -{formatBalance(order.tokenDiscount)}</div>
        )}

        <div className="flex gap-1.5 pt-1">
          {order.status === "Created" && (
            <Button size="sm" variant="outline" className="h-7 text-xs flex-1" onClick={() => actions.cancelOrder(order.id)}>
              Cancel
            </Button>
          )}
          {order.status === "Paid" && (
            <Button size="sm" className="h-7 text-xs flex-1" onClick={() => actions.shipOrder(order.id, "")}>
              <Truck className="mr-1 h-3 w-3" />Ship
            </Button>
          )}
          {order.status === "Shipped" && (
            <Button size="sm" className="h-7 text-xs flex-1" onClick={() => actions.confirmReceipt(order.id)}>
              <CheckCircle className="mr-1 h-3 w-3" />Confirm
            </Button>
          )}
          <Button size="sm" variant="ghost" className="h-7 text-xs" asChild>
            <Link href={`/shops/${shopId}/orders/${order.id}`}>View</Link>
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

function KanbanView({ orders, shopId, actions }: { orders: OrderData[]; shopId: number; actions: ReturnType<typeof useOrderActions> }) {
  return (
    <div className="flex gap-3 overflow-x-auto pb-4">
      {KANBAN_COLUMNS.map((col) => {
        const Icon = col.icon;
        const colOrders = orders.filter((o) => o.status === col.key);
        return (
          <div key={col.key} className={`min-w-[260px] w-[260px] shrink-0 rounded-lg border-t-4 ${col.color} ${col.bg} p-2`}>
            <div className="flex items-center justify-between mb-2 px-1">
              <div className="flex items-center gap-1.5">
                <Icon className="h-3.5 w-3.5 text-muted-foreground" />
                <span className="text-xs font-semibold">{col.label}</span>
              </div>
              <Badge variant="secondary" className="h-5 px-1.5 text-[10px]">{colOrders.length}</Badge>
            </div>
            <div className="space-y-2 max-h-[calc(100vh-320px)] overflow-y-auto">
              {colOrders.length === 0 ? (
                <div className="flex items-center justify-center py-6 text-xs text-muted-foreground">No orders</div>
              ) : (
                colOrders.map((order) => (
                  <OrderCard key={order.id} order={order} shopId={shopId} actions={actions} />
                ))
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}

function ListView({ orders, shopId, actions }: { orders: OrderData[]; shopId: number; actions: ReturnType<typeof useOrderActions> }) {
  if (orders.length === 0) {
    return (
      <Card>
        <CardContent className="flex flex-col items-center justify-center py-12">
          <ShoppingCart className="h-12 w-12 text-muted-foreground/50" />
          <p className="mt-4 text-lg font-medium">No Orders</p>
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-2">
      {orders.map((order) => (
        <Card key={order.id} className="hover:shadow-sm transition-shadow">
          <CardContent className="flex items-center gap-4 p-4">
            <div className="flex-1 space-y-1">
              <div className="flex items-center gap-3">
                <span className="font-semibold">Order #{order.id}</span>
                <StatusBadge status={order.status} />
                <PaymentBadge asset={order.paymentAsset} />
                <span className="text-xs text-muted-foreground">Product #{order.productId}</span>
              </div>
              <div className="flex items-center gap-4 text-sm text-muted-foreground">
                <span>Qty: {order.quantity}</span>
                <span>{formatBalance(order.totalPrice)} NEX</span>
                <AddressDisplay address={order.buyer} chars={4} />
              </div>
            </div>
            <div className="flex gap-2">
              {order.status === "Paid" && (
                <Button size="sm" onClick={() => actions.shipOrder(order.id, "")}>
                  <Truck className="mr-1 h-3 w-3" />Ship
                </Button>
              )}
              {order.status === "Shipped" && (
                <Button size="sm" onClick={() => actions.confirmReceipt(order.id)}>
                  <CheckCircle className="mr-1 h-3 w-3" />Confirm
                </Button>
              )}
              <Button variant="outline" size="sm" asChild>
                <Link href={`/shops/${shopId}/orders/${order.id}`}>Details</Link>
              </Button>
            </div>
          </CardContent>
        </Card>
      ))}
    </div>
  );
}

export default function OrdersPage({ params }: { params: Promise<{ shopId: string }> }) {
  const { shopId: shopIdStr } = use(params);
  const shopId = Number(shopIdStr);
  const { orders, isLoading, refetch } = useOrders(shopId);
  const actions = useOrderActions();
  const [viewMode, setViewMode] = useState<"kanban" | "list">("kanban");

  const statusCounts = KANBAN_COLUMNS.map((col) => ({
    ...col,
    count: orders.filter((o) => o.status === col.key).length,
  }));

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href={`/shops/${shopId}`}><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">Orders</h1>
          <p className="text-muted-foreground">Manage orders for Shop #{shopId}</p>
        </div>
        <div className="flex items-center gap-2">
          <div className="flex rounded-lg border">
            <Button
              variant={viewMode === "kanban" ? "secondary" : "ghost"}
              size="sm"
              className="rounded-r-none"
              onClick={() => setViewMode("kanban")}
            >
              <LayoutGrid className="mr-1.5 h-3.5 w-3.5" />Board
            </Button>
            <Button
              variant={viewMode === "list" ? "secondary" : "ghost"}
              size="sm"
              className="rounded-l-none"
              onClick={() => setViewMode("list")}
            >
              <List className="mr-1.5 h-3.5 w-3.5" />List
            </Button>
          </div>
          <Button variant="outline" size="sm" onClick={() => refetch()}>
            <RotateCcw className="mr-2 h-3 w-3" />Refresh
          </Button>
        </div>
      </div>

      <div className="grid gap-3 grid-cols-4 xl:grid-cols-8">
        {statusCounts.map((col) => {
          const Icon = col.icon;
          return (
            <Card key={col.key} className="p-3">
              <div className="flex items-center gap-2">
                <Icon className="h-4 w-4 text-muted-foreground shrink-0" />
                <div className="min-w-0">
                  <p className="text-xs text-muted-foreground truncate">{col.label}</p>
                  <p className="text-lg font-bold">{col.count}</p>
                </div>
              </div>
            </Card>
          );
        })}
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
      ) : viewMode === "kanban" ? (
        <KanbanView orders={orders} shopId={shopId} actions={actions} />
      ) : (
        <ListView orders={orders} shopId={shopId} actions={actions} />
      )}

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
