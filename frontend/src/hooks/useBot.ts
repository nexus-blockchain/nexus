"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { BotInfo, AttestationRecord, PeerEndpoint, CommunityBinding, OperatorInfo } from "@/lib/types";

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
          if (raw.isNone) return null;
          const b = raw.toJSON() as BotInfo;
          b.botIdHash = botIdHash;
          return b;
        })
      );
      setBots(results.filter(Boolean) as BotInfo[]);
    } catch { /* ignore */ }
    finally { setIsLoading(false); }
  }, [owner]);

  useEffect(() => { fetch(); }, [fetch]);
  return { bots, isLoading, refetch: fetch };
}

export function useBotCount() {
  const [count, setCount] = useState(0);
  const fetch = useCallback(async () => {
    try {
      const api = await getApi();
      const raw = await (api.query as any).groupRobotRegistry.botCount();
      setCount((raw.toJSON() ?? 0) as number);
    } catch { /* ignore */ }
  }, []);
  useEffect(() => { fetch(); }, [fetch]);
  return { count, refetch: fetch };
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
    } catch { /* ignore */ }
    finally { setIsLoading(false); }
  }, [botIdHash]);

  useEffect(() => { fetch(); }, [fetch]);
  return { bot, attestation, peers, communityBindings, isLoading, refetch: fetch };
}

export function useOperators() {
  const [operators, setOperators] = useState<(OperatorInfo & { account: string })[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).groupRobotRegistry.operators.entries();
      const all = entries.map(
        ([k, v]: [{ args: [{ toString: () => string }, { toString: () => string }] }, { toJSON: () => OperatorInfo }]) => {
          const op = v.toJSON();
          return { ...op, account: k.args[0].toString() };
        }
      ).filter(Boolean) as (OperatorInfo & { account: string })[];
      setOperators(all);
    } catch { /* ignore */ }
    finally { setIsLoading(false); }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { operators, isLoading, refetch: fetch };
}

export function useBotActions() {
  const { submit, state, reset } = useTx();
  return {
    registerBot: (botIdHash: string, publicKey: string) =>
      submit("groupRobotRegistry", "registerBot", [botIdHash, publicKey]),
    updatePublicKey: (botIdHash: string, newKey: string) =>
      submit("groupRobotRegistry", "updatePublicKey", [botIdHash, newKey]),
    deactivateBot: (botIdHash: string) =>
      submit("groupRobotRegistry", "deactivateBot", [botIdHash]),
    reactivateBot: (botIdHash: string) =>
      submit("groupRobotRegistry", "reactivateBot", [botIdHash]),
    suspendBot: (botIdHash: string) =>
      submit("groupRobotRegistry", "suspendBot", [botIdHash]),
    transferBotOwnership: (botIdHash: string, newOwner: string) =>
      submit("groupRobotRegistry", "transferBotOwnership", [botIdHash, newOwner]),
    forceDeactivateBot: (botIdHash: string) =>
      submit("groupRobotRegistry", "forceDeactivateBot", [botIdHash]),
    cleanupDeactivatedBot: (botIdHash: string) =>
      submit("groupRobotRegistry", "cleanupDeactivatedBot", [botIdHash]),

    bindCommunity: (botIdHash: string, communityIdHash: string, platform: string) =>
      submit("groupRobotRegistry", "bindCommunity", [botIdHash, communityIdHash, platform]),
    unbindCommunity: (communityIdHash: string) =>
      submit("groupRobotRegistry", "unbindCommunity", [communityIdHash]),
    bindUserPlatform: (platform: string, platformUserIdHash: string) =>
      submit("groupRobotRegistry", "bindUserPlatform", [platform, platformUserIdHash]),
    unbindUserPlatform: (platform: string) =>
      submit("groupRobotRegistry", "unbindUserPlatform", [platform]),

    submitTeeAttestation: (botIdHash: string, quoteRaw: string, platformId: string | null, pckCertDer: string | null, intermediateCertDer: string | null) =>
      submit("groupRobotRegistry", "submitTeeAttestation", [botIdHash, quoteRaw, platformId, pckCertDer, intermediateCertDer]),
    refreshAttestation: (botIdHash: string, tdxQuoteHash: string, sgxQuoteHash: string | null, mrtd: string, mrenclave: string | null) =>
      submit("groupRobotRegistry", "refreshAttestation", [botIdHash, tdxQuoteHash, sgxQuoteHash, mrtd, mrenclave]),
    forceExpireAttestation: (botIdHash: string) =>
      submit("groupRobotRegistry", "forceExpireAttestation", [botIdHash]),

    registerPeer: (botIdHash: string, peerPublicKey: string, endpoint: string) =>
      submit("groupRobotRegistry", "registerPeer", [botIdHash, peerPublicKey, endpoint]),
    deregisterPeer: (botIdHash: string, peerPublicKey: string) =>
      submit("groupRobotRegistry", "deregisterPeer", [botIdHash, peerPublicKey]),
    heartbeatPeer: (botIdHash: string, peerPublicKey: string) =>
      submit("groupRobotRegistry", "heartbeatPeer", [botIdHash, peerPublicKey]),
    updatePeerEndpoint: (botIdHash: string, peerPublicKey: string, newEndpoint: string) =>
      submit("groupRobotRegistry", "updatePeerEndpoint", [botIdHash, peerPublicKey, newEndpoint]),
    reportStalePeer: (botIdHash: string, peerPublicKey: string) =>
      submit("groupRobotRegistry", "reportStalePeer", [botIdHash, peerPublicKey]),

    registerOperator: (platform: string, platformAppHash: string, name: string, contact: string) =>
      submit("groupRobotRegistry", "registerOperator", [platform, platformAppHash, name, contact]),
    updateOperator: (platform: string, name: string, contact: string) =>
      submit("groupRobotRegistry", "updateOperator", [platform, name, contact]),
    deregisterOperator: (platform: string) =>
      submit("groupRobotRegistry", "deregisterOperator", [platform]),
    setOperatorSla: (operator: string, platform: string, slaLevel: number) =>
      submit("groupRobotRegistry", "setOperatorSla", [operator, platform, slaLevel]),
    assignBotToOperator: (botIdHash: string, platform: string) =>
      submit("groupRobotRegistry", "assignBotToOperator", [botIdHash, platform]),
    unassignBotFromOperator: (botIdHash: string) =>
      submit("groupRobotRegistry", "unassignBotFromOperator", [botIdHash]),
    operatorUnassignBot: (botIdHash: string) =>
      submit("groupRobotRegistry", "operatorUnassignBot", [botIdHash]),
    suspendOperator: (operator: string, platform: string) =>
      submit("groupRobotRegistry", "suspendOperator", [operator, platform]),
    unsuspendOperator: (operator: string, platform: string) =>
      submit("groupRobotRegistry", "unsuspendOperator", [operator, platform]),

    approveMrtd: (mrtd: string, version: number) =>
      submit("groupRobotRegistry", "approveMrtd", [mrtd, version]),
    revokeMrtd: (mrtd: string) =>
      submit("groupRobotRegistry", "revokeMrtd", [mrtd]),
    approveMrenclave: (mrenclave: string, version: number) =>
      submit("groupRobotRegistry", "approveMrenclave", [mrenclave, version]),
    revokeMrenclave: (mrenclave: string) =>
      submit("groupRobotRegistry", "revokeMrenclave", [mrenclave]),

    txState: state,
    resetTx: reset,
  };
}
