import { ApiPromise } from '@polkadot/api';
import { readFreeBalance } from '../framework/accounts.js';
import { submitTx } from '../framework/api.js';
import { assert, assertTxSuccess } from '../framework/assert.js';
import { codecToJson, readObjectField } from '../framework/codec.js';
import { TestSuite } from '../framework/types.js';
import { nex } from '../framework/units.js';

const VALID_TRON_ADDRESSES = {
  seller: 'TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t',
  buyer: 'TQn9Y2khEsLJW1ChVWFMSMeRDow5KcbLSE',
};

function uniqueSuffix(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function toStringMaybe(value: unknown): string {
  if (value == null) {
    return '';
  }
  if (typeof value === 'string') {
    return value;
  }
  if (typeof value === 'number' || typeof value === 'bigint' || typeof value === 'boolean') {
    return String(value);
  }
  if (Array.isArray(value)) {
    return value.map((item) => toStringMaybe(item)).join(',');
  }
  if (typeof (value as any)?.toString === 'function') {
    return (value as any).toString();
  }
  return JSON.stringify(value);
}

function statusContains(record: Record<string, unknown>, ...expected: string[]): boolean {
  const status = readObjectField(record, 'status');
  const text = toStringMaybe(status).toLowerCase();
  return expected.some((item) => text.includes(item.toLowerCase()));
}

async function readUserEntityIds(api: ApiPromise, address: string): Promise<number[]> {
  const query = (api.query as any).entityRegistry.userEntities
    ?? (api.query as any).entityRegistry.userEntity;
  assert(typeof query === 'function', 'entityRegistry user entity index query should exist');
  const value = codecToJson<unknown[]>(await query(address));
  return Array.isArray(value) ? value.map((item) => Number(item)) : [];
}

async function readEntity(api: ApiPromise, entityId: number): Promise<Record<string, unknown>> {
  const value = await (api.query as any).entityRegistry.entities(entityId);
  assert((value as any).isSome, `entity ${entityId} should exist`);
  return codecToJson((value as any).unwrap());
}

async function readShop(api: ApiPromise, shopId: number): Promise<Record<string, unknown>> {
  const value = await (api.query as any).entityShop.shops(shopId);
  assert((value as any).isSome, `shop ${shopId} should exist`);
  return codecToJson((value as any).unwrap());
}

async function readProduct(api: ApiPromise, productId: number): Promise<Record<string, unknown>> {
  const value = await (api.query as any).entityProduct.products(productId);
  assert((value as any).isSome, `product ${productId} should exist`);
  return codecToJson((value as any).unwrap());
}

async function readOrder(api: ApiPromise, orderId: number): Promise<Record<string, unknown>> {
  const value = await (api.query as any).entityTransaction.orders(orderId);
  assert((value as any).isSome, `order ${orderId} should exist`);
  return codecToJson((value as any).unwrap());
}

async function readPendingMember(api: ApiPromise, entityId: number, address: string): Promise<unknown> {
  return codecToJson(await (api.query as any).entityMember.pendingMembers(entityId, address));
}

function readMaxEntitiesPerUser(api: ApiPromise): number {
  const value = (api.consts as any).entityRegistry?.maxEntitiesPerUser;
  const parsed = Number(codecToJson(value));
  return Number.isFinite(parsed) && parsed > 0 ? parsed : 1;
}

export const remoteBusinessFlowsSuite: TestSuite = {
  id: 'remote-business-flows',
  title: 'Remote business flows',
  description: 'Run only the additional remote business flows that are not already covered by the existing smoke suites.',
  tags: ['remote', 'business', 'entity', 'market', 'commission'],
  async run(ctx) {
    const actors = ctx.actors;
    const tx = (ctx.api.tx as any);
    const query = (ctx.api.query as any);

    type BaseContext = {
      ownerName: string;
      owner: typeof actors.alice;
      entityId: number;
      primaryShopId: number;
      shopId: number;
    };

    let baseContext: BaseContext | null = null;

    async function chooseOwner(): Promise<{ ownerName: string; owner: typeof actors.alice; entityIds: number[] }> {
      const maxEntitiesPerUser = readMaxEntitiesPerUser(ctx.api);
      for (const name of ['dave', 'ferdie', 'charlie', 'alice']) {
        const actor = actors[name];
        const entityIds = await readUserEntityIds(ctx.api, actor.address);
        if (entityIds.length < maxEntitiesPerUser) {
          return { ownerName: name, owner: actor, entityIds };
        }
      }
      throw new Error(`No dev actor has remaining entity capacity (maxEntitiesPerUser=${maxEntitiesPerUser})`);
    }

    async function ensureBaseContext(): Promise<BaseContext> {
      if (baseContext) {
        return baseContext;
      }

      const ownerInfo = await chooseOwner();
      await ctx.ensureFundsFor([ownerInfo.ownerName], 25_000);

      const entityId = Number((await query.entityRegistry.nextEntityId()).toString());
      const secondaryShopId = Number((await query.entityShop.nextShopId()).toString());
      const entityName = `rbf-${uniqueSuffix()}`;
      const shopName = `rbf-shop-${uniqueSuffix()}`;

      let receipt = await submitTx(
        ctx.api,
        tx.entityRegistry.createEntity(entityName, null, null, null),
        ownerInfo.owner,
        'create base entity',
      );
      assertTxSuccess(receipt, 'create base entity should succeed');

      const entity = await readEntity(ctx.api, entityId);
      const primaryShopId = Number(readObjectField(entity, 'primaryShopId', 'primary_shop_id'));
      assert(primaryShopId > 0, 'expected auto-created primary shop');

      receipt = await submitTx(
        ctx.api,
        tx.entityShop.createShop(entityId, shopName, 'OnlineStore', nex(200).toString()),
        ownerInfo.owner,
        'create secondary shop',
      );
      assertTxSuccess(receipt, 'create secondary shop should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityShop.addManager(secondaryShopId, actors.bob.address),
        ownerInfo.owner,
        'add shop manager',
      );
      assertTxSuccess(receipt, 'add manager should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityShop.updateShop(
          secondaryShopId,
          `rbf-shop-updated-${uniqueSuffix()}`,
          null,
          null,
          null,
          null,
        ),
        actors.bob,
        'manager updates shop metadata',
      );
      assertTxSuccess(receipt, 'manager update shop should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityShop.setPrimaryShop(entityId, secondaryShopId),
        ownerInfo.owner,
        'set primary shop',
      );
      assertTxSuccess(receipt, 'set primary shop should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityShop.fundOperating(secondaryShopId, nex(5_000).toString()),
        ownerInfo.owner,
        'fund operating',
      );
      assertTxSuccess(receipt, 'fund operating should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityShop.withdrawOperatingFund(secondaryShopId, nex(50).toString()),
        ownerInfo.owner,
        'withdraw operating fund',
      );
      assertTxSuccess(receipt, 'withdraw operating fund should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityShop.removeManager(secondaryShopId, actors.bob.address),
        ownerInfo.owner,
        'remove shop manager',
      );
      assertTxSuccess(receipt, 'remove manager should succeed');

      baseContext = {
        ownerName: ownerInfo.ownerName,
        owner: ownerInfo.owner,
        entityId,
        primaryShopId,
        shopId: secondaryShopId,
      };

      return baseContext;
    }

    const base = await ctx.step('create base entity and secondary shop context', async () => {
      const context = await ensureBaseContext();
      ctx.note(`owner=${context.ownerName} entityId=${context.entityId} primaryShopId=${context.primaryShopId} shopId=${context.shopId}`);
      return context;
    });

    await ctx.step('entity-shop extended lifecycle is reflected in storage', async () => {
      const entity = await readEntity(ctx.api, base.entityId);
      const shop = await readShop(ctx.api, base.shopId);
      const primaryShopId = Number(readObjectField(entity, 'primaryShopId', 'primary_shop_id'));
      const managers = (readObjectField(shop, 'managers') as unknown[]) ?? [];
      assert(primaryShopId === base.shopId, `expected entity primary shop to be ${base.shopId}, got ${primaryShopId}`);
      assert(Array.isArray(managers), 'shop managers should be an array');
      assert(!managers.includes(actors.bob.address), 'bob should have been removed from managers');
      ctx.note(`shop flow verified on entity=${base.entityId} shop=${base.shopId}`);
    });

    await ctx.step('entity-member + entity-loyalty approval onboarding and points flow', async () => {
      await ctx.ensureFundsFor(['charlie', 'dave'], 5_000);

      let receipt = await submitTx(
        ctx.api,
        tx.entityMember.setMemberPolicy(base.shopId, 4),
        base.owner,
        'set member policy approval-required',
      );
      assertTxSuccess(receipt, 'set member policy should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityMember.registerMember(base.shopId, null),
        actors.charlie,
        'charlie register pending',
      );
      assertTxSuccess(receipt, 'charlie register should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityMember.registerMember(base.shopId, null),
        actors.dave,
        'dave register pending',
      );
      assertTxSuccess(receipt, 'dave register should succeed');

      const pendingCharlie = await readPendingMember(ctx.api, base.entityId, actors.charlie.address);
      const pendingDave = await readPendingMember(ctx.api, base.entityId, actors.dave.address);
      assert(pendingCharlie != null, 'charlie pending member record should exist');
      assert(pendingDave != null, 'dave pending member record should exist');

      receipt = await submitTx(
        ctx.api,
        tx.entityMember.batchApproveMembers(base.shopId, [actors.charlie.address, actors.dave.address]),
        base.owner,
        'batch approve members',
      );
      assertTxSuccess(receipt, 'batch approve members should succeed');

      const memberCharlie = await query.entityMember.entityMembers(base.entityId, actors.charlie.address);
      const memberDave = await query.entityMember.entityMembers(base.entityId, actors.dave.address);
      assert((memberCharlie as any).isSome, 'charlie should become approved member');
      assert((memberDave as any).isSome, 'dave should become approved member');

      receipt = await submitTx(
        ctx.api,
        tx.entityLoyalty.enablePoints(base.shopId, 'RemoteFlowPts', 'RFP', 500, 10_000, true),
        base.owner,
        'enable points',
      );
      assertTxSuccess(receipt, 'enable points should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityLoyalty.updatePointsConfig(base.shopId, 800, null, null),
        base.owner,
        'update points config',
      );
      assertTxSuccess(receipt, 'update points config should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityLoyalty.managerIssuePoints(base.shopId, actors.charlie.address, nex(20).toString()),
        base.owner,
        'issue points to charlie',
      );
      assertTxSuccess(receipt, 'issue points should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityLoyalty.transferPoints(base.shopId, actors.dave.address, nex(5).toString()),
        actors.charlie,
        'charlie transfers points',
      );
      assertTxSuccess(receipt, 'transfer points should succeed');

      const daveBeforeRedeem = await readFreeBalance(ctx.api, actors.dave.address);
      receipt = await submitTx(
        ctx.api,
        tx.entityLoyalty.redeemPoints(base.shopId, nex(2).toString()),
        actors.dave,
        'dave redeems points',
      );
      assertTxSuccess(receipt, 'redeem points should succeed');

      const charliePoints = BigInt(toStringMaybe(await query.entityLoyalty.shopPointsBalances(base.shopId, actors.charlie.address)));
      const davePoints = BigInt(toStringMaybe(await query.entityLoyalty.shopPointsBalances(base.shopId, actors.dave.address)));
      const daveAfterRedeem = await readFreeBalance(ctx.api, actors.dave.address);

      assert(charliePoints === nex(15), `expected charlie points = 15 NEX-equivalent, got ${charliePoints}`);
      assert(davePoints === nex(3), `expected dave points = 3 NEX-equivalent, got ${davePoints}`);
      assert(daveAfterRedeem > daveBeforeRedeem, 'dave free balance should increase after redeem');

      ctx.note(`points flow verified: charliePoints=${charliePoints} davePoints=${davePoints}`);
    });

    await ctx.step('entity-product + entity-order physical lifecycle flow', async () => {
      await ctx.ensureFundsFor(['charlie', 'dave'], 5_000);

      const productId = Number((await query.entityProduct.nextProductId()).toString());

      let receipt = await submitTx(
        ctx.api,
        tx.entityProduct.createProduct(
          base.shopId,
          `physical-name-${uniqueSuffix()}`,
          `physical-images-${uniqueSuffix()}`,
          `physical-detail-${uniqueSuffix()}`,
          nex(60).toString(),
          0,
          10,
          'Physical',
          0,
          '',
          '',
          1,
          5,
          'Public',
        ),
        base.owner,
        'create physical product',
      );
      assertTxSuccess(receipt, 'create physical product should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityProduct.updateProduct(
          productId,
          `physical-name-updated-${uniqueSuffix()}`,
          null,
          null,
          nex(75).toString(),
          null,
          12,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ),
        base.owner,
        'update physical product',
      );
      assertTxSuccess(receipt, 'update physical product should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityProduct.publishProduct(productId),
        base.owner,
        'publish physical product',
      );
      assertTxSuccess(receipt, 'publish physical product should succeed');

      const product = await readProduct(ctx.api, productId);
      assert(statusContains(product, 'onsale', 'onSale'), `expected product on sale, got ${toStringMaybe(readObjectField(product, 'status'))}`);

      const orderId1 = Number((await query.entityTransaction.nextOrderId()).toString());
      receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.placeOrder(
          productId,
          1,
          `shipping-${uniqueSuffix()}`,
          null,
          null,
          null,
          `note-${uniqueSuffix()}`,
          null,
        ),
        actors.charlie,
        'charlie place physical order',
      );
      assertTxSuccess(receipt, 'place physical order should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.updateShippingAddress(orderId1, `shipping-updated-${uniqueSuffix()}`),
        actors.charlie,
        'update shipping address',
      );
      assertTxSuccess(receipt, 'update shipping address should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.shipOrder(orderId1, `tracking-${uniqueSuffix()}`),
        base.owner,
        'ship order',
      );
      assertTxSuccess(receipt, 'ship order should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.updateTracking(orderId1, `tracking-updated-${uniqueSuffix()}`),
        base.owner,
        'update tracking',
      );
      assertTxSuccess(receipt, 'update tracking should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.extendConfirmTimeout(orderId1),
        actors.charlie,
        'extend confirm timeout',
      );
      assertTxSuccess(receipt, 'extend confirm timeout should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.confirmReceipt(orderId1),
        actors.charlie,
        'confirm receipt',
      );
      assertTxSuccess(receipt, 'confirm receipt should succeed');

      const completedOrder = await readOrder(ctx.api, orderId1);
      assert(statusContains(completedOrder, 'completed'), `expected completed order, got ${toStringMaybe(readObjectField(completedOrder, 'status'))}`);

      const orderId2 = Number((await query.entityTransaction.nextOrderId()).toString());
      receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.placeOrder(
          productId,
          1,
          `shipping-dave-${uniqueSuffix()}`,
          null,
          null,
          null,
          `note-dave-${uniqueSuffix()}`,
          null,
        ),
        actors.dave,
        'dave place physical order',
      );
      assertTxSuccess(receipt, 'dave place physical order should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.requestRefund(orderId2, `refund-${uniqueSuffix()}`),
        actors.dave,
        'request refund',
      );
      assertTxSuccess(receipt, 'request refund should succeed');

      const disputedOrder = await readOrder(ctx.api, orderId2);
      assert(statusContains(disputedOrder, 'disputed'), `expected disputed order, got ${toStringMaybe(readObjectField(disputedOrder, 'status'))}`);

      receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.approveRefund(orderId2),
        base.owner,
        'approve refund',
      );
      assertTxSuccess(receipt, 'approve refund should succeed');

      const refundedOrder = await readOrder(ctx.api, orderId2);
      assert(statusContains(refundedOrder, 'refunded'), `expected refunded order, got ${toStringMaybe(readObjectField(refundedOrder, 'status'))}`);

      receipt = await submitTx(
        ctx.api,
        tx.entityProduct.unpublishProduct(productId),
        base.owner,
        'unpublish product',
      );
      assertTxSuccess(receipt, 'unpublish product should succeed');

      const unpublished = await readProduct(ctx.api, productId);
      assert(
        statusContains(unpublished, 'offshelf', 'offShelf', 'draft'),
        `expected off-shelf/draft after unpublish, got ${toStringMaybe(readObjectField(unpublished, 'status'))}`,
      );

      ctx.note(`physical product/order flow verified on product=${productId}`);
    });

    await ctx.step('commission admin control-plane flows', async () => {
      let receipt = await submitTx(
        ctx.api,
        tx.commissionSingleLine.setSingleLineConfig(base.entityId, 100, 100, 3, 3, 0, 4, 4),
        base.owner,
        'set single-line config',
      );
      assertTxSuccess(receipt, 'set single-line config should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.commissionSingleLine.updateSingleLineParams(base.entityId, 120, 150, null, null, null, null, null),
        base.owner,
        'update single-line params',
      );
      assertTxSuccess(receipt, 'update single-line params should succeed');

      receipt = await submitTx(ctx.api, tx.commissionSingleLine.pauseSingleLine(base.entityId), base.owner, 'pause single-line');
      assertTxSuccess(receipt, 'pause single-line should succeed');
      receipt = await submitTx(ctx.api, tx.commissionSingleLine.resumeSingleLine(base.entityId), base.owner, 'resume single-line');
      assertTxSuccess(receipt, 'resume single-line should succeed');
      receipt = await submitTx(
        ctx.api,
        tx.commissionSingleLine.scheduleConfigChange(base.entityId, 130, 140, 3, 3, 0, 5, 5),
        base.owner,
        'schedule single-line config',
      );
      assertTxSuccess(receipt, 'schedule single-line config should succeed');

      const singleLineConfig = codecToJson(await query.commissionSingleLine.singleLineConfigs(base.entityId));
      const singleLinePending = codecToJson(await query.commissionSingleLine.pendingConfigChanges(base.entityId));
      assert(singleLineConfig != null, 'single-line config should exist');
      assert(singleLinePending != null, 'single-line pending config should exist');

      receipt = await submitTx(
        ctx.api,
        tx.commissionMultiLevel.setMultiLevelConfig(
          base.entityId,
          [
            { rate: 200, required_directs: 0, required_team_size: 0, required_spent: 0 },
            { rate: 100, required_directs: 0, required_team_size: 0, required_spent: 0 },
          ],
          300,
        ),
        base.owner,
        'set multi-level config',
      );
      assertTxSuccess(receipt, 'set multi-level config should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.commissionMultiLevel.addTier(
          base.entityId,
          2,
          { rate: 50, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ),
        base.owner,
        'add multi-level tier',
      );
      assertTxSuccess(receipt, 'add multi-level tier should succeed');

      receipt = await submitTx(ctx.api, tx.commissionMultiLevel.removeTier(base.entityId, 2), base.owner, 'remove multi-level tier');
      assertTxSuccess(receipt, 'remove multi-level tier should succeed');
      receipt = await submitTx(ctx.api, tx.commissionMultiLevel.pauseMultiLevel(base.entityId), base.owner, 'pause multi-level');
      assertTxSuccess(receipt, 'pause multi-level should succeed');
      receipt = await submitTx(ctx.api, tx.commissionMultiLevel.resumeMultiLevel(base.entityId), base.owner, 'resume multi-level');
      assertTxSuccess(receipt, 'resume multi-level should succeed');
      receipt = await submitTx(
        ctx.api,
        tx.commissionMultiLevel.scheduleConfigChange(
          base.entityId,
          [
            { rate: 220, required_directs: 0, required_team_size: 0, required_spent: 0 },
            { rate: 80, required_directs: 0, required_team_size: 0, required_spent: 0 },
          ],
          320,
        ),
        base.owner,
        'schedule multi-level config',
      );
      assertTxSuccess(receipt, 'schedule multi-level config should succeed');

      const multiConfig = codecToJson(await query.commissionMultiLevel.multiLevelConfigs(base.entityId));
      const multiPending = codecToJson(await query.commissionMultiLevel.pendingConfigs(base.entityId));
      assert(multiConfig != null, 'multi-level config should exist');
      assert(multiPending != null, 'multi-level pending config should exist');

      receipt = await submitTx(
        ctx.api,
        tx.commissionPoolReward.setPoolRewardConfig(base.entityId, [[0, 10_000]], 14_400),
        base.owner,
        'set pool reward config',
      );
      assertTxSuccess(receipt, 'set pool reward config should succeed');

      receipt = await submitTx(ctx.api, tx.commissionPoolReward.pausePoolReward(base.entityId), base.owner, 'pause pool reward');
      assertTxSuccess(receipt, 'pause pool reward should succeed');
      receipt = await submitTx(ctx.api, tx.commissionPoolReward.resumePoolReward(base.entityId), base.owner, 'resume pool reward');
      assertTxSuccess(receipt, 'resume pool reward should succeed');
      receipt = await submitTx(
        ctx.api,
        tx.commissionPoolReward.schedulePoolRewardConfigChange(base.entityId, [[0, 9_000], [1, 1_000]], 14_400),
        base.owner,
        'schedule pool reward config',
      );
      assertTxSuccess(receipt, 'schedule pool reward config should succeed');

      const poolConfig = codecToJson(await query.commissionPoolReward.poolRewardConfigs(base.entityId));
      const poolPending = codecToJson(await query.commissionPoolReward.pendingPoolRewardConfig(base.entityId));
      assert(poolConfig != null, 'pool reward config should exist');
      assert(poolPending != null, 'pool reward pending config should exist');

      ctx.note(`commission admin flows verified on entity=${base.entityId}`);
    });

    await ctx.step('nex-market matched sell order → reserve → confirm payment → seller settlement', async () => {
      await ctx.ensureFundsFor(['charlie'], 65_000);

      const marketPrice = await ctx.readMarketPrice();
      assert(marketPrice > 0, 'market price should be positive');

      const orderId = Number((await query.nexMarket.nextOrderId()).toString());
      let receipt = await submitTx(
        ctx.api,
        tx.nexMarket.placeSellOrder(nex(10).toString(), marketPrice, VALID_TRON_ADDRESSES.seller, null),
        actors.bob,
        'place sell order',
      );
      assertTxSuccess(receipt, 'place sell order should succeed');

      const orderAfterCreate = codecToJson<Record<string, unknown>>(await query.nexMarket.orders(orderId));
      assert(orderAfterCreate != null, 'sell order should exist');
      assert(statusContains(orderAfterCreate, 'open'), `expected open sell order, got ${toStringMaybe(readObjectField(orderAfterCreate, 'status'))}`);

      const tradeId = Number((await query.nexMarket.nextUsdtTradeId()).toString());
      receipt = await submitTx(
        ctx.api,
        tx.nexMarket.reserveSellOrder(orderId, null, VALID_TRON_ADDRESSES.buyer),
        actors.charlie,
        'reserve sell order',
      );
      assertTxSuccess(receipt, 'reserve sell order should succeed');

      const tradeAfterReserve = codecToJson<Record<string, unknown>>(await query.nexMarket.usdtTrades(tradeId));
      assert(tradeAfterReserve != null, 'trade should exist after reserve');
      assert(statusContains(tradeAfterReserve, 'awaitingpayment'), `expected awaiting payment trade, got ${toStringMaybe(readObjectField(tradeAfterReserve, 'status'))}`);

      receipt = await submitTx(ctx.api, tx.nexMarket.confirmPayment(tradeId), actors.charlie, 'confirm payment');
      assertTxSuccess(receipt, 'confirm payment should succeed');
      receipt = await submitTx(ctx.api, tx.nexMarket.sellerConfirmReceived(tradeId), actors.bob, 'seller confirm received');
      assertTxSuccess(receipt, 'seller confirm received should succeed');

      const tradeCompleted = codecToJson<Record<string, unknown>>(await query.nexMarket.usdtTrades(tradeId));
      const orderFilled = codecToJson<Record<string, unknown>>(await query.nexMarket.orders(orderId));
      assert(statusContains(tradeCompleted, 'completed'), `expected completed trade, got ${toStringMaybe(readObjectField(tradeCompleted, 'status'))}`);
      assert(statusContains(orderFilled, 'filled'), `expected filled order, got ${toStringMaybe(readObjectField(orderFilled, 'status'))}`);

      ctx.note(`market trade flow verified on order=${orderId} trade=${tradeId}`);
    });
  },
};
