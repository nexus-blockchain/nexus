"use client";

import { useState, useEffect } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { TxButton } from "@/components/shared/TxButton";
import {
  Network,
  ArrowLeft,
  Plus,
  RotateCcw,
  Server,
  Shield,
  LogOut,
  TrendingUp,
  CheckCircle,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";
import { formatBalance, formatNumber } from "@/lib/utils";
import { shortenAddress } from "@/lib/utils";
import { useWalletStore } from "@/stores/wallet";
import { useConsensusNodes, useConsensusActions } from "@/hooks/useConsensus";
import type { ConsensusNode } from "@/lib/types";

function truncateHash(hash: string, start = 10, end = 8): string {
  if (!hash) return "";
  const s = hash.startsWith("0x") ? hash.slice(2) : hash;
  return s.length > start + end ? `${s.slice(0, start)}…${s.slice(-end)}` : hash;
}

export default function RobotNodesPage() {
  const t = useTranslations("robot");
  const address = useWalletStore((s) => s.address);
  const [registerDialogOpen, setRegisterDialogOpen] = useState(false);
  const [increaseStakeOpen, setIncreaseStakeOpen] = useState(false);
  const [verifyTeeOpen, setVerifyTeeOpen] = useState(false);
  const [selectedNode, setSelectedNode] = useState<ConsensusNode | null>(null);

  const { nodes, activeList, currentEra, isLoading, refetch } = useConsensusNodes();
  const actions = useConsensusActions();
  const txStatus = actions.txState.status;

  const teeCount = nodes.filter((n) => n.isTeeNode).length;

  const handleRefetch = () => refetch();

  useEffect(() => {
    if (txStatus === "finalized") {
      handleRefetch();
      setRegisterDialogOpen(false);
      setIncreaseStakeOpen(false);
      setVerifyTeeOpen(false);
      setSelectedNode(null);
      actions.resetTx();
    }
  }, [txStatus]);

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
            <Network className="h-7 w-7" />
            {t("nodes")}
          </h1>
          <p className="text-muted-foreground">
            Nodes participating in robot network consensus
          </p>
        </div>
        <Button
          onClick={() => setRegisterDialogOpen(true)}
          disabled={!address}
        >
          <Plus className="mr-2 h-4 w-4" />
          Register Node
        </Button>
        <Button variant="outline" size="sm" onClick={handleRefetch} disabled={isLoading}>
          <RotateCcw className="mr-2 h-3 w-3" />
          {t("refresh")}
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Total Nodes</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            <Server className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">{formatNumber(nodes.length)}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Active Nodes</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-green-600">
              {formatNumber(activeList.length)}
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Current Era</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{formatNumber(currentEra)}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">TEE Nodes</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            <Shield className="h-4 w-4 text-green-500" />
            <p className="text-2xl font-bold">{formatNumber(teeCount)}</p>
          </CardContent>
        </Card>
      </div>

      {isLoading ? (
        <Card>
          <CardContent className="py-12 text-center text-muted-foreground">
            {t("processing")}
          </CardContent>
        </Card>
      ) : nodes.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Network className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No consensus nodes</p>
            <p className="text-sm text-muted-foreground">
              Nodes will appear here once they join the consensus network
            </p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Node ID</TableHead>
                <TableHead>Operator</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Stake</TableHead>
                <TableHead>TEE</TableHead>
                <TableHead>Registered</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {nodes.map((node) => (
                <TableRow key={node.nodeId}>
                  <TableCell className="font-mono text-xs" title={node.nodeId}>
                    {truncateHash(node.nodeId)}
                  </TableCell>
                  <TableCell className="font-mono text-xs" title={node.operator}>
                    {shortenAddress(node.operator)}
                  </TableCell>
                  <TableCell>
                    <StatusBadge status={node.status} />
                  </TableCell>
                  <TableCell className="text-right font-mono">
                    {formatBalance(node.stake)}
                  </TableCell>
                  <TableCell>
                    {node.isTeeNode ? (
                      <span className="inline-flex items-center gap-1 text-green-600">
                        <Shield className="h-3 w-3" />
                        Yes
                      </span>
                    ) : (
                      <span className="text-muted-foreground">No</span>
                    )}
                  </TableCell>
                  <TableCell>
                    {node.registeredAt != null
                      ? `#${formatNumber(node.registeredAt)}`
                      : "—"}
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex flex-wrap gap-2 justify-end">
                      {node.status === "Active" && (
                        <>
                          <TxButton
                            variant="outline"
                            size="sm"
                            txStatus={txStatus}
                            onClick={() => actions.requestExit(node.nodeId)}
                            disabled={!address}
                          >
                            <LogOut className="mr-1 h-3 w-3" />
                            Request Exit
                          </TxButton>
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => {
                              setSelectedNode(node);
                              setIncreaseStakeOpen(true);
                            }}
                            disabled={!address}
                          >
                            <TrendingUp className="mr-1 h-3 w-3" />
                            Increase Stake
                          </Button>
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => {
                              setSelectedNode(node);
                              setVerifyTeeOpen(true);
                            }}
                            disabled={!address}
                          >
                            <CheckCircle className="mr-1 h-3 w-3" />
                            Verify TEE
                          </Button>
                        </>
                      )}
                      {node.status === "Suspended" && (
                        <TxButton
                          variant="outline"
                          size="sm"
                          txStatus={txStatus}
                          onClick={() => actions.reinstateNode(node.nodeId)}
                          disabled={!address}
                        >
                          Reinstate
                        </TxButton>
                      )}
                      {node.status === "Exiting" && (
                        <TxButton
                          variant="outline"
                          size="sm"
                          txStatus={txStatus}
                          onClick={() => actions.finalizeExit(node.nodeId)}
                          disabled={!address}
                        >
                          Finalize Exit
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

      <RegisterNodeDialog
        open={registerDialogOpen}
        onOpenChange={setRegisterDialogOpen}
        onSubmit={(nodeId, stake) => actions.registerNode(nodeId, stake)}
        txStatus={txStatus}
        disabled={!address}
      />

      {selectedNode && (
        <>
          <IncreaseStakeDialog
            open={increaseStakeOpen}
            onOpenChange={(open) => {
              setIncreaseStakeOpen(open);
              if (!open) setSelectedNode(null);
            }}
            node={selectedNode}
            onSubmit={(amount) =>
              actions.increaseStake(selectedNode.nodeId, amount)
            }
            txStatus={txStatus}
            disabled={!address}
          />
          <VerifyTeeDialog
            open={verifyTeeOpen}
            onOpenChange={(open) => {
              setVerifyTeeOpen(open);
              if (!open) setSelectedNode(null);
            }}
            node={selectedNode}
            onSubmit={(botIdHash) =>
              actions.verifyNodeTee(selectedNode.nodeId, botIdHash)
            }
            txStatus={txStatus}
            disabled={!address}
          />
        </>
      )}
    </div>
  );
}

function RegisterNodeDialog({
  open,
  onOpenChange,
  onSubmit,
  txStatus,
  disabled,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSubmit: (nodeId: string, stake: bigint) => void;
  txStatus: string;
  disabled: boolean;
}) {
  const t = useTranslations("robot");
  const [nodeId, setNodeId] = useState("");
  const [stakeInput, setStakeInput] = useState("");

  const handleSubmit = () => {
    if (!nodeId.trim() || !stakeInput) return;
    const stake = BigInt(stakeInput.replace(/\D/g, "") || "0");
    if (stake <= 0n) return;
    onSubmit(nodeId.trim(), stake);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Register Node</DialogTitle>
          <DialogDescription>
            Register a consensus node. Node ID must be 32 bytes hex (0x + 64 chars).
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 py-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">Node ID (hex 32 bytes)</label>
            <Input
              placeholder="0x..."
              value={nodeId}
              onChange={(e) => setNodeId(e.target.value)}
              className="font-mono"
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Stake (planck)</label>
            <Input
              placeholder="1000000000000"
              value={stakeInput}
              onChange={(e) => setStakeInput(e.target.value)}
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <TxButton
            txStatus={txStatus}
            onClick={handleSubmit}
            disabled={disabled || !nodeId.trim() || !stakeInput}
          >
            Register Node
          </TxButton>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function IncreaseStakeDialog({
  open,
  onOpenChange,
  node,
  onSubmit,
  txStatus,
  disabled,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  node: ConsensusNode;
  onSubmit: (amount: bigint) => void;
  txStatus: string;
  disabled: boolean;
}) {
  const [amount, setAmount] = useState("");

  const handleSubmit = () => {
    const val = BigInt(amount.replace(/\D/g, "") || "0");
    if (val <= 0n) return;
    onSubmit(val);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Increase Stake</DialogTitle>
          <DialogDescription>
            Add stake for node {truncateHash(node.nodeId)}. Current: {formatBalance(node.stake)}
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 py-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">Additional amount (planck)</label>
            <Input
              placeholder="1000000000000"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <TxButton
            txStatus={txStatus}
            onClick={handleSubmit}
            disabled={disabled || !amount}
          >
            Increase Stake
          </TxButton>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function VerifyTeeDialog({
  open,
  onOpenChange,
  node,
  onSubmit,
  txStatus,
  disabled,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  node: ConsensusNode;
  onSubmit: (botIdHash: string) => void;
  txStatus: string;
  disabled: boolean;
}) {
  const t = useTranslations("robot");
  const [botIdHash, setBotIdHash] = useState("");

  const handleSubmit = () => {
    if (!botIdHash.trim()) return;
    onSubmit(botIdHash.trim());
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Verify TEE</DialogTitle>
          <DialogDescription>
            Verify TEE attestation for node {truncateHash(node.nodeId)}. Provide the bot ID hash.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 py-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("botIdHash")}</label>
            <Input
              placeholder={t("botIdHashPlaceholder")}
              value={botIdHash}
              onChange={(e) => setBotIdHash(e.target.value)}
              className="font-mono"
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <TxButton
            txStatus={txStatus}
            onClick={handleSubmit}
            disabled={disabled || !botIdHash.trim()}
          >
            Verify TEE
          </TxButton>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
