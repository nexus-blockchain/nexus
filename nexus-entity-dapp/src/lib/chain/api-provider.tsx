'use client';

import React, { createContext, useContext, useEffect, useState, useRef, useCallback } from 'react';
import { ApiPromise, WsProvider } from '@polkadot/api';

type ConnectionStatus = 'connecting' | 'connected' | 'disconnected' | 'error';

interface ApiContextValue {
  api: ApiPromise | null;
  isReady: boolean;
  connectionStatus: ConnectionStatus;
  error: string | null;
}

const ApiContext = createContext<ApiContextValue>({
  api: null,
  isReady: false,
  connectionStatus: 'disconnected',
  error: null,
});

const DEFAULT_WS_ENDPOINT = process.env.NEXT_PUBLIC_WS_ENDPOINT || 'ws://127.0.0.1:9944';

const RECONNECT_CONFIG = {
  maxRetries: Infinity,
  baseDelay: 1000,
  maxDelay: 30000,
};

export function ApiProvider({
  endpoint,
  children,
}: {
  endpoint?: string;
  children: React.ReactNode;
}) {
  const [api, setApi] = useState<ApiPromise | null>(null);
  const [isReady, setIsReady] = useState(false);
  const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>('disconnected');
  const [error, setError] = useState<string | null>(null);
  const retryCount = useRef(0);
  const apiRef = useRef<ApiPromise | null>(null);

  const connect = useCallback(async () => {
    const wsEndpoint = endpoint || DEFAULT_WS_ENDPOINT;
    setConnectionStatus('connecting');
    setError(null);

    try {
      const provider = new WsProvider(wsEndpoint);

      provider.on('disconnected', () => {
        setConnectionStatus('disconnected');
        setIsReady(false);
        // Auto-reconnect with exponential backoff
        const delay = Math.min(
          RECONNECT_CONFIG.baseDelay * Math.pow(2, retryCount.current),
          RECONNECT_CONFIG.maxDelay,
        );
        retryCount.current += 1;
        setTimeout(() => connect(), delay);
      });

      provider.on('error', () => {
        setConnectionStatus('error');
        setError('WebSocket connection error');
      });

      const apiInstance = await ApiPromise.create({ provider });
      await apiInstance.isReady;

      apiRef.current = apiInstance;
      setApi(apiInstance);
      setIsReady(true);
      setConnectionStatus('connected');
      retryCount.current = 0; // Reset on successful connection
    } catch (err) {
      setConnectionStatus('error');
      setError(err instanceof Error ? err.message : 'Failed to connect');
      // Retry with exponential backoff
      const delay = Math.min(
        RECONNECT_CONFIG.baseDelay * Math.pow(2, retryCount.current),
        RECONNECT_CONFIG.maxDelay,
      );
      retryCount.current += 1;
      setTimeout(() => connect(), delay);
    }
  }, [endpoint]);

  useEffect(() => {
    connect();
    return () => {
      apiRef.current?.disconnect();
    };
  }, [connect]);

  return (
    <ApiContext.Provider value={{ api, isReady, connectionStatus, error }}>
      {children}
    </ApiContext.Provider>
  );
}

/** Hook to access the Polkadot.js API instance */
export function useApi(): ApiContextValue {
  return useContext(ApiContext);
}
