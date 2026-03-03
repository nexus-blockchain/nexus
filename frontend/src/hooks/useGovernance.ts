"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { ProposalData } from "@/lib/types";

export function useProposals(entityId: number | null) {
  const [proposals, setProposals] = useState<ProposalData[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const ids = await (api.query as any).entityGovernance.entityProposals(entityId);
      const idList = (ids.toJSON() || []) as number[];
      const results = await Promise.all(
        idList.map(async (id: number) => {
          const raw = await (api.query as any).entityGovernance.proposals(id);
          return raw.toJSON() as ProposalData;
        })
      );
      setProposals(results.filter(Boolean));
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { proposals, isLoading, refetch: fetch };
}

export function useGovernanceActions() {
  const { submit, state, reset } = useTx();
  return {
    createProposal: (entityId: number, proposalType: string, title: string, descriptionCid: string | null) =>
      submit("entityGovernance", "createProposal", [entityId, proposalType, title, descriptionCid]),
    vote: (proposalId: number, voteType: string) =>
      submit("entityGovernance", "vote", [proposalId, voteType]),
    finalizeVoting: (proposalId: number) =>
      submit("entityGovernance", "finalizeVoting", [proposalId]),
    executeProposal: (proposalId: number) =>
      submit("entityGovernance", "executeProposal", [proposalId]),
    cancelProposal: (proposalId: number) =>
      submit("entityGovernance", "cancelProposal", [proposalId]),
    vetoProposal: (proposalId: number) =>
      submit("entityGovernance", "vetoProposal", [proposalId]),
    configureGovernance: (entityId: number, mode: string, quorum: number, votingPeriod: number, executionDelay: number) =>
      submit("entityGovernance", "configureGovernance", [entityId, mode, quorum, votingPeriod, executionDelay]),
    lockGovernance: (entityId: number) =>
      submit("entityGovernance", "lockGovernance", [entityId]),
    cleanupProposal: (proposalId: number) =>
      submit("entityGovernance", "cleanupProposal", [proposalId]),
    txState: state,
    resetTx: reset,
  };
}
