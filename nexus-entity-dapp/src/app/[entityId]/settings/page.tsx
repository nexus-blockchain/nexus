'use client';

import React, { useState, useCallback } from 'react';
import { useTranslations } from 'next-intl';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { PermissionGuard } from '@/components/permission-guard';
import { TxConfirmDialog } from '@/components/tx-confirm-dialog';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { IpfsImage } from '@/components/ipfs-image';
import { useEntityMutation } from '@/hooks/use-entity-mutation';
import { useIpfsUpload } from '@/hooks/use-ipfs-upload';
import { AdminPermission } from '@/lib/types/models';
import { EntityType, GovernanceMode } from '@/lib/types/enums';
import type { ConfirmDialogConfig } from '@/lib/types/models';

// ─── Helpers ────────────────────────────────────────────────

function formatNexBalance(balance: bigint): string {
  const divisor = BigInt('1000000000000');
  const whole = balance / divisor;
  const remainder = balance % divisor;
  const decStr = remainder.toString().padStart(12, '0').slice(0, 4);
  const trimmed = decStr.replace(/0+$/, '');
  return trimmed ? `${whole.toLocaleString()}.${trimmed}` : whole.toLocaleString();
}

/** Permission labels for admin checkboxes */
const PERMISSION_OPTIONS = [
  { label: 'Shop Manage', value: AdminPermission.SHOP_MANAGE },
  { label: 'Member Manage', value: AdminPermission.MEMBER_MANAGE },
  { label: 'Token Manage', value: AdminPermission.TOKEN_MANAGE },
  { label: 'Ads Manage', value: AdminPermission.ADS_MANAGE },
  { label: 'Review Manage', value: AdminPermission.REVIEW_MANAGE },
  { label: 'Disclosure Manage', value: AdminPermission.DISCLOSURE_MANAGE },
  { label: 'Entity Manage', value: AdminPermission.ENTITY_MANAGE },
  { label: 'KYC Manage', value: AdminPermission.KYC_MANAGE },
  { label: 'Governance Manage', value: AdminPermission.GOVERNANCE_MANAGE },
  { label: 'Order Manage', value: AdminPermission.ORDER_MANAGE },
  { label: 'Commission Manage', value: AdminPermission.COMMISSION_MANAGE },
] as const;

// ─── Section: Entity Info Edit ──────────────────────────────

function EntityInfoSection() {
  const { entityId, entity } = useEntityContext();
  const ipfsUpload = useIpfsUpload();

  const [name, setName] = useState(entity?.name ?? '');
  const [logoCid, setLogoCid] = useState(entity?.logoCid ?? '');
  const [descriptionCid, setDescriptionCid] = useState(entity?.descriptionCid ?? '');
  const [metadataUri, setMetadataUri] = useState(entity?.metadataUri ?? '');

  const updateEntity = useEntityMutation('entityRegistry', 'updateEntity', {
    invalidateKeys: [['entity', entityId]],
  });

  const handleLogoUpload = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;
      const cid = await ipfsUpload.upload(file);
      if (cid) setLogoCid(cid);
    },
    [ipfsUpload],
  );

  const handleDescriptionUpload = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;
      const cid = await ipfsUpload.upload(file);
      if (cid) setDescriptionCid(cid);
    },
    [ipfsUpload],
  );

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      updateEntity.mutate([
        entityId,
        name,
        logoCid || null,
        descriptionCid || null,
        metadataUri || null,
      ]);
    },
    [entityId, name, logoCid, descriptionCid, metadataUri, updateEntity],
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">Entity Information</h2>

      {entity?.logoCid && (
        <div className="mb-4">
          <IpfsImage cid={entity.logoCid} alt="Entity logo" className="h-20 w-20 rounded-lg" />
        </div>
      )}

      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label htmlFor="entity-name" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
            Name
          </label>
          <input
            id="entity-name"
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
        </div>

        <div>
          <label htmlFor="logo-upload" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
            Logo (IPFS)
          </label>
          <div className="flex items-center gap-2">
            <input
              id="logo-upload"
              type="file"
              accept="image/*"
              onChange={handleLogoUpload}
              className="text-sm text-gray-500 file:mr-2 file:rounded-md file:border-0 file:bg-blue-50 file:px-3 file:py-1.5 file:text-sm file:text-blue-700 hover:file:bg-blue-100"
            />
            {ipfsUpload.isUploading && <span className="text-xs text-gray-400">Uploading…</span>}
          </div>
          {logoCid && <p className="mt-1 text-xs text-gray-400 break-all">CID: {logoCid}</p>}
        </div>

        <div>
          <label htmlFor="desc-upload" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
            Description (IPFS)
          </label>
          <div className="flex items-center gap-2">
            <input
              id="desc-upload"
              type="file"
              onChange={handleDescriptionUpload}
              className="text-sm text-gray-500 file:mr-2 file:rounded-md file:border-0 file:bg-blue-50 file:px-3 file:py-1.5 file:text-sm file:text-blue-700 hover:file:bg-blue-100"
            />
          </div>
          {descriptionCid && <p className="mt-1 text-xs text-gray-400 break-all">CID: {descriptionCid}</p>}
        </div>

        <div>
          <label htmlFor="metadata-uri" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
            Metadata URI
          </label>
          <input
            id="metadata-uri"
            type="text"
            value={metadataUri}
            onChange={(e) => setMetadataUri(e.target.value)}
            placeholder="https://..."
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
          />
        </div>

        <div className="flex items-center gap-3">
          <button
            type="submit"
            disabled={updateEntity.txState.status === 'signing' || updateEntity.txState.status === 'broadcasting'}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            Save Changes
          </button>
          <TxStatusIndicator txState={updateEntity.txState} />
        </div>
      </form>
    </section>
  );
}


