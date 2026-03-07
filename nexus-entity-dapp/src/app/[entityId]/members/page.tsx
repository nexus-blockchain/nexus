'use client';

import React, { useState, useCallback } from 'react';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useMembers } from '@/hooks/use-members';
import { PermissionGuard } from '@/components/permission-guard';
import { TxConfirmDialog } from '@/components/tx-confirm-dialog';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { AdminPermission } from '@/lib/types/models';
import { MemberStatus, RegistrationPolicy } from '@/lib/types/enums';

import { useTranslations } from 'next-intl';
// ─── Helpers ────────────────────────────────────────────────

function isTxBusy(m: { txState: { status: string } }): boolean {
  return m.txState.status === 'signing' || m.txState.status === 'broadcasting';
}

const STATUS_BADGE: Record<MemberStatus, string> = {
  [MemberStatus.Active]: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300',
  [MemberStatus.Pending]: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-300',
  [MemberStatus.Frozen]: 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-300',
  [MemberStatus.Banned]: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300',
  [MemberStatus.Expired]: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300',
};

const STATUS_LABEL: Record<MemberStatus, string> = {
  [MemberStatus.Active]: '活跃',
  [MemberStatus.Pending]: '待审核',
  [MemberStatus.Frozen]: '已冻结',
  [MemberStatus.Banned]: '已封禁',
  [MemberStatus.Expired]: '已过期',
};

const POLICY_FLAGS = [
  { bit: RegistrationPolicy.OPEN, label: '开放注册' },
  { bit: RegistrationPolicy.PURCHASE_REQUIRED, label: '需要购买' },
  { bit: RegistrationPolicy.REFERRAL_REQUIRED, label: '需要推荐人' },
  { bit: RegistrationPolicy.APPROVAL_REQUIRED, label: '需要审批' },
  { bit: RegistrationPolicy.KYC_REQUIRED, label: '需要 KYC' },
  { bit: RegistrationPolicy.KYC_UPGRADE_REQUIRED, label: '需要 KYC 升级' },
];

function shortAddr(addr: string): string {
  return addr.length > 12 ? `${addr.slice(0, 6)}…${addr.slice(-6)}` : addr;
}

// ─── Registration Policy Section ────────────────────────────

function PolicySection() {
  const { entityId } = useEntityContext();
  const { policy, setRegistrationPolicy } = useMembers();
  const [localPolicy, setLocalPolicy] = useState<number | null>(null);
  const current = localPolicy ?? policy;

  const toggleBit = useCallback((bit: number) => {
    setLocalPolicy((prev) => (prev ?? policy) ^ bit);
  }, [policy]);

  const handleSave = useCallback(() => {
    if (localPolicy !== null && localPolicy !== policy) {
      setRegistrationPolicy.mutate([entityId, localPolicy]);
      setLocalPolicy(null);
    }
  }, [entityId, localPolicy, policy, setRegistrationPolicy]);

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">注册策略</h2>
      <div className="space-y-2">
        {POLICY_FLAGS.map(({ bit, label }) => (
          <label key={bit} className="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
            <input
              type="checkbox"
              checked={(current & bit) !== 0 || bit === 0}
              disabled={bit === 0}
              onChange={() => toggleBit(bit)}
              className="rounded border-gray-300"
            />
            {label}
          </label>
        ))}
      </div>
      {localPolicy !== null && localPolicy !== policy && (
        <div className="mt-4 flex items-center gap-3">
          <button
            type="button"
            onClick={handleSave}
            disabled={isTxBusy(setRegistrationPolicy)}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            保存策略
          </button>
          <TxStatusIndicator txState={setRegistrationPolicy.txState} />
        </div>
      )}
    </section>
  );
}

// ─── Pending Members Section ────────────────────────────────

