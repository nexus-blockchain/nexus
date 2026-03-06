"use client";

import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import {
  FileText,
  ArrowLeft,
  Upload,
  Search,
  FileImage,
  File,
  ExternalLink,
  RotateCcw,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const PLACEHOLDER_EVIDENCE = [
  { id: 1, disputeId: 1, type: "Screenshot", cid: "QmYwAPJz...nemtYgP", submitter: "5GrwvaEF...RjJQTPW", date: "2025-01-15", description: "Payment confirmation screenshot" },
  { id: 2, disputeId: 1, type: "Document", cid: "QmT5NvUt...nPbdG", submitter: "5FHneW46...8BnWJ9S", date: "2025-01-15", description: "Transaction receipt from bank" },
  { id: 3, disputeId: 2, type: "Log", cid: "QmPZ9gcC...gaTQ", submitter: "5DAAnrj7...4dKtWZq", date: "2025-01-12", description: "IPFS node logs showing pin failure" },
  { id: 4, disputeId: 3, type: "Screenshot", cid: "QmW2WQi7...dsgaTQ", submitter: "5GrwvaEF...RjJQTPW", date: "2025-01-08", description: "USDT transfer showing wrong amount" },
];

export default function DisputeEvidencePage() {
  const t = useTranslations("common");

  const typeIcon = (type: string) => {
    switch (type) {
      case "Screenshot": return <FileImage className="h-4 w-4 text-blue-500" />;
      case "Document": return <File className="h-4 w-4 text-amber-500" />;
      case "Log": return <FileText className="h-4 w-4 text-green-500" />;
      default: return <File className="h-4 w-4 text-muted-foreground" />;
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/dispute"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <FileText className="h-7 w-7" />
            Evidence
          </h1>
          <p className="text-muted-foreground">Submit and manage evidence for dispute cases</p>
        </div>
        <Button>
          <Upload className="mr-2 h-4 w-4" />Upload Evidence
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Evidence</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">4</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Disputes with Evidence</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">3</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Your Submissions</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">2</p></CardContent>
        </Card>
      </div>

      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input placeholder="Search by dispute ID or CID..." className="pl-9" />
        </div>
        <Button variant="outline" size="sm">
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {PLACEHOLDER_EVIDENCE.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <FileText className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No evidence submitted</p>
            <p className="text-sm text-muted-foreground">Upload evidence to support your dispute case</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>ID</TableHead>
                <TableHead>Dispute</TableHead>
                <TableHead>Type</TableHead>
                <TableHead>Description</TableHead>
                <TableHead>CID</TableHead>
                <TableHead>Submitter</TableHead>
                <TableHead>Date</TableHead>
                <TableHead className="text-right">View</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {PLACEHOLDER_EVIDENCE.map((evidence) => (
                <TableRow key={evidence.id}>
                  <TableCell className="font-mono">#{evidence.id}</TableCell>
                  <TableCell className="font-mono">#{evidence.disputeId}</TableCell>
                  <TableCell>
                    <div className="flex items-center gap-1.5">
                      {typeIcon(evidence.type)}
                      <span className="text-sm">{evidence.type}</span>
                    </div>
                  </TableCell>
                  <TableCell className="max-w-[200px] truncate">{evidence.description}</TableCell>
                  <TableCell className="font-mono text-xs">{evidence.cid}</TableCell>
                  <TableCell className="font-mono text-xs">{evidence.submitter}</TableCell>
                  <TableCell className="text-muted-foreground">{evidence.date}</TableCell>
                  <TableCell className="text-right">
                    <Button variant="ghost" size="sm">
                      <ExternalLink className="h-3 w-3" />
                    </Button>
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
