'use client';

import React, { useState, useCallback } from 'react';
import { useParams } from 'next/navigation';
import { z } from 'zod';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { useProducts } from '@/hooks/use-products';
import { useIpfsUpload } from '@/hooks/use-ipfs-upload';
import { PermissionGuard } from '@/components/permission-guard';
import { IpfsImage } from '@/components/ipfs-image';
import { TxStatusIndicator } from '@/components/tx-status-indicator';
import { AdminPermission } from '@/lib/types/models';
import { ProductCategory, ProductVisibility, ProductStatus } from '@/lib/types/enums';
import { getValidProductTransitions } from '@/lib/utils';
import type { ProductData } from '@/lib/types/models';

import { useTranslations } from 'next-intl';
// ─── Zod Schema ─────────────────────────────────────────────

export const createProductSchema = z
  .object({
    nameCid: z.string().min(1, '请上传商品名称'),
    imageCid: z.string().min(1, '请上传商品图片'),
    detailCid: z.string().min(1, '请上传商品详情'),
    priceNex: z.string().refine(
      (v) => { const n = Number(v); return !isNaN(n) && n > 0; },
      { message: 'NEX 价格必须大于 0' },
    ),
    priceUsdt: z.string().refine(
      (v) => { const n = Number(v); return !isNaN(n) && n >= 0; },
      { message: 'USDT 价格必须 >= 0' },
    ),
    stock: z.string().refine(
      (v) => { const n = Number(v); return Number.isInteger(n) && n >= 0; },
      { message: '库存必须 >= 0（0 表示无限）' },
    ),
    category: z.nativeEnum(ProductCategory),
    visibility: z.nativeEnum(ProductVisibility),
    levelGate: z.string().optional(),
    minQuantity: z.string().refine(
      (v) => { const n = Number(v); return Number.isInteger(n) && n >= 1; },
      { message: '最小购买量必须 >= 1' },
    ),
    maxQuantity: z.string().refine(
      (v) => { const n = Number(v); return Number.isInteger(n) && n >= 0; },
      { message: '最大购买量必须 >= 0（0 表示不限）' },
    ),
  })
  .refine(
    (data) => {
      const max = Number(data.maxQuantity);
      const min = Number(data.minQuantity);
      return max === 0 || max >= min;
    },
    { message: '最大购买量必须 >= 最小购买量（或为 0 表示不限）', path: ['maxQuantity'] },
  );

export type CreateProductFormData = z.infer<typeof createProductSchema>;

// ─── Constants ──────────────────────────────────────────────

const CATEGORY_LABELS: Record<ProductCategory, string> = {
  [ProductCategory.Digital]: '数字商品',
  [ProductCategory.Physical]: '实物商品',
  [ProductCategory.Service]: '服务',
  [ProductCategory.Subscription]: '订阅',
  [ProductCategory.Bundle]: '套餐',
  [ProductCategory.Other]: '其他',
};

const VISIBILITY_LABELS: Record<ProductVisibility, string> = {
  [ProductVisibility.Public]: '公开',
  [ProductVisibility.MembersOnly]: '仅会员',
  [ProductVisibility.LevelGated]: '等级限制',
};

const STATUS_CONFIG: Record<ProductStatus, { label: string; color: string }> = {
  [ProductStatus.Draft]: { label: '草稿', color: 'bg-gray-100 text-gray-800' },
  [ProductStatus.OnSale]: { label: '在售', color: 'bg-green-100 text-green-800' },
  [ProductStatus.SoldOut]: { label: '售罄', color: 'bg-yellow-100 text-yellow-800' },
  [ProductStatus.OffShelf]: { label: '已下架', color: 'bg-red-100 text-red-800' },
};

function formatNexBalance(balance: bigint): string {
  const divisor = BigInt('1000000000000');
  const whole = balance / divisor;
  const remainder = balance % divisor;
  const decStr = remainder.toString().padStart(12, '0').slice(0, 4);
  const trimmed = decStr.replace(/0+$/, '');
  return trimmed ? `${whole.toLocaleString()}.${trimmed}` : whole.toLocaleString();
}

function formatUsdtPrice(raw: number): string {
  return (raw / 1_000_000).toFixed(2);
}

// ─── IPFS Upload Field ──────────────────────────────────────

