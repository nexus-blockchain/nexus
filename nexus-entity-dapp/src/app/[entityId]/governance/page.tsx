'use client';

import React, { useState, useCallback } from 'react';
import Link from 'next/link';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useGovernance, computeProposalResult } from '@/hooks/use-governance';
import { PermissionGuard } from '@/components/permission-guard';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { AdminPermission } from '@/lib/types/models';
import { GovernanceMode, ProposalCategory } from '@/lib/types/enums';

import { useTranslations } from 'next-intl';
// ─── Proposal Types by Category ─────────────────────────────

const PROPOSAL_TYPES: Record<string, { label: string; types: { value: string; label: string }[] }> = {
  [ProposalCategory.EntityManagement]: {
    label: '实体管理',
    types: [
      { value: 'UpdateEntityName', label: '修改实体名称' },
      { value: 'UpdateEntityLogo', label: '修改实体Logo' },
      { value: 'UpdateEntityDescription', label: '修改实体描述' },
      { value: 'UpdateEntityMetadata', label: '修改实体元数据' },
      { value: 'TransferOwnership', label: '转让所有权' },
      { value: 'RequestCloseEntity', label: '申请关闭实体' },
    ],
  },
  [ProposalCategory.ShopManagement]: {
    label: '店铺管理',
    types: [
      { value: 'CreateShop', label: '创建店铺' },
      { value: 'UpdateShop', label: '更新店铺' },
      { value: 'PauseShop', label: '暂停店铺' },
      { value: 'ResumeShop', label: '恢复店铺' },
      { value: 'CloseShop', label: '关闭店铺' },
    ],
  },
  [ProposalCategory.TokenManagement]: {
    label: '代币管理',
    types: [
      { value: 'MintTokens', label: '铸造代币' },
      { value: 'BurnTokens', label: '销毁代币' },
      { value: 'SetTransferRestriction', label: '设置转账限制' },
      { value: 'UpdateTokenMetadata', label: '更新代币元数据' },
      { value: 'CreateSaleRound', label: '创建发售轮次' },
      { value: 'SetDividendPolicy', label: '设置分红策略' },
    ],
  },
  [ProposalCategory.MarketManagement]: {
    label: '市场管理',
    types: [
      { value: 'SetPriceProtection', label: '设置价格保护' },
      { value: 'ToggleCircuitBreaker', label: '切换熔断机制' },
      { value: 'UpdateMarketConfig', label: '更新市场配置' },
      { value: 'SetTradingFee', label: '设置交易费率' },
    ],
  },
  [ProposalCategory.MemberManagement]: {
    label: '会员管理',
    types: [
      { value: 'SetRegistrationPolicy', label: '设置注册策略' },
      { value: 'AddCustomLevel', label: '添加自定义等级' },
      { value: 'UpdateCustomLevel', label: '更新自定义等级' },
      { value: 'SetUpgradeTrigger', label: '设置升级触发' },
      { value: 'BanMember', label: '封禁会员' },
      { value: 'UnbanMember', label: '解封会员' },
    ],
  },
  [ProposalCategory.CommissionManagement]: {
    label: '佣金管理',
    types: [
      { value: 'SetCommissionRate', label: '设置佣金费率' },
      { value: 'EnableCommissionPlugin', label: '启用佣金插件' },
      { value: 'DisableCommissionPlugin', label: '禁用佣金插件' },
      { value: 'UpdateWithdrawalConfig', label: '更新提现配置' },
      { value: 'PauseWithdrawal', label: '暂停提现' },
      { value: 'ResumeWithdrawal', label: '恢复提现' },
    ],
  },
  [ProposalCategory.DisclosureManagement]: {
    label: '披露管理',
    types: [
      { value: 'PublishDisclosure', label: '发布披露' },
      { value: 'WithdrawDisclosure', label: '撤回披露' },
      { value: 'SetDisclosurePolicy', label: '设置披露策略' },
    ],
  },
  [ProposalCategory.GovernanceManagement]: {
    label: '治理管理',
    types: [
      { value: 'UpdateQuorum', label: '更新法定人数' },
      { value: 'UpdatePassThreshold', label: '更新通过阈值' },
      { value: 'UpdateVotingPeriod', label: '更新投票周期' },
      { value: 'AddAdmin', label: '添加管理员' },
      { value: 'RemoveAdmin', label: '移除管理员' },
      { value: 'LockGovernance', label: '锁定治理' },
    ],
  },
};

// ─── Helpers ────────────────────────────────────────────────

function isTxBusy(m: { txState: { status: string } }): boolean {
  return m.txState.status === 'signing' || m.txState.status === 'broadcasting';
}

function shortAddr(addr: string): string {
  return addr.length > 12 ? `${addr.slice(0, 6)}…${addr.slice(-6)}` : addr;
}

const STATUS_BADGE: Record<string, string> = {
  Pending: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-300',
  Active: 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-300',
  Passed: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300',
  Rejected: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300',
  Executed: 'bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-300',
  Cancelled: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300',
};

const STATUS_LABEL: Record<string, string> = {
  Pending: '待投票',
  Active: '投票中',
  Passed: '已通过',
  Rejected: '已否决',
  Executed: '已执行',
  Cancelled: '已取消',
};


// ─── Create Proposal Form ───────────────────────────────────

