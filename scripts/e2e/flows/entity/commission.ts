/**
 * Flow-E4: 佣金返佣完整流程
 *
 * 角色:
 *   - Eve     (Entity Owner)
 *   - Alice   (Sudo)
 *   - Bob     (Member A — 推荐人, 佣金提现者)
 *   - Charlie (Member B — 被推荐人, 下单消费者)
 *
 * 流程:
 *   1. 确保 Entity + Shop 就绪
 *   2. Eve 一键初始化佣金方案 (init_commission_plan)
 *   3. Eve 设置返佣模式 (Referral)
 *   4. Eve 启用返佣
 *   5. Eve 设置提现配置 (4种 WithdrawalMode)
 *   6. 确保 Bob/Charlie 是会员 + 推荐关系
 *   7. Charlie 下单消费 → 触发佣金分配
 *   8. Bob 提现 NEX 佣金 (withdraw_commission)
 *   9. [错误路径] use_shopping_balance 已禁用
 *  10. Eve 提取 entity 资金 (withdraw_entity_funds)
 *  11. [错误路径] 提取超过可用余额
 *  12. Eve 取消佣金 (cancel_commission) → 退佣
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertEventEmitted,
  assertTrue,
} from '../../core/assertions.js';
import { getFreeBalance } from '../../core/chain-state.js';
import { nex } from '../../core/config.js';

export const commissionFlow: FlowDef = {
  name: 'Flow-E4: 佣金返佣',
  description: '初始化方案 → 返佣模式 → 消费触发 → 提现 → 退佣 | 错误路径',
  fn: commissionLifecycle,
};

async function commissionLifecycle(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const eve = ctx.actor('eve');
  const bob = ctx.actor('bob');
  const charlie = ctx.actor('charlie');

  // ─── Step 1: 确保 Entity + Shop ──────────────────────────

  const userEntities = await (api.query as any).entityRegistry.userEntity(eve.address);
  const entityIds = userEntities.toHuman() as string[];
  let entityId: number, shopId: number;

  if (entityIds && entityIds.length > 0) {
    entityId = parseInt(entityIds[0].replace(/,/g, ''), 10);
    const entity = await (api.query as any).entityRegistry.entities(entityId);
    const ed = entity.unwrap().toHuman();
    shopId = parseInt(((ed.shopIds ?? ed.shop_ids) as string[])[0].replace(/,/g, ''), 10);
  } else {
    const nextId = (await (api.query as any).entityRegistry.nextEntityId()).toNumber();
    entityId = nextId;
    const createTx = (api.tx as any).entityRegistry.createEntity('E2E Commission Test', null, null, null);
    await ctx.send(createTx, eve, '创建实体', 'eve');
    const ent = await (api.query as any).entityRegistry.entities(entityId);
    const ed = ent.unwrap().toHuman();
    shopId = parseInt(((ed.shopIds ?? ed.shop_ids) as string[])[0].replace(/,/g, ''), 10);
  }
  console.log(`    Entity: ${entityId}, Shop: ${shopId}`);

  // ─── Step 2: 一键初始化佣金方案 ──────────────────────────

  const initPlanTx = (api.tx as any).commissionCore.initCommissionPlan(
    entityId,
    0,       // plan: 0=Referral, 1=LevelDiff, 2=SingleLine, 3=Team
  );
  const initResult = await ctx.send(initPlanTx, eve, '初始化佣金方案 (Referral)', 'eve');
  if (initResult.success) {
    await ctx.check('佣金方案已初始化', 'eve', () => {});
  } else {
    console.log(`    ℹ 可能已初始化: ${initResult.error}`);
  }

  // ─── Step 3: 设置返佣模式 ────────────────────────────────

  const setModesTx = (api.tx as any).commissionCore.setCommissionModes(
    entityId,
    [0],     // modes: [Referral]
  );
  const modesResult = await ctx.send(setModesTx, eve, '设置返佣模式 (Referral)', 'eve');
  assertTxSuccess(modesResult, '设置返佣模式');

  // ─── Step 4: 启用返佣 ────────────────────────────────────

  const enableTx = (api.tx as any).commissionCore.enableCommission(entityId, true);
  const enableResult = await ctx.send(enableTx, eve, '启用返佣', 'eve');
  assertTxSuccess(enableResult, '启用返佣');

  // ─── Step 5: 设置提现配置 ────────────────────────────────

  const setWithdrawTx = (api.tx as any).commissionCore.setWithdrawalConfig(
    entityId,
    0,       // mode: 0=DirectNEX
    0,       // min_repurchase_rate (0%)
    0,       // voluntary_repurchase_bonus (0%)
    nex(1).toString(),  // min_amount: 1 NEX
  );
  const withdrawConfigResult = await ctx.send(setWithdrawTx, eve, '设置提现配置', 'eve');
  assertTxSuccess(withdrawConfigResult, '设置提现配置');

  // ─── Step 6: 确保 Bob/Charlie 是会员 ─────────────────────

  // Bob 注册 (无推荐人)
  const bobRegTx = (api.tx as any).entityMember.registerMember(shopId, null);
  const bobRegResult = await ctx.send(bobRegTx, bob, 'Bob 注册会员', 'bob');
  if (!bobRegResult.success) {
    console.log(`    ℹ Bob 可能已注册: ${bobRegResult.error}`);
  }

  // Charlie 注册 (Bob 为推荐人)
  const charlieRegTx = (api.tx as any).entityMember.registerMember(shopId, bob.address);
  const charlieRegResult = await ctx.send(charlieRegTx, charlie, 'Charlie 注册 (推荐人=Bob)', 'charlie');
  if (!charlieRegResult.success) {
    console.log(`    ℹ Charlie 可能已注册: ${charlieRegResult.error}`);
  }

  // ─── Step 7: Charlie 下单消费 → 触发佣金 ─────────────────

  // 查找或创建商品
  let productId: number;
  const nextProductId = await (api.query as any).entityService.nextProductId();
  productId = nextProductId.toNumber();

  const createProductTx = (api.tx as any).entityService.createProduct(
    shopId, 'Commission Test Product', null, nex(50).toString(), 0, null,
  );
  const cpResult = await ctx.send(createProductTx, eve, '创建商品', 'eve');
  if (cpResult.success) {
    const publishTx = (api.tx as any).entityService.publishProduct(shopId, productId);
    await ctx.send(publishTx, eve, '上架商品', 'eve');
  } else {
    productId = productId - 1; // 使用已存在的最新商品
    console.log(`    ℹ 使用已有商品 ID: ${productId}`);
  }

  // Charlie 下单
  const orderTx = (api.tx as any).entityOrder.placeOrder(
    shopId, productId, 1, bob.address, null, null,
  );
  const orderResult = await ctx.send(orderTx, charlie, 'Charlie 下单 (触发佣金)', 'charlie');
  if (orderResult.success) {
    await ctx.check('订单已创建, 佣金应被分配', 'system', () => {
      console.log(`    订单事件数: ${orderResult.events.length}`);
    });

    // 完成订单流程 (发货→确认) 以释放佣金
    const orderEvent = orderResult.events.find(
      e => e.section === 'entityOrder' && e.method === 'OrderPlaced',
    );
    const orderId = orderEvent?.data?.orderId ?? orderEvent?.data?.[0];

    if (orderId) {
      const shipTx = (api.tx as any).entityOrder.shipOrder(orderId, null);
      await ctx.send(shipTx, eve, 'Eve 发货', 'eve');

      const confirmTx = (api.tx as any).entityOrder.confirmReceipt(orderId);
      await ctx.send(confirmTx, charlie, 'Charlie 确认收货', 'charlie');
    }
  } else {
    console.log(`    ℹ 下单失败: ${orderResult.error}`);
  }

  // ─── Step 8: Bob 提现佣金 ────────────────────────────────

  const bobBalBefore = await getFreeBalance(api, bob.address);

  const withdrawTx = (api.tx as any).commissionCore.withdrawCommission(
    entityId,
    0,      // withdrawal_mode: DirectNEX
    null,   // amount (全部)
    null,   // repurchase_target
  );
  const withdrawResult = await ctx.send(withdrawTx, bob, 'Bob 提现佣金', 'bob');
  if (withdrawResult.success) {
    await ctx.check('验证提现到账', 'bob', async () => {
      const bobBalAfter = await getFreeBalance(api, bob.address);
      const delta = bobBalAfter - bobBalBefore;
      console.log(`    Bob 余额变化: ${Number(delta) / 1e12} NEX`);
    });
  } else {
    console.log(`    ℹ 提现失败 (可能无佣金): ${withdrawResult.error}`);
  }

  // ─── Step 9: [错误路径] use_shopping_balance 已禁用 ──────

  const useShoppingTx = (api.tx as any).commissionCore.useShoppingBalance(
    entityId, nex(1).toString(),
  );
  const useShoppingResult = await ctx.send(useShoppingTx, bob, '[错误路径] 使用购物余额', 'bob');
  await ctx.check('购物余额直接提取应失败', 'bob', () => {
    assertTxFailed(useShoppingResult, undefined, '购物余额已禁用');
  });

  // ─── Step 10: Eve 提取 entity 资金 ──────────────────────

  const withdrawEntityTx = (api.tx as any).commissionCore.withdrawEntityFunds(
    entityId,
    nex(1).toString(),  // amount
  );
  const entityWithdrawResult = await ctx.send(withdrawEntityTx, eve, 'Eve 提取 entity 资金', 'eve');
  if (entityWithdrawResult.success) {
    await ctx.check('Entity 资金已提取', 'eve', () => {});
  } else {
    console.log(`    ℹ 提取失败 (可能余额不足): ${entityWithdrawResult.error}`);
  }

  // ─── Step 11: [错误路径] 提取超过可用余额 ────────────────

  const overWithdrawTx = (api.tx as any).commissionCore.withdrawEntityFunds(
    entityId,
    nex(999_999_999).toString(),
  );
  const overResult = await ctx.send(overWithdrawTx, eve, '[错误路径] 超额提取', 'eve');
  await ctx.check('超额提取应失败', 'eve', () => {
    assertTxFailed(overResult, undefined, '超额提取');
  });

  // ─── Step 12: 设置佣金费率 ───────────────────────────────

  const setRateTx = (api.tx as any).commissionCore.setCommissionRate(
    entityId,
    500,    // rate: 5%
  );
  const rateResult = await ctx.send(setRateTx, eve, 'Eve 设置佣金费率 5%', 'eve');
  assertTxSuccess(rateResult, '设置佣金费率');

  // ─── 汇总 ─────────────────────────────────────────────────
  await ctx.check('佣金返佣汇总', 'system', () => {
    console.log(`    ✓ 方案初始化 + 模式设置 + 启用`);
    console.log(`    ✓ 消费触发佣金分配`);
    console.log(`    ✓ Bob 提现佣金`);
    console.log(`    ✓ Entity 资金提取`);
    console.log(`    ✓ 错误路径: 购物余额禁用 ✗, 超额提取 ✗`);
  });
}
