"use client";

import { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { TxButton } from "@/components/shared/TxButton";
import {
  useBillingParams,
  useUserFundingBalance,
  useStorageActions,
} from "@/hooks/useStorageService";
import { useWalletStore } from "@/stores/wallet";
import {
  ArrowLeft,
  CreditCard,
  Wallet,
  Settings,
  AlertTriangle,
  ChevronDown,
  ChevronUp,
  Users,
  Database,
  Droplets,
  Pause,
  Play,
  Zap,
  RotateCcw,
} from "lucide-react";
import Link from "next/link";

const formatBal = (b: bigint) => (Number(b) / 1e12).toFixed(4);

export default function StorageBillingPage() {
  const { address } = useWalletStore();
  const { balance, isLoading: balLoading, refetch: refetchBal } = useUserFundingBalance(address);
  const { params, isLoading: paramsLoading, refetch: refetchParams } = useBillingParams();
  const actions = useStorageActions();

  const [topUpAmount, setTopUpAmount] = useState("");
  const [adminOpen, setAdminOpen] = useState(false);

  const [cfgPrice, setCfgPrice] = useState("");
  const [cfgPeriod, setCfgPeriod] = useState("");
  const [cfgGrace, setCfgGrace] = useState("");
  const [cfgMaxCharge, setCfgMaxCharge] = useState("");
  const [cfgMinReserve, setCfgMinReserve] = useState("");

  const [chargeLimit, setChargeLimit] = useState("");
  const [distributeMax, setDistributeMax] = useState("");

  const [fundUserAddr, setFundUserAddr] = useState("");
  const [fundUserAmt, setFundUserAmt] = useState("");
  const [fundSubjectId, setFundSubjectId] = useState("");
  const [fundSubjectAmt, setFundSubjectAmt] = useState("");
  const [fundPoolAmt, setFundPoolAmt] = useState("");

  const handleTopUp = async () => {
    if (!address || !topUpAmount) return;
    await actions.fundUserAccount(address, BigInt(topUpAmount));
    setTopUpAmount("");
    refetchBal();
  };

  const handleSetBillingParams = async () => {
    await actions.setBillingParams(
      cfgPrice ? BigInt(cfgPrice) : null,
      cfgPeriod ? Number(cfgPeriod) : null,
      cfgGrace ? Number(cfgGrace) : null,
      cfgMaxCharge ? Number(cfgMaxCharge) : null,
      cfgMinReserve ? BigInt(cfgMinReserve) : null,
      null,
    );
    refetchParams();
  };

  const handleFundUser = async () => {
    if (!fundUserAddr || !fundUserAmt) return;
    await actions.fundUserAccount(fundUserAddr, BigInt(fundUserAmt));
    setFundUserAddr("");
    setFundUserAmt("");
  };

  const handleFundSubject = async () => {
    if (!fundSubjectId || !fundSubjectAmt) return;
    await actions.fundSubjectAccount(Number(fundSubjectId), BigInt(fundSubjectAmt));
    setFundSubjectId("");
    setFundSubjectAmt("");
  };

  const handleFundPool = async () => {
    if (!fundPoolAmt) return;
    await actions.fundIpfsPool(BigInt(fundPoolAmt));
    setFundPoolAmt("");
  };

  const isLoading = balLoading || paramsLoading;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/storage"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <CreditCard className="h-7 w-7" />
            Storage Billing
          </h1>
          <p className="text-muted-foreground">Manage balances, view billing parameters, and administer billing</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => { refetchBal(); refetchParams(); }}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {params?.paused && (
        <div className="flex items-center gap-3 rounded-lg border border-amber-500/50 bg-amber-50 dark:bg-amber-950/20 p-4">
          <AlertTriangle className="h-5 w-5 text-amber-600 shrink-0" />
          <div>
            <p className="font-medium text-amber-800 dark:text-amber-400">Billing is Paused</p>
            <p className="text-sm text-amber-700 dark:text-amber-500">
              No charges are being processed. Contact an admin to resume billing.
            </p>
          </div>
        </div>
      )}

      {isLoading ? (
        <div className="flex justify-center py-12">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
        </div>
      ) : (
        <>
          {/* Your Balance */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Wallet className="h-5 w-5" />Your Funding Balance
              </CardTitle>
              <CardDescription>Your on-chain storage funding balance used for pin billing</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-baseline gap-2">
                <span className="text-3xl font-bold">{formatBal(balance)}</span>
                <span className="text-muted-foreground">NEX</span>
              </div>
              <Separator />
              <div className="space-y-2">
                <label className="text-sm font-medium">Top Up Your Balance</label>
                <div className="flex gap-2">
                  <Input
                    type="number"
                    placeholder="Amount (smallest unit)"
                    value={topUpAmount}
                    onChange={(e) => setTopUpAmount(e.target.value)}
                    className="max-w-xs"
                  />
                  <TxButton
                    onClick={handleTopUp}
                    txStatus={actions.txState.status}
                    disabled={!address || !topUpAmount}
                    size="sm"
                  >
                    Top Up
                  </TxButton>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* Billing Parameters */}
          <Card>
            <CardHeader>
              <div className="flex items-center justify-between">
                <div>
                  <CardTitle className="flex items-center gap-2">
                    <Settings className="h-5 w-5" />Billing Parameters
                  </CardTitle>
                  <CardDescription>Current on-chain billing configuration</CardDescription>
                </div>
                {params && (
                  <Badge variant={params.paused ? "destructive" : "default"}>
                    {params.paused ? "Paused" : "Active"}
                  </Badge>
                )}
              </div>
            </CardHeader>
            <CardContent>
              {params ? (
                <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                  <div className="rounded-lg border p-4 space-y-1">
                    <p className="text-xs text-muted-foreground">Price per GiB/Week</p>
                    <p className="text-lg font-semibold font-mono">{formatBal(params.pricePerGibWeek)} NEX</p>
                  </div>
                  <div className="rounded-lg border p-4 space-y-1">
                    <p className="text-xs text-muted-foreground">Billing Period</p>
                    <p className="text-lg font-semibold font-mono">{params.periodBlocks.toLocaleString()} blocks</p>
                  </div>
                  <div className="rounded-lg border p-4 space-y-1">
                    <p className="text-xs text-muted-foreground">Grace Period</p>
                    <p className="text-lg font-semibold font-mono">{params.graceBlocks.toLocaleString()} blocks</p>
                  </div>
                  <div className="rounded-lg border p-4 space-y-1">
                    <p className="text-xs text-muted-foreground">Max Charge per Block</p>
                    <p className="text-lg font-semibold font-mono">{params.maxChargePerBlock.toLocaleString()}</p>
                  </div>
                  <div className="rounded-lg border p-4 space-y-1">
                    <p className="text-xs text-muted-foreground">Subject Min Reserve</p>
                    <p className="text-lg font-semibold font-mono">{formatBal(params.subjectMinReserve)} NEX</p>
                  </div>
                  <div className="rounded-lg border p-4 space-y-1">
                    <p className="text-xs text-muted-foreground">Billing Status</p>
                    <p className={`text-lg font-semibold ${params.paused ? "text-destructive" : "text-green-600"}`}>
                      {params.paused ? "Paused" : "Active"}
                    </p>
                  </div>
                </div>
              ) : (
                <p className="text-muted-foreground">No billing parameters configured.</p>
              )}
            </CardContent>
          </Card>

          {/* Admin Billing Config (collapsible) */}
          <Card>
            <CardHeader
              className="cursor-pointer select-none"
              onClick={() => setAdminOpen(!adminOpen)}
            >
              <div className="flex items-center justify-between">
                <div>
                  <CardTitle className="flex items-center gap-2">
                    <Zap className="h-5 w-5" />Admin Billing Config
                  </CardTitle>
                  <CardDescription>Update billing parameters and run admin operations</CardDescription>
                </div>
                {adminOpen ? <ChevronUp className="h-5 w-5" /> : <ChevronDown className="h-5 w-5" />}
              </div>
            </CardHeader>
            {adminOpen && (
              <CardContent className="space-y-6">
                {/* Update billing params */}
                <div className="space-y-3">
                  <h3 className="text-sm font-semibold">Update Billing Parameters</h3>
                  <p className="text-xs text-muted-foreground">Leave fields empty to keep current values.</p>
                  <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
                    <div className="space-y-1">
                      <label className="text-xs font-medium">Price per GiB/Week</label>
                      <Input placeholder="e.g. 1000000000000" value={cfgPrice} onChange={(e) => setCfgPrice(e.target.value)} />
                    </div>
                    <div className="space-y-1">
                      <label className="text-xs font-medium">Period (blocks)</label>
                      <Input type="number" placeholder="e.g. 50400" value={cfgPeriod} onChange={(e) => setCfgPeriod(e.target.value)} />
                    </div>
                    <div className="space-y-1">
                      <label className="text-xs font-medium">Grace (blocks)</label>
                      <Input type="number" placeholder="e.g. 7200" value={cfgGrace} onChange={(e) => setCfgGrace(e.target.value)} />
                    </div>
                    <div className="space-y-1">
                      <label className="text-xs font-medium">Max Charge per Block</label>
                      <Input type="number" placeholder="e.g. 100" value={cfgMaxCharge} onChange={(e) => setCfgMaxCharge(e.target.value)} />
                    </div>
                    <div className="space-y-1">
                      <label className="text-xs font-medium">Subject Min Reserve</label>
                      <Input placeholder="e.g. 5000000000000" value={cfgMinReserve} onChange={(e) => setCfgMinReserve(e.target.value)} />
                    </div>
                  </div>
                  <TxButton size="sm" onClick={handleSetBillingParams} txStatus={actions.txState.status}>
                    Save Parameters
                  </TxButton>
                </div>

                <Separator />

                {/* Emergency controls */}
                <div className="space-y-3">
                  <h3 className="text-sm font-semibold">Emergency Controls</h3>
                  <div className="flex flex-wrap gap-2">
                    <TxButton
                      size="sm"
                      variant="destructive"
                      onClick={() => actions.emergencyPauseBilling()}
                      txStatus={actions.txState.status}
                    >
                      <Pause className="mr-1.5 h-3.5 w-3.5" />Emergency Pause
                    </TxButton>
                    <TxButton
                      size="sm"
                      variant="outline"
                      onClick={() => actions.resumeBilling()}
                      txStatus={actions.txState.status}
                    >
                      <Play className="mr-1.5 h-3.5 w-3.5" />Resume Billing
                    </TxButton>
                  </div>
                </div>

                <Separator />

                {/* Charge Due */}
                <div className="space-y-3">
                  <h3 className="text-sm font-semibold">Charge Due</h3>
                  <div className="flex gap-2">
                    <Input
                      type="number"
                      placeholder="Max accounts to charge"
                      value={chargeLimit}
                      onChange={(e) => setChargeLimit(e.target.value)}
                      className="max-w-xs"
                    />
                    <TxButton
                      size="sm"
                      onClick={() => { if (chargeLimit) actions.chargeDue(Number(chargeLimit)); }}
                      txStatus={actions.txState.status}
                      disabled={!chargeLimit}
                    >
                      Charge Due
                    </TxButton>
                  </div>
                </div>

                <Separator />

                {/* Distribute to operators */}
                <div className="space-y-3">
                  <h3 className="text-sm font-semibold">Distribute to Operators</h3>
                  <div className="flex gap-2">
                    <Input
                      type="number"
                      placeholder="Max amount (smallest unit)"
                      value={distributeMax}
                      onChange={(e) => setDistributeMax(e.target.value)}
                      className="max-w-xs"
                    />
                    <TxButton
                      size="sm"
                      onClick={() => { if (distributeMax) actions.distributeToOperators(BigInt(distributeMax)); }}
                      txStatus={actions.txState.status}
                      disabled={!distributeMax}
                    >
                      Distribute
                    </TxButton>
                  </div>
                </div>
              </CardContent>
            )}
          </Card>

          {/* Fund Others */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Users className="h-5 w-5" />Fund Others
              </CardTitle>
              <CardDescription>Send funds to other users, subjects, or the IPFS pool</CardDescription>
            </CardHeader>
            <CardContent className="space-y-6">
              {/* Fund user account */}
              <div className="space-y-3">
                <h3 className="text-sm font-semibold flex items-center gap-2">
                  <Users className="h-4 w-4" />Fund User Account
                </h3>
                <div className="grid gap-2 sm:grid-cols-3">
                  <Input
                    placeholder="User address"
                    value={fundUserAddr}
                    onChange={(e) => setFundUserAddr(e.target.value)}
                  />
                  <Input
                    type="number"
                    placeholder="Amount (smallest unit)"
                    value={fundUserAmt}
                    onChange={(e) => setFundUserAmt(e.target.value)}
                  />
                  <TxButton
                    size="sm"
                    onClick={handleFundUser}
                    txStatus={actions.txState.status}
                    disabled={!fundUserAddr || !fundUserAmt}
                  >
                    Fund User
                  </TxButton>
                </div>
              </div>

              <Separator />

              {/* Fund subject account */}
              <div className="space-y-3">
                <h3 className="text-sm font-semibold flex items-center gap-2">
                  <Database className="h-4 w-4" />Fund Subject Account
                </h3>
                <div className="grid gap-2 sm:grid-cols-3">
                  <Input
                    type="number"
                    placeholder="Subject ID"
                    value={fundSubjectId}
                    onChange={(e) => setFundSubjectId(e.target.value)}
                  />
                  <Input
                    type="number"
                    placeholder="Amount (smallest unit)"
                    value={fundSubjectAmt}
                    onChange={(e) => setFundSubjectAmt(e.target.value)}
                  />
                  <TxButton
                    size="sm"
                    onClick={handleFundSubject}
                    txStatus={actions.txState.status}
                    disabled={!fundSubjectId || !fundSubjectAmt}
                  >
                    Fund Subject
                  </TxButton>
                </div>
              </div>

              <Separator />

              {/* Fund IPFS pool */}
              <div className="space-y-3">
                <h3 className="text-sm font-semibold flex items-center gap-2">
                  <Droplets className="h-4 w-4" />Fund IPFS Pool
                </h3>
                <div className="flex gap-2">
                  <Input
                    type="number"
                    placeholder="Amount (smallest unit)"
                    value={fundPoolAmt}
                    onChange={(e) => setFundPoolAmt(e.target.value)}
                    className="max-w-xs"
                  />
                  <TxButton
                    size="sm"
                    onClick={handleFundPool}
                    txStatus={actions.txState.status}
                    disabled={!fundPoolAmt}
                  >
                    Fund Pool
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
