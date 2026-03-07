import { describe, test, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { NextIntlClientProvider } from 'next-intl';
import { DashboardPage } from './dashboard-client';
import type { EntityContext } from '@/lib/types/models';
import { EntityType, EntityStatus, GovernanceMode } from '@/lib/types/enums';
import messages from '../../../messages/en.json';

const mockUseEntityContext = vi.fn();
vi.mock('./entity-provider', () => ({
  useEntityContext: () => mockUseEntityContext(),
}));

function renderWithIntl(ui: React.ReactElement) {
  return render(
    <NextIntlClientProvider locale="en" messages={messages}>
      {ui}
    </NextIntlClientProvider>
  );
}

function makeContext(overrides: Partial<EntityContext> = {}): EntityContext {
  return {
    entityId: 1,
    entity: {
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
      fundBalance: BigInt('50000000000000'), // 50 NEX
      createdAt: 100,
    },
    isLoading: false,
    error: null,
    permissions: 0,
    isOwner: false,
    isReadOnly: false,
    isSuspended: false,
    entityType: EntityType.Merchant,
    governanceMode: GovernanceMode.None,
    ...overrides,
  };
}

describe('DashboardPage', () => {
  test('shows loading spinner when isLoading is true', () => {
    mockUseEntityContext.mockReturnValue(makeContext({ isLoading: true, entity: null }));
    const { container } = renderWithIntl(<DashboardPage />);
    expect(container.querySelector('.animate-spin')).toBeTruthy();
  });

  test('renders entity name and type', () => {
    mockUseEntityContext.mockReturnValue(makeContext());
    renderWithIntl(<DashboardPage />);
    expect(screen.getByText('Test Entity')).toBeInTheDocument();
    expect(screen.getByText('Merchant')).toBeInTheDocument();
  });

  test('renders fund balance stat card', () => {
    mockUseEntityContext.mockReturnValue(makeContext());
    renderWithIntl(<DashboardPage />);
    expect(screen.getByText('Fund Balance')).toBeInTheDocument();
    expect(screen.getByText('50 NEX')).toBeInTheDocument();
  });

  test('shows fund warning when balance is below threshold', () => {
    const ctx = makeContext();
    ctx.entity = { ...ctx.entity!, fundBalance: BigInt('5000000000000') }; // 5 NEX < 10 NEX threshold
    mockUseEntityContext.mockReturnValue(ctx);
    renderWithIntl(<DashboardPage />);
    expect(
      screen.getByText('Fund balance is below the warning threshold. Please top up to avoid service disruption.'),
    ).toBeInTheDocument();
    expect(screen.getByText('Low balance')).toBeInTheDocument();
  });

  test('does not show fund warning when balance is above threshold', () => {
    mockUseEntityContext.mockReturnValue(makeContext()); // 50 NEX > 10 NEX
    renderWithIntl(<DashboardPage />);
    expect(
      screen.queryByText('Fund balance is below the warning threshold. Please top up to avoid service disruption.'),
    ).not.toBeInTheDocument();
  });

  test('renders placeholder stat cards for members, orders, and token', () => {
    mockUseEntityContext.mockReturnValue(makeContext());
    renderWithIntl(<DashboardPage />);
    expect(screen.getByText('Total Members')).toBeInTheDocument();
    expect(screen.getByText('Total Orders')).toBeInTheDocument();
    expect(screen.getByText('Token')).toBeInTheDocument();
  });

  test('renders chart placeholder areas', () => {
    mockUseEntityContext.mockReturnValue(makeContext());
    renderWithIntl(<DashboardPage />);
    expect(screen.getByText('Sales Trend')).toBeInTheDocument();
    expect(screen.getByText('Member Growth')).toBeInTheDocument();
    expect(screen.getByText('Token Price Trend')).toBeInTheDocument();
  });

  test('shows verified badge when entity is verified', () => {
    const ctx = makeContext();
    ctx.entity = { ...ctx.entity!, verified: true };
    mockUseEntityContext.mockReturnValue(ctx);
    renderWithIntl(<DashboardPage />);
    expect(screen.getByText('✓ Verified')).toBeInTheDocument();
  });

  test('shows FullDAO governance mode label', () => {
    const ctx = makeContext();
    ctx.entity = { ...ctx.entity!, governanceMode: GovernanceMode.FullDAO };
    mockUseEntityContext.mockReturnValue(ctx);
    renderWithIntl(<DashboardPage />);
    expect(screen.getByText('Full DAO')).toBeInTheDocument();
  });

  test('shows entity status badge', () => {
    mockUseEntityContext.mockReturnValue(makeContext());
    renderWithIntl(<DashboardPage />);
    expect(screen.getByText('Active')).toBeInTheDocument();
  });

  test('formats zero balance correctly', () => {
    const ctx = makeContext();
    ctx.entity = { ...ctx.entity!, fundBalance: BigInt(0) };
    mockUseEntityContext.mockReturnValue(ctx);
    renderWithIntl(<DashboardPage />);
    expect(screen.getByText('0 NEX')).toBeInTheDocument();
  });

  test('formats balance with decimals correctly', () => {
    const ctx = makeContext();
    // 1.5 NEX = 1_500_000_000_000
    ctx.entity = { ...ctx.entity!, fundBalance: BigInt('1500000000000') };
    mockUseEntityContext.mockReturnValue(ctx);
    renderWithIntl(<DashboardPage />);
    expect(screen.getByText('1.5 NEX')).toBeInTheDocument();
  });
});
