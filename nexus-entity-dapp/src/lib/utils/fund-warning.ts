/** Check if fund balance is below warning threshold */
export function isFundWarning(balance: bigint, threshold: bigint): boolean {
  return balance < threshold;
}
