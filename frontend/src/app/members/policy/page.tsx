"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useShops } from "@/hooks/useShop";
import { useMemberActions } from "@/hooks/useMember";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Select } from "@/components/ui/select";
import { TxButton } from "@/components/shared/TxButton";
import { ArrowLeft, Shield, Settings } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const POLICY_FLAGS = [
  { bit: 0x01, label: "Open Registration", desc: "Anyone can register as a member" },
  { bit: 0x02, label: "Require Approval", desc: "New members need admin approval" },
  { bit: 0x04, label: "Require Referrer", desc: "Must have a referrer to join" },
  { bit: 0x08, label: "Require KYC", desc: "KYC verification required for membership" },
  { bit: 0x10, label: "Auto Level Up", desc: "Members automatically upgrade when meeting criteria" },
  { bit: 0x20, label: "Spillover Enabled", desc: "Referral tree spillover placement" },
] as const;

const UPGRADE_MODES = ["Manual", "Automatic", "Hybrid"] as const;

export default function MemberPolicyPage() {
  const { currentEntityId } = useEntityStore();
  const { shops } = useShops(currentEntityId);
  const primaryShop = shops.find((s) => s.isPrimary) || shops[0];
  const shopId = primaryShop?.id ?? null;
  const actions = useMemberActions();
  const tc = useTranslations("common");

  const [policyBits, setPolicyBits] = useState(0x01);
  const [upgradeMode, setUpgradeMode] = useState("Automatic");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const toggleBit = (bit: number) => {
    setPolicyBits((prev) => prev ^ bit);
  };

  const handleSavePolicy = () => {
    if (shopId) actions.setMemberPolicy(shopId, policyBits);
  };

  const handleSetUpgradeMode = () => {
    if (shopId) actions.setUpgradeMode(shopId, upgradeMode);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/members"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Member Policy</h1>
          <p className="text-muted-foreground">Configure registration and membership policies</p>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Shield className="h-5 w-5" />Registration Policy</CardTitle>
          <CardDescription>Control how new members can join your entity</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {POLICY_FLAGS.map((flag) => (
            <div key={flag.bit} className="flex items-center justify-between rounded-lg border p-3">
              <div>
                <p className="text-sm font-medium">{flag.label}</p>
                <p className="text-xs text-muted-foreground">{flag.desc}</p>
              </div>
              <Switch
                checked={!!(policyBits & flag.bit)}
                onCheckedChange={() => toggleBit(flag.bit)}
              />
            </div>
          ))}
          <div className="rounded-lg border bg-muted/50 p-3">
            <p className="text-xs text-muted-foreground">Policy bits: <span className="font-mono">0x{policyBits.toString(16).padStart(2, "0")}</span> ({policyBits})</p>
          </div>
          <TxButton onClick={handleSavePolicy} txStatus={actions.txState.status} disabled={!shopId}>
            Save Registration Policy
          </TxButton>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Settings className="h-5 w-5" />Upgrade Mode</CardTitle>
          <CardDescription>How member level upgrades are processed</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <Select value={upgradeMode} onChange={(e) => setUpgradeMode(e.target.value)}>
            {UPGRADE_MODES.map((m) => <option key={m} value={m}>{m}</option>)}
          </Select>
          <div className="rounded-lg border p-3 space-y-2 text-sm">
            <p><strong>Manual</strong> — Admin manually triggers upgrades</p>
            <p><strong>Automatic</strong> — Members upgrade when rule thresholds are met</p>
            <p><strong>Hybrid</strong> — Automatic upgrades with admin override capability</p>
          </div>
          <TxButton onClick={handleSetUpgradeMode} txStatus={actions.txState.status} disabled={!shopId}>
            Set Upgrade Mode
          </TxButton>
        </CardContent>
      </Card>

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Policy saved!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
