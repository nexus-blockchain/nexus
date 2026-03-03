"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useEntity, useEntityActions } from "@/hooks/useEntity";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { TxButton } from "@/components/shared/TxButton";
import { Progress } from "@/components/ui/progress";
import { formatBalance } from "@/lib/utils";
import { Wallet, ArrowUpFromLine, AlertTriangle, CheckCircle } from "lucide-react";

export default function EntityFundPage() {
  const { currentEntityId } = useEntityStore();
  const { data: entity, isLoading } = useEntity(currentEntityId);
  const actions = useEntityActions(currentEntityId || 0);
  const [topUpAmount, setTopUpAmount] = useState("");

  if (!currentEntityId || isLoading || !entity) {
    return <div className="flex h-full items-center justify-center"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>;
  }

  const handleTopUp = () => {
    if (topUpAmount && Number(topUpAmount) > 0) {
      actions.topUpFund(BigInt(topUpAmount));
      setTopUpAmount("");
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Operating Fund</h1>
        <p className="text-muted-foreground">Manage your entity&apos;s operating fund balance</p>
      </div>

      <div className="grid gap-6 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Wallet className="h-5 w-5" />Fund Balance</CardTitle>
            <CardDescription>Current operating fund status</CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="text-center">
              <p className="text-4xl font-bold">-- NEX</p>
              <p className="mt-1 text-sm text-muted-foreground">Available Balance</p>
            </div>
            <Progress value={75} className="h-3" />
            <div className="grid grid-cols-3 gap-4 text-center text-sm">
              <div>
                <p className="font-medium text-red-600">100 NEX</p>
                <p className="text-xs text-muted-foreground">Minimum</p>
              </div>
              <div>
                <p className="font-medium text-yellow-600">500 NEX</p>
                <p className="text-xs text-muted-foreground">Warning</p>
              </div>
              <div>
                <p className="font-medium text-green-600">1000+ NEX</p>
                <p className="text-xs text-muted-foreground">Healthy</p>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><ArrowUpFromLine className="h-5 w-5" />Top Up Fund</CardTitle>
            <CardDescription>Add funds to your entity&apos;s operating account</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Amount (NEX)</label>
              <Input type="number" value={topUpAmount} onChange={(e) => setTopUpAmount(e.target.value)} placeholder="Enter amount" min="0" step="0.01" />
            </div>
            <TxButton onClick={handleTopUp} txStatus={actions.txState.status} className="w-full">
              <ArrowUpFromLine className="mr-2 h-4 w-4" />Top Up Fund
            </TxButton>
            {actions.txState.status === "finalized" && (
              <div className="flex items-center gap-2 text-sm text-green-600">
                <CheckCircle className="h-4 w-4" />Top up successful!
              </div>
            )}
            {actions.txState.status === "error" && (
              <div className="flex items-center gap-2 text-sm text-destructive">
                <AlertTriangle className="h-4 w-4" />{actions.txState.error}
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Fund History</CardTitle>
          <CardDescription>Recent operating fund transactions</CardDescription>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">Transaction history will be populated from chain events.</p>
        </CardContent>
      </Card>
    </div>
  );
}
