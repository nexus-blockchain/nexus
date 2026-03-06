"use client";

import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import {
  Wallet,
  ArrowLeft,
  CreditCard,
  TrendingUp,
  Plus,
  Receipt,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const PLACEHOLDER_HISTORY = [
  { id: 1, date: "2025-01-15", description: "Weekly pin billing – 3 pins", amount: "-0.45 NEX", type: "Charge" },
  { id: 2, date: "2025-01-14", description: "Deposit to storage balance", amount: "+50.00 NEX", type: "Deposit" },
  { id: 3, date: "2025-01-08", description: "Weekly pin billing – 2 pins", amount: "-0.30 NEX", type: "Charge" },
  { id: 4, date: "2025-01-01", description: "Weekly pin billing – 2 pins", amount: "-0.30 NEX", type: "Charge" },
];

export default function StorageBillingPage() {
  const t = useTranslations("common");

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
          <p className="text-muted-foreground">Manage your storage balance and view billing history</p>
        </div>
        <Button>
          <Plus className="mr-2 h-4 w-4" />Add Funds
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Current Balance</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Wallet className="h-5 w-5 text-muted-foreground" />
            <p className="text-2xl font-bold">48.95 NEX</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Price per GiB/Week</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <TrendingUp className="h-5 w-5 text-muted-foreground" />
            <p className="text-2xl font-bold">0.15 NEX</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Est. Weeks Remaining</CardTitle></CardHeader>
          <CardContent>
            <p className="text-2xl font-bold text-green-600">~108</p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Billing History</CardTitle>
          <CardDescription>Recent charges and deposits</CardDescription>
        </CardHeader>
        <CardContent>
          {PLACEHOLDER_HISTORY.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12">
              <Receipt className="h-12 w-12 text-muted-foreground/50" />
              <p className="mt-4 text-lg font-medium">No billing history</p>
              <p className="text-sm text-muted-foreground">Charges and deposits will appear here</p>
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Date</TableHead>
                  <TableHead>Description</TableHead>
                  <TableHead>Type</TableHead>
                  <TableHead className="text-right">Amount</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {PLACEHOLDER_HISTORY.map((entry) => (
                  <TableRow key={entry.id}>
                    <TableCell className="text-muted-foreground">{entry.date}</TableCell>
                    <TableCell>{entry.description}</TableCell>
                    <TableCell>
                      <Badge variant={entry.type === "Deposit" ? "default" : "secondary"}>
                        {entry.type}
                      </Badge>
                    </TableCell>
                    <TableCell className={`text-right font-mono ${entry.type === "Deposit" ? "text-green-600" : "text-red-600"}`}>
                      {entry.amount}
                    </TableCell>
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
