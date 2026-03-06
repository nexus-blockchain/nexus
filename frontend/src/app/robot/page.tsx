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
  Bot,
  Plus,
  RotateCcw,
  Key,
  Power,
  PowerOff,
  Send,
  Link2,
} from "lucide-react";
import { useTranslations } from "next-intl";
import { useBots, useBotCount, useBotActions } from "@/hooks/useBot";
import { useWalletStore } from "@/stores/wallet";
import { formatNumber } from "@/lib/utils";

const PLATFORMS = ["Telegram", "Discord", "Slack", "Matrix", "Farcaster"];

function truncateHash(hash: string, chars = 8): string {
  if (!hash) return "";
  const clean = hash.startsWith("0x") ? hash.slice(2) : hash;
  if (clean.length <= chars * 2) return hash;
  return `0x${clean.slice(0, chars)}…${clean.slice(-chars)}`;
}

export default function RobotPage() {
  const t = useTranslations("robot");
  const address = useWalletStore((s) => s.address);
  const { bots, isLoading, refetch } = useBots(address);
  const { count: totalCount, refetch: refetchCount } = useBotCount();
  const {
    registerBot,
    updatePublicKey,
    deactivateBot,
    reactivateBot,
    transferBotOwnership,
    bindCommunity,
    txState,
    resetTx,
  } = useBotActions();

  const [registerOpen, setRegisterOpen] = useState(false);
  const [bindOpen, setBindOpen] = useState(false);
  const [updateKeyBot, setUpdateKeyBot] = useState<string | null>(null);
  const [transferBot, setTransferBot] = useState<string | null>(null);

  const [regBotIdHash, setRegBotIdHash] = useState("");
  const [regPublicKey, setRegPublicKey] = useState("");
  const [bindBotId, setBindBotId] = useState("");
  const [bindCommunityId, setBindCommunityId] = useState("");
  const [bindPlatform, setBindPlatform] = useState(PLATFORMS[0]);
  const [newPublicKey, setNewPublicKey] = useState("");
  const [newOwner, setNewOwner] = useState("");

  useEffect(() => {
    if (txState.status === "finalized") {
      refetch();
      refetchCount();
      resetTx();
      setRegisterOpen(false);
      setBindOpen(false);
      setUpdateKeyBot(null);
      setTransferBot(null);
      setRegBotIdHash("");
      setRegPublicKey("");
      setNewPublicKey("");
      setNewOwner("");
      setBindBotId("");
      setBindCommunityId("");
    }
  }, [txState.status, refetch, refetchCount, resetTx]);

  const activeCount = bots.filter((b) => b.status === "Active").length;
  const suspendedCount = bots.filter((b) => b.status === "Suspended").length;
  const deactivatedCount = bots.filter((b) => b.status === "Deactivated").length;

  const handleRegister = () => {
    if (!regBotIdHash.trim() || !regPublicKey.trim()) return;
    registerBot(regBotIdHash.trim(), regPublicKey.trim());
  };

  const handleUpdateKey = () => {
    if (!updateKeyBot || !newPublicKey.trim()) return;
    updatePublicKey(updateKeyBot, newPublicKey.trim());
  };

  const handleTransfer = () => {
    if (!transferBot || !newOwner.trim()) return;
    transferBotOwnership(transferBot, newOwner.trim());
  };

  const handleBind = () => {
    if (!bindBotId.trim() || !bindCommunityId.trim()) return;
    bindCommunity(bindBotId.trim(), bindCommunityId.trim(), bindPlatform);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Bot className="h-8 w-8" />
            {t("title")}
          </h1>
          <p className="text-muted-foreground">{t("subtitle")}</p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            onClick={() => {
              setBindOpen(true);
              setBindBotId(bots[0]?.botIdHash ?? "");
              setBindCommunityId("");
              setBindPlatform(PLATFORMS[0]);
            }}
            disabled={!address || bots.length === 0}
          >
            <Link2 className="mr-2 h-4 w-4" />
            {t("bindCommunity")}
          </Button>
          <Button onClick={() => setRegisterOpen(true)} disabled={!address}>
            <Plus className="mr-2 h-4 w-4" />
            {t("registerBot")}
          </Button>
        </div>
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

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-5">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">{t("totalBots")}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{formatNumber(totalCount)}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">{t("myBots")}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{formatNumber(bots.length)}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">{t("active")}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-green-600">
              {formatNumber(activeCount)}
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">{t("suspended")}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-amber-600">
              {formatNumber(suspendedCount)}
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">{t("deactivated")}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-muted-foreground">
              {formatNumber(deactivatedCount)}
            </p>
          </CardContent>
        </Card>
      </div>

      <div className="flex items-center gap-2">
        <Button variant="outline" size="sm" onClick={() => { refetch(); refetchCount(); }}>
          <RotateCcw className="mr-2 h-3 w-3" />
          {t("refresh")}
        </Button>
      </div>

      {isLoading ? (
        <Card>
          <CardContent className="flex items-center justify-center py-12">
            <div className="animate-pulse text-muted-foreground">{t("loading")}</div>
          </CardContent>
        </Card>
      ) : bots.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Bot className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">{t("noBots")}</p>
            <p className="text-sm text-muted-foreground">{t("noBotsDesc")}</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t("botIdHashCol")}</TableHead>
                <TableHead>{t("status")}</TableHead>
                <TableHead>{t("nodeType")}</TableHead>
                <TableHead>{t("communities")}</TableHead>
                <TableHead>{t("registered")}</TableHead>
                <TableHead className="text-right">{t("actions")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {bots.map((bot) => (
                <TableRow key={bot.botIdHash}>
                  <TableCell className="font-mono text-xs">
                    {truncateHash(bot.botIdHash)}
                  </TableCell>
                  <TableCell>
                    <StatusBadge status={bot.status} />
                  </TableCell>
                  <TableCell>{bot.nodeType || "—"}</TableCell>
                  <TableCell>{formatNumber(bot.communityCount ?? 0)}</TableCell>
                  <TableCell className="text-muted-foreground">
                    #{formatNumber(bot.registeredAt ?? 0)}
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex items-center justify-end gap-1">
                      <TxButton
                        txStatus={txState.status}
                        variant="ghost"
                        size="sm"
                        onClick={() => {
                          setUpdateKeyBot(bot.botIdHash);
                          setNewPublicKey("");
                        }}
                      >
                        <Key className="h-4 w-4" />
                      </TxButton>
                      {bot.status === "Active" && (
                        <>
                          <TxButton
                            txStatus={txState.status}
                            variant="ghost"
                            size="sm"
                            onClick={() => deactivateBot(bot.botIdHash)}
                            loadingText={t("processing")}
                          >
                            <PowerOff className="h-4 w-4" />
                          </TxButton>
                          <TxButton
                            txStatus={txState.status}
                            variant="ghost"
                            size="sm"
                            onClick={() => {
                              setTransferBot(bot.botIdHash);
                              setNewOwner("");
                            }}
                          >
                            <Send className="h-4 w-4" />
                          </TxButton>
                        </>
                      )}
                      {(bot.status === "Deactivated" || bot.status === "Suspended") && (
                        <TxButton
                          txStatus={txState.status}
                          variant="ghost"
                          size="sm"
                          onClick={() => reactivateBot(bot.botIdHash)}
                          loadingText={t("processing")}
                        >
                          <Power className="h-4 w-4" />
                        </TxButton>
                      )}
                      <TxButton
                        txStatus={txState.status}
                        variant="ghost"
                        size="sm"
                        onClick={() => {
                          setBindOpen(true);
                          setBindBotId(bot.botIdHash);
                          setBindCommunityId("");
                          setBindPlatform(PLATFORMS[0]);
                        }}
                        title={t("bindCommunity")}
                      >
                        <Link2 className="h-4 w-4" />
                      </TxButton>
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

      {/* Register Bot Dialog */}
      <Dialog open={registerOpen} onOpenChange={setRegisterOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("registerBot")}</DialogTitle>
            <DialogDescription>
              Provide bot ID hash (32 bytes hex) and public key (32 bytes hex).
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("botIdHash")}</label>
              <Input
                placeholder={t("botIdHashPlaceholder")}
                value={regBotIdHash}
                onChange={(e) => setRegBotIdHash(e.target.value)}
                className="font-mono text-sm"
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("publicKey")}</label>
              <Input
                placeholder={t("publicKeyPlaceholder")}
                value={regPublicKey}
                onChange={(e) => setRegPublicKey(e.target.value)}
                className="font-mono text-sm"
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
              disabled={!regBotIdHash.trim() || !regPublicKey.trim()}
              loadingText={t("processing")}
            >
              {t("registerBot")}
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Update Key Dialog */}
      <Dialog open={!!updateKeyBot} onOpenChange={(o) => !o && setUpdateKeyBot(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("updateKey")}</DialogTitle>
            <DialogDescription>
              Set a new public key for this bot.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("publicKey")}</label>
              <Input
                placeholder={t("publicKeyPlaceholder")}
                value={newPublicKey}
                onChange={(e) => setNewPublicKey(e.target.value)}
                className="font-mono text-sm"
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setUpdateKeyBot(null)}>
              {t("cancel")}
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleUpdateKey}
              disabled={!newPublicKey.trim()}
              loadingText={t("processing")}
            >
              {t("updateKey")}
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Transfer Ownership Dialog */}
      <Dialog open={!!transferBot} onOpenChange={(o) => !o && setTransferBot(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("transferOwnership")}</DialogTitle>
            <DialogDescription>
              Transfer bot ownership to a new address. Only for Active bots.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("newOwner")}</label>
              <Input
                placeholder={t("newOwnerPlaceholder")}
                value={newOwner}
                onChange={(e) => setNewOwner(e.target.value)}
                className="font-mono text-sm"
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setTransferBot(null)}>
              {t("cancel")}
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleTransfer}
              disabled={!newOwner.trim()}
              loadingText={t("processing")}
            >
              {t("transferOwnership")}
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Bind Community Dialog */}
      <Dialog open={bindOpen} onOpenChange={setBindOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("bindCommunity")}</DialogTitle>
            <DialogDescription>
              Bind a bot to a community on a platform.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("botIdHash")}</label>
              <select
                value={bindBotId}
                onChange={(e) => setBindBotId(e.target.value)}
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              >
                <option value="">{t("selectBot")}</option>
                {bots.map((b) => (
                  <option key={b.botIdHash} value={b.botIdHash}>
                    {truncateHash(b.botIdHash)}
                  </option>
                ))}
              </select>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("communityIdHash")}</label>
              <Input
                value={bindCommunityId}
                onChange={(e) => setBindCommunityId(e.target.value)}
                placeholder={t("communityIdHashPlaceholder")}
                className="font-mono text-sm"
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("platform")}</label>
              <select
                value={bindPlatform}
                onChange={(e) => setBindPlatform(e.target.value)}
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              >
                {PLATFORMS.map((p) => (
                  <option key={p} value={p}>
                    {p}
                  </option>
                ))}
              </select>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setBindOpen(false)}>
              {t("cancel")}
            </Button>
            <TxButton
              txStatus={txState.status}
              onClick={handleBind}
              disabled={!bindBotId.trim() || !bindCommunityId.trim()}
              loadingText={t("processing")}
            >
              {t("bindCommunity")}
            </TxButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
