"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import {
  useToken,
  useTokenActions,
  useTokenWhitelist,
  useTokenBlacklist,
} from "@/hooks/useToken";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import { TRANSFER_RESTRICTION_MODES, KYC_LEVELS } from "@/lib/constants";
import {
  Shield,
  ShieldCheck,
  UserPlus,
  UserMinus,
  X,
  CheckCircle2,
} from "lucide-react";
import { useTranslations } from "next-intl";

const MODE_DESCRIPTIONS: Record<string, string> = {
  None: "No restrictions — anyone can freely transfer tokens between accounts.",
  Whitelist:
    "Only addresses on the whitelist can receive token transfers.",
  Blacklist:
    "All addresses except those on the blacklist can receive transfers.",
  KycRequired:
    "Recipients must have completed KYC verification above the minimum level.",
  MembersOnly:
    "Only registered entity members can transfer tokens to each other.",
};

const MODE_ICONS: Record<string, string> = {
  None: "🔓",
  Whitelist: "✅",
  Blacklist: "🚫",
  KycRequired: "🛡️",
  MembersOnly: "👥",
};

const KYC_DESCRIPTIONS: Record<string, string> = {
  None: "No KYC required",
  Basic: "Email & phone verification",
  Standard: "ID document verification",
  Enhanced: "ID + address + liveness",
  Full: "Full regulatory compliance",
};

