"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import { useTranslations } from "next-intl";
import {
  Gift,
  ArrowLeft,
  Wallet,
  Percent,
  Download,
  Settings2,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from "@/components/ui/dialog";
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { TxButton } from "@/components/shared/TxButton";
import { formatBalance, formatNumber, shortenAddress } from "@/lib/utils";
import { useWalletStore } from "@/stores/wallet";
import { useConsensusNodes } from "@/hooks/useConsensus";
import {
  useAllNodeRewards,
  useEraRewards,
  useDistributionPaused,
  useRewardActions,
} from "@/hooks/useRewards";

export default function RobotRewardsPage() {
  const t = useTranslations("robot");
  const tCommon = useTranslations("common");
  const address = useWalletStore((s) => s.address);
  const { nodes, currentEra } = useConsensusNodes();
  const myNodeIds = useMemo(
    () => (address ? nodes.filter((n) => n.operator === address).map((n) => n.nodeId) : []),
    [nodes, address]
  );
  const { rewards, isLoading: rewardsLoading } = useAllNodeRewards(myNodeIds);
  const { info: eraInfo } = useEraRewards(currentEra);
  const { paused } = useDistributionPaused();
  const {
    claimRewards,
    batchClaimRewards,
    setRewardRecipient,
    setRewardSplit,
    txState,
  } = useRewardActions();

  const [recipientOpen, setRecipientOpen] = useState(false);
  const [recipientNodeId, setRecipientNodeId] = useState("");
  const [recipientAddress, setRecipientAddress] = useState("");

  const [splitOpen, setSplitOpen] = useState(false);
  const [splitBotIdHash, setSplitBotIdHash] = useState("");
  const [splitOwnerBps, setSplitOwnerBps] = useState("");

  const nodesWithPending = useMemo(
    () => rewards.filter((r) => r.pending > BigInt(0)).map((r) => r.nodeId),
    [rewards]
  );

  const handleSetRecipient = () => {
    if (!recipientNodeId || !recipientAddress) return;
    setRewardRecipient(recipientNodeId, recipientAddress || null);
    setRecipientOpen(false);
    setRecipientNodeId("");
    setRecipientAddress("");
  };

  const handleSetSplit = () => {
    if (!splitBotIdHash) return;
    const bps = Math.min(10000, Math.max(0, parseInt(splitOwnerBps, 10) || 0));
    setRewardSplit(splitBotIdHash, bps);
    setSplitOpen(false);
    setSplitBotIdHash("");
    setSplitOwnerBps("");
  };

  const toBigInt = (v: unknown) => BigInt(String(v ?? 0));

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/robot">
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Gift className="h-7 w-7" />
            {t("rewards.title")}
          </h1>
          <p className="text-muted-foreground">{t("rewards.subtitle")}</p>
        </div>
        <TxButton
          txStatus={txState.status}
          disabled={!address || nodesWithPending.length === 0}
          onClick={() => batchClaimRewards(nodesWithPending)}
        >
          <Download className="mr-2 h-4 w-4" />
          {t("rewards.batchClaim")}
        </TxButton>
      </div>

      {paused && (
        <div className="rounded-lg border border-amber-500/50 bg-amber-500/10 px-4 py-3 text-amber-700 dark:text-amber-400">
          <span className="font-medium">{t("rewards.distributionPaused")}</span>
        </div>
      )}

      {eraInfo && (
        <Card>
          <CardHeader>
            <CardTitle>{t("rewards.eraStats")}</CardTitle>
            <CardDescription>
              {t("rewards.eraStatsDesc", { era: currentEra })}
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6">
              <div className="rounded-lg border p-3">
                <p className="text-xs text-muted-foreground">{t("rewards.subscriptionIncome")}</p>
                <p className="font-mono font-semibold">{formatBalance(toBigInt(eraInfo.subscriptionIncome))}</p>
              </div>
              <div className="rounded-lg border p-3">
                <p className="text-xs text-muted-foreground">{t("rewards.adsIncome")}</p>
                <p className="font-mono font-semibold">{formatBalance(toBigInt(eraInfo.adsIncome))}</p>
              </div>
              <div className="rounded-lg border p-3">
                <p className="text-xs text-muted-foreground">{t("rewards.inflationMint")}</p>
                <p className="font-mono font-semibold">{formatBalance(toBigInt(eraInfo.inflationMint))}</p>
              </div>
              <div className="rounded-lg border p-3">
                <p className="text-xs text-muted-foreground">{t("rewards.totalDistributed")}</p>
                <p className="font-mono font-semibold">{formatBalance(toBigInt(eraInfo.totalDistributed))}</p>
              </div>
              <div className="rounded-lg border p-3">
                <p className="text-xs text-muted-foreground">{t("rewards.treasuryShare")}</p>
                <p className="font-mono font-semibold">{formatBalance(toBigInt(eraInfo.treasuryShare))}</p>
              </div>
              <div className="rounded-lg border p-3">
                <p className="text-xs text-muted-foreground">{t("rewards.nodeCount")}</p>
                <p className="font-mono font-semibold">{formatNumber(eraInfo.nodeCount)}</p>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle>{t("rewards.tableTitle")}</CardTitle>
          <CardDescription>{t("rewards.tableDesc")}</CardDescription>
        </CardHeader>
        <CardContent>
          {!address ? (
            <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
              <Wallet className="h-12 w-12 mb-4 opacity-50" />
              <p>{t("connectWallet")}</p>
            </div>
          ) : rewardsLoading ? (
            <div className="flex justify-center py-12">
              <div className="h-8 w-8 animate-spin rounded-full border-2 border-primary border-t-transparent" />
            </div>
          ) : rewards.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12">
              <Gift className="h-12 w-12 text-muted-foreground/50" />
              <p className="mt-4 text-lg font-medium">{t("rewards.noRewards")}</p>
              <p className="text-sm text-muted-foreground">{t("rewards.noRewardsDesc")}</p>
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("rewards.nodeId")}</TableHead>
                  <TableHead className="text-right">{t("rewards.pending")}</TableHead>
                  <TableHead className="text-right">{t("rewards.totalEarned")}</TableHead>
                  <TableHead className="text-right">{t("actions")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {rewards.map((r) => (
                  <TableRow key={r.nodeId}>
                    <TableCell className="font-mono text-sm">
                      {shortenAddress(r.nodeId, 10)}
                    </TableCell>
                    <TableCell className="text-right font-mono font-medium text-amber-600">
                      {formatBalance(r.pending)}
                    </TableCell>
                    <TableCell className="text-right font-mono">
                      {formatBalance(r.totalEarned)}
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-2">
                        <TxButton
                          size="sm"
                          variant="outline"
                          txStatus={txState.status}
                          disabled={r.pending === BigInt(0)}
                          onClick={() => claimRewards(r.nodeId)}
                        >
                          {t("rewards.claim")}
                        </TxButton>
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={() => {
                            setRecipientNodeId(r.nodeId);
                            setRecipientAddress("");
                            setRecipientOpen(true);
                          }}
                        >
                          <Settings2 className="h-4 w-4" />
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      <div className="flex gap-2">
        <Button variant="outline" onClick={() => setSplitOpen(true)}>
          <Percent className="mr-2 h-4 w-4" />
          {t("rewards.setSplit")}
        </Button>
      </div>

      <Dialog open={recipientOpen} onOpenChange={setRecipientOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("rewards.setRecipient")}</DialogTitle>
            <DialogDescription>{t("rewards.setRecipientDesc")}</DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div>
              <label className="text-sm font-medium">{t("rewards.nodeId")}</label>
              <Input
                className="mt-1"
                value={recipientNodeId}
                onChange={(e) => setRecipientNodeId(e.target.value)}
                placeholder="0x..."
              />
            </div>
            <div>
              <label className="text-sm font-medium">{t("rewards.recipientAddress")}</label>
              <Input
                className="mt-1"
                value={recipientAddress}
                onChange={(e) => setRecipientAddress(e.target.value)}
                placeholder="0x..."
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setRecipientOpen(false)}>
              {tCommon("cancel")}
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleSetRecipient}
              disabled={!recipientNodeId || !recipientAddress}
            >
              {t("rewards.submit")}
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={splitOpen} onOpenChange={setSplitOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("rewards.setSplit")}</DialogTitle>
            <DialogDescription>{t("rewards.setSplitDesc")}</DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div>
              <label className="text-sm font-medium">{t("botIdHash")}</label>
              <Input
                className="mt-1"
                value={splitBotIdHash}
                onChange={(e) => setSplitBotIdHash(e.target.value)}
                placeholder={t("botIdHashPlaceholder")}
              />
            </div>
            <div>
              <label className="text-sm font-medium">{t("rewards.ownerBps")}</label>
              <Input
                type="number"
                min={0}
                max={10000}
                className="mt-1"
                value={splitOwnerBps}
                onChange={(e) => setSplitOwnerBps(e.target.value)}
                placeholder="0-10000 (100 = 1%)"
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setSplitOpen(false)}>
              {tCommon("cancel")}
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleSetSplit}
              disabled={!splitBotIdHash}
            >
              {t("rewards.submit")}
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