function CreateProposalForm() {
  const { entityId } = useEntityContext();
  const { createProposal } = useGovernance();
  const [category, setCategory] = useState<string>(ProposalCategory.EntityManagement);
  const [proposalType, setProposalType] = useState<string>(PROPOSAL_TYPES[ProposalCategory.EntityManagement].types[0].value);
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [open, setOpen] = useState(false);

  const handleCategoryChange = useCallback((cat: string) => {
    setCategory(cat);
    const firstType = PROPOSAL_TYPES[cat]?.types[0]?.value ?? '';
    setProposalType(firstType);
  }, []);

  const handleSubmit = useCallback(() => {
    if (!title.trim() || !proposalType) return;
    createProposal.mutate([entityId, proposalType, title.trim(), description.trim()]);
    setTitle('');
    setDescription('');
  }, [entityId, proposalType, title, description, createProposal]);

  if (!open) {
    return (
      <button
        type="button"
        onClick={() => setOpen(true)}
        className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700"
      >
        创建提案
      </button>
    );
  }

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">创建提案</h2>

      <div className="space-y-4">
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <div>
            <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">提案分类</label>
            <select
              value={category}
              onChange={(e) => handleCategoryChange(e.target.value)}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            >
              {Object.entries(PROPOSAL_TYPES).map(([key, { label }]) => (
                <option key={key} value={key}>{label}</option>
              ))}
            </select>
          </div>
          <div>
            <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">提案类型</label>
            <select
              value={proposalType}
              onChange={(e) => setProposalType(e.target.value)}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            >
              {PROPOSAL_TYPES[category]?.types.map(({ value, label }) => (
                <option key={value} value={value}>{label}</option>
              ))}
            </select>
          </div>
        </div>

        <div>
          <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">标题</label>
          <input
            type="text"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="提案标题"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
          />
        </div>

        <div>
          <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">描述</label>
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="提案描述"
            rows={3}
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
          />
        </div>

        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={handleSubmit}
            disabled={!title.trim() || isTxBusy(createProposal)}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            提交提案
          </button>
          <button
            type="button"
            onClick={() => setOpen(false)}
            className="rounded-md border border-gray-300 px-4 py-2 text-sm text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:text-gray-300 dark:hover:bg-gray-800"
          >
            取消
          </button>
          <TxStatusIndicator txState={createProposal.txState} />
        </div>
      </div>
    </section>
  );
}

// ─── Proposal List ──────────────────────────────────────────

function ProposalListSection() {
  const { entityId } = useEntityContext();
  const { proposals, proposalCount } = useGovernance();
  const [filter, setFilter] = useState<string>('all');

  const filtered = filter === 'all' ? proposals : proposals.filter((p) => p.status === filter);

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          提案列表 ({proposalCount})
        </h2>
        <select
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          className="rounded-md border border-gray-300 px-3 py-1.5 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
        >
          <option value="all">全部</option>
          {Object.entries(STATUS_LABEL).map(([key, label]) => (
            <option key={key} value={key}>{label}</option>
          ))}
        </select>
      </div>

      {filtered.length === 0 ? (
        <p className="py-8 text-center text-sm text-gray-400">暂无提案</p>
      ) : (
        <div className="space-y-3">
          {filtered.map((p) => {
            const result = computeProposalResult(p);
            return (
              <Link
                key={p.id}
                href={`/${entityId}/governance/${p.id}`}
                className="block rounded-md border border-gray-100 p-4 transition-colors hover:bg-gray-50 dark:border-gray-700 dark:hover:bg-gray-800"
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <span className="text-sm font-medium text-gray-900 dark:text-gray-100">
                      #{p.id} {p.title}
                    </span>
                    <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${STATUS_BADGE[p.status] ?? STATUS_BADGE.Pending}`}>
                      {STATUS_LABEL[p.status] ?? p.status}
                    </span>
                  </div>
                  <span className="text-xs text-gray-500">{p.proposalType}</span>
                </div>
                <div className="mt-2 flex gap-4 text-xs text-gray-500">
                  <span>提案人: {shortAddr(p.proposer)}</span>
                  <span>赞成: {p.votesApprove.toString()}</span>
                  <span>反对: {p.votesReject.toString()}</span>
                  <span>弃权: {p.votesAbstain.toString()}</span>
                  <span>支持率: {result.approvalPct.toFixed(1)}%</span>
                </div>
              </Link>
            );
          })}
        </div>
      )}
    </section>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function GovernancePage() {
  const t = useTranslations('governance');
  const { governanceMode, isLoading: entityLoading } = useEntityContext();
  const { isLoading, error } = useGovernance();

  if (entityLoading || isLoading) {
    return <div className="flex items-center justify-center p-12 text-sm text-gray-500">{t('loading')}</div>;
  }

  if (governanceMode === GovernanceMode.None) {
    return (
      <div className="flex items-center justify-center p-12">
        <p className="text-sm text-gray-500">该实体未启用 DAO 治理</p>
      </div>
    );
  }

  if (error) {
    return <div className="flex items-center justify-center p-12 text-sm text-red-500">加载失败: {String(error)}</div>;
  }

  return (
    <div className="mx-auto max-w-5xl space-y-6 p-4 sm:p-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('title')}</h1>
        <PermissionGuard required={AdminPermission.GOVERNANCE_MANAGE} fallback={null}>
          <CreateProposalForm />
        </PermissionGuard>
      </div>

      <ProposalListSection />
    </div>
  );
}
