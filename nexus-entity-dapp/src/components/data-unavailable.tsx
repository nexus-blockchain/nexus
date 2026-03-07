'use client';

import React from 'react';

export interface DataUnavailableProps {
  /** Custom message to display. Defaults to "Data temporarily unavailable" */
  message?: string;
  /** Optional retry callback */
  onRetry?: () => void;
  /** Additional CSS class names */
  className?: string;
}

/**
 * Reusable component for graceful degradation when external pallet queries fail.
 * Displays a non-intrusive "data unavailable" notice without affecting the rest of the page.
 *
 * Validates: Requirements 23.5
 */
export function DataUnavailable({ message, onRetry, className = '' }: DataUnavailableProps) {
  const displayMessage = message || 'Data temporarily unavailable';

  return (
    <div
      role="status"
      aria-live="polite"
      className={`rounded-md border border-dashed border-muted-foreground/30 bg-muted/30 px-4 py-3 text-sm text-muted-foreground ${className}`.trim()}
    >
      <div className="flex items-center justify-between gap-2">
        <span>{displayMessage}</span>
        {onRetry && (
          <button
            type="button"
            onClick={onRetry}
            className="text-xs underline hover:text-foreground transition-colors"
          >
            Retry
          </button>
        )}
      </div>
    </div>
  );
}
