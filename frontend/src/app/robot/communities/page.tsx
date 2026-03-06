"use client";

import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/shared/StatusBadge";
import {
  Users,
  ArrowLeft,
  Plus,
  Search,
  MessageSquare,
  Settings,
  Star,
  RotateCcw,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const PLACEHOLDER_COMMUNITIES = [
  { id: 1, name: "Crypto Trading Chat", platform: "Telegram", botId: "0xab3f…c812", reputation: "Enabled", members: 256, status: "Active" },
  { id: 2, name: "NFT Collectors", platform: "Discord", botId: "0xab3f…c812", reputation: "Enabled", members: 1024, status: "Active" },
  { id: 3, name: "DeFi Strategies", platform: "Telegram", botId: "0x7e9d…f4a1", reputation: "Disabled", members: 89, status: "Active" },
  { id: 4, name: "Gaming Guild", platform: "Discord", botId: "0xab3f…c812", reputation: "Enabled", members: 512, status: "Paused" },
];

export default function RobotCommunitiesPage() {
  const t = useTranslations("common");

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/robot"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Users className="h-7 w-7" />
            Communities
          </h1>
          <p className="text-muted-foreground">Manage bot-bound communities and their configurations</p>
        </div>
        <Button>
          <Plus className="mr-2 h-4 w-4" />Bind Community
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Communities</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">4</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Active</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-green-600">3</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Reputation Enabled</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Star className="h-4 w-4 text-amber-500" />
            <p className="text-2xl font-bold">3</p>
          </CardContent>
        </Card>
      </div>

      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input placeholder="Search communities..." className="pl-9" />
        </div>
        <Button variant="outline" size="sm">
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {PLACEHOLDER_COMMUNITIES.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Users className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No communities bound</p>
            <p className="text-sm text-muted-foreground">Bind a community to a bot to start managing it on-chain</p>
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          {PLACEHOLDER_COMMUNITIES.map((community) => (
            <Card key={community.id} className="hover:border-primary/50 transition-colors">
              <CardHeader>
                <div className="flex items-center justify-between">
                  <CardTitle className="text-base">{community.name}</CardTitle>
                  <StatusBadge status={community.status} />
                </div>
                <CardDescription>Bot: {community.botId}</CardDescription>
              </CardHeader>
              <CardContent className="space-y-2">
                <div className="flex items-center gap-2 text-sm">
                  <MessageSquare className="h-4 w-4 text-muted-foreground" />
                  <span className="text-muted-foreground">Platform:</span>
                  <Badge variant="outline">{community.platform}</Badge>
                </div>
                <div className="flex items-center gap-2 text-sm">
                  <Users className="h-4 w-4 text-muted-foreground" />
                  <span className="text-muted-foreground">Members:</span>
                  <span className="font-medium">{community.members}</span>
                </div>
                <div className="flex items-center gap-2 text-sm">
                  <Star className="h-4 w-4 text-muted-foreground" />
                  <span className="text-muted-foreground">Reputation:</span>
                  <Badge variant={community.reputation === "Enabled" ? "default" : "secondary"}>
                    {community.reputation}
                  </Badge>
                </div>
                <div className="pt-2">
                  <Button variant="outline" size="sm" className="w-full">
                    <Settings className="mr-2 h-3 w-3" />Configure
                  </Button>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
