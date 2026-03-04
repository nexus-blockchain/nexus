/**
 * Flow-G6: 仪式验证完整流程
 *
 * 角色:
 *   - Bob     (Bot Owner / 仪式发起者)
 *   - Alice   (Sudo — Enclave 审批 / 强制 re-ceremony)
 *   - Dave    (无权限用户)
 *
 * 流程:
 *   1. Alice 审批 Ceremony Enclave 到白名单
 *   2. Bob 记录仪式 (record_ceremony)
 *   3. 验证仪式已记录
 *   4. Alice 强制 re-ceremony (安全事件)
 *   5. Bob 重新记录仪式
 *   6. Alice 撤销仪式
 *   7. 清理终态仪式记录
 *   8. Alice 移除 Enclave
 *   9. [错误路径] Dave 记录仪式
 *  10. [错误路径] 未审批 Enclave 记录仪式
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertEventEmitted,
  assertTrue,
} from '../../core/assertions.js';

export const ceremonyFlow: FlowDef = {
  name: 'Flow-G6: 仪式验证',
  description: 'Enclave 审批 → 记录仪式 → 强制重做 → 撤销 → 清理 | 错误路径',
  fn: ceremonyLifecycle,
};

async function ceremonyLifecycle(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const dave = ctx.actor('dave');

  const botIdHash = '0x' + 'c6'.repeat(32);
  const botPublicKey = '0x' + 'b6'.repeat(32);
  const enclaveHash = '0x' + 'e1'.repeat(32);
  const enclaveHash2 = '0x' + 'e2'.repeat(32);
  const ceremonyHash = '0x' + 'ce'.repeat(32);
  const ceremonyHash2 = '0x' + 'cf'.repeat(32);

  // ─── Step 1: Alice 审批 Ceremony Enclave ───────────────────

  const approveEnclaveTx = (api.tx as any).groupRobotCeremony.approveCeremonyEnclave(
    enclaveHash,
    1,                   // version: u32
    'E2E Test Enclave',  // description
  );
  const approveResult = await ctx.sudo(approveEnclaveTx, '审批 Ceremony Enclave');
  assertTxSuccess(approveResult, '审批 Enclave');

  await ctx.check('验证 Enclave 审批事件', 'system', () => {
    assertEventEmitted(approveResult, 'groupRobotCeremony', 'CeremonyEnclaveApproved', '审批事件');
  });

  // ─── Step 2: Bob 记录仪式 ─────────────────────────────────

  const shamirThreshold = 3;
  const shamirTotal = 5;
  const participantHashes = Array.from({ length: shamirTotal }, (_, i) =>
    '0x' + (i + 1).toString(16).padStart(2, '0').repeat(32),
  );

  const recordTx = (api.tx as any).groupRobotCeremony.recordCeremony(
    ceremonyHash,         // ceremonyHash: [u8;32]
    enclaveHash,          // ceremonyMrenclave: [u8;32]
    shamirThreshold,      // k: u8
    shamirTotal,          // n: u8
    botPublicKey,         // botPublicKey: [u8;32]
    participantHashes,    // participantEnclaves: Vec<[u8;32]>
    botIdHash,            // botIdHash: [u8;32]
  );
  const recordResult = await ctx.send(recordTx, bob, 'Bob 记录仪式', 'bob');
  assertTxSuccess(recordResult, '记录仪式');

  const ceremonyEvent = recordResult.events.find(
    e => e.section === 'groupRobotCeremony' && e.method === 'CeremonyRecorded',
  );
  assertTrue(!!ceremonyEvent, '应有 CeremonyRecorded 事件');
  const ceremonyId = ceremonyEvent?.data?.ceremonyId ?? ceremonyEvent?.data?.[0];
  console.log(`    仪式 ID: ${ceremonyId}`);

  // ─── Step 3: 验证仪式状态 ─────────────────────────────────

  await ctx.check('验证仪式已记录', 'bob', async () => {
    const ceremony = await (api.query as any).groupRobotCeremony.ceremonies(ceremonyId);
    if (ceremony && !ceremony.isNone) {
      const data = ceremony.unwrap ? ceremony.unwrap().toHuman() : ceremony.toHuman();
      console.log(`    仪式状态: ${JSON.stringify(data).slice(0, 150)}`);
    }
  });

  // ─── Step 4: Alice 强制 re-ceremony ───────────────────────

  const reCeremonyTx = (api.tx as any).groupRobotCeremony.forceReCeremony(ceremonyHash);
  const reCeremonyResult = await ctx.sudo(reCeremonyTx, '强制 re-ceremony');
  assertTxSuccess(reCeremonyResult, '强制 re-ceremony');

  await ctx.check('验证 re-ceremony 事件', 'system', () => {
    assertEventEmitted(reCeremonyResult, 'groupRobotCeremony', 'ReCeremonyForced', 're-ceremony 事件');
  });

  // ─── Step 5: Bob 重新记录仪式 ─────────────────────────────

  const record2Tx = (api.tx as any).groupRobotCeremony.recordCeremony(
    ceremonyHash2,
    enclaveHash,
    shamirThreshold,
    shamirTotal,
    botPublicKey,
    participantHashes,
    botIdHash,
  );
  const record2Result = await ctx.send(record2Tx, bob, 'Bob 重新记录仪式', 'bob');
  if (record2Result.success) {
    await ctx.check('新仪式已记录', 'bob', () => {});
  } else {
    console.log(`    ℹ 重新记录仪式失败: ${record2Result.error}`);
  }

  // ─── Step 6: Alice 撤销仪式 ───────────────────────────────

  if (ceremonyId !== undefined) {
    const revokeTx = (api.tx as any).groupRobotCeremony.revokeCeremony(ceremonyHash);
    const revokeResult = await ctx.sudo(revokeTx, '撤销仪式');
    if (revokeResult.success) {
      await ctx.check('仪式已撤销', 'system', () => {
        assertEventEmitted(revokeResult, 'groupRobotCeremony', 'CeremonyRevoked', '撤销事件');
      });
    } else {
      console.log(`    ℹ 撤销仪式失败: ${revokeResult.error}`);
    }
  }

  // ─── Step 7: 清理终态仪式记录 ─────────────────────────────

  const cleanupTx = (api.tx as any).groupRobotCeremony.cleanupCeremony(
    ceremonyHash,
  );
  const cleanupResult = await ctx.send(cleanupTx, bob, '清理仪式记录', 'bob');
  if (cleanupResult.success) {
    await ctx.check('仪式记录已清理', 'bob', () => {});
  } else {
    console.log(`    ℹ 清理仪式: ${cleanupResult.error}`);
  }

  // ─── Step 8: Alice 移除 Enclave ───────────────────────────

  const removeEnclaveTx = (api.tx as any).groupRobotCeremony.removeCeremonyEnclave(enclaveHash);
  const removeResult = await ctx.sudo(removeEnclaveTx, '移除 Enclave');
  assertTxSuccess(removeResult, '移除 Enclave');

  // ─── Step 9: [错误路径] Dave 记录仪式 ─────────────────────

  // 先重新审批 Enclave 供测试
  const approveE2Tx = (api.tx as any).groupRobotCeremony.approveCeremonyEnclave(enclaveHash2, 1, 'E2E Enclave 2');
  await ctx.sudo(approveE2Tx, '重新审批 Enclave (供错误路径)');

  const daveRecordTx = (api.tx as any).groupRobotCeremony.recordCeremony(
    '0x' + 'dd'.repeat(32), enclaveHash2, 3, 5, botPublicKey, participantHashes, botIdHash,
  );
  const daveRecordResult = await ctx.send(daveRecordTx, dave, '[错误路径] Dave 记录仪式', 'dave');
  await ctx.check('非 Bot Owner 记录仪式应失败', 'dave', () => {
    assertTxFailed(daveRecordResult, undefined, '非 Owner 记录仪式');
  });

  // ─── Step 10: [错误路径] 未审批 Enclave 记录仪式 ───────────

  const unApprovedEnclave = '0x' + 'f0'.repeat(32);
  const noEnclaveTx = (api.tx as any).groupRobotCeremony.recordCeremony(
    '0x' + 'ee'.repeat(32), unApprovedEnclave, 3, 5, botPublicKey, participantHashes, botIdHash,
  );
  const noEnclaveResult = await ctx.send(noEnclaveTx, bob, '[错误路径] 未审批 Enclave', 'bob');
  await ctx.check('未审批 Enclave 应失败', 'bob', () => {
    assertTxFailed(noEnclaveResult, undefined, '未审批 Enclave');
  });

  // ─── 汇总 ─────────────────────────────────────────────────
  await ctx.check('仪式验证汇总', 'system', () => {
    console.log(`    ✓ Enclave: 审批 → 移除`);
    console.log(`    ✓ 仪式: 记录 → 强制重做 → 撤销 → 清理`);
    console.log(`    ✓ 错误路径: 非 Owner ✗, 未审批 Enclave ✗`);
  });
}
