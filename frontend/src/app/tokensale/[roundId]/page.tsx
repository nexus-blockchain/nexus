"use client";

import { useState, useEffect, useCallback } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { useWalletStore } from "@/stores/wallet";
import { getApi } from "@/hooks/useApi";
import {
  useRoundPaymentOptions,
  useSubscription,
  useRoundWhitelist,
  useTokensaleActions,
} from "@/hooks/useTokensale";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import { Separator } from "@/components/ui/separator";
import { Select } from "@/components/ui/select";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { TxButton } from "@/components/shared/TxButton";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatBalance, formatNumber, basisPointsToPercent } from "@/lib/utils";
import { useTranslations } from "next-intl";
import type { SaleRound } from "@/lib/types";
import {
  ArrowLeft, Rocket, Users, Coins, Clock, ShoppingCart,
  Play, Pause, Square, XCircle, Download, Unlock,
  RefreshCw, TrendingDown, Settings, Shield,
  Target, ChevronRight, CalendarPlus, Trash2,
} from "lucide-react";

export default function SaleRoundDetailPage() {
  const params = useParams();
  const roundId = Number(params.roundId);
  const { address } = useWalletStore();
  const actions = useTokensaleActions();
  const t = useTranslations("tokensale");
  const tc = useTranslations("common");

  const [round, setRound] = useState<SaleRound | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  const [subscribeAmount, setSubscribeAmount] = useState("");
  const [increaseAmount, setIncreaseAmount] = useState("");
  const [selectedPaymentAsset, setSelectedPaymentAsset] = useState<string>("native");
  const [extendEndBlock, setExtendEndBlock] = useState("");

  const { options: paymentOptions, isLoading: optionsLoading, refetch: refetchOptions } =
    useRoundPaymentOptions(roundId);
  const { subscription, isLoading: subLoading, refetch: refetchSub } =
    useSubscription(roundId, address);
  const { whitelist, isLoading: wlLoading, refetch: refetchWl } =
    useRoundWhitelist(roundId);

  const fetchRound = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).entityTokensale.saleRounds(roundId);
      if (raw && !raw.isNone) {
        const data = raw.toJSON() as Record<string, unknown>;
        setRound({
          ...data,
          id: roundId,
          totalSupply: BigInt(String(data.totalSupply || 0)),
          soldAmount: BigInt(String(data.soldAmount || 0)),
          remainingAmount: BigInt(String(data.remainingAmount || 0)),
          dutchStartPrice: data.dutchStartPrice
            ? BigInt(String(data.dutchStartPrice))
            : null,
          dutchEndPrice: data.dutchEndPrice
            ? BigInt(String(data.dutchEndPrice))
            : null,
          totalRefundedTokens: BigInt(String(data.totalRefundedTokens || 0)),
          totalRefundedNex: BigInt(String(data.totalRefundedNex || 0)),
          softCap: BigInt(String(data.softCap || 0)),
        } as SaleRound);
      }
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [roundId]);

  useEffect(() => {
    fetchRound();
  }, [fetchRound]);

  const refetchAll = () => {
    fetchRound();
    refetchOptions();
    refetchSub();
    refetchWl();
    actions.resetTx();
  };

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
      </div>
    );
  }

  if (!round) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4">
        <Rocket className="h-16 w-16 text-muted-foreground/50" />
        <p className="text-muted-foreground">Sale round not found</p>
        <Button variant="outline" asChild>
          <Link href="/tokensale">Back to Token Sale</Link>
        </Button>
      </div>
    );
  }

  const soldPct =
    round.totalSupply > BigInt(0)
      ? Number((round.soldAmount * BigInt(10000)) / round.totalSupply) / 100
      : 0;

  const estimatedCurrentPrice = () => {
    if (!round.dutchStartPrice || !round.dutchEndPrice) return null;
    if (round.endBlock <= round.startBlock) return round.dutchStartPrice;
    const elapsed = Math.max(0, Date.now() / 6000 - round.startBlock);
    const duration = round.endBlock - round.startBlock;
    const ratio = Math.min(1, elapsed / duration);
    const start = Number(round.dutchStartPrice);
    const end = Number(round.dutchEndPrice);
    return BigInt(Math.round(start - (start - end) * ratio));
  };

  const paymentAssetValue =
    selectedPaymentAsset === "native" ? null : Number(selectedPaymentAsset);

  const isTerminal = ["Ended", "Completed", "Cancelled"].includes(round.status);

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/tokensale">
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <div className="flex-1">
          <div className="flex items-center gap-3">
            <h1 className="text-3xl font-bold tracking-tight">Round #{roundId}</h1>
            <StatusBadge status={round.status} />
            <Badge variant="outline">{round.mode}</Badge>
          </div>
          <p className="text-muted-foreground">Token Sale Round Details</p>
        </div>
        <Button variant="outline" size="sm" onClick={refetchAll}>
          <RefreshCw className="mr-2 h-3.5 w-3.5" />
          Refresh
        </Button>
      </div>

      {/* Stats cards */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Total Supply</CardTitle>
            <Coins className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{formatBalance(round.totalSupply)}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Sold</CardTitle>
            <ShoppingCart className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{formatBalance(round.soldAmount)}</p>
            <Progress value={soldPct} className="mt-2 h-2" />
            <p className="mt-1 text-xs text-muted-foreground">{soldPct.toFixed(1)}% sold</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Remaining</CardTitle>
            <Coins className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{formatBalance(round.remainingAmount)}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Participants</CardTitle>
            <Users className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{formatNumber(round.participantsCount)}</p>
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        {/* Round Info */}
        <Card>
          <CardHeader>
            <CardTitle>Round Information</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Mode</span>
              <Badge variant="outline">{round.mode}</Badge>
            </div>
            <Separator />
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Status</span>
              <StatusBadge status={round.status} />
            </div>
            <Separator />
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Creator</span>
              <AddressDisplay address={round.creator} />
            </div>
            <Separator />
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Start Block</span>
              <span className="text-sm font-mono">#{round.startBlock || "Immediate"}</span>
            </div>
            <Separator />
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">End Block</span>
              <span className="text-sm font-mono">#{round.endBlock || "No end"}</span>
            </div>
            <Separator />
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Soft Cap</span>
              <span className="text-sm font-mono">{formatBalance(round.softCap)}</span>
            </div>
            <Separator />
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">KYC Required</span>
              <div className="flex items-center gap-2">
                {round.kycRequired ? (
                  <>
                    <Shield className="h-3.5 w-3.5 text-blue-500" />
                    <span className="text-sm">Level {round.minKycLevel}</span>
                  </>
                ) : (
                  <span className="text-sm text-muted-foreground">No</span>
                )}
              </div>
            </div>
            <Separator />
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Funds Withdrawn</span>
              <span className="text-sm">{round.fundsWithdrawn ? "Yes" : "No"}</span>
            </div>
            {round.cancelledAt && (
              <>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Cancelled At</span>
                  <span className="text-sm font-mono">Block #{round.cancelledAt}</span>
                </div>
              </>
            )}
            {round.totalRefundedTokens > BigInt(0) && (
              <>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Refunded Tokens</span>
                  <span className="text-sm font-mono">
                    {formatBalance(round.totalRefundedTokens)}
                  </span>
                </div>
              </>
            )}
          </CardContent>
        </Card>

        {/* Dutch Auction / Vesting */}
        <div className="space-y-6">
          {round.mode === "DutchAuction" &&
            round.dutchStartPrice !== null &&
            round.dutchEndPrice !== null && (
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    <TrendingDown className="h-5 w-5" />
                    Dutch Auction
                  </CardTitle>
                </CardHeader>
                <CardContent className="space-y-3">
                  <div className="flex justify-between">
                    <span className="text-sm text-muted-foreground">Start Price</span>
                    <span className="text-sm font-mono">
                      {formatBalance(round.dutchStartPrice)}
                    </span>
                  </div>
                  <Separator />
                  <div className="flex justify-between">
                    <span className="text-sm text-muted-foreground">End Price</span>
                    <span className="text-sm font-mono">
                      {formatBalance(round.dutchEndPrice)}
                    </span>
                  </div>
                  {round.status === "Active" && (
                    <>
                      <Separator />
                      <div className="flex justify-between">
                        <span className="text-sm text-muted-foreground">
                          Estimated Current Price
                        </span>
                        <span className="text-sm font-mono font-bold text-primary">
                          {estimatedCurrentPrice()
                            ? formatBalance(estimatedCurrentPrice()!)
                            : "—"}
                        </span>
                      </div>
                    </>
                  )}
                </CardContent>
              </Card>
            )}

          {round.vestingConfig && (
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Settings className="h-5 w-5" />
                  Vesting Configuration
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Type</span>
                  <Badge variant="outline">{round.vestingConfig.vestingType}</Badge>
                </div>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Initial Unlock</span>
                  <span className="text-sm font-mono">
                    {basisPointsToPercent(round.vestingConfig.initialUnlockBps)}
                  </span>
                </div>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Cliff Duration</span>
                  <span className="text-sm font-mono">
                    {formatNumber(round.vestingConfig.cliffDuration)} blocks
                  </span>
                </div>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Total Duration</span>
                  <span className="text-sm font-mono">
                    {formatNumber(round.vestingConfig.totalDuration)} blocks
                  </span>
                </div>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Unlock Interval</span>
                  <span className="text-sm font-mono">
                    {formatNumber(round.vestingConfig.unlockInterval)} blocks
                  </span>
                </div>
              </CardContent>
            </Card>
          )}
        </div>
      </div>

      {/* Payment Options */}
      <Card>
        <CardHeader>
          <CardTitle>Payment Options</CardTitle>
          <CardDescription>
            {paymentOptions.length} payment option{paymentOptions.length !== 1 && "s"} configured
          </CardDescription>
        </CardHeader>
        <CardContent>
          {optionsLoading ? (
            <div className="flex justify-center py-6">
              <div className="h-6 w-6 animate-spin rounded-full border-4 border-primary border-t-transparent" />
            </div>
          ) : paymentOptions.length === 0 ? (
            <p className="text-sm text-muted-foreground">No payment options configured yet.</p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Asset</TableHead>
                  <TableHead>Price</TableHead>
                  <TableHead>Min Purchase</TableHead>
                  <TableHead>Max Per Account</TableHead>
                  <TableHead>Status</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {paymentOptions.map((opt, i) => (
                  <TableRow key={i}>
                    <TableCell className="font-medium">
                      {opt.assetId === null ? "NEX (Native)" : `Asset #${opt.assetId}`}
                    </TableCell>
                    <TableCell className="font-mono">{formatBalance(opt.price)}</TableCell>
                    <TableCell className="font-mono">{formatBalance(opt.minPurchase)}</TableCell>
                    <TableCell className="font-mono">
                      {formatBalance(opt.maxPurchasePerAccount)}
                    </TableCell>
                    <TableCell>
                      <StatusBadge status={opt.enabled ? "Active" : "Paused"} />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      {/* User Subscription */}
      {address && (
        <Card>
          <CardHeader>
            <CardTitle>Your Subscription</CardTitle>
            <CardDescription>
              {subscription ? "You are subscribed to this round" : "You have not subscribed yet"}
            </CardDescription>
          </CardHeader>
          <CardContent>
            {subLoading ? (
              <div className="flex justify-center py-6">
                <div className="h-6 w-6 animate-spin rounded-full border-4 border-primary border-t-transparent" />
              </div>
            ) : subscription ? (
              <div className="space-y-3">
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Amount</span>
                  <span className="text-sm font-mono">{formatBalance(subscription.amount)}</span>
                </div>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Payment Asset</span>
                  <span className="text-sm">
                    {subscription.paymentAsset === null
                      ? "NEX (Native)"
                      : `Asset #${subscription.paymentAsset}`}
                  </span>
                </div>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Payment Amount</span>
                  <span className="text-sm font-mono">
                    {formatBalance(subscription.paymentAmount)}
                  </span>
                </div>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Subscribed At</span>
                  <span className="text-sm font-mono">Block #{subscription.subscribedAt}</span>
                </div>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Claimed</span>
                  <StatusBadge status={subscription.claimed ? "Completed" : "Pending"} />
                </div>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Unlocked Amount</span>
                  <span className="text-sm font-mono">
                    {formatBalance(subscription.unlockedAmount)}
                  </span>
                </div>
                {subscription.refunded && (
                  <>
                    <Separator />
                    <div className="flex justify-between">
                      <span className="text-sm text-muted-foreground">Refunded</span>
                      <Badge variant="secondary">Yes</Badge>
                    </div>
                  </>
                )}
              </div>
            ) : (
              <p className="text-sm text-muted-foreground">
                Subscribe to this round to participate in the token sale.
              </p>
            )}
          </CardContent>
        </Card>
      )}

      {/* Whitelist (only for WhitelistAllocation mode) */}
      {round.mode === "WhitelistAllocation" && (
        <Card>
          <CardHeader>
            <CardTitle>Whitelist</CardTitle>
            <CardDescription>
              {whitelist.length} address{whitelist.length !== 1 && "es"} whitelisted
            </CardDescription>
          </CardHeader>
          <CardContent>
            {wlLoading ? (
              <div className="flex justify-center py-6">
                <div className="h-6 w-6 animate-spin rounded-full border-4 border-primary border-t-transparent" />
              </div>
            ) : whitelist.length === 0 ? (
              <p className="text-sm text-muted-foreground">No addresses whitelisted yet.</p>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Account</TableHead>
                    <TableHead>Allocation</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {whitelist.map((entry, i) => (
                    <TableRow key={i}>
                      <TableCell>
                        <AddressDisplay address={entry.account} />
                      </TableCell>
                      <TableCell className="font-mono">
                        {entry.allocation !== null
                          ? formatBalance(entry.allocation)
                          : "Unlimited"}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </CardContent>
        </Card>
      )}

      {/* Actions */}
      <Card>
        <CardHeader>
          <CardTitle>Actions</CardTitle>
          <CardDescription>Available actions based on round status</CardDescription>
        </CardHeader>
        <CardContent className="space-y-6">
          {/* Start */}
          {round.status === "NotStarted" && (
            <TxButton onClick={() => actions.startSale(roundId)} txStatus={actions.txState.status}>
              <Play className="mr-2 h-4 w-4" />
              Start Sale
            </TxButton>
          )}

          {/* Subscribe */}
          {round.status === "Active" && (
            <div className="space-y-4">
              <h4 className="text-sm font-semibold">Subscribe</h4>
              <div className="flex items-end gap-4">
                <div className="flex-1 space-y-2">
                  <label className="text-sm font-medium">Amount</label>
                  <Input
                    type="number"
                    value={subscribeAmount}
                    onChange={(e) => setSubscribeAmount(e.target.value)}
                    placeholder="Token amount to buy"
                    min="0"
                  />
                </div>
                {paymentOptions.length > 0 && (
                  <div className="w-48 space-y-2">
                    <label className="text-sm font-medium">Payment Asset</label>
                    <Select
                      value={selectedPaymentAsset}
                      onChange={(e) => setSelectedPaymentAsset(e.target.value)}
                    >
                      <option value="native">NEX (Native)</option>
                      {paymentOptions
                        .filter((o) => o.assetId !== null)
                        .map((o) => (
                          <option key={o.assetId} value={String(o.assetId)}>
                            Asset #{o.assetId}
                          </option>
                        ))}
                    </Select>
                  </div>
                )}
                <TxButton
                  onClick={() => {
                    if (subscribeAmount) {
                      actions.subscribe(
                        roundId,
                        BigInt(subscribeAmount),
                        paymentAssetValue
                      );
                    }
                  }}
                  txStatus={actions.txState.status}
                  disabled={!subscribeAmount}
                >
                  <ShoppingCart className="mr-2 h-4 w-4" />
                  Subscribe
                </TxButton>
              </div>

              {subscription && (
                <>
                  <Separator />
                  <h4 className="text-sm font-semibold">Increase Subscription</h4>
                  <div className="flex items-end gap-4">
                    <div className="flex-1 space-y-2">
                      <label className="text-sm font-medium">Additional Amount</label>
                      <Input
                        type="number"
                        value={increaseAmount}
                        onChange={(e) => setIncreaseAmount(e.target.value)}
                        placeholder="Additional tokens"
                        min="0"
                      />
                    </div>
                    <TxButton
                      onClick={() => {
                        if (increaseAmount) {
                          actions.increaseSubscription(
                            roundId,
                            BigInt(increaseAmount),
                            paymentAssetValue
                          );
                        }
                      }}
                      txStatus={actions.txState.status}
                      disabled={!increaseAmount}
                    >
                      <ShoppingCart className="mr-2 h-4 w-4" />
                      Increase
                    </TxButton>
                  </div>
                </>
              )}
            </div>
          )}

          {/* Admin controls */}
          {(round.status === "Active" || round.status === "Paused") && (
            <>
              <Separator />
              <h4 className="text-sm font-semibold">Admin Controls</h4>
              <div className="flex flex-wrap gap-3">
                {round.status === "Active" && (
                  <TxButton
                    variant="outline"
                    onClick={() => actions.pauseSale(roundId)}
                    txStatus={actions.txState.status}
                  >
                    <Pause className="mr-2 h-4 w-4" />
                    Pause Sale
                  </TxButton>
                )}
                {round.status === "Paused" && (
                  <TxButton
                    onClick={() => actions.resumeSale(roundId)}
                    txStatus={actions.txState.status}
                  >
                    <Play className="mr-2 h-4 w-4" />
                    Resume Sale
                  </TxButton>
                )}
                {round.status === "Active" && (
                  <TxButton
                    variant="outline"
                    onClick={() => actions.endSale(roundId)}
                    txStatus={actions.txState.status}
                  >
                    <Square className="mr-2 h-4 w-4" />
                    End Sale
                  </TxButton>
                )}
              </div>
            </>
          )}

          {/* Extend sale */}
          {(round.status === "Active" || round.status === "Paused") && (
            <>
              <Separator />
              <h4 className="text-sm font-semibold">Extend Sale</h4>
              <div className="flex items-end gap-4">
                <div className="flex-1 space-y-2">
                  <label className="text-sm font-medium">New End Block</label>
                  <Input
                    type="number"
                    value={extendEndBlock}
                    onChange={(e) => setExtendEndBlock(e.target.value)}
                    placeholder={`Current: ${round.endBlock}`}
                    min={round.endBlock + 1}
                  />
                </div>
                <TxButton
                  variant="outline"
                  onClick={() => {
                    if (extendEndBlock) {
                      actions.extendSale(roundId, Number(extendEndBlock));
                    }
                  }}
                  txStatus={actions.txState.status}
                  disabled={!extendEndBlock}
                >
                  <CalendarPlus className="mr-2 h-4 w-4" />
                  Extend
                </TxButton>
              </div>
            </>
          )}

          {/* Cancel */}
          {!isTerminal && (
            <>
              <Separator />
              <TxButton
                variant="destructive"
                onClick={() => actions.cancelSale(roundId)}
                txStatus={actions.txState.status}
              >
                <XCircle className="mr-2 h-4 w-4" />
                Cancel Sale
              </TxButton>
            </>
          )}

          {/* Post-sale actions */}
          {["Ended", "Completed"].includes(round.status) && (
            <>
              <Separator />
              <h4 className="text-sm font-semibold">Post-Sale Actions</h4>
              <div className="flex flex-wrap gap-3">
                <TxButton
                  onClick={() => actions.claimTokens(roundId)}
                  txStatus={actions.txState.status}
                >
                  <Download className="mr-2 h-4 w-4" />
                  Claim Tokens
                </TxButton>
                <TxButton
                  variant="outline"
                  onClick={() => actions.unlockTokens(roundId)}
                  txStatus={actions.txState.status}
                >
                  <Unlock className="mr-2 h-4 w-4" />
                  Unlock Tokens
                </TxButton>
                <TxButton
                  variant="outline"
                  onClick={() => actions.withdrawFunds(roundId)}
                  txStatus={actions.txState.status}
                >
                  <Download className="mr-2 h-4 w-4" />
                  Withdraw Funds
                </TxButton>
              </div>
            </>
          )}

          {/* Refund (Cancelled) */}
          {round.status === "Cancelled" && (
            <>
              <Separator />
              <TxButton
                onClick={() => actions.claimRefund(roundId)}
                txStatus={actions.txState.status}
              >
                <RefreshCw className="mr-2 h-4 w-4" />
                Claim Refund
              </TxButton>
            </>
          )}

          {/* Finalized / terminal */}
          {isTerminal && (
            <p className="text-sm text-muted-foreground">
              This round is {round.status.toLowerCase()}. Limited actions available.
            </p>
          )}
        </CardContent>
      </Card>

      {/* Tx feedback */}
      {actions.txState.status === "finalized" && (
        <div className="rounded-lg border border-green-200 bg-green-50 p-4 dark:border-green-800 dark:bg-green-950">
          <p className="text-sm text-green-800 dark:text-green-200">
            Transaction completed successfully!
          </p>
          <Button
            variant="link"
            className="mt-1 h-auto p-0 text-green-700 dark:text-green-300"
            onClick={refetchAll}
          >
            Refresh data
          </Button>
        </div>
      )}
      {actions.txState.status === "error" && (
        <p className="text-sm text-destructive">{actions.txState.error}</p>
      )}
    </div>
  );
}
