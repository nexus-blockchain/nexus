"use client";

import { useState, useEffect, useMemo } from "react";
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
  DialogTrigger,
} from "@/components/ui/dialog";
import { Select } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import {
  Scale,
  Plus,
  Search,
  AlertTriangle,
  Clock,
  CheckCircle2,
  RotateCcw,
  Filter,
  MessageSquare,
  XCircle,
  ArrowUpCircle,
  Handshake,
  FilePlus,
  Loader2,
} from "lucide-react";
import { useTranslations } from "next-intl";
import {
  useComplaints,
  useArbitrationStats,
  useArbitrationPaused,
  useArbitrationActions,
} from "@/hooks/useArbitration";
import { useWalletStore } from "@/stores/wallet";
import {
  COMPLAINT_TYPE_CATEGORIES,
  COMPLAINT_STATUS,
} from "@/lib/constants";
import type { ComplaintData } from "@/lib/types";

const PENDING_STATUSES = ["Submitted", "Responded", "Mediating", "Arbitrating"];
const RESOLVED_STATUSES = [
  "ResolvedComplainantWin",
  "ResolvedRespondentWin",
  "ResolvedSettlement",
];

function formatAmount(amount: bigint | null): string {
  if (amount === null) return "—";
  const n = Number(amount) / 1e12;
  return n >= 1_000_000 ? `${(n / 1_000_000).toFixed(2)}M` : n.toFixed(4);
}

function formatBlock(block: number): string {
  return `#${block.toLocaleString()}`;
}

