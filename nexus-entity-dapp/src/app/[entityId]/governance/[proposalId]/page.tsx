'use client';

import React, { useState, useCallback } from 'react';
import { useParams } from 'next/navigation';
import Link from 'next/link';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useGovernance, computeProposalResult } from '@/hooks/use-governance';
import { PermissionGuard } from '@/components/permission-guard';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { AdminPermission } from '@/lib/types/models';
import { VoteOption, GovernanceMode } from '@/lib/types/enums';
import { useWalletStore } from '@/stores/wallet-store';

import { useTranslations } from 'next-intl';
// ─── Helpers ────────────────────────────────────────────────

function isTxBusy(m: { txState: { status: string } }): boolean {
  return m.txState.status === 'signing' || m.txState.status === 'broadcasting';
}

function shortAddr(addr: string): string {
  return addr.length > 12 ? `${addr.slice(0, 6)}…${addr.slice(-6)}` : addr;
}

// ─── Vote Progress Bar ──────────────────────────────────────

function VoteBar({ label, count, total, color }: { label: string; count: bigint; total: number; color: string }) {
  const pct = total > 0 ? (Number(count) / total) * 100 : 0;
  return (
    <div className="space-y-1">
      <div className="flex justify-between text-sm">
        <span className="text-gray-700 dark:text-gray-300">{label}</span>
        <span className="text-gray-500">{count.toString()} ({pct.toFixed(1)}%)</span>
      </div>
      <div className="h-2 w-full overflow-hidden rounded-full bg-gray-200 dark:bg-gray-700">
        <div className={`h-full rounded-full ${color}`} style={{ width: `${Math.min(pct, 100)}%` }} />
      </div>
    </div>
  );
}

// ─── Voting Section ─────────────────────────────────────────

function VotingActions({ proposalId }: { proposalId: number }) {
  const { entityId } = useEntityContext();
  const { vote } = useGovernance();
  const walletAddress = useWalletStore((s) => s.address);

  const handleVote = useCallback((option: VoteOption) => {
    vote.mutate([entityId, proposalId, option]);
  }, [entityId, proposalId, vote]);

  if (!walletAddress) {
    return <p className="text-sm text-gray-500">请先连接钱包以参与投票</p>;
  }

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap gap-3">
        <button
          type="button"
          onClick={() => handleVote(VoteOption.Approve)}
          disabled={isTxBusy(vote)}
          className="rounded-md bg-green-600 px-4 py-2 text-sm text-white hover:bg-green-700 disabled:cursor-not-allowed disabled:opacity-50"
        >
          赞成
        </button>
        <button
          type="button"
          onClick={() => handleVote(VoteOption.Reject)}
          disabled={isTxBusy(vote)}
          className="rounded-md bg-red-600 px-4 py-2 text-sm text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50"
        >
          反对
        </button>
        <button
          type="button"
          onClick={() => handleVote(VoteOption.Abstain)}
          disabled={isTxBusy(vote)}
          className="rounded-md bg-gray-500 px-4 py-2 text-sm text-white hover:bg-gray-600 disabled:cursor-not-allowed disabled:opacity-50"
        >
          弃权
        </button>
      </div>
      <TxStatusIndicator txState={vote.txState} />
    </div>
  );
}


// ─── Admin Actions ──────────────────────────────────────────

