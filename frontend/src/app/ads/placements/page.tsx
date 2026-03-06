"use client";

import { useState, useEffect } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from "@/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { TxButton } from "@/components/shared/TxButton";
import {
  LayoutGrid,
  ArrowLeft,
  Plus,
  RotateCcw,
  Power,
  PowerOff,
  Eye,
  MousePointerClick,
  Trash2,
} from "lucide-react";
import Link from "next/link";
import {
  usePlacements,
  usePlacementActions,
} from "@/hooks/useAdCampaign";
import type { AdPlacement } from "@/lib/types";

function truncateId(id: string, start = 8, end = 6): string {
  if (!id || id.length <= start + end) return id;
  return `${id.slice(0, start)}...${id.slice(-end)}`;
}

function formatRegisteredAt(ts: number): string {
  if (!ts) return "—";
  const ms = ts > 1e12 ? ts : ts * 1000;
  return new Date(ms).toLocaleDateString(undefined, {
    dateStyle: "short",
    timeStyle: "short",
  });
}

export default function AdsPlacementsPage() {
  const { placements, isLoading, refetch } = usePlacements();
  const actions = usePlacementActions();

  const [entityDialogOpen, setEntityDialogOpen] = useState(false);
  const [shopDialogOpen, setShopDialogOpen] = useState(false);
  const [entityId, setEntityId] = useState("");
  const [shopEntityId, setShopEntityId] = useState("");
  const [shopId, setShopId] = useState("");
  const [impressionCapDialog, setImpressionCapDialog] = useState<AdPlacement | null>(null);
  const [clickCapDialog, setClickCapDialog] = useState<AdPlacement | null>(null);
  const [impressionCapValue, setImpressionCapValue] = useState("");
  const [clickCapValue, setClickCapValue] = useState("");

  useEffect(() => {
    if (actions.txState.status === "finalized") {
      actions.resetTx();
      refetch();
      setEntityDialogOpen(false);
      setShopDialogOpen(false);
      setImpressionCapDialog(null);
      setClickCapDialog(null);
      setEntityId("");
      setShopEntityId("");
      setShopId("");
    }
  }, [actions.txState.status, actions.resetTx, refetch]);

  const handleRegisterEntity = () => {
    const eid = parseInt(entityId, 10);
    if (!isNaN(eid)) {
      actions.registerEntityPlacement(eid);
    }
  };

  const handleRegisterShop = () => {
    const eid = parseInt(shopEntityId, 10);
    const sid = parseInt(shopId, 10);
    if (!isNaN(eid) && !isNaN(sid)) {
      actions.registerShopPlacement(eid, sid);
    }
  };

  const handleSetImpressionCap = () => {
    if (impressionCapDialog) {
      const cap = parseInt(impressionCapValue, 10);
      if (!isNaN(cap) && cap >= 0) {
        actions.setImpressionCap(impressionCapDialog.placementId, cap);
      }
    }
  };

  const handleSetClickCap = () => {
    if (clickCapDialog) {
      const cap = parseInt(clickCapValue, 10);
      if (!isNaN(cap) && cap >= 0) {
        actions.setClickCap(clickCapDialog.placementId, cap);
      }
    }
  };

  const activeCount = placements.filter((p) => p.active).length;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/ads">
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <LayoutGrid className="h-7 w-7" />
            Ad Placements
          </h1>
          <p className="text-muted-foreground">
            Manage where your ads appear across communities
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => setEntityDialogOpen(true)}>
            <Plus className="mr-2 h-4 w-4" />
            Register Entity Placement
          </Button>
          <Button variant="outline" onClick={() => setShopDialogOpen(true)}>
            <Plus className="mr-2 h-4 w-4" />
            Register Shop Placement
          </Button>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Active Placements</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-green-600">{activeCount}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Total Placements</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{placements.length}</p>
          </CardContent>
        </Card>
      </div>

      <div className="flex items-center gap-2">
        <Button variant="outline" size="sm" onClick={refetch} disabled={isLoading}>
          <RotateCcw className="mr-2 h-3 w-3" />
          Refresh
        </Button>
      </div>

      {isLoading ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <div className="h-8 w-8 animate-spin rounded-full border-2 border-primary border-t-transparent" />
            <p className="mt-4 text-muted-foreground">Loading placements...</p>
          </CardContent>
        </Card>
      ) : placements.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <LayoutGrid className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No placements configured</p>
            <p className="text-sm text-muted-foreground">
              Register an entity or shop placement to display ads
            </p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Placement ID</TableHead>
                <TableHead>Entity</TableHead>
                <TableHead>Shop</TableHead>
                <TableHead>Level</TableHead>
                <TableHead className="text-right">Impression Cap</TableHead>
                <TableHead className="text-right">Click Cap</TableHead>
                <TableHead>Active</TableHead>
                <TableHead>Registered</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {placements.map((placement) => (
                <TableRow key={placement.placementId}>
                  <TableCell className="font-mono text-xs">
                    {truncateId(placement.placementId)}
                  </TableCell>
                  <TableCell className="font-mono">
                    {placement.entityId ?? "—"}
                  </TableCell>
                  <TableCell className="font-mono">
                    {placement.shopId ?? "—"}
                  </TableCell>
                  <TableCell>
                    <span className="text-sm">{placement.level || "—"}</span>
                  </TableCell>
                  <TableCell className="text-right font-mono">
                    {placement.dailyImpressionCap ?? "—"}
                  </TableCell>
                  <TableCell className="text-right font-mono">
                    {placement.dailyClickCap ?? "—"}
                  </TableCell>
                  <TableCell>
                    <StatusBadge status={placement.active ? "Active" : "Inactive"} />
                  </TableCell>
                  <TableCell className="text-sm text-muted-foreground">
                    {formatRegisteredAt(placement.registeredAt)}
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex items-center justify-end gap-1">
                      <TxButton
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8"
                        txStatus={actions.txState.status}
                        title={placement.active ? "Deactivate" : "Activate"}
                        onClick={() =>
                          actions.setPlacementActive(
                            placement.placementId,
                            !placement.active
                          )
                        }
                      >
                        {placement.active ? (
                          <PowerOff className="h-4 w-4" />
                        ) : (
                          <Power className="h-4 w-4" />
                        )}
                      </TxButton>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8"
                        title="Set Impression Cap"
                        onClick={() => {
                          setImpressionCapDialog(placement);
                          setImpressionCapValue(String(placement.dailyImpressionCap ?? ""));
                        }}
                      >
                        <Eye className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8"
                        title="Set Click Cap"
                        onClick={() => {
                          setClickCapDialog(placement);
                          setClickCapValue(String(placement.dailyClickCap ?? ""));
                        }}
                      >
                        <MousePointerClick className="h-4 w-4" />
                      </Button>
                      <TxButton
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8 text-destructive"
                        txStatus={actions.txState.status}
                        title="Deregister"
                        onClick={() =>
                          actions.deregisterPlacement(placement.placementId)
                        }
                      >
                        <Trash2 className="h-4 w-4" />
                      </TxButton>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}

      {/* Register Entity Placement Dialog */}
      <Dialog open={entityDialogOpen} onOpenChange={setEntityDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Register Entity Placement</DialogTitle>
            <DialogDescription>
              Register an entity to display ads. Requires entity ID.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Entity ID</label>
              <Input
                type="number"
                placeholder="e.g. 1"
                value={entityId}
                onChange={(e) => setEntityId(e.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setEntityDialogOpen(false)}>
              Cancel
            </Button>
            <TxButton
              txStatus={actions.txState.status}
              loadingText="Registering..."
              disabled={!entityId}
              onClick={handleRegisterEntity}
            >
              Register
            </TxButton>
          </DialogFooter>
          {actions.txState.status === "error" && (
            <p className="text-sm text-destructive">{actions.txState.error}</p>
          )}
        </DialogContent>
      </Dialog>

      {/* Register Shop Placement Dialog */}
      <Dialog open={shopDialogOpen} onOpenChange={setShopDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Register Shop Placement</DialogTitle>
            <DialogDescription>
              Register a shop to display ads. Requires entity ID and shop ID.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Entity ID</label>
              <Input
                type="number"
                placeholder="e.g. 1"
                value={shopEntityId}
                onChange={(e) => setShopEntityId(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Shop ID</label>
              <Input
                type="number"
                placeholder="e.g. 1"
                value={shopId}
                onChange={(e) => setShopId(e.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShopDialogOpen(false)}>
              Cancel
            </Button>
            <TxButton
              txStatus={actions.txState.status}
              loadingText="Registering..."
              disabled={!shopEntityId || !shopId}
              onClick={handleRegisterShop}
            >
              Register
            </TxButton>
          </DialogFooter>
          {actions.txState.status === "error" && (
            <p className="text-sm text-destructive">{actions.txState.error}</p>
          )}
        </DialogContent>
      </Dialog>

      {/* Set Impression Cap Dialog */}
      <Dialog
        open={!!impressionCapDialog}
        onOpenChange={(o) => {
          if (!o) setImpressionCapDialog(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Set Impression Cap</DialogTitle>
            <DialogDescription>
              Set daily impression cap for this placement.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Daily Impression Cap</label>
              <Input
                type="number"
                placeholder="e.g. 10000"
                min="0"
                value={impressionCapValue}
                onChange={(e) => setImpressionCapValue(e.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setImpressionCapDialog(null)}>
              Cancel
            </Button>
            <TxButton
              txStatus={actions.txState.status}
              loadingText="Setting..."
              disabled={!impressionCapValue}
              onClick={handleSetImpressionCap}
            >
              Set Cap
            </TxButton>
          </DialogFooter>
          {actions.txState.status === "error" && (
            <p className="text-sm text-destructive">{actions.txState.error}</p>
          )}
        </DialogContent>
      </Dialog>

      {/* Set Click Cap Dialog */}
      <Dialog
        open={!!clickCapDialog}
        onOpenChange={(o) => {
          if (!o) setClickCapDialog(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Set Click Cap</DialogTitle>
            <DialogDescription>
              Set daily click cap for this placement.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Daily Click Cap</label>
              <Input
                type="number"
                placeholder="e.g. 1000"
                min="0"
                value={clickCapValue}
                onChange={(e) => setClickCapValue(e.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setClickCapDialog(null)}>
              Cancel
            </Button>
            <TxButton
              txStatus={actions.txState.status}
              loadingText="Setting..."
              disabled={!clickCapValue}
              onClick={handleSetClickCap}
            >
              Set Cap
            </TxButton>
          </DialogFooter>
          {actions.txState.status === "error" && (
            <p className="text-sm text-destructive">{actions.txState.error}</p>
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}
