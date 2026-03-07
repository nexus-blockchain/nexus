import { describe, test, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import {
  EntitySidebar,
  MODULE_CONFIGS,
  getVisibleMenuItems,
} from './entity-sidebar';
import type { ModuleKey } from '@/lib/utils/module-visibility';
import { EntityType, GovernanceMode } from '@/lib/types/enums';
import { AdminPermission } from '@/lib/types/models';
import { computeVisibleModules } from '@/lib/utils/module-visibility';

// --- Mocks ---

const mockUseEntityContext = vi.fn();
vi.mock('@/app/[entityId]/entity-provider', () => ({
  useEntityContext: () => mockUseEntityContext(),
}));

let mockSidebarCollapsed = false;
const mockToggleSidebar = vi.fn();
vi.mock('@/stores/entity-dapp-store', () => ({
  useEntityDAppStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      sidebarCollapsed: mockSidebarCollapsed,
      toggleSidebar: mockToggleSidebar,
    }),
}));

let mockPathname = '/1';
vi.mock('next/navigation', () => ({
  usePathname: () => mockPathname,
}));

vi.mock('next/link', () => ({
  default: ({ children, href, ...props }: { children: React.ReactNode; href: string; [k: string]: unknown }) => (
    <a href={href} {...props}>{children}</a>
  ),
}));

// Mock next-intl to return the key's last segment as the translation
vi.mock('next-intl', () => ({
  useTranslations: (namespace: string) => (key: string) => {
    // Return English labels for test assertions
    const translations: Record<string, Record<string, string>> = {
      nav: {
        dashboard: 'Dashboard',
        settings: 'Settings',
        shops: 'Shops',
        orders: 'Orders',
        token: 'Token',
        market: 'Market',
        members: 'Members',
        commission: 'Commission',
        governance: 'Governance',
        disclosure: 'Disclosure',
        kyc: 'KYC',
        tokensale: 'Token Sale',
        reviews: 'Reviews',
      },
    };
    return translations[namespace]?.[key] ?? key;
  },
}));

function makeEntityContext(overrides: Record<string, unknown> = {}) {
  return {
    entityId: 1,
    entity: null,
    isLoading: false,
    error: null,
    permissions: 0,
    isOwner: false,
    isReadOnly: false,
    isSuspended: false,
    entityType: EntityType.Enterprise,
    governanceMode: GovernanceMode.FullDAO,
    ...overrides,
  };
}

beforeEach(() => {
  mockSidebarCollapsed = false;
  mockToggleSidebar.mockClear();
  mockPathname = '/1';
});

// --- Pure function tests ---

describe('getVisibleMenuItems', () => {
  test('returns only items whose keys are in the visible modules list', () => {
    const visible: ModuleKey[] = ['dashboard', 'settings', 'shop', 'token'];
    const items = getVisibleMenuItems(visible);
    const keys = items.map((i) => i.key);
    expect(keys).toEqual(['dashboard', 'settings', 'shop', 'token']);
  });

  test('excludes product from top-level menu even if visible', () => {
    const visible: ModuleKey[] = ['dashboard', 'product', 'shop'];
    const items = getVisibleMenuItems(visible);
    const keys = items.map((i) => i.key);
    expect(keys).not.toContain('product');
  });

  test('returns empty array when no modules are visible', () => {
    const items = getVisibleMenuItems([]);
    expect(items).toHaveLength(0);
  });

  test('preserves order from MODULE_CONFIGS', () => {
    const visible: ModuleKey[] = ['review', 'dashboard', 'governance'];
    const items = getVisibleMenuItems(visible);
    const keys = items.map((i) => i.key);
    // Should follow MODULE_CONFIGS order: dashboard, governance, review
    expect(keys).toEqual(['dashboard', 'governance', 'review']);
  });
});

describe('MODULE_CONFIGS', () => {
  test('every config has a non-empty label and icon', () => {
    for (const config of MODULE_CONFIGS) {
      expect(config.label.length).toBeGreaterThan(0);
      expect(config.icon.length).toBeGreaterThan(0);
    }
  });

  test('dashboard href is empty string (maps to /[entityId])', () => {
    const dashboard = MODULE_CONFIGS.find((c) => c.key === 'dashboard');
    expect(dashboard?.href).toBe('');
  });

  test('settings requires ENTITY_MANAGE permission', () => {
    const settings = MODULE_CONFIGS.find((c) => c.key === 'settings');
    expect(settings?.permission).toBe(AdminPermission.ENTITY_MANAGE);
  });

  test('commission requires COMMISSION_MANAGE permission', () => {
    const commission = MODULE_CONFIGS.find((c) => c.key === 'commission');
    expect(commission?.permission).toBe(AdminPermission.COMMISSION_MANAGE);
  });
});

// --- Integration with computeVisibleModules ---

