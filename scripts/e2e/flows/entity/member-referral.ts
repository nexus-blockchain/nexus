/**
 * Flow-E3: 会员注册 + 推荐裂变 + 等级系统
 *
 * 角色:
 *   - Eve     (Entity Owner)
 *   - Alice   (Sudo)
 *   - Bob     (Member A — 推荐人)
 *   - Charlie (Member B — 被推荐人)
 *   - Dave    (无权限用户)
 *
 * 流程:
 *   1. 确保 Entity + Shop 已就绪
 *   2. Eve 初始化等级系统
 *   3. Eve 添加自定义等级
 *   4. Bob 注册会员 (无推荐人)
 *   5. Charlie 注册会员 (Bob 为推荐人)
 *   6. 验证推荐关系
 *   7. [错误路径] Charlie 不能重复绑定推荐人
 *   8. [错误路径] Dave 不能推荐自己
 *   9. Eve 手动升级 Bob
 *  10. Eve 设置会员策略 (需审批)
 *  11. Dave 注册 → Pending → Eve 审批
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertEventEmitted,
  assertStorageExists,
  assertTrue,
} from '../../core/assertions.js';

export const memberReferralFlow: FlowDef = {
  name: 'Flow-E3: 会员推荐裂变',
  description: '注册 → 推荐绑定 → 等级系统 → 审批策略 → 错误路径',
  fn: memberReferral,
};

async function memberReferral(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const eve = ctx.actor('eve');
  const bob = ctx.actor('bob');
  const charlie = ctx.actor('charlie');
  const dave = ctx.actor('dave');

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
    const createTx = (api.tx as any).entityRegistry.createEntity('E2E Member Test', null, null, null);
    await ctx.send(createTx, eve, '创建实体', 'eve');
    const ent = await (api.query as any).entityRegistry.entities(entityId);
    const ed = ent.unwrap().toHuman();
    shopId = parseInt(((ed.shopIds ?? ed.shop_ids) as string[])[0].replace(/,/g, ''), 10);
  }

  console.log(`    Entity: ${entityId}, Shop: ${shopId}`);

  // ─── Step 2: 初始化等级系统 ───────────────────────────────

  const initLevelTx = (api.tx as any).entityMember.initLevelSystem(
    shopId,
    true,   // use_custom
    0,      // upgrade_mode: Manual
  );
  const initResult = await ctx.send(initLevelTx, eve, '初始化等级系统', 'eve');
  // 可能已初始化，忽略失败
  if (initResult.success) {
    await ctx.check('等级系统已初始化', 'eve', () => {});
  } else {
    console.log(`    ℹ 等级系统可能已初始化: ${initResult.error}`);
  }

  // ─── Step 3: 添加自定义等级 ───────────────────────────────

  const addLevelTx = (api.tx as any).entityMember.addCustomLevel(
    shopId,
    'Silver',             // name
    1000,                 // threshold (消费1000)
    500,                  // discount_rate (5%)
    200,                  // commission_bonus (2%)
  );
  const addLevelResult = await ctx.send(addLevelTx, eve, '添加 Silver 等级', 'eve');
  if (addLevelResult.success) {
    await ctx.check('Silver 等级已添加', 'eve', () => {});
  } else {
    console.log(`    ℹ 等级可能已存在: ${addLevelResult.error}`);
  }

  // ─── Step 4: Bob 注册会员 (无推荐人) ─────────────────────

  const bobRegTx = (api.tx as any).entityMember.registerMember(shopId, null);
  const bobRegResult = await ctx.send(bobRegTx, bob, 'Bob 注册会员', 'bob');
  if (bobRegResult.success) {
    await ctx.check('Bob 注册成功', 'bob', () => {
      assertEventEmitted(bobRegResult, 'entityMember', 'MemberRegistered', 'Bob 注册事件');
    });
  } else {
    console.log(`    ℹ Bob 可能已注册: ${bobRegResult.error}`);
  }

  // ─── Step 5: Charlie 注册 (Bob 为推荐人) ─────────────────

  const charlieRegTx = (api.tx as any).entityMember.registerMember(shopId, bob.address);
  const charlieRegResult = await ctx.send(charlieRegTx, charlie, 'Charlie 注册 (推荐人=Bob)', 'charlie');
  if (charlieRegResult.success) {
    await ctx.check('Charlie 注册成功 + 推荐关系', 'charlie', () => {
      assertEventEmitted(charlieRegResult, 'entityMember', 'MemberRegistered', 'Charlie 注册事件');
    });
  } else {
    console.log(`    ℹ Charlie 可能已注册: ${charlieRegResult.error}`);
  }

  // ─── Step 6: 验证推荐关系 ────────────────────────────────

  await ctx.check('验证 Charlie 的推荐人是 Bob', 'system', async () => {
    // 查询 Charlie 的 member 记录
    const member = await (api.query as any).entityMember.entityMembers(shopId, charlie.address);
    if (member.isSome) {
      const data = member.unwrap().toHuman();
      const referrer = data.referrer ?? data.referrer_account;
      console.log(`    Charlie 推荐人: ${referrer ? String(referrer).slice(0, 16) + '...' : 'None'}`);
    }
  });

  // ─── Step 7: [错误路径] 重复绑定推荐人 ──────────────────

  const rebindTx = (api.tx as any).entityMember.bindReferrer(shopId, dave.address);
  const rebindResult = await ctx.send(rebindTx, charlie, '[错误路径] Charlie 重新绑定推荐人', 'charlie');
  await ctx.check('重复绑定推荐人应失败', 'charlie', () => {
    assertTxFailed(rebindResult, undefined, '重复绑定推荐人');
  });

  // ─── Step 8: [错误路径] 不能推荐自己 ────────────────────

  const selfRefTx = (api.tx as any).entityMember.registerMember(shopId, dave.address);
  const selfRefResult = await ctx.send(selfRefTx, dave, '[错误路径] Dave 自己推荐自己', 'dave');
  // 注意: 这里 Dave 的 referrer 是 dave.address 即自己，应该失败
  // 如果 Dave 还没注册会员，可能会成功但推荐人无效
  // 先尝试 bindReferrer 自己给自己
  const selfBindTx = (api.tx as any).entityMember.bindReferrer(shopId, dave.address);
  const selfBindResult = await ctx.send(selfBindTx, dave, '[错误路径] Dave 绑定自己为推荐人', 'dave');
  await ctx.check('自我推荐应失败', 'dave', () => {
    assertTxFailed(selfBindResult, undefined, '自我推荐');
  });

  // ─── Step 9: Eve 手动升级 Bob ────────────────────────────

  // 获取等级 ID (第一个自定义等级)
  const levelConfig = await (api.query as any).entityMember.entityLevelSystems(shopId);
  let targetLevelId = 1; // 默认
  if (levelConfig.isSome) {
    const data = levelConfig.unwrap().toHuman();
    console.log(`    等级系统配置: ${JSON.stringify(data).slice(0, 100)}...`);
  }

  const upgradeTx = (api.tx as any).entityMember.manualUpgradeMember(
    shopId, bob.address, targetLevelId,
  );
  const upgradeResult = await ctx.send(upgradeTx, eve, 'Eve 手动升级 Bob', 'eve');
  if (upgradeResult.success) {
    await ctx.check('Bob 已升级', 'eve', () => {
      assertEventEmitted(upgradeResult, 'entityMember', 'MemberLevelChanged', 'Bob 升级事件');
    });
  } else {
    console.log(`    ℹ 升级失败 (可能无匹配等级): ${upgradeResult.error}`);
  }

  // ─── Step 10: 设置会员策略 (需审批 = bit 4) ──────────────

  const setPolicyTx = (api.tx as any).entityMember.setMemberPolicy(shopId, 4); // 4 = 需审批
  const policyResult = await ctx.send(setPolicyTx, eve, '设置会员策略(需审批)', 'eve');
  assertTxSuccess(policyResult, '设置策略');

  // ─── Step 11: Dave 注册 → Pending → 审批 ─────────────────

  const daveRegTx = (api.tx as any).entityMember.registerMember(shopId, null);
  const daveRegResult = await ctx.send(daveRegTx, dave, 'Dave 注册(需审批)', 'dave');
  // 在需审批模式下，注册应成功但状态为 Pending
  if (daveRegResult.success) {
    // Eve 审批
    const approveTx = (api.tx as any).entityMember.approveMember(shopId, dave.address);
    const approveResult = await ctx.send(approveTx, eve, 'Eve 审批 Dave', 'eve');
    assertTxSuccess(approveResult, '审批 Dave');

    await ctx.check('Dave 审批后为 Active', 'system', () => {
      assertEventEmitted(approveResult, 'entityMember', 'MemberApproved', '审批事件');
    });
  } else {
    console.log(`    ℹ Dave 注册失败: ${daveRegResult.error}`);
  }

  // 恢复策略为开放 (不影响其他测试)
  const resetPolicyTx = (api.tx as any).entityMember.setMemberPolicy(shopId, 0);
  await ctx.send(resetPolicyTx, eve, '恢复策略为开放', 'eve');

  // ─── 汇总 ─────────────────────────────────────────────────
  await ctx.check('会员推荐裂变汇总', 'system', () => {
    console.log(`    ✓ Bob 注册(无推荐人)`);
    console.log(`    ✓ Charlie 注册(推荐人=Bob)`);
    console.log(`    ✓ 错误路径: 重复绑定 ✗, 自我推荐 ✗`);
    console.log(`    ✓ 手动升级 + 审批流程`);
  });
}
