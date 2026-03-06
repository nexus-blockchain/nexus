"use client";

import { use, useState } from "react";
import { useOrders, useOrderActions } from "@/hooks/useOrder";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { CidDisplay } from "@/components/shared/CidDisplay";
import { TxButton } from "@/components/shared/TxButton";
import { formatBalance } from "@/lib/utils";
import {
  ArrowLeft, ShoppingCart, Truck, CheckCircle, XCircle,
  Package, CreditCard, AlertTriangle, Clock, Coins, Banknote, FileText,
} from "lucide-react";
import Link from "next/link";

const ORDER_TIMELINE_STEPS = [
  { status: "Created", label: "Created", icon: Clock },
  { status: "Paid", label: "Paid", icon: CreditCard },
  { status: "Shipped", label: "Shipped", icon: Truck },
  { status: "Completed", label: "Completed", icon: CheckCircle },
] as const;

const STATUS_INDEX: Record<string, number> = {
  Created: 0,
  Paid: 1,
  Shipped: 2,
  Completed: 3,
};

const TERMINAL_STATUSES = ["Cancelled", "Disputed", "Refunded", "Expired"];

export default function OrderDetailPage({ params }: { params: Promise<{ shopId: string; orderId: string }> }) {
  const { shopId: shopIdStr, orderId: orderIdStr } = use(params);
  const shopId = Number(shopIdStr);
  const orderId = Number(orderIdStr);
  const { orders, isLoading } = useOrders(shopId);
  const actions = useOrderActions();
  const [trackingCid, setTrackingCid] = useState("");
  const [refundReason, setRefundReason] = useState("");
  const [disputeReason, setDisputeReason] = useState("");

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

  const isTerminal = TERMINAL_STATUSES.includes(order.status);
  const currentStep = STATUS_INDEX[order.status] ?? -1;

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
            {order.paymentAsset === "EntityToken" ? (
              <Badge variant="secondary" className="gap-1"><Coins className="h-3 w-3" />Token Payment</Badge>
            ) : (
              <Badge variant="outline" className="gap-1"><Banknote className="h-3 w-3" />NEX Payment</Badge>
            )}
          </div>
          <p className="text-muted-foreground">Shop #{shopId} &middot; Product #{order.productId}</p>
        </div>
      </div>

      {!isTerminal && (
        <Card>
          <CardContent className="py-6">
            <div className="flex items-center justify-between">
              {ORDER_TIMELINE_STEPS.map((step, i) => {
                const Icon = step.icon;
                const isCompleted = i <= currentStep;
                const isCurrent = i === currentStep;
                return (
                  <div key={step.status} className="flex flex-1 items-center">
                    <div className="flex flex-col items-center gap-1.5">
                      <div className={`flex h-10 w-10 items-center justify-center rounded-full border-2 transition-colors ${
                        isCurrent
                          ? "border-primary bg-primary text-primary-foreground"
                          : isCompleted
                            ? "border-primary bg-primary/10 text-primary"
                            : "border-muted bg-background text-muted-foreground"
                      }`}>
                        <Icon className="h-5 w-5" />
                      </div>
                      <span className={`text-xs font-medium ${isCurrent ? "text-primary" : isCompleted ? "text-foreground" : "text-muted-foreground"}`}>
                        {step.label}
                      </span>
                    </div>
                    {i < ORDER_TIMELINE_STEPS.length - 1 && (
                      <div className={`mx-2 h-0.5 flex-1 ${i < currentStep ? "bg-primary" : "bg-muted"}`} />
                    )}
                  </div>
                );
              })}
            </div>
          </CardContent>
        </Card>
      )}

      {isTerminal && (
        <Card className="border-destructive/50">
          <CardContent className="flex items-center gap-3 py-4">
            {order.status === "Cancelled" && <XCircle className="h-5 w-5 text-muted-foreground" />}
            {order.status === "Disputed" && <AlertTriangle className="h-5 w-5 text-red-500" />}
            {order.status === "Refunded" && <CreditCard className="h-5 w-5 text-purple-500" />}
            {order.status === "Expired" && <Clock className="h-5 w-5 text-muted-foreground" />}
            <div>
              <p className="text-sm font-medium">This order is {order.status.toLowerCase()}</p>
              <p className="text-xs text-muted-foreground">No further actions available for this order.</p>
            </div>
          </CardContent>
        </Card>
      )}

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Package className="h-5 w-5" />Order Details</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Order ID</span><span className="text-sm font-mono">#{order.id}</span></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Entity</span><span className="text-sm">#{order.entityId}</span></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Product</span><span className="text-sm">#{order.productId}</span></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Quantity</span><span className="text-sm">{order.quantity}</span></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Payment Asset</span>
              <span className="text-sm">{order.paymentAsset === "EntityToken" ? "Entity Token" : "NEX (Native)"}</span>
            </div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Total Price</span><span className="text-sm font-semibold">{formatBalance(order.totalPrice)} NEX</span></div>
            {order.tokenDiscount > 0 && (
              <>
                <Separator />
                <div className="flex justify-between"><span className="text-sm text-muted-foreground">Token Discount</span><span className="text-sm text-green-600">-{formatBalance(order.tokenDiscount)} NEX</span></div>
              </>
            )}
            {order.shoppingBalanceDiscount > 0 && (
              <>
                <Separator />
                <div className="flex justify-between"><span className="text-sm text-muted-foreground">Balance Discount</span><span className="text-sm text-green-600">-{formatBalance(order.shoppingBalanceDiscount)} NEX</span></div>
              </>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Parties & Timestamps</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex justify-between items-center"><span className="text-sm text-muted-foreground">Buyer</span><AddressDisplay address={order.buyer} /></div>
            <Separator />
            <div className="flex justify-between items-center"><span className="text-sm text-muted-foreground">Seller</span><AddressDisplay address={order.seller} /></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Created</span><span className="text-sm">Block #{order.createdAt}</span></div>
            {order.paidAt && (
              <>
                <Separator />
                <div className="flex justify-between"><span className="text-sm text-muted-foreground">Paid At</span><span className="text-sm">Block #{order.paidAt}</span></div>
              </>
            )}
            {order.shippedAt && (
              <>
                <Separator />
                <div className="flex justify-between"><span className="text-sm text-muted-foreground">Shipped At</span><span className="text-sm">Block #{order.shippedAt}</span></div>
              </>
            )}
            {order.completedAt && (
              <>
                <Separator />
                <div className="flex justify-between"><span className="text-sm text-muted-foreground">Completed At</span><span className="text-sm">Block #{order.completedAt}</span></div>
              </>
            )}
            {order.trackingCid && (
              <>
                <Separator />
                <div className="flex justify-between items-center"><span className="text-sm text-muted-foreground">Tracking</span><CidDisplay cid={order.trackingCid} /></div>
              </>
            )}
            {order.reasonCid && (
              <>
                <Separator />
                <div className="flex justify-between items-center"><span className="text-sm text-muted-foreground">Reason</span><CidDisplay cid={order.reasonCid} /></div>
              </>
            )}
          </CardContent>
        </Card>
      </div>

      {!isTerminal && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><FileText className="h-5 w-5" />Actions</CardTitle>
            <CardDescription>Available actions based on current order status</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {order.status === "Paid" && (
              <div className="flex items-end gap-4 rounded-lg border p-4">
                <div className="flex-1 space-y-2">
                  <label className="text-sm font-medium">Tracking CID</label>
                  <Input value={trackingCid} onChange={(e) => setTrackingCid(e.target.value)} placeholder="IPFS CID for tracking info" />
                </div>
                <TxButton onClick={() => actions.shipOrder(orderId, trackingCid)} txStatus={actions.txState.status}>
                  <Truck className="mr-2 h-4 w-4" />Ship Order
                </TxButton>
              </div>
            )}

            {order.status === "Shipped" && (
              <div className="rounded-lg border p-4">
                <TxButton onClick={() => actions.confirmReceipt(orderId)} txStatus={actions.txState.status}>
                  <CheckCircle className="mr-2 h-4 w-4" />Confirm Receipt
                </TxButton>
              </div>
            )}

            {(order.status === "Created" || order.status === "Paid") && (
              <div className="space-y-4 border-t pt-4">
                <div className="flex items-end gap-4">
                  <div className="flex-1 space-y-2">
                    <label className="text-sm font-medium">Cancel Reason (optional)</label>
                    <Input value={refundReason} onChange={(e) => setRefundReason(e.target.value)} placeholder="Reason CID" />
                  </div>
                  <Button variant="destructive" onClick={() => actions.cancelOrder(orderId)}>
                    <XCircle className="mr-2 h-4 w-4" />Cancel Order
                  </Button>
                </div>
              </div>
            )}

            {(order.status === "Paid" || order.status === "Shipped") && (
              <div className="space-y-2 border-t pt-4">
                <label className="text-sm font-medium text-destructive">Open Dispute</label>
                <div className="flex items-end gap-4">
                  <div className="flex-1 space-y-2">
                    <Input value={disputeReason} onChange={(e) => setDisputeReason(e.target.value)} placeholder="Evidence CID for dispute" />
                  </div>
                  <Button variant="destructive" onClick={() => actions.disputeOrder(orderId, disputeReason)} disabled={!disputeReason}>
                    <AlertTriangle className="mr-2 h-4 w-4" />Dispute
                  </Button>
                </div>
              </div>
            )}
          </CardContent>
        </Card>
      )}

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
