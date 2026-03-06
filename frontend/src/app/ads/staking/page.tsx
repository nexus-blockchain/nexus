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
  Coins,
  ArrowLeft,
  Plus,
  RotateCcw,
  Wallet,
  Users,
  Pause,
  Play,
} from "lucide-react";
import Link from "next/link";
import {
  useCommunityAdStakes,
  useAdGrouprobotActions,
} from "@/hooks/useAdCampaign";
import { formatBalance } from "@/lib/utils";

const DECIMALS = 12;

function truncateHash(hash: string, start = 10, end = 8): string {
  if (!hash || hash.length <= start + end) return hash;
  return `${hash.slice(0, start)}...${hash.slice(-end)}`;
}

function parseAmount(value: string): bigint {
  const num = parseFloat(value);
  if (isNaN(num) || num < 0) return BigInt(0);
  const [whole = "0", frac = ""] = value.split(".");
  const padded = frac.padEnd(DECIMALS, "0").slice(0, DECIMALS);
  return BigInt(whole) * BigInt(10 ** DECIMALS) + BigInt(padded || "0");
}

export default function AdsStakingPage() {
  const { stakes, isLoading, refetch } = useCommunityAdStakes();
  const actions = useAdGrouprobotActions();

  const [stakeDialogOpen, setStakeDialogOpen] = useState(false);
  const [unstakeDialogOpen, setUnstakeDialogOpen] = useState(false);
  const [stakeCommunityHash, setStakeCommunityHash] = useState("");
  const [unstakeCommunityHash, setUnstakeCommunityHash] = useState("");
  const [stakeAmount, setStakeAmount] = useState("");
  const [unstakeAmount, setUnstakeAmount] = useState("");
  const [selectedStake, setSelectedStake] = useState<typeof stakes[0] | null>(null);

  useEffect(() => {
    if (actions.txState.status === "finalized") {
      actions.resetTx();
      refetch();
      setStakeDialogOpen(false);
      setUnstakeDialogOpen(false);
      setStakeCommunityHash("");
      setUnstakeCommunityHash("");
      setStakeAmount("");
      setUnstakeAmount("");
      setSelectedStake(null);
    }
  }, [actions.txState.status, actions.resetTx, refetch]);

  const handleStake = () => {
    const amount = parseAmount(stakeAmount);
    if (stakeCommunityHash && amount > 0n) {
      actions.stakeForAds(stakeCommunityHash, amount);
    }
  };

  const handleUnstake = () => {
    const amount = parseAmount(unstakeAmount);
    if (unstakeCommunityHash && amount > 0n) {
      actions.unstakeForAds(unstakeCommunityHash, amount);
    }
  };

  const totalStaked = stakes.reduce((acc, s) => acc + s.totalStake, BigInt(0));
  const activeCount = stakes.filter((s) => !s.adminPaused).length;

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
            <Coins className="h-7 w-7" />
            Ad Staking
          </h1>
          <p className="text-muted-foreground">
            Stake NEX in communities to earn a share of ad revenue
          </p>
        </div>
        <Button onClick={() => setStakeDialogOpen(true)}>
          <Plus className="mr-2 h-4 w-4" />
          Stake
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Total Staked</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            <Wallet className="h-5 w-5 text-muted-foreground" />
            <p className="text-2xl font-bold">{formatBalance(totalStaked)} NEX</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Active Communities</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-green-600">{activeCount}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Communities</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            <Users className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">{stakes.length}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Paused</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-amber-600">
              {stakes.filter((s) => s.adminPaused).length}
            </p>
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
            <p className="mt-4 text-muted-foreground">Loading stakes...</p>
          </CardContent>
        </Card>
      ) : stakes.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Coins className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No active stakes</p>
            <p className="text-sm text-muted-foreground">
              Stake NEX in communities to earn ad revenue share
            </p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Community Hash</TableHead>
                <TableHead className="text-right">Total Stake</TableHead>
                <TableHead className="text-right">Audience Cap</TableHead>
                <TableHead>Admin Paused</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {stakes.map((stake) => (
                <TableRow key={stake.communityIdHash}>
                  <TableCell className="font-mono text-xs">
                    {truncateHash(stake.communityIdHash)}
                  </TableCell>
                  <TableCell className="text-right font-mono">
                    {formatBalance(stake.totalStake)} NEX
                  </TableCell>
                  <TableCell className="text-right font-mono">
                    {stake.audienceCap ?? 0}
                  </TableCell>
                  <TableCell>
                    <StatusBadge
                      status={stake.adminPaused ? "Paused" : "Active"}
                    />
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex items-center justify-end gap-1">
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => {
                          setUnstakeDialogOpen(true);
                          setUnstakeCommunityHash(stake.communityIdHash);
                          setSelectedStake(stake);
                        }}
                      >
                        Unstake
                      </Button>
                      <TxButton
                        variant="outline"
                        size="sm"
                        txStatus={actions.txState.status}
                        loadingText="Withdrawing..."
                        onClick={() =>
                          actions.withdrawUnbonded(stake.communityIdHash)
                        }
                      >
                        Withdraw
                      </TxButton>
                      <TxButton
                        variant="outline"
                        size="sm"
                        txStatus={actions.txState.status}
                        loadingText="Claiming..."
                        onClick={() =>
                          actions.claimStakerReward(stake.communityIdHash)
                        }
                      >
                        Claim
                      </TxButton>
                      {stake.adminPaused ? (
                        <TxButton
                          variant="outline"
                          size="sm"
                          txStatus={actions.txState.status}
                          loadingText="Resuming..."
                          onClick={() =>
                            actions.adminResumeAds(stake.communityIdHash)
                          }
                        >
                          <Play className="h-4 w-4" />
                        </TxButton>
                      ) : (
                        <TxButton
                          variant="outline"
                          size="sm"
                          txStatus={actions.txState.status}
                          loadingText="Pausing..."
                          onClick={() =>
                            actions.adminPauseAds(stake.communityIdHash)
                          }
                        >
                          <Pause className="h-4 w-4" />
                        </TxButton>
                      )}
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}

      {/* Stake Dialog */}
      <Dialog open={stakeDialogOpen} onOpenChange={setStakeDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Stake for Ads</DialogTitle>
            <DialogDescription>
              Stake NEX in a community to earn a share of ad revenue.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Community ID Hash</label>
              <Input
                type="text"
                placeholder="0x..."
                value={stakeCommunityHash}
                onChange={(e) => setStakeCommunityHash(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Amount (NEX)</label>
              <Input
                type="text"
                placeholder="e.g. 100"
                value={stakeAmount}
                onChange={(e) => setStakeAmount(e.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setStakeDialogOpen(false)}>
              Cancel
            </Button>
            <TxButton
              txStatus={actions.txState.status}
              loadingText="Staking..."
              disabled={!stakeCommunityHash || !stakeAmount}
              onClick={handleStake}
            >
              Stake
            </TxButton>
          </DialogFooter>
          {actions.txState.status === "error" && (
            <p className="text-sm text-destructive">{actions.txState.error}</p>
          )}
        </DialogContent>
      </Dialog>

      {/* Unstake Dialog */}
      <Dialog open={unstakeDialogOpen} onOpenChange={setUnstakeDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Unstake for Ads</DialogTitle>
            <DialogDescription>
              Unstake NEX from a community. Funds will enter unbonding period.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Community ID Hash</label>
              <Input
                type="text"
                placeholder="0x..."
                value={unstakeCommunityHash}
                onChange={(e) => setUnstakeCommunityHash(e.target.value)}
              />
              {selectedStake && (
                <p className="text-xs text-muted-foreground">
                  Available: {formatBalance(selectedStake.totalStake)} NEX
                </p>
              )}
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Amount (NEX)</label>
              <Input
                type="text"
                placeholder="e.g. 50"
                value={unstakeAmount}
                onChange={(e) => setUnstakeAmount(e.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setUnstakeDialogOpen(false)}>
              Cancel
            </Button>
            <TxButton
              txStatus={actions.txState.status}
              loadingText="Unstaking..."
              disabled={!unstakeCommunityHash || !unstakeAmount}
              onClick={handleUnstake}
            >
              Unstake
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
