/**
 * Flow-E10: 订单治理/维护回归
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertEventEmitted,
  assertStorageExists,
  assertTxSuccess,
  assertTrue,
} from '../../core/assertions.js';
import { nex } from '../../core/config.js';

export const orderAdminFlow: FlowDef = {
  name: 'Flow-E10: 订单治理/维护',
  description: '服务确认 + 卖家退款 + 强制部分退款 + 争议撤回 + 索引清理 + 过期补偿',
  fn: runOrderAdminFlow,
};

const E10_STOP_AFTER = process.env.E10_STOP_AFTER ?? '';

async function runOrderAdminFlow(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const eve = ctx.actor('eve');
  const bob = ctx.actor('bob');

  traceE10('flow_start');
  const { entityId, shopId } = await ensureEntityAndShop(ctx, eve.address);
  await ctx.check('订单测试上下文就绪', 'eve', () => {
    console.log(`    Entity: ${entityId}, Shop: ${shopId}`);
  });
  if (await maybeStopAfter(ctx, 'after_context')) return;
  await ensureShopOperatingFund(ctx, shopId, nex(100).toString());
  if (await maybeStopAfter(ctx, 'after_fund_shop')) return;

  const serviceProductId = await createProduct(ctx, eve, shopId, 'Service', nex(5).toString());
  traceE10('service_product_ready', `productId=${serviceProductId}`);
  if (await maybeStopAfter(ctx, 'after_service_product')) return;
  const physicalProductId = await createProduct(ctx, eve, shopId, 'Physical', nex(7).toString());
  traceE10('physical_product_ready', `productId=${physicalProductId}`);
  if (await maybeStopAfter(ctx, 'after_physical_product')) return;

  traceE10('before_service_order', `productId=${serviceProductId}`);
  const serviceOrderId = await placeOrder(ctx, bob, serviceProductId, '服务单下单');
  traceE10('after_service_order', `orderId=${serviceOrderId}`);
  if (await maybeStopAfter(ctx, 'after_service_order')) return;
  traceE10('before_start_service', `orderId=${serviceOrderId}`);
  const startServiceResult = await ctx.send(
    (api.tx as any).entityTransaction.startService(serviceOrderId),
    eve,
    '服务单开始服务',
    'eve',
  );
  assertTxSuccess(startServiceResult, '开始服务');
  traceE10('after_start_service', `orderId=${serviceOrderId}`);
  if (await maybeStopAfter(ctx, 'after_start_service')) return;
  traceE10('before_complete_service', `orderId=${serviceOrderId}`);
  const completeServiceResult = await ctx.send(
    (api.tx as any).entityTransaction.completeService(serviceOrderId),
    eve,
    '服务单完成服务',
    'eve',
  );
  assertTxSuccess(completeServiceResult, '完成服务');
  traceE10('after_complete_service', `orderId=${serviceOrderId}`);
  if (await maybeStopAfter(ctx, 'after_complete_service')) return;
  traceE10('before_confirm_service', `orderId=${serviceOrderId}`);
  const confirmServiceResult = await ctx.send(
    (api.tx as any).entityTransaction.confirmService(serviceOrderId),
    bob,
    '买家确认服务完成',
    'bob',
  );
  assertTxSuccess(confirmServiceResult, '确认服务');
  await ctx.check('服务确认事件', 'bob', () => {
    assertEventEmitted(confirmServiceResult, 'entityTransaction', 'OrderCompleted', 'confirm_service');
  });
  traceE10('after_confirm_service', `orderId=${serviceOrderId}`);
  if (await maybeStopAfter(ctx, 'after_confirm_service')) return;

  traceE10('before_withdraw_dispute_order', `productId=${physicalProductId}`);
  const shippedOrderId = await placeOrder(
    ctx,
    bob,
    physicalProductId,
    '实物单下单(争议撤回)',
    `e10-shipping-${physicalProductId}-withdraw`,
  );
  traceE10('after_withdraw_dispute_order', `orderId=${shippedOrderId}`);
  if (await maybeStopAfter(ctx, 'after_withdraw_dispute_order')) return;
  traceE10('before_ship_withdraw_dispute_order', `orderId=${shippedOrderId}`);
  const shipResult = await ctx.send(
    (api.tx as any).entityTransaction.shipOrder(shippedOrderId, 'tracking_e10_a'),
    eve,
    '卖家发货 (争议撤回)',
    'eve',
  );
  assertTxSuccess(shipResult, '发货');
  traceE10('after_ship_withdraw_dispute_order', `orderId=${shippedOrderId}`);
  if (await maybeStopAfter(ctx, 'after_ship_withdraw_dispute_order')) return;
  traceE10('before_request_refund', `orderId=${shippedOrderId}`);
  const refundRequestResult = await ctx.send(
    (api.tx as any).entityTransaction.requestRefund(shippedOrderId, 'reason_withdraw_dispute'),
    bob,
    '买家申请退款(待撤回争议)',
    'bob',
  );
  assertTxSuccess(refundRequestResult, '申请退款');
  traceE10('after_request_refund', `orderId=${shippedOrderId}`);
  if (await maybeStopAfter(ctx, 'after_request_refund')) return;
  traceE10('before_withdraw_dispute', `orderId=${shippedOrderId}`);
  const withdrawDisputeResult = await ctx.send(
    (api.tx as any).entityTransaction.withdrawDispute(shippedOrderId),
    bob,
    '买家撤回争议',
    'bob',
  );
  assertTxSuccess(withdrawDisputeResult, '撤回争议');
  await ctx.check('撤回争议事件', 'bob', () => {
    assertEventEmitted(withdrawDisputeResult, 'entityTransaction', 'DisputeWithdrawn', 'withdraw_dispute');
  });
  traceE10('after_withdraw_dispute', `orderId=${shippedOrderId}`);
  if (await maybeStopAfter(ctx, 'after_withdraw_dispute')) return;

  traceE10('before_seller_refund_order', `productId=${physicalProductId}`);
  const sellerRefundOrderId = await placeOrder(
    ctx,
    bob,
    physicalProductId,
    '实物单下单(卖家退款)',
    `e10-shipping-${physicalProductId}-seller-refund`,
  );
  traceE10('after_seller_refund_order', `orderId=${sellerRefundOrderId}`);
  if (await maybeStopAfter(ctx, 'after_seller_refund_order')) return;
  traceE10('before_ship_seller_refund_order', `orderId=${sellerRefundOrderId}`);
  const shipRefundOrderResult = await ctx.send(
    (api.tx as any).entityTransaction.shipOrder(sellerRefundOrderId, 'tracking_e10_b'),
    eve,
    '卖家发货 (卖家退款)',
    'eve',
  );
  assertTxSuccess(shipRefundOrderResult, '发货');
  traceE10('after_ship_seller_refund_order', `orderId=${sellerRefundOrderId}`);
  if (await maybeStopAfter(ctx, 'after_ship_seller_refund_order')) return;
  traceE10('before_seller_refund', `orderId=${sellerRefundOrderId}`);
  const sellerRefundResult = await ctx.send(
    (api.tx as any).entityTransaction.sellerRefundOrder(sellerRefundOrderId, 'seller_refund_reason'),
    eve,
    '卖家主动退款',
    'eve',
  );
  assertTxSuccess(sellerRefundResult, '卖家退款');
  await ctx.check('卖家退款事件', 'eve', () => {
    assertEventEmitted(sellerRefundResult, 'entityTransaction', 'OrderSellerRefunded', 'seller_refund_order');
  });
  traceE10('after_seller_refund', `orderId=${sellerRefundOrderId}`);
  if (await maybeStopAfter(ctx, 'after_seller_refund')) return;

  traceE10('before_force_refund_order', `productId=${physicalProductId}`);
  const forceRefundOrderId = await placeOrder(
    ctx,
    bob,
    physicalProductId,
    '实物单下单(强制部分退款)',
    `e10-shipping-${physicalProductId}-force-refund`,
  );
  traceE10('after_force_refund_order', `orderId=${forceRefundOrderId}`);
  if (await maybeStopAfter(ctx, 'after_force_refund_order')) return;
  traceE10('before_force_partial_refund', `orderId=${forceRefundOrderId}`);
  const forcePartialRefundResult = await ctx.sudo(
    (api.tx as any).entityTransaction.forcePartialRefund(forceRefundOrderId, 5000, null),
    '管理员强制部分退款',
  );
  assertTxSuccess(forcePartialRefundResult, '强制部分退款');
  await ctx.check('强制部分退款事件', 'sudo(alice)', () => {
    assertEventEmitted(forcePartialRefundResult, 'entityTransaction', 'OrderPartialRefunded', 'force_partial_refund');
  });
  traceE10('after_force_partial_refund', `orderId=${forceRefundOrderId}`);
  if (await maybeStopAfter(ctx, 'after_force_partial_refund')) return;

  traceE10('before_cleanup_buyer_orders');
  const cleanupBuyerResult = await ctx.send(
    (api.tx as any).entityTransaction.cleanupBuyerOrders(),
    bob,
    '清理买家订单索引',
    'bob',
  );
  assertTxSuccess(cleanupBuyerResult, '清理买家订单索引');
  await ctx.check('买家索引清理事件', 'bob', () => {
    assertEventEmitted(cleanupBuyerResult, 'entityTransaction', 'BuyerOrdersCleaned', 'cleanup_buyer_orders');
  });
  traceE10('after_cleanup_buyer_orders');
  if (await maybeStopAfter(ctx, 'after_cleanup_buyer_orders')) return;

  traceE10('before_cleanup_shop_orders', `shopId=${shopId}`);
  const cleanupShopResult = await ctx.send(
    (api.tx as any).entityTransaction.cleanupShopOrders(shopId),
    eve,
    '清理店铺订单索引',
    'eve',
  );
  assertTxSuccess(cleanupShopResult, '清理店铺订单索引');
  await ctx.check('店铺索引清理事件', 'eve', () => {
    assertEventEmitted(cleanupShopResult, 'entityTransaction', 'ShopOrdersCleaned', 'cleanup_shop_orders');
  });
  traceE10('after_cleanup_shop_orders', `shopId=${shopId}`);
  if (await maybeStopAfter(ctx, 'after_cleanup_shop_orders')) return;

  const currentBlock = (await api.rpc.chain.getHeader()).number.toNumber();
  traceE10('before_force_process_expirations', `block=${currentBlock}`);
  const processExpirationsResult = await ctx.sudo(
    (api.tx as any).entityTransaction.forceProcessExpirations(currentBlock),
    '管理员补偿处理过期订单',
  );
  assertTxSuccess(processExpirationsResult, '处理过期订单');
  await ctx.check('过期订单补偿事件', 'sudo(alice)', () => {
    assertEventEmitted(processExpirationsResult, 'entityTransaction', 'StaleExpirationsProcessed', 'force_process_expirations');
  });
  traceE10('after_force_process_expirations', `block=${currentBlock}`);
  if (await maybeStopAfter(ctx, 'after_force_process_expirations')) return;

  await ctx.check('验证订单与商品状态仍可查询', 'system', async () => {
    await assertStorageExists(api, 'entityProduct', 'products', [serviceProductId], '服务商品存在');
    await assertStorageExists(api, 'entityProduct', 'products', [physicalProductId], '实物商品存在');
  });
  traceE10('flow_done');
}

function traceE10(marker: string, detail?: string): void {
  console.log(`    [E10] ${marker}${detail ? ` :: ${detail}` : ''}`);
}

async function maybeStopAfter(ctx: FlowContext, marker: string): Promise<boolean> {
  if (E10_STOP_AFTER !== marker) return false;
  await ctx.check(`E10 调试短路 @ ${marker}`, 'system', () => {
    console.log(`    [E10] short-circuit at ${marker}`);
  });
  return true;
}

async function ensureEntityAndShop(ctx: FlowContext, ownerAddress: string): Promise<{ entityId: number; shopId: number }> {
  const { api } = ctx;
  const userEntities = await (api.query as any).entityRegistry.userEntity(ownerAddress);
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

  const eve = ctx.actor('eve');
  const nextEntityId = (await (api.query as any).entityRegistry.nextEntityId()).toNumber();
  const createEntityResult = await ctx.send(
    (api.tx as any).entityRegistry.createEntity(
      `E10 Entity ${nextEntityId}`,
      null,
      `QmE10EntityDesc${nextEntityId}`,
      null,
    ),
    eve,
    '为 E10 创建最小实体/店铺上下文',
    'eve',
  );
  assertTxSuccess(createEntityResult, '创建 E10 Entity');

  const shopIdsRaw = await (api.query as any).entityRegistry.entityShops(nextEntityId);
  const shopIds = shopIdsRaw.toHuman() as string[];
  const shopId = parseInt(shopIds[0].replace(/,/g, ''), 10);
  return { entityId: nextEntityId, shopId };
}

async function ensureShopOperatingFund(
  ctx: FlowContext,
  shopId: number,
  amount: string,
): Promise<void> {
  const { api } = ctx;
  const eve = ctx.actor('eve');
  traceE10('before_fund_shop', `shopId=${shopId}`);
  const fundResult = await ctx.send(
    (api.tx as any).entityShop.fundOperating(shopId, amount),
    eve,
    `为店铺 #${shopId} 充值运营资金`,
    'eve',
  );
  if (!fundResult.success && fundResult.error?.includes('Priority is too low')) {
    await ctx.check('复用已有店铺运营资金', 'eve', () => {
      console.log(`    ℹ 店铺 #${shopId} 充值命中交易池重复，继续复用已有运营余额`);
    });
    traceE10('after_fund_shop_reused', `shopId=${shopId}`);
    return;
  }
  assertTxSuccess(fundResult, '充值店铺运营资金');
  traceE10('after_fund_shop', `shopId=${shopId}`);
}

async function createProduct(
  ctx: FlowContext,
  signer: any,
  shopId: number,
  category: 'Service' | 'Physical',
  price: string,
): Promise<number> {
  const { api } = ctx;
  const productId = (await (api.query as any).entityProduct.nextProductId()).toNumber();
  traceE10('before_create_product', `category=${category}, productId=${productId}`);
  const createResult = await ctx.send(
    (api.tx as any).entityProduct.createProduct(
      shopId,
      `E10-${category}-name-${productId}`,
      `E10-${category}-images-${productId}`,
      `E10-${category}-detail-${productId}`,
      price,
      0,
      100,
      category,
      0,
      '',
      '',
      1,
      0,
      'Public',
    ),
    signer,
    `创建${category}商品 #${productId}`,
    'eve',
  );
  assertTxSuccess(createResult, `创建${category}商品`);
  traceE10('after_create_product', `category=${category}, productId=${productId}`);

  traceE10('before_publish_product', `category=${category}, productId=${productId}`);
  const publishResult = await ctx.send(
    (api.tx as any).entityProduct.publishProduct(productId),
    signer,
    `上架${category}商品 #${productId}`,
    'eve',
  );
  assertTxSuccess(publishResult, `上架${category}商品`);
  traceE10('after_publish_product', `category=${category}, productId=${productId}`);
  return productId;
}

async function placeOrder(
  ctx: FlowContext,
  signer: any,
  productId: number,
  stepName: string,
  shippingCid: string | null = null,
): Promise<number> {
  const { api } = ctx;
  traceE10('before_place_order', `${stepName}, productId=${productId}`);
  const placeOrderResult = await ctx.send(
    (api.tx as any).entityTransaction.placeOrder(
      productId,
      1,
      shippingCid,
      null,
      null,
      null,
      null,
      null,
    ),
    signer,
    stepName,
    'bob',
  );
  assertTxSuccess(placeOrderResult, stepName);
  traceE10('after_place_order', `${stepName}, productId=${productId}`);
  const orderEvent = placeOrderResult.events.find(
    e => e.section === 'entityTransaction' && e.method === 'OrderCreated',
  );
  assertTrue(!!orderEvent, '应产生 OrderCreated');
  return Number(orderEvent?.data?.order_id ?? orderEvent?.data?.orderId ?? orderEvent?.data?.[0]);
}
