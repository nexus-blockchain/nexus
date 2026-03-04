"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useToken, useTokenActions } from "@/hooks/useToken";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { TxButton } from "@/components/shared/TxButton";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { ArrowLeft, Coins, Settings, Send } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function DividendPage() {
  const { currentEntityId } = useEntityStore();
  const { config, isLoading } = useToken(currentEntityId);
  const actions = useTokenActions();
  const tc = useTranslations("common");

  const [enabled, setEnabled] = useState(false);
  const [interval, setInterval] = useState("100");
  const [minAmount, setMinAmount] = useState("0");
  const [distAmount, setDistAmount] = useState("");
  const [recipients, setRecipients] = useState("");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  if (isLoading) {
    return <div className="flex h-full items-center justify-center"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>;
  }

  const handleConfigure = () => {
    actions.configureDividend(currentEntityId, enabled, Number(interval), BigInt(minAmount || 0));
  };

  const handleDistribute = () => {
    if (!distAmount) return;
    const recipientList = recipients
      .split("\n")
      .map((r) => r.trim())
      .filter(Boolean);
    actions.distributeDividend(currentEntityId, BigInt(distAmount), recipientList);
  };

  const handleClaim = () => {
    actions.claimDividend(currentEntityId);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/token/config"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Dividend Management</h1>
          <p className="text-muted-foreground">Configure and distribute token dividends</p>
        </div>
      </div>

      {config?.dividendConfig && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">Current Configuration</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Status</span><Badge variant={config.dividendConfig.enabled ? "default" : "secondary"}>{config.dividendConfig.enabled ? "Enabled" : "Disabled"}</Badge></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Interval</span><span className="text-sm">{config.dividendConfig.interval} blocks</span></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Min Amount</span><span className="text-sm">{config.dividendConfig.minAmount.toString()}</span></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Last Distributed</span><span className="text-sm">Block #{config.dividendConfig.lastDistributed}</span></div>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Settings className="h-5 w-5" />Configure Dividend</CardTitle>
          <CardDescription>Set dividend parameters for your token</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <label className="text-sm font-medium">Enable Dividends</label>
              <p className="text-xs text-muted-foreground">Allow dividend distribution to holders</p>
            </div>
            <Switch checked={enabled} onCheckedChange={setEnabled} />
          </div>
          <div className="grid gap-4 md:grid-cols-2">
            <div className="space-y-2">
              <label className="text-sm font-medium">Distribution Interval (blocks)</label>
              <Input type="number" value={interval} onChange={(e) => setInterval(e.target.value)} min="1" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Minimum Amount</label>
              <Input type="number" value={minAmount} onChange={(e) => setMinAmount(e.target.value)} min="0" />
            </div>
          </div>
          <TxButton onClick={handleConfigure} txStatus={actions.txState.status}>
            Save Configuration
          </TxButton>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Send className="h-5 w-5" />Distribute Dividend</CardTitle>
          <CardDescription>Send dividends to token holders</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">Total Amount</label>
            <Input type="number" value={distAmount} onChange={(e) => setDistAmount(e.target.value)} placeholder="0" min="0" />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Recipients (one address per line, leave empty for all holders)</label>
            <textarea
              value={recipients}
              onChange={(e) => setRecipients(e.target.value)}
              placeholder="5xxx...&#10;5yyy...&#10;(leave empty for all)"
              rows={4}
              className="flex w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
            />
          </div>
          <TxButton onClick={handleDistribute} txStatus={actions.txState.status} disabled={!distAmount}>
            <Send className="mr-2 h-4 w-4" />Distribute
          </TxButton>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Coins className="h-5 w-5" />Claim Dividend</CardTitle>
          <CardDescription>Claim your pending dividend rewards</CardDescription>
        </CardHeader>
        <CardContent>
          <TxButton onClick={handleClaim} txStatus={actions.txState.status}>
            Claim My Dividend
          </TxButton>
        </CardContent>
      </Card>

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
