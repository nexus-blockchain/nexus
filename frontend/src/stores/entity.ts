import { create } from "zustand";
import type { EntityData } from "@/lib/types";

interface EntitySummary {
  id: number;
  name: string;
  status: string;
  entityType: string;
}

interface EntityStore {
  currentEntityId: number | null;
  setCurrentEntityId: (id: number | null) => void;
  userEntities: EntitySummary[];
  setUserEntities: (entities: EntitySummary[]) => void;
  currentEntity: EntityData | null;
  setCurrentEntity: (entity: EntityData | null) => void;
  permissions: Record<number, number>;
  setPermissions: (entityId: number, perms: number) => void;
  hasPermission: (entityId: number, required: number) => boolean;
}

export const useEntityStore = create<EntityStore>((set, get) => ({
  currentEntityId: null,
  setCurrentEntityId: (id) => set({ currentEntityId: id }),
  userEntities: [],
  setUserEntities: (entities) => set({ userEntities: entities }),
  currentEntity: null,
  setCurrentEntity: (entity) => set({ currentEntity: entity }),
  permissions: {},
  setPermissions: (entityId, perms) =>
    set((state) => ({
      permissions: { ...state.permissions, [entityId]: perms },
    })),
  hasPermission: (entityId, required) => {
    const perms = get().permissions[entityId];
    if (perms === undefined) return false;
    if (perms === 0xff) return true;
    return (perms & required) === required;
  },
}));
