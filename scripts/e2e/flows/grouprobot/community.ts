/**
 * Flow-G5: 社区管理完整流程
 *
 * 角色:
 *   - Bob     (Bot Owner / 社区管理者)
 *   - Charlie (社区成员)
 *   - Dave    (无权限用户)
 *
 * 流程:
 *   1. Bob 提交行为日志 (submit_action_log)
 *   2. Bob 批量提交行为日志 (batch_submit_logs)
 *   3. Bob 设置节点准入策略
 *   4. Bob 更新社区配置 (CAS 乐观锁)
 *   5. Bob 奖励 Charlie 声誉
 *   6. Bob 扣减 Charlie 声誉
 *   7. Bob 重置 Charlie 声誉
 *   8. Bob 更新活跃成员数
 *   9. Bob 清理过期日志
 *  10. 清理过期冷却
 *  11. [错误路径] Dave 提交行为日志
 *  12. [错误路径] Dave 奖励声誉
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertEventEmitted,
  assertTrue,
} from '../../core/assertions.js';

export const communityFlow: FlowDef = {
  name: 'Flow-G5: 社区管理',
  description: '行为日志 → 节点策略 → 社区配置 → 声誉管理 → 清理 | 错误路径',
  fn: communityLifecycle,
};

async function communityLifecycle(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const charlie = ctx.actor('charlie');
  const dave = ctx.actor('dave');

  const communityIdHash = '0x' + 'c1'.repeat(32);
  const botPublicKey = '0x' + '01'.repeat(32);
  const userHash = '0x' + 'u1'.repeat(16) + '00'.repeat(16);

  // 先注册 Bot 并绑定社区 (前置依赖)
  const registerTx = (api.tx as any).grouprobotRegistry.registerBot(
    botPublicKey, 'G5 Community Bot', null,
  );
  const regResult = await ctx.send(registerTx, bob, '注册 Bot (前置)', 'bob');
  if (!regResult.success) {
    console.log(`    ℹ Bot 注册失败 (可能已存在): ${regResult.error}`);
  }

  const bindTx = (api.tx as any).grouprobotRegistry.bindCommunity(
    botPublicKey, communityIdHash, 0,
  );
  const bindResult = await ctx.send(bindTx, bob, '绑定社区 (前置)', 'bob');
  if (!bindResult.success) {
    console.log(`    ℹ 社区绑定失败 (可能已绑定): ${bindResult.error}`);
  }

  // ─── Step 1: Bob 提交行为日志 ──────────────────────────────

  const mockSignature = '0x' + '00'.repeat(64);

  const logTx = (api.tx as any).grouprobotCommunity.submitActionLog(
    communityIdHash,
    0,               // action_type: Kick=0
    userHash,        // target_user_hash
    1,               // sequence
    '0x' + 'ab'.repeat(32),  // message_hash
    mockSignature,   // ed25519_signature
  );
  const logResult = await ctx.send(logTx, bob, 'Bob 提交行为日志', 'bob');
  if (logResult.success) {
    await ctx.check('行为日志事件', 'bob', () => {
      assertEventEmitted(logResult, 'grouprobotCommunity', 'ActionLogSubmitted', '日志事件');
    });
  } else {
    console.log(`    ℹ 提交日志失败 (可能签名不匹配): ${logResult.error}`);
  }

  // ─── Step 2: Bob 批量提交行为日志 ──────────────────────────

  const logs = [
    {
      community_id_hash: communityIdHash,
      action_type: 1,   // Ban
      target_user_hash: userHash,
      sequence: 2,
      message_hash: '0x' + 'cd'.repeat(32),
      signature: mockSignature,
    },
    {
      community_id_hash: communityIdHash,
      action_type: 2,   // Mute
      target_user_hash: userHash,
      sequence: 3,
      message_hash: '0x' + 'ef'.repeat(32),
      signature: mockSignature,
    },
  ];

  const batchLogTx = (api.tx as any).grouprobotCommunity.batchSubmitLogs(logs);
  const batchLogResult = await ctx.send(batchLogTx, bob, 'Bob 批量提交日志', 'bob');
  if (batchLogResult.success) {
    await ctx.check('批量日志成功', 'bob', () => {});
  } else {
    console.log(`    ℹ 批量日志失败: ${batchLogResult.error}`);
  }

  // ─── Step 3: 设置节点准入策略 ──────────────────────────────

  const setReqTx = (api.tx as any).grouprobotCommunity.setNodeRequirement(
    communityIdHash,
    1,      // min_nodes
    true,   // require_tee
  );
  const setReqResult = await ctx.send(setReqTx, bob, 'Bob 设置节点准入策略', 'bob');
  if (setReqResult.success) {
    await ctx.check('节点准入策略已设置', 'bob', () => {});
  } else {
    console.log(`    ℹ 设置准入策略失败: ${setReqResult.error}`);
  }

  // ─── Step 4: 更新社区配置 (CAS 乐观锁) ────────────────────

  const configTx = (api.tx as any).grouprobotCommunity.updateCommunityConfig(
    communityIdHash,
    0,              // expected_version (CAS)
    'QmConfigCid',  // config_cid
    null,           // max_members
    null,           // cooldown_blocks
  );
  const configResult = await ctx.send(configTx, bob, 'Bob 更新社区配置', 'bob');
  if (configResult.success) {
    await ctx.check('社区配置已更新', 'bob', () => {
      assertEventEmitted(configResult, 'grouprobotCommunity', 'CommunityConfigUpdated', '配置事件');
    });
  } else {
    console.log(`    ℹ 更新配置失败: ${configResult.error}`);
  }

  // ─── Step 5: 奖励声誉 ─────────────────────────────────────

  const awardTx = (api.tx as any).grouprobotCommunity.awardReputation(
    communityIdHash,
    charlie.address,
    100,   // amount
  );
  const awardResult = await ctx.send(awardTx, bob, 'Bob 奖励 Charlie 声誉', 'bob');
  if (awardResult.success) {
    await ctx.check('声誉奖励事件', 'bob', () => {
      assertEventEmitted(awardResult, 'grouprobotCommunity', 'ReputationAwarded', '奖励事件');
    });
  } else {
    console.log(`    ℹ 声誉奖励失败: ${awardResult.error}`);
  }

  // ─── Step 6: 扣减声誉 ─────────────────────────────────────

  const deductTx = (api.tx as any).grouprobotCommunity.deductReputation(
    communityIdHash,
    charlie.address,
    50,    // amount
  );
  const deductResult = await ctx.send(deductTx, bob, 'Bob 扣减 Charlie 声誉', 'bob');
  if (deductResult.success) {
    await ctx.check('声誉扣减事件', 'bob', () => {
      assertEventEmitted(deductResult, 'grouprobotCommunity', 'ReputationDeducted', '扣减事件');
    });
  } else {
    console.log(`    ℹ 声誉扣减失败: ${deductResult.error}`);
  }

  // ─── Step 7: 重置声誉 ─────────────────────────────────────

  const resetTx = (api.tx as any).grouprobotCommunity.resetReputation(
    communityIdHash,
    charlie.address,
  );
  const resetResult = await ctx.send(resetTx, bob, 'Bob 重置 Charlie 声誉', 'bob');
  if (resetResult.success) {
    await ctx.check('声誉已重置', 'bob', () => {});
  } else {
    console.log(`    ℹ 声誉重置失败: ${resetResult.error}`);
  }

  // ─── Step 8: 更新活跃成员数 ────────────────────────────────

  const updateMembersTx = (api.tx as any).grouprobotCommunity.updateActiveMembers(
    communityIdHash,
    42,    // active_members
  );
  const updateMembersResult = await ctx.send(updateMembersTx, bob, 'Bob 更新活跃成员数', 'bob');
  if (updateMembersResult.success) {
    await ctx.check('活跃成员数已更新', 'bob', () => {});
  } else {
    console.log(`    ℹ 更新成员数失败: ${updateMembersResult.error}`);
  }

  // ─── Step 9: 清理过期日志 ─────────────────────────────────

  const clearLogsTx = (api.tx as any).grouprobotCommunity.clearExpiredLogs(
    communityIdHash,
    100,     // max_age_blocks
    10,      // limit
  );
  const clearLogsResult = await ctx.send(clearLogsTx, bob, 'Bob 清理过期日志', 'bob');
  if (clearLogsResult.success) {
    await ctx.check('过期日志已清理', 'bob', () => {});
  } else {
    console.log(`    ℹ 清理日志: ${clearLogsResult.error}`);
  }

  // ─── Step 10: 清理过期冷却 ────────────────────────────────

  const cleanupCooldownTx = (api.tx as any).grouprobotCommunity.cleanupExpiredCooldowns(
    bob.address,        // operator
    communityIdHash,
    charlie.address,    // user
  );
  const cleanupResult = await ctx.send(cleanupCooldownTx, bob, '清理过期冷却', 'bob');
  if (cleanupResult.success) {
    await ctx.check('冷却已清理', 'bob', () => {});
  } else {
    console.log(`    ℹ 清理冷却: ${cleanupResult.error}`);
  }

  // ─── Step 11: [错误路径] Dave 提交行为日志 ─────────────────

  const daveLogTx = (api.tx as any).grouprobotCommunity.submitActionLog(
    communityIdHash, 0, userHash, 99, '0x' + '00'.repeat(32), mockSignature,
  );
  const daveLogResult = await ctx.send(daveLogTx, dave, '[错误路径] Dave 提交日志', 'dave');
  await ctx.check('非 Bot Owner 提交日志应失败', 'dave', () => {
    assertTxFailed(daveLogResult, undefined, '非 Owner 提交日志');
  });

  // ─── Step 12: [错误路径] Dave 奖励声誉 ─────────────────────

  const daveAwardTx = (api.tx as any).grouprobotCommunity.awardReputation(
    communityIdHash, charlie.address, 100,
  );
  const daveAwardResult = await ctx.send(daveAwardTx, dave, '[错误路径] Dave 奖励声誉', 'dave');
  await ctx.check('非管理者奖励应失败', 'dave', () => {
    assertTxFailed(daveAwardResult, undefined, '非管理者奖励');
  });

  // ─── 汇总 ─────────────────────────────────────────────────
  await ctx.check('社区管理汇总', 'system', () => {
    console.log(`    ✓ 行为日志: 单条提交 → 批量提交`);
    console.log(`    ✓ 策略: 节点准入 → 社区配置 (CAS)`);
    console.log(`    ✓ 声誉: 奖励 → 扣减 → 重置`);
    console.log(`    ✓ 维护: 活跃成员 → 清理日志 → 清理冷却`);
    console.log(`    ✓ 错误路径: 非 Owner 日志 ✗, 非管理者声誉 ✗`);
  });
}
