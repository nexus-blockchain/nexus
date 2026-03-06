"use client";

import { useState, useEffect } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
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
import { TxButton } from "@/components/shared/TxButton";
import {
  BarChart3,
  ArrowLeft,
  DollarSign,
  Wallet,
  Clock,
  RotateCcw,
  Wallet as WalletIcon,
} from "lucide-react";
import Link from "next/link";
import {
  usePlacements,
  usePlacementRevenue,
  useCampaignActions,
} from "@/hooks/useAdCampaign";
import { formatBalance } from "@/lib/utils";

const DECIMALS = 12;

function truncateId(id: string, start = 8, end = 6): string {
  if (!id || id.length <= start + end) return id;
  return `${id.slice(0, start)}...${id.slice(-end)}`;
}

function parseAmount(value: string): bigint {
  const num = parseFloat(value);
  if (isNaN(num) || num < 0) return BigInt(0);
  const [whole = "0", frac = ""] = value.split(".");
  const padded = frac.padEnd(DECIMALS, "0").slice(0, DECIMALS);
  return BigInt(whole) * BigInt(10 ** DECIMALS) + BigInt(padded || "0");
}

export default function AdsRevenuePage() {
  const { placements, isLoading, refetch } = usePlacements();
  const actions = useCampaignActions();

  const [selectedPlacementId, setSelectedPlacementId] = useState<string | null>(null);
  const [claimDialogOpen, setClaimDialogOpen] = useState(false);
  const [claimPlacementId, setClaimPlacementId] = useState<string | null>(null);
  const [claimAmount, setClaimAmount] = useState("");

  const { total, claimable, eraRevenue, isLoading: revenueLoading } =
    usePlacementRevenue(selectedPlacementId);

  useEffect(() => {
    if (actions.txState.status === "finalized") {
      actions.resetTx();
      refetch();
      setClaimDialogOpen(false);
      setClaimPlacementId(null);
      setClaimAmount("");
    }
  }, [actions.txState.status, actions.resetTx, refetch]);

  const handleClaimRevenue = () => {
    if (claimPlacementId) {
      const amount = parseAmount(claimAmount);
      if (amount > 0n) {
        actions.claimAdRevenue(claimPlacementId, amount);
      }
    }
  };

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
            <BarChart3 className="h-7 w-7" />
            Ad Revenue
          </h1>
          <p className="text-muted-foreground">
            Track your earned ad revenue and claim rewards
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={refetch} disabled={isLoading}>
          <RotateCcw className="mr-2 h-3 w-3" />
          Refresh
        </Button>
      </div>

      {selectedPlacementId && (
        <Card>
          <CardHeader>
            <CardTitle>Placement: {truncateId(selectedPlacementId)}</CardTitle>
            <CardDescription>
              Revenue details for this placement
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid gap-4 md:grid-cols-3">
              <div className="rounded-lg border p-4">
                <div className="flex items-center gap-2 text-muted-foreground">
                  <DollarSign className="h-5 w-5" />
                  <span className="text-sm font-medium">Total Revenue</span>
                </div>
                <p className="mt-2 text-2xl font-bold">
                  {revenueLoading ? "..." : formatBalance(total)} NEX
                </p>
              </div>
              <div className="rounded-lg border p-4">
                <div className="flex items-center gap-2 text-muted-foreground">
                  <Wallet className="h-5 w-5" />
                  <span className="text-sm font-medium">Claimable</span>
                </div>
                <p className="mt-2 text-2xl font-bold text-green-600">
                  {revenueLoading ? "..." : formatBalance(claimable)} NEX
                </p>
              </div>
              <div className="rounded-lg border p-4">
                <div className="flex items-center gap-2 text-muted-foreground">
                  <Clock className="h-5 w-5" />
                  <span className="text-sm font-medium">Era Revenue</span>
                </div>
                <p className="mt-2 text-2xl font-bold">
                  {revenueLoading ? "..." : formatBalance(eraRevenue)} NEX
                </p>
              </div>
            </div>
            <div className="flex gap-2">
              <Button
                onClick={() => {
                  setClaimPlacementId(selectedPlacementId);
                  setClaimAmount("");
                  setClaimDialogOpen(true);
                }}
                disabled={revenueLoading || claimable === 0n}
              >
                <WalletIcon className="mr-2 h-4 w-4" />
                Claim Revenue
              </Button>
              <TxButton
                variant="outline"
                txStatus={actions.txState.status}
                loadingText="Settling..."
                onClick={() =>
                  actions.settleEraAds(selectedPlacementId)
                }
              >
                Settle Era
              </TxButton>
            </div>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle>Placements with Revenue</CardTitle>
          <CardDescription>
            Select a placement to view revenue details and claim
          </CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="flex flex-col items-center justify-center py-12">
              <div className="h-8 w-8 animate-spin rounded-full border-2 border-primary border-t-transparent" />
              <p className="mt-4 text-muted-foreground">Loading placements...</p>
            </div>
          ) : placements.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12">
              <BarChart3 className="h-12 w-12 text-muted-foreground/50" />
              <p className="mt-4 text-lg font-medium">No placements</p>
              <p className="text-sm text-muted-foreground">
                Register placements to earn ad revenue
              </p>
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Placement ID</TableHead>
                  <TableHead>Entity</TableHead>
                  <TableHead>Shop</TableHead>
                  <TableHead>Level</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {placements.map((placement) => (
                  <TableRow
                    key={placement.placementId}
                    className={
                      selectedPlacementId === placement.placementId
                        ? "bg-muted/50"
                        : ""
                    }
                  >
                    <TableCell className="font-mono text-xs">
                      {truncateId(placement.placementId)}
                    </TableCell>
                    <TableCell className="font-mono">
                      {placement.entityId ?? "—"}
                    </TableCell>
                    <TableCell className="font-mono">
                      {placement.shopId ?? "—"}
                    </TableCell>
                    <TableCell className="text-sm">
                      {placement.level || "—"}
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-2">
                        <Button
                          variant={
                            selectedPlacementId === placement.placementId
                              ? "secondary"
                              : "outline"
                          }
                          size="sm"
                          onClick={() =>
                            setSelectedPlacementId(
                              selectedPlacementId === placement.placementId
                                ? null
                                : placement.placementId
                            )
                          }
                        >
                          {selectedPlacementId === placement.placementId
                            ? "Selected"
                            : "Select"}
                        </Button>
                        <TxButton
                          variant="outline"
                          size="sm"
                          txStatus={actions.txState.status}
                          loadingText="Settling..."
                          onClick={() =>
                            actions.settleEraAds(placement.placementId)
                          }
                        >
                          Settle Era
                        </TxButton>
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      {/* Claim Revenue Dialog */}
      <Dialog open={claimDialogOpen} onOpenChange={setClaimDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Claim Ad Revenue</DialogTitle>
            <DialogDescription>
              Claim revenue from your placement. Enter the amount to claim.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            {claimPlacementId && (
              <div className="rounded-lg border p-3 text-sm">
                <span className="text-muted-foreground">Placement: </span>
                <span className="font-mono">{truncateId(claimPlacementId)}</span>
              </div>
            )}
            <div className="space-y-2">
              <label className="text-sm font-medium">Amount (NEX)</label>
              <Input
                type="text"
                placeholder="e.g. 100"
                value={claimAmount}
                onChange={(e) => setClaimAmount(e.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setClaimDialogOpen(false)}>
              Cancel
            </Button>
            <TxButton
              txStatus={actions.txState.status}
              loadingText="Claiming..."
              disabled={!claimPlacementId || !claimAmount}
              onClick={handleClaimRevenue}
            >
              Claim Revenue
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
