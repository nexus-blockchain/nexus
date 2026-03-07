import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { NotificationCenter } from './notification-center';
import type { Notification } from '@/lib/types/models';

function makeNotification(overrides: Partial<Notification> = {}): Notification {
  return {
    id: 'test-1',
    pallet: 'entityShop',
    event: 'ShopCreated',
    summary: 'A new shop has been created',
    timestamp: Date.now() - 60_000, // 1 minute ago
    read: false,
    ...overrides,
  };
}

describe('NotificationCenter', () => {
  it('renders bell button', () => {
    render(
      <NotificationCenter
        notifications={[]}
        unreadCount={0}
        onMarkAsRead={vi.fn()}
        onClearAll={vi.fn()}
      />,
    );
    expect(screen.getByTestId('notification-bell')).toBeDefined();
  });

  it('shows unread badge when unreadCount > 0', () => {
    render(
      <NotificationCenter
        notifications={[makeNotification()]}
        unreadCount={3}
        onMarkAsRead={vi.fn()}
        onClearAll={vi.fn()}
      />,
    );
    const badge = screen.getByTestId('notification-badge');
    expect(badge.textContent).toBe('3');
  });

  it('does not show badge when unreadCount is 0', () => {
    render(
      <NotificationCenter
        notifications={[]}
        unreadCount={0}
        onMarkAsRead={vi.fn()}
        onClearAll={vi.fn()}
      />,
    );
    expect(screen.queryByTestId('notification-badge')).toBeNull();
  });

  it('shows 99+ when unreadCount exceeds 99', () => {
    render(
      <NotificationCenter
        notifications={[]}
        unreadCount={150}
        onMarkAsRead={vi.fn()}
        onClearAll={vi.fn()}
      />,
    );
    const badge = screen.getByTestId('notification-badge');
    expect(badge.textContent).toBe('99+');
  });

  it('toggles panel open/closed on bell click', () => {
    render(
      <NotificationCenter
        notifications={[]}
        unreadCount={0}
        onMarkAsRead={vi.fn()}
        onClearAll={vi.fn()}
      />,
    );
    expect(screen.queryByTestId('notification-panel')).toBeNull();

    fireEvent.click(screen.getByTestId('notification-bell'));
    expect(screen.getByTestId('notification-panel')).toBeDefined();

    fireEvent.click(screen.getByTestId('notification-bell'));
    expect(screen.queryByTestId('notification-panel')).toBeNull();
  });

  it('shows empty state when no notifications', () => {
    render(
      <NotificationCenter
        notifications={[]}
        unreadCount={0}
        onMarkAsRead={vi.fn()}
        onClearAll={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('notification-bell'));
    expect(screen.getByTestId('notification-empty')).toBeDefined();
  });

  it('renders notification items', () => {
    const n = makeNotification({ id: 'n1', summary: 'Test notification' });
    render(
      <NotificationCenter
        notifications={[n]}
        unreadCount={1}
        onMarkAsRead={vi.fn()}
        onClearAll={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('notification-bell'));
    expect(screen.getByText('Test notification')).toBeDefined();
    expect(screen.getByText('entityShop')).toBeDefined();
  });

  it('calls onMarkAsRead when mark-read button is clicked', () => {
    const onMarkAsRead = vi.fn();
    const n = makeNotification({ id: 'n1' });
    render(
      <NotificationCenter
        notifications={[n]}
        unreadCount={1}
        onMarkAsRead={onMarkAsRead}
        onClearAll={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('notification-bell'));
    fireEvent.click(screen.getByTestId('notification-mark-read-n1'));
    expect(onMarkAsRead).toHaveBeenCalledWith('n1');
  });

  it('does not show mark-read button for already-read notifications', () => {
    const n = makeNotification({ id: 'n1', read: true });
    render(
      <NotificationCenter
        notifications={[n]}
        unreadCount={0}
        onMarkAsRead={vi.fn()}
        onClearAll={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('notification-bell'));
    expect(screen.queryByTestId('notification-mark-read-n1')).toBeNull();
  });

  it('calls onClearAll when clear all button is clicked', () => {
    const onClearAll = vi.fn();
    const n = makeNotification();
    render(
      <NotificationCenter
        notifications={[n]}
        unreadCount={1}
        onMarkAsRead={vi.fn()}
        onClearAll={onClearAll}
      />,
    );
    fireEvent.click(screen.getByTestId('notification-bell'));
    fireEvent.click(screen.getByTestId('notification-clear-all'));
    expect(onClearAll).toHaveBeenCalledOnce();
  });

  it('unread notification has blue background', () => {
    const n = makeNotification({ id: 'n1', read: false });
    render(
      <NotificationCenter
        notifications={[n]}
        unreadCount={1}
        onMarkAsRead={vi.fn()}
        onClearAll={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('notification-bell'));
    const item = screen.getByTestId('notification-item-n1');
    expect(item.className).toContain('bg-blue-50');
  });

  it('read notification has white background', () => {
    const n = makeNotification({ id: 'n1', read: true });
    render(
      <NotificationCenter
        notifications={[n]}
        unreadCount={0}
        onMarkAsRead={vi.fn()}
        onClearAll={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('notification-bell'));
    const item = screen.getByTestId('notification-item-n1');
    expect(item.className).toContain('bg-white');
  });
});
