"use client";

import { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { StatusBadge } from "@/components/shared/StatusBadge";
import {
  ArrowLeftRight,
  TrendingUp,
  TrendingDown,
  BarChart3,
  Activity,
  DollarSign,
  Clock,
} from "lucide-react";
import { useTranslations } from "next-intl";

const PLACEHOLDER_SELL_ORDERS = [
  { id: 1, seller: "5GrwvaEF...RjJQTPW", amount: "500.00", price: "1.05", total: "525.00" },
  { id: 2, seller: "5FHneW46...8BnWJ9S", amount: "1,200.00", price: "1.06", total: "1,272.00" },
  { id: 3, seller: "5DAAnrj7...4dKtWZq", amount: "250.00", price: "1.08", total: "270.00" },
];

const PLACEHOLDER_BUY_ORDERS = [
  { id: 4, buyer: "5HGjWAeF...TnZjNFP", amount: "800.00", price: "1.03", total: "824.00" },
  { id: 5, buyer: "5CiPPseX...QPjkpHd", amount: "2,000.00", price: "1.02", total: "2,040.00" },
  { id: 6, buyer: "5GNJqTPy...JCwDX26", amount: "350.00", price: "1.00", total: "350.00" },
];

export default function TradingPage() {
  const t = useTranslations("common");
  const [orderSide, setOrderSide] = useState<"buy" | "sell">("buy");
  const [price, setPrice] = useState("");
  const [amount, setAmount] = useState("");

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
          <ArrowLeftRight className="h-8 w-8" />
          NEX/USDT P2P Market
        </h1>
        <p className="text-muted-foreground">Peer-to-peer trading of NEX tokens with USDT settlement</p>
      </div>

      <div className="grid gap-4 md:grid-cols-5">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Last Price</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            <DollarSign className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">$1.04</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Best Ask</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            <TrendingUp className="h-4 w-4 text-red-500" />
            <p className="text-2xl font-bold text-red-600">$1.05</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Best Bid</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            <TrendingDown className="h-4 w-4 text-green-500" />
            <p className="text-2xl font-bold text-green-600">$1.03</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">24h Volume</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            <BarChart3 className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">12,450</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">TWAP</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            <Activity className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">$1.04</p>
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
            </CardHeader>
            <CardContent>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Seller</TableHead>
                    <TableHead className="text-right">Amount (NEX)</TableHead>
                    <TableHead className="text-right">Price (USDT)</TableHead>
                    <TableHead className="text-right">Total (USDT)</TableHead>
                    <TableHead className="text-right">Action</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {PLACEHOLDER_SELL_ORDERS.map((order) => (
                    <TableRow key={order.id}>
                      <TableCell className="font-mono text-xs">{order.seller}</TableCell>
                      <TableCell className="text-right font-mono">{order.amount}</TableCell>
                      <TableCell className="text-right font-mono text-red-600">{order.price}</TableCell>
                      <TableCell className="text-right font-mono">{order.total}</TableCell>
                      <TableCell className="text-right">
                        <Button size="sm" variant="outline">Buy</Button>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-green-600">
                <TrendingUp className="h-5 w-5" />
                Buy Orders (Bids)
              </CardTitle>
            </CardHeader>
            <CardContent>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Buyer</TableHead>
                    <TableHead className="text-right">Amount (NEX)</TableHead>
                    <TableHead className="text-right">Price (USDT)</TableHead>
                    <TableHead className="text-right">Total (USDT)</TableHead>
                    <TableHead className="text-right">Action</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {PLACEHOLDER_BUY_ORDERS.map((order) => (
                    <TableRow key={order.id}>
                      <TableCell className="font-mono text-xs">{order.buyer}</TableCell>
                      <TableCell className="text-right font-mono">{order.amount}</TableCell>
                      <TableCell className="text-right font-mono text-green-600">{order.price}</TableCell>
                      <TableCell className="text-right font-mono">{order.total}</TableCell>
                      <TableCell className="text-right">
                        <Button size="sm" variant="outline">Sell</Button>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </CardContent>
          </Card>
        </div>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Clock className="h-5 w-5" />
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
              <Input type="number" placeholder="0.00" value={price} onChange={(e) => setPrice(e.target.value)} />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Amount (NEX)</label>
              <Input type="number" placeholder="0" value={amount} onChange={(e) => setAmount(e.target.value)} />
            </div>
            <div className="rounded-lg border p-3 text-sm text-muted-foreground">
              <div className="flex justify-between">
                <span>Total</span>
                <span className="font-mono">
                  {price && amount ? (parseFloat(price) * parseFloat(amount)).toFixed(2) : "0.00"} USDT
                </span>
              </div>
            </div>
            <Button
              className={`w-full ${orderSide === "buy" ? "bg-green-600 hover:bg-green-700" : "bg-red-600 hover:bg-red-700"}`}
            >
              {orderSide === "buy" ? "Place Buy Order" : "Place Sell Order"}
            </Button>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
