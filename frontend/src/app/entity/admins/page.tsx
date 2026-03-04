"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useEntity, useEntityActions } from "@/hooks/useEntity";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { TxButton } from "@/components/shared/TxButton";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { Badge } from "@/components/ui/badge";
import { PERMISSION_LABELS } from "@/lib/constants";
import { UserPlus, Trash2, Shield } from "lucide-react";
import { useTranslations } from "next-intl";

export default function EntityAdminsPage() {
  const { currentEntityId } = useEntityStore();
  const { data: entity, isLoading, refetch } = useEntity(currentEntityId);
  const actions = useEntityActions(currentEntityId || 0);
  const [newAdmin, setNewAdmin] = useState("");
  const [newPerms, setNewPerms] = useState(0);
  const t = useTranslations("entity.admins");
  const tc = useTranslations("common");
  const [transferTo, setTransferTo] = useState("");

  if (!currentEntityId || isLoading) {
    return <div className="flex h-full items-center justify-center"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>;
  }
  if (!entity) return <div className="flex h-full items-center justify-center text-muted-foreground">Entity not found</div>;

  const handleAddAdmin = () => {
    if (newAdmin) {
      actions.addAdmin(newAdmin, newPerms);
      setNewAdmin("");
      setNewPerms(0);
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Admin Management</h1>
        <p className="text-muted-foreground">Manage entity administrators and their permissions</p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Shield className="h-5 w-5" />Owner</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center gap-4">
            <AddressDisplay address={entity.owner} />
            <div className="ml-auto flex items-center gap-2">
              <Input value={transferTo} onChange={(e) => setTransferTo(e.target.value)} placeholder="New owner address" className="w-64" />
              <TxButton variant="outline" onClick={() => { if (transferTo) actions.transferOwnership(transferTo); }} txStatus={actions.txState.status}>
                Transfer Ownership
              </TxButton>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><UserPlus className="h-5 w-5" />Add Admin</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-end gap-4">
            <div className="flex-1 space-y-2">
              <label className="text-sm font-medium">Address</label>
              <Input value={newAdmin} onChange={(e) => setNewAdmin(e.target.value)} placeholder="5xxx..." />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Permissions (bitmask)</label>
              <Input type="number" value={newPerms} onChange={(e) => setNewPerms(Number(e.target.value))} className="w-40" />
            </div>
            <TxButton onClick={handleAddAdmin} txStatus={actions.txState.status}>
              <UserPlus className="mr-2 h-4 w-4" />Add
            </TxButton>
          </div>
          <div className="mt-3 flex flex-wrap gap-2">
            {Object.entries(PERMISSION_LABELS).map(([bit, label]) => (
              <button
                key={bit}
                onClick={() => setNewPerms((prev) => prev ^ Number(bit))}
                className={`rounded-full border px-2.5 py-0.5 text-xs transition-colors ${
                  (newPerms & Number(bit)) ? "bg-primary text-primary-foreground" : "bg-background text-foreground hover:bg-accent"
                }`}
              >
                {label}
              </button>
            ))}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Current Admins ({entity.admins.length})</CardTitle>
        </CardHeader>
        <CardContent>
          {entity.admins.length === 0 ? (
            <p className="text-sm text-muted-foreground">No admins configured</p>
          ) : (
            <div className="space-y-4">
              {entity.admins.map((admin) => (
                <div key={admin.address} className="flex items-center gap-4 rounded-lg border p-4">
                  <AddressDisplay address={admin.address} />
                  <div className="flex flex-wrap gap-1">
                    {Object.entries(PERMISSION_LABELS)
                      .filter(([bit]) => admin.permissions & Number(bit))
                      .map(([bit, label]) => (
                        <Badge key={bit} variant="secondary" className="text-xs">{label}</Badge>
                      ))}
                  </div>
                  <div className="ml-auto flex gap-2">
                    <Button variant="ghost" size="icon" onClick={() => actions.removeAdmin(admin.address)} title="Remove admin">
                      <Trash2 className="h-4 w-4 text-destructive" />
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Action completed! Refresh to see changes.</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
