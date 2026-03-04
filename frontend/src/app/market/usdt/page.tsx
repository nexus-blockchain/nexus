"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useMarketActions } from "@/hooks/useMarket";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { TxButton } from "@/components/shared/TxButton";
import { Separator } from "@/components/ui/separator";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ArrowLeft, DollarSign, ArrowUpRight, ArrowDownRight, CheckCircle } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function UsdtOtcPage() {
  const { currentEntityId } = useEntityStore();
  const actions = useMarketActions();
  const tc = useTranslations("common");

  const [sellAmount, setSellAmount] = useState("");
  const [sellUsdtPrice, setSellUsdtPrice] = useState("");
  const [tronAddress, setTronAddress] = useState("");
  const [buyAmount, setBuyAmount] = useState("");
  const [buyUsdtPrice, setBuyUsdtPrice] = useState("");
  const [confirmTradeId, setConfirmTradeId] = useState("");
  const [confirmTxHash, setConfirmTxHash] = useState("");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const handlePlaceSell = () => {
    if (!sellAmount || !sellUsdtPrice || !tronAddress) return;
    actions.placeUsdtSellOrder(currentEntityId, BigInt(sellAmount), Number(sellUsdtPrice), tronAddress);
  };

  const handlePlaceBuy = () => {
    if (!buyAmount || !buyUsdtPrice) return;
    actions.placeUsdtBuyOrder(currentEntityId, BigInt(buyAmount), Number(buyUsdtPrice));
  };

  const handleConfirmPayment = () => {
    if (!confirmTradeId || !confirmTxHash) return;
    actions.confirmUsdtPayment(Number(confirmTradeId), confirmTxHash);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/market"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">USDT OTC Market</h1>
          <p className="text-muted-foreground">Cross-chain USDT trading with on-chain verification</p>
        </div>
      </div>

      <Card>
        <CardContent className="p-4">
          <div className="flex items-start gap-3">
            <DollarSign className="mt-0.5 h-5 w-5 text-blue-500" />
            <div className="text-sm">
              <p className="font-medium">How USDT OTC Works</p>
              <p className="text-muted-foreground mt-1">
                1. Seller locks tokens on-chain and provides a TRON/ERC20 address.
                2. Buyer places a matching order and sends USDT off-chain.
                3. Buyer confirms payment with the USDT transaction hash.
                4. An off-chain worker verifies the payment and releases tokens.
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      <Tabs defaultValue="sell">
        <TabsList className="grid w-full grid-cols-3">
          <TabsTrigger value="sell">Sell for USDT</TabsTrigger>
          <TabsTrigger value="buy">Buy with USDT</TabsTrigger>
          <TabsTrigger value="confirm">Confirm Payment</TabsTrigger>
        </TabsList>

        <TabsContent value="sell" className="mt-4">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2"><ArrowUpRight className="h-5 w-5 text-red-500" />Sell Tokens for USDT</CardTitle>
              <CardDescription>Lock tokens and receive USDT payment off-chain</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Token Amount</label>
                  <Input type="number" value={sellAmount} onChange={(e) => setSellAmount(e.target.value)} placeholder="0" min="0" />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">USDT Price (per token)</label>
                  <Input type="number" value={sellUsdtPrice} onChange={(e) => setSellUsdtPrice(e.target.value)} placeholder="0.00" min="0" step="0.01" />
                </div>
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">TRON Receiving Address</label>
                <Input value={tronAddress} onChange={(e) => setTronAddress(e.target.value)} placeholder="T..." />
                <p className="text-xs text-muted-foreground">Your TRON address to receive USDT (TRC20)</p>
              </div>
              {sellAmount && sellUsdtPrice && (
                <div className="rounded-lg border bg-muted/50 p-3">
                  <p className="text-sm"><span className="text-muted-foreground">Total USDT:</span> <span className="font-semibold">${(Number(sellAmount) * Number(sellUsdtPrice)).toFixed(2)}</span></p>
                </div>
              )}
              <TxButton onClick={handlePlaceSell} txStatus={actions.txState.status} disabled={!sellAmount || !sellUsdtPrice || !tronAddress}>
                <ArrowUpRight className="mr-2 h-4 w-4" />Place Sell Order
              </TxButton>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="buy" className="mt-4">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2"><ArrowDownRight className="h-5 w-5 text-green-500" />Buy Tokens with USDT</CardTitle>
              <CardDescription>Deposit NEX collateral and pay USDT off-chain</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Token Amount</label>
                  <Input type="number" value={buyAmount} onChange={(e) => setBuyAmount(e.target.value)} placeholder="0" min="0" />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">USDT Price (per token)</label>
                  <Input type="number" value={buyUsdtPrice} onChange={(e) => setBuyUsdtPrice(e.target.value)} placeholder="0.00" min="0" step="0.01" />
                </div>
              </div>
              {buyAmount && buyUsdtPrice && (
                <div className="rounded-lg border bg-muted/50 p-3">
                  <p className="text-sm"><span className="text-muted-foreground">Total USDT to pay:</span> <span className="font-semibold">${(Number(buyAmount) * Number(buyUsdtPrice)).toFixed(2)}</span></p>
                </div>
              )}
              <TxButton onClick={handlePlaceBuy} txStatus={actions.txState.status} disabled={!buyAmount || !buyUsdtPrice}>
                <ArrowDownRight className="mr-2 h-4 w-4" />Place Buy Order
              </TxButton>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="confirm" className="mt-4">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2"><CheckCircle className="h-5 w-5 text-blue-500" />Confirm USDT Payment</CardTitle>
              <CardDescription>After sending USDT off-chain, confirm with the transaction hash</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Trade ID</label>
                <Input type="number" value={confirmTradeId} onChange={(e) => setConfirmTradeId(e.target.value)} placeholder="Trade ID from the order" />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">USDT Transaction Hash</label>
                <Input value={confirmTxHash} onChange={(e) => setConfirmTxHash(e.target.value)} placeholder="0x... or TRON tx hash" />
                <p className="text-xs text-muted-foreground">The off-chain worker will verify this transaction on the TRON network</p>
              </div>
              <TxButton onClick={handleConfirmPayment} txStatus={actions.txState.status} disabled={!confirmTradeId || !confirmTxHash}>
                <CheckCircle className="mr-2 h-4 w-4" />Confirm Payment
              </TxButton>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
