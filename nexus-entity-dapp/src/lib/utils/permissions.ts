import { AdminPermission } from '@/lib/types';

/** Check if permission_bits contains the required permission */
export function hasPermission(permissionBits: number, required: number): boolean {
  return (permissionBits & required) === required;
}

/** Check if permission_bits represents Owner (ALL permissions) */
export function isOwnerPermission(permissionBits: number): boolean {
  return permissionBits === AdminPermission.ALL;
}
