"use client";

import { useState, useEffect } from "react";
import { useEntityStore } from "@/stores/entity";
import { useEntity, useEntityActions } from "@/hooks/useEntity";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { TxButton } from "@/components/shared/TxButton";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { Settings, Save, Upload, ArrowUpRight } from "lucide-react";

export default function EntitySettingsPage() {
  const { currentEntityId } = useEntityStore();
  const { data: entity, isLoading, refetch } = useEntity(currentEntityId);
  const actions = useEntityActions(currentEntityId || 0);

  const [name, setName] = useState("");
  const [logoCid, setLogoCid] = useState("");
  const [descriptionCid, setDescriptionCid] = useState("");
  const [metadataUri, setMetadataUri] = useState("");

  useEffect(() => {
    if (entity) {
      setName(entity.name);
      setLogoCid(entity.logoCid || "");
      setDescriptionCid(entity.descriptionCid || "");
      setMetadataUri(entity.metadataUri || "");
    }
  }, [entity]);

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">Select an entity first</div>;
  }

  if (isLoading) {
    return <div className="flex h-full items-center justify-center"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>;
  }

  if (!entity) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">Entity not found</div>;
  }

  const handleSave = () => {
    actions.updateEntity({
      name: name !== entity.name ? name : undefined,
      logoCid: logoCid !== (entity.logoCid || "") ? logoCid || null : undefined,
      descriptionCid: descriptionCid !== (entity.descriptionCid || "") ? descriptionCid || null : undefined,
      metadataUri: metadataUri !== (entity.metadataUri || "") ? metadataUri || null : undefined,
    });
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Entity Settings</h1>
        <p className="text-muted-foreground">Configure your entity details and metadata</p>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Settings className="h-5 w-5" />Basic Information</CardTitle>
            <CardDescription>Core entity details stored on-chain</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Entity ID</label>
              <Input value={String(entity.id)} disabled />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Owner</label>
              <div className="flex items-center gap-2 rounded-md border p-2">
                <AddressDisplay address={entity.owner} />
              </div>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Status</label>
              <div><StatusBadge status={entity.status} /></div>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Type</label>
              <Input value={entity.entityType} disabled />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Governance Mode</label>
              <Input value={entity.governanceMode} disabled />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Verified</label>
              <div><StatusBadge status={entity.verified ? "Verified" : "Unverified"} /></div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Upload className="h-5 w-5" />Editable Fields</CardTitle>
            <CardDescription>Update entity name and IPFS content references</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Name</label>
              <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="Entity name" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Logo CID</label>
              <Input value={logoCid} onChange={(e) => setLogoCid(e.target.value)} placeholder="IPFS CID for logo" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Description CID</label>
              <Input value={descriptionCid} onChange={(e) => setDescriptionCid(e.target.value)} placeholder="IPFS CID for description" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Metadata URI</label>
              <Input value={metadataUri} onChange={(e) => setMetadataUri(e.target.value)} placeholder="https://..." />
            </div>

            <div className="flex gap-2 pt-4">
              <TxButton onClick={handleSave} txStatus={actions.txState.status}>
                <Save className="mr-2 h-4 w-4" />Save Changes
              </TxButton>
            </div>

            {actions.txState.status === "finalized" && (
              <p className="text-sm text-green-600">Changes saved successfully!</p>
            )}
            {actions.txState.status === "error" && (
              <p className="text-sm text-destructive">{actions.txState.error}</p>
            )}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Advanced Actions</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-wrap gap-3">
          <Button variant="outline" onClick={() => actions.reopenEntity()}>Reopen Entity</Button>
          <Button variant="outline" onClick={() => actions.upgradeType("Enterprise")}>
            <ArrowUpRight className="mr-2 h-4 w-4" />Upgrade Type
          </Button>
          <Button variant="destructive" onClick={() => actions.requestClose()}>Request Close</Button>
        </CardContent>
      </Card>
    </div>
  );
}
