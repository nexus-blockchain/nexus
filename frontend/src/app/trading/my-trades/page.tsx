"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { ArrowLeftRight, ArrowLeft, RotateCcw, Receipt } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const PLACEHOLDER_TRADES = [
  { tradeId: 201, orderId: 101, role: "Maker", nexAmount: "250.00", usdtAmount: "257.50", status: "Completed", txHash: "0xabcd...1234" },
  { tradeId: 202, orderId: 98, role: "Taker", nexAmount: "500.00", usdtAmount: "510.00", status: "Completed", txHash: "0xef56...7890" },
  { tradeId: 203, orderId: 102, role: "Maker", nexAmount: "200.00", usdtAmount: "212.00", status: "AwaitingPayment", txHash: "—" },
  { tradeId: 204, orderId: 95, role: "Taker", nexAmount: "750.00", usdtAmount: "787.50", status: "PaymentSent", txHash: "—" },
  { tradeId: 205, orderId: 90, role: "Maker", nexAmount: "100.00", usdtAmount: "103.00", status: "Disputed", txHash: "—" },
];

export default function TradingMyTradesPage() {
  const t = useTranslations("common");

  const statusVariant = (status: string) => {
    switch (status) {
      case "Completed": return "default" as const;
      case "AwaitingPayment": return "outline" as const;
      case "PaymentSent": return "secondary" as const;
      case "Disputed": return "destructive" as const;
      default: return "outline" as const;
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
          <p className="text-muted-foreground">Your USDT trade history and active settlements</p>
        </div>
        <Button variant="outline" size="sm">
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Trades</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">5</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Completed</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-green-600">2</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Pending Settlement</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-amber-600">2</p></CardContent>
        </Card>
      </div>

      {PLACEHOLDER_TRADES.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Receipt className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No trades yet</p>
            <p className="text-sm text-muted-foreground">Your completed and pending trades will appear here</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Trade ID</TableHead>
                <TableHead>Order ID</TableHead>
                <TableHead>Role</TableHead>
                <TableHead className="text-right">NEX Amount</TableHead>
                <TableHead className="text-right">USDT Amount</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>TX Hash</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {PLACEHOLDER_TRADES.map((trade) => (
                <TableRow key={trade.tradeId}>
                  <TableCell className="font-mono">#{trade.tradeId}</TableCell>
                  <TableCell className="font-mono">#{trade.orderId}</TableCell>
                  <TableCell>
                    <Badge variant={trade.role === "Maker" ? "default" : "secondary"}>{trade.role}</Badge>
                  </TableCell>
                  <TableCell className="text-right font-mono">{trade.nexAmount}</TableCell>
                  <TableCell className="text-right font-mono">{trade.usdtAmount}</TableCell>
                  <TableCell>
                    <Badge variant={statusVariant(trade.status)}>{trade.status}</Badge>
                  </TableCell>
                  <TableCell className="font-mono text-xs text-muted-foreground">{trade.txHash}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </div>
  );
}
