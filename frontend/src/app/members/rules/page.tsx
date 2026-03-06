"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useShops } from "@/hooks/useShop";
import { useUpgradeRules, useUpgradeRuleSystem, useLevels, useMemberActions } from "@/hooks/useMember";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { TxButton } from "@/components/shared/TxButton";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { ArrowLeft, Plus, Trash2, Zap, Power, Settings, RotateCcw, ShoppingCart, DollarSign, Package, Users, UserPlus, Hash } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const TRIGGER_TYPES = [
  { key: "PurchaseProduct", label: "Purchase Product", icon: ShoppingCart, field: "product_id", fieldLabel: "Product ID", fieldType: "number" },
  { key: "TotalSpent", label: "Total Spent", icon: DollarSign, field: "threshold", fieldLabel: "Threshold (USDT)", fieldType: "number" },
  { key: "SingleOrder", label: "Single Order", icon: Package, field: "threshold", fieldLabel: "Min Order Amount", fieldType: "number" },
  { key: "ReferralCount", label: "Referral Count", icon: UserPlus, field: "count", fieldLabel: "Required Referrals", fieldType: "number" },
  { key: "TeamSize", label: "Team Size", icon: Users, field: "size", fieldLabel: "Min Team Size", fieldType: "number" },
  { key: "OrderCount", label: "Order Count", icon: Hash, field: "count", fieldLabel: "Required Orders", fieldType: "number" },
] as const;

const CONFLICT_STRATEGIES = [
  { key: "HighestLevel", label: "Highest Level", desc: "Pick the rule that results in the highest level" },
  { key: "HighestPriority", label: "Highest Priority", desc: "Pick the rule with highest priority value" },
  { key: "LongestDuration", label: "Longest Duration", desc: "Pick the rule with longest duration" },
  { key: "FirstMatch", label: "First Match", desc: "Use the first rule that matches" },
] as const;

