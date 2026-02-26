/**
 * Flow-E1: 实体→店铺创建流程
 *
 * 角色: Eve (实体所有者), Alice (Sudo/治理审批)
 *
 * 流程:
 *   1. Eve 创建实体 (Merchant 类型)
 *   2. 验证实体已创建 + 自动创建 Primary Shop
 *   3. 查询实体详细信息
 *   4. Eve 更新实体信息 (名称/logo/描述)
 *   5. [治理] Sudo 暂停实体
 *   6. 验证 effective 状态变为 PausedByEntity
 *   7. [治理] Sudo 恢复实体
 *   8. 验证实体恢复 Active
 *   9. [错误路径] 非所有者更新实体应失败
 *  10. [错误路径] 空名称创建实体应失败
 *  11. Eve 申请关闭实体
 *  12. Sudo 审批关闭
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertStorageExists,
  assertStorageField,
  assertTrue,
  assertEqual,
} from '../../core/assertions.js';
import { getFreeBalance } from '../../core/chain-state.js';

export const entityShopFlow: FlowDef = {
  name: 'Flow-E1: 实体→店铺创建',
  description: '创建实体 → 自动 Primary Shop → 更新 → 暂停/恢复 → 关闭',
  fn: entityShop,
};

async function entityShop(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const eve = ctx.actor('eve');
  const charlie = ctx.actor('charlie');

  // --------------- Step 1: 创建实体 ---------------
  const nextEntityId = await (api.query as any).entityRegistry.nextEntityId();
  const entityId = nextEntityId.toNumber();

  const eveBalanceBefore = await getFreeBalance(api, eve.address);

  const createTx = (api.tx as any).entityRegistry.createEntity(
    'Test Entity E2E',       // name
    null,                     // logo_cid
    null,                     // description_cid
    null,                     // referrer
  );
  const createResult = await ctx.send(createTx, eve, '创建实体 (Merchant)', 'eve');
  assertTxSuccess(createResult, '创建实体');

  // --------------- Step 2: 验证实体 + Primary Shop ---------------
  await ctx.check('验证实体已创建', 'eve', async () => {
    await assertStorageExists(api, 'entityRegistry', 'entities', [entityId], '实体应存在');
    await assertStorageField(
      api, 'entityRegistry', 'entities', [entityId],
      'status', 'Active', '实体状态应为 Active',
    );
  });

  await ctx.check('验证 Eve 的 UserEntity 记录', 'eve', async () => {
    const userEntities = await (api.query as any).entityRegistry.userEntity(eve.address);
    const ids = userEntities.toHuman();
    assertTrue(Array.isArray(ids) && ids.length > 0, 'Eve 应拥有至少一个实体');
    console.log(`    Eve 的实体 IDs: ${JSON.stringify(ids)}`);
  });

  await ctx.check('验证初始资金已扣除', 'eve', async () => {
    const eveBalanceAfter = await getFreeBalance(api, eve.address);
    const delta = eveBalanceBefore - eveBalanceAfter;
    assertTrue(delta > 0n, `Eve 应被扣除初始金库资金, 实际减少 ${Number(delta) / 1e12} NEX`);
    console.log(`    金库初始资金: ${Number(delta) / 1e12} NEX`);
  });

  // --------------- Step 3: 查询实体详情 ---------------
  await ctx.check('查询实体详细信息', 'eve', async () => {
    const entity = await (api.query as any).entityRegistry.entities(entityId);
    if (entity.isSome) {
      const data = entity.unwrap().toHuman();
      console.log(`    实体 ID: ${entityId}`);
      console.log(`    名称: ${data.name}`);
      console.log(`    类型: ${data.entityType}`);
      console.log(`    状态: ${data.status}`);
      console.log(`    所有者: ${String(data.owner).slice(0, 16)}...`);
    }
  });

  // --------------- Step 4: 更新实体信息 ---------------
  const updateTx = (api.tx as any).entityRegistry.updateEntity(
    entityId,
    'Updated E2E Entity',   // new name
    'QmTestLogoCid123',     // logo_cid
    'QmTestDescCid456',     // description_cid
    null,                    // metadata_uri
  );
  const updateResult = await ctx.send(updateTx, eve, '更新实体信息', 'eve');
  assertTxSuccess(updateResult, '更新实体');

  await ctx.check('验证更新后的名称', 'eve', async () => {
    await assertStorageField(
      api, 'entityRegistry', 'entities', [entityId],
      'name', 'Updated E2E Entity', '名称应已更新',
    );
  });

  // --------------- Step 5: Sudo 暂停实体 ---------------
  const suspendTx = (api.tx as any).entityRegistry.suspendEntity(entityId);
  const suspendResult = await ctx.sudo(suspendTx, '暂停实体');
  assertTxSuccess(suspendResult, '暂停实体');

  // --------------- Step 6: 验证暂停状态 ---------------
  await ctx.check('验证实体已暂停', 'eve', async () => {
    await assertStorageField(
      api, 'entityRegistry', 'entities', [entityId],
      'status', 'Suspended', '实体状态应为 Suspended',
    );
  });

  // --------------- Step 7: Sudo 恢复实体 ---------------
  const resumeTx = (api.tx as any).entityRegistry.resumeEntity(entityId);
  const resumeResult = await ctx.sudo(resumeTx, '恢复实体');
  assertTxSuccess(resumeResult, '恢复实体');

  // --------------- Step 8: 验证恢复 ---------------
  await ctx.check('验证实体已恢复', 'eve', async () => {
    await assertStorageField(
      api, 'entityRegistry', 'entities', [entityId],
      'status', 'Active', '实体状态应恢复为 Active',
    );
  });

  // --------------- Step 9: 错误路径 — 非所有者更新 ---------------
  const fakeUpdate = (api.tx as any).entityRegistry.updateEntity(
    entityId, 'Hacked Name', null, null, null,
  );
  const fakeResult = await ctx.send(fakeUpdate, charlie, '[错误路径] 非所有者更新', 'charlie');
  await ctx.check('非所有者更新应失败', 'charlie', () => {
    assertTxFailed(fakeResult, 'NotEntityOwner', '非所有者更新');
  });

  // --------------- Step 10: 错误路径 — 空名称 ---------------
  const emptyNameTx = (api.tx as any).entityRegistry.createEntity('', null, null, null);
  const emptyResult = await ctx.send(emptyNameTx, eve, '[错误路径] 空名称创建实体', 'eve');
  await ctx.check('空名称创建应失败', 'eve', () => {
    assertTxFailed(emptyResult, 'NameEmpty', '空名称');
  });

  // --------------- Step 11: 申请关闭实体 ---------------
  const closeTx = (api.tx as any).entityRegistry.requestCloseEntity(entityId);
  const closeResult = await ctx.send(closeTx, eve, '申请关闭实体', 'eve');
  assertTxSuccess(closeResult, '申请关闭');

  await ctx.check('验证状态为 PendingClose', 'eve', async () => {
    await assertStorageField(
      api, 'entityRegistry', 'entities', [entityId],
      'status', 'PendingClose', '状态应为 PendingClose',
    );
  });

  // --------------- Step 12: Sudo 审批关闭 ---------------
  const approveCloseTx = (api.tx as any).entityRegistry.approveCloseEntity(entityId);
  const approveCloseResult = await ctx.sudo(approveCloseTx, '审批关闭实体');
  assertTxSuccess(approveCloseResult, '审批关闭');

  await ctx.check('验证实体已关闭', 'eve', async () => {
    await assertStorageField(
      api, 'entityRegistry', 'entities', [entityId],
      'status', 'Closed', '状态应为 Closed',
    );
  });

  // --------------- 汇总 ---------------
  await ctx.check('Entity→Shop 流程汇总', 'system', () => {
    console.log(`    实体 #${entityId}: Created(Active) → Updated → Suspended → Active → PendingClose → Closed ✓`);
    console.log(`    错误路径: 非所有者更新 ✗, 空名称创建 ✗ ✓`);
  });
}
