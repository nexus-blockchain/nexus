"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useToken, useTokenActions } from "@/hooks/useToken";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { TxButton } from "@/components/shared/TxButton";
import { Separator } from "@/components/ui/separator";
import { TRANSFER_RESTRICTION_MODES } from "@/lib/constants";
import { ArrowLeft, Shield, UserPlus, UserMinus, ShieldCheck } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function TransferRestrictionPage() {
  const { currentEntityId } = useEntityStore();
  const { config, isLoading } = useToken(currentEntityId);
  const actions = useTokenActions();
  const tc = useTranslations("common");

  const [mode, setMode] = useState("None");
  const [whitelistAddr, setWhitelistAddr] = useState("");
  const [blacklistAddr, setBlacklistAddr] = useState("");
  const [removeAddr, setRemoveAddr] = useState("");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  if (isLoading) {
    return <div className="flex h-full items-center justify-center"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>;
  }

  const handleSetMode = () => {
    actions.setTransferRestriction(currentEntityId, mode);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/token/config"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Transfer Restrictions</h1>
          <p className="text-muted-foreground">Control who can transfer entity tokens</p>
        </div>
      </div>

      {config && (
        <Card>
          <CardHeader><CardTitle>Current Status</CardTitle></CardHeader>
          <CardContent className="space-y-3">
            <div className="flex justify-between items-center">
              <span className="text-sm text-muted-foreground">Restriction Mode</span>
              <Badge>{config.transferRestriction || "None"}</Badge>
            </div>
            <Separator />
            <div className="flex justify-between items-center">
              <span className="text-sm text-muted-foreground">Transferable</span>
              <Badge variant={config.transferable ? "default" : "secondary"}>{config.transferable ? "Yes" : "No"}</Badge>
            </div>
            <Separator />
            <div className="flex justify-between items-center">
              <span className="text-sm text-muted-foreground">Min KYC Level</span>
              <span className="text-sm">{config.minReceiverKyc}</span>
            </div>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Shield className="h-5 w-5" />Set Restriction Mode</CardTitle>
          <CardDescription>Choose how token transfers are restricted</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <Select value={mode} onChange={(e) => setMode(e.target.value)}>
            {TRANSFER_RESTRICTION_MODES.map((m) => (
              <option key={m} value={m}>{m}</option>
            ))}
          </Select>
          <div className="rounded-lg border p-3 space-y-2 text-sm">
            <p className="font-medium">Mode Descriptions:</p>
            <p><strong>None</strong> — No restrictions, anyone can transfer</p>
            <p><strong>Whitelist</strong> — Only whitelisted addresses can receive</p>
            <p><strong>Blacklist</strong> — Blacklisted addresses cannot receive</p>
            <p><strong>KycRequired</strong> — Receiver must have KYC above minimum level</p>
            <p><strong>MembersOnly</strong> — Only entity members can transfer</p>
          </div>
          <TxButton onClick={handleSetMode} txStatus={actions.txState.status}>
            <ShieldCheck className="mr-2 h-4 w-4" />Set Mode
          </TxButton>
        </CardContent>
      </Card>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><UserPlus className="h-5 w-5" />Whitelist</CardTitle>
            <CardDescription>Add addresses to the transfer whitelist</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Address</label>
              <Input value={whitelistAddr} onChange={(e) => setWhitelistAddr(e.target.value)} placeholder="5xxx..." />
            </div>
            <div className="flex gap-2">
              <TxButton
                onClick={() => { actions.addToWhitelist(currentEntityId, [whitelistAddr]); setWhitelistAddr(""); }}
                txStatus={actions.txState.status}
                disabled={!whitelistAddr}
                className="flex-1"
              >
                <UserPlus className="mr-2 h-4 w-4" />Add
              </TxButton>
              <div className="flex-1 space-y-2">
                <Input value={removeAddr} onChange={(e) => setRemoveAddr(e.target.value)} placeholder="Remove address" />
                <Button
                  variant="outline"
                  className="w-full"
                  onClick={() => { actions.removeFromWhitelist(currentEntityId, [removeAddr]); setRemoveAddr(""); }}
                  disabled={!removeAddr}
                >
                  <UserMinus className="mr-2 h-4 w-4" />Remove
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><UserMinus className="h-5 w-5" />Blacklist</CardTitle>
            <CardDescription>Block addresses from receiving tokens</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Address</label>
              <Input value={blacklistAddr} onChange={(e) => setBlacklistAddr(e.target.value)} placeholder="5xxx..." />
            </div>
            <div className="flex gap-2">
              <TxButton
                onClick={() => { actions.addToBlacklist(currentEntityId, [blacklistAddr]); setBlacklistAddr(""); }}
                txStatus={actions.txState.status}
                disabled={!blacklistAddr}
                className="flex-1"
              >
                Add to Blacklist
              </TxButton>
              <Button
                variant="outline"
                className="flex-1"
                onClick={() => { actions.removeFromBlacklist(currentEntityId, [blacklistAddr]); }}
                disabled={!blacklistAddr}
              >
                Remove from Blacklist
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
