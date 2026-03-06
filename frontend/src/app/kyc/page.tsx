"use client";

import { useState, useMemo } from "react";
import { useEntityStore } from "@/stores/entity";
import { useKycRecords, useEntityKycRequirement, useKycActions } from "@/hooks/useKyc";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Select } from "@/components/ui/select";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
  DialogDescription, DialogFooter,
} from "@/components/ui/dialog";
import { KYC_LEVELS, KYC_STATUS, REJECTION_REASONS } from "@/lib/constants";
import {
  ShieldCheck, Clock, Users, CheckCircle, XCircle,
  RotateCcw, Timer, RefreshCw, ShieldAlert, TrendingUp,
} from "lucide-react";
import { useTranslations } from "next-intl";

const LEVEL_COLORS: Record<string, string> = {
  None: "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-400",
  Basic: "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400",
  Standard: "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400",
  Enhanced: "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400",
  Full: "bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400",
};

function RiskScoreBar({ score }: { score: number }) {
  const clamped = Math.max(0, Math.min(score, 100));
  const color = clamped < 30 ? "bg-green-500" : clamped <= 60 ? "bg-yellow-500" : "bg-red-500";
  const textColor = clamped < 30 ? "text-green-700" : clamped <= 60 ? "text-yellow-700" : "text-red-700";
  return (
    <div className="flex items-center gap-2">
      <div className="h-2 w-16 rounded-full bg-muted overflow-hidden">
        <div className={`h-full rounded-full transition-all ${color}`} style={{ width: `${clamped}%` }} />
      </div>
      <span className={`text-xs font-mono font-medium ${textColor}`}>{score}</span>
    </div>
  );
}

