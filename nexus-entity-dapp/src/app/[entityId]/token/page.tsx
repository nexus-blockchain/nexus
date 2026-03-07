'use client';

import React, { useState, useCallback } from 'react';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useEntityToken, getTokenRightsLabel } from '@/hooks/use-entity-token';
import { PermissionGuard } from '@/components/permission-guard';
import { TxConfirmDialog } from '@/components/tx-confirm-dialog';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { AdminPermission } from '@/lib/types/models';
import { TokenType, TransferRestrictionMode } from '@/lib/types/enums';

import { useTranslations } from 'next-intl';
// ─── Constants ──────────────────────────────────────────────

const TOKEN_TYPE_LABELS: Record<TokenType, string> = {
  [TokenType.Points]: '积分',
  [TokenType.Governance]: '治理代币',
  [TokenType.Equity]: '权益代币',
  [TokenType.Membership]: '会员代币',
  [TokenType.Share]: '份额代币',
  [TokenType.Bond]: '债券代币',
  [TokenType.Hybrid]: '混合代币',
};

const TRANSFER_RESTRICTION_LABELS: Record<TransferRestrictionMode, string> = {
  [TransferRestrictionMode.None]: '无限制',
  [TransferRestrictionMode.Whitelist]: '白名单',
  [TransferRestrictionMode.Blacklist]: '黑名单',
  [TransferRestrictionMode.KycRequired]: '需要 KYC',
  [TransferRestrictionMode.MembersOnly]: '仅会员',
};

// ─── Helpers ────────────────────────────────────────────────

function formatTokenAmount(amount: bigint, decimals: number): string {
  if (decimals === 0) return amount.toLocaleString();
  const divisor = BigInt('1' + '0'.repeat(decimals));
  const whole = amount / divisor;
  const remainder = amount % divisor;
  const decStr = remainder.toString().padStart(decimals, '0').replace(/0+$/, '');
  return decStr ? `${whole.toLocaleString()}.${decStr}` : whole.toLocaleString();
}

function shortenAddress(addr: string): string {
  if (addr.length <= 12) return addr;
  return `${addr.slice(0, 6)}…${addr.slice(-6)}`;
}

// ─── Token Info Card ────────────────────────────────────────

function TokenInfoCard({
  tokenConfig,
}: {
  tokenConfig: NonNullable<ReturnType<typeof useEntityToken>['tokenConfig']>;
}) {
  const decimals = tokenConfig.decimals;

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">代币信息</h2>
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <InfoItem label="名称" value={tokenConfig.name} />
        <InfoItem label="符号" value={tokenConfig.symbol} />
        <InfoItem label="精度" value={String(decimals)} />
        <InfoItem
          label="类型"
          value={TOKEN_TYPE_LABELS[tokenConfig.tokenType] ?? tokenConfig.tokenType}
        />
        <InfoItem label="总供应量" value={formatTokenAmount(tokenConfig.totalSupply, decimals)} />
        <InfoItem label="最大供应量" value={formatTokenAmount(tokenConfig.maxSupply, decimals)} />
        <InfoItem
          label="转账限制"
          value={TRANSFER_RESTRICTION_LABELS[tokenConfig.transferRestriction]}
        />
        <InfoItem label="持有人数" value={String(tokenConfig.holderCount)} />
      </div>

      <div className="mt-4 rounded-md bg-blue-50 px-4 py-3 dark:bg-blue-900/20">
        <p className="text-sm font-medium text-blue-800 dark:text-blue-300">
          权利属性: {getTokenRightsLabel(tokenConfig.tokenType)}
        </p>
      </div>
    </section>
  );
}

function InfoItem({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-xs text-gray-500 dark:text-gray-400">{label}</dt>
      <dd className="mt-0.5 text-sm font-medium text-gray-900 dark:text-gray-100">{value}</dd>
    </div>
  );
}

// ─── Token Rights Overview ──────────────────────────────────

