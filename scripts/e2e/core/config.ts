/**
 * E2E 测试环境配置
 */

export interface E2EConfig {
  wsUrl: string;
  /** 交易等待超时 (ms) */
  txTimeout: number;
  /** 是否输出详细日志 */
  verbose: boolean;
  /** 1 NEX = 10^12 最小单位 */
  NEX_DECIMALS: bigint;
  /** 1 USDT = 10^6 最小单位 */
  USDT_DECIMALS: bigint;
}

export const defaultConfig: E2EConfig = {
  wsUrl: process.env.WS_URL ?? 'ws://127.0.0.1:9944',
  txTimeout: 60_000,
  verbose: process.env.VERBOSE === 'true',
  NEX_DECIMALS: 1_000_000_000_000n,
  USDT_DECIMALS: 1_000_000n,
};

export function nex(amount: number): bigint {
  return BigInt(Math.floor(amount * 1e12));
}

export function usdt(amount: number): bigint {
  return BigInt(Math.floor(amount * 1e6));
}

export function formatNex(raw: bigint | string | number): string {
  const v = Number(BigInt(raw)) / 1e12;
  return `${v.toLocaleString()} NEX`;
}

export function formatUsdt(raw: bigint | string | number): string {
  const v = Number(BigInt(raw)) / 1e6;
  return `$${v.toFixed(2)} USDT`;
}
