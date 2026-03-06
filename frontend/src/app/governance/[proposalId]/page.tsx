"use client";

import { useState, useEffect, useCallback } from "react";
import { useParams } from "next/navigation";
import { getApi } from "@/hooks/useApi";
import { useVoteRecords, useGovernanceActions } from "@/hooks/useGovernance";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import {
  Table, TableHeader, TableBody, TableRow, TableHead, TableCell,
} from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { TxButton } from "@/components/shared/TxButton";
import { formatBalance } from "@/lib/utils";
import type { ProposalData } from "@/lib/types";
import {
  ArrowLeft, ThumbsUp, ThumbsDown, MinusCircle, Play, Ban,
  Shield, Trash2, Gavel, RefreshCw, BarChart3, Users,
} from "lucide-react";
import Link from "next/link";

function formatType(type: string | Record<string, unknown>): string {
  const key = typeof type === "string" ? type : Object.keys(type)[0] || "Unknown";
  return key.replace(/([A-Z])/g, " $1").trim();
}

export default function ProposalDetailPage() {
  const { proposalId: proposalIdStr } = useParams();
  const proposalId = Number(proposalIdStr);
  const { votes, isLoading: votesLoading, refetch: refetchVotes } = useVoteRecords(proposalId);
  const actions = useGovernanceActions();

  const [proposal, setProposal] = useState<ProposalData | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  const fetchProposal = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).entityGovernance.proposals(proposalId);
      if (raw && !raw.isNone) {
        const data = raw.toJSON() as Record<string, unknown>;
        setProposal({
          ...data,
          id: proposalId,
          yesVotes: BigInt(String(data.yesVotes || 0)),
          noVotes: BigInt(String(data.noVotes || 0)),
          abstainVotes: BigInt(String(data.abstainVotes || 0)),
          snapshotTotalSupply: BigInt(String(data.snapshotTotalSupply || 0)),
        } as ProposalData);
      }
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [proposalId]);

  useEffect(() => {
    fetchProposal();
  }, [fetchProposal]);

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
      </div>
    );
  }

  if (!proposal) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4">
        <p className="text-muted-foreground">Proposal #{proposalId} not found</p>
        <Button variant="outline" asChild>
          <Link href="/governance">Back to Governance</Link>
        </Button>
      </div>
    );
  }

  const totalVotes =
    Number(proposal.yesVotes) + Number(proposal.noVotes) + Number(proposal.abstainVotes);
  const yesPct = totalVotes > 0 ? (Number(proposal.yesVotes) / totalVotes) * 100 : 0;
  const noPct = totalVotes > 0 ? (Number(proposal.noVotes) / totalVotes) * 100 : 0;
  const abstainPct = totalVotes > 0 ? (Number(proposal.abstainVotes) / totalVotes) * 100 : 0;

  const quorumRequired =
    proposal.snapshotTotalSupply > 0n
      ? (Number(proposal.snapshotTotalSupply) * proposal.snapshotQuorum) / 100
      : 0;
  const quorumReached = totalVotes >= quorumRequired;
  const quorumPct = quorumRequired > 0 ? (totalVotes / quorumRequired) * 100 : 0;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/governance">
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <div className="flex-1">
          <div className="flex items-center gap-3">
            <h1 className="text-3xl font-bold tracking-tight">
              {proposal.title || "Untitled"}
            </h1>
            <StatusBadge status={proposal.status} />
          </div>
          <p className="text-muted-foreground">
            Proposal #{proposalId} &middot; {formatType(proposal.proposalType)}
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => {
            fetchProposal();
            refetchVotes();
          }}
        >
          <RefreshCw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      <div className="grid gap-6 lg:grid-cols-3">
        <Card className="lg:col-span-2">
          <CardHeader>
            <CardTitle>Proposal Details</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Proposer</span>
              <AddressDisplay address={proposal.proposer} />
            </div>
            <Separator />
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Type</span>
              <Badge variant="outline">{formatType(proposal.proposalType)}</Badge>
            </div>
            <Separator />
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Status</span>
              <StatusBadge status={proposal.status} />
            </div>
            <Separator />
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Created At</span>
              <span className="text-sm">Block #{proposal.createdAt}</span>
            </div>
            <Separator />
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Voting Start</span>
              <span className="text-sm">Block #{proposal.votingStart}</span>
            </div>
            <Separator />
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Voting End</span>
              <span className="text-sm">Block #{proposal.votingEnd}</span>
            </div>
            {proposal.executionTime && (
              <>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Execution Time</span>
                  <span className="text-sm">Block #{proposal.executionTime}</span>
                </div>
              </>
            )}
            {proposal.descriptionCid && (
              <>
                <Separator />
                <div className="flex justify-between">
                  <span className="text-sm text-muted-foreground">Description CID</span>
                  <span className="max-w-[200px] truncate text-sm font-mono">
                    {proposal.descriptionCid}
                  </span>
                </div>
              </>
            )}
            <Separator />
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Snapshot Quorum</span>
              <span className="text-sm">{proposal.snapshotQuorum}%</span>
            </div>
            <Separator />
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Snapshot Pass</span>
              <span className="text-sm">{proposal.snapshotPass}%</span>
            </div>
            <Separator />
            <div className="flex justify-between">
              <span className="text-sm text-muted-foreground">Execution Delay</span>
              <span className="text-sm">{proposal.snapshotExecutionDelay} blocks</span>
            </div>
          </CardContent>
        </Card>

        <div className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <BarChart3 className="h-5 w-5" />Vote Distribution
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex h-4 w-full overflow-hidden rounded-full bg-muted">
                {yesPct > 0 && (
                  <div
                    className="bg-green-500 transition-all"
                    style={{ width: `${yesPct}%` }}
                  />
                )}
                {noPct > 0 && (
                  <div
                    className="bg-red-500 transition-all"
                    style={{ width: `${noPct}%` }}
                  />
                )}
                {abstainPct > 0 && (
                  <div
                    className="bg-gray-400 transition-all"
                    style={{ width: `${abstainPct}%` }}
                  />
                )}
              </div>
              <div className="space-y-2">
                <div className="flex justify-between text-sm">
                  <span className="flex items-center gap-1.5">
                    <span className="h-2.5 w-2.5 rounded-full bg-green-500" />
                    Yes
                  </span>
                  <span>
                    {formatBalance(proposal.yesVotes)} ({yesPct.toFixed(1)}%)
                  </span>
                </div>
                <div className="flex justify-between text-sm">
                  <span className="flex items-center gap-1.5">
                    <span className="h-2.5 w-2.5 rounded-full bg-red-500" />
                    No
                  </span>
                  <span>
                    {formatBalance(proposal.noVotes)} ({noPct.toFixed(1)}%)
                  </span>
                </div>
                <div className="flex justify-between text-sm">
                  <span className="flex items-center gap-1.5">
                    <span className="h-2.5 w-2.5 rounded-full bg-gray-400" />
                    Abstain
                  </span>
                  <span>
                    {formatBalance(proposal.abstainVotes)} ({abstainPct.toFixed(1)}%)
                  </span>
                </div>
              </div>
              <Separator />
              <div className="flex justify-between text-sm font-medium">
                <span>Total Votes</span>
                <span>{totalVotes.toLocaleString()}</span>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="text-sm">Quorum Check</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-sm text-muted-foreground">
                  Required ({proposal.snapshotQuorum}%)
                </span>
                <Badge variant={quorumReached ? "default" : "secondary"}>
                  {quorumReached ? "Reached" : "Not Reached"}
                </Badge>
              </div>
              <div className="h-2 w-full overflow-hidden rounded-full bg-muted">
                <div
                  className={`h-full transition-all ${quorumReached ? "bg-green-500" : "bg-amber-500"}`}
                  style={{ width: `${Math.min(quorumPct, 100)}%` }}
                />
              </div>
              <p className="text-xs text-muted-foreground">
                {totalVotes.toLocaleString()} / {Math.round(quorumRequired).toLocaleString()} votes
                ({Math.min(quorumPct, 100).toFixed(1)}%)
              </p>
            </CardContent>
          </Card>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Actions</CardTitle>
          <CardDescription>Available actions for this proposal</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap gap-3">
          {proposal.status === "Voting" && (
            <>
              <TxButton
                className="bg-green-600 hover:bg-green-700"
                onClick={() => actions.vote(proposalId, "Yes")}
                txStatus={actions.txState.status}
              >
                <ThumbsUp className="mr-2 h-4 w-4" />Vote Yes
              </TxButton>
              <TxButton
                variant="destructive"
                onClick={() => actions.vote(proposalId, "No")}
                txStatus={actions.txState.status}
              >
                <ThumbsDown className="mr-2 h-4 w-4" />Vote No
              </TxButton>
              <TxButton
                variant="outline"
                onClick={() => actions.vote(proposalId, "Abstain")}
                txStatus={actions.txState.status}
              >
                <MinusCircle className="mr-2 h-4 w-4" />Abstain
              </TxButton>
              <Separator orientation="vertical" className="h-9" />
              <TxButton
                variant="outline"
                onClick={() => actions.changeVote(proposalId, "Yes")}
                txStatus={actions.txState.status}
              >
                Change to Yes
              </TxButton>
              <TxButton
                variant="outline"
                onClick={() => actions.changeVote(proposalId, "No")}
                txStatus={actions.txState.status}
              >
                Change to No
              </TxButton>
              <TxButton
                variant="outline"
                onClick={() => actions.changeVote(proposalId, "Abstain")}
                txStatus={actions.txState.status}
              >
                Change to Abstain
              </TxButton>
              <Separator orientation="vertical" className="h-9" />
              <TxButton
                variant="outline"
                onClick={() => actions.finalizeVoting(proposalId)}
                txStatus={actions.txState.status}
              >
                <Gavel className="mr-2 h-4 w-4" />Finalize
              </TxButton>
            </>
          )}
          {proposal.status === "Passed" && (
            <TxButton
              onClick={() => actions.executeProposal(proposalId)}
              txStatus={actions.txState.status}
            >
              <Play className="mr-2 h-4 w-4" />Execute Proposal
            </TxButton>
          )}
          <TxButton
            variant="outline"
            onClick={() => actions.cancelProposal(proposalId)}
            txStatus={actions.txState.status}
          >
            <Ban className="mr-2 h-4 w-4" />Cancel
          </TxButton>
          <TxButton
            variant="outline"
            onClick={() => actions.vetoProposal(proposalId)}
            txStatus={actions.txState.status}
          >
            <Shield className="mr-2 h-4 w-4" />Veto
          </TxButton>
          <TxButton
            variant="outline"
            onClick={() => actions.cleanupProposal(proposalId)}
            txStatus={actions.txState.status}
          >
            <Trash2 className="mr-2 h-4 w-4" />Cleanup
          </TxButton>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Users className="h-5 w-5" />Vote Records
          </CardTitle>
          <CardDescription>
            {votes.length} vote{votes.length !== 1 ? "s" : ""} recorded
          </CardDescription>
        </CardHeader>
        <CardContent>
          {votesLoading ? (
            <div className="flex justify-center py-8">
              <div className="h-6 w-6 animate-spin rounded-full border-4 border-primary border-t-transparent" />
            </div>
          ) : votes.length === 0 ? (
            <p className="py-4 text-center text-sm text-muted-foreground">
              No votes have been cast yet.
            </p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Voter</TableHead>
                  <TableHead>Vote</TableHead>
                  <TableHead className="text-right">Weight</TableHead>
                  <TableHead className="text-right">Block</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {votes.map((v, i) => (
                  <TableRow key={i}>
                    <TableCell>
                      <AddressDisplay address={v.voter} chars={6} />
                    </TableCell>
                    <TableCell>
                      <Badge
                        variant={
                          v.vote === "Yes"
                            ? "default"
                            : v.vote === "No"
                              ? "destructive"
                              : "secondary"
                        }
                      >
                        {v.vote}
                      </Badge>
                    </TableCell>
                    <TableCell className="text-right font-mono">
                      {formatBalance(v.weight)}
                    </TableCell>
                    <TableCell className="text-right">#{v.votedAt}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      {actions.txState.status === "finalized" && (
        <div className="rounded-lg border border-green-200 bg-green-50 p-3 text-sm text-green-700 dark:border-green-800 dark:bg-green-900/20 dark:text-green-400">
          Action completed successfully!
          <Button
            variant="ghost"
            size="sm"
            className="ml-2"
            onClick={() => {
              actions.resetTx();
              fetchProposal();
              refetchVotes();
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
