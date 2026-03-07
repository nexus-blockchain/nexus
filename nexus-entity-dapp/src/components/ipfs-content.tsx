'use client';

import React, { useState, useEffect, useCallback } from 'react';
import { ipfsUrl } from '@/lib/utils/ipfs';

interface IpfsContentProps {
  cid: string;
  className?: string;
}

export function IpfsContent({ cid, className }: IpfsContentProps) {
  const [content, setContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchContent = useCallback(async () => {
    setLoading(true);
    setError(null);
    setContent(null);

    try {
      const response = await fetch(ipfsUrl(cid));
      if (!response.ok) {
        throw new Error(`Failed to fetch (${response.status})`);
      }
      const text = await response.text();
      setContent(text);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load content');
    } finally {
      setLoading(false);
    }
  }, [cid]);

  useEffect(() => {
    fetchContent();
  }, [fetchContent]);

  if (loading) {
    return (
      <div className={className} role="status" aria-label="Loading content">
        <span style={{ color: '#9ca3af' }}>Loading content…</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className={className} role="alert">
        <span style={{ color: '#ef4444' }}>Failed to load content</span>
        <button
          onClick={fetchContent}
          style={{
            marginLeft: 8,
            color: '#3b82f6',
            background: 'none',
            border: 'none',
            cursor: 'pointer',
            textDecoration: 'underline',
          }}
          type="button"
        >
          Retry
        </button>
      </div>
    );
  }

  return (
    <div className={className}>
      <p style={{ whiteSpace: 'pre-wrap' }}>{content}</p>
    </div>
  );
}
