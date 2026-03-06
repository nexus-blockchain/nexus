import { create } from "zustand";

interface TradingState {
  selectedOrderId: number | null;
  setSelectedOrderId: (id: number | null) => void;
  selectedTradeId: number | null;
  setSelectedTradeId: (id: number | null) => void;
  orderSideFilter: "all" | "Sell" | "Buy";
  setOrderSideFilter: (side: "all" | "Sell" | "Buy") => void;
  marketPaused: boolean;
  setMarketPaused: (paused: boolean) => void;
}

export const useTradingStore = create<TradingState>((set) => ({
  selectedOrderId: null,
  setSelectedOrderId: (id) => set({ selectedOrderId: id }),
  selectedTradeId: null,
  setSelectedTradeId: (id) => set({ selectedTradeId: id }),
  orderSideFilter: "all",
  setOrderSideFilter: (side) => set({ orderSideFilter: side }),
  marketPaused: false,
  setMarketPaused: (paused) => set({ marketPaused: paused }),
}));
