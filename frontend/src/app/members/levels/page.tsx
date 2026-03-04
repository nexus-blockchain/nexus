"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useShops } from "@/hooks/useShop";
import { useLevels, useMemberActions } from "@/hooks/useMember";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { TxButton } from "@/components/shared/TxButton";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { basisPointsToPercent } from "@/lib/utils";
import { ArrowLeft, Plus, Trash2, Layers, Edit } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function LevelsPage() {
  const { currentEntityId } = useEntityStore();
  const { shops } = useShops(currentEntityId);
  const primaryShop = shops.find((s) => s.isPrimary) || shops[0];
  const shopId = primaryShop?.id ?? null;
  const { levels, isLoading, refetch } = useLevels(shopId);
  const actions = useMemberActions();
  const tc = useTranslations("common");

  const [name, setName] = useState("");
  const [threshold, setThreshold] = useState("");
  const [discountRate, setDiscountRate] = useState("");
  const [commissionBonus, setCommissionBonus] = useState("");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const handleAdd = async () => {
    if (!shopId || !name) return;
    await actions.addCustomLevel(shopId, name, Number(threshold || 0), Number(discountRate || 0), Number(commissionBonus || 0));
    setName(""); setThreshold(""); setDiscountRate(""); setCommissionBonus("");
    refetch();
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/members"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Level Management</h1>
          <p className="text-muted-foreground">Configure member levels and their benefits</p>
        </div>
      </div>

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
          <CardHeader><CardTitle className="flex items-center gap-2"><Layers className="h-5 w-5" />Current Levels</CardTitle></CardHeader>
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
                  <TableCell className="font-mono">{level.id}</TableCell>
                  <TableCell className="font-medium">{level.name}</TableCell>
                  <TableCell className="text-right">{level.threshold.toLocaleString()}</TableCell>
                  <TableCell className="text-right">{basisPointsToPercent(level.discountRate)}</TableCell>
                  <TableCell className="text-right">{basisPointsToPercent(level.commissionBonus)}</TableCell>
                  <TableCell className="text-right">
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => shopId && actions.removeCustomLevel(shopId, level.id)}
                    >
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
