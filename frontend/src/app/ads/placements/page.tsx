"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import {
  LayoutGrid,
  ArrowLeft,
  Search,
  Plus,
  Eye,
  MousePointerClick,
  RotateCcw,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const PLACEHOLDER_PLACEMENTS = [
  { id: 1, campaignId: 1, community: "Crypto Trading Chat", position: "Top Banner", impressions: 5_200, clicks: 312, ctr: "6.0%", status: "Active" },
  { id: 2, campaignId: 1, community: "NFT Collectors", position: "Sidebar", impressions: 7_300, clicks: 219, ctr: "3.0%", status: "Active" },
  { id: 3, campaignId: 3, community: "DeFi Strategies", position: "Inline", impressions: 3_100, clicks: 155, ctr: "5.0%", status: "Active" },
  { id: 4, campaignId: 2, community: "Gaming Guild", position: "Top Banner", impressions: 8_200, clicks: 410, ctr: "5.0%", status: "Ended" },
];

export default function AdsPlacementsPage() {
  const t = useTranslations("common");

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/ads"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <LayoutGrid className="h-7 w-7" />
            Ad Placements
          </h1>
          <p className="text-muted-foreground">Manage where your ads appear across communities</p>
        </div>
        <Button>
          <Plus className="mr-2 h-4 w-4" />New Placement
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Active Placements</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-green-600">3</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Impressions</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Eye className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">23.8K</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Avg. CTR</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <MousePointerClick className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">4.6%</p>
          </CardContent>
        </Card>
      </div>

      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input placeholder="Search placements..." className="pl-9" />
        </div>
        <Button variant="outline" size="sm">
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {PLACEHOLDER_PLACEMENTS.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <LayoutGrid className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No placements configured</p>
            <p className="text-sm text-muted-foreground">Create a placement to display your ads in communities</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>ID</TableHead>
                <TableHead>Campaign</TableHead>
                <TableHead>Community</TableHead>
                <TableHead>Position</TableHead>
                <TableHead className="text-right">Impressions</TableHead>
                <TableHead className="text-right">Clicks</TableHead>
                <TableHead className="text-right">CTR</TableHead>
                <TableHead>Status</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {PLACEHOLDER_PLACEMENTS.map((placement) => (
                <TableRow key={placement.id}>
                  <TableCell className="font-mono">#{placement.id}</TableCell>
                  <TableCell className="font-mono">#{placement.campaignId}</TableCell>
                  <TableCell>{placement.community}</TableCell>
                  <TableCell><Badge variant="outline">{placement.position}</Badge></TableCell>
                  <TableCell className="text-right font-mono">{placement.impressions.toLocaleString()}</TableCell>
                  <TableCell className="text-right font-mono">{placement.clicks.toLocaleString()}</TableCell>
                  <TableCell className="text-right font-mono">{placement.ctr}</TableCell>
                  <TableCell><StatusBadge status={placement.status} /></TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </div>
  );
}
