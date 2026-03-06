"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useToken, useTokenActions } from "@/hooks/useToken";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { TxButton } from "@/components/shared/TxButton";
import { TOKEN_TYPES } from "@/lib/constants";
import { formatBalance } from "@/lib/utils";
import {
  Coins,
  Settings,
  Sparkles,
  ArrowRightLeft,
  Crown,
  PlusCircle,
  Send,
  RefreshCw,
} from "lucide-react";
import { useTranslations } from "next-intl";

const ALL_TYPES = [...TOKEN_TYPES, "Hybrid"] as const;

const TYPE_DESCRIPTIONS: Record<string, string> = {
  Points: "Reward points for loyalty programs",
  Governance: "Voting power in DAO governance",
  Equity: "Ownership shares with dividend rights",
  Membership: "Access tokens for member-only features",
  Share: "Profit-sharing tokens for investors",
  Bond: "Fixed-income tokens with maturity dates",
  Hybrid: "Custom multi-purpose token type",
};

export default function TokenConfigPage() {
  const { currentEntityId } = useEntityStore();
  const { config, isLoading, refetch } = useToken(currentEntityId);
  const actions = useTokenActions();
  const t = useTranslations("token");
  const tc = useTranslations("common");

  const [name, setName] = useState("");
  const [symbol, setSymbol] = useState("");
  const [decimals, setDecimals] = useState("12");
  const [tokenType, setTokenType] = useState<string>("Points");
  const [rewardRate, setRewardRate] = useState("");
  const [exchangeRate, setExchangeRate] = useState("");

  const [updRewardRate, setUpdRewardRate] = useState("");
  const [updExchangeRate, setUpdExchangeRate] = useState("");
  const [updMinRedeem, setUpdMinRedeem] = useState("");
  const [updMaxRedeem, setUpdMaxRedeem] = useState("");
  const [updTransferable, setUpdTransferable] = useState(true);

  const [newMaxSupply, setNewMaxSupply] = useState("");
  const [changeType, setChangeType] = useState<string>("");

  const [mintTo, setMintTo] = useState("");
  const [mintAmount, setMintAmount] = useState("");
  const [transferTo, setTransferTo] = useState("");
  const [transferAmount, setTransferAmount] = useState("");

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

  if (!config) {
    return (
      <div className="space-y-6">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t("title")}</h1>
          <p className="text-muted-foreground">{t("subtitle")}</p>
        </div>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Sparkles className="h-5 w-5" />
              Create Entity Token
            </CardTitle>
            <CardDescription>
              Configure and deploy a new token for this entity
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="grid gap-4 md:grid-cols-2">
              <div className="space-y-2">
                <label className="text-sm font-medium">Token Name</label>
                <Input
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="My Token"
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">Symbol</label>
                <Input
                  value={symbol}
                  onChange={(e) => setSymbol(e.target.value)}
                  placeholder="MTK"
                />
              </div>
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">Decimals</label>
              <Input
                type="number"
                value={decimals}
                onChange={(e) => setDecimals(e.target.value)}
                min="0"
                max="18"
              />
            </div>

            <div className="space-y-3">
              <label className="text-sm font-medium">Token Type</label>
              <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
                {ALL_TYPES.map((tp) => (
                  <button
                    key={tp}
                    type="button"
                    onClick={() => setTokenType(tp)}
                    className={`rounded-lg border-2 p-3 text-left transition-all ${
                      tokenType === tp
                        ? "border-primary bg-primary/5 ring-1 ring-primary/20"
                        : "border-border hover:border-primary/40"
                    }`}
                  >
                    <span className="text-sm font-medium">{tp}</span>
                    <p className="mt-1 text-xs text-muted-foreground leading-tight">
                      {TYPE_DESCRIPTIONS[tp]}
                    </p>
                  </button>
                ))}
              </div>
            </div>

            <div className="grid gap-4 md:grid-cols-2">
              <div className="space-y-2">
                <label className="text-sm font-medium">
                  Reward Rate (basis points)
                </label>
                <Input
                  type="number"
                  value={rewardRate}
                  onChange={(e) => setRewardRate(e.target.value)}
                  placeholder="100"
                  min="0"
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">
                  Exchange Rate (basis points)
                </label>
                <Input
                  type="number"
                  value={exchangeRate}
                  onChange={(e) => setExchangeRate(e.target.value)}
                  placeholder="10000"
                  min="0"
                />
              </div>
            </div>

            <Separator />

            <TxButton
              onClick={() => {
                if (!name || !symbol || !currentEntityId) return;
                actions.createToken(
                  currentEntityId,
                  name,
                  symbol,
                  Number(decimals) || 12,
                  tokenType,
                  Number(rewardRate) || 0,
                  Number(exchangeRate) || 0
                );
              }}
              txStatus={actions.txState.status}
              disabled={!name || !symbol}
              className="w-full"
            >
              <Sparkles className="mr-2 h-4 w-4" />
              Create Token
            </TxButton>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t("title")}</h1>
          <p className="text-muted-foreground">{t("subtitle")}</p>
        </div>
        <Button variant="outline" size="sm" onClick={refetch}>
          <RefreshCw className="mr-2 h-3 w-3" />
          Refresh
        </Button>
      </div>

      {/* Stats Row */}
      <div className="grid gap-4 md:grid-cols-3 lg:grid-cols-6">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Name</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-lg font-bold truncate">
              {config.name}{" "}
              <span className="text-muted-foreground text-sm font-normal">
                ({config.symbol})
              </span>
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Type</CardTitle>
          </CardHeader>
          <CardContent>
            <Badge variant="outline" className="text-sm">
              {config.tokenType}
            </Badge>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Total Supply</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-lg font-bold">{formatBalance(config.totalSupply)}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Max Supply</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-lg font-bold">
              {config.maxSupply > BigInt(0)
                ? formatBalance(config.maxSupply)
                : "Unlimited"}
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Holders</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-lg font-bold">{config.holderCount}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Status</CardTitle>
          </CardHeader>
          <CardContent>
            <StatusBadge status={config.enabled ? "Active" : "Disabled"} />
          </CardContent>
        </Card>
      </div>

      {/* Token Type Change */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Crown className="h-5 w-5" />
            Change Token Type
          </CardTitle>
          <CardDescription>
            Current type: <Badge variant="outline">{config.tokenType}</Badge>
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-4 lg:grid-cols-7">
            {ALL_TYPES.map((tp) => (
              <button
                key={tp}
                type="button"
                onClick={() => setChangeType(tp)}
                className={`rounded-lg border-2 p-3 text-center transition-all ${
                  config.tokenType === tp
                    ? "border-primary bg-primary/10 ring-1 ring-primary/30"
                    : changeType === tp
                      ? "border-primary/60 bg-primary/5"
                      : "border-border hover:border-primary/40"
                }`}
              >
                <span className="text-sm font-medium">{tp}</span>
                {config.tokenType === tp && (
                  <p className="mt-1 text-xs text-primary">Current</p>
                )}
              </button>
            ))}
          </div>
          <TxButton
            onClick={() => {
              if (changeType && currentEntityId) {
                actions.changeTokenType(currentEntityId, changeType);
              }
            }}
            txStatus={actions.txState.status}
            disabled={!changeType || changeType === config.tokenType}
          >
            Change Type
          </TxButton>
        </CardContent>
      </Card>

      {/* Configuration Update */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Settings className="h-5 w-5" />
            Update Configuration
          </CardTitle>
          <CardDescription>
            Modify token economic parameters
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 md:grid-cols-2">
            <div className="space-y-2">
              <label className="text-sm font-medium">
                Reward Rate (basis points)
              </label>
              <Input
                type="number"
                value={updRewardRate}
                onChange={(e) => setUpdRewardRate(e.target.value)}
                placeholder={String(config.rewardRate)}
                min="0"
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">
                Exchange Rate (basis points)
              </label>
              <Input
                type="number"
                value={updExchangeRate}
                onChange={(e) => setUpdExchangeRate(e.target.value)}
                placeholder={String(config.exchangeRate)}
                min="0"
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Min Redeem</label>
              <Input
                type="number"
                value={updMinRedeem}
                onChange={(e) => setUpdMinRedeem(e.target.value)}
                placeholder={formatBalance(config.minRedeem)}
                min="0"
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Max Redeem Per Order</label>
              <Input
                type="number"
                value={updMaxRedeem}
                onChange={(e) => setUpdMaxRedeem(e.target.value)}
                placeholder={formatBalance(config.maxRedeemPerOrder)}
                min="0"
              />
            </div>
          </div>
          <div className="flex items-center gap-3">
            <Switch
              checked={updTransferable}
              onCheckedChange={setUpdTransferable}
            />
            <label className="text-sm font-medium">Transferable</label>
          </div>
          <TxButton
            onClick={() => {
              if (!currentEntityId) return;
              actions.updateConfig(
                currentEntityId,
                Number(updRewardRate) || config.rewardRate,
                Number(updExchangeRate) || config.exchangeRate,
                updMinRedeem ? BigInt(updMinRedeem) : config.minRedeem,
                updMaxRedeem ? BigInt(updMaxRedeem) : config.maxRedeemPerOrder,
                updTransferable
              );
            }}
            txStatus={actions.txState.status}
          >
            <Settings className="mr-2 h-4 w-4" />
            Update Config
          </TxButton>
        </CardContent>
      </Card>

      {/* Max Supply */}
      <Card>
        <CardHeader>
          <CardTitle>Set Max Supply</CardTitle>
          <CardDescription>
            Current max supply:{" "}
            {config.maxSupply > BigInt(0)
              ? formatBalance(config.maxSupply)
              : "Unlimited"}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <Input
            type="number"
            value={newMaxSupply}
            onChange={(e) => setNewMaxSupply(e.target.value)}
            placeholder="New max supply (raw units)"
            min="0"
          />
          <TxButton
            onClick={() => {
              if (newMaxSupply && currentEntityId) {
                actions.setMaxSupply(currentEntityId, BigInt(newMaxSupply));
              }
            }}
            txStatus={actions.txState.status}
            disabled={!newMaxSupply}
          >
            Set Max Supply
          </TxButton>
        </CardContent>
      </Card>

      <div className="grid gap-6 md:grid-cols-2">
        {/* Mint */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <PlusCircle className="h-5 w-5" />
              Mint Tokens
            </CardTitle>
            <CardDescription>
              Issue new tokens to a recipient address
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Recipient Address</label>
              <Input
                value={mintTo}
                onChange={(e) => setMintTo(e.target.value)}
                placeholder="5xxx..."
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Amount</label>
              <Input
                type="number"
                value={mintAmount}
                onChange={(e) => setMintAmount(e.target.value)}
                placeholder="0"
                min="0"
              />
            </div>
            <TxButton
              onClick={() => {
                if (mintTo && mintAmount && currentEntityId) {
                  actions.mintTokens(currentEntityId, mintTo, BigInt(mintAmount));
                }
              }}
              txStatus={actions.txState.status}
              disabled={!mintTo || !mintAmount}
              className="w-full"
            >
              <PlusCircle className="mr-2 h-4 w-4" />
              Mint Tokens
            </TxButton>
          </CardContent>
        </Card>

        {/* Transfer */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Send className="h-5 w-5" />
              Transfer Tokens
            </CardTitle>
            <CardDescription>
              Transfer entity tokens to another address
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Recipient Address</label>
              <Input
                value={transferTo}
                onChange={(e) => setTransferTo(e.target.value)}
                placeholder="5xxx..."
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Amount</label>
              <Input
                type="number"
                value={transferAmount}
                onChange={(e) => setTransferAmount(e.target.value)}
                placeholder="0"
                min="0"
              />
            </div>
            <TxButton
              onClick={() => {
                if (transferTo && transferAmount && currentEntityId) {
                  actions.transferTokens(
                    currentEntityId,
                    transferTo,
                    BigInt(transferAmount)
                  );
                }
              }}
              txStatus={actions.txState.status}
              disabled={!transferTo || !transferAmount}
              className="w-full"
            >
              <Send className="mr-2 h-4 w-4" />
              Transfer Tokens
            </TxButton>
          </CardContent>
        </Card>
      </div>

      {actions.txState.status === "finalized" && (
        <div className="rounded-lg border border-green-200 bg-green-50 p-3 text-sm text-green-700 dark:border-green-800 dark:bg-green-950/50 dark:text-green-400">
          Transaction finalized successfully.
        </div>
      )}
      {actions.txState.status === "error" && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
          {actions.txState.error}
        </div>
      )}
    </div>
  );
}
