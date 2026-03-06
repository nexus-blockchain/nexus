"use client";

import { useState, useMemo } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Separator } from "@/components/ui/separator";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from "@/components/ui/dialog";
import {
  ArrowLeftRight,
  TrendingUp,
  TrendingDown,
  BarChart3,
  Activity,
  DollarSign,
  AlertTriangle,
  ShieldAlert,
  Percent,
  Loader2,
  RefreshCw,
} from "lucide-react";
import {
  useNexMarket,
  useTwap,
  usePriceProtection,
  useNexMarketActions,
} from "@/hooks/useNexMarket";
import { useWalletStore } from "@/stores/wallet";
import { NEX_ORDER_STATUS } from "@/lib/constants";
import type { NexOrder } from "@/lib/types";

const formatUsdt = (raw: number) => (raw / 1_000_000).toFixed(6);
const formatNex = (raw: bigint) => (Number(raw) / 1e12).toFixed(4);

function computeTwapPrice(
  currentCumulative: bigint,
  currentBlock: number,
  snapshot: { cumulativePrice: bigint; blockNumber: number }
): number | null {
  const blockDelta = currentBlock - snapshot.blockNumber;
  if (blockDelta <= 0) return null;
  const priceDelta = Number(currentCumulative - snapshot.cumulativePrice);
  return priceDelta / blockDelta;
}

