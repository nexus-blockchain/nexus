'use client';

import React, { useState, useCallback } from 'react';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useMembers } from '@/hooks/use-members';
import { PermissionGuard } from '@/components/permission-guard';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { AdminPermission } from '@/lib/types/models';
import { UpgradeTrigger } from '@/lib/types/enums';

import { useTranslations } from 'next-intl';
// ─── Helpers ────────────────────────────────────────────────

function isTxBusy(m: { txState: { status: string } }): boolean {
  return m.txState.status === 'signing' || m.txState.status === 'broadcasting';
}

const TRIGGER_LABELS: Record<UpgradeTrigger, string> = {
  [UpgradeTrigger.PurchaseProduct]: '购买商品',
  [UpgradeTrigger.SingleOrder]: '单笔订单金额',
  [UpgradeTrigger.TotalSpent]: '累计消费',
  [UpgradeTrigger.OrderCount]: '订单数量',
  [UpgradeTrigger.TotalSpentUsdt]: '累计消费 (USDT)',
  [UpgradeTrigger.SingleOrderUsdt]: '单笔订单 (USDT)',
  [UpgradeTrigger.ReferralCount]: '直推人数',
  [UpgradeTrigger.TeamSize]: '团队人数',
  [UpgradeTrigger.ReferralLevelCount]: '推荐等级人数',
};

// ─── Level Form ─────────────────────────────────────────────

interface LevelFormData {
  name: string;
  threshold: string;
  discountRate: string;
  commissionBonus: string;
}

const EMPTY_FORM: LevelFormData = { name: '', threshold: '', discountRate: '', commissionBonus: '' };

function LevelForm({ initial, onSubmit, submitLabel, busy }: {
  initial?: LevelFormData;
  onSubmit: (data: LevelFormData) => void;
  submitLabel: string;
  busy: boolean;
}) {
  const [form, setForm] = useState<LevelFormData>(initial ?? EMPTY_FORM);

  const handleSubmit = useCallback((e: React.FormEvent) => {
    e.preventDefault();
    onSubmit(form);
    if (!initial) setForm(EMPTY_FORM);
  }, [form, onSubmit, initial]);

  return (
    <form onSubmit={handleSubmit} className="grid grid-cols-2 gap-3 sm:grid-cols-4">
      <input
        type="text"
        value={form.name}
        onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
        placeholder="等级名称"
        className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
        required
      />
      <input
        type="text"
        inputMode="decimal"
        value={form.threshold}
        onChange={(e) => setForm((f) => ({ ...f, threshold: e.target.value }))}
        placeholder="升级阈值"
        className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
        required
      />
      <input
        type="number"
        value={form.discountRate}
        onChange={(e) => setForm((f) => ({ ...f, discountRate: e.target.value }))}
        placeholder="折扣率"
        className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
        required
      />
      <div className="flex gap-2">
        <input
          type="number"
          value={form.commissionBonus}
          onChange={(e) => setForm((f) => ({ ...f, commissionBonus: e.target.value }))}
          placeholder="佣金加成"
          className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
          required
        />
        <button
          type="submit"
          disabled={busy}
          className="whitespace-nowrap rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {submitLabel}
        </button>
      </div>
    </form>
  );
}

// ─── Custom Levels Section ──────────────────────────────────

