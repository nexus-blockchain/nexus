'use client';

import React, { createContext, useContext, useEffect, useMemo } from 'react';
import type { EntityContext, EntityData } from '@/lib/types/models';
import { EntityStatus, EntityType, GovernanceMode } from '@/lib/types/enums';
import { useEntityQuery } from '@/hooks/use-entity-query';
import { isOwnerPermission } from '@/lib/utils/permissions';
import { STALE_TIMES } from '@/lib/chain/constants';
import { useWalletStore } from '@/stores/wallet-store';
import { useEntityDAppStore } from '@/stores/entity-dapp-store';

const EntityCtx = createContext<EntityContext | null>(null);

function parseEntityData(raw: unknown): EntityData | null {
  if (!raw || (raw as { isNone?: boolean }).isNone) return null;
  const unwrapped = (raw as { unwrapOr?: (d: null) => unknown }).unwrapOr?.(null) ?? raw;
  if (!unwrapped) return null;
  const obj = unwrapped as Record<string, unknown>;
  return {
    id: Number(obj.id ?? 0),
    owner: String(obj.owner ?? ''),
    name: String(obj.name ?? ''),
    logoCid: obj.logoCid ? String(obj.logoCid) : null,
    descriptionCid: obj.descriptionCid ? String(obj.descriptionCid) : null,
    metadataUri: obj.metadataUri ? String(obj.metadataUri) : null,
    status: String(obj.status ?? 'Active') as EntityStatus,
    entityType: String(obj.entityType ?? 'Merchant') as EntityType,
    governanceMode: String(obj.governanceMode ?? 'None') as GovernanceMode,
    verified: Boolean(obj.verified),
    governanceLocked: Boolean(obj.governanceLocked),
    fundBalance: BigInt(String(obj.fundBalance ?? 0)),
    createdAt: Number(obj.createdAt ?? 0),
  };
}

function parsePermissions(raw: unknown): number {
  if (!raw || (raw as { isNone?: boolean }).isNone) return 0;
  const unwrapped = (raw as { unwrapOr?: (d: number) => unknown }).unwrapOr?.(0) ?? raw;
  return Number(unwrapped);
}

interface EntityProviderProps {
  entityId: number;
  children: React.ReactNode;
}

export function EntityProvider({ entityId, children }: EntityProviderProps) {
  const walletAddress = useWalletStore((s) => s.address);
  const setEntityContext = useEntityDAppStore((s) => s.setEntityContext);
  const clearEntityContext = useEntityDAppStore((s) => s.clearEntityContext);

  // Query entity basic info
  const {
    data: entity,
    isLoading: entityLoading,
    error: entityError,
  } = useEntityQuery<EntityData | null>(
    ['entity', entityId],
    async (api) => {
      const raw = await api.query.entityRegistry.entities(entityId);
      return parseEntityData(raw);
    },
    { staleTime: STALE_TIMES.entity },
  );

  // Query current user's admin permissions
  const {
    data: permissions,
    isLoading: permissionsLoading,
  } = useEntityQuery<number>(
    ['entity', entityId, 'permissions', walletAddress],
    async (api) => {
      if (!walletAddress) return 0;
      const raw = await api.query.entityRegistry.entityAdmins(entityId, walletAddress);
      return parsePermissions(raw);
    },
    {
      staleTime: STALE_TIMES.entity,
      enabled: !!walletAddress,
    },
  );

  const isOwner = useMemo(
    () => isOwnerPermission(permissions ?? 0),
    [permissions],
  );

  const isReadOnly = useMemo(() => {
    if (!entity) return false;
    return entity.status === EntityStatus.PendingClose || entity.status === EntityStatus.Closed;
  }, [entity]);

  const isSuspended = useMemo(() => {
    if (!entity) return false;
    return entity.status === EntityStatus.Suspended || entity.status === EntityStatus.Banned;
  }, [entity]);

  // Sync to Zustand store
  useEffect(() => {
    if (entity) {
      setEntityContext(entity, permissions ?? 0, isOwner);
    }
    return () => {
      clearEntityContext();
    };
  }, [entity, permissions, isOwner, setEntityContext, clearEntityContext]);

  const value = useMemo<EntityContext>(
    () => ({
      entityId,
      entity: entity ?? null,
      isLoading: entityLoading || permissionsLoading,
      error: entityError ? String(entityError) : null,
      permissions: permissions ?? 0,
      isOwner,
      isReadOnly,
      isSuspended,
      entityType: entity?.entityType ?? EntityType.Merchant,
      governanceMode: entity?.governanceMode ?? GovernanceMode.None,
    }),
    [entityId, entity, entityLoading, permissionsLoading, entityError, permissions, isOwner, isReadOnly, isSuspended],
  );

  return <EntityCtx.Provider value={value}>{children}</EntityCtx.Provider>;
}

/** Hook to access the EntityContext. Must be used within EntityProvider. */
export function useEntityContext(): EntityContext {
  const ctx = useContext(EntityCtx);
  if (!ctx) {
    throw new Error('useEntityContext must be used within an EntityProvider');
  }
  return ctx;
}
