/**
 * Flow-D3: 仲裁申诉回归
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxFailed,
  assertTxSuccess,
  assertTrue,
} from '../../core/assertions.js';
import { nex } from '../../core/config.js';

export const arbitrationAppealFlow: FlowDef = {
  name: 'Flow-D3: 仲裁申诉',
  description: '实体订单投诉 -> 升级仲裁 -> 校验仲裁委员会权限边界',
  fn: runArbitrationAppealFlow,
};

async function runArbitrationAppealFlow(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const eve = ctx.actor('eve');

  const domain = '0x656e746f72646572'; // "entorder"
  const { shopId } = await ensureEntityAndShop(ctx);
  await ensureShopOperatingFund(ctx, shopId, nex(100).toString());
  const productId = await createProduct(ctx, shopId);
  const orderId = await placeOrder(ctx, productId);

  const complaintResult = await ctx.send(
    (api.tx as any).arbitration.fileComplaint(
      domain,
      orderId,
      'EntityOrderNotDeliver',
      'QmD3ComplaintCid',
      null,
    ),
    bob,
    'Bob 发起投诉 (D3)',
    'bob',
  );
  assertTxSuccess(complaintResult, '发起投诉');

  const complaintEvent = complaintResult.events.find(
    e => e.section === 'arbitration' && e.method === 'ComplaintFiled',
  );
  assertTrue(!!complaintEvent, '应产生 ComplaintFiled');
  const complaintId = Number(
    complaintEvent?.data?.complaint_id ?? complaintEvent?.data?.complaintId ?? complaintEvent?.data?.[0],
  );

  const respondResult = await ctx.send(
    (api.tx as any).arbitration.respondToComplaint(complaintId, 'QmD3ResponseCid'),
    eve,
    'Eve 响应投诉 (D3)',
    'eve',
  );
  assertTxSuccess(respondResult, '响应投诉');

  const escalateResult = await ctx.send(
    (api.tx as any).arbitration.escalateToArbitration(complaintId),
    bob,
    'Bob 升级到仲裁 (D3)',
    'bob',
  );
  assertTxSuccess(escalateResult, '升级仲裁');

  const resolveComplaintResult = await ctx.sudo(
    (api.tx as any).arbitration.resolveComplaint(complaintId, 0, 'QmD3ResolutionCid', null),
    '[错误路径] Root 尝试仲裁裁决',
  );
  await ctx.check('Root 无法替代仲裁委员会裁决', 'sudo(alice)', () => {
    assertTxFailed(resolveComplaintResult, 'BadOrigin', 'resolve_complaint');
  });

  const appealResult = await ctx.send(
    (api.tx as any).arbitration.appeal(complaintId, 'QmD3AppealCid'),
    eve,
    '[错误路径] 未裁决前直接申诉',
    'eve',
  );
  await ctx.check('未裁决前申诉失败', 'eve', () => {
    assertTxFailed(appealResult, 'CannotAppeal', 'appeal');
  });
}

async function ensureEntityAndShop(ctx: FlowContext): Promise<{ entityId: number; shopId: number }> {
  const { api } = ctx;
  const eve = ctx.actor('eve');
  const userEntities = await (api.query as any).entityRegistry.userEntity(eve.address);
  const entityIds = userEntities.toHuman() as string[];

  if (entityIds && entityIds.length > 0) {
    for (const rawEntityId of entityIds) {
      const entityId = parseInt(rawEntityId.replace(/,/g, ''), 10);
      const shopIdsRaw = await (api.query as any).entityRegistry.entityShops(entityId);
      const shopIds = shopIdsRaw.toHuman() as string[];
      if (shopIds && shopIds.length > 0) {
        return {
          entityId,
          shopId: parseInt(shopIds[0].replace(/,/g, ''), 10),
        };
      }
    }
  }

  const nextEntityId = (await (api.query as any).entityRegistry.nextEntityId()).toNumber();
  const createEntityResult = await ctx.send(
    (api.tx as any).entityRegistry.createEntity(
      `D3 Entity ${nextEntityId}`,
      null,
      `QmD3Entity${nextEntityId}`,
      null,
    ),
    eve,
    '为 D3 创建最小实体/店铺上下文',
    'eve',
  );
  assertTxSuccess(createEntityResult, '创建 D3 Entity');

  const shopIdsRaw = await (api.query as any).entityRegistry.entityShops(nextEntityId);
  const shopIds = shopIdsRaw.toHuman() as string[];
  return {
    entityId: nextEntityId,
    shopId: parseInt(shopIds[0].replace(/,/g, ''), 10),
  };
}

async function ensureShopOperatingFund(
  ctx: FlowContext,
  shopId: number,
  amount: string,
): Promise<void> {
  const { api } = ctx;
  const eve = ctx.actor('eve');
  const fundResult = await ctx.send(
    (api.tx as any).entityShop.fundOperating(shopId, amount),
    eve,
    `为 D3 店铺 #${shopId} 充值运营资金`,
    'eve',
  );
  if (!fundResult.success && fundResult.error?.includes('Priority is too low')) {
    await ctx.check('复用已有 D3 店铺运营资金', 'eve', () => {
      console.log(`    ℹ D3 店铺 #${shopId} 充值命中交易池重复，继续复用已有运营余额`);
    });
    return;
  }
  assertTxSuccess(fundResult, '充值 D3 店铺运营资金');
}

async function createProduct(ctx: FlowContext, shopId: number): Promise<number> {
  const { api } = ctx;
  const eve = ctx.actor('eve');
  const productId = (await (api.query as any).entityProduct.nextProductId()).toNumber();
  const createResult = await ctx.send(
    (api.tx as any).entityProduct.createProduct(
      shopId,
      `D3 Product ${productId}`,
      `D3-images-${productId}`,
      `D3-detail-${productId}`,
      nex(3).toString(),
      0,
      10,
      'Physical',
      0,
      '',
      '',
      1,
      0,
      'Public',
    ),
    eve,
    `创建 D3 商品 #${productId}`,
    'eve',
  );
  assertTxSuccess(createResult, '创建 D3 商品');

  const publishResult = await ctx.send(
    (api.tx as any).entityProduct.publishProduct(productId),
    eve,
    `上架 D3 商品 #${productId}`,
    'eve',
  );
  assertTxSuccess(publishResult, '上架 D3 商品');
  return productId;
}

async function placeOrder(ctx: FlowContext, productId: number): Promise<number> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const orderResult = await ctx.send(
    (api.tx as any).entityTransaction.placeOrder(
      productId,
      1,
      `d3-shipping-${productId}`,
      null,
      null,
      null,
      null,
      null,
    ),
    bob,
    'Bob 创建 D3 订单',
    'bob',
  );
  assertTxSuccess(orderResult, '创建 D3 订单');
  const orderEvent = orderResult.events.find(
    e => e.section === 'entityTransaction' && e.method === 'OrderCreated',
  );
  assertTrue(!!orderEvent, '应产生 OrderCreated');
  return Number(orderEvent?.data?.order_id ?? orderEvent?.data?.orderId ?? orderEvent?.data?.[0]);
}