export default function DisputePage() {
  const t = useTranslations("dispute");
  const address = useWalletStore((s) => s.address);
  const { complaints, isLoading, refetch } = useComplaints();
  const { stats: arbitrationStats, isLoading: statsLoading } = useArbitrationStats();
  const { paused, refetch: refetchPaused } = useArbitrationPaused();
  const actions = useArbitrationActions();
  const { txState, resetTx } = actions;

  const [statusFilter, setStatusFilter] = useState<string>("All");
  const [searchQuery, setSearchQuery] = useState("");
  const [fileDialogOpen, setFileDialogOpen] = useState(false);
  const [actionDialog, setActionDialog] = useState<{
    type: "respond" | "withdraw" | "escalate" | "settle" | "supplement";
    complaint: ComplaintData;
  } | null>(null);

  // Form state for File Complaint
  const [fileDomain, setFileDomain] = useState("");
  const [fileObjectId, setFileObjectId] = useState("");
  const [fileComplaintType, setFileComplaintType] = useState("");
  const [fileDetailsCid, setFileDetailsCid] = useState("");
  const [fileAmount, setFileAmount] = useState("");

  // Form state for actions
  const [responseCid, setResponseCid] = useState("");
  const [settlementCid, setSettlementCid] = useState("");
  const [evidenceCid, setEvidenceCid] = useState("");

  const refetchAll = () => {
    refetch();
    refetchPaused();
  };

  useEffect(() => {
    if (txState.status === "finalized") {
      refetchAll();
      setFileDialogOpen(false);
      setActionDialog(null);
      resetTx();
    }
  }, [txState.status, resetTx]);

  const derivedStats = useMemo(() => {
    const total = complaints.length;
    const pending = complaints.filter((c) => PENDING_STATUSES.includes(c.status)).length;
    const resolved = complaints.filter((c) => RESOLVED_STATUSES.includes(c.status)).length;
    const withdrawn = complaints.filter((c) => c.status === "Withdrawn").length;
    const expired = complaints.filter((c) => c.status === "Expired").length;
    return { total, pending, resolved, withdrawn, expired };
  }, [complaints]);

  const filteredComplaints = useMemo(() => {
    let list = complaints;
    if (statusFilter !== "All") {
      list = list.filter((c) => c.status === statusFilter);
    }
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      list = list.filter(
        (c) =>
          String(c.id).includes(q) ||
          c.domain.toLowerCase().includes(q) ||
          c.complaintType.toLowerCase().includes(q) ||
          c.complainant.toLowerCase().includes(q) ||
          c.respondent.toLowerCase().includes(q)
      );
    }
    return list.sort((a, b) => b.id - a.id);
  }, [complaints, statusFilter, searchQuery]);

  const handleFileComplaint = async () => {
    if (!fileDomain.trim() || !fileObjectId || !fileComplaintType.trim() || !fileDetailsCid.trim()) return;
    const objectId = parseInt(fileObjectId, 10);
    if (isNaN(objectId)) return;
    const amount = fileAmount.trim() ? BigInt(fileAmount) : null;
    await actions.fileComplaint(
      fileDomain.trim(),
      objectId,
      fileComplaintType.trim(),
      fileDetailsCid.trim(),
      amount
    );
  };

  const handleRespond = async () => {
    if (!actionDialog || !responseCid.trim()) return;
    await actions.respondToComplaint(actionDialog.complaint.id, responseCid.trim());
  };

  const handleWithdraw = async () => {
    if (!actionDialog) return;
    await actions.withdrawComplaint(actionDialog.complaint.id);
  };

  const handleEscalate = async () => {
    if (!actionDialog) return;
    await actions.escalateToArbitration(actionDialog.complaint.id);
  };

  const handleSettle = async () => {
    if (!actionDialog || !settlementCid.trim()) return;
    await actions.settleComplaint(actionDialog.complaint.id, settlementCid.trim());
  };

  const handleSupplement = async () => {
    if (!actionDialog || !evidenceCid.trim()) return;
    const isComplainant = address === actionDialog.complaint.complainant;
    if (isComplainant) {
      await actions.supplementComplaintEvidence(actionDialog.complaint.id, evidenceCid.trim());
    } else {
      await actions.supplementResponseEvidence(actionDialog.complaint.id, evidenceCid.trim());
    }
  };

  const canRespond = (c: ComplaintData) =>
    c.status === "Submitted" && address && c.respondent.toLowerCase() === address.toLowerCase();
  const canWithdraw = (c: ComplaintData) =>
    c.status === "Submitted" && address && c.complainant.toLowerCase() === address.toLowerCase();
  const canEscalate = (c: ComplaintData) =>
    (c.status === "Responded" || c.status === "Mediating") && address;
  const canSettle = (c: ComplaintData) =>
    (c.status === "Responded" || c.status === "Mediating") && address;
  const canSupplement = (c: ComplaintData) =>
    PENDING_STATUSES.includes(c.status) &&
    address &&
    (c.complainant.toLowerCase() === address.toLowerCase() ||
      c.respondent.toLowerCase() === address.toLowerCase());

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Scale className="h-8 w-8" />
            {t("title")}
          </h1>
          <p className="text-muted-foreground">
            {t("subtitle")}
          </p>
        </div>
        <Dialog open={fileDialogOpen} onOpenChange={setFileDialogOpen}>
          <DialogTrigger asChild>
            <Button disabled={paused || !address}>
              <Plus className="mr-2 h-4 w-4" />
              {t("fileComplaint")}
            </Button>
          </DialogTrigger>
          <DialogContent className="max-w-md">
            <DialogHeader>
              <DialogTitle>{t("fileComplaint")}</DialogTitle>
              <DialogDescription>
                {t("fileComplaintDesc")}
              </DialogDescription>
            </DialogHeader>
            <div className="grid gap-4 py-4">
              <div>
                <label className="text-sm font-medium mb-1 block">
                  {t("domain")} (8 chars)
                </label>
                <Input
                  placeholder="e.g. nexmarket"
                  value={fileDomain}
                  onChange={(e) => setFileDomain(e.target.value)}
                  maxLength={8}
                />
              </div>
              <div>
                <label className="text-sm font-medium mb-1 block">
                  {t("objectId")}
                </label>
                <Input
                  type="number"
                  placeholder="Object ID"
                  value={fileObjectId}
                  onChange={(e) => setFileObjectId(e.target.value)}
                />
              </div>
              <div>
                <label className="text-sm font-medium mb-1 block">
                  {t("complaintType")}
                </label>
                <Select
                  value={fileComplaintType}
                  onChange={(e) => setFileComplaintType(e.target.value)}
                >
                  <option value="">{t("selectType")}</option>
                  {Object.entries(COMPLAINT_TYPE_CATEGORIES).map(([cat, types]) => (
                    <optgroup key={cat} label={cat}>
                      {types.map((type) => (
                        <option key={type} value={type}>
                          {type}
                        </option>
                      ))}
                    </optgroup>
                  ))}
                </Select>
              </div>
              <div>
                <label className="text-sm font-medium mb-1 block">
                  {t("detailsCid")}
                </label>
                <Input
                  placeholder="IPFS CID"
                  value={fileDetailsCid}
                  onChange={(e) => setFileDetailsCid(e.target.value)}
                />
              </div>
              <div>
                <label className="text-sm font-medium mb-1 block">
                  {t("amount")} ({t("optional")})
                </label>
                <Input
                  type="number"
                  placeholder="Amount in smallest unit"
                  value={fileAmount}
                  onChange={(e) => setFileAmount(e.target.value)}
                />
              </div>
            </div>
            {txState.status === "error" && (
              <p className="text-sm text-destructive">{txState.error}</p>
            )}
            <DialogFooter>
              <Button variant="outline" onClick={() => setFileDialogOpen(false)}>
                {t("cancel")}
              </Button>
              <TxButton
                txStatus={txState.status}
                onClick={handleFileComplaint}
                disabled={
                  fileDomain.trim().length !== 8 ||
                  !fileObjectId ||
                  !fileComplaintType ||
                  !fileDetailsCid.trim()
                }
              >
                {t("submit")}
              </TxButton>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </div>

      {paused && (
        <div className="flex items-center gap-2 rounded-lg border border-amber-500/50 bg-amber-500/10 px-4 py-3">
          <AlertTriangle className="h-5 w-5 text-amber-600" />
          <p className="font-medium text-amber-800 dark:text-amber-200">
            {t("paused")}
          </p>
        </div>
      )}

      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">{t("total")}</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            {statsLoading || isLoading ? (
              <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
            ) : (
              <p className="text-2xl font-bold">{derivedStats.total}</p>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">{t("pending")}</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            {statsLoading || isLoading ? (
              <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
            ) : (
              <>
                <Clock className="h-4 w-4 text-amber-500" />
                <p className="text-2xl font-bold text-amber-600">{derivedStats.pending}</p>
              </>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">{t("resolved")}</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            {statsLoading || isLoading ? (
              <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
            ) : (
              <>
                <CheckCircle2 className="h-4 w-4 text-green-500" />
                <p className="text-2xl font-bold text-green-600">{derivedStats.resolved}</p>
              </>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">{t("withdrawnExpired")}</CardTitle>
          </CardHeader>
          <CardContent>
            {statsLoading || isLoading ? (
              <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
            ) : (
              <p className="text-2xl font-bold">{derivedStats.withdrawn + derivedStats.expired}</p>
            )}
          </CardContent>
        </Card>
      </div>

      {arbitrationStats && (
        <div className="grid gap-4 md:grid-cols-4">
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm font-medium">{t("totalDisputes")}</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-xl font-bold">{arbitrationStats.totalDisputes}</p>
            </CardContent>
          </Card>
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm font-medium">{t("releaseCount")}</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-xl font-bold text-green-600">{arbitrationStats.releaseCount}</p>
            </CardContent>
          </Card>
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm font-medium">{t("refundCount")}</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-xl font-bold text-blue-600">{arbitrationStats.refundCount}</p>
            </CardContent>
          </Card>
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm font-medium">{t("partialCount")}</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-xl font-bold">{arbitrationStats.partialCount}</p>
            </CardContent>
          </Card>
        </div>
      )}

      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            placeholder={t("searchPlaceholder")}
            className="pl-9"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
        </div>
        <div className="flex items-center gap-1">
          <Filter className="h-4 w-4 text-muted-foreground" />
          <Select
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value)}
            className="w-[180px]"
          >
            <option value="All">{t("filterAll")}</option>
            {COMPLAINT_STATUS.map((s) => (
              <option key={s} value={s}>
                {s}
              </option>
            ))}
          </Select>
        </div>
        <Button variant="outline" size="sm" onClick={refetchAll} disabled={isLoading}>
          <RotateCcw className="mr-2 h-3 w-3" />
          {t("refresh")}
        </Button>
      </div>

      {txState.status === "error" && (
        <div className="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3">
          <p className="text-sm text-destructive">{txState.error}</p>
          <Button variant="ghost" size="sm" className="mt-2" onClick={resetTx}>
            {t("dismiss")}
          </Button>
        </div>
      )}

      {txState.status === "finalized" && (
        <div className="rounded-lg border border-green-500/50 bg-green-500/10 px-4 py-3">
          <p className="text-sm text-green-700 dark:text-green-400">{t("success")}</p>
        </div>
      )}

      {filteredComplaints.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Scale className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">{t("noComplaints")}</p>
            <p className="text-sm text-muted-foreground">{t("noComplaintsDesc")}</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>ID</TableHead>
                <TableHead>Domain</TableHead>
                <TableHead>Type</TableHead>
                <TableHead>Complainant</TableHead>
                <TableHead>Respondent</TableHead>
                <TableHead>Amount</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Created</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {filteredComplaints.map((complaint) => (
                <TableRow key={complaint.id}>
                  <TableCell className="font-mono">#{complaint.id}</TableCell>
                  <TableCell className="font-mono text-xs">{complaint.domain}</TableCell>
                  <TableCell>
                    <Badge variant="outline">{complaint.complaintType}</Badge>
                  </TableCell>
                  <TableCell>
                    <AddressDisplay address={complaint.complainant} chars={6} />
                  </TableCell>
                  <TableCell>
                    <AddressDisplay address={complaint.respondent} chars={6} />
                  </TableCell>
                  <TableCell>{formatAmount(complaint.amount)}</TableCell>
                  <TableCell>
                    <StatusBadge status={complaint.status} />
                  </TableCell>
                  <TableCell className="text-muted-foreground">
                    {formatBlock(complaint.createdAt)}
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex items-center justify-end gap-1 flex-wrap">
                      {canRespond(complaint) && (
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() =>
                            setActionDialog({ type: "respond", complaint })
                          }
                        >
                          <MessageSquare className="h-3.5 w-3.5 mr-1" />
                          {t("respond")}
                        </Button>
                      )}
                      {canWithdraw(complaint) && (
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() =>
                            setActionDialog({ type: "withdraw", complaint })
                          }
                        >
                          <XCircle className="h-3.5 w-3.5 mr-1" />
                          {t("withdraw")}
                        </Button>
                      )}
                      {canEscalate(complaint) && (
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() =>
                            setActionDialog({ type: "escalate", complaint })
                          }
                        >
                          <ArrowUpCircle className="h-3.5 w-3.5 mr-1" />
                          {t("escalate")}
                        </Button>
                      )}
                      {canSettle(complaint) && (
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() =>
                            setActionDialog({ type: "settle", complaint })
                          }
                        >
                          <Handshake className="h-3.5 w-3.5 mr-1" />
                          {t("settle")}
                        </Button>
                      )}
                      {canSupplement(complaint) && (
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() =>
                            setActionDialog({ type: "supplement", complaint })
                          }
                        >
                          <FilePlus className="h-3.5 w-3.5 mr-1" />
                          {t("supplement")}
                        </Button>
                      )}
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}

      {/* Action dialogs */}
      {actionDialog && (
        <Dialog
          open={!!actionDialog}
          onOpenChange={(open) => {
            if (!open) {
              setActionDialog(null);
              setResponseCid("");
              setSettlementCid("");
              setEvidenceCid("");
            }
          }}
        >
          <DialogContent className="max-w-md">
            <DialogHeader>
              <DialogTitle>
                {actionDialog.type === "respond" && t("respond")}
                {actionDialog.type === "withdraw" && t("withdraw")}
                {actionDialog.type === "escalate" && t("escalate")}
                {actionDialog.type === "settle" && t("settle")}
                {actionDialog.type === "supplement" && t("supplement")}
                {" — #"} {actionDialog.complaint.id}
              </DialogTitle>
              <DialogDescription>
                {actionDialog.type === "respond" && t("respondDesc")}
                {actionDialog.type === "withdraw" && t("withdrawDesc")}
                {actionDialog.type === "escalate" && t("escalateDesc")}
                {actionDialog.type === "settle" && t("settleDesc")}
                {actionDialog.type === "supplement" && t("supplementDesc")}
              </DialogDescription>
            </DialogHeader>
            <div className="grid gap-4 py-4">
              {actionDialog.type === "respond" && (
                <div>
                  <label className="text-sm font-medium mb-1 block">
                    {t("responseCid")}
                  </label>
                  <Input
                    placeholder="IPFS CID"
                    value={responseCid}
                    onChange={(e) => setResponseCid(e.target.value)}
                  />
                </div>
              )}
              {actionDialog.type === "settle" && (
                <div>
                  <label className="text-sm font-medium mb-1 block">
                    {t("settlementCid")}
                  </label>
                  <Input
                    placeholder="IPFS CID"
                    value={settlementCid}
                    onChange={(e) => setSettlementCid(e.target.value)}
                  />
                </div>
              )}
              {actionDialog.type === "supplement" && (
                <div>
                  <label className="text-sm font-medium mb-1 block">
                    {t("evidenceCid")}
                  </label>
                  <Input
                    placeholder="IPFS CID"
                    value={evidenceCid}
                    onChange={(e) => setEvidenceCid(e.target.value)}
                  />
                </div>
              )}
            </div>
            {txState.status === "error" && (
              <p className="text-sm text-destructive">{txState.error}</p>
            )}
            <DialogFooter>
              <Button variant="outline" onClick={() => setActionDialog(null)}>
                {t("cancel")}
              </Button>
              <TxButton
                txStatus={txState.status}
                onClick={async () => {
                  if (actionDialog.type === "respond") await handleRespond();
                  else if (actionDialog.type === "withdraw") await handleWithdraw();
                  else if (actionDialog.type === "escalate") await handleEscalate();
                  else if (actionDialog.type === "settle") await handleSettle();
                  else if (actionDialog.type === "supplement") await handleSupplement();
                }}
                disabled={
                  (actionDialog.type === "respond" && !responseCid.trim()) ||
                  (actionDialog.type === "settle" && !settlementCid.trim()) ||
                  (actionDialog.type === "supplement" && !evidenceCid.trim())
                }
              >
                {t("submit")}
              </TxButton>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      )}
    </div>
  );
}
