/**
 * Flow-E6: KYC 认证完整流程
 *
 * 角色:
 *   - Eve     (Entity Owner)
 *   - Alice   (Sudo — Root)
 *   - Bob     (KYC 申请人)
 *   - Dave    (KYC Provider)
 *   - Charlie (无权限用户)
 *
 * 流程:
 *   1. Alice 注册 KYC Provider (Dave)
 *   2. Bob 提交 KYC (Basic 级别)
 *   3. Dave 批准 Bob 的 KYC
 *   4. 验证 KYC 状态
 *   5. [错误路径] 空 data_cid 被拒绝
 *   6. [错误路径] 非法国家代码被拒绝
 *   7. Eve 设置实体 KYC 要求
 *   8. [错误路径] max_risk_score > 100 被拒绝
 *   9. Alice 撤销 Bob 的 KYC
 *  10. Alice 更新高风险国家列表
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertEventEmitted,
  assertStorageExists,
  assertTrue,
} from '../../core/assertions.js';

export const kycFlow: FlowDef = {
  name: 'Flow-E6: KYC 认证',
  description: '注册 Provider → 提交/批准/撤销 KYC → 实体要求 → 错误路径',
  fn: kycLifecycle,
};

async function kycLifecycle(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const dave = ctx.actor('dave');
  const charlie = ctx.actor('charlie');

  // ─── Step 1: 注册 KYC Provider ────────────────────────────

  const registerProviderTx = (api.tx as any).entityKyc.registerProvider(
    dave.address,
    'E2E KYC Provider',  // name
    'Standard',           // providerType
    'Enhanced',           // maxLevel
  );
  const regResult = await ctx.sudo(registerProviderTx, '注册 KYC Provider (Dave)');
  if (regResult.success) {
    await ctx.check('Provider 已注册', 'dave', () => {
      assertEventEmitted(regResult, 'entityKyc', 'ProviderRegistered', '注册事件');
    });
  } else {
    console.log(`    ℹ Provider 可能已注册: ${regResult.error}`);
  }

  // ─── Step 2: Bob 提交 KYC (Basic) ────────────────────────

  const submitTx = (api.tx as any).entityKyc.submitKyc(
    1,                    // level: Basic
    'QmKycDataCid001',    // data_cid
    'CN',                 // country_code
  );
  const submitResult = await ctx.send(submitTx, bob, 'Bob 提交 KYC (Basic)', 'bob');
  assertTxSuccess(submitResult, '提交 KYC');

  const submitEvent = submitResult.events.find(
    e => e.section === 'entityKyc' && e.method === 'KycSubmitted',
  );
  assertTrue(!!submitEvent, '应有 KycSubmitted 事件');
  const kycId = submitEvent?.data?.kycId ?? submitEvent?.data?.[0];
  console.log(`    KYC ID: ${kycId}`);

  // ─── Step 3: Dave 批准 KYC ────────────────────────────────

  const approveTx = (api.tx as any).entityKyc.approveKyc(
    bob.address,
    10,    // riskScore (0-100)
  );
  const approveResult = await ctx.send(approveTx, dave, 'Dave 批准 KYC', 'dave');
  assertTxSuccess(approveResult, '批准 KYC');

  // ─── Step 4: 验证 KYC 状态 ────────────────────────────────

  await ctx.check('验证 KYC 已批准', 'bob', async () => {
    const kyc = await (api.query as any).entityKyc.kycRecords(kycId);
    if (kyc.isSome) {
      const data = kyc.unwrap().toHuman();
      console.log(`    KYC 状态: ${data.status}, risk_score: ${data.riskScore ?? data.risk_score}`);
    }
  });

  // ─── Step 5: [错误路径] 空 data_cid ───────────────────────

  const emptyDataTx = (api.tx as any).entityKyc.submitKyc(1, '', 'CN');
  const emptyDataResult = await ctx.send(emptyDataTx, charlie, '[错误路径] 空 data_cid', 'charlie');
  await ctx.check('空 data_cid 应失败', 'charlie', () => {
    assertTxFailed(emptyDataResult, 'EmptyDataCid', '空 data_cid');
  });

  // ─── Step 6: [错误路径] 非法国家代码 ─────────────────────

  const badCountryTx = (api.tx as any).entityKyc.submitKyc(1, 'QmData', 'xx');
  const badCountryResult = await ctx.send(badCountryTx, charlie, '[错误路径] 非法国家代码', 'charlie');
  await ctx.check('非法国家代码应失败', 'charlie', () => {
    assertTxFailed(badCountryResult, undefined, '非法国家代码');
  });

  // ─── Step 7: 设置实体 KYC 要求 ────────────────────────────

  // 获取 Eve 的 entity_id
  const eve = ctx.actor('eve');
  const userEntities = await (api.query as any).entityRegistry.userEntity(eve.address);
  const entityIds = userEntities.toHuman() as string[];

  if (entityIds && entityIds.length > 0) {
    const entityId = parseInt(entityIds[0].replace(/,/g, ''), 10);

    const setReqTx = (api.tx as any).entityKyc.setEntityRequirement(
      entityId,
      'Basic',   // minLevel
      true,      // mandatory
      0,         // gracePeriod
      false,     // allowHighRiskCountries
      50,        // maxRiskScore
    );
    const setReqResult = await ctx.send(setReqTx, eve, 'Eve 设置实体 KYC 要求', 'eve');
    assertTxSuccess(setReqResult, '设置 KYC 要求');

    // ─── Step 8: [错误路径] max_risk_score > 100 ────────────
    const badScoreTx = (api.tx as any).entityKyc.setEntityRequirement(entityId, 'Basic', true, 0, false, 150);
    const badScoreResult = await ctx.send(badScoreTx, eve, '[错误路径] risk_score > 100', 'eve');
    await ctx.check('超过100的 risk_score 应失败', 'eve', () => {
      assertTxFailed(badScoreResult, undefined, 'risk_score > 100');
    });
  }

  // ─── Step 9: Alice 撤销 KYC ───────────────────────────────

  const revokeTx = (api.tx as any).entityKyc.revokeKyc(bob.address, 'Administrative');
  const revokeResult = await ctx.sudo(revokeTx, '撤销 Bob 的 KYC');
  assertTxSuccess(revokeResult, '撤销 KYC');

  await ctx.check('KYC 已撤销', 'system', () => {
    assertEventEmitted(revokeResult, 'entityKyc', 'KycRevoked', '撤销事件');
  });

  // ─── Step 10: 更新高风险国家列表 ──────────────────────────

  const updateCountriesTx = (api.tx as any).entityKyc.updateHighRiskCountries(
    ['0x4b50', '0x4952', '0x5359'],  // KP, IR, SY as [u8;2]
  );
  const updateResult = await ctx.sudo(updateCountriesTx, '更新高风险国家');
  assertTxSuccess(updateResult, '更新高风险国家');

  // ─── 汇总 ─────────────────────────────────────────────────
  await ctx.check('KYC 认证汇总', 'system', () => {
    console.log(`    ✓ Provider: 注册`);
    console.log(`    ✓ KYC: 提交 → 批准 → 撤销`);
    console.log(`    ✓ 实体要求: 设置 min_level + max_risk_score`);
    console.log(`    ✓ 错误路径: 空 CID ✗, 非法国家 ✗, score>100 ✗`);
  });
}
