"use client";

import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import {
  HardDrive,
  Plus,
  Search,
  Database,
  CheckCircle2,
  AlertCircle,
  Clock,
  RotateCcw,
} from "lucide-react";
import { useTranslations } from "next-intl";

const PLACEHOLDER_PINS = [
  { cid: "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG", state: "Pinned", tier: "Standard", size: "2.4 MiB", replicas: 3 },
  { cid: "QmT5NvUtoM5nWFfrQdVrFtvGfKFmG7AHE8P34isapyhCxX", state: "Pinning", tier: "Premium", size: "15.7 MiB", replicas: 1 },
  { cid: "QmPZ9gcCEpqKTo6aq61g2nXGUhM4iCL3ewB6LDXZCtioEB", state: "Requested", tier: "Standard", size: "512 KiB", replicas: 0 },
  { cid: "QmW2WQi7j6c7UgJTarActp7tDNikE4B2qXtFCfLPdsgaTQ", state: "Degraded", tier: "Standard", size: "1.1 MiB", replicas: 1 },
  { cid: "QmNZiPk974vDsPmQii3YbrMKfi12KTSNM7XMiYyiea4VYZ", state: "Failed", tier: "Standard", size: "45.2 MiB", replicas: 0 },
];

export default function StoragePage() {
  const t = useTranslations("common");

  const stateIcon = (state: string) => {
    switch (state) {
      case "Pinned": return <CheckCircle2 className="h-4 w-4 text-green-500" />;
      case "Pinning": return <Clock className="h-4 w-4 text-blue-500 animate-spin" />;
      case "Requested": return <Clock className="h-4 w-4 text-amber-500" />;
      case "Degraded": return <AlertCircle className="h-4 w-4 text-amber-500" />;
      case "Failed": return <AlertCircle className="h-4 w-4 text-red-500" />;
      default: return null;
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <HardDrive className="h-8 w-8" />
            IPFS Storage
          </h1>
          <p className="text-muted-foreground">Manage your pinned content on the decentralized storage network</p>
        </div>
        <Button>
          <Plus className="mr-2 h-4 w-4" />Request Pin
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Pins</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Database className="h-4 w-4 text-muted-foreground" />
            <p className="text-2xl font-bold">5</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Active</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-green-600">1</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Pending</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-amber-600">2</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Storage Used</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">64.9 MiB</p></CardContent>
        </Card>
      </div>

      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input placeholder="Search by CID..." className="pl-9" />
        </div>
        <Button variant="outline" size="sm">
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {PLACEHOLDER_PINS.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <HardDrive className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No pins yet</p>
            <p className="text-sm text-muted-foreground">Request a pin to start storing content on IPFS</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>CID</TableHead>
                <TableHead>State</TableHead>
                <TableHead>Tier</TableHead>
                <TableHead className="text-right">Size</TableHead>
                <TableHead className="text-right">Replicas</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {PLACEHOLDER_PINS.map((pin) => (
                <TableRow key={pin.cid}>
                  <TableCell className="font-mono text-xs max-w-[200px] truncate">{pin.cid}</TableCell>
                  <TableCell>
                    <div className="flex items-center gap-1.5">
                      {stateIcon(pin.state)}
                      <StatusBadge status={pin.state} />
                    </div>
                  </TableCell>
                  <TableCell><Badge variant="outline">{pin.tier}</Badge></TableCell>
                  <TableCell className="text-right font-mono">{pin.size}</TableCell>
                  <TableCell className="text-right font-mono">{pin.replicas}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </div>
  );
}
