"use client";

import { useState, useMemo } from "react";
import { useEntityStore } from "@/stores/entity";
import {
  useDisclosures,
  useAnnouncements,
  useDisclosureConfig,
  useBlackout,
  useDisclosureActions,
} from "@/hooks/useDisclosure";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import { Separator } from "@/components/ui/separator";
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  FileText,
  Megaphone,
  Settings,
  Plus,
  Pin,
  PinOff,
  Trash2,
  Send,
  Eye,
  EyeOff,
  RotateCcw,
  AlertTriangle,
  ShieldAlert,
  Clock,
  Ban,
  CheckCircle,
  PenLine,
  Timer,
} from "lucide-react";
import { useTranslations } from "next-intl";
import {
  DISCLOSURE_TYPES,
  DISCLOSURE_STATUS,
  DISCLOSURE_LEVELS,
  ANNOUNCEMENT_CATEGORIES,
  VIOLATION_TYPES,
} from "@/lib/constants";
import type { DisclosureData, AnnouncementData } from "@/lib/types";

const DISCLOSURE_TYPE_ICONS: Record<string, string> = {
  AnnualReport: "📊",
  QuarterlyReport: "📈",
  MonthlyReport: "📋",
  MaterialEvent: "⚡",
  RelatedPartyTransaction: "🤝",
  OwnershipChange: "🔄",
  ManagementChange: "👥",
  BusinessChange: "🏢",
  RiskWarning: "⚠️",
  DividendAnnouncement: "💰",
  TokenIssuance: "🪙",
  Buyback: "🔙",
  Other: "📄",
};

const CATEGORY_COLORS: Record<string, string> = {
  General: "bg-slate-100 text-slate-700 dark:bg-slate-800 dark:text-slate-300",
  Promotion:
    "bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400",
  SystemUpdate:
    "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400",
  Event:
    "bg-pink-100 text-pink-700 dark:bg-pink-900/30 dark:text-pink-400",
  Policy:
    "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400",
  Partnership:
    "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400",
  Product:
    "bg-cyan-100 text-cyan-700 dark:bg-cyan-900/30 dark:text-cyan-400",
  Other: "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-400",
};

