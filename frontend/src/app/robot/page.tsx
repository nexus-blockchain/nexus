"use client";

import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/shared/StatusBadge";
import {
  Bot,
  Plus,
  Search,
  Shield,
  Users,
  Radio,
  RotateCcw,
} from "lucide-react";
import { useTranslations } from "next-intl";

const PLACEHOLDER_BOTS = [
  { id: "0xab3f…c812", status: "Active", teeType: "SGX", communities: 3, peers: 12 },
  { id: "0x7e9d…f4a1", status: "Active", teeType: "TDX", communities: 1, peers: 5 },
  { id: "0x12cd…8b37", status: "Inactive", teeType: "SGX", communities: 0, peers: 0 },
];

export default function RobotPage() {
  const t = useTranslations("common");

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Bot className="h-8 w-8" />
            Group Robots
          </h1>
          <p className="text-muted-foreground">Manage your registered bots and their community bindings</p>
        </div>
        <Button>
          <Plus className="mr-2 h-4 w-4" />Register Bot
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Bots</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">3</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Active</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-green-600">2</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Communities</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">4</p></CardContent>
        </Card>
      </div>

      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input placeholder="Search bots..." className="pl-9" />
        </div>
        <Button variant="outline" size="sm">
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {PLACEHOLDER_BOTS.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Bot className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No bots registered</p>
            <p className="text-sm text-muted-foreground">Register your first bot to get started with group automation</p>
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {PLACEHOLDER_BOTS.map((bot) => (
            <Card key={bot.id} className="hover:border-primary/50 transition-colors">
              <CardHeader>
                <div className="flex items-center justify-between">
                  <CardTitle className="font-mono text-sm">{bot.id}</CardTitle>
                  <StatusBadge status={bot.status} />
                </div>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="flex items-center gap-2 text-sm">
                  <Shield className="h-4 w-4 text-muted-foreground" />
                  <span className="text-muted-foreground">TEE:</span>
                  <Badge variant="outline">{bot.teeType}</Badge>
                </div>
                <div className="flex items-center gap-2 text-sm">
                  <Users className="h-4 w-4 text-muted-foreground" />
                  <span className="text-muted-foreground">Communities:</span>
                  <span className="font-medium">{bot.communities}</span>
                </div>
                <div className="flex items-center gap-2 text-sm">
                  <Radio className="h-4 w-4 text-muted-foreground" />
                  <span className="text-muted-foreground">Connected Peers:</span>
                  <span className="font-medium">{bot.peers}</span>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
