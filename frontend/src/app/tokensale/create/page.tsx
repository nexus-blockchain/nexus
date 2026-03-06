"use client";

import { useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEntityStore } from "@/stores/entity";
import { useTokensaleActions } from "@/hooks/useTokensale";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { Select } from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { TxButton } from "@/components/shared/TxButton";
import { cn, basisPointsToPercent } from "@/lib/utils";
import { SALE_MODES, VESTING_TYPES } from "@/lib/constants";
import { useTranslations } from "next-intl";
import {
  ArrowLeft, ArrowRight, Check, Rocket, Target,
  TrendingUp, Users, CircleDot, Zap, Settings,
  Coins, Shield, Plus, Trash2, UserPlus,
} from "lucide-react";

const MODE_META: Record<
  string,
  { icon: typeof Rocket; label: string; description: string }
> = {
  FixedPrice: {
    icon: Target,
    label: "Fixed Price",
    description: "Tokens sold at a fixed price per unit",
  },
  DutchAuction: {
    icon: TrendingUp,
    label: "Dutch Auction",
    description: "Price decreases over time until all tokens sold",
  },
  WhitelistAllocation: {
    icon: Users,
    label: "Whitelist Allocation",
    description: "Only whitelisted addresses can participate",
  },
  FCFS: {
    icon: Zap,
    label: "First Come First Served",
    description: "Tokens sold on a first-come, first-served basis",
  },
  Lottery: {
    icon: CircleDot,
    label: "Lottery",
    description: "Random selection of participants for token allocation",
  },
};

const VESTING_META: Record<string, { label: string; description: string }> = {
  None: { label: "None", description: "All tokens unlocked immediately" },
  Linear: { label: "Linear", description: "Tokens unlock linearly over time" },
  Cliff: { label: "Cliff", description: "All tokens unlock after cliff period" },
  Custom: { label: "Custom", description: "Custom unlock schedule with intervals" },
};

interface WhitelistEntry {
  address: string;
  allocation: string;
}

