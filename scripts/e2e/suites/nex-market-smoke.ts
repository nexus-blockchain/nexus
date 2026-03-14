import type { ApiPromise } from '@polkadot/api';
import { submitTx } from '../framework/api.js';
import { assert, assertTxSuccess } from '../framework/assert.js';
import { codecToHuman, codecToJson, coerceNumber, describeValue, readObjectField } from '../framework/codec.js';
import { TestSuite } from '../framework/types.js';
import { nex } from '../framework/units.js';

const TRON_ADDRESS = 'TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t';
const ORDER_AMOUNT = nex(10).toString();

async function readNextOrderId(api: ApiPromise): Promise<number> {
  const value = await (api.query as any).nexMarket.nextOrderId();
  return Number(value.toString());
}

async function readUserOrders(api: ApiPromise, address: string): Promise<number[]> {
  const value = await (api.query as any).nexMarket.userOrders(address);
  const json = codecToJson<unknown[]>(value);
  return Array.isArray(json) ? json.map((item) => Number(item)) : [];
}

async function readOrder(api: ApiPromise, orderId: number): Promise<{ json: Record<string, unknown>; human: Record<string, unknown> }> {
  const value = await (api.query as any).nexMarket.orders(orderId);
  assert((value as any).isSome, `order ${orderId} should exist`);
  const order = (value as any).unwrap();
  return {
    json: codecToJson<Record<string, unknown>>(order),
    human: codecToHuman<Record<string, unknown>>(order),
  };
}

