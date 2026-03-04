"use client";

import { use } from "react";
import { useOrders, useOrderActions } from "@/hooks/useOrder";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import { formatBalance } from "@/lib/utils";
import { Separator } from "@/components/ui/separator";
import {
  ArrowLeft, ShoppingCart, Truck, CheckCircle, XCircle, RotateCcw, Package,
} from "lucide-react";
import Link from "next/link";
import { useState } from "react";

export default function OrderDetailPage({ params }: { params: Promise<{ shopId: string; orderId: string }> }) {
  const { shopId: shopIdStr, orderId: orderIdStr } = use(params);
  const shopId = Number(shopIdStr);
  const orderId = Number(orderIdStr);
  const { orders, isLoading } = useOrders(shopId);
  const actions = useOrderActions();
  const [trackingCid, setTrackingCid] = useState("");
  const [refundReason, setRefundReason] = useState("");

  const order = orders.find((o) => o.id === orderId);

  if (isLoading) {
    return <div className="flex h-full items-center justify-center"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>;
  }

  if (!order) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4">
        <ShoppingCart className="h-16 w-16 text-muted-foreground/50" />
        <p className="text-muted-foreground">Order not found</p>
        <Button variant="outline" asChild><Link href={`/shops/${shopId}/orders`}>Back to Orders</Link></Button>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href={`/shops/${shopId}/orders`}><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <div className="flex items-center gap-3">
            <h1 className="text-3xl font-bold tracking-tight">Order #{orderId}</h1>
            <StatusBadge status={order.status} />
          </div>
          <p className="text-muted-foreground">Shop #{shopId} &middot; Product #{order.productId}</p>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Package className="h-5 w-5" />Order Details</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Order ID</span><span className="text-sm font-mono">#{order.id}</span></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Product</span><span className="text-sm">#{order.productId}</span></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Quantity</span><span className="text-sm">{order.quantity}</span></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Total Price</span><span className="text-sm font-semibold">{formatBalance(order.totalPrice)} NEX</span></div>
            <Separator />
            {order.tokenDiscount > 0 && (
              <>
                <div className="flex justify-between"><span className="text-sm text-muted-foreground">Token Discount</span><span className="text-sm text-green-600">-{formatBalance(order.tokenDiscount)} NEX</span></div>
                <Separator />
              </>
            )}
            {order.shoppingBalanceDiscount > 0 && (
              <>
                <div className="flex justify-between"><span className="text-sm text-muted-foreground">Balance Discount</span><span className="text-sm text-green-600">-{formatBalance(order.shoppingBalanceDiscount)} NEX</span></div>
                <Separator />
              </>
            )}
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Created</span><span className="text-sm">Block #{order.createdAt}</span></div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Buyer Information</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex justify-between items-center"><span className="text-sm text-muted-foreground">Address</span><AddressDisplay address={order.buyer} /></div>
            <Separator />
            {order.trackingCid && (
              <>
                <div className="flex justify-between"><span className="text-sm text-muted-foreground">Tracking</span><span className="text-sm font-mono">{order.trackingCid.slice(0, 20)}...</span></div>
                <Separator />
              </>
            )}
            {order.reasonCid && (
              <>
                <div className="flex justify-between"><span className="text-sm text-muted-foreground">Reason</span><span className="text-sm font-mono">{order.reasonCid.slice(0, 20)}...</span></div>
                <Separator />
              </>
            )}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Actions</CardTitle>
          <CardDescription>Available actions based on current order status</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {order.status === "Paid" && (
            <div className="flex items-end gap-4">
              <div className="flex-1 space-y-2">
                <label className="text-sm font-medium">Tracking CID</label>
                <Input value={trackingCid} onChange={(e) => setTrackingCid(e.target.value)} placeholder="IPFS CID for tracking info" />
              </div>
              <TxButton onClick={() => actions.shipOrder(orderId, trackingCid)} txStatus={actions.txState.status}>
                <Truck className="mr-2 h-4 w-4" />Ship Order
              </TxButton>
            </div>
          )}

          {order.status === "Delivered" && (
            <TxButton onClick={() => actions.confirmReceipt(orderId)} txStatus={actions.txState.status}>
              <CheckCircle className="mr-2 h-4 w-4" />Confirm Receipt
            </TxButton>
          )}

          {order.status === "RefundRequested" && (
            <div className="flex gap-4">
              <TxButton onClick={() => actions.approveRefund(orderId)} txStatus={actions.txState.status}>
                <CheckCircle className="mr-2 h-4 w-4" />Approve Refund
              </TxButton>
            </div>
          )}

          {order.status === "ServiceStarted" && (
            <TxButton onClick={() => actions.completeService(orderId)} txStatus={actions.txState.status}>
              <CheckCircle className="mr-2 h-4 w-4" />Complete Service
            </TxButton>
          )}

          {(order.status === "Pending" || order.status === "Paid") && (
            <div className="flex items-end gap-4 border-t pt-4">
              <div className="flex-1 space-y-2">
                <label className="text-sm font-medium">Reason (optional)</label>
                <Input value={refundReason} onChange={(e) => setRefundReason(e.target.value)} placeholder="Reason CID for cancellation" />
              </div>
              <Button variant="destructive" onClick={() => actions.cancelOrder(orderId)}>
                <XCircle className="mr-2 h-4 w-4" />Cancel Order
              </Button>
            </div>
          )}

          {["Completed", "Cancelled", "Refunded"].includes(order.status) && (
            <p className="text-sm text-muted-foreground">This order is finalized. No actions available.</p>
          )}
        </CardContent>
      </Card>

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