function TokenRightsOverview() {
  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">代币类型权利说明</h2>
      <div className="grid gap-2 sm:grid-cols-2">
        {Object.values(TokenType).map((tt) => (
          <div key={tt} className="flex items-center gap-2 rounded-md bg-gray-50 px-3 py-2 text-sm dark:bg-gray-800">
            <span className="font-medium text-gray-700 dark:text-gray-300">
              {TOKEN_TYPE_LABELS[tt]}
            </span>
            <span className="text-gray-500 dark:text-gray-400">— {getTokenRightsLabel(tt)}</span>
          </div>
        ))}
      </div>
    </section>
  );
}

// ─── Mint / Burn Forms ──────────────────────────────────────

function MintBurnSection() {
  const { entityId, isReadOnly, isSuspended } = useEntityContext();
  const { mintTokens, burnTokens } = useEntityToken();
  const [mintAccount, setMintAccount] = useState('');
  const [mintAmount, setMintAmount] = useState('');
  const [burnAccount, setBurnAccount] = useState('');
  const [burnAmount, setBurnAmount] = useState('');
  const [showBurnConfirm, setShowBurnConfirm] = useState(false);

  const handleMint = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!mintAccount.trim() || !mintAmount.trim()) return;
      mintTokens.mutate([entityId, mintAccount.trim(), mintAmount.trim()]);
      setMintAccount('');
      setMintAmount('');
    },
    [entityId, mintAccount, mintAmount, mintTokens],
  );

  const handleBurnSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!burnAccount.trim() || !burnAmount.trim()) return;
      setShowBurnConfirm(true);
    },
    [burnAccount, burnAmount],
  );

  const handleBurnConfirm = useCallback(() => {
    burnTokens.mutate([entityId, burnAccount.trim(), burnAmount.trim()]);
    setBurnAccount('');
    setBurnAmount('');
    setShowBurnConfirm(false);
  }, [entityId, burnAccount, burnAmount, burnTokens]);

  if (isReadOnly || isSuspended) return null;

  return (
    <PermissionGuard required={AdminPermission.TOKEN_MANAGE} fallback={null}>
      <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
        <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">铸造 / 销毁</h2>
        <div className="grid gap-6 lg:grid-cols-2">
          {/* Mint */}
          <form onSubmit={handleMint} className="space-y-3">
            <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">铸造代币</h3>
            <input
              type="text"
              value={mintAccount}
              onChange={(e) => setMintAccount(e.target.value)}
              placeholder="接收地址"
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              required
            />
            <input
              type="text"
              inputMode="decimal"
              value={mintAmount}
              onChange={(e) => setMintAmount(e.target.value)}
              placeholder="数量"
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              required
            />
            <div className="flex items-center gap-3">
              <button
                type="submit"
                disabled={mintTokens.txState.status === 'signing' || mintTokens.txState.status === 'broadcasting'}
                className="rounded-md bg-green-600 px-4 py-2 text-sm text-white hover:bg-green-700 disabled:cursor-not-allowed disabled:opacity-50"
              >
                铸造
              </button>
              <TxStatusIndicator txState={mintTokens.txState} />
            </div>
          </form>

          {/* Burn */}
          <form onSubmit={handleBurnSubmit} className="space-y-3">
            <h3 className="text-sm font-medium text-red-700 dark:text-red-400">销毁代币 ⚠️</h3>
            <input
              type="text"
              value={burnAccount}
              onChange={(e) => setBurnAccount(e.target.value)}
              placeholder="目标地址"
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              required
            />
            <input
              type="text"
              inputMode="decimal"
              value={burnAmount}
              onChange={(e) => setBurnAmount(e.target.value)}
              placeholder="数量"
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              required
            />
            <div className="flex items-center gap-3">
              <button
                type="submit"
                disabled={burnTokens.txState.status === 'signing' || burnTokens.txState.status === 'broadcasting'}
                className="rounded-md bg-red-600 px-4 py-2 text-sm text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50"
              >
                销毁
              </button>
              <TxStatusIndicator txState={burnTokens.txState} />
            </div>
          </form>
        </div>
      </section>

      <TxConfirmDialog
        open={showBurnConfirm}
        onClose={() => setShowBurnConfirm(false)}
        onConfirm={handleBurnConfirm}
        config={{
          title: '确认销毁代币',
          description: '代币销毁是不可逆操作，销毁后无法恢复。请确认您要销毁指定数量的代币。',
          severity: 'danger',
        }}
      />
    </PermissionGuard>
  );
}

