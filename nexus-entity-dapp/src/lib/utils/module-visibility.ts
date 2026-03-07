import { EntityType, GovernanceMode } from '@/lib/types';

export type ModuleVisibility = 'required' | 'optional' | 'hidden';

export const MODULE_KEYS = [
  'dashboard', 'settings', 'shop', 'product', 'order', 'token', 'market',
  'member', 'commission', 'governance', 'disclosure', 'kyc', 'tokensale', 'review',
] as const;

export type ModuleKey = (typeof MODULE_KEYS)[number];

const ENTITY_TYPE_MODULE_MAP: Record<EntityType, Partial<Record<ModuleKey, ModuleVisibility>>> = {
  [EntityType.Merchant]: {
    shop: 'required', product: 'required', order: 'required', member: 'required',
    commission: 'required', token: 'required', governance: 'optional', disclosure: 'optional',
    review: 'required', market: 'optional', kyc: 'optional', tokensale: 'optional',
  },
  [EntityType.Enterprise]: {
    shop: 'required', product: 'required', order: 'required', member: 'required',
    commission: 'required', token: 'required', governance: 'required', disclosure: 'required',
    review: 'required', market: 'required', kyc: 'required', tokensale: 'required',
  },
  [EntityType.DAO]: {
    governance: 'required', token: 'required', market: 'required', member: 'required',
    disclosure: 'required', product: 'optional', order: 'optional', shop: 'optional',
    commission: 'optional', review: 'optional', kyc: 'optional', tokensale: 'optional',
  },
  [EntityType.Community]: {
    member: 'required', token: 'required', disclosure: 'required',
    product: 'optional', kyc: 'optional', shop: 'optional', order: 'optional',
    commission: 'optional', governance: 'optional', review: 'optional',
    market: 'optional', tokensale: 'optional',
  },
  [EntityType.Project]: {
    token: 'required', tokensale: 'required', governance: 'required',
    disclosure: 'required', kyc: 'required', member: 'optional', market: 'optional',
    shop: 'optional', product: 'optional', order: 'optional',
    commission: 'optional', review: 'optional',
  },
  [EntityType.ServiceProvider]: {
    shop: 'required', order: 'required', member: 'required', commission: 'required',
    token: 'optional', review: 'required', governance: 'optional', disclosure: 'optional',
    kyc: 'optional', market: 'optional', product: 'optional', tokensale: 'optional',
  },
  [EntityType.Fund]: {
    token: 'required', governance: 'required', market: 'required',
    disclosure: 'required', kyc: 'required', product: 'hidden', order: 'hidden',
    shop: 'hidden', member: 'optional', commission: 'optional',
    review: 'hidden', tokensale: 'optional',
  },
};

/** Compute visible modules based on EntityType and GovernanceMode */
export function computeVisibleModules(
  entityType: EntityType,
  governanceMode: GovernanceMode,
): ModuleKey[] {
  const map = ENTITY_TYPE_MODULE_MAP[entityType] || {};
  const modules: ModuleKey[] = ['dashboard', 'settings']; // always visible

  for (const key of MODULE_KEYS) {
    if (key === 'dashboard' || key === 'settings') continue;
    const visibility = map[key] ?? 'hidden';
    if (visibility === 'hidden') continue;

    // Hide governance voting when GovernanceMode is None
    if (key === 'governance' && governanceMode === GovernanceMode.None) continue;

    modules.push(key);
  }

  return modules;
}

export { ENTITY_TYPE_MODULE_MAP };
