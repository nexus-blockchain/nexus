'use client';

import React, { useState, useCallback } from 'react';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useDisclosure } from '@/hooks/use-disclosure';
import { PermissionGuard } from '@/components/permission-guard';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { AdminPermission } from '@/lib/types/models';
import { DisclosureLevel, DisclosureStatus, InsiderRole } from '@/lib/types/enums';
import { useCurrentBlock } from '@/hooks/use-current-block';

import { useTranslations } from 'next-intl';
// ─── Helpers ────────────────────────────────────────────────

function isTxBusy(m: { txState: { status: string } }): boolean {
  return m.txState.status === 'signing' || m.txState.status === 'broadcasting';
}

function shortAddr(addr: string): string {
  return addr.length > 12 ? `${addr.slice(0, 6)}…${addr.slice(-6)}` : addr;
}

const DISCLOSURE_LEVELS: { key: DisclosureLevel; label: string; desc: string }[] = [
  { key: DisclosureLevel.Basic, label: '基础', desc: '基本信息披露' },
  { key: DisclosureLevel.Standard, label: '标准', desc: '标准财务披露' },
  { key: DisclosureLevel.Enhanced, label: '增强', desc: '增强合规披露' },
  { key: DisclosureLevel.Full, label: '完整', desc: '全面透明披露' },
];

const STATUS_BADGE: Record<DisclosureStatus, string> = {
  [DisclosureStatus.Draft]: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300',
  [DisclosureStatus.Published]: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300',
  [DisclosureStatus.Withdrawn]: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300',
  [DisclosureStatus.Corrected]: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-300',
};

const STATUS_LABEL: Record<DisclosureStatus, string> = {
  [DisclosureStatus.Draft]: '草稿',
  [DisclosureStatus.Published]: '已发布',
  [DisclosureStatus.Withdrawn]: '已撤回',
  [DisclosureStatus.Corrected]: '已更正',
};

const INSIDER_ROLE_LABEL: Record<InsiderRole, string> = {
  [InsiderRole.Owner]: '所有者',
  [InsiderRole.Admin]: '管理员',
  [InsiderRole.Auditor]: '审计员',
  [InsiderRole.Advisor]: '顾问',
  [InsiderRole.MajorHolder]: '大额持有人',
};

// ─── Disclosure Level Config ────────────────────────────────

function LevelConfigSection() {
  const { entityId } = useEntityContext();
  const { disclosureLevel, setDisclosureLevel } = useDisclosure();

  const handleSetLevel = useCallback(
    (level: DisclosureLevel) => {
      setDisclosureLevel.mutate([entityId, level]);
    },
    [entityId, setDisclosureLevel],
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">披露等级配置</h2>
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        {DISCLOSURE_LEVELS.map(({ key, label, desc }) => (
          <button
            key={key}
            type="button"
            onClick={() => handleSetLevel(key)}
            disabled={isTxBusy(setDisclosureLevel)}
            className={`rounded-md border px-4 py-3 text-left transition disabled:cursor-not-allowed disabled:opacity-50 ${
              disclosureLevel === key
                ? 'border-blue-500 bg-blue-50 dark:border-blue-400 dark:bg-blue-900/20'
                : 'border-gray-200 hover:border-gray-300 dark:border-gray-700 dark:hover:border-gray-600'
            }`}
          >
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100">{label}</p>
            <p className="mt-1 text-xs text-gray-500">{desc}</p>
          </button>
        ))}
      </div>
      <TxStatusIndicator txState={setDisclosureLevel.txState} />
    </section>
  );
}

// ─── Blackout Period Indicator ───────────────────────────────