export default function DisclosurePage() {
  const { currentEntityId } = useEntityStore();
  const { disclosures, isLoading: discLoading, refetch: refetchDisc } = useDisclosures(currentEntityId);
  const { announcements, isLoading: annLoading, refetch: refetchAnn } = useAnnouncements(currentEntityId);
  const { config, isLoading: configLoading, refetch: refetchConfig } = useDisclosureConfig(currentEntityId);
  const { blackout, isLoading: blackoutLoading, refetch: refetchBlackout } = useBlackout(currentEntityId);
  const actions = useDisclosureActions();
  const t = useTranslations("disclosure");
  const tc = useTranslations("common");

  const [statusFilter, setStatusFilter] = useState<string>("All");
  const [discType, setDiscType] = useState<string>("");
  const [contentCid, setContentCid] = useState("");
  const [summaryCid, setSummaryCid] = useState("");

  const [annCategory, setAnnCategory] = useState<string>("");
  const [annTitle, setAnnTitle] = useState("");
  const [annCid, setAnnCid] = useState("");
  const [annExpiresAt, setAnnExpiresAt] = useState("");

  const [cfgLevel, setCfgLevel] = useState<string>("");
  const [cfgInsiderControl, setCfgInsiderControl] = useState(false);
  const [cfgBlackoutPeriod, setCfgBlackoutPeriod] = useState("");
  const [blackoutDuration, setBlackoutDuration] = useState("");
  const [violationType, setViolationType] = useState<string>("");

  const [correctId, setCorrectId] = useState<number | null>(null);
  const [correctCid, setCorrectCid] = useState("");
  const [correctSummary, setCorrectSummary] = useState("");

  const filteredDisclosures = useMemo(() => {
    if (statusFilter === "All") return disclosures;
    return disclosures.filter((d: DisclosureData) => d.status === statusFilter);
  }, [disclosures, statusFilter]);

  const stats = useMemo(() => ({
    total: disclosures.length,
    drafts: disclosures.filter((d: DisclosureData) => d.status === "Draft").length,
    published: disclosures.filter((d: DisclosureData) => d.status === "Published").length,
    violations: config?.violationCount ?? 0,
  }), [disclosures, config]);

  if (!currentEntityId) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        {tc("selectEntity")}
      </div>
    );
  }

  const handlePublishDisclosure = () => {
    if (!discType || !contentCid || !currentEntityId) return;
    actions.publishDisclosure(currentEntityId, discType, contentCid, summaryCid || null);
    setDiscType("");
    setContentCid("");
    setSummaryCid("");
  };

  const handleSaveDraft = () => {
    if (!discType || !contentCid || !currentEntityId) return;
    actions.createDraftDisclosure(currentEntityId, discType, contentCid, summaryCid || null);
    setDiscType("");
    setContentCid("");
    setSummaryCid("");
  };

  const handleCorrect = () => {
    if (correctId === null || !correctCid) return;
    actions.correctDisclosure(correctId, correctCid, correctSummary || null);
    setCorrectId(null);
    setCorrectCid("");
    setCorrectSummary("");
  };

  const handlePublishAnnouncement = () => {
    if (!annCategory || !annTitle || !annCid || !currentEntityId) return;
    actions.publishAnnouncement(
      currentEntityId,
      annCategory,
      annTitle,
      annCid,
      annExpiresAt ? Number(annExpiresAt) : null
    );
    setAnnCategory("");
    setAnnTitle("");
    setAnnCid("");
    setAnnExpiresAt("");
  };

  const handleConfigure = () => {
    if (!cfgLevel || !currentEntityId) return;
    actions.configureDisclosure(
      currentEntityId,
      cfgLevel,
      cfgInsiderControl,
      Number(cfgBlackoutPeriod) || 0
    );
  };

  const handleStartBlackout = () => {
    if (!blackoutDuration || !currentEntityId) return;
    actions.startBlackout(currentEntityId, Number(blackoutDuration));
    setBlackoutDuration("");
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">
            Disclosure & Announcements
          </h1>
          <p className="text-muted-foreground">
            Financial disclosures, compliance reports, and public announcements
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => {
            refetchDisc();
            refetchAnn();
            refetchConfig();
            refetchBlackout();
          }}
        >
          <RotateCcw className="mr-2 h-3 w-3" />
          Refresh
        </Button>
      </div>

      <Tabs defaultValue="disclosures">
        <TabsList>
          <TabsTrigger value="disclosures" className="gap-2">
            <FileText className="h-4 w-4" />
            Disclosures
          </TabsTrigger>
          <TabsTrigger value="announcements" className="gap-2">
            <Megaphone className="h-4 w-4" />
            Announcements
          </TabsTrigger>
          <TabsTrigger value="config" className="gap-2">
            <Settings className="h-4 w-4" />
            Config
          </TabsTrigger>
        </TabsList>

        {/* ================================================================ */}
        {/* DISCLOSURES TAB                                                   */}
        {/* ================================================================ */}
        <TabsContent value="disclosures" className="space-y-6">
          <div className="grid gap-4 md:grid-cols-4">
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium">Total</CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold">{stats.total}</p>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium">Drafts</CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold text-muted-foreground">
                  {stats.drafts}
                </p>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium">Published</CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold text-green-600">
                  {stats.published}
                </p>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium">Violations</CardTitle>
              </CardHeader>
              <CardContent>
                <p className={`text-2xl font-bold ${stats.violations > 0 ? "text-red-600" : ""}`}>
                  {stats.violations}
                </p>
              </CardContent>
            </Card>
          </div>

          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-sm font-medium mr-1">Filter:</span>
            {["All", ...DISCLOSURE_STATUS].map((s) => (
              <Button
                key={s}
                size="sm"
                variant={statusFilter === s ? "default" : "outline"}
                onClick={() => setStatusFilter(s)}
              >
                {s}
              </Button>
            ))}
          </div>

          {discLoading ? (
            <div className="flex justify-center py-12">
              <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
            </div>
          ) : filteredDisclosures.length === 0 ? (
            <Card>
              <CardContent className="flex flex-col items-center justify-center py-12">
                <FileText className="h-12 w-12 text-muted-foreground/50" />
                <p className="mt-4 text-lg font-medium">No Disclosures</p>
                <p className="text-sm text-muted-foreground">
                  {statusFilter !== "All"
                    ? `No ${statusFilter.toLowerCase()} disclosures found.`
                    : "Create your first disclosure below."}
                </p>
              </CardContent>
            </Card>
          ) : (
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <FileText className="h-5 w-5" />
                  Disclosure List
                </CardTitle>
                <CardDescription>
                  {filteredDisclosures.length} disclosure
                  {filteredDisclosures.length !== 1 ? "s" : ""}
                  {statusFilter !== "All" ? ` (${statusFilter})` : ""}
                </CardDescription>
              </CardHeader>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Type</TableHead>
                    <TableHead>Content CID</TableHead>
                    <TableHead>Discloser</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Date</TableHead>
                    <TableHead className="text-right">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {filteredDisclosures.map((disc: DisclosureData) => (
                    <TableRow key={disc.id}>
                      <TableCell>
                        <Badge variant="outline" className="gap-1">
                          <span>{DISCLOSURE_TYPE_ICONS[disc.disclosureType] || "📄"}</span>
                          {disc.disclosureType}
                        </Badge>
                      </TableCell>
                      <TableCell>
                        <span className="font-mono text-xs" title={disc.contentCid}>
                          {disc.contentCid
                            ? `${disc.contentCid.slice(0, 8)}...${disc.contentCid.slice(-6)}`
                            : "—"}
                        </span>
                      </TableCell>
                      <TableCell>
                        <AddressDisplay address={disc.discloser} />
                      </TableCell>
                      <TableCell>
                        <StatusBadge status={disc.status} />
                      </TableCell>
                      <TableCell className="text-sm text-muted-foreground">
                        <span className="flex items-center gap-1">
                          <Clock className="h-3 w-3" />
                          Block #{disc.disclosedAt}
                        </span>
                      </TableCell>
                      <TableCell className="text-right">
                        <div className="flex items-center justify-end gap-1">
                          {disc.status === "Published" && (
                            <>
                              <Button
                                size="sm"
                                variant="ghost"
                                onClick={() => actions.withdrawDisclosure(disc.id)}
                                title="Withdraw"
                              >
                                <EyeOff className="h-4 w-4" />
                              </Button>
                              <Button
                                size="sm"
                                variant="ghost"
                                onClick={() => {
                                  setCorrectId(disc.id);
                                  setCorrectCid("");
                                  setCorrectSummary("");
                                }}
                                title="Correct"
                              >
                                <PenLine className="h-4 w-4" />
                              </Button>
                            </>
                          )}
                          {disc.status === "Draft" && (
                            <>
                              <Button
                                size="sm"
                                variant="ghost"
                                onClick={() => actions.publishDraft(disc.id)}
                                title="Publish Draft"
                              >
                                <Send className="h-4 w-4 text-green-600" />
                              </Button>
                              <Button
                                size="sm"
                                variant="ghost"
                                onClick={() => actions.deleteDraft(disc.id)}
                                title="Delete Draft"
                              >
                                <Trash2 className="h-4 w-4 text-destructive" />
                              </Button>
                            </>
                          )}
                        </div>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </Card>
          )}

          {correctId !== null && (
            <Card className="border-amber-300 dark:border-amber-700">
              <CardHeader>
                <CardTitle className="flex items-center gap-2 text-amber-600">
                  <PenLine className="h-5 w-5" />
                  Correct Disclosure #{correctId}
                </CardTitle>
                <CardDescription>
                  Submit a corrected version of this disclosure
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="grid gap-4 md:grid-cols-2">
                  <div className="space-y-2">
                    <label className="text-sm font-medium">New Content CID</label>
                    <Input
                      value={correctCid}
                      onChange={(e) => setCorrectCid(e.target.value)}
                      placeholder="Corrected content IPFS CID"
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">
                      New Summary CID (optional)
                    </label>
                    <Input
                      value={correctSummary}
                      onChange={(e) => setCorrectSummary(e.target.value)}
                      placeholder="Updated summary CID"
                    />
                  </div>
                </div>
                <div className="flex gap-2">
                  <TxButton
                    onClick={handleCorrect}
                    txStatus={actions.txState.status}
                    disabled={!correctCid}
                  >
                    <CheckCircle className="mr-2 h-4 w-4" />
                    Submit Correction
                  </TxButton>
                  <Button
                    variant="ghost"
                    onClick={() => setCorrectId(null)}
                  >
                    Cancel
                  </Button>
                </div>
              </CardContent>
            </Card>
          )}

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Plus className="h-5 w-5" />
                Create Disclosure
              </CardTitle>
              <CardDescription>
                Select a disclosure type and provide the content CID
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Disclosure Type</label>
                <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5">
                  {DISCLOSURE_TYPES.map((dtype) => (
                    <button
                      key={dtype}
                      onClick={() => setDiscType(dtype)}
                      className={`flex items-center gap-2 rounded-lg border p-3 text-left text-sm transition-colors hover:bg-accent ${
                        discType === dtype
                          ? "border-primary bg-primary/5 ring-1 ring-primary"
                          : "border-border"
                      }`}
                    >
                      <span className="text-base">
                        {DISCLOSURE_TYPE_ICONS[dtype] || "📄"}
                      </span>
                      <span className="font-medium leading-tight">
                        {dtype.replace(/([A-Z])/g, " $1").trim()}
                      </span>
                    </button>
                  ))}
                </div>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Content CID</label>
                  <Input
                    value={contentCid}
                    onChange={(e) => setContentCid(e.target.value)}
                    placeholder="IPFS content identifier"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">
                    Summary CID (optional)
                  </label>
                  <Input
                    value={summaryCid}
                    onChange={(e) => setSummaryCid(e.target.value)}
                    placeholder="IPFS summary identifier"
                  />
                </div>
              </div>

              <div className="flex gap-2">
                <TxButton
                  onClick={handlePublishDisclosure}
                  txStatus={actions.txState.status}
                  disabled={!discType || !contentCid}
                >
                  <Send className="mr-2 h-4 w-4" />
                  Publish Now
                </TxButton>
                <TxButton
                  variant="outline"
                  onClick={handleSaveDraft}
                  txStatus={actions.txState.status}
                  disabled={!discType || !contentCid}
                >
                  <FileText className="mr-2 h-4 w-4" />
                  Save as Draft
                </TxButton>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        {/* ================================================================ */}
        {/* ANNOUNCEMENTS TAB                                                 */}
        {/* ================================================================ */}
        <TabsContent value="announcements" className="space-y-6">
          {annLoading ? (
            <div className="flex justify-center py-12">
              <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
            </div>
          ) : announcements.length === 0 ? (
            <Card>
              <CardContent className="flex flex-col items-center justify-center py-12">
                <Megaphone className="h-12 w-12 text-muted-foreground/50" />
                <p className="mt-4 text-lg font-medium">No Announcements</p>
                <p className="text-sm text-muted-foreground">
                  Publish your first announcement below.
                </p>
              </CardContent>
            </Card>
          ) : (
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Megaphone className="h-5 w-5" />
                  Announcements
                </CardTitle>
                <CardDescription>
                  {announcements.length} announcement
                  {announcements.length !== 1 ? "s" : ""}
                </CardDescription>
              </CardHeader>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Category</TableHead>
                    <TableHead>Title</TableHead>
                    <TableHead>Publisher</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Pinned</TableHead>
                    <TableHead className="text-right">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {announcements.map((ann: AnnouncementData) => (
                    <TableRow key={ann.id}>
                      <TableCell>
                        <span
                          className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-semibold ${
                            CATEGORY_COLORS[ann.category] || CATEGORY_COLORS.Other
                          }`}
                        >
                          {ann.category}
                        </span>
                      </TableCell>
                      <TableCell className="font-medium max-w-[200px] truncate">
                        {ann.title}
                      </TableCell>
                      <TableCell>
                        <AddressDisplay address={ann.publisher} />
                      </TableCell>
                      <TableCell>
                        <StatusBadge status={ann.status} />
                      </TableCell>
                      <TableCell>
                        {ann.isPinned && (
                          <Pin className="h-4 w-4 text-primary" />
                        )}
                      </TableCell>
                      <TableCell className="text-right">
                        <div className="flex items-center justify-end gap-1">
                          {ann.isPinned ? (
                            <Button
                              size="sm"
                              variant="ghost"
                              onClick={() =>
                                actions.unpinAnnouncement(
                                  currentEntityId,
                                  ann.id
                                )
                              }
                              title="Unpin"
                            >
                              <PinOff className="h-4 w-4" />
                            </Button>
                          ) : (
                            <Button
                              size="sm"
                              variant="ghost"
                              onClick={() =>
                                actions.pinAnnouncement(
                                  currentEntityId,
                                  ann.id
                                )
                              }
                              title="Pin"
                            >
                              <Pin className="h-4 w-4" />
                            </Button>
                          )}
                          {ann.status === "Active" && (
                            <Button
                              size="sm"
                              variant="ghost"
                              onClick={() =>
                                actions.withdrawAnnouncement(ann.id)
                              }
                              title="Withdraw"
                            >
                              <EyeOff className="h-4 w-4" />
                            </Button>
                          )}
                          {ann.status === "Active" && (
                            <Button
                              size="sm"
                              variant="ghost"
                              onClick={() =>
                                actions.expireAnnouncement(ann.id)
                              }
                              title="Expire"
                            >
                              <Timer className="h-4 w-4 text-muted-foreground" />
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

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Plus className="h-5 w-5" />
                Publish Announcement
              </CardTitle>
              <CardDescription>
                Select a category and provide the announcement content
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Category</label>
                <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
                  {ANNOUNCEMENT_CATEGORIES.map((cat) => (
                    <button
                      key={cat}
                      onClick={() => setAnnCategory(cat)}
                      className={`rounded-lg border p-3 text-center text-sm font-medium transition-colors hover:bg-accent ${
                        annCategory === cat
                          ? "border-primary bg-primary/5 ring-1 ring-primary"
                          : "border-border"
                      }`}
                    >
                      {cat}
                    </button>
                  ))}
                </div>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Title</label>
                  <Input
                    value={annTitle}
                    onChange={(e) => setAnnTitle(e.target.value)}
                    placeholder="Announcement title"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Content CID</label>
                  <Input
                    value={annCid}
                    onChange={(e) => setAnnCid(e.target.value)}
                    placeholder="IPFS content identifier"
                  />
                </div>
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">
                  Expires At Block (optional)
                </label>
                <Input
                  value={annExpiresAt}
                  onChange={(e) => setAnnExpiresAt(e.target.value)}
                  placeholder="Block number for auto-expiration"
                  className="max-w-xs"
                />
              </div>

              <TxButton
                onClick={handlePublishAnnouncement}
                txStatus={actions.txState.status}
                disabled={!annCategory || !annTitle || !annCid}
              >
                <Megaphone className="mr-2 h-4 w-4" />
                Publish Announcement
              </TxButton>
            </CardContent>
          </Card>
        </TabsContent>

        {/* ================================================================ */}
        {/* CONFIG TAB                                                        */}
        {/* ================================================================ */}
        <TabsContent value="config" className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Eye className="h-5 w-5" />
                Current Configuration
              </CardTitle>
              <CardDescription>
                Disclosure settings loaded from on-chain state
              </CardDescription>
            </CardHeader>
            <CardContent>
              {configLoading ? (
                <div className="flex justify-center py-8">
                  <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
                </div>
              ) : config ? (
                <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
                  <div className="rounded-lg border p-4 space-y-1">
                    <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                      Disclosure Level
                    </p>
                    <p className="text-lg font-semibold">{config.level}</p>
                  </div>
                  <div className="rounded-lg border p-4 space-y-1">
                    <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                      Insider Trading Control
                    </p>
                    <p className="text-lg font-semibold">
                      {config.insiderTradingControl ? "Enabled" : "Disabled"}
                    </p>
                  </div>
                  <div className="rounded-lg border p-4 space-y-1">
                    <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                      Blackout Period After
                    </p>
                    <p className="text-lg font-semibold">
                      {config.blackoutPeriodAfter} blocks
                    </p>
                  </div>
                  <div className="rounded-lg border p-4 space-y-1">
                    <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                      Last Disclosure
                    </p>
                    <p className="text-lg font-semibold">
                      {config.lastDisclosure ? `Block #${config.lastDisclosure}` : "None"}
                    </p>
                  </div>
                  <div className="rounded-lg border p-4 space-y-1">
                    <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                      Next Required
                    </p>
                    <p className="text-lg font-semibold">
                      {config.nextRequiredDisclosure
                        ? `Block #${config.nextRequiredDisclosure}`
                        : "—"}
                    </p>
                  </div>
                  <div className="rounded-lg border p-4 space-y-1">
                    <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                      Violation Count
                    </p>
                    <p className={`text-lg font-semibold ${config.violationCount > 0 ? "text-red-600" : ""}`}>
                      {config.violationCount}
                    </p>
                  </div>
                </div>
              ) : (
                <p className="text-sm text-muted-foreground">
                  No configuration found. Configure disclosure settings below.
                </p>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Settings className="h-5 w-5" />
                Configure Disclosure
              </CardTitle>
              <CardDescription>
                Set the disclosure level, insider trading controls, and blackout
                period
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Disclosure Level</label>
                <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
                  {DISCLOSURE_LEVELS.map((level) => (
                    <button
                      key={level}
                      onClick={() => setCfgLevel(level)}
                      className={`rounded-lg border p-3 text-center text-sm font-medium transition-colors hover:bg-accent ${
                        cfgLevel === level
                          ? "border-primary bg-primary/5 ring-1 ring-primary"
                          : "border-border"
                      }`}
                    >
                      {level}
                    </button>
                  ))}
                </div>
              </div>

              <div className="flex items-center justify-between rounded-lg border p-4">
                <div className="space-y-0.5">
                  <p className="text-sm font-medium">
                    Insider Trading Control
                  </p>
                  <p className="text-xs text-muted-foreground">
                    Restrict insider trading during sensitive periods
                  </p>
                </div>
                <Switch
                  checked={cfgInsiderControl}
                  onCheckedChange={setCfgInsiderControl}
                />
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">
                  Blackout Period After Disclosure (blocks)
                </label>
                <Input
                  value={cfgBlackoutPeriod}
                  onChange={(e) => setCfgBlackoutPeriod(e.target.value)}
                  placeholder="Number of blocks"
                  className="max-w-xs"
                />
              </div>

              <TxButton
                onClick={handleConfigure}
                txStatus={actions.txState.status}
                disabled={!cfgLevel}
              >
                <CheckCircle className="mr-2 h-4 w-4" />
                Save Configuration
              </TxButton>
            </CardContent>
          </Card>

          <Separator />

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Ban className="h-5 w-5" />
                Blackout Period
              </CardTitle>
              <CardDescription>
                Trading restriction window for insiders
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {blackoutLoading ? (
                <div className="flex justify-center py-4">
                  <div className="h-6 w-6 animate-spin rounded-full border-4 border-primary border-t-transparent" />
                </div>
              ) : blackout ? (
                <div className="rounded-lg border border-amber-300 bg-amber-50 p-4 dark:border-amber-700 dark:bg-amber-950/30">
                  <div className="flex items-center gap-2 text-amber-700 dark:text-amber-400">
                    <ShieldAlert className="h-5 w-5" />
                    <span className="font-semibold">Blackout Active</span>
                  </div>
                  <div className="mt-2 grid gap-2 sm:grid-cols-2 text-sm">
                    <p>
                      <span className="text-muted-foreground">Start:</span>{" "}
                      Block #{blackout.start}
                    </p>
                    <p>
                      <span className="text-muted-foreground">End:</span> Block
                      #{blackout.end}
                    </p>
                  </div>
                  <div className="mt-3 flex gap-2">
                    <TxButton
                      size="sm"
                      variant="destructive"
                      onClick={() => actions.endBlackout(currentEntityId)}
                      txStatus={actions.txState.status}
                    >
                      End Blackout
                    </TxButton>
                    <TxButton
                      size="sm"
                      variant="outline"
                      onClick={() => actions.expireBlackout(currentEntityId)}
                      txStatus={actions.txState.status}
                    >
                      <Timer className="mr-2 h-3 w-3" />
                      Expire
                    </TxButton>
                  </div>
                </div>
              ) : (
                <p className="text-sm text-muted-foreground">
                  No active blackout period.
                </p>
              )}

              {!blackout && (
                <div className="flex items-end gap-4">
                  <div className="flex-1 space-y-2">
                    <label className="text-sm font-medium">
                      Duration (blocks)
                    </label>
                    <Input
                      value={blackoutDuration}
                      onChange={(e) => setBlackoutDuration(e.target.value)}
                      placeholder="e.g. 1000"
                      className="max-w-xs"
                    />
                  </div>
                  <TxButton
                    onClick={handleStartBlackout}
                    txStatus={actions.txState.status}
                    disabled={!blackoutDuration}
                  >
                    <Ban className="mr-2 h-4 w-4" />
                    Start Blackout
                  </TxButton>
                </div>
              )}
            </CardContent>
          </Card>

          <Separator />

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <AlertTriangle className="h-5 w-5" />
                Violation Management
              </CardTitle>
              <CardDescription>
                Report disclosure violations or reset the counter
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Violation Type</label>
                <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
                  {VIOLATION_TYPES.map((vt) => (
                    <button
                      key={vt}
                      onClick={() => setViolationType(vt)}
                      className={`rounded-lg border p-3 text-center text-sm font-medium transition-colors hover:bg-accent ${
                        violationType === vt
                          ? "border-red-500 bg-red-50 text-red-700 ring-1 ring-red-500 dark:bg-red-950/30 dark:text-red-400"
                          : "border-border"
                      }`}
                    >
                      {vt.replace(/([A-Z])/g, " $1").trim()}
                    </button>
                  ))}
                </div>
              </div>
              <div className="flex gap-2">
                <TxButton
                  variant="destructive"
                  onClick={() => {
                    if (violationType && currentEntityId)
                      actions.reportViolation(currentEntityId, violationType);
                  }}
                  txStatus={actions.txState.status}
                  disabled={!violationType}
                >
                  <AlertTriangle className="mr-2 h-4 w-4" />
                  Report Violation
                </TxButton>
                <TxButton
                  variant="outline"
                  onClick={() => actions.resetViolationCount(currentEntityId)}
                  txStatus={actions.txState.status}
                >
                  <RotateCcw className="mr-2 h-4 w-4" />
                  Reset Violations
                </TxButton>
              </div>
            </CardContent>
          </Card>

          {actions.txState.status === "finalized" && (
            <p className="text-sm text-green-600">
              Transaction finalized successfully.
            </p>
          )}
          {actions.txState.status === "error" && (
            <p className="text-sm text-destructive">
              {actions.txState.error}
            </p>
          )}
        </TabsContent>
      </Tabs>
    </div>
  );
}
