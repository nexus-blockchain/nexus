"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useWalletStore } from "@/stores/wallet";
import { useWithdrawable, useCommissionRecords, useCommissionActions, useCommissionConfig, useWithdrawalConfig } from "@/hooks/useCommission";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { Separator } from "@/components/ui/separator";
import { TxButton } from "@/components/shared/TxButton";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { formatBalance, basisPointsToPercent } from "@/lib/utils";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { ArrowLeft, Wallet, Download, RotateCcw, Clock, Shield, Percent } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function CommissionWithdrawPage() {
  const { currentEntityId } = useEntityStore();
  const address = useWalletStore((s) => s.address);
  const { balance, isLoading: balLoading, refetch: refetchBal } = useWithdrawable(currentEntityId, address);
  const { records, isLoading: recLoading, refetch: refetchRec } = useCommissionRecords(currentEntityId);
  const { config } = useCommissionConfig(currentEntityId);
  const { config: withdrawalConfig, tokenConfig: tokenWithdrawalConfig } = useWithdrawalConfig(currentEntityId);
  const actions = useCommissionActions();
  const tc = useTranslations("common");

  const [nexAmount, setNexAmount] = useState("");
  const [nexRepurchaseRate, setNexRepurchaseRate] = useState("");
  const [tokenAmount, setTokenAmount] = useState("");
  const [tokenRepurchaseRate, setTokenRepurchaseRate] = useState("");
  const [useFullWithdraw, setUseFullWithdraw] = useState(true);

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const handleWithdrawNex = () => {
    const amount = useFullWithdraw ? null : BigInt(nexAmount || 0);
    const repurchase = nexRepurchaseRate ? Number(nexRepurchaseRate) : null;
    actions.withdrawCommission(currentEntityId, amount, repurchase, null);
  };

  const handleWithdrawToken = () => {
    const amount = useFullWithdraw ? null : BigInt(tokenAmount || 0);
    const repurchase = tokenRepurchaseRate ? Number(tokenRepurchaseRate) : null;
    actions.withdrawTokenCommission(currentEntityId, amount, repurchase, null);
  };

  const getWithdrawalModeLabel = (mode: unknown): string => {
    if (typeof mode === "string") return mode;
    if (typeof mode === "object" && mode !== null) {
      if ("FixedRate" in (mode as Record<string, unknown>)) return `Fixed Rate (${(mode as { FixedRate: { repurchaseRate: number } }).FixedRate.repurchaseRate} bps)`;
      if ("MemberChoice" in (mode as Record<string, unknown>)) return `Member Choice (min ${(mode as { MemberChoice: { minRepurchaseRate: number } }).MemberChoice.minRepurchaseRate} bps)`;
      if ("LevelBased" in (mode as Record<string, unknown>)) return "Level Based";
    }
    return "Full Withdrawal";
  };

  const myRecords = records.filter((r) => r.beneficiary === address);

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/commission"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">Commission Withdrawal</h1>
          <p className="text-muted-foreground">Withdraw your earned commission rewards</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => { refetchBal(); refetchRec(); }}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {config && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Shield className="h-5 w-5" />Withdrawal Settings</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
              <div>
                <p className="text-xs text-muted-foreground">NEX Cooldown</p>
                <p className="text-sm font-medium flex items-center gap-1"><Clock className="h-3 w-3" />{config.withdrawalCooldown} blocks</p>
              </div>
              <div>
                <p className="text-xs text-muted-foreground">Token Cooldown</p>
                <p className="text-sm font-medium flex items-center gap-1"><Clock className="h-3 w-3" />{config.tokenWithdrawalCooldown} blocks</p>
              </div>
              {withdrawalConfig && (
                <>
                  <div>
                    <p className="text-xs text-muted-foreground">NEX Withdrawal Mode</p>
                    <Badge variant="outline">{getWithdrawalModeLabel(withdrawalConfig.mode)}</Badge>
                  </div>
                  <div>
                    <p className="text-xs text-muted-foreground">Default Repurchase</p>
                    <p className="text-sm font-medium">{basisPointsToPercent(withdrawalConfig.defaultTier.repurchaseRate)}</p>
                  </div>
                </>
              )}
            </div>
          </CardContent>
        </Card>
      )}

      <div className="flex items-center gap-3 rounded-lg border p-3">
        <Switch checked={useFullWithdraw} onCheckedChange={setUseFullWithdraw} />
        <span className="text-sm font-medium">{useFullWithdraw ? "Withdraw All" : "Partial Withdrawal"}</span>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Wallet className="h-5 w-5" />NEX Commission</CardTitle>
            <CardDescription>Withdrawable NEX commission balance</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <p className="text-3xl font-bold">{balLoading ? "..." : formatBalance(balance.nex)} NEX</p>

            {!useFullWithdraw && (
              <div className="space-y-2">
                <div className="space-y-1">
                  <label className="text-xs font-medium">Amount (smallest unit)</label>
                  <Input value={nexAmount} onChange={(e) => setNexAmount(e.target.value)} placeholder="Leave empty for full" />
                </div>
                <div className="space-y-1">
                  <label className="text-xs font-medium">Repurchase Rate (bps, optional)</label>
                  <Input type="number" value={nexRepurchaseRate} onChange={(e) => setNexRepurchaseRate(e.target.value)} placeholder="Optional" min="0" max="10000" />
                  {nexRepurchaseRate && <p className="text-xs text-muted-foreground">{basisPointsToPercent(Number(nexRepurchaseRate))} repurchased</p>}
                </div>
              </div>
            )}

            <TxButton onClick={handleWithdrawNex} txStatus={actions.txState.status} disabled={balance.nex <= BigInt(0)}>
              <Download className="mr-2 h-4 w-4" />Withdraw NEX
            </TxButton>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Wallet className="h-5 w-5" />Token Commission</CardTitle>
            <CardDescription>Withdrawable entity token commission</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <p className="text-3xl font-bold">{balLoading ? "..." : formatBalance(balance.token)} Tokens</p>

            {!useFullWithdraw && (
              <div className="space-y-2">
                <div className="space-y-1">
                  <label className="text-xs font-medium">Amount (smallest unit)</label>
                  <Input value={tokenAmount} onChange={(e) => setTokenAmount(e.target.value)} placeholder="Leave empty for full" />
                </div>
                <div className="space-y-1">
                  <label className="text-xs font-medium">Repurchase Rate (bps, optional)</label>
                  <Input type="number" value={tokenRepurchaseRate} onChange={(e) => setTokenRepurchaseRate(e.target.value)} placeholder="Optional" min="0" max="10000" />
                </div>
              </div>
            )}

            <TxButton onClick={handleWithdrawToken} txStatus={actions.txState.status} disabled={balance.token <= BigInt(0)}>
              <Download className="mr-2 h-4 w-4" />Withdraw Tokens
            </TxButton>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Your Commission History</CardTitle>
          <CardDescription>{myRecords.length} records for your address</CardDescription>
        </CardHeader>
        {recLoading ? (
          <CardContent><div className="flex justify-center py-8"><div className="h-6 w-6 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div></CardContent>
        ) : myRecords.length === 0 ? (
          <CardContent><p className="text-sm text-muted-foreground py-4">No commission records for your address.</p></CardContent>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>ID</TableHead>
                <TableHead>Source</TableHead>
                <TableHead>Order</TableHead>
                <TableHead className="text-right">Amount</TableHead>
                <TableHead>Asset</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Block</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {myRecords.slice(0, 50).map((r) => (
                <TableRow key={r.id}>
                  <TableCell className="font-mono">#{r.id}</TableCell>
                  <TableCell><Badge variant="outline">{r.source}</Badge></TableCell>
                  <TableCell className="font-mono">#{r.orderId}</TableCell>
                  <TableCell className="text-right font-mono">{formatBalance(BigInt(r.amount || 0))}</TableCell>
                  <TableCell><Badge variant="secondary">NEX</Badge></TableCell>
                  <TableCell><Badge variant={r.status === "Pending" ? "default" : "secondary"}>{r.status}</Badge></TableCell>
                  <TableCell className="text-muted-foreground">#{r.createdAt}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </Card>

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Withdrawal successful!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
