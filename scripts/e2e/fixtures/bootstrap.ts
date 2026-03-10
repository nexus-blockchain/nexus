/**
 * 开发链引导 — 在运行 E2E 之前设置必要的链上状态
 *
 * 包括:
 *   - 设置 NEX/USDT 初始价格 (实体创建依赖价格预言机)
 */

import { ApiPromise } from '@polkadot/api';
import { KeyringPair } from '@polkadot/keyring/types';
import { xxhashAsU8a } from '@polkadot/util-crypto';
import { u8aToHex } from '@polkadot/util';
import { sudoSend } from '../core/chain-state.js';

function getStorageValueKeyHex(pallet: string, item: string): string {
  const key1 = xxhashAsU8a(pallet, 128);
  const key2 = xxhashAsU8a(item, 128);
  return u8aToHex(new Uint8Array([...key1, ...key2]));
}

async function ensureReasonableTwapAccumulator(
  api: ApiPromise,
  sudoAccount: KeyringPair,
  priceU64: bigint = 100_000n,
): Promise<void> {
  const minReasonablePriceU64 = 100_000n;
  const existing = await (api.query as any).nexMarket.twapAccumulatorStore();
  const raw = existing?.toJSON?.() as Record<string, unknown> | null | undefined;
  if (!raw) {
    return;
  }

  const lastPrice = BigInt(String(raw.lastPrice ?? 0));
  if (lastPrice >= minReasonablePriceU64) {
    console.log(`  ✓ TWAP 累积器价格正常: ${existing.toHuman()?.lastPrice ?? lastPrice.toString()}`);
    return;
  }

  const storageKey = getStorageValueKeyHex('NexMarket', 'TwapAccumulatorStore');
  console.log(`  ℹ TWAP 累积器价格异常偏低，清理污染数据: ${lastPrice} -> fallback LastTradePrice=${priceU64}`);
  const killStorageTx = api.tx.system.killStorage([storageKey]);
  const result = await sudoSend(api, killStorageTx, sudoAccount, 'killStorage(TwapAccumulatorStore)');

  if (!result.success) {
    console.warn(`  ⚠ 清理 TWAP 累积器失败: ${result.error}`);
    return;
  }

  const verify = await (api.query as any).nexMarket.twapAccumulatorStore();
  const verifyRaw = verify?.toJSON?.();
  if (verifyRaw == null) {
    console.log('  ✓ TWAP 累积器已清理，将回退到 LastTradePrice');
  } else {
    console.warn(`  ⚠ TWAP 累积器仍存在: ${JSON.stringify(verifyRaw)}`);
  }
}

/**
 * 通过 sudo(system.setStorage) 直接写入 LastTradePrice。
 * nexMarket.setInitialPrice 需要 MarketAdminOrigin (council),
 * 在 dev 链上用 setStorage 更便捷。
 */
async function ensureInitialPrice(
  api: ApiPromise,
  sudoAccount: KeyringPair,
  priceU64: bigint = 100_000n, // 默认 1 NEX = 0.1 USDT（100 USDT ≈ 1000 NEX）
): Promise<void> {
  const minReasonablePriceU64 = 100_000n;
  // 检查是否已有价格
  const existing = await (api.query as any).nexMarket.lastTradePrice();
  const hasPrice = existing && (
    (existing.isSome !== undefined && existing.isSome) ||
    (existing.isSome === undefined && existing.toString() !== '0' && existing.toString() !== '')
  );
  if (hasPrice) {
    const raw = BigInt(existing.toString());
    if (raw >= minReasonablePriceU64) {
      console.log(`  ✓ 价格已存在: ${existing.toHuman()}`);
      return;
    }
    console.log(`  ℹ 价格异常偏低，重置 LastTradePrice: ${existing.toHuman()} -> ${priceU64}`);
  }

  // 计算 storage key: twox128("NexMarket") ++ twox128("LastTradePrice")
  const storageKey = getStorageValueKeyHex('NexMarket', 'LastTradePrice');

  // 编码 u64 LE
  const buf = new Uint8Array(8);
  const view = new DataView(buf.buffer);
  view.setBigUint64(0, priceU64, true);
  const storageValue = u8aToHex(buf);

  const setStorageTx = api.tx.system.setStorage([[storageKey, storageValue]]);
  const result = await sudoSend(api, setStorageTx, sudoAccount, 'setStorage(LastTradePrice)');

  if (!result.success) {
    console.warn(`  ⚠ 设置初始价格失败: ${result.error}`);
    return;
  }

  // 验证
  const verify = await (api.query as any).nexMarket.lastTradePrice();
  console.log(`  ✓ 初始价格已设置: ${verify.toHuman()} (${priceU64})`);
}

/**
 * 运行所有引导步骤
 */
export async function bootstrapDevChain(
  api: ApiPromise,
  sudoAccount: KeyringPair,
): Promise<void> {
  console.log('🔧 引导开发链状态...');
  await ensureReasonableTwapAccumulator(api, sudoAccount);
  await ensureInitialPrice(api, sudoAccount);
}
