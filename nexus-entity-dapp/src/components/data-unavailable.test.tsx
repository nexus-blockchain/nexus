import { describe, test, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { DataUnavailable } from './data-unavailable';

describe('DataUnavailable', () => {
  test('renders default message when no message prop provided', () => {
    render(<DataUnavailable />);
    expect(screen.getByText('Data temporarily unavailable')).toBeInTheDocument();
  });

  test('renders custom message when provided', () => {
    render(<DataUnavailable message="NEX balance data temporarily unavailable" />);
    expect(screen.getByText('NEX balance data temporarily unavailable')).toBeInTheDocument();
  });

  test('does not render retry button when onRetry is not provided', () => {
    render(<DataUnavailable />);
    expect(screen.queryByText('Retry')).not.toBeInTheDocument();
  });

  test('renders retry button when onRetry is provided', () => {
    const onRetry = vi.fn();
    render(<DataUnavailable onRetry={onRetry} />);
    expect(screen.getByText('Retry')).toBeInTheDocument();
  });

  test('calls onRetry when retry button is clicked', () => {
    const onRetry = vi.fn();
    render(<DataUnavailable onRetry={onRetry} />);
    fireEvent.click(screen.getByText('Retry'));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  test('has role="status" for accessibility', () => {
    render(<DataUnavailable />);
    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  test('applies custom className', () => {
    render(<DataUnavailable className="mt-4" />);
    const el = screen.getByRole('status');
    expect(el.className).toContain('mt-4');
  });

  test('does not crash the page — renders as a contained element', () => {
    const { container } = render(
      <div>
        <p>Other content</p>
        <DataUnavailable message="Escrow data unavailable" />
        <p>More content</p>
      </div>,
    );
    expect(screen.getByText('Other content')).toBeInTheDocument();
    expect(screen.getByText('More content')).toBeInTheDocument();
    expect(screen.getByText('Escrow data unavailable')).toBeInTheDocument();
  });
});
