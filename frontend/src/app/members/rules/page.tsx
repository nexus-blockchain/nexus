"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useShops } from "@/hooks/useShop";
import { useUpgradeRules, useLevels, useMemberActions } from "@/hooks/useMember";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { TxButton } from "@/components/shared/TxButton";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { ArrowLeft, Plus, Trash2, Zap } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const TRIGGER_TYPES = [
  "TotalSpent",
  "OrderCount",
  "ReferralCount",
  "TokenBalance",
  "PointBalance",
  "ConsecutiveOrders",
  "MonthlySpend",
  "ReviewCount",
  "Manual",
] as const;

export default function UpgradeRulesPage() {
  const { currentEntityId } = useEntityStore();
  const { shops } = useShops(currentEntityId);
  const primaryShop = shops.find((s) => s.isPrimary) || shops[0];
  const shopId = primaryShop?.id ?? null;
  const { rules, isLoading, refetch } = useUpgradeRules(shopId);
  const { levels } = useLevels(shopId);
  const actions = useMemberActions();
  const tc = useTranslations("common");

  const [trigger, setTrigger] = useState<string>("TotalSpent");
  const [targetLevelId, setTargetLevelId] = useState("");
  const [threshold, setThreshold] = useState("");
  const [priority, setPriority] = useState("0");
  const [stackable, setStackable] = useState(false);
  const [maxTriggers, setMaxTriggers] = useState("1");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const handleAdd = async () => {
    if (!shopId || !targetLevelId || !threshold) return;
    await actions.addUpgradeRule(
      shopId, trigger, Number(targetLevelId), BigInt(threshold),
      Number(priority), stackable, Number(maxTriggers)
    );
    refetch();
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/members"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Upgrade Rules</h1>
          <p className="text-muted-foreground">Automatic member level upgrade triggers</p>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Plus className="h-5 w-5" />Add Upgrade Rule</CardTitle>
          <CardDescription>Define conditions that trigger automatic member upgrades</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 md:grid-cols-3">
            <div className="space-y-2">
              <label className="text-sm font-medium">Trigger Type</label>
              <Select value={trigger} onChange={(e) => setTrigger(e.target.value)}>
                {TRIGGER_TYPES.map((t) => <option key={t} value={t}>{t}</option>)}
              </Select>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Target Level</label>
              <Select value={targetLevelId} onChange={(e) => setTargetLevelId(e.target.value)}>
                <option value="">Select level</option>
                {levels.map((l) => <option key={l.id} value={l.id}>{l.name} (#{l.id})</option>)}
              </Select>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Threshold</label>
              <Input type="number" value={threshold} onChange={(e) => setThreshold(e.target.value)} placeholder="Value" min="0" />
            </div>
          </div>
          <div className="grid gap-4 md:grid-cols-3">
            <div className="space-y-2">
              <label className="text-sm font-medium">Priority</label>
              <Input type="number" value={priority} onChange={(e) => setPriority(e.target.value)} min="0" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Max Triggers</label>
              <Input type="number" value={maxTriggers} onChange={(e) => setMaxTriggers(e.target.value)} min="1" />
            </div>
            <div className="flex items-center gap-3 pt-6">
              <Switch checked={stackable} onCheckedChange={setStackable} />
              <label className="text-sm font-medium">Stackable</label>
            </div>
          </div>
          <TxButton onClick={handleAdd} txStatus={actions.txState.status} disabled={!targetLevelId || !threshold || !shopId}>
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
          <CardHeader><CardTitle className="flex items-center gap-2"><Zap className="h-5 w-5" />Active Rules</CardTitle></CardHeader>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>ID</TableHead>
                <TableHead>Trigger</TableHead>
                <TableHead>Target Level</TableHead>
                <TableHead className="text-right">Threshold</TableHead>
                <TableHead className="text-right">Priority</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {rules.map((rule) => (
                <TableRow key={rule.id}>
                  <TableCell className="font-mono">{rule.id}</TableCell>
                  <TableCell><Badge variant="outline">{rule.trigger}</Badge></TableCell>
                  <TableCell>{levels.find((l) => l.id === rule.targetLevelId)?.name || `#${rule.targetLevelId}`}</TableCell>
                  <TableCell className="text-right font-mono">{rule.threshold.toString()}</TableCell>
                  <TableCell className="text-right">{rule.priority}</TableCell>
                  <TableCell>
                    <Badge variant={rule.enabled ? "default" : "secondary"}>
                      {rule.enabled ? "Enabled" : "Disabled"}
                    </Badge>
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
