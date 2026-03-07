'use client';

import React, { useState } from 'react';
import type { Notification } from '@/lib/types/models';

export interface NotificationCenterProps {
  notifications: Notification[];
  unreadCount: number;
  onMarkAsRead: (id: string) => void;
  onClearAll: () => void;
}

/**
 * Notification center component.
 * Displays a bell icon with unread badge, and a dropdown panel listing
 * event notifications with mark-as-read and clear-all actions.
 */
export function NotificationCenter({
  notifications,
  unreadCount,
  onMarkAsRead,
  onClearAll,
}: NotificationCenterProps) {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <div className="relative" data-testid="notification-center">
      {/* Bell button with unread badge */}
      <button
        type="button"
        className="relative rounded-md p-2 text-gray-600 hover:bg-gray-100 hover:text-gray-900 focus:outline-none focus:ring-2 focus:ring-blue-500"
        onClick={() => setIsOpen((prev) => !prev)}
        aria-label={`Notifications${unreadCount > 0 ? ` (${unreadCount} unread)` : ''}`}
        data-testid="notification-bell"
      >
        <BellIcon />
        {unreadCount > 0 && (
          <span
            className="absolute -right-1 -top-1 flex h-5 min-w-5 items-center justify-center rounded-full bg-red-500 px-1 text-xs font-medium text-white"
            data-testid="notification-badge"
          >
            {unreadCount > 99 ? '99+' : unreadCount}
          </span>
        )}
      </button>

      {/* Dropdown panel */}
      {isOpen && (
        <div
          className="absolute right-0 top-full z-50 mt-2 w-80 rounded-lg border border-gray-200 bg-white shadow-lg"
          data-testid="notification-panel"
        >
          {/* Header */}
          <div className="flex items-center justify-between border-b border-gray-100 px-4 py-3">
            <h3 className="text-sm font-semibold text-gray-900">Notifications</h3>
            {notifications.length > 0 && (
              <button
                type="button"
                className="text-xs text-blue-600 hover:text-blue-800"
                onClick={onClearAll}
                data-testid="notification-clear-all"
              >
                Clear all
              </button>
            )}
          </div>

          {/* Notification list */}
          <div className="max-h-96 overflow-y-auto">
            {notifications.length === 0 ? (
              <div className="px-4 py-8 text-center text-sm text-gray-400" data-testid="notification-empty">
                No notifications
              </div>
            ) : (
              <ul>
                {notifications.map((notification) => (
                  <NotificationItem
                    key={notification.id}
                    notification={notification}
                    onMarkAsRead={onMarkAsRead}
                  />
                ))}
              </ul>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

// ─── Notification Item ──────────────────────────────────────

interface NotificationItemProps {
  notification: Notification;
  onMarkAsRead: (id: string) => void;
}

function NotificationItem({ notification, onMarkAsRead }: NotificationItemProps) {
  const timeAgo = formatTimeAgo(notification.timestamp);

  return (
    <li
      className={`border-b border-gray-50 px-4 py-3 last:border-b-0 ${
        notification.read ? 'bg-white' : 'bg-blue-50'
      }`}
      data-testid={`notification-item-${notification.id}`}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          <p className="text-sm text-gray-900">{notification.summary}</p>
          <div className="mt-1 flex items-center gap-2">
            <span className="rounded bg-gray-100 px-1.5 py-0.5 text-xs text-gray-500">
              {notification.pallet}
            </span>
            <span className="text-xs text-gray-400">{timeAgo}</span>
          </div>
        </div>
        {!notification.read && (
          <button
            type="button"
            className="mt-0.5 shrink-0 rounded p-1 text-blue-500 hover:bg-blue-100"
            onClick={() => onMarkAsRead(notification.id)}
            aria-label="Mark as read"
            data-testid={`notification-mark-read-${notification.id}`}
          >
            <span className="block h-2 w-2 rounded-full bg-blue-500" />
          </button>
        )}
      </div>
    </li>
  );
}

// ─── Helpers ────────────────────────────────────────────────

function formatTimeAgo(timestamp: number): string {
  const seconds = Math.floor((Date.now() - timestamp) / 1000);
  if (seconds < 60) return 'just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

function BellIcon() {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9" />
      <path d="M10.3 21a1.94 1.94 0 0 0 3.4 0" />
    </svg>
  );
}
