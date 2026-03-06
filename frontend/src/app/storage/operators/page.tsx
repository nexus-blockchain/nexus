"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { Server, Search, ArrowLeft, RotateCcw, Users } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const PLACEHOLDER_OPERATORS = [
  { account: "5GrwvaEF...RjJQTPW", peerId: "12D3KooW...abc", capacity: "100 GiB", used: "42 GiB", status: "Active", layer: "L1", bond: "10,000 NEX" },
  { account: "5FHneW46...8BnWJ9S", peerId: "12D3KooW...def", capacity: "500 GiB", used: "310 GiB", status: "Active", layer: "L2", bond: "50,000 NEX" },
  { account: "5DAAnrj7...4dKtWZq", peerId: "12D3KooW...ghi", capacity: "50 GiB", used: "48 GiB", status: "NearCapacity", layer: "L1", bond: "5,000 NEX" },
  { account: "5HGjWAeF...TnZjNFP", peerId: "12D3KooW...jkl", capacity: "200 GiB", used: "0 GiB", status: "Offline", layer: "L2", bond: "20,000 NEX" },
];

export default function StorageOperatorsPage() {
  const t = useTranslations("common");

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/storage"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Server className="h-7 w-7" />
            Storage Operators
          </h1>
          <p className="text-muted-foreground">Active storage nodes providing IPFS pinning services</p>
        </div>
        <Button variant="outline" size="sm">
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Operators</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Users className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">4</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Online</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-green-600">3</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Capacity</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">850 GiB</p></CardContent>
        </Card>
      </div>

      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input placeholder="Search operators..." className="pl-9" />
        </div>
      </div>

      {PLACEHOLDER_OPERATORS.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Server className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No operators registered</p>
            <p className="text-sm text-muted-foreground">Storage operators will appear here once they register</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Account</TableHead>
                <TableHead>Peer ID</TableHead>
                <TableHead className="text-right">Capacity</TableHead>
                <TableHead className="text-right">Used</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Layer</TableHead>
                <TableHead className="text-right">Bond</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {PLACEHOLDER_OPERATORS.map((op) => (
                <TableRow key={op.account}>
                  <TableCell className="font-mono text-xs">{op.account}</TableCell>
                  <TableCell className="font-mono text-xs">{op.peerId}</TableCell>
                  <TableCell className="text-right font-mono">{op.capacity}</TableCell>
                  <TableCell className="text-right font-mono">{op.used}</TableCell>
                  <TableCell><StatusBadge status={op.status} /></TableCell>
                  <TableCell><Badge variant="outline">{op.layer}</Badge></TableCell>
                  <TableCell className="text-right font-mono">{op.bond}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </div>
  );
}
