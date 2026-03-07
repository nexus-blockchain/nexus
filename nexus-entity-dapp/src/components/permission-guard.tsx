'use client';

import React from 'react';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { hasPermission } from '@/lib/utils/permissions';

export interface PermissionGuardProps {
  required: number;
  fallback?: React.ReactNode;
  children: React.ReactNode;
}

/**
 * Controls child component visibility based on AdminPermission bitmask.
 * Renders children if the current user has the required permission bits,
 * otherwise renders fallback content or a default "permission denied" message.
 */
export function PermissionGuard({ required, fallback, children }: PermissionGuardProps) {
  const { permissions } = useEntityContext();

  if (hasPermission(permissions, required)) {
    return <>{children}</>;
  }

  if (fallback !== undefined) {
    return <>{fallback}</>;
  }

  return (
    <div className="flex items-center justify-center rounded-md border border-dashed p-6 text-sm text-muted-foreground">
      Permission denied
    </div>
  );
}

/**
 * Hook for programmatic permission checks.
 * Returns true if the current user has the required permission bits.
 */
export function useHasPermission(required: number): boolean {
  const { permissions } = useEntityContext();
  return hasPermission(permissions, required);
}
