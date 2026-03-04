"use client";

import { useEntityStore } from "@/stores/entity";
import { ShieldAlert } from "lucide-react";

interface RequirePermissionProps {
  entityId?: number | null;
  permission: number;
  children: React.ReactNode;
  fallback?: React.ReactNode;
  showFallback?: boolean;
}

export function RequirePermission({
  entityId,
  permission,
  children,
  fallback,
  showFallback = false,
}: RequirePermissionProps) {
  const { currentEntityId, hasPermission } = useEntityStore();
  const id = entityId ?? currentEntityId;

  if (id === null || id === undefined) return null;

  const allowed = hasPermission(id, permission);

  if (allowed) return <>{children}</>;

  if (fallback) return <>{fallback}</>;

  if (showFallback) {
    return (
      <div className="flex flex-col items-center justify-center gap-2 rounded-lg border border-dashed p-6 text-center">
        <ShieldAlert className="h-8 w-8 text-muted-foreground" />
        <p className="text-sm text-muted-foreground">
          You don&apos;t have permission to access this feature.
        </p>
      </div>
    );
  }

  return null;
}
