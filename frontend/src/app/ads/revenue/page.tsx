"use client";

import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import {
  DollarSign,
  ArrowLeft,
  TrendingUp,
  Download,
  Clock,
  Wallet,
  BarChart3,
  RotateCcw,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const PLACEHOLDER_REVENUE = [
  { id: 1, period: "Week 3, Jan 2025", source: "Crypto Trading Chat", earned: "45.00 NEX", status: "Claimed", claimedAt: "2025-01-21" },
  { id: 2, period: "Week 2, Jan 2025", source: "NFT Collectors", earned: "32.50 NEX", status: "Claimed", claimedAt: "2025-01-14" },
  { id: 3, period: "Week 3, Jan 2025", source: "DeFi Strategies", earned: "18.75 NEX", status: "Claimable", claimedAt: "—" },
  { id: 4, period: "Week 3, Jan 2025", source: "NFT Collectors", earned: "28.00 NEX", status: "Claimable", claimedAt: "—" },
  { id: 5, period: "Week 3, Jan 2025", source: "Crypto Trading Chat", earned: "52.25 NEX", status: "Pending", claimedAt: "—" },
];

export default function AdsRevenuePage() {
  const t = useTranslations("common");

  const statusVariant = (status: string) => {
    switch (status) {
      case "Claimed": return "default" as const;
      case "Claimable": return "secondary" as const;
      case "Pending": return "outline" as const;
      default: return "outline" as const;
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/ads"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <BarChart3 className="h-7 w-7" />
            Ad Revenue
          </h1>
          <p className="text-muted-foreground">Track your earned ad revenue and claim rewards</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm">
            <RotateCcw className="mr-2 h-3 w-3" />Refresh
          </Button>
          <Button>
            <Download className="mr-2 h-4 w-4" />Claim All
          </Button>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Earned</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <DollarSign className="h-5 w-5 text-muted-foreground" />
            <p className="text-2xl font-bold">176.50 NEX</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Claimed</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Wallet className="h-5 w-5 text-green-500" />
            <p className="text-2xl font-bold text-green-600">77.50 NEX</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Claimable</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <TrendingUp className="h-5 w-5 text-blue-500" />
            <p className="text-2xl font-bold text-blue-600">46.75 NEX</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Pending</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Clock className="h-5 w-5 text-amber-500" />
            <p className="text-2xl font-bold text-amber-600">52.25 NEX</p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Revenue History</CardTitle>
          <CardDescription>Earnings from ad placements in your staked communities</CardDescription>
        </CardHeader>
        <CardContent>
          {PLACEHOLDER_REVENUE.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12">
              <BarChart3 className="h-12 w-12 text-muted-foreground/50" />
              <p className="mt-4 text-lg font-medium">No revenue yet</p>
              <p className="text-sm text-muted-foreground">Earn revenue by staking in communities with active ad placements</p>
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Period</TableHead>
                  <TableHead>Source Community</TableHead>
                  <TableHead className="text-right">Earned</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Claimed At</TableHead>
                  <TableHead className="text-right">Action</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {PLACEHOLDER_REVENUE.map((rev) => (
                  <TableRow key={rev.id}>
                    <TableCell className="text-muted-foreground">{rev.period}</TableCell>
                    <TableCell className="font-medium">{rev.source}</TableCell>
                    <TableCell className="text-right font-mono font-medium">{rev.earned}</TableCell>
                    <TableCell>
                      <Badge variant={statusVariant(rev.status)}>{rev.status}</Badge>
                    </TableCell>
                    <TableCell className="text-muted-foreground">{rev.claimedAt}</TableCell>
                    <TableCell className="text-right">
                      {rev.status === "Claimable" && (
                        <Button size="sm">Claim</Button>
                      )}
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
