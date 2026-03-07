'use client';

import React, { useState, useCallback } from 'react';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useTokensale, computeDutchAuctionPrice } from '@/hooks/use-tokensale';
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

const STATUS_BADGE: Record<string, string> = {
  Created: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300',
  Started: 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-300',
  Subscribing: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-300',
  Ended: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300',
  Claiming: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300',
};

const STATUS_LABEL: Record<string, string> = {
  Created: '已创建',
  Started: '已开始',
  Subscribing: '认购中',
  Ended: '已结束',
  Claiming: '领取中',
};

// ─── Create Sale Round Form ─────────────────────────────────

function CreateSaleRoundForm() {
  const { entityId } = useEntityContext();
  const { createSaleRound } = useTokensale();
  const [open, setOpen] = useState(false);
  const [form, setForm] = useState({
    name: '', totalSupply: '', price: '', startBlock: '', endBlock: '',
    minPurchase: '', maxPurchase: '', softCap: '', hardCap: '',
  });

  const set = useCallback((k: string, v: string) => setForm((f) => ({ ...f, [k]: v })), []);

  const handleSubmit = useCallback(() => {
    const { name, totalSupply, price, startBlock, endBlock, minPurchase, maxPurchase, softCap, hardCap } = form;
    if (!name.trim() || !totalSupply || !price) return;
    createSaleRound.mutate([
      entityId, name.trim(), totalSupply, price,
      Number(startBlock), Number(endBlock),
      minPurchase, maxPurchase, softCap, hardCap,
    ]);
  }, [entityId, form, createSaleRound]);

  if (!open) {
    return (
      <button type="button" onClick={() => setOpen(true)}
        className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700">
        创建发售轮次
      </button>
    );
  }

  const fields = [
    { key: 'name', label: '名称', type: 'text', placeholder: '轮次名称' },
    { key: 'totalSupply', label: '总供应量', type: 'text', placeholder: '总供应量' },
    { key: 'price', label: '价格', type: 'text', placeholder: '单价' },
    { key: 'startBlock', label: '开始区块', type: 'number', placeholder: '开始区块号' },
    { key: 'endBlock', label: '结束区块', type: 'number', placeholder: '结束区块号' },
    { key: 'minPurchase', label: '最低购买', type: 'text', placeholder: '最低购买量' },
    { key: 'maxPurchase', label: '最高购买', type: 'text', placeholder: '最高购买量' },
    { key: 'softCap', label: '软顶', type: 'text', placeholder: '软顶金额' },
    { key: 'hardCap', label: '硬顶', type: 'text', placeholder: '硬顶金额' },
  ];

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">创建发售轮次</h2>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {fields.map(({ key, label, type, placeholder }) => (
          <div key={key}>
            <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">{label}</label>
            <input type={type} value={(form as any)[key]} onChange={(e) => set(key, e.target.value)}
              placeholder={placeholder}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
          </div>
        ))}
      </div>
      <div className="mt-4 flex items-center gap-3">
        <button type="button" onClick={handleSubmit} disabled={!form.name.trim() || isTxBusy(createSaleRound)}
          className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">
          提交
        </button>
        <button type="button" onClick={() => setOpen(false)}
          className="rounded-md border border-gray-300 px-4 py-2 text-sm text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:text-gray-300 dark:hover:bg-gray-800">
          取消
        </button>
        <TxStatusIndicator txState={createSaleRound.txState} />
      </div>
    </section>
  );
}

// ─── Dutch Auction Config ───────────────────────────────────

