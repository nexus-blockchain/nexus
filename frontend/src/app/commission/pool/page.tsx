"use client";

import { useState, useEffect, useCallback } from "react";
import { useEntityStore } from "@/stores/entity";
import { getApi } from "@/hooks/useApi";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { formatBalance, basisPointsToPercent } from "@/lib/utils";
import { ArrowLeft, Gift, Clock, Trophy, Users, Percent, BarChart3 } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

interface PoolRound {
  id: number;
  entityId: number;
  totalPool: bigint;
  distributed: bigint;
  qualifierCount: number;
  status: string;
  startBlock: number;
  endBlock: number;
}

interface PoolConfig {
  entityId: number;
  enabled: boolean;
  poolShareBps: number;
  minQualifyingOrders: number;
  roundDurationBlocks: number;
}

export default function CommissionPoolPage() {
  const { currentEntityId } = useEntityStore();
  const tc = useTranslations("common");

  const [config, setConfig] = useState<PoolConfig | null>(null);
  const [rounds, setRounds] = useState<PoolRound[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (currentEntityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;

      const rawConfig = await q.commissionPoolReward?.poolConfig?.(currentEntityId);
      if (rawConfig && !rawConfig.isNone) {
        setConfig(rawConfig.toJSON() as unknown as PoolConfig);
      }

      const rawRounds = await q.commissionPoolReward?.poolRounds?.entries(currentEntityId);
      if (rawRounds) {
        const parsed = rawRounds.map(([_k, v]: [unknown, { toJSON: () => PoolRound }]) => {
          const r = v.toJSON();
          return {
            ...r,
            totalPool: BigInt(String(r.totalPool || 0)),
            distributed: BigInt(String(r.distributed || 0)),
          };
        });
        setRounds(parsed);
      }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [currentEntityId]);

  useEffect(() => { fetch(); }, [fetch]);

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const totalDistributed = rounds.reduce((sum, r) => sum + BigInt(r.distributed || 0), BigInt(0));
  const totalQualifiers = rounds.reduce((sum, r) => sum + r.qualifierCount, 0);
  const currentRound = rounds.find((r) => r.status === "Active");
  const pastRounds = rounds.filter((r) => r.status !== "Active");

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/commission"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Reward Pool</h1>
          <p className="text-muted-foreground">Pool-based commission distribution rounds</p>
        </div>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
      ) : (
        <>
          <div className="grid gap-4 md:grid-cols-4">
            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Current Pool</CardTitle>
                <Gift className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold">
                  {currentRound ? formatBalance(currentRound.totalPool) : "—"} NEX
                </p>
                <p className="text-xs text-muted-foreground">
                  {currentRound ? `Round #${currentRound.id}` : "No active round"}
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Current Round</CardTitle>
                <Clock className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold">
                  {currentRound ? `#${currentRound.id}` : "—"}
                </p>
                <p className="text-xs text-muted-foreground">
                  {currentRound
                    ? `Block ${currentRound.startBlock} → ${currentRound.endBlock}`
                    : "No active round"
                  }
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Total Distributed</CardTitle>
                <Trophy className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold">{formatBalance(totalDistributed)} NEX</p>
                <p className="text-xs text-muted-foreground">{rounds.length} rounds total</p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Total Qualifiers</CardTitle>
                <Users className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold">{totalQualifiers}</p>
                <p className="text-xs text-muted-foreground">Across all rounds</p>
              </CardContent>
            </Card>
          </div>

          {config && (
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2"><Percent className="h-5 w-5" />Pool Configuration</CardTitle>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Status</span>
                  <Badge variant={config.enabled ? "default" : "secondary"}>{config.enabled ? "Enabled" : "Disabled"}</Badge>
                </div>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Pool Share</span>
                  <span className="text-sm font-medium">{basisPointsToPercent(config.poolShareBps)}</span>
                </div>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Min Qualifying Orders</span>
                  <span className="text-sm font-medium">{config.minQualifyingOrders}</span>
                </div>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Round Duration</span>
                  <span className="text-sm font-medium">{config.roundDurationBlocks} blocks</span>
                </div>
              </CardContent>
            </Card>
          )}

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2"><BarChart3 className="h-5 w-5" />Pool Rounds</CardTitle>
              <CardDescription>{rounds.length} rounds recorded</CardDescription>
            </CardHeader>
            <CardContent>
              {rounds.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-12">
                  <Trophy className="h-12 w-12 text-muted-foreground/50" />
                  <p className="mt-4 text-lg font-medium">No Pool Rounds</p>
                  <p className="text-sm text-muted-foreground">Pool rounds will appear here when the pool reward plugin is enabled and configured.</p>
                </div>
              ) : (
                <div className="space-y-2">
                  {rounds.map((round) => (
                    <div key={round.id} className="flex items-center gap-4 rounded-lg border p-4">
                      <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-primary/10">
                        <span className="text-sm font-bold">#{round.id}</span>
                      </div>
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <StatusBadge status={round.status} />
                          <span className="text-xs text-muted-foreground">
                            Block {round.startBlock} → {round.endBlock}
                          </span>
                        </div>
                        <div className="flex items-center gap-3 mt-1 text-xs text-muted-foreground">
                          <span>{round.qualifierCount} qualifiers</span>
                          <span>Pool: {formatBalance(round.totalPool)} NEX</span>
                        </div>
                      </div>
                      <div className="text-right">
                        <p className="text-sm font-semibold">{formatBalance(round.distributed)} NEX</p>
                        <p className="text-xs text-muted-foreground">distributed</p>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
        </>
      )}
    </div>
  );
}
