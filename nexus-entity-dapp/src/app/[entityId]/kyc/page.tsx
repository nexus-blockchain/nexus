'use client';

import React, { useState, useCallback } from 'react';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useKyc } from '@/hooks/use-kyc';
import { PermissionGuard } from '@/components/permission-guard';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { AdminPermission } from '@/lib/types/models';
import { KycLevel, KycStatus } from '@/lib/types/enums';

import { useTranslations } from 'next-intl';
// ─── Helpers ────────────────────────────────────────────────

function isTxBusy(m: { txState: { status: string } }): boolean {
  return m.txState.status === 'signing' || m.txState.status === 'broadcasting';
}

function shortAddr(addr: string): string {
  return addr.length > 12 ? `${addr.slice(0, 6)}…${addr.slice(-6)}` : addr;
}

const KYC_LEVEL_LABEL: Record<number, string> = {
  [KycLevel.None]: '无',
  [KycLevel.Basic]: '基础',
  [KycLevel.Standard]: '标准',
  [KycLevel.Enhanced]: '增强',
  [KycLevel.Full]: '完整',
};

const KYC_STATUS_BADGE: Record<KycStatus, string> = {
  [KycStatus.NotSubmitted]: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300',
  [KycStatus.Pending]: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-300',
  [KycStatus.Approved]: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300',
  [KycStatus.Rejected]: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300',
  [KycStatus.Expired]: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300',
  [KycStatus.Revoked]: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300',
};

const KYC_STATUS_LABEL: Record<KycStatus, string> = {
  [KycStatus.NotSubmitted]: '未提交',
  [KycStatus.Pending]: '审核中',
  [KycStatus.Approved]: '已通过',
  [KycStatus.Rejected]: '已拒绝',
  [KycStatus.Expired]: '已过期',
  [KycStatus.Revoked]: '已撤销',
};

const KYC_LEVELS_SELECTABLE = [
  { value: KycLevel.Basic, label: '基础 (Lv.1)' },
  { value: KycLevel.Standard, label: '标准 (Lv.2)' },
  { value: KycLevel.Enhanced, label: '增强 (Lv.3)' },
  { value: KycLevel.Full, label: '完整 (Lv.4)' },
];

// ─── Status Flow Display ────────────────────────────────────

const STATUS_FLOW: KycStatus[] = [
  KycStatus.NotSubmitted,
  KycStatus.Pending,
  KycStatus.Approved,
  KycStatus.Rejected,
  KycStatus.Expired,
  KycStatus.Revoked,
];

function StatusFlowIndicator({ current }: { current: KycStatus }) {
  return (
    <div className="flex flex-wrap items-center gap-1 text-xs">
      {STATUS_FLOW.map((s, i) => (
        <React.Fragment key={s}>
          {i > 0 && <span className="text-gray-300 dark:text-gray-600">→</span>}
          <span className={`rounded-full px-2 py-0.5 font-medium ${current === s ? KYC_STATUS_BADGE[s] : 'bg-gray-50 text-gray-400 dark:bg-gray-800 dark:text-gray-600'}`}>
            {KYC_STATUS_LABEL[s]}
          </span>
        </React.Fragment>
      ))}
    </div>
  );
}

// ─── Requirement Display & Config ───────────────────────────

function RequirementSection() {
  const { entityId } = useEntityContext();
  const { requirement, setEntityRequirement } = useKyc();

  const [editing, setEditing] = useState(false);
  const [minLevel, setMinLevel] = useState<KycLevel>(requirement.minLevel);
  const [maxRiskScore, setMaxRiskScore] = useState(requirement.maxRiskScore);

  const handleSave = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      setEntityRequirement.mutate([entityId, minLevel, maxRiskScore]);
      setEditing(false);
    },
    [entityId, minLevel, maxRiskScore, setEntityRequirement],
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">KYC 要求</h2>
        <PermissionGuard required={AdminPermission.KYC_MANAGE} fallback={null}>
          {!editing && (
            <button type="button" onClick={() => { setMinLevel(requirement.minLevel); setMaxRiskScore(requirement.maxRiskScore); setEditing(true); }}
              className="rounded-md bg-gray-100 px-3 py-1 text-xs text-gray-700 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600">
              配置
            </button>
          )}
        </PermissionGuard>
      </div>

      {editing ? (
        <form onSubmit={handleSave} className="space-y-3">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="mb-1 block text-xs text-gray-500">最低等级</label>
              <select value={minLevel} onChange={(e) => setMinLevel(Number(e.target.value) as KycLevel)}
                className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100">
                <option value={KycLevel.None}>无 (Lv.0)</option>
                {KYC_LEVELS_SELECTABLE.map(({ value, label }) => (
                  <option key={value} value={value}>{label}</option>
                ))}
              </select>
            </div>
            <div>
              <label className="mb-1 block text-xs text-gray-500">最大风险评分</label>
              <input type="number" min={0} max={100} value={maxRiskScore} onChange={(e) => setMaxRiskScore(Number(e.target.value))}
                className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
            </div>
          </div>
          <div className="flex items-center gap-2">
            <button type="submit" disabled={isTxBusy(setEntityRequirement)}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">保存</button>
            <button type="button" onClick={() => setEditing(false)}
              className="rounded-md bg-gray-400 px-4 py-2 text-sm text-white hover:bg-gray-500">取消</button>
            <TxStatusIndicator txState={setEntityRequirement.txState} />
          </div>
        </form>
      ) : (
        <div className="grid grid-cols-2 gap-4">
          <div>
            <p className="text-xs text-gray-500">最低等级</p>
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100">
              {KYC_LEVEL_LABEL[requirement.minLevel] ?? '无'} (Lv.{requirement.minLevel})
            </p>
          </div>
          <div>
            <p className="text-xs text-gray-500">最大风险评分</p>
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100">{requirement.maxRiskScore}</p>
          </div>
        </div>
      )}
    </section>
  );
}