// ─── Section: Admin Management (Owner only) ─────────────────

function AdminManagementSection() {
  const { entityId } = useEntityContext();

  const [adminAddress, setAdminAddress] = useState('');
  const [selectedPermissions, setSelectedPermissions] = useState(0);
  const [removeAddress, setRemoveAddress] = useState('');

  const addAdmin = useEntityMutation('entityRegistry', 'addAdmin', {
    invalidateKeys: [['entity', entityId, 'permissions']],
  });

  const removeAdmin = useEntityMutation('entityRegistry', 'removeAdmin', {
    invalidateKeys: [['entity', entityId, 'permissions']],
  });

  const updatePermissions = useEntityMutation('entityRegistry', 'updateAdminPermissions', {
    invalidateKeys: [['entity', entityId, 'permissions']],
  });

  const togglePermission = useCallback((bit: number) => {
    setSelectedPermissions((prev) => prev ^ bit);
  }, []);

  const handleAddAdmin = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!adminAddress.trim()) return;
      addAdmin.mutate([entityId, adminAddress.trim(), selectedPermissions]);
      setAdminAddress('');
      setSelectedPermissions(0);
    },
    [entityId, adminAddress, selectedPermissions, addAdmin],
  );

  const handleRemoveAdmin = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!removeAddress.trim()) return;
      removeAdmin.mutate([entityId, removeAddress.trim()]);
      setRemoveAddress('');
    },
    [entityId, removeAddress, removeAdmin],
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">Admin Management</h2>
      <p className="mb-4 text-sm text-gray-500 dark:text-gray-400">Only the Entity Owner can manage admins.</p>

      {/* Add Admin */}
      <form onSubmit={handleAddAdmin} className="mb-6 space-y-3">
        <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">Add Admin</h3>
        <div>
          <label htmlFor="admin-address" className="mb-1 block text-xs text-gray-500">
            Wallet Address
          </label>
          <input
            id="admin-address"
            type="text"
            value={adminAddress}
            onChange={(e) => setAdminAddress(e.target.value)}
            placeholder="5..."
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
        </div>

        <div>
          <p className="mb-2 text-xs text-gray-500">Permissions</p>
          <div className="flex flex-wrap gap-2">
            {PERMISSION_OPTIONS.map((opt) => (
              <label
                key={opt.value}
                className="flex items-center gap-1.5 rounded-md border border-gray-200 px-2 py-1 text-xs dark:border-gray-600"
              >
                <input
                  type="checkbox"
                  checked={(selectedPermissions & opt.value) === opt.value}
                  onChange={() => togglePermission(opt.value)}
                  className="rounded"
                />
                <span className="text-gray-700 dark:text-gray-300">{opt.label}</span>
              </label>
            ))}
          </div>
        </div>

        <div className="flex items-center gap-3">
          <button
            type="submit"
            disabled={addAdmin.txState.status === 'signing' || addAdmin.txState.status === 'broadcasting'}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            Add Admin
          </button>
          <TxStatusIndicator txState={addAdmin.txState} />
        </div>
      </form>

      <hr className="my-4 border-gray-200 dark:border-gray-700" />

      {/* Remove Admin */}
      <form onSubmit={handleRemoveAdmin} className="space-y-3">
        <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">Remove Admin</h3>
        <div>
          <label htmlFor="remove-admin-address" className="mb-1 block text-xs text-gray-500">
            Wallet Address
          </label>
          <input
            id="remove-admin-address"
            type="text"
            value={removeAddress}
            onChange={(e) => setRemoveAddress(e.target.value)}
            placeholder="5..."
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
        </div>
        <div className="flex items-center gap-3">
          <button
            type="submit"
            disabled={removeAdmin.txState.status === 'signing' || removeAdmin.txState.status === 'broadcasting'}
            className="rounded-md bg-red-600 px-4 py-2 text-sm text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            Remove Admin
          </button>
          <TxStatusIndicator txState={removeAdmin.txState} />
        </div>
      </form>
    </section>
  );
}


