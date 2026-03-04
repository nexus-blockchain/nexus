"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useCommissionConfig, useCommissionActions } from "@/hooks/useCommission";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { TxButton } from "@/components/shared/TxButton";
import { Separator } from "@/components/ui/separator";
import { ArrowLeft, Settings, GitBranch, Layers, TrendingUp, Gift } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";
import { basisPointsToPercent } from "@/lib/utils";

export default function CommissionConfigPage() {
  const { currentEntityId } = useEntityStore();
  const { config, isLoading } = useCommissionConfig(currentEntityId);
  const actions = useCommissionActions();
  const tc = useTranslations("common");

  const [referralEnabled, setReferralEnabled] = useState(false);
  const [levelDiffEnabled, setLevelDiffEnabled] = useState(false);
  const [singleLineEnabled, setSingleLineEnabled] = useState(false);
  const [poolRewardEnabled, setPoolRewardEnabled] = useState(false);
  const [referralRate, setReferralRate] = useState("500");
  const [maxDepth, setMaxDepth] = useState("5");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const handleSavePlugins = () => {
    actions.configureCommission(currentEntityId, referralEnabled, levelDiffEnabled, singleLineEnabled, poolRewardEnabled);
  };

  const handleSaveReferral = () => {
    actions.setReferralRate(currentEntityId, Number(referralRate), Number(maxDepth));
  };

  const plugins = [
    { key: "referral", label: "Referral Commission", icon: GitBranch, desc: "Multi-level referral chain rewards", enabled: referralEnabled, toggle: setReferralEnabled },
    { key: "levelDiff", label: "Level Difference", icon: Layers, desc: "Bonus based on level gap between referrer and member", enabled: levelDiffEnabled, toggle: setLevelDiffEnabled },
    { key: "singleLine", label: "Single Line", icon: TrendingUp, desc: "Upline/downline linear commission", enabled: singleLineEnabled, toggle: setSingleLineEnabled },
    { key: "poolReward", label: "Pool Reward", icon: Gift, desc: "Periodic pool distribution to top performers", enabled: poolRewardEnabled, toggle: setPoolRewardEnabled },
  ];

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/commission"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Commission Configuration</h1>
          <p className="text-muted-foreground">Enable and configure commission plugins</p>
        </div>
      </div>

      {config && (
        <Card>
          <CardHeader><CardTitle>Current Status</CardTitle></CardHeader>
          <CardContent className="space-y-3">
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Referral</span><Badge variant={config.referralEnabled ? "default" : "secondary"}>{config.referralEnabled ? "On" : "Off"}</Badge></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Level Diff</span><Badge variant={config.levelDiffEnabled ? "default" : "secondary"}>{config.levelDiffEnabled ? "On" : "Off"}</Badge></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Single Line</span><Badge variant={config.singleLineEnabled ? "default" : "secondary"}>{config.singleLineEnabled ? "On" : "Off"}</Badge></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Pool Reward</span><Badge variant={config.poolRewardEnabled ? "default" : "secondary"}>{config.poolRewardEnabled ? "On" : "Off"}</Badge></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Referral Rate</span><span className="text-sm">{basisPointsToPercent(config.referralRate)}</span></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Max Depth</span><span className="text-sm">{config.maxDepth}</span></div>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Settings className="h-5 w-5" />Commission Plugins</CardTitle>
          <CardDescription>Enable or disable commission calculation modules</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {plugins.map((plugin) => {
            const Icon = plugin.icon;
            return (
              <div key={plugin.key} className="flex items-center justify-between rounded-lg border p-4">
                <div className="flex items-center gap-3">
                  <Icon className="h-5 w-5 text-muted-foreground" />
                  <div>
                    <p className="text-sm font-medium">{plugin.label}</p>
                    <p className="text-xs text-muted-foreground">{plugin.desc}</p>
                  </div>
                </div>
                <Switch checked={plugin.enabled} onCheckedChange={plugin.toggle} />
              </div>
            );
          })}
          <TxButton onClick={handleSavePlugins} txStatus={actions.txState.status}>
            Save Plugin Configuration
          </TxButton>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><GitBranch className="h-5 w-5" />Referral Settings</CardTitle>
          <CardDescription>Configure referral commission rate and depth</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 md:grid-cols-2">
            <div className="space-y-2">
              <label className="text-sm font-medium">Referral Rate (basis points)</label>
              <Input type="number" value={referralRate} onChange={(e) => setReferralRate(e.target.value)} min="0" max="10000" />
              <p className="text-xs text-muted-foreground">{basisPointsToPercent(Number(referralRate))} per level</p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Max Depth</label>
              <Input type="number" value={maxDepth} onChange={(e) => setMaxDepth(e.target.value)} min="1" max="20" />
              <p className="text-xs text-muted-foreground">How many levels up the referral chain</p>
            </div>
          </div>
          <TxButton onClick={handleSaveReferral} txStatus={actions.txState.status}>
            Save Referral Settings
          </TxButton>
        </CardContent>
      </Card>

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Configuration saved!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