export default function TradingPage() {
  const { sellOrders, buyOrders, bestAsk, bestBid, lastTradePrice, marketStats, isPaused, tradingFeeBps, isLoading, refetch } = useNexMarket();
  const { twap, isLoading: twapLoading } = useTwap();
  const { config: priceProtection } = usePriceProtection();
  const { placeSellOrder, placeBuyOrder, reserveSellOrder, acceptBuyOrder, txState, resetTx } = useNexMarketActions();
  const address = useWalletStore((s) => s.address);

  const [orderSide, setOrderSide] = useState<"buy" | "sell">("buy");
  const [price, setPrice] = useState("");
  const [amount, setAmount] = useState("");
  const [tronAddress, setTronAddress] = useState("");

  const [reserveDialog, setReserveDialog] = useState<NexOrder | null>(null);
  const [reserveAmount, setReserveAmount] = useState("");
  const [reserveTron, setReserveTron] = useState("");

  const [acceptDialog, setAcceptDialog] = useState<NexOrder | null>(null);
  const [acceptAmount, setAcceptAmount] = useState("");
  const [acceptTron, setAcceptTron] = useState("");

  const activeSellOrders = useMemo(
    () =>
      sellOrders
        .filter((o) => o.status === "Open" || o.status === "PartiallyFilled")
        .sort((a, b) => a.usdtPrice - b.usdtPrice),
    [sellOrders]
  );

  const activeBuyOrders = useMemo(
    () =>
      buyOrders
        .filter((o) => o.status === "Open" || o.status === "PartiallyFilled")
        .sort((a, b) => b.usdtPrice - a.usdtPrice),
    [buyOrders]
  );

  const twapPrices = useMemo(() => {
    if (!twap) return { h1: null, h24: null, w1: null };
    return {
      h1: computeTwapPrice(twap.currentCumulative, twap.currentBlock, twap.hourSnapshot),
      h24: computeTwapPrice(twap.currentCumulative, twap.currentBlock, twap.daySnapshot),
      w1: computeTwapPrice(twap.currentCumulative, twap.currentBlock, twap.weekSnapshot),
    };
  }, [twap]);

  const totalPreview = useMemo(() => {
    const p = parseFloat(price);
    const a = parseFloat(amount);
    if (isNaN(p) || isNaN(a) || p <= 0 || a <= 0) return "0.000000";
    return (p * a).toFixed(6);
  }, [price, amount]);

  const handlePlaceOrder = async () => {
    const p = parseFloat(price);
    const a = parseFloat(amount);
    if (isNaN(p) || isNaN(a) || p <= 0 || a <= 0 || !tronAddress) return;
    const nexRaw = BigInt(Math.round(a * 1e12));
    const usdtRaw = Math.round(p * 1_000_000);
    if (orderSide === "sell") {
      await placeSellOrder(nexRaw, usdtRaw, tronAddress);
    } else {
      await placeBuyOrder(nexRaw, usdtRaw, tronAddress);
    }
    refetch();
  };

  const handleReserve = async () => {
    if (!reserveDialog) return;
    const amt = reserveAmount ? BigInt(Math.round(parseFloat(reserveAmount) * 1e12)) : null;
    await reserveSellOrder(reserveDialog.orderId, amt, reserveTron);
    setReserveDialog(null);
    setReserveAmount("");
    setReserveTron("");
    refetch();
  };

  const handleAccept = async () => {
    if (!acceptDialog) return;
    const amt = acceptAmount ? BigInt(Math.round(parseFloat(acceptAmount) * 1e12)) : null;
    await acceptBuyOrder(acceptDialog.orderId, amt, acceptTron);
    setAcceptDialog(null);
    setAcceptAmount("");
    setAcceptTron("");
    refetch();
  };

  const remaining = (o: NexOrder) => o.nexAmount - o.filledAmount;

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <ArrowLeftRight className="h-8 w-8" />
            NEX/USDT P2P Market
          </h1>
          <p className="text-muted-foreground">
            Peer-to-peer trading of NEX tokens with USDT settlement
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Badge variant="outline" className="gap-1">
            <Percent className="h-3 w-3" />
            Fee: {(tradingFeeBps / 100).toFixed(2)}%
          </Badge>
          <Button variant="outline" size="sm" onClick={() => refetch()} disabled={isLoading}>
            <RefreshCw className={`mr-2 h-3 w-3 ${isLoading ? "animate-spin" : ""}`} />
            Refresh
          </Button>
        </div>
      </div>

      {isPaused && (
        <div className="rounded-lg border border-red-300 bg-red-50 dark:bg-red-950/20 p-4 flex items-start gap-3">
          <AlertTriangle className="h-5 w-5 text-red-600 mt-0.5 shrink-0" />
          <div>
            <p className="font-semibold text-red-700 dark:text-red-400">Market Paused</p>
            <p className="text-sm text-red-600 dark:text-red-300">
              Trading is currently suspended.
              {priceProtection?.circuitBreakerActive && (
                <> Circuit breaker triggered — threshold: {priceProtection.circuitBreakerThreshold / 100}%, resumes block #{priceProtection.circuitBreakerUntil}.</>
              )}
            </p>
          </div>
        </div>
      )}

      {priceProtection?.circuitBreakerActive && !isPaused && (
        <div className="rounded-lg border border-amber-300 bg-amber-50 dark:bg-amber-950/20 p-4 flex items-start gap-3">
          <ShieldAlert className="h-5 w-5 text-amber-600 mt-0.5 shrink-0" />
          <div>
            <p className="font-semibold text-amber-700 dark:text-amber-400">Circuit Breaker Active</p>
            <p className="text-sm text-amber-600 dark:text-amber-300">
              Price deviation exceeded {priceProtection.circuitBreakerThreshold / 100}%. Protection active until block #{priceProtection.circuitBreakerUntil}.
            </p>
          </div>
        </div>
      )}

      <div className="grid gap-4 md:grid-cols-5">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Last Price</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            <DollarSign className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">
              {lastTradePrice !== null ? `$${formatUsdt(lastTradePrice)}` : "—"}
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Best Ask</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            <TrendingUp className="h-4 w-4 text-red-500" />
            <p className="text-2xl font-bold text-red-600">
              {bestAsk !== null ? `$${formatUsdt(bestAsk)}` : "—"}
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Best Bid</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            <TrendingDown className="h-4 w-4 text-green-500" />
            <p className="text-2xl font-bold text-green-600">
              {bestBid !== null ? `$${formatUsdt(bestBid)}` : "—"}
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">24h Volume</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            <BarChart3 className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">
              {marketStats ? `$${formatUsdt(marketStats.totalVolumeUsdt)}` : "—"}
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">TWAP</CardTitle>
          </CardHeader>
          <CardContent>
            {twapLoading ? (
              <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
            ) : (
              <div className="space-y-1 text-sm">
                <div className="flex justify-between">
                  <span className="text-muted-foreground">1H</span>
                  <span className="font-mono font-semibold">
                    {twapPrices.h1 !== null ? `$${formatUsdt(twapPrices.h1)}` : twap?.lastPrice ? `$${formatUsdt(twap.lastPrice)}` : "—"}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">24H</span>
                  <span className="font-mono font-semibold">
                    {twapPrices.h24 !== null ? `$${formatUsdt(twapPrices.h24)}` : twap?.lastPrice ? `$${formatUsdt(twap.lastPrice)}` : "—"}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">7D</span>
                  <span className="font-mono font-semibold">
                    {twapPrices.w1 !== null ? `$${formatUsdt(twapPrices.w1)}` : twap?.lastPrice ? `$${formatUsdt(twap.lastPrice)}` : "—"}
                  </span>
                </div>
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-6 lg:grid-cols-3">
        <div className="lg:col-span-2 space-y-4">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-red-600">
                <TrendingDown className="h-5 w-5" />
                Sell Orders (Asks)
              </CardTitle>
              <CardDescription>{activeSellOrders.length} active sell orders — sorted by price ascending</CardDescription>
            </CardHeader>
            <CardContent>
              {isLoading ? (
                <div className="flex justify-center py-8">
                  <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
                </div>
              ) : activeSellOrders.length === 0 ? (
                <p className="text-center py-8 text-muted-foreground">No sell orders</p>
              ) : (
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Maker</TableHead>
                      <TableHead className="text-right">Amount (NEX)</TableHead>
                      <TableHead className="text-right">Price (USDT)</TableHead>
                      <TableHead className="text-right">Total (USDT)</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead className="text-right">Action</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {activeSellOrders.map((order) => {
                      const rem = remaining(order);
                      const totalUsdt = (Number(rem) / 1e12) * (order.usdtPrice / 1_000_000);
                      return (
                        <TableRow key={order.orderId}>
                          <TableCell>
                            <AddressDisplay address={order.maker} chars={6} />
                          </TableCell>
                          <TableCell className="text-right font-mono">{formatNex(rem)}</TableCell>
                          <TableCell className="text-right font-mono text-red-600">
                            ${formatUsdt(order.usdtPrice)}
                          </TableCell>
                          <TableCell className="text-right font-mono">
                            ${totalUsdt.toFixed(6)}
                          </TableCell>
                          <TableCell>
                            <StatusBadge status={order.status} />
                          </TableCell>
                          <TableCell className="text-right">
                            <Button
                              size="sm"
                              variant="outline"
                              disabled={order.maker === address || isPaused}
                              onClick={() => {
                                resetTx();
                                setReserveDialog(order);
                                setReserveAmount(formatNex(rem));
                              }}
                            >
                              Buy
                            </Button>
                          </TableCell>
                        </TableRow>
                      );
                    })}
                  </TableBody>
                </Table>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-green-600">
                <TrendingUp className="h-5 w-5" />
                Buy Orders (Bids)
              </CardTitle>
              <CardDescription>{activeBuyOrders.length} active buy orders — sorted by price descending</CardDescription>
            </CardHeader>
            <CardContent>
              {isLoading ? (
                <div className="flex justify-center py-8">
                  <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
                </div>
              ) : activeBuyOrders.length === 0 ? (
                <p className="text-center py-8 text-muted-foreground">No buy orders</p>
              ) : (
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Maker</TableHead>
                      <TableHead className="text-right">Amount (NEX)</TableHead>
                      <TableHead className="text-right">Price (USDT)</TableHead>
                      <TableHead className="text-right">Total (USDT)</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead className="text-right">Action</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {activeBuyOrders.map((order) => {
                      const rem = remaining(order);
                      const totalUsdt = (Number(rem) / 1e12) * (order.usdtPrice / 1_000_000);
                      return (
                        <TableRow key={order.orderId}>
                          <TableCell>
                            <AddressDisplay address={order.maker} chars={6} />
                          </TableCell>
                          <TableCell className="text-right font-mono">{formatNex(rem)}</TableCell>
                          <TableCell className="text-right font-mono text-green-600">
                            ${formatUsdt(order.usdtPrice)}
                          </TableCell>
                          <TableCell className="text-right font-mono">
                            ${totalUsdt.toFixed(6)}
                          </TableCell>
                          <TableCell>
                            <StatusBadge status={order.status} />
                          </TableCell>
                          <TableCell className="text-right">
                            <Button
                              size="sm"
                              variant="outline"
                              disabled={order.maker === address || isPaused}
                              onClick={() => {
                                resetTx();
                                setAcceptDialog(order);
                                setAcceptAmount(formatNex(rem));
                              }}
                            >
                              Sell
                            </Button>
                          </TableCell>
                        </TableRow>
                      );
                    })}
                  </TableBody>
                </Table>
              )}
            </CardContent>
          </Card>
        </div>

        <Card className="h-fit">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <DollarSign className="h-5 w-5" />
              Place Order
            </CardTitle>
            <CardDescription>Submit a new buy or sell order</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <Tabs value={orderSide} onValueChange={(v) => setOrderSide(v as "buy" | "sell")}>
              <TabsList className="w-full">
                <TabsTrigger value="buy" className="flex-1">Buy NEX</TabsTrigger>
                <TabsTrigger value="sell" className="flex-1">Sell NEX</TabsTrigger>
              </TabsList>
            </Tabs>

            <div className="space-y-2">
              <label className="text-sm font-medium">Price (USDT)</label>
              <Input
                type="number"
                placeholder="0.000000"
                step="0.000001"
                min="0"
                value={price}
                onChange={(e) => setPrice(e.target.value)}
              />
              {bestAsk !== null && bestBid !== null && (
                <div className="flex gap-2">
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-6 text-xs text-red-600"
                    onClick={() => setPrice((bestAsk / 1_000_000).toFixed(6))}
                  >
                    Ask ${formatUsdt(bestAsk)}
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-6 text-xs text-green-600"
                    onClick={() => setPrice((bestBid / 1_000_000).toFixed(6))}
                  >
                    Bid ${formatUsdt(bestBid)}
                  </Button>
                </div>
              )}
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">Amount (NEX)</label>
              <Input
                type="number"
                placeholder="0.0000"
                step="0.0001"
                min="0"
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
              />
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">TRON Address (TRC-20)</label>
              <Input
                type="text"
                placeholder="T..."
                value={tronAddress}
                onChange={(e) => setTronAddress(e.target.value)}
              />
            </div>

            <Separator />

            <div className="rounded-lg border p-3 space-y-2 text-sm">
              <div className="flex justify-between">
                <span className="text-muted-foreground">Total</span>
                <span className="font-mono font-semibold">{totalPreview} USDT</span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">Trading Fee</span>
                <span className="font-mono">{(tradingFeeBps / 100).toFixed(2)}%</span>
              </div>
            </div>

            <TxButton
              className={`w-full ${orderSide === "buy" ? "bg-green-600 hover:bg-green-700" : "bg-red-600 hover:bg-red-700"}`}
              txStatus={txState.status}
              loadingText="Submitting..."
              disabled={!address || isPaused || !price || !amount || !tronAddress}
              onClick={handlePlaceOrder}
            >
              {orderSide === "buy" ? "Place Buy Order" : "Place Sell Order"}
            </TxButton>

            {txState.status === "error" && (
              <p className="text-sm text-red-600">{"Transaction failed"}</p>
            )}
            {txState.status === "finalized" && (
              <p className="text-sm text-green-600">Order placed successfully!</p>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Reserve Sell Order Dialog (Buy from seller) */}
      <Dialog open={!!reserveDialog} onOpenChange={(open) => { if (!open) setReserveDialog(null); }}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Buy from Sell Order #{reserveDialog?.orderId}</DialogTitle>
            <DialogDescription>
              Reserve NEX from this sell order at ${reserveDialog ? formatUsdt(reserveDialog.usdtPrice) : "0"}/NEX
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="rounded-lg border p-3 text-sm space-y-1">
              <div className="flex justify-between">
                <span className="text-muted-foreground">Available</span>
                <span className="font-mono">{reserveDialog ? formatNex(remaining(reserveDialog)) : "0"} NEX</span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">Price</span>
                <span className="font-mono">${reserveDialog ? formatUsdt(reserveDialog.usdtPrice) : "0"}</span>
              </div>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Amount (NEX)</label>
              <Input
                type="number"
                placeholder="Leave empty for full amount"
                value={reserveAmount}
                onChange={(e) => setReserveAmount(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Your TRON Address</label>
              <Input
                type="text"
                placeholder="T..."
                value={reserveTron}
                onChange={(e) => setReserveTron(e.target.value)}
              />
            </div>
            {reserveAmount && reserveDialog && (
              <div className="rounded-lg border p-3 text-sm">
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Total Cost</span>
                  <span className="font-mono font-semibold">
                    ${(parseFloat(reserveAmount) * (reserveDialog.usdtPrice / 1_000_000)).toFixed(6)} USDT
                  </span>
                </div>
              </div>
            )}
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setReserveDialog(null)}>Cancel</Button>
            <TxButton
              txStatus={txState.status}
              loadingText="Reserving..."
              disabled={!reserveTron}
              onClick={handleReserve}
            >
              Confirm Buy
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Accept Buy Order Dialog (Sell to buyer) */}
      <Dialog open={!!acceptDialog} onOpenChange={(open) => { if (!open) setAcceptDialog(null); }}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Sell to Buy Order #{acceptDialog?.orderId}</DialogTitle>
            <DialogDescription>
              Accept this buy order at ${acceptDialog ? formatUsdt(acceptDialog.usdtPrice) : "0"}/NEX
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="rounded-lg border p-3 text-sm space-y-1">
              <div className="flex justify-between">
                <span className="text-muted-foreground">Requested</span>
                <span className="font-mono">{acceptDialog ? formatNex(remaining(acceptDialog)) : "0"} NEX</span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">Price</span>
                <span className="font-mono">${acceptDialog ? formatUsdt(acceptDialog.usdtPrice) : "0"}</span>
              </div>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Amount (NEX)</label>
              <Input
                type="number"
                placeholder="Leave empty for full amount"
                value={acceptAmount}
                onChange={(e) => setAcceptAmount(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Your TRON Address (receive USDT)</label>
              <Input
                type="text"
                placeholder="T..."
                value={acceptTron}
                onChange={(e) => setAcceptTron(e.target.value)}
              />
            </div>
            {acceptAmount && acceptDialog && (
              <div className="rounded-lg border p-3 text-sm">
                <div className="flex justify-between">
                  <span className="text-muted-foreground">You Receive</span>
                  <span className="font-mono font-semibold">
                    ${(parseFloat(acceptAmount) * (acceptDialog.usdtPrice / 1_000_000)).toFixed(6)} USDT
                  </span>
                </div>
              </div>
            )}
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setAcceptDialog(null)}>Cancel</Button>
            <TxButton
              txStatus={txState.status}
              loadingText="Accepting..."
              disabled={!acceptTron}
              onClick={handleAccept}
            >
              Confirm Sell
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
