"use client";

import { useEntityStore } from "@/stores/entity";
import { useToken, useTokenActions } from "@/hooks/useToken";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { Badge } from "@/components/ui/badge";
import { Coins, Settings, Lock, Unlock } from "lucide-react";
import { useState } from "react";
import { useTranslations } from "next-intl";

export default function TokenConfigPage() {
  const { currentEntityId } = useEntityStore();
  const { config, isLoading } = useToken(currentEntityId);
  const actions = useTokenActions();
  const [mintAmount, setMintAmount] = useState("");
  const [mintTo, setMintTo] = useState("");
  const t = useTranslations("token");
  const tc = useTranslations("common");

  if (!currentEntityId) return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  if (isLoading) return <div className="flex h-full items-center justify-center"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">{t("title")}</h1>
        <p className="text-muted-foreground">{t("subtitle")}</p>
      </div>

      {!config ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Coins className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">{t("noToken")}</p>
            <p className="text-sm text-muted-foreground">{t("noTokenDesc")}</p>
          </CardContent>
        </Card>
      ) : (
        <>
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Token Type</CardTitle></CardHeader>
              <CardContent><Badge variant="outline">{config.tokenType}</Badge></CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Supply</CardTitle></CardHeader>
              <CardContent><p className="text-2xl font-bold">—</p></CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Max Supply</CardTitle></CardHeader>
              <CardContent><p className="text-2xl font-bold">{config.maxSupply?.toLocaleString() || "Unlimited"}</p></CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Transfer</CardTitle></CardHeader>
              <CardContent><StatusBadge status={config.transferRestriction || "Free"} /></CardContent>
            </Card>
          </div>

          <div className="grid gap-6 md:grid-cols-2">
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2"><Settings className="h-5 w-5" />Token Details</CardTitle>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="flex justify-between"><span className="text-sm text-muted-foreground">Enabled</span><StatusBadge status={config.enabled ? "Active" : "Disabled"} /></div>
                <div className="flex justify-between"><span className="text-sm text-muted-foreground">Transferable</span><span className="text-sm">{config.transferable ? "Yes" : "No"}</span></div>
                <div className="flex justify-between"><span className="text-sm text-muted-foreground">Dividend Config</span><span className="text-sm">{config.dividendConfig?.enabled ? "Enabled" : "Disabled"}</span></div>
                <div className="flex justify-between"><span className="text-sm text-muted-foreground">Min KYC Level</span><span className="text-sm font-mono">{config.minReceiverKyc || "N/A"}</span></div>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2"><Coins className="h-5 w-5" />Mint Tokens</CardTitle>
                <CardDescription>Mint new tokens to a specified address</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Recipient Address</label>
                  <Input value={mintTo} onChange={(e) => setMintTo(e.target.value)} placeholder="5xxx..." />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Amount</label>
                  <Input type="number" value={mintAmount} onChange={(e) => setMintAmount(e.target.value)} placeholder="0" />
                </div>
                <Button onClick={() => { if (mintTo && mintAmount && currentEntityId) actions.mintTokens(currentEntityId, mintTo, BigInt(mintAmount)); }} className="w-full">
                  Mint Tokens
                </Button>
              </CardContent>
            </Card>
          </div>
        </>
      )}
    </div>
  );
}
