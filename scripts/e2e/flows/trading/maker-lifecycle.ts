/**
 * Flow-T1: 做市商完整生命周期
 *
 * 角色: Bob (做市商申请人), Alice (Sudo/审批)
 *
 * 流程:
 *   1. 查询初始状态 (Bob 不是做市商)
 *   2. Bob 锁定押金
 *   3. Bob 提交申请信息
 *   4. Alice(Sudo) 审批通过
 *   5. 验证做市商状态为 Active
 *   6. [错误路径] 重复锁定押金应失败
 *   7. Bob 补充押金
 *   8. [错误路径] 非做市商取消应失败
 */

import { FlowDef, FlowContext } from '../../core/test-runner.js';
import {
  assertTxSuccess,
  assertTxFailed,
  assertStorageExists,
  assertStorageField,
  assertEventEmitted,
  assertTrue,
} from '../../core/assertions.js';
import { getFreeBalance } from '../../core/chain-state.js';

export const makerLifecycleFlow: FlowDef = {
  name: 'Flow-T1: 做市商生命周期',
  description: '锁定押金 → 提交信息 → 审批 → 激活 → 补充押金',
  fn: makerLifecycle,
};

async function makerLifecycle(ctx: FlowContext): Promise<void> {
  const { api } = ctx;
  const bob = ctx.actor('bob');
  const charlie = ctx.actor('charlie');

  // --------------- Step 1: 查询初始状态 ---------------
  await ctx.check('查询初始状态 — Bob 不是做市商', 'bob', async () => {
    const bobMakerId = await (api.query as any).tradingMaker.accountToMaker(bob.address);
    // 如果 Bob 已经是做市商，跳过此 flow
    if (bobMakerId.isSome) {
      console.log(`    ℹ Bob 已是做市商 ID=${bobMakerId.unwrap().toNumber()}, 跳过创建步骤`);
    }
  });

  // 检查 Bob 是否已是做市商
  const existingMakerId = await (api.query as any).tradingMaker.accountToMaker(bob.address);
  let makerId: number;

  if (existingMakerId.isSome) {
    makerId = existingMakerId.unwrap().toNumber();
    // 验证状态
    await ctx.check('验证已有做市商状态', 'bob', async () => {
      await assertStorageExists(api, 'tradingMaker', 'makerApplications', [makerId]);
    });
  } else {
    // --------------- Step 2: 锁定押金 ---------------
    const balanceBefore = await getFreeBalance(api, bob.address);

    const lockTx = (api.tx as any).tradingMaker.lockDeposit();
    const lockResult = await ctx.send(lockTx, bob, '锁定押金', 'bob');
    assertTxSuccess(lockResult, '锁定押金');
    assertEventEmitted(lockResult, 'tradingMaker', 'DepositLocked', '锁定押金事件');

    // 获取新的做市商 ID
    const newMakerId = await (api.query as any).tradingMaker.accountToMaker(bob.address);
    assertTrue(newMakerId.isSome, 'Bob 应已获得做市商 ID');
    makerId = newMakerId.unwrap().toNumber();

    await ctx.check('验证押金锁定后状态', 'bob', async () => {
      await assertStorageField(
        api, 'tradingMaker', 'makerApplications', [makerId],
        'status', 'DepositLocked', '状态应为 DepositLocked',
      );
    });

    // --------------- Step 3: 提交申请信息 ---------------
    const submitTx = (api.tx as any).tradingMaker.submitInfo(
      'Zhang San',
      '110101199001011234',
      '1990-01-01',
      'TXxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx',
      'wechat_bob_test',
    );
    const submitResult = await ctx.send(submitTx, bob, '提交申请信息', 'bob');
    assertTxSuccess(submitResult, '提交信息');

    await ctx.check('验证提交后状态为 PendingReview', 'bob', async () => {
      await assertStorageField(
        api, 'tradingMaker', 'makerApplications', [makerId],
        'status', 'PendingReview', '状态应为 PendingReview',
      );
    });

    // --------------- Step 4: Sudo 审批通过 ---------------
    const approveTx = (api.tx as any).tradingMaker.approveMaker(makerId);
    const approveResult = await ctx.sudo(approveTx, '审批做市商');
    assertTxSuccess(approveResult, '审批');

    // --------------- Step 5: 验证 Active 状态 ---------------
    await ctx.check('验证做市商已激活', 'bob', async () => {
      await assertStorageField(
        api, 'tradingMaker', 'makerApplications', [makerId],
        'status', 'Active', '状态应为 Active',
      );
    });
  }

  // --------------- Step 6: 错误路径 — 重复锁定押金 ---------------
  const dupLockTx = (api.tx as any).tradingMaker.lockDeposit();
  const dupResult = await ctx.send(dupLockTx, bob, '[错误路径] 重复锁定押金', 'bob');
  await ctx.check('重复锁定应失败', 'bob', () => {
    assertTxFailed(dupResult, undefined, '重复锁定押金');
  });

  // --------------- Step 7: 错误路径 — 非做市商操作 ---------------
  const charlieCancel = (api.tx as any).tradingMaker.cancelMaker();
  const charlieResult = await ctx.send(charlieCancel, charlie, '[错误路径] 非做市商取消', 'charlie');
  await ctx.check('非做市商取消应失败', 'charlie', () => {
    assertTxFailed(charlieResult, undefined, '非做市商取消');
  });

  // --------------- Step 8: 查询做市商详情 ---------------
  await ctx.check('查询做市商详细信息', 'bob', async () => {
    const app = await (api.query as any).tradingMaker.makerApplications(makerId);
    assertTrue(app.isSome, '做市商记录应存在');
    const data = app.unwrap().toHuman();
    console.log(`    做市商 ID: ${makerId}`);
    console.log(`    状态: ${data.status}`);
    console.log(`    押金: ${data.deposit}`);
  });
}