function IpfsUploadField({
  label,
  value,
  onChange,
  accept,
  error,
  id,
}: {
  label: string;
  value: string;
  onChange: (cid: string) => void;
  accept?: string;
  error?: string;
  id: string;
}) {
  const { upload, uploadText, isUploading, error: uploadError } = useIpfsUpload();

  const handleFile = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;
      const cid = await upload(file);
      if (cid) onChange(cid);
    },
    [upload, onChange],
  );

  const handleText = useCallback(
    async (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const text = e.target.value.trim();
      if (!text) return;
      const cid = await uploadText(text);
      if (cid) onChange(cid);
    },
    [uploadText, onChange],
  );

  return (
    <div>
      <label htmlFor={id} className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
        {label}
      </label>
      {accept ? (
        <input
          id={id}
          type="file"
          accept={accept}
          onChange={handleFile}
          disabled={isUploading}
          className="w-full text-sm file:mr-3 file:rounded-md file:border-0 file:bg-blue-50 file:px-3 file:py-1.5 file:text-sm file:text-blue-700 hover:file:bg-blue-100"
        />
      ) : (
        <textarea
          id={id}
          rows={2}
          placeholder="输入文本内容后自动上传至 IPFS"
          onBlur={handleText}
          disabled={isUploading}
          className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
        />
      )}
      {isUploading && <p className="mt-1 text-xs text-blue-500">上传中…</p>}
      {uploadError && <p className="mt-1 text-xs text-red-500">{uploadError}</p>}
      {value && <p className="mt-1 truncate text-xs text-green-600">CID: {value}</p>}
      {error && <p className="mt-1 text-xs text-red-500">{error}</p>}
    </div>
  );
}

// ─── Product Card ───────────────────────────────────────────

// ─── Transition Action Labels ───────────────────────────────

const TRANSITION_ACTION: Record<ProductStatus, { label: string; color: string }> = {
  [ProductStatus.OnSale]: { label: '上架', color: 'bg-green-600 hover:bg-green-700 text-white' },
  [ProductStatus.OffShelf]: { label: '下架', color: 'bg-orange-600 hover:bg-orange-700 text-white' },
  [ProductStatus.SoldOut]: { label: '售罄', color: 'bg-gray-400 text-white cursor-not-allowed' },
  [ProductStatus.Draft]: { label: '转为草稿', color: 'bg-gray-600 hover:bg-gray-700 text-white' },
};

function ProductCard({ product, shopId }: { product: ProductData; shopId: number }) {
  const statusCfg = STATUS_CONFIG[product.status];
  const { isReadOnly, isSuspended } = useEntityContext();
  const { listProduct, delistProduct } = useProducts(shopId);
  const validTransitions = getValidProductTransitions(product.status);
  const canAct = !isReadOnly && !isSuspended;

  const handleTransition = useCallback(
    (target: ProductStatus) => {
      if (target === ProductStatus.OnSale) {
        // Draft→OnSale or OffShelf→OnSale
        listProduct.mutate([product.id]);
      } else if (target === ProductStatus.OffShelf) {
        // OnSale→OffShelf
        delistProduct.mutate([product.id]);
      }
      // SoldOut is set automatically by chain when stock reaches 0
    },
    [product.id, listProduct, delistProduct],
  );

  return (
    <div className="rounded-lg border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900">
      <div className="aspect-square overflow-hidden rounded-t-lg">
        <IpfsImage cid={product.imageCid} alt="商品图片" className="h-full w-full" />
      </div>
      <div className="p-4">
        <div className="flex items-start justify-between">
          <p className="truncate text-xs text-gray-500">ID: {product.id}</p>
          <span className={`inline-flex rounded-full px-2 py-0.5 text-xs font-medium ${statusCfg.color}`}>
            {statusCfg.label}
          </span>
        </div>
        <div className="mt-2 space-y-1 text-sm">
          <p className="font-semibold text-gray-900 dark:text-gray-100">
            {formatNexBalance(product.priceNex)} NEX
          </p>
          {product.priceUsdt > 0 && (
            <p className="text-gray-500">${formatUsdtPrice(product.priceUsdt)} USDT</p>
          )}
        </div>
        <div className="mt-2 flex flex-wrap gap-1 text-xs text-gray-500">
          <span>{CATEGORY_LABELS[product.category]}</span>
          <span>·</span>
          <span>{VISIBILITY_LABELS[product.visibility]}</span>
          {product.visibility === ProductVisibility.LevelGated && product.levelGate != null && (
            <>
              <span>·</span>
              <span>等级≥{product.levelGate}</span>
            </>
          )}
          <span>·</span>
          <span>库存: {product.stock === 0 ? '无限' : product.stock}</span>
        </div>
        {product.minQuantity > 1 || product.maxQuantity > 0 ? (
          <p className="mt-1 text-xs text-gray-400">
            购买量: {product.minQuantity}
            {product.maxQuantity > 0 ? ` ~ ${product.maxQuantity}` : '+'}
          </p>
        ) : null}

        {/* Status transition action buttons */}
        {canAct && validTransitions.length > 0 && (
          <PermissionGuard required={AdminPermission.SHOP_MANAGE} fallback={null}>
            <div className="mt-3 flex flex-wrap gap-2">
              {validTransitions
                .filter((t) => t !== ProductStatus.SoldOut) // SoldOut is chain-automatic
                .map((target) => {
                  const action = TRANSITION_ACTION[target];
                  return (
                    <button
                      key={target}
                      onClick={() => handleTransition(target)}
                      className={`rounded-md px-3 py-1 text-xs font-medium ${action.color}`}
                    >
                      {action.label}
                    </button>
                  );
                })}
            </div>
          </PermissionGuard>
        )}
      </div>
    </div>
  );
}

