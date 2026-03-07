import { describe, test, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { TxConfirmDialog } from './tx-confirm-dialog';
import { TxPreview } from './tx-preview';
import { TxStatusIndicator } from './tx-status-indicator';
import type { ConfirmDialogConfig, TxState } from '@/lib/types/models';

// ---------------------------------------------------------------------------
// TxConfirmDialog
// ---------------------------------------------------------------------------
describe('TxConfirmDialog', () => {
  const baseConfig: ConfirmDialogConfig = {
    title: 'Transfer Ownership',
    description: 'This action is irreversible.',
    severity: 'danger',
  };

  test('renders nothing when open is false', () => {
    const { container } = render(
      <TxConfirmDialog open={false} onClose={vi.fn()} onConfirm={vi.fn()} config={baseConfig} />,
    );
    expect(container.innerHTML).toBe('');
  });

  test('renders dialog with title and description when open', () => {
    render(
      <TxConfirmDialog open={true} onClose={vi.fn()} onConfirm={vi.fn()} config={baseConfig} />,
    );
    expect(screen.getByText('Transfer Ownership')).toBeInTheDocument();
    expect(screen.getByText('This action is irreversible.')).toBeInTheDocument();
  });

  test('calls onConfirm when confirm button is clicked (no requireInput)', () => {
    const onConfirm = vi.fn();
    render(
      <TxConfirmDialog open={true} onClose={vi.fn()} onConfirm={onConfirm} config={baseConfig} />,
    );
    fireEvent.click(screen.getByText('Confirm'));
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  test('calls onClose when cancel button is clicked', () => {
    const onClose = vi.fn();
    render(
      <TxConfirmDialog open={true} onClose={onClose} onConfirm={vi.fn()} config={baseConfig} />,
    );
    fireEvent.click(screen.getByText('Cancel'));
    expect(onClose).toHaveBeenCalledOnce();
  });

  test('confirm button is disabled until requireInput text matches', () => {
    const config: ConfirmDialogConfig = { ...baseConfig, requireInput: 'DELETE' };
    const onConfirm = vi.fn();
    render(
      <TxConfirmDialog open={true} onClose={vi.fn()} onConfirm={onConfirm} config={config} />,
    );

    const confirmBtn = screen.getByText('Confirm');
    expect(confirmBtn).toBeDisabled();

    // Type partial text — still disabled
    const input = screen.getByPlaceholderText('DELETE');
    fireEvent.change(input, { target: { value: 'DEL' } });
    expect(confirmBtn).toBeDisabled();

    // Type exact match — enabled
    fireEvent.change(input, { target: { value: 'DELETE' } });
    expect(confirmBtn).not.toBeDisabled();

    fireEvent.click(confirmBtn);
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  test('applies severity-based styling (info)', () => {
    const config: ConfirmDialogConfig = {
      title: 'Info',
      description: 'Informational.',
      severity: 'info',
    };
    render(
      <TxConfirmDialog open={true} onClose={vi.fn()} onConfirm={vi.fn()} config={config} />,
    );
    const title = screen.getByText('Info');
    expect(title.className).toContain('text-blue-800');
  });

  test('applies severity-based styling (warning)', () => {
    const config: ConfirmDialogConfig = {
      title: 'Warning',
      description: 'Be careful.',
      severity: 'warning',
    };
    render(
      <TxConfirmDialog open={true} onClose={vi.fn()} onConfirm={vi.fn()} config={config} />,
    );
    const title = screen.getByText('Warning');
    expect(title.className).toContain('text-yellow-800');
  });

  test('closes when clicking overlay', () => {
    const onClose = vi.fn();
    render(
      <TxConfirmDialog open={true} onClose={onClose} onConfirm={vi.fn()} config={baseConfig} />,
    );
    fireEvent.click(screen.getByTestId('tx-confirm-overlay'));
    expect(onClose).toHaveBeenCalledOnce();
  });
});

// ---------------------------------------------------------------------------
// TxPreview
// ---------------------------------------------------------------------------
describe('TxPreview', () => {
  test('renders pallet, call, params, and estimated fee', () => {
    render(
      <TxPreview
        pallet="entityRegistry"
        call="transferOwnership"
        params={{ entity_id: 42, new_owner: '5GrwvaEF...' }}
        estimatedFee="0.0012 NEX"
      />,
    );
    expect(screen.getByTestId('tx-preview-pallet')).toHaveTextContent('entityRegistry');
    expect(screen.getByTestId('tx-preview-call')).toHaveTextContent('transferOwnership');
    expect(screen.getByTestId('tx-preview-fee')).toHaveTextContent('0.0012 NEX');
    expect(screen.getByText('entity_id')).toBeInTheDocument();
    expect(screen.getByText('42')).toBeInTheDocument();
  });

  test('shows dash when estimatedFee is not provided', () => {
    render(
      <TxPreview pallet="entityShop" call="createShop" params={{}} />,
    );
    expect(screen.getByTestId('tx-preview-fee')).toHaveTextContent('—');
  });

  test('renders empty params section gracefully', () => {
    render(
      <TxPreview pallet="entityShop" call="createShop" params={{}} estimatedFee="0.001 NEX" />,
    );
    expect(screen.queryByTestId('tx-preview-params')).not.toBeInTheDocument();
  });

  test('formats bigint param values', () => {
    render(
      <TxPreview
        pallet="entityToken"
        call="mintTokens"
        params={{ amount: BigInt('1000000000000') }}
        estimatedFee="0.002 NEX"
      />,
    );
    expect(screen.getByText('1000000000000')).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// TxStatusIndicator
// ---------------------------------------------------------------------------
describe('TxStatusIndicator', () => {
  const idle: TxState = { status: 'idle', hash: null, error: null, blockNumber: null };

  test('renders nothing when status is idle', () => {
    const { container } = render(<TxStatusIndicator txState={idle} />);
    expect(container.innerHTML).toBe('');
  });

  test('shows signing message', () => {
    render(<TxStatusIndicator txState={{ ...idle, status: 'signing' }} />);
    expect(screen.getByTestId('tx-status-label')).toHaveTextContent('Waiting for signature...');
  });

  test('shows broadcasting message', () => {
    render(<TxStatusIndicator txState={{ ...idle, status: 'broadcasting' }} />);
    expect(screen.getByTestId('tx-status-label')).toHaveTextContent('Broadcasting...');
  });

  test('shows inBlock with block number', () => {
    render(
      <TxStatusIndicator txState={{ status: 'inBlock', hash: '0xabc', error: null, blockNumber: 12345 }} />,
    );
    expect(screen.getByTestId('tx-status-label')).toHaveTextContent('Included in block #12345');
  });

  test('shows finalized with truncated hash', () => {
    const hash = '0xabcdef1234567890';
    render(
      <TxStatusIndicator txState={{ status: 'finalized', hash, error: null, blockNumber: 12345 }} />,
    );
    const label = screen.getByTestId('tx-status-label').textContent!;
    expect(label).toContain('Finalized ✓');
    expect(label).toContain('0xabcdef12');
  });

  test('shows error message', () => {
    render(
      <TxStatusIndicator txState={{ status: 'error', hash: null, error: 'Insufficient balance', blockNumber: null }} />,
    );
    expect(screen.getByTestId('tx-status-label')).toHaveTextContent('Insufficient balance');
  });

  test('applies correct color classes per status', () => {
    const { rerender } = render(
      <TxStatusIndicator txState={{ ...idle, status: 'signing' }} />,
    );
    expect(screen.getByTestId('tx-status').className).toContain('bg-yellow-50');

    rerender(<TxStatusIndicator txState={{ ...idle, status: 'broadcasting' }} />);
    expect(screen.getByTestId('tx-status').className).toContain('bg-blue-50');

    rerender(
      <TxStatusIndicator txState={{ status: 'error', hash: null, error: 'fail', blockNumber: null }} />,
    );
    expect(screen.getByTestId('tx-status').className).toContain('bg-red-50');

    rerender(
      <TxStatusIndicator txState={{ status: 'finalized', hash: '0x1', error: null, blockNumber: 1 }} />,
    );
    expect(screen.getByTestId('tx-status').className).toContain('bg-green-50');
  });
});
