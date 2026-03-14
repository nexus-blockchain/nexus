import { bnToU8a, stringToU8a, u8aConcat, u8aToHex } from '@polkadot/util';
import type { ApiPromise } from '@polkadot/api';
import { submitTx } from '../framework/api.js';
import { assert, assertEvent, assertTxSuccess } from '../framework/assert.js';
import { codecToJson, readObjectField } from '../framework/codec.js';
import { TestSuite } from '../framework/types.js';
import {
  asBigInt,
  bytes,
  createAndPublishProduct,
  readNextOrderId,
  readOrder,
  setupFreshEntity,
  setupMembers,
} from './helpers.js';

const COMMISSION_MASK =
  0b0000_0010 + // MULTI_LEVEL
  0b1000_0000 + // SINGLE_LINE_UPLINE
  0b1_0000_0000 + // SINGLE_LINE_DOWNLINE
  0b10_0000_0000; // POOL_REWARD

const TOKEN_PRODUCT_PRICE = 100_000n;
const PARTICIPANT_TOKEN_MINT = 1_000_000n;
const ENTITY_RESERVE_TOKEN_MINT = 2_000_000n;
const ENTITY_PALLET_ID = 'et/enty/';
const SUB_ACCOUNT_TYPE_ID = 'modl';
const TOKEN_ASSET_OFFSET = 1_000_000;

type CandidateSnapshot = {
  name: 'bob' | 'dave';
  actor: any;
  before: Record<string, unknown>;
  after?: Record<string, unknown>;
  delta?: bigint;
};

function deriveEntityAccountHex(entityId: number): string {
  const seed = u8aConcat(
    stringToU8a(SUB_ACCOUNT_TYPE_ID),
    stringToU8a(ENTITY_PALLET_ID),
    bnToU8a(BigInt(entityId), { bitLength: 64, isLe: true }),
  );
  return u8aToHex(u8aConcat(seed, new Uint8Array(32 - seed.length)));
}

async function readAssetBalance(api: ApiPromise, assetId: number, address: string): Promise<bigint> {
  const value = await (api.query as any).assets.account(assetId, address);
  if ((value as any)?.isSome) {
    const json = codecToJson((value as any).unwrap());
    return asBigInt(readObjectField(json, 'balance') ?? 0);
  }
  const json = codecToJson(value);
  if (json == null) {
    return 0n;
  }
  return asBigInt(readObjectField(json, 'balance') ?? 0);
}

async function readTokenCommissionStats(api: ApiPromise, entityId: number, address: string): Promise<Record<string, unknown>> {
  return codecToJson(await (api.query as any).commissionCore.memberTokenCommissionStats(entityId, address));
}

async function readTokenShoppingBalance(api: ApiPromise, entityId: number, address: string): Promise<bigint> {
  return asBigInt(await (api.query as any).entityLoyalty.memberTokenShoppingBalance(entityId, address));
}

function pendingOf(stats: Record<string, unknown>): bigint {
  return asBigInt(readObjectField(stats, 'pending') ?? 0);
}

function withdrawnOf(stats: Record<string, unknown>): bigint {
  return asBigInt(readObjectField(stats, 'withdrawn') ?? 0);
}

function repurchasedOf(stats: Record<string, unknown>): bigint {
  return asBigInt(readObjectField(stats, 'repurchased') ?? 0);
}

