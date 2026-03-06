"use client";

import { useState, useMemo } from "react";
import { useEntityStore } from "@/stores/entity";
import {
  useProposals, useGovernanceConfig, useGovernanceActions,
} from "@/hooks/useGovernance";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import { PROPOSAL_STATUS, PROPOSAL_TYPE_CATEGORIES } from "@/lib/constants";
import { formatBalance } from "@/lib/utils";
import type { ProposalData } from "@/lib/types";
import {
  Vote, Plus, Settings, Activity, Filter,
  ThumbsUp, ThumbsDown, MinusCircle, Play, Ban, Gavel,
  FileText, Clock, RotateCcw, Shield, X,
} from "lucide-react";
import Link from "next/link";

const PROPOSAL_FIELDS: Record<string, Array<{ key: string; label: string; type: "text" | "number" }>> = {
  PriceChange: [
    { key: "product_id", label: "Product ID", type: "number" },
    { key: "new_price", label: "New Price", type: "number" },
  ],
  ProductListing: [{ key: "product_cid", label: "Product Data CID", type: "text" }],
  ProductDelisting: [{ key: "product_id", label: "Product ID", type: "number" }],
  InventoryAdjustment: [
    { key: "product_id", label: "Product ID", type: "number" },
    { key: "new_stock", label: "New Stock", type: "number" },
  ],
  Promotion: [{ key: "promo_cid", label: "Promotion Details CID", type: "text" }],
  ShopNameChange: [
    { key: "shop_id", label: "Shop ID", type: "number" },
    { key: "new_name", label: "New Name", type: "text" },
  ],
  ShopDescriptionChange: [
    { key: "shop_id", label: "Shop ID", type: "number" },
    { key: "description_cid", label: "Description CID", type: "text" },
  ],
  ShopPause: [{ key: "shop_id", label: "Shop ID", type: "number" }],
  ShopResume: [{ key: "shop_id", label: "Shop ID", type: "number" }],
  TokenConfigChange: [{ key: "config_cid", label: "Config Data CID", type: "text" }],
  TokenMint: [
    { key: "amount", label: "Amount", type: "number" },
    { key: "recipient", label: "Recipient Address", type: "text" },
  ],
  TokenBurn: [{ key: "amount", label: "Amount", type: "number" }],
  AirdropDistribution: [{ key: "airdrop_cid", label: "Airdrop Data CID", type: "text" }],
  Dividend: [{ key: "amount", label: "Amount", type: "number" }],
  TreasurySpend: [
    { key: "amount", label: "Amount", type: "number" },
    { key: "recipient", label: "Recipient Address", type: "text" },
  ],
  FeeAdjustment: [{ key: "new_fee", label: "New Fee (bps)", type: "number" }],
  RevenueShare: [{ key: "config_cid", label: "Config CID", type: "text" }],
  RefundPolicy: [{ key: "policy_cid", label: "Policy CID", type: "text" }],
  VotingPeriodChange: [{ key: "new_period", label: "New Period (blocks)", type: "number" }],
  QuorumChange: [{ key: "new_quorum", label: "New Quorum (1-100)", type: "number" }],
  ProposalThresholdChange: [{ key: "new_threshold", label: "New Threshold", type: "number" }],
  ExecutionDelayChange: [{ key: "new_delay", label: "New Delay (blocks)", type: "number" }],
  PassThresholdChange: [{ key: "new_pass", label: "New Pass Threshold (1-100)", type: "number" }],
  AdminVetoToggle: [{ key: "enabled", label: "Enabled (1=yes, 0=no)", type: "number" }],
  CommissionModesChange: [{ key: "new_modes", label: "New Modes Bitmask", type: "number" }],
  DirectRewardChange: [{ key: "new_rate", label: "New Rate (bps)", type: "number" }],
  MultiLevelChange: [{ key: "config_cid", label: "Config CID", type: "text" }],
  LevelDiffChange: [{ key: "config_cid", label: "Config CID", type: "text" }],
  FixedAmountChange: [{ key: "new_amount", label: "New Amount", type: "number" }],
  FirstOrderChange: [{ key: "config_cid", label: "Config CID", type: "text" }],
  RepeatPurchaseChange: [{ key: "config_cid", label: "Config CID", type: "text" }],
  SingleLineChange: [{ key: "config_cid", label: "Config CID", type: "text" }],
  WithdrawalConfigChange: [{ key: "config_cid", label: "Config CID", type: "text" }],
  MinRepurchaseRateChange: [{ key: "new_rate", label: "New Rate (bps)", type: "number" }],
  AddCustomLevel: [
    { key: "name", label: "Level Name", type: "text" },
    { key: "threshold", label: "Threshold", type: "number" },
  ],
  UpdateCustomLevel: [
    { key: "level_id", label: "Level ID", type: "number" },
    { key: "name", label: "New Name", type: "text" },
  ],
  RemoveCustomLevel: [{ key: "level_id", label: "Level ID", type: "number" }],
  SetUpgradeMode: [{ key: "mode", label: "Upgrade Mode", type: "text" }],
  EnableCustomLevels: [{ key: "enabled", label: "Enabled (1=yes, 0=no)", type: "number" }],
  AddUpgradeRule: [{ key: "config_cid", label: "Rule Config CID", type: "text" }],
  RemoveUpgradeRule: [{ key: "rule_id", label: "Rule ID", type: "number" }],
  TeamPerformanceChange: [{ key: "config_cid", label: "Config CID", type: "text" }],
  DisclosureLevelChange: [{ key: "new_level", label: "New Level", type: "text" }],
  CommunityEvent: [{ key: "event_cid", label: "Event Details CID", type: "text" }],
  RuleSuggestion: [{ key: "suggestion_cid", label: "Suggestion CID", type: "text" }],
};

