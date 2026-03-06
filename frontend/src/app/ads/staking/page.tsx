"use client";

import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import {
  Coins,
  ArrowLeft,
  Plus,
  TrendingUp,
  Users,
  RotateCcw,
  Wallet,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const PLACEHOLDER_STAKES = [
  { id: 1, community: "Crypto Trading Chat", staked: "500 NEX", share: "12.5%", rewards: "25.00 NEX", status: "Active" },
  { id: 2, community: "NFT Collectors", staked: "1,000 NEX", share: "8.3%", rewards: "42.50 NEX", status: "Active" },
  { id: 3, community: "DeFi Strategies", staked: "250 NEX", share: "5.0%", rewards: "10.00 NEX", status: "Active" },
  { id: 4, community: "Gaming Guild", staked: "750 NEX", share: "15.0%", rewards: "0 NEX", status: "Unstaking" },
];

export default function AdsStakingPage() {
  const t = useTranslations("common");

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/ads"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Coins className="h-7 w-7" />
            Ad Staking
          </h1>
          <p className="text-muted-foreground">Stake NEX in communities to earn a share of ad revenue</p>
        </div>
        <Button>
          <Plus className="mr-2 h-4 w-4" />Stake in Community
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Staked</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Wallet className="h-5 w-5 text-muted-foreground" />
            <p className="text-2xl font-bold">2,500 NEX</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Active Stakes</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-green-600">3</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Rewards</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <TrendingUp className="h-5 w-5 text-green-500" />
            <p className="text-2xl font-bold text-green-600">77.50 NEX</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Communities</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Users className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">4</p>
          </CardContent>
        </Card>
      </div>

      {PLACEHOLDER_STAKES.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Coins className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No active stakes</p>
            <p className="text-sm text-muted-foreground">Stake NEX in communities to earn ad revenue share</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Community</TableHead>
                <TableHead className="text-right">Staked</TableHead>
                <TableHead className="text-right">Revenue Share</TableHead>
                <TableHead className="text-right">Rewards Earned</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Action</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {PLACEHOLDER_STAKES.map((stake) => (
                <TableRow key={stake.id}>
                  <TableCell className="font-medium">{stake.community}</TableCell>
                  <TableCell className="text-right font-mono">{stake.staked}</TableCell>
                  <TableCell className="text-right font-mono">{stake.share}</TableCell>
                  <TableCell className="text-right font-mono text-green-600">{stake.rewards}</TableCell>
                  <TableCell><StatusBadge status={stake.status} /></TableCell>
                  <TableCell className="text-right">
                    {stake.status === "Active" ? (
                      <Button variant="outline" size="sm">Unstake</Button>
                    ) : (
                      <Badge variant="secondary">Pending</Badge>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </div>
  );
}
