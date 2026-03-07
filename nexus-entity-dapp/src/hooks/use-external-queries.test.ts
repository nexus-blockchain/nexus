import { describe, test, expect } from 'vitest';
import {
  toExternalResult,
  type ExternalQueryResult,
} from './use-external-queries';
import type { UseQueryResult } from '@tanstack/react-query';

// ─── toExternalResult tests ─────────────────────────────────

function makeQueryResult<T>(overrides: Partial<UseQueryResult<T>>): UseQueryResult<T> {
  return {
    data: undefined as T | undefined,
    error: null,
    isError: false,
    isLoading: false,
    isLoadingError: false,
    isPending: false,
    isRefetchError: false,
    isSuccess: true,
    status: 'success',
    dataUpdatedAt: 0,
    errorUpdatedAt: 0,
    failureCount: 0,
    failureReason: null,
    errorUpdateCount: 0,
    isFetched: true,
    isFetchedAfterMount: true,
    isFetching: false,
    isInitialLoading: false,
    isPaused: false,
    isPlaceholderData: false,
    isRefetching: false,
    isStale: false,
    refetch: (() => Promise.resolve({} as any)) as any,
    fetchStatus: 'idle',
    promise: Promise.resolve({} as any),
    ...overrides,
  } as UseQueryResult<T>;
}

describe('toExternalResult', () => {
  test('returns data when query succeeds', () => {
    const query = makeQueryResult<string>({ data: 'hello', isError: false });
    const result = toExternalResult(query);

    expect(result.data).toBe('hello');
    expect(result.isUnavailable).toBe(false);
    expect(result.isLoading).toBe(false);
    expect(result.error).toBeNull();
  });

  test('returns isUnavailable=true when query errors', () => {
    const err = new Error('Network failure');
    const query = makeQueryResult<string>({
      data: undefined,
      isError: true,
      error: err,
    });
    const result = toExternalResult(query);

    expect(result.data).toBeNull();
    expect(result.isUnavailable).toBe(true);
    expect(result.error).toBe(err);
  });

  test('returns isLoading=true when query is loading', () => {
    const query = makeQueryResult<string>({
      data: undefined,
      isLoading: true,
      isError: false,
    });
    const result = toExternalResult(query);

    expect(result.data).toBeNull();
    expect(result.isLoading).toBe(true);
    expect(result.isUnavailable).toBe(false);
  });

  test('wraps non-Error error objects into Error instances', () => {
    const query = makeQueryResult<string>({
      data: undefined,
      isError: true,
      error: 'string error' as any,
    });
    const result = toExternalResult(query);

    expect(result.isUnavailable).toBe(true);
    expect(result.error).toBeInstanceOf(Error);
    expect(result.error?.message).toBe('string error');
  });

  test('returns null error when query has no error', () => {
    const query = makeQueryResult<number>({
      data: 42,
      isError: false,
      error: null,
    });
    const result = toExternalResult(query);

    expect(result.error).toBeNull();
  });
});
