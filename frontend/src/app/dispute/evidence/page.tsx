"use client";

import { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
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
  FileText,
  ArrowLeft,
  Upload,
  RotateCcw,
  Lock,
  Unlock,
  Trash2,
  Plus,
  FileImage,
  FileVideo,
  File,
} from "lucide-react";
import Link from "next/link";
import {
  useEvidenceList,
  useEvidenceActions,
} from "@/hooks/useEvidence";
import { EVIDENCE_CONTENT_TYPES } from "@/lib/constants";
import { useWalletStore } from "@/stores/wallet";
import type { EvidenceData } from "@/lib/types";

const parseCids = (s: string) =>
  s
    .split(/[\n,]+/)
    .map((x) => x.trim())
    .filter(Boolean);

const formatTimestamp = (ts: number) => {
  if (ts < 1e12) return `Block ${ts}`;
  return new Date(ts * 1000).toLocaleString();
};

export default function DisputeEvidencePage() {
  const address = useWalletStore((s) => s.address);

  const { evidences, isLoading, refetch } = useEvidenceList();
  const {
    commit,
    appendEvidence,
    sealEvidence,
    unsealEvidence,
    withdrawEvidence,
    txState,
    resetTx,
  } = useEvidenceActions();

  const [submitDialogOpen, setSubmitDialogOpen] = useState(false);
  const [appendDialogEvidence, setAppendDialogEvidence] = useState<EvidenceData | null>(null);

  const [domain, setDomain] = useState("");
  const [targetId, setTargetId] = useState("");
  const [imageCids, setImageCids] = useState("");
  const [videoCids, setVideoCids] = useState("");
  const [docCids, setDocCids] = useState("");
  const [memo, setMemo] = useState("");

  const [appendImgs, setAppendImgs] = useState("");
  const [appendVids, setAppendVids] = useState("");
  const [appendDocs, setAppendDocs] = useState("");
  const [appendMemo, setAppendMemo] = useState("");

  const handleSubmit = async () => {
    const d = parseInt(domain, 10);
    const t = parseInt(targetId, 10);
    if (isNaN(d) || isNaN(t)) return;
    const imgs = parseCids(imageCids);
    const vids = parseCids(videoCids);
    const docs = parseCids(docCids);
    if (imgs.length === 0 && vids.length === 0 && docs.length === 0) return;
    await commit(d, t, imgs, vids, docs, memo.trim() || null);
    setSubmitDialogOpen(false);
    setDomain("");
    setTargetId("");
    setImageCids("");
    setVideoCids("");
    setDocCids("");
    setMemo("");
    refetch();
  };

  const handleAppend = async () => {
    if (!appendDialogEvidence) return;
    const imgs = parseCids(appendImgs);
    const vids = parseCids(appendVids);
    const docs = parseCids(appendDocs);
    if (imgs.length === 0 && vids.length === 0 && docs.length === 0) return;
    await appendEvidence(
      appendDialogEvidence.id,
      imgs,
      vids,
      docs,
      appendMemo.trim() || null
    );
    setAppendDialogEvidence(null);
    setAppendImgs("");
    setAppendVids("");
    setAppendDocs("");
    setAppendMemo("");
    refetch();
  };

  const handleSeal = async (id: number) => {
    await sealEvidence(id);
    refetch();
  };

  const handleUnseal = async (id: number) => {
    await unsealEvidence(id);
    refetch();
  };

  const handleWithdraw = async (id: number) => {
    await withdrawEvidence(id);
    refetch();
  };

  const isOwner = (owner: string) => address && owner === address;

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
            <FileText className="h-7 w-7" />
            Evidence
          </h1>
          <p className="text-muted-foreground">
            Submit and manage evidence for dispute cases
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={refetch}>
          <RotateCcw className="mr-2 h-3 w-3" />
          Refresh
        </Button>
        <Button onClick={() => setSubmitDialogOpen(true)}>
          <Upload className="mr-2 h-4 w-4" />
          Submit Evidence
        </Button>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Evidence Records</CardTitle>
          <CardDescription>
            All evidence submitted on-chain. Seal, unseal, withdraw, or append based on status.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="flex items-center justify-center py-12">
              <div className="h-8 w-8 animate-spin rounded-full border-2 border-primary border-t-transparent" />
            </div>
          ) : evidences.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12">
              <FileText className="h-12 w-12 text-muted-foreground/50" />
              <p className="mt-4 text-lg font-medium">No evidence submitted</p>
              <p className="text-sm text-muted-foreground">
                Submit evidence to support your dispute case
              </p>
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>ID</TableHead>
                  <TableHead>Domain</TableHead>
                  <TableHead>Target</TableHead>
                  <TableHead>Content Type</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Created</TableHead>
                  <TableHead>Encrypted</TableHead>
                  <TableHead>Owner</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {evidences.map((ev) => (
                  <TableRow key={ev.id}>
                    <TableCell className="font-mono">#{ev.id}</TableCell>
                    <TableCell className="font-mono">{ev.domain}</TableCell>
                    <TableCell className="font-mono">#{ev.targetId}</TableCell>
                    <TableCell>
                      <span className="text-sm">
                        {EVIDENCE_CONTENT_TYPES.includes(ev.contentType as any)
                          ? ev.contentType
                          : ev.contentType || "—"}
                      </span>
                    </TableCell>
                    <TableCell>
                      <StatusBadge status={ev.status} />
                    </TableCell>
                    <TableCell className="text-muted-foreground text-sm">
                      {formatTimestamp(ev.createdAt)}
                    </TableCell>
                    <TableCell>
                      {ev.isEncrypted ? (
                        <Lock className="h-4 w-4 text-amber-500" />
                      ) : (
                        <span className="text-muted-foreground">—</span>
                      )}
                    </TableCell>
                    <TableCell>
                      <AddressDisplay address={ev.owner} chars={6} />
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-1">
                        {ev.status === "Active" && (
                          <>
                            <TxButton
                              txStatus={txState.status}
                              variant="ghost"
                              size="sm"
                              onClick={() => handleSeal(ev.id)}
                            >
                              <Lock className="h-3 w-3" />
                            </TxButton>
                            {isOwner(ev.owner) && (
                              <TxButton
                                txStatus={txState.status}
                                variant="ghost"
                                size="sm"
                                onClick={() => handleWithdraw(ev.id)}
                              >
                                <Trash2 className="h-3 w-3 text-destructive" />
                              </TxButton>
                            )}
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => setAppendDialogEvidence(ev)}
                            >
                              <Plus className="h-3 w-3" />
                            </Button>
                          </>
                        )}
                        {ev.status === "Sealed" && (
                          <TxButton
                            txStatus={txState.status}
                            variant="ghost"
                            size="sm"
                            onClick={() => handleUnseal(ev.id)}
                          >
                            <Unlock className="h-3 w-3" />
                          </TxButton>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      <Dialog open={submitDialogOpen} onOpenChange={setSubmitDialogOpen}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>Submit Evidence</DialogTitle>
            <DialogDescription>
              Commit new evidence for a domain and target. Provide at least one CID (image, video, or document).
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="grid grid-cols-2 gap-4">
              <div className="grid gap-2">
                <label htmlFor="domain" className="text-sm font-medium">Domain</label>
                <Input
                  id="domain"
                  type="number"
                  placeholder="e.g. 0"
                  value={domain}
                  onChange={(e) => setDomain(e.target.value)}
                />
              </div>
              <div className="grid gap-2">
                <label htmlFor="targetId" className="text-sm font-medium">Target ID</label>
                <Input
                  id="targetId"
                  type="number"
                  placeholder="e.g. 1"
                  value={targetId}
                  onChange={(e) => setTargetId(e.target.value)}
                />
              </div>
            </div>
            <div className="grid gap-2">
              <label className="flex items-center gap-1.5 text-sm font-medium">
                <FileImage className="h-4 w-4" />
                Image CIDs (comma or newline separated)
              </label>
              <Textarea
                placeholder="QmYwAPJz..."
                value={imageCids}
                onChange={(e) => setImageCids(e.target.value)}
                rows={2}
                className="font-mono text-sm"
              />
            </div>
            <div className="grid gap-2">
              <label className="flex items-center gap-1.5 text-sm font-medium">
                <FileVideo className="h-4 w-4" />
                Video CIDs (comma or newline separated)
              </label>
              <Textarea
                placeholder="QmT5NvUt..."
                value={videoCids}
                onChange={(e) => setVideoCids(e.target.value)}
                rows={2}
                className="font-mono text-sm"
              />
            </div>
            <div className="grid gap-2">
              <label className="flex items-center gap-1.5 text-sm font-medium">
                <File className="h-4 w-4" />
                Document CIDs (comma or newline separated)
              </label>
              <Textarea
                placeholder="QmPZ9gcC..."
                value={docCids}
                onChange={(e) => setDocCids(e.target.value)}
                rows={2}
                className="font-mono text-sm"
              />
            </div>
            <div className="grid gap-2">
              <label htmlFor="memo" className="text-sm font-medium">Memo (optional)</label>
              <Input
                id="memo"
                placeholder="Brief description"
                value={memo}
                onChange={(e) => setMemo(e.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setSubmitDialogOpen(false)}>
              Cancel
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleSubmit}
              disabled={
                !domain ||
                !targetId ||
                (parseCids(imageCids).length === 0 &&
                  parseCids(videoCids).length === 0 &&
                  parseCids(docCids).length === 0)
              }
            >
              Submit
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={!!appendDialogEvidence}
        onOpenChange={(open) => !open && setAppendDialogEvidence(null)}
      >
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>Append Evidence</DialogTitle>
            <DialogDescription>
              Add more content to evidence #{appendDialogEvidence?.id}. Provide at least one CID.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <label className="flex items-center gap-1.5 text-sm font-medium">
                <FileImage className="h-4 w-4" />
                Image CIDs
              </label>
              <Textarea
                placeholder="QmYwAPJz..."
                value={appendImgs}
                onChange={(e) => setAppendImgs(e.target.value)}
                rows={2}
                className="font-mono text-sm"
              />
            </div>
            <div className="grid gap-2">
              <label className="flex items-center gap-1.5 text-sm font-medium">
                <FileVideo className="h-4 w-4" />
                Video CIDs
              </label>
              <Textarea
                placeholder="QmT5NvUt..."
                value={appendVids}
                onChange={(e) => setAppendVids(e.target.value)}
                rows={2}
                className="font-mono text-sm"
              />
            </div>
            <div className="grid gap-2">
              <label className="flex items-center gap-1.5 text-sm font-medium">
                <File className="h-4 w-4" />
                Document CIDs
              </label>
              <Textarea
                placeholder="QmPZ9gcC..."
                value={appendDocs}
                onChange={(e) => setAppendDocs(e.target.value)}
                rows={2}
                className="font-mono text-sm"
              />
            </div>
            <div className="grid gap-2">
              <label htmlFor="append-memo" className="text-sm font-medium">Memo (optional)</label>
              <Input
                id="append-memo"
                placeholder="Brief description"
                value={appendMemo}
                onChange={(e) => setAppendMemo(e.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setAppendDialogEvidence(null)}>
              Cancel
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleAppend}
              disabled={
                parseCids(appendImgs).length === 0 &&
                parseCids(appendVids).length === 0 &&
                parseCids(appendDocs).length === 0
              }
            >
              Append
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
