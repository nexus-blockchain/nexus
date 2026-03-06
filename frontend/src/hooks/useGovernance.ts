"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { ProposalData, GovernanceConfigData, VoteRecordData } from "@/lib/types";

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
          if (raw.isNone) return null;
          const data = raw.toJSON() as Record<string, unknown>;
          return {
            ...data,
            id,
            yesVotes: BigInt(String(data.yesVotes || 0)),
            noVotes: BigInt(String(data.noVotes || 0)),
            abstainVotes: BigInt(String(data.abstainVotes || 0)),
            snapshotTotalSupply: BigInt(String(data.snapshotTotalSupply || 0)),
          } as ProposalData;
        })
      );
      setProposals(results.filter(Boolean) as ProposalData[]);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { proposals, isLoading, refetch: fetch };
}

export function useGovernanceConfig(entityId: number | null) {
  const [config, setConfig] = useState<GovernanceConfigData | null>(null);
  const [locked, setLocked] = useState(false);
  const [paused, setPaused] = useState(false);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const [rawConfig, rawLocked, rawPaused] = await Promise.all([
        q.entityGovernance.governanceConfigs(entityId),
        q.entityGovernance.governanceLocked(entityId),
        q.entityGovernance.governancePaused(entityId),
      ]);
      if (rawConfig && !rawConfig.isNone) {
        setConfig(rawConfig.toJSON() as unknown as GovernanceConfigData);
      }
      setLocked(rawLocked?.toJSON?.() ?? false);
      setPaused(rawPaused?.toJSON?.() ?? false);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { config, locked, paused, isLoading, refetch: fetch };
}

export function useVoteRecords(proposalId: number | null) {
  const [votes, setVotes] = useState<VoteRecordData[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (proposalId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityGovernance.voteRecords.entries(proposalId);
      const results = entries.map(([key, v]: [{ args: [unknown, { toString: () => string }] }, { toJSON: () => Record<string, unknown> }]) => {
        const data = v.toJSON();
        return {
          voter: key.args[1].toString(),
          vote: data.vote,
          weight: BigInt(String(data.weight || 0)),
          votedAt: data.votedAt,
        } as VoteRecordData;
      });
      setVotes(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [proposalId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { votes, isLoading, refetch: fetch };
}

export function useGovernanceActions() {
  const { submit, state, reset } = useTx();
  return {
    createProposal: (entityId: number, proposalType: Record<string, unknown>, title: string, descriptionCid: string | null) =>
      submit("entityGovernance", "createProposal", [entityId, proposalType, title, descriptionCid]),
    vote: (proposalId: number, voteType: string) =>
      submit("entityGovernance", "vote", [proposalId, voteType]),
    changeVote: (proposalId: number, newVote: string) =>
      submit("entityGovernance", "changeVote", [proposalId, newVote]),
    finalizeVoting: (proposalId: number) =>
      submit("entityGovernance", "finalizeVoting", [proposalId]),
    executeProposal: (proposalId: number) =>
      submit("entityGovernance", "executeProposal", [proposalId]),
    cancelProposal: (proposalId: number) =>
      submit("entityGovernance", "cancelProposal", [proposalId]),
    vetoProposal: (proposalId: number) =>
      submit("entityGovernance", "vetoProposal", [proposalId]),
    configureGovernance: (
      entityId: number, mode: string,
      votingPeriod: number | null, executionDelay: number | null,
      quorumThreshold: number | null, passThreshold: number | null,
      proposalThreshold: number | null, adminVetoEnabled: boolean | null,
    ) => submit("entityGovernance", "configureGovernance", [
      entityId, mode, votingPeriod, executionDelay,
      quorumThreshold, passThreshold, proposalThreshold, adminVetoEnabled,
    ]),
    lockGovernance: (entityId: number) =>
      submit("entityGovernance", "lockGovernance", [entityId]),
    cleanupProposal: (proposalId: number) =>
      submit("entityGovernance", "cleanupProposal", [proposalId]),
    delegateVote: (entityId: number, delegate: string) =>
      submit("entityGovernance", "delegateVote", [entityId, delegate]),
    undelegateVote: (entityId: number) =>
      submit("entityGovernance", "undelegateVote", [entityId]),
    pauseGovernance: (entityId: number) =>
      submit("entityGovernance", "pauseGovernance", [entityId]),
    resumeGovernance: (entityId: number) =>
      submit("entityGovernance", "resumeGovernance", [entityId]),
    batchCancelProposals: (entityId: number) =>
      submit("entityGovernance", "batchCancelProposals", [entityId]),
    txState: state,
    resetTx: reset,
  };
}
