export type AssetType = 'NEX' | 'USDT';

const PRECISION: Record<AssetType, number> = {
  NEX: 12,   // 12 decimal places
  USDT: 6,   // 6 decimal places (10^6)
};

export interface AmountValidationResult {
  valid: boolean;
  error?: string;
  value?: bigint; // parsed value in smallest unit
}

/** Validate amount input string for given asset type */
export function validateAmount(input: string, assetType: AssetType): AmountValidationResult {
  if (!input || input.trim() === '') {
    return { valid: false, error: 'Amount is required' };
  }

  const trimmed = input.trim();

  // Reject non-numeric (allow decimal point)
  if (!/^\d+(\.\d+)?$/.test(trimmed)) {
    return { valid: false, error: 'Invalid number format' };
  }

  const num = Number(trimmed);
  if (num < 0) {
    return { valid: false, error: 'Amount must be non-negative' };
  }
  if (num === 0) {
    return { valid: false, error: 'Amount must be greater than zero' };
  }

  const maxDecimals = PRECISION[assetType];
  const parts = trimmed.split('.');
  if (parts.length === 2 && parts[1].length > maxDecimals) {
    return { valid: false, error: `Maximum ${maxDecimals} decimal places for ${assetType}` };
  }

  // Convert to smallest unit
  const factor = BigInt(10 ** maxDecimals);
  const [intPart, decPart = ''] = parts;
  const paddedDec = decPart.padEnd(maxDecimals, '0');
  const value = BigInt(intPart) * factor + BigInt(paddedDec);

  return { valid: true, value };
}
