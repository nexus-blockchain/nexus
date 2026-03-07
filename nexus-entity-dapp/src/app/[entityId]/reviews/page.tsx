'use client';

import React, { useState, useCallback } from 'react';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useReview, validateRating } from '@/hooks/use-review';
import { PermissionGuard } from '@/components/permission-guard';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { AdminPermission } from '@/lib/types/models';

import { useTranslations } from 'next-intl';
// ─── Helpers ────────────────────────────────────────────────

function isTxBusy(m: { txState: { status: string } }): boolean {
  return m.txState.status === 'signing' || m.txState.status === 'broadcasting';
}

function shortAddr(addr: string): string {
  return addr.length > 12 ? `${addr.slice(0, 6)}…${addr.slice(-6)}` : addr;
}

function StarDisplay({ rating }: { rating: number }) {
  return (
    <span className="text-yellow-500">
      {Array.from({ length: 5 }, (_, i) => (i < rating ? '★' : '☆')).join('')}
    </span>
  );
}

// ─── Star Rating Selector ───────────────────────────────────

function StarSelector({ value, onChange }: { value: number; onChange: (v: number) => void }) {
  return (
    <div className="flex gap-1">
      {[1, 2, 3, 4, 5].map((star) => (
        <button key={star} type="button" onClick={() => onChange(star)}
          className={`text-2xl ${star <= value ? 'text-yellow-500' : 'text-gray-300 dark:text-gray-600'}`}>
          {star <= value ? '★' : '☆'}
        </button>
      ))}
    </div>
  );
}

// ─── Review Toggle (Admin) ──────────────────────────────────

function ReviewToggleSection() {
  const { entityId } = useEntityContext();
  const { reviewEnabled, setReviewEnabled } = useReview();

  const handleToggle = useCallback(() => {
    setReviewEnabled.mutate([entityId, !reviewEnabled]);
  }, [entityId, reviewEnabled, setReviewEnabled]);

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">评价开关</h2>
          <p className="mt-1 text-sm text-gray-500">
            当前状态：
            <span className={reviewEnabled ? 'text-green-600' : 'text-red-500'}>
              {reviewEnabled ? '已开启' : '已关闭'}
            </span>
          </p>
        </div>
        <div className="flex items-center gap-3">
          <button type="button" onClick={handleToggle} disabled={isTxBusy(setReviewEnabled)}
            className={`rounded-md px-4 py-2 text-sm text-white disabled:cursor-not-allowed disabled:opacity-50 ${
              reviewEnabled
                ? 'bg-red-500 hover:bg-red-600'
                : 'bg-green-600 hover:bg-green-700'
            }`}>
            {reviewEnabled ? '关闭评价' : '开启评价'}
          </button>
          <TxStatusIndicator txState={setReviewEnabled.txState} />
        </div>
      </div>
    </section>
  );
}


// ─── Submit Review Form ─────────────────────────────────────

function SubmitReviewForm() {
  const { entityId } = useEntityContext();
  const { submitReview } = useReview();
  const [orderId, setOrderId] = useState('');
  const [rating, setRating] = useState(0);
  const [contentCid, setContentCid] = useState('');
  const [error, setError] = useState('');

  const handleSubmit = useCallback(() => {
    setError('');
    if (!orderId.trim()) { setError('请输入订单ID'); return; }
    if (!validateRating(rating)) { setError('请选择1-5星评分'); return; }
    if (!contentCid.trim()) { setError('请输入内容CID'); return; }
    submitReview.mutate([entityId, Number(orderId), rating, contentCid.trim()]);
    setOrderId('');
    setRating(0);
    setContentCid('');
  }, [entityId, orderId, rating, contentCid, submitReview]);

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">提交评价</h2>
      <p className="mb-3 text-xs text-gray-500">仅限已完成订单可评价</p>
      <div className="space-y-3">
        <div>
          <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">订单 ID</label>
          <input type="number" value={orderId} onChange={(e) => setOrderId(e.target.value)} placeholder="已完成的订单ID"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
        </div>
        <div>
          <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">评分</label>
          <StarSelector value={rating} onChange={setRating} />
        </div>
        <div>
          <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">内容 CID (IPFS)</label>
          <input type="text" value={contentCid} onChange={(e) => setContentCid(e.target.value)} placeholder="QmXxx..."
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
        </div>
        {error && <p className="text-sm text-red-500">{error}</p>}
        <div className="flex items-center gap-3">
          <button type="button" onClick={handleSubmit} disabled={isTxBusy(submitReview)}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">
            提交评价
          </button>
          <TxStatusIndicator txState={submitReview.txState} />
        </div>
      </div>
    </section>
  );
}

// ─── Review List ────────────────────────────────────────────

function ReviewListSection() {
  const { reviews } = useReview();

  if (reviews.length === 0) {
    return (
      <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
        <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">评价列表</h2>
        <p className="py-8 text-center text-sm text-gray-400">暂无评价</p>
      </section>
    );
  }

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">评价列表</h2>
      <div className="space-y-3">
        {reviews.map((r) => (
          <div key={r.id} className="rounded-md border border-gray-100 p-4 dark:border-gray-700">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <span className="text-sm font-medium text-gray-900 dark:text-gray-100">订单 #{r.orderId}</span>
                <StarDisplay rating={r.rating} />
              </div>
              <span className="font-mono text-xs text-gray-500">{shortAddr(r.reviewer)}</span>
            </div>
            <p className="mt-1 text-xs text-gray-500">CID: {r.contentCid}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function ReviewsPage() {
  const t = useTranslations('reviews');
  const { isLoading, error } = useReview();

  if (isLoading) {
    return <div className="flex items-center justify-center p-12 text-sm text-gray-500">{t('loading')}</div>;
  }

  if (error) {
    return <div className="flex items-center justify-center p-12 text-sm text-red-500">加载失败: {String(error)}</div>;
  }

  return (
    <div className="mx-auto max-w-5xl space-y-6 p-4 sm:p-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('title')}</h1>

      <PermissionGuard required={AdminPermission.REVIEW_MANAGE} fallback={null}>
        <ReviewToggleSection />
      </PermissionGuard>

      <SubmitReviewForm />
      <ReviewListSection />
    </div>
  );
}
