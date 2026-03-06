"use client";

import { useState } from "react";
import Link from "next/link";
import { useEntityStore } from "@/stores/entity";
import { useWalletStore } from "@/stores/wallet";
import { useSaleRounds, useTokensaleActions } from "@/hooks/useTokensale";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import { Separator } from "@/components/ui/separator";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { TxButton } from "@/components/shared/TxButton";
import { formatBalance, formatNumber } from "@/lib/utils";
import { SALE_MODES } from "@/lib/constants";
import { useTranslations } from "next-intl";
import {
  Rocket, Clock, Users, Plus, TrendingUp, BarChart3,
  Coins, Play, Pause, Square, XCircle, ShoppingCart,
  Download, ChevronRight, Target, CircleDot,
} from "lucide-react";

const MODE_ICONS: Record<string, typeof Rocket> = {
  FixedPrice: Target,
  DutchAuction: TrendingUp,
  WhitelistAllocation: Users,
  FCFS: Rocket,
  Lottery: CircleDot,
};

export default function TokensalePage() {
  const { currentEntityId } = useEntityStore();
  const { address } = useWalletStore();
  const { rounds, isLoading, refetch } = useSaleRounds(currentEntityId);
  const actions = useTokensaleActions();
  const [subscribeAmounts, setSubscribeAmounts] = useState<Record<number, string>>({});
  const t = useTranslations("tokensale");
  const tc = useTranslations("common");

  if (!currentEntityId) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        {tc("selectEntity")}
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
      </div>
    );
  }

  const activeRounds = rounds.filter((r) => r.status === "Active").length;
  const totalSold = rounds.reduce((sum, r) => sum + r.soldAmount, BigInt(0));
  const totalParticipants = rounds.reduce((sum, r) => sum + r.participantsCount, 0);

  const progress = (round: typeof rounds[0]) => {
    if (round.totalSupply === BigInt(0)) return 0;
    return Number((round.soldAmount * BigInt(10000)) / round.totalSupply) / 100;
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t("title")}</h1>
          <p className="text-muted-foreground">{t("subtitle")}</p>
        </div>
        <Button asChild>
          <Link href="/tokensale/create">
            <Plus className="mr-2 h-4 w-4" />
            Create Round
          </Link>
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Active Rounds</CardTitle>
            <Rocket className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{activeRounds}</p>
            <p className="text-xs text-muted-foreground">Currently running</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Total Rounds</CardTitle>
            <BarChart3 className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{rounds.length}</p>
            <p className="text-xs text-muted-foreground">All time</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Total Sold</CardTitle>
            <Coins className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{formatBalance(totalSold)}</p>
            <p className="text-xs text-muted-foreground">Across all rounds</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Total Participants</CardTitle>
            <Users className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{formatNumber(totalParticipants)}</p>
            <p className="text-xs text-muted-foreground">All subscribers</p>
          </CardContent>
        </Card>
      </div>

      {rounds.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Rocket className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No Sale Rounds</p>
            <p className="text-sm text-muted-foreground">
              Create a token sale round to raise funds for your entity.
            </p>
            <Button className="mt-4" asChild>
              <Link href="/tokensale/create">
                <Plus className="mr-2 h-4 w-4" />
                Create First Round
              </Link>
            </Button>
          </CardContent>
        </Card>
      ) : (
        <div className="space-y-4">
          {rounds.map((round) => {
            const pct = progress(round);
            const ModeIcon = MODE_ICONS[round.mode] || Rocket;

            return (
              <Card key={round.id}>
                <CardHeader className="pb-3">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-3">
                      <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary/10">
                        <ModeIcon className="h-4 w-4 text-primary" />
                      </div>
                      <div>
                        <CardTitle className="text-lg">
                          <Link
                            href={`/tokensale/${round.id}`}
                            className="hover:underline"
                          >
                            Round #{round.id}
                          </Link>
                        </CardTitle>
                        <CardDescription className="flex items-center gap-2">
                          <Badge variant="outline" className="text-xs">
                            {round.mode}
                          </Badge>
                          {round.kycRequired && (
                            <Badge variant="secondary" className="text-xs">
                              KYC Lv.{round.minKycLevel}
                            </Badge>
                          )}
                        </CardDescription>
                      </div>
                    </div>
                    <div className="flex items-center gap-3">
                      <StatusBadge status={round.status} />
                      <Button variant="ghost" size="icon" asChild>
                        <Link href={`/tokensale/${round.id}`}>
                          <ChevronRight className="h-4 w-4" />
                        </Link>
                      </Button>
                    </div>
                  </div>
                </CardHeader>
                <CardContent className="space-y-4">
                  <div className="grid gap-4 md:grid-cols-5">
                    <div className="flex items-center gap-2">
                      <Coins className="h-4 w-4 text-muted-foreground" />
                      <div>
                        <p className="text-sm font-medium">
                          {formatBalance(round.soldAmount)} / {formatBalance(round.totalSupply)}
                        </p>
                        <p className="text-xs text-muted-foreground">Sold / Supply</p>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <Users className="h-4 w-4 text-muted-foreground" />
                      <div>
                        <p className="text-sm font-medium">
                          {formatNumber(round.participantsCount)}
                        </p>
                        <p className="text-xs text-muted-foreground">Participants</p>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <Clock className="h-4 w-4 text-muted-foreground" />
                      <div>
                        <p className="text-sm font-medium">#{round.startBlock || "—"}</p>
                        <p className="text-xs text-muted-foreground">Start Block</p>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <Clock className="h-4 w-4 text-muted-foreground" />
                      <div>
                        <p className="text-sm font-medium">#{round.endBlock || "—"}</p>
                        <p className="text-xs text-muted-foreground">End Block</p>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <Target className="h-4 w-4 text-muted-foreground" />
                      <div>
                        <p className="text-sm font-medium">
                          {formatBalance(round.softCap)}
                        </p>
                        <p className="text-xs text-muted-foreground">Soft Cap</p>
                      </div>
                    </div>
                  </div>

                  <div>
                    <div className="mb-1 flex justify-between text-xs">
                      <span>Progress</span>
                      <span>{pct.toFixed(1)}%</span>
                    </div>
                    <Progress value={pct} className="h-2" />
                  </div>

                  {round.status === "Active" && (
                    <>
                      <Separator />
                      <div className="flex items-end gap-4">
                        <div className="flex-1 space-y-2">
                          <label className="text-sm font-medium">Subscribe Amount</label>
                          <Input
                            type="number"
                            value={subscribeAmounts[round.id] || ""}
                            onChange={(e) =>
                              setSubscribeAmounts((prev) => ({
                                ...prev,
                                [round.id]: e.target.value,
                              }))
                            }
                            placeholder="Token amount"
                            min="0"
                          />
                        </div>
                        <TxButton
                          onClick={() => {
                            const amt = subscribeAmounts[round.id];
                            if (amt) actions.subscribe(round.id, BigInt(amt), null);
                          }}
                          txStatus={actions.txState.status}
                          disabled={!subscribeAmounts[round.id]}
                        >
                          <ShoppingCart className="mr-2 h-4 w-4" />
                          Subscribe
                        </TxButton>
                      </div>
                    </>
                  )}

                  <div className="flex flex-wrap gap-2">
                    {round.status === "NotStarted" && (
                      <TxButton
                        size="sm"
                        onClick={() => actions.startSale(round.id)}
                        txStatus={actions.txState.status}
                      >
                        <Play className="mr-2 h-3.5 w-3.5" />
                        Start
                      </TxButton>
                    )}
                    {round.status === "Active" && (
                      <>
                        <TxButton
                          size="sm"
                          variant="outline"
                          onClick={() => actions.pauseSale(round.id)}
                          txStatus={actions.txState.status}
                        >
                          <Pause className="mr-2 h-3.5 w-3.5" />
                          Pause
                        </TxButton>
                        <TxButton
                          size="sm"
                          variant="outline"
                          onClick={() => actions.endSale(round.id)}
                          txStatus={actions.txState.status}
                        >
                          <Square className="mr-2 h-3.5 w-3.5" />
                          End
                        </TxButton>
                      </>
                    )}
                    {round.status === "Paused" && (
                      <TxButton
                        size="sm"
                        onClick={() => actions.resumeSale(round.id)}
                        txStatus={actions.txState.status}
                      >
                        <Play className="mr-2 h-3.5 w-3.5" />
                        Resume
                      </TxButton>
                    )}
                    {["Ended", "Completed"].includes(round.status) && (
                      <>
                        <TxButton
                          size="sm"
                          onClick={() => actions.claimTokens(round.id)}
                          txStatus={actions.txState.status}
                        >
                          <Download className="mr-2 h-3.5 w-3.5" />
                          Claim
                        </TxButton>
                        <TxButton
                          size="sm"
                          variant="outline"
                          onClick={() => actions.withdrawFunds(round.id)}
                          txStatus={actions.txState.status}
                        >
                          <Download className="mr-2 h-3.5 w-3.5" />
                          Withdraw
                        </TxButton>
                      </>
                    )}
                    {!["Ended", "Completed", "Cancelled"].includes(round.status) && (
                      <TxButton
                        size="sm"
                        variant="destructive"
                        onClick={() => actions.cancelSale(round.id)}
                        txStatus={actions.txState.status}
                      >
                        <XCircle className="mr-2 h-3.5 w-3.5" />
                        Cancel
                      </TxButton>
                    )}
                  </div>
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}

      {actions.txState.status === "finalized" && (
        <div className="rounded-lg border border-green-200 bg-green-50 p-4 dark:border-green-800 dark:bg-green-950">
          <p className="text-sm text-green-800 dark:text-green-200">
            Transaction completed successfully!
          </p>
          <Button
            variant="link"
            className="mt-1 h-auto p-0 text-green-700 dark:text-green-300"
            onClick={() => {
              actions.resetTx();
              refetch();
            }}
          >
            Refresh data
          </Button>
        </div>
      )}
      {actions.txState.status === "error" && (
        <p className="text-sm text-destructive">{actions.txState.error}</p>
      )}
    </div>
  );
}
