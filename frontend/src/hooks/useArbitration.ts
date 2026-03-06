"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { ComplaintData, ArbitrationStats, DomainStatistics } from "@/lib/types";

export function useComplaints(status?: string) {
  const [complaints, setComplaints] = useState<ComplaintData[]>([]);
  const [nextId, setNextId] = useState(0);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;

      const nextIdRaw = await q.arbitration.nextComplaintId();
      setNextId((nextIdRaw.toJSON() ?? 0) as number);

      const entries = await q.arbitration.complaints.entries();
      const all = entries
        .map(([_k, v]: [unknown, { toJSON: () => ComplaintData }]) => v.toJSON())
        .filter(Boolean) as ComplaintData[];

      setComplaints(status ? all.filter((c) => c.status === status) : all);
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [status]);

  useEffect(() => { fetch(); }, [fetch]);
  return { complaints, nextId, isLoading, refetch: fetch };
}

export function useComplaint(complaintId: number | null) {
  const [complaint, setComplaint] = useState<ComplaintData | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (complaintId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).arbitration.complaints(complaintId);
      setComplaint(raw.isNone ? null : (raw.toJSON() as ComplaintData));
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [complaintId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { complaint, isLoading, refetch: fetch };
}

export function useUserComplaints(address: string | null) {
  const [ids, setIds] = useState<number[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!address) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const raw = await q.arbitration.userActiveComplaints(address);
      setIds((raw.toJSON() || []) as number[]);
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [address]);

  useEffect(() => { fetch(); }, [fetch]);
  return { ids, isLoading, refetch: fetch };
}

export function useArbitrationStats() {
  const [stats, setStats] = useState<ArbitrationStats | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).arbitration.arbitrationStats();
      setStats(raw.isNone ? null : (raw.toJSON() as ArbitrationStats));
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { stats, isLoading, refetch: fetch };
}

export function useDomainStats(domain: string | null) {
  const [stats, setStats] = useState<DomainStatistics | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!domain) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).arbitration.domainStats(domain);
      setStats(raw.isNone ? null : (raw.toJSON() as DomainStatistics));
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [domain]);

  useEffect(() => { fetch(); }, [fetch]);
  return { stats, isLoading, refetch: fetch };
}

export function useArbitrationPaused() {
  const [paused, setPaused] = useState(false);
  const fetch = useCallback(async () => {
    try {
      const api = await getApi();
      const raw = await (api.query as any).arbitration.paused();
      setPaused((raw.toJSON() ?? false) as boolean);
    } catch { /* ignore */ }
  }, []);
  useEffect(() => { fetch(); }, [fetch]);
  return { paused, refetch: fetch };
}

export function useArbitrationActions() {
  const { submit, state, reset } = useTx();
  return {
    // Complaint lifecycle
    fileComplaint: (domain: string, objectId: number, complaintType: string, detailsCid: string, amount: bigint | null) =>
      submit("arbitration", "fileComplaint", [domain, objectId, complaintType, detailsCid, amount]),
    respondToComplaint: (complaintId: number, responseCid: string) =>
      submit("arbitration", "respondToComplaint", [complaintId, responseCid]),
    withdrawComplaint: (complaintId: number) =>
      submit("arbitration", "withdrawComplaint", [complaintId]),
    settleComplaint: (complaintId: number, settlementCid: string) =>
      submit("arbitration", "settleComplaint", [complaintId, settlementCid]),
    escalateToArbitration: (complaintId: number) =>
      submit("arbitration", "escalateToArbitration", [complaintId]),
    resolveComplaint: (complaintId: number, decision: number, reasonCid: string, partialBps: number | null) =>
      submit("arbitration", "resolveComplaint", [complaintId, decision, reasonCid, partialBps]),
    supplementComplaintEvidence: (complaintId: number, evidenceCid: string) =>
      submit("arbitration", "supplementComplaintEvidence", [complaintId, evidenceCid]),
    supplementResponseEvidence: (complaintId: number, evidenceCid: string) =>
      submit("arbitration", "supplementResponseEvidence", [complaintId, evidenceCid]),
    startMediation: (complaintId: number) =>
      submit("arbitration", "startMediation", [complaintId]),
    dismissComplaint: (complaintId: number) =>
      submit("arbitration", "dismissComplaint", [complaintId]),
    forceCloseComplaint: (complaintId: number) =>
      submit("arbitration", "forceCloseComplaint", [complaintId]),

    // Dispute (domain-level)
    dispute: (domain: string, id: number, evidence: string[]) =>
      submit("arbitration", "dispute", [domain, id, evidence]),
    disputeWithEvidenceId: (domain: string, id: number, evidenceId: number) =>
      submit("arbitration", "disputeWithEvidenceId", [domain, id, evidenceId]),
    disputeWithTwoWayDeposit: (domain: string, id: number, evidenceId: number) =>
      submit("arbitration", "disputeWithTwoWayDeposit", [domain, id, evidenceId]),
    respondToDispute: (domain: string, id: number, counterEvidenceId: number) =>
      submit("arbitration", "respondToDispute", [domain, id, counterEvidenceId]),
    appendEvidenceId: (domain: string, id: number, evidenceId: number) =>
      submit("arbitration", "appendEvidenceId", [domain, id, evidenceId]),
    arbitrate: (domain: string, id: number, decisionCode: number, bps: number | null) =>
      submit("arbitration", "arbitrate", [domain, id, decisionCode, bps]),
    settleDispute: (domain: string, id: number) =>
      submit("arbitration", "settleDispute", [domain, id]),
    dismissDispute: (domain: string, id: number) =>
      submit("arbitration", "dismissDispute", [domain, id]),
    requestDefaultJudgment: (domain: string, id: number) =>
      submit("arbitration", "requestDefaultJudgment", [domain, id]),
    forceCloseDispute: (domain: string, id: number) =>
      submit("arbitration", "forceCloseDispute", [domain, id]),

    // Admin
    setPaused: (paused: boolean) =>
      submit("arbitration", "setPaused", [paused]),
    setDomainPenaltyRate: (domain: string, rateBps: number | null) =>
      submit("arbitration", "setDomainPenaltyRate", [domain, rateBps]),

    txState: state,
    resetTx: reset,
  };
}