async function tryReadOrder(api: ApiPromise, orderId: number): Promise<{ json: Record<string, unknown>; human: Record<string, unknown> } | undefined> {
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

function statusIncludes(order: { json: Record<string, unknown>; human: Record<string, unknown> }, keyword: string): boolean {
  const status = readObjectField(order.human, 'status') ?? readObjectField(order.json, 'status');
  return describeValue(status).toLowerCase().includes(keyword.toLowerCase());
}

function describeStatus(order: { json: Record<string, unknown>; human: Record<string, unknown> }): string {
  const status = readObjectField(order.human, 'status') ?? readObjectField(order.json, 'status');
  return describeValue(status);
}

function makerMatches(order: { json: Record<string, unknown>; human: Record<string, unknown> }, address: string): boolean {
  const maker = readObjectField(order.human, 'maker') ?? readObjectField(order.json, 'maker');
  return String(maker) === address;
}

function sideMatches(order: { json: Record<string, unknown>; human: Record<string, unknown> }, sideKeyword: 'buy' | 'sell'): boolean {
  const side = readObjectField(order.human, 'side') ?? readObjectField(order.json, 'side');
  return describeValue(side).toLowerCase().includes(sideKeyword);
}

async function findRecentMakerOrder(
  api: ApiPromise,
  maker: string,
  sideKeyword: 'buy' | 'sell',
  fromOrderId: number,
  toOrderIdExclusive: number,
): Promise<number | undefined> {
  for (let orderId = toOrderIdExclusive - 1; orderId >= fromOrderId; orderId -= 1) {
    const order = await tryReadOrder(api, orderId);
    if (!order) {
      continue;
    }
    if (makerMatches(order, maker) && sideMatches(order, sideKeyword)) {
      return orderId;
    }
  }

  return undefined;
}

async function readSmokeTestPrices(api: ApiPromise, marketPrice: number): Promise<{ sellPrice: number; buyPrice: number }> {
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

export const nexMarketSmokeSuite: TestSuite = {
  id: 'nex-market-smoke',
  title: 'NEX market smoke',
  description: 'Place and cancel both sell and buy orders using the current runtime event and storage model.',
  tags: ['market', 'smoke'],
  async run(ctx) {
    const seller = ctx.actors.dave;
    const buyer = ctx.actors.alice;

    await ctx.step('market actors are funded', async () => {
      await ctx.ensureFundsFor(['dave'], 25_000);
      ctx.note(`buyer=${buyer.meta.name ?? buyer.address} uses pre-funded treasury-grade account for deposit-heavy remote buy orders`);
    });

    const { sellPrice, buyPrice } = await ctx.step('resolve safe smoke-test market prices', async () => {
      const price = await ctx.readMarketPrice();
      assert(price > 0, 'market price should be positive');
      const prices = await readSmokeTestPrices(ctx.api, price);
      ctx.note(`marketPrice=${price} sellPrice=${prices.sellPrice} buyPrice=${prices.buyPrice}`);
      return prices;
    });

    const sellOrder = await ctx.step('place a sell order with the 4-arg placeSellOrder call', async () => {
      const beforeNextOrderId = await readNextOrderId(ctx.api);
      const beforeOrders = await readUserOrders(ctx.api, seller.address);

      const tx = (ctx.api.tx as any).nexMarket.placeSellOrder(
        ORDER_AMOUNT,
        sellPrice,
        TRON_ADDRESS,
        null,
      );
      const receipt = await submitTx(ctx.api, tx, seller, 'place sell order');
      assertTxSuccess(receipt, 'sell order should succeed');

      const afterNextOrderId = await readNextOrderId(ctx.api);
      const afterOrders = await readUserOrders(ctx.api, seller.address);
      const orderId = await findRecentMakerOrder(ctx.api, seller.address, 'sell', beforeNextOrderId, afterNextOrderId);
      assert(
        orderId != null,
        `expected to find a recent sell order for ${seller.address} between ids [${beforeNextOrderId}, ${afterNextOrderId})`,
      );

      const order = await readOrder(ctx.api, orderId);
      const status = describeStatus(order);
      assert(
        statusIncludes(order, 'open') || statusIncludes(order, 'fill'),
        `new sell order should be open or filled immediately, actual=${status}`,
      );

      if (afterOrders.includes(orderId)) {
        ctx.note(`seller active order index contains ${orderId}`);
      } else {
        ctx.note(`seller active order index did not retain ${orderId}; status=${status} before=${beforeOrders.length} after=${afterOrders.length}`);
      }

      return { orderId, status };
    });

    await ctx.step('cancel the sell order as owner when it remains open', async () => {
      if (!sellOrder.status.toLowerCase().includes('open')) {
        ctx.note(`skip cancel sell order ${sellOrder.orderId}; current status=${sellOrder.status}`);
        return;
      }

      const tx = (ctx.api.tx as any).nexMarket.cancelOrder(sellOrder.orderId);
      const receipt = await submitTx(ctx.api, tx, seller, 'cancel sell order');
      assertTxSuccess(receipt, 'sell order cancel should succeed');

      const order = await readOrder(ctx.api, sellOrder.orderId);
      assert(statusIncludes(order, 'cancel'), 'sell order should be cancelled');
    });

    const buyOrder = await ctx.step('place a buy order with the current placeBuyOrder call', async () => {
      const beforeNextOrderId = await readNextOrderId(ctx.api);
      const beforeOrders = await readUserOrders(ctx.api, buyer.address);

      const tx = (ctx.api.tx as any).nexMarket.placeBuyOrder(
        ORDER_AMOUNT,
        buyPrice,
        TRON_ADDRESS,
      );
      const receipt = await submitTx(ctx.api, tx, buyer, 'place buy order');
      assertTxSuccess(receipt, 'buy order should succeed');

      const afterNextOrderId = await readNextOrderId(ctx.api);
      const afterOrders = await readUserOrders(ctx.api, buyer.address);
      const orderId = await findRecentMakerOrder(ctx.api, buyer.address, 'buy', beforeNextOrderId, afterNextOrderId);
      assert(
        orderId != null,
        `expected to find a recent buy order for ${buyer.address} between ids [${beforeNextOrderId}, ${afterNextOrderId})`,
      );

      const order = await readOrder(ctx.api, orderId);
      const status = describeStatus(order);
      const side = readObjectField(order.human, 'side') ?? readObjectField(order.json, 'side');
      assert(describeValue(side).toLowerCase().includes('buy'), 'new order side should be buy');
      assert(
        statusIncludes(order, 'open') || statusIncludes(order, 'fill'),
        `new buy order should be open or filled immediately, actual=${status}`,
      );

      if (afterOrders.includes(orderId)) {
        ctx.note(`buyer active order index contains ${orderId}`);
      } else {
        ctx.note(`buyer active order index did not retain ${orderId}; status=${status} before=${beforeOrders.length} after=${afterOrders.length}`);
      }

      return { orderId, status };
    });

    await ctx.step('cancel the buy order as owner when it remains open', async () => {
      if (!buyOrder.status.toLowerCase().includes('open')) {
        ctx.note(`skip cancel buy order ${buyOrder.orderId}; current status=${buyOrder.status}`);
        return;
      }

      const tx = (ctx.api.tx as any).nexMarket.cancelOrder(buyOrder.orderId);
      const receipt = await submitTx(ctx.api, tx, buyer, 'cancel buy order');
      assertTxSuccess(receipt, 'buy order cancel should succeed');

      const order = await readOrder(ctx.api, buyOrder.orderId);
      assert(statusIncludes(order, 'cancel'), 'buy order should be cancelled');
    });
  },
};
