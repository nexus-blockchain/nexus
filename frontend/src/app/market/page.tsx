"use client";

import { useState, useMemo } from "react";
import { useEntityStore } from "@/stores/entity";
import { useOrderbook, useMarketActions } from "@/hooks/useMarket";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import { formatBalance } from "@/lib/utils";
import {
  TrendingUp,
  TrendingDown,
  ArrowUpDown,
  BarChart3,
  Activity,
  RefreshCw,
  ShoppingCart,
  Tag,
} from "lucide-react";
import { useTranslations } from "next-intl";

export default function MarketPage() {
  const { currentEntityId } = useEntityStore();
  const { buyOrders, sellOrders, isLoading, refetch } =
    useOrderbook(currentEntityId);
  const actions = useMarketActions();
  const t = useTranslations("market");
  const tc = useTranslations("common");

  const [orderTab, setOrderTab] = useState<"buy" | "sell">("buy");
  const [orderMode, setOrderMode] = useState<"limit" | "market">("limit");

  const [buyPrice, setBuyPrice] = useState("");
  const [buyAmount, setBuyAmount] = useState("");
  const [sellPrice, setSellPrice] = useState("");
  const [sellAmount, setSellAmount] = useState("");

  const [marketBuyAmount, setMarketBuyAmount] = useState("");
  const [marketBuyMaxCost, setMarketBuyMaxCost] = useState("");
  const [marketSellAmount, setMarketSellAmount] = useState("");
  const [marketSellMinReceive, setMarketSellMinReceive] = useState("");

  const [takeAmount, setTakeAmount] = useState<Record<number, string>>({});

  const stats = useMemo(() => {
    const bestBid =
      buyOrders.length > 0 ? BigInt(String(buyOrders[0].price)) : null;
    const bestAsk =
      sellOrders.length > 0 ? BigInt(String(sellOrders[0].price)) : null;
    const spread =
      bestBid !== null && bestAsk !== null ? bestAsk - bestBid : null;

    const maxBuyAmount =
      buyOrders.length > 0
        ? Math.max(
            ...buyOrders.map((o) => Number(BigInt(String(o.tokenAmount))))
          )
        : 1;
    const maxSellAmount =
      sellOrders.length > 0
        ? Math.max(
            ...sellOrders.map((o) => Number(BigInt(String(o.tokenAmount))))
          )
        : 1;

    return { bestBid, bestAsk, spread, maxBuyAmount, maxSellAmount };
  }, [buyOrders, sellOrders]);

  if (!currentEntityId) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        {tc("selectEntity")}
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t("title")}</h1>
          <p className="text-muted-foreground">{t("subtitle")}</p>
        </div>
        <Button variant="outline" size="sm" onClick={refetch}>
          <RefreshCw className="mr-2 h-3 w-3" />
          Refresh
        </Button>
      </div>

      <Tabs defaultValue="nex">
        <TabsList>
          <TabsTrigger value="nex">NEX Orderbook</TabsTrigger>
          <TabsTrigger value="usdt">USDT OTC</TabsTrigger>
        </TabsList>

        <TabsContent value="nex" className="space-y-6">
          {/* Market Stats */}
          <div className="grid gap-4 md:grid-cols-4">
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium flex items-center gap-2">
                  <Activity className="h-4 w-4 text-muted-foreground" />
                  Spread
                </CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold">
                  {stats.spread !== null
                    ? formatBalance(stats.spread)
                    : "—"}
                </p>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium flex items-center gap-2">
                  <TrendingUp className="h-4 w-4 text-green-600" />
                  Best Bid
                </CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold text-green-600">
                  {stats.bestBid !== null
                    ? formatBalance(stats.bestBid)
                    : "—"}
                </p>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium flex items-center gap-2">
                  <TrendingDown className="h-4 w-4 text-red-600" />
                  Best Ask
                </CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold text-red-600">
                  {stats.bestAsk !== null
                    ? formatBalance(stats.bestAsk)
                    : "—"}
                </p>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium flex items-center gap-2">
                  <BarChart3 className="h-4 w-4 text-muted-foreground" />
                  Total Volume
                </CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold text-muted-foreground">—</p>
                <p className="text-xs text-muted-foreground">Coming soon</p>
              </CardContent>
            </Card>
          </div>

          {/* Orderbook Visualization */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <BarChart3 className="h-5 w-5" />
                Order Book
              </CardTitle>
              <CardDescription>
                {sellOrders.length} sell orders · {buyOrders.length} buy orders
              </CardDescription>
            </CardHeader>
            <CardContent>
              {isLoading ? (
                <div className="flex justify-center py-12">
                  <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
                </div>
              ) : sellOrders.length === 0 && buyOrders.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-12">
                  <ArrowUpDown className="h-12 w-12 text-muted-foreground/50" />
                  <p className="mt-4 text-sm text-muted-foreground">
                    No open orders
                  </p>
                </div>
              ) : (
                <div className="grid gap-6 md:grid-cols-2">
                  {/* Sell Orders (Asks) */}
                  <div className="space-y-3">
                    <div className="flex items-center gap-2">
                      <TrendingDown className="h-4 w-4 text-red-600" />
                      <h4 className="text-sm font-semibold text-red-600">
                        Sell Orders (Asks)
                      </h4>
                    </div>
                    <div className="space-y-1">
                      <div className="grid grid-cols-4 text-xs text-muted-foreground px-2 pb-1">
                        <span>Price</span>
                        <span>Amount</span>
                        <span>Maker</span>
                        <span className="text-right">Action</span>
                      </div>
                      {sellOrders.length === 0 ? (
                        <p className="text-xs text-muted-foreground text-center py-4">
                          No sell orders
                        </p>
                      ) : (
                        sellOrders.map((order, i) => {
                          const amount = Number(
                            BigInt(String(order.tokenAmount))
                          );
                          const barWidth =
                            stats.maxSellAmount > 0
                              ? (amount / stats.maxSellAmount) * 100
                              : 0;
                          return (
                            <div
                              key={`sell-${i}`}
                              className="relative rounded px-2 py-2"
                            >
                              <div
                                className="absolute inset-y-0 right-0 rounded bg-red-100/60 dark:bg-red-900/20 transition-all"
                                style={{ width: `${barWidth}%` }}
                              />
                              <div className="relative grid grid-cols-4 items-center text-xs">
                                <span className="text-red-600 font-mono font-medium">
                                  {formatBalance(
                                    BigInt(String(order.price))
                                  )}
                                </span>
                                <span className="font-mono">
                                  {formatBalance(
                                    BigInt(String(order.tokenAmount))
                                  )}
                                </span>
                                <AddressDisplay
                                  address={order.maker}
                                  chars={3}
                                />
                                <div className="text-right">
                                  <Button
                                    variant="ghost"
                                    size="sm"
                                    className="h-6 px-2 text-xs text-green-600 hover:text-green-700"
                                    onClick={() =>
                                      actions.takeOrder(
                                        order.id,
                                        takeAmount[order.id]
                                          ? BigInt(takeAmount[order.id])
                                          : null
                                      )
                                    }
                                  >
                                    Buy
                                  </Button>
                                </div>
                              </div>
                            </div>
                          );
                        })
                      )}
                    </div>
                  </div>

                  {/* Buy Orders (Bids) */}
                  <div className="space-y-3">
                    <div className="flex items-center gap-2">
                      <TrendingUp className="h-4 w-4 text-green-600" />
                      <h4 className="text-sm font-semibold text-green-600">
                        Buy Orders (Bids)
                      </h4>
                    </div>
                    <div className="space-y-1">
                      <div className="grid grid-cols-4 text-xs text-muted-foreground px-2 pb-1">
                        <span>Price</span>
                        <span>Amount</span>
                        <span>Maker</span>
                        <span className="text-right">Action</span>
                      </div>
                      {buyOrders.length === 0 ? (
                        <p className="text-xs text-muted-foreground text-center py-4">
                          No buy orders
                        </p>
                      ) : (
                        buyOrders.map((order, i) => {
                          const amount = Number(
                            BigInt(String(order.tokenAmount))
                          );
                          const barWidth =
                            stats.maxBuyAmount > 0
                              ? (amount / stats.maxBuyAmount) * 100
                              : 0;
                          return (
                            <div
                              key={`buy-${i}`}
                              className="relative rounded px-2 py-2"
                            >
                              <div
                                className="absolute inset-y-0 left-0 rounded bg-green-100/60 dark:bg-green-900/20 transition-all"
                                style={{ width: `${barWidth}%` }}
                              />
                              <div className="relative grid grid-cols-4 items-center text-xs">
                                <span className="text-green-600 font-mono font-medium">
                                  {formatBalance(
                                    BigInt(String(order.price))
                                  )}
                                </span>
                                <span className="font-mono">
                                  {formatBalance(
                                    BigInt(String(order.tokenAmount))
                                  )}
                                </span>
                                <AddressDisplay
                                  address={order.maker}
                                  chars={3}
                                />
                                <div className="text-right">
                                  <Button
                                    variant="ghost"
                                    size="sm"
                                    className="h-6 px-2 text-xs text-red-600 hover:text-red-700"
                                    onClick={() =>
                                      actions.takeOrder(
                                        order.id,
                                        takeAmount[order.id]
                                          ? BigInt(takeAmount[order.id])
                                          : null
                                      )
                                    }
                                  >
                                    Sell
                                  </Button>
                                </div>
                              </div>
                            </div>
                          );
                        })
                      )}
                    </div>
                  </div>
                </div>
              )}
            </CardContent>
          </Card>

          {/* Place Order Form */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <ShoppingCart className="h-5 w-5" />
                Place Order
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              {/* Buy/Sell Toggle */}
              <div className="flex rounded-lg border p-1">
                <button
                  type="button"
                  onClick={() => setOrderTab("buy")}
                  className={`flex-1 rounded-md py-2 text-sm font-medium transition-colors ${
                    orderTab === "buy"
                      ? "bg-green-600 text-white"
                      : "text-muted-foreground hover:text-foreground"
                  }`}
                >
                  <TrendingUp className="inline mr-1 h-4 w-4" />
                  Buy
                </button>
                <button
                  type="button"
                  onClick={() => setOrderTab("sell")}
                  className={`flex-1 rounded-md py-2 text-sm font-medium transition-colors ${
                    orderTab === "sell"
                      ? "bg-red-600 text-white"
                      : "text-muted-foreground hover:text-foreground"
                  }`}
                >
                  <TrendingDown className="inline mr-1 h-4 w-4" />
                  Sell
                </button>
              </div>

              {/* Limit / Market Toggle */}
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={() => setOrderMode("limit")}
                  className={`rounded-md border px-4 py-1.5 text-sm font-medium transition-colors ${
                    orderMode === "limit"
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-border text-muted-foreground hover:border-primary/40"
                  }`}
                >
                  <Tag className="inline mr-1 h-3 w-3" />
                  Limit
                </button>
                <button
                  type="button"
                  onClick={() => setOrderMode("market")}
                  className={`rounded-md border px-4 py-1.5 text-sm font-medium transition-colors ${
                    orderMode === "market"
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-border text-muted-foreground hover:border-primary/40"
                  }`}
                >
                  <Activity className="inline mr-1 h-3 w-3" />
                  Market
                </button>
              </div>

              <Separator />

              {orderTab === "buy" ? (
                orderMode === "limit" ? (
                  <div className="space-y-4">
                    <div className="grid gap-4 md:grid-cols-2">
                      <div className="space-y-2">
                        <label className="text-sm font-medium">
                          Price (NEX per token)
                        </label>
                        <Input
                          type="number"
                          value={buyPrice}
                          onChange={(e) => setBuyPrice(e.target.value)}
                          placeholder="0.00"
                        />
                      </div>
                      <div className="space-y-2">
                        <label className="text-sm font-medium">
                          Amount (tokens)
                        </label>
                        <Input
                          type="number"
                          value={buyAmount}
                          onChange={(e) => setBuyAmount(e.target.value)}
                          placeholder="0"
                        />
                      </div>
                    </div>
                    <TxButton
                      onClick={() => {
                        if (buyPrice && buyAmount && currentEntityId)
                          actions.placeBuyOrder(
                            currentEntityId,
                            BigInt(buyAmount),
                            BigInt(buyPrice)
                          );
                      }}
                      txStatus={actions.txState.status}
                      disabled={!buyPrice || !buyAmount}
                      className="w-full bg-green-600 hover:bg-green-700"
                    >
                      <TrendingUp className="mr-2 h-4 w-4" />
                      Place Limit Buy Order
                    </TxButton>
                  </div>
                ) : (
                  <div className="space-y-4">
                    <div className="grid gap-4 md:grid-cols-2">
                      <div className="space-y-2">
                        <label className="text-sm font-medium">
                          Amount (tokens to buy)
                        </label>
                        <Input
                          type="number"
                          value={marketBuyAmount}
                          onChange={(e) => setMarketBuyAmount(e.target.value)}
                          placeholder="0"
                        />
                      </div>
                      <div className="space-y-2">
                        <label className="text-sm font-medium">
                          Max Cost (NEX)
                        </label>
                        <Input
                          type="number"
                          value={marketBuyMaxCost}
                          onChange={(e) =>
                            setMarketBuyMaxCost(e.target.value)
                          }
                          placeholder="0"
                        />
                      </div>
                    </div>
                    <TxButton
                      onClick={() => {
                        if (
                          marketBuyAmount &&
                          marketBuyMaxCost &&
                          currentEntityId
                        )
                          actions.marketBuy(
                            currentEntityId,
                            BigInt(marketBuyAmount),
                            BigInt(marketBuyMaxCost)
                          );
                      }}
                      txStatus={actions.txState.status}
                      disabled={!marketBuyAmount || !marketBuyMaxCost}
                      className="w-full bg-green-600 hover:bg-green-700"
                    >
                      <TrendingUp className="mr-2 h-4 w-4" />
                      Market Buy
                    </TxButton>
                  </div>
                )
              ) : orderMode === "limit" ? (
                <div className="space-y-4">
                  <div className="grid gap-4 md:grid-cols-2">
                    <div className="space-y-2">
                      <label className="text-sm font-medium">
                        Price (NEX per token)
                      </label>
                      <Input
                        type="number"
                        value={sellPrice}
                        onChange={(e) => setSellPrice(e.target.value)}
                        placeholder="0.00"
                      />
                    </div>
                    <div className="space-y-2">
                      <label className="text-sm font-medium">
                        Amount (tokens)
                      </label>
                      <Input
                        type="number"
                        value={sellAmount}
                        onChange={(e) => setSellAmount(e.target.value)}
                        placeholder="0"
                      />
                    </div>
                  </div>
                  <TxButton
                    onClick={() => {
                      if (sellPrice && sellAmount && currentEntityId)
                        actions.placeSellOrder(
                          currentEntityId,
                          BigInt(sellAmount),
                          BigInt(sellPrice)
                        );
                    }}
                    txStatus={actions.txState.status}
                    disabled={!sellPrice || !sellAmount}
                    className="w-full bg-red-600 hover:bg-red-700"
                  >
                    <TrendingDown className="mr-2 h-4 w-4" />
                    Place Limit Sell Order
                  </TxButton>
                </div>
              ) : (
                <div className="space-y-4">
                  <div className="grid gap-4 md:grid-cols-2">
                    <div className="space-y-2">
                      <label className="text-sm font-medium">
                        Amount (tokens to sell)
                      </label>
                      <Input
                        type="number"
                        value={marketSellAmount}
                        onChange={(e) => setMarketSellAmount(e.target.value)}
                        placeholder="0"
                      />
                    </div>
                    <div className="space-y-2">
                      <label className="text-sm font-medium">
                        Min Receive (NEX)
                      </label>
                      <Input
                        type="number"
                        value={marketSellMinReceive}
                        onChange={(e) =>
                          setMarketSellMinReceive(e.target.value)
                        }
                        placeholder="0"
                      />
                    </div>
                  </div>
                  <TxButton
                    onClick={() => {
                      if (
                        marketSellAmount &&
                        marketSellMinReceive &&
                        currentEntityId
                      )
                        actions.marketSell(
                          currentEntityId,
                          BigInt(marketSellAmount),
                          BigInt(marketSellMinReceive)
                        );
                    }}
                    txStatus={actions.txState.status}
                    disabled={!marketSellAmount || !marketSellMinReceive}
                    className="w-full bg-red-600 hover:bg-red-700"
                  >
                    <TrendingDown className="mr-2 h-4 w-4" />
                    Market Sell
                  </TxButton>
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="usdt" className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <ArrowUpDown className="h-5 w-5" />
                USDT OTC Market
              </CardTitle>
              <CardDescription>
                Over-the-counter trading with USDT payment verification
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div className="flex flex-col items-center justify-center py-12">
                <div className="rounded-full bg-muted p-4">
                  <ArrowUpDown className="h-8 w-8 text-muted-foreground" />
                </div>
                <p className="mt-4 text-lg font-medium">
                  USDT OTC Trading
                </p>
                <p className="mt-1 text-sm text-muted-foreground text-center max-w-md">
                  The USDT OTC trading interface will be available once the
                  market is configured for this entity. Contact the entity
                  administrator to enable OTC trading.
                </p>
              </div>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>

      {actions.txState.status === "finalized" && (
        <div className="rounded-lg border border-green-200 bg-green-50 p-3 text-sm text-green-700 dark:border-green-800 dark:bg-green-950/50 dark:text-green-400">
          Order placed successfully.
        </div>
      )}
      {actions.txState.status === "error" && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
          {actions.txState.error}
        </div>
      )}
    </div>
  );
}
