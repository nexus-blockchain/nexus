"use client";

import { useState, useMemo } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import {
  Table, TableHeader, TableBody, TableRow, TableHead, TableCell,
} from "@/components/ui/table";
import { TxButton } from "@/components/shared/TxButton";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { StatusBadge } from "@/components/shared/StatusBadge";
import {
  useStorageOperators,
  useStorageActions,
  useOperatorSla,
} from "@/hooks/useStorageService";
import { useWalletStore } from "@/stores/wallet";
import { OPERATOR_LAYERS, OPERATOR_STATUS_MAP } from "@/lib/constants";
import {
  ArrowLeft,
  Server,
  Users,
  HardDrive,
  Pin,
  RotateCcw,
  ChevronDown,
  ChevronUp,
  Shield,
  Pause,
  Play,
  LogOut,
  Gift,
  Plus,
  Minus,
  Activity,
  Settings,
  Layers,
  Slash,
  ArrowRightLeft,
  UserPlus,
} from "lucide-react";
import Link from "next/link";

const formatBal = (b: bigint) => (Number(b) / 1e12).toFixed(4);

const LAYER_COLORS: Record<string, string> = {
  Core: "bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-300",
  Community: "bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-300",
  External: "bg-gray-100 text-gray-800 dark:bg-gray-900/30 dark:text-gray-300",
};

function HealthBar({ score }: { score: number }) {
  const color = score > 80 ? "bg-green-500" : score >= 50 ? "bg-yellow-500" : "bg-red-500";
  return (
    <div className="flex items-center gap-2">
      <div className="h-2 w-16 rounded-full bg-muted overflow-hidden">
        <div className={`h-full rounded-full ${color}`} style={{ width: `${Math.min(score, 100)}%` }} />
      </div>
      <span className="text-xs font-mono">{score}</span>
    </div>
  );
}

function SlaDetail({ account }: { account: string }) {
  const { sla, isLoading } = useOperatorSla(account);

  if (isLoading) return <p className="text-xs text-muted-foreground py-2">Loading SLA...</p>;
  if (!sla) return <p className="text-xs text-muted-foreground py-2">No SLA data available.</p>;

  return (
    <div className="grid gap-3 sm:grid-cols-5 py-2">
      <div className="space-y-0.5">
        <p className="text-xs text-muted-foreground">Pinned Bytes</p>
        <p className="text-sm font-mono font-medium">{sla.pinnedBytes.toLocaleString()}</p>
      </div>
      <div className="space-y-0.5">
        <p className="text-xs text-muted-foreground">Probe OK</p>
        <p className="text-sm font-mono font-medium text-green-600">{sla.probeOk.toLocaleString()}</p>
      </div>
      <div className="space-y-0.5">
        <p className="text-xs text-muted-foreground">Probe Fail</p>
        <p className="text-sm font-mono font-medium text-red-600">{sla.probeFail.toLocaleString()}</p>
      </div>
      <div className="space-y-0.5">
        <p className="text-xs text-muted-foreground">Degraded</p>
        <p className="text-sm font-mono font-medium text-amber-600">{sla.degraded.toLocaleString()}</p>
      </div>
      <div className="space-y-0.5">
        <p className="text-xs text-muted-foreground">Last Update</p>
        <p className="text-sm font-mono font-medium">{sla.lastUpdate.toLocaleString()}</p>
      </div>
    </div>
  );
}

