export const NEX_PLANCK = 1_000_000_000_000n;

export function nex(amount: number): bigint {
  return BigInt(Math.round(amount * 1_000_000_000_000));
}

export function formatNex(raw: bigint): string {
  return `${(Number(raw) / 1e12).toLocaleString()} NEX`;
}
