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
import { ArrowLeft, Plus, Trash2, ShieldCheck, Users } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

interface KycProvider {
  account: string;
  name: string;
  maxLevel: number;
  isActive: boolean;
}

export default function KycProvidersPage() {
  const { currentEntityId } = useEntityStore();
  const { submit, state: txState } = useTx();
  const tc = useTranslations("common");

  const [providers, setProviders] = useState<KycProvider[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [newAddr, setNewAddr] = useState("");
  const [newName, setNewName] = useState("");
  const [newMaxLevel, setNewMaxLevel] = useState("3");

  const fetchProviders = useCallback(async () => {
    if (currentEntityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityKyc.kycProviders.entries(currentEntityId);
      const results = entries.map(([key, val]: [{ args: [unknown, { toString: () => string }] }, { toJSON: () => Omit<KycProvider, "account"> }]) => ({
        account: key.args[1].toString(),
        ...val.toJSON(),
      }));
      setProviders(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [currentEntityId]);

  useEffect(() => { fetchProviders(); }, [fetchProviders]);

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const handleAdd = () => {
    if (!newAddr.trim() || !newName.trim()) return;
    submit("entityKyc", "addProvider", [currentEntityId, newAddr, newName, Number(newMaxLevel)]);
    setNewAddr(""); setNewName(""); setNewMaxLevel("3");
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/kyc"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">KYC Providers</h1>
          <p className="text-muted-foreground">Manage trusted identity verification providers</p>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Plus className="h-5 w-5" />Add Provider</CardTitle>
          <CardDescription>Register a trusted KYC verification provider</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 md:grid-cols-3">
            <div className="space-y-2">
              <label className="text-sm font-medium">Provider Address</label>
              <Input value={newAddr} onChange={(e) => setNewAddr(e.target.value)} placeholder="5xxx..." />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Provider Name</label>
              <Input value={newName} onChange={(e) => setNewName(e.target.value)} placeholder="e.g. Jumio" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Max Level</label>
              <Input type="number" value={newMaxLevel} onChange={(e) => setNewMaxLevel(e.target.value)} min="1" max="5" />
            </div>
          </div>
          <TxButton onClick={handleAdd} txStatus={txState.status} disabled={!newAddr.trim() || !newName.trim()}>
            <Plus className="mr-2 h-4 w-4" />Add Provider
          </TxButton>
        </CardContent>
      </Card>

      {isLoading ? (
        <div className="flex justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
      ) : providers.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Users className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No KYC Providers</p>
            <p className="text-sm text-muted-foreground">Register providers to enable KYC verification.</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader><CardTitle className="flex items-center gap-2"><ShieldCheck className="h-5 w-5" />Registered Providers</CardTitle></CardHeader>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Address</TableHead>
                <TableHead>Name</TableHead>
                <TableHead>Max Level</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {providers.map((provider) => (
                <TableRow key={provider.account}>
                  <TableCell><AddressDisplay address={provider.account} /></TableCell>
                  <TableCell className="font-medium">{provider.name}</TableCell>
                  <TableCell>{provider.maxLevel}</TableCell>
                  <TableCell>
                    <Badge variant={provider.isActive ? "default" : "secondary"}>
                      {provider.isActive ? "Active" : "Inactive"}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-right">
                    <Button variant="ghost" size="icon" onClick={() => submit("entityKyc", "removeProvider", [currentEntityId, provider.account])}>
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
