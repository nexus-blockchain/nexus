"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import {
  Network,
  ArrowLeft,
  Search,
  Shield,
  Coins,
  RotateCcw,
  Server,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const PLACEHOLDER_NODES = [
  { id: 1, account: "5GrwvaEF...RjJQTPW", stake: "100,000 NEX", status: "Active", teeVerified: true, uptime: "99.8%", lastBlock: 1_234_567 },
  { id: 2, account: "5FHneW46...8BnWJ9S", stake: "75,000 NEX", status: "Active", teeVerified: true, uptime: "99.5%", lastBlock: 1_234_566 },
  { id: 3, account: "5DAAnrj7...4dKtWZq", stake: "50,000 NEX", status: "Validating", teeVerified: true, uptime: "98.2%", lastBlock: 1_234_565 },
  { id: 4, account: "5HGjWAeF...TnZjNFP", stake: "25,000 NEX", status: "Offline", teeVerified: false, uptime: "—", lastBlock: 1_230_000 },
];

export default function RobotNodesPage() {
  const t = useTranslations("common");

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/robot"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Network className="h-7 w-7" />
            Consensus Nodes
          </h1>
          <p className="text-muted-foreground">Nodes participating in robot network consensus</p>
        </div>
        <Button variant="outline" size="sm">
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Nodes</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Server className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">4</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Active</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-green-600">3</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Staked</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Coins className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">250K NEX</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">TEE Verified</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Shield className="h-4 w-4 text-green-500" />
            <p className="text-2xl font-bold">3</p>
          </CardContent>
        </Card>
      </div>

      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input placeholder="Search nodes..." className="pl-9" />
        </div>
      </div>

      {PLACEHOLDER_NODES.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Network className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No consensus nodes</p>
            <p className="text-sm text-muted-foreground">Nodes will appear here once they join the consensus network</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Account</TableHead>
                <TableHead className="text-right">Stake</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>TEE</TableHead>
                <TableHead className="text-right">Uptime</TableHead>
                <TableHead className="text-right">Last Block</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {PLACEHOLDER_NODES.map((node) => (
                <TableRow key={node.id}>
                  <TableCell className="font-mono text-xs">{node.account}</TableCell>
                  <TableCell className="text-right font-mono">{node.stake}</TableCell>
                  <TableCell><StatusBadge status={node.status} /></TableCell>
                  <TableCell>
                    {node.teeVerified ? (
                      <Badge variant="default" className="bg-green-600">Verified</Badge>
                    ) : (
                      <Badge variant="secondary">Unverified</Badge>
                    )}
                  </TableCell>
                  <TableCell className="text-right font-mono">{node.uptime}</TableCell>
                  <TableCell className="text-right font-mono">#{node.lastBlock.toLocaleString()}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </div>
  );
}
