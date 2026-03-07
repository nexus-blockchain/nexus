import { EntityStatus, ShopOperatingStatus, EffectiveShopStatus } from '@/lib/types';

/** Compute EffectiveShopStatus from EntityStatus × ShopOperatingStatus */
export function computeEffectiveShopStatus(
  entityStatus: EntityStatus,
  shopStatus: ShopOperatingStatus,
  fundDepleted: boolean = false,
): EffectiveShopStatus {
  // Entity-level overrides
  if (entityStatus === EntityStatus.Banned) return EffectiveShopStatus.Banned;
  if (entityStatus === EntityStatus.Closed) return EffectiveShopStatus.ClosedByEntity;
  if (entityStatus === EntityStatus.PendingClose) return EffectiveShopStatus.ClosedByEntity;
  if (entityStatus === EntityStatus.Suspended) return EffectiveShopStatus.PausedByEntity;

  // Shop-level status
  if (shopStatus === ShopOperatingStatus.Closed) return EffectiveShopStatus.Closed;
  if (shopStatus === ShopOperatingStatus.Closing) return EffectiveShopStatus.Closing;
  if (shopStatus === ShopOperatingStatus.Paused) return EffectiveShopStatus.PausedBySelf;

  // Fund check for active shops
  if (fundDepleted) return EffectiveShopStatus.FundDepleted;

  return EffectiveShopStatus.Active;
}
