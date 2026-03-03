"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useOrderbook, useMarketActions } from "@/hooks/useMarket";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Badge } from "@/components/ui/badge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TrendingUp, TrendingDown, ArrowUpDown } from "lucide-react";

export default function MarketPage() {
  const { currentEntityId } = useEntityStore();
  const { buyOrders, sellOrders, isLoading } = useOrderbook(currentEntityId);
  const actions = useMarketActions();
  const [buyPrice, setBuyPrice] = useState("");
  const [buyAmount, setBuyAmount] = useState("");
  const [sellPrice, setSellPrice] = useState("");
  const [sellAmount, setSellAmount] = useState("");

  if (!currentEntityId) return <div className="flex h-full items-center justify-center text-muted-foreground">Select an entity first</div>;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Token Market</h1>
        <p className="text-muted-foreground">NEX orderbook and USDT OTC trading</p>
      </div>

      <Tabs defaultValue="nex">
        <TabsList>
          <TabsTrigger value="nex">NEX Orderbook</TabsTrigger>
          <TabsTrigger value="usdt">USDT OTC</TabsTrigger>
        </TabsList>

        <TabsContent value="nex" className="space-y-6">
          <div className="grid gap-6 md:grid-cols-2">
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2 text-green-600"><TrendingUp className="h-5 w-5" />Buy Orders</CardTitle>
                <CardDescription>Place a buy order for entity tokens</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Price (NEX per token)</label>
                  <Input type="number" value={buyPrice} onChange={(e) => setBuyPrice(e.target.value)} placeholder="0.00" />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Amount (tokens)</label>
                  <Input type="number" value={buyAmount} onChange={(e) => setBuyAmount(e.target.value)} placeholder="0" />
                </div>
                <Button className="w-full bg-green-600 hover:bg-green-700" onClick={() => { if (buyPrice && buyAmount && currentEntityId) actions.placeBuyOrder(currentEntityId, BigInt(buyAmount), BigInt(buyPrice)); }}>
                  Place Buy Order
                </Button>

                <div className="mt-4 space-y-2">
                  <h4 className="text-sm font-semibold">Open Buy Orders</h4>
                  {isLoading ? (
                    <p className="text-xs text-muted-foreground">Loading...</p>
                  ) : buyOrders.length === 0 ? (
                    <p className="text-xs text-muted-foreground">No buy orders</p>
                  ) : (
                    <div className="max-h-48 space-y-1 overflow-y-auto">
                      {buyOrders.map((order, i) => (
                        <div key={i} className="flex items-center justify-between rounded border px-2 py-1 text-xs">
                          <span className="text-green-600 font-medium">{order.price}</span>
                          <span>{String(order.tokenAmount)}</span>
                          <AddressDisplay address={order.maker} chars={3} showCopy={false} />
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2 text-red-600"><TrendingDown className="h-5 w-5" />Sell Orders</CardTitle>
                <CardDescription>Place a sell order for entity tokens</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Price (NEX per token)</label>
                  <Input type="number" value={sellPrice} onChange={(e) => setSellPrice(e.target.value)} placeholder="0.00" />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Amount (tokens)</label>
                  <Input type="number" value={sellAmount} onChange={(e) => setSellAmount(e.target.value)} placeholder="0" />
                </div>
                <Button variant="destructive" className="w-full" onClick={() => { if (sellPrice && sellAmount && currentEntityId) actions.placeSellOrder(currentEntityId, BigInt(sellAmount), BigInt(sellPrice)); }}>
                  Place Sell Order
                </Button>

                <div className="mt-4 space-y-2">
                  <h4 className="text-sm font-semibold">Open Sell Orders</h4>
                  {isLoading ? (
                    <p className="text-xs text-muted-foreground">Loading...</p>
                  ) : sellOrders.length === 0 ? (
                    <p className="text-xs text-muted-foreground">No sell orders</p>
                  ) : (
                    <div className="max-h-48 space-y-1 overflow-y-auto">
                      {sellOrders.map((order, i) => (
                        <div key={i} className="flex items-center justify-between rounded border px-2 py-1 text-xs">
                          <span className="text-red-600 font-medium">{order.price}</span>
                          <span>{String(order.tokenAmount)}</span>
                          <AddressDisplay address={order.maker} chars={3} showCopy={false} />
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </CardContent>
            </Card>
          </div>
        </TabsContent>

        <TabsContent value="usdt" className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2"><ArrowUpDown className="h-5 w-5" />USDT OTC Market</CardTitle>
              <CardDescription>Over-the-counter trading with USDT payment verification</CardDescription>
            </CardHeader>
            <CardContent>
              <p className="text-sm text-muted-foreground">USDT OTC trading interface will be available once the market is configured for this entity.</p>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
