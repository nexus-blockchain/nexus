"use client";

import { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { TxButton } from "@/components/shared/TxButton";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import {
  Lock,
  ArrowLeft,
  RotateCcw,
  Search,
  Plus,
  Wallet,
  Shield,
  AlertTriangle,
  Send,
  RotateCcw as RefundIcon,
  Gavel,
  ShieldAlert,
} from "lucide-react";
import Link from "next/link";
import {
  useEscrowRecord,
  useEscrowPaused,
  useEscrowActions,
} from "@/hooks/useEscrow";
import { ESCROW_STATE_MAP } from "@/lib/constants";
const formatNex = (raw: bigint) => (Number(raw) / 1e12).toFixed(4);
const formatTimestamp = (ts: number | null) =>
  ts == null ? "—" : new Date(ts * 1000).toLocaleString();

export default function DisputeEscrowPage() {

  const [lookupId, setLookupId] = useState<string>("");
  const [lookupIdNum, setLookupIdNum] = useState<number | null>(null);
  const [lockDialogOpen, setLockDialogOpen] = useState(false);
  const [releaseTo, setReleaseTo] = useState("");
  const [refundTo, setRefundTo] = useState("");
  const [disputeReason, setDisputeReason] = useState("0");
  const [disputeDetail, setDisputeDetail] = useState("");
  const [forceReleaseTo, setForceReleaseTo] = useState("");
  const [forceRefundTo, setForceRefundTo] = useState("");

  const [lockId, setLockId] = useState("");
  const [lockPayer, setLockPayer] = useState("");
  const [lockAmount, setLockAmount] = useState("");

  const { escrow, isLoading, refetch } = useEscrowRecord(lookupIdNum);
  const { paused, refetch: refetchPaused } = useEscrowPaused();
  const {
    lock,
    release,
    refund,
    dispute,
    forceRelease,
    forceRefund,
    txState,
    resetTx,
  } = useEscrowActions();

  const handleLookup = () => {
    const n = parseInt(lookupId, 10);
    if (!isNaN(n) && n >= 0) setLookupIdNum(n);
    else setLookupIdNum(null);
  };

  const handleLock = async () => {
    const id = parseInt(lockId, 10);
    const amt = lockAmount ? BigInt(Math.round(parseFloat(lockAmount) * 1e12)) : BigInt(0);
    if (isNaN(id) || !lockPayer || amt <= 0n) return;
    await lock(id, lockPayer, amt);
    setLockDialogOpen(false);
    setLockId("");
    setLockPayer("");
    setLockAmount("");
    refetch();
  };

  const handleRelease = async () => {
    if (!escrow || !releaseTo.trim()) return;
    await release(escrow.id, releaseTo.trim());
    setReleaseTo("");
    refetch();
  };

  const handleRefund = async () => {
    if (!escrow || !refundTo.trim()) return;
    await refund(escrow.id, refundTo.trim());
    setRefundTo("");
    refetch();
  };

  const handleDispute = async () => {
    if (!escrow) return;
    await dispute(escrow.id, parseInt(disputeReason, 10), disputeDetail);
    setDisputeReason("0");
    setDisputeDetail("");
    refetch();
  };

  const handleForceRelease = async () => {
    if (!escrow || !forceReleaseTo.trim()) return;
    await forceRelease(escrow.id, forceReleaseTo.trim());
    setForceReleaseTo("");
    refetch();
  };

  const handleForceRefund = async () => {
    if (!escrow || !forceRefundTo.trim()) return;
    await forceRefund(escrow.id, forceRefundTo.trim());
    setForceRefundTo("");
    refetch();
  };

  const stateLabel = escrow ? ESCROW_STATE_MAP[escrow.state] ?? `State ${escrow.state}` : "";
  const canRelease = escrow?.state === 0;
  const canRefund = escrow?.state === 0;
  const canDispute = escrow?.state === 0;
  const canForce = escrow && (escrow.state === 0 || escrow.state === 1);

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/dispute">
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Lock className="h-7 w-7" />
            Escrow Funds
          </h1>
          <p className="text-muted-foreground">
            Look up escrow records and manage locked funds
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={() => { refetch(); refetchPaused(); }}>
          <RotateCcw className="mr-2 h-3 w-3" />
          Refresh
        </Button>
        <Button onClick={() => setLockDialogOpen(true)}>
          <Plus className="mr-2 h-4 w-4" />
          Lock New Escrow
        </Button>
      </div>

      {paused && (
        <div className="flex items-center gap-3 rounded-lg border border-amber-500/50 bg-amber-500/10 px-4 py-3">
          <AlertTriangle className="h-5 w-5 text-amber-600" />
          <p className="font-medium text-amber-800 dark:text-amber-200">
            Escrow pallet is paused. Lock, release, and refund operations are disabled.
          </p>
        </div>
      )}

      <Card>
        <CardHeader>
          <CardTitle>Lookup Escrow</CardTitle>
          <CardDescription>Enter an escrow ID to view details and perform actions</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex gap-2">
            <div className="relative flex-1">
              <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <Input
                type="number"
                placeholder="Escrow ID"
                className="pl-9"
                value={lookupId}
                onChange={(e) => setLookupId(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleLookup()}
              />
            </div>
            <Button variant="secondary" onClick={handleLookup}>
              Lookup
            </Button>
          </div>

          {lookupIdNum !== null && (
            <>
              {isLoading ? (
                <div className="flex items-center justify-center py-12">
                  <div className="h-8 w-8 animate-spin rounded-full border-2 border-primary border-t-transparent" />
                </div>
              ) : !escrow ? (
                <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
                  <Shield className="h-12 w-12 opacity-50" />
                  <p className="mt-4 font-medium">No escrow found</p>
                  <p className="text-sm">Escrow ID #{lookupIdNum} does not exist or has no locked amount</p>
                </div>
              ) : (
                <div className="space-y-6 rounded-lg border bg-muted/30 p-4">
                  <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                    <div>
                      <p className="text-sm text-muted-foreground">Amount</p>
                      <p className="text-lg font-mono font-semibold flex items-center gap-1.5">
                        <Wallet className="h-4 w-4 text-amber-500" />
                        {formatNex(escrow.amount)} NEX
                      </p>
                    </div>
                    <div>
                      <p className="text-sm text-muted-foreground">State</p>
                      <StatusBadge status={stateLabel} />
                    </div>
                    <div>
                      <p className="text-sm text-muted-foreground">Nonce</p>
                      <p className="font-mono">{escrow.nonce}</p>
                    </div>
                    <div>
                      <p className="text-sm text-muted-foreground">Expires At</p>
                      <p className="text-sm">{formatTimestamp(escrow.expiresAt)}</p>
                    </div>
                    <div>
                      <p className="text-sm text-muted-foreground">Disputed At</p>
                      <p className="text-sm">
                        {escrow.disputedAt != null
                          ? formatTimestamp(escrow.disputedAt)
                          : "—"}
                      </p>
                    </div>
                  </div>

                  <div className="space-y-4 border-t pt-4">
                    <h4 className="font-medium">Actions</h4>
                    {txState.error && (
                      <p className="text-sm text-destructive">{txState.error}</p>
                    )}
                    <div className="flex flex-wrap gap-2">
                      {canRelease && (
                        <div className="flex items-center gap-2">
                          <Input
                            placeholder="Release to address"
                            className="w-48 font-mono text-sm"
                            value={releaseTo}
                            onChange={(e) => setReleaseTo(e.target.value)}
                          />
                          <TxButton
                            txStatus={txState.status}
                            onClick={handleRelease}
                            disabled={!releaseTo.trim() || paused}
                            size="sm"
                          >
                            <Send className="mr-1 h-3 w-3" />
                            Release
                          </TxButton>
                        </div>
                      )}
                      {canRefund && (
                        <div className="flex items-center gap-2">
                          <Input
                            placeholder="Refund to address"
                            className="w-48 font-mono text-sm"
                            value={refundTo}
                            onChange={(e) => setRefundTo(e.target.value)}
                          />
                          <TxButton
                            txStatus={txState.status}
                            onClick={handleRefund}
                            disabled={!refundTo.trim() || paused}
                            variant="outline"
                            size="sm"
                          >
                            <RefundIcon className="mr-1 h-3 w-3" />
                            Refund
                          </TxButton>
                        </div>
                      )}
                      {canDispute && (
                        <div className="flex items-center gap-2">
                          <Input
                            type="number"
                            placeholder="Reason"
                            className="w-20"
                            value={disputeReason}
                            onChange={(e) => setDisputeReason(e.target.value)}
                          />
                          <Input
                            placeholder="Detail"
                            className="w-40"
                            value={disputeDetail}
                            onChange={(e) => setDisputeDetail(e.target.value)}
                          />
                          <TxButton
                            txStatus={txState.status}
                            onClick={handleDispute}
                            disabled={paused}
                            variant="outline"
                            size="sm"
                          >
                            <Gavel className="mr-1 h-3 w-3" />
                            Dispute
                          </TxButton>
                        </div>
                      )}
                      {canForce && (
                        <>
                          <div className="flex items-center gap-2">
                            <Input
                              placeholder="Force release to"
                              className="w-48 font-mono text-sm"
                              value={forceReleaseTo}
                              onChange={(e) => setForceReleaseTo(e.target.value)}
                            />
                            <TxButton
                              txStatus={txState.status}
                              onClick={handleForceRelease}
                              disabled={!forceReleaseTo.trim() || paused}
                              variant="destructive"
                              size="sm"
                            >
                              <ShieldAlert className="mr-1 h-3 w-3" />
                              Force Release
                            </TxButton>
                          </div>
                          <div className="flex items-center gap-2">
                            <Input
                              placeholder="Force refund to"
                              className="w-48 font-mono text-sm"
                              value={forceRefundTo}
                              onChange={(e) => setForceRefundTo(e.target.value)}
                            />
                            <TxButton
                              txStatus={txState.status}
                              onClick={handleForceRefund}
                              disabled={!forceRefundTo.trim() || paused}
                              variant="destructive"
                              size="sm"
                            >
                              <ShieldAlert className="mr-1 h-3 w-3" />
                              Force Refund
                            </TxButton>
                          </div>
                        </>
                      )}
                      <Button variant="ghost" size="sm" onClick={resetTx}>
                        Reset
                      </Button>
                    </div>
                  </div>
                </div>
              )}
            </>
          )}
        </CardContent>
      </Card>

      <Dialog open={lockDialogOpen} onOpenChange={setLockDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Lock New Escrow</DialogTitle>
            <DialogDescription>
              Create a new escrow lock. The payer will need to approve the transfer.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <label htmlFor="lock-id" className="text-sm font-medium">Escrow ID</label>
              <Input
                id="lock-id"
                type="number"
                placeholder="e.g. 1"
                value={lockId}
                onChange={(e) => setLockId(e.target.value)}
              />
            </div>
            <div className="grid gap-2">
              <label htmlFor="lock-payer" className="text-sm font-medium">Payer Address</label>
              <Input
                id="lock-payer"
                placeholder="5GrwvaEF..."
                value={lockPayer}
                onChange={(e) => setLockPayer(e.target.value)}
              />
            </div>
            <div className="grid gap-2">
              <label htmlFor="lock-amount" className="text-sm font-medium">Amount (NEX)</label>
              <Input
                id="lock-amount"
                type="number"
                step="0.0001"
                placeholder="e.g. 100"
                value={lockAmount}
                onChange={(e) => setLockAmount(e.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setLockDialogOpen(false)}>
              Cancel
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleLock}
              disabled={
                !lockId ||
                !lockPayer.trim() ||
                !lockAmount ||
                parseFloat(lockAmount) <= 0 ||
                paused
              }
            >
              Lock
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
