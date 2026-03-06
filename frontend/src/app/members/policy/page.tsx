"use client";

import { useState, useEffect } from "react";
import { useEntityStore } from "@/stores/entity";
import { useShops } from "@/hooks/useShop";
import { useMemberActions, useMemberPolicy } from "@/hooks/useMember";
import { MemberRegistrationPolicy } from "@/lib/constants";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { TxButton } from "@/components/shared/TxButton";
import { ArrowLeft, Shield, Settings, RotateCcw, BarChart3 } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const POLICY_FLAGS = [
  { bit: MemberRegistrationPolicy.PURCHASE_REQUIRED, label: "Purchase Required", desc: "Must make a purchase before becoming a member" },
  { bit: MemberRegistrationPolicy.REFERRAL_REQUIRED, label: "Referral Required", desc: "Must have a referrer to register as a member" },
  { bit: MemberRegistrationPolicy.APPROVAL_REQUIRED, label: "Approval Required", desc: "Admin must approve each new member application" },
  { bit: MemberRegistrationPolicy.KYC_REQUIRED, label: "KYC Required", desc: "Must complete KYC verification to register" },
  { bit: MemberRegistrationPolicy.KYC_UPGRADE_REQUIRED, label: "KYC Upgrade Required", desc: "Must upgrade KYC level for higher member tiers" },
] as const;

const STATS_POLICY_FLAGS = [
  { bit: 0x01, label: "Include Repurchase (Direct)", desc: "Count direct repurchase orders in member stats" },
  { bit: 0x02, label: "Include Repurchase (Indirect)", desc: "Count indirect repurchase orders in member stats" },
] as const;

const UPGRADE_MODES = [
  { key: "AutoUpgrade", label: "Automatic", desc: "Members upgrade automatically when rule thresholds are met" },
  { key: "ManualUpgrade", label: "Manual", desc: "Admin must manually approve and trigger level upgrades" },
] as const;

export default function MemberPolicyPage() {
  const { currentEntityId } = useEntityStore();
  const { shops } = useShops(currentEntityId);
  const primaryShop = shops.find((s) => s.isPrimary) || shops[0];
  const shopId = primaryShop?.id ?? null;
  const actions = useMemberActions();
  const { policyBits: chainPolicy, statsPolicyBits: chainStatsPolicy, isLoading, refetch } = useMemberPolicy(currentEntityId);
  const tc = useTranslations("common");

  const [policyBits, setPolicyBits] = useState(0);
  const [statsPolicyBits, setStatsPolicyBits] = useState(0);
  const [upgradeMode, setUpgradeMode] = useState("AutoUpgrade");
  const [initialized, setInitialized] = useState(false);

  useEffect(() => {
    if (!initialized && chainPolicy !== null) {
      setPolicyBits(chainPolicy);
      setInitialized(true);
    }
  }, [chainPolicy, initialized]);

  useEffect(() => {
    if (chainStatsPolicy !== null) {
      setStatsPolicyBits(chainStatsPolicy);
    }
  }, [chainStatsPolicy]);

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const toggleBit = (bit: number) => {
    setPolicyBits((prev) => prev ^ bit);
  };

  const toggleStatsBit = (bit: number) => {
    setStatsPolicyBits((prev) => prev ^ bit);
  };

  const handleSavePolicy = async () => {
    if (shopId) {
      await actions.setMemberPolicy(shopId, policyBits);
      refetch();
    }
  };

  const handleSaveStatsPolicy = async () => {
    if (shopId) {
      await actions.setMemberStatsPolicy(shopId, statsPolicyBits);
      refetch();
    }
  };

  const handleSetUpgradeMode = () => {
    if (shopId) actions.setUpgradeMode(shopId, upgradeMode);
  };

  const isOpen = policyBits === MemberRegistrationPolicy.OPEN;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/members"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">Member Policy</h1>
          <p className="text-muted-foreground">Configure registration and membership policies</p>
        </div>
        <Button variant="outline" size="sm" onClick={refetch}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
      ) : (
        <>
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2"><Shield className="h-5 w-5" />Registration Policy</CardTitle>
              <CardDescription>
                Control how new members can join. When no flags are set, registration is open to everyone.
                {chainPolicy !== null && (
                  <span className="ml-2 text-xs font-mono">
                    Chain: 0x{chainPolicy.toString(16).padStart(2, "0")} | Local: 0x{policyBits.toString(16).padStart(2, "0")}
                  </span>
                )}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className={`rounded-lg border-2 p-4 transition-all ${isOpen ? "border-green-500/50 bg-green-50 dark:bg-green-950/20" : "border-transparent"}`}>
                <div className="flex items-center justify-between">
                  <div>
                    <p className="text-sm font-medium">Open Registration</p>
                    <p className="text-xs text-muted-foreground">Anyone can register without restrictions (no policy bits set)</p>
                  </div>
                  <Badge variant={isOpen ? "default" : "secondary"}>{isOpen ? "Active" : "Inactive"}</Badge>
                </div>
              </div>

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
                <p className="text-xs text-muted-foreground">
                  Policy bits: <span className="font-mono">0x{policyBits.toString(16).padStart(2, "0")}</span> ({policyBits})
                  {chainPolicy !== null && policyBits !== chainPolicy && (
                    <span className="ml-2 text-amber-600">(unsaved changes)</span>
                  )}
                </p>
              </div>
              <TxButton onClick={handleSavePolicy} txStatus={actions.txState.status} disabled={!shopId}>
                Save Registration Policy
              </TxButton>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2"><BarChart3 className="h-5 w-5" />Member Stats Policy</CardTitle>
              <CardDescription>Control what counts towards member statistics</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {STATS_POLICY_FLAGS.map((flag) => (
                <div key={flag.bit} className="flex items-center justify-between rounded-lg border p-3">
                  <div>
                    <p className="text-sm font-medium">{flag.label}</p>
                    <p className="text-xs text-muted-foreground">{flag.desc}</p>
                  </div>
                  <Switch
                    checked={!!(statsPolicyBits & flag.bit)}
                    onCheckedChange={() => toggleStatsBit(flag.bit)}
                  />
                </div>
              ))}
              <div className="rounded-lg border bg-muted/50 p-3">
                <p className="text-xs text-muted-foreground">
                  Stats policy bits: <span className="font-mono">0x{statsPolicyBits.toString(16).padStart(2, "0")}</span>
                </p>
              </div>
              <TxButton onClick={handleSaveStatsPolicy} txStatus={actions.txState.status} disabled={!shopId}>
                Save Stats Policy
              </TxButton>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2"><Settings className="h-5 w-5" />Upgrade Mode</CardTitle>
              <CardDescription>How member level upgrades are processed</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid gap-3 md:grid-cols-2">
                {UPGRADE_MODES.map((mode) => (
                  <div
                    key={mode.key}
                    className={`cursor-pointer rounded-lg border-2 p-4 transition-all ${upgradeMode === mode.key ? "border-primary bg-primary/5" : "border-transparent hover:border-muted-foreground/25"}`}
                    onClick={() => setUpgradeMode(mode.key)}
                  >
                    <p className="text-sm font-medium">{mode.label}</p>
                    <p className="text-xs text-muted-foreground">{mode.desc}</p>
                  </div>
                ))}
              </div>
              <TxButton onClick={handleSetUpgradeMode} txStatus={actions.txState.status} disabled={!shopId}>
                Set Upgrade Mode
              </TxButton>
            </CardContent>
          </Card>
        </>
      )}

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Policy saved!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
