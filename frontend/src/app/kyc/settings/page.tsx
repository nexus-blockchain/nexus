"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useTx } from "@/hooks/useTx";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { TxButton } from "@/components/shared/TxButton";
import { KYC_LEVELS } from "@/lib/constants";
import { ArrowLeft, Shield, Settings } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function KycSettingsPage() {
  const { currentEntityId } = useEntityStore();
  const { submit, state: txState } = useTx();
  const tc = useTranslations("common");

  const [required, setRequired] = useState(false);
  const [minLevel, setMinLevel] = useState("1");
  const [autoApprove, setAutoApprove] = useState(false);
  const [maxPending, setMaxPending] = useState("100");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const handleSave = () => {
    submit("entityKyc", "configureKyc", [
      currentEntityId, required, Number(minLevel), autoApprove, Number(maxPending),
    ]);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/kyc"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">KYC Settings</h1>
          <p className="text-muted-foreground">Configure identity verification requirements</p>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Shield className="h-5 w-5" />KYC Requirements</CardTitle>
          <CardDescription>Set whether KYC is required and at what level</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between rounded-lg border p-4">
            <div>
              <p className="text-sm font-medium">KYC Required</p>
              <p className="text-xs text-muted-foreground">Require KYC verification for entity operations</p>
            </div>
            <Switch checked={required} onCheckedChange={setRequired} />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Minimum KYC Level</label>
            <Select value={minLevel} onChange={(e) => setMinLevel(e.target.value)}>
              {KYC_LEVELS.map((level) => (
                <option key={level} value={level}>{level}</option>
              ))}
            </Select>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Settings className="h-5 w-5" />Processing Settings</CardTitle>
          <CardDescription>Control how KYC submissions are processed</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between rounded-lg border p-4">
            <div>
              <p className="text-sm font-medium">Auto-Approve</p>
              <p className="text-xs text-muted-foreground">Automatically approve KYC submissions from trusted providers</p>
            </div>
            <Switch checked={autoApprove} onCheckedChange={setAutoApprove} />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Max Pending Submissions</label>
            <Input type="number" value={maxPending} onChange={(e) => setMaxPending(e.target.value)} min="1" />
            <p className="text-xs text-muted-foreground">Maximum pending KYC submissions before new ones are rejected</p>
          </div>
          <TxButton onClick={handleSave} txStatus={txState.status}>
            Save KYC Settings
          </TxButton>
        </CardContent>
      </Card>

      {txState.status === "finalized" && <p className="text-sm text-green-600">Settings saved!</p>}
      {txState.status === "error" && <p className="text-sm text-destructive">{txState.error}</p>}
    </div>
  );
}
