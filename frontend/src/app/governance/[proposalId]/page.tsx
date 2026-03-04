"use client";

import { use, useState, useEffect, useCallback } from "react";
import { useEntityStore } from "@/stores/entity";
import { getApi } from "@/hooks/useApi";
import { useTx } from "@/hooks/useTx";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import { Separator } from "@/components/ui/separator";
import { formatBalance } from "@/lib/utils";
import { ArrowLeft, ThumbsUp, ThumbsDown, MinusCircle, Play, Clock } from "lucide-react";
import Link from "next/link";
import type { ProposalData } from "@/lib/types";

export default function ProposalDetailPage({ params }: { params: Promise<{ proposalId: string }> }) {
  const { proposalId: proposalIdStr } = use(params);
  const proposalId = Number(proposalIdStr);
  const { currentEntityId } = useEntityStore();
  const { submit, state: txState, reset } = useTx();

  const [proposal, setProposal] = useState<ProposalData | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetchProposal = useCallback(async () => {
    if (currentEntityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).entityGovernance.proposals(currentEntityId, proposalId);
      if (!raw.isNone) setProposal(raw.toJSON() as unknown as ProposalData);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [currentEntityId, proposalId]);

  useEffect(() => { fetchProposal(); }, [fetchProposal]);

  if (isLoading) {
    return <div className="flex h-full items-center justify-center"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>;
  }

  if (!proposal) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4">
        <p className="text-muted-foreground">Proposal not found</p>
        <Button variant="outline" asChild><Link href="/governance">Back to Governance</Link></Button>
      </div>
    );
  }

  const totalVotes = Number(proposal.yesVotes) + Number(proposal.noVotes) + Number(proposal.abstainVotes);
  const yesPct = totalVotes > 0 ? (Number(proposal.yesVotes) / totalVotes) * 100 : 0;
  const noPct = totalVotes > 0 ? (Number(proposal.noVotes) / totalVotes) * 100 : 0;
  const abstainPct = totalVotes > 0 ? (Number(proposal.abstainVotes) / totalVotes) * 100 : 0;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/governance"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <div className="flex items-center gap-3">
            <h1 className="text-3xl font-bold tracking-tight">{proposal.title}</h1>
            <StatusBadge status={proposal.status} />
          </div>
          <p className="text-muted-foreground">Proposal #{proposalId} &middot; {proposal.proposalType}</p>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-3">
        <Card className="lg:col-span-2">
          <CardHeader><CardTitle>Proposal Details</CardTitle></CardHeader>
          <CardContent className="space-y-3">
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Proposer</span><AddressDisplay address={proposal.proposer} /></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Type</span><Badge variant="outline">{proposal.proposalType}</Badge></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Created</span><span className="text-sm">Block #{proposal.createdAt}</span></div>
            <Separator />
            <div className="flex justify-between"><span className="text-sm text-muted-foreground">Voting Ends</span><span className="text-sm">Block #{proposal.votingEnd}</span></div>
            <Separator />
            {proposal.executionTime && (
              <>
                <div className="flex justify-between"><span className="text-sm text-muted-foreground">Execution Time</span><span className="text-sm">Block #{proposal.executionTime}</span></div>
                <Separator />
              </>
            )}
            {proposal.descriptionCid && (
              <div className="flex justify-between"><span className="text-sm text-muted-foreground">Description CID</span><span className="text-sm font-mono">{proposal.descriptionCid}</span></div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader><CardTitle>Voting Results</CardTitle></CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <div className="flex justify-between text-sm">
                <span className="flex items-center gap-1 text-green-600"><ThumbsUp className="h-3 w-3" />Aye</span>
                <span>{formatBalance(proposal.yesVotes)} ({yesPct.toFixed(1)}%)</span>
              </div>
              <Progress value={yesPct} className="h-2" />
            </div>
            <div className="space-y-2">
              <div className="flex justify-between text-sm">
                <span className="flex items-center gap-1 text-red-600"><ThumbsDown className="h-3 w-3" />Nay</span>
                <span>{formatBalance(proposal.noVotes)} ({noPct.toFixed(1)}%)</span>
              </div>
              <Progress value={noPct} className="h-2" />
            </div>
            <div className="space-y-2">
              <div className="flex justify-between text-sm">
                <span className="flex items-center gap-1 text-muted-foreground"><MinusCircle className="h-3 w-3" />Abstain</span>
                <span>{formatBalance(proposal.abstainVotes)} ({abstainPct.toFixed(1)}%)</span>
              </div>
              <Progress value={abstainPct} className="h-2" />
            </div>
            <Separator />
            <div className="flex justify-between text-sm font-medium">
              <span>Total Votes</span>
              <span>{totalVotes.toLocaleString()}</span>
            </div>
          </CardContent>
        </Card>
      </div>

      {proposal.status === "Active" && (
        <Card>
          <CardHeader>
            <CardTitle>Cast Your Vote</CardTitle>
            <CardDescription>Vote on this proposal before block #{proposal.votingEnd}</CardDescription>
          </CardHeader>
          <CardContent className="flex gap-4">
            <TxButton
              onClick={() => currentEntityId !== null && submit("entityGovernance", "vote", [currentEntityId, proposalId, "Aye"])}
              txStatus={txState.status}
              className="flex-1"
            >
              <ThumbsUp className="mr-2 h-4 w-4" />Vote Aye
            </TxButton>
            <TxButton
              variant="outline"
              onClick={() => currentEntityId !== null && submit("entityGovernance", "vote", [currentEntityId, proposalId, "Nay"])}
              txStatus={txState.status}
              className="flex-1"
            >
              <ThumbsDown className="mr-2 h-4 w-4" />Vote Nay
            </TxButton>
            <TxButton
              variant="outline"
              onClick={() => currentEntityId !== null && submit("entityGovernance", "vote", [currentEntityId, proposalId, "Abstain"])}
              txStatus={txState.status}
              className="flex-1"
            >
              <MinusCircle className="mr-2 h-4 w-4" />Abstain
            </TxButton>
          </CardContent>
        </Card>
      )}

      {proposal.status === "Passed" && (
        <Card>
          <CardHeader><CardTitle>Execute Proposal</CardTitle></CardHeader>
          <CardContent>
            <TxButton
              onClick={() => currentEntityId !== null && submit("entityGovernance", "executeProposal", [currentEntityId, proposalId])}
              txStatus={txState.status}
            >
              <Play className="mr-2 h-4 w-4" />Execute
            </TxButton>
          </CardContent>
        </Card>
      )}

      {txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {txState.status === "error" && <p className="text-sm text-destructive">{txState.error}</p>}
    </div>
  );
}
