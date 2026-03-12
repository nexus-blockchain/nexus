import { ApiPromise } from '@polkadot/api';
import { submitTx } from '../framework/api.js';
import { assert, assertEvent, assertEqual, assertTxSuccess } from '../framework/assert.js';
import { codecToHuman, codecToJson, coerceNumber, decodeTextValue, readObjectField } from '../framework/codec.js';
import { TestSuite } from '../framework/types.js';

const encoder = new TextEncoder();

function bytes(value: string): Uint8Array {
  return encoder.encode(value);
}

async function readEntityIds(api: ApiPromise, address: string): Promise<number[]> {
  const value = await (api.query as any).entityRegistry.userEntities(address);
  const json = codecToJson<unknown[]>(value);
  return Array.isArray(json) ? json.map((item) => Number(item)) : [];
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

export const entityLifecycleSuite: TestSuite = {
  id: 'entity-lifecycle',
  title: 'Entity lifecycle smoke',
  description: 'Create an entity, verify the auto-created primary shop, then update metadata using the current 6-arg signature.',
  tags: ['entity', 'smoke'],
  async run(ctx) {
    const owner = ctx.actors.eve;
    const baseName = `smoke-entity-${Date.now()}`;
    const updatedName = `${baseName}-updated`;
    const contactCid = `contact-${Date.now()}`;

    await ctx.step('entity owner is funded', async () => {
      await ctx.ensureFunds(25_000);
    });

    const beforeEntityIds = await ctx.step('capture owner entity ids before create', async () => {
      return readEntityIds(ctx.api, owner.address);
    });

    await ctx.step('create entity with current runtime signature', async () => {
      const tx = (ctx.api.tx as any).entityRegistry.createEntity(bytes(baseName), null, null, null);
      const receipt = await submitTx(ctx.api, tx, owner, 'create entity');
      assertTxSuccess(receipt, 'create entity should succeed');
      assertEvent(receipt, 'entityRegistry', 'EntityCreated', 'entity create should emit EntityCreated');
    });

    const entityId = await ctx.step('entity is indexed and has a primary shop', async () => {
      const afterEntityIds = await readEntityIds(ctx.api, owner.address);
      const created = afterEntityIds.find((candidate) => !beforeEntityIds.includes(candidate));
      assert(created != null, 'expected a new entity id to be added to owner index');

      const entity = await readEntity(ctx.api, created);
      const primaryShopId = coerceNumber(readObjectField(entity.json, 'primaryShopId', 'primary_shop_id'));
      assert(primaryShopId != null && primaryShopId > 0, 'expected auto-created primary shop id');
      ctx.note(`entityId=${created} primaryShopId=${primaryShopId}`);
      return created;
    });

    await ctx.step('update entity metadata through the 6-arg updateEntity call', async () => {
      const tx = (ctx.api.tx as any).entityRegistry.updateEntity(
        entityId,
        bytes(updatedName),
        null,
        null,
        null,
        bytes(contactCid),
      );
      const receipt = await submitTx(ctx.api, tx, owner, 'update entity');
      assertTxSuccess(receipt, 'update entity should succeed');
      assertEvent(receipt, 'entityRegistry', 'EntityUpdated', 'entity update should emit EntityUpdated');
    });

    await ctx.step('entity storage reflects the updated owner metadata', async () => {
      const entity = await readEntity(ctx.api, entityId);
      const actualName = decodeTextValue(readObjectField(entity.json, 'name')) ?? String(readObjectField(entity.human, 'name'));
      const actualContact = decodeTextValue(readObjectField(entity.json, 'contactCid', 'contact_cid'))
        ?? String(readObjectField(entity.human, 'contactCid', 'contact_cid'));

      assertEqual(actualName, updatedName, 'entity name should be updated');
      assertEqual(actualContact, contactCid, 'entity contact cid should be updated');
    });
  },
};
