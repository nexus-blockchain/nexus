import { assertCallShape, assertEvent, assertPallet, assertStorage } from '../framework/metadata.js';
import { TestSuite } from '../framework/types.js';

const REQUIRED_CALLS: Array<{ pallet: string; call: string; args: string[] }> = [
  { pallet: 'nexMarket', call: 'placeSellOrder', args: ['nexAmount', 'usdtPrice', 'tronAddress', 'minFillAmount'] },
  { pallet: 'nexMarket', call: 'placeBuyOrder', args: ['nexAmount', 'usdtPrice', 'buyerTronAddress'] },
  { pallet: 'nexMarket', call: 'reserveSellOrder', args: ['orderId', 'amount', 'buyerTronAddress'] },
  { pallet: 'nexMarket', call: 'acceptBuyOrder', args: ['orderId', 'amount', 'tronAddress'] },
  { pallet: 'nexMarket', call: 'cancelOrder', args: ['orderId'] },
  { pallet: 'entityRegistry', call: 'createEntity', args: ['name', 'logoCid', 'descriptionCid', 'referrer'] },
  { pallet: 'entityRegistry', call: 'updateEntity', args: ['entityId', 'name', 'logoCid', 'descriptionCid', 'metadataUri', 'contactCid'] },
  { pallet: 'entityRegistry', call: 'suspendEntity', args: ['entityId', 'reason'] },
  { pallet: 'entityRegistry', call: 'resumeEntity', args: ['entityId'] },
  { pallet: 'entityRegistry', call: 'setPrimaryShop', args: ['entityId', 'shopId'] },
  { pallet: 'entityShop', call: 'createShop', args: ['entityId', 'name', 'shopType', 'initialFund'] },
  { pallet: 'entityShop', call: 'withdrawOperatingFund', args: ['shopId', 'amount'] },
];

const REQUIRED_STORAGE: Array<{ pallet: string; storage: string }> = [
  { pallet: 'nexMarket', storage: 'orders' },
  { pallet: 'nexMarket', storage: 'userOrders' },
  { pallet: 'nexMarket', storage: 'priceProtection' },
  { pallet: 'entityRegistry', storage: 'entities' },
  { pallet: 'entityRegistry', storage: 'userEntities' },
  { pallet: 'entityRegistry', storage: 'entityShopIds' },
  { pallet: 'entityShop', storage: 'shops' },
  { pallet: 'entityShop', storage: 'entityPrimaryShop' },
];

const REQUIRED_EVENTS: Array<{ pallet: string; event: string }> = [
  { pallet: 'nexMarket', event: 'OrderCreated' },
  { pallet: 'nexMarket', event: 'OrderCancelled' },
  { pallet: 'nexMarket', event: 'UsdtTradeCreated' },
  { pallet: 'entityRegistry', event: 'EntityCreated' },
  { pallet: 'entityRegistry', event: 'EntityUpdated' },
  { pallet: 'entityRegistry', event: 'ShopAddedToEntity' },
  { pallet: 'entityShop', event: 'ShopCreated' },
  { pallet: 'entityShop', event: 'OperatingFundWithdrawn' },
];

export const runtimeContractsSuite: TestSuite = {
  id: 'runtime-contracts',
  title: 'Runtime contracts',
  description: 'Validate the current runtime ABI shape so signature drift fails fast before business flows run.',
  tags: ['metadata', 'contract'],
  async run(ctx) {
    await ctx.step('required pallets are exposed', async () => {
      for (const pallet of ['nexMarket', 'entityRegistry', 'entityShop']) {
        assertPallet(ctx.api, 'tx', pallet);
        assertPallet(ctx.api, 'query', pallet);
        assertPallet(ctx.api, 'events', pallet);
      }
    });

    await ctx.step('critical extrinsic signatures match the rebuilt contract', async () => {
      for (const item of REQUIRED_CALLS) {
        assertCallShape(ctx.api, item.pallet, item.call, item.args);
      }
    });

    await ctx.step('critical storage accessors exist', async () => {
      for (const item of REQUIRED_STORAGE) {
        assertStorage(ctx.api, item.pallet, item.storage);
      }
    });

    await ctx.step('critical events exist', async () => {
      for (const item of REQUIRED_EVENTS) {
        assertEvent(ctx.api, item.pallet, item.event);
      }
    });
  },
};
