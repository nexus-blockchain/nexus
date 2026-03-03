"use client";

import { useEntityStore } from "@/stores/entity";
import { useKycRecords, useKycActions } from "@/hooks/useKyc";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { Badge } from "@/components/ui/badge";
import { ShieldCheck, Clock, CheckCircle, XCircle } from "lucide-react";

export default function KycPage() {
  const { currentEntityId } = useEntityStore();
  const { records, isLoading } = useKycRecords(currentEntityId);
  const actions = useKycActions();

  if (!currentEntityId) return <div className="flex h-full items-center justify-center text-muted-foreground">Select an entity first</div>;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">KYC Management</h1>
        <p className="text-muted-foreground">Identity verification records and compliance</p>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Records</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{records.length}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Pending Review</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{records.filter((r: any) => r.status === "Pending").length}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Approved</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{records.filter((r: any) => r.status === "Approved").length}</p></CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><ShieldCheck className="h-5 w-5" />KYC Records</CardTitle>
          <CardDescription>Review and manage identity verification submissions</CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="flex justify-center py-8"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
          ) : records.length === 0 ? (
            <p className="text-sm text-muted-foreground">No KYC records submitted.</p>
          ) : (
            <div className="space-y-3">
              {records.map((record: any, i: number) => (
                <div key={i} className="flex items-center gap-4 rounded-lg border p-4">
                  <AddressDisplay address={record.account || record.who} />
                  <Badge variant="outline">{record.level || "Basic"}</Badge>
                  <StatusBadge status={record.status} />
                  <span className="text-xs text-muted-foreground">
                    <Clock className="mr-1 inline h-3 w-3" />
                    {record.submittedAt ? `Block #${record.submittedAt}` : "—"}
                  </span>
                  {record.status === "Pending" && (
                    <div className="ml-auto flex gap-2">
                      <Button size="sm" onClick={() => actions.approveKyc(record.account || record.who, record.level || "Basic", 0, 0)}>
                        <CheckCircle className="mr-1 h-3 w-3" />Approve
                      </Button>
                      <Button size="sm" variant="destructive" onClick={() => actions.rejectKyc(record.account || record.who, "Rejected by admin")}>
                        <XCircle className="mr-1 h-3 w-3" />Reject
                      </Button>
                    </div>
                  )}
                  {record.status === "Approved" && (
                    <div className="ml-auto">
                      <Button size="sm" variant="outline" onClick={() => actions.revokeKyc(record.account || record.who, "Revoked by admin")}>Revoke</Button>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