function AdminActions({ proposalId, status, executed }: { proposalId: number; status: string; executed: boolean }) {
  const { entityId } = useEntityContext();
  const { finalizeVoting, executeProposal } = useGovernance();

  const canFinalize = status === 'Active' || status === 'Pending';
  const canExecute = status === 'Passed' && !executed;

  return (
    <div className="space-y-3">
      {canFinalize && (
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={() => finalizeVoting.mutate([entityId, proposalId])}
            disabled={isTxBusy(finalizeVoting)}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            结束投票
          </button>
          <TxStatusIndicator txState={finalizeVoting.txState} />
        </div>
      )}
      {canExecute && (
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={() => executeProposal.mutate([entityId, proposalId])}
            disabled={isTxBusy(executeProposal)}
            className="rounded-md bg-purple-600 px-4 py-2 text-sm text-white hover:bg-purple-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            执行提案
          </button>
          <TxStatusIndicator txState={executeProposal.txState} />
        </div>
      )}
    </div>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function ProposalDetailPage() {
  const t = useTranslations('governance');
  const params = useParams();
  const entityId = Number(params.entityId);
  const proposalId = Number(params.proposalId);
  const { governanceMode } = useEntityContext();
  const { proposals, isLoading, error } = useGovernance();

  if (governanceMode === GovernanceMode.None) {
    return (
      <div className="flex items-center justify-center p-12">
        <p className="text-sm text-gray-500">该实体未启用 DAO 治理</p>
      </div>
    );
  }

  if (isLoading) {
    return <div className="flex items-center justify-center p-12 text-sm text-gray-500">{t('loading')}</div>;
  }

  if (error) {
    return <div className="flex items-center justify-center p-12 text-sm text-red-500">加载失败: {String(error)}</div>;
  }

  const proposal = proposals.find((p) => p.id === proposalId);

  if (!proposal) {
    return <div className="flex items-center justify-center p-12 text-sm text-gray-500">提案不存在</div>;
  }

  const result = computeProposalResult(proposal);
  const totalVotes = Number(proposal.votesApprove) + Number(proposal.votesReject) + Number(proposal.votesAbstain);
  const isVotingOpen = proposal.status === 'Active' || proposal.status === 'Pending';

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

  return (
    <div className="mx-auto max-w-4xl space-y-6 p-4 sm:p-6">
      <Link href={`/${entityId}/governance`} className="text-sm text-blue-600 hover:text-blue-700">
        ← 返回提案列表
      </Link>

      {/* Proposal Info */}
      <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
        <div className="mb-4 flex items-center gap-3">
          <h1 className="text-xl font-bold text-gray-900 dark:text-gray-100">#{proposal.id} {proposal.title}</h1>
          <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${STATUS_BADGE[proposal.status] ?? STATUS_BADGE.Pending}`}>
            {STATUS_LABEL[proposal.status] ?? proposal.status}
          </span>
          {proposal.executed && (
            <span className="rounded-full bg-purple-100 px-2 py-0.5 text-xs font-medium text-purple-800 dark:bg-purple-900/30 dark:text-purple-300">
              已执行
            </span>
          )}
        </div>

        <div className="space-y-2 text-sm text-gray-600 dark:text-gray-400">
          <p>提案类型: {proposal.proposalType}</p>
          <p>提案人: {shortAddr(proposal.proposer)}</p>
          <p>截止区块: #{proposal.endBlock}</p>
          {proposal.description && <p className="mt-3 text-gray-700 dark:text-gray-300">{proposal.description}</p>}
        </div>
      </section>

      {/* Vote Progress */}
      <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
        <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">投票进度</h2>

        <div className="space-y-4">
          <VoteBar label="赞成" count={proposal.votesApprove} total={totalVotes} color="bg-green-500" />
          <VoteBar label="反对" count={proposal.votesReject} total={totalVotes} color="bg-red-500" />
          <VoteBar label="弃权" count={proposal.votesAbstain} total={totalVotes} color="bg-gray-400" />
        </div>

        <div className="mt-4 grid grid-cols-2 gap-4 border-t border-gray-100 pt-4 dark:border-gray-700 sm:grid-cols-4">
          <div>
            <p className="text-xs text-gray-500">总投票数</p>
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100">{totalVotes}</p>
          </div>
          <div>
            <p className="text-xs text-gray-500">法定人数要求</p>
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100">{proposal.quorumPct}%</p>
          </div>
          <div>
            <p className="text-xs text-gray-500">通过阈值</p>
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100">{proposal.passThreshold}%</p>
          </div>
          <div>
            <p className="text-xs text-gray-500">当前支持率</p>
            <p className={`text-sm font-medium ${result.passed ? 'text-green-600' : 'text-red-600'}`}>
              {result.approvalPct.toFixed(1)}%
            </p>
          </div>
        </div>

        {/* Result Summary */}
        <div className="mt-4 flex gap-3">
          <span className={`rounded-full px-3 py-1 text-xs font-medium ${result.quorumMet ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300' : 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-300'}`}>
            法定人数: {result.quorumMet ? '已达到' : '未达到'}
          </span>
          <span className={`rounded-full px-3 py-1 text-xs font-medium ${result.passed ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300' : 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300'}`}>
            投票结果: {result.passed ? '通过' : '未通过'}
          </span>
        </div>
      </section>

      {/* Voting Actions */}
      {isVotingOpen && (
        <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
          <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">参与投票</h2>
          <VotingActions proposalId={proposalId} />
        </section>
      )}

      {/* Admin Actions */}
      <PermissionGuard required={AdminPermission.GOVERNANCE_MANAGE} fallback={null}>
        <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
          <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">管理操作</h2>
          <AdminActions proposalId={proposalId} status={proposal.status} executed={proposal.executed} />
        </section>
      </PermissionGuard>
    </div>
  );
}