export const phase1TokenPaymentTokenCommissionSuite: TestSuite = {
  id: 'phase1-token-payment-token-commission',
  title: 'Phase 1 / S1-03 token payment + token commission',
  description: 'Verify EntityToken-paid orders generate token commission and withdrawTokenCommission splits into wallet tokens plus token shopping balance.',
  tags: ['phase1', 'token', 'commission', 'entity'],
  async run(ctx) {
    const seller = ctx.actors.ferdie;
    const bob = ctx.actors.bob;
    const charlie = ctx.actors.charlie;
    const dave = ctx.actors.dave;
    const tx = ctx.api.tx as any;

    await ctx.step('fund seller and member accounts', async () => {
      await ctx.ensureFundsFor(['ferdie', 'bob', 'charlie', 'dave'], 25_000);
    });

    const setup = await ctx.step('create entity, activate referral-chain members, publish a digital product, and bootstrap entity tokens', async () => {
      const { entityId, shopId } = await setupFreshEntity(ctx.api, seller);
      await setupMembers(ctx.api, seller, shopId, entityId, [bob, charlie, dave], true);

      const productId = await createAndPublishProduct(ctx.api, seller, shopId, {
        price: TOKEN_PRODUCT_PRICE,
        category: 'Digital',
      });

      const entityAccount = deriveEntityAccountHex(entityId);
      const assetId = TOKEN_ASSET_OFFSET + entityId;

      let receipt = await submitTx(
        ctx.api,
        tx.entityToken.createShopToken(
          entityId,
          bytes(`entity-token-${entityId}`),
          bytes(`ET${entityId}`),
          0,
          0,
          0,
        ),
        seller,
        'create entity token',
      );
      assertTxSuccess(receipt, 'createShopToken should succeed');
      assertEvent(receipt, 'entityToken', 'EntityTokenCreated', 'createShopToken should emit EntityTokenCreated');

      for (const [to, amount, label] of [
        [entityAccount, ENTITY_RESERVE_TOKEN_MINT, 'mint entity reserve tokens'],
        [bob.address, PARTICIPANT_TOKEN_MINT, 'mint bob tokens'],
        [charlie.address, PARTICIPANT_TOKEN_MINT, 'mint charlie tokens'],
        [dave.address, PARTICIPANT_TOKEN_MINT, 'mint dave tokens'],
      ] as const) {
        receipt = await submitTx(
          ctx.api,
          tx.entityToken.mintTokens(entityId, to, amount.toString()),
          seller,
          label,
        );
        assertTxSuccess(receipt, `${label} should succeed`);
        assertEvent(receipt, 'entityToken', 'TokensMinted', `${label} should emit TokensMinted`);
      }

      ctx.note(`entityId=${entityId} shopId=${shopId} productId=${productId} assetId=${assetId}`);
      return { entityId, shopId, productId, entityAccount, assetId };
    });

    await ctx.step('configure commission core, token withdrawal, and shared single-line + multi-level plugins', async () => {
      const fixedWithdrawalMode = { FixedRate: { repurchase_rate: 5000 } };
      const defaultTier = { withdrawal_rate: 10000, repurchase_rate: 0 };

      let receipt = await submitTx(
        ctx.api,
        tx.commissionCore.setCommissionRate(setup.entityId, 2000),
        seller,
        'set commission rate',
      );
      assertTxSuccess(receipt, 'setCommissionRate should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.commissionCore.setCommissionModes(setup.entityId, COMMISSION_MASK),
        seller,
        'set commission modes',
      );
      assertTxSuccess(receipt, 'setCommissionModes should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.commissionCore.enableCommission(setup.entityId, true),
        seller,
        'enable commission',
      );
      assertTxSuccess(receipt, 'enableCommission should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.commissionCore.setTokenWithdrawalConfig(
          setup.entityId,
          fixedWithdrawalMode,
          defaultTier,
          [],
          0,
          true,
        ),
        seller,
        'set token withdrawal config',
      );
      assertTxSuccess(receipt, 'setTokenWithdrawalConfig should succeed');
      assertEvent(receipt, 'commissionCore', 'TokenWithdrawalConfigUpdated', 'setTokenWithdrawalConfig should emit TokenWithdrawalConfigUpdated');

      receipt = await submitTx(
        ctx.api,
        tx.commissionSingleLine.setSingleLineConfig(
          setup.entityId,
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
      assertTxSuccess(receipt, 'setSingleLineConfig should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.commissionMultiLevel.setMultiLevelConfig(
          setup.entityId,
          [
            { rate: 200, required_directs: 0, required_team_size: 0, required_spent: 0 },
            { rate: 100, required_directs: 0, required_team_size: 0, required_spent: 0 },
          ],
          300,
        ),
        seller,
        'set multi-level config',
      );
      assertTxSuccess(receipt, 'setMultiLevelConfig should succeed');

      receipt = await submitTx(
        ctx.api,
        tx.commissionPoolReward.setPoolRewardConfig(
          setup.entityId,
          [[0, 10_000]],
          14_400,
        ),
        seller,
        'set pool reward config',
      );
      assertTxSuccess(receipt, 'setPoolRewardConfig should succeed');
    });

    await ctx.step('seed single-line membership order positions with token-paid orders', async () => {
      for (const actor of [bob, charlie, dave]) {
        const receipt = await submitTx(
          ctx.api,
          tx.entityTransaction.placeOrder(
            setup.productId,
            1,
            null,
            null,
            null,
            'EntityToken',
            null,
            null,
          ),
          actor,
          `seed token order for ${actor.meta.name ?? actor.address}`,
        );
        assertTxSuccess(receipt, 'seed token order should succeed');
        assertEvent(receipt, 'entityTransaction', 'OrderCompleted', 'seed digital token order should auto-complete');
      }
    });

    const commissionState = await ctx.step('a second Charlie token order generates fresh token commission for at least one beneficiary', async () => {
      const candidates: CandidateSnapshot[] = [
        { name: 'bob', actor: bob, before: await readTokenCommissionStats(ctx.api, setup.entityId, bob.address) },
        { name: 'dave', actor: dave, before: await readTokenCommissionStats(ctx.api, setup.entityId, dave.address) },
      ];

      const nextOrderId = await readNextOrderId(ctx.api);
      const receipt = await submitTx(
        ctx.api,
        tx.entityTransaction.placeOrder(
          setup.productId,
          1,
          null,
          null,
          null,
          'EntityToken',
          null,
          null,
        ),
        charlie,
        'charlie token commission order',
      );
      assertTxSuccess(receipt, 'second Charlie token order should succeed');
      assertEvent(receipt, 'entityTransaction', 'OrderCompleted', 'second Charlie token order should auto-complete');
      assertEvent(receipt, 'commissionCore', 'OrderRecordsSettled', 'token order should settle order commission records');

      const order = await readOrder(ctx.api, nextOrderId);
      assert(asBigInt(readObjectField(order.json, 'tokenPaymentAmount', 'token_payment_amount') ?? 0) > 0n, 'token-paid order should store token_payment_amount');

      for (const candidate of candidates) {
        const after = await readTokenCommissionStats(ctx.api, setup.entityId, candidate.actor.address);
        candidate.after = after;
        candidate.delta = pendingOf(after) - pendingOf(candidate.before);
      }

      const beneficiary = candidates.find((candidate) => (candidate.delta ?? 0n) > 0n);
      assert(beneficiary != null, 'expected at least one beneficiary pending token commission to increase after Charlie token order');
      assertEvent(receipt, 'commissionCore', 'TokenCommissionDistributed', 'token order should emit TokenCommissionDistributed');
      ctx.note(`beneficiary=${beneficiary.name} pendingDelta=${beneficiary.delta}`);
      return { beneficiary, orderId: nextOrderId };
    });

    await ctx.step('the selected beneficiary withdraws token commission into wallet tokens plus token shopping balance', async () => {
      const { beneficiary } = commissionState;
      const beforePending = pendingOf(beneficiary.after!);
      assert(beforePending > 0n, 'selected beneficiary should have pending token commission before withdraw');

      const beforeWithdrawn = withdrawnOf(beneficiary.after!);
      const beforeRepurchased = repurchasedOf(beneficiary.after!);
      const beforeWallet = await readAssetBalance(ctx.api, setup.assetId, beneficiary.actor.address);
      const beforeShopping = await readTokenShoppingBalance(ctx.api, setup.entityId, beneficiary.actor.address);

      const receipt = await submitTx(
        ctx.api,
        tx.commissionCore.withdrawTokenCommission(setup.entityId, null, null, null),
        beneficiary.actor,
        `withdraw token commission for ${beneficiary.name}`,
      );
      assertTxSuccess(receipt, 'withdrawTokenCommission should succeed');
      assertEvent(receipt, 'commissionCore', 'TokenTieredWithdrawal', 'withdrawTokenCommission should emit TokenTieredWithdrawal');
      assertEvent(receipt, 'entityLoyalty', 'TokenShoppingBalanceCredited', 'withdrawTokenCommission should credit token shopping balance');

      const afterStats = await readTokenCommissionStats(ctx.api, setup.entityId, beneficiary.actor.address);
      const afterWallet = await readAssetBalance(ctx.api, setup.assetId, beneficiary.actor.address);
      const afterShopping = await readTokenShoppingBalance(ctx.api, setup.entityId, beneficiary.actor.address);

      const afterPending = pendingOf(afterStats);
      const afterWithdrawn = withdrawnOf(afterStats);
      const afterRepurchased = repurchasedOf(afterStats);

      assert(afterPending < beforePending, `pending token commission should decrease (${beforePending} -> ${afterPending})`);
      assert(afterWithdrawn > beforeWithdrawn, `withdrawn token commission should increase (${beforeWithdrawn} -> ${afterWithdrawn})`);
      assert(afterRepurchased > beforeRepurchased, `repurchased token commission should increase (${beforeRepurchased} -> ${afterRepurchased})`);
      assert(afterWallet > beforeWallet, `wallet token balance should increase after withdraw (${beforeWallet} -> ${afterWallet})`);
      assert(afterShopping > beforeShopping, `token shopping balance should increase after withdraw (${beforeShopping} -> ${afterShopping})`);

      ctx.note(
        `beneficiary=${beneficiary.name} orderId=${commissionState.orderId} wallet=${afterWallet} tokenShopping=${afterShopping}`,
      );
    });
  },
};
