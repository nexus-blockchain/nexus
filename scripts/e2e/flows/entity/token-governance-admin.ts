/**
 * Flow-E11: Token/Governance 管理回归
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxFailed,
  assertTxSuccess,
} from '../../core/assertions.js';

const ENTITY_TOKEN_ASSET_OFFSET = 1_000_000;

export const tokenGovernanceAdminFlow: FlowDef = {
  name: 'Flow-E11: Token/Governance 管理',
  description: 'approve/transfer_from + 全局暂停开关/冻结 + 分红取消 + 治理改票/暂停/否决/清理/强制解锁',
  fn: runTokenGovernanceAdminFlow,
};

async function runTokenGovernanceAdminFlow(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const allActors = {
    eve: ctx.actor('eve'),
    bob: ctx.actor('bob'),
    charlie: ctx.actor('charlie'),
    dave: ctx.actor('dave'),
  };

  const globalPauseBefore = await (api.query as any).entityToken.globalTokenPaused();
  if (globalPauseBefore.isTrue || globalPauseBefore.toString() === 'true') {
    const clearGlobalPauseResult = await ctx.sudo(
      (api.tx as any).entityToken.setGlobalTokenPause(false),
      '[兼容清理] 解除遗留全局代币暂停',
    );
    assertTxSuccess(clearGlobalPauseResult, '解除遗留全局代币暂停');
  }

  const { entityId, owner, ownerName } = await ensureFreshEntity(ctx);
  const participants = Object.entries(allActors)
    .filter(([name]) => name !== ownerName)
    .map(([name, account]) => ({ name, account }));
  const [holderA, holderB, holderC] = participants;

  const createTokenResult = await ctx.send(
    (api.tx as any).entityToken.createShopToken(
      entityId,
      'E11 Governance Token',
      'E11GT',
      12,
      0,
      100,
    ),
    owner,
    '创建 E11 Token',
    ownerName,
  );
  if (createTokenResult.success) {
    await ctx.check('E11 Token 已创建', ownerName, async () => {
      const config = await (api.query as any).entityToken.entityTokenConfigs(entityId);
      if (hasStorageValue(config)) {
        return;
      }

      const assetId = ENTITY_TOKEN_ASSET_OFFSET + entityId;
      const asset = await (api.query as any).assets.asset(assetId);
      if (!hasStorageValue(asset)) {
        throw new Error(`Token 配置/资产均未写入: entityId=${entityId} assetId=${assetId}`);
      }

      console.log(`    [E11] entityTokenConfigs 未写入，回退到 assets.asset(${assetId}) 作为兼容校验`);
    });
  } else {
    await ctx.check('复用已有 E11 Token', ownerName, () => {
      const error = createTokenResult.error ?? '';
      if (!error.includes('TokenAlreadyExists')) {
        throw new Error(`创建 token 失败: ${error}`);
      }
    });
  }

  const mintBobResult = await ctx.send(
    (api.tx as any).entityToken.mintTokens(entityId, holderA.account.address, 100_000),
    owner,
    `铸造给 ${holderA.name}`,
    ownerName,
  );
  assertTxSuccess(mintBobResult, `铸造给 ${holderA.name}`);

  const mintEveResult = await ctx.send(
    (api.tx as any).entityToken.mintTokens(entityId, holderB.account.address, 20_000),
    owner,
    `铸造给 ${holderB.name}`,
    ownerName,
  );
  assertTxSuccess(mintEveResult, `铸造给 ${holderB.name}`);

  const mintCharlieResult = await ctx.send(
    (api.tx as any).entityToken.mintTokens(entityId, holderC.account.address, 20_000),
    owner,
    `铸造给 ${holderC.name}`,
    ownerName,
  );
  assertTxSuccess(mintCharlieResult, `铸造给 ${holderC.name}`);

  const approveResult = await ctx.send(
    (api.tx as any).entityToken.approveTokens(entityId, holderB.account.address, 2_000),
    holderA.account,
    `${holderA.name} 授权 ${holderB.name} 使用额度`,
    holderA.name,
  );
  assertTxSuccess(approveResult, 'approve_tokens');
  await ctx.check('授权额度已落库', holderA.name, async () => {
    const allowance = await (api.query as any).entityToken.tokenApprovals(
      entityId,
      holderA.account.address,
      holderB.account.address,
    );
    if (allowance.toString() !== '2000') {
      throw new Error(`授权额度异常: expected=2000 actual=${allowance.toString()}`);
    }
  });

  const holderABalanceBefore = await getEntityTokenBalance(api, entityId, holderA.account.address);
  const ownerBalanceBefore = await getEntityTokenBalance(api, entityId, owner.address);
  const transferFromResult = await ctx.send(
    (api.tx as any).entityToken.transferFrom(entityId, holderA.account.address, owner.address, 500),
    holderB.account,
    `${holderB.name} 使用授权转账`,
    holderB.name,
  );
  assertTxSuccess(transferFromResult, 'transfer_from');
  await ctx.check('授权转账状态已落库', holderB.name, async () => {
    const allowance = await (api.query as any).entityToken.tokenApprovals(
      entityId,
      holderA.account.address,
      holderB.account.address,
    );
    const holderABalanceAfter = await getEntityTokenBalance(api, entityId, holderA.account.address);
    const ownerBalanceAfter = await getEntityTokenBalance(api, entityId, owner.address);
    if (allowance.toString() !== '1500') {
      throw new Error(`transfer_from 后授权额度异常: expected=1500 actual=${allowance.toString()}`);
    }
    if (holderABalanceAfter !== holderABalanceBefore - 500n) {
      throw new Error(
        `transfer_from 后 owner 余额异常: expected=${holderABalanceBefore - 500n} actual=${holderABalanceAfter}`,
      );
    }
    if (ownerBalanceAfter !== ownerBalanceBefore + 500n) {
      throw new Error(
        `transfer_from 后接收方余额异常: expected=${ownerBalanceBefore + 500n} actual=${ownerBalanceAfter}`,
      );
    }
  });

  const globalPauseResult = await ctx.sudo(
    (api.tx as any).entityToken.setGlobalTokenPause(true),
    '开启全局代币暂停',
  );
  assertTxSuccess(globalPauseResult, '开启全局代币暂停');
  await ctx.check('全局代币暂停状态已写入', 'sudo(alice)', async () => {
    const paused = await (api.query as any).entityToken.globalTokenPaused();
    if (!paused.isTrue && paused.toString() !== 'true') {
      throw new Error('global token pause 未生效');
    }
  });

  const globalResumeResult = await ctx.sudo(
    (api.tx as any).entityToken.setGlobalTokenPause(false),
    '关闭全局代币暂停',
  );
  assertTxSuccess(globalResumeResult, '关闭全局代币暂停');
  await ctx.check('全局代币暂停已解除', 'sudo(alice)', async () => {
    const paused = await (api.query as any).entityToken.globalTokenPaused();
    if (paused.isTrue || paused.toString() === 'true') {
      throw new Error('global token pause 未解除');
    }
  });

  const freezeTransfersResult = await ctx.sudo(
    (api.tx as any).entityToken.forceFreezeTransfers(entityId),
    '冻结实体代币转账',
  );
  assertTxSuccess(freezeTransfersResult, '冻结实体代币转账');
  await ctx.check('实体代币转账冻结状态已写入', 'sudo(alice)', async () => {
    const frozen = await (api.query as any).entityToken.transfersFrozen(entityId);
    if (!hasStorageValue(frozen)) {
      throw new Error(`transfersFrozen 未写入: entityId=${entityId}`);
    }
  });

  const frozenTransferResult = await ctx.send(
    (api.tx as any).entityToken.transferTokens(entityId, holderB.account.address, 100),
    holderA.account,
    '[错误路径] 转账冻结时转账',
    holderA.name,
  );
  await ctx.check('转账冻结时转账应失败', holderA.name, () => {
    assertTxFailed(frozenTransferResult, 'TokenTransfersFrozen', 'frozen transfer');
  });

  const unfreezeTransfersResult = await ctx.sudo(
    (api.tx as any).entityToken.forceUnfreezeTransfers(entityId),
    '解除实体代币转账冻结',
  );
  assertTxSuccess(unfreezeTransfersResult, '解除实体代币转账冻结');
  await ctx.check('实体代币转账冻结已解除', 'sudo(alice)', async () => {
    const frozen = await (api.query as any).entityToken.transfersFrozen(entityId);
    if (hasStorageValue(frozen)) {
      throw new Error(`transfersFrozen 未清除: entityId=${entityId}`);
    }
  });

  const changeTypeResult = await ctx.send(
    (api.tx as any).entityToken.changeTokenType(entityId, 'Hybrid'),
    owner,
    '切换为 Hybrid Token',
    ownerName,
  );
  if (!changeTypeResult.success) {
    await ctx.check('复用已有 Hybrid Token 类型', ownerName, () => {
      const error = changeTypeResult.error ?? '';
      if (!error.includes('SameTokenType')) {
        throw new Error(`切换 token 类型失败: ${error}`);
      }
    });
  }

  const configDividendResult = await ctx.send(
    (api.tx as any).entityToken.configureDividend(entityId, true, 0),
    owner,
    '启用分红',
    ownerName,
  );
  assertTxSuccess(configDividendResult, '启用分红');

  const distributeDividendResult = await ctx.send(
    (api.tx as any).entityToken.distributeDividend(
      entityId,
      300,
      [
        [holderA.account.address, 200],
        [holderC.account.address, 100],
      ],
    ),
    owner,
    '分发待领取分红',
    ownerName,
  );
  assertTxSuccess(distributeDividendResult, '分发分红');

  const cancelPendingDividendsResult = await ctx.sudo(
    (api.tx as any).entityToken.forceCancelPendingDividends(
      entityId,
      [holderA.account.address, holderC.account.address],
    ),
    '强制取消待领取分红',
  );
  assertTxSuccess(cancelPendingDividendsResult, '强制取消待领取分红');
  await ctx.check('待领取分红已清空', 'sudo(alice)', async () => {
    const holderAPending = await (api.query as any).entityToken.pendingDividends(entityId, holderA.account.address);
    const holderCPending = await (api.query as any).entityToken.pendingDividends(entityId, holderC.account.address);
    const totalPending = await (api.query as any).entityToken.totalPendingDividends(entityId);
    if (holderAPending.toString() !== '0' || holderCPending.toString() !== '0' || totalPending.toString() !== '0') {
      throw new Error(
        `待领取分红未清空: holderA=${holderAPending.toString()} holderC=${holderCPending.toString()} total=${totalPending.toString()}`,
      );
    }
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
    owner,
    '配置 FullDAO',
    ownerName,
  );
  assertTxSuccess(governanceConfigResult, '配置治理');

  const proposal1Id = await createPromotionProposal(ctx, entityId, holderA.account, 'E11 提案-改票', holderA.name);
  const vote1Result = await ctx.send(
    (api.tx as any).entityGovernance.vote(proposal1Id, 'Yes'),
    holderA.account,
    `${holderA.name} 首次投票`,
    holderA.name,
  );
  assertTxSuccess(vote1Result, '首次投票');
  const changeVoteResult = await ctx.send(
    (api.tx as any).entityGovernance.changeVote(proposal1Id, 'Abstain'),
    holderA.account,
    `${holderA.name} 修改投票为弃权`,
    holderA.name,
  );
  assertTxSuccess(changeVoteResult, 'change_vote');
  await ctx.check('改票状态已落库', holderA.name, async () => {
    const voteRecord = await (api.query as any).entityGovernance.voteRecords(proposal1Id, holderA.account.address);
    if (!hasStorageValue(voteRecord)) {
      throw new Error(`VoteRecord 不存在: proposalId=${proposal1Id}`);
    }
    const vote = readVoteValue(voteRecord);
    if (vote !== 'Abstain') {
      throw new Error(`改票未落库为 Abstain: actual=${vote}`);
    }
  });

  const finalizeVotingResult = await ctx.send(
    (api.tx as any).entityGovernance.finalizeVoting(proposal1Id),
    holderA.account,
    '[错误路径] 投票期未结束时结束投票',
    holderA.name,
  );
  await ctx.check('投票期未结束前结束投票失败', holderA.name, () => {
    assertTxFailed(finalizeVotingResult, 'VotingNotEnded', 'finalize_voting');
  });

  const proposal2Id = await createPromotionProposal(
    ctx,
    entityId,
    holderB.account,
    'E11 提案-批量取消',
    holderB.name,
  );
  const pauseResult = await ctx.send(
    (api.tx as any).entityGovernance.pauseGovernance(entityId),
    owner,
    '暂停治理',
    ownerName,
  );
  assertTxSuccess(pauseResult, 'pause_governance');
  await ctx.check('治理已暂停', ownerName, async () => {
    const paused = await (api.query as any).entityGovernance.governancePaused(entityId);
    if (!paused.isTrue && paused.toString() !== 'true') {
      throw new Error('治理暂停状态未写入');
    }
  });

  const resumeResult = await ctx.send(
    (api.tx as any).entityGovernance.resumeGovernance(entityId),
    owner,
    '恢复治理',
    ownerName,
  );
  assertTxSuccess(resumeResult, 'resume_governance');
  await ctx.check('治理已恢复', ownerName, async () => {
    const paused = await (api.query as any).entityGovernance.governancePaused(entityId);
    if (paused.isTrue || paused.toString() === 'true') {
      throw new Error('治理暂停状态未清除');
    }
  });

  const batchCancelResult = await ctx.send(
    (api.tx as any).entityGovernance.batchCancelProposals(entityId),
    owner,
    '批量取消活跃提案',
    ownerName,
  );
  assertTxSuccess(batchCancelResult, 'batch_cancel_proposals');
  await ctx.check('活跃提案已批量取消', ownerName, async () => {
    const proposal1 = await (api.query as any).entityGovernance.proposals(proposal1Id);
    const proposal2 = await (api.query as any).entityGovernance.proposals(proposal2Id);
    const activeIds = await (api.query as any).entityGovernance.entityProposals(entityId);
    if (readProposalStatus(proposal1) !== 'Cancelled' || readProposalStatus(proposal2) !== 'Cancelled') {
      throw new Error(
        `批量取消后提案状态异常: proposal1=${readProposalStatus(proposal1)} proposal2=${readProposalStatus(proposal2)}`,
      );
    }
    if (parseIdList(activeIds).length !== 0) {
      throw new Error(`活跃提案列表未清空: ${JSON.stringify(activeIds.toHuman?.() ?? activeIds.toJSON?.() ?? activeIds)}`);
    }
  });

  const cleanupProposalResult = await ctx.send(
    (api.tx as any).entityGovernance.cleanupProposal(proposal2Id),
    holderA.account,
    '清理终态提案',
    holderA.name,
  );
  assertTxSuccess(cleanupProposalResult, 'cleanup_proposal');
  await ctx.check('终态提案已清理', holderA.name, async () => {
    const proposal = await (api.query as any).entityGovernance.proposals(proposal2Id);
    if (hasStorageValue(proposal)) {
      throw new Error(`cleanup_proposal 后提案仍存在: proposalId=${proposal2Id}`);
    }
  });

  const proposal3Id = await createPromotionProposal(
    ctx,
    entityId,
    holderC.account,
    'E11 提案-否决',
    holderC.name,
  );
  const vetoResult = await ctx.send(
    (api.tx as any).entityGovernance.vetoProposal(proposal3Id),
    owner,
    'Owner 否决提案',
    ownerName,
  );
  assertTxSuccess(vetoResult, 'veto_proposal');
  await ctx.check('提案已否决', ownerName, async () => {
    const proposal = await (api.query as any).entityGovernance.proposals(proposal3Id);
    const status = readProposalStatus(proposal);
    if (status !== 'Cancelled') {
      throw new Error(`提案未进入 Cancelled: actual=${status}`);
    }
  });

  const lockResult = await ctx.send(
    (api.tx as any).entityGovernance.lockGovernance(entityId),
    owner,
    '锁定治理配置',
    ownerName,
  );
  assertTxSuccess(lockResult, '锁定治理');
  await ctx.check('治理已锁定', ownerName, async () => {
    const locked = await (api.query as any).entityGovernance.governanceLocked(entityId);
    if (locked.toString() !== 'true') {
      throw new Error('治理锁定状态未写入');
    }
  });

  const forceUnlockResult = await ctx.sudo(
    (api.tx as any).entityGovernance.forceUnlockGovernance(entityId),
    '紧急强制解锁治理',
  );
  assertTxSuccess(forceUnlockResult, 'force_unlock_governance');
  await ctx.check('治理已强制解锁', 'sudo(alice)', async () => {
    const locked = await (api.query as any).entityGovernance.governanceLocked(entityId);
    const paused = await (api.query as any).entityGovernance.governancePaused(entityId);
    if (locked.toString() === 'true') {
      throw new Error('治理锁定状态未解除');
    }
    if (paused.toString() === 'true') {
      throw new Error('治理暂停状态未恢复');
    }
  });
}

async function ensureFreshEntity(
  ctx: FlowContext,
): Promise<{ entityId: number; owner: any; ownerName: string }> {
  const { api } = ctx;
  const candidates = ['dave', 'eve', 'charlie', 'bob'] as const;

  for (const ownerName of candidates) {
    const owner = ctx.actor(ownerName);
    const nextEntityId = (await (api.query as any).entityRegistry.nextEntityId()).toNumber();
    const createEntityResult = await ctx.send(
      (api.tx as any).entityRegistry.createEntity(
        `E11 Entity ${nextEntityId}`,
        null,
        `QmE11EntityDesc${nextEntityId}`,
        null,
      ),
      owner,
      `[错误路径] 尝试创建 E11 Entity (${ownerName})`,
      ownerName,
    );
    if (createEntityResult.success) {
      await ctx.check(`创建 E11 Entity (${ownerName})`, ownerName, () => {});
      return { entityId: nextEntityId, owner, ownerName };
    }
    if (!createEntityResult.error?.includes('MaxEntitiesReached')) {
      throw new Error(`创建 E11 Entity (${ownerName}) 失败: ${createEntityResult.error}`);
    }
    await ctx.check(`跳过已达实体上限账户 (${ownerName})`, ownerName, () => {});
  }

  throw new Error('所有候选账户都已达到实体上限，无法为 E11 创建干净上下文');
}

async function createPromotionProposal(
  ctx: FlowContext,
  entityId: number,
  signer: any,
  title: string,
  actorName: string = 'bob',
): Promise<number> {
  const { api } = ctx;
  const proposalId = (await (api.query as any).entityGovernance.nextProposalId()).toNumber();
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
  await ctx.check(`${title} 已落库`, actorName, async () => {
    const proposal = await (api.query as any).entityGovernance.proposals(proposalId);
    const status = readProposalStatus(proposal);
    if (status !== 'Voting') {
      throw new Error(`提案状态异常: proposalId=${proposalId} status=${status}`);
    }
  });
  return proposalId;
}

function hasStorageValue(value: any): boolean {
  if (typeof value?.isSome === 'boolean') return value.isSome;
  if (typeof value?.isEmpty === 'boolean') return !value.isEmpty;
  return value?.toJSON?.() != null;
}

function normalizeVariant(value: any): string {
  const human = value?.toHuman?.() ?? value?.toJSON?.() ?? value?.toString?.() ?? value;
  if (typeof human === 'string') return human;
  if (human && typeof human === 'object') {
    const keys = Object.keys(human);
    if (keys.length === 1) return keys[0];
  }
  return String(human);
}

function readProposalStatus(value: any): string {
  if (!hasStorageValue(value)) return 'Missing';
  const raw = value?.toHuman?.() ?? value?.toJSON?.() ?? value;
  return normalizeVariant(raw?.status);
}

function readVoteValue(value: any): string {
  const raw = value?.toHuman?.() ?? value?.toJSON?.() ?? value;
  return normalizeVariant(raw?.vote);
}

function parseIdList(value: any): string[] {
  const raw = value?.toHuman?.() ?? value?.toJSON?.() ?? [];
  if (Array.isArray(raw)) {
    return raw.map((item) => String(item).replace(/,/g, ''));
  }
  return [];
}

async function getEntityTokenBalance(api: any, entityId: number, address: string): Promise<bigint> {
  const assetId = ENTITY_TOKEN_ASSET_OFFSET + entityId;
  const account = await api.query.assets.account(assetId, address);
  if (!hasStorageValue(account)) return 0n;
  const raw = account.toJSON?.() as { balance?: string | number } | null | undefined;
  return BigInt(String(raw?.balance ?? 0));
}
