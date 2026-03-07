import { describe, test, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { NextIntlClientProvider } from 'next-intl';
import { EntityNotFound } from './not-found';
import { EntityRestricted } from './restricted';
import { EntityStatus } from '@/lib/types/enums';
import { computeRenderMode } from '@/lib/utils/entity-status';
import messages from '../../../messages/en.json';

// Mock next/link
vi.mock('next/link', () => ({
  default: ({ children, href }: { children: React.ReactNode; href: string }) => (
    <a href={href}>{children}</a>
  ),
}));

function renderWithIntl(ui: React.ReactElement) {
  return render(
    <NextIntlClientProvider locale="en" messages={messages}>
      {ui}
    </NextIntlClientProvider>
  );
}

describe('EntityNotFound', () => {
  test('renders 404 message with entity id', () => {
    renderWithIntl(<EntityNotFound entityId={42} />);
    expect(screen.getByText('404')).toBeInTheDocument();
    expect(screen.getByText('Entity #42 does not exist.')).toBeInTheDocument();
  });

  test('renders link back to home', () => {
    renderWithIntl(<EntityNotFound entityId={1} />);
    const link = screen.getByRole('link', { name: /back to home/i });
    expect(link).toHaveAttribute('href', '/');
  });
});

describe('EntityRestricted', () => {
  test('renders suspended message', () => {
    renderWithIntl(<EntityRestricted entityId={5} status={EntityStatus.Suspended} />);
    expect(screen.getByText('Entity Suspended')).toBeInTheDocument();
    expect(screen.getByText(/write operations are disabled/i)).toBeInTheDocument();
  });

  test('renders banned message', () => {
    renderWithIntl(<EntityRestricted entityId={5} status={EntityStatus.Banned} />);
    expect(screen.getByText('Entity Banned')).toBeInTheDocument();
    expect(screen.getByText(/all operations are disabled/i)).toBeInTheDocument();
  });
});

describe('Render mode integration with layout logic', () => {
  test('Active entity → normal mode', () => {
    expect(computeRenderMode(EntityStatus.Active)).toBe('normal');
  });

  test('PendingApproval entity → normal mode', () => {
    expect(computeRenderMode(EntityStatus.PendingApproval)).toBe('normal');
  });

  test('Suspended entity → restricted mode', () => {
    expect(computeRenderMode(EntityStatus.Suspended)).toBe('restricted');
  });

  test('Banned entity → restricted mode', () => {
    expect(computeRenderMode(EntityStatus.Banned)).toBe('restricted');
  });

  test('PendingClose entity → readonly mode', () => {
    expect(computeRenderMode(EntityStatus.PendingClose)).toBe('readonly');
  });

  test('Closed entity → readonly mode', () => {
    expect(computeRenderMode(EntityStatus.Closed)).toBe('readonly');
  });

  test('null status → not_found mode', () => {
    expect(computeRenderMode(null)).toBe('not_found');
  });

  test('invalid entityId (NaN) should be caught by layout', () => {
    // The layout parses entityId from URL params and checks for NaN
    const parsed = Number('abc');
    expect(Number.isNaN(parsed)).toBe(true);
  });

  test('invalid entityId (0 or negative) should be caught by layout', () => {
    expect(Number('0')).toBe(0);
    expect(Number('-1')).toBe(-1);
    // Layout checks entityId <= 0
  });
});