function DutchAuctionSection() {
  const { entityId } = useEntityContext();
  const { configureDutchAuction } = useTokensale();
  const [roundId, setRoundId] = useState('');
  const [startPrice, setStartPrice] = useState('');
  const [endPrice, setEndPrice] = useState('');
  const [decayBlocks, setDecayBlocks] = useState('');
  const [elapsed, setElapsed] = useState('');

  const calculatedPrice = startPrice && endPrice && decayBlocks && elapsed
    ? computeDutchAuctionPrice(
        { startPrice: BigInt(startPrice), endPrice: BigInt(endPrice), decayBlocks: Number(decayBlocks) },
        Number(elapsed),
      ).toString()
    : null;

  const handleSubmit = useCallback(() => {
    if (!roundId || !startPrice || !endPrice || !decayBlocks) return;
    configureDutchAuction.mutate([entityId, Number(roundId), startPrice, endPrice, Number(decayBlocks)]);
  }, [entityId, roundId, startPrice, endPrice, decayBlocks, configureDutchAuction]);

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">荷兰拍配置</h2>
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <div>
          <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">轮次 ID</label>
          <input type="number" value={roundId} onChange={(e) => setRoundId(e.target.value)} placeholder="轮次ID"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
        </div>
        <div>
          <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">起始价格</label>
          <input type="text" value={startPrice} onChange={(e) => setStartPrice(e.target.value)} placeholder="起始价格"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
        </div>
        <div>
          <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">结束价格</label>
          <input type="text" value={endPrice} onChange={(e) => setEndPrice(e.target.value)} placeholder="结束价格"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
        </div>
        <div>
          <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">衰减区块数</label>
          <input type="number" value={decayBlocks} onChange={(e) => setDecayBlocks(e.target.value)} placeholder="衰减区块"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
        </div>
      </div>

      {/* Price calculator */}
      <div className="mt-4 border-t border-gray-100 pt-4 dark:border-gray-700">
        <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">价格计算器 — 已过区块数</label>
        <div className="flex items-center gap-3">
          <input type="number" value={elapsed} onChange={(e) => setElapsed(e.target.value)} placeholder="已过区块数"
            className="w-40 rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
          {calculatedPrice && (
            <span className="text-sm font-medium text-gray-900 dark:text-gray-100">当前价格: {calculatedPrice}</span>
          )}
        </div>
      </div>

      <div className="mt-4 flex items-center gap-3">
        <button type="button" onClick={handleSubmit} disabled={isTxBusy(configureDutchAuction)}
          className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">
          配置荷兰拍
        </button>
        <TxStatusIndicator txState={configureDutchAuction.txState} />
      </div>
    </section>
  );
}

// ─── Vesting Config ─────────────────────────────────────────

function VestingSection() {
  const { entityId } = useEntityContext();
  const { configureVesting } = useTokensale();
  const [roundId, setRoundId] = useState('');
  const [cliffBlocks, setCliffBlocks] = useState('');
  const [vestingBlocks, setVestingBlocks] = useState('');

  const handleSubmit = useCallback(() => {
    if (!roundId || !cliffBlocks || !vestingBlocks) return;
    configureVesting.mutate([entityId, Number(roundId), Number(cliffBlocks), Number(vestingBlocks)]);
  }, [entityId, roundId, cliffBlocks, vestingBlocks, configureVesting]);

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">归属期配置</h2>
      <div className="grid grid-cols-3 gap-3">
        <div>
          <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">轮次 ID</label>
          <input type="number" value={roundId} onChange={(e) => setRoundId(e.target.value)} placeholder="轮次ID"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
        </div>
        <div>
          <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">锁定期 (区块)</label>
          <input type="number" value={cliffBlocks} onChange={(e) => setCliffBlocks(e.target.value)} placeholder="锁定区块数"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
        </div>
        <div>
          <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">释放期 (区块)</label>
          <input type="number" value={vestingBlocks} onChange={(e) => setVestingBlocks(e.target.value)} placeholder="释放区块数"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
        </div>
      </div>
      <div className="mt-4 flex items-center gap-3">
        <button type="button" onClick={handleSubmit} disabled={isTxBusy(configureVesting)}
          className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">
          配置归属期
        </button>
        <TxStatusIndicator txState={configureVesting.txState} />
      </div>
    </section>
  );
}

// ─── Subscribe / Increase Subscription ──────────────────────

function SubscribeSection() {
  const { entityId } = useEntityContext();
  const { subscribe, increaseSubscription } = useTokensale();
  const [roundId, setRoundId] = useState('');
  const [amount, setAmount] = useState('');
  const [increaseAmt, setIncreaseAmt] = useState('');

  const handleSubscribe = useCallback(() => {
    if (!roundId || !amount) return;
    subscribe.mutate([entityId, Number(roundId), amount]);
    setAmount('');
  }, [entityId, roundId, amount, subscribe]);

  const handleIncrease = useCallback(() => {
    if (!roundId || !increaseAmt) return;
    increaseSubscription.mutate([entityId, Number(roundId), increaseAmt]);
    setIncreaseAmt('');
  }, [entityId, roundId, increaseAmt, increaseSubscription]);

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">认购</h2>
      <div className="mb-3">
        <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">轮次 ID</label>
        <input type="number" value={roundId} onChange={(e) => setRoundId(e.target.value)} placeholder="轮次ID"
          className="w-40 rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
      </div>
      <div className="grid gap-4 md:grid-cols-2">
        <div className="space-y-3">
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">新认购</h3>
          <input type="text" value={amount} onChange={(e) => setAmount(e.target.value)} placeholder="认购金额"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
          <div className="flex items-center gap-3">
            <button type="button" onClick={handleSubscribe} disabled={isTxBusy(subscribe)}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">
              认购
            </button>
            <TxStatusIndicator txState={subscribe.txState} />
          </div>
        </div>
        <div className="space-y-3">
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">追加认购</h3>
          <input type="text" value={increaseAmt} onChange={(e) => setIncreaseAmt(e.target.value)} placeholder="追加金额"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
          <div className="flex items-center gap-3">
            <button type="button" onClick={handleIncrease} disabled={isTxBusy(increaseSubscription)}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">
              追加
            </button>
            <TxStatusIndicator txState={increaseSubscription.txState} />
          </div>
        </div>
      </div>
    </section>
  );
}