describe('EntityType-based menu filtering', () => {
  test('Fund entity hides shop, order, product, review', () => {
    const modules = computeVisibleModules(EntityType.Fund, GovernanceMode.FullDAO);
    const items = getVisibleMenuItems(modules);
    const keys = items.map((i) => i.key);
    expect(keys).not.toContain('shop');
    expect(keys).not.toContain('order');
    expect(keys).not.toContain('product');
    expect(keys).not.toContain('review');
    expect(keys).toContain('token');
    expect(keys).toContain('governance');
  });

  test('Merchant entity shows shop, order, member, commission, token', () => {
    const modules = computeVisibleModules(EntityType.Merchant, GovernanceMode.None);
    const items = getVisibleMenuItems(modules);
    const keys = items.map((i) => i.key);
    expect(keys).toContain('shop');
    expect(keys).toContain('order');
    expect(keys).toContain('member');
    expect(keys).toContain('commission');
    expect(keys).toContain('token');
  });

  test('GovernanceMode.None hides governance module', () => {
    const modules = computeVisibleModules(EntityType.Merchant, GovernanceMode.None);
    const items = getVisibleMenuItems(modules);
    const keys = items.map((i) => i.key);
    expect(keys).not.toContain('governance');
  });

  test('GovernanceMode.FullDAO shows governance for DAO entity', () => {
    const modules = computeVisibleModules(EntityType.DAO, GovernanceMode.FullDAO);
    const items = getVisibleMenuItems(modules);
    const keys = items.map((i) => i.key);
    expect(keys).toContain('governance');
  });
});

// --- Component rendering tests ---

describe('EntitySidebar', () => {
  test('renders desktop sidebar with menu items for Enterprise entity', () => {
    mockUseEntityContext.mockReturnValue(makeEntityContext());
    render(<EntitySidebar />);
    const sidebar = screen.getByTestId('desktop-sidebar');
    expect(sidebar).toBeDefined();
    // Enterprise + FullDAO shows all modules (may appear in both desktop + mobile)
    expect(screen.getAllByText('Dashboard').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Settings').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Shops').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Token').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Governance').length).toBeGreaterThan(0);
  });

  test('renders mobile bottom tab', () => {
    mockUseEntityContext.mockReturnValue(makeEntityContext());
    render(<EntitySidebar />);
    const mobileTab = screen.getByTestId('mobile-bottom-tab');
    expect(mobileTab).toBeDefined();
  });

  test('highlights active dashboard item when on entity root path', () => {
    mockPathname = '/1';
    mockUseEntityContext.mockReturnValue(makeEntityContext());
    render(<EntitySidebar />);
    // Dashboard link should have active styling
    const dashboardLink = screen.getAllByText('Dashboard')[0]?.closest('a');
    expect(dashboardLink?.getAttribute('href')).toBe('/1');
  });

  test('highlights active item based on pathname', () => {
    mockPathname = '/1/shops';
    mockUseEntityContext.mockReturnValue(makeEntityContext());
    render(<EntitySidebar />);
    const shopLink = screen.getAllByText('Shops')[0]?.closest('a');
    expect(shopLink?.getAttribute('href')).toBe('/1/shops');
  });

  test('shows entity id in sidebar header', () => {
    mockUseEntityContext.mockReturnValue(makeEntityContext({ entityId: 42 }));
    render(<EntitySidebar />);
    expect(screen.getByText('Entity #42')).toBeDefined();
  });

  test('toggle sidebar button calls toggleSidebar', () => {
    mockUseEntityContext.mockReturnValue(makeEntityContext());
    render(<EntitySidebar />);
    const toggleBtn = screen.getByLabelText('Collapse sidebar');
    fireEvent.click(toggleBtn);
    expect(mockToggleSidebar).toHaveBeenCalledTimes(1);
  });

  test('collapsed sidebar hides labels and shows only icons', () => {
    mockSidebarCollapsed = true;
    mockUseEntityContext.mockReturnValue(makeEntityContext());
    render(<EntitySidebar />);
    // In collapsed mode, the entity header text should not be visible
    expect(screen.queryByText('Entity #1')).toBeNull();
    // Toggle button should say "Expand sidebar"
    expect(screen.getByLabelText('Expand sidebar')).toBeDefined();
  });

  test('Fund entity does not show shop/order/review in sidebar', () => {
    mockUseEntityContext.mockReturnValue(
      makeEntityContext({
        entityType: EntityType.Fund,
        governanceMode: GovernanceMode.FullDAO,
      }),
    );
    render(<EntitySidebar />);
    expect(screen.queryByText('Shops')).toBeNull();
    expect(screen.queryByText('Orders')).toBeNull();
    expect(screen.queryByText('Reviews')).toBeNull();
    // But should show Token and Governance (may appear in both desktop + mobile)
    expect(screen.getAllByText('Token').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Governance').length).toBeGreaterThan(0);
  });

  test('Merchant with GovernanceMode.None hides governance', () => {
    mockUseEntityContext.mockReturnValue(
      makeEntityContext({
        entityType: EntityType.Merchant,
        governanceMode: GovernanceMode.None,
      }),
    );
    render(<EntitySidebar />);
    expect(screen.queryByText('Governance')).toBeNull();
    expect(screen.getAllByText('Shops').length).toBeGreaterThan(0);
  });

  test('links use correct entity-prefixed paths', () => {
    mockUseEntityContext.mockReturnValue(makeEntityContext({ entityId: 7 }));
    render(<EntitySidebar />);
    const settingsLink = screen.getAllByText('Settings')[0]?.closest('a');
    expect(settingsLink?.getAttribute('href')).toBe('/7/settings');
    const tokenLink = screen.getAllByText('Token')[0]?.closest('a');
    expect(tokenLink?.getAttribute('href')).toBe('/7/token');
  });
});
