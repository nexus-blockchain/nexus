"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useShops } from "@/hooks/useShop";
import { useLevels, useLevelSystem, useMemberActions } from "@/hooks/useMember";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { TxButton } from "@/components/shared/TxButton";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { basisPointsToPercent } from "@/lib/utils";
import { ArrowLeft, Plus, Trash2, Layers, Edit, Save, X, Settings, RotateCcw } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function LevelsPage() {
  const { currentEntityId } = useEntityStore();
  const { shops } = useShops(currentEntityId);
  const primaryShop = shops.find((s) => s.isPrimary) || shops[0];
  const shopId = primaryShop?.id ?? null;
  const { levels, isLoading, refetch } = useLevels(currentEntityId);
  const { system, refetch: refetchSystem } = useLevelSystem(currentEntityId);
  const actions = useMemberActions();
  const tc = useTranslations("common");

  const [name, setName] = useState("");
  const [threshold, setThreshold] = useState("");
  const [discountRate, setDiscountRate] = useState("");
  const [commissionBonus, setCommissionBonus] = useState("");

  const [editingId, setEditingId] = useState<number | null>(null);
  const [editName, setEditName] = useState("");
  const [editThreshold, setEditThreshold] = useState("");
  const [editDiscount, setEditDiscount] = useState("");
  const [editBonus, setEditBonus] = useState("");

  const [initCustom, setInitCustom] = useState(true);
  const [initMode, setInitMode] = useState("AutoUpgrade");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const handleAdd = async () => {
    if (!shopId || !name) return;
    await actions.addCustomLevel(shopId, name, Number(threshold || 0), Number(discountRate || 0), Number(commissionBonus || 0));
    setName(""); setThreshold(""); setDiscountRate(""); setCommissionBonus("");
    refetch();
  };

  const startEdit = (level: typeof levels[0]) => {
    setEditingId(level.id);
    setEditName(level.name);
    setEditThreshold(String(level.threshold));
    setEditDiscount(String(level.discountRate));
    setEditBonus(String(level.commissionBonus));
  };

  const handleUpdate = async () => {
    if (!shopId || editingId === null) return;
    const original = levels.find((l) => l.id === editingId);
    if (!original) return;
    await actions.updateCustomLevel(
      shopId,
      editingId,
      editName !== original.name ? editName : null,
      editThreshold !== String(original.threshold) ? Number(editThreshold) : null,
      editDiscount !== String(original.discountRate) ? Number(editDiscount) : null,
      editBonus !== String(original.commissionBonus) ? Number(editBonus) : null,
    );
    setEditingId(null);
    refetch();
  };

  const handleInitSystem = async () => {
    if (!shopId) return;
    await actions.initLevelSystem(shopId, initCustom, initMode);
    refetchSystem();
  };

  const handleResetSystem = async () => {
    if (!shopId) return;
    await actions.resetLevelSystem(shopId);
    refetchSystem();
    refetch();
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/members"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">Level Management</h1>
          <p className="text-muted-foreground">Configure member levels and their benefits</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => { refetch(); refetchSystem(); }}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {system ? (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Settings className="h-5 w-5" />System Status</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Custom Levels</span>
              <Badge variant={system.useCustom ? "default" : "secondary"}>{system.useCustom ? "Enabled" : "Disabled"}</Badge>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Upgrade Mode</span>
              <Badge variant="outline">{system.upgradeMode}</Badge>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Configured Levels</span>
              <span className="text-sm font-medium">{levels.length}</span>
            </div>
            <div className="flex gap-2 pt-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => shopId && actions.setUseCustomLevels(shopId, !system.useCustom)}
              >
                {system.useCustom ? "Disable Custom" : "Enable Custom"}
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => shopId && actions.setUpgradeMode(shopId, system.upgradeMode === "AutoUpgrade" ? "ManualUpgrade" : "AutoUpgrade")}
              >
                Switch to {system.upgradeMode === "AutoUpgrade" ? "Manual" : "Auto"}
              </Button>
              <Button variant="destructive" size="sm" onClick={handleResetSystem}>
                Reset System
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Settings className="h-5 w-5" />Initialize Level System</CardTitle>
            <CardDescription>Set up the member level system before adding levels</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between rounded-lg border p-3">
              <div>
                <p className="text-sm font-medium">Use Custom Levels</p>
                <p className="text-xs text-muted-foreground">Enable custom level definitions with thresholds</p>
              </div>
              <Switch checked={initCustom} onCheckedChange={setInitCustom} />
            </div>
            <div className="grid gap-2 md:grid-cols-2">
              {(["AutoUpgrade", "ManualUpgrade"] as const).map((mode) => (
                <div
                  key={mode}
                  className={`cursor-pointer rounded-lg border-2 p-4 transition-all ${initMode === mode ? "border-primary bg-primary/5" : "border-transparent hover:border-muted-foreground/25"}`}
                  onClick={() => setInitMode(mode)}
                >
                  <p className="text-sm font-medium">{mode === "AutoUpgrade" ? "Automatic" : "Manual"}</p>
                  <p className="text-xs text-muted-foreground">
                    {mode === "AutoUpgrade" ? "Members upgrade when thresholds are met" : "Admin manually approves upgrades"}
                  </p>
                </div>
              ))}
            </div>
            <TxButton onClick={handleInitSystem} txStatus={actions.txState.status} disabled={!shopId}>
              Initialize Level System
            </TxButton>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Plus className="h-5 w-5" />Add New Level</CardTitle>
          <CardDescription>Create a custom member level with thresholds and benefits</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 md:grid-cols-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Level Name</label>
              <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Gold" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Threshold</label>
              <Input type="number" value={threshold} onChange={(e) => setThreshold(e.target.value)} placeholder="Min spend" min="0" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Discount (bps)</label>
              <Input type="number" value={discountRate} onChange={(e) => setDiscountRate(e.target.value)} placeholder="0" min="0" max="10000" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Commission Bonus (bps)</label>
              <Input type="number" value={commissionBonus} onChange={(e) => setCommissionBonus(e.target.value)} placeholder="0" min="0" />
            </div>
          </div>
          <TxButton onClick={handleAdd} txStatus={actions.txState.status} disabled={!name.trim() || !shopId}>
            <Plus className="mr-2 h-4 w-4" />Add Level
          </TxButton>
        </CardContent>
      </Card>

      {isLoading ? (
        <div className="flex justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
      ) : levels.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Layers className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No Levels Configured</p>
            <p className="text-sm text-muted-foreground">Add your first member level above.</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader><CardTitle className="flex items-center gap-2"><Layers className="h-5 w-5" />Current Levels ({levels.length})</CardTitle></CardHeader>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>ID</TableHead>
                <TableHead>Name</TableHead>
                <TableHead className="text-right">Threshold</TableHead>
                <TableHead className="text-right">Discount</TableHead>
                <TableHead className="text-right">Comm. Bonus</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {levels.map((level) => (
                <TableRow key={level.id}>
                  {editingId === level.id ? (
                    <>
                      <TableCell className="font-mono">{level.id}</TableCell>
                      <TableCell><Input value={editName} onChange={(e) => setEditName(e.target.value)} className="h-8" /></TableCell>
                      <TableCell><Input type="number" value={editThreshold} onChange={(e) => setEditThreshold(e.target.value)} className="h-8 text-right" /></TableCell>
                      <TableCell><Input type="number" value={editDiscount} onChange={(e) => setEditDiscount(e.target.value)} className="h-8 text-right" /></TableCell>
                      <TableCell><Input type="number" value={editBonus} onChange={(e) => setEditBonus(e.target.value)} className="h-8 text-right" /></TableCell>
                      <TableCell className="text-right">
                        <div className="flex justify-end gap-1">
                          <Button variant="ghost" size="icon" onClick={handleUpdate}><Save className="h-4 w-4 text-green-600" /></Button>
                          <Button variant="ghost" size="icon" onClick={() => setEditingId(null)}><X className="h-4 w-4" /></Button>
                        </div>
                      </TableCell>
                    </>
                  ) : (
                    <>
                      <TableCell className="font-mono">{level.id}</TableCell>
                      <TableCell className="font-medium">{level.name}</TableCell>
                      <TableCell className="text-right">{level.threshold.toLocaleString()}</TableCell>
                      <TableCell className="text-right">{basisPointsToPercent(level.discountRate)}</TableCell>
                      <TableCell className="text-right">{basisPointsToPercent(level.commissionBonus)}</TableCell>
                      <TableCell className="text-right">
                        <div className="flex justify-end gap-1">
                          <Button variant="ghost" size="icon" onClick={() => startEdit(level)}><Edit className="h-4 w-4" /></Button>
                          <Button variant="ghost" size="icon" onClick={() => shopId && actions.removeCustomLevel(shopId, level.id)}>
                            <Trash2 className="h-4 w-4 text-destructive" />
                          </Button>
                        </div>
                      </TableCell>
                    </>
                  )}
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