// ─── Create Product Form ────────────────────────────────────

function CreateProductForm({ shopId }: { shopId: number }) {
  const { createProduct } = useProducts(shopId);

  const [form, setForm] = useState({
    nameCid: '',
    imageCid: '',
    detailCid: '',
    priceNex: '',
    priceUsdt: '0',
    stock: '0',
    category: ProductCategory.Digital as ProductCategory,
    visibility: ProductVisibility.Public as ProductVisibility,
    levelGate: '',
    minQuantity: '1',
    maxQuantity: '0',
  });
  const [errors, setErrors] = useState<Record<string, string>>({});

  const setField = useCallback((key: string, value: string) => {
    setForm((prev) => ({ ...prev, [key]: value }));
    setErrors((prev) => {
      const next = { ...prev };
      delete next[key];
      return next;
    });
  }, []);

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      const result = createProductSchema.safeParse(form);
      if (!result.success) {
        const fieldErrors: Record<string, string> = {};
        for (const issue of result.error.issues) {
          const key = issue.path[0];
          if (key) fieldErrors[String(key)] = issue.message;
        }
        setErrors(fieldErrors);
        return;
      }

      // Convert NEX price to chain balance (12 decimals)
      const nexParts = form.priceNex.split('.');
      const nexWhole = nexParts[0] ?? '0';
      const nexFrac = (nexParts[1] ?? '').padEnd(12, '0').slice(0, 12);
      const rawNex = BigInt(nexWhole) * BigInt('1000000000000') + BigInt(nexFrac);

      // Convert USDT price to u64 (10^6 precision)
      const usdtVal = Math.round(Number(form.priceUsdt) * 1_000_000);

      const levelGate = form.visibility === ProductVisibility.LevelGated && form.levelGate
        ? Number(form.levelGate)
        : null;

      createProduct.mutate([
        shopId,
        form.nameCid,
        form.imageCid,
        form.detailCid,
        rawNex.toString(),
        usdtVal,
        Number(form.stock),
        form.category,
        form.visibility,
        levelGate,
        Number(form.minQuantity),
        Number(form.maxQuantity),
      ]);
    },
    [form, shopId, createProduct],
  );

  return (
    <section className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
      <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">创建商品</h2>
      <form onSubmit={handleSubmit} className="space-y-4">
        {/* IPFS uploads */}
        <IpfsUploadField
          id="product-name"
          label="商品名称 (IPFS)"
          value={form.nameCid}
          onChange={(cid) => setField('nameCid', cid)}
          error={errors.nameCid}
        />
        <IpfsUploadField
          id="product-image"
          label="商品图片 (IPFS)"
          value={form.imageCid}
          onChange={(cid) => setField('imageCid', cid)}
          accept="image/*"
          error={errors.imageCid}
        />
        <IpfsUploadField
          id="product-detail"
          label="商品详情 (IPFS)"
          value={form.detailCid}
          onChange={(cid) => setField('detailCid', cid)}
          error={errors.detailCid}
        />

        {/* Pricing */}
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <div>
            <label htmlFor="price-nex" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              NEX 价格
            </label>
            <input
              id="price-nex"
              type="text"
              inputMode="decimal"
              value={form.priceNex}
              onChange={(e) => setField('priceNex', e.target.value)}
              placeholder="必须 > 0"
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            />
            {errors.priceNex && <p className="mt-1 text-xs text-red-500">{errors.priceNex}</p>}
          </div>
          <div>
            <label htmlFor="price-usdt" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              USDT 价格 (0=不支持)
            </label>
            <input
              id="price-usdt"
              type="text"
              inputMode="decimal"
              value={form.priceUsdt}
              onChange={(e) => setField('priceUsdt', e.target.value)}
              placeholder="0"
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            />
            {errors.priceUsdt && <p className="mt-1 text-xs text-red-500">{errors.priceUsdt}</p>}
          </div>
        </div>

        {/* Stock & Category */}
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
          <div>
            <label htmlFor="stock" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              库存 (0=无限)
            </label>
            <input
              id="stock"
              type="number"
              min="0"
              value={form.stock}
              onChange={(e) => setField('stock', e.target.value)}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            />
            {errors.stock && <p className="mt-1 text-xs text-red-500">{errors.stock}</p>}
          </div>
          <div>
            <label htmlFor="category" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              类别
            </label>
            <select
              id="category"
              value={form.category}
              onChange={(e) => setField('category', e.target.value)}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            >
              {Object.values(ProductCategory).map((c) => (
                <option key={c} value={c}>{CATEGORY_LABELS[c]}</option>
              ))}
            </select>
          </div>
          <div>
            <label htmlFor="visibility" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              可见性
            </label>
            <select
              id="visibility"
              value={form.visibility}
              onChange={(e) => setField('visibility', e.target.value)}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            >
              {Object.values(ProductVisibility).map((v) => (
                <option key={v} value={v}>{VISIBILITY_LABELS[v]}</option>
              ))}
            </select>
          </div>
        </div>

        {/* Level gate (only when LevelGated) */}
        {form.visibility === ProductVisibility.LevelGated && (
          <div>
            <label htmlFor="level-gate" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              等级门槛
            </label>
            <input
              id="level-gate"
              type="number"
              min="1"
              value={form.levelGate}
              onChange={(e) => setField('levelGate', e.target.value)}
              placeholder="最低会员等级"
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            />
          </div>
        )}

        {/* Purchase quantity */}
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <div>
            <label htmlFor="min-qty" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              最小购买量
            </label>
            <input
              id="min-qty"
              type="number"
              min="1"
              value={form.minQuantity}
              onChange={(e) => setField('minQuantity', e.target.value)}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            />
            {errors.minQuantity && <p className="mt-1 text-xs text-red-500">{errors.minQuantity}</p>}
          </div>
          <div>
            <label htmlFor="max-qty" className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              最大购买量 (0=不限)
            </label>
            <input
              id="max-qty"
              type="number"
              min="0"
              value={form.maxQuantity}
              onChange={(e) => setField('maxQuantity', e.target.value)}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            />
            {errors.maxQuantity && <p className="mt-1 text-xs text-red-500">{errors.maxQuantity}</p>}
          </div>
        </div>

        <div className="flex items-center gap-3">
          <button
            type="submit"
            disabled={createProduct.txState.status === 'signing' || createProduct.txState.status === 'broadcasting'}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            创建商品
          </button>
          <TxStatusIndicator txState={createProduct.txState} />
        </div>
      </form>
    </section>
  );
}

// ─── Page ───────────────────────────────────────────────────

export default function ProductsPage() {
  const t = useTranslations('shops');
  const params = useParams();
  const shopId = Number(params.shopId);
  const { isReadOnly, isSuspended } = useEntityContext();
  const { products, isLoading, error } = useProducts(shopId);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center p-12 text-sm text-gray-500">
        {t('products.loading')}
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

  return (
    <div className="mx-auto max-w-4xl space-y-6 p-4 sm:p-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">{t('products.title')}</h1>

      {/* Create product form — requires SHOP_MANAGE */}
      {!isReadOnly && !isSuspended && (
        <PermissionGuard required={AdminPermission.SHOP_MANAGE} fallback={null}>
          <CreateProductForm shopId={shopId} />
        </PermissionGuard>
      )}

      {/* Product list */}
      {products.length === 0 ? (
        <div className="rounded-lg border border-dashed border-gray-300 p-8 text-center text-sm text-gray-500 dark:border-gray-600">
          暂无商品
        </div>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {products.map((product) => (
            <ProductCard key={product.id} product={product} shopId={shopId} />
          ))}
        </div>
      )}
    </div>
  );
}
