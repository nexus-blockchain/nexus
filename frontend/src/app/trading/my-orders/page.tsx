"use client";

import { useState, useMemo } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import {
  Table, TableHeader, TableBody, TableRow, TableHead, TableCell,
} from "@/components/ui/table";
import { Separator } from "@/components/ui/separator";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import {
  ShoppingCart, ArrowLeft, RotateCcw, XCircle, Pencil, Check,
  Loader2, PackageOpen,
} from "lucide-react";
import Link from "next/link";
import { useWalletStore } from "@/stores/wallet";
import { useUserOrders } from "@/hooks/useNexMarket";
import { useNexMarketActions } from "@/hooks/useNexMarket";
import { NEX_ORDER_STATUS } from "@/lib/constants";
import type { NexOrder } from "@/lib/types";

const formatUsdt = (raw: number) => (raw / 1_000_000).toFixed(6);
const formatNex = (raw: bigint) => (Number(raw) / 1e12).toFixed(4);

const STATUS_FILTERS = ["All", ...NEX_ORDER_STATUS] as const;

export default function TradingMyOrdersPage() {
  const { address } = useWalletStore();
  const { orders, isLoading, refetch } = useUserOrders(address);
  const {
    cancelOrder, updateOrderPrice, txState, resetTx,
  } = useNexMarketActions();

  const [statusFilter, setStatusFilter] = useState<string>("All");
  const [editingOrderId, setEditingOrderId] = useState<number | null>(null);
  const [newPrice, setNewPrice] = useState("");

  const filteredOrders = useMemo(() => {
    if (statusFilter === "All") return orders;
    return orders.filter((o) => o.status === statusFilter);
  }, [orders, statusFilter]);

  const stats = useMemo(() => ({
    open: orders.filter((o) => o.status === "Open").length,
    partial: orders.filter((o) => o.status === "PartiallyFilled").length,
    filled: orders.filter((o) => o.status === "Filled").length,
    cancelled: orders.filter((o) => o.status === "Cancelled").length,
  }), [orders]);

  const handleCancel = async (orderId: number) => {
    await cancelOrder(orderId);
    refetch();
  };

  const handleUpdatePrice = async (orderId: number) => {
    if (!newPrice) return;
    const priceVal = Math.round(parseFloat(newPrice) * 1_000_000);
    await updateOrderPrice(orderId, priceVal);
    setEditingOrderId(null);
    setNewPrice("");
    refetch();
  };

  const fillPercent = (order: NexOrder) => {
    if (order.nexAmount === 0n) return 0;
    return Number((order.filledAmount * 10000n) / order.nexAmount) / 100;
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/trading"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">My Orders</h1>
          <p className="text-muted-foreground">
            Manage your active and historical NEX/USDT orders
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={() => refetch()} disabled={isLoading}>
          {isLoading
            ? <Loader2 className="mr-2 h-3 w-3 animate-spin" />
            : <RotateCcw className="mr-2 h-3 w-3" />}
          Refresh
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Open</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-blue-600">{stats.open}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Partially Filled</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-cyan-600">{stats.partial}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Filled</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-green-600">{stats.filled}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Cancelled</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-gray-500">{stats.cancelled}</p>
          </CardContent>
        </Card>
      </div>

      <div className="flex flex-wrap gap-2">
        {STATUS_FILTERS.map((s) => (
          <Button
            key={s}
            variant={statusFilter === s ? "default" : "outline"}
            size="sm"
            onClick={() => setStatusFilter(s)}
          >
            {s === "PartiallyFilled" ? "Partial" : s}
          </Button>
        ))}
      </div>

      {filteredOrders.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <PackageOpen className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">
              {isLoading ? "Loading orders..." : "No orders found"}
            </p>
            <p className="text-sm text-muted-foreground">
              {isLoading
                ? "Fetching your orders from the chain"
                : "Place your first order on the trading page"}
            </p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-[70px]">ID</TableHead>
                  <TableHead className="w-[70px]">Side</TableHead>
                  <TableHead className="text-right">NEX Amount</TableHead>
                  <TableHead className="w-[160px]">Fill Progress</TableHead>
                  <TableHead className="text-right">Price (USDT)</TableHead>
                  <TableHead className="text-right">Total (USDT)</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Tron Address</TableHead>
                  <TableHead className="text-right">Created</TableHead>
                  <TableHead className="text-right">Expires</TableHead>
                  <TableHead className="text-right w-[200px]">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filteredOrders.map((order) => {
                  const pct = fillPercent(order);
                  const usdtTotal = (Number(order.nexAmount) / 1e12) * (order.usdtPrice / 1_000_000);
                  const isActive = order.status === "Open" || order.status === "PartiallyFilled";

                  return (
                    <TableRow key={order.orderId}>
                      <TableCell className="font-mono">#{order.orderId}</TableCell>
                      <TableCell>
                        <Badge
                          variant={order.side === "Buy" ? "default" : "secondary"}
                          className={
                            order.side === "Buy"
                              ? "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400"
                              : "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400"
                          }
                        >
                          {order.side}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-right font-mono">
                        {formatNex(order.nexAmount)}
                      </TableCell>
                      <TableCell>
                        <div className="flex items-center gap-2">
                          <div className="flex-1 h-2 rounded-full bg-muted overflow-hidden">
                            <div
                              className="h-full rounded-full bg-primary transition-all"
                              style={{ width: `${pct}%` }}
                            />
                          </div>
                          <span className="text-xs text-muted-foreground w-[40px] text-right">
                            {pct.toFixed(0)}%
                          </span>
                        </div>
                        <p className="text-xs text-muted-foreground mt-0.5">
                          {formatNex(order.filledAmount)} / {formatNex(order.nexAmount)}
                        </p>
                      </TableCell>
                      <TableCell className="text-right font-mono">
                        {editingOrderId === order.orderId ? (
                          <div className="flex items-center gap-1 justify-end">
                            <Input
                              type="number"
                              step="0.000001"
                              className="w-24 h-7 text-xs"
                              value={newPrice}
                              onChange={(e) => setNewPrice(e.target.value)}
                              placeholder={formatUsdt(order.usdtPrice)}
                            />
                            <TxButton
                              size="icon"
                              variant="ghost"
                              className="h-7 w-7"
                              txStatus={txState.status}
                              onClick={() => handleUpdatePrice(order.orderId)}
                            >
                              <Check className="h-3 w-3" />
                            </TxButton>
                            <Button
                              size="icon"
                              variant="ghost"
                              className="h-7 w-7"
                              onClick={() => { setEditingOrderId(null); setNewPrice(""); }}
                            >
                              <XCircle className="h-3 w-3" />
                            </Button>
                          </div>
                        ) : (
                          formatUsdt(order.usdtPrice)
                        )}
                      </TableCell>
                      <TableCell className="text-right font-mono">
                        {usdtTotal.toFixed(2)}
                      </TableCell>
                      <TableCell>
                        <StatusBadge status={order.status} />
                      </TableCell>
                      <TableCell className="font-mono text-xs">
                        {order.tronAddress ? (
                          <span title={order.tronAddress}>
                            {order.tronAddress.slice(0, 6)}...{order.tronAddress.slice(-4)}
                          </span>
                        ) : (
                          <span className="text-muted-foreground">—</span>
                        )}
                      </TableCell>
                      <TableCell className="text-right font-mono text-xs text-muted-foreground">
                        #{order.createdAt}
                      </TableCell>
                      <TableCell className="text-right font-mono text-xs text-muted-foreground">
                        #{order.expiresAt}
                      </TableCell>
                      <TableCell className="text-right">
                        {isActive && (
                          <div className="flex items-center gap-1 justify-end">
                            <Button
                              variant="ghost"
                              size="sm"
                              className="h-7 text-xs"
                              onClick={() => {
                                setEditingOrderId(order.orderId);
                                setNewPrice("");
                                resetTx();
                              }}
                              disabled={editingOrderId === order.orderId}
                            >
                              <Pencil className="mr-1 h-3 w-3" />
                              Price
                            </Button>
                            <TxButton
                              variant="ghost"
                              size="sm"
                              className="h-7 text-xs text-destructive hover:text-destructive"
                              txStatus={txState.status}
                              onClick={() => handleCancel(order.orderId)}
                            >
                              <XCircle className="mr-1 h-3 w-3" />
                              Cancel
                            </TxButton>
                          </div>
                        )}
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        </Card>
      )}
    </div>
  );
}