// ─── Section: Fund Management ───────────────────────────────

function FundManagementSection() {
  const { entityId, entity } = useEntityContext();
  const [topUpAmount, setTopUpAmount] = useState('');

  const topUpFund = useEntityMutation('entityRegistry', 'topUpFund', {
    invalidateKeys: [['entity', entityId]],
  });

  const handleTopUp = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      const trimmed = topUpAmount.trim();
      if (!trimmed) return;
      // Convert NEX amount (12 decimals) to chain balance
      const parts = trimmed.split('.');
      const whole = parts[0] ?? '0';
      const frac = (parts[1] ?? '').padEnd(12, '0').slice(0, 12);
      const raw = BigInt(whole) * BigInt('1000000000000') + BigInt(frac);
      topUpFund.mutate([entityId, raw.toString()]);
      setTopUpAmount('');
    },
    [entityId, topUpAmount, topUpFund],
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">资金管理</h2>

      {/* Balance display */}
      <div className="mb-6 rounded-md bg-gray-50 p-4 dark:bg-gray-800">
        <p className="text-sm text-gray-500 dark:text-gray-400">Entity 资金余额</p>
        <p className="mt-1 text-2xl font-bold text-gray-900 dark:text-gray-100">
          {entity ? formatNexBalance(entity.fundBalance) : '—'} <span className="text-sm font-normal text-gray-500">NEX</span>
        </p>
      </div>

      {/* Top up form */}
      <form onSubmit={handleTopUp} className="space-y-3">
        <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">充值</h3>
        <div>
          <label htmlFor="topup-amount" className="mb-1 block text-xs text-gray-500">
            充值金额 (NEX)
          </label>
          <input
            id="topup-amount"
            type="text"
            inputMode="decimal"
            value={topUpAmount}
            onChange={(e) => setTopUpAmount(e.target.value)}
            placeholder="0.0"
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            required
          />
        </div>
        <div className="flex items-center gap-3">
          <button
            type="submit"
            disabled={topUpFund.txState.status === 'signing' || topUpFund.txState.status === 'broadcasting'}
            className="rounded-md bg-green-600 px-4 py-2 text-sm text-white hover:bg-green-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            充值
          </button>
          <TxStatusIndicator txState={topUpFund.txState} />
        </div>
      </form>
    </section>
  );
}


// ─── Section: Lifecycle Operations ──────────────────────────

