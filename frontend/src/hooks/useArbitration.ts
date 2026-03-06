"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { ComplaintData, EscrowData, EvidenceData } from "@/lib/types";

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
      const nextIdVal = (nextIdRaw.toJSON() ?? 0) as number;
      setNextId(nextIdVal);

      const entries = await q.arbitration.complaints.entries();
      const all = entries
        .map(([_k, v]: [unknown, { toJSON: () => ComplaintData }]) => v.toJSON())
        .filter(Boolean) as ComplaintData[];

      if (status) {
        setComplaints(all.filter((c: ComplaintData) => c.status === status));
      } else {
        setComplaints(all);
      }
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [status]);

  useEffect(() => {
    fetch();
  }, [fetch]);

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
      const q = api.query as any;

      const raw = await q.arbitration.complaints(complaintId);
      setComplaint(raw.isNone ? null : (raw.toJSON() as ComplaintData));
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [complaintId]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { complaint, isLoading, refetch: fetch };
}

export function useArbitrationActions() {
  const { submit, state, reset } = useTx();
  return {
    fileComplaint: (
      respondent: string,
      complaintType: string,
      domain: string,
      objectId: number | null,
      detailCid: string
    ) =>
      submit("arbitration", "fileComplaint", [
        respondent,
        complaintType,
        domain,
        objectId,
        detailCid,
      ]),
    respondToComplaint: (complaintId: number, responseCid: string) =>
      submit("arbitration", "respondToComplaint", [complaintId, responseCid]),
    withdrawComplaint: (complaintId: number) =>
      submit("arbitration", "withdrawComplaint", [complaintId]),
    escalateToArbitration: (complaintId: number) =>
      submit("arbitration", "escalateToArbitration", [complaintId]),
    settleComplaint: (complaintId: number, settlementCid: string) =>
      submit("arbitration", "settleComplaint", [complaintId, settlementCid]),
    supplementComplaintEvidence: (complaintId: number, evidenceCid: string) =>
      submit("arbitration", "supplementComplaintEvidence", [complaintId, evidenceCid]),
    txState: state,
    resetTx: reset,
  };
}
