import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function shortenAddress(address: string, chars = 6): string {
  if (!address) return "";
  return `${address.slice(0, chars)}...${address.slice(-chars)}`;
}

export function formatBalance(balance: bigint, decimals = 12): string {
  const divisor = BigInt(10 ** decimals);
  const whole = balance / divisor;
  const fraction = balance % divisor;
  const fractionStr = fraction.toString().padStart(decimals, "0").slice(0, 4);
  return `${whole.toLocaleString()}.${fractionStr}`;
}

export function formatNumber(num: number | bigint): string {
  return Number(num).toLocaleString();
}

export function basisPointsToPercent(bp: number): string {
  return `${(bp / 100).toFixed(2)}%`;
}

export function blockToDate(blockNumber: number, currentBlock: number, blockTimeMs = 6000): Date {
  const diff = blockNumber - currentBlock;
  return new Date(Date.now() + diff * blockTimeMs);
}

export function ipfsUrl(cid: string): string {
  const gateway = process.env.NEXT_PUBLIC_IPFS_GATEWAY || "https://gateway.pinata.cloud/ipfs";
  return `${gateway}/${cid}`;
}
