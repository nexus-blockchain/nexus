const PINATA_API_URL = 'https://api.pinata.cloud/pinning/pinFileToIPFS';

function getPinataJwt(jwt?: string): string {
  const token = jwt || process.env.NEXT_PUBLIC_PINATA_JWT || '';
  if (!token) {
    throw new Error('Pinata JWT is not configured');
  }
  return token;
}

/**
 * Upload a File or Blob to IPFS via Pinata, returns the CID.
 */
export async function uploadToIpfs(file: File | Blob, jwt?: string): Promise<string> {
  const token = getPinataJwt(jwt);
  const formData = new FormData();
  formData.append('file', file);

  const response = await fetch(PINATA_API_URL, {
    method: 'POST',
    headers: { Authorization: `Bearer ${token}` },
    body: formData,
  });

  if (!response.ok) {
    const text = await response.text().catch(() => 'Unknown error');
    throw new Error(`IPFS upload failed (${response.status}): ${text}`);
  }

  const data = await response.json();
  return data.IpfsHash as string;
}

/**
 * Upload a JSON object to IPFS via Pinata, returns the CID.
 */
export async function uploadJsonToIpfs(data: unknown, jwt?: string): Promise<string> {
  const blob = new Blob([JSON.stringify(data)], { type: 'application/json' });
  return uploadToIpfs(blob, jwt);
}

/**
 * Upload plain text to IPFS via Pinata, returns the CID.
 */
export async function uploadTextToIpfs(text: string, jwt?: string): Promise<string> {
  const blob = new Blob([text], { type: 'text/plain' });
  return uploadToIpfs(blob, jwt);
}