function CustomLevelsSection() {
  const { entityId } = useEntityContext();
  const { customLevels, initializeLevels, addCustomLevel, updateCustomLevel, deleteCustomLevel } = useMembers();
  const [editIndex, setEditIndex] = useState<number | null>(null);

  const handleAdd = useCallback((data: LevelFormData) => {
    addCustomLevel.mutate([entityId, data.name, data.threshold, Number(data.discountRate), Number(data.commissionBonus)]);
  }, [entityId, addCustomLevel]);

  const handleUpdate = useCallback((index: number, data: LevelFormData) => {
    updateCustomLevel.mutate([entityId, index, data.name, data.threshold, Number(data.discountRate), Number(data.commissionBonus)]);
    setEditIndex(null);
  }, [entityId, updateCustomLevel]);

  const handleDelete = useCallback((index: number) => {
    deleteCustomLevel.mutate([entityId, index]);
  }, [entityId, deleteCustomLevel]);

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">自定义等级</h2>
        {customLevels.length === 0 && (
          <button
            type="button"
            onClick={() => initializeLevels.mutate([entityId])}
            disabled={isTxBusy(initializeLevels)}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            初始化等级系统
          </button>
        )}
      </div>

      <TxStatusIndicator txState={initializeLevels.txState} />

      {customLevels.length > 0 && (
        <div className="space-y-3">
          {/* Header */}
          <div className="hidden grid-cols-5 gap-3 text-xs font-medium text-gray-500 sm:grid">
            <span>名称</span><span>阈值</span><span>折扣率</span><span>佣金加成</span><span>操作</span>
          </div>

          {customLevels.map((level, idx) => (
            <div key={idx}>
              {editIndex === idx ? (
                <LevelForm
                  initial={{ name: level.name, threshold: level.threshold.toString(), discountRate: String(level.discountRate), commissionBonus: String(level.commissionBonus) }}
                  onSubmit={(data) => handleUpdate(idx, data)}
                  submitLabel="保存"
                  busy={isTxBusy(updateCustomLevel)}
                />
              ) : (
                <div className="grid grid-cols-5 items-center gap-3 rounded-md border border-gray-100 px-3 py-2 text-sm dark:border-gray-700">
                  <span className="font-medium text-gray-900 dark:text-gray-100">{level.name}</span>
                  <span className="text-gray-700 dark:text-gray-300">{level.threshold.toString()}</span>
                  <span className="text-gray-700 dark:text-gray-300">{level.discountRate}</span>
                  <span className="text-gray-700 dark:text-gray-300">{level.commissionBonus}</span>
                  <div className="flex gap-1">
                    <button type="button" onClick={() => setEditIndex(idx)} className="rounded-md bg-blue-600 px-2 py-1 text-xs text-white hover:bg-blue-700">编辑</button>
                    <button type="button" onClick={() => handleDelete(idx)} disabled={isTxBusy(deleteCustomLevel)} className="rounded-md bg-red-600 px-2 py-1 text-xs text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50">删除</button>
                  </div>
                </div>
              )}
            </div>
          ))}

          {/* Add new level */}
          <div className="border-t border-gray-100 pt-3 dark:border-gray-700">
            <p className="mb-2 text-sm font-medium text-gray-700 dark:text-gray-300">添加新等级</p>
            <LevelForm onSubmit={handleAdd} submitLabel="添加" busy={isTxBusy(addCustomLevel)} />
            <TxStatusIndicator txState={addCustomLevel.txState} />
          </div>
        </div>
      )}
    </section>
  );
}

// ─── Upgrade Triggers Section ───────────────────────────────

function UpgradeTriggersSection() {
  const { entityId } = useEntityContext();
  const { upgradeTriggers, setUpgradeTrigger } = useMembers();
  const [editTrigger, setEditTrigger] = useState<{ trigger: UpgradeTrigger; value: string } | null>(null);

  const triggerMap = new Map(upgradeTriggers.map((t) => [t.trigger, t.value]));

  const handleSave = useCallback(() => {
    if (editTrigger) {
      setUpgradeTrigger.mutate([entityId, editTrigger.trigger, editTrigger.value]);
      setEditTrigger(null);
    }
  }, [entityId, editTrigger, setUpgradeTrigger]);

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">升级触发规则</h2>
      <div className="space-y-2">
        {Object.values(UpgradeTrigger).map((trigger) => {
          const currentValue = triggerMap.get(trigger);
          const isEditing = editTrigger?.trigger === trigger;

          return (
            <div key={trigger} className="flex items-center justify-between rounded-md border border-gray-100 px-4 py-2 dark:border-gray-700">
              <span className="text-sm text-gray-700 dark:text-gray-300">{TRIGGER_LABELS[trigger]}</span>
              <div className="flex items-center gap-2">
                {isEditing ? (
                  <>
                    <input
                      type="text"
                      inputMode="decimal"
                      value={editTrigger.value}
                      onChange={(e) => setEditTrigger({ trigger, value: e.target.value })}
                      className="w-32 rounded-md border border-gray-300 px-2 py-1 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
                    />
                    <button type="button" onClick={handleSave} disabled={isTxBusy(setUpgradeTrigger)} className="rounded-md bg-blue-600 px-2 py-1 text-xs text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">保存</button>
                    <button type="button" onClick={() => setEditTrigger(null)} className="rounded-md bg-gray-400 px-2 py-1 text-xs text-white hover:bg-gray-500">取消</button>
                  </>
                ) : (
                  <>
                    <span className="font-mono text-sm text-gray-900 dark:text-gray-100">
                      {currentValue !== undefined ? currentValue.toString() : '未设置'}
                    </span>
                    <button type="button" onClick={() => setEditTrigger({ trigger, value: currentValue?.toString() ?? '0' })} className="rounded-md bg-blue-600 px-2 py-1 text-xs text-white hover:bg-blue-700">编辑</button>
                  </>
                )}
              </div>
            </div>
          );
        })}
      </div>
      <div className="mt-3">
        <TxStatusIndicator txState={setUpgradeTrigger.txState} />
      </div>
    </section>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function LevelsPage() {
  const t = useTranslations('members');
  return (
    <PermissionGuard required={AdminPermission.MEMBER_MANAGE}>
      <div className="mx-auto max-w-5xl space-y-6 p-4 sm:p-6">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('levels.title')}</h1>
        <CustomLevelsSection />
        <UpgradeTriggersSection />
      </div>
    </PermissionGuard>
  );
}
