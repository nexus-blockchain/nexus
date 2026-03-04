"use client";

import { useState } from "react";
import { useEntityStore } from "@/stores/entity";
import { useTx } from "@/hooks/useTx";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { TxButton } from "@/components/shared/TxButton";
import { GOVERNANCE_MODES } from "@/lib/constants";
import { ArrowLeft, Settings, Lock, Shield } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function GovernanceConfigPage() {
  const { currentEntityId } = useEntityStore();
  const { submit, state: txState } = useTx();
  const tc = useTranslations("common");

  const [govMode, setGovMode] = useState("FullDAO");
  const [votingPeriod, setVotingPeriod] = useState("14400");
  const [quorum, setQuorum] = useState("5000");
  const [vetoThreshold, setVetoThreshold] = useState("3000");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const handleConfigure = () => {
    submit("entityGovernance", "configureGovernance", [
      currentEntityId, govMode, Number(votingPeriod), Number(quorum), Number(vetoThreshold),
    ]);
  };

  const handleLock = () => {
    submit("entityGovernance", "lockGovernance", [currentEntityId]);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/governance"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Governance Configuration</h1>
          <p className="text-muted-foreground">Set up DAO governance parameters</p>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Settings className="h-5 w-5" />Governance Parameters</CardTitle>
          <CardDescription>Configure voting period, quorum, and thresholds</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">Governance Mode</label>
            <Select value={govMode} onChange={(e) => setGovMode(e.target.value)}>
              {GOVERNANCE_MODES.map((m) => <option key={m} value={m}>{m}</option>)}
            </Select>
          </div>
          <div className="grid gap-4 md:grid-cols-3">
            <div className="space-y-2">
              <label className="text-sm font-medium">Voting Period (blocks)</label>
              <Input type="number" value={votingPeriod} onChange={(e) => setVotingPeriod(e.target.value)} min="1" />
              <p className="text-xs text-muted-foreground">~{Math.round(Number(votingPeriod) * 6 / 3600)} hours</p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Quorum (basis points)</label>
              <Input type="number" value={quorum} onChange={(e) => setQuorum(e.target.value)} min="0" max="10000" />
              <p className="text-xs text-muted-foreground">{Number(quorum) / 100}% participation required</p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Veto Threshold (bps)</label>
              <Input type="number" value={vetoThreshold} onChange={(e) => setVetoThreshold(e.target.value)} min="0" max="10000" />
              <p className="text-xs text-muted-foreground">{Number(vetoThreshold) / 100}% to veto</p>
            </div>
          </div>
          <TxButton onClick={handleConfigure} txStatus={txState.status}>
            Save Configuration
          </TxButton>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-destructive"><Lock className="h-5 w-5" />Lock Governance</CardTitle>
          <CardDescription>Permanently lock governance configuration. This action is irreversible.</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="rounded-lg border border-destructive/50 bg-destructive/5 p-4 mb-4">
            <p className="text-sm text-destructive">Once locked, governance configuration can never be changed again. This is permanent and irreversible.</p>
          </div>
          <TxButton variant="destructive" onClick={handleLock} txStatus={txState.status}>
            <Lock className="mr-2 h-4 w-4" />Lock Governance Forever
          </TxButton>
        </CardContent>
      </Card>

      {txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {txState.status === "error" && <p className="text-sm text-destructive">{txState.error}</p>}
    </div>
  );
}
