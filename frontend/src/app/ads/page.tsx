"use client";

import { useState, useMemo, useCallback, useEffect } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
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
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Select } from "@/components/ui/select";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { TxButton } from "@/components/shared/TxButton";
import {
  Megaphone,
  Plus,
  Search,
  DollarSign,
  RotateCcw,
  Wallet,
  Pause,
  Play,
  XCircle,
  Flag,
  Loader2,
} from "lucide-react";
import { useTranslations } from "next-intl";
import { useCampaigns, useCampaignActions } from "@/hooks/useAdCampaign";
import { useWalletStore } from "@/stores/wallet";
import { formatBalance, formatNumber } from "@/lib/utils";
import { CAMPAIGN_STATUS, CAMPAIGN_TYPES, AD_REVIEW_STATUS } from "@/lib/constants";
import type { AdCampaign } from "@/lib/types";

const DECIMALS = 12;
const NEX_UNIT = BigInt(10 ** DECIMALS);

function toBigInt(v: unknown): bigint {
  if (typeof v === "bigint") return v;
  if (typeof v === "string") return BigInt(v);
  return BigInt(Number(v) || 0);
}

function parseNex(input: string): bigint {
  const n = parseFloat(input || "0");
  if (isNaN(n) || n < 0) return BigInt(0);
  return BigInt(Math.floor(n * Number(NEX_UNIT)));
}

