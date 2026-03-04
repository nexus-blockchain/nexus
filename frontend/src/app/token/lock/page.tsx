"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useTokenActions } from "@/hooks/useToken";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { TxButton } from "@/components/shared/TxButton";
import { ArrowLeft, Lock, Unlock } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function TokenLockPage() {
  const { currentEntityId } = useEntityStore();
  const actions = useTokenActions();
  const tc = useTranslations("common");

  const [lockUser, setLockUser] = useState("");
  const [lockAmount, setLockAmount] = useState("");
  const [unlockAt, setUnlockAt] = useState("");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const handleLock = () => {
    if (!lockUser || !lockAmount || !unlockAt) return;
    actions.lockTokens(currentEntityId, lockUser, BigInt(lockAmount), Number(unlockAt));
  };

  const handleUnlock = () => {
    actions.unlockTokens(currentEntityId);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/token/config"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Token Lock Management</h1>
          <p className="text-muted-foreground">Lock and unlock entity tokens with vesting schedules</p>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Lock className="h-5 w-5" />Lock Tokens</CardTitle>
          <CardDescription>Lock tokens for a user until a specified block number</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">User Address</label>
            <Input value={lockUser} onChange={(e) => setLockUser(e.target.value)} placeholder="5xxx..." />
          </div>
          <div className="grid gap-4 md:grid-cols-2">
            <div className="space-y-2">
              <label className="text-sm font-medium">Amount</label>
              <Input type="number" value={lockAmount} onChange={(e) => setLockAmount(e.target.value)} placeholder="0" min="0" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Unlock At (block number)</label>
              <Input type="number" value={unlockAt} onChange={(e) => setUnlockAt(e.target.value)} placeholder="Block #" min="0" />
            </div>
          </div>
          <TxButton onClick={handleLock} txStatus={actions.txState.status} disabled={!lockUser || !lockAmount || !unlockAt}>
            <Lock className="mr-2 h-4 w-4" />Lock Tokens
          </TxButton>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Unlock className="h-5 w-5" />Unlock Tokens</CardTitle>
          <CardDescription>Release all expired token locks for the current entity</CardDescription>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground mb-4">
            This will release all token locks that have reached their unlock block. Locks that have not yet reached their unlock time will remain locked.
          </p>
          <TxButton onClick={handleUnlock} txStatus={actions.txState.status}>
            <Unlock className="mr-2 h-4 w-4" />Unlock Expired Tokens
          </TxButton>
        </CardContent>
      </Card>

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
