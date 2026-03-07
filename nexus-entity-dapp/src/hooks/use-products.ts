'use client';

import { useEntityQuery } from './use-entity-query';
import { useEntityMutation } from './use-entity-mutation';
import { useEntityContext } from '@/app/[entityId]/entity-provider';
import { STALE_TIMES } from '@/lib/chain/constants';
import type { ProductData } from '@/lib/types/models';
import { ProductCategory, ProductStatus, ProductVisibility } from '@/lib/types/enums';

// ─── Parser ─────────────────────────────────────────────────

function parseProductData(raw: unknown): ProductData | null {
  if (!raw || (raw as { isNone?: boolean }).isNone) return null;
  const unwrapped = (raw as { unwrapOr?: (d: null) => unknown }).unwrapOr?.(null) ?? raw;
  if (!unwrapped) return null;
  const obj = unwrapped as Record<string, unknown>;

  return {
    id: Number(obj.id ?? 0),
    shopId: Number(obj.shopId ?? 0),
    nameCid: String(obj.nameCid ?? ''),
    imageCid: String(obj.imageCid ?? ''),
    detailCid: String(obj.detailCid ?? ''),
    priceNex: BigInt(String(obj.priceNex ?? 0)),
    priceUsdt: Number(obj.priceUsdt ?? 0),
    stock: Number(obj.stock ?? 0),
    category: String(obj.category ?? 'Other') as ProductCategory,
    visibility: String(obj.visibility ?? 'Public') as ProductVisibility,
    levelGate: obj.levelGate != null ? Number(obj.levelGate) : null,
    status: String(obj.status ?? 'Draft') as ProductStatus,
    minQuantity: Number(obj.minQuantity ?? 1),
    maxQuantity: Number(obj.maxQuantity ?? 0),
  };
}

// ─── Hook ───────────────────────────────────────────────────

export function useProducts(shopId: number) {
  const { entityId } = useEntityContext();

  // Query product IDs for this shop
  const productIdsQuery = useEntityQuery<number[]>(
    ['entity', entityId, 'shop', shopId, 'products'],
    async (api) => {
      const raw = await (api.query as any).entityProduct.shopProducts(shopId);
      if (!raw) return [];
      const arr = raw.toJSON?.() ?? raw;
      return Array.isArray(arr) ? arr.map(Number) : [];
    },
    { staleTime: STALE_TIMES.products, enabled: shopId > 0 },
  );

  // Query full product data
  const productsQuery = useEntityQuery<ProductData[]>(
    ['entity', entityId, 'shop', shopId, 'products', 'data', productIdsQuery.data],
    async (api) => {
      const ids = productIdsQuery.data;
      if (!ids || ids.length === 0) return [];

      const results = await Promise.all(
        ids.map((id) => (api.query as any).entityProduct.products(id)),
      );

      return results
        .map((raw) => parseProductData(raw))
        .filter((p): p is ProductData => p !== null);
    },
    {
      staleTime: STALE_TIMES.products,
      enabled: !!productIdsQuery.data && productIdsQuery.data.length > 0,
    },
  );

  // Mutations
  const createProduct = useEntityMutation('entityProduct', 'createProduct', {
    invalidateKeys: [['entity', entityId, 'shop', shopId, 'products']],
  });

  const updateProduct = useEntityMutation('entityProduct', 'updateProduct', {
    invalidateKeys: [['entity', entityId, 'shop', shopId, 'products']],
  });

  const listProduct = useEntityMutation('entityProduct', 'listProduct', {
    invalidateKeys: [['entity', entityId, 'shop', shopId, 'products']],
  });

  const delistProduct = useEntityMutation('entityProduct', 'delistProduct', {
    invalidateKeys: [['entity', entityId, 'shop', shopId, 'products']],
  });

  return {
    products: productsQuery.data ?? [],
    isLoading: productIdsQuery.isLoading || productsQuery.isLoading,
    error: productIdsQuery.error || productsQuery.error,
    createProduct,
    updateProduct,
    listProduct,
    delistProduct,
  };
}
