'use client';

import React, { useRef, useState, useEffect } from 'react';
import { ipfsUrl } from '@/lib/utils/ipfs';

interface IpfsImageProps {
  cid: string;
  alt: string;
  className?: string;
  width?: number;
  height?: number;
}

export function IpfsImage({ cid, alt, className, width, height }: IpfsImageProps) {
  const imgRef = useRef<HTMLDivElement>(null);
  const [isVisible, setIsVisible] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState(false);

  useEffect(() => {
    const el = imgRef.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setIsVisible(true);
          observer.disconnect();
        }
      },
      { rootMargin: '200px' },
    );

    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const src = ipfsUrl(cid);

  return (
    <div
      ref={imgRef}
      className={className}
      style={{ width: width ?? undefined, height: height ?? undefined, position: 'relative' }}
    >
      {!isVisible || (!loaded && !error) ? (
        <div
          style={{
            width: '100%',
            height: '100%',
            minHeight: 48,
            backgroundColor: '#e5e7eb',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            borderRadius: 4,
          }}
          aria-label="Loading image"
        >
          <span style={{ color: '#9ca3af', fontSize: 12 }}>Loading…</span>
        </div>
      ) : null}
      {error ? (
        <div
          style={{
            width: '100%',
            height: '100%',
            minHeight: 48,
            backgroundColor: '#fee2e2',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            borderRadius: 4,
          }}
          role="img"
          aria-label={`Failed to load: ${alt}`}
        >
          <span style={{ color: '#ef4444', fontSize: 12 }}>Failed to load image</span>
        </div>
      ) : null}
      {isVisible && !error ? (
        <img
          src={src}
          alt={alt}
          width={width}
          height={height}
          onLoad={() => setLoaded(true)}
          onError={() => setError(true)}
          style={{
            display: loaded ? 'block' : 'none',
            width: '100%',
            height: '100%',
            objectFit: 'cover',
          }}
        />
      ) : null}
    </div>
  );
}