export default function TransferRestrictionPage() {
  const { currentEntityId } = useEntityStore();
  const { config, isLoading } = useToken(currentEntityId);
  const actions = useTokenActions();
  const {
    list: whitelist,
    isLoading: whitelistLoading,
    refetch: refetchWhitelist,
  } = useTokenWhitelist(currentEntityId);
  const {
    list: blacklist,
    isLoading: blacklistLoading,
    refetch: refetchBlacklist,
  } = useTokenBlacklist(currentEntityId);
  const tc = useTranslations("common");

  const [selectedMode, setSelectedMode] = useState<string>("None");
  const [selectedKyc, setSelectedKyc] = useState<number>(0);
  const [whitelistAddr, setWhitelistAddr] = useState("");
  const [blacklistAddr, setBlacklistAddr] = useState("");

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

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">
          Transfer Restrictions
        </h1>
        <p className="text-muted-foreground">
          Control who can send and receive entity tokens
        </p>
      </div>

      {/* Current Status */}
      {config && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <ShieldCheck className="h-5 w-5" />
              Current Configuration
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid gap-4 md:grid-cols-3">
              <div className="rounded-lg border p-4 space-y-1">
                <p className="text-xs text-muted-foreground uppercase tracking-wider">
                  Restriction Mode
                </p>
                <Badge variant="outline" className="text-sm">
                  {config.transferRestriction || "None"}
                </Badge>
                <p className="text-xs text-muted-foreground mt-1">
                  {MODE_DESCRIPTIONS[config.transferRestriction || "None"]}
                </p>
              </div>
              <div className="rounded-lg border p-4 space-y-1">
                <p className="text-xs text-muted-foreground uppercase tracking-wider">
                  Transferable
                </p>
                <Badge
                  variant={config.transferable ? "default" : "secondary"}
                >
                  {config.transferable ? "Yes" : "No"}
                </Badge>
              </div>
              <div className="rounded-lg border p-4 space-y-1">
                <p className="text-xs text-muted-foreground uppercase tracking-wider">
                  Min KYC Level
                </p>
                <Badge variant="outline">
                  {KYC_LEVELS[config.minReceiverKyc] || "None"}
                </Badge>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Restriction Mode Selector */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Shield className="h-5 w-5" />
            Transfer Restriction Mode
          </CardTitle>
          <CardDescription>
            Select how token transfers should be restricted
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-5">
            {TRANSFER_RESTRICTION_MODES.map((mode) => (
              <button
                key={mode}
                type="button"
                onClick={() => setSelectedMode(mode)}
                className={`rounded-lg border-2 p-4 text-left transition-all ${
                  selectedMode === mode
                    ? "border-primary bg-primary/5 ring-1 ring-primary/20"
                    : "border-border hover:border-primary/40"
                }`}
              >
                <div className="flex items-center gap-2">
                  <span className="text-lg">{MODE_ICONS[mode]}</span>
                  <span className="text-sm font-semibold">{mode}</span>
                </div>
                <p className="mt-2 text-xs text-muted-foreground leading-relaxed">
                  {MODE_DESCRIPTIONS[mode]}
                </p>
                {selectedMode === mode && (
                  <CheckCircle2 className="mt-2 h-4 w-4 text-primary" />
                )}
              </button>
            ))}
          </div>
          <TxButton
            onClick={() => {
              if (currentEntityId)
                actions.setTransferRestriction(currentEntityId, selectedMode);
            }}
            txStatus={actions.txState.status}
          >
            <ShieldCheck className="mr-2 h-4 w-4" />
            Set Restriction Mode
          </TxButton>
        </CardContent>
      </Card>

      {/* KYC Level Selector */}
      <Card>
        <CardHeader>
          <CardTitle>Minimum Receiver KYC Level</CardTitle>
          <CardDescription>
            Set the minimum KYC level required for token recipients
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-5">
            {KYC_LEVELS.map((level, i) => (
              <button
                key={level}
                type="button"
                onClick={() => setSelectedKyc(i)}
                className={`rounded-lg border-2 p-4 text-left transition-all ${
                  selectedKyc === i
                    ? "border-primary bg-primary/5 ring-1 ring-primary/20"
                    : "border-border hover:border-primary/40"
                }`}
              >
                <span className="text-sm font-semibold">{level}</span>
                <p className="mt-1 text-xs text-muted-foreground">
                  {KYC_DESCRIPTIONS[level]}
                </p>
                {selectedKyc === i && (
                  <CheckCircle2 className="mt-2 h-4 w-4 text-primary" />
                )}
              </button>
            ))}
          </div>
          <TxButton
            onClick={() => {
              if (currentEntityId)
                actions.setMinReceiverKyc(currentEntityId, selectedKyc);
            }}
            txStatus={actions.txState.status}
          >
            Set KYC Level
          </TxButton>
        </CardContent>
      </Card>

      {/* Whitelist / Blacklist Management */}
      <div className="grid gap-6 lg:grid-cols-2">
        {/* Whitelist */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <UserPlus className="h-5 w-5 text-green-600" />
              Whitelist
            </CardTitle>
            <CardDescription>
              {whitelist.length} address{whitelist.length !== 1 ? "es" : ""}{" "}
              whitelisted
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {whitelistLoading ? (
              <div className="flex justify-center py-4">
                <div className="h-5 w-5 animate-spin rounded-full border-2 border-primary border-t-transparent" />
              </div>
            ) : whitelist.length === 0 ? (
              <p className="text-center text-sm text-muted-foreground py-4">
                No addresses whitelisted
              </p>
            ) : (
              <div className="max-h-48 space-y-2 overflow-y-auto">
                {whitelist.map((entry) => (
                  <div
                    key={entry.account}
                    className="flex items-center justify-between rounded-lg border px-3 py-2"
                  >
                    <AddressDisplay address={entry.account} chars={4} />
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7 text-muted-foreground hover:text-destructive"
                      onClick={() => {
                        actions.removeFromWhitelist(currentEntityId, [
                          entry.account,
                        ]);
                        setTimeout(refetchWhitelist, 2000);
                      }}
                    >
                      <X className="h-3 w-3" />
                    </Button>
                  </div>
                ))}
              </div>
            )}
            <Separator />
            <div className="flex gap-2">
              <Input
                value={whitelistAddr}
                onChange={(e) => setWhitelistAddr(e.target.value)}
                placeholder="Add address to whitelist..."
                className="flex-1"
              />
              <TxButton
                onClick={() => {
                  if (whitelistAddr && currentEntityId) {
                    actions.addToWhitelist(currentEntityId, [whitelistAddr]);
                    setWhitelistAddr("");
                    setTimeout(refetchWhitelist, 2000);
                  }
                }}
                txStatus={actions.txState.status}
                disabled={!whitelistAddr}
              >
                <UserPlus className="mr-2 h-4 w-4" />
                Add
              </TxButton>
            </div>
          </CardContent>
        </Card>

        {/* Blacklist */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <UserMinus className="h-5 w-5 text-red-600" />
              Blacklist
            </CardTitle>
            <CardDescription>
              {blacklist.length} address{blacklist.length !== 1 ? "es" : ""}{" "}
              blacklisted
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {blacklistLoading ? (
              <div className="flex justify-center py-4">
                <div className="h-5 w-5 animate-spin rounded-full border-2 border-primary border-t-transparent" />
              </div>
            ) : blacklist.length === 0 ? (
              <p className="text-center text-sm text-muted-foreground py-4">
                No addresses blacklisted
              </p>
            ) : (
              <div className="max-h-48 space-y-2 overflow-y-auto">
                {blacklist.map((entry) => (
                  <div
                    key={entry.account}
                    className="flex items-center justify-between rounded-lg border px-3 py-2"
                  >
                    <AddressDisplay address={entry.account} chars={4} />
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7 text-muted-foreground hover:text-destructive"
                      onClick={() => {
                        actions.removeFromBlacklist(currentEntityId, [
                          entry.account,
                        ]);
                        setTimeout(refetchBlacklist, 2000);
                      }}
                    >
                      <X className="h-3 w-3" />
                    </Button>
                  </div>
                ))}
              </div>
            )}
            <Separator />
            <div className="flex gap-2">
              <Input
                value={blacklistAddr}
                onChange={(e) => setBlacklistAddr(e.target.value)}
                placeholder="Add address to blacklist..."
                className="flex-1"
              />
              <TxButton
                onClick={() => {
                  if (blacklistAddr && currentEntityId) {
                    actions.addToBlacklist(currentEntityId, [blacklistAddr]);
                    setBlacklistAddr("");
                    setTimeout(refetchBlacklist, 2000);
                  }
                }}
                txStatus={actions.txState.status}
                disabled={!blacklistAddr}
              >
                <UserMinus className="mr-2 h-4 w-4" />
                Add
              </TxButton>
            </div>
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