// ─── KYC Application Form ───────────────────────────────────

function ApplicationSection() {
  const { entityId } = useEntityContext();
  const { submitKyc } = useKyc();

  const [level, setLevel] = useState<KycLevel>(KycLevel.Basic);
  const [dataCid, setDataCid] = useState('');
  const [countryCode, setCountryCode] = useState('');

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!dataCid.trim()) return;
      const args: (string | number | null)[] = [entityId, level, dataCid.trim()];
      if (countryCode.trim()) args.push(countryCode.trim());
      else args.push(null);
      submitKyc.mutate(args);
      setDataCid('');
      setCountryCode('');
    },
    [entityId, level, dataCid, countryCode, submitKyc],
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">提交 KYC 申请</h2>
      <form onSubmit={handleSubmit} className="space-y-3">
        <div className="grid gap-3 sm:grid-cols-3">
          <div>
            <label className="mb-1 block text-xs text-gray-500">认证等级</label>
            <select value={level} onChange={(e) => setLevel(Number(e.target.value) as KycLevel)}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100">
              {KYC_LEVELS_SELECTABLE.map(({ value, label }) => (
                <option key={value} value={value}>{label}</option>
              ))}
            </select>
          </div>
          <div>
            <label className="mb-1 block text-xs text-gray-500">数据 IPFS CID</label>
            <input type="text" value={dataCid} onChange={(e) => setDataCid(e.target.value)} placeholder="Qm..."
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
          </div>
          <div>
            <label className="mb-1 block text-xs text-gray-500">国家代码 (可选)</label>
            <input type="text" value={countryCode} onChange={(e) => setCountryCode(e.target.value)} placeholder="CN, US, ..."
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
          </div>
        </div>
        <div className="flex items-center gap-3">
          <button type="submit" disabled={!dataCid.trim() || isTxBusy(submitKyc)}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">
            提交申请
          </button>
          <TxStatusIndicator txState={submitKyc.txState} />
        </div>
      </form>
    </section>
  );
}

// ─── KYC Management (Admin) ─────────────────────────────────

