"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useMarketActions } from "@/hooks/useMarket";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { TxButton } from "@/components/shared/TxButton";
import { Separator } from "@/components/ui/separator";
import { ArrowLeft, Settings, Shield, DollarSign, AlertTriangle } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function MarketSettingsPage() {
  const { currentEntityId } = useEntityStore();
  const actions = useMarketActions();
  const tc = useTranslations("common");

  const [feeRate, setFeeRate] = useState("30");
  const [orderTtl, setOrderTtl] = useState("14400");
  const [minOrderAmount, setMinOrderAmount] = useState("0");
  const [initialPrice, setInitialPrice] = useState("");
  const [maxSlippage, setMaxSlippage] = useState("500");
  const [cbThreshold, setCbThreshold] = useState("1000");
  const [minTrades, setMinTrades] = useState("3");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/market"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Market Settings</h1>
          <p className="text-muted-foreground">Configure market parameters for your entity</p>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Settings className="h-5 w-5" />General Configuration</CardTitle>
          <CardDescription>Trading fees, order expiry, and minimum amounts</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 md:grid-cols-3">
            <div className="space-y-2">
              <label className="text-sm font-medium">Fee Rate (basis points)</label>
              <Input type="number" value={feeRate} onChange={(e) => setFeeRate(e.target.value)} min="0" max="10000" />
              <p className="text-xs text-muted-foreground">{Number(feeRate) / 100}% per trade</p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Order TTL (blocks)</label>
              <Input type="number" value={orderTtl} onChange={(e) => setOrderTtl(e.target.value)} min="0" />
              <p className="text-xs text-muted-foreground">~{Math.round(Number(orderTtl) * 6 / 3600)} hours</p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Min Order Amount</label>
              <Input type="number" value={minOrderAmount} onChange={(e) => setMinOrderAmount(e.target.value)} min="0" />
            </div>
          </div>
          <TxButton
            onClick={() => actions.configureMarket(currentEntityId, Number(feeRate), Number(orderTtl), BigInt(minOrderAmount))}
            txStatus={actions.txState.status}
          >
            Save Market Config
          </TxButton>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><DollarSign className="h-5 w-5" />Initial Price</CardTitle>
          <CardDescription>Set an initial reference price for the token market</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">Price (NEX per token)</label>
            <Input type="number" value={initialPrice} onChange={(e) => setInitialPrice(e.target.value)} placeholder="0" min="0" />
          </div>
          <TxButton
            onClick={() => initialPrice && actions.setInitialPrice(currentEntityId, BigInt(initialPrice))}
            txStatus={actions.txState.status}
            disabled={!initialPrice}
          >
            Set Initial Price
          </TxButton>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Shield className="h-5 w-5" />Price Protection</CardTitle>
          <CardDescription>Circuit breaker and slippage protection</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 md:grid-cols-3">
            <div className="space-y-2">
              <label className="text-sm font-medium">Max Slippage (bps)</label>
              <Input type="number" value={maxSlippage} onChange={(e) => setMaxSlippage(e.target.value)} min="0" />
              <p className="text-xs text-muted-foreground">{Number(maxSlippage) / 100}% max price deviation</p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Circuit Breaker (bps)</label>
              <Input type="number" value={cbThreshold} onChange={(e) => setCbThreshold(e.target.value)} min="0" />
              <p className="text-xs text-muted-foreground">{Number(cbThreshold) / 100}% triggers halt</p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Min Trades for TWAP</label>
              <Input type="number" value={minTrades} onChange={(e) => setMinTrades(e.target.value)} min="1" />
            </div>
          </div>
          <TxButton
            onClick={() => actions.configurePriceProtection(currentEntityId, Number(maxSlippage), Number(cbThreshold), Number(minTrades))}
            txStatus={actions.txState.status}
          >
            Save Price Protection
          </TxButton>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-orange-600"><AlertTriangle className="h-5 w-5" />Circuit Breaker</CardTitle>
          <CardDescription>Manually lift the circuit breaker if trading has been halted</CardDescription>
        </CardHeader>
        <CardContent>
          <TxButton variant="outline" onClick={() => actions.liftCircuitBreaker(currentEntityId)} txStatus={actions.txState.status}>
            <AlertTriangle className="mr-2 h-4 w-4" />Lift Circuit Breaker
          </TxButton>
        </CardContent>
      </Card>

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Settings saved!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