function PendingSection() {
  const { entityId } = useEntityContext();
  const { pendingMembers, policy, approveMember, rejectMember } = useMembers();
  const hasApproval = (policy & RegistrationPolicy.APPROVAL_REQUIRED) !== 0;

  if (!hasApproval || pendingMembers.length === 0) return null;

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">
        待审批会员 ({pendingMembers.length})
      </h2>
      <div className="space-y-2">
        {pendingMembers.map((account) => (
          <div key={account} className="flex items-center justify-between rounded-md border border-gray-100 px-4 py-2 dark:border-gray-700">
            <span className="font-mono text-sm text-gray-900 dark:text-gray-100">{shortAddr(account)}</span>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => approveMember.mutate([entityId, account])}
                disabled={isTxBusy(approveMember)}
                className="rounded-md bg-green-600 px-3 py-1 text-xs text-white hover:bg-green-700 disabled:cursor-not-allowed disabled:opacity-50"
              >
                通过
              </button>
              <button
                type="button"
                onClick={() => rejectMember.mutate([entityId, account])}
                disabled={isTxBusy(rejectMember)}
                className="rounded-md bg-red-600 px-3 py-1 text-xs text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50"
              >
                拒绝
              </button>
            </div>
          </div>
        ))}
      </div>
      <div className="mt-3 flex items-center gap-3">
        <TxStatusIndicator txState={approveMember.txState} />
        <TxStatusIndicator txState={rejectMember.txState} />
      </div>
    </section>
  );
}

// ─── Referral Display ───────────────────────────────────────

function ReferralSection({ account }: { account: string }) {
  const { useReferralTree } = useMembers();
  const { data } = useReferralTree(account);

  if (!data || data.directReferrals.length === 0) return null;

  return (
    <div className="mt-2 rounded-md bg-gray-50 p-3 dark:bg-gray-800">
      <p className="mb-1 text-xs font-medium text-gray-500">直推列表 (团队: {data.teamSize})</p>
      <div className="flex flex-wrap gap-1">
        {data.directReferrals.map((ref) => (
          <span key={ref} className="rounded bg-gray-200 px-2 py-0.5 font-mono text-xs text-gray-700 dark:bg-gray-700 dark:text-gray-300">
            {shortAddr(ref)}
          </span>
        ))}
      </div>
    </div>
  );
}

// ─── Member Actions ─────────────────────────────────────────

function MemberActions({ account, status }: { account: string; status: MemberStatus }) {
  const { entityId } = useEntityContext();
  const { freezeMember, unfreezeMember, banMember, unbanMember, removeMember } = useMembers();
  const [confirmAction, setConfirmAction] = useState<{ mutation: ReturnType<typeof useMembers>['banMember']; config: { title: string; description: string; severity: 'danger' } } | null>(null);

  const handleDangerous = useCallback((mutation: typeof banMember, config: typeof confirmAction extends null ? never : NonNullable<typeof confirmAction>['config']) => {
    setConfirmAction({ mutation, config });
  }, []);

  const onConfirm = useCallback(() => {
    if (confirmAction) {
      confirmAction.mutation.mutate([entityId, account]);
      setConfirmAction(null);
    }
  }, [confirmAction, entityId, account]);

  return (
    <>
      <div className="flex flex-wrap gap-1">
        {status === MemberStatus.Active && (
          <>
            <button type="button" onClick={() => freezeMember.mutate([entityId, account])} disabled={isTxBusy(freezeMember)} className="rounded-md bg-blue-600 px-2 py-1 text-xs text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">冻结</button>
            <button type="button" onClick={() => handleDangerous(banMember, { title: '确认封禁', description: `确定封禁会员 ${shortAddr(account)}？`, severity: 'danger' })} className="rounded-md bg-red-600 px-2 py-1 text-xs text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50">封禁</button>
          </>
        )}
        {status === MemberStatus.Frozen && (
          <button type="button" onClick={() => unfreezeMember.mutate([entityId, account])} disabled={isTxBusy(unfreezeMember)} className="rounded-md bg-green-600 px-2 py-1 text-xs text-white hover:bg-green-700 disabled:cursor-not-allowed disabled:opacity-50">解冻</button>
        )}
        {status === MemberStatus.Banned && (
          <button type="button" onClick={() => unbanMember.mutate([entityId, account])} disabled={isTxBusy(unbanMember)} className="rounded-md bg-yellow-600 px-2 py-1 text-xs text-white hover:bg-yellow-700 disabled:cursor-not-allowed disabled:opacity-50">解封</button>
        )}
        <button type="button" onClick={() => handleDangerous(removeMember, { title: '确认移除', description: `确定移除会员 ${shortAddr(account)}？此操作不可撤销。`, severity: 'danger' })} className="rounded-md bg-gray-500 px-2 py-1 text-xs text-white hover:bg-gray-600 disabled:cursor-not-allowed disabled:opacity-50">移除</button>
      </div>
      {confirmAction && (
        <TxConfirmDialog
          open
          onClose={() => setConfirmAction(null)}
          onConfirm={onConfirm}
          config={confirmAction.config}
        />
      )}
    </>
  );
}

