const DEFAULT_IPFS_GATEWAY = process.env.NEXT_PUBLIC_IPFS_GATEWAY || 'https://gateway.pinata.cloud/ipfs';

/** Construct IPFS URL from CID */
export function ipfsUrl(cid: string, gateway?: string): string {
  const gw = gateway || DEFAULT_IPFS_GATEWAY;
  // Remove trailing slash from gateway
  const base = gw.endsWith('/') ? gw.slice(0, -1) : gw;
  return `${base}/${cid}`;
}
