/**
 * Flow-E11: Token/Governance 管理回归
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertEventEmitted,
  assertTxFailed,
  assertTxSuccess,
  assertTrue,
} from '../../core/assertions.js';

export const tokenGovernanceAdminFlow: FlowDef = {
  name: 'Flow-E11: Token/Governance 管理',
  description: 'approve/transfer_from + 分红取消 + 治理改票/暂停/否决/清理/强制解锁',
  fn: runTokenGovernanceAdminFlow,
};

async function runTokenGovernanceAdminFlow(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const eve = ctx.actor('eve');
  const bob = ctx.actor('bob');
  const charlie = ctx.actor('charlie');
  const dave = ctx.actor('dave');

  const { entityId } = await ensureFreshEntity(ctx);

  const createTokenResult = await ctx.send(
    (api.tx as any).entityToken.createShopToken(
      entityId,
      'E11 Governance Token',
      'E11GT',
      12,
      0,
      100,
    ),
    eve,
    '创建 E11 Token',
    'eve',
  );
  assertTxSuccess(createTokenResult, '创建 token');

  const mintBobResult = await ctx.send(
    (api.tx as any).entityToken.mintTokens(entityId, bob.address, 100_000),
    eve,
    '铸造给 Bob',
    'eve',
  );
  assertTxSuccess(mintBobResult, '铸造给 Bob');

  const mintEveResult = await ctx.send(
    (api.tx as any).entityToken.mintTokens(entityId, eve.address, 20_000),
    eve,
    '铸造给 Eve',
    'eve',
  );
  assertTxSuccess(mintEveResult, '铸造给 Eve');

  const mintCharlieResult = await ctx.send(
    (api.tx as any).entityToken.mintTokens(entityId, charlie.address, 20_000),
    eve,
    '铸造给 Charlie',
    'eve',
  );
  assertTxSuccess(mintCharlieResult, '铸造给 Charlie');

  const mintDaveResult = await ctx.send(
    (api.tx as any).entityToken.mintTokens(entityId, dave.address, 20_000),
    eve,
    '铸造给 Dave',
    'eve',
  );
  assertTxSuccess(mintDaveResult, '铸造给 Dave');

  const approveResult = await ctx.send(
    (api.tx as any).entityToken.approveTokens(entityId, charlie.address, 2_000),
    bob,
    'Bob 授权 Charlie 使用额度',
    'bob',
  );
  assertTxSuccess(approveResult, 'approve_tokens');
  await ctx.check('授权事件', 'bob', () => {
    assertEventEmitted(approveResult, 'entityToken', 'TokenApprovalSet', 'approve event');
  });

  const transferFromResult = await ctx.send(
    (api.tx as any).entityToken.transferFrom(entityId, bob.address, eve.address, 500),
    charlie,
    'Charlie 使用授权转账',
    'charlie',
  );
  assertTxSuccess(transferFromResult, 'transfer_from');
  await ctx.check('授权转账事件', 'charlie', () => {
    assertEventEmitted(transferFromResult, 'entityToken', 'TokensTransferredFrom', 'transfer_from event');
  });

  const changeTypeResult = await ctx.send(
    (api.tx as any).entityToken.changeTokenType(entityId, 'Hybrid'),
    eve,
    '切换为 Hybrid Token',
    'eve',
  );
  assertTxSuccess(changeTypeResult, '切换 token 类型');

  const configDividendResult = await ctx.send(
    (api.tx as any).entityToken.configureDividend(entityId, true, 0),
    eve,
    '启用分红',
    'eve',
  );
  assertTxSuccess(configDividendResult, '启用分红');

  const distributeDividendResult = await ctx.send(
    (api.tx as any).entityToken.distributeDividend(
      entityId,
      300,
      [
        [bob.address, 200],
        [eve.address, 100],
      ],
    ),
    eve,
    '分发待领取分红',
    'eve',
  );
  assertTxSuccess(distributeDividendResult, '分发分红');

  const cancelPendingDividendsResult = await ctx.sudo(
    (api.tx as any).entityToken.forceCancelPendingDividends(entityId, [bob.address, eve.address]),
    '强制取消待领取分红',
  );
  assertTxSuccess(cancelPendingDividendsResult, '强制取消待领取分红');
  await ctx.check('取消待领取分红事件', 'sudo(alice)', () => {
    assertEventEmitted(
      cancelPendingDividendsResult,
      'entityToken',
      'PendingDividendsCancelled',
      'force_cancel_pending_dividends',
    );
  });

  const governanceConfigResult = await ctx.send(
    (api.tx as any).entityGovernance.configureGovernance(
      entityId,
      'FullDao',
      14_400,
      2_400,
      1,
      1,
      100,
      true,
    ),
    eve,
    '配置 FullDAO',
    'eve',
  );
  assertTxSuccess(governanceConfigResult, '配置治理');

  const proposal1Id = await createPromotionProposal(ctx, entityId, bob, 'E11 提案-改票');
  const vote1Result = await ctx.send(
    (api.tx as any).entityGovernance.vote(proposal1Id, 'Yes'),
    bob,
    'Bob 首次投票',
    'bob',
  );
  assertTxSuccess(vote1Result, '首次投票');
  const changeVoteResult = await ctx.send(
    (api.tx as any).entityGovernance.changeVote(proposal1Id, 'Abstain'),
    bob,
    'Bob 修改投票为弃权',
    'bob',
  );
  assertTxSuccess(changeVoteResult, 'change_vote');
  await ctx.check('改票事件', 'bob', () => {
    assertEventEmitted(changeVoteResult, 'entityGovernance', 'VoteChanged', 'change_vote event');
  });

  const finalizeVotingResult = await ctx.send(
    (api.tx as any).entityGovernance.finalizeVoting(proposal1Id),
    bob,
    '[错误路径] 投票期未结束时结束投票',
    'bob',
  );
  await ctx.check('投票期未结束前结束投票失败', 'bob', () => {
    assertTxFailed(finalizeVotingResult, 'VotingNotEnded', 'finalize_voting');
  });

  const proposal2Id = await createPromotionProposal(ctx, entityId, charlie, 'E11 提案-批量取消', 'charlie');
  const pauseResult = await ctx.send(
    (api.tx as any).entityGovernance.pauseGovernance(entityId),
    eve,
    '暂停治理',
    'eve',
  );
  assertTxSuccess(pauseResult, 'pause_governance');
  await ctx.check('暂停治理事件', 'eve', () => {
    assertEventEmitted(pauseResult, 'entityGovernance', 'GovernancePausedEvent', 'pause event');
  });

  const resumeResult = await ctx.send(
    (api.tx as any).entityGovernance.resumeGovernance(entityId),
    eve,
    '恢复治理',
    'eve',
  );
  assertTxSuccess(resumeResult, 'resume_governance');
  await ctx.check('恢复治理事件', 'eve', () => {
    assertEventEmitted(resumeResult, 'entityGovernance', 'GovernanceResumedEvent', 'resume event');
  });

  const batchCancelResult = await ctx.send(
    (api.tx as any).entityGovernance.batchCancelProposals(entityId),
    eve,
    '批量取消活跃提案',
    'eve',
  );
  assertTxSuccess(batchCancelResult, 'batch_cancel_proposals');
  await ctx.check('批量取消事件', 'eve', () => {
    assertEventEmitted(batchCancelResult, 'entityGovernance', 'BatchProposalsCancelled', 'batch cancel');
  });

  const cleanupProposalResult = await ctx.send(
    (api.tx as any).entityGovernance.cleanupProposal(proposal2Id),
    bob,
    '清理终态提案',
    'bob',
  );
  assertTxSuccess(cleanupProposalResult, 'cleanup_proposal');

  const proposal3Id = await createPromotionProposal(ctx, entityId, dave, 'E11 提案-否决', 'dave');
  const vetoResult = await ctx.send(
    (api.tx as any).entityGovernance.vetoProposal(proposal3Id),
    eve,
    'Owner 否决提案',
    'eve',
  );
  assertTxSuccess(vetoResult, 'veto_proposal');
  await ctx.check('否决事件', 'eve', () => {
    assertEventEmitted(vetoResult, 'entityGovernance', 'ProposalVetoed', 'veto event');
  });

  const lockResult = await ctx.send(
    (api.tx as any).entityGovernance.lockGovernance(entityId),
    eve,
    '锁定治理配置',
    'eve',
  );
  assertTxSuccess(lockResult, '锁定治理');

  const forceUnlockResult = await ctx.sudo(
    (api.tx as any).entityGovernance.forceUnlockGovernance(entityId),
    '紧急强制解锁治理',
  );
  assertTxSuccess(forceUnlockResult, 'force_unlock_governance');
  await ctx.check('强制解锁事件', 'sudo(alice)', () => {
    assertEventEmitted(forceUnlockResult, 'entityGovernance', 'GovernanceForceUnlocked', 'force unlock');
  });
}

async function ensureFreshEntity(ctx: FlowContext): Promise<{ entityId: number }> {
  const { api } = ctx;
  const eve = ctx.actor('eve');
  const nextEntityId = (await (api.query as any).entityRegistry.nextEntityId()).toNumber();
  const createEntityResult = await ctx.send(
    (api.tx as any).entityRegistry.createEntity(
      `E11 Entity ${nextEntityId}`,
      null,
      `QmE11EntityDesc${nextEntityId}`,
      null,
    ),
    eve,
    '创建 E11 Entity',
    'eve',
  );
  assertTxSuccess(createEntityResult, '创建 E11 Entity');
  return { entityId: nextEntityId };
}

async function createPromotionProposal(
  ctx: FlowContext,
  entityId: number,
  signer: any,
  title: string,
  actorName: string = 'bob',
): Promise<number> {
  const { api } = ctx;
  const createProposalResult = await ctx.send(
    (api.tx as any).entityGovernance.createProposal(
      entityId,
      { Promotion: { discount_rate: 100, duration_blocks: 10 } },
      title,
      `Qm-${title.replace(/\s+/g, '-')}`,
    ),
    signer,
    title,
    actorName,
  );
  assertTxSuccess(createProposalResult, title);
  const proposalEvent = createProposalResult.events.find(
    e => e.section === 'entityGovernance' && e.method === 'ProposalCreated',
  );
  assertTrue(!!proposalEvent, '应产生 ProposalCreated');
  return Number(proposalEvent?.data?.proposalId ?? proposalEvent?.data?.[0]);
}
