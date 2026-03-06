"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type {
  BotInfo,
  AttestationRecord,
  PeerEndpoint,
  CommunityBinding,
} from "@/lib/types";

export function useBots(owner: string | null) {
  const [bots, setBots] = useState<BotInfo[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!owner) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;

      const idsRaw = await q.groupRobotRegistry.ownerBots(owner);
      const idHashes = (idsRaw.toJSON() || []) as string[];

      const results = await Promise.all(
        idHashes.map(async (botIdHash: string) => {
          const raw = await q.groupRobotRegistry.bots(botIdHash);
          return raw.isNone ? null : (raw.toJSON() as BotInfo);
        })
      );

      setBots(results.filter(Boolean) as BotInfo[]);
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [owner]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { bots, isLoading, refetch: fetch };
}

export function useBot(botIdHash: string | null) {
  const [bot, setBot] = useState<BotInfo | null>(null);
  const [attestation, setAttestation] = useState<AttestationRecord | null>(null);
  const [peers, setPeers] = useState<PeerEndpoint[]>([]);
  const [communityBindings, setCommunityBindings] = useState<CommunityBinding[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!botIdHash) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;

      const [botRaw, attRaw, peersRaw] = await Promise.all([
        q.groupRobotRegistry.bots(botIdHash),
        q.groupRobotRegistry.attestationsV2(botIdHash),
        q.groupRobotRegistry.peerRegistry(botIdHash),
      ]);

      setBot(botRaw.isNone ? null : (botRaw.toJSON() as BotInfo));
      setAttestation(attRaw.isNone ? null : (attRaw.toJSON() as AttestationRecord));
      setPeers((peersRaw.toJSON() || []) as PeerEndpoint[]);

      const bindingEntries = await q.groupRobotRegistry.communityBindings.entries();
      const bindings = bindingEntries
        .map(([_k, v]: [unknown, { toJSON: () => CommunityBinding }]) => v.toJSON())
        .filter((b: CommunityBinding) => b.botIdHash === botIdHash);
      setCommunityBindings(bindings);
    } catch {
      /* ignore */
    } finally {
      setIsLoading(false);
    }
  }, [botIdHash]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { bot, attestation, peers, communityBindings, isLoading, refetch: fetch };
}

export function useBotActions() {
  const { submit, state, reset } = useTx();
  return {
    registerBot: (botIdHash: string, publicKey: string) =>
      submit("groupRobotRegistry", "registerBot", [botIdHash, publicKey]),
    updatePublicKey: (botIdHash: string, newPublicKey: string) =>
      submit("groupRobotRegistry", "updatePublicKey", [botIdHash, newPublicKey]),
    deactivateBot: (botIdHash: string) =>
      submit("groupRobotRegistry", "deactivateBot", [botIdHash]),
    bindCommunity: (botIdHash: string, communityIdHash: string, platform: string) =>
      submit("groupRobotRegistry", "bindCommunity", [botIdHash, communityIdHash, platform]),
    unbindCommunity: (communityIdHash: string) =>
      submit("groupRobotRegistry", "unbindCommunity", [communityIdHash]),
    submitTeeAttestation: (botIdHash: string, quote: string, platformId: string | null) =>
      submit("groupRobotRegistry", "submitTeeAttestation", [botIdHash, quote, platformId]),
    registerPeer: (botIdHash: string, endpoint: string) =>
      submit("groupRobotRegistry", "registerPeer", [botIdHash, endpoint]),
    heartbeatPeer: (botIdHash: string, peerPublicKey: string) =>
      submit("groupRobotRegistry", "heartbeatPeer", [botIdHash, peerPublicKey]),
    txState: state,
    resetTx: reset,
  };
}
