"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ArrowLeft, Gift, Clock, Trophy } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function CommissionPoolPage() {
  const { currentEntityId } = useEntityStore();
  const tc = useTranslations("common");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/commission"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Reward Pool</h1>
          <p className="text-muted-foreground">Pool-based commission distribution rounds</p>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Current Pool</CardTitle>
            <Gift className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">—</p>
            <p className="text-xs text-muted-foreground">Accumulated from settled orders</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Current Round</CardTitle>
            <Clock className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">—</p>
            <p className="text-xs text-muted-foreground">Active distribution round</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Total Distributed</CardTitle>
            <Trophy className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">—</p>
            <p className="text-xs text-muted-foreground">Across all rounds</p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Gift className="h-5 w-5" />How Pool Rewards Work</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3 text-sm text-muted-foreground">
          <p>The pool reward system collects a portion of commission from each order into a shared pool.</p>
          <p>At the end of each distribution round, the pool is distributed proportionally to qualifying members based on their performance metrics.</p>
          <div className="rounded-lg border p-4 space-y-2">
            <p className="font-medium text-foreground">Distribution Process:</p>
            <p>1. <strong>Accumulation</strong> — Commission portion flows into the pool each order</p>
            <p>2. <strong>Snapshot</strong> — At round end, member performance is snapshot</p>
            <p>3. <strong>Distribution</strong> — Pool is split proportionally among qualifiers</p>
            <p>4. <strong>Claim</strong> — Members claim their share from the pool</p>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader><CardTitle>Pool Rounds</CardTitle></CardHeader>
        <CardContent className="flex flex-col items-center justify-center py-12">
          <Trophy className="h-12 w-12 text-muted-foreground/50" />
          <p className="mt-4 text-lg font-medium">No Active Rounds</p>
          <p className="text-sm text-muted-foreground">Pool rounds will appear here when the pool reward plugin is enabled and configured.</p>
        </CardContent>
      </Card>
    </div>
  );
}
