"use client";

import { use, useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useShops, useShopActions } from "@/hooks/useShop";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { TxButton } from "@/components/shared/TxButton";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { ArrowLeft, Gift, Settings, Send, Trash2 } from "lucide-react";
import Link from "next/link";

export default function PointsPage({ params }: { params: Promise<{ shopId: string }> }) {
  const { shopId: shopIdStr } = use(params);
  const shopId = Number(shopIdStr);
  const { currentEntityId } = useEntityStore();
  const { shops, isLoading } = useShops(currentEntityId);
  const actions = useShopActions();

  const [ptName, setPtName] = useState("");
  const [ptSymbol, setPtSymbol] = useState("");
  const [rewardRate, setRewardRate] = useState("100");
  const [exchangeRate, setExchangeRate] = useState("100");
  const [transferable, setTransferable] = useState(false);
  const [transferTo, setTransferTo] = useState("");
  const [transferAmount, setTransferAmount] = useState("");

  const shop = shops.find((s) => s.id === shopId);

  if (isLoading) {
    return <div className="flex h-full items-center justify-center"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>;
  }

  if (!shop) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">Shop not found</div>;
  }

  const handleEnable = () => {
    if (ptName && ptSymbol) {
      actions.enablePoints(shopId, ptName, ptSymbol, Number(rewardRate), Number(exchangeRate), transferable);
    }
  };

  const handleUpdateConfig = () => {
    actions.updatePointsConfig(shopId, Number(rewardRate), Number(exchangeRate), transferable);
  };

  const handleTransfer = () => {
    if (transferTo && transferAmount) {
      actions.transferPoints(shopId, transferTo, BigInt(transferAmount));
    }
  };

  // Points system is implicitly configured if shop has product count > 0 or a flag
  // For now we show enable/configure UI
  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href={`/shops/${shopId}`}><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Points System</h1>
          <p className="text-muted-foreground">Loyalty points for Shop #{shopId}</p>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Gift className="h-5 w-5" />Enable Points</CardTitle>
          <CardDescription>Set up a loyalty points system for your customers</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 md:grid-cols-2">
            <div className="space-y-2">
              <label className="text-sm font-medium">Points Name</label>
              <Input value={ptName} onChange={(e) => setPtName(e.target.value)} placeholder="e.g. Reward Points" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Points Symbol</label>
              <Input value={ptSymbol} onChange={(e) => setPtSymbol(e.target.value)} placeholder="e.g. RPT" />
            </div>
          </div>
          <TxButton onClick={handleEnable} txStatus={actions.txState.status} disabled={!ptName || !ptSymbol}>
            <Gift className="mr-2 h-4 w-4" />Enable Points
          </TxButton>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Settings className="h-5 w-5" />Points Configuration</CardTitle>
          <CardDescription>Adjust reward and exchange rates</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 md:grid-cols-2">
            <div className="space-y-2">
              <label className="text-sm font-medium">Reward Rate (basis points)</label>
              <Input type="number" value={rewardRate} onChange={(e) => setRewardRate(e.target.value)} min="0" />
              <p className="text-xs text-muted-foreground">{Number(rewardRate) / 100}% of order value as points</p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Exchange Rate (basis points)</label>
              <Input type="number" value={exchangeRate} onChange={(e) => setExchangeRate(e.target.value)} min="0" />
              <p className="text-xs text-muted-foreground">{Number(exchangeRate) / 100}% redemption value</p>
            </div>
          </div>
          <div className="flex items-center justify-between">
            <div>
              <label className="text-sm font-medium">Transferable</label>
              <p className="text-xs text-muted-foreground">Allow members to transfer points between each other</p>
            </div>
            <Switch checked={transferable} onCheckedChange={setTransferable} />
          </div>
          <TxButton onClick={handleUpdateConfig} txStatus={actions.txState.status}>
            Update Configuration
          </TxButton>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Send className="h-5 w-5" />Transfer Points</CardTitle>
          <CardDescription>Send points to a specific address</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 md:grid-cols-2">
            <div className="space-y-2">
              <label className="text-sm font-medium">Recipient</label>
              <Input value={transferTo} onChange={(e) => setTransferTo(e.target.value)} placeholder="5xxx..." />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Amount</label>
              <Input type="number" value={transferAmount} onChange={(e) => setTransferAmount(e.target.value)} placeholder="0" min="0" />
            </div>
          </div>
          <TxButton onClick={handleTransfer} txStatus={actions.txState.status} disabled={!transferTo || !transferAmount}>
            <Send className="mr-2 h-4 w-4" />Transfer
          </TxButton>
        </CardContent>
      </Card>

      <Card>
        <CardHeader><CardTitle className="text-destructive">Danger Zone</CardTitle></CardHeader>
        <CardContent>
          <Button variant="destructive" onClick={() => actions.disablePoints(shopId)}>
            <Trash2 className="mr-2 h-4 w-4" />Disable Points System
          </Button>
        </CardContent>
      </Card>

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
