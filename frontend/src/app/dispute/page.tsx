"use client";

import { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import {
  Scale,
  Plus,
  Search,
  AlertTriangle,
  Clock,
  CheckCircle2,
  RotateCcw,
  Filter,
} from "lucide-react";
import { useTranslations } from "next-intl";

const PLACEHOLDER_COMPLAINTS = [
  { id: 1, type: "TradeDispute", respondent: "5GrwvaEF...RjJQTPW", status: "Open", date: "2025-01-15", description: "Seller did not release tokens after payment" },
  { id: 2, type: "ServiceComplaint", respondent: "5FHneW46...8BnWJ9S", status: "UnderReview", date: "2025-01-12", description: "Storage operator lost pinned data" },
  { id: 3, type: "TradeDispute", respondent: "5DAAnrj7...4dKtWZq", status: "Resolved", date: "2025-01-08", description: "Incorrect USDT amount received" },
  { id: 4, type: "BotAbuse", respondent: "5HGjWAeF...TnZjNFP", status: "Dismissed", date: "2025-01-05", description: "Bot spamming community chat" },
];

const STATUS_FILTERS = ["All", "Open", "UnderReview", "Resolved", "Dismissed"];

export default function DisputePage() {
  const t = useTranslations("common");
  const [statusFilter, setStatusFilter] = useState("All");

  const filtered = statusFilter === "All"
    ? PLACEHOLDER_COMPLAINTS
    : PLACEHOLDER_COMPLAINTS.filter((c) => c.status === statusFilter);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Scale className="h-8 w-8" />
            Dispute Resolution
          </h1>
          <p className="text-muted-foreground">File and track complaints through on-chain arbitration</p>
        </div>
        <Button>
          <Plus className="mr-2 h-4 w-4" />File Complaint
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Open</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <AlertTriangle className="h-4 w-4 text-red-500" />
            <p className="text-2xl font-bold text-red-600">1</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Under Review</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <Clock className="h-4 w-4 text-amber-500" />
            <p className="text-2xl font-bold text-amber-600">1</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Resolved</CardTitle></CardHeader>
          <CardContent className="flex items-center gap-2">
            <CheckCircle2 className="h-4 w-4 text-green-500" />
            <p className="text-2xl font-bold text-green-600">1</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Dismissed</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">1</p></CardContent>
        </Card>
      </div>

      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input placeholder="Search complaints..." className="pl-9" />
        </div>
        <div className="flex items-center gap-1">
          <Filter className="h-4 w-4 text-muted-foreground" />
          {STATUS_FILTERS.map((s) => (
            <Button
              key={s}
              variant={statusFilter === s ? "default" : "outline"}
              size="sm"
              onClick={() => setStatusFilter(s)}
            >
              {s}
            </Button>
          ))}
        </div>
        <Button variant="outline" size="sm">
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {filtered.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Scale className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No complaints found</p>
            <p className="text-sm text-muted-foreground">Complaints matching your filter will appear here</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>ID</TableHead>
                <TableHead>Type</TableHead>
                <TableHead>Respondent</TableHead>
                <TableHead>Description</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Date</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {filtered.map((complaint) => (
                <TableRow key={complaint.id}>
                  <TableCell className="font-mono">#{complaint.id}</TableCell>
                  <TableCell><Badge variant="outline">{complaint.type}</Badge></TableCell>
                  <TableCell className="font-mono text-xs">{complaint.respondent}</TableCell>
                  <TableCell className="max-w-[250px] truncate">{complaint.description}</TableCell>
                  <TableCell><StatusBadge status={complaint.status} /></TableCell>
                  <TableCell className="text-muted-foreground">{complaint.date}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </div>
  );
}
