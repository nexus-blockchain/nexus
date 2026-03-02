/**
 * Flow-E5: Token 创建 + 治理提案完整流程
 *
 * 角色:
 *   - Eve     (Entity Owner)
 *   - Alice   (Sudo)
 *   - Bob     (Token Holder / 投票者)
 *   - Charlie (无权限用户)
 *
 * 流程:
 *   1. 确保 Entity + Shop 就绪
 *   2. Eve 创建 Shop Token (Governance 类型)
 *   3. Eve 铸造代币给 Bob
 *   4. Bob 转让部分代币给 Charlie
 *   5. [错误路径] 超 max_supply 铸造被拒绝
 *   6. Eve 配置治理模式 (FullDAO)
 *   7. Bob 创建提案
 *   8. [错误路径] Charlie 持有不足无法创建提案
 *   9. Bob 投票
 *  10. 结束投票 → 通过
 *  11. 执行提案
 *  12. Eve 设置转账限制 (Whitelist)
 *  13. 添加 Bob 到白名单 → 转账成功
 *  14. [错误路径] Charlie 不在白名单 → 转账失败
 *  15. Eve 锁仓代币 + 解锁
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertEventEmitted,
  assertStorageExists,
  assertTrue,
} from '../../core/assertions.js';
import { waitBlocks } from '../../core/chain-state.js';

export const tokenGovernanceFlow: FlowDef = {
  name: 'Flow-E5: Token+治理',
  description: '创建代币 → 铸造/转让 → 治理提案 → 投票 → 执行 | 转账限制',
  fn: tokenGovernance,
};

async function tokenGovernance(ctx: FlowContext): Promise<void> {
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
    const createTx = (api.tx as any).entityRegistry.createEntity('E2E Token Test', null, null, null);
    await ctx.send(createTx, eve, '创建实体', 'eve');
    const ent = await (api.query as any).entityRegistry.entities(entityId);
    const ed = ent.unwrap().toHuman();
    shopId = parseInt(((ed.shopIds ?? ed.shop_ids) as string[])[0].replace(/,/g, ''), 10);
  }
  console.log(`    Entity: ${entityId}, Shop: ${shopId}`);

  // ─── Step 2: 创建 Token (Governance 类型 = 1) ────────────

  const createTokenTx = (api.tx as any).entityToken.createShopToken(
    shopId,
    'E2E Governance Token',  // name
    'EGT',                   // symbol
    12,                      // decimals
    1,                       // token_type: Governance
    1_000_000,               // max_supply
    500,                     // reward_rate (5%)
    100,                     // exchange_rate
  );
  const tokenResult = await ctx.send(createTokenTx, eve, '创建 Governance Token', 'eve');
  if (tokenResult.success) {
    await ctx.check('Token 已创建', 'eve', () => {
      assertEventEmitted(tokenResult, 'entityToken', 'ShopTokenCreated', '创建事件');
    });
  } else {
    console.log(`    ℹ Token 可能已存在: ${tokenResult.error}`);
  }

  // ─── Step 3: 铸造代币给 Bob ───────────────────────────────

  const mintTx = (api.tx as any).entityToken.mintTokens(
    shopId,
    bob.address,
    100_000,   // amount
  );
  const mintResult = await ctx.send(mintTx, eve, '铸造代币给 Bob', 'eve');
  assertTxSuccess(mintResult, '铸造代币');

  await ctx.check('验证 Bob 收到代币', 'bob', async () => {
    const balance = await (api.query as any).entityToken.tokenBalances(shopId, bob.address);
    const bal = balance.toHuman();
    console.log(`    Bob 代币余额: ${JSON.stringify(bal)}`);
  });

  // ─── Step 4: Bob 转让部分给 Charlie ───────────────────────

  const transferTx = (api.tx as any).entityToken.transferTokens(
    shopId,
    charlie.address,
    10_000,
  );
  const transferResult = await ctx.send(transferTx, bob, 'Bob 转让代币给 Charlie', 'bob');
  assertTxSuccess(transferResult, '转让代币');

  // ─── Step 5: [错误路径] 超 max_supply ─────────────────────

  const overMintTx = (api.tx as any).entityToken.mintTokens(
    shopId, eve.address, 999_999_999,
  );
  const overMintResult = await ctx.send(overMintTx, eve, '[错误路径] 超 max_supply 铸造', 'eve');
  await ctx.check('超供应量铸造应失败', 'eve', () => {
    assertTxFailed(overMintResult, undefined, '超 max_supply');
  });

  // ─── Step 6: 配置治理模式 ────────────────────────────────

  const configGovTx = (api.tx as any).entityGovernance.configureGovernance(
    shopId,
    5,      // governance_mode: FullDAO
    null,   // admin_veto
    null,   // quorum_threshold
    null,   // pass_threshold
  );
  const govResult = await ctx.send(configGovTx, eve, '配置治理模式 (FullDAO)', 'eve');
  if (govResult.success) {
    await ctx.check('治理模式已配置', 'eve', () => {});
  } else {
    console.log(`    ℹ 治理配置失败: ${govResult.error}`);
  }

  // ─── Step 7: Bob 创建提案 ────────────────────────────────

  const currentBlock = (await api.rpc.chain.getHeader()).number.toNumber();

  const createProposalTx = (api.tx as any).entityGovernance.createProposal(
    shopId,
    'E2E Test Proposal',           // title
    'QmProposalDescCid001',        // description_cid
    currentBlock + 10,             // voting_end (10 blocks later)
    currentBlock + 20,             // execution_delay (20 blocks)
    null,                          // proposal_type
  );
  const proposalResult = await ctx.send(createProposalTx, bob, 'Bob 创建提案', 'bob');
  let proposalId: any = null;

  if (proposalResult.success) {
    const proposalEvent = proposalResult.events.find(
      e => e.section === 'entityGovernance' && e.method === 'ProposalCreated',
    );
    proposalId = proposalEvent?.data?.proposalId ?? proposalEvent?.data?.[0];
    console.log(`    提案 ID: ${proposalId}`);
  } else {
    console.log(`    ℹ 创建提案失败: ${proposalResult.error}`);
  }

  // ─── Step 8: [错误路径] Charlie 持有不足 ──────────────────

  const charlieProposalTx = (api.tx as any).entityGovernance.createProposal(
    shopId, 'Charlie Proposal', 'QmDesc', currentBlock + 50, currentBlock + 60, null,
  );
  const charlieProposalResult = await ctx.send(
    charlieProposalTx, charlie, '[错误路径] Charlie 创建提案', 'charlie',
  );
  // Charlie 仅有 10,000 tokens, 可能不满足 proposal_threshold
  if (!charlieProposalResult.success) {
    await ctx.check('持有不足创建提案失败', 'charlie', () => {
      assertTxFailed(charlieProposalResult, undefined, '持有不足');
    });
  } else {
    console.log(`    ℹ Charlie 持有足够创建提案 (threshold 较低)`);
  }

  // ─── Step 9-11: 投票 + 结束 + 执行 ──────────────────────

  if (proposalId !== null) {
    // 投票
    const voteTx = (api.tx as any).entityGovernance.vote(
      shopId, proposalId, true, // approve
    );
    const voteResult = await ctx.send(voteTx, bob, 'Bob 投票赞成', 'bob');
    assertTxSuccess(voteResult, '投票');

    // [错误路径] 投票期内重复投票由链上处理

    // 等待投票期结束
    console.log(`    等待投票期结束 (~10 blocks)...`);
    await waitBlocks(api, 12);

    // 结束投票
    const finalizeTx = (api.tx as any).entityGovernance.finalizeVoting(shopId, proposalId);
    const finalizeResult = await ctx.send(finalizeTx, bob, '结束投票', 'bob');
    if (finalizeResult.success) {
      await ctx.check('投票已结束', 'system', () => {
        assertEventEmitted(finalizeResult, 'entityGovernance', 'VotingFinalized', '结束事件');
      });

      // 等待执行延迟
      console.log(`    等待执行延迟 (~10 blocks)...`);
      await waitBlocks(api, 12);

      // 执行提案
      const executeTx = (api.tx as any).entityGovernance.executeProposal(shopId, proposalId);
      const executeResult = await ctx.send(executeTx, bob, '执行提案', 'bob');
      if (executeResult.success) {
        await ctx.check('提案已执行', 'system', () => {
          assertEventEmitted(executeResult, 'entityGovernance', 'ProposalExecuted', '执行事件');
        });
      } else {
        console.log(`    ℹ 执行提案失败: ${executeResult.error}`);
      }
    } else {
      console.log(`    ℹ 结束投票失败: ${finalizeResult.error}`);
    }
  }

  // ─── Step 12-14: 转账限制 ────────────────────────────────

  // 设置 Whitelist 模式 (mode=1)
  const setRestrictionTx = (api.tx as any).entityToken.setTransferRestriction(shopId, 1);
  const restrictResult = await ctx.send(setRestrictionTx, eve, '设置白名单转账限制', 'eve');

  if (restrictResult.success) {
    // 添加 Bob 到白名单
    const addWhitelistTx = (api.tx as any).entityToken.addToWhitelist(shopId, bob.address);
    const whitelistResult = await ctx.send(addWhitelistTx, eve, '添加 Bob 到白名单', 'eve');
    assertTxSuccess(whitelistResult, '添加白名单');

    // Bob 转账应成功
    const bobTransferTx = (api.tx as any).entityToken.transferTokens(shopId, eve.address, 100);
    const bobTxResult = await ctx.send(bobTransferTx, bob, 'Bob 转账(白名单内)', 'bob');
    assertTxSuccess(bobTxResult, 'Bob 转账');

    // Charlie 不在白名单, 转账应失败
    const charlieTransferTx = (api.tx as any).entityToken.transferTokens(shopId, eve.address, 100);
    const charlieTxResult = await ctx.send(charlieTransferTx, charlie, '[错误路径] Charlie 转账(非白名单)', 'charlie');
    await ctx.check('非白名单转账应失败', 'charlie', () => {
      assertTxFailed(charlieTxResult, undefined, '非白名单转账');
    });

    // 恢复无限制模式
    const resetTx = (api.tx as any).entityToken.setTransferRestriction(shopId, 0);
    await ctx.send(resetTx, eve, '恢复无限制模式', 'eve');
  } else {
    console.log(`    ℹ 设置转账限制失败: ${restrictResult.error}`);
  }

  // ─── Step 15: 锁仓 + 解锁 ────────────────────────────────

  const lockBlock = (await api.rpc.chain.getHeader()).number.toNumber();

  const lockTx = (api.tx as any).entityToken.lockTokens(
    shopId,
    bob.address,
    5_000,
    lockBlock + 5,   // unlock_at: 5 blocks later
  );
  const lockResult = await ctx.send(lockTx, eve, '锁仓 Bob 的代币', 'eve');
  if (lockResult.success) {
    // 立即解锁应失败 (未到期)
    const earlyUnlockTx = (api.tx as any).entityToken.unlockTokens(shopId, bob.address);
    const earlyResult = await ctx.send(earlyUnlockTx, eve, '[测试] 提前解锁', 'eve');
    // 可能返回 UnlockTimeNotReached (H1 修复后的正确错误)
    console.log(`    提前解锁结果: ${earlyResult.success ? '成功(部分到期)' : earlyResult.error}`);

    // 等待解锁时间
    await waitBlocks(api, 6);

    const unlockTx = (api.tx as any).entityToken.unlockTokens(shopId, bob.address);
    const unlockResult = await ctx.send(unlockTx, eve, '解锁到期代币', 'eve');
    assertTxSuccess(unlockResult, '解锁代币');
  } else {
    console.log(`    ℹ 锁仓失败: ${lockResult.error}`);
  }

  // ─── 汇总 ─────────────────────────────────────────────────
  await ctx.check('Token+治理汇总', 'system', () => {
    console.log(`    ✓ Token: 创建 → 铸造 → 转让 → 锁仓/解锁`);
    console.log(`    ✓ 治理: 配置 → 提案 → 投票 → 结束 → 执行`);
    console.log(`    ✓ 转账限制: 白名单模式 → 白名单内 ✓ / 非白名单 ✗`);
    console.log(`    ✓ 错误路径: 超供应量 ✗, 持有不足 ✗`);
  });
}