// ─── Whitelist Management ───────────────────────────────────

function WhitelistSection() {
  const { entityId } = useEntityContext();
  const { addToWhitelist, removeFromWhitelist } = useTokensale();
  const [roundId, setRoundId] = useState('');
  const [account, setAccount] = useState('');

  const handleAdd = useCallback(() => {
    if (!roundId || !account.trim()) return;
    addToWhitelist.mutate([entityId, Number(roundId), account.trim()]);
    setAccount('');
  }, [entityId, roundId, account, addToWhitelist]);

  const handleRemove = useCallback(() => {
    if (!roundId || !account.trim()) return;
    removeFromWhitelist.mutate([entityId, Number(roundId), account.trim()]);
    setAccount('');
  }, [entityId, roundId, account, removeFromWhitelist]);

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">白名单管理</h2>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
        <div>
          <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">轮次 ID</label>
          <input type="number" value={roundId} onChange={(e) => setRoundId(e.target.value)} placeholder="轮次ID"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
        </div>
        <div className="sm:col-span-2">
          <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">账户地址</label>
          <input type="text" value={account} onChange={(e) => setAccount(e.target.value)} placeholder="账户地址"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
        </div>
      </div>
      <div className="mt-4 flex items-center gap-3">
        <button type="button" onClick={handleAdd} disabled={isTxBusy(addToWhitelist)}
          className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">
          添加白名单
        </button>
        <button type="button" onClick={handleRemove} disabled={isTxBusy(removeFromWhitelist)}
          className="rounded-md bg-red-500 px-4 py-2 text-sm text-white hover:bg-red-600 disabled:cursor-not-allowed disabled:opacity-50">
          移除白名单
        </button>
        <TxStatusIndicator txState={addToWhitelist.txState} />
        <TxStatusIndicator txState={removeFromWhitelist.txState} />
      </div>
    </section>
  );
}

// ─── Sale Round Lifecycle Actions ───────────────────────────

function LifecycleActions() {
  const { entityId } = useEntityContext();
  const { startSaleRound, endSaleRound, claimTokens, unlockTokens, claimRefund } = useTokensale();
  const [roundId, setRoundId] = useState('');

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">轮次操作</h2>
      <div className="mb-4">
        <label className="mb-1 block text-sm text-gray-700 dark:text-gray-300">轮次 ID</label>
        <input type="number" value={roundId} onChange={(e) => setRoundId(e.target.value)} placeholder="轮次ID"
          className="w-40 rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100" />
      </div>
      <div className="flex flex-wrap items-center gap-3">
        <button type="button" onClick={() => roundId && startSaleRound.mutate([entityId, Number(roundId)])}
          disabled={!roundId || isTxBusy(startSaleRound)}
          className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">
          开始轮次
        </button>
        <button type="button" onClick={() => roundId && endSaleRound.mutate([entityId, Number(roundId)])}
          disabled={!roundId || isTxBusy(endSaleRound)}
          className="rounded-md bg-red-500 px-4 py-2 text-sm text-white hover:bg-red-600 disabled:cursor-not-allowed disabled:opacity-50">
          结束轮次
        </button>
        <button type="button" onClick={() => roundId && claimTokens.mutate([entityId, Number(roundId)])}
          disabled={!roundId || isTxBusy(claimTokens)}
          className="rounded-md bg-green-600 px-4 py-2 text-sm text-white hover:bg-green-700 disabled:cursor-not-allowed disabled:opacity-50">
          领取代币
        </button>
        <button type="button" onClick={() => roundId && unlockTokens.mutate([entityId, Number(roundId)])}
          disabled={!roundId || isTxBusy(unlockTokens)}
          className="rounded-md bg-purple-600 px-4 py-2 text-sm text-white hover:bg-purple-700 disabled:cursor-not-allowed disabled:opacity-50">
          解锁代币
        </button>
        <button type="button" onClick={() => roundId && claimRefund.mutate([entityId, Number(roundId)])}
          disabled={!roundId || isTxBusy(claimRefund)}
          className="rounded-md border border-gray-300 px-4 py-2 text-sm text-gray-700 hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-50 dark:border-gray-600 dark:text-gray-300 dark:hover:bg-gray-800">
          申请退款
        </button>
      </div>
      <div className="mt-3 flex flex-wrap items-center gap-3">
        <TxStatusIndicator txState={startSaleRound.txState} />
        <TxStatusIndicator txState={endSaleRound.txState} />
        <TxStatusIndicator txState={claimTokens.txState} />
        <TxStatusIndicator txState={unlockTokens.txState} />
        <TxStatusIndicator txState={claimRefund.txState} />
      </div>
    </section>
  );
}