function formatType(type: string | Record<string, unknown>): string {
  const key = typeof type === "string" ? type : Object.keys(type)[0] || "Unknown";
  return key.replace(/([A-Z])/g, " $1").trim();
}

function VoteBar({ yes, no, abstain }: { yes: bigint; no: bigint; abstain: bigint }) {
  const total = Number(yes) + Number(no) + Number(abstain);
  if (total === 0) return <div className="h-2 w-full rounded-full bg-muted" />;
  const yesPct = (Number(yes) / total) * 100;
  const noPct = (Number(no) / total) * 100;
  const abstainPct = (Number(abstain) / total) * 100;
  return (
    <div className="flex h-2 w-full overflow-hidden rounded-full bg-muted">
      {yesPct > 0 && <div className="bg-green-500 transition-all" style={{ width: `${yesPct}%` }} />}
      {noPct > 0 && <div className="bg-red-500 transition-all" style={{ width: `${noPct}%` }} />}
      {abstainPct > 0 && <div className="bg-gray-400 transition-all" style={{ width: `${abstainPct}%` }} />}
    </div>
  );
}

export default function GovernancePage() {
  const { currentEntityId } = useEntityStore();
  const { proposals, isLoading, refetch } = useProposals(currentEntityId);
  const { config, locked, paused } = useGovernanceConfig(currentEntityId);
  const actions = useGovernanceActions();

  const [statusFilter, setStatusFilter] = useState<string | null>(null);
  const [selectedType, setSelectedType] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [descriptionCid, setDescriptionCid] = useState("");
  const [typeParams, setTypeParams] = useState<Record<string, string>>({});

  const filteredProposals = useMemo(() => {
    if (!statusFilter) return proposals;
    return proposals.filter((p) => p.status === statusFilter);
  }, [proposals, statusFilter]);

  const activeCount = useMemo(
    () => proposals.filter((p) => p.status === "Voting").length,
    [proposals],
  );

  if (!currentEntityId) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        Select an entity to view governance
      </div>
    );
  }

  const handleCreate = () => {
    if (!selectedType || !title) return;
    const fields = PROPOSAL_FIELDS[selectedType] || [];
    const params: Record<string, unknown> = {};
    fields.forEach((f) => {
      const val = typeParams[f.key];
      if (val) params[f.key] = f.type === "number" ? Number(val) : val;
    });
    const proposalType = {
      [selectedType]: Object.keys(params).length > 0 ? params : null,
    };
    actions.createProposal(
      currentEntityId,
      proposalType,
      title,
      descriptionCid || null,
    );
  };

  const resetForm = () => {
    setSelectedType(null);
    setTitle("");
    setDescriptionCid("");
    setTypeParams({});
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Governance</h1>
          <p className="text-muted-foreground">DAO proposals, voting, and execution</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" asChild>
            <Link href="/governance/config">
              <Settings className="mr-2 h-4 w-4" />Configure
            </Link>
          </Button>
          <Button variant="outline" size="sm" onClick={() => refetch()}>
            <RotateCcw className="mr-2 h-3 w-3" />Refresh
          </Button>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardContent className="pt-6">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-purple-100 dark:bg-purple-900/30">
                <Activity className="h-5 w-5 text-purple-600 dark:text-purple-400" />
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Active Proposals</p>
                <p className="text-2xl font-bold">{activeCount}</p>
              </div>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="pt-6">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-blue-100 dark:bg-blue-900/30">
                <FileText className="h-5 w-5 text-blue-600 dark:text-blue-400" />
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Total Proposals</p>
                <p className="text-2xl font-bold">{proposals.length}</p>
              </div>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="pt-6">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-green-100 dark:bg-green-900/30">
                <Settings className="h-5 w-5 text-green-600 dark:text-green-400" />
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Governance Mode</p>
                <p className="text-2xl font-bold">{config?.mode || "—"}</p>
              </div>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="pt-6">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-amber-100 dark:bg-amber-900/30">
                <Shield className="h-5 w-5 text-amber-600 dark:text-amber-400" />
              </div>
              <div>
                <p className="text-sm text-muted-foreground">Status</p>
                <div className="mt-1 flex gap-1">
                  {locked && <Badge variant="destructive" className="text-xs">Locked</Badge>}
                  {paused && <Badge variant="secondary" className="text-xs">Paused</Badge>}
                  {!locked && !paused && <Badge variant="outline" className="text-xs">Active</Badge>}
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <Filter className="h-4 w-4 text-muted-foreground" />
        <Button
          variant={statusFilter === null ? "default" : "outline"}
          size="sm"
          onClick={() => setStatusFilter(null)}
        >
          All
        </Button>
        {PROPOSAL_STATUS.map((status) => (
          <Button
            key={status}
            variant={statusFilter === status ? "default" : "outline"}
            size="sm"
            onClick={() => setStatusFilter(statusFilter === status ? null : status)}
          >
            {status}
          </Button>
        ))}
      </div>

      {isLoading ? (
        <div className="flex justify-center py-8">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
        </div>
      ) : filteredProposals.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Vote className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No Proposals</p>
            <p className="text-sm text-muted-foreground">
              {statusFilter
                ? `No ${statusFilter.toLowerCase()} proposals found.`
                : "Create the first governance proposal."}
            </p>
          </CardContent>
        </Card>
      ) : (
        <div className="space-y-3">
          {filteredProposals.map((proposal) => (
            <Card key={proposal.id} className="transition-colors hover:bg-muted/30">
              <CardContent className="pt-6">
                <div className="flex items-start justify-between gap-4">
                  <div className="min-w-0 flex-1 space-y-3">
                    <div className="flex flex-wrap items-center gap-2">
                      <Link
                        href={`/governance/${proposal.id}`}
                        className="text-lg font-semibold hover:underline"
                      >
                        #{proposal.id} — {proposal.title || "Untitled"}
                      </Link>
                      <StatusBadge status={proposal.status} />
                      <Badge variant="outline" className="text-xs">
                        {formatType(proposal.proposalType)}
                      </Badge>
                    </div>
                    <div className="flex flex-wrap items-center gap-4 text-sm text-muted-foreground">
                      <AddressDisplay address={proposal.proposer} chars={4} />
                      <span className="flex items-center gap-1">
                        <Clock className="h-3 w-3" />
                        Ends block #{proposal.votingEnd}
                      </span>
                    </div>
                    <div className="space-y-1">
                      <VoteBar
                        yes={proposal.yesVotes}
                        no={proposal.noVotes}
                        abstain={proposal.abstainVotes}
                      />
                      <div className="flex gap-4 text-xs text-muted-foreground">
                        <span className="text-green-600">
                          Yes: {formatBalance(proposal.yesVotes)}
                        </span>
                        <span className="text-red-600">
                          No: {formatBalance(proposal.noVotes)}
                        </span>
                        <span>Abstain: {formatBalance(proposal.abstainVotes)}</span>
                      </div>
                    </div>
                  </div>

                  <div className="flex shrink-0 flex-col gap-1">
                    {proposal.status === "Voting" && (
                      <>
                        <div className="flex gap-1">
                          <TxButton
                            size="sm"
                            className="h-7 bg-green-600 hover:bg-green-700"
                            onClick={() => actions.vote(proposal.id, "Yes")}
                            txStatus={actions.txState.status}
                          >
                            <ThumbsUp className="mr-1 h-3 w-3" />Yes
                          </TxButton>
                          <TxButton
                            size="sm"
                            variant="destructive"
                            className="h-7"
                            onClick={() => actions.vote(proposal.id, "No")}
                            txStatus={actions.txState.status}
                          >
                            <ThumbsDown className="mr-1 h-3 w-3" />No
                          </TxButton>
                          <TxButton
                            size="sm"
                            variant="outline"
                            className="h-7"
                            onClick={() => actions.vote(proposal.id, "Abstain")}
                            txStatus={actions.txState.status}
                          >
                            <MinusCircle className="mr-1 h-3 w-3" />Abstain
                          </TxButton>
                        </div>
                        <TxButton
                          size="sm"
                          variant="outline"
                          className="h-7"
                          onClick={() => actions.finalizeVoting(proposal.id)}
                          txStatus={actions.txState.status}
                        >
                          <Gavel className="mr-1 h-3 w-3" />Finalize
                        </TxButton>
                      </>
                    )}
                    {proposal.status === "Passed" && (
                      <TxButton
                        size="sm"
                        className="h-7"
                        onClick={() => actions.executeProposal(proposal.id)}
                        txStatus={actions.txState.status}
                      >
                        <Play className="mr-1 h-3 w-3" />Execute
                      </TxButton>
                    )}
                    {(proposal.status === "Voting" || proposal.status === "Passed") && (
                      <TxButton
                        size="sm"
                        variant="outline"
                        className="h-7 text-destructive"
                        onClick={() => actions.cancelProposal(proposal.id)}
                        txStatus={actions.txState.status}
                      >
                        <Ban className="mr-1 h-3 w-3" />Cancel
                      </TxButton>
                    )}
                  </div>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}

      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div>
              <CardTitle className="flex items-center gap-2">
                <Plus className="h-5 w-5" />Create Proposal
              </CardTitle>
              <CardDescription>Submit a new governance proposal for voting</CardDescription>
            </div>
            {selectedType && (
              <Button variant="ghost" size="sm" onClick={resetForm}>
                <X className="mr-1 h-3 w-3" />Clear
              </Button>
            )}
          </div>
        </CardHeader>
        <CardContent>
          {!selectedType ? (
            <div className="space-y-6">
              {Object.entries(PROPOSAL_TYPE_CATEGORIES).map(([category, types]) => (
                <div key={category}>
                  <h4 className="mb-2 text-sm font-semibold text-muted-foreground">
                    {category}
                  </h4>
                  <div className="flex flex-wrap gap-2">
                    {types.map((type) => (
                      <Button
                        key={type}
                        variant="outline"
                        size="sm"
                        className="h-auto py-1.5"
                        onClick={() => {
                          setSelectedType(type);
                          setTypeParams({});
                        }}
                      >
                        {type.replace(/([A-Z])/g, " $1").trim()}
                      </Button>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="space-y-4">
              <Badge variant="secondary" className="text-sm">
                {selectedType.replace(/([A-Z])/g, " $1").trim()}
              </Badge>
              <div className="space-y-2">
                <label className="text-sm font-medium">Title</label>
                <Input
                  value={title}
                  onChange={(e) => setTitle(e.target.value)}
                  placeholder="Proposal title"
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">Description CID</label>
                <Input
                  value={descriptionCid}
                  onChange={(e) => setDescriptionCid(e.target.value)}
                  placeholder="IPFS CID (optional)"
                />
              </div>
              {(PROPOSAL_FIELDS[selectedType] || []).map((field) => (
                <div key={field.key} className="space-y-2">
                  <label className="text-sm font-medium">{field.label}</label>
                  <Input
                    type={field.type}
                    value={typeParams[field.key] || ""}
                    onChange={(e) =>
                      setTypeParams((prev) => ({ ...prev, [field.key]: e.target.value }))
                    }
                    placeholder={field.label}
                  />
                </div>
              ))}
              <TxButton
                onClick={handleCreate}
                txStatus={actions.txState.status}
                disabled={!title}
              >
                Submit Proposal
              </TxButton>
            </div>
          )}
        </CardContent>
      </Card>

      {actions.txState.status === "finalized" && (
        <div className="rounded-lg border border-green-200 bg-green-50 p-3 text-sm text-green-700 dark:border-green-800 dark:bg-green-900/20 dark:text-green-400">
          Transaction completed successfully!
          <Button
            variant="ghost"
            size="sm"
            className="ml-2"
            onClick={() => { actions.resetTx(); refetch(); resetForm(); }}
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
