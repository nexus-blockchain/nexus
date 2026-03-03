"use client";

import { useEntityStore } from "@/stores/entity";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Percent, Layers, ArrowRight } from "lucide-react";

export default function CommissionPage() {
  const { currentEntityId } = useEntityStore();

  if (!currentEntityId) return <div className="flex h-full items-center justify-center text-muted-foreground">Select an entity first</div>;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Commission</h1>
        <p className="text-muted-foreground">Multi-level commission distribution configuration</p>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Commission Tiers</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Layers className="h-5 w-5 text-muted-foreground" />
            <p className="text-2xl font-bold">—</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Distributed</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Percent className="h-5 w-5 text-muted-foreground" />
            <p className="text-2xl font-bold">— NEX</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Active Referrers</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <ArrowRight className="h-5 w-5 text-muted-foreground" />
            <p className="text-2xl font-bold">—</p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Commission Rules</CardTitle>
          <CardDescription>Commission distribution rules are derived from entity member levels and referral chains</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <div className="rounded-lg border p-4">
              <h4 className="text-sm font-semibold mb-2">How Commission Works</h4>
              <ul className="space-y-2 text-sm text-muted-foreground">
                <li className="flex items-start gap-2"><span className="mt-1 h-1.5 w-1.5 rounded-full bg-primary shrink-0" />When a member places an order, commission is calculated based on the order amount and the entity&apos;s commission rate.</li>
                <li className="flex items-start gap-2"><span className="mt-1 h-1.5 w-1.5 rounded-full bg-primary shrink-0" />Commission is distributed up the referral chain according to configured tier percentages.</li>
                <li className="flex items-start gap-2"><span className="mt-1 h-1.5 w-1.5 rounded-full bg-primary shrink-0" />Each level in the referral chain receives a decreasing percentage of the commission.</li>
                <li className="flex items-start gap-2"><span className="mt-1 h-1.5 w-1.5 rounded-full bg-primary shrink-0" />Unclaimed commission from broken referral chains is returned to the entity treasury.</li>
              </ul>
            </div>
            <p className="text-sm text-muted-foreground">Commission tier configuration and distribution history will be populated from on-chain data.</p>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
