"use client";

import { useState, useEffect } from "react";
import { useEntityStore } from "@/stores/entity";
import {
  useCommissionConfig, useCommissionActions, useReferralConfig,
  useMultiLevelConfig, useLevelDiffConfig, useSingleLineConfig,
  useTeamConfig, usePoolRewardConfig, useWithdrawalConfig,
  COMMISSION_MODE_BITS, COMMISSION_MODE_LABELS,
} from "@/hooks/useCommission";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { TxButton } from "@/components/shared/TxButton";
import { basisPointsToPercent } from "@/lib/utils";
import {
  ArrowLeft, Settings, GitBranch, Layers, TrendingUp, Gift,
  Users, DollarSign, Repeat, ArrowUp, ArrowDown, Crown, Star,
  Power, RotateCcw, ChevronDown, ChevronUp, Shield,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const MODE_ICONS: Record<number, typeof GitBranch> = {
  [COMMISSION_MODE_BITS.DIRECT_REWARD]: GitBranch,
  [COMMISSION_MODE_BITS.MULTI_LEVEL]: Layers,
  [COMMISSION_MODE_BITS.TEAM_PERFORMANCE]: Users,
  [COMMISSION_MODE_BITS.LEVEL_DIFF]: TrendingUp,
  [COMMISSION_MODE_BITS.FIXED_AMOUNT]: DollarSign,
  [COMMISSION_MODE_BITS.FIRST_ORDER]: Star,
  [COMMISSION_MODE_BITS.REPEAT_PURCHASE]: Repeat,
  [COMMISSION_MODE_BITS.SINGLE_LINE_UPLINE]: ArrowUp,
  [COMMISSION_MODE_BITS.SINGLE_LINE_DOWNLINE]: ArrowDown,
  [COMMISSION_MODE_BITS.POOL_REWARD]: Gift,
  [COMMISSION_MODE_BITS.CREATOR_REWARD]: Crown,
};

export default function CommissionConfigPage() {
  const { currentEntityId } = useEntityStore();
  const { config, isLoading, refetch } = useCommissionConfig(currentEntityId);
  const { config: referralConfig, refetch: refetchReferral } = useReferralConfig(currentEntityId);
  const { config: multiLevelConfig, refetch: refetchMulti } = useMultiLevelConfig(currentEntityId);
  const { config: levelDiffConfig, refetch: refetchLevelDiff } = useLevelDiffConfig(currentEntityId);
  const { config: singleLineConfig, refetch: refetchSingleLine } = useSingleLineConfig(currentEntityId);
  const { config: teamConfig, refetch: refetchTeam } = useTeamConfig(currentEntityId);
  const { config: poolConfig, refetch: refetchPool } = usePoolRewardConfig(currentEntityId);
  const { config: withdrawalConfig } = useWithdrawalConfig(currentEntityId);
  const actions = useCommissionActions();
  const tc = useTranslations("common");

  const [modes, setModes] = useState(0);
  const [maxRate, setMaxRate] = useState("1000");
  const [creatorRate, setCreatorRate] = useState("0");
  const [expandedMode, setExpandedMode] = useState<number | null>(null);

  const [directRate, setDirectRate] = useState("500");
  const [fixedAmount, setFixedAmount] = useState("1000000000000");
  const [firstOrderAmount, setFirstOrderAmount] = useState("1000000000000");
  const [firstOrderRate, setFirstOrderRate] = useState("500");
  const [firstOrderUseAmount, setFirstOrderUseAmount] = useState(true);
  const [repeatRate, setRepeatRate] = useState("200");
  const [repeatMinOrders, setRepeatMinOrders] = useState("3");

  const [levelDiffRates, setLevelDiffRates] = useState("500,300,100");
  const [levelDiffMaxDepth, setLevelDiffMaxDepth] = useState("3");

  const [singleUplineRate, setSingleUplineRate] = useState("300");
  const [singleDownlineRate, setSingleDownlineRate] = useState("200");
  const [singleBaseUpline, setSingleBaseUpline] = useState("3");
  const [singleBaseDownline, setSingleBaseDownline] = useState("3");
  const [singleThreshold, setSingleThreshold] = useState("1000000000000");
  const [singleMaxUpline, setSingleMaxUpline] = useState("10");
  const [singleMaxDownline, setSingleMaxDownline] = useState("10");

  const [nexCooldown, setNexCooldown] = useState("100");
  const [tokenCooldown, setTokenCooldown] = useState("100");

  useEffect(() => {
    if (config) {
      setModes(config.enabledModes);
      setMaxRate(String(config.maxCommissionRate));
      setCreatorRate(String(config.creatorRewardRate));
      setNexCooldown(String(config.withdrawalCooldown));
      setTokenCooldown(String(config.tokenWithdrawalCooldown));
    }
  }, [config]);

  useEffect(() => {
    if (referralConfig) {
      if (referralConfig.directRewardRate) setDirectRate(String(referralConfig.directRewardRate));
      if (referralConfig.repeatPurchaseRate) setRepeatRate(String(referralConfig.repeatPurchaseRate));
      if (referralConfig.repeatPurchaseMinOrders) setRepeatMinOrders(String(referralConfig.repeatPurchaseMinOrders));
    }
  }, [referralConfig]);

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const toggleMode = (bit: number) => setModes((prev) => prev ^ bit);
  const isModeEnabled = (bit: number) => !!(modes & bit);

  const handleSaveModes = async () => {
    await actions.setCommissionModes(currentEntityId, modes);
    refetch();
  };

  const handleSaveGlobal = async () => {
    await actions.setCommissionRate(currentEntityId, Number(maxRate));
    refetch();
  };

  const handleSaveCreatorRate = async () => {
    await actions.setCreatorRewardRate(currentEntityId, Number(creatorRate));
    refetch();
  };

  const handleSaveCooldown = async () => {
    await actions.setWithdrawalCooldown(currentEntityId, Number(nexCooldown), Number(tokenCooldown));
    refetch();
  };

  const handleToggleEnabled = async () => {
    if (!config) return;
    await actions.enableCommission(currentEntityId, !config.enabled);
    refetch();
  };

  const handleSaveDirect = async () => {
    await actions.setDirectRewardConfig(currentEntityId, Number(directRate));
    refetchReferral();
  };

  const handleSaveFixed = async () => {
    await actions.setFixedAmountConfig(currentEntityId, BigInt(fixedAmount));
    refetchReferral();
  };

  const handleSaveFirstOrder = async () => {
    await actions.setFirstOrderConfig(currentEntityId, BigInt(firstOrderAmount), Number(firstOrderRate), firstOrderUseAmount);
    refetchReferral();
  };

  const handleSaveRepeat = async () => {
    await actions.setRepeatPurchaseConfig(currentEntityId, Number(repeatRate), Number(repeatMinOrders));
    refetchReferral();
  };

  const handleSaveLevelDiff = async () => {
    const rates = levelDiffRates.split(",").map((s) => Number(s.trim()));
    await actions.setLevelDiffConfig(currentEntityId, rates, Number(levelDiffMaxDepth));
    refetchLevelDiff();
  };

  const handleSaveSingleLine = async () => {
    await actions.setSingleLineConfig(
      currentEntityId, Number(singleUplineRate), Number(singleDownlineRate),
      Number(singleBaseUpline), Number(singleBaseDownline),
      BigInt(singleThreshold), Number(singleMaxUpline), Number(singleMaxDownline),
    );
    refetchSingleLine();
  };

  const refreshAll = () => {
    refetch(); refetchReferral(); refetchMulti(); refetchLevelDiff(); refetchSingleLine(); refetchTeam(); refetchPool();
  };

  const allModeBits = Object.entries(COMMISSION_MODE_BITS).filter(([k]) => k !== "NONE").map(([, v]) => v);
  const enabledCount = allModeBits.filter((b) => isModeEnabled(b)).length;

  const renderModeConfig = (bit: number) => {
    switch (bit) {
      case COMMISSION_MODE_BITS.DIRECT_REWARD:
        return (
          <div className="space-y-3 pt-3 border-t">
            <div className="grid gap-3 md:grid-cols-2">
              <div className="space-y-1">
                <label className="text-xs font-medium">Direct Reward Rate (bps)</label>
                <Input type="number" value={directRate} onChange={(e) => setDirectRate(e.target.value)} min="0" max="10000" />
                <p className="text-xs text-muted-foreground">{basisPointsToPercent(Number(directRate))} per referral</p>
              </div>
            </div>
            <TxButton size="sm" onClick={handleSaveDirect} txStatus={actions.txState.status}>Save Direct Reward</TxButton>
            {referralConfig?.directRewardRate !== undefined && (
              <p className="text-xs text-muted-foreground">Current: {basisPointsToPercent(referralConfig.directRewardRate)}</p>
            )}
          </div>
        );

      case COMMISSION_MODE_BITS.FIXED_AMOUNT:
        return (
          <div className="space-y-3 pt-3 border-t">
            <div className="space-y-1">
              <label className="text-xs font-medium">Fixed Amount (smallest unit)</label>
              <Input value={fixedAmount} onChange={(e) => setFixedAmount(e.target.value)} />
            </div>
            <TxButton size="sm" onClick={handleSaveFixed} txStatus={actions.txState.status}>Save Fixed Amount</TxButton>
          </div>
        );

      case COMMISSION_MODE_BITS.FIRST_ORDER:
        return (
          <div className="space-y-3 pt-3 border-t">
            <div className="grid gap-3 md:grid-cols-3">
              <div className="space-y-1">
                <label className="text-xs font-medium">Amount</label>
                <Input value={firstOrderAmount} onChange={(e) => setFirstOrderAmount(e.target.value)} />
              </div>
              <div className="space-y-1">
                <label className="text-xs font-medium">Rate (bps)</label>
                <Input type="number" value={firstOrderRate} onChange={(e) => setFirstOrderRate(e.target.value)} />
              </div>
              <div className="flex items-center gap-2 pt-5">
                <Switch checked={firstOrderUseAmount} onCheckedChange={setFirstOrderUseAmount} />
                <label className="text-xs">Use Amount</label>
              </div>
            </div>
            <TxButton size="sm" onClick={handleSaveFirstOrder} txStatus={actions.txState.status}>Save First Order</TxButton>
          </div>
        );

      case COMMISSION_MODE_BITS.REPEAT_PURCHASE:
        return (
          <div className="space-y-3 pt-3 border-t">
            <div className="grid gap-3 md:grid-cols-2">
              <div className="space-y-1">
                <label className="text-xs font-medium">Rate (bps)</label>
                <Input type="number" value={repeatRate} onChange={(e) => setRepeatRate(e.target.value)} />
              </div>
              <div className="space-y-1">
                <label className="text-xs font-medium">Min Orders</label>
                <Input type="number" value={repeatMinOrders} onChange={(e) => setRepeatMinOrders(e.target.value)} min="1" />
              </div>
            </div>
            <TxButton size="sm" onClick={handleSaveRepeat} txStatus={actions.txState.status}>Save Repeat Purchase</TxButton>
          </div>
        );

      case COMMISSION_MODE_BITS.LEVEL_DIFF:
        return (
          <div className="space-y-3 pt-3 border-t">
            <div className="grid gap-3 md:grid-cols-2">
              <div className="space-y-1">
                <label className="text-xs font-medium">Level Rates (bps, comma-separated)</label>
                <Input value={levelDiffRates} onChange={(e) => setLevelDiffRates(e.target.value)} placeholder="500,300,100" />
              </div>
              <div className="space-y-1">
                <label className="text-xs font-medium">Max Depth</label>
                <Input type="number" value={levelDiffMaxDepth} onChange={(e) => setLevelDiffMaxDepth(e.target.value)} min="1" max="20" />
              </div>
            </div>
            <TxButton size="sm" onClick={handleSaveLevelDiff} txStatus={actions.txState.status}>Save Level Diff</TxButton>
            {levelDiffConfig && (
              <p className="text-xs text-muted-foreground">Current: [{levelDiffConfig.levelRates.map((r) => basisPointsToPercent(r)).join(", ")}], depth={levelDiffConfig.maxDepth}</p>
            )}
          </div>
        );

      case COMMISSION_MODE_BITS.SINGLE_LINE_UPLINE:
      case COMMISSION_MODE_BITS.SINGLE_LINE_DOWNLINE:
        return (
          <div className="space-y-3 pt-3 border-t">
            <div className="grid gap-3 md:grid-cols-4">
              <div className="space-y-1">
                <label className="text-xs font-medium">Upline Rate (bps)</label>
                <Input type="number" value={singleUplineRate} onChange={(e) => setSingleUplineRate(e.target.value)} />
              </div>
              <div className="space-y-1">
                <label className="text-xs font-medium">Downline Rate (bps)</label>
                <Input type="number" value={singleDownlineRate} onChange={(e) => setSingleDownlineRate(e.target.value)} />
              </div>
              <div className="space-y-1">
                <label className="text-xs font-medium">Base Upline Levels</label>
                <Input type="number" value={singleBaseUpline} onChange={(e) => setSingleBaseUpline(e.target.value)} />
              </div>
              <div className="space-y-1">
                <label className="text-xs font-medium">Base Downline Levels</label>
                <Input type="number" value={singleBaseDownline} onChange={(e) => setSingleBaseDownline(e.target.value)} />
              </div>
            </div>
            <div className="grid gap-3 md:grid-cols-3">
              <div className="space-y-1">
                <label className="text-xs font-medium">Level Increment Threshold</label>
                <Input value={singleThreshold} onChange={(e) => setSingleThreshold(e.target.value)} />
              </div>
              <div className="space-y-1">
                <label className="text-xs font-medium">Max Upline Levels</label>
                <Input type="number" value={singleMaxUpline} onChange={(e) => setSingleMaxUpline(e.target.value)} />
              </div>
              <div className="space-y-1">
                <label className="text-xs font-medium">Max Downline Levels</label>
                <Input type="number" value={singleMaxDownline} onChange={(e) => setSingleMaxDownline(e.target.value)} />
              </div>
            </div>
            <TxButton size="sm" onClick={handleSaveSingleLine} txStatus={actions.txState.status}>Save Single Line Config</TxButton>
            {singleLineConfig && (
              <p className="text-xs text-muted-foreground">
                Current: up={basisPointsToPercent(singleLineConfig.uplineRate)}, down={basisPointsToPercent(singleLineConfig.downlineRate)}, base={singleLineConfig.baseUplineLevels}/{singleLineConfig.baseDownlineLevels}
              </p>
            )}
          </div>
        );

      case COMMISSION_MODE_BITS.MULTI_LEVEL:
        return (
          <div className="space-y-2 pt-3 border-t">
            {multiLevelConfig ? (
              <div className="space-y-2">
                <p className="text-xs text-muted-foreground">Max Total Rate: {basisPointsToPercent(multiLevelConfig.maxTotalRate)}</p>
                <p className="text-xs text-muted-foreground">Tiers: {multiLevelConfig.tiers?.length || 0}</p>
                <p className="text-xs text-muted-foreground">Status: {multiLevelConfig.paused ? "Paused" : "Active"}</p>
              </div>
            ) : (
              <p className="text-xs text-muted-foreground">No multi-level config set. Use chain calls to configure tiers.</p>
            )}
          </div>
        );

      case COMMISSION_MODE_BITS.TEAM_PERFORMANCE:
        return (
          <div className="space-y-2 pt-3 border-t">
            {teamConfig ? (
              <div className="space-y-2">
                <p className="text-xs text-muted-foreground">Tiers: {teamConfig.tiers?.length || 0}</p>
                <p className="text-xs text-muted-foreground">Max Depth: {teamConfig.maxDepth}</p>
                <p className="text-xs text-muted-foreground">Stacking: {teamConfig.allowStacking ? "Yes" : "No"}</p>
                <p className="text-xs text-muted-foreground">Threshold Mode: {teamConfig.thresholdMode}</p>
              </div>
            ) : (
              <p className="text-xs text-muted-foreground">No team performance config set.</p>
            )}
          </div>
        );

      case COMMISSION_MODE_BITS.POOL_REWARD:
        return (
          <div className="space-y-2 pt-3 border-t">
            {poolConfig ? (
              <div className="space-y-2">
                <p className="text-xs text-muted-foreground">Level Ratios: {poolConfig.levelRatios?.length || 0} levels configured</p>
                <p className="text-xs text-muted-foreground">Round Duration: {poolConfig.roundDuration} blocks</p>
              </div>
            ) : (
              <p className="text-xs text-muted-foreground">No pool reward config set.</p>
            )}
          </div>
        );

      case COMMISSION_MODE_BITS.CREATOR_REWARD:
        return (
          <div className="space-y-3 pt-3 border-t">
            <div className="space-y-1">
              <label className="text-xs font-medium">Creator Reward Rate (bps)</label>
              <Input type="number" value={creatorRate} onChange={(e) => setCreatorRate(e.target.value)} min="0" max="10000" />
              <p className="text-xs text-muted-foreground">{basisPointsToPercent(Number(creatorRate))} of each order</p>
            </div>
            <TxButton size="sm" onClick={handleSaveCreatorRate} txStatus={actions.txState.status}>Save Creator Rate</TxButton>
          </div>
        );

      default:
        return null;
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/commission"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">Commission Configuration</h1>
          <p className="text-muted-foreground">Configure commission modes and per-mode settings</p>
        </div>
        <Button variant="outline" size="sm" onClick={refreshAll}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
      ) : (
        <>
          <Card>
            <CardHeader>
              <div className="flex items-center justify-between">
                <div>
                  <CardTitle className="flex items-center gap-2"><Shield className="h-5 w-5" />Global Settings</CardTitle>
                  <CardDescription>Master commission switch and global parameters</CardDescription>
                </div>
                {config && (
                  <Button
                    variant={config.enabled ? "default" : "outline"}
                    size="sm"
                    onClick={handleToggleEnabled}
                  >
                    <Power className="mr-1 h-3 w-3" />{config.enabled ? "Enabled" : "Disabled"}
                  </Button>
                )}
              </div>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
                <div className="space-y-1">
                  <label className="text-xs font-medium">Max Commission Rate (bps)</label>
                  <Input type="number" value={maxRate} onChange={(e) => setMaxRate(e.target.value)} min="0" max="10000" />
                  <p className="text-xs text-muted-foreground">{basisPointsToPercent(Number(maxRate))}</p>
                </div>
                <div className="space-y-1">
                  <label className="text-xs font-medium">NEX Withdrawal Cooldown (blocks)</label>
                  <Input type="number" value={nexCooldown} onChange={(e) => setNexCooldown(e.target.value)} min="0" />
                </div>
                <div className="space-y-1">
                  <label className="text-xs font-medium">Token Withdrawal Cooldown (blocks)</label>
                  <Input type="number" value={tokenCooldown} onChange={(e) => setTokenCooldown(e.target.value)} min="0" />
                </div>
                <div className="flex flex-col justify-end gap-2">
                  <TxButton size="sm" onClick={handleSaveGlobal} txStatus={actions.txState.status}>Save Rate</TxButton>
                  <TxButton size="sm" variant="outline" onClick={handleSaveCooldown} txStatus={actions.txState.status}>Save Cooldowns</TxButton>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Settings className="h-5 w-5" />Commission Modes
                <Badge variant="secondary" className="ml-2">{enabledCount} / {allModeBits.length} enabled</Badge>
              </CardTitle>
              <CardDescription>
                Toggle individual commission calculation modules. Modes are stored as a bitmask (current: 0x{modes.toString(16)}).
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              {allModeBits.map((bit) => {
                const meta = COMMISSION_MODE_LABELS[bit];
                if (!meta) return null;
                const Icon = MODE_ICONS[bit] || Settings;
                const enabled = isModeEnabled(bit);
                const isExpanded = expandedMode === bit;

                return (
                  <div key={bit} className={`rounded-lg border transition-all ${enabled ? "border-primary/50 bg-primary/5" : ""}`}>
                    <div className="flex items-center justify-between p-4">
                      <div className="flex items-center gap-3" onClick={() => setExpandedMode(isExpanded ? null : bit)} role="button">
                        <div className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg ${enabled ? "bg-primary/20" : "bg-muted"}`}>
                          <Icon className={`h-4 w-4 ${enabled ? "text-primary" : "text-muted-foreground"}`} />
                        </div>
                        <div>
                          <p className="text-sm font-medium">{meta.name}</p>
                          <p className="text-xs text-muted-foreground">{meta.desc}</p>
                        </div>
                        {enabled && (
                          <Button variant="ghost" size="icon" className="h-6 w-6">
                            {isExpanded ? <ChevronUp className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />}
                          </Button>
                        )}
                      </div>
                      <Switch checked={enabled} onCheckedChange={() => toggleMode(bit)} />
                    </div>
                    {enabled && isExpanded && (
                      <div className="px-4 pb-4">
                        {renderModeConfig(bit)}
                      </div>
                    )}
                  </div>
                );
              })}

              <Separator />

              <div className="flex items-center gap-4">
                <div className="rounded-lg border bg-muted/50 p-3 flex-1">
                  <p className="text-xs text-muted-foreground">
                    Modes bitmask: <span className="font-mono">0x{modes.toString(16).padStart(4, "0")}</span> ({modes})
                    {config && modes !== config.enabledModes && <span className="ml-2 text-amber-600">(unsaved)</span>}
                  </p>
                </div>
                <TxButton onClick={handleSaveModes} txStatus={actions.txState.status}>
                  Save Commission Modes
                </TxButton>
              </div>
            </CardContent>
          </Card>
        </>
      )}

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Configuration saved!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
