'use client';

import React, { useState, useCallback } from 'react';
import type { ConfirmDialogConfig } from '@/lib/types/models';

export interface TxConfirmDialogProps {
  open: boolean;
  onClose: () => void;
  onConfirm: () => void;
  config: ConfirmDialogConfig;
}

const severityStyles: Record<ConfirmDialogConfig['severity'], { border: string; bg: string; text: string; button: string }> = {
  info: {
    border: 'border-blue-300',
    bg: 'bg-blue-50',
    text: 'text-blue-800',
    button: 'bg-blue-600 hover:bg-blue-700 text-white',
  },
  warning: {
    border: 'border-yellow-300',
    bg: 'bg-yellow-50',
    text: 'text-yellow-800',
    button: 'bg-yellow-600 hover:bg-yellow-700 text-white',
  },
  danger: {
    border: 'border-red-300',
    bg: 'bg-red-50',
    text: 'text-red-800',
    button: 'bg-red-600 hover:bg-red-700 text-white',
  },
};

/**
 * Confirmation dialog for dangerous / irreversible operations.
 * When `config.requireInput` is set the user must type the exact text
 * before the confirm button becomes enabled.
 */
export function TxConfirmDialog({ open, onClose, onConfirm, config }: TxConfirmDialogProps) {
  const [inputValue, setInputValue] = useState('');

  const styles = severityStyles[config.severity];
  const needsInput = typeof config.requireInput === 'string' && config.requireInput.length > 0;
  const confirmEnabled = needsInput ? inputValue === config.requireInput : true;

  const handleConfirm = useCallback(() => {
    if (confirmEnabled) {
      setInputValue('');
      onConfirm();
    }
  }, [confirmEnabled, onConfirm]);

  const handleClose = useCallback(() => {
    setInputValue('');
    onClose();
  }, [onClose]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      data-testid="tx-confirm-overlay"
      onClick={handleClose}
      onKeyDown={undefined}
      role="presentation"
    >
      <div
        className={`mx-4 w-full max-w-md rounded-lg border ${styles.border} bg-white p-6 shadow-lg`}
        role="dialog"
        aria-modal="true"
        aria-labelledby="tx-confirm-title"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={undefined}
      >
        <h2 id="tx-confirm-title" className={`mb-2 text-lg font-semibold ${styles.text}`}>
          {config.title}
        </h2>

        <div className={`mb-4 rounded-md ${styles.bg} p-3 text-sm ${styles.text}`}>
          {config.description}
        </div>

        {needsInput && (
          <div className="mb-4">
            <label htmlFor="confirm-input" className="mb-1 block text-sm text-gray-600">
              Type <span className="font-mono font-semibold">{config.requireInput}</span> to confirm
            </label>
            <input
              id="confirm-input"
              type="text"
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              placeholder={config.requireInput}
              autoComplete="off"
            />
          </div>
        )}

        <div className="flex justify-end gap-3">
          <button
            type="button"
            className="rounded-md border border-gray-300 px-4 py-2 text-sm text-gray-700 hover:bg-gray-50"
            onClick={handleClose}
          >
            Cancel
          </button>
          <button
            type="button"
            className={`rounded-md px-4 py-2 text-sm ${styles.button} disabled:cursor-not-allowed disabled:opacity-50`}
            disabled={!confirmEnabled}
            onClick={handleConfirm}
          >
            Confirm
          </button>
        </div>
      </div>
    </div>
  );
}
