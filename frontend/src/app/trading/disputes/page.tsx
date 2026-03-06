"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AlertTriangle, ArrowLeft, RotateCcw, Scale } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const PLACEHOLDER_DISPUTES = [
  { id: 1, tradeId: 205, initiator: "5GrwvaEF...RjJQTPW", reason: "Payment not received", status: "Open", createdAt: "2025-01-15" },
  { id: 2, tradeId: 198, initiator: "5FHneW46...8BnWJ9S", reason: "Wrong amount sent", status: "UnderReview", createdAt: "2025-01-12" },
  { id: 3, tradeId: 180, initiator: "5DAAnrj7...4dKtWZq", reason: "Seller unresponsive", status: "Resolved", createdAt: "2025-01-08" },
];

export default function TradingDisputesPage() {
  const t = useTranslations("common");

  const statusVariant = (status: string) => {
    switch (status) {
      case "Open": return "destructive" as const;
      case "UnderReview": return "outline" as const;
      case "Resolved": return "default" as const;
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
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Scale className="h-7 w-7" />
            Trade Disputes
          </h1>
          <p className="text-muted-foreground">Disputes raised on P2P trades</p>
        </div>
        <Button variant="outline" size="sm">
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Open Disputes</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-red-600">1</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Under Review</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-amber-600">1</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Resolved</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-green-600">1</p></CardContent>
        </Card>
      </div>

      {PLACEHOLDER_DISPUTES.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <AlertTriangle className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No disputes</p>
            <p className="text-sm text-muted-foreground">Trade disputes will appear here when raised</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>ID</TableHead>
                <TableHead>Trade ID</TableHead>
                <TableHead>Initiator</TableHead>
                <TableHead>Reason</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Date</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {PLACEHOLDER_DISPUTES.map((dispute) => (
                <TableRow key={dispute.id}>
                  <TableCell className="font-mono">#{dispute.id}</TableCell>
                  <TableCell className="font-mono">#{dispute.tradeId}</TableCell>
                  <TableCell className="font-mono text-xs">{dispute.initiator}</TableCell>
                  <TableCell>{dispute.reason}</TableCell>
                  <TableCell>
                    <Badge variant={statusVariant(dispute.status)}>{dispute.status}</Badge>
                  </TableCell>
                  <TableCell className="text-muted-foreground">{dispute.createdAt}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </div>
  );
}
