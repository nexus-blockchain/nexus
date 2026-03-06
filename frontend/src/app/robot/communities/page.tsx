"use client";

import { useState, useEffect } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
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
  Users,
  ArrowLeft,
  Plus,
  RotateCcw,
  Settings,
  Unlink,
  Ban,
  ShieldCheck,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";
import { shortenAddress, formatNumber } from "@/lib/utils";
import { useWalletStore } from "@/stores/wallet";
import {
  useCommunityBindings,
  useCommunityConfigs,
  useCommunityActions,
} from "@/hooks/useCommunity";
import { useBotActions } from "@/hooks/useBot";
import type { CommunityBinding, CommunityConfig } from "@/lib/types";
import { PLATFORMS, WARN_ACTIONS } from "@/lib/constants";

type Tab = "bindings" | "configs";

function truncateHash(hash: string, start = 10, end = 8): string {
  if (!hash) return "";
  const s = hash.startsWith("0x") ? hash.slice(2) : hash;
  return s.length > start + end ? `${s.slice(0, start)}…${s.slice(-end)}` : hash;
}

export default function RobotCommunitiesPage() {
  const t = useTranslations("robot");
  const address = useWalletStore((s) => s.address);
  const [tab, setTab] = useState<Tab>("bindings");
  const [bindDialogOpen, setBindDialogOpen] = useState(false);
  const [configDialogOpen, setConfigDialogOpen] = useState(false);
  const [selectedConfig, setSelectedConfig] = useState<(CommunityConfig & { communityIdHash: string }) | null>(null);

  const { bindings, isLoading: bindingsLoading, refetch: refetchBindings } = useCommunityBindings();
  const { configs, isLoading: configsLoading, refetch: refetchConfigs } = useCommunityConfigs();
  const communityActions = useCommunityActions();
  const botActions = useBotActions();

  const txStatus = communityActions.txState.status || botActions.txState.status;

  const handleRefetch = () => {
    refetchBindings();
    refetchConfigs();
  };

  useEffect(() => {
    if (txStatus === "finalized") {
      handleRefetch();
      setBindDialogOpen(false);
      setConfigDialogOpen(false);
      setSelectedConfig(null);
      communityActions.resetTx();
      botActions.resetTx();
    }
  }, [txStatus]);

  const openUpdateConfig = (cfg: CommunityConfig & { communityIdHash: string }) => {
    setSelectedConfig(cfg);
    setConfigDialogOpen(true);
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
            <Users className="h-7 w-7" />
            {t("communities")}
          </h1>
          <p className="text-muted-foreground">
            Manage bot-bound communities and their configurations
          </p>
        </div>
        <Button onClick={() => setBindDialogOpen(true)} disabled={!address}>
          <Plus className="mr-2 h-4 w-4" />
          {t("bindCommunity")}
        </Button>
      </div>

      <div className="flex gap-2 border-b">
        <button
          onClick={() => setTab("bindings")}
          className={`px-4 py-2 text-sm font-medium transition-colors ${
            tab === "bindings"
              ? "border-b-2 border-primary text-primary"
              : "text-muted-foreground hover:text-foreground"
          }`}
        >
          Bindings
        </button>
        <button
          onClick={() => setTab("configs")}
          className={`px-4 py-2 text-sm font-medium transition-colors ${
            tab === "configs"
              ? "border-b-2 border-primary text-primary"
              : "text-muted-foreground hover:text-foreground"
          }`}
        >
          Configurations
        </button>
      </div>

      {tab === "bindings" && (
        <Card>
          <CardHeader className="flex flex-row items-center justify-between">
            <CardTitle>Bindings</CardTitle>
            <Button variant="outline" size="sm" onClick={handleRefetch} disabled={bindingsLoading}>
              <RotateCcw className="mr-2 h-3 w-3" />
              {t("refresh")}
            </Button>
          </CardHeader>
          <CardContent>
            {bindingsLoading ? (
              <p className="py-8 text-center text-muted-foreground">{t("processing")}</p>
            ) : bindings.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-12">
                <Users className="h-12 w-12 text-muted-foreground/50" />
                <p className="mt-4 text-lg font-medium">No bindings</p>
                <p className="text-sm text-muted-foreground">
                  Bind a community to a bot to start managing it on-chain
                </p>
              </div>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Community Hash</TableHead>
                    <TableHead>Platform</TableHead>
                    <TableHead>Bot Hash</TableHead>
                    <TableHead>Bound By</TableHead>
                    <TableHead>Bound At</TableHead>
                    <TableHead className="text-right">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {bindings.map((b: CommunityBinding) => (
                    <TableRow key={`${b.communityIdHash}-${b.platform}-${b.botIdHash}`}>
                      <TableCell className="font-mono text-xs" title={b.communityIdHash}>
                        {truncateHash(b.communityIdHash)}
                      </TableCell>
                      <TableCell>{b.platform}</TableCell>
                      <TableCell className="font-mono text-xs" title={b.botIdHash}>
                        {truncateHash(b.botIdHash)}
                      </TableCell>
                      <TableCell className="font-mono text-xs" title={b.boundBy}>
                        {shortenAddress(b.boundBy)}
                      </TableCell>
                      <TableCell>#{formatNumber(b.boundAt)}</TableCell>
                      <TableCell className="text-right">
                        <TxButton
                          variant="outline"
                          size="sm"
                          txStatus={txStatus}
                          onClick={() => botActions.unbindCommunity(b.communityIdHash)}
                          disabled={!address}
                        >
                          <Unlink className="mr-1 h-3 w-3" />
                          Unbind
                        </TxButton>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </CardContent>
        </Card>
      )}

      {tab === "configs" && (
        <Card>
          <CardHeader className="flex flex-row items-center justify-between">
            <CardTitle>Configurations</CardTitle>
            <Button variant="outline" size="sm" onClick={handleRefetch} disabled={configsLoading}>
              <RotateCcw className="mr-2 h-3 w-3" />
              {t("refresh")}
            </Button>
          </CardHeader>
          <CardContent>
            {configsLoading ? (
              <p className="py-8 text-center text-muted-foreground">{t("processing")}</p>
            ) : configs.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-12">
                <Settings className="h-12 w-12 text-muted-foreground/50" />
                <p className="mt-4 text-lg font-medium">No configurations</p>
                <p className="text-sm text-muted-foreground">
                  Community configs appear after bindings are created
                </p>
              </div>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Community Hash</TableHead>
                    <TableHead>Node Req</TableHead>
                    <TableHead>Anti-flood</TableHead>
                    <TableHead>Flood</TableHead>
                    <TableHead>Warn</TableHead>
                    <TableHead>Warn Action</TableHead>
                    <TableHead>Ads</TableHead>
                    <TableHead>Members</TableHead>
                    <TableHead>Language</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead className="text-right">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {configs.map((c) => (
                    <TableRow key={c.communityIdHash}>
                      <TableCell className="font-mono text-xs" title={c.communityIdHash}>
                        {truncateHash(c.communityIdHash)}
                      </TableCell>
                      <TableCell>{c.nodeRequirement}</TableCell>
                      <TableCell>{c.antiFloodEnabled ? "Yes" : "No"}</TableCell>
                      <TableCell>{c.floodLimit}</TableCell>
                      <TableCell>{c.warnLimit}</TableCell>
                      <TableCell>{c.warnAction}</TableCell>
                      <TableCell>{c.adsEnabled ? "Yes" : "No"}</TableCell>
                      <TableCell>{c.activeMembers}</TableCell>
                      <TableCell>{c.language || "—"}</TableCell>
                      <TableCell>
                        <StatusBadge status={c.status} />
                      </TableCell>
                      <TableCell className="text-right space-x-2">
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => openUpdateConfig(c)}
                          disabled={!address}
                        >
                          <Settings className="mr-1 h-3 w-3" />
                          Update
                        </Button>
                        {c.status === "Active" ? (
                          <TxButton
                            variant="outline"
                            size="sm"
                            txStatus={txStatus}
                            onClick={() => communityActions.banCommunity(c.communityIdHash)}
                            disabled={!address}
                          >
                            <Ban className="mr-1 h-3 w-3" />
                            Ban
                          </TxButton>
                        ) : (
                          <TxButton
                            variant="outline"
                            size="sm"
                            txStatus={txStatus}
                            onClick={() => communityActions.unbanCommunity(c.communityIdHash)}
                            disabled={!address}
                          >
                            <ShieldCheck className="mr-1 h-3 w-3" />
                            Unban
                          </TxButton>
                        )}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </CardContent>
        </Card>
      )}

      {/* Bind Community Dialog */}
      <BindCommunityDialog
        open={bindDialogOpen}
        onOpenChange={setBindDialogOpen}
        onSubmit={(botIdHash, communityIdHash, platform) => {
          botActions.bindCommunity(botIdHash, communityIdHash, platform);
        }}
        txStatus={txStatus}
        disabled={!address}
      />

      {/* Update Config Dialog */}
      {selectedConfig && (
        <UpdateConfigDialog
          open={configDialogOpen}
          onOpenChange={(open) => {
            setConfigDialogOpen(open);
            if (!open) setSelectedConfig(null);
          }}
          config={selectedConfig}
          onSubmit={(cfg) => {
            communityActions.updateCommunityConfig(
              cfg.communityIdHash,
              cfg.version,
              cfg.antiFloodEnabled,
              cfg.floodLimit,
              cfg.warnLimit,
              cfg.warnAction,
              cfg.welcomeEnabled,
              cfg.adsEnabled,
              cfg.language
            );
          }}
          txStatus={txStatus}
          disabled={!address}
        />
      )}
    </div>
  );
}

function BindCommunityDialog({
  open,
  onOpenChange,
  onSubmit,
  txStatus,
  disabled,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSubmit: (botIdHash: string, communityIdHash: string, platform: string) => void;
  txStatus: string;
  disabled: boolean;
}) {
  const t = useTranslations("robot");
  const [botIdHash, setBotIdHash] = useState("");
  const [communityIdHash, setCommunityIdHash] = useState("");
  const [platform, setPlatform] = useState<string>(PLATFORMS[0]);

  const handleSubmit = () => {
    if (!botIdHash.trim() || !communityIdHash.trim()) return;
    onSubmit(botIdHash.trim(), communityIdHash.trim(), platform);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("bindCommunity")}</DialogTitle>
          <DialogDescription>
            Bind a community to a bot. Both hashes must be 32 bytes hex (0x + 64 chars).
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 py-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">Bot ID Hash</label>
            <Input
              placeholder={t("botIdHashPlaceholder")}
              value={botIdHash}
              onChange={(e) => setBotIdHash(e.target.value)}
              className="font-mono"
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("communityIdHash")}</label>
            <Input
              placeholder={t("communityIdHashPlaceholder")}
              value={communityIdHash}
              onChange={(e) => setCommunityIdHash(e.target.value)}
              className="font-mono"
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("platform")}</label>
            <select
              value={platform}
              onChange={(e) => setPlatform(e.target.value)}
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
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <TxButton
            txStatus={txStatus}
            onClick={handleSubmit}
            disabled={disabled || !botIdHash.trim() || !communityIdHash.trim()}
          >
            {t("bindCommunity")}
          </TxButton>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function UpdateConfigDialog({
  open,
  onOpenChange,
  config,
  onSubmit,
  txStatus,
  disabled,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  config: CommunityConfig & { communityIdHash: string };
  onSubmit: (cfg: CommunityConfig & { communityIdHash: string }) => void;
  txStatus: string;
  disabled: boolean;
}) {
  const [antiFloodEnabled, setAntiFloodEnabled] = useState(config.antiFloodEnabled);
  const [floodLimit, setFloodLimit] = useState(config.floodLimit);
  const [warnLimit, setWarnLimit] = useState(config.warnLimit);
  const [warnAction, setWarnAction] = useState(config.warnAction);
  const [welcomeEnabled, setWelcomeEnabled] = useState(config.welcomeEnabled);
  const [adsEnabled, setAdsEnabled] = useState(config.adsEnabled);
  const [activeMembers, setActiveMembers] = useState(config.activeMembers);
  const [language, setLanguage] = useState(config.language || "");

  useEffect(() => {
    setAntiFloodEnabled(config.antiFloodEnabled);
    setFloodLimit(config.floodLimit);
    setWarnLimit(config.warnLimit);
    setWarnAction(config.warnAction);
    setWelcomeEnabled(config.welcomeEnabled);
    setAdsEnabled(config.adsEnabled);
    setActiveMembers(config.activeMembers);
    setLanguage(config.language || "");
  }, [config]);

  const handleSubmit = () => {
    onSubmit({
      ...config,
      antiFloodEnabled,
      floodLimit,
      warnLimit,
      warnAction,
      welcomeEnabled,
      adsEnabled,
      activeMembers,
      language,
    });
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Update Community Config</DialogTitle>
          <DialogDescription>
            Update configuration for community {truncateHash(config.communityIdHash)}
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 py-4">
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="antiFlood"
              checked={antiFloodEnabled}
              onChange={(e) => setAntiFloodEnabled(e.target.checked)}
              className="rounded border"
            />
            <label htmlFor="antiFlood" className="text-sm font-medium">
              Anti-flood enabled
            </label>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Flood Limit</label>
              <Input
                type="number"
                value={floodLimit}
                onChange={(e) => setFloodLimit(Number(e.target.value) || 0)}
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Warn Limit</label>
              <Input
                type="number"
                value={warnLimit}
                onChange={(e) => setWarnLimit(Number(e.target.value) || 0)}
              />
            </div>
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Warn Action</label>
            <select
              value={warnAction}
              onChange={(e) => setWarnAction(e.target.value)}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              {WARN_ACTIONS.map((a) => (
                <option key={a} value={a}>
                  {a}
                </option>
              ))}
            </select>
          </div>
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="welcome"
              checked={welcomeEnabled}
              onChange={(e) => setWelcomeEnabled(e.target.checked)}
              className="rounded border"
            />
            <label htmlFor="welcome" className="text-sm font-medium">
              Welcome enabled
            </label>
          </div>
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="ads"
              checked={adsEnabled}
              onChange={(e) => setAdsEnabled(e.target.checked)}
              className="rounded border"
            />
            <label htmlFor="ads" className="text-sm font-medium">
              Ads enabled
            </label>
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Active Members</label>
            <Input
              type="number"
              value={activeMembers}
              onChange={(e) => setActiveMembers(Number(e.target.value) || 0)}
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Language</label>
            <Input
              value={language}
              onChange={(e) => setLanguage(e.target.value)}
              placeholder="en"
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <TxButton txStatus={txStatus} onClick={handleSubmit} disabled={disabled}>
            Update Config
          </TxButton>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
