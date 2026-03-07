import { EntityStatus, RenderMode } from '@/lib/types';

/** Map EntityStatus to RenderMode */
export function computeRenderMode(status: EntityStatus | null): RenderMode {
  if (status === null) return 'not_found';
  switch (status) {
    case EntityStatus.Active:
    case EntityStatus.PendingApproval:
      return 'normal';
    case EntityStatus.Suspended:
    case EntityStatus.Banned:
      return 'restricted';
    case EntityStatus.PendingClose:
    case EntityStatus.Closed:
      return 'readonly';
    default:
      return 'not_found';
  }
}
