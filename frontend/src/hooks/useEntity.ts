"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { EntityData } from "@/lib/types";
import { useEntityStore } from "@/stores/entity";

function parseEntity(raw: Record<string, unknown>): EntityData {
  return {
    id: Number(raw.id),
    owner: String(raw.owner),
    name: new TextDecoder().decode(new Uint8Array(raw.name as number[])),
    logoCid: raw.logoCid ? String(raw.logoCid) : null,
    descriptionCid: raw.descriptionCid ? String(raw.descriptionCid) : null,
    status: String(raw.status),
    createdAt: Number(raw.createdAt),
    entityType: typeof raw.entityType === "object" ? JSON.stringify(raw.entityType) : String(raw.entityType),
    admins: Array.isArray(raw.admins)
      ? raw.admins.map((a: [string, number]) => ({ address: String(a[0]), permissions: Number(a[1]) }))
      : [],
    governanceMode: String(raw.governanceMode),
    verified: Boolean(raw.verified),
    metadataUri: raw.metadataUri ? String(raw.metadataUri) : null,
    primaryShopId: Number(raw.primaryShopId),
    totalSales: BigInt(String(raw.totalSales || 0)),
    totalOrders: Number(raw.totalOrders),
  };
}

export function useEntity(entityId: number | null) {
  const [data, setData] = useState<EntityData | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    setError(null);
    try {
      const api = await getApi();
      const raw = await (api.query as Record<string, Record<string, (id: number) => Promise<{ toJSON: () => Record<string, unknown>; isNone?: boolean }>>>)
        .entityRegistry.entities(entityId);
      if (raw.isNone) {
        setData(null);
      } else {
        const json = raw.toJSON() as Record<string, unknown>;
        setData(parseEntity(json));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to fetch entity");
    } finally {
      setIsLoading(false);
    }
  }, [entityId]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { data, isLoading, error, refetch: fetch };
}

export function useEntityActions(entityId: number) {
  const { submit, state, reset } = useTx();

  const updateEntity = useCallback(
    (params: { name?: string; logoCid?: string | null; descriptionCid?: string | null; metadataUri?: string | null }) =>
      submit("entityRegistry", "updateEntity", [
        entityId,
        params.name || null,
        params.logoCid,
        params.descriptionCid,
        params.metadataUri,
      ]),
    [entityId, submit]
  );

  const topUpFund = useCallback(
    (amount: bigint) => submit("entityRegistry", "topUpFund", [entityId, amount]),
    [entityId, submit]
  );

  const addAdmin = useCallback(
    (admin: string, permissions: number) =>
      submit("entityRegistry", "addAdmin", [entityId, admin, permissions]),
    [entityId, submit]
  );

  const removeAdmin = useCallback(
    (admin: string) => submit("entityRegistry", "removeAdmin", [entityId, admin]),
    [entityId, submit]
  );

  const updateAdminPermissions = useCallback(
    (admin: string, permissions: number) =>
      submit("entityRegistry", "updateAdminPermissions", [entityId, admin, permissions]),
    [entityId, submit]
  );

  const transferOwnership = useCallback(
    (newOwner: string) => submit("entityRegistry", "transferOwnership", [entityId, newOwner]),
    [entityId, submit]
  );

  const requestClose = useCallback(
    () => submit("entityRegistry", "requestCloseEntity", [entityId]),
    [entityId, submit]
  );

  const reopenEntity = useCallback(
    () => submit("entityRegistry", "reopenEntity", [entityId]),
    [entityId, submit]
  );

  const upgradeType = useCallback(
    (newType: string, newGovernance?: string) =>
      submit("entityRegistry", "upgradeEntityType", [entityId, newType, newGovernance || null]),
    [entityId, submit]
  );

  const bindReferrer = useCallback(
    (referrer: string) => submit("entityRegistry", "bindEntityReferrer", [entityId, referrer]),
    [entityId, submit]
  );

  return {
    updateEntity,
    topUpFund,
    addAdmin,
    removeAdmin,
    updateAdminPermissions,
    transferOwnership,
    requestClose,
    reopenEntity,
    upgradeType,
    bindReferrer,
    txState: state,
    resetTx: reset,
  };
}

export function useRegisterEntity() {
  const { submit, state, reset } = useTx();

  const registerEntity = useCallback(
    (name: string, entityType: string, referrer: string | null) =>
      submit("entityRegistry", "registerEntity", [name, entityType, referrer]),
    [submit]
  );

  return { registerEntity, txState: state, resetTx: reset };
}

export function useUserEntities(address: string | null) {
  const [entities, setEntities] = useState<Array<{ id: number; name: string; status: string; entityType: string }>>([]);
  const [isLoading, setIsLoading] = useState(false);
  const setUserEntities = useEntityStore((s) => s.setUserEntities);

  useEffect(() => {
    if (!address) return;
    let cancelled = false;

    const load = async () => {
      setIsLoading(true);
      try {
        const api = await getApi();
        const ids = await (api.query as Record<string, Record<string, (addr: string) => Promise<{ toJSON: () => number[] }>>>)
          .entityRegistry.userEntity(address);
        const idList = ids.toJSON() as number[];

        const results = await Promise.all(
          idList.map(async (id) => {
            const raw = await (api.query as Record<string, Record<string, (id: number) => Promise<{ toJSON: () => Record<string, unknown> }>>>)
              .entityRegistry.entities(id);
            const json = raw.toJSON() as Record<string, unknown>;
            return {
              id: Number(json.id),
              name: new TextDecoder().decode(new Uint8Array(json.name as number[])),
              status: String(json.status),
              entityType: typeof json.entityType === "object" ? JSON.stringify(json.entityType) : String(json.entityType),
            };
          })
        );

        if (!cancelled) {
          setEntities(results);
          setUserEntities(results);
        }
      } catch {
        // ignore
      } finally {
        if (!cancelled) setIsLoading(false);
      }
    };

    load();
    return () => { cancelled = true; };
  }, [address, setUserEntities]);

  return { entities, isLoading };
}
