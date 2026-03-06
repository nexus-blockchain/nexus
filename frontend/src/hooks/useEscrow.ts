"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { EscrowData } from "@/lib/types";

export function useEscrowRecord(id: number | null) {
  const [escrow, setEscrow] = useState<EscrowData | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (id === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;
      const lockedRaw = await q.escrow.locked(id);
      const stateRaw = await q.escrow.lockStateOf(id);
      const nonceRaw = await q.escrow.lockNonces(id);
      const expiryRaw = await q.escrow.expiryOf(id);
      const disputedRaw = await q.escrow.disputedAt(id);

      const rawAmount = BigInt(lockedRaw.toString());
      const rawState = (stateRaw.toJSON() ?? 0) as number;
      if (rawAmount === 0n && rawState === 0) { setEscrow(null); return; }

      setEscrow({
        id,
        amount: rawAmount,
        state: rawState,
        nonce: (nonceRaw.toJSON() ?? 0) as number,
        expiresAt: expiryRaw.isNone ? null : (expiryRaw.toJSON() as number),
        disputedAt: disputedRaw.isNone ? null : (disputedRaw.toJSON() as number),
      });
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [id]);

  useEffect(() => { fetch(); }, [fetch]);
  return { escrow, isLoading, refetch: fetch };
}

export function useEscrowBatch(ids: number[]) {
  const [escrows, setEscrows] = useState<EscrowData[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (ids.length === 0) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;

      const results = await Promise.all(
        ids.map(async (id) => {
          const lockedRaw = await q.escrow.locked(id);
          const stateRaw = await q.escrow.lockStateOf(id);
          const amount = lockedRaw.isNone ? null : BigInt(lockedRaw.toString());
          if (amount === null) return null;
          return { id, amount, state: (stateRaw.toJSON() ?? 0) as number, nonce: 0, expiresAt: null, disputedAt: null } as EscrowData;
        })
      );
      setEscrows(results.filter(Boolean) as EscrowData[]);
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [ids]);

  useEffect(() => { fetch(); }, [fetch]);
  return { escrows, isLoading, refetch: fetch };
}

export function useEscrowPaused() {
  const [paused, setPaused] = useState(false);
  const fetch = useCallback(async () => {
    try {
      const api = await getApi();
      const raw = await (api.query as any).escrow.paused();
      setPaused((raw.toJSON() ?? false) as boolean);
    } catch { /* ignore */ }
  }, []);
  useEffect(() => { fetch(); }, [fetch]);
  return { paused, refetch: fetch };
}

export function useEscrowActions() {
  const { submit, state, reset } = useTx();
  return {
    lock: (id: number, payer: string, amount: bigint) =>
      submit("escrow", "lock", [id, payer, amount]),
    lockWithNonce: (id: number, payer: string, amount: bigint, nonce: number) =>
      submit("escrow", "lockWithNonce", [id, payer, amount, nonce]),
    release: (id: number, to: string) =>
      submit("escrow", "release", [id, to]),
    refund: (id: number, to: string) =>
      submit("escrow", "refund", [id, to]),
    releaseSplit: (id: number, entries: [string, bigint][]) =>
      submit("escrow", "releaseSplit", [id, entries]),
    releasePartial: (id: number, to: string, amount: bigint) =>
      submit("escrow", "releasePartial", [id, to, amount]),
    refundPartial: (id: number, to: string, amount: bigint) =>
      submit("escrow", "refundPartial", [id, to, amount]),
    dispute: (id: number, reason: number, detail: string) =>
      submit("escrow", "dispute", [id, reason, detail]),
    applyDecisionReleaseAll: (id: number, to: string) =>
      submit("escrow", "applyDecisionReleaseAll", [id, to]),
    applyDecisionRefundAll: (id: number, to: string) =>
      submit("escrow", "applyDecisionRefundAll", [id, to]),
    applyDecisionPartialBps: (id: number, releaseTo: string, refundTo: string, bps: number) =>
      submit("escrow", "applyDecisionPartialBps", [id, releaseTo, refundTo, bps]),
    scheduleExpiry: (id: number, at: number) =>
      submit("escrow", "scheduleExpiry", [id, at]),
    cancelExpiry: (id: number) =>
      submit("escrow", "cancelExpiry", [id]),
    forceRelease: (id: number, to: string) =>
      submit("escrow", "forceRelease", [id, to]),
    forceRefund: (id: number, to: string) =>
      submit("escrow", "forceRefund", [id, to]),
    setPause: (paused: boolean) =>
      submit("escrow", "setPause", [paused]),
    cleanupClosed: (ids: number[]) =>
      submit("escrow", "cleanupClosed", [ids]),
    tokenLock: (entityId: number, escrowId: number, payer: string, amount: bigint) =>
      submit("escrow", "tokenLock", [entityId, escrowId, payer, amount]),
    tokenRelease: (entityId: number, escrowId: number, to: string, amount: bigint) =>
      submit("escrow", "tokenRelease", [entityId, escrowId, to, amount]),
    tokenRefund: (entityId: number, escrowId: number, to: string, amount: bigint) =>
      submit("escrow", "tokenRefund", [entityId, escrowId, to, amount]),

    txState: state,
    resetTx: reset,
  };
}
