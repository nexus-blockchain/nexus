"use client";

import { use, useState, useEffect, useCallback } from "react";
import { useEntityStore } from "@/stores/entity";
import { getApi } from "@/hooks/useApi";
import { useTx } from "@/hooks/useTx";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { TxButton } from "@/components/shared/TxButton";
import { Separator } from "@/components/ui/separator";
import { formatBalance, formatNumber } from "@/lib/utils";
import { ArrowLeft, Rocket, Users, ShoppingCart, Play, Square, Download, Coins } from "lucide-react";
import Link from "next/link";
import type { SaleRound } from "@/lib/types";

export default function SaleRoundDetailPage({ params }: { params: Promise<{ roundId: string }> }) {
  const { roundId: roundIdStr } = use(params);
  const roundId = Number(roundIdStr);
  const { currentEntityId } = useEntityStore();
  const { submit, state: txState } = useTx();

  const [round, setRound] = useState<SaleRound | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [subscribeAmount, setSubscribeAmount] = useState("");

  const fetchRound = useCallback(async () => {
    if (currentEntityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).entityTokensale.saleRounds(currentEntityId, roundId);
      if (!raw.isNone) setRound(raw.toJSON() as unknown as SaleRound);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [currentEntityId, roundId]);

  useEffect(() => { fetchRound(); }, [fetchRound]);

  if (isLoading) {
    return <div className="flex h-full items-center justify-center"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>;
  }

  if (!round) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4">
        <Rocket className="h-16 w-16 text-muted-foreground/50" />
        <p className="text-muted-foreground">Sale round not found</p>
        <Button variant="outline" asChild><Link href="/tokensale">Back to Token Sale</Link></Button>
      </div>
    );
  }

  const sold = BigInt(round.totalAmount) - BigInt(round.remaining);
  const soldPct = Number(round.totalAmount) > 0 ? (Number(sold) / Number(round.totalAmount)) * 100 : 0;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/tokensale"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <div className="flex items-center gap-3">
            <h1 className="text-3xl font-bold tracking-tight">Round #{roundId}</h1>
            <StatusBadge status={round.status} />
          </div>
          <p className="text-muted-foreground">Token Sale Round Details</p>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Total Supply</CardTitle>
            <Coins className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent><p className="text-2xl font-bold">{formatBalance(BigInt(round.totalAmount))}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Sold</CardTitle>
            <ShoppingCart className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{formatBalance(sold)}</p>
            <Progress value={soldPct} className="mt-2 h-2" />
            <p className="text-xs text-muted-foreground mt-1">{soldPct.toFixed(1)}% sold</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Subscribers</CardTitle>
            <Users className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent><p className="text-2xl font-bold">{formatNumber(round.subscriberCount)}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Price</CardTitle>
            <Coins className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent><p className="text-2xl font-bold">{formatBalance(BigInt(round.price))} NEX</p></CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader><CardTitle>Round Details</CardTitle></CardHeader>
        <CardContent className="space-y-3">
          <div className="flex justify-between"><span className="text-sm text-muted-foreground">Remaining</span><span className="text-sm font-mono">{formatBalance(BigInt(round.remaining))}</span></div>
          <Separator />
          <div className="flex justify-between"><span className="text-sm text-muted-foreground">Total Raised</span><span className="text-sm font-mono">{formatBalance(BigInt(round.totalRaised))} NEX</span></div>
          <Separator />
          <div className="flex justify-between"><span className="text-sm text-muted-foreground">Mode</span><Badge variant="outline">{round.mode}</Badge></div>
          <Separator />
          <div className="flex justify-between"><span className="text-sm text-muted-foreground">Start Block</span><span className="text-sm">#{round.startBlock || "Immediate"}</span></div>
          <Separator />
          <div className="flex justify-between"><span className="text-sm text-muted-foreground">End Block</span><span className="text-sm">#{round.endBlock || "No end"}</span></div>
          {round.vestingConfig && (
            <>
              <Separator />
              <div className="flex justify-between"><span className="text-sm text-muted-foreground">Vesting</span><Badge variant="outline">Enabled</Badge></div>
              <div className="flex justify-between"><span className="text-sm text-muted-foreground">Initial Unlock</span><span className="text-sm">{round.vestingConfig.initialUnlockPct}%</span></div>
              <div className="flex justify-between"><span className="text-sm text-muted-foreground">Vesting Blocks</span><span className="text-sm">{round.vestingConfig.vestingBlocks} blocks</span></div>
            </>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Actions</CardTitle>
          <CardDescription>Available actions based on round status</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {round.status === "Pending" && (
            <TxButton onClick={() => currentEntityId !== null && submit("entityTokensale", "startSale", [currentEntityId, roundId])} txStatus={txState.status}>
              <Play className="mr-2 h-4 w-4" />Start Sale
            </TxButton>
          )}

          {round.status === "Active" && (
            <>
              <div className="flex items-end gap-4">
                <div className="flex-1 space-y-2">
                  <label className="text-sm font-medium">Subscribe Amount</label>
                  <Input type="number" value={subscribeAmount} onChange={(e) => setSubscribeAmount(e.target.value)} placeholder="Token amount to buy" min="0" />
                </div>
                <TxButton
                  onClick={() => currentEntityId !== null && subscribeAmount && submit("entityTokensale", "subscribe", [currentEntityId, roundId, BigInt(subscribeAmount)])}
                  txStatus={txState.status}
                  disabled={!subscribeAmount}
                >
                  <ShoppingCart className="mr-2 h-4 w-4" />Subscribe
                </TxButton>
              </div>
              <Separator />
              <TxButton
                variant="outline"
                onClick={() => currentEntityId !== null && submit("entityTokensale", "endSale", [currentEntityId, roundId])}
                txStatus={txState.status}
              >
                <Square className="mr-2 h-4 w-4" />End Sale
              </TxButton>
            </>
          )}

          {round.status === "Ended" && (
            <div className="flex gap-4">
              <TxButton
                onClick={() => currentEntityId !== null && submit("entityTokensale", "claimTokens", [currentEntityId, roundId])}
                txStatus={txState.status}
              >
                <Download className="mr-2 h-4 w-4" />Claim Tokens
              </TxButton>
              <TxButton
                variant="outline"
                onClick={() => currentEntityId !== null && submit("entityTokensale", "withdrawFunds", [currentEntityId, roundId])}
                txStatus={txState.status}
              >
                <Download className="mr-2 h-4 w-4" />Withdraw Funds
              </TxButton>
            </div>
          )}

          {["Completed", "Cancelled"].includes(round.status) && (
            <p className="text-sm text-muted-foreground">This round is finalized. No actions available.</p>
          )}
        </CardContent>
      </Card>

      {txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {txState.status === "error" && <p className="text-sm text-destructive">{txState.error}</p>}
    </div>
  );
}
