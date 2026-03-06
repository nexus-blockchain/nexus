"use client";

import { useState, useMemo, useEffect, useCallback } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { Separator } from "@/components/ui/separator";
import { Select } from "@/components/ui/select";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import {
  Scale,
  ArrowLeft,
  RefreshCw,
  AlertTriangle,
  ShieldCheck,
  ShieldX,
  FileWarning,
  Loader2,
  Gavel,
  XCircle,
  CheckCircle2,
} from "lucide-react";
import Link from "next/link";
import { useUserTrades, useTradeDispute, useNexMarketActions } from "@/hooks/useNexMarket";
import { useWalletStore } from "@/stores/wallet";
import { DISPUTE_RESOLUTION, USDT_TRADE_STATUS } from "@/lib/constants";
import { getApi } from "@/hooks/useApi";
import type { UsdtTrade, TradeDispute } from "@/lib/types";

const formatUsdt = (raw: number) => (raw / 1_000_000).toFixed(6);
const formatNex = (raw: bigint) => (Number(raw) / 1e12).toFixed(4);

interface TradeWithDispute extends UsdtTrade {
  dispute: TradeDispute | null;
}

export default function TradingDisputesPage() {
  const address = useWalletStore((s) => s.address);
  const { trades, isLoading: tradesLoading, refetch: refetchTrades } = useUserTrades(address);
  const {
    disputeTrade,
    resolveDispute,
    forceSettleTrade,
    forceCancelTrade,
    txState,
    resetTx,
  } = useNexMarketActions();

  const [tradesWithDisputes, setTradesWithDisputes] = useState<TradeWithDispute[]>([]);
  const [disputesLoading, setDisputesLoading] = useState(false);

  const [selectedTradeId, setSelectedTradeId] = useState("");
  const [evidenceCid, setEvidenceCid] = useState("");

  const [resolveTradeId, setResolveTradeId] = useState("");
  const [resolution, setResolution] = useState<string>(DISPUTE_RESOLUTION[0]);

  const [settleTradeId, setSettleTradeId] = useState("");
  const [settleAmount, setSettleAmount] = useState("");
  const [settleResolution, setSettleResolution] = useState<string>(DISPUTE_RESOLUTION[0]);

  const fetchDisputes = useCallback(async () => {
    if (!trades.length) {
      setTradesWithDisputes([]);
      return;
    }
    setDisputesLoading(true);
    try {
      const api = await getApi();
      const results = await Promise.all(
        trades.map(async (trade) => {
          try {
            const raw = await (api.query as any).nexMarket.tradeDisputeStore(trade.tradeId);
            const dispute = raw && !raw.isNone ? (raw.toJSON() as unknown as TradeDispute) : null;
            return { ...trade, dispute } as TradeWithDispute;
          } catch {
            return { ...trade, dispute: null } as TradeWithDispute;
          }
        })
      );
      setTradesWithDisputes(results);
    } catch {
      setTradesWithDisputes(trades.map((t) => ({ ...t, dispute: null })));
    } finally {
      setDisputesLoading(false);
    }
  }, [trades]);

  useEffect(() => {
    fetchDisputes();
  }, [fetchDisputes]);

  const disputedTrades = useMemo(
    () => tradesWithDisputes.filter((t) => t.dispute !== null),
    [tradesWithDisputes]
  );

  const openDisputes = useMemo(
    () => disputedTrades.filter((t) => t.dispute?.status === "Open").length,
    [disputedTrades]
  );
  const resolvedForBuyer = useMemo(
    () => disputedTrades.filter((t) => t.dispute?.status === "ResolvedForBuyer").length,
    [disputedTrades]
  );
  const resolvedForSeller = useMemo(
    () => disputedTrades.filter((t) => t.dispute?.status === "ResolvedForSeller").length,
    [disputedTrades]
  );

  const activeTrades = useMemo(
    () =>
      tradesWithDisputes.filter(
        (t) =>
          t.dispute === null &&
          (t.status === "AwaitingPayment" ||
            t.status === "AwaitingVerification" ||
            t.status === "UnderpaidPending")
      ),
    [tradesWithDisputes]
  );

  const handleOpenDispute = async () => {
    if (!selectedTradeId || !evidenceCid) return;
    resetTx();
    await disputeTrade(parseInt(selectedTradeId, 10), evidenceCid);
    setSelectedTradeId("");
    setEvidenceCid("");
    refetchTrades();
  };

  const handleResolve = async () => {
    if (!resolveTradeId || !resolution) return;
    resetTx();
    await resolveDispute(parseInt(resolveTradeId, 10), resolution);
    setResolveTradeId("");
    refetchTrades();
  };

  const handleForceSettle = async () => {
    if (!settleTradeId || !settleAmount) return;
    resetTx();
    const amountRaw = Math.round(parseFloat(settleAmount) * 1_000_000);
    await forceSettleTrade(parseInt(settleTradeId, 10), amountRaw, settleResolution);
    setSettleTradeId("");
    setSettleAmount("");
    refetchTrades();
  };

  const handleForceCancel = async (tradeId: number) => {
    resetTx();
    await forceCancelTrade(tradeId);
    refetchTrades();
  };

  const isRefreshing = tradesLoading || disputesLoading;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/trading">
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Scale className="h-7 w-7" />
            Trade Disputes
          </h1>
          <p className="text-muted-foreground">Manage and resolve disputes on your P2P trades</p>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => refetchTrades()}
          disabled={isRefreshing}
        >
          <RefreshCw className={`mr-2 h-3 w-3 ${isRefreshing ? "animate-spin" : ""}`} />
          Refresh
        </Button>
      </div>

      {!address && (
        <div className="rounded-lg border border-amber-300 bg-amber-50 dark:bg-amber-950/20 p-4 flex items-start gap-3">
          <AlertTriangle className="h-5 w-5 text-amber-600 mt-0.5 shrink-0" />
          <p className="text-sm text-amber-700 dark:text-amber-400">
            Connect your wallet to view your trade disputes.
          </p>
        </div>
      )}

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium flex items-center gap-2">
              <FileWarning className="h-4 w-4 text-red-500" />
              Open Disputes
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-red-600">{openDisputes}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium flex items-center gap-2">
              <ShieldCheck className="h-4 w-4 text-blue-500" />
              Resolved for Buyer
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-blue-600">{resolvedForBuyer}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium flex items-center gap-2">
              <ShieldX className="h-4 w-4 text-green-500" />
              Resolved for Seller
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-green-600">{resolvedForSeller}</p>
          </CardContent>
        </Card>
      </div>

      {isRefreshing ? (
        <Card>
          <CardContent className="flex justify-center py-12">
            <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
          </CardContent>
        </Card>
      ) : disputedTrades.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <AlertTriangle className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No disputes</p>
            <p className="text-sm text-muted-foreground">
              Trade disputes will appear here when raised
            </p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader>
            <CardTitle>Disputed Trades</CardTitle>
            <CardDescription>{disputedTrades.length} trade(s) with disputes</CardDescription>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Trade ID</TableHead>
                  <TableHead>Order ID</TableHead>
                  <TableHead>Counterparty</TableHead>
                  <TableHead className="text-right">NEX Amount</TableHead>
                  <TableHead className="text-right">USDT Amount</TableHead>
                  <TableHead>Trade Status</TableHead>
                  <TableHead>Dispute Status</TableHead>
                  <TableHead>Evidence</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {disputedTrades.map((trade) => {
                  const counterparty =
                    trade.seller === address ? trade.buyer : trade.seller;
                  return (
                    <TableRow key={trade.tradeId}>
                      <TableCell className="font-mono">#{trade.tradeId}</TableCell>
                      <TableCell className="font-mono">#{trade.orderId}</TableCell>
                      <TableCell>
                        <AddressDisplay address={counterparty} chars={6} />
                      </TableCell>
                      <TableCell className="text-right font-mono">
                        {formatNex(trade.nexAmount)}
                      </TableCell>
                      <TableCell className="text-right font-mono">
                        ${formatUsdt(trade.usdtAmount)}
                      </TableCell>
                      <TableCell>
                        <StatusBadge status={trade.status} />
                      </TableCell>
                      <TableCell>
                        <StatusBadge status={trade.dispute!.status} />
                      </TableCell>
                      <TableCell>
                        <span className="font-mono text-xs truncate max-w-[100px] inline-block" title={trade.dispute!.evidenceCid}>
                          {trade.dispute!.evidenceCid.slice(0, 12)}...
                        </span>
                      </TableCell>
                      <TableCell className="text-right">
                        <TxButton
                          size="sm"
                          variant="destructive"
                          txStatus={txState.status}
                          loadingText="Cancelling..."
                          onClick={() => handleForceCancel(trade.tradeId)}
                        >
                          <XCircle className="mr-1 h-3 w-3" />
                          Cancel
                        </TxButton>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <FileWarning className="h-5 w-5" />
              Open Dispute
            </CardTitle>
            <CardDescription>
              Raise a dispute on one of your active trades
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Select Trade</label>
              <Select
                value={selectedTradeId}
                onChange={(e) => setSelectedTradeId(e.target.value)}
              >
                <option value="">Choose an active trade...</option>
                {activeTrades.map((t) => (
                  <option key={t.tradeId} value={t.tradeId}>
                    #{t.tradeId} — {formatNex(t.nexAmount)} NEX @ ${formatUsdt(t.usdtAmount)} ({t.status})
                  </option>
                ))}
              </Select>
              {activeTrades.length === 0 && (
                <p className="text-xs text-muted-foreground">
                  No eligible trades to dispute
                </p>
              )}
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">Evidence CID</label>
              <Input
                placeholder="Qm... or bafy..."
                value={evidenceCid}
                onChange={(e) => setEvidenceCid(e.target.value)}
              />
              <p className="text-xs text-muted-foreground">
                IPFS CID pointing to your evidence (screenshots, transaction hashes, etc.)
              </p>
            </div>

            {selectedTradeId && (
              <div className="rounded-lg border p-3 text-sm space-y-1">
                {(() => {
                  const t = activeTrades.find(
                    (tr) => tr.tradeId === parseInt(selectedTradeId, 10)
                  );
                  if (!t) return null;
                  return (
                    <>
                      <div className="flex justify-between">
                        <span className="text-muted-foreground">NEX Amount</span>
                        <span className="font-mono">{formatNex(t.nexAmount)}</span>
                      </div>
                      <div className="flex justify-between">
                        <span className="text-muted-foreground">USDT Amount</span>
                        <span className="font-mono">${formatUsdt(t.usdtAmount)}</span>
                      </div>
                      <div className="flex justify-between">
                        <span className="text-muted-foreground">Seller</span>
                        <AddressDisplay address={t.seller} chars={6} />
                      </div>
                      <div className="flex justify-between">
                        <span className="text-muted-foreground">Buyer</span>
                        <AddressDisplay address={t.buyer} chars={6} />
                      </div>
                    </>
                  );
                })()}
              </div>
            )}

            <TxButton
              className="w-full"
              variant="destructive"
              txStatus={txState.status}
              loadingText="Opening dispute..."
              disabled={!selectedTradeId || !evidenceCid}
              onClick={handleOpenDispute}
            >
              <AlertTriangle className="mr-2 h-4 w-4" />
              Open Dispute
            </TxButton>

            {txState.status === "error" && (
              <p className="text-sm text-red-600">{"Transaction failed"}</p>
            )}
            {txState.status === "finalized" && (
              <p className="text-sm text-green-600">Dispute opened successfully!</p>
            )}
          </CardContent>
        </Card>

        <div className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Gavel className="h-5 w-5" />
                Resolve Dispute (Admin)
              </CardTitle>
              <CardDescription>
                Resolve an open dispute with a final decision
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Trade ID</label>
                <Input
                  type="number"
                  placeholder="Trade ID to resolve"
                  value={resolveTradeId}
                  onChange={(e) => setResolveTradeId(e.target.value)}
                />
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">Resolution</label>
                <Select
                  value={resolution}
                  onChange={(e) => setResolution(e.target.value)}
                >
                  {DISPUTE_RESOLUTION.map((r) => (
                    <option key={r} value={r}>
                      {r === "ReleaseToBuyer" ? "Release to Buyer" : "Refund to Seller"}
                    </option>
                  ))}
                </Select>
              </div>

              <TxButton
                className="w-full"
                txStatus={txState.status}
                loadingText="Resolving..."
                disabled={!resolveTradeId}
                onClick={handleResolve}
              >
                <CheckCircle2 className="mr-2 h-4 w-4" />
                Resolve Dispute
              </TxButton>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <ShieldCheck className="h-5 w-5" />
                Force Settle / Cancel (Admin)
              </CardTitle>
              <CardDescription>
                Administrative override for stuck trades
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Trade ID</label>
                <Input
                  type="number"
                  placeholder="Trade ID"
                  value={settleTradeId}
                  onChange={(e) => setSettleTradeId(e.target.value)}
                />
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">Actual USDT Amount</label>
                <Input
                  type="number"
                  placeholder="0.000000"
                  step="0.000001"
                  value={settleAmount}
                  onChange={(e) => setSettleAmount(e.target.value)}
                />
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">Resolution</label>
                <Select
                  value={settleResolution}
                  onChange={(e) => setSettleResolution(e.target.value)}
                >
                  {DISPUTE_RESOLUTION.map((r) => (
                    <option key={r} value={r}>
                      {r === "ReleaseToBuyer" ? "Release to Buyer" : "Refund to Seller"}
                    </option>
                  ))}
                </Select>
              </div>

              <div className="flex gap-2">
                <TxButton
                  className="flex-1"
                  txStatus={txState.status}
                  loadingText="Settling..."
                  disabled={!settleTradeId || !settleAmount}
                  onClick={handleForceSettle}
                >
                  <Gavel className="mr-2 h-4 w-4" />
                  Force Settle
                </TxButton>
                <TxButton
                  className="flex-1"
                  variant="destructive"
                  txStatus={txState.status}
                  loadingText="Cancelling..."
                  disabled={!settleTradeId}
                  onClick={() => {
                    if (settleTradeId) {
                      resetTx();
                      handleForceCancel(parseInt(settleTradeId, 10));
                    }
                  }}
                >
                  <XCircle className="mr-2 h-4 w-4" />
                  Force Cancel
                </TxButton>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
