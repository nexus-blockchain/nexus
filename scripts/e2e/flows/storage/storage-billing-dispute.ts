/**
 * Flow-S2: 存储计费/Slash 争议回归
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertEventEmitted,
  assertTxFailed,
  assertTxSuccess,
} from '../../core/assertions.js';
import { nex } from '../../core/config.js';

export const storageBillingDisputeFlow: FlowDef = {
  name: 'Flow-S2: 存储计费/Slash 争议',
  description: '用户资金提现 + tier 降级 + slash 争议 + 已弃用接口校验',
  fn: runStorageBillingDisputeFlow,
};

async function runStorageBillingDisputeFlow(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const charlie = ctx.actor('charlie');

  const peerId = '0x' + '11'.repeat(32);
  const endpointHash = '0x' + '22'.repeat(32);
  const certFingerprint = '0x' + '33'.repeat(32);
  const cid = 'QmS2PinCid001';

  const joinOperatorResult = await ctx.send(
    (api.tx as any).storageService.joinOperator(
      peerId,
      128,
      endpointHash,
      certFingerprint,
      nex(50).toString(),
    ),
    charlie,
    'Charlie 加入存储运营者 (S2)',
    'charlie',
  );
  if (joinOperatorResult.success) {
    await ctx.check('运营者加入事件', 'charlie', () => {
      assertEventEmitted(joinOperatorResult, 'storageService', 'OperatorJoined', 'join_operator');
    });
  } else {
    await ctx.check('复用已有运营者上下文', 'charlie', () => {
      const error = joinOperatorResult.error ?? '';
      if (!error.includes('AlreadyOperator')) {
        throw new Error(`加入运营者失败: ${error}`);
      }
    });
  }

  const fundUserResult = await ctx.send(
    (api.tx as any).storageService.fundUserAccount(bob.address, nex(20).toString()),
    bob,
    'Bob 充值用户资金 (S2)',
    'bob',
  );
  assertTxSuccess(fundUserResult, '充值用户资金');

  const withdrawFundingResult = await ctx.send(
    (api.tx as any).storageService.withdrawUserFunding(nex(5).toString()),
    bob,
    'Bob 提现用户资金',
    'bob',
  );
  assertTxSuccess(withdrawFundingResult, '提现用户资金');
  await ctx.check('用户资金提现事件', 'bob', () => {
    assertEventEmitted(withdrawFundingResult, 'storageService', 'UserFundingWithdrawn', 'withdraw_user_funding');
  });

  const pinRequestResult = await ctx.send(
    (api.tx as any).storageService.requestPinForSubject(0, cid, 1024, null),
    bob,
    'Bob 请求 Pin (S2)',
    'bob',
  );
  assertTxSuccess(pinRequestResult, '请求 Pin');

  const downgradeTierResult = await ctx.send(
    (api.tx as any).storageService.downgradePinTier(cid, 'Temporary'),
    bob,
    'Bob 降级 Pin Tier',
    'bob',
  );
  assertTxSuccess(downgradeTierResult, 'downgrade_pin_tier');
  await ctx.check('Pin 降级事件', 'bob', () => {
    assertEventEmitted(downgradeTierResult, 'storageService', 'PinTierDowngraded', 'downgrade_pin_tier');
  });

  const disputeSlashResult = await ctx.send(
    (api.tx as any).storageService.disputeSlash(nex(1).toString(), 'slash dispute evidence'),
    charlie,
    '运营者发起 Slash 争议',
    'charlie',
  );
  assertTxSuccess(disputeSlashResult, 'dispute_slash');
  await ctx.check('Slash 争议事件', 'charlie', () => {
    assertEventEmitted(disputeSlashResult, 'storageService', 'SlashDisputed', 'dispute_slash');
  });

  const deprecatedFundSubjectResult = await ctx.send(
    (api.tx as any).storageService.fundSubjectAccount(0, nex(1).toString()),
    bob,
    '[错误路径] 调用已弃用的 fund_subject_account',
    'bob',
  );
  await ctx.check('已弃用接口拒绝调用', 'bob', () => {
    assertTxFailed(deprecatedFundSubjectResult, 'BadParams', 'fund_subject_account');
  });
}
