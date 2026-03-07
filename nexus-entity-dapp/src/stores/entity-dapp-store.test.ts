import { describe, test, expect, beforeEach } from 'vitest';
import { useEntityDAppStore } from './entity-dapp-store';
import { EntityType, EntityStatus, GovernanceMode } from '@/lib/types/enums';
import type { EntityData, Notification } from '@/lib/types/models';

function makeEntity(overrides: Partial<EntityData> = {}): EntityData {
  return {
    id: 1,
    owner: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
    name: 'Test Entity',
    logoCid: null,
    descriptionCid: null,
    metadataUri: null,
    status: EntityStatus.Active,
    entityType: EntityType.Merchant,
    governanceMode: GovernanceMode.None,
    verified: false,
    governanceLocked: false,
    fundBalance: BigInt(1000),
    createdAt: 100,
    ...overrides,
  };
}

function makeNotification(overrides: Partial<Notification> = {}): Notification {
  return {
    id: 'n-1',
    pallet: 'entityRegistry',
    event: 'EntityCreated',
    summary: 'Entity created',
    timestamp: Date.now(),
    read: false,
    ...overrides,
  };
}

describe('useEntityDAppStore', () => {
  beforeEach(() => {
    useEntityDAppStore.setState({
      currentEntityId: null,
      currentEntity: null,
      permissions: 0,
      isOwner: false,
      visibleModules: ['dashboard', 'settings'],
      sidebarCollapsed: false,
      notifications: [],
      unreadCount: 0,
    });
  });

  test('should have correct initial state', () => {
    const state = useEntityDAppStore.getState();
    expect(state.currentEntityId).toBeNull();
    expect(state.currentEntity).toBeNull();
    expect(state.permissions).toBe(0);
    expect(state.isOwner).toBe(false);
    expect(state.visibleModules).toEqual(['dashboard', 'settings']);
    expect(state.sidebarCollapsed).toBe(false);
    expect(state.notifications).toEqual([]);
    expect(state.unreadCount).toBe(0);
  });

  test('setEntityContext sets entity data and computes visible modules', () => {
    const entity = makeEntity({ entityType: EntityType.Enterprise, governanceMode: GovernanceMode.FullDAO });
    useEntityDAppStore.getState().setEntityContext(entity, 0x001, false);

    const state = useEntityDAppStore.getState();
    expect(state.currentEntityId).toBe(entity.id);
    expect(state.currentEntity).toBe(entity);
    expect(state.permissions).toBe(0x001);
    expect(state.isOwner).toBe(false);
    // Enterprise + FullDAO should include governance and many modules
    expect(state.visibleModules).toContain('dashboard');
    expect(state.visibleModules).toContain('governance');
    expect(state.visibleModules).toContain('token');
  });

  test('setEntityContext hides governance when GovernanceMode is None', () => {
    const entity = makeEntity({ entityType: EntityType.Merchant, governanceMode: GovernanceMode.None });
    useEntityDAppStore.getState().setEntityContext(entity, 0xFFFFFFFF, true);

    const state = useEntityDAppStore.getState();
    expect(state.isOwner).toBe(true);
    expect(state.visibleModules).not.toContain('governance');
  });

  test('clearEntityContext resets to defaults', () => {
    const entity = makeEntity();
    useEntityDAppStore.getState().setEntityContext(entity, 0x001, true);
    useEntityDAppStore.getState().clearEntityContext();

    const state = useEntityDAppStore.getState();
    expect(state.currentEntityId).toBeNull();
    expect(state.currentEntity).toBeNull();
    expect(state.permissions).toBe(0);
    expect(state.isOwner).toBe(false);
    expect(state.visibleModules).toEqual(['dashboard', 'settings']);
  });

  test('toggleSidebar flips sidebarCollapsed', () => {
    expect(useEntityDAppStore.getState().sidebarCollapsed).toBe(false);
    useEntityDAppStore.getState().toggleSidebar();
    expect(useEntityDAppStore.getState().sidebarCollapsed).toBe(true);
    useEntityDAppStore.getState().toggleSidebar();
    expect(useEntityDAppStore.getState().sidebarCollapsed).toBe(false);
  });

  test('setSidebarCollapsed sets explicit value', () => {
    useEntityDAppStore.getState().setSidebarCollapsed(true);
    expect(useEntityDAppStore.getState().sidebarCollapsed).toBe(true);
    useEntityDAppStore.getState().setSidebarCollapsed(false);
    expect(useEntityDAppStore.getState().sidebarCollapsed).toBe(false);
  });

  test('addNotification prepends and increments unreadCount', () => {
    const n1 = makeNotification({ id: 'n-1' });
    const n2 = makeNotification({ id: 'n-2' });

    useEntityDAppStore.getState().addNotification(n1);
    expect(useEntityDAppStore.getState().notifications).toHaveLength(1);
    expect(useEntityDAppStore.getState().unreadCount).toBe(1);

    useEntityDAppStore.getState().addNotification(n2);
    expect(useEntityDAppStore.getState().notifications).toHaveLength(2);
    expect(useEntityDAppStore.getState().notifications[0].id).toBe('n-2');
    expect(useEntityDAppStore.getState().unreadCount).toBe(2);
  });

  test('addNotification caps at 100 notifications', () => {
    for (let i = 0; i < 105; i++) {
      useEntityDAppStore.getState().addNotification(makeNotification({ id: `n-${i}` }));
    }
    expect(useEntityDAppStore.getState().notifications).toHaveLength(100);
  });

  test('markNotificationRead marks specific notification and updates unreadCount', () => {
    useEntityDAppStore.getState().addNotification(makeNotification({ id: 'a' }));
    useEntityDAppStore.getState().addNotification(makeNotification({ id: 'b' }));
    expect(useEntityDAppStore.getState().unreadCount).toBe(2);

    useEntityDAppStore.getState().markNotificationRead('a');
    const state = useEntityDAppStore.getState();
    expect(state.notifications.find((n) => n.id === 'a')?.read).toBe(true);
    expect(state.notifications.find((n) => n.id === 'b')?.read).toBe(false);
    expect(state.unreadCount).toBe(1);
  });

  test('clearAllNotifications empties list and resets count', () => {
    useEntityDAppStore.getState().addNotification(makeNotification({ id: 'x' }));
    useEntityDAppStore.getState().addNotification(makeNotification({ id: 'y' }));
    useEntityDAppStore.getState().clearAllNotifications();

    const state = useEntityDAppStore.getState();
    expect(state.notifications).toEqual([]);
    expect(state.unreadCount).toBe(0);
  });
});
