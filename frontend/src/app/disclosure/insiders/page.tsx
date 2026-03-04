"use client";

import { useState, useEffect, useCallback } from "react";
import { useEntityStore } from "@/stores/entity";
import { getApi } from "@/hooks/useApi";
import { useTx } from "@/hooks/useTx";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { ArrowLeft, UserPlus, Trash2, ShieldAlert, Users, RotateCcw } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

interface InsiderEntry {
  account: string;
  role: string;
  addedAt: number;
  isActive: boolean;
}

export default function InsidersPage() {
  const { currentEntityId } = useEntityStore();
  const { submit, state: txState } = useTx();
  const tc = useTranslations("common");

  const [insiders, setInsiders] = useState<InsiderEntry[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [newAddr, setNewAddr] = useState("");
  const [newRole, setNewRole] = useState("");

  const fetchInsiders = useCallback(async () => {
    if (currentEntityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityDisclosure.insiders.entries(currentEntityId);
      const results = entries.map(([key, val]: [{ args: [unknown, { toString: () => string }] }, { toJSON: () => Omit<InsiderEntry, "account"> }]) => ({
        account: key.args[1].toString(),
        ...val.toJSON(),
      }));
      setInsiders(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [currentEntityId]);

  useEffect(() => { fetchInsiders(); }, [fetchInsiders]);

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const handleAdd = () => {
    if (!newAddr.trim() || !newRole.trim()) return;
    submit("entityDisclosure", "addInsider", [currentEntityId, newAddr, newRole]);
    setNewAddr("");
    setNewRole("");
  };

  const handleRemove = (account: string) => {
    submit("entityDisclosure", "removeInsider", [currentEntityId, account]);
  };

  const activeCount = insiders.filter((i) => i.isActive).length;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/disclosure"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">Insider Management</h1>
          <p className="text-muted-foreground">Manage insiders and blackout periods</p>
        </div>
        <Button variant="outline" size="sm" onClick={fetchInsiders}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Insiders</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{insiders.length}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Active</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{activeCount}</p></CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><UserPlus className="h-5 w-5" />Add Insider</CardTitle>
          <CardDescription>Register an insider with their role for disclosure compliance</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 md:grid-cols-2">
            <div className="space-y-2">
              <label className="text-sm font-medium">Address</label>
              <Input value={newAddr} onChange={(e) => setNewAddr(e.target.value)} placeholder="5xxx..." />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Role</label>
              <Input value={newRole} onChange={(e) => setNewRole(e.target.value)} placeholder="e.g. Director, Officer, Employee" />
            </div>
          </div>
          <TxButton onClick={handleAdd} txStatus={txState.status} disabled={!newAddr.trim() || !newRole.trim()}>
            <UserPlus className="mr-2 h-4 w-4" />Add Insider
          </TxButton>
        </CardContent>
      </Card>

      {isLoading ? (
        <div className="flex justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
      ) : insiders.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Users className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No Insiders Registered</p>
            <p className="text-sm text-muted-foreground">Add insiders above for compliance tracking.</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader><CardTitle className="flex items-center gap-2"><ShieldAlert className="h-5 w-5" />Registered Insiders</CardTitle></CardHeader>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Address</TableHead>
                <TableHead>Role</TableHead>
                <TableHead>Added</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {insiders.map((insider) => (
                <TableRow key={insider.account}>
                  <TableCell><AddressDisplay address={insider.account} /></TableCell>
                  <TableCell>{insider.role}</TableCell>
                  <TableCell className="text-muted-foreground">Block #{insider.addedAt}</TableCell>
                  <TableCell>
                    <Badge variant={insider.isActive ? "default" : "secondary"}>
                      {insider.isActive ? "Active" : "Inactive"}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-right">
                    <Button variant="ghost" size="icon" onClick={() => handleRemove(insider.account)}>
                      <Trash2 className="h-4 w-4 text-destructive" />
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}

      {txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {txState.status === "error" && <p className="text-sm text-destructive">{txState.error}</p>}
    </div>
  );
}