export default function CreateTokenSalePage() {
  const { currentEntityId } = useEntityStore();
  const actions = useTokensaleActions();
  const router = useRouter();
  const tc = useTranslations("common");

  const [currentStep, setCurrentStep] = useState(0);
  const [createdRoundId, setCreatedRoundId] = useState<number | null>(null);

  // Step 1: Basic
  const [mode, setMode] = useState<string>("FixedPrice");
  const [totalSupply, setTotalSupply] = useState("");
  const [startBlock, setStartBlock] = useState("");
  const [endBlock, setEndBlock] = useState("");
  const [softCap, setSoftCap] = useState("");
  const [kycRequired, setKycRequired] = useState(false);
  const [minKycLevel, setMinKycLevel] = useState("0");

  // Step 2: Payment options
  const [paymentOptions, setPaymentOptions] = useState<
    Array<{ assetId: string; price: string; minPurchase: string; maxPerAccount: string }>
  >([{ assetId: "", price: "", minPurchase: "", maxPerAccount: "" }]);

  // Step 3: Vesting
  const [vestingType, setVestingType] = useState<string>("None");
  const [initialUnlockBps, setInitialUnlockBps] = useState("0");
  const [cliffDuration, setCliffDuration] = useState("");
  const [totalDuration, setTotalDuration] = useState("");
  const [unlockInterval, setUnlockInterval] = useState("");

  // Step 4: Dutch Auction
  const [dutchStartPrice, setDutchStartPrice] = useState("");
  const [dutchEndPrice, setDutchEndPrice] = useState("");

  // Step 5: Whitelist
  const [whitelistEntries, setWhitelistEntries] = useState<WhitelistEntry[]>([
    { address: "", allocation: "" },
  ]);

  if (!currentEntityId) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        {tc("selectEntity")}
      </div>
    );
  }

  const steps = [
    { label: "Basic", required: true },
    { label: "Payment", required: true },
    { label: "Vesting", required: false },
    ...(mode === "DutchAuction" ? [{ label: "Dutch Auction", required: true }] : []),
    ...(mode === "WhitelistAllocation"
      ? [{ label: "Whitelist", required: true }]
      : []),
  ];

  const handleCreateRound = async () => {
    if (!totalSupply) return;
    await actions.createSaleRound(
      currentEntityId,
      mode,
      BigInt(totalSupply),
      Number(startBlock || 0),
      Number(endBlock || 0),
      kycRequired,
      Number(minKycLevel),
      BigInt(softCap || 0)
    );
  };

  const handleAddPaymentOption = async (opt: (typeof paymentOptions)[0]) => {
    if (createdRoundId === null || !opt.price) return;
    const assetId = opt.assetId === "" ? null : Number(opt.assetId);
    await actions.addPaymentOption(
      createdRoundId,
      assetId,
      BigInt(opt.price),
      BigInt(opt.minPurchase || 0),
      BigInt(opt.maxPerAccount || 0)
    );
  };

  const handleSetVesting = async () => {
    if (createdRoundId === null || vestingType === "None") return;
    await actions.setVestingConfig(
      createdRoundId,
      vestingType,
      Number(initialUnlockBps),
      Number(cliffDuration || 0),
      Number(totalDuration || 0),
      Number(unlockInterval || 0)
    );
  };

  const handleConfigureDutchAuction = async () => {
    if (createdRoundId === null || !dutchStartPrice || !dutchEndPrice) return;
    await actions.configureDutchAuction(
      createdRoundId,
      BigInt(dutchStartPrice),
      BigInt(dutchEndPrice)
    );
  };

  const handleAddWhitelist = async () => {
    if (createdRoundId === null) return;
    const accounts: Array<[string, bigint | null]> = whitelistEntries
      .filter((e) => e.address.trim())
      .map((e) => [e.address.trim(), e.allocation ? BigInt(e.allocation) : null]);
    if (accounts.length === 0) return;
    await actions.addToWhitelist(createdRoundId, accounts);
  };

  const addPaymentRow = () => {
    setPaymentOptions((prev) => [
      ...prev,
      { assetId: "", price: "", minPurchase: "", maxPerAccount: "" },
    ]);
  };

  const removePaymentRow = (index: number) => {
    setPaymentOptions((prev) => prev.filter((_, i) => i !== index));
  };

  const updatePaymentRow = (
    index: number,
    field: keyof (typeof paymentOptions)[0],
    value: string
  ) => {
    setPaymentOptions((prev) =>
      prev.map((row, i) => (i === index ? { ...row, [field]: value } : row))
    );
  };

  const addWhitelistRow = () => {
    setWhitelistEntries((prev) => [...prev, { address: "", allocation: "" }]);
  };

  const removeWhitelistRow = (index: number) => {
    setWhitelistEntries((prev) => prev.filter((_, i) => i !== index));
  };

  const updateWhitelistRow = (
    index: number,
    field: keyof WhitelistEntry,
    value: string
  ) => {
    setWhitelistEntries((prev) =>
      prev.map((row, i) => (i === index ? { ...row, [field]: value } : row))
    );
  };

  const isStep1Valid = !!totalSupply && !!mode;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/tokensale">
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Create Sale Round</h1>
          <p className="text-muted-foreground">
            Set up a new token sale round for your entity
          </p>
        </div>
      </div>

      {/* Step indicator */}
      <div className="flex items-center gap-2">
        {steps.map((step, i) => (
          <div key={i} className="flex items-center gap-2">
            {i > 0 && (
              <div
                className={cn(
                  "h-px w-8",
                  i <= currentStep ? "bg-primary" : "bg-border"
                )}
              />
            )}
            <button
              onClick={() => {
                if (i === 0 || createdRoundId !== null) setCurrentStep(i);
              }}
              disabled={i > 0 && createdRoundId === null}
              className={cn(
                "flex items-center gap-2 rounded-full px-3 py-1.5 text-xs font-medium transition-colors",
                i === currentStep
                  ? "bg-primary text-primary-foreground"
                  : i < currentStep
                    ? "bg-primary/20 text-primary"
                    : "bg-muted text-muted-foreground"
              )}
            >
              {i < currentStep ? (
                <Check className="h-3 w-3" />
              ) : (
                <span>{i + 1}</span>
              )}
              {step.label}
            </button>
          </div>
        ))}
      </div>

      {createdRoundId !== null && (
        <div className="rounded-lg border border-blue-200 bg-blue-50 px-4 py-3 dark:border-blue-800 dark:bg-blue-950">
          <p className="text-sm text-blue-800 dark:text-blue-200">
            Round <span className="font-bold">#{createdRoundId}</span> created.
            Continue configuring below or{" "}
            <Link
              href={`/tokensale/${createdRoundId}`}
              className="underline hover:no-underline"
            >
              view round details
            </Link>
            .
          </p>
        </div>
      )}

      {/* Step 1: Basic */}
      {currentStep === 0 && (
        <div className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Rocket className="h-5 w-5" />
                Sale Mode
              </CardTitle>
              <CardDescription>Choose how tokens will be sold</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
                {SALE_MODES.map((m) => {
                  const meta = MODE_META[m];
                  const Icon = meta?.icon || Rocket;
                  return (
                    <button
                      key={m}
                      onClick={() => setMode(m)}
                      className={cn(
                        "flex flex-col items-start gap-2 rounded-lg border-2 p-4 text-left transition-all hover:bg-accent",
                        mode === m
                          ? "border-primary bg-primary/5"
                          : "border-border"
                      )}
                    >
                      <div className="flex items-center gap-2">
                        <div
                          className={cn(
                            "flex h-8 w-8 items-center justify-center rounded-lg",
                            mode === m
                              ? "bg-primary text-primary-foreground"
                              : "bg-muted"
                          )}
                        >
                          <Icon className="h-4 w-4" />
                        </div>
                        <span className="font-medium">{meta?.label || m}</span>
                      </div>
                      <p className="text-xs text-muted-foreground">
                        {meta?.description || m}
                      </p>
                    </button>
                  );
                })}
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Coins className="h-5 w-5" />
                Sale Parameters
              </CardTitle>
              <CardDescription>Configure the basic sale settings</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Total Supply *</label>
                <Input
                  type="number"
                  value={totalSupply}
                  onChange={(e) => setTotalSupply(e.target.value)}
                  placeholder="Total tokens available for sale"
                  min="0"
                />
              </div>

              <div className="grid gap-4 grid-cols-2">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Start Block</label>
                  <Input
                    type="number"
                    value={startBlock}
                    onChange={(e) => setStartBlock(e.target.value)}
                    placeholder="0 = immediate"
                    min="0"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">End Block</label>
                  <Input
                    type="number"
                    value={endBlock}
                    onChange={(e) => setEndBlock(e.target.value)}
                    placeholder="0 = no end"
                    min="0"
                  />
                </div>
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">Soft Cap</label>
                <Input
                  type="number"
                  value={softCap}
                  onChange={(e) => setSoftCap(e.target.value)}
                  placeholder="0 = no soft cap"
                  min="0"
                />
              </div>

              <Separator />

              <div className="flex items-center justify-between rounded-lg border p-4">
                <div className="flex items-center gap-3">
                  <Shield className="h-5 w-5 text-muted-foreground" />
                  <div>
                    <p className="text-sm font-medium">KYC Required</p>
                    <p className="text-xs text-muted-foreground">
                      Participants must pass KYC verification
                    </p>
                  </div>
                </div>
                <Switch checked={kycRequired} onCheckedChange={setKycRequired} />
              </div>

              {kycRequired && (
                <div className="space-y-2">
                  <label className="text-sm font-medium">Minimum KYC Level</label>
                  <Select
                    value={minKycLevel}
                    onChange={(e) => setMinKycLevel(e.target.value)}
                  >
                    <option value="0">None (0)</option>
                    <option value="1">Basic (1)</option>
                    <option value="2">Standard (2)</option>
                    <option value="3">Enhanced (3)</option>
                    <option value="4">Full (4)</option>
                  </Select>
                </div>
              )}
            </CardContent>
          </Card>

          <div className="flex items-center gap-4">
            <TxButton
              onClick={handleCreateRound}
              txStatus={actions.txState.status}
              disabled={!isStep1Valid || createdRoundId !== null}
            >
              <Rocket className="mr-2 h-4 w-4" />
              Create Sale Round
            </TxButton>
            {createdRoundId !== null && (
              <Button onClick={() => setCurrentStep(1)}>
                Next: Payment Options
                <ArrowRight className="ml-2 h-4 w-4" />
              </Button>
            )}
            <Button variant="outline" asChild>
              <Link href="/tokensale">{tc("cancel")}</Link>
            </Button>
          </div>
        </div>
      )}

      {/* Step 2: Payment Options */}
      {currentStep === 1 && (
        <div className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Coins className="h-5 w-5" />
                Payment Options
              </CardTitle>
              <CardDescription>
                Configure accepted payment methods. Leave Asset ID empty for NEX
                (native token).
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {paymentOptions.map((opt, i) => (
                <div key={i} className="space-y-3 rounded-lg border p-4">
                  <div className="flex items-center justify-between">
                    <Badge variant="outline">Option {i + 1}</Badge>
                    {paymentOptions.length > 1 && (
                      <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => removePaymentRow(i)}
                      >
                        <Trash2 className="h-4 w-4 text-destructive" />
                      </Button>
                    )}
                  </div>
                  <div className="grid gap-4 md:grid-cols-2">
                    <div className="space-y-2">
                      <label className="text-sm font-medium">Asset ID</label>
                      <Input
                        type="number"
                        value={opt.assetId}
                        onChange={(e) => updatePaymentRow(i, "assetId", e.target.value)}
                        placeholder="Empty = NEX (native)"
                        min="0"
                      />
                    </div>
                    <div className="space-y-2">
                      <label className="text-sm font-medium">Price per Token *</label>
                      <Input
                        type="number"
                        value={opt.price}
                        onChange={(e) => updatePaymentRow(i, "price", e.target.value)}
                        placeholder="Token price"
                        min="0"
                      />
                    </div>
                    <div className="space-y-2">
                      <label className="text-sm font-medium">Min Purchase</label>
                      <Input
                        type="number"
                        value={opt.minPurchase}
                        onChange={(e) =>
                          updatePaymentRow(i, "minPurchase", e.target.value)
                        }
                        placeholder="0 = no min"
                        min="0"
                      />
                    </div>
                    <div className="space-y-2">
                      <label className="text-sm font-medium">Max Per Account</label>
                      <Input
                        type="number"
                        value={opt.maxPerAccount}
                        onChange={(e) =>
                          updatePaymentRow(i, "maxPerAccount", e.target.value)
                        }
                        placeholder="0 = no max"
                        min="0"
                      />
                    </div>
                  </div>
                  <TxButton
                    size="sm"
                    variant="outline"
                    onClick={() => handleAddPaymentOption(opt)}
                    txStatus={actions.txState.status}
                    disabled={!opt.price || createdRoundId === null}
                  >
                    <Plus className="mr-2 h-3.5 w-3.5" />
                    Add This Option
                  </TxButton>
                </div>
              ))}

              <Button variant="outline" onClick={addPaymentRow}>
                <Plus className="mr-2 h-4 w-4" />
                Add Another Option
              </Button>
            </CardContent>
          </Card>

          <div className="flex items-center gap-4">
            <Button variant="outline" onClick={() => setCurrentStep(0)}>
              <ArrowLeft className="mr-2 h-4 w-4" />
              Back
            </Button>
            <Button onClick={() => setCurrentStep(2)}>
              Next: Vesting
              <ArrowRight className="ml-2 h-4 w-4" />
            </Button>
          </div>
        </div>
      )}

      {/* Step 3: Vesting */}
      {currentStep === 2 && (
        <div className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Settings className="h-5 w-5" />
                Vesting Configuration
              </CardTitle>
              <CardDescription>
                Optional. Configure how purchased tokens unlock over time.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid gap-3 md:grid-cols-2">
                {VESTING_TYPES.map((vt) => {
                  const meta = VESTING_META[vt];
                  return (
                    <button
                      key={vt}
                      onClick={() => setVestingType(vt)}
                      className={cn(
                        "flex flex-col items-start gap-1 rounded-lg border-2 p-4 text-left transition-all hover:bg-accent",
                        vestingType === vt
                          ? "border-primary bg-primary/5"
                          : "border-border"
                      )}
                    >
                      <span className="font-medium">{meta?.label || vt}</span>
                      <p className="text-xs text-muted-foreground">
                        {meta?.description || vt}
                      </p>
                    </button>
                  );
                })}
              </div>

              {vestingType !== "None" && (
                <>
                  <Separator />
                  <div className="grid gap-4 md:grid-cols-2">
                    <div className="space-y-2">
                      <label className="text-sm font-medium">
                        Initial Unlock (basis points)
                      </label>
                      <Input
                        type="number"
                        value={initialUnlockBps}
                        onChange={(e) => setInitialUnlockBps(e.target.value)}
                        placeholder="0 - 10000"
                        min="0"
                        max="10000"
                      />
                      <p className="text-xs text-muted-foreground">
                        {basisPointsToPercent(Number(initialUnlockBps))} unlocked
                        immediately
                      </p>
                    </div>
                    <div className="space-y-2">
                      <label className="text-sm font-medium">
                        Cliff Duration (blocks)
                      </label>
                      <Input
                        type="number"
                        value={cliffDuration}
                        onChange={(e) => setCliffDuration(e.target.value)}
                        placeholder="Blocks before vesting starts"
                        min="0"
                      />
                      <p className="text-xs text-muted-foreground">
                        ~{Math.round((Number(cliffDuration || 0) * 6) / 3600)} hours
                      </p>
                    </div>
                    <div className="space-y-2">
                      <label className="text-sm font-medium">
                        Total Duration (blocks)
                      </label>
                      <Input
                        type="number"
                        value={totalDuration}
                        onChange={(e) => setTotalDuration(e.target.value)}
                        placeholder="Total vesting period"
                        min="0"
                      />
                      <p className="text-xs text-muted-foreground">
                        ~{Math.round((Number(totalDuration || 0) * 6) / 3600)} hours
                      </p>
                    </div>
                    <div className="space-y-2">
                      <label className="text-sm font-medium">
                        Unlock Interval (blocks)
                      </label>
                      <Input
                        type="number"
                        value={unlockInterval}
                        onChange={(e) => setUnlockInterval(e.target.value)}
                        placeholder="How often tokens unlock"
                        min="0"
                      />
                      <p className="text-xs text-muted-foreground">
                        ~{Math.round((Number(unlockInterval || 0) * 6) / 3600)} hours
                      </p>
                    </div>
                  </div>

                  <TxButton
                    onClick={handleSetVesting}
                    txStatus={actions.txState.status}
                    disabled={createdRoundId === null}
                  >
                    <Settings className="mr-2 h-4 w-4" />
                    Set Vesting Config
                  </TxButton>
                </>
              )}
            </CardContent>
          </Card>

          <div className="flex items-center gap-4">
            <Button variant="outline" onClick={() => setCurrentStep(1)}>
              <ArrowLeft className="mr-2 h-4 w-4" />
              Back
            </Button>
            {mode === "DutchAuction" ? (
              <Button onClick={() => setCurrentStep(3)}>
                Next: Dutch Auction
                <ArrowRight className="ml-2 h-4 w-4" />
              </Button>
            ) : mode === "WhitelistAllocation" ? (
              <Button onClick={() => setCurrentStep(3)}>
                Next: Whitelist
                <ArrowRight className="ml-2 h-4 w-4" />
              </Button>
            ) : (
              <Button
                onClick={() => router.push(`/tokensale/${createdRoundId}`)}
                disabled={createdRoundId === null}
              >
                <Check className="mr-2 h-4 w-4" />
                Finish
              </Button>
            )}
          </div>
        </div>
      )}

      {/* Step 4: Dutch Auction (conditional) */}
      {currentStep === 3 && mode === "DutchAuction" && (
        <div className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <TrendingUp className="h-5 w-5" />
                Dutch Auction Configuration
              </CardTitle>
              <CardDescription>
                Set the starting and ending prices for the dutch auction. The price
                will linearly decrease from start to end over the sale duration.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Start Price *</label>
                  <Input
                    type="number"
                    value={dutchStartPrice}
                    onChange={(e) => setDutchStartPrice(e.target.value)}
                    placeholder="Highest price at sale start"
                    min="0"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">End Price *</label>
                  <Input
                    type="number"
                    value={dutchEndPrice}
                    onChange={(e) => setDutchEndPrice(e.target.value)}
                    placeholder="Lowest price at sale end"
                    min="0"
                  />
                </div>
              </div>

              {dutchStartPrice && dutchEndPrice && (
                <div className="rounded-lg bg-muted p-4">
                  <p className="text-sm text-muted-foreground">
                    Price will decrease from{" "}
                    <span className="font-mono font-medium text-foreground">
                      {Number(dutchStartPrice).toLocaleString()}
                    </span>{" "}
                    to{" "}
                    <span className="font-mono font-medium text-foreground">
                      {Number(dutchEndPrice).toLocaleString()}
                    </span>{" "}
                    over the sale duration.
                  </p>
                </div>
              )}

              <TxButton
                onClick={handleConfigureDutchAuction}
                txStatus={actions.txState.status}
                disabled={
                  !dutchStartPrice || !dutchEndPrice || createdRoundId === null
                }
              >
                <TrendingUp className="mr-2 h-4 w-4" />
                Configure Dutch Auction
              </TxButton>
            </CardContent>
          </Card>

          <div className="flex items-center gap-4">
            <Button variant="outline" onClick={() => setCurrentStep(2)}>
              <ArrowLeft className="mr-2 h-4 w-4" />
              Back
            </Button>
            <Button
              onClick={() => router.push(`/tokensale/${createdRoundId}`)}
              disabled={createdRoundId === null}
            >
              <Check className="mr-2 h-4 w-4" />
              Finish
            </Button>
          </div>
        </div>
      )}

      {/* Step 5: Whitelist (conditional) */}
      {((currentStep === 3 && mode === "WhitelistAllocation") ||
        (currentStep === 4 && mode === "WhitelistAllocation")) && (
        <div className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <UserPlus className="h-5 w-5" />
                Whitelist Management
              </CardTitle>
              <CardDescription>
                Add addresses that are allowed to participate. Leave allocation
                empty for unlimited.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {whitelistEntries.map((entry, i) => (
                <div
                  key={i}
                  className="flex items-end gap-3 rounded-lg border p-3"
                >
                  <div className="flex-1 space-y-2">
                    <label className="text-sm font-medium">Address</label>
                    <Input
                      value={entry.address}
                      onChange={(e) =>
                        updateWhitelistRow(i, "address", e.target.value)
                      }
                      placeholder="Account address"
                    />
                  </div>
                  <div className="w-48 space-y-2">
                    <label className="text-sm font-medium">Allocation</label>
                    <Input
                      type="number"
                      value={entry.allocation}
                      onChange={(e) =>
                        updateWhitelistRow(i, "allocation", e.target.value)
                      }
                      placeholder="Empty = unlimited"
                      min="0"
                    />
                  </div>
                  {whitelistEntries.length > 1 && (
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => removeWhitelistRow(i)}
                    >
                      <Trash2 className="h-4 w-4 text-destructive" />
                    </Button>
                  )}
                </div>
              ))}

              <Button variant="outline" onClick={addWhitelistRow}>
                <Plus className="mr-2 h-4 w-4" />
                Add Another Address
              </Button>

              <Separator />

              <TxButton
                onClick={handleAddWhitelist}
                txStatus={actions.txState.status}
                disabled={
                  createdRoundId === null ||
                  whitelistEntries.every((e) => !e.address.trim())
                }
              >
                <UserPlus className="mr-2 h-4 w-4" />
                Add to Whitelist
              </TxButton>
            </CardContent>
          </Card>

          <div className="flex items-center gap-4">
            <Button variant="outline" onClick={() => setCurrentStep(2)}>
              <ArrowLeft className="mr-2 h-4 w-4" />
              Back
            </Button>
            <Button
              onClick={() => router.push(`/tokensale/${createdRoundId}`)}
              disabled={createdRoundId === null}
            >
              <Check className="mr-2 h-4 w-4" />
              Finish
            </Button>
          </div>
        </div>
      )}

      {/* Tx feedback */}
      {actions.txState.status === "finalized" && (
        <div className="rounded-lg border border-green-200 bg-green-50 p-4 dark:border-green-800 dark:bg-green-950">
          <p className="text-sm text-green-800 dark:text-green-200">
            Transaction completed successfully!
          </p>
          {currentStep === 0 && createdRoundId === null && (
            <p className="mt-1 text-xs text-green-700 dark:text-green-300">
              Check the round ID in the transaction events, then proceed to the
              next step.
            </p>
          )}
          <Button
            variant="link"
            className="mt-1 h-auto p-0 text-green-700 dark:text-green-300"
            onClick={() => actions.resetTx()}
          >
            Dismiss
          </Button>
        </div>
      )}
      {actions.txState.status === "error" && (
        <p className="text-sm text-destructive">{actions.txState.error}</p>
      )}
    </div>
  );
}
