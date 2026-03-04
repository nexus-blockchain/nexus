"use client";

import { use, useState } from "react";
import { useOrders, useOrderActions } from "@/hooks/useOrder";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { formatBalance } from "@/lib/utils";
import { ORDER_STATUS } from "@/lib/constants";
import {
  ArrowLeft, ShoppingCart, Clock, Truck, CheckCircle, XCircle, RotateCcw,
} from "lucide-react";
import Link from "next/link";

const STATUS_GROUPS = [
  { label: "Pending", statuses: ["Pending", "Paid"], icon: Clock, color: "text-yellow-600" },
  { label: "Processing", statuses: ["Shipped", "ServiceStarted"], icon: Truck, color: "text-blue-600" },
  { label: "Completed", statuses: ["Delivered", "Completed", "ServiceCompleted"], icon: CheckCircle, color: "text-green-600" },
  { label: "Cancelled / Refund", statuses: ["Cancelled", "RefundRequested", "Refunded"], icon: XCircle, color: "text-red-600" },
] as const;

export default function OrdersPage({ params }: { params: Promise<{ shopId: string }> }) {
  const { shopId: shopIdStr } = use(params);
  const shopId = Number(shopIdStr);
  const { orders, isLoading, refetch } = useOrders(shopId);
  const actions = useOrderActions();
  const [activeGroup, setActiveGroup] = useState(0);

  const groupOrders = (statuses: readonly string[]) =>
    orders.filter((o) => statuses.includes(o.status));

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
        <Button variant="outline" size="sm" onClick={() => refetch()}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-4">
        {STATUS_GROUPS.map((group, i) => {
          const count = groupOrders(group.statuses).length;
          const Icon = group.icon;
          return (
            <Card
              key={group.label}
              className={`cursor-pointer transition-shadow hover:shadow-md ${activeGroup === i ? "ring-2 ring-primary" : ""}`}
              onClick={() => setActiveGroup(i)}
            >
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">{group.label}</CardTitle>
                <Icon className={`h-4 w-4 ${group.color}`} />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">{count}</div>
              </CardContent>
            </Card>
          );
        })}
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
      ) : (
        <div className="space-y-3">
          {groupOrders(STATUS_GROUPS[activeGroup].statuses).length === 0 ? (
            <Card>
              <CardContent className="flex flex-col items-center justify-center py-12">
                <ShoppingCart className="h-12 w-12 text-muted-foreground/50" />
                <p className="mt-4 text-lg font-medium">No {STATUS_GROUPS[activeGroup].label} Orders</p>
              </CardContent>
            </Card>
          ) : (
            groupOrders(STATUS_GROUPS[activeGroup].statuses).map((order) => (
              <Card key={order.id} className="hover:shadow-sm transition-shadow">
                <CardContent className="flex items-center gap-4 p-4">
                  <div className="flex-1 space-y-1">
                    <div className="flex items-center gap-3">
                      <span className="font-semibold">Order #{order.id}</span>
                      <Badge variant="outline">{order.status}</Badge>
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
                    {order.status === "Delivered" && (
                      <Button size="sm" onClick={() => actions.confirmReceipt(order.id)}>
                        <CheckCircle className="mr-1 h-3 w-3" />Confirm
                      </Button>
                    )}
                    {order.status === "RefundRequested" && (
                      <Button size="sm" onClick={() => actions.approveRefund(order.id)}>Approve Refund</Button>
                    )}
                    {order.status === "ServiceStarted" && (
                      <Button size="sm" onClick={() => actions.completeService(order.id)}>Complete Service</Button>
                    )}
                    <Button variant="outline" size="sm" asChild>
                      <Link href={`/shops/${shopId}/orders/${order.id}`}>Details</Link>
                    </Button>
                  </div>
                </CardContent>
              </Card>
            ))
          )}
        </div>
      )}

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
