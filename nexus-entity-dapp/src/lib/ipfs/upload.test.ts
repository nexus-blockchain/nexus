import { describe, test, expect, vi, beforeEach } from 'vitest';
import { uploadToIpfs, uploadJsonToIpfs, uploadTextToIpfs } from './upload';

const FAKE_CID = 'QmTestCid123456789abcdef';

// Mock global fetch
const mockFetch = vi.fn();
vi.stubGlobal('fetch', mockFetch);

beforeEach(() => {
  vi.clearAllMocks();
  vi.stubEnv('NEXT_PUBLIC_PINATA_JWT', 'test-jwt-token');
});

describe('uploadToIpfs', () => {
  test('uploads a file and returns CID on success', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => ({ IpfsHash: FAKE_CID }),
    });

    const file = new File(['hello'], 'test.txt', { type: 'text/plain' });
    const cid = await uploadToIpfs(file);

    expect(cid).toBe(FAKE_CID);
    expect(mockFetch).toHaveBeenCalledOnce();

    const [url, options] = mockFetch.mock.calls[0];
    expect(url).toBe('https://api.pinata.cloud/pinning/pinFileToIPFS');
    expect(options.method).toBe('POST');
    expect(options.headers.Authorization).toBe('Bearer test-jwt-token');
    expect(options.body).toBeInstanceOf(FormData);
  });

  test('uploads a Blob and returns CID', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => ({ IpfsHash: FAKE_CID }),
    });

    const blob = new Blob(['data'], { type: 'application/octet-stream' });
    const cid = await uploadToIpfs(blob);
    expect(cid).toBe(FAKE_CID);
  });

  test('uses provided JWT over env var', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => ({ IpfsHash: FAKE_CID }),
    });

    const file = new File(['x'], 'x.txt');
    await uploadToIpfs(file, 'custom-jwt');

    const [, options] = mockFetch.mock.calls[0];
    expect(options.headers.Authorization).toBe('Bearer custom-jwt');
  });

  test('throws when Pinata JWT is not configured', async () => {
    vi.stubEnv('NEXT_PUBLIC_PINATA_JWT', '');
    const file = new File(['x'], 'x.txt');
    await expect(uploadToIpfs(file)).rejects.toThrow('Pinata JWT is not configured');
  });

  test('throws on non-ok response', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 401,
      text: async () => 'Unauthorized',
    });

    const file = new File(['x'], 'x.txt');
    await expect(uploadToIpfs(file)).rejects.toThrow('IPFS upload failed (401): Unauthorized');
  });

  test('throws on network error', async () => {
    mockFetch.mockRejectedValueOnce(new Error('Network error'));

    const file = new File(['x'], 'x.txt');
    await expect(uploadToIpfs(file)).rejects.toThrow('Network error');
  });
});

describe('uploadJsonToIpfs', () => {
  test('uploads JSON data and returns CID', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => ({ IpfsHash: FAKE_CID }),
    });

    const data = { name: 'Test', value: 42 };
    const cid = await uploadJsonToIpfs(data);

    expect(cid).toBe(FAKE_CID);
    expect(mockFetch).toHaveBeenCalledOnce();
  });

  test('throws on failure', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 500,
      text: async () => 'Server error',
    });

    await expect(uploadJsonToIpfs({ a: 1 })).rejects.toThrow('IPFS upload failed (500)');
  });
});

describe('uploadTextToIpfs', () => {
  test('uploads text and returns CID', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => ({ IpfsHash: FAKE_CID }),
    });

    const cid = await uploadTextToIpfs('Hello IPFS');

    expect(cid).toBe(FAKE_CID);
    expect(mockFetch).toHaveBeenCalledOnce();
  });

  test('throws on failure', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 403,
      text: async () => 'Forbidden',
    });

    await expect(uploadTextToIpfs('test')).rejects.toThrow('IPFS upload failed (403)');
  });
});