export default function AdsPage() {
  const t = useTranslations("ads");
  const tCommon = useTranslations("common");
  const address = useWalletStore((s) => s.address);
  const { campaigns, isLoading, refetch } = useCampaigns(address ?? undefined);
  const actions = useCampaignActions();

  const [statusFilter, setStatusFilter] = useState<string>("");
  const [typeFilter, setTypeFilter] = useState<string>("");
  const [search, setSearch] = useState("");
  const [createOpen, setCreateOpen] = useState(false);
  const [fundCampaign, setFundCampaign] = useState<AdCampaign | null>(null);
  const [fundAmount, setFundAmount] = useState("");

  const filtered = useMemo(() => {
    let list = campaigns;
    if (statusFilter) list = list.filter((c) => c.status === statusFilter);
    if (typeFilter) list = list.filter((c) => c.campaignType === typeFilter);
    if (search.trim()) {
      const q = search.toLowerCase();
      list = list.filter(
        (c) =>
          String(c.id).includes(q) ||
          (c.text || "").toLowerCase().includes(q) ||
          (c.url || "").toLowerCase().includes(q)
      );
    }
    return list;
  }, [campaigns, statusFilter, typeFilter, search]);

  const stats = useMemo(() => {
    const total = campaigns.length;
    const active = campaigns.filter((c) => c.status === "Active").length;
    const paused = campaigns.filter((c) => c.status === "Paused").length;
    const totalSpent = campaigns.reduce((acc, c) => acc + toBigInt(c.spent), BigInt(0));
    return { total, active, paused, totalSpent };
  }, [campaigns]);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Megaphone className="h-8 w-8" />
            {t("title")}
          </h1>
          <p className="text-muted-foreground">{t("subtitle")}</p>
        </div>
        <Button onClick={() => setCreateOpen(true)}>
          <Plus className="mr-2 h-4 w-4" />
          {t("createCampaign")}
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">{t("totalCampaigns")}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{isLoading ? "—" : stats.total}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">{t("active")}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-green-600 dark:text-green-400">
              {isLoading ? "—" : stats.active}
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">{t("paused")}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-yellow-600 dark:text-yellow-400">
              {isLoading ? "—" : stats.paused}
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">{t("totalSpent")}</CardTitle>
          </CardHeader>
          <CardContent className="flex items-center gap-2">
            <DollarSign className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">
              {isLoading ? "—" : `${formatBalance(stats.totalSpent)} NEX`}
            </p>
          </CardContent>
        </Card>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <div className="relative flex-1 min-w-[200px]">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            placeholder={t("searchPlaceholder")}
            className="pl-9"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <Select
          value={statusFilter}
          onChange={(e) => setStatusFilter(e.target.value)}
          className="w-[140px]"
        >
          <option value="">{t("filterStatus")}: {t("all")}</option>
          {CAMPAIGN_STATUS.map((s) => (
            <option key={s} value={s}>
              {s}
            </option>
          ))}
        </Select>
        <Select
          value={typeFilter}
          onChange={(e) => setTypeFilter(e.target.value)}
          className="w-[120px]"
        >
          <option value="">{t("filterType")}: {t("all")}</option>
          {CAMPAIGN_TYPES.map((ty) => (
            <option key={ty} value={ty}>
              {ty}
            </option>
          ))}
        </Select>
        <Button variant="outline" size="sm" onClick={refetch} disabled={isLoading}>
          {isLoading ? (
            <Loader2 className="mr-2 h-3 w-3 animate-spin" />
          ) : (
            <RotateCcw className="mr-2 h-3 w-3" />
          )}
          {t("refresh")}
        </Button>
      </div>

      {actions.txState.status === "finalized" && (
        <div className="rounded-lg border border-green-200 bg-green-50 dark:border-green-800 dark:bg-green-900/20 p-3 text-sm text-green-700 dark:text-green-400 flex items-center justify-between">
          {t("success")}
          <Button variant="ghost" size="sm" onClick={() => { actions.resetTx(); refetch(); }}>
            {t("dismiss")}
          </Button>
        </div>
      )}
      {actions.txState.status === "error" && (
        <div className="rounded-lg border border-red-200 bg-red-50 dark:border-red-800 dark:bg-red-900/20 p-3 text-sm text-destructive flex items-center justify-between">
          {actions.txState.error}
          <Button variant="ghost" size="sm" onClick={() => actions.resetTx()}>
            {t("dismiss")}
          </Button>
        </div>
      )}

      {filtered.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Megaphone className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">{t("noCampaigns")}</p>
            <p className="text-sm text-muted-foreground">{t("noCampaignsDesc")}</p>
            <Button className="mt-4" onClick={() => setCreateOpen(true)}>
              <Plus className="mr-2 h-4 w-4" />
              {t("createCampaign")}
            </Button>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("id")}</TableHead>
                  <TableHead>{t("text")}</TableHead>
                  <TableHead>{t("type")}</TableHead>
                  <TableHead>{t("bid")}</TableHead>
                  <TableHead>{t("budget")}</TableHead>
                  <TableHead>{t("spent")}</TableHead>
                  <TableHead>{t("status")}</TableHead>
                  <TableHead>{t("review")}</TableHead>
                  <TableHead>{t("deliveries")}</TableHead>
                  <TableHead>{t("clicks")}</TableHead>
                  <TableHead className="text-right">{t("actions")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filtered.map((c) => (
                  <TableRow key={c.id}>
                    <TableCell className="font-mono text-xs">{c.id}</TableCell>
                    <TableCell className="max-w-[180px] truncate" title={c.text || ""}>
                      {c.text ? (c.text.length > 40 ? `${c.text.slice(0, 40)}…` : c.text) : "—"}
                    </TableCell>
                    <TableCell>
                      <Badge variant="outline">{c.campaignType}</Badge>
                    </TableCell>
                    <TableCell className="font-mono text-xs">
                      {c.campaignType === "Cpc"
                        ? `${formatBalance(toBigInt(c.bidPerClick))}/clk`
                        : `${formatBalance(toBigInt(c.bidPerMille))}/1k`}
                    </TableCell>
                    <TableCell className="font-mono text-xs">
                      {formatBalance(toBigInt(c.totalBudget))}
                    </TableCell>
                    <TableCell className="font-mono text-xs">
                      {formatBalance(toBigInt(c.spent))}
                    </TableCell>
                    <TableCell>
                      <StatusBadge status={c.status} />
                    </TableCell>
                    <TableCell>
                      <StatusBadge status={c.reviewStatus} />
                    </TableCell>
                    <TableCell>{formatNumber(c.totalDeliveries ?? 0)}</TableCell>
                    <TableCell>{formatNumber(c.totalClicks ?? 0)}</TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-1 flex-wrap">
                        {c.status !== "Cancelled" &&
                          c.status !== "Expired" &&
                          c.status !== "Exhausted" && (
                            <Button
                              size="sm"
                              variant="outline"
                              className="h-7 text-xs"
                              onClick={() => setFundCampaign(c)}
                            >
                              <Wallet className="mr-1 h-3 w-3" />
                              {t("fund")}
                            </Button>
                          )}
                        {c.status === "Active" && (
                          <TxButton
                            size="sm"
                            variant="outline"
                            className="h-7 text-xs"
                            txStatus={actions.txState.status}
                            onClick={() => actions.pauseCampaign(c.id)}
                          >
                            <Pause className="mr-1 h-3 w-3" />
                            {t("pause")}
                          </TxButton>
                        )}
                        {c.status === "Paused" && (
                          <TxButton
                            size="sm"
                            variant="outline"
                            className="h-7 text-xs"
                            txStatus={actions.txState.status}
                            onClick={() => actions.resumeCampaign(c.id)}
                          >
                            <Play className="mr-1 h-3 w-3" />
                            {t("resume")}
                          </TxButton>
                        )}
                        {(c.status === "Active" || c.status === "Paused") && (
                          <TxButton
                            size="sm"
                            variant="outline"
                            className="h-7 text-xs text-destructive"
                            txStatus={actions.txState.status}
                            onClick={() => actions.cancelCampaign(c.id)}
                          >
                            <XCircle className="mr-1 h-3 w-3" />
                            {t("cancel")}
                          </TxButton>
                        )}
                        {c.reviewStatus !== "Flagged" && (
                          <TxButton
                            size="sm"
                            variant="ghost"
                            className="h-7 text-xs"
                            txStatus={actions.txState.status}
                            onClick={() => actions.flagCampaign(c.id)}
                          >
                            <Flag className="mr-1 h-3 w-3" />
                            {t("flag")}
                          </TxButton>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        </Card>
      )}

      <CreateCampaignDialog
        open={createOpen}
        onOpenChange={setCreateOpen}
        actions={actions}
        refetch={refetch}
        t={t}
        tCommon={tCommon}
      />

      <FundCampaignDialog
        campaign={fundCampaign}
        onClose={() => {
          setFundCampaign(null);
          setFundAmount("");
        }}
        amount={fundAmount}
        onAmountChange={setFundAmount}
        actions={actions}
        refetch={refetch}
        t={t}
        tCommon={tCommon}
      />
    </div>
  );
}

function CreateCampaignDialog({
  open,
  onOpenChange,
  actions,
  refetch,
  t,
  tCommon,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  actions: ReturnType<typeof useCampaignActions>;
  refetch: () => void;
  t: (k: string) => string;
  tCommon: (k: string) => string;
}) {
  const [text, setText] = useState("");
  const [url, setUrl] = useState("");
  const [bidPerMille, setBidPerMille] = useState("");
  const [bidPerClick, setBidPerClick] = useState("");
  const [campaignType, setCampaignType] = useState("Cpm");
  const [dailyBudget, setDailyBudget] = useState("");
  const [totalBudget, setTotalBudget] = useState("");
  const [deliveryTypes, setDeliveryTypes] = useState("1");
  const [expiresAt, setExpiresAt] = useState("0");

  useEffect(() => {
    if (open && actions.txState.status === "finalized") {
      actions.resetTx();
      onOpenChange(false);
      refetch();
    }
  }, [open, actions.txState.status, actions.resetTx, onOpenChange, refetch]);

  const handleSubmit = () => {
    const bpm = parseNex(bidPerMille);
    const bpc = parseNex(bidPerClick);
    const daily = parseNex(dailyBudget);
    const total = parseNex(totalBudget);
    const dt = Math.max(1, Math.min(7, parseInt(deliveryTypes, 10) || 1));
    const exp = parseInt(expiresAt, 10) || 0;

    if (!text.trim() || !url.trim() || total === BigInt(0)) return;

    actions.createCampaign(
      text.trim(),
      url.trim(),
      bpm,
      daily,
      total,
      dt,
      exp,
      null,
      campaignType,
      bpc
    );
  };

  const canSubmit =
    text.trim() &&
    url.trim() &&
    totalBudget &&
    parseFloat(totalBudget) > 0 &&
    (parseFloat(bidPerMille) > 0 || parseFloat(bidPerClick) > 0);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{t("createDialogTitle")}</DialogTitle>
          <DialogDescription>{t("createDialogDesc")}</DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 py-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("textLabel")}</label>
            <Textarea
              placeholder={t("textPlaceholder")}
              value={text}
              onChange={(e) => setText(e.target.value)}
              rows={3}
              className="resize-none"
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("urlLabel")}</label>
            <Input
              placeholder={t("urlPlaceholder")}
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              type="url"
            />
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("campaignTypeLabel")}</label>
              <Select
                value={campaignType}
                onChange={(e) => setCampaignType(e.target.value)}
              >
                {CAMPAIGN_TYPES.map((ty) => (
                  <option key={ty} value={ty}>
                    {ty}
                  </option>
                ))}
              </Select>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("deliveryTypesLabel")}</label>
              <Input
                type="number"
                min={1}
                max={7}
                value={deliveryTypes}
                onChange={(e) => setDeliveryTypes(e.target.value)}
              />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("bidPerMilleLabel")}</label>
              <Input
                type="number"
                min="0"
                step="0.0001"
                placeholder="0"
                value={bidPerMille}
                onChange={(e) => setBidPerMille(e.target.value)}
                disabled={campaignType === "Cpc"}
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("bidPerClickLabel")}</label>
              <Input
                type="number"
                min="0"
                step="0.0001"
                placeholder="0"
                value={bidPerClick}
                onChange={(e) => setBidPerClick(e.target.value)}
                disabled={campaignType === "Cpm"}
              />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("dailyBudgetLabel")}</label>
              <Input
                type="number"
                min="0"
                step="0.01"
                placeholder="0"
                value={dailyBudget}
                onChange={(e) => setDailyBudget(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("totalBudgetLabel")}</label>
              <Input
                type="number"
                min="0"
                step="0.01"
                placeholder="0"
                value={totalBudget}
                onChange={(e) => setTotalBudget(e.target.value)}
              />
            </div>
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("expiresAtLabel")}</label>
            <Input
              type="number"
              min="0"
              placeholder={t("expiresAtPlaceholder")}
              value={expiresAt}
              onChange={(e) => setExpiresAt(e.target.value)}
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {tCommon("cancel")}
          </Button>
          <TxButton
            txStatus={actions.txState.status}
            loadingText={t("processing")}
            onClick={handleSubmit}
            disabled={!canSubmit}
          >
            {tCommon("create")}
          </TxButton>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function FundCampaignDialog({
  campaign,
  onClose,
  amount,
  onAmountChange,
  actions,
  refetch,
  t,
  tCommon,
}: {
  campaign: AdCampaign | null;
  onClose: () => void;
  amount: string;
  onAmountChange: (v: string) => void;
  actions: ReturnType<typeof useCampaignActions>;
  refetch: () => void;
  t: (k: string) => string;
  tCommon: (k: string) => string;
}) {
  const open = !!campaign;

  useEffect(() => {
    if (open && actions.txState.status === "finalized") {
      actions.resetTx();
      onClose();
      refetch();
    }
  }, [open, actions.txState.status, actions.resetTx, onClose, refetch]);

  const handleFund = () => {
    if (!campaign) return;
    const amt = parseNex(amount);
    if (amt <= BigInt(0)) return;
    actions.fundCampaign(campaign.id, amt);
  };

  return (
    <Dialog open={open} onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("fundDialogTitle")}</DialogTitle>
          <DialogDescription>
            {t("fundDialogDesc")} {campaign && `Campaign #${campaign.id}`}
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-4 py-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("fundAmountLabel")}</label>
            <Input
              type="number"
              min="0"
              step="0.01"
              placeholder="0"
              value={amount}
              onChange={(e) => onAmountChange(e.target.value)}
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {tCommon("cancel")}
          </Button>
          <TxButton
            txStatus={actions.txState.status}
            loadingText={t("processing")}
            onClick={handleFund}
            disabled={!amount || parseFloat(amount) <= 0}
          >
            {t("fund")}
          </TxButton>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
