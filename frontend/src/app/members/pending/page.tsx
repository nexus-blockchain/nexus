"use client";

import { useState, useEffect, useCallback } from "react";
import { useEntityStore } from "@/stores/entity";
import { useShops } from "@/hooks/useShop";
import { useMemberActions } from "@/hooks/useMember";
import { getApi } from "@/hooks/useApi";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import { ArrowLeft, UserCheck, UserX, Users, RotateCcw } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

interface PendingMember {
  account: string;
  referrer: string | null;
  appliedAt: number;
}

export default function PendingMembersPage() {
  const { currentEntityId } = useEntityStore();
  const { shops } = useShops(currentEntityId);
  const primaryShop = shops.find((s) => s.isPrimary) || shops[0];
  const shopId = primaryShop?.id ?? null;
  const actions = useMemberActions();
  const tc = useTranslations("common");

  const [pending, setPending] = useState<PendingMember[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetchPending = useCallback(async () => {
    if (shopId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityMember.pendingMembers.entries(shopId);
      const results = entries.map(([key, val]: [{ args: [unknown, { toString: () => string }] }, { toJSON: () => Omit<PendingMember, "account"> }]) => ({
        account: key.args[1].toString(),
        ...val.toJSON(),
      }));
      setPending(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [shopId]);

  useEffect(() => { fetchPending(); }, [fetchPending]);

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

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
        <Button variant="outline" size="sm" onClick={fetchPending}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      <Card>
        <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Pending Applications</CardTitle></CardHeader>
        <CardContent><p className="text-2xl font-bold">{pending.length}</p></CardContent>
      </Card>

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
            <Card key={member.account}>
              <CardContent className="flex items-center gap-4 p-4">
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
