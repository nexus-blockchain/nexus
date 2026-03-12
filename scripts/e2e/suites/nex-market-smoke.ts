import { ApiPromise } from '@polkadot/api';
import { submitTx } from '../framework/api.js';
import { assert, assertEvent, assertTxSuccess } from '../framework/assert.js';
import { codecToHuman, codecToJson, describeValue, readObjectField } from '../framework/codec.js';
import { TestSuite } from '../framework/types.js';
import { nex } from '../framework/units.js';

const TRON_ADDRESS = 'TJYo36u5BbBVKguFVpsBj3yfHdR65VRj7G';

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

function statusIncludes(order: { json: Record<string, unknown>; human: Record<string, unknown> }, keyword: string): boolean {
  const status = readObjectField(order.human, 'status') ?? readObjectField(order.json, 'status');
  return describeValue(status).toLowerCase().includes(keyword.toLowerCase());
}

export const nexMarketSmokeSuite: TestSuite = {
  id: 'nex-market-smoke',
  title: 'NEX market smoke',
  description: 'Place and cancel both sell and buy orders using the current runtime event and storage model.',
  tags: ['market', 'smoke'],
  async run(ctx) {
    const seller = ctx.actors.bob;
    const buyer = ctx.actors.charlie;

    await ctx.step('market actors are funded', async () => {
      await ctx.ensureFunds(25_000);
    });

    const marketPrice = await ctx.step('resolve a safe smoke-test market price', async () => {
      const price = await ctx.readMarketPrice();
      assert(price > 0, 'market price should be positive');
      ctx.note(`marketPrice=${price}`);
      return price;
    });

    const sellOrderId = await ctx.step('place a sell order with the 4-arg placeSellOrder call', async () => {
      const nextOrderId = await readNextOrderId(ctx.api);
      const beforeOrders = await readUserOrders(ctx.api, seller.address);

      const tx = (ctx.api.tx as any).nexMarket.placeSellOrder(
        nex(100).toString(),
        marketPrice,
        TRON_ADDRESS,
        null,
      );
      const receipt = await submitTx(ctx.api, tx, seller, 'place sell order');
      assertTxSuccess(receipt, 'sell order should succeed');
      assertEvent(receipt, 'nexMarket', 'OrderCreated', 'sell order should emit OrderCreated');

      const afterOrders = await readUserOrders(ctx.api, seller.address);
      assert(afterOrders.length === beforeOrders.length + 1, 'seller should gain one active order id');
      assert(afterOrders.includes(nextOrderId), 'seller order index should include new sell order id');

      const order = await readOrder(ctx.api, nextOrderId);
      assert(statusIncludes(order, 'open'), 'new sell order should be open');
      return nextOrderId;
    });

    await ctx.step('cancel the sell order as owner', async () => {
      const tx = (ctx.api.tx as any).nexMarket.cancelOrder(sellOrderId);
      const receipt = await submitTx(ctx.api, tx, seller, 'cancel sell order');
      assertTxSuccess(receipt, 'sell order cancel should succeed');
      assertEvent(receipt, 'nexMarket', 'OrderCancelled', 'sell order cancel should emit OrderCancelled');

      const order = await readOrder(ctx.api, sellOrderId);
      assert(statusIncludes(order, 'cancel'), 'sell order should be cancelled');
    });

    const buyOrderId = await ctx.step('place a buy order with the current placeBuyOrder call', async () => {
      const nextOrderId = await readNextOrderId(ctx.api);
      const beforeOrders = await readUserOrders(ctx.api, buyer.address);

      const tx = (ctx.api.tx as any).nexMarket.placeBuyOrder(
        nex(100).toString(),
        marketPrice,
        TRON_ADDRESS,
      );
      const receipt = await submitTx(ctx.api, tx, buyer, 'place buy order');
      assertTxSuccess(receipt, 'buy order should succeed');
      assertEvent(receipt, 'nexMarket', 'OrderCreated', 'buy order should emit OrderCreated');

      const afterOrders = await readUserOrders(ctx.api, buyer.address);
      assert(afterOrders.length === beforeOrders.length + 1, 'buyer should gain one active order id');
      assert(afterOrders.includes(nextOrderId), 'buyer order index should include new buy order id');

      const order = await readOrder(ctx.api, nextOrderId);
      const side = readObjectField(order.human, 'side') ?? readObjectField(order.json, 'side');
      assert(describeValue(side).toLowerCase().includes('buy'), 'new order side should be buy');
      return nextOrderId;
    });

    await ctx.step('cancel the buy order as owner', async () => {
      const tx = (ctx.api.tx as any).nexMarket.cancelOrder(buyOrderId);
      const receipt = await submitTx(ctx.api, tx, buyer, 'cancel buy order');
      assertTxSuccess(receipt, 'buy order cancel should succeed');
      assertEvent(receipt, 'nexMarket', 'OrderCancelled', 'buy order cancel should emit OrderCancelled');

      const order = await readOrder(ctx.api, buyOrderId);
      assert(statusIncludes(order, 'cancel'), 'buy order should be cancelled');
    });
  },
};
