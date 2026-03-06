"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { ProductData } from "@/lib/types";

export function useProducts(shopId: number | null) {
  const [products, setProducts] = useState<ProductData[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (shopId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const entries = await (api.query as any).entityProduct.shopProducts.entries(shopId);
      const results = entries.map(([_k, v]: [unknown, { toJSON: () => ProductData }]) => v.toJSON());
      setProducts(results);
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [shopId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { products, isLoading, refetch: fetch };
}

export function useProduct(productId: number | null) {
  const [product, setProduct] = useState<ProductData | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (productId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const raw = await (api.query as any).entityProduct.products(productId);
      if (raw.isNone) { setProduct(null); } else { setProduct(raw.toJSON() as unknown as ProductData); }
    } catch { /* ignore */ } finally { setIsLoading(false); }
  }, [productId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { product, isLoading, refetch: fetch };
}

export function useProductActions() {
  const { submit, state, reset } = useTx();
  return {
    createProduct: (
      shopId: number,
      nameCid: string,
      imagesCid: string,
      detailCid: string,
      price: bigint,
      usdtPrice: number,
      stock: number,
      category: string,
      visibility: string,
      minOrderQty: number,
      maxOrderQty: number,
    ) =>
      submit("entityProduct", "createProduct", [
        shopId, nameCid, imagesCid, detailCid,
        price, usdtPrice, stock,
        category, visibility,
        minOrderQty, maxOrderQty,
      ]),
    updateProduct: (
      productId: number,
      nameCid?: string,
      imagesCid?: string,
      detailCid?: string,
      price?: bigint,
      stock?: number,
    ) =>
      submit("entityProduct", "updateProduct", [productId, nameCid, imagesCid, detailCid, price, stock]),
    setProductVisibility: (productId: number, visibility: string) =>
      submit("entityProduct", "setProductVisibility", [productId, visibility]),
    setProductCategory: (productId: number, category: string) =>
      submit("entityProduct", "setProductCategory", [productId, category]),
    activateProduct: (productId: number) => submit("entityProduct", "activateProduct", [productId]),
    deactivateProduct: (productId: number) => submit("entityProduct", "deactivateProduct", [productId]),
    deleteProduct: (productId: number) => submit("entityProduct", "deleteProduct", [productId]),
    txState: state,
    resetTx: reset,
  };
}
