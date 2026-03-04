"use client";

import { useEntityStore } from "@/stores/entity";
import { useWalletStore } from "@/stores/wallet";
import { useWithdrawable, useCommissionRecords, useCommissionActions } from "@/hooks/useCommission";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { TxButton } from "@/components/shared/TxButton";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { formatBalance } from "@/lib/utils";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { ArrowLeft, Wallet, Download, RotateCcw } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function CommissionWithdrawPage() {
  const { currentEntityId } = useEntityStore();
  const address = useWalletStore((s) => s.address);
  const { balance, isLoading: balLoading, refetch: refetchBal } = useWithdrawable(currentEntityId, address);
  const { records, isLoading: recLoading, refetch: refetchRec } = useCommissionRecords(currentEntityId);
  const actions = useCommissionActions();
  const tc = useTranslations("common");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const handleWithdrawNex = () => actions.withdrawCommission(currentEntityId, "Nex");
  const handleWithdrawToken = () => actions.withdrawCommission(currentEntityId, "Token");

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

      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Wallet className="h-5 w-5" />NEX Balance</CardTitle>
            <CardDescription>Withdrawable NEX commission</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <p className="text-3xl font-bold">{balLoading ? "..." : formatBalance(balance.nex)} NEX</p>
            <TxButton onClick={handleWithdrawNex} txStatus={actions.txState.status} disabled={balance.nex <= BigInt(0)}>
              <Download className="mr-2 h-4 w-4" />Withdraw NEX
            </TxButton>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Wallet className="h-5 w-5" />Token Balance</CardTitle>
            <CardDescription>Withdrawable entity token commission</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <p className="text-3xl font-bold">{balLoading ? "..." : formatBalance(balance.token)} Tokens</p>
            <TxButton onClick={handleWithdrawToken} txStatus={actions.txState.status} disabled={balance.token <= BigInt(0)}>
              <Download className="mr-2 h-4 w-4" />Withdraw Tokens
            </TxButton>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Commission History</CardTitle>
          <CardDescription>Recent commission records for this entity</CardDescription>
        </CardHeader>
        {recLoading ? (
          <CardContent><div className="flex justify-center py-8"><div className="h-6 w-6 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div></CardContent>
        ) : records.length === 0 ? (
          <CardContent><p className="text-sm text-muted-foreground py-4">No commission records found.</p></CardContent>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>ID</TableHead>
                <TableHead>Beneficiary</TableHead>
                <TableHead>Source</TableHead>
                <TableHead className="text-right">Amount</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Block</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {records.slice(0, 50).map((r) => (
                <TableRow key={r.id}>
                  <TableCell className="font-mono">#{r.id}</TableCell>
                  <TableCell><AddressDisplay address={r.beneficiary} chars={4} /></TableCell>
                  <TableCell><Badge variant="outline">{r.source}</Badge></TableCell>
                  <TableCell className="text-right font-mono">{formatBalance(r.amount)}</TableCell>
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
