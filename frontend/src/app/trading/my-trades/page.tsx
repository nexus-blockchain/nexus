"use client";

import { useState, useMemo } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import {
  Table, TableHeader, TableBody, TableRow, TableHead, TableCell,
} from "@/components/ui/table";
import { Separator } from "@/components/ui/separator";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import {
  ArrowLeft, RotateCcw, Loader2, Receipt,
  ChevronDown, ChevronUp, CheckCircle, Clock,
  AlertTriangle, Award, FileWarning, Shield,
} from "lucide-react";
import Link from "next/link";
import { useWalletStore } from "@/stores/wallet";
import { useUserTrades } from "@/hooks/useNexMarket";
import { useNexMarketActions } from "@/hooks/useNexMarket";
import { USDT_TRADE_STATUS, BUYER_DEPOSIT_STATUS } from "@/lib/constants";
import type { UsdtTrade } from "@/lib/types";

const formatUsdt = (raw: number) => (raw / 1_000_000).toFixed(6);
const formatNex = (raw: bigint) => (Number(raw) / 1e12).toFixed(4);

const STATUS_FILTERS = ["All", ...USDT_TRADE_STATUS] as const;

export default function TradingMyTradesPage() {
  const { address } = useWalletStore();
  const { trades, isLoading, refetch } = useUserTrades(address);
  const {
    confirmPayment, processTimeout, disputeTrade,
    finalizeUnderpaid, claimVerificationReward,
    txState, resetTx,
  } = useNexMarketActions();

  const [statusFilter, setStatusFilter] = useState<string>("All");
  const [expandedTradeId, setExpandedTradeId] = useState<number | null>(null);
  const [disputeInputs, setDisputeInputs] = useState<Record<number, string>>({});

  const filteredTrades = useMemo(() => {
    if (statusFilter === "All") return trades;
    return trades.filter((t) => t.status === statusFilter);
  }, [trades, statusFilter]);

  const stats = useMemo(() => ({
    total: trades.length,
    awaitingPayment: trades.filter((t) => t.status === "AwaitingPayment").length,
    awaitingVerification: trades.filter((t) => t.status === "AwaitingVerification").length,
    completed: trades.filter((t) => t.status === "Completed").length,
    underpaid: trades.filter((t) => t.status === "UnderpaidPending").length,
  }), [trades]);

  const getUserRole = (trade: UsdtTrade) => {
    if (!address) return "Unknown";
    return trade.seller === address ? "Seller" : "Buyer";
  };

  const getCounterparty = (trade: UsdtTrade) => {
    if (!address) return trade.buyer;
    return trade.seller === address ? trade.buyer : trade.seller;
  };

  const isActiveTrade = (status: string) =>
    status === "AwaitingPayment" || status === "AwaitingVerification" || status === "UnderpaidPending";

  const handleConfirmPayment = async (tradeId: number) => {
    await confirmPayment(tradeId);
    refetch();
  };

  const handleProcessTimeout = async (tradeId: number) => {
    await processTimeout(tradeId);
    refetch();
  };

  const handleFinalizeUnderpaid = async (tradeId: number) => {
    await finalizeUnderpaid(tradeId);
    refetch();
  };

  const handleClaimReward = async (tradeId: number) => {
    await claimVerificationReward(tradeId);
    refetch();
  };

  const handleDispute = async (tradeId: number) => {
    const cid = disputeInputs[tradeId];
    if (!cid) return;
    await disputeTrade(tradeId, cid);
    setDisputeInputs((prev) => ({ ...prev, [tradeId]: "" }));
    refetch();
  };

  const toggleExpand = (tradeId: number) => {
    setExpandedTradeId((prev) => (prev === tradeId ? null : tradeId));
  };

  const depositColor = (status: string) => {
    switch (status) {
      case "Locked": return "bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400";
      case "Released": return "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400";
      case "Forfeited": return "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400";
      default: return "bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400";
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/trading"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">My Trades</h1>
          <p className="text-muted-foreground">
            Your USDT trade history and active settlements
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={() => refetch()} disabled={isLoading}>
          {isLoading
            ? <Loader2 className="mr-2 h-3 w-3 animate-spin" />
            : <RotateCcw className="mr-2 h-3 w-3" />}
          Refresh
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-5">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Total Trades</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{stats.total}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Awaiting Payment</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-amber-600">{stats.awaitingPayment}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Awaiting Verification</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-blue-600">{stats.awaitingVerification}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Completed</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-green-600">{stats.completed}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Underpaid</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-orange-600">{stats.underpaid}</p>
          </CardContent>
        </Card>
      </div>

      <div className="flex flex-wrap gap-2">
        {STATUS_FILTERS.map((s) => (
          <Button
            key={s}
            variant={statusFilter === s ? "default" : "outline"}
            size="sm"
            onClick={() => setStatusFilter(s)}
          >
            {s === "AwaitingPayment" ? "Await Pay"
              : s === "AwaitingVerification" ? "Await Verify"
              : s === "UnderpaidPending" ? "Underpaid"
              : s}
          </Button>
        ))}
      </div>

      {filteredTrades.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Receipt className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">
              {isLoading ? "Loading trades..." : "No trades found"}
            </p>
            <p className="text-sm text-muted-foreground">
              {isLoading
                ? "Fetching your trades from the chain"
                : "Your completed and pending trades will appear here"}
            </p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-[40px]" />
                  <TableHead className="w-[70px]">Trade</TableHead>
                  <TableHead className="w-[70px]">Order</TableHead>
                  <TableHead>Role</TableHead>
                  <TableHead>Counterparty</TableHead>
                  <TableHead className="text-right">NEX Amount</TableHead>
                  <TableHead className="text-right">USDT Amount</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Deposit</TableHead>
                  <TableHead className="text-right">Timeout</TableHead>
                  <TableHead className="text-right">Created</TableHead>
                  <TableHead className="text-right w-[240px]">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filteredTrades.map((trade) => {
                  const role = getUserRole(trade);
                  const counterparty = getCounterparty(trade);
                  const isExpanded = expandedTradeId === trade.tradeId;

                  return (
                    <>
                      <TableRow
                        key={trade.tradeId}
                        className="cursor-pointer hover:bg-muted/50"
                        onClick={() => toggleExpand(trade.tradeId)}
                      >
                        <TableCell>
                          {isExpanded
                            ? <ChevronUp className="h-4 w-4 text-muted-foreground" />
                            : <ChevronDown className="h-4 w-4 text-muted-foreground" />}
                        </TableCell>
                        <TableCell className="font-mono">#{trade.tradeId}</TableCell>
                        <TableCell className="font-mono">#{trade.orderId}</TableCell>
                        <TableCell>
                          <Badge
                            variant="outline"
                            className={
                              role === "Seller"
                                ? "border-red-300 text-red-700 dark:border-red-700 dark:text-red-400"
                                : "border-green-300 text-green-700 dark:border-green-700 dark:text-green-400"
                            }
                          >
                            {role}
                          </Badge>
                        </TableCell>
                        <TableCell>
                          <AddressDisplay address={counterparty} chars={6} />
                        </TableCell>
                        <TableCell className="text-right font-mono">
                          {formatNex(trade.nexAmount)}
                        </TableCell>
                        <TableCell className="text-right font-mono">
                          {formatUsdt(trade.usdtAmount)}
                        </TableCell>
                        <TableCell>
                          <StatusBadge status={trade.status} />
                        </TableCell>
                        <TableCell>
                          <span className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-semibold ${depositColor(trade.depositStatus)}`}>
                            {trade.depositStatus}
                          </span>
                        </TableCell>
                        <TableCell className="text-right font-mono text-xs text-muted-foreground">
                          #{trade.timeoutAt}
                        </TableCell>
                        <TableCell className="text-right font-mono text-xs text-muted-foreground">
                          #{trade.createdAt}
                        </TableCell>
                        <TableCell className="text-right" onClick={(e) => e.stopPropagation()}>
                          <div className="flex items-center gap-1 justify-end flex-wrap">
                            {trade.status === "AwaitingPayment" && (
                              <>
                                <TxButton
                                  size="sm"
                                  variant="outline"
                                  className="h-7 text-xs"
                                  txStatus={txState.status}
                                  onClick={() => handleConfirmPayment(trade.tradeId)}
                                >
                                  <CheckCircle className="mr-1 h-3 w-3" />
                                  Confirm
                                </TxButton>
                                <TxButton
                                  size="sm"
                                  variant="ghost"
                                  className="h-7 text-xs"
                                  txStatus={txState.status}
                                  onClick={() => handleProcessTimeout(trade.tradeId)}
                                >
                                  <Clock className="mr-1 h-3 w-3" />
                                  Timeout
                                </TxButton>
                              </>
                            )}

                            {trade.status === "AwaitingVerification" && (
                              <span className="text-xs text-muted-foreground italic">
                                Awaiting OCW...
                              </span>
                            )}

                            {trade.status === "UnderpaidPending" && (
                              <TxButton
                                size="sm"
                                variant="outline"
                                className="h-7 text-xs"
                                txStatus={txState.status}
                                onClick={() => handleFinalizeUnderpaid(trade.tradeId)}
                              >
                                <FileWarning className="mr-1 h-3 w-3" />
                                Finalize
                              </TxButton>
                            )}

                            {trade.status === "Completed" && (
                              <TxButton
                                size="sm"
                                variant="outline"
                                className="h-7 text-xs"
                                txStatus={txState.status}
                                onClick={() => handleClaimReward(trade.tradeId)}
                              >
                                <Award className="mr-1 h-3 w-3" />
                                Claim Reward
                              </TxButton>
                            )}
                          </div>
                        </TableCell>
                      </TableRow>

                      {isExpanded && (
                        <TableRow key={`${trade.tradeId}-detail`} className="bg-muted/30">
                          <TableCell colSpan={12} className="p-0">
                            <div className="px-6 py-4 space-y-4">
                              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                                <div className="space-y-2">
                                  <h4 className="text-sm font-semibold">Tron Addresses</h4>
                                  <div className="space-y-1 text-sm">
                                    <div className="flex justify-between">
                                      <span className="text-muted-foreground">Seller:</span>
                                      <span className="font-mono text-xs" title={trade.sellerTronAddress}>
                                        {trade.sellerTronAddress.slice(0, 8)}...{trade.sellerTronAddress.slice(-6)}
                                      </span>
                                    </div>
                                    <div className="flex justify-between">
                                      <span className="text-muted-foreground">Buyer:</span>
                                      {trade.buyerTronAddress ? (
                                        <span className="font-mono text-xs" title={trade.buyerTronAddress}>
                                          {trade.buyerTronAddress.slice(0, 8)}...{trade.buyerTronAddress.slice(-6)}
                                        </span>
                                      ) : (
                                        <span className="text-muted-foreground">—</span>
                                      )}
                                    </div>
                                  </div>
                                </div>

                                <div className="space-y-2">
                                  <h4 className="text-sm font-semibold">Deposit Info</h4>
                                  <div className="space-y-1 text-sm">
                                    <div className="flex justify-between">
                                      <span className="text-muted-foreground">Amount:</span>
                                      <span className="font-mono">
                                        {formatNex(trade.buyerDeposit)} NEX
                                      </span>
                                    </div>
                                    <div className="flex justify-between">
                                      <span className="text-muted-foreground">Status:</span>
                                      <span className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-semibold ${depositColor(trade.depositStatus)}`}>
                                        {trade.depositStatus}
                                      </span>
                                    </div>
                                  </div>
                                </div>

                                <div className="space-y-2">
                                  <h4 className="text-sm font-semibold">Verification</h4>
                                  <div className="space-y-1 text-sm">
                                    <div className="flex justify-between">
                                      <span className="text-muted-foreground">First verified:</span>
                                      <span className="font-mono">
                                        {trade.firstVerifiedAt !== null ? `#${trade.firstVerifiedAt}` : "—"}
                                      </span>
                                    </div>
                                    <div className="flex justify-between">
                                      <span className="text-muted-foreground">Actual USDT:</span>
                                      <span className="font-mono">
                                        {trade.firstActualAmount !== null
                                          ? formatUsdt(trade.firstActualAmount)
                                          : "—"}
                                      </span>
                                    </div>
                                    <div className="flex justify-between">
                                      <span className="text-muted-foreground">Underpaid deadline:</span>
                                      <span className="font-mono">
                                        {trade.underpaidDeadline !== null ? `#${trade.underpaidDeadline}` : "—"}
                                      </span>
                                    </div>
                                  </div>
                                </div>
                              </div>

                              {isActiveTrade(trade.status) && (
                                <>
                                  <Separator />
                                  <div className="flex items-end gap-2">
                                    <div className="flex-1 max-w-sm space-y-1">
                                      <label className="text-xs font-medium flex items-center gap-1">
                                        <Shield className="h-3 w-3" />
                                        Dispute Trade
                                      </label>
                                      <Input
                                        placeholder="Evidence CID (IPFS hash)"
                                        className="h-8 text-sm"
                                        value={disputeInputs[trade.tradeId] || ""}
                                        onChange={(e) =>
                                          setDisputeInputs((prev) => ({
                                            ...prev,
                                            [trade.tradeId]: e.target.value,
                                          }))
                                        }
                                      />
                                    </div>
                                    <TxButton
                                      size="sm"
                                      variant="destructive"
                                      className="h-8"
                                      txStatus={txState.status}
                                      disabled={!disputeInputs[trade.tradeId]}
                                      onClick={() => handleDispute(trade.tradeId)}
                                    >
                                      <AlertTriangle className="mr-1 h-3 w-3" />
                                      Dispute
                                    </TxButton>
                                  </div>
                                </>
                              )}
                            </div>
                          </TableCell>
                        </TableRow>
                      )}
                    </>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        </Card>
      )}
    </div>
  );
}
