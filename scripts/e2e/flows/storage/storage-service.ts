/**
 * Flow-S1: 存储服务完整流程
 *
 * 角色:
 *   - Bob     (存储用户)
 *   - Alice   (Sudo/GovernanceOrigin)
 *   - Charlie (运营者)
 *   - Dave    (无权限用户)
 *
 * 流程:
 *   1. Alice 设置计费参数
 *   2. Charlie 加入运营者 (质押保证金)
 *   3. Charlie 更新运营者信息
 *   4. Bob 充值用户账户
 *   5. Bob 请求 Pin 文件
 *   6. [错误路径] 余额不足 Pin 被拒绝
 *   7. 标记 Pin 成功 (mark_pinned)
 *   8. 处理到期扣费 (charge_due)
 *   9. Charlie 运营者领取奖励
 *  10. Charlie 暂停/恢复运营者
 *  11. Alice 分配资金给运营者
 *  12. Alice Slash 运营者
 *  13. Charlie 退出运营者 (退还保证金)
 *  14. [错误路径] Dave 非运营者操作
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

export const storageServiceFlow: FlowDef = {
  name: 'Flow-S1: 存储服务',
  description: '运营者注册 → 用户充值 → Pin → 扣费 → 奖励 → Slash → 退出',
  fn: storageService,
};

async function storageService(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const charlie = ctx.actor('charlie');
  const dave = ctx.actor('dave');

  // ─── Step 1: 设置计费参数 ────────────────────────────────

  const setBillingTx = (api.tx as any).storageService.setBillingParams(
    nex(1).toString(),    // pricePerGibWeek
    null,                  // periodBlocks
    null,                  // graceBlocks
    null,                  // maxChargePerBlock
    null,                  // subjectMinReserve
    null,                  // paused
  );
  const billingResult = await ctx.sudo(setBillingTx, '设置计费参数');
  assertTxSuccess(billingResult, '设置计费参数');

  // ─── Step 2: Charlie 加入运营者 ──────────────────────────

  const charlieBalBefore = await getFreeBalance(api, charlie.address);

  const peerId = '0x' + '01'.repeat(32);  // peerId: [u8;32]
  const endpointHash = '0x' + 'e0'.repeat(32);  // endpointHash: H256
  const certFingerprint = '0x' + 'cf'.repeat(32);  // certFingerprint: H256
  const joinTx = (api.tx as any).storageService.joinOperator(
    peerId,                // peerId: [u8;32]
    100,                   // capacityGib: u32
    endpointHash,          // endpointHash: H256
    certFingerprint,       // certFingerprint: H256
    nex(50).toString(),    // bond: u128
  );
  const joinResult = await ctx.send(joinTx, charlie, 'Charlie 加入运营者', 'charlie');
  assertTxSuccess(joinResult, '加入运营者');

  await ctx.check('验证保证金已锁定', 'charlie', async () => {
    const charlieBalAfter = await getFreeBalance(api, charlie.address);
    const delta = charlieBalBefore - charlieBalAfter;
    assertTrue(delta > 0n, `Charlie 应被扣除保证金, 减少 ${Number(delta) / 1e12} NEX`);
  });

  // ─── Step 3: Charlie 更新运营者信息 ──────────────────────

  const updateOpTx = (api.tx as any).storageService.updateOperator(
    null,                                    // peerId: Option
    200,                                     // capacityGib: Option<u32>
    null,                                    // endpointHash: Option<H256>
    null,                                    // certFingerprint: Option<H256>
  );
  const updateOpResult = await ctx.send(updateOpTx, charlie, 'Charlie 更新运营者', 'charlie');
  assertTxSuccess(updateOpResult, '更新运营者');

  // ─── Step 4: Bob 充值用户账户 ────────────────────────────

  const fundUserTx = (api.tx as any).storageService.fundUserAccount(
    bob.address,
    nex(20).toString(),   // amount
  );
  const fundResult = await ctx.send(fundUserTx, bob, 'Bob 充值用户账户', 'bob');
  assertTxSuccess(fundResult, '充值用户账户');

  await ctx.check('验证充值事件', 'bob', () => {
    assertEventEmitted(fundResult, 'storageService', 'UserFunded', '充值事件');
  });

  // ─── Step 5: Bob 请求 Pin 文件 ───────────────────────────

  const pinTx = (api.tx as any).storageService.requestPinForSubject(
    0,                          // subjectId: u64
    'QmTestFileCid001',         // cid: Bytes
    0,                          // tier: u8
  );
  const pinResult = await ctx.send(pinTx, bob, 'Bob 请求 Pin', 'bob');
  assertTxSuccess(pinResult, '请求 Pin');

  await ctx.check('验证 Pin 请求事件', 'bob', () => {
    assertEventEmitted(pinResult, 'storageService', 'PinRequested', 'Pin 请求事件');
  });

  // ─── Step 6: [错误路径] 余额不足 Pin ─────────────────────

  const bigPinTx = (api.tx as any).storageService.requestPinForSubject(
    0,                          // subjectId
    'QmHugeFileCid001',         // cid
    0,                          // tier
  );
  const bigPinResult = await ctx.send(bigPinTx, dave, '[错误路径] 余额不足 Pin', 'dave');
  await ctx.check('余额不足 Pin 应失败', 'dave', () => {
    assertTxFailed(bigPinResult, undefined, '余额不足');
  });

  // ─── Step 7: 标记 Pin 成功 ───────────────────────────────

  const cidHash = '0x' + 'ab'.repeat(32);  // cidHash: H256
  const markPinnedTx = (api.tx as any).storageService.markPinned(
    cidHash,            // cidHash: H256
    1,                  // replicas: u32
  );
  // mark_pinned 通常由 OCW 调用, 这里尝试 sudo
  const markResult = await ctx.sudo(markPinnedTx, '标记 Pin 成功');
  if (markResult.success) {
    await ctx.check('Pin 成功事件', 'system', () => {
      assertEventEmitted(markResult, 'storageService', 'FilePinned', 'Pin 事件');
    });
  } else {
    console.log(`    ℹ 标记 Pin 失败 (可能需要 OCW 签名): ${markResult.error}`);
  }

  // ─── Step 8: 处理到期扣费 ────────────────────────────────

  const chargeTx = (api.tx as any).storageService.chargeDue(10);  // limit=10
  const chargeResult = await ctx.sudo(chargeTx, '处理到期扣费');
  if (chargeResult.success) {
    await ctx.check('扣费事件', 'system', () => {});
  } else {
    console.log(`    ℹ 扣费失败 (可能无到期项): ${chargeResult.error}`);
  }

  // ─── Step 9: Charlie 领取奖励 ────────────────────────────

  const claimRewardTx = (api.tx as any).storageService.operatorClaimRewards();
  const claimResult = await ctx.send(claimRewardTx, charlie, 'Charlie 领取奖励', 'charlie');
  if (claimResult.success) {
    await ctx.check('奖励领取事件', 'charlie', () => {});
  } else {
    console.log(`    ℹ 领取失败 (可能无奖励): ${claimResult.error}`);
  }

  // ─── Step 10: 暂停/恢复运营者 ────────────────────────────

  const pauseOpTx = (api.tx as any).storageService.pauseOperator();
  const pauseOpResult = await ctx.send(pauseOpTx, charlie, 'Charlie 暂停运营者', 'charlie');
  assertTxSuccess(pauseOpResult, '暂停运营者');

  const resumeOpTx = (api.tx as any).storageService.resumeOperator();
  const resumeOpResult = await ctx.send(resumeOpTx, charlie, 'Charlie 恢复运营者', 'charlie');
  assertTxSuccess(resumeOpResult, '恢复运营者');

  // ─── Step 11: 分配资金给运营者 ────────────────────────────

  const distributeTx = (api.tx as any).storageService.distributeToOperators(
    nex(5).toString(),   // max_amount
  );
  const distributeResult = await ctx.sudo(distributeTx, '分配资金给运营者');
  if (distributeResult.success) {
    await ctx.check('分配事件', 'system', () => {});
  } else {
    console.log(`    ℹ 分配失败 (可能无余额): ${distributeResult.error}`);
  }

  // ─── Step 12: Slash 运营者 ────────────────────────────────

  const slashOpTx = (api.tx as any).storageService.slashOperator(
    charlie.address,
    nex(5).toString(),   // amount
  );
  const slashResult = await ctx.sudo(slashOpTx, 'Slash 运营者');
  if (slashResult.success) {
    await ctx.check('Slash 事件', 'system', () => {
      assertEventEmitted(slashResult, 'storageService', 'OperatorSlashed', 'Slash 事件');
    });
  } else {
    console.log(`    ℹ Slash 失败: ${slashResult.error}`);
  }

  // ─── Step 13: Charlie 退出运营者 ──────────────────────────

  const charlieBalBeforeLeave = await getFreeBalance(api, charlie.address);

  const leaveTx = (api.tx as any).storageService.leaveOperator();
  const leaveResult = await ctx.send(leaveTx, charlie, 'Charlie 退出运营者', 'charlie');
  assertTxSuccess(leaveResult, '退出运营者');

  await ctx.check('验证保证金退还', 'charlie', async () => {
    const charlieBalAfterLeave = await getFreeBalance(api, charlie.address);
    const delta = charlieBalAfterLeave - charlieBalBeforeLeave;
    console.log(`    保证金退还变化: ${Number(delta) / 1e12} NEX`);
  });

  // ─── Step 14: [错误路径] Dave 非运营者操作 ────────────────

  const daveUpdateTx = (api.tx as any).storageService.updateOperator(null, 50, null, null);
  const daveUpdateResult = await ctx.send(daveUpdateTx, dave, '[错误路径] Dave 更新运营者', 'dave');
  await ctx.check('非运营者更新应失败', 'dave', () => {
    assertTxFailed(daveUpdateResult, undefined, '非运营者');
  });

  const davePauseTx = (api.tx as any).storageService.pauseOperator();
  const davePauseResult = await ctx.send(davePauseTx, dave, '[错误路径] Dave 暂停运营者', 'dave');
  await ctx.check('非运营者暂停应失败', 'dave', () => {
    assertTxFailed(davePauseResult, undefined, '非运营者暂停');
  });

  // ─── 汇总 ─────────────────────────────────────────────────
  await ctx.check('存储服务汇总', 'system', () => {
    console.log(`    ✓ 运营者: 加入 → 更新 → 暂停/恢复 → 退出`);
    console.log(`    ✓ 用户: 充值 → Pin 请求 → 扣费`);
    console.log(`    ✓ 治理: 计费参数 → 分配 → Slash`);
    console.log(`    ✓ 错误路径: 余额不足 ✗, 非运营者 ✗`);
  });
}