export default function UpgradeRulesPage() {
  const { currentEntityId } = useEntityStore();
  const { shops } = useShops(currentEntityId);
  const primaryShop = shops.find((s) => s.isPrimary) || shops[0];
  const shopId = primaryShop?.id ?? null;
  const { rules, isLoading, refetch } = useUpgradeRules(currentEntityId);
  const { ruleSystem, refetch: refetchSystem } = useUpgradeRuleSystem(currentEntityId);
  const { levels } = useLevels(currentEntityId);
  const actions = useMemberActions();
  const tc = useTranslations("common");

  const [triggerKey, setTriggerKey] = useState("TotalSpent");
  const [triggerValue, setTriggerValue] = useState("");
  const [ruleName, setRuleName] = useState("");
  const [targetLevelId, setTargetLevelId] = useState("");
  const [priority, setPriority] = useState("0");
  const [stackable, setStackable] = useState(false);
  const [maxTriggers, setMaxTriggers] = useState("");
  const [duration, setDuration] = useState("");

  const [initStrategy, setInitStrategy] = useState("HighestLevel");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const selectedTrigger = TRIGGER_TYPES.find((t) => t.key === triggerKey)!;

  const buildTriggerEnum = (): Record<string, Record<string, number>> => {
    const val = Number(triggerValue || 0);
    return { [triggerKey]: { [selectedTrigger.field]: val } };
  };

  const handleAdd = async () => {
    if (!shopId || !targetLevelId || !triggerValue || !ruleName) return;
    const trigger = buildTriggerEnum();
    await actions.addUpgradeRule(
      shopId, ruleName, trigger, Number(targetLevelId),
      duration ? Number(duration) : null,
      Number(priority), stackable,
      maxTriggers ? Number(maxTriggers) : null,
    );
    setRuleName(""); setTriggerValue(""); setTargetLevelId("");
    refetch();
  };

  const handleInitSystem = async () => {
    if (!shopId) return;
    await actions.initUpgradeRuleSystem(shopId, initStrategy);
    refetchSystem();
  };

  const handleToggleRule = async (ruleId: number, currentEnabled: boolean) => {
    if (!shopId) return;
    await actions.updateUpgradeRule(shopId, ruleId, !currentEnabled, null);
    refetch();
  };

  const formatTrigger = (trigger: string | Record<string, unknown>): string => {
    if (typeof trigger === "string") return trigger;
    if (typeof trigger === "object" && trigger !== null) {
      const [key, val] = Object.entries(trigger)[0] || [];
      if (!key) return "Unknown";
      if (typeof val === "object" && val !== null) {
        const [field, fval] = Object.entries(val)[0] || [];
        return `${key} (${field}: ${fval})`;
      }
      return `${key}: ${val}`;
    }
    return String(trigger);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/members"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">Upgrade Rules</h1>
          <p className="text-muted-foreground">Automatic member level upgrade triggers</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => { refetch(); refetchSystem(); }}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {ruleSystem ? (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Settings className="h-5 w-5" />Rule System</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Status</span>
              <Badge variant={ruleSystem.enabled ? "default" : "secondary"}>{ruleSystem.enabled ? "Enabled" : "Disabled"}</Badge>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Conflict Strategy</span>
              <Badge variant="outline">{ruleSystem.conflictStrategy}</Badge>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Rules Count</span>
              <span className="text-sm font-medium">{rules.length}</span>
            </div>
            <div className="flex gap-2 pt-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => shopId && actions.setUpgradeRuleSystemEnabled(shopId, !ruleSystem.enabled)}
              >
                <Power className="mr-1 h-3 w-3" />{ruleSystem.enabled ? "Disable" : "Enable"} System
              </Button>
              <Button variant="destructive" size="sm" onClick={() => shopId && actions.resetUpgradeRuleSystem(shopId)}>
                Reset Rules
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Settings className="h-5 w-5" />Initialize Rule System</CardTitle>
            <CardDescription>Set up the upgrade rule system with a conflict resolution strategy</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid gap-2 md:grid-cols-2 lg:grid-cols-4">
              {CONFLICT_STRATEGIES.map((s) => (
                <div
                  key={s.key}
                  className={`cursor-pointer rounded-lg border-2 p-3 transition-all ${initStrategy === s.key ? "border-primary bg-primary/5" : "border-transparent hover:border-muted-foreground/25"}`}
                  onClick={() => setInitStrategy(s.key)}
                >
                  <p className="text-sm font-medium">{s.label}</p>
                  <p className="text-xs text-muted-foreground">{s.desc}</p>
                </div>
              ))}
            </div>
            <TxButton onClick={handleInitSystem} txStatus={actions.txState.status} disabled={!shopId}>
              Initialize Rule System
            </TxButton>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Plus className="h-5 w-5" />Add Upgrade Rule</CardTitle>
          <CardDescription>Define conditions that trigger automatic member upgrades</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">Rule Name</label>
            <Input value={ruleName} onChange={(e) => setRuleName(e.target.value)} placeholder="e.g. Gold Spender Rule" />
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium">Trigger Type</label>
            <div className="grid gap-2 md:grid-cols-3 lg:grid-cols-6">
              {TRIGGER_TYPES.map((t) => {
                const Icon = t.icon;
                return (
                  <div
                    key={t.key}
                    className={`cursor-pointer rounded-lg border-2 p-3 text-center transition-all ${triggerKey === t.key ? "border-primary bg-primary/5" : "border-transparent hover:border-muted-foreground/25"}`}
                    onClick={() => setTriggerKey(t.key)}
                  >
                    <Icon className="mx-auto h-5 w-5 mb-1" />
                    <p className="text-xs font-medium">{t.label}</p>
                  </div>
                );
              })}
            </div>
          </div>

          <div className="grid gap-4 md:grid-cols-3">
            <div className="space-y-2">
              <label className="text-sm font-medium">{selectedTrigger.fieldLabel}</label>
              <Input type="number" value={triggerValue} onChange={(e) => setTriggerValue(e.target.value)} placeholder="Value" min="0" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Target Level</label>
              <select
                className="w-full rounded-md border px-3 py-2 text-sm"
                value={targetLevelId}
                onChange={(e) => setTargetLevelId(e.target.value)}
              >
                <option value="">Select level</option>
                {levels.map((l) => <option key={l.id} value={l.id}>{l.name} (#{l.id})</option>)}
              </select>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Priority</label>
              <Input type="number" value={priority} onChange={(e) => setPriority(e.target.value)} min="0" max="255" />
            </div>
          </div>
          <div className="grid gap-4 md:grid-cols-3">
            <div className="space-y-2">
              <label className="text-sm font-medium">Max Triggers (optional)</label>
              <Input type="number" value={maxTriggers} onChange={(e) => setMaxTriggers(e.target.value)} placeholder="Unlimited" min="1" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Duration (blocks, optional)</label>
              <Input type="number" value={duration} onChange={(e) => setDuration(e.target.value)} placeholder="Permanent" min="1" />
            </div>
            <div className="flex items-center gap-3 pt-6">
              <Switch checked={stackable} onCheckedChange={setStackable} />
              <label className="text-sm font-medium">Stackable</label>
            </div>
          </div>
          <TxButton onClick={handleAdd} txStatus={actions.txState.status} disabled={!targetLevelId || !triggerValue || !shopId || !ruleName}>
            <Plus className="mr-2 h-4 w-4" />Add Rule
          </TxButton>
        </CardContent>
      </Card>

      {isLoading ? (
        <div className="flex justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
      ) : rules.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Zap className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No Upgrade Rules</p>
            <p className="text-sm text-muted-foreground">Add rules above to enable automatic member upgrades.</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader><CardTitle className="flex items-center gap-2"><Zap className="h-5 w-5" />Active Rules ({rules.length})</CardTitle></CardHeader>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>ID</TableHead>
                <TableHead>Trigger</TableHead>
                <TableHead>Target Level</TableHead>
                <TableHead className="text-right">Priority</TableHead>
                <TableHead className="text-center">Enabled</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {rules.map((rule) => (
                <TableRow key={rule.id}>
                  <TableCell className="font-mono">{rule.id}</TableCell>
                  <TableCell><Badge variant="outline">{formatTrigger(rule.trigger as unknown as string | Record<string, unknown>)}</Badge></TableCell>
                  <TableCell>{levels.find((l) => l.id === rule.targetLevelId)?.name || `#${rule.targetLevelId}`}</TableCell>
                  <TableCell className="text-right">{rule.priority}</TableCell>
                  <TableCell className="text-center">
                    <Switch
                      checked={rule.enabled}
                      onCheckedChange={() => handleToggleRule(rule.id, rule.enabled)}
                    />
                  </TableCell>
                  <TableCell className="text-right">
                    <Button variant="ghost" size="icon" onClick={() => shopId && actions.removeUpgradeRule(shopId, rule.id)}>
                      <Trash2 className="h-4 w-4 text-destructive" />
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
