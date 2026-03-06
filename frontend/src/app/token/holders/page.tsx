"use client";

import { useState, useEffect, useCallback, useMemo } from "react";
import { useEntityStore } from "@/stores/entity";
import { getApi } from "@/hooks/useApi";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { formatBalance } from "@/lib/utils";
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from "@/components/ui/table";
import { Users, Search, RotateCcw, BarChart3, TrendingUp } from "lucide-react";
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
      const entries = await (api.query as any).entityToken.tokenBalances.entries(
        currentEntityId
      );
      const results: HolderEntry[] = entries.map(
        ([key, val]: [
          { args: [unknown, { toString: () => string }] },
          { toJSON: () => string },
        ]) => ({
          account: key.args[1].toString(),
          balance: BigInt(val.toJSON() || 0),
        })
      );
      results.sort((a, b) =>
        b.balance > a.balance ? 1 : b.balance < a.balance ? -1 : 0
      );
      setHolders(results);
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [currentEntityId]);

  useEffect(() => {
    fetchHolders();
  }, [fetchHolders]);

  const totalSupply = useMemo(
    () => holders.reduce((sum, h) => sum + h.balance, BigInt(0)),
    [holders]
  );

  const shareOf = (balance: bigint) =>
    totalSupply > BigInt(0)
      ? (Number(balance) / Number(totalSupply)) * 100
      : 0;

  const concentration = useMemo(() => {
    const sorted = [...holders];
    const top1 = sorted.length >= 1 ? shareOf(sorted[0].balance) : 0;
    const top5 =
      sorted.length >= 1
        ? sorted.slice(0, 5).reduce((s, h) => s + shareOf(h.balance), 0)
        : 0;
    const top10 =
      sorted.length >= 1
        ? sorted.slice(0, 10).reduce((s, h) => s + shareOf(h.balance), 0)
        : 0;
    return { top1, top5, top10 };
  }, [holders, totalSupply]);

  const top10 = useMemo(() => holders.slice(0, 10), [holders]);

  const maxShare = useMemo(
    () =>
      top10.length > 0 ? Math.max(...top10.map((h) => shareOf(h.balance))) : 1,
    [top10, totalSupply]
  );

  const filtered = search
    ? holders.filter((h) =>
        h.account.toLowerCase().includes(search.toLowerCase())
      )
    : holders;

  if (!currentEntityId) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        {tc("selectEntity")}
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Token Holders</h1>
          <p className="text-muted-foreground">
            Distribution analysis and holder directory
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={fetchHolders}>
          <RotateCcw className="mr-2 h-3 w-3" />
          Refresh
        </Button>
      </div>

      {/* Stats */}
      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium flex items-center gap-2">
              <Users className="h-4 w-4 text-muted-foreground" />
              Total Holders
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{holders.length}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Total Supply</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{formatBalance(totalSupply)}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Top Holder</CardTitle>
          </CardHeader>
          <CardContent>
            {holders.length > 0 ? (
              <div>
                <p className="text-sm font-mono truncate">
                  {holders[0].account.slice(0, 16)}...
                </p>
                <p className="text-lg font-bold">
                  {formatBalance(holders[0].balance)}{" "}
                  <span className="text-sm text-muted-foreground font-normal">
                    ({shareOf(holders[0].balance).toFixed(2)}%)
                  </span>
                </p>
              </div>
            ) : (
              <p className="text-sm text-muted-foreground">—</p>
            )}
          </CardContent>
        </Card>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
        </div>
      ) : holders.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Users className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No Holders Found</p>
            <p className="text-sm text-muted-foreground">
              No tokens have been distributed yet
            </p>
          </CardContent>
        </Card>
      ) : (
        <>
          {/* Distribution Chart */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <BarChart3 className="h-5 w-5" />
                Top 10 Distribution
              </CardTitle>
              <CardDescription>
                Relative share of token holdings
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              {top10.map((holder, i) => {
                const share = shareOf(holder.balance);
                const barWidth = maxShare > 0 ? (share / maxShare) * 100 : 0;
                return (
                  <div key={holder.account} className="flex items-center gap-3">
                    <span className="w-6 text-right text-xs text-muted-foreground font-mono">
                      {i + 1}
                    </span>
                    <div className="w-24 shrink-0 truncate">
                      <AddressDisplay address={holder.account} chars={4} />
                    </div>
                    <div className="flex-1 relative h-7">
                      <div
                        className="absolute inset-y-0 left-0 rounded bg-primary/20 transition-all"
                        style={{ width: `${barWidth}%` }}
                      />
                      <div
                        className="absolute inset-y-0 left-0 rounded bg-primary/60 transition-all"
                        style={{ width: `${barWidth}%` }}
                      />
                      <span className="relative z-10 flex h-full items-center pl-2 text-xs font-medium">
                        {share.toFixed(2)}%
                      </span>
                    </div>
                    <span className="w-28 text-right text-xs font-mono text-muted-foreground shrink-0">
                      {formatBalance(holder.balance)}
                    </span>
                  </div>
                );
              })}
            </CardContent>
          </Card>

          {/* Concentration Metrics */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <TrendingUp className="h-5 w-5" />
                Concentration Metrics
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="grid gap-6 md:grid-cols-3">
                <div className="space-y-2">
                  <div className="flex justify-between text-sm">
                    <span className="text-muted-foreground">Top 1 Holder</span>
                    <span className="font-bold">
                      {concentration.top1.toFixed(2)}%
                    </span>
                  </div>
                  <div className="h-2 rounded-full bg-secondary overflow-hidden">
                    <div
                      className="h-full rounded-full bg-red-500 transition-all"
                      style={{ width: `${Math.min(concentration.top1, 100)}%` }}
                    />
                  </div>
                </div>
                <div className="space-y-2">
                  <div className="flex justify-between text-sm">
                    <span className="text-muted-foreground">
                      Top 5 Holders
                    </span>
                    <span className="font-bold">
                      {concentration.top5.toFixed(2)}%
                    </span>
                  </div>
                  <div className="h-2 rounded-full bg-secondary overflow-hidden">
                    <div
                      className="h-full rounded-full bg-orange-500 transition-all"
                      style={{ width: `${Math.min(concentration.top5, 100)}%` }}
                    />
                  </div>
                </div>
                <div className="space-y-2">
                  <div className="flex justify-between text-sm">
                    <span className="text-muted-foreground">
                      Top 10 Holders
                    </span>
                    <span className="font-bold">
                      {concentration.top10.toFixed(2)}%
                    </span>
                  </div>
                  <div className="h-2 rounded-full bg-secondary overflow-hidden">
                    <div
                      className="h-full rounded-full bg-yellow-500 transition-all"
                      style={{
                        width: `${Math.min(concentration.top10, 100)}%`,
                      }}
                    />
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* Search */}
          <div className="relative">
            <Search className="absolute left-3 top-3 h-4 w-4 text-muted-foreground" />
            <Input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search by address..."
              className="pl-10"
            />
          </div>

          {/* Holders Table */}
          {filtered.length === 0 ? (
            <Card>
              <CardContent className="flex flex-col items-center justify-center py-12">
                <Search className="h-10 w-10 text-muted-foreground/50" />
                <p className="mt-4 text-sm text-muted-foreground">
                  No addresses match your search
                </p>
              </CardContent>
            </Card>
          ) : (
            <Card>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead className="w-12">Rank</TableHead>
                    <TableHead>Address</TableHead>
                    <TableHead className="text-right">Balance</TableHead>
                    <TableHead className="text-right w-24">Share</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {filtered.map((holder, i) => {
                    const rank = holders.indexOf(holder) + 1;
                    return (
                      <TableRow key={holder.account}>
                        <TableCell className="font-mono text-muted-foreground">
                          {rank}
                        </TableCell>
                        <TableCell>
                          <AddressDisplay address={holder.account} chars={4} />
                        </TableCell>
                        <TableCell className="text-right font-mono">
                          {formatBalance(holder.balance)}
                        </TableCell>
                        <TableCell className="text-right">
                          <Badge variant="secondary" className="font-mono">
                            {shareOf(holder.balance).toFixed(2)}%
                          </Badge>
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </Card>
          )}
        </>
      )}
    </div>
  );
}