// ─── Holders List ───────────────────────────────────────────

function HoldersList({
  holders,
  decimals,
}: {
  holders: { account: string; balance: bigint }[];
  decimals: number;
}) {
  if (holders.length === 0) {
    return (
      <div className="rounded-lg border border-dashed border-gray-300 p-6 text-center text-sm text-gray-500 dark:border-gray-600">
        暂无持有人
      </div>
    );
  }

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">
        持有人列表 ({holders.length})
      </h2>
      <div className="overflow-x-auto">
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b text-xs text-gray-500 dark:border-gray-700">
              <th className="pb-2 pr-4">地址</th>
              <th className="pb-2 text-right">余额</th>
            </tr>
          </thead>
          <tbody>
            {holders.map((h) => (
              <tr key={h.account} className="border-b last:border-0 dark:border-gray-700">
                <td className="py-2 pr-4 font-mono text-xs text-gray-700 dark:text-gray-300">
                  {shortenAddress(h.account)}
                </td>
                <td className="py-2 text-right text-gray-900 dark:text-gray-100">
                  {formatTokenAmount(h.balance, decimals)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

// ─── Transfer Restriction Config ────────────────────────────

function TransferRestrictionSection() {
  const { entityId, isReadOnly, isSuspended } = useEntityContext();
  const { tokenConfig, setTransferRestriction } = useEntityToken();
  const [selectedMode, setSelectedMode] = useState<TransferRestrictionMode>(
    tokenConfig?.transferRestriction ?? TransferRestrictionMode.None,
  );

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      setTransferRestriction.mutate([entityId, selectedMode]);
    },
    [entityId, selectedMode, setTransferRestriction],
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">转账限制模式</h2>
      <div className="mb-3 flex flex-wrap gap-2">
        {Object.values(TransferRestrictionMode).map((mode) => (
          <span
            key={mode}
            className={`rounded-full px-3 py-1 text-xs font-medium ${
              tokenConfig?.transferRestriction === mode
                ? 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-300'
                : 'bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400'
            }`}
          >
            {TRANSFER_RESTRICTION_LABELS[mode]}
          </span>
        ))}
      </div>

      {!isReadOnly && !isSuspended && (
        <PermissionGuard required={AdminPermission.TOKEN_MANAGE} fallback={null}>
          <form onSubmit={handleSubmit} className="flex items-center gap-3">
            <select
              value={selectedMode}
              onChange={(e) => setSelectedMode(e.target.value as TransferRestrictionMode)}
              className="rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            >
              {Object.values(TransferRestrictionMode).map((mode) => (
                <option key={mode} value={mode}>
                  {TRANSFER_RESTRICTION_LABELS[mode]}
                </option>
              ))}
            </select>
            <button
              type="submit"
              disabled={
                setTransferRestriction.txState.status === 'signing' ||
                setTransferRestriction.txState.status === 'broadcasting'
              }
              className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              更新
            </button>
            <TxStatusIndicator txState={setTransferRestriction.txState} />
          </form>
        </PermissionGuard>
      )}
    </section>
  );
}

// ─── Whitelist / Blacklist Management ───────────────────────

function ListManagement({
  title,
  items,
  onAdd,
  onRemove,
  addTxState,
  removeTxState,
}: {
  title: string;
  items: string[];
  onAdd: (account: string) => void;
  onRemove: (account: string) => void;
  addTxState: import('@/lib/types/models').TxState;
  removeTxState: import('@/lib/types/models').TxState;
}) {
  const { isReadOnly, isSuspended } = useEntityContext();
  const [newAccount, setNewAccount] = useState('');

  const handleAdd = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!newAccount.trim()) return;
      onAdd(newAccount.trim());
      setNewAccount('');
    },
    [newAccount, onAdd],
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">
        {title} ({items.length})
      </h2>

      {!isReadOnly && !isSuspended && (
        <PermissionGuard required={AdminPermission.TOKEN_MANAGE} fallback={null}>
          <form onSubmit={handleAdd} className="mb-4 flex gap-2">
            <input
              type="text"
              value={newAccount}
              onChange={(e) => setNewAccount(e.target.value)}
              placeholder="输入账户地址"
              className="flex-1 rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              required
            />
            <button
              type="submit"
              disabled={addTxState.status === 'signing' || addTxState.status === 'broadcasting'}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              添加
            </button>
          </form>
          <TxStatusIndicator txState={addTxState} />
        </PermissionGuard>
      )}

      {items.length === 0 ? (
        <p className="text-sm text-gray-500">列表为空</p>
      ) : (
        <ul className="space-y-1">
          {items.map((account) => (
            <li key={account} className="flex items-center justify-between rounded-md bg-gray-50 px-3 py-2 text-sm dark:bg-gray-800">
              <span className="font-mono text-xs text-gray-700 dark:text-gray-300">
                {shortenAddress(account)}
              </span>
              {!isReadOnly && !isSuspended && (
                <PermissionGuard required={AdminPermission.TOKEN_MANAGE} fallback={null}>
                  <button
                    type="button"
                    onClick={() => onRemove(account)}
                    disabled={removeTxState.status === 'signing' || removeTxState.status === 'broadcasting'}
                    className="text-xs text-red-600 hover:text-red-800 disabled:opacity-50"
                  >
                    移除
                  </button>
                </PermissionGuard>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function TokenPage() {
  const t = useTranslations('token');
  const { entityId } = useEntityContext();
  const {
    tokenConfig,
    holders,
    whitelist,
    blacklist,
    isLoading,
    error,
    addToWhitelist,
    removeFromWhitelist,
    addToBlacklist,
    removeFromBlacklist,
  } = useEntityToken();

  if (isLoading) {
    return (
      <div className="flex items-center justify-center p-12 text-sm text-gray-500">
        {t('loading')}
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center p-12 text-sm text-red-500">
        加载失败: {String(error)}
      </div>
    );
  }

  if (!tokenConfig) {
    return (
      <div className="flex items-center justify-center p-12 text-sm text-gray-500">
        该 Entity 尚未配置代币
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-4xl space-y-6 p-4 sm:p-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('title')}</h1>

      <TokenInfoCard tokenConfig={tokenConfig} />
      <TokenRightsOverview />
      <MintBurnSection />
      <TransferRestrictionSection />
      <HoldersList holders={holders} decimals={tokenConfig.decimals} />

      <div className="grid gap-6 lg:grid-cols-2">
        <ListManagement
          title="白名单"
          items={whitelist}
          onAdd={(account) => addToWhitelist.mutate([entityId, account])}
          onRemove={(account) => removeFromWhitelist.mutate([entityId, account])}
          addTxState={addToWhitelist.txState}
          removeTxState={removeFromWhitelist.txState}
        />
        <ListManagement
          title="黑名单"
          items={blacklist}
          onAdd={(account) => addToBlacklist.mutate([entityId, account])}
          onRemove={(account) => removeFromBlacklist.mutate([entityId, account])}
          addTxState={addToBlacklist.txState}
          removeTxState={removeFromBlacklist.txState}
        />
      </div>
    </div>
  );
}
