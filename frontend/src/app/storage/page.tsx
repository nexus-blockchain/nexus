"use client";

import { useState, useMemo, useCallback } from "react";
import { useWalletStore } from "@/stores/wallet";
import {
  useStoragePins, useHealthStats, useTierConfigs,
  useRegisteredDomains, useStorageActions,
} from "@/hooks/useStorageService";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { TxButton } from "@/components/shared/TxButton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Table, TableHeader, TableBody, TableRow, TableHead, TableCell,
} from "@/components/ui/table";
import {
  HardDrive, Database, CheckCircle2, AlertTriangle, XCircle, Shield,
  RotateCcw, Plus, Trash2, ArrowUpCircle, Clock, Globe, Wrench,
  Activity, Heart, Filter, Eye, Hash,
} from "lucide-react";
import { PIN_TIERS, PIN_STATES, SUBJECT_TYPES } from "@/lib/constants";

const formatSize = (bytes: number) =>
  bytes < 1024
    ? bytes + " B"
    : bytes < 1048576
      ? (bytes / 1024).toFixed(1) + " KB"
      : (bytes / 1048576).toFixed(2) + " MB";

function tierBadgeVariant(tier: string) {
  switch (tier) {
    case "Critical": return "destructive" as const;
    case "Standard": return "default" as const;
    default: return "secondary" as const;
  }
}

function stateBadgeColor(state: string) {
  switch (state) {
    case "Pinned": return "text-green-600 dark:text-green-400";
    case "Pending": return "text-yellow-600 dark:text-yellow-400";
    case "Failed": return "text-red-600 dark:text-red-400";
    case "Restored": return "text-blue-600 dark:text-blue-400";
    default: return "text-muted-foreground";
  }
}

function stateIcon(state: string) {
  switch (state) {
    case "Pinned": return <CheckCircle2 className="h-3.5 w-3.5 text-green-500" />;
    case "Pending": return <Clock className="h-3.5 w-3.5 text-yellow-500" />;
    case "Failed": return <XCircle className="h-3.5 w-3.5 text-red-500" />;
    case "Restored": return <RotateCcw className="h-3.5 w-3.5 text-blue-500" />;
    default: return null;
  }
}

// ---------------------------------------------------------------------------
// My Pins Tab
// ---------------------------------------------------------------------------

