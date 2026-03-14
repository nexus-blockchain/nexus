import { submitTx } from '../framework/api.js';
import { assert, assertEvent, assertTxSuccess } from '../framework/assert.js';
import { codecToJson, readObjectField } from '../framework/codec.js';
import { TestSuite } from '../framework/types.js';
import { nex } from '../framework/units.js';
import {
  asOptionalNumber,
  bytes,
  createAndPublishProduct,
  decodeStatus,
  readMember,
  readNextOrderId,
  readOrder,
  setupFreshEntity,
  setupMembers,
} from './helpers.js';

async function readMemberUpgradeHistory(api: any, entityId: number, address: string): Promise<Array<Record<string, unknown>>> {
  const value = await api.query.entityMember.memberUpgradeHistory(entityId, address);
  const json = codecToJson<unknown[]>(value);
  return Array.isArray(json) ? json.filter((item): item is Record<string, unknown> => typeof item === 'object' && item != null) : [];
}

async function readLevelMemberCount(api: any, entityId: number, levelId: number): Promise<number> {
  return Number((await api.query.entityMember.levelMemberCount(entityId, levelId)).toString());
}

const DIGITAL_PRICE = nex(5);
const VIP2_RULE_THRESHOLD = 1;

export const phase1SpendDrivenLevelUpgradeSuite: TestSuite = {
  id: 'phase1-spend-driven-level-upgrade',
  title: 'Phase 1 / S1-05 spend-driven level upgrade',
  description: 'Initialize level + upgrade-rule systems, then verify order spending upgrades the buyer from level 0 to a rule-driven higher level.',
  tags: ['phase1', 'member', 'upgrade', 'entity'],
  async run(ctx) {
    const seller = ctx.actors.ferdie;
    const buyer = ctx.actors.bob;
    const tx = ctx.api.tx as any;

    await ctx.step('fund seller and buyer accounts', async () => {
      await ctx.ensureFundsFor(['ferdie', 'bob'], 25_000);
    });

    const setup = await ctx.step('create a fresh entity, activate buyer membership, and publish a digital product', async () => {
      const { entityId, shopId } = await setupFreshEntity(ctx.api, seller, nex(2_500));
      await setupMembers(ctx.api, seller, shopId, entityId, [buyer]);
      const productId = await createAndPublishProduct(ctx.api, seller, shopId, {
        price: DIGITAL_PRICE,
        category: 'Digital',
      });
      const memberBefore = await readMember(ctx.api, entityId, buyer.address);
      ctx.note(`entityId=${entityId} shopId=${shopId} productId=${productId}`);
      return { entityId, shopId, productId, memberBefore };
    });

    await ctx.step('initialize level and upgrade-rule systems for spend-driven promotion', async () => {
      const initLevelReceipt = await submitTx(
        ctx.api,
        tx.entityMember.initLevelSystem(setup.shopId, true, 'AutoUpgrade'),
        seller,
        'init level system',
      );
      assertTxSuccess(initLevelReceipt, 'initLevelSystem should succeed');
      assertEvent(initLevelReceipt, 'entityMember', 'LevelSystemInitialized', 'initLevelSystem should emit LevelSystemInitialized');

      const addVip1Receipt = await submitTx(
        ctx.api,
        tx.entityMember.addCustomLevel(setup.shopId, bytes('VIP1'), 1, 0, 0),
        seller,
        'add VIP1 level',
      );
      assertTxSuccess(addVip1Receipt, 'first addCustomLevel should succeed');
      assertEvent(addVip1Receipt, 'entityMember', 'CustomLevelAdded', 'addCustomLevel should emit CustomLevelAdded for VIP1');

      const addVip2Receipt = await submitTx(
        ctx.api,
        tx.entityMember.addCustomLevel(setup.shopId, bytes('VIP2'), 1_000_000_000, 0, 0),
        seller,
        'add VIP2 level',
      );
      assertTxSuccess(addVip2Receipt, 'second addCustomLevel should succeed');
      assertEvent(addVip2Receipt, 'entityMember', 'CustomLevelAdded', 'addCustomLevel should emit CustomLevelAdded for VIP2');

      const initRulesReceipt = await submitTx(
        ctx.api,
        tx.entityMember.initUpgradeRuleSystem(setup.shopId, 'HighestLevel'),
        seller,
        'init upgrade rule system',
      );
      assertTxSuccess(initRulesReceipt, 'initUpgradeRuleSystem should succeed');
      assertEvent(initRulesReceipt, 'entityMember', 'UpgradeRuleSystemInitialized', 'initUpgradeRuleSystem should emit UpgradeRuleSystemInitialized');

      const addRuleReceipt = await submitTx(
        ctx.api,
        tx.entityMember.addUpgradeRule(
          setup.shopId,
          bytes('SpendToVip2'),
          { TotalSpent: { threshold: VIP2_RULE_THRESHOLD } },
          2,
          null,
          1,
          false,
          null,
        ),
        seller,
        'add total-spent upgrade rule',
      );
      assertTxSuccess(addRuleReceipt, 'addUpgradeRule should succeed');
      assertEvent(addRuleReceipt, 'entityMember', 'UpgradeRuleAdded', 'addUpgradeRule should emit UpgradeRuleAdded');
    });

    await ctx.step('a paid order upgrades the buyer to the rule target level', async () => {
      const nextOrderId = await readNextOrderId(ctx.api);
      const receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.placeOrder(
          setup.productId,
          1,
          null,
          null,
          null,
          null,
          null,
          null,
        ),
        buyer,
        'place digital order for level-up',
      );
      assertTxSuccess(receipt, 'placeOrder should succeed for level-up scenario');
      assertEvent(receipt, 'entityTransaction', 'OrderCompleted', 'digital order should auto-complete');
      assertEvent(receipt, 'entityMember', 'MemberUpgradedByRule', 'order completion should trigger rule-driven level upgrade');

      const order = await readOrder(ctx.api, nextOrderId);
      const memberAfter = await readMember(ctx.api, setup.entityId, buyer.address);
      const upgradeHistory = await readMemberUpgradeHistory(ctx.api, setup.entityId, buyer.address);
      const level2Count = await readLevelMemberCount(ctx.api, setup.entityId, 2);

      const orderStatus = decodeStatus(order, 'status').toLowerCase();
      const spentBefore = asOptionalNumber(readObjectField(setup.memberBefore.json, 'totalSpent', 'total_spent')) ?? 0;
      const spentAfter = asOptionalNumber(readObjectField(memberAfter.json, 'totalSpent', 'total_spent')) ?? 0;
      const finalLevel = asOptionalNumber(readObjectField(memberAfter.json, 'customLevelId', 'custom_level_id')) ?? 0;

      assert(orderStatus.includes('completed'), `digital order should complete immediately, got ${orderStatus}`);
      assert(spentAfter > spentBefore, `member total_spent should increase (${spentBefore} -> ${spentAfter})`);
      assert(finalLevel === 2, `member should be upgraded to level 2 by the spend rule, got ${finalLevel}`);
      assert(upgradeHistory.length > 0, 'member upgrade history should record the applied spend rule');
      assert(level2Count >= 1, `level 2 member count should include the buyer, got ${level2Count}`);

      ctx.note(`orderId=${nextOrderId} spentAfter=${spentAfter} finalLevel=${finalLevel} upgradeHistory=${upgradeHistory.length}`);
    });
  },
};
