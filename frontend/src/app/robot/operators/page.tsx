"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { Cpu, Search, ArrowLeft, RotateCcw, Users, Shield } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const PLACEHOLDER_OPERATORS = [
  { account: "5GrwvaEF...RjJQTPW", botsManaged: 5, status: "Active", bond: "25,000 NEX", teeVerified: true },
  { account: "5FHneW46...8BnWJ9S", botsManaged: 2, status: "Active", bond: "10,000 NEX", teeVerified: true },
  { account: "5DAAnrj7...4dKtWZq", botsManaged: 0, status: "Inactive", bond: "5,000 NEX", teeVerified: false },
];

export default function RobotOperatorsPage() {
  const t = useTranslations("common");

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/robot"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Cpu className="h-7 w-7" />
            Robot Operators
          </h1>
          <p className="text-muted-foreground">Registered operators running group robot infrastructure</p>
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
            <p className="text-2xl font-bold">3</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Active</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold text-green-600">2</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">TEE Verified</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Shield className="h-4 w-4 text-green-500" />
            <p className="text-2xl font-bold">2</p>
          </CardContent>
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
            <Cpu className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No operators registered</p>
            <p className="text-sm text-muted-foreground">Robot operators will appear here once they register</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Account</TableHead>
                <TableHead className="text-right">Bots Managed</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Bond</TableHead>
                <TableHead>TEE Verified</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {PLACEHOLDER_OPERATORS.map((op) => (
                <TableRow key={op.account}>
                  <TableCell className="font-mono text-xs">{op.account}</TableCell>
                  <TableCell className="text-right font-mono">{op.botsManaged}</TableCell>
                  <TableCell><StatusBadge status={op.status} /></TableCell>
                  <TableCell className="text-right font-mono">{op.bond}</TableCell>
                  <TableCell>
                    {op.teeVerified ? (
                      <Badge variant="default" className="bg-green-600">Verified</Badge>
                    ) : (
                      <Badge variant="secondary">Unverified</Badge>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </div>
  );
}
