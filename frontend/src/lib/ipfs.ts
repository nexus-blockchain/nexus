const IPFS_GATEWAY = process.env.NEXT_PUBLIC_IPFS_GATEWAY || "https://gateway.pinata.cloud/ipfs";
const PINATA_API_KEY = process.env.NEXT_PUBLIC_PINATA_API_KEY || "";
const PINATA_SECRET = process.env.NEXT_PUBLIC_PINATA_SECRET || "";
const PINATA_ENDPOINT = "https://api.pinata.cloud";

export function ipfsUrl(cid: string): string {
  return `${IPFS_GATEWAY}/${cid}`;
}

export type CidContentType = "image" | "json" | "text" | "unknown";

export async function detectCidType(cid: string): Promise<CidContentType> {
  try {
    const res = await fetch(ipfsUrl(cid), { method: "HEAD" });
    const ct = res.headers.get("content-type") || "";
    if (ct.startsWith("image/")) return "image";
    if (ct.includes("json")) return "json";
    if (ct.startsWith("text/")) return "text";
    return "unknown";
  } catch {
    return "unknown";
  }
}

export async function fetchCidContent(cid: string): Promise<string> {
  const res = await fetch(ipfsUrl(cid));
  return res.text();
}

export interface UploadResult {
  cid: string;
  size: number;
}

export async function uploadToIpfs(file: File): Promise<UploadResult> {
  if (!PINATA_API_KEY || !PINATA_SECRET) {
    throw new Error("Pinata API credentials not configured. Set NEXT_PUBLIC_PINATA_API_KEY and NEXT_PUBLIC_PINATA_SECRET.");
  }

  const formData = new FormData();
  formData.append("file", file);

  const res = await fetch(`${PINATA_ENDPOINT}/pinning/pinFileToIPFS`, {
    method: "POST",
    headers: {
      pinata_api_key: PINATA_API_KEY,
      pinata_secret_api_key: PINATA_SECRET,
    },
    body: formData,
  });

  if (!res.ok) {
    const errText = await res.text();
    throw new Error(`IPFS upload failed: ${errText}`);
  }

  const data = await res.json();
  return { cid: data.IpfsHash, size: data.PinSize };
}

export async function uploadJsonToIpfs(json: Record<string, unknown>, name?: string): Promise<UploadResult> {
  if (!PINATA_API_KEY || !PINATA_SECRET) {
    throw new Error("Pinata API credentials not configured.");
  }

  const res = await fetch(`${PINATA_ENDPOINT}/pinning/pinJSONToIPFS`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      pinata_api_key: PINATA_API_KEY,
      pinata_secret_api_key: PINATA_SECRET,
    },
    body: JSON.stringify({
      pinataContent: json,
      pinataMetadata: { name: name || "nexus-entity-data" },
    }),
  });

  if (!res.ok) {
    const errText = await res.text();
    throw new Error(`IPFS JSON upload failed: ${errText}`);
  }

  const data = await res.json();
  return { cid: data.IpfsHash, size: data.PinSize };
}