function MyPinsTab() {
  const { address } = useWalletStore();
  const { pins, isLoading, refetch } = useStoragePins(address);
  const actions = useStorageActions();

  const [tierFilter, setTierFilter] = useState<string | null>(null);
  const [stateFilter, setStateFilter] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());

  const [renewCid, setRenewCid] = useState<string | null>(null);
  const [renewPeriods, setRenewPeriods] = useState("1");
  const [upgradeCid, setUpgradeCid] = useState<string | null>(null);
  const [upgradeTier, setUpgradeTier] = useState<string>("Critical");
  const [viewHash, setViewHash] = useState<string | null>(null);

  const [pinCid, setPinCid] = useState("");
  const [pinSubject, setPinSubject] = useState("");
  const [pinSize, setPinSize] = useState("");
  const [pinTier, setPinTier] = useState<string>("Standard");

  const filtered = useMemo(() => {
    let list = pins;
    if (tierFilter) list = list.filter((p) => p.tier === tierFilter);
    if (stateFilter) list = list.filter((p) => p.state === stateFilter);
    return list;
  }, [pins, tierFilter, stateFilter]);

  const totalSize = useMemo(() => pins.reduce((s, p) => s + p.size, 0), [pins]);
  const pinnedCount = useMemo(() => pins.filter((p) => p.state === "Pinned").length, [pins]);
  const failedCount = useMemo(() => pins.filter((p) => p.state === "Failed").length, [pins]);

  const toggleSelect = useCallback((cidHash: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      next.has(cidHash) ? next.delete(cidHash) : next.add(cidHash);
      return next;
    });
  }, []);

  const toggleAll = useCallback(() => {
    setSelected((prev) =>
      prev.size === filtered.length
        ? new Set()
        : new Set(filtered.map((p) => p.cid)),
    );
  }, [filtered]);

  const handleBatchUnpin = () => {
    if (selected.size === 0) return;
    actions.batchUnpin(Array.from(selected));
  };

  const handleRequestPin = () => {
    if (!pinCid) return;
    actions.requestPinForSubject(
      pinSubject ? Number(pinSubject) : 0,
      pinCid,
      pinSize ? Number(pinSize) : 0,
      pinTier || null,
    );
  };

  if (!address) {
    return (
      <Card>
        <CardContent className="flex flex-col items-center justify-center py-12">
          <HardDrive className="h-12 w-12 text-muted-foreground/50" />
          <p className="mt-4 text-lg font-medium">Connect Wallet</p>
          <p className="text-sm text-muted-foreground">Connect your wallet to view your pinned content</p>
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      {/* Stats */}
      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardContent className="pt-6">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-blue-100 dark:bg-blue-900/30">
                <Database className="h-5 w-5 text-blue-600 dark:text-blue-400" />
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Total Pins</p>
                <p className="text-2xl font-bold">{pins.length}</p>
              </div>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="pt-6">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-purple-100 dark:bg-purple-900/30">
                <HardDrive className="h-5 w-5 text-purple-600 dark:text-purple-400" />
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Total Size</p>
                <p className="text-2xl font-bold">{formatSize(totalSize)}</p>
              </div>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="pt-6">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-green-100 dark:bg-green-900/30">
                <CheckCircle2 className="h-5 w-5 text-green-600 dark:text-green-400" />
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Pinned</p>
                <p className="text-2xl font-bold">{pinnedCount}</p>
              </div>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="pt-6">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-red-100 dark:bg-red-900/30">
                <XCircle className="h-5 w-5 text-red-600 dark:text-red-400" />
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Failed</p>
                <p className="text-2xl font-bold">{failedCount}</p>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Filters */}
      <div className="flex flex-wrap items-center gap-2">
        <Filter className="h-4 w-4 text-muted-foreground" />
        <span className="text-xs font-medium text-muted-foreground mr-1">Tier:</span>
        <Button variant={tierFilter === null ? "default" : "outline"} size="sm" onClick={() => setTierFilter(null)}>All</Button>
        {PIN_TIERS.map((t) => (
          <Button key={t} variant={tierFilter === t ? "default" : "outline"} size="sm" onClick={() => setTierFilter(tierFilter === t ? null : t)}>{t}</Button>
        ))}
        <Separator orientation="vertical" className="mx-1 h-6" />
        <span className="text-xs font-medium text-muted-foreground mr-1">State:</span>
        <Button variant={stateFilter === null ? "default" : "outline"} size="sm" onClick={() => setStateFilter(null)}>All</Button>
        {PIN_STATES.map((s) => (
          <Button key={s} variant={stateFilter === s ? "default" : "outline"} size="sm" onClick={() => setStateFilter(stateFilter === s ? null : s)}>{s}</Button>
        ))}
        <div className="ml-auto flex gap-2">
          {selected.size > 0 && (
            <TxButton variant="destructive" size="sm" onClick={handleBatchUnpin} txStatus={actions.txState.status}>
              <Trash2 className="mr-1 h-3.5 w-3.5" />Unpin {selected.size}
            </TxButton>
          )}
          <Button variant="outline" size="sm" onClick={() => refetch()}>
            <RotateCcw className="mr-2 h-3 w-3" />Refresh
          </Button>
        </div>
      </div>

      {/* Pins Table */}
      {isLoading ? (
        <div className="flex justify-center py-8">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
        </div>
      ) : filtered.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <HardDrive className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No pins found</p>
            <p className="text-sm text-muted-foreground">
              {tierFilter || stateFilter ? "Try adjusting your filters." : "Request a pin to start storing on IPFS."}
            </p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-10">
                  <input
                    type="checkbox"
                    className="h-4 w-4 rounded border-gray-300"
                    checked={selected.size === filtered.length && filtered.length > 0}
                    onChange={toggleAll}
                  />
                </TableHead>
                <TableHead>CID</TableHead>
                <TableHead>State</TableHead>
                <TableHead>Tier</TableHead>
                <TableHead className="text-right">Size</TableHead>
                <TableHead className="text-right">Replicas</TableHead>
                <TableHead className="text-right">Created</TableHead>
                <TableHead className="text-right">Last Activity</TableHead>
                <TableHead className="text-right">Subject</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {filtered.map((pin) => (
                <TableRow key={pin.cidHash} className={selected.has(pin.cid) ? "bg-muted/40" : ""}>
                  <TableCell>
                    <input
                      type="checkbox"
                      className="h-4 w-4 rounded border-gray-300"
                      checked={selected.has(pin.cid)}
                      onChange={() => toggleSelect(pin.cid)}
                    />
                  </TableCell>
                  <TableCell className="font-mono text-xs max-w-[180px] truncate" title={pin.cid}>
                    {pin.cid.length > 16 ? pin.cid.slice(0, 8) + "..." + pin.cid.slice(-8) : pin.cid}
                  </TableCell>
                  <TableCell>
                    <div className="flex items-center gap-1.5">
                      {stateIcon(pin.state)}
                      <StatusBadge status={pin.state} />
                    </div>
                  </TableCell>
                  <TableCell>
                    <Badge variant={tierBadgeVariant(pin.tier)}>{pin.tier}</Badge>
                  </TableCell>
                  <TableCell className="text-right font-mono text-xs">{formatSize(pin.size)}</TableCell>
                  <TableCell className="text-right font-mono">{pin.replicas}</TableCell>
                  <TableCell className="text-right font-mono text-xs">#{pin.createdAt}</TableCell>
                  <TableCell className="text-right font-mono text-xs">#{pin.lastActivity}</TableCell>
                  <TableCell className="text-right font-mono text-xs">
                    {pin.subjectId != null ? pin.subjectId : "—"}
                  </TableCell>
                  <TableCell>
                    <div className="flex justify-end gap-1">
                      <Button
                        variant="ghost" size="sm" className="h-7 px-2"
                        title="View CID Hash"
                        onClick={() => setViewHash(viewHash === pin.cidHash ? null : pin.cidHash)}
                      >
                        <Eye className="h-3.5 w-3.5" />
                      </Button>
                      <TxButton
                        variant="ghost" size="sm" className="h-7 px-2 text-destructive"
                        title="Unpin"
                        onClick={() => actions.requestUnpin(pin.cid)}
                        txStatus={actions.txState.status}
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </TxButton>
                      <Button
                        variant="ghost" size="sm" className="h-7 px-2"
                        title="Renew"
                        onClick={() => setRenewCid(renewCid === pin.cidHash ? null : pin.cidHash)}
                      >
                        <RotateCcw className="h-3.5 w-3.5" />
                      </Button>
                      <Button
                        variant="ghost" size="sm" className="h-7 px-2"
                        title="Upgrade Tier"
                        onClick={() => setUpgradeCid(upgradeCid === pin.cidHash ? null : pin.cidHash)}
                      >
                        <ArrowUpCircle className="h-3.5 w-3.5" />
                      </Button>
                    </div>

                    {viewHash === pin.cidHash && (
                      <div className="mt-1 rounded bg-muted p-1.5 text-xs font-mono break-all">
                        <Hash className="mr-1 inline h-3 w-3" />{pin.cidHash}
                      </div>
                    )}

                    {renewCid === pin.cidHash && (
                      <div className="mt-1 flex items-center gap-1">
                        <Input
                          type="number" min={1} value={renewPeriods}
                          onChange={(e) => setRenewPeriods(e.target.value)}
                          className="h-7 w-20 text-xs"
                          placeholder="Periods"
                        />
                        <TxButton
                          size="sm" className="h-7 text-xs"
                          onClick={() => { actions.renewPin(pin.cidHash, Number(renewPeriods)); setRenewCid(null); }}
                          txStatus={actions.txState.status}
                        >
                          Renew
                        </TxButton>
                      </div>
                    )}

                    {upgradeCid === pin.cidHash && (
                      <div className="mt-1 flex items-center gap-1">
                        <select
                          value={upgradeTier}
                          onChange={(e) => setUpgradeTier(e.target.value)}
                          className="h-7 rounded border bg-background px-2 text-xs"
                        >
                          {PIN_TIERS.map((t) => <option key={t} value={t}>{t}</option>)}
                        </select>
                        <TxButton
                          size="sm" className="h-7 text-xs"
                          onClick={() => { actions.upgradePinTier(pin.cidHash, upgradeTier); setUpgradeCid(null); }}
                          txStatus={actions.txState.status}
                        >
                          Upgrade
                        </TxButton>
                      </div>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}

      {/* Request Pin Form */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Plus className="h-5 w-5" />Request Pin
          </CardTitle>
          <CardDescription>Pin new content to the decentralized IPFS storage network</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <label className="text-sm font-medium">CID</label>
              <Input value={pinCid} onChange={(e) => setPinCid(e.target.value)} placeholder="QmYwAPJzv5CZsnA..." />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Subject ID</label>
              <Input type="number" value={pinSubject} onChange={(e) => setPinSubject(e.target.value)} placeholder="0" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Size (bytes)</label>
              <Input type="number" value={pinSize} onChange={(e) => setPinSize(e.target.value)} placeholder="1048576" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Tier</label>
              <select
                value={pinTier}
                onChange={(e) => setPinTier(e.target.value)}
                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background"
              >
                {PIN_TIERS.map((t) => <option key={t} value={t}>{t}</option>)}
              </select>
            </div>
          </div>
          <TxButton className="mt-4" onClick={handleRequestPin} txStatus={actions.txState.status} disabled={!pinCid}>
            <Plus className="mr-2 h-4 w-4" />Submit Pin Request
          </TxButton>
        </CardContent>
      </Card>

      <TxFeedback txState={actions.txState} resetTx={actions.resetTx} onSuccess={refetch} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Health Overview Tab
// ---------------------------------------------------------------------------

function HealthOverviewTab() {
  const { stats, isLoading: statsLoading, refetch: refetchStats } = useHealthStats();
  const { configs, isLoading: configsLoading } = useTierConfigs();
  const actions = useStorageActions();

  const [cleanupLimit, setCleanupLimit] = useState("50");

  const healthTotal = (stats?.healthyCount ?? 0) + (stats?.degradedCount ?? 0) + (stats?.criticalCount ?? 0);
  const healthyPct = healthTotal > 0 ? ((stats?.healthyCount ?? 0) / healthTotal) * 100 : 0;
  const degradedPct = healthTotal > 0 ? ((stats?.degradedCount ?? 0) / healthTotal) * 100 : 0;
  const criticalPct = healthTotal > 0 ? ((stats?.criticalCount ?? 0) / healthTotal) * 100 : 0;

  const totalSizeGiB = stats ? (stats.totalSizeBytes / (1024 * 1024 * 1024)).toFixed(2) : "0.00";

  return (
    <div className="space-y-6">
      {/* Stats Cards */}
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold">Network Health</h2>
        <Button variant="outline" size="sm" onClick={() => refetchStats()}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {statsLoading ? (
        <div className="flex justify-center py-8">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
        </div>
      ) : !stats ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Activity className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No health data</p>
            <p className="text-sm text-muted-foreground">Health statistics are not yet available.</p>
          </CardContent>
        </Card>
      ) : (
        <>
          <div className="grid gap-4 md:grid-cols-3 lg:grid-cols-6">
            <Card>
              <CardContent className="pt-6">
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-blue-100 dark:bg-blue-900/30">
                    <Database className="h-5 w-5 text-blue-600 dark:text-blue-400" />
                  </div>
                  <div>
                    <p className="text-xs text-muted-foreground">Total Pins</p>
                    <p className="text-xl font-bold">{stats.totalPins}</p>
                  </div>
                </div>
              </CardContent>
            </Card>
            <Card>
              <CardContent className="pt-6">
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-purple-100 dark:bg-purple-900/30">
                    <HardDrive className="h-5 w-5 text-purple-600 dark:text-purple-400" />
                  </div>
                  <div>
                    <p className="text-xs text-muted-foreground">Total Size</p>
                    <p className="text-xl font-bold">{totalSizeGiB} GiB</p>
                  </div>
                </div>
              </CardContent>
            </Card>
            <Card>
              <CardContent className="pt-6">
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-green-100 dark:bg-green-900/30">
                    <Heart className="h-5 w-5 text-green-600 dark:text-green-400" />
                  </div>
                  <div>
                    <p className="text-xs text-muted-foreground">Healthy</p>
                    <p className="text-xl font-bold text-green-600">{stats.healthyCount}</p>
                  </div>
                </div>
              </CardContent>
            </Card>
            <Card>
              <CardContent className="pt-6">
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-yellow-100 dark:bg-yellow-900/30">
                    <AlertTriangle className="h-5 w-5 text-yellow-600 dark:text-yellow-400" />
                  </div>
                  <div>
                    <p className="text-xs text-muted-foreground">Degraded</p>
                    <p className="text-xl font-bold text-yellow-600">{stats.degradedCount}</p>
                  </div>
                </div>
              </CardContent>
            </Card>
            <Card>
              <CardContent className="pt-6">
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-red-100 dark:bg-red-900/30">
                    <XCircle className="h-5 w-5 text-red-600 dark:text-red-400" />
                  </div>
                  <div>
                    <p className="text-xs text-muted-foreground">Critical</p>
                    <p className="text-xl font-bold text-red-600">{stats.criticalCount}</p>
                  </div>
                </div>
              </CardContent>
            </Card>
            <Card>
              <CardContent className="pt-6">
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-cyan-100 dark:bg-cyan-900/30">
                    <Wrench className="h-5 w-5 text-cyan-600 dark:text-cyan-400" />
                  </div>
                  <div>
                    <p className="text-xs text-muted-foreground">Repairs</p>
                    <p className="text-xl font-bold">{stats.totalRepairs}</p>
                  </div>
                </div>
              </CardContent>
            </Card>
          </div>

          {/* Distribution Bar */}
          <Card>
            <CardHeader className="pb-3">
              <CardTitle className="text-sm font-medium">Health Distribution</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              <div className="flex h-4 w-full overflow-hidden rounded-full bg-muted">
                {healthyPct > 0 && (
                  <div className="bg-green-500 transition-all" style={{ width: `${healthyPct}%` }} />
                )}
                {degradedPct > 0 && (
                  <div className="bg-yellow-500 transition-all" style={{ width: `${degradedPct}%` }} />
                )}
                {criticalPct > 0 && (
                  <div className="bg-red-500 transition-all" style={{ width: `${criticalPct}%` }} />
                )}
              </div>
              <div className="flex gap-6 text-sm">
                <span className="flex items-center gap-1.5">
                  <span className="h-2.5 w-2.5 rounded-full bg-green-500" />
                  Healthy {healthyPct.toFixed(1)}%
                </span>
                <span className="flex items-center gap-1.5">
                  <span className="h-2.5 w-2.5 rounded-full bg-yellow-500" />
                  Degraded {degradedPct.toFixed(1)}%
                </span>
                <span className="flex items-center gap-1.5">
                  <span className="h-2.5 w-2.5 rounded-full bg-red-500" />
                  Critical {criticalPct.toFixed(1)}%
                </span>
              </div>
              <p className="text-xs text-muted-foreground">
                Last full scan: block #{stats.lastFullScan}
              </p>
            </CardContent>
          </Card>
        </>
      )}

      {/* Tier Configs */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <Shield className="h-5 w-5" />Tier Configurations
          </CardTitle>
        </CardHeader>
        <CardContent>
          {configsLoading ? (
            <div className="flex justify-center py-4">
              <div className="h-6 w-6 animate-spin rounded-full border-4 border-primary border-t-transparent" />
            </div>
          ) : Object.keys(configs).length === 0 ? (
            <p className="text-sm text-muted-foreground">No tier configurations found.</p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Tier</TableHead>
                  <TableHead className="text-right">Replicas</TableHead>
                  <TableHead className="text-right">Health Check Interval</TableHead>
                  <TableHead className="text-right">Fee Multiplier</TableHead>
                  <TableHead className="text-right">Grace Period</TableHead>
                  <TableHead className="text-center">Enabled</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {Object.entries(configs).map(([tier, cfg]) => (
                  <TableRow key={tier}>
                    <TableCell>
                      <Badge variant={tierBadgeVariant(tier)}>{tier}</Badge>
                    </TableCell>
                    <TableCell className="text-right font-mono">{cfg.replicas}</TableCell>
                    <TableCell className="text-right font-mono">{cfg.healthCheckInterval} blocks</TableCell>
                    <TableCell className="text-right font-mono">{cfg.feeMultiplier}x</TableCell>
                    <TableCell className="text-right font-mono">{cfg.gracePeriodBlocks} blocks</TableCell>
                    <TableCell className="text-center">
                      <Badge variant={cfg.enabled ? "default" : "secondary"}>
                        {cfg.enabled ? "Yes" : "No"}
                      </Badge>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      {/* Cleanup */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <Trash2 className="h-5 w-5" />Cleanup Expired CIDs
          </CardTitle>
          <CardDescription>Remove expired pin entries from storage to free up capacity</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex items-center gap-3">
            <Input
              type="number" min={1} value={cleanupLimit}
              onChange={(e) => setCleanupLimit(e.target.value)}
              className="w-32" placeholder="Limit"
            />
            <TxButton
              onClick={() => actions.cleanupExpiredCids(Number(cleanupLimit))}
              txStatus={actions.txState.status}
            >
              <Trash2 className="mr-2 h-4 w-4" />Run Cleanup
            </TxButton>
          </div>
        </CardContent>
      </Card>

      <TxFeedback txState={actions.txState} resetTx={actions.resetTx} onSuccess={refetchStats} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Domains Tab
// ---------------------------------------------------------------------------

function DomainsTab() {
  const { domains, isLoading, refetch } = useRegisteredDomains();
  const actions = useStorageActions();

  const [regName, setRegName] = useState("");
  const [regSubjectType, setRegSubjectType] = useState("0");
  const [regTier, setRegTier] = useState<string>("Standard");
  const [regAutoPin, setRegAutoPin] = useState(true);

  const [editDomain, setEditDomain] = useState<string | null>(null);
  const [editAutoPin, setEditAutoPin] = useState<boolean | null>(null);
  const [editTier, setEditTier] = useState<string | null>(null);
  const [editSubjectType, setEditSubjectType] = useState<string | null>(null);

  const handleRegister = () => {
    if (!regName) return;
    actions.registerDomain(regName, Number(regSubjectType), regTier, regAutoPin);
  };

  const handleUpdate = (name: string) => {
    actions.updateDomainConfig(name, editAutoPin, editTier, editSubjectType ? Number(editSubjectType) : null);
    setEditDomain(null);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold">Registered Domains</h2>
        <Button variant="outline" size="sm" onClick={() => refetch()}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-8">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
        </div>
      ) : domains.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Globe className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No domains registered</p>
            <p className="text-sm text-muted-foreground">Register a domain to enable auto-pinning for a pallet.</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Subject Type</TableHead>
                <TableHead>Default Tier</TableHead>
                <TableHead className="text-center">Auto-Pin</TableHead>
                <TableHead>Owner Pallet</TableHead>
                <TableHead className="text-right">Priority</TableHead>
                <TableHead className="text-right">Created</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {domains.map((d) => (
                <TableRow key={d.name}>
                  <TableCell className="font-medium">{d.name}</TableCell>
                  <TableCell>
                    <Badge variant="outline">
                      {SUBJECT_TYPES[d.config.subjectTypeId] ?? `Type ${d.config.subjectTypeId}`}
                    </Badge>
                  </TableCell>
                  <TableCell>
                    <Badge variant={tierBadgeVariant(d.config.defaultTier)}>{d.config.defaultTier}</Badge>
                  </TableCell>
                  <TableCell className="text-center">
                    <Badge variant={d.config.autoPinEnabled ? "default" : "secondary"}>
                      {d.config.autoPinEnabled ? "On" : "Off"}
                    </Badge>
                  </TableCell>
                  <TableCell className="font-mono text-xs">{d.config.ownerPallet}</TableCell>
                  <TableCell className="text-right font-mono">{d.priority}</TableCell>
                  <TableCell className="text-right font-mono text-xs">#{d.config.createdAt}</TableCell>
                  <TableCell className="text-right">
                    <Button
                      variant="ghost" size="sm" className="h-7 px-2"
                      onClick={() => {
                        if (editDomain === d.name) { setEditDomain(null); return; }
                        setEditDomain(d.name);
                        setEditAutoPin(d.config.autoPinEnabled);
                        setEditTier(d.config.defaultTier);
                        setEditSubjectType(String(d.config.subjectTypeId));
                      }}
                    >
                      <Wrench className="h-3.5 w-3.5" />
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>

          {editDomain && (
            <div className="border-t p-4 space-y-3">
              <p className="text-sm font-medium">Update &ldquo;{editDomain}&rdquo;</p>
              <div className="grid gap-3 sm:grid-cols-3">
                <div className="space-y-1">
                  <label className="text-xs font-medium">Auto-Pin</label>
                  <select
                    value={editAutoPin ? "true" : "false"}
                    onChange={(e) => setEditAutoPin(e.target.value === "true")}
                    className="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm"
                  >
                    <option value="true">Enabled</option>
                    <option value="false">Disabled</option>
                  </select>
                </div>
                <div className="space-y-1">
                  <label className="text-xs font-medium">Default Tier</label>
                  <select
                    value={editTier ?? ""}
                    onChange={(e) => setEditTier(e.target.value || null)}
                    className="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm"
                  >
                    {PIN_TIERS.map((t) => <option key={t} value={t}>{t}</option>)}
                  </select>
                </div>
                <div className="space-y-1">
                  <label className="text-xs font-medium">Subject Type</label>
                  <select
                    value={editSubjectType ?? ""}
                    onChange={(e) => setEditSubjectType(e.target.value || null)}
                    className="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm"
                  >
                    {SUBJECT_TYPES.map((s, i) => <option key={s} value={String(i)}>{s}</option>)}
                  </select>
                </div>
              </div>
              <TxButton size="sm" onClick={() => handleUpdate(editDomain)} txStatus={actions.txState.status}>
                Save Changes
              </TxButton>
            </div>
          )}
        </Card>
      )}

      {/* Register Domain */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Plus className="h-5 w-5" />Register Domain
          </CardTitle>
          <CardDescription>Register a new storage domain for auto-pinning integration</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <label className="text-sm font-medium">Domain Name</label>
              <Input value={regName} onChange={(e) => setRegName(e.target.value)} placeholder="my-domain" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Subject Type</label>
              <select
                value={regSubjectType}
                onChange={(e) => setRegSubjectType(e.target.value)}
                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background"
              >
                {SUBJECT_TYPES.map((s, i) => <option key={s} value={String(i)}>{s}</option>)}
              </select>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Default Tier</label>
              <select
                value={regTier}
                onChange={(e) => setRegTier(e.target.value)}
                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background"
              >
                {PIN_TIERS.map((t) => <option key={t} value={t}>{t}</option>)}
              </select>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Auto-Pin</label>
              <select
                value={regAutoPin ? "true" : "false"}
                onChange={(e) => setRegAutoPin(e.target.value === "true")}
                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background"
              >
                <option value="true">Enabled</option>
                <option value="false">Disabled</option>
              </select>
            </div>
          </div>
          <TxButton className="mt-4" onClick={handleRegister} txStatus={actions.txState.status} disabled={!regName}>
            <Globe className="mr-2 h-4 w-4" />Register Domain
          </TxButton>
        </CardContent>
      </Card>

      <TxFeedback txState={actions.txState} resetTx={actions.resetTx} onSuccess={refetch} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Shared Tx Feedback
// ---------------------------------------------------------------------------

function TxFeedback({ txState, resetTx, onSuccess }: {
  txState: { status: string; error?: string | null };
  resetTx: () => void;
  onSuccess?: () => void;
}) {
  if (txState.status === "finalized") {
    return (
      <div className="rounded-lg border border-green-200 bg-green-50 p-3 text-sm text-green-700 dark:border-green-800 dark:bg-green-900/20 dark:text-green-400">
        Transaction completed successfully!
        <Button variant="ghost" size="sm" className="ml-2" onClick={() => { resetTx(); onSuccess?.(); }}>
          Dismiss
        </Button>
      </div>
    );
  }
  if (txState.status === "error") {
    return (
      <div className="rounded-lg border border-red-200 bg-red-50 p-3 text-sm text-destructive dark:border-red-800 dark:bg-red-900/20">
        {txState.error}
        <Button variant="ghost" size="sm" className="ml-2" onClick={resetTx}>Dismiss</Button>
      </div>
    );
  }
  return null;
}

// ---------------------------------------------------------------------------
// Main Page
// ---------------------------------------------------------------------------

export default function StoragePage() {
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
          <HardDrive className="h-8 w-8" />
          IPFS Storage
        </h1>
        <p className="text-muted-foreground">Manage pinned content, network health, and storage domains</p>
      </div>

      <Tabs defaultValue="pins">
        <TabsList>
          <TabsTrigger value="pins">
            <Database className="mr-2 h-4 w-4" />My Pins
          </TabsTrigger>
          <TabsTrigger value="health">
            <Activity className="mr-2 h-4 w-4" />Health Overview
          </TabsTrigger>
          <TabsTrigger value="domains">
            <Globe className="mr-2 h-4 w-4" />Domains
          </TabsTrigger>
        </TabsList>

        <TabsContent value="pins"><MyPinsTab /></TabsContent>
        <TabsContent value="health"><HealthOverviewTab /></TabsContent>
        <TabsContent value="domains"><DomainsTab /></TabsContent>
      </Tabs>
    </div>
  );
}