// ─── Member List Section ────────────────────────────────────

function MemberListSection() {
  const { members, memberCount } = useMembers();
  const [filter, setFilter] = useState<MemberStatus | 'all'>('all');
  const [expandedAccount, setExpandedAccount] = useState<string | null>(null);

  const filtered = filter === 'all' ? members : members.filter((m) => m.status === filter);

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          会员列表 ({memberCount})
        </h2>
        <select
          value={filter}
          onChange={(e) => setFilter(e.target.value as MemberStatus | 'all')}
          className="rounded-md border border-gray-300 px-3 py-1.5 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
        >
          <option value="all">全部</option>
          {Object.values(MemberStatus).map((s) => (
            <option key={s} value={s}>{STATUS_LABEL[s]}</option>
          ))}
        </select>
      </div>

      {filtered.length === 0 ? (
        <p className="py-8 text-center text-sm text-gray-400">暂无会员</p>
      ) : (
        <div className="space-y-2">
          {filtered.map((m) => (
            <div key={m.account} className="rounded-md border border-gray-100 p-4 dark:border-gray-700">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <button
                    type="button"
                    onClick={() => setExpandedAccount(expandedAccount === m.account ? null : m.account)}
                    className="font-mono text-sm text-gray-900 hover:text-blue-600 dark:text-gray-100"
                  >
                    {shortAddr(m.account)}
                  </button>
                  <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${STATUS_BADGE[m.status]}`}>
                    {STATUS_LABEL[m.status]}
                  </span>
                  <span className="text-xs text-gray-500">Lv.{m.level}</span>
                </div>
                <PermissionGuard required={AdminPermission.MEMBER_MANAGE} fallback={null}>
                  <MemberActions account={m.account} status={m.status} />
                </PermissionGuard>
              </div>
              <div className="mt-2 flex gap-4 text-xs text-gray-500">
                <span>消费: {m.totalSpent.toString()}</span>
                <span>订单: {m.orderCount}</span>
                {m.referrer && <span>推荐人: {shortAddr(m.referrer)}</span>}
              </div>
              {expandedAccount === m.account && <ReferralSection account={m.account} />}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function MembersPage() {
  const t = useTranslations('members');
  const { isLoading, error } = useMembers();

  if (isLoading) {
    return <div className="flex items-center justify-center p-12 text-sm text-gray-500">{t('loading')}</div>;
  }

  if (error) {
    return <div className="flex items-center justify-center p-12 text-sm text-red-500">加载失败: {String(error)}</div>;
  }

  return (
    <div className="mx-auto max-w-5xl space-y-6 p-4 sm:p-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('title')}</h1>

      <PermissionGuard required={AdminPermission.MEMBER_MANAGE} fallback={null}>
        <PolicySection />
        <PendingSection />
      </PermissionGuard>

      <MemberListSection />
    </div>
  );
}
