import type { ApiPromise } from '@polkadot/api';
import { assert } from '../framework/assert.js';
import { codecToHuman, codecToJson, coerceNumber, describeValue, readObjectField } from '../framework/codec.js';

export const VALID_TRON_ADDRESSES = {
  seller: 'TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t',
  buyer: 'TQn9Y2khEsLJW1ChVWFMSMeRDow5KcbLSE',
};

export interface MarketRecord {
  json: Record<string, unknown>;
  human: Record<string, unknown>;
}

export async function readNextMarketOrderId(api: ApiPromise): Promise<number> {
  return Number((await (api.query as any).nexMarket.nextOrderId()).toString());
}

export async function readNextUsdtTradeId(api: ApiPromise): Promise<number> {
  return Number((await (api.query as any).nexMarket.nextUsdtTradeId()).toString());
}

export async function readUserOrders(api: ApiPromise, address: string): Promise<number[]> {
  const value = await (api.query as any).nexMarket.userOrders(address);
  const json = codecToJson<unknown[]>(value);
  return Array.isArray(json) ? json.map((item) => Number(item)) : [];
}

export async function readUserTrades(api: ApiPromise, address: string): Promise<number[]> {
  const value = await (api.query as any).nexMarket.userTrades(address);
  const json = codecToJson<unknown[]>(value);
  return Array.isArray(json) ? json.map((item) => Number(item)) : [];
}

export async function readOrderTrades(api: ApiPromise, orderId: number): Promise<number[]> {
  const value = await (api.query as any).nexMarket.orderTrades(orderId);
  const json = codecToJson<unknown[]>(value);
  return Array.isArray(json) ? json.map((item) => Number(item)) : [];
}

export async function readMarketOrder(api: ApiPromise, orderId: number): Promise<MarketRecord> {
  const value = await (api.query as any).nexMarket.orders(orderId);
  assert((value as any).isSome, `market order ${orderId} should exist`);
  const order = (value as any).unwrap();
  return {
    json: codecToJson<Record<string, unknown>>(order),
    human: codecToHuman<Record<string, unknown>>(order),
  };
}

export async function tryReadMarketOrder(api: ApiPromise, orderId: number): Promise<MarketRecord | undefined> {
  const value = await (api.query as any).nexMarket.orders(orderId);
  if (!(value as any).isSome) {
    return undefined;
  }
  const order = (value as any).unwrap();
  return {
    json: codecToJson<Record<string, unknown>>(order),
    human: codecToHuman<Record<string, unknown>>(order),
  };
}

export async function readMarketTrade(api: ApiPromise, tradeId: number): Promise<MarketRecord> {
  const value = await (api.query as any).nexMarket.usdtTrades(tradeId);
  assert((value as any).isSome, `market trade ${tradeId} should exist`);
  const trade = (value as any).unwrap();
  return {
    json: codecToJson<Record<string, unknown>>(trade),
    human: codecToHuman<Record<string, unknown>>(trade),
  };
}

export function describeMarketField(record: MarketRecord, field: string): string {
  const value = readObjectField(record.human, field) ?? readObjectField(record.json, field);
  return describeValue(value);
}

export function marketFieldContains(record: MarketRecord, field: string, keyword: string): boolean {
  return describeMarketField(record, field).toLowerCase().includes(keyword.toLowerCase());
}

function makerMatches(record: MarketRecord, address: string): boolean {
  const maker = readObjectField(record.human, 'maker') ?? readObjectField(record.json, 'maker');
  return String(maker) === address;
}

function sideMatches(record: MarketRecord, sideKeyword: 'buy' | 'sell'): boolean {
  return describeMarketField(record, 'side').toLowerCase().includes(sideKeyword);
}

export async function findRecentMakerOrder(
  api: ApiPromise,
  maker: string,
  sideKeyword: 'buy' | 'sell',
  fromOrderId: number,
  toOrderIdExclusive: number,
): Promise<number | undefined> {
  for (let orderId = toOrderIdExclusive - 1; orderId >= fromOrderId; orderId -= 1) {
    const order = await tryReadMarketOrder(api, orderId);
    if (!order) {
      continue;
    }
    if (makerMatches(order, maker) && sideMatches(order, sideKeyword)) {
      return orderId;
    }
  }

  return undefined;
}

export async function readSafeMarketPrices(api: ApiPromise, marketPrice: number): Promise<{ sellPrice: number; buyPrice: number }> {
  const protectionQuery = (api.query as any).nexMarket?.priceProtection
    ?? (api.query as any).nexMarket?.priceProtectionStore;

  if (protectionQuery) {
    const protection = codecToJson(await protectionQuery());
    const maxDeviationBps = coerceNumber(readObjectField(protection, 'maxPriceDeviation', 'max_price_deviation'));
    if (maxDeviationBps && maxDeviationBps > 0) {
      const offset = Math.max(1, Math.floor((marketPrice * maxDeviationBps) / 10_000));
      return {
        sellPrice: marketPrice + offset,
        buyPrice: Math.max(1, marketPrice - offset),
      };
    }
  }

  const fallbackOffset = Math.max(1, Math.floor(marketPrice / 5));
  return {
    sellPrice: marketPrice + fallbackOffset,
    buyPrice: Math.max(1, marketPrice - fallbackOffset),
  };
}
