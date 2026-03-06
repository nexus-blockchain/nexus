"use client";

import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/shared/StatusBadge";
import {
  Megaphone,
  Plus,
  Search,
  BarChart3,
  DollarSign,
  Eye,
  RotateCcw,
} from "lucide-react";
import { useTranslations } from "next-intl";

const PLACEHOLDER_CAMPAIGNS = [
  { id: 1, name: "Token Launch Promo", type: "Banner", budget: "1,000 NEX", spent: "450 NEX", impressions: 12_500, status: "Active" },
  { id: 2, name: "Community Growth", type: "Sponsored", budget: "500 NEX", spent: "500 NEX", impressions: 8_200, status: "Completed" },
  { id: 3, name: "NFT Drop Awareness", type: "Native", budget: "2,000 NEX", spent: "120 NEX", impressions: 3_100, status: "Active" },
  { id: 4, name: "DeFi Product Launch", type: "Banner", budget: "750 NEX", spent: "0 NEX", impressions: 0, status: "Draft" },
];

export default function AdsPage() {
  const t = useTranslations("common");

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Megaphone className="h-8 w-8" />
            Ad Campaigns
          </h1>
          <p className="text-muted-foreground">Create and manage advertising campaigns across communities</p>
        </div>
        <Button>
          <Plus className="mr-2 h-4 w-4" />Create Campaign
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Campaigns</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">4</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Active</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-green-600">2</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Budget</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <DollarSign className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">4,250 NEX</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Impressions</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Eye className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">23.8K</p>
          </CardContent>
        </Card>
      </div>

      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input placeholder="Search campaigns..." className="pl-9" />
        </div>
        <Button variant="outline" size="sm">
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {PLACEHOLDER_CAMPAIGNS.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Megaphone className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No campaigns yet</p>
            <p className="text-sm text-muted-foreground">Create your first ad campaign to reach communities</p>
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          {PLACEHOLDER_CAMPAIGNS.map((campaign) => (
            <Card key={campaign.id} className="hover:border-primary/50 transition-colors">
              <CardHeader>
                <div className="flex items-center justify-between">
                  <CardTitle className="text-base">{campaign.name}</CardTitle>
                  <StatusBadge status={campaign.status} />
                </div>
                <CardDescription>Campaign #{campaign.id}</CardDescription>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="flex items-center justify-between text-sm">
                  <span className="text-muted-foreground">Type</span>
                  <Badge variant="outline">{campaign.type}</Badge>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-muted-foreground">Budget</span>
                  <span className="font-mono font-medium">{campaign.budget}</span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-muted-foreground">Spent</span>
                  <span className="font-mono">{campaign.spent}</span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-muted-foreground">Impressions</span>
                  <span className="font-mono">{campaign.impressions.toLocaleString()}</span>
                </div>
                <div className="w-full bg-secondary rounded-full h-2">
                  <div
                    className="bg-primary h-2 rounded-full"
                    style={{
                      width: `${Math.min(100, (parseFloat(campaign.spent.replace(/[^0-9.]/g, "")) / parseFloat(campaign.budget.replace(/[^0-9.]/g, ""))) * 100)}%`,
                    }}
                  />
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
