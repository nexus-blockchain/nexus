"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useShops } from "@/hooks/useShop";
import { useMemberActions, usePendingMembers } from "@/hooks/useMember";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import { ArrowLeft, UserCheck, UserX, Users, RotateCcw, CheckCheck, XCircle, Trash2 } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function PendingMembersPage() {
  const { currentEntityId } = useEntityStore();
  const { shops } = useShops(currentEntityId);
  const primaryShop = shops.find((s) => s.isPrimary) || shops[0];
  const shopId = primaryShop?.id ?? null;
  const { pending, isLoading, refetch } = usePendingMembers(currentEntityId);
  const actions = useMemberActions();
  const tc = useTranslations("common");

  const [selectedAccounts, setSelectedAccounts] = useState<Set<string>>(new Set());
  const [maxClean, setMaxClean] = useState("10");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const toggleSelect = (account: string) => {
    setSelectedAccounts((prev) => {
      const next = new Set(prev);
      if (next.has(account)) next.delete(account); else next.add(account);
      return next;
    });
  };

  const selectAll = () => {
    if (selectedAccounts.size === pending.length) {
      setSelectedAccounts(new Set());
    } else {
      setSelectedAccounts(new Set(pending.map((p) => p.account)));
    }
  };

  const handleBatchApprove = async () => {
    if (!shopId || selectedAccounts.size === 0) return;
    await actions.batchApproveMembers(shopId, Array.from(selectedAccounts));
    setSelectedAccounts(new Set());
    refetch();
  };

  const handleBatchReject = async () => {
    if (!shopId || selectedAccounts.size === 0) return;
    await actions.batchRejectMembers(shopId, Array.from(selectedAccounts));
    setSelectedAccounts(new Set());
    refetch();
  };

  const handleCleanupExpired = async () => {
    if (currentEntityId === null) return;
    await actions.cleanupExpiredPending(currentEntityId, Number(maxClean || 10));
    refetch();
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/members"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">Pending Members</h1>
          <p className="text-muted-foreground">Review and approve membership applications</p>
        </div>
        <Button variant="outline" size="sm" onClick={refetch}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Pending Applications</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{pending.length}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Selected</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{selectedAccounts.size}</p></CardContent>
        </Card>
        <Card>
          <CardContent className="flex items-center gap-2 pt-6">
            <div className="space-y-1 flex-1">
              <label className="text-xs text-muted-foreground">Max cleanup</label>
              <Input type="number" value={maxClean} onChange={(e) => setMaxClean(e.target.value)} min="1" max="100" className="h-8" />
            </div>
            <TxButton
              variant="outline"
              size="sm"
              className="mt-5"
              onClick={handleCleanupExpired}
              txStatus={actions.txState.status}
            >
              <Trash2 className="mr-1 h-3 w-3" />Cleanup Expired
            </TxButton>
          </CardContent>
        </Card>
      </div>

      {pending.length > 0 && (
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={selectAll}>
            {selectedAccounts.size === pending.length ? "Deselect All" : "Select All"}
          </Button>
          {selectedAccounts.size > 0 && (
            <>
              <TxButton size="sm" onClick={handleBatchApprove} txStatus={actions.txState.status}>
                <CheckCheck className="mr-1 h-3 w-3" />Approve Selected ({selectedAccounts.size})
              </TxButton>
              <TxButton size="sm" variant="outline" onClick={handleBatchReject} txStatus={actions.txState.status}>
                <XCircle className="mr-1 h-3 w-3" />Reject Selected ({selectedAccounts.size})
              </TxButton>
            </>
          )}
        </div>
      )}

      {isLoading ? (
        <div className="flex justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
      ) : pending.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Users className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No Pending Applications</p>
            <p className="text-sm text-muted-foreground">All membership requests have been processed.</p>
          </CardContent>
        </Card>
      ) : (
        <div className="space-y-3">
          {pending.map((member) => (
            <Card key={member.account} className={selectedAccounts.has(member.account) ? "ring-2 ring-primary" : ""}>
              <CardContent className="flex items-center gap-4 p-4">
                <input
                  type="checkbox"
                  checked={selectedAccounts.has(member.account)}
                  onChange={() => toggleSelect(member.account)}
                  className="h-4 w-4 rounded"
                />
                <div className="flex-1 space-y-1">
                  <AddressDisplay address={member.account} />
                  <div className="flex items-center gap-3 text-sm text-muted-foreground">
                    {member.referrer && (
                      <span>Referred by: <span className="font-mono">{member.referrer.slice(0, 8)}...</span></span>
                    )}
                    <span>Block #{member.appliedAt}</span>
                  </div>
                </div>
                <div className="flex gap-2">
                  <TxButton
                    size="sm"
                    onClick={() => shopId && actions.approveMember(shopId, member.account)}
                    txStatus={actions.txState.status}
                  >
                    <UserCheck className="mr-1 h-3 w-3" />Approve
                  </TxButton>
                  <TxButton
                    variant="outline"
                    size="sm"
                    onClick={() => shopId && actions.rejectMember(shopId, member.account)}
                    txStatus={actions.txState.status}
                  >
                    <UserX className="mr-1 h-3 w-3" />Reject
                  </TxButton>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
