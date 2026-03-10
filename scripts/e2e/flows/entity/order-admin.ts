/**
 * Flow-E10: 订单治理/维护回归
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertStorageExists,
  assertTxSuccess,
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
  const startServiceResult = await sendE10(
    ctx,
    (api.tx as any).entityTransaction.startService(serviceOrderId),
    eve,
    '服务单开始服务',
    'eve',
  );
  assertTxSuccess(startServiceResult, '开始服务');
  traceE10('after_start_service', `orderId=${serviceOrderId}`);
  if (await maybeStopAfter(ctx, 'after_start_service')) return;
  traceE10('before_complete_service', `orderId=${serviceOrderId}`);
  await logOrderStateSnapshot(ctx, serviceOrderId, 'before_complete_service');
  const completeServiceResult = await sendE10(
    ctx,
    (api.tx as any).entityTransaction.completeService(serviceOrderId),
    eve,
    '服务单完成服务',
    'eve',
  );
  assertTxSuccess(completeServiceResult, '完成服务');
  await logOrderStateSnapshot(ctx, serviceOrderId, 'after_complete_service');
  traceE10('after_complete_service', `orderId=${serviceOrderId}`);
  if (await maybeStopAfter(ctx, 'after_complete_service')) return;
  traceE10('before_confirm_service', `orderId=${serviceOrderId}`);
  await logOrderStateSnapshot(ctx, serviceOrderId, 'before_confirm_service');
  const confirmServiceResult = await sendE10(
    ctx,
    (api.tx as any).entityTransaction.confirmService(serviceOrderId),
    bob,
    '买家确认服务完成',
    'bob',
  );
  assertTxSuccess(confirmServiceResult, '确认服务');
  await ctx.check('服务确认状态已落库', 'bob', async () => {
    const order = await getOrderHuman(ctx, serviceOrderId);
    if (readOrderStatus(order) !== 'Completed') {
      throw new Error(`服务单状态未更新为 Completed: actual=${readOrderStatus(order)}`);
    }
    if (!order?.completedAt) {
      throw new Error('服务单 completedAt 未写入');
    }
  });
  await ctx.check('服务确认触发自动会员注册', 'bob', () => {
    return assertMemberActivated(ctx, entityId, bob.address);
  });
  await logOrderStateSnapshot(ctx, serviceOrderId, 'after_confirm_service');
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
  const shipResult = await sendE10(
    ctx,
    (api.tx as any).entityTransaction.shipOrder(shippedOrderId, 'tracking_e10_a'),
    eve,
    '卖家发货 (争议撤回)',
    'eve',
  );
  assertTxSuccess(shipResult, '发货');
  traceE10('after_ship_withdraw_dispute_order', `orderId=${shippedOrderId}`);
  if (await maybeStopAfter(ctx, 'after_ship_withdraw_dispute_order')) return;
  traceE10('before_request_refund', `orderId=${shippedOrderId}`);
  const refundRequestResult = await sendE10(
    ctx,
    (api.tx as any).entityTransaction.requestRefund(shippedOrderId, 'reason_withdraw_dispute'),
    bob,
    '买家申请退款(待撤回争议)',
    'bob',
  );
  assertTxSuccess(refundRequestResult, '申请退款');
  traceE10('after_request_refund', `orderId=${shippedOrderId}`);
  if (await maybeStopAfter(ctx, 'after_request_refund')) return;
  traceE10('before_withdraw_dispute', `orderId=${shippedOrderId}`);
  const withdrawDisputeResult = await sendE10(
    ctx,
    (api.tx as any).entityTransaction.withdrawDispute(shippedOrderId),
    bob,
    '买家撤回争议',
    'bob',
  );
  assertTxSuccess(withdrawDisputeResult, '撤回争议');
  await ctx.check('撤回争议后订单已恢复', 'bob', async () => {
    const order = await getOrderHuman(ctx, shippedOrderId);
    if (readOrderStatus(order) !== 'Shipped') {
      throw new Error(`撤回争议后订单状态异常: actual=${readOrderStatus(order)}`);
    }
    if (order?.disputeDeadline != null || order?.refundReasonCid != null) {
      throw new Error(`撤回争议后争议字段未清理: ${JSON.stringify(order)}`);
    }
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
  const shipRefundOrderResult = await sendE10(
    ctx,
    (api.tx as any).entityTransaction.shipOrder(sellerRefundOrderId, 'tracking_e10_b'),
    eve,
    '卖家发货 (卖家退款)',
    'eve',
  );
  assertTxSuccess(shipRefundOrderResult, '发货');
  traceE10('after_ship_seller_refund_order', `orderId=${sellerRefundOrderId}`);
  if (await maybeStopAfter(ctx, 'after_ship_seller_refund_order')) return;
  traceE10('before_seller_refund', `orderId=${sellerRefundOrderId}`);
  const sellerRefundResult = await sendE10(
    ctx,
    (api.tx as any).entityTransaction.sellerRefundOrder(sellerRefundOrderId, 'seller_refund_reason'),
    eve,
    '卖家主动退款',
    'eve',
  );
  assertTxSuccess(sellerRefundResult, '卖家退款');
  await ctx.check('卖家退款已生效', 'eve', async () => {
    const order = await getOrderHuman(ctx, sellerRefundOrderId);
    if (readOrderStatus(order) !== 'Refunded') {
      throw new Error(`卖家退款后订单状态异常: actual=${readOrderStatus(order)}`);
    }
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
  const forcePartialRefundResult = await sudoE10(
    ctx,
    (api.tx as any).entityTransaction.forcePartialRefund(forceRefundOrderId, 5000, null),
    '管理员强制部分退款',
  );
  assertTxSuccess(forcePartialRefundResult, '强制部分退款');
  await ctx.check('强制部分退款已生效', 'sudo(alice)', async () => {
    const order = await getOrderHuman(ctx, forceRefundOrderId);
    if (readOrderStatus(order) !== 'Refunded') {
      throw new Error(`强制部分退款后订单状态异常: actual=${readOrderStatus(order)}`);
    }
  });
  traceE10('after_force_partial_refund', `orderId=${forceRefundOrderId}`);
  if (await maybeStopAfter(ctx, 'after_force_partial_refund')) return;

  traceE10('before_cleanup_buyer_orders');
  const cleanupBuyerResult = await sendE10(
    ctx,
    (api.tx as any).entityTransaction.cleanupBuyerOrders(),
    bob,
    '清理买家订单索引',
    'bob',
  );
  assertTxSuccess(cleanupBuyerResult, '清理买家订单索引');
  await ctx.check('买家终态订单索引已清理', 'bob', async () => {
    const buyerOrderIds = await getOrderIdList((api.query as any).entityTransaction.buyerOrders(bob.address));
    const terminalIds = [serviceOrderId, sellerRefundOrderId, forceRefundOrderId].map(String);
    for (const terminalId of terminalIds) {
      if (buyerOrderIds.includes(terminalId)) {
        throw new Error(`买家终态订单仍存在于索引中: orderId=${terminalId}`);
      }
    }
    if (!buyerOrderIds.includes(String(shippedOrderId))) {
      throw new Error(`非终态订单被错误清理: orderId=${shippedOrderId}`);
    }
  });
  traceE10('after_cleanup_buyer_orders');
  if (await maybeStopAfter(ctx, 'after_cleanup_buyer_orders')) return;

  traceE10('before_cleanup_shop_orders', `shopId=${shopId}`);
  const cleanupShopResult = await sendE10(
    ctx,
    (api.tx as any).entityTransaction.cleanupShopOrders(shopId),
    eve,
    '清理店铺订单索引',
    'eve',
  );
  assertTxSuccess(cleanupShopResult, '清理店铺订单索引');
  await ctx.check('店铺终态订单索引已清理', 'eve', async () => {
    const shopOrderIds = await getOrderIdList((api.query as any).entityTransaction.shopOrders(shopId));
    const terminalIds = [serviceOrderId, sellerRefundOrderId, forceRefundOrderId].map(String);
    for (const terminalId of terminalIds) {
      if (shopOrderIds.includes(terminalId)) {
        throw new Error(`店铺终态订单仍存在于索引中: orderId=${terminalId}`);
      }
    }
    if (!shopOrderIds.includes(String(shippedOrderId))) {
      throw new Error(`店铺非终态订单被错误清理: orderId=${shippedOrderId}`);
    }
  });
  traceE10('after_cleanup_shop_orders', `shopId=${shopId}`);
  if (await maybeStopAfter(ctx, 'after_cleanup_shop_orders')) return;

  const currentBlock = (await api.rpc.chain.getHeader()).number.toNumber();
  traceE10('before_force_process_expirations', `block=${currentBlock}`);
  const processExpirationsResult = await sudoE10(
    ctx,
    (api.tx as any).entityTransaction.forceProcessExpirations(currentBlock),
    '管理员补偿处理过期订单',
  );
  assertTxSuccess(processExpirationsResult, '处理过期订单');
  await ctx.check('过期订单补偿已执行', 'sudo(alice)', () => {
    console.log(`    [E10] expirations_processed_at_block=${currentBlock}`);
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

async function sendE10(
  ctx: FlowContext,
  tx: any,
  signer: any,
  stepName: string,
  actorName?: string,
): Promise<any> {
  const headBefore = (await ctx.api.rpc.chain.getHeader()).number.toNumber();
  const signerStateBefore = await getSignerState(ctx, signer);
  traceE10(
    'tx_send_begin',
    `${stepName}, actor=${actorName ?? 'unknown'}, signer=${signer.address}, head=${headBefore}, nonce=${signerStateBefore.nonce}, nextIndex=${signerStateBefore.nextIndex}`,
  );
  const result = await ctx.send(tx, signer, stepName, actorName);
  const headAfter = (await ctx.api.rpc.chain.getHeader()).number.toNumber();
  const signerStateAfter = await getSignerState(ctx, signer);
  traceE10(
    'tx_send_end',
    `${stepName}, success=${result.success}, head=${headAfter}, nonce=${signerStateAfter.nonce}, nextIndex=${signerStateAfter.nextIndex}, blockHash=${result.blockHash ?? 'n/a'}, events=${formatE10Events(result)}, error=${result.error ?? 'none'}`,
  );
  return result;
}

async function sudoE10(
  ctx: FlowContext,
  tx: any,
  stepName: string,
): Promise<any> {
  const headBefore = (await ctx.api.rpc.chain.getHeader()).number.toNumber();
  traceE10('tx_sudo_begin', `${stepName}, head=${headBefore}`);
  const result = await ctx.sudo(tx, stepName);
  const headAfter = (await ctx.api.rpc.chain.getHeader()).number.toNumber();
  traceE10(
    'tx_sudo_end',
    `${stepName}, success=${result.success}, head=${headAfter}, blockHash=${result.blockHash ?? 'n/a'}, events=${formatE10Events(result)}, error=${result.error ?? 'none'}`,
  );
  return result;
}

function formatE10Events(result: any): string {
  if (!Array.isArray(result.events) || result.events.length === 0) return 'none';
  return result.events.map((e: any) => `${e.section}.${e.method}`).join(',');
}

async function getSignerState(
  ctx: FlowContext,
  signer: any,
): Promise<{ nonce: string; nextIndex: string }> {
  const account = await ctx.api.query.system.account(signer.address);
  let nextIndexValue = 'rpc_unavailable';
  try {
    const nextIndex = await ctx.api.rpc.system.accountNextIndex(signer.address);
    nextIndexValue = nextIndex.toString();
  } catch (error: any) {
    nextIndexValue = `rpc_error:${error.message}`;
  }
  return {
    nonce: (account as any).nonce.toString(),
    nextIndex: nextIndexValue,
  };
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
  const createEntityResult = await sendE10(
    ctx,
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
  const fundResult = await sendE10(
    ctx,
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
  const createResult = await sendE10(
    ctx,
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
  const publishResult = await sendE10(
    ctx,
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
  const orderId = (await (api.query as any).entityTransaction.nextOrderId()).toNumber();
  traceE10('before_place_order', `${stepName}, productId=${productId}`);
  if (stepName.includes('卖家退款')) {
    await logOrderPlacementSnapshot(ctx, signer.address, productId, `${stepName}:before`);
  }
  const placeOrderResult = await sendE10(
    ctx,
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
  if (stepName.includes('卖家退款')) {
    await logOrderPlacementSnapshot(ctx, signer.address, productId, `${stepName}:after`);
    traceE10(
      'seller_refund_order_tx_result',
      `success=${placeOrderResult.success}, error=${placeOrderResult.error ?? 'none'}`,
    );
  }
  assertTxSuccess(placeOrderResult, stepName);
  traceE10('after_place_order', `${stepName}, productId=${productId}`);
  await ctx.check(`下单结果已落库 @ ${stepName}`, 'system', async () => {
    const order = await (api.query as any).entityTransaction.orders(orderId);
    if (!hasStorageValue(order)) {
      throw new Error(`订单未写入: orderId=${orderId}`);
    }
  });
  return orderId;
}

async function logOrderPlacementSnapshot(
  ctx: FlowContext,
  buyerAddress: string,
  productId: number,
  label: string,
): Promise<void> {
  const { api } = ctx;
  await ctx.check(`E10 下单快照 @ ${label}`, 'system', async () => {
    const product = await (api.query as any).entityProduct.products(productId);
    const productHuman = product.toHuman() as Record<string, unknown>;
    const nextOrderId = (await (api.query as any).entityTransaction.nextOrderId()).toString();
    const buyerOrders = await (api.query as any).entityTransaction.buyerOrders(buyerAddress);
    const bobAccount = await api.query.system.account(buyerAddress);
    const bobFree = (bobAccount as any).data.free.toString();

    traceE10(
      'order_snapshot',
      `${label}, nextOrderId=${nextOrderId}, bobFree=${bobFree}, buyerOrders=${JSON.stringify(buyerOrders.toHuman())}`,
    );
    traceE10('order_snapshot_product', `${label}, product=${JSON.stringify(productHuman)}`);
  });
}

async function logOrderStateSnapshot(
  ctx: FlowContext,
  orderId: number,
  label: string,
): Promise<void> {
  const { api } = ctx;
  await ctx.check(`E10 订单快照 @ ${label}`, 'system', async () => {
    const order = await (api.query as any).entityTransaction.orders(orderId);
    const orderHuman = order.toHuman();
    traceE10('service_order_snapshot', `${label}, order=${JSON.stringify(orderHuman)}`);
  });
}

async function getOrderHuman(ctx: FlowContext, orderId: number): Promise<Record<string, any>> {
  const order = await (ctx.api.query as any).entityTransaction.orders(orderId);
  if (!hasStorageValue(order)) {
    throw new Error(`订单不存在: orderId=${orderId}`);
  }
  return (order.toHuman?.() as Record<string, any>) ?? (order.toJSON?.() as Record<string, any>) ?? {};
}

async function assertMemberActivated(
  ctx: FlowContext,
  entityId: number,
  account: string,
): Promise<void> {
  const member = await (ctx.api.query as any).entityMember.entityMembers(entityId, account);
  if (!hasStorageValue(member)) {
    throw new Error(`会员未自动注册: entityId=${entityId} account=${account}`);
  }
  const human = (member.toHuman?.() as Record<string, any>) ?? (member.toJSON?.() as Record<string, any>) ?? {};
  if (human.activated !== true && human.activated !== 'true') {
    throw new Error(`会员未激活: ${JSON.stringify(human)}`);
  }
}

async function getOrderIdList(query: Promise<any>): Promise<string[]> {
  const value = await query;
  const human = value.toHuman?.() ?? value.toJSON?.() ?? [];
  if (!Array.isArray(human)) return [];
  return human.map((item) => String(item).replace(/,/g, ''));
}

function hasStorageValue(value: any): boolean {
  if (typeof value?.isSome === 'boolean') return value.isSome;
  if (typeof value?.isEmpty === 'boolean') return !value.isEmpty;
  return value?.toJSON?.() != null;
}

function readOrderStatus(order: Record<string, any>): string {
  const status = order?.status;
  if (typeof status === 'string') return status;
  if (status && typeof status === 'object') {
    const keys = Object.keys(status);
    if (keys.length === 1) return keys[0];
  }
  return String(status);
}
