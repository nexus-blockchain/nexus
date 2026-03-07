'use client';

import { useEntityQuery } from './use-entity-query';
import { useEntityMutation } from './use-entity-mutation';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { STALE_TIMES } from '@/lib/chain/constants';
import type { ProposalData } from '@/lib/types/models';
import { VoteOption } from '@/lib/types/enums';

// ─── Helpers ────────────────────────────────────────────────

function parseProposalEntries(rawEntries: [any, any][]): ProposalData[] {
  if (!rawEntries || !Array.isArray(rawEntries)) return [];
  return rawEntries.map(([key, value]) => {
    const obj = value?.toJSON?.() ?? value;
    return {
      id: Number(key.args?.[1]?.toString() ?? obj.id ?? 0),
      entityId: Number(key.args?.[0]?.toString() ?? obj.entityId ?? 0),
      proposer: String(obj.proposer ?? ''),
      proposalType: String(obj.proposalType ?? obj.proposal_type ?? ''),
      title: String(obj.title ?? ''),
      description: String(obj.description ?? ''),
      votesApprove: BigInt(String(obj.votesApprove ?? obj.votes_approve ?? 0)),
      votesReject: BigInt(String(obj.votesReject ?? obj.votes_reject ?? 0)),
      votesAbstain: BigInt(String(obj.votesAbstain ?? obj.votes_abstain ?? 0)),
      quorumPct: Number(obj.quorumPct ?? obj.quorum_pct ?? 0),
      passThreshold: Number(obj.passThreshold ?? obj.pass_threshold ?? 0),
      endBlock: Number(obj.endBlock ?? obj.end_block ?? 0),
      status: String(obj.status ?? 'Pending'),
      executed: Boolean(obj.executed),
    };
  });
}

/**
 * Compute proposal voting result.
 */
export function computeProposalResult(proposal: ProposalData): {
  quorumMet: boolean;
  passed: boolean;
  approvalPct: number;
} {
  const approve = Number(proposal.votesApprove);
  const reject = Number(proposal.votesReject);
  const abstain = Number(proposal.votesAbstain);
  const totalVotes = approve + reject + abstain;

  const quorumMet = totalVotes > 0 && proposal.quorumPct > 0;

  const activeVotes = approve + reject;
  const passed = activeVotes > 0
    ? (approve / activeVotes) * 100 >= proposal.passThreshold
    : false;

  const approvalPct = activeVotes > 0 ? (approve / activeVotes) * 100 : 0;

  return { quorumMet, passed, approvalPct };
}

// ─── Hook ───────────────────────────────────────────────────

export function useGovernance() {
  const { entityId } = useEntityContext();

  const invalidateKeys = [['entity', entityId, 'proposals']];

  // ─── Queries ──────────────────────────────────────────

  const proposalsQuery = useEntityQuery<ProposalData[]>(
    ['entity', entityId, 'proposals'],
    async (api) => {
      const raw = await (api.query as any).entityGovernance.proposals.entries(entityId);
      return parseProposalEntries(raw);
    },
    { staleTime: STALE_TIMES.proposals },
  );

  const proposalCountQuery = useEntityQuery<number>(
    ['entity', entityId, 'proposals', 'count'],
    async (api) => {
      const raw = await (api.query as any).entityGovernance.proposalCount(entityId);
      return Number(raw?.toString() ?? 0);
    },
    { staleTime: STALE_TIMES.proposals },
  );

  const useVote = (proposalId: number, account: string | null) =>
    useEntityQuery<VoteOption | null>(
      ['entity', entityId, 'proposals', proposalId, 'votes', account],
      async (api) => {
        if (!account) return null;
        const raw = await (api.query as any).entityGovernance.votes(entityId, proposalId, account);
        if (!raw || (raw as any).isNone) return null;
        const val = (raw as any).unwrapOr?.(null) ?? raw;
        return val ? (String(val) as VoteOption) : null;
      },
      { staleTime: STALE_TIMES.proposals, enabled: !!account },
    );

  // ─── Mutations ──────────────────────────────────────────

  const createProposal = useEntityMutation('entityGovernance', 'createProposal', { invalidateKeys });
  const vote = useEntityMutation('entityGovernance', 'vote', { invalidateKeys });
  const finalizeVoting = useEntityMutation('entityGovernance', 'finalizeVoting', { invalidateKeys });
  const executeProposal = useEntityMutation('entityGovernance', 'executeProposal', { invalidateKeys });

  return {
    proposals: proposalsQuery.data ?? [],
    proposalCount: proposalCountQuery.data ?? 0,
    isLoading: proposalsQuery.isLoading,
    error: proposalsQuery.error,
    useVote,
    createProposal,
    vote,
    finalizeVoting,
    executeProposal,
  };
}
