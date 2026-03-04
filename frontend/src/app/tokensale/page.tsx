"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useSaleRounds, useTokensaleActions } from "@/hooks/useTokensale";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { Progress } from "@/components/ui/progress";
import { Rocket, Clock, DollarSign, Users } from "lucide-react";
import { useTranslations } from "next-intl";

export default function TokensalePage() {
  const { currentEntityId } = useEntityStore();
  const { rounds, isLoading } = useSaleRounds(currentEntityId);
  const actions = useTokensaleActions();
  const [subscribeAmount, setSubscribeAmount] = useState("");
  const t = useTranslations("tokensale");
  const tc = useTranslations("common");

  if (!currentEntityId) return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">{t("title")}</h1>
        <p className="text-muted-foreground">{t("subtitle")}</p>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
      ) : rounds.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Rocket className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No Sale Rounds</p>
            <p className="text-sm text-muted-foreground">Create a token sale round to raise funds for your entity.</p>
            <Button className="mt-4" onClick={() => actions.createSaleRound(currentEntityId, "Public", BigInt(1000000), BigInt(100), 0, 0)}>
              <Rocket className="mr-2 h-4 w-4" />Create Sale Round
            </Button>
          </CardContent>
        </Card>
      ) : (
        <div className="space-y-6">
          {rounds.map((round: any, i: number) => {
            const progress = round.totalSupply > 0 ? ((round.sold || 0) / round.totalSupply) * 100 : 0;
            return (
              <Card key={i}>
                <CardHeader>
                  <div className="flex items-center justify-between">
                    <CardTitle className="text-lg">Round #{round.id || i + 1} — {round.name || round.saleType || "Token Sale"}</CardTitle>
                    <StatusBadge status={round.status || "Pending"} />
                  </div>
                  <CardDescription>Price: {round.price || "—"} NEX per token</CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                  <div className="grid gap-4 md:grid-cols-4">
                    <div className="flex items-center gap-2">
                      <DollarSign className="h-4 w-4 text-muted-foreground" />
                      <div>
                        <p className="text-sm font-medium">{round.totalSupply?.toLocaleString() || 0}</p>
                        <p className="text-xs text-muted-foreground">Total Supply</p>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <Rocket className="h-4 w-4 text-muted-foreground" />
                      <div>
                        <p className="text-sm font-medium">{round.sold?.toLocaleString() || 0}</p>
                        <p className="text-xs text-muted-foreground">Sold</p>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <Users className="h-4 w-4 text-muted-foreground" />
                      <div>
                        <p className="text-sm font-medium">{round.subscribers || 0}</p>
                        <p className="text-xs text-muted-foreground">Subscribers</p>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <Clock className="h-4 w-4 text-muted-foreground" />
                      <div>
                        <p className="text-sm font-medium">{round.endBlock || "—"}</p>
                        <p className="text-xs text-muted-foreground">End Block</p>
                      </div>
                    </div>
                  </div>

                  <div>
                    <div className="mb-1 flex justify-between text-xs">
                      <span>Progress</span>
                      <span>{progress.toFixed(1)}%</span>
                    </div>
                    <Progress value={progress} className="h-2" />
                  </div>

                  {round.status === "Active" && (
                    <div className="flex items-end gap-4">
                      <div className="flex-1 space-y-2">
                        <label className="text-sm font-medium">Subscribe Amount (NEX)</label>
                        <Input type="number" value={subscribeAmount} onChange={(e) => setSubscribeAmount(e.target.value)} placeholder="0" />
                      </div>
                      <Button onClick={() => { if (subscribeAmount) actions.subscribe(round.id, BigInt(subscribeAmount), "NEX"); }}>Subscribe</Button>
                    </div>
                  )}

                  <div className="flex flex-wrap gap-2">
                    {round.status === "Pending" && <Button size="sm" onClick={() => actions.startSale(round.id)}>Start Sale</Button>}
                    {round.status === "Active" && <Button size="sm" variant="outline" onClick={() => actions.endSale(round.id)}>End Sale</Button>}
                    {round.status === "Ended" && (
                      <>
                        <Button size="sm" onClick={() => actions.claimTokens(round.id)}>Claim Tokens</Button>
                        <Button size="sm" variant="outline" onClick={() => actions.withdrawFunds(round.id)}>Withdraw Funds</Button>
                      </>
                    )}
                  </div>
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}
    </div>
  );
}