export default function StorageOperatorsPage() {
  const { address } = useWalletStore();
  const { operators, isLoading, refetch } = useStorageOperators();
  const actions = useStorageActions();

  const [expandedSla, setExpandedSla] = useState<string | null>(null);
  const [myOpOpen, setMyOpOpen] = useState(true);

  // Join form
  const [joinPeerId, setJoinPeerId] = useState("");
  const [joinCapacity, setJoinCapacity] = useState("");
  const [joinEndpoint, setJoinEndpoint] = useState("");
  const [joinCert, setJoinCert] = useState("");
  const [joinBond, setJoinBond] = useState("");

  // Update form
  const [updPeerId, setUpdPeerId] = useState("");
  const [updCapacity, setUpdCapacity] = useState("");
  const [updEndpoint, setUpdEndpoint] = useState("");
  const [updCert, setUpdCert] = useState("");
  const [topUpAmt, setTopUpAmt] = useState("");
  const [reduceAmt, setReduceAmt] = useState("");

  // Admin per-operator actions
  const [adminTarget, setAdminTarget] = useState("");
  const [adminStatus, setAdminStatus] = useState("");
  const [adminLayer, setAdminLayer] = useState("");
  const [adminPriority, setAdminPriority] = useState("");
  const [slashTarget, setSlashTarget] = useState("");
  const [slashAmount, setSlashAmount] = useState("");
  const [migrateFrom, setMigrateFrom] = useState("");
  const [migrateTo, setMigrateTo] = useState("");
  const [migrateMax, setMigrateMax] = useState("");

  const myOperator = useMemo(
    () => (address ? operators.find((op) => op.account === address) : undefined),
    [operators, address],
  );

  const stats = useMemo(() => {
    const active = operators.filter((o) => o.status === 1).length;
    const paused = operators.filter((o) => o.status === 2).length;
    const totalCap = operators.reduce((s, o) => s + o.capacityGib, 0);
    const totalPins = operators.reduce((s, o) => s + o.pinCount, 0);
    return { total: operators.length, active, paused, totalCap, totalPins };
  }, [operators]);

  const handleJoin = async () => {
    if (!joinPeerId || !joinCapacity || !joinEndpoint || !joinBond) return;
    await actions.joinOperator(
      joinPeerId,
      Number(joinCapacity),
      joinEndpoint,
      joinCert || null,
      BigInt(joinBond),
    );
    setJoinPeerId(""); setJoinCapacity(""); setJoinEndpoint(""); setJoinCert(""); setJoinBond("");
    refetch();
  };

  const handleUpdate = async () => {
    await actions.updateOperator(
      updPeerId || null,
      updCapacity ? Number(updCapacity) : null,
      updEndpoint || null,
      updCert || null,
    );
    setUpdPeerId(""); setUpdCapacity(""); setUpdEndpoint(""); setUpdCert("");
    refetch();
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/storage"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Server className="h-7 w-7" />
            Storage Operators
          </h1>
          <p className="text-muted-foreground">View operator nodes and manage your operator status</p>
        </div>
        <Button variant="outline" size="sm" onClick={refetch}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {/* Stats */}
      <div className="grid gap-4 md:grid-cols-5">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Operators</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Users className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">{stats.total}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Active</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-green-600">{stats.active}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Paused</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-amber-600">{stats.paused}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Capacity</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <HardDrive className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">{stats.totalCap.toLocaleString()} GiB</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Pins</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Pin className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">{stats.totalPins.toLocaleString()}</p>
          </CardContent>
        </Card>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
        </div>
      ) : (
        <>
          {/* Operators Table */}
          {operators.length === 0 ? (
            <Card>
              <CardContent className="flex flex-col items-center justify-center py-12">
                <Server className="h-12 w-12 text-muted-foreground/50" />
                <p className="mt-4 text-lg font-medium">No operators registered</p>
                <p className="text-sm text-muted-foreground">Storage operators will appear here once they register</p>
              </CardContent>
            </Card>
          ) : (
            <Card>
              <CardHeader>
                <CardTitle>All Operators</CardTitle>
                <CardDescription>{operators.length} registered operator{operators.length !== 1 ? "s" : ""}</CardDescription>
              </CardHeader>
              <CardContent className="overflow-x-auto p-0">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Account</TableHead>
                      <TableHead>Peer ID</TableHead>
                      <TableHead className="text-right">Cap (GiB)</TableHead>
                      <TableHead className="text-right">Used</TableHead>
                      <TableHead className="text-right">Pins</TableHead>
                      <TableHead>Health</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead>Layer</TableHead>
                      <TableHead className="text-right">Priority</TableHead>
                      <TableHead className="text-right">Bond</TableHead>
                      <TableHead className="text-right">Rewards</TableHead>
                      <TableHead className="text-right">Registered</TableHead>
                      <TableHead></TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {operators.map((op) => {
                      const statusLabel = OPERATOR_STATUS_MAP[op.status] ?? `Unknown(${op.status})`;
                      const isSlaOpen = expandedSla === op.account;
                      return (
                        <TableRow key={op.account} className="group">
                          <TableCell><AddressDisplay address={op.account} chars={6} /></TableCell>
                          <TableCell className="font-mono text-xs max-w-[120px] truncate" title={op.peerId}>
                            {op.peerId.length > 16 ? `${op.peerId.slice(0, 8)}…${op.peerId.slice(-6)}` : op.peerId}
                          </TableCell>
                          <TableCell className="text-right font-mono">{op.capacityGib}</TableCell>
                          <TableCell className="text-right font-mono">{op.usedBytes.toLocaleString()}</TableCell>
                          <TableCell className="text-right font-mono">{op.pinCount.toLocaleString()}</TableCell>
                          <TableCell><HealthBar score={op.healthScore} /></TableCell>
                          <TableCell><StatusBadge status={statusLabel} /></TableCell>
                          <TableCell>
                            <span className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-semibold ${LAYER_COLORS[op.layer] || LAYER_COLORS.External}`}>
                              {op.layer}
                            </span>
                          </TableCell>
                          <TableCell className="text-right font-mono">{op.priority}</TableCell>
                          <TableCell className="text-right font-mono">{formatBal(op.bond)}</TableCell>
                          <TableCell className="text-right font-mono">{formatBal(op.rewards)}</TableCell>
                          <TableCell className="text-right font-mono">{op.registeredAt.toLocaleString()}</TableCell>
                          <TableCell>
                            <Button
                              variant="ghost"
                              size="icon"
                              className="h-7 w-7"
                              onClick={() => setExpandedSla(isSlaOpen ? null : op.account)}
                              title="SLA Details"
                            >
                              {isSlaOpen ? <ChevronUp className="h-3.5 w-3.5" /> : <ChevronDown className="h-3.5 w-3.5" />}
                            </Button>
                          </TableCell>
                        </TableRow>
                      );
                    })}
                  </TableBody>
                </Table>
                {expandedSla && (
                  <div className="border-t px-6 py-3 bg-muted/30">
                    <p className="text-xs font-semibold mb-1 flex items-center gap-1.5">
                      <Activity className="h-3.5 w-3.5" />SLA Detail for <AddressDisplay address={expandedSla} chars={8} showCopy={false} />
                    </p>
                    <SlaDetail account={expandedSla} />
                  </div>
                )}
              </CardContent>
            </Card>
          )}

          {/* My Operator Section */}
          {myOperator ? (
            <Card>
              <CardHeader className="cursor-pointer select-none" onClick={() => setMyOpOpen(!myOpOpen)}>
                <div className="flex items-center justify-between">
                  <div>
                    <CardTitle className="flex items-center gap-2">
                      <Shield className="h-5 w-5" />My Operator
                    </CardTitle>
                    <CardDescription>
                      Status: {OPERATOR_STATUS_MAP[myOperator.status] ?? "Unknown"} — Bond: {formatBal(myOperator.bond)} NEX — Rewards: {formatBal(myOperator.rewards)} NEX
                    </CardDescription>
                  </div>
                  {myOpOpen ? <ChevronUp className="h-5 w-5" /> : <ChevronDown className="h-5 w-5" />}
                </div>
              </CardHeader>
              {myOpOpen && (
                <CardContent className="space-y-6">
                  {/* Current details */}
                  <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
                    <div className="rounded-lg border p-3 space-y-0.5">
                      <p className="text-xs text-muted-foreground">Peer ID</p>
                      <p className="text-sm font-mono truncate" title={myOperator.peerId}>{myOperator.peerId}</p>
                    </div>
                    <div className="rounded-lg border p-3 space-y-0.5">
                      <p className="text-xs text-muted-foreground">Capacity</p>
                      <p className="text-sm font-mono">{myOperator.capacityGib} GiB</p>
                    </div>
                    <div className="rounded-lg border p-3 space-y-0.5">
                      <p className="text-xs text-muted-foreground">Used / Pins</p>
                      <p className="text-sm font-mono">{myOperator.usedBytes.toLocaleString()} B / {myOperator.pinCount}</p>
                    </div>
                    <div className="rounded-lg border p-3 space-y-0.5">
                      <p className="text-xs text-muted-foreground">Health Score</p>
                      <HealthBar score={myOperator.healthScore} />
                    </div>
                  </div>

                  <Separator />

                  {/* Update operator form */}
                  <div className="space-y-3">
                    <h3 className="text-sm font-semibold flex items-center gap-2">
                      <Settings className="h-4 w-4" />Update Operator
                    </h3>
                    <p className="text-xs text-muted-foreground">Leave fields empty to keep current values.</p>
                    <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
                      <div className="space-y-1">
                        <label className="text-xs font-medium">Peer ID</label>
                        <Input placeholder={myOperator.peerId} value={updPeerId} onChange={(e) => setUpdPeerId(e.target.value)} />
                      </div>
                      <div className="space-y-1">
                        <label className="text-xs font-medium">Capacity (GiB)</label>
                        <Input type="number" placeholder={String(myOperator.capacityGib)} value={updCapacity} onChange={(e) => setUpdCapacity(e.target.value)} />
                      </div>
                      <div className="space-y-1">
                        <label className="text-xs font-medium">Endpoint Hash</label>
                        <Input placeholder={myOperator.endpointHash} value={updEndpoint} onChange={(e) => setUpdEndpoint(e.target.value)} />
                      </div>
                      <div className="space-y-1">
                        <label className="text-xs font-medium">Cert Fingerprint</label>
                        <Input placeholder={myOperator.certFingerprint ?? "None"} value={updCert} onChange={(e) => setUpdCert(e.target.value)} />
                      </div>
                    </div>
                    <TxButton size="sm" onClick={handleUpdate} txStatus={actions.txState.status}>
                      Update Operator
                    </TxButton>
                  </div>

                  <Separator />

                  {/* Quick actions */}
                  <div className="space-y-3">
                    <h3 className="text-sm font-semibold">Quick Actions</h3>
                    <div className="flex flex-wrap gap-2">
                      {myOperator.status === 1 && (
                        <TxButton size="sm" variant="outline" onClick={() => { actions.pauseOperator(); }} txStatus={actions.txState.status}>
                          <Pause className="mr-1.5 h-3.5 w-3.5" />Pause
                        </TxButton>
                      )}
                      {myOperator.status === 2 && (
                        <TxButton size="sm" variant="outline" onClick={() => { actions.resumeOperator(); }} txStatus={actions.txState.status}>
                          <Play className="mr-1.5 h-3.5 w-3.5" />Resume
                        </TxButton>
                      )}
                      <TxButton size="sm" variant="outline" onClick={() => { actions.operatorClaimRewards(); refetch(); }} txStatus={actions.txState.status}>
                        <Gift className="mr-1.5 h-3.5 w-3.5" />Claim Rewards
                      </TxButton>
                      <TxButton size="sm" variant="destructive" onClick={() => { actions.leaveOperator(); refetch(); }} txStatus={actions.txState.status}>
                        <LogOut className="mr-1.5 h-3.5 w-3.5" />Leave
                      </TxButton>
                    </div>
                  </div>

                  <Separator />

                  {/* Bond management */}
                  <div className="space-y-3">
                    <h3 className="text-sm font-semibold">Bond Management</h3>
                    <p className="text-xs text-muted-foreground">Current bond: {formatBal(myOperator.bond)} NEX</p>
                    <div className="grid gap-3 sm:grid-cols-2">
                      <div className="flex gap-2">
                        <Input
                          type="number"
                          placeholder="Top up amount"
                          value={topUpAmt}
                          onChange={(e) => setTopUpAmt(e.target.value)}
                        />
                        <TxButton
                          size="sm"
                          onClick={() => { if (topUpAmt) { actions.topUpBond(BigInt(topUpAmt)); setTopUpAmt(""); refetch(); } }}
                          txStatus={actions.txState.status}
                          disabled={!topUpAmt}
                        >
                          <Plus className="mr-1 h-3.5 w-3.5" />Top Up
                        </TxButton>
                      </div>
                      <div className="flex gap-2">
                        <Input
                          type="number"
                          placeholder="Reduce amount"
                          value={reduceAmt}
                          onChange={(e) => setReduceAmt(e.target.value)}
                        />
                        <TxButton
                          size="sm"
                          variant="outline"
                          onClick={() => { if (reduceAmt) { actions.reduceBond(BigInt(reduceAmt)); setReduceAmt(""); refetch(); } }}
                          txStatus={actions.txState.status}
                          disabled={!reduceAmt}
                        >
                          <Minus className="mr-1 h-3.5 w-3.5" />Reduce
                        </TxButton>
                      </div>
                    </div>
                  </div>

                  <Separator />

                  {/* SLA */}
                  <div className="space-y-2">
                    <h3 className="text-sm font-semibold flex items-center gap-2">
                      <Activity className="h-4 w-4" />My SLA
                    </h3>
                    <SlaDetail account={myOperator.account} />
                  </div>
                </CardContent>
              )}
            </Card>
          ) : (
            /* Join Operator Form (non-operators only) */
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <UserPlus className="h-5 w-5" />Join as Operator
                </CardTitle>
                <CardDescription>Register as a new storage operator on the network</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
                  <div className="space-y-1">
                    <label className="text-xs font-medium">Peer ID *</label>
                    <Input placeholder="12D3KooW..." value={joinPeerId} onChange={(e) => setJoinPeerId(e.target.value)} />
                  </div>
                  <div className="space-y-1">
                    <label className="text-xs font-medium">Capacity (GiB) *</label>
                    <Input type="number" placeholder="100" value={joinCapacity} onChange={(e) => setJoinCapacity(e.target.value)} min="1" />
                  </div>
                  <div className="space-y-1">
                    <label className="text-xs font-medium">Endpoint Hash *</label>
                    <Input placeholder="0x..." value={joinEndpoint} onChange={(e) => setJoinEndpoint(e.target.value)} />
                  </div>
                  <div className="space-y-1">
                    <label className="text-xs font-medium">Cert Fingerprint</label>
                    <Input placeholder="Optional" value={joinCert} onChange={(e) => setJoinCert(e.target.value)} />
                  </div>
                  <div className="space-y-1">
                    <label className="text-xs font-medium">Bond Amount *</label>
                    <Input type="number" placeholder="Smallest unit" value={joinBond} onChange={(e) => setJoinBond(e.target.value)} />
                  </div>
                </div>
                <TxButton
                  onClick={handleJoin}
                  txStatus={actions.txState.status}
                  disabled={!joinPeerId || !joinCapacity || !joinEndpoint || !joinBond}
                >
                  <UserPlus className="mr-2 h-4 w-4" />Join Operator
                </TxButton>
              </CardContent>
            </Card>
          )}

          {/* Admin Actions */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Shield className="h-5 w-5" />Admin Operator Actions
              </CardTitle>
              <CardDescription>Administrative controls for managing operators</CardDescription>
            </CardHeader>
            <CardContent className="space-y-6">
              {/* Set status */}
              <div className="space-y-3">
                <h3 className="text-sm font-semibold flex items-center gap-2">
                  <Settings className="h-4 w-4" />Set Operator Status
                </h3>
                <div className="grid gap-2 sm:grid-cols-3">
                  <Input placeholder="Operator address" value={adminTarget} onChange={(e) => setAdminTarget(e.target.value)} />
                  <Input
                    type="number"
                    placeholder="Status (0-4)"
                    value={adminStatus}
                    onChange={(e) => setAdminStatus(e.target.value)}
                    min="0" max="4"
                  />
                  <TxButton
                    size="sm"
                    onClick={() => { if (adminTarget && adminStatus !== "") { actions.setOperatorStatus(adminTarget, Number(adminStatus)); } }}
                    txStatus={actions.txState.status}
                    disabled={!adminTarget || adminStatus === ""}
                  >
                    Set Status
                  </TxButton>
                </div>
                <div className="flex gap-1.5 flex-wrap">
                  {Object.entries(OPERATOR_STATUS_MAP).map(([k, v]) => (
                    <Badge key={k} variant="outline" className="text-xs">{k}: {v}</Badge>
                  ))}
                </div>
              </div>

              <Separator />

              {/* Set layer */}
              <div className="space-y-3">
                <h3 className="text-sm font-semibold flex items-center gap-2">
                  <Layers className="h-4 w-4" />Set Operator Layer
                </h3>
                <div className="grid gap-2 sm:grid-cols-4">
                  <Input placeholder="Operator address" value={adminLayer ? adminTarget : ""} onChange={(e) => setAdminTarget(e.target.value)} />
                  <Input placeholder="Layer (Core/Community/External)" value={adminLayer} onChange={(e) => setAdminLayer(e.target.value)} />
                  <Input type="number" placeholder="Priority (optional)" value={adminPriority} onChange={(e) => setAdminPriority(e.target.value)} />
                  <TxButton
                    size="sm"
                    onClick={() => { if (adminTarget && adminLayer) { actions.setOperatorLayer(adminTarget, adminLayer, adminPriority ? Number(adminPriority) : null); } }}
                    txStatus={actions.txState.status}
                    disabled={!adminTarget || !adminLayer}
                  >
                    Set Layer
                  </TxButton>
                </div>
                <div className="flex gap-1.5 flex-wrap">
                  {OPERATOR_LAYERS.map((l) => (
                    <span key={l} className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-semibold ${LAYER_COLORS[l]}`}>
                      {l}
                    </span>
                  ))}
                </div>
              </div>

              <Separator />

              {/* Slash */}
              <div className="space-y-3">
                <h3 className="text-sm font-semibold flex items-center gap-2">
                  <Slash className="h-4 w-4" />Slash Operator
                </h3>
                <div className="grid gap-2 sm:grid-cols-3">
                  <Input placeholder="Operator address" value={slashTarget} onChange={(e) => setSlashTarget(e.target.value)} />
                  <Input type="number" placeholder="Amount (smallest unit)" value={slashAmount} onChange={(e) => setSlashAmount(e.target.value)} />
                  <TxButton
                    size="sm"
                    variant="destructive"
                    onClick={() => { if (slashTarget && slashAmount) { actions.slashOperator(slashTarget, BigInt(slashAmount)); } }}
                    txStatus={actions.txState.status}
                    disabled={!slashTarget || !slashAmount}
                  >
                    Slash
                  </TxButton>
                </div>
              </div>

              <Separator />

              {/* Migrate pins */}
              <div className="space-y-3">
                <h3 className="text-sm font-semibold flex items-center gap-2">
                  <ArrowRightLeft className="h-4 w-4" />Migrate Operator Pins
                </h3>
                <div className="grid gap-2 sm:grid-cols-4">
                  <Input placeholder="From (address)" value={migrateFrom} onChange={(e) => setMigrateFrom(e.target.value)} />
                  <Input placeholder="To (address)" value={migrateTo} onChange={(e) => setMigrateTo(e.target.value)} />
                  <Input type="number" placeholder="Max pins" value={migrateMax} onChange={(e) => setMigrateMax(e.target.value)} />
                  <TxButton
                    size="sm"
                    onClick={() => { if (migrateFrom && migrateTo && migrateMax) { actions.migrateOperatorPins(migrateFrom, migrateTo, Number(migrateMax)); } }}
                    txStatus={actions.txState.status}
                    disabled={!migrateFrom || !migrateTo || !migrateMax}
                  >
                    Migrate
                  </TxButton>
                </div>
              </div>
            </CardContent>
          </Card>
        </>
      )}

      {actions.txState.status === "finalized" && (
        <p className="text-sm text-green-600">Transaction finalized successfully.</p>
      )}
      {actions.txState.status === "error" && (
        <p className="text-sm text-destructive">Error: {actions.txState.error}</p>
      )}
    </div>
  );
}