// ─── Sale Rounds List ───────────────────────────────────────

function SaleRoundsList() {
  const { saleRounds } = useTokensale();

  if (saleRounds.length === 0) {
    return (
      <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
        <h2 className="mb-2 text-lg font-semibold text-gray-900 dark:text-gray-100">发售轮次</h2>
        <p className="py-8 text-center text-sm text-gray-400">暂无发售轮次</p>
      </section>
    );
  }

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">发售轮次 ({saleRounds.length})</h2>
      <div className="space-y-3">
        {saleRounds.map((r) => (
          <div key={r.id} className="rounded-md border border-gray-100 p-4 dark:border-gray-700">
            <div className="flex items-center justify-between">
              <span className="text-sm font-medium text-gray-900 dark:text-gray-100">#{r.id} {r.name}</span>
              <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${STATUS_BADGE[r.status] ?? STATUS_BADGE.Created}`}>
                {STATUS_LABEL[r.status] ?? r.status}
              </span>
            </div>
            <div className="mt-2 grid grid-cols-2 gap-2 text-xs text-gray-500 sm:grid-cols-4">
              <span>价格: {r.price.toString()}</span>
              <span>总量: {r.totalSupply.toString()}</span>
              <span>已募: {r.totalRaised.toString()}</span>
              <span>参与: {r.participantCount}</span>
              <span>区块: {r.startBlock}–{r.endBlock}</span>
              <span>购买: {r.minPurchase.toString()}–{r.maxPurchase.toString()}</span>
              <span>软顶: {r.softCap.toString()}</span>
              <span>硬顶: {r.hardCap.toString()}</span>
            </div>
            {r.dutchAuction && (
              <div className="mt-1 text-xs text-gray-500">
                荷兰拍: {r.dutchAuction.startPrice.toString()} → {r.dutchAuction.endPrice.toString()} / {r.dutchAuction.decayBlocks} 区块
              </div>
            )}
            {r.vestingConfig && (
              <div className="mt-1 text-xs text-gray-500">
                归属期: 锁定 {r.vestingConfig.cliffBlocks} 区块, 释放 {r.vestingConfig.vestingBlocks} 区块
              </div>
            )}
          </div>
        ))}
      </div>
    </section>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function TokenSalePage() {
  const t = useTranslations('tokensale');
  const { isLoading, error } = useTokensale();

  if (isLoading) {
    return <div className="flex items-center justify-center p-12 text-sm text-gray-500">{t('loading')}</div>;
  }
  if (error) {
    return <div className="flex items-center justify-center p-12 text-sm text-red-500">加载失败: {String(error)}</div>;
  }

  return (
    <div className="mx-auto max-w-5xl space-y-6 p-4 sm:p-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('title')}</h1>
        <PermissionGuard required={AdminPermission.TOKEN_MANAGE} fallback={null}>
          <CreateSaleRoundForm />
        </PermissionGuard>
      </div>

      <SaleRoundsList />
      <SubscribeSection />
      <LifecycleActions />

      <PermissionGuard required={AdminPermission.TOKEN_MANAGE} fallback={null}>
        <DutchAuctionSection />
        <VestingSection />
        <WhitelistSection />
      </PermissionGuard>
    </div>
  );
}
