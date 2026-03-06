"use client";

import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import {
  Lock,
  ArrowLeft,
  Wallet,
  Shield,
  ArrowUpDown,
  RotateCcw,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const PLACEHOLDER_ESCROWS = [
  { id: 1, disputeId: 1, depositor: "5GrwvaEF...RjJQTPW", amount: "500.00 NEX", status: "Locked", createdAt: "2025-01-15" },
  { id: 2, disputeId: 2, depositor: "5FHneW46...8BnWJ9S", amount: "150.00 NEX", status: "Locked", createdAt: "2025-01-12" },
  { id: 3, disputeId: 3, depositor: "5DAAnrj7...4dKtWZq", amount: "200.00 NEX", status: "Released", createdAt: "2025-01-08" },
];

export default function DisputeEscrowPage() {
  const t = useTranslations("common");

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/dispute"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Lock className="h-7 w-7" />
            Escrow Funds
          </h1>
          <p className="text-muted-foreground">Funds held in escrow for active disputes</p>
        </div>
        <Button variant="outline" size="sm">
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Locked</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Lock className="h-5 w-5 text-amber-500" />
            <p className="text-2xl font-bold text-amber-600">650.00 NEX</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Released</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Wallet className="h-5 w-5 text-green-500" />
            <p className="text-2xl font-bold text-green-600">200.00 NEX</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Active Escrows</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Shield className="h-5 w-5 text-muted-foreground" />
            <p className="text-2xl font-bold">2</p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Escrow Records</CardTitle>
          <CardDescription>All escrow deposits linked to disputes</CardDescription>
        </CardHeader>
        <CardContent>
          {PLACEHOLDER_ESCROWS.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12">
              <Lock className="h-12 w-12 text-muted-foreground/50" />
              <p className="mt-4 text-lg font-medium">No escrow funds</p>
              <p className="text-sm text-muted-foreground">Escrow deposits will appear here when disputes are filed</p>
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>ID</TableHead>
                  <TableHead>Dispute</TableHead>
                  <TableHead>Depositor</TableHead>
                  <TableHead className="text-right">Amount</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Date</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {PLACEHOLDER_ESCROWS.map((escrow) => (
                  <TableRow key={escrow.id}>
                    <TableCell className="font-mono">#{escrow.id}</TableCell>
                    <TableCell className="font-mono">#{escrow.disputeId}</TableCell>
                    <TableCell className="font-mono text-xs">{escrow.depositor}</TableCell>
                    <TableCell className="text-right font-mono font-medium">{escrow.amount}</TableCell>
                    <TableCell>
                      <Badge variant={escrow.status === "Locked" ? "destructive" : "default"}>
                        {escrow.status}
                      </Badge>
                    </TableCell>
                    <TableCell className="text-muted-foreground">{escrow.createdAt}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