function LifecycleSection() {
  const { entityId, entity, isOwner } = useEntityContext();

  // Transfer ownership state
  const [newOwner, setNewOwner] = useState('');
  const [transferDialogOpen, setTransferDialogOpen] = useState(false);

  // Close entity state
  const [closeDialogOpen, setCloseDialogOpen] = useState(false);

  // Bind referrer state
  const [referrerAddress, setReferrerAddress] = useState('');

  // Type upgrade state
  const [newEntityType, setNewEntityType] = useState<EntityType>(EntityType.Merchant);
  const [newGovernanceMode, setNewGovernanceMode] = useState<GovernanceMode>(GovernanceMode.None);

  // Mutations
  const requestClose = useEntityMutation('entityRegistry', 'requestCloseEntity', {
    invalidateKeys: [['entity', entityId]],
  });

  const reopenEntity = useEntityMutation('entityRegistry', 'reopenEntity', {
    invalidateKeys: [['entity', entityId]],
  });

  const transferOwnership = useEntityMutation('entityRegistry', 'transferOwnership', {
    invalidateKeys: [['entity', entityId]],
  });

  const bindReferrer = useEntityMutation('entityRegistry', 'bindEntityReferrer', {
    invalidateKeys: [['entity', entityId]],
  });

  const upgradeType = useEntityMutation('entityRegistry', 'upgradeEntityType', {
    invalidateKeys: [['entity', entityId]],
  });

  // Handlers
  const handleRequestClose = useCallback(() => {
    setCloseDialogOpen(true);
  }, []);

  const confirmClose = useCallback(() => {
    requestClose.mutate([entityId]);
    setCloseDialogOpen(false);
  }, [entityId, requestClose]);

  const handleReopen = useCallback(() => {
    reopenEntity.mutate([entityId]);
  }, [entityId, reopenEntity]);

  const handleTransferClick = useCallback(() => {
    if (!newOwner.trim()) return;
    setTransferDialogOpen(true);
  }, [newOwner]);

  const confirmTransfer = useCallback(() => {
    transferOwnership.mutate([entityId, newOwner.trim()]);
    setTransferDialogOpen(false);
    setNewOwner('');
  }, [entityId, newOwner, transferOwnership]);

  const handleBindReferrer = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!referrerAddress.trim()) return;
      bindReferrer.mutate([entityId, referrerAddress.trim()]);
      setReferrerAddress('');
    },
    [entityId, referrerAddress, bindReferrer],
  );

  const handleUpgradeType = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      upgradeType.mutate([entityId, newEntityType, newGovernanceMode]);
    },
    [entityId, newEntityType, newGovernanceMode, upgradeType],
  );

  const transferDialogConfig: ConfirmDialogConfig = {
    title: '转移所有权',
    description: `此操作不可逆！您将把 Entity 的所有权转移给 ${newOwner.trim() || '...'}。转移后您将失去 Owner 权限，无法撤销。`,
    severity: 'danger',
    requireInput: entity?.name,
  };

  const closeDialogConfig: ConfirmDialogConfig = {
    title: '申请关闭 Entity',
    description: '申请关闭后 Entity 将进入 PendingClose 状态，所有写操作将被禁止。请确认您要继续。',
    severity: 'danger',
  };

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">生命周期管理</h2>

      <div className="space-y-6">
        {/* Reopen Entity */}
        <div>
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">重新开放</h3>
          <p className="mb-2 text-xs text-gray-500">将 PendingClose 状态的 Entity 重新开放。</p>
          <div className="flex items-center gap-3">
            <button
              type="button"
              onClick={handleReopen}
              disabled={reopenEntity.txState.status === 'signing' || reopenEntity.txState.status === 'broadcasting'}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              重新开放
            </button>
            <TxStatusIndicator txState={reopenEntity.txState} />
          </div>
        </div>

        <hr className="border-gray-200 dark:border-gray-700" />

        {/* Bind Referrer */}
        <form onSubmit={handleBindReferrer} className="space-y-3">
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">绑定推荐人</h3>
          <p className="mb-1 text-xs text-gray-500">一次性操作，绑定后不可更改。</p>
          <div>
            <label htmlFor="referrer-address" className="mb-1 block text-xs text-gray-500">
              推荐人地址
            </label>
            <input
              id="referrer-address"
              type="text"
              value={referrerAddress}
              onChange={(e) => setReferrerAddress(e.target.value)}
              placeholder="5..."
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              required
            />
          </div>
          <div className="flex items-center gap-3">
            <button
              type="submit"
              disabled={bindReferrer.txState.status === 'signing' || bindReferrer.txState.status === 'broadcasting'}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              绑定
            </button>
            <TxStatusIndicator txState={bindReferrer.txState} />
          </div>
        </form>

        <hr className="border-gray-200 dark:border-gray-700" />

        {/* Type Upgrade */}
        <form onSubmit={handleUpgradeType} className="space-y-3">
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">类型升级</h3>
          <p className="mb-1 text-xs text-gray-500">
            当前类型: <span className="font-medium">{entity?.entityType ?? '—'}</span>
          </p>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <div>
              <label htmlFor="new-entity-type" className="mb-1 block text-xs text-gray-500">
                新类型
              </label>
              <select
                id="new-entity-type"
                value={newEntityType}
                onChange={(e) => setNewEntityType(e.target.value as EntityType)}
                className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              >
                {Object.values(EntityType).map((t) => (
                  <option key={t} value={t}>
                    {t}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label htmlFor="new-governance-mode" className="mb-1 block text-xs text-gray-500">
                新治理模式
              </label>
              <select
                id="new-governance-mode"
                value={newGovernanceMode}
                onChange={(e) => setNewGovernanceMode(e.target.value as GovernanceMode)}
                className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              >
                {Object.values(GovernanceMode).map((m) => (
                  <option key={m} value={m}>
                    {m}
                  </option>
                ))}
              </select>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <button
              type="submit"
              disabled={upgradeType.txState.status === 'signing' || upgradeType.txState.status === 'broadcasting'}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              升级类型
            </button>
            <TxStatusIndicator txState={upgradeType.txState} />
          </div>
        </form>

        <hr className="border-gray-200 dark:border-gray-700" />

        {/* Transfer Ownership (Owner only, dangerous) */}
        {isOwner && (
          <div className="space-y-3">
            <h3 className="text-sm font-medium text-red-700 dark:text-red-400">转移所有权</h3>
            <p className="text-xs text-red-500">⚠️ 此操作不可逆。转移后您将失去 Owner 权限。</p>
            <div>
              <label htmlFor="new-owner-address" className="mb-1 block text-xs text-gray-500">
                新 Owner 地址
              </label>
              <input
                id="new-owner-address"
                type="text"
                value={newOwner}
                onChange={(e) => setNewOwner(e.target.value)}
                placeholder="5..."
                className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-red-500 focus:outline-none focus:ring-1 focus:ring-red-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              />
            </div>
            <div className="flex items-center gap-3">
              <button
                type="button"
                onClick={handleTransferClick}
                disabled={
                  !newOwner.trim() ||
                  transferOwnership.txState.status === 'signing' ||
                  transferOwnership.txState.status === 'broadcasting'
                }
                className="rounded-md bg-red-600 px-4 py-2 text-sm text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50"
              >
                转移所有权
              </button>
              <TxStatusIndicator txState={transferOwnership.txState} />
            </div>

            <TxConfirmDialog
              open={transferDialogOpen}
              onClose={() => setTransferDialogOpen(false)}
              onConfirm={confirmTransfer}
              config={transferDialogConfig}
            />
          </div>
        )}

        <hr className="border-gray-200 dark:border-gray-700" />

        {/* Request Close Entity (dangerous) */}
        <div className="space-y-3">
          <h3 className="text-sm font-medium text-red-700 dark:text-red-400">申请关闭</h3>
          <p className="text-xs text-red-500">⚠️ 关闭后 Entity 将进入 PendingClose 状态，所有写操作将被禁止。</p>
          <div className="flex items-center gap-3">
            <button
              type="button"
              onClick={handleRequestClose}
              disabled={requestClose.txState.status === 'signing' || requestClose.txState.status === 'broadcasting'}
              className="rounded-md bg-red-600 px-4 py-2 text-sm text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              申请关闭
            </button>
            <TxStatusIndicator txState={requestClose.txState} />
          </div>

          <TxConfirmDialog
            open={closeDialogOpen}
            onClose={() => setCloseDialogOpen(false)}
            onConfirm={confirmClose}
            config={closeDialogConfig}
          />
        </div>
      </div>
    </section>
  );
}


// ─── Page: Settings ─────────────────────────────────────────

export default function SettingsPage() {
  const { isOwner } = useEntityContext();
  const t = useTranslations('settings');

  return (
    <div className="mx-auto max-w-3xl space-y-6 p-4 sm:p-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('title')}</h1>

      {/* Entity info editing — requires ENTITY_MANAGE permission */}
      <PermissionGuard required={AdminPermission.ENTITY_MANAGE}>
        <EntityInfoSection />
      </PermissionGuard>

      {/* Admin management — Owner only */}
      {isOwner && <AdminManagementSection />}

      {/* Fund management — requires ENTITY_MANAGE permission */}
      <PermissionGuard required={AdminPermission.ENTITY_MANAGE}>
        <FundManagementSection />
      </PermissionGuard>

      {/* Lifecycle operations — requires ENTITY_MANAGE permission */}
      <PermissionGuard required={AdminPermission.ENTITY_MANAGE}>
        <LifecycleSection />
      </PermissionGuard>
    </div>
  );
}
