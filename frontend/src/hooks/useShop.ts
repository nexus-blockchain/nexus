"use client";

import { useState, useEffect, useCallback } from "react";
import { getApi } from "./useApi";
import { useTx } from "./useTx";
import type { ShopData } from "@/lib/types";

export function useShops(entityId: number | null) {
  const [shops, setShops] = useState<ShopData[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetch = useCallback(async () => {
    if (entityId === null) return;
    setIsLoading(true);
    try {
      const api = await getApi();
      const ids = await (api.query as any).entityShop.entityShops(entityId);
      const idList = (ids.toJSON() || []) as number[];
      const results = await Promise.all(
        idList.map(async (id: number) => {
          const raw = await (api.query as any).entityShop.shops(id);
          return raw.toJSON() as ShopData;
        })
      );
      setShops(results.filter(Boolean));
    } catch {
      // ignore
    } finally {
      setIsLoading(false);
    }
  }, [entityId]);

  useEffect(() => { fetch(); }, [fetch]);
  return { shops, isLoading, refetch: fetch };
}

export function useShopActions() {
  const { submit, state, reset } = useTx();

  return {
    createShop: (entityId: number, name: string, shopType: string, initialFund: bigint) =>
      submit("entityShop", "createShop", [entityId, name, shopType, initialFund]),
    updateShop: (shopId: number, name?: string, logoCid?: string | null, descriptionCid?: string | null) =>
      submit("entityShop", "updateShop", [shopId, name, logoCid, descriptionCid]),
    pauseShop: (shopId: number) => submit("entityShop", "pauseShop", [shopId]),
    resumeShop: (shopId: number) => submit("entityShop", "resumeShop", [shopId]),
    closeShop: (shopId: number) => submit("entityShop", "closeShop", [shopId]),
    fundOperating: (shopId: number, amount: bigint) =>
      submit("entityShop", "fundOperating", [shopId, amount]),
    withdrawOperatingFund: (shopId: number, amount: bigint) =>
      submit("entityShop", "withdrawOperatingFund", [shopId, amount]),
    addManager: (shopId: number, manager: string) =>
      submit("entityShop", "addManager", [shopId, manager]),
    removeManager: (shopId: number, manager: string) =>
      submit("entityShop", "removeManager", [shopId, manager]),
    setLocation: (shopId: number, lat: number, lng: number, addressCid?: string, hoursCid?: string) =>
      submit("entityShop", "setLocation", [shopId, lat, lng, addressCid || null, hoursCid || null]),
    enablePoints: (shopId: number, name: string, symbol: string, rewardRate: number, exchangeRate: number, transferable: boolean) =>
      submit("entityShop", "enablePoints", [shopId, name, symbol, rewardRate, exchangeRate, transferable]),
    disablePoints: (shopId: number) => submit("entityShop", "disablePoints", [shopId]),
    updatePointsConfig: (shopId: number, rewardRate: number, exchangeRate: number, transferable: boolean) =>
      submit("entityShop", "updatePointsConfig", [shopId, rewardRate, exchangeRate, transferable]),
    transferPoints: (shopId: number, to: string, amount: bigint) =>
      submit("entityShop", "transferPoints", [shopId, to, amount]),
    txState: state,
    resetTx: reset,
  };
}
