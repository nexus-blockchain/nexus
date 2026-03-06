"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { ConsensusNode } from "@/lib/types";

export function useConsensusNodes() {
  const [nodes, setNodes] = useState<ConsensusNode[]>([]);
  const [activeList, setActiveList] = useState<string[]>([]);
  const [currentEra, setCurrentEra] = useState(0);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    setIsLoading(true);
    try {
      const api = await getApi();
      const q = api.query as any;

      const [eraRaw, listRaw] = await Promise.all([
        q.groupRobotConsensus.currentEra(),
        q.groupRobotConsensus.activeNodeList(),
      ]);
      setCurrentEra((eraRaw.toJSON() ?? 0) as number);
      const list = (listRaw.toJSON() || []) as string[];
      setActiveList(list);

      const entries = await q.groupRobotConsensus.nodes.entries();
      const all = entries.map(
        ([k, v]: [{ args: [{ toHex: () => string }] }, { toJSON: () => ConsensusNode }]) => {
          const n = v.toJSON();
          if (n) n.nodeId = k.args[0].toHex();
          return n;
        }
      ).filter(Boolean) as ConsensusNode[];
      setNodes(all);
    } catch { /* ignore */ }
    finally { setIsLoading(false); }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);
  return { nodes, activeList, currentEra, isLoading, refetch: fetch };
}

export function useConsensusNode(nodeId: string | null) {
  const [node, setNode] = useState<ConsensusNode | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (!nodeId) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).groupRobotConsensus.nodes(nodeId);
      setNode(raw.isNone ? null : (raw.toJSON() as ConsensusNode));
    } catch { /* ignore */ }
    finally { setIsLoading(false); }
  }, [nodeId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { node, isLoading, refetch: fetch };
}

export function useConsensusActions() {
  const { submit, state, reset } = useTx();
  return {
    registerNode: (nodeId: string, stake: bigint) =>
      submit("groupRobotConsensus", "registerNode", [nodeId, stake]),
    requestExit: (nodeId: string) =>
      submit("groupRobotConsensus", "requestExit", [nodeId]),
    finalizeExit: (nodeId: string) =>
      submit("groupRobotConsensus", "finalizeExit", [nodeId]),
    increaseStake: (nodeId: string, amount: bigint) =>
      submit("groupRobotConsensus", "increaseStake", [nodeId, amount]),
    reinstateNode: (nodeId: string) =>
      submit("groupRobotConsensus", "reinstateNode", [nodeId]),
    replaceOperator: (nodeId: string, newOperator: string) =>
      submit("groupRobotConsensus", "replaceOperator", [nodeId, newOperator]),
    unbindBot: (nodeId: string) =>
      submit("groupRobotConsensus", "unbindBot", [nodeId]),
    verifyNodeTee: (nodeId: string, botIdHash: string) =>
      submit("groupRobotConsensus", "verifyNodeTee", [nodeId, botIdHash]),
    reportEquivocation: (nodeId: string, sequence: number, msgHashA: string, sigA: string, msgHashB: string, sigB: string) =>
      submit("groupRobotConsensus", "reportEquivocation", [nodeId, sequence, msgHashA, sigA, msgHashB, sigB]),

    // Admin
    forceSuspendNode: (nodeId: string) =>
      submit("groupRobotConsensus", "forceSuspendNode", [nodeId]),
    forceRemoveNode: (nodeId: string) =>
      submit("groupRobotConsensus", "forceRemoveNode", [nodeId]),
    forceReinstateNode: (nodeId: string) =>
      submit("groupRobotConsensus", "forceReinstateNode", [nodeId]),
    setTeeRewardParams: (teeMultiplier: number, sgxBonus: number) =>
      submit("groupRobotConsensus", "setTeeRewardParams", [teeMultiplier, sgxBonus]),
    setSlashPercentage: (newPct: number) =>
      submit("groupRobotConsensus", "setSlashPercentage", [newPct]),
    setReporterRewardPct: (pct: number) =>
      submit("groupRobotConsensus", "setReporterRewardPct", [pct]),
    forceEraEnd: () =>
      submit("groupRobotConsensus", "forceEraEnd", []),

    txState: state,
    resetTx: reset,
  };
}
