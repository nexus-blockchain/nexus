"use client";

import { useState, useEffect } from "react";
import { Card, CardContent } from "@/components/ui/card";
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
import {
  Cpu,
  ArrowLeft,
  RotateCcw,
  Plus,
  Pencil,
  Trash2,
  Ban,
  CheckCircle,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";
import { useOperators, useBotActions } from "@/hooks/useBot";
import { useWalletStore } from "@/stores/wallet";
import { formatNumber } from "@/lib/utils";

const PLATFORMS = ["Telegram", "Discord", "Slack", "Matrix", "Farcaster"];
function truncateAddress(addr: string, chars = 6): string {
  if (!addr) return "";
  return `${addr.slice(0, chars)}…${addr.slice(-chars)}`;
}

export default function RobotOperatorsPage() {
  const t = useTranslations("robotOperators");
  const address = useWalletStore((s) => s.address);
  const { operators, isLoading, refetch } = useOperators();
  const {
    registerOperator,
    updateOperator,
    deregisterOperator,
    suspendOperator,
    unsuspendOperator,
    txState,
    resetTx,
  } = useBotActions();

  const [registerOpen, setRegisterOpen] = useState(false);
  const [updateOpen, setUpdateOpen] = useState<{ account: string; platform: string } | null>(null);
  const [deregisterOp, setDeregisterOp] = useState<{ account: string; platform: string } | null>(null);
  const [suspendOp, setSuspendOp] = useState<{ account: string; platform: string } | null>(null);

  const [regPlatform, setRegPlatform] = useState(PLATFORMS[0]);
  const [regPlatformAppHash, setRegPlatformAppHash] = useState("");
  const [regName, setRegName] = useState("");
  const [regContact, setRegContact] = useState("");
  const [updName, setUpdName] = useState("");
  const [updContact, setUpdContact] = useState("");

  useEffect(() => {
    if (txState.status === "finalized") {
      refetch();
      resetTx();
      setRegisterOpen(false);
      setUpdateOpen(null);
      setDeregisterOp(null);
      setSuspendOp(null);
      setRegPlatformAppHash("");
      setRegName("");
      setRegContact("");
    }
  }, [txState.status, refetch, resetTx]);

  const handleRegister = () => {
    if (!regPlatform || !regPlatformAppHash.trim() || !regName.trim() || !regContact.trim())
      return;
    registerOperator(
      regPlatform,
      regPlatformAppHash.trim(),
      regName.trim(),
      regContact.trim()
    );
  };

  const handleUpdate = () => {
    if (!updateOpen) return;
    updateOperator(updateOpen.platform, updName.trim(), updContact.trim());
  };

  const handleDeregister = () => {
    if (!deregisterOp) return;
    deregisterOperator(deregisterOp.platform);
  };

  const handleSuspend = () => {
    if (!suspendOp) return;
    suspendOperator(suspendOp.account, suspendOp.platform);
  };

  const handleUnsuspend = (account: string, platform: string) => {
    unsuspendOperator(account, platform);
  };

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
            <Cpu className="h-7 w-7" />
            {t("title")}
          </h1>
          <p className="text-muted-foreground">{t("subtitle")}</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => refetch()}>
          <RotateCcw className="mr-2 h-3 w-3" />
          {t("refresh")}
        </Button>
        <Button onClick={() => setRegisterOpen(true)} disabled={!address}>
          <Plus className="mr-2 h-4 w-4" />
          {t("registerOperator")}
        </Button>
      </div>

      {!address && (
        <Card className="border-amber-500/50 bg-amber-500/5">
          <CardContent className="py-4">
            <p className="text-sm text-amber-600 dark:text-amber-400">
              {t("connectWallet")}
            </p>
          </CardContent>
        </Card>
      )}

      {isLoading ? (
        <Card>
          <CardContent className="flex items-center justify-center py-12">
            <div className="animate-pulse text-muted-foreground">{t("loading")}</div>
          </CardContent>
        </Card>
      ) : operators.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Cpu className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">{t("noOperators")}</p>
            <p className="text-sm text-muted-foreground">{t("noOperatorsDesc")}</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t("account")}</TableHead>
                <TableHead>{t("platform")}</TableHead>
                <TableHead>{t("name")}</TableHead>
                <TableHead>{t("contact")}</TableHead>
                <TableHead>{t("botCount")}</TableHead>
                <TableHead>{t("slaLevel")}</TableHead>
                <TableHead>{t("reputation")}</TableHead>
                <TableHead>{t("status")}</TableHead>
                <TableHead className="text-right">{t("actions")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {operators.map((op) => (
                <TableRow key={`${op.account}-${op.platform}`}>
                  <TableCell className="font-mono text-xs">
                    {truncateAddress(op.account)}
                  </TableCell>
                  <TableCell>{op.platform}</TableCell>
                  <TableCell>{op.name || "—"}</TableCell>
                  <TableCell className="max-w-[120px] truncate">
                    {op.contact || "—"}
                  </TableCell>
                  <TableCell>{formatNumber(op.botCount ?? 0)}</TableCell>
                  <TableCell>{formatNumber(op.slaLevel ?? 0)}</TableCell>
                  <TableCell>{formatNumber(op.reputationScore ?? 0)}</TableCell>
                  <TableCell>
                    <StatusBadge status={op.status} />
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex items-center justify-end gap-1">
                      {address === op.account && (
                        <TxButton
                          txStatus={txState.status}
                          variant="ghost"
                          size="sm"
                          onClick={() => {
                            setUpdateOpen({ account: op.account, platform: op.platform });
                            setUpdName(op.name || "");
                            setUpdContact(op.contact || "");
                          }}
                          title={t("updateOperator")}
                        >
                          <Pencil className="h-4 w-4" />
                        </TxButton>
                      )}
                      {address === op.account && op.botCount === 0 && (
                        <TxButton
                          txStatus={txState.status}
                          variant="ghost"
                          size="sm"
                          onClick={() =>
                            setDeregisterOp({ account: op.account, platform: op.platform })
                          }
                          title={t("deregister")}
                        >
                          <Trash2 className="h-4 w-4" />
                        </TxButton>
                      )}
                      {op.status === "Active" && (
                        <TxButton
                          txStatus={txState.status}
                          variant="ghost"
                          size="sm"
                          onClick={() =>
                            setSuspendOp({ account: op.account, platform: op.platform })
                          }
                          title={t("suspend")}
                        >
                          <Ban className="h-4 w-4" />
                        </TxButton>
                      )}
                      {op.status === "Suspended" && (
                        <TxButton
                          txStatus={txState.status}
                          variant="ghost"
                          size="sm"
                          onClick={() => handleUnsuspend(op.account, op.platform)}
                          loadingText={t("processing")}
                          title={t("unsuspend")}
                        >
                          <CheckCircle className="h-4 w-4" />
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

      {txState.status === "finalized" && (
        <Card className="border-green-500/50 bg-green-500/5">
          <CardContent className="py-3">
            <p className="text-sm text-green-600 dark:text-green-400">
              {t("success")}
            </p>
          </CardContent>
        </Card>
      )}
      {txState.status === "error" && txState.error && (
        <Card className="border-destructive/50 bg-destructive/5">
          <CardContent className="py-3">
            <p className="text-sm text-destructive">{txState.error}</p>
          </CardContent>
        </Card>
      )}

      {/* Register Operator Dialog */}
      <Dialog open={registerOpen} onOpenChange={setRegisterOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("registerOperator")}</DialogTitle>
            <DialogDescription>
              Register as a robot operator. Provide platform, app hash, name, and contact.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("platform")}</label>
              <select
                value={regPlatform}
                onChange={(e) => setRegPlatform(e.target.value)}
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              >
                {PLATFORMS.map((p) => (
                  <option key={p} value={p}>
                    {p}
                  </option>
                ))}
              </select>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("platformAppHash")}</label>
              <Input
                placeholder={t("platformAppHashPlaceholder")}
                value={regPlatformAppHash}
                onChange={(e) => setRegPlatformAppHash(e.target.value)}
                className="font-mono text-sm"
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("name")}</label>
              <Input
                placeholder="Operator name"
                value={regName}
                onChange={(e) => setRegName(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("contact")}</label>
              <Input
                placeholder="Contact (email, URL, etc.)"
                value={regContact}
                onChange={(e) => setRegContact(e.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setRegisterOpen(false)}>
              {t("cancel")}
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleRegister}
              disabled={
                !regPlatformAppHash.trim() || !regName.trim() || !regContact.trim()
              }
              loadingText={t("processing")}
            >
              {t("registerOperator")}
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Update Operator Dialog */}
      <Dialog open={!!updateOpen} onOpenChange={(o) => !o && setUpdateOpen(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("updateOperator")}</DialogTitle>
            <DialogDescription>
              Update operator name and contact.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("name")}</label>
              <Input
                placeholder="Operator name"
                value={updName}
                onChange={(e) => setUpdName(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("contact")}</label>
              <Input
                placeholder="Contact (email, URL, etc.)"
                value={updContact}
                onChange={(e) => setUpdContact(e.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setUpdateOpen(null)}>
              {t("cancel")}
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleUpdate}
              loadingText={t("processing")}
            >
              {t("updateOperator")}
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Deregister Confirmation Dialog */}
      <Dialog open={!!deregisterOp} onOpenChange={(o) => !o && setDeregisterOp(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("deregister")}</DialogTitle>
            <DialogDescription>
              Deregister this operator. Only allowed when bot count is 0.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeregisterOp(null)}>
              {t("cancel")}
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleDeregister}
              variant="destructive"
              loadingText={t("processing")}
            >
              {t("deregister")}
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Suspend Confirmation Dialog */}
      <Dialog open={!!suspendOp} onOpenChange={(o) => !o && setSuspendOp(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("suspend")}</DialogTitle>
            <DialogDescription>
              Suspend this operator. Admin action.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setSuspendOp(null)}>
              {t("cancel")}
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleSuspend}
              variant="destructive"
              loadingText={t("processing")}
            >
              {t("suspend")}
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
