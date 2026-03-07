import { create } from 'zustand';
import type { EntityData, Notification } from '@/lib/types/models';
import { computeVisibleModules, type ModuleKey } from '@/lib/utils/module-visibility';

interface EntityDAppStore {
  // Current Entity context
  currentEntityId: number | null;
  currentEntity: EntityData | null;
  permissions: number;
  isOwner: boolean;

  // Menu state
  visibleModules: ModuleKey[];
  sidebarCollapsed: boolean;

  // Notifications
  notifications: Notification[];
  unreadCount: number;

  // Actions
  setEntityContext: (entity: EntityData, permissions: number, isOwner: boolean) => void;
  clearEntityContext: () => void;
  toggleSidebar: () => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  addNotification: (notification: Notification) => void;
  markNotificationRead: (id: string) => void;
  clearAllNotifications: () => void;
}

export const useEntityDAppStore = create<EntityDAppStore>((set) => ({
  currentEntityId: null,
  currentEntity: null,
  permissions: 0,
  isOwner: false,
  visibleModules: ['dashboard', 'settings'],
  sidebarCollapsed: false,
  notifications: [],
  unreadCount: 0,

  setEntityContext: (entity, permissions, isOwner) => {
    const modules = computeVisibleModules(entity.entityType, entity.governanceMode);
    set({
      currentEntityId: entity.id,
      currentEntity: entity,
      permissions,
      isOwner,
      visibleModules: modules,
    });
  },

  clearEntityContext: () =>
    set({
      currentEntityId: null,
      currentEntity: null,
      permissions: 0,
      isOwner: false,
      visibleModules: ['dashboard', 'settings'],
    }),

  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),

  setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),

  addNotification: (notification) =>
    set((state) => ({
      notifications: [notification, ...state.notifications].slice(0, 100),
      unreadCount: state.unreadCount + 1,
    })),

  markNotificationRead: (id) =>
    set((state) => {
      const updated = state.notifications.map((n) =>
        n.id === id ? { ...n, read: true } : n,
      );
      return {
        notifications: updated,
        unreadCount: updated.filter((n) => !n.read).length,
      };
    }),

  clearAllNotifications: () => set({ notifications: [], unreadCount: 0 }),
}));
