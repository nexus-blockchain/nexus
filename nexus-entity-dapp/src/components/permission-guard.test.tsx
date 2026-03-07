import { describe, test, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { PermissionGuard, useHasPermission } from './permission-guard';
import { AdminPermission } from '@/lib/types/models';

// Mock useEntityContext to control permissions in tests
const mockUseEntityContext = vi.fn();
vi.mock('@/app/[entityId]/entity-provider', () => ({
  useEntityContext: () => mockUseEntityContext(),
}));

function makeContext(permissions: number) {
  return {
    entityId: 1,
    entity: null,
    isLoading: false,
    error: null,
    permissions,
    isOwner: permissions === AdminPermission.ALL,
    isReadOnly: false,
    isSuspended: false,
    entityType: 'Merchant',
    governanceMode: 'None',
  };
}

describe('PermissionGuard', () => {
  test('renders children when user has required permission', () => {
    mockUseEntityContext.mockReturnValue(makeContext(AdminPermission.SHOP_MANAGE));
    render(
      <PermissionGuard required={AdminPermission.SHOP_MANAGE}>
        <span>Shop Content</span>
      </PermissionGuard>,
    );
    expect(screen.getByText('Shop Content')).toBeInTheDocument();
  });

  test('renders children when user has multiple permissions including required', () => {
    mockUseEntityContext.mockReturnValue(
      makeContext(AdminPermission.SHOP_MANAGE | AdminPermission.TOKEN_MANAGE),
    );
    render(
      <PermissionGuard required={AdminPermission.TOKEN_MANAGE}>
        <span>Token Content</span>
      </PermissionGuard>,
    );
    expect(screen.getByText('Token Content')).toBeInTheDocument();
  });

  test('Owner (ALL) always passes any permission check', () => {
    mockUseEntityContext.mockReturnValue(makeContext(AdminPermission.ALL));
    render(
      <PermissionGuard required={AdminPermission.GOVERNANCE_MANAGE}>
        <span>Governance Content</span>
      </PermissionGuard>,
    );
    expect(screen.getByText('Governance Content')).toBeInTheDocument();
  });

  test('renders default "Permission denied" when user lacks permission and no fallback', () => {
    mockUseEntityContext.mockReturnValue(makeContext(0));
    render(
      <PermissionGuard required={AdminPermission.SHOP_MANAGE}>
        <span>Hidden</span>
      </PermissionGuard>,
    );
    expect(screen.queryByText('Hidden')).not.toBeInTheDocument();
    expect(screen.getByText('Permission denied')).toBeInTheDocument();
  });

  test('renders fallback content when user lacks permission', () => {
    mockUseEntityContext.mockReturnValue(makeContext(AdminPermission.MEMBER_MANAGE));
    render(
      <PermissionGuard
        required={AdminPermission.TOKEN_MANAGE}
        fallback={<span>No access to tokens</span>}
      >
        <span>Token Content</span>
      </PermissionGuard>,
    );
    expect(screen.queryByText('Token Content')).not.toBeInTheDocument();
    expect(screen.getByText('No access to tokens')).toBeInTheDocument();
  });

  test('renders fallback even when fallback is null (no default message)', () => {
    mockUseEntityContext.mockReturnValue(makeContext(0));
    const { container } = render(
      <PermissionGuard required={AdminPermission.SHOP_MANAGE} fallback={null}>
        <span>Hidden</span>
      </PermissionGuard>,
    );
    expect(screen.queryByText('Hidden')).not.toBeInTheDocument();
    expect(screen.queryByText('Permission denied')).not.toBeInTheDocument();
    expect(container.innerHTML).toBe('');
  });

  test('checks combined permission bits correctly', () => {
    const required = AdminPermission.SHOP_MANAGE | AdminPermission.ORDER_MANAGE;
    // User only has SHOP_MANAGE, missing ORDER_MANAGE
    mockUseEntityContext.mockReturnValue(makeContext(AdminPermission.SHOP_MANAGE));
    render(
      <PermissionGuard required={required}>
        <span>Combined Content</span>
      </PermissionGuard>,
    );
    expect(screen.queryByText('Combined Content')).not.toBeInTheDocument();
    expect(screen.getByText('Permission denied')).toBeInTheDocument();
  });

  test('passes when user has all combined required bits', () => {
    const required = AdminPermission.SHOP_MANAGE | AdminPermission.ORDER_MANAGE;
    mockUseEntityContext.mockReturnValue(
      makeContext(AdminPermission.SHOP_MANAGE | AdminPermission.ORDER_MANAGE | AdminPermission.MEMBER_MANAGE),
    );
    render(
      <PermissionGuard required={required}>
        <span>Combined Content</span>
      </PermissionGuard>,
    );
    expect(screen.getByText('Combined Content')).toBeInTheDocument();
  });
});

// Helper component to test the hook
function PermissionDisplay({ required }: { required: number }) {
  const has = useHasPermission(required);
  return <span>{has ? 'yes' : 'no'}</span>;
}

describe('useHasPermission', () => {
  test('returns true when user has the required permission', () => {
    mockUseEntityContext.mockReturnValue(makeContext(AdminPermission.KYC_MANAGE));
    render(<PermissionDisplay required={AdminPermission.KYC_MANAGE} />);
    expect(screen.getByText('yes')).toBeInTheDocument();
  });

  test('returns false when user lacks the required permission', () => {
    mockUseEntityContext.mockReturnValue(makeContext(0));
    render(<PermissionDisplay required={AdminPermission.KYC_MANAGE} />);
    expect(screen.getByText('no')).toBeInTheDocument();
  });

  test('returns true for Owner regardless of required permission', () => {
    mockUseEntityContext.mockReturnValue(makeContext(AdminPermission.ALL));
    render(<PermissionDisplay required={AdminPermission.COMMISSION_MANAGE} />);
    expect(screen.getByText('yes')).toBeInTheDocument();
  });

  test('returns false for zero permissions with any non-zero required', () => {
    mockUseEntityContext.mockReturnValue(makeContext(0));
    render(<PermissionDisplay required={AdminPermission.ENTITY_MANAGE} />);
    expect(screen.getByText('no')).toBeInTheDocument();
  });
});
