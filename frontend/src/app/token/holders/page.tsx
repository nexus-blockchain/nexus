"use client";

import { useState, useEffect, useCallback } from "react";
import { useEntityStore } from "@/stores/entity";
import { getApi } from "@/hooks/useApi";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { formatBalance } from "@/lib/utils";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { ArrowLeft, Users, Search, RotateCcw } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

interface HolderEntry {
  account: string;
  balance: bigint;
}

export default function TokenHoldersPage() {
  const { currentEntityId } = useEntityStore();
  const tc = useTranslations("common");
  const [holders, setHolders] = useState<HolderEntry[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [search, setSearch] = useState("");

  const fetchHolders = useCallback(async () => {
    if (currentEntityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityToken.tokenBalances.entries(currentEntityId);
      const results: HolderEntry[] = entries.map(
        ([key, val]: [{ args: [unknown, { toString: () => string }] }, { toJSON: () => string }]) => ({
          account: key.args[1].toString(),
          balance: BigInt(val.toJSON() || 0),
        })
      );
      results.sort((a, b) => (b.balance > a.balance ? 1 : b.balance < a.balance ? -1 : 0));
      setHolders(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [currentEntityId]);

  useEffect(() => { fetchHolders(); }, [fetchHolders]);

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const filtered = search
    ? holders.filter((h) => h.account.toLowerCase().includes(search.toLowerCase()))
    : holders;

  const totalSupply = holders.reduce((sum, h) => sum + h.balance, BigInt(0));

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/token/config"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">Token Holders</h1>
          <p className="text-muted-foreground">All token holders for the current entity</p>
        </div>
        <Button variant="outline" size="sm" onClick={fetchHolders}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Holders</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{holders.length}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Supply</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{formatBalance(totalSupply)}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Top Holder</CardTitle></CardHeader>
          <CardContent>
            {holders.length > 0 ? (
              <p className="text-sm font-mono">{holders[0].account.slice(0, 12)}... ({formatBalance(holders[0].balance)})</p>
            ) : (
              <p className="text-sm text-muted-foreground">—</p>
            )}
          </CardContent>
        </Card>
      </div>

      <div className="relative">
        <Search className="absolute left-3 top-3 h-4 w-4 text-muted-foreground" />
        <Input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search by address..."
          className="pl-10"
        />
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
      ) : filtered.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Users className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No Holders Found</p>
            <p className="text-sm text-muted-foreground">{search ? "No addresses match your search" : "No tokens have been distributed yet"}</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-12">#</TableHead>
                <TableHead>Address</TableHead>
                <TableHead className="text-right">Balance</TableHead>
                <TableHead className="text-right">Share</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {filtered.map((holder, i) => (
                <TableRow key={holder.account}>
                  <TableCell className="font-mono text-muted-foreground">{i + 1}</TableCell>
                  <TableCell><AddressDisplay address={holder.account} /></TableCell>
                  <TableCell className="text-right font-mono">{formatBalance(holder.balance)}</TableCell>
                  <TableCell className="text-right text-muted-foreground">
                    {totalSupply > 0 ? ((Number(holder.balance) / Number(totalSupply)) * 100).toFixed(2) : "0.00"}%
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </div>
  );
}
