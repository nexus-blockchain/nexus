"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { EvidenceData } from "@/lib/types";

export function useEvidence(evidenceId: number | null) {
  const [evidence, setEvidence] = useState<EvidenceData | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (evidenceId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).evidence.evidences(evidenceId);
      if (raw.isNone) { setEvidence(null); return; }
      const data = raw.toJSON() as EvidenceData;
      const statusRaw = await (api.query as any).evidence.evidenceStatuses(evidenceId);
      data.status = statusRaw.isNone ? "Active" : String(statusRaw.toJSON());
      setEvidence(data);
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [evidenceId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { evidence, isLoading, refetch: fetch };
}

export function useEvidenceByTarget(domain: number, targetId: number) {
  const [count, setCount] = useState(0);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).evidence.evidenceCountByTarget([domain, targetId]);
      setCount((raw.toJSON() ?? 0) as number);
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [domain, targetId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { count, isLoading, refetch: fetch };
}

export function useEvidenceList() {
  const [evidences, setEvidences] = useState<EvidenceData[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const nextId = ((await q.evidence.nextEvidenceId()).toJSON() ?? 0) as number;
      const results: EvidenceData[] = [];
      for (let i = 0; i < nextId && i < 200; i++) {
        const raw = await q.evidence.evidences(i);
        if (!raw.isNone) {
          const data = raw.toJSON() as EvidenceData;
          data.id = i;
          const statusRaw = await q.evidence.evidenceStatuses(i);
          data.status = statusRaw.isNone ? "Active" : String(statusRaw.toJSON());
          results.push(data);
        }
      }
      setEvidences(results);
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { evidences, isLoading, refetch: fetch };
}

export function useEvidenceActions() {
  const { submit, state, reset } = useTx();
  return {
    commit: (domain: number, targetId: number, imgs: string[], vids: string[], docs: string[], memo: string | null) =>
      submit("evidence", "commit", [domain, targetId, imgs, vids, docs, memo]),
    commitHash: (ns: string, subjectId: number, commitHash: string, memo: string | null) =>
      submit("evidence", "commitHash", [ns, subjectId, commitHash, memo]),
    appendEvidence: (parentId: number, imgs: string[], vids: string[], docs: string[], memo: string | null) =>
      submit("evidence", "appendEvidence", [parentId, imgs, vids, docs, memo]),
    updateEvidenceManifest: (evidenceId: number, imgs: string[], vids: string[], docs: string[], memo: string | null) =>
      submit("evidence", "updateEvidenceManifest", [evidenceId, imgs, vids, docs, memo]),
    link: (domain: number, targetId: number, id: number) =>
      submit("evidence", "link", [domain, targetId, id]),
    linkByNs: (ns: string, subjectId: number, id: number) =>
      submit("evidence", "linkByNs", [ns, subjectId, id]),
    unlink: (domain: number, targetId: number, id: number) =>
      submit("evidence", "unlink", [domain, targetId, id]),
    unlinkByNs: (ns: string, subjectId: number, id: number) =>
      submit("evidence", "unlinkByNs", [ns, subjectId, id]),
    sealEvidence: (evidenceId: number) =>
      submit("evidence", "sealEvidence", [evidenceId]),
    unsealEvidence: (evidenceId: number) =>
      submit("evidence", "unsealEvidence", [evidenceId]),
    withdrawEvidence: (evidenceId: number) =>
      submit("evidence", "withdrawEvidence", [evidenceId]),
    forceRemoveEvidence: (evidenceId: number) =>
      submit("evidence", "forceRemoveEvidence", [evidenceId]),
    forceArchiveEvidence: (evidenceId: number) =>
      submit("evidence", "forceArchiveEvidence", [evidenceId]),
    revealCommitment: (evidenceId: number, cid: string, salt: string, version: number) =>
      submit("evidence", "revealCommitment", [evidenceId, cid, salt, version]),

    // Private content
    storePrivateContent: (ns: string, subjectId: number, cid: string, contentHash: string, encryptionMethod: string, accessPolicy: unknown, encryptedKeys: unknown) =>
      submit("evidence", "storePrivateContent", [ns, subjectId, cid, contentHash, encryptionMethod, accessPolicy, encryptedKeys]),
    grantAccess: (contentId: number, user: string, encryptedKey: string) =>
      submit("evidence", "grantAccess", [contentId, user, encryptedKey]),
    revokeAccess: (contentId: number, user: string) =>
      submit("evidence", "revokeAccess", [contentId, user]),
    requestAccess: (contentId: number) =>
      submit("evidence", "requestAccess", [contentId]),
    cancelAccessRequest: (contentId: number) =>
      submit("evidence", "cancelAccessRequest", [contentId]),
    deletePrivateContent: (contentId: number) =>
      submit("evidence", "deletePrivateContent", [contentId]),
    rotateContentKeys: (contentId: number, newContentHash: string, newEncryptedKeys: [string, string][]) =>
      submit("evidence", "rotateContentKeys", [contentId, newContentHash, newEncryptedKeys]),

    // Key management
    registerPublicKey: (keyData: string, keyType: string) =>
      submit("evidence", "registerPublicKey", [keyData, keyType]),
    revokePublicKey: () =>
      submit("evidence", "revokePublicKey", []),

    txState: state,
    resetTx: reset,
  };
}
