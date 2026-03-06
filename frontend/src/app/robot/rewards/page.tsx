"use client";

import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import {
  Gift,
  ArrowLeft,
  Coins,
  TrendingUp,
  Clock,
  Download,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const PLACEHOLDER_REWARDS = [
  { epoch: 42, earned: "125.50 NEX", type: "Operator", status: "Claimed", claimedAt: "2025-01-14" },
  { epoch: 41, earned: "118.00 NEX", type: "Operator", status: "Claimed", claimedAt: "2025-01-07" },
  { epoch: 40, earned: "132.75 NEX", type: "Node", status: "Pending", claimedAt: "—" },
  { epoch: 39, earned: "98.25 NEX", type: "Node", status: "Pending", claimedAt: "—" },
];

export default function RobotRewardsPage() {
  const t = useTranslations("common");

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/robot"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Gift className="h-7 w-7" />
            Rewards
          </h1>
          <p className="text-muted-foreground">Track and claim your robot network rewards</p>
        </div>
        <Button>
          <Download className="mr-2 h-4 w-4" />Claim All Pending
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Pending Rewards</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Clock className="h-5 w-5 text-amber-500" />
            <p className="text-2xl font-bold text-amber-600">231.00 NEX</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Earned</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Coins className="h-5 w-5 text-muted-foreground" />
            <p className="text-2xl font-bold">474.50 NEX</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Avg. per Epoch</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <TrendingUp className="h-5 w-5 text-muted-foreground" />
            <p className="text-2xl font-bold">118.63 NEX</p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Reward History</CardTitle>
          <CardDescription>Rewards earned per epoch</CardDescription>
        </CardHeader>
        <CardContent>
          {PLACEHOLDER_REWARDS.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12">
              <Gift className="h-12 w-12 text-muted-foreground/50" />
              <p className="mt-4 text-lg font-medium">No rewards yet</p>
              <p className="text-sm text-muted-foreground">Earn rewards by operating bots and nodes</p>
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Epoch</TableHead>
                  <TableHead className="text-right">Earned</TableHead>
                  <TableHead>Type</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Claimed At</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {PLACEHOLDER_REWARDS.map((reward) => (
                  <TableRow key={reward.epoch}>
                    <TableCell className="font-mono">#{reward.epoch}</TableCell>
                    <TableCell className="text-right font-mono font-medium">{reward.earned}</TableCell>
                    <TableCell><Badge variant="outline">{reward.type}</Badge></TableCell>
                    <TableCell>
                      <Badge variant={reward.status === "Claimed" ? "default" : "secondary"}>
                        {reward.status}
                      </Badge>
                    </TableCell>
                    <TableCell className="text-muted-foreground">{reward.claimedAt}</TableCell>
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
