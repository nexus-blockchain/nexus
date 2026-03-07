'use client';

import React, { useMemo } from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useEntityDAppStore } from '@/stores/entity-dapp-store';
import { computeVisibleModules, type ModuleKey } from '@/lib/utils/module-visibility';
import { AdminPermission } from '@/lib/types/models';
import { LocaleSwitcher } from '@/components/locale-switcher';
import { useTranslations } from 'next-intl';

export interface ModuleConfig {
  key: ModuleKey;
  label: string;
  href: string; // relative to /[entityId]
  icon: string; // emoji placeholder
  permission?: number;
}

/**
 * Module configuration array mapping ModuleKey to label, route, icon, and optional permission.
 * `product` is nested under shop and excluded from top-level menu.
 */
export const MODULE_CONFIGS: ModuleConfig[] = [
  { key: 'dashboard', label: 'Dashboard', href: '', icon: '📊' },
  { key: 'settings', label: 'Settings', href: '/settings', icon: '⚙️', permission: AdminPermission.ENTITY_MANAGE },
  { key: 'shop', label: 'Shops', href: '/shops', icon: '🏪' },
  { key: 'order', label: 'Orders', href: '/orders', icon: '📦' },
  { key: 'token', label: 'Token', href: '/token', icon: '🪙' },
  { key: 'market', label: 'Market', href: '/market', icon: '📈' },
  { key: 'member', label: 'Members', href: '/members', icon: '👥' },
  { key: 'commission', label: 'Commission', href: '/commission', icon: '💰', permission: AdminPermission.COMMISSION_MANAGE },
  { key: 'governance', label: 'Governance', href: '/governance', icon: '🗳️' },
  { key: 'disclosure', label: 'Disclosure', href: '/disclosure', icon: '📄' },
  { key: 'kyc', label: 'KYC', href: '/kyc', icon: '🔐' },
  { key: 'tokensale', label: 'Token Sale', href: '/tokensale', icon: '🚀' },
  { key: 'review', label: 'Reviews', href: '/reviews', icon: '⭐' },
];

/** Top modules shown in mobile bottom tab (max 5 for usability) */
const MOBILE_TAB_KEYS: ModuleKey[] = ['dashboard', 'shop', 'order', 'member', 'token'];

/**
 * Filters MODULE_CONFIGS to only include visible modules based on EntityType and GovernanceMode.
 * Excludes `product` since it's nested under shop.
 */
export function getVisibleMenuItems(
  visibleModules: ModuleKey[],
): ModuleConfig[] {
  const visibleSet = new Set(visibleModules);
  return MODULE_CONFIGS.filter(
    (config) => config.key !== 'product' && visibleSet.has(config.key),
  );
}

interface SidebarItemProps {
  item: ModuleConfig;
  entityId: number;
  isActive: boolean;
  collapsed: boolean;
  label: string;
}

function SidebarItem({ item, entityId, isActive, collapsed, label }: SidebarItemProps) {
  const basePath = `/${entityId}`;
  const fullHref = item.href ? `${basePath}${item.href}` : basePath;

  return (
    <Link
      href={fullHref}
      className={`flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors ${
        isActive
          ? 'bg-blue-50 text-blue-700 font-medium'
          : 'text-gray-700 hover:bg-gray-100'
      } ${collapsed ? 'justify-center' : ''}`}
      title={collapsed ? label : undefined}
    >
      <span className="text-base flex-shrink-0">{item.icon}</span>
      {!collapsed && <span>{label}</span>}
    </Link>
  );
}

interface MobileTabItemProps {
  item: ModuleConfig;
  entityId: number;
  isActive: boolean;
  label: string;
}

function MobileTabItem({ item, entityId, isActive, label }: MobileTabItemProps) {
  const basePath = `/${entityId}`;
  const fullHref = item.href ? `${basePath}${item.href}` : basePath;

  return (
    <Link
      href={fullHref}
      className={`flex flex-1 flex-col items-center gap-0.5 py-2 text-xs transition-colors ${
        isActive ? 'text-blue-700 font-medium' : 'text-gray-500'
      }`}
    >
      <span className="text-lg">{item.icon}</span>
      <span className="truncate">{label}</span>
    </Link>
  );
}

/** Map module key to nav i18n key */
const MODULE_NAV_KEY: Record<string, string> = {
  dashboard: 'dashboard',
  settings: 'settings',
  shop: 'shops',
  order: 'orders',
  token: 'token',
  market: 'market',
  member: 'members',
  commission: 'commission',
  governance: 'governance',
  disclosure: 'disclosure',
  kyc: 'kyc',
  tokensale: 'tokensale',
  review: 'reviews',
};

export function EntitySidebar() {
  const { entityId, entityType, governanceMode } = useEntityContext();
  const sidebarCollapsed = useEntityDAppStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useEntityDAppStore((s) => s.toggleSidebar);
  const pathname = usePathname();
  const t = useTranslations('nav');

  const visibleModules = useMemo(
    () => computeVisibleModules(entityType, governanceMode),
    [entityType, governanceMode],
  );

  const menuItems = useMemo(
    () => getVisibleMenuItems(visibleModules),
    [visibleModules],
  );

  const mobileItems = useMemo(
    () => menuItems.filter((item) => MOBILE_TAB_KEYS.includes(item.key)),
    [menuItems],
  );

  const isActive = (item: ModuleConfig): boolean => {
    const basePath = `/${entityId}`;
    const fullHref = item.href ? `${basePath}${item.href}` : basePath;
    if (item.key === 'dashboard') {
      return pathname === basePath || pathname === `${basePath}/`;
    }
    return pathname.startsWith(fullHref);
  };

  return (
    <>
      {/* Desktop sidebar - hidden on mobile */}
      <aside
        data-testid="desktop-sidebar"
        className={`hidden md:flex flex-col border-r border-gray-200 bg-white transition-all ${
          sidebarCollapsed ? 'w-16' : 'w-56'
        }`}
      >
        <div className="flex items-center justify-between border-b border-gray-200 px-3 py-3">
          {!sidebarCollapsed && (
            <span className="text-sm font-semibold text-gray-800 truncate">
              Entity #{entityId}
            </span>
          )}
          <button
            onClick={toggleSidebar}
            className="rounded p-1 text-gray-500 hover:bg-gray-100"
            aria-label={sidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar'}
          >
            {sidebarCollapsed ? '→' : '←'}
          </button>
        </div>
        <nav className="flex-1 overflow-y-auto p-2 space-y-1">
          {menuItems.map((item) => (
            <SidebarItem
              key={item.key}
              item={item}
              entityId={entityId}
              isActive={isActive(item)}
              collapsed={sidebarCollapsed}
              label={t(MODULE_NAV_KEY[item.key] ?? item.key)}
            />
          ))}
        </nav>
        <div className="border-t border-gray-200 p-2">
          {!sidebarCollapsed && <LocaleSwitcher />}
        </div>
      </aside>

      {/* Mobile bottom tab - visible only on mobile */}
      <nav
        data-testid="mobile-bottom-tab"
        className="fixed bottom-0 left-0 right-0 z-50 flex border-t border-gray-200 bg-white md:hidden"
      >
        {mobileItems.map((item) => (
          <MobileTabItem
            key={item.key}
            item={item}
            entityId={entityId}
            isActive={isActive(item)}
            label={t(MODULE_NAV_KEY[item.key] ?? item.key)}
          />
        ))}
      </nav>
    </>
  );
}
