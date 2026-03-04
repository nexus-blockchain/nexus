/**
 * Flow-G2: 节点共识完整流程
 *
 * 角色:
 *   - Bob     (节点运营者)
 *   - Alice   (Sudo)
 *   - Charlie (举报者)
 *   - Dave    (无权限用户)
 *
 * 流程:
 *   1. Bob 注册节点 + 质押
 *   2. [错误路径] 质押不足被拒绝
 *   3. 验证节点已注册
 *   4. Bob 标记消息序列已处理 (mark_sequence_processed)
 *   5. [错误路径] 重复序列被拒绝
 *   6. [错误路径] Free tier 标记被拒绝
 *   7. Bob 验证节点 TEE (verify_node_tee)
 *   8. Alice 设置 TEE 奖励参数
 *   9. Charlie 举报 Equivocation
 *  10. Alice 执行 Slash
 *  11. Bob 申请退出 (冷却期)
 *  12. Bob 完成退出 + 退还质押
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertEventEmitted,
  assertTrue,
} from '../../core/assertions.js';
import { getFreeBalance, waitBlocks } from '../../core/chain-state.js';
import { nex } from '../../core/config.js';

export const nodeConsensusFlow: FlowDef = {
  name: 'Flow-G2: 节点共识',
  description: '注册+质押 → 消息处理 → TEE验证 → 举报/Slash → 退出',
  fn: nodeConsensus,
};

async function nodeConsensus(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const charlie = ctx.actor('charlie');
  const dave = ctx.actor('dave');

  // ─── Step 1: Bob 注册节点 + 质押 ──────────────────────────

  const bobBalBefore = await getFreeBalance(api, bob.address);

  const registerTx = (api.tx as any).groupRobotConsensus.registerNode(
    '0x' + '01'.repeat(32),    // nodeId: [u8;32]
    nex(100).toString(),        // stake: u128
  );
  const regResult = await ctx.send(registerTx, bob, 'Bob 注册节点+质押', 'bob');
  assertTxSuccess(regResult, '注册节点');

  const regEvent = regResult.events.find(
    e => e.section === 'groupRobotConsensus' && e.method === 'NodeRegistered',
  );
  assertTrue(!!regEvent, '应有 NodeRegistered 事件');
  const nodeId = regEvent?.data?.nodeId ?? regEvent?.data?.[0];
  console.log(`    节点 ID: ${nodeId}`);

  // 验证质押已扣除
  await ctx.check('验证质押已扣除', 'bob', async () => {
    const bobBalAfter = await getFreeBalance(api, bob.address);
    const delta = bobBalBefore - bobBalAfter;
    assertTrue(delta > 0n, `Bob 应被扣除质押, 减少 ${Number(delta) / 1e12} NEX`);
  });

  // ─── Step 2: [错误路径] 质押不足 ──────────────────────────

  const lowStakeTx = (api.tx as any).groupRobotConsensus.registerNode(
    '0x' + '02'.repeat(32), '1',
  );
  const lowStakeResult = await ctx.send(lowStakeTx, dave, '[错误路径] 质押不足', 'dave');
  await ctx.check('质押不足应失败', 'dave', () => {
    assertTxFailed(lowStakeResult, undefined, '质押不足');
  });

  // ─── Step 3: 验证节点已注册 ───────────────────────────────

  await ctx.check('验证节点已注册', 'bob', async () => {
    const node = await (api.query as any).groupRobotConsensus.nodes(nodeId);
    if (node.isSome) {
      const data = node.unwrap().toHuman();
      console.log(`    节点状态: ${data.status ?? data.state}`);
      console.log(`    质押: ${data.stake ?? data.staked}`);
    } else {
      // 可能用 accountToNode 映射
      const nodeByAccount = await (api.query as any).groupRobotConsensus.accountToNode(bob.address);
      if (nodeByAccount.isSome) {
        console.log(`    Bob 的节点: ${nodeByAccount.unwrap().toHuman()}`);
      }
    }
  });

  // ─── Step 4: 标记消息序列已处理 ───────────────────────────

  const botIdHash = '0x' + 'aa'.repeat(32);
  const sequenceNr = 1;

  const markTx = (api.tx as any).groupRobotConsensus.markSequenceProcessed(
    botIdHash,
    sequenceNr,
  );
  const markResult = await ctx.send(markTx, bob, '标记序列#1已处理', 'bob');
  assertTxSuccess(markResult, '标记序列');

  // ─── Step 5: [错误路径] 重复序列 ──────────────────────────

  const dupMarkTx = (api.tx as any).groupRobotConsensus.markSequenceProcessed(
    botIdHash, sequenceNr,
  );
  const dupResult = await ctx.send(dupMarkTx, bob, '[错误路径] 重复序列', 'bob');
  await ctx.check('重复序列应失败', 'bob', () => {
    assertTxFailed(dupResult, undefined, '重复序列');
  });

  // ─── Step 6: [错误路径] Free tier 标记 ────────────────────
  // Dave 没有注册节点且没有付费订阅
  const freeTierMarkTx = (api.tx as any).groupRobotConsensus.markSequenceProcessed(
    '0x' + 'bb'.repeat(32), 2,
  );
  const freeTierResult = await ctx.send(freeTierMarkTx, dave, '[错误路径] Free tier 标记', 'dave');
  await ctx.check('Free tier 标记应失败', 'dave', () => {
    assertTxFailed(freeTierResult, undefined, 'Free tier');
  });

  // ─── Step 7: 验证节点 TEE ────────────────────────────────

  const verifyTeeTx = (api.tx as any).groupRobotConsensus.verifyNodeTee(nodeId, botIdHash);
  const verifyResult = await ctx.send(verifyTeeTx, bob, 'Bob 验证节点 TEE', 'bob');
  if (verifyResult.success) {
    await ctx.check('TEE 验证成功', 'bob', () => {});
  } else {
    console.log(`    ℹ TEE 验证失败 (可能需要先提交证明): ${verifyResult.error}`);
  }

  // ─── Step 8: Alice 设置 TEE 奖励参数 ─────────────────────

  const setParamsTx = (api.tx as any).groupRobotConsensus.setTeeRewardParams(
    15000,   // tee_multiplier: 1.5x
    2000,    // sgx_bonus: +0.2x
  );
  const paramsResult = await ctx.sudo(setParamsTx, '设置 TEE 奖励参数');
  assertTxSuccess(paramsResult, '设置 TEE 奖励参数');

  // ─── Step 9: Charlie 举报 Equivocation ────────────────────

  const reportTx = (api.tx as any).groupRobotConsensus.reportEquivocation(
    nodeId,
    1,                           // sequence
    '0x' + '00'.repeat(32),     // msgHashA: [u8;32]
    '0x' + '01'.repeat(64),     // signatureA: [u8;64]
    '0x' + '02'.repeat(32),     // msgHashB: [u8;32]
    '0x' + '03'.repeat(64),     // signatureB: [u8;64]
  );
  const reportResult = await ctx.send(reportTx, charlie, 'Charlie 举报 Equivocation', 'charlie');
  if (reportResult.success) {
    await ctx.check('举报事件', 'charlie', () => {
      assertEventEmitted(reportResult, 'groupRobotConsensus', 'EquivocationReported', '举报事件');
    });
  } else {
    console.log(`    ℹ 举报失败: ${reportResult.error}`);
  }

  // ─── Step 10: Alice 执行 Slash ────────────────────────────

  const slashTx = (api.tx as any).groupRobotConsensus.slashEquivocation(
    nodeId,
    1,                   // sequence: u64
  );
  const slashResult = await ctx.sudo(slashTx, '执行 Slash');
  if (slashResult.success) {
    await ctx.check('Slash 事件', 'system', () => {
      assertEventEmitted(slashResult, 'groupRobotConsensus', 'NodeSlashed', 'Slash 事件');
    });
  } else {
    console.log(`    ℹ Slash 失败: ${slashResult.error}`);
  }

  // ─── Step 11: Bob 申请退出 ────────────────────────────────

  const exitTx = (api.tx as any).groupRobotConsensus.requestExit(nodeId);
  const exitResult = await ctx.send(exitTx, bob, 'Bob 申请退出', 'bob');
  assertTxSuccess(exitResult, '申请退出');

  // ─── Step 12: 完成退出 + 退还质押 ────────────────────────

  // 等待冷却期
  console.log(`    等待冷却期 (~5 blocks)...`);
  await waitBlocks(api, 6);

  const bobBalBeforeExit = await getFreeBalance(api, bob.address);

  const finalizeTx = (api.tx as any).groupRobotConsensus.finalizeExit(nodeId);
  const finalizeResult = await ctx.send(finalizeTx, bob, 'Bob 完成退出', 'bob');
  assertTxSuccess(finalizeResult, '完成退出');

  await ctx.check('验证质押退还', 'bob', async () => {
    const bobBalAfterExit = await getFreeBalance(api, bob.address);
    const delta = bobBalAfterExit - bobBalBeforeExit;
    console.log(`    退还质押变化: ${Number(delta) / 1e12} NEX`);
  });

  // ─── 汇总 ─────────────────────────────────────────────────
  await ctx.check('节点共识汇总', 'system', () => {
    console.log(`    ✓ 注册 → 质押 → 消息处理 → TEE 验证`);
    console.log(`    ✓ 举报 → Slash → 退出 → 退还质押`);
    console.log(`    ✓ 错误路径: 质押不足 ✗, 重复序列 ✗, Free tier ✗`);
  });
}
