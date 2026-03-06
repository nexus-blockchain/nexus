"use client";

import { useEntityStore } from "@/stores/entity";
import { useWalletStore } from "@/stores/wallet";
import { ShieldAlert } from "lucide-react";

interface PermissionGuardProps {
  requiredBits?: number;
  ownerOnly?: boolean;
  fallback?: React.ReactNode;
  children: React.ReactNode;
}

const DEFAULT_FALLBACK = (
  <div className="flex flex-col items-center justify-center py-16 text-center">
    <ShieldAlert className="h-12 w-12 text-muted-foreground/40" />
    <h3 className="mt-4 text-lg font-semibold">Insufficient Permissions</h3>
    <p className="mt-1 text-sm text-muted-foreground">
      You do not have permission to access this section.
    </p>
  </div>
);

export function PermissionGuard({ requiredBits, ownerOnly, fallback = DEFAULT_FALLBACK, children }: PermissionGuardProps) {
  const { address } = useWalletStore();
  const { currentEntityId, currentEntity, hasPermission } = useEntityStore();

  if (!address) {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-center">
        <ShieldAlert className="h-12 w-12 text-muted-foreground/40" />
        <h3 className="mt-4 text-lg font-semibold">Wallet Not Connected</h3>
        <p className="mt-1 text-sm text-muted-foreground">
          Connect your wallet to access this section.
        </p>
      </div>
    );
  }

  if (!currentEntityId) {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-center">
        <ShieldAlert className="h-12 w-12 text-muted-foreground/40" />
        <h3 className="mt-4 text-lg font-semibold">No Entity Selected</h3>
        <p className="mt-1 text-sm text-muted-foreground">
          Select an entity from the header to continue.
        </p>
      </div>
    );
  }

  if (ownerOnly && currentEntity && currentEntity.owner !== address) {
    return <>{fallback}</>;
  }

  if (requiredBits !== undefined && !hasPermission(currentEntityId, requiredBits)) {
    if (!currentEntity || currentEntity.owner !== address) {
      return <>{fallback}</>;
    }
  }

  return <>{children}</>;
}
