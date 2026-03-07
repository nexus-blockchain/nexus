'use client';

import { useState, useCallback } from 'react';
import { uploadToIpfs, uploadJsonToIpfs, uploadTextToIpfs } from '@/lib/ipfs';

type UploadStatus = 'idle' | 'uploading' | 'success' | 'error';

interface UseIpfsUploadReturn {
  upload: (file: File | Blob) => Promise<string | null>;
  uploadJson: (data: unknown) => Promise<string | null>;
  uploadText: (text: string) => Promise<string | null>;
  isUploading: boolean;
  error: string | null;
  cid: string | null;
}

export function useIpfsUpload(): UseIpfsUploadReturn {
  const [status, setStatus] = useState<UploadStatus>('idle');
  const [error, setError] = useState<string | null>(null);
  const [cid, setCid] = useState<string | null>(null);

  const reset = useCallback(() => {
    setStatus('uploading');
    setError(null);
    setCid(null);
  }, []);

  const upload = useCallback(async (file: File | Blob): Promise<string | null> => {
    reset();
    try {
      const result = await uploadToIpfs(file);
      setCid(result);
      setStatus('success');
      return result;
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Upload failed';
      setError(msg);
      setStatus('error');
      return null;
    }
  }, [reset]);

  const uploadJson = useCallback(async (data: unknown): Promise<string | null> => {
    reset();
    try {
      const result = await uploadJsonToIpfs(data);
      setCid(result);
      setStatus('success');
      return result;
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Upload failed';
      setError(msg);
      setStatus('error');
      return null;
    }
  }, [reset]);

  const uploadText = useCallback(async (text: string): Promise<string | null> => {
    reset();
    try {
      const result = await uploadTextToIpfs(text);
      setCid(result);
      setStatus('success');
      return result;
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Upload failed';
      setError(msg);
      setStatus('error');
      return null;
    }
  }, [reset]);

  return {
    upload,
    uploadJson,
    uploadText,
    isUploading: status === 'uploading',
    error,
    cid,
  };
}
