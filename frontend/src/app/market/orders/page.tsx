"use client";

import { useState, useEffect, useCallback } from "react";
import { useEntityStore } from "@/stores/entity";
import { useMarketActions } from "@/hooks/useMarket";
import { useWalletStore } from "@/stores/wallet";
import { getApi } from "@/hooks/useApi";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { TxButton } from "@/components/shared/TxButton";
import { formatBalance } from "@/lib/utils";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ArrowLeft, RotateCcw, XCircle, ShoppingCart } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";
import type { MarketOrder } from "@/lib/types";

export default function MyMarketOrdersPage() {
  const { currentEntityId } = useEntityStore();
  const address = useWalletStore((s) => s.address);
  const actions = useMarketActions();
  const tc = useTranslations("common");

  const [orders, setOrders] = useState<MarketOrder[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetchOrders = useCallback(async () => {
    if (currentEntityId === null || !address) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityMarket.orders.entries();
      const myOrders = entries
        .map(([_k, v]: [unknown, { toJSON: () => MarketOrder }]) => v.toJSON())
        .filter((o: MarketOrder) => o.entityId === currentEntityId && o.maker === address);
      setOrders(myOrders);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [currentEntityId, address]);

  useEffect(() => { fetchOrders(); }, [fetchOrders]);

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const openOrders = orders.filter((o) => o.status === "Open");
  const filledOrders = orders.filter((o) => o.status === "Filled" || o.status === "PartiallyFilled");
  const cancelledOrders = orders.filter((o) => o.status === "Cancelled");

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/market"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">My Orders</h1>
          <p className="text-muted-foreground">Your market orders for the current entity</p>
        </div>
        <Button variant="outline" size="sm" onClick={fetchOrders}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Open Orders</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{openOrders.length}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Filled Orders</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{filledOrders.length}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Cancelled</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{cancelledOrders.length}</p></CardContent>
        </Card>
      </div>

      <Tabs defaultValue="open">
        <TabsList>
          <TabsTrigger value="open">Open ({openOrders.length})</TabsTrigger>
          <TabsTrigger value="filled">Filled ({filledOrders.length})</TabsTrigger>
          <TabsTrigger value="cancelled">Cancelled ({cancelledOrders.length})</TabsTrigger>
        </TabsList>

        {["open", "filled", "cancelled"].map((tab) => {
          const list = tab === "open" ? openOrders : tab === "filled" ? filledOrders : cancelledOrders;
          return (
            <TabsContent key={tab} value={tab} className="mt-4">
              {isLoading ? (
                <div className="flex justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
              ) : list.length === 0 ? (
                <Card>
                  <CardContent className="flex flex-col items-center justify-center py-12">
                    <ShoppingCart className="h-12 w-12 text-muted-foreground/50" />
                    <p className="mt-4 text-lg font-medium">No {tab} orders</p>
                  </CardContent>
                </Card>
              ) : (
                <Card>
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead>ID</TableHead>
                        <TableHead>Side</TableHead>
                        <TableHead className="text-right">Amount</TableHead>
                        <TableHead className="text-right">Price</TableHead>
                        <TableHead className="text-right">Filled</TableHead>
                        <TableHead>Status</TableHead>
                        {tab === "open" && <TableHead className="text-right">Action</TableHead>}
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {list.map((order) => (
                        <TableRow key={order.id}>
                          <TableCell className="font-mono">#{order.id}</TableCell>
                          <TableCell>
                            <Badge variant={order.side === "Buy" ? "default" : "secondary"}>
                              {order.side}
                            </Badge>
                          </TableCell>
                          <TableCell className="text-right font-mono">{formatBalance(order.tokenAmount)}</TableCell>
                          <TableCell className="text-right font-mono">{formatBalance(order.price)}</TableCell>
                          <TableCell className="text-right font-mono">{formatBalance(order.filled)}</TableCell>
                          <TableCell><Badge variant="outline">{order.status}</Badge></TableCell>
                          {tab === "open" && (
                            <TableCell className="text-right">
                              <TxButton
                                variant="ghost"
                                size="sm"
                                onClick={() => actions.cancelOrder(order.id)}
                                txStatus={actions.txState.status}
                              >
                                <XCircle className="mr-1 h-3 w-3" />Cancel
                              </TxButton>
                            </TableCell>
                          )}
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </Card>
              )}
            </TabsContent>
          );
        })}
      </Tabs>

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Order cancelled!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