function BlackoutSection() {
  const { blackout, insiders } = useDisclosure();
  const currentBlock = useCurrentBlock();

  if (!blackout) return null;

  const isActive = currentBlock >= blackout.start && currentBlock <= blackout.end;

  return (
    <section className={`rounded-lg border p-6 ${
      isActive
        ? 'border-red-200 bg-red-50 dark:border-red-800 dark:bg-red-900/20'
        : 'border-yellow-200 bg-yellow-50 dark:border-yellow-800 dark:bg-yellow-900/20'
    }`}>
      <h2 className={`mb-2 text-lg font-semibold ${
        isActive ? 'text-red-800 dark:text-red-300' : 'text-yellow-800 dark:text-yellow-300'
      }`}>
        {isActive ? '⚠️ 黑窗口期（生效中）' : '⏳ 黑窗口期（已配置）'}
      </h2>
      <p className={`text-sm ${isActive ? 'text-red-700 dark:text-red-400' : 'text-yellow-700 dark:text-yellow-400'}`}>
        {isActive
          ? `当前处于黑窗口期（区块 ${blackout.start} ~ ${blackout.end}，当前区块 ${currentBlock}），内幕人员禁止交易。`
          : `黑窗口期已配置（区块 ${blackout.start} ~ ${blackout.end}），当前区块 ${currentBlock}。`}
      </p>
      {isActive && insiders.length > 0 && (
        <div className="mt-3 rounded-md border border-red-300 bg-red-100 p-3 dark:border-red-700 dark:bg-red-900/30">
          <p className="text-xs font-medium text-red-800 dark:text-red-300">受限内幕人员（禁止代币交易）:</p>
          <div className="mt-1 flex flex-wrap gap-2">
            {insiders.map((i) => (
              <span key={i.account} className="rounded-full bg-red-200 px-2 py-0.5 text-xs text-red-800 dark:bg-red-800 dark:text-red-200">
                {shortAddr(i.account)} ({INSIDER_ROLE_LABEL[i.role]})
              </span>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}

// ─── Disclosure Lifecycle ───────────────────────────────────

function DisclosureListSection() {
  const { entityId } = useEntityContext();
  const {
    disclosures, createDraftDisclosure, updateDraft, publishDraft,
    withdrawDisclosure, correctDisclosure,
  } = useDisclosure();

  const [title, setTitle] = useState('');
  const [contentCid, setContentCid] = useState('');
  const [level, setLevel] = useState<DisclosureLevel>(DisclosureLevel.Basic);
  const [editId, setEditId] = useState<number | null>(null);
  const [editTitle, setEditTitle] = useState('');
  const [editCid, setEditCid] = useState('');
  const [correctId, setCorrectId] = useState<number | null>(null);
  const [correctCid, setCorrectCid] = useState('');

  const handleCreate = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!title.trim() || !contentCid.trim()) return;
      createDraftDisclosure.mutate([entityId, title.trim(), contentCid.trim(), level]);
      setTitle('');
      setContentCid('');
    },
    [entityId, title, contentCid, level, createDraftDisclosure],
  );

  const handleUpdate = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (editId === null || !editTitle.trim() || !editCid.trim()) return;
      updateDraft.mutate([entityId, editId, editTitle.trim(), editCid.trim()]);
      setEditId(null);
    },
    [entityId, editId, editTitle, editCid, updateDraft],
  );

  const handleCorrect = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (correctId === null || !correctCid.trim()) return;
      correctDisclosure.mutate([entityId, correctId, correctCid.trim()]);
      setCorrectId(null);
    },
    [entityId, correctId, correctCid, correctDisclosure],
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">披露管理</h2>

      <form onSubmit={handleCreate} className="mb-6 space-y-3 border-b border-gray-100 pb-6 dark:border-gray-700">
        <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">创建草稿</h3>
        <div className="grid gap-3 sm:grid-cols-2">
          <input type="text" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="标题"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
          <input type="text" value={contentCid} onChange={(e) => setContentCid(e.target.value)} placeholder="内容 CID"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
        </div>
        <div className="flex items-center gap-3">
          <select value={level} onChange={(e) => setLevel(e.target.value as DisclosureLevel)}
            className="rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100">
            {DISCLOSURE_LEVELS.map(({ key, label }) => (
              <option key={key} value={key}>{label}</option>
            ))}
          </select>
          <button type="submit" disabled={isTxBusy(createDraftDisclosure)}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">
            创建草稿
          </button>
          <TxStatusIndicator txState={createDraftDisclosure.txState} />
        </div>
      </form>

      {disclosures.length === 0 ? (
        <p className="py-8 text-center text-sm text-gray-400">暂无披露记录</p>
      ) : (
        <div className="space-y-3">
          {disclosures.map((d) => (
            <div key={d.id} className="rounded-md border border-gray-100 p-4 dark:border-gray-700">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <span className="text-sm font-medium text-gray-900 dark:text-gray-100">#{d.id} {d.title}</span>
                  <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${STATUS_BADGE[d.status]}`}>
                    {STATUS_LABEL[d.status]}
                  </span>
                </div>
                <div className="flex gap-1">
                  {d.status === DisclosureStatus.Draft && (
                    <>
                      <button type="button" onClick={() => { setEditId(d.id); setEditTitle(d.title); setEditCid(d.contentCid); }}
                        className="rounded-md bg-gray-500 px-2 py-1 text-xs text-white hover:bg-gray-600">编辑</button>
                      <button type="button" onClick={() => publishDraft.mutate([entityId, d.id])} disabled={isTxBusy(publishDraft)}
                        className="rounded-md bg-green-600 px-2 py-1 text-xs text-white hover:bg-green-700 disabled:cursor-not-allowed disabled:opacity-50">发布</button>
                    </>
                  )}
                  {d.status === DisclosureStatus.Published && (
                    <>
                      <button type="button" onClick={() => withdrawDisclosure.mutate([entityId, d.id])} disabled={isTxBusy(withdrawDisclosure)}
                        className="rounded-md bg-red-500 px-2 py-1 text-xs text-white hover:bg-red-600 disabled:cursor-not-allowed disabled:opacity-50">撤回</button>
                      <button type="button" onClick={() => { setCorrectId(d.id); setCorrectCid(''); }}
                        className="rounded-md bg-yellow-600 px-2 py-1 text-xs text-white hover:bg-yellow-700">更正</button>
                    </>
                  )}
                </div>
              </div>
              <p className="mt-1 text-xs text-gray-500">CID: {d.contentCid} · 等级: {d.level}</p>
            </div>
          ))}
        </div>
      )}

      {editId !== null && (
        <form onSubmit={handleUpdate} className="mt-4 space-y-3 border-t border-gray-100 pt-4 dark:border-gray-700">
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">编辑草稿 #{editId}</h3>
          <div className="grid gap-3 sm:grid-cols-2">
            <input type="text" value={editTitle} onChange={(e) => setEditTitle(e.target.value)} placeholder="标题"
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
            <input type="text" value={editCid} onChange={(e) => setEditCid(e.target.value)} placeholder="内容 CID"
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
          </div>
          <div className="flex items-center gap-3">
            <button type="submit" disabled={isTxBusy(updateDraft)}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">更新草稿</button>
            <button type="button" onClick={() => setEditId(null)} className="rounded-md bg-gray-400 px-4 py-2 text-sm text-white hover:bg-gray-500">取消</button>
            <TxStatusIndicator txState={updateDraft.txState} />
          </div>
        </form>
      )}

      {correctId !== null && (
        <form onSubmit={handleCorrect} className="mt-4 space-y-3 border-t border-gray-100 pt-4 dark:border-gray-700">
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">更正披露 #{correctId}</h3>
          <input type="text" value={correctCid} onChange={(e) => setCorrectCid(e.target.value)} placeholder="更正内容 CID"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
          <div className="flex items-center gap-3">
            <button type="submit" disabled={isTxBusy(correctDisclosure)}
              className="rounded-md bg-yellow-600 px-4 py-2 text-sm text-white hover:bg-yellow-700 disabled:cursor-not-allowed disabled:opacity-50">提交更正</button>
            <button type="button" onClick={() => setCorrectId(null)} className="rounded-md bg-gray-400 px-4 py-2 text-sm text-white hover:bg-gray-500">取消</button>
            <TxStatusIndicator txState={correctDisclosure.txState} />
          </div>
        </form>
      )}

      <div className="mt-3 flex items-center gap-3">
        <TxStatusIndicator txState={publishDraft.txState} />
        <TxStatusIndicator txState={withdrawDisclosure.txState} />
      </div>
    </section>
  );
}

// ─── Insider Management ─────────────────────────────────────

function InsiderSection() {
  const { entityId } = useEntityContext();
  const { insiders, addInsider, updateInsiderRole, removeInsider } = useDisclosure();

  const [account, setAccount] = useState('');
  const [role, setRole] = useState<InsiderRole>(InsiderRole.Admin);

  const handleAdd = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!account.trim()) return;
      addInsider.mutate([entityId, account.trim(), role]);
      setAccount('');
    },
    [entityId, account, role, addInsider],
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">内幕人员管理</h2>

      <form onSubmit={handleAdd} className="mb-4 flex flex-wrap items-center gap-3">
        <input type="text" value={account} onChange={(e) => setAccount(e.target.value)} placeholder="账户地址"
          className="w-64 rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
        <select value={role} onChange={(e) => setRole(e.target.value as InsiderRole)}
          className="rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100">
          {Object.values(InsiderRole).map((r) => (
            <option key={r} value={r}>{INSIDER_ROLE_LABEL[r]}</option>
          ))}
        </select>
        <button type="submit" disabled={isTxBusy(addInsider)}
          className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">
          添加
        </button>
        <TxStatusIndicator txState={addInsider.txState} />
      </form>

      {insiders.length === 0 ? (
        <p className="py-4 text-center text-sm text-gray-400">暂无内幕人员</p>
      ) : (
        <div className="space-y-2">
          {insiders.map((ins) => (
            <div key={ins.account} className="flex items-center justify-between rounded-md border border-gray-100 px-4 py-2 dark:border-gray-700">
              <div className="flex items-center gap-3">
                <span className="font-mono text-sm text-gray-900 dark:text-gray-100">{shortAddr(ins.account)}</span>
                <span className="rounded-full bg-blue-100 px-2 py-0.5 text-xs font-medium text-blue-800 dark:bg-blue-900/30 dark:text-blue-300">
                  {INSIDER_ROLE_LABEL[ins.role]}
                </span>
              </div>
              <div className="flex gap-1">
                {Object.values(InsiderRole).filter((r) => r !== ins.role).map((r) => (
                  <button key={r} type="button" onClick={() => updateInsiderRole.mutate([entityId, ins.account, r])}
                    disabled={isTxBusy(updateInsiderRole)}
                    className="rounded-md bg-gray-200 px-2 py-1 text-xs text-gray-700 hover:bg-gray-300 disabled:cursor-not-allowed disabled:opacity-50 dark:bg-gray-700 dark:text-gray-300">
                    → {INSIDER_ROLE_LABEL[r]}
                  </button>
                ))}
                <button type="button" onClick={() => removeInsider.mutate([entityId, ins.account])}
                  disabled={isTxBusy(removeInsider)}
                  className="rounded-md bg-red-600 px-2 py-1 text-xs text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50">
                  移除
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
      <div className="mt-3 flex items-center gap-3">
        <TxStatusIndicator txState={updateInsiderRole.txState} />
        <TxStatusIndicator txState={removeInsider.txState} />
      </div>
    </section>
  );
}

// ─── Announcement Management ────────────────────────────────

function AnnouncementSection() {
  const { entityId } = useEntityContext();
  const { announcements, publishAnnouncement, updateAnnouncement, withdrawAnnouncement, pinAnnouncement } = useDisclosure();

  const [title, setTitle] = useState('');
  const [contentCid, setContentCid] = useState('');
  const [category, setCategory] = useState('');
  const [expiresAt, setExpiresAt] = useState('');
  const [editId, setEditId] = useState<number | null>(null);
  const [editTitle, setEditTitle] = useState('');
  const [editCid, setEditCid] = useState('');

  const handlePublish = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!title.trim() || !contentCid.trim() || !category.trim()) return;
      publishAnnouncement.mutate([entityId, title.trim(), contentCid.trim(), category.trim(), expiresAt ? Number(expiresAt) : null]);
      setTitle('');
      setContentCid('');
      setCategory('');
      setExpiresAt('');
    },
    [entityId, title, contentCid, category, expiresAt, publishAnnouncement],
  );

  const handleUpdate = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (editId === null || !editTitle.trim() || !editCid.trim()) return;
      updateAnnouncement.mutate([entityId, editId, editTitle.trim(), editCid.trim()]);
      setEditId(null);
    },
    [entityId, editId, editTitle, editCid, updateAnnouncement],
  );

  const sorted = [...announcements].sort((a, b) => (b.pinned ? 1 : 0) - (a.pinned ? 1 : 0));

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">公告管理</h2>

      <form onSubmit={handlePublish} className="mb-6 space-y-3 border-b border-gray-100 pb-6 dark:border-gray-700">
        <div className="grid gap-3 sm:grid-cols-2">
          <input type="text" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="公告标题"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
          <input type="text" value={contentCid} onChange={(e) => setContentCid(e.target.value)} placeholder="内容 CID"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
          <input type="text" value={category} onChange={(e) => setCategory(e.target.value)} placeholder="分类"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
          <input type="number" value={expiresAt} onChange={(e) => setExpiresAt(e.target.value)} placeholder="过期区块 (可选)"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
        </div>
        <div className="flex items-center gap-3">
          <button type="submit" disabled={isTxBusy(publishAnnouncement)}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">
            发布公告
          </button>
          <TxStatusIndicator txState={publishAnnouncement.txState} />
        </div>
      </form>

      {sorted.length === 0 ? (
        <p className="py-8 text-center text-sm text-gray-400">暂无公告</p>
      ) : (
        <div className="space-y-2">
          {sorted.map((a) => (
            <div key={a.id} className="rounded-md border border-gray-100 p-4 dark:border-gray-700">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  {a.pinned && <span className="text-xs text-orange-500">📌</span>}
                  <span className="text-sm font-medium text-gray-900 dark:text-gray-100">{a.title}</span>
                  <span className="rounded-full bg-gray-100 px-2 py-0.5 text-xs text-gray-600 dark:bg-gray-700 dark:text-gray-400">{a.category}</span>
                </div>
                <div className="flex gap-1">
                  <button type="button" onClick={() => pinAnnouncement.mutate([entityId, a.id])} disabled={isTxBusy(pinAnnouncement)}
                    className="rounded-md bg-orange-500 px-2 py-1 text-xs text-white hover:bg-orange-600 disabled:cursor-not-allowed disabled:opacity-50">
                    {a.pinned ? '取消置顶' : '置顶'}
                  </button>
                  <button type="button" onClick={() => { setEditId(a.id); setEditTitle(a.title); setEditCid(a.contentCid); }}
                    className="rounded-md bg-gray-500 px-2 py-1 text-xs text-white hover:bg-gray-600">编辑</button>
                  <button type="button" onClick={() => withdrawAnnouncement.mutate([entityId, a.id])} disabled={isTxBusy(withdrawAnnouncement)}
                    className="rounded-md bg-red-500 px-2 py-1 text-xs text-white hover:bg-red-600 disabled:cursor-not-allowed disabled:opacity-50">撤回</button>
                </div>
              </div>
              <p className="mt-1 text-xs text-gray-500">
                CID: {a.contentCid}
                {a.expiresAt && <> · 过期: 区块 #{a.expiresAt}</>}
              </p>
            </div>
          ))}
        </div>
      )}

      {editId !== null && (
        <form onSubmit={handleUpdate} className="mt-4 space-y-3 border-t border-gray-100 pt-4 dark:border-gray-700">
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">编辑公告 #{editId}</h3>
          <div className="grid gap-3 sm:grid-cols-2">
            <input type="text" value={editTitle} onChange={(e) => setEditTitle(e.target.value)} placeholder="标题"
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
            <input type="text" value={editCid} onChange={(e) => setEditCid(e.target.value)} placeholder="内容 CID"
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
          </div>
          <div className="flex items-center gap-3">
            <button type="submit" disabled={isTxBusy(updateAnnouncement)}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">更新公告</button>
            <button type="button" onClick={() => setEditId(null)} className="rounded-md bg-gray-400 px-4 py-2 text-sm text-white hover:bg-gray-500">取消</button>
            <TxStatusIndicator txState={updateAnnouncement.txState} />
          </div>
        </form>
      )}

      <div className="mt-3 flex items-center gap-3">
        <TxStatusIndicator txState={pinAnnouncement.txState} />
        <TxStatusIndicator txState={withdrawAnnouncement.txState} />
      </div>
    </section>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function DisclosurePage() {
  const t = useTranslations('disclosure');
  const { isLoading, error } = useDisclosure();

  if (isLoading) {
    return <div className="flex items-center justify-center p-12 text-sm text-gray-500">{t('loading')}</div>;
  }

  if (error) {
    return <div className="flex items-center justify-center p-12 text-sm text-red-500">加载失败: {String(error)}</div>;
  }

  return (
    <div className="mx-auto max-w-5xl space-y-6 p-4 sm:p-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('title')}</h1>

      <BlackoutSection />

      <PermissionGuard required={AdminPermission.DISCLOSURE_MANAGE} fallback={null}>
        <LevelConfigSection />
        <DisclosureListSection />
        <InsiderSection />
        <AnnouncementSection />
      </PermissionGuard>
    </div>
  );
}
