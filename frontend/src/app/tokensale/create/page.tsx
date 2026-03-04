"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useEntityStore } from "@/stores/entity";
import { useTx } from "@/hooks/useTx";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { TxButton } from "@/components/shared/TxButton";
import { ArrowLeft, Rocket, Settings } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function CreateTokenSalePage() {
  const { currentEntityId } = useEntityStore();
  const { submit, state: txState } = useTx();
  const router = useRouter();
  const tc = useTranslations("common");

  const [totalSupply, setTotalSupply] = useState("");
  const [pricePerToken, setPricePerToken] = useState("");
  const [minPurchase, setMinPurchase] = useState("");
  const [maxPurchase, setMaxPurchase] = useState("");
  const [startBlock, setStartBlock] = useState("");
  const [endBlock, setEndBlock] = useState("");
  const [vestingEnabled, setVestingEnabled] = useState(false);
  const [vestingPeriod, setVestingPeriod] = useState("");
  const [cliffPeriod, setCliffPeriod] = useState("");
  const [initialUnlock, setInitialUnlock] = useState("1000");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const handleCreate = () => {
    if (!totalSupply || !pricePerToken) return;
    const vestingConfig = vestingEnabled
      ? { period: Number(vestingPeriod), cliff: Number(cliffPeriod), initialUnlockBps: Number(initialUnlock) }
      : null;
    submit("entityTokensale", "createRound", [
      currentEntityId,
      BigInt(totalSupply),
      BigInt(pricePerToken),
      BigInt(minPurchase || 0),
      BigInt(maxPurchase || 0),
      Number(startBlock || 0),
      Number(endBlock || 0),
      vestingConfig,
    ]);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/tokensale"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Create Sale Round</h1>
          <p className="text-muted-foreground">Set up a new token sale round</p>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Rocket className="h-5 w-5" />Sale Parameters</CardTitle>
            <CardDescription>Configure the token sale round</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Total Supply *</label>
              <Input type="number" value={totalSupply} onChange={(e) => setTotalSupply(e.target.value)} placeholder="Tokens available for sale" min="0" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Price per Token (NEX) *</label>
              <Input type="number" value={pricePerToken} onChange={(e) => setPricePerToken(e.target.value)} placeholder="0" min="0" />
            </div>
            <div className="grid gap-4 grid-cols-2">
              <div className="space-y-2">
                <label className="text-sm font-medium">Min Purchase</label>
                <Input type="number" value={minPurchase} onChange={(e) => setMinPurchase(e.target.value)} placeholder="0 = no min" min="0" />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">Max Purchase</label>
                <Input type="number" value={maxPurchase} onChange={(e) => setMaxPurchase(e.target.value)} placeholder="0 = no max" min="0" />
              </div>
            </div>
            <div className="grid gap-4 grid-cols-2">
              <div className="space-y-2">
                <label className="text-sm font-medium">Start Block</label>
                <Input type="number" value={startBlock} onChange={(e) => setStartBlock(e.target.value)} placeholder="0 = immediate" min="0" />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">End Block</label>
                <Input type="number" value={endBlock} onChange={(e) => setEndBlock(e.target.value)} placeholder="0 = no end" min="0" />
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Settings className="h-5 w-5" />Vesting Configuration</CardTitle>
            <CardDescription>Token release schedule after purchase</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between rounded-lg border p-4">
              <div>
                <p className="text-sm font-medium">Enable Vesting</p>
                <p className="text-xs text-muted-foreground">Tokens unlock gradually over time</p>
              </div>
              <Switch checked={vestingEnabled} onCheckedChange={setVestingEnabled} />
            </div>
            {vestingEnabled && (
              <>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Cliff Period (blocks)</label>
                  <Input type="number" value={cliffPeriod} onChange={(e) => setCliffPeriod(e.target.value)} placeholder="Blocks before first unlock" min="0" />
                  <p className="text-xs text-muted-foreground">~{Math.round(Number(cliffPeriod || 0) * 6 / 3600)} hours</p>
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Vesting Period (blocks)</label>
                  <Input type="number" value={vestingPeriod} onChange={(e) => setVestingPeriod(e.target.value)} placeholder="Total unlock duration" min="0" />
                  <p className="text-xs text-muted-foreground">~{Math.round(Number(vestingPeriod || 0) * 6 / 3600)} hours</p>
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Initial Unlock (basis points)</label>
                  <Input type="number" value={initialUnlock} onChange={(e) => setInitialUnlock(e.target.value)} min="0" max="10000" />
                  <p className="text-xs text-muted-foreground">{Number(initialUnlock) / 100}% unlocked immediately at cliff</p>
                </div>
              </>
            )}
          </CardContent>
        </Card>
      </div>

      <div className="flex items-center gap-4">
        <TxButton onClick={handleCreate} txStatus={txState.status} disabled={!totalSupply || !pricePerToken}>
          <Rocket className="mr-2 h-4 w-4" />Create Sale Round
        </TxButton>
        <Button variant="outline" asChild>
          <Link href="/tokensale">{tc("cancel")}</Link>
        </Button>
      </div>

      {txState.status === "finalized" && (
        <div className="rounded-lg border border-green-200 bg-green-50 p-4">
          <p className="text-sm text-green-800">Sale round created successfully!</p>
          <Button variant="link" className="mt-1 h-auto p-0 text-green-700" onClick={() => router.push("/tokensale")}>
            Back to Token Sale
          </Button>
        </div>
      )}
      {txState.status === "error" && <p className="text-sm text-destructive">{txState.error}</p>}
    </div>
  );
}
