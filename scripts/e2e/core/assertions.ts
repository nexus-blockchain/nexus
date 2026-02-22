/**
 * 链上状态断言库
 */

import { ApiPromise } from '@polkadot/api';
import { getFreeBalance, queryStorage, TxResult } from './chain-state.js';

export class AssertionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'AssertionError';
  }
}

function fail(msg: string): never {
  throw new AssertionError(msg);
}

/** 断言交易成功 */
export function assertTxSuccess(result: TxResult, context?: string): void {
  if (!result.success) {
    fail(`交易失败${context ? ` [${context}]` : ''}: ${result.error}`);
  }
}

/** 断言交易失败，且包含指定错误 */
export function assertTxFailed(result: TxResult, expectedError?: string, context?: string): void {
  if (result.success) {
    fail(`期望交易失败${context ? ` [${context}]` : ''}，但交易成功了`);
  }
  if (expectedError && result.error && !result.error.includes(expectedError)) {
    fail(`错误消息不匹配: 期望包含 "${expectedError}", 实际 "${result.error}"`);
  }
}

/** 断言事件被触发 */
export function assertEventEmitted(
  result: TxResult,
  section: string,
  method: string,
  context?: string,
): void {
  const found = result.events.some(
    (e) => e.section === section && e.method === method,
  );
  if (!found) {
    const actual = result.events.map((e) => `${e.section}.${e.method}`).join(', ');
    fail(
      `期望事件 ${section}.${method} 未触发${context ? ` [${context}]` : ''}. 实际事件: [${actual}]`,
    );
  }
}

/** 断言余额变化 (允许误差，因为手续费) */
export async function assertBalanceChange(
  api: ApiPromise,
  address: string,
  balanceBefore: bigint,
  expectedDelta: bigint,
  toleranceBps: number = 100, // 1% 默认容差 (手续费)
  context?: string,
): Promise<void> {
  const balanceAfter = await getFreeBalance(api, address);
  const actualDelta = balanceAfter - balanceBefore;
  const diff = actualDelta - expectedDelta;
  const absDiff = diff < 0n ? -diff : diff;
  const absExpected = expectedDelta < 0n ? -expectedDelta : expectedDelta;

  // 对于 0 期望值特殊处理
  if (absExpected === 0n) {
    // 只要变化不超过合理手续费即可
    const maxFee = 10_000_000_000n; // 0.01 NEX
    if (absDiff > maxFee) {
      fail(
        `余额变化超出预期${context ? ` [${context}]` : ''}: 期望 ~0, 实际 ${actualDelta}`,
      );
    }
    return;
  }

  const toleranceAmount = (absExpected * BigInt(toleranceBps)) / 10000n;
  if (absDiff > toleranceAmount) {
    fail(
      `余额变化超出容差${context ? ` [${context}]` : ''}: 期望 ${expectedDelta}, 实际 ${actualDelta}, 差异 ${diff}`,
    );
  }
}

/** 断言 storage 值存在 */
export async function assertStorageExists(
  api: ApiPromise,
  pallet: string,
  storage: string,
  keys: any[],
  context?: string,
): Promise<any> {
  const value = await queryStorage(api, pallet, storage, ...keys);
  if (value.isNone !== undefined && value.isNone) {
    fail(`Storage ${pallet}.${storage}(${keys.join(',')}) 不存在${context ? ` [${context}]` : ''}`);
  }
  return value;
}

/** 断言 storage 值不存在 */
export async function assertStorageEmpty(
  api: ApiPromise,
  pallet: string,
  storage: string,
  keys: any[],
  context?: string,
): Promise<void> {
  const value = await queryStorage(api, pallet, storage, ...keys);
  if (value.isNone !== undefined && !value.isNone) {
    fail(`Storage ${pallet}.${storage}(${keys.join(',')}) 应为空${context ? ` [${context}]` : ''}`);
  }
}

/** 断言 storage 中某字段值 */
export async function assertStorageField(
  api: ApiPromise,
  pallet: string,
  storage: string,
  keys: any[],
  fieldPath: string,
  expectedValue: any,
  context?: string,
): Promise<void> {
  const raw = await queryStorage(api, pallet, storage, ...keys);
  const value = raw.isSome !== undefined ? raw.unwrap() : raw;
  const human = value.toHuman ? value.toHuman() : value;

  const fields = fieldPath.split('.');
  let current: any = human;
  for (const field of fields) {
    if (current === undefined || current === null) {
      fail(
        `字段路径 "${fieldPath}" 在 ${pallet}.${storage} 中不存在${context ? ` [${context}]` : ''}`,
      );
    }
    current = current[field];
  }

  const actual = typeof current === 'object' ? JSON.stringify(current) : String(current);
  const expected = typeof expectedValue === 'object' ? JSON.stringify(expectedValue) : String(expectedValue);

  if (actual !== expected) {
    fail(
      `${pallet}.${storage}.${fieldPath}: 期望 ${expected}, 实际 ${actual}${context ? ` [${context}]` : ''}`,
    );
  }
}

/** 通用值相等断言 */
export function assertEqual<T>(actual: T, expected: T, context?: string): void {
  const a = typeof actual === 'object' ? JSON.stringify(actual) : String(actual);
  const e = typeof expected === 'object' ? JSON.stringify(expected) : String(expected);
  if (a !== e) {
    fail(`断言失败${context ? ` [${context}]` : ''}: 期望 ${e}, 实际 ${a}`);
  }
}

/** 通用 truthy 断言 */
export function assertTrue(value: any, context?: string): void {
  if (!value) {
    fail(`断言失败${context ? ` [${context}]` : ''}: 期望 truthy, 实际 ${value}`);
  }
}
