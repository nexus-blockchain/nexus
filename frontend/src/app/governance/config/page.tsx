"use client";

import { useState, useEffect } from "react";
import { useEntityStore } from "@/stores/entity";
import { useGovernanceConfig, useGovernanceActions } from "@/hooks/useGovernance";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { Separator } from "@/components/ui/separator";
import { TxButton } from "@/components/shared/TxButton";
import { GOVERNANCE_MODES } from "@/lib/constants";
import {
  ArrowLeft, Settings, Lock, Shield, Pause, Play, Users,
  RotateCcw, AlertTriangle, CheckCircle,
} from "lucide-react";
import Link from "next/link";

export default function GovernanceConfigPage() {
  const { currentEntityId } = useEntityStore();
  const { config, locked, paused, isLoading, refetch } = useGovernanceConfig(currentEntityId);
  const actions = useGovernanceActions();

  const [mode, setMode] = useState("FullDAO");
  const [votingPeriod, setVotingPeriod] = useState("14400");
  const [executionDelay, setExecutionDelay] = useState("100");
  const [quorumThreshold, setQuorumThreshold] = useState("10");
  const [passThreshold, setPassThreshold] = useState("50");
  const [proposalThreshold, setProposalThreshold] = useState("100");
  const [adminVetoEnabled, setAdminVetoEnabled] = useState(false);
  const [delegateAddress, setDelegateAddress] = useState("");

  useEffect(() => {
    if (config) {
      setMode(config.mode);
      setVotingPeriod(String(config.votingPeriod));
      setExecutionDelay(String(config.executionDelay));
      setQuorumThreshold(String(config.quorumThreshold));
      setPassThreshold(String(config.passThreshold));
      setProposalThreshold(String(config.proposalThreshold));
      setAdminVetoEnabled(config.adminVetoEnabled);
    }
  }, [config]);

  if (!currentEntityId) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        Select an entity to configure governance
      </div>
    );
  }

  const handleSave = () => {
    actions.configureGovernance(
      currentEntityId,
      mode,
      Number(votingPeriod),
      Number(executionDelay),
      Number(quorumThreshold),
      Number(passThreshold),
      Number(proposalThreshold),
      adminVetoEnabled,
    );
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/governance">
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">Governance Configuration</h1>
          <p className="text-muted-foreground">Manage DAO governance parameters and controls</p>
        </div>
        <div className="flex gap-2">
          {locked && (
            <Badge variant="destructive">
              <Lock className="mr-1 h-3 w-3" />Locked
            </Badge>
          )}
          {paused && (
            <Badge variant="secondary">
              <Pause className="mr-1 h-3 w-3" />Paused
            </Badge>
          )}
          {!locked && !paused && (
            <Badge variant="outline">
              <CheckCircle className="mr-1 h-3 w-3" />Active
            </Badge>
          )}
        </div>
        <Button variant="outline" size="sm" onClick={refetch}>
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
        </div>
      ) : (
        <>
          {config && (
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Shield className="h-5 w-5" />Current Configuration
                </CardTitle>
              </CardHeader>
              <CardContent>
                <div className="grid gap-4 md:grid-cols-3 lg:grid-cols-4">
                  <div className="rounded-lg border p-3">
                    <p className="text-xs text-muted-foreground">Mode</p>
                    <p className="text-lg font-semibold">{config.mode}</p>
                  </div>
                  <div className="rounded-lg border p-3">
                    <p className="text-xs text-muted-foreground">Voting Period</p>
                    <p className="text-lg font-semibold">
                      {config.votingPeriod.toLocaleString()} blocks
                    </p>
                    <p className="text-xs text-muted-foreground">
                      ~{Math.round((config.votingPeriod * 6) / 3600)} hours
                    </p>
                  </div>
                  <div className="rounded-lg border p-3">
                    <p className="text-xs text-muted-foreground">Execution Delay</p>
                    <p className="text-lg font-semibold">
                      {config.executionDelay.toLocaleString()} blocks
                    </p>
                  </div>
                  <div className="rounded-lg border p-3">
                    <p className="text-xs text-muted-foreground">Quorum Threshold</p>
                    <p className="text-lg font-semibold">{config.quorumThreshold}%</p>
                  </div>
                  <div className="rounded-lg border p-3">
                    <p className="text-xs text-muted-foreground">Pass Threshold</p>
                    <p className="text-lg font-semibold">{config.passThreshold}%</p>
                  </div>
                  <div className="rounded-lg border p-3">
                    <p className="text-xs text-muted-foreground">Proposal Threshold</p>
                    <p className="text-lg font-semibold">{config.proposalThreshold}</p>
                  </div>
                  <div className="rounded-lg border p-3">
                    <p className="text-xs text-muted-foreground">Admin Veto</p>
                    <p className="text-lg font-semibold">
                      {config.adminVetoEnabled ? "Enabled" : "Disabled"}
                    </p>
                  </div>
                </div>
              </CardContent>
            </Card>
          )}

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Settings className="h-5 w-5" />Update Configuration
              </CardTitle>
              <CardDescription>
                Modify governance parameters. All fields will be submitted together.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Governance Mode</label>
                <div className="flex gap-2">
                  {GOVERNANCE_MODES.map((m) => (
                    <Button
                      key={m}
                      variant={mode === m ? "default" : "outline"}
                      size="sm"
                      onClick={() => setMode(m)}
                    >
                      {m}
                    </Button>
                  ))}
                </div>
              </div>

              <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Voting Period (blocks)</label>
                  <Input
                    type="number"
                    value={votingPeriod}
                    onChange={(e) => setVotingPeriod(e.target.value)}
                    min="1"
                  />
                  <p className="text-xs text-muted-foreground">
                    ~{Math.round((Number(votingPeriod) * 6) / 3600)} hours
                  </p>
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Execution Delay (blocks)</label>
                  <Input
                    type="number"
                    value={executionDelay}
                    onChange={(e) => setExecutionDelay(e.target.value)}
                    min="0"
                  />
                  <p className="text-xs text-muted-foreground">
                    Delay after passing before execution
                  </p>
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Quorum Threshold (1-100)</label>
                  <Input
                    type="number"
                    value={quorumThreshold}
                    onChange={(e) => setQuorumThreshold(e.target.value)}
                    min="1"
                    max="100"
                  />
                  <p className="text-xs text-muted-foreground">
                    {quorumThreshold}% participation required
                  </p>
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Pass Threshold (1-100)</label>
                  <Input
                    type="number"
                    value={passThreshold}
                    onChange={(e) => setPassThreshold(e.target.value)}
                    min="1"
                    max="100"
                  />
                  <p className="text-xs text-muted-foreground">
                    {passThreshold}% yes votes to pass
                  </p>
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Proposal Threshold</label>
                  <Input
                    type="number"
                    value={proposalThreshold}
                    onChange={(e) => setProposalThreshold(e.target.value)}
                    min="0"
                    max="65535"
                  />
                  <p className="text-xs text-muted-foreground">
                    Minimum token balance to create proposals (u16)
                  </p>
                </div>
                <div className="flex items-center gap-3 rounded-lg border p-3">
                  <Switch checked={adminVetoEnabled} onCheckedChange={setAdminVetoEnabled} />
                  <div>
                    <p className="text-sm font-medium">Admin Veto</p>
                    <p className="text-xs text-muted-foreground">
                      Allow admins to veto proposals
                    </p>
                  </div>
                </div>
              </div>

              <TxButton
                onClick={handleSave}
                txStatus={actions.txState.status}
                disabled={locked}
              >
                <Settings className="mr-2 h-4 w-4" />Save Configuration
              </TxButton>
              {locked && (
                <p className="text-xs text-destructive">
                  Governance is locked. Configuration cannot be changed.
                </p>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-destructive">
                <Lock className="h-5 w-5" />Lock Governance
              </CardTitle>
              <CardDescription>
                Permanently lock governance configuration. This action is irreversible.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div className="mb-4 rounded-lg border border-destructive/50 bg-destructive/5 p-4">
                <div className="flex items-start gap-3">
                  <AlertTriangle className="mt-0.5 h-5 w-5 text-destructive" />
                  <div>
                    <p className="text-sm font-medium text-destructive">Irreversible Action</p>
                    <p className="text-sm text-destructive/80">
                      Once locked, governance configuration can never be changed again. The current
                      mode, thresholds, and all parameters will be permanently fixed.
                    </p>
                  </div>
                </div>
              </div>
              <TxButton
                variant="destructive"
                onClick={() => actions.lockGovernance(currentEntityId)}
                txStatus={actions.txState.status}
                disabled={locked}
              >
                <Lock className="mr-2 h-4 w-4" />
                {locked ? "Already Locked" : "Lock Governance Forever"}
              </TxButton>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                {paused ? <Play className="h-5 w-5" /> : <Pause className="h-5 w-5" />}
                Pause / Resume Governance
              </CardTitle>
              <CardDescription>
                {paused
                  ? "Governance is currently paused. Resume to allow new proposals and voting."
                  : "Pause governance to temporarily halt proposals and voting."}
              </CardDescription>
            </CardHeader>
            <CardContent className="flex gap-3">
              {paused ? (
                <TxButton
                  onClick={() => actions.resumeGovernance(currentEntityId)}
                  txStatus={actions.txState.status}
                >
                  <Play className="mr-2 h-4 w-4" />Resume Governance
                </TxButton>
              ) : (
                <TxButton
                  variant="secondary"
                  onClick={() => actions.pauseGovernance(currentEntityId)}
                  txStatus={actions.txState.status}
                >
                  <Pause className="mr-2 h-4 w-4" />Pause Governance
                </TxButton>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Users className="h-5 w-5" />Vote Delegation
              </CardTitle>
              <CardDescription>Delegate your voting power to another address</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Delegate Address</label>
                <Input
                  value={delegateAddress}
                  onChange={(e) => setDelegateAddress(e.target.value)}
                  placeholder="5Grw..."
                />
              </div>
              <div className="flex gap-3">
                <TxButton
                  onClick={() =>
                    delegateAddress &&
                    actions.delegateVote(currentEntityId, delegateAddress)
                  }
                  txStatus={actions.txState.status}
                  disabled={!delegateAddress}
                >
                  Delegate Vote
                </TxButton>
                <TxButton
                  variant="outline"
                  onClick={() => actions.undelegateVote(currentEntityId)}
                  txStatus={actions.txState.status}
                >
                  Undelegate
                </TxButton>
              </div>
            </CardContent>
          </Card>
        </>
      )}

      {actions.txState.status === "finalized" && (
        <div className="rounded-lg border border-green-200 bg-green-50 p-3 text-sm text-green-700 dark:border-green-800 dark:bg-green-900/20 dark:text-green-400">
          Configuration updated successfully!
          <Button
            variant="ghost"
            size="sm"
            className="ml-2"
            onClick={() => {
              actions.resetTx();
              refetch();
            }}
          >
            Dismiss
          </Button>
        </div>
      )}
      {actions.txState.status === "error" && (
        <div className="rounded-lg border border-red-200 bg-red-50 p-3 text-sm text-destructive dark:border-red-800 dark:bg-red-900/20">
          {actions.txState.error}
          <Button variant="ghost" size="sm" className="ml-2" onClick={actions.resetTx}>
            Dismiss
          </Button>
        </div>
      )}
    </div>
  );
}
