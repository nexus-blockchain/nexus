import type { ApiPromise } from '@polkadot/api';
import { submitTx } from '../framework/api.js';
import { assert, assertEqual, assertEvent, assertTxSuccess, findEvent } from '../framework/assert.js';
import { codecToHuman, codecToJson, coerceNumber, decodeTextValue, readObjectField } from '../framework/codec.js';
import { TestSuite } from '../framework/types.js';
import { formatNex, nex } from '../framework/units.js';

const SHOP_FUND = nex(2_000);
const PRODUCT_PRICE = nex(100);
const COMMISSION_MASK =
  0b0000_0010 + // MULTI_LEVEL
  0b1000_0000 + // SINGLE_LINE_UPLINE
  0b1_0000_0000 + // SINGLE_LINE_DOWNLINE
  0b10_0000_0000; // POOL_REWARD

function bytes(value: string): string {
  return value;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function asBigInt(value: unknown): bigint {
  if (typeof value === 'bigint') {
    return value;
  }
  if (typeof value === 'number') {
    return BigInt(value);
  }
  if (typeof value === 'string') {
    return BigInt(value.replace(/,/g, '').trim());
  }
  if (value && typeof (value as any).toString === 'function') {
    return BigInt((value as any).toString());
  }
  throw new Error(`Unable to coerce value to bigint: ${String(value)}`);
}

function asOptionalNumber(value: unknown): number | undefined {
  if (value == null) {
    return undefined;
  }
  if (typeof value === 'object' && value !== null && 'toJSON' in (value as any)) {
    return asOptionalNumber(codecToJson(value));
  }
  return coerceNumber(value);
}

async function readEntityIds(api: ApiPromise, address: string): Promise<number[]> {
  const query = (api.query as any).entityRegistry.userEntities
    ?? (api.query as any).entityRegistry.userEntity;
  assert(typeof query === 'function', 'entityRegistry user entity index query should exist');
  const value = await query(address);
  const json = codecToJson<unknown[]>(value);
  return Array.isArray(json) ? json.map((item) => Number(item)) : [];
}

function readMaxEntitiesPerUser(api: ApiPromise): number {
  const value = (api.consts as any).entityRegistry?.maxEntitiesPerUser;
  return coerceNumber(codecToJson(value)) ?? 1;
}

async function readEntity(api: ApiPromise, entityId: number): Promise<{ json: Record<string, unknown>; human: Record<string, unknown> }> {
  const value = await (api.query as any).entityRegistry.entities(entityId);
  assert((value as any).isSome, `entity ${entityId} should exist`);
  const entity = (value as any).unwrap();
  return {
    json: codecToJson<Record<string, unknown>>(entity),
    human: codecToHuman<Record<string, unknown>>(entity),
  };
}

async function readProduct(api: ApiPromise, productId: number): Promise<{ json: Record<string, unknown>; human: Record<string, unknown> }> {
  const value = await (api.query as any).entityProduct.products(productId);
  assert((value as any).isSome, `product ${productId} should exist`);
  const product = (value as any).unwrap();
  return {
    json: codecToJson<Record<string, unknown>>(product),
    human: codecToHuman<Record<string, unknown>>(product),
  };
}

async function readOrder(api: ApiPromise, orderId: number): Promise<{ json: Record<string, unknown>; human: Record<string, unknown> }> {
  const value = await (api.query as any).entityTransaction.orders(orderId);
  assert((value as any).isSome, `order ${orderId} should exist`);
  const order = (value as any).unwrap();
  return {
    json: codecToJson<Record<string, unknown>>(order),
    human: codecToHuman<Record<string, unknown>>(order),
  };
}

async function readMember(api: ApiPromise, entityId: number, address: string): Promise<{ json: Record<string, unknown>; human: Record<string, unknown> }> {
  const value = await (api.query as any).entityMember.entityMembers(entityId, address);
  assert((value as any).isSome, `${address} should be a member of entity ${entityId}`);
  const member = (value as any).unwrap();
  return {
    json: codecToJson<Record<string, unknown>>(member),
    human: codecToHuman<Record<string, unknown>>(member),
  };
}

async function readCommissionStats(api: ApiPromise, entityId: number, address: string): Promise<Record<string, unknown>> {
  return codecToJson(await (api.query as any).commissionCore.memberCommissionStats(entityId, address));
}

async function readShoppingBalance(api: ApiPromise, entityId: number, address: string): Promise<bigint> {
  return asBigInt(await (api.query as any).entityLoyalty.memberShoppingBalance(entityId, address));
}

async function readUnallocatedPool(api: ApiPromise, entityId: number): Promise<bigint> {
  return asBigInt(await (api.query as any).commissionCore.unallocatedPool(entityId));
}

async function readSingleLineIndex(api: ApiPromise, entityId: number, address: string): Promise<number | undefined> {
  const value = await (api.query as any).commissionSingleLine.singleLineIndex(entityId, address);
  if ((value as any).isSome) {
    return (value as any).unwrap().toNumber();
  }
  const json = codecToJson(value);
  return asOptionalNumber(json);
}

async function readLastClaimedRound(api: ApiPromise, entityId: number, address: string): Promise<number> {
  return Number((await (api.query as any).commissionPoolReward.lastClaimedRound(entityId, address)).toString());
}

function resolvePrimaryShopId(entity: { json: Record<string, unknown>; human: Record<string, unknown> }): number {
  const primaryShopId = coerceNumber(readObjectField(entity.json, 'primaryShopId', 'primary_shop_id'))
    ?? coerceNumber(readObjectField(entity.human, 'primaryShopId', 'primary_shop_id'));
  assert(primaryShopId != null && primaryShopId > 0, 'expected entity to have an auto-created primary shop');
  return primaryShopId;
}

function decodeStatus(record: { json: Record<string, unknown>; human: Record<string, unknown> }, field: string): string {
  const value = readObjectField(record.human, field) ?? readObjectField(record.json, field);
  return String(value ?? '');
}

function assertEventIfPresent(
  receipt: { events: Array<{ section: string; method: string }> },
  section: string,
  method: string,
  message: string,
): void {
  if (receipt.events.length === 0) {
    return;
  }
  const decodedEvents = receipt.events.filter((event) => event.section && event.method);
  if (decodedEvents.length === 0) {
    return;
  }
  if (!decodedEvents.some((event) => event.section === section)) {
    return;
  }
  assertEvent({ ...receipt, events: decodedEvents } as any, section, method, message);
}

function readNumericEventField(
  data: unknown,
  ...candidates: string[]
): number | undefined {
  const direct = coerceNumber(readObjectField(data, ...candidates));
  if (direct != null) {
    return direct;
  }

  if (Array.isArray(data)) {
    for (const item of data) {
      const parsed = coerceNumber(item);
      if (parsed != null) {
        return parsed;
      }
    }
  }

  return undefined;
}

async function waitForEntityIndex(
  api: ApiPromise,
  address: string,
  entityId: number,
  attempts: number = 5,
  delayMs: number = 1_500,
): Promise<number[]> {
  let lastIds: number[] = [];

  for (let attempt = 0; attempt < attempts; attempt += 1) {
    lastIds = await readEntityIds(api, address);
    if (lastIds.includes(entityId)) {
      return lastIds;
    }

    if (attempt < attempts - 1) {
      await sleep(delayMs);
    }
  }

  return lastIds;
}

async function waitForNewEntityId(
  api: ApiPromise,
  address: string,
  beforeEntityIds: number[],
  attempts: number = 5,
  delayMs: number = 1_500,
): Promise<{ entityId?: number; ids: number[] }> {
  let lastIds: number[] = beforeEntityIds;

  for (let attempt = 0; attempt < attempts; attempt += 1) {
    lastIds = await readEntityIds(api, address);
    const created = lastIds.find((candidate) => !beforeEntityIds.includes(candidate));
    if (created != null) {
      return { entityId: created, ids: lastIds };
    }

    if (attempt < attempts - 1) {
      await sleep(delayMs);
    }
  }

  return { ids: lastIds };
}

export const entityCommerceCommissionFlowSuite: TestSuite = {
  id: 'entity-commerce-commission-flow',
  title: 'Entity commerce + commission flow',
  description: 'Create an entity commerce setup, route orders through member + single-line + multi-level + pool reward, then withdraw commission into loyalty shopping balance and spend it.',
  tags: ['entity', 'commission', 'loyalty', 'market', 'smoke'],
  async run(ctx) {
    const bob = ctx.actors.bob;
    const charlie = ctx.actors.charlie;
    const dave = ctx.actors.dave;
    const resumeEntityId = coerceNumber(process.env.E2E_RESUME_ENTITY_ID);
    const resumeShopId = coerceNumber(process.env.E2E_RESUME_SHOP_ID);
    const resumeSellerName = process.env.E2E_RESUME_SELLER ?? 'ferdie';
    const isResume = resumeEntityId != null && resumeShopId != null;

    await ctx.step('actors are funded and nex-market price oracle is usable', async () => {
      await ctx.ensureFunds(25_000);
      const marketPrice = await ctx.readMarketPrice();
      assert(marketPrice > 0, 'nex-market price should be available for product deposit pricing');
      ctx.note(`marketPrice=${marketPrice}`);
    });

    const seller = isResume
      ? await ctx.step('select the seller actor for the resume context', async () => {
        const actor = ctx.actors[resumeSellerName];
        assert(actor != null, `resume seller actor should exist: ${resumeSellerName}`);
        ctx.note(`resume seller=${resumeSellerName} address=${actor.address}`);
        return actor;
      })
      : await ctx.step('select a seller actor with remaining entity capacity', async () => {
        const maxEntitiesPerUser = readMaxEntitiesPerUser(ctx.api);

        for (const candidate of [ctx.actors.ferdie, ctx.actors.eve, ctx.actors.alice]) {
          const ids = await readEntityIds(ctx.api, candidate.address);
          if (ids.length < maxEntitiesPerUser) {
            ctx.note(`seller=${candidate.meta.name ?? candidate.address} existingEntities=${ids.length}/${maxEntitiesPerUser}`);
            return candidate;
          }
        }

        throw new Error(`no seller actor has remaining entity capacity (max=${maxEntitiesPerUser})`);
      });

    const { entityId, shopId } = isResume
      ? await ctx.step('resume from an existing configured entity and shop', async () => {
        assert(resumeEntityId != null && resumeShopId != null, 'resume entity/shop ids should be provided together');
        const entity = await readEntity(ctx.api, resumeEntityId);
        const resolvedShopId = resolvePrimaryShopId(entity);
        assertEqual(resolvedShopId, resumeShopId, 'resume shop id should match the entity primary shop');
        ctx.note(`resumeEntityId=${resumeEntityId} resumeShopId=${resumeShopId}`);
        return { entityId: resumeEntityId, shopId: resumeShopId };
      })
      : await ctx.step('create an entity and capture its auto-created primary shop', async () => {
        const entityName = `flow-${Date.now()}`;
        const beforeEntityIds = await readEntityIds(ctx.api, seller.address);

        const createTx = (ctx.api.tx as any).entityRegistry.createEntity(bytes(entityName), null, null, null);
        const createReceipt = await submitTx(ctx.api, createTx, seller, 'create entity');
        assertTxSuccess(createReceipt, 'create entity should succeed');
        const createdEvent = findEvent(createReceipt, 'entityRegistry', 'EntityCreated');

        let entityId = createdEvent
          ? readNumericEventField(createdEvent.data, 'entityId', 'entity_id')
          : undefined;

        let afterEntityIds: number[];
        if (entityId != null && entityId > 0) {
          afterEntityIds = await waitForEntityIndex(ctx.api, seller.address, entityId);
        } else {
          const detected = await waitForNewEntityId(ctx.api, seller.address, beforeEntityIds);
          entityId = detected.entityId;
          afterEntityIds = detected.ids;
        }

        assert(entityId != null && entityId > 0, 'expected to detect the newly created entity id');
        assert(afterEntityIds.includes(entityId), 'owner entity index should include the newly created entity id');

        const entity = await readEntity(ctx.api, entityId);
        const shopId = resolvePrimaryShopId(entity);
        ctx.note(`entityId=${entityId} primaryShopId=${shopId}`);
        return { entityId, shopId };
      });

    if (!isResume) {
      await ctx.step('fund the primary shop operating account for product deposits and payouts', async () => {
        const tx = (ctx.api.tx as any).entityShop.fundOperating(shopId, SHOP_FUND.toString());
        const receipt = await submitTx(ctx.api, tx, seller, 'fund operating');
        assertTxSuccess(receipt, 'fund operating should succeed');
        assertEventIfPresent(receipt, 'entityShop', 'OperatingFundDeposited', 'fund operating should emit OperatingFundDeposited');
        ctx.note(`shopOperatingFund=${formatNex(SHOP_FUND)}`);
      });

      await ctx.step('open member registration so a referral chain can be created explicitly', async () => {
        const tx = (ctx.api.tx as any).entityMember.setMemberPolicy(shopId, 0);
        const receipt = await submitTx(ctx.api, tx, seller, 'set member policy');
        assertTxSuccess(receipt, 'set member policy should succeed');
      });

      await ctx.step('register Bob → Charlie → Dave as members with a referral chain', async () => {
        const bobReceipt = await submitTx(
          ctx.api,
          (ctx.api.tx as any).entityMember.registerMember(shopId, null),
          bob,
          'register bob',
        );
        assertTxSuccess(bobReceipt, 'bob should register successfully');
        assertEventIfPresent(bobReceipt, 'entityMember', 'MemberRegistered', 'bob registration should emit MemberRegistered');

        const charlieReceipt = await submitTx(
          ctx.api,
          (ctx.api.tx as any).entityMember.registerMember(shopId, bob.address),
          charlie,
          'register charlie',
        );
        assertTxSuccess(charlieReceipt, 'charlie should register successfully');
        assertEventIfPresent(charlieReceipt, 'entityMember', 'MemberRegistered', 'charlie registration should emit MemberRegistered');

        const daveReceipt = await submitTx(
          ctx.api,
          (ctx.api.tx as any).entityMember.registerMember(shopId, charlie.address),
          dave,
          'register dave',
        );
        assertTxSuccess(daveReceipt, 'dave should register successfully');
        assertEventIfPresent(daveReceipt, 'entityMember', 'MemberRegistered', 'dave registration should emit MemberRegistered');

        const charlieMember = await readMember(ctx.api, entityId, charlie.address);
        const daveMember = await readMember(ctx.api, entityId, dave.address);
        assertEqual(
          String(readObjectField(charlieMember.json, 'referrer')),
          bob.address,
          'charlie referrer should be bob',
        );
        assertEqual(
          String(readObjectField(daveMember.json, 'referrer')),
          charlie.address,
          'dave referrer should be charlie',
        );
      });

      await ctx.step('activate the members so they are eligible for commission distribution', async () => {
        for (const actor of [bob, charlie, dave]) {
          const receipt = await submitTx(
            ctx.api,
            (ctx.api.tx as any).entityMember.activateMember(shopId, actor.address),
            seller,
            `activate ${actor.meta.name ?? actor.address}`,
          );
          assertTxSuccess(receipt, `activate ${actor.address} should succeed`);
          assertEventIfPresent(receipt, 'entityMember', 'MemberActivated', 'activation should emit MemberActivated');
        }
      });

      await ctx.step('configure commission core, single-line, multi-level, pool reward, and fixed-rate withdrawal', async () => {
        const commissionModes = COMMISSION_MASK;
        const withdrawalMode = { FixedRate: { repurchase_rate: 5000 } };
        const defaultTier = {
          withdrawal_rate: 5000,
          repurchase_rate: 5000,
        };

        const setRateReceipt = await submitTx(
          ctx.api,
          (ctx.api.tx as any).commissionCore.setCommissionRate(entityId, 2000),
          seller,
          'set commission rate',
        );
        assertTxSuccess(setRateReceipt, 'set commission rate should succeed');
        assertEventIfPresent(setRateReceipt, 'commissionCore', 'CommissionConfigUpdated', 'set commission rate should emit CommissionConfigUpdated');

        const setModesReceipt = await submitTx(
          ctx.api,
          (ctx.api.tx as any).commissionCore.setCommissionModes(entityId, commissionModes),
          seller,
          'set commission modes',
        );
        assertTxSuccess(setModesReceipt, 'set commission modes should succeed');
        assertEventIfPresent(setModesReceipt, 'commissionCore', 'CommissionModesUpdated', 'set commission modes should emit CommissionModesUpdated');

        const enableReceipt = await submitTx(
          ctx.api,
          (ctx.api.tx as any).commissionCore.enableCommission(entityId, true),
          seller,
          'enable commission',
        );
        assertTxSuccess(enableReceipt, 'enable commission should succeed');
        assertEventIfPresent(enableReceipt, 'commissionCore', 'CommissionConfigUpdated', 'enable commission should emit CommissionConfigUpdated');

        const withdrawalReceipt = await submitTx(
          ctx.api,
          (ctx.api.tx as any).commissionCore.setWithdrawalConfig(
            entityId,
            withdrawalMode,
            defaultTier,
            [],
            0,
            true,
          ),
          seller,
          'set withdrawal config',
        );
        assertTxSuccess(withdrawalReceipt, 'set withdrawal config should succeed');
        assertEventIfPresent(withdrawalReceipt, 'commissionCore', 'WithdrawalConfigUpdated', 'set withdrawal config should emit WithdrawalConfigUpdated');

        const singleLineReceipt = await submitTx(
          ctx.api,
          (ctx.api.tx as any).commissionSingleLine.setSingleLineConfig(
            entityId,
            100,
            100,
            3,
            3,
            0,
            3,
            3,
          ),
          seller,
          'set single-line config',
        );
        assertTxSuccess(singleLineReceipt, 'set single-line config should succeed');
        assertEventIfPresent(singleLineReceipt, 'commissionSingleLine', 'SingleLineConfigUpdated', 'set single-line config should emit SingleLineConfigUpdated');

        const multiLevelReceipt = await submitTx(
          ctx.api,
          (ctx.api.tx as any).commissionMultiLevel.setMultiLevelConfig(
            entityId,
            [
              { rate: 200, required_directs: 0, required_team_size: 0, required_spent: 0 },
              { rate: 100, required_directs: 0, required_team_size: 0, required_spent: 0 },
            ],
            300,
          ),
          seller,
          'set multi-level config',
        );
        assertTxSuccess(multiLevelReceipt, 'set multi-level config should succeed');
        assertEventIfPresent(multiLevelReceipt, 'commissionMultiLevel', 'MultiLevelConfigUpdated', 'set multi-level config should emit MultiLevelConfigUpdated');

        const poolRewardReceipt = await submitTx(
          ctx.api,
          (ctx.api.tx as any).commissionPoolReward.setPoolRewardConfig(
            entityId,
            [[0, 10_000]],
            14_400,
          ),
          seller,
          'set pool reward config',
        );
        assertTxSuccess(poolRewardReceipt, 'set pool reward config should succeed');
        assertEventIfPresent(poolRewardReceipt, 'commissionPoolReward', 'PoolRewardConfigUpdated', 'set pool reward config should emit PoolRewardConfigUpdated');
      });
    } else {
      await ctx.step('verify the existing membership and commission prerequisites before resuming', async () => {
        const charlieMember = await readMember(ctx.api, entityId, charlie.address);
        const daveMember = await readMember(ctx.api, entityId, dave.address);
        assertEqual(
          String(readObjectField(charlieMember.json, 'referrer')),
          bob.address,
          'charlie referrer should already be bob in the resume context',
        );
        assertEqual(
          String(readObjectField(daveMember.json, 'referrer')),
          charlie.address,
          'dave referrer should already be charlie in the resume context',
        );
        assert(
          Boolean(readObjectField(charlieMember.json, 'activated')),
          'charlie should already be activated in the resume context',
        );
        assert(
          Boolean(readObjectField(daveMember.json, 'activated')),
          'dave should already be activated in the resume context',
        );

        const commissionConfig = codecToJson<Record<string, unknown>>(await (ctx.api.query as any).commissionCore.commissionConfigs(entityId));
        assert(
          asBigInt(readObjectField(commissionConfig, 'maxCommissionRate', 'max_commission_rate') ?? 0) > 0n,
          'resume context should already have commission core configured',
        );

        const withdrawalConfig = await (ctx.api.query as any).commissionCore.withdrawalConfigs(entityId);
        const singleLineConfig = await (ctx.api.query as any).commissionSingleLine.singleLineConfigs(entityId);
        const multiLevelConfig = await (ctx.api.query as any).commissionMultiLevel.multiLevelConfigs(entityId);
        const poolRewardConfig = await (ctx.api.query as any).commissionPoolReward.poolRewardConfigs(entityId);

        assert(!(withdrawalConfig as any).isNone, 'resume context should already have a withdrawal config');
        assert(!(singleLineConfig as any).isNone, 'resume context should already have a single-line config');
        assert(!(multiLevelConfig as any).isNone, 'resume context should already have a multi-level config');
        assert(!(poolRewardConfig as any).isNone, 'resume context should already have a pool reward config');
      });
    }

    const productId = await ctx.step('create and publish a members-only digital product priced via the active nex-market oracle', async () => {
      const nextProductId = Number((await (ctx.api.query as any).entityProduct.nextProductId()).toString());
      const category = 'Digital';
      const visibility = 'MembersOnly';

      const createReceipt = await submitTx(
        ctx.api,
        (ctx.api.tx as any).entityProduct.createProduct(
          shopId,
          bytes(`name-${Date.now()}`),
          bytes(`images-${Date.now()}`),
          bytes(`detail-${Date.now()}`),
          PRODUCT_PRICE.toString(),
          0,
          0,
          category,
          0,
          bytes(''),
          bytes(''),
          0,
          0,
          visibility,
        ),
        seller,
        'create product',
      );
      assertTxSuccess(createReceipt, 'create product should succeed');
      assertEventIfPresent(createReceipt, 'entityProduct', 'ProductCreated', 'create product should emit ProductCreated');

      const publishReceipt = await submitTx(
        ctx.api,
        (ctx.api.tx as any).entityProduct.publishProduct(nextProductId),
        seller,
        'publish product',
      );
      assertTxSuccess(publishReceipt, 'publish product should succeed');
      assertEventIfPresent(publishReceipt, 'entityProduct', 'ProductStatusChanged', 'publish product should emit ProductStatusChanged');

      const product = await readProduct(ctx.api, nextProductId);
      assert(
        decodeStatus(product, 'status').toLowerCase().includes('onsale'),
        'product should be on sale after publishing',
      );
      assert(
        decodeStatus(product, 'visibility').toLowerCase().includes('members'),
        'product should require membership visibility',
      );

      return nextProductId;
    });

    await ctx.step('seed the single-line membership order by letting Bob, Charlie, and Dave buy once each', async () => {
      for (const [expectedIndex, actor] of [bob, charlie, dave].entries()) {
        const receipt = await submitTx(
          ctx.api,
          (ctx.api.tx as any).entityTransaction.placeOrder(
            productId,
            1,
            null,
            null,
            null,
            null,
            null,
            null,
          ),
          actor,
          `seed order for ${actor.meta.name ?? actor.address}`,
        );
        assertTxSuccess(receipt, 'seed digital order should succeed');
        assertEventIfPresent(receipt, 'entityTransaction', 'OrderCompleted', 'digital seed order should auto-complete');

        const singleLineIndex = await readSingleLineIndex(ctx.api, entityId, actor.address);
        assertEqual(singleLineIndex, expectedIndex, `single-line index should be assigned for ${actor.address}`);
      }
    });

    const commissionSnapshot = await ctx.step('run a second Charlie order that exercises member + single-line + multi-level + pool reward together', async () => {
      const bobBefore = await readCommissionStats(ctx.api, entityId, bob.address);
      const daveBefore = await readCommissionStats(ctx.api, entityId, dave.address);
      const poolBefore = await readUnallocatedPool(ctx.api, entityId);
      const nextOrderId = Number((await (ctx.api.query as any).entityTransaction.nextOrderId()).toString());

      const receipt = await submitTx(
        ctx.api,
        (ctx.api.tx as any).entityTransaction.placeOrder(
          productId,
          1,
          null,
          null,
          null,
          null,
          null,
          null,
        ),
        charlie,
        'charlie commission order',
      );
      assertTxSuccess(receipt, 'charlie commission order should succeed');
      assertEventIfPresent(receipt, 'entityTransaction', 'OrderCompleted', 'charlie commission order should complete');

      const order = await readOrder(ctx.api, nextOrderId);
      assert(
        decodeStatus(order, 'status').toLowerCase().includes('completed'),
        'charlie commission order should be completed',
      );

      const bobAfter = await readCommissionStats(ctx.api, entityId, bob.address);
      const daveAfter = await readCommissionStats(ctx.api, entityId, dave.address);
      const poolAfter = await readUnallocatedPool(ctx.api, entityId);

      assert(
        asBigInt(readObjectField(bobAfter, 'pending') ?? 0) > asBigInt(readObjectField(bobBefore, 'pending') ?? 0),
        'bob pending commission should increase after Charlie order',
      );
      assert(
        asBigInt(readObjectField(daveAfter, 'pending') ?? 0) > asBigInt(readObjectField(daveBefore, 'pending') ?? 0),
        'dave pending commission should increase after Charlie order',
      );
      assert(poolAfter > poolBefore, 'unallocated pool should grow after Charlie order');

      ctx.note(`poolAfterCharlieOrder=${formatNex(poolAfter)}`);
      return { bobAfter, poolAfter };
    });

    const shoppingBalanceAfterWithdraw = await ctx.step('withdraw Bob commission with a 50% fixed repurchase split into loyalty shopping balance', async () => {
      const bobPendingBefore = asBigInt(readObjectField(commissionSnapshot.bobAfter, 'pending') ?? 0);
      assert(bobPendingBefore > 0n, 'bob should have pending commission before withdraw');

      const shoppingBefore = await readShoppingBalance(ctx.api, entityId, bob.address);

      const receipt = await submitTx(
        ctx.api,
        (ctx.api.tx as any).commissionCore.withdrawCommission(entityId, null, null, null),
        bob,
        'withdraw bob commission',
      );
      assertTxSuccess(receipt, 'bob commission withdraw should succeed');
      assertEventIfPresent(receipt, 'commissionCore', 'TieredWithdrawal', 'withdraw should emit TieredWithdrawal');
      assertEventIfPresent(receipt, 'entityLoyalty', 'ShoppingBalanceCredited', 'withdraw should credit shopping balance');

      const shoppingAfter = await readShoppingBalance(ctx.api, entityId, bob.address);
      const bobAfterWithdraw = await readCommissionStats(ctx.api, entityId, bob.address);
      const bobPendingAfter = asBigInt(readObjectField(bobAfterWithdraw, 'pending') ?? 0);
      const repurchased = asBigInt(readObjectField(bobAfterWithdraw, 'repurchased') ?? 0);

      assert(shoppingAfter > shoppingBefore, 'bob shopping balance should increase after withdraw');
      assert(bobPendingAfter < bobPendingBefore, 'bob pending commission should decrease after withdraw');
      assert(repurchased > 0n, 'bob repurchased amount should be tracked after withdraw');

      ctx.note(`bobShoppingBalance=${formatNex(shoppingAfter)}`);
      return shoppingAfter;
    });

    await ctx.step('spend part of Bob shopping balance in a new order and verify loyalty/order coupling', async () => {
      const useShoppingBalance = shoppingBalanceAfterWithdraw / 2n;
      assert(useShoppingBalance > 0n, 'shopping balance should be large enough to spend');

      const shoppingBefore = await readShoppingBalance(ctx.api, entityId, bob.address);
      const nextOrderId = Number((await (ctx.api.query as any).entityTransaction.nextOrderId()).toString());

      const receipt = await submitTx(
        ctx.api,
        (ctx.api.tx as any).entityTransaction.placeOrder(
          productId,
          1,
          null,
          null,
          useShoppingBalance.toString(),
          null,
          null,
          null,
        ),
        bob,
        'bob spend shopping balance',
      );
      assertTxSuccess(receipt, 'shopping-balance order should succeed');
      assertEventIfPresent(receipt, 'entityLoyalty', 'ShoppingBalanceUsed', 'shopping balance spend should emit ShoppingBalanceUsed');
      assertEventIfPresent(receipt, 'entityTransaction', 'OrderCompleted', 'shopping-balance digital order should complete');

      const shoppingAfter = await readShoppingBalance(ctx.api, entityId, bob.address);
      const order = await readOrder(ctx.api, nextOrderId);
      const totalAmount = asBigInt(readObjectField(order.json, 'totalAmount', 'total_amount') ?? 0);

      assertEqual(shoppingAfter, shoppingBefore - useShoppingBalance, 'shopping balance should be deducted by the requested amount');
      assert(totalAmount < PRODUCT_PRICE, 'shopping balance should reduce the final order payment amount');
    });

    await ctx.step('claim pool reward and verify the pool reward round/accounting updates', async () => {
      const poolBefore = await readUnallocatedPool(ctx.api, entityId);
      assert(poolBefore > 0n, 'pool reward should have accumulated before claim');

      const receipt = await submitTx(
        ctx.api,
        (ctx.api.tx as any).commissionPoolReward.claimPoolReward(entityId),
        bob,
        'claim pool reward',
      );
      assertTxSuccess(receipt, 'pool reward claim should succeed');
      assertEventIfPresent(receipt, 'commissionPoolReward', 'PoolRewardClaimed', 'claim should emit PoolRewardClaimed');

      const lastClaimedRound = await readLastClaimedRound(ctx.api, entityId, bob.address);
      const poolAfter = await readUnallocatedPool(ctx.api, entityId);
      assert(lastClaimedRound > 0, 'claim should record a non-zero last claimed round');
      assert(poolAfter < poolBefore, 'claim should reduce the unallocated pool balance');

      ctx.note(`poolBeforeClaim=${formatNex(poolBefore)} poolAfterClaim=${formatNex(poolAfter)}`);
    });

    await ctx.step('final state snapshot confirms the composed business flow stayed healthy', async () => {
      const product = await readProduct(ctx.api, productId);
      const bobStats = await readCommissionStats(ctx.api, entityId, bob.address);
      const orderStats = codecToJson<Record<string, unknown>>(await (ctx.api.query as any).entityTransaction.orderStats());

      assert(
        decodeTextValue(readObjectField(product.json, 'nameCid', 'name_cid')) != null,
        'product metadata should remain readable',
      );
      assert(
        asBigInt(readObjectField(bobStats, 'repurchased') ?? 0) > 0n,
        'bob repurchased commission should remain recorded',
      );
      assert(
        asBigInt(readObjectField(orderStats, 'completedOrders', 'completed_orders') ?? 0) >= 5n,
        'completed order count should reflect the composed flow',
      );
    });
  },
};