export default function KycPage() {
  const { currentEntityId } = useEntityStore();
  const { records, isLoading, refetch } = useKycRecords(currentEntityId);
  const { requirement } = useEntityKycRequirement(currentEntityId);
  const actions = useKycActions();
  const tc = useTranslations("common");

  const [statusFilter, setStatusFilter] = useState<string>("All");

  const [dialogType, setDialogType] = useState<string | null>(null);
  const [selectedAccount, setSelectedAccount] = useState("");

  const [approveRiskScore, setApproveRiskScore] = useState("0");
  const [rejectReason, setRejectReason] = useState<string>(REJECTION_REASONS[0]);
  const [rejectDetailsCid, setRejectDetailsCid] = useState("");
  const [revokeReason, setRevokeReason] = useState("");
  const [newRiskScore, setNewRiskScore] = useState("0");

  const filteredRecords = useMemo(() => {
    if (statusFilter === "All") return records;
    return records.filter((r) => r.status === statusFilter);
  }, [records, statusFilter]);

  const stats = useMemo(() => ({
    total: records.length,
    pending: records.filter((r) => r.status === "Pending").length,
    approved: records.filter((r) => r.status === "Approved").length,
    rejectedRevoked: records.filter((r) => r.status === "Rejected" || r.status === "Revoked").length,
  }), [records]);

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const openDialog = (type: string, account: string) => {
    setDialogType(type);
    setSelectedAccount(account);
    setApproveRiskScore("0");
    setRejectReason(REJECTION_REASONS[0]);
    setRejectDetailsCid("");
    setRevokeReason("");
    setNewRiskScore("0");
    actions.resetTx();
  };

  const closeDialog = () => {
    setDialogType(null);
    setSelectedAccount("");
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">KYC Management</h1>
        <p className="text-muted-foreground">Identity verification records and compliance</p>
      </div>

      {/* Stats */}
      <div className="grid gap-4 md:grid-cols-5">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium flex items-center gap-1.5">
              <Users className="h-4 w-4 text-muted-foreground" />Total Records
            </CardTitle>
          </CardHeader>
          <CardContent><p className="text-2xl font-bold">{stats.total}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium flex items-center gap-1.5">
              <Clock className="h-4 w-4 text-yellow-500" />Pending
            </CardTitle>
          </CardHeader>
          <CardContent><p className="text-2xl font-bold text-yellow-600">{stats.pending}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium flex items-center gap-1.5">
              <CheckCircle className="h-4 w-4 text-green-500" />Approved
            </CardTitle>
          </CardHeader>
          <CardContent><p className="text-2xl font-bold text-green-600">{stats.approved}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium flex items-center gap-1.5">
              <XCircle className="h-4 w-4 text-red-500" />Rejected / Revoked
            </CardTitle>
          </CardHeader>
          <CardContent><p className="text-2xl font-bold text-red-600">{stats.rejectedRevoked}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium flex items-center gap-1.5">
              <ShieldCheck className="h-4 w-4 text-primary" />Min KYC Level
            </CardTitle>
          </CardHeader>
          <CardContent><p className="text-2xl font-bold">{requirement?.minLevel ?? "—"}</p></CardContent>
        </Card>
      </div>

      {/* Status Filter */}
      <div className="flex flex-wrap gap-2">
        {["All", ...KYC_STATUS].map((status) => (
          <Button
            key={status}
            variant={statusFilter === status ? "default" : "outline"}
            size="sm"
            onClick={() => setStatusFilter(status)}
          >
            {status}
            {status !== "All" && (
              <span className="ml-1.5 rounded-full bg-background/20 px-1.5 text-xs">
                {records.filter((r) => r.status === status).length}
              </span>
            )}
          </Button>
        ))}
      </div>

      {/* Records */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle className="flex items-center gap-2">
              <ShieldCheck className="h-5 w-5" />KYC Records
            </CardTitle>
            <Button variant="outline" size="sm" onClick={refetch}>
              <RefreshCw className="mr-2 h-3.5 w-3.5" />Refresh
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="flex justify-center py-8">
              <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
            </div>
          ) : filteredRecords.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12">
              <ShieldCheck className="h-12 w-12 text-muted-foreground/50" />
              <p className="mt-4 text-lg font-medium">No KYC Records</p>
              <p className="text-sm text-muted-foreground">
                {statusFilter === "All"
                  ? "No KYC submissions found."
                  : `No records with status "${statusFilter}".`}
              </p>
            </div>
          ) : (
            <div className="space-y-3">
              {filteredRecords.map((record) => (
                <div key={record.account} className="rounded-lg border p-4 space-y-3">
                  <div className="flex flex-wrap items-center gap-3">
                    <AddressDisplay address={record.account} />

                    <Badge
                      className={LEVEL_COLORS[record.level] || LEVEL_COLORS.None}
                      variant="outline"
                    >
                      {record.level}
                    </Badge>

                    <StatusBadge status={record.status} />

                    {record.countryCode && (
                      <span className="text-xs font-mono bg-muted px-2 py-0.5 rounded">
                        {record.countryCode}
                      </span>
                    )}

                    <RiskScoreBar score={record.riskScore} />

                    {record.provider && (
                      <span className="text-xs text-muted-foreground">
                        Provider: <AddressDisplay address={record.provider} chars={4} showCopy={false} />
                      </span>
                    )}

                    <span className="text-xs text-muted-foreground ml-auto whitespace-nowrap">
                      <Clock className="mr-1 inline h-3 w-3" />
                      Block #{record.submittedAt}
                    </span>
                  </div>

                  {record.status === "Pending" && (
                    <div className="flex flex-wrap gap-2 pt-1 border-t">
                      <Button size="sm" onClick={() => openDialog("approve", record.account)}>
                        <CheckCircle className="mr-1.5 h-3.5 w-3.5" />Approve
                      </Button>
                      <Button
                        size="sm"
                        variant="destructive"
                        onClick={() => openDialog("reject", record.account)}
                      >
                        <XCircle className="mr-1.5 h-3.5 w-3.5" />Reject
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => actions.timeoutPendingKyc(currentEntityId, record.account)}
                      >
                        <Timer className="mr-1.5 h-3.5 w-3.5" />Timeout
                      </Button>
                    </div>
                  )}

                  {record.status === "Approved" && (
                    <div className="flex flex-wrap gap-2 pt-1 border-t">
                      <Button
                        size="sm"
                        variant="destructive"
                        onClick={() => openDialog("revoke", record.account)}
                      >
                        <ShieldAlert className="mr-1.5 h-3.5 w-3.5" />Revoke
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => openDialog("updateRisk", record.account)}
                      >
                        <TrendingUp className="mr-1.5 h-3.5 w-3.5" />Update Risk Score
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => actions.renewKyc(currentEntityId, record.account)}
                      >
                        <RotateCcw className="mr-1.5 h-3.5 w-3.5" />Renew
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => actions.expireKyc(currentEntityId, record.account)}
                      >
                        <Clock className="mr-1.5 h-3.5 w-3.5" />Expire
                      </Button>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Approve Dialog */}
      <Dialog open={dialogType === "approve"} onOpenChange={(o) => { if (!o) closeDialog(); }}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Approve KYC</DialogTitle>
            <DialogDescription>Approve identity verification for this account.</DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-1">
              <label className="text-sm font-medium">Account</label>
              <p className="text-sm font-mono text-muted-foreground break-all">{selectedAccount}</p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Risk Score (0–100)</label>
              <Input
                type="number"
                value={approveRiskScore}
                onChange={(e) => setApproveRiskScore(e.target.value)}
                min="0"
                max="100"
              />
              <RiskScoreBar score={Number(approveRiskScore)} />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={closeDialog}>Cancel</Button>
            <TxButton
              onClick={() => actions.approveKyc(currentEntityId, selectedAccount, Number(approveRiskScore))}
              txStatus={actions.txState.status}
            >
              <CheckCircle className="mr-2 h-4 w-4" />Approve
            </TxButton>
          </DialogFooter>
          {actions.txState.status === "finalized" && (
            <p className="text-sm text-green-600">KYC approved successfully.</p>
          )}
          {actions.txState.status === "error" && (
            <p className="text-sm text-destructive">{actions.txState.error}</p>
          )}
        </DialogContent>
      </Dialog>

      {/* Reject Dialog */}
      <Dialog open={dialogType === "reject"} onOpenChange={(o) => { if (!o) closeDialog(); }}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Reject KYC</DialogTitle>
            <DialogDescription>Reject this KYC submission with a reason.</DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-1">
              <label className="text-sm font-medium">Account</label>
              <p className="text-sm font-mono text-muted-foreground break-all">{selectedAccount}</p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Rejection Reason</label>
              <Select value={rejectReason} onChange={(e) => setRejectReason(e.target.value)}>
                {REJECTION_REASONS.map((reason) => (
                  <option key={reason} value={reason}>{reason}</option>
                ))}
              </Select>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Details CID (optional)</label>
              <Input
                value={rejectDetailsCid}
                onChange={(e) => setRejectDetailsCid(e.target.value)}
                placeholder="IPFS CID with rejection details"
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={closeDialog}>Cancel</Button>
            <TxButton
              variant="destructive"
              onClick={() => actions.rejectKyc(currentEntityId, selectedAccount, rejectReason, rejectDetailsCid || null)}
              txStatus={actions.txState.status}
            >
              <XCircle className="mr-2 h-4 w-4" />Reject
            </TxButton>
          </DialogFooter>
          {actions.txState.status === "finalized" && (
            <p className="text-sm text-green-600">KYC rejected.</p>
          )}
          {actions.txState.status === "error" && (
            <p className="text-sm text-destructive">{actions.txState.error}</p>
          )}
        </DialogContent>
      </Dialog>

      {/* Revoke Dialog */}
      <Dialog open={dialogType === "revoke"} onOpenChange={(o) => { if (!o) closeDialog(); }}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Revoke KYC</DialogTitle>
            <DialogDescription>Revoke an approved KYC verification.</DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-1">
              <label className="text-sm font-medium">Account</label>
              <p className="text-sm font-mono text-muted-foreground break-all">{selectedAccount}</p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Reason</label>
              <Input
                value={revokeReason}
                onChange={(e) => setRevokeReason(e.target.value)}
                placeholder="Reason for revocation"
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={closeDialog}>Cancel</Button>
            <TxButton
              variant="destructive"
              onClick={() => actions.revokeKyc(currentEntityId, selectedAccount, revokeReason)}
              txStatus={actions.txState.status}
              disabled={!revokeReason.trim()}
            >
              <ShieldAlert className="mr-2 h-4 w-4" />Revoke
            </TxButton>
          </DialogFooter>
          {actions.txState.status === "finalized" && (
            <p className="text-sm text-green-600">KYC revoked.</p>
          )}
          {actions.txState.status === "error" && (
            <p className="text-sm text-destructive">{actions.txState.error}</p>
          )}
        </DialogContent>
      </Dialog>

      {/* Update Risk Score Dialog */}
      <Dialog open={dialogType === "updateRisk"} onOpenChange={(o) => { if (!o) closeDialog(); }}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Update Risk Score</DialogTitle>
            <DialogDescription>Adjust the risk assessment score for this account.</DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-1">
              <label className="text-sm font-medium">Account</label>
              <p className="text-sm font-mono text-muted-foreground break-all">{selectedAccount}</p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">New Risk Score (0–100)</label>
              <Input
                type="number"
                value={newRiskScore}
                onChange={(e) => setNewRiskScore(e.target.value)}
                min="0"
                max="100"
              />
              <RiskScoreBar score={Number(newRiskScore)} />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={closeDialog}>Cancel</Button>
            <TxButton
              onClick={() => actions.updateRiskScore(currentEntityId, selectedAccount, Number(newRiskScore))}
              txStatus={actions.txState.status}
            >
              Update Score
            </TxButton>
          </DialogFooter>
          {actions.txState.status === "finalized" && (
            <p className="text-sm text-green-600">Risk score updated.</p>
          )}
          {actions.txState.status === "error" && (
            <p className="text-sm text-destructive">{actions.txState.error}</p>
          )}
        </DialogContent>
      </Dialog>

      {actions.txState.status === "finalized" && !dialogType && (
        <p className="text-sm text-green-600">Action completed successfully.</p>
      )}
      {actions.txState.status === "error" && !dialogType && (
        <p className="text-sm text-destructive">{actions.txState.error}</p>
      )}
    </div>
  );
}