function ManagementSection() {
  const { entityId } = useEntityContext();
  const { kycRecords, approveKyc, rejectKyc, revokeKyc, updateKycData, purgeKycData } = useKyc();

  const [updateAccount, setUpdateAccount] = useState<string | null>(null);
  const [newCid, setNewCid] = useState('');

  const handleUpdate = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!updateAccount || !newCid.trim()) return;
      updateKycData.mutate([entityId, updateAccount, newCid.trim()]);
      setUpdateAccount(null);
      setNewCid('');
    },
    [entityId, updateAccount, newCid, updateKycData],
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">KYC 管理</h2>

      {kycRecords.length === 0 ? (
        <p className="py-8 text-center text-sm text-gray-400">暂无 KYC 记录</p>
      ) : (
        <div className="space-y-3">
          {kycRecords.map((r) => (
            <div key={r.account} className="rounded-md border border-gray-100 p-4 dark:border-gray-700">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-3">
                  <span className="font-mono text-sm text-gray-900 dark:text-gray-100">{shortAddr(r.account)}</span>
                  <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${KYC_STATUS_BADGE[r.status]}`}>
                    {KYC_STATUS_LABEL[r.status]}
                  </span>
                  <span className="text-xs text-gray-500">Lv.{r.level} {KYC_LEVEL_LABEL[r.level]}</span>
                </div>
                <div className="flex flex-wrap gap-1">
                  {r.status === KycStatus.Pending && (
                    <>
                      <button type="button" onClick={() => approveKyc.mutate([entityId, r.account])}
                        disabled={isTxBusy(approveKyc)}
                        className="rounded-md bg-green-600 px-2 py-1 text-xs text-white hover:bg-green-700 disabled:cursor-not-allowed disabled:opacity-50">
                        通过
                      </button>
                      <button type="button" onClick={() => rejectKyc.mutate([entityId, r.account])}
                        disabled={isTxBusy(rejectKyc)}
                        className="rounded-md bg-red-600 px-2 py-1 text-xs text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50">
                        拒绝
                      </button>
                    </>
                  )}
                  {r.status === KycStatus.Approved && (
                    <button type="button" onClick={() => revokeKyc.mutate([entityId, r.account])}
                      disabled={isTxBusy(revokeKyc)}
                      className="rounded-md bg-orange-600 px-2 py-1 text-xs text-white hover:bg-orange-700 disabled:cursor-not-allowed disabled:opacity-50">
                      撤销
                    </button>
                  )}
                  <button type="button" onClick={() => { setUpdateAccount(r.account); setNewCid(''); }}
                    className="rounded-md bg-gray-500 px-2 py-1 text-xs text-white hover:bg-gray-600">
                    更新数据
                  </button>
                  <button type="button" onClick={() => purgeKycData.mutate([entityId, r.account])}
                    disabled={isTxBusy(purgeKycData)}
                    className="rounded-md bg-red-600 px-2 py-1 text-xs text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50">
                    清除数据
                  </button>
                </div>
              </div>

              <div className="mt-2 grid grid-cols-2 gap-2 text-xs text-gray-500 sm:grid-cols-4">
                <span>风险评分: <span className="font-medium text-gray-700 dark:text-gray-300">{r.riskScore}</span></span>
                <span>CID: <span className="font-mono">{r.dataCid ? shortAddr(r.dataCid) : '—'}</span></span>
                {r.countryCode && <span>国家: {r.countryCode}</span>}
                {r.expiresAt && <span>过期: 区块 #{r.expiresAt}</span>}
                <span>提交: 区块 #{r.submittedAt}</span>
              </div>

              <div className="mt-2">
                <StatusFlowIndicator current={r.status} />
              </div>
            </div>
          ))}
        </div>
      )}

      {updateAccount && (
        <form onSubmit={handleUpdate} className="mt-4 space-y-3 border-t border-gray-100 pt-4 dark:border-gray-700">
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">更新 KYC 数据 — {shortAddr(updateAccount)}</h3>
          <input type="text" value={newCid} onChange={(e) => setNewCid(e.target.value)} placeholder="新数据 CID"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
          <div className="flex items-center gap-3">
            <button type="submit" disabled={!newCid.trim() || isTxBusy(updateKycData)}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">更新</button>
            <button type="button" onClick={() => setUpdateAccount(null)}
              className="rounded-md bg-gray-400 px-4 py-2 text-sm text-white hover:bg-gray-500">取消</button>
            <TxStatusIndicator txState={updateKycData.txState} />
          </div>
        </form>
      )}

      <div className="mt-3 flex flex-wrap items-center gap-3">
        <TxStatusIndicator txState={approveKyc.txState} />
        <TxStatusIndicator txState={rejectKyc.txState} />
        <TxStatusIndicator txState={revokeKyc.txState} />
        <TxStatusIndicator txState={purgeKycData.txState} />
      </div>
    </section>
  );
}

// ─── User KYC Status Section ────────────────────────────────

function UserKycSection() {
  const { kycRecords } = useKyc();

  if (kycRecords.length === 0) return null;

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">KYC 记录</h2>
      <div className="space-y-3">
        {kycRecords.map((r) => (
          <div key={r.account} className="rounded-md border border-gray-100 p-4 dark:border-gray-700">
            <div className="flex items-center gap-3">
              <span className="font-mono text-sm text-gray-900 dark:text-gray-100">{shortAddr(r.account)}</span>
              <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${KYC_STATUS_BADGE[r.status]}`}>
                {KYC_STATUS_LABEL[r.status]}
              </span>
              <span className="text-xs text-gray-500">Lv.{r.level} {KYC_LEVEL_LABEL[r.level]}</span>
            </div>
            <div className="mt-2">
              <StatusFlowIndicator current={r.status} />
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function KycPage() {
  const t = useTranslations('kyc');
  const { isLoading, error } = useKyc();

  if (isLoading) {
    return <div className="flex items-center justify-center p-12 text-sm text-gray-500">{t('loading')}</div>;
  }

  if (error) {
    return <div className="flex items-center justify-center p-12 text-sm text-red-500">加载失败: {String(error)}</div>;
  }

  return (
    <div className="mx-auto max-w-5xl space-y-6 p-4 sm:p-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('title')}</h1>

      <RequirementSection />
      <ApplicationSection />
      <UserKycSection />

      <PermissionGuard required={AdminPermission.KYC_MANAGE} fallback={null}>
        <ManagementSection />
      </PermissionGuard>
    </div>
  );
}
